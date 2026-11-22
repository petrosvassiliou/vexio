//! Exhaustive End-to-End Smoke Test for Vexio Engine, Commands, Themes, Connectors,
//! Pixel-Level Multilingual & Rich Text Rendering, and All 15 Save/Export Formats.

use vexio_core::boolean_ops::{BooleanOpKind, apply_boolean_operation};
use vexio_core::themes_styles::apply_document_theme_variant;
use vexio_core::workspace::{DocumentSession, TextStylePatch};
use vexio_core::{
    vexio_render::{
        MeshCache, PRIM_FLAG_TEXT_GLYPH, ViewportCamera, build_page_scene_mesh,
        render_page_software_rgba_with_camera,
    },
    vexio_shapesheet::{ReactiveEngine, RgbaColor},
};

fn rel_luminance(c: RgbaColor) -> f64 {
    fn chan(v: u8) -> f64 {
        let s = v as f64 / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * chan(c.r) + 0.7152 * chan(c.g) + 0.0722 * chan(c.b)
}

fn contrast_ratio(a: RgbaColor, b: RgbaColor) -> f64 {
    let la = rel_luminance(a);
    let lb = rel_luminance(b);
    let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

fn count_non_white_pixels(rgba: &[u8]) -> usize {
    rgba.chunks_exact(4)
        .filter(|px| px[0] < 245 || px[1] < 245 || px[2] < 245)
        .count()
}

fn count_yellow_highlight_pixels(rgba: &[u8]) -> usize {
    rgba.chunks_exact(4)
        .filter(|px| px[0] > 235 && px[1] > 215 && px[2] < 180)
        .count()
}

#[test]
fn smoke_all_starter_templates_layouts_validators_and_boolean_ops() {
    let template_slugs = [
        "blank",
        "flowchart",
        "bpmn",
        "bpmn2",
        "bpmn_fulfillment",
        "network",
        "cloud_microservices",
        "uml",
        "erd",
        "sql_ecommerce_erd",
        "orgchart",
        "executive_org_chart",
        "gantt",
        "release_gantt",
        "timeline",
        "project_timeline",
        "floorplan",
        "cad_floorplan",
        "datacenter_floorplan",
    ];

    let mut sess = DocumentSession::new_blank("Template Smoke");
    for slug in template_slugs {
        sess.load_starter_template(slug);
        assert!(
            !sess.doc.pages.is_empty(),
            "Starter template '{slug}' must create at least 1 page"
        );
        if slug != "blank" {
            assert!(
                !sess.doc.pages[0].shapes.is_empty(),
                "Non-blank starter template '{slug}' must populate shapes"
            );
        }
    }

    // Exercise all auto-layout algorithms + cross-platform aliases
    sess.load_starter_template("bpmn_fulfillment");
    for algo in [
        "tb",
        "lr",
        "hierarchical_tb",
        "hierarchical_lr",
        "radial",
        "circular",
        "force_directed",
        "compact_orthogonal",
    ] {
        sess.layout(1, algo);
        let visuals = ReactiveEngine::evaluate_page_visuals(&sess.doc, 1);
        assert!(!visuals.is_empty(), "Auto-layout '{algo}' preserved shapes");
    }

    // Exercise all structural validators
    for ruleset in ["bpmn", "uml", "workflow"] {
        let _issues = sess.validate(ruleset);
    }

    // Exercise all 8 constructive boolean operations
    for op in [
        BooleanOpKind::Union,
        BooleanOpKind::Combine,
        BooleanOpKind::Fragment,
        BooleanOpKind::Intersect,
        BooleanOpKind::Subtract,
        BooleanOpKind::Trim,
        BooleanOpKind::Join,
        BooleanOpKind::Offset(0.2),
    ] {
        let mut bsess = DocumentSession::new_blank("Boolean Smoke");
        let a = bsess
            .draw_primitive_2d(1, "rect", 2.0, 2.0, 4.5, 4.0)
            .unwrap();
        let b = bsess
            .draw_primitive_2d(1, "ellipse", 3.2, 2.5, 5.8, 4.5)
            .unwrap();
        let ids = if matches!(op, BooleanOpKind::Offset(_)) {
            vec![a]
        } else {
            vec![a, b]
        };
        let created = apply_boolean_operation(&mut bsess.doc, 1, &ids, op);
        assert!(
            !created.is_empty(),
            "Boolean op {op:?} must produce >= 1 resulting shape"
        );
    }
}

#[test]
fn smoke_all_24_theme_variants_guarantee_readable_text_contrast() {
    let themes = [
        "modern",
        "professional",
        "vibrant",
        "blueprint",
        "monochrome",
        "hand_drawn",
    ];

    for theme_slug in themes {
        for variant in 0u8..4u8 {
            let mut sess = DocumentSession::new_blank("Theme Contrast Smoke");
            // 1. Filled 2D shape
            let box_a = sess
                .drop_builtin_shape(1, "Basic Flowchart", "Process", 2.5, 5.5)
                .unwrap();
            sess.set_shape_text(1, box_a, "Process Box").unwrap();
            let box_b = sess
                .drop_builtin_shape(1, "Basic Flowchart", "Decision", 6.5, 5.5)
                .unwrap();

            // 2. 1D Connector with label sitting over page background
            let conn = sess.connect_shapes(1, box_a, box_b).unwrap();
            sess.set_shape_text(1, conn, "Approved Flow").unwrap();

            // 3. Transparent fill-less Text Box (`FillPattern = 0`)
            let txt_box = sess
                .draw_primitive_2d(1, "text", 2.0, 2.0, 4.5, 2.8)
                .unwrap();
            sess.set_shape_text(1, txt_box, "Standalone Caption").unwrap();

            // 4. External below-icon stencil caption (`TxtPinY = -0.18 in`)
            let srv = sess
                .drop_builtin_shape(1, "Servers (Standard Roles)", "Web Server", 8.5, 3.0)
                .unwrap();
            sess.set_cell_formula(1, srv, "TxtPinY", "-0.20 in", true)
                .unwrap();
            sess.set_shape_text(1, srv, "prod-web-01").unwrap();

            let updated = apply_document_theme_variant(&mut sess.doc, theme_slug, variant);
            assert!(updated >= 4);
            ReactiveEngine::recalculate_document(&mut sess.doc);

            let page_bg = sess
                .doc
                .page(1)
                .and_then(|p| p.page_sheet.get_color("PageColor"))
                .unwrap_or(RgbaColor::WHITE);
            let visuals = ReactiveEngine::evaluate_page_visuals(&sess.doc, 1);

            for vis in &visuals {
                if vis.text.trim().is_empty() {
                    continue;
                }
                let bg = if vis.shape_id == box_a || vis.shape_id == box_b {
                    vis.fill_color.unwrap_or(page_bg)
                } else {
                    page_bg
                };
                let ratio = contrast_ratio(vis.text_color, bg);
                assert!(
                    ratio >= 3.0,
                    "Theme {theme_slug} variant {variant} shape {} ('{}') had unreadable contrast {ratio:.2} (text={:?}, bg={:?})",
                    vis.shape_id,
                    vis.text,
                    vis.text_color,
                    bg
                );
            }
        }
    }
}

#[test]
fn smoke_connector_labels_all_four_directions_and_pixel_text_rendering() {
    // Part A: 1D Connector label midpoint placement across all 4 cardinal directions
    let dirs = [
        ("East (+X)", (2.0, 5.0), (7.0, 5.0)),
        ("West (-X)", (7.0, 5.0), (2.0, 5.0)),
        ("North (+Y)", (4.5, 2.0), (4.5, 7.0)),
        ("South (-Y)", (4.5, 7.0), (4.5, 2.0)),
    ];

    for (label, (x1, y1), (x2, y2)) in dirs {
        let mut sess = DocumentSession::new_blank(label);
        let cid = sess.draw_line(1, x1, y1, x2, y2).unwrap();
        sess.set_shape_text(1, cid, label).unwrap();
        ReactiveEngine::recalculate_document(&mut sess.doc);
        let visuals = ReactiveEngine::evaluate_page_visuals(&sess.doc, 1);
        let cvis = visuals.iter().find(|v| v.shape_id == cid).unwrap();
        let expected_mx = (x1 + x2) * 0.5;
        let expected_my = (y1 + y2) * 0.5;
        assert!(
            (cvis.txt_pos[0] - expected_mx).abs() < 0.25
                && (cvis.txt_pos[1] - expected_my).abs() < 0.25,
            "1D connector '{label}' txt_pos {:?} drifted away from midpoint ({expected_mx}, {expected_my})",
            cvis.txt_pos
        );
    }

    // Part B: Pixel-level RGBA glyph & highlight verification across styles, rotations & scripts
    let cases = [
        ("Rich B/I/U/S + Highlight", "Architecture Core\nLine Two", true, 0.0),
        ("Rotated 90 deg", "Vertical Bus Label", false, 90.0),
        ("Rotated 180 deg", "Never Upside Down", false, 180.0),
        ("German Umlauts & Quotes", "Größe ÄÖÜ äöüß „Test“ – —", false, 0.0),
        ("Greek Tonos & Dialytika", "Αρχιτεκτονική ΆΈΉΊΌΎΏ άέήίϊΐόύϋΰώ", false, 0.0),
        ("Arabic & Hebrew Bidi", "مرحبا שלום System", false, 0.0),
        ("CJK Vector Strokes", "架構設計 テスト", false, 0.0),
    ];

    for (case_name, sample_text, with_highlight, angle_deg) in cases {
        let mut sess = DocumentSession::new_blank(case_name);
        let txt_id = sess
            .draw_primitive_2d(1, "text", 2.0, 3.0, 9.0, 5.5)
            .unwrap();
        sess.set_shape_text(1, txt_id, sample_text).unwrap();
        let patch = TextStylePatch {
            font_family: Some("Georgia".into()),
            font_size_pt: Some(22.0),
            bold: Some(true),
            italic: Some(with_highlight),
            underline: Some(with_highlight),
            strikethrough: Some(with_highlight),
            text_color: Some("#0F172A".into()),
            text_bg_color: Some(if with_highlight {
                "#FEF08A".into()
            } else {
                "none".into()
            }),
            horiz_align: Some(1),
            vert_align: Some(1),
            bullet_style: Some(if with_highlight { 1 } else { 0 }),
            ..Default::default()
        };
        sess.apply_text_style_patch(1, &[txt_id], &patch).unwrap();
        if angle_deg != 0.0 {
            sess.rotate_or_flip_shapes(1, &[txt_id], angle_deg, false, false)
                .unwrap();
        }
        ReactiveEngine::recalculate_document(&mut sess.doc);

        let mut cam = ViewportCamera::new(640, 480, 1.0);
        cam.fit_to_page(11.0, 8.5);
        let mut cache = MeshCache::new();
        let (mesh, _, _) =
            build_page_scene_mesh(&sess.doc, 1, &cam, &mut cache, false, false, &[]).unwrap();
        let glyph_verts = mesh
            .vertices
            .iter()
            .filter(|v| (v.flags[0] - PRIM_FLAG_TEXT_GLYPH).abs() < 0.1)
            .count();
        assert!(
            glyph_verts >= 12,
            "Case '{case_name}' produced 0 glyph vertices ({glyph_verts})"
        );

        let rgba = render_page_software_rgba_with_camera(&sess.doc, 1, &cam, false).unwrap();
        let ink_pixels = count_non_white_pixels(&rgba);
        assert!(
            ink_pixels > 80,
            "Case '{case_name}' failed to rasterize visible dark glyph pixels (got {ink_pixels})"
        );
        if with_highlight {
            let yellow_px = count_yellow_highlight_pixels(&rgba);
            assert!(
                yellow_px > 50,
                "Highlight case '{case_name}' failed to rasterize yellow #FEF08A background quad (got {yellow_px})"
            );
        }
    }
}

#[test]
fn smoke_all_15_save_and_export_formats_including_pdf_text_visibility() {
    let mut sess = DocumentSession::new_blank("All 15 Exports");
    let s1 = sess
        .drop_builtin_shape(1, "BPMN 2.0", "Task", 3.0, 5.0)
        .unwrap();
    sess.set_shape_text(1, s1, "Prüfung Größe\nSecond Line").unwrap();
    let s2 = sess
        .drop_builtin_shape(1, "BPMN 2.0", "End Event", 7.5, 5.0)
        .unwrap();
    sess.set_shape_text(1, s2, "Έλεγχος Ολοκλήρωσης").unwrap();
    let conn = sess.connect_shapes(1, s1, s2).unwrap();
    sess.set_shape_text(1, conn, "OK -> Next").unwrap();

    let patch = TextStylePatch {
        font_family: Some("Inter".into()),
        font_size_pt: Some(14.0),
        bold: Some(true),
        underline: Some(true),
        text_color: Some("#0F172A".into()),
        text_bg_color: Some("#FEF08A".into()),
        bullet_style: Some(1),
        ..Default::default()
    };
    sess.apply_text_style_patch(1, &[s1], &patch).unwrap();

    let formats: [(&str, &[u8]); 15] = [
        ("vsdx", b"PK\x03\x04"),
        ("vssx", b"PK\x03\x04"),
        ("vsdm", b"PK\x03\x04"),
        ("vsd", b"\xD0\xCF\x11\xE0"),
        ("vdx", b"<?xml"),
        ("dwg", b"AC10"),
        ("dxf", b"0\nSECTION"),
        ("pdf", b"%PDF-"),
        ("svg", b"<svg"),
        ("png", b"\x89PNG\r\n\x1a\n"),
        ("jpg", b"\xFF\xD8\xFF"),
        ("pptx", b"PK\x03\x04"),
        ("docx", b"PK\x03\x04"),
        ("powerautomate", b"{"),
        ("powerbi", b"{"),
    ];

    for (ext, magic) in formats {
        let bytes = sess
            .save_format(ext)
            .unwrap_or_else(|e| panic!("Export format '{ext}' failed: {e}"));
        assert!(
            bytes.len() > magic.len(),
            "Export '{ext}' produced empty payload"
        );
        assert!(
            bytes.starts_with(magic),
            "Export '{ext}' did not start with expected magic {magic:?}"
        );

        // Round-trip open check for readable formats
        if matches!(ext, "vsdx" | "vssx" | "vsdm" | "vsd" | "vdx" | "dwg" | "dxf") {
            let mut rt = DocumentSession::new_blank("RoundTrip");
            rt.open_bytes(&bytes, ext)
                .unwrap_or_else(|e| panic!("Round-trip open of '{ext}' failed: {e}"));
            assert!(!rt.doc.pages.is_empty());
        }
    }

    // Deep SVG verification: multi-line, bold, underline, bullet, highlight rect
    let svg_str = String::from_utf8(sess.save_format("svg").unwrap()).unwrap();
    assert!(svg_str.contains("Prüfung Größe"));
    assert!(svg_str.contains("Second Line"));
    assert!(svg_str.contains("Έλεγχος Ολοκλήρωσης"));
    assert!(svg_str.contains("#FEF08A") || svg_str.contains("#fef08a"));

    // Deep PDF verification: WinAnsiEncoding, explicit dark text `rg` before `BT`, both lines emitted, and vector triangles for Greek!
    let pdf_bytes = sess.save_format("pdf").unwrap();
    let pdf_lossy = String::from_utf8_lossy(&pdf_bytes);
    assert!(
        pdf_lossy.contains("/WinAnsiEncoding"),
        "PDF must declare /WinAnsiEncoding so European umlauts render accurately"
    );
    assert!(
        pdf_lossy.contains("Second Line)"),
        "PDF must emit multi-line text instead of truncating after line 1"
    );
    assert!(
        pdf_lossy.contains("0.059 0.090 0.165 rg\nBT"),
        "PDF must explicitly set vis.text_color (#0F172A -> 0.059 0.090 0.165 rg) immediately before BT so text is never painted in shape fill color!"
    );
    assert!(
        pdf_lossy.contains(" l h f\n"),
        "PDF must emit filled vector glyph triangles for non-WinAnsi Greek text!"
    );
}
