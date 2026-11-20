//! Constructive 2D Boolean Shape Operations (`Union`, `Combine`, `Fragment`, `Intersect`,
//! `Subtract`, `Trim`, `Join`, `Offset`), Interactive Vertex / Bezier Editing, Freeform Curve
//! Fitting (Ramer-Douglas-Peucker + Catmull-Rom to `RelCubBezTo`), and Draggable Yellow Diamond
//! Control Handles (`Controls.*`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::{PI, TAU};
use vexio_shapesheet::{
    ControlRow, EvaluatedGeometryPath, EvaluatedPathVerb, EvaluatedShapeVisual, GeometryRow,
    GeometryRowKind, GeometrySection, ReactiveEngine, ShapeKind, ShapeSheet, ShapeTransform2D,
    Unit, VisioDocument, VisioPage, VisioShape, canonical_cell_name, format_clean_f64,
    parse_scalar_with_unit,
};

const SNAP_EPS: f64 = 1e-5;
const AREA_EPS: f64 = 1e-7;
const JOIN_TOLERANCE_IN: f64 = 0.05;
const RDP_EPSILON_IN: f64 = 0.02;
const MITER_LIMIT: f64 = 4.0;

/// Visio Shape Design -> Operations constructive geometry modes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BooleanOpKind {
    Union,
    Combine,
    Fragment,
    Intersect,
    Subtract,
    Trim,
    Join,
    Offset(f64),
}

/// Page-space editable geometry vertex handle for interactive vertex/path editing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditableVertex {
    pub geom_index: u32,
    pub row_ix: u32,
    pub kind: String,
    pub page_x_in: f64,
    pub page_y_in: f64,
}

/// Page-space yellow diamond `Controls.*` handle descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlHandleInfo {
    pub name: String,
    pub page_x_in: f64,
    pub page_y_in: f64,
    pub prompt: String,
    pub can_glue: bool,
}

#[derive(Clone, Debug)]
struct PrimaryStyleSnapshot {
    name_base: String,
    fill_foregnd: String,
    fill_bkgnd: String,
    fill_pattern: String,
    line_weight: String,
    line_color: String,
    line_pattern: String,
    char_size: String,
    char_color: String,
    layer_membership: Vec<u32>,
}

impl PrimaryStyleSnapshot {
    fn capture(shape: &VisioShape) -> Self {
        let cell_formula = |key: &str, fallback: &str| -> String {
            shape
                .sheet
                .get_cell(key)
                .map(|c| c.formula.clone())
                .unwrap_or_else(|| fallback.to_string())
        };
        Self {
            name_base: if shape.name_u.is_empty() {
                "Shape".to_string()
            } else {
                shape.name_u.clone()
            },
            fill_foregnd: cell_formula("FillForegnd", "=RGB(255,255,255)"),
            fill_bkgnd: cell_formula("FillBkgnd", "=RGB(240,244,248)"),
            fill_pattern: cell_formula("FillPattern", "1"),
            line_weight: cell_formula("LineWeight", "1 pt"),
            line_color: cell_formula("LineColor", "=RGB(30,41,59)"),
            line_pattern: cell_formula("LinePattern", "1"),
            char_size: cell_formula("Char.Size", "10 pt"),
            char_color: cell_formula("Char.Color", "=RGB(15,23,42)"),
            layer_membership: shape.layer_membership.clone(),
        }
    }

    fn apply_to(&self, shape: &mut VisioShape, is_open_curve: bool) {
        shape
            .sheet
            .set_formula("FillForegnd", self.fill_foregnd.clone(), Some(Unit::Color));
        shape
            .sheet
            .set_formula("FillBkgnd", self.fill_bkgnd.clone(), Some(Unit::Color));
        shape.sheet.set_formula(
            "FillPattern",
            if is_open_curve {
                "0".to_string()
            } else if self.fill_pattern.trim() == "0" {
                "1".to_string()
            } else {
                self.fill_pattern.clone()
            },
            Some(Unit::Number),
        );
        shape
            .sheet
            .set_formula("LineWeight", self.line_weight.clone(), Some(Unit::Point));
        shape
            .sheet
            .set_formula("LineColor", self.line_color.clone(), Some(Unit::Color));
        shape
            .sheet
            .set_formula("LinePattern", self.line_pattern.clone(), Some(Unit::Number));
        shape
            .sheet
            .set_formula("Char.Size", self.char_size.clone(), Some(Unit::Point));
        shape
            .sheet
            .set_formula("Char.Color", self.char_color.clone(), Some(Unit::Color));
        shape.layer_membership = self.layer_membership.clone();
    }
}

#[derive(Clone, Debug)]
struct FlattenedCurve {
    points: Vec<[f64; 2]>,
    closed: bool,
}

#[derive(Clone, Debug)]
struct ShapeCurveSet {
    closed_rings: Vec<Vec<[f64; 2]>>,
    all_curves: Vec<FlattenedCurve>,
}

/// Apply a constructive 2D boolean or path operation (`Union`, `Combine`, `Fragment`, `Intersect`,
/// `Subtract`, `Trim`, `Join`, `Offset`) to `shape_ids` on `page_id`, returning the newly created
/// `VisioShape` IDs in deterministic creation order.
pub fn apply_boolean_operation(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_ids: &[u64],
    op: BooleanOpKind,
) -> Vec<u64> {
    if shape_ids.is_empty() || doc.page(page_id).is_none() {
        return Vec::new();
    }

    ReactiveEngine::recalculate_document(doc);
    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    let mut visual_by_id: BTreeMap<u64, EvaluatedShapeVisual> = BTreeMap::new();
    for v in visuals {
        visual_by_id.insert(v.shape_id, v);
    }

    let mut ordered_ids = Vec::new();
    let mut seen_ids = BTreeSet::new();
    {
        let Some(page) = doc.page(page_id) else {
            return Vec::new();
        };
        for &sid in shape_ids {
            if seen_ids.insert(sid) && page.find_shape(sid).is_some() {
                ordered_ids.push(sid);
            }
        }
    }
    if ordered_ids.is_empty() {
        return Vec::new();
    }

    let style_snapshot = {
        let page = doc.page(page_id).expect("verified page");
        let primary = page.find_shape(ordered_ids[0]).expect("verified primary");
        PrimaryStyleSnapshot::capture(primary)
    };

    let mut shape_curves: Vec<ShapeCurveSet> = Vec::with_capacity(ordered_ids.len());
    for &sid in &ordered_ids {
        if let Some(vis) = visual_by_id.get(&sid) {
            shape_curves.push(flatten_visual_paths(vis));
        } else {
            shape_curves.push(ShapeCurveSet {
                closed_rings: Vec::new(),
                all_curves: Vec::new(),
            });
        }
    }

    // Each entry in `output_specs` represents one new `VisioShape` composed of `(points, closed)` sections.
    let output_specs: Vec<Vec<(Vec<[f64; 2]>, bool)>> = match op {
        BooleanOpKind::Union
        | BooleanOpKind::Combine
        | BooleanOpKind::Fragment
        | BooleanOpKind::Intersect
        | BooleanOpKind::Subtract => compute_planar_boolean_regions(&shape_curves, op),
        BooleanOpKind::Trim => compute_trimmed_subcurves(&shape_curves),
        BooleanOpKind::Join => compute_joined_curves(&shape_curves),
        BooleanOpKind::Offset(dist_in) => compute_offset_curves(&shape_curves, dist_in),
    };

    let replace_sources = !matches!(op, BooleanOpKind::Offset(_));
    let mut created_ids = Vec::with_capacity(output_specs.len());

    if let Some(page) = doc.page_mut(page_id) {
        if replace_sources {
            remove_shapes_by_id_set(&mut page.shapes, &seen_ids);
            page.connects.retain(|c| {
                !seen_ids.contains(&c.connector_shape_id) && !seen_ids.contains(&c.target_shape_id)
            });
        }

        for (idx, sections) in output_specs.into_iter().enumerate() {
            if sections.is_empty() {
                continue;
            }
            let new_id = page.next_shape_id();
            let op_label = match op {
                BooleanOpKind::Union => "Union",
                BooleanOpKind::Combine => "Combine",
                BooleanOpKind::Fragment => "Fragment",
                BooleanOpKind::Intersect => "Intersect",
                BooleanOpKind::Subtract => "Subtract",
                BooleanOpKind::Trim => "Trim",
                BooleanOpKind::Join => "Join",
                BooleanOpKind::Offset(_) => "Offset",
            };
            let shape_name = if idx == 0 {
                format!("{}.{op_label}.{new_id}", style_snapshot.name_base)
            } else {
                format!("{}.{op_label}.{new_id}.{}", style_snapshot.name_base, idx + 1)
            };
            if let Some(mut new_shape) = build_shape_from_world_sections(new_id, &shape_name, &sections) {
                let all_open = sections.iter().all(|(_, closed)| !*closed);
                style_snapshot.apply_to(&mut new_shape, all_open);
                page.shapes.push(new_shape);
                created_ids.push(new_id);
            }
        }
    }

    ReactiveEngine::recalculate_document(doc);
    created_ids
}

/// List all editable vertices across active `Geometry` sections of `(page_id, shape_id)` in page inches.
pub fn list_shape_vertices(doc: &VisioDocument, page_id: u32, shape_id: u64) -> Vec<EditableVertex> {
    let Some(page) = doc.page(page_id) else {
        return Vec::new();
    };
    let Some((shape, world_tf)) = find_shape_with_world_transform(page, shape_id) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for geom in &shape.sheet.geometries {
        if geom.deleted {
            continue;
        }
        let prefix = format!("Geometry{}", geom.index);
        for row in &geom.rows {
            if row.deleted {
                continue;
            }
            let raw_x = eval_or_parse_row_coord(&shape.sheet, &format!("{prefix}.X{}", row.ix), &row.x);
            let raw_y = eval_or_parse_row_coord(&shape.sheet, &format!("{prefix}.Y{}", row.ix), &row.y);
            let (lx, ly) = if is_relative_row_kind(row.kind) {
                (raw_x * world_tf.width, raw_y * world_tf.height)
            } else {
                (raw_x, raw_y)
            };
            let (px, py) = world_tf.local_to_parent(lx, ly);
            out.push(EditableVertex {
                geom_index: geom.index,
                row_ix: row.ix,
                kind: row.kind.vsdx_tag().to_string(),
                page_x_in: px,
                page_y_in: py,
            });
        }
    }
    out
}

