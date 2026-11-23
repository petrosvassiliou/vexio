//! Built-in Visio ShapeSheet functions and reactive topological DAG evaluator.
//!
//! Implements Visio's complete formula evaluation semantics:
//! - Protection & side-effects: `GUARD`, `DEPENDSON`, `SETF`, `SETATREF`, `SETATREFEXPR`, `GETREF`, `GETVAL`, `EVALCELL`
//! - Coordinate transforms & geometry: `PNT`, `PNTX`, `PNTY`, `PAR`, `LOCTOPAR`, `PARTOLOC`, `INTERSECTX`, `INTERSECTY`, `GRAVITY`, `DIST`, `MAGNITUDE`, `ANG360`, `ATAN2`, `BOUND`, `HITTEST`
//! - Math, trig, logical, string, color (`RGB`, `HSL`, `LUM`, `HUE`, `SAT`), lookup (`LOOKUP`, `INDEX`), and date/time functions
//! - Cycle-safe topological recalculation across Document, Pages, Masters, Styles, and Shapes with 3-tier inheritance.

use crate::model::{ForeignMediaData, ShapeKind, VisioDocument, VisioPage, VisioShape};
use crate::parser::{BinaryOp, CellRefPath, Expr, SheetTarget, UnaryOp, canonical_cell_name};
use crate::sections::{GeometryRowKind, default_unit_for_cell, visio_standard_palette_color};
use crate::units::{CellError, CellValue, RgbaColor, Unit, normalize_f64};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SheetKey {
    Doc,
    Page(u32),
    Style(u32),
    Master { master_id: u32, shape_id: u64 },
    Shape { page_id: u32, shape_id: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalCellId {
    pub sheet: SheetKey,
    pub cell: String,
}

#[derive(Debug, Clone)]
pub struct PendingSetf {
    pub target: GlobalCellId,
    pub new_formula: String,
    pub force_setf: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ShapeTransform2D {
    pub pin_x: f64,
    pub pin_y: f64,
    pub width: f64,
    pub height: f64,
    pub loc_pin_x: f64,
    pub loc_pin_y: f64,
    pub angle_rad: f64,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl ShapeTransform2D {
    /// Convert local shape coordinates `(lx, ly)` in inches into parent/page coordinates `(px, py)`.
    pub fn local_to_parent(&self, mut lx: f64, mut ly: f64) -> (f64, f64) {
        lx -= self.loc_pin_x;
        ly -= self.loc_pin_y;
        if self.flip_x {
            lx = -lx;
        }
        if self.flip_y {
            ly = -ly;
        }
        let (s, c) = self.angle_rad.sin_cos();
        let rx = lx * c - ly * s;
        let ry = lx * s + ly * c;
        (self.pin_x + rx, self.pin_y + ry)
    }

    /// Convert parent/page coordinates `(px, py)` into local shape coordinates `(lx, ly)`.
    pub fn parent_to_local(&self, px: f64, py: f64) -> (f64, f64) {
        let dx = px - self.pin_x;
        let dy = py - self.pin_y;
        let (s, c) = (-self.angle_rad).sin_cos();
        let mut lx = dx * c - dy * s;
        let mut ly = dx * s + dy * c;
        if self.flip_x {
            lx = -lx;
        }
        if self.flip_y {
            ly = -ly;
        }
        (lx + self.loc_pin_x, ly + self.loc_pin_y)
    }

    pub fn world_aabb(&self) -> ([f64; 2], [f64; 2]) {
        let corners = [
            self.local_to_parent(0.0, 0.0),
            self.local_to_parent(self.width, 0.0),
            self.local_to_parent(self.width, self.height),
            self.local_to_parent(0.0, self.height),
        ];
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (x, y) in corners {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        ([min_x, min_y], [max_x, max_y])
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EvaluatedPathVerb {
    MoveTo([f64; 2]),
    LineTo([f64; 2]),
    QuadTo { ctrl: [f64; 2], to: [f64; 2] },
    CubicTo {
        ctrl1: [f64; 2],
        ctrl2: [f64; 2],
        to: [f64; 2],
    },
    ArcTo { to: [f64; 2], bow: f64 },
    Ellipse {
        center: [f64; 2],
        rx: f64,
        ry: f64,
        angle_rad: f64,
    },
    Close,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedGeometryPath {
    pub no_fill: bool,
    pub no_line: bool,
    pub no_show: bool,
    pub fill_color: Option<RgbaColor>,
    pub stroke_color: Option<RgbaColor>,
    pub stroke_weight_in: Option<f64>,
    pub verbs_page_space: Vec<EvaluatedPathVerb>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedGradientStop {
    pub position: f32,
    pub color: RgbaColor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedConnectionPort {
    pub ix: u32,
    pub cell_name: String,
    pub pos_page_in: [f64; 2],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedControlHandle {
    pub name: String,
    pub pos_page_in: [f64; 2],
    pub can_glue: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluatedShapeVisual {
    pub shape_id: u64,
    pub kind: ShapeKind,
    pub transform: ShapeTransform2D,
    pub aabb_min: [f64; 2],
    pub aabb_max: [f64; 2],
    pub paths: Vec<EvaluatedGeometryPath>,
    pub fill_color: Option<RgbaColor>,
    #[serde(default)]
    pub fill_bkgnd: Option<RgbaColor>,
    #[serde(default)]
    pub fill_pattern: u8,
    #[serde(default)]
    pub fill_gradient_angle_rad: f64,
    #[serde(default)]
    pub fill_gradient_radial: bool,
    #[serde(default)]
    pub gradient_stops: Vec<EvaluatedGradientStop>,
    pub stroke_color: Option<RgbaColor>,
    pub stroke_weight_in: f64,
    #[serde(default)]
    pub line_pattern: u8,
    pub rounding_in: f64,
    pub end_arrow: u8,
    pub begin_arrow: u8,
    #[serde(default = "default_arrow_size")]
    pub end_arrow_size: u8,
    #[serde(default = "default_arrow_size")]
    pub begin_arrow_size: u8,
    pub text: String,
    pub txt_pos: [f64; 2],
    #[serde(default)]
    pub txt_width: Option<f64>,
    #[serde(default)]
    pub txt_height: Option<f64>,
    pub font_size_pt: f64,
    pub text_color: RgbaColor,
    pub font_family: String,
    /// Bitmask (`1 = Bold`, `2 = Italic`, `4 = Underline`, `8 = Strikethrough`).
    pub char_style: u8,
    /// `0 = Left`, `1 = Center`, `2 = Right`, `3 = Justify`.
    pub horiz_align: u8,
    /// `0 = Top`, `1 = Middle`, `2 = Bottom`.
    pub vert_align: u8,
    /// `0 = None`, `1 = Disc`, `2 = Numbered`.
    pub bullet_style: u8,
    pub text_bg_color: Option<RgbaColor>,
    #[serde(default)]
    pub foreign_media: Option<ForeignMediaData>,
    #[serde(default)]
    pub connection_points: Vec<EvaluatedConnectionPort>,
    #[serde(default)]
    pub control_handles: Vec<EvaluatedControlHandle>,
    #[serde(default)]
    pub is_swimlane: bool,
}

fn default_arrow_size() -> u8 {
    2
}

pub struct ReactiveEngine;

impl ReactiveEngine {
    /// Recalculate all ShapeSheet formulas in `doc` cleanly with cycle detection and
    /// bounded `SETF` / `SETATREF` side-effect convergence (up to 4 passes).
    pub fn recalculate_document(doc: &mut VisioDocument) {
        // Ensure structured sections are synced into cell maps first.
        doc.doc_sheet.sync_all_sections_to_cells();
        for style in &mut doc.styles {
            style.sheet.sync_all_sections_to_cells();
        }
        for master in &mut doc.masters {
            master.page_sheet.sync_all_sections_to_cells();
            for s in &mut master.shapes {
                sync_shape_tree(s);
            }
        }
        for page in &mut doc.pages {
            page.page_sheet.sync_all_sections_to_cells();
            for s in &mut page.shapes {
                sync_shape_tree(s);
            }
        }

        for _pass in 0..4 {
            let mut eval = EvalSession {
                doc,
                cache: BTreeMap::new(),
                visiting: HashSet::new(),
                pending_setf: Vec::new(),
            };
            let all_cells = eval.collect_all_cell_ids();
            for cid in all_cells {
                let _ = eval.eval_cell_id(&cid);
            }
            let computed = eval.cache;
            let side_effects = eval.pending_setf;

            // Write back computed values to all sheets.
            apply_computed_values(doc, &computed);

            if side_effects.is_empty() {
                break;
            }
            let mut changed = false;
            for se in side_effects {
                if apply_setf_mutation(doc, &se) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// Evaluate all shapes on a page into world/page-space visual primitives ready for rendering,
    /// spatial R*-tree indexing, CAD export, or PDF/SVG serialization.
    pub fn evaluate_page_visuals(doc: &VisioDocument, page_id: u32) -> Vec<EvaluatedShapeVisual> {
        let Some(page) = doc.page(page_id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for shape in &page.shapes {
            collect_shape_visuals(doc, page, shape, None, &mut out);
        }
        out
    }
}

fn sync_shape_tree(shape: &mut VisioShape) {
    shape.sheet.sync_all_sections_to_cells();
    for c in &mut shape.children {
        sync_shape_tree(c);
    }
}

struct EvalSession<'a> {
    doc: &'a VisioDocument,
    cache: BTreeMap<GlobalCellId, CellValue>,
    visiting: HashSet<GlobalCellId>,
    pending_setf: Vec<PendingSetf>,
}

impl<'a> EvalSession<'a> {
    fn collect_all_cell_ids(&self) -> Vec<GlobalCellId> {
        let mut ids = Vec::new();
        for k in self.doc.doc_sheet.cells.keys() {
            ids.push(GlobalCellId {
                sheet: SheetKey::Doc,
                cell: k.clone(),
            });
        }
        for st in &self.doc.styles {
            for k in st.sheet.cells.keys() {
                ids.push(GlobalCellId {
                    sheet: SheetKey::Style(st.id),
                    cell: k.clone(),
                });
            }
        }
        for m in &self.doc.masters {
            for s in &m.shapes {
                collect_shape_cell_ids_master(m.id, s, &mut ids);
            }
        }
        for p in &self.doc.pages {
            for k in p.page_sheet.cells.keys() {
                ids.push(GlobalCellId {
                    sheet: SheetKey::Page(p.id),
                    cell: k.clone(),
                });
            }
            for s in &p.shapes {
                collect_shape_cell_ids_page(p.id, s, &mut ids);
            }
        }
        ids
    }

    fn lookup_quickstyle_num(&mut self, ctx_sheet: &SheetKey, canonical_key: &str) -> Option<i32> {
        let qid = GlobalCellId {
            sheet: ctx_sheet.clone(),
            cell: canonical_key.to_string(),
        };
        if self.visiting.contains(&qid) {
            return None;
        }
        self.eval_cell_id(&qid)
            .as_internal_f64()
            .ok()
            .filter(|v| v.is_finite())
            .map(|v| v.round() as i32)
    }

    fn eval_cell_id(&mut self, id: &GlobalCellId) -> CellValue {
        if let Some(v) = self.cache.get(id) {
            return v.clone();
        }
        if !self.visiting.insert(id.clone()) {
            // Cycle detected! Return Visio's `#REF!` cleanly without panicking.
            return CellValue::Err(CellError::Ref);
        }

        let val = match self.lookup_cell_with_inheritance(id) {
            Some((owner_sheet, cell)) => {
                if let Some(cached) = &cell.vsdx_cached_value {
                    let f_up = cell.formula.to_ascii_uppercase();
                    if f_up.contains("THEME")
                        || f_up.contains("TINT(")
                        || f_up.contains("SHADE(")
                        || f_up.contains("TONE(")
                        || f_up.contains("LUMMOD(")
                        || f_up.contains("LUMOFF(")
                        || f_up.contains("SATMOD(")
                        || f_up.contains("BLEND(")
                    {
                        cached.clone()
                    } else if let Some(ast) = &cell.ast {
                        let raw = self.eval_expr(&owner_sheet, &id.cell, ast);
                        if matches!(raw, CellValue::Err(_)) {
                            cached.clone()
                        } else {
                            coerce_to_unit_hint(
                                raw,
                                cell.unit_hint.or_else(|| default_unit_for_cell(&id.cell)),
                            )
                        }
                    } else {
                        cached.clone()
                    }
                } else if let Some(ast) = &cell.ast {
                    let raw = self.eval_expr(&owner_sheet, &id.cell, ast);
                    coerce_to_unit_hint(raw, cell.unit_hint.or_else(|| default_unit_for_cell(&id.cell)))
                } else {
                    cell.value.clone()
                }
            }
            None => CellValue::Err(CellError::Name),
        };

        self.visiting.remove(id);
        self.cache.insert(id.clone(), val.clone());
        val
    }

    /// Implements Visio's 3-tier cell inheritance:
    /// `Local Shape` -> `Master Shape` -> `StyleSheet` (`LineStyle` / `FillStyle` / `TextStyle`).
    fn lookup_cell_with_inheritance(
        &self,
        id: &GlobalCellId,
    ) -> Option<(SheetKey, crate::sections::Cell)> {
        self.lookup_cell_with_inheritance_bounded(id, 0)
    }

    fn lookup_cell_with_inheritance_bounded(
        &self,
        id: &GlobalCellId,
        depth: u8,
    ) -> Option<(SheetKey, crate::sections::Cell)> {
        if depth > 16 {
            return None;
        }
        match &id.sheet {
            SheetKey::Doc => self
                .doc
                .doc_sheet
                .cells
                .get(&id.cell)
                .cloned()
                .map(|c| (SheetKey::Doc, c)),
            SheetKey::Page(pid) => self
                .doc
                .page(*pid)?
                .page_sheet
                .cells
                .get(&id.cell)
                .cloned()
                .map(|c| (SheetKey::Page(*pid), c)),
            SheetKey::Style(sid) => {
                let st = self.doc.styles.iter().find(|s| s.id == *sid)?;
                if let Some(c) = st.sheet.cells.get(&id.cell) {
                    return Some((SheetKey::Style(*sid), c.clone()));
                }
                let parent_opt = select_style_parent_for_cell(
                    &id.cell,
                    st.fill_parent,
                    st.line_parent,
                    st.text_parent,
                );
                if let Some(parent_id) = parent_opt {
                    if parent_id != *sid {
                        return self.lookup_cell_with_inheritance_bounded(
                            &GlobalCellId {
                                sheet: SheetKey::Style(parent_id),
                                cell: id.cell.clone(),
                            },
                            depth + 1,
                        );
                    }
                }
                None
            }
            SheetKey::Master {
                master_id,
                shape_id,
            } => {
                let m = self.doc.master(*master_id)?;
                let s = m
                    .shapes
                    .iter()
                    .find_map(|sh| sh.find_shape(*shape_id))
                    .or_else(|| m.shapes.first())?;
                if let Some(c) = s.sheet.cells.get(&id.cell) {
                    if !c.deleted {
                        return Some((
                            SheetKey::Master {
                                master_id: *master_id,
                                shape_id: s.id,
                            },
                            c.clone(),
                        ));
                    }
                }
                if let Some(style_id) = select_style_parent_for_cell(
                    &id.cell,
                    s.fill_style_id,
                    s.line_style_id,
                    s.text_style_id,
                ) {
                    if let Some((_, mut styled)) = self.lookup_cell_with_inheritance_bounded(
                        &GlobalCellId {
                            sheet: SheetKey::Style(style_id),
                            cell: id.cell.clone(),
                        },
                        depth + 1,
                    ) {
                        if styled.formula.to_ascii_uppercase().contains("THEME") {
                            styled.vsdx_cached_value = None;
                        }
                        return Some((
                            SheetKey::Master {
                                master_id: *master_id,
                                shape_id: s.id,
                            },
                            styled,
                        ));
                    }
                }
                None
            }
            SheetKey::Shape { page_id, shape_id } => {
                let page = self.doc.page(*page_id)?;
                let shape = page.find_shape(*shape_id)?;
                if let Some(local) = shape.sheet.cells.get(&id.cell) {
                    if !local.deleted {
                        // Evaluate formula in the local shape's own context!
                        return Some((id.sheet.clone(), local.clone()));
                    }
                }
                // Inherit from Master shape if instantiated from a Master (defaulting `master_shape_id` to root Master shape)
                if let Some(mid) = shape.master_id {
                    let resolved_msid = shape.master_shape_id.or_else(|| {
                        self.doc
                            .master(mid)
                            .and_then(|m| m.shapes.first().map(|s| s.id))
                    });
                    if let Some(msid) = resolved_msid {
                        if let Some((_, inherited)) = self.lookup_cell_with_inheritance_bounded(
                            &GlobalCellId {
                                sheet: SheetKey::Master {
                                    master_id: mid,
                                    shape_id: msid,
                                },
                                cell: id.cell.clone(),
                            },
                            depth + 1,
                        ) {
                            // Crucial Visio rule: inherited formulas evaluate against the INSTANCE shape (`Width*0.5` uses instance `Width`!).
                            return Some((id.sheet.clone(), inherited));
                        }
                    }
                }
                // Inherit from StyleSheet
                let style_id = select_style_parent_for_cell(
                    &id.cell,
                    shape.fill_style_id,
                    shape.line_style_id,
                    shape.text_style_id,
                )
                .unwrap_or(0);
                if let Some((_, mut styled)) = self.lookup_cell_with_inheritance_bounded(
                    &GlobalCellId {
                        sheet: SheetKey::Style(style_id),
                        cell: id.cell.clone(),
                    },
                    depth + 1,
                ) {
                    // StyleSheet `V` was cached for the StyleSheet's own QuickStyle matrix, not this Shape!
                    if styled.formula.to_ascii_uppercase().contains("THEME") {
                        styled.vsdx_cached_value = None;
                    }
                    return Some((id.sheet.clone(), styled));
                }
                None
            }
        }
    }

    fn resolve_ref_path(&self, ctx_sheet: &SheetKey, path: &CellRefPath) -> Option<GlobalCellId> {
        let cell_key = path.canonical_cell_key();
        let sheet = self.resolve_target(ctx_sheet, &path.target)?;
        Some(GlobalCellId {
            sheet,
            cell: cell_key,
        })
    }

    fn resolve_target(&self, ctx_sheet: &SheetKey, target: &SheetTarget) -> Option<SheetKey> {
        match target {
            SheetTarget::SelfSheet => Some(ctx_sheet.clone()),
            SheetTarget::TheDoc => Some(SheetKey::Doc),
            SheetTarget::ThePage => match ctx_sheet {
                SheetKey::Page(pid) | SheetKey::Shape { page_id: pid, .. } => {
                    Some(SheetKey::Page(*pid))
                }
                _ => self.doc.pages.first().map(|p| SheetKey::Page(p.id)),
            },
            SheetTarget::ShapeId(sid) => match ctx_sheet {
                SheetKey::Master { master_id, .. } => Some(SheetKey::Master {
                    master_id: *master_id,
                    shape_id: *sid,
                }),
                SheetKey::Shape { page_id, shape_id } => {
                    if let Some(page) = self.doc.page(*page_id) {
                        if let Some(mapped_id) =
                            resolve_master_instance_sibling_id(self.doc, page, *shape_id, *sid)
                        {
                            return Some(SheetKey::Shape {
                                page_id: *page_id,
                                shape_id: mapped_id,
                            });
                        }
                    }
                    Some(SheetKey::Shape {
                        page_id: *page_id,
                        shape_id: *sid,
                    })
                }
                SheetKey::Page(pid) => Some(SheetKey::Shape {
                    page_id: *pid,
                    shape_id: *sid,
                }),
                _ => Some(SheetKey::Shape {
                    page_id: self.doc.pages.first()?.id,
                    shape_id: *sid,
                }),
            },
            SheetTarget::NamedShape(name) => {
                if let SheetKey::Master { master_id, .. } = ctx_sheet {
                    let m = self.doc.master(*master_id)?;
                    let sh = m.shapes.iter().find(|s| {
                        s.name.eq_ignore_ascii_case(name) || s.name_u.eq_ignore_ascii_case(name)
                    })?;
                    return Some(SheetKey::Master {
                        master_id: *master_id,
                        shape_id: sh.id,
                    });
                }
                let pid = match ctx_sheet {
                    SheetKey::Page(pid) | SheetKey::Shape { page_id: pid, .. } => *pid,
                    _ => self.doc.pages.first()?.id,
                };
                let page = self.doc.page(pid)?;
                let shape = page.find_shape_by_name(name)?;
                Some(SheetKey::Shape {
                    page_id: pid,
                    shape_id: shape.id,
                })
            }
            SheetTarget::PageTarget { page_name, inner } => {
                let page = self.doc.page_by_name(page_name)?;
                let page_ctx = SheetKey::Page(page.id);
                self.resolve_target(&page_ctx, inner)
            }
        }
    }

    fn eval_expr(&mut self, ctx_sheet: &SheetKey, ctx_cell: &str, expr: &Expr) -> CellValue {
        match expr {
            Expr::Number { value, unit } => CellValue::with_unit(*value, *unit),
            Expr::Bool(b) => CellValue::Bool(*b),
            Expr::Str(s) => CellValue::Str(s.clone()),
            Expr::CellRef(path) => {
                if let Some(cid) = self.resolve_ref_path(ctx_sheet, path) {
                    self.eval_cell_id(&cid)
                } else {
                    CellValue::Err(CellError::Ref)
                }
            }
            Expr::Unary { op, expr: sub } => {
                let v = self.eval_expr(ctx_sheet, ctx_cell, sub);
                match op {
                    UnaryOp::Pos => v,
                    UnaryOp::Neg => match v {
                        CellValue::Num { internal, unit } => CellValue::Num {
                            internal: normalize_f64(-internal),
                            unit,
                        },
                        other => match other.as_internal_f64() {
                            Ok(n) => CellValue::number(-n),
                            Err(e) => CellValue::Err(e),
                        },
                    },
                    UnaryOp::Not => match v.as_bool() {
                        Ok(b) => CellValue::Bool(!b),
                        Err(e) => CellValue::Err(e),
                    },
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let lv = self.eval_expr(ctx_sheet, ctx_cell, lhs);
                let rv = self.eval_expr(ctx_sheet, ctx_cell, rhs);
                eval_binary_op(*op, lv, rv)
            }
            Expr::Call { name, args } => self.eval_builtin(ctx_sheet, ctx_cell, name, args),
        }
    }

    fn eval_builtin(
        &mut self,
        ctx_sheet: &SheetKey,
        ctx_cell: &str,
        name: &str,
        args: &[Expr],
    ) -> CellValue {
        let upper = name.to_ascii_uppercase();
        match upper.as_str() {
            // --- Protection, References & Reactive Side-Effects ---
            "GUARD" => {
                let Some(first) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                self.eval_expr(ctx_sheet, ctx_cell, first)
            }
            "DEPENDSON" => {
                // Evaluates all dependency arguments to register reactive evaluation, returns 0 (so `DEPENDSON(a,b)+expr` works) or first value when used standalone.
                for a in args {
                    let _ = self.eval_expr(ctx_sheet, ctx_cell, a);
                }
                CellValue::number(0.0)
            }
            "GETREF" => {
                if let Some(Expr::CellRef(path)) = args.first() {
                    if let Some(cid) = self.resolve_ref_path(ctx_sheet, path) {
                        return CellValue::Ref(encode_global_cell_ref(&cid));
                    }
                }
                CellValue::Err(CellError::Ref)
            }
            "GETVAL" => {
                let Some(first) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match first {
                    Expr::CellRef(path) => match self.resolve_ref_path(ctx_sheet, path) {
                        Some(cid) => self.eval_cell_id(&cid),
                        None => CellValue::Err(CellError::Ref),
                    },
                    other => {
                        let v = self.eval_expr(ctx_sheet, ctx_cell, other);
                        if let CellValue::Ref(ref encoded) = v {
                            if let Some(cid) = decode_global_cell_ref(encoded) {
                                return self.eval_cell_id(&cid);
                            }
                        }
                        v
                    }
                }
            }
            "SETF" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let target_cid = match &args[0] {
                    Expr::CellRef(path) => self.resolve_ref_path(ctx_sheet, path),
                    other => match self.eval_expr(ctx_sheet, ctx_cell, other) {
                        CellValue::Ref(s) => decode_global_cell_ref(&s),
                        _ => None,
                    },
                };
                let formula_val = self.eval_expr(ctx_sheet, ctx_cell, &args[1]);
                let Ok(formula_str) = formula_val.as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                if let Some(target) = target_cid {
                    self.pending_setf.push(PendingSetf {
                        target,
                        new_formula: formula_str,
                        force_setf: true,
                    });
                    CellValue::Bool(true)
                } else {
                    CellValue::Err(CellError::Ref)
                }
            }
            "SETATREF" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let target_cid = match &args[0] {
                    Expr::CellRef(path) => self.resolve_ref_path(ctx_sheet, path),
                    other => match self.eval_expr(ctx_sheet, ctx_cell, other) {
                        CellValue::Ref(s) => decode_global_cell_ref(&s),
                        _ => None,
                    },
                };
                if let Some(set_expr) = args.get(1) {
                    let v = self.eval_expr(ctx_sheet, ctx_cell, set_expr);
                    if let (Some(target), Ok(s)) = (&target_cid, v.as_str_coerced()) {
                        self.pending_setf.push(PendingSetf {
                            target: target.clone(),
                            new_formula: s,
                            force_setf: false,
                        });
                    }
                }
                if let Some(cid) = target_cid {
                    self.eval_cell_id(&cid)
                } else {
                    CellValue::Err(CellError::Ref)
                }
            }
            "SETATREFEXPR" => args
                .first()
                .map(|a| self.eval_expr(ctx_sheet, ctx_cell, a))
                .unwrap_or_default(),
            "EVALCELL" => {
                let Some(first) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                self.eval_expr(ctx_sheet, ctx_cell, first)
            }

            // --- Coordinate Transforms & Geometry ---
            "PNT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let x = match self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let y = match self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                CellValue::Point { x, y }
            }
            "PNTX" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a) {
                    CellValue::Point { x, .. } => CellValue::inches(x),
                    other => match other.as_internal_f64() {
                        Ok(v) => CellValue::inches(v),
                        Err(e) => CellValue::Err(e),
                    },
                }
            }
            "PNTY" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a) {
                    CellValue::Point { y, .. } => CellValue::inches(y),
                    other => match other.as_internal_f64() {
                        Ok(v) => CellValue::inches(v),
                        Err(e) => CellValue::Err(e),
                    },
                }
            }
            "PAR" => {
                // Transforms a `PNT(lx, ly)` from the referenced child/other shape's local coords into parent/page coords,
                // then extracts X or Y if evaluated inside an X or Y cell, or returns `Point`.
                let Some(arg0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let src_sheet = infer_point_source_sheet(ctx_sheet, arg0, self);
                let pnt_val = self.eval_expr(ctx_sheet, ctx_cell, arg0);
                let CellValue::Point { x, y } = pnt_val else {
                    return pnt_val;
                };
                let (px, py) = if let Some(tf) = self.eval_shape_transform(&src_sheet) {
                    tf.local_to_parent(x, y)
                } else {
                    (x, y)
                };
                select_point_component_for_cell(ctx_cell, px, py)
            }
            "LOC" => {
                let Some(arg0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let pnt_val = self.eval_expr(ctx_sheet, ctx_cell, arg0);
                let CellValue::Point { x, y } = pnt_val else {
                    return pnt_val;
                };
                let (lx, ly) = if let Some(tf) = self.eval_shape_transform(ctx_sheet) {
                    tf.parent_to_local(x, y)
                } else {
                    (x, y)
                };
                select_point_component_for_cell(ctx_cell, lx, ly)
            }
            "LOCTOPAR" | "PARTOLOC" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let pnt = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let CellValue::Point { x, y } = pnt else {
                    return CellValue::Err(CellError::Value);
                };
                let src_sheet = match &args[1] {
                    Expr::CellRef(p) => self
                        .resolve_ref_path(ctx_sheet, p)
                        .map(|c| c.sheet)
                        .unwrap_or_else(|| ctx_sheet.clone()),
                    _ => ctx_sheet.clone(),
                };
                let dst_sheet = match &args[2] {
                    Expr::CellRef(p) => self
                        .resolve_ref_path(ctx_sheet, p)
                        .map(|c| c.sheet)
                        .unwrap_or_else(|| ctx_sheet.clone()),
                    _ => ctx_sheet.clone(),
                };
                let (wx, wy) = if let Some(tf) = self.eval_shape_transform(&src_sheet) {
                    tf.local_to_parent(x, y)
                } else {
                    (x, y)
                };
                let (dx, dy) = if let Some(tf) = self.eval_shape_transform(&dst_sheet) {
                    tf.parent_to_local(wx, wy)
                } else {
                    (wx, wy)
                };
                select_point_component_for_cell(ctx_cell, dx, dy)
            }
            "INTERSECTX" | "INTERSECTY" => {
                // Intersection of infinite line 1 `(x1, y1, ang1)` and line 2 `(x2, y2, ang2)`.
                if args.len() < 6 {
                    return CellValue::Err(CellError::Value);
                }
                let mut v = [0.0; 6];
                for (i, a) in args.iter().take(6).enumerate() {
                    match self.eval_expr(ctx_sheet, ctx_cell, a).as_internal_f64() {
                        Ok(val) => v[i] = val,
                        Err(e) => return CellValue::Err(e),
                    }
                }
                let (x1, y1, a1, x2, y2, a2) = (v[0], v[1], v[2], v[3], v[4], v[5]);
                let (dy1, dx1) = a1.sin_cos();
                let (dy2, dx2) = a2.sin_cos();
                let denom = dx1 * dy2 - dy1 * dx2;
                if denom.abs() < 1e-12 {
                    return CellValue::Err(CellError::DivByZero);
                }
                let t = ((x2 - x1) * dy2 - (y2 - y1) * dx2) / denom;
                let ix = x1 + t * dx1;
                let iy = y1 + t * dy1;
                if upper == "INTERSECTX" {
                    CellValue::inches(ix)
                } else {
                    CellValue::inches(iy)
                }
            }
            "GRAVITY" => {
                // `GRAVITY(angle, a1, a2, ...)`: returns the candidate angle `ai` closest to `angle` (used to keep text upright when shapes rotate!).
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(base_ang) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64()
                else {
                    return CellValue::Err(CellError::Value);
                };
                let norm_base = ang360_rad(base_ang);
                let mut best_cand = 0.0;
                let mut best_diff = f64::INFINITY;
                for cand_expr in &args[1..] {
                    if let Ok(c) = self.eval_expr(ctx_sheet, ctx_cell, cand_expr).as_internal_f64()
                    {
                        let nc = ang360_rad(c);
                        let mut diff = (norm_base - nc).abs();
                        if diff > std::f64::consts::PI {
                            diff = std::f64::consts::TAU - diff;
                        }
                        if diff < best_diff {
                            best_diff = diff;
                            best_cand = c;
                        }
                    }
                }
                CellValue::radians(best_cand)
            }
            "DIST" => {
                if args.len() < 4 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(x1) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(y1) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(x2) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(y2) = self.eval_expr(ctx_sheet, ctx_cell, &args[3]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::inches(((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt())
            }
            "MAGNITUDE" => {
                if args.len() < 4 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(c1) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(a) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(c2) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[3]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::inches(((c1 * a).powi(2) + (c2 * b).powi(2)).sqrt())
            }
            "ANG360" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a).as_internal_f64() {
                    Ok(rad) => CellValue::radians(ang360_rad(rad)),
                    Err(e) => CellValue::Err(e),
                }
            }
            "BOUND" => {
                // `BOUND(value, type, disabled, min1, max1, ...)`
                if args.len() < 5 {
                    return CellValue::Err(CellError::Value);
                }
                let val_cv = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let unit = val_cv.unit();
                let Ok(val) = val_cv.as_internal_f64() else {
                    return val_cv;
                };
                let bound_type = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(0.0) as i32;
                let disabled = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[2])
                    .as_bool()
                    .unwrap_or(false);
                if disabled || bound_type == 2 {
                    return val_cv;
                }
                let lo = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[3])
                    .as_internal_f64()
                    .unwrap_or(f64::NEG_INFINITY);
                let hi = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[4])
                    .as_internal_f64()
                    .unwrap_or(f64::INFINITY);
                let res = if bound_type == 0 {
                    // Inclusive clamp [lo, hi]
                    val.clamp(lo.min(hi), lo.max(hi))
                } else {
                    // Exclusive outside (lo, hi) -> snap to nearer edge
                    let a = lo.min(hi);
                    let b = lo.max(hi);
                    if val > a && val < b {
                        if (val - a).abs() <= (b - val).abs() {
                            a
                        } else {
                            b
                        }
                    } else {
                        val
                    }
                };
                CellValue::Num {
                    internal: normalize_f64(res),
                    unit,
                }
            }

            // --- Logical & Error Handling ---
            "IF" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                match self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_bool() {
                    Ok(true) => self.eval_expr(ctx_sheet, ctx_cell, &args[1]),
                    Ok(false) => args
                        .get(2)
                        .map(|e| self.eval_expr(ctx_sheet, ctx_cell, e))
                        .unwrap_or(CellValue::Bool(false)),
                    Err(e) => CellValue::Err(e),
                }
            }
            "IFERROR" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let v = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                if matches!(v, CellValue::Err(_)) {
                    self.eval_expr(ctx_sheet, ctx_cell, &args[1])
                } else {
                    v
                }
            }
            "AND" => {
                for a in args {
                    match self.eval_expr(ctx_sheet, ctx_cell, a).as_bool() {
                        Ok(true) => {}
                        Ok(false) => return CellValue::Bool(false),
                        Err(e) => return CellValue::Err(e),
                    }
                }
                CellValue::Bool(true)
            }
            "OR" => {
                for a in args {
                    match self.eval_expr(ctx_sheet, ctx_cell, a).as_bool() {
                        Ok(true) => return CellValue::Bool(true),
                        Ok(false) => {}
                        Err(e) => return CellValue::Err(e),
                    }
                }
                CellValue::Bool(false)
            }
            "XOR" => {
                let mut acc = false;
                for a in args {
                    match self.eval_expr(ctx_sheet, ctx_cell, a).as_bool() {
                        Ok(b) => acc ^= b,
                        Err(e) => return CellValue::Err(e),
                    }
                }
                CellValue::Bool(acc)
            }
            "NOT" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a).as_bool() {
                    Ok(b) => CellValue::Bool(!b),
                    Err(e) => CellValue::Err(e),
                }
            }
            "ISERR" | "ISERROR" => {
                let Some(a) = args.first() else {
                    return CellValue::Bool(true);
                };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a), CellValue::Err(_)))
            }
            "ISERRNA" => {
                let Some(a) = args.first() else {
                    return CellValue::Bool(false);
                };
                CellValue::Bool(matches!(
                    self.eval_expr(ctx_sheet, ctx_cell, a),
                    CellValue::Err(CellError::NotAvailable)
                ))
            }
            "ISERRVALUE" => {
                let Some(a) = args.first() else {
                    return CellValue::Bool(false);
                };
                CellValue::Bool(matches!(
                    self.eval_expr(ctx_sheet, ctx_cell, a),
                    CellValue::Err(CellError::Value)
                ))
            }
            "NA" => CellValue::Err(CellError::NotAvailable),
            "CHOOSE" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(idx) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let i = idx.trunc() as isize;
                if i < 1 || (i as usize) >= args.len() {
                    CellValue::Err(CellError::Value)
                } else {
                    self.eval_expr(ctx_sheet, ctx_cell, &args[i as usize])
                }
            }

            // --- Math, Trig & Bitwise ---
            "PI" => CellValue::number(std::f64::consts::PI),
            "SIN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.sin())),
            "COS" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.cos())),
            "TAN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.tan())),
            "ASIN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if (-1.0..=1.0).contains(&x) {
                    CellValue::radians(x.asin())
                } else {
                    CellValue::Err(CellError::Num)
                }
            }),
            "ACOS" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if (-1.0..=1.0).contains(&x) {
                    CellValue::radians(x.acos())
                } else {
                    CellValue::Err(CellError::Num)
                }
            }),
            "ATAN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::radians(x.atan())),
            "ATAN2" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(y) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(x) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::radians(y.atan2(x))
            }
            "DEG" => self.unary_num(ctx_sheet, ctx_cell, args, |rad| CellValue::degrees(rad.to_degrees())),
            "RAD" => self.unary_num(ctx_sheet, ctx_cell, args, |deg_or_rad| {
                // If input was plain number in degrees, convert to radians; if already radians, preserve.
                CellValue::radians(deg_or_rad)
            }),
            "SQRT" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x < 0.0 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number(x.sqrt())
                }
            }),
            "ABS" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let cv = self.eval_expr(ctx_sheet, ctx_cell, a);
                let u = cv.unit();
                match cv.as_internal_f64() {
                    Ok(v) => CellValue::Num {
                        internal: v.abs(),
                        unit: u,
                    },
                    Err(e) => CellValue::Err(e),
                }
            }
            "SIGN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let s = if x > 0.0 {
                    1.0
                } else if x < 0.0 {
                    -1.0
                } else {
                    0.0
                };
                CellValue::number(s)
            }),
            "INT" | "FLOOR" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.floor())),
            "CEILING" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.ceil())),
            "TRUNC" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.trunc())),
            "ROUND" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let cv = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let unit = cv.unit();
                let Ok(v) = cv.as_internal_f64() else {
                    return cv;
                };
                let digits = args
                    .get(1)
                    .and_then(|d| self.eval_expr(ctx_sheet, ctx_cell, d).as_internal_f64().ok())
                    .unwrap_or(0.0) as i32;
                let factor = 10_f64.powi(digits);
                let disp = unit.from_internal(v);
                let rounded_disp = (disp * factor).round() / factor;
                CellValue::with_unit(rounded_disp, unit)
            }
            "MOD" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(a) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                if b.abs() < 1e-15 {
                    CellValue::Err(CellError::DivByZero)
                } else {
                    CellValue::number(a.rem_euclid(b))
                }
            }
            "POW" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(a) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::number(a.powf(b))
            }
            "LN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x <= 0.0 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number(x.ln())
                }
            }),
            "LOG10" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x <= 0.0 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number(x.log10())
                }
            }),
            "EXP" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.exp())),
            "MIN" | "MAX" | "SUM" | "AVERAGE" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let mut vals = Vec::with_capacity(args.len());
                let mut first_unit = Unit::Number;
                for (i, a) in args.iter().enumerate() {
                    let cv = self.eval_expr(ctx_sheet, ctx_cell, a);
                    if i == 0 {
                        first_unit = cv.unit();
                    }
                    match cv.as_internal_f64() {
                        Ok(v) => vals.push(v),
                        Err(e) => return CellValue::Err(e),
                    }
                }
                let res = match upper.as_str() {
                    "MIN" => vals.into_iter().fold(f64::INFINITY, f64::min),
                    "MAX" => vals.into_iter().fold(f64::NEG_INFINITY, f64::max),
                    "SUM" => vals.into_iter().sum(),
                    _ => vals.iter().sum::<f64>() / vals.len() as f64,
                };
                CellValue::Num {
                    internal: normalize_f64(res),
                    unit: first_unit,
                }
            }
            "BITAND" | "BITOR" | "BITXOR" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(a) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let ai = a as i64;
                let bi = b as i64;
                let r = match upper.as_str() {
                    "BITAND" => ai & bi,
                    "BITOR" => ai | bi,
                    _ => ai ^ bi,
                };
                CellValue::number(r as f64)
            }
            "BITNOT" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number((!(x as i64)) as f64)),

            // --- Color Functions ---
            "RGB" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let r = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[0])
                    .as_internal_f64()
                    .unwrap_or(0.0)
                    .clamp(0.0, 255.0) as u8;
                let g = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(0.0)
                    .clamp(0.0, 255.0) as u8;
                let b = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[2])
                    .as_internal_f64()
                    .unwrap_or(0.0)
                    .clamp(0.0, 255.0) as u8;
                CellValue::Color(RgbaColor { r, g, b, a: 255 })
            }
            "HSL" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let h = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[0])
                    .as_internal_f64()
                    .unwrap_or(0.0);
                let s = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(0.0);
                let l = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[2])
                    .as_internal_f64()
                    .unwrap_or(0.0);
                CellValue::Color(RgbaColor::from_hsl(h, s, l))
            }
            "RED" | "GREEN" | "BLUE" | "HUE" | "SAT" | "LUM" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let CellValue::Color(c) = self.eval_expr(ctx_sheet, ctx_cell, a) else {
                    return CellValue::Err(CellError::Value);
                };
                let (h, s, l) = c.to_visio_hsl();
                let v = match upper.as_str() {
                    "RED" => c.r as f64,
                    "GREEN" => c.g as f64,
                    "BLUE" => c.b as f64,
                    "HUE" => h,
                    "SAT" => s,
                    _ => l,
                };
                CellValue::number(v)
            }
            "THEMEGUARD" | "THEMECBV" => args
                .first()
                .map(|a| self.eval_expr(ctx_sheet, ctx_cell, a))
                .unwrap_or(CellValue::Err(CellError::Value)),
            "COLOR" => {
                let idx = args
                    .first()
                    .and_then(|a| self.eval_expr(ctx_sheet, ctx_cell, a).as_internal_f64().ok())
                    .unwrap_or(0.0)
                    .max(0.0)
                    .round() as u32;
                CellValue::Color(visio_standard_palette_color(idx).unwrap_or(RgbaColor::BLACK))
            }
            "TINT" | "TONE" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let base = match self.eval_expr(ctx_sheet, ctx_cell, &args[0]) {
                    CellValue::Color(c) => c,
                    CellValue::Num { internal, .. } if internal >= 0.0 => {
                        visio_standard_palette_color(internal.round() as u32).unwrap_or(RgbaColor::WHITE)
                    }
                    _ => return CellValue::Err(CellError::Value),
                };
                let raw_amt = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(0.0);
                let t = if raw_amt > 1.0 && raw_amt <= 100.0 {
                    (raw_amt / 100.0).clamp(0.0, 1.0)
                } else if raw_amt > 100.0 {
                    (raw_amt / 240.0).clamp(0.0, 1.0)
                } else {
                    raw_amt.clamp(0.0, 1.0)
                };
                let mix = |ch: u8| -> u8 {
                    ((ch as f64) + (255.0 - ch as f64) * t).round().clamp(0.0, 255.0) as u8
                };
                CellValue::Color(RgbaColor {
                    r: mix(base.r),
                    g: mix(base.g),
                    b: mix(base.b),
                    a: base.a,
                })
            }
            "SHADE" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let base = match self.eval_expr(ctx_sheet, ctx_cell, &args[0]) {
                    CellValue::Color(c) => c,
                    CellValue::Num { internal, .. } if internal >= 0.0 => {
                        visio_standard_palette_color(internal.round() as u32).unwrap_or(RgbaColor::BLACK)
                    }
                    _ => return CellValue::Err(CellError::Value),
                };
                let raw_amt = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(0.0);
                let s = if raw_amt > 1.0 && raw_amt <= 100.0 {
                    (raw_amt / 100.0).clamp(0.0, 1.0)
                } else if raw_amt > 100.0 {
                    (raw_amt / 240.0).clamp(0.0, 1.0)
                } else {
                    raw_amt.clamp(0.0, 1.0)
                };
                let factor = 1.0 - s * 0.65;
                let mix = |ch: u8| -> u8 { ((ch as f64) * factor).round().clamp(0.0, 255.0) as u8 };
                CellValue::Color(RgbaColor {
                    r: mix(base.r),
                    g: mix(base.g),
                    b: mix(base.b),
                    a: base.a,
                })
            }
            "BLEND" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let CellValue::Color(c1) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]) else {
                    return CellValue::Err(CellError::Value);
                };
                let CellValue::Color(c2) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]) else {
                    return CellValue::Err(CellError::Value);
                };
                let w = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[2])
                    .as_internal_f64()
                    .unwrap_or(0.5)
                    .clamp(0.0, 1.0);
                let lerp = |a: u8, b: u8| -> u8 {
                    ((a as f64) * (1.0 - w) + (b as f64) * w).round().clamp(0.0, 255.0) as u8
                };
                CellValue::Color(RgbaColor {
                    r: lerp(c1.r, c2.r),
                    g: lerp(c1.g, c2.g),
                    b: lerp(c1.b, c2.b),
                    a: 255,
                })
            }
            "LUMMOD" | "LUMOFF" | "SATMOD" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let base = match self.eval_expr(ctx_sheet, ctx_cell, &args[0]) {
                    CellValue::Color(c) => c,
                    CellValue::Num { internal, .. } if internal >= 0.0 => {
                        visio_standard_palette_color(internal.round() as u32)
                            .unwrap_or(RgbaColor::WHITE)
                    }
                    _ => return CellValue::Err(CellError::Value),
                };
                let raw = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(1.0);
                let norm = if raw.abs() > 1.5 { raw / 100.0 } else { raw };
                let (h, mut s, mut l) = base.to_visio_hsl();
                match upper.as_str() {
                    "LUMMOD" => l = (l * norm).clamp(0.0, 240.0),
                    "LUMOFF" => l = (l + norm * 240.0).clamp(0.0, 240.0),
                    _ => s = (s * norm).clamp(0.0, 240.0),
                }
                let mut out = RgbaColor::from_hsl(h, s, l);
                out.a = base.a;
                CellValue::Color(out)
            }
            "THEMEVAL" | "THEME" => {
                let theme = self
                    .doc
                    .active_theme_id
                    .and_then(|tid| self.doc.themes.iter().find(|t| t.id == tid))
                    .or_else(|| self.doc.themes.first())
                    .cloned()
                    .unwrap_or_default();
                let key = args
                    .first()
                    .and_then(|a| self.eval_expr(ctx_sheet, ctx_cell, a).as_str_coerced().ok())
                    .unwrap_or_else(|| ctx_cell.to_string())
                    .to_ascii_lowercase();
                if let Some(rest) = key.strip_prefix("accentcolor") {
                    let idx = rest
                        .trim()
                        .parse::<usize>()
                        .ok()
                        .and_then(|n| n.checked_sub(1))
                        .unwrap_or(0)
                        .min(5);
                    CellValue::Color(theme.accent_colors[idx])
                } else if key.contains("background") || key == "lightcolor" {
                    CellValue::Color(theme.background_color)
                } else if key.contains("textbkgnd") && !key.contains("trans") {
                    CellValue::number(0.0)
                } else if key.contains("fillforegndtrans") || key.contains("trans") {
                    CellValue::number(0.0)
                } else if key.contains("fillpattern") {
                    let qs_mat = self
                        .lookup_quickstyle_num(ctx_sheet, "quickstylefillmatrix")
                        .unwrap_or(0);
                    if (25..=36).contains(&qs_mat) {
                        CellValue::number(28.0) // Top-to-bottom linear gradient
                    } else {
                        CellValue::number(1.0)
                    }
                } else if key.contains("linepattern") {
                    CellValue::number(1.0)
                } else if key.contains("fillbkgnd") {
                    let qs_mat = self
                        .lookup_quickstyle_num(ctx_sheet, "quickstylefillmatrix")
                        .unwrap_or(0);
                    let qs_col = self.lookup_quickstyle_num(ctx_sheet, "quickstylefillcolor");
                    CellValue::Color(resolve_quickstyle_fill_bkgnd(&theme, qs_mat, qs_col))
                } else if key.contains("fill") {
                    let qs_mat = self
                        .lookup_quickstyle_num(ctx_sheet, "quickstylefillmatrix")
                        .unwrap_or(0);
                    let qs_col = self.lookup_quickstyle_num(ctx_sheet, "quickstylefillcolor");
                    CellValue::Color(resolve_quickstyle_fill_color(&theme, qs_mat, qs_col))
                } else if key.contains("linecolor") {
                    let qs_mat = self
                        .lookup_quickstyle_num(ctx_sheet, "quickstylelinematrix")
                        .unwrap_or(0);
                    let qs_col = self.lookup_quickstyle_num(ctx_sheet, "quickstylelinecolor");
                    CellValue::Color(resolve_quickstyle_line_color(&theme, qs_mat, qs_col))
                } else if key.contains("char.color") || key.contains("fontcolor") {
                    let qs_fmat = self
                        .lookup_quickstyle_num(ctx_sheet, "quickstylefontmatrix")
                        .unwrap_or(0);
                    let qs_fill_mat = self
                        .lookup_quickstyle_num(ctx_sheet, "quickstylefillmatrix")
                        .unwrap_or(0);
                    let qs_col = self.lookup_quickstyle_num(ctx_sheet, "quickstylefontcolor");
                    CellValue::Color(resolve_quickstyle_font_color(
                        &theme,
                        qs_fmat,
                        qs_fill_mat,
                        qs_col,
                    ))
                } else if key.contains("font") {
                    if key.contains("heading") || key.contains("major") {
                        CellValue::Str(theme.heading_font)
                    } else {
                        CellValue::Str(theme.body_font)
                    }
                } else {
                    CellValue::Color(theme.dark_color)
                }
            }

            // --- String, Lookup & Shape Metadata Functions ---
            "CONCATENATE" => {
                let mut out = String::new();
                for a in args {
                    match self.eval_expr(ctx_sheet, ctx_cell, a).as_str_coerced() {
                        Ok(s) => out.push_str(&s),
                        Err(e) => return CellValue::Err(e),
                    }
                }
                CellValue::Str(out)
            }
            "LEN" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a).as_str_coerced() {
                    Ok(s) => CellValue::number(s.chars().count() as f64),
                    Err(e) => CellValue::Err(e),
                }
            }
            "UPPER" | "LOWER" | "TRIM" => {
                let Some(a) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a).as_str_coerced() {
                    Ok(s) => CellValue::Str(match upper.as_str() {
                        "UPPER" => s.to_uppercase(),
                        "LOWER" => s.to_lowercase(),
                        _ => s.trim().to_string(),
                    }),
                    Err(e) => CellValue::Err(e),
                }
            }
            "LEFT" | "RIGHT" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let count = args
                    .get(1)
                    .and_then(|n| self.eval_expr(ctx_sheet, ctx_cell, n).as_internal_f64().ok())
                    .unwrap_or(1.0)
                    .max(0.0) as usize;
                let chars: Vec<char> = s.chars().collect();
                let res: String = if upper == "LEFT" {
                    chars.into_iter().take(count).collect()
                } else {
                    let skip = chars.len().saturating_sub(count);
                    chars.into_iter().skip(skip).collect()
                };
                CellValue::Str(res)
            }
            "MID" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let start_1 = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[1])
                    .as_internal_f64()
                    .unwrap_or(1.0)
                    .max(1.0) as usize;
                let len = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[2])
                    .as_internal_f64()
                    .unwrap_or(0.0)
                    .max(0.0) as usize;
                let res: String = s.chars().skip(start_1 - 1).take(len).collect();
                CellValue::Str(res)
            }
            "FIND" | "SEARCH" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(needle) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(haystack) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced()
                else {
                    return CellValue::Err(CellError::Value);
                };
                let (n, h) = if upper == "SEARCH" {
                    (needle.to_lowercase(), haystack.to_lowercase())
                } else {
                    (needle, haystack)
                };
                match h.find(&n) {
                    Some(byte_idx) => {
                        let char_pos = h[..byte_idx].chars().count() + 1;
                        CellValue::number(char_pos as f64)
                    }
                    None => CellValue::Err(CellError::Value),
                }
            }
            "SUBSTITUTE" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(old) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(new_s) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_str_coerced()
                else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::Str(s.replace(&old, &new_s))
            }
            "STRSAME" | "EXACT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(a) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let ignore_case = args
                    .get(2)
                    .and_then(|c| self.eval_expr(ctx_sheet, ctx_cell, c).as_bool().ok())
                    .unwrap_or(false);
                if ignore_case {
                    CellValue::Bool(a.eq_ignore_ascii_case(&b))
                } else {
                    CellValue::Bool(a == b)
                }
            }
            "FORMAT" | "FORMATEX" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let val = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let pic = args
                    .get(1)
                    .and_then(|p| self.eval_expr(ctx_sheet, ctx_cell, p).as_str_coerced().ok())
                    .unwrap_or_default();
                if let Ok(num) = val.as_internal_f64() {
                    let disp = val.unit().from_internal(num);
                    if let Some(dot_idx) = pic.find('.') {
                        let decimals = pic[dot_idx + 1..]
                            .chars()
                            .take_while(|c| *c == '0' || *c == '#')
                            .count();
                        return CellValue::Str(format!("{:.*}", decimals, disp));
                    }
                }
                match val.as_str_coerced() {
                    Ok(s) => CellValue::Str(s),
                    Err(e) => CellValue::Err(e),
                }
            }
            "LOOKUP" => {
                // `LOOKUP("key", "a;b;c", ";")`: 0-based index in delimiter-separated list, or -1 if absent.
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(key) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(list) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let delim = args
                    .get(2)
                    .and_then(|d| self.eval_expr(ctx_sheet, ctx_cell, d).as_str_coerced().ok())
                    .unwrap_or_else(|| ";".into());
                for (i, item) in list.split(&delim).enumerate() {
                    if item.eq_ignore_ascii_case(&key) {
                        return CellValue::number(i as f64);
                    }
                }
                CellValue::number(-1.0)
            }
            "INDEX" => {
                // `INDEX(idx, "a;b;c", ";", [error_val])`
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(idx_f) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64()
                else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(list) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let delim = args
                    .get(2)
                    .and_then(|d| self.eval_expr(ctx_sheet, ctx_cell, d).as_str_coerced().ok())
                    .unwrap_or_else(|| ";".into());
                let idx = idx_f.trunc() as isize;
                let parts: Vec<&str> = list.split(&delim).collect();
                if idx >= 0 && (idx as usize) < parts.len() {
                    CellValue::Str(parts[idx as usize].to_string())
                } else if let Some(fallback) = args.get(3) {
                    self.eval_expr(ctx_sheet, ctx_cell, fallback)
                } else {
                    CellValue::Str(String::new())
                }
            }
            "SHAPETEXT" => {
                if let SheetKey::Shape { page_id, shape_id } = ctx_sheet {
                    if let Some(s) = self.doc.page(*page_id).and_then(|p| p.find_shape(*shape_id)) {
                        return CellValue::Str(s.text.clone());
                    }
                }
                CellValue::Str(String::new())
            }
            "PAGENAME" => {
                let pid = match ctx_sheet {
                    SheetKey::Page(p) | SheetKey::Shape { page_id: p, .. } => Some(*p),
                    _ => self.doc.pages.first().map(|p| p.id),
                };
                let name = pid
                    .and_then(|id| self.doc.page(id))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                CellValue::Str(name)
            }
            "HITTEST" => {
                // `HITTEST(x, y, tolerance)`: checks if `(x, y)` lies inside current shape's world AABB.
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(x) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(y) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let tol = self
                    .eval_expr(ctx_sheet, ctx_cell, &args[2])
                    .as_internal_f64()
                    .unwrap_or(0.0);
                if let Some(tf) = self.eval_shape_transform(ctx_sheet) {
                    let (mn, mx) = tf.world_aabb();
                    let hit = x >= mn[0] - tol
                        && x <= mx[0] + tol
                        && y >= mn[1] - tol
                        && y <= mx[1] + tol;
                    CellValue::number(if hit { 2.0 } else { 0.0 })
                } else {
                    CellValue::number(0.0)
                }
            }
            "NURBS" | "POLYLINE" => {
                let mut nums = Vec::with_capacity(args.len());
                for a in args {
                    match self.eval_expr(ctx_sheet, ctx_cell, a).as_internal_f64() {
                        Ok(v) if v.is_finite() => nums.push(crate::units::format_clean_f64(v)),
                        _ => nums.push("0".into()),
                    }
                }
                CellValue::Str(format!("{}({})", upper, nums.join(",")))
            }
            "DOCMD" => CellValue::number(0.0),
            _ => CellValue::Err(CellError::Name),
        }
    }

    fn unary_num<F>(
        &mut self,
        ctx_sheet: &SheetKey,
        ctx_cell: &str,
        args: &[Expr],
        f: F,
    ) -> CellValue
    where
        F: FnOnce(f64) -> CellValue,
    {
        let Some(a) = args.first() else {
            return CellValue::Err(CellError::Value);
        };
        match self.eval_expr(ctx_sheet, ctx_cell, a).as_internal_f64() {
            Ok(x) => f(x),
            Err(e) => CellValue::Err(e),
        }
    }

    fn eval_sheet_f64(&mut self, sheet: &SheetKey, cell_name: &str, default: f64) -> f64 {
        let cid = GlobalCellId {
            sheet: sheet.clone(),
            cell: canonical_cell_name(cell_name),
        };
        self.eval_cell_id(&cid).as_internal_f64().unwrap_or(default)
    }

    fn eval_sheet_bool(&mut self, sheet: &SheetKey, cell_name: &str) -> bool {
        let cid = GlobalCellId {
            sheet: sheet.clone(),
            cell: canonical_cell_name(cell_name),
        };
        self.eval_cell_id(&cid).as_bool().unwrap_or(false)
    }

    fn eval_shape_transform(&mut self, sheet: &SheetKey) -> Option<ShapeTransform2D> {
        if !matches!(sheet, SheetKey::Shape { .. } | SheetKey::Master { .. }) {
            return None;
        }
        let width = self.eval_sheet_f64(sheet, "Width", 1.0);
        let height = self.eval_sheet_f64(sheet, "Height", 1.0);
        Some(ShapeTransform2D {
            pin_x: self.eval_sheet_f64(sheet, "PinX", 0.0),
            pin_y: self.eval_sheet_f64(sheet, "PinY", 0.0),
            width,
            height,
            loc_pin_x: self.eval_sheet_f64(sheet, "LocPinX", width * 0.5),
            loc_pin_y: self.eval_sheet_f64(sheet, "LocPinY", height * 0.5),
            angle_rad: self.eval_sheet_f64(sheet, "Angle", 0.0),
            flip_x: self.eval_sheet_bool(sheet, "FlipX"),
            flip_y: self.eval_sheet_bool(sheet, "FlipY"),
        })
    }
}

