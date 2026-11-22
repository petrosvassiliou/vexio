//! End-to-end integration tests for Vexio `v0.2.0-alpha`:
//! 1. TrueType / OpenType proportional shaping, kerning, and cubic Bezier PDF contour export.
//! 2. Complex `.vsdx` visual fidelity (`QuickStyle*Matrix`, `LUMMOD`/`LUMOFF`/`SATMOD`,
//!    multi-stop `<Section N="FillGradient">` round-trip, arrowheads `1..=29` & sizes `0..=6`,
//!    and GDI EMF + EMF+ + 16-bit WMF vector playback).
//! 3. Interactive canvas ergonomics (Smart Guides + 3-shape Equal-Spacing guides, Align 6 modes,
//!    Distribute H/V equal bounding-box gaps, Group/Ungroup + Undo/Redo, rotated static port glue,
//!    and orthogonal connector mid-segment dragging).

use vexio_core::export_print::{export_document_pdf, export_page_svg};
use vexio_core::workspace::DocumentSession;
use vexio_render::foreign_media::tessellate_foreign_media;
use vexio_render::{
    measure_text_width_in, shape_line_to_pdf_contours, tessellate_shape, PdfContourOp,
};
use vexio_shapesheet::{
    ConnectionPointRow, ForeignMediaData, GradientStopRow, ReactiveEngine, ShapeKind,
    ShapeTransform2D, ThemeDef, Unit, VisioDocument, VisioShape,
};
use vexio_vsdx::{VsdxPackageKind, read_vsdx, write_vsdx};

#[test]
fn truetype_proportional_metrics_kerning_and_pdf_bezier_contours_work() {
    // 1) Bundled Sans ("Inter" -> DejaVuSans) must measure narrow "iiiii" much narrower than wide "MMMMM",
    //    whereas Monospace ("JetBrains Mono" -> DejaVuSansMono) must measure equal character widths!
    let narrow_sans = measure_text_width_in("iiiii", "Inter", 18.0, false, false);
    let wide_sans = measure_text_width_in("MMMMM", "Inter", 18.0, false, false);
    assert!(
        narrow_sans > 0.05 && wide_sans > narrow_sans * 2.2,
        "TrueType proportional sans must measure MMMMM ({wide_sans:.4}) >> iiiii ({narrow_sans:.4})"
    );

    let narrow_mono = measure_text_width_in("iiiii", "JetBrains Mono", 18.0, false, false);
    let wide_mono = measure_text_width_in("MMMMM", "JetBrains Mono", 18.0, false, false);
    assert!(
        (narrow_mono - wide_mono).abs() < 1e-4,
        "TrueType monospace must have identical advance for iiiii ({narrow_mono:.4}) and MMMMM ({wide_mono:.4})"
    );

    // 2) Pair kerning ("AV" <= "A" + "V")
    let w_av = measure_text_width_in("AV", "Inter", 24.0, false, false);
    let w_a = measure_text_width_in("A", "Inter", 24.0, false, false);
    let w_v = measure_text_width_in("V", "Inter", 24.0, false, false);
    assert!(
        w_av <= w_a + w_v + 1e-6,
        "kerned AV ({w_av:.5}) must be <= A+V ({:.5})",
        w_a + w_v
    );

    // 3) Cubic Bezier PDF glyph contour extraction for Latin + Greek + accented Unicode
    let (ops, _) = shape_line_to_pdf_contours(
        "Größe • Έλεγχος",
        "Inter",
        14.0,
        true,
        false,
        1,
        3.2,
        0.08,
        0.0,
        4.0,
        5.0,
        0.0,
    );
    let cubic_count = ops
        .iter()
        .filter(|op| matches!(op, PdfContourOp::CubicTo(..)))
        .count();
    assert!(
        cubic_count >= 25,
        "TrueType outline shaper must emit cubic Bezier curves for curved glyphs, got {cubic_count}"
    );

    // 4) Full PDF export with `User.PdfOutlineText = TRUE` and Unicode text emits `c` Bezier operators
    let mut doc = VisioDocument::new_blank("PDF Contour Spec");
    let mut sh = VisioShape::new_rect(1, "TitleCard", 4.0, 5.0, 3.2, 1.2);
    sh.text = "Architecture • Έλεγχος".into();
    sh.sheet
        .set_formula("User.PdfOutlineText", "TRUE", Some(Unit::Boolean));
    doc.page_mut(1).unwrap().shapes.push(sh);
    ReactiveEngine::recalculate_document(&mut doc);

    let pdf_bytes = export_document_pdf(&doc);
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf_bytes.starts_with(b"%PDF-1."));
    assert!(
        pdf_str.contains(" c ") && (pdf_str.contains("h f\n") || pdf_str.contains("h B\n")),
        "PDF export with Unicode / PdfOutlineText must emit TrueType cubic Bezier (` c `) contour operators"
    );
}

