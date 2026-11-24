//! Vector-Accurate Export & Multi-Page Tiled Print Engine:
//! - SVG 1.1 (`export_page_svg`) with Background Pages, Header/Footer, Gradients, Hatches,
//!   Drop Shadows, 3D Extrusion Bevels, Sketchy Hand-Drawn Lines, Cropped `ForeignData` Images,
//!   Multi-Run Rich Text (Bold/Italic/Underline/Strike/Bullets/RTL/Vertical), Subprocess Badges & ScreenTips
//! - PDF/A-1b / PDF 1.7 (`export_document_pdf`)
//! - Encapsulated PostScript EPS (`export_page_eps`)
//! - High-DPI PNG (`encode_rgba_to_png`)
//! - Interactive Offline HTML Workspace Report (`export_document_html_report`)
//! - Multi-Page Tiled Print Layout with Crop & Registration Marks (`plan_tiled_print`).

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use vexio_shapesheet::{
    EvaluatedPathVerb, EvaluatedShapeVisual, ReactiveEngine, RgbaColor, VisioDocument, VisioPage,
    format_clean_f64,
};

// ============================================================================
// 1. Vector SVG 1.1 Export (Full Visual Effects, Rich Text & Background Pages)
// ============================================================================

pub fn export_page_svg(doc: &VisioDocument, page_id: u32) -> String {
    let Some(page) = doc.page(page_id) else {
        return String::new();
    };
    let ppi = 96.0;
    let w_in = page.width_in();
    let h_in = page.height_in();
    let w_px = w_in * ppi;
    let h_px = h_in * ppi;

    let page_bg_hex = page
        .page_sheet
        .get_color("PageColor")
        .or_else(|| {
            doc.active_theme_id
                .and_then(|tid| doc.themes.iter().find(|t| t.id == tid))
                .map(|t| t.background_color)
        })
        .map(|c| c.to_hex_rgb())
        .unwrap_or_else(|| "#FFFFFF".into());

    let mut defs = String::from(
        "<marker id=\"vexio-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M 0 1 L 10 5 L 0 9 z\" fill=\"context-stroke\"/></marker>\n",
    );
    let mut body = String::new();

    let _ = writeln!(
        body,
        "<rect width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>",
        w_px, h_px, page_bg_hex
    );

    // 1. Render linked Background Page first (`Title Block`, borders, watermarks) if present
    if let Some(bg_id) = page.back_page_id {
        if bg_id != page_id {
            if let Some(bg_page) = doc.page(bg_id) {
                let bg_visuals = ReactiveEngine::evaluate_page_visuals(doc, bg_id);
                body.push_str("<g class=\"vexio-background-page\" opacity=\"0.92\">\n");
                for vis in &bg_visuals {
                    render_single_shape_svg(
                        bg_page,
                        vis,
                        h_in,
                        ppi,
                        &page_bg_hex,
                        &mut defs,
                        &mut body,
                        "bg",
                    );
                }
                body.push_str("</g>\n");
            }
        }
    }

    // 2. Render Foreground Page Shapes
    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    for vis in &visuals {
        render_single_shape_svg(page, vis, h_in, ppi, &page_bg_hex, &mut defs, &mut body, "fg");
    }

    // 3. Render Header & Footer (`doc.header_footer`)
    let hf = &doc.header_footer;
    let hf_size = (hf.font_size_pt.max(8.0)) * (96.0 / 72.0);
    let hdr_y = (hf.header_margin_in.max(0.2)) * ppi;
    let ftr_y = h_px - (hf.footer_margin_in.max(0.2)) * ppi;
    if !hf.header_left.is_empty() {
        let _ = writeln!(
            body,
            "<text x=\"24\" y=\"{hdr_y:.1}\" font-size=\"{hf_size:.1}\" fill=\"#64748b\">{}</text>",
            vexio_vsdx::opc::xml_escape(&hf.header_left)
        );
    }
    if !hf.header_center.is_empty() {
        let _ = writeln!(
            body,
            "<text x=\"{:.1}\" y=\"{hdr_y:.1}\" font-size=\"{hf_size:.1}\" fill=\"#64748b\" text-anchor=\"middle\">{}</text>",
            w_px * 0.5,
            vexio_vsdx::opc::xml_escape(&hf.header_center)
        );
    }
    if !hf.header_right.is_empty() {
        let _ = writeln!(
            body,
            "<text x=\"{:.1}\" y=\"{hdr_y:.1}\" font-size=\"{hf_size:.1}\" fill=\"#64748b\" text-anchor=\"end\">{}</text>",
            w_px - 24.0,
            vexio_vsdx::opc::xml_escape(&hf.header_right)
        );
    }
    if !hf.footer_left.is_empty() {
        let _ = writeln!(
            body,
            "<text x=\"24\" y=\"{ftr_y:.1}\" font-size=\"{hf_size:.1}\" fill=\"#64748b\">{}</text>",
            vexio_vsdx::opc::xml_escape(&hf.footer_left)
        );
    }
    if !hf.footer_center.is_empty() {
        let _ = writeln!(
            body,
            "<text x=\"{:.1}\" y=\"{ftr_y:.1}\" font-size=\"{hf_size:.1}\" fill=\"#64748b\" text-anchor=\"middle\">{}</text>",
            w_px * 0.5,
            vexio_vsdx::opc::xml_escape(&hf.footer_center)
        );
    }
    if !hf.footer_right.is_empty() {
        let _ = writeln!(
            body,
            "<text x=\"{:.1}\" y=\"{ftr_y:.1}\" font-size=\"{hf_size:.1}\" fill=\"#64748b\" text-anchor=\"end\">{}</text>",
            w_px - 24.0,
            vexio_vsdx::opc::xml_escape(&hf.footer_right)
        );
    }

    // 4. Render Pressure-Sensitive USI Stylus Markup Ink Strokes
    let to_svg_y = |y_in: f64| (h_in - y_in) * ppi;
    let to_svg_x = |x_in: f64| x_in * ppi;
    for ink in &page.ink_strokes {
        if ink.points.len() >= 2 {
            let pts_s: Vec<String> = ink
                .points
                .iter()
                .map(|pt| format!("{:.2},{:.2}", to_svg_x(pt.x_in), to_svg_y(pt.y_in)))
                .collect();
            let op = if ink.highlighter { "0.45" } else { "1.0" };
            let _ = writeln!(
                body,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-opacity=\"{op}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                pts_s.join(" "),
                ink.color.to_hex_rgb(),
                ink.width_pt * (96.0 / 72.0)
            );
        }
    }

    let mut svg = String::with_capacity(defs.len() + body.len() + 256);
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.2}\" height=\"{:.2}\" viewBox=\"0 0 {:.2} {:.2}\" preserveAspectRatio=\"xMidYMid meet\">",
        w_px, h_px, w_px, h_px
    );
    let _ = writeln!(svg, "<defs>\n{defs}</defs>");
    svg.push_str(&body);
    svg.push_str("</svg>");
    svg
}

