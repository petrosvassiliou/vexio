//! OLE2 Compound File Binary (`.vsd`, `.vss`, `.vst`) MS-VSD v6/v11 parser and writer.

use crate::lz_visio::{compress_vsd_stream, decompress_vsd_stream};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use thiserror::Error;
use vexio_shapesheet::{
    ForeignMediaData, GeometryRow, GeometryRowKind, GeometrySection, GlueEndpoint, LayerDef,
    ReactiveEngine, ShapeKind, ShapeSheet, Unit, VisioDocument, VisioMaster, VisioPage, VisioShape,
    format_clean_f64,
};

pub const VSD_HEADER_MAGIC: &[u8; 21] = b"Visio (TM) Drawing\r\n\0";

// MS-VSD Chunk / Stream Record Type IDs
const VSD_TRAILER: u16 = 0x0014;
const VSD_DOC_SHEET: u16 = 0x0015;
const VSD_PAGE: u16 = 0x0016;
const VSD_MASTER: u16 = 0x001E;
const VSD_SHAPE_2D: u16 = 0x0046;
const VSD_SHAPE_1D: u16 = 0x0047;
const VSD_SHAPE_GROUP: u16 = 0x0048;
const VSD_XFORM_DATA: u16 = 0x009B;
const VSD_XFORM_1D: u16 = 0x009C;
const VSD_TEXT_UTF16: u16 = 0x000C;
const VSD_LINE_FMT: u16 = 0x009A;
const VSD_FILL_FMT: u16 = 0x009D;
const VSD_GEOM_HDR: u16 = 0x0089;
const VSD_MOVE_TO: u16 = 0x008A;
const VSD_LINE_TO: u16 = 0x008B;
const VSD_ARC_TO: u16 = 0x008C;
const VSD_ELLIPSE: u16 = 0x008F;
const VSD_CONNECT: u16 = 0x00A8;
const VSD_LAYER_DEF: u16 = 0x00B2;
const VSD_NAME_UTF8: u16 = 0x00D1;

#[derive(Debug, Error)]
pub enum LegacyVisioError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML parse error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("VSDX sub-stream error: {0}")]
    Vsdx(#[from] vexio_vsdx::VsdxError),
    #[error("Invalid or unrecognized legacy Visio file header: {0}")]
    InvalidHeader(String),
}

#[derive(Debug, Clone, Copy)]
struct VsdPointer {
    type_id: u16,
    offset: u32,
    length: u32,
    compressed: bool,
}

pub fn read_vsd_cfb(bytes: &[u8]) -> Result<VisioDocument, LegacyVisioError> {
    let cursor = Cursor::new(bytes);
    let mut comp = cfb::CompoundFile::open(cursor)?;
    let mut stream_bytes = Vec::new();
    {
        let stream_path = if comp.exists("/VisioDocument") {
            "/VisioDocument"
        } else if comp.exists("VisioDocument") {
            "VisioDocument"
        } else {
            return Err(LegacyVisioError::InvalidHeader(
                "OLE2 container missing VisioDocument stream".into(),
            ));
        };
        let mut s = comp.open_stream(stream_path)?;
        s.read_to_end(&mut stream_bytes)?;
    }

    parse_visio_document_stream(&stream_bytes)
}

fn parse_visio_document_stream(buf: &[u8]) -> Result<VisioDocument, LegacyVisioError> {
    if buf.len() < 0x36 || !buf.starts_with(b"Visio (TM) Drawing") {
        return Err(LegacyVisioError::InvalidHeader(
            "VisioDocument signature mismatch".into(),
        ));
    }
    let version = buf[0x1A];
    if version == 5 {
        return crate::formula_emf::parse_visio_v5_stream(buf);
    }
    if version < 6 {
        return Err(LegacyVisioError::InvalidHeader(format!(
            "Unsupported pre-v5 Visio version {version}"
        )));
    }
    // Detect real Microsoft `[MS-VSD]` v6/v11 18-byte `Pointer6` at `0x24`:
    // `[type_id: u32, address: u32, offset: u32, length: u32, format: u16]`
    if is_real_ms_vsd_pointer6_header(buf) {
        return parse_real_ms_vsd_v6_v11(buf, version);
    }

    // Synthetic 12-byte Trailer pointer at 0x24: [type_id: u16, format: u16, offset: u32, length: u32]
    let trailer_ptr = read_pointer_at(buf, 0x24)?;
    if trailer_ptr.type_id != VSD_TRAILER {
        return Err(LegacyVisioError::InvalidHeader(format!(
            "Expected trailer pointer 0x0014, got 0x{:04X}",
            trailer_ptr.type_id
        )));
    }

    let trailer_data = extract_stream(buf, trailer_ptr)?;
    let child_ptrs = read_pointer_list(&trailer_data);

    let mut doc = VisioDocument::new_blank("Legacy Visio Drawing");
    doc.pages.clear();
    doc.masters.clear();

    for ptr in child_ptrs {
        match ptr.type_id {
            VSD_DOC_SHEET => {
                let data = extract_stream(buf, ptr)?;
                if let Some(title) = parse_name_record(&data) {
                    doc.properties.title = title;
                }
            }
            VSD_PAGE => {
                let data = extract_stream(buf, ptr)?;
                let page = parse_page_stream(&data, (doc.pages.len() as u32) + 1)?;
                doc.pages.push(page);
            }
            VSD_MASTER => {
                let data = extract_stream(buf, ptr)?;
                let master = parse_master_stream(&data, (doc.masters.len() as u32) + 1)?;
                doc.masters.push(master);
            }
            _ => {}
        }
    }

    if doc.pages.is_empty() {
        doc.pages.push(VisioPage::new(1, "Page-1", 11.0, 8.5));
    }

    ReactiveEngine::recalculate_document(&mut doc);
    Ok(doc)
}

pub fn write_vsd_cfb(doc: &VisioDocument) -> Result<Vec<u8>, LegacyVisioError> {
    let stream_bytes = build_visio_document_stream(doc);
    let cursor = Cursor::new(Vec::new());
    let mut comp = cfb::CompoundFile::create(cursor)?;
    {
        let mut s = comp.create_stream("/VisioDocument")?;
        s.write_all(&stream_bytes)?;
    }
    Ok(comp.into_inner().into_inner())
}

fn build_visio_document_stream(doc: &VisioDocument) -> Vec<u8> {
    let mut buf = vec![0u8; 0x40];
    buf[0..21].copy_from_slice(VSD_HEADER_MAGIC);
    buf[0x1A] = 11; // MS-VSD version 11 (Visio 2003-2010 binary format)

    let mut top_pointers = Vec::new();

    // 1. DocSheet stream
    let mut doc_rec = Vec::new();
    write_record(&mut doc_rec, VSD_NAME_UTF8, doc.properties.title.as_bytes());
    top_pointers.push(append_compressed_stream(&mut buf, VSD_DOC_SHEET, &doc_rec));

    // 2. Master streams
    for m in &doc.masters {
        let m_bytes = encode_master_stream(m);
        top_pointers.push(append_compressed_stream(&mut buf, VSD_MASTER, &m_bytes));
    }

    // 3. Page streams
    for p in &doc.pages {
        let p_bytes = encode_page_stream(p);
        top_pointers.push(append_compressed_stream(&mut buf, VSD_PAGE, &p_bytes));
    }

    // 4. Trailer stream holding pointer directory
    let trailer_raw = encode_pointer_list(&top_pointers);
    let trailer_ptr = append_compressed_stream(&mut buf, VSD_TRAILER, &trailer_raw);
    write_pointer_at(&mut buf, 0x24, trailer_ptr);

    // Write file length at 0x1C and CRC32 at 0x30
    let total_len = buf.len() as u32;
    buf[0x1C..0x20].copy_from_slice(&total_len.to_le_bytes());
    let crc = crc32fast::hash(&buf[0x40..]);
    buf[0x30..0x34].copy_from_slice(&crc.to_le_bytes());
    buf
}

