//! Authentic 1:1 Microsoft Visio Stencil Vector Artwork & Isotropic Multi-Color ShapeSheet Geometry Builder.
//!
//! Every stencil master is authored in an **isotropic `(0..vb_w, 0..vb_h)` coordinate frame** matching
//! its exact canonical `(Width, Height)` (`vb_w = Width * 100`, `vb_h = Height * 100`, i.e., `1 unit == 0.01 in`
//! identically in both X and Y). All shapes emit pure multi-color `ShapeSheet` `Geometry1..N` sections
//! (including rounded `ArcTo` corners, ellipses, 3D bevels, role badges, and vector glyphs), guaranteeing:
//! 1. **Zero aspect-ratio distortion** (circles are 100% mathematical circles, tiles have uniform corner radii,
//!    1U switches/servers and 42U racks fill their exact physical proportions without double-squashing).
//! 2. **100% identical multi-color vector rendering across all 4 platforms** — macOS (`StencilVectorPreviewCanvas`
//!    + Metal `wgpu`), Windows (WinUI 3 `Viewbox/Path` + DirectX 12 `wgpu`), Android/Googlebook (Compose `Canvas`
//!    + Vulkan `wgpu`), ChromeOS PWA (SVG + WebGPU), PDF/A, PNG, JPG, and `.vsdx`/`.vssx` round-trips.

use vexio_shapesheet::{
    GeometryRow, GeometryRowKind, GeometrySection, Unit, VisioShape, format_clean_f64,
};

struct IsoArtwork {
    vb_w: f64,
    vb_h: f64,
    geoms: Vec<(
        GeometrySection,
        Option<&'static str>,
        Option<&'static str>,
        f64,
    )>,
}

impl IsoArtwork {
    fn for_shape(sh: &VisioShape) -> Self {
        let w_in = sh.sheet.get_f64("Width").unwrap_or(1.05).max(0.20);
        let h_in = sh.sheet.get_f64("Height").unwrap_or(1.05).max(0.20);
        Self {
            vb_w: w_in * 100.0,
            vb_h: h_in * 100.0,
            geoms: Vec::new(),
        }
    }

    fn next_idx(&self) -> u32 {
        (self.geoms.len() as u32) + 1
    }

    #[inline]
    fn fx(&self, x: f64) -> String {
        format!("=Width*{}", format_clean_f64(x / self.vb_w))
    }

    #[inline]
    fn fy(&self, y: f64) -> String {
        format!("=Height*{}", format_clean_f64((self.vb_h - y) / self.vb_h))
    }

    #[inline]
    fn bow_formula(&self, bow_units: f64) -> String {
        let vb_min = self.vb_w.min(self.vb_h).max(1.0);
        format!("=MIN(Width,Height)*{}", format_clean_f64(bow_units / vb_min))
    }

