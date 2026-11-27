//! Milestone 4 (`v0.9.0-beta`): Enterprise AEC Architecture & Plan 2 Analytical Calculators
//!
//! 1. Associative Architectural Dimensions (`Aligned`, `Horizontal`, `Vertical`, `Angular`, `Radial`, `Diameter`)
//!    with live cross-shape ShapeSheet formulas and `<fld>` dynamic measurement runs.
//! 2. Architectural Walls with exact L-Corner Miter & T-Join Auto-Healing (`draw_architectural_wall`,
//!    `heal_wall_intersections`).
//! 3. Parametric Door & Window Wall Openings (`insert_wall_opening`) that split host wall geometry
//!    around clear openings and attach architectural door-swing arcs / glazing panes.
//! 4. Deterministic Proleptic Gregorian Calendar Wizard (`build_calendar_diagram`).
//! 5. Hierarchical PivotDiagram Rollup Builder (`build_pivot_diagram` — `SUM`, `AVG`, `COUNT`, `MIN`, `MAX`).
//! 6. Brainstorming / Mind Map Generator, Topic Mutator & Markdown Outline Exporter
//!    (`build_mind_map_from_outline`, `add_mind_map_topic`, `export_mind_map_outline`).
//! 7. Lean Value Stream Mapping (`VSM` Lead Time / Value-Added / PCE% Sawtooth Ladder) and
//!    Fault Tree Analysis (`FTA` `AND`/`OR`/`XOR`/`NOT`/`k-of-n` Gate Probabilities, MOCUS Minimal
//!    Cut Sets, and Birnbaum Importance) (`build_vsm_diagram`, `build_fault_tree_diagram`,
//!    `recalculate_vsm_and_fta`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use vexio_shapesheet::{
    ControlRow, GeometryRow, GeometrySection, GlueEndpoint, ReactiveEngine, ShapeDataRow,
    ShapeDataType, ShapeKind, TextFieldRow, Unit, VisioDocument, VisioShape, VisioTextRun,
    format_clean_f64, runs_plain_text,
};

// ============================================================================
// 1. Associative Architectural Dimensions
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssociativeDimensionKind {
    Aligned,
    Horizontal,
    Vertical,
    Angular,
    Radial,
    Diameter,
}

impl AssociativeDimensionKind {
    pub fn from_slug(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "horizontal" | "horiz" | "linear_h" => Self::Horizontal,
            "vertical" | "vert" | "linear_v" => Self::Vertical,
            "angular" | "angle" => Self::Angular,
            "radial" | "radius" => Self::Radial,
            "diameter" | "diam" => Self::Diameter,
            _ => Self::Aligned,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Aligned => "aligned",
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
            Self::Angular => "angular",
            Self::Radial => "radial",
            Self::Diameter => "diameter",
        }
    }
}

fn resolve_dimension_unit_spec(unit_format: &str, doc: &VisioDocument, page_id: u32) -> (String, f64) {
    let norm = unit_format.trim().to_ascii_lowercase();
    let effective = if norm.is_empty() || norm == "auto" {
        let ps = doc
            .page(page_id)
            .and_then(|p| p.page_sheet.get_cell("DrawingScale"))
            .map(|c| c.formula.to_ascii_lowercase())
            .unwrap_or_default();
        if ps.contains("mm") {
            "mm"
        } else if ps.contains("m") && !ps.contains("mm") {
            "m"
        } else if ps.contains("ft") || ps.contains('\'') {
            "ft"
        } else {
            "in"
        }
    } else {
        norm.as_str()
    };
    match effective {
        "ft" | "feet" | "ft-in" => ("0.00 ft".to_string(), 1.0 / 12.0),
        "mm" | "millimeters" => ("0.0 mm".to_string(), 25.4),
        "cm" | "centimeters" => ("0.00 cm".to_string(), 2.54),
        "m" | "meters" => ("0.00 m".to_string(), 0.0254),
        _ => ("0.00 in".to_string(), 1.0),
    }
}

fn attach_live_dimension_field(
    shape: &mut VisioShape,
    prefix: &str,
    formula: &str,
    format_spec: &str,
) {
    shape.sheet.text_fields.clear();
    shape.sheet.text_runs.clear();
    if !prefix.is_empty() {
        shape.sheet.text_runs.push(VisioTextRun::Text {
            text: prefix.to_string(),
            cp_ix: 0,
            pp_ix: 0,
        });
    }
    shape.sheet.upsert_text_field(TextFieldRow {
        ix: 0,
        value: formula.to_string(),
        format: format_spec.to_string(),
    });
    shape.sheet.text_runs.push(VisioTextRun::Field {
        field_ix: 0,
        cp_ix: 0,
        pp_ix: 0,
        cached: String::new(),
    });
    shape.text = runs_plain_text(&shape.sheet.text_runs);
}

