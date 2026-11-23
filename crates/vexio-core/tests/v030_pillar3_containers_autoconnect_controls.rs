//! End-to-end integration & `.vsdx` round-trip tests for Vexio `v0.3.0-alpha` Pillar #3:
//! 1. Visio AutoConnect directional triangles (`North`, `East`, `South`, `West`): one-click stamp & wire,
//!    aligned neighbor auto-linking, Container inheritance, and C-ABI pointer click/hover dispatch.
//! 2. Interactive Containers (`User.msvStructureType = "Container"`): drag-in enrollment, `ExpandOnly`
//!    auto-fit, synchronous Container-moves-members translation, drag-out unenrollment, and emerald drop glow.
//! 3. Cross-Functional Swimlanes (`User.visSwimlane = 1`): 3-lane pool creation, contiguous lane append,
//!    `Prop.Function` auto-assignment, and synchronized pool right-edge expansion.
//! 4. Yellow Diamond `Controls.*` Handles & Reactive Callouts: `Geometry2` leader line tracking target motion,
//!    C-ABI yellow handle pointer drag + Callout target re-tethering, Rounded Rectangle corner radius handle,
//!    Undo/Redo integrity, and lossless `.vsdx` OpenXML round-trip.

use std::ffi::CString;
use vexio_core::containers::{
    find_drop_target_container_or_swimlane, is_container_shape, is_swimlane_shape,
    parse_container_members,
};
use vexio_core::ffi::c_abi::{
    VexioPointerEvent, vexio_doc_attach_callout, vexio_doc_auto_connect, vexio_doc_create_blank,
    vexio_doc_destroy, vexio_doc_draw_primitive, vexio_doc_insert_container,
    vexio_doc_insert_swimlanes, vexio_engine_create, vexio_engine_destroy,
    vexio_viewport_create_headless, vexio_viewport_destroy, vexio_viewport_dispatch_pointer,
    vexio_viewport_get_primary_selection, vexio_viewport_set_selection,
};
use vexio_core::workspace::DocumentSession;
use vexio_render::{
    AutoConnectDir, InteractiveCanvasOverlay, ViewportCamera, autoconnect_arrow_center_in,
    build_page_scene_mesh_with_overlay, tessellate_autoconnect_triangles,
    tessellate_interactive_overlay, tessellate_selection_handles,
};
use vexio_shapesheet::ReactiveEngine;
use vexio_vsdx::{VsdxPackageKind, read_vsdx, write_vsdx};