/// Move a specific vertex `(geom_index, row_ix)` on `(page_id, shape_id)` to `(new_page_x_in, new_page_y_in)`.
pub fn move_shape_vertex(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    geom_index: u32,
    row_ix: u32,
    new_page_x_in: f64,
    new_page_y_in: f64,
) -> bool {
    if !new_page_x_in.is_finite() || !new_page_y_in.is_finite() {
        return false;
    }
    let world_tf = {
        let Some(page) = doc.page(page_id) else {
            return false;
        };
        let Some((_, tf)) = find_shape_with_world_transform(page, shape_id) else {
            return false;
        };
        tf
    };

    let (lx, ly) = world_tf.parent_to_local(new_page_x_in, new_page_y_in);

    let updated = {
        let Some(page) = doc.page_mut(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape_mut(shape_id) else {
            return false;
        };
        let x_cell_name = format!("Geometry{geom_index}.X{row_ix}");
        let y_cell_name = format!("Geometry{geom_index}.Y{row_ix}");
        if shape.sheet.get_cell(&x_cell_name).is_some_and(|c| c.is_guarded())
            || shape.sheet.get_cell(&y_cell_name).is_some_and(|c| c.is_guarded())
        {
            return false;
        }

        let Some(geom) = shape
            .sheet
            .geometries
            .iter_mut()
            .find(|g| g.index == geom_index && !g.deleted)
        else {
            return false;
        };
        let Some(row) = geom.rows.iter_mut().find(|r| r.ix == row_ix && !r.deleted) else {
            return false;
        };

        if is_relative_row_kind(row.kind) {
            let rx = if world_tf.width.abs() > 1e-9 {
                lx / world_tf.width
            } else {
                0.0
            };
            let ry = if world_tf.height.abs() > 1e-9 {
                ly / world_tf.height
            } else {
                0.0
            };
            row.x = format_clean_f64(rx);
            row.y = format_clean_f64(ry);
        } else {
            row.x = format!("{} in", format_clean_f64(lx));
            row.y = format!("{} in", format_clean_f64(ly));
        }

        purge_geometry_cells(&mut shape.sheet, geom_index);
        shape.sheet.sync_all_sections_to_cells();
        true
    };

    if updated {
        ReactiveEngine::recalculate_document(doc);
    }
    updated
}

/// Insert a new vertex after `after_row_ix` in `Geometry{geom_index}` at `(page_x_in, page_y_in)`.
pub fn insert_shape_vertex(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    geom_index: u32,
    after_row_ix: u32,
    page_x_in: f64,
    page_y_in: f64,
) -> bool {
    if !page_x_in.is_finite() || !page_y_in.is_finite() {
        return false;
    }
    let world_tf = {
        let Some(page) = doc.page(page_id) else {
            return false;
        };
        let Some((_, tf)) = find_shape_with_world_transform(page, shape_id) else {
            return false;
        };
        tf
    };

    let (lx, ly) = world_tf.parent_to_local(page_x_in, page_y_in);

    let inserted = {
        let Some(page) = doc.page_mut(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape_mut(shape_id) else {
            return false;
        };
        let Some(geom) = shape
            .sheet
            .geometries
            .iter_mut()
            .find(|g| g.index == geom_index && !g.deleted)
        else {
            return false;
        };
        let Some(pos) = geom
            .rows
            .iter()
            .position(|r| r.ix == after_row_ix && !r.deleted)
        else {
            return false;
        };

        let use_relative = is_relative_row_kind(geom.rows[pos].kind);
        let new_row = if use_relative {
            let rx = if world_tf.width.abs() > 1e-9 {
                lx / world_tf.width
            } else {
                0.0
            };
            let ry = if world_tf.height.abs() > 1e-9 {
                ly / world_tf.height
            } else {
                0.0
            };
            GeometryRow {
                ix: 0,
                kind: GeometryRowKind::RelLineTo,
                x: format_clean_f64(rx),
                y: format_clean_f64(ry),
                a: None,
                b: None,
                c: None,
                d: None,
                e: None,
                deleted: false,
            }
        } else {
            GeometryRow::line_to(
                0,
                format!("{} in", format_clean_f64(lx)),
                format!("{} in", format_clean_f64(ly)),
            )
        };

        geom.rows.insert(pos + 1, new_row);
        renumber_geometry_rows(&mut geom.rows);
        purge_geometry_cells(&mut shape.sheet, geom_index);
        shape.sheet.sync_all_sections_to_cells();
        true
    };

    if inserted {
        ReactiveEngine::recalculate_document(doc);
    }
    inserted
}

/// Delete vertex `row_ix` in `Geometry{geom_index}` on `(page_id, shape_id)`, preserving valid `MoveTo` start.
pub fn delete_shape_vertex(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    geom_index: u32,
    row_ix: u32,
) -> bool {
    let deleted = {
        let Some(page) = doc.page_mut(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape_mut(shape_id) else {
            return false;
        };
        let Some(geom) = shape
            .sheet
            .geometries
            .iter_mut()
            .find(|g| g.index == geom_index && !g.deleted)
        else {
            return false;
        };

        let active_count = geom.rows.iter().filter(|r| !r.deleted).count();
        if active_count <= 2 {
            return false;
        }
        let Some(pos) = geom.rows.iter().position(|r| r.ix == row_ix && !r.deleted) else {
            return false;
        };

        geom.rows.remove(pos);
        if let Some(first_active) = geom.rows.iter_mut().find(|r| !r.deleted) {
            if !matches!(
                first_active.kind,
                GeometryRowKind::MoveTo | GeometryRowKind::RelMoveTo | GeometryRowKind::Ellipse
            ) {
                first_active.kind = if is_relative_row_kind(first_active.kind) {
                    GeometryRowKind::RelMoveTo
                } else {
                    GeometryRowKind::MoveTo
                };
                first_active.a = None;
                first_active.b = None;
                first_active.c = None;
                first_active.d = None;
                first_active.e = None;
            }
        }
        renumber_geometry_rows(&mut geom.rows);
        purge_geometry_cells(&mut shape.sheet, geom_index);
        shape.sheet.sync_all_sections_to_cells();
        true
    };

    if deleted {
        ReactiveEngine::recalculate_document(doc);
    }
    deleted
}

/// Simplify a freehand point stream via Ramer-Douglas-Peucker (`0.02 in`) and fit a smooth
/// Catmull-Rom -> cubic Bezier (`RelCubBezTo`) `VisioShape` onto `page_id`.
pub fn draw_freeform_curve(
    doc: &mut VisioDocument,
    page_id: u32,
    points_page_in: &[[f64; 2]],
    closed: bool,
) -> Option<u64> {
    if doc.page(page_id).is_none() {
        return None;
    }

    let mut cleaned: Vec<[f64; 2]> = Vec::with_capacity(points_page_in.len());
    for &p in points_page_in {
        if !p[0].is_finite() || !p[1].is_finite() {
            continue;
        }
        if cleaned.last().is_none_or(|&prev| dist2(prev, p) > 1e-6) {
            cleaned.push(p);
        }
    }
    if closed && cleaned.len() >= 2 && dist2(cleaned[0], *cleaned.last().unwrap()) < 1e-5 {
        cleaned.pop();
    }
    let min_pts = if closed { 3 } else { 2 };
    if cleaned.len() < min_pts {
        return None;
    }

    let mut knots = rdp_simplify(&cleaned, RDP_EPSILON_IN);
    if closed && knots.len() < 3 {
        knots = select_extremal_triangle(&cleaned);
    }
    if knots.len() < min_pts {
        return None;
    }

    // Fit Catmull-Rom spline knots -> cubic Bezier segments `(ctrl1, ctrl2, to)`.
    let n = knots.len();
    let seg_count = if closed { n } else { n - 1 };
    let mut bezier_segs: Vec<([f64; 2], [f64; 2], [f64; 2])> = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let p0 = if closed {
            knots[(i + n - 1) % n]
        } else if i == 0 {
            knots[0]
        } else {
            knots[i - 1]
        };
        let p1 = knots[i];
        let p2 = knots[(i + 1) % n];
        let p3 = if closed {
            knots[(i + 2) % n]
        } else if i + 2 < n {
            knots[i + 2]
        } else {
            knots[n - 1]
        };

        let c1 = [
            p1[0] + (p2[0] - p0[0]) / 6.0,
            p1[1] + (p2[1] - p0[1]) / 6.0,
        ];
        let c2 = [
            p2[0] - (p3[0] - p1[0]) / 6.0,
            p2[1] - (p3[1] - p1[1]) / 6.0,
        ];
        bezier_segs.push((c1, c2, p2));
    }

    let mut min_x = knots[0][0];
    let mut min_y = knots[0][1];
    let mut max_x = knots[0][0];
    let mut max_y = knots[0][1];
    let mut expand = |pt: [f64; 2]| {
        min_x = min_x.min(pt[0]);
        min_y = min_y.min(pt[1]);
        max_x = max_x.max(pt[0]);
        max_y = max_y.max(pt[1]);
    };
    for &k in &knots {
        expand(k);
    }
    for &(c1, c2, to) in &bezier_segs {
        expand(c1);
        expand(c2);
        expand(to);
    }

    let w = (max_x - min_x).max(0.05);
    let h = (max_y - min_y).max(0.05);
    let pin_x = min_x + w * 0.5;
    let pin_y = min_y + h * 0.5;

    let norm_x = |x: f64| format_clean_f64((x - min_x) / w);
    let norm_y = |y: f64| format_clean_f64((y - min_y) / h);

    let mut geom = GeometrySection::new(1);
    geom.no_fill = if closed { "FALSE".into() } else { "TRUE".into() };
    geom.rows.push(GeometryRow {
        ix: 1,
        kind: GeometryRowKind::RelMoveTo,
        x: norm_x(knots[0][0]),
        y: norm_y(knots[0][1]),
        a: None,
        b: None,
        c: None,
        d: None,
        e: None,
        deleted: false,
    });
    for (idx, &(c1, c2, to)) in bezier_segs.iter().enumerate() {
        geom.rows.push(GeometryRow {
            ix: (idx + 2) as u32,
            kind: GeometryRowKind::RelCubBezTo,
            x: norm_x(to[0]),
            y: norm_y(to[1]),
            a: Some(norm_x(c1[0])),
            b: Some(norm_y(c1[1])),
            c: Some(norm_x(c2[0])),
            d: Some(norm_y(c2[1])),
            e: None,
            deleted: false,
        });
    }

    let page = doc.page_mut(page_id)?;
    let new_id = page.next_shape_id();
    let name = format!("Freeform.{new_id}");
    let mut sheet = ShapeSheet::new_2d(pin_x, pin_y, w, h);
    if !closed {
        sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    }
    sheet.geometries.clear();
    purge_geometry_cells(&mut sheet, 1);
    sheet.add_geometry(geom);

    let shape = VisioShape {
        id: new_id,
        name: name.clone(),
        name_u: name,
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: String::new(),
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    page.shapes.push(shape);
    ReactiveEngine::recalculate_document(doc);
    Some(new_id)
}

/// Ensure `(page_id, shape_id)` has at least one draggable yellow diamond `Controls.Row_1` handle.
/// If none exists, adds `Controls.Row_1` and wires `Rounding` to track `Controls.Row_1.X` when unguarded.
pub fn ensure_default_control_handle(doc: &mut VisioDocument, page_id: u32, shape_id: u64) -> bool {
    let changed = {
        let Some(page) = doc.page_mut(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape_mut(shape_id) else {
            return false;
        };
        if !shape.sheet.controls.is_empty() {
            return true;
        }
        shape.sheet.add_control(ControlRow {
            name: "Row_1".into(),
            x: "=Width*0.25".into(),
            y: "=Height*1".into(),
            x_dyn: "=Controls.Row_1.X".into(),
            y_dyn: "=Controls.Row_1.Y".into(),
            x_con: "0".into(),
            y_con: "0".into(),
            can_glue: true,
            prompt: "Adjust Corner Rounding / Tip".into(),
        });
        let rounding_unmodified = shape
            .sheet
            .get_cell("Rounding")
            .is_none_or(|c| !c.is_guarded() && shape.sheet.get_f64("Rounding").unwrap_or(0.0) <= 1e-9);
        if rounding_unmodified {
            let _ = shape.sheet.try_set_formula_respecting_guard(
                "Rounding",
                "=BOUND(Controls.Row_1.X, 0, FALSE, 0 in, Width*0.5)",
                false,
            );
        }
        true
    };
    if changed {
        ReactiveEngine::recalculate_document(doc);
    }
    changed
}

/// List all yellow diamond `Controls.*` handles on `(page_id, shape_id)` in world/page inches.
pub fn list_control_handles(
    doc: &VisioDocument,
    page_id: u32,
    shape_id: u64,
) -> Vec<ControlHandleInfo> {
    let Some(page) = doc.page(page_id) else {
        return Vec::new();
    };
    let Some((shape, world_tf)) = find_shape_with_world_transform(page, shape_id) else {
        return Vec::new();
    };

    let mut handles = Vec::with_capacity(shape.sheet.controls.len());
    for ctrl in &shape.sheet.controls {
        let lx = shape
            .sheet
            .get_f64(&format!("Controls.{}.X", ctrl.name))
            .or_else(|| shape.sheet.get_f64(&format!("Controls.{}", ctrl.name)))
            .unwrap_or_else(|| parse_fallback_inches(&ctrl.x));
        let ly = shape
            .sheet
            .get_f64(&format!("Controls.{}.Y", ctrl.name))
            .unwrap_or_else(|| parse_fallback_inches(&ctrl.y));
        let (px, py) = world_tf.local_to_parent(lx, ly);
        handles.push(ControlHandleInfo {
            name: ctrl.name.clone(),
            page_x_in: px,
            page_y_in: py,
            prompt: ctrl.prompt.clone(),
            can_glue: ctrl.can_glue,
        });
    }
    handles
}

/// Drag yellow diamond control handle `row_name` (e.g., `"Row_1"` or `"Controls.Row_1"`) to
/// `(new_page_x_in, new_page_y_in)` in page coordinates, converting to local coordinates and
/// recalculating dependent ShapeSheet formulas.
pub fn move_control_handle(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    row_name: &str,
    new_page_x_in: f64,
    new_page_y_in: f64,
) -> bool {
    if !new_page_x_in.is_finite() || !new_page_y_in.is_finite() {
        return false;
    }
    let clean_name = row_name
        .trim()
        .strip_prefix("Controls.")
        .or_else(|| row_name.trim().strip_prefix("controls."))
        .unwrap_or(row_name.trim());
    if clean_name.is_empty() {
        return false;
    }

    let world_tf = {
        let Some(page) = doc.page(page_id) else {
            return false;
        };
        let Some((_, tf)) = find_shape_with_world_transform(page, shape_id) else {
            return false;
        };
        tf
    };

    let (lx, ly) = world_tf.parent_to_local(new_page_x_in, new_page_y_in);
    let x_formula = format!("{} in", format_clean_f64(lx));
    let y_formula = format!("{} in", format_clean_f64(ly));

    let moved = {
        let Some(page) = doc.page_mut(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape_mut(shape_id) else {
            return false;
        };
        let Some(ctrl_idx) = shape
            .sheet
            .controls
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(clean_name))
        else {
            return false;
        };
        let actual_name = shape.sheet.controls[ctrl_idx].name.clone();

        for guarded_check in [
            format!("Controls.{actual_name}"),
            format!("Controls.{actual_name}.X"),
            format!("Controls.{actual_name}.Y"),
        ] {
            if shape
                .sheet
                .get_cell(&guarded_check)
                .is_some_and(|c| c.is_guarded())
            {
                return false;
            }
        }

        shape.sheet.controls[ctrl_idx].x = x_formula.clone();
        shape.sheet.controls[ctrl_idx].y = y_formula.clone();
        shape.sheet.controls[ctrl_idx].x_dyn = x_formula.clone();
        shape.sheet.controls[ctrl_idx].y_dyn = y_formula.clone();

        shape
            .sheet
            .set_formula(format!("Controls.{actual_name}"), x_formula.clone(), Some(Unit::Inch));
        shape
            .sheet
            .set_formula(format!("Controls.{actual_name}.X"), x_formula.clone(), Some(Unit::Inch));
        shape
            .sheet
            .set_formula(format!("Controls.{actual_name}.Y"), y_formula.clone(), Some(Unit::Inch));
        shape
            .sheet
            .set_formula(format!("Controls.{actual_name}.XDyn"), x_formula, Some(Unit::Inch));
        shape
            .sheet
            .set_formula(format!("Controls.{actual_name}.YDyn"), y_formula, Some(Unit::Inch));
        true
    };

    if moved {
        ReactiveEngine::recalculate_document(doc);
    }
    moved
}

// ============================================================================
// Internal Path Flattening & Shape Builders
// ============================================================================

fn flatten_visual_paths(visual: &EvaluatedShapeVisual) -> ShapeCurveSet {
    let mut closed_rings = Vec::new();
    let mut all_curves = Vec::new();

    let source_paths: Vec<EvaluatedGeometryPath> = if visual.paths.is_empty() && visual.kind != ShapeKind::Guide {
        let tf = &visual.transform;
        let c0: [f64; 2] = tf.local_to_parent(0.0, 0.0).into();
        let c1: [f64; 2] = tf.local_to_parent(tf.width, 0.0).into();
        let c2: [f64; 2] = tf.local_to_parent(tf.width, tf.height).into();
        let c3: [f64; 2] = tf.local_to_parent(0.0, tf.height).into();
        vec![EvaluatedGeometryPath {
            no_fill: visual.kind == ShapeKind::Connector1D,
            no_line: false,
            no_show: false,
            verbs_page_space: vec![
                EvaluatedPathVerb::MoveTo(c0),
                EvaluatedPathVerb::LineTo(c1),
                EvaluatedPathVerb::LineTo(c2),
                EvaluatedPathVerb::LineTo(c3),
                EvaluatedPathVerb::Close,
            ],
        }]
    } else {
        visual.paths.clone()
    };

    for gp in &source_paths {
        if gp.no_show || gp.verbs_page_space.is_empty() {
            continue;
        }
        let mut cur_pts: Vec<[f64; 2]> = Vec::new();

        let flush = |pts: &mut Vec<[f64; 2]>,
                     explicit_close: bool,
                     closed_out: &mut Vec<Vec<[f64; 2]>>,
                     all_out: &mut Vec<FlattenedCurve>| {
            if pts.len() < 2 {
                pts.clear();
                return;
            }
            let first = pts[0];
            let last = *pts.last().unwrap();
            let ends_meet = dist2(first, last) < 1e-4;
            let is_closed = explicit_close || (ends_meet && pts.len() >= 4);
            if is_closed {
                if ends_meet && pts.len() >= 2 {
                    pts.pop();
                }
                let dedup = dedup_ring_points(pts);
                if dedup.len() >= 3 && polygon_signed_area(&dedup).abs() > AREA_EPS {
                    closed_out.push(dedup.clone());
                    all_out.push(FlattenedCurve {
                        points: dedup,
                        closed: true,
                    });
                }
            } else {
                let dedup = dedup_open_points(pts);
                if dedup.len() >= 2 && polyline_length(&dedup) > 1e-5 {
                    all_out.push(FlattenedCurve {
                        points: dedup,
                        closed: false,
                    });
                }
            }
            pts.clear();
        };

        for verb in &gp.verbs_page_space {
            match *verb {
                EvaluatedPathVerb::MoveTo(p) => {
                    if !is_finite_pt(p) {
                        continue;
                    }
                    flush(&mut cur_pts, false, &mut closed_rings, &mut all_curves);
                    cur_pts.push(p);
                }
                EvaluatedPathVerb::LineTo(p) => {
                    if !is_finite_pt(p) {
                        continue;
                    }
                    if cur_pts.is_empty() || dist2(*cur_pts.last().unwrap(), p) > 1e-7 {
                        cur_pts.push(p);
                    }
                }
                EvaluatedPathVerb::QuadTo { ctrl, to } => {
                    if !is_finite_pt(ctrl) || !is_finite_pt(to) {
                        continue;
                    }
                    if let Some(&from) = cur_pts.last() {
                        flatten_quad_bezier(from, ctrl, to, 0.003, 0, &mut cur_pts);
                    } else {
                        cur_pts.push(to);
                    }
                }
                EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                    if !is_finite_pt(ctrl1) || !is_finite_pt(ctrl2) || !is_finite_pt(to) {
                        continue;
                    }
                    if let Some(&from) = cur_pts.last() {
                        flatten_cubic_bezier(from, ctrl1, ctrl2, to, 0.003, 0, &mut cur_pts);
                    } else {
                        cur_pts.push(to);
                    }
                }
                EvaluatedPathVerb::ArcTo { to, bow } => {
                    if !is_finite_pt(to) || !bow.is_finite() {
                        continue;
                    }
                    if let Some(&from) = cur_pts.last() {
                        flatten_sagitta_arc(from, to, bow, &mut cur_pts);
                    } else {
                        cur_pts.push(to);
                    }
                }
                EvaluatedPathVerb::Ellipse {
                    center,
                    rx,
                    ry,
                    angle_rad,
                } => {
                    flush(&mut cur_pts, false, &mut closed_rings, &mut all_curves);
                    if is_finite_pt(center) && rx.abs() > 1e-5 && ry.abs() > 1e-5 {
                        let ring = sample_rotated_ellipse(center, rx.abs(), ry.abs(), angle_rad, 48);
                        closed_rings.push(ring.clone());
                        all_curves.push(FlattenedCurve {
                            points: ring,
                            closed: true,
                        });
                    }
                }
                EvaluatedPathVerb::Close => {
                    flush(&mut cur_pts, true, &mut closed_rings, &mut all_curves);
                }
            }
        }
        flush(&mut cur_pts, false, &mut closed_rings, &mut all_curves);
    }

    ShapeCurveSet {
        closed_rings,
        all_curves,
    }
}

fn flatten_quad_bezier(
    p0: [f64; 2],
    p1: [f64; 2],
    p2: [f64; 2],
    tol: f64,
    depth: u8,
    out: &mut Vec<[f64; 2]>,
) {
    let mx = 0.25 * p0[0] + 0.5 * p1[0] + 0.25 * p2[0];
    let my = 0.25 * p0[1] + 0.5 * p1[1] + 0.25 * p2[1];
    let chord_mx = 0.5 * (p0[0] + p2[0]);
    let chord_my = 0.5 * (p0[1] + p2[1]);
    let err = ((mx - chord_mx).powi(2) + (my - chord_my).powi(2)).sqrt();
    if err <= tol || depth >= 8 {
        out.push(p2);
        return;
    }
    let p01 = [0.5 * (p0[0] + p1[0]), 0.5 * (p0[1] + p1[1])];
    let p12 = [0.5 * (p1[0] + p2[0]), 0.5 * (p1[1] + p2[1])];
    let mid = [0.5 * (p01[0] + p12[0]), 0.5 * (p01[1] + p12[1])];
    flatten_quad_bezier(p0, p01, mid, tol, depth + 1, out);
    flatten_quad_bezier(mid, p12, p2, tol, depth + 1, out);
}

fn flatten_cubic_bezier(
    p0: [f64; 2],
    p1: [f64; 2],
    p2: [f64; 2],
    p3: [f64; 2],
    tol: f64,
    depth: u8,
    out: &mut Vec<[f64; 2]>,
) {
    let d1 = point_to_segment_dist(p1, p0, p3);
    let d2 = point_to_segment_dist(p2, p0, p3);
    if d1.max(d2) <= tol || depth >= 8 {
        out.push(p3);
        return;
    }
    let p01 = midpoint(p0, p1);
    let p12 = midpoint(p1, p2);
    let p23 = midpoint(p2, p3);
    let p012 = midpoint(p01, p12);
    let p123 = midpoint(p12, p23);
    let mid = midpoint(p012, p123);
    flatten_cubic_bezier(p0, p01, p012, mid, tol, depth + 1, out);
    flatten_cubic_bezier(mid, p123, p23, p3, tol, depth + 1, out);
}

fn flatten_sagitta_arc(p0: [f64; 2], p1: [f64; 2], bow: f64, out: &mut Vec<[f64; 2]>) {
    let chord = dist2(p0, p1);
    if chord < 1e-7 {
        return;
    }
    if bow.abs() < 1e-6 {
        out.push(p1);
        return;
    }
    let ux = (p1[0] - p0[0]) / chord;
    let uy = (p1[1] - p0[1]) / chord;
    let nx = -uy;
    let ny = ux;
    let mx = 0.5 * (p0[0] + p1[0]);
    let my = 0.5 * (p0[1] + p1[1]);
    let arc_mx = mx + bow * nx;
    let arc_my = my + bow * ny;

    let half_c = 0.5 * chord;
    let radius = (half_c * half_c + bow * bow) / (2.0 * bow.abs());
    let center_offset = bow - radius * bow.signum();
    let cx = mx + center_offset * nx;
    let cy = my + center_offset * ny;

    let a0 = (p0[1] - cy).atan2(p0[0] - cx);
    let amid = (arc_my - cy).atan2(arc_mx - cx);
    let a1 = (p1[1] - cy).atan2(p1[0] - cx);

    let ccw_end = (a1 - a0).rem_euclid(TAU);
    let ccw_mid = (amid - a0).rem_euclid(TAU);
    let sweep = if ccw_mid <= ccw_end {
        ccw_end
    } else {
        ccw_end - TAU
    };

    let steps = ((sweep.abs() / (PI / 16.0)).ceil() as usize).clamp(2, 64);
    for i in 1..steps {
        let ang = a0 + sweep * (i as f64 / steps as f64);
        let (s, c) = ang.sin_cos();
        out.push([cx + radius * c, cy + radius * s]);
    }
    out.push(p1);
}

fn sample_rotated_ellipse(
    center: [f64; 2],
    rx: f64,
    ry: f64,
    angle_rad: f64,
    steps: usize,
) -> Vec<[f64; 2]> {
    let (sa, ca) = angle_rad.sin_cos();
    let mut ring = Vec::with_capacity(steps);
    for i in 0..steps {
        let theta = TAU * (i as f64 / steps as f64);
        let (st, ct) = theta.sin_cos();
        let lx = rx * ct;
        let ly = ry * st;
        ring.push([
            center[0] + lx * ca - ly * sa,
            center[1] + lx * sa + ly * ca,
        ]);
    }
    ring
}

fn build_shape_from_world_sections(
    id: u64,
    name: &str,
    sections: &[(Vec<[f64; 2]>, bool)],
) -> Option<VisioShape> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (pts, _) in sections {
        for &p in pts {
            if is_finite_pt(p) {
                min_x = min_x.min(p[0]);
                min_y = min_y.min(p[1]);
                max_x = max_x.max(p[0]);
                max_y = max_y.max(p[1]);
            }
        }
    }
    if !min_x.is_finite() || !min_y.is_finite() {
        return None;
    }

    let w = (max_x - min_x).max(0.001);
    let h = (max_y - min_y).max(0.001);
    let pin_x = min_x + w * 0.5;
    let pin_y = min_y + h * 0.5;

    let mut sheet = ShapeSheet::new_2d(pin_x, pin_y, w, h);
    sheet.geometries.clear();
    purge_geometry_cells(&mut sheet, 1);

    for (sec_idx, (pts, closed)) in sections.iter().enumerate() {
        if pts.len() < 2 {
            continue;
        }
        let geom_index = (sec_idx + 1) as u32;
        let mut geom = GeometrySection::new(geom_index);
        geom.no_fill = if *closed { "FALSE".into() } else { "TRUE".into() };

        let mut rows = Vec::with_capacity(pts.len() + usize::from(*closed));
        for (i, &p) in pts.iter().enumerate() {
            let lx = format!("{} in", format_clean_f64(p[0] - min_x));
            let ly = format!("{} in", format_clean_f64(p[1] - min_y));
            if i == 0 {
                rows.push(GeometryRow::move_to(1, lx, ly));
            } else {
                rows.push(GeometryRow::line_to((i + 1) as u32, lx, ly));
            }
        }
        if *closed && dist2(pts[0], *pts.last().unwrap()) > 1e-6 {
            let lx0 = format!("{} in", format_clean_f64(pts[0][0] - min_x));
            let ly0 = format!("{} in", format_clean_f64(pts[0][1] - min_y));
            rows.push(GeometryRow::line_to((rows.len() + 1) as u32, lx0, ly0));
        }
        geom.rows = rows;
        sheet.add_geometry(geom);
    }

    if sheet.geometries.is_empty() {
        return None;
    }

    Some(VisioShape {
        id,
        name: name.to_string(),
        name_u: name.to_string(),
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: String::new(),
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    })
}

// ============================================================================
// 2D Planar Arrangement & Constructive Polygon Boolean Engine
// ============================================================================

#[derive(Clone, Debug)]
struct ElementaryFace {
    half_edges: Vec<usize>,
    mask: Vec<bool>,
}

#[derive(Clone, Copy, Debug)]
struct HalfEdge {
    from: usize,
    to: usize,
    twin: usize,
}

fn compute_planar_boolean_regions(
    shapes: &[ShapeCurveSet],
    op: BooleanOpKind,
) -> Vec<Vec<(Vec<[f64; 2]>, bool)>> {
    let mut raw_segments: Vec<([f64; 2], [f64; 2])> = Vec::new();
    for sc in shapes {
        for ring in &sc.closed_rings {
            for i in 0..ring.len() {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                if dist2(a, b) > SNAP_EPS {
                    raw_segments.push((a, b));
                }
            }
        }
    }
    if raw_segments.is_empty() {
        return Vec::new();
    }

    let (verts, undirected_edges) = build_snapped_planar_graph(&raw_segments);
    if undirected_edges.is_empty() {
        return Vec::new();
    }

    // Build half-edges `2 * e` (`u -> v`) and `2 * e + 1` (`v -> u`).
    let mut half_edges: Vec<HalfEdge> = Vec::with_capacity(undirected_edges.len() * 2);
    let mut outgoing: Vec<Vec<(f64, usize)>> = vec![Vec::new(); verts.len()];
    for &(u, v) in &undirected_edges {
        let fwd = half_edges.len();
        let rev = fwd + 1;
        half_edges.push(HalfEdge {
            from: u,
            to: v,
            twin: rev,
        });
        half_edges.push(HalfEdge {
            from: v,
            to: u,
            twin: fwd,
        });
        let ang_uv = (verts[v][1] - verts[u][1]).atan2(verts[v][0] - verts[u][0]);
        let ang_vu = (verts[u][1] - verts[v][1]).atan2(verts[u][0] - verts[v][0]);
        outgoing[u].push((ang_uv, fwd));
        outgoing[v].push((ang_vu, rev));
    }

    for adj in &mut outgoing {
        adj.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    }

    // Precompute `next_he` so tracing a face keeps its interior strictly to the LEFT.
    // Arriving at `v` via `he` (`u -> v`), locate `twin` (`v -> u`) in `outgoing[v]` (sorted CCW)
    // and step one slot CLOCKWISE (`prev_idx`).
    let mut next_he = vec![0usize; half_edges.len()];
    for (he_idx, he) in half_edges.iter().enumerate() {
        let adj = &outgoing[he.to];
        let pos = adj
            .iter()
            .position(|&(_, cand)| cand == he.twin)
            .unwrap_or(0);
        let prev_pos = if pos == 0 { adj.len() - 1 } else { pos - 1 };
        next_he[he_idx] = adj[prev_pos].1;
    }

    let mut visited_he = vec![false; half_edges.len()];
    let mut pos_cycles: Vec<(Vec<[f64; 2]>, Vec<usize>, f64)> = Vec::new();
    let mut neg_cycles: Vec<(Vec<[f64; 2]>, Vec<usize>)> = Vec::new();

    for start_he in 0..half_edges.len() {
        if visited_he[start_he] {
            continue;
        }
        let mut cycle_hes = Vec::new();
        let mut cur = start_he;
        let mut closed_cleanly = false;
        while !visited_he[cur] {
            visited_he[cur] = true;
            cycle_hes.push(cur);
            cur = next_he[cur];
            if cur == start_he {
                closed_cleanly = true;
                break;
            }
        }
        if !closed_cleanly || cycle_hes.len() < 3 {
            continue;
        }
        // Decompose pinched self-touching vertex loops into elementary simple sub-cycles.
        for sub_hes in split_into_simple_vertex_cycles(&cycle_hes, &half_edges) {
            if sub_hes.len() < 3 {
                continue;
            }
            let poly: Vec<[f64; 2]> = sub_hes
                .iter()
                .map(|&h| verts[half_edges[h].from])
                .collect();
            let area = polygon_signed_area(&poly);
            if area > AREA_EPS {
                pos_cycles.push((poly, sub_hes, area));
            } else if area < -AREA_EPS {
                neg_cycles.push((poly, sub_hes));
            }
        }
    }

    if pos_cycles.is_empty() {
        return Vec::new();
    }

    // Sort positive cycles by area ascending so smallest enclosing face wins hole assignment.
    pos_cycles.sort_by(|a, b| a.2.total_cmp(&b.2));

    let mut face_holes: Vec<Vec<Vec<[f64; 2]>>> = vec![Vec::new(); pos_cycles.len()];
    let mut face_all_hes: Vec<Vec<usize>> = pos_cycles.iter().map(|c| c.1.clone()).collect();

    // Assign negative-area cycles (exterior boundaries of disconnected inner islands) to their
    // smallest strictly containing positive cycle.
    for (neg_poly, neg_hes) in &neg_cycles {
        let probe = neg_poly[0];
        for (f_idx, (pos_poly, _, _)) in pos_cycles.iter().enumerate() {
            if point_in_ring_strict(probe, pos_poly) {
                face_holes[f_idx].push(neg_poly.clone());
                face_all_hes[f_idx].extend_from_slice(neg_hes);
                break;
            }
        }
    }

    let mut faces: Vec<ElementaryFace> = Vec::with_capacity(pos_cycles.len());
    for (f_idx, (outer_poly, _, _)) in pos_cycles.iter().enumerate() {
        let sample = interior_sample_point(outer_poly, &face_holes[f_idx]);
        let mask: Vec<bool> = shapes
            .iter()
            .map(|sc| point_in_shape_evenodd(sample, &sc.closed_rings))
            .collect();
        faces.push(ElementaryFace {
            half_edges: face_all_hes[f_idx].clone(),
            mask,
        });
    }

    match op {
        BooleanOpKind::Union => {
            let kept: Vec<usize> = (0..faces.len())
                .filter(|&i| faces[i].mask.iter().any(|&b| b))
                .collect();
            let rings = extract_merged_boundary_rings(&kept, &faces, &half_edges, &outgoing, &verts);
            if rings.is_empty() {
                Vec::new()
            } else {
                vec![rings]
            }
        }
        BooleanOpKind::Intersect => {
            let kept: Vec<usize> = (0..faces.len())
                .filter(|&i| !faces[i].mask.is_empty() && faces[i].mask.iter().all(|&b| b))
                .collect();
            let rings = extract_merged_boundary_rings(&kept, &faces, &half_edges, &outgoing, &verts);
            if rings.is_empty() {
                Vec::new()
            } else {
                vec![rings]
            }
        }
        BooleanOpKind::Subtract => {
            let kept: Vec<usize> = (0..faces.len())
                .filter(|&i| {
                    let m = &faces[i].mask;
                    m.first().copied().unwrap_or(false) && !m.iter().skip(1).any(|&b| b)
                })
                .collect();
            let rings = extract_merged_boundary_rings(&kept, &faces, &half_edges, &outgoing, &verts);
            if rings.is_empty() {
                Vec::new()
            } else {
                vec![rings]
            }
        }
        BooleanOpKind::Combine => {
            // Symmetric XOR compound GeometrySections (`Geometry1`, `Geometry2`, ...) on a single shape.
            let kept: Vec<usize> = (0..faces.len())
                .filter(|&i| faces[i].mask.iter().filter(|&&b| b).count() % 2 == 1)
                .collect();
            let rings = extract_merged_boundary_rings(&kept, &faces, &half_edges, &outgoing, &verts);
            if rings.is_empty() {
                Vec::new()
            } else {
                vec![rings]
            }
        }
        BooleanOpKind::Fragment => {
            // Group connected faces sharing identical non-empty membership masks into separate shapes.
            let active_faces: Vec<usize> = (0..faces.len())
                .filter(|&i| faces[i].mask.iter().any(|&b| b))
                .collect();
            if active_faces.is_empty() {
                return Vec::new();
            }
            let mut he_owner: BTreeMap<usize, usize> = BTreeMap::new();
            for &fi in &active_faces {
                for &h in &faces[fi].half_edges {
                    he_owner.insert(h, fi);
                }
            }
            let mut visited_face = BTreeSet::new();
            let mut fragment_shapes = Vec::new();
            for &start_fi in &active_faces {
                if !visited_face.insert(start_fi) {
                    continue;
                }
                let mut component = vec![start_fi];
                let mut queue = vec![start_fi];
                while let Some(cur_fi) = queue.pop() {
                    for &h in &faces[cur_fi].half_edges {
                        let twin = half_edges[h].twin;
                        if let Some(&nbr_fi) = he_owner.get(&twin) {
                            if faces[nbr_fi].mask == faces[cur_fi].mask
                                && visited_face.insert(nbr_fi)
                            {
                                component.push(nbr_fi);
                                queue.push(nbr_fi);
                            }
                        }
                    }
                }
                let rings =
                    extract_merged_boundary_rings(&component, &faces, &half_edges, &outgoing, &verts);
                if !rings.is_empty() {
                    fragment_shapes.push(rings);
                }
            }
            fragment_shapes
        }
        _ => Vec::new(),
    }
}

fn split_into_simple_vertex_cycles(
    cycle_hes: &[usize],
    half_edges: &[HalfEdge],
) -> Vec<Vec<usize>> {
    let mut sub_cycles = Vec::new();
    let mut stack_hes: Vec<usize> = Vec::with_capacity(cycle_hes.len());
    let mut vert_pos: BTreeMap<usize, usize> = BTreeMap::new();

    for &h in cycle_hes {
        let u = half_edges[h].from;
        if let Some(&idx) = vert_pos.get(&u) {
            let extracted: Vec<usize> = stack_hes.drain(idx..).collect();
            for &eh in &extracted {
                vert_pos.remove(&half_edges[eh].from);
            }
            if extracted.len() >= 3 {
                sub_cycles.push(extracted);
            }
        }
        vert_pos.insert(u, stack_hes.len());
        stack_hes.push(h);
    }
    if stack_hes.len() >= 3 {
        sub_cycles.push(stack_hes);
    }
    sub_cycles
}

fn extract_merged_boundary_rings(
    kept_faces: &[usize],
    faces: &[ElementaryFace],
    half_edges: &[HalfEdge],
    outgoing: &[Vec<(f64, usize)>],
    verts: &[[f64; 2]],
) -> Vec<(Vec<[f64; 2]>, bool)> {
    let mut active_hes = BTreeSet::new();
    for &fi in kept_faces {
        for &h in &faces[fi].half_edges {
            active_hes.insert(h);
        }
    }
    // Cancel interior shared half-edge pairs `(h, twin(h))`.
    let surviving: BTreeSet<usize> = active_hes
        .iter()
        .copied()
        .filter(|&h| !active_hes.contains(&half_edges[h].twin))
        .collect();

    let mut used = BTreeSet::new();
    let mut rings = Vec::new();

    for &start_h in &surviving {
        if used.contains(&start_h) {
            continue;
        }
        let mut loop_pts = Vec::new();
        let mut cur = start_h;
        let mut closed = false;
        while used.insert(cur) {
            let he = half_edges[cur];
            loop_pts.push(verts[he.from]);
            let adj = &outgoing[he.to];
            let twin_pos = adj
                .iter()
                .position(|&(_, cand)| cand == he.twin)
                .unwrap_or(0);
            // Step clockwise from `twin` until hitting the next surviving boundary half-edge.
            let mut chosen = None;
            for step in 1..=adj.len() {
                let idx = (twin_pos + adj.len() - (step % adj.len())) % adj.len();
                let cand = adj[idx].1;
                if surviving.contains(&cand) && (!used.contains(&cand) || cand == start_h) {
                    chosen = Some(cand);
                    break;
                }
            }
            match chosen {
                Some(nxt) if nxt == start_h => {
                    closed = true;
                    break;
                }
                Some(nxt) => cur = nxt,
                None => break,
            }
        }
        if closed && loop_pts.len() >= 3 {
            let simplified = simplify_collinear_ring(&loop_pts);
            if simplified.len() >= 3 && polygon_signed_area(&simplified).abs() > AREA_EPS {
                rings.push((simplified, true));
            }
        }
    }
    rings
}

fn build_snapped_planar_graph(
    raw_segments: &[([f64; 2], [f64; 2])],
) -> (Vec<[f64; 2]>, Vec<(usize, usize)>) {
    let mut split_params: Vec<Vec<f64>> = vec![vec![0.0, 1.0]; raw_segments.len()];

    for i in 0..raw_segments.len() {
        for j in (i + 1)..raw_segments.len() {
            for (t, u) in segment_intersections(
                raw_segments[i].0,
                raw_segments[i].1,
                raw_segments[j].0,
                raw_segments[j].1,
            ) {
                split_params[i].push(t);
                split_params[j].push(u);
            }
        }
    }

    let mut verts: Vec<[f64; 2]> = Vec::new();
    let mut grid: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();

    let mut intern_vertex = |pt: [f64; 2]| -> usize {
        let cell_size = SNAP_EPS * 2.0;
        let gx = (pt[0] / cell_size).floor() as i64;
        let gy = (pt[1] / cell_size).floor() as i64;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(bucket) = grid.get(&(gx + dx, gy + dy)) {
                    for &vid in bucket {
                        if dist2(verts[vid], pt) <= SNAP_EPS {
                            return vid;
                        }
                    }
                }
            }
        }
        let vid = verts.len();
        verts.push(pt);
        grid.entry((gx, gy)).or_default().push(vid);
        vid
    };

    let mut raw_edges = BTreeSet::new();
    for (seg_idx, &(a, b)) in raw_segments.iter().enumerate() {
        let params = &mut split_params[seg_idx];
        params.sort_by(|x, y| x.total_cmp(y));
        let mut prev_vid = intern_vertex(a);
        for &t in params.iter().skip(1) {
            let p = [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
            let vid = intern_vertex(p);
            if vid != prev_vid {
                raw_edges.insert((prev_vid.min(vid), prev_vid.max(vid)));
                prev_vid = vid;
            }
        }
    }

    // Resolve any T-junctions where a snapped vertex lies strictly inside an elementary edge.
    let mut clean_edges = BTreeSet::new();
    for &(u, v) in &raw_edges {
        let pu = verts[u];
        let pv = verts[v];
        let dx = pv[0] - pu[0];
        let dy = pv[1] - pu[1];
        let len_sq = dx * dx + dy * dy;
        if len_sq <= SNAP_EPS * SNAP_EPS {
            continue;
        }
        let mut mids: Vec<(f64, usize)> = Vec::new();
        let min_x = pu[0].min(pv[0]) - SNAP_EPS;
        let max_x = pu[0].max(pv[0]) + SNAP_EPS;
        let min_y = pu[1].min(pv[1]) - SNAP_EPS;
        let max_y = pu[1].max(pv[1]) + SNAP_EPS;
        for (w, &pw) in verts.iter().enumerate() {
            if w == u || w == v {
                continue;
            }
            if pw[0] < min_x || pw[0] > max_x || pw[1] < min_y || pw[1] > max_y {
                continue;
            }
            let t = ((pw[0] - pu[0]) * dx + (pw[1] - pu[1]) * dy) / len_sq;
            if t > 1e-6 && t < 1.0 - 1e-6 {
                let proj = [pu[0] + t * dx, pu[1] + t * dy];
                if dist2(pw, proj) <= SNAP_EPS {
                    mids.push((t, w));
                }
            }
        }
        if mids.is_empty() {
            clean_edges.insert((u, v));
        } else {
            mids.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut cur_u = u;
            for (_, w) in mids {
                if w != cur_u {
                    clean_edges.insert((cur_u.min(w), cur_u.max(w)));
                    cur_u = w;
                }
            }
            if cur_u != v {
                clean_edges.insert((cur_u.min(v), cur_u.max(v)));
            }
        }
    }

    (verts, clean_edges.into_iter().collect())
}

fn segment_intersections(
    a0: [f64; 2],
    a1: [f64; 2],
    b0: [f64; 2],
    b1: [f64; 2],
) -> Vec<(f64, f64)> {
    let d1x = a1[0] - a0[0];
    let d1y = a1[1] - a0[1];
    let d2x = b1[0] - b0[0];
    let d2y = b1[1] - b0[1];
    let len1_sq = d1x * d1x + d1y * d1y;
    let len2_sq = d2x * d2x + d2y * d2y;
    if len1_sq < 1e-14 || len2_sq < 1e-14 {
        return Vec::new();
    }

    let cross = d1x * d2y - d1y * d2x;
    let scale = (len1_sq * len2_sq).sqrt();
    let rx = b0[0] - a0[0];
    let ry = b0[1] - a0[1];

    if cross.abs() > 1e-10 * scale {
        let t = (rx * d2y - ry * d2x) / cross;
        let u = (rx * d1y - ry * d1x) / cross;
        let eps = 1e-7;
        if (-eps..=1.0 + eps).contains(&t) && (-eps..=1.0 + eps).contains(&u) {
            return vec![(t.clamp(0.0, 1.0), u.clamp(0.0, 1.0))];
        }
        return Vec::new();
    }

    // Collinear overlap check.
    let perp = (rx * d1y - ry * d1x).abs() / len1_sq.sqrt();
    if perp > SNAP_EPS {
        return Vec::new();
    }
    let mut hits = Vec::new();
    let tb0 = (rx * d1x + ry * d1y) / len1_sq;
    let tb1 = ((b1[0] - a0[0]) * d1x + (b1[1] - a0[1]) * d1y) / len1_sq;
    let ua0 = (-rx * d2x - ry * d2y) / len2_sq;
    let ua1 = ((a1[0] - b0[0]) * d2x + (a1[1] - b0[1]) * d2y) / len2_sq;

    if (-1e-7..=1.0 + 1e-7).contains(&tb0) {
        hits.push((tb0.clamp(0.0, 1.0), 0.0));
    }
    if (-1e-7..=1.0 + 1e-7).contains(&tb1) {
        hits.push((tb1.clamp(0.0, 1.0), 1.0));
    }
    if (-1e-7..=1.0 + 1e-7).contains(&ua0) {
        hits.push((0.0, ua0.clamp(0.0, 1.0)));
    }
    if (-1e-7..=1.0 + 1e-7).contains(&ua1) {
        hits.push((1.0, ua1.clamp(0.0, 1.0)));
    }
    hits
}

// ============================================================================
// Trim, Join & Miter-Clamped Parallel Offset
// ============================================================================

fn compute_trimmed_subcurves(shapes: &[ShapeCurveSet]) -> Vec<Vec<(Vec<[f64; 2]>, bool)>> {
    #[derive(Clone)]
    struct IndexedCurve {
        pts: Vec<[f64; 2]>,
        closed: bool,
        seg_count: usize,
    }

    let mut curves = Vec::new();
    for sc in shapes {
        for c in &sc.all_curves {
            let seg_count = if c.closed {
                c.points.len()
            } else {
                c.points.len().saturating_sub(1)
            };
            if seg_count >= 1 {
                curves.push(IndexedCurve {
                    pts: c.points.clone(),
                    closed: c.closed,
                    seg_count,
                });
            }
        }
    }
    if curves.is_empty() {
        return Vec::new();
    }

    // Parameterize each curve `ci` by cumulative segment index `s = seg_idx + t` in `[0, seg_count]`.
    let mut curve_cuts: Vec<Vec<f64>> = vec![Vec::new(); curves.len()];

    for ci in 0..curves.len() {
        for si in 0..curves[ci].seg_count {
            let a0 = curves[ci].pts[si];
            let a1 = curves[ci].pts[(si + 1) % curves[ci].pts.len()];
            for cj in (ci + 1)..curves.len() {
                for sj in 0..curves[cj].seg_count {
                    let b0 = curves[cj].pts[sj];
                    let b1 = curves[cj].pts[(sj + 1) % curves[cj].pts.len()];
                    for (t, u) in segment_intersections(a0, a1, b0, b1) {
                        curve_cuts[ci].push(si as f64 + t);
                        curve_cuts[cj].push(sj as f64 + u);
                    }
                }
            }
        }
    }

    let mut trimmed_shapes = Vec::new();
    for (ci, curve) in curves.iter().enumerate() {
        let total_s = curve.seg_count as f64;
        let mut cuts = curve_cuts[ci].clone();
        if !curve.closed {
            cuts.push(0.0);
            cuts.push(total_s);
        }
        cuts.sort_by(|a, b| a.total_cmp(b));

        // Deduplicate cuts within parameter tolerance.
        let mut unique_cuts: Vec<f64> = Vec::new();
        for mut c in cuts {
            if curve.closed && (c - total_s).abs() < 1e-5 {
                c = 0.0;
            }
            if unique_cuts.iter().all(|&u| (u - c).abs() > 1e-5) {
                unique_cuts.push(c);
            }
        }
        unique_cuts.sort_by(|a, b| a.total_cmp(b));

        if curve.closed {
            if unique_cuts.len() < 2 {
                // Uncut or single-tangent closed curve: emit as open loop from cut/origin.
                let mut open_pts = curve.pts.clone();
                open_pts.push(curve.pts[0]);
                trimmed_shapes.push(vec![(open_pts, false)]);
            } else {
                for k in 0..unique_cuts.len() {
                    let s0 = unique_cuts[k];
                    let s1 = if k + 1 < unique_cuts.len() {
                        unique_cuts[k + 1]
                    } else {
                        unique_cuts[0] + total_s
                    };
                    let sub = slice_parameterized_curve(&curve.pts, true, s0, s1);
                    if sub.len() >= 2 && polyline_length(&sub) > 1e-4 {
                        trimmed_shapes.push(vec![(sub, false)]);
                    }
                }
            }
        } else {
            for pair in unique_cuts.windows(2) {
                let sub = slice_parameterized_curve(&curve.pts, false, pair[0], pair[1]);
                if sub.len() >= 2 && polyline_length(&sub) > 1e-4 {
                    trimmed_shapes.push(vec![(sub, false)]);
                }
            }
        }
    }
    trimmed_shapes
}

fn slice_parameterized_curve(pts: &[[f64; 2]], closed: bool, s0: f64, s1: f64) -> Vec<[f64; 2]> {
    if s1 <= s0 + 1e-6 {
        return Vec::new();
    }
    let eval_at = |s: f64| -> [f64; 2] {
        let n = pts.len();
        let max_seg = if closed { n } else { n - 1 };
        let wrapped = if closed {
            s.rem_euclid(max_seg as f64)
        } else {
            s.clamp(0.0, max_seg as f64)
        };
        let mut idx = wrapped.floor() as usize;
        let mut frac = wrapped - idx as f64;
        if idx >= max_seg {
            idx = max_seg - 1;
            frac = 1.0;
        }
        let a = pts[idx % n];
        let b = pts[(idx + 1) % n];
        [a[0] + (b[0] - a[0]) * frac, a[1] + (b[1] - a[1]) * frac]
    };

    let mut out = vec![eval_at(s0)];
    let first_knot = s0.floor() as usize + 1;
    let last_knot = s1.ceil() as usize;
    for k in first_knot..last_knot {
        let kf = k as f64;
        if kf > s0 + 1e-6 && kf < s1 - 1e-6 {
            let p = eval_at(kf);
            if dist2(*out.last().unwrap(), p) > 1e-6 {
                out.push(p);
            }
        }
    }
    let end_p = eval_at(s1);
    if dist2(*out.last().unwrap(), end_p) > 1e-6 {
        out.push(end_p);
    }
    out
}

fn compute_joined_curves(shapes: &[ShapeCurveSet]) -> Vec<Vec<(Vec<[f64; 2]>, bool)>> {
    let mut open_chains: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut closed_sections: Vec<(Vec<[f64; 2]>, bool)> = Vec::new();

    for sc in shapes {
        for c in &sc.all_curves {
            if c.closed {
                closed_sections.push((c.points.clone(), true));
            } else if c.points.len() >= 2 {
                open_chains.push(c.points.clone());
            }
        }
    }

    loop {
        let mut best_merge: Option<(usize, usize, u8, f64)> = None;
        for i in 0..open_chains.len() {
            let a_start = open_chains[i][0];
            let a_end = *open_chains[i].last().unwrap();
            for j in (i + 1)..open_chains.len() {
                let b_start = open_chains[j][0];
                let b_end = *open_chains[j].last().unwrap();
                let candidates = [
                    (0u8, dist2(a_end, b_start)),
                    (1u8, dist2(a_end, b_end)),
                    (2u8, dist2(a_start, b_start)),
                    (3u8, dist2(a_start, b_end)),
                ];
                for (mode, d) in candidates {
                    if d <= JOIN_TOLERANCE_IN
                        && best_merge.is_none_or(|(_, _, _, best_d)| d < best_d)
                    {
                        best_merge = Some((i, j, mode, d));
                    }
                }
            }
        }

        let Some((i, j, mode, _)) = best_merge else {
            break;
        };
        let mut chain_b = open_chains.remove(j);
        let mut chain_a = open_chains.remove(i);
        match mode {
            0 => {}
            1 => chain_b.reverse(),
            2 => chain_a.reverse(),
            3 => {
                chain_a.reverse();
                chain_b.reverse();
            }
            _ => {}
        }
        let weld = midpoint(*chain_a.last().unwrap(), chain_b[0]);
        *chain_a.last_mut().unwrap() = weld;
        for &pt in chain_b.iter().skip(1) {
            if dist2(*chain_a.last().unwrap(), pt) > 1e-6 {
                chain_a.push(pt);
            }
        }
        open_chains.push(chain_a);
    }

    let mut sections = closed_sections;
    for mut chain in open_chains {
        if chain.len() >= 4 && dist2(chain[0], *chain.last().unwrap()) <= JOIN_TOLERANCE_IN {
            let weld = midpoint(chain[0], *chain.last().unwrap());
            chain[0] = weld;
            chain.pop();
            sections.push((chain, true));
        } else if chain.len() >= 2 {
            sections.push((chain, false));
        }
    }

    if sections.is_empty() {
        Vec::new()
    } else {
        vec![sections]
    }
}

fn compute_offset_curves(
    shapes: &[ShapeCurveSet],
    dist_in: f64,
) -> Vec<Vec<(Vec<[f64; 2]>, bool)>> {
    if !dist_in.is_finite() {
        return Vec::new();
    }
    let mut out_shapes = Vec::new();
    for sc in shapes {
        let mut sections = Vec::new();
        for c in &sc.all_curves {
            if c.closed {
                if let Some(ring) = offset_closed_polygon(&c.points, dist_in) {
                    sections.push((ring, true));
                }
            } else if let Some(line) = offset_open_polyline(&c.points, dist_in) {
                sections.push((line, false));
            }
        }
        if !sections.is_empty() {
            out_shapes.push(sections);
        }
    }
    out_shapes
}

fn offset_closed_polygon(pts: &[[f64; 2]], dist_in: f64) -> Option<Vec<[f64; 2]>> {
    let mut ring = dedup_ring_points(pts);
    if ring.len() < 3 {
        return None;
    }
    if dist_in.abs() < 1e-9 {
        return Some(ring);
    }
    // Ensure CCW orientation so right-hand normal `(ty, -tx)` points strictly outward.
    if polygon_signed_area(&ring) < 0.0 {
        ring.reverse();
    }
    let n = ring.len();
    let mut normals = Vec::with_capacity(n);
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        normals.push([dy / len, -dx / len]);
    }

    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let n_prev = normals[(i + n - 1) % n];
        let n_curr = normals[i];
        let p = ring[i];
        emit_miter_clamped_joint(p, n_prev, n_curr, dist_in, &mut out);
    }
    let cleaned = dedup_ring_points(&out);
    if cleaned.len() < 3 || polygon_signed_area(&cleaned) <= AREA_EPS {
        return None;
    }
    Some(cleaned)
}