    fn add_poly(
        &mut self,
        pts: &[(f64, f64)],
        fill_rgb: Option<&'static str>,
        stroke_rgb: Option<&'static str>,
        weight_pt: f64,
    ) {
        if pts.len() < 3 {
            return;
        }
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = if fill_rgb.is_none() {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.no_line = if stroke_rgb.is_none() {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        let mut rows = Vec::with_capacity(pts.len() + 1);
        for (i, &(x, y)) in pts.iter().enumerate() {
            if i == 0 {
                rows.push(GeometryRow::move_to(1, self.fx(x), self.fy(y)));
            } else {
                rows.push(GeometryRow::line_to((i + 1) as u32, self.fx(x), self.fy(y)));
            }
        }
        let (x0, y0) = pts[0];
        rows.push(GeometryRow::line_to(
            (pts.len() + 1) as u32,
            self.fx(x0),
            self.fy(y0),
        ));
        g.rows = rows;
        self.geoms
            .push((g, fill_rgb, stroke_rgb, weight_pt.clamp(0.75, 3.2)));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_rect(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        rx: f64,
        fill_rgb: Option<&'static str>,
        stroke_rgb: Option<&'static str>,
        weight_pt: f64,
    ) {
        let r = rx.min(w * 0.48).min(h * 0.48).max(0.0);
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = if fill_rgb.is_none() {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.no_line = if stroke_rgb.is_none() {
            "TRUE".into()
        } else {
            "FALSE".into()
        };

        if r <= 0.35 {
            g.rows = vec![
                GeometryRow::move_to(1, self.fx(x), self.fy(y)),
                GeometryRow::line_to(2, self.fx(x + w), self.fy(y)),
                GeometryRow::line_to(3, self.fx(x + w), self.fy(y + h)),
                GeometryRow::line_to(4, self.fx(x), self.fy(y + h)),
                GeometryRow::line_to(5, self.fx(x), self.fy(y)),
            ];
        } else {
            // Clockwise in screen space `(X right, Y down)` -> convex outward sagitta `bow = +0.29289322 * r`
            let bow = self.bow_formula(0.292_893_22 * r);
            g.rows = vec![
                GeometryRow::move_to(1, self.fx(x + r), self.fy(y)),
                GeometryRow::line_to(2, self.fx(x + w - r), self.fy(y)),
                GeometryRow::arc_to(3, self.fx(x + w), self.fy(y + r), &bow),
                GeometryRow::line_to(4, self.fx(x + w), self.fy(y + h - r)),
                GeometryRow::arc_to(5, self.fx(x + w - r), self.fy(y + h), &bow),
                GeometryRow::line_to(6, self.fx(x + r), self.fy(y + h)),
                GeometryRow::arc_to(7, self.fx(x), self.fy(y + h - r), &bow),
                GeometryRow::line_to(8, self.fx(x), self.fy(y + r)),
                GeometryRow::arc_to(9, self.fx(x + r), self.fy(y), &bow),
            ];
        }
        self.geoms
            .push((g, fill_rgb, stroke_rgb, weight_pt.clamp(0.75, 3.2)));
    }

    #[allow(clippy::too_many_arguments)]
    fn add_ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        fill_rgb: Option<&'static str>,
        stroke_rgb: Option<&'static str>,
        weight_pt: f64,
    ) {
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = if fill_rgb.is_none() {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.no_line = if stroke_rgb.is_none() {
            "TRUE".into()
        } else {
            "FALSE".into()
        };
        g.rows = vec![GeometryRow {
            ix: 1,
            kind: GeometryRowKind::Ellipse,
            x: self.fx(cx),
            y: self.fy(cy),
            a: Some(self.fx(cx + rx)),
            b: Some(self.fy(cy)),
            c: Some(self.fx(cx)),
            d: Some(self.fy(cy - ry)),
            e: None,
            deleted: false,
        }];
        self.geoms
            .push((g, fill_rgb, stroke_rgb, weight_pt.clamp(0.75, 3.2)));
    }

    fn add_lines(
        &mut self,
        segs: &[((f64, f64), (f64, f64))],
        stroke_rgb: &'static str,
        weight_pt: f64,
    ) {
        if segs.is_empty() {
            return;
        }
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = "TRUE".into();
        let mut ix = 1u32;
        for &((x1, y1), (x2, y2)) in segs {
            g.rows.push(GeometryRow::move_to(ix, self.fx(x1), self.fy(y1)));
            g.rows
                .push(GeometryRow::line_to(ix + 1, self.fx(x2), self.fy(y2)));
            ix += 2;
        }
        self.geoms
            .push((g, None, Some(stroke_rgb), weight_pt.clamp(0.75, 3.5)));
    }

    fn add_open_arc(
        &mut self,
        p0: (f64, f64),
        p1: (f64, f64),
        bow_units: f64,
        stroke_rgb: &'static str,
        weight_pt: f64,
    ) {
        let mut g = GeometrySection::new(self.next_idx());
        g.no_fill = "TRUE".into();
        let bow = self.bow_formula(bow_units);
        g.rows = vec![
            GeometryRow::move_to(1, self.fx(p0.0), self.fy(p0.1)),
            GeometryRow::arc_to(2, self.fx(p1.0), self.fy(p1.1), &bow),
        ];
        self.geoms
            .push((g, None, Some(stroke_rgb), weight_pt.clamp(0.75, 3.5)));
    }

    fn apply_to_shape(
        self,
        sh: &mut VisioShape,
        primary_fill_rgb: &str,
        primary_stroke_rgb: &str,
        label_below: bool,
    ) {
        sh.sheet.geometries.clear();
        sh.sheet.cells.retain(|k, _| !k.starts_with("geometry"));
        sh.foreign_data = None;
        sh.sheet
            .cells
            .remove("user.foreignsvgprimary");

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
    }
}

/// Apply authentic 1:1 Microsoft Visio multi-color vector artwork and below-icon `TxtPinY` captions
/// for Microsoft 365/Office, Active Directory, Azure, Power Platform, Servers, Network & Rack,
/// and Cloud Architecture. Returns `true` if handled.
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
// 1. Microsoft 365 & Office 1:1 Product Iconography (`105 × 105` Isotropic Square)
// ============================================================================

fn build_office_app_tile(
    sh: &mut VisioShape,
    letter: &str,
    tile_rgb: &'static str,
    top_rgb: &'static str,
    mid_rgb: &'static str,
    bot_rgb: &'static str,
    right_mode: &str,
) {
    let mut art = IsoArtwork::for_shape(sh);
    let s = art.vb_w / 100.0;

    match right_mode {
        "ppt_circle" => {
            art.add_ellipse(58.0 * s, 50.0 * s, 35.0 * s, 35.0 * s, Some(mid_rgb), Some(tile_rgb), 1.4);
            art.add_poly(
                &[
                    (58.0 * s, 50.0 * s),
                    (58.0 * s, 15.0 * s),
                    (83.0 * s, 25.0 * s),
                    (93.0 * s, 50.0 * s),
                ],
                Some(top_rgb),
                Some("255,255,255"),
                1.4,
            );
            art.add_lines(
                &[
                    ((58.0 * s, 15.0 * s), (58.0 * s, 50.0 * s)),
                    ((58.0 * s, 50.0 * s), (93.0 * s, 50.0 * s)),
                ],
                "255,255,255",
                1.8,
            );
        }
        "db_cyl" => {
            art.add_rect(30.0 * s, 22.0 * s, 62.0 * s, 56.0 * s, 6.0 * s, Some(mid_rgb), Some(tile_rgb), 1.4);
            art.add_ellipse(61.0 * s, 22.0 * s, 31.0 * s, 10.0 * s, Some(top_rgb), Some(tile_rgb), 1.4);
            art.add_ellipse(61.0 * s, 50.0 * s, 31.0 * s, 8.5 * s, Some(bot_rgb), Some("255,255,255"), 1.2);
            art.add_ellipse(61.0 * s, 78.0 * s, 31.0 * s, 10.0 * s, Some(bot_rgb), Some(tile_rgb), 1.4);
        }
        "visio_nodes" => {
            art.add_rect(28.0 * s, 14.0 * s, 64.0 * s, 72.0 * s, 8.0 * s, Some("239,246,255"), Some(mid_rgb), 1.6);
            art.add_poly(
                &[
                    (66.0 * s, 22.0 * s),
                    (81.0 * s, 35.0 * s),
                    (66.0 * s, 48.0 * s),
                    (51.0 * s, 35.0 * s),
                ],
                Some(top_rgb),
                Some(tile_rgb),
                1.4,
            );
            art.add_rect(52.0 * s, 60.0 * s, 28.0 * s, 16.0 * s, 4.0 * s, Some(mid_rgb), Some(tile_rgb), 1.4);
            art.add_lines(&[((66.0 * s, 48.0 * s), (66.0 * s, 60.0 * s))], tile_rgb, 1.8);
        }
        "onenote_tabs" => {
            art.add_rect(26.0 * s, 14.0 * s, 56.0 * s, 72.0 * s, 7.0 * s, Some(mid_rgb), Some(tile_rgb), 1.5);
            art.add_rect(81.0 * s, 18.0 * s, 12.0 * s, 18.0 * s, 3.0 * s, Some(top_rgb), Some(tile_rgb), 1.2);
            art.add_rect(81.0 * s, 40.0 * s, 12.0 * s, 18.0 * s, 3.0 * s, Some(mid_rgb), Some(tile_rgb), 1.2);
            art.add_rect(81.0 * s, 62.0 * s, 12.0 * s, 18.0 * s, 3.0 * s, Some(bot_rgb), Some(tile_rgb), 1.2);
            art.add_lines(
                &[
                    ((48.0 * s, 32.0 * s), (72.0 * s, 32.0 * s)),
                    ((48.0 * s, 48.0 * s), (72.0 * s, 48.0 * s)),
                    ((48.0 * s, 64.0 * s), (68.0 * s, 64.0 * s)),
                ],
                "255,255,255",
                1.8,
            );
        }
        "outlook_env" => {
            art.add_rect(30.0 * s, 14.0 * s, 60.0 * s, 46.0 * s, 6.0 * s, Some(top_rgb), Some(tile_rgb), 1.4);
            art.add_lines(
                &[
                    ((44.0 * s, 26.0 * s), (78.0 * s, 26.0 * s)),
                    ((44.0 * s, 36.0 * s), (72.0 * s, 36.0 * s)),
                ],
                "255,255,255",
                1.8,
            );
            art.add_rect(28.0 * s, 44.0 * s, 66.0 * s, 42.0 * s, 6.0 * s, Some(mid_rgb), Some(tile_rgb), 1.5);
            art.add_lines(
                &[
                    ((30.0 * s, 46.0 * s), (61.0 * s, 67.0 * s)),
                    ((92.0 * s, 46.0 * s), (61.0 * s, 67.0 * s)),
                ],
                "255,255,255",
                1.8,
            );
        }
        _ => {
            // Fluent 3-tier rounded document card (Word, Excel, Project, Publisher, Forms, Loop)
            art.add_rect(28.0 * s, 14.0 * s, 64.0 * s, 72.0 * s, 8.0 * s, Some(mid_rgb), None, 0.8);
            art.add_rect(28.0 * s, 14.0 * s, 64.0 * s, 25.0 * s, 8.0 * s, Some(top_rgb), None, 0.8);
            art.add_rect(28.0 * s, 61.0 * s, 64.0 * s, 25.0 * s, 8.0 * s, Some(bot_rgb), None, 0.8);
            art.add_rect(28.0 * s, 36.0 * s, 64.0 * s, 28.0 * s, 0.0, Some(mid_rgb), None, 0.8);
            art.add_rect(28.0 * s, 14.0 * s, 64.0 * s, 72.0 * s, 8.0 * s, None, Some(tile_rgb), 1.5);
            if right_mode == "excel_grid" {
                art.add_lines(
                    &[
                        ((60.0 * s, 15.0 * s), (60.0 * s, 85.0 * s)),
                        ((29.0 * s, 38.0 * s), (91.0 * s, 38.0 * s)),
                        ((29.0 * s, 62.0 * s), (91.0 * s, 62.0 * s)),
                    ],
                    "255,255,255",
                    1.5,
                );
            } else if right_mode == "project_gantt" {
                art.add_rect(50.0 * s, 23.0 * s, 28.0 * s, 6.5 * s, 2.5 * s, Some("255,255,255"), None, 0.8);
                art.add_rect(56.0 * s, 47.0 * s, 28.0 * s, 6.5 * s, 2.5 * s, Some("255,255,255"), None, 0.8);
                art.add_rect(48.0 * s, 71.0 * s, 24.0 * s, 6.5 * s, 2.5 * s, Some("255,255,255"), None, 0.8);
            } else {
                art.add_lines(
                    &[
                        ((52.0 * s, 26.0 * s), (83.0 * s, 26.0 * s)),
                        ((52.0 * s, 50.0 * s), (83.0 * s, 50.0 * s)),
                        ((52.0 * s, 74.0 * s), (76.0 * s, 74.0 * s)),
                    ],
                    "255,255,255",
                    2.0,
                );
            }
        }
    }

    // Subtle elevation shadow + Left Fluent App Shield Tile + Crisp Vector Letter
    art.add_rect(9.0 * s, 28.0 * s, 46.0 * s, 46.0 * s, 9.0 * s, Some("148,163,184"), None, 0.8);
    art.add_rect(6.0 * s, 25.0 * s, 46.0 * s, 46.0 * s, 9.0 * s, Some(tile_rgb), Some("255,255,255"), 1.6);
    draw_badge_letter(&mut art, letter, 29.0 * s, 48.0 * s, 12.5 * s);
    art.apply_to_shape(sh, top_rgb, tile_rgb, true);
}

fn draw_badge_letter(art: &mut IsoArtwork, letter: &str, cx: f64, cy: f64, half: f64) {
    let l = cx - half * 0.68;
    let r = cx + half * 0.68;
    let t = cy - half * 0.78;
    let b = cy + half * 0.78;
    match letter {
        "O" => {
            art.add_ellipse(cx, cy, half * 0.68, half * 0.80, None, Some("255,255,255"), 2.6);
        }
        "P" => {
            let mut g = GeometrySection::new(art.next_idx());
            g.no_fill = "TRUE".into();
            let bow = art.bow_formula(0.414 * half * 0.42);
            g.rows = vec![
                GeometryRow::move_to(1, art.fx(l), art.fy(b)),
                GeometryRow::line_to(2, art.fx(l), art.fy(t)),
                GeometryRow::line_to(3, art.fx(cx + half * 0.18), art.fy(t)),
                GeometryRow::arc_to(4, art.fx(cx + half * 0.18), art.fy(cy + half * 0.04), &bow),
                GeometryRow::line_to(5, art.fx(l), art.fy(cy + half * 0.04)),
            ];
            art.geoms.push((g, None, Some("255,255,255"), 2.6));
        }
        "S" => {
            let mut g = GeometrySection::new(art.next_idx());
            g.no_fill = "TRUE".into();
            let bow_l = art.bow_formula(-0.36 * half);
            let bow_r = art.bow_formula(0.36 * half);
            g.rows = vec![
                GeometryRow::move_to(1, art.fx(r - half * 0.1), art.fy(t + half * 0.15)),
                GeometryRow::arc_to(2, art.fx(cx), art.fy(cy), &bow_l),
                GeometryRow::arc_to(3, art.fx(l + half * 0.1), art.fy(b - half * 0.15), &bow_r),
            ];
            art.geoms.push((g, None, Some("255,255,255"), 2.5));
        }
        _ => {
            let segs: Vec<((f64, f64), (f64, f64))> = match letter {
                "W" => vec![
                    ((l, t), (cx - half * 0.34, b)),
                    ((cx - half * 0.34, b), (cx, cy - half * 0.12)),
                    ((cx, cy - half * 0.12), (cx + half * 0.34, b)),
                    ((cx + half * 0.34, b), (r, t)),
                ],
                "X" => vec![((l, t), (r, b)), ((r, t), (l, b))],
                "N" => vec![((l, b), (l, t)), ((l, t), (r, b)), ((r, b), (r, t))],
                "V" => vec![((l, t), (cx, b)), ((cx, b), (r, t))],
                "A" => vec![
                    ((l, b), (cx, t)),
                    ((cx, t), (r, b)),
                    ((l + half * 0.22, cy + half * 0.22), (r - half * 0.22, cy + half * 0.22)),
                ],
                "T" => vec![((l, t), (r, t)), ((cx, t), (cx, b))],
                "F" => vec![
                    ((l, b), (l, t)),
                    ((l, t), (r, t)),
                    ((l, cy), (r - half * 0.2, cy)),
                ],
                "L" => vec![((l, t), (l, b)), ((l, b), (r, b))],
                _ => vec![((l, cy), (r, cy))],
            };
            art.add_lines(&segs, "255,255,255", 2.6);
        }
    }
}

fn build_microsoft_365_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "office_word" => build_office_app_tile(
            sh, "W", "24,90,189", "65,165,238", "43,124,211", "20,74,158", "doc",
        ),
        "office_excel" => build_office_app_tile(
            sh, "X", "16,124,65", "51,196,129", "33,163,102", "16,124,65", "excel_grid",
        ),
        "office_powerpoint" => build_office_app_tile(
            sh, "P", "196,62,28", "255,143,107", "237,108,71", "210,71,38", "ppt_circle",
        ),
        "office_onenote" => build_office_app_tile(
            sh, "N", "119,25,170", "202,100,234", "174,75,213", "147,50,191", "onenote_tabs",
        ),
        "office_visio" => build_office_app_tile(
            sh, "V", "25,85,166", "75,160,255", "40,116,208", "25,85,166", "visio_nodes",
        ),
        "office_project" => build_office_app_tile(
            sh, "P", "16,124,65", "51,196,129", "33,163,102", "24,92,55", "project_gantt",
        ),
        "office_access" => build_office_app_tile(
            sh, "A", "164,55,58", "224,128,131", "201,79,82", "140,42,45", "db_cyl",
        ),
        "office_publisher" => build_office_app_tile(
            sh, "P", "7,117,104", "46,196,182", "19,153,138", "7,117,104", "doc",
        ),
        "m365_outlook" => build_office_app_tile(
            sh, "O", "0,120,212", "80,217,255", "40,168,234", "0,90,158", "outlook_env",
        ),
        "m365_forms" => build_office_app_tile(
            sh, "F", "13,148,136", "45,212,191", "20,184,166", "15,118,110", "doc",
        ),
        "m365_loop" => build_office_app_tile(
            sh, "L", "124,58,237", "167,139,250", "139,92,246", "109,40,217", "doc",
        ),
        "m365_teams" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(78.0 * s, 29.0 * s, 9.0 * s, 9.0 * s, Some("123,131,235"), Some("75,83,188"), 1.2);
            art.add_rect(64.0 * s, 41.0 * s, 28.0 * s, 32.0 * s, 10.0 * s, Some("123,131,235"), Some("75,83,188"), 1.2);
            art.add_ellipse(56.0 * s, 23.0 * s, 12.0 * s, 12.0 * s, Some("80,89,201"), Some("59,66,159"), 1.4);
            art.add_rect(36.0 * s, 38.0 * s, 42.0 * s, 45.0 * s, 12.0 * s, Some("80,89,201"), Some("59,66,159"), 1.4);
            art.add_rect(9.0 * s, 30.0 * s, 44.0 * s, 44.0 * s, 9.0 * s, Some("148,163,184"), None, 0.8);
            art.add_rect(6.0 * s, 27.0 * s, 44.0 * s, 44.0 * s, 9.0 * s, Some("75,83,188"), Some("255,255,255"), 1.6);
            draw_badge_letter(&mut art, "T", 28.0 * s, 49.0 * s, 12.0 * s);
            art.apply_to_shape(sh, "80,89,201", "59,66,159", true);
        }
        "m365_sharepoint" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(58.0 * s, 29.0 * s, 22.0 * s, 22.0 * s, Some("3,108,112"), Some("255,255,255"), 1.4);
            art.add_ellipse(72.0 * s, 56.0 * s, 19.0 * s, 19.0 * s, Some("26,155,161"), Some("255,255,255"), 1.4);
            art.add_ellipse(54.0 * s, 73.0 * s, 15.0 * s, 15.0 * s, Some("55,198,208"), Some("255,255,255"), 1.4);
            art.add_rect(8.0 * s, 28.0 * s, 44.0 * s, 44.0 * s, 9.0 * s, Some("3,131,135"), Some("255,255,255"), 1.6);
            draw_badge_letter(&mut art, "S", 30.0 * s, 50.0 * s, 12.0 * s);
            art.apply_to_shape(sh, "26,155,161", "3,108,112", true);
        }
        "m365_onedrive" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_ellipse(w * 0.44, h * 0.42, h * 0.25, h * 0.25, Some("3,100,184"), None, 0.8);
            art.add_ellipse(w * 0.66, h * 0.47, h * 0.21, h * 0.21, Some("0,120,212"), None, 0.8);
            art.add_ellipse(w * 0.25, h * 0.60, h * 0.19, h * 0.19, Some("40,168,234"), None, 0.8);
            art.add_ellipse(w * 0.76, h * 0.62, h * 0.17, h * 0.17, Some("80,217,255"), None, 0.8);
            art.add_rect(w * 0.23, h * 0.54, w * 0.54, h * 0.25, h * 0.12, Some("40,168,234"), None, 0.8);
            art.apply_to_shape(sh, "40,168,234", "0,120,212", true);
        }
        "m365_exchange" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (50.0 * s, 8.0 * s),
                    (92.0 * s, 50.0 * s),
                    (50.0 * s, 92.0 * s),
                    (8.0 * s, 50.0 * s),
                ],
                Some("0,120,212"),
                Some("0,90,158"),
                1.6,
            );
            art.add_rect(28.0 * s, 34.0 * s, 44.0 * s, 32.0 * s, 4.0 * s, Some("255,255,255"), Some("0,90,158"), 1.4);
            art.add_lines(
                &[
                    ((29.0 * s, 36.0 * s), (50.0 * s, 52.0 * s)),
                    ((71.0 * s, 36.0 * s), (50.0 * s, 52.0 * s)),
                ],
                "0,120,212",
                1.8,
            );
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "m365_tenant" => {
            // Isotropic Container Frame (`240 × 150` — corner Microsoft 4-pane logo is 100% square!)
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(5.0, 5.0, w - 10.0, h - 10.0, 10.0, Some("248,250,252"), Some("0,120,212"), 1.8);
            art.add_rect(16.0, 16.0, 14.0, 14.0, 2.0, Some("242,80,34"), None, 0.8);
            art.add_rect(33.0, 16.0, 14.0, 14.0, 2.0, Some("127,186,0"), None, 0.8);
            art.add_rect(16.0, 33.0, 14.0, 14.0, 2.0, Some("0,164,239"), None, 0.8);
            art.add_rect(33.0, 33.0, 14.0, 14.0, 2.0, Some("255,185,0"), None, 0.8);
            art.add_lines(&[((56.0, 24.0), (118.0, 24.0)), ((56.0, 36.0), (96.0, 36.0))], "0,120,212", 2.0);
            art.apply_to_shape(sh, "248,250,252", "0,120,212", false);
            sh.sheet
                .set_formula("TxtPinY", "=Height - 0.22 in", Some(Unit::Inch));
        }
        "m365_copilot" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (28.0 * s, 16.0 * s),
                    (62.0 * s, 16.0 * s),
                    (46.0 * s, 84.0 * s),
                    (12.0 * s, 84.0 * s),
                ],
                Some("14,165,233"),
                Some("255,255,255"),
                1.6,
            );
            art.add_poly(
                &[
                    (54.0 * s, 16.0 * s),
                    (88.0 * s, 16.0 * s),
                    (72.0 * s, 84.0 * s),
                    (38.0 * s, 84.0 * s),
                ],
                Some("168,85,247"),
                Some("255,255,255"),
                1.6,
            );
            art.add_poly(
                &[
                    (54.0 * s, 16.0 * s),
                    (88.0 * s, 16.0 * s),
                    (80.0 * s, 48.0 * s),
                    (46.0 * s, 48.0 * s),
                ],
                Some("249,115,22"),
                None,
                0.8,
            );
            art.apply_to_shape(sh, "168,85,247", "126,34,206", true);
        }
        "m365_defender" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (50.0 * s, 8.0 * s),
                    (88.0 * s, 22.0 * s),
                    (84.0 * s, 60.0 * s),
                    (50.0 * s, 92.0 * s),
                    (16.0 * s, 60.0 * s),
                    (12.0 * s, 22.0 * s),
                ],
                Some("29,78,216"),
                Some("30,58,138"),
                1.8,
            );
            art.add_poly(
                &[
                    (50.0 * s, 8.0 * s),
                    (88.0 * s, 22.0 * s),
                    (84.0 * s, 50.0 * s),
                    (50.0 * s, 50.0 * s),
                ],
                Some("56,189,248"),
                None,
                0.8,
            );
            art.add_lines(
                &[
                    ((34.0 * s, 52.0 * s), (46.0 * s, 65.0 * s)),
                    ((46.0 * s, 65.0 * s), (68.0 * s, 38.0 * s)),
                ],
                "255,255,255",
                2.6,
            );
            art.apply_to_shape(sh, "29,78,216", "30,58,138", true);
        }
        "m365_intune" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(10.0 * s, 16.0 * s, 64.0 * s, 46.0 * s, 6.0 * s, Some("0,120,212"), Some("0,69,120"), 1.6);
            art.add_rect(16.0 * s, 22.0 * s, 52.0 * s, 34.0 * s, 3.0 * s, Some("224,242,254"), None, 0.8);
            art.add_rect(60.0 * s, 38.0 * s, 30.0 * s, 48.0 * s, 6.0 * s, Some("40,168,234"), Some("0,69,120"), 1.6);
            art.add_lines(
                &[
                    ((68.0 * s, 62.0 * s), (74.0 * s, 68.0 * s)),
                    ((74.0 * s, 68.0 * s), (84.0 * s, 55.0 * s)),
                ],
                "255,255,255",
                2.4,
            );
            art.apply_to_shape(sh, "0,120,212", "0,69,120", true);
        }
        "m365_purview" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(50.0 * s, 50.0 * s, 39.0 * s, 24.0 * s, Some("243,232,255"), Some("126,34,206"), 2.2);
            art.add_ellipse(50.0 * s, 50.0 * s, 16.0 * s, 16.0 * s, Some("147,51,234"), Some("255,255,255"), 1.6);
            art.add_ellipse(55.0 * s, 45.0 * s, 5.5 * s, 5.5 * s, Some("255,255,255"), None, 0.8);
            art.apply_to_shape(sh, "147,51,234", "126,34,206", true);
        }
        "m365_planner" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(12.0 * s, 14.0 * s, 76.0 * s, 72.0 * s, 9.0 * s, Some("236,253,245"), Some("16,124,65"), 1.8);
            art.add_rect(20.0 * s, 24.0 * s, 16.0 * s, 52.0 * s, 4.0 * s, Some("16,124,65"), None, 0.8);
            art.add_rect(42.0 * s, 24.0 * s, 16.0 * s, 38.0 * s, 4.0 * s, Some("33,163,102"), None, 0.8);
            art.add_rect(64.0 * s, 24.0 * s, 16.0 * s, 26.0 * s, 4.0 * s, Some("51,196,129"), None, 0.8);
            art.apply_to_shape(sh, "33,163,102", "16,124,65", true);
        }
        "m365_todo" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(10.0 * s, 10.0 * s, 80.0 * s, 80.0 * s, 16.0 * s, Some("239,246,255"), Some("37,99,235"), 1.6);
            // Authentic 2-tone Microsoft To Do Ribbon Checkmark
            art.add_poly(
                &[
                    (20.0 * s, 52.0 * s),
                    (32.0 * s, 40.0 * s),
                    (52.0 * s, 60.0 * s),
                    (40.0 * s, 72.0 * s),
                ],
                Some("29,78,216"),
                None,
                0.8,
            );
            art.add_poly(
                &[
                    (40.0 * s, 72.0 * s),
                    (29.0 * s, 61.0 * s),
                    (70.0 * s, 20.0 * s),
                    (82.0 * s, 32.0 * s),
                ],
                Some("56,189,248"),
                Some("2,132,199"),
                1.2,
            );
            art.apply_to_shape(sh, "37,99,235", "29,78,216", true);
        }
        "m365_stream" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(10.0 * s, 18.0 * s, 80.0 * s, 64.0 * s, 14.0 * s, Some("225,29,72"), Some("159,18,57"), 1.6);
            art.add_rect(10.0 * s, 18.0 * s, 80.0 * s, 24.0 * s, 12.0 * s, Some("244,63,94"), None, 0.8);
            art.add_poly(
                &[(40.0 * s, 32.0 * s), (68.0 * s, 50.0 * s), (40.0 * s, 68.0 * s)],
                Some("255,255,255"),
                None,
                0.8,
            );
            art.apply_to_shape(sh, "225,29,72", "159,18,57", true);
        }
        "m365_whiteboard" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(10.0 * s, 16.0 * s, 80.0 * s, 68.0 * s, 9.0 * s, Some("2,132,199"), Some("3,105,161"), 1.6);
            art.add_rect(18.0 * s, 24.0 * s, 64.0 * s, 48.0 * s, 4.0 * s, Some("255,255,255"), None, 0.8);
            art.add_rect(24.0 * s, 30.0 * s, 20.0 * s, 18.0 * s, 3.0 * s, Some("254,240,138"), Some("202,138,4"), 1.2);
            art.add_poly(
                &[
                    (50.0 * s, 62.0 * s),
                    (70.0 * s, 34.0 * s),
                    (76.0 * s, 39.0 * s),
                    (56.0 * s, 66.0 * s),
                ],
                Some("37,99,235"),
                None,
                0.8,
            );
            art.apply_to_shape(sh, "2,132,199", "3,105,161", true);
        }
        _ => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(38.0 * s, 38.0 * s, 21.0 * s, 21.0 * s, Some("0,120,212"), Some("255,255,255"), 1.4);
            art.add_ellipse(62.0 * s, 38.0 * s, 21.0 * s, 21.0 * s, Some("56,189,248"), Some("255,255,255"), 1.4);
            art.add_ellipse(50.0 * s, 62.0 * s, 21.0 * s, 21.0 * s, Some("99,102,241"), Some("255,255,255"), 1.4);
            art.apply_to_shape(sh, "0,120,212", "59,130,246", true);
        }
    }
}

