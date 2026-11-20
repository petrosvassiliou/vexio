//! `vexio-cad` — Native 100% Offline AutoCAD `.dwg` & `.dxf` Interop Engine for Vexio.
//!
//! Provides deterministic, zero-UI-dependency CAD import, export, layer mapping,
//! architectural/metric scale locking, and CAD-to-ShapeSheet editable shape conversion.

pub mod dwg_modern;
pub mod dwg_reader;
pub mod dxf_codec;
pub mod layers_scale;

use thiserror::Error;

/// Structured error type for offline DXF and DWG CAD operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CadError {
    #[error("Invalid DXF stream: {0}")]
    InvalidDxf(String),
    #[error("Invalid DWG magic header: {0}")]
    InvalidDwgMagic(String),
    #[error("Unsupported DWG version: {0}")]
    UnsupportedDwgVersion(String),
    #[error("Corrupt DWG bit-stream: {0}")]
    CorruptDwgStream(String),
    #[error("Visio page {0} not found")]
    PageNotFound(u32),
    #[error("CAD layer '{0}' not found")]
    LayerNotFound(String),
}

pub use dwg_modern::{
    CadHatchLoop, build_cad_hatch_shape, build_cad_linear_dimension_shape,
    build_cad_viewport_shape, compress_dwg_r2004_lz77, decode_dwg_modern_container,
    decompress_dwg_r2004_lz77, decrypt_r2004_header_block, deinterleave_reed_solomon_blocks,
    encode_dwg_modern_container, encrypt_r2004_header_block, interleave_reed_solomon_blocks,
    parse_dxf_extended_entities, read_tu_utf16_string,
};
pub use dwg_reader::{
    DwgBitReader, DwgBitWriter, DwgEntitySpec, DwgLayerSpec, DwgMinimalScene, DwgVersion,
    encode_dwg_r2000_from_page, encode_minimal_dwg_r2000, import_dwg_to_page,
};
pub use dxf_codec::{
    BINARY_DXF_SENTINEL, DxfValue, aci_to_rgba, encode_binary_dxf_from_ascii, export_page_to_dxf,
    import_dxf_to_page, parse_dxf_groups, rgba_to_aci, strip_mtext_formatting,
};
pub use layers_scale::{
    CadScalePreset, apply_cad_scale_lock, convert_cad_layer_to_visio_shapes, set_cad_layer_state,
};

#[cfg(test)]
mod tests {
    use super::*;
    use vexio_shapesheet::{
        CellError, EvaluatedPathVerb, GeometryRowKind, ReactiveEngine, RgbaColor, ShapeKind,
        VisioDocument, VisioPage,
    };