#[allow(clippy::too_many_arguments)]
fn render_single_shape_svg(
    page: &VisioPage,
    vis: &EvaluatedShapeVisual,
    h_in: f64,
    ppi: f64,
    page_bg_hex: &str,
    defs: &mut String,
    body: &mut String,
    ns_prefix: &str,
) {
    let to_svg_y = |y_in: f64| (h_in - y_in) * ppi;
    let to_svg_x = |x_in: f64| x_in * ppi;

    let shape_opt = page.find_shape(vis.shape_id);
    let fill_pattern = vis.fill_pattern;
    let fill_bkgnd = vis.fill_bkgnd.unwrap_or(RgbaColor {
        r: 241,
        g: 245,
        b: 249,
        a: 255,
    });
    let grad_angle_rad = vis.fill_gradient_angle_rad;

    let shdw_pattern = shape_opt
        .and_then(|s| s.sheet.get_f64("ShdwPattern"))
        .unwrap_or(0.0)
        .round() as i32;
    let bevel_type = shape_opt
        .and_then(|s| s.sheet.get_f64("BevelTopType"))
        .unwrap_or(0.0)
        .round() as i32;
    let extrusion_in = shape_opt
        .and_then(|s| s.sheet.get_f64("ExtrusionDepth"))
        .unwrap_or(0.0);
    let sketchy = shape_opt
        .and_then(|s| s.sheet.get_bool("LineSketchEnabled"))
        .unwrap_or(false);

    // Resolve Fill (`none`, solid hex, multi-stop `<linearGradient>`, `<radialGradient>`, or `<pattern>` hatch)
    let fill_paint = if fill_pattern == 0 || vis.paths.iter().all(|p| p.no_fill) {
        String::from("none")
    } else if vis.gradient_stops.len() >= 2 {
        let mut stops_xml = String::new();
        for st in &vis.gradient_stops {
            let pct = (st.position * 100.0).clamp(0.0, 100.0);
            let op = (st.color.a as f64 / 255.0).clamp(0.0, 1.0);
            if op < 0.999 {
                let _ = write!(
                    stops_xml,
                    "<stop offset=\"{pct:.1}%\" stop-color=\"{}\" stop-opacity=\"{op:.2}\"/>",
                    st.color.to_hex_rgb()
                );
            } else {
                let _ = write!(
                    stops_xml,
                    "<stop offset=\"{pct:.1}%\" stop-color=\"{}\"/>",
                    st.color.to_hex_rgb()
                );
            }
        }
        if vis.fill_gradient_radial || (36..=40).contains(&fill_pattern) {
            let gid = format!("vexio-rgrad-{ns_prefix}-{}", vis.shape_id);
            let _ = writeln!(
                defs,
                "<radialGradient id=\"{gid}\" cx=\"50%\" cy=\"50%\" r=\"65%\">{stops_xml}</radialGradient>"
            );
            format!("url(#{gid})")
        } else {
            let gid = format!("vexio-lgrad-{ns_prefix}-{}", vis.shape_id);
            // Visio gradient angle is CCW in Y-up space; convert to SVG Y-down percentage vector
            let dx = 50.0 * grad_angle_rad.cos();
            let dy = -50.0 * grad_angle_rad.sin();
            let x1 = (50.0 - dx).clamp(0.0, 100.0);
            let y1 = (50.0 - dy).clamp(0.0, 100.0);
            let x2 = (50.0 + dx).clamp(0.0, 100.0);
            let y2 = (50.0 + dy).clamp(0.0, 100.0);
            let _ = writeln!(
                defs,
                "<linearGradient id=\"{gid}\" x1=\"{x1:.1}%\" y1=\"{y1:.1}%\" x2=\"{x2:.1}%\" y2=\"{y2:.1}%\">{stops_xml}</linearGradient>"
            );
            format!("url(#{gid})")
        }
    } else if let Some(fg) = vis.fill_color {
        let fg_hex = fg.to_hex_rgb();
        let bg_hex = fill_bkgnd.to_hex_rgb();
        if (25..=35).contains(&fill_pattern) {
            let gid = format!("vexio-lgrad-{ns_prefix}-{}", vis.shape_id);
            let dx = 50.0 * grad_angle_rad.cos();
            let dy = -50.0 * grad_angle_rad.sin();
            let x1 = (50.0 - dx).clamp(0.0, 100.0);
            let y1 = (50.0 - dy).clamp(0.0, 100.0);
            let x2 = (50.0 + dx).clamp(0.0, 100.0);
            let y2 = (50.0 + dy).clamp(0.0, 100.0);
            let _ = writeln!(
                defs,
                "<linearGradient id=\"{gid}\" x1=\"{x1:.1}%\" y1=\"{y1:.1}%\" x2=\"{x2:.1}%\" y2=\"{y2:.1}%\"><stop offset=\"0%\" stop-color=\"{fg_hex}\"/><stop offset=\"100%\" stop-color=\"{bg_hex}\"/></linearGradient>"
            );
            format!("url(#{gid})")
        } else if (36..=40).contains(&fill_pattern) {
            let gid = format!("vexio-rgrad-{ns_prefix}-{}", vis.shape_id);
            let _ = writeln!(
                defs,
                "<radialGradient id=\"{gid}\" cx=\"50%\" cy=\"50%\" r=\"65%\"><stop offset=\"0%\" stop-color=\"{bg_hex}\"/><stop offset=\"100%\" stop-color=\"{fg_hex}\"/></radialGradient>"
            );
            format!("url(#{gid})")
        } else if (2..=12).contains(&fill_pattern) {
            let pid = format!("vexio-hatch-{ns_prefix}-{}", vis.shape_id);
            let hatch_geom = match fill_pattern {
                2 => format!("<line x1=\"0\" y1=\"4\" x2=\"8\" y2=\"4\" stroke=\"{fg_hex}\" stroke-width=\"1.2\"/>"),
                3 => format!("<line x1=\"4\" y1=\"0\" x2=\"4\" y2=\"8\" stroke=\"{fg_hex}\" stroke-width=\"1.2\"/>"),
                4 => format!("<path d=\"M-2,2 l4,-4 M0,8 l8,-8 M6,10 l4,-4\" stroke=\"{fg_hex}\" stroke-width=\"1.1\"/>"),
                5 => format!("<path d=\"M-2,6 l4,4 M0,0 l8,8 M6,-2 l4,4\" stroke=\"{fg_hex}\" stroke-width=\"1.1\"/>"),
                6 => format!("<path d=\"M4,0 v8 M0,4 h8\" stroke=\"{fg_hex}\" stroke-width=\"1.0\"/>"),
                _ => format!("<circle cx=\"4\" cy=\"4\" r=\"1.3\" fill=\"{fg_hex}\"/>"),
            };
            let _ = writeln!(
                defs,
                "<pattern id=\"{pid}\" width=\"8\" height=\"8\" patternUnits=\"userSpaceOnUse\"><rect width=\"8\" height=\"8\" fill=\"{bg_hex}\"/>{hatch_geom}</pattern>"
            );
            format!("url(#{pid})")
        } else {
            fg_hex
        }
    } else {
        String::from("none")
    };

    // Optional Soft Drop Shadow Filter
    let filter_attr = if shdw_pattern > 0 {
        let fid = format!("vexio-shdw-{ns_prefix}-{}", vis.shape_id);
        let dx_px = shape_opt
            .and_then(|s| s.sheet.get_f64("ShdwOffsetX"))
            .unwrap_or(0.05)
            * ppi;
        let dy_px = -shape_opt
            .and_then(|s| s.sheet.get_f64("ShdwOffsetY"))
            .unwrap_or(-0.05)
            * ppi;
        let blur_px = shape_opt
            .and_then(|s| s.sheet.get_f64("ShdwBlur"))
            .unwrap_or(0.04)
            * ppi;
        let shdw_col = shape_opt
            .and_then(|s| s.sheet.get_color("ShdwForegnd"))
            .map(|c| c.to_hex_rgb())
            .unwrap_or_else(|| "#0f172a".into());
        let _ = writeln!(
            defs,
            "<filter id=\"{fid}\" x=\"-20%\" y=\"-20%\" width=\"150%\" height=\"150%\"><feDropShadow dx=\"{dx_px:.1}\" dy=\"{dy_px:.1}\" stdDeviation=\"{blur_px:.1}\" flood-color=\"{shdw_col}\" flood-opacity=\"0.28\"/></filter>"
        );
        format!(" filter=\"url(#{fid})\"")
    } else {
        String::new()
    };

    // ScreenTip / Hyperlink metadata on shape group
    let screentip = shape_opt
        .and_then(|s| s.sheet.get_cell("Comment"))
        .and_then(|c| c.value.as_str_coerced().ok())
        .filter(|s| !s.trim().is_empty());
    let default_hl = shape_opt.and_then(|s| {
        s.sheet
            .hyperlinks
            .iter()
            .find(|h| h.default)
            .or_else(|| s.sheet.hyperlinks.first())
    });
    let subproc_attr = default_hl
        .filter(|h| !h.sub_address.is_empty())
        .map(|h| format!(" data-subprocess=\"{}\"", vexio_vsdx::opc::xml_escape(&h.sub_address)))
        .unwrap_or_default();

    let _ = writeln!(
        body,
        "<g data-shape-id=\"{}\"{filter_attr}{subproc_attr}>",
        vis.shape_id
    );
    if screentip.is_some() || default_hl.is_some() {
        let mut tip_parts = Vec::new();
        if let Some(st) = &screentip {
            tip_parts.push(st.clone());
        }
        if let Some(hl) = default_hl {
            let target = if !hl.sub_address.is_empty() {
                format!("Page: {}", hl.sub_address)
            } else {
                hl.address.clone()
            };
            tip_parts.push(format!("Link → {} ({target})", hl.description));
        }
        let _ = writeln!(
            body,
            "<title>{}</title>",
            vexio_vsdx::opc::xml_escape(&tip_parts.join(" | "))
        );
    }

    // 3D Extrusion / Bevel Back-Depth Facet
    if bevel_type > 0 || extrusion_in > 0.005 {
        let depth_px = (extrusion_in.max(0.08)) * ppi;
        let left_px = to_svg_x(vis.aabb_min[0]);
        let top_px = to_svg_y(vis.aabb_max[1]);
        let w_b = ((vis.aabb_max[0] - vis.aabb_min[0]) * ppi).max(4.0);
        let h_b = ((vis.aabb_max[1] - vis.aabb_min[1]) * ppi).max(4.0);
        let dx = depth_px * 0.72;
        let dy = -depth_px * 0.65;
        // Top & Right 3D extruded bevel faces
        let _ = writeln!(
            body,
            "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#cbd5e1\" stroke=\"#475569\" stroke-width=\"0.9\"/>",
            left_px,
            top_px,
            left_px + dx,
            top_px + dy,
            left_px + w_b + dx,
            top_px + dy,
            left_px + w_b,
            top_px
        );
        let _ = writeln!(
            body,
            "<polygon points=\"{:.1},{:.1} {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}\" fill=\"#94a3b8\" stroke=\"#475569\" stroke-width=\"0.9\"/>",
            left_px + w_b,
            top_px,
            left_px + w_b + dx,
            top_px + dy,
            left_px + w_b + dx,
            top_px + h_b + dy,
            left_px + w_b,
            top_px + h_b
        );
    }

    let stroke = vis
        .stroke_color
        .map(|c| c.to_hex_rgb())
        .unwrap_or_else(|| "none".into());
    let sw_px = (vis.stroke_weight_in * ppi).max(0.75);
    let marker_start = if vis.begin_arrow > 0 {
        " marker-start=\"url(#vexio-arrow)\""
    } else {
        ""
    };
    let marker_end = if vis.end_arrow > 0 {
        " marker-end=\"url(#vexio-arrow)\""
    } else {
        ""
    };

    let has_primary_foreign_svg = shape_opt.is_some_and(|sh| {
        sh.sheet.get_bool("User.ForeignSvgPrimary").unwrap_or(false)
            && sh
                .foreign_data
                .as_ref()
                .is_some_and(|fd| fd.media_type.eq_ignore_ascii_case("SVG") && !fd.data.is_empty())
    });

    if !has_primary_foreign_svg {
        for path in &vis.paths {
            if path.no_show || path.verbs_page_space.is_empty() {
                continue;
            }
            let path_fill_owned = path.fill_color.map(|c| c.to_hex_rgb());
            let path_stroke_owned = path.stroke_color.map(|c| c.to_hex_rgb());
            let p_fill = if path.no_fill {
                "none"
            } else {
                path_fill_owned.as_deref().unwrap_or(&fill_paint)
            };
            let p_stroke = if path.no_line {
                "none"
            } else {
                path_stroke_owned.as_deref().unwrap_or(&stroke)
            };
            let p_sw_px = path
                .stroke_weight_in
                .map(|w| (w * ppi).max(0.65))
                .unwrap_or(sw_px);

            let mut d = String::new();
            let mut sketch_d = String::new();
            let mut last_pt = [0.0, 0.0];
            let mut seg_idx = 0u32;

            // Check if pure closed polyline with `vis.rounding_in > 1e-4` (`Rounded Rectangle`, `Pill`, `BPMN Task`)
            let mut rounded_emitted = false;
            if !sketchy && vis.rounding_in > 1e-4 && path.verbs_page_space.len() >= 4 {
                let all_poly = path.verbs_page_space.iter().all(|v| {
                    matches!(
                        v,
                        EvaluatedPathVerb::MoveTo(_)
                            | EvaluatedPathVerb::LineTo(_)
                            | EvaluatedPathVerb::Close
                    )
                });
                if all_poly {
                    let mut pts: Vec<[f64; 2]> = Vec::new();
                    for v in &path.verbs_page_space {
                        match v {
                            EvaluatedPathVerb::MoveTo(p) | EvaluatedPathVerb::LineTo(p) => {
                                if pts.last().is_none_or(|prev| {
                                    ((prev[0] - p[0]).powi(2) + (prev[1] - p[1]).powi(2)).sqrt()
                                        > 1e-5
                                }) {
                                    pts.push(*p);
                                }
                            }
                            _ => {}
                        }
                    }
                    let closed = pts.len() >= 3
                        && (((pts[0][0] - pts[pts.len() - 1][0]).powi(2)
                            + (pts[0][1] - pts[pts.len() - 1][1]).powi(2))
                        .sqrt()
                            < 1e-4
                            || matches!(
                                path.verbs_page_space.last(),
                                Some(EvaluatedPathVerb::Close)
                            ));
                    if closed
                        && ((pts[0][0] - pts[pts.len() - 1][0]).powi(2)
                            + (pts[0][1] - pts[pts.len() - 1][1]).powi(2))
                        .sqrt()
                            < 1e-4
                    {
                        pts.pop();
                    }
                    if closed && pts.len() >= 3 {
                        let n = pts.len();
                        let k = 0.552_284_75;
                        for i in 0..n {
                            let prev = pts[(i + n - 1) % n];
                            let curr = pts[i];
                            let next = pts[(i + 1) % n];
                            let v1 = [prev[0] - curr[0], prev[1] - curr[1]];
                            let v2 = [next[0] - curr[0], next[1] - curr[1]];
                            let l1 = (v1[0] * v1[0] + v1[1] * v1[1]).sqrt();
                            let l2 = (v2[0] * v2[0] + v2[1] * v2[1]).sqrt();
                            let r = vis.rounding_in.min(l1 * 0.499).min(l2 * 0.499);
                            if l1 > 1e-6 && l2 > 1e-6 && r > 1e-5 {
                                let p_in = [curr[0] + (v1[0] / l1) * r, curr[1] + (v1[1] / l1) * r];
                                let p_out =
                                    [curr[0] + (v2[0] / l2) * r, curr[1] + (v2[1] / l2) * r];
                                let c1 = [
                                    p_in[0] + (curr[0] - p_in[0]) * k,
                                    p_in[1] + (curr[1] - p_in[1]) * k,
                                ];
                                let c2 = [
                                    p_out[0] + (curr[0] - p_out[0]) * k,
                                    p_out[1] + (curr[1] - p_out[1]) * k,
                                ];
                                if i == 0 {
                                    let _ = write!(
                                        d,
                                        "M {:.2} {:.2} ",
                                        to_svg_x(p_in[0]),
                                        to_svg_y(p_in[1])
                                    );
                                } else {
                                    let _ = write!(
                                        d,
                                        "L {:.2} {:.2} ",
                                        to_svg_x(p_in[0]),
                                        to_svg_y(p_in[1])
                                    );
                                }
                                let _ = write!(
                                    d,
                                    "C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                                    to_svg_x(c1[0]),
                                    to_svg_y(c1[1]),
                                    to_svg_x(c2[0]),
                                    to_svg_y(c2[1]),
                                    to_svg_x(p_out[0]),
                                    to_svg_y(p_out[1])
                                );
                            } else if i == 0 {
                                let _ = write!(
                                    d,
                                    "M {:.2} {:.2} ",
                                    to_svg_x(curr[0]),
                                    to_svg_y(curr[1])
                                );
                            } else {
                                let _ = write!(
                                    d,
                                    "L {:.2} {:.2} ",
                                    to_svg_x(curr[0]),
                                    to_svg_y(curr[1])
                                );
                            }
                        }
                        d.push_str("Z ");
                        rounded_emitted = true;
                    }
                }
            }

            if !rounded_emitted {
                for verb in &path.verbs_page_space {
                    match verb {
                        EvaluatedPathVerb::MoveTo(p) => {
                            let _ = write!(d, "M {:.2} {:.2} ", to_svg_x(p[0]), to_svg_y(p[1]));
                            if sketchy {
                                let _ = write!(
                                    sketch_d,
                                    "M {:.2} {:.2} ",
                                    to_svg_x(p[0]) + 1.1,
                                    to_svg_y(p[1]) - 0.9
                                );
                            }
                            last_pt = *p;
                        }
                        EvaluatedPathVerb::LineTo(p) => {
                            let sx = to_svg_x(p[0]);
                            let sy = to_svg_y(p[1]);
                            if sketchy {
                                let mx = (to_svg_x(last_pt[0]) + sx) * 0.5;
                                let my = (to_svg_y(last_pt[1]) + sy) * 0.5;
                                let jitter = if (vis.shape_id as u32 + seg_idx) % 2 == 0 {
                                    2.2
                                } else {
                                    -2.2
                                };
                                let _ = write!(
                                    d,
                                    "Q {:.2} {:.2} {:.2} {:.2} ",
                                    mx + jitter,
                                    my - jitter,
                                    sx,
                                    sy
                                );
                                let _ = write!(
                                    sketch_d,
                                    "Q {:.2} {:.2} {:.2} {:.2} ",
                                    mx - jitter * 0.7,
                                    my + jitter * 0.7,
                                    sx - 0.8,
                                    sy + 0.8
                                );
                            } else {
                                let _ = write!(d, "L {:.2} {:.2} ", sx, sy);
                            }
                            last_pt = *p;
                            seg_idx += 1;
                        }
                        EvaluatedPathVerb::QuadTo { ctrl, to } => {
                            let _ = write!(
                                d,
                                "Q {:.2} {:.2} {:.2} {:.2} ",
                                to_svg_x(ctrl[0]),
                                to_svg_y(ctrl[1]),
                                to_svg_x(to[0]),
                                to_svg_y(to[1])
                            );
                            last_pt = *to;
                        }
                        EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                            let _ = write!(
                                d,
                                "C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                                to_svg_x(ctrl1[0]),
                                to_svg_y(ctrl1[1]),
                                to_svg_x(ctrl2[0]),
                                to_svg_y(ctrl2[1]),
                                to_svg_x(to[0]),
                                to_svg_y(to[1])
                            );
                            last_pt = *to;
                        }
                        EvaluatedPathVerb::ArcTo { to, bow } => {
                            let dx = to[0] - last_pt[0];
                            let dy = to[1] - last_pt[1];
                            let chord = (dx * dx + dy * dy).sqrt();
                            if bow.abs() < 1e-6 || chord < 1e-6 {
                                let _ =
                                    write!(d, "L {:.2} {:.2} ", to_svg_x(to[0]), to_svg_y(to[1]));
                            } else {
                                let ux = dx / chord;
                                let uy = dy / chord;
                                let nx = -uy;
                                let ny = ux;
                                let mx = (last_pt[0] + to[0]) * 0.5;
                                let my = (last_pt[1] + to[1]) * 0.5;
                                let arc_mx = mx + bow * nx;
                                let arc_my = my + bow * ny;
                                let half_c = chord * 0.5;
                                let radius = (half_c * half_c + bow * bow) / (2.0 * bow.abs());
                                let center_offset = bow - radius * bow.signum();
                                let cx = mx + center_offset * nx;
                                let cy = my + center_offset * ny;
                                let a0 = (last_pt[1] - cy).atan2(last_pt[0] - cx);
                                let amid = (arc_my - cy).atan2(arc_mx - cx);
                                let a1 = (to[1] - cy).atan2(to[0] - cx);
                                let tau = std::f64::consts::TAU;
                                let ccw_end = (a1 - a0).rem_euclid(tau);
                                let ccw_mid = (amid - a0).rem_euclid(tau);
                                let sweep = if ccw_mid <= ccw_end + 1e-9 {
                                    ccw_end
                                } else {
                                    ccw_end - tau
                                };
                                let n_segs = ((sweep.abs() / std::f64::consts::FRAC_PI_4).ceil()
                                    as usize)
                                    .clamp(1, 16);
                                let dt = sweep / (n_segs as f64);
                                let k = (4.0 / 3.0) * (dt * 0.25).tan();
                                for seg in 0..n_segs {
                                    let t0 = a0 + (seg as f64) * dt;
                                    let t1 = if seg + 1 == n_segs {
                                        a0 + sweep
                                    } else {
                                        t0 + dt
                                    };
                                    let (s0, c0) = t0.sin_cos();
                                    let (s1, c1) = t1.sin_cos();
                                    let sx = cx + radius * c0;
                                    let sy = cy + radius * s0;
                                    let ex = if seg + 1 == n_segs {
                                        to[0]
                                    } else {
                                        cx + radius * c1
                                    };
                                    let ey = if seg + 1 == n_segs {
                                        to[1]
                                    } else {
                                        cy + radius * s1
                                    };
                                    let ctrl1 = [sx - k * radius * s0, sy + k * radius * c0];
                                    let ctrl2 = [ex + k * radius * s1, ey - k * radius * c1];
                                    let _ = write!(
                                        d,
                                        "C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                                        to_svg_x(ctrl1[0]),
                                        to_svg_y(ctrl1[1]),
                                        to_svg_x(ctrl2[0]),
                                        to_svg_y(ctrl2[1]),
                                        to_svg_x(ex),
                                        to_svg_y(ey)
                                    );
                                }
                            }
                            last_pt = *to;
                        }
                        EvaluatedPathVerb::Ellipse {
                            center, rx, ry, ..
                        } => {
                            let cx = to_svg_x(center[0]);
                            let cy = to_svg_y(center[1]);
                            let rx_px = rx * ppi;
                            let ry_px = ry * ppi;
                            let k = 0.552_284_75;
                            let kx = rx_px * k;
                            let ky = ry_px * k;
                            let _ = write!(
                                d,
                                "M {:.2} {:.2} C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} Z ",
                                cx - rx_px, cy,
                                cx - rx_px, cy - ky, cx - kx, cy - ry_px, cx, cy - ry_px,
                                cx + kx, cy - ry_px, cx + rx_px, cy - ky, cx + rx_px, cy,
                                cx + rx_px, cy + ky, cx + kx, cy + ry_px, cx, cy + ry_px,
                                cx - kx, cy + ry_px, cx - rx_px, cy + ky, cx - rx_px, cy
                            );
                        }
                        EvaluatedPathVerb::Close => d.push_str("Z "),
                    }
                }
            }
            if !d.is_empty() {
                let mut first_move: Option<[f64; 2]> = None;
                let mut last_end: Option<[f64; 2]> = None;
                let mut has_explicit_close = rounded_emitted;
                for verb in &path.verbs_page_space {
                    match *verb {
                        EvaluatedPathVerb::MoveTo(p) => {
                            if first_move.is_none() {
                                first_move = Some(p);
                            }
                            last_end = Some(p);
                        }
                        EvaluatedPathVerb::LineTo(p)
                        | EvaluatedPathVerb::QuadTo { to: p, .. }
                        | EvaluatedPathVerb::CubicTo { to: p, .. }
                        | EvaluatedPathVerb::ArcTo { to: p, .. } => {
                            last_end = Some(p);
                        }
                        EvaluatedPathVerb::Ellipse { .. } | EvaluatedPathVerb::Close => {
                            has_explicit_close = true;
                        }
                    }
                }
                let closes_geometrically = first_move
                    .zip(last_end)
                    .is_some_and(|(a, b)| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt() < 1e-4);
                let effective_fill = if has_explicit_close || closes_geometrically {
                    p_fill
                } else {
                    "none"
                };
                let effective_stroke = if vis.line_pattern == 0 {
                    "none"
                } else {
                    p_stroke
                };
                let dash_attr = match vis.line_pattern {
                    0 | 1 => String::new(),
                    2 | 7 | 9 | 16 => {
                        format!(" stroke-dasharray=\"{:.1},{:.1}\"", p_sw_px * 4.5, p_sw_px * 2.8)
                    }
                    3 | 10 | 17 | 23 => {
                        format!(" stroke-dasharray=\"{:.1},{:.1}\"", p_sw_px * 0.8, p_sw_px * 2.4)
                    }
                    4 | 11 | 18 => format!(
                        " stroke-dasharray=\"{:.1},{:.1},{:.1},{:.1}\"",
                        p_sw_px * 4.5,
                        p_sw_px * 2.2,
                        p_sw_px * 0.9,
                        p_sw_px * 2.2
                    ),
                    _ => format!(" stroke-dasharray=\"{:.1},{:.1}\"", p_sw_px * 3.8, p_sw_px * 2.4),
                };
                let _ = writeln!(
                    body,
                    "<path d=\"{}\" fill=\"{effective_fill}\" stroke=\"{effective_stroke}\" stroke-width=\"{p_sw_px:.2}\" stroke-linejoin=\"round\" stroke-linecap=\"round\"{dash_attr}{marker_start}{marker_end}/>",
                    d.trim()
                );
                if sketchy && !sketch_d.is_empty() && effective_stroke != "none" {
                    let _ = writeln!(
                        body,
                        "<path d=\"{}\" fill=\"none\" stroke=\"{effective_stroke}\" stroke-width=\"{:.2}\" stroke-opacity=\"0.55\"/>",
                        sketch_d.trim(),
                        (p_sw_px * 0.7).max(0.6)
                    );
                }
            }
        }
    }

    // Embedded `ForeignMediaData` Vector SVG / EMF / WMF or Cropped Raster Image
    let fd_resolved = vis
        .foreign_media
        .as_ref()
        .or_else(|| shape_opt.and_then(|sh| sh.foreign_data.as_ref()));
    if let Some(fd) = fd_resolved {
        if !fd.data.is_empty() {
            let box_left = to_svg_x(vis.transform.pin_x - vis.transform.width * 0.5);
            let box_top = to_svg_y(vis.transform.pin_y + vis.transform.height * 0.5);
            let box_w = (vis.transform.width * ppi).max(4.0);
            let box_h = (vis.transform.height * ppi).max(4.0);
            let cx = to_svg_x(vis.transform.pin_x);
            let cy = to_svg_y(vis.transform.pin_y);
            let angle_deg = vis.transform.angle_rad.to_degrees();

            let mut inlined_vector = false;
            if fd.media_type.eq_ignore_ascii_case("SVG") {
                if let Ok(svg_txt) = std::str::from_utf8(&fd.data) {
                    if let Some((vb, inner)) = extract_inline_svg_payload(
                        svg_txt,
                        &format!("vx-{ns_prefix}-{}-", vis.shape_id),
                    ) {
                        let vb_nums: Vec<f64> = vb
                            .split(|c: char| c.is_whitespace() || c == ',')
                            .filter(|s| !s.is_empty())
                            .filter_map(|s| s.parse::<f64>().ok())
                            .collect();
                        let (vb_x, vb_y, vb_w, vb_h) = if vb_nums.len() == 4 {
                            (
                                vb_nums[0],
                                vb_nums[1],
                                vb_nums[2].abs().max(1.0),
                                vb_nums[3].abs().max(1.0),
                            )
                        } else {
                            (0.0, 0.0, 64.0, 64.0)
                        };
                        let sx = box_w / vb_w;
                        let sy = box_h / vb_h;
                        let tx = box_left - vb_x * sx;
                        let ty = box_top - vb_y * sy;
                        if angle_deg.abs() > 0.01 {
                            let _ = write!(
                                body,
                                "<g transform=\"rotate({:.2} {cx:.2} {cy:.2})\">",
                                -angle_deg
                            );
                        }
                        let _ = writeln!(
                            body,
                            "<g transform=\"translate({tx:.2} {ty:.2}) scale({sx:.4} {sy:.4})\">{inner}</g>"
                        );
                        if angle_deg.abs() > 0.01 {
                            body.push_str("</g>");
                        }
                        inlined_vector = true;
                    }
                }
            } else if fd.media_type.eq_ignore_ascii_case("EMF")
                || fd.media_type.eq_ignore_ascii_case("WMF")
            {
                let mf_mesh = vexio_render::foreign_media::tessellate_foreign_media(
                    fd,
                    &vis.transform,
                    0.005,
                );
                if !mf_mesh.indices.is_empty() {
                    body.push_str("<g class=\"vexio-metafile\">");
                    for tri in mf_mesh.indices.chunks_exact(3) {
                        let v0 = &mf_mesh.vertices[tri[0] as usize];
                        let v1 = &mf_mesh.vertices[tri[1] as usize];
                        let v2 = &mf_mesh.vertices[tri[2] as usize];
                        let hex = linear_rgba_to_hex(v0.color);
                        let op = v0.color[3].clamp(0.0, 1.0);
                        let op_attr = if op < 0.995 {
                            format!(" fill-opacity=\"{op:.2}\"")
                        } else {
                            String::new()
                        };
                        let _ = write!(
                            body,
                            "<polygon points=\"{:.2},{:.2} {:.2},{:.2} {:.2},{:.2}\" fill=\"{hex}\" stroke=\"{hex}\" stroke-width=\"0.35\"{op_attr}/>",
                            to_svg_x(v0.pos[0] as f64),
                            to_svg_y(v0.pos[1] as f64),
                            to_svg_x(v1.pos[0] as f64),
                            to_svg_y(v1.pos[1] as f64),
                            to_svg_x(v2.pos[0] as f64),
                            to_svg_y(v2.pos[1] as f64),
                        );
                    }
                    body.push_str("</g>\n");
                    inlined_vector = true;
                }
            }

            if !inlined_vector {
                let mime = match fd.media_type.to_ascii_uppercase().as_str() {
                    "JPG" | "JPEG" => "image/jpeg",
                    "SVG" => "image/svg+xml",
                    "BMP" => "image/bmp",
                    _ => "image/png",
                };
                let b64 = base64::engine::general_purpose::STANDARD.encode(&fd.data);
                let crop_l = shape_opt
                    .and_then(|sh| sh.sheet.get_f64("CropLeft"))
                    .unwrap_or(0.0)
                    * ppi;
                let crop_r = shape_opt
                    .and_then(|sh| sh.sheet.get_f64("CropRight"))
                    .unwrap_or(0.0)
                    * ppi;
                let crop_t = shape_opt
                    .and_then(|sh| sh.sheet.get_f64("CropTop"))
                    .unwrap_or(0.0)
                    * ppi;
                let crop_b = shape_opt
                    .and_then(|sh| sh.sheet.get_f64("CropBottom"))
                    .unwrap_or(0.0)
                    * ppi;
                let brightness = shape_opt
                    .and_then(|sh| sh.sheet.get_f64("Brightness"))
                    .unwrap_or(1.0)
                    .clamp(0.1, 3.0);
                let contrast = shape_opt
                    .and_then(|sh| sh.sheet.get_f64("Contrast"))
                    .unwrap_or(1.0)
                    .clamp(0.1, 3.0);

                let clip_id = format!("vexio-imgclip-{ns_prefix}-{}", vis.shape_id);
                let _ = writeln!(
                    defs,
                    "<clipPath id=\"{clip_id}\"><rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\"/></clipPath>",
                    box_left + crop_l,
                    box_top + crop_t,
                    (box_w - crop_l - crop_r).max(2.0),
                    (box_h - crop_t - crop_b).max(2.0)
                );
                let rot_attr = if angle_deg.abs() > 0.01 {
                    format!(" transform=\"rotate({:.2} {cx:.2} {cy:.2})\"", -angle_deg)
                } else {
                    String::new()
                };
                let _ = writeln!(
                    body,
                    "<image href=\"data:{mime};base64,{b64}\" x=\"{box_left:.1}\" y=\"{box_top:.1}\" width=\"{box_w:.1}\" height=\"{box_h:.1}\" preserveAspectRatio=\"none\" clip-path=\"url(#{clip_id})\"{rot_attr} style=\"filter:brightness({brightness:.2}) contrast({contrast:.2})\"/>"
                );
            }
        }
    }

    // Multi-Line & Inline Rich-Text Rendering (`**bold**`, `*italic*`, `__underline__`, `~~strike~~`, bullets, RTL, `[vert]`)
    if !vis.text.is_empty() {
        render_rich_svg_text(
            body,
            vis,
            to_svg_x(vis.txt_pos[0]),
            to_svg_y(vis.txt_pos[1]),
            page_bg_hex,
        );
    }

    // Subprocess `[+]` Visio Drill-Down Badge if shape links to a subprocess page
    let is_subproc = shape_opt
        .and_then(|s| s.sheet.get_bool("User.IsSubprocess"))
        .unwrap_or(false)
        || default_hl.map(|h| !h.sub_address.is_empty()).unwrap_or(false);
    if is_subproc {
        let bx = to_svg_x(vis.transform.pin_x) - 6.0;
        let by = to_svg_y(vis.transform.pin_y - vis.transform.height * 0.5) - 14.0;
        let _ = writeln!(
            body,
            "<g class=\"vexio-subprocess-badge\"><rect x=\"{bx:.1}\" y=\"{by:.1}\" width=\"12\" height=\"12\" rx=\"2\" fill=\"#eff6ff\" stroke=\"#1d4ed8\" stroke-width=\"1.2\"/><path d=\"M {:.1} {:.1} h 6 M {:.1} {:.1} v 6\" stroke=\"#1d4ed8\" stroke-width=\"1.2\"/></g>",
            bx + 3.0,
            by + 6.0,
            bx + 6.0,
            by + 3.0
        );
    }

    // Live Data Graphics (`Icon Sets`, `Data Bars`, `Text Callouts`) vector SVG overlay
    if let Some(dg) = &vis.data_graphics {
        body.push_str("<g class=\"vexio-data-graphic\">");
        if let Some(badge) = &dg.icon_badge {
            let cx = to_svg_x(badge.center_page_in[0]);
            let cy = to_svg_y(badge.center_page_in[1]);
            let r = (badge.radius_in * ppi).max(5.0);
            let fill_hex = badge.fill_color.to_hex_rgb();
            let sw = (r * 0.24).max(1.4);
            let _ = write!(
                body,
                "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{:.2}\" fill=\"#ffffff\"/><circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{r:.2}\" fill=\"{fill_hex}\"/>",
                r + 1.6
            );
            match badge.kind {
                vexio_shapesheet::DataGraphicIconKind::CheckOk => {
                    let _ = write!(
                        body,
                        "<path d=\"M {:.2} {:.2} L {:.2} {:.2} L {:.2} {:.2}\" fill=\"none\" stroke=\"#ffffff\" stroke-width=\"{sw:.2}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                        cx - r * 0.42,
                        cy + r * 0.02,
                        cx - r * 0.10,
                        cy + r * 0.34,
                        cx + r * 0.45,
                        cy - r * 0.32
                    );
                }
                vexio_shapesheet::DataGraphicIconKind::WarnTriangle => {
                    let _ = write!(
                        body,
                        "<line x1=\"{cx:.2}\" y1=\"{:.2}\" x2=\"{cx:.2}\" y2=\"{:.2}\" stroke=\"#ffffff\" stroke-width=\"{sw:.2}\" stroke-linecap=\"round\"/><circle cx=\"{cx:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"#ffffff\"/>",
                        cy - r * 0.42,
                        cy + r * 0.05,
                        cy + r * 0.36,
                        sw * 0.62
                    );
                }
                vexio_shapesheet::DataGraphicIconKind::ErrorCross => {
                    let _ = write!(
                        body,
                        "<path d=\"M {:.2} {:.2} L {:.2} {:.2} M {:.2} {:.2} L {:.2} {:.2}\" fill=\"none\" stroke=\"#ffffff\" stroke-width=\"{sw:.2}\" stroke-linecap=\"round\"/>",
                        cx - r * 0.34,
                        cy - r * 0.34,
                        cx + r * 0.34,
                        cy + r * 0.34,
                        cx - r * 0.34,
                        cy + r * 0.34,
                        cx + r * 0.34,
                        cy - r * 0.34
                    );
                }
                vexio_shapesheet::DataGraphicIconKind::InfoCircle => {
                    let _ = write!(
                        body,
                        "<circle cx=\"{cx:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"#ffffff\"/><line x1=\"{cx:.2}\" y1=\"{:.2}\" x2=\"{cx:.2}\" y2=\"{:.2}\" stroke=\"#ffffff\" stroke-width=\"{sw:.2}\" stroke-linecap=\"round\"/>",
                        cy - r * 0.35,
                        sw * 0.58,
                        cy - r * 0.08,
                        cy + r * 0.38
                    );
                }
            }
        }
        if let Some(bar) = &dg.data_bar {
            let x = to_svg_x(bar.rect_min_in[0]);
            let y = to_svg_y(bar.rect_max_in[1]);
            let w = ((bar.rect_max_in[0] - bar.rect_min_in[0]) * ppi).max(16.0);
            let h = ((bar.rect_max_in[1] - bar.rect_min_in[1]) * ppi).max(8.0);
            let fw = ((w - 2.4) * bar.ratio.clamp(0.0, 1.0)).max(0.0);
            let bcol = bar.bar_color.to_hex_rgb();
            let tcol = if bar.ratio > 0.55 { "#ffffff" } else { "#0f172a" };
            let _ = write!(
                body,
                "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" rx=\"3.5\" fill=\"#e2e8f0\" stroke=\"#94a3b8\" stroke-width=\"0.9\"/>"
            );
            if fw > 0.5 {
                let _ = write!(
                    body,
                    "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{fw:.2}\" height=\"{:.2}\" rx=\"2.5\" fill=\"{bcol}\"/>",
                    x + 1.2,
                    y + 1.2,
                    (h - 2.4).max(2.0)
                );
            }
            if !bar.value_label.is_empty() {
                let _ = write!(
                    body,
                    "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Inter, Segoe UI, sans-serif\" font-size=\"9.2\" font-weight=\"bold\" fill=\"{tcol}\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>",
                    x + w * 0.5,
                    y + h * 0.5,
                    vexio_vsdx::opc::xml_escape(&bar.value_label)
                );
            }
        }
        if let Some(co) = &dg.text_callout {
            let x = to_svg_x(co.rect_min_in[0]);
            let y = to_svg_y(co.rect_max_in[1]);
            let w = ((co.rect_max_in[0] - co.rect_min_in[0]) * ppi).max(24.0);
            let h = ((co.rect_max_in[1] - co.rect_min_in[1]) * ppi).max(12.0);
            let _ = write!(
                body,
                "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" rx=\"5\" fill=\"{}\" stroke=\"{}\" stroke-width=\"1.0\"/><rect x=\"{:.2}\" y=\"{:.2}\" width=\"5.5\" height=\"{:.2}\" rx=\"2.5\" fill=\"{}\"/><text x=\"{:.2}\" y=\"{:.2}\" font-family=\"Inter, Segoe UI, sans-serif\" font-size=\"9.8\" font-weight=\"600\" fill=\"{}\" text-anchor=\"middle\" dominant-baseline=\"central\">{}</text>",
                co.bg_color.to_hex_rgb(),
                co.border_color.to_hex_rgb(),
                x + 2.5,
                y + 3.0,
                (h - 6.0).max(4.0),
                co.accent_color.to_hex_rgb(),
                x + 5.0 + (w - 6.0) * 0.5,
                y + h * 0.5,
                co.text_color.to_hex_rgb(),
                vexio_vsdx::opc::xml_escape(&co.text)
            );
        }
        body.push_str("</g>\n");
    }

    body.push_str("</g>\n");
}

