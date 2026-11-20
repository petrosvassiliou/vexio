//! OLE2 Compound File Binary (`.vsd`, `.vss`, `.vst`) MS-VSD v6/v11 parser and writer.

use crate::lz_visio::{compress_vsd_stream, decompress_vsd_stream};
use std::io::{Cursor, Read, Write};
use thiserror::Error;
use vexio_shapesheet::{
    GeometryRow, GeometrySection, GlueEndpoint, LayerDef, ReactiveEngine, ShapeKind,
    ShapeSheet, Unit, VisioDocument, VisioMaster, VisioPage, VisioShape,
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
    if version < 6 {
        return Err(LegacyVisioError::InvalidHeader(format!(
            "Unsupported pre-v6 Visio version {version}"
        )));
    }
    // Trailer pointer resides at 0x24: [type_id: u16, format: u16, offset: u32, length: u32]
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