fn eval_binary_op(op: BinaryOp, lv: CellValue, rv: CellValue) -> CellValue {
    if let CellValue::Err(e) = lv {
        return CellValue::Err(e);
    }
    if let CellValue::Err(e) = rv {
        return CellValue::Err(e);
    }
    match op {
        BinaryOp::Concat => match (lv.as_str_coerced(), rv.as_str_coerced()) {
            (Ok(a), Ok(b)) => CellValue::Str(format!("{a}{b}")),
            (Err(e), _) | (_, Err(e)) => CellValue::Err(e),
        },
        BinaryOp::And => match (lv.as_bool(), rv.as_bool()) {
            (Ok(a), Ok(b)) => CellValue::Bool(a && b),
            (Err(e), _) | (_, Err(e)) => CellValue::Err(e),
        },
        BinaryOp::Or => match (lv.as_bool(), rv.as_bool()) {
            (Ok(a), Ok(b)) => CellValue::Bool(a || b),
            (Err(e), _) | (_, Err(e)) => CellValue::Err(e),
        },
        BinaryOp::Eq | BinaryOp::NotEq | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            if let (CellValue::Str(a), CellValue::Str(b)) = (&lv, &rv) {
                let ord = a.cmp(b);
                let res = match op {
                    BinaryOp::Eq => ord.is_eq(),
                    BinaryOp::NotEq => ord.is_ne(),
                    BinaryOp::Lt => ord.is_lt(),
                    BinaryOp::Le => ord.is_le(),
                    BinaryOp::Gt => ord.is_gt(),
                    _ => ord.is_ge(),
                };
                return CellValue::Bool(res);
            }
            let (Ok(a), Ok(b)) = (lv.as_internal_f64(), rv.as_internal_f64()) else {
                return CellValue::Err(CellError::Value);
            };
            let diff = a - b;
            let eq = diff.abs() < 1e-10;
            let res = match op {
                BinaryOp::Eq => eq,
                BinaryOp::NotEq => !eq,
                BinaryOp::Lt => !eq && a < b,
                BinaryOp::Le => eq || a < b,
                BinaryOp::Gt => !eq && a > b,
                _ => eq || a > b,
            };
            CellValue::Bool(res)
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow => {
            let out_unit = match (lv.unit(), rv.unit(), op) {
                (u, Unit::Number, _) => u,
                (Unit::Number, u, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul) => u,
                (u1, u2, BinaryOp::Add | BinaryOp::Sub) if u1 == u2 => u1,
                (u1, _, BinaryOp::Add | BinaryOp::Sub) => u1,
                _ => Unit::Number,
            };
            let (Ok(a), Ok(b)) = (lv.as_internal_f64(), rv.as_internal_f64()) else {
                return CellValue::Err(CellError::Value);
            };
            let res = match op {
                BinaryOp::Add => a + b,
                BinaryOp::Sub => a - b,
                BinaryOp::Mul => a * b,
                BinaryOp::Div => {
                    if b.abs() < 1e-15 {
                        return CellValue::Err(CellError::DivByZero);
                    }
                    a / b
                }
                _ => a.powf(b),
            };
            if res.is_nan() || res.is_infinite() {
                CellValue::Err(CellError::Num)
            } else {
                CellValue::Num {
                    internal: normalize_f64(res),
                    unit: out_unit,
                }
            }
        }
    }
}