#[test]
fn autoconnect_one_click_neighbor_linking_and_container_inheritance_work() {
    let mut sess = DocumentSession::new_blank("AutoConnectSuite");
    let pid = 1;
    let s1 = sess
        .draw_primitive_2d(pid, "round_rect", 2.0, 4.0, 3.6, 5.0)
        .unwrap();
    sess.set_shape_text(pid, s1, "Start Step").unwrap();

    // Enclose `s1` in a Container first so AutoConnecting from `s1` automatically enrolls & expands the Container!
    let cont_id = sess.insert_container(pid, "Order Service", &[s1]).unwrap();
    let initial_cont_w = sess
        .doc
        .page(pid)
        .unwrap()
        .find_shape(cont_id)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();

    // 1) One-click AutoConnect East stamps `s2` to the right of `s1`, glues a dynamic connector, and expands `cont_id`!
    let (s2, conn1) = sess
        .auto_connect_shape(pid, s1, AutoConnectDir::East)
        .unwrap();
    assert_ne!(s2, s1);
    assert!(conn1 > 0);

    let page = sess.doc.page(pid).unwrap();
    let sh1 = page.find_shape(s1).unwrap();
    let sh2 = page.find_shape(s2).unwrap();
    assert!(sh2.sheet.get_f64("PinX").unwrap() > sh1.sheet.get_f64("PinX").unwrap() + 2.0);
    assert!(
        parse_container_members(page.find_shape(cont_id).unwrap()).contains(&s2),
        "AutoConnected step inside a Container must automatically join User.msvContainerMembers"
    );
    let expanded_cont_w = page
        .find_shape(cont_id)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();
    assert!(expanded_cont_w > initial_cont_w + 1.5);

    // 2) Place an unlinked shape `s3` 1.0 in South of `s2` -> AutoConnect South must link directly to `s3` without duplicating!
    let s2_px = sh2.sheet.get_f64("PinX").unwrap();
    let s2_py = sh2.sheet.get_f64("PinY").unwrap();
    let s3 = sess
        .draw_primitive_2d(pid, "diamond", s2_px - 0.65, s2_py - 2.6, s2_px + 0.65, s2_py - 1.5)
        .unwrap();
    let shape_count_before = sess.doc.page(pid).unwrap().shapes.len();
    let (linked_id, _conn2) = sess
        .auto_connect_shape(pid, s2, AutoConnectDir::South)
        .unwrap();
    assert_eq!(
        linked_id, s3,
        "AutoConnect must wire directly to an aligned existing neighbor within corridor"
    );
    // Only 1 new shape (the connector) was added!
    assert_eq!(sess.doc.page(pid).unwrap().shapes.len(), shape_count_before + 1);

    // 3) Undo cleanly reverts the neighbor link (#1), the drawn diamond `s3` (#2), AND the stamped East shape `s2` + Container expansion (#3)!
    assert!(sess.journal.undo(&mut sess.doc).unwrap());
    assert!(sess.journal.undo(&mut sess.doc).unwrap());
    assert!(sess.doc.page(pid).unwrap().find_shape(s3).is_none());
    assert!(sess.journal.undo(&mut sess.doc).unwrap());
    assert!(sess.doc.page(pid).unwrap().find_shape(s2).is_none());
    let restored_cont_w = sess
        .doc
        .page(pid)
        .unwrap()
        .find_shape(cont_id)
        .unwrap()
        .sheet
        .get_f64("Width")
        .unwrap();
    assert!((restored_cont_w - initial_cont_w).abs() < 1e-4);
}

