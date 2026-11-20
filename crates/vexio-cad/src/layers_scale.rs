//! Architectural / Engineering CAD Scale Locking, Layer State Control,
//! and CAD-to-Visio Editable Shape Conversion.

use crate::dxf_codec::set_shape_lock_flags;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use vexio_shapesheet::{
    CellValue, ConnectionPointRow, GeometryRowKind, LayerDef, RgbaColor, ShapeKind, Unit,
    VisioPage, VisioShape, format_clean_f64,
};

/// Canonical architectural, metric, and custom CAD drawing scale presets.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CadScalePreset {
    /// `1/4" = 1'-0"` (ratio 1:48)
    ArchitecturalQuarterInchPerFoot,
    /// `1/8" = 1'-0"` (ratio 1:96)
    ArchitecturalEighthInchPerFoot,
    /// `1:50` metric architectural/engineering scale (`1 mm = 50 mm`)
    Metric1To50,
    /// `1:100` metric architectural/engineering scale (`1 mm = 100 mm`)
    Metric1To100,
    /// `1:200` metric site/floorplan scale (`1 mm = 200 mm`)
    Metric1To200,
    /// Arbitrary custom scale (`page_in` inches on paper = `drawing_in` inches in real world)
    Custom { page_in: f64, drawing_in: f64 },
}

impl CadScalePreset {
    /// Return `(page_scale_in, drawing_scale_in)` in Visio canonical inches.
    pub fn to_inches_pair(self) -> (f64, f64) {
        match self {
            Self::ArchitecturalQuarterInchPerFoot => (0.25, 12.0),
            Self::ArchitecturalEighthInchPerFoot => (0.125, 12.0),
            Self::Metric1To50 => (1.0 / 25.4, 50.0 / 25.4),
            Self::Metric1To100 => (1.0 / 25.4, 100.0 / 25.4),
            Self::Metric1To200 => (1.0 / 25.4, 200.0 / 25.4),
            Self::Custom {
                page_in,
                drawing_in,
            } => {
                let p = if page_in.is_finite() && page_in.abs() > 1e-9 {
                    page_in.abs()
                } else {
                    1.0
                };
                let d = if drawing_in.is_finite() && drawing_in.abs() > 1e-9 {
                    drawing_in.abs()
                } else {
                    1.0
                };
                (p, d)
            }
        }
    }

    /// Dimensionless ratio `DrawingScale / PageScale` (e.g. `48.0` for `1/4" = 1'-0"`).
    pub fn ratio(self) -> f64 {
        let (p, d) = self.to_inches_pair();
        d / p
    }

    fn display_literals_and_units(self) -> (f64, Unit, f64, Unit) {
        match self {
            Self::ArchitecturalQuarterInchPerFoot => (0.25, Unit::Inch, 1.0, Unit::Foot),
            Self::ArchitecturalEighthInchPerFoot => (0.125, Unit::Inch, 1.0, Unit::Foot),
            Self::Metric1To50 => (1.0, Unit::Millimeter, 50.0, Unit::Millimeter),
            Self::Metric1To100 => (1.0, Unit::Millimeter, 100.0, Unit::Millimeter),
            Self::Metric1To200 => (1.0, Unit::Millimeter, 200.0, Unit::Millimeter),
            Self::Custom { .. } => {
                let (p, d) = self.to_inches_pair();
                (p, Unit::Inch, d, Unit::Inch)
            }
        }
    }
}