fn coerce_to_unit_hint(val: CellValue, hint: Option<Unit>) -> CellValue {
    match (val, hint) {
        (CellValue::Num { internal, unit: Unit::Number }, Some(u))
            if !matches!(u, Unit::Number | Unit::String | Unit::Color | Unit::Point2D | Unit::Guid) =>
        {
            // In Visio, if a bare number `2` is written into an inch cell (`Width = 2`), Visio interprets bare unitless literals in the cell's default unit (`2 in`) ONLY when unit wasn't specified, whereas computed expressions like `Width * 0.5` already carry `internal` in inches!
            CellValue::Num { internal, unit: u }
        }
        (other, _) => other,
    }
}

fn ang360_rad(rad: f64) -> f64 {
    let r = rad.rem_euclid(std::f64::consts::TAU);
    normalize_f64(r)
}

fn select_point_component_for_cell(ctx_cell: &str, x: f64, y: f64) -> CellValue {
    let c = ctx_cell.to_ascii_lowercase();
    if c.ends_with('x') || c.contains(".x") {
        CellValue::inches(x)
    } else if c.ends_with('y') || c.contains(".y") {
        CellValue::inches(y)
    } else {
        CellValue::Point { x, y }
    }
}

fn infer_point_source_sheet(
    ctx_sheet: &SheetKey,
    arg: &Expr,
    session: &EvalSession<'_>,
) -> SheetKey {
    if let Expr::Call { name, args } = arg {
        if name.eq_ignore_ascii_case("PNT") {
            if let Some(Expr::CellRef(path)) = args.first() {
                if let Some(cid) = session.resolve_ref_path(ctx_sheet, path) {
                    return cid.sheet;
                }
            }
        }
    }
    ctx_sheet.clone()
}

