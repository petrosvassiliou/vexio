//! Containers, Ordered Lists, Callouts & Cross-Functional Swimlane Systems.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use vexio_shapesheet::{
    ControlRow, GeometryRow, GeometrySection, ReactiveEngine, RgbaColor, ShapeDataRow,
    ShapeDataType, ShapeKind, Unit, VisioDocument, VisioShape, format_clean_f64,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerResizeMode {
    FitToContents,
    ExpandOnly,
    Locked,
}

/// Returns `true` when `sh` is a Cross-Functional Swimlane (`User.visSwimlane == 1`).
pub fn is_swimlane_shape(sh: &VisioShape) -> bool {
    sh.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5
}

/// Returns `true` when `sh` is a Visio Container (`User.msvStructureType == "Container"`) and not a Swimlane.
pub fn is_container_shape(sh: &VisioShape) -> bool {
    if is_swimlane_shape(sh) {
        return false;
    }
    sh.sheet
        .get_cell("User.msvStructureType")
        .and_then(|c| c.value.as_str_coerced().ok())
        .is_some_and(|s| s.trim_matches('"').eq_ignore_ascii_case("container"))
}

/// Returns `true` when `sh` is a Visio Callout (`User.msvStructureType == "Callout"`).
pub fn is_callout_shape(sh: &VisioShape) -> bool {
    sh.sheet
        .get_cell("User.msvStructureType")
        .and_then(|c| c.value.as_str_coerced().ok())
        .is_some_and(|s| s.trim_matches('"').eq_ignore_ascii_case("callout"))
}

/// Parse deterministic member IDs from `User.msvContainerMembers`.
pub fn parse_container_members(sh: &VisioShape) -> Vec<u64> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    if let Some(raw) = sh
        .sheet
        .get_cell("User.msvContainerMembers")
        .and_then(|c| c.value.as_str_coerced().ok())
    {
        for tok in raw.trim_matches('"').split(';') {
            if let Ok(id) = tok.trim().parse::<u64>() {
                if id > 0 && seen.insert(id) {
                    out.push(id);
                }
            }
        }
    }
    out
}

/// Persist deterministic member IDs into `User.msvContainerMembers`.
pub fn set_container_members(sh: &mut VisioShape, member_ids: &[u64]) {
    let mut seen = BTreeSet::new();
    let cleaned: Vec<String> = member_ids
        .iter()
        .copied()
        .filter(|&id| id > 0 && seen.insert(id))
        .map(|id| id.to_string())
        .collect();
    sh.sheet.set_user_cell(
        "msvContainerMembers",
        format!("\"{}\"", cleaned.join(";")),
        "Member IDs",
    );
}

fn shape_aabb(sh: &VisioShape) -> ([f64; 2], [f64; 2]) {
    let px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
    let py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
    let w = sh.sheet.get_f64("Width").unwrap_or(1.0).abs();
    let h = sh.sheet.get_f64("Height").unwrap_or(1.0).abs();
    ([px - w * 0.5, py - h * 0.5], [px + w * 0.5, py + h * 0.5])
}