    #[test]
    fn full_dxf_pipeline_scale_lock_layer_toggle_conversion_export_and_roundtrip() {
        let sample_dxf = "\
0\nSECTION\n2\nHEADER\n\
9\n$ACADVER\n1\nAC1015\n\
9\n$INSUNITS\n70\n1\n\
9\n$EXTMIN\n10\n0.0\n20\n0.0\n30\n0.0\n\
9\n$EXTMAX\n10\n24.0\n20\n18.0\n30\n0.0\n\
0\nENDSEC\n\
0\nSECTION\n2\nTABLES\n\
0\nTABLE\n2\nLAYER\n70\n5\n\
0\nLAYER\n2\n0\n70\n0\n62\n7\n\
0\nLAYER\n2\nWALLS\n70\n0\n62\n1\n\
0\nLAYER\n2\nDOORS\n70\n4\n62\n3\n\
0\nLAYER\n2\nFROZEN_HVAC\n70\n1\n62\n4\n\
0\nLAYER\n2\nOFF_ELEC\n70\n0\n62\n-5\n\
0\nENDTAB\n\
0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n\
0\nBLOCK\n8\n0\n2\nDESK_SYM\n10\n0.0\n20\n0.0\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n2.0\n21\n0.0\n\
0\nCIRCLE\n8\n0\n10\n1.0\n20\n1.0\n40\n0.5\n\
0\nENDBLK\n\
0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\nWALLS\n10\n1.0\n20\n1.0\n11\n9.0\n21\n1.0\n\
0\nLWPOLYLINE\n8\nWALLS\n90\n4\n70\n1\n\
10\n2.0\n20\n2.0\n\
10\n6.0\n20\n2.0\n42\n0.5\n\
10\n6.0\n20\n5.0\n\
10\n2.0\n20\n5.0\n\
0\nPOLYLINE\n8\nWALLS\n66\n1\n70\n0\n\
0\nVERTEX\n8\nWALLS\n10\n0.5\n20\n8.0\n\
0\nVERTEX\n8\nWALLS\n10\n2.5\n20\n9.5\n\
0\nVERTEX\n8\nWALLS\n10\n4.5\n20\n8.0\n\
0\nSEQEND\n\
0\nCIRCLE\n8\nWALLS\n10\n12.0\n20\n6.0\n40\n2.0\n\
0\nARC\n8\nDOORS\n10\n8.0\n20\n4.0\n40\n1.5\n50\n0.0\n51\n90.0\n\
0\nELLIPSE\n8\nWALLS\n10\n15.0\n20\n10.0\n11\n3.0\n21\n0.0\n40\n0.5\n41\n0.0\n42\n6.283185307179586\n\
0\nSPLINE\n8\nWALLS\n70\n0\n71\n3\n\
40\n0.0\n40\n0.0\n40\n0.0\n40\n0.0\n40\n1.0\n40\n1.0\n40\n1.0\n40\n1.0\n\
10\n1.0\n20\n12.0\n\
10\n3.0\n20\n15.0\n\
10\n6.0\n20\n15.0\n\
10\n8.0\n20\n12.0\n\
0\nTEXT\n8\nWALLS\n10\n3.0\n20\n16.0\n40\n0.25\n1\nMAIN CORRIDOR %%c12\n\
0\nMTEXT\n8\nWALLS\n10\n10.0\n20\n14.0\n40\n0.3\n41\n4.0\n1\n{\\fArial|b1;ROOM 101}\\PAREA: 240 SQFT\n\
0\nSOLID\n8\nWALLS\n10\n18.0\n20\n2.0\n11\n20.0\n21\n2.0\n12\n18.0\n22\n4.0\n13\n20.0\n23\n4.0\n\
0\nHATCH\n8\nWALLS\n70\n1\n91\n1\n92\n2\n72\n0\n73\n1\n93\n3\n10\n20.5\n20\n2.0\n10\n22.5\n20\n2.0\n10\n21.5\n20\n4.0\n\
0\nDIMENSION\n8\nWALLS\n10\n1.0\n20\n0.5\n11\n5.0\n21\n0.6\n13\n1.0\n23\n1.0\n14\n9.0\n24\n1.0\n1\n<>\n\
0\nINSERT\n8\nWALLS\n2\nDESK_SYM\n10\n10.0\n20\n2.0\n41\n2.0\n42\n2.0\n50\n90.0\n\
0\nLINE\n8\nFROZEN_HVAC\n10\n0.0\n20\n0.0\n11\n24.0\n21\n18.0\n\
0\nENDSEC\n\
0\nEOF\n";

        let mut doc = VisioDocument::new_blank("CAD Floorplan");
        let created_ids = {
            let page = doc.page_mut(1).unwrap();
            import_dxf_to_page(page, sample_dxf.as_bytes()).expect("DXF import must succeed")
        };
        assert_eq!(created_ids.len(), 14);

        // Verify LAYER table mapping (colors, frozen, locked, negative=off)
        {
            let page = doc.page(1).unwrap();
            assert!((page.width_in() - 24.0).abs() < 1e-6);
            assert!((page.height_in() - 18.0).abs() < 1e-6);

            let walls = page.layers.iter().find(|l| l.name == "WALLS").unwrap();
            assert!(walls.visible);
            assert!(!walls.lock);
            assert_eq!(
                walls.color_override,
                Some(RgbaColor { r: 255, g: 0, b: 0, a: 255 })
            );

            let doors = page.layers.iter().find(|l| l.name == "DOORS").unwrap();
            assert!(doors.visible);
            assert!(doors.lock);
            assert_eq!(
                doors.color_override,
                Some(RgbaColor { r: 0, g: 255, b: 0, a: 255 })
            );

            let frozen = page.layers.iter().find(|l| l.name == "FROZEN_HVAC").unwrap();
            assert!(!frozen.visible);
            assert!(!frozen.print);

            let off_elec = page.layers.iter().find(|l| l.name == "OFF_ELEC").unwrap();
            assert!(!off_elec.visible);
            assert!(off_elec.print);
            assert_eq!(
                off_elec.color_override,
                Some(RgbaColor { r: 0, g: 0, b: 255, a: 255 })
            );

            // Verify LWPOLYLINE bulge -> ArcTo bow on shape 2
            let lwpoly = page.find_shape(created_ids[1]).unwrap();
            let arc_row = lwpoly.sheet.geometries[0]
                .rows
                .iter()
                .find(|r| r.kind == GeometryRowKind::ArcTo)
                .expect("LWPOLYLINE bulge must map to GeometryRowKind::ArcTo");
            let bow = lwpoly
                .sheet
                .get_f64(&format!("Geometry1.A{}", arc_row.ix))
                .unwrap();
            // Segment (6,2)->(6,5) has chord = 3.0, bulge = 0.5 -> bow = 0.5 * 1.5 = 0.75 in
            assert!((bow - 0.75).abs() < 1e-6);

            // Verify MTEXT stripped `\f...;` and converted `\P` -> `\n`
            let mtext_shape = page
                .shapes
                .iter()
                .find(|s| s.text.contains("ROOM 101"))
                .expect("MTEXT shape must exist");
            assert_eq!(mtext_shape.text, "ROOM 101\nAREA: 240 SQFT");

            // Verify TEXT decoded `%%c` -> `⌀`
            let text_shape = page
                .shapes
                .iter()
                .find(|s| s.text.contains("MAIN CORRIDOR"))
                .expect("TEXT shape must exist");
            assert_eq!(text_shape.text, "MAIN CORRIDOR ⌀12");

            // Verify INSERT block created Group with 2 transformed child entities
            let insert_grp = page
                .shapes
                .iter()
                .find(|s| s.kind == ShapeKind::Group && s.name.contains("DESK_SYM"))
                .expect("INSERT block group must exist");
            assert_eq!(insert_grp.children.len(), 2);
        }

        // Step 2: Apply Architectural 1/4" = 1'-0" (1:48) Scale Lock!
        {
            let page = doc.page_mut(1).unwrap();
            apply_cad_scale_lock(page, CadScalePreset::ArchitecturalQuarterInchPerFoot, true);
            assert!((page.page_sheet.get_f64("PageScale").unwrap() - 0.25).abs() < 1e-9);
            assert!((page.page_sheet.get_f64("DrawingScale").unwrap() - 12.0).abs() < 1e-9);
            // Interactive overwrite must be blocked by GUARD!
            assert_eq!(
                page.page_sheet
                    .try_set_formula_respecting_guard("PageScale", "1 in", false),
                Err(CellError::Ref)
            );
            let bg = page
                .layers
                .iter()
                .find(|l| l.name == "CAD Background")
                .unwrap();
            assert!(bg.lock);
        }

        // Step 3: Toggle CAD layer state (thaw FROZEN_HVAC & set custom color override on WALLS)
        let custom_teal = RgbaColor {
            r: 0,
            g: 180,
            b: 180,
            a: 255,
        };
        {
            let page = doc.page_mut(1).unwrap();
            set_cad_layer_state(page, "FROZEN_HVAC", Some(true), Some(false), None);
            set_cad_layer_state(page, "WALLS", Some(true), Some(true), Some(Some(custom_teal)));
            let hvac = page.layers.iter().find(|l| l.name == "FROZEN_HVAC").unwrap();
            assert!(hvac.visible && hvac.print && !hvac.lock);
            let walls = page.layers.iter().find(|l| l.name == "WALLS").unwrap();
            assert!(walls.lock);
            assert_eq!(walls.color_override, Some(custom_teal));
        }

        // Step 4: Convert CAD layer "WALLS" into standard editable Visio shapes with connection points!
        {
            let page = doc.page_mut(1).unwrap();
            let converted = convert_cad_layer_to_visio_shapes(page, "WALLS");
            assert!(converted.len() >= 11);

            let walls = page.layers.iter().find(|l| l.name == "WALLS").unwrap();
            assert!(!walls.lock);

            // Check converted LINE shape: unlocked + 3 connection points (start, end, center)
            let line_shape = page.find_shape(created_ids[0]).unwrap();
            assert_eq!(line_shape.sheet.get_bool("LockMoveX"), Some(false));
            assert_eq!(line_shape.sheet.get_bool("User.CadConverted"), Some(true));
            assert_eq!(line_shape.sheet.connection_points.len(), 3);

            // Check converted CIRCLE shape: 5 connection points (center + 4 quadrants)
            let circle_shape = page.find_shape(created_ids[3]).unwrap();
            assert_eq!(circle_shape.sheet.connection_points.len(), 5);
        }

        // Step 5: Export page to R2000+ ASCII DXF and round-trip re-import onto a fresh page!
        ReactiveEngine::recalculate_document(&mut doc);
        let exported_dxf = export_page_to_dxf(&doc, 1);
        assert!(exported_dxf.contains("LWPOLYLINE"));
        assert!(exported_dxf.contains("CIRCLE"));
        assert!(exported_dxf.contains("ELLIPSE"));
        assert!(exported_dxf.contains("ARC"));
        assert!(exported_dxf.contains("MTEXT"));

        let mut rt_doc = VisioDocument::new_blank("RoundTrip DXF");
        let rt_ids = {
            let rt_page = rt_doc.page_mut(1).unwrap();
            import_dxf_to_page(rt_page, exported_dxf.as_bytes())
                .expect("Exported DXF must re-import cleanly")
        };
        assert!(rt_ids.len() >= 14);
        ReactiveEngine::recalculate_document(&mut rt_doc);
        let rt_visuals = ReactiveEngine::evaluate_page_visuals(&rt_doc, 1);

        // Verify bulged LWPOLYLINE round-tripped its exact ArcTo bow (0.75 in)
        let has_bulged_arc = rt_visuals.iter().any(|v| {
            v.paths.iter().any(|p| {
                p.verbs_page_space.iter().any(|verb| {
                    matches!(verb, EvaluatedPathVerb::ArcTo { bow, .. } if (*bow - 0.75).abs() < 1e-4)
                })
            })
        });
        assert!(has_bulged_arc, "Round-tripped DXF must preserve LWPOLYLINE bulge -> ArcTo bow");

        // Verify Binary DXF (`AutoCAD Binary DXF\r\n\x1a\0`) round-trip as well!
        let bin_dxf = encode_binary_dxf_from_ascii(&exported_dxf)
            .expect("Binary DXF encoding must succeed");
        assert!(bin_dxf.starts_with(BINARY_DXF_SENTINEL));
        let mut bin_page = VisioPage::new(1, "BinDxfPage", 11.0, 8.5);
        let bin_ids = import_dxf_to_page(&mut bin_page, &bin_dxf)
            .expect("Binary DXF import must succeed");
        assert_eq!(bin_ids.len(), rt_ids.len());
    }