fn encode_global_cell_ref(cid: &GlobalCellId) -> String {
    match &cid.sheet {
        SheetKey::Doc => format!("DOC|{}", cid.cell),
        SheetKey::Page(p) => format!("PAGE|{p}|{}", cid.cell),
        SheetKey::Style(s) => format!("STYLE|{s}|{}", cid.cell),
        SheetKey::Master {
            master_id,
            shape_id,
        } => format!("MASTER|{master_id}|{shape_id}|{}", cid.cell),
        SheetKey::Shape { page_id, shape_id } => format!("SHAPE|{page_id}|{shape_id}|{}", cid.cell),
    }
}

fn decode_global_cell_ref(encoded: &str) -> Option<GlobalCellId> {
    let parts: Vec<&str> = encoded.split('|').collect();
    match parts.as_slice() {
        ["DOC", cell] => Some(GlobalCellId {
            sheet: SheetKey::Doc,
            cell: (*cell).into(),
        }),
        ["PAGE", p, cell] => Some(GlobalCellId {
            sheet: SheetKey::Page(p.parse().ok()?),
            cell: (*cell).into(),
        }),
        ["STYLE", s, cell] => Some(GlobalCellId {
            sheet: SheetKey::Style(s.parse().ok()?),
            cell: (*cell).into(),
        }),
        ["MASTER", m, s, cell] => Some(GlobalCellId {
            sheet: SheetKey::Master {
                master_id: m.parse().ok()?,
                shape_id: s.parse().ok()?,
            },
            cell: (*cell).into(),
        }),
        ["SHAPE", p, s, cell] => Some(GlobalCellId {
            sheet: SheetKey::Shape {
                page_id: p.parse().ok()?,
                shape_id: s.parse().ok()?,
            },
            cell: (*cell).into(),
        }),
        _ => None,
    }
}