/// Apply an architectural or metric CAD scale to `page` (`PageScale` & `DrawingScale`),
/// optionally locking the scale with `GUARD(...)` and locking the CAD background layer(s).
pub fn apply_cad_scale_lock(page: &mut VisioPage, scale: CadScalePreset, lock: bool) {
    let (p_val, p_unit, d_val, d_unit) = scale.display_literals_and_units();
    let p_lit = format!("{}{}", format_clean_f64(p_val), p_unit.suffix());
    let d_lit = format!("{}{}", format_clean_f64(d_val), d_unit.suffix());

    let p_formula = if lock {
        format!("=GUARD({p_lit})")
    } else {
        p_lit
    };
    let d_formula = if lock {
        format!("=GUARD({d_lit})")
    } else {
        d_lit
    };

    page.page_sheet
        .set_formula("PageScale", p_formula, Some(p_unit));
    if let Some(cell) = page.page_sheet.get_cell_mut("PageScale") {
        cell.value = CellValue::with_unit(p_val, p_unit);
    }

    page.page_sheet
        .set_formula("DrawingScale", d_formula, Some(d_unit));
    if let Some(cell) = page.page_sheet.get_cell_mut("DrawingScale") {
        cell.value = CellValue::with_unit(d_val, d_unit);
    }

    page.page_sheet.set_user_cell(
        "CadScaleLocked",
        if lock { "TRUE" } else { "FALSE" },
        "CAD Architectural Scale Lock",
    );
    page.page_sheet.set_user_cell(
        "CadScaleRatio",
        format_clean_f64(scale.ratio()),
        "CAD Drawing/Page Scale Ratio",
    );

    // Identify layers populated by unconverted CAD shapes
    let mut cad_layer_indices = BTreeSet::new();
    for s in &page.shapes {
        collect_unconverted_cad_layer_indices(s, &mut cad_layer_indices);
    }

    // Ensure a canonical "CAD Background" layer exists (or update it)
    let bg_ix = if let Some(existing) = page
        .layers
        .iter_mut()
        .find(|l| l.name.eq_ignore_ascii_case("CAD Background"))
    {
        existing.lock = lock;
        existing.ix
    } else {
        let next_ix = page.layers.iter().map(|l| l.ix).max().map_or(0, |m| m + 1);
        let mut bg = LayerDef::new(next_ix, "CAD Background");
        bg.lock = lock;
        page.layers.push(bg);
        next_ix
    };

    if lock {
        for layer in &mut page.layers {
            if layer.ix == bg_ix || cad_layer_indices.contains(&layer.ix) {
                layer.lock = true;
            }
        }
        for s in &mut page.shapes {
            sync_cad_tree_lock(s, &cad_layer_indices, bg_ix, true);
        }
    } else {
        for layer in &mut page.layers {
            if layer.ix == bg_ix {
                layer.lock = false;
            }
        }
        for s in &mut page.shapes {
            let single_bg = BTreeSet::from([bg_ix]);
            sync_cad_tree_lock(s, &single_bg, bg_ix, false);
        }
    }
}

fn collect_unconverted_cad_layer_indices(shape: &VisioShape, set: &mut BTreeSet<u32>) {
    let is_cad = shape
        .sheet
        .get_f64("User.CadEntity")
        .map_or(false, |v| v > 0.5);
    let converted = shape.sheet.get_bool("User.CadConverted").unwrap_or(false);
    if is_cad && !converted {
        for &ix in &shape.layer_membership {
            set.insert(ix);
        }
    }
    for ch in &shape.children {
        collect_unconverted_cad_layer_indices(ch, set);
    }
}

fn sync_cad_tree_lock(
    shape: &mut VisioShape,
    target_layers: &BTreeSet<u32>,
    extra_ix: u32,
    locked: bool,
) {
    let converted = shape.sheet.get_bool("User.CadConverted").unwrap_or(false);
    if !converted
        && shape
            .layer_membership
            .iter()
            .any(|ix| *ix == extra_ix || target_layers.contains(ix))
    {
        set_shape_lock_flags(&mut shape.sheet, locked);
    }
    for ch in &mut shape.children {
        sync_cad_tree_lock(ch, target_layers, extra_ix, locked);
    }
}

/// Toggle visibility, lock status, or layer color override for a CAD layer on `page`.
pub fn set_cad_layer_state(
    page: &mut VisioPage,
    layer_name: &str,
    visible: Option<bool>,
    locked: Option<bool>,
    color_override: Option<Option<RgbaColor>>,
) {
    let Some(layer) = page
        .layers
        .iter_mut()
        .find(|l| l.name.eq_ignore_ascii_case(layer_name) || l.name_u.eq_ignore_ascii_case(layer_name))
    else {
        return;
    };

    if let Some(vis) = visible {
        layer.visible = vis;
        if vis && !layer.print {
            // Thawing a previously frozen CAD layer enables printing as well
            layer.print = true;
        }
    }
    if let Some(lck) = locked {
        layer.lock = lck;
    }
    if let Some(co) = color_override {
        layer.color_override = co;
    }
    let target_ix = layer.ix;

    if let Some(lck) = locked {
        for shape in &mut page.shapes {
            apply_layer_lock_to_shape(shape, target_ix, lck);
        }
    }
}

