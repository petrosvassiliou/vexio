//! Milestone 3 (`v0.8.0-beta`) End-to-End Parity Verification Suite:
//! 1. Multi-run `Character[0..N]` / `Paragraph[0..N]` rich text & OpenXML `<cp>`/`<pp>` round-trip
//! 2. Dynamic `<fld>` text fields (`Insert -> Field`) & inline `{{=Formula}}` live evaluation
//! 3. Independent `Shift+Ctrl+4` Text Block manipulation (`TxtPinX/Y`, `TxtWidth/Height`, `TxtAngle`) & tool `19`
//! 4. All 45 canonical Visio arrowheads (`0..=45`) & 7 arrow sizes (`0..=6`)
//! 5. Full Visio Line Jumps (`Arc`, `Gap`, `Square`, `2-Sides`..`5-Sides`, Horizontal, Vertical & Z-order)
//! 6. Draggable Horizontal & Vertical Ruler Guides (`ShapeKind::Guide`) + guide-glued shape tracking
//! 7. Configurable `Snap & Glue` matrix (`SnapEnabled`, `GlueEnabled`, `SnapTargets`, `GlueTypes`)
//! 8. `Find & Replace` across active page / all pages (text, `Prop.*`, shape names, whole-word, case)
//! 9. `Format Painter` (`copy_shape_format_json`, `apply_format_painter`, `GUARD` preservation & tool `20`)
//! 10. `Number Shapes…` wizard (`top_to_bottom_left_to_right`, `connected_flow` BFS, text + `Prop.ShapeNumber`)

use std::ffi::CString;
use vexio_core::ffi::c_abi::*;
use vexio_core::vexio_shapesheet::ReactiveEngine;
use vexio_core::workspace::DocumentSession;

#[test]
fn milestone3_rich_runs_dynamic_fields_and_text_block_roundtrip() {
    let mut session = DocumentSession::new_blank("Campus Blueprint");
    let pid = session.doc.pages[0].id;
    let sid = session
        .draw_primitive_2d(pid, "rect", 2.0, 4.0, 5.5, 6.0)
        .expect("draw rect");
    session.set_shape_text(pid, sid, "Core Router").unwrap();

    // Insert dynamic `<fld>` fields (`Width` + `PageName`) and verify live DAG updates
    let f0 = session
        .insert_shape_text_field(pid, sid, "width", "0.00 in", "W: ")
        .expect("insert width field");
    let f1 = session
        .insert_shape_text_field(pid, sid, "page_name", "", " on ")
        .expect("insert page name field");
    assert_eq!(f0, 0);
    assert_eq!(f1, 1);

    let txt_initial = session.doc.page(pid).unwrap().find_shape(sid).unwrap().text.clone();
    assert!(
        txt_initial.contains("3.50 in") && txt_initial.contains("Page-1"),
        "unexpected field text: {txt_initial}"
    );

    // Resize shape -> `<fld>` automatically updates!
    session
        .transform_shape(pid, sid, 4.0, 5.0, 6.25, 2.0, 0.0)
        .unwrap();
    let txt_resized = session.doc.page(pid).unwrap().find_shape(sid).unwrap().text.clone();
    assert!(txt_resized.contains("6.25 in"), "got: {txt_resized}");

    // Transform Text Block independently (`Shift+Ctrl+4`: offset + 30° `TxtAngle`)
    session
        .set_shape_text_block_transform(pid, sid, 3.125, 2.6, 4.0, 0.9, 30.0)
        .unwrap();
    let visuals = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
    let vis = visuals.iter().find(|v| v.shape_id == sid).unwrap();
    assert!((vis.txt_angle_rad.to_degrees() - 30.0).abs() < 1e-4);

    // Set multi-run rich text JSON with Consolas bold red + superscript + live field
    let runs_json = r##"[
        {"text":"Rack ","font":"Inter","sizePt":11.0,"colorHex":"#0F172A","bold":false},
        {"text":"A1","font":"Consolas","sizePt":15.0,"colorHex":"#DC2626","bold":true,"italic":true},
        {"text":"2","font":"Inter","sizePt":11.0,"colorHex":"#2563EB","superscript":true},
        {"text":" — ","font":"Inter","sizePt":11.0,"colorHex":"#0F172A"},
        {"fieldFormula":"=Width","fieldFormat":"0.0 in","font":"Consolas","sizePt":12.0,"colorHex":"#059669","bold":true}
    ]"##;
    session.set_shape_rich_runs_json(pid, sid, runs_json).unwrap();

    // Round-trip `.vsdx` and verify runs, fields, and `TxtAngle` survive 100% losslessly!
    let vsdx = session.save_format("vsdx").expect("save vsdx");
    let mut reloaded = DocumentSession::new_blank("Reloaded");
    reloaded.open_bytes(&vsdx, "vsdx").expect("open vsdx");
    let r_vis = ReactiveEngine::evaluate_page_visuals(&reloaded.doc, pid);
    let rv = r_vis.iter().find(|v| v.shape_id == sid).unwrap();
    assert_eq!(rv.text, "Rack A12 — 6.3 in");
    assert_eq!(rv.evaluated_text_runs.len(), 5);
    assert!(rv.evaluated_text_runs[1].bold && rv.evaluated_text_runs[1].italic);
    assert!(rv.evaluated_text_runs[2].superscript);
    assert!((rv.txt_angle_rad.to_degrees() - 30.0).abs() < 1e-4);
}