// ============================================================================
// 2. Standard Servers & Microsoft Servers / Active Directory (`110 × 130` Isotropic Tower)
// ============================================================================

fn draw_3d_server_tower_base(art: &mut IsoArtwork, accent_rgb: &'static str) {
    // Designed natively in `110 × 130` isotropic units (`Width = 1.10 in, Height = 1.30 in`)
    let sx = art.vb_w / 110.0;
    let sy = art.vb_h / 130.0;
    let s = sx.min(sy);
    art.add_poly(
        &[
            (14.0 * sx, 20.0 * sy),
            (30.0 * sx, 8.0 * sy),
            (84.0 * sx, 8.0 * sy),
            (68.0 * sx, 20.0 * sy),
        ],
        Some("148,163,184"),
        Some("51,65,85"),
        1.4,
    );
    art.add_poly(
        &[
            (68.0 * sx, 20.0 * sy),
            (84.0 * sx, 8.0 * sy),
            (84.0 * sx, 104.0 * sy),
            (68.0 * sx, 118.0 * sy),
        ],
        Some("71,85,105"),
        Some("30,41,59"),
        1.4,
    );
    art.add_rect(14.0 * sx, 20.0 * sy, 54.0 * sx, 98.0 * sy, 3.0 * s, Some("100,116,139"), Some("30,41,59"), 1.6);
    art.add_rect(19.0 * sx, 26.0 * sy, 44.0 * sx, 6.5 * sy, 1.5 * s, Some(accent_rgb), None, 0.8);
    art.add_rect(21.0 * sx, 40.0 * sy, 40.0 * sx, 9.0 * sy, 1.5 * s, Some("30,41,59"), None, 0.8);
    art.add_rect(21.0 * sx, 54.0 * sy, 40.0 * sx, 9.0 * sy, 1.5 * s, Some("30,41,59"), None, 0.8);
    art.add_lines(
        &[
            ((22.0 * sx, 74.0 * sy), (54.0 * sx, 74.0 * sy)),
            ((22.0 * sx, 82.0 * sy), (54.0 * sx, 82.0 * sy)),
            ((22.0 * sx, 90.0 * sy), (54.0 * sx, 90.0 * sy)),
        ],
        "51,65,85",
        1.6,
    );
    art.add_ellipse(28.0 * sx, 106.0 * sy, 4.0 * s, 4.0 * s, Some("34,197,94"), Some("20,83,45"), 1.0);
}

