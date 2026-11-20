//! Deterministic Hybrid Logical Clock (`HlcTimestamp`) & Fine-Grained Per-Cell CRDT
//! Co-Editing Engine for Offline-First & Real-Time Multi-Peer Visio Collaboration.
//!
//! Design properties:
//! - **Total Ordering (`HlcTimestamp`)**: `(wall_ms, logical, replica_id)` forms a strict
//!   total order across arbitrary offline/online peers without centralized coordination.
//! - **Add-Wins / LWW-Element-Set Shape Lifecycle**: Per-shape birth and tombstone timestamps
//!   guarantee out-of-order `UpsertShape`, `TombstoneShape`, and `SetCell` convergence.
//! - **Per-Cell LWW-Register (`SetCell`) & Text Register (`SetShapeText`)**: Concurrent edits
//!   to distinct cells (e.g. `FillForegnd` vs `PinX` + `Text`) on the *same* shape merge with
//!   zero lost updates regardless of network arrival order (`[A, B]` == `[B, A]`), while
//!   honoring Visio `GUARD(...)` protection unless `force_setf` is set.

use crate::domains::build_all_builtin_stencils;
use crate::routing::route_page_connectors;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use vexio_shapesheet::{
    GlueEndpoint, ReactiveEngine, ShapeDataRow, ShapeDataType, ShapeKind, Unit, VisioDocument,
    VisioPage, VisioShape, canonical_cell_name, format_clean_f64,
};

/// Hybrid Logical Clock timestamp providing a deterministic global total order across peers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HlcTimestamp {
    pub wall_ms: u64,
    pub logical: u32,
    pub replica_id: String,
}

impl HlcTimestamp {
    pub fn zero() -> Self {
        Self {
            wall_ms: 0,
            logical: 0,
            replica_id: String::new(),
        }
    }
}

impl Ord for HlcTimestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.wall_ms
            .cmp(&other.wall_ms)
            .then_with(|| self.logical.cmp(&other.logical))
            .then_with(|| self.replica_id.cmp(&other.replica_id))
    }
}

impl PartialOrd for HlcTimestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Fine-grained CRDT operation replicated over WebSockets, WebRTC, or Firestore offline sync.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CrdtOp {
    UpsertPage {
        ts: HlcTimestamp,
        page_id: u32,
        name: String,
        width_in: f64,
        height_in: f64,
    },
    UpsertShape {
        ts: HlcTimestamp,
        page_id: u32,
        shape_id: u64,
        name: String,
        kind: ShapeKind,
        master_info: Option<(String, String)>,
    },
    TombstoneShape {
        ts: HlcTimestamp,
        page_id: u32,
        shape_id: u64,
    },
    SetCell {
        ts: HlcTimestamp,
        page_id: u32,
        shape_id: u64,
        cell_name: String,
        formula: String,
        force_setf: bool,
    },
    SetShapeText {
        ts: HlcTimestamp,
        page_id: u32,
        shape_id: u64,
        text: String,
    },
    UpsertConnect {
        ts: HlcTimestamp,
        page_id: u32,
        connector_id: u64,
        from_id: u64,
        to_id: u64,
    },
}

