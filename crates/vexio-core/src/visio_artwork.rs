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
    ForeignMediaData, GeometryRow, GeometryRowKind, GeometrySection, ShapeKind, Unit, VisioShape,
    format_clean_f64,
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
            apply_visio_below_icon_caption(sh);
        }
    }
}

/// Configures Visio-standard external below-icon caption cells (`TxtPinY = -TxtHeight*0.5 - 0.04 in`,
/// `TxtLocPinY = TxtHeight*0.5`) so dropped architecture icons hang crisp, multi-line labels cleanly underneath.
pub fn apply_visio_below_icon_caption(sh: &mut VisioShape) {
    sh.sheet
        .set_formula("TxtPinX", "=Width*0.5", Some(Unit::Inch));
    sh.sheet
        .set_formula("TxtPinY", "=-TxtHeight*0.5 - 0.04 in", Some(Unit::Inch));
    sh.sheet
        .set_formula("TxtWidth", "=MAX(Width*1.85, 1.18 in)", Some(Unit::Inch));
    sh.sheet
        .set_formula("TxtHeight", "0.28 in", Some(Unit::Inch));
    sh.sheet
        .set_formula("TxtLocPinX", "=TxtWidth*0.5", Some(Unit::Inch));
    sh.sheet
        .set_formula("TxtLocPinY", "=TxtHeight*0.5", Some(Unit::Inch));
    sh.sheet
        .set_formula("TextBkgnd", "0", Some(Unit::Number));
    sh.sheet
        .set_formula("Char.Size", "9 pt", Some(Unit::Point));
    sh.sheet
        .set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));
}

fn apply_svg_foreign_artwork(
    sh: &mut VisioShape,
    svg_markup: &str,
    fallback_fill_rgb: &'static str,
    fallback_line_rgb: &'static str,
    label_below: bool,
) {
    sh.kind = ShapeKind::ForeignObject;
    sh.sheet.geometries.clear();
    let mut frame = GeometrySection::rectangle(1);
    frame.no_fill = "TRUE".into();
    frame.no_line = "TRUE".into();
    sh.sheet.add_geometry(frame);
    sh.sheet
        .set_formula("FillForegnd", format!("=RGB({fallback_fill_rgb})"), Some(Unit::Color));
    sh.sheet
        .set_formula("FillPattern", "0", Some(Unit::Number));
    sh.sheet
        .set_formula("LineColor", format!("=RGB({fallback_line_rgb})"), Some(Unit::Color));
    sh.sheet
        .set_formula("LinePattern", "0", Some(Unit::Number));
    sh.sheet
        .set_user_cell("ForeignSvgPrimary", "TRUE", "Primary SVG Stencil Vector");
    sh.foreign_data = Some(ForeignMediaData {
        media_type: "SVG".into(),
        rel_id: None,
        data: svg_markup.as_bytes().to_vec(),
    });
    if label_below {
        apply_visio_below_icon_caption(sh);
    }
    sh.sheet.sync_all_sections_to_cells();
}