fn draw_server_role_badge(art: &mut IsoArtwork, badge: &str) {
    let sx = art.vb_w / 110.0;
    let sy = art.vb_h / 130.0;
    let s = sx.min(sy);
    let cx = 80.0 * sx;
    let cy = 94.0 * sy;
    match badge {
        "globe" => {
            art.add_ellipse(cx, cy, 22.0 * s, 22.0 * s, Some("2,132,199"), Some("255,255,255"), 1.6);
            art.add_ellipse(cx, cy, 10.0 * s, 21.0 * s, None, Some("224,242,254"), 1.3);
            art.add_lines(
                &[
                    ((cx - 21.0 * s, cy), (cx + 21.0 * s, cy)),
                    ((cx - 17.0 * s, cy - 10.0 * s), (cx + 17.0 * s, cy - 10.0 * s)),
                    ((cx - 17.0 * s, cy + 10.0 * s), (cx + 17.0 * s, cy + 10.0 * s)),
                ],
                "224,242,254",
                1.2,
            );
        }
        "db" | "sql" => {
            let rgb = if badge == "sql" { "220,38,38" } else { "217,119,6" };
            art.add_rect(cx - 19.0 * s, cy - 18.0 * s, 38.0 * s, 36.0 * s, 4.0 * s, Some(rgb), Some("255,255,255"), 1.5);
            art.add_ellipse(cx, cy - 18.0 * s, 19.0 * s, 7.0 * s, Some("253,224,71"), Some("180,83,9"), 1.2);
            art.add_ellipse(cx, cy + 18.0 * s, 19.0 * s, 7.0 * s, Some(rgb), Some("255,255,255"), 1.2);
        }
        "folder" => {
            art.add_poly(
                &[
                    (cx - 22.0 * s, cy - 14.0 * s),
                    (cx - 5.0 * s, cy - 14.0 * s),
                    (cx, cy - 8.0 * s),
                    (cx + 22.0 * s, cy - 8.0 * s),
                    (cx + 22.0 * s, cy + 18.0 * s),
                    (cx - 22.0 * s, cy + 18.0 * s),
                ],
                Some("250,204,21"),
                Some("180,83,9"),
                1.4,
            );
        }
        "mail" => {
            art.add_rect(cx - 22.0 * s, cy - 15.0 * s, 44.0 * s, 30.0 * s, 4.0 * s, Some("37,99,235"), Some("255,255,255"), 1.6);
            art.add_lines(
                &[
                    ((cx - 20.0 * s, cy - 13.0 * s), (cx, cy + 2.0 * s)),
                    ((cx + 20.0 * s, cy - 13.0 * s), (cx, cy + 2.0 * s)),
                ],
                "255,255,255",
                1.8,
            );
        }
        "printer" => {
            art.add_rect(cx - 14.0 * s, cy - 20.0 * s, 28.0 * s, 12.0 * s, 2.0 * s, Some("255,255,255"), Some("71,85,105"), 1.3);
            art.add_rect(cx - 21.0 * s, cy - 10.0 * s, 42.0 * s, 20.0 * s, 4.0 * s, Some("51,65,85"), Some("255,255,255"), 1.5);
            art.add_rect(cx - 13.0 * s, cy + 5.0 * s, 26.0 * s, 14.0 * s, 2.0 * s, Some("226,232,240"), Some("71,85,105"), 1.2);
        }
        "ad_pyramid" => {
            art.add_poly(
                &[
                    (cx, cy - 22.0 * s),
                    (cx + 23.0 * s, cy + 20.0 * s),
                    (cx - 23.0 * s, cy + 20.0 * s),
                ],
                Some("0,120,212"),
                Some("255,255,255"),
                1.6,
            );
            art.add_lines(
                &[
                    ((cx, cy - 20.0 * s), (cx, cy + 20.0 * s)),
                    ((cx - 13.0 * s, cy + 5.0 * s), (cx + 13.0 * s, cy + 5.0 * s)),
                ],
                "253,224,71",
                1.7,
            );
        }
        "shield_brick" => {
            art.add_poly(
                &[
                    (cx, cy - 22.0 * s),
                    (cx + 21.0 * s, cy - 14.0 * s),
                    (cx + 17.0 * s, cy + 8.0 * s),
                    (cx, cy + 22.0 * s),
                    (cx - 17.0 * s, cy + 8.0 * s),
                    (cx - 21.0 * s, cy - 14.0 * s),
                ],
                Some("220,38,38"),
                Some("255,255,255"),
                1.6,
            );
            art.add_lines(
                &[
                    ((cx - 17.0 * s, cy - 3.0 * s), (cx + 17.0 * s, cy - 3.0 * s)),
                    ((cx, cy - 18.0 * s), (cx, cy + 18.0 * s)),
                ],
                "254,226,226",
                1.4,
            );
        }
        "pki_seal" => {
            art.add_poly(
                &[
                    (cx - 9.0 * s, cy + 8.0 * s),
                    (cx - 14.0 * s, cy + 25.0 * s),
                    (cx, cy + 18.0 * s),
                    (cx + 14.0 * s, cy + 25.0 * s),
                    (cx + 9.0 * s, cy + 8.0 * s),
                ],
                Some("220,38,38"),
                None,
                0.8,
            );
            art.add_ellipse(cx, cy - 2.0 * s, 17.0 * s, 17.0 * s, Some("245,158,11"), Some("255,255,255"), 1.6);
        }
        "vms" => {
            art.add_rect(cx - 22.0 * s, cy - 12.0 * s, 22.0 * s, 22.0 * s, 3.0 * s, Some("14,165,233"), Some("255,255,255"), 1.5);
            art.add_rect(cx - 6.0 * s, cy - 2.0 * s, 22.0 * s, 22.0 * s, 3.0 * s, Some("99,102,241"), Some("255,255,255"), 1.5);
            art.add_rect(cx + 2.0 * s, cy - 20.0 * s, 20.0 * s, 20.0 * s, 3.0 * s, Some("16,185,129"), Some("255,255,255"), 1.5);
        }
        "clock" => {
            art.add_ellipse(cx, cy, 19.0 * s, 19.0 * s, Some("255,255,255"), Some("2,132,199"), 1.8);
            art.add_lines(
                &[((cx, cy), (cx, cy - 11.0 * s)), ((cx, cy), (cx + 10.0 * s, cy + 4.0 * s))],
                "15,23,42",
                1.8,
            );
        }
        "pulse" => {
            art.add_rect(cx - 23.0 * s, cy - 16.0 * s, 46.0 * s, 32.0 * s, 5.0 * s, Some("15,23,42"), Some("56,189,248"), 1.5);
            art.add_lines(
                &[
                    ((cx - 18.0 * s, cy + 2.0 * s), (cx - 8.0 * s, cy + 2.0 * s)),
                    ((cx - 8.0 * s, cy + 2.0 * s), (cx - 2.0 * s, cy - 10.0 * s)),
                    ((cx - 2.0 * s, cy - 10.0 * s), (cx + 5.0 * s, cy + 11.0 * s)),
                    ((cx + 5.0 * s, cy + 11.0 * s), (cx + 11.0 * s, cy)),
                    ((cx + 11.0 * s, cy), (cx + 19.0 * s, cy)),
                ],
                "34,197,94",
                1.8,
            );
        }
        "backup" => {
            art.add_ellipse(cx, cy, 19.0 * s, 19.0 * s, Some("16,185,129"), Some("255,255,255"), 1.6);
            art.add_lines(
                &[
                    ((cx, cy - 10.0 * s), (cx, cy + 10.0 * s)),
                    ((cx - 8.0 * s, cy - 2.0 * s), (cx, cy - 10.0 * s)),
                    ((cx + 8.0 * s, cy - 2.0 * s), (cx, cy - 10.0 * s)),
                ],
                "255,255,255",
                2.0,
            );
        }
        "arrows" => {
            art.add_ellipse(cx, cy, 19.0 * s, 19.0 * s, Some("2,132,199"), Some("255,255,255"), 1.6);
            art.add_lines(
                &[
                    ((cx - 11.0 * s, cy - 5.0 * s), (cx + 11.0 * s, cy - 5.0 * s)),
                    ((cx - 11.0 * s, cy + 5.0 * s), (cx + 11.0 * s, cy + 5.0 * s)),
                ],
                "255,255,255",
                2.0,
            );
        }
        "gear" => {
            art.add_ellipse(cx, cy, 19.0 * s, 19.0 * s, Some("79,70,229"), Some("255,255,255"), 1.6);
            art.add_ellipse(cx, cy, 7.5 * s, 7.5 * s, Some("255,255,255"), None, 0.8);
        }
        _ => {}
    }
}