fn offset_open_polyline(pts: &[[f64; 2]], dist_in: f64) -> Option<Vec<[f64; 2]>> {
    let line = dedup_open_points(pts);
    if line.len() < 2 {
        return None;
    }
    if dist_in.abs() < 1e-9 {
        return Some(line);
    }
    let segs = line.len() - 1;
    let mut normals = Vec::with_capacity(segs);
    for i in 0..segs {
        let dx = line[i + 1][0] - line[i][0];
        let dy = line[i + 1][1] - line[i][1];
        let len = (dx * dx + dy * dy).sqrt().max(1e-12);
        normals.push([dy / len, -dx / len]);
    }

    let mut out = Vec::with_capacity(line.len() + 2);
    out.push([
        line[0][0] + dist_in * normals[0][0],
        line[0][1] + dist_in * normals[0][1],
    ]);
    for i in 1..segs {
        emit_miter_clamped_joint(line[i], normals[i - 1], normals[i], dist_in, &mut out);
    }
    let last_n = normals[segs - 1];
    let last_p = line[segs];
    out.push([last_p[0] + dist_in * last_n[0], last_p[1] + dist_in * last_n[1]]);
    let cleaned = dedup_open_points(&out);
    (cleaned.len() >= 2).then_some(cleaned)
}

fn emit_miter_clamped_joint(
    p: [f64; 2],
    n_prev: [f64; 2],
    n_curr: [f64; 2],
    dist_in: f64,
    out: &mut Vec<[f64; 2]>,
) {
    let dot = n_prev[0] * n_curr[0] + n_prev[1] * n_curr[1];
    let denom = 1.0 + dot;
    if denom > 1e-4 {
        let mx = (n_prev[0] + n_curr[0]) / denom;
        let my = (n_prev[1] + n_curr[1]) / denom;
        let miter_len = (mx * mx + my * my).sqrt();
        if miter_len <= MITER_LIMIT {
            out.push([p[0] + dist_in * mx, p[1] + dist_in * my]);
            return;
        }
    }
    // Bevel clamp when miter ratio exceeds `MITER_LIMIT`.
    out.push([p[0] + dist_in * n_prev[0], p[1] + dist_in * n_prev[1]]);
    out.push([p[0] + dist_in * n_curr[0], p[1] + dist_in * n_curr[1]]);
}

