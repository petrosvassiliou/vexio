//! `vexio-vsd-legacy` — Legacy Microsoft Visio Binary & XML Compatibility Engine.
//!
//! Supports `.vsd`, `.vss`, `.vst` (OLE2 Compound File Binary MS-VSD streams),
//! `.vdx` (DatadiagramML XML), and `.vdw` (Visio Web Drawing packages).

pub mod formula_emf;
pub mod lz_visio;
pub mod vdx_xml;
pub mod vsd_cfb;

pub use formula_emf::{
    compile_formula_to_vsd_bytecode, decompile_vsd_bytecode_to_formula, encode_shape_to_emf,
    encode_visio_v5_stream, parse_emf_to_shape, parse_visio_v5_stream, parse_wmf_to_shape,
};
pub use vdx_xml::{read_vdw, read_vdx_xml, write_vdx_xml};
pub use vsd_cfb::{LegacyVisioError, read_vsd_cfb, write_vsd_cfb};

/// Auto-detect and parse any legacy Visio or Metafile format (`.vsd` v5/v6/v11, `.vss`, `.vst`, `.vdx`, `.vdw`, `.emf`, `.wmf`).
pub fn read_legacy_auto(bytes: &[u8], ext_hint: &str) -> Result<vexio_shapesheet::VisioDocument, LegacyVisioError> {
    if bytes.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
        return read_vsd_cfb(bytes);
    }
    if bytes.starts_with(b"Visio (TM) Drawing") && bytes.len() > 0x1A && bytes[0x1A] == 5 {
        return parse_visio_v5_stream(bytes);
    }
    let ext = ext_hint.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext == "emf" || (bytes.len() >= 44 && &bytes[40..44] == b" EMF") {
        let mut doc = vexio_shapesheet::VisioDocument::new_blank("Imported EMF Drawing");
        let sh = parse_emf_to_shape(1, "Metafile.1", bytes, 5.5, 4.25)
            .map_err(LegacyVisioError::InvalidHeader)?;
        if let Some(p) = doc.page_mut(1) {
            p.shapes.push(sh);
        }
        vexio_shapesheet::ReactiveEngine::recalculate_document(&mut doc);
        return Ok(doc);
    }
    if ext == "wmf" || bytes.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        let mut doc = vexio_shapesheet::VisioDocument::new_blank("Imported WMF Drawing");
        let sh = parse_wmf_to_shape(1, "Metafile.1", bytes, 5.5, 4.25)
            .map_err(LegacyVisioError::InvalidHeader)?;
        if let Some(p) = doc.page_mut(1) {
            p.shapes.push(sh);
        }
        vexio_shapesheet::ReactiveEngine::recalculate_document(&mut doc);
        return Ok(doc);
    }
    if ext == "vdw" || bytes.starts_with(b"PK\x03\x04") {
        return read_vdw(bytes);
    }
    let xml = String::from_utf8_lossy(bytes);
    read_vdx_xml(&xml)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vexio_shapesheet::*;

    #[test]
    fn ole2_binary_vsd_and_vdx_xml_roundtrip() {
        let mut doc = VisioDocument::new_blank("Legacy Industrial P&ID");
        {
            let page = doc.page_mut(1).unwrap();
            let mut pump = VisioShape::new_rect(1, "CentrifugalPump", 3.0, 4.0, 2.0, 1.5);
            pump.text = "P-101A".into();
            pump.sheet
                .set_formula("FillForegnd", "=RGB(14,165,233)", Some(Unit::Color));
            page.shapes.push(pump);

            let mut tank = VisioShape::new_rect(2, "StorageTank", 8.0, 4.0, 2.5, 3.0);
            tank.text = "TK-200".into();
            page.shapes.push(tank);

            let pipe = VisioShape::new_connector(3, "ProcessLine", 4.0, 4.0, 6.75, 4.0);
            page.shapes.push(pipe);
            page.connects.push(GlueEndpoint {
                connector_shape_id: 3,
                from_cell: "BeginX".into(),
                target_shape_id: 1,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
        }
        ReactiveEngine::recalculate_document(&mut doc);

        // 1. OLE2 CFB `.vsd` round-trip
        let vsd_bytes = write_vsd_cfb(&doc).unwrap();
        assert!(vsd_bytes.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1"));
        let from_vsd = read_legacy_auto(&vsd_bytes, "vsd").unwrap();
        assert_eq!(from_vsd.properties.title, "Legacy Industrial P&ID");
        let p1 = from_vsd.page(1).unwrap();
        assert_eq!(p1.shapes.len(), 3);
        assert_eq!(p1.find_shape(1).unwrap().text, "P-101A");
        assert_eq!(p1.connects.len(), 1);

        // 2. DatadiagramML `.vdx` round-trip
        let vdx_str = write_vdx_xml(&doc);
        let from_vdx = read_legacy_auto(vdx_str.as_bytes(), "vdx").unwrap();
        assert_eq!(from_vdx.properties.title, "Legacy Industrial P&ID");
        assert_eq!(from_vdx.page(1).unwrap().find_shape(2).unwrap().text, "TK-200");
    }
}