fn build_standard_server_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "srv_rack" | "srv_rack_4u" => {
            // Designed natively for exact `1.85 × 0.50` (1U/2U) or `1.85 × 0.85` (4U) box — zero padding squash!
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            let ear_w = 10.0;
            art.add_rect(3.0, 4.0, ear_w, h - 8.0, 2.0, Some("71,85,105"), Some("30,41,59"), 1.3);
            art.add_rect(w - 3.0 - ear_w, 4.0, ear_w, h - 8.0, 2.0, Some("71,85,105"), Some("30,41,59"), 1.3);
            art.add_rect(3.0 + ear_w, 4.0, w - 6.0 - 2.0 * ear_w, h - 8.0, 3.0, Some("148,163,184"), Some("30,41,59"), 1.6);
            let bay_w = (w - 58.0) / 4.0;
            for col in 0..4 {
                let bx = 20.0 + (col as f64) * (bay_w + 4.0);
                art.add_rect(bx, 10.0, bay_w, h - 20.0, 2.0, Some("30,41,59"), Some("51,65,85"), 1.1);
            }
            let r_led = (h * 0.08).clamp(2.5, 4.5);
            art.add_ellipse(w - 22.0, h * 0.36, r_led, r_led, Some("34,197,94"), None, 0.8);
            art.add_ellipse(w - 22.0, h * 0.66, r_led, r_led, Some("56,189,248"), None, 0.8);
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
        "srv_blade" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(6.0, 6.0, w - 12.0, h - 12.0, 5.0, Some("51,65,85"), Some("15,23,42"), 1.8);
            let slot_w = (w - 30.0) / 6.0;
            for b in 0..6 {
                let bx = 13.0 + (b as f64) * slot_w;
                art.add_rect(bx, 13.0, slot_w - 3.5, h - 38.0, 2.0, Some("148,163,184"), Some("30,41,59"), 1.1);
                art.add_ellipse(bx + (slot_w - 3.5) * 0.5, 22.0, 2.6, 2.6, Some("34,197,94"), None, 0.8);
            }
            art.add_rect(13.0, h - 20.0, w - 26.0, 9.0, 2.0, Some("2,132,199"), None, 0.8);
            art.apply_to_shape(sh, "100,116,139", "15,23,42", true);
        }
        "srv_blade_mod" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(8.0, 6.0, w - 16.0, h - 12.0, 4.0, Some("148,163,184"), Some("30,41,59"), 1.6);
            art.add_rect(14.0, 14.0, w - 28.0, 18.0, 2.0, Some("2,132,199"), None, 0.8);
            art.add_rect(14.0, 38.0, w - 28.0, 28.0, 2.0, Some("30,41,59"), None, 0.8);
            art.add_rect(14.0, 72.0, w - 28.0, 28.0, 2.0, Some("30,41,59"), None, 0.8);
            art.add_ellipse(w * 0.5, h - 18.0, 4.0, 4.0, Some("34,197,94"), None, 0.8);
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
        "srv_mainframe" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(8.0, 8.0, w - 16.0, h - 16.0, 5.0, Some("30,41,59"), Some("15,23,42"), 1.8);
            let col_w = (w - 32.0) / 3.0;
            for c in 0..3 {
                let cx = 14.0 + (c as f64) * (col_w + 2.0);
                art.add_rect(cx, 14.0, col_w - 2.0, h - 28.0, 2.5, Some("51,65,85"), Some("15,23,42"), 1.2);
            }
            art.add_rect(w * 0.38, 24.0, w * 0.24, 16.0, 2.0, Some("56,189,248"), None, 0.8);
            art.apply_to_shape(sh, "30,41,59", "15,23,42", true);
        }
        "srv_edge" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(8.0, 18.0, w - 16.0, h - 32.0, 6.0, Some("71,85,105"), Some("15,23,42"), 1.6);
            art.add_rect(15.0, 26.0, w - 30.0, 10.0, 2.0, Some("14,165,233"), None, 0.8);
            art.add_ellipse(24.0, h - 26.0, 4.0, 4.0, Some("34,197,94"), None, 0.8);
            art.apply_to_shape(sh, "71,85,105", "15,23,42", true);
        }
        _ => {
            let (accent_rgb, badge) = match role {
                "srv_web" => ("2,132,199", "globe"),
                "srv_db" => ("220,38,38", "db"),
                "srv_app" => ("79,70,229", "gear"),
                "srv_file" => ("234,179,8", "folder"),
                "srv_print" => ("71,85,105", "printer"),
                "srv_mail" => ("37,99,235", "mail"),
                "srv_ldap" => ("0,120,212", "ad_pyramid"),
                "srv_dns" | "srv_dhcp" | "srv_proxy" | "srv_ftp" => ("2,132,199", "arrows"),
                "srv_ntp" => ("2,132,199", "clock"),
                "srv_bastion" => ("220,38,38", "shield_brick"),
                "srv_siem" | "srv_nms" => ("16,185,129", "pulse"),
                "srv_pki" => ("217,119,6", "pki_seal"),
                "srv_hypervisor" | "srv_rds" => ("99,102,241", "vms"),
                "srv_backup" => ("16,185,129", "backup"),
                _ => ("0,120,212", ""),
            };
            let mut art = IsoArtwork::for_shape(sh);
            draw_3d_server_tower_base(&mut art, accent_rgb);
            if !badge.is_empty() {
                draw_server_role_badge(&mut art, badge);
            }
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
    }
}

fn build_microsoft_server_ad_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "ms_entra_id" | "ms_entra_connect" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[(50.0 * s, 8.0 * s), (12.0 * s, 60.0 * s), (50.0 * s, 92.0 * s)],
                Some("0,120,212"),
                Some("255,255,255"),
                1.4,
            );
            art.add_poly(
                &[(50.0 * s, 8.0 * s), (88.0 * s, 60.0 * s), (50.0 * s, 92.0 * s)],
                Some("80,230,255"),
                Some("0,120,212"),
                1.4,
            );
            art.add_poly(
                &[
                    (50.0 * s, 34.0 * s),
                    (74.0 * s, 60.0 * s),
                    (50.0 * s, 80.0 * s),
                    (26.0 * s, 60.0 * s),
                ],
                Some("56,189,248"),
                Some("255,255,255"),
                1.2,
            );
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "ms_ad_ou" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (10.0 * s, 20.0 * s),
                    (40.0 * s, 20.0 * s),
                    (48.0 * s, 30.0 * s),
                    (90.0 * s, 30.0 * s),
                    (90.0 * s, 84.0 * s),
                    (10.0 * s, 84.0 * s),
                ],
                Some("250,204,21"),
                Some("180,83,9"),
                1.6,
            );
            art.add_poly(
                &[(50.0 * s, 40.0 * s), (72.0 * s, 76.0 * s), (28.0 * s, 76.0 * s)],
                Some("0,120,212"),
                Some("255,255,255"),
                1.5,
            );
            art.add_lines(
                &[((50.0 * s, 42.0 * s), (50.0 * s, 75.0 * s))],
                "253,224,71",
                1.5,
            );
            art.apply_to_shape(sh, "250,204,21", "180,83,9", true);
        }
        "ms_gpo" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(16.0 * s, 10.0 * s, 64.0 * s, 80.0 * s, 6.0 * s, Some("255,255,255"), Some("0,120,212"), 1.8);
            art.add_lines(
                &[
                    ((26.0 * s, 26.0 * s), (70.0 * s, 26.0 * s)),
                    ((26.0 * s, 38.0 * s), (70.0 * s, 38.0 * s)),
                    ((26.0 * s, 50.0 * s), (54.0 * s, 50.0 * s)),
                ],
                "100,116,139",
                1.8,
            );
            art.add_poly(
                &[
                    (68.0 * s, 54.0 * s),
                    (88.0 * s, 70.0 * s),
                    (68.0 * s, 86.0 * s),
                    (48.0 * s, 70.0 * s),
                ],
                Some("245,158,11"),
                Some("180,83,9"),
                1.5,
            );
            art.apply_to_shape(sh, "255,255,255", "0,120,212", true);
        }
        "ms_ad_trust" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_poly(
                &[(w * 0.25, h * 0.18), (w * 0.44, h * 0.80), (w * 0.06, h * 0.80)],
                Some("0,120,212"),
                Some("0,69,120"),
                1.6,
            );
            art.add_poly(
                &[(w * 0.75, h * 0.18), (w * 0.94, h * 0.80), (w * 0.56, h * 0.80)],
                Some("16,124,65"),
                Some("6,78,59"),
                1.6,
            );
            art.add_lines(&[((w * 0.38, h * 0.52), (w * 0.62, h * 0.52))], "217,119,6", 2.6);
            art.apply_to_shape(sh, "0,120,212", "0,69,120", true);
        }
        "ms_win11_client" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(10.0 * s, 14.0 * s, 80.0 * s, 54.0 * s, 6.0 * s, Some("0,120,212"), Some("30,41,59"), 1.8);
            art.add_rect(37.0 * s, 29.0 * s, 11.0 * s, 11.0 * s, 1.5 * s, Some("255,255,255"), None, 0.8);
            art.add_rect(52.0 * s, 29.0 * s, 11.0 * s, 11.0 * s, 1.5 * s, Some("255,255,255"), None, 0.8);
            art.add_rect(37.0 * s, 44.0 * s, 11.0 * s, 11.0 * s, 1.5 * s, Some("255,255,255"), None, 0.8);
            art.add_rect(52.0 * s, 44.0 * s, 11.0 * s, 11.0 * s, 1.5 * s, Some("255,255,255"), None, 0.8);
            art.add_poly(
                &[
                    (36.0 * s, 68.0 * s),
                    (64.0 * s, 68.0 * s),
                    (74.0 * s, 84.0 * s),
                    (26.0 * s, 84.0 * s),
                ],
                Some("148,163,184"),
                Some("51,65,85"),
                1.5,
            );
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
            let mut art = IsoArtwork::for_shape(sh);
            draw_3d_server_tower_base(&mut art, "0,120,212");
            if !badge.is_empty() {
                draw_server_role_badge(&mut art, badge);
            }
            art.apply_to_shape(sh, "148,163,184", "30,41,59", true);
        }
    }
}

// ============================================================================
// 3. Visio / Cisco-Standard 3D Network & Rack Equipment (Exact Physical Aspect Ratios)
// ============================================================================