// ============================================================================
// Geometric Predicates, RDP Simplification & ShapeSheet Helpers
// ============================================================================

fn rdp_simplify(pts: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let first = pts[0];
    let last = *pts.last().unwrap();
    let mut max_dist = -1.0;
    let mut split_idx = 0usize;
    for (i, &p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = point_to_segment_dist(p, first, last);
        if d > max_dist {
            max_dist = d;
            split_idx = i;
        }
    }
    if max_dist > epsilon {
        let mut left = rdp_simplify(&pts[..=split_idx], epsilon);
        let right = rdp_simplify(&pts[split_idx..], epsilon);
        left.pop();
        left.extend_from_slice(&right);
        left
    } else {
        vec![first, last]
    }
}

fn select_extremal_triangle(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if pts.len() <= 3 {
        return pts.to_vec();
    }
    let i0 = 0usize;
    let i1 = (1..pts.len())
        .max_by(|&a, &b| dist2(pts[i0], pts[a]).total_cmp(&dist2(pts[i0], pts[b])))
        .unwrap_or(1);
    let i2 = (1..pts.len())
        .filter(|&k| k != i1)
        .max_by(|&a, &b| {
            point_to_segment_dist(pts[a], pts[i0], pts[i1])
                .total_cmp(&point_to_segment_dist(pts[b], pts[i0], pts[i1]))
        })
        .unwrap_or((i1 + 1).min(pts.len() - 1));
    let mut idxs = [i0, i1, i2];
    idxs.sort_unstable();
    vec![pts[idxs[0]], pts[idxs[1]], pts[idxs[2]]]
}