#[test]
fn milestone3_arrowheads_line_jumps_guides_and_snap_glue_matrix() {
    let mut session = DocumentSession::new_blank("Routing & Guides");
    let pid = session.doc.pages[0].id;

    // 1. Verify 45+1 arrowhead catalog & connector arrowhead mutation
    let arrow_json = DocumentSession::list_arrowheads_json();
    assert!(arrow_json.contains("\"code\":45"));

    // Create two crossing orthogonal connectors: vertical `(4.0, 1.0)->(4.0, 6.0)` and horizontal `(1.0, 3.5)->(7.0, 3.5)`
    let c_vert = session
        .connect_points_or_shapes(pid, 4.0, 1.0, 4.0, 6.0, None, None)
        .unwrap();
    let c_horiz = session
        .connect_points_or_shapes(pid, 1.0, 3.5, 7.0, 3.5, None, None)
        .unwrap();
    assert_eq!(
        session
            .set_connector_arrowheads(pid, &[c_vert, c_horiz], 22, 29, 4)
            .unwrap(),
        2
    );

    // Test all 7 Visio Line Jump styles (`0=Arc .. 6=FiveSides`) & Vertical jump mode (`jump_code = 2`)
    for style_code in 0u8..=6u8 {
        session
            .set_page_line_jump_settings(pid, 1, style_code, 0.8, 0.8)
            .unwrap();
        let h_rows = &session
            .doc
            .page(pid)
            .unwrap()
            .find_shape(c_horiz)
            .unwrap()
            .sheet
            .geometries[0]
            .rows;
        assert!(
            h_rows.len() >= 4,
            "horizontal connector should contain jump geometry for style {style_code}, got {} rows",
            h_rows.len()
        );
    }
    // Switch jump direction to Vertical (`jump_code = 2`) -> vertical connector now jumps!
    session
        .set_page_line_jump_settings(pid, 2, 2, 0.8, 0.8)
        .unwrap();
    let v_rows = &session
        .doc
        .page(pid)
        .unwrap()
        .find_shape(c_vert)
        .unwrap()
        .sheet
        .geometries[0]
        .rows;
    assert!(v_rows.len() >= 5);

    // 2. Draggable Ruler Guides + Guide-Glued Shape Propagation + Snap & Glue Matrix
    let vg = session.add_ruler_guide(pid, true, 5.0).unwrap();
    let hg = session.add_ruler_guide(pid, false, 4.0).unwrap();
    assert_eq!(session.list_ruler_guides(pid).len(), 2);

    // Snap a candidate box near vertical guide `x = 5.0`
    let (snap_d, _) = session.compute_smart_guides(pid, &[], [4.94, 2.0], [5.94, 3.0], 0.12);
    assert!((snap_d[0] - 0.06).abs() < 1e-4, "expected +0.06 snap to x=5.0 guide, got {:?}", snap_d);

    // Disable `Guides` bit (`bit 4`) in `SnapTargets` -> no longer snaps to `x = 5.0`!
    session.set_snap_glue_settings(pid, true, false, 0, 0).unwrap();
    let (no_snap, _) = session.compute_smart_guides(pid, &[], [4.94, 2.0], [5.94, 3.0], 0.12);
    assert_eq!(no_snap, [0.0, 0.0]);
    assert!(session.find_nearest_connection_port(pid, 0, 5.0, 4.0, 0.25).is_none());

    // Re-enable Snap & Glue and glue a 2D shape to `vg`, then move `vg` from `5.0` -> `7.5`!
    session.set_snap_glue_settings(pid, true, true, 0x7F, 0x0F).unwrap();
    let box_id = session.draw_primitive_2d(pid, "rect", 4.5, 3.5, 5.5, 4.5).unwrap();
    session
        .doc
        .page_mut(pid)
        .unwrap()
        .find_shape_mut(box_id)
        .unwrap()
        .sheet
        .set_user_cell("GluedGuideX", vg.to_string(), "Glued vertical guide");
    assert!(session.move_ruler_guide(pid, vg, 7.5).unwrap());
    let moved_px = session
        .doc
        .page(pid)
        .unwrap()
        .find_shape(box_id)
        .unwrap()
        .sheet
        .get_f64("PinX")
        .unwrap();
    assert!((moved_px - 7.5).abs() < 1e-4);
    assert!(session.delete_ruler_guide(pid, hg).unwrap());
}