/// High-precision Visio architecture vector icons matching `MSSC_Architektur_1` / `OfficeSymbols` / `Azure V25` / `NetEquip`.
fn try_apply_high_precision_svg_stencil(sh: &mut VisioShape, role: &str) -> bool {
    let spec: Option<(&'static str, &'static str, &'static str)> = match role {
        "m365_cloud" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 96 60"><path d="M22,50 C11,50 4,43 4,33 C4,24 10,17 19,16 C23,7 33,2 45,3 C58,4 68,12 71,23 C81,23 90,30 90,39 C90,46 84,50 75,50 Z M37,43 L37,20 L51,15 L60,18 L60,44 L51,47 L37,42 L51,44 L51,19 L42,22 L42,40 Z" fill="#DC3C00" fill-rule="evenodd"/></svg>"##,
            "220,60,0",
            "180,45,0",
        )),
        "m365_exchange" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 64"><path d="M16,49 C8,49 3,43 3,36 C3,29 8,24 15,23 C18,14 27,9 37,10 C47,11 55,17 57,26 C64,26 70,31 70,38 C70,45 64,49 56,49 Z" fill="#FFFFFF" stroke="#1E293B" stroke-width="2.6"/><path d="M17,46 C11,46 7,42 7,36 C7,31 11,27 17,26 C20,18 27,14 36,15 C45,16 51,21 53,29 C59,29 65,33 65,38 C65,43 60,46 54,46 Z" fill="none" stroke="#0078D4" stroke-width="2.2"/><rect x="44" y="10" width="24" height="24" rx="3" fill="#0078D4"/><rect x="60" y="12" width="14" height="9" rx="1.5" fill="#50E6FF"/><rect x="60" y="23" width="14" height="9" rx="1.5" fill="#28A8EA"/><path d="M49,15 H58 V17.5 H52 V20.5 H57.5 V23 H52 V26.5 H58 V29 H49 Z" fill="#FFFFFF"/></svg>"##,
            "0,120,212",
            "30,41,59",
        )),
        "m365_eop" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 84 68"><path d="M16,49 C8,49 3,43 3,36 C3,29 8,24 15,23 C18,14 27,9 37,10 C47,11 55,17 57,26 C64,26 70,31 70,38 C70,45 64,49 56,49 Z" fill="#FFFFFF" stroke="#1E293B" stroke-width="2.6"/><path d="M17,46 C11,46 7,42 7,36 C7,31 11,27 17,26 C20,18 27,14 36,15 C45,16 51,21 53,29 C59,29 65,33 65,38 C65,43 60,46 54,46 Z" fill="none" stroke="#0078D4" stroke-width="2.2"/><rect x="44" y="9" width="22" height="22" rx="3" fill="#0078D4"/><rect x="59" y="11" width="12" height="8" rx="1.5" fill="#50E6FF"/><rect x="59" y="21" width="12" height="8" rx="1.5" fill="#28A8EA"/><path d="M48,14 H56.5 V16.5 H51 V19 H56 V21.2 H51 V24 H56.5 V26.5 H48 Z" fill="#FFFFFF"/><path d="M65,36 L77,40 V49 C77,56 71.5,61 65,64 C58.5,61 53,56 53,49 V40 Z" fill="#57A300" stroke="#FFFFFF" stroke-width="1.6"/><rect x="61" y="48" width="8" height="7" rx="1.2" fill="#FFFFFF"/><path d="M62.5,48 V45.5 C62.5,44 63.6,43 65,43 C66.4,43 67.5,44 67.5,45.5 V48" fill="none" stroke="#FFFFFF" stroke-width="1.8"/></svg>"##,
            "0,120,212",
            "87,163,0",
        )),
        "m365_sharepoint" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="38" cy="21" r="14" fill="#036C70"/><circle cx="46" cy="38" r="12" fill="#1A9BA1"/><circle cx="35" cy="49" r="10" fill="#37C6D0"/><rect x="6" y="16" width="32" height="32" rx="5" fill="#038387"/><path d="M26,25.5 C24.2,24.4 22.2,24 20.4,24 C17.2,24 15,25.7 15,28.2 C15,30.6 16.8,31.8 20.6,33 C23.6,33.9 24.6,34.7 24.6,36.2 C24.6,37.8 23.1,38.9 20.7,38.9 C18.5,38.9 16.5,38.1 14.9,36.8 L13.4,39.4 C15.3,41 17.8,41.8 20.6,41.8 C25,41.8 28,39.6 28,36 C28,33.3 26.3,31.8 22.2,30.6 C19.3,29.7 18.3,29 18.3,27.8 C18.3,26.6 19.6,25.7 21.6,25.7 C23.3,25.7 24.8,26.2 26,27.2 Z" fill="#FFFFFF"/></svg>"##,
            "3,131,135",
            "3,108,112",
        )),
        "m365_teams" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><circle cx="49" cy="19" r="6" fill="#7B83EB"/><rect x="41" y="27" width="18" height="21" rx="5" fill="#7B83EB"/><circle cx="36" cy="16" r="7.5" fill="#5059C9"/><rect x="24" y="26" width="25" height="28" rx="6" fill="#5059C9"/><rect x="5" y="17" width="30" height="30" rx="5" fill="#4B53BC"/><path d="M13,24 H27 V27.5 H21.8 V40 H18.2 V27.5 H13 Z" fill="#FFFFFF"/></svg>"##,
            "80,89,201",
            "75,83,188",
        )),
        "m365_onedrive" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 76 56"><path d="M29,18 C34,11 44,10 51,16 C55,19 57,24 57,28 C64,28 70,33 70,40 C70,47 64,51 56,51 H19 C11,51 5,45 5,38 C5,31 10,25 18,24 C20,20 24,18 29,18 Z" fill="#0078D4"/><path d="M18,24 C24,23 31,26 35,32 L19,51 C11,51 5,45 5,38 C5,31 10,25 18,24 Z" fill="#0364B8"/><path d="M35,32 C41,28 50,27 57,28 C64,28 70,33 70,40 C70,47 64,51 56,51 H19 Z" fill="#28A8EA"/></svg>"##,
            "40,168,234",
            "0,120,212",
        )),
        "m365_planner" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="8" y="8" width="18" height="48" rx="4" fill="#185C37"/><rect x="24" y="8" width="18" height="36" rx="4" fill="#21A366"/><rect x="40" y="8" width="17" height="25" rx="4" fill="#33C481"/></svg>"##,
            "33,163,102",
            "24,92,55",
        )),
        "m365_purview" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 56"><defs><linearGradient id="pv1" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" stop-color="#50E6FF"/><stop offset="100%" stop-color="#0078D4"/></linearGradient></defs><path d="M8,30 C14,15 31,9 48,15 C57,18 63,24 66,30 C59,20 45,17 33,21 C24,24 19,31 21,37 C15,36 10,33 8,30 Z" fill="url(#pv1)"/><path d="M64,26 C58,41 41,47 24,41 C15,38 9,32 6,26 C13,36 27,39 39,35 C48,32 53,25 51,19 C57,20 62,23 64,26 Z" fill="#0078D4"/><circle cx="36" cy="28" r="7.5" fill="#32BEDD"/></svg>"##,
            "0,120,212",
            "50,190,221",
        )),
        "m365_dep" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><path d="M32,6 L54,14 V31 C54,45 44,54 32,59 C20,54 10,45 10,31 V14 Z" fill="#D83B01"/><rect x="29.5" y="17" width="5" height="20" rx="2.5" fill="#FFFFFF"/><circle cx="32" cy="45" r="3.2" fill="#FFFFFF"/></svg>"##,
            "216,59,1",
            "168,42,0",
        )),
        "m365_universal_print" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="18" y="8" width="28" height="18" rx="2" fill="#E2E8F0" stroke="#0078D4" stroke-width="1.8"/><rect x="9" y="24" width="46" height="22" rx="4" fill="#50E6FF" stroke="#0078D4" stroke-width="1.8"/><circle cx="42" cy="34" r="2" fill="#0078D4"/><circle cx="48" cy="34" r="2" fill="#0078D4"/><rect x="18" y="40" width="28" height="16" rx="2" fill="#FFFFFF" stroke="#0078D4" stroke-width="1.8"/></svg>"##,
            "80,230,255",
            "0,120,212",
        )),
        "m365_apps" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="o365r" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" stop-color="#FF5C26"/><stop offset="100%" stop-color="#D83B01"/></linearGradient></defs><path d="M14,47 L14,17 L39,8 L39,15 L23,20 L23,44 Z" fill="#EB3C00"/><path d="M39,8 L52,12 L52,52 L39,56 Z" fill="url(#o365r)"/><path d="M14,47 L23,44 L39,50 L39,56 Z" fill="#A82400"/></svg>"##,
            "216,59,1",
            "168,42,0",
        )),
        "pp_powerbi" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="pbi1" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" stop-color="#E6AD10"/><stop offset="100%" stop-color="#C87E0E"/></linearGradient><linearGradient id="pbi2" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" stop-color="#F6D751"/><stop offset="100%" stop-color="#E6AD10"/></linearGradient><linearGradient id="pbi3" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" stop-color="#F9E589"/><stop offset="100%" stop-color="#F6D751"/></linearGradient></defs><rect x="38" y="8" width="16" height="48" rx="3" fill="url(#pbi1)"/><rect x="24" y="21" width="16" height="35" rx="3" fill="url(#pbi2)"/><rect x="10" y="34" width="16" height="22" rx="3" fill="url(#pbi3)"/></svg>"##,
            "242,200,17",
            "200,126,14",
        )),
        "ms_ad_dc" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 68 62"><path d="M30,6 C32,2.5 36,2.5 38,6 L63,50 C65,53.5 62.5,58 58.5,58 H9.5 C5.5,58 3,53.5 5,50 Z" fill="#0078D4" stroke="#FFFFFF" stroke-width="2"/><rect x="29" y="19" width="10" height="8" rx="1.2" fill="#FFFFFF"/><rect x="17" y="42" width="10" height="8" rx="1.2" fill="#FFFFFF"/><rect x="41" y="42" width="10" height="8" rx="1.2" fill="#FFFFFF"/><path d="M34,27 V35 M22,35 H46 M22,35 V42 M46,35 V42" stroke="#FFFFFF" stroke-width="2.4" fill="none"/></svg>"##,
            "0,120,212",
            "0,90,158",
        )),
        "ms_exchange_srv" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 68 68"><rect x="12" y="6" width="32" height="50" rx="3" fill="#64748B" stroke="#334155" stroke-width="1.8"/><rect x="17" y="13" width="22" height="6" rx="1.5" fill="#1E293B"/><rect x="17" y="24" width="22" height="6" rx="1.5" fill="#1E293B"/><circle cx="37" cy="16" r="1.8" fill="#38BDF8"/><circle cx="37" cy="27" r="1.8" fill="#38BDF8"/><rect x="33" y="34" width="22" height="22" rx="3" fill="#0078D4"/><rect x="48" y="36" width="12" height="8" rx="1.5" fill="#50E6FF"/><rect x="48" y="46" width="12" height="8" rx="1.5" fill="#28A8EA"/><path d="M37,39 H45.5 V41.5 H40 V44 H45 V46.2 H40 V49 H45.5 V51.5 H37 Z" fill="#FFFFFF"/></svg>"##,
            "100,116,139",
            "0,120,212",
        )),
        "ms_entra_connect" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 68 68"><rect x="12" y="6" width="32" height="50" rx="3" fill="#64748B" stroke="#334155" stroke-width="1.8"/><rect x="17" y="13" width="22" height="6" rx="1.5" fill="#1E293B"/><rect x="17" y="24" width="22" height="6" rx="1.5" fill="#1E293B"/><circle cx="46" cy="46" r="13" fill="#8B5CF6" stroke="#FFFFFF" stroke-width="1.8"/><path d="M40,44 A6.5,6.5 0 0,1 51,41 M52,48 A6.5,6.5 0 0,1 41,51" stroke="#FFFFFF" stroke-width="2.4" fill="none"/><polygon points="52,38 53,44 47,43" fill="#FFFFFF"/><polygon points="40,54 39,48 45,49" fill="#FFFFFF"/></svg>"##,
            "139,92,246",
            "51,65,85",
        )),
        "ms_entra_id" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="eid1" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" stop-color="#50E6FF"/><stop offset="100%" stop-color="#0078D4"/></linearGradient></defs><polygon points="32,6 6,41 32,58 58,41" fill="url(#eid1)"/><polygon points="32,6 32,58 58,41" fill="#32BEDD" fill-opacity="0.65"/><polygon points="32,24 16,41 32,51 48,41" fill="#9CEBFF"/><line x1="6" y1="41" x2="58" y2="41" stroke="#FFFFFF" stroke-width="1.5"/></svg>"##,
            "0,120,212",
            "50,190,221",
        )),
        "ms_system_center" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="10" y="8" width="26" height="46" rx="2" fill="#0078D4"/><rect x="14" y="13" width="5" height="5" fill="#FFFFFF"/><rect x="21" y="13" width="5" height="5" fill="#FFFFFF"/><rect x="28" y="13" width="5" height="5" fill="#FFFFFF"/><rect x="14" y="21" width="5" height="5" fill="#FFFFFF"/><rect x="21" y="21" width="5" height="5" fill="#FFFFFF"/><rect x="28" y="21" width="5" height="5" fill="#FFFFFF"/><rect x="14" y="29" width="5" height="5" fill="#FFFFFF"/><rect x="21" y="29" width="5" height="5" fill="#FFFFFF"/><circle cx="45" cy="33" r="6" fill="#0078D4"/><path d="M34,54 C34,45 39,41 45,41 C51,41 56,45 56,54 Z" fill="#0078D4"/><circle cx="15" cy="49" r="7" fill="#50E6FF" stroke="#FFFFFF" stroke-width="1.8"/></svg>"##,
            "0,120,212",
            "0,90,158",
        )),
        "m365_intune" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 68 64"><rect x="14" y="8" width="46" height="32" rx="4" fill="#0078D4"/><rect x="18" y="13" width="38" height="22" rx="1.5" fill="#FFFFFF"/><rect x="29" y="18" width="16" height="11" rx="1.5" fill="none" stroke="#0078D4" stroke-width="2"/><path d="M33,18 V15.5 H41 V18" fill="none" stroke="#0078D4" stroke-width="1.8"/><rect x="8" y="26" width="18" height="32" rx="3" fill="#0078D4" stroke="#FFFFFF" stroke-width="1.8"/><rect x="11" y="30" width="12" height="22" rx="1" fill="#FFFFFF"/></svg>"##,
            "0,120,212",
            "0,90,158",
        )),
        "ms_onprem_building" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 56 68"><rect x="10" y="6" width="36" height="56" rx="2" fill="#0078D4"/><rect x="16" y="13" width="6" height="6" fill="#FFFFFF"/><rect x="25" y="13" width="6" height="6" fill="#FFFFFF"/><rect x="34" y="13" width="6" height="6" fill="#FFFFFF"/><rect x="16" y="24" width="6" height="6" fill="#FFFFFF"/><rect x="25" y="24" width="6" height="6" fill="#FFFFFF"/><rect x="34" y="24" width="6" height="6" fill="#FFFFFF"/><rect x="16" y="35" width="6" height="6" fill="#FFFFFF"/><rect x="25" y="35" width="6" height="6" fill="#FFFFFF"/><rect x="34" y="35" width="6" height="6" fill="#FFFFFF"/><rect x="24" y="48" width="8" height="14" fill="#FFFFFF"/></svg>"##,
            "0,120,212",
            "0,90,158",
        )),
        "ms_device_stack" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 60"><rect x="8" y="10" width="40" height="26" rx="2.5" fill="#64748B" stroke="#0078D4" stroke-width="2"/><rect x="12" y="14" width="32" height="18" fill="#FFFFFF"/><polygon points="4,36 52,36 56,41 0,41" fill="#64748B" stroke="#0078D4" stroke-width="1.6"/><rect x="52" y="20" width="20" height="34" rx="3" fill="#64748B" stroke="#0078D4" stroke-width="2"/><rect x="55" y="25" width="14" height="22" fill="#FFFFFF"/><circle cx="62" cy="50.5" r="1.5" fill="#FFFFFF"/></svg>"##,
            "100,116,139",
            "0,120,212",
        )),
        "ms_win11_client" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><polygon points="8,14 29,11 29,30 8,30" fill="#002080"/><polygon points="32,10.5 56,7 56,30 32,30" fill="#002080"/><polygon points="8,33 29,33 29,52 8,49" fill="#002080"/><polygon points="32,33 56,33 56,56 32,52.5" fill="#002080"/></svg>"##,
            "0,32,128",
            "0,32,128",
        )),
        "az_sentinel" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="snt1" x1="0%" y1="0%" x2="0%" y2="100%"><stop offset="0%" stop-color="#50E6FF"/><stop offset="100%" stop-color="#0078D4"/></linearGradient></defs><path d="M32,5 L55,13 V31 C55,45 44.5,55 32,60 C19.5,55 9,45 9,31 V13 Z" fill="url(#snt1)"/><path d="M18,31 A14,14 0 0,1 46,31" fill="none" stroke="#FFFFFF" stroke-width="4.2"/><circle cx="32" cy="34" r="5.5" fill="#FFFFFF"/></svg>"##,
            "0,120,212",
            "80,230,255",
        )),
        "az_vnet_ring" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="vnr1" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" stop-color="#60A5FA"/><stop offset="100%" stop-color="#2563EB"/></linearGradient></defs><circle cx="32" cy="32" r="22" fill="#FFFFFF" stroke="url(#vnr1)" stroke-width="4.5"/><circle cx="24" cy="32" r="2.6" fill="#0078D4"/><circle cx="32" cy="32" r="2.6" fill="#50E6FF"/><circle cx="40" cy="32" r="2.6" fill="#0078D4"/><path d="M19,23 L14,32 L19,41 M45,23 L50,32 L45,41" fill="none" stroke="#3B82F6" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##,
            "255,255,255",
            "59,130,246",
        )),
        "az_app_service" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><defs><linearGradient id="aps1" x1="0%" y1="0%" x2="100%" y2="100%"><stop offset="0%" stop-color="#50E6FF"/><stop offset="100%" stop-color="#0078D4"/></linearGradient></defs><circle cx="32" cy="32" r="24" fill="url(#aps1)"/><path d="M13,22 C24,18 42,24 51,38 M15,42 C25,30 39,22 49,18 M24,10 C26,26 34,42 42,53" fill="none" stroke="#FFFFFF" stroke-width="2.6"/><circle cx="23" cy="24" r="4" fill="#FFFFFF"/><circle cx="42" cy="32" r="3.5" fill="#FFFFFF"/><circle cx="27" cy="43" r="3.5" fill="#FFFFFF"/></svg>"##,
            "0,120,212",
            "80,230,255",
        )),
        "net_cloud_hub" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 72 72"><line x1="36" y1="36" x2="36" y2="11" stroke="#0078D4" stroke-width="4"/><line x1="36" y1="36" x2="13" y2="26" stroke="#0078D4" stroke-width="4"/><line x1="36" y1="36" x2="59" y2="26" stroke="#0078D4" stroke-width="4"/><line x1="36" y1="36" x2="21" y2="57" stroke="#0078D4" stroke-width="4"/><line x1="36" y1="36" x2="51" y2="57" stroke="#0078D4" stroke-width="4"/><circle cx="36" cy="36" r="9" fill="#0078D4"/><circle cx="36" cy="11" r="6.5" fill="#0078D4"/><circle cx="13" cy="26" r="6.5" fill="#0078D4"/><circle cx="59" cy="26" r="6.5" fill="#0078D4"/><circle cx="21" cy="57" r="6.5" fill="#0078D4"/><circle cx="51" cy="57" r="6.5" fill="#0078D4"/></svg>"##,
            "0,120,212",
            "0,90,158",
        )),
        "rack_cisco_48p" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 190 36"><rect x="1" y="2" width="8" height="32" rx="1" fill="#64748B" stroke="#1E293B" stroke-width="1"/><rect x="181" y="2" width="8" height="32" rx="1" fill="#64748B" stroke="#1E293B" stroke-width="1"/><rect x="9" y="2" width="172" height="32" rx="1.5" fill="#0F4C68" stroke="#082F49" stroke-width="1.4"/><rect x="13" y="6" width="14" height="24" rx="1" fill="#082F49"/><circle cx="17" cy="11" r="1.6" fill="#22C55E"/><circle cx="17" cy="17" r="1.6" fill="#22C55E"/><circle cx="17" cy="23" r="1.6" fill="#38BDF8"/><rect x="31" y="6" width="116" height="10" rx="1" fill="#082F49" stroke="#38BDF8" stroke-width="0.7"/><rect x="31" y="20" width="116" height="10" rx="1" fill="#082F49" stroke="#38BDF8" stroke-width="0.7"/><path d="M40,6 V30 M50,6 V30 M60,6 V30 M70,6 V30 M80,6 V30 M90,6 V30 M100,6 V30 M110,6 V30 M120,6 V30 M130,6 V30 M140,6 V30" stroke="#38BDF8" stroke-width="0.7"/><rect x="151" y="7" width="11" height="9" fill="#020617" stroke="#FACC15" stroke-width="0.9"/><rect x="165" y="7" width="11" height="9" fill="#020617" stroke="#FACC15" stroke-width="0.9"/><rect x="151" y="20" width="11" height="9" fill="#020617" stroke="#FACC15" stroke-width="0.9"/><rect x="165" y="20" width="11" height="9" fill="#020617" stroke="#FACC15" stroke-width="0.9"/></svg>"##,
            "15,76,104",
            "8,47,73",
        )),
        "rack_cisco_asr2u" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 190 62"><rect x="1" y="2" width="8" height="58" rx="1" fill="#64748B" stroke="#1E293B" stroke-width="1"/><rect x="181" y="2" width="8" height="58" rx="1" fill="#64748B" stroke="#1E293B" stroke-width="1"/><rect x="9" y="2" width="172" height="58" rx="2" fill="#94A3B8" stroke="#1E293B" stroke-width="1.5"/><rect x="14" y="7" width="46" height="48" rx="1.5" fill="#1E293B"/><path d="M18,13 H56 M18,19 H56 M18,25 H56 M18,31 H56 M18,37 H56 M18,43 H56 M18,49 H56" stroke="#475569" stroke-width="1.5"/><rect x="65" y="7" width="56" height="22" rx="1.5" fill="#0F4C68" stroke="#082F49" stroke-width="1.1"/><rect x="65" y="33" width="56" height="22" rx="1.5" fill="#0F4C68" stroke="#082F49" stroke-width="1.1"/><rect x="126" y="7" width="50" height="22" rx="1.5" fill="#334155" stroke="#0F172A" stroke-width="1.1"/><rect x="126" y="33" width="50" height="22" rx="1.5" fill="#334155" stroke="#0F172A" stroke-width="1.1"/><circle cx="169" cy="14" r="2.2" fill="#22C55E"/><circle cx="169" cy="40" r="2.2" fill="#22C55E"/></svg>"##,
            "148,163,184",
            "30,41,59",
        )),
        "rack_panduit_24p" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 190 36"><rect x="1" y="2" width="8" height="32" rx="1" fill="#334155" stroke="#0F172A" stroke-width="1"/><rect x="181" y="2" width="8" height="32" rx="1" fill="#334155" stroke="#0F172A" stroke-width="1"/><rect x="9" y="2" width="172" height="32" rx="1.5" fill="#1E293B" stroke="#0F172A" stroke-width="1.4"/><rect x="15" y="10" width="36" height="16" fill="#0F172A" stroke="#64748B" stroke-width="0.9"/><rect x="55" y="10" width="36" height="16" fill="#0F172A" stroke="#64748B" stroke-width="0.9"/><rect x="99" y="10" width="36" height="16" fill="#0F172A" stroke="#64748B" stroke-width="0.9"/><rect x="139" y="10" width="36" height="16" fill="#0F172A" stroke="#64748B" stroke-width="0.9"/><path d="M21,10 V26 M27,10 V26 M33,10 V26 M39,10 V26 M45,10 V26 M61,10 V26 M67,10 V26 M73,10 V26 M79,10 V26 M85,10 V26 M105,10 V26 M111,10 V26 M117,10 V26 M123,10 V26 M129,10 V26 M145,10 V26 M151,10 V26 M157,10 V26 M163,10 V26 M169,10 V26" stroke="#94A3B8" stroke-width="0.8"/></svg>"##,
            "30,41,59",
            "15,23,42",
        )),
        "rack_apc_ups2u" => Some((
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 190 62"><rect x="1" y="2" width="8" height="58" rx="1" fill="#334155" stroke="#0F172A" stroke-width="1"/><rect x="181" y="2" width="8" height="58" rx="1" fill="#334155" stroke="#0F172A" stroke-width="1"/><rect x="9" y="2" width="172" height="58" rx="2" fill="#18181B" stroke="#09090B" stroke-width="1.5"/><rect x="16" y="9" width="58" height="44" rx="1.5" fill="#27272A"/><path d="M20,16 H70 M20,23 H70 M20,30 H70 M20,37 H70 M20,44 H70" stroke="#52525B" stroke-width="2"/><rect x="81" y="14" width="34" height="22" rx="2" fill="#0284C7" stroke="#38BDF8" stroke-width="1.2"/><rect x="86" y="20" width="16" height="4" fill="#E0F2FE"/><rect x="86" y="27" width="24" height="4" fill="#34D399"/><rect x="124" y="9" width="50" height="44" rx="1.5" fill="#27272A"/><path d="M128,16 H170 M128,23 H170 M128,30 H170 M128,37 H170 M128,44 H170" stroke="#52525B" stroke-width="2"/><rect x="82" y="42" width="14" height="6" rx="1" fill="#DC2626"/></svg>"##,
            "24,24,27",
            "9,9,11",
        )),
        _ => None,
    };
    if let Some((svg, fill_rgb, line_rgb)) = spec {
        apply_svg_foreign_artwork(sh, svg, fill_rgb, line_rgb, true);
        true
    } else {
        false
    }
}