fn interior_sample_point(outer: &[[f64; 2]], holes: &[Vec<[f64; 2]>]) -> [f64; 2] {
    let is_valid = |p: [f64; 2]| -> bool {
        point_in_ring_strict(p, outer) && holes.iter().all(|h| !point_in_ring_inclusive(p, h))
    };

    let mut cx = 0.0;
    let mut cy = 0.0;
    for &p in outer {
        cx += p[0];
        cy += p[1];
    }
    let centroid = [cx / outer.len() as f64, cy / outer.len() as f64];
    if is_valid(centroid) {
        return centroid;
    }

    // Horizontal slab scan strictly avoiding all vertex Y-coordinates.
    let mut ys: Vec<f64> = outer.iter().map(|p| p[1]).collect();
    for h in holes {
        ys.extend(h.iter().map(|p| p[1]));
    }
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 1e-8);

    let mut slabs: Vec<(f64, f64)> = ys
        .windows(2)
        .map(|w| (w[1] - w[0], 0.5 * (w[0] + w[1])))
        .collect();
    slabs.sort_by(|a, b| b.0.total_cmp(&a.0));

    for (_, y_mid) in slabs {
        let mut xs = Vec::new();
        collect_scanline_intersections(outer, y_mid, &mut xs);
        for h in holes {
            collect_scanline_intersections(h, y_mid, &mut xs);
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            let cand = [0.5 * (pair[0] + pair[1]), y_mid];
            if is_valid(cand) {
                return cand;
            }
        }
    }
    centroid
}

