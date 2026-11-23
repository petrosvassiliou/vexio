use vexio_core::ffi::c_abi::{
    vexio_buffer_free, vexio_doc_create_blank, vexio_doc_destroy, vexio_doc_group_shapes,
    vexio_doc_ungroup_shapes, vexio_engine_create, vexio_engine_destroy,
    vexio_viewport_create_headless, vexio_viewport_destroy, vexio_viewport_dispatch_pointer,
    vexio_viewport_hit_test_shape, VexioOwnedBuffer, VexioPointerEvent, VexioStatus,
    VexioViewport,
};
use vexio_core::DocumentSession;
use vexio_shapesheet::{
    GeometryRow, GeometrySection, ReactiveEngine, ShapeKind, Unit, VisioShape,
};

#[test]
fn group_ungroup_vsdx_parent_formula_baking_nested_groups_and_proportional_resize() {
    let mut sess = DocumentSession::new_blank("GroupUngroupParity");

    // 1. Create two standard shapes (PIM & PAM)
    let pim_id = sess
        .drop_builtin_shape(1, "Basic Shapes", "Rectangle", 3.0, 5.0)
        .unwrap();
    sess.set_shape_text(1, pim_id, "PIM".into()).unwrap();

    let pam_id = sess
        .drop_builtin_shape(1, "Basic Shapes", "Rectangle", 3.0, 4.0)
        .unwrap();
    sess.set_shape_text(1, pam_id, "PAM".into()).unwrap();

    // 2. Construct a `.vsdx`-style Master Group (like `Entra ID` / `Active Directory` in MSSC_Architektur_1.vsdx):
    //    - Group root has own caption text ("Entra ID") and invisible NoShow wrapper geometry
    //    - Child subshapes reference `Sheet.<gid>!Width` and `Sheet.<gid>!Height` in Transform & Geometry!
    let vsdx_gid = 500u64;
    let mut vsdx_grp = VisioShape::new_rect(vsdx_gid, "EntraGroup.500", 5.5, 4.5, 1.2, 1.2);
    vsdx_grp.kind = ShapeKind::Group;
    vsdx_grp.text = "Entra ID".into();
    vsdx_grp.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    vsdx_grp.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    let mut g_wrap = GeometrySection::rectangle(1);
    g_wrap.no_fill = "TRUE".into();
    g_wrap.no_line = "TRUE".into();
    g_wrap.no_show = "TRUE".into();
    vsdx_grp.sheet.add_geometry(g_wrap);

    let mut sub1 = VisioShape::new_rect(501, "FacetTop", 0.6, 0.8, 1.0, 0.6);
    sub1.sheet
        .set_formula("PinX", "=Sheet.500!Width*0.5", Some(Unit::Inch));
    sub1.sheet
        .set_formula("PinY", "=Sheet.500!Height*0.65", Some(Unit::Inch));
    sub1.sheet
        .set_formula("Width", "=Sheet.500!Width*0.8", Some(Unit::Inch));
    sub1.sheet
        .set_formula("Height", "=Sheet.500!Height*0.5", Some(Unit::Inch));
    let mut sub1_geom = GeometrySection::new(1);
    sub1_geom.no_fill = "FALSE".into();
    sub1_geom.no_line = "FALSE".into();
    sub1_geom.rows = vec![
        GeometryRow::move_to(1, "0 in", "0 in"),
        GeometryRow::line_to(2, "=Sheet.500!Width*0.8", "0 in"),
        GeometryRow::line_to(3, "=Sheet.500!Width*0.4", "=Sheet.500!Height*0.5"),
        GeometryRow::line_to(4, "0 in", "0 in"),
    ];
    sub1.sheet.add_geometry(sub1_geom);

    let mut sub2 = VisioShape::new_rect(502, "FacetBottom", 0.6, 0.3, 1.0, 0.4);
    sub2.sheet
        .set_formula("PinX", "=Sheet.500!Width*0.5", Some(Unit::Inch));
    sub2.sheet
        .set_formula("PinY", "=Sheet.500!Height*0.25", Some(Unit::Inch));
    sub2.sheet
        .set_formula("Width", "=Sheet.500!Width*0.8", Some(Unit::Inch));
    sub2.sheet
        .set_formula("Height", "=Sheet.500!Height*0.35", Some(Unit::Inch));
    vsdx_grp.children = vec![sub1, sub2];

    sess.doc.page_mut(1).unwrap().shapes.push(vsdx_grp);
    ReactiveEngine::recalculate_document(&mut sess.doc);

    // 3. Group [PIM, PAM, EntraGroup.500] — passing child ID `501` instead of `500` to verify descendant -> top-level resolution!
    let outer_gid = sess.group_shapes(1, &[pim_id, pam_id, 501]).unwrap();
    assert!(outer_gid > 502, "expected valid created outer group ID");

    let orig_w = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(outer_gid)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();
    let orig_h = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(outer_gid)
        .unwrap()
        .sheet
        .get_f64("Height")
        .unwrap();
    let orig_pim_w = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(pim_id)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();

    // 4. Resize the outer Group by 1.5x -> verify proportional scaling of children!
    sess.transform_shape(1, outer_gid, 4.5, 4.5, orig_w * 1.5, orig_h * 1.5, 0.0)
        .unwrap();
    let scaled_pim_w = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(pim_id)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();
    assert!(
        (scaled_pim_w - orig_pim_w * 1.5).abs() < 1e-2,
        "child width should scale 1.5x with group resize (expected {}, got {scaled_pim_w})",
        orig_pim_w * 1.5
    );

    // 5. Lossless .vsdx round-trip of nested resized group
    let vsdx_bytes = sess.save_format("vsdx").unwrap();
    let mut reloaded = DocumentSession::new_blank("Reloaded");
    reloaded.open_bytes(&vsdx_bytes, "vsdx").unwrap();
    let ungrouped_outer = reloaded.ungroup_shapes(1, &[outer_gid]).unwrap();
    assert_eq!(ungrouped_outer.len(), 3);
    assert!(ungrouped_outer.contains(&vsdx_gid));

    // 6. Ungroup `.vsdx` group `500` by passing child ID `502` -> verify:
    //    - Group root caption "Entra ID" is preserved as a standalone shape
    //    - `Sheet.500!` formulas in `sub1` (including `Geometry1.X2`/`Y3`) baked cleanly with ZERO `#REF!` errors!
    let ungrouped_inner = reloaded.ungroup_shapes(1, &[502]).unwrap();
    assert_eq!(
        ungrouped_inner.len(),
        3,
        "expected base label shape + 2 facets when ungrouping labeled group"
    );
    let page = reloaded.doc.page(1).unwrap();
    assert!(
        page.shapes.iter().any(|s| s.text == "Entra ID"),
        "group root caption 'Entra ID' must survive ungroup"
    );
    let restored_sub1 = page.find_shape(501).unwrap();
    for cell in restored_sub1.sheet.cells.values() {
        assert!(
            !cell.formula.to_ascii_lowercase().contains("sheet.500!"),
            "dangling parent reference remained in {}: {}",
            cell.name,
            cell.formula
        );
        assert!(
            !matches!(cell.value, vexio_shapesheet::CellValue::Err(_)),
            "cell {} evaluated to error {:?} after ungroup",
            cell.name,
            cell.value
        );
    }

    // 7. Undo + Redo restores and re-applies cleanly
    assert!(reloaded.journal.undo(&mut reloaded.doc).unwrap());
    assert!(reloaded.doc.page(1).unwrap().find_shape(vsdx_gid).is_some());
    assert!(reloaded.journal.redo(&mut reloaded.doc).unwrap());
}