fn apply_layer_lock_to_shape(shape: &mut VisioShape, layer_ix: u32, locked: bool) {
    if shape.layer_membership.contains(&layer_ix) {
        set_shape_lock_flags(&mut shape.sheet, locked);
    }
    for ch in &mut shape.children {
        apply_layer_lock_to_shape(ch, layer_ix, locked);
    }
}

/// Unlock and convert imported CAD entities on `layer_name` into standard editable Visio shapes
/// with static/dynamic connection points (`Connections.X{i}` / `Connections.Y{i}`) at endpoints,
/// vertices, quadrants, and centers. Returns the IDs of all converted shapes.
pub fn convert_cad_layer_to_visio_shapes(page: &mut VisioPage, layer_name: &str) -> Vec<u64> {
    let Some(layer) = page
        .layers
        .iter_mut()
        .find(|l| l.name.eq_ignore_ascii_case(layer_name) || l.name_u.eq_ignore_ascii_case(layer_name))
    else {
        return Vec::new();
    };
    layer.lock = false;
    layer.snap = true;
    layer.glue = true;
    let target_ix = layer.ix;

    let mut converted_ids = Vec::new();
    for shape in &mut page.shapes {
        convert_shape_recursive(shape, target_ix, false, &mut converted_ids);
    }
    converted_ids
}

fn convert_shape_recursive(
    shape: &mut VisioShape,
    target_ix: u32,
    parent_matched: bool,
    converted_ids: &mut Vec<u64>,
) {
    let matches_layer = parent_matched || shape.layer_membership.contains(&target_ix);
    if matches_layer {
        unlock_and_populate_connections(shape);
        converted_ids.push(shape.id);
    }
    for child in &mut shape.children {
        convert_shape_recursive(child, target_ix, matches_layer, converted_ids);
    }
}