fn collect_scanline_intersections(ring: &[[f64; 2]], y: f64, xs: &mut Vec<f64>) {
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        if (a[1] > y) != (b[1] > y) {
            let t = (y - a[1]) / (b[1] - a[1]);
            xs.push(a[0] + t * (b[0] - a[0]));
        }
    }
}

fn point_in_shape_evenodd(pt: [f64; 2], rings: &[Vec<[f64; 2]>]) -> bool {
    let mut inside = false;
    for ring in rings {
        if ray_crossings_evenodd(pt, ring) {
            inside = !inside;
        }
    }
    inside
}

fn point_in_ring_strict(pt: [f64; 2], ring: &[[f64; 2]]) -> bool {
    for i in 0..ring.len() {
        if point_to_segment_dist(pt, ring[i], ring[(i + 1) % ring.len()]) <= 1e-6 {
            return false;
        }
    }
    ray_crossings_evenodd(pt, ring)
}

fn point_in_ring_inclusive(pt: [f64; 2], ring: &[[f64; 2]]) -> bool {
    for i in 0..ring.len() {
        if point_to_segment_dist(pt, ring[i], ring[(i + 1) % ring.len()]) <= 1e-6 {
            return true;
        }
    }
    ray_crossings_evenodd(pt, ring)
}