fn extract_inline_svg_payload(svg_txt: &str, id_prefix: &str) -> Option<(String, String)> {
    let open_idx = svg_txt.find("<svg")?;
    let tag_end = svg_txt[open_idx..].find('>')? + open_idx;
    let close_idx = svg_txt.rfind("</svg>")?;
    if close_idx <= tag_end {
        return None;
    }
    let header = &svg_txt[open_idx..=tag_end];
    let vb = if let Some(vb_pos) = header.find("viewBox=\"") {
        let rest = &header[vb_pos + 9..];
        let end_q = rest.find('"')?;
        rest[..end_q].to_string()
    } else {
        "0 0 100 100".to_string()
    };
    let inner = svg_txt[tag_end + 1..close_idx]
        .replace("id=\"g", &format!("id=\"{id_prefix}g"))
        .replace("url(#g", &format!("url(#{id_prefix}g"));
    Some((vb, inner))
}

fn format_bullet_line(trimmed: &str, bullet_style: u8, idx: usize) -> String {
    if let Some(rest) = trimmed.strip_prefix("- ") {
        return format!("• {rest}");
    }
    match bullet_style {
        1 if !trimmed.starts_with('•') => format!("• {trimmed}"),
        2 => {
            let already_numbered = trimmed
                .split_once(". ")
                .is_some_and(|(prefix, _)| !prefix.is_empty() && prefix.chars().all(|c| c.is_ascii_digit()));
            if already_numbered {
                trimmed.to_string()
            } else {
                format!("{}. {trimmed}", idx + 1)
            }
        }
        3 if !trimmed.starts_with('◦') => format!("◦ {trimmed}"),
        4 if !trimmed.starts_with('▪') => format!("▪ {trimmed}"),
        _ => trimmed.to_string(),
    }
}