#[test]
fn vsdx_quickstyle_color_mods_multistop_gradients_arrows_and_emf_wmf_fidelity() {
    let mut doc = VisioDocument::new_blank("Visual Fidelity v0.2");
    doc.themes.insert(1, ThemeDef::default());
    doc.active_theme_id = Some(1);

    // Shape 1: QuickStyleFillMatrix + LUMMOD/SATMOD color formula + Multi-stop FillGradient
    let mut grad_shape = VisioShape::new_rect(1, "ThemeGradCard", 3.0, 5.0, 2.6, 1.6);
    grad_shape
        .sheet
        .set_formula("QuickStyleFillColor", "4", Some(Unit::Number));
    grad_shape
        .sheet
        .set_formula("QuickStyleFillMatrix", "102", Some(Unit::Number));
    grad_shape.sheet.set_formula(
        "LineColor",
        "LUMMOD(SATMOD(RGB(37, 99, 235), 110%), 75%)",
        Some(Unit::Color),
    );
    grad_shape
        .sheet
        .set_formula("FillPattern", "25", Some(Unit::Number));
    grad_shape
        .sheet
        .set_formula("FillGradientAngle", "45 deg", Some(Unit::Degree));
    grad_shape.sheet.fill_gradient_stops = vec![
        GradientStopRow {
            ix: 0,
            position: "0%".into(),
            color: "RGB(239, 246, 255)".into(),
            color_trans: "0%".into(),
        },
        GradientStopRow {
            ix: 1,
            position: "50%".into(),
            color: "RGB(147, 197, 253)".into(),
            color_trans: "0%".into(),
        },
        GradientStopRow {
            ix: 2,
            position: "100%".into(),
            color: "RGB(29, 78, 216)".into(),
            color_trans: "0%".into(),
        },
    ];
    doc.page_mut(1).unwrap().shapes.push(grad_shape);

    // Shape 2: Connector exercising extended Visio arrowhead catalog (`1..=29`) and `EndArrowSize = 6` (Colossal)
    let mut conn = VisioShape::new_connector(2, "CrowFootLink", 1.5, 2.5, 5.5, 2.5);
    conn.sheet
        .set_formula("BeginArrow", "24", Some(Unit::Number));
    conn.sheet.set_formula("EndArrow", "29", Some(Unit::Number));
    conn.sheet
        .set_formula("BeginArrowSize", "1", Some(Unit::Number));
    conn.sheet
        .set_formula("EndArrowSize", "6", Some(Unit::Number));
    doc.page_mut(1).unwrap().shapes.push(conn);

    ReactiveEngine::recalculate_document(&mut doc);

    // Round-trip through lossless `.vsdx` writer + reader and verify multi-stop `<Section N="FillGradient">` survives!
    let vsdx_bytes = write_vsdx(&doc, VsdxPackageKind::Drawing).expect("write_vsdx");
    let (mut reloaded, _) = read_vsdx(&vsdx_bytes).expect("read_vsdx");
    ReactiveEngine::recalculate_document(&mut reloaded);

    let reloaded_sh = reloaded.page(1).unwrap().find_shape(1).unwrap();
    assert_eq!(reloaded_sh.sheet.fill_gradient_stops.len(), 3);

    let visuals = ReactiveEngine::evaluate_page_visuals(&reloaded, 1);
    let vis_grad = visuals.iter().find(|v| v.shape_id == 1).unwrap();
    assert_eq!(vis_grad.gradient_stops.len(), 3);
    let grad_mesh = tessellate_shape(vis_grad, 96.0);
    assert!(
        grad_mesh.vertices.len() >= 16,
        "multi-stop gradient must subdivide into smooth color-interpolated strips"
    );

    // Test all 29 Visio arrowhead styles produce valid non-NaN triangles
    let base_conn_vis = visuals.iter().find(|v| v.shape_id == 2).unwrap().clone();
    for arrow_code in 1..=29 {
        let mut v = base_conn_vis.clone();
        v.end_arrow = arrow_code;
        v.begin_arrow = arrow_code;
        let m = tessellate_shape(&v, 96.0);
        assert!(
            !m.is_empty(),
            "arrowhead code {arrow_code} must tessellate cleanly"
        );
        for vert in &m.vertices {
            assert!(vert.pos[0].is_finite() && vert.pos[1].is_finite());
        }
    }

    // SVG export must emit `<linearGradient>` with 3 `<stop>` elements and `marker-start` / `marker-end`
    let svg = export_page_svg(&reloaded, 1);
    assert!(svg.contains("<linearGradient"));
    assert!(svg.matches("<stop offset=").count() >= 3);
    assert!(svg.contains("marker-start=") && svg.contains("marker-end="));

    // Verify GDI EMF, Dual-Mode EMF+, and 16-bit Aldus Placeable WMF vector playback!
    let tf = ShapeTransform2D {
        pin_x: 2.0,
        pin_y: 2.0,
        width: 2.0,
        height: 1.5,
        loc_pin_x: 1.0,
        loc_pin_y: 0.75,
        angle_rad: 0.0,
        flip_x: false,
        flip_y: false,
    };

    // 1) GDI EMF (`EMR_HEADER` + `EMR_CREATEBRUSHINDIRECT` + `EMR_SELECTOBJECT` + `EMR_RECTANGLE` + `EMR_EOF`)
    let mut emf = vec![0u8; 88];
    emf[0..4].copy_from_slice(&1u32.to_le_bytes()); // EMR_HEADER
    emf[4..8].copy_from_slice(&88u32.to_le_bytes());
    emf[8..12].copy_from_slice(&0i32.to_le_bytes());
    emf[12..16].copy_from_slice(&0i32.to_le_bytes());
    emf[16..20].copy_from_slice(&200i32.to_le_bytes());
    emf[20..24].copy_from_slice(&150i32.to_le_bytes());
    emf[40..44].copy_from_slice(b" EMF");
    emf.extend_from_slice(&39u32.to_le_bytes());
    emf.extend_from_slice(&24u32.to_le_bytes());
    emf.extend_from_slice(&1u32.to_le_bytes());
    emf.extend_from_slice(&0u32.to_le_bytes());
    emf.extend_from_slice(&[37, 99, 235, 0]);
    emf.extend_from_slice(&0u32.to_le_bytes());
    emf.extend_from_slice(&37u32.to_le_bytes());
    emf.extend_from_slice(&12u32.to_le_bytes());
    emf.extend_from_slice(&1u32.to_le_bytes());
    emf.extend_from_slice(&43u32.to_le_bytes());
    emf.extend_from_slice(&24u32.to_le_bytes());
    emf.extend_from_slice(&10i32.to_le_bytes());
    emf.extend_from_slice(&10i32.to_le_bytes());
    emf.extend_from_slice(&180i32.to_le_bytes());
    emf.extend_from_slice(&140i32.to_le_bytes());
    emf.extend_from_slice(&14u32.to_le_bytes());
    emf.extend_from_slice(&20u32.to_le_bytes());
    emf.extend_from_slice(&[0u8; 12]);

    let emf_mesh = tessellate_foreign_media(
        &ForeignMediaData {
            media_type: "EMF".into(),
            rel_id: None,
            data: emf.clone(),
        },
        &tf,
        0.005,
    );
    assert!(
        !emf_mesh.is_empty(),
        "GDI EMF vector record stream must tessellate into triangles"
    );

    // 2) Dual-Mode EMF+ (`EMR_COMMENT` `"EMF+"` containing `EmfPlusFillRects` with inline ARGB + compressed i16 rect)
    let mut emf_plus = vec![0u8; 88];
    emf_plus.copy_from_slice(&emf[0..88]);
    let mut plus_payload = Vec::new();
    plus_payload.extend_from_slice(b"EMF+");
    plus_payload.extend_from_slice(&0x400Au16.to_le_bytes());
    plus_payload.extend_from_slice(&0xC000u16.to_le_bytes());
    plus_payload.extend_from_slice(&28u32.to_le_bytes());
    plus_payload.extend_from_slice(&16u32.to_le_bytes());
    plus_payload.extend_from_slice(&0xFF10B981u32.to_le_bytes());
    plus_payload.extend_from_slice(&1u32.to_le_bytes());
    plus_payload.extend_from_slice(&20i16.to_le_bytes());
    plus_payload.extend_from_slice(&20i16.to_le_bytes());
    plus_payload.extend_from_slice(&120i16.to_le_bytes());
    plus_payload.extend_from_slice(&90i16.to_le_bytes());
    let comment_size = (16 + plus_payload.len()) as u32;
    emf_plus.extend_from_slice(&70u32.to_le_bytes());
    emf_plus.extend_from_slice(&comment_size.to_le_bytes());
    emf_plus.extend_from_slice(&(plus_payload.len() as u32).to_le_bytes());
    emf_plus.extend_from_slice(&plus_payload);
    emf_plus.extend_from_slice(&14u32.to_le_bytes());
    emf_plus.extend_from_slice(&20u32.to_le_bytes());
    emf_plus.extend_from_slice(&[0u8; 12]);

    let emf_plus_mesh = tessellate_foreign_media(
        &ForeignMediaData {
            media_type: "EMF".into(),
            rel_id: None,
            data: emf_plus,
        },
        &tf,
        0.005,
    );
    assert!(
        !emf_plus_mesh.is_empty(),
        "Dual-mode EMF+ EmfPlusFillRects record must tessellate into triangles"
    );

    // 3) 16-bit Aldus Placeable WMF (`0x9AC6CDD7` + `META_HEADER` + `META_RECTANGLE` + `META_EOF`)
    let mut wmf = Vec::new();
    wmf.extend_from_slice(&[0xD7, 0xCD, 0xC6, 0x9A]);
    wmf.extend_from_slice(&0u16.to_le_bytes());
    wmf.extend_from_slice(&0i16.to_le_bytes());
    wmf.extend_from_slice(&0i16.to_le_bytes());
    wmf.extend_from_slice(&200i16.to_le_bytes());
    wmf.extend_from_slice(&150i16.to_le_bytes());
    wmf.extend_from_slice(&96u16.to_le_bytes());
    wmf.extend_from_slice(&0u32.to_le_bytes());
    wmf.extend_from_slice(&0u16.to_le_bytes());
    wmf.extend_from_slice(&1u16.to_le_bytes());
    wmf.extend_from_slice(&9u16.to_le_bytes());
    wmf.extend_from_slice(&0x0300u16.to_le_bytes());
    wmf.extend_from_slice(&30u32.to_le_bytes());
    wmf.extend_from_slice(&2u16.to_le_bytes());
    wmf.extend_from_slice(&7u32.to_le_bytes());
    wmf.extend_from_slice(&0u16.to_le_bytes());
    wmf.extend_from_slice(&7u32.to_le_bytes());
    wmf.extend_from_slice(&0x02FCu16.to_le_bytes());
    wmf.extend_from_slice(&0u16.to_le_bytes());
    wmf.extend_from_slice(&[37, 99, 235, 0]);
    wmf.extend_from_slice(&0u16.to_le_bytes());
    wmf.extend_from_slice(&4u32.to_le_bytes());
    wmf.extend_from_slice(&0x012Du16.to_le_bytes());
    wmf.extend_from_slice(&0u16.to_le_bytes());
    wmf.extend_from_slice(&7u32.to_le_bytes());
    wmf.extend_from_slice(&0x041Bu16.to_le_bytes());
    wmf.extend_from_slice(&130i16.to_le_bytes());
    wmf.extend_from_slice(&180i16.to_le_bytes());
    wmf.extend_from_slice(&15i16.to_le_bytes());
    wmf.extend_from_slice(&15i16.to_le_bytes());
    wmf.extend_from_slice(&3u32.to_le_bytes());
    wmf.extend_from_slice(&0u16.to_le_bytes());

    let wmf_mesh = tessellate_foreign_media(
        &ForeignMediaData {
            media_type: "WMF".into(),
            rel_id: None,
            data: wmf,
        },
        &tf,
        0.005,
    );
    assert!(
        !wmf_mesh.is_empty(),
        "16-bit Placeable WMF record stream must tessellate into triangles"
    );
}