fn collect_shape_cell_ids_master(master_id: u32, s: &VisioShape, out: &mut Vec<GlobalCellId>) {
    for k in s.sheet.cells.keys() {
        out.push(GlobalCellId {
            sheet: SheetKey::Master {
                master_id,
                shape_id: s.id,
            },
            cell: k.clone(),
        });
    }
    for c in &s.children {
        collect_shape_cell_ids_master(master_id, c, out);
    }
}

fn select_style_parent_for_cell(
    canonical_cell: &str,
    fill_id: Option<u32>,
    line_id: Option<u32>,
    text_id: Option<u32>,
) -> Option<u32> {
    if canonical_cell.starts_with("fill") || canonical_cell.starts_with("shdw") {
        fill_id.or(line_id).or(text_id)
    } else if canonical_cell.starts_with("line")
        || canonical_cell.starts_with("beginarrow")
        || canonical_cell.starts_with("endarrow")
        || canonical_cell == "rounding"
    {
        line_id.or(fill_id).or(text_id)
    } else if canonical_cell.starts_with("char.")
        || canonical_cell.starts_with("para.")
        || canonical_cell.starts_with("txt")
        || canonical_cell.starts_with("text")
        || canonical_cell == "verticalalign"
        || canonical_cell.ends_with("margin")
    {
        text_id.or(fill_id).or(line_id)
    } else {
        fill_id.or(line_id).or(text_id)
    }
}

fn collect_shape_cell_ids_page(page_id: u32, s: &VisioShape, out: &mut Vec<GlobalCellId>) {
    for k in s.sheet.cells.keys() {
        out.push(GlobalCellId {
            sheet: SheetKey::Shape {
                page_id,
                shape_id: s.id,
            },
            cell: k.clone(),
        });
    }
    // Ensure core inherited cells are evaluated on Master or StyleSheet instances even if not overridden locally!
    if s.master_id.is_some()
        || s.fill_style_id.is_some()
        || s.line_style_id.is_some()
        || s.text_style_id.is_some()
    {
        for core_key in [
            "pinx",
            "piny",
            "width",
            "height",
            "locpinx",
            "locpiny",
            "angle",
            "fillpattern",
            "fillforegnd",
            "fillforegndtrans",
            "fillbkgnd",
            "fillgradientenabled",
            "fillgradientangle",
            "fillgradientdir",
            "linepattern",
            "linecolor",
            "linecolortrans",
            "lineweight",
            "beginarrow",
            "endarrow",
            "beginarrowsize",
            "endarrowsize",
            "rounding",
            "quickstylefillmatrix",
            "quickstylelinematrix",
            "quickstylefontmatrix",
            "quickstylefillcolor",
            "quickstylelinecolor",
            "quickstylefontcolor",
            "char.size",
            "char.color",
            "char.style",
            "char.smallcaps",
            "char.strikethru",
            "char.font",
            "para.horzalign",
            "para.bullet",
            "verticalalign",
            "textbkgnd",
            "textbkgndtrans",
            "txtpinx",
            "txtpiny",
            "txtwidth",
            "txtheight",
            "txtlocpinx",
            "txtlocpiny",
            "txtangle",
            "hidetext",
        ] {
            if !s.sheet.cells.contains_key(core_key) {
                out.push(GlobalCellId {
                    sheet: SheetKey::Shape {
                        page_id,
                        shape_id: s.id,
                    },
                    cell: core_key.into(),
                });
            }
        }
    }
    for c in &s.children {
        collect_shape_cell_ids_page(page_id, c, out);
    }
}

fn apply_computed_values(doc: &mut VisioDocument, computed: &BTreeMap<GlobalCellId, CellValue>) {
    for (cid, val) in computed {
        match &cid.sheet {
            SheetKey::Doc => {
                if let Some(c) = doc.doc_sheet.cells.get_mut(&cid.cell) {
                    c.value = val.clone();
                }
            }
            SheetKey::Page(pid) => {
                if let Some(p) = doc.page_mut(*pid) {
                    if let Some(c) = p.page_sheet.cells.get_mut(&cid.cell) {
                        c.value = val.clone();
                    }
                }
            }
            SheetKey::Style(sid) => {
                if let Some(st) = doc.styles.iter_mut().find(|s| s.id == *sid) {
                    if let Some(c) = st.sheet.cells.get_mut(&cid.cell) {
                        c.value = val.clone();
                    }
                }
            }
            SheetKey::Master {
                master_id,
                shape_id,
            } => {
                if let Some(m) = doc.masters.iter_mut().find(|m| m.id == *master_id) {
                    if let Some(s) = m.shapes.iter_mut().find_map(|sh| sh.find_shape_mut(*shape_id))
                    {
                        if let Some(c) = s.sheet.cells.get_mut(&cid.cell) {
                            c.value = val.clone();
                        }
                    }
                }
            }
            SheetKey::Shape { page_id, shape_id } => {
                if let Some(p) = doc.page_mut(*page_id) {
                    if let Some(s) = p.find_shape_mut(*shape_id) {
                        if let Some(c) = s.sheet.cells.get_mut(&cid.cell) {
                            c.value = val.clone();
                        } else if !matches!(val, CellValue::Err(_)) {
                            // Cache valid inherited evaluated value on instance cell entry
                            let mut cell = crate::sections::Cell::from_formula(
                                cid.cell.clone(),
                                val.to_vsdx_val_string(),
                                Some(val.unit()),
                            );
                            cell.value = val.clone();
                            s.sheet.cells.insert(cid.cell.clone(), cell);
                        }
                    }
                }
            }
        }
    }
}

fn apply_setf_mutation(doc: &mut VisioDocument, se: &PendingSetf) -> bool {
    let target_sheet = match &se.target.sheet {
        SheetKey::Doc => Some(&mut doc.doc_sheet),
        SheetKey::Page(pid) => doc.page_mut(*pid).map(|p| &mut p.page_sheet),
        SheetKey::Style(sid) => doc
            .styles
            .iter_mut()
            .find(|s| s.id == *sid)
            .map(|s| &mut s.sheet),
        SheetKey::Master {
            master_id,
            shape_id,
        } => doc
            .masters
            .iter_mut()
            .find(|m| m.id == *master_id)
            .and_then(|m| m.shapes.iter_mut().find_map(|s| s.find_shape_mut(*shape_id)))
            .map(|s| &mut s.sheet),
        SheetKey::Shape { page_id, shape_id } => doc
            .page_mut(*page_id)
            .and_then(|p| p.find_shape_mut(*shape_id))
            .map(|s| &mut s.sheet),
    };
    let Some(ss) = target_sheet else {
        return false;
    };
    if let Some(existing) = ss.get_cell(&se.target.cell) {
        if existing.formula == se.new_formula {
            return false;
        }
    }
    ss.try_set_formula_respecting_guard(&se.target.cell, se.new_formula.clone(), se.force_setf)
        .is_ok()
}