/// Apply authentic 1:1 Microsoft Visio multi-color vector artwork and below-icon `TxtPinY` captions
/// for Microsoft 365/Office, Active Directory, Azure, Power Platform, Servers, Network & Rack,
/// and Cloud Architecture. Returns `true` if handled.
pub fn apply_visio_1to1_icon_artwork(sh: &mut VisioShape, category: &str, role: &str) -> bool {
    if try_apply_high_precision_svg_stencil(sh, role) {
        return true;
    }
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
        "IT Generic" => {
            build_it_generic_security_artwork(sh, role);
            true
        }
        "Security Vendors" => {
            build_security_vendors_artwork(sh, role);
            true
        }
        "Multi-Cloud & K8s" => {
            build_multicloud_k8s_artwork(sh, role);
            true
        }
        "Rack Power & Cabling" => {
            build_rack_power_cabling_artwork(sh, role);
            true
        }
        "Storage & Unified Comms" => {
            build_storage_uc_artwork(sh, role);
            true
        }
        "C4 & EIP" => {
            build_c4_and_eip_artwork(sh, role);
            true
        }
        "CMMN, DMN & CORAS" => {
            build_cmmn_dmn_coras_artwork(sh, role);
            true
        }
        "Data Vault & ADAPT" => {
            build_datavault_adapt_artwork(sh, role);
            true
        }
        "SysML & IDEF" => {
            build_sysml_idef_artwork(sh, role);
            true
        }
        "IoT, Logic & GDS UX" => {
            build_iot_phs_gds_artwork(sh, role);
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

// ============================================================================
// Awesome-Visio Extended Built-In Packs (IT Generic, Vendors, Methodologies, IoT & UX)
// ============================================================================

fn build_it_generic_security_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("ovas_") {
        // OpenVAS / Greenbone Vulnerability Management (#48A842 / #166534)
        art.add_rect(w * 0.08, h * 0.08, w * 0.84, h * 0.84, m * 0.14, Some("240,253,244"), Some("22,101,52"), 1.8);
        art.add_poly(
            &[
                (w * 0.50, h * 0.18),
                (w * 0.78, h * 0.28),
                (w * 0.74, h * 0.60),
                (w * 0.50, h * 0.82),
                (w * 0.26, h * 0.60),
                (w * 0.22, h * 0.28),
            ],
            Some("34,197,94"),
            Some("21,128,61"),
            1.6,
        );
        if role.contains("scanner") || role.contains("target") {
            art.add_ellipse(w * 0.50, h * 0.48, m * 0.15, m * 0.15, None, Some("255,255,255"), 2.0);
            art.add_lines(
                &[
                    ((w * 0.50, h * 0.28), (w * 0.50, h * 0.68)),
                    ((w * 0.30, h * 0.48), (w * 0.70, h * 0.48)),
                ],
                "255,255,255",
                1.8,
            );
        } else {
            art.add_lines(
                &[
                    ((w * 0.36, h * 0.48), (w * 0.46, h * 0.58)),
                    ((w * 0.46, h * 0.58), (w * 0.66, h * 0.36)),
                ],
                "255,255,255",
                2.4,
            );
        }
        art.apply_to_shape(sh, "34,197,94", "22,101,52", true);
    } else if role.starts_with("mtm_") {
        // MTMnet Telecom & Carrier Network Icons (Deep Indigo / Cyan)
        art.add_rect(w * 0.08, h * 0.08, w * 0.84, h * 0.84, m * 0.12, Some("238,242,255"), Some("49,46,129"), 1.7);
        if role.contains("cell") || role.contains("sat") {
            art.add_poly(
                &[(w * 0.50, h * 0.26), (w * 0.72, h * 0.80), (w * 0.28, h * 0.80)],
                Some("79,70,229"),
                Some("49,46,129"),
                1.5,
            );
            art.add_ellipse(w * 0.50, h * 0.24, m * 0.08, m * 0.08, Some("14,165,233"), Some("49,46,129"), 1.3);
            art.add_open_arc((w * 0.30, h * 0.28), (w * 0.70, h * 0.28), -m * 0.14, "14,165,233", 1.8);
        } else {
            art.add_ellipse(w * 0.50, h * 0.50, m * 0.26, m * 0.26, Some("79,70,229"), Some("49,46,129"), 1.6);
            art.add_ellipse(w * 0.50, h * 0.50, m * 0.12, m * 0.12, Some("255,255,255"), None, 1.0);
            art.add_lines(
                &[
                    ((w * 0.20, h * 0.50), (w * 0.80, h * 0.50)),
                    ((w * 0.50, h * 0.20), (w * 0.50, h * 0.80)),
                ],
                "199,210,254",
                1.6,
            );
        }
        art.apply_to_shape(sh, "79,70,229", "49,46,129", true);
    } else {
        // ECCEMAN Clean Flat 2D Network Symbols (Slate & Sky Blue)
        art.add_rect(w * 0.08, h * 0.14, w * 0.84, h * 0.72, m * 0.10, Some("14,165,233"), Some("15,23,42"), 1.8);
        art.add_rect(w * 0.16, h * 0.24, w * 0.68, h * 0.24, m * 0.05, Some("248,250,252"), None, 1.0);
        art.add_lines(
            &[
                ((w * 0.22, h * 0.64), (w * 0.78, h * 0.64)),
                ((w * 0.66, h * 0.56), (w * 0.78, h * 0.64)),
                ((w * 0.66, h * 0.72), (w * 0.78, h * 0.64)),
            ],
            "255,255,255",
            2.1,
        );
        art.add_ellipse(w * 0.26, h * 0.36, m * 0.045, m * 0.045, Some("34,197,94"), None, 0.8);
        art.apply_to_shape(sh, "14,165,233", "15,23,42", true);
    }
}

fn build_security_vendors_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    let (primary, dark, badge_ltr) = if role.starts_with("pan_") {
        ("250,88,45", "154,52,18", "P") // Palo Alto Networks Flame Orange
    } else if role.starts_with("fg_") {
        ("238,49,36", "153,27,27", "F") // Fortinet Crimson Red
    } else if role.starts_with("cp_") {
        ("219,39,119", "131,24,67", "X") // Check Point Magenta
    } else if role.starts_with("jnp_") {
        ("2,132,199", "12,74,110", "N") // Juniper Cyan-Slate
    } else {
        ("220,38,38", "127,29,29", "A") // Huawei Enterprise Ruby
    };

    // Appliance chassis / security tile
    art.add_rect(w * 0.07, h * 0.14, w * 0.86, h * 0.72, m * 0.12, Some("248,250,252"), Some(dark), 1.8);
    art.add_rect(w * 0.07, h * 0.14, w * 0.86, h * 0.24, m * 0.08, Some(primary), Some(dark), 1.4);
    art.add_rect(w * 0.15, h * 0.48, w * 0.42, h * 0.24, m * 0.04, Some("30,41,59"), None, 0.9);
    art.add_ellipse(w * 0.72, h * 0.60, m * 0.12, m * 0.12, Some(primary), Some(dark), 1.3);
    draw_badge_letter(&mut art, badge_ltr, w * 0.72, h * 0.60, m * 0.07);
    art.add_ellipse(w * 0.23, h * 0.60, m * 0.035, m * 0.035, Some("34,197,94"), None, 0.8);
    art.add_ellipse(w * 0.34, h * 0.60, m * 0.035, m * 0.035, Some("56,189,248"), None, 0.8);
    art.apply_to_shape(sh, primary, dark, true);
}

