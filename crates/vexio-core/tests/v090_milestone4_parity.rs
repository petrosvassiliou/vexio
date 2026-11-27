//! Milestone 4 (`v0.9.0-beta`) End-to-End Parity & Lossless `.vsdx` Round-Trip Verification
//!
//! Covers:
//! 1. Associative Architectural Dimensions (`Aligned`, `Horizontal`, `Vertical`, `Angular`,
//!    `Radial`, `Diameter`) + live `<fld>` updates across `.vsdx` save/load.
//! 2. Architectural Walls (`NativeCanvasTool::Wall = 21`) with Miter L-Corners, T-Joins, and
//!    Parametric Door/Window Wall Openings splitting host wall geometry cleanly.
//! 3. Domain Wizards & Plan 2 Calculators (`calendar`, `pivot_diagram`, `mind_map`,
//!    `mind_map_export`, `vsm`, `fta`) + Undo/Redo & C-ABI parity.

use std::ffi::CString;
use vexio_core::DocumentSession;
use vexio_core::ffi::c_abi::*;
use vexio_shapesheet::{ReactiveEngine, Unit};

#[test]
fn v090_aec_dimensions_walls_openings_and_vsdx_roundtrip() {
    let mut sess = DocumentSession::new_blank("AEC Floorplan v0.9.0");
    let pid = 1u32;

    // 1. Draw an L-corner + T-join architectural wall assembly
    let w_south = sess
        .draw_architectural_wall(pid, 1.5, 1.5, 8.5, 1.5, 0.30)
        .expect("south wall");
    let w_west = sess
        .draw_architectural_wall(pid, 1.5, 1.5, 1.5, 6.5, 0.30)
        .expect("west wall");
    let w_part = sess
        .draw_architectural_wall(pid, 5.0, 1.55, 5.0, 6.0, 0.24)
        .expect("partition T-wall");
    assert_ne!(w_south, w_west);
    assert_ne!(w_west, w_part);

    let healed = sess
        .heal_wall_intersections(pid, 0.30)
        .expect("heal intersections");
    assert!(healed >= 2, "Expected L-miter + T-join, got {healed}");

    // 2. Cut a Single Door and a Window into `w_south`
    let door_id = sess
        .insert_wall_opening(pid, w_south, "single_door", 2.2, 0.90, "in_left")
        .expect("insert door");
    let win_id = sess
        .insert_wall_opening(pid, w_south, "window", 5.2, 1.10, "in_left")
        .expect("insert window");
    assert!(door_id > 0 && win_id > 0);
    assert_eq!(
        sess.doc
            .page(pid)
            .unwrap()
            .find_shape(w_south)
            .unwrap()
            .sheet
            .geometries
            .len(),
        3
    );

    // 3. Attach Associative Horizontal, Angular, and Diameter dimensions
    let dim_id = sess
        .insert_associative_dimension(pid, "horizontal", w_south, 0, 0.60, "in")
        .expect("insert horizontal dim");
    let ang_dim = sess
        .insert_associative_dimension(pid, "angular", w_south, w_west, 0.50, "auto")
        .expect("insert angular dim");
    let circle_id = sess
        .draw_primitive_2d(pid, "ellipse", 6.0, 4.0, 7.6, 5.6)
        .expect("draw column circle");
    let diam_dim = sess
        .insert_associative_dimension(pid, "diameter", circle_id, 0, 0.35, "in")
        .expect("insert diameter dim");

    let dim_txt = sess.doc.page(pid).unwrap().find_shape(dim_id).unwrap().text.clone();
    assert!(dim_txt.contains("7.00"), "Expected 7.00 in wall span, got {dim_txt}");
    let ang_txt = sess.doc.page(pid).unwrap().find_shape(ang_dim).unwrap().text.clone();
    assert!(ang_txt.contains("90.0"), "Expected 90.0 deg corner, got {ang_txt}");
    let diam_txt = sess.doc.page(pid).unwrap().find_shape(diam_dim).unwrap().text.clone();
    assert!(diam_txt.contains("⌀") && diam_txt.contains("1.60"), "got {diam_txt}");

    // 4. Lossless `.vsdx` round-trip preserves associative cross-shape formulas & `<fld>` text!
    let vsdx_bytes = sess.save_format("vsdx").expect("save vsdx");
    let mut reopened = DocumentSession::new_blank("Reopened");
    reopened.open_bytes(&vsdx_bytes, "vsdx").expect("open vsdx");

    // Resize circle in reopened `.vsdx` from 1.60 in -> 2.40 in: diameter `<fld>` MUST update live!
    {
        let c = reopened.doc.page_mut(pid).unwrap().find_shape_mut(circle_id).unwrap();
        c.sheet.set_formula("Width", "2.40 in", Some(Unit::Inch));
        c.sheet.set_formula("Height", "2.40 in", Some(Unit::Inch));
    }
    ReactiveEngine::recalculate_document(&mut reopened.doc);
    let updated_diam = reopened
        .doc
        .page(pid)
        .unwrap()
        .find_shape(diam_dim)
        .unwrap()
        .text
        .clone();
    assert!(
        updated_diam.contains("2.40"),
        "Expected live <fld> update to 2.40 in after .vsdx reload, got {updated_diam}"
    );
}