fn build_network_equipment_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "net_router" | "net_edge_router" | "net_isr_router" => {
            // 3D Turquoise Cylindrical Router Puck (`120 × 85` isotropic units)
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            let rx = w * 0.42;
            let ry = h * 0.24;
            let cx = w * 0.5;
            let top_y = h * 0.36;
            let bot_y = h * 0.68;
            art.add_rect(cx - rx, top_y, rx * 2.0, bot_y - top_y, 0.0, Some("3,105,161"), Some("12,74,110"), 1.5);
            art.add_ellipse(cx, bot_y, rx, ry, Some("3,105,161"), Some("12,74,110"), 1.5);
            art.add_ellipse(cx, top_y, rx, ry, Some("56,189,248"), Some("12,74,110"), 1.6);
            art.add_lines(
                &[
                    ((cx - rx * 0.65, top_y), (cx - rx * 0.16, top_y)),
                    ((cx - rx * 0.30, top_y - ry * 0.26), (cx - rx * 0.14, top_y)),
                    ((cx - rx * 0.30, top_y + ry * 0.26), (cx - rx * 0.14, top_y)),
                    ((cx + rx * 0.65, top_y), (cx + rx * 0.16, top_y)),
                    ((cx + rx * 0.30, top_y - ry * 0.26), (cx + rx * 0.14, top_y)),
                    ((cx + rx * 0.30, top_y + ry * 0.26), (cx + rx * 0.14, top_y)),
                    ((cx, top_y - ry * 0.72), (cx, top_y - ry * 0.22)),
                    ((cx - rx * 0.12, top_y - ry * 0.44), (cx, top_y - ry * 0.18)),
                    ((cx + rx * 0.12, top_y - ry * 0.44), (cx, top_y - ry * 0.18)),
                    ((cx, top_y + ry * 0.22), (cx, top_y + ry * 0.72)),
                    ((cx - rx * 0.12, top_y + ry * 0.46), (cx, top_y + ry * 0.74)),
                    ((cx + rx * 0.12, top_y + ry * 0.46), (cx, top_y + ry * 0.74)),
                ],
                "255,255,255",
                1.9,
            );
            art.apply_to_shape(sh, "14,165,233", "12,74,110", true);
        }
        "switch_1u" | "net_l2_switch" | "net_poe_switch" | "net_fiber_switch" => {
            // Designed natively for `185 × 52` 1U Isometric Switch Box — fills tight bounding box!
            let top_rgb = if role == "net_poe_switch" { "45,212,191" } else { "56,189,248" };
            let front_rgb = if role == "net_poe_switch" { "15,118,110" } else { "3,105,161" };
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            let dx = 18.0;
            let dy = 14.0;
            art.add_poly(
                &[(6.0, 6.0 + dy), (6.0 + dx, 6.0), (w - 6.0, 6.0), (w - 6.0 - dx, 6.0 + dy)],
                Some(top_rgb),
                Some("15,23,42"),
                1.4,
            );
            art.add_poly(
                &[(w - 6.0 - dx, 6.0 + dy), (w - 6.0, 6.0), (w - 6.0, h - 6.0 - dy), (w - 6.0 - dx, h - 6.0)],
                Some("12,74,110"),
                Some("15,23,42"),
                1.4,
            );
            let fw = w - 12.0 - dx;
            let fh = h - 12.0 - dy;
            art.add_rect(6.0, 6.0 + dy, fw, fh, 2.0, Some(front_rgb), Some("15,23,42"), 1.5);
            art.add_lines(
                &[
                    ((36.0, 11.0), (104.0, 11.0)),
                    ((94.0, 8.0), (105.0, 11.0)),
                    ((56.0, 16.0), (126.0, 16.0)),
                    ((66.0, 13.0), (55.0, 16.0)),
                ],
                "255,255,255",
                1.5,
            );
            for p in 0..8 {
                let px = 14.0 + (p as f64) * ((fw - 42.0) / 8.0);
                art.add_rect(px, 11.0 + dy, 10.0, fh - 10.0, 1.0, Some("15,23,42"), Some("56,189,248"), 0.9);
            }
            art.add_ellipse(6.0 + fw - 12.0, 6.0 + dy + fh * 0.5, 3.2, 3.2, Some("34,197,94"), None, 0.8);
            art.apply_to_shape(sh, front_rgb, "15,23,42", true);
        }
        "net_din_switch" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(8.0, 6.0, w - 16.0, h - 12.0, 4.0, Some("3,105,161"), Some("15,23,42"), 1.6);
            for r in 0..4 {
                let py = 20.0 + (r as f64) * 22.0;
                art.add_rect(18.0, py, w - 36.0, 14.0, 2.0, Some("15,23,42"), Some("56,189,248"), 1.0);
            }
            art.add_ellipse(w * 0.5, h - 16.0, 4.0, 4.0, Some("34,197,94"), None, 0.8);
            art.apply_to_shape(sh, "3,105,161", "15,23,42", true);
        }
        "net_l3_switch" | "net_chassis_sw" => {
            // Iconic Visio / Cisco 3D Square Layer-3 Multilayer Switch with Central Hub + 8 White Arrows!
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            let dx = 14.0;
            let dy = 12.0;
            let fw = w - 16.0 - dx;
            let fh = h - 16.0 - dy;
            art.add_poly(
                &[(8.0, 8.0 + dy), (8.0 + dx, 8.0), (8.0 + fw + dx, 8.0), (8.0 + fw, 8.0 + dy)],
                Some("56,189,248"),
                Some("12,74,110"),
                1.5,
            );
            art.add_poly(
                &[(8.0 + fw, 8.0 + dy), (8.0 + fw + dx, 8.0), (8.0 + fw + dx, 8.0 + fh), (8.0 + fw, 8.0 + dy + fh)],
                Some("7,89,133"),
                Some("12,74,110"),
                1.5,
            );
            art.add_rect(8.0, 8.0 + dy, fw, fh, 5.0, Some("2,132,199"), Some("12,74,110"), 1.6);
            let cx = 8.0 + fw * 0.5;
            let cy = 8.0 + dy + fh * 0.5;
            let r_hub = fw.min(fh) * 0.16;
            let r_arm = fw.min(fh) * 0.38;
            let r_diag = r_arm * 0.72;
            art.add_ellipse(cx, cy, r_hub, r_hub, None, Some("255,255,255"), 2.0);
            art.add_lines(
                &[
                    ((cx, cy - r_hub), (cx, cy - r_arm)),
                    ((cx, cy + r_hub), (cx, cy + r_arm)),
                    ((cx - r_hub, cy), (cx - r_arm, cy)),
                    ((cx + r_hub, cy), (cx + r_arm, cy)),
                    ((cx - r_hub * 0.7, cy - r_hub * 0.7), (cx - r_diag, cy - r_diag)),
                    ((cx + r_hub * 0.7, cy - r_hub * 0.7), (cx + r_diag, cy - r_diag)),
                    ((cx - r_hub * 0.7, cy + r_hub * 0.7), (cx - r_diag, cy + r_diag)),
                    ((cx + r_hub * 0.7, cy + r_hub * 0.7), (cx + r_diag, cy + r_diag)),
                ],
                "255,255,255",
                2.0,
            );
            art.apply_to_shape(sh, "2,132,199", "12,74,110", true);
        }
        "firewall" | "net_ngfw" | "net_waf" | "net_ids_ips" | "net_ddos" | "net_nac" => {
            // Iconic Visio 3D Crimson Mortared Brick Wall Firewall (`150 × 95` or `115 × 110`)
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            let dx = 12.0;
            let dy = 10.0;
            let fw = w - 18.0 - dx;
            let fh = h - 18.0 - dy;
            let x0 = 6.0;
            let y0 = 8.0 + dy;
            art.add_poly(
                &[(x0, y0), (x0 + dx, y0 - dy), (x0 + fw + dx, y0 - dy), (x0 + fw, y0)],
                Some("248,113,113"),
                Some("127,29,29"),
                1.4,
            );
            art.add_poly(
                &[(x0 + fw, y0), (x0 + fw + dx, y0 - dy), (x0 + fw + dx, y0 + fh - dy), (x0 + fw, y0 + fh)],
                Some("153,27,27"),
                Some("127,29,29"),
                1.4,
            );
            art.add_rect(x0, y0, fw, fh, 3.0, Some("220,38,38"), Some("127,29,29"), 1.6);
            let r1 = y0 + fh * 0.25;
            let r2 = y0 + fh * 0.50;
            let r3 = y0 + fh * 0.75;
            art.add_lines(
                &[
                    ((x0, r1), (x0 + fw, r1)),
                    ((x0, r2), (x0 + fw, r2)),
                    ((x0, r3), (x0 + fw, r3)),
                    ((x0 + fw * 0.33, y0), (x0 + fw * 0.33, r1)),
                    ((x0 + fw * 0.67, y0), (x0 + fw * 0.67, r1)),
                    ((x0 + fw * 0.18, r1), (x0 + fw * 0.18, r2)),
                    ((x0 + fw * 0.50, r1), (x0 + fw * 0.50, r2)),
                    ((x0 + fw * 0.82, r1), (x0 + fw * 0.82, r2)),
                    ((x0 + fw * 0.33, r2), (x0 + fw * 0.33, r3)),
                    ((x0 + fw * 0.67, r2), (x0 + fw * 0.67, r3)),
                    ((x0 + fw * 0.18, r3), (x0 + fw * 0.18, y0 + fh)),
                    ((x0 + fw * 0.50, r3), (x0 + fw * 0.50, y0 + fh)),
                ],
                "254,226,226",
                1.5,
            );
            if role != "firewall" {
                let scx = w - 24.0;
                let scy = h - 24.0;
                art.add_poly(
                    &[
                        (scx, scy - 18.0),
                        (scx + 18.0, scy - 11.0),
                        (scx + 15.0, scy + 7.0),
                        (scx, scy + 19.0),
                        (scx - 15.0, scy + 7.0),
                        (scx - 18.0, scy - 11.0),
                    ],
                    Some("245,158,11"),
                    Some("255,255,255"),
                    1.6,
                );
            }
            art.apply_to_shape(sh, "220,38,38", "127,29,29", true);
        }
        "net_wifi" | "net_mesh_ap" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(50.0 * s, 72.0 * s, 36.0 * s, 13.0 * s, Some("226,232,240"), Some("2,132,199"), 1.6);
            art.add_ellipse(50.0 * s, 70.0 * s, 7.0 * s, 4.5 * s, Some("2,132,199"), None, 0.8);
            art.add_open_arc((28.0 * s, 48.0 * s), (72.0 * s, 48.0 * s), 13.0 * s, "2,132,199", 2.4);
            art.add_open_arc((16.0 * s, 34.0 * s), (84.0 * s, 34.0 * s), 20.0 * s, "14,165,233", 2.4);
            art.apply_to_shape(sh, "226,232,240", "2,132,199", true);
        }
        "rack_42u" => {
            // Authentic Visio 42U Rack Elevation (`140 × 260` isotropic units)
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(8.0, 6.0, w - 16.0, h - 12.0, 4.0, Some("30,41,59"), Some("15,23,42"), 2.0);
            art.add_rect(18.0, 16.0, w - 36.0, h - 32.0, 2.0, Some("248,250,252"), Some("71,85,105"), 1.4);
            art.add_rect(22.0, 22.0, w - 44.0, 18.0, 1.5, Some("2,132,199"), Some("15,23,42"), 1.1);
            art.add_rect(22.0, 45.0, w - 44.0, 18.0, 1.5, Some("100,116,139"), Some("15,23,42"), 1.1);
            art.add_rect(22.0, 68.0, w - 44.0, 26.0, 1.5, Some("148,163,184"), Some("15,23,42"), 1.1);
            art.add_rect(22.0, h - 54.0, w - 44.0, 30.0, 1.5, Some("51,65,85"), Some("15,23,42"), 1.1);
            art.apply_to_shape(sh, "248,250,252", "15,23,42", true);
        }
        "cloud_vpc" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(5.0, 5.0, w - 10.0, h - 10.0, 10.0, Some("248,250,252"), Some("2,132,199"), 1.8);
            art.add_ellipse(24.0, 20.0, 10.0, 8.0, Some("56,189,248"), None, 0.8);
            art.add_ellipse(34.0, 22.0, 8.0, 6.5, Some("2,132,199"), None, 0.8);
            art.apply_to_shape(sh, "248,250,252", "2,132,199", false);
            sh.sheet
                .set_formula("TxtPinY", "=Height - 0.22 in", Some(Unit::Inch));
        }
        "net_patch_panel" | "net_pdu" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(4.0, 6.0, w - 8.0, h - 12.0, 3.0, Some("30,41,59"), Some("15,23,42"), 1.6);
            let n_ports = 10;
            let step = (w - 28.0) / (n_ports as f64);
            for p in 0..n_ports {
                let px = 14.0 + (p as f64) * step;
                let fill = if role == "net_pdu" { "239,68,68" } else { "56,189,248" };
                art.add_rect(px, 14.0, step - 4.0, h - 28.0, 1.0, Some(fill), None, 0.8);
            }
            art.apply_to_shape(sh, "30,41,59", "15,23,42", true);
        }
        "net_ups" => {
            let mut art = IsoArtwork::for_shape(sh);
            draw_3d_server_tower_base(&mut art, "234,179,8");
            art.apply_to_shape(sh, "100,116,139", "30,41,59", true);
        }
        "net_laptop" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(w * 0.18, h * 0.14, w * 0.64, h * 0.50, 4.0, Some("30,41,59"), Some("15,23,42"), 1.6);
            art.add_rect(w * 0.23, h * 0.20, w * 0.54, h * 0.38, 2.0, Some("56,189,248"), None, 0.8);
            art.add_poly(
                &[(w * 0.18, h * 0.64), (w * 0.82, h * 0.64), (w * 0.92, h * 0.82), (w * 0.08, h * 0.82)],
                Some("203,213,225"),
                Some("71,85,105"),
                1.4,
            );
            art.apply_to_shape(sh, "203,213,225", "30,41,59", true);
        }
        "net_voip" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(w * 0.24, h * 0.16, w * 0.62, h * 0.68, 7.0, Some("51,65,85"), Some("15,23,42"), 1.6);
            art.add_rect(w * 0.12, h * 0.12, w * 0.16, h * 0.76, 7.0, Some("30,41,59"), Some("15,23,42"), 1.4);
            art.add_rect(w * 0.36, h * 0.24, w * 0.40, h * 0.20, 2.0, Some("56,189,248"), None, 0.8);
            art.add_lines(
                &[
                    ((w * 0.40, h * 0.54), (w * 0.72, h * 0.54)),
                    ((w * 0.40, h * 0.64), (w * 0.72, h * 0.64)),
                    ((w * 0.40, h * 0.74), (w * 0.72, h * 0.74)),
                ],
                "203,213,225",
                2.0,
            );
            art.apply_to_shape(sh, "51,65,85", "15,23,42", true);
        }
        "net_printer" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(w * 0.26, h * 0.10, w * 0.48, h * 0.24, 2.0, Some("255,255,255"), Some("100,116,139"), 1.4);
            art.add_rect(w * 0.12, h * 0.32, w * 0.76, h * 0.38, 6.0, Some("71,85,105"), Some("30,41,59"), 1.6);
            art.add_rect(w * 0.26, h * 0.58, w * 0.48, h * 0.28, 2.0, Some("248,250,252"), Some("100,116,139"), 1.3);
            art.add_lines(
                &[((w * 0.34, h * 0.70), (w * 0.66, h * 0.70)), ((w * 0.34, h * 0.78), (w * 0.58, h * 0.78))],
                "148,163,184",
                1.5,
            );
            art.apply_to_shape(sh, "71,85,105", "30,41,59", true);
        }
        "net_ip_cam" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(50.0 * s, 48.0 * s, 34.0 * s, 26.0 * s, Some("226,232,240"), Some("51,65,85"), 1.6);
            art.add_ellipse(50.0 * s, 52.0 * s, 18.0 * s, 18.0 * s, Some("30,41,59"), Some("15,23,42"), 1.4);
            art.add_ellipse(50.0 * s, 52.0 * s, 8.0 * s, 8.0 * s, Some("2,132,199"), Some("56,189,248"), 1.2);
            art.apply_to_shape(sh, "226,232,240", "51,65,85", true);
        }
        "db_cluster" | "net_nas_san" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(w * 0.12, h * 0.22, w * 0.46, h * 0.56, 4.0, Some("2,132,199"), Some("7,89,133"), 1.5);
            art.add_ellipse(w * 0.35, h * 0.22, w * 0.23, h * 0.10, Some("56,189,248"), Some("7,89,133"), 1.4);
            art.add_rect(w * 0.42, h * 0.32, w * 0.46, h * 0.54, 4.0, Some("3,105,161"), Some("255,255,255"), 1.5);
            art.add_ellipse(w * 0.65, h * 0.32, w * 0.23, h * 0.10, Some("125,211,252"), Some("7,89,133"), 1.4);
            art.apply_to_shape(sh, "2,132,199", "7,89,133", true);
        }
        _ => {
            // 3D Network Appliance (`Load Balancer`, `VPN`, `SD-WAN`, `WLC`, `ODF`, `Modem`, `Tape Library`)
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            let dx = 14.0;
            let dy = 12.0;
            let fw = w - 16.0 - dx;
            let fh = h - 16.0 - dy;
            art.add_poly(
                &[(8.0, 8.0 + dy), (8.0 + dx, 8.0), (8.0 + fw + dx, 8.0), (8.0 + fw, 8.0 + dy)],
                Some("100,116,139"),
                Some("30,41,59"),
                1.4,
            );
            art.add_poly(
                &[(8.0 + fw, 8.0 + dy), (8.0 + fw + dx, 8.0), (8.0 + fw + dx, 8.0 + fh), (8.0 + fw, 8.0 + dy + fh)],
                Some("51,65,85"),
                Some("30,41,59"),
                1.4,
            );
            art.add_rect(8.0, 8.0 + dy, fw, fh, 3.0, Some("30,41,59"), Some("15,23,42"), 1.6);
            let cy = 8.0 + dy + fh * 0.5;
            art.add_lines(
                &[
                    ((22.0, cy), (8.0 + fw * 0.48, cy)),
                    ((8.0 + fw * 0.48, cy), (8.0 + fw * 0.78, cy - fh * 0.24)),
                    ((8.0 + fw * 0.48, cy), (8.0 + fw * 0.78, cy + fh * 0.24)),
                ],
                "56,189,248",
                2.0,
            );
            art.add_ellipse(20.0, 8.0 + dy + 10.0, 3.2, 3.2, Some("34,197,94"), None, 0.8);
            art.apply_to_shape(sh, "51,65,85", "15,23,42", true);
        }
    }
}