fn unlock_and_populate_connections(shape: &mut VisioShape) {
    set_shape_lock_flags(&mut shape.sheet, false);
    for extra_lock in ["LockSelect", "LockFormat", "LockTextEdit", "LockAspect"] {
        shape
            .sheet
            .set_formula(extra_lock, "FALSE", Some(Unit::Boolean));
    }
    shape.sheet.set_user_cell(
        "CadConverted",
        "TRUE",
        "Converted to standard editable Visio shape",
    );
    if let Some(fd) = &shape.foreign_data {
        if fd.media_type.eq_ignore_ascii_case("CAD") {
            shape.foreign_data = None;
        }
    }

    // Collect local candidate connection points: endpoints, vertices, ellipse quadrants, and center
    let w = shape.sheet.get_f64("Width").unwrap_or(0.0);
    let h = shape.sheet.get_f64("Height").unwrap_or(0.0);
    let center = [w * 0.5, h * 0.5];
    let mut candidate_pts: Vec<([f64; 2], &'static str)> = Vec::new();

    for geom in &shape.sheet.geometries {
        if geom.deleted || geom.no_show.eq_ignore_ascii_case("TRUE") {
            continue;
        }
        let prefix = format!("Geometry{}", geom.index);
        let mut prev_pt: Option<[f64; 2]> = None;
        for row in &geom.rows {
            if row.deleted {
                continue;
            }
            let ix = row.ix;
            let rx = shape.sheet.get_f64(&format!("{prefix}.X{ix}")).unwrap_or(0.0);
            let ry = shape.sheet.get_f64(&format!("{prefix}.Y{ix}")).unwrap_or(0.0);
            match row.kind {
                GeometryRowKind::MoveTo | GeometryRowKind::LineTo | GeometryRowKind::PolylineTo => {
                    push_unique_pt(&mut candidate_pts, [rx, ry], "Vertex");
                    prev_pt = Some([rx, ry]);
                }
                GeometryRowKind::ArcTo => {
                    if let Some(p0) = prev_pt {
                        let bow = shape.sheet.get_f64(&format!("{prefix}.A{ix}")).unwrap_or(0.0);
                        let dx = rx - p0[0];
                        let dy = ry - p0[1];
                        let chord = dx.hypot(dy);
                        if chord > 1e-9 && bow.abs() > 1e-9 {
                            // Midpoint of arc = chord midpoint displaced along right normal by `bow`
                            // (or `-left_normal * bow` matching bulge sagitta)
                            let mx = (p0[0] + rx) * 0.5 + (dy / chord) * bow;
                            let my = (p0[1] + ry) * 0.5 - (dx / chord) * bow;
                            push_unique_pt(&mut candidate_pts, [mx, my], "ArcMid");
                        }
                    }
                    push_unique_pt(&mut candidate_pts, [rx, ry], "Vertex");
                    prev_pt = Some([rx, ry]);
                }
                GeometryRowKind::Ellipse => {
                    let ax = shape
                        .sheet
                        .get_f64(&format!("{prefix}.A{ix}"))
                        .unwrap_or(rx + w * 0.5);
                    let ay = shape
                        .sheet
                        .get_f64(&format!("{prefix}.B{ix}"))
                        .unwrap_or(ry);
                    let cx_p = shape
                        .sheet
                        .get_f64(&format!("{prefix}.C{ix}"))
                        .unwrap_or(rx);
                    let dy_p = shape
                        .sheet
                        .get_f64(&format!("{prefix}.D{ix}"))
                        .unwrap_or(ry + h * 0.5);
                    let rad_x = ((ax - rx).powi(2) + (ay - ry).powi(2)).sqrt();
                    let rad_y = ((cx_p - rx).powi(2) + (dy_p - ry).powi(2)).sqrt();
                    push_unique_pt(&mut candidate_pts, [rx, ry], "Center");
                    push_unique_pt(&mut candidate_pts, [rx + rad_x, ry], "QuadEast");
                    push_unique_pt(&mut candidate_pts, [rx - rad_x, ry], "QuadWest");
                    push_unique_pt(&mut candidate_pts, [rx, ry + rad_y], "QuadNorth");
                    push_unique_pt(&mut candidate_pts, [rx, ry - rad_y], "QuadSouth");
                }
                _ => {
                    let scale = if matches!(
                        row.kind,
                        GeometryRowKind::RelMoveTo
                            | GeometryRowKind::RelLineTo
                            | GeometryRowKind::RelCubBezTo
                            | GeometryRowKind::RelQuadBezTo
                            | GeometryRowKind::RelEllipticalArcTo
                    ) {
                        (w, h)
                    } else {
                        (1.0, 1.0)
                    };
                    push_unique_pt(
                        &mut candidate_pts,
                        [rx * scale.0, ry * scale.1],
                        "Vertex",
                    );
                }
            }
        }
    }

    // Always include the shape's geometric center (`LocPin`)
    push_unique_pt(&mut candidate_pts, center, "Center");

    // For pure text or Group containers (where Geometry1 is NoShow), add 4 cardinal midpoints
    if shape.kind == ShapeKind::Group || candidate_pts.len() == 1 {
        if w > 1e-5 || h > 1e-5 {
            push_unique_pt(&mut candidate_pts, [w * 0.5, 0.0], "Bottom");
            push_unique_pt(&mut candidate_pts, [w, h * 0.5], "Right");
            push_unique_pt(&mut candidate_pts, [w * 0.5, h], "Top");
            push_unique_pt(&mut candidate_pts, [0.0, h * 0.5], "Left");
        }
    }

    shape.sheet.connection_points.clear();
    for (idx, (pt, label)) in candidate_pts.into_iter().enumerate() {
        let cp_ix = (idx + 1) as u32;
        let dx = pt[0] - center[0];
        let dy = pt[1] - center[1];
        let norm = dx.hypot(dy);
        let (dir_x, dir_y) = if norm > 1e-6 {
            (dx / norm, dy / norm)
        } else {
            (0.0, 0.0)
        };
        shape.sheet.add_connection_point(ConnectionPointRow {
            name: format!("{label}_{cp_ix}"),
            ix: cp_ix,
            x: format!("{} in", format_clean_f64(pt[0])),
            y: format!("{} in", format_clean_f64(pt[1])),
            dir_x: format!("{} in", format_clean_f64(dir_x)),
            dir_y: format!("{} in", format_clean_f64(dir_y)),
            point_type: 2, // Inward & Outward glue
        });
    }
}

fn push_unique_pt(list: &mut Vec<([f64; 2], &'static str)>, pt: [f64; 2], label: &'static str) {
    if !pt[0].is_finite() || !pt[1].is_finite() {
        return;
    }
    if list
        .iter()
        .all(|(existing, _)| (existing[0] - pt[0]).hypot(existing[1] - pt[1]) > 1e-5)
    {
        list.push((pt, label));
    }
}