fn read_pointer_at(buf: &[u8], off: usize) -> Result<VsdPointer, LegacyVisioError> {
    if off + 12 > buf.len() {
        return Err(LegacyVisioError::InvalidHeader("Truncated VSD pointer".into()));
    }
    let type_id = u16::from_le_bytes([buf[off], buf[off + 1]]);
    let format = u16::from_le_bytes([buf[off + 2], buf[off + 3]]);
    let offset = u32::from_le_bytes([buf[off + 4], buf[off + 5], buf[off + 6], buf[off + 7]]);
    let length = u32::from_le_bytes([buf[off + 8], buf[off + 9], buf[off + 10], buf[off + 11]]);
    Ok(VsdPointer {
        type_id,
        offset,
        length,
        compressed: (format & 0x0002) != 0,
    })
}

fn write_pointer_at(buf: &mut [u8], off: usize, ptr: VsdPointer) {
    let fmt: u16 = if ptr.compressed { 0x0002 } else { 0x0000 };
    buf[off..off + 2].copy_from_slice(&ptr.type_id.to_le_bytes());
    buf[off + 2..off + 4].copy_from_slice(&fmt.to_le_bytes());
    buf[off + 4..off + 8].copy_from_slice(&ptr.offset.to_le_bytes());
    buf[off + 8..off + 12].copy_from_slice(&ptr.length.to_le_bytes());
}

fn append_compressed_stream(buf: &mut Vec<u8>, type_id: u16, raw: &[u8]) -> VsdPointer {
    let comp = compress_vsd_stream(raw);
    let offset = buf.len() as u32;
    let length = comp.len() as u32;
    buf.extend_from_slice(&comp);
    VsdPointer {
        type_id,
        offset,
        length,
        compressed: true,
    }
}

fn extract_stream(buf: &[u8], ptr: VsdPointer) -> Result<Vec<u8>, LegacyVisioError> {
    let start = ptr.offset as usize;
    let end = start.saturating_add(ptr.length as usize);
    if end > buf.len() {
        return Err(LegacyVisioError::InvalidHeader(
            "VSD stream offset exceeds file bounds".into(),
        ));
    }
    let slice = &buf[start..end];
    if ptr.compressed {
        // MS-VSD compressed streams optionally start with a 4-byte header if third-party; our encoder writes pure LZW blocks, while real MS-VSD streams with 4-byte subheaders are detected cleanly.
        Ok(decompress_vsd_stream(slice))
    } else {
        Ok(slice.to_vec())
    }
}

fn encode_pointer_list(ptrs: &[VsdPointer]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + ptrs.len() * 12);
    out.extend_from_slice(&(ptrs.len() as u32).to_le_bytes());
    for p in ptrs {
        let mut slot = [0u8; 12];
        write_pointer_at(&mut slot, 0, *p);
        out.extend_from_slice(&slot);
    }
    out
}

fn read_pointer_list(data: &[u8]) -> Vec<VsdPointer> {
    if data.len() < 4 {
        return Vec::new();
    }
    let count = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
    let mut out = Vec::new();
    let mut off = 4usize;
    for _ in 0..count {
        if let Ok(p) = read_pointer_at(data, off) {
            out.push(p);
            off += 12;
        } else {
            break;
        }
    }
    out
}

fn write_record(out: &mut Vec<u8>, rec_type: u16, payload: &[u8]) {
    out.extend_from_slice(&rec_type.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
}

fn iter_records(mut data: &[u8]) -> Vec<(u16, &[u8])> {
    let mut recs = Vec::new();
    while data.len() >= 6 {
        let rtype = u16::from_le_bytes([data[0], data[1]]);
        let rlen = u32::from_le_bytes([data[2], data[3], data[4], data[5]]) as usize;
        data = &data[6..];
        if rlen > data.len() {
            break;
        }
        recs.push((rtype, &data[..rlen]));
        data = &data[rlen..];
    }
    recs
}

fn parse_name_record(data: &[u8]) -> Option<String> {
    for (rt, payload) in iter_records(data) {
        if rt == VSD_NAME_UTF8 {
            return Some(String::from_utf8_lossy(payload).into_owned());
        }
    }
    None
}

fn encode_page_stream(page: &VisioPage) -> Vec<u8> {
    let mut out = Vec::new();
    // Header: [id: u32, width: f64, height: f64, is_bg: u8, back_id: u32]
    let mut hdr = Vec::with_capacity(25);
    hdr.extend_from_slice(&page.id.to_le_bytes());
    hdr.extend_from_slice(&page.width_in().to_le_bytes());
    hdr.extend_from_slice(&page.height_in().to_le_bytes());
    hdr.push(if page.is_background { 1 } else { 0 });
    hdr.extend_from_slice(&page.back_page_id.unwrap_or(0).to_le_bytes());
    write_record(&mut out, VSD_PAGE, &hdr);
    write_record(&mut out, VSD_NAME_UTF8, page.name.as_bytes());

    for l in &page.layers {
        let mut lp = Vec::new();
        lp.extend_from_slice(&l.ix.to_le_bytes());
        lp.push(
            (if l.visible { 1 } else { 0 })
                | (if l.print { 2 } else { 0 })
                | (if l.lock { 4 } else { 0 }),
        );
        lp.extend_from_slice(l.name.as_bytes());
        write_record(&mut out, VSD_LAYER_DEF, &lp);
    }

    for sh in &page.shapes {
        let (tag, sbytes) = encode_shape_record(sh);
        write_record(&mut out, tag, &sbytes);
    }

    for c in &page.connects {
        let mut cp = Vec::new();
        cp.extend_from_slice(&c.connector_shape_id.to_le_bytes());
        cp.extend_from_slice(&c.target_shape_id.to_le_bytes());
        cp.push(if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            0
        } else {
            1
        });
        cp.extend_from_slice(c.to_cell.as_bytes());
        write_record(&mut out, VSD_CONNECT, &cp);
    }
    out
}

fn parse_page_stream(data: &[u8], fallback_id: u32) -> Result<VisioPage, LegacyVisioError> {
    let mut id = fallback_id;
    let mut w = 11.0;
    let mut h = 8.5;
    let mut is_bg = false;
    let mut back_id = None;
    let mut name = format!("Page-{fallback_id}");
    let mut layers = Vec::new();
    let mut shapes = Vec::new();
    let mut connects = Vec::new();

    for (rt, payload) in iter_records(data) {
        match rt {
            VSD_PAGE if payload.len() >= 25 => {
                id = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                w = f64::from_le_bytes(payload[4..12].try_into().unwrap());
                h = f64::from_le_bytes(payload[12..20].try_into().unwrap());
                is_bg = payload[20] != 0;
                let bid = u32::from_le_bytes(payload[21..25].try_into().unwrap());
                if bid != 0 {
                    back_id = Some(bid);
                }
            }
            VSD_NAME_UTF8 => {
                name = String::from_utf8_lossy(payload).into_owned();
            }
            VSD_LAYER_DEF if payload.len() >= 5 => {
                let ix = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                let flags = payload[4];
                let lname = String::from_utf8_lossy(&payload[5..]).into_owned();
                let mut ldef = LayerDef::new(ix, lname);
                ldef.visible = (flags & 1) != 0;
                ldef.print = (flags & 2) != 0;
                ldef.lock = (flags & 4) != 0;
                layers.push(ldef);
            }
            VSD_SHAPE_2D | VSD_SHAPE_1D | VSD_SHAPE_GROUP => {
                shapes.push(parse_shape_record(rt, payload));
            }
            VSD_CONNECT if payload.len() >= 17 => {
                let from_id = u64::from_le_bytes(payload[0..8].try_into().unwrap());
                let to_id = u64::from_le_bytes(payload[8..16].try_into().unwrap());
                let from_cell = if payload[16] == 0 {
                    "BeginX".into()
                } else {
                    "EndX".into()
                };
                let to_cell = if payload.len() > 17 {
                    String::from_utf8_lossy(&payload[17..]).into_owned()
                } else {
                    "PinX".into()
                };
                let is_dynamic = to_cell.eq_ignore_ascii_case("PinX");
                connects.push(GlueEndpoint {
                    connector_shape_id: from_id,
                    from_cell,
                    target_shape_id: to_id,
                    to_cell,
                    is_dynamic,
                });
            }
            _ => {}
        }
    }

    let mut page = VisioPage::new(id, name, w, h);
    page.is_background = is_bg;
    page.back_page_id = back_id;
    if !layers.is_empty() {
        page.layers = layers;
    }
    page.shapes = shapes;
    page.connects = connects;
    Ok(page)
}