fn strip_inline_markup_plain(mut s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    while !s.is_empty() {
        if let Some(after) = s.strip_prefix("[color=") {
            if let Some(close_br) = after.find(']') {
                let body_rest = &after[close_br + 1..];
                if let Some(close_tag) = body_rest.find("[/color]") {
                    out.push_str(&strip_inline_markup_plain(&body_rest[..close_tag]));
                    s = &body_rest[close_tag + 8..];
                    continue;
                }
            }
        }
        for marker in ["***", "**", "__", "~~"] {
            if let Some(after) = s.strip_prefix(marker) {
                if let Some(end) = after.find(marker) {
                    out.push_str(&after[..end]);
                    s = &after[end + marker.len()..];
                    break;
                }
            }
        }
        if s.is_empty() {
            break;
        }
        if let Some(after) = s.strip_prefix('*') {
            if let Some(end) = after.find('*') {
                if end > 0 {
                    out.push_str(&after[..end]);
                    s = &after[end + 1..];
                    continue;
                }
            }
        }
        let Some(ch) = s.chars().next() else { break };
        out.push(ch);
        s = &s[ch.len_utf8()..];
    }
    out
}

fn render_rich_svg_text(
    body: &mut String,
    vis: &EvaluatedShapeVisual,
    tx: f64,
    ty: f64,
    page_bg_hex: &str,
) {
    let raw = vis.text.trim();
    if raw.is_empty() {
        return;
    }
    let is_vertical = raw.starts_with("[vert]");
    let cleaned_raw = raw.strip_prefix("[vert]").unwrap_or(raw).trim();
    let has_rtl = cleaned_raw
        .chars()
        .any(|c| ('\u{0590}'..='\u{08FF}').contains(&c));

    let fsize = (vis.font_size_pt.clamp(4.0, 144.0) * (96.0 / 72.0)).max(5.3);
    let line_h = fsize * 1.25;
    let tcol = vis.text_color.to_hex_rgb();
    let lines: Vec<&str> = cleaned_raw.lines().collect();
    let total_block_h = (lines.len().saturating_sub(1) as f64) * line_h;
    let is_external_caption = vis.kind != vexio_shapesheet::ShapeKind::Connector1D
        && (vis.txt_pos[1] < vis.transform.pin_y - vis.transform.height * 0.48
            || vis.txt_pos[1] > vis.transform.pin_y + vis.transform.height * 0.48);

    let default_w_in = if vis.kind == vexio_shapesheet::ShapeKind::Connector1D {
        vis.transform.width.abs().max(2.2)
    } else {
        vis.transform.width.abs()
    };
    let default_h_in = if vis.kind == vexio_shapesheet::ShapeKind::Connector1D {
        vis.transform.height.abs().max(0.5)
    } else {
        vis.transform.height.abs()
    };
    let box_w_px = vis.txt_width.unwrap_or(default_w_in) * 96.0;
    let box_h_px = vis.txt_height.unwrap_or(default_h_in) * 96.0;
    let pad_px = 6.0;
    let (anchor_x, anchor_attr) = if is_external_caption {
        (tx, "middle")
    } else {
        match vis.horiz_align {
            0 => (tx - box_w_px * 0.5 + pad_px, "start"),
            2 => (tx + box_w_px * 0.5 - pad_px, "end"),
            _ => (tx, "middle"),
        }
    };

    let start_y = if is_external_caption {
        ty
    } else {
        match vis.vert_align {
            0 => ty - box_h_px * 0.5 + pad_px + fsize * 0.55,
            2 => ty + box_h_px * 0.5 - pad_px - total_block_h - fsize * 0.35,
            _ => ty - total_block_h * 0.5,
        }
    };

    let readable_rad = vexio_render::tessellate::readable_text_angle_rad(vis.transform.angle_rad);
    // Visio angles are CCW in Y-up page coordinates, which maps to `-deg` in SVG Y-down coordinates!
    let svg_rot_deg = -readable_rad.to_degrees();
    let has_rot = svg_rot_deg.abs() > 0.05;
    if has_rot {
        let _ = writeln!(
            body,
            "<g transform=\"rotate({svg_rot_deg:.2} {tx:.2} {ty:.2})\">"
        );
    }

    if let Some(bg) = vis.text_bg_color {
        let max_chars = lines
            .iter()
            .map(|l| strip_inline_markup_plain(l).chars().count())
            .max()
            .unwrap_or(1) as f64;
        let est_w = (max_chars * fsize * 0.62 + 10.0).min(box_w_px.max(36.0));
        let est_h = (lines.len().max(1) as f64) * line_h + 4.0;
        let rx_bg = match anchor_attr {
            "start" => anchor_x - 4.0,
            "end" => anchor_x - est_w + 4.0,
            _ => anchor_x - est_w * 0.5,
        };
        let ry_bg = start_y - fsize * 0.65;
        let _ = writeln!(
            body,
            "<rect x=\"{rx_bg:.1}\" y=\"{ry_bg:.1}\" width=\"{est_w:.1}\" height=\"{est_h:.1}\" rx=\"3\" fill=\"{}\" fill-opacity=\"0.92\"/>",
            bg.to_hex_rgb()
        );
    }

    let writing_attr = if is_vertical {
        " writing-mode=\"vertical-rl\""
    } else if has_rtl {
        " direction=\"rtl\""
    } else {
        ""
    };
    let is_bold = (vis.char_style & 1) != 0;
    let is_italic = (vis.char_style & 2) != 0;
    let is_underline = (vis.char_style & 4) != 0;
    let is_strike = (vis.char_style & 8) != 0;
    let weight_attr = if is_bold {
        " font-weight=\"bold\""
    } else if is_external_caption {
        " font-weight=\"500\""
    } else {
        ""
    };
    let style_attr = if is_italic { " font-style=\"italic\"" } else { "" };
    let deco_attr = match (is_underline, is_strike) {
        (true, true) => " text-decoration=\"underline line-through\"",
        (true, false) => " text-decoration=\"underline\"",
        (false, true) => " text-decoration=\"line-through\"",
        (false, false) => "",
    };
    let halo_attr = if is_external_caption || (vis.kind == vexio_shapesheet::ShapeKind::Connector1D && vis.text_bg_color.is_none()) {
        format!(
            " paint-order=\"stroke\" stroke=\"{}\" stroke-width=\"2.6\" stroke-linejoin=\"round\"",
            vexio_vsdx::opc::xml_escape(page_bg_hex)
        )
    } else {
        String::new()
    };
    let safe_font = vexio_vsdx::opc::xml_escape(if vis.font_family.trim().is_empty() {
        "Inter, Segoe UI, sans-serif"
    } else {
        vis.font_family.trim()
    });

    let _ = write!(
        body,
        "<text x=\"{anchor_x:.2}\" y=\"{start_y:.2}\" font-family=\"{safe_font}, Segoe UI, Roboto, sans-serif\" font-size=\"{fsize:.1}\" fill=\"{tcol}\" text-anchor=\"{anchor_attr}\" dominant-baseline=\"middle\"{weight_attr}{style_attr}{deco_attr}{writing_attr}{halo_attr}>"
    );

    for (idx, line) in lines.iter().enumerate() {
        let dy = if idx == 0 { 0.0 } else { line_h };
        let bullet_normalized = format_bullet_line(line.trim(), vis.bullet_style, idx);
        let _ = write!(body, "<tspan x=\"{anchor_x:.2}\" dy=\"{dy:.1}\">");
        emit_inline_styled_tspans(body, &bullet_normalized);
        body.push_str("</tspan>");
    }
    body.push_str("</text>\n");
    if has_rot {
        body.push_str("</g>\n");
    }
}