fn collect_shape_visuals(
    doc: &VisioDocument,
    page: &VisioPage,
    shape: &VisioShape,
    parent_tf: Option<&ShapeTransform2D>,
    out: &mut Vec<EvaluatedShapeVisual>,
) {
    // Check layer visibility and color overrides.
    let mut any_visible = true;
    let mut layer_color_override = None;
    if !shape.layer_membership.is_empty() {
        any_visible = false;
        for &lix in &shape.layer_membership {
            if let Some(ldef) = page.layers.iter().find(|l| l.ix == lix) {
                if ldef.visible {
                    any_visible = true;
                    if ldef.color_override.is_some() {
                        layer_color_override = ldef.color_override;
                    }
                }
            } else {
                any_visible = true;
            }
        }
    }
    if !any_visible {
        return;
    }

    let master_shape = shape.master_id.and_then(|mid| {
        let m = doc.master(mid)?;
        if let Some(msid) = shape.master_shape_id {
            m.shapes.iter().find_map(|s| s.find_shape(msid))
        } else {
            m.shapes.first()
        }
    });

    let ss = &shape.sheet;
    let w = ss
        .get_f64("Width")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("Width")))
        .unwrap_or(1.0);
    let h = ss
        .get_f64("Height")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("Height")))
        .unwrap_or(if shape.kind == ShapeKind::Connector1D {
            0.0
        } else {
            1.0
        });
    let local_tf = ShapeTransform2D {
        pin_x: ss.get_f64("PinX").unwrap_or(0.0),
        pin_y: ss.get_f64("PinY").unwrap_or(0.0),
        width: w,
        height: h,
        loc_pin_x: ss.get_f64("LocPinX").unwrap_or(w * 0.5),
        loc_pin_y: ss.get_f64("LocPinY").unwrap_or(h * 0.5),
        angle_rad: ss.get_f64("Angle").unwrap_or(0.0),
        flip_x: ss.get_bool("FlipX").unwrap_or(false),
        flip_y: ss.get_bool("FlipY").unwrap_or(false),
    };
    let world_tf = if let Some(ptf) = parent_tf {
        let (wx, wy) = ptf.local_to_parent(local_tf.pin_x, local_tf.pin_y);
        ShapeTransform2D {
            pin_x: wx,
            pin_y: wy,
            angle_rad: ptf.angle_rad + local_tf.angle_rad,
            flip_x: ptf.flip_x ^ local_tf.flip_x,
            flip_y: ptf.flip_y ^ local_tf.flip_y,
            ..local_tf
        }
    } else {
        local_tf
    };

    let mut paths = Vec::new();
    let mut aabb_min = [f64::INFINITY, f64::INFINITY];
    let mut aabb_max = [f64::NEG_INFINITY, f64::NEG_INFINITY];

    let mut expand = |pt: [f64; 2]| {
        aabb_min[0] = aabb_min[0].min(pt[0]);
        aabb_min[1] = aabb_min[1].min(pt[1]);
        aabb_max[0] = aabb_max[0].max(pt[0]);
        aabb_max[1] = aabb_max[1].max(pt[1]);
    };

    let active_geometries = if !ss.geometries.is_empty() {
        &ss.geometries
    } else if let Some(ms) = master_shape {
        &ms.sheet.geometries
    } else {
        &ss.geometries
    };
    let geom_cell_sheet = |cell_key: &str| -> Option<f64> {
        ss.get_f64(cell_key)
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(cell_key)))
    };
    let geom_bool_sheet = |cell_key: &str| -> Option<bool> {
        ss.get_bool(cell_key)
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_bool(cell_key)))
    };

    for geom in active_geometries {
        if geom.deleted {
            continue;
        }
        let prefix = format!("Geometry{}", geom.index);
        let no_fill = geom_bool_sheet(&format!("{prefix}.NoFill")).unwrap_or(false);
        let no_line = geom_bool_sheet(&format!("{prefix}.NoLine")).unwrap_or(false);
        let no_show = geom_bool_sheet(&format!("{prefix}.NoShow")).unwrap_or(false);
        if no_show {
            continue;
        }
        let geom_str_sheet = |cell_key: &str| -> Option<String> {
            ss.get_str(cell_key)
                .or_else(|| ss.get_cell(cell_key).map(|c| c.formula.clone()))
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_str(cell_key)))
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_cell(cell_key).map(|c| c.formula.clone())))
        };
        let mut verbs = Vec::new();
        let mut last_local_pt: Option<[f64; 2]> = None;
        let active_rows: Vec<&crate::sections::GeometryRow> =
            geom.rows.iter().filter(|r| !r.deleted).collect();
        let mut r_pos = 0usize;
        while r_pos < active_rows.len() {
            let row = active_rows[r_pos];
            let ix = row.ix;
            let raw_x = geom_cell_sheet(&format!("{prefix}.X{ix}")).unwrap_or(0.0);
            let raw_y = geom_cell_sheet(&format!("{prefix}.Y{ix}")).unwrap_or(0.0);
            match row.kind {
                GeometryRowKind::MoveTo => {
                    let (px, py) = world_tf.local_to_parent(raw_x, raw_y);
                    expand([px, py]);
                    last_local_pt = Some([raw_x, raw_y]);
                    verbs.push(EvaluatedPathVerb::MoveTo([px, py]));
                }
                GeometryRowKind::RelMoveTo => {
                    let lx = raw_x * w;
                    let ly = raw_y * h;
                    let (px, py) = world_tf.local_to_parent(lx, ly);
                    expand([px, py]);
                    last_local_pt = Some([lx, ly]);
                    verbs.push(EvaluatedPathVerb::MoveTo([px, py]));
                }
                GeometryRowKind::LineTo | GeometryRowKind::InfiniteLine => {
                    let (px, py) = world_tf.local_to_parent(raw_x, raw_y);
                    expand([px, py]);
                    last_local_pt = Some([raw_x, raw_y]);
                    verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                }
                GeometryRowKind::PolylineTo => {
                    if let Some(poly_str) = geom_str_sheet(&format!("{prefix}.A{ix}"))
                        .or_else(|| row.a.clone())
                    {
                        for [lx, ly] in parse_visio_polyline_local_points(&poly_str, w, h) {
                            let (wx, wy) = world_tf.local_to_parent(lx, ly);
                            expand([wx, wy]);
                            verbs.push(EvaluatedPathVerb::LineTo([wx, wy]));
                        }
                    }
                    let (px, py) = world_tf.local_to_parent(raw_x, raw_y);
                    expand([px, py]);
                    last_local_pt = Some([raw_x, raw_y]);
                    verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                }
                GeometryRowKind::RelLineTo => {
                    let lx = raw_x * w;
                    let ly = raw_y * h;
                    let (px, py) = world_tf.local_to_parent(lx, ly);
                    expand([px, py]);
                    last_local_pt = Some([lx, ly]);
                    verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                }
                GeometryRowKind::ArcTo => {
                    let raw_bow = geom_cell_sheet(&format!("{prefix}.A{ix}")).unwrap_or(0.0);
                    let bow = if world_tf.flip_x ^ world_tf.flip_y {
                        -raw_bow
                    } else {
                        raw_bow
                    };
                    let (px, py) = world_tf.local_to_parent(raw_x, raw_y);
                    expand([px - bow.abs(), py - bow.abs()]);
                    expand([px + bow.abs(), py + bow.abs()]);
                    last_local_pt = Some([raw_x, raw_y]);
                    verbs.push(EvaluatedPathVerb::ArcTo {
                        to: [px, py],
                        bow,
                    });
                }
                GeometryRowKind::RelQuadBezTo => {
                    let ax = geom_cell_sheet(&format!("{prefix}.A{ix}")).unwrap_or(0.0) * w;
                    let by = geom_cell_sheet(&format!("{prefix}.B{ix}")).unwrap_or(0.0) * h;
                    let lx = raw_x * w;
                    let ly = raw_y * h;
                    let (cx, cy) = world_tf.local_to_parent(ax, by);
                    let (px, py) = world_tf.local_to_parent(lx, ly);
                    expand([cx, cy]);
                    expand([px, py]);
                    last_local_pt = Some([lx, ly]);
                    verbs.push(EvaluatedPathVerb::QuadTo {
                        ctrl: [cx, cy],
                        to: [px, py],
                    });
                }
                GeometryRowKind::RelCubBezTo => {
                    let c1x = geom_cell_sheet(&format!("{prefix}.A{ix}")).unwrap_or(0.0) * w;
                    let c1y = geom_cell_sheet(&format!("{prefix}.B{ix}")).unwrap_or(0.0) * h;
                    let c2x = geom_cell_sheet(&format!("{prefix}.C{ix}")).unwrap_or(0.0) * w;
                    let c2y = geom_cell_sheet(&format!("{prefix}.D{ix}")).unwrap_or(0.0) * h;
                    let lx = raw_x * w;
                    let ly = raw_y * h;
                    let (p1x, p1y) = world_tf.local_to_parent(c1x, c1y);
                    let (p2x, p2y) = world_tf.local_to_parent(c2x, c2y);
                    let (px, py) = world_tf.local_to_parent(lx, ly);
                    expand([p1x, p1y]);
                    expand([p2x, p2y]);
                    expand([px, py]);
                    last_local_pt = Some([lx, ly]);
                    verbs.push(EvaluatedPathVerb::CubicTo {
                        ctrl1: [p1x, p1y],
                        ctrl2: [p2x, p2y],
                        to: [px, py],
                    });
                }
                GeometryRowKind::Ellipse => {
                    let ax = geom_cell_sheet(&format!("{prefix}.A{ix}")).unwrap_or(raw_x + w * 0.5);
                    let ay = geom_cell_sheet(&format!("{prefix}.B{ix}")).unwrap_or(raw_y);
                    let cx_pt = geom_cell_sheet(&format!("{prefix}.C{ix}")).unwrap_or(raw_x);
                    let dy_pt = geom_cell_sheet(&format!("{prefix}.D{ix}")).unwrap_or(raw_y + h * 0.5);
                    let rx = ((ax - raw_x).powi(2) + (ay - raw_y).powi(2)).sqrt();
                    let ry = ((cx_pt - raw_x).powi(2) + (dy_pt - raw_y).powi(2)).sqrt();
                    let major_ang = (ay - raw_y).atan2(ax - raw_x);
                    let eff_ang = if world_tf.flip_x ^ world_tf.flip_y {
                        world_tf.angle_rad - major_ang
                    } else {
                        world_tf.angle_rad + major_ang
                    };
                    let (wx, wy) = world_tf.local_to_parent(raw_x, raw_y);
                    let r_max = rx.max(ry);
                    expand([wx - r_max, wy - r_max]);
                    expand([wx + r_max, wy + r_max]);
                    last_local_pt = None;
                    verbs.push(EvaluatedPathVerb::Ellipse {
                        center: [wx, wy],
                        rx,
                        ry,
                        angle_rad: eff_ang,
                    });
                }
                GeometryRowKind::EllipticalArcTo | GeometryRowKind::RelEllipticalArcTo => {
                    let scale = if row.kind == GeometryRowKind::RelEllipticalArcTo {
                        (w, h)
                    } else {
                        (1.0, 1.0)
                    };
                    let end_local = [raw_x * scale.0, raw_y * scale.1];
                    let (px, py) = world_tf.local_to_parent(end_local[0], end_local[1]);
                    expand([px, py]);
                    let a_opt = geom_cell_sheet(&format!("{prefix}.A{ix}"));
                    let b_opt = geom_cell_sheet(&format!("{prefix}.B{ix}"));
                    let c_rad = geom_cell_sheet(&format!("{prefix}.C{ix}")).unwrap_or(0.0);
                    let d_ratio = geom_cell_sheet(&format!("{prefix}.D{ix}")).unwrap_or(1.0);
                    if let (Some(p0_local), Some(ax), Some(by)) = (last_local_pt, a_opt, b_opt) {
                        let mid_local = [ax * scale.0, by * scale.1];
                        let cubics = decompose_visio_elliptical_arc_local_cubics(
                            p0_local,
                            mid_local,
                            end_local,
                            c_rad,
                            d_ratio,
                            w,
                            h,
                        );
                        if cubics.is_empty() {
                            verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                        } else {
                            for (c1_l, c2_l, end_l) in cubics {
                                let (c1x, c1y) = world_tf.local_to_parent(c1_l[0], c1_l[1]);
                                let (c2x, c2y) = world_tf.local_to_parent(c2_l[0], c2_l[1]);
                                let (ex, ey) = world_tf.local_to_parent(end_l[0], end_l[1]);
                                expand([c1x, c1y]);
                                expand([c2x, c2y]);
                                expand([ex, ey]);
                                verbs.push(EvaluatedPathVerb::CubicTo {
                                    ctrl1: [c1x, c1y],
                                    ctrl2: [c2x, c2y],
                                    to: [ex, ey],
                                });
                            }
                        }
                    } else {
                        verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                    }
                    last_local_pt = Some(end_local);
                }
                GeometryRowKind::NURBSTo => {
                    let end_local = [raw_x, raw_y];
                    let knot_prev = geom_cell_sheet(&format!("{prefix}.A{ix}")).unwrap_or(0.0);
                    let weight_last = geom_cell_sheet(&format!("{prefix}.B{ix}")).unwrap_or(1.0);
                    let knot_first = geom_cell_sheet(&format!("{prefix}.C{ix}")).unwrap_or(0.0);
                    let weight_first = geom_cell_sheet(&format!("{prefix}.D{ix}")).unwrap_or(1.0);
                    let nurbs_str = geom_str_sheet(&format!("{prefix}.E{ix}"))
                        .or_else(|| row.e.clone());
                    let mut emitted = false;
                    if let (Some(p0_local), Some(expr_s)) = (last_local_pt, nurbs_str) {
                        let sampled = evaluate_visio_nurbs_local_points(
                            p0_local,
                            end_local,
                            knot_first,
                            weight_first,
                            knot_prev,
                            weight_last,
                            &expr_s,
                            w,
                            h,
                        );
                        if !sampled.is_empty() {
                            for [lx, ly] in sampled {
                                let (wx, wy) = world_tf.local_to_parent(lx, ly);
                                expand([wx, wy]);
                                verbs.push(EvaluatedPathVerb::LineTo([wx, wy]));
                            }
                            emitted = true;
                        }
                    }
                    if !emitted {
                        let (px, py) = world_tf.local_to_parent(raw_x, raw_y);
                        expand([px, py]);
                        verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                    }
                    last_local_pt = Some(end_local);
                }
                GeometryRowKind::SplineStart => {
                    let degree = geom_cell_sheet(&format!("{prefix}.D{ix}"))
                        .unwrap_or(3.0)
                        .round()
                        .clamp(1.0, 8.0) as usize;
                    let k0 = geom_cell_sheet(&format!("{prefix}.A{ix}")).unwrap_or(0.0);
                    let k1 = geom_cell_sheet(&format!("{prefix}.B{ix}")).unwrap_or(0.0);
                    let k_last = geom_cell_sheet(&format!("{prefix}.C{ix}")).unwrap_or(1.0);
                    let mut ctrl_pts: Vec<[f64; 2]> = Vec::new();
                    if let Some(p0) = last_local_pt {
                        ctrl_pts.push(p0);
                    }
                    ctrl_pts.push([raw_x, raw_y]);
                    let mut raw_knots = vec![k0, k1];
                    while r_pos + 1 < active_rows.len()
                        && active_rows[r_pos + 1].kind == GeometryRowKind::SplineKnot
                    {
                        r_pos += 1;
                        let krow = active_rows[r_pos];
                        let kix = krow.ix;
                        let kx = geom_cell_sheet(&format!("{prefix}.X{kix}")).unwrap_or(0.0);
                        let ky = geom_cell_sheet(&format!("{prefix}.Y{kix}")).unwrap_or(0.0);
                        let ka = geom_cell_sheet(&format!("{prefix}.A{kix}")).unwrap_or(0.0);
                        ctrl_pts.push([kx, ky]);
                        raw_knots.push(ka);
                    }
                    raw_knots.push(k_last);
                    let weights = vec![1.0; ctrl_pts.len()];
                    let sampled = sample_rational_bspline(&ctrl_pts, &weights, &raw_knots, degree);
                    for [lx, ly] in &sampled {
                        let (wx, wy) = world_tf.local_to_parent(*lx, *ly);
                        expand([wx, wy]);
                        verbs.push(EvaluatedPathVerb::LineTo([wx, wy]));
                    }
                    if let Some(&last_c) = ctrl_pts.last() {
                        last_local_pt = Some(last_c);
                    }
                }
                GeometryRowKind::SplineKnot => {
                    let (px, py) = world_tf.local_to_parent(raw_x, raw_y);
                    expand([px, py]);
                    last_local_pt = Some([raw_x, raw_y]);
                    verbs.push(EvaluatedPathVerb::LineTo([px, py]));
                }
            }
            r_pos += 1;
        }
        let path_fill = layer_color_override.or_else(|| ss.get_color(&format!("{prefix}.FillForegnd")));
        let path_stroke = layer_color_override.or_else(|| ss.get_color(&format!("{prefix}.LineColor")));
        let path_weight = geom_cell_sheet(&format!("{prefix}.LineWeight"));
        paths.push(EvaluatedGeometryPath {
            no_fill,
            no_line,
            no_show,
            fill_color: path_fill,
            stroke_color: path_stroke,
            stroke_weight_in: path_weight,
            verbs_page_space: verbs,
        });
    }

    if !aabb_min[0].is_finite() {
        let (mn, mx) = world_tf.world_aabb();
        aabb_min = mn;
        aabb_max = mx;
    }

    let active_theme = doc
        .active_theme_id
        .and_then(|tid| doc.themes.iter().find(|t| t.id == tid))
        .or_else(|| doc.themes.first())
        .cloned()
        .unwrap_or_default();

    let qs_fill_mat = ss.get_f64("QuickStyleFillMatrix").map(|v| v.round() as i32).unwrap_or(0);
    let qs_fill_col = ss.get_f64("QuickStyleFillColor").map(|v| v.round() as i32);
    let qs_line_mat = ss.get_f64("QuickStyleLineMatrix").map(|v| v.round() as i32).unwrap_or(0);
    let qs_line_col = ss.get_f64("QuickStyleLineColor").map(|v| v.round() as i32);
    let qs_font_mat = ss.get_f64("QuickStyleFontMatrix").map(|v| v.round() as i32).unwrap_or(0);
    let qs_font_col = ss.get_f64("QuickStyleFontColor").map(|v| v.round() as i32);

    let mut fill_pat = ss.get_f64("FillPattern").unwrap_or(1.0).round() as i32;
    if fill_pat == 1 && (25..=36).contains(&qs_fill_mat) && ss.get_cell("FillPattern").is_none() {
        fill_pat = 28;
    }
    let line_pat = ss.get_f64("LinePattern").unwrap_or(1.0).round() as i32;
    let fill_trans = ss.get_f64("FillForegndTrans").unwrap_or(0.0).clamp(0.0, 1.0);
    let line_trans = ss.get_f64("LineColorTrans").unwrap_or(0.0).clamp(0.0, 1.0);

    let has_drawable_paths = !paths.is_empty();
    let fill_color = if fill_pat == 0
        || fill_trans >= 0.995
        || !has_drawable_paths
        || matches!(
            shape.kind,
            ShapeKind::Connector1D | ShapeKind::Group | ShapeKind::Guide | ShapeKind::ForeignObject
        )
    {
        None
    } else {
        layer_color_override
            .or_else(|| ss.get_color("FillForegnd"))
            .or_else(|| {
                if qs_fill_mat > 0 || qs_fill_col.is_some() {
                    Some(resolve_quickstyle_fill_color(&active_theme, qs_fill_mat, qs_fill_col))
                } else {
                    None
                }
            })
            .or(Some(RgbaColor::WHITE))
            .map(|mut c| {
                if fill_trans > 0.001 {
                    c.a = (((1.0 - fill_trans) * (c.a as f64)).round() as u8).max(1);
                }
                c
            })
    };

    let fill_bkgnd = if fill_color.is_some() {
        ss.get_color("FillBkgnd").or_else(|| {
            if (25..=36).contains(&qs_fill_mat) {
                Some(resolve_quickstyle_fill_bkgnd(&active_theme, qs_fill_mat, qs_fill_col))
            } else {
                None
            }
        })
    } else {
        None
    };

    // Resolve linear/radial gradient parameters & stops (`FillPattern 25..=40`, `FillGradientEnabled`, or `<Section N="FillGradient">`).
    let grad_enabled = ss.get_bool("FillGradientEnabled").unwrap_or(false)
        || (25..=40).contains(&fill_pat)
        || !ss.fill_gradient_stops.is_empty();
    let fill_gradient_radial = (36..=40).contains(&fill_pat)
        || matches!(ss.get_f64("FillGradientDir").map(|v| v.round() as i32), Some(5..=9));
    let fill_gradient_angle_rad = ss.get_f64("FillGradientAngle").unwrap_or_else(|| {
        // Visio FillPattern preset angles (`25`=L->R 0°, `27`=R->L 180°, `28`=T->B -90°, `30`=B->T +90°, `31..35`=diagonals)
        match fill_pat {
            25 | 26 => 0.0,
            27 => std::f64::consts::PI,
            28 | 29 => -std::f64::consts::FRAC_PI_2,
            30 => std::f64::consts::FRAC_PI_2,
            31 | 32 => -std::f64::consts::FRAC_PI_4,
            33 | 34 => std::f64::consts::FRAC_PI_4,
            _ => -std::f64::consts::FRAC_PI_2,
        }
    });

    let mut gradient_stops: Vec<EvaluatedGradientStop> = Vec::new();
    if grad_enabled && fill_color.is_some() {
        let stop_source = if !ss.fill_gradient_stops.is_empty() {
            &ss.fill_gradient_stops
        } else if let Some(ms) = master_shape.filter(|m| !m.sheet.fill_gradient_stops.is_empty()) {
            &ms.sheet.fill_gradient_stops
        } else {
            &ss.fill_gradient_stops
        };
        for st in stop_source {
            let pos_key = format!("FillGradient.Stop{}.Position", st.ix);
            let col_key = format!("FillGradient.Stop{}.Color", st.ix);
            let tr_key = format!("FillGradient.Stop{}.ColorTrans", st.ix);
            let raw_pos = ss
                .get_f64(&pos_key)
                .or_else(|| st.position.trim().trim_end_matches('%').parse::<f64>().ok().map(|v| if v > 1.0 { v / 100.0 } else { v }))
                .unwrap_or(0.0)
                .clamp(0.0, 1.0) as f32;
            let mut col = ss
                .get_color(&col_key)
                .or_else(|| RgbaColor::from_hex(&st.color))
                .unwrap_or_else(|| fill_color.unwrap_or(RgbaColor::WHITE));
            let tr = ss.get_f64(&tr_key).unwrap_or(0.0).clamp(0.0, 1.0);
            if tr > 0.001 {
                col.a = (((1.0 - tr) * (col.a as f64)).round() as u8).max(1);
            }
            gradient_stops.push(EvaluatedGradientStop {
                position: raw_pos,
                color: col,
            });
        }
        gradient_stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap_or(std::cmp::Ordering::Equal));
        if gradient_stops.len() < 2 {
            gradient_stops.clear();
            if let (Some(fg), Some(bg)) = (fill_color, fill_bkgnd) {
                if fg != bg {
                    gradient_stops.push(EvaluatedGradientStop {
                        position: 0.0,
                        color: fg,
                    });
                    gradient_stops.push(EvaluatedGradientStop {
                        position: 1.0,
                        color: bg,
                    });
                }
            }
        }
    }

    let stroke_color = if line_pat == 0
        || line_trans >= 0.995
        || shape.kind == ShapeKind::ForeignObject
        || (!has_drawable_paths
            && matches!(shape.kind, ShapeKind::Group | ShapeKind::Guide))
        || !has_drawable_paths
    {
        None
    } else {
        layer_color_override
            .or_else(|| ss.get_color("LineColor"))
            .or_else(|| {
                if qs_line_mat > 0 || qs_line_col.is_some() {
                    Some(resolve_quickstyle_line_color(&active_theme, qs_line_mat, qs_line_col))
                } else {
                    None
                }
            })
            .or(Some(RgbaColor::BLACK))
            .map(|mut c| {
                if line_trans > 0.001 {
                    c.a = (((1.0 - line_trans) * (c.a as f64)).round() as u8).max(1);
                }
                c
            })
    };

    let txt_width = ss
        .get_f64("TxtWidth")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtWidth")))
        .filter(|v| v.is_finite() && *v > 0.05);
    let txt_height = ss
        .get_f64("TxtHeight")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtHeight")))
        .filter(|v| v.is_finite() && *v > 0.05);
    let mut txt_local_center = [w * 0.5, h * 0.5];

    let (txt_px, txt_py) = if shape.kind == ShapeKind::Connector1D
        && ss.get_cell("TxtPinX").is_none()
        && ss.get_cell("TxtPinY").is_none()
    {
        // Unrouted 1D connector fallback: use midpoint of evaluated path vertices (or Begin..End)
        let mut pts = Vec::new();
        for p in &paths {
            for v in &p.verbs_page_space {
                match v {
                    EvaluatedPathVerb::MoveTo(pt)
                    | EvaluatedPathVerb::LineTo(pt)
                    | EvaluatedPathVerb::ArcTo { to: pt, .. }
                    | EvaluatedPathVerb::QuadTo { to: pt, .. }
                    | EvaluatedPathVerb::CubicTo { to: pt, .. } => pts.push(*pt),
                    EvaluatedPathVerb::Ellipse { center, .. } => pts.push(*center),
                    EvaluatedPathVerb::Close => {}
                }
            }
        }
        if pts.len() >= 2 {
            let mid = pts.len() / 2;
            (
                (pts[mid - 1][0] + pts[mid][0]) * 0.5,
                (pts[mid - 1][1] + pts[mid][1]) * 0.5,
            )
        } else if let (Some(bx), Some(by), Some(ex), Some(ey)) = (
            ss.get_f64("BeginX"),
            ss.get_f64("BeginY"),
            ss.get_f64("EndX"),
            ss.get_f64("EndY"),
        ) {
            ((bx + ex) * 0.5, (by + ey) * 0.5)
        } else {
            world_tf.local_to_parent(w * 0.5, h * 0.5)
        }
    } else {
        let txt_lx = ss
            .get_f64("TxtPinX")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtPinX")))
            .unwrap_or(w * 0.5);
        let txt_ly = ss
            .get_f64("TxtPinY")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtPinY")))
            .unwrap_or(h * 0.5);
        let tw_ref = txt_width.unwrap_or(w);
        let th_ref = txt_height.unwrap_or(h);
        let txt_lpx = ss
            .get_f64("TxtLocPinX")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtLocPinX")))
            .unwrap_or(tw_ref * 0.5);
        let txt_lpy = ss
            .get_f64("TxtLocPinY")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtLocPinY")))
            .unwrap_or(th_ref * 0.5);
        let txt_ang = ss
            .get_f64("TxtAngle")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TxtAngle")))
            .unwrap_or(0.0);
        let (tsa, tca) = txt_ang.sin_cos();
        let off_x = tw_ref * 0.5 - txt_lpx;
        let off_y = th_ref * 0.5 - txt_lpy;
        let cx_l = txt_lx + off_x * tca - off_y * tsa;
        let cy_l = txt_ly + off_x * tsa + off_y * tca;
        txt_local_center = [cx_l, cy_l];
        world_tf.local_to_parent(cx_l, cy_l)
    };

    // Visio `[MS-VSDX]` rule: `TextBkgndTrans` defaults to `0.0` (`0%` transparent), while `TextBkgnd` defaults to `0` (`None`).
    let txt_bg_trans = ss
        .get_f64("TextBkgndTrans")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("TextBkgndTrans")))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let text_bg_color = if txt_bg_trans < 0.995 {
        ss.get_color("TextBkgnd")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_color("TextBkgnd")))
            .map(|mut c| {
                if txt_bg_trans > 0.001 {
                    c.a = (((1.0 - txt_bg_trans) * (c.a as f64)).round() as u8).max(1);
                }
                c
            })
    } else {
        None
    };

    let mut char_style = ss.get_f64("Char.Style").unwrap_or(0.0).round().clamp(0.0, 15.0) as u8;
    if ss.get_bool("Char.SmallCaps").unwrap_or(false) {
        char_style &= !8;
    }
    if ss.get_bool("Char.Strikethru").unwrap_or(false) {
        char_style |= 8;
    }

    let foreign_media = shape
        .foreign_data
        .clone()
        .or_else(|| master_shape.and_then(|ms| ms.foreign_data.clone()))
        .filter(|fm| !fm.data.is_empty());

    let hide_text = ss
        .get_bool("HideText")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_bool("HideText")))
        .unwrap_or(false);
    let effective_text = if hide_text {
        String::new()
    } else if !shape.text.is_empty() {
        shape.text.clone()
    } else if let Some(ms) = master_shape {
        let explicit_empty_text = shape
            .opaque_xml_fragments
            .iter()
            .any(|f| f.trim() == "<Text/>");
        if shape.children.is_empty() && !explicit_empty_text {
            ms.text.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Expand AABB to encompass external captions / connector labels so R*-tree frustum culling never clips text!
    if !effective_text.trim().is_empty() && txt_px.is_finite() && txt_py.is_finite() {
        let half_tw = txt_width.unwrap_or_else(|| w.abs().max(1.2)) * 0.5;
        let half_th = txt_height.unwrap_or(0.32) * 0.5;
        aabb_min[0] = aabb_min[0].min(txt_px - half_tw);
        aabb_min[1] = aabb_min[1].min(txt_py - half_th);
        aabb_max[0] = aabb_max[0].max(txt_px + half_tw);
        aabb_max[1] = aabb_max[1].max(txt_py + half_th);
    }

    let page_bg = page
        .page_sheet
        .get_color("PageColor")
        .or_else(|| {
            doc.active_theme_id
                .and_then(|tid| doc.themes.iter().find(|t| t.id == tid))
                .map(|t| t.background_color)
        })
        .unwrap_or(RgbaColor::WHITE);
    let page_lum =
        (0.2126 * (page_bg.r as f64) + 0.7152 * (page_bg.g as f64) + 0.0722 * (page_bg.b as f64)) / 255.0;
    let text_inside_shape_body = shape.kind == ShapeKind::Shape2D
        && w.abs() > 1e-4
        && h.abs() > 1e-4
        && txt_local_center[0] >= -0.04 * w.abs()
        && txt_local_center[0] <= 1.04 * w.abs()
        && txt_local_center[1] >= -0.04 * h.abs()
        && txt_local_center[1] <= 1.04 * h.abs();
    let has_dark_fill_behind_text = text_inside_shape_body
        && fill_color.is_some_and(|fc| {
            let fl = (0.2126 * (fc.r as f64) + 0.7152 * (fc.g as f64) + 0.0722 * (fc.b as f64)) / 255.0;
            fc.a >= 120 && fl < 0.48
        });

    let default_contrast_text = if page_lum < 0.38 {
        RgbaColor {
            r: 248,
            g: 250,
            b: 252,
            a: 255,
        }
    } else {
        let dk = active_theme.dark_color;
        let dk_lum = (0.2126 * (dk.r as f64) + 0.7152 * (dk.g as f64) + 0.0722 * (dk.b as f64)) / 255.0;
        if dk_lum < 0.55 {
            dk
        } else {
            RgbaColor::BLACK
        }
    };

    let fallback_text_color = if qs_font_mat > 0 || qs_fill_mat > 0 || qs_font_col.is_some() {
        let cand = resolve_quickstyle_font_color(&active_theme, qs_font_mat, qs_fill_mat, qs_font_col);
        let cand_lum = (0.2126 * (cand.r as f64) + 0.7152 * (cand.g as f64) + 0.0722 * (cand.b as f64)) / 255.0;
        if cand_lum > 0.85 && !has_dark_fill_behind_text && text_bg_color.is_none() && page_lum >= 0.55 {
            default_contrast_text
        } else {
            cand
        }
    } else {
        default_contrast_text
    };

    let resolved_text_color = match ss.get_color("Char.Color") {
        Some(c) => {
            let c_lum = (0.2126 * (c.r as f64) + 0.7152 * (c.g as f64) + 0.0722 * (c.b as f64)) / 255.0;
            let from_theme_or_inh = ss
                .get_cell("Char.Color")
                .map(|cell| {
                    let fu = cell.formula.to_ascii_uppercase();
                    fu.contains("THEME") || !fu.contains("RGB(")
                })
                .unwrap_or(true);
            if c_lum > 0.90
                && from_theme_or_inh
                && !has_dark_fill_behind_text
                && text_bg_color.is_none()
                && page_lum >= 0.65
            {
                default_contrast_text
            } else {
                c
            }
        }
        None => fallback_text_color,
    };

    // Evaluate world-space connection ports (`Connections.X{ix}` / `Connections.Y{ix}` + cardinal fallback for 2D shapes).
    let mut connection_points: Vec<EvaluatedConnectionPort> = Vec::new();
    if matches!(shape.kind, ShapeKind::Shape2D | ShapeKind::Group) && parent_tf.is_none() {
        let cp_rows = if !ss.connection_points.is_empty() {
            &ss.connection_points
        } else if let Some(ms) = master_shape.filter(|m| !m.sheet.connection_points.is_empty()) {
            &ms.sheet.connection_points
        } else {
            &ss.connection_points
        };
        for cp in cp_rows {
            let xk = format!("Connections.X{}", cp.ix);
            let yk = format!("Connections.Y{}", cp.ix);
            let lx = ss
                .get_f64(&xk)
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(&xk)))
                .unwrap_or(w * 0.5);
            let ly = ss
                .get_f64(&yk)
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(&yk)))
                .unwrap_or(h * 0.5);
            let (wx, wy) = world_tf.local_to_parent(lx, ly);
            if wx.is_finite() && wy.is_finite() {
                connection_points.push(EvaluatedConnectionPort {
                    ix: cp.ix,
                    cell_name: xk,
                    pos_page_in: [wx, wy],
                });
            }
        }
        if connection_points.is_empty() && w.abs() > 0.05 && h.abs() > 0.05 {
            for (ix, lx, ly) in [
                (1u32, w * 0.5, 0.0),
                (2u32, w * 0.5, h),
                (3u32, 0.0, h * 0.5),
                (4u32, w, h * 0.5),
            ] {
                let (wx, wy) = world_tf.local_to_parent(lx, ly);
                connection_points.push(EvaluatedConnectionPort {
                    ix,
                    cell_name: format!("Connections.X{ix}"),
                    pos_page_in: [wx, wy],
                });
            }
        }
    }

    let mut control_handles: Vec<EvaluatedControlHandle> = Vec::new();
    if matches!(shape.kind, ShapeKind::Shape2D | ShapeKind::Group) && parent_tf.is_none() {
        let ctrl_rows = if !ss.controls.is_empty() {
            &ss.controls
        } else if let Some(ms) = master_shape.filter(|m| !m.sheet.controls.is_empty()) {
            &ms.sheet.controls
        } else {
            &ss.controls
        };
        for ctrl in ctrl_rows {
            let xk = format!("Controls.{}.X", ctrl.name);
            let yk = format!("Controls.{}.Y", ctrl.name);
            let bk = format!("Controls.{}", ctrl.name);
            let lx = ss
                .get_f64(&xk)
                .or_else(|| ss.get_f64(&bk))
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(&xk)))
                .unwrap_or(w * 0.5);
            let ly = ss
                .get_f64(&yk)
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(&yk)))
                .unwrap_or(h * 0.5);
            let (wx, wy) = world_tf.local_to_parent(lx, ly);
            if wx.is_finite() && wy.is_finite() {
                control_handles.push(EvaluatedControlHandle {
                    name: ctrl.name.clone(),
                    pos_page_in: [wx, wy],
                    can_glue: ctrl.can_glue,
                    prompt: ctrl.prompt.clone(),
                });
            }
        }
    }

    let is_swimlane = ss.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5
        || master_shape
            .and_then(|ms| ms.sheet.get_f64("User.visSwimlane"))
            .unwrap_or(0.0)
            > 0.5;

    let end_arrow_size = ss
        .get_f64("EndArrowSize")
        .unwrap_or(2.0)
        .round()
        .clamp(0.0, 6.0) as u8;
    let begin_arrow_size = ss
        .get_f64("BeginArrowSize")
        .unwrap_or(2.0)
        .round()
        .clamp(0.0, 6.0) as u8;

    out.push(EvaluatedShapeVisual {
        shape_id: shape.id,
        kind: shape.kind,
        transform: world_tf,
        aabb_min,
        aabb_max,
        paths,
        fill_color,
        fill_bkgnd,
        fill_pattern: fill_pat.clamp(0, 255) as u8,
        fill_gradient_angle_rad,
        fill_gradient_radial,
        gradient_stops,
        stroke_color,
        stroke_weight_in: ss.get_f64("LineWeight").unwrap_or(1.0 / 72.0),
        line_pattern: line_pat.clamp(0, 255) as u8,
        rounding_in: ss.get_f64("Rounding").unwrap_or(0.0),
        end_arrow: ss.get_f64("EndArrow").unwrap_or(0.0) as u8,
        begin_arrow: ss.get_f64("BeginArrow").unwrap_or(0.0) as u8,
        end_arrow_size,
        begin_arrow_size,
        text: effective_text,
        txt_pos: [txt_px, txt_py],
        txt_width,
        txt_height,
        font_size_pt: ss
            .get_f64("Char.Size")
            .map(|inches| inches * 72.0)
            .unwrap_or(10.0),
        text_color: resolved_text_color,
        font_family: ss
            .get_str("Char.Font")
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| active_theme.body_font.clone()),
        char_style,
        horiz_align: ss
            .get_f64("Para.HorzAlign")
            .unwrap_or(1.0)
            .round()
            .clamp(0.0, 3.0) as u8,
        vert_align: ss
            .get_f64("VerticalAlign")
            .unwrap_or(1.0)
            .round()
            .clamp(0.0, 2.0) as u8,
        bullet_style: ss
            .get_f64("Para.Bullet")
            .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("Para.Bullet")))
            .unwrap_or(0.0)
            .round()
            .clamp(0.0, 4.0) as u8,
        text_bg_color,
        foreign_media,
        connection_points,
        control_handles,
        is_swimlane,
    });

    if !shape.children.is_empty() {
        for child in &shape.children {
            collect_shape_visuals(doc, page, child, Some(&world_tf), out);
        }
    } else if let Some(ms) = master_shape {
        for child in &ms.children {
            collect_shape_visuals(doc, page, child, Some(&world_tf), out);
        }
    }
}