fn encode_master_stream(m: &VisioMaster) -> Vec<u8> {
    let mut out = Vec::new();
    write_record(&mut out, VSD_MASTER, &m.id.to_le_bytes());
    write_record(&mut out, VSD_NAME_UTF8, m.name.as_bytes());
    for s in &m.shapes {
        let (tag, sbytes) = encode_shape_record(s);
        write_record(&mut out, tag, &sbytes);
    }
    out
}

fn parse_master_stream(data: &[u8], fallback_id: u32) -> Result<VisioMaster, LegacyVisioError> {
    let mut id = fallback_id;
    let mut name = format!("Master.{fallback_id}");
    let mut shapes = Vec::new();
    for (rt, payload) in iter_records(data) {
        match rt {
            VSD_MASTER if payload.len() >= 4 => {
                id = u32::from_le_bytes(payload[0..4].try_into().unwrap());
            }
            VSD_NAME_UTF8 => name = String::from_utf8_lossy(payload).into_owned(),
            VSD_SHAPE_2D | VSD_SHAPE_1D | VSD_SHAPE_GROUP => {
                shapes.push(parse_shape_record(rt, payload));
            }
            _ => {}
        }
    }
    Ok(VisioMaster {
        id,
        name: name.clone(),
        name_u: name,
        base_id: String::new(),
        unique_id: String::new(),
        prompt: String::new(),
        icon_bytes: None,
        page_sheet: ShapeSheet::new(),
        shapes,
    })
}

fn encode_shape_record(sh: &VisioShape) -> (u16, Vec<u8>) {
    let tag = match sh.kind {
        ShapeKind::Connector1D => VSD_SHAPE_1D,
        ShapeKind::Group => VSD_SHAPE_GROUP,
        _ => VSD_SHAPE_2D,
    };
    let mut out = Vec::new();
    // Shape ID + Master ID
    let mut id_hdr = [0u8; 12];
    id_hdr[0..8].copy_from_slice(&sh.id.to_le_bytes());
    id_hdr[8..12].copy_from_slice(&sh.master_id.unwrap_or(0).to_le_bytes());
    write_record(&mut out, tag, &id_hdr);
    write_record(&mut out, VSD_NAME_UTF8, sh.name.as_bytes());

    // XFormData (`PinX`, `PinY`, `Width`, `Height`, `LocPinX`, `LocPinY`, `Angle`)
    let ss = &sh.sheet;
    let mut xf = Vec::with_capacity(56);
    let w = ss.get_f64("Width").unwrap_or(1.0);
    let h = ss.get_f64("Height").unwrap_or(1.0);
    for v in [
        ss.get_f64("PinX").unwrap_or(0.0),
        ss.get_f64("PinY").unwrap_or(0.0),
        w,
        h,
        ss.get_f64("LocPinX").unwrap_or(w * 0.5),
        ss.get_f64("LocPinY").unwrap_or(h * 0.5),
        ss.get_f64("Angle").unwrap_or(0.0),
    ] {
        xf.extend_from_slice(&v.to_le_bytes());
    }
    write_record(&mut out, VSD_XFORM_DATA, &xf);

    if sh.kind == ShapeKind::Connector1D {
        let mut x1d = Vec::with_capacity(32);
        for v in [
            ss.get_f64("BeginX").unwrap_or(0.0),
            ss.get_f64("BeginY").unwrap_or(0.0),
            ss.get_f64("EndX").unwrap_or(1.0),
            ss.get_f64("EndY").unwrap_or(1.0),
        ] {
            x1d.extend_from_slice(&v.to_le_bytes());
        }
        write_record(&mut out, VSD_XFORM_1D, &x1d);
    }

    if let Some(fc) = ss.get_color("FillForegnd") {
        write_record(&mut out, VSD_FILL_FMT, &[fc.r, fc.g, fc.b, fc.a]);
    }
    if let Some(lc) = ss.get_color("LineColor") {
        let lw = ss.get_f64("LineWeight").unwrap_or(1.0 / 72.0);
        let mut lp = Vec::with_capacity(12);
        lp.extend_from_slice(&lw.to_le_bytes());
        lp.extend_from_slice(&[lc.r, lc.g, lc.b, lc.a]);
        write_record(&mut out, VSD_LINE_FMT, &lp);
    }

    if !sh.text.is_empty() {
        let utf16: Vec<u8> = sh
            .text
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        write_record(&mut out, VSD_TEXT_UTF16, &utf16);
    }

    // Geometry rows
    for g in &sh.sheet.geometries {
        let flags = (if g.no_fill == "TRUE" || g.no_fill == "1" { 1u8 } else { 0 })
            | (if g.no_line == "TRUE" || g.no_line == "1" { 2u8 } else { 0 });
        write_record(&mut out, VSD_GEOM_HDR, &[g.index as u8, flags]);
        let prefix = format!("Geometry{}", g.index);
        for row in &g.rows {
            if row.deleted {
                continue;
            }
            let x = ss.get_f64(&format!("{prefix}.X{}", row.ix)).unwrap_or(0.0);
            let y = ss.get_f64(&format!("{prefix}.Y{}", row.ix)).unwrap_or(0.0);
            match row.kind {
                vexio_shapesheet::GeometryRowKind::MoveTo => {
                    let mut p = [0u8; 16];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    write_record(&mut out, VSD_MOVE_TO, &p);
                }
                vexio_shapesheet::GeometryRowKind::ArcTo => {
                    let bow = ss.get_f64(&format!("{prefix}.A{}", row.ix)).unwrap_or(0.0);
                    let mut p = [0u8; 24];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    p[16..24].copy_from_slice(&bow.to_le_bytes());
                    write_record(&mut out, VSD_ARC_TO, &p);
                }
                vexio_shapesheet::GeometryRowKind::Ellipse => {
                    let mut p = [0u8; 16];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    write_record(&mut out, VSD_ELLIPSE, &p);
                }
                _ => {
                    let mut p = [0u8; 16];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    write_record(&mut out, VSD_LINE_TO, &p);
                }
            }
        }
    }

    for child in &sh.children {
        let (ctag, cbytes) = encode_shape_record(child);
        write_record(&mut out, ctag, &cbytes);
    }
    (tag, out)
}