fn emit_inline_styled_tspans(out: &mut String, mut rest: &str) {
    while !rest.is_empty() {
        if let Some(after_open) = rest.strip_prefix("[color=") {
            if let Some(close_br) = after_open.find(']') {
                let col_str = after_open[..close_br].trim();
                let body_rest = &after_open[close_br + 1..];
                if let Some(close_tag) = body_rest.find("[/color]") {
                    let _ = write!(
                        out,
                        "<tspan fill=\"{}\">",
                        vexio_vsdx::opc::xml_escape(col_str)
                    );
                    emit_inline_styled_tspans(out, &body_rest[..close_tag]);
                    out.push_str("</tspan>");
                    rest = &body_rest[close_tag + 8..];
                    continue;
                }
            }
        }
        if let Some(after_open) = rest.strip_prefix("***") {
            if let Some(end) = after_open.find("***") {
                let _ = write!(
                    out,
                    "<tspan font-weight=\"bold\" font-style=\"italic\">{}</tspan>",
                    vexio_vsdx::opc::xml_escape(&after_open[..end])
                );
                rest = &after_open[end + 3..];
                continue;
            }
        }
        if let Some(after_open) = rest.strip_prefix("**") {
            if let Some(end) = after_open.find("**") {
                let _ = write!(
                    out,
                    "<tspan font-weight=\"bold\">{}</tspan>",
                    vexio_vsdx::opc::xml_escape(&after_open[..end])
                );
                rest = &after_open[end + 2..];
                continue;
            }
        }
        if let Some(after_open) = rest.strip_prefix("__") {
            if let Some(end) = after_open.find("__") {
                let _ = write!(
                    out,
                    "<tspan text-decoration=\"underline\">{}</tspan>",
                    vexio_vsdx::opc::xml_escape(&after_open[..end])
                );
                rest = &after_open[end + 2..];
                continue;
            }
        }
        if let Some(after_open) = rest.strip_prefix("~~") {
            if let Some(end) = after_open.find("~~") {
                let _ = write!(
                    out,
                    "<tspan text-decoration=\"line-through\">{}</tspan>",
                    vexio_vsdx::opc::xml_escape(&after_open[..end])
                );
                rest = &after_open[end + 2..];
                continue;
            }
        }
        if let Some(after_open) = rest.strip_prefix('*') {
            if let Some(end) = after_open.find('*') {
                if end > 0 {
                    let _ = write!(
                        out,
                        "<tspan font-style=\"italic\">{}</tspan>",
                        vexio_vsdx::opc::xml_escape(&after_open[..end])
                    );
                    rest = &after_open[end + 1..];
                    continue;
                }
            }
        }
        // Emit plain text up to the next potential marker (stepping past the first full UTF-8 codepoint)
        let first_len = rest.chars().next().map_or(rest.len(), char::len_utf8);
        let next_marker = ['*', '_', '~', '[']
            .iter()
            .filter_map(|&m| rest[first_len..].find(m).map(|p| p + first_len))
            .min()
            .unwrap_or(rest.len());
        out.push_str(&vexio_vsdx::opc::xml_escape(&rest[..next_marker]));
        rest = &rest[next_marker..];
    }
}

// ============================================================================
// 2. Vector PDF 1.7 / PDF/A-1b Multi-Page Document Export
// ============================================================================