impl CrdtOp {
    pub fn timestamp(&self) -> &HlcTimestamp {
        match self {
            Self::UpsertPage { ts, .. }
            | Self::UpsertShape { ts, .. }
            | Self::TombstoneShape { ts, .. }
            | Self::SetCell { ts, .. }
            | Self::SetShapeText { ts, .. }
            | Self::UpsertConnect { ts, .. } => ts,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PageRegister {
    ts: HlcTimestamp,
    name: String,
    width_in: f64,
    height_in: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct ShapeLifecycle {
    birth_ts: Option<HlcTimestamp>,
    tombstone_ts: Option<HlcTimestamp>,
    name: String,
    kind: Option<ShapeKind>,
    master_info: Option<(String, String)>,
}

impl ShapeLifecycle {
    /// Add-Wins / LWW-Element-Set membership predicate:
    /// - If both `birth_ts` and `tombstone_ts` exist, alive iff `birth_ts >= tombstone_ts`.
    /// - If only `tombstone_ts` exists (deleting a baseline shape), dead (`false`).
    /// - Otherwise alive (`true`).
    fn is_alive(&self) -> bool {
        match (&self.birth_ts, &self.tombstone_ts) {
            (Some(b), Some(t)) => b >= t,
            (None, Some(_)) => false,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CellRegister {
    ts: HlcTimestamp,
    cell_name: String,
    formula: String,
    force_setf: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct TextRegister {
    ts: HlcTimestamp,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct ConnectRegister {
    ts: HlcTimestamp,
    from_id: u64,
    to_id: u64,
}

/// Replica-local HLC + CRDT metadata store tracking causal LWW registers for pages, shapes,
/// cells, text blocks, and dynamic connector bindings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrdtDocumentState {
    pub replica_id: String,
    pub max_wall_ms: u64,
    pub logical: u32,
    pages: BTreeMap<u32, PageRegister>,
    shapes: BTreeMap<(u32, u64), ShapeLifecycle>,
    cells: BTreeMap<(u32, u64, String), CellRegister>,
    texts: BTreeMap<(u32, u64), TextRegister>,
    connects: BTreeMap<(u32, u64), ConnectRegister>,
}

impl CrdtDocumentState {
    pub fn new(replica_id: impl Into<String>) -> Self {
        Self {
            replica_id: replica_id.into(),
            max_wall_ms: 0,
            logical: 0,
            pages: BTreeMap::new(),
            shapes: BTreeMap::new(),
            cells: BTreeMap::new(),
            texts: BTreeMap::new(),
            connects: BTreeMap::new(),
        }
    }

    /// Issue a strictly monotonic `HlcTimestamp` for a local mutation at `now_ms`.
    pub fn next_timestamp(&mut self, now_ms: u64) -> HlcTimestamp {
        if now_ms > self.max_wall_ms {
            self.max_wall_ms = now_ms;
            self.logical = 0;
        } else {
            self.logical = self.logical.saturating_add(1);
        }
        HlcTimestamp {
            wall_ms: self.max_wall_ms,
            logical: self.logical,
            replica_id: self.replica_id.clone(),
        }
    }

    fn observe_timestamp(&mut self, remote: &HlcTimestamp) {
        if remote.replica_id == self.replica_id {
            if remote.wall_ms > self.max_wall_ms {
                self.max_wall_ms = remote.wall_ms;
                self.logical = remote.logical;
            } else if remote.wall_ms == self.max_wall_ms && remote.logical > self.logical {
                self.logical = remote.logical;
            }
            return;
        }
        if remote.wall_ms > self.max_wall_ms {
            self.max_wall_ms = remote.wall_ms;
            self.logical = remote.logical.saturating_add(1);
        } else if remote.wall_ms == self.max_wall_ms {
            self.logical = self.logical.max(remote.logical).saturating_add(1);
        } else {
            self.logical = self.logical.saturating_add(1);
        }
    }

    /// Apply a single `CrdtOp` to `doc`.
    ///
    /// Guarantees idempotency, commutativity, and associativity:
    /// - Mutates a register iff `op.timestamp()` strictly dominates its current `HlcTimestamp`.
    /// - Respects `GUARD(...)` protection unless `force_setf` is `true`.
    /// - Triggers `ReactiveEngine::recalculate_document` and `route_page_connectors`.
    pub fn apply_op(&mut self, doc: &mut VisioDocument, op: CrdtOp) -> bool {
        self.observe_timestamp(op.timestamp());

        match op {
            CrdtOp::UpsertPage {
                ts,
                page_id,
                name,
                width_in,
                height_in,
            } => {
                if let Some(existing) = self.pages.get(&page_id) {
                    if ts <= existing.ts {
                        return false;
                    }
                }
                let reg = PageRegister {
                    ts,
                    name,
                    width_in,
                    height_in,
                };
                upsert_page_in_doc(doc, page_id, &reg);
                self.pages.insert(page_id, reg);
                self.replay_buffered_for_page(doc, page_id);
                ReactiveEngine::recalculate_document(doc);
                route_page_connectors(doc, page_id);
                true
            }

            CrdtOp::UpsertShape {
                ts,
                page_id,
                shape_id,
                name,
                kind,
                master_info,
            } => {
                let life = self.shapes.entry((page_id, shape_id)).or_default();
                if let Some(prev_birth) = &life.birth_ts {
                    if ts <= *prev_birth {
                        return false;
                    }
                }
                life.birth_ts = Some(ts);
                life.name = name.clone();
                life.kind = Some(kind);
                life.master_info = master_info.clone();
                let alive = life.is_alive();

                if !alive {
                    return false;
                }
                if doc.page(page_id).is_none() {
                    return true;
                }

                materialize_shape(doc, page_id, shape_id, &name, kind, master_info.as_ref());
                self.replay_buffered_for_shape(doc, page_id, shape_id);
                ReactiveEngine::recalculate_document(doc);
                route_page_connectors(doc, page_id);
                true
            }

            CrdtOp::TombstoneShape {
                ts,
                page_id,
                shape_id,
            } => {
                let life = self.shapes.entry((page_id, shape_id)).or_default();
                if let Some(prev_tomb) = &life.tombstone_ts {
                    if ts <= *prev_tomb {
                        return false;
                    }
                }
                life.tombstone_ts = Some(ts);
                if life.is_alive() {
                    return false;
                }
                let mut removed = false;
                if let Some(page) = doc.page_mut(page_id) {
                    let before_len = page.shapes.len();
                    page.shapes.retain(|s| s.id != shape_id);
                    for parent in &mut page.shapes {
                        parent.children.retain(|c| c.id != shape_id);
                    }
                    page.connects.retain(|c| {
                        c.connector_shape_id != shape_id && c.target_shape_id != shape_id
                    });
                    removed = page.shapes.len() != before_len;
                }
                if removed {
                    ReactiveEngine::recalculate_document(doc);
                    route_page_connectors(doc, page_id);
                }
                true
            }

            CrdtOp::SetCell {
                ts,
                page_id,
                shape_id,
                cell_name,
                formula,
                force_setf,
            } => {
                let key = canonical_cell_name(&cell_name);
                if key.is_empty() {
                    return false;
                }
                let reg_key = (page_id, shape_id, key.clone());
                if let Some(existing) = self.cells.get(&reg_key) {
                    if ts <= existing.ts {
                        return false;
                    }
                }

                // Check if shape is tombstoned: buffer the LWW cell value without resurrecting!
                let alive = self
                    .shapes
                    .get(&(page_id, shape_id))
                    .map(ShapeLifecycle::is_alive)
                    .unwrap_or(true);

                if !alive || doc.page(page_id).and_then(|p| p.find_shape(shape_id)).is_none() {
                    // Check GUARD if the shape doesn't exist yet? No cell exists yet, so buffer it
                    // unless tombstoned.
                    self.cells.insert(
                        reg_key,
                        CellRegister {
                            ts,
                            cell_name,
                            formula,
                            force_setf,
                        },
                    );
                    return alive;
                }

                // Shape is present in `doc`: enforce Visio GUARD semantics before committing register!
                let applied = if let Some(sh) =
                    doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id))
                {
                    apply_cell_formula_to_shape(sh, &key, &cell_name, &formula, force_setf)
                } else {
                    false
                };

                if !applied {
                    return false;
                }

                self.cells.insert(
                    reg_key,
                    CellRegister {
                        ts,
                        cell_name,
                        formula,
                        force_setf,
                    },
                );
                ReactiveEngine::recalculate_document(doc);
                route_page_connectors(doc, page_id);
                true
            }

            CrdtOp::SetShapeText {
                ts,
                page_id,
                shape_id,
                text,
            } => {
                if let Some(existing) = self.texts.get(&(page_id, shape_id)) {
                    if ts <= existing.ts {
                        return false;
                    }
                }
                self.texts.insert(
                    (page_id, shape_id),
                    TextRegister {
                        ts,
                        text: text.clone(),
                    },
                );

                let alive = self
                    .shapes
                    .get(&(page_id, shape_id))
                    .map(ShapeLifecycle::is_alive)
                    .unwrap_or(true);
                if !alive {
                    return false;
                }

                if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                    sh.text = text;
                    ReactiveEngine::recalculate_document(doc);
                    route_page_connectors(doc, page_id);
                }
                true
            }

            CrdtOp::UpsertConnect {
                ts,
                page_id,
                connector_id,
                from_id,
                to_id,
            } => {
                if let Some(existing) = self.connects.get(&(page_id, connector_id)) {
                    if ts <= existing.ts {
                        return false;
                    }
                }
                self.connects.insert(
                    (page_id, connector_id),
                    ConnectRegister {
                        ts: ts.clone(),
                        from_id,
                        to_id,
                    },
                );

                let conn_alive = self
                    .shapes
                    .get(&(page_id, connector_id))
                    .map(ShapeLifecycle::is_alive)
                    .unwrap_or(true);
                let from_alive = self
                    .shapes
                    .get(&(page_id, from_id))
                    .map(ShapeLifecycle::is_alive)
                    .unwrap_or(true);
                let to_alive = self
                    .shapes
                    .get(&(page_id, to_id))
                    .map(ShapeLifecycle::is_alive)
                    .unwrap_or(true);

                if !conn_alive || !from_alive || !to_alive {
                    return false;
                }

                if doc.page(page_id).is_some() {
                    materialize_connect(doc, page_id, connector_id, from_id, to_id);
                    ReactiveEngine::recalculate_document(doc);
                    route_page_connectors(doc, page_id);
                }
                true
            }
        }
    }

    /// Parse and apply a JSON array of `CrdtOp` operations, returning how many ops advanced state.
    pub fn apply_batch_json(
        &mut self,
        doc: &mut VisioDocument,
        ops_json: &str,
    ) -> Result<usize, serde_json::Error> {
        let ops: Vec<CrdtOp> = serde_json::from_str(ops_json)?;
        let mut applied_count = 0usize;
        for op in ops {
            if self.apply_op(doc, op) {
                applied_count += 1;
            }
        }
        Ok(applied_count)
    }

    /// Compute minimal fine-grained per-cell / per-shape CRDT delta operations transforming
    /// `prev` into `curr`, advancing and recording this replica's causal HLC registers.
    pub fn diff_to_ops(
        &mut self,
        prev: &VisioDocument,
        curr: &VisioDocument,
        now_ms: u64,
    ) -> Vec<CrdtOp> {
        let mut ops = Vec::new();

        // 1. Page additions & page dimension/title updates
        let prev_pages: BTreeMap<u32, &VisioPage> = prev.pages.iter().map(|p| (p.id, p)).collect();
        for cp in &curr.pages {
            let needs_upsert = match prev_pages.get(&cp.id) {
                None => true,
                Some(pp) => {
                    pp.name != cp.name
                        || (pp.width_in() - cp.width_in()).abs() > 1e-6
                        || (pp.height_in() - cp.height_in()).abs() > 1e-6
                }
            };
            if needs_upsert {
                let ts = self.next_timestamp(now_ms);
                self.pages.insert(
                    cp.id,
                    PageRegister {
                        ts: ts.clone(),
                        name: cp.name.clone(),
                        width_in: cp.width_in(),
                        height_in: cp.height_in(),
                    },
                );
                ops.push(CrdtOp::UpsertPage {
                    ts,
                    page_id: cp.id,
                    name: cp.name.clone(),
                    width_in: cp.width_in(),
                    height_in: cp.height_in(),
                });
            }

            let pp_opt = prev_pages.get(&cp.id).copied();
            let prev_shapes: BTreeMap<u64, &VisioShape> = pp_opt
                .map(|pp| pp.shapes.iter().map(|s| (s.id, s)).collect())
                .unwrap_or_default();
            let curr_shapes: BTreeMap<u64, &VisioShape> =
                cp.shapes.iter().map(|s| (s.id, s)).collect();

            // 2. Tombstoned shapes
            for &sid in prev_shapes.keys() {
                if !curr_shapes.contains_key(&sid) {
                    let ts = self.next_timestamp(now_ms);
                    let life = self.shapes.entry((cp.id, sid)).or_default();
                    life.tombstone_ts = Some(ts.clone());
                    ops.push(CrdtOp::TombstoneShape {
                        ts,
                        page_id: cp.id,
                        shape_id: sid,
                    });
                }
            }

            // 3. Added and modified shapes
            for (&sid, &csh) in &curr_shapes {
                match prev_shapes.get(&sid) {
                    None => {
                        let master_info = infer_master_info(curr, csh);
                        let ts_birth = self.next_timestamp(now_ms);
                        self.shapes.insert(
                            (cp.id, sid),
                            ShapeLifecycle {
                                birth_ts: Some(ts_birth.clone()),
                                tombstone_ts: None,
                                name: csh.name.clone(),
                                kind: Some(csh.kind),
                                master_info: master_info.clone(),
                            },
                        );
                        ops.push(CrdtOp::UpsertShape {
                            ts: ts_birth,
                            page_id: cp.id,
                            shape_id: sid,
                            name: csh.name.clone(),
                            kind: csh.kind,
                            master_info: master_info.clone(),
                        });

                        let baseline_template =
                            build_template_shape(curr, sid, &csh.name, csh.kind, master_info.as_ref());

                        if csh.text != baseline_template.text {
                            let ts_txt = self.next_timestamp(now_ms);
                            self.texts.insert(
                                (cp.id, sid),
                                TextRegister {
                                    ts: ts_txt.clone(),
                                    text: csh.text.clone(),
                                },
                            );
                            ops.push(CrdtOp::SetShapeText {
                                ts: ts_txt,
                                page_id: cp.id,
                                shape_id: sid,
                                text: csh.text.clone(),
                            });
                        }

                        for (ckey, ccell) in &csh.sheet.cells {
                            let differs = baseline_template
                                .sheet
                                .cells
                                .get(ckey)
                                .map(|b| b.formula != ccell.formula)
                                .unwrap_or(true);
                            if differs {
                                let force_setf = ccell.is_guarded()
                                    || baseline_template
                                        .sheet
                                        .cells
                                        .get(ckey)
                                        .map(|b| b.is_guarded())
                                        .unwrap_or(false);
                                let ts_cell = self.next_timestamp(now_ms);
                                self.cells.insert(
                                    (cp.id, sid, ckey.clone()),
                                    CellRegister {
                                        ts: ts_cell.clone(),
                                        cell_name: ccell.name.clone(),
                                        formula: ccell.formula.clone(),
                                        force_setf,
                                    },
                                );
                                ops.push(CrdtOp::SetCell {
                                    ts: ts_cell,
                                    page_id: cp.id,
                                    shape_id: sid,
                                    cell_name: ccell.name.clone(),
                                    formula: ccell.formula.clone(),
                                    force_setf,
                                });
                            }
                        }
                    }
                    Some(&psh) => {
                        if psh.text != csh.text {
                            let ts_txt = self.next_timestamp(now_ms);
                            self.texts.insert(
                                (cp.id, sid),
                                TextRegister {
                                    ts: ts_txt.clone(),
                                    text: csh.text.clone(),
                                },
                            );
                            ops.push(CrdtOp::SetShapeText {
                                ts: ts_txt,
                                page_id: cp.id,
                                shape_id: sid,
                                text: csh.text.clone(),
                            });
                        }

                        for (ckey, ccell) in &csh.sheet.cells {
                            let prev_cell = psh.sheet.cells.get(ckey);
                            let formula_changed =
                                prev_cell.map(|p| p.formula != ccell.formula).unwrap_or(true);
                            if formula_changed {
                                // Skip auto-routed transform & Geometry1 cells on glued 1D connectors,
                                // since `route_page_connectors` recomputes them deterministically.
                                if csh.kind == ShapeKind::Connector1D
                                    && is_routed_connector_derived_cell(ckey)
                                    && cp.connects.iter().any(|g| g.connector_shape_id == sid)
                                {
                                    continue;
                                }
                                let force_setf = ccell.is_guarded()
                                    || prev_cell.map(|p| p.is_guarded()).unwrap_or(false);
                                let ts_cell = self.next_timestamp(now_ms);
                                self.cells.insert(
                                    (cp.id, sid, ckey.clone()),
                                    CellRegister {
                                        ts: ts_cell.clone(),
                                        cell_name: ccell.name.clone(),
                                        formula: ccell.formula.clone(),
                                        force_setf,
                                    },
                                );
                                ops.push(CrdtOp::SetCell {
                                    ts: ts_cell,
                                    page_id: cp.id,
                                    shape_id: sid,
                                    cell_name: ccell.name.clone(),
                                    formula: ccell.formula.clone(),
                                    force_setf,
                                });
                            }
                        }
                    }
                }
            }

            // 4. Connector endpoint pairs `(connector_id -> (from_id, to_id))`
            let prev_conns = pp_opt.map(extract_connect_pairs).unwrap_or_default();
            let curr_conns = extract_connect_pairs(cp);
            for (conn_id, (from_id, to_id)) in curr_conns {
                if prev_conns.get(&conn_id) != Some(&(from_id, to_id)) {
                    let ts_conn = self.next_timestamp(now_ms);
                    self.connects.insert(
                        (cp.id, conn_id),
                        ConnectRegister {
                            ts: ts_conn.clone(),
                            from_id,
                            to_id,
                        },
                    );
                    ops.push(CrdtOp::UpsertConnect {
                        ts: ts_conn,
                        page_id: cp.id,
                        connector_id: conn_id,
                        from_id,
                        to_id,
                    });
                }
            }
        }

        ops
    }

    fn replay_buffered_for_page(&self, doc: &mut VisioDocument, page_id: u32) {
        let shape_ids: Vec<u64> = self
            .shapes
            .iter()
            .filter_map(|(&(pid, sid), life)| {
                if pid == page_id && life.is_alive() && life.birth_ts.is_some() {
                    Some(sid)
                } else {
                    None
                }
            })
            .collect();

        for sid in shape_ids {
            if let Some(life) = self.shapes.get(&(page_id, sid)) {
                let kind = life.kind.unwrap_or(ShapeKind::Shape2D);
                materialize_shape(
                    doc,
                    page_id,
                    sid,
                    &life.name,
                    kind,
                    life.master_info.as_ref(),
                );
                self.replay_buffered_for_shape(doc, page_id, sid);
            }
        }

        for (&(pid, cid), creg) in &self.connects {
            if pid == page_id {
                let alive = self
                    .shapes
                    .get(&(page_id, cid))
                    .map(ShapeLifecycle::is_alive)
                    .unwrap_or(true);
                if alive {
                    materialize_connect(doc, page_id, cid, creg.from_id, creg.to_id);
                }
            }
        }
    }

    fn replay_buffered_for_shape(&self, doc: &mut VisioDocument, page_id: u32, shape_id: u64) {
        if let Some(treg) = self.texts.get(&(page_id, shape_id)) {
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                sh.text = treg.text.clone();
            }
        }

        let start = (page_id, shape_id, String::new());
        for ((pid, sid, ckey), creg) in self.cells.range(start..) {
            if *pid != page_id || *sid != shape_id {
                break;
            }
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                let _ = apply_cell_formula_to_shape(
                    sh,
                    ckey,
                    &creg.cell_name,
                    &creg.formula,
                    creg.force_setf,
                );
            }
        }

        // Replay any buffered connects involving this shape
        let related_conns: Vec<(u64, u64, u64)> = self
            .connects
            .iter()
            .filter_map(|(&(pid, cid), creg)| {
                if pid == page_id
                    && (cid == shape_id || creg.from_id == shape_id || creg.to_id == shape_id)
                {
                    let c_alive = self
                        .shapes
                        .get(&(page_id, cid))
                        .map(ShapeLifecycle::is_alive)
                        .unwrap_or(true);
                    let f_alive = self
                        .shapes
                        .get(&(page_id, creg.from_id))
                        .map(ShapeLifecycle::is_alive)
                        .unwrap_or(true);
                    let t_alive = self
                        .shapes
                        .get(&(page_id, creg.to_id))
                        .map(ShapeLifecycle::is_alive)
                        .unwrap_or(true);
                    if c_alive && f_alive && t_alive {
                        return Some((cid, creg.from_id, creg.to_id));
                    }
                }
                None
            })
            .collect();

        for (cid, from_id, to_id) in related_conns {
            materialize_connect(doc, page_id, cid, from_id, to_id);
        }
    }
}

fn is_routed_connector_derived_cell(ckey: &str) -> bool {
    ckey.starts_with("geometry1.")
        || matches!(
            ckey,
            "beginx"
                | "beginy"
                | "endx"
                | "endy"
                | "pinx"
                | "piny"
                | "locpinx"
                | "locpiny"
                | "width"
                | "height"
                | "angle"
        )
}

fn upsert_page_in_doc(doc: &mut VisioDocument, page_id: u32, reg: &PageRegister) {
    if let Some(page) = doc.page_mut(page_id) {
        page.name = reg.name.clone();
        page.name_u = reg.name.clone();
        page.page_sheet.set_formula(
            "PageWidth",
            format!("{} in", format_clean_f64(reg.width_in)),
            Some(Unit::Inch),
        );
        page.page_sheet.set_formula(
            "PageHeight",
            format!("{} in", format_clean_f64(reg.height_in)),
            Some(Unit::Inch),
        );
    } else {
        let new_page = VisioPage::new(page_id, &reg.name, reg.width_in, reg.height_in);
        let pos = doc
            .pages
            .iter()
            .position(|p| p.id > page_id)
            .unwrap_or(doc.pages.len());
        doc.pages.insert(pos, new_page);
    }
}

fn build_template_shape(
    doc: &VisioDocument,
    shape_id: u64,
    name: &str,
    kind: ShapeKind,
    master_info: Option<&(String, String)>,
) -> VisioShape {
    let mut shape = if let Some((cat, mname)) = master_info {
        doc.masters
            .iter()
            .find(|m| m.name.eq_ignore_ascii_case(mname) || m.name_u.eq_ignore_ascii_case(mname))
            .and_then(|m| m.shapes.first())
            .cloned()
            .or_else(|| {
                build_all_builtin_stencils()
                    .into_iter()
                    .find(|p| {
                        p.title.eq_ignore_ascii_case(cat) || p.category.eq_ignore_ascii_case(cat)
                    })
                    .and_then(|p| {
                        p.masters
                            .into_iter()
                            .find(|m| m.name.eq_ignore_ascii_case(mname))
                    })
                    .and_then(|m| m.shapes.into_iter().next())
            })
            .unwrap_or_else(|| match kind {
                ShapeKind::Connector1D => {
                    VisioShape::new_connector(shape_id, name, 0.0, 0.0, 1.0, 0.0)
                }
                _ => VisioShape::new_rect(shape_id, name, 2.0, 2.0, 1.6, 0.9),
            })
    } else {
        match kind {
            ShapeKind::Connector1D => VisioShape::new_connector(shape_id, name, 0.0, 0.0, 1.0, 0.0),
            _ => VisioShape::new_rect(shape_id, name, 2.0, 2.0, 1.6, 0.9),
        }
    };
    shape.id = shape_id;
    shape.name = name.to_string();
    shape.name_u = name.to_string();
    shape.kind = kind;
    shape
}

fn materialize_shape(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    name: &str,
    kind: ShapeKind,
    master_info: Option<&(String, String)>,
) {
    let template = build_template_shape(doc, shape_id, name, kind, master_info);
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    if let Some(existing) = page.find_shape_mut(shape_id) {
        existing.name = name.to_string();
        existing.name_u = name.to_string();
        existing.kind = kind;
        return;
    }
    insert_shape_deterministically(page, template);
}

fn insert_shape_deterministically(page: &mut VisioPage, shape: VisioShape) {
    let pos = page
        .shapes
        .iter()
        .position(|s| {
            let is_bg_container = s
                .sheet
                .get_cell("User.msvStructureType")
                .and_then(|c| c.value.as_str_coerced().ok())
                .map(|v| v.trim_matches('"') == "Container")
                .unwrap_or(false);
            !is_bg_container && s.id > shape.id
        })
        .unwrap_or(page.shapes.len());
    page.shapes.insert(pos, shape);
}

fn materialize_connect(
    doc: &mut VisioDocument,
    page_id: u32,
    connector_id: u64,
    from_id: u64,
    to_id: u64,
) {
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    if page.find_shape(from_id).is_none() || page.find_shape(to_id).is_none() {
        return;
    }
    if page.find_shape(connector_id).is_none() {
        let conn = VisioShape::new_connector(
            connector_id,
            format!("Dynamic connector.{connector_id}"),
            0.0,
            0.0,
            1.0,
            0.0,
        );
        insert_shape_deterministically(page, conn);
    }
    page.connects
        .retain(|c| c.connector_shape_id != connector_id);
    page.connects.push(GlueEndpoint {
        connector_shape_id: connector_id,
        from_cell: "BeginX".into(),
        target_shape_id: from_id,
        to_cell: "PinX".into(),
        is_dynamic: true,
    });
    page.connects.push(GlueEndpoint {
        connector_shape_id: connector_id,
        from_cell: "EndX".into(),
        target_shape_id: to_id,
        to_cell: "PinX".into(),
        is_dynamic: true,
    });
    page.connects.sort_by(|a, b| {
        a.connector_shape_id
            .cmp(&b.connector_shape_id)
            .then_with(|| a.from_cell.cmp(&b.from_cell))
    });
}

fn apply_cell_formula_to_shape(
    shape: &mut VisioShape,
    canonical_key: &str,
    display_name: &str,
    formula: &str,
    force_setf: bool,
) -> bool {
    if !force_setf {
        if let Some(existing) = shape.sheet.cells.get(canonical_key) {
            if existing.is_guarded() {
                return false;
            }
        }
    }

    // Ensure structured `User.*` and `Prop.*` dictionaries stay synchronized when new cells arrive.
    if let Some(user_rest) = canonical_key.strip_prefix("user.") {
        if !user_rest.contains('.') && !shape.sheet.user_cells.contains_key(user_rest) {
            let raw_name = display_name
                .strip_prefix("User.")
                .or_else(|| display_name.strip_prefix("user."))
                .unwrap_or(user_rest);
            shape.sheet.set_user_cell(raw_name, formula, "");
            return true;
        }
    } else if let Some(prop_rest) = canonical_key.strip_prefix("prop.") {
        if !prop_rest.contains('.') && !shape.sheet.shape_data.contains_key(prop_rest) {
            let raw_name = display_name
                .strip_prefix("Prop.")
                .or_else(|| display_name.strip_prefix("prop."))
                .unwrap_or(prop_rest);
            let is_num = formula.trim().parse::<f64>().is_ok();
            shape.sheet.set_shape_data(ShapeDataRow {
                name: raw_name.to_string(),
                label: raw_name.to_string(),
                prompt: String::new(),
                data_type: if is_num {
                    ShapeDataType::Number
                } else {
                    ShapeDataType::String
                },
                format: String::new(),
                value: formula.to_string(),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            return true;
        }
    }

    shape
        .sheet
        .try_set_formula_respecting_guard(display_name, formula.to_string(), force_setf)
        .is_ok()
}

fn infer_master_info(doc: &VisioDocument, shape: &VisioShape) -> Option<(String, String)> {
    if let Some(mid) = shape.master_id {
        if let Some(m) = doc.master(mid) {
            return Some(("Document".into(), m.name.clone()));
        }
    }
    None
}

fn extract_connect_pairs(page: &VisioPage) -> BTreeMap<u64, (u64, u64)> {
    let mut beg: BTreeMap<u64, u64> = BTreeMap::new();
    let mut end: BTreeMap<u64, u64> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for c in &page.connects {
        ids.insert(c.connector_shape_id);
        if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            beg.insert(c.connector_shape_id, c.target_shape_id);
        } else {
            end.insert(c.connector_shape_id, c.target_shape_id);
        }
    }
    let mut out = BTreeMap::new();
    for cid in ids {
        if let (Some(&f), Some(&t)) = (beg.get(&cid), end.get(&cid)) {
            out.insert(cid, (f, t));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_offline_same_shape_edits_commute_and_converge_without_lost_updates() {
        // Shared initial document containing Shape 1 + Shape 2 + glued connector 3
        let mut base_doc = VisioDocument::new_blank("Collaborative Architecture");
        {
            let p = base_doc.page_mut(1).unwrap();
            let mut s1 = VisioShape::new_rect(1, "GatewayService", 3.0, 4.5, 2.0, 1.0);
            s1.text = "Gateway v1".into();
            let mut s2 = VisioShape::new_rect(2, "DatabaseCluster", 8.0, 4.5, 2.0, 1.0);
            s2.text = "Primary DB".into();
            let c3 = VisioShape::new_connector(3, "Dynamic connector.3", 4.0, 4.5, 7.0, 4.5);
            p.shapes.extend([s1, s2, c3]);
            p.connects.extend([
                GlueEndpoint {
                    connector_shape_id: 3,
                    from_cell: "BeginX".into(),
                    target_shape_id: 1,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
                GlueEndpoint {
                    connector_shape_id: 3,
                    from_cell: "EndX".into(),
                    target_shape_id: 2,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
            ]);
        }
        ReactiveEngine::recalculate_document(&mut base_doc);
        route_page_connectors(&mut base_doc, 1);

        // Peer A edits FillForegnd on Shape 1 while offline
        let mut doc_peer_a = base_doc.clone();
        let prev_a = doc_peer_a.clone();
        doc_peer_a
            .page_mut(1)
            .unwrap()
            .find_shape_mut(1)
            .unwrap()
            .sheet
            .set_formula("FillForegnd", "=RGB(16,185,129)", Some(Unit::Color));
        ReactiveEngine::recalculate_document(&mut doc_peer_a);
        route_page_connectors(&mut doc_peer_a, 1);
        let mut crdt_a = CrdtDocumentState::new("peer-a");
        let ops_a = crdt_a.diff_to_ops(&prev_a, &doc_peer_a, 1_700_000_100);
        assert_eq!(ops_a.len(), 1);

        // Peer B concurrently edits Text AND PinX on the EXACT SAME Shape 1 while offline!
        let mut doc_peer_b = base_doc.clone();
        let prev_b = doc_peer_b.clone();
        {
            let s1_b = doc_peer_b.page_mut(1).unwrap().find_shape_mut(1).unwrap();
            s1_b.text = "Gateway v2 (Active-Active)".into();
            s1_b.sheet
                .set_formula("PinX", "4.25 in", Some(Unit::Inch));
        }
        ReactiveEngine::recalculate_document(&mut doc_peer_b);
        route_page_connectors(&mut doc_peer_b, 1);
        let mut crdt_b = CrdtDocumentState::new("peer-b");
        let ops_b = crdt_b.diff_to_ops(&prev_b, &doc_peer_b, 1_700_000_100);
        assert_eq!(ops_b.len(), 2);

        // Apply in [A, B] order onto replica 1
        let mut doc_ab = base_doc.clone();
        let mut state_ab = CrdtDocumentState::new("verifier-ab");
        let batch_a_json = serde_json::to_string(&ops_a).unwrap();
        let batch_b_json = serde_json::to_string(&ops_b).unwrap();
        assert_eq!(state_ab.apply_batch_json(&mut doc_ab, &batch_a_json).unwrap(), 1);
        assert_eq!(state_ab.apply_batch_json(&mut doc_ab, &batch_b_json).unwrap(), 2);
        // Re-applying duplicate packets must be 100% idempotent (0 mutations)
        assert_eq!(state_ab.apply_batch_json(&mut doc_ab, &batch_a_json).unwrap(), 0);

        // Apply in [B, A] order onto replica 2
        let mut doc_ba = base_doc.clone();
        let mut state_ba = CrdtDocumentState::new("verifier-ba");
        assert_eq!(state_ba.apply_batch_json(&mut doc_ba, &batch_b_json).unwrap(), 2);
        assert_eq!(state_ba.apply_batch_json(&mut doc_ba, &batch_a_json).unwrap(), 1);

        // Peer A and Peer B also exchange ops directly
        assert_eq!(crdt_a.apply_batch_json(&mut doc_peer_a, &batch_b_json).unwrap(), 2);
        assert_eq!(crdt_b.apply_batch_json(&mut doc_peer_b, &batch_a_json).unwrap(), 1);

        // Verify byte-exact document convergence across all 4 replicas with ZERO lost updates!
        assert_eq!(doc_ab, doc_ba);
        assert_eq!(doc_peer_a, doc_ab);
        assert_eq!(doc_peer_b, doc_ba);

        let converged_s1 = doc_ab.page(1).unwrap().find_shape(1).unwrap();
        assert_eq!(converged_s1.text, "Gateway v2 (Active-Active)");
        assert!((converged_s1.sheet.get_f64("PinX").unwrap() - 4.25).abs() < 1e-9);
        assert_eq!(
            converged_s1.sheet.get_color("FillForegnd"),
            Some(vexio_shapesheet::RgbaColor {
                r: 16,
                g: 185,
                b: 129,
                a: 255
            })
        );
    }

    #[test]
    fn guard_enforcement_and_out_of_order_tombstone_convergence() {
        let mut doc = VisioDocument::new_blank("Guard & Tombstone");
        let mut s = VisioShape::new_rect(1, "LockedBox", 3.0, 3.0, 4.0, 2.0);
        s.sheet
            .set_formula("Width", "=GUARD(4 in)", Some(Unit::Inch));
        doc.page_mut(1).unwrap().shapes.push(s);
        ReactiveEngine::recalculate_document(&mut doc);

        let mut crdt = CrdtDocumentState::new("node-1");
        let t1 = crdt.next_timestamp(1000);
        // Non-SETF write against GUARD cell must be rejected
        let rejected = crdt.apply_op(
            &mut doc,
            CrdtOp::SetCell {
                ts: t1,
                page_id: 1,
                shape_id: 1,
                cell_name: "Width".into(),
                formula: "9 in".into(),
                force_setf: false,
            },
        );
        assert!(!rejected);
        assert!((doc.page(1).unwrap().find_shape(1).unwrap().sheet.get_f64("Width").unwrap() - 4.0).abs() < 1e-9);

        // Explicit force_setf write overrides GUARD cleanly
        let t2 = crdt.next_timestamp(1001);
        let accepted = crdt.apply_op(
            &mut doc,
            CrdtOp::SetCell {
                ts: t2,
                page_id: 1,
                shape_id: 1,
                cell_name: "Width".into(),
                formula: "6.5 in".into(),
                force_setf: true,
            },
        );
        assert!(accepted);
        assert!((doc.page(1).unwrap().find_shape(1).unwrap().sheet.get_f64("Width").unwrap() - 6.5).abs() < 1e-9);

        // Out-of-order Tombstone(t=2000) arriving BEFORE UpsertShape(t=1500) must stay tombstoned!
        let op_create = CrdtOp::UpsertShape {
            ts: HlcTimestamp {
                wall_ms: 1500,
                logical: 0,
                replica_id: "peer-x".into(),
            },
            page_id: 1,
            shape_id: 77,
            name: "Ephemeral".into(),
            kind: ShapeKind::Shape2D,
            master_info: None,
        };
        let op_delete = CrdtOp::TombstoneShape {
            ts: HlcTimestamp {
                wall_ms: 2000,
                logical: 0,
                replica_id: "peer-y".into(),
            },
            page_id: 1,
            shape_id: 77,
        };

        let mut d1 = doc.clone();
        let mut s1 = CrdtDocumentState::new("r1");
        s1.apply_op(&mut d1, op_create.clone());
        s1.apply_op(&mut d1, op_delete.clone());

        let mut d2 = doc.clone();
        let mut s2 = CrdtDocumentState::new("r2");
        s2.apply_op(&mut d2, op_delete);
        s2.apply_op(&mut d2, op_create);

        assert!(d1.page(1).unwrap().find_shape(77).is_none());
        assert_eq!(d1, d2);
    }
}