fn build_multicloud_k8s_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("k8s_") {
        // CNCF Kubernetes Royal Blue (#326CE5) 7-sided / helm wheel emblem
        art.add_poly(
            &[
                (w * 0.50, h * 0.08),
                (w * 0.85, h * 0.25),
                (w * 0.93, h * 0.62),
                (w * 0.69, h * 0.90),
                (w * 0.31, h * 0.90),
                (w * 0.07, h * 0.62),
                (w * 0.15, h * 0.25),
            ],
            Some("50,108,229"),
            Some("30,58,138"),
            1.8,
        );
        art.add_ellipse(w * 0.50, h * 0.52, m * 0.20, m * 0.20, None, Some("255,255,255"), 2.1);
        art.add_ellipse(w * 0.50, h * 0.52, m * 0.07, m * 0.07, Some("255,255,255"), None, 0.9);
        art.add_lines(
            &[
                ((w * 0.50, h * 0.24), (w * 0.50, h * 0.80)),
                ((w * 0.24, h * 0.42), (w * 0.76, h * 0.62)),
                ((w * 0.76, h * 0.42), (w * 0.24, h * 0.62)),
            ],
            "255,255,255",
            1.8,
        );
        art.apply_to_shape(sh, "50,108,229", "30,58,138", true);
    } else if role.starts_with("aws_") {
        // AWS Architecture Orange/Amber (#FF9900 / #232F3E)
        art.add_rect(w * 0.09, h * 0.09, w * 0.82, h * 0.82, m * 0.14, Some("255,153,0"), Some("35,47,62"), 1.8);
        art.add_poly(
            &[
                (w * 0.50, h * 0.22),
                (w * 0.76, h * 0.36),
                (w * 0.76, h * 0.66),
                (w * 0.50, h * 0.78),
                (w * 0.24, h * 0.66),
                (w * 0.24, h * 0.36),
            ],
            None,
            Some("255,255,255"),
            2.2,
        );
        art.add_lines(
            &[
                ((w * 0.24, h * 0.36), (w * 0.50, h * 0.50)),
                ((w * 0.76, h * 0.36), (w * 0.50, h * 0.50)),
                ((w * 0.50, h * 0.50), (w * 0.50, h * 0.78)),
            ],
            "255,255,255",
            1.9,
        );
        art.apply_to_shape(sh, "255,153,0", "35,47,62", true);
    } else if role.starts_with("gcp_") {
        // Google Cloud Hexagon (#4285F4 + Multi-color accents)
        art.add_poly(
            &[
                (w * 0.26, h * 0.12),
                (w * 0.74, h * 0.12),
                (w * 0.94, h * 0.50),
                (w * 0.74, h * 0.88),
                (w * 0.26, h * 0.88),
                (w * 0.06, h * 0.50),
            ],
            Some("255,255,255"),
            Some("66,133,244"),
            2.0,
        );
        art.add_ellipse(w * 0.50, h * 0.50, m * 0.22, m * 0.22, Some("66,133,244"), None, 1.0);
        art.add_ellipse(w * 0.50, h * 0.50, m * 0.10, m * 0.10, Some("255,255,255"), None, 1.0);
        art.add_rect(w * 0.50, h * 0.44, w * 0.24, h * 0.12, m * 0.03, Some("52,168,83"), None, 0.8);
        art.apply_to_shape(sh, "66,133,244", "26,115,232", true);
    } else if role.starts_with("oci_") {
        // Oracle Cloud Infrastructure Red (#C74634)
        art.add_rect(w * 0.08, h * 0.14, w * 0.84, h * 0.72, m * 0.22, Some("199,70,52"), Some("127,29,29"), 1.8);
        art.add_rect(w * 0.22, h * 0.30, w * 0.56, h * 0.40, m * 0.16, None, Some("255,255,255"), 2.4);
        art.add_ellipse(w * 0.50, h * 0.50, m * 0.07, m * 0.07, Some("254,215,170"), None, 0.8);
        art.apply_to_shape(sh, "199,70,52", "127,29,29", true);
    } else {
        // VMware vSphere / NSX / vSAN / Tanzu Teal-Blue (#0091DA / #1D4ED8)
        art.add_rect(w * 0.09, h * 0.09, w * 0.82, h * 0.82, m * 0.14, Some("240,249,255"), Some("3,105,161"), 1.8);
        art.add_rect(w * 0.20, h * 0.20, w * 0.38, h * 0.38, m * 0.08, None, Some("2,132,199"), 2.4);
        art.add_rect(w * 0.42, h * 0.42, w * 0.38, h * 0.38, m * 0.08, Some("14,165,233"), Some("3,105,161"), 1.8);
        art.apply_to_shape(sh, "14,165,233", "3,105,161", true);
    }
}

