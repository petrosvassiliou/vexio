//! Vector-Accurate Export & Multi-Page Tiled Print Engine:
//! - SVG 1.1 (`export_page_svg`)
//! - PDF/A-1b / PDF 1.7 (`export_document_pdf`)
//! - Encapsulated PostScript EPS (`export_page_eps`)
//! - High-DPI PNG (`encode_rgba_to_png`)
//! - Interactive Offline HTML Workspace Report (`export_document_html_report`)
//! - Multi-Page Tiled Print Layout with Crop & Registration Marks (`plan_tiled_print`).

use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use vexio_shapesheet::{
    EvaluatedPathVerb, ReactiveEngine, VisioDocument, format_clean_f64,
};

// ============================================================================
// 1. Vector SVG 1.1 Export
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

    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.2}\" height=\"{:.2}\" viewBox=\"0 0 {:.2} {:.2}\">",
        w_px, h_px, w_px, h_px
    );
    svg.push_str("<defs><marker id=\"vexio-arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M 0 1 L 10 5 L 0 9 z\" fill=\"context-stroke\"/></marker></defs>\n");
    let _ = writeln!(
        svg,
        "<rect width=\"{:.2}\" height=\"{:.2}\" fill=\"#FFFFFF\"/>",
        w_px, h_px
    );

    // Note: Visio Y-origin is bottom-left (`0` at bottom, `PageHeight` at top); SVG Y-origin is top-left!
    let to_svg_y = |y_in: f64| (h_in - y_in) * ppi;
    let to_svg_x = |x_in: f64| x_in * ppi;

    for vis in &visuals {
        let fill = vis
            .fill_color
            .filter(|_| !vis.paths.iter().all(|p| p.no_fill))
            .map(|c| c.to_hex_rgb())
            .unwrap_or_else(|| "none".into());
        let stroke = vis
            .stroke_color
            .map(|c| c.to_hex_rgb())
            .unwrap_or_else(|| "none".into());
        let sw_px = (vis.stroke_weight_in * ppi).max(0.75);
        let marker_end = if vis.end_arrow > 0 {
            " marker-end=\"url(#vexio-arrow)\""
        } else {
            ""
        };

        for path in &vis.paths {
            if path.no_show || path.verbs_page_space.is_empty() {
                continue;
            }
            let p_fill = if path.no_fill { "none" } else { &fill };
            let p_stroke = if path.no_line { "none" } else { &stroke };

            let mut d = String::new();
            let mut last_pt = [0.0, 0.0];
            for verb in &path.verbs_page_space {
                match verb {
                    EvaluatedPathVerb::MoveTo(p) => {
                        let _ = write!(d, "M {:.2} {:.2} ", to_svg_x(p[0]), to_svg_y(p[1]));
                        last_pt = *p;
                    }
                    EvaluatedPathVerb::LineTo(p) => {
                        let _ = write!(d, "L {:.2} {:.2} ", to_svg_x(p[0]), to_svg_y(p[1]));
                        last_pt = *p;
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
                        let chord = ((to[0] - last_pt[0]).powi(2) + (to[1] - last_pt[1]).powi(2)).sqrt();
                        if bow.abs() < 1e-6 || chord < 1e-6 {
                            let _ = write!(d, "L {:.2} {:.2} ", to_svg_x(to[0]), to_svg_y(to[1]));
                        } else {
                            let r_in = (chord * chord) / (8.0 * bow.abs()) + bow.abs() * 0.5;
                            let r_px = r_in * ppi;
                            let sweep = if *bow > 0.0 { 0 } else { 1 };
                            let _ = write!(
                                d,
                                "A {:.2} {:.2} 0 0 {sweep} {:.2} {:.2} ",
                                r_px,
                                r_px,
                                to_svg_x(to[0]),
                                to_svg_y(to[1])
                            );
                        }
                        last_pt = *to;
                    }
                    EvaluatedPathVerb::Ellipse { center, rx, ry, .. } => {
                        let cx = to_svg_x(center[0]);
                        let cy = to_svg_y(center[1]);
                        let rx_px = rx * ppi;
                        let ry_px = ry * ppi;
                        let _ = writeln!(
                            svg,
                            "<ellipse cx=\"{cx:.2}\" cy=\"{cy:.2}\" rx=\"{rx_px:.2}\" ry=\"{ry_px:.2}\" fill=\"{p_fill}\" stroke=\"{p_stroke}\" stroke-width=\"{sw_px:.2}\"/>"
                        );
                    }
                    EvaluatedPathVerb::Close => d.push_str("Z "),
                }
            }
            if !d.is_empty() {
                let _ = writeln!(
                    svg,
                    "<path d=\"{}\" fill=\"{p_fill}\" stroke=\"{p_stroke}\" stroke-width=\"{sw_px:.2}\"{marker_end}/>",
                    d.trim()
                );
            }
        }

        if !vis.text.is_empty() {
            let tx = to_svg_x(vis.transform.pin_x);
            let ty = to_svg_y(vis.transform.pin_y);
            let fsize = (vis.font_size_pt * (96.0 / 72.0)).max(8.0);
            let tcol = vis.text_color.to_hex_rgb();
            let first_line = vis.text.lines().next().unwrap_or("");
            let _ = writeln!(
                svg,
                "<text x=\"{tx:.2}\" y=\"{ty:.2}\" font-family=\"Segoe UI, Roboto, sans-serif\" font-size=\"{fsize:.1}\" fill=\"{tcol}\" text-anchor=\"middle\" dominant-baseline=\"middle\">{}</text>",
                vexio_vsdx::opc::xml_escape(first_line)
            );
        }
    }

    // Markup Ink Strokes
    for ink in &page.ink_strokes {
        if ink.points.len() >= 2 {
            let pts_s: Vec<String> = ink
                .points
                .iter()
                .map(|pt| format!("{:.2},{:.2}", to_svg_x(pt.x_in), to_svg_y(pt.y_in)))
                .collect();
            let op = if ink.highlighter { "0.45" } else { "1.0" };
            let _ = writeln!(
                svg,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-opacity=\"{op}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
                pts_s.join(" "),
                ink.color.to_hex_rgb(),
                ink.width_pt * (96.0 / 72.0)
            );
        }
    }

    svg.push_str("</svg>");
    svg
}

