//! Integration tests for Milestone 1 (`v0.6.0-beta` — 4-OS Parity Lock):
//! 1. Sandboxed VBA Macro IDE & Runner (`vexio_doc_run_vba_macro`) + Undo/Redo
//! 2. Sub-Shape Vertex Editing (`list/move/insert/delete`) & Freeform Spline Tool + `NativeViewport` Tool `16`
//! 3. Threaded Review Comments (`add/resolve/delete`) & Markup Ink Eraser / Clear (`Tool 17`)
//! 4. Shape Hyperlinks (`Ctrl+K`) & ScreenTips (`NativeStateDto` round-trip)
//! 5. Parameterized Enterprise Domain Wizards (`SQL ⇄ ERD`, `Gantt`, `Timeline`, `PERT/CPM`, `Org Chart`, `42U Rack`, `Arch Room`)
//! 6. CAD Scale Lock (`1/4" = 1'-0"`, `1:50`) & CAD Layer-to-Visio Conversion + Binary `.dwg` Export

use std::ffi::CString;
use std::ptr;
use vexio_core::DocumentSession;
use vexio_core::ffi::c_abi::*;

unsafe fn take_buf_string(buf: VexioOwnedBuffer) -> String {
    let s = if buf.ptr.is_null() || buf.len == 0 {
        String::new()
    } else {
        // SAFETY: `buf` came from a `VexioOwnedBuffer` returned by `c_abi`.
        unsafe { std::slice::from_raw_parts(buf.ptr, buf.len) }
            .iter()
            .copied()
            .collect::<Vec<u8>>()
            .pipe_utf8()
    };
    // SAFETY: Frees buffer allocated by `VexioOwnedBuffer::from_vec`.
    unsafe { vexio_buffer_free(buf) };
    s
}

trait PipeUtf8 {
    fn pipe_utf8(self) -> String;
}

impl PipeUtf8 for Vec<u8> {
    fn pipe_utf8(self) -> String {
        String::from_utf8_lossy(&self).into_owned()
    }
}

