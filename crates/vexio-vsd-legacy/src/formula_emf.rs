//! Legacy Visio `v5` (Visio 5.0 / 5.0c) Stream Codec, MS-VSD RPN Binary Formula
//! Bytecode Compiler/Decompiler, and Standalone Offline `.emf` / `.wmf` Metafile Vector Parser.
//!
//! Implements:
//! - Pre-2000 Visio `v5` (`header[0x1A] == 5`) 10-byte short pointer directory reader & writer
//! - Deterministic postfix RPN formula compiler & precedence-aware infix decompiler over
//!   `vexio_shapesheet::Expr` (`0x03..0x40` opcodes)
//! - Pure-Rust Windows Enhanced Metafile (`.emf`, `0x464D4520` `" EMF"`) parser & encoder
//!   and Aldus Placeable (`0x9AC6CDD7`) / Standard Windows Metafile (`.wmf`) vector parser.

use crate::lz_visio::{compress_vsd_stream, decompress_vsd_stream};
use crate::vsd_cfb::{LegacyVisioError, VSD_HEADER_MAGIC};
use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use vexio_shapesheet::{
    BinaryOp, CellRefPath, Expr, ForeignMediaData, GeometryRow, GeometryRowKind, GeometrySection,
    GlueEndpoint, LayerDef, ReactiveEngine, RgbaColor, ShapeKind, ShapeSheet, SheetTarget, UnaryOp,
    Unit, VisioDocument, VisioMaster, VisioPage, VisioShape, format_clean_f64, parse_formula,
};

// ============================================================================
// 1. Visio `v5` (Visio 5.0 / 5.0c) Stream Reader & Writer (`version == 5`)
// ============================================================================

const VSD_TRAILER: u16 = 0x0014;
const VSD_DOC_SHEET: u16 = 0x0015;
const VSD_PAGE: u16 = 0x0016;
const VSD_MASTER: u16 = 0x001E;
const VSD_SHAPE_2D: u16 = 0x0046;
const VSD_SHAPE_1D: u16 = 0x0047;
const VSD_SHAPE_GROUP: u16 = 0x0048;
const VSD_XFORM_DATA: u16 = 0x009B;
const VSD_XFORM_1D: u16 = 0x009C;
const VSD_TEXT_ANSI: u16 = 0x0008;
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
const VSD_NAME_STR: u16 = 0x00D1;
const VSD_FORMULA_CELL: u16 = 0x00D8;

/// 10-byte Visio `v5` short pointer record: `[type_id: u16, offset: u32, length: u16, flags: u16]`.
#[derive(Debug, Clone, Copy)]
struct VsdV5ShortPointer {
    type_id: u16,
    offset: u32,
    length: u16,
    flags: u16,
}

impl VsdV5ShortPointer {
    fn is_compressed(self) -> bool {
        (self.flags & 0x0002) != 0
    }
}

fn read_v5_short_pointer(buf: &[u8], off: usize) -> Result<VsdV5ShortPointer, LegacyVisioError> {
    if off + 10 > buf.len() {
        return Err(LegacyVisioError::InvalidHeader(
            "Truncated 10-byte Visio v5 short pointer".into(),
        ));
    }
    let type_id = u16::from_le_bytes([buf[off], buf[off + 1]]);
    let offset = u32::from_le_bytes([buf[off + 2], buf[off + 3], buf[off + 4], buf[off + 5]]);
    let length = u16::from_le_bytes([buf[off + 6], buf[off + 7]]);
    let flags = u16::from_le_bytes([buf[off + 8], buf[off + 9]]);
    Ok(VsdV5ShortPointer {
        type_id,
        offset,
        length,
        flags,
    })
}

fn write_v5_short_pointer(dst: &mut [u8], off: usize, ptr: VsdV5ShortPointer) {
    dst[off..off + 2].copy_from_slice(&ptr.type_id.to_le_bytes());
    dst[off + 2..off + 6].copy_from_slice(&ptr.offset.to_le_bytes());
    dst[off + 6..off + 8].copy_from_slice(&ptr.length.to_le_bytes());
    dst[off + 8..off + 10].copy_from_slice(&ptr.flags.to_le_bytes());
}

fn append_v5_stream(buf: &mut Vec<u8>, type_id: u16, raw: &[u8]) -> VsdV5ShortPointer {
    let comp = compress_vsd_stream(raw);
    let offset = buf.len() as u32;
    if comp.len() <= u16::MAX as usize {
        buf.extend_from_slice(&comp);
        VsdV5ShortPointer {
            type_id,
            offset,
            length: comp.len() as u16,
            flags: 0x0002,
        }
    } else {
        let clamped = raw.len().min(u16::MAX as usize);
        buf.extend_from_slice(&raw[..clamped]);
        VsdV5ShortPointer {
            type_id,
            offset,
            length: clamped as u16,
            flags: 0x0000,
        }
    }
}

fn extract_v5_stream(buf: &[u8], ptr: VsdV5ShortPointer) -> Result<Vec<u8>, LegacyVisioError> {
    let start = ptr.offset as usize;
    let end = start.saturating_add(ptr.length as usize);
    if end > buf.len() {
        return Err(LegacyVisioError::InvalidHeader(format!(
            "Visio v5 pointer (offset={start}, len={}) exceeds stream bounds ({})",
            ptr.length,
            buf.len()
        )));
    }
    let slice = &buf[start..end];
    if ptr.is_compressed() {
        Ok(decompress_vsd_stream(slice))
    } else {
        Ok(slice.to_vec())
    }
}

fn encode_v5_pointer_list(ptrs: &[VsdV5ShortPointer]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + ptrs.len() * 10);
    out.extend_from_slice(&(ptrs.len() as u16).to_le_bytes());
    for p in ptrs {
        let mut slot = [0u8; 10];
        write_v5_short_pointer(&mut slot, 0, *p);
        out.extend_from_slice(&slot);
    }
    out
}

fn read_v5_pointer_list(data: &[u8]) -> Vec<VsdV5ShortPointer> {
    if data.len() < 2 {
        return Vec::new();
    }
    // Support both 2-byte u16 count (standard v5) and 4-byte u32 count
    let count_u16 = u16::from_le_bytes([data[0], data[1]]) as usize;
    let (count, mut off) = if 2 + count_u16 * 10 == data.len() {
        (count_u16, 2usize)
    } else if data.len() >= 4 {
        let count_u32 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if 4 + count_u32 * 10 <= data.len() {
            (count_u32, 4usize)
        } else {
            (count_u16, 2usize)
        }
    } else {
        (count_u16, 2usize)
    };

    let mut out = Vec::with_capacity(count.min(4096));
    for _ in 0..count {
        if let Ok(p) = read_v5_short_pointer(data, off) {
            out.push(p);
            off += 10;
        } else {
            break;
        }
    }
    out
}