fn parse_shape_record(outer_tag: u16, data: &[u8]) -> VisioShape {
    let kind = match outer_tag {
        VSD_SHAPE_1D => ShapeKind::Connector1D,
        VSD_SHAPE_GROUP => ShapeKind::Group,
        _ => ShapeKind::Shape2D,
    };
    let mut id = 1u64;
    let mut master_id = None;
    let mut name = "Sheet.1".to_string();
    let mut sheet = if kind == ShapeKind::Connector1D {
        ShapeSheet::new_1d_connector(0.0, 0.0, 1.0, 1.0)
    } else {
        ShapeSheet::new_2d(1.0, 1.0, 2.0, 1.0)
    };
    sheet.geometries.clear();
    let mut cur_geom: Option<GeometrySection> = None;
    let mut text = String::new();
    let mut children = Vec::new();
    let mut saw_own_header = false;

    for (rt, payload) in iter_records(data) {
        match rt {
            t if (t == VSD_SHAPE_2D || t == VSD_SHAPE_1D || t == VSD_SHAPE_GROUP) && !saw_own_header => {
                saw_own_header = true;
                if payload.len() >= 12 {
                    id = u64::from_le_bytes(payload[0..8].try_into().unwrap());
                    let mid = u32::from_le_bytes(payload[8..12].try_into().unwrap());
                    if mid != 0 {
                        master_id = Some(mid);
                    }
                }
            }
            VSD_SHAPE_2D | VSD_SHAPE_1D | VSD_SHAPE_GROUP => {
                children.push(parse_shape_record(rt, payload));
            }
            VSD_NAME_UTF8 => {
                name = String::from_utf8_lossy(payload).into_owned();
            }
            VSD_XFORM_DATA if payload.len() >= 56 => {
                let px = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                let py = f64::from_le_bytes(payload[8..16].try_into().unwrap());
                let w = f64::from_le_bytes(payload[16..24].try_into().unwrap());
                let h = f64::from_le_bytes(payload[24..32].try_into().unwrap());
                let lpx = f64::from_le_bytes(payload[32..40].try_into().unwrap());
                let lpy = f64::from_le_bytes(payload[40..48].try_into().unwrap());
                let ang = f64::from_le_bytes(payload[48..56].try_into().unwrap());
                if kind != ShapeKind::Connector1D {
                    sheet.set_formula("PinX", format!("{px} in"), Some(Unit::Inch));
                    sheet.set_formula("PinY", format!("{py} in"), Some(Unit::Inch));
                    sheet.set_formula("Width", format!("{w} in"), Some(Unit::Inch));
                    sheet.set_formula("Height", format!("{h} in"), Some(Unit::Inch));
                    sheet.set_formula("LocPinX", format!("{lpx} in"), Some(Unit::Inch));
                    sheet.set_formula("LocPinY", format!("{lpy} in"), Some(Unit::Inch));
                    sheet.set_formula("Angle", format!("{ang} rad"), Some(Unit::Radian));
                }
            }
            VSD_XFORM_1D if payload.len() >= 32 => {
                let bx = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                let by = f64::from_le_bytes(payload[8..16].try_into().unwrap());
                let ex = f64::from_le_bytes(payload[16..24].try_into().unwrap());
                let ey = f64::from_le_bytes(payload[24..32].try_into().unwrap());
                sheet.set_formula("BeginX", format!("{bx} in"), Some(Unit::Inch));
                sheet.set_formula("BeginY", format!("{by} in"), Some(Unit::Inch));
                sheet.set_formula("EndX", format!("{ex} in"), Some(Unit::Inch));
                sheet.set_formula("EndY", format!("{ey} in"), Some(Unit::Inch));
            }
            VSD_FILL_FMT if payload.len() >= 4 => {
                sheet.set_formula(
                    "FillForegnd",
                    format!("=RGB({},{},{})", payload[0], payload[1], payload[2]),
                    Some(Unit::Color),
                );
            }
            VSD_LINE_FMT if payload.len() >= 12 => {
                let lw_in = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                sheet.set_formula(
                    "LineWeight",
                    format!("{} pt", lw_in * 72.0),
                    Some(Unit::Point),
                );
                sheet.set_formula(
                    "LineColor",
                    format!("=RGB({},{},{})", payload[8], payload[9], payload[10]),
                    Some(Unit::Color),
                );
            }
            VSD_TEXT_UTF16 => {
                let units: Vec<u16> = payload
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                text = String::from_utf16_lossy(&units);
            }
            VSD_GEOM_HDR if payload.len() >= 2 => {
                if let Some(prev) = cur_geom.take() {
                    sheet.add_geometry(prev);
                }
                let mut g = GeometrySection::new(payload[0].max(1) as u32);
                g.no_fill = if (payload[1] & 1) != 0 {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                };
                g.no_line = if (payload[1] & 2) != 0 {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                };
                cur_geom = Some(g);
            }
            VSD_MOVE_TO if payload.len() >= 16 => {
                let x = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                let y = f64::from_le_bytes(payload[8..16].try_into().unwrap());
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows
                    .push(GeometryRow::move_to(ix, format!("{x} in"), format!("{y} in")));
            }
            VSD_LINE_TO if payload.len() >= 16 => {
                let x = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                let y = f64::from_le_bytes(payload[8..16].try_into().unwrap());
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows
                    .push(GeometryRow::line_to(ix, format!("{x} in"), format!("{y} in")));
            }
            VSD_ARC_TO if payload.len() >= 24 => {
                let x = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                let y = f64::from_le_bytes(payload[8..16].try_into().unwrap());
                let bow = f64::from_le_bytes(payload[16..24].try_into().unwrap());
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows.push(GeometryRow::arc_to(
                    ix,
                    format!("{x} in"),
                    format!("{y} in"),
                    format!("{bow} in"),
                ));
            }
            _ => {}
        }
    }
    if let Some(last) = cur_geom.take() {
        sheet.add_geometry(last);
    }
    if sheet.geometries.is_empty() && kind == ShapeKind::Shape2D {
        sheet.add_geometry(GeometrySection::rectangle(1));
    }

    VisioShape {
        id,
        name: name.clone(),
        name_u: name,
        kind,
        master_id,
        master_shape_id: master_id.map(|_| 1),
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text,
        sheet,
        children,
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    }
}

// ============================================================================
// Real Microsoft `[MS-VSD]` v6–v11 (Visio 2000 / 2002 / 2003 / 2007 / 2010)
// 18-Byte `Pointer6` & Packed `ChunkHeader` Stream Parser
// ============================================================================

#[derive(Debug, Clone, Copy)]
struct MsVsdPointer6 {
    type_id: u32,
    offset: u32,
    length: u32,
    format: u16,
}

fn read_ms_vsd_pointer6(buf: &[u8], off: usize) -> Option<MsVsdPointer6> {
    if off.checked_add(18)? > buf.len() {
        return None;
    }
    let type_id = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]]);
    let offset = u32::from_le_bytes([buf[off + 8], buf[off + 9], buf[off + 10], buf[off + 11]]);
    let length = u32::from_le_bytes([buf[off + 12], buf[off + 13], buf[off + 14], buf[off + 15]]);
    let format = u16::from_le_bytes([buf[off + 16], buf[off + 17]]);
    Some(MsVsdPointer6 {
        type_id,
        offset,
        length,
        format,
    })
}

fn is_real_ms_vsd_pointer6_header(buf: &[u8]) -> bool {
    if buf.len() < 0x36 {
        return false;
    }
    // Check synthetic 12-byte pointer first: in our synthetic writer, `buf[0x26..0x28]` is `0x0002` or `0x0000`
    // AND `offset + length == buf.len()` at `0x28..0x30`.
    let synth_fmt = u16::from_le_bytes([buf[0x26], buf[0x27]]);
    let synth_off = u32::from_le_bytes([buf[0x28], buf[0x29], buf[0x2A], buf[0x2B]]) as usize;
    let synth_len = u32::from_le_bytes([buf[0x2C], buf[0x2D], buf[0x2E], buf[0x2F]]) as usize;
    if (synth_fmt == 0x0002 || synth_fmt == 0x0000)
        && synth_off >= 0x40
        && synth_len > 0
        && synth_off.saturating_add(synth_len) == buf.len()
    {
        return false;
    }
    if let Some(p6) = read_ms_vsd_pointer6(buf, 0x24) {
        let end = (p6.offset as usize).saturating_add(p6.length as usize);
        p6.type_id == (VSD_TRAILER as u32) && p6.offset >= 0x36 && p6.length > 0 && end <= buf.len()
    } else {
        false
    }
}

fn extract_ms_vsd_stream(vd: &[u8], ptr: MsVsdPointer6) -> Option<(Vec<u8>, Vec<MsVsdPointer6>)> {
    if ptr.type_id == 0 || ptr.length == 0 {
        return Some((Vec::new(), Vec::new()));
    }
    let start = ptr.offset as usize;
    let end = start.checked_add(ptr.length as usize)?;
    if end > vd.len() {
        return None;
    }
    let raw = &vd[start..end];
    let body = if (ptr.format & 0x0002) != 0 {
        let dec = decompress_vsd_stream(raw);
        if dec.len() >= 4 {
            dec[4..].to_vec()
        } else {
            dec
        }
    } else {
        raw.to_vec()
    };

    let mut ptrs = Vec::new();
    if (ptr.format & 0x0010) != 0 && (ptr.format & 0x0080) == 0 && body.len() >= 4 {
        let list_off = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
        if let Some(hdr_end) = list_off.checked_add(8) {
            if hdr_end <= body.len() {
                let cnt = u32::from_le_bytes([
                    body[list_off],
                    body[list_off + 1],
                    body[list_off + 2],
                    body[list_off + 3],
                ]) as usize;
                for i in 0..cnt.min(16_384) {
                    let poff = hdr_end + i * 18;
                    if let Some(cp) = read_ms_vsd_pointer6(&body, poff) {
                        ptrs.push(cp);
                    } else {
                        break;
                    }
                }
            }
        }
    }
    Some((body, ptrs))
}

fn decode_ms_vsd_string(bytes: &[u8], version: u8, skip_prefix: usize) -> String {
    if bytes.len() <= skip_prefix {
        return String::new();
    }
    let slice = &bytes[skip_prefix..];
    if version >= 11 {
        let mut units = Vec::with_capacity(slice.len() / 2);
        for pair in slice.chunks_exact(2) {
            let u = u16::from_le_bytes([pair[0], pair[1]]);
            if u == 0 {
                break;
            }
            units.push(u);
        }
        String::from_utf16_lossy(&units).trim().to_string()
    } else {
        let nul = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
        slice[..nul]
            .iter()
            .map(|&b| b as char)
            .collect::<String>()
            .trim()
            .to_string()
    }
}

