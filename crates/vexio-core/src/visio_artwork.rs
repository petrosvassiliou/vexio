//! Authentic 1:1 Microsoft Visio Stencil Vector Artwork & Multi-Color ShapeSheet Geometry Builder.
//!
//! Produces dual-representation Visio masters:
//! 1. Synchronous inline-ready vector SVG (`ForeignMediaData { media_type: "SVG", .. }`) with
//!    Visio-authentic 3D isometric bevels, linear/radial metallic & product gradients, and badges.
//! 2. Proportional multi-color `ShapeSheet` `Geometry1..N` sections (`=Width*..`, `=Height*..`,
//!    `=MIN(Width,Height)*..` with per-section `FillForegnd` & `LineColor`) so `wgpu` (`DX12`,
//!    `Metal`, `Vulkan`, `WebGPU`), 192-DPI `PNG`/`JPG` rasterizers, `PDF/A`, and `DXF` exports
//!    render matching multi-color vector geometry at every scale.

use vexio_shapesheet::{
    ForeignMediaData, GeometryRow, GeometryRowKind, GeometrySection, Unit, VisioShape,
    format_clean_f64,
};

struct DualArtwork {
    svg: String,
    geoms: Vec<(GeometrySection, Option<&'static str>, Option<&'static str>, f64)>,
}

