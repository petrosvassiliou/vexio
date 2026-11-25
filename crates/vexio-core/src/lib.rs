//! `vexio-core` — Shared Deterministic High-Performance Core Engine for Vexio.

pub mod autolayout;
pub mod automation;
pub mod boolean_ops;
pub mod containers;
pub mod crdt;
pub mod data_link;
pub mod domains;
pub mod enterprise_plan2;
pub mod export_print;
pub mod ffi;
pub mod journal;
pub mod plan2_connectors;
pub mod routing;
pub mod sqlite_reader;
pub mod themes_styles;
pub mod validation;
pub mod visio_artwork;
pub mod workspace;

pub use vexio_cad;
pub use vexio_render;
pub use vexio_shapesheet;
pub use vexio_vsd_legacy;
pub use vexio_vsdx;
pub use workspace::{CoreError, DocumentSession};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autolayout::{LayoutAlgorithm, LayoutOptions, apply_auto_layout};
    use crate::automation::{AutomationError, SandboxLimits, execute_vba_macro};
    use crate::containers::{
        attach_callout, build_cross_functional_swimlanes, create_container,
    };
    use crate::data_link::{
        DataGraphicItem, DataGraphicSpec, DataRecordset, apply_data_graphics,
        bind_recordset_to_page, generate_bom_shape_report, insert_page_legend,
        sync_shapes_back_to_recordset,
    };
    use crate::domains::{
        PertTaskInput, build_pert_critical_path_diagram, create_architectural_room_space,
        forward_engineer_sql_ddl, mount_rack_equipment, reverse_engineer_sql_ddl,
    };
    use crate::export_print::{
        encode_rgba_to_jpeg, encode_rgba_to_png, export_document_html_report, export_document_pdf,
        export_page_eps, export_page_jpeg, export_page_svg, plan_tiled_print,
    };
    use crate::routing::{ObstacleRect, orthogonal_astar_route, route_page_connectors};
    use crate::validation::{DiagramRuleSet, validate_document};
    use std::collections::BTreeMap;
    use std::ffi::CString;
    use vexio_shapesheet::*;

    #[test]
    fn orthogonal_router_avoids_obstacles_and_generates_line_jumps() {
        // Obstacle sits squarely at (5.0, 5.0) between (2.0, 5.0) and (8.0, 5.0)
        let obs = ObstacleRect {
            shape_id: 99,
            min_x: 4.0,
            min_y: 4.0,
            max_x: 6.0,
            max_y: 6.0,
        };
        let path = orthogonal_astar_route([2.0, 5.0], [8.0, 5.0], &[obs]);
        assert!(path.len() >= 4, "Path must detour orthogonally around obstacle");
        for seg in path.windows(2) {
            assert!(
                !obs.intersects_open_segment(seg[0], seg[1]),
                "Orthogonal segment {:?}->{:?} intersected obstacle!",
                seg[0],
                seg[1]
            );
        }

        // Verify crossing connectors emit ArcTo line jump in Geometry1
        let mut doc = VisioDocument::new_blank("LineJumps");
        {
            let page = doc.page_mut(1).unwrap();
            let v_conn = VisioShape::new_connector(1, "Vert", 5.0, 2.0, 5.0, 8.0);
            let h_conn = VisioShape::new_connector(2, "Horiz", 2.0, 5.0, 8.0, 5.0);
            page.shapes.push(v_conn);
            page.shapes.push(h_conn);
        }
        route_page_connectors(&mut doc, 1);
        let horiz = doc.page(1).unwrap().find_shape(2).unwrap();
        let has_arc_jump = horiz.sheet.geometries[0]
            .rows
            .iter()
            .any(|r| r.kind == GeometryRowKind::ArcTo);
        assert!(has_arc_jump, "Horizontal connector must emit ArcTo jump over vertical connector");
    }

    #[test]
    fn bpmn20_uml25_and_workflow_validators_detect_errors_and_accept_valid_graphs() {
        let mut sess = DocumentSession::new_blank("BPMN Check");
        let start_id = sess.drop_builtin_shape(1, "BPMN 2.0", "Start Event", 2.0, 5.0).unwrap();
        let orphan_task = sess.drop_builtin_shape(1, "BPMN 2.0", "Task", 5.0, 5.0).unwrap();
        let end_id = sess.drop_builtin_shape(1, "BPMN 2.0", "End Event", 8.0, 5.0).unwrap();

        // Orphaned task & disconnected start/end must raise BPMN validation errors
        let bad_issues = validate_document(&sess.doc, DiagramRuleSet::Bpmn20);
        assert!(!bad_issues.is_empty());

        // Wire `Start -> Task -> End`: BPMN 2.0 validation must now pass with 0 issues!
        sess.connect_shapes(1, start_id, orphan_task).unwrap();
        sess.connect_shapes(1, orphan_task, end_id).unwrap();
        let clean_issues = validate_document(&sess.doc, DiagramRuleSet::Bpmn20);
        assert_eq!(clean_issues.len(), 0, "Expected 0 BPMN issues, got: {clean_issues:?}");

        // UML circular inheritance detection
        let mut uml_doc = VisioDocument::new_blank("UML Cycle");
        {
            let p = uml_doc.page_mut(1).unwrap();
            let mut c1 = VisioShape::new_rect(1, "ClassA", 2.0, 4.0, 2.0, 1.0);
            c1.text = "ClassA".into();
            c1.sheet.set_user_cell("DomainRole", "\"uml_class\"", "");
            let mut c2 = VisioShape::new_rect(2, "ClassB", 6.0, 4.0, 2.0, 1.0);
            c2.text = "ClassB".into();
            c2.sheet.set_user_cell("DomainRole", "\"uml_class\"", "");
            let mut g1 = VisioShape::new_connector(3, "Generalization.1", 0.0, 0.0, 1.0, 0.0);
            g1.sheet.set_user_cell("DomainRole", "\"generalization\"", "");
            let mut g2 = VisioShape::new_connector(4, "Generalization.2", 0.0, 0.0, 1.0, 0.0);
            g2.sheet.set_user_cell("DomainRole", "\"generalization\"", "");
            p.shapes.extend([c1, c2, g1, g2]);
            p.connects.extend([
                GlueEndpoint { connector_shape_id: 3, from_cell: "BeginX".into(), target_shape_id: 1, to_cell: "PinX".into(), is_dynamic: true },
                GlueEndpoint { connector_shape_id: 3, from_cell: "EndX".into(), target_shape_id: 2, to_cell: "PinX".into(), is_dynamic: true },
                GlueEndpoint { connector_shape_id: 4, from_cell: "BeginX".into(), target_shape_id: 2, to_cell: "PinX".into(), is_dynamic: true },
                GlueEndpoint { connector_shape_id: 4, from_cell: "EndX".into(), target_shape_id: 1, to_cell: "PinX".into(), is_dynamic: true },
            ]);
        }
        let uml_issues = validate_document(&uml_doc, DiagramRuleSet::Uml25);
        assert!(uml_issues.iter().any(|i| i.rule_code == "UML-002"));
    }

    #[test]
    fn sql_erd_reverse_and_forward_engineering_and_pert_critical_path() {
        let mut doc = VisioDocument::new_blank("ERD & PERT");
        let ddl = "
            CREATE TABLE customers (
                id INT PRIMARY KEY,
                email VARCHAR NOT NULL
            );
            CREATE TABLE orders (
                id INT PRIMARY KEY,
                customer_id INT NOT NULL REFERENCES customers(id),
                total DECIMAL NOT NULL
            );
        ";
        let tables = reverse_engineer_sql_ddl(&mut doc, 1, ddl, true);
        assert_eq!(tables.len(), 2);
        let forward_sql = forward_engineer_sql_ddl(&doc, 1);
        assert!(forward_sql.contains("CREATE TABLE customers"));
        assert!(forward_sql.contains("FOREIGN KEY (customer_id) REFERENCES customers(id)"));

        // PERT CPM on Page 2
        doc.pages.push(VisioPage::new(2, "PERT", 11.0, 8.5));
        let pert = build_pert_critical_path_diagram(
            &mut doc,
            2,
            &[
                PertTaskInput { id: "A".into(), name: "Design".into(), duration_days: 4.0, predecessors: vec![] },
                PertTaskInput { id: "B".into(), name: "Rust Core".into(), duration_days: 6.0, predecessors: vec!["A".into()] },
                PertTaskInput { id: "C".into(), name: "Docs".into(), duration_days: 2.0, predecessors: vec!["A".into()] },
                PertTaskInput { id: "D".into(), name: "Ship".into(), duration_days: 3.0, predecessors: vec!["B".into(), "C".into()] },
            ],
        );
        let node_b = pert.iter().find(|n| n.id == "B").unwrap();
        let node_c = pert.iter().find(|n| n.id == "C").unwrap();
        assert!(node_b.is_critical);
        assert!(!node_c.is_critical);
        assert!((node_c.slack - 4.0).abs() < 1e-9);
    }

    #[test]
    fn two_way_xlsx_data_linking_data_graphics_swimlanes_rack_and_floorplan() {
        let mut doc = VisioDocument::new_blank("Enterprise Ops");
        build_cross_functional_swimlanes(&mut doc, 1, &["Network Ops", "Storage Tier"], 1.0, 8.0, 9.0, 3.0);
        {
            let p = doc.page_mut(1).unwrap();
            let mut s1 = VisioShape::new_rect(10, "SRV-01", 3.0, 6.5, 2.0, 1.0);
            s1.text = "SRV-01".into();
            let mut s2 = VisioShape::new_rect(11, "SRV-02", 6.0, 3.5, 2.0, 1.0);
            s2.text = "SRV-02".into();
            p.shapes.extend([s1, s2]);
        }
        crate::containers::sync_swimlane_shape_metadata(&mut doc, 1);
        let cont_id = create_container(&mut doc, 1, "Cluster Pod", &[10, 11], 0.3).unwrap();
        assert!(cont_id > 0);
        let callout_id = attach_callout(&mut doc, 1, 10, "Primary Ingress", 1.5, 1.0).unwrap();
        assert!(callout_id > 0);

        // Build `.xlsx` workbook in-memory, bind to shapes, apply Data Graphics, edit shape Prop, and sync back!
        let csv = "Host,CPU_Pct,Status\nSRV-01,85,Healthy\nSRV-02,42,Warning\n";
        let rs_initial = DataRecordset::from_csv(1, "Telemetry", "Host", csv).unwrap();
        let xlsx_bytes = rs_initial.to_xlsx_bytes().unwrap();
        let mut rs = DataRecordset::from_xlsx_bytes(1, "Telemetry", "Host", &xlsx_bytes).unwrap();

        let bound = bind_recordset_to_page(&mut doc, 1, &rs, None);
        assert_eq!(bound, 2);

        let mut palette = BTreeMap::new();
        palette.insert("Healthy".into(), RgbaColor { r: 220, g: 252, b: 231, a: 255 });
        palette.insert("Warning".into(), RgbaColor { r: 254, g: 243, b: 199, a: 255 });
        let dg = DataGraphicSpec {
            name: "Server Health".into(),
            items: vec![
                DataGraphicItem::ColorByValue { prop_name: "Status".into(), palette },
                DataGraphicItem::DataBar {
                    prop_name: "CPU_Pct".into(),
                    min_val: 0.0,
                    max_val: 100.0,
                    bar_color: RgbaColor { r: 37, g: 99, b: 235, a: 255 },
                },
                DataGraphicItem::IconSet { prop_name: "CPU_Pct".into(), green_min: 70.0, amber_min: 40.0 },
                DataGraphicItem::TextCallout { prop_name: "Status".into(), prefix: "State: ".into() },
            ],
        };
        assert_eq!(apply_data_graphics(&mut doc, 1, &dg), 2);
        assert!(insert_page_legend(&mut doc, 1, "Status", 9.0, 1.5).is_some());

        // Mutate SRV-02 CPU_Pct on shape and sync back to recordset
        doc.page_mut(1)
            .unwrap()
            .find_shape_mut(11)
            .unwrap()
            .sheet
            .set_formula("Prop.CPU_Pct", "91", Some(Unit::Number));
        ReactiveEngine::recalculate_document(&mut doc);
        assert_eq!(sync_shapes_back_to_recordset(&doc, 1, &mut rs), 1);
        assert_eq!(rs.rows[1].get("CPU_Pct").map(String::as_str), Some("91"));

        let bom = generate_bom_shape_report(&doc, &["Host", "CPU_Pct", "Function"]);
        assert!(bom.to_xlsx_bytes().unwrap().starts_with(b"PK\x03\x04"));

        // Verify 42U Rack & Architectural Room Space reactive area
        doc.pages.push(VisioPage::new(2, "Facilities", 11.0, 8.5));
        doc.page_mut(2).unwrap().page_sheet.set_formula("PageScale", "0.25 in", Some(Unit::Inch));
        doc.page_mut(2).unwrap().page_sheet.set_formula("DrawingScale", "12 in", Some(Unit::Inch)); // 1/4" = 1'-0"
        let room_id = create_architectural_room_space(&mut doc, 2, "Server Room", 4.0, 4.0, 2.5, 2.0).unwrap();
        // 2.5 in @ 1/4"=1' -> 10 ft; 2.0 in @ 1/4"=1' -> 8 ft => Area = 80 sq ft!
        let area = doc.page(2).unwrap().find_shape(room_id).unwrap().sheet.get_f64("Prop.AreaSqFt").unwrap();
        assert!((area - 80.0).abs() < 1e-6);

        let rack_id = doc.page(2).unwrap().next_shape_id();
        doc.page_mut(2).unwrap().shapes.push(VisioShape::new_rect(rack_id, "Rack", 8.0, 4.5, 2.2, 7.35));
        assert!(mount_rack_equipment(&mut doc, 2, rack_id, "CoreRouter", 20, 2).is_some());
    }

    #[test]
    fn sandboxed_vba_automation_exports_and_c_abi_roundtrip() {
        let mut doc = VisioDocument::new_blank("Automation & Exports");
        let vba = "
            Sub BuildFlow()
                For i = 1 To 3
                    Set shp = ActivePage.DrawRectangle(1.0 + i * 2.0, 4.0, 2.2 + i * 2.0, 4.8)
                    shp.Text = \"Step \" & i
                Next i
            End Sub
        ";
        let rep = execute_vba_macro(&mut doc, vba, SandboxLimits::default()).unwrap();
        assert_eq!(rep.shapes_created, 3);
        apply_auto_layout(&mut doc, 1, LayoutOptions { algorithm: LayoutAlgorithm::Circular, ..Default::default() });

        // Infinite loop in VBA MUST be stopped cleanly by InstructionQuota!
        let bad_vba = "
            Sub Runaway()
                For i = 1 To 100000
                    x = i * 2
                Next i
            End Sub
        ";
        let quota_err = execute_vba_macro(
            &mut doc,
            bad_vba,
            SandboxLimits { max_instructions: 200, max_created_shapes: 50 },
        );
        assert!(matches!(quota_err, Err(AutomationError::QuotaExceeded { .. })));

        // Exports: PDF/A, SVG, EPS, PNG, JPG (Baseline DCT JFIF), HTML, Tiled Print
        let pdf = export_document_pdf(&doc);
        assert!(pdf.starts_with(b"%PDF-1.7"));
        assert!(export_page_svg(&doc, 1).contains("<svg"));
        assert!(export_page_eps(&doc, 1).starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
        assert!(export_document_html_report(&doc).contains("<!DOCTYPE html>"));
        let png = encode_rgba_to_png(4, 4, &[255u8; 64], 300);
        assert!(png.starts_with(&[137, 80, 78, 71]));
        let mut sample_rgba = vec![255u8; 16 * 12 * 4];
        for px in sample_rgba.chunks_exact_mut(4).take(40) {
            px.copy_from_slice(&[37, 99, 235, 255]);
        }
        let jpg = encode_rgba_to_jpeg(16, 12, &sample_rgba, 192, 92);
        assert!(jpg.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0]), "JPEG must start with SOI + APP0");
        assert_eq!(&jpg[6..11], b"JFIF\0", "JPEG APP0 must carry JFIF identifier");
        assert_eq!(&jpg[jpg.len() - 2..], &[0xFF, 0xD9], "JPEG must end with EOI marker");
        let page_jpg = export_page_jpeg(&doc, 1, 96, 90);
        assert!(page_jpg.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0]));
        assert_eq!(&page_jpg[page_jpg.len() - 2..], &[0xFF, 0xD9]);
        let tiles = plan_tiled_print(22.0, 17.0, 10.5, 8.0, 0.5);
        assert!(tiles.len() >= 4);

        // C-ABI round-trip test (.vsdx + .jpg)
        unsafe {
            let eng = ffi::c_abi::vexio_engine_create();
            let title_c = CString::new("C-ABI Doc").unwrap();
            let cdoc = ffi::c_abi::vexio_doc_create_blank(title_c.as_ptr());
            assert!(!cdoc.is_null());

            let ext_jpg = CString::new("jpg").unwrap();
            let mut jpg_owned = ffi::c_abi::VexioOwnedBuffer { ptr: std::ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                ffi::c_abi::vexio_doc_save_format(cdoc, ext_jpg.as_ptr(), &mut jpg_owned),
                ffi::c_abi::VexioStatus::Ok
            );
            assert!(jpg_owned.len > 200);
            let jpg_slice = std::slice::from_raw_parts(jpg_owned.ptr, jpg_owned.len);
            assert!(jpg_slice.starts_with(&[0xFF, 0xD8, 0xFF, 0xE0]));
            assert_eq!(&jpg_slice[jpg_slice.len() - 2..], &[0xFF, 0xD9]);
            ffi::c_abi::vexio_buffer_free(jpg_owned);

            // Save to .vsdx through C-ABI, re-open through C-ABI, and render RGBA preview!
            let ext_vsdx = CString::new("vsdx").unwrap();
            let mut owned = ffi::c_abi::VexioOwnedBuffer { ptr: std::ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                ffi::c_abi::vexio_doc_save_format(cdoc, ext_vsdx.as_ptr(), &mut owned),
                ffi::c_abi::VexioStatus::Ok
            );
            assert!(owned.len > 100);

            let mut reopened: *mut ffi::c_abi::VexioDoc = std::ptr::null_mut();
            let slice = ffi::c_abi::VexioByteSlice { ptr: owned.ptr, len: owned.len };
            assert_eq!(
                ffi::c_abi::vexio_doc_open_bytes(eng, slice, ext_vsdx.as_ptr(), &mut reopened),
                ffi::c_abi::VexioStatus::Ok
            );
            ffi::c_abi::vexio_buffer_free(owned);

            let mut rgba_buf = ffi::c_abi::VexioOwnedBuffer { ptr: std::ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                ffi::c_abi::vexio_render_preview_rgba(reopened, 1, 64, 64, &mut rgba_buf),
                ffi::c_abi::VexioStatus::Ok
            );
            assert_eq!(rgba_buf.len, 64 * 64 * 4);
            ffi::c_abi::vexio_buffer_free(rgba_buf);

            ffi::c_abi::vexio_doc_destroy(reopened);
            ffi::c_abi::vexio_doc_destroy(cdoc);
            ffi::c_abi::vexio_engine_destroy(eng);
        }
    }

    #[test]
    fn twelve_stencil_packs_org_chart_gantt_timeline_and_eight_starter_templates() {
        use crate::domains::{
            GanttTaskInput, TimelineMilestone, build_all_builtin_stencils, build_gantt_chart,
            build_org_chart_from_csv, build_project_timeline, instantiate_starter_template,
        };

        // 1. Verify all 33 built-in stencil packs (750+ masters across Visio Pro & Awesome-Visio domains) round-trip cleanly
        let packs = build_all_builtin_stencils();
        assert_eq!(packs.len(), 33, "Expected 33 built-in domain stencil packs");
        for pack in &packs {
            assert!(!pack.masters.is_empty(), "Pack {} must have masters", pack.title);
            let mut sess = DocumentSession::new_blank(&pack.title);
            for m in &pack.masters {
                let sid = sess
                    .drop_builtin_shape(1, &pack.title, &m.name, 3.0, 4.0)
                    .unwrap_or_else(|e| panic!("Failed dropping {} / {}: {e}", pack.title, m.name));
                assert!(sid > 0);
            }
        }

        // 2. Org Chart CSV Wizard
        let mut org_doc = VisioDocument::new_blank("Org Chart");
        let csv = "Name,Title,Manager,Department\n\
                   Ada Lovelace,Chief Executive Officer,,Executive\n\
                   Alan Turing,VP Engineering,Ada Lovelace,Systems\n\
                   Grace Hopper,Compiler Director,Alan Turing,Compilers\n";
        let org_count = build_org_chart_from_csv(&mut org_doc, 1, csv);
        assert_eq!(org_count, 3);
        assert!(org_doc.page(1).unwrap().connects.len() >= 4);

        // 3. Gantt Schedule Builder
        let mut gantt_doc = VisioDocument::new_blank("Gantt");
        let gantt_count = build_gantt_chart(
            &mut gantt_doc,
            1,
            &[
                GanttTaskInput {
                    id: "1".into(),
                    name: "Spec".into(),
                    start_day: 0.0,
                    duration_days: 4.0,
                    percent_complete: 100.0,
                    depends_on: vec![],
                },
                GanttTaskInput {
                    id: "2".into(),
                    name: "Build".into(),
                    start_day: 4.0,
                    duration_days: 6.0,
                    percent_complete: 60.0,
                    depends_on: vec!["1".into()],
                },
            ],
        );
        assert_eq!(gantt_count, 2);
        assert!(gantt_doc.page(1).unwrap().shapes.len() >= 5);

        // 4. Executive Project Timeline Builder
        let mut tl_doc = VisioDocument::new_blank("Timeline");
        let tl_count = build_project_timeline(
            &mut tl_doc,
            1,
            "Release Roadmap",
            90.0,
            &[
                TimelineMilestone { day: 15.0, label: "Alpha".into(), is_major: true },
                TimelineMilestone { day: 60.0, label: "Beta".into(), is_major: false },
            ],
        );
        assert_eq!(tl_count, 2);
        assert!(tl_doc.page(1).unwrap().shapes.len() >= 5);

        // 5. All 8 Backstage Starter Templates + VSDX lossless round-trip
        for tid in [
            "bpmn_fulfillment",
            "cloud_microservices",
            "sql_ecommerce_erd",
            "executive_org_chart",
            "release_gantt",
            "project_timeline",
            "datacenter_floorplan",
            "blank",
        ] {
            let tdoc = instantiate_starter_template(tid);
            assert!(!tdoc.pages.is_empty(), "Template {tid} must have at least 1 page");
            if tid != "blank" {
                assert!(
                    !tdoc.page(1).unwrap().shapes.is_empty(),
                    "Template {tid} must populate starter shapes"
                );
            }
            let bytes = vexio_vsdx::write_vsdx(&tdoc, vexio_vsdx::VsdxPackageKind::Drawing).unwrap();
            let (reopened, kind) = vexio_vsdx::read_vsdx(&bytes).unwrap();
            assert_eq!(kind, vexio_vsdx::VsdxPackageKind::Drawing);
            assert_eq!(reopened.pages.len(), tdoc.pages.len());
        }
    }

    #[test]
    fn vba_userforms_events_selection_and_cropped_foreign_image_svg_roundtrip() {
        use crate::automation::{
            VbaDocumentEvent, execute_vba_macro_with_selection, fire_vba_document_event,
        };

        let mut doc = VisioDocument::new_blank("VBA Advanced & Foreign Image");
        {
            let p = doc.page_mut(1).unwrap();
            let mut s1 = VisioShape::new_rect(1, "ServerA", 3.0, 4.0, 2.0, 1.2);
            s1.text = "Alpha".into();
            let mut s2 = VisioShape::new_rect(2, "ServerB", 6.0, 4.0, 2.0, 1.2);
            s2.text = "Beta".into();
            s2.foreign_data = Some(ForeignMediaData {
                media_type: "PNG".into(),
                rel_id: None,
                data: encode_rgba_to_png(2, 2, &[255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255], 96),
            });
            s2.sheet.set_formula("CropLeft", "0.1 in", Some(Unit::Inch));
            s2.sheet.set_formula("Brightness", "1.15", Some(Unit::Number));
            s2.sheet.set_formula("Contrast", "1.10", Some(Unit::Number));
            s2.sheet.set_formula("FillPattern", "28", Some(Unit::Number)); // Linear vertical gradient
            s2.sheet.set_formula("ShdwPattern", "1", Some(Unit::Number)); // Soft drop shadow
            s2.sheet.set_formula("ExtrusionDepth", "0.12 in", Some(Unit::Inch)); // 3D extrusion bevel
            s2.sheet.set_formula("LineSketchEnabled", "TRUE", Some(Unit::Boolean));
            s2.text = "**Beta** *Node*\n- __Primary__".into();
            p.shapes.extend([s1, s2]);
        }
        ReactiveEngine::recalculate_document(&mut doc);

        let vba_module = r#"
            Begin UserForm frmConfig
                Caption = "Cluster Deployment"
                Control TextBox txtEnv = "Production"
                Control CheckBox chkHA = "TRUE"
            End

            Sub ApplySelectionConfig()
                frmConfig.Show
                ActiveWindow.Zoom = 1.25
                For Each shp In ActiveWindow.Selection
                    If shp.Name = "ServerA" Then
                        shp.Text = frmConfig.txtEnv.Text & " Primary"
                    Else
                        shp.Text = "Standby"
                    End If
                Next shp
            End Sub

            Private Sub Document_ShapeAdded(ByVal Shape As IVShape)
                Shape.Cells("FillForegnd").FormulaU = "RGB(220,252,231)"
                Debug.Print "Added:" & Shape.Name
            End Sub
        "#;

        let rep = execute_vba_macro_with_selection(
            &mut doc,
            vba_module,
            &[1],
            None,
            SandboxLimits::default(),
        )
        .unwrap();
        assert_eq!(rep.user_forms.len(), 1);
        assert!(rep.user_forms[0].shown);
        assert_eq!(rep.window_zoom, Some(1.25));
        assert_eq!(doc.page(1).unwrap().find_shape(1).unwrap().text, "Production Primary");

        // Fire Document_ShapeAdded event sink on shape 2
        let ev_rep = fire_vba_document_event(
            &mut doc,
            vba_module,
            VbaDocumentEvent::ShapeAdded {
                page_id: 1,
                shape_id: 2,
            },
            SandboxLimits::default(),
        )
        .unwrap();
        assert_eq!(ev_rep.log_output, vec!["Added:ServerB"]);

        // Verify SVG renders gradient, shadow filter, 3D extrusion polygon, cropped <image>, and bold/italic/underline <tspan>
        let svg = export_page_svg(&doc, 1);
        assert!(svg.contains("<linearGradient"));
        assert!(svg.contains("<feDropShadow"));
        assert!(svg.contains("clip-path=\"url(#vexio-imgclip-fg-2)\""));
        assert!(svg.contains("font-weight=\"bold\""));
        assert!(svg.contains("font-style=\"italic\""));
        assert!(svg.contains("text-decoration=\"underline\""));

        // Verify ForeignMediaData round-trips losslessly through .vsdx
        let vsdx_bytes = vexio_vsdx::write_vsdx(&doc, vexio_vsdx::VsdxPackageKind::Drawing).unwrap();
        let (rt_doc, _) = vexio_vsdx::read_vsdx(&vsdx_bytes).unwrap();
        assert!(rt_doc.page(1).unwrap().find_shape(2).unwrap().foreign_data.is_some());
    }

    #[test]
    fn all_builtin_stencils_resize_freely_and_vssx_import_roundtrips() {
        let packs = crate::domains::build_all_builtin_stencils();
        assert_eq!(packs.len(), 33);
        let mut total_tested = 0usize;
        for pack in &packs {
            let mut sess = DocumentSession::new_blank(&pack.title);
            for m in &pack.masters {
                let sid = sess
                    .drop_builtin_shape(1, &pack.title, &m.name, 5.0, 4.0)
                    .unwrap();
                // Resize down to 0.45 x 0.40 in and up to 3.80 x 2.60 in
                sess.transform_shape(1, sid, 4.2, 3.8, 0.45, 0.40, 0.0).unwrap();
                let sh_small = sess.doc.page(1).unwrap().find_shape(sid).unwrap();
                assert!(
                    (sh_small.sheet.get_f64("Width").unwrap() - 0.45).abs() < 1e-6,
                    "Width failed to resize on {}:{}",
                    pack.title,
                    m.name
                );
                assert!(
                    (sh_small.sheet.get_f64("Height").unwrap() - 0.40).abs() < 1e-6,
                    "Height failed to resize on {}:{}",
                    pack.title,
                    m.name
                );

                sess.transform_shape(1, sid, 5.5, 4.5, 3.80, 2.60, 15.0).unwrap();
                let sh_big = sess.doc.page(1).unwrap().find_shape(sid).unwrap();
                assert!((sh_big.sheet.get_f64("Width").unwrap() - 3.80).abs() < 1e-6);
                assert!((sh_big.sheet.get_f64("Height").unwrap() - 2.60).abs() < 1e-6);
                total_tested += 1;
            }
            // Clear page between packs? Actually each pack uses its own fresh `sess`!
        }
        assert!(total_tested >= 715);

        // Verify .vssx stencil package export & re-import via `import_stencil_package`
        let mut vssx_doc = VisioDocument::new_blank("Custom M365 Stencil");
        vssx_doc.masters = packs[2].masters.iter().take(4).cloned().collect();
        let vssx_bytes =
            vexio_vsdx::write_vsdx(&vssx_doc, vexio_vsdx::VsdxPackageKind::Stencil).unwrap();

        let mut target_sess = DocumentSession::new_blank("Target");
        let (imported_title, previews) = target_sess
            .import_stencil_package("Custom_M365_Pack.vssx", &vssx_bytes)
            .unwrap();
        assert_eq!(imported_title, "Custom M365 Pack");
        assert_eq!(previews.len(), 4);
        let dropped = target_sess
            .drop_builtin_shape(1, &imported_title, &previews[0].name, 3.0, 3.0)
            .unwrap();
        target_sess
            .transform_shape(1, dropped, 3.0, 3.0, 2.9, 2.1, 0.0)
            .unwrap();
        let sh = target_sess.doc.page(1).unwrap().find_shape(dropped).unwrap();
        assert!((sh.sheet.get_f64("Width").unwrap() - 2.9).abs() < 1e-6);
    }

    #[test]
    fn pillar4_live_data_graphics_legends_and_two_way_data_linking_roundtrip() {
        let mut sess = DocumentSession::new_blank("Enterprise Infrastructure");
        let s1 = sess
            .drop_builtin_shape(1, "Basic Shapes", "Rectangle", 2.4, 4.5)
            .unwrap();
        let s2 = sess
            .drop_builtin_shape(1, "Basic Shapes", "Rectangle", 5.1, 4.5)
            .unwrap();
        let s3 = sess
            .drop_builtin_shape(1, "Basic Shapes", "Rectangle", 7.8, 4.5)
            .unwrap();
        sess.set_shape_text(1, s1, "Exchange-01").unwrap();
        sess.set_shape_text(1, s2, "Entra-Connect").unwrap();
        sess.set_shape_text(1, s3, "Sentinel-Collector").unwrap();

        // Untouched .vsdx must NOT contain `vexio_data_state.json`
        let clean_vsdx = sess.save_format("vsdx").unwrap();
        {
            let cursor = std::io::Cursor::new(&clean_vsdx);
            let mut zip = zip::ZipArchive::new(cursor).unwrap();
            assert!(zip.by_name("visio/data/vexio_data_state.json").is_err());
        }

        // Build initial CSV -> export to .xlsx, .sqlite, .json -> verify multi-format auto-import!
        let seed_csv = "\
Label,Status,CompletionPct,Owner
Exchange-01,Healthy,92,Alice
Entra-Connect,Warning,54,Bob
Sentinel-Collector,Critical,18,Clara
Unplaced-Firewall,Healthy,100,Dan
";
        let seed_rs =
            crate::data_link::DataRecordset::from_csv(1, "Infra", "Label", seed_csv).unwrap();
        let xlsx_bytes = seed_rs.export_bytes("xlsx", "Infra").unwrap();
        let sqlite_bytes = seed_rs.export_bytes("sqlite", "infra_nodes").unwrap();
        let json_bytes = seed_rs.export_bytes("json", "Infra").unwrap();
        assert!(sqlite_bytes.starts_with(b"SQLite format 3\0"));

        // Verify .sqlite and .json auto-parsers decode all 4 rows cleanly
        let (parsed_sqlite, sqlite_kind) = crate::data_link::DataRecordset::from_auto_bytes(
            1,
            "Nodes",
            "nodes.sqlite",
            &sqlite_bytes,
            "Label",
            "",
        )
        .unwrap();
        assert_eq!(sqlite_kind, "sqlite");
        assert_eq!(parsed_sqlite.rows.len(), 4);
        let (parsed_json, json_kind) = crate::data_link::DataRecordset::from_auto_bytes(
            1,
            "Nodes",
            "nodes.json",
            &json_bytes,
            "Label",
            "",
        )
        .unwrap();
        assert_eq!(json_kind, "json");
        assert_eq!(parsed_json.rows.len(), 4);

        // Import .xlsx recordset onto page 1 with auto-bind & live Data Graphics
        let (rs_id, linked) = sess
            .import_external_recordset(1, "Infra_Inventory.xlsx", &xlsx_bytes, "Label", "", true, true)
            .unwrap();
        assert_eq!(rs_id, 1);
        assert_eq!(linked, 3);

        // Insert live Diagram Legend for `Status`
        let legend_id = sess
            .insert_or_update_data_legend(1, "Status", Some(9.2), Some(2.4))
            .unwrap();
        assert!(legend_id > 0);

        // Verify EvaluatedShapeVisual.data_graphics on Exchange-01 (CheckOk, 92%, Alice) & Sentinel-Collector (ErrorCross, 18%, Clara)
        let visuals = vexio_shapesheet::ReactiveEngine::evaluate_page_visuals(&sess.doc, 1);
        let v1 = visuals.iter().find(|v| v.shape_id == s1).unwrap();
        let dg1 = v1.data_graphics.as_ref().expect("Data graphics overlay present");
        assert_eq!(
            dg1.icon_badge.as_ref().unwrap().kind,
            vexio_shapesheet::DataGraphicIconKind::CheckOk
        );
        assert!((dg1.data_bar.as_ref().unwrap().ratio - 0.92).abs() < 1e-4);
        assert!(dg1.text_callout.as_ref().unwrap().text.contains("Alice"));
        assert!(dg1.color_by_value_applied.is_some());

        let v3 = visuals.iter().find(|v| v.shape_id == s3).unwrap();
        let dg3 = v3.data_graphics.as_ref().unwrap();
        assert_eq!(
            dg3.icon_badge.as_ref().unwrap().kind,
            vexio_shapesheet::DataGraphicIconKind::ErrorCross
        );
        assert!((dg3.data_bar.as_ref().unwrap().ratio - 0.18).abs() < 1e-4);

        // Verify GPU/software vector tessellation emits triangles for badges, bars & callouts
        let mesh1 = vexio_render::tessellate_shape_full(v1, 0.005);
        assert!(mesh1.indices.len() >= 36);

        // Verify SVG & PDF exports include live Data Graphics & Legend
        let svg_out = sess.save_format("svg").unwrap();
        let svg_str = std::str::from_utf8(&svg_out).unwrap();
        assert!(svg_str.contains("class=\"vexio-data-graphic\""));
        assert!(svg_str.contains("92"));
        assert!(svg_str.contains("Healthy: 1"));
        let pdf_out = sess.save_format("pdf").unwrap();
        assert!(pdf_out.starts_with(b"%PDF-"));
        assert!(pdf_out.len() > 800);

        // Stamp & bind row #3 ("Unplaced-Firewall") when `shape_ids` is empty
        let stamped = sess.bind_external_row_to_shapes(1, rs_id, 3, &[]).unwrap();
        assert_eq!(stamped, 1);

        // Live reactivity: change Sentinel-Collector's Prop.Status from "Critical" -> "Healthy" and CompletionPct -> "100"
        sess.upsert_shape_data(1, s3, "Status", "Status", 0, "Healthy")
            .unwrap();
        sess.upsert_shape_data(1, s3, "CompletionPct", "CompletionPct", 2, "100")
            .unwrap();

        let visuals_after = vexio_shapesheet::ReactiveEngine::evaluate_page_visuals(&sess.doc, 1);
        let v3_after = visuals_after.iter().find(|v| v.shape_id == s3).unwrap();
        assert_eq!(
            v3_after
                .data_graphics
                .as_ref()
                .unwrap()
                .icon_badge
                .as_ref()
                .unwrap()
                .kind,
            vexio_shapesheet::DataGraphicIconKind::CheckOk
        );
        // Legend auto-recalculated: Healthy now has 3 shapes (Exchange-01 + Unplaced-Firewall + Sentinel-Collector)!
        let legend_sh = sess.doc.page(1).unwrap().find_shape(legend_id).unwrap();
        assert!(
            legend_sh.text.contains("Healthy: 3"),
            "Expected Legend to auto-update to Healthy: 3, got: {}",
            legend_sh.text
        );

        // Two-Way Writeback Export: push canvas Prop.* edits back to .xlsx, .sqlite, .json, and .csv!
        for fmt in ["xlsx", "sqlite", "json", "csv"] {
            let exported = sess.sync_and_export_recordset(1, rs_id, fmt).unwrap();
            let (rt_rs, _) = crate::data_link::DataRecordset::from_auto_bytes(
                1,
                "Writeback",
                &format!("writeback.{fmt}"),
                &exported,
                "Label",
                "",
            )
            .unwrap();
            let sentinel_row = rt_rs
                .rows
                .iter()
                .find(|r| r.get("Label").map(String::as_str) == Some("Sentinel-Collector"))
                .unwrap();
            assert_eq!(sentinel_row.get("Status").map(String::as_str), Some("Healthy"));
            assert_eq!(
                sentinel_row.get("CompletionPct").map(String::as_str),
                Some("100")
            );
        }

        // Lossless .vsdx persistence round-trip of recordsets, page Data Graphics, and shape row bindings!
        let saved_vsdx = sess.save_format("vsdx").unwrap();
        let mut reopened = DocumentSession::new_blank("Reopened");
        reopened.open_bytes(&saved_vsdx, "vsdx").unwrap();
        assert_eq!(reopened.doc.data_recordsets.len(), 1);
        let rt_page = reopened.doc.page(1).unwrap();
        assert!(rt_page.data_graphic_config.as_ref().unwrap().enabled);
        assert_eq!(rt_page.find_shape(s1).unwrap().bound_data_row, Some((1, 0)));

        // Undo/Redo journal check + Auto-discovered BOM report check
        assert!(sess.journal.undo(&mut sess.doc).is_ok());
        assert!(sess.journal.redo(&mut sess.doc).is_ok());
        let bom_csv = String::from_utf8(sess.export_bom_report_bytes("csv").unwrap()).unwrap();
        assert!(bom_csv.contains("Exchange-01"));
        assert!(bom_csv.contains("Owner"));
    }
}