fn build_rack_power_cabling_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    let (chassis_rgb, accent_rgb, border_rgb) = if role.starts_with("cpwr_") {
        ("30,41,59", "234,179,8", "15,23,42") // CyberPower Slate + Gold LCD
    } else if role.starts_with("tl_") {
        ("30,41,59", "22,163,74", "15,23,42") // Tripp Lite Slate + Emerald
    } else if role.starts_with("rar_") {
        ("30,58,138", "56,189,248", "23,37,84") // Raritan Dominion/PX Blue
    } else if role.starts_with("bb_") {
        ("15,23,42", "239,68,68", "2,6,23") // BlackBox Obsidian + Red Diamond
    } else if role.starts_with("ht_") || role.starts_with("pnd_") {
        ("226,232,240", "37,99,235", "71,85,105") // HellermannTyton / Panduit Structured Cabling
    } else {
        ("71,85,105", "14,165,233", "30,41,59") // VRT Rack Systems
    };

    // 19-inch rack mounting ears + faceplate
    art.add_rect(w * 0.01, h * 0.08, w * 0.05, h * 0.84, m * 0.03, Some("148,163,184"), Some(border_rgb), 1.1);
    art.add_rect(w * 0.94, h * 0.08, w * 0.05, h * 0.84, m * 0.03, Some("148,163,184"), Some(border_rgb), 1.1);
    art.add_rect(w * 0.06, h * 0.08, w * 0.88, h * 0.84, m * 0.05, Some(chassis_rgb), Some(border_rgb), 1.5);
    // LCD telemetry / port bank + status LEDs
    art.add_rect(w * 0.11, h * 0.24, w * 0.24, h * 0.52, m * 0.04, Some(accent_rgb), None, 0.9);
    for idx in 0..4 {
        let px = w * (0.40 + idx as f64 * 0.12);
        art.add_rect(px, h * 0.26, w * 0.09, h * 0.48, m * 0.03, Some("15,23,42"), Some("203,213,225"), 1.0);
    }
    art.add_ellipse(w * 0.90, h * 0.36, m * 0.06, m * 0.06, Some("34,197,94"), None, 0.8);
    art.apply_to_shape(sh, chassis_rgb, border_rgb, true);
}