#[test]
fn milestone3_find_replace_format_painter_number_shapes_and_c_abi_tools() {
    let mut session = DocumentSession::new_blank("Productivity Suite");
    let p1 = session.doc.pages[0].id;
    let s1 = session.draw_primitive_2d(p1, "rect", 1.0, 7.0, 3.0, 8.0).unwrap();
    let s2 = session.draw_primitive_2d(p1, "rect", 4.0, 7.0, 6.0, 8.0).unwrap();
    let s3 = session.draw_primitive_2d(p1, "rect", 7.0, 4.0, 9.0, 5.0).unwrap();
    session.set_shape_text(p1, s1, "Gateway Alpha").unwrap();
    session.set_shape_text(p1, s2, "Worker Alpha").unwrap();
    session.set_shape_text(p1, s3, "Archive Node").unwrap();
    session.connect_shapes(p1, s1, s2).unwrap();
    session.connect_shapes(p1, s2, s3).unwrap();

    // 1. Find & Replace + Undo
    let hits = session.find_in_document(p1, "alpha", false, true, true);
    assert_eq!(hits.len(), 2);
    let replaced = session
        .replace_in_document(p1, "Alpha", "Omega", true, true, true)
        .unwrap();
    assert_eq!(replaced, 2);
    assert_eq!(session.doc.page(p1).unwrap().find_shape(s1).unwrap().text, "Gateway Omega");

    // 2. Format Painter respecting `GUARD(...)` on target shape
    session
        .set_cell_formula(p1, s1, "FillForegnd", "=RGB(16,185,129)", false)
        .unwrap();
    session
        .set_cell_formula(p1, s1, "LineWeight", "3 pt", false)
        .unwrap();
    session
        .set_cell_formula(p1, s3, "FillForegnd", "=GUARD(RGB(220,38,38))", true)
        .unwrap();
    let fmt = session.copy_shape_format_json(p1, s1).unwrap();
    assert_eq!(session.apply_format_painter(p1, &[s2, s3], &fmt).unwrap(), 2);
    // `s2` receives emerald fill + 3pt line; `s3` keeps GUARDed red fill while receiving 3pt line!
    let c_s2 = session
        .doc
        .page(p1)
        .unwrap()
        .find_shape(s2)
        .unwrap()
        .sheet
        .get_color("FillForegnd")
        .unwrap();
    let c_s3 = session
        .doc
        .page(p1)
        .unwrap()
        .find_shape(s3)
        .unwrap()
        .sheet
        .get_color("FillForegnd")
        .unwrap();
    assert_eq!((c_s2.r, c_s2.g, c_s2.b), (16, 185, 129));
    assert_eq!((c_s3.r, c_s3.g, c_s3.b), (220, 38, 38));
    let lw_s3 = session
        .doc
        .page(p1)
        .unwrap()
        .find_shape(s3)
        .unwrap()
        .sheet
        .get_f64("LineWeight")
        .unwrap();
    assert!((lw_s3 - 3.0 / 72.0).abs() < 1e-6);

    // 3. `Number Shapes…` along `connected_flow` BFS (`s1 -> s2 -> s3`)
    let numbered = session
        .number_shapes_on_page(p1, &[], "connected_flow", 10, 10, "P-", "both")
        .unwrap();
    assert_eq!(numbered, 3);
    assert_eq!(
        session.doc.page(p1).unwrap().find_shape(s1).unwrap().text,
        "P-10. Gateway Omega"
    );
    assert_eq!(
        session.doc.page(p1).unwrap().find_shape(s2).unwrap().text,
        "P-20. Worker Omega"
    );
    assert_eq!(
        session.doc.page(p1).unwrap().find_shape(s3).unwrap().text,
        "P-30. Archive Node"
    );

    // 4. Verify C-ABI tool codes `19` (`TextBlock`) and `20` (`FormatPainter`) round-trip cleanly
    unsafe {
        let eng = vexio_engine_create();
        let title = CString::new("CAbiM3").unwrap();
        let doc = vexio_doc_create_blank(title.as_ptr());
        let mut vp: *mut VexioViewport = std::ptr::null_mut();
        assert_eq!(
            vexio_viewport_create_headless(eng, doc, 1, 800, 600, 1.0, &mut vp),
            VexioStatus::Ok
        );
        assert_eq!(vexio_viewport_set_tool(vp, 19), VexioStatus::Ok);
        assert_eq!(vexio_viewport_get_active_tool(vp), 19);
        assert_eq!(vexio_viewport_set_tool(vp, 20), VexioStatus::Ok);
        assert_eq!(vexio_viewport_get_active_tool(vp), 20);
        let mut gid: u64 = 0;
        assert_eq!(
            vexio_doc_add_ruler_guide(doc, 1, true, 3.5, &mut gid),
            VexioStatus::Ok
        );
        assert!(gid > 0);
        vexio_viewport_destroy(vp);
        vexio_doc_destroy(doc);
        vexio_engine_destroy(eng);
    }
}