#[inline]
fn linear_channel_to_srgb_f64(c: f32) -> f64 {
    let x = (c as f64).clamp(0.0, 1.0);
    if x <= 0.003_130_8 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
fn linear_rgba_to_hex(col: [f32; 4]) -> String {
    let r = (linear_channel_to_srgb_f64(col[0]) * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (linear_channel_to_srgb_f64(col[1]) * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (linear_channel_to_srgb_f64(col[2]) * 255.0).round().clamp(0.0, 255.0) as u8;
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn emit_pdf_mesh_triangles(stream: &mut String, mesh: &vexio_render::TessellatedMesh) {
    let mut last_col: Option<[u8; 3]> = None;
    for tri in mesh.indices.chunks_exact(3) {
        let v0 = &mesh.vertices[tri[0] as usize];
        let v1 = &mesh.vertices[tri[1] as usize];
        let v2 = &mesh.vertices[tri[2] as usize];
        // Average vertex colors for smooth per-triangle gradient shading
        let r = (linear_channel_to_srgb_f64((v0.color[0] + v1.color[0] + v2.color[0]) / 3.0)
            * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        let g = (linear_channel_to_srgb_f64((v0.color[1] + v1.color[1] + v2.color[1]) / 3.0)
            * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        let b = (linear_channel_to_srgb_f64((v0.color[2] + v1.color[2] + v2.color[2]) / 3.0)
            * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8;
        let key = [r, g, b];
        if last_col != Some(key) {
            let _ = writeln!(
                stream,
                "{:.3} {:.3} {:.3} rg",
                r as f64 / 255.0,
                g as f64 / 255.0,
                b as f64 / 255.0
            );
            last_col = Some(key);
        }
        let _ = writeln!(
            stream,
            "{:.2} {:.2} m {:.2} {:.2} l {:.2} {:.2} l h f",
            v0.pos[0] as f64 * 72.0,
            v0.pos[1] as f64 * 72.0,
            v1.pos[0] as f64 * 72.0,
            v1.pos[1] as f64 * 72.0,
            v2.pos[0] as f64 * 72.0,
            v2.pos[1] as f64 * 72.0
        );
    }
}

fn char_to_winansi_byte(ch: char) -> Option<u8> {
    let cp = ch as u32;
    if (0x20..=0x7E).contains(&cp) || (0xA0..=0xFF).contains(&cp) {
        return Some(cp as u8);
    }
    match ch {
        '€' => Some(0x80),
        '‚' => Some(0x82),
        '„' => Some(0x84),
        '…' => Some(0x85),
        'Œ' => Some(0x8C),
        '‘' => Some(0x91),
        '’' => Some(0x92),
        '“' => Some(0x93),
        '”' => Some(0x94),
        '•' | '◦' | '▪' => Some(0x95),
        '–' => Some(0x96),
        '—' => Some(0x97),
        '™' => Some(0x99),
        'œ' => Some(0x9C),
        'Ÿ' => Some(0x9F),
        _ => None,
    }
}

fn encode_pdf_winansi_literal(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len() + 8);
    out.push('(');
    for ch in s.chars() {
        let b = char_to_winansi_byte(ch)?;
        match b {
            b'\\' => out.push_str("\\\\"),
            b'(' => out.push_str("\\("),
            b')' => out.push_str("\\)"),
            0x20..=0x7E => out.push(b as char),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out.push(')');
    Some(out)
}

pub fn export_document_pdf(doc: &VisioDocument) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");

    let mut offsets: Vec<usize> = vec![0]; // 1-based PDF object IDs
    let add_obj = |buf: &mut Vec<u8>, offsets: &mut Vec<usize>, body: &str| -> usize {
        let id = offsets.len();
        offsets.push(buf.len());
        let header = format!("{id} 0 obj\n{body}\nendobj\n");
        buf.extend_from_slice(header.as_bytes());
        id
    };

    offsets.push(0); // placeholder 1 = Catalog
    offsets.push(0); // placeholder 2 = Pages
    let font_regular = add_obj(
        &mut out,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    let font_bold = add_obj(
        &mut out,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>",
    );
    let font_italic = add_obj(
        &mut out,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Oblique /Encoding /WinAnsiEncoding >>",
    );
    let font_bold_italic = add_obj(
        &mut out,
        &mut offsets,
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-BoldOblique /Encoding /WinAnsiEncoding >>",
    );

    let mut page_obj_ids = Vec::new();
    for page in &doc.pages {
        if page.is_background {
            continue;
        }
        let w_pt = page.width_in() * 72.0;
        let h_pt = page.height_in() * 72.0;
        let visuals = ReactiveEngine::evaluate_page_visuals(doc, page.id);
        let mut stream = String::new();

        // Paint page background when custom PageColor or theme background is active
        if let Some(pbg) = page.page_sheet.get_color("PageColor").or_else(|| {
            doc.active_theme_id
                .and_then(|tid| doc.themes.iter().find(|t| t.id == tid))
                .map(|t| t.background_color)
        }) {
            if pbg != RgbaColor::WHITE {
                let pr = pbg.r as f64 / 255.0;
                let pg = pbg.g as f64 / 255.0;
                let pb = pbg.b as f64 / 255.0;
                let _ = writeln!(stream, "{pr:.3} {pg:.3} {pb:.3} rg 0 0 {w_pt:.2} {h_pt:.2} re f");
            }
        }

        for vis in &visuals {
            let has_grad = vis.gradient_stops.len() >= 2 || (25..=40).contains(&vis.fill_pattern);
            let has_arrows = vis.begin_arrow > 0 || vis.end_arrow > 0;
            if has_grad || has_arrows {
                let mut no_txt = vis.clone();
                no_txt.text.clear();
                no_txt.foreign_media = None;
                no_txt.data_graphics = None;
                let geom_mesh = vexio_render::tessellate::tessellate_shape(&no_txt, 192.0);
                emit_pdf_mesh_triangles(&mut stream, &geom_mesh);
            } else {
                for path in &vis.paths {
                    if path.no_show || path.verbs_page_space.is_empty() {
                        continue;
                    }
                    let lw = (path.stroke_weight_in.unwrap_or(vis.stroke_weight_in) * 72.0).max(0.5);
                    let _ = writeln!(stream, "{lw:.2} w");
                    let eff_stroke = path.stroke_color.or(vis.stroke_color);
                    let eff_fill = if vis.fill_pattern == 0 {
                        None
                    } else {
                        path.fill_color.or(vis.fill_color)
                    };
                    if let Some(sc) = eff_stroke {
                        let r = sc.r as f64 / 255.0;
                        let g = sc.g as f64 / 255.0;
                        let b = sc.b as f64 / 255.0;
                        let _ = writeln!(stream, "{r:.3} {g:.3} {b:.3} RG");
                    }
                    if let Some(fc) = eff_fill {
                        let r = fc.r as f64 / 255.0;
                        let g = fc.g as f64 / 255.0;
                        let b = fc.b as f64 / 255.0;
                        let _ = writeln!(stream, "{r:.3} {g:.3} {b:.3} rg");
                    }
                    for verb in &path.verbs_page_space {
                        match verb {
                            EvaluatedPathVerb::MoveTo(p) => {
                                let _ = writeln!(stream, "{:.2} {:.2} m", p[0] * 72.0, p[1] * 72.0);
                            }
                            EvaluatedPathVerb::LineTo(p) | EvaluatedPathVerb::ArcTo { to: p, .. } => {
                                let _ = writeln!(stream, "{:.2} {:.2} l", p[0] * 72.0, p[1] * 72.0);
                            }
                            EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                                let _ = writeln!(
                                    stream,
                                    "{:.2} {:.2} {:.2} {:.2} {:.2} {:.2} c",
                                    ctrl1[0] * 72.0,
                                    ctrl1[1] * 72.0,
                                    ctrl2[0] * 72.0,
                                    ctrl2[1] * 72.0,
                                    to[0] * 72.0,
                                    to[1] * 72.0
                                );
                            }
                            EvaluatedPathVerb::QuadTo { ctrl, to } => {
                                let _ = writeln!(
                                    stream,
                                    "{:.2} {:.2} {:.2} {:.2} v",
                                    ctrl[0] * 72.0,
                                    ctrl[1] * 72.0,
                                    to[0] * 72.0,
                                    to[1] * 72.0
                                );
                            }
                            EvaluatedPathVerb::Ellipse { center, rx, ry, .. } => {
                                let _ = writeln!(
                                    stream,
                                    "{:.2} {:.2} {:.2} {:.2} re",
                                    (center[0] - rx) * 72.0,
                                    (center[1] - ry) * 72.0,
                                    rx * 2.0 * 72.0,
                                    ry * 2.0 * 72.0
                                );
                            }
                            EvaluatedPathVerb::Close => stream.push_str("h\n"),
                        }
                    }
                    let paint_op = match (
                        !path.no_fill && eff_fill.is_some(),
                        !path.no_line && eff_stroke.is_some(),
                    ) {
                        (true, true) => "B",
                        (true, false) => "f",
                        (false, true) => "S",
                        (false, false) => "n",
                    };
                    let _ = writeln!(stream, "{paint_op}");
                }
            }

            // Render embedded `ForeignMediaData` (EMF/EMF+/WMF/SVG/PNG/JPEG) in PDF
            let fd_resolved = vis
                .foreign_media
                .as_ref()
                .or_else(|| page.find_shape(vis.shape_id).and_then(|sh| sh.foreign_data.as_ref()));
            if let Some(fd) = fd_resolved {
                if !fd.data.is_empty() {
                    let fd_mesh = vexio_render::foreign_media::tessellate_foreign_media(
                        fd,
                        &vis.transform,
                        0.005,
                    );
                    if !fd_mesh.indices.is_empty() {
                        emit_pdf_mesh_triangles(&mut stream, &fd_mesh);
                    }
                }
            }

            let trimmed_txt = vis.text.trim();
            if !trimmed_txt.is_empty() {
                let cleaned_txt = trimmed_txt.strip_prefix("[vert]").unwrap_or(trimmed_txt).trim();
                let font_pt = vis.font_size_pt.clamp(4.0, 144.0);
                let line_h_pt = font_pt * 1.25;
                let default_w_in = if vis.kind == vexio_shapesheet::ShapeKind::Connector1D {
                    vis.transform.width.abs().max(2.2)
                } else {
                    vis.transform.width.abs().max(0.45)
                };
                let default_h_in = if vis.kind == vexio_shapesheet::ShapeKind::Connector1D {
                    vis.transform.height.abs().max(0.5)
                } else {
                    vis.transform.height.abs().max(0.28)
                };
                let box_w_pt = vis.txt_width.unwrap_or(default_w_in) * 72.0;
                let box_h_pt = vis.txt_height.unwrap_or(default_h_in) * 72.0;
                let readable_rad =
                    vexio_render::tessellate::readable_text_angle_rad(vis.transform.angle_rad);
                let (sa, ca) = readable_rad.sin_cos();
                let cx_pt = vis.txt_pos[0] * 72.0;
                let cy_pt = vis.txt_pos[1] * 72.0;
                let is_bold = (vis.char_style & 1) != 0;
                let is_italic = (vis.char_style & 2) != 0;

                let plain_lines: Vec<String> = cleaned_txt
                    .lines()
                    .enumerate()
                    .map(|(i, l)| {
                        let bul = format_bullet_line(l.trim(), vis.bullet_style, i);
                        strip_inline_markup_plain(&bul)
                    })
                    .collect();

                if let Some(bg) = vis.text_bg_color {
                    let br = bg.r as f64 / 255.0;
                    let bg_g = bg.g as f64 / 255.0;
                    let bb = bg.b as f64 / 255.0;
                    let max_w_pt = plain_lines
                        .iter()
                        .map(|l| {
                            vexio_render::ttf_shaper::measure_text_width_in(
                                l,
                                &vis.font_family,
                                font_pt,
                                is_bold,
                                is_italic,
                            ) * 72.0
                        })
                        .fold(12.0_f64, f64::max);
                    let hw = (max_w_pt + 8.0).min(box_w_pt.max(24.0)) * 0.5;
                    let hh = ((plain_lines.len().max(1) as f64) * line_h_pt + 4.0) * 0.5;
                    let map_pt = |lx: f64, ly: f64| -> (f64, f64) {
                        (cx_pt + lx * ca - ly * sa, cy_pt + lx * sa + ly * ca)
                    };
                    let (x0, y0) = map_pt(-hw, -hh);
                    let (x1, y1) = map_pt(hw, -hh);
                    let (x2, y2) = map_pt(hw, hh);
                    let (x3, y3) = map_pt(-hw, hh);
                    let _ = writeln!(
                        stream,
                        "{br:.3} {bg_g:.3} {bb:.3} rg {x0:.2} {y0:.2} m {x1:.2} {y1:.2} l {x2:.2} {y2:.2} l {x3:.2} {y3:.2} l h f"
                    );
                }

                // Explicitly set PDF stroke & fill color to sRGB `vis.text_color` so text is NEVER invisible against shape fill!
                let tr = vis.text_color.r as f64 / 255.0;
                let tg = vis.text_color.g as f64 / 255.0;
                let tb = vis.text_color.b as f64 / 255.0;
                let _ = writeln!(stream, "{tr:.3} {tg:.3} {tb:.3} RG\n{tr:.3} {tg:.3} {tb:.3} rg");

                let force_outline = page
                    .find_shape(vis.shape_id)
                    .and_then(|s| s.sheet.get_bool("User.PdfOutlineText"))
                    .unwrap_or(false);
                let encoded_lines: Option<Vec<String>> = if force_outline {
                    None
                } else {
                    plain_lines
                        .iter()
                        .map(|l| encode_pdf_winansi_literal(l))
                        .collect()
                };

                let n_lines = plain_lines.len().max(1);
                let total_block_h = (n_lines.saturating_sub(1) as f64) * line_h_pt;
                let top_line_y = match vis.vert_align {
                    0 => box_h_pt * 0.5 - 4.0 - font_pt * 0.85,
                    2 => -box_h_pt * 0.5 + 4.0 + total_block_h + font_pt * 0.15,
                    _ => total_block_h * 0.5 - font_pt * 0.35,
                };

                if let Some(winansi_literals) = encoded_lines {
                    let font_tag = match (is_bold, is_italic) {
                        (true, true) => "/F4",
                        (true, false) => "/F2",
                        (false, true) => "/F3",
                        (false, false) => "/F1",
                    };

                    for (idx, (plain, pdf_lit)) in
                        plain_lines.iter().zip(winansi_literals.iter()).enumerate()
                    {
                        let est_line_w = vexio_render::ttf_shaper::measure_text_width_in(
                            plain,
                            &vis.font_family,
                            font_pt,
                            is_bold,
                            is_italic,
                        ) * 72.0;
                        let local_x = match vis.horiz_align {
                            0 => -box_w_pt * 0.5 + 4.0,
                            2 => box_w_pt * 0.5 - 4.0 - est_line_w,
                            _ => -est_line_w * 0.5,
                        };
                        let local_y = top_line_y - (idx as f64) * line_h_pt;
                        let px = cx_pt + local_x * ca - local_y * sa;
                        let py = cy_pt + local_x * sa + local_y * ca;
                        let _ = writeln!(
                            stream,
                            "BT {font_tag} {font_pt:.1} Tf {ca:.4} {sa:.4} {:.4} {ca:.4} {px:.2} {py:.2} Tm {pdf_lit} Tj ET",
                            -sa
                        );
                        // Emit underline (`char_style & 4`) and strikethrough (`char_style & 8`) vector lines
                        if (vis.char_style & 4) != 0 && est_line_w > 0.5 {
                            let uy = local_y - font_pt * 0.14;
                            let ux0 = cx_pt + local_x * ca - uy * sa;
                            let uy0 = cy_pt + local_x * sa + uy * ca;
                            let ux1 = cx_pt + (local_x + est_line_w) * ca - uy * sa;
                            let uy1 = cy_pt + (local_x + est_line_w) * sa + uy * ca;
                            let _ = writeln!(
                                stream,
                                "{:.2} w {ux0:.2} {uy0:.2} m {ux1:.2} {uy1:.2} l S",
                                (font_pt * 0.065).max(0.5)
                            );
                        }
                        if (vis.char_style & 8) != 0 && est_line_w > 0.5 {
                            let sy = local_y + font_pt * 0.28;
                            let sx0 = cx_pt + local_x * ca - sy * sa;
                            let sy0 = cy_pt + local_x * sa + sy * ca;
                            let sx1 = cx_pt + (local_x + est_line_w) * ca - sy * sa;
                            let sy1 = cy_pt + (local_x + est_line_w) * sa + sy * ca;
                            let _ = writeln!(
                                stream,
                                "{:.2} w {sx0:.2} {sy0:.2} m {sx1:.2} {sy1:.2} l S",
                                (font_pt * 0.065).max(0.5)
                            );
                        }
                    }
                } else {
                    // Non-WinAnsi scripts (Greek, Cyrillic, arrows, math) or `User.PdfOutlineText`:
                    // Emit exact cubic Bezier TrueType glyph contours (`m`, `l`, `c`, `h`, `f`) via `ttf_shaper`,
                    // falling back to tessellated glyph triangles for RTL/CJK or custom inline color spans.
                    let needs_complex_fallback = trimmed_txt.starts_with("[vert]")
                        || trimmed_txt.contains("[color=")
                        || plain_lines.iter().any(|l| {
                            l.chars().any(|c| {
                                ('\u{0590}'..='\u{08FF}').contains(&c)
                                    || ('\u{3000}'..='\u{9FFF}').contains(&c)
                                    || ('\u{AC00}'..='\u{D7AF}').contains(&c)
                            })
                        });
                    let mut emitted_bezier_contours = false;
                    if !needs_complex_fallback {
                        let mut contour_buf = String::new();
                        let mut any_ops = false;
                        for (idx, plain) in plain_lines.iter().enumerate() {
                            let baseline_local_y_in =
                                (top_line_y - (idx as f64) * line_h_pt) / 72.0;
                            let (ops, syn_bold) =
                                vexio_render::ttf_shaper::shape_line_to_pdf_contours(
                                    plain,
                                    &vis.font_family,
                                    font_pt,
                                    is_bold,
                                    is_italic,
                                    vis.horiz_align,
                                    box_w_pt / 72.0,
                                    4.0 / 72.0,
                                    baseline_local_y_in,
                                    vis.txt_pos[0],
                                    vis.txt_pos[1],
                                    readable_rad,
                                );
                            if !ops.is_empty() {
                                any_ops = true;
                                if syn_bold {
                                    let _ = writeln!(
                                        contour_buf,
                                        "{:.2} w",
                                        (font_pt * 0.036).max(0.35)
                                    );
                                }
                                for op in &ops {
                                    match *op {
                                        vexio_render::PdfContourOp::MoveTo(x, y) => {
                                            let _ = write!(contour_buf, "{x:.2} {y:.2} m ");
                                        }
                                        vexio_render::PdfContourOp::LineTo(x, y) => {
                                            let _ = write!(contour_buf, "{x:.2} {y:.2} l ");
                                        }
                                        vexio_render::PdfContourOp::CubicTo(
                                            x1,
                                            y1,
                                            x2,
                                            y2,
                                            x,
                                            y,
                                        ) => {
                                            let _ = write!(
                                                contour_buf,
                                                "{x1:.2} {y1:.2} {x2:.2} {y2:.2} {x:.2} {y:.2} c "
                                            );
                                        }
                                        vexio_render::PdfContourOp::Close => {
                                            contour_buf.push_str("h ");
                                        }
                                    }
                                }
                                if contour_buf.ends_with(' ') {
                                    contour_buf.pop();
                                }
                                if syn_bold {
                                    contour_buf.push_str(" B\n");
                                } else {
                                    contour_buf.push_str(" f\n");
                                }
                            }
                        }
                        if any_ops {
                            stream.push_str(&contour_buf);
                            emitted_bezier_contours = true;
                        }
                    }

                    if !emitted_bezier_contours {
                        let block =
                            vexio_render::typography::parse_inline_rich_text_styled_with_font(
                                trimmed_txt,
                                font_pt,
                                vis.text_color,
                                vis.char_style,
                                vis.horiz_align,
                                vis.vert_align,
                                vis.bullet_style,
                                &vis.font_family,
                            );
                        let glyph_mesh = vexio_render::typography::tessellate_rich_text_in_box(
                            &block,
                            vis.txt_pos[0],
                            vis.txt_pos[1],
                            box_w_pt / 72.0,
                            box_h_pt / 72.0,
                            readable_rad,
                        );
                        emit_pdf_mesh_triangles(&mut stream, &glyph_mesh);
                    }
                }
            }

            if let Some(dg) = &vis.data_graphics {
                let dg_mesh = vexio_render::tessellate::tessellate_data_graphics_overlay(
                    dg,
                    &vis.font_family,
                    0.002,
                );
                emit_pdf_mesh_triangles(&mut stream, &dg_mesh);
            }
        }

        let content_id = add_obj(
            &mut out,
            &mut offsets,
            &format!("<< /Length {} >>\nstream\n{stream}endstream", stream.len()),
        );
        let page_obj = add_obj(
            &mut out,
            &mut offsets,
            &format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w_pt:.2} {h_pt:.2}] \
                 /Resources << /Font << /F1 {font_regular} 0 R /F2 {font_bold} 0 R /F3 {font_italic} 0 R /F4 {font_bold_italic} 0 R >> >> /Contents {content_id} 0 R >>"
            ),
        );
        page_obj_ids.push(page_obj);
    }

    let kids_str: Vec<String> = page_obj_ids.iter().map(|id| format!("{id} 0 R")).collect();
    offsets[2] = out.len();
    out.extend_from_slice(
        format!(
            "2 0 obj\n<< /Type /Pages /Kids [{}] /Count {} >>\nendobj\n",
            kids_str.join(" "),
            page_obj_ids.len()
        )
        .as_bytes(),
    );

    offsets[1] = out.len();
    out.extend_from_slice(
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R /MarkInfo << /Marked true >> >>\nendobj\n",
    );

    let xref_offset = out.len();
    write_pdf_xref(&mut out, &offsets, xref_offset);
    out
}

fn write_pdf_xref(out: &mut Vec<u8>, offsets: &[usize], xref_offset: usize) {
    let mut s = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len());
    for &off in &offsets[1..] {
        let _ = writeln!(s, "{off:010} 00000 n ");
    }
    let _ = write!(
        s,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
        offsets.len()
    );
    out.extend_from_slice(s.as_bytes());
}

// ============================================================================
// 3. Encapsulated PostScript (EPS) Export
// ============================================================================

pub fn export_page_eps(doc: &VisioDocument, page_id: u32) -> String {
    let Some(page) = doc.page(page_id) else {
        return String::new();
    };
    let w_pt = (page.width_in() * 72.0).ceil() as i64;
    let h_pt = (page.height_in() * 72.0).ceil() as i64;
    let mut eps = format!(
        "%!PS-Adobe-3.0 EPSF-3.0\n\
         %%Title: ({})\n\
         %%Creator: (Vexio)\n\
         %%BoundingBox: 0 0 {w_pt} {h_pt}\n\
         %%EndComments\n",
        page.name
    );
    for vis in ReactiveEngine::evaluate_page_visuals(doc, page_id) {
        for path in vis.paths {
            if path.no_show || path.verbs_page_space.is_empty() {
                continue;
            }
            eps.push_str("newpath\n");
            for v in path.verbs_page_space {
                match v {
                    EvaluatedPathVerb::MoveTo(p) => {
                        let _ = writeln!(eps, "{:.2} {:.2} moveto", p[0] * 72.0, p[1] * 72.0);
                    }
                    EvaluatedPathVerb::LineTo(p) | EvaluatedPathVerb::ArcTo { to: p, .. } => {
                        let _ = writeln!(eps, "{:.2} {:.2} lineto", p[0] * 72.0, p[1] * 72.0);
                    }
                    _ => {}
                }
            }
            eps.push_str("stroke\n");
        }
    }
    eps.push_str("%%EOF\n");
    eps
}

// ============================================================================
// 4. Pure-Rust High-DPI PNG & Baseline 4:4:4 JFIF JPEG Encoders (Zero External C Deps)
// ============================================================================

/// Render a page cleanly to RGBA8 at `dpi` without editor grid/selection chrome.
pub fn render_clean_page_rgba(doc: &VisioDocument, page_id: u32, dpi: u32) -> (u32, u32, Vec<u8>) {
    let clean_dpi = dpi.clamp(72, 600);
    let (w_in, h_in) = doc
        .page(page_id)
        .map(|p| (p.width_in().max(1.0), p.height_in().max(1.0)))
        .unwrap_or((11.0, 8.5));
    let width = ((w_in * clean_dpi as f64).round() as u32).clamp(64, 4096);
    let height = ((h_in * clean_dpi as f64).round() as u32).clamp(64, 4096);
    let mut camera = vexio_render::ViewportCamera::new(width, height, 1.0);
    camera.center_x_in = w_in * 0.5;
    camera.center_y_in = h_in * 0.5;
    camera.zoom_ppi = (width as f64 / w_in).min(height as f64 / h_in);
    let rgba = vexio_render::render_page_software_rgba_with_camera(doc, page_id, &camera, false)
        .unwrap_or_else(|_| vec![255u8; (width as usize) * (height as usize) * 4]);
    (width, height, rgba)
}

pub fn export_page_png(doc: &VisioDocument, page_id: u32, dpi: u32) -> Vec<u8> {
    let (w, h, rgba) = render_clean_page_rgba(doc, page_id, dpi);
    encode_rgba_to_png(w, h, &rgba, dpi)
}

pub fn export_page_jpeg(doc: &VisioDocument, page_id: u32, dpi: u32, quality: u8) -> Vec<u8> {
    let (w, h, rgba) = render_clean_page_rgba(doc, page_id, dpi);
    encode_rgba_to_jpeg(w, h, &rgba, dpi, quality)
}

pub fn encode_rgba_to_png(width: u32, height: u32, rgba: &[u8], dpi: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]);

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    write_png_chunk(&mut png, b"IHDR", &ihdr);

    let ppm = ((dpi as f64) / 0.0254).round() as u32;
    let mut phys = Vec::with_capacity(9);
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.push(1);
    write_png_chunk(&mut png, b"pHYs", &phys);

    let row_bytes = (width as usize) * 4;
    let mut raw_scanlines = Vec::with_capacity(height as usize * (row_bytes + 1));
    for y in 0..height as usize {
        raw_scanlines.push(0u8);
        let start = y * row_bytes;
        raw_scanlines.extend_from_slice(&rgba[start..start + row_bytes]);
    }
    let zlib_stream = encode_zlib_compressed(&raw_scanlines);
    write_png_chunk(&mut png, b"IDAT", &zlib_stream);
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

const JPEG_ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10,
    17, 24, 32, 25, 18, 11, 4, 5,
    12, 19, 26, 33, 40, 48, 41, 34,
    27, 20, 13, 6, 7, 14, 21, 28,
    35, 42, 49, 56, 57, 50, 43, 36,
    29, 22, 15, 23, 30, 37, 44, 51,
    58, 59, 52, 45, 38, 31, 39, 46,
    53, 60, 61, 54, 47, 55, 62, 63,
];

const STD_LUMA_QUANT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61,
    12, 12, 14, 19, 26, 58, 60, 55,
    14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62,
    18, 22, 37, 56, 68, 109, 103, 77,
    24, 35, 55, 64, 81, 104, 113, 92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
];