fn read_ms_vsd_cell_f64(payload: &[u8], off: usize) -> Option<(u8, f64)> {
    if off.checked_add(9)? > payload.len() {
        return None;
    }
    let unit_code = payload[off];
    let val = f64::from_le_bytes(payload[off + 1..off + 9].try_into().ok()?);
    if val.is_finite() {
        Some((unit_code, val))
    } else {
        None
    }
}

fn infer_ms_vsd_foreign_media(raw: &[u8]) -> Option<ForeignMediaData> {
    if raw.len() < 8 {
        return None;
    }
    // 1. Standard Windows Enhanced Metafile (`ENHMETAHEADER`: `iType == 1` at `0..4`, `" EMF"` at `40..44`)
    if raw.len() >= 44 && &raw[40..44] == b" EMF" {
        return Some(ForeignMediaData {
            media_type: "EMF".into(),
            rel_id: None,
            data: raw.to_vec(),
        });
    }
    // Scan small wrapper prefix (up to 64 bytes) for embedded EMF / WMF / PNG / JPEG / BMP signatures
    let scan_limit = raw.len().min(96);
    if let Some(emf_sig) = raw[..scan_limit.min(raw.len())]
        .windows(4)
        .position(|w| w == b" EMF")
    {
        if emf_sig >= 40 {
            let start = emf_sig - 40;
            return Some(ForeignMediaData {
                media_type: "EMF".into(),
                rel_id: None,
                data: raw[start..].to_vec(),
            });
        }
    }
    if let Some(png_pos) = raw[..scan_limit]
        .windows(8)
        .position(|w| w == b"\x89PNG\r\n\x1a\n")
    {
        return Some(ForeignMediaData {
            media_type: "PNG".into(),
            rel_id: None,
            data: raw[png_pos..].to_vec(),
        });
    }
    if let Some(jpg_pos) = raw[..scan_limit]
        .windows(3)
        .position(|w| w == [0xFF, 0xD8, 0xFF])
    {
        return Some(ForeignMediaData {
            media_type: "JPEG".into(),
            rel_id: None,
            data: raw[jpg_pos..].to_vec(),
        });
    }
    if let Some(wmf_pos) = raw[..scan_limit]
        .windows(4)
        .position(|w| w == [0xD7, 0xCD, 0xC6, 0x9A] || w == [0x01, 0x00, 0x09, 0x00])
    {
        return Some(ForeignMediaData {
            media_type: "WMF".into(),
            rel_id: None,
            data: raw[wmf_pos..].to_vec(),
        });
    }
    if raw.starts_with(b"BM") {
        return Some(ForeignMediaData {
            media_type: "BMP".into(),
            rel_id: None,
            data: raw.to_vec(),
        });
    }
    // Device-Independent Bitmap (`BITMAPINFOHEADER` `biSize == 40` at offset 0) — wrap in 14-byte `BITMAPFILEHEADER`!
    if raw.len() >= 40 {
        let bi_size = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
        let planes = u16::from_le_bytes([raw[12], raw[13]]);
        let bpp = u16::from_le_bytes([raw[14], raw[15]]);
        if bi_size == 40 && planes == 1 && matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32) {
            let clr_used = u32::from_le_bytes([raw[32], raw[33], raw[34], raw[35]]);
            let pal_entries = if bpp <= 8 {
                if clr_used > 0 { clr_used } else { 1u32 << bpp }
            } else {
                0
            };
            let pixel_off = 14u32 + 40u32 + pal_entries * 4;
            let file_sz = 14u32 + (raw.len() as u32);
            let mut bmp = Vec::with_capacity(raw.len() + 14);
            bmp.extend_from_slice(b"BM");
            bmp.extend_from_slice(&file_sz.to_le_bytes());
            bmp.extend_from_slice(&0u32.to_le_bytes());
            bmp.extend_from_slice(&pixel_off.to_le_bytes());
            bmp.extend_from_slice(raw);
            return Some(ForeignMediaData {
                media_type: "BMP".into(),
                rel_id: None,
                data: bmp,
            });
        }
    }
    None
}

#[derive(Debug, Clone)]
struct MsVsdChunk {
    ctype: u32,
    level: u16,
    payload: Vec<u8>,
}

fn is_ms_vsd_list_chunk(ctype: u32) -> bool {
    matches!(
        ctype,
        0x2C | 0x46..=0x4F | 0x64..=0x76
    )
}

fn calc_ms_vsd_chunk_trailer(version: u8, ctype: u32, clist: u32, cunk: u8) -> usize {
    let has_list = clist != 0 || is_ms_vsd_list_chunk(ctype);
    let mut tr = if has_list { 8usize } else { 0usize };
    if version >= 11
        && (has_list
            || matches!(cunk, 0x50 | 0x51 | 0x54 | 0x55 | 0x57)
            || (ctype == 0x1F && cunk == 0))
    {
        tr += 4;
    }
    tr
}

fn walk_ms_vsd_dx_chunks(body: &[u8], version: u8, root_fmt: u16) -> Vec<MsVsdChunk> {
    if body.len() < 15 {
        return Vec::new();
    }
    let first_u32 = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
    let mut off = if first_u32 == 0xFFFF_FFFF {
        let r_list = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
        let r_len = u32::from_le_bytes([body[8], body[9], body[10], body[11]]) as usize;
        let r_unk = body[14];
        let r_tr = calc_ms_vsd_chunk_trailer(
            version,
            0x48,
            r_list,
            if r_unk != 0 { r_unk } else { (root_fmt & 0xFF) as u8 },
        );
        15usize.saturating_add(r_len).saturating_add(r_tr)
    } else {
        0usize
    };

    let mut out = Vec::new();
    while off.saturating_add(19) <= body.len() {
        let ctype = u32::from_le_bytes([body[off], body[off + 1], body[off + 2], body[off + 3]]);
        let clist = u32::from_le_bytes([body[off + 8], body[off + 9], body[off + 10], body[off + 11]]);
        let clen = u32::from_le_bytes([body[off + 12], body[off + 13], body[off + 14], body[off + 15]]) as usize;
        let clvl = u16::from_le_bytes([body[off + 16], body[off + 17]]);
        let cunk = body[off + 18];
        if ctype == 0 || ctype > 0x0200 || off.saturating_add(19).saturating_add(clen) > body.len() {
            break;
        }
        let base_tr = calc_ms_vsd_chunk_trailer(version, ctype, clist, cunk);
        let candidates = [base_tr, base_tr + 4, base_tr.saturating_sub(4), 0, 4, 8, 12];
        let mut chosen_tr = base_tr;
        for cand in candidates {
            let nxt = off + 19 + clen + cand;
            if nxt == body.len() {
                chosen_tr = cand;
                break;
            }
            if nxt + 19 <= body.len() {
                let nt = u32::from_le_bytes([body[nxt], body[nxt + 1], body[nxt + 2], body[nxt + 3]]);
                let nlen = u32::from_le_bytes([body[nxt + 12], body[nxt + 13], body[nxt + 14], body[nxt + 15]]) as usize;
                let nlvl = u16::from_le_bytes([body[nxt + 16], body[nxt + 17]]);
                let nunk = body[nxt + 18];
                if (0x08..=0x0100).contains(&nt)
                    && nlvl <= 16
                    && (nunk == 0 || (0x40..=0x7F).contains(&nunk))
                    && nxt + 19 + nlen <= body.len()
                {
                    chosen_tr = cand;
                    break;
                }
            }
        }
        let payload = body[off + 19..off + 19 + clen].to_vec();
        out.push(MsVsdChunk {
            ctype,
            level: clvl,
            payload,
        });
        off = off + 19 + clen + chosen_tr;
    }
    out
}