fn ray_crossings_evenodd(pt: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = ring.len();
    for i in 0..n {
        let a = ring[i];
        let b = ring[(i + 1) % n];
        if (a[1] > pt[1]) != (b[1] > pt[1]) {
            let x_int = a[0] + (pt[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1]);
            if pt[0] < x_int {
                inside = !inside;
            }
        }
    }
    inside
}

fn polygon_signed_area(ring: &[[f64; 2]]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[(i + 1) % ring.len()];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    0.5 * sum
}

fn simplify_collinear_ring(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut cur = dedup_ring_points(pts);
    if cur.len() <= 3 {
        return cur;
    }
    let mut changed = true;
    while changed && cur.len() > 3 {
        changed = false;
        let mut next = Vec::with_capacity(cur.len());
        let n = cur.len();
        for i in 0..n {
            let prev = cur[(i + n - 1) % n];
            let p = cur[i];
            let nxt = cur[(i + 1) % n];
            let v1 = [p[0] - prev[0], p[1] - prev[1]];
            let v2 = [nxt[0] - p[0], nxt[1] - p[1]];
            let cross = (v1[0] * v2[1] - v1[1] * v2[0]).abs();
            let dot = v1[0] * v2[0] + v1[1] * v2[1];
            let scale = ((v1[0] * v1[0] + v1[1] * v1[1]) * (v2[0] * v2[0] + v2[1] * v2[1])).sqrt();
            if dot > 0.0 && cross <= 1e-6 * scale.max(1.0) {
                changed = true;
            } else {
                next.push(p);
            }
        }
        if next.len() >= 3 {
            cur = next;
        } else {
            break;
        }
    }
    cur
}

fn dedup_ring_points(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for &p in pts {
        if out.last().is_none_or(|&prev| dist2(prev, p) > SNAP_EPS) {
            out.push(p);
        }
    }
    while out.len() >= 2 && dist2(out[0], *out.last().unwrap()) <= SNAP_EPS {
        out.pop();
    }
    out
}

fn dedup_open_points(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for &p in pts {
        if out.last().is_none_or(|&prev| dist2(prev, p) > SNAP_EPS) {
            out.push(p);
        }
    }
    out
}

fn polyline_length(pts: &[[f64; 2]]) -> f64 {
    pts.windows(2).map(|w| dist2(w[0], w[1])).sum()
}

fn find_shape_with_world_transform(
    page: &VisioPage,
    target_id: u64,
) -> Option<(&VisioShape, ShapeTransform2D)> {
    fn walk<'a>(
        shape: &'a VisioShape,
        parent_tf: Option<&ShapeTransform2D>,
        target_id: u64,
    ) -> Option<(&'a VisioShape, ShapeTransform2D)> {
        let ss = &shape.sheet;
        let w = ss.get_f64("Width").unwrap_or(1.0);
        let h = ss.get_f64("Height").unwrap_or(if shape.kind == ShapeKind::Connector1D {
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
                ..local_tf
            }
        } else {
            local_tf
        };
        if shape.id == target_id {
            return Some((shape, world_tf));
        }
        for child in &shape.children {
            if let Some(found) = walk(child, Some(&world_tf), target_id) {
                return Some(found);
            }
        }
        None
    }

    for s in &page.shapes {
        if let Some(found) = walk(s, None, target_id) {
            return Some(found);
        }
    }
    None
}

fn remove_shapes_by_id_set(shapes: &mut Vec<VisioShape>, ids: &BTreeSet<u64>) {
    shapes.retain(|s| !ids.contains(&s.id));
    for s in shapes {
        remove_shapes_by_id_set(&mut s.children, ids);
    }
}

fn is_relative_row_kind(kind: GeometryRowKind) -> bool {
    matches!(
        kind,
        GeometryRowKind::RelMoveTo
            | GeometryRowKind::RelLineTo
            | GeometryRowKind::RelCubBezTo
            | GeometryRowKind::RelQuadBezTo
            | GeometryRowKind::RelEllipticalArcTo
    )
}

fn renumber_geometry_rows(rows: &mut [GeometryRow]) {
    let mut next_ix = 1u32;
    for r in rows {
        if !r.deleted {
            r.ix = next_ix;
            next_ix += 1;
        }
    }
}

fn purge_geometry_cells(sheet: &mut ShapeSheet, geom_index: u32) {
    let prefix = canonical_cell_name(&format!("Geometry{geom_index}."));
    sheet.cells.retain(|k, _| !k.starts_with(&prefix));
}

fn eval_or_parse_row_coord(sheet: &ShapeSheet, cell_name: &str, raw_formula: &str) -> f64 {
    sheet
        .get_f64(cell_name)
        .unwrap_or_else(|| parse_fallback_inches(raw_formula))
}

fn parse_fallback_inches(raw: &str) -> f64 {
    let trimmed = raw.trim().trim_start_matches('=');
    parse_scalar_with_unit(trimmed)
        .map(|(v, u)| u.to_internal(v))
        .unwrap_or(0.0)
}