impl DualArtwork {
    fn new(defs: &str) -> Self {
        let mut svg = String::with_capacity(1024);
        svg.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 100 100\">");
        if !defs.is_empty() {
            svg.push_str("<defs>");
            svg.push_str(defs);
            svg.push_str("</defs>");
        }
        Self {
            svg,
            geoms: Vec::new(),
        }
    }

    fn next_idx(&self) -> u32 {
        (self.geoms.len() as u32) + 1
    }

    fn add_poly(
        &mut self,
        pts_100: &[(f64, f64)],
        svg_fill: &str,
        svg_stroke: &str,
        sw_100: f64,
        ss_fill_rgb: Option<&'static str>,
        ss_stroke_rgb: Option<&'static str>,
    ) {
        if pts_100.len() < 2 {
            return;
        }
        self.svg.push_str("<polygon points=\"");
        for (i, &(x, y)) in pts_100.iter().enumerate() {
            if i > 0 {
                self.svg.push(' ');
            }
            self.svg.push_str(&format!("{x:.1},{y:.1}"));
        }
        self.svg.push_str(&format!(
            "\" fill=\"{svg_fill}\" stroke=\"{svg_stroke}\" stroke-width=\"{sw_100:.1}\" stroke-linejoin=\"round\"/>"
        ));

        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = if ss_fill_rgb.is_none() || svg_fill == "none" {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.no_line = if ss_stroke_rgb.is_none() || svg_stroke == "none" {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        let mut rows = Vec::with_capacity(pts_100.len() + 1);
        for (i, &(x, y)) in pts_100.iter().enumerate() {
            let fx = format!("=Width*{}", format_clean_f64(x / 100.0));
            let fy = format!("=Height*{}", format_clean_f64((100.0 - y) / 100.0));
            if i == 0 {
                rows.push(GeometryRow::move_to(1, fx, fy));
            } else {
                rows.push(GeometryRow::line_to((i + 1) as u32, fx, fy));
            }
        }
        let (x0, y0) = pts_100[0];
        rows.push(GeometryRow::line_to(
            (pts_100.len() + 1) as u32,
            format!("=Width*{}", format_clean_f64(x0 / 100.0)),
            format!("=Height*{}", format_clean_f64((100.0 - y0) / 100.0)),
        ));
        g.rows = rows;
        self.geoms
            .push((g, ss_fill_rgb, ss_stroke_rgb, (sw_100 * 0.65).clamp(0.75, 2.5)));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rx: f64,
        svg_fill: &str,
        svg_stroke: &str,
        sw_100: f64,
        ss_fill_rgb: Option<&'static str>,
        ss_stroke_rgb: Option<&'static str>,
    ) {
        self.svg.push_str(&format!(
            "<rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"{rx:.1}\" fill=\"{svg_fill}\" stroke=\"{svg_stroke}\" stroke-width=\"{sw_100:.1}\"/>"
        ));
        let pts = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = if ss_fill_rgb.is_none() || svg_fill == "none" {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.no_line = if ss_stroke_rgb.is_none() || svg_stroke == "none" {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        let mut rows = Vec::with_capacity(5);
        for (i, &(px, py)) in pts.iter().enumerate() {
            let fx = format!("=Width*{}", format_clean_f64(px / 100.0));
            let fy = format!("=Height*{}", format_clean_f64((100.0 - py) / 100.0));
            if i == 0 {
                rows.push(GeometryRow::move_to(1, fx, fy));
            } else {
                rows.push(GeometryRow::line_to((i + 1) as u32, fx, fy));
            }
        }
        rows.push(GeometryRow::line_to(
            5,
            format!("=Width*{}", format_clean_f64(x / 100.0)),
            format!("=Height*{}", format_clean_f64((100.0 - y) / 100.0)),
        ));
        g.rows = rows;
        self.geoms
            .push((g, ss_fill_rgb, ss_stroke_rgb, (sw_100 * 0.65).clamp(0.75, 2.5)));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        svg_fill: &str,
        svg_stroke: &str,
        sw_100: f64,
        ss_fill_rgb: Option<&'static str>,
        ss_stroke_rgb: Option<&'static str>,
    ) {
        self.svg.push_str(&format!(
            "<ellipse cx=\"{cx:.1}\" cy=\"{cy:.1}\" rx=\"{rx:.1}\" ry=\"{ry:.1}\" fill=\"{svg_fill}\" stroke=\"{svg_stroke}\" stroke-width=\"{sw_100:.1}\"/>"
        ));
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = if ss_fill_rgb.is_none() || svg_fill == "none" {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.no_line = if ss_stroke_rgb.is_none() || svg_stroke == "none" {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        let wy = 100.0 - cy;
        g.rows = vec![GeometryRow {
            ix: 1,
            kind: GeometryRowKind::Ellipse,
            x: format!("=Width*{}", format_clean_f64(cx / 100.0)),
            y: format!("=Height*{}", format_clean_f64(wy / 100.0)),
            a: Some(format!("=Width*{}", format_clean_f64((cx + rx) / 100.0))),
            b: Some(format!("=Height*{}", format_clean_f64(wy / 100.0))),
            c: Some(format!("=Width*{}", format_clean_f64(cx / 100.0))),
            d: Some(format!("=Height*{}", format_clean_f64((wy + ry) / 100.0))),
            e: None,
            deleted: false,
        }];
        self.geoms
            .push((g, ss_fill_rgb, ss_stroke_rgb, (sw_100 * 0.65).clamp(0.75, 2.5)));
    }

    fn add_lines(
        &mut self,
        segs: &[((f64, f64), (f64, f64))],
        svg_stroke: &str,
        sw_100: f64,
        ss_stroke_rgb: &'static str,
    ) {
        if segs.is_empty() {
            return;
        }
        let mut d = String::new();
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = "TRUE".into();
        let mut ix = 1u32;
        for &((x1, y1), (x2, y2)) in segs {
            d.push_str(&format!("M {x1:.1} {y1:.1} L {x2:.1} {y2:.1} "));
            g.rows.push(GeometryRow::move_to(
                ix,
                format!("=Width*{}", format_clean_f64(x1 / 100.0)),
                format!("=Height*{}", format_clean_f64((100.0 - y1) / 100.0)),
            ));
            g.rows.push(GeometryRow::line_to(
                ix + 1,
                format!("=Width*{}", format_clean_f64(x2 / 100.0)),
                format!("=Height*{}", format_clean_f64((100.0 - y2) / 100.0)),
            ));
            ix += 2;
        }
        self.svg.push_str(&format!(
            "<path d=\"{}\" fill=\"none\" stroke=\"{svg_stroke}\" stroke-width=\"{sw_100:.1}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",
            d.trim()
        ));
        self.geoms
            .push((g, None, Some(ss_stroke_rgb), (sw_100 * 0.65).clamp(0.75, 2.8)));
    }

    fn add_svg_only(&mut self, fragment: &str) {
        self.svg.push_str(fragment);
    }

    fn apply_to_shape(mut self, sh: &mut VisioShape, primary_fill_rgb: &str, primary_stroke_rgb: &str, label_below: bool) {
        self.svg.push_str("</svg>");
        sh.sheet.geometries.clear();
        sh.sheet.cells.retain(|k, _| !k.starts_with("geometry"));
        sh.sheet.set_formula(
            "FillForegnd",
            format!("=RGB({primary_fill_rgb})"),
            Some(Unit::Color),
        );
        sh.sheet.set_formula(
            "LineColor",
            format!("=RGB({primary_stroke_rgb})"),
            Some(Unit::Color),
        );
        sh.sheet
            .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
        sh.sheet
            .set_formula("Rounding", "0 in", Some(Unit::Inch));

        for (geom, fill_opt, stroke_opt, weight_pt) in self.geoms {
            let idx = geom.index;
            sh.sheet.add_geometry(geom);
            if let Some(f_rgb) = fill_opt {
                sh.sheet.set_formula(
                    format!("Geometry{idx}.FillForegnd"),
                    format!("=RGB({f_rgb})"),
                    Some(Unit::Color),
                );
            }
            if let Some(s_rgb) = stroke_opt {
                sh.sheet.set_formula(
                    format!("Geometry{idx}.LineColor"),
                    format!("=RGB({s_rgb})"),
                    Some(Unit::Color),
                );
            }
            sh.sheet.set_formula(
                format!("Geometry{idx}.LineWeight"),
                format!("{} pt", format_clean_f64(weight_pt)),
                Some(Unit::Point),
            );
        }

        if label_below {
            sh.sheet
                .set_formula("TxtPinX", "=Width*0.5", Some(Unit::Inch));
            sh.sheet
                .set_formula("TxtPinY", "-0.16 in", Some(Unit::Inch));
            sh.sheet
                .set_formula("Char.Size", "9 pt", Some(Unit::Point));
            sh.sheet
                .set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));
        }

        sh.sheet.set_user_cell(
            "ForeignSvgPrimary",
            "TRUE",
            "Render embedded 1:1 Visio SVG artwork as primary vector visual",
        );
        sh.foreign_data = Some(ForeignMediaData {
            media_type: "SVG".into(),
            rel_id: None,
            data: self.svg.into_bytes(),
        });
    }
}

/// Apply authentic 1:1 Microsoft Visio multi-color vector artwork and below-icon `TxtPinY` captions
/// for Microsoft 365/Office, Active Directory, Azure, Power Platform, Servers, Network & Rack,
/// Cloud Architecture, and BPMN 2.0 events/gateways. Returns `true` if handled.
pub fn apply_visio_1to1_icon_artwork(sh: &mut VisioShape, category: &str, role: &str) -> bool {
    match category {
        "Microsoft 365" => {
            build_microsoft_365_artwork(sh, role);
            true
        }
        "Microsoft Servers" => {
            build_microsoft_server_ad_artwork(sh, role);
            true
        }
        "Microsoft Azure" => {
            build_azure_artwork(sh, role);
            true
        }
        "Power Platform" => {
            build_power_platform_artwork(sh, role);
            true
        }
        "Servers" => {
            build_standard_server_artwork(sh, role);
            true
        }
        "Network" => {
            build_network_equipment_artwork(sh, role);
            true
        }
        "Cloud" => {
            build_cloud_arch_artwork(sh, role);
            true
        }
        _ => false,
    }
}

// ============================================================================
// 1. Microsoft 365 & Office 1:1 Product Iconography
// ============================================================================

fn build_office_app_tile(
    sh: &mut VisioShape,
    letter: &str,
    tile_rgb: &'static str,
    tile_hex: &str,
    top_rgb: &'static str,
    top_hex: &str,
    mid_rgb: &'static str,
    mid_hex: &str,
    bot_rgb: &'static str,
    bot_hex: &str,
    right_mode: &str,
) {
    let defs = format!(
        "<linearGradient id=\"g1\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"{top_hex}\"/><stop offset=\"100%\" stop-color=\"{tile_hex}\"/></linearGradient>"
    );
    let mut art = DualArtwork::new(&defs);

    match right_mode {
        "ppt_circle" => {
            // PowerPoint signature circular pie chart backing
            art.add_ellipse(58.0, 50.0, 36.0, 36.0, mid_hex, tile_hex, 1.8, Some(mid_rgb), Some(tile_rgb));
            art.add_poly(
                &[(58.0, 50.0), (58.0, 14.0), (88.0, 28.0), (94.0, 50.0)],
                top_hex,
                "none",
                0.0,
                Some(top_rgb),
                None,
            );
            art.add_lines(&[((58.0, 14.0), (58.0, 50.0)), ((58.0, 50.0), (94.0, 50.0))], "#FFFFFF", 2.2, "255,255,255");
        }
        "db_cyl" => {
            // Access signature 3-tier 3D burgundy cylinder
            art.add_rect(30.0, 20.0, 62.0, 60.0, 8.0, mid_hex, tile_hex, 1.8, Some(mid_rgb), Some(tile_rgb));
            art.add_ellipse(61.0, 22.0, 31.0, 10.0, top_hex, tile_hex, 1.8, Some(top_rgb), Some(tile_rgb));
            art.add_ellipse(61.0, 50.0, 31.0, 9.0, bot_hex, "#FFFFFF", 1.5, Some(bot_rgb), Some("255,255,255"));
            art.add_ellipse(61.0, 78.0, 31.0, 10.0, bot_hex, tile_hex, 1.8, Some(bot_rgb), Some(tile_rgb));
        }
        "visio_nodes" => {
            // Visio diagram card with diamond + connected node
            art.add_rect(28.0, 14.0, 64.0, 72.0, 7.0, "#EFF6FF", mid_hex, 2.2, Some("239,246,255"), Some(mid_rgb));
            art.add_poly(
                &[(64.0, 24.0), (78.0, 36.0), (64.0, 48.0), (50.0, 36.0)],
                top_hex,
                tile_hex,
                1.6,
                Some(top_rgb),
                Some(tile_rgb),
            );
            art.add_rect(52.0, 60.0, 30.0, 16.0, 3.0, mid_hex, tile_hex, 1.6, Some(mid_rgb), Some(tile_rgb));
            art.add_lines(&[((64.0, 48.0), (64.0, 60.0))], tile_hex, 2.4, tile_rgb);
        }
        "onenote_tabs" => {
            art.add_rect(26.0, 14.0, 56.0, 72.0, 6.0, mid_hex, tile_hex, 1.8, Some(mid_rgb), Some(tile_rgb));
            art.add_rect(82.0, 18.0, 11.0, 18.0, 3.0, top_hex, tile_hex, 1.4, Some(top_rgb), Some(tile_rgb));
            art.add_rect(82.0, 40.0, 11.0, 18.0, 3.0, mid_hex, tile_hex, 1.4, Some(mid_rgb), Some(tile_rgb));
            art.add_rect(82.0, 62.0, 11.0, 18.0, 3.0, bot_hex, tile_hex, 1.4, Some(bot_rgb), Some(tile_rgb));
        }
        "outlook_env" => {
            art.add_rect(30.0, 14.0, 62.0, 46.0, 5.0, top_hex, tile_hex, 1.8, Some(top_rgb), Some(tile_rgb));
            art.add_lines(&[((42.0, 26.0), (80.0, 26.0)), ((42.0, 36.0), (74.0, 36.0))], "#FFFFFF", 2.6, "255,255,255");
            art.add_poly(
                &[(28.0, 44.0), (94.0, 44.0), (94.0, 86.0), (28.0, 86.0)],
                mid_hex,
                tile_hex,
                1.8,
                Some(mid_rgb),
                Some(tile_rgb),
            );
            art.add_lines(&[((28.0, 45.0), (61.0, 68.0)), ((94.0, 45.0), (61.0, 68.0))], "#FFFFFF", 2.4, "255,255,255");
        }
        _ => {
            // Fluent 3-tier document/sheet bands (Word, Excel, Project, Publisher)
            art.add_rect(28.0, 14.0, 64.0, 24.0, 5.0, top_hex, "none", 0.0, Some(top_rgb), None);
            art.add_rect(28.0, 38.0, 64.0, 24.0, 0.0, mid_hex, "none", 0.0, Some(mid_rgb), None);
            art.add_rect(28.0, 62.0, 64.0, 24.0, 5.0, bot_hex, "none", 0.0, Some(bot_rgb), None);
            art.add_rect(28.0, 14.0, 64.0, 72.0, 6.0, "none", tile_hex, 1.8, None, Some(tile_rgb));
            if right_mode == "excel_grid" {
                art.add_lines(
                    &[((58.0, 14.0), (58.0, 86.0)), ((28.0, 38.0), (92.0, 38.0)), ((28.0, 62.0), (92.0, 62.0))],
                    "#FFFFFF",
                    1.8,
                    "255,255,255",
                );
            } else if right_mode == "project_gantt" {
                art.add_lines(
                    &[((48.0, 26.0), (76.0, 26.0)), ((54.0, 50.0), (84.0, 50.0)), ((46.0, 74.0), (72.0, 74.0))],
                    "#FFFFFF",
                    4.2,
                    "255,255,255",
                );
            } else {
                art.add_lines(
                    &[((50.0, 26.0), (84.0, 26.0)), ((50.0, 50.0), (84.0, 50.0)), ((50.0, 74.0), (76.0, 74.0))],
                    "#FFFFFF",
                    2.8,
                    "255,255,255",
                );
            }
        }
    }

    // Left Fluent App Shield Tile + Crisp Vector Letter
    art.add_svg_only("<rect x=\"8\" y=\"27\" width=\"46\" height=\"46\" rx=\"8\" fill=\"#000000\" fill-opacity=\"0.18\"/>");
    art.add_rect(6.0, 25.0, 46.0, 46.0, 8.0, "url(#g1)", "#FFFFFF", 1.8, Some(tile_rgb), Some("255,255,255"));
    draw_badge_letter(&mut art, letter, 29.0, 48.0, 13.0);
    art.apply_to_shape(sh, top_rgb, tile_rgb, true);
}

fn draw_badge_letter(art: &mut DualArtwork, letter: &str, cx: f64, cy: f64, half: f64) {
    art.add_svg_only(&format!(
        "<text x=\"{cx:.1}\" y=\"{:.1}\" font-family=\"Segoe UI, Roboto, Arial, sans-serif\" font-size=\"{:.1}\" font-weight=\"800\" fill=\"#FFFFFF\" text-anchor=\"middle\" dominant-baseline=\"central\">{letter}</text>",
        cy + 0.5,
        half * 2.05
    ));
    let l = cx - half * 0.68;
    let r = cx + half * 0.68;
    let t = cy - half * 0.78;
    let b = cy + half * 0.78;
    let segs: Vec<((f64, f64), (f64, f64))> = match letter {
        "W" => vec![
            ((l, t), (cx - half * 0.32, b)),
            ((cx - half * 0.32, b), (cx, cy - half * 0.15)),
            ((cx, cy - half * 0.15), (cx + half * 0.32, b)),
            ((cx + half * 0.32, b), (r, t)),
        ],
        "X" => vec![((l, t), (r, b)), ((r, t), (l, b))],
        "P" => vec![
            ((l, b), (l, t)),
            ((l, t), (r, t)),
            ((r, t), (r, cy)),
            ((r, cy), (l, cy)),
        ],
        "N" => vec![((l, b), (l, t)), ((l, t), (r, b)), ((r, b), (r, t))],
        "V" => vec![((l, t), (cx, b)), ((cx, b), (r, t))],
        "A" => vec![((l, b), (cx, t)), ((cx, t), (r, b)), ((l + 3.0, cy + 2.0), (r - 3.0, cy + 2.0))],
        "O" => vec![((l, t), (r, t)), ((r, t), (r, b)), ((r, b), (l, b)), ((l, b), (l, t))],
        "T" => vec![((l, t), (r, t)), ((cx, t), (cx, b))],
        "S" => vec![
            ((r, t), (l, t)),
            ((l, t), (l, cy)),
            ((l, cy), (r, cy)),
            ((r, cy), (r, b)),
            ((r, b), (l, b)),
        ],
        "F" => vec![((l, b), (l, t)), ((l, t), (r, t)), ((l, cy), (r - 3.0, cy))],
        "L" => vec![((l, t), (l, b)), ((l, b), (r, b))],
        _ => vec![((l, cy), (r, cy))],
    };
    let mut g = GeometrySection::new(art.next_idx());
    g.no_fill = "TRUE".into();
    let mut ix = 1u32;
    for ((x1, y1), (x2, y2)) in segs {
        g.rows.push(GeometryRow::move_to(
            ix,
            format!("=Width*{}", format_clean_f64(x1 / 100.0)),
            format!("=Height*{}", format_clean_f64((100.0 - y1) / 100.0)),
        ));
        g.rows.push(GeometryRow::line_to(
            ix + 1,
            format!("=Width*{}", format_clean_f64(x2 / 100.0)),
            format!("=Height*{}", format_clean_f64((100.0 - y2) / 100.0)),
        ));
        ix += 2;
    }
    art.geoms.push((g, None, Some("255,255,255"), 2.2));
}

fn build_microsoft_365_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "office_word" => build_office_app_tile(
            sh, "W", "24,90,189", "#185ABD", "65,165,238", "#41A5EE", "43,124,211", "#2B7CD3", "20,74,158", "#144A9E", "doc",
        ),
        "office_excel" => build_office_app_tile(
            sh, "X", "16,124,65", "#107C41", "51,196,129", "#33C481", "33,163,102", "#21A366", "16,124,65", "#107C41", "excel_grid",
        ),
        "office_powerpoint" => build_office_app_tile(
            sh, "P", "196,62,28", "#C43E1C", "255,143,107", "#FF8F6B", "237,108,71", "#ED6C47", "210,71,38", "#D24726", "ppt_circle",
        ),
        "office_onenote" => build_office_app_tile(
            sh, "N", "119,25,170", "#7719AA", "202,100,234", "#CA64EA", "174,75,213", "#AE4BD5", "147,50,191", "#9332BF", "onenote_tabs",
        ),
        "office_visio" => build_office_app_tile(
            sh, "V", "25,85,166", "#1955A6", "75,160,255", "#4BA0FF", "40,116,208", "#2874D0", "25,85,166", "#1955A6", "visio_nodes",
        ),
        "office_project" => build_office_app_tile(
            sh, "P", "16,124,65", "#107C41", "51,196,129", "#33C481", "33,163,102", "#21A366", "24,92,55", "#185C37", "project_gantt",
        ),
        "office_access" => build_office_app_tile(
            sh, "A", "164,55,58", "#A4373A", "224,128,131", "#E08083", "201,79,82", "#C94F52", "140,42,45", "#8C2A2D", "db_cyl",
        ),
        "office_publisher" => build_office_app_tile(
            sh, "P", "7,117,104", "#077568", "46,196,182", "#2EC4B6", "19,153,138", "#13998A", "7,117,104", "#077568", "doc",
        ),
        "m365_outlook" => build_office_app_tile(
            sh, "O", "0,120,212", "#0078D4", "80,217,255", "#50D9FF", "40,168,234", "#28A8EA", "0,90,158", "#005A9E", "outlook_env",
        ),
        "m365_teams" => {
            let mut art = DualArtwork::new(
                "<linearGradient id=\"g1\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#7B83EB\"/><stop offset=\"100%\" stop-color=\"#4B53BC\"/></linearGradient>",
            );
            // Secondary & Primary Teams People Silhouettes
            art.add_ellipse(78.0, 30.0, 9.0, 9.0, "#7B83EB", "#4B53BC", 1.4, Some("123,131,235"), Some("75,83,188"));
            art.add_rect(64.0, 42.0, 28.0, 32.0, 10.0, "#7B83EB", "#4B53BC", 1.4, Some("123,131,235"), Some("75,83,188"));
            art.add_ellipse(56.0, 24.0, 12.0, 12.0, "#5059C9", "#3B429F", 1.6, Some("80,89,201"), Some("59,66,159"));
            art.add_rect(36.0, 39.0, 42.0, 44.0, 12.0, "url(#g1)", "#3B429F", 1.6, Some("80,89,201"), Some("59,66,159"));
            art.add_rect(6.0, 27.0, 44.0, 44.0, 8.0, "#4B53BC", "#FFFFFF", 1.8, Some("75,83,188"), Some("255,255,255"));
            draw_badge_letter(&mut art, "T", 28.0, 49.0, 12.5);
            art.apply_to_shape(sh, "80,89,201", "59,66,159", true);
        }
        "m365_sharepoint" => {
            let mut art = DualArtwork::new("");
            art.add_ellipse(58.0, 30.0, 22.0, 22.0, "#036C70", "#FFFFFF", 1.5, Some("3,108,112"), Some("255,255,255"));
            art.add_ellipse(72.0, 56.0, 19.0, 19.0, "#1A9BA1", "#FFFFFF", 1.5, Some("26,155,161"), Some("255,255,255"));
            art.add_ellipse(54.0, 72.0, 15.0, 15.0, "#37C6D0", "#FFFFFF", 1.5, Some("55,198,208"), Some("255,255,255"));
            art.add_rect(8.0, 28.0, 44.0, 44.0, 8.0, "#038387", "#FFFFFF", 1.8, Some("3,131,135"), Some("255,255,255"));
            draw_badge_letter(&mut art, "S", 30.0, 50.0, 12.5);
            art.apply_to_shape(sh, "26,155,161", "3,108,112", true);
        }
        "m365_onedrive" => {
            let mut art = DualArtwork::new(
                "<linearGradient id=\"g1\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#28A8EA\"/><stop offset=\"100%\" stop-color=\"#0078D4\"/></linearGradient>",
            );
            art.add_ellipse(44.0, 44.0, 24.0, 22.0, "#0364B8", "none", 0.0, Some("3,100,184"), None);
            art.add_ellipse(66.0, 48.0, 20.0, 18.0, "#0078D4", "none", 0.0, Some("0,120,212"), None);
            art.add_ellipse(26.0, 58.0, 18.0, 16.0, "#28A8EA", "none", 0.0, Some("40,168,234"), None);
            art.add_rect(24.0, 52.0, 56.0, 22.0, 10.0, "url(#g1)", "none", 0.0, Some("40,168,234"), None);
            art.add_ellipse(76.0, 60.0, 15.0, 14.0, "#50D9FF", "none", 0.0, Some("80,217,255"), None);
            art.apply_to_shape(sh, "40,168,234", "0,120,212", true);
        }
        "m365_exchange" => {
            let mut art = DualArtwork::new(
                "<linearGradient id=\"g1\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#28A8EA\"/><stop offset=\"100%\" stop-color=\"#0078D4\"/></linearGradient>",
            );
            art.add_poly(
                &[(50.0, 8.0), (92.0, 50.0), (50.0, 92.0), (8.0, 50.0)],
                "url(#g1)",
                "#005A9E",
                2.0,
                Some("0,120,212"),
                Some("0,90,158"),
            );
            art.add_rect(28.0, 34.0, 44.0, 32.0, 4.0, "#FFFFFF", "#005A9E", 1.8, Some("255,255,255"), Some("0,90,158"));
            art.add_lines(
                &[((28.0, 35.0), (50.0, 52.0)), ((72.0, 35.0), (50.0, 52.0))],
                "#0078D4",
                2.4,
                "0,120,212",
            );
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "m365_tenant" => {
            // Microsoft 4-color Fluent Window + Cloud Tenant Frame (Container-capable!)
            let mut art = DualArtwork::new("");
            art.add_rect(4.0, 4.0, 92.0, 92.0, 10.0, "#F8FAFC", "#0078D4", 2.4, Some("248,250,252"), Some("0,120,212"));
            art.add_rect(14.0, 14.0, 14.0, 14.0, 2.0, "#F25022", "none", 0.0, Some("242,80,34"), None);
            art.add_rect(31.0, 14.0, 14.0, 14.0, 2.0, "#7FBA00", "none", 0.0, Some("127,186,0"), None);
            art.add_rect(14.0, 31.0, 14.0, 14.0, 2.0, "#00A4EF", "none", 0.0, Some("0,164,239"), None);
            art.add_rect(31.0, 31.0, 14.0, 14.0, 2.0, "#FFB900", "none", 0.0, Some("255,185,0"), None);
            art.add_lines(&[((54.0, 22.0), (84.0, 22.0)), ((54.0, 34.0), (76.0, 34.0))], "#0078D4", 3.0, "0,120,212");
            art.apply_to_shape(sh, "248,250,252", "0,120,212", true);
        }
        "m365_copilot" => {
            let mut art = DualArtwork::new(
                "<linearGradient id=\"g1\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#38BDF8\"/><stop offset=\"50%\" stop-color=\"#A855F7\"/><stop offset=\"100%\" stop-color=\"#F97316\"/></linearGradient>",
            );
            art.add_poly(
                &[(26.0, 18.0), (62.0, 18.0), (48.0, 82.0), (12.0, 82.0)],
                "#0EA5E9",
                "#FFFFFF",
                1.8,
                Some("14,165,233"),
                Some("255,255,255"),
            );
            art.add_poly(
                &[(52.0, 18.0), (88.0, 18.0), (74.0, 82.0), (38.0, 82.0)],
                "url(#g1)",
                "#FFFFFF",
                1.8,
                Some("168,85,247"),
                Some("255,255,255"),
            );
            art.apply_to_shape(sh, "168,85,247", "126,34,206", true);
        }
        "m365_defender" => {
            let mut art = DualArtwork::new(
                "<linearGradient id=\"g1\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#38BDF8\"/><stop offset=\"100%\" stop-color=\"#1D4ED8\"/></linearGradient>",
            );
            art.add_poly(
                &[(50.0, 8.0), (88.0, 22.0), (84.0, 60.0), (50.0, 92.0), (16.0, 60.0), (12.0, 22.0)],
                "url(#g1)",
                "#1E3A8A",
                2.2,
                Some("29,78,216"),
                Some("30,58,138"),
            );
            art.add_lines(
                &[((50.0, 12.0), (50.0, 88.0)), ((16.0, 44.0), (84.0, 44.0)), ((36.0, 52.0), (47.0, 64.0)), ((47.0, 64.0), (68.0, 38.0))],
                "#FFFFFF",
                3.2,
                "255,255,255",
            );
            art.apply_to_shape(sh, "29,78,216", "30,58,138", true);
        }
        "m365_intune" => {
            let mut art = DualArtwork::new("");
            art.add_rect(12.0, 16.0, 62.0, 46.0, 6.0, "#0078D4", "#004578", 2.0, Some("0,120,212"), Some("0,69,120"));
            art.add_rect(18.0, 22.0, 50.0, 34.0, 3.0, "#E0F2FE", "none", 0.0, Some("224,242,254"), None);
            art.add_rect(60.0, 38.0, 28.0, 48.0, 5.0, "#28A8EA", "#004578", 2.0, Some("40,168,234"), Some("0,69,120"));
            art.add_lines(&[((68.0, 62.0), (74.0, 68.0)), ((74.0, 68.0), (83.0, 55.0))], "#FFFFFF", 3.0, "255,255,255");
            art.apply_to_shape(sh, "0,120,212", "0,69,120", true);
        }
        "m365_purview" => {
            let mut art = DualArtwork::new("");
            art.add_ellipse(50.0, 50.0, 38.0, 24.0, "#F3E8FF", "#7E22CE", 3.2, Some("243,232,255"), Some("126,34,206"));
            art.add_ellipse(50.0, 50.0, 14.0, 14.0, "#9333EA", "#FFFFFF", 2.0, Some("147,51,234"), Some("255,255,255"));
            art.apply_to_shape(sh, "147,51,234", "126,34,206", true);
        }
        "m365_forms" => build_office_app_tile(
            sh, "F", "13,148,136", "#0D9488", "45,212,191", "#2DD4BF", "20,184,166", "#14B8A6", "15,118,110", "#0F766E", "doc",
        ),
        "m365_loop" => build_office_app_tile(
            sh, "L", "124,58,237", "#7C3AED", "167,139,250", "#A78BFA", "139,92,246", "#8B5CF6", "109,40,217", "#6D28D9", "doc",
        ),
        "m365_planner" => {
            let mut art = DualArtwork::new("");
            art.add_rect(12.0, 14.0, 76.0, 72.0, 8.0, "#ECFDF5", "#107C41", 2.2, Some("236,253,245"), Some("16,124,65"));
            art.add_rect(20.0, 24.0, 18.0, 52.0, 4.0, "#107C41", "none", 0.0, Some("16,124,65"), None);
            art.add_rect(42.0, 24.0, 18.0, 38.0, 4.0, "#21A366", "none", 0.0, Some("33,163,102"), None);
            art.add_rect(64.0, 24.0, 18.0, 26.0, 4.0, "#33C481", "none", 0.0, Some("51,196,129"), None);
            art.apply_to_shape(sh, "33,163,102", "16,124,65", true);
        }
        "m365_todo" => {
            let mut art = DualArtwork::new("");
            art.add_poly(
                &[(16.0, 52.0), (40.0, 76.0), (86.0, 24.0), (74.0, 14.0), (40.0, 52.0), (28.0, 40.0)],
                "#2563EB",
                "#1D4ED8",
                1.8,
                Some("37,99,235"),
                Some("29,78,216"),
            );
            art.apply_to_shape(sh, "37,99,235", "29,78,216", true);
        }
        "m365_stream" => {
            let mut art = DualArtwork::new("");
            art.add_rect(10.0, 18.0, 80.0, 64.0, 14.0, "#E11D48", "#9F1239", 2.0, Some("225,29,72"), Some("159,18,57"));
            art.add_poly(
                &[(40.0, 32.0), (70.0, 50.0), (40.0, 68.0)],
                "#FFFFFF",
                "none",
                0.0,
                Some("255,255,255"),
                None,
            );
            art.apply_to_shape(sh, "225,29,72", "159,18,57", true);
        }
        "m365_whiteboard" => {
            let mut art = DualArtwork::new("");
            art.add_rect(10.0, 14.0, 80.0, 68.0, 8.0, "#0284C7", "#0369A1", 2.0, Some("2,132,199"), Some("3,105,161"));
            art.add_rect(18.0, 22.0, 64.0, 48.0, 4.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
            art.add_rect(26.0, 30.0, 20.0, 18.0, 2.0, "#FEF08A", "#CA8A04", 1.4, Some("254,240,138"), Some("202,138,4"));
            art.add_lines(&[((52.0, 56.0), (74.0, 32.0))], "#2563EB", 4.0, "37,99,235");
            art.apply_to_shape(sh, "2,132,199", "3,105,161", true);
        }
        _ => {
            // Viva Engage / Viva Insights multi-petal Fluent emblem
            let mut art = DualArtwork::new("");
            art.add_ellipse(38.0, 38.0, 22.0, 22.0, "#0078D4", "#FFFFFF", 1.5, Some("0,120,212"), Some("255,255,255"));
            art.add_ellipse(62.0, 38.0, 22.0, 22.0, "#50E6FF", "#FFFFFF", 1.5, Some("80,230,255"), Some("255,255,255"));
            art.add_ellipse(50.0, 62.0, 22.0, 22.0, "#6366F1", "#FFFFFF", 1.5, Some("99,102,241"), Some("255,255,255"));
            art.apply_to_shape(sh, "0,120,212", "59,130,246", true);
        }
    }
}

// ============================================================================
// 2. Standard Servers & Microsoft Servers / Active Directory (3D Visio Chassis + Role Badges)
// ============================================================================

fn draw_3d_server_tower_base(art: &mut DualArtwork, accent_hex: &str, accent_rgb: &'static str) {
    // 3D Isometric Visio Server Tower (Front face + Top bevel + Right depth side)
    art.add_poly(
        &[(18.0, 16.0), (32.0, 8.0), (78.0, 8.0), (64.0, 16.0)],
        "#94A3B8",
        "#334155",
        1.6,
        Some("148,163,184"),
        Some("51,65,85"),
    );
    art.add_poly(
        &[(64.0, 16.0), (78.0, 8.0), (78.0, 78.0), (64.0, 88.0)],
        "#475569",
        "#1E293B",
        1.6,
        Some("71,85,105"),
        Some("30,41,59"),
    );
    art.add_rect(18.0, 16.0, 46.0, 72.0, 3.0, "url(#srvFront)", "#1E293B", 1.8, Some("100,116,139"), Some("30,41,59"));
    // Optical & Hot-Swap Drive Bays + Accent Bezel Stripe + Power LED
    art.add_rect(22.0, 21.0, 38.0, 5.0, 1.0, accent_hex, "none", 0.0, Some(accent_rgb), None);
    art.add_rect(24.0, 31.0, 34.0, 7.0, 1.0, "#1E293B", "none", 0.0, Some("30,41,59"), None);
    art.add_rect(24.0, 42.0, 34.0, 7.0, 1.0, "#1E293B", "none", 0.0, Some("30,41,59"), None);
    art.add_lines(
        &[((25.0, 58.0), (52.0, 58.0)), ((25.0, 64.0), (52.0, 64.0)), ((25.0, 70.0), (52.0, 70.0))],
        "#334155",
        2.0,
        "51,65,85",
    );
    art.add_ellipse(30.0, 80.0, 3.2, 3.2, "#22C55E", "#14532D", 1.0, Some("34,197,94"), Some("20,83,45"));
}

fn draw_server_role_badge(art: &mut DualArtwork, badge: &str) {
    match badge {
        "globe" => {
            art.add_ellipse(72.0, 68.0, 20.0, 20.0, "#0284C7", "#FFFFFF", 2.0, Some("2,132,199"), Some("255,255,255"));
            art.add_ellipse(72.0, 68.0, 9.0, 19.0, "none", "#E0F2FE", 1.6, None, Some("224,242,254"));
            art.add_lines(&[((53.0, 68.0), (91.0, 68.0)), ((56.0, 59.0), (88.0, 59.0)), ((56.0, 77.0), (88.0, 77.0))], "#E0F2FE", 1.5, "224,242,254");
        }
        "db" | "sql" => {
            let col = if badge == "sql" { "#DC2626" } else { "#D97706" };
            let rgb = if badge == "sql" { "220,38,38" } else { "217,119,6" };
            art.add_rect(54.0, 52.0, 36.0, 34.0, 4.0, col, "#FFFFFF", 1.8, Some(rgb), Some("255,255,255"));
            art.add_ellipse(72.0, 52.0, 18.0, 6.5, "#FDE047", "#B45309", 1.5, Some("253,224,71"), Some("180,83,9"));
            art.add_ellipse(72.0, 86.0, 18.0, 6.5, col, "#FFFFFF", 1.5, Some(rgb), Some("255,255,255"));
        }
        "folder" => {
            art.add_poly(
                &[(50.0, 56.0), (66.0, 56.0), (70.0, 62.0), (94.0, 62.0), (94.0, 90.0), (50.0, 90.0)],
                "#FACC15",
                "#B45309",
                1.8,
                Some("250,204,21"),
                Some("180,83,9"),
            );
        }
        "mail" => {
            art.add_rect(50.0, 58.0, 44.0, 30.0, 4.0, "#2563EB", "#FFFFFF", 2.0, Some("37,99,235"), Some("255,255,255"));
            art.add_lines(&[((51.0, 60.0), (72.0, 75.0)), ((93.0, 60.0), (72.0, 75.0))], "#FFFFFF", 2.4, "255,255,255");
        }
        "printer" => {
            art.add_rect(56.0, 52.0, 30.0, 12.0, 2.0, "#FFFFFF", "#475569", 1.6, Some("255,255,255"), Some("71,85,105"));
            art.add_rect(50.0, 62.0, 42.0, 20.0, 4.0, "#334155", "#FFFFFF", 1.8, Some("51,65,85"), Some("255,255,255"));
            art.add_rect(58.0, 76.0, 26.0, 14.0, 2.0, "#E2E8F0", "#475569", 1.4, Some("226,232,240"), Some("71,85,105"));
        }
        "ad_pyramid" => {
            art.add_poly(
                &[(72.0, 46.0), (95.0, 88.0), (49.0, 88.0)],
                "#0078D4",
                "#FFFFFF",
                2.0,
                Some("0,120,212"),
                Some("255,255,255"),
            );
            art.add_lines(&[((72.0, 48.0), (72.0, 88.0)), ((58.0, 72.0), (86.0, 72.0))], "#FDE047", 2.2, "253,224,71");
        }
        "shield_brick" => {
            art.add_poly(
                &[(72.0, 48.0), (94.0, 56.0), (90.0, 78.0), (72.0, 92.0), (54.0, 78.0), (50.0, 56.0)],
                "#DC2626",
                "#FFFFFF",
                2.0,
                Some("220,38,38"),
                Some("255,255,255"),
            );
            art.add_lines(&[((54.0, 66.0), (90.0, 66.0)), ((72.0, 52.0), (72.0, 88.0))], "#FCA5A5", 1.8, "252,165,165");
        }
        "pki_seal" => {
            art.add_ellipse(72.0, 66.0, 16.0, 16.0, "#F59E0B", "#FFFFFF", 2.0, Some("245,158,11"), Some("255,255,255"));
            art.add_poly(
                &[(64.0, 78.0), (60.0, 94.0), (72.0, 87.0), (84.0, 94.0), (80.0, 78.0)],
                "#DC2626",
                "none",
                0.0,
                Some("220,38,38"),
                None,
            );
        }
        "vms" => {
            art.add_rect(50.0, 56.0, 22.0, 22.0, 3.0, "#0EA5E9", "#FFFFFF", 1.8, Some("14,165,233"), Some("255,255,255"));
            art.add_rect(64.0, 66.0, 22.0, 22.0, 3.0, "#6366F1", "#FFFFFF", 1.8, Some("99,102,241"), Some("255,255,255"));
            art.add_rect(74.0, 50.0, 20.0, 20.0, 3.0, "#10B981", "#FFFFFF", 1.8, Some("16,185,129"), Some("255,255,255"));
        }
        "clock" => {
            art.add_ellipse(72.0, 70.0, 18.0, 18.0, "#FFFFFF", "#0284C7", 2.4, Some("255,255,255"), Some("2,132,199"));
            art.add_lines(&[((72.0, 70.0), (72.0, 58.0)), ((72.0, 70.0), (82.0, 74.0))], "#0F172A", 2.4, "15,23,42");
        }
        "pulse" => {
            art.add_rect(48.0, 54.0, 46.0, 34.0, 5.0, "#0F172A", "#38BDF8", 2.0, Some("15,23,42"), Some("56,189,248"));
            art.add_lines(
                &[((52.0, 72.0), (62.0, 72.0)), ((62.0, 72.0), (68.0, 59.0)), ((68.0, 59.0), (75.0, 82.0)), ((75.0, 82.0), (81.0, 70.0)), ((81.0, 70.0), (90.0, 70.0))],
                "#22C55E",
                2.4,
                "34,197,94",
            );
        }
        "backup" => {
            art.add_ellipse(72.0, 70.0, 18.0, 18.0, "#10B981", "#FFFFFF", 2.0, Some("16,185,129"), Some("255,255,255"));
            art.add_lines(&[((72.0, 58.0), (72.0, 78.0)), ((64.0, 66.0), (72.0, 58.0)), ((80.0, 66.0), (72.0, 58.0))], "#FFFFFF", 2.6, "255,255,255");
        }
        "arrows" => {
            art.add_ellipse(72.0, 70.0, 18.0, 18.0, "#0284C7", "#FFFFFF", 2.0, Some("2,132,199"), Some("255,255,255"));
            art.add_lines(&[((60.0, 65.0), (84.0, 65.0)), ((60.0, 75.0), (84.0, 75.0))], "#FFFFFF", 2.6, "255,255,255");
        }
        "gear" => {
            art.add_ellipse(72.0, 70.0, 18.0, 18.0, "#4F46E5", "#FFFFFF", 2.0, Some("79,70,229"), Some("255,255,255"));
            art.add_ellipse(72.0, 70.0, 7.0, 7.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
        }
        _ => {}
    }
}

fn build_standard_server_artwork(sh: &mut VisioShape, role: &str) {
    let srv_defs = "<linearGradient id=\"srvFront\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#CBD5E1\"/><stop offset=\"55%\" stop-color=\"#94A3B8\"/><stop offset=\"100%\" stop-color=\"#64748B\"/></linearGradient>";

    match role {
        "srv_rack" | "srv_rack_4u" => {
            let mut art = DualArtwork::new(srv_defs);
            let h = if role == "srv_rack_4u" { 68.0 } else { 46.0 };
            let y = (100.0 - h) * 0.5;
            // Rack mounting ears + brushed steel 19" chassis + NVMe drive caddies + status LEDs
            art.add_rect(4.0, y, 8.0, h, 2.0, "#475569", "#1E293B", 1.6, Some("71,85,105"), Some("30,41,59"));
            art.add_rect(88.0, y, 8.0, h, 2.0, "#475569", "#1E293B", 1.6, Some("71,85,105"), Some("30,41,59"));
            art.add_rect(12.0, y, 76.0, h, 3.0, "url(#srvFront)", "#1E293B", 2.0, Some("148,163,184"), Some("30,41,59"));
            for col in 0..4 {
                let bx = 18.0 + (col as f64) * 14.0;
                art.add_rect(bx, y + 8.0, 11.0, h - 16.0, 1.5, "#1E293B", "#334155", 1.2, Some("30,41,59"), Some("51,65,85"));
            }
            art.add_ellipse(80.0, y + h * 0.38, 3.5, 3.5, "#22C55E", "none", 0.0, Some("34,197,94"), None);
            art.add_ellipse(80.0, y + h * 0.66, 3.5, 3.5, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
        "srv_blade" => {
            let mut art = DualArtwork::new(srv_defs);
            art.add_rect(8.0, 12.0, 84.0, 76.0, 4.0, "#334155", "#0F172A", 2.2, Some("51,65,85"), Some("15,23,42"));
            for b in 0..6 {
                let bx = 14.0 + (b as f64) * 12.2;
                art.add_rect(bx, 18.0, 9.5, 52.0, 1.5, "url(#srvFront)", "#1E293B", 1.2, Some("148,163,184"), Some("30,41,59"));
                art.add_ellipse(bx + 4.8, 25.0, 2.0, 2.0, "#22C55E", "none", 0.0, Some("34,197,94"), None);
            }
            art.add_rect(14.0, 74.0, 72.0, 9.0, 2.0, "#0284C7", "none", 0.0, Some("2,132,199"), None);
            art.apply_to_shape(sh, "100,116,139", "15,23,42", true);
        }
        "srv_mainframe" => {
            let mut art = DualArtwork::new("");
            art.add_rect(10.0, 8.0, 80.0, 84.0, 4.0, "#1E293B", "#0F172A", 2.2, Some("30,41,59"), Some("15,23,42"));
            art.add_rect(14.0, 12.0, 22.0, 76.0, 2.0, "#334155", "#0F172A", 1.4, Some("51,65,85"), Some("15,23,42"));
            art.add_rect(39.0, 12.0, 22.0, 76.0, 2.0, "#334155", "#0F172A", 1.4, Some("51,65,85"), Some("15,23,42"));
            art.add_rect(64.0, 12.0, 22.0, 76.0, 2.0, "#334155", "#0F172A", 1.4, Some("51,65,85"), Some("15,23,42"));
            art.add_rect(42.0, 22.0, 16.0, 12.0, 1.5, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.apply_to_shape(sh, "30,41,59", "15,23,42", true);
        }
        _ => {
            let (accent_hex, accent_rgb, badge) = match role {
                "srv_web" => ("#0284C7", "2,132,199", "globe"),
                "srv_db" => ("#DC2626", "220,38,38", "db"),
                "srv_app" => ("#4F46E5", "79,70,229", "gear"),
                "srv_file" => ("#EAB308", "234,179,8", "folder"),
                "srv_print" => ("#475569", "71,85,105", "printer"),
                "srv_mail" => ("#2563EB", "37,99,235", "mail"),
                "srv_ldap" => ("#0078D4", "0,120,212", "ad_pyramid"),
                "srv_dns" | "srv_dhcp" | "srv_proxy" | "srv_ftp" => ("#0284C7", "2,132,199", "arrows"),
                "srv_ntp" => ("#0284C7", "2,132,199", "clock"),
                "srv_bastion" => ("#DC2626", "220,38,38", "shield_brick"),
                "srv_siem" | "srv_nms" => ("#10B981", "16,185,129", "pulse"),
                "srv_pki" => ("#D97706", "217,119,6", "pki_seal"),
                "srv_hypervisor" | "srv_rds" => ("#6366F1", "99,102,241", "vms"),
                "srv_backup" => ("#10B981", "16,185,129", "backup"),
                _ => ("#0078D4", "0,120,212", ""),
            };
            let mut art = DualArtwork::new(srv_defs);
            draw_3d_server_tower_base(&mut art, accent_hex, accent_rgb);
            if !badge.is_empty() {
                draw_server_role_badge(&mut art, badge);
            }
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
    }
}

fn build_microsoft_server_ad_artwork(sh: &mut VisioShape, role: &str) {
    let srv_defs = "<linearGradient id=\"srvFront\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#CBD5E1\"/><stop offset=\"55%\" stop-color=\"#94A3B8\"/><stop offset=\"100%\" stop-color=\"#64748B\"/></linearGradient>";
    match role {
        "ms_entra_id" | "ms_entra_connect" => {
            // Iconic Microsoft Entra ID faceted cyan/cobalt crystal prism
            let mut art = DualArtwork::new("");
            art.add_poly(&[(50.0, 8.0), (12.0, 62.0), (50.0, 92.0)], "#0078D4", "#FFFFFF", 1.6, Some("0,120,212"), Some("255,255,255"));
            art.add_poly(&[(50.0, 8.0), (88.0, 62.0), (50.0, 92.0)], "#50E6FF", "#0078D4", 1.6, Some("80,230,255"), Some("0,120,212"));
            art.add_poly(&[(50.0, 34.0), (74.0, 62.0), (50.0, 80.0), (26.0, 62.0)], "#38BDF8", "#FFFFFF", 1.4, Some("56,189,248"), Some("255,255,255"));
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "ms_ad_ou" => {
            let mut art = DualArtwork::new("");
            art.add_poly(
                &[(10.0, 22.0), (40.0, 22.0), (48.0, 32.0), (90.0, 32.0), (90.0, 84.0), (10.0, 84.0)],
                "#FACC15",
                "#B45309",
                2.0,
                Some("250,204,21"),
                Some("180,83,9"),
            );
            art.add_poly(&[(50.0, 42.0), (72.0, 76.0), (28.0, 76.0)], "#0078D4", "#FFFFFF", 1.8, Some("0,120,212"), Some("255,255,255"));
            art.apply_to_shape(sh, "250,204,21", "180,83,9", true);
        }
        "ms_gpo" => {
            let mut art = DualArtwork::new("");
            art.add_rect(18.0, 10.0, 64.0, 80.0, 5.0, "#FFFFFF", "#0078D4", 2.2, Some("255,255,255"), Some("0,120,212"));
            art.add_lines(&[((28.0, 26.0), (72.0, 26.0)), ((28.0, 38.0), (72.0, 38.0)), ((28.0, 50.0), (58.0, 50.0))], "#64748B", 2.4, "100,116,139");
            art.add_poly(&[(68.0, 54.0), (86.0, 70.0), (68.0, 86.0), (50.0, 70.0)], "#F59E0B", "#B45309", 1.8, Some("245,158,11"), Some("180,83,9"));
            art.apply_to_shape(sh, "255,255,255", "0,120,212", true);
        }
        "ms_ad_trust" => {
            let mut art = DualArtwork::new("");
            art.add_poly(&[(26.0, 24.0), (46.0, 76.0), (6.0, 76.0)], "#0078D4", "#004578", 1.8, Some("0,120,212"), Some("0,69,120"));
            art.add_poly(&[(74.0, 24.0), (94.0, 76.0), (54.0, 76.0)], "#107C41", "#064E3B", 1.8, Some("16,124,65"), Some("6,78,59"));
            art.add_lines(&[((40.0, 50.0), (60.0, 50.0))], "#D97706", 4.0, "217,119,6");
            art.apply_to_shape(sh, "0,120,212", "0,69,120", true);
        }
        "ms_win11_client" => {
            let mut art = DualArtwork::new("");
            art.add_rect(12.0, 14.0, 76.0, 52.0, 5.0, "#0078D4", "#1E293B", 2.2, Some("0,120,212"), Some("30,41,59"));
            art.add_rect(38.0, 30.0, 11.0, 11.0, 1.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
            art.add_rect(51.0, 30.0, 11.0, 11.0, 1.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
            art.add_rect(38.0, 43.0, 11.0, 11.0, 1.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
            art.add_rect(51.0, 43.0, 11.0, 11.0, 1.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
            art.add_poly(&[(36.0, 66.0), (64.0, 66.0), (72.0, 82.0), (28.0, 82.0)], "#94A3B8", "#334155", 1.8, Some("148,163,184"), Some("51,65,85"));
            art.apply_to_shape(sh, "0,120,212", "30,41,59", true);
        }
        _ => {
            let badge = match role {
                "ms_ad_dc" | "ms_rodc" | "ms_adfs" => "ad_pyramid",
                "ms_sql_server" | "ms_ssrs" | "ms_ssis" => "sql",
                "ms_iis" => "globe",
                "ms_exchange_srv" => "mail",
                "ms_sharepoint_srv" => "gear",
                "ms_ad_cs" => "pki_seal",
                "ms_hyperv" | "ms_failover_cluster" => "vms",
                "ms_system_center" => "pulse",
                "ms_wsus" => "backup",
                "ms_dns_dhcp" | "ms_win_dns" | "ms_win_dhcp" => "arrows",
                _ => "",
            };
            let mut art = DualArtwork::new(srv_defs);
            draw_3d_server_tower_base(&mut art, "#0078D4", "0,120,212");
            // Windows 4-pane blue badge on top bezel
            art.add_rect(24.0, 20.0, 6.0, 6.0, 0.5, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.add_rect(31.0, 20.0, 6.0, 6.0, 0.5, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            if !badge.is_empty() {
                draw_server_role_badge(&mut art, badge);
            }
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
    }
}

// ============================================================================
// 3. Visio / Cisco-Standard 3D Network & Rack Equipment
// ============================================================================

fn build_network_equipment_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "net_router" | "net_edge_router" | "net_isr_router" => {
            // Iconic Visio / Cisco 3D Shaded Turquoise Cylindrical Router Puck + 4 White Arrows!
            let mut art = DualArtwork::new(
                "<linearGradient id=\"rCyl\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\"><stop offset=\"0%\" stop-color=\"#0369A1\"/><stop offset=\"45%\" stop-color=\"#0EA5E9\"/><stop offset=\"100%\" stop-color=\"#075985\"/></linearGradient>",
            );
            art.add_rect(10.0, 36.0, 80.0, 34.0, 4.0, "url(#rCyl)", "#0C4A6E", 2.0, Some("3,105,161"), Some("12,74,110"));
            art.add_ellipse(50.0, 70.0, 40.0, 18.0, "url(#rCyl)", "#0C4A6E", 2.0, Some("3,105,161"), Some("12,74,110"));
            art.add_ellipse(50.0, 36.0, 40.0, 20.0, "#38BDF8", "#0C4A6E", 2.0, Some("56,189,248"), Some("12,74,110"));
            // 4 crisp white Visio/Cisco router arrows on top face
            art.add_lines(
                &[
                    ((24.0, 36.0), (43.0, 36.0)),
                    ((38.0, 31.0), (44.0, 36.0)),
                    ((38.0, 41.0), (44.0, 36.0)),
                    ((76.0, 36.0), (57.0, 36.0)),
                    ((62.0, 31.0), (56.0, 36.0)),
                    ((62.0, 41.0), (56.0, 36.0)),
                    ((50.0, 21.0), (50.0, 31.0)),
                    ((46.0, 26.0), (50.0, 32.0)),
                    ((54.0, 26.0), (50.0, 32.0)),
                    ((50.0, 40.0), (50.0, 51.0)),
                    ((46.0, 46.0), (50.0, 52.0)),
                    ((54.0, 46.0), (50.0, 52.0)),
                ],
                "#FFFFFF",
                2.8,
                "255,255,255",
            );
            art.apply_to_shape(sh, "14,165,233", "12,74,110", true);
        }
        "switch_1u" | "net_l2_switch" | "net_poe_switch" | "net_fiber_switch" | "net_din_switch" => {
            // Iconic Visio / Cisco 3D Isometric Teal Switch Box + Opposing Arrows + RJ45 Ports!
            let top_col = if role == "net_poe_switch" { "#14B8A6" } else { "#0EA5E9" };
            let front_col = if role == "net_poe_switch" { "#0F766E" } else { "#0369A1" };
            let front_rgb = if role == "net_poe_switch" { "15,118,110" } else { "3,105,161" };
            let mut art = DualArtwork::new("");
            art.add_poly(
                &[(8.0, 44.0), (24.0, 20.0), (94.0, 20.0), (78.0, 44.0)],
                top_col,
                "#0F172A",
                1.8,
                Some("14,165,233"),
                Some("15,23,42"),
            );
            art.add_poly(
                &[(78.0, 44.0), (94.0, 20.0), (94.0, 54.0), (78.0, 78.0)],
                "#0C4A6E",
                "#0F172A",
                1.8,
                Some("12,74,110"),
                Some("15,23,42"),
            );
            art.add_rect(8.0, 44.0, 70.0, 34.0, 2.0, front_col, "#0F172A", 1.8, Some(front_rgb), Some("15,23,42"));
            // Opposing white switching arrows on top face
            art.add_lines(
                &[
                    ((28.0, 28.0), (62.0, 28.0)),
                    ((56.0, 24.0), (63.0, 28.0)),
                    ((36.0, 36.0), (72.0, 36.0)),
                    ((42.0, 32.0), (35.0, 36.0)),
                ],
                "#FFFFFF",
                2.5,
                "255,255,255",
            );
            // Front RJ45 port blocks + green link LEDs
            for p in 0..6 {
                let px = 14.0 + (p as f64) * 9.5;
                art.add_rect(px, 53.0, 6.8, 12.0, 1.0, "#0F172A", "#38BDF8", 1.0, Some("15,23,42"), Some("56,189,248"));
            }
            art.add_ellipse(72.0, 58.0, 2.8, 2.8, "#22C55E", "none", 0.0, Some("34,197,94"), None);
            art.apply_to_shape(sh, front_rgb, "15,23,42", true);
        }
        "net_l3_switch" | "net_chassis_sw" => {
            // Iconic Visio / Cisco 3D Square Layer-3 Multilayer Switch with Central Hub + 8 White Arrows!
            let mut art = DualArtwork::new(
                "<linearGradient id=\"l3g\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#0EA5E9\"/><stop offset=\"100%\" stop-color=\"#0369A1\"/></linearGradient>",
            );
            art.add_poly(&[(12.0, 18.0), (24.0, 8.0), (92.0, 8.0), (80.0, 18.0)], "#38BDF8", "#0C4A6E", 1.8, Some("56,189,248"), Some("12,74,110"));
            art.add_poly(&[(80.0, 18.0), (92.0, 8.0), (92.0, 78.0), (80.0, 90.0)], "#075985", "#0C4A6E", 1.8, Some("7,89,133"), Some("12,74,110"));
            art.add_rect(12.0, 18.0, 68.0, 72.0, 4.0, "url(#l3g)", "#0C4A6E", 2.0, Some("2,132,199"), Some("12,74,110"));
            art.add_ellipse(46.0, 54.0, 11.0, 11.0, "none", "#FFFFFF", 2.6, None, Some("255,255,255"));
            art.add_lines(
                &[
                    ((46.0, 43.0), (46.0, 25.0)),
                    ((46.0, 65.0), (46.0, 83.0)),
                    ((35.0, 54.0), (19.0, 54.0)),
                    ((57.0, 54.0), (73.0, 54.0)),
                    ((38.0, 46.0), (25.0, 33.0)),
                    ((54.0, 46.0), (67.0, 33.0)),
                    ((38.0, 62.0), (25.0, 75.0)),
                    ((54.0, 62.0), (67.0, 75.0)),
                ],
                "#FFFFFF",
                2.6,
                "255,255,255",
            );
            art.apply_to_shape(sh, "2,132,199", "12,74,110", true);
        }
        "firewall" | "net_ngfw" | "net_waf" | "net_ids_ips" | "net_ddos" => {
            // Iconic Visio 3D Crimson Mortared Brick Wall Firewall (+ Inspection Shield/Flame on NGFW/WAF!)
            let mut art = DualArtwork::new(
                "<linearGradient id=\"fwg\" x1=\"0\" y1=\"0\" x2=\"0\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#EF4444\"/><stop offset=\"100%\" stop-color=\"#B91C1C\"/></linearGradient>",
            );
            art.add_poly(&[(8.0, 22.0), (20.0, 12.0), (92.0, 12.0), (80.0, 22.0)], "#F87171", "#7F1D1D", 1.8, Some("248,113,113"), Some("127,29,29"));
            art.add_poly(&[(80.0, 22.0), (92.0, 12.0), (92.0, 76.0), (80.0, 86.0)], "#991B1B", "#7F1D1D", 1.8, Some("153,27,27"), Some("127,29,29"));
            art.add_rect(8.0, 22.0, 72.0, 64.0, 3.0, "url(#fwg)", "#7F1D1D", 2.0, Some("220,38,38"), Some("127,29,29"));
            // Crisp white staggered brick mortar joints
            art.add_lines(
                &[
                    ((8.0, 38.0), (80.0, 38.0)),
                    ((8.0, 54.0), (80.0, 54.0)),
                    ((8.0, 70.0), (80.0, 70.0)),
                    ((32.0, 22.0), (32.0, 38.0)),
                    ((56.0, 22.0), (56.0, 38.0)),
                    ((20.0, 38.0), (20.0, 54.0)),
                    ((44.0, 38.0), (44.0, 54.0)),
                    ((68.0, 38.0), (68.0, 54.0)),
                    ((32.0, 54.0), (32.0, 70.0)),
                    ((56.0, 54.0), (56.0, 70.0)),
                    ((20.0, 70.0), (20.0, 86.0)),
                    ((44.0, 70.0), (44.0, 86.0)),
                    ((68.0, 70.0), (68.0, 86.0)),
                ],
                "#FEE2E2",
                2.0,
                "254,226,226",
            );
            if role != "firewall" {
                art.add_poly(
                    &[(74.0, 44.0), (94.0, 52.0), (90.0, 76.0), (74.0, 92.0), (58.0, 76.0), (54.0, 52.0)],
                    "#F59E0B",
                    "#FFFFFF",
                    2.0,
                    Some("245,158,11"),
                    Some("255,255,255"),
                );
            }
            art.apply_to_shape(sh, "220,38,38", "127,29,29", true);
        }
        "net_wifi" | "net_mesh_ap" => {
            let mut art = DualArtwork::new("");
            art.add_ellipse(50.0, 68.0, 36.0, 14.0, "#E2E8F0", "#0284C7", 2.0, Some("226,232,240"), Some("2,132,199"));
            art.add_ellipse(50.0, 66.0, 8.0, 5.0, "#0284C7", "none", 0.0, Some("2,132,199"), None);
            art.add_svg_only("<path d=\"M 28 46 A 30 30 0 0 1 72 46 M 18 32 A 44 44 0 0 1 82 32\" fill=\"none\" stroke=\"#0284C7\" stroke-width=\"4.0\" stroke-linecap=\"round\"/>");
            art.add_lines(&[((30.0, 44.0), (50.0, 32.0)), ((50.0, 32.0), (70.0, 44.0)), ((20.0, 30.0), (50.0, 14.0)), ((50.0, 14.0), (80.0, 30.0))], "#0284C7", 2.8, "2,132,199");
            art.apply_to_shape(sh, "226,232,240", "2,132,199", true);
        }
        "rack_42u" => {
            // 42U Enterprise Rack Cabinet (Container-friendly with top header!)
            let mut art = DualArtwork::new("");
            art.add_rect(8.0, 4.0, 84.0, 92.0, 3.0, "#1E293B", "#0F172A", 2.4, Some("30,41,59"), Some("15,23,42"));
            art.add_rect(16.0, 10.0, 68.0, 80.0, 1.5, "#F8FAFC", "#475569", 1.6, Some("248,250,252"), Some("71,85,105"));
            art.add_rect(18.0, 14.0, 64.0, 10.0, 1.0, "#0284C7", "#0F172A", 1.2, Some("2,132,199"), Some("15,23,42"));
            art.add_rect(18.0, 27.0, 64.0, 10.0, 1.0, "#64748B", "#0F172A", 1.2, Some("100,116,139"), Some("15,23,42"));
            art.add_rect(18.0, 74.0, 64.0, 12.0, 1.0, "#334155", "#0F172A", 1.2, Some("51,65,85"), Some("15,23,42"));
            art.apply_to_shape(sh, "248,250,252", "15,23,42", true);
        }
        "cloud_vpc" => {
            // Large VPC Cloud Boundary Container (Label at top!)
            let mut art = DualArtwork::new("");
            art.add_rect(4.0, 4.0, 92.0, 92.0, 8.0, "#F8FAFC", "#0284C7", 2.0, Some("248,250,252"), Some("2,132,199"));
            art.add_ellipse(18.0, 16.0, 8.0, 6.0, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.apply_to_shape(sh, "248,250,252", "2,132,199", false);
            sh.sheet.set_formula("TxtPinY", "=Height - 0.18 in", Some(Unit::Inch));
        }
        "net_laptop" => {
            let mut art = DualArtwork::new("");
            art.add_rect(18.0, 16.0, 64.0, 46.0, 4.0, "#1E293B", "#0F172A", 2.0, Some("30,41,59"), Some("15,23,42"));
            art.add_rect(22.0, 20.0, 56.0, 38.0, 2.0, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.add_poly(&[(18.0, 62.0), (82.0, 62.0), (92.0, 78.0), (8.0, 78.0)], "#CBD5E1", "#475569", 1.8, Some("203,213,225"), Some("71,85,105"));
            art.apply_to_shape(sh, "203,213,225", "30,41,59", true);
        }
        "net_voip" => {
            let mut art = DualArtwork::new("");
            art.add_rect(22.0, 18.0, 62.0, 64.0, 7.0, "#334155", "#0F172A", 2.0, Some("51,65,85"), Some("15,23,42"));
            art.add_rect(14.0, 14.0, 16.0, 72.0, 8.0, "#1E293B", "#0F172A", 1.8, Some("30,41,59"), Some("15,23,42"));
            art.add_rect(36.0, 24.0, 40.0, 18.0, 2.0, "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.add_lines(&[((40.0, 52.0), (72.0, 52.0)), ((40.0, 62.0), (72.0, 62.0)), ((40.0, 72.0), (72.0, 72.0))], "#CBD5E1", 3.0, "203,213,225");
            art.apply_to_shape(sh, "51,65,85", "15,23,42", true);
        }
        "net_printer" => {
            let mut art = DualArtwork::new("");
            art.add_rect(26.0, 12.0, 48.0, 22.0, 2.0, "#FFFFFF", "#64748B", 1.8, Some("255,255,255"), Some("100,116,139"));
            art.add_rect(12.0, 32.0, 76.0, 38.0, 6.0, "#475569", "#1E293B", 2.0, Some("71,85,105"), Some("30,41,59"));
            art.add_rect(26.0, 58.0, 48.0, 26.0, 2.0, "#F8FAFC", "#64748B", 1.6, Some("248,250,252"), Some("100,116,139"));
            art.add_lines(&[((34.0, 68.0), (66.0, 68.0)), ((34.0, 76.0), (58.0, 76.0))], "#94A3B8", 2.0, "148,163,184");
            art.apply_to_shape(sh, "71,85,105", "30,41,59", true);
        }
        "net_ip_cam" => {
            let mut art = DualArtwork::new("");
            art.add_ellipse(50.0, 46.0, 34.0, 26.0, "#E2E8F0", "#334155", 2.0, Some("226,232,240"), Some("51,65,85"));
            art.add_ellipse(50.0, 50.0, 18.0, 18.0, "#1E293B", "#0F172A", 1.8, Some("30,41,59"), Some("15,23,42"));
            art.add_ellipse(50.0, 50.0, 8.0, 8.0, "#0284C7", "#38BDF8", 1.5, Some("2,132,199"), Some("56,189,248"));
            art.apply_to_shape(sh, "226,232,240", "51,65,85", true);
        }
        "db_cluster" => {
            let mut art = DualArtwork::new("");
            art.add_rect(14.0, 24.0, 48.0, 56.0, 6.0, "#0284C7", "#075985", 2.0, Some("2,132,199"), Some("7,89,133"));
            art.add_ellipse(38.0, 24.0, 24.0, 9.0, "#38BDF8", "#075985", 1.8, Some("56,189,248"), Some("7,89,133"));
            art.add_rect(42.0, 34.0, 44.0, 50.0, 6.0, "#0369A1", "#FFFFFF", 1.8, Some("3,105,161"), Some("255,255,255"));
            art.add_ellipse(64.0, 34.0, 22.0, 8.0, "#7DD3FC", "#075985", 1.8, Some("125,211,252"), Some("7,89,133"));
            art.apply_to_shape(sh, "2,132,199", "7,89,133", true);
        }
        _ => {
            // 3D Network / Rack Appliance (Load Balancer, VPN, SD-WAN, Patch Panel, NAS/SAN, UPS, PDU)
            let mut art = DualArtwork::new("");
            art.add_poly(&[(8.0, 34.0), (22.0, 18.0), (94.0, 18.0), (80.0, 34.0)], "#64748B", "#1E293B", 1.8, Some("100,116,139"), Some("30,41,59"));
            art.add_poly(&[(80.0, 34.0), (94.0, 18.0), (94.0, 62.0), (80.0, 78.0)], "#334155", "#1E293B", 1.8, Some("51,65,85"), Some("30,41,59"));
            art.add_rect(8.0, 34.0, 72.0, 44.0, 3.0, "#1E293B", "#0F172A", 2.0, Some("30,41,59"), Some("15,23,42"));
            art.add_lines(
                &[((18.0, 56.0), (42.0, 56.0)), ((42.0, 56.0), (66.0, 44.0)), ((42.0, 56.0), (66.0, 68.0))],
                "#38BDF8",
                3.0,
                "56,189,248",
            );
            art.add_ellipse(22.0, 44.0, 3.0, 3.0, "#22C55E", "none", 0.0, Some("34,197,94"), None);
            art.apply_to_shape(sh, "51,65,85", "15,23,42", true);
        }
    }
}

// ============================================================================
// 4. Microsoft Azure, Power Platform & Multi-Cloud Architecture Icons
// ============================================================================

fn build_azure_artwork(sh: &mut VisioShape, role: &str) {
    let az_defs = "<linearGradient id=\"azg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0%\" stop-color=\"#50E6FF\"/><stop offset=\"100%\" stop-color=\"#0078D4\"/></linearGradient>";
    match role {
        "az_resource_group" | "az_mgmt_group" | "az_vnet" => {
            let mut art = DualArtwork::new("");
            art.add_rect(4.0, 4.0, 92.0, 92.0, 8.0, "#F8FAFC", "#0078D4", 2.2, Some("248,250,252"), Some("0,120,212"));
            art.add_poly(&[(12.0, 24.0), (22.0, 10.0), (32.0, 24.0)], "#0078D4", "none", 0.0, Some("0,120,212"), None);
            art.apply_to_shape(sh, "248,250,252", "0,120,212", false);
            sh.sheet.set_formula("TxtPinY", "=Height - 0.16 in", Some(Unit::Inch));
        }
        "az_vm" | "az_vmss" => {
            let mut art = DualArtwork::new(az_defs);
            art.add_rect(12.0, 14.0, 76.0, 54.0, 6.0, "url(#azg)", "#005A9E", 2.0, Some("0,120,212"), Some("0,90,158"));
            art.add_poly(&[(50.0, 24.0), (66.0, 33.0), (66.0, 50.0), (50.0, 59.0), (34.0, 50.0), (34.0, 33.0)], "#FFFFFF", "#005A9E", 1.6, Some("255,255,255"), Some("0,90,158"));
            art.add_poly(&[(36.0, 68.0), (64.0, 68.0), (72.0, 84.0), (28.0, 84.0)], "#94A3B8", "#475569", 1.8, Some("148,163,184"), Some("71,85,105"));
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "az_aks" | "az_container_apps" => {
            let mut art = DualArtwork::new("");
            for &(cx, cy) in &[(34.0, 34.0), (66.0, 34.0), (50.0, 64.0)] {
                art.add_poly(
                    &[(cx, cy - 18.0), (cx + 16.0, cy - 9.0), (cx + 16.0, cy + 9.0), (cx, cy + 18.0), (cx - 16.0, cy + 9.0), (cx - 16.0, cy - 9.0)],
                    "#7C3AED",
                    "#FFFFFF",
                    2.0,
                    Some("124,58,237"),
                    Some("255,255,255"),
                );
            }
            art.apply_to_shape(sh, "124,58,237", "91,33,182", true);
        }
        "az_functions" => {
            let mut art = DualArtwork::new("");
            art.add_lines(&[((26.0, 32.0), (10.0, 50.0)), ((10.0, 50.0), (26.0, 68.0)), ((74.0, 32.0), (90.0, 50.0)), ((90.0, 50.0), (74.0, 68.0))], "#0078D4", 4.0, "0,120,212");
            art.add_poly(&[(56.0, 10.0), (32.0, 52.0), (50.0, 52.0), (42.0, 90.0), (70.0, 44.0), (52.0, 44.0)], "#FACC15", "#CA8A04", 1.8, Some("250,204,21"), Some("202,138,4"));
            art.apply_to_shape(sh, "250,204,21", "0,120,212", true);
        }
        "az_sql_db" | "az_sql_cosmos" => {
            let mut art = DualArtwork::new(az_defs);
            art.add_rect(20.0, 20.0, 60.0, 62.0, 6.0, "url(#azg)", "#005A9E", 2.0, Some("0,120,212"), Some("0,90,158"));
            art.add_ellipse(50.0, 20.0, 30.0, 11.0, "#E0F2FE", "#005A9E", 2.0, Some("224,242,254"), Some("0,90,158"));
            art.add_svg_only("<text x=\"50\" y=\"58\" font-family=\"Segoe UI, Roboto, sans-serif\" font-size=\"18\" font-weight=\"800\" fill=\"#FFFFFF\" text-anchor=\"middle\">SQL</text>");
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "az_firewall" => {
            build_network_equipment_artwork(sh, "net_ngfw");
        }
        "az_key_vault" => {
            let mut art = DualArtwork::new("");
            art.add_ellipse(50.0, 50.0, 38.0, 38.0, "#FACC15", "#B45309", 2.4, Some("250,204,21"), Some("180,83,9"));
            art.add_ellipse(42.0, 44.0, 11.0, 11.0, "#FFFFFF", "#B45309", 2.4, Some("255,255,255"), Some("180,83,9"));
            art.add_lines(&[((50.0, 52.0), (70.0, 72.0)), ((64.0, 66.0), (58.0, 72.0))], "#FFFFFF", 4.0, "255,255,255");
            art.apply_to_shape(sh, "250,204,21", "180,83,9", true);
        }
        _ => {
            let mut art = DualArtwork::new(az_defs);
            art.add_rect(14.0, 14.0, 72.0, 72.0, 14.0, "url(#azg)", "#005A9E", 2.0, Some("0,120,212"), Some("0,90,158"));
            art.add_poly(&[(50.0, 24.0), (74.0, 70.0), (56.0, 70.0), (46.0, 50.0), (34.0, 70.0), (24.0, 70.0)], "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
    }
}

fn build_power_platform_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "pp_managed_env" => {
            let mut art = DualArtwork::new("");
            art.add_rect(4.0, 4.0, 92.0, 92.0, 8.0, "#F8FAFC", "#742774", 2.2, Some("248,250,252"), Some("116,39,116"));
            art.apply_to_shape(sh, "248,250,252", "116,39,116", false);
            sh.sheet.set_formula("TxtPinY", "=Height - 0.16 in", Some(Unit::Inch));
        }
        "pp_powerbi" | "pp_pbi_model" | "pp_pbi_paginated" => {
            let mut art = DualArtwork::new("");
            art.add_rect(18.0, 48.0, 18.0, 40.0, 4.0, "#FDE047", "#CA8A04", 1.6, Some("253,224,71"), Some("202,138,4"));
            art.add_rect(41.0, 30.0, 18.0, 58.0, 4.0, "#FACC15", "#CA8A04", 1.6, Some("250,204,21"), Some("202,138,4"));
            art.add_rect(64.0, 12.0, 18.0, 76.0, 4.0, "#EAB308", "#A16207", 1.6, Some("234,179,8"), Some("161,98,7"));
            art.apply_to_shape(sh, "250,204,21", "161,98,7", true);
        }
        "pp_automate" | "pp_rpa" => {
            let mut art = DualArtwork::new("");
            art.add_poly(&[(10.0, 20.0), (56.0, 20.0), (88.0, 50.0), (56.0, 80.0), (10.0, 80.0), (38.0, 50.0)], "#0066FF", "#0044CC", 1.8, Some("0,102,255"), Some("0,68,204"));
            art.add_poly(&[(10.0, 20.0), (56.0, 20.0), (68.0, 32.0), (26.0, 62.0)], "#38BDF8", "none", 0.0, Some("56,189,248"), None);
            art.apply_to_shape(sh, "0,102,255", "0,68,204", true);
        }
        "pp_powerapps" | "pp_model_app" => {
            let mut art = DualArtwork::new("");
            art.add_poly(&[(50.0, 10.0), (90.0, 50.0), (50.0, 90.0), (10.0, 50.0)], "#742774", "#4A154B", 2.0, Some("116,39,116"), Some("74,21,75"));
            art.add_poly(&[(50.0, 26.0), (74.0, 50.0), (50.0, 74.0), (26.0, 50.0)], "#C084FC", "#FFFFFF", 1.6, Some("192,132,252"), Some("255,255,255"));
            art.apply_to_shape(sh, "116,39,116", "74,21,75", true);
        }
        "pp_dataverse" | "ms_fabric" => {
            let mut art = DualArtwork::new("");
            art.add_rect(14.0, 18.0, 72.0, 20.0, 6.0, "#10B981", "#047857", 1.8, Some("16,185,129"), Some("4,120,87"));
            art.add_rect(14.0, 42.0, 72.0, 20.0, 6.0, "#059669", "#047857", 1.8, Some("5,150,105"), Some("4,120,87"));
            art.add_rect(14.0, 66.0, 72.0, 20.0, 6.0, "#047857", "#064E3B", 1.8, Some("4,120,87"), Some("6,78,59"));
            art.apply_to_shape(sh, "16,185,129", "4,120,87", true);
        }
        _ => {
            // Dynamics 365 / Copilot Studio / AI Builder / Power Pages
            let mut art = DualArtwork::new("");
            art.add_poly(&[(20.0, 12.0), (84.0, 34.0), (54.0, 52.0), (20.0, 42.0)], "#0078D4", "#002050", 1.8, Some("0,120,212"), Some("0,32,80"));
            art.add_poly(&[(36.0, 48.0), (84.0, 34.0), (84.0, 72.0), (20.0, 90.0)], "#28A8EA", "#002050", 1.8, Some("40,168,234"), Some("0,32,80"));
            art.apply_to_shape(sh, "0,120,212", "0,32,80", true);
        }
    }
}

fn build_cloud_arch_artwork(sh: &mut VisioShape, role: &str) {
    let (bg_hex, bg_rgb, border_hex, border_rgb) = match role {
        "cloud_vm" | "cloud_asg" | "cloud_spot" | "cloud_fn" | "cloud_batch" => ("#FF9900", "255,153,0", "#B45309", "180,83,9"),
        "cloud_bucket" | "cloud_ebs" | "cloud_glacier" => ("#16A34A", "22,163,74", "#15803D", "21,128,61"),
        "cloud_sql" | "cloud_nosql" | "cloud_cache" | "cloud_dwh" => ("#3B82F6", "59,130,246", "#1D4ED8", "29,78,216"),
        "cloud_iam" | "cloud_kms" => ("#EF4444", "239,68,68", "#B91C1C", "185,28,28"),
        _ => ("#6366F1", "99,102,241", "#4338CA", "67,56,202"),
    };
    let mut art = DualArtwork::new("");
    art.add_rect(12.0, 12.0, 76.0, 76.0, 14.0, bg_hex, border_hex, 2.2, Some(bg_rgb), Some(border_rgb));
    if role.contains("k8s") || role.contains("pod") {
        art.add_poly(
            &[(50.0, 22.0), (74.0, 36.0), (74.0, 64.0), (50.0, 78.0), (26.0, 64.0), (26.0, 36.0)],
            "none",
            "#FFFFFF",
            3.0,
            None,
            Some("255,255,255"),
        );
        art.add_ellipse(50.0, 50.0, 10.0, 10.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
    } else if role.contains("sql") || role.contains("nosql") || role.contains("dwh") || role.contains("cache") {
        art.add_ellipse(50.0, 32.0, 22.0, 8.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
        art.add_lines(&[((28.0, 32.0), (28.0, 68.0)), ((72.0, 32.0), (72.0, 68.0)), ((28.0, 68.0), (72.0, 68.0))], "#FFFFFF", 3.0, "255,255,255");
    } else if role == "cloud_fn" {
        art.add_lines(&[((34.0, 72.0), (54.0, 28.0)), ((46.0, 48.0), (66.0, 72.0))], "#FFFFFF", 4.2, "255,255,255");
    } else {
        art.add_rect(28.0, 28.0, 44.0, 44.0, 6.0, "none", "#FFFFFF", 3.0, None, Some("255,255,255"));
        art.add_rect(38.0, 38.0, 24.0, 24.0, 3.0, "#FFFFFF", "none", 0.0, Some("255,255,255"), None);
    }
    art.apply_to_shape(sh, bg_rgb, border_rgb, true);
}