#[test]
fn interactive_containers_and_swimlanes_enroll_move_members_and_expand() {
    let mut sess = DocumentSession::new_blank("ContainersAndSwimlanes");
    let pid = 1;

    // 1) Create standalone Container + external shape, then drag shape into Container
    let cont_id = sess.insert_container(pid, "VPC Subnet", &[]).unwrap();
    let cont_px = sess.doc.page(pid).unwrap().find_shape(cont_id).unwrap().sheet.get_f64("PinX").unwrap();
    let cont_py = sess.doc.page(pid).unwrap().find_shape(cont_id).unwrap().sheet.get_f64("PinY").unwrap();

    let node_id = sess
        .draw_primitive_2d(pid, "rect", 1.0, 1.0, 2.4, 1.9)
        .unwrap();
    assert!(!parse_container_members(sess.doc.page(pid).unwrap().find_shape(cont_id).unwrap()).contains(&node_id));

    // Check drop-target glow lookup at `(cont_px, cont_py)`
    let drop_hit =
        find_drop_target_container_or_swimlane(&sess.doc, pid, &[node_id], [cont_px, cont_py]);
    assert_eq!(drop_hit.map(|(id, _, _)| id), Some(cont_id));
    let mut ov = InteractiveCanvasOverlay::default();
    ov.container_drop_target_in = drop_hit.map(|(_, mn, mx)| (mn, mx));
    let glow_mesh = tessellate_interactive_overlay(&ov, 96.0);
    assert!(!glow_mesh.vertices.is_empty());

    // Move `node_id` inside `cont_id` -> automatically enrolled in `User.msvContainerMembers`!
    let node_px = sess.doc.page(pid).unwrap().find_shape(node_id).unwrap().sheet.get_f64("PinX").unwrap();
    let node_py = sess.doc.page(pid).unwrap().find_shape(node_id).unwrap().sheet.get_f64("PinY").unwrap();
    sess.move_shapes_delta(pid, &[node_id], cont_px - node_px, cont_py - node_py)
        .unwrap();
    assert!(
        parse_container_members(sess.doc.page(pid).unwrap().find_shape(cont_id).unwrap())
            .contains(&node_id)
    );

    // 2) Moving the Container itself moves all enrolled member shapes synchronously!
    let before_member_x = sess.doc.page(pid).unwrap().find_shape(node_id).unwrap().sheet.get_f64("PinX").unwrap();
    sess.move_shapes_delta(pid, &[cont_id], 1.75, -0.5).unwrap();
    let after_member_x = sess.doc.page(pid).unwrap().find_shape(node_id).unwrap().sheet.get_f64("PinX").unwrap();
    assert!((after_member_x - (before_member_x + 1.75)).abs() < 1e-4);

    // 3) Dragging `node_id` completely outside the Container unenrolls it!
    sess.move_shapes_delta(pid, &[node_id], -5.0, -3.0).unwrap();
    assert!(
        !parse_container_members(sess.doc.page(pid).unwrap().find_shape(cont_id).unwrap())
            .contains(&node_id)
    );

    // 4) Cross-Functional Swimlanes: first call creates 3-lane pool, second call appends contiguous Lane 4!
    let mut swim_sess = DocumentSession::new_blank("SwimlaneSuite");
    let lanes_1_to_3 = swim_sess.insert_swimlanes(1, "").unwrap();
    assert_eq!(lanes_1_to_3.len(), 3);
    let lane_4 = swim_sess.insert_swimlanes(1, "").unwrap();
    assert_eq!(lane_4.len(), 1);
    assert_eq!(
        swim_sess.doc.page(1).unwrap().find_shape(lane_4[0]).unwrap().text,
        "Lane 4"
    );

    // Drop a task inside the top swimlane (`Customer`) -> `Prop.Function` automatically becomes `"Customer"`!
    let top_lane = swim_sess.doc.page(1).unwrap().find_shape(lanes_1_to_3[0]).unwrap();
    let top_lane_y = top_lane.sheet.get_f64("PinY").unwrap();
    let orig_lane_w = top_lane.sheet.get_f64("Width").unwrap();
    let task_id = swim_sess
        .drop_builtin_shape(1, "Basic Flowchart", "Process", 3.0, top_lane_y)
        .unwrap();
    let fn_val = swim_sess
        .doc
        .page(1)
        .unwrap()
        .find_shape(task_id)
        .unwrap()
        .sheet
        .get_str("Prop.Function")
        .unwrap_or_default();
    assert_eq!(fn_val, "Customer");

    // Drag `task_id` past the right edge of the swimlane -> entire 4-lane pool expands in unison!
    swim_sess.move_shapes_delta(1, &[task_id], 9.0, 0.0).unwrap();
    for &lid in lanes_1_to_3.iter().chain(lane_4.iter()) {
        let w_now = swim_sess.doc.page(1).unwrap().find_shape(lid).unwrap().sheet.get_f64("Width").unwrap();
        assert!(w_now > orig_lane_w + 1.0);
    }
}