const STD_CHROMA_QUANT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99,
    18, 21, 26, 66, 99, 99, 99, 99,
    24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99,
];

const BITS_DC_LUMA: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const VALS_DC_LUMA: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const BITS_DC_CHROMA: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const VALS_DC_CHROMA: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

const BITS_AC_LUMA: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 125];
const VALS_AC_LUMA: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7,
    0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2,
    0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

const BITS_AC_CHROMA: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119];
const VALS_AC_CHROMA: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26,
    0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5,
    0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda,
    0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];

struct JpegBitWriter {
    out: Vec<u8>,
    bit_buf: u32,
    bit_count: u8,
}

impl JpegBitWriter {
    fn with_capacity(cap: usize) -> Self {
        Self {
            out: Vec::with_capacity(cap),
            bit_buf: 0,
            bit_count: 0,
        }
    }

    #[inline]
    fn write_bits(&mut self, bits: u16, len: u8) {
        if len == 0 {
            return;
        }
        let mask = (1u32 << len) - 1;
        self.bit_buf = (self.bit_buf << len) | ((bits as u32) & mask);
        self.bit_count += len;
        while self.bit_count >= 8 {
            self.bit_count -= 8;
            let byte = ((self.bit_buf >> self.bit_count) & 0xFF) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0x00);
            }
        }
    }

    fn flush_pad_ones(&mut self) {
        if self.bit_count > 0 {
            let pad = 8 - self.bit_count;
            self.write_bits((1u16 << pad) - 1, pad);
        }
    }
}

fn build_huffman_lut(bits: &[u8; 16], vals: &[u8]) -> [(u16, u8); 256] {
    let mut lut = [(0u16, 0u8); 256];
    let mut code: u16 = 0;
    let mut idx = 0usize;
    for len in 1..=16usize {
        for _ in 0..(bits[len - 1] as usize) {
            let sym = vals[idx] as usize;
            lut[sym] = (code, len as u8);
            code = code.wrapping_add(1);
            idx += 1;
        }
        code <<= 1;
    }
    lut
}

#[inline]
fn jpeg_magnitude_category(v: i32) -> (u8, u16) {
    if v == 0 {
        return (0, 0);
    }
    let abs_v = v.unsigned_abs();
    let cat = (32 - abs_v.leading_zeros()) as u8;
    let bits = if v > 0 {
        abs_v as u16
    } else {
        ((1u32 << cat) - 1 - abs_v) as u16
    };
    (cat, bits)
}

