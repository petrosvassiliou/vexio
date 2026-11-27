//! Milestone 2 (`v0.7.0-beta`) Integration & C-ABI Verification Suite:
//! 1. 238-Function ShapeSheet Catalog (`vexio_list_shapesheet_functions_json` + live formula evaluation)
//! 2. Smart-Shape `Actions.*` Context Menus (`upsert` / `list` / `execute` with `SETF`, `GOTOPAGE`, `RUNMACRO` + Undo/Redo)
//! 3. `Shift+Ctrl+1` Interactive Connection Point Tool (`NativeCanvasTool::ConnectionPoint = 18` + `add`/`move`/`delete` + connector glue)
//! 4. `"My Shapes"` Custom Stencil Authoring, Master Editing (`update_custom_master` instance propagation), Rename/Delete & `.vssx` Export

use std::ffi::CString;
use std::ptr;
use vexio_core::ffi::c_abi::*;
use vexio_core::workspace::DocumentSession;

#[test]
fn milestone2_smart_actions_connection_points_and_my_shapes_vssx_roundtrip() {
    // 1. Verify 238-function catalog via DocumentSession
    let fn_json = DocumentSession::list_shapesheet_functions_json();
    let fn_list: Vec<String> = serde_json::from_str(&fn_json).expect("valid functions JSON");
    assert!(fn_list.len() >= 235);
    assert!(fn_list.contains(&"CONTAINERSHEETREF".to_string()));
    assert!(fn_list.contains(&"POINTALONGPATH".to_string()));
    assert!(fn_list.contains(&"NETWORKDAYS".to_string()));

    let mut session = DocumentSession::new_blank("Milestone 2 Parity");
    let p1 = session.doc.pages[0].id;
    let p2 = session.add_page("Diagnostics").expect("add page 2");

    // 2. Smart-Shape `Actions.*`: Toggle state via `SETF`, navigate via `GOTOPAGE`, Undo/Redo
    let valve_id = session
        .draw_primitive_2d(p1, "rect", 2.0, 3.0, 4.0, 4.5)
        .expect("draw valve");
    session
        .set_cell_formula(p1, valve_id, "User.ValveOpen", "0", true)
        .expect("init User.ValveOpen");
    session
        .set_cell_formula(
            p1,
            valve_id,
            "FillForegnd",
            "=IF(User.ValveOpen, RGB(34,197,94), RGB(239,68,68))",
            true,
        )
        .expect("reactive fill");

    session
        .upsert_shape_action(
            p1,
            valve_id,
            "ToggleValve",
            "Toggle Valve",
            "=SETF(GetRef(User.ValveOpen), IF(User.ValveOpen, 0, 1))",
            "=User.ValveOpen",
            "FALSE",
            false,
        )
        .expect("upsert ToggleValve");
    session
        .upsert_shape_action(
            p1,
            valve_id,
            "OpenDiag",
            "Go to Diagnostics",
            "=GOTOPAGE(\"Diagnostics\")",
            "FALSE",
            "FALSE",
            true,
        )
        .expect("upsert OpenDiag");

    let actions_before = session.list_shape_actions(p1, valve_id);
    assert_eq!(actions_before.len(), 2);
    assert!(!actions_before[0].checked);

    let exec_res = session
        .execute_shape_action(p1, valve_id, "ToggleValve")
        .expect("execute ToggleValve");
    assert!(exec_res.contains("\"executed\":true"));
    let actions_after = session.list_shape_actions(p1, valve_id);
    assert!(actions_after[0].checked, "User.ValveOpen must flip to 1 and check the menu item!");

    // Undo & Redo action execution!
    assert!(session.journal.undo(&mut session.doc).unwrap());
    assert!(!session.list_shape_actions(p1, valve_id)[0].checked);
    assert!(session.journal.redo(&mut session.doc).unwrap());
    assert!(session.list_shape_actions(p1, valve_id)[0].checked);

    let goto_res = session
        .execute_shape_action(p1, valve_id, "OpenDiag")
        .expect("execute OpenDiag");
    assert!(goto_res.contains(&format!("\"targetPageId\":{p2}")));

    // 3. Connection Points (`add` / `move` / `delete`) with proportional `Width*fx` formulas
    let cp_ix = session
        .add_connection_point(p1, valve_id, 2.5, 4.5, 2)
        .expect("add connection point");
    let ports = session.list_connection_points(p1, valve_id);
    let added_cp = ports.iter().find(|c| c.ix == cp_ix).expect("find added port");
    assert!(added_cp.x_formula.contains("Width"));
    assert!((added_cp.page_x_in - 2.5).abs() < 1e-3);

    assert!(session
        .move_connection_point(p1, valve_id, cp_ix, 3.5, 4.5)
        .expect("move port"));
    let moved_cp = session
        .list_connection_points(p1, valve_id)
        .into_iter()
        .find(|c| c.ix == cp_ix)
        .unwrap();
    assert!((moved_cp.page_x_in - 3.5).abs() < 1e-3);

    // 4. `"My Shapes"` Custom Stencil Authoring, Instance Drop, Master Update Propagation & `.vssx` Export!
    let custom_json = session
        .add_selection_to_custom_stencil(p1, &[valve_id], "My Shapes", "Smart Valve")
        .expect("add to My Shapes");
    assert!(custom_json.contains("\"Smart Valve\""));

    let inst1 = session
        .drop_builtin_shape(p1, "My Shapes", "Smart Valve", 7.0, 4.0)
        .expect("drop Smart Valve instance 1");
    let inst2 = session
        .drop_builtin_shape(p2, "My Shapes", "Smart Valve", 5.0, 5.0)
        .expect("drop Smart Valve instance 2 on page 2");

    // Modify source shape rounding & add a 3rd action, then propagate via `update_custom_master_from_shape`!
    session
        .set_cell_formula(p1, valve_id, "Rounding", "0.25 in", true)
        .expect("set rounding");
    session
        .upsert_shape_action(
            p1,
            valve_id,
            "ResetAlarm",
            "Reset Alarm",
            "=SETF(GetRef(User.ValveOpen), 0)",
            "FALSE",
            "FALSE",
            false,
        )
        .expect("add 3rd action");

    let updated_cnt = session
        .update_custom_master_from_shape(p1, valve_id, "My Shapes", "Smart Valve")
        .expect("update custom master");
    assert_eq!(updated_cnt, 2, "both instances across pages 1 and 2 must receive master updates");
    assert_eq!(session.list_shape_actions(p1, inst1).len(), 3);
    assert_eq!(session.list_shape_actions(p2, inst2).len(), 3);

    // Export `"My Shapes"` as `.vssx` and re-import into a fresh session!
    let vssx_bytes = session
        .export_custom_stencil_vssx("My Shapes")
        .expect("export .vssx");
    assert!(vssx_bytes.starts_with(b"PK\x03\x04"));

    let mut fresh = DocumentSession::new_blank("Imported My Shapes Test");
    let (_, imported_previews) = fresh
        .import_stencil_package("My_Shapes.vssx", &vssx_bytes)
        .expect("re-import exported .vssx");
    assert_eq!(imported_previews.len(), 1);
    assert_eq!(imported_previews[0].name, "Smart Valve");

    // Rename & Delete Custom Master
    assert!(session.rename_custom_master("My Shapes", "Smart Valve", "Industrial Valve").unwrap());
    assert!(session.delete_custom_master("My Shapes", "Industrial Valve").unwrap());
    assert!(session.delete_connection_point(p1, valve_id, cp_ix).unwrap());
    assert!(session.delete_shape_action(p1, valve_id, "ResetAlarm").unwrap());
}