// ============================================================================
// 4. Microsoft Azure, Power Platform & Multi-Cloud Architecture Icons (`105 × 105` Square)
// ============================================================================

fn build_azure_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "az_resource_group" | "az_mgmt_group" | "az_vnet" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(5.0, 5.0, w - 10.0, h - 10.0, 9.0, Some("248,250,252"), Some("0,120,212"), 1.8);
            art.add_rect(14.0, 12.0, 28.0, 24.0, 4.0, Some("224,242,254"), Some("0,120,212"), 1.2);
            art.add_poly(&[(18.0, 31.0), (28.0, 15.0), (38.0, 31.0)], Some("0,120,212"), None, 0.8);
            art.apply_to_shape(sh, "248,250,252", "0,120,212", false);
            sh.sheet
                .set_formula("TxtPinY", "=Height - 0.22 in", Some(Unit::Inch));
        }
        "az_vm" | "az_vmss" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(12.0 * s, 14.0 * s, 76.0 * s, 54.0 * s, 7.0 * s, Some("0,120,212"), Some("0,90,158"), 1.6);
            art.add_poly(
                &[
                    (50.0 * s, 23.0 * s),
                    (66.0 * s, 32.0 * s),
                    (66.0 * s, 50.0 * s),
                    (50.0 * s, 59.0 * s),
                    (34.0 * s, 50.0 * s),
                    (34.0 * s, 32.0 * s),
                ],
                Some("255,255,255"),
                Some("0,90,158"),
                1.3,
            );
            art.add_poly(
                &[
                    (36.0 * s, 68.0 * s),
                    (64.0 * s, 68.0 * s),
                    (72.0 * s, 84.0 * s),
                    (28.0 * s, 84.0 * s),
                ],
                Some("148,163,184"),
                Some("71,85,105"),
                1.5,
            );
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "az_aks" | "az_container_apps" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            for &(cx, cy) in &[(34.0, 34.0), (66.0, 34.0), (50.0, 64.0)] {
                art.add_poly(
                    &[
                        (cx * s, (cy - 18.0) * s),
                        ((cx + 16.0) * s, (cy - 9.0) * s),
                        ((cx + 16.0) * s, (cy + 9.0) * s),
                        (cx * s, (cy + 18.0) * s),
                        ((cx - 16.0) * s, (cy + 9.0) * s),
                        ((cx - 16.0) * s, (cy - 9.0) * s),
                    ],
                    Some("124,58,237"),
                    Some("255,255,255"),
                    1.6,
                );
            }
            art.apply_to_shape(sh, "124,58,237", "91,33,182", true);
        }
        "az_functions" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(8.0 * s, 8.0 * s, 84.0 * s, 84.0 * s, 14.0 * s, Some("240,249,255"), Some("186,230,253"), 1.2);
            art.add_lines(
                &[
                    ((26.0 * s, 32.0 * s), (10.0 * s, 50.0 * s)),
                    ((10.0 * s, 50.0 * s), (26.0 * s, 68.0 * s)),
                    ((74.0 * s, 32.0 * s), (90.0 * s, 50.0 * s)),
                    ((90.0 * s, 50.0 * s), (74.0 * s, 68.0 * s)),
                ],
                "0,120,212",
                2.6,
            );
            art.add_poly(
                &[
                    (56.0 * s, 10.0 * s),
                    (32.0 * s, 52.0 * s),
                    (50.0 * s, 52.0 * s),
                    (42.0 * s, 90.0 * s),
                    (70.0 * s, 44.0 * s),
                    (52.0 * s, 44.0 * s),
                ],
                Some("250,204,21"),
                Some("202,138,4"),
                1.5,
            );
            art.apply_to_shape(sh, "250,204,21", "0,120,212", true);
        }
        "az_sql_db" | "az_sql_cosmos" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(20.0 * s, 20.0 * s, 60.0 * s, 62.0 * s, 6.0 * s, Some("0,120,212"), Some("0,90,158"), 1.6);
            art.add_ellipse(50.0 * s, 20.0 * s, 30.0 * s, 11.0 * s, Some("224,242,254"), Some("0,90,158"), 1.5);
            art.add_ellipse(50.0 * s, 82.0 * s, 30.0 * s, 10.0 * s, Some("0,120,212"), Some("0,90,158"), 1.5);
            draw_badge_letter(&mut art, "S", 36.0 * s, 54.0 * s, 9.0 * s);
            draw_badge_letter(&mut art, "O", 51.0 * s, 54.0 * s, 9.0 * s);
            draw_badge_letter(&mut art, "L", 66.0 * s, 54.0 * s, 9.0 * s);
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
        "az_firewall" => {
            build_network_equipment_artwork(sh, "net_ngfw");
        }
        "az_key_vault" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_ellipse(50.0 * s, 50.0 * s, 38.0 * s, 38.0 * s, Some("250,204,21"), Some("180,83,9"), 1.8);
            art.add_ellipse(42.0 * s, 44.0 * s, 11.0 * s, 11.0 * s, Some("255,255,255"), Some("180,83,9"), 1.8);
            art.add_lines(
                &[((50.0 * s, 52.0 * s), (70.0 * s, 72.0 * s)), ((64.0 * s, 66.0 * s), (58.0 * s, 72.0 * s))],
                "255,255,255",
                2.6,
            );
            art.apply_to_shape(sh, "250,204,21", "180,83,9", true);
        }
        _ => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(12.0 * s, 12.0 * s, 76.0 * s, 76.0 * s, 15.0 * s, Some("0,120,212"), Some("0,90,158"), 1.6);
            art.add_rect(12.0 * s, 12.0 * s, 76.0 * s, 28.0 * s, 15.0 * s, Some("40,168,234"), None, 0.8);
            art.add_poly(
                &[
                    (50.0 * s, 24.0 * s),
                    (74.0 * s, 70.0 * s),
                    (56.0 * s, 70.0 * s),
                    (46.0 * s, 50.0 * s),
                    (34.0 * s, 70.0 * s),
                    (24.0 * s, 70.0 * s),
                ],
                Some("255,255,255"),
                None,
                0.8,
            );
            art.apply_to_shape(sh, "0,120,212", "0,90,158", true);
        }
    }
}