fn apply_ms_vsd_record_to_shape(
    ctype: u32,
    payload: &[u8],
    version: u8,
    shape: &mut VisioShape,
    cur_geom: &mut Option<GeometrySection>,
) {
    match ctype {
        0x9B => {
            // XFormData: PinX [0..9], PinY [9..18], Width [18..27], Height [27..36], LocPinX [36..45], LocPinY [45..54], Angle [54..63]
            if let Some((_, px)) = read_ms_vsd_cell_f64(payload, 0) {
                shape.sheet.set_formula("PinX", format!("{} in", format_clean_f64(px)), Some(Unit::Inch));
            }
            if let Some((_, py)) = read_ms_vsd_cell_f64(payload, 9) {
                shape.sheet.set_formula("PinY", format!("{} in", format_clean_f64(py)), Some(Unit::Inch));
            }
            if let Some((_, w)) = read_ms_vsd_cell_f64(payload, 18) {
                if w.abs() > 1e-6 {
                    shape.sheet.set_formula("Width", format!("{} in", format_clean_f64(w.abs())), Some(Unit::Inch));
                }
            }
            if let Some((_, h)) = read_ms_vsd_cell_f64(payload, 27) {
                if h.abs() > 1e-6 {
                    shape.sheet.set_formula("Height", format!("{} in", format_clean_f64(h.abs())), Some(Unit::Inch));
                }
            }
            if let Some((_, lpx)) = read_ms_vsd_cell_f64(payload, 36) {
                shape.sheet.set_formula("LocPinX", format!("{} in", format_clean_f64(lpx)), Some(Unit::Inch));
            }
            if let Some((_, lpy)) = read_ms_vsd_cell_f64(payload, 45) {
                shape.sheet.set_formula("LocPinY", format!("{} in", format_clean_f64(lpy)), Some(Unit::Inch));
            }
            if let Some((_, ang)) = read_ms_vsd_cell_f64(payload, 54) {
                if ang.abs() > 1e-7 {
                    shape.sheet.set_formula("Angle", format!("{} rad", format_clean_f64(ang)), Some(Unit::Radian));
                }
            }
            if payload.len() >= 65 {
                if payload[63] != 0 {
                    shape.sheet.set_formula("FlipX", "TRUE", Some(Unit::Boolean));
                }
                if payload[64] != 0 {
                    shape.sheet.set_formula("FlipY", "TRUE", Some(Unit::Boolean));
                }
            }
        }
        0x99 => {
            // XForm1D: BeginX [0..9], BeginY [9..18], EndX [18..27], EndY [27..36]
            shape.kind = ShapeKind::Connector1D;
            if let Some((_, bx)) = read_ms_vsd_cell_f64(payload, 0) {
                shape.sheet.set_formula("BeginX", format!("{} in", format_clean_f64(bx)), Some(Unit::Inch));
            }
            if let Some((_, by)) = read_ms_vsd_cell_f64(payload, 9) {
                shape.sheet.set_formula("BeginY", format!("{} in", format_clean_f64(by)), Some(Unit::Inch));
            }
            if let Some((_, ex)) = read_ms_vsd_cell_f64(payload, 18) {
                shape.sheet.set_formula("EndX", format!("{} in", format_clean_f64(ex)), Some(Unit::Inch));
            }
            if let Some((_, ey)) = read_ms_vsd_cell_f64(payload, 27) {
                shape.sheet.set_formula("EndY", format!("{} in", format_clean_f64(ey)), Some(Unit::Inch));
            }
        }
        0x9C => {
            // TextXForm: TxtPinX [0..9], TxtPinY [9..18], TxtWidth [18..27], TxtHeight [27..36], TxtLocPinX [36..45], TxtLocPinY [45..54], TxtAngle [54..63]
            if let Some((_, tpx)) = read_ms_vsd_cell_f64(payload, 0) {
                shape.sheet.set_formula("TxtPinX", format!("{} in", format_clean_f64(tpx)), Some(Unit::Inch));
            }
            if let Some((_, tpy)) = read_ms_vsd_cell_f64(payload, 9) {
                shape.sheet.set_formula("TxtPinY", format!("{} in", format_clean_f64(tpy)), Some(Unit::Inch));
            }
            if let Some((_, tw)) = read_ms_vsd_cell_f64(payload, 18) {
                if tw.abs() > 0.02 {
                    shape.sheet.set_formula("TxtWidth", format!("{} in", format_clean_f64(tw.abs())), Some(Unit::Inch));
                }
            }
            if let Some((_, th)) = read_ms_vsd_cell_f64(payload, 27) {
                if th.abs() > 0.02 {
                    shape.sheet.set_formula("TxtHeight", format!("{} in", format_clean_f64(th.abs())), Some(Unit::Inch));
                }
            }
            if let Some((_, tlx)) = read_ms_vsd_cell_f64(payload, 36) {
                shape.sheet.set_formula("TxtLocPinX", format!("{} in", format_clean_f64(tlx)), Some(Unit::Inch));
            }
            if let Some((_, tly)) = read_ms_vsd_cell_f64(payload, 45) {
                shape.sheet.set_formula("TxtLocPinY", format!("{} in", format_clean_f64(tly)), Some(Unit::Inch));
            }
        }
        0x85 => {
            // Line: LineWeight [0..9], LineColor [9..13], LinePattern [13]
            if let Some((_, lw)) = read_ms_vsd_cell_f64(payload, 0) {
                if lw > 0.0 {
                    shape.sheet.set_formula(
                        "LineWeight",
                        format!("{} pt", format_clean_f64((lw * 72.0).clamp(0.25, 36.0))),
                        Some(Unit::Point),
                    );
                }
            }
            if payload.len() >= 14 {
                let pat = payload[13];
                shape.sheet.set_formula("LinePattern", pat.to_string(), Some(Unit::Number));
                if pat > 0 {
                    shape.sheet.set_formula(
                        "LineColor",
                        format!("=RGB({},{},{})", payload[9], payload[10], payload[11]),
                        Some(Unit::Color),
                    );
                }
            }
        }
        0x86 => {
            // Fill: FillForegnd [0..4], FillForegndTrans [4..13], FillBkgnd [13..17], FillBkgndTrans [17..26], FillPattern [26]
            if payload.len() >= 27 {
                let pat = payload[26];
                shape.sheet.set_formula("FillPattern", pat.to_string(), Some(Unit::Number));
                if pat > 0 {
                    shape.sheet.set_formula(
                        "FillForegnd",
                        format!("=RGB({},{},{})", payload[0], payload[1], payload[2]),
                        Some(Unit::Color),
                    );
                    shape.sheet.set_formula(
                        "FillBkgnd",
                        format!("=RGB({},{},{})", payload[13], payload[14], payload[15]),
                        Some(Unit::Color),
                    );
                }
            }
        }
        0x0C => {
            if let Some(fm) = infer_ms_vsd_foreign_media(payload) {
                shape.kind = ShapeKind::ForeignObject;
                shape.foreign_data = Some(fm);
            }
        }
        0x08 => {
            let txt = decode_ms_vsd_string(payload, 6, 8.min(payload.len()));
            if !txt.is_empty() {
                shape.text = txt;
            }
        }
        0x0E => {
            let txt = decode_ms_vsd_string(payload, version.max(11), 8.min(payload.len()));
            if !txt.is_empty() {
                shape.text = txt;
            }
        }
        0x89 => {
            if let Some(prev) = cur_geom.take() {
                if !prev.rows.is_empty() {
                    shape.sheet.add_geometry(prev);
                }
            }
            let next_ix = (shape.sheet.geometries.len() as u32) + 1;
            let mut g = GeometrySection::new(next_ix);
            if !payload.is_empty() {
                let flags = payload[0];
                g.no_fill = if (flags & 0x01) != 0 { "TRUE".into() } else { "FALSE".into() };
                g.no_line = if (flags & 0x02) != 0 { "TRUE".into() } else { "FALSE".into() };
                g.no_show = if (flags & 0x04) != 0 { "TRUE".into() } else { "FALSE".into() };
            }
            *cur_geom = Some(g);
        }
        0x8A => {
            if let (Some((_, x)), Some((_, y))) =
                (read_ms_vsd_cell_f64(payload, 0), read_ms_vsd_cell_f64(payload, 9))
            {
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows.push(GeometryRow::move_to(
                    ix,
                    format!("{} in", format_clean_f64(x)),
                    format!("{} in", format_clean_f64(y)),
                ));
            }
        }
        0x8B => {
            if let (Some((_, x)), Some((_, y))) =
                (read_ms_vsd_cell_f64(payload, 0), read_ms_vsd_cell_f64(payload, 9))
            {
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows.push(GeometryRow::line_to(
                    ix,
                    format!("{} in", format_clean_f64(x)),
                    format!("{} in", format_clean_f64(y)),
                ));
            }
        }
        0x8C => {
            if let (Some((_, x)), Some((_, y)), Some((_, bow))) = (
                read_ms_vsd_cell_f64(payload, 0),
                read_ms_vsd_cell_f64(payload, 9),
                read_ms_vsd_cell_f64(payload, 18),
            ) {
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows.push(GeometryRow::arc_to(
                    ix,
                    format!("{} in", format_clean_f64(x)),
                    format!("{} in", format_clean_f64(y)),
                    format!("{} in", format_clean_f64(bow)),
                ));
            }
        }
        0x8F | 0x90 => {
            if let (Some((_, x)), Some((_, y))) =
                (read_ms_vsd_cell_f64(payload, 0), read_ms_vsd_cell_f64(payload, 9))
            {
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                let kind = if ctype == 0x8F {
                    GeometryRowKind::Ellipse
                } else {
                    GeometryRowKind::EllipticalArcTo
                };
                let mut row = GeometryRow {
                    ix,
                    kind,
                    x: format!("{} in", format_clean_f64(x)),
                    y: format!("{} in", format_clean_f64(y)),
                    a: read_ms_vsd_cell_f64(payload, 18).map(|(_, v)| format!("{} in", format_clean_f64(v))),
                    b: read_ms_vsd_cell_f64(payload, 27).map(|(_, v)| format!("{} in", format_clean_f64(v))),
                    c: read_ms_vsd_cell_f64(payload, 36).map(|(_, v)| {
                        if ctype == 0x8F {
                            format!("{} in", format_clean_f64(v))
                        } else {
                            format!("{} rad", format_clean_f64(v))
                        }
                    }),
                    d: read_ms_vsd_cell_f64(payload, 45).map(|(_, v)| {
                        if ctype == 0x8F {
                            format!("{} in", format_clean_f64(v))
                        } else {
                            format_clean_f64(v)
                        }
                    }),
                    e: None,
                    deleted: false,
                };
                if row.a.is_none() {
                    row.kind = GeometryRowKind::LineTo;
                }
                g.rows.push(row);
            }
        }
        0xEE | 0xED => {
            if let (Some((_, x)), Some((_, y))) =
                (read_ms_vsd_cell_f64(payload, 0), read_ms_vsd_cell_f64(payload, 9))
            {
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows.push(GeometryRow {
                    ix,
                    kind: if ctype == 0xEE {
                        GeometryRowKind::RelMoveTo
                    } else {
                        GeometryRowKind::RelLineTo
                    },
                    x: format_clean_f64(x),
                    y: format_clean_f64(y),
                    a: None,
                    b: None,
                    c: None,
                    d: None,
                    e: None,
                    deleted: false,
                });
            }
        }
        _ => {}
    }
}