#[test]
fn c_abi_milestone2_tool_18_and_ffi_endpoints_smoke() {
    unsafe {
        let eng = vexio_engine_create();
        let title = CString::new("C-ABI M2").unwrap();
        let doc = vexio_doc_create_blank(title.as_ptr());

        let mut fn_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
        assert_eq!(vexio_list_shapesheet_functions_json(&mut fn_buf), VexioStatus::Ok);
        assert!(fn_buf.len > 500);
        vexio_buffer_free(fn_buf);

        let rect_c = CString::new("rect").unwrap();
        let mut sid = 0u64;
        assert_eq!(
            vexio_doc_draw_primitive(doc, 1, rect_c.as_ptr(), 3.0, 3.0, 5.0, 4.5, &mut sid),
            VexioStatus::Ok
        );

        // Test C-ABI Smart Actions
        let r_name = CString::new("FlipColor").unwrap();
        let menu = CString::new("Highlight Gold").unwrap();
        let act = CString::new("=SETF(GetRef(FillForegnd), \"RGB(250,204,21)\")").unwrap();
        let fls = CString::new("FALSE").unwrap();
        assert_eq!(
            vexio_doc_upsert_shape_action(
                doc,
                1,
                sid,
                r_name.as_ptr(),
                menu.as_ptr(),
                act.as_ptr(),
                fls.as_ptr(),
                fls.as_ptr(),
                false,
            ),
            VexioStatus::Ok
        );
        let mut exec_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
        assert_eq!(
            vexio_doc_execute_shape_action(doc, 1, sid, r_name.as_ptr(), &mut exec_buf),
            VexioStatus::Ok
        );
        vexio_buffer_free(exec_buf);

        // Test C-ABI Connection Point Tool 18 (`NativeCanvasTool::ConnectionPoint = 18`)
        let mut vp: *mut VexioViewport = ptr::null_mut();
        assert_eq!(
            vexio_viewport_create_headless(eng, doc, 1, 800, 600, 1.0, &mut vp),
            VexioStatus::Ok
        );
        assert_eq!(vexio_viewport_set_selection(vp, doc, &sid, 1), VexioStatus::Ok);
        assert_eq!(vexio_viewport_set_tool(vp, 18), VexioStatus::Ok);
        assert_eq!(vexio_viewport_get_active_tool(vp), 18);

        let mut cp_ix = 0u32;
        assert_eq!(
            vexio_doc_add_connection_point(doc, 1, sid, 4.0, 4.5, 2, &mut cp_ix),
            VexioStatus::Ok
        );
        assert!(cp_ix >= 1);
        assert_eq!(
            vexio_doc_move_connection_point(doc, 1, sid, cp_ix, 4.5, 4.5),
            VexioStatus::Ok
        );

        // Custom Stencil C-ABI
        let my_shapes = CString::new("My Shapes").unwrap();
        let m_name = CString::new("GoldBox").unwrap();
        let mut st_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
        assert_eq!(
            vexio_doc_add_to_custom_stencil(doc, 1, &sid, 1, my_shapes.as_ptr(), m_name.as_ptr(), &mut st_buf),
            VexioStatus::Ok
        );
        vexio_buffer_free(st_buf);

        let mut vssx_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
        assert_eq!(
            vexio_doc_export_custom_stencil_vssx(doc, my_shapes.as_ptr(), &mut vssx_buf),
            VexioStatus::Ok
        );
        assert!(vssx_buf.len > 200);
        vexio_buffer_free(vssx_buf);

        vexio_viewport_destroy(vp);
        vexio_doc_destroy(doc);
        vexio_engine_destroy(eng);
    }
}