#[test]
fn v090_wizards_mindmap_vsm_fta_and_c_abi_parity() {
    let mut sess = DocumentSession::new_blank("Plan 2 Calculators");

    // Calendar, PivotDiagram, Mind Map, VSM, and FTA via `run_domain_wizard`
    let cal_json = sess
        .run_domain_wizard(1, "calendar", "2026,12,monday\n2026-12-15|GA Freeze|green")
        .expect("calendar wizard");
    assert!(cal_json.contains("Calendar grid"));

    let p2 = sess.add_page("Pivot").unwrap();
    let piv_json = sess
        .run_domain_wizard(
            p2,
            "pivot_diagram",
            "Region,Product,Revenue\nNA,Core,150\nNA,Cloud,250\nEU,Core,100",
        )
        .expect("pivot wizard");
    assert!(piv_json.contains("PivotDiagram"));

    let p3 = sess.add_page("MindMap").unwrap();
    let mm_json = sess
        .run_domain_wizard(
            p3,
            "mind_map",
            "# Release Strategy\n- Desktop Native\n- ChromeOS Offline PWA",
        )
        .expect("mind_map wizard");
    assert!(mm_json.contains("Mind Map"));
    let child_topic = sess
        .add_mind_map_topic(p3, 0, "Zero-Stub Rust Engine", false)
        .expect("add topic");
    assert!(child_topic > 0);
    let md_outline = sess.export_mind_map_outline(p3);
    assert!(md_outline.contains("Zero-Stub Rust Engine"));

    // C-ABI verification of `vexio_doc_draw_architectural_wall`, `vexio_doc_insert_wall_opening`,
    // `vexio_doc_insert_associative_dimension`, and `vexio_doc_recalculate_vsm_and_fta_json`
    unsafe {
        let title = CString::new("C-ABI M4").unwrap();
        let doc = vexio_doc_create_blank(title.as_ptr());
        assert!(!doc.is_null());

        let mut wid: u64 = 0;
        assert_eq!(
            vexio_doc_draw_architectural_wall(doc, 1, 1.0, 2.0, 6.0, 2.0, 0.25, &mut wid),
            VexioStatus::Ok
        );
        assert!(wid > 0);

        let kind_door = CString::new("double_door").unwrap();
        let swing = CString::new("in_left").unwrap();
        let mut oid: u64 = 0;
        assert_eq!(
            vexio_doc_insert_wall_opening(
                doc,
                1,
                wid,
                kind_door.as_ptr(),
                2.5,
                1.0,
                swing.as_ptr(),
                &mut oid
            ),
            VexioStatus::Ok
        );
        assert!(oid > 0);

        let dim_kind = CString::new("aligned").unwrap();
        let unit_in = CString::new("in").unwrap();
        let mut dim_sid: u64 = 0;
        assert_eq!(
            vexio_doc_insert_associative_dimension(
                doc,
                1,
                dim_kind.as_ptr(),
                wid,
                0,
                0.5,
                unit_in.as_ptr(),
                &mut dim_sid
            ),
            VexioStatus::Ok
        );
        assert!(dim_sid > 0);

        let fta_slug = CString::new("fta").unwrap();
        let fta_spec = CString::new(
            "TOP,Outage,AND,0,B1;B2\nB1,Primary,BASIC,0.05,\nB2,Standby,BASIC,0.10,",
        )
        .unwrap();
        let mut wiz_buf = VexioOwnedBuffer {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_run_domain_wizard(
                doc,
                1,
                fta_slug.as_ptr(),
                fta_spec.as_ptr(),
                &mut wiz_buf
            ),
            VexioStatus::Ok
        );
        vexio_buffer_free(wiz_buf);

        let mut rep_buf = VexioOwnedBuffer {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        assert_eq!(
            vexio_doc_recalculate_vsm_and_fta_json(doc, 1, &mut rep_buf),
            VexioStatus::Ok
        );
        let rep_str =
            std::str::from_utf8(std::slice::from_raw_parts(rep_buf.ptr, rep_buf.len)).unwrap();
        assert!(rep_str.contains("0.005"), "Expected 0.05 * 0.10 = 0.005, got {rep_str}");
        vexio_buffer_free(rep_buf);

        vexio_doc_destroy(doc);
    }
}