/// Expand a selected move set so dragging a Visio Container or Swimlane automatically moves
/// all of its enclosed member shapes (and internal connectors glued at both ends to moved shapes).
pub fn expand_move_set_with_container_and_swimlane_members(
    doc: &VisioDocument,
    page_id: u32,
    root_ids: &[u64],
) -> Vec<u64> {
    let Some(page) = doc.page(page_id) else {
        return root_ids.to_vec();
    };
    let mut expanded: BTreeSet<u64> = root_ids.iter().copied().collect();
    let mut queue: Vec<u64> = root_ids.to_vec();

    while let Some(sid) = queue.pop() {
        let Some(sh) = page.find_shape(sid) else {
            continue;
        };
        if is_container_shape(sh) {
            for mid in parse_container_members(sh) {
                if page.find_shape(mid).is_some() && expanded.insert(mid) {
                    queue.push(mid);
                }
            }
        } else if is_swimlane_shape(sh) {
            let (mn, mx) = shape_aabb(sh);
            for cand in &page.shapes {
                if cand.id == sid
                    || cand.kind == ShapeKind::Connector1D
                    || is_swimlane_shape(cand)
                {
                    continue;
                }
                let cx = cand.sheet.get_f64("PinX").unwrap_or(0.0);
                let cy = cand.sheet.get_f64("PinY").unwrap_or(0.0);
                if cx >= mn[0] && cx <= mx[0] && cy >= mn[1] && cy <= mx[1] && expanded.insert(cand.id)
                {
                    queue.push(cand.id);
                }
            }
        }
    }

    // Also include any 1D connector whose BOTH endpoints are glued to shapes inside `expanded`
    // so internal `MidCoord` overrides translate cleanly with the container!
    for sh in &page.shapes {
        if sh.kind == ShapeKind::Connector1D && !expanded.contains(&sh.id) {
            let beg_target = page
                .connects
                .iter()
                .find(|c| {
                    c.connector_shape_id == sh.id
                        && c.from_cell.to_ascii_lowercase().starts_with("begin")
                })
                .map(|c| c.target_shape_id);
            let end_target = page
                .connects
                .iter()
                .find(|c| {
                    c.connector_shape_id == sh.id
                        && !c.from_cell.to_ascii_lowercase().starts_with("begin")
                })
                .map(|c| c.target_shape_id);
            if let (Some(bt), Some(et)) = (beg_target, end_target) {
                if expanded.contains(&bt) && expanded.contains(&et) {
                    expanded.insert(sh.id);
                }
            }
        }
    }

    // Preserve deterministic ordering starting with `root_ids`.
    let mut result: Vec<u64> = Vec::with_capacity(expanded.len());
    let mut emitted = BTreeSet::new();
    for &id in root_ids {
        if emitted.insert(id) {
            result.push(id);
        }
    }
    for id in expanded {
        if emitted.insert(id) {
            result.push(id);
        }
    }
    result
}

/// Locate the most-specific (smallest-area) Container or Swimlane under `candidate_center`
/// while dragging `moving_ids`, returning `(target_shape_id, aabb_min, aabb_max)` for drop-zone glow.
pub fn find_drop_target_container_or_swimlane(
    doc: &VisioDocument,
    page_id: u32,
    moving_ids: &[u64],
    candidate_center: [f64; 2],
) -> Option<(u64, [f64; 2], [f64; 2])> {
    let page = doc.page(page_id)?;
    let moving_set: BTreeSet<u64> = moving_ids.iter().copied().collect();
    // Do not highlight container drop zones when dragging a swimlane itself.
    if moving_ids
        .iter()
        .filter_map(|&id| page.find_shape(id))
        .any(is_swimlane_shape)
    {
        return None;
    }

    let margin = 0.14;
    let mut best: Option<(f64, u64, [f64; 2], [f64; 2])> = None;
    for sh in &page.shapes {
        if moving_set.contains(&sh.id) || (!is_container_shape(sh) && !is_swimlane_shape(sh)) {
            continue;
        }
        let (mn, mx) = shape_aabb(sh);
        if candidate_center[0] >= mn[0] - margin
            && candidate_center[0] <= mx[0] + margin
            && candidate_center[1] >= mn[1] - margin
            && candidate_center[1] <= mx[1] + margin
        {
            let area = ((mx[0] - mn[0]).max(0.1)) * ((mx[1] - mn[1]).max(0.1));
            if best.is_none_or(|(b_area, _, _, _)| area < b_area) {
                best = Some((area, sh.id, mn, mx));
            }
        }
    }
    best.map(|(_, id, mn, mx)| (id, mn, mx))
}