fn build_storage_uc_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("syno_") || role.starts_with("qnap_") {
        let accent = if role.starts_with("syno_") { "249,115,22" } else { "14,165,233" };
        art.add_rect(w * 0.08, h * 0.12, w * 0.84, h * 0.76, m * 0.10, Some("30,41,59"), Some("15,23,42"), 1.8);
        for bay in 0..4 {
            let bx = w * (0.14 + bay as f64 * 0.18);
            art.add_rect(bx, h * 0.22, w * 0.14, h * 0.56, m * 0.03, Some("51,65,85"), Some("148,163,184"), 1.1);
            art.add_ellipse(bx + w * 0.07, h * 0.30, m * 0.028, m * 0.028, Some(accent), None, 0.8);
        }
        art.apply_to_shape(sh, "30,41,59", "15,23,42", true);
    } else if role.starts_with("tpl_") {
        // TP-Link Omada Teal (#00B09B)
        art.add_rect(w * 0.08, h * 0.16, w * 0.84, h * 0.68, m * 0.14, Some("240,253,250"), Some("13,148,136"), 1.8);
        art.add_ellipse(w * 0.50, h * 0.50, m * 0.22, m * 0.22, Some("20,184,166"), Some("15,118,110"), 1.5);
        art.add_ellipse(w * 0.50, h * 0.50, m * 0.09, m * 0.09, Some("255,255,255"), None, 0.8);
        art.apply_to_shape(sh, "20,184,166", "15,118,110", true);
    } else {
        // Poly / Polycom & Avaya Unified Communications (Crimson / Slate Audio-Video)
        let accent = if role.starts_with("avaya_") { "220,38,38" } else { "79,70,229" };
        art.add_rect(w * 0.08, h * 0.14, w * 0.84, h * 0.72, m * 0.14, Some("248,250,252"), Some("51,65,85"), 1.7);
        art.add_ellipse(w * 0.38, h * 0.50, m * 0.18, m * 0.18, Some(accent), Some("30,41,59"), 1.4);
        art.add_ellipse(w * 0.38, h * 0.50, m * 0.07, m * 0.07, Some("255,255,255"), None, 0.8);
        art.add_poly(
            &[(w * 0.62, h * 0.42), (w * 0.82, h * 0.30), (w * 0.82, h * 0.70), (w * 0.62, h * 0.58)],
            Some(accent),
            Some("30,41,59"),
            1.3,
        );
        art.apply_to_shape(sh, accent, "30,41,59", true);
    }
}