fn build_power_platform_artwork(sh: &mut VisioShape, role: &str) {
    match role {
        "pp_managed_env" => {
            let mut art = IsoArtwork::for_shape(sh);
            let w = art.vb_w;
            let h = art.vb_h;
            art.add_rect(5.0, 5.0, w - 10.0, h - 10.0, 9.0, Some("248,250,252"), Some("116,39,116"), 1.8);
            art.add_rect(14.0, 12.0, 28.0, 26.0, 4.0, Some("243,232,255"), Some("116,39,116"), 1.2);
            art.add_poly(&[(28.0, 16.0), (37.0, 25.0), (28.0, 34.0), (19.0, 25.0)], Some("116,39,116"), None, 0.8);
            art.apply_to_shape(sh, "248,250,252", "116,39,116", false);
            sh.sheet
                .set_formula("TxtPinY", "=Height - 0.22 in", Some(Unit::Inch));
        }
        "pp_powerbi" | "pp_pbi_model" | "pp_pbi_paginated" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(18.0 * s, 48.0 * s, 18.0 * s, 40.0 * s, 5.0 * s, Some("253,224,71"), Some("202,138,4"), 1.4);
            art.add_rect(41.0 * s, 30.0 * s, 18.0 * s, 58.0 * s, 5.0 * s, Some("250,204,21"), Some("202,138,4"), 1.4);
            art.add_rect(64.0 * s, 12.0 * s, 18.0 * s, 76.0 * s, 5.0 * s, Some("234,179,8"), Some("161,98,7"), 1.4);
            art.apply_to_shape(sh, "250,204,21", "161,98,7", true);
        }
        "pp_automate" | "pp_rpa" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (10.0 * s, 20.0 * s),
                    (56.0 * s, 20.0 * s),
                    (88.0 * s, 50.0 * s),
                    (56.0 * s, 80.0 * s),
                    (10.0 * s, 80.0 * s),
                    (38.0 * s, 50.0 * s),
                ],
                Some("0,102,255"),
                Some("0,68,204"),
                1.6,
            );
            art.add_poly(
                &[
                    (10.0 * s, 20.0 * s),
                    (56.0 * s, 20.0 * s),
                    (72.0 * s, 35.0 * s),
                    (28.0 * s, 64.0 * s),
                ],
                Some("56,189,248"),
                None,
                0.8,
            );
            art.add_poly(
                &[
                    (38.0 * s, 50.0 * s),
                    (88.0 * s, 50.0 * s),
                    (56.0 * s, 80.0 * s),
                    (10.0 * s, 80.0 * s),
                ],
                Some("29,78,216"),
                None,
                0.8,
            );
            art.apply_to_shape(sh, "0,102,255", "0,68,204", true);
        }
        "pp_powerapps" | "pp_model_app" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (50.0 * s, 10.0 * s),
                    (90.0 * s, 50.0 * s),
                    (50.0 * s, 90.0 * s),
                    (10.0 * s, 50.0 * s),
                ],
                Some("116,39,116"),
                Some("74,21,75"),
                1.6,
            );
            art.add_poly(
                &[
                    (50.0 * s, 26.0 * s),
                    (74.0 * s, 50.0 * s),
                    (50.0 * s, 74.0 * s),
                    (26.0 * s, 50.0 * s),
                ],
                Some("192,132,252"),
                Some("255,255,255"),
                1.4,
            );
            art.add_poly(
                &[
                    (50.0 * s, 38.0 * s),
                    (62.0 * s, 50.0 * s),
                    (50.0 * s, 62.0 * s),
                    (38.0 * s, 50.0 * s),
                ],
                Some("255,255,255"),
                None,
                0.8,
            );
            art.apply_to_shape(sh, "116,39,116", "74,21,75", true);
        }
        "pp_dataverse" | "ms_fabric" => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_rect(14.0 * s, 18.0 * s, 72.0 * s, 20.0 * s, 7.0 * s, Some("16,185,129"), Some("4,120,87"), 1.5);
            art.add_rect(14.0 * s, 42.0 * s, 72.0 * s, 20.0 * s, 7.0 * s, Some("5,150,105"), Some("4,120,87"), 1.5);
            art.add_rect(14.0 * s, 66.0 * s, 72.0 * s, 20.0 * s, 7.0 * s, Some("4,120,87"), Some("6,78,59"), 1.5);
            art.apply_to_shape(sh, "16,185,129", "4,120,87", true);
        }
        _ => {
            let mut art = IsoArtwork::for_shape(sh);
            let s = art.vb_w / 100.0;
            art.add_poly(
                &[
                    (20.0 * s, 12.0 * s),
                    (84.0 * s, 34.0 * s),
                    (54.0 * s, 52.0 * s),
                    (20.0 * s, 42.0 * s),
                ],
                Some("0,120,212"),
                Some("0,32,80"),
                1.5,
            );
            art.add_poly(
                &[
                    (36.0 * s, 48.0 * s),
                    (84.0 * s, 34.0 * s),
                    (84.0 * s, 72.0 * s),
                    (20.0 * s, 90.0 * s),
                ],
                Some("40,168,234"),
                Some("0,32,80"),
                1.5,
            );
            art.add_ellipse(74.0 * s, 34.0 * s, 6.0 * s, 6.0 * s, Some("255,255,255"), None, 0.8);
            art.apply_to_shape(sh, "0,120,212", "0,32,80", true);
        }
    }
}

fn build_cloud_arch_artwork(sh: &mut VisioShape, role: &str) {
    let (bg_rgb, border_rgb) = match role {
        "cloud_vm" | "cloud_asg" | "cloud_spot" | "cloud_fn" | "cloud_batch" => {
            ("255,153,0", "180,83,9")
        }
        "cloud_bucket" | "cloud_ebs" | "cloud_glacier" => ("22,163,74", "21,128,61"),
        "cloud_sql" | "cloud_nosql" | "cloud_cache" | "cloud_dwh" => ("59,130,246", "29,78,216"),
        "cloud_iam" | "cloud_kms" => ("239,68,68", "185,28,28"),
        _ => ("99,102,241", "67,56,202"),
    };
    let mut art = IsoArtwork::for_shape(sh);
    let s = art.vb_w / 100.0;
    art.add_rect(12.0 * s, 12.0 * s, 76.0 * s, 76.0 * s, 15.0 * s, Some(bg_rgb), Some(border_rgb), 1.8);
    if role.contains("k8s") || role.contains("pod") {
        art.add_poly(
            &[
                (50.0 * s, 22.0 * s),
                (74.0 * s, 36.0 * s),
                (74.0 * s, 64.0 * s),
                (50.0 * s, 78.0 * s),
                (26.0 * s, 64.0 * s),
                (26.0 * s, 36.0 * s),
            ],
            None,
            Some("255,255,255"),
            2.2,
        );
        art.add_ellipse(50.0 * s, 50.0 * s, 10.0 * s, 10.0 * s, Some("255,255,255"), None, 0.8);
    } else if role.contains("sql")
        || role.contains("nosql")
        || role.contains("dwh")
        || role.contains("cache")
    {
        art.add_ellipse(50.0 * s, 32.0 * s, 22.0 * s, 8.0 * s, Some("255,255,255"), None, 0.8);
        art.add_lines(
            &[
                ((28.0 * s, 32.0 * s), (28.0 * s, 68.0 * s)),
                ((72.0 * s, 32.0 * s), (72.0 * s, 68.0 * s)),
                ((28.0 * s, 68.0 * s), (72.0 * s, 68.0 * s)),
            ],
            "255,255,255",
            2.2,
        );
    } else if role == "cloud_fn" {
        art.add_lines(
            &[
                ((34.0 * s, 72.0 * s), (54.0 * s, 28.0 * s)),
                ((46.0 * s, 48.0 * s), (66.0 * s, 72.0 * s)),
            ],
            "255,255,255",
            2.8,
        );
    } else {
        art.add_rect(28.0 * s, 28.0 * s, 44.0 * s, 44.0 * s, 7.0 * s, None, Some("255,255,255"), 2.2);
        art.add_rect(38.0 * s, 38.0 * s, 24.0 * s, 24.0 * s, 4.0 * s, Some("255,255,255"), None, 0.8);
    }
    art.apply_to_shape(sh, bg_rgb, border_rgb, true);
}