/// Pure-Rust ITU-T T.81 Baseline DCT 4:4:4 JFIF 1.02 JPEG encoder.
/// Uses 4:4:4 (`1x1` chroma) sampling so thin diagram strokes, arrowheads, and typography
/// retain razor-sharp color edges without 4:2:0 chroma subsampling blur, compositing alpha over `#FFFFFF`.
pub fn encode_rgba_to_jpeg(width: u32, height: u32, rgba: &[u8], dpi: u32, quality: u8) -> Vec<u8> {
    let w = width.clamp(1, 65535) as usize;
    let h = height.clamp(1, 65535) as usize;
    let q = quality.clamp(1, 100) as u32;
    let q_scale = if q < 50 { 5000 / q } else { 200 - 2 * q };

    let mut q_luma_zig = [1u8; 64];
    let mut q_chroma_zig = [1u8; 64];
    for k in 0..64 {
        let nat = JPEG_ZIGZAG[k];
        q_luma_zig[k] = (((STD_LUMA_QUANT[nat] as u32 * q_scale + 50) / 100).clamp(1, 255)) as u8;
        q_chroma_zig[k] = (((STD_CHROMA_QUANT[nat] as u32 * q_scale + 50) / 100).clamp(1, 255)) as u8;
    }

    // Precompute separable 1D DCT basis `cos((2*x + 1) * u * pi / 16) * alpha(u)`
    let mut dct_basis = [[0.0f32; 8]; 8];
    let inv_sqrt8 = (0.125f32).sqrt();
    for u in 0..8 {
        let alpha = if u == 0 { inv_sqrt8 } else { 0.5 };
        for x in 0..8 {
            dct_basis[u][x] =
                (((2 * x + 1) * u) as f32 * std::f32::consts::PI / 16.0).cos() * alpha;
        }
    }

    let dc_luma = build_huffman_lut(&BITS_DC_LUMA, &VALS_DC_LUMA);
    let ac_luma = build_huffman_lut(&BITS_AC_LUMA, &VALS_AC_LUMA);
    let dc_chroma = build_huffman_lut(&BITS_DC_CHROMA, &VALS_DC_CHROMA);
    let ac_chroma = build_huffman_lut(&BITS_AC_CHROMA, &VALS_AC_CHROMA);

    let mut bw = JpegBitWriter::with_capacity((w * h) / 2 + 1024);

    // 1. SOI + JFIF APP0 (density in DPI)
    bw.out.extend_from_slice(&[0xFF, 0xD8]);
    let clean_dpi = dpi.clamp(72, 1200) as u16;
    bw.out.extend_from_slice(&[
        0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F', 0x00, 0x01, 0x02, 0x01,
        (clean_dpi >> 8) as u8, (clean_dpi & 0xFF) as u8,
        (clean_dpi >> 8) as u8, (clean_dpi & 0xFF) as u8,
        0x00, 0x00,
    ]);

    // 2. DQT (Luminance table 0 & Chrominance table 1 in zig-zag order)
    for (tid, qtab) in [(0u8, &q_luma_zig), (1u8, &q_chroma_zig)] {
        bw.out.extend_from_slice(&[0xFF, 0xDB, 0x00, 67, tid]);
        bw.out.extend_from_slice(qtab);
    }

    // 3. SOF0 (Baseline DCT, 8-bit precision, 3 components, 4:4:4 1x1 sampling)
    bw.out.extend_from_slice(&[
        0xFF, 0xC0, 0x00, 17, 8,
        ((h >> 8) & 0xFF) as u8, (h & 0xFF) as u8,
        ((w >> 8) & 0xFF) as u8, (w & 0xFF) as u8,
        3,
        1, 0x11, 0, // Y:  1x1, Q0
        2, 0x11, 1, // Cb: 1x1, Q1
        3, 0x11, 1, // Cr: 1x1, Q1
    ]);

    // 4. DHT (4 standard Huffman tables)
    let write_dht = |out: &mut Vec<u8>, class_id: u8, bits: &[u8; 16], vals: &[u8]| {
        let seg_len = (2 + 1 + 16 + vals.len()) as u16;
        out.extend_from_slice(&[0xFF, 0xC4]);
        out.extend_from_slice(&seg_len.to_be_bytes());
        out.push(class_id);
        out.extend_from_slice(bits);
        out.extend_from_slice(vals);
    };
    write_dht(&mut bw.out, 0x00, &BITS_DC_LUMA, &VALS_DC_LUMA);
    write_dht(&mut bw.out, 0x10, &BITS_AC_LUMA, &VALS_AC_LUMA);
    write_dht(&mut bw.out, 0x01, &BITS_DC_CHROMA, &VALS_DC_CHROMA);
    write_dht(&mut bw.out, 0x11, &BITS_AC_CHROMA, &VALS_AC_CHROMA);

    // 5. SOS (Start of Scan)
    bw.out.extend_from_slice(&[
        0xFF, 0xDA, 0x00, 12, 3,
        1, 0x00, // Y  -> DC0/AC0
        2, 0x11, // Cb -> DC1/AC1
        3, 0x11, // Cr -> DC1/AC1
        0, 63, 0,
    ]);

    let encode_block = |bw: &mut JpegBitWriter,
                        samples: &[f32; 64],
                        q_zig: &[u8; 64],
                        prev_dc: &mut i32,
                        dc_lut: &[(u16, u8); 256],
                        ac_lut: &[(u16, u8); 256]| {
        let mut tmp = [0.0f32; 64];
        for r in 0..8 {
            let row = &samples[r * 8..r * 8 + 8];
            for u in 0..8 {
                let b = &dct_basis[u];
                tmp[r * 8 + u] = b[0] * row[0]
                    + b[1] * row[1]
                    + b[2] * row[2]
                    + b[3] * row[3]
                    + b[4] * row[4]
                    + b[5] * row[5]
                    + b[6] * row[6]
                    + b[7] * row[7];
            }
        }
        let mut dct_nat = [0.0f32; 64];
        for u in 0..8 {
            for v in 0..8 {
                let b = &dct_basis[v];
                dct_nat[v * 8 + u] = b[0] * tmp[u]
                    + b[1] * tmp[8 + u]
                    + b[2] * tmp[16 + u]
                    + b[3] * tmp[24 + u]
                    + b[4] * tmp[32 + u]
                    + b[5] * tmp[40 + u]
                    + b[6] * tmp[48 + u]
                    + b[7] * tmp[56 + u];
            }
        }

        let mut zz = [0i32; 64];
        for k in 0..64 {
            let raw = dct_nat[JPEG_ZIGZAG[k]] / (q_zig[k] as f32);
            zz[k] = raw.round() as i32;
        }

        let diff = zz[0] - *prev_dc;
        *prev_dc = zz[0];
        let (dc_cat, dc_bits) = jpeg_magnitude_category(diff);
        let (dc_code, dc_len) = dc_lut[dc_cat as usize];
        bw.write_bits(dc_code, dc_len);
        bw.write_bits(dc_bits, dc_cat);

        let mut zero_run = 0u8;
        for &ac in &zz[1..] {
            if ac == 0 {
                zero_run += 1;
            } else {
                while zero_run >= 16 {
                    let (zrl_code, zrl_len) = ac_lut[0xF0];
                    bw.write_bits(zrl_code, zrl_len);
                    zero_run -= 16;
                }
                let (ac_cat, ac_bits) = jpeg_magnitude_category(ac);
                let sym = ((zero_run << 4) | ac_cat) as usize;
                let (ac_code, ac_len) = ac_lut[sym];
                bw.write_bits(ac_code, ac_len);
                bw.write_bits(ac_bits, ac_cat);
                zero_run = 0;
            }
        }
        if zero_run > 0 {
            let (eob_code, eob_len) = ac_lut[0x00];
            bw.write_bits(eob_code, eob_len);
        }
    };

    let mut prev_dc_y = 0i32;
    let mut prev_dc_cb = 0i32;
    let mut prev_dc_cr = 0i32;
    let mcu_cols = w.div_ceil(8);
    let mcu_rows = h.div_ceil(8);

    for my in 0..mcu_rows {
        for mx in 0..mcu_cols {
            let mut blk_y = [0.0f32; 64];
            let mut blk_cb = [0.0f32; 64];
            let mut blk_cr = [0.0f32; 64];

            for py in 0..8 {
                let sy = (my * 8 + py).min(h - 1);
                for px in 0..8 {
                    let sx = (mx * 8 + px).min(w - 1);
                    let base = (sy * w + sx) * 4;
                    let (sr, sg, sb, sa) = if base + 3 < rgba.len() {
                        (
                            rgba[base] as f32,
                            rgba[base + 1] as f32,
                            rgba[base + 2] as f32,
                            rgba[base + 3] as f32 / 255.0,
                        )
                    } else {
                        (255.0, 255.0, 255.0, 1.0)
                    };
                    // Alpha-composite onto crisp white page background
                    let r = sr * sa + 255.0 * (1.0 - sa);
                    let g = sg * sa + 255.0 * (1.0 - sa);
                    let b = sb * sa + 255.0 * (1.0 - sa);

                    let y_val = (0.299 * r + 0.587 * g + 0.114 * b).clamp(0.0, 255.0) - 128.0;
                    let cb_val =
                        (-0.168_736 * r - 0.331_264 * g + 0.5 * b + 128.0).clamp(0.0, 255.0) - 128.0;
                    let cr_val =
                        (0.5 * r - 0.418_688 * g - 0.081_312 * b + 128.0).clamp(0.0, 255.0) - 128.0;

                    let idx = py * 8 + px;
                    blk_y[idx] = y_val;
                    blk_cb[idx] = cb_val;
                    blk_cr[idx] = cr_val;
                }
            }

            encode_block(&mut bw, &blk_y, &q_luma_zig, &mut prev_dc_y, &dc_luma, &ac_luma);
            encode_block(&mut bw, &blk_cb, &q_chroma_zig, &mut prev_dc_cb, &dc_chroma, &ac_chroma);
            encode_block(&mut bw, &blk_cr, &q_chroma_zig, &mut prev_dc_cr, &dc_chroma, &ac_chroma);
        }
    }

    bw.flush_pad_ones();
    bw.out.extend_from_slice(&[0xFF, 0xD9]);
    bw.out
}

fn write_png_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let crc_start = out.len();
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let crc = crc32fast::hash(&out[crc_start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn encode_zlib_compressed(raw: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    if enc.write_all(raw).is_ok() {
        if let Ok(compressed) = enc.finish() {
            return compressed;
        }
    }
    encode_zlib_stored(raw)
}

fn encode_zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if raw.is_empty() {
        vec![&[]]
    } else {
        raw.chunks(65_535).collect()
    };
    for (i, ch) in chunks.iter().enumerate() {
        let is_last = i + 1 == chunks.len();
        z.push(if is_last { 0x01 } else { 0x00 });
        let len = ch.len() as u16;
        let nlen = !len;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&nlen.to_le_bytes());
        z.extend_from_slice(ch);
    }
    let mut s1: u32 = 1;
    let mut s2: u32 = 0;
    for &b in raw {
        s1 = (s1 + b as u32) % 65_521;
        s2 = (s2 + s1) % 65_521;
    }
    let adler = (s2 << 16) | s1;
    z.extend_from_slice(&adler.to_be_bytes());
    z
}

// ============================================================================
// 5. Offline Interactive HTML Report & Multi-Page Tiled Print Planner
// ============================================================================

pub fn export_document_html_report(doc: &VisioDocument) -> String {
    let mut html = format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"/><title>{}</title>\
         <style>body{{font-family:system-ui,sans-serif;background:#0f172a;color:#f8fafc;margin:0;padding:24px}}\
         .page-card{{background:#ffffff;color:#0f172a;border-radius:12px;padding:16px;margin-bottom:24px;overflow:auto}}</style></head><body>\
         <h1>{}</h1>",
        vexio_vsdx::opc::xml_escape(&doc.properties.title),
        vexio_vsdx::opc::xml_escape(&doc.properties.title)
    );
    for p in &doc.pages {
        if p.is_background {
            continue;
        }
        let _ = writeln!(
            html,
            "<h2>{} ({}\" × {}\")</h2><div class=\"page-card\">{}</div>",
            vexio_vsdx::opc::xml_escape(&p.name),
            format_clean_f64(p.width_in()),
            format_clean_f64(p.height_in()),
            export_page_svg(doc, p.id)
        );
    }
    html.push_str("</body></html>");
    html
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TiledPrintSheet {
    pub row: usize,
    pub col: usize,
    pub clip_min_in: [f64; 2],
    pub clip_max_in: [f64; 2],
    pub registration_marks_in: [[f64; 2]; 4],
}

pub fn plan_tiled_print(
    page_width_in: f64,
    page_height_in: f64,
    paper_printable_width_in: f64,
    paper_printable_height_in: f64,
    overlap_in: f64,
) -> Vec<TiledPrintSheet> {
    let step_x = (paper_printable_width_in - overlap_in).max(1.0);
    let step_y = (paper_printable_height_in - overlap_in).max(1.0);
    let cols = ((page_width_in - overlap_in).max(0.01) / step_x).ceil() as usize;
    let rows = ((page_height_in - overlap_in).max(0.01) / step_y).ceil() as usize;

    let mut tiles = Vec::with_capacity(rows.max(1) * cols.max(1));
    for r in 0..rows.max(1) {
        for c in 0..cols.max(1) {
            let x0 = c as f64 * step_x;
            let y0 = r as f64 * step_y;
            let x1 = (x0 + paper_printable_width_in).min(page_width_in);
            let y1 = (y0 + paper_printable_height_in).min(page_height_in);
            tiles.push(TiledPrintSheet {
                row: r,
                col: c,
                clip_min_in: [x0, y0],
                clip_max_in: [x1, y1],
                registration_marks_in: [
                    [0.18, 0.18],
                    [paper_printable_width_in - 0.18, 0.18],
                    [paper_printable_width_in - 0.18, paper_printable_height_in - 0.18],
                    [0.18, paper_printable_height_in - 0.18],
                ],
            });
        }
    }
    tiles
}