fn build_c4_and_eip_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("c4_") {
        let (fill, stroke) = match role {
            "c4_person" => ("8,66,123", "5,46,89"),
            "c4_ext_actor" | "c4_ext_sys" => ("153,153,153", "102,102,102"),
            "c4_system" => ("17,104,189", "11,72,132"),
            "c4_comp" => ("133,187,240", "67,141,213"),
            "c4_boundary" => ("248,250,252", "71,85,105"),
            _ => ("67,141,213", "29,78,216"), // C4 Container
        };
        if role == "c4_person" || role == "c4_ext_actor" {
            art.add_ellipse(w * 0.50, h * 0.24, m * 0.17, m * 0.17, Some(fill), Some(stroke), 1.8);
            art.add_rect(w * 0.14, h * 0.42, w * 0.72, h * 0.52, m * 0.16, Some(fill), Some(stroke), 1.8);
            art.add_lines(&[((w * 0.28, h * 0.84), (w * 0.72, h * 0.84))], "255,255,255", 1.3);
            art.apply_to_shape(sh, fill, stroke, false);
            sh.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        } else if role == "c4_boundary" {
            art.add_rect(w * 0.03, h * 0.03, w * 0.94, h * 0.94, m * 0.05, Some(fill), Some(stroke), 1.6);
            art.add_rect(w * 0.06, h * 0.06, w * 0.34, h * 0.14, m * 0.03, Some("226,232,240"), Some(stroke), 1.1);
            art.apply_to_shape(sh, fill, stroke, false);
            sh.sheet.set_formula("LinePattern", "2", Some(Unit::Number));
        } else if role == "c4_db" {
            art.add_rect(w * 0.08, h * 0.18, w * 0.84, h * 0.72, m * 0.06, Some(fill), Some(stroke), 1.8);
            art.add_ellipse(w * 0.50, h * 0.18, w * 0.42, h * 0.12, Some("96,165,250"), Some(stroke), 1.6);
            art.apply_to_shape(sh, fill, stroke, false);
            sh.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        } else {
            art.add_rect(w * 0.04, h * 0.06, w * 0.92, h * 0.88, m * 0.08, Some(fill), Some(stroke), 1.8);
            art.add_rect(w * 0.10, h * 0.13, w * 0.26, h * 0.10, m * 0.03, Some("255,255,255"), None, 0.8);
            art.apply_to_shape(sh, fill, stroke, false);
            if role != "c4_comp" {
                sh.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
            }
        }
    } else {
        // Gregor Hohpe Enterprise Integration Patterns (Soft Gold / Warm Amber + Routing Arrows)
        art.add_rect(w * 0.06, h * 0.12, w * 0.88, h * 0.76, m * 0.09, Some("254,249,195"), Some("161,98,7"), 1.8);
        art.add_lines(
            &[
                ((w * 0.18, h * 0.50), (w * 0.82, h * 0.50)),
                ((w * 0.68, h * 0.36), (w * 0.82, h * 0.50)),
                ((w * 0.68, h * 0.64), (w * 0.82, h * 0.50)),
            ],
            "161,98,7",
            2.1,
        );
        art.add_poly(
            &[(w * 0.42, h * 0.50), (w * 0.50, h * 0.34), (w * 0.58, h * 0.50), (w * 0.50, h * 0.66)],
            Some("245,158,11"),
            Some("146,64,14"),
            1.4,
        );
        art.apply_to_shape(sh, "254,249,195", "161,98,7", true);
    }
}

fn build_cmmn_dmn_coras_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("cmmn_") {
        // OMG CMMN 1.1 (Teal-Slate folder / chamfered stage / criterion sentry)
        if role == "cmmn_case" {
            art.add_rect(w * 0.06, h * 0.06, w * 0.34, h * 0.14, m * 0.04, Some("204,251,241"), Some("15,118,110"), 1.6);
            art.add_rect(w * 0.04, h * 0.18, w * 0.92, h * 0.76, m * 0.05, Some("240,253,250"), Some("15,118,110"), 1.8);
        } else if role == "cmmn_entry" || role == "cmmn_exit" {
            let fill = if role == "cmmn_entry" { "255,255,255" } else { "15,23,42" };
            art.add_poly(
                &[(w * 0.50, h * 0.08), (w * 0.88, h * 0.50), (w * 0.50, h * 0.92), (w * 0.12, h * 0.50)],
                Some(fill),
                Some("15,23,42"),
                2.0,
            );
            art.add_ellipse(w * 0.50, h * 0.50, m * 0.08, m * 0.08, Some("20,184,166"), None, 0.8);
        } else {
            art.add_poly(
                &[
                    (w * 0.12, h * 0.08),
                    (w * 0.88, h * 0.08),
                    (w * 0.96, h * 0.22),
                    (w * 0.96, h * 0.78),
                    (w * 0.88, h * 0.92),
                    (w * 0.12, h * 0.92),
                    (w * 0.04, h * 0.78),
                    (w * 0.04, h * 0.22),
                ],
                Some("240,253,250"),
                Some("15,118,110"),
                1.7,
            );
            art.add_rect(w * 0.44, h * 0.76, w * 0.12, h * 0.12, m * 0.02, None, Some("15,118,110"), 1.3);
        }
        art.apply_to_shape(sh, "240,253,250", "15,118,110", role == "cmmn_entry" || role == "cmmn_exit");
    } else if role.starts_with("dmn_") {
        // OMG DMN 1.3 (Indigo Decision, Clipped BKM, Pill Input Data)
        if role == "dmn_input" {
            art.add_rect(w * 0.06, h * 0.14, w * 0.88, h * 0.72, h * 0.34, Some("238,242,255"), Some("67,56,202"), 1.8);
        } else if role == "dmn_bkm" {
            art.add_poly(
                &[
                    (w * 0.16, h * 0.08),
                    (w * 0.94, h * 0.08),
                    (w * 0.94, h * 0.78),
                    (w * 0.84, h * 0.92),
                    (w * 0.06, h * 0.92),
                    (w * 0.06, h * 0.22),
                ],
                Some("224,231,255"),
                Some("67,56,202"),
                1.8,
            );
        } else {
            art.add_rect(w * 0.05, h * 0.08, w * 0.90, h * 0.84, m * 0.04, Some("238,242,255"), Some("67,56,202"), 1.9);
            art.add_lines(&[((w * 0.10, h * 0.26), (w * 0.32, h * 0.26))], "99,102,241", 1.8);
        }
        art.add_ellipse(w * 0.86, h * 0.22, m * 0.05, m * 0.05, Some("99,102,241"), None, 0.8);
        art.apply_to_shape(sh, "238,242,255", "67,56,202", false);
    } else {
        // CORAS Model-Driven Risk Analysis (Crimson / Amber / Emerald Risk Badges)
        let (fill, stroke) = if role.contains("incident") || role.contains("threat") {
            ("254,226,226", "185,28,28")
        } else if role.contains("vuln") {
            ("254,243,199", "180,83,9")
        } else {
            ("220,252,231", "21,128,61")
        };
        art.add_poly(
            &[(w * 0.50, h * 0.10), (w * 0.90, h * 0.84), (w * 0.10, h * 0.84)],
            Some(fill),
            Some(stroke),
            2.0,
        );
        art.add_lines(&[((w * 0.50, h * 0.36), (w * 0.50, h * 0.62))], stroke, 2.4);
        art.add_ellipse(w * 0.50, h * 0.72, m * 0.045, m * 0.045, Some(stroke), None, 0.8);
        art.apply_to_shape(sh, fill, stroke, true);
    }
}

fn build_datavault_adapt_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("dv_") {
        // Data Vault 2.0 Standard Colors: Hub = Sky Blue, Link = Rose/Coral, Satellite = Gold/Yellow, PIT/Bridge = Mint
        let (hdr, body, border) = if role.contains("hub") {
            ("59,130,246", "219,234,254", "29,78,216")
        } else if role.contains("link") {
            ("239,68,68", "254,226,226", "185,28,28")
        } else if role.contains("sat") {
            ("234,179,8", "254,249,195", "161,98,7")
        } else {
            ("16,185,129", "209,250,229", "4,120,87")
        };
        art.add_rect(w * 0.04, h * 0.06, w * 0.92, h * 0.88, m * 0.08, Some(body), Some(border), 1.8);
        art.add_rect(w * 0.04, h * 0.06, w * 0.92, h * 0.24, m * 0.06, Some(hdr), Some(border), 1.5);
        art.add_lines(&[((w * 0.10, h * 0.52), (w * 0.90, h * 0.52))], border, 1.1);
        art.apply_to_shape(sh, body, border, false);
    } else {
        // ADAPT OLAP & Multidimensional BI (Violet / Slate 3D Hypercube & Dimension Axes)
        art.add_poly(
            &[
                (w * 0.50, h * 0.10),
                (w * 0.88, h * 0.28),
                (w * 0.88, h * 0.72),
                (w * 0.50, h * 0.90),
                (w * 0.12, h * 0.72),
                (w * 0.12, h * 0.28),
            ],
            Some("243,232,255"),
            Some("109,40,217"),
            1.9,
        );
        art.add_lines(
            &[
                ((w * 0.12, h * 0.28), (w * 0.50, h * 0.46)),
                ((w * 0.88, h * 0.28), (w * 0.50, h * 0.46)),
                ((w * 0.50, h * 0.46), (w * 0.50, h * 0.90)),
            ],
            "124,58,237",
            1.7,
        );
        art.apply_to_shape(sh, "243,232,255", "109,40,217", true);
    }
}