#[test]
fn unfilled_layer_frame_click_through_and_enclosure_marquee_select_inner_shapes_and_groups() {
    unsafe {
        let engine = vexio_engine_create();
        let title = std::ffi::CString::new("LayerHitAndMarquee").unwrap();
        let doc = vexio_doc_create_blank(title.as_ptr());
        assert!(!doc.is_null());

        {
            let sess = &mut (*doc).session;
            // Create two inner shapes at (3.0, 4.5) and (3.8, 4.5) and group them -> Group A
            let a1 = sess
                .drop_builtin_shape(1, "Basic Shapes", "Rectangle", 3.0, 4.5)
                .unwrap();
            let a2 = sess
                .drop_builtin_shape(1, "Basic Shapes", "Circle", 3.8, 4.5)
                .unwrap();
            let grp_a = sess.group_shapes(1, &[a1, a2]).unwrap();
            assert!(grp_a > 0);

            // Create another inner shape at (5.5, 4.5)
            let b1 = sess
                .drop_builtin_shape(1, "Basic Shapes", "Diamond", 5.5, 4.5)
                .unwrap();
            assert!(b1 > 0);

            // Add an outer unfilled layer frame (`User & Device Layer` style: ID higher in Z-order,
            // Geometry1 unfilled outer boundary [1.5..7.5, 2.0..7.0], Geometry2 filled header strip at top)
            let mut layer_box = VisioShape::new_rect(900, "UserDeviceLayer", 4.5, 4.5, 6.0, 5.0);
            layer_box.text = "User & Device Layer".into();
            layer_box
                .sheet
                .set_formula("TxtPinY", "=Height - 0.18 in", Some(Unit::Inch));
            layer_box
                .sheet
                .set_formula("TxtHeight", "0.35 in", Some(Unit::Inch));
            layer_box.sheet.geometries.clear();
            let mut g_border = GeometrySection::rectangle(1);
            g_border.no_fill = "TRUE".into();
            g_border.no_line = "FALSE".into();
            layer_box.sheet.add_geometry(g_border);

            let mut g_hdr = GeometrySection::new(2);
            g_hdr.no_fill = "FALSE".into();
            g_hdr.no_line = "FALSE".into();
            g_hdr.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height - 0.35 in"),
                GeometryRow::line_to(2, "=Width", "=Height - 0.35 in"),
                GeometryRow::line_to(3, "=Width", "=Height"),
                GeometryRow::line_to(4, "0 in", "=Height"),
                GeometryRow::line_to(5, "0 in", "=Height - 0.35 in"),
            ];
            layer_box.sheet.add_geometry(g_hdr);
            sess.doc.page_mut(1).unwrap().shapes.push(layer_box);
            ReactiveEngine::recalculate_document(&mut sess.doc);
        }

        let mut vp: *mut VexioViewport = std::ptr::null_mut();
        assert_eq!(
            vexio_viewport_create_headless(engine, doc, 1, 1100, 850, 1.0, &mut vp),
            VexioStatus::Ok
        );
        assert!(!vp.is_null());

        // Clicking on inner Group child at page (3.0, 4.5) MUST click through unfilled layer interior
        // AND resolve child `a1` to top-level `grp_a`!
        let [sx, sy] = (*vp).camera.page_in_to_screen(3.0, 4.5);
        let hit_id = vexio_viewport_hit_test_shape(vp, doc, sx, sy);
        let grp_a_id = (*doc)
            .session
            .doc
            .page(1)
            .unwrap()
            .shapes
            .iter()
            .find(|s| s.kind == ShapeKind::Group)
            .unwrap()
            .id;
        assert_eq!(
            hit_id, grp_a_id,
            "clicking inside unfilled layer over a group member must hit top-level Group ID"
        );

        // Clicking on the filled top header strip of `layer_box` (at page 4.5, 6.82) MUST still hit `900`!
        let [hx, hy] = (*vp).camera.page_in_to_screen(4.5, 6.82);
        assert_eq!(
            vexio_viewport_hit_test_shape(vp, doc, hx, hy),
            900,
            "clicking the filled header bar of User & Device Layer must select the layer box"
        );

        // Dragging an enclosure marquee from (1.9, 6.0) to (6.9, 3.0) inside the [1.5..7.5, 2.0..7.0] layer box
        // MUST select `[grp_a, b1]` WITHOUT grabbing the outer `layer_box` (900)!
        let [mx1, my1] = (*vp).camera.page_in_to_screen(1.9, 6.0);
        let [mx2, my2] = (*vp).camera.page_in_to_screen(6.9, 3.0);
        let mut cur = 0u32;
        let down = VexioPointerEvent {
            kind: 1,
            buttons: 1,
            modifiers: 0,
            x_px: mx1,
            y_px: my1,
            pressure: 0.5,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            timestamp_us: 1000,
        };
        let mv = VexioPointerEvent {
            kind: 0,
            x_px: mx2,
            y_px: my2,
            ..down
        };
        let up = VexioPointerEvent {
            kind: 2,
            buttons: 0,
            x_px: mx2,
            y_px: my2,
            ..down
        };
        assert_eq!(
            vexio_viewport_dispatch_pointer(vp, doc, &down, &mut cur),
            VexioStatus::Ok
        );
        assert_eq!(
            vexio_viewport_dispatch_pointer(vp, doc, &mv, &mut cur),
            VexioStatus::Ok
        );
        assert_eq!(
            vexio_viewport_dispatch_pointer(vp, doc, &up, &mut cur),
            VexioStatus::Ok
        );

        let selected = (*vp).selected_ids.clone();
        assert_eq!(
            selected.len(),
            2,
            "enclosure marquee inside layer box must select the 2 inner shapes without outer layer: {selected:?}"
        );
        assert!(!selected.contains(&900));

        // Now Group those 2 marquee-selected items via C-ABI and Ungroup them back!
        let mut super_gid = 0u64;
        assert_eq!(
            vexio_doc_group_shapes(doc, 1, selected.as_ptr(), selected.len(), &mut super_gid),
            VexioStatus::Ok
        );
        assert!(super_gid > 900);

        let mut out_json = VexioOwnedBuffer {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_ungroup_shapes(doc, 1, &super_gid, 1, &mut out_json),
            VexioStatus::Ok
        );
        vexio_buffer_free(out_json);

        vexio_viewport_destroy(vp);
        vexio_doc_destroy(doc);
        vexio_engine_destroy(engine);
    }
}