#[test]
fn smart_guides_equal_spacing_align_distribute_group_ungroup_and_connector_ergonomics() {
    let mut session = DocumentSession::new_blank("Ergonomics v0.2");

    // Create 3 shapes of different widths to test Equal-Spacing Smart Guides, Align, Distribute, and Group/Ungroup
    let s1 = session
        .draw_primitive_2d(1, "rect", 1.0, 4.0, 2.0, 5.0)
        .unwrap(); // width 1.0, left=1.0, right=2.0, center=(1.5, 4.5)
    let s2 = session
        .draw_primitive_2d(1, "rect", 3.0, 4.0, 5.0, 5.0)
        .unwrap(); // width 2.0, left=3.0, right=5.0, center=(4.0, 4.5) => gap(s1..s2) = 1.0 in!
    let s3 = session
        .draw_primitive_2d(1, "rect", 6.06, 4.04, 7.56, 5.04)
        .unwrap(); // width 1.5, left=6.06 (0.06 in off from equal gap left=6.0!)

    // 1) Smart Guides + Equal-Spacing Snap: dragging s3 near [6.06..7.56] must snap left edge to 6.00 (dx = -0.06) and center-Y to 4.50 (dy = -0.04)!
    let (snap_delta, guides) =
        session.compute_smart_guides(1, &[s3], [6.06, 4.04], [7.56, 5.04], 0.10);
    assert!(
        (snap_delta[0] + 0.06).abs() < 1e-4,
        "expected equal-spacing X snap dx = -0.06, got {}",
        snap_delta[0]
    );
    assert!(
        (snap_delta[1] + 0.04).abs() < 1e-4,
        "expected Y alignment snap dy = -0.04, got {}",
        snap_delta[1]
    );
    assert!(
        guides.iter().any(|g| g.is_equal_spacing),
        "expected at least one equal-spacing guide across s1, s2, s3"
    );

    // 2) Distribute Horizontally with unequal widths (`1.0`, `2.0`, `1.5`) across span `[1.0 .. 10.0]` (`total_span = 9.0`, `sum_widths = 4.5` -> `2 gaps of 2.25 in`!)
    session.transform_shape(1, s3, 9.25, 6.2, 1.5, 1.0, 0.0).unwrap();
    assert!(
        session
            .distribute_shapes(1, &[s1, s2, s3], "horizontal")
            .unwrap()
            >= 1
    );
    {
        let p = session.doc.page(1).unwrap();
        let x1 = p.find_shape(s1).unwrap().sheet.get_f64("PinX").unwrap();
        let x2 = p.find_shape(s2).unwrap().sheet.get_f64("PinX").unwrap();
        let x3 = p.find_shape(s3).unwrap().sheet.get_f64("PinX").unwrap();
        let gap12 = (x2 - 1.0) - (x1 + 0.5);
        let gap23 = (x3 - 0.75) - (x2 + 1.0);
        assert!(
            (gap12 - 2.25).abs() < 1e-3 && (gap23 - 2.25).abs() < 1e-3,
            "expected equal 2.25 in bounding-box gaps, got gap12={gap12:.4}, gap23={gap23:.4}"
        );
    }

    // 3) Align Top across all 3 shapes
    assert!(session.align_shapes(1, &[s1, s2, s3], "top").unwrap() >= 1);
    {
        let p = session.doc.page(1).unwrap();
        let y1 = p.find_shape(s1).unwrap().sheet.get_f64("PinY").unwrap();
        let y2 = p.find_shape(s2).unwrap().sheet.get_f64("PinY").unwrap();
        let y3 = p.find_shape(s3).unwrap().sheet.get_f64("PinY").unwrap();
        assert!((y1 - y2).abs() < 1e-4 && (y2 - y3).abs() < 1e-4);
    }

    // 4) Group -> Move Group -> Ungroup -> Undo/Redo preserves exact world coordinates!
    let orig_x1 = session
        .doc
        .page(1)
        .unwrap()
        .find_shape(s1)
        .unwrap()
        .sheet
        .get_f64("PinX")
        .unwrap();
    let gid = session.group_shapes(1, &[s1, s2]).unwrap();
    assert_eq!(
        session.doc.page(1).unwrap().find_shape(gid).unwrap().kind,
        ShapeKind::Group
    );
    session.move_shapes_delta(1, &[gid], 0.5, -0.25).unwrap();
    let restored = session.ungroup_shapes(1, &[gid]).unwrap();
    assert_eq!(restored.len(), 2);
    let moved_s1_x = session
        .doc
        .page(1)
        .unwrap()
        .find_shape(restored[0])
        .unwrap()
        .sheet
        .get_f64("PinX")
        .unwrap();
    assert!(
        (moved_s1_x - (orig_x1 + 0.5)).abs() < 1e-3,
        "member shape world PinX must include group translation after ungroup"
    );
    assert!(session.journal.undo(&mut session.doc).unwrap());
    assert!(session.doc.page(1).unwrap().find_shape(gid).is_some());
    assert!(session.journal.redo(&mut session.doc).unwrap());
    assert!(session.doc.page(1).unwrap().find_shape(gid).is_none());

    // 5) Static Rotated Port Glue (`Connections.X1`) + Orthogonal Mid-Segment Dragging
    let rot_target = session
        .draw_primitive_2d(1, "rect", 2.0, 1.0, 4.0, 2.0)
        .unwrap();
    {
        let sh = session
            .doc
            .page_mut(1)
            .unwrap()
            .find_shape_mut(rot_target)
            .unwrap();
        sh.sheet.add_connection_point(ConnectionPointRow {
            name: "Row_1".into(),
            ix: 1,
            x: "Width*1".into(),
            y: "Height*0.5".into(),
            dir_x: "1".into(),
            dir_y: "0".into(),
            point_type: 0,
        });
    }
    // Rotate target 90 degrees CCW around `(3.0, 1.5)` -> local `(2.0, 0.5)` relative to `(1.0, 0.5)` rotates `(1.0, 0.0)` to `(0.0, 1.0)` => world `(3.0, 2.5)`!
    session
        .transform_shape(1, rot_target, 3.0, 1.5, 2.0, 1.0, 90.0)
        .unwrap();

    let src = session
        .draw_primitive_2d(1, "rect", 7.0, 1.0, 8.5, 2.0)
        .unwrap();
    let conn_id = session.connect_shapes(1, src, rot_target).unwrap();
    session
        .move_connector_endpoint_with_port(
            1,
            conn_id,
            false,
            3.0,
            2.5,
            Some(rot_target),
            Some("Connections.X1"),
        )
        .unwrap();

    let end_x = session
        .doc
        .page(1)
        .unwrap()
        .find_shape(conn_id)
        .unwrap()
        .sheet
        .get_f64("EndX")
        .unwrap();
    let end_y = session
        .doc
        .page(1)
        .unwrap()
        .find_shape(conn_id)
        .unwrap()
        .sheet
        .get_f64("EndY")
        .unwrap();
    assert!(
        (end_x - 3.0).abs() < 1e-2 && (end_y - 2.5).abs() < 1e-2,
        "rotated static port Connections.X1 must resolve to (3.0, 2.5), got ({end_x:.3}, {end_y:.3})"
    );

    // Drag interior orthogonal segment of `conn_id` to custom X = 5.25 in
    assert!(session.drag_connector_segment(1, conn_id, 1, 5.25).unwrap());
    let mid_coord = session
        .doc
        .page(1)
        .unwrap()
        .find_shape(conn_id)
        .unwrap()
        .sheet
        .get_f64("User.MidCoord")
        .unwrap();
    assert!((mid_coord - 5.25).abs() < 1e-3);
}