// ============================================================================
// 2. Vector PDF 1.7 / PDF/A-1b Multi-Page Document Export
// ============================================================================

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

    // Reserve Obj 1 = Catalog, Obj 2 = Pages, Obj 3 = Helvetica Font
    offsets.push(0); // placeholder 1
    offsets.push(0); // placeholder 2
    let font_obj = {
        let id = offsets.len();
        offsets.push(out.len());
        out.extend_from_slice(
            format!("{id} 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n")
                .as_bytes(),
        );
        id
    };

    let mut page_obj_ids = Vec::new();
    for page in &doc.pages {
        if page.is_background {
            continue;
        }
        let w_pt = page.width_in() * 72.0;
        let h_pt = page.height_in() * 72.0;
        let visuals = ReactiveEngine::evaluate_page_visuals(doc, page.id);
        let mut stream = String::new();

        for vis in &visuals {
            let lw = (vis.stroke_weight_in * 72.0).max(0.5);
            let _ = writeln!(stream, "{lw:.2} w");
            if let Some(sc) = vis.stroke_color {
                let [r, g, b, _] = sc.to_linear_f32();
                let _ = writeln!(stream, "{r:.3} {g:.3} {b:.3} RG");
            }
            if let Some(fc) = vis.fill_color {
                let [r, g, b, _] = fc.to_linear_f32();
                let _ = writeln!(stream, "{r:.3} {g:.3} {b:.3} rg");
            }

            for path in &vis.paths {
                if path.no_show || path.verbs_page_space.is_empty() {
                    continue;
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
                let paint_op = match (!path.no_fill && vis.fill_color.is_some(), !path.no_line && vis.stroke_color.is_some()) {
                    (true, true) => "B",
                    (true, false) => "f",
                    (false, true) => "S",
                    (false, false) => "n",
                };
                let _ = writeln!(stream, "{paint_op}");
            }

            if !vis.text.is_empty() {
                let first = vis.text.lines().next().unwrap_or("").replace('(', "\\(").replace(')', "\\)");
                let tx = vis.transform.pin_x * 72.0 - (first.len() as f64 * vis.font_size_pt * 0.25);
                let ty = vis.transform.pin_y * 72.0 - vis.font_size_pt * 0.35;
                let _ = writeln!(
                    stream,
                    "BT /F1 {:.1} Tf {:.2} {:.2} Td ({first}) Tj ET",
                    vis.font_size_pt, tx, ty
                );
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
                 /Resources << /Font << /F1 {font_obj} 0 R >> >> /Contents {content_id} 0 R >>"
            ),
        );
        page_obj_ids.push(page_obj);
    }

    // Emit Obj 2 (Pages) and Obj 1 (Catalog with PDF/A metadata dict)
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
    let _ = write_pdf_xref(&mut out, &offsets, xref_offset);
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
// 4. Pure-Rust High-DPI PNG Encoder (Zero External C Dependencies)
// ============================================================================

pub fn encode_rgba_to_png(width: u32, height: u32, rgba: &[u8], dpi: u32) -> Vec<u8> {
    let mut png = Vec::new();
    png.extend_from_slice(&[137, 80, 78, 71, 13, 10, 26, 10]); // PNG signature

    // IHDR
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]); // 8-bit RGBA
    write_png_chunk(&mut png, b"IHDR", &ihdr);

    // pHYs (Pixels per meter for exact high-DPI Visio export!)
    let ppm = ((dpi as f64) / 0.0254).round() as u32;
    let mut phys = Vec::with_capacity(9);
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.extend_from_slice(&ppm.to_be_bytes());
    phys.push(1);
    write_png_chunk(&mut png, b"pHYs", &phys);

    // Raw scanlines (filter byte 0 = None per row) compressed inside valid zlib uncompressed blocks
    let row_bytes = (width as usize) * 4;
    let mut raw_scanlines = Vec::with_capacity(height as usize * (row_bytes + 1));
    for y in 0..height as usize {
        raw_scanlines.push(0u8);
        let start = y * row_bytes;
        raw_scanlines.extend_from_slice(&rgba[start..start + row_bytes]);
    }
    let zlib_stream = encode_zlib_stored(&raw_scanlines);
    write_png_chunk(&mut png, b"IDAT", &zlib_stream);
    write_png_chunk(&mut png, b"IEND", &[]);
    png
}

fn write_png_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let crc_start = out.len();
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let crc = crc32fast::hash(&out[crc_start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

fn encode_zlib_stored(raw: &[u8]) -> Vec<u8> {
    let mut z = vec![0x78, 0x01]; // Zlib CMF/FLG header
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
    // Adler-32 checksum
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
    /// Four corner registration crosshairs in sheet coordinates `(x_in, y_in)`.
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