fn parse_ms_vsd_pointer_shape(
    vd: &[u8],
    ptr: MsVsdPointer6,
    version: u8,
    next_id: &mut u64,
    depth: u8,
) -> Option<VisioShape> {
    if depth > 16 {
        return None;
    }
    let my_id = *next_id;
    *next_id += 1;

    let mut shape = VisioShape::new_rect(my_id, format!("Shape.{my_id}"), 1.0, 1.0, 1.5, 1.0);
    shape.sheet.geometries.clear();
    shape.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    shape.sheet.set_formula("FillPattern", "0", Some(Unit::Number));

    let mut cur_geom: Option<GeometrySection> = None;

    if (ptr.format & 0x0080) != 0 {
        let (body, _) = extract_ms_vsd_stream(vd, ptr)?;
        let chunks = walk_ms_vsd_dx_chunks(&body, version, ptr.format);
        for ch in chunks {
            apply_ms_vsd_record_to_shape(ch.ctype, &ch.payload, version, &mut shape, &mut cur_geom);
        }
    } else {
        let (_, sub_ptrs) = extract_ms_vsd_stream(vd, ptr)?;
        for sp in sub_ptrs {
            match sp.type_id {
                0x4D => {
                    // Nested Shapes container inside GroupShape
                    if let Some((_, child_ptrs)) = extract_ms_vsd_stream(vd, sp) {
                        for cp in child_ptrs {
                            if matches!(cp.type_id, 0x47..=0x4F) {
                                if let Some(child_sh) =
                                    parse_ms_vsd_pointer_shape(vd, cp, version, next_id, depth + 1)
                                {
                                    shape.children.push(child_sh);
                                }
                            }
                        }
                    }
                }
                0x64..=0x76 => {
                    if sp.type_id == 0x6C {
                        if let Some(prev) = cur_geom.take() {
                            if !prev.rows.is_empty() {
                                shape.sheet.add_geometry(prev);
                            }
                        }
                    }
                    if let Some((_, sec_ptrs)) = extract_ms_vsd_stream(vd, sp) {
                        for item_ptr in sec_ptrs {
                            if let Some((payload, _)) = extract_ms_vsd_stream(vd, item_ptr) {
                                apply_ms_vsd_record_to_shape(
                                    item_ptr.type_id,
                                    &payload,
                                    version,
                                    &mut shape,
                                    &mut cur_geom,
                                );
                            }
                        }
                    }
                }
                other => {
                    if let Some((payload, _)) = extract_ms_vsd_stream(vd, sp) {
                        apply_ms_vsd_record_to_shape(
                            other,
                            &payload,
                            version,
                            &mut shape,
                            &mut cur_geom,
                        );
                    }
                }
            }
        }
    }

    if let Some(last_g) = cur_geom.take() {
        if !last_g.rows.is_empty() {
            shape.sheet.add_geometry(last_g);
        }
    }

    if !shape.children.is_empty() {
        shape.kind = ShapeKind::Group;
    } else if shape.foreign_data.is_some() {
        shape.kind = ShapeKind::ForeignObject;
        // Hide text by default on ForeignObject hardware stencils where `text` is only a part SKU and `HideText` was set in Misc
        shape.sheet.set_formula("HideText", "TRUE", Some(Unit::Boolean));
    }

    shape.sheet.sync_all_sections_to_cells();
    if shape.foreign_data.is_none()
        && shape.sheet.geometries.is_empty()
        && shape.children.is_empty()
        && shape.text.trim().is_empty()
    {
        return None;
    }
    Some(shape)
}