/// Reconcile Container membership (`User.msvContainerMembers` + `ExpandOnly` auto-fit) and
/// Swimlane metadata (`Prop.Function` + synchronized lane right-edge expansion) after moving shapes.
pub fn reconcile_containers_and_swimlanes_after_move(
    doc: &mut VisioDocument,
    page_id: u32,
    moved_root_ids: &[u64],
) {
    let (candidate_shape_ids, has_containers, has_swimlanes) = {
        let Some(page) = doc.page(page_id) else {
            return;
        };
        // If the user explicitly dragged a Container or Swimlane itself, its members moved with it;
        // only update membership for non-container, non-swimlane 2D/Group shapes!
        let cands: Vec<u64> = moved_root_ids
            .iter()
            .copied()
            .filter(|&id| {
                page.find_shape(id).is_some_and(|sh| {
                    matches!(sh.kind, ShapeKind::Shape2D | ShapeKind::Group)
                        && !is_container_shape(sh)
                        && !is_swimlane_shape(sh)
                        && !is_callout_shape(sh)
                })
            })
            .collect();
        let has_cont = page.shapes.iter().any(is_container_shape);
        let has_swim = page.shapes.iter().any(is_swimlane_shape);
        (cands, has_cont, has_swim)
    };

    if candidate_shape_ids.is_empty() {
        if has_swimlanes {
            sync_swimlane_shape_metadata(doc, page_id);
        }
        return;
    }

    if has_containers {
        let mut containers_to_refit = BTreeSet::new();
        if let Some(page) = doc.page_mut(page_id) {
            let cont_snapshots: Vec<(u64, [f64; 2], [f64; 2], f64)> = page
                .shapes
                .iter()
                .filter(|s| is_container_shape(s))
                .map(|s| {
                    let (mn, mx) = shape_aabb(s);
                    let area = ((mx[0] - mn[0]).max(0.1)) * ((mx[1] - mn[1]).max(0.1));
                    (s.id, mn, mx, area)
                })
                .collect();

            for &sid in &candidate_shape_ids {
                let Some(sh) = page.find_shape(sid) else {
                    continue;
                };
                let cx = sh.sheet.get_f64("PinX").unwrap_or(0.0);
                let cy = sh.sheet.get_f64("PinY").unwrap_or(0.0);

                // Find smallest enclosing container (with 0.14 in edge tolerance for intuitive drops)
                let target_cont_id = cont_snapshots
                    .iter()
                    .filter(|(cid, mn, mx, _)| {
                        *cid != sid
                            && cx >= mn[0] - 0.14
                            && cx <= mx[0] + 0.14
                            && cy >= mn[1] - 0.14
                            && cy <= mx[1] + 0.14
                    })
                    .min_by(|a, b| a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(cid, _, _, _)| *cid);

                for (cid, mn, mx, _) in &cont_snapshots {
                    let Some(cont_sh) = page.find_shape_mut(*cid) else {
                        continue;
                    };
                    let mut members = parse_container_members(cont_sh);
                    if Some(*cid) == target_cont_id {
                        if !members.contains(&sid) {
                            members.push(sid);
                            set_container_members(cont_sh, &members);
                        }
                        containers_to_refit.insert(*cid);
                    } else if members.contains(&sid) {
                        // Dragged completely outside `cid` -> remove from `msvContainerMembers`!
                        let strictly_outside =
                            cx < mn[0] || cx > mx[0] || cy < mn[1] || cy > mx[1];
                        if strictly_outside {
                            members.retain(|&m| m != sid);
                            set_container_members(cont_sh, &members);
                        } else {
                            containers_to_refit.insert(*cid);
                        }
                    }
                }
            }
        }
        for cid in containers_to_refit {
            refit_container(doc, page_id, cid, 0.35, ContainerResizeMode::ExpandOnly);
        }
    }

    if has_swimlanes {
        // Auto-expand horizontal swimlane pool width if any shapeinside a lane extends past the right border!
        if let Some(page) = doc.page_mut(page_id) {
            let mut max_needed_right: Option<f64> = None;
            let lanes: Vec<(u64, [f64; 2], [f64; 2])> = page
                .shapes
                .iter()
                .filter(|s| is_swimlane_shape(s))
                .map(|s| {
                    let (mn, mx) = shape_aabb(s);
                    (s.id, mn, mx)
                })
                .collect();

            for &sid in &candidate_shape_ids {
                if let Some(sh) = page.find_shape(sid) {
                    let (smn, smx) = shape_aabb(sh);
                    let cy = 0.5 * (smn[1] + smx[1]);
                    let cx = 0.5 * (smn[0] + smx[0]);
                    for &(_, lmn, lmx) in &lanes {
                        let is_horiz_lane = (lmx[0] - lmn[0]) >= (lmx[1] - lmn[1]);
                        if is_horiz_lane && cy >= lmn[1] && cy <= lmx[1] && cx >= lmn[0] {
                            let req_right = smx[0] + 0.45;
                            if req_right > lmx[0] {
                                max_needed_right =
                                    Some(max_needed_right.map_or(req_right, |r| r.max(req_right)));
                            }
                        }
                    }
                }
            }

            if let Some(new_right) = max_needed_right {
                for (lid, lmn, lmx) in lanes {
                    let is_horiz_lane = (lmx[0] - lmn[0]) >= (lmx[1] - lmn[1]);
                    if is_horiz_lane && new_right > lmx[0] {
                        let new_w = new_right - lmn[0];
                        let new_cx = lmn[0] + new_w * 0.5;
                        if let Some(lane_sh) = page.find_shape_mut(lid) {
                            lane_sh.sheet.set_formula(
                                "Width",
                                format!("{} in", format_clean_f64(new_w)),
                                Some(Unit::Inch),
                            );
                            lane_sh.sheet.set_formula(
                                "PinX",
                                format!("{} in", format_clean_f64(new_cx)),
                                Some(Unit::Inch),
                            );
                        }
                    }
                }
            }
        }
        sync_swimlane_shape_metadata(doc, page_id);
    }
}