    #[test]
    fn dwg_bit_reader_primitives_and_version_magic_validation() {
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1012____"), Ok(DwgVersion::R13));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1014____"), Ok(DwgVersion::R14));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1015____"), Ok(DwgVersion::R2000));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1018____"), Ok(DwgVersion::R2004));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1021____"), Ok(DwgVersion::R2007));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1024____"), Ok(DwgVersion::R2010));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1027____"), Ok(DwgVersion::R2013));
        assert_eq!(DwgVersion::from_magic_bytes(b"AC1032____"), Ok(DwgVersion::R2018));
        assert!(matches!(
            DwgVersion::from_magic_bytes(b"NOTDWG"),
            Err(CadError::InvalidDwgMagic(_))
        ));
        assert!(matches!(
            DwgVersion::from_magic_bytes(b"AC9999"),
            Err(CadError::UnsupportedDwgVersion(_))
        ));

        // Verify unaligned bit-stream round-trip for every DWG primitive
        let mut w = DwgBitWriter::new();
        w.write_bit(1);
        w.write_bb(0b10);
        w.write_bs(0);
        w.write_bs(256);
        w.write_bs(42);
        w.write_bs(-1234);
        w.write_bl(0);
        w.write_bl(200);
        w.write_bl(987_654);
        w.write_bd(0.0);
        w.write_bd(1.0);
        w.write_bd(-3.141592653589793);
        w.write_2rd([12.5, -7.25]);
        w.write_3bd([1.0, 0.0, 48.125]);
        w.write_mc(0);
        w.write_mc(63);
        w.write_mc(64);
        w.write_mc(-123_456);
        w.write_ms(120);
        w.write_ms(65_000);
        w.write_tv("AcDb:AcDbObjects");

        let bytes = w.into_bytes();
        let mut r = DwgBitReader::new(&bytes);
        assert_eq!(r.read_bit().unwrap(), 1);
        assert_eq!(r.read_bb().unwrap(), 0b10);
        assert_eq!(r.read_bs().unwrap(), 0);
        assert_eq!(r.read_bs().unwrap(), 256);
        assert_eq!(r.read_bs().unwrap(), 42);
        assert_eq!(r.read_bs().unwrap(), -1234);
        assert_eq!(r.read_bl().unwrap(), 0);
        assert_eq!(r.read_bl().unwrap(), 200);
        assert_eq!(r.read_bl().unwrap(), 987_654);
        assert_eq!(r.read_bd().unwrap(), 0.0);
        assert_eq!(r.read_bd().unwrap(), 1.0);
        assert!((r.read_bd().unwrap() - (-3.141592653589793)).abs() < 1e-14);
        assert_eq!(r.read_2rd().unwrap(), [12.5, -7.25]);
        assert_eq!(r.read_3bd().unwrap(), [1.0, 0.0, 48.125]);
        assert_eq!(r.read_mc().unwrap(), 0);
        assert_eq!(r.read_mc().unwrap(), 63);
        assert_eq!(r.read_mc().unwrap(), 64);
        assert_eq!(r.read_mc().unwrap(), -123_456);
        assert_eq!(r.read_ms().unwrap(), 120);
        assert_eq!(r.read_ms().unwrap(), 65_000);
        assert_eq!(r.read_tv().unwrap(), "AcDb:AcDbObjects");
    }

    #[test]
    fn binary_dwg_ac1015_encode_and_bit_stream_decode_roundtrip() {
        let scene = DwgMinimalScene {
            insunits: 1, // Inches
            ext_min: [0.0, 0.0, 0.0],
            ext_max: [20.0, 15.0, 0.0],
            layers: vec![
                DwgLayerSpec {
                    name: "0".into(),
                    aci_color: 7,
                    flags: 0,
                    plot: true,
                },
                DwgLayerSpec {
                    name: "STRUCTURAL".into(),
                    aci_color: 1,
                    flags: 4, // Locked
                    plot: true,
                },
                DwgLayerSpec {
                    name: "ANNOT".into(),
                    aci_color: 5,
                    flags: 0,
                    plot: true,
                },
            ],
            entities: vec![
                DwgEntitySpec::Line {
                    layer: "STRUCTURAL".into(),
                    aci_color: 256, // BYLAYER
                    start: [1.0, 2.0],
                    end: [9.0, 2.0],
                },
                DwgEntitySpec::Circle {
                    layer: "STRUCTURAL".into(),
                    aci_color: 3,
                    center: [5.0, 6.0],
                    radius: 1.75,
                },
                DwgEntitySpec::Arc {
                    layer: "STRUCTURAL".into(),
                    aci_color: 2,
                    center: [10.0, 6.0],
                    radius: 2.0,
                    start_angle_rad: 0.0,
                    end_angle_rad: std::f64::consts::FRAC_PI_2,
                },
                DwgEntitySpec::LwPolyline {
                    layer: "STRUCTURAL".into(),
                    aci_color: 256,
                    closed: true,
                    vertices: vec![
                        ([12.0, 2.0], 0.0),
                        ([16.0, 2.0], 0.5),
                        ([16.0, 5.0], 0.0),
                        ([12.0, 5.0], 0.0),
                    ],
                },
                DwgEntitySpec::Text {
                    layer: "ANNOT".into(),
                    aci_color: 256,
                    position: [4.0, 11.0],
                    height: 0.25,
                    rotation_rad: 0.0,
                    value: "COLUMN GRID A-1".into(),
                    is_mtext: false,
                },
            ],
        };

        let dwg_bytes = encode_minimal_dwg_r2000(&scene);
        assert_eq!(&dwg_bytes[0..6], b"AC1015");

        let mut doc = VisioDocument::new_blank("DWG Import");
        let (ver, shape_ids) = {
            let page = doc.page_mut(1).unwrap();
            import_dwg_to_page(page, &dwg_bytes).expect("DWG R2000 decode must succeed")
        };
        assert_eq!(ver, DwgVersion::R2000);
        assert_eq!(shape_ids.len(), 5);

        {
            let page = doc.page(1).unwrap();
            let struct_layer = page
                .layers
                .iter()
                .find(|l| l.name == "STRUCTURAL")
                .expect("STRUCTURAL layer must be decoded");
            assert!(struct_layer.lock);
            assert_eq!(
                struct_layer.color_override,
                Some(RgbaColor { r: 255, g: 0, b: 0, a: 255 })
            );

            let poly = page.find_shape(shape_ids[3]).unwrap();
            assert!(
                poly.sheet.geometries[0]
                    .rows
                    .iter()
                    .any(|r| r.kind == GeometryRowKind::ArcTo)
            );
            let txt = page.find_shape(shape_ids[4]).unwrap();
            assert_eq!(txt.text, "COLUMN GRID A-1");
        }

        // Now test `encode_dwg_r2000_from_page` -> `import_dwg_to_page` full page round-trip!
        ReactiveEngine::recalculate_document(&mut doc);
        let reencoded_dwg =
            encode_dwg_r2000_from_page(&doc, 1).expect("Page-to-DWG R2000 encode must succeed");
        let mut doc2 = VisioDocument::new_blank("DWG RoundTrip 2");
        let (ver2, ids2) = {
            let p2 = doc2.page_mut(1).unwrap();
            import_dwg_to_page(p2, &reencoded_dwg).expect("Re-encoded DWG decode must succeed")
        };
        assert_eq!(ver2, DwgVersion::R2000);
        assert_eq!(ids2.len(), 5);
        assert!(
            doc2.page(1)
                .unwrap()
                .shapes
                .iter()
                .any(|s| s.text == "COLUMN GRID A-1")
        );
    }

    #[test]
    fn cad_scale_presets_and_millimeter_insunits() {
        let mm_dxf = "\
0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n254.0\n21\n0.0\n\
0\nENDSEC\n0\nEOF\n";

        let mut page = VisioPage::new(1, "MetricCAD", 11.0, 8.5);
        let ids = import_dxf_to_page(&mut page, mm_dxf.as_bytes()).unwrap();
        assert_eq!(ids.len(), 1);
        let line = page.find_shape(ids[0]).unwrap();
        // 254 mm == 10.0 inches exact!
        assert!((line.sheet.get_f64("Width").unwrap() - 10.0).abs() < 1e-9);

        for (preset, expected_ratio) in [
            (CadScalePreset::ArchitecturalQuarterInchPerFoot, 48.0),
            (CadScalePreset::ArchitecturalEighthInchPerFoot, 96.0),
            (CadScalePreset::Metric1To50, 50.0),
            (CadScalePreset::Metric1To100, 100.0),
            (CadScalePreset::Metric1To200, 200.0),
            (
                CadScalePreset::Custom {
                    page_in: 0.5,
                    drawing_in: 60.0,
                },
                120.0,
            ),
        ] {
            apply_cad_scale_lock(&mut page, preset, false);
            let p = page.page_sheet.get_f64("PageScale").unwrap();
            let d = page.page_sheet.get_f64("DrawingScale").unwrap();
            assert!(((d / p) - expected_ratio).abs() < 1e-6);
        }
    }
}