#[test]
fn group_ungroup_preserves_connector_glues_world_routing_and_bpmn_validation() {
    let mut sess = DocumentSession::new_blank("GroupConnectorIntegrity");
    let s1 = sess
        .drop_builtin_shape(1, "BPMN 2.0", "Start Event", 2.0, 5.0)
        .unwrap();
    let s2 = sess
        .drop_builtin_shape(1, "BPMN 2.0", "Task", 5.0, 5.0)
        .unwrap();
    let s3 = sess
        .drop_builtin_shape(1, "BPMN 2.0", "End Event", 8.0, 5.0)
        .unwrap();
    let c12 = sess.connect_shapes(1, s1, s2).unwrap();
    let c23 = sess.connect_shapes(1, s2, s3).unwrap();
    assert!(sess.validate("bpmn").is_empty());

    // Group [s1, s2] -> connectors c12 and c23 must stay glued to s1 and s2 and BPMN must remain 100% valid!
    let gid = sess.group_shapes(1, &[s1, s2]).unwrap();
    assert!(gid > 0);
    assert!(
        sess.validate("bpmn").is_empty(),
        "BPMN validation must remain clean while [Start Event, Task] are grouped"
    );

    // Move the group +1.5 in up -> c23 BeginY (glued to s2 inside gid) must follow to ~6.5 in!
    let gw = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(gid)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();
    let gh = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(gid)
        .unwrap()
        .sheet
        .get_f64("Height")
        .unwrap();
    let gx = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(gid)
        .unwrap()
        .sheet
        .get_f64("PinX")
        .unwrap();
    sess.transform_shape(1, gid, gx, 6.5, gw, gh, 0.0).unwrap();
    let c23_by = sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(c23)
        .unwrap()
        .sheet
        .get_f64("BeginY")
        .unwrap();
    assert!(
        (c23_by - 6.5).abs() < 0.6,
        "glued connector c23 must follow grouped member s2 in world space (got BeginY={c23_by})"
    );
    assert!(c12 > 0);

    // Ungroup + Undo + Redo must preserve every glue endpoint and 0 BPMN validation issues!
    let restored = sess.ungroup_shapes(1, &[gid]).unwrap();
    assert_eq!(restored.len(), 2);
    assert!(
        sess.validate("bpmn").is_empty(),
        "BPMN validation must remain clean after ungrouping connected shapes"
    );
    assert!(sess.journal.undo(&mut sess.doc).unwrap());
    assert!(sess.validate("bpmn").is_empty());
    assert!(sess.journal.redo(&mut sess.doc).unwrap());
    assert!(sess.validate("bpmn").is_empty());
}