fn build_sysml_idef_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("sysml_") {
        let (hdr, body, stroke) = if role.contains("req") {
            ("254,215,170", "255,247,237", "194,65,12")
        } else if role.contains("constraint") {
            ("221,214,254", "245,243,255", "109,40,217")
        } else {
            ("226,232,240", "248,250,252", "51,65,85")
        };
        art.add_rect(w * 0.04, h * 0.06, w * 0.92, h * 0.88, m * 0.05, Some(body), Some(stroke), 1.7);
        art.add_rect(w * 0.04, h * 0.06, w * 0.92, h * 0.24, m * 0.04, Some(hdr), Some(stroke), 1.4);
        art.add_rect(w * 0.01, h * 0.44, w * 0.07, h * 0.14, m * 0.02, Some("56,189,248"), Some(stroke), 1.2);
        art.add_rect(w * 0.92, h * 0.44, w * 0.07, h * 0.14, m * 0.02, Some("56,189,248"), Some(stroke), 1.2);
        art.apply_to_shape(sh, body, stroke, false);
    } else if role.starts_with("idef5_") {
        // IDEF5 Ontology Kind Circle / Individual Dot-Circle
        art.add_ellipse(w * 0.50, h * 0.50, w * 0.42, h * 0.42, Some("240,249,255"), Some("3,105,161"), 1.9);
        if role.contains("indiv") {
            art.add_ellipse(w * 0.50, h * 0.80, m * 0.07, m * 0.07, Some("3,105,161"), None, 0.9);
        }
        art.add_lines(&[((w * 0.22, h * 0.50), (w * 0.78, h * 0.50))], "186,230,253", 1.2);
        art.apply_to_shape(sh, "240,249,255", "3,105,161", false);
    } else {
        // IDEF0 Function Box / IDEF3 UOB & Junction (& / O / X)
        art.add_rect(w * 0.05, h * 0.08, w * 0.90, h * 0.84, m * 0.03, Some("248,250,252"), Some("30,41,59"), 1.8);
        art.add_lines(
            &[
                ((w * 0.05, h * 0.74), (w * 0.95, h * 0.74)),
                ((w * 0.28, h * 0.74), (w * 0.28, h * 0.92)),
            ],
            "71,85,105",
            1.3,
        );
        art.apply_to_shape(sh, "248,250,252", "30,41,59", false);
    }
}

fn build_iot_phs_gds_artwork(sh: &mut VisioShape, role: &str) {
    let mut art = IsoArtwork::for_shape(sh);
    let w = art.vb_w;
    let h = art.vb_h;
    let m = w.min(h);

    if role.starts_with("iot_") {
        // Domotics & Smart Home IoT (Cyan / Emerald Wireless Node)
        art.add_rect(w * 0.10, h * 0.14, w * 0.80, h * 0.76, m * 0.16, Some("236,254,255"), Some("8,145,178"), 1.8);
        art.add_ellipse(w * 0.50, h * 0.58, m * 0.16, m * 0.16, Some("6,182,212"), Some("14,116,144"), 1.5);
        art.add_open_arc((w * 0.32, h * 0.34), (w * 0.68, h * 0.34), -m * 0.12, "8,145,178", 2.0);
        art.add_open_arc((w * 0.22, h * 0.24), (w * 0.78, h * 0.24), -m * 0.16, "14,165,233", 1.8);
        art.apply_to_shape(sh, "236,254,255", "8,145,178", true);
    } else if role.starts_with("phs_") {
        // Paul Herber Digital Logic IC / 66-Block / KNX Bus Coupler
        art.add_rect(w * 0.18, h * 0.12, w * 0.64, h * 0.76, m * 0.05, Some("30,41,59"), Some("15,23,42"), 1.7);
        for pin in 0..4 {
            let py = h * (0.22 + pin as f64 * 0.18);
            art.add_rect(w * 0.06, py, w * 0.12, h * 0.08, m * 0.01, Some("203,213,225"), Some("71,85,105"), 1.0);
            art.add_rect(w * 0.82, py, w * 0.12, h * 0.08, m * 0.01, Some("203,213,225"), Some("71,85,105"), 1.0);
        }
        art.add_ellipse(w * 0.50, h * 0.22, m * 0.06, m * 0.06, Some("245,158,11"), None, 0.8);
        art.apply_to_shape(sh, "30,41,59", "15,23,42", true);
    } else if role.starts_with("gds_") {
        // UK Government Digital Service (GDS) Design System Components
        if role == "gds_start_btn" {
            art.add_rect(w * 0.04, h * 0.10, w * 0.92, h * 0.80, m * 0.06, Some("0,112,60"), Some("0,64,34"), 1.8);
            art.add_lines(
                &[
                    ((w * 0.80, h * 0.34), (w * 0.88, h * 0.50)),
                    ((w * 0.80, h * 0.66), (w * 0.88, h * 0.50)),
                ],
                "255,255,255",
                2.2,
            );
            art.apply_to_shape(sh, "0,112,60", "0,64,34", false);
            sh.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        } else if role == "gds_header" {
            art.add_rect(w * 0.02, h * 0.10, w * 0.96, h * 0.80, m * 0.03, Some("11,12,12"), Some("11,12,12"), 1.6);
            art.add_rect(w * 0.02, h * 0.78, w * 0.96, h * 0.14, 0.0, Some("29,112,184"), None, 0.8);
            art.apply_to_shape(sh, "11,12,12", "29,112,184", false);
            sh.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        } else {
            let accent = if role == "gds_error" { "212,53,28" } else { "29,112,184" };
            art.add_rect(w * 0.03, h * 0.08, w * 0.94, h * 0.84, m * 0.04, Some("255,255,255"), Some("11,12,12"), 1.8);
            art.add_rect(w * 0.03, h * 0.08, w * 0.06, h * 0.84, 0.0, Some(accent), None, 0.8);
            art.apply_to_shape(sh, "255,255,255", "11,12,12", false);
        }
    } else {
        // Paper Plane / Jesse James Garrett Information Architecture (IA Page / Page Stack / Decision)
        if role == "ia_stack" {
            art.add_rect(w * 0.12, h * 0.06, w * 0.82, h * 0.78, m * 0.03, Some("226,232,240"), Some("71,85,105"), 1.4);
            art.add_rect(w * 0.06, h * 0.14, w * 0.82, h * 0.80, m * 0.03, Some("255,255,255"), Some("30,41,59"), 1.7);
        } else if role == "ia_decision" {
            art.add_poly(
                &[(w * 0.50, h * 0.06), (w * 0.94, h * 0.50), (w * 0.50, h * 0.94), (w * 0.06, h * 0.50)],
                Some("241,245,249"),
                Some("30,41,59"),
                1.8,
            );
        } else {
            art.add_rect(w * 0.06, h * 0.08, w * 0.88, h * 0.84, m * 0.03, Some("255,255,255"), Some("30,41,59"), 1.7);
            art.add_rect(w * 0.06, h * 0.08, w * 0.88, h * 0.20, m * 0.02, Some("226,232,240"), Some("30,41,59"), 1.2);
        }
        art.add_ellipse(w * 0.84, h * 0.18, m * 0.04, m * 0.04, Some("59,130,246"), None, 0.8);
        art.apply_to_shape(sh, "255,255,255", "30,41,59", false);
    }
}