#[test]
fn v060_vba_vertices_comments_hyperlinks_wizards_and_cad_c_abi_roundtrip() {
    // SAFETY: Exercises valid C-ABI handles and buffers within a single thread.
    unsafe {
        let eng = vexio_engine_create();
        let title_c = CString::new("Milestone 1 Parity").unwrap();
        let doc = vexio_doc_create_blank(title_c.as_ptr());
        assert!(!doc.is_null());

        // 1. Run VBA Macro via C-ABI (`vexio_doc_run_vba_macro`) & verify Undo/Redo
        let vba_script = CString::new(
            "Sub BuildCluster()\n\
             For i = 1 To 3\n\
                 Set shp = ActivePage.DrawRectangle(1.0 + i * 2.0, 4.0, 2.4 + i * 2.0, 5.0)\n\
                 shp.Text = \"Node-\" & i\n\
             Next i\n\
             End Sub",
        )
        .unwrap();
        let mut vba_rep = VexioOwnedBuffer {
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_run_vba_macro(doc, ptr::null(), 0, vba_script.as_ptr(), &mut vba_rep),
            VexioStatus::Ok
        );
        let rep_json = take_buf_string(vba_rep);
        assert!(
            rep_json.contains("\"shapes_created\":3"),
            "VBA report must create 3 shapes: {rep_json}"
        );
        assert_eq!(vexio_doc_undo(doc), VexioStatus::Ok);
        assert_eq!(vexio_doc_redo(doc), VexioStatus::Ok);

        // 2. Freeform Curve + Sub-Shape Vertex List / Insert / Move / Delete + Viewport Tool 16
        let ff_pts = CString::new("[[1.0, 1.5], [2.5, 2.8], [4.0, 1.7], [5.2, 2.6]]").unwrap();
        let mut ff_sid = 0u64;
        assert_eq!(
            vexio_doc_draw_freeform_json(doc, 1, ff_pts.as_ptr(), false, &mut ff_sid),
            VexioStatus::Ok
        );
        assert!(ff_sid > 0);

        let mut verts_buf = VexioOwnedBuffer {
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_list_vertices_json(doc, 1, ff_sid, &mut verts_buf),
            VexioStatus::Ok
        );
        let verts_before: serde_json::Value =
            serde_json::from_str(&take_buf_string(verts_buf)).unwrap();
        let count_before = verts_before.as_array().unwrap().len();
        assert!(count_before >= 2);

        assert_eq!(
            vexio_doc_insert_vertex(doc, 1, ff_sid, 1, 1, 2.0, 3.2),
            VexioStatus::Ok
        );
        assert_eq!(
            vexio_doc_move_vertex(doc, 1, ff_sid, 1, 2, 2.2, 3.5),
            VexioStatus::Ok
        );
        assert_eq!(
            vexio_doc_delete_vertex(doc, 1, ff_sid, 1, 2),
            VexioStatus::Ok
        );

        // Verify Viewport Tool 16 (`EditVertices`) & Tool 12 (`InkPen`) & Tool 17 (`InkEraser`)
        let mut vp: *mut VexioViewport = ptr::null_mut();
        assert_eq!(
            vexio_viewport_create_headless(eng, doc, 1, 1280, 800, 1.0, &mut vp),
            VexioStatus::Ok
        );
        assert_eq!(
            vexio_viewport_set_selection(vp, doc, &ff_sid, 1),
            VexioStatus::Ok
        );
        assert_eq!(vexio_viewport_set_tool(vp, 16), VexioStatus::Ok);
        let _ = vexio_viewport_render_frame(vp, doc);
        assert!(
            !(*vp).overlay.editable_vertices.is_empty(),
            "EditVertices tool must populate editable_vertices overlay"
        );

        // Draw an Ink stroke with Tool 12 (`InkPen`) and clear it via `vexio_doc_clear_page_ink`
        assert_eq!(vexio_viewport_set_tool(vp, 12), VexioStatus::Ok);
        let mut cur = 0u32;
        for (kind, x_px, y_px) in [(1u32, 200.0, 200.0), (0, 260.0, 240.0), (2, 320.0, 260.0)] {
            let ev = VexioPointerEvent {
                kind,
                buttons: 1,
                modifiers: 0,
                x_px,
                y_px,
                pressure: 0.75,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 1000,
            };
            assert_eq!(
                vexio_viewport_dispatch_pointer(vp, doc, &ev, &mut cur),
                VexioStatus::Ok
            );
        }
        let mut cleared_ink = 0u32;
        assert_eq!(
            vexio_doc_clear_page_ink(doc, 1, &mut cleared_ink),
            VexioStatus::Ok
        );
        assert_eq!(cleared_ink, 1);

        // 3. Threaded Review Comments (`add`, `resolve`, `delete`) + Hyperlinks & ScreenTips
        let author_c = CString::new("Architecture Lead").unwrap();
        let text_c = CString::new("Verify failover SLA on Node-1").unwrap();
        let mut comment_id = 0u32;
        assert_eq!(
            vexio_doc_add_comment(
                doc,
                1,
                ff_sid,
                author_c.as_ptr(),
                text_c.as_ptr(),
                2.0,
                2.0,
                &mut comment_id,
            ),
            VexioStatus::Ok
        );
        assert!(comment_id > 0);
        assert_eq!(
            vexio_doc_resolve_comment(doc, comment_id, true),
            VexioStatus::Ok
        );

        let desc_c = CString::new("Runbook").unwrap();
        let addr_c = CString::new("https://vexio-chromeos.web.app/docs").unwrap();
        let sub_c = CString::new("").unwrap();
        let tip_c = CString::new("Primary ingress spline").unwrap();
        assert_eq!(
            vexio_doc_set_hyperlink_and_screentip(
                doc,
                1,
                ff_sid,
                desc_c.as_ptr(),
                addr_c.as_ptr(),
                sub_c.as_ptr(),
                tip_c.as_ptr(),
            ),
            VexioStatus::Ok
        );

        let mut state_buf = VexioOwnedBuffer {
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_get_state_json(doc, 1, &mut state_buf),
            VexioStatus::Ok
        );
        let state_json = take_buf_string(state_buf);
        assert!(state_json.contains("Primary ingress spline"));
        assert!(state_json.contains("https://vexio-chromeos.web.app/docs"));
        assert!(state_json.contains("Verify failover SLA on Node-1"));

        assert_eq!(
            vexio_doc_delete_comment(doc, comment_id),
            VexioStatus::Ok
        );

        // 4. All Parameterized Enterprise Domain Wizards (`sql_reverse`, `sql_forward`, `gantt`, `timeline`, `pert`, `org_chart`, `rack_42u`, `arch_room`)
        let mut p_sql = 0u32;
        let name_sql = CString::new("ERD").unwrap();
        assert_eq!(vexio_doc_add_page(doc, name_sql.as_ptr(), &mut p_sql), VexioStatus::Ok);
        let wiz_sql_rev = CString::new("sql_reverse").unwrap();
        let ddl_spec = CString::new(
            "CREATE TABLE accounts (id INT PRIMARY KEY, email VARCHAR NOT NULL);\n\
             CREATE TABLE invoices (id INT PRIMARY KEY, account_id INT REFERENCES accounts(id), amount DECIMAL);",
        )
        .unwrap();
        let mut wiz_out = VexioOwnedBuffer {
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_run_domain_wizard(doc, p_sql, wiz_sql_rev.as_ptr(), ddl_spec.as_ptr(), &mut wiz_out),
            VexioStatus::Ok
        );
        assert!(take_buf_string(wiz_out).contains("\"createdCount\":2"));

        let wiz_sql_fwd = CString::new("sql_forward").unwrap();
        let empty_c = CString::new("").unwrap();
        let mut fwd_out = VexioOwnedBuffer {
            ptr: ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_run_domain_wizard(doc, p_sql, wiz_sql_fwd.as_ptr(), empty_c.as_ptr(), &mut fwd_out),
            VexioStatus::Ok
        );
        let fwd_str = take_buf_string(fwd_out);
        assert!(fwd_str.contains("CREATE TABLE accounts") && fwd_str.contains("FOREIGN KEY"));

        for (wslug, wspec) in [
            (
                "gantt",
                "ID,Task,Start,Duration,Pct,DependsOn\n1,Design,0,4,100,\n2,Build,4,6,60,1",
            ),
            (
                "timeline",
                "# Title: Q4 Roadmap | Horizon: 60\n10,Alpha Ship,true\n45,GA Launch,true",
            ),
            (
                "pert",
                "ID,Name,Duration,Pred\nA,Spec,3,\nB,Core,5,A\nC,Docs,2,A\nD,GA,2,B;C",
            ),
            (
                "org_chart",
                "Name,Title,Manager,Department\nAda Lovelace,CEO,,Executive\nGrace Hopper,VP Engineering,Ada Lovelace,Platform",
            ),
            (
                "rack_42u",
                "Device,StartU,HeightU\nCore Switch,40,2\nCompute Blade,20,4",
            ),
            ("arch_room", "Room,WidthIn,HeightIn\nServer Hall,3.0,2.2"),
        ] {
            let mut pid = 0u32;
            let pname = CString::new(wslug).unwrap();
            assert_eq!(vexio_doc_add_page(doc, pname.as_ptr(), &mut pid), VexioStatus::Ok);
            let slug_c = CString::new(wslug).unwrap();
            let spec_c = CString::new(wspec).unwrap();
            let mut out = VexioOwnedBuffer {
                ptr: ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_run_domain_wizard(doc, pid, slug_c.as_ptr(), spec_c.as_ptr(), &mut out),
                VexioStatus::Ok,
                "wizard {wslug} failed"
            );
            let s = take_buf_string(out);
            assert!(
                !s.contains("\"createdCount\":0"),
                "wizard {wslug} created 0 shapes: {s}"
            );
        }

        // 5. CAD Scale Lock, Layer Conversion & Binary `.dwg` Export
        let mut sess = DocumentSession::new_blank("CAD Test");
        let mini_dxf = b"0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nWALLS\n10\n1.0\n20\n1.0\n11\n6.0\n21\n1.0\n0\nENDSEC\n0\nEOF\n";
        sess.open_bytes(mini_dxf, "dxf").expect("open dxf");
        let ratio = sess.apply_cad_scale(1, "1/4_in_ft", true).expect("cad scale");
        assert!((ratio - 48.0).abs() < 1e-6);
        let converted = sess.convert_cad_layers(1, "*").expect("convert cad");
        assert_eq!(converted, 1);
        let dwg_bytes = sess.save_format("dwg").expect("export dwg");
        assert!(dwg_bytes.starts_with(b"AC1015"));

        vexio_viewport_destroy(vp);
        vexio_doc_destroy(doc);
        vexio_engine_destroy(eng);
    }
}