/// Style a `VisioShape` as a standard Visio Container with top header divider bar and top-left bold title.
pub fn configure_container_shape_sheet(container: &mut VisioShape, member_ids: &[u64]) {
    container
        .sheet
        .set_user_cell("msvStructureType", "\"Container\"", "Structure Type");
    set_container_members(container, member_ids);
    container
        .sheet
        .set_formula("FillForegnd", "=RGB(248,250,252)", Some(Unit::Color));
    container
        .sheet
        .set_formula("FillBkgnd", "=RGB(226,232,240)", Some(Unit::Color));
    container
        .sheet
        .set_formula("LineColor", "=RGB(100,116,139)", Some(Unit::Color));
    container
        .sheet
        .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
    container
        .sheet
        .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
    container
        .sheet
        .set_formula("Char.Style", "1", Some(Unit::Number));
    container
        .sheet
        .set_formula("Para.HorzAlign", "0", Some(Unit::Number));
    container
        .sheet
        .set_formula("VerticalAlign", "0", Some(Unit::Number));
    container
        .sheet
        .set_formula("TxtPinX", "=Width*0.5", Some(Unit::Inch));
    container
        .sheet
        .set_formula("TxtPinY", "=MAX(0.18 in, Height - 0.18 in)", Some(Unit::Inch));
    container
        .sheet
        .set_formula("TxtWidth", "=MAX(0.4 in, Width - 0.24 in)", Some(Unit::Inch));
    container
        .sheet
        .set_formula("TxtHeight", "0.36 in", Some(Unit::Inch));

    let mut hdr_bar = GeometrySection::new(2);
    hdr_bar.no_fill = "TRUE".into();
    hdr_bar.rows = vec![
        GeometryRow::move_to(1, "0 in", "=MAX(0 in, Height - 0.36 in)"),
        GeometryRow::line_to(2, "=Width", "=MAX(0 in, Height - 0.36 in)"),
    ];
    container.sheet.add_geometry(hdr_bar);
}