#[inline]
fn is_finite_pt(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

#[inline]
fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]
}

#[inline]
fn dist2(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

fn point_to_segment_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-14 {
        return dist2(p, a);
    }
    let t = (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len_sq).clamp(0.0, 1.0);
    dist2(p, [a[0] + t * dx, a[1] + t * dy])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_overlapping_rects_doc() -> VisioDocument {
        let mut doc = VisioDocument::new_blank("BooleanOpsTest");
        {
            let page = doc.page_mut(1).unwrap();
            // Shape 1: `[1.0..3.0] x [1.0..3.0]` (Pin = (2, 2), W = 2, H = 2, Area = 4)
            let s1 = VisioShape::new_rect(1, "RectA", 2.0, 2.0, 2.0, 2.0);
            // Shape 2: `[2.0..4.0] x [1.0..3.0]` (Pin = (3, 2), W = 2, H = 2, Area = 4)
            // Overlap `[2.0..3.0] x [1.0..3.0]` has Area = 2; Union `[1.0..4.0] x [1.0..3.0]` has Area = 6.
            let s2 = VisioShape::new_rect(2, "RectB", 3.0, 2.0, 2.0, 2.0);
            page.shapes.push(s1);
            page.shapes.push(s2);
        }
        ReactiveEngine::recalculate_document(&mut doc);
        doc
    }

    #[test]
    fn boolean_union_intersect_subtract_combine_and_fragment_produce_exact_geometry() {
        // 1. Union -> single `[1..4] x [1..3]` shape (`Width = 3.0, Height = 2.0, PinX = 2.5`)
        let mut doc_u = two_overlapping_rects_doc();
        let u_ids = apply_boolean_operation(&mut doc_u, 1, &[1, 2], BooleanOpKind::Union);
        assert_eq!(u_ids.len(), 1);
        assert_eq!(doc_u.page(1).unwrap().shapes.len(), 1);
        let u_shape = doc_u.page(1).unwrap().find_shape(u_ids[0]).unwrap();
        assert!((u_shape.sheet.get_f64("Width").unwrap() - 3.0).abs() < 1e-4);
        assert!((u_shape.sheet.get_f64("Height").unwrap() - 2.0).abs() < 1e-4);
        assert!((u_shape.sheet.get_f64("PinX").unwrap() - 2.5).abs() < 1e-4);

        // 2. Intersect -> single `[2..3] x [1..3]` shape (`Width = 1.0, Height = 2.0, PinX = 2.5`)
        let mut doc_i = two_overlapping_rects_doc();
        let i_ids = apply_boolean_operation(&mut doc_i, 1, &[1, 2], BooleanOpKind::Intersect);
        assert_eq!(i_ids.len(), 1);
        let i_shape = doc_i.page(1).unwrap().find_shape(i_ids[0]).unwrap();
        assert!((i_shape.sheet.get_f64("Width").unwrap() - 1.0).abs() < 1e-4);
        assert!((i_shape.sheet.get_f64("Height").unwrap() - 2.0).abs() < 1e-4);
        assert!((i_shape.sheet.get_f64("PinX").unwrap() - 2.5).abs() < 1e-4);

        // 3. Subtract (`RectA \ RectB`) -> `[1..2] x [1..3]` (`Width = 1.0, Height = 2.0, PinX = 1.5`)
        let mut doc_s = two_overlapping_rects_doc();
        let s_ids = apply_boolean_operation(&mut doc_s, 1, &[1, 2], BooleanOpKind::Subtract);
        assert_eq!(s_ids.len(), 1);
        let s_shape = doc_s.page(1).unwrap().find_shape(s_ids[0]).unwrap();
        assert!((s_shape.sheet.get_f64("Width").unwrap() - 1.0).abs() < 1e-4);
        assert!((s_shape.sheet.get_f64("PinX").unwrap() - 1.5).abs() < 1e-4);

        // 4. Combine (symmetric XOR) -> 1 shape with 2 GeometrySections (`[1..2]x[1..3]` and `[3..4]x[1..3]`)
        let mut doc_c = two_overlapping_rects_doc();
        let c_ids = apply_boolean_operation(&mut doc_c, 1, &[1, 2], BooleanOpKind::Combine);
        assert_eq!(c_ids.len(), 1);
        let c_shape = doc_c.page(1).unwrap().find_shape(c_ids[0]).unwrap();
        assert_eq!(c_shape.sheet.geometries.len(), 2);
        assert!((c_shape.sheet.get_f64("Width").unwrap() - 3.0).abs() < 1e-4);

        // 5. Fragment -> 3 independent shapes (`A \ B`, `A ∩ B`, `B \ A`), each `1.0 x 2.0 in`!
        let mut doc_f = two_overlapping_rects_doc();
        let f_ids = apply_boolean_operation(&mut doc_f, 1, &[1, 2], BooleanOpKind::Fragment);
        assert_eq!(f_ids.len(), 3);
        assert_eq!(doc_f.page(1).unwrap().shapes.len(), 3);
        for fid in f_ids {
            let f_sh = doc_f.page(1).unwrap().find_shape(fid).unwrap();
            assert!((f_sh.sheet.get_f64("Width").unwrap() - 1.0).abs() < 1e-4);
            assert!((f_sh.sheet.get_f64("Height").unwrap() - 2.0).abs() < 1e-4);
        }
    }

    #[test]
    fn boolean_trim_join_and_offset_operations_work_deterministically() {
        // Trim: crossing horizontal & vertical lines split into 4 open sub-curves.
        let mut doc_t = VisioDocument::new_blank("TrimTest");
        {
            let p = doc_t.page_mut(1).unwrap();
            p.shapes.push(VisioShape::new_connector(1, "Horiz", 1.0, 3.0, 5.0, 3.0));
            p.shapes.push(VisioShape::new_connector(2, "Vert", 3.0, 1.0, 3.0, 5.0));
        }
        let trim_ids = apply_boolean_operation(&mut doc_t, 1, &[1, 2], BooleanOpKind::Trim);
        assert_eq!(trim_ids.len(), 4);

        // Join: two open segments within `0.05 in` (`0.03 in` gap) weld into a single continuous polyline.
        let mut doc_j = VisioDocument::new_blank("JoinTest");
        {
            let p = doc_j.page_mut(1).unwrap();
            p.shapes.push(VisioShape::new_connector(1, "Seg1", 1.0, 2.0, 3.0, 2.0));
            p.shapes.push(VisioShape::new_connector(2, "Seg2", 3.03, 2.0, 5.0, 4.0));
        }
        let join_ids = apply_boolean_operation(&mut doc_j, 1, &[1, 2], BooleanOpKind::Join);
        assert_eq!(join_ids.len(), 1);
        let joined = doc_j.page(1).unwrap().find_shape(join_ids[0]).unwrap();
        assert_eq!(joined.sheet.geometries.len(), 1);
        assert_eq!(joined.sheet.geometries[0].rows.len(), 3);
        assert!((joined.sheet.get_f64("Width").unwrap() - 4.0).abs() < 1e-3);

        // Offset: `+0.25 in` outward offset on a `2.0 x 2.0 in` square adds a `2.5 x 2.5 in` square while preserving source.
        let mut doc_o = VisioDocument::new_blank("OffsetTest");
        doc_o
            .page_mut(1)
            .unwrap()
            .shapes
            .push(VisioShape::new_rect(1, "Base", 4.0, 4.0, 2.0, 2.0));
        let off_ids = apply_boolean_operation(&mut doc_o, 1, &[1], BooleanOpKind::Offset(0.25));
        assert_eq!(off_ids.len(), 1);
        assert_eq!(doc_o.page(1).unwrap().shapes.len(), 2);
        let off_shape = doc_o.page(1).unwrap().find_shape(off_ids[0]).unwrap();
        assert!((off_shape.sheet.get_f64("Width").unwrap() - 2.5).abs() < 1e-4);
        assert!((off_shape.sheet.get_f64("Height").unwrap() - 2.5).abs() < 1e-4);
    }

    #[test]
    fn vertex_edit_freeform_curve_and_yellow_control_handle_drag() {
        let mut doc = VisioDocument::new_blank("InteractiveEditTest");
        doc.page_mut(1)
            .unwrap()
            .shapes
            .push(VisioShape::new_rect(1, "EditableBox", 3.0, 3.0, 2.0, 2.0));
        ReactiveEngine::recalculate_document(&mut doc);

        let initial_verts = list_shape_vertices(&doc, 1, 1);
        assert_eq!(initial_verts.len(), 5);

        // Move vertex 3 (`(4.0, 4.0)`) to `(4.5, 4.25)`, insert vertex after row 3, then delete row 4.
        assert!(move_shape_vertex(&mut doc, 1, 1, 1, 3, 4.5, 4.25));
        let moved_verts = list_shape_vertices(&doc, 1, 1);
        assert!((moved_verts[2].page_x_in - 4.5).abs() < 1e-6);
        assert!((moved_verts[2].page_y_in - 4.25).abs() < 1e-6);

        assert!(insert_shape_vertex(&mut doc, 1, 1, 1, 3, 3.5, 4.75));
        assert_eq!(list_shape_vertices(&doc, 1, 1).len(), 6);

        assert!(delete_shape_vertex(&mut doc, 1, 1, 1, 4));
        assert_eq!(list_shape_vertices(&doc, 1, 1).len(), 5);

        // Freeform curve fitting (dense noisy stroke -> RDP simplified smooth `RelCubBezTo` shape).
        let raw_stroke: Vec<[f64; 2]> = (0..=24)
            .map(|i| {
                let t = i as f64 / 24.0;
                [1.0 + t * 4.0, 2.0 + (t * PI).sin() * 1.2]
            })
            .collect();
        let curve_id = draw_freeform_curve(&mut doc, 1, &raw_stroke, false).expect("freeform shape");
        let curve_shape = doc.page(1).unwrap().find_shape(curve_id).unwrap();
        assert!(curve_shape.sheet.geometries[0].rows.len() < raw_stroke.len());
        assert!(
            curve_shape.sheet.geometries[0]
                .rows
                .iter()
                .any(|r| r.kind == GeometryRowKind::RelCubBezTo)
        );

        // Yellow diamond `Controls.Row_1` creation & drag -> updates Rounding reactively!
        assert!(ensure_default_control_handle(&mut doc, 1, 1));
        let handles = list_control_handles(&doc, 1, 1);
        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0].name, "Row_1");
        assert!(handles[0].can_glue);

        // Shape 1 has Pin=(3,3), W=2, H=2 => local origin is `(2.0, 2.0)`.
        // Dragging `Row_1` to page `(2.4, 4.0)` sets local `Controls.Row_1.X = 0.4 in`, `Rounding = 0.4 in`.
        assert!(move_control_handle(&mut doc, 1, 1, "Row_1", 2.4, 4.0));
        let updated_handles = list_control_handles(&doc, 1, 1);
        assert!((updated_handles[0].page_x_in - 2.4).abs() < 1e-6);
        assert!((updated_handles[0].page_y_in - 4.0).abs() < 1e-6);
        let box_shape = doc.page(1).unwrap().find_shape(1).unwrap();
        assert!((box_shape.sheet.get_f64("Rounding").unwrap() - 0.4).abs() < 1e-6);
    }
}