fn quickstyle_base_color(theme: &crate::model::ThemeDef, matrix: i32, color_idx: Option<i32>) -> RgbaColor {
    if let Some(ci) = color_idx {
        match ci {
            0 | 106 => return theme.dark_color,
            1 | 107 => return theme.light_color,
            2..=7 => return theme.accent_colors[(ci - 2) as usize],
            100..=105 => return theme.accent_colors[((ci - 100) as usize) % 6],
            _ => {}
        }
    }
    if (1..=36).contains(&matrix) {
        return theme.accent_colors[((matrix - 1) as usize) % 6];
    }
    if (100..=105).contains(&matrix) {
        return theme.accent_colors[((matrix - 100) as usize) % 6];
    }
    theme.accent_colors[0]
}

fn tint_rgb(c: RgbaColor, amount: f64) -> RgbaColor {
    let t = amount.clamp(0.0, 1.0);
    RgbaColor {
        r: ((c.r as f64) + (255.0 - c.r as f64) * t).round().clamp(0.0, 255.0) as u8,
        g: ((c.g as f64) + (255.0 - c.g as f64) * t).round().clamp(0.0, 255.0) as u8,
        b: ((c.b as f64) + (255.0 - c.b as f64) * t).round().clamp(0.0, 255.0) as u8,
        a: c.a,
    }
}