fn write_v5_rec(out: &mut Vec<u8>, rec_type: u16, payload: &[u8]) {
    out.extend_from_slice(&rec_type.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
}

fn iter_v5_recs(mut data: &[u8]) -> Vec<(u16, &[u8])> {
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

fn encode_v5_shape(sh: &VisioShape) -> (u16, Vec<u8>) {
    let tag = match sh.kind {
        ShapeKind::Connector1D => VSD_SHAPE_1D,
        ShapeKind::Group => VSD_SHAPE_GROUP,
        _ => VSD_SHAPE_2D,
    };
    let mut out = Vec::new();
    let mut id_hdr = [0u8; 12];
    id_hdr[0..8].copy_from_slice(&sh.id.to_le_bytes());
    id_hdr[8..12].copy_from_slice(&sh.master_id.unwrap_or(0).to_le_bytes());
    write_v5_rec(&mut out, tag, &id_hdr);
    write_v5_rec(&mut out, VSD_NAME_STR, sh.name.as_bytes());

    let ss = &sh.sheet;
    let w = ss.get_f64("Width").unwrap_or(1.0);
    let h = ss.get_f64("Height").unwrap_or(1.0);
    let mut xf = Vec::with_capacity(56);
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
    write_v5_rec(&mut out, VSD_XFORM_DATA, &xf);

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
        write_v5_rec(&mut out, VSD_XFORM_1D, &x1d);
    }

    if let Some(fc) = ss.get_color("FillForegnd") {
        write_v5_rec(&mut out, VSD_FILL_FMT, &[fc.r, fc.g, fc.b, fc.a]);
    }
    if let Some(lc) = ss.get_color("LineColor") {
        let lw = ss.get_f64("LineWeight").unwrap_or(1.0 / 72.0);
        let mut lp = Vec::with_capacity(12);
        lp.extend_from_slice(&lw.to_le_bytes());
        lp.extend_from_slice(&[lc.r, lc.g, lc.b, lc.a]);
        write_v5_rec(&mut out, VSD_LINE_FMT, &lp);
    }

    if !sh.text.is_empty() {
        if sh.text.is_ascii() {
            write_v5_rec(&mut out, VSD_TEXT_ANSI, sh.text.as_bytes());
        } else {
            let utf16: Vec<u8> = sh
                .text
                .encode_utf16()
                .flat_map(|u| u.to_le_bytes())
                .collect();
            write_v5_rec(&mut out, VSD_TEXT_UTF16, &utf16);
        }
    }

    // Preserve explicit User.* cells via RPN bytecode records
    for urow in ss.user_cells.values() {
        if let Ok(bc) = compile_formula_to_vsd_bytecode(&urow.value) {
            let cell_name = format!("User.{}", urow.name);
            let nb = cell_name.as_bytes();
            let mut fp = Vec::with_capacity(2 + nb.len() + bc.len());
            fp.extend_from_slice(&(nb.len() as u16).to_le_bytes());
            fp.extend_from_slice(nb);
            fp.extend_from_slice(&bc);
            write_v5_rec(&mut out, VSD_FORMULA_CELL, &fp);
        }
    }

    for g in &ss.geometries {
        let flags = (if g.no_fill == "TRUE" || g.no_fill == "1" {
            1u8
        } else {
            0
        }) | (if g.no_line == "TRUE" || g.no_line == "1" {
            2u8
        } else {
            0
        });
        write_v5_rec(&mut out, VSD_GEOM_HDR, &[g.index as u8, flags]);
        let prefix = format!("Geometry{}", g.index);
        for row in &g.rows {
            if row.deleted {
                continue;
            }
            let x = ss.get_f64(&format!("{prefix}.X{}", row.ix)).unwrap_or(0.0);
            let y = ss.get_f64(&format!("{prefix}.Y{}", row.ix)).unwrap_or(0.0);
            match row.kind {
                GeometryRowKind::MoveTo => {
                    let mut p = [0u8; 16];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    write_v5_rec(&mut out, VSD_MOVE_TO, &p);
                }
                GeometryRowKind::ArcTo => {
                    let bow = ss.get_f64(&format!("{prefix}.A{}", row.ix)).unwrap_or(0.0);
                    let mut p = [0u8; 24];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    p[16..24].copy_from_slice(&bow.to_le_bytes());
                    write_v5_rec(&mut out, VSD_ARC_TO, &p);
                }
                GeometryRowKind::Ellipse => {
                    let mut p = [0u8; 16];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    write_v5_rec(&mut out, VSD_ELLIPSE, &p);
                }
                _ => {
                    let mut p = [0u8; 16];
                    p[0..8].copy_from_slice(&x.to_le_bytes());
                    p[8..16].copy_from_slice(&y.to_le_bytes());
                    write_v5_rec(&mut out, VSD_LINE_TO, &p);
                }
            }
        }
    }

    for child in &sh.children {
        let (ctag, cbytes) = encode_v5_shape(child);
        write_v5_rec(&mut out, ctag, &cbytes);
    }
    (tag, out)
}

fn parse_v5_shape(outer_tag: u16, data: &[u8]) -> VisioShape {
    let kind = match outer_tag {
        VSD_SHAPE_1D => ShapeKind::Connector1D,
        VSD_SHAPE_GROUP => ShapeKind::Group,
        _ => ShapeKind::Shape2D,
    };
    let mut id = 1u64;
    let mut master_id = None;
    let mut name = String::from("Sheet.1");
    let mut sheet = if kind == ShapeKind::Connector1D {
        ShapeSheet::new_1d_connector(0.0, 0.0, 1.0, 1.0)
    } else {
        ShapeSheet::new_2d(1.0, 1.0, 2.0, 1.0)
    };
    sheet.geometries.clear();
    let mut cur_geom: Option<GeometrySection> = None;
    let mut text = String::new();
    let mut children = Vec::new();
    let mut saw_own_hdr = false;

    for (rt, payload) in iter_v5_recs(data) {
        match rt {
            t if (t == VSD_SHAPE_2D || t == VSD_SHAPE_1D || t == VSD_SHAPE_GROUP)
                && !saw_own_hdr =>
            {
                saw_own_hdr = true;
                if payload.len() >= 12 {
                    id = u64::from_le_bytes(payload[0..8].try_into().unwrap());
                    let mid = u32::from_le_bytes(payload[8..12].try_into().unwrap());
                    if mid != 0 {
                        master_id = Some(mid);
                    }
                }
            }
            VSD_SHAPE_2D | VSD_SHAPE_1D | VSD_SHAPE_GROUP => {
                children.push(parse_v5_shape(rt, payload));
            }
            VSD_NAME_STR => {
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
            VSD_TEXT_ANSI => {
                text = payload.iter().map(|&b| b as char).collect();
            }
            VSD_TEXT_UTF16 => {
                let units: Vec<u16> = payload
                    .chunks_exact(2)
                    .map(|c| u16::from_le_bytes([c[0], c[1]]))
                    .collect();
                text = String::from_utf16_lossy(&units);
            }
            VSD_FORMULA_CELL if payload.len() >= 3 => {
                let nlen = u16::from_le_bytes([payload[0], payload[1]]) as usize;
                if 2 + nlen <= payload.len() {
                    let cname = String::from_utf8_lossy(&payload[2..2 + nlen]).into_owned();
                    let bc = &payload[2 + nlen..];
                    if let Ok(formula) = decompile_vsd_bytecode_to_formula(bc) {
                        if let Some(uname) = cname.strip_prefix("User.") {
                            sheet.set_user_cell(uname, formula, "");
                        } else {
                            sheet.set_formula(cname, formula, None);
                        }
                    }
                }
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
            VSD_ELLIPSE if payload.len() >= 16 => {
                let x = f64::from_le_bytes(payload[0..8].try_into().unwrap());
                let y = f64::from_le_bytes(payload[8..16].try_into().unwrap());
                let g = cur_geom.get_or_insert_with(|| GeometrySection::new(1));
                let ix = (g.rows.len() as u32) + 1;
                g.rows.push(GeometryRow {
                    ix,
                    kind: GeometryRowKind::Ellipse,
                    x: format!("{x} in"),
                    y: format!("{y} in"),
                    a: None,
                    b: None,
                    c: None,
                    d: None,
                    e: None,
                    deleted: false,
                });
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

/// Encodes `doc` into a Visio 5.0 / 5.0c (`version == 5` at `0x1A`) binary `VisioDocument`
/// stream using 10-byte short pointer records (`[type_id: u16, offset: u32, length: u16, flags: u16]`).
pub fn encode_visio_v5_stream(doc: &VisioDocument) -> Vec<u8> {
    let mut buf = vec![0u8; 0x36];
    buf[0..21].copy_from_slice(VSD_HEADER_MAGIC);
    buf[0x1A] = 5; // Visio 5.0 / 5.0c format version

    let mut top_ptrs = Vec::new();

    // 1. DocSheet
    let mut doc_rec = Vec::new();
    write_v5_rec(&mut doc_rec, VSD_NAME_STR, doc.properties.title.as_bytes());
    top_ptrs.push(append_v5_stream(&mut buf, VSD_DOC_SHEET, &doc_rec));

    // 2. Masters
    for m in &doc.masters {
        let mut m_rec = Vec::new();
        write_v5_rec(&mut m_rec, VSD_MASTER, &m.id.to_le_bytes());
        write_v5_rec(&mut m_rec, VSD_NAME_STR, m.name.as_bytes());
        for s in &m.shapes {
            let (tag, sbytes) = encode_v5_shape(s);
            write_v5_rec(&mut m_rec, tag, &sbytes);
        }
        top_ptrs.push(append_v5_stream(&mut buf, VSD_MASTER, &m_rec));
    }

    // 3. Pages
    for page in &doc.pages {
        let mut p_rec = Vec::new();
        let mut hdr = Vec::with_capacity(25);
        hdr.extend_from_slice(&page.id.to_le_bytes());
        hdr.extend_from_slice(&page.width_in().to_le_bytes());
        hdr.extend_from_slice(&page.height_in().to_le_bytes());
        hdr.push(u8::from(page.is_background));
        hdr.extend_from_slice(&page.back_page_id.unwrap_or(0).to_le_bytes());
        write_v5_rec(&mut p_rec, VSD_PAGE, &hdr);
        write_v5_rec(&mut p_rec, VSD_NAME_STR, page.name.as_bytes());

        for l in &page.layers {
            let mut lp = Vec::new();
            lp.extend_from_slice(&l.ix.to_le_bytes());
            lp.push(
                (if l.visible { 1 } else { 0 })
                    | (if l.print { 2 } else { 0 })
                    | (if l.lock { 4 } else { 0 }),
            );
            lp.extend_from_slice(l.name.as_bytes());
            write_v5_rec(&mut p_rec, VSD_LAYER_DEF, &lp);
        }

        for sh in &page.shapes {
            let (tag, sbytes) = encode_v5_shape(sh);
            write_v5_rec(&mut p_rec, tag, &sbytes);
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
            write_v5_rec(&mut p_rec, VSD_CONNECT, &cp);
        }
        top_ptrs.push(append_v5_stream(&mut buf, VSD_PAGE, &p_rec));
    }

    // 4. Trailer stream + 10-byte short pointer at header `0x24`
    let trailer_raw = encode_v5_pointer_list(&top_ptrs);
    let trailer_ptr = append_v5_stream(&mut buf, VSD_TRAILER, &trailer_raw);
    write_v5_short_pointer(&mut buf, 0x24, trailer_ptr);

    let total_len = buf.len() as u32;
    buf[0x1C..0x20].copy_from_slice(&total_len.to_le_bytes());
    let crc = crc32fast::hash(&buf[0x36..]);
    buf[0x30..0x34].copy_from_slice(&crc.to_le_bytes());
    buf
}

/// Parses a pre-2000 Visio `v5` (`version == 5` at `0x1A`) `VisioDocument` binary stream
/// (or an OLE2 CFB container housing a `v5` `VisioDocument` stream) into a `VisioDocument`.
pub fn parse_visio_v5_stream(buf: &[u8]) -> Result<VisioDocument, LegacyVisioError> {
    if buf.starts_with(b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1") {
        let mut comp = cfb::CompoundFile::open(Cursor::new(buf))?;
        let path = if comp.exists("/VisioDocument") {
            "/VisioDocument"
        } else if comp.exists("VisioDocument") {
            "VisioDocument"
        } else {
            return Err(LegacyVisioError::InvalidHeader(
                "OLE2 CFB missing VisioDocument stream".into(),
            ));
        };
        let mut inner = Vec::new();
        comp.open_stream(path)?.read_to_end(&mut inner)?;
        return parse_visio_v5_stream(&inner);
    }

    if buf.len() < 0x2E || !buf.starts_with(b"Visio (TM) Drawing") {
        return Err(LegacyVisioError::InvalidHeader(
            "Visio v5 stream signature mismatch".into(),
        ));
    }
    let version = buf[0x1A];
    if version != 5 {
        return Err(LegacyVisioError::InvalidHeader(format!(
            "Expected Visio v5 header (version=5 at 0x1A), found version={version}"
        )));
    }

    let trailer_ptr = read_v5_short_pointer(buf, 0x24)?;
    if trailer_ptr.type_id != VSD_TRAILER {
        return Err(LegacyVisioError::InvalidHeader(format!(
            "Expected v5 trailer type 0x0014 at 0x24, got 0x{:04X}",
            trailer_ptr.type_id
        )));
    }

    let trailer_data = extract_v5_stream(buf, trailer_ptr)?;
    let child_ptrs = read_v5_pointer_list(&trailer_data);

    let mut doc = VisioDocument::new_blank("Visio 5 Drawing");
    doc.pages.clear();
    doc.masters.clear();

    for ptr in child_ptrs {
        match ptr.type_id {
            VSD_DOC_SHEET => {
                let data = extract_v5_stream(buf, ptr)?;
                for (rt, payload) in iter_v5_recs(&data) {
                    if rt == VSD_NAME_STR {
                        doc.properties.title = String::from_utf8_lossy(payload).into_owned();
                    }
                }
            }
            VSD_MASTER => {
                let data = extract_v5_stream(buf, ptr)?;
                let mut mid = (doc.masters.len() as u32) + 1;
                let mut mname = format!("Master.{mid}");
                let mut shapes = Vec::new();
                for (rt, payload) in iter_v5_recs(&data) {
                    match rt {
                        VSD_MASTER if payload.len() >= 4 => {
                            mid = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                        }
                        VSD_NAME_STR => {
                            mname = String::from_utf8_lossy(payload).into_owned();
                        }
                        VSD_SHAPE_2D | VSD_SHAPE_1D | VSD_SHAPE_GROUP => {
                            shapes.push(parse_v5_shape(rt, payload));
                        }
                        _ => {}
                    }
                }
                doc.masters.push(VisioMaster {
                    id: mid,
                    name: mname.clone(),
                    name_u: mname,
                    base_id: String::new(),
                    unique_id: String::new(),
                    prompt: String::new(),
                    icon_bytes: None,
                    page_sheet: ShapeSheet::new(),
                    shapes,
                });
            }
            VSD_PAGE => {
                let data = extract_v5_stream(buf, ptr)?;
                let fallback_id = (doc.pages.len() as u32) + 1;
                let mut pid = fallback_id;
                let mut pw = 11.0;
                let mut ph = 8.5;
                let mut is_bg = false;
                let mut back_id = None;
                let mut pname = format!("Page-{fallback_id}");
                let mut layers = Vec::new();
                let mut shapes = Vec::new();
                let mut connects = Vec::new();

                for (rt, payload) in iter_v5_recs(&data) {
                    match rt {
                        VSD_PAGE if payload.len() >= 25 => {
                            pid = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                            pw = f64::from_le_bytes(payload[4..12].try_into().unwrap());
                            ph = f64::from_le_bytes(payload[12..20].try_into().unwrap());
                            is_bg = payload[20] != 0;
                            let bid = u32::from_le_bytes(payload[21..25].try_into().unwrap());
                            if bid != 0 {
                                back_id = Some(bid);
                            }
                        }
                        VSD_NAME_STR => {
                            pname = String::from_utf8_lossy(payload).into_owned();
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
                            shapes.push(parse_v5_shape(rt, payload));
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

                let mut page = VisioPage::new(pid, pname, pw, ph);
                page.is_background = is_bg;
                page.back_page_id = back_id;
                if !layers.is_empty() {
                    page.layers = layers;
                }
                page.shapes = shapes;
                page.connects = connects;
                doc.pages.push(page);
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

// ============================================================================
// 2. MS-VSD RPN Binary Formula Bytecode Compiler & Decompiler
// ============================================================================

pub const VSD_OP_ADD: u8 = 0x03;
pub const VSD_OP_SUB: u8 = 0x04;
pub const VSD_OP_MUL: u8 = 0x05;
pub const VSD_OP_DIV: u8 = 0x06;
pub const VSD_OP_POW: u8 = 0x07;
pub const VSD_OP_NEG: u8 = 0x08;
pub const VSD_OP_EQ: u8 = 0x09;
pub const VSD_OP_NE: u8 = 0x0A;
pub const VSD_OP_LT: u8 = 0x0B;
pub const VSD_OP_LE: u8 = 0x0C;
pub const VSD_OP_GT: u8 = 0x0D;
pub const VSD_OP_GE: u8 = 0x0E;
pub const VSD_OP_CONCAT: u8 = 0x0F;
pub const VSD_OP_AND: u8 = 0x10;
pub const VSD_OP_OR: u8 = 0x11;
pub const VSD_OP_NOT: u8 = 0x12;
pub const VSD_OP_POS: u8 = 0x13;
pub const VSD_OP_PUSH_NUM_UNIT: u8 = 0x20;
pub const VSD_OP_PUSH_BOOL: u8 = 0x21;
pub const VSD_OP_PUSH_STR: u8 = 0x22;
pub const VSD_OP_PUSH_CELL_REF: u8 = 0x30;
pub const VSD_OP_FUNC_CALL: u8 = 0x40;

fn unit_to_vsd_code(unit: Unit) -> u8 {
    match unit {
        Unit::Number => 0,
        Unit::Inch => 1,
        Unit::Millimeter => 2,
        Unit::Centimeter => 3,
        Unit::Meter => 4,
        Unit::Kilometer => 5,
        Unit::Foot => 6,
        Unit::Yard => 7,
        Unit::Mile => 8,
        Unit::Point => 9,
        Unit::Pica => 10,
        Unit::Didot => 11,
        Unit::Cicero => 12,
        Unit::Degree => 13,
        Unit::Radian => 14,
        Unit::ArcMinute => 15,
        Unit::ArcSecond => 16,
        Unit::Percent => 17,
        Unit::Currency => 18,
        Unit::ElapsedSecond => 19,
        Unit::ElapsedMinute => 20,
        Unit::ElapsedHour => 21,
        Unit::ElapsedDay => 22,
        Unit::ElapsedWeek => 23,
        Unit::Boolean => 24,
        Unit::String => 25,
        Unit::Color => 26,
        Unit::Point2D => 27,
        Unit::Guid => 28,
    }
}

fn vsd_code_to_unit(code: u8) -> Result<Unit, String> {
    match code {
        0 => Ok(Unit::Number),
        1 => Ok(Unit::Inch),
        2 => Ok(Unit::Millimeter),
        3 => Ok(Unit::Centimeter),
        4 => Ok(Unit::Meter),
        5 => Ok(Unit::Kilometer),
        6 => Ok(Unit::Foot),
        7 => Ok(Unit::Yard),
        8 => Ok(Unit::Mile),
        9 => Ok(Unit::Point),
        10 => Ok(Unit::Pica),
        11 => Ok(Unit::Didot),
        12 => Ok(Unit::Cicero),
        13 => Ok(Unit::Degree),
        14 => Ok(Unit::Radian),
        15 => Ok(Unit::ArcMinute),
        16 => Ok(Unit::ArcSecond),
        17 => Ok(Unit::Percent),
        18 => Ok(Unit::Currency),
        19 => Ok(Unit::ElapsedSecond),
        20 => Ok(Unit::ElapsedMinute),
        21 => Ok(Unit::ElapsedHour),
        22 => Ok(Unit::ElapsedDay),
        23 => Ok(Unit::ElapsedWeek),
        24 => Ok(Unit::Boolean),
        25 => Ok(Unit::String),
        26 => Ok(Unit::Color),
        27 => Ok(Unit::Point2D),
        28 => Ok(Unit::Guid),
        other => Err(format!("Unknown MS-VSD unit code {other}")),
    }
}

fn format_cell_ref_path(path: &CellRefPath) -> String {
    fn fmt_target(target: &SheetTarget, cell: &str) -> String {
        match target {
            SheetTarget::SelfSheet => cell.to_string(),
            SheetTarget::ThePage => format!("ThePage!{cell}"),
            SheetTarget::TheDoc => format!("TheDoc!{cell}"),
            SheetTarget::ShapeId(id) => format!("Sheet.{id}!{cell}"),
            SheetTarget::NamedShape(name) => {
                let needs_brackets = name
                    .chars()
                    .any(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != '$');
                if needs_brackets {
                    format!("[{name}]!{cell}")
                } else {
                    format!("{name}!{cell}")
                }
            }
            SheetTarget::PageTarget { page_name, inner } => {
                format!("Pages[{page_name}]!{}", fmt_target(inner, cell))
            }
        }
    }
    fmt_target(&path.target, &path.cell)
}

fn emit_rpn_expr(expr: &Expr, out: &mut Vec<u8>) -> Result<(), String> {
    match expr {
        Expr::Number { value, unit } => {
            out.push(VSD_OP_PUSH_NUM_UNIT);
            out.extend_from_slice(&value.to_le_bytes());
            out.push(unit_to_vsd_code(*unit));
        }
        Expr::Bool(b) => {
            out.push(VSD_OP_PUSH_BOOL);
            out.push(u8::from(*b));
        }
        Expr::Str(s) => {
            let bytes = s.as_bytes();
            if bytes.len() > u16::MAX as usize {
                return Err("Formula string literal exceeds 65535 bytes".into());
            }
            out.push(VSD_OP_PUSH_STR);
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Expr::CellRef(path) => {
            let formatted = format_cell_ref_path(path);
            let bytes = formatted.as_bytes();
            if bytes.len() > u16::MAX as usize {
                return Err("Formula cell reference exceeds 65535 bytes".into());
            }
            out.push(VSD_OP_PUSH_CELL_REF);
            out.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Expr::Unary { op, expr: inner } => {
            emit_rpn_expr(inner, out)?;
            let code = match op {
                UnaryOp::Neg => VSD_OP_NEG,
                UnaryOp::Not => VSD_OP_NOT,
                UnaryOp::Pos => VSD_OP_POS,
            };
            out.push(code);
        }
        Expr::Binary { op, lhs, rhs } => {
            emit_rpn_expr(lhs, out)?;
            emit_rpn_expr(rhs, out)?;
            let code = match op {
                BinaryOp::Add => VSD_OP_ADD,
                BinaryOp::Sub => VSD_OP_SUB,
                BinaryOp::Mul => VSD_OP_MUL,
                BinaryOp::Div => VSD_OP_DIV,
                BinaryOp::Pow => VSD_OP_POW,
                BinaryOp::Eq => VSD_OP_EQ,
                BinaryOp::NotEq => VSD_OP_NE,
                BinaryOp::Lt => VSD_OP_LT,
                BinaryOp::Le => VSD_OP_LE,
                BinaryOp::Gt => VSD_OP_GT,
                BinaryOp::Ge => VSD_OP_GE,
                BinaryOp::Concat => VSD_OP_CONCAT,
                BinaryOp::And => VSD_OP_AND,
                BinaryOp::Or => VSD_OP_OR,
            };
            out.push(code);
        }
        Expr::Call { name, args } => {
            if args.len() > u8::MAX as usize {
                return Err(format!(
                    "Function '{}' has {} args (max 255)",
                    name,
                    args.len()
                ));
            }
            for arg in args {
                emit_rpn_expr(arg, out)?;
            }
            let upper = name.to_ascii_uppercase();
            let nb = upper.as_bytes();
            if nb.len() > u8::MAX as usize {
                return Err(format!("Function name '{}' exceeds 255 bytes", upper));
            }
            out.push(VSD_OP_FUNC_CALL);
            out.push(args.len() as u8);
            out.push(nb.len() as u8);
            out.extend_from_slice(nb);
        }
    }
    Ok(())
}

/// Compiles a Visio ShapeSheet formula string into deterministic postfix MS-VSD RPN binary bytecode.
pub fn compile_formula_to_vsd_bytecode(formula: &str) -> Result<Vec<u8>, String> {
    let ast = parse_formula(formula).map_err(|e| format!("Formula parse error: {e}"))?;
    let mut bytecode = Vec::new();
    emit_rpn_expr(&ast, &mut bytecode)?;
    Ok(bytecode)
}

#[derive(Debug, Clone)]
struct DecompItem {
    text: String,
    prec: u8,
}

fn wrap_if_lower(item: &DecompItem, min_prec: u8) -> String {
    if item.prec < min_prec {
        format!("({})", item.text)
    } else {
        item.text.clone()
    }
}

/// Decompiles MS-VSD postfix RPN binary bytecode back into a canonical, precedence-parenthesized
/// infix Visio formula string (`=GUARD(Width * 0.5 + 2 in)`).
pub fn decompile_vsd_bytecode_to_formula(bytecode: &[u8]) -> Result<String, String> {
    if bytecode.is_empty() {
        return Err("Empty MS-VSD formula bytecode".into());
    }
    let mut stack: Vec<DecompItem> = Vec::new();
    let mut pos = 0usize;

    while pos < bytecode.len() {
        let op = bytecode[pos];
        pos += 1;

        match op {
            0x00 => break,
            VSD_OP_PUSH_NUM_UNIT => {
                if pos + 9 > bytecode.len() {
                    return Err("Truncated PushNumWithUnit (0x20) in VSD bytecode".into());
                }
                let val = f64::from_le_bytes(bytecode[pos..pos + 8].try_into().unwrap());
                let unit = vsd_code_to_unit(bytecode[pos + 8])?;
                pos += 9;
                let prec = if *val.to_le_bytes().last().unwrap() & 0x80 != 0 && val < 0.0 {
                    13
                } else {
                    20
                };
                stack.push(DecompItem {
                    text: format!("{}{}", format_clean_f64(val), unit.suffix()),
                    prec,
                });
            }
            VSD_OP_PUSH_BOOL => {
                if pos >= bytecode.len() {
                    return Err("Truncated PushBool (0x21) in VSD bytecode".into());
                }
                let b = bytecode[pos] != 0;
                pos += 1;
                stack.push(DecompItem {
                    text: if b { "TRUE".into() } else { "FALSE".into() },
                    prec: 20,
                });
            }
            VSD_OP_PUSH_STR => {
                if pos + 2 > bytecode.len() {
                    return Err("Truncated PushStr (0x22) length in VSD bytecode".into());
                }
                let slen = u16::from_le_bytes([bytecode[pos], bytecode[pos + 1]]) as usize;
                pos += 2;
                if pos + slen > bytecode.len() {
                    return Err("Truncated PushStr (0x22) payload in VSD bytecode".into());
                }
                let s = std::str::from_utf8(&bytecode[pos..pos + slen])
                    .map_err(|e| format!("Invalid UTF-8 in VSD string literal: {e}"))?;
                pos += slen;
                let escaped = s.replace('"', "\"\"");
                stack.push(DecompItem {
                    text: format!("\"{escaped}\""),
                    prec: 20,
                });
            }
            VSD_OP_PUSH_CELL_REF => {
                if pos + 2 > bytecode.len() {
                    return Err("Truncated PushCellRef (0x30) length in VSD bytecode".into());
                }
                let rlen = u16::from_le_bytes([bytecode[pos], bytecode[pos + 1]]) as usize;
                pos += 2;
                if pos + rlen > bytecode.len() {
                    return Err("Truncated PushCellRef (0x30) payload in VSD bytecode".into());
                }
                let r = std::str::from_utf8(&bytecode[pos..pos + rlen])
                    .map_err(|e| format!("Invalid UTF-8 in VSD cell ref: {e}"))?;
                pos += rlen;
                stack.push(DecompItem {
                    text: r.to_string(),
                    prec: 20,
                });
            }
            VSD_OP_NEG | VSD_OP_NOT | VSD_OP_POS => {
                let inner = stack
                    .pop()
                    .ok_or_else(|| format!("Stack underflow on unary opcode 0x{op:02X}"))?;
                let wrapped = wrap_if_lower(&inner, 13);
                let text = match op {
                    VSD_OP_NEG => format!("-{wrapped}"),
                    VSD_OP_NOT => format!("NOT {wrapped}"),
                    _ => format!("+{wrapped}"),
                };
                stack.push(DecompItem { text, prec: 13 });
            }
            VSD_OP_ADD | VSD_OP_SUB | VSD_OP_MUL | VSD_OP_DIV | VSD_OP_POW | VSD_OP_EQ
            | VSD_OP_NE | VSD_OP_LT | VSD_OP_LE | VSD_OP_GT | VSD_OP_GE | VSD_OP_CONCAT
            | VSD_OP_AND | VSD_OP_OR => {
                let rhs = stack
                    .pop()
                    .ok_or_else(|| format!("Stack underflow (rhs) on binary opcode 0x{op:02X}"))?;
                let lhs = stack
                    .pop()
                    .ok_or_else(|| format!("Stack underflow (lhs) on binary opcode 0x{op:02X}"))?;
                let (sym, l_prec, r_min_prec, out_prec) = match op {
                    VSD_OP_OR => ("OR", 1, 2, 1),
                    VSD_OP_AND => ("AND", 3, 4, 3),
                    VSD_OP_EQ => ("=", 5, 6, 5),
                    VSD_OP_NE => ("<>", 5, 6, 5),
                    VSD_OP_LT => ("<", 5, 6, 5),
                    VSD_OP_LE => ("<=", 5, 6, 5),
                    VSD_OP_GT => (">", 5, 6, 5),
                    VSD_OP_GE => (">=", 5, 6, 5),
                    VSD_OP_CONCAT => ("&", 7, 8, 7),
                    VSD_OP_ADD => ("+", 9, 10, 9),
                    VSD_OP_SUB => ("-", 9, 10, 9),
                    VSD_OP_MUL => ("*", 11, 12, 11),
                    VSD_OP_DIV => ("/", 11, 12, 11),
                    VSD_OP_POW => ("^", 15, 14, 14),
                    _ => unreachable!(),
                };
                let ls = wrap_if_lower(&lhs, l_prec);
                let rs = wrap_if_lower(&rhs, r_min_prec);
                stack.push(DecompItem {
                    text: format!("{ls} {sym} {rs}"),
                    prec: out_prec,
                });
            }
            VSD_OP_FUNC_CALL => {
                if pos + 2 > bytecode.len() {
                    return Err("Truncated FuncCall (0x40) header in VSD bytecode".into());
                }
                let argc = bytecode[pos] as usize;
                let nlen = bytecode[pos + 1] as usize;
                pos += 2;
                if pos + nlen > bytecode.len() {
                    return Err("Truncated FuncCall (0x40) function name in VSD bytecode".into());
                }
                let fname = std::str::from_utf8(&bytecode[pos..pos + nlen])
                    .map_err(|e| format!("Invalid UTF-8 in VSD function name: {e}"))?;
                pos += nlen;
                if stack.len() < argc {
                    return Err(format!(
                        "Stack underflow calling {fname}/{argc} (stack has {})",
                        stack.len()
                    ));
                }
                let split_at = stack.len() - argc;
                let args: Vec<String> = stack.drain(split_at..).map(|it| it.text).collect();
                stack.push(DecompItem {
                    text: format!("{}({})", fname.to_ascii_uppercase(), args.join(", ")),
                    prec: 20,
                });
            }
            other => {
                return Err(format!("Unsupported MS-VSD RPN opcode 0x{other:02X}"));
            }
        }
    }

    if stack.len() != 1 {
        return Err(format!(
            "Invalid RPN formula stack state at end of bytecode: {} items remaining",
            stack.len()
        ));
    }
    Ok(format!("={}", stack.pop().unwrap().text))
}

// ============================================================================
// 3. Standalone Offline `.emf` & `.wmf` Metafile Vector Parser & Encoder
// ============================================================================

const EMR_HEADER: u32 = 1;
const EMR_POLYGON: u32 = 3;
const EMR_POLYLINE: u32 = 4;
const EMR_EOF: u32 = 14;
const EMR_MOVETOEX: u32 = 27;
const EMR_SELECTOBJECT: u32 = 37;
const EMR_CREATEPEN: u32 = 38;
const EMR_CREATEBRUSHINDIRECT: u32 = 39;
const EMR_ELLIPSE: u32 = 42;
const EMR_RECTANGLE: u32 = 43;
const EMR_LINETO: u32 = 54;
const EMR_POLYGON16: u32 = 85;
const EMR_POLYLINE16: u32 = 86;
const EMR_POLYBEZIER16: u32 = 88;
const EMR_EXTCREATEPEN: u32 = 95;

const EMF_SIGNATURE: u32 = 0x464D_4520; // `" EMF"` in little-endian
const ALDUS_WMF_MAGIC: u32 = 0x9AC6_CDD7;

const META_EOF: u16 = 0x0000;
const META_SELECTOBJECT: u16 = 0x012D;
const META_DELETEOBJECT: u16 = 0x01F0;
const META_SETWINDOWORG: u16 = 0x020B;
const META_SETWINDOWEXT: u16 = 0x020C;
const META_LINETO: u16 = 0x0213;
const META_MOVETO: u16 = 0x0214;
const META_CREATEPENINDIRECT: u16 = 0x02FA;
const META_CREATEBRUSHINDIRECT: u16 = 0x02FC;
const META_POLYGON: u16 = 0x0324;
const META_POLYLINE: u16 = 0x0325;
const META_ELLIPSE: u16 = 0x0418;
const META_RECTANGLE: u16 = 0x041B;

#[derive(Debug, Clone, Copy)]
enum GdiObject {
    Pen {
        is_null: bool,
        width_logical: i32,
        color: RgbaColor,
    },
    Brush {
        is_null: bool,
        color: RgbaColor,
    },
}

#[derive(Debug, Clone)]
enum MetaPrimitive {
    Polyline {
        pts: Vec<[f64; 2]>,
        closed: bool,
        filled: bool,
        lined: bool,
    },
    CubicBezier {
        start: [f64; 2],
        segments: Vec<([f64; 2], [f64; 2], [f64; 2])>,
        lined: bool,
    },
    Ellipse {
        left: f64,
        top: f64,
        right: f64,
        bottom: f64,
        filled: bool,
        lined: bool,
    },
}

fn read_i32_le(buf: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_u32_le(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

fn read_i16_le(buf: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([buf[off], buf[off + 1]])
}

fn read_u16_le(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn apply_stock_gdi_object(
    stock_id: u32,
    pen_null: &mut bool,
    pen_color: &mut RgbaColor,
    brush_null: &mut bool,
    brush_color: &mut RgbaColor,
) {
    match stock_id & 0x7FFF_FFFF {
        0 => {
            *brush_null = false;
            *brush_color = RgbaColor::WHITE;
        }
        1 => {
            *brush_null = false;
            *brush_color = RgbaColor {
                r: 192,
                g: 192,
                b: 192,
                a: 255,
            };
        }
        2 | 3 => {
            *brush_null = false;
            *brush_color = RgbaColor {
                r: 128,
                g: 128,
                b: 128,
                a: 255,
            };
        }
        4 => {
            *brush_null = false;
            *brush_color = RgbaColor::BLACK;
        }
        5 => {
            *brush_null = true;
        }
        6 => {
            *pen_null = false;
            *pen_color = RgbaColor::WHITE;
        }
        7 => {
            *pen_null = false;
            *pen_color = RgbaColor::BLACK;
        }
        8 => {
            *pen_null = true;
        }
        _ => {}
    }
}

fn build_shape_from_metafile_primitives(
    id: u64,
    name: &str,
    pin_x_in: f64,
    pin_y_in: f64,
    bounds_opt: Option<[f64; 4]>,
    size_in_opt: Option<[f64; 2]>,
    primitives: &[MetaPrimitive],
    stroke_color: RgbaColor,
    stroke_weight_pt: f64,
    fill_color: Option<RgbaColor>,
    media_type: &str,
    raw_bytes: &[u8],
) -> VisioShape {
    // Compute logical coordinate bounding box `[min_lx, min_ly, max_lx, max_ly]`
    let (min_lx, min_ly, max_lx, max_ly) = if let Some([l, t, r, b]) = bounds_opt
        .filter(|b| (b[2] - b[0]).abs() > 1e-9 && (b[3] - b[1]).abs() > 1e-9)
    {
        (l.min(r), t.min(b), l.max(r), t.max(b))
    } else {
        let mut mnx = f64::INFINITY;
        let mut mny = f64::INFINITY;
        let mut mxx = f64::NEG_INFINITY;
        let mut mxy = f64::NEG_INFINITY;
        for prim in primitives {
            match prim {
                MetaPrimitive::Polyline { pts, .. } => {
                    for p in pts {
                        mnx = mnx.min(p[0]);
                        mny = mny.min(p[1]);
                        mxx = mxx.max(p[0]);
                        mxy = mxy.max(p[1]);
                    }
                }
                MetaPrimitive::CubicBezier {
                    start, segments, ..
                } => {
                    mnx = mnx.min(start[0]);
                    mny = mny.min(start[1]);
                    mxx = mxx.max(start[0]);
                    mxy = mxy.max(start[1]);
                    for (c1, c2, end) in segments {
                        for p in [c1, c2, end] {
                            mnx = mnx.min(p[0]);
                            mny = mny.min(p[1]);
                            mxx = mxx.max(p[0]);
                            mxy = mxy.max(p[1]);
                        }
                    }
                }
                MetaPrimitive::Ellipse {
                    left,
                    top,
                    right,
                    bottom,
                    ..
                } => {
                    mnx = mnx.min(left.min(*right));
                    mny = mny.min(top.min(*bottom));
                    mxx = mxx.max(left.max(*right));
                    mxy = mxy.max(top.max(*bottom));
                }
            }
        }
        if !mnx.is_finite() {
            (0.0, 0.0, 100.0, 100.0)
        } else {
            (mnx, mny, mxx, mxy)
        }
    };

    let span_lx = (max_lx - min_lx).max(1e-6);
    let span_ly = (max_ly - min_ly).max(1e-6);

    let [width_in, height_in] = size_in_opt.unwrap_or([
        (span_lx / 96.0).clamp(0.25, 24.0),
        (span_ly / 96.0).clamp(0.25, 24.0),
    ]);
    let width_in = width_in.max(0.05);
    let height_in = height_in.max(0.05);

    // Map GDI top-down `(lx, ly)` to Visio bottom-up local inches `(vx, vy)`
    let map_pt = |pt: [f64; 2]| -> [f64; 2] {
        let nx = ((pt[0] - min_lx) / span_lx).clamp(0.0, 1.0);
        let ny = (1.0 - ((pt[1] - min_ly) / span_ly)).clamp(0.0, 1.0);
        [nx * width_in, ny * height_in]
    };

    let mut sheet = ShapeSheet::new_2d(pin_x_in, pin_y_in, width_in, height_in);
    sheet.geometries.clear();

    let mut geom_idx = 1u32;
    for prim in primitives {
        match prim {
            MetaPrimitive::Polyline {
                pts,
                closed,
                filled,
                lined,
            } => {
                if pts.is_empty() {
                    continue;
                }
                let mut g = GeometrySection::new(geom_idx);
                g.no_fill = if *filled && *closed {
                    "FALSE".into()
                } else {
                    "TRUE".into()
                };
                g.no_line = if *lined { "FALSE".into() } else { "TRUE".into() };
                let p0 = map_pt(pts[0]);
                g.rows.push(GeometryRow::move_to(
                    1,
                    format!("{} in", format_clean_f64(p0[0])),
                    format!("{} in", format_clean_f64(p0[1])),
                ));
                let mut r_ix = 2u32;
                for &pt in &pts[1..] {
                    let mp = map_pt(pt);
                    g.rows.push(GeometryRow::line_to(
                        r_ix,
                        format!("{} in", format_clean_f64(mp[0])),
                        format!("{} in", format_clean_f64(mp[1])),
                    ));
                    r_ix += 1;
                }
                if *closed && pts.len() >= 2 {
                    let plast = map_pt(*pts.last().unwrap());
                    if (plast[0] - p0[0]).hypot(plast[1] - p0[1]) > 1e-6 {
                        g.rows.push(GeometryRow::line_to(
                            r_ix,
                            format!("{} in", format_clean_f64(p0[0])),
                            format!("{} in", format_clean_f64(p0[1])),
                        ));
                    }
                }
                sheet.add_geometry(g);
                geom_idx += 1;
            }
            MetaPrimitive::CubicBezier {
                start,
                segments,
                lined,
            } => {
                let mut g = GeometrySection::new(geom_idx);
                g.no_fill = "TRUE".into();
                g.no_line = if *lined { "FALSE".into() } else { "TRUE".into() };
                let s0 = map_pt(*start);
                g.rows.push(GeometryRow {
                    ix: 1,
                    kind: GeometryRowKind::RelMoveTo,
                    x: format_clean_f64(s0[0] / width_in),
                    y: format_clean_f64(s0[1] / height_in),
                    a: None,
                    b: None,
                    c: None,
                    d: None,
                    e: None,
                    deleted: false,
                });
                for (k, (c1, c2, end)) in segments.iter().enumerate() {
                    let mc1 = map_pt(*c1);
                    let mc2 = map_pt(*c2);
                    let me = map_pt(*end);
                    g.rows.push(GeometryRow {
                        ix: (k as u32) + 2,
                        kind: GeometryRowKind::RelCubBezTo,
                        x: format_clean_f64(me[0] / width_in),
                        y: format_clean_f64(me[1] / height_in),
                        a: Some(format_clean_f64(mc1[0] / width_in)),
                        b: Some(format_clean_f64(mc1[1] / height_in)),
                        c: Some(format_clean_f64(mc2[0] / width_in)),
                        d: Some(format_clean_f64(mc2[1] / height_in)),
                        e: None,
                        deleted: false,
                    });
                }
                sheet.add_geometry(g);
                geom_idx += 1;
            }
            MetaPrimitive::Ellipse {
                left,
                top,
                right,
                bottom,
                filled,
                lined,
            } => {
                let p_lt = map_pt([*left, *top]);
                let p_rb = map_pt([*right, *bottom]);
                let cx = (p_lt[0] + p_rb[0]) * 0.5;
                let cy = (p_lt[1] + p_rb[1]) * 0.5;
                let rx = (p_rb[0] - p_lt[0]).abs() * 0.5;
                let ry = (p_lt[1] - p_rb[1]).abs() * 0.5;
                let mut g = GeometrySection::new(geom_idx);
                g.no_fill = if *filled { "FALSE".into() } else { "TRUE".into() };
                g.no_line = if *lined { "FALSE".into() } else { "TRUE".into() };
                g.rows.push(GeometryRow {
                    ix: 1,
                    kind: GeometryRowKind::Ellipse,
                    x: format!("{} in", format_clean_f64(cx)),
                    y: format!("{} in", format_clean_f64(cy)),
                    a: Some(format!("{} in", format_clean_f64(cx + rx))),
                    b: Some(format!("{} in", format_clean_f64(cy))),
                    c: Some(format!("{} in", format_clean_f64(cx))),
                    d: Some(format!("{} in", format_clean_f64(cy + ry))),
                    e: None,
                    deleted: false,
                });
                sheet.add_geometry(g);
                geom_idx += 1;
            }
        }
    }

    if sheet.geometries.is_empty() {
        sheet.add_geometry(GeometrySection::rectangle(1));
    }

    sheet.set_formula(
        "LineColor",
        format!(
            "=RGB({},{},{})",
            stroke_color.r, stroke_color.g, stroke_color.b
        ),
        Some(Unit::Color),
    );
    sheet.set_formula(
        "LineWeight",
        format!("{} pt", format_clean_f64(stroke_weight_pt.clamp(0.25, 36.0))),
        Some(Unit::Point),
    );
    if let Some(fc) = fill_color {
        sheet.set_formula("FillPattern", "1", Some(Unit::Number));
        sheet.set_formula(
            "FillForegnd",
            format!("=RGB({},{},{})", fc.r, fc.g, fc.b),
            Some(Unit::Color),
        );
    } else {
        sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    }

    VisioShape {
        id,
        name: name.to_string(),
        name_u: name.to_string(),
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: String::new(),
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: Some(ForeignMediaData {
            media_type: media_type.to_string(),
            rel_id: None,
            data: raw_bytes.to_vec(),
        }),
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    }
}

/// Parses a binary Windows Enhanced Metafile (`.emf`) byte stream (`EMR_HEADER` signature
/// `0x464D4520` `" EMF"` at offset 40) into a normalized vector `VisioShape` at `(pin_x_in, pin_y_in)`.
pub fn parse_emf_to_shape(
    id: u64,
    name: &str,
    emf_bytes: &[u8],
    pin_x_in: f64,
    pin_y_in: f64,
) -> Result<VisioShape, String> {
    if emf_bytes.len() < 88 {
        return Err("EMF buffer shorter than 88-byte EMR_HEADER".into());
    }
    let hdr_type = read_u32_le(emf_bytes, 0);
    let hdr_size = read_u32_le(emf_bytes, 4) as usize;
    if hdr_type != EMR_HEADER || hdr_size < 88 || hdr_size > emf_bytes.len() || hdr_size % 4 != 0 {
        return Err(format!(
            "Invalid EMR_HEADER (type={hdr_type}, size={hdr_size})"
        ));
    }
    let sig = read_u32_le(emf_bytes, 40);
    if sig != EMF_SIGNATURE {
        return Err(format!(
            "Invalid EMF signature 0x{sig:08X} at offset 40 (expected 0x464D4520 ' EMF')"
        ));
    }

    let bounds = [
        read_i32_le(emf_bytes, 8) as f64,
        read_i32_le(emf_bytes, 12) as f64,
        read_i32_le(emf_bytes, 16) as f64,
        read_i32_le(emf_bytes, 20) as f64,
    ];
    let frame_l = read_i32_le(emf_bytes, 24) as f64;
    let frame_t = read_i32_le(emf_bytes, 28) as f64;
    let frame_r = read_i32_le(emf_bytes, 32) as f64;
    let frame_b = read_i32_le(emf_bytes, 36) as f64;
    let size_in_opt = if (frame_r - frame_l).abs() > 1.0 && (frame_b - frame_t).abs() > 1.0 {
        // `rclFrame` is in 0.01 mm units (`2540.0` units == `1.0 inch`)
        Some([
            (frame_r - frame_l).abs() / 2540.0,
            (frame_b - frame_t).abs() / 2540.0,
        ])
    } else {
        None
    };

    let mut gdi_table: BTreeMap<u32, GdiObject> = BTreeMap::new();
    let mut pen_null = false;
    let mut pen_color = RgbaColor::BLACK;
    let mut pen_width_logical = 1i32;
    let mut brush_null = false;
    let mut brush_color = RgbaColor::WHITE;
    let mut active_fill: Option<RgbaColor> = None;
    let mut active_stroke = RgbaColor::BLACK;

    let mut cur_pt = [0.0f64, 0.0f64];
    let mut open_line_strip: Vec<[f64; 2]> = Vec::new();
    let mut primitives: Vec<MetaPrimitive> = Vec::new();

    let flush_strip = |strip: &mut Vec<[f64; 2]>,
                       prims: &mut Vec<MetaPrimitive>,
                       lined: bool| {
        if strip.len() >= 2 {
            prims.push(MetaPrimitive::Polyline {
                pts: std::mem::take(strip),
                closed: false,
                filled: false,
                lined,
            });
        } else {
            strip.clear();
        }
    };

    let mut off = hdr_size;
    while off + 8 <= emf_bytes.len() {
        let rtype = read_u32_le(emf_bytes, off);
        let rsize = read_u32_le(emf_bytes, off + 4) as usize;
        if rsize < 8 || rsize % 4 != 0 || off + rsize > emf_bytes.len() {
            return Err(format!(
                "Malformed EMR record (type={rtype}, size={rsize}) at offset {off}"
            ));
        }
        let rec = &emf_bytes[off..off + rsize];

        if rtype != EMR_MOVETOEX && rtype != EMR_LINETO {
            flush_strip(&mut open_line_strip, &mut primitives, !pen_null);
        }

        match rtype {
            EMR_EOF => break,
            EMR_MOVETOEX if rsize >= 16 => {
                flush_strip(&mut open_line_strip, &mut primitives, !pen_null);
                cur_pt = [read_i32_le(rec, 8) as f64, read_i32_le(rec, 12) as f64];
                open_line_strip.push(cur_pt);
            }
            EMR_LINETO if rsize >= 16 => {
                let next_pt = [read_i32_le(rec, 8) as f64, read_i32_le(rec, 12) as f64];
                if open_line_strip.is_empty() {
                    open_line_strip.push(cur_pt);
                }
                open_line_strip.push(next_pt);
                cur_pt = next_pt;
                if !pen_null {
                    active_stroke = pen_color;
                }
            }
            EMR_RECTANGLE if rsize >= 24 => {
                let l = read_i32_le(rec, 8) as f64;
                let t = read_i32_le(rec, 12) as f64;
                let r = read_i32_le(rec, 16) as f64;
                let b = read_i32_le(rec, 20) as f64;
                if !brush_null {
                    active_fill = Some(brush_color);
                }
                if !pen_null {
                    active_stroke = pen_color;
                }
                primitives.push(MetaPrimitive::Polyline {
                    pts: vec![[l, t], [r, t], [r, b], [l, b]],
                    closed: true,
                    filled: !brush_null,
                    lined: !pen_null,
                });
            }
            EMR_ELLIPSE if rsize >= 24 => {
                let l = read_i32_le(rec, 8) as f64;
                let t = read_i32_le(rec, 12) as f64;
                let r = read_i32_le(rec, 16) as f64;
                let b = read_i32_le(rec, 20) as f64;
                if !brush_null {
                    active_fill = Some(brush_color);
                }
                if !pen_null {
                    active_stroke = pen_color;
                }
                primitives.push(MetaPrimitive::Ellipse {
                    left: l,
                    top: t,
                    right: r,
                    bottom: b,
                    filled: !brush_null,
                    lined: !pen_null,
                });
            }
            EMR_POLYLINE | EMR_POLYGON if rsize >= 28 => {
                let count = read_u32_le(rec, 24) as usize;
                let needed = 28usize.saturating_add(count.saturating_mul(8));
                if needed <= rsize && count >= 2 {
                    let mut pts = Vec::with_capacity(count);
                    for i in 0..count {
                        let p = 28 + i * 8;
                        pts.push([read_i32_le(rec, p) as f64, read_i32_le(rec, p + 4) as f64]);
                    }
                    let is_poly = rtype == EMR_POLYGON;
                    if is_poly && !brush_null {
                        active_fill = Some(brush_color);
                    }
                    if !pen_null {
                        active_stroke = pen_color;
                    }
                    primitives.push(MetaPrimitive::Polyline {
                        pts,
                        closed: is_poly,
                        filled: is_poly && !brush_null,
                        lined: !pen_null,
                    });
                }
            }
            EMR_POLYLINE16 | EMR_POLYGON16 if rsize >= 28 => {
                let count = read_u32_le(rec, 24) as usize;
                let needed = 28usize.saturating_add(count.saturating_mul(4));
                if needed <= rsize && count >= 2 {
                    let mut pts = Vec::with_capacity(count);
                    for i in 0..count {
                        let p = 28 + i * 4;
                        pts.push([read_i16_le(rec, p) as f64, read_i16_le(rec, p + 2) as f64]);
                    }
                    let is_poly = rtype == EMR_POLYGON16;
                    if is_poly && !brush_null {
                        active_fill = Some(brush_color);
                    }
                    if !pen_null {
                        active_stroke = pen_color;
                    }
                    primitives.push(MetaPrimitive::Polyline {
                        pts,
                        closed: is_poly,
                        filled: is_poly && !brush_null,
                        lined: !pen_null,
                    });
                }
            }
            EMR_POLYBEZIER16 if rsize >= 28 => {
                let count = read_u32_le(rec, 24) as usize;
                let needed = 28usize.saturating_add(count.saturating_mul(4));
                if needed <= rsize && count >= 4 && (count - 1) % 3 == 0 {
                    let start = [read_i16_le(rec, 28) as f64, read_i16_le(rec, 30) as f64];
                    let seg_count = (count - 1) / 3;
                    let mut segments = Vec::with_capacity(seg_count);
                    for s in 0..seg_count {
                        let base = 32 + s * 12;
                        let c1 = [
                            read_i16_le(rec, base) as f64,
                            read_i16_le(rec, base + 2) as f64,
                        ];
                        let c2 = [
                            read_i16_le(rec, base + 4) as f64,
                            read_i16_le(rec, base + 6) as f64,
                        ];
                        let end = [
                            read_i16_le(rec, base + 8) as f64,
                            read_i16_le(rec, base + 10) as f64,
                        ];
                        segments.push((c1, c2, end));
                    }
                    if !pen_null {
                        active_stroke = pen_color;
                    }
                    primitives.push(MetaPrimitive::CubicBezier {
                        start,
                        segments,
                        lined: !pen_null,
                    });
                }
            }
            EMR_CREATEPEN if rsize >= 28 => {
                let ih = read_u32_le(rec, 8);
                let style = read_u32_le(rec, 12);
                let width_x = read_i32_le(rec, 16);
                let color = RgbaColor {
                    r: rec[24],
                    g: rec[25],
                    b: rec[26],
                    a: 255,
                };
                gdi_table.insert(
                    ih,
                    GdiObject::Pen {
                        is_null: (style & 0x0F) == 5,
                        width_logical: width_x.max(1),
                        color,
                    },
                );
            }
            EMR_EXTCREATEPEN if rsize >= 52 => {
                let ih = read_u32_le(rec, 8);
                let style = read_u32_le(rec, 28);
                let width = read_u32_le(rec, 32) as i32;
                let color = RgbaColor {
                    r: rec[40],
                    g: rec[41],
                    b: rec[42],
                    a: 255,
                };
                gdi_table.insert(
                    ih,
                    GdiObject::Pen {
                        is_null: (style & 0x0F) == 5,
                        width_logical: width.max(1),
                        color,
                    },
                );
            }
            EMR_CREATEBRUSHINDIRECT if rsize >= 24 => {
                let ih = read_u32_le(rec, 8);
                let lb_style = read_u32_le(rec, 12);
                let color = RgbaColor {
                    r: rec[16],
                    g: rec[17],
                    b: rec[18],
                    a: 255,
                };
                gdi_table.insert(
                    ih,
                    GdiObject::Brush {
                        is_null: lb_style == 1,
                        color,
                    },
                );
            }
            EMR_SELECTOBJECT if rsize >= 12 => {
                let ih = read_u32_le(rec, 8);
                if (ih & 0x8000_0000) != 0 {
                    apply_stock_gdi_object(
                        ih,
                        &mut pen_null,
                        &mut pen_color,
                        &mut brush_null,
                        &mut brush_color,
                    );
                } else if let Some(obj) = gdi_table.get(&ih) {
                    match *obj {
                        GdiObject::Pen {
                            is_null,
                            width_logical,
                            color,
                        } => {
                            pen_null = is_null;
                            pen_width_logical = width_logical;
                            pen_color = color;
                            if !is_null {
                                active_stroke = color;
                            }
                        }
                        GdiObject::Brush { is_null, color } => {
                            brush_null = is_null;
                            brush_color = color;
                        }
                    }
                }
            }
            _ => {}
        }

        off += rsize;
    }
    flush_strip(&mut open_line_strip, &mut primitives, !pen_null);

    let logical_w = (bounds[2] - bounds[0]).abs().max(1.0);
    let phys_w_in = size_in_opt.map_or(logical_w / 96.0, |s| s[0].max(0.05));
    let stroke_pt = ((pen_width_logical as f64 / logical_w) * phys_w_in * 72.0).clamp(0.5, 24.0);

    Ok(build_shape_from_metafile_primitives(
        id,
        name,
        pin_x_in,
        pin_y_in,
        Some(bounds),
        size_in_opt,
        &primitives,
        active_stroke,
        stroke_pt,
        active_fill,
        "EMF",
        emf_bytes,
    ))
}

fn eval_local_coord(ss: &ShapeSheet, cell_name: &str, fallback_formula: &str, w: f64, h: f64) -> f64 {
    if let Some(v) = ss.get_f64(cell_name) {
        if v.abs() > 0.0 || !fallback_formula.contains("Width") && !fallback_formula.contains("Height") {
            return v;
        }
    }
    let f = fallback_formula.trim().trim_start_matches('=').trim();
    if f.eq_ignore_ascii_case("Width") || f.eq_ignore_ascii_case("Width*1") {
        w
    } else if f.eq_ignore_ascii_case("Width*0.5") {
        w * 0.5
    } else if f.eq_ignore_ascii_case("Height") || f.eq_ignore_ascii_case("Height*1") {
        h
    } else if f.eq_ignore_ascii_case("Height*0.5") {
        h * 0.5
    } else {
        ss.get_f64(cell_name).unwrap_or(0.0)
    }
}

/// Encodes a `VisioShape` into a standards-compliant binary `.emf` (Enhanced Metafile) byte stream
/// containing `EMR_HEADER`, `EMR_CREATEPEN`, `EMR_CREATEBRUSHINDIRECT`, `EMR_SELECTOBJECT`,
/// `EMR_POLYGON16` / `EMR_POLYLINE16` / `EMR_ELLIPSE`, and `EMR_EOF`.
pub fn encode_shape_to_emf(shape: &VisioShape) -> Vec<u8> {
    let ss = &shape.sheet;
    let w_in = ss.get_f64("Width").unwrap_or(2.0).abs().max(0.05);
    let h_in = ss.get_f64("Height").unwrap_or(1.0).abs().max(0.05);

    let max_dim = w_in.max(h_in).max(1.0);
    let units_per_in = 2540.0_f64.min(30_000.0 / max_dim);
    let bounds_r = (w_in * units_per_in).round() as i32;
    let bounds_b = (h_in * units_per_in).round() as i32;
    let frame_r = (w_in * 2540.0).round() as i32;
    let frame_b = (h_in * 2540.0).round() as i32;

    let mut out = Vec::with_capacity(512);
    let mut num_records = 0u32;

    // 1. Reserve 88-byte `EMR_HEADER`
    out.resize(88, 0);
    out[0..4].copy_from_slice(&EMR_HEADER.to_le_bytes());
    out[4..8].copy_from_slice(&88u32.to_le_bytes());
    // rclBounds: [0, 0, bounds_r, bounds_b]
    out[16..20].copy_from_slice(&bounds_r.to_le_bytes());
    out[20..24].copy_from_slice(&bounds_b.to_le_bytes());
    // rclFrame (0.01 mm): [0, 0, frame_r, frame_b]
    out[32..36].copy_from_slice(&frame_r.to_le_bytes());
    out[36..40].copy_from_slice(&frame_b.to_le_bytes());
    // dSignature (`" EMF"` = 0x464D4520) + nVersion (0x00010000)
    out[40..44].copy_from_slice(&EMF_SIGNATURE.to_le_bytes());
    out[44..48].copy_from_slice(&0x0001_0000u32.to_le_bytes());
    // nHandles = 3 (1=pen, 2=solid brush)
    out[56..58].copy_from_slice(&3u16.to_le_bytes());
    // szlDevice & szlMillimeters
    out[72..76].copy_from_slice(&1920i32.to_le_bytes());
    out[76..80].copy_from_slice(&1080i32.to_le_bytes());
    out[80..84].copy_from_slice(&508i32.to_le_bytes());
    out[84..88].copy_from_slice(&285i32.to_le_bytes());
    num_records += 1;

    // 2. `EMR_CREATEPEN` (handle 1) + `EMR_SELECTOBJECT` (1)
    let stroke_col = ss.get_color("LineColor").unwrap_or(RgbaColor::BLACK);
    let line_pat = ss.get_f64("LinePattern").unwrap_or(1.0) as i32;
    let lw_in = ss.get_f64("LineWeight").unwrap_or(1.0 / 72.0);
    let pen_w = (lw_in * units_per_in).round().max(1.0) as i32;
    let pen_style: u32 = if line_pat == 0 { 5 } else { 0 };
    {
        let mut rec = [0u8; 28];
        rec[0..4].copy_from_slice(&EMR_CREATEPEN.to_le_bytes());
        rec[4..8].copy_from_slice(&28u32.to_le_bytes());
        rec[8..12].copy_from_slice(&1u32.to_le_bytes());
        rec[12..16].copy_from_slice(&pen_style.to_le_bytes());
        rec[16..20].copy_from_slice(&pen_w.to_le_bytes());
        rec[24] = stroke_col.r;
        rec[25] = stroke_col.g;
        rec[26] = stroke_col.b;
        out.extend_from_slice(&rec);
        num_records += 1;

        let mut sel = [0u8; 12];
        sel[0..4].copy_from_slice(&EMR_SELECTOBJECT.to_le_bytes());
        sel[4..8].copy_from_slice(&12u32.to_le_bytes());
        sel[8..12].copy_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&sel);
        num_records += 1;
    }

    // 3. `EMR_CREATEBRUSHINDIRECT` (handle 2)
    let fill_pat = ss.get_f64("FillPattern").unwrap_or(1.0) as i32;
    let fill_col = ss.get_color("FillForegnd").unwrap_or(RgbaColor::WHITE);
    let shape_has_fill = fill_pat != 0 && shape.kind != ShapeKind::Connector1D;
    {
        let mut rec = [0u8; 24];
        rec[0..4].copy_from_slice(&EMR_CREATEBRUSHINDIRECT.to_le_bytes());
        rec[4..8].copy_from_slice(&24u32.to_le_bytes());
        rec[8..12].copy_from_slice(&2u32.to_le_bytes());
        rec[12..16].copy_from_slice(&(if shape_has_fill { 0u32 } else { 1u32 }).to_le_bytes());
        rec[16] = fill_col.r;
        rec[17] = fill_col.g;
        rec[18] = fill_col.b;
        out.extend_from_slice(&rec);
        num_records += 1;
    }

    let emit_select = |out: &mut Vec<u8>, num_records: &mut u32, handle: u32| {
        let mut sel = [0u8; 12];
        sel[0..4].copy_from_slice(&EMR_SELECTOBJECT.to_le_bytes());
        sel[4..8].copy_from_slice(&12u32.to_le_bytes());
        sel[8..12].copy_from_slice(&handle.to_le_bytes());
        out.extend_from_slice(&sel);
        *num_records += 1;
    };

    let to_emf_i16 = |vx_in: f64, vy_in: f64| -> [i16; 2] {
        let ex = (vx_in * units_per_in)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        let ey = ((h_in - vy_in) * units_per_in)
            .round()
            .clamp(i16::MIN as f64, i16::MAX as f64) as i16;
        [ex, ey]
    };

    // 4. Emit vector geometry sections
    for geom in &ss.geometries {
        if geom.deleted || geom.rows.is_empty() {
            continue;
        }
        let prefix = format!("Geometry{}", geom.index);
        let g_no_fill = geom.no_fill.eq_ignore_ascii_case("TRUE") || geom.no_fill == "1";
        let use_fill = shape_has_fill && !g_no_fill;
        emit_select(
            &mut out,
            &mut num_records,
            if use_fill { 2 } else { 0x8000_0005 },
        );

        if geom.rows.len() == 1 && geom.rows[0].kind == GeometryRowKind::Ellipse {
            let r0 = &geom.rows[0];
            let cx = eval_local_coord(ss, &format!("{prefix}.X{}", r0.ix), &r0.x, w_in, h_in);
            let cy = eval_local_coord(ss, &format!("{prefix}.Y{}", r0.ix), &r0.y, w_in, h_in);
            let ax = ss
                .get_f64(&format!("{prefix}.A{}", r0.ix))
                .unwrap_or(cx + w_in * 0.5);
            let dy = ss
                .get_f64(&format!("{prefix}.D{}", r0.ix))
                .unwrap_or(cy + h_in * 0.5);
            let rx = (ax - cx).abs();
            let ry = (dy - cy).abs();
            let [l, t] = to_emf_i16(cx - rx, cy + ry);
            let [r, b] = to_emf_i16(cx + rx, cy - ry);
            let mut rec = [0u8; 24];
            rec[0..4].copy_from_slice(&EMR_ELLIPSE.to_le_bytes());
            rec[4..8].copy_from_slice(&24u32.to_le_bytes());
            rec[8..12].copy_from_slice(&(l as i32).to_le_bytes());
            rec[12..16].copy_from_slice(&(t as i32).to_le_bytes());
            rec[16..20].copy_from_slice(&(r as i32).to_le_bytes());
            rec[20..24].copy_from_slice(&(b as i32).to_le_bytes());
            out.extend_from_slice(&rec);
            num_records += 1;
            continue;
        }

        let mut pts16: Vec<[i16; 2]> = Vec::new();
        for row in &geom.rows {
            if row.deleted {
                continue;
            }
            let is_rel = matches!(
                row.kind,
                GeometryRowKind::RelMoveTo
                    | GeometryRowKind::RelLineTo
                    | GeometryRowKind::RelCubBezTo
                    | GeometryRowKind::RelQuadBezTo
                    | GeometryRowKind::RelEllipticalArcTo
            );
            let rx = eval_local_coord(ss, &format!("{prefix}.X{}", row.ix), &row.x, w_in, h_in);
            let ry = eval_local_coord(ss, &format!("{prefix}.Y{}", row.ix), &row.y, w_in, h_in);
            let (vx, vy) = if is_rel {
                (rx * w_in, ry * h_in)
            } else {
                (rx, ry)
            };
            pts16.push(to_emf_i16(vx, vy));
        }

        if pts16.len() >= 2 {
            let mut closed = false;
            if pts16.len() >= 3 && pts16.first() == pts16.last() {
                pts16.pop();
                closed = true;
            }
            let rtype = if closed {
                EMR_POLYGON16
            } else {
                EMR_POLYLINE16
            };
            let mut min_x = i16::MAX;
            let mut min_y = i16::MAX;
            let mut max_x = i16::MIN;
            let mut max_y = i16::MIN;
            for &[px, py] in &pts16 {
                min_x = min_x.min(px);
                min_y = min_y.min(py);
                max_x = max_x.max(px);
                max_y = max_y.max(py);
            }
            let byte_len = (28 + pts16.len() * 4 + 3) & !3;
            let mut rec = vec![0u8; byte_len];
            rec[0..4].copy_from_slice(&rtype.to_le_bytes());
            rec[4..8].copy_from_slice(&(byte_len as u32).to_le_bytes());
            rec[8..12].copy_from_slice(&(min_x as i32).to_le_bytes());
            rec[12..16].copy_from_slice(&(min_y as i32).to_le_bytes());
            rec[16..20].copy_from_slice(&(max_x as i32).to_le_bytes());
            rec[20..24].copy_from_slice(&(max_y as i32).to_le_bytes());
            rec[24..28].copy_from_slice(&(pts16.len() as u32).to_le_bytes());
            for (idx, &[px, py]) in pts16.iter().enumerate() {
                let p = 28 + idx * 4;
                rec[p..p + 2].copy_from_slice(&px.to_le_bytes());
                rec[p + 2..p + 4].copy_from_slice(&py.to_le_bytes());
            }
            out.extend_from_slice(&rec);
            num_records += 1;
        }
    }

    // 5. `EMR_EOF` (20 bytes)
    let mut eof = [0u8; 20];
    eof[0..4].copy_from_slice(&EMR_EOF.to_le_bytes());
    eof[4..8].copy_from_slice(&20u32.to_le_bytes());
    eof[12..16].copy_from_slice(&16u32.to_le_bytes());
    eof[16..20].copy_from_slice(&20u32.to_le_bytes());
    out.extend_from_slice(&eof);
    num_records += 1;

    // Patch `EMR_HEADER.nBytes` and `EMR_HEADER.nRecords`
    let total_bytes = out.len() as u32;
    out[48..52].copy_from_slice(&total_bytes.to_le_bytes());
    out[52..56].copy_from_slice(&num_records.to_le_bytes());
    out
}

/// Parses an Aldus Placeable (`0x9AC6CDD7` magic + 22-byte header) or standard 18-byte Windows
/// Metafile (`.wmf`) stream into a normalized vector `VisioShape` at `(pin_x_in, pin_y_in)`.
pub fn parse_wmf_to_shape(
    id: u64,
    name: &str,
    wmf_bytes: &[u8],
    pin_x_in: f64,
    pin_y_in: f64,
) -> Result<VisioShape, String> {
    if wmf_bytes.len() < 18 {
        return Err("WMF buffer shorter than 18-byte META_HEADER".into());
    }

    let mut bounds_opt: Option<[f64; 4]> = None;
    let mut size_in_opt: Option<[f64; 2]> = None;
    let mut hdr_off = 0usize;

    if wmf_bytes.len() >= 40 && read_u32_le(wmf_bytes, 0) == ALDUS_WMF_MAGIC {
        let left = read_i16_le(wmf_bytes, 6) as f64;
        let top = read_i16_le(wmf_bytes, 8) as f64;
        let right = read_i16_le(wmf_bytes, 10) as f64;
        let bottom = read_i16_le(wmf_bytes, 12) as f64;
        let units_per_in = read_u16_le(wmf_bytes, 14) as f64;
        bounds_opt = Some([left, top, right, bottom]);
        if units_per_in > 0.0
            && (right - left).abs() > 0.0
            && (bottom - top).abs() > 0.0
        {
            size_in_opt = Some([
                (right - left).abs() / units_per_in,
                (bottom - top).abs() / units_per_in,
            ]);
        }
        hdr_off = 22;
    }

    if hdr_off + 18 > wmf_bytes.len() {
        return Err("Truncated WMF META_HEADER after Aldus Placeable header".into());
    }
    let mt_type = read_u16_le(wmf_bytes, hdr_off);
    let mt_hdr_words = read_u16_le(wmf_bytes, hdr_off + 2) as usize;
    if (mt_type != 1 && mt_type != 2) || mt_hdr_words < 9 {
        return Err(format!(
            "Invalid WMF META_HEADER (type={mt_type}, header_words={mt_hdr_words})"
        ));
    }

    let mut pos = hdr_off + mt_hdr_words * 2;
    let mut win_org: Option<[f64; 2]> = None;
    let mut win_ext: Option<[f64; 2]> = None;

    let mut gdi_slots: Vec<Option<GdiObject>> = Vec::new();
    let mut pen_null = false;
    let mut pen_color = RgbaColor::BLACK;
    let mut brush_null = false;
    let mut brush_color = RgbaColor::WHITE;
    let mut active_stroke = RgbaColor::BLACK;
    let mut active_fill: Option<RgbaColor> = None;

    let mut cur_pt = [0.0f64, 0.0f64];
    let mut open_strip: Vec<[f64; 2]> = Vec::new();
    let mut primitives: Vec<MetaPrimitive> = Vec::new();

    let flush_wmf_strip = |strip: &mut Vec<[f64; 2]>,
                           prims: &mut Vec<MetaPrimitive>,
                           lined: bool| {
        if strip.len() >= 2 {
            prims.push(MetaPrimitive::Polyline {
                pts: std::mem::take(strip),
                closed: false,
                filled: false,
                lined,
            });
        } else {
            strip.clear();
        }
    };

    let alloc_wmf_slot = |slots: &mut Vec<Option<GdiObject>>, obj: GdiObject| {
        if let Some(empty) = slots.iter_mut().find(|s| s.is_none()) {
            *empty = Some(obj);
        } else {
            slots.push(Some(obj));
        }
    };

    while pos + 6 <= wmf_bytes.len() {
        let rd_size_words = read_u32_le(wmf_bytes, pos) as usize;
        let rd_func = read_u16_le(wmf_bytes, pos + 4);
        if rd_func == META_EOF {
            break;
        }
        let byte_len = rd_size_words.saturating_mul(2);
        if byte_len < 6 || pos + byte_len > wmf_bytes.len() {
            return Err(format!(
                "Malformed WMF record 0x{rd_func:04X} (words={rd_size_words}) at offset {pos}"
            ));
        }
        let rec = &wmf_bytes[pos..pos + byte_len];

        if rd_func != META_MOVETO && rd_func != META_LINETO {
            flush_wmf_strip(&mut open_strip, &mut primitives, !pen_null);
        }

        match rd_func {
            META_SETWINDOWORG if byte_len >= 10 => {
                let y = read_i16_le(rec, 6) as f64;
                let x = read_i16_le(rec, 8) as f64;
                win_org = Some([x, y]);
            }
            META_SETWINDOWEXT if byte_len >= 10 => {
                let y = read_i16_le(rec, 6) as f64;
                let x = read_i16_le(rec, 8) as f64;
                win_ext = Some([x, y]);
            }
            META_MOVETO if byte_len >= 10 => {
                flush_wmf_strip(&mut open_strip, &mut primitives, !pen_null);
                let y = read_i16_le(rec, 6) as f64;
                let x = read_i16_le(rec, 8) as f64;
                cur_pt = [x, y];
                open_strip.push(cur_pt);
            }
            META_LINETO if byte_len >= 10 => {
                let y = read_i16_le(rec, 6) as f64;
                let x = read_i16_le(rec, 8) as f64;
                if open_strip.is_empty() {
                    open_strip.push(cur_pt);
                }
                cur_pt = [x, y];
                open_strip.push(cur_pt);
                if !pen_null {
                    active_stroke = pen_color;
                }
            }
            META_RECTANGLE if byte_len >= 14 => {
                let b = read_i16_le(rec, 6) as f64;
                let r = read_i16_le(rec, 8) as f64;
                let t = read_i16_le(rec, 10) as f64;
                let l = read_i16_le(rec, 12) as f64;
                if !brush_null {
                    active_fill = Some(brush_color);
                }
                if !pen_null {
                    active_stroke = pen_color;
                }
                primitives.push(MetaPrimitive::Polyline {
                    pts: vec![[l, t], [r, t], [r, b], [l, b]],
                    closed: true,
                    filled: !brush_null,
                    lined: !pen_null,
                });
            }
            META_ELLIPSE if byte_len >= 14 => {
                let b = read_i16_le(rec, 6) as f64;
                let r = read_i16_le(rec, 8) as f64;
                let t = read_i16_le(rec, 10) as f64;
                let l = read_i16_le(rec, 12) as f64;
                if !brush_null {
                    active_fill = Some(brush_color);
                }
                if !pen_null {
                    active_stroke = pen_color;
                }
                primitives.push(MetaPrimitive::Ellipse {
                    left: l,
                    top: t,
                    right: r,
                    bottom: b,
                    filled: !brush_null,
                    lined: !pen_null,
                });
            }
            META_POLYGON | META_POLYLINE if byte_len >= 8 => {
                let npts = read_u16_le(rec, 6) as usize;
                let needed = 8usize.saturating_add(npts.saturating_mul(4));
                if needed <= byte_len && npts >= 2 {
                    let mut pts = Vec::with_capacity(npts);
                    for i in 0..npts {
                        let p = 8 + i * 4;
                        pts.push([read_i16_le(rec, p) as f64, read_i16_le(rec, p + 2) as f64]);
                    }
                    let is_poly = rd_func == META_POLYGON;
                    if is_poly && !brush_null {
                        active_fill = Some(brush_color);
                    }
                    if !pen_null {
                        active_stroke = pen_color;
                    }
                    primitives.push(MetaPrimitive::Polyline {
                        pts,
                        closed: is_poly,
                        filled: is_poly && !brush_null,
                        lined: !pen_null,
                    });
                }
            }
            META_CREATEPENINDIRECT if byte_len >= 16 => {
                let style = read_u16_le(rec, 6);
                let width_x = read_i16_le(rec, 8) as i32;
                let col = RgbaColor {
                    r: rec[12],
                    g: rec[13],
                    b: rec[14],
                    a: 255,
                };
                alloc_wmf_slot(
                    &mut gdi_slots,
                    GdiObject::Pen {
                        is_null: (style & 0x0F) == 5,
                        width_logical: width_x.max(1),
                        color: col,
                    },
                );
            }
            META_CREATEBRUSHINDIRECT if byte_len >= 14 => {
                let style = read_u16_le(rec, 6);
                let col = RgbaColor {
                    r: rec[8],
                    g: rec[9],
                    b: rec[10],
                    a: 255,
                };
                alloc_wmf_slot(
                    &mut gdi_slots,
                    GdiObject::Brush {
                        is_null: style == 1,
                        color: col,
                    },
                );
            }
            META_SELECTOBJECT if byte_len >= 8 => {
                let idx = read_u16_le(rec, 6) as usize;
                if let Some(Some(obj)) = gdi_slots.get(idx) {
                    match *obj {
                        GdiObject::Pen { is_null, color, .. } => {
                            pen_null = is_null;
                            pen_color = color;
                            if !is_null {
                                active_stroke = color;
                            }
                        }
                        GdiObject::Brush { is_null, color } => {
                            brush_null = is_null;
                            brush_color = color;
                        }
                    }
                }
            }
            META_DELETEOBJECT if byte_len >= 8 => {
                let idx = read_u16_le(rec, 6) as usize;
                if let Some(slot) = gdi_slots.get_mut(idx) {
                    *slot = None;
                }
            }
            _ => {}
        }

        pos += byte_len;
    }
    flush_wmf_strip(&mut open_strip, &mut primitives, !pen_null);

    if bounds_opt.is_none() {
        if let (Some(org), Some(ext)) = (win_org, win_ext) {
            bounds_opt = Some([org[0], org[1], org[0] + ext[0], org[1] + ext[1]]);
        }
    }

    Ok(build_shape_from_metafile_primitives(
        id,
        name,
        pin_x_in,
        pin_y_in,
        bounds_opt,
        size_in_opt,
        &primitives,
        active_stroke,
        1.0,
        active_fill,
        "WMF",
        wmf_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visio_v5_short_pointer_stream_roundtrip() {
        let mut doc = VisioDocument::new_blank("Visio 5.0c Schematics");
        {
            let page = doc.page_mut(1).unwrap();
            let mut valve = VisioShape::new_rect(1, "GateValve", 2.5, 3.5, 1.5, 1.0);
            valve.text = "HV-501".into();
            valve
                .sheet
                .set_formula("FillForegnd", "=RGB(37,99,235)", Some(Unit::Color));
            valve
                .sheet
                .set_user_cell("HalfW", "=GUARD(Width * 0.5)", "Guarded half width");
            page.shapes.push(valve);

            let mut vessel = VisioShape::new_rect(2, "Separator", 7.0, 3.5, 2.0, 3.0);
            vessel.text = "V-502".into();
            page.shapes.push(vessel);

            let line = VisioShape::new_connector(3, "FeedPipe", 3.25, 3.5, 6.0, 3.5);
            page.shapes.push(line);
            page.connects.push(GlueEndpoint {
                connector_shape_id: 3,
                from_cell: "BeginX".into(),
                target_shape_id: 1,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
        }
        ReactiveEngine::recalculate_document(&mut doc);

        let v5_bytes = encode_visio_v5_stream(&doc);
        assert_eq!(&v5_bytes[0..21], VSD_HEADER_MAGIC);
        assert_eq!(v5_bytes[0x1A], 5);

        let decoded = parse_visio_v5_stream(&v5_bytes).unwrap();
        assert_eq!(decoded.properties.title, "Visio 5.0c Schematics");
        let p1 = decoded.page(1).unwrap();
        assert_eq!(p1.shapes.len(), 3);
        let s1 = p1.find_shape(1).unwrap();
        assert_eq!(s1.text, "HV-501");
        assert!((s1.sheet.get_f64("User.HalfW").unwrap() - 0.75).abs() < 1e-9);
        assert!(s1.sheet.get_cell("User.HalfW").unwrap().is_guarded());
        assert_eq!(p1.connects.len(), 1);
    }

    #[test]
    fn rpn_formula_bytecode_compile_and_decompile_roundtrip() {
        for formula in [
            "=GUARD(Width*0.5 + 2 in)",
            "=RGB(37,99,235)",
            "=IF(Width >= 3 in, MAX(Height, 1.5 in), MIN(PinX, 0.5 in))",
            "=PAR(PNT(Sheet.1!Controls.Row_1.X, Sheet.1!Controls.Row_1.Y))",
            "=SIN(45 deg)^2 + COS(45 deg)^2",
        ] {
            let orig_ast = parse_formula(formula).unwrap();
            let bytecode = compile_formula_to_vsd_bytecode(formula).unwrap();
            assert!(!bytecode.is_empty());

            let decompiled = decompile_vsd_bytecode_to_formula(&bytecode).unwrap();
            assert!(decompiled.starts_with('='));
            let roundtrip_ast = parse_formula(&decompiled).unwrap();
            assert_eq!(
                roundtrip_ast, orig_ast,
                "AST mismatch for '{formula}' -> '{decompiled}'"
            );
        }

        let guard_bc = compile_formula_to_vsd_bytecode("=GUARD(Width*0.5 + 2 in)").unwrap();
        assert!(guard_bc.contains(&VSD_OP_PUSH_CELL_REF));
        assert!(guard_bc.contains(&VSD_OP_PUSH_NUM_UNIT));
        assert!(guard_bc.contains(&VSD_OP_MUL));
        assert!(guard_bc.contains(&VSD_OP_ADD));
        assert!(guard_bc.contains(&VSD_OP_FUNC_CALL));
        assert_eq!(
            decompile_vsd_bytecode_to_formula(&guard_bc).unwrap(),
            "=GUARD(Width * 0.5 + 2 in)"
        );

        let rgb_bc = compile_formula_to_vsd_bytecode("=RGB(37,99,235)").unwrap();
        assert_eq!(
            decompile_vsd_bytecode_to_formula(&rgb_bc).unwrap(),
            "=RGB(37, 99, 235)"
        );
    }

    #[test]
    fn emf_encode_and_parse_vector_roundtrip() {
        let mut src = VisioShape::new_rect(10, "EmfBadge", 4.0, 5.0, 3.0, 2.0);
        src.sheet
            .set_formula("FillForegnd", "=RGB(16,185,129)", Some(Unit::Color));
        src.sheet
            .set_formula("LineColor", "=RGB(15,23,42)", Some(Unit::Color));

        let emf_bytes = encode_shape_to_emf(&src);
        assert!(emf_bytes.len() >= 108);
        assert_eq!(&emf_bytes[40..44], b" EMF");

        let parsed = parse_emf_to_shape(99, "ImportedEmf", &emf_bytes, 6.0, 7.0).unwrap();
        assert_eq!(parsed.id, 99);
        assert!((parsed.sheet.get_f64("Width").unwrap() - 3.0).abs() < 1e-2);
        assert!((parsed.sheet.get_f64("Height").unwrap() - 2.0).abs() < 1e-2);
        assert!((parsed.sheet.get_f64("PinX").unwrap() - 6.0).abs() < 1e-6);
        assert!((parsed.sheet.get_f64("PinY").unwrap() - 7.0).abs() < 1e-6);
        assert!(!parsed.sheet.geometries.is_empty());
        assert!(parsed.sheet.geometries[0].rows.len() >= 4);
        assert_eq!(
            parsed.sheet.get_color("FillForegnd"),
            Some(RgbaColor {
                r: 16,
                g: 185,
                b: 129,
                a: 255
            })
        );
        assert_eq!(
            parsed.sheet.get_color("LineColor"),
            Some(RgbaColor {
                r: 15,
                g: 23,
                b: 42,
                a: 255
            })
        );
    }

    #[test]
    fn placeable_wmf_vector_parsing() {
        let mut wmf = Vec::new();
        // 1. 22-byte Aldus Placeable WMF Header (`0x9AC6CDD7`, bbox [0, 0, 2880, 1440] @ 1440 twips/in => 2.0 in x 1.0 in)
        wmf.extend_from_slice(&ALDUS_WMF_MAGIC.to_le_bytes());
        wmf.extend_from_slice(&0u16.to_le_bytes()); // hmf
        wmf.extend_from_slice(&0i16.to_le_bytes()); // left
        wmf.extend_from_slice(&0i16.to_le_bytes()); // top
        wmf.extend_from_slice(&2880i16.to_le_bytes()); // right
        wmf.extend_from_slice(&1440i16.to_le_bytes()); // bottom
        wmf.extend_from_slice(&1440u16.to_le_bytes()); // inch = 1440
        wmf.extend_from_slice(&0u32.to_le_bytes()); // reserved
        let mut checksum = 0u16;
        for chunk in wmf[0..20].chunks_exact(2) {
            checksum ^= u16::from_le_bytes([chunk[0], chunk[1]]);
        }
        wmf.extend_from_slice(&checksum.to_le_bytes());
        assert_eq!(wmf.len(), 22);

        // 2. 18-byte Standard WMF `META_HEADER`
        let hdr_pos = wmf.len();
        wmf.extend_from_slice(&1u16.to_le_bytes()); // Type = 1
        wmf.extend_from_slice(&9u16.to_le_bytes()); // HeaderSize = 9 words (18 bytes)
        wmf.extend_from_slice(&0x0300u16.to_le_bytes()); // Version 3.0
        wmf.extend_from_slice(&0u32.to_le_bytes()); // Size placeholder
        wmf.extend_from_slice(&2u16.to_le_bytes()); // NumberOfObjects
        wmf.extend_from_slice(&16u32.to_le_bytes()); // MaxRecord
        wmf.extend_from_slice(&0u16.to_le_bytes()); // NumberOfMembers

        // 3. `META_RECTANGLE` (7 words = 14 bytes: bottom=1440, right=2880, top=0, left=0)
        wmf.extend_from_slice(&7u32.to_le_bytes());
        wmf.extend_from_slice(&META_RECTANGLE.to_le_bytes());
        wmf.extend_from_slice(&1440i16.to_le_bytes());
        wmf.extend_from_slice(&2880i16.to_le_bytes());
        wmf.extend_from_slice(&0i16.to_le_bytes());
        wmf.extend_from_slice(&0i16.to_le_bytes());

        // 4. `META_MOVETO` (5 words) + `META_LINETO` (5 words) diagonal
        wmf.extend_from_slice(&5u32.to_le_bytes());
        wmf.extend_from_slice(&META_MOVETO.to_le_bytes());
        wmf.extend_from_slice(&0i16.to_le_bytes()); // Y
        wmf.extend_from_slice(&0i16.to_le_bytes()); // X

        wmf.extend_from_slice(&5u32.to_le_bytes());
        wmf.extend_from_slice(&META_LINETO.to_le_bytes());
        wmf.extend_from_slice(&1440i16.to_le_bytes()); // Y
        wmf.extend_from_slice(&2880i16.to_le_bytes()); // X

        // 5. `META_POLYGON` (3 + 1 + 6 = 10 words = 20 bytes: 3 triangle points)
        wmf.extend_from_slice(&10u32.to_le_bytes());
        wmf.extend_from_slice(&META_POLYGON.to_le_bytes());
        wmf.extend_from_slice(&3u16.to_le_bytes());
        for (px, py) in [(200i16, 200i16), (1200, 200), (700, 1000)] {
            wmf.extend_from_slice(&px.to_le_bytes());
            wmf.extend_from_slice(&py.to_le_bytes());
        }

        // 6. `META_EOF` (3 words = 6 bytes)
        wmf.extend_from_slice(&3u32.to_le_bytes());
        wmf.extend_from_slice(&META_EOF.to_le_bytes());

        let size_words = ((wmf.len() - hdr_pos) / 2) as u32;
        wmf[hdr_pos + 6..hdr_pos + 10].copy_from_slice(&size_words.to_le_bytes());

        let shape = parse_wmf_to_shape(42, "AldusSymbol", &wmf, 3.0, 4.0).unwrap();
        assert_eq!(shape.id, 42);
        assert!((shape.sheet.get_f64("Width").unwrap() - 2.0).abs() < 1e-4);
        assert!((shape.sheet.get_f64("Height").unwrap() - 1.0).abs() < 1e-4);
        assert_eq!(shape.sheet.geometries.len(), 3);
    }
}