#[test]
fn yellow_control_handles_reactive_callout_retethering_c_abi_and_vsdx_roundtrip() {
    unsafe {
        let eng = vexio_engine_create();
        let title = CString::new("CAbiPillar3").unwrap();
        let doc = vexio_doc_create_blank(title.as_ptr());
        let mut vp = std::ptr::null_mut();
        assert_eq!(
            vexio_viewport_create_headless(eng, doc, 1, 1280, 800, 2.0, &mut vp) as u32,
            0
        );

        let kind = CString::new("round_rect").unwrap();
        let mut s1: u64 = 0;
        let mut s2: u64 = 0;
        assert_eq!(
            vexio_doc_draw_primitive(doc, 1, kind.as_ptr(), 2.5, 4.0, 4.1, 5.0, &mut s1) as u32,
            0
        );
        assert_eq!(
            vexio_doc_draw_primitive(doc, 1, kind.as_ptr(), 7.0, 4.0, 8.6, 5.0, &mut s2) as u32,
            0
        );

        // Select `s1` and verify RoundedRect exposes a yellow `Controls.Row_1` diamond handle AND 4 AutoConnect triangles
        assert_eq!(vexio_viewport_set_selection(vp, doc, &s1, 1) as u32, 0);
        {
            let doc_ref = &(*doc).session.doc;
            let visuals = ReactiveEngine::evaluate_page_visuals(doc_ref, 1);
            let v1 = visuals.iter().find(|v| v.shape_id == s1).unwrap();
            assert_eq!(v1.control_handles.len(), 1);
            let sel_mesh = tessellate_selection_handles(v1, 96.0);
            let ac_mesh = tessellate_autoconnect_triangles(v1, 96.0, Some(AutoConnectDir::East));
            assert!(!sel_mesh.vertices.is_empty());
            assert_eq!(ac_mesh.indices.len() % 3, 0);
            assert!(!ac_mesh.vertices.is_empty());

            // Verify scene builder includes AutoConnect + yellow diamond handle without overlapping Rotate knob
            let cam = ViewportCamera::new(1280, 800, 2.0);
            let (scene, _, _) = build_page_scene_mesh_with_overlay(
                doc_ref,
                1,
                &cam,
                &mut Default::default(),
                false,
                false,
                &[s1],
                None,
            )
            .unwrap();
            assert!(scene.vertices.len() > 80);

            // Dispatch C-ABI click directly on `s1`'s North AutoConnect triangle (`50 dp` above top edge, cleanly above `28 dp` Rotate knob!)
            let vp_cam = (*vp).camera;
            let (north_pt_in, _) =
                autoconnect_arrow_center_in(&v1.transform, AutoConnectDir::North, vp_cam.zoom_ppi);
            let north_px = vp_cam.page_in_to_screen(north_pt_in[0], north_pt_in[1]);
            let mut cursor = 0u32;
            let down = VexioPointerEvent {
                kind: 1,
                buttons: 1,
                modifiers: 0,
                x_px: north_px[0],
                y_px: north_px[1],
                pressure: 0.5,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 100,
            };
            let up = VexioPointerEvent { kind: 2, buttons: 0, ..down };
            assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &down, &mut cursor) as u32, 0);
            assert_eq!(cursor, 2);
            assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &up, &mut cursor) as u32, 0);
        }

        let auto_created_id = vexio_viewport_get_primary_selection(vp);
        assert!(
            auto_created_id != s1 && auto_created_id != s2 && auto_created_id > 0,
            "Clicking North AutoConnect triangle must stamp & select a new connected shape"
        );

        // Attach Callout to `s1`, then drag its yellow `Controls.Row_1` diamond handle onto `s2` via C-ABI pointer events!
        let note = CString::new("Latency Budget").unwrap();
        let mut callout_id: u64 = 0;
        assert_eq!(
            vexio_doc_attach_callout(doc, 1, s1, note.as_ptr(), &mut callout_id) as u32,
            0
        );
        assert!(callout_id > 0);
        assert_eq!(vexio_viewport_set_selection(vp, doc, &callout_id, 1) as u32, 0);

        let (ctrl_screen_px, s2_screen_px) = {
            let doc_ref = &(*doc).session.doc;
            let visuals = ReactiveEngine::evaluate_page_visuals(doc_ref, 1);
            let vc = visuals.iter().find(|v| v.shape_id == callout_id).unwrap();
            assert_eq!(vc.control_handles.len(), 1);
            // Initially tethered to `s1` (`PinX = 3.3, PinY = 4.5`)
            assert!((vc.control_handles[0].pos_page_in[0] - 3.3).abs() < 1e-3);
            assert!((vc.control_handles[0].pos_page_in[1] - 4.5).abs() < 1e-3);
            let vp_cam = (*vp).camera;
            (
                vp_cam.page_in_to_screen(
                    vc.control_handles[0].pos_page_in[0],
                    vc.control_handles[0].pos_page_in[1],
                ),
                vp_cam.page_in_to_screen(7.8, 4.5),
            )
        };

        let mut cur = 0u32;
        let c_down = VexioPointerEvent {
            kind: 1,
            buttons: 1,
            modifiers: 0,
            x_px: ctrl_screen_px[0],
            y_px: ctrl_screen_px[1],
            pressure: 0.5,
            tilt_x_deg: 0.0,
            tilt_y_deg: 0.0,
            timestamp_us: 200,
        };
        let c_move = VexioPointerEvent {
            kind: 0,
            x_px: s2_screen_px[0],
            y_px: s2_screen_px[1],
            timestamp_us: 220,
            ..c_down
        };
        let c_up = VexioPointerEvent {
            kind: 2,
            buttons: 0,
            x_px: s2_screen_px[0],
            y_px: s2_screen_px[1],
            timestamp_us: 240,
            ..c_down
        };
        assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &c_down, &mut cur) as u32, 0);
        assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &c_move, &mut cur) as u32, 0);
        assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &c_up, &mut cur) as u32, 0);

        // Verify Callout re-tethered reactively to `Sheet.{s2}`!
        let ctrl_formula = (*doc)
            .session
            .doc
            .page(1)
            .unwrap()
            .find_shape(callout_id)
            .unwrap()
            .sheet
            .get_cell("Controls.Row_1.X")
            .unwrap()
            .formula
            .clone();
        assert!(
            ctrl_formula.contains(&format!("Sheet.{s2}!PinX")),
            "Expected Callout leader to re-tether to Sheet.{s2}, got: {ctrl_formula}"
        );

        // Also test C-ABI `vexio_doc_insert_container`, `vexio_doc_insert_swimlanes`, and `vexio_doc_auto_connect`
        let c_title = CString::new("Backend Cluster").unwrap();
        let members = [s1, s2];
        let mut cont_id: u64 = 0;
        assert_eq!(
            vexio_doc_insert_container(doc, 1, c_title.as_ptr(), members.as_ptr(), members.len(), &mut cont_id) as u32,
            0
        );
        assert!(cont_id > 0);

        let lanes_c = CString::new("Web;API").unwrap();
        let mut lane_cnt: u32 = 0;
        assert_eq!(
            vexio_doc_insert_swimlanes(doc, 1, lanes_c.as_ptr(), &mut lane_cnt) as u32,
            0
        );
        assert_eq!(lane_cnt, 2);

        let east_c = CString::new("east").unwrap();
        let mut ac_tid: u64 = 0;
        let mut ac_cid: u64 = 0;
        assert_eq!(
            vexio_doc_auto_connect(doc, 1, s2, east_c.as_ptr(), &mut ac_tid, &mut ac_cid) as u32,
            0
        );
        assert!(ac_tid > 0 && ac_cid > 0);

        // Lossless `.vsdx` Round-Trip preserving `Controls.Row_1`, `msvStructureType`, `msvContainerMembers`, and `visSwimlane`!
        let vsdx_bytes = write_vsdx(&(*doc).session.doc, VsdxPackageKind::Drawing).unwrap();
        let (reloaded, _) = read_vsdx(&vsdx_bytes).unwrap();
        let rp = reloaded.page(1).unwrap();
        let r_cont = rp.find_shape(cont_id).unwrap();
        assert!(is_container_shape(r_cont));
        assert!(parse_container_members(r_cont).contains(&s1));
        assert!(rp.shapes.iter().any(is_swimlane_shape));
        let r_call = rp.find_shape(callout_id).unwrap();
        assert!(!r_call.sheet.controls.is_empty());

        vexio_viewport_destroy(vp);
        vexio_doc_destroy(doc);
        vexio_engine_destroy(eng);
    }
}