/// Create a Visio Container shape enclosing `member_ids` with header band and live `User.msvStructureType = "Container"`.
pub fn create_container(
    doc: &mut VisioDocument,
    page_id: u32,
    title: &str,
    member_ids: &[u64],
    padding_in: f64,
) -> Option<u64> {
    let (page_w, page_h, next_id) = {
        let page = doc.page(page_id)?;
        (page.width_in(), page.height_in(), page.next_shape_id())
    };
    let mut container = VisioShape::new_rect(
        next_id,
        format!("Container.{next_id}"),
        page_w * 0.5,
        page_h * 0.5,
        3.6,
        2.4,
    );
    container.text = title.into();
    configure_container_shape_sheet(&mut container, member_ids);

    if let Some(page) = doc.page_mut(page_id) {
        // Insert behind foreground member shapes (right after any background swimlanes).
        let insert_idx = page
            .shapes
            .iter()
            .position(|s| !is_swimlane_shape(s))
            .unwrap_or(0);
        page.shapes.insert(insert_idx, container);
    }
    if !member_ids.is_empty() {
        refit_container(
            doc,
            page_id,
            next_id,
            padding_in,
            ContainerResizeMode::FitToContents,
        );
    } else {
        ReactiveEngine::recalculate_document(doc);
    }
    Some(next_id)
}