fn shade_rgb(c: RgbaColor, amount: f64) -> RgbaColor {
    let f = (1.0 - amount.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    RgbaColor {
        r: ((c.r as f64) * f).round().clamp(0.0, 255.0) as u8,
        g: ((c.g as f64) * f).round().clamp(0.0, 255.0) as u8,
        b: ((c.b as f64) * f).round().clamp(0.0, 255.0) as u8,
        a: c.a,
    }
}

fn resolve_quickstyle_fill_color(
    theme: &crate::model::ThemeDef,
    matrix: i32,
    color_idx: Option<i32>,
) -> RgbaColor {
    if matrix <= 0 && color_idx.is_none() {
        return theme.light_color;
    }
    let base = quickstyle_base_color(theme, matrix, color_idx);
    match matrix {
        1..=6 => tint_rgb(base, 0.82),
        7..=12 => tint_rgb(base, 0.90),
        13..=18 | 100 | 101 => base,
        19..=24 | 102 | 103 => shade_rgb(base, 0.24),
        25..=30 => tint_rgb(base, 0.74),
        31..=36 => base,
        _ => base,
    }
}

fn resolve_quickstyle_fill_bkgnd(
    theme: &crate::model::ThemeDef,
    matrix: i32,
    color_idx: Option<i32>,
) -> RgbaColor {
    let base = quickstyle_base_color(theme, matrix, color_idx);
    match matrix {
        25..=30 => tint_rgb(base, 0.94),
        31..=36 => shade_rgb(base, 0.28),
        _ => tint_rgb(base, 0.88),
    }
}

fn resolve_quickstyle_line_color(
    theme: &crate::model::ThemeDef,
    matrix: i32,
    color_idx: Option<i32>,
) -> RgbaColor {
    if matrix <= 0 && color_idx.is_none() {
        return theme.dark_color;
    }
    let base = quickstyle_base_color(theme, matrix, color_idx);
    match matrix {
        1..=6 => shade_rgb(base, 0.15),
        7..=12 | 25..=30 => base,
        13..=18 => shade_rgb(base, 0.22),
        19..=24 | 31..=36 => shade_rgb(base, 0.36),
        _ => base,
    }
}

fn resolve_quickstyle_font_color(
    theme: &crate::model::ThemeDef,
    font_matrix: i32,
    fill_matrix: i32,
    color_idx: Option<i32>,
) -> RgbaColor {
    if let Some(ci) = color_idx {
        match ci {
            0 | 106 => return theme.dark_color,
            1 | 107 => return theme.light_color,
            2..=7 => return shade_rgb(theme.accent_colors[(ci - 2) as usize], 0.35),
            _ => {}
        }
    }
    let fill = resolve_quickstyle_fill_color(theme, fill_matrix, None);
    let lum = (0.2126 * (fill.r as f64) + 0.7152 * (fill.g as f64) + 0.0722 * (fill.b as f64)) / 255.0;
    if matches!(fill_matrix, 13..=24 | 31..=36 | 100..=103) || lum < 0.45 {
        RgbaColor::WHITE
    } else if (7..=12).contains(&font_matrix) {
        shade_rgb(quickstyle_base_color(theme, font_matrix, None), 0.45)
    } else {
        theme.dark_color
    }
}

/// Resolve a `Sheet.<target_sid>!` reference inside an instantiated Master group subtree
/// when `<target_sid>` refers to a Master shape ID (`master_shape_id`) within the same group instance.
fn resolve_master_instance_sibling_id(
    doc: &VisioDocument,
    page: &VisioPage,
    ctx_shape_id: u64,
    target_sid: u64,
) -> Option<u64> {
    let top_root = page.shapes.iter().find(|s| s.find_shape(ctx_shape_id).is_some())?;
    // If `target_sid` already exists directly inside `top_root`, keep it!
    if top_root.find_shape(target_sid).is_some() {
        return Some(target_sid);
    }
    let ctx_shape = top_root.find_shape(ctx_shape_id)?;
    let mid = ctx_shape.master_id.or(top_root.master_id)?;
    let master_root_id = doc.master(mid).and_then(|m| m.shapes.first().map(|s| s.id));
    fn find_by_msid(node: &VisioShape, target_msid: u64, root_msid: Option<u64>, is_root: bool) -> Option<u64> {
        if node.master_shape_id == Some(target_msid)
            || (is_root && node.master_shape_id.is_none() && root_msid == Some(target_msid))
        {
            return Some(node.id);
        }
        for c in &node.children {
            if let Some(found) = find_by_msid(c, target_msid, root_msid, false) {
                return Some(found);
            }
        }
        None
    }
    find_by_msid(top_root, target_sid, master_root_id, true)
}

/// Analytically decompose a Visio `[MS-VSDX]` `EllipticalArcTo` / `RelEllipticalArcTo` segment
/// (`p0 -> pm -> p1`, major-axis angle `c_rad`, aspect ratio `d_ratio = rx / ry`) in local shape inches
/// into exact cubic Bézier curves (`<= 45°` per segment).
fn decompose_visio_elliptical_arc_local_cubics(
    p0: [f64; 2],
    pm: [f64; 2],
    p1: [f64; 2],
    c_rad: f64,
    d_ratio: f64,
    shape_w: f64,
    shape_h: f64,
) -> Vec<([f64; 2], [f64; 2], [f64; 2])> {
    if !p0[0].is_finite()
        || !p0[1].is_finite()
        || !pm[0].is_finite()
        || !pm[1].is_finite()
        || !p1[0].is_finite()
        || !p1[1].is_finite()
    {
        return Vec::new();
    }
    let chord_local = (p1[0] - p0[0]).hypot(p1[1] - p0[1]);
    if chord_local < 1e-7 {
        return Vec::new();
    }
    let c = if c_rad.is_finite() { c_rad } else { 0.0 };
    let d = if d_ratio.is_finite() && d_ratio.abs() >= 1e-4 {
        d_ratio.abs().clamp(1e-4, 1e4)
    } else {
        1.0
    };
    let (sc, cc) = c.sin_cos();
    let to_uv = |pt: [f64; 2]| -> [f64; 2] {
        let u_raw = pt[0] * cc + pt[1] * sc;
        let v_raw = -pt[0] * sc + pt[1] * cc;
        [u_raw, v_raw * d]
    };
    let from_uv = |uv: [f64; 2]| -> [f64; 2] {
        let u_raw = uv[0];
        let v_raw = uv[1] / d;
        [u_raw * cc - v_raw * sc, u_raw * sc + v_raw * cc]
    };

    let q0 = to_uv(p0);
    let qm = to_uv(pm);
    let q1 = to_uv(p1);
    let chord_uv = (q1[0] - q0[0]).hypot(q1[1] - q0[1]);
    if chord_uv < 1e-7 {
        return Vec::new();
    }

    let det = 2.0
        * (q0[0] * (qm[1] - q1[1]) + qm[0] * (q1[1] - q0[1]) + q1[0] * (q0[1] - qm[1]));
    if det.abs() < 1e-9 {
        return Vec::new();
    }
    let a2 = q0[0] * q0[0] + q0[1] * q0[1];
    let b2 = qm[0] * qm[0] + qm[1] * qm[1];
    let c2 = q1[0] * q1[0] + q1[1] * q1[1];
    let uc = (a2 * (qm[1] - q1[1]) + b2 * (q1[1] - q0[1]) + c2 * (q0[1] - qm[1])) / det;
    let vc = (a2 * (q1[0] - qm[0]) + b2 * (q0[0] - q1[0]) + c2 * (qm[0] - q0[0])) / det;
    let r = (q0[0] - uc).hypot(q0[1] - vc);
    if !r.is_finite() || r < 1e-7 || r > chord_uv * 250.0 {
        return Vec::new();
    }

    let a0 = (q0[1] - vc).atan2(q0[0] - uc);
    let amid = (qm[1] - vc).atan2(qm[0] - uc);
    let a1 = (q1[1] - vc).atan2(q1[0] - uc);
    let ccw_end = (a1 - a0).rem_euclid(std::f64::consts::TAU);
    let ccw_mid = (amid - a0).rem_euclid(std::f64::consts::TAU);
    let sweep = if ccw_mid <= ccw_end + 1e-9 {
        ccw_end
    } else {
        ccw_end - std::f64::consts::TAU
    };

    // Guard against degenerate near-collinear control-point rounding that flips a tiny minor arc
    // into a >180° reflex balloon extending multiples of the shape size outside local bounds!
    if sweep.abs() > std::f64::consts::PI && r > chord_uv * 1.35 {
        let apex_ang = a0 + sweep * 0.5;
        let apex_local = from_uv([uc + r * apex_ang.cos(), vc + r * apex_ang.sin()]);
        let ref_span = shape_w.abs().max(shape_h.abs()).max(chord_local).max(0.25);
        let mid_chord = [(p0[0] + p1[0]) * 0.5, (p0[1] + p1[1]) * 0.5];
        let apex_dev = (apex_local[0] - mid_chord[0]).hypot(apex_local[1] - mid_chord[1]);
        let pm_dev = (pm[0] - mid_chord[0]).hypot(pm[1] - mid_chord[1]);
        if apex_dev > ref_span * 1.75 && pm_dev < apex_dev * 0.35 {
            // Smooth quadratic-equivalent cubic passing through `pm` without reflex explosion
            let q_ctrl = [
                2.0 * pm[0] - 0.5 * (p0[0] + p1[0]),
                2.0 * pm[1] - 0.5 * (p0[1] + p1[1]),
            ];
            let c1 = [
                p0[0] + (2.0 / 3.0) * (q_ctrl[0] - p0[0]),
                p0[1] + (2.0 / 3.0) * (q_ctrl[1] - p0[1]),
            ];
            let c2 = [
                p1[0] + (2.0 / 3.0) * (q_ctrl[0] - p1[0]),
                p1[1] + (2.0 / 3.0) * (q_ctrl[1] - p1[1]),
            ];
            return vec![(c1, c2, p1)];
        }
    }

    let n_segs = ((sweep.abs() / std::f64::consts::FRAC_PI_4).ceil() as usize).clamp(1, 24);
    let dt = sweep / (n_segs as f64);
    let k = (4.0 / 3.0) * (dt * 0.25).tan();
    let mut out = Vec::with_capacity(n_segs);
    for i in 0..n_segs {
        let t0 = a0 + (i as f64) * dt;
        let t1 = if i + 1 == n_segs { a0 + sweep } else { t0 + dt };
        let (s0, c0) = t0.sin_cos();
        let (s1, c1) = t1.sin_cos();
        let su = uc + r * c0;
        let sv = vc + r * s0;
        let eu = uc + r * c1;
        let ev = vc + r * s1;
        let ctrl1_uv = [su - k * r * s0, sv + k * r * c0];
        let ctrl2_uv = [eu + k * r * s1, ev - k * r * c1];
        let end_local = if i + 1 == n_segs {
            p1
        } else {
            from_uv([eu, ev])
        };
        out.push((from_uv(ctrl1_uv), from_uv(ctrl2_uv), end_local));
    }
    out
}

fn extract_func_arg_f64s(expr_str: &str, func_name: &str) -> Option<Vec<f64>> {
    let trimmed = expr_str.trim().trim_start_matches('=').trim().trim_matches('"').trim();
    let upper = trimmed.to_ascii_uppercase();
    let prefix = format!("{func_name}(");
    let start = upper.find(&prefix)? + prefix.len();
    let end = trimmed.rfind(')')?;
    if end <= start {
        return None;
    }
    let body = &trimmed[start..end];
    let mut vals = Vec::new();
    for part in body.split(|c: char| c == ',' || c == ';') {
        let tok = part
            .trim()
            .trim_end_matches("in")
            .trim_end_matches("mm")
            .trim_end_matches("cm")
            .trim_end_matches("pt")
            .trim();
        let v = tok.parse::<f64>().ok().filter(|n| n.is_finite())?;
        vals.push(v);
    }
    Some(vals)
}

/// Parse Visio `POLYLINE(xType, yType, x1, y1, ...)` into local shape coordinates `(lx, ly)` in inches.
fn parse_visio_polyline_local_points(expr_str: &str, w: f64, h: f64) -> Vec<[f64; 2]> {
    let Some(args) = extract_func_arg_f64s(expr_str, "POLYLINE") else {
        return Vec::new();
    };
    if args.len() < 4 {
        return Vec::new();
    }
    let x_scale = if args[0].round() as i32 == 0 { w } else { 1.0 };
    let y_scale = if args[1].round() as i32 == 0 { h } else { 1.0 };
    let mut pts = Vec::with_capacity((args.len() - 2) / 2);
    for pair in args[2..].chunks_exact(2) {
        pts.push([pair[0] * x_scale, pair[1] * y_scale]);
    }
    pts
}

/// Evaluate Visio `[MS-VSDX]` `NURBSTo` (`E = NURBS(knotLast, degree, xType, yType, x1, y1, knot1, weight1, ...)`)
/// via rational Cox-de Boor B-spline interpolation in local shape inches.
#[allow(clippy::too_many_arguments)]
fn evaluate_visio_nurbs_local_points(
    p0_local: [f64; 2],
    end_local: [f64; 2],
    knot_first: f64,
    weight_first: f64,
    knot_prev: f64,
    weight_last: f64,
    expr_str: &str,
    w: f64,
    h: f64,
) -> Vec<[f64; 2]> {
    let Some(args) = extract_func_arg_f64s(expr_str, "NURBS") else {
        return Vec::new();
    };
    if args.len() < 8 {
        return Vec::new();
    }
    let knot_last = args[0];
    let degree = (args[1].round() as usize).clamp(1, 8);
    let x_scale = if args[2].round() as i32 == 0 { w } else { 1.0 };
    let y_scale = if args[3].round() as i32 == 0 { h } else { 1.0 };

    let tuples = &args[4..];
    let m = tuples.len() / 4;
    if m == 0 {
        return Vec::new();
    }

    let mut ctrl_pts = Vec::with_capacity(m + 2);
    let mut weights = Vec::with_capacity(m + 2);
    let mut base_knots = Vec::with_capacity(m + 3);

    ctrl_pts.push(p0_local);
    weights.push(if weight_first.is_finite() && weight_first > 1e-6 {
        weight_first
    } else {
        1.0
    });
    base_knots.push(knot_first);

    for i in 0..m {
        let base = i * 4;
        ctrl_pts.push([tuples[base] * x_scale, tuples[base + 1] * y_scale]);
        base_knots.push(tuples[base + 2]);
        let wt = tuples[base + 3];
        weights.push(if wt.is_finite() && wt > 1e-6 { wt } else { 1.0 });
    }

    ctrl_pts.push(end_local);
    weights.push(if weight_last.is_finite() && weight_last > 1e-6 {
        weight_last
    } else {
        1.0
    });
    base_knots.push(knot_prev);
    base_knots.push(knot_last);

    sample_rational_bspline(&ctrl_pts, &weights, &base_knots, degree)
}

/// Evaluate a clamped rational B-spline (`ctrl_pts`, `weights`, `raw_knots`, `degree`) using Cox-de Boor.
fn sample_rational_bspline(
    ctrl_pts: &[[f64; 2]],
    weights: &[f64],
    raw_knots: &[f64],
    degree: usize,
) -> Vec<[f64; 2]> {
    let n = ctrl_pts.len();
    if n < 2 {
        return Vec::new();
    }
    let p = degree.min(n - 1).max(1);
    let needed_knots = n + p + 1;

    let mut knots = raw_knots.to_vec();
    let monotonic = knots.windows(2).all(|w| w[0].is_finite() && w[1].is_finite() && w[1] >= w[0] - 1e-9)
        && knots.last().copied().unwrap_or(0.0) > knots.first().copied().unwrap_or(0.0) + 1e-9;

    if monotonic && !knots.is_empty() && knots.len() <= needed_knots {
        let k_start = knots[0];
        let k_end = *knots.last().unwrap();
        // Visio omits the extra clamped boundary multiplicities at the start/end of `NURBS(...)`:
        // pad start and end symmetrically to reach `n + p + 1` knots.
        let mut pad_front = true;
        while knots.len() < needed_knots {
            if pad_front {
                knots.insert(0, k_start);
            } else {
                knots.push(k_end);
            }
            pad_front = !pad_front;
        }
    } else {
        knots.clear();
        for _ in 0..=p {
            knots.push(0.0);
        }
        let internal = n - p - 1;
        for j in 1..=internal {
            knots.push((j as f64) / ((internal + 1) as f64));
        }
        for _ in 0..=p {
            knots.push(1.0);
        }
    }

    let t_min = knots[p];
    let t_max = knots[n];
    if !(t_max > t_min + 1e-9) {
        return vec![*ctrl_pts.last().unwrap()];
    }

    let steps = (n * 6).clamp(12, 96);
    let mut out = Vec::with_capacity(steps);
    let mut basis = vec![0.0f64; n];

    for step in 1..=steps {
        if step == steps {
            out.push(*ctrl_pts.last().unwrap());
            break;
        }
        let t = t_min + (t_max - t_min) * ((step as f64) / (steps as f64));
        // Cox-de Boor basis evaluation `N_{i, p}(t)`
        let mut n_cur = vec![0.0f64; knots.len() - 1];
        for i in 0..n_cur.len() {
            if t >= knots[i] && t < knots[i + 1] {
                n_cur[i] = 1.0;
            }
        }
        for d in 1..=p {
            let max_i = knots.len() - 1 - d;
            let mut n_next = vec![0.0f64; max_i];
            for i in 0..max_i {
                let d1 = knots[i + d] - knots[i];
                let t1 = if d1 > 1e-12 {
                    ((t - knots[i]) / d1) * n_cur[i]
                } else {
                    0.0
                };
                let d2 = knots[i + d + 1] - knots[i + 1];
                let t2 = if d2 > 1e-12 {
                    ((knots[i + d + 1] - t) / d2) * n_cur[i + 1]
                } else {
                    0.0
                };
                n_next[i] = t1 + t2;
            }
            n_cur = n_next;
        }
        basis.copy_from_slice(&n_cur[..n]);

        let mut wx_sum = 0.0;
        let mut wy_sum = 0.0;
        let mut w_sum = 0.0;
        for i in 0..n {
            let bw = basis[i] * weights.get(i).copied().unwrap_or(1.0);
            wx_sum += bw * ctrl_pts[i][0];
            wy_sum += bw * ctrl_pts[i][1];
            w_sum += bw;
        }
        if w_sum > 1e-12 {
            out.push([wx_sum / w_sum, wy_sum / w_sum]);
        }
    }
    out
}