fn parse_real_ms_vsd_v6_v11(vd: &[u8], version: u8) -> Result<VisioDocument, LegacyVisioError> {
    let trailer_ptr = read_ms_vsd_pointer6(vd, 0x24).ok_or_else(|| {
        LegacyVisioError::InvalidHeader("Truncated MS-VSD v6/v11 Trailer Pointer6".into())
    })?;
    let (_, top_ptrs) = extract_ms_vsd_stream(vd, trailer_ptr).ok_or_else(|| {
        LegacyVisioError::InvalidHeader("Corrupt MS-VSD v6/v11 Trailer stream".into())
    })?;

    // 1. Extract global `NameTable` (`0x32` -> `0x33` entries) so `0xC9` `NameIDX` resolves exact Master & Page names!
    let mut name_table: BTreeMap<u32, String> = BTreeMap::new();
    for tp in &top_ptrs {
        if tp.type_id == 0x32 {
            if let Some((_, n_ptrs)) = extract_ms_vsd_stream(vd, *tp) {
                for (idx, np) in n_ptrs.iter().enumerate() {
                    if np.type_id == 0x33 {
                        if let Some((nb, _)) = extract_ms_vsd_stream(vd, *np) {
                            let decoded = decode_ms_vsd_string(&nb, version, 4);
                            if !decoded.is_empty() {
                                name_table.insert(idx as u32, decoded);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut doc = VisioDocument::new_blank("Legacy Visio Stencil");
    doc.pages.clear();
    doc.masters.clear();
    let mut next_shape_id = 1u64;

    // 2. Parse `Masters` (`0x1D` -> `0x1E` `Master` + `0xC9` `NameIDX`)
    for tp in &top_ptrs {
        if tp.type_id == 0x1D {
            if let Some((_, m_ptrs)) = extract_ms_vsd_stream(vd, *tp) {
                // Collect ordered `NameIDX` (`0xC9`) entries: `[count: u32] + count * 13-byte [rec_idx: u32, name_id1: u32, name_id2: u32, flags: u8]`
                let mut ordered_master_names: Vec<String> = Vec::new();
                for mp in &m_ptrs {
                    if mp.type_id == 0xC9 {
                        if let Some((cb, _)) = extract_ms_vsd_stream(vd, *mp) {
                            if cb.len() >= 4 {
                                let cnt = u32::from_le_bytes([cb[0], cb[1], cb[2], cb[3]]) as usize;
                                for i in 0..cnt {
                                    let off = 4 + i * 13;
                                    if off + 13 <= cb.len() {
                                        let nid = u32::from_le_bytes([
                                            cb[off + 4],
                                            cb[off + 5],
                                            cb[off + 6],
                                            cb[off + 7],
                                        ]);
                                        if let Some(nm) = name_table.get(&nid) {
                                            ordered_master_names.push(nm.clone());
                                        } else {
                                            ordered_master_names.push(String::new());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                let mut master_seq = 0usize;
                for mp in m_ptrs {
                    if mp.type_id != 0x1E {
                        continue;
                    }
                    let my_mid = (master_seq + 1) as u32;
                    let indexed_name = ordered_master_names
                        .get(master_seq)
                        .cloned()
                        .unwrap_or_default();
                    master_seq += 1;

                    let Some((_, ms_sub)) = extract_ms_vsd_stream(vd, mp) else {
                        continue;
                    };
                    let mut prompt = String::new();
                    let mut shapes: Vec<VisioShape> = Vec::new();
                    let mut fallback_shapes: Vec<VisioShape> = Vec::new();

                    for sp in ms_sub {
                        match sp.type_id {
                            0x0A => {
                                if prompt.is_empty() {
                                    if let Some((pb, _)) = extract_ms_vsd_stream(vd, sp) {
                                        let s = decode_ms_vsd_string(&pb, version, 0);
                                        if !s.is_empty() {
                                            prompt = s;
                                        }
                                    }
                                }
                            }
                            0x4E => {
                                // Primary ForeignShape or Shapes container of the Master!
                                if let Some(sh) = parse_ms_vsd_pointer_shape(
                                    vd,
                                    sp,
                                    version,
                                    &mut next_shape_id,
                                    0,
                                ) {
                                    shapes.push(sh);
                                }
                            }
                            0x48 | 0x49 => {
                                if let Some(sh) = parse_ms_vsd_pointer_shape(
                                    vd,
                                    sp,
                                    version,
                                    &mut next_shape_id,
                                    0,
                                ) {
                                    fallback_shapes.push(sh);
                                }
                            }
                            _ => {}
                        }
                    }

                    if shapes.is_empty() && !fallback_shapes.is_empty() {
                        shapes = fallback_shapes;
                    }
                    if shapes.is_empty() {
                        continue;
                    }

                    let final_name = if !indexed_name.is_empty() {
                        indexed_name
                    } else if !prompt.is_empty() {
                        prompt.lines().next().unwrap_or(&prompt).trim().to_string()
                    } else {
                        format!("Master.{my_mid}")
                    };
                    for sh in &mut shapes {
                        sh.name = final_name.clone();
                        sh.name_u = final_name.clone();
                    }

                    doc.masters.push(VisioMaster {
                        id: my_mid,
                        name: final_name.clone(),
                        name_u: final_name,
                        base_id: format!("{{MSVSD-{my_mid:04}}}"),
                        unique_id: format!("{{MSVSD-U-{my_mid:04}}}"),
                        prompt,
                        icon_bytes: None,
                        page_sheet: ShapeSheet::new(),
                        shapes,
                    });
                }
            }
        }
    }

    // 3. Parse `Pages` (`0x27` -> `0x15` `Page`)
    for tp in &top_ptrs {
        if tp.type_id == 0x27 {
            if let Some((_, p_ptrs)) = extract_ms_vsd_stream(vd, *tp) {
                for pp in p_ptrs {
                    if pp.type_id != 0x15 {
                        continue;
                    }
                    let pid = (doc.pages.len() as u32) + 1;
                    let mut page = VisioPage::new(pid, format!("Page-{pid}"), 11.0, 8.5);
                    if (pp.format & 0x0080) != 0 {
                        if let Some((pbody, _)) = extract_ms_vsd_stream(vd, pp) {
                            let chunks = walk_ms_vsd_dx_chunks(&pbody, version, pp.format);
                            let mut active_sh: Option<VisioShape> = None;
                            let mut active_lvl: u16 = 0;
                            let mut active_geom: Option<GeometrySection> = None;
                            for ch in chunks {
                                if ch.ctype == 0x92 && ch.level <= 2 {
                                    if let Some((_, pw)) = read_ms_vsd_cell_f64(&ch.payload, 0) {
                                        if pw > 0.5 {
                                            page.page_sheet.set_formula(
                                                "PageWidth",
                                                format!("{} in", format_clean_f64(pw)),
                                                Some(Unit::Inch),
                                            );
                                        }
                                    }
                                    if let Some((_, ph)) = read_ms_vsd_cell_f64(&ch.payload, 9) {
                                        if ph > 0.5 {
                                            page.page_sheet.set_formula(
                                                "PageHeight",
                                                format!("{} in", format_clean_f64(ph)),
                                                Some(Unit::Inch),
                                            );
                                        }
                                    }
                                } else if matches!(ch.ctype, 0x47..=0x4F) && ch.level >= 1 {
                                    if let Some(mut prev) = active_sh.take() {
                                        if let Some(lg) = active_geom.take() {
                                            if !lg.rows.is_empty() {
                                                prev.sheet.add_geometry(lg);
                                            }
                                        }
                                        prev.sheet.sync_all_sections_to_cells();
                                        if prev.foreign_data.is_some()
                                            || !prev.sheet.geometries.is_empty()
                                            || !prev.text.trim().is_empty()
                                        {
                                            page.shapes.push(prev);
                                        }
                                    }
                                    let sid = next_shape_id;
                                    next_shape_id += 1;
                                    let mut nsh = VisioShape::new_rect(
                                        sid,
                                        format!("Shape.{sid}"),
                                        1.0,
                                        1.0,
                                        1.5,
                                        1.0,
                                    );
                                    nsh.sheet.geometries.clear();
                                    active_lvl = ch.level;
                                    active_sh = Some(nsh);
                                } else if let Some(sh) = active_sh.as_mut() {
                                    if ch.level > active_lvl {
                                        if ch.ctype == 0x6C {
                                            if let Some(lg) = active_geom.take() {
                                                if !lg.rows.is_empty() {
                                                    sh.sheet.add_geometry(lg);
                                                }
                                            }
                                        }
                                        apply_ms_vsd_record_to_shape(
                                            ch.ctype,
                                            &ch.payload,
                                            version,
                                            sh,
                                            &mut active_geom,
                                        );
                                    }
                                }
                            }
                            if let Some(mut last_sh) = active_sh.take() {
                                if let Some(lg) = active_geom.take() {
                                    if !lg.rows.is_empty() {
                                        last_sh.sheet.add_geometry(lg);
                                    }
                                }
                                last_sh.sheet.sync_all_sections_to_cells();
                                if last_sh.foreign_data.is_some()
                                    || !last_sh.sheet.geometries.is_empty()
                                    || !last_sh.text.trim().is_empty()
                                {
                                    page.shapes.push(last_sh);
                                }
                            }
                        }
                    } else if let Some((_, p_sub)) = extract_ms_vsd_stream(vd, pp) {
                        for sp in p_sub {
                            if matches!(sp.type_id, 0x0D | 0x4D) {
                                if let Some((_, sh_ptrs)) = extract_ms_vsd_stream(vd, sp) {
                                    for shp in sh_ptrs {
                                        if let Some(sh) = parse_ms_vsd_pointer_shape(
                                            vd,
                                            shp,
                                            version,
                                            &mut next_shape_id,
                                            0,
                                        ) {
                                            page.shapes.push(sh);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    doc.pages.push(page);
                }
            }
        }
    }

    if doc.pages.is_empty() {
        doc.pages.push(VisioPage::new(1, "Page-1", 11.0, 8.5));
    }

    ReactiveEngine::recalculate_document(&mut doc);
    Ok(doc)
}