pub fn refit_container(
    doc: &mut VisioDocument,
    page_id: u32,
    container_id: u64,
    padding_in: f64,
    mode: ContainerResizeMode,
) {
    if mode == ContainerResizeMode::Locked {
        return;
    }
    ReactiveEngine::recalculate_document(doc);
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    let member_ids = page
        .find_shape(container_id)
        .map(parse_container_members)
        .unwrap_or_default();

    if member_ids.is_empty() {
        return;
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for mid in member_ids {
        if let Some(m) = page.find_shape(mid) {
            let px = m.sheet.get_f64("PinX").unwrap_or(0.0);
            let py = m.sheet.get_f64("PinY").unwrap_or(0.0);
            let w = m.sheet.get_f64("Width").unwrap_or(1.0);
            let h = m.sheet.get_f64("Height").unwrap_or(1.0);
            min_x = min_x.min(px - w * 0.5);
            min_y = min_y.min(py - h * 0.5);
            max_x = max_x.max(px + w * 0.5);
            max_y = max_y.max(py + h * 0.5);
        }
    }
    if !min_x.is_finite() {
        return;
    }

    let header_extra = 0.42;
    let req_min_x = min_x - padding_in;
    let req_max_x = max_x + padding_in;
    let req_min_y = min_y - padding_in;
    let req_max_y = max_y + padding_in + header_extra;

    if let Some(cont) = page.find_shape_mut(container_id) {
        let (cur_min, cur_max) = shape_aabb(cont);
        let (final_min_x, final_max_x, final_min_y, final_max_y) =
            if mode == ContainerResizeMode::ExpandOnly {
                (
                    req_min_x.min(cur_min[0]),
                    req_max_x.max(cur_max[0]),
                    req_min_y.min(cur_min[1]),
                    req_max_y.max(cur_max[1]),
                )
            } else {
                (req_min_x, req_max_x, req_min_y, req_max_y)
            };
        let final_w = (final_max_x - final_min_x).max(1.2);
        let final_h = (final_max_y - final_min_y).max(1.0);
        let cx = 0.5 * (final_min_x + final_max_x);
        let cy = 0.5 * (final_min_y + final_max_y);

        cont.sheet.set_formula(
            "PinX",
            format!("{} in", format_clean_f64(cx)),
            Some(Unit::Inch),
        );
        cont.sheet.set_formula(
            "PinY",
            format!("{} in", format_clean_f64(cy)),
            Some(Unit::Inch),
        );
        cont.sheet.set_formula(
            "Width",
            format!("{} in", format_clean_f64(final_w)),
            Some(Unit::Inch),
        );
        cont.sheet.set_formula(
            "Height",
            format!("{} in", format_clean_f64(final_h)),
            Some(Unit::Inch),
        );
    }
    ReactiveEngine::recalculate_document(doc);
}

/// Attach a Visio Callout annotation tethered reactively to `target_shape_id` via `Controls.Row_1`
/// (or placed standalone with a draggable leader tip when `target_shape_id == 0`).
pub fn attach_callout(
    doc: &mut VisioDocument,
    page_id: u32,
    target_shape_id: u64,
    text: &str,
    offset_dx_in: f64,
    offset_dy_in: f64,
) -> Option<u64> {
    let (cid, cx, cy, has_target) = {
        let page = doc.page(page_id)?;
        let cid = page.next_shape_id();
        if target_shape_id > 0 {
            let target = page.find_shape(target_shape_id)?;
            let tx = target.sheet.get_f64("PinX").unwrap_or(2.0);
            let ty = target.sheet.get_f64("PinY").unwrap_or(2.0);
            (cid, tx + offset_dx_in, ty + offset_dy_in, true)
        } else {
            (cid, page.width_in() * 0.55, page.height_in() * 0.58, false)
        }
    };

    let mut callout = VisioShape::new_rect(cid, format!("Callout.{cid}"), cx, cy, 1.85, 0.78);
    callout.text = text.into();
    callout
        .sheet
        .set_user_cell("msvStructureType", "\"Callout\"", "Structure Type");
    callout.sheet.set_user_cell(
        "msvCalloutTargetSheet",
        format!("{target_shape_id}"),
        "Target Shape ID",
    );
    callout
        .sheet
        .set_formula("FillForegnd", "=RGB(254,249,195)", Some(Unit::Color));
    callout
        .sheet
        .set_formula("LineColor", "=RGB(202,138,4)", Some(Unit::Color));
    callout
        .sheet
        .set_formula("LineWeight", "1.2 pt", Some(Unit::Point));
    callout
        .sheet
        .set_formula("Rounding", "0.08 in", Some(Unit::Inch));

    let (ctrl_x, ctrl_y) = if has_target {
        (
            format!("=Sheet.{target_shape_id}!PinX - PinX + LocPinX"),
            format!("=Sheet.{target_shape_id}!PinY - PinY + LocPinY"),
        )
    } else {
        ("-0.65 in".to_string(), "-0.45 in".to_string())
    };

    callout.sheet.add_control(ControlRow {
        name: "Row_1".into(),
        x: ctrl_x.clone(),
        y: ctrl_y.clone(),
        x_dyn: ctrl_x,
        y_dyn: ctrl_y,
        x_con: "0".into(),
        y_con: "0".into(),
        can_glue: true,
        prompt: "Drag Yellow Callout Leader Tip onto any Shape".into(),
    });

    // Geometry2: Live leader line from nearest clamped point on the callout perimeter to `(Controls.Row_1.X, Controls.Row_1.Y)`!
    let mut leader = GeometrySection::new(2);
    leader.no_fill = "TRUE".into();
    leader.rows = vec![
        GeometryRow::move_to(
            1,
            "=BOUND(Controls.Row_1.X, 0, FALSE, 0 in, Width)",
            "=BOUND(Controls.Row_1.Y, 0, FALSE, 0 in, Height)",
        ),
        GeometryRow::line_to(2, "=Controls.Row_1.X", "=Controls.Row_1.Y"),
    ];
    callout.sheet.add_geometry(leader);

    doc.page_mut(page_id)?.shapes.push(callout);
    ReactiveEngine::recalculate_document(doc);
    Some(cid)
}

fn configure_horizontal_swimlane_sheet(lane: &mut VisioShape, index: usize) {
    lane.sheet
        .set_user_cell("msvStructureType", "\"Container\"", "Swimlane Container");
    lane.sheet
        .set_user_cell("visSwimlane", "1", "Cross-Functional Swimlane");
    let bg = if index % 2 == 0 {
        RgbaColor {
            r: 248,
            g: 250,
            b: 252,
            a: 255,
        }
    } else {
        RgbaColor {
            r: 241,
            g: 245,
            b: 249,
            a: 255,
        }
    };
    lane.sheet.set_formula(
        "FillForegnd",
        format!("=RGB({},{},{})", bg.r, bg.g, bg.b),
        Some(Unit::Color),
    );
    lane.sheet
        .set_formula("LineColor", "=RGB(148,163,184)", Some(Unit::Color));
    lane.sheet
        .set_formula("Char.Style", "1", Some(Unit::Number));
    lane.sheet
        .set_formula("Para.HorzAlign", "1", Some(Unit::Number));
    lane.sheet
        .set_formula("VerticalAlign", "1", Some(Unit::Number));
    lane.sheet
        .set_formula("TxtPinX", "0.52 in", Some(Unit::Inch));
    lane.sheet
        .set_formula("TxtPinY", "=Height*0.5", Some(Unit::Inch));
    lane.sheet
        .set_formula("TxtWidth", "0.96 in", Some(Unit::Inch));
    lane.sheet
        .set_formula("TxtHeight", "=MAX(0.4 in, Height - 0.16 in)", Some(Unit::Inch));

    let mut hdr_col = GeometrySection::new(2);
    hdr_col.no_fill = "TRUE".into();
    hdr_col.rows = vec![
        GeometryRow::move_to(1, "1.04 in", "0 in"),
        GeometryRow::line_to(2, "1.04 in", "=Height"),
    ];
    lane.sheet.add_geometry(hdr_col);
}

/// Build a Cross-Functional Swimlane system (horizontal bands) and assign shape `Prop.Function` automatically.
pub fn build_cross_functional_swimlanes(
    doc: &mut VisioDocument,
    page_id: u32,
    lane_titles: &[&str],
    origin_x: f64,
    top_y: f64,
    lane_width: f64,
    lane_height: f64,
) -> Vec<u64> {
    let mut lane_ids = Vec::new();
    {
        let Some(page) = doc.page_mut(page_id) else {
            return lane_ids;
        };
        for (i, &title) in lane_titles.iter().enumerate() {
            let id = page.next_shape_id();
            let cy = top_y - (i as f64 + 0.5) * lane_height;
            let cx = origin_x + lane_width * 0.5;
            let mut lane =
                VisioShape::new_rect(id, format!("Swimlane.{id}"), cx, cy, lane_width, lane_height);
            lane.text = title.into();
            configure_horizontal_swimlane_sheet(&mut lane, i);
            page.shapes.insert(i, lane);
            lane_ids.push(id);
        }
    }
    sync_swimlane_shape_metadata(doc, page_id);
    lane_ids
}

/// Smart Cross-Functional Swimlane builder:
/// - If `explicit_titles` has 2+ entries or no swimlanes exist yet on `page_id`, creates the requested lanes
///   (defaulting to `["Customer", "Operations", "Engineering"]` when empty).
/// - If `explicit_titles` is empty or has 1 entry and swimlanes ALREADY exist on `page_id`, appends a new
///   contiguous swimlane band snapped flush beneath the lowest existing swimlane!
pub fn insert_or_append_swimlanes(
    doc: &mut VisioDocument,
    page_id: u32,
    explicit_titles: &[&str],
) -> Vec<u64> {
    let existing_stats = doc.page(page_id).map(|page| {
        let mut count = 0usize;
        let mut lowest_bottom = f64::INFINITY;
        let mut ref_cx = page.width_in() * 0.5;
        let mut ref_w = (page.width_in() - 1.2).max(6.5);
        let mut ref_h = 1.9;
        for sh in &page.shapes {
            if is_swimlane_shape(sh) {
                let (mn, mx) = shape_aabb(sh);
                count += 1;
                if mn[1] < lowest_bottom {
                    lowest_bottom = mn[1];
                    ref_cx = 0.5 * (mn[0] + mx[0]);
                    ref_w = (mx[0] - mn[0]).max(3.0);
                    ref_h = (mx[1] - mn[1]).clamp(1.1, 3.2);
                }
            }
        }
        (count, lowest_bottom, ref_cx, ref_w, ref_h, page.width_in(), page.height_in())
    });

    let Some((existing_count, lowest_bottom, ref_cx, ref_w, ref_h, page_w, page_h)) =
        existing_stats
    else {
        return Vec::new();
    };

    if existing_count > 0 && explicit_titles.len() <= 1 {
        let next_num = existing_count + 1;
        let title = explicit_titles
            .first()
            .copied()
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Lane {next_num}"));
        let Some(page) = doc.page_mut(page_id) else {
            return Vec::new();
        };
        let id = page.next_shape_id();
        let cy = lowest_bottom - ref_h * 0.5;
        let mut lane = VisioShape::new_rect(id, format!("Swimlane.{id}"), ref_cx, cy, ref_w, ref_h);
        lane.text = title;
        configure_horizontal_swimlane_sheet(&mut lane, existing_count);
        let insert_pos = page
            .shapes
            .iter()
            .rposition(is_swimlane_shape)
            .map(|p| p + 1)
            .unwrap_or(0);
        page.shapes.insert(insert_pos, lane);
        sync_swimlane_shape_metadata(doc, page_id);
        return vec![id];
    }

    let defaults = ["Customer", "Operations", "Engineering"];
    let titles: Vec<&str> = if explicit_titles.is_empty() {
        defaults.to_vec()
    } else {
        explicit_titles.to_vec()
    };
    let origin_x = 0.65;
    let lane_w = (page_w - 1.3).max(6.5);
    let top_y = (page_h - 0.65).max(5.5);
    let lane_h = ((page_h - 1.4) / (titles.len().max(1) as f64)).clamp(1.45, 2.25);
    build_cross_functional_swimlanes(doc, page_id, &titles, origin_x, top_y, lane_w, lane_h)
}

/// Inspect all non-swimlane 2D shapes on `page_id` and populate `Prop.Function` based on enclosing swimlane band.
pub fn sync_swimlane_shape_metadata(doc: &mut VisioDocument, page_id: u32) {
    ReactiveEngine::recalculate_document(doc);
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    let lanes: Vec<(String, f64, f64, f64, f64)> = page
        .shapes
        .iter()
        .filter(|s| is_swimlane_shape(s))
        .map(|s| {
            let px = s.sheet.get_f64("PinX").unwrap_or(0.0);
            let py = s.sheet.get_f64("PinY").unwrap_or(0.0);
            let w = s.sheet.get_f64("Width").unwrap_or(1.0);
            let h = s.sheet.get_f64("Height").unwrap_or(1.0);
            (
                s.text.clone(),
                px - w * 0.5,
                py - h * 0.5,
                px + w * 0.5,
                py + h * 0.5,
            )
        })
        .collect();

    for sh in &mut page.shapes {
        if sh.kind != ShapeKind::Shape2D || is_swimlane_shape(sh) || is_container_shape(sh) {
            continue;
        }
        let px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
        let py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
        if let Some((lane_name, _, _, _, _)) = lanes
            .iter()
            .find(|(_, x0, y0, x1, y1)| px >= *x0 && px <= *x1 && py >= *y0 && py <= *y1)
        {
            sh.sheet.set_shape_data(ShapeDataRow {
                name: "Function".into(),
                label: "Function".into(),
                prompt: "Assigned Swimlane Function".into(),
                data_type: ShapeDataType::String,
                format: String::new(),
                value: format!("\"{}\"", lane_name.replace('"', "\"\"")),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
        }
    }
    ReactiveEngine::recalculate_document(doc);
}