/// Insert a reactive Associative Architectural Dimension (`Aligned`, `Horizontal`, `Vertical`,
/// `Angular`, `Radial`, or `Diameter`) bound via ShapeSheet cross-shape formulas and `<fld>` runs.
pub fn insert_associative_dimension(
    doc: &mut VisioDocument,
    page_id: u32,
    dim_kind_slug: &str,
    target_shape_id: u64,
    second_shape_id: u64,
    offset_in: f64,
    unit_format: &str,
) -> Option<u64> {
    let mut kind = AssociativeDimensionKind::from_slug(dim_kind_slug);
    let clean_offset = if offset_in.abs() < 0.05 { 0.45 } else { offset_in };
    let (fmt_spec, unit_scale_from_in) = resolve_dimension_unit_spec(unit_format, doc, page_id);

    // Auto-select target shape if 0 was passed and the page has a non-guide shape
    let resolved_target = if target_shape_id > 0 {
        target_shape_id
    } else {
        doc.page(page_id)?
            .shapes
            .iter()
            .find(|s| s.kind != ShapeKind::Guide)
            .map(|s| s.id)
            .unwrap_or(0)
    };
    if resolved_target == 0 || doc.page(page_id)?.find_shape(resolved_target).is_none() {
        return None;
    }

    // If Aligned on a single shape with no explicit axis, pick Horizontal vs Vertical by shape aspect
    if kind == AssociativeDimensionKind::Aligned && second_shape_id == 0 {
        if let Some(tsh) = doc.page(page_id)?.find_shape(resolved_target) {
            let w = tsh.sheet.get_f64("Width").unwrap_or(2.0).abs();
            let h = tsh.sheet.get_f64("Height").unwrap_or(1.0).abs();
            let ang = tsh.sheet.get_f64("Angle").unwrap_or(0.0).abs();
            if ang < 1e-3 && h > w * 1.25 {
                kind = AssociativeDimensionKind::Vertical;
            } else if ang < 1e-3 {
                kind = AssociativeDimensionKind::Horizontal;
            }
        }
    }

    let dim_id = doc.page_mut(page_id)?.next_shape_id();
    let mut dim = VisioShape::new_rect(
        dim_id,
        format!("Dimension.{dim_id}"),
        4.0,
        4.0,
        2.0,
        0.36,
    );
    dim.sheet.geometries.clear();
    dim.sheet.set_user_cell("DomainRole", "\"arch_dimension\"", "Associative Architectural Dimension");
    dim.sheet.set_user_cell("DimensionKind", format!("\"{}\"", kind.slug()), "Dimension Type");
    dim.sheet.set_user_cell("TargetShapeId", resolved_target.to_string(), "Measured Target Shape");
    dim.sheet.set_user_cell("SecondShapeId", second_shape_id.to_string(), "Secondary Target Shape");
    dim.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    dim.sheet.set_formula("LineColor", "=RGB(15,23,42)", Some(Unit::Color));
    dim.sheet.set_formula("LineWeight", "0.75 pt", Some(Unit::Point));
    dim.sheet.set_formula("Char.Size", "8.5 pt", Some(Unit::Point));
    dim.sheet.set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));
    dim.sheet.set_formula("TextBkgnd", "=RGB(255,255,255)", Some(Unit::Color));

    let scale_expr = format!(
        "(ThePage!DrawingScale / ThePage!PageScale) * {}",
        format_clean_f64(unit_scale_from_in)
    );

    match kind {
        AssociativeDimensionKind::Horizontal => {
            if second_shape_id > 0 && doc.page(page_id)?.find_shape(second_shape_id).is_some() {
                dim.sheet.set_formula(
                    "PinX",
                    format!("=(Sheet.{resolved_target}!PinX + Sheet.{second_shape_id}!PinX) * 0.5"),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "PinY",
                    format!(
                        "=MAX(Sheet.{resolved_target}!PinY, Sheet.{second_shape_id}!PinY) + {} in",
                        format_clean_f64(clean_offset)
                    ),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "Width",
                    format!("=MAX(ABS(Sheet.{second_shape_id}!PinX - Sheet.{resolved_target}!PinX), 0.15 in)"),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula("Height", "0.36 in", Some(Unit::Inch));
                let meas_formula = format!(
                    "=ABS(Sheet.{second_shape_id}!PinX - Sheet.{resolved_target}!PinX) * {scale_expr}"
                );
                dim.sheet.set_shape_data(ShapeDataRow {
                    name: "MeasuredValue".into(),
                    label: "Measured Span".into(),
                    prompt: "Live Horizontal Span".into(),
                    data_type: ShapeDataType::Number,
                    format: fmt_spec.clone(),
                    value: meas_formula,
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            } else {
                dim.sheet.set_formula("PinX", format!("=Sheet.{resolved_target}!PinX"), Some(Unit::Inch));
                dim.sheet.set_formula(
                    "PinY",
                    format!(
                        "=Sheet.{resolved_target}!PinY + Sheet.{resolved_target}!Height * 0.5 + {} in",
                        format_clean_f64(clean_offset)
                    ),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "Width",
                    format!("=MAX(Sheet.{resolved_target}!Width, 0.15 in)"),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula("Height", "0.36 in", Some(Unit::Inch));
                let meas_formula = format!("=Sheet.{resolved_target}!Width * {scale_expr}");
                dim.sheet.set_shape_data(ShapeDataRow {
                    name: "MeasuredValue".into(),
                    label: "Measured Width".into(),
                    prompt: "Live Horizontal Dimension".into(),
                    data_type: ShapeDataType::Number,
                    format: fmt_spec.clone(),
                    value: meas_formula,
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }
            dim.sheet.set_formula("BeginArrow", "41", Some(Unit::Number));
            dim.sheet.set_formula("EndArrow", "41", Some(Unit::Number));
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows.push(GeometryRow::move_to(1, "0 in", "=Height*0.5"));
            g1.rows.push(GeometryRow::line_to(2, "=Width", "=Height*0.5"));
            dim.sheet.add_geometry(g1);

            let mut g2 = GeometrySection::new(2);
            g2.no_fill = "TRUE".into();
            g2.rows.push(GeometryRow::move_to(1, "0 in", "=-0.22 in"));
            g2.rows.push(GeometryRow::line_to(2, "0 in", "=Height*0.85"));
            g2.rows.push(GeometryRow::move_to(3, "=Width", "=-0.22 in"));
            g2.rows.push(GeometryRow::line_to(4, "=Width", "=Height*0.85"));
            dim.sheet.add_geometry(g2);

            attach_live_dimension_field(&mut dim, "", "=Prop.MeasuredValue", &fmt_spec);
        }
        AssociativeDimensionKind::Vertical => {
            dim.sheet.set_formula(
                "PinX",
                format!(
                    "=Sheet.{resolved_target}!PinX - Sheet.{resolved_target}!Width * 0.5 - {} in",
                    format_clean_f64(clean_offset.abs())
                ),
                Some(Unit::Inch),
            );
            dim.sheet.set_formula("PinY", format!("=Sheet.{resolved_target}!PinY"), Some(Unit::Inch));
            dim.sheet.set_formula("Width", "0.36 in", Some(Unit::Inch));
            dim.sheet.set_formula(
                "Height",
                format!("=MAX(Sheet.{resolved_target}!Height, 0.15 in)"),
                Some(Unit::Inch),
            );
            dim.sheet.set_formula("TxtAngle", "90 deg", Some(Unit::Degree));
            dim.sheet.set_formula("BeginArrow", "41", Some(Unit::Number));
            dim.sheet.set_formula("EndArrow", "41", Some(Unit::Number));
            let meas_formula = format!("=Sheet.{resolved_target}!Height * {scale_expr}");
            dim.sheet.set_shape_data(ShapeDataRow {
                name: "MeasuredValue".into(),
                label: "Measured Height".into(),
                prompt: "Live Vertical Dimension".into(),
                data_type: ShapeDataType::Number,
                format: fmt_spec.clone(),
                value: meas_formula,
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });

            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows.push(GeometryRow::move_to(1, "=Width*0.5", "0 in"));
            g1.rows.push(GeometryRow::line_to(2, "=Width*0.5", "=Height"));
            dim.sheet.add_geometry(g1);

            let mut g2 = GeometrySection::new(2);
            g2.no_fill = "TRUE".into();
            g2.rows.push(GeometryRow::move_to(1, "=Width*0.15", "0 in"));
            g2.rows.push(GeometryRow::line_to(2, "=Width+0.22 in", "0 in"));
            g2.rows.push(GeometryRow::move_to(3, "=Width*0.15", "=Height"));
            g2.rows.push(GeometryRow::line_to(4, "=Width+0.22 in", "=Height"));
            dim.sheet.add_geometry(g2);

            attach_live_dimension_field(&mut dim, "", "=Prop.MeasuredValue", &fmt_spec);
        }
        AssociativeDimensionKind::Aligned => {
            if second_shape_id > 0 && doc.page(page_id)?.find_shape(second_shape_id).is_some() {
                dim.sheet.set_formula(
                    "PinX",
                    format!("=(Sheet.{resolved_target}!PinX + Sheet.{second_shape_id}!PinX) * 0.5"),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "PinY",
                    format!(
                        "=(Sheet.{resolved_target}!PinY + Sheet.{second_shape_id}!PinY) * 0.5 + {} in",
                        format_clean_f64(clean_offset)
                    ),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "Width",
                    format!(
                        "=MAX(SQRT((Sheet.{second_shape_id}!PinX - Sheet.{resolved_target}!PinX)^2 + (Sheet.{second_shape_id}!PinY - Sheet.{resolved_target}!PinY)^2), 0.20 in)"
                    ),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "Angle",
                    format!(
                        "=ATAN2(Sheet.{second_shape_id}!PinY - Sheet.{resolved_target}!PinY, Sheet.{second_shape_id}!PinX - Sheet.{resolved_target}!PinX)"
                    ),
                    Some(Unit::Radian),
                );
                dim.sheet.set_formula("Height", "0.36 in", Some(Unit::Inch));
                let meas_formula = format!(
                    "=SQRT((Sheet.{second_shape_id}!PinX - Sheet.{resolved_target}!PinX)^2 + (Sheet.{second_shape_id}!PinY - Sheet.{resolved_target}!PinY)^2) * {scale_expr}"
                );
                dim.sheet.set_shape_data(ShapeDataRow {
                    name: "MeasuredValue".into(),
                    label: "Aligned Distance".into(),
                    prompt: "Live Aligned Center-to-Center Span".into(),
                    data_type: ShapeDataType::Number,
                    format: fmt_spec.clone(),
                    value: meas_formula,
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            } else {
                dim.sheet.set_formula(
                    "Angle",
                    format!("=Sheet.{resolved_target}!Angle"),
                    Some(Unit::Radian),
                );
                dim.sheet.set_formula(
                    "PinX",
                    format!(
                        "=Sheet.{resolved_target}!PinX - SIN(Sheet.{resolved_target}!Angle) * (Sheet.{resolved_target}!Height * 0.5 + {} in)",
                        format_clean_f64(clean_offset)
                    ),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "PinY",
                    format!(
                        "=Sheet.{resolved_target}!PinY + COS(Sheet.{resolved_target}!Angle) * (Sheet.{resolved_target}!Height * 0.5 + {} in)",
                        format_clean_f64(clean_offset)
                    ),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula(
                    "Width",
                    format!("=MAX(Sheet.{resolved_target}!Width, 0.20 in)"),
                    Some(Unit::Inch),
                );
                dim.sheet.set_formula("Height", "0.36 in", Some(Unit::Inch));
                let meas_formula = format!("=Sheet.{resolved_target}!Width * {scale_expr}");
                dim.sheet.set_shape_data(ShapeDataRow {
                    name: "MeasuredValue".into(),
                    label: "Aligned Length".into(),
                    prompt: "Live Aligned Length".into(),
                    data_type: ShapeDataType::Number,
                    format: fmt_spec.clone(),
                    value: meas_formula,
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }
            dim.sheet.set_formula("BeginArrow", "41", Some(Unit::Number));
            dim.sheet.set_formula("EndArrow", "41", Some(Unit::Number));
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows.push(GeometryRow::move_to(1, "0 in", "=Height*0.5"));
            g1.rows.push(GeometryRow::line_to(2, "=Width", "=Height*0.5"));
            dim.sheet.add_geometry(g1);

            let mut g2 = GeometrySection::new(2);
            g2.no_fill = "TRUE".into();
            g2.rows.push(GeometryRow::move_to(1, "0 in", "=-0.20 in"));
            g2.rows.push(GeometryRow::line_to(2, "0 in", "=Height*0.85"));
            g2.rows.push(GeometryRow::move_to(3, "=Width", "=-0.20 in"));
            g2.rows.push(GeometryRow::line_to(4, "=Width", "=Height*0.85"));
            dim.sheet.add_geometry(g2);

            attach_live_dimension_field(&mut dim, "", "=Prop.MeasuredValue", &fmt_spec);
        }
        AssociativeDimensionKind::Angular => {
            dim.sheet.set_formula(
                "PinX",
                format!("=Sheet.{resolved_target}!PinX + {} in", format_clean_f64(clean_offset)),
                Some(Unit::Inch),
            );
            dim.sheet.set_formula(
                "PinY",
                format!("=Sheet.{resolved_target}!PinY + {} in", format_clean_f64(clean_offset)),
                Some(Unit::Inch),
            );
            dim.sheet.set_formula("Width", "0.90 in", Some(Unit::Inch));
            dim.sheet.set_formula("Height", "0.90 in", Some(Unit::Inch));
            dim.sheet.set_formula("BeginArrow", "1", Some(Unit::Number));
            dim.sheet.set_formula("EndArrow", "1", Some(Unit::Number));
            let ang_formula = if second_shape_id > 0
                && doc.page(page_id)?.find_shape(second_shape_id).is_some()
            {
                format!(
                    "=MOD(ABS(Sheet.{second_shape_id}!Angle - Sheet.{resolved_target}!Angle), PI())"
                )
            } else {
                format!("=ABS(Sheet.{resolved_target}!Angle)")
            };
            dim.sheet.set_shape_data(ShapeDataRow {
                name: "MeasuredValue".into(),
                label: "Measured Angle (deg)".into(),
                prompt: "Live Angular Dimension".into(),
                data_type: ShapeDataType::Number,
                format: "0.0 deg".into(),
                value: ang_formula,
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows.push(GeometryRow::move_to(1, "0.10 in", "0.15 in"));
            g1.rows.push(GeometryRow::arc_to(2, "0.80 in", "0.80 in", "-0.22 in"));
            dim.sheet.add_geometry(g1);

            attach_live_dimension_field(&mut dim, "", "=Prop.MeasuredValue", "0.0 deg");
        }
        AssociativeDimensionKind::Radial | AssociativeDimensionKind::Diameter => {
            let is_diam = kind == AssociativeDimensionKind::Diameter;
            dim.sheet.set_formula("PinX", format!("=Sheet.{resolved_target}!PinX"), Some(Unit::Inch));
            dim.sheet.set_formula("PinY", format!("=Sheet.{resolved_target}!PinY"), Some(Unit::Inch));
            dim.sheet.set_formula(
                "Width",
                format!("=MAX(Sheet.{resolved_target}!Width, 0.30 in)"),
                Some(Unit::Inch),
            );
            dim.sheet.set_formula(
                "Height",
                format!("=MAX(Sheet.{resolved_target}!Height, 0.30 in)"),
                Some(Unit::Inch),
            );
            dim.sheet.set_formula("BeginArrow", if is_diam { "1" } else { "0" }, Some(Unit::Number));
            dim.sheet.set_formula("EndArrow", "1", Some(Unit::Number));
            let factor = if is_diam { 1.0 } else { 0.5 };
            let meas_formula = format!(
                "=Sheet.{resolved_target}!Width * {} * {scale_expr}",
                format_clean_f64(factor)
            );
            dim.sheet.set_shape_data(ShapeDataRow {
                name: "MeasuredValue".into(),
                label: if is_diam { "Diameter".into() } else { "Radius".into() },
                prompt: "Live Radial/Diameter Dimension".into(),
                data_type: ShapeDataType::Number,
                format: fmt_spec.clone(),
                value: meas_formula,
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            if is_diam {
                g1.rows.push(GeometryRow::move_to(1, "0 in", "=Height*0.5"));
                g1.rows.push(GeometryRow::line_to(2, "=Width", "=Height*0.5"));
            } else {
                g1.rows.push(GeometryRow::move_to(1, "=Width*0.5", "=Height*0.5"));
                g1.rows.push(GeometryRow::line_to(2, "=Width", "=Height*0.5"));
            }
            dim.sheet.add_geometry(g1);

            let prefix = if is_diam { "⌀ " } else { "R = " };
            attach_live_dimension_field(&mut dim, prefix, "=Prop.MeasuredValue", &fmt_spec);
        }
    }

    dim.sheet.sync_all_sections_to_cells();
    if let Some(page) = doc.page_mut(page_id) {
        page.shapes.push(dim);
    }
    ReactiveEngine::recalculate_document(doc);
    Some(dim_id)
}

// ============================================================================
// 2. Architectural Walls, Miter L-Join / T-Join Auto-Healing & Wall Openings
// ============================================================================

#[derive(Debug, Clone)]
struct WallRecord {
    shape_id: u64,
    p0: [f64; 2],
    p1: [f64; 2],
    thickness: f64,
    openings: Vec<[f64; 2]>, // [t_start_in, t_end_in] along wall length
}

fn parse_wall_openings(raw: &str) -> Vec<[f64; 2]> {
    let mut out = Vec::new();
    for part in raw.trim_matches('"').split(';') {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((a, b)) = trimmed.split_once(':') {
            if let (Ok(t0), Ok(t1)) = (a.trim().parse::<f64>(), b.trim().parse::<f64>()) {
                if t1 > t0 + 1e-4 {
                    out.push([t0.max(0.0), t1]);
                }
            }
        }
    }
    out.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    out
}

fn format_wall_openings(openings: &[[f64; 2]]) -> String {
    let body = openings
        .iter()
        .map(|[a, b]| format!("{:.4}:{:.4}", a, b))
        .collect::<Vec<_>>()
        .join(";");
    format!("\"{body}\"")
}

fn user_cell_str<'a>(shape: &'a VisioShape, key: &str) -> Option<&'a str> {
    shape
        .sheet
        .user_cells
        .get(&key.to_ascii_lowercase())
        .map(|u| u.value.trim_matches('"'))
}

fn read_wall_record(shape: &VisioShape) -> Option<WallRecord> {
    let role = user_cell_str(shape, "DomainRole")?.to_ascii_lowercase();
    if role != "arch_wall" {
        return None;
    }
    let thickness = shape
        .sheet
        .get_f64("User.WallThickness")
        .unwrap_or(0.25)
        .clamp(0.06, 2.5);

    // Derive centerline from live PinX, PinY, Width, Angle so interactive drags on the canvas are honored!
    let px = shape.sheet.get_f64("PinX").unwrap_or(0.0);
    let py = shape.sheet.get_f64("PinY").unwrap_or(0.0);
    let len = shape.sheet.get_f64("Width").unwrap_or(1.0).max(0.10);
    let ang_rad = shape.sheet.get_f64("Angle").unwrap_or(0.0);
    let dx = ang_rad.cos();
    let dy = ang_rad.sin();
    let p0 = [px - dx * len * 0.5, py - dy * len * 0.5];
    let p1 = [px + dx * len * 0.5, py + dy * len * 0.5];
    let openings_raw = user_cell_str(shape, "WallOpenings").unwrap_or("");
    Some(WallRecord {
        shape_id: shape.id,
        p0,
        p1,
        thickness,
        openings: parse_wall_openings(openings_raw),
    })
}

fn line_intersection_2d(p: [f64; 2], r: [f64; 2], q: [f64; 2], s: [f64; 2]) -> Option<[f64; 2]> {
    let cross = r[0] * s[1] - r[1] * s[0];
    if cross.abs() < 1e-6 {
        return None;
    }
    let qp = [q[0] - p[0], q[1] - p[1]];
    let t = (qp[0] * s[1] - qp[1] * s[0]) / cross;
    Some([p[0] + t * r[0], p[1] + t * r[1]])
}

fn world_to_wall_local(
    pt: [f64; 2],
    center: [f64; 2],
    dir: [f64; 2],
    normal: [f64; 2],
    len: f64,
    thickness: f64,
) -> [f64; 2] {
    let rel = [pt[0] - center[0], pt[1] - center[1]];
    let u = rel[0] * dir[0] + rel[1] * dir[1] + len * 0.5;
    let v = rel[0] * normal[0] + rel[1] * normal[1] + thickness * 0.5;
    [u, v]
}

fn rebuild_wall_geometry_with_corners_and_openings(
    shape: &mut VisioShape,
    rec: &WallRecord,
    start_left_world: [f64; 2],
    start_right_world: [f64; 2],
    end_left_world: [f64; 2],
    end_right_world: [f64; 2],
) {
    let vx = rec.p1[0] - rec.p0[0];
    let vy = rec.p1[1] - rec.p0[1];
    let len = vx.hypot(vy).max(0.10);
    let dir = [vx / len, vy / len];
    let normal = [-dir[1], dir[0]];
    let center = [(rec.p0[0] + rec.p1[0]) * 0.5, (rec.p0[1] + rec.p1[1]) * 0.5];
    let angle_deg = dir[1].atan2(dir[0]).to_degrees();

    shape
        .sheet
        .set_formula("PinX", format!("{} in", format_clean_f64(center[0])), Some(Unit::Inch));
    shape
        .sheet
        .set_formula("PinY", format!("{} in", format_clean_f64(center[1])), Some(Unit::Inch));
    shape
        .sheet
        .set_formula("Width", format!("{} in", format_clean_f64(len)), Some(Unit::Inch));
    shape.sheet.set_formula(
        "Height",
        format!("{} in", format_clean_f64(rec.thickness)),
        Some(Unit::Inch),
    );
    shape
        .sheet
        .set_formula("LocPinX", "=Width*0.5", Some(Unit::Inch));
    shape
        .sheet
        .set_formula("LocPinY", "=Height*0.5", Some(Unit::Inch));
    shape.sheet.set_formula(
        "Angle",
        format!("{} deg", format_clean_f64(angle_deg)),
        Some(Unit::Degree),
    );
    shape
        .sheet
        .set_user_cell("WallX1", format!("{} in", format_clean_f64(rec.p0[0])), "");
    shape
        .sheet
        .set_user_cell("WallY1", format!("{} in", format_clean_f64(rec.p0[1])), "");
    shape
        .sheet
        .set_user_cell("WallX2", format!("{} in", format_clean_f64(rec.p1[0])), "");
    shape
        .sheet
        .set_user_cell("WallY2", format!("{} in", format_clean_f64(rec.p1[1])), "");

    let sl = world_to_wall_local(start_left_world, center, dir, normal, len, rec.thickness);
    let sr = world_to_wall_local(start_right_world, center, dir, normal, len, rec.thickness);
    let el = world_to_wall_local(end_left_world, center, dir, normal, len, rec.thickness);
    let er = world_to_wall_local(end_right_world, center, dir, normal, len, rec.thickness);

    // Build solid intervals along [0..len] after subtracting all door/window openings
    let mut solid_spans: Vec<[f64; 2]> = vec![[0.0, len]];
    for &[op_a, op_b] in &rec.openings {
        let cut_a = op_a.clamp(0.02, len - 0.02);
        let cut_b = op_b.clamp(0.02, len - 0.02);
        if cut_b <= cut_a + 1e-3 {
            continue;
        }
        let mut next = Vec::new();
        for [sa, sb] in solid_spans {
            if cut_b <= sa || cut_a >= sb {
                next.push([sa, sb]);
            } else {
                if cut_a > sa + 1e-3 {
                    next.push([sa, cut_a]);
                }
                if cut_b + 1e-3 < sb {
                    next.push([cut_b, sb]);
                }
            }
        }
        solid_spans = next;
    }

    shape.sheet.geometries.clear();
    let h = rec.thickness;
    for (idx, [sa, sb]) in solid_spans.into_iter().enumerate() {
        let mut sec = GeometrySection::new((idx + 1) as u32);
        sec.no_fill = "FALSE".into();
        sec.no_line = "FALSE".into();

        let p_sr = if sa <= 1e-3 { sr } else { [sa, 0.0] };
        let p_er = if (sb - len).abs() <= 1e-3 { er } else { [sb, 0.0] };
        let p_el = if (sb - len).abs() <= 1e-3 { el } else { [sb, h] };
        let p_sl = if sa <= 1e-3 { sl } else { [sa, h] };

        let fmt_pt = |p: [f64; 2]| -> (String, String) {
            (
                format!("{} in", format_clean_f64(p[0])),
                format!("{} in", format_clean_f64(p[1])),
            )
        };
        let (x1, y1) = fmt_pt(p_sr);
        let (x2, y2) = fmt_pt(p_er);
        let (x3, y3) = fmt_pt(p_el);
        let (x4, y4) = fmt_pt(p_sl);
        sec.rows.push(GeometryRow::move_to(1, x1.clone(), y1.clone()));
        sec.rows.push(GeometryRow::line_to(2, x2, y2));
        sec.rows.push(GeometryRow::line_to(3, x3, y3));
        sec.rows.push(GeometryRow::line_to(4, x4, y4));
        sec.rows.push(GeometryRow::line_to(5, x1, y1));
        shape.sheet.add_geometry(sec);
    }
    shape.sheet.sync_all_sections_to_cells();
}

/// Draw a structural Architectural Wall between `(x1_in, y1_in)` and `(x2_in, y2_in)` and
/// automatically heal any Miter (`L-Corner`) and Butt (`T-Join`) wall intersections on the page!
pub fn draw_architectural_wall(
    doc: &mut VisioDocument,
    page_id: u32,
    x1_in: f64,
    y1_in: f64,
    x2_in: f64,
    y2_in: f64,
    thickness_in: f64,
) -> Option<u64> {
    let mut dx = x2_in - x1_in;
    let mut dy = y2_in - y1_in;
    if dx.hypot(dy) < 0.15 {
        dx = 2.50;
        dy = 0.0;
    }
    let clean_t = if thickness_in <= 0.02 {
        0.25
    } else {
        thickness_in.clamp(0.06, 2.5)
    };
    let wid = doc.page_mut(page_id)?.next_shape_id();
    let cx = x1_in + dx * 0.5;
    let cy = y1_in + dy * 0.5;
    let len = dx.hypot(dy);
    let mut wall = VisioShape::new_rect(wid, format!("Wall.{wid}"), cx, cy, len, clean_t);
    wall.text.clear();
    wall.sheet
        .set_user_cell("DomainRole", "\"arch_wall\"", "Architectural Structural Wall");
    wall.sheet.set_user_cell(
        "WallThickness",
        format!("{} in", format_clean_f64(clean_t)),
        "Wall Thickness",
    );
    wall.sheet
        .set_user_cell("WallOpenings", "\"\"", "Door/Window Openings");
    wall.sheet
        .set_formula("FillForegnd", "=RGB(148,163,184)", Some(Unit::Color));
    wall.sheet
        .set_formula("LineColor", "=RGB(30,41,59)", Some(Unit::Color));
    wall.sheet
        .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
    let ang_deg = dy.atan2(dx).to_degrees();
    wall.sheet.set_formula(
        "Angle",
        format!("{} deg", format_clean_f64(ang_deg)),
        Some(Unit::Degree),
    );
    wall.sheet.set_shape_data(ShapeDataRow {
        name: "WallLength".into(),
        label: "Length".into(),
        prompt: "Scaled Architectural Wall Length".into(),
        data_type: ShapeDataType::Number,
        format: "0.00 in".into(),
        value: "=Width * (ThePage!DrawingScale / ThePage!PageScale)".into(),
        sort_key: "1".into(),
        invisible: false,
        ask: false,
        lang_id: 1033,
    });
    wall.sheet.set_shape_data(ShapeDataRow {
        name: "WallThickness".into(),
        label: "Thickness".into(),
        prompt: "Wall Thickness".into(),
        data_type: ShapeDataType::Number,
        format: "0.00 in".into(),
        value: format!("{} in", format_clean_f64(clean_t)),
        sort_key: "2".into(),
        invisible: false,
        ask: false,
        lang_id: 1033,
    });

    let rec = WallRecord {
        shape_id: wid,
        p0: [x1_in, y1_in],
        p1: [x1_in + dx, y1_in + dy],
        thickness: clean_t,
        openings: Vec::new(),
    };
    let dir = [dx / len, dy / len];
    let normal = [-dir[1], dir[0]];
    let ht = clean_t * 0.5;
    rebuild_wall_geometry_with_corners_and_openings(
        &mut wall,
        &rec,
        [rec.p0[0] + normal[0] * ht, rec.p0[1] + normal[1] * ht],
        [rec.p0[0] - normal[0] * ht, rec.p0[1] - normal[1] * ht],
        [rec.p1[0] + normal[0] * ht, rec.p1[1] + normal[1] * ht],
        [rec.p1[0] - normal[0] * ht, rec.p1[1] - normal[1] * ht],
    );
    if let Some(page) = doc.page_mut(page_id) {
        page.shapes.push(wall);
    }
    let _ = heal_wall_intersections(doc, page_id, 0.30);
    Some(wid)
}

/// Auto-heal all L-Corner Miter joins and T-Join intersections among `arch_wall` shapes on `page_id`.
/// Returns the number of healed wall joints (`miter_count + t_join_count`).
pub fn heal_wall_intersections(
    doc: &mut VisioDocument,
    page_id: u32,
    tolerance_in: f64,
) -> usize {
    let tol = if tolerance_in <= 0.01 { 0.30 } else { tolerance_in };
    let mut walls: Vec<WallRecord> = match doc.page(page_id) {
        Some(p) => p.shapes.iter().filter_map(read_wall_record).collect(),
        None => return 0,
    };
    if walls.is_empty() {
        return 0;
    }

    // Step 1: Snap near-coincident wall endpoints so L-corners share an exact vertex
    let n = walls.len();
    for i in 0..n {
        for j in (i + 1)..n {
            for ei in 0..2 {
                for ej in 0..2 {
                    let pi = if ei == 0 { walls[i].p0 } else { walls[i].p1 };
                    let pj = if ej == 0 { walls[j].p0 } else { walls[j].p1 };
                    let dist = (pi[0] - pj[0]).hypot(pi[1] - pj[1]);
                    if dist <= tol {
                        let mid = [(pi[0] + pj[0]) * 0.5, (pi[1] + pj[1]) * 0.5];
                        if ei == 0 {
                            walls[i].p0 = mid;
                        } else {
                            walls[i].p1 = mid;
                        }
                        if ej == 0 {
                            walls[j].p0 = mid;
                        } else {
                            walls[j].p1 = mid;
                        }
                    }
                }
            }
        }
    }

    // Default rectangular world corners for each wall: [start_left, start_right, end_left, end_right]
    let mut corners: Vec<[[f64; 2]; 4]> = walls
        .iter()
        .map(|w| {
            let vx = w.p1[0] - w.p0[0];
            let vy = w.p1[1] - w.p0[1];
            let len = vx.hypot(vy).max(1e-4);
            let dir = [vx / len, vy / len];
            let normal = [-dir[1], dir[0]];
            let ht = w.thickness * 0.5;
            [
                [w.p0[0] + normal[0] * ht, w.p0[1] + normal[1] * ht],
                [w.p0[0] - normal[0] * ht, w.p0[1] - normal[1] * ht],
                [w.p1[0] + normal[0] * ht, w.p1[1] + normal[1] * ht],
                [w.p1[0] - normal[0] * ht, w.p1[1] - normal[1] * ht],
            ]
        })
        .collect();

    let mut healed_joints = 0usize;

    // Step 2: L-Corner Miter Joins
    for i in 0..n {
        for j in (i + 1)..n {
            for ei in 0..2 {
                for ej in 0..2 {
                    let pi = if ei == 0 { walls[i].p0 } else { walls[i].p1 };
                    let pj = if ej == 0 { walls[j].p0 } else { walls[j].p1 };
                    if (pi[0] - pj[0]).hypot(pi[1] - pj[1]) > 1e-3 {
                        continue;
                    }
                    let joint = pi;
                    let other_i = if ei == 0 { walls[i].p1 } else { walls[i].p0 };
                    let other_j = if ej == 0 { walls[j].p1 } else { walls[j].p0 };
                    // Outward rays AWAY from the shared joint along each wall
                    let vi = [other_i[0] - joint[0], other_i[1] - joint[1]];
                    let vj = [other_j[0] - joint[0], other_j[1] - joint[1]];
                    let li = vi[0].hypot(vi[1]).max(1e-4);
                    let lj = vj[0].hypot(vj[1]).max(1e-4);
                    let ui = [vi[0] / li, vi[1] / li];
                    let uj = [vj[0] / lj, vj[1] / lj];
                    let cross = ui[0] * uj[1] - ui[1] * uj[0];
                    if cross.abs() < 0.15 {
                        continue; // Collinear or anti-parallel walls need no miter
                    }

                    // Canonical wall directions (p0 -> p1) & normals for walls i and j
                    let di_raw = [walls[i].p1[0] - walls[i].p0[0], walls[i].p1[1] - walls[i].p0[1]];
                    let di_len = di_raw[0].hypot(di_raw[1]).max(1e-4);
                    let di = [di_raw[0] / di_len, di_raw[1] / di_len];
                    let ni = [-di[1], di[0]];
                    let hti = walls[i].thickness * 0.5;

                    let dj_raw = [walls[j].p1[0] - walls[j].p0[0], walls[j].p1[1] - walls[j].p0[1]];
                    let dj_len = dj_raw[0].hypot(dj_raw[1]).max(1e-4);
                    let dj = [dj_raw[0] / dj_len, dj_raw[1] / dj_len];
                    let nj = [-dj[1], dj[0]];
                    let htj = walls[j].thickness * 0.5;

                    // Relative to outward ray `ui`, wall i's `+ni` side corresponds to left-of-ui when ei==0, right-of-ui when ei==1.
                    // Pairing the side of wall i that faces wall j (inner corner) with the side of wall j that faces wall i,
                    // and outer side with outer side:
                    let ni_out = [-ui[1], ui[0]]; // left of ui
                    let nj_out = [uj[1], -uj[0]]; // right of uj (faces left of ui when cross > 0!)
                    let sign = if cross > 0.0 { 1.0 } else { -1.0 };

                    let p_i_inner = [joint[0] + ni_out[0] * sign * hti, joint[1] + ni_out[1] * sign * hti];
                    let p_j_inner = [joint[0] + nj_out[0] * sign * htj, joint[1] + nj_out[1] * sign * htj];
                    let p_i_outer = [joint[0] - ni_out[0] * sign * hti, joint[1] - ni_out[1] * sign * hti];
                    let p_j_outer = [joint[0] - nj_out[0] * sign * htj, joint[1] - nj_out[1] * sign * htj];

                    let max_miter = 2.5 * walls[i].thickness.max(walls[j].thickness);
                    let clamp_miter = |pt: [f64; 2]| -> [f64; 2] {
                        let d = (pt[0] - joint[0]).hypot(pt[1] - joint[1]);
                        if d > max_miter && d > 1e-6 {
                            [
                                joint[0] + (pt[0] - joint[0]) * (max_miter / d),
                                joint[1] + (pt[1] - joint[1]) * (max_miter / d),
                            ]
                        } else {
                            pt
                        }
                    };

                    if let (Some(m_in), Some(m_out)) = (
                        line_intersection_2d(p_i_inner, ui, p_j_inner, uj),
                        line_intersection_2d(p_i_outer, ui, p_j_outer, uj),
                    ) {
                        let m_in = clamp_miter(m_in);
                        let m_out = clamp_miter(m_out);

                        // Assign back to wall i's (+ni = left, -ni = right) at endpoint ei
                        let dot_i_pos = ni[0] * (ni_out[0] * sign) + ni[1] * (ni_out[1] * sign);
                        let (i_left, i_right) = if dot_i_pos >= 0.0 {
                            (m_in, m_out)
                        } else {
                            (m_out, m_in)
                        };
                        if ei == 0 {
                            corners[i][0] = i_left;
                            corners[i][1] = i_right;
                        } else {
                            corners[i][2] = i_left;
                            corners[i][3] = i_right;
                        }

                        // Assign back to wall j's (+nj = left, -nj = right) at endpoint ej
                        let dot_j_pos = nj[0] * (nj_out[0] * sign) + nj[1] * (nj_out[1] * sign);
                        let (j_left, j_right) = if dot_j_pos >= 0.0 {
                            (m_in, m_out)
                        } else {
                            (m_out, m_in)
                        };
                        if ej == 0 {
                            corners[j][0] = j_left;
                            corners[j][1] = j_right;
                        } else {
                            corners[j][2] = j_left;
                            corners[j][3] = j_right;
                        }

                        healed_joints += 1;
                    }
                }
            }
        }
    }

    // Step 3: T-Join Butt Trimming (endpoint of wall i abutting interior span of wall j)
    for i in 0..n {
        let di_raw = [walls[i].p1[0] - walls[i].p0[0], walls[i].p1[1] - walls[i].p0[1]];
        let li = di_raw[0].hypot(di_raw[1]).max(1e-4);
        let di = [di_raw[0] / li, di_raw[1] / li];
        let ni = [-di[1], di[0]];
        let hti = walls[i].thickness * 0.5;

        for ei in 0..2 {
            let end_pt = if ei == 0 { walls[i].p0 } else { walls[i].p1 };
            let interior_pt = if ei == 0 { walls[i].p1 } else { walls[i].p0 };

            for j in 0..n {
                if i == j {
                    continue;
                }
                let dj_raw = [walls[j].p1[0] - walls[j].p0[0], walls[j].p1[1] - walls[j].p0[1]];
                let lj = dj_raw[0].hypot(dj_raw[1]).max(1e-4);
                let dj = [dj_raw[0] / lj, dj_raw[1] / lj];
                let nj = [-dj[1], dj[0]];
                let htj = walls[j].thickness * 0.5;

                if (di[0] * dj[1] - di[1] * dj[0]).abs() < 0.20 {
                    continue;
                }
                let rel = [end_pt[0] - walls[j].p0[0], end_pt[1] - walls[j].p0[1]];
                let u_along = rel[0] * dj[0] + rel[1] * dj[1];
                let v_perp = (rel[0] * nj[0] + rel[1] * nj[1]).abs();

                if u_along > walls[j].thickness * 0.5
                    && u_along < lj - walls[j].thickness * 0.5
                    && v_perp <= htj + tol
                {
                    // Which face of wall j does wall i approach from?
                    let rel_int = [interior_pt[0] - walls[j].p0[0], interior_pt[1] - walls[j].p0[1]];
                    let side_sign = if rel_int[0] * nj[0] + rel_int[1] * nj[1] >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    let face_origin = [
                        walls[j].p0[0] + nj[0] * side_sign * htj,
                        walls[j].p0[1] + nj[1] * side_sign * htj,
                    ];
                    let i_left_origin = [walls[i].p0[0] + ni[0] * hti, walls[i].p0[1] + ni[1] * hti];
                    let i_right_origin = [walls[i].p0[0] - ni[0] * hti, walls[i].p0[1] - ni[1] * hti];

                    if let (Some(trim_l), Some(trim_r)) = (
                        line_intersection_2d(i_left_origin, di, face_origin, dj),
                        line_intersection_2d(i_right_origin, di, face_origin, dj),
                    ) {
                        if ei == 0 {
                            corners[i][0] = trim_l;
                            corners[i][1] = trim_r;
                        } else {
                            corners[i][2] = trim_l;
                            corners[i][3] = trim_r;
                        }
                        healed_joints += 1;
                    }
                }
            }
        }
    }

    // Apply updated wall polygons to page shapes
    if let Some(page) = doc.page_mut(page_id) {
        for (idx, wrec) in walls.iter().enumerate() {
            if let Some(sh) = page.find_shape_mut(wrec.shape_id) {
                let c = corners[idx];
                rebuild_wall_geometry_with_corners_and_openings(sh, wrec, c[0], c[1], c[2], c[3]);
            }
        }
    }
    ReactiveEngine::recalculate_document(doc);
    healed_joints
}

/// Insert a parametric Door or Window opening (`single_door`, `double_door`, `sliding_door`,
/// `pocket_door`, `window`, `cased_opening`) into a host `arch_wall`, splitting the wall's solid
/// polygon cleanly around the opening and placing an aligned architectural symbol!
pub fn insert_wall_opening(
    doc: &mut VisioDocument,
    page_id: u32,
    wall_shape_id: u64,
    opening_kind_slug: &str,
    offset_along_in: f64,
    opening_width_in: f64,
    swing_dir_slug: &str,
) -> Option<u64> {
    let resolved_wall_id = if wall_shape_id > 0
        && doc
            .page(page_id)?
            .find_shape(wall_shape_id)
            .and_then(read_wall_record)
            .is_some()
    {
        wall_shape_id
    } else {
        doc.page(page_id)?
            .shapes
            .iter()
            .find_map(|s| read_wall_record(s).map(|r| r.shape_id))?
    };

    let mut rec = doc
        .page(page_id)?
        .find_shape(resolved_wall_id)
        .and_then(read_wall_record)?;

    let vx = rec.p1[0] - rec.p0[0];
    let vy = rec.p1[1] - rec.p0[1];
    let wall_len = vx.hypot(vy).max(0.40);
    let dir = [vx / wall_len, vy / wall_len];
    let normal = [-dir[1], dir[0]];
    let wall_ang_deg = dir[1].atan2(dir[0]).to_degrees();

    let clean_w = if opening_width_in <= 0.05 {
        0.75_f64.min(wall_len * 0.65)
    } else {
        opening_width_in.clamp(0.20, (wall_len - 0.16).max(0.20))
    };
    let center_t = if offset_along_in <= 0.0 {
        wall_len * 0.5
    } else {
        offset_along_in.clamp(clean_w * 0.5 + 0.08, wall_len - clean_w * 0.5 - 0.08)
    };
    let t0 = (center_t - clean_w * 0.5).max(0.05);
    let t1 = (center_t + clean_w * 0.5).min(wall_len - 0.05);
    rec.openings.push([t0, t1]);
    rec.openings
        .sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));

    if let Some(wall_sh) = doc.page_mut(page_id)?.find_shape_mut(resolved_wall_id) {
        wall_sh.sheet.set_user_cell(
            "WallOpenings",
            format_wall_openings(&rec.openings),
            "Door/Window Openings",
        );
    }
    // Rebuild walls + preserve healed corners
    let _ = heal_wall_intersections(doc, page_id, 0.30);

    // Place parametric Door / Window shape centered at `center_t` along the host wall
    let kind_norm = opening_kind_slug.trim().to_ascii_lowercase();
    let swing_out = swing_dir_slug.to_ascii_lowercase().contains("out");
    let hinge_right = swing_dir_slug.to_ascii_lowercase().contains("right");
    let swing_sign = if swing_out { -1.0 } else { 1.0 };

    let op_id = doc.page_mut(page_id)?.next_shape_id();
    let cx = rec.p0[0] + dir[0] * center_t;
    let cy = rec.p0[1] + dir[1] * center_t;
    let is_window = kind_norm.contains("window");
    let is_cased = kind_norm.contains("cased");
    let is_sliding = kind_norm.contains("sliding") || kind_norm.contains("pocket");
    let is_double = kind_norm.contains("double");

    let box_h = if is_window || is_cased || is_sliding {
        rec.thickness.max(0.18)
    } else {
        clean_w.max(rec.thickness)
    };
    let shift_n = if is_window || is_cased || is_sliding {
        0.0
    } else {
        swing_sign * (box_h * 0.5 - rec.thickness * 0.5)
    };
    let pin_x = cx + normal[0] * shift_n;
    let pin_y = cy + normal[1] * shift_n;

    let label_name = if is_window {
        format!("Window.{op_id}")
    } else {
        format!("Door.{op_id}")
    };
    let mut op_shape = VisioShape::new_rect(op_id, label_name, pin_x, pin_y, clean_w, box_h);
    op_shape.text.clear();
    op_shape.sheet.geometries.clear();
    op_shape.sheet.set_formula(
        "Angle",
        format!("{} deg", format_clean_f64(wall_ang_deg)),
        Some(Unit::Degree),
    );
    op_shape
        .sheet
        .set_formula("FillPattern", "0", Some(Unit::Number));
    op_shape
        .sheet
        .set_formula("LineColor", "=RGB(15,23,42)", Some(Unit::Color));
    op_shape
        .sheet
        .set_formula("LineWeight", "1.0 pt", Some(Unit::Point));
    op_shape
        .sheet
        .set_user_cell("DomainRole", "\"wall_opening\"", "Architectural Wall Opening");
    op_shape
        .sheet
        .set_user_cell("HostWallId", resolved_wall_id.to_string(), "Host Wall Shape ID");
    op_shape
        .sheet
        .set_user_cell("OpeningKind", format!("\"{kind_norm}\""), "Opening Kind");

    if is_window {
        let mut g1 = GeometrySection::new(1);
        g1.no_fill = "FALSE".into();
        op_shape
            .sheet
            .set_formula("FillPattern", "1", Some(Unit::Number));
        op_shape
            .sheet
            .set_formula("FillForegnd", "=RGB(224,242,254)", Some(Unit::Color));
        // Outer sill frame + dual interior glazing lines
        g1.rows.push(GeometryRow::move_to(1, "0 in", "0 in"));
        g1.rows.push(GeometryRow::line_to(2, "=Width", "0 in"));
        g1.rows.push(GeometryRow::line_to(3, "=Width", "=Height"));
        g1.rows.push(GeometryRow::line_to(4, "0 in", "=Height"));
        g1.rows.push(GeometryRow::line_to(5, "0 in", "0 in"));
        op_shape.sheet.add_geometry(g1);

        let mut g2 = GeometrySection::new(2);
        g2.no_fill = "TRUE".into();
        g2.rows.push(GeometryRow::move_to(1, "0 in", "=Height*0.38"));
        g2.rows.push(GeometryRow::line_to(2, "=Width", "=Height*0.38"));
        g2.rows.push(GeometryRow::move_to(3, "0 in", "=Height*0.62"));
        g2.rows.push(GeometryRow::line_to(4, "=Width", "=Height*0.62"));
        op_shape.sheet.add_geometry(g2);
    } else if is_sliding {
        let mut g1 = GeometrySection::new(1);
        g1.no_fill = "TRUE".into();
        g1.rows.push(GeometryRow::move_to(1, "0 in", "=Height*0.35"));
        g1.rows
            .push(GeometryRow::line_to(2, "=Width*0.58", "=Height*0.35"));
        g1.rows
            .push(GeometryRow::move_to(3, "=Width*0.42", "=Height*0.65"));
        g1.rows
            .push(GeometryRow::line_to(4, "=Width", "=Height*0.65"));
        op_shape.sheet.add_geometry(g1);
    } else if is_cased {
        let mut g1 = GeometrySection::new(1);
        g1.no_fill = "TRUE".into();
        g1.rows.push(GeometryRow::move_to(1, "0 in", "0 in"));
        g1.rows.push(GeometryRow::line_to(2, "0 in", "=Height"));
        g1.rows.push(GeometryRow::move_to(3, "=Width", "0 in"));
        g1.rows.push(GeometryRow::line_to(4, "=Width", "=Height"));
        op_shape.sheet.add_geometry(g1);
    } else if is_double {
        let base_y = if swing_out { "=Height" } else { "0 in" };
        let tip_y = if swing_out { "0 in" } else { "=Height" };
        let bow = format!(
            "{} in",
            format_clean_f64(clean_w * 0.5 * (std::f64::consts::SQRT_2 - 1.0) * swing_sign)
        );
        let mut g1 = GeometrySection::new(1);
        g1.no_fill = "TRUE".into();
        // Left leaf + swing arc to center
        g1.rows.push(GeometryRow::move_to(1, "0 in", base_y));
        g1.rows.push(GeometryRow::line_to(2, "0 in", tip_y));
        g1.rows
            .push(GeometryRow::arc_to(3, "=Width*0.5", base_y, bow.clone()));
        // Right leaf + swing arc from center
        g1.rows.push(GeometryRow::move_to(4, "=Width", base_y));
        g1.rows.push(GeometryRow::line_to(5, "=Width", tip_y));
        g1.rows.push(GeometryRow::arc_to(
            6,
            "=Width*0.5",
            base_y,
            format!(
                "{} in",
                format_clean_f64(-clean_w * 0.5 * (std::f64::consts::SQRT_2 - 1.0) * swing_sign)
            ),
        ));
        op_shape.sheet.add_geometry(g1);
    } else {
        // Single hinged architectural door leaf + 90-degree quarter-circle swing arc
        let hinge_x = if hinge_right { "=Width" } else { "0 in" };
        let latch_x = if hinge_right { "0 in" } else { "=Width" };
        let base_y = if swing_out { "=Height" } else { "0 in" };
        let tip_y = if swing_out { "0 in" } else { "=Height" };
        let arc_sign = swing_sign * if hinge_right { -1.0 } else { 1.0 };
        let bow = format!(
            "{} in",
            format_clean_f64(clean_w * (std::f64::consts::SQRT_2 - 1.0) * arc_sign)
        );
        let mut g1 = GeometrySection::new(1);
        g1.no_fill = "TRUE".into();
        g1.rows.push(GeometryRow::move_to(1, hinge_x, base_y));
        g1.rows.push(GeometryRow::line_to(2, hinge_x, tip_y));
        g1.rows.push(GeometryRow::arc_to(3, latch_x, base_y, bow));
        op_shape.sheet.add_geometry(g1);
    }

    op_shape.sheet.controls.push(ControlRow {
        name: "Row_1".into(),
        x: "=Width*0.5".into(),
        y: "=Height*0.5".into(),
        x_dyn: "=Width*0.5".into(),
        y_dyn: "=Height*0.5".into(),
        x_con: "0".into(),
        y_con: "0".into(),
        can_glue: false,
        prompt: "Adjust Opening Swing / Aperture".into(),
    });
    op_shape.sheet.set_shape_data(ShapeDataRow {
        name: "OpeningType".into(),
        label: "Opening Type".into(),
        prompt: "Architectural Door/Window Type".into(),
        data_type: ShapeDataType::String,
        format: String::new(),
        value: format!("\"{kind_norm}\""),
        sort_key: "1".into(),
        invisible: false,
        ask: false,
        lang_id: 1033,
    });
    op_shape.sheet.set_shape_data(ShapeDataRow {
        name: "ClearWidth".into(),
        label: "Clear Width".into(),
        prompt: "Scaled Clear Opening Width".into(),
        data_type: ShapeDataType::Number,
        format: "0.00 in".into(),
        value: "=Width * (ThePage!DrawingScale / ThePage!PageScale)".into(),
        sort_key: "2".into(),
        invisible: false,
        ask: false,
        lang_id: 1033,
    });
    op_shape.sheet.sync_all_sections_to_cells();

    if let Some(page) = doc.page_mut(page_id) {
        page.shapes.push(op_shape);
    }
    ReactiveEngine::recalculate_document(doc);
    Some(op_id)
}

// ============================================================================
// 3. Proleptic Gregorian Calendar Wizard (`build_calendar_diagram`)
// ============================================================================

fn is_gregorian_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

pub fn days_in_gregorian_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_gregorian_leap_year(year) => 29,
        2 => 28,
        _ => 30,
    }
}

/// Sakamoto's exact proleptic Gregorian weekday (`0 = Sunday .. 6 = Saturday`).
pub fn weekday_sunday0(mut year: i32, month: u32, day: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let m = month.clamp(1, 12) as usize;
    if m < 3 {
        year -= 1;
    }
    let w = (year + year / 4 - year / 100 + year / 400 + T[m - 1] + day as i32)
        .rem_euclid(7);
    w as u32
}

fn month_display_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "Month",
    }
}

/// Parse a Calendar Wizard spec:
/// Line 1 (optional): `2026,11,monday` (Year, Month `1..=12`, `monday`|`sunday`)
/// Subsequent lines: `2026-11-15|Sprint Review|blue` or `15|Sprint Review`
pub fn build_calendar_diagram(doc: &mut VisioDocument, page_id: u32, spec_text: &str) -> usize {
    let mut year = 2026i32;
    let mut month = 11u32;
    let mut start_monday = true;
    let mut events_by_day: BTreeMap<u32, Vec<String>> = BTreeMap::new();

    for (line_idx, raw_line) in spec_text.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line_idx == 0 && !line.contains('|') {
            let parts: Vec<&str> = line.split(',').map(str::trim).collect();
            if let Some(y) = parts.first().and_then(|s| s.parse::<i32>().ok()) {
                year = y.clamp(1900, 2200);
            }
            if let Some(m) = parts.get(1).and_then(|s| s.parse::<u32>().ok()) {
                month = m.clamp(1, 12);
            }
            if let Some(dow) = parts.get(2) {
                let dlow = dow.to_ascii_lowercase();
                start_monday = !(dlow.starts_with("sun") || dlow == "false" || dlow == "0");
            }
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        if cols.len() >= 2 {
            let date_part = cols[0];
            let day_opt = if let Some(last) = date_part.rsplit('-').next() {
                last.parse::<u32>().ok()
            } else {
                date_part.parse::<u32>().ok()
            };
            if let Some(d) = day_opt {
                if (1..=31).contains(&d) {
                    events_by_day
                        .entry(d)
                        .or_default()
                        .push(cols[1].to_string());
                }
            }
        }
    }

    let total_days = days_in_gregorian_month(year, month);
    let first_sun0 = weekday_sunday0(year, month, 1);
    let leading_offset = if start_monday {
        (first_sun0 + 6) % 7
    } else {
        first_sun0
    };
    let week_rows = (leading_offset + total_days).div_ceil(7).max(5);

    let (pw, ph) = doc
        .page(page_id)
        .map(|p| {
            (
                p.page_sheet.get_f64("PageWidth").unwrap_or(11.0),
                p.page_sheet.get_f64("PageHeight").unwrap_or(8.5),
            )
        })
        .unwrap_or((11.0, 8.5));

    let margin_x = 0.65;
    let grid_w = (pw - margin_x * 2.0).max(6.3);
    let col_w = grid_w / 7.0;
    let top_y = ph - 0.65;
    let banner_h = 0.52;
    let dow_h = 0.32;
    let grid_h = (ph - 1.95).max(4.0);
    let row_h = grid_h / (week_rows as f64);

    let mut created = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        // 1. Month Title Banner
        let bid = page.next_shape_id();
        let mut banner = VisioShape::new_rect(
            bid,
            "CalendarHeader",
            margin_x + grid_w * 0.5,
            top_y - banner_h * 0.5,
            grid_w,
            banner_h,
        );
        banner.text = format!("{} {}", month_display_name(month), year);
        banner
            .sheet
            .set_formula("FillForegnd", "=RGB(30,58,138)", Some(Unit::Color));
        banner
            .sheet
            .set_formula("LineColor", "=RGB(30,58,138)", Some(Unit::Color));
        banner
            .sheet
            .set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        banner
            .sheet
            .set_formula("Char.Size", "15 pt", Some(Unit::Point));
        banner
            .sheet
            .set_formula("Char.Style", "1", Some(Unit::Number));
        page.shapes.push(banner);
        created += 1;

        // 2. Weekday Headers
        let headers = if start_monday {
            ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
        } else {
            ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
        };
        let dow_cy = top_y - banner_h - dow_h * 0.5;
        for (col, &label) in headers.iter().enumerate() {
            let hid = page.next_shape_id();
            let cx = margin_x + (col as f64 + 0.5) * col_w;
            let mut hcell = VisioShape::new_rect(hid, format!("Weekday.{label}"), cx, dow_cy, col_w, dow_h);
            hcell.text = label.to_string();
            hcell
                .sheet
                .set_formula("FillForegnd", "=RGB(226,232,240)", Some(Unit::Color));
            hcell
                .sheet
                .set_formula("LineColor", "=RGB(148,163,184)", Some(Unit::Color));
            hcell
                .sheet
                .set_formula("Char.Style", "1", Some(Unit::Number));
            hcell
                .sheet
                .set_formula("Char.Size", "9.5 pt", Some(Unit::Point));
            page.shapes.push(hcell);
            created += 1;
        }

        // 3. Day Cells + Event Pills
        let grid_top = top_y - banner_h - dow_h;
        for day in 1..=total_days {
            let slot = leading_offset + (day - 1);
            let row = slot / 7;
            let col = slot % 7;
            let cx = margin_x + (col as f64 + 0.5) * col_w;
            let cy = grid_top - (row as f64 + 0.5) * row_h;
            let sun0 = weekday_sunday0(year, month, day);
            let is_weekend = sun0 == 0 || sun0 == 6;

            let did = page.next_shape_id();
            let iso_date = format!("{year:04}-{month:02}-{day:02}");
            let mut dcell = VisioShape::new_rect(did, format!("Day.{iso_date}"), cx, cy, col_w, row_h);
            let day_events = events_by_day.get(&day).cloned().unwrap_or_default();
            dcell.text = if day_events.is_empty() {
                format!("{day}")
            } else {
                format!("{day}\n• {}", day_events.join("\n• "))
            };
            dcell.sheet.set_formula(
                "FillForegnd",
                if !day_events.is_empty() {
                    "=RGB(239,246,255)"
                } else if is_weekend {
                    "=RGB(248,250,252)"
                } else {
                    "=RGB(255,255,255)"
                },
                Some(Unit::Color),
            );
            dcell
                .sheet
                .set_formula("LineColor", "=RGB(203,213,225)", Some(Unit::Color));
            dcell
                .sheet
                .set_formula("VerticalAlign", "0", Some(Unit::Number));
            dcell
                .sheet
                .set_formula("Para.HorzAlign", "0", Some(Unit::Number));
            dcell
                .sheet
                .set_formula("Char.Size", "8.5 pt", Some(Unit::Point));
            dcell.sheet.set_shape_data(ShapeDataRow {
                name: "Date".into(),
                label: "ISO Date".into(),
                prompt: "Calendar Date".into(),
                data_type: ShapeDataType::String,
                format: String::new(),
                value: format!("\"{iso_date}\""),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            dcell.sheet.set_shape_data(ShapeDataRow {
                name: "EventCount".into(),
                label: "Events".into(),
                prompt: "Scheduled Events Count".into(),
                data_type: ShapeDataType::Number,
                format: "0".into(),
                value: day_events.len().to_string(),
                sort_key: "2".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            page.shapes.push(dcell);
            created += 1;
        }
    }
    ReactiveEngine::recalculate_document(doc);
    created
}

// ============================================================================
// 4. PivotDiagram Rollup Builder (`build_pivot_diagram`)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotAggOp {
    Sum,
    Avg,
    Count,
    Min,
    Max,
}

impl PivotAggOp {
    pub fn from_slug(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "avg" | "average" | "mean" => Self::Avg,
            "count" | "cnt" => Self::Count,
            "min" => Self::Min,
            "max" => Self::Max,
            _ => Self::Sum,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Sum => "SUM",
            Self::Avg => "AVG",
            Self::Count => "COUNT",
            Self::Min => "MIN",
            Self::Max => "MAX",
        }
    }

    pub fn compute(self, vals: &[f64]) -> f64 {
        if vals.is_empty() {
            return 0.0;
        }
        match self {
            Self::Sum => vals.iter().sum(),
            Self::Avg => vals.iter().sum::<f64>() / (vals.len() as f64),
            Self::Count => vals.len() as f64,
            Self::Min => vals.iter().copied().fold(f64::INFINITY, f64::min),
            Self::Max => vals.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        }
    }
}

#[derive(Debug, Clone)]
struct PivotTreeNode {
    label: String,
    depth: usize,
    values: Vec<f64>,
    children: BTreeMap<String, PivotTreeNode>,
    leaf_weight: usize,
}

impl PivotTreeNode {
    fn new(label: String, depth: usize) -> Self {
        Self {
            label,
            depth,
            values: Vec::new(),
            children: BTreeMap::new(),
            leaf_weight: 1,
        }
    }

    fn compute_weights(&mut self) -> usize {
        if self.children.is_empty() {
            self.leaf_weight = 1;
        } else {
            self.leaf_weight = self
                .children
                .values_mut()
                .map(Self::compute_weights)
                .sum::<usize>()
                .max(1);
        }
        self.leaf_weight
    }
}

/// Build a hierarchical Visio PivotDiagram from CSV text.
/// Optional first directive line: `# GROUP=Region,Category;METRIC=Revenue;AGG=SUM`
/// Otherwise automatically uses all non-numeric leading columns as hierarchy dimensions and the
/// last numeric column as the rolled-up metric!
pub fn build_pivot_diagram(doc: &mut VisioDocument, page_id: u32, spec_text: &str) -> usize {
    let mut group_cols_cfg: Vec<String> = Vec::new();
    let mut metric_col_cfg: Option<String> = None;
    let mut agg_op = PivotAggOp::Sum;
    let mut csv_lines: Vec<&str> = Vec::new();

    for raw in spec_text.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(dir) = trimmed.strip_prefix('#') {
            for kv in dir.split(';') {
                if let Some((k, v)) = kv.split_once('=') {
                    match k.trim().to_ascii_uppercase().as_str() {
                        "GROUP" | "GROUPS" | "DIMS" => {
                            group_cols_cfg = v
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                        }
                        "METRIC" | "VALUE" => {
                            metric_col_cfg = Some(v.trim().to_string());
                        }
                        "AGG" | "OP" => {
                            agg_op = PivotAggOp::from_slug(v);
                        }
                        _ => {}
                    }
                }
            }
        } else {
            csv_lines.push(trimmed);
        }
    }

    if csv_lines.len() < 2 {
        return 0;
    }
    let headers: Vec<String> = csv_lines[0]
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .collect();
    if headers.is_empty() {
        return 0;
    }
    let rows: Vec<Vec<String>> = csv_lines[1..]
        .iter()
        .map(|line| {
            line.split(',')
                .map(|c| c.trim().trim_matches('"').to_string())
                .collect::<Vec<_>>()
        })
        .filter(|r| r.len() >= headers.len().min(2))
        .collect();
    if rows.is_empty() {
        return 0;
    }

    // Resolve metric column index (explicit or rightmost numeric column)
    let metric_idx = metric_col_cfg
        .as_ref()
        .and_then(|mc| headers.iter().position(|h| h.eq_ignore_ascii_case(mc)))
        .or_else(|| {
            (0..headers.len()).rev().find(|&ci| {
                rows.iter().any(|r| {
                    r.get(ci)
                        .and_then(|v| v.replace(['$', ',', ' '], "").parse::<f64>().ok())
                        .is_some()
                })
            })
        })
        .unwrap_or(headers.len() - 1);

    let group_indices: Vec<usize> = if !group_cols_cfg.is_empty() {
        group_cols_cfg
            .iter()
            .filter_map(|gc| headers.iter().position(|h| h.eq_ignore_ascii_case(gc)))
            .collect()
    } else {
        (0..headers.len())
            .filter(|&i| i != metric_idx)
            .take(2)
            .collect()
    };

    let metric_name = headers
        .get(metric_idx)
        .cloned()
        .unwrap_or_else(|| "Value".into());
    let mut root = PivotTreeNode::new(format!("Total ({metric_name})"), 0);
    for r in &rows {
        let val = r
            .get(metric_idx)
            .and_then(|s| s.replace(['$', ',', ' '], "").parse::<f64>().ok())
            .unwrap_or(1.0);
        root.values.push(val);
        let mut curr = &mut root;
        for (d_offset, &gi) in group_indices.iter().enumerate() {
            let key = r
                .get(gi)
                .cloned()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "(Unassigned)".into());
            curr = curr
                .children
                .entry(key.clone())
                .or_insert_with(|| PivotTreeNode::new(key, d_offset + 1));
            curr.values.push(val);
        }
    }
    root.compute_weights();
    let root_total = PivotAggOp::Sum.compute(&root.values).abs().max(1e-9);

    let mut created_nodes = 0usize;
    let level_fills = [
        "=RGB(30,58,138)",
        "=RGB(219,234,254)",
        "=RGB(220,252,231)",
        "=RGB(254,249,195)",
    ];
    let total_span_y = (root.leaf_weight as f64 * 0.95).max(3.6);
    let base_top_y = 4.25 + total_span_y * 0.5;

    #[allow(clippy::too_many_arguments)]
    fn emit_pivot_subtree(
        doc: &mut VisioDocument,
        page_id: u32,
        node: &PivotTreeNode,
        parent_id: Option<u64>,
        top_y: f64,
        span_y: f64,
        agg_op: PivotAggOp,
        root_total: f64,
        metric_name: &str,
        level_fills: &[&str; 4],
        created_nodes: &mut usize,
    ) {
        let cx = 1.65 + (node.depth as f64) * 2.55;
        let cy = top_y - span_y * 0.5;
        let agg_val = agg_op.compute(&node.values);
        let sum_val = PivotAggOp::Sum.compute(&node.values);
        let share_pct = ((sum_val / root_total) * 100.0).clamp(0.0, 100.0);

        let sid = if let Some(page) = doc.page_mut(page_id) {
            let id = page.next_shape_id();
            let mut card = VisioShape::new_rect(id, format!("Pivot.{}", id), cx, cy, 1.95, 0.72);
            card.text = format!(
                "{}\n{}: {:.2} (n={})",
                node.label,
                agg_op.label(),
                agg_val,
                node.values.len()
            );
            let fill = level_fills[node.depth.min(level_fills.len() - 1)];
            card.sheet.set_formula("FillForegnd", fill, Some(Unit::Color));
            card.sheet
                .set_formula("LineColor", "=RGB(30,64,175)", Some(Unit::Color));
            card.sheet
                .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
            card.sheet.set_formula(
                "Char.Color",
                if node.depth == 0 {
                    "=RGB(255,255,255)"
                } else {
                    "=RGB(15,23,42)"
                },
                Some(Unit::Color),
            );
            card.sheet
                .set_formula("Char.Size", "8.5 pt", Some(Unit::Point));
            card.sheet
                .set_user_cell("DomainRole", "\"pivot_node\"", "PivotDiagram Rollup Card");
            card.sheet.set_shape_data(ShapeDataRow {
                name: "PivotLevel".into(),
                label: "Pivot Level".into(),
                prompt: "Hierarchy Depth".into(),
                data_type: ShapeDataType::Number,
                format: "0".into(),
                value: node.depth.to_string(),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            card.sheet.set_shape_data(ShapeDataRow {
                name: "RecordCount".into(),
                label: "Count".into(),
                prompt: "Underlying Record Count".into(),
                data_type: ShapeDataType::Number,
                format: "0".into(),
                value: node.values.len().to_string(),
                sort_key: "2".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            card.sheet.set_shape_data(ShapeDataRow {
                name: "AggregatedValue".into(),
                label: format!("{} ({})", metric_name, agg_op.label()),
                prompt: "Aggregated Metric".into(),
                data_type: ShapeDataType::Number,
                format: "0.00".into(),
                value: format_clean_f64(agg_val),
                sort_key: "3".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            card.sheet.set_shape_data(ShapeDataRow {
                name: "SharePct".into(),
                label: "Share (%)".into(),
                prompt: "Percentage of Grand Total".into(),
                data_type: ShapeDataType::Number,
                format: "0.0".into(),
                value: format_clean_f64(share_pct),
                sort_key: "4".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            page.shapes.push(card);
            *created_nodes += 1;

            if let Some(pid) = parent_id {
                let cid = page.next_shape_id();
                let mut conn = VisioShape::new_connector(cid, format!("PivotLink.{cid}"), cx - 1.2, cy, cx - 0.98, cy);
                conn.sheet.set_formula("EndArrow", "4", Some(Unit::Number));
                conn.sheet
                    .set_formula("LineColor", "=RGB(100,116,139)", Some(Unit::Color));
                page.shapes.push(conn);
                page.connects.push(GlueEndpoint {
                    connector_shape_id: cid,
                    from_cell: "BeginX".into(),
                    target_shape_id: pid,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                });
                page.connects.push(GlueEndpoint {
                    connector_shape_id: cid,
                    from_cell: "EndX".into(),
                    target_shape_id: id,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                });
            }
            id
        } else {
            return;
        };

        let mut cursor_top = top_y;
        for child in node.children.values() {
            let child_span = span_y * (child.leaf_weight as f64) / (node.leaf_weight.max(1) as f64);
            emit_pivot_subtree(
                doc,
                page_id,
                child,
                Some(sid),
                cursor_top,
                child_span,
                agg_op,
                root_total,
                metric_name,
                level_fills,
                created_nodes,
            );
            cursor_top -= child_span;
        }
    }

    emit_pivot_subtree(
        doc,
        page_id,
        &root,
        None,
        base_top_y,
        total_span_y,
        agg_op,
        root_total,
        &metric_name,
        &level_fills,
        &mut created_nodes,
    );
    crate::routing::route_page_connectors(doc, page_id);
    ReactiveEngine::recalculate_document(doc);
    created_nodes
}

// ============================================================================
// 5. Brainstorming / Mind Map Generator, Mutator & Markdown Outline Exporter
// ============================================================================

#[derive(Debug, Clone)]
struct MindTopicNode {
    title: String,
    children: Vec<MindTopicNode>,
    weight: usize,
}

impl MindTopicNode {
    fn compute_weights(&mut self) -> usize {
        if self.children.is_empty() {
            self.weight = 1;
        } else {
            self.weight = self
                .children
                .iter_mut()
                .map(Self::compute_weights)
                .sum::<usize>()
                .max(1);
        }
        self.weight
    }
}

fn parse_mind_map_outline(outline: &str) -> Option<MindTopicNode> {
    let mut items: Vec<(usize, String)> = Vec::new();
    for raw in outline.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        let trimmed = raw.trim();
        if trimmed.starts_with('#') {
            let hashes = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed[hashes..].trim().to_string();
            if !text.is_empty() {
                items.push((hashes.saturating_sub(1), text));
            }
            continue;
        }
        let indent_spaces: usize = raw
            .chars()
            .take_while(|&c| c == ' ' || c == '\t')
            .map(|c| if c == '\t' { 2 } else { 1 })
            .sum();
        let depth = if items.is_empty() {
            0
        } else {
            (indent_spaces / 2) + 1
        };
        let clean = trimmed
            .trim_start_matches(['-', '*', '+', '•'])
            .trim()
            .to_string();
        if !clean.is_empty() {
            items.push((depth, clean));
        }
    }
    if items.is_empty() {
        return None;
    }

    // Normalize depths if multiple depth-0 items exist: wrap or attach under first item
    let root_title = items[0].1.clone();
    let mut root = MindTopicNode {
        title: root_title,
        children: Vec::new(),
        weight: 1,
    };

    fn insert_at_depth(node: &mut MindTopicNode, rel_depth: usize, title: String) {
        if rel_depth <= 1 || node.children.is_empty() {
            node.children.push(MindTopicNode {
                title,
                children: Vec::new(),
                weight: 1,
            });
        } else if let Some(last) = node.children.last_mut() {
            insert_at_depth(last, rel_depth - 1, title);
        }
    }

    let base_depth = items[0].0;
    for (d, title) in items.into_iter().skip(1) {
        let rel = d.saturating_sub(base_depth).max(1);
        insert_at_depth(&mut root, rel, title);
    }
    root.compute_weights();
    Some(root)
}

/// Build a Visio Brainstorming / Mind Map diagram (`balanced`, `radial`, or `right`) from an indented outline.
pub fn build_mind_map_from_outline(
    doc: &mut VisioDocument,
    page_id: u32,
    outline_text: &str,
    layout_style: &str,
) -> usize {
    let Some(root) = parse_mind_map_outline(outline_text) else {
        return 0;
    };
    let style = layout_style.trim().to_ascii_lowercase();
    let (pw, ph) = doc
        .page(page_id)
        .map(|p| {
            (
                p.page_sheet.get_f64("PageWidth").unwrap_or(11.0),
                p.page_sheet.get_f64("PageHeight").unwrap_or(8.5),
            )
        })
        .unwrap_or((11.0, 8.5));
    let cx = pw * 0.5;
    let cy = ph * 0.5;

    let branch_colors = [
        ("=RGB(219,234,254)", "=RGB(29,78,216)"),
        ("=RGB(220,252,231)", "=RGB(21,128,61)"),
        ("=RGB(254,243,199)", "=RGB(180,83,9)"),
        ("=RGB(243,232,255)", "=RGB(126,34,206)"),
        ("=RGB(255,228,230)", "=RGB(190,18,60)"),
        ("=RGB(204,251,241)", "=RGB(15,118,110)"),
    ];

    let mut created = 0usize;
    let root_id = if let Some(page) = doc.page_mut(page_id) {
        let id = page.next_shape_id();
        let mut center_node = VisioShape::new_rect(id, "CentralTopic", cx, cy, 2.20, 0.88);
        center_node.text = root.title.clone();
        center_node
            .sheet
            .set_formula("FillForegnd", "=RGB(30,41,59)", Some(Unit::Color));
        center_node
            .sheet
            .set_formula("LineColor", "=RGB(15,23,42)", Some(Unit::Color));
        center_node
            .sheet
            .set_formula("Rounding", "0.22 in", Some(Unit::Inch));
        center_node
            .sheet
            .set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        center_node
            .sheet
            .set_formula("Char.Size", "12 pt", Some(Unit::Point));
        center_node
            .sheet
            .set_formula("Char.Style", "1", Some(Unit::Number));
        center_node
            .sheet
            .set_user_cell("DomainRole", "\"mind_map_topic\"", "Mind Map Topic");
        center_node
            .sheet
            .set_user_cell("MindMapDepth", "0", "Topic Depth");
        center_node
            .sheet
            .set_user_cell("MindMapParentId", "0", "Parent Topic ID");
        center_node
            .sheet
            .set_user_cell("MindMapOrder", "0", "Sibling Order");
        page.shapes.push(center_node);
        created += 1;
        id
    } else {
        return 0;
    };

    #[allow(clippy::too_many_arguments)]
    fn emit_branch(
        doc: &mut VisioDocument,
        page_id: u32,
        node: &MindTopicNode,
        parent_id: u64,
        depth: usize,
        order_idx: usize,
        dir_sign: f64, // +1.0 right, -1.0 left
        base_x: f64,
        top_y: f64,
        span_y: f64,
        fill_f: &str,
        line_f: &str,
        created: &mut usize,
    ) {
        let nx = base_x + dir_sign * (depth as f64) * 2.25;
        let ny = top_y - span_y * 0.5;
        let w = if depth == 1 { 1.75 } else { 1.55 };
        let h = if depth == 1 { 0.62 } else { 0.50 };

        let my_id = if let Some(page) = doc.page_mut(page_id) {
            let id = page.next_shape_id();
            let mut sh = VisioShape::new_rect(id, format!("Topic.{id}"), nx, ny, w, h);
            sh.text = node.title.clone();
            sh.sheet.set_formula("FillForegnd", fill_f, Some(Unit::Color));
            sh.sheet.set_formula("LineColor", line_f, Some(Unit::Color));
            sh.sheet.set_formula("Rounding", "0.14 in", Some(Unit::Inch));
            sh.sheet.set_formula(
                "Char.Size",
                if depth == 1 { "10 pt" } else { "9 pt" },
                Some(Unit::Point),
            );
            sh.sheet
                .set_user_cell("DomainRole", "\"mind_map_topic\"", "Mind Map Topic");
            sh.sheet
                .set_user_cell("MindMapDepth", depth.to_string(), "Topic Depth");
            sh.sheet
                .set_user_cell("MindMapParentId", parent_id.to_string(), "Parent Topic ID");
            sh.sheet
                .set_user_cell("MindMapOrder", order_idx.to_string(), "Sibling Order");
            page.shapes.push(sh);
            *created += 1;

            let cid = page.next_shape_id();
            let mut conn = VisioShape::new_connector(cid, format!("Branch.{cid}"), base_x, ny, nx, ny);
            conn.sheet.set_formula("LineColor", line_f, Some(Unit::Color));
            conn.sheet.set_formula("LineWeight", "1.5 pt", Some(Unit::Point));
            conn.sheet
                .set_formula("ConLineRouteExt", "2", Some(Unit::Number));
            conn.sheet
                .set_formula("ShapeRouteStyle", "2", Some(Unit::Number));
            page.shapes.push(conn);
            page.connects.push(GlueEndpoint {
                connector_shape_id: cid,
                from_cell: "BeginX".into(),
                target_shape_id: parent_id,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
            page.connects.push(GlueEndpoint {
                connector_shape_id: cid,
                from_cell: "EndX".into(),
                target_shape_id: id,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
            id
        } else {
            return;
        };

        let mut cursor = top_y;
        for (c_idx, child) in node.children.iter().enumerate() {
            let c_span = span_y * (child.weight as f64) / (node.weight.max(1) as f64);
            emit_branch(
                doc,
                page_id,
                child,
                my_id,
                depth + 1,
                c_idx,
                dir_sign,
                base_x,
                cursor,
                c_span,
                fill_f,
                line_f,
                created,
            );
            cursor -= c_span;
        }
    }

    if style == "radial" {
        let total_w = root.weight.max(1) as f64;
        let mut angle_cursor = 0.0_f64;
        for (idx, branch) in root.children.iter().enumerate() {
            let sector = std::f64::consts::TAU * (branch.weight as f64) / total_w;
            let mid_ang = angle_cursor + sector * 0.5;
            let (fill_f, line_f) = branch_colors[idx % branch_colors.len()];
            let bx = cx + mid_ang.cos() * 2.55;
            let by = cy + mid_ang.sin() * 2.15;
            if let Some(page) = doc.page_mut(page_id) {
                let bid = page.next_shape_id();
                let mut sh = VisioShape::new_rect(bid, format!("Topic.{bid}"), bx, by, 1.70, 0.60);
                sh.text = branch.title.clone();
                sh.sheet.set_formula("FillForegnd", fill_f, Some(Unit::Color));
                sh.sheet.set_formula("LineColor", line_f, Some(Unit::Color));
                sh.sheet.set_formula("Rounding", "0.14 in", Some(Unit::Inch));
                sh.sheet
                    .set_user_cell("DomainRole", "\"mind_map_topic\"", "Mind Map Topic");
                sh.sheet.set_user_cell("MindMapDepth", "1", "Topic Depth");
                sh.sheet
                    .set_user_cell("MindMapParentId", root_id.to_string(), "Parent Topic ID");
                sh.sheet
                    .set_user_cell("MindMapOrder", idx.to_string(), "Sibling Order");
                page.shapes.push(sh);
                created += 1;

                let cid = page.next_shape_id();
                let mut conn = VisioShape::new_connector(cid, format!("Branch.{cid}"), cx, cy, bx, by);
                conn.sheet.set_formula("LineColor", line_f, Some(Unit::Color));
                conn.sheet
                    .set_formula("ConLineRouteExt", "2", Some(Unit::Number));
                page.shapes.push(conn);
                page.connects.extend([
                    GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "BeginX".into(),
                        target_shape_id: root_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                    GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "EndX".into(),
                        target_shape_id: bid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                ]);

                for (sub_i, sub) in branch.children.iter().enumerate() {
                    let spread = (sub_i as f64 - (branch.children.len().saturating_sub(1) as f64) * 0.5) * 0.28;
                    let sa = mid_ang + spread;
                    let sx = cx + sa.cos() * 4.45;
                    let sy = cy + sa.sin() * 3.65;
                    let sid = page.next_shape_id();
                    let mut sub_sh = VisioShape::new_rect(sid, format!("Topic.{sid}"), sx, sy, 1.50, 0.48);
                    sub_sh.text = sub.title.clone();
                    sub_sh
                        .sheet
                        .set_formula("FillForegnd", fill_f, Some(Unit::Color));
                    sub_sh
                        .sheet
                        .set_formula("LineColor", line_f, Some(Unit::Color));
                    sub_sh
                        .sheet
                        .set_formula("Rounding", "0.12 in", Some(Unit::Inch));
                    sub_sh
                        .sheet
                        .set_user_cell("DomainRole", "\"mind_map_topic\"", "Mind Map Topic");
                    sub_sh.sheet.set_user_cell("MindMapDepth", "2", "Topic Depth");
                    sub_sh
                        .sheet
                        .set_user_cell("MindMapParentId", bid.to_string(), "Parent Topic ID");
                    sub_sh
                        .sheet
                        .set_user_cell("MindMapOrder", sub_i.to_string(), "Sibling Order");
                    page.shapes.push(sub_sh);
                    created += 1;

                    let scid = page.next_shape_id();
                    let mut sconn = VisioShape::new_connector(scid, format!("Branch.{scid}"), bx, by, sx, sy);
                    sconn
                        .sheet
                        .set_formula("LineColor", line_f, Some(Unit::Color));
                    sconn
                        .sheet
                        .set_formula("ConLineRouteExt", "2", Some(Unit::Number));
                    page.shapes.push(sconn);
                    page.connects.extend([
                        GlueEndpoint {
                            connector_shape_id: scid,
                            from_cell: "BeginX".into(),
                            target_shape_id: bid,
                            to_cell: "PinX".into(),
                            is_dynamic: true,
                        },
                        GlueEndpoint {
                            connector_shape_id: scid,
                            from_cell: "EndX".into(),
                            target_shape_id: sid,
                            to_cell: "PinX".into(),
                            is_dynamic: true,
                        },
                    ]);
                }
            }
            angle_cursor += sector;
        }
    } else {
        // Balanced right/left (or right-only) organic mind map
        let right_only = style.contains("right");
        let mut right_branches = Vec::new();
        let mut left_branches = Vec::new();
        for (idx, b) in root.children.iter().enumerate() {
            if right_only || idx % 2 == 0 {
                right_branches.push((idx, b));
            } else {
                left_branches.push((idx, b));
            }
        }
        let rw_sum: usize = right_branches.iter().map(|(_, b)| b.weight).sum::<usize>().max(1);
        let r_span = (rw_sum as f64 * 0.85).max(3.0);
        let mut r_cur = cy + r_span * 0.5;
        for (orig_idx, b) in right_branches {
            let (fill_f, line_f) = branch_colors[orig_idx % branch_colors.len()];
            let sub_span = r_span * (b.weight as f64) / (rw_sum as f64);
            emit_branch(
                doc,
                page_id,
                b,
                root_id,
                1,
                orig_idx,
                1.0,
                cx,
                r_cur,
                sub_span,
                fill_f,
                line_f,
                &mut created,
            );
            r_cur -= sub_span;
        }

        if !left_branches.is_empty() {
            let lw_sum: usize = left_branches.iter().map(|(_, b)| b.weight).sum::<usize>().max(1);
            let l_span = (lw_sum as f64 * 0.85).max(3.0);
            let mut l_cur = cy + l_span * 0.5;
            for (orig_idx, b) in left_branches {
                let (fill_f, line_f) = branch_colors[orig_idx % branch_colors.len()];
                let sub_span = l_span * (b.weight as f64) / (lw_sum as f64);
                emit_branch(
                    doc,
                    page_id,
                    b,
                    root_id,
                    1,
                    orig_idx,
                    -1.0,
                    cx,
                    l_cur,
                    sub_span,
                    fill_f,
                    line_f,
                    &mut created,
                );
                l_cur -= sub_span;
            }
        }
    }

    crate::routing::route_page_connectors(doc, page_id);
    ReactiveEngine::recalculate_document(doc);
    created
}

/// Add a child or sibling topic to an existing Mind Map node and connect it cleanly!
pub fn add_mind_map_topic(
    doc: &mut VisioDocument,
    page_id: u32,
    anchor_shape_id: u64,
    topic_text: &str,
    as_sibling: bool,
) -> Option<u64> {
    let page = doc.page(page_id)?;
    let anchor = if anchor_shape_id > 0 {
        page.find_shape(anchor_shape_id)
    } else {
        page.shapes
            .iter()
            .find(|s| user_cell_str(s, "DomainRole").is_some_and(|v| v == "mind_map_topic"))
    }?;

    let anchor_id = anchor.id;
    let anchor_depth = anchor
        .sheet
        .get_f64("User.MindMapDepth")
        .unwrap_or(0.0)
        .round() as usize;
    let anchor_parent = anchor
        .sheet
        .get_f64("User.MindMapParentId")
        .unwrap_or(0.0)
        .round() as u64;
    let parent_id = if as_sibling && anchor_parent > 0 {
        anchor_parent
    } else {
        anchor_id
    };
    let parent_sh = page.find_shape(parent_id)?;
    let parent_x = parent_sh.sheet.get_f64("PinX").unwrap_or(5.5);
    let parent_y = parent_sh.sheet.get_f64("PinY").unwrap_or(4.25);
    let new_depth = if parent_id == anchor_id {
        anchor_depth + 1
    } else {
        anchor_depth.max(1)
    };
    let fill_f = parent_sh
        .sheet
        .get_cell("FillForegnd")
        .map(|c| c.formula.clone())
        .filter(|f| !f.contains("30,41,59"))
        .unwrap_or_else(|| "=RGB(219,234,254)".into());
    let line_f = parent_sh
        .sheet
        .get_cell("LineColor")
        .map(|c| c.formula.clone())
        .unwrap_or_else(|| "=RGB(29,78,216)".into());

    // Count existing children of `parent_id` for clean vertical stagger
    let existing_children = page
        .shapes
        .iter()
        .filter(|s| {
            s.sheet
                .get_f64("User.MindMapParentId")
                .is_some_and(|pid| (pid.round() as u64) == parent_id)
        })
        .count();
    let root_x = page
        .shapes
        .iter()
        .find(|s| s.sheet.get_f64("User.MindMapDepth").is_some_and(|d| d.round() == 0.0))
        .and_then(|s| s.sheet.get_f64("PinX"))
        .unwrap_or(5.5);
    let dir_sign = if parent_x < root_x - 0.1 { -1.0 } else { 1.0 };
    let nx = parent_x + dir_sign * 2.15;
    let stagger = (existing_children as f64) * 0.72 - 0.36;
    let ny = parent_y - stagger;

    let page_mut = doc.page_mut(page_id)?;
    let tid = page_mut.next_shape_id();
    let mut topic = VisioShape::new_rect(tid, format!("Topic.{tid}"), nx, ny, 1.60, 0.54);
    topic.text = if topic_text.trim().is_empty() {
        format!("Subtopic {}", existing_children + 1)
    } else {
        topic_text.trim().to_string()
    };
    topic.sheet.set_formula("FillForegnd", fill_f, Some(Unit::Color));
    topic.sheet.set_formula("LineColor", line_f.clone(), Some(Unit::Color));
    topic.sheet.set_formula("Rounding", "0.14 in", Some(Unit::Inch));
    topic
        .sheet
        .set_user_cell("DomainRole", "\"mind_map_topic\"", "Mind Map Topic");
    topic
        .sheet
        .set_user_cell("MindMapDepth", new_depth.to_string(), "Topic Depth");
    topic
        .sheet
        .set_user_cell("MindMapParentId", parent_id.to_string(), "Parent Topic ID");
    topic
        .sheet
        .set_user_cell("MindMapOrder", existing_children.to_string(), "Sibling Order");
    page_mut.shapes.push(topic);

    let cid = page_mut.next_shape_id();
    let mut conn = VisioShape::new_connector(cid, format!("Branch.{cid}"), parent_x, parent_y, nx, ny);
    conn.sheet.set_formula("LineColor", line_f, Some(Unit::Color));
    conn.sheet
        .set_formula("ConLineRouteExt", "2", Some(Unit::Number));
    page_mut.shapes.push(conn);
    page_mut.connects.extend([
        GlueEndpoint {
            connector_shape_id: cid,
            from_cell: "BeginX".into(),
            target_shape_id: parent_id,
            to_cell: "PinX".into(),
            is_dynamic: true,
        },
        GlueEndpoint {
            connector_shape_id: cid,
            from_cell: "EndX".into(),
            target_shape_id: tid,
            to_cell: "PinX".into(),
            is_dynamic: true,
        },
    ]);

    crate::routing::route_page_connectors(doc, page_id);
    ReactiveEngine::recalculate_document(doc);
    Some(tid)
}

/// Export all Mind Map topics on `page_id` back into a lossless indented Markdown outline!
pub fn export_mind_map_outline(doc: &VisioDocument, page_id: u32) -> String {
    let Some(page) = doc.page(page_id) else {
        return String::new();
    };
    let mut by_parent: BTreeMap<u64, Vec<(usize, u64, String)>> = BTreeMap::new();
    let mut roots: Vec<(u64, String)> = Vec::new();

    for sh in &page.shapes {
        let is_mm = user_cell_str(sh, "DomainRole").is_some_and(|v| v == "mind_map_topic");
        if !is_mm {
            continue;
        }
        let pid = sh
            .sheet
            .get_f64("User.MindMapParentId")
            .unwrap_or(0.0)
            .round() as u64;
        let ord = sh
            .sheet
            .get_f64("User.MindMapOrder")
            .unwrap_or(0.0)
            .round() as usize;
        let label = sh.text.trim().replace('\n', " ");
        if pid == 0 {
            roots.push((sh.id, label));
        } else {
            by_parent.entry(pid).or_default().push((ord, sh.id, label));
        }
    }
    for kids in by_parent.values_mut() {
        kids.sort_by_key(|(ord, sid, _)| (*ord, *sid));
    }

    fn emit_md(
        id: u64,
        label: &str,
        depth: usize,
        by_parent: &BTreeMap<u64, Vec<(usize, u64, String)>>,
        lines: &mut Vec<String>,
    ) {
        if depth == 0 {
            lines.push(format!("# {label}"));
        } else {
            let indent = "  ".repeat(depth - 1);
            lines.push(format!("{indent}- {label}"));
        }
        if let Some(children) = by_parent.get(&id) {
            for (_, cid, clabel) in children {
                emit_md(*cid, clabel, depth + 1, by_parent, lines);
            }
        }
    }

    let mut out = Vec::new();
    for (rid, rlabel) in roots {
        emit_md(rid, &rlabel, 0, &by_parent, &mut out);
    }
    out.join("\n")
}

// ============================================================================
// 6. Lean Value Stream Mapping (`VSM`) & Fault Tree Analysis (`FTA`) Calculators
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VsmFtaCalculationReport {
    pub vsm_step_count: usize,
    pub vsm_total_lead_time_days: f64,
    pub vsm_total_value_added_sec: f64,
    pub vsm_pce_percent: f64,
    pub vsm_bottleneck_step: String,
    pub fta_gate_count: usize,
    pub fta_basic_event_count: usize,
    pub fta_top_event_id: u64,
    pub fta_top_probability: f64,
    pub fta_minimal_cut_sets: Vec<Vec<String>>,
    pub fta_birnbaum_importance: BTreeMap<String, f64>,
}

/// Build a complete Lean Value Stream Map (`VSM`) with Process Data Boxes, Inventory Wait
/// Triangles, a Sawtooth Lead-Time / Value-Added Ladder, and a reactive PCE% Scorecard!
/// Spec format: `StepName,CycleTimeSec,WaitDays,UptimePct,Operators` (one step per line).
pub fn build_vsm_diagram(doc: &mut VisioDocument, page_id: u32, spec_text: &str) -> usize {
    #[derive(Clone)]
    struct VsmStep {
        name: String,
        ct_sec: f64,
        wait_days: f64,
        uptime_pct: f64,
        operators: u32,
    }

    let mut steps = Vec::new();
    for raw in spec_text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        if cols.is_empty() {
            continue;
        }
        // Skip optional CSV header row
        if cols.get(1).is_some_and(|c| c.parse::<f64>().is_err()) && steps.is_empty() {
            continue;
        }
        let name = cols[0].to_string();
        let ct_sec = cols.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(45.0).max(0.0);
        let wait_days = cols.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(1.5).max(0.0);
        let uptime_pct = cols.get(3).and_then(|s| s.parse::<f64>().ok()).unwrap_or(95.0).clamp(0.0, 100.0);
        let operators = cols.get(4).and_then(|s| s.parse::<u32>().ok()).unwrap_or(1).max(1);
        steps.push(VsmStep {
            name,
            ct_sec,
            wait_days,
            uptime_pct,
            operators,
        });
    }
    if steps.is_empty() {
        return 0;
    }

    let mut created = 0usize;
    let start_x = 1.65;
    let step_dx = 2.15;
    let proc_y = 4.65;

    if let Some(page) = doc.page_mut(page_id) {
        // Production Control top box
        let ctrl_id = page.next_shape_id();
        let mid_x = start_x + ((steps.len().saturating_sub(1)) as f64) * step_dx * 0.5;
        let mut ctrl = VisioShape::new_rect(ctrl_id, "ProductionControl", mid_x, 6.85, 2.40, 0.72);
        ctrl.text = "Production Control\n(MRP / Kanban Schedule)".into();
        ctrl.sheet
            .set_formula("FillForegnd", "=RGB(224,231,255)", Some(Unit::Color));
        ctrl.sheet
            .set_formula("LineColor", "=RGB(67,56,202)", Some(Unit::Color));
        page.shapes.push(ctrl);
        created += 1;

        let mut prev_proc_id: Option<u64> = None;
        let mut ladder_pts: Vec<[f64; 2]> = Vec::new();
        let ladder_base_y = 2.05;

        for (idx, st) in steps.iter().enumerate() {
            let sx = start_x + (idx as f64) * step_dx;

            // Inventory Wait Triangle before step
            let inv_x = sx - step_dx * 0.45;
            let inv_id = page.next_shape_id();
            let mut inv = VisioShape::new_rect(inv_id, format!("Inventory.{}", idx + 1), inv_x, proc_y, 0.62, 0.56);
            inv.text = format!("I\n{:.1}d", st.wait_days);
            inv.sheet.geometries.clear();
            let mut tri = GeometrySection::new(1);
            tri.rows.push(GeometryRow::move_to(1, "0 in", "0 in"));
            tri.rows.push(GeometryRow::line_to(2, "=Width", "0 in"));
            tri.rows.push(GeometryRow::line_to(3, "=Width*0.5", "=Height"));
            tri.rows.push(GeometryRow::line_to(4, "0 in", "0 in"));
            inv.sheet.add_geometry(tri);
            inv.sheet
                .set_formula("FillForegnd", "=RGB(254,243,199)", Some(Unit::Color));
            inv.sheet
                .set_formula("LineColor", "=RGB(180,83,9)", Some(Unit::Color));
            inv.sheet
                .set_user_cell("DomainRole", "\"vsm_inventory\"", "VSM Inventory Buffer");
            inv.sheet.set_shape_data(ShapeDataRow {
                name: "WaitDays".into(),
                label: "Wait Time (days)".into(),
                prompt: "Non-Value-Added Queue Wait".into(),
                data_type: ShapeDataType::Number,
                format: "0.00".into(),
                value: format_clean_f64(st.wait_days),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            page.shapes.push(inv);
            created += 1;

            // Process Step Data Box
            let pid = page.next_shape_id();
            let mut proc_sh = VisioShape::new_rect(pid, format!("VsmStep.{}", idx + 1), sx, proc_y, 1.35, 1.05);
            proc_sh.text = format!(
                "{}\nC/T: {:.0}s • Up: {:.0}%\nOps: {}",
                st.name, st.ct_sec, st.uptime_pct, st.operators
            );
            proc_sh
                .sheet
                .set_formula("FillForegnd", "=RGB(219,234,254)", Some(Unit::Color));
            proc_sh
                .sheet
                .set_formula("LineColor", "=RGB(30,64,175)", Some(Unit::Color));
            proc_sh
                .sheet
                .set_user_cell("DomainRole", "\"vsm_process\"", "VSM Process Step");
            proc_sh.sheet.set_shape_data(ShapeDataRow {
                name: "CycleTimeSec".into(),
                label: "Cycle Time (s)".into(),
                prompt: "Value-Added Cycle Time (sec)".into(),
                data_type: ShapeDataType::Number,
                format: "0.0".into(),
                value: format_clean_f64(st.ct_sec),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            proc_sh.sheet.set_shape_data(ShapeDataRow {
                name: "WaitDays".into(),
                label: "Lead Wait (days)".into(),
                prompt: "Upstream Inventory Wait (days)".into(),
                data_type: ShapeDataType::Number,
                format: "0.00".into(),
                value: format_clean_f64(st.wait_days),
                sort_key: "2".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            proc_sh.sheet.set_shape_data(ShapeDataRow {
                name: "UptimePct".into(),
                label: "Uptime (%)".into(),
                prompt: "Machine Uptime Percentage".into(),
                data_type: ShapeDataType::Number,
                format: "0.0".into(),
                value: format_clean_f64(st.uptime_pct),
                sort_key: "3".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            proc_sh.sheet.set_shape_data(ShapeDataRow {
                name: "Operators".into(),
                label: "Operators".into(),
                prompt: "Operator Headcount".into(),
                data_type: ShapeDataType::Number,
                format: "0".into(),
                value: st.operators.to_string(),
                sort_key: "4".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            page.shapes.push(proc_sh);
            created += 1;

            if let Some(prev_id) = prev_proc_id {
                let cid = page.next_shape_id();
                let mut push = VisioShape::new_connector(cid, format!("PushFlow.{cid}"), sx - step_dx, proc_y, sx, proc_y);
                push.sheet.set_formula("EndArrow", "4", Some(Unit::Number));
                push.sheet.set_formula("LineWeight", "2.0 pt", Some(Unit::Point));
                page.shapes.push(push);
                page.connects.extend([
                    GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "BeginX".into(),
                        target_shape_id: prev_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                    GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "EndX".into(),
                        target_shape_id: pid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                ]);
            }
            prev_proc_id = Some(pid);

            // Sawtooth ladder coordinates: high segment under inventory, low trough under process box
            let seg_left = inv_x - 0.35;
            let seg_mid = sx - 0.65;
            let seg_right = sx + 0.65;
            ladder_pts.push([seg_left, ladder_base_y + 0.48]);
            ladder_pts.push([seg_mid, ladder_base_y + 0.48]);
            ladder_pts.push([seg_mid, ladder_base_y]);
            ladder_pts.push([seg_right, ladder_base_y]);
        }

        // Sawtooth Timeline Ladder shape
        if ladder_pts.len() >= 2 {
            let min_lx = ladder_pts.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
            let max_lx = ladder_pts.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
            let lw = (max_lx - min_lx).max(1.0);
            let lh = 0.55;
            let lid = page.next_shape_id();
            let mut ladder = VisioShape::new_rect(
                lid,
                "VsmSawtoothLadder",
                min_lx + lw * 0.5,
                ladder_base_y + lh * 0.5,
                lw,
                lh,
            );
            ladder.text.clear();
            ladder.sheet.geometries.clear();
            ladder.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
            ladder
                .sheet
                .set_formula("LineColor", "=RGB(15,23,42)", Some(Unit::Color));
            ladder
                .sheet
                .set_formula("LineWeight", "1.5 pt", Some(Unit::Point));
            let mut g = GeometrySection::new(1);
            g.no_fill = "TRUE".into();
            for (ri, pt) in ladder_pts.iter().enumerate() {
                let lx = format!("{} in", format_clean_f64(pt[0] - min_lx));
                let ly = format!("{} in", format_clean_f64(pt[1] - ladder_base_y));
                if ri == 0 {
                    g.rows.push(GeometryRow::move_to(1, lx, ly));
                } else {
                    g.rows.push(GeometryRow::line_to((ri + 1) as u32, lx, ly));
                }
            }
            ladder.sheet.add_geometry(g);
            ladder
                .sheet
                .set_user_cell("DomainRole", "\"vsm_ladder\"", "VSM Sawtooth Timeline");
            page.shapes.push(ladder);
            created += 1;
        }

        // VSM Summary Scorecard
        let sum_id = page.next_shape_id();
        let sum_x = start_x + (steps.len() as f64) * step_dx + 0.35;
        let mut scorecard = VisioShape::new_rect(sum_id, "VsmScorecard", sum_x, ladder_base_y + 0.35, 2.35, 1.05);
        scorecard
            .sheet
            .set_formula("FillForegnd", "=RGB(220,252,231)", Some(Unit::Color));
        scorecard
            .sheet
            .set_formula("LineColor", "=RGB(21,128,61)", Some(Unit::Color));
        scorecard
            .sheet
            .set_formula("Char.Size", "8.5 pt", Some(Unit::Point));
        scorecard
            .sheet
            .set_user_cell("DomainRole", "\"vsm_scorecard\"", "VSM Summary Scorecard");
        page.shapes.push(scorecard);
        created += 1;
    }

    let _ = recalculate_vsm_and_fta(doc, page_id);
    crate::routing::route_page_connectors(doc, page_id);
    created
}

/// Build a Fault Tree Analysis (`FTA`) diagram with `AND`, `OR`, `XOR`, `NOT`, and `VOTING(k/n)`
/// logic gates, live reactive ShapeSheet gate probability formulas, and basic-event probabilities!
/// Spec format per line: `NodeID,Label,GateOrBasic,ProbOrK,ChildIDs(; separated)`
/// Example:
/// `TOP,Core Outage,OR,0,G1;G2`
/// `G1,Primary Power Lost,AND,0,B1;B2`
/// `G2,Cooling Failure,2/3,0,B3;B4;B5`
/// `B1,Grid Utility Fault,BASIC,0.02,`
pub fn build_fault_tree_diagram(doc: &mut VisioDocument, page_id: u32, spec_text: &str) -> usize {
    #[derive(Clone)]
    struct FtaRow {
        key: String,
        label: String,
        gate_type: String, // "AND", "OR", "XOR", "NOT", "2/3", "BASIC"
        prob: f64,
        children: Vec<String>,
    }

    let mut rows: Vec<FtaRow> = Vec::new();
    for raw in spec_text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(str::trim).collect();
        if cols.len() < 3 {
            continue;
        }
        let key = cols[0].to_string();
        let label = cols[1].to_string();
        let gate_type = cols[2].to_ascii_uppercase();
        if gate_type == "GATEORBASIC" || gate_type == "TYPE" {
            continue;
        }
        let prob = cols
            .get(3)
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.01)
            .clamp(0.0, 1.0);
        let children = cols
            .get(4)
            .map(|c| {
                c.split([';', '|', ' '])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        rows.push(FtaRow {
            key,
            label,
            gate_type,
            prob,
            children,
        });
    }
    if rows.is_empty() {
        return 0;
    }

    // Compute tree depths via BFS from root (first node not referenced as a child, or rows[0])
    let all_children: BTreeSet<String> = rows.iter().flat_map(|r| r.children.iter().cloned()).collect();
    let root_key = rows
        .iter()
        .find(|r| !all_children.contains(&r.key))
        .map(|r| r.key.clone())
        .unwrap_or_else(|| rows[0].key.clone());

    let row_map: BTreeMap<String, FtaRow> = rows.iter().map(|r| (r.key.clone(), r.clone())).collect();
    let mut depth_by_key: BTreeMap<String, usize> = BTreeMap::new();
    let mut queue = std::collections::VecDeque::new();
    depth_by_key.insert(root_key.clone(), 0);
    queue.push_back(root_key);
    while let Some(k) = queue.pop_front() {
        let d = depth_by_key[&k];
        if let Some(r) = row_map.get(&k) {
            for ch in &r.children {
                let next_d = d + 1;
                if depth_by_key.get(ch).is_none_or(|&old| next_d > old) {
                    depth_by_key.insert(ch.clone(), next_d);
                    queue.push_back(ch.clone());
                }
            }
        }
    }
    for r in &rows {
        depth_by_key.entry(r.key.clone()).or_insert(1);
    }

    let mut by_level: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for r in &rows {
        by_level.entry(depth_by_key[&r.key]).or_default().push(r.key.clone());
    }

    let mut shape_id_by_key: BTreeMap<String, u64> = BTreeMap::new();
    let mut created = 0usize;

    if let Some(page) = doc.page_mut(page_id) {
        let pw = page.page_sheet.get_f64("PageWidth").unwrap_or(11.0);
        let ph = page.page_sheet.get_f64("PageHeight").unwrap_or(8.5);
        let top_y = ph - 1.15;

        for (&lvl, keys) in &by_level {
            let count = keys.len().max(1);
            let span_w = (pw - 1.6).max(6.0);
            let step_x = span_w / (count as f64);
            let ny = (top_y - (lvl as f64) * 1.85).max(1.1);

            for (idx, k) in keys.iter().enumerate() {
                let r = &row_map[k];
                let nx = 0.8 + (idx as f64 + 0.5) * step_x;
                let sid = page.next_shape_id();
                let is_basic = r.gate_type == "BASIC" || r.children.is_empty();
                let mut sh = VisioShape::new_rect(
                    sid,
                    format!("FTA.{}", r.key),
                    nx,
                    ny,
                    if is_basic { 1.45 } else { 1.75 },
                    if is_basic { 0.85 } else { 0.92 },
                );
                sh.sheet.geometries.clear();
                if is_basic {
                    // Circle/capsule basic event symbol
                    sh.sheet
                        .set_formula("FillForegnd", "=RGB(254,249,195)", Some(Unit::Color));
                    sh.sheet
                        .set_formula("LineColor", "=RGB(180,83,9)", Some(Unit::Color));
                    sh.sheet
                        .set_formula("Rounding", "0.42 in", Some(Unit::Inch));
                    sh.sheet.add_geometry(GeometrySection::rectangle(1));
                    sh.sheet
                        .set_user_cell("DomainRole", "\"fta_basic\"", "FTA Basic Event");
                    sh.sheet
                        .set_user_cell("FtaGateType", "\"BASIC\"", "FTA Node Type");
                } else if r.gate_type == "AND" {
                    // Classic flat-bottom, semicircular-arch top AND gate!
                    let mut g = GeometrySection::new(1);
                    g.rows.push(GeometryRow::move_to(1, "0.15 in", "0 in"));
                    g.rows.push(GeometryRow::line_to(2, "=Width-0.15 in", "0 in"));
                    g.rows
                        .push(GeometryRow::line_to(3, "=Width-0.15 in", "=Height*0.55"));
                    g.rows
                        .push(GeometryRow::arc_to(4, "0.15 in", "=Height*0.55", "0.38 in"));
                    g.rows.push(GeometryRow::line_to(5, "0.15 in", "0 in"));
                    sh.sheet.add_geometry(g);
                    sh.sheet
                        .set_formula("FillForegnd", "=RGB(219,234,254)", Some(Unit::Color));
                    sh.sheet
                        .set_formula("LineColor", "=RGB(29,78,216)", Some(Unit::Color));
                    sh.sheet
                        .set_user_cell("DomainRole", "\"fta_gate\"", "FTA Logic Gate");
                    sh.sheet
                        .set_user_cell("FtaGateType", "\"AND\"", "FTA Gate Type");
                } else {
                    // Classic curved-shield OR / VOTING / XOR gate!
                    let mut g = GeometrySection::new(1);
                    g.rows.push(GeometryRow::move_to(1, "0.12 in", "0 in"));
                    g.rows
                        .push(GeometryRow::arc_to(2, "=Width-0.12 in", "0 in", "0.14 in"));
                    g.rows
                        .push(GeometryRow::arc_to(3, "=Width*0.5", "=Height", "0.24 in"));
                    g.rows
                        .push(GeometryRow::arc_to(4, "0.12 in", "0 in", "0.24 in"));
                    sh.sheet.add_geometry(g);
                    sh.sheet.set_formula(
                        "FillForegnd",
                        if lvl == 0 {
                            "=RGB(254,226,226)"
                        } else {
                            "=RGB(224,231,255)"
                        },
                        Some(Unit::Color),
                    );
                    sh.sheet.set_formula(
                        "LineColor",
                        if lvl == 0 {
                            "=RGB(185,28,28)"
                        } else {
                            "=RGB(67,56,202)"
                        },
                        Some(Unit::Color),
                    );
                    sh.sheet
                        .set_user_cell("DomainRole", "\"fta_gate\"", "FTA Logic Gate");
                    sh.sheet.set_user_cell(
                        "FtaGateType",
                        format!("\"{}\"", r.gate_type),
                        "FTA Gate Type",
                    );
                }
                sh.sheet
                    .set_user_cell("FtaKey", format!("\"{}\"", r.key), "FTA Event Code");
                sh.sheet
                    .set_user_cell("FtaLabel", format!("\"{}\"", r.label), "FTA Event Label");
                sh.sheet
                    .set_formula("Char.Size", "8 pt", Some(Unit::Point));
                sh.sheet.set_shape_data(ShapeDataRow {
                    name: "Probability".into(),
                    label: "Probability".into(),
                    prompt: "Failure Probability [0..1]".into(),
                    data_type: ShapeDataType::Number,
                    format: "0.000000".into(),
                    value: format!("{:.6}", r.prob),
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
                page.shapes.push(sh);
                shape_id_by_key.insert(r.key.clone(), sid);
                created += 1;
            }
        }

        // Connect gate -> child inputs AND wire reactive ShapeSheet `Prop.Probability` formulas!
        for r in &rows {
            let Some(&gate_sid) = shape_id_by_key.get(&r.key) else {
                continue;
            };
            let child_sids: Vec<u64> = r
                .children
                .iter()
                .filter_map(|ck| shape_id_by_key.get(ck).copied())
                .collect();
            if child_sids.is_empty() {
                continue;
            }

            // Wire live ShapeSheet formula for AND / OR / NOT / XOR gates
            let formula_opt = match r.gate_type.as_str() {
                "AND" => Some(format!(
                    "={}",
                    child_sids
                        .iter()
                        .map(|id| format!("Sheet.{id}!Prop.Probability"))
                        .collect::<Vec<_>>()
                        .join(" * ")
                )),
                "OR" => Some(format!(
                    "=1 - ({})",
                    child_sids
                        .iter()
                        .map(|id| format!("(1 - Sheet.{id}!Prop.Probability)"))
                        .collect::<Vec<_>>()
                        .join(" * ")
                )),
                "NOT" => Some(format!("=1 - Sheet.{}!Prop.Probability", child_sids[0])),
                "XOR" if child_sids.len() == 2 => {
                    let a = child_sids[0];
                    let b = child_sids[1];
                    Some(format!(
                        "=Sheet.{a}!Prop.Probability + Sheet.{b}!Prop.Probability - 2 * Sheet.{a}!Prop.Probability * Sheet.{b}!Prop.Probability"
                    ))
                }
                _ => None, // k/n voting computed deterministically in `recalculate_vsm_and_fta`
            };
            if let (Some(f), Some(gsh)) = (formula_opt, page.find_shape_mut(gate_sid)) {
                gsh.sheet.set_shape_data(ShapeDataRow {
                    name: "Probability".into(),
                    label: "Probability".into(),
                    prompt: "Computed Gate Failure Probability".into(),
                    data_type: ShapeDataType::Number,
                    format: "0.000000".into(),
                    value: f,
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
                gsh.sheet.sync_all_sections_to_cells();
            }

            for csid in child_sids {
                let cid = page.next_shape_id();
                let mut conn = VisioShape::new_connector(cid, format!("FtaEdge.{cid}"), 0.0, 0.0, 1.0, 1.0);
                conn.sheet
                    .set_formula("LineColor", "=RGB(71,85,105)", Some(Unit::Color));
                page.shapes.push(conn);
                page.connects.extend([
                    GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "BeginX".into(),
                        target_shape_id: gate_sid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                    GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "EndX".into(),
                        target_shape_id: csid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                ]);
            }
        }
    }

    crate::routing::route_page_connectors(doc, page_id);
    let _ = recalculate_vsm_and_fta(doc, page_id);
    created
}

fn parse_voting_k_of_n(gate_type: &str, n_inputs: usize) -> Option<usize> {
    let norm = gate_type
        .trim_start_matches("VOTING")
        .trim_matches(['(', ')', ' ']);
    let (k_str, _n_str) = norm.split_once('/').or_else(|| norm.split_once("_OF_"))?;
    let k = k_str.trim().parse::<usize>().ok()?;
    Some(k.clamp(1, n_inputs.max(1)))
}

fn eval_fta_gate_prob(gate_type: &str, child_probs: &[f64]) -> f64 {
    if child_probs.is_empty() {
        return 0.0;
    }
    match gate_type {
        "AND" => child_probs.iter().product::<f64>().clamp(0.0, 1.0),
        "OR" => (1.0 - child_probs.iter().map(|&p| 1.0 - p).product::<f64>()).clamp(0.0, 1.0),
        "NOT" => (1.0 - child_probs[0]).clamp(0.0, 1.0),
        "XOR" if child_probs.len() == 2 => {
            let (a, b) = (child_probs[0], child_probs[1]);
            (a + b - 2.0 * a * b).clamp(0.0, 1.0)
        }
        other => {
            if let Some(k) = parse_voting_k_of_n(other, child_probs.len()) {
                // Exact heterogeneous binomial DP for P(>= k of n active)
                let n = child_probs.len();
                let mut dp = vec![0.0_f64; n + 1];
                dp[0] = 1.0;
                for &p in child_probs {
                    let pc = p.clamp(0.0, 1.0);
                    for m in (1..=n).rev() {
                        dp[m] = dp[m] * (1.0 - pc) + dp[m - 1] * pc;
                    }
                    dp[0] *= 1.0 - pc;
                }
                dp[k..=n].iter().sum::<f64>().clamp(0.0, 1.0)
            } else {
                (1.0 - child_probs.iter().map(|&p| 1.0 - p).product::<f64>()).clamp(0.0, 1.0)
            }
        }
    }
}

/// Recalculate both Lean VSM (Total Lead Time, Value-Added Time, PCE%, Bottleneck, Scorecard text)
/// and Fault Tree Analysis (gate probabilities including `k/n` voting, MOCUS Minimal Cut Sets, and
/// Birnbaum Importance) across `page_id`!
pub fn recalculate_vsm_and_fta(doc: &mut VisioDocument, page_id: u32) -> VsmFtaCalculationReport {
    ReactiveEngine::recalculate_document(doc);

    let mut report = VsmFtaCalculationReport {
        vsm_step_count: 0,
        vsm_total_lead_time_days: 0.0,
        vsm_total_value_added_sec: 0.0,
        vsm_pce_percent: 0.0,
        vsm_bottleneck_step: String::new(),
        fta_gate_count: 0,
        fta_basic_event_count: 0,
        fta_top_event_id: 0,
        fta_top_probability: 0.0,
        fta_minimal_cut_sets: Vec::new(),
        fta_birnbaum_importance: BTreeMap::new(),
    };

    let Some(page) = doc.page(page_id) else {
        return report;
    };

    // --- 1. Lean VSM Rollup ---
    let mut max_ct = -1.0_f64;
    let mut scorecard_ids = Vec::new();
    for sh in &page.shapes {
        let role = user_cell_str(sh, "DomainRole").unwrap_or("");
        if role == "vsm_process" {
            report.vsm_step_count += 1;
            let ct = sh.sheet.get_f64("Prop.CycleTimeSec").unwrap_or(0.0).max(0.0);
            let wd = sh.sheet.get_f64("Prop.WaitDays").unwrap_or(0.0).max(0.0);
            report.vsm_total_value_added_sec += ct;
            report.vsm_total_lead_time_days += wd;
            if ct > max_ct {
                max_ct = ct;
                report.vsm_bottleneck_step = sh
                    .text
                    .lines()
                    .next()
                    .unwrap_or(&sh.name)
                    .to_string();
            }
        } else if role == "vsm_scorecard" {
            scorecard_ids.push(sh.id);
        }
    }
    if report.vsm_step_count > 0 {
        let total_lead_sec =
            report.vsm_total_lead_time_days * 86_400.0 + report.vsm_total_value_added_sec;
        if total_lead_sec > 1e-6 {
            report.vsm_pce_percent = (report.vsm_total_value_added_sec / total_lead_sec) * 100.0;
        }
        if let Some(pm) = doc.page_mut(page_id) {
            for sc_id in scorecard_ids {
                if let Some(sc) = pm.find_shape_mut(sc_id) {
                    sc.text = format!(
                        "VSM Summary\nLead Time: {:.2} d\nValue-Added: {:.1} s\nPCE: {:.4}%\nBottleneck: {}",
                        report.vsm_total_lead_time_days,
                        report.vsm_total_value_added_sec,
                        report.vsm_pce_percent,
                        report.vsm_bottleneck_step
                    );
                    sc.sheet.set_shape_data(ShapeDataRow {
                        name: "TotalLeadTimeDays".into(),
                        label: "Lead Time (d)".into(),
                        prompt: String::new(),
                        data_type: ShapeDataType::Number,
                        format: "0.00".into(),
                        value: format_clean_f64(report.vsm_total_lead_time_days),
                        sort_key: "1".into(),
                        invisible: false,
                        ask: false,
                        lang_id: 1033,
                    });
                    sc.sheet.set_shape_data(ShapeDataRow {
                        name: "TotalValueAddedSec".into(),
                        label: "Value-Added (s)".into(),
                        prompt: String::new(),
                        data_type: ShapeDataType::Number,
                        format: "0.0".into(),
                        value: format_clean_f64(report.vsm_total_value_added_sec),
                        sort_key: "2".into(),
                        invisible: false,
                        ask: false,
                        lang_id: 1033,
                    });
                    sc.sheet.set_shape_data(ShapeDataRow {
                        name: "PcePercent".into(),
                        label: "PCE (%)".into(),
                        prompt: String::new(),
                        data_type: ShapeDataType::Number,
                        format: "0.0000".into(),
                        value: format!("{:.6}", report.vsm_pce_percent),
                        sort_key: "3".into(),
                        invisible: false,
                        ask: false,
                        lang_id: 1033,
                    });
                    sc.sheet.sync_all_sections_to_cells();
                }
            }
        }
    }

    // --- 2. Fault Tree Analysis (FTA) Exact Evaluation, MOCUS & Birnbaum Importance ---
    let mut gate_types: BTreeMap<u64, String> = BTreeMap::new();
    let mut fta_keys: BTreeMap<u64, String> = BTreeMap::new();
    let mut fta_labels: BTreeMap<u64, String> = BTreeMap::new();
    let mut basic_probs: BTreeMap<u64, f64> = BTreeMap::new();
    let mut children_by_gate: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    let mut referenced_as_child: BTreeSet<u64> = BTreeSet::new();

    if let Some(p) = doc.page(page_id) {
        for sh in &p.shapes {
            let role = user_cell_str(sh, "DomainRole").unwrap_or("");
            if role != "fta_gate" && role != "fta_basic" {
                continue;
            }
            let gtype = user_cell_str(sh, "FtaGateType")
                .map(str::to_ascii_uppercase)
                .unwrap_or_else(|| if role == "fta_basic" { "BASIC".into() } else { "OR".into() });
            let key = user_cell_str(sh, "FtaKey")
                .map(String::from)
                .unwrap_or_else(|| format!("E{}", sh.id));
            let lbl = user_cell_str(sh, "FtaLabel")
                .map(String::from)
                .unwrap_or_else(|| sh.name.clone());
            fta_keys.insert(sh.id, key);
            fta_labels.insert(sh.id, lbl);
            if role == "fta_basic" || gtype == "BASIC" {
                report.fta_basic_event_count += 1;
                let pr = sh
                    .sheet
                    .get_f64("Prop.Probability")
                    .unwrap_or(0.01)
                    .clamp(0.0, 1.0);
                basic_probs.insert(sh.id, pr);
                gate_types.insert(sh.id, "BASIC".into());
            } else {
                report.fta_gate_count += 1;
                gate_types.insert(sh.id, gtype);
            }
        }

        // Build adjacency from page.connects (`BeginX` = parent gate, `EndX` = child event/gate)
        let mut begin_map: BTreeMap<u64, u64> = BTreeMap::new();
        let mut end_map: BTreeMap<u64, u64> = BTreeMap::new();
        for c in &p.connects {
            if c.from_cell == "BeginX" {
                begin_map.insert(c.connector_shape_id, c.target_shape_id);
            } else if c.from_cell == "EndX" {
                end_map.insert(c.connector_shape_id, c.target_shape_id);
            }
        }
        for (conn_id, &src) in &begin_map {
            if let Some(&dst) = end_map.get(conn_id) {
                if gate_types.contains_key(&src) && gate_types.contains_key(&dst) && src != dst {
                    children_by_gate.entry(src).or_default().push(dst);
                    referenced_as_child.insert(dst);
                }
            }
        }
    }

    if !gate_types.is_empty() {
        let top_id = gate_types
            .keys()
            .copied()
            .find(|id| !referenced_as_child.contains(id) && gate_types.get(id).is_some_and(|g| g != "BASIC"))
            .or_else(|| gate_types.keys().copied().next())
            .unwrap_or(0);
        report.fta_top_event_id = top_id;

        fn eval_tree(
            node_id: u64,
            gate_types: &BTreeMap<u64, String>,
            children_by_gate: &BTreeMap<u64, Vec<u64>>,
            basic_override: &BTreeMap<u64, f64>,
            memo: &mut BTreeMap<u64, f64>,
            visiting: &mut BTreeSet<u64>,
        ) -> f64 {
            if let Some(&cached) = memo.get(&node_id) {
                return cached;
            }
            if !visiting.insert(node_id) {
                return 0.0;
            }
            let gtype = gate_types.get(&node_id).map(String::as_str).unwrap_or("BASIC");
            let val = if gtype == "BASIC" {
                basic_override.get(&node_id).copied().unwrap_or(0.0)
            } else {
                let cprobs: Vec<f64> = children_by_gate
                    .get(&node_id)
                    .map(|kids| {
                        kids.iter()
                            .map(|&cid| {
                                eval_tree(
                                    cid,
                                    gate_types,
                                    children_by_gate,
                                    basic_override,
                                    memo,
                                    visiting,
                                )
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                eval_fta_gate_prob(gtype, &cprobs)
            };
            visiting.remove(&node_id);
            memo.insert(node_id, val);
            val
        }

        let mut memo = BTreeMap::new();
        let mut vis = BTreeSet::new();
        report.fta_top_probability =
            eval_tree(top_id, &gate_types, &children_by_gate, &basic_probs, &mut memo, &mut vis);

        // Update gate text labels & k/n voting cells on page
        if let Some(pm) = doc.page_mut(page_id) {
            for (&nid, &prob) in &memo {
                if let Some(sh) = pm.find_shape_mut(nid) {
                    let key = fta_keys.get(&nid).cloned().unwrap_or_default();
                    let lbl = fta_labels.get(&nid).cloned().unwrap_or_default();
                    let gt = gate_types.get(&nid).cloned().unwrap_or_default();
                    sh.text = if gt == "BASIC" {
                        format!("{key}: {lbl}\np = {prob:.4}")
                    } else {
                        format!("[{gt}] {lbl}\nP = {prob:.6}")
                    };
                    if parse_voting_k_of_n(&gt, 3).is_some() {
                        sh.sheet.set_shape_data(ShapeDataRow {
                            name: "Probability".into(),
                            label: "Probability".into(),
                            prompt: "Voting Gate Probability".into(),
                            data_type: ShapeDataType::Number,
                            format: "0.000000".into(),
                            value: format!("{prob:.6}"),
                            sort_key: "1".into(),
                            invisible: false,
                            ask: false,
                            lang_id: 1033,
                        });
                        sh.sheet.sync_all_sections_to_cells();
                    }
                }
            }
        }

        // Compute Birnbaum structural importance: P(Top | E_i = 1) - P(Top | E_i = 0)
        for &bid in basic_probs.keys() {
            let mut p1_map = basic_probs.clone();
            p1_map.insert(bid, 1.0);
            let p_when_1 = eval_tree(
                top_id,
                &gate_types,
                &children_by_gate,
                &p1_map,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            );
            let mut p0_map = basic_probs.clone();
            p0_map.insert(bid, 0.0);
            let p_when_0 = eval_tree(
                top_id,
                &gate_types,
                &children_by_gate,
                &p0_map,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            );
            let bkey = fta_keys.get(&bid).cloned().unwrap_or_else(|| format!("E{bid}"));
            report
                .fta_birnbaum_importance
                .insert(bkey, (p_when_1 - p_when_0).max(0.0));
        }

        // Compute MOCUS Minimal Cut Sets
        fn mocus_cut_sets(
            node_id: u64,
            gate_types: &BTreeMap<u64, String>,
            children_by_gate: &BTreeMap<u64, Vec<u64>>,
            fta_keys: &BTreeMap<u64, String>,
            visiting: &mut BTreeSet<u64>,
        ) -> Vec<BTreeSet<String>> {
            if !visiting.insert(node_id) {
                return Vec::new();
            }
            let gt = gate_types.get(&node_id).map(String::as_str).unwrap_or("BASIC");
            let kids = children_by_gate.get(&node_id).cloned().unwrap_or_default();
            let res = if gt == "BASIC" || kids.is_empty() {
                let mut s = BTreeSet::new();
                s.insert(fta_keys.get(&node_id).cloned().unwrap_or_else(|| format!("E{node_id}")));
                vec![s]
            } else if gt == "AND" {
                let mut prod: Vec<BTreeSet<String>> = vec![BTreeSet::new()];
                for cid in kids {
                    let sub = mocus_cut_sets(cid, gate_types, children_by_gate, fta_keys, visiting);
                    let mut next = Vec::new();
                    for a in &prod {
                        for b in &sub {
                            let mut merged = a.clone();
                            merged.extend(b.iter().cloned());
                            next.push(merged);
                        }
                    }
                    prod = next;
                }
                prod
            } else if let Some(k) = parse_voting_k_of_n(gt, kids.len()) {
                // All k-combinations of child cut sets
                let child_sets: Vec<Vec<BTreeSet<String>>> = kids
                    .iter()
                    .map(|&cid| mocus_cut_sets(cid, gate_types, children_by_gate, fta_keys, visiting))
                    .collect();
                let n = child_sets.len();
                let mut combos = Vec::new();
                for mask in 0usize..(1usize << n.min(12)) {
                    if mask.count_ones() as usize == k {
                        let mut prod = vec![BTreeSet::new()];
                        for (idx, cset) in child_sets.iter().enumerate() {
                            if (mask & (1 << idx)) != 0 {
                                let mut next = Vec::new();
                                for a in &prod {
                                    for b in cset {
                                        let mut m = a.clone();
                                        m.extend(b.iter().cloned());
                                        next.push(m);
                                    }
                                }
                                prod = next;
                            }
                        }
                        combos.extend(prod);
                    }
                }
                combos
            } else {
                // OR / XOR union of child cut sets
                let mut union_vec = Vec::new();
                for cid in kids {
                    union_vec.extend(mocus_cut_sets(
                        cid,
                        gate_types,
                        children_by_gate,
                        fta_keys,
                        visiting,
                    ));
                }
                union_vec
            };
            visiting.remove(&node_id);

            // Boolean absorption: remove supersets & duplicates
            let mut minimal: Vec<BTreeSet<String>> = Vec::new();
            let mut sorted = res;
            sorted.sort_by_key(|s| s.len());
            for cand in sorted {
                if !minimal.iter().any(|m| m.is_subset(&cand)) {
                    minimal.push(cand);
                }
            }
            minimal
        }

        let cut_sets = mocus_cut_sets(
            top_id,
            &gate_types,
            &children_by_gate,
            &fta_keys,
            &mut BTreeSet::new(),
        );
        report.fta_minimal_cut_sets = cut_sets
            .into_iter()
            .map(|s| s.into_iter().collect::<Vec<_>>())
            .collect();
    }

    ReactiveEngine::recalculate_document(doc);
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associative_dimensions_update_reactively_when_target_shape_resizes() {
        let mut doc = VisioDocument::new_blank("AEC Dimensions");
        let room_id = crate::domains::create_architectural_room_space(
            &mut doc,
            1,
            "Conference",
            4.0,
            4.0,
            3.0,
            2.0,
        )
        .unwrap();
        let dim_h = insert_associative_dimension(&mut doc, 1, "horizontal", room_id, 0, 0.5, "in").unwrap();
        let dim_v = insert_associative_dimension(&mut doc, 1, "vertical", room_id, 0, 0.5, "in").unwrap();

        let dh_text_1 = doc.page(1).unwrap().find_shape(dim_h).unwrap().text.clone();
        assert!(dh_text_1.contains("3.00"), "Expected 3.00 in, got {dh_text_1}");

        // Resize target room from 3.0 in -> 5.25 in: associative dimension MUST update automatically!
        {
            let room = doc.page_mut(1).unwrap().find_shape_mut(room_id).unwrap();
            room.sheet.set_formula("Width", "5.25 in", Some(Unit::Inch));
            room.sheet.set_formula("Height", "3.50 in", Some(Unit::Inch));
        }
        ReactiveEngine::recalculate_document(&mut doc);
        let dh_text_2 = doc.page(1).unwrap().find_shape(dim_h).unwrap().text.clone();
        let dv_text_2 = doc.page(1).unwrap().find_shape(dim_v).unwrap().text.clone();
        assert!(dh_text_2.contains("5.25"), "Expected 5.25 in after resize, got {dh_text_2}");
        assert!(dv_text_2.contains("3.50"), "Expected 3.50 in after resize, got {dv_text_2}");
    }

    #[test]
    fn wall_miter_t_join_and_door_window_openings_split_geometry_cleanly() {
        let mut doc = VisioDocument::new_blank("Walls & Openings");
        // L-corner at (2.0, 2.0) + T-join partition at x = 4.5
        let w1 = draw_architectural_wall(&mut doc, 1, 2.0, 2.0, 7.0, 2.0, 0.30).unwrap();
        let _w2 = draw_architectural_wall(&mut doc, 1, 2.0, 2.0, 2.0, 6.0, 0.30).unwrap();
        let _w3 = draw_architectural_wall(&mut doc, 1, 4.5, 2.05, 4.5, 5.5, 0.24).unwrap();
        let joints = heal_wall_intersections(&mut doc, 1, 0.30);
        assert!(joints >= 2, "Expected L-corner miter + T-join, got {joints}");

        // Cut a single door and a window into wall w1 -> w1 must split into 3 solid Geometry sections!
        let door_id = insert_wall_opening(&mut doc, 1, w1, "single_door", 1.5, 0.80, "in_left").unwrap();
        let win_id = insert_wall_opening(&mut doc, 1, w1, "window", 3.6, 0.90, "in_left").unwrap();
        assert_ne!(door_id, win_id);
        let wall_sh = doc.page(1).unwrap().find_shape(w1).unwrap();
        assert_eq!(wall_sh.sheet.geometries.len(), 3, "Wall with 2 openings must split into 3 solid segments");
    }

    #[test]
    fn calendar_pivot_mindmap_vsm_and_fta_calculators_are_exact() {
        // 1. Gregorian weekday check: 2026-11-01 is Sunday (0); 2024-02 has 29 days
        assert_eq!(weekday_sunday0(2026, 11, 1), 0);
        assert_eq!(days_in_gregorian_month(2024, 2), 29);

        let mut doc = VisioDocument::new_blank("M4 Suite");
        let cal_n = build_calendar_diagram(
            &mut doc,
            1,
            "2026,11,monday\n2026-11-27|Ship v0.9.0-beta|blue",
        );
        assert_eq!(cal_n, 1 + 7 + 30);

        // 2. PivotDiagram on Page 2
        doc.pages.push(vexio_shapesheet::VisioPage::new(2, "Pivot", 11.0, 8.5));
        let pivot_n = build_pivot_diagram(
            &mut doc,
            2,
            "# GROUP=Region,Tier;METRIC=ARR;AGG=SUM\nRegion,Tier,ARR\nEMEA,Enterprise,120\nEMEA,SMB,30\nAMER,Enterprise,200",
        );
        assert_eq!(pivot_n, 1 + 2 + 3);

        // 3. Mind Map + Add Topic + Lossless Markdown Outline Export on Page 3
        doc.pages.push(vexio_shapesheet::VisioPage::new(3, "MindMap", 11.0, 8.5));
        let mm_n = build_mind_map_from_outline(
            &mut doc,
            3,
            "# Vexio 1.0\n- AEC Architecture\n  - Walls & Doors\n- Plan 2 Analytics",
            "balanced",
        );
        assert_eq!(mm_n, 4);
        let added = add_mind_map_topic(&mut doc, 3, 0, "Fault Tree FTA", false).unwrap();
        assert!(added > 0);
        let md = export_mind_map_outline(&doc, 3);
        assert!(md.contains("# Vexio 1.0") && md.contains("- Walls & Doors") && md.contains("- Fault Tree FTA"));

        // 4. Lean VSM + Fault Tree FTA on Page 4 & 5
        doc.pages.push(vexio_shapesheet::VisioPage::new(4, "VSM", 11.0, 8.5));
        let vsm_n = build_vsm_diagram(
            &mut doc,
            4,
            "Stamping,10,2.0,98,1\nWelding,40,3.0,92,2\nAssembly,50,5.0,96,2",
        );
        assert!(vsm_n >= 8);
        let vsm_rep = recalculate_vsm_and_fta(&mut doc, 4);
        assert_eq!(vsm_rep.vsm_step_count, 3);
        assert!((vsm_rep.vsm_total_lead_time_days - 10.0).abs() < 1e-6);
        assert!((vsm_rep.vsm_total_value_added_sec - 100.0).abs() < 1e-6);
        assert_eq!(vsm_rep.vsm_bottleneck_step, "Assembly");

        doc.pages.push(vexio_shapesheet::VisioPage::new(5, "FTA", 11.0, 8.5));
        let fta_n = build_fault_tree_diagram(
            &mut doc,
            5,
            "TOP,System Outage,OR,0,G1;G2\nG1,Dual Power Loss,AND,0,B1;B2\nG2,2-of-3 Sensor Fault,2/3,0,B3;B4;B5\nB1,Grid A,BASIC,0.10,\nB2,Gen B,BASIC,0.20,\nB3,Sens 1,BASIC,0.10,\nB4,Sens 2,BASIC,0.10,\nB5,Sens 3,BASIC,0.10,",
        );
        assert_eq!(fta_n, 8);
        let fta_rep = recalculate_vsm_and_fta(&mut doc, 5);
        // P(G1) = 0.1 * 0.2 = 0.02
        // P(G2) = 3 * (0.1^2 * 0.9) + 0.1^3 = 0.027 + 0.001 = 0.028
        // P(TOP) = 1 - (1 - 0.02)*(1 - 0.028) = 1 - 0.98 * 0.972 = 0.04744
        assert!((fta_rep.fta_top_probability - 0.04744).abs() < 1e-6, "got {}", fta_rep.fta_top_probability);
        // Minimal cut sets: {B1,B2}, {B3,B4}, {B3,B5}, {B4,B5} -> 4 minimal cut sets!
        assert_eq!(fta_rep.fta_minimal_cut_sets.len(), 4);
    }
}
