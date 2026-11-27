//! `vexio-vsdx` — Lossless OpenXML Visio & Stencil Compatibility Engine.
//!
//! Supports `.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, and `.vstm` with full
//! round-trip preservation of ShapeSheets, Masters, Background Pages, Layers,
//! Headers/Footers, Comments, VBA macro streams (`vbaProject.bin`), and custom OPC parts.

pub mod opc;
pub mod reader;
pub mod writer;

pub use opc::{RawOpcArchive, VsdxError, VsdxPackageKind};
pub use reader::{read_stencil_pack, read_vsdx};
pub use writer::write_vsdx;

#[cfg(test)]
mod tests {
    use super::*;
    use vexio_shapesheet::*;

    #[test]
    fn lossless_vsdx_vsdm_vssx_roundtrip_preserves_formulas_masters_layers_vba_and_opaque_parts() {
        let mut doc = VisioDocument::new_blank("Enterprise Network Topology");
        doc.properties.creator = "Principal Architect".into();
        doc.properties.company = "Vexio Corp".into();
        doc.properties
            .custom_properties
            .insert("CostCenter".into(), "ENG-900".into());
        doc.header_footer.header_center = "CONFIDENTIAL ARCHITECTURE".into();
        doc.header_footer.footer_right = "Page &p of &P".into();
        doc.macro_enabled = true;
        // Inject a binary `visio/vbaproject.bin` + custom XML part into `opaque_bag` to test byte-exact preservation!
        let fake_vba_bytes = b"VBA_PROJECT_STREAM_BINARY_PAYLOAD_0123456789".to_vec();
        doc.opaque_bag
            .parts
            .insert("visio/vbaproject.bin".into(), fake_vba_bytes.clone());
        doc.opaque_bag.parts.insert(
            "customXml/item1.xml".into(),
            b"<enterpriseMeta id=\"42\"/>".to_vec(),
        );
        doc.opaque_bag.content_type_overrides.insert(
            "/customXml/item1.xml".into(),
            "application/xml".into(),
        );

        // Add a custom Stencil Master
        let mut master_shape = VisioShape::new_rect(1, "CoreSwitch", 0.0, 0.0, 2.5, 1.2);
        master_shape
            .sheet
            .set_user_cell("Ports", "48", "Switch port count");
        doc.masters.push(VisioMaster {
            id: 1,
            name: "CoreSwitch".into(),
            name_u: "CoreSwitch".into(),
            base_id: "{A111-2222}".into(),
            unique_id: "{B333-4444}".into(),
            prompt: "48-Port Core Switch".into(),
            icon_bytes: None,
            page_sheet: ShapeSheet::new(),
            shapes: vec![master_shape],
        });

        // Add a Background Page + Foreground Page linked to Background Page
        let mut bg_page = VisioPage::new(2, "TitleBlock-BG", 11.0, 8.5);
        bg_page.is_background = true;
        doc.pages.push(bg_page);
        doc.page_mut(1).unwrap().back_page_id = Some(2);

        // Drop Master instance + Shape with GUARD, Controls, Shape Data, Hyperlink, Unknown XML, and 1D Connector Glue
        let sw_id = doc.drop_master(1, 1, 3.0, 5.0).unwrap();
        {
            let page = doc.page_mut(1).unwrap();
            let sw = page.find_shape_mut(sw_id).unwrap();
            sw.text = "Core-SW-01".into();
            sw.layer_membership = vec![0];
            sw.sheet.set_formula("Width", "=GUARD(3 in)", Some(Unit::Inch));
            sw.sheet.set_shape_data(ShapeDataRow {
                name: "AssetTag".into(),
                label: "Asset Tag".into(),
                prompt: "Inventory ID".into(),
                data_type: ShapeDataType::String,
                format: "".into(),
                value: "\"NET-0042\"".into(),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
            sw.opaque_xml_fragments
                .push("<extLst><ext uri=\"{9999}\"/></extLst>".into());

            let mut db = VisioShape::new_rect(2, "ReplicaDB", 8.0, 5.0, 2.0, 1.5);
            db.text = "Primary SQL".into();
            db.sheet
                .set_formula("Height", "=Sheet.1!Width * 0.5", Some(Unit::Inch));
            page.shapes.push(db);

            let conn = VisioShape::new_connector(3, "Dynamic connector", 4.5, 5.0, 7.0, 5.0);
            page.shapes.push(conn);
            page.connects.push(GlueEndpoint {
                connector_shape_id: 3,
                from_cell: "BeginX".into(),
                target_shape_id: sw_id,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
            page.connects.push(GlueEndpoint {
                connector_shape_id: 3,
                from_cell: "EndX".into(),
                target_shape_id: 2,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
        }

        doc.comments.push(CommentAnnotation {
            id: 1,
            author: "Alice Chen".into(),
            initials: "AC".into(),
            timestamp_iso: "2026-11-20T10:00:00Z".into(),
            page_id: 1,
            shape_id: Some(sw_id),
            x_in: 3.0,
            y_in: 5.0,
            text: "Verify redundant uplink capacity.".into(),
            resolved: false,
        });

        ReactiveEngine::recalculate_document(&mut doc);

        // Serialize to .vsdm and read back!
        let bytes = write_vsdx(&doc, VsdxPackageKind::MacroDrawing).unwrap();
        let (reloaded, kind) = read_vsdx(&bytes).unwrap();

        assert_eq!(kind, VsdxPackageKind::MacroDrawing);
        assert!(reloaded.macro_enabled);
        assert_eq!(
            reloaded.opaque_bag.parts.get("visio/vbaproject.bin"),
            Some(&fake_vba_bytes)
        );
        assert_eq!(
            reloaded.opaque_bag.parts.get("customXml/item1.xml"),
            Some(&b"<enterpriseMeta id=\"42\"/>".to_vec())
        );
        assert_eq!(reloaded.properties.title, "Enterprise Network Topology");
        assert_eq!(
            reloaded.properties.custom_properties.get("CostCenter"),
            Some(&"ENG-900".to_string())
        );
        assert_eq!(reloaded.header_footer.header_center, "CONFIDENTIAL ARCHITECTURE");
        assert_eq!(reloaded.masters.len(), 1);
        assert_eq!(reloaded.pages.len(), 2);
        assert!(reloaded.page(2).unwrap().is_background);
        assert_eq!(reloaded.page(1).unwrap().back_page_id, Some(2));
        assert_eq!(reloaded.comments.len(), 1);

        let p1 = reloaded.page(1).unwrap();
        let sw = p1.find_shape(sw_id).unwrap();
        assert_eq!(sw.text, "Core-SW-01");
        assert!(sw.sheet.get_cell("Width").unwrap().is_guarded());
        assert!((sw.sheet.get_f64("Width").unwrap() - 3.0).abs() < 1e-9);
        assert_eq!(
            sw.sheet.get_cell("Prop.AssetTag").unwrap().value,
            CellValue::Str("NET-0042".into())
        );
        assert_eq!(sw.opaque_xml_fragments.len(), 1);

        let db = p1.find_shape(2).unwrap();
        assert!((db.sheet.get_f64("Height").unwrap() - 1.5).abs() < 1e-9);
        assert_eq!(p1.connects.len(), 2);

        // Also verify `.vssx` StencilPack export & reload!
        let stencil_bytes = write_vsdx(&reloaded, VsdxPackageKind::Stencil).unwrap();
        let pack = read_stencil_pack(&stencil_bytes).unwrap();
        assert_eq!(pack.masters.len(), 1);
        assert_eq!(pack.masters[0].name, "CoreSwitch");
    }

    #[test]
    fn vsdx_actions_and_connection_points_roundtrip_losslessly() {
        let mut doc = VisioDocument::new_blank("ActionsRoundTrip");
        let mut s = VisioShape::new_rect(1, "SmartValve", 4.0, 4.0, 2.0, 1.5);
        s.sheet.set_user_cell("IsOpen", "1", "Valve state");
        s.sheet.add_action(ActionRow {
            name: "ToggleValve".into(),
            action: "=SETF(GetRef(User.IsOpen), IF(User.IsOpen, 0, 1))".into(),
            menu: "\"Toggle Valve\"".into(),
            tag_name: "ValveToggle".into(),
            checked: "=User.IsOpen".into(),
            disabled: "0".into(),
            invisible: "0".into(),
            begin_group: true,
        });
        s.sheet.add_connection_point(ConnectionPointRow {
            name: "X1".into(),
            ix: 1,
            x: "=Width*0".into(),
            y: "=Height*0.5".into(),
            dir_x: "-1".into(),
            dir_y: "0".into(),
            point_type: 0,
        });
        doc.page_mut(1).unwrap().shapes.push(s);
        ReactiveEngine::recalculate_document(&mut doc);

        let bytes = write_vsdx(&doc, VsdxPackageKind::Drawing).unwrap();
        let (reloaded, _) = read_vsdx(&bytes).unwrap();
        let sh = reloaded.page(1).unwrap().find_shape(1).unwrap();
        assert_eq!(sh.sheet.actions.len(), 1);
        let act = &sh.sheet.actions[0];
        assert_eq!(act.name, "ToggleValve");
        assert!(act.action.contains("SETF"));
        assert!(act.checked.contains("User.IsOpen"));
        assert!(act.begin_group);
        assert!(!sh.sheet.connection_points.is_empty());
    }
}
