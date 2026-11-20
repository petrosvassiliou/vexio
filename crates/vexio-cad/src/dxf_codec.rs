//! Native Offline AutoCAD DXF (ASCII & Binary) Reader and R2000+ Exporter.
//!
//! Supports:
//! - ASCII DXF and Binary DXF (`AutoCAD Binary DXF\r\n\x1a\0` sentinel + binary group pairs)
//! - `HEADER` (`$INSUNITS`, `$EXTMIN`, `$EXTMAX`)
//! - `TABLES` -> `LAYER` (`2` name, `62` ACI color with negative=off, `420` truecolor,
//!   `70` flags bit 1=frozen & bit 4=locked) mapped to `VisioPage::layers` (`LayerDef`)
//! - `BLOCKS` (`BLOCK`..`ENDBLK`) instantiated via `INSERT` with base point, translation
//!   `(10,20)`, non-uniform scale `(41,42)`, rotation `(50)`, and rectangular arrays
//! - `ENTITIES`: `LINE`, `LWPOLYLINE` (with `42` bulge -> `ArcTo` bow), `POLYLINE`/`VERTEX`/`SEQEND`,
//!   `CIRCLE`, `ARC`, `ELLIPSE`, `SPLINE` (De Boor NURBS + Catmull-Rom fit points), `TEXT`,
//!   `MTEXT` (stripping formatting codes including `\P` -> `\n`), `SOLID`, `HATCH` boundary loops,
//!   and `DIMENSION`
//! - `export_page_to_dxf`: R2000+ (`AC1015`) ASCII DXF serializer driven by
//!   `ReactiveEngine::evaluate_page_visuals` that round-trips cleanly back through `import_dxf_to_page`.

use crate::CadError;
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use vexio_shapesheet::{
    CellValue, EvaluatedPathVerb, GeometryRow, GeometryRowKind, GeometrySection, LayerDef,
    ReactiveEngine, RgbaColor, ShapeKind, ShapeSheet, Unit, VisioDocument, VisioPage, VisioShape,
    format_clean_f64,
};

/// Exact 22-byte AutoCAD Binary DXF file header sentinel.
pub const BINARY_DXF_SENTINEL: &[u8; 22] = b"AutoCAD Binary DXF\r\n\x1a\0";

#[derive(Debug, Clone, PartialEq)]
pub enum DxfValue {
    Str(String),
    Float(f64),
    Int(i64),
    Bool(bool),
    Binary(Vec<u8>),
}

impl DxfValue {
    pub fn as_str(&self) -> String {
        match self {
            Self::Str(s) => s.clone(),
            Self::Float(v) => format_clean_f64(*v),
            Self::Int(i) => i.to_string(),
            Self::Bool(b) => if *b { "1".into() } else { "0".into() },
            Self::Binary(bytes) => {
                let mut hex = String::with_capacity(bytes.len() * 2);
                for b in bytes {
                    hex.push_str(&format!("{b:02X}"));
                }
                hex
            }
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => {
                if v.is_finite() {
                    Some(*v)
                } else {
                    Some(0.0)
                }
            }
            Self::Int(i) => Some(*i as f64),
            Self::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Self::Str(s) => s.trim().parse::<f64>().ok().filter(|v| v.is_finite()),
            Self::Binary(_) => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Bool(b) => Some(if *b { 1 } else { 0 }),
            Self::Float(v) if v.is_finite() => Some(v.trunc() as i64),
            Self::Str(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupValueKind {
    Str,
    Float64,
    Int16,
    Int32,
    Int64,
    Bool,
    BinaryChunk,
}

fn group_value_kind(code: i32) -> GroupValueKind {
    match code {
        0..=9 | 100 | 102 | 105 | 300..=309 | 320..=369 | 390..=399 | 410..=419 | 430..=439
        | 470..=481 | 999 | 1000..=1003 | 1005..=1009 => GroupValueKind::Str,
        10..=59 | 110..=149 | 210..=239 | 460..=469 | 1010..=1059 => GroupValueKind::Float64,
        60..=79 | 170..=179 | 270..=289 | 370..=389 | 400..=409 | 1060..=1070 => {
            GroupValueKind::Int16
        }
        90..=99 | 420..=429 | 440..=459 | 1071 => GroupValueKind::Int32,
        160..=169 => GroupValueKind::Int64,
        290..=299 => GroupValueKind::Bool,
        310..=319 | 1004 => GroupValueKind::BinaryChunk,
        _ => GroupValueKind::Str,
    }
}

/// Tokenize either Binary DXF or ASCII DXF into `(group_code, DxfValue)` pairs.
pub fn parse_dxf_groups(bytes: &[u8]) -> Result<Vec<(i32, DxfValue)>, CadError> {
    if bytes.starts_with(BINARY_DXF_SENTINEL) {
        parse_binary_dxf_groups(&bytes[BINARY_DXF_SENTINEL.len()..])
    } else {
        let text = std::str::from_utf8(bytes)
            .map_err(|e| CadError::InvalidDxf(format!("DXF is not valid UTF-8 or Binary DXF: {e}")))?;
        parse_ascii_dxf_groups(text)
    }
}

fn parse_ascii_dxf_groups(text: &str) -> Result<Vec<(i32, DxfValue)>, CadError> {
    let mut lines = text.lines();
    let mut groups = Vec::new();
    while let Some(code_line) = lines.next() {
        let trimmed = code_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let code: i32 = trimmed.parse().map_err(|_| {
            CadError::InvalidDxf(format!("Expected integer DXF group code, found '{trimmed}'"))
        })?;
        let Some(val_line) = lines.next() else {
            return Err(CadError::InvalidDxf(format!(
                "Truncated ASCII DXF: missing value line after group code {code}"
            )));
        };
        let val_str = val_line.strip_suffix('\r').unwrap_or(val_line);
        let value = match group_value_kind(code) {
            GroupValueKind::Str | GroupValueKind::BinaryChunk => {
                DxfValue::Str(val_str.trim().to_string())
            }
            GroupValueKind::Float64 => {
                let f = val_str.trim().parse::<f64>().map_err(|_| {
                    CadError::InvalidDxf(format!(
                        "Invalid float '{val_str}' for DXF group code {code}"
                    ))
                })?;
                DxfValue::Float(if f == 0.0 || !f.is_finite() { 0.0 } else { f })
            }
            GroupValueKind::Int16 | GroupValueKind::Int32 | GroupValueKind::Int64 => {
                let n = val_str.trim().parse::<i64>().map_err(|_| {
                    CadError::InvalidDxf(format!(
                        "Invalid integer '{val_str}' for DXF group code {code}"
                    ))
                })?;
                DxfValue::Int(n)
            }
            GroupValueKind::Bool => {
                let b = val_str.trim().parse::<i64>().unwrap_or(0) != 0;
                DxfValue::Bool(b)
            }
        };
        let is_eof = code == 0 && val_str.trim().eq_ignore_ascii_case("EOF");
        groups.push((code, value));
        if is_eof {
            break;
        }
    }
    if groups.is_empty() {
        return Err(CadError::InvalidDxf("Empty DXF input".into()));
    }
    Ok(groups)
}

fn parse_binary_dxf_groups(buf: &[u8]) -> Result<Vec<(i32, DxfValue)>, CadError> {
    if buf.len() < 2 {
        return Err(CadError::InvalidDxf("Truncated Binary DXF payload".into()));
    }
    // AutoCAD R13+ uses 2-byte LE group codes (starts with `0x00, 0x00, 'S', ...`),
    // while pre-R13 uses 1-byte group codes (`0x00, 'S', ...`, with `0xFF` escape for extended).
    let two_byte_codes = !(buf[0] == 0 && buf.get(1).copied().unwrap_or(0) != 0);
    let mut pos = 0usize;
    let mut groups = Vec::new();

    while pos < buf.len() {
        let code: i32 = if two_byte_codes {
            if pos + 2 > buf.len() {
                break;
            }
            let c = i16::from_le_bytes([buf[pos], buf[pos + 1]]) as i32;
            pos += 2;
            c
        } else {
            let b = buf[pos];
            pos += 1;
            if b == 0xFF {
                if pos + 2 > buf.len() {
                    return Err(CadError::InvalidDxf("Truncated extended Binary DXF code".into()));
                }
                let c = i16::from_le_bytes([buf[pos], buf[pos + 1]]) as i32;
                pos += 2;
                c
            } else {
                b as i32
            }
        };

        let value = match group_value_kind(code) {
            GroupValueKind::Str => {
                let start = pos;
                while pos < buf.len() && buf[pos] != 0 {
                    pos += 1;
                }
                if pos >= buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Unterminated NUL string in Binary DXF".into(),
                    ));
                }
                let s = String::from_utf8_lossy(&buf[start..pos]).into_owned();
                pos += 1; // consume NUL
                DxfValue::Str(s)
            }
            GroupValueKind::Float64 => {
                if pos + 8 > buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated f64 in Binary DXF".into(),
                    ));
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&buf[pos..pos + 8]);
                pos += 8;
                let f = f64::from_le_bytes(arr);
                DxfValue::Float(if f == 0.0 || !f.is_finite() { 0.0 } else { f })
            }
            GroupValueKind::Int16 => {
                if pos + 2 > buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated i16 in Binary DXF".into(),
                    ));
                }
                let n = i16::from_le_bytes([buf[pos], buf[pos + 1]]) as i64;
                pos += 2;
                DxfValue::Int(n)
            }
            GroupValueKind::Int32 => {
                if pos + 4 > buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated i32 in Binary DXF".into(),
                    ));
                }
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&buf[pos..pos + 4]);
                pos += 4;
                DxfValue::Int(i32::from_le_bytes(arr) as i64)
            }
            GroupValueKind::Int64 => {
                if pos + 8 > buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated i64 in Binary DXF".into(),
                    ));
                }
                let mut arr = [0u8; 8];
                arr.copy_from_slice(&buf[pos..pos + 8]);
                pos += 8;
                DxfValue::Int(i64::from_le_bytes(arr))
            }
            GroupValueKind::Bool => {
                if pos >= buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated bool in Binary DXF".into(),
                    ));
                }
                let b = buf[pos] != 0;
                pos += 1;
                DxfValue::Bool(b)
            }
            GroupValueKind::BinaryChunk => {
                if pos >= buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated binary chunk length in Binary DXF".into(),
                    ));
                }
                let len = buf[pos] as usize;
                pos += 1;
                if pos + len > buf.len() {
                    return Err(CadError::InvalidDxf(
                        "Truncated binary chunk data in Binary DXF".into(),
                    ));
                }
                let chunk = buf[pos..pos + len].to_vec();
                pos += len;
                DxfValue::Binary(chunk)
            }
        };

        let is_eof = matches!((code, &value), (0, DxfValue::Str(s)) if s.eq_ignore_ascii_case("EOF"));
        groups.push((code, value));
        if is_eof {
            break;
        }
    }
    if groups.is_empty() {
        return Err(CadError::InvalidDxf("Empty Binary DXF stream".into()));
    }
    Ok(groups)
}

/// Encode an ASCII DXF document into a valid R2000+ AutoCAD Binary DXF byte buffer.
pub fn encode_binary_dxf_from_ascii(ascii_dxf: &str) -> Result<Vec<u8>, CadError> {
    let groups = parse_ascii_dxf_groups(ascii_dxf)?;
    let mut out = Vec::with_capacity(BINARY_DXF_SENTINEL.len() + groups.len() * 12);
    out.extend_from_slice(BINARY_DXF_SENTINEL);
    for (code, val) in groups {
        out.extend_from_slice(&(code as i16).to_le_bytes());
        match group_value_kind(code) {
            GroupValueKind::Str => {
                out.extend_from_slice(val.as_str().as_bytes());
                out.push(0);
            }
            GroupValueKind::Float64 => {
                let f = val.as_f64().unwrap_or(0.0);
                out.extend_from_slice(&f.to_le_bytes());
            }
            GroupValueKind::Int16 => {
                let n = val.as_i64().unwrap_or(0) as i16;
                out.extend_from_slice(&n.to_le_bytes());
            }
            GroupValueKind::Int32 => {
                let n = val.as_i64().unwrap_or(0) as i32;
                out.extend_from_slice(&n.to_le_bytes());
            }
            GroupValueKind::Int64 => {
                let n = val.as_i64().unwrap_or(0);
                out.extend_from_slice(&n.to_le_bytes());
            }
            GroupValueKind::Bool => {
                out.push(if val.as_i64().unwrap_or(0) != 0 { 1 } else { 0 });
            }
            GroupValueKind::BinaryChunk => match val {
                DxfValue::Binary(b) => {
                    let n = b.len().min(255);
                    out.push(n as u8);
                    out.extend_from_slice(&b[..n]);
                }
                _ => out.push(0),
            },
        }
    }
    Ok(out)
}

/// Convert AutoCAD Color Index (`1..=255`) into `RgbaColor`.
pub fn aci_to_rgba(aci: u16) -> RgbaColor {
    match aci {
        1 => RgbaColor { r: 255, g: 0, b: 0, a: 255 },
        2 => RgbaColor { r: 255, g: 255, b: 0, a: 255 },
        3 => RgbaColor { r: 0, g: 255, b: 0, a: 255 },
        4 => RgbaColor { r: 0, g: 255, b: 255, a: 255 },
        5 => RgbaColor { r: 0, g: 0, b: 255, a: 255 },
        6 => RgbaColor { r: 255, g: 0, b: 255, a: 255 },
        0 | 7 | 256.. => RgbaColor::BLACK,
        8 => RgbaColor { r: 128, g: 128, b: 128, a: 255 },
        9 => RgbaColor { r: 192, g: 192, b: 192, a: 255 },
        250 => RgbaColor { r: 51, g: 51, b: 51, a: 255 },
        251 => RgbaColor { r: 91, g: 91, b: 91, a: 255 },
        252 => RgbaColor { r: 132, g: 132, b: 132, a: 255 },
        253 => RgbaColor { r: 173, g: 173, b: 173, a: 255 },
        254 => RgbaColor { r: 214, g: 214, b: 214, a: 255 },
        255 => RgbaColor::WHITE,
        10..=249 => {
            // AutoCAD 24-hue spectral wheel (10..249: 24 hue columns * 10 brightness/saturation steps).
            let idx = aci - 10;
            let hue_step = (idx / 10) as f64; // 0..23 -> 0..345 deg
            let shade = idx % 10;
            let hue_deg = hue_step * 15.0;
            let (val_scale, pastel) = match shade {
                0 => (1.00, false),
                1 => (1.00, true),
                2 => (0.80, false),
                3 => (0.80, true),
                4 => (0.60, false),
                5 => (0.60, true),
                6 => (0.50, false),
                7 => (0.50, true),
                8 => (0.30, false),
                _ => (0.30, true),
            };
            let s = if pastel { 0.5 } else { 1.0 };
            let c = val_scale * s;
            let x = c * (1.0 - ((hue_deg / 60.0) % 2.0 - 1.0).abs());
            let m = val_scale - c;
            let (r1, g1, b1) = match (hue_deg as u32) / 60 {
                0 => (c, x, 0.0),
                1 => (x, c, 0.0),
                2 => (0.0, c, x),
                3 => (0.0, x, c),
                4 => (x, 0.0, c),
                _ => (c, 0.0, x),
            };
            RgbaColor {
                r: ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
                g: ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
                b: ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
                a: 255,
            }
        }
    }
}

/// Map an `RgbaColor` to the closest AutoCAD Color Index (`1..=255`).
pub fn rgba_to_aci(color: RgbaColor) -> i16 {
    if color.r == 0 && color.g == 0 && color.b == 0 {
        return 7;
    }
    let mut best_aci = 7i16;
    let mut best_dist = i32::MAX;
    for candidate in 1u16..=255u16 {
        let c = aci_to_rgba(candidate);
        let dr = color.r as i32 - c.r as i32;
        let dg = color.g as i32 - c.g as i32;
        let db = color.b as i32 - c.b as i32;
        let dist = dr * dr + dg * dg + db * db;
        if dist == 0 {
            return candidate as i16;
        }
        if dist < best_dist {
            best_dist = dist;
            best_aci = candidate as i16;
        }
    }
    best_aci
}

/// Convert AutoCAD `$INSUNITS` integer code into `(Unit, scale_to_inches)`.
pub(crate) fn insunits_to_unit_and_factor(insunits: i32) -> (Unit, f64) {
    match insunits {
        1 => (Unit::Inch, 1.0),
        2 => (Unit::Foot, 12.0),
        3 => (Unit::Mile, 63_360.0),
        4 => (Unit::Millimeter, 1.0 / 25.4),
        5 => (Unit::Centimeter, 1.0 / 2.54),
        6 => (Unit::Meter, 1.0 / 0.0254),
        7 => (Unit::Kilometer, 1.0 / 0.000_025_4),
        8 => (Unit::Inch, 1e-6),
        9 => (Unit::Inch, 1e-3),
        10 => (Unit::Yard, 36.0),
        14 => (Unit::Centimeter, 10.0 / 2.54),
        _ => (Unit::Inch, 1.0),
    }
}

/// Strip AutoCAD MTEXT formatting codes (`\P` -> `\n`, `\f...;`, `\H...;`, `\C...;`, `{...}`, `\S...;`, etc.).
pub fn strip_mtext_formatting(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '{' || ch == '}' {
            i += 1;
            continue;
        }
        if ch == '%' && i + 2 < chars.len() && chars[i + 1] == '%' {
            match chars[i + 2].to_ascii_lowercase() {
                'd' => out.push('°'),
                'p' => out.push('±'),
                'c' => out.push('⌀'),
                '%' => out.push('%'),
                _ => {}
            }
            i += 3;
            continue;
        }
        if ch != '\\' {
            out.push(ch);
            i += 1;
            continue;
        }
        if i + 1 >= chars.len() {
            break;
        }
        let cmd = chars[i + 1];
        match cmd {
            'P' => {
                out.push('\n');
                i += 2;
            }
            '~' => {
                out.push(' ');
                i += 2;
            }
            '\\' | '{' | '}' => {
                out.push(cmd);
                i += 2;
            }
            'L' | 'l' | 'O' | 'o' | 'K' | 'k' => {
                i += 2;
            }
            'U' | 'u' if i + 6 < chars.len() && chars[i + 2] == '+' => {
                let hex: String = chars[i + 3..i + 7].iter().collect();
                if let Ok(cp) = u32::from_str_radix(&hex, 16) {
                    if let Some(decoded) = char::from_u32(cp) {
                        out.push(decoded);
                    }
                }
                i += 7;
            }
            'S' | 's' => {
                // Stacked text `\Snum/den;` or `\Snum^den;` -> `num/den`
                i += 2;
                while i < chars.len() && chars[i] != ';' {
                    let sc = chars[i];
                    out.push(if sc == '^' || sc == '#' { '/' } else { sc });
                    i += 1;
                }
                if i < chars.len() && chars[i] == ';' {
                    i += 1;
                }
            }
            'A' | 'C' | 'c' | 'F' | 'f' | 'H' | 'Q' | 'T' | 'W' | 'p' => {
                i += 2;
                while i < chars.len() && chars[i] != ';' {
                    i += 1;
                }
                if i < chars.len() && chars[i] == ';' {
                    i += 1;
                }
            }
            _ => {
                out.push(cmd);
                i += 2;
            }
        }
    }
    out
}

pub(crate) fn set_sheet_color(sheet: &mut ShapeSheet, cell_name: &str, color: RgbaColor) {
    sheet.set_formula(
        cell_name,
        format!("=RGB({},{},{})", color.r, color.g, color.b),
        Some(Unit::Color),
    );
    if let Some(cell) = sheet.get_cell_mut(cell_name) {
        cell.value = CellValue::Color(color);
    }
}

pub(crate) fn init_2d_cad_sheet(min_x: f64, min_y: f64, w: f64, h: f64) -> ShapeSheet {
    let pin_x = min_x + w * 0.5;
    let pin_y = min_y + h * 0.5;
    let mut ss = ShapeSheet::new_2d(pin_x, pin_y, w, h);
    if let Some(c) = ss.get_cell_mut("LocPinX") {
        c.value = CellValue::inches(w * 0.5);
    }
    if let Some(c) = ss.get_cell_mut("LocPinY") {
        c.value = CellValue::inches(h * 0.5);
    }
    ss
}

pub(crate) fn apply_cad_shape_metadata(
    shape: &mut VisioShape,
    entity_type: &str,
    layer_ix: u32,
    layer_locked: bool,
    stroke: RgbaColor,
    fill: Option<RgbaColor>,
) {
    shape.layer_membership = vec![layer_ix];
    shape.sheet.set_user_cell(
        "CadEntity",
        "1",
        "Imported CAD entity marker",
    );
    shape.sheet.set_user_cell(
        "CadEntityType",
        format!("\"{entity_type}\""),
        "Source CAD entity type",
    );
    shape.sheet.set_user_cell(
        "CadConverted",
        "FALSE",
        "Converted to standard editable Visio shape",
    );
    set_sheet_color(&mut shape.sheet, "LineColor", stroke);
    if let Some(fc) = fill {
        shape.sheet.set_formula("FillPattern", "1", Some(Unit::Number));
        set_sheet_color(&mut shape.sheet, "FillForegnd", fc);
    } else {
        shape.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    }
    set_shape_lock_flags(&mut shape.sheet, layer_locked);
}

pub(crate) fn set_shape_lock_flags(sheet: &mut ShapeSheet, locked: bool) {
    let val = if locked { "TRUE" } else { "FALSE" };
    for cell in [
        "LockWidth",
        "LockHeight",
        "LockMoveX",
        "LockMoveY",
        "LockRotate",
        "LockDelete",
    ] {
        sheet.set_formula(cell, val, Some(Unit::Boolean));
    }
}

/// Ensure or update a `LayerDef` on `VisioPage` from CAD layer attributes.
pub(crate) fn upsert_cad_layer(
    page: &mut VisioPage,
    name: &str,
    aci_raw: i16,
    truecolor: Option<u32>,
    flags: i32,
    plot: bool,
) -> u32 {
    let is_off = aci_raw < 0;
    let is_frozen = (flags & 1) != 0;
    let is_locked = (flags & 4) != 0;
    let visible = !is_off && !is_frozen;
    let print = plot && !is_frozen;
    let rgba = if let Some(tc) = truecolor {
        RgbaColor {
            r: ((tc >> 16) & 0xFF) as u8,
            g: ((tc >> 8) & 0xFF) as u8,
            b: (tc & 0xFF) as u8,
            a: 255,
        }
    } else {
        aci_to_rgba(aci_raw.unsigned_abs())
    };

    if let Some(existing) = page
        .layers
        .iter_mut()
        .find(|l| l.name.eq_ignore_ascii_case(name) || l.name_u.eq_ignore_ascii_case(name))
    {
        existing.visible = visible;
        existing.print = print;
        existing.lock = is_locked;
        existing.color_override = Some(rgba);
        return existing.ix;
    }

    let next_ix = page.layers.iter().map(|l| l.ix).max().map_or(0, |m| m + 1);
    let mut layer = LayerDef::new(next_ix, name);
    layer.visible = visible;
    layer.print = print;
    layer.lock = is_locked;
    layer.color_override = Some(rgba);
    page.layers.push(layer);
    next_ix
}

pub(crate) fn resolve_or_create_layer(page: &mut VisioPage, name: &str) -> (u32, RgbaColor, bool) {
    let layer_name = if name.trim().is_empty() { "0" } else { name.trim() };
    if let Some(l) = page
        .layers
        .iter()
        .find(|l| l.name.eq_ignore_ascii_case(layer_name) || l.name_u.eq_ignore_ascii_case(layer_name))
    {
        return (
            l.ix,
            l.color_override.unwrap_or(RgbaColor::BLACK),
            l.lock,
        );
    }
    let ix = upsert_cad_layer(page, layer_name, 7, None, 0, true);
    (ix, RgbaColor::BLACK, false)
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Affine2D {
    pub base_x: f64,
    pub base_y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rot_rad: f64,
    pub tx: f64,
    pub ty: f64,
}

impl Default for Affine2D {
    fn default() -> Self {
        Self {
            base_x: 0.0,
            base_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rot_rad: 0.0,
            tx: 0.0,
            ty: 0.0,
        }
    }
}

impl Affine2D {
    pub fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        let sx = (x - self.base_x) * self.scale_x;
        let sy = (y - self.base_y) * self.scale_y;
        let (s, c) = self.rot_rad.sin_cos();
        [self.tx + sx * c - sy * s, self.ty + sx * s + sy * c]
    }

    pub fn uniform_scale(&self) -> f64 {
        ((self.scale_x.powi(2) + self.scale_y.powi(2)) * 0.5).sqrt()
    }

    pub fn orientation_sign(&self) -> f64 {
        if self.scale_x * self.scale_y < 0.0 {
            -1.0
        } else {
            1.0
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PolyVertex {
    pub x: f64,
    pub y: f64,
    pub bulge: f64,
}

pub(crate) fn build_line_shape(
    id: u64,
    p1: [f64; 2],
    p2: [f64; 2],
    layer_ix: u32,
    layer_locked: bool,
    stroke: RgbaColor,
) -> VisioShape {
    let min_x = p1[0].min(p2[0]);
    let min_y = p1[1].min(p2[1]);
    let w = (p1[0] - p2[0]).abs();
    let h = (p1[1] - p2[1]).abs();
    let mut sheet = init_2d_cad_sheet(min_x, min_y, w, h);
    let mut geom = GeometrySection::new(1);
    geom.no_fill = "TRUE".into();
    geom.rows = vec![
        GeometryRow::move_to(
            1,
            format!("{} in", format_clean_f64(p1[0] - min_x)),
            format!("{} in", format_clean_f64(p1[1] - min_y)),
        ),
        GeometryRow::line_to(
            2,
            format!("{} in", format_clean_f64(p2[0] - min_x)),
            format!("{} in", format_clean_f64(p2[1] - min_y)),
        ),
    ];
    sheet.add_geometry(geom);
    let name = format!("CAD.Line.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
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
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, "LINE", layer_ix, layer_locked, stroke, None);
    shape
}

pub(crate) fn build_polyline_shape(
    id: u64,
    entity_tag: &str,
    verts: &[PolyVertex],
    closed: bool,
    layer_ix: u32,
    layer_locked: bool,
    stroke: RgbaColor,
    fill: Option<RgbaColor>,
) -> Option<VisioShape> {
    if verts.is_empty() {
        return None;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let seg_count = if closed && verts.len() >= 2 {
        verts.len()
    } else {
        verts.len().saturating_sub(1)
    };

    for (i, v) in verts.iter().enumerate() {
        min_x = min_x.min(v.x);
        min_y = min_y.min(v.y);
        max_x = max_x.max(v.x);
        max_y = max_y.max(v.y);
        if i < seg_count && v.bulge.abs() > 1e-9 {
            let next = verts[(i + 1) % verts.len()];
            let chord = (next.x - v.x).hypot(next.y - v.y);
            let bow = (v.bulge * chord * 0.5).abs();
            min_x = min_x.min(v.x.min(next.x) - bow);
            min_y = min_y.min(v.y.min(next.y) - bow);
            max_x = max_x.max(v.x.max(next.x) + bow);
            max_y = max_y.max(v.y.max(next.y) + bow);
        }
    }
    if !min_x.is_finite() {
        return None;
    }
    let w = (max_x - min_x).max(0.0);
    let h = (max_y - min_y).max(0.0);
    let mut sheet = init_2d_cad_sheet(min_x, min_y, w, h);
    let mut geom = GeometrySection::new(1);
    geom.no_fill = if fill.is_some() {
        "FALSE".into()
    } else {
        "TRUE".into()
    };

    let mut rows = Vec::with_capacity(seg_count + 1);
    rows.push(GeometryRow::move_to(
        1,
        format!("{} in", format_clean_f64(verts[0].x - min_x)),
        format!("{} in", format_clean_f64(verts[0].y - min_y)),
    ));
    let mut ix = 2u32;
    for i in 0..seg_count {
        let cur = verts[i];
        let next = verts[(i + 1) % verts.len()];
        let lx = format!("{} in", format_clean_f64(next.x - min_x));
        let ly = format!("{} in", format_clean_f64(next.y - min_y));
        let chord = (next.x - cur.x).hypot(next.y - cur.y);
        if cur.bulge.abs() > 1e-9 && chord > 1e-12 {
            let bow = cur.bulge * chord * 0.5;
            rows.push(GeometryRow::arc_to(
                ix,
                lx,
                ly,
                format!("{} in", format_clean_f64(bow)),
            ));
        } else {
            rows.push(GeometryRow::line_to(ix, lx, ly));
        }
        ix += 1;
    }
    geom.rows = rows;
    sheet.add_geometry(geom);

    let name = format!("CAD.{entity_tag}.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
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
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, entity_tag, layer_ix, layer_locked, stroke, fill);
    Some(shape)
}

pub(crate) fn build_ellipse_or_circle_shape(
    id: u64,
    entity_tag: &str,
    cx: f64,
    cy: f64,
    rx: f64,
    ry: f64,
    angle_rad: f64,
    layer_ix: u32,
    layer_locked: bool,
    stroke: RgbaColor,
) -> VisioShape {
    let rx = rx.abs().max(1e-6);
    let ry = ry.abs().max(1e-6);
    let w = rx * 2.0;
    let h = ry * 2.0;
    let mut sheet = ShapeSheet::new_2d(cx, cy, w, h);
    if let Some(c) = sheet.get_cell_mut("LocPinX") {
        c.value = CellValue::inches(rx);
    }
    if let Some(c) = sheet.get_cell_mut("LocPinY") {
        c.value = CellValue::inches(ry);
    }
    sheet.set_formula(
        "Angle",
        format!("{} rad", format_clean_f64(angle_rad)),
        Some(Unit::Radian),
    );
    let mut geom = GeometrySection::new(1);
    geom.no_fill = "TRUE".into();
    geom.rows = vec![GeometryRow {
        ix: 1,
        kind: GeometryRowKind::Ellipse,
        x: format!("{} in", format_clean_f64(rx)),
        y: format!("{} in", format_clean_f64(ry)),
        a: Some(format!("{} in", format_clean_f64(w))),
        b: Some(format!("{} in", format_clean_f64(ry))),
        c: Some(format!("{} in", format_clean_f64(rx))),
        d: Some(format!("{} in", format_clean_f64(h))),
        e: None,
        deleted: false,
    }];
    sheet.add_geometry(geom);
    let name = format!("CAD.{entity_tag}.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
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
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, entity_tag, layer_ix, layer_locked, stroke, None);
    shape
}

pub(crate) fn build_arc_shape(
    id: u64,
    cx: f64,
    cy: f64,
    radius: f64,
    start_rad: f64,
    end_rad: f64,
    ccw: bool,
    layer_ix: u32,
    layer_locked: bool,
    stroke: RgbaColor,
) -> VisioShape {
    let r = radius.abs().max(1e-6);
    let mut sweep = if ccw {
        (end_rad - start_rad).rem_euclid(TAU)
    } else {
        (start_rad - end_rad).rem_euclid(TAU)
    };
    if sweep.abs() < 1e-9 {
        sweep = TAU;
    }
    if sweep >= TAU - 1e-5 {
        return build_ellipse_or_circle_shape(
            id,
            "CIRCLE",
            cx,
            cy,
            r,
            r,
            0.0,
            layer_ix,
            layer_locked,
            stroke,
        );
    }
    let p1 = [cx + r * start_rad.cos(), cy + r * start_rad.sin()];
    let p2 = [cx + r * end_rad.cos(), cy + r * end_rad.sin()];
    let sign = if ccw { 1.0 } else { -1.0 };
    let bulge = sign * (sweep * 0.25).tan();
    let verts = [
        PolyVertex {
            x: p1[0],
            y: p1[1],
            bulge,
        },
        PolyVertex {
            x: p2[0],
            y: p2[1],
            bulge: 0.0,
        },
    ];
    build_polyline_shape(id, "ARC", &verts, false, layer_ix, layer_locked, stroke, None)
        .unwrap_or_else(|| build_line_shape(id, p1, p2, layer_ix, layer_locked, stroke))
}

pub(crate) fn build_text_shape(
    id: u64,
    entity_tag: &str,
    x: f64,
    y: f64,
    height_in: f64,
    width_hint_in: Option<f64>,
    angle_rad: f64,
    text: String,
    layer_ix: u32,
    layer_locked: bool,
    color: RgbaColor,
) -> VisioShape {
    let h = height_in.abs().max(0.08);
    let line_count = text.lines().count().max(1) as f64;
    let max_chars = text
        .lines()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(4)
        .max(1) as f64;
    let w = width_hint_in
        .filter(|v| *v > 1e-4)
        .unwrap_or((max_chars * h * 0.65).max(h));
    let box_h = (h * line_count * 1.25).max(h);
    let mut sheet = ShapeSheet::new_2d(x, y, w, box_h);
    if let Some(c) = sheet.get_cell_mut("LocPinX") {
        c.value = CellValue::inches(w * 0.5);
    }
    if let Some(c) = sheet.get_cell_mut("LocPinY") {
        c.value = CellValue::inches(box_h * 0.5);
    }
    sheet.set_formula(
        "Angle",
        format!("{} rad", format_clean_f64(angle_rad)),
        Some(Unit::Radian),
    );
    let pt_size = (h * 72.0).max(1.0);
    sheet.set_formula(
        "Char.Size",
        format!("{} pt", format_clean_f64(pt_size)),
        Some(Unit::Point),
    );
    set_sheet_color(&mut sheet, "Char.Color", color);
    sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    let mut geom = GeometrySection::rectangle(1);
    geom.no_show = "TRUE".into();
    geom.no_fill = "TRUE".into();
    geom.no_line = "TRUE".into();
    sheet.add_geometry(geom);

    let name = format!("CAD.{entity_tag}.{id}");
    let mut shape = VisioShape {
        id,
        name: name.clone(),
        name_u: name,
        kind: ShapeKind::Shape2D,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text,
        sheet,
        children: Vec::new(),
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut shape, entity_tag, layer_ix, layer_locked, color, None);
    shape.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    shape
}

/// Reconstruct circle `(cx, cy, radius, start_deg, end_deg)` (CCW AutoCAD convention)
/// from start point `p1`, end point `p2`, and signed Visio `bow` (`sagitta = bulge * chord / 2`).
pub(crate) fn arc_params_from_endpoints_and_bow(
    p1: [f64; 2],
    p2: [f64; 2],
    bow: f64,
) -> Option<([f64; 2], f64, f64, f64)> {
    let dx = p2[0] - p1[0];
    let dy = p2[1] - p1[1];
    let chord = dx.hypot(dy);
    if chord < 1e-9 || bow.abs() < 1e-9 {
        return None;
    }
    let b = (2.0 * bow) / chord;
    if b.abs() < 1e-9 {
        return None;
    }
    let ux = dx / chord;
    let uy = dy / chord;
    let nlx = -uy;
    let nly = ux;
    let mx = (p1[0] + p2[0]) * 0.5;
    let my = (p1[1] + p2[1]) * 0.5;
    let dist_to_center = chord * (1.0 - b * b) / (4.0 * b);
    let cx = mx + dist_to_center * nlx;
    let cy = my + dist_to_center * nly;
    let radius = chord * (1.0 + b * b) / (4.0 * b.abs());
    let a1 = (p1[1] - cy).atan2(p1[0] - cx).to_degrees().rem_euclid(360.0);
    let a2 = (p2[1] - cy).atan2(p2[0] - cx).to_degrees().rem_euclid(360.0);
    let (start_deg, end_deg) = if b > 0.0 { (a1, a2) } else { (a2, a1) };
    Some(([cx, cy], radius, start_deg, end_deg))
}

// --- Intermediate Parsed DXF AST ---

#[derive(Debug, Clone)]
struct RawDxfLayer {
    name: String,
    aci: i16,
    truecolor: Option<u32>,
    flags: i32,
    plot: bool,
}

#[derive(Debug, Clone)]
struct RawDxfBlock {
    base_x: f64,
    base_y: f64,
    entities: Vec<RawDxfEntity>,
}

#[derive(Debug, Clone)]
enum RawDxfEntityKind {
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    },
    Polyline {
        verts: Vec<PolyVertex>,
        closed: bool,
    },
    Circle {
        cx: f64,
        cy: f64,
        r: f64,
    },
    Arc {
        cx: f64,
        cy: f64,
        r: f64,
        start_deg: f64,
        end_deg: f64,
        extrusion_z: f64,
    },
    Ellipse {
        cx: f64,
        cy: f64,
        mx: f64,
        my: f64,
        ratio: f64,
        start_param: f64,
        end_param: f64,
    },
    Spline {
        degree: usize,
        closed: bool,
        knots: Vec<f64>,
        ctrl_pts: Vec<[f64; 2]>,
        weights: Vec<f64>,
        fit_pts: Vec<[f64; 2]>,
    },
    Text {
        x: f64,
        y: f64,
        height: f64,
        rot_deg: f64,
        text: String,
        is_mtext: bool,
        width_hint: Option<f64>,
    },
    Solid {
        pts: [[f64; 2]; 4],
    },
    Hatch {
        solid_fill: bool,
        loops: Vec<(Vec<PolyVertex>, bool)>,
    },
    Insert {
        block_name: String,
        x: f64,
        y: f64,
        sx: f64,
        sy: f64,
        rot_deg: f64,
        cols: u32,
        rows: u32,
        col_spacing: f64,
        row_spacing: f64,
    },
    Dimension {
        block_name: Option<String>,
        def_pt: [f64; 2],
        text_mid: [f64; 2],
        ext1: [f64; 2],
        ext2: [f64; 2],
        text_override: String,
    },
}

#[derive(Debug, Clone)]
struct RawDxfEntity {
    layer: String,
    aci: Option<i16>,
    truecolor: Option<u32>,
    kind: RawDxfEntityKind,
}

/// Parse an ASCII or Binary DXF file onto `page`, mapping layers, blocks, units, and CAD entities.
/// Returns the IDs of the top-level shapes created on `page`.
pub fn import_dxf_to_page(page: &mut VisioPage, dxf_bytes: &[u8]) -> Result<Vec<u64>, CadError> {
    let groups = parse_dxf_groups(dxf_bytes)?;

    let mut insunits = 1i32;
    let mut ext_min: Option<[f64; 2]> = None;
    let mut ext_max: Option<[f64; 2]> = None;
    let mut raw_layers: Vec<RawDxfLayer> = Vec::new();
    let mut blocks: BTreeMap<String, RawDxfBlock> = BTreeMap::new();
    let mut model_entities: Vec<RawDxfEntity> = Vec::new();

    let mut idx = 0usize;
    while idx < groups.len() {
        if groups[idx].0 == 0 && groups[idx].1.as_str().eq_ignore_ascii_case("SECTION") {
            idx += 1;
            let mut sec_name = String::new();
            if idx < groups.len() && groups[idx].0 == 2 {
                sec_name = groups[idx].1.as_str().to_ascii_uppercase();
                idx += 1;
            }
            let sec_start = idx;
            while idx < groups.len() {
                if groups[idx].0 == 0 && groups[idx].1.as_str().eq_ignore_ascii_case("ENDSEC") {
                    break;
                }
                idx += 1;
            }
            let slice = &groups[sec_start..idx];
            match sec_name.as_str() {
                "HEADER" => parse_header_section(slice, &mut insunits, &mut ext_min, &mut ext_max),
                "TABLES" => parse_tables_section(slice, &mut raw_layers),
                "BLOCKS" => parse_blocks_section(slice, &mut blocks),
                "ENTITIES" => parse_entities_slice(slice, &mut model_entities),
                _ => {}
            }
            if idx < groups.len() {
                idx += 1; // consume ENDSEC
            }
        } else {
            idx += 1;
        }
    }

    let (unit_enum, to_in) = insunits_to_unit_and_factor(insunits);
    page.page_sheet.set_user_cell(
        "CadInsUnits",
        format!("\"{}\"", unit_enum.vsdx_code()),
        "DXF $INSUNITS",
    );
    if let (Some(mn), Some(mx)) = (ext_min, ext_max) {
        let min_x_in = mn[0] * to_in;
        let min_y_in = mn[1] * to_in;
        let max_x_in = mx[0] * to_in;
        let max_y_in = mx[1] * to_in;
        page.page_sheet
            .set_user_cell("CadExtMinX", format!("{} in", format_clean_f64(min_x_in)), "DXF EXTMIN.X");
        page.page_sheet
            .set_user_cell("CadExtMinY", format!("{} in", format_clean_f64(min_y_in)), "DXF EXTMIN.Y");
        page.page_sheet
            .set_user_cell("CadExtMaxX", format!("{} in", format_clean_f64(max_x_in)), "DXF EXTMAX.X");
        page.page_sheet
            .set_user_cell("CadExtMaxY", format!("{} in", format_clean_f64(max_y_in)), "DXF EXTMAX.Y");
        let span_w = max_x_in - min_x_in;
        let span_h = max_y_in - min_y_in;
        if span_w.is_finite() && span_h.is_finite() && span_w > 0.1 && span_h > 0.1 {
            let new_w = page.width_in().max(max_x_in.max(span_w));
            let new_h = page.height_in().max(max_y_in.max(span_h));
            page.page_sheet.set_formula(
                "PageWidth",
                format!("{} in", format_clean_f64(new_w)),
                Some(Unit::Inch),
            );
            page.page_sheet.set_formula(
                "PageHeight",
                format!("{} in", format_clean_f64(new_h)),
                Some(Unit::Inch),
            );
        }
    }

    // Ensure layer "0" and all declared DXF layers exist on `page`.
    if !raw_layers
        .iter()
        .any(|l| l.name.eq_ignore_ascii_case("0"))
    {
        upsert_cad_layer(page, "0", 7, None, 0, true);
    }
    for rl in &raw_layers {
        upsert_cad_layer(page, &rl.name, rl.aci, rl.truecolor, rl.flags, rl.plot);
    }

    let base_tf = Affine2D {
        base_x: 0.0,
        base_y: 0.0,
        scale_x: to_in,
        scale_y: to_in,
        rot_rad: 0.0,
        tx: 0.0,
        ty: 0.0,
    };

    let mut next_id = page.next_shape_id();
    let mut created_ids = Vec::new();
    for ent in &model_entities {
        let shapes = instantiate_entity(page, ent, &blocks, &base_tf, None, 0, &mut next_id);
        for s in shapes {
            created_ids.push(s.id);
            page.shapes.push(s);
        }
    }

    Ok(created_ids)
}

fn parse_header_section(
    slice: &[(i32, DxfValue)],
    insunits: &mut i32,
    ext_min: &mut Option<[f64; 2]>,
    ext_max: &mut Option<[f64; 2]>,
) {
    let mut cur_var = String::new();
    for (code, val) in slice {
        if *code == 9 {
            cur_var = val.as_str().to_ascii_uppercase();
            continue;
        }
        match cur_var.as_str() {
            "$INSUNITS" if *code == 70 => {
                if let Some(n) = val.as_i64() {
                    *insunits = n as i32;
                }
            }
            "$EXTMIN" => {
                let entry = ext_min.get_or_insert([0.0, 0.0]);
                if *code == 10 {
                    if let Some(x) = val.as_f64() {
                        entry[0] = x;
                    }
                } else if *code == 20 {
                    if let Some(y) = val.as_f64() {
                        entry[1] = y;
                    }
                }
            }
            "$EXTMAX" => {
                let entry = ext_max.get_or_insert([0.0, 0.0]);
                if *code == 10 {
                    if let Some(x) = val.as_f64() {
                        entry[0] = x;
                    }
                } else if *code == 20 {
                    if let Some(y) = val.as_f64() {
                        entry[1] = y;
                    }
                }
            }
            _ => {}
        }
    }
}

fn parse_tables_section(slice: &[(i32, DxfValue)], layers: &mut Vec<RawDxfLayer>) {
    let mut i = 0usize;
    while i < slice.len() {
        if slice[i].0 == 0 && slice[i].1.as_str().eq_ignore_ascii_case("LAYER") {
            i += 1;
            let mut name = String::from("0");
            let mut aci = 7i16;
            let mut truecolor = None;
            let mut flags = 0i32;
            let mut plot = true;
            while i < slice.len() && slice[i].0 != 0 {
                match slice[i].0 {
                    2 => name = slice[i].1.as_str(),
                    62 => {
                        if let Some(c) = slice[i].1.as_i64() {
                            aci = c as i16;
                        }
                    }
                    420 => {
                        if let Some(tc) = slice[i].1.as_i64() {
                            truecolor = Some(tc as u32);
                        }
                    }
                    70 => {
                        if let Some(f) = slice[i].1.as_i64() {
                            flags = f as i32;
                        }
                    }
                    290 => {
                        if let Some(p) = slice[i].1.as_i64() {
                            plot = p != 0;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            layers.push(RawDxfLayer {
                name,
                aci,
                truecolor,
                flags,
                plot,
            });
        } else {
            i += 1;
        }
    }
}

fn parse_blocks_section(
    slice: &[(i32, DxfValue)],
    blocks: &mut BTreeMap<String, RawDxfBlock>,
) {
    let mut i = 0usize;
    while i < slice.len() {
        if slice[i].0 == 0 && slice[i].1.as_str().eq_ignore_ascii_case("BLOCK") {
            i += 1;
            let mut name = String::new();
            let mut base_x = 0.0;
            let mut base_y = 0.0;
            while i < slice.len() && slice[i].0 != 0 {
                match slice[i].0 {
                    2 | 3 if name.is_empty() => name = slice[i].1.as_str(),
                    10 => base_x = slice[i].1.as_f64().unwrap_or(0.0),
                    20 => base_y = slice[i].1.as_f64().unwrap_or(0.0),
                    _ => {}
                }
                i += 1;
            }
            let ent_start = i;
            while i < slice.len() {
                if slice[i].0 == 0 && slice[i].1.as_str().eq_ignore_ascii_case("ENDBLK") {
                    break;
                }
                i += 1;
            }
            let mut entities = Vec::new();
            parse_entities_slice(&slice[ent_start..i], &mut entities);
            if !name.is_empty() {
                blocks.insert(
                    name.to_ascii_uppercase(),
                    RawDxfBlock {
                        base_x,
                        base_y,
                        entities,
                    },
                );
            }
            if i < slice.len() {
                i += 1; // consume ENDBLK
            }
        } else {
            i += 1;
        }
    }
}

fn parse_entities_slice(slice: &[(i32, DxfValue)], out: &mut Vec<RawDxfEntity>) {
    let mut i = 0usize;
    while i < slice.len() {
        if slice[i].0 != 0 {
            i += 1;
            continue;
        }
        let ent_type = slice[i].1.as_str().to_ascii_uppercase();
        i += 1;
        let rec_start = i;
        while i < slice.len() && slice[i].0 != 0 {
            i += 1;
        }
        let props = &slice[rec_start..i];

        if ent_type == "POLYLINE" {
            // Classic multi-record `POLYLINE` -> `VERTEX`* -> `SEQEND`
            let (layer, aci, truecolor) = extract_common_entity_meta(props);
            let mut flags = 0i64;
            for (c, v) in props {
                if *c == 70 {
                    flags = v.as_i64().unwrap_or(0);
                }
            }
            let closed = (flags & 1) != 0;
            let mut verts = Vec::new();
            while i < slice.len() {
                if slice[i].0 == 0 {
                    let sub_type = slice[i].1.as_str().to_ascii_uppercase();
                    i += 1;
                    let v_start = i;
                    while i < slice.len() && slice[i].0 != 0 {
                        i += 1;
                    }
                    if sub_type == "SEQEND" {
                        break;
                    }
                    if sub_type == "VERTEX" {
                        let mut vx = 0.0;
                        let mut vy = 0.0;
                        let mut bulge = 0.0;
                        let mut vflags = 0i64;
                        for (c, v) in &slice[v_start..i] {
                            match *c {
                                10 => vx = v.as_f64().unwrap_or(0.0),
                                20 => vy = v.as_f64().unwrap_or(0.0),
                                42 => bulge = v.as_f64().unwrap_or(0.0),
                                70 => vflags = v.as_i64().unwrap_or(0),
                                _ => {}
                            }
                        }
                        // Skip spline-control-frame dummy vertices (bit 16)
                        if (vflags & 16) == 0 {
                            verts.push(PolyVertex { x: vx, y: vy, bulge });
                        }
                    }
                } else {
                    i += 1;
                }
            }
            if !verts.is_empty() {
                out.push(RawDxfEntity {
                    layer,
                    aci,
                    truecolor,
                    kind: RawDxfEntityKind::Polyline { verts, closed },
                });
            }
            continue;
        }

        if let Some(ent) = parse_single_entity(&ent_type, props) {
            out.push(ent);
        }
    }
}

fn extract_common_entity_meta(props: &[(i32, DxfValue)]) -> (String, Option<i16>, Option<u32>) {
    let mut layer = String::from("0");
    let mut aci = None;
    let mut truecolor = None;
    for (code, val) in props {
        match *code {
            8 => layer = val.as_str(),
            62 => {
                if let Some(c) = val.as_i64() {
                    if (1..=255).contains(&c) {
                        aci = Some(c as i16);
                    }
                }
            }
            420 => {
                if let Some(tc) = val.as_i64() {
                    truecolor = Some(tc as u32);
                }
            }
            _ => {}
        }
    }
    (layer, aci, truecolor)
}

fn parse_single_entity(ent_type: &str, props: &[(i32, DxfValue)]) -> Option<RawDxfEntity> {
    let (layer, aci, truecolor) = extract_common_entity_meta(props);
    let kind = match ent_type {
        "LINE" => {
            let mut x1 = 0.0;
            let mut y1 = 0.0;
            let mut x2 = 0.0;
            let mut y2 = 0.0;
            for (c, v) in props {
                match *c {
                    10 => x1 = v.as_f64().unwrap_or(0.0),
                    20 => y1 = v.as_f64().unwrap_or(0.0),
                    11 => x2 = v.as_f64().unwrap_or(0.0),
                    21 => y2 = v.as_f64().unwrap_or(0.0),
                    _ => {}
                }
            }
            RawDxfEntityKind::Line { x1, y1, x2, y2 }
        }
        "LWPOLYLINE" => {
            let mut closed = false;
            let mut verts: Vec<PolyVertex> = Vec::new();
            for (c, v) in props {
                match *c {
                    70 => closed = (v.as_i64().unwrap_or(0) & 1) != 0,
                    10 => verts.push(PolyVertex {
                        x: v.as_f64().unwrap_or(0.0),
                        y: 0.0,
                        bulge: 0.0,
                    }),
                    20 => {
                        if let Some(last) = verts.last_mut() {
                            last.y = v.as_f64().unwrap_or(0.0);
                        }
                    }
                    42 => {
                        if let Some(last) = verts.last_mut() {
                            last.bulge = v.as_f64().unwrap_or(0.0);
                        }
                    }
                    _ => {}
                }
            }
            if verts.is_empty() {
                return None;
            }
            RawDxfEntityKind::Polyline { verts, closed }
        }
        "CIRCLE" => {
            let mut cx = 0.0;
            let mut cy = 0.0;
            let mut r = 1.0;
            for (c, v) in props {
                match *c {
                    10 => cx = v.as_f64().unwrap_or(0.0),
                    20 => cy = v.as_f64().unwrap_or(0.0),
                    40 => r = v.as_f64().unwrap_or(1.0),
                    _ => {}
                }
            }
            RawDxfEntityKind::Circle { cx, cy, r }
        }
        "ARC" => {
            let mut cx = 0.0;
            let mut cy = 0.0;
            let mut r = 1.0;
            let mut start_deg = 0.0;
            let mut end_deg = 180.0;
            let mut extrusion_z = 1.0;
            for (c, v) in props {
                match *c {
                    10 => cx = v.as_f64().unwrap_or(0.0),
                    20 => cy = v.as_f64().unwrap_or(0.0),
                    40 => r = v.as_f64().unwrap_or(1.0),
                    50 => start_deg = v.as_f64().unwrap_or(0.0),
                    51 => end_deg = v.as_f64().unwrap_or(180.0),
                    230 => extrusion_z = v.as_f64().unwrap_or(1.0),
                    _ => {}
                }
            }
            RawDxfEntityKind::Arc {
                cx,
                cy,
                r,
                start_deg,
                end_deg,
                extrusion_z,
            }
        }
        "ELLIPSE" => {
            let mut cx = 0.0;
            let mut cy = 0.0;
            let mut mx = 1.0;
            let mut my = 0.0;
            let mut ratio = 0.5;
            let mut start_param = 0.0;
            let mut end_param = TAU;
            for (c, v) in props {
                match *c {
                    10 => cx = v.as_f64().unwrap_or(0.0),
                    20 => cy = v.as_f64().unwrap_or(0.0),
                    11 => mx = v.as_f64().unwrap_or(1.0),
                    21 => my = v.as_f64().unwrap_or(0.0),
                    40 => ratio = v.as_f64().unwrap_or(0.5),
                    41 => start_param = v.as_f64().unwrap_or(0.0),
                    42 => end_param = v.as_f64().unwrap_or(TAU),
                    _ => {}
                }
            }
            RawDxfEntityKind::Ellipse {
                cx,
                cy,
                mx,
                my,
                ratio,
                start_param,
                end_param,
            }
        }
        "SPLINE" => {
            let mut degree = 3usize;
            let mut closed = false;
            let mut knots = Vec::new();
            let mut ctrl_pts: Vec<[f64; 2]> = Vec::new();
            let mut weights = Vec::new();
            let mut fit_pts: Vec<[f64; 2]> = Vec::new();
            for (c, v) in props {
                match *c {
                    70 => closed = (v.as_i64().unwrap_or(0) & 1) != 0,
                    71 => degree = v.as_i64().unwrap_or(3).clamp(1, 10) as usize,
                    40 => knots.push(v.as_f64().unwrap_or(0.0)),
                    41 => weights.push(v.as_f64().unwrap_or(1.0)),
                    10 => ctrl_pts.push([v.as_f64().unwrap_or(0.0), 0.0]),
                    20 => {
                        if let Some(last) = ctrl_pts.last_mut() {
                            last[1] = v.as_f64().unwrap_or(0.0);
                        }
                    }
                    11 => fit_pts.push([v.as_f64().unwrap_or(0.0), 0.0]),
                    21 => {
                        if let Some(last) = fit_pts.last_mut() {
                            last[1] = v.as_f64().unwrap_or(0.0);
                        }
                    }
                    _ => {}
                }
            }
            if ctrl_pts.is_empty() && fit_pts.is_empty() {
                return None;
            }
            RawDxfEntityKind::Spline {
                degree,
                closed,
                knots,
                ctrl_pts,
                weights,
                fit_pts,
            }
        }
        "TEXT" | "ATTRIB" | "ATTDEF" => {
            let mut x = 0.0;
            let mut y = 0.0;
            let mut ax = None;
            let mut ay = None;
            let mut halign = 0i64;
            let mut valign = 0i64;
            let mut height = 0.2;
            let mut rot_deg = 0.0;
            let mut raw_text = String::new();
            for (c, v) in props {
                match *c {
                    10 => x = v.as_f64().unwrap_or(0.0),
                    20 => y = v.as_f64().unwrap_or(0.0),
                    11 => ax = v.as_f64(),
                    21 => ay = v.as_f64(),
                    40 => height = v.as_f64().unwrap_or(0.2),
                    50 => rot_deg = v.as_f64().unwrap_or(0.0),
                    72 => halign = v.as_i64().unwrap_or(0),
                    73 => valign = v.as_i64().unwrap_or(0),
                    1 => raw_text = v.as_str(),
                    _ => {}
                }
            }
            if (halign != 0 || valign != 0) && ax.is_some() && ay.is_some() {
                x = ax.unwrap_or(x);
                y = ay.unwrap_or(y);
            }
            RawDxfEntityKind::Text {
                x,
                y,
                height,
                rot_deg,
                text: strip_mtext_formatting(&raw_text),
                is_mtext: false,
                width_hint: None,
            }
        }
        "MTEXT" => {
            let mut x = 0.0;
            let mut y = 0.0;
            let mut height = 0.2;
            let mut width_hint = None;
            let mut rot_deg = 0.0;
            let mut dir_vec: Option<[f64; 2]> = None;
            let mut raw = String::new();
            for (c, v) in props {
                match *c {
                    10 => x = v.as_f64().unwrap_or(0.0),
                    20 => y = v.as_f64().unwrap_or(0.0),
                    40 => height = v.as_f64().unwrap_or(0.2),
                    41 => width_hint = v.as_f64(),
                    50 => rot_deg = v.as_f64().unwrap_or(0.0),
                    11 => {
                        let d = dir_vec.get_or_insert([1.0, 0.0]);
                        d[0] = v.as_f64().unwrap_or(1.0);
                    }
                    21 => {
                        let d = dir_vec.get_or_insert([1.0, 0.0]);
                        d[1] = v.as_f64().unwrap_or(0.0);
                    }
                    3 | 1 => raw.push_str(&v.as_str()),
                    _ => {}
                }
            }
            if let Some([dx, dy]) = dir_vec {
                if dx.hypot(dy) > 1e-9 {
                    rot_deg = dy.atan2(dx).to_degrees();
                }
            }
            RawDxfEntityKind::Text {
                x,
                y,
                height,
                rot_deg,
                text: strip_mtext_formatting(&raw),
                is_mtext: true,
                width_hint,
            }
        }
        "SOLID" | "TRACE" | "3DFACE" => {
            let mut p0 = [0.0, 0.0];
            let mut p1 = [0.0, 0.0];
            let mut p2 = [0.0, 0.0];
            let mut p3: Option<[f64; 2]> = None;
            for (c, v) in props {
                match *c {
                    10 => p0[0] = v.as_f64().unwrap_or(0.0),
                    20 => p0[1] = v.as_f64().unwrap_or(0.0),
                    11 => p1[0] = v.as_f64().unwrap_or(0.0),
                    21 => p1[1] = v.as_f64().unwrap_or(0.0),
                    12 => p2[0] = v.as_f64().unwrap_or(0.0),
                    22 => p2[1] = v.as_f64().unwrap_or(0.0),
                    13 => p3.get_or_insert(p2)[0] = v.as_f64().unwrap_or(0.0),
                    23 => p3.get_or_insert(p2)[1] = v.as_f64().unwrap_or(0.0),
                    _ => {}
                }
            }
            let p3 = p3.unwrap_or(p2);
            // AutoCAD SOLID orders corners 0, 1, 3, 2 around perimeter (3DFACE uses 0, 1, 2, 3).
            let pts = if ent_type == "3DFACE" {
                [p0, p1, p2, p3]
            } else {
                [p0, p1, p3, p2]
            };
            RawDxfEntityKind::Solid { pts }
        }
        "HATCH" => parse_hatch_entity(props)?,
        "INSERT" => {
            let mut block_name = String::new();
            let mut x = 0.0;
            let mut y = 0.0;
            let mut sx = 1.0;
            let mut sy = 1.0;
            let mut rot_deg = 0.0;
            let mut cols = 1u32;
            let mut rows = 1u32;
            let mut col_spacing = 0.0;
            let mut row_spacing = 0.0;
            for (c, v) in props {
                match *c {
                    2 => block_name = v.as_str(),
                    10 => x = v.as_f64().unwrap_or(0.0),
                    20 => y = v.as_f64().unwrap_or(0.0),
                    41 => sx = v.as_f64().unwrap_or(1.0),
                    42 => sy = v.as_f64().unwrap_or(1.0),
                    50 => rot_deg = v.as_f64().unwrap_or(0.0),
                    70 => cols = v.as_i64().unwrap_or(1).max(1) as u32,
                    71 => rows = v.as_i64().unwrap_or(1).max(1) as u32,
                    44 => col_spacing = v.as_f64().unwrap_or(0.0),
                    45 => row_spacing = v.as_f64().unwrap_or(0.0),
                    _ => {}
                }
            }
            if block_name.is_empty() {
                return None;
            }
            RawDxfEntityKind::Insert {
                block_name,
                x,
                y,
                sx,
                sy,
                rot_deg,
                cols,
                rows,
                col_spacing,
                row_spacing,
            }
        }
        "DIMENSION" => {
            let mut block_name = None;
            let mut def_pt = [0.0, 0.0];
            let mut text_mid = [0.0, 0.0];
            let mut ext1 = [0.0, 0.0];
            let mut ext2 = [0.0, 0.0];
            let mut text_override = String::new();
            for (c, v) in props {
                match *c {
                    2 => {
                        let s = v.as_str();
                        if !s.is_empty() {
                            block_name = Some(s);
                        }
                    }
                    10 => def_pt[0] = v.as_f64().unwrap_or(0.0),
                    20 => def_pt[1] = v.as_f64().unwrap_or(0.0),
                    11 => text_mid[0] = v.as_f64().unwrap_or(0.0),
                    21 => text_mid[1] = v.as_f64().unwrap_or(0.0),
                    13 => ext1[0] = v.as_f64().unwrap_or(0.0),
                    23 => ext1[1] = v.as_f64().unwrap_or(0.0),
                    14 => ext2[0] = v.as_f64().unwrap_or(0.0),
                    24 => ext2[1] = v.as_f64().unwrap_or(0.0),
                    1 => text_override = v.as_str(),
                    _ => {}
                }
            }
            RawDxfEntityKind::Dimension {
                block_name,
                def_pt,
                text_mid,
                ext1,
                ext2,
                text_override,
            }
        }
        _ => return None,
    };
    Some(RawDxfEntity {
        layer,
        aci,
        truecolor,
        kind,
    })
}

fn parse_hatch_entity(props: &[(i32, DxfValue)]) -> Option<RawDxfEntityKind> {
    let mut solid_fill = false;
    let mut loops: Vec<(Vec<PolyVertex>, bool)> = Vec::new();
    let mut i = 0usize;
    while i < props.len() {
        let (code, val) = &props[i];
        if *code == 70 {
            solid_fill = val.as_i64().unwrap_or(0) != 0;
            i += 1;
            continue;
        }
        if *code == 92 {
            // Start of a HATCH boundary path
            let path_flags = val.as_i64().unwrap_or(0);
            let is_polyline_path = (path_flags & 2) != 0;
            i += 1;
            if is_polyline_path {
                let mut closed = true;
                let mut num_verts = usize::MAX;
                let mut verts: Vec<PolyVertex> = Vec::new();
                while i < props.len() {
                    let (c, v) = &props[i];
                    if *c == 92 || *c == 75 || *c == 76 {
                        break;
                    }
                    match *c {
                        73 => closed = v.as_i64().unwrap_or(1) != 0,
                        93 => num_verts = v.as_i64().unwrap_or(0).max(0) as usize,
                        10 => {
                            if verts.len() >= num_verts {
                                break;
                            }
                            verts.push(PolyVertex {
                                x: v.as_f64().unwrap_or(0.0),
                                y: 0.0,
                                bulge: 0.0,
                            });
                        }
                        20 => {
                            if let Some(last) = verts.last_mut() {
                                last.y = v.as_f64().unwrap_or(0.0);
                            }
                        }
                        42 => {
                            if let Some(last) = verts.last_mut() {
                                last.bulge = v.as_f64().unwrap_or(0.0);
                            }
                        }
                        97 => {
                            // Source boundary object handles count — skip past them
                            i += 1;
                            while i < props.len() && props[i].0 == 330 {
                                i += 1;
                            }
                            break;
                        }
                        _ => {}
                    }
                    i += 1;
                }
                if !verts.is_empty() {
                    loops.push((verts, closed));
                }
            } else {
                // Edge-type HATCH boundary path (`93` edge count, `72` edge type: 1=Line, 2=Arc)
                let mut num_edges = 0usize;
                if i < props.len() && props[i].0 == 93 {
                    num_edges = props[i].1.as_i64().unwrap_or(0).max(0) as usize;
                    i += 1;
                }
                let mut verts: Vec<PolyVertex> = Vec::new();
                for _edge in 0..num_edges {
                    while i < props.len() && props[i].0 != 72 && props[i].0 != 92 && props[i].0 != 97
                    {
                        i += 1;
                    }
                    if i >= props.len() || props[i].0 != 72 {
                        break;
                    }
                    let edge_type = props[i].1.as_i64().unwrap_or(1);
                    i += 1;
                    match edge_type {
                        1 => {
                            let mut x1 = 0.0;
                            let mut y1 = 0.0;
                            let mut x2 = 0.0;
                            let mut y2 = 0.0;
                            while i < props.len()
                                && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76)
                            {
                                match props[i].0 {
                                    10 => x1 = props[i].1.as_f64().unwrap_or(0.0),
                                    20 => y1 = props[i].1.as_f64().unwrap_or(0.0),
                                    11 => x2 = props[i].1.as_f64().unwrap_or(0.0),
                                    21 => y2 = props[i].1.as_f64().unwrap_or(0.0),
                                    _ => {}
                                }
                                i += 1;
                            }
                            if verts.is_empty() {
                                verts.push(PolyVertex { x: x1, y: y1, bulge: 0.0 });
                            }
                            verts.push(PolyVertex { x: x2, y: y2, bulge: 0.0 });
                        }
                        2 => {
                            let mut cx = 0.0;
                            let mut cy = 0.0;
                            let mut r = 1.0;
                            let mut sa = 0.0f64;
                            let mut ea = 180.0f64;
                            let mut ccw = true;
                            while i < props.len()
                                && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76)
                            {
                                match props[i].0 {
                                    10 => cx = props[i].1.as_f64().unwrap_or(0.0),
                                    20 => cy = props[i].1.as_f64().unwrap_or(0.0),
                                    40 => r = props[i].1.as_f64().unwrap_or(1.0),
                                    50 => sa = props[i].1.as_f64().unwrap_or(0.0),
                                    51 => ea = props[i].1.as_f64().unwrap_or(180.0),
                                    73 => ccw = props[i].1.as_i64().unwrap_or(1) != 0,
                                    _ => {}
                                }
                                i += 1;
                            }
                            let sr = sa.to_radians();
                            let er = ea.to_radians();
                            let sweep = if ccw {
                                (er - sr).rem_euclid(TAU)
                            } else {
                                (sr - er).rem_euclid(TAU)
                            };
                            let bulge = (if ccw { 1.0 } else { -1.0 }) * (sweep * 0.25).tan();
                            let p1 = [cx + r * sr.cos(), cy + r * sr.sin()];
                            let p2 = [cx + r * er.cos(), cy + r * er.sin()];
                            if verts.is_empty() {
                                verts.push(PolyVertex {
                                    x: p1[0],
                                    y: p1[1],
                                    bulge,
                                });
                            } else if let Some(last) = verts.last_mut() {
                                last.bulge = bulge;
                            }
                            verts.push(PolyVertex {
                                x: p2[0],
                                y: p2[1],
                                bulge: 0.0,
                            });
                        }
                        _ => {
                            while i < props.len()
                                && !matches!(props[i].0, 72 | 92 | 97 | 75 | 76)
                            {
                                i += 1;
                            }
                        }
                    }
                }
                if verts.len() >= 2 {
                    let first = verts[0];
                    let last = verts[verts.len() - 1];
                    if (first.x - last.x).hypot(first.y - last.y) < 1e-6 && last.bulge.abs() < 1e-9
                    {
                        verts.pop();
                    }
                }
                if !verts.is_empty() {
                    loops.push((verts, true));
                }
            }
            continue;
        }
        i += 1;
    }
    if loops.is_empty() {
        return None;
    }
    Some(RawDxfEntityKind::Hatch { solid_fill, loops })
}

/// Evaluate rational B-Spline / NURBS curve via De Boor / Cox-de Boor basis functions,
/// or Catmull-Rom interpolation if only fit points are supplied.
fn evaluate_spline_points(
    degree: usize,
    closed: bool,
    knots: &[f64],
    ctrl_pts: &[[f64; 2]],
    weights: &[f64],
    fit_pts: &[[f64; 2]],
) -> Vec<[f64; 2]> {
    if ctrl_pts.len() >= 2 {
        let n = ctrl_pts.len();
        let p = degree.min(n - 1).max(1);
        let needed_knots = n + p + 1;
        let kv: Vec<f64> = if knots.len() >= needed_knots {
            knots[..needed_knots].to_vec()
        } else {
            // Synthesize clamped uniform knot vector
            let mut synth = Vec::with_capacity(needed_knots);
            let interior = needed_knots - 2 * (p + 1);
            for _ in 0..=p {
                synth.push(0.0);
            }
            for j in 1..=interior {
                synth.push(j as f64 / (interior + 1) as f64);
            }
            for _ in 0..=p {
                synth.push(1.0);
            }
            synth
        };
        let u_min = kv[p];
        let u_max = kv[n];
        if (u_max - u_min).abs() < 1e-12 {
            return ctrl_pts.to_vec();
        }
        let samples = (n * 12).clamp(24, 96);
        let mut pts = Vec::with_capacity(samples + 1);
        for s in 0..=samples {
            let u = if s == samples {
                u_max - 1e-11 * (u_max - u_min)
            } else {
                u_min + (u_max - u_min) * (s as f64 / samples as f64)
            };
            let mut wx = 0.0;
            let mut wy = 0.0;
            let mut w_sum = 0.0;
            for (idx, cp) in ctrl_pts.iter().enumerate() {
                let basis = cox_de_boor(idx, p, u, &kv);
                if basis > 0.0 {
                    let w = weights.get(idx).copied().unwrap_or(1.0).max(1e-9);
                    let bw = basis * w;
                    wx += cp[0] * bw;
                    wy += cp[1] * bw;
                    w_sum += bw;
                }
            }
            if w_sum > 1e-12 {
                pts.push([wx / w_sum, wy / w_sum]);
            }
        }
        if !closed {
            if let Some(last_out) = pts.last_mut() {
                *last_out = ctrl_pts[n - 1];
            }
        }
        return pts;
    }

    if fit_pts.len() >= 2 {
        let mut pts = Vec::new();
        let m = fit_pts.len();
        for idx in 0..m.saturating_sub(1) {
            let p0 = if idx == 0 { fit_pts[0] } else { fit_pts[idx - 1] };
            let p1 = fit_pts[idx];
            let p2 = fit_pts[idx + 1];
            let p3 = if idx + 2 < m {
                fit_pts[idx + 2]
            } else {
                fit_pts[m - 1]
            };
            let steps = 12usize;
            for step in 0..steps {
                let t = step as f64 / steps as f64;
                let t2 = t * t;
                let t3 = t2 * t;
                let x = 0.5
                    * ((2.0 * p1[0])
                        + (-p0[0] + p2[0]) * t
                        + (2.0 * p0[0] - 5.0 * p1[0] + 4.0 * p2[0] - p3[0]) * t2
                        + (-p0[0] + 3.0 * p1[0] - 3.0 * p2[0] + p3[0]) * t3);
                let y = 0.5
                    * ((2.0 * p1[1])
                        + (-p0[1] + p2[1]) * t
                        + (2.0 * p0[1] - 5.0 * p1[1] + 4.0 * p2[1] - p3[1]) * t2
                        + (-p0[1] + 3.0 * p1[1] - 3.0 * p2[1] + p3[1]) * t3);
                pts.push([x, y]);
            }
        }
        if let Some(&end_pt) = fit_pts.last() {
            pts.push(end_pt);
        }
        return pts;
    }
    Vec::new()
}

fn cox_de_boor(i: usize, p: usize, u: f64, knots: &[f64]) -> f64 {
    if p == 0 {
        return if u >= knots[i] && u < knots[i + 1] {
            1.0
        } else {
            0.0
        };
    }
    let d1 = knots[i + p] - knots[i];
    let left = if d1.abs() > 1e-12 {
        ((u - knots[i]) / d1) * cox_de_boor(i, p - 1, u, knots)
    } else {
        0.0
    };
    let d2 = knots[i + p + 1] - knots[i + 1];
    let right = if d2.abs() > 1e-12 {
        ((knots[i + p + 1] - u) / d2) * cox_de_boor(i + 1, p - 1, u, knots)
    } else {
        0.0
    };
    left + right
}

fn instantiate_entity(
    page: &mut VisioPage,
    ent: &RawDxfEntity,
    blocks: &BTreeMap<String, RawDxfBlock>,
    tf: &Affine2D,
    inherited_layer: Option<&str>,
    depth: usize,
    next_id: &mut u64,
) -> Vec<VisioShape> {
    if depth > 12 {
        return Vec::new();
    }
    // AutoCAD layer "0" inside a BLOCK inherits the enclosing INSERT's layer.
    let effective_layer = if ent.layer.eq_ignore_ascii_case("0") {
        inherited_layer.unwrap_or(&ent.layer)
    } else {
        ent.layer.as_str()
    };
    let (layer_ix, layer_color, layer_locked) = resolve_or_create_layer(page, effective_layer);
    let entity_color = if let Some(tc) = ent.truecolor {
        RgbaColor {
            r: ((tc >> 16) & 0xFF) as u8,
            g: ((tc >> 8) & 0xFF) as u8,
            b: (tc & 0xFF) as u8,
            a: 255,
        }
    } else if let Some(aci) = ent.aci {
        aci_to_rgba(aci.unsigned_abs())
    } else {
        layer_color
    };

    match &ent.kind {
        RawDxfEntityKind::Line { x1, y1, x2, y2 } => {
            let p1 = tf.apply(*x1, *y1);
            let p2 = tf.apply(*x2, *y2);
            let id = *next_id;
            *next_id += 1;
            vec![build_line_shape(id, p1, p2, layer_ix, layer_locked, entity_color)]
        }
        RawDxfEntityKind::Polyline { verts, closed } => {
            let sign = tf.orientation_sign();
            let transformed: Vec<PolyVertex> = verts
                .iter()
                .map(|v| {
                    let [px, py] = tf.apply(v.x, v.y);
                    PolyVertex {
                        x: px,
                        y: py,
                        bulge: v.bulge * sign,
                    }
                })
                .collect();
            let id = *next_id;
            if let Some(s) = build_polyline_shape(
                id,
                "LWPOLYLINE",
                &transformed,
                *closed,
                layer_ix,
                layer_locked,
                entity_color,
                None,
            ) {
                *next_id += 1;
                vec![s]
            } else {
                Vec::new()
            }
        }
        RawDxfEntityKind::Circle { cx, cy, r } => {
            let [wcx, wcy] = tf.apply(*cx, *cy);
            let rx = (*r * tf.scale_x).abs();
            let ry = (*r * tf.scale_y).abs();
            let tag = if (rx - ry).abs() < 1e-6 {
                "CIRCLE"
            } else {
                "ELLIPSE"
            };
            let id = *next_id;
            *next_id += 1;
            vec![build_ellipse_or_circle_shape(
                id,
                tag,
                wcx,
                wcy,
                rx,
                ry,
                tf.rot_rad,
                layer_ix,
                layer_locked,
                entity_color,
            )]
        }
        RawDxfEntityKind::Arc {
            cx,
            cy,
            r,
            start_deg,
            end_deg,
            extrusion_z,
        } => {
            let eff_cx = if *extrusion_z < 0.0 { -*cx } else { *cx };
            let (s_deg, e_deg) = if *extrusion_z < 0.0 {
                (180.0 - *end_deg, 180.0 - *start_deg)
            } else {
                (*start_deg, *end_deg)
            };
            let [wcx, wcy] = tf.apply(eff_cx, *cy);
            if (tf.scale_x.abs() - tf.scale_y.abs()).abs() < 1e-6 {
                let rad = (*r * tf.scale_x).abs();
                let ccw = tf.orientation_sign() > 0.0;
                let s_rad = if tf.scale_x < 0.0 {
                    PI - s_deg.to_radians()
                } else {
                    s_deg.to_radians()
                } + tf.rot_rad;
                let e_rad = if tf.scale_x < 0.0 {
                    PI - e_deg.to_radians()
                } else {
                    e_deg.to_radians()
                } + tf.rot_rad;
                let (sr, er) = if tf.scale_y < 0.0 && tf.scale_x > 0.0 {
                    (-s_deg.to_radians() + tf.rot_rad, -e_deg.to_radians() + tf.rot_rad)
                } else {
                    (s_rad, e_rad)
                };
                let id = *next_id;
                *next_id += 1;
                vec![build_arc_shape(
                    id,
                    wcx,
                    wcy,
                    rad,
                    sr,
                    er,
                    ccw,
                    layer_ix,
                    layer_locked,
                    entity_color,
                )]
            } else {
                // Non-uniformly scaled ARC becomes a smooth polyline curve
                let mut sweep = (e_deg - s_deg).rem_euclid(360.0);
                if sweep.abs() < 1e-9 {
                    sweep = 360.0;
                }
                let steps = 24usize;
                let verts: Vec<PolyVertex> = (0..=steps)
                    .map(|k| {
                        let a = (s_deg + sweep * (k as f64 / steps as f64)).to_radians();
                        let [px, py] = tf.apply(eff_cx + *r * a.cos(), *cy + *r * a.sin());
                        PolyVertex {
                            x: px,
                            y: py,
                            bulge: 0.0,
                        }
                    })
                    .collect();
                let id = *next_id;
                if let Some(s) = build_polyline_shape(
                    id,
                    "ARC",
                    &verts,
                    false,
                    layer_ix,
                    layer_locked,
                    entity_color,
                    None,
                ) {
                    *next_id += 1;
                    vec![s]
                } else {
                    Vec::new()
                }
            }
        }
        RawDxfEntityKind::Ellipse {
            cx,
            cy,
            mx,
            my,
            ratio,
            start_param,
            end_param,
        } => {
            let [wcx, wcy] = tf.apply(*cx, *cy);
            let [wmx_pt, wmy_pt] = tf.apply(*cx + *mx, *cy + *my);
            let dmx = wmx_pt - wcx;
            let dmy = wmy_pt - wcy;
            let rx = dmx.hypot(dmy).max(1e-6);
            let angle_rad = dmy.atan2(dmx);
            let ry = (rx * ratio.abs() * (tf.scale_y.abs() / tf.scale_x.abs().max(1e-9))).max(1e-6);
            if (*end_param - *start_param).abs() >= TAU - 1e-4 {
                let id = *next_id;
                *next_id += 1;
                vec![build_ellipse_or_circle_shape(
                    id,
                    "ELLIPSE",
                    wcx,
                    wcy,
                    rx,
                    ry,
                    angle_rad,
                    layer_ix,
                    layer_locked,
                    entity_color,
                )]
            } else {
                let base_ang = my.atan2(*mx);
                let maj_len = mx.hypot(*my);
                let min_len = maj_len * ratio.abs();
                let (bs, bc) = base_ang.sin_cos();
                let mut sweep = (*end_param - *start_param).rem_euclid(TAU);
                if sweep.abs() < 1e-9 {
                    sweep = TAU;
                }
                let steps = 32usize;
                let verts: Vec<PolyVertex> = (0..=steps)
                    .map(|k| {
                        let t = *start_param + sweep * (k as f64 / steps as f64);
                        let lx = maj_len * t.cos();
                        let ly = min_len * t.sin();
                        let ex = *cx + lx * bc - ly * bs;
                        let ey = *cy + lx * bs + ly * bc;
                        let [px, py] = tf.apply(ex, ey);
                        PolyVertex {
                            x: px,
                            y: py,
                            bulge: 0.0,
                        }
                    })
                    .collect();
                let id = *next_id;
                if let Some(s) = build_polyline_shape(
                    id,
                    "ELLIPSE",
                    &verts,
                    false,
                    layer_ix,
                    layer_locked,
                    entity_color,
                    None,
                ) {
                    *next_id += 1;
                    vec![s]
                } else {
                    Vec::new()
                }
            }
        }
        RawDxfEntityKind::Spline {
            degree,
            closed,
            knots,
            ctrl_pts,
            weights,
            fit_pts,
        } => {
            let pts = evaluate_spline_points(*degree, *closed, knots, ctrl_pts, weights, fit_pts);
            let verts: Vec<PolyVertex> = pts
                .into_iter()
                .map(|[x, y]| {
                    let [px, py] = tf.apply(x, y);
                    PolyVertex {
                        x: px,
                        y: py,
                        bulge: 0.0,
                    }
                })
                .collect();
            let id = *next_id;
            if let Some(s) = build_polyline_shape(
                id,
                "SPLINE",
                &verts,
                *closed,
                layer_ix,
                layer_locked,
                entity_color,
                None,
            ) {
                *next_id += 1;
                vec![s]
            } else {
                Vec::new()
            }
        }
        RawDxfEntityKind::Text {
            x,
            y,
            height,
            rot_deg,
            text,
            is_mtext,
            width_hint,
        } => {
            let [wx, wy] = tf.apply(*x, *y);
            let h_in = (*height * tf.uniform_scale()).abs().max(0.05);
            let w_in = width_hint.map(|w| (w * tf.scale_x).abs());
            let ang = rot_deg.to_radians() + tf.rot_rad;
            let id = *next_id;
            *next_id += 1;
            vec![build_text_shape(
                id,
                if *is_mtext { "MTEXT" } else { "TEXT" },
                wx,
                wy,
                h_in,
                w_in,
                ang,
                text.clone(),
                layer_ix,
                layer_locked,
                entity_color,
            )]
        }
        RawDxfEntityKind::Solid { pts } => {
            let mut verts = Vec::with_capacity(4);
            for &[x, y] in pts {
                let [px, py] = tf.apply(x, y);
                if verts
                    .last()
                    .is_none_or(|prev: &PolyVertex| (prev.x - px).hypot(prev.y - py) > 1e-9)
                {
                    verts.push(PolyVertex {
                        x: px,
                        y: py,
                        bulge: 0.0,
                    });
                }
            }
            let id = *next_id;
            if let Some(s) = build_polyline_shape(
                id,
                "SOLID",
                &verts,
                true,
                layer_ix,
                layer_locked,
                entity_color,
                Some(entity_color),
            ) {
                *next_id += 1;
                vec![s]
            } else {
                Vec::new()
            }
        }
        RawDxfEntityKind::Hatch { solid_fill, loops } => {
            let sign = tf.orientation_sign();
            let mut min_x = f64::INFINITY;
            let mut min_y = f64::INFINITY;
            let mut max_x = f64::NEG_INFINITY;
            let mut max_y = f64::NEG_INFINITY;
            let mut tf_loops: Vec<(Vec<PolyVertex>, bool)> = Vec::new();
            for (lp, closed) in loops {
                let tv: Vec<PolyVertex> = lp
                    .iter()
                    .map(|v| {
                        let [px, py] = tf.apply(v.x, v.y);
                        min_x = min_x.min(px);
                        min_y = min_y.min(py);
                        max_x = max_x.max(px);
                        max_y = max_y.max(py);
                        PolyVertex {
                            x: px,
                            y: py,
                            bulge: v.bulge * sign,
                        }
                    })
                    .collect();
                if !tv.is_empty() {
                    tf_loops.push((tv, *closed));
                }
            }
            if tf_loops.is_empty() || !min_x.is_finite() {
                return Vec::new();
            }
            let w = (max_x - min_x).max(0.0);
            let h = (max_y - min_y).max(0.0);
            let mut sheet = init_2d_cad_sheet(min_x, min_y, w, h);
            sheet.geometries.clear();
            for (g_idx, (verts, closed)) in tf_loops.iter().enumerate() {
                let mut geom = GeometrySection::new((g_idx + 1) as u32);
                geom.no_fill = if *solid_fill {
                    "FALSE".into()
                } else {
                    "TRUE".into()
                };
                let seg_count = if *closed && verts.len() >= 2 {
                    verts.len()
                } else {
                    verts.len().saturating_sub(1)
                };
                let mut rows = Vec::with_capacity(seg_count + 1);
                rows.push(GeometryRow::move_to(
                    1,
                    format!("{} in", format_clean_f64(verts[0].x - min_x)),
                    format!("{} in", format_clean_f64(verts[0].y - min_y)),
                ));
                let mut ix = 2u32;
                for s_i in 0..seg_count {
                    let cur = verts[s_i];
                    let nxt = verts[(s_i + 1) % verts.len()];
                    let lx = format!("{} in", format_clean_f64(nxt.x - min_x));
                    let ly = format!("{} in", format_clean_f64(nxt.y - min_y));
                    let chord = (nxt.x - cur.x).hypot(nxt.y - cur.y);
                    if cur.bulge.abs() > 1e-9 && chord > 1e-12 {
                        rows.push(GeometryRow::arc_to(
                            ix,
                            lx,
                            ly,
                            format!("{} in", format_clean_f64(cur.bulge * chord * 0.5)),
                        ));
                    } else {
                        rows.push(GeometryRow::line_to(ix, lx, ly));
                    }
                    ix += 1;
                }
                geom.rows = rows;
                sheet.add_geometry(geom);
            }
            let id = *next_id;
            *next_id += 1;
            let name = format!("CAD.HATCH.{id}");
            let mut shape = VisioShape {
                id,
                name: name.clone(),
                name_u: name,
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
                foreign_data: None,
                opaque_xml_fragments: Vec::new(),
                bound_data_row: None,
            };
            let fill = if *solid_fill { Some(entity_color) } else { None };
            apply_cad_shape_metadata(&mut shape, "HATCH", layer_ix, layer_locked, entity_color, fill);
            vec![shape]
        }
        RawDxfEntityKind::Insert {
            block_name,
            x,
            y,
            sx,
            sy,
            rot_deg,
            cols,
            rows,
            col_spacing,
            row_spacing,
        } => {
            let Some(blk) = blocks.get(&block_name.to_ascii_uppercase()) else {
                return Vec::new();
            };
            let mut instances = Vec::new();
            let rot_r = rot_deg.to_radians();
            let (rs, rc) = rot_r.sin_cos();
            for r_idx in 0..*rows {
                for c_idx in 0..*cols {
                    let dx_local = (c_idx as f64) * *col_spacing;
                    let dy_local = (r_idx as f64) * *row_spacing;
                    let ins_x = *x + dx_local * rc - dy_local * rs;
                    let ins_y = *y + dx_local * rs + dy_local * rc;
                    let [world_tx, world_ty] = tf.apply(ins_x, ins_y);
                    let child_tf = Affine2D {
                        base_x: blk.base_x,
                        base_y: blk.base_y,
                        scale_x: *sx * tf.scale_x,
                        scale_y: *sy * tf.scale_y,
                        rot_rad: rot_r + tf.rot_rad,
                        tx: world_tx,
                        ty: world_ty,
                    };
                    let mut children = Vec::new();
                    for sub in &blk.entities {
                        children.extend(instantiate_entity(
                            page,
                            sub,
                            blocks,
                            &child_tf,
                            Some(effective_layer),
                            depth + 1,
                            next_id,
                        ));
                    }
                    if let Some(group_shape) = wrap_block_instance_group(
                        block_name,
                        children,
                        layer_ix,
                        layer_locked,
                        entity_color,
                        next_id,
                    ) {
                        instances.push(group_shape);
                    }
                }
            }
            instances
        }
        RawDxfEntityKind::Dimension {
            block_name,
            def_pt,
            text_mid,
            ext1,
            ext2,
            text_override,
        } => {
            if let Some(bname) = block_name {
                if let Some(blk) = blocks.get(&bname.to_ascii_uppercase()) {
                    let child_tf = Affine2D {
                        base_x: blk.base_x,
                        base_y: blk.base_y,
                        ..*tf
                    };
                    let mut children = Vec::new();
                    for sub in &blk.entities {
                        children.extend(instantiate_entity(
                            page,
                            sub,
                            blocks,
                            &child_tf,
                            Some(effective_layer),
                            depth + 1,
                            next_id,
                        ));
                    }
                    if let Some(grp) = wrap_block_instance_group(
                        bname,
                        children,
                        layer_ix,
                        layer_locked,
                        entity_color,
                        next_id,
                    ) {
                        return vec![grp];
                    }
                }
            }
            // Synthesize dimension geometry + measurement label when anonymous block is absent
            let p_ext1 = tf.apply(ext1[0], ext1[1]);
            let p_ext2 = tf.apply(ext2[0], ext2[1]);
            let p_def = tf.apply(def_pt[0], def_pt[1]);
            let p_txt = tf.apply(text_mid[0], text_mid[1]);
            let measured = (ext2[0] - ext1[0]).hypot(ext2[1] - ext1[1]);
            let label = if text_override.trim().is_empty() || text_override.trim() == "<>" {
                format_clean_f64((measured * 1000.0).round() / 1000.0)
            } else {
                strip_mtext_formatting(&text_override.replace("<>", &format_clean_f64(measured)))
            };
            let dim_start = [p_ext1[0], p_def[1]];
            let dim_end = [p_ext2[0], p_def[1]];
            let mut children = Vec::new();
            let id1 = *next_id;
            *next_id += 1;
            children.push(build_line_shape(
                id1,
                p_ext1,
                dim_start,
                layer_ix,
                layer_locked,
                entity_color,
            ));
            let id2 = *next_id;
            *next_id += 1;
            children.push(build_line_shape(
                id2,
                p_ext2,
                dim_end,
                layer_ix,
                layer_locked,
                entity_color,
            ));
            let id3 = *next_id;
            *next_id += 1;
            children.push(build_line_shape(
                id3,
                dim_start,
                dim_end,
                layer_ix,
                layer_locked,
                entity_color,
            ));
            let id4 = *next_id;
            *next_id += 1;
            let mid_pos = if p_txt[0].abs() < 1e-9 && p_txt[1].abs() < 1e-9 {
                [(dim_start[0] + dim_end[0]) * 0.5, dim_start[1] + 0.12]
            } else {
                p_txt
            };
            children.push(build_text_shape(
                id4,
                "MTEXT",
                mid_pos[0],
                mid_pos[1],
                0.14,
                None,
                0.0,
                label,
                layer_ix,
                layer_locked,
                entity_color,
            ));
            wrap_block_instance_group(
                "DIMENSION",
                children,
                layer_ix,
                layer_locked,
                entity_color,
                next_id,
            )
            .into_iter()
            .collect()
        }
    }
}

fn wrap_block_instance_group(
    block_name: &str,
    mut children: Vec<VisioShape>,
    layer_ix: u32,
    layer_locked: bool,
    stroke: RgbaColor,
    next_id: &mut u64,
) -> Option<VisioShape> {
    if children.is_empty() {
        return None;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for ch in &children {
        let px = ch.sheet.get_f64("PinX").unwrap_or(0.0);
        let py = ch.sheet.get_f64("PinY").unwrap_or(0.0);
        let cw = ch.sheet.get_f64("Width").unwrap_or(0.0);
        let ch_h = ch.sheet.get_f64("Height").unwrap_or(0.0);
        min_x = min_x.min(px - cw * 0.5);
        min_y = min_y.min(py - ch_h * 0.5);
        max_x = max_x.max(px + cw * 0.5);
        max_y = max_y.max(py + ch_h * 0.5);
    }
    let gw = (max_x - min_x).max(0.01);
    let gh = (max_y - min_y).max(0.01);
    // Shift children PinX/PinY from page space into group-local space (`0..gw`, `0..gh`)
    for ch in &mut children {
        let px = ch.sheet.get_f64("PinX").unwrap_or(0.0) - min_x;
        let py = ch.sheet.get_f64("PinY").unwrap_or(0.0) - min_y;
        ch.sheet.set_formula(
            "PinX",
            format!("{} in", format_clean_f64(px)),
            Some(Unit::Inch),
        );
        ch.sheet.set_formula(
            "PinY",
            format!("{} in", format_clean_f64(py)),
            Some(Unit::Inch),
        );
    }

    let gid = *next_id;
    *next_id += 1;
    let mut sheet = init_2d_cad_sheet(min_x, min_y, gw, gh);
    sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    let mut geom = GeometrySection::rectangle(1);
    geom.no_show = "TRUE".into();
    geom.no_fill = "TRUE".into();
    geom.no_line = "TRUE".into();
    sheet.add_geometry(geom);
    sheet.set_user_cell(
        "CadBlockName",
        format!("\"{block_name}\""),
        "Source DXF Block Name",
    );

    let name = format!("CAD.Insert.{block_name}.{gid}");
    let mut grp = VisioShape {
        id: gid,
        name: name.clone(),
        name_u: name,
        kind: ShapeKind::Group,
        master_id: None,
        master_shape_id: None,
        line_style_id: None,
        fill_style_id: None,
        text_style_id: None,
        text: String::new(),
        sheet,
        children,
        layer_membership: Vec::new(),
        foreign_data: None,
        opaque_xml_fragments: Vec::new(),
        bound_data_row: None,
    };
    apply_cad_shape_metadata(&mut grp, "INSERT", layer_ix, layer_locked, stroke, None);
    grp.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    Some(grp)
}

// --- DXF R2000+ Exporter ---

/// Export `page_id` from `doc` as a valid AutoCAD R2000+ (`AC1015`) ASCII DXF document.
pub fn export_page_to_dxf(doc: &VisioDocument, page_id: u32) -> String {
    let mut eval_doc = doc.clone();
    ReactiveEngine::recalculate_document(&mut eval_doc);

    let Some(page) = eval_doc.page(page_id) else {
        return "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n0\nEOF\n".into();
    };
    let visuals = ReactiveEngine::evaluate_page_visuals(&eval_doc, page_id);

    let mut dxf = String::with_capacity(4096 + visuals.len() * 256);
    let push_pair = |buf: &mut String, code: i32, val: &str| {
        buf.push_str(&format!("{code}\n{val}\n"));
    };

    let page_w = page.width_in();
    let page_h = page.height_in();

    // 1. HEADER section
    push_pair(&mut dxf, 0, "SECTION");
    push_pair(&mut dxf, 2, "HEADER");
    push_pair(&mut dxf, 9, "$ACADVER");
    push_pair(&mut dxf, 1, "AC1015");
    push_pair(&mut dxf, 9, "$INSUNITS");
    push_pair(&mut dxf, 70, "1"); // Canonical Visio Inches
    push_pair(&mut dxf, 9, "$EXTMIN");
    push_pair(&mut dxf, 10, "0");
    push_pair(&mut dxf, 20, "0");
    push_pair(&mut dxf, 30, "0");
    push_pair(&mut dxf, 9, "$EXTMAX");
    push_pair(&mut dxf, 10, &format_clean_f64(page_w));
    push_pair(&mut dxf, 20, &format_clean_f64(page_h));
    push_pair(&mut dxf, 30, "0");
    push_pair(&mut dxf, 0, "ENDSEC");

    // 2. TABLES -> LAYER table
    push_pair(&mut dxf, 0, "SECTION");
    push_pair(&mut dxf, 2, "TABLES");
    push_pair(&mut dxf, 0, "TABLE");
    push_pair(&mut dxf, 2, "LAYER");
    let has_zero = page.layers.iter().any(|l| l.name == "0");
    let layer_count = page.layers.len() + usize::from(!has_zero);
    push_pair(&mut dxf, 70, &layer_count.to_string());

    if !has_zero {
        emit_dxf_layer_record(&mut dxf, "0", true, true, false, None);
    }
    for l in &page.layers {
        let clean_name = sanitize_dxf_symbol(&l.name);
        emit_dxf_layer_record(
            &mut dxf,
            &clean_name,
            l.visible,
            l.print,
            l.lock,
            l.color_override,
        );
    }
    push_pair(&mut dxf, 0, "ENDTAB");
    push_pair(&mut dxf, 0, "ENDSEC");

    // 3. BLOCKS section (minimal R2000 *Model_Space / *Paper_Space)
    push_pair(&mut dxf, 0, "SECTION");
    push_pair(&mut dxf, 2, "BLOCKS");
    for bname in ["*Model_Space", "*Paper_Space"] {
        push_pair(&mut dxf, 0, "BLOCK");
        push_pair(&mut dxf, 8, "0");
        push_pair(&mut dxf, 2, bname);
        push_pair(&mut dxf, 70, "0");
        push_pair(&mut dxf, 10, "0");
        push_pair(&mut dxf, 20, "0");
        push_pair(&mut dxf, 30, "0");
        push_pair(&mut dxf, 0, "ENDBLK");
        push_pair(&mut dxf, 8, "0");
    }
    push_pair(&mut dxf, 0, "ENDSEC");

    // 4. ENTITIES section
    push_pair(&mut dxf, 0, "SECTION");
    push_pair(&mut dxf, 2, "ENTITIES");

    for vis in &visuals {
        let shape_ref = page.find_shape(vis.shape_id);
        let layer_name = shape_ref
            .and_then(|s| s.layer_membership.first())
            .and_then(|&lix| page.layers.iter().find(|l| l.ix == lix))
            .map(|l| sanitize_dxf_symbol(&l.name))
            .unwrap_or_else(|| "0".into());

        let stroke_color = vis.stroke_color.or(vis.fill_color).unwrap_or(RgbaColor::BLACK);
        let aci = rgba_to_aci(stroke_color);
        let truecolor = ((stroke_color.r as u32) << 16)
            | ((stroke_color.g as u32) << 8)
            | (stroke_color.b as u32);

        for path in &vis.paths {
            if path.no_show || (path.no_line && path.no_fill) {
                continue;
            }
            emit_visual_path_to_dxf(
                &mut dxf,
                &path.verbs_page_space,
                &layer_name,
                aci,
                truecolor,
            );
        }

        if !vis.text.trim().is_empty() {
            let txt_color = vis.text_color;
            let txt_aci = rgba_to_aci(txt_color);
            let txt_tc = ((txt_color.r as u32) << 16)
                | ((txt_color.g as u32) << 8)
                | (txt_color.b as u32);
            let encoded_txt = vis
                .text
                .replace('\\', "\\\\")
                .replace('{', "\\{")
                .replace('}', "\\}")
                .replace('\n', "\\P");
            let text_h_in = (vis.font_size_pt / 72.0).max(0.05);
            push_pair(&mut dxf, 0, "MTEXT");
            push_pair(&mut dxf, 100, "AcDbEntity");
            push_pair(&mut dxf, 8, &layer_name);
            push_pair(&mut dxf, 62, &txt_aci.to_string());
            push_pair(&mut dxf, 420, &txt_tc.to_string());
            push_pair(&mut dxf, 100, "AcDbMText");
            push_pair(&mut dxf, 10, &format_clean_f64(vis.transform.pin_x));
            push_pair(&mut dxf, 20, &format_clean_f64(vis.transform.pin_y));
            push_pair(&mut dxf, 30, "0");
            push_pair(&mut dxf, 40, &format_clean_f64(text_h_in));
            push_pair(&mut dxf, 41, &format_clean_f64(vis.transform.width.max(text_h_in)));
            push_pair(&mut dxf, 71, "5"); // Middle-center attachment
            push_pair(&mut dxf, 50, &format_clean_f64(vis.transform.angle_rad.to_degrees()));
            push_pair(&mut dxf, 1, &encoded_txt);
        }
    }

    push_pair(&mut dxf, 0, "ENDSEC");
    push_pair(&mut dxf, 0, "EOF");
    dxf
}

fn sanitize_dxf_symbol(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if matches!(c, '<' | '>' | '/' | '\\' | '"' | ':' | ';' | '?' | '*' | '|' | '=' | '`')
            {
                '_'
            } else {
                c
            }
        })
        .collect();
    let trimmed = s.trim();
    if trimmed.is_empty() {
        "0".into()
    } else {
        trimmed.to_string()
    }
}

fn emit_dxf_layer_record(
    dxf: &mut String,
    name: &str,
    visible: bool,
    print: bool,
    lock: bool,
    color_override: Option<RgbaColor>,
) {
    let mut flags = 0i32;
    if !visible && !print {
        flags |= 1; // Frozen
    }
    if lock {
        flags |= 4; // Locked
    }
    let base_aci = color_override.map_or(7, rgba_to_aci);
    let signed_aci = if !visible && print { -base_aci } else { base_aci };
    dxf.push_str("0\nLAYER\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n");
    dxf.push_str(&format!("2\n{name}\n70\n{flags}\n62\n{signed_aci}\n"));
    if let Some(c) = color_override {
        let tc = ((c.r as u32) << 16) | ((c.g as u32) << 8) | (c.b as u32);
        dxf.push_str(&format!("420\n{tc}\n"));
    }
    dxf.push_str(&format!("290\n{}\n6\nCONTINUOUS\n", if print { 1 } else { 0 }));
}

fn emit_visual_path_to_dxf(
    dxf: &mut String,
    verbs: &[EvaluatedPathVerb],
    layer: &str,
    aci: i16,
    truecolor: u32,
) {
    if verbs.is_empty() {
        return;
    }
    // Single CIRCLE / ELLIPSE primitive
    if verbs.len() == 1 {
        if let EvaluatedPathVerb::Ellipse {
            center,
            rx,
            ry,
            angle_rad,
        } = verbs[0]
        {
            if (rx - ry).abs() <= 1e-6 {
                dxf.push_str(&format!(
                    "0\nCIRCLE\n100\nAcDbEntity\n8\n{layer}\n62\n{aci}\n420\n{truecolor}\n100\nAcDbCircle\n10\n{}\n20\n{}\n30\n0\n40\n{}\n",
                    format_clean_f64(center[0]),
                    format_clean_f64(center[1]),
                    format_clean_f64(rx),
                ));
            } else {
                let (major, minor, maj_ang) = if rx >= ry {
                    (rx, ry, angle_rad)
                } else {
                    (ry, rx, angle_rad + PI * 0.5)
                };
                let mx = major * maj_ang.cos();
                let my = major * maj_ang.sin();
                let ratio = (minor / major.max(1e-9)).clamp(1e-6, 1.0);
                dxf.push_str(&format!(
                    "0\nELLIPSE\n100\nAcDbEntity\n8\n{layer}\n62\n{aci}\n420\n{truecolor}\n100\nAcDbEllipse\n10\n{}\n20\n{}\n30\n0\n11\n{}\n21\n{}\n31\n0\n40\n{}\n41\n0\n42\n{}\n",
                    format_clean_f64(center[0]),
                    format_clean_f64(center[1]),
                    format_clean_f64(mx),
                    format_clean_f64(my),
                    format_clean_f64(ratio),
                    format_clean_f64(TAU),
                ));
            }
            return;
        }
    }

    // Single LINE primitive (`[MoveTo(p1), LineTo(p2)]`)
    if let [EvaluatedPathVerb::MoveTo(p1), EvaluatedPathVerb::LineTo(p2)] = verbs {
        dxf.push_str(&format!(
            "0\nLINE\n100\nAcDbEntity\n8\n{layer}\n62\n{aci}\n420\n{truecolor}\n100\nAcDbLine\n10\n{}\n20\n{}\n30\n0\n11\n{}\n21\n{}\n31\n0\n",
            format_clean_f64(p1[0]),
            format_clean_f64(p1[1]),
            format_clean_f64(p2[0]),
            format_clean_f64(p2[1]),
        ));
        return;
    }

    // Single circular ARC primitive (`[MoveTo(p1), ArcTo { to: p2, bow }]`)
    if let [EvaluatedPathVerb::MoveTo(p1), EvaluatedPathVerb::ArcTo { to: p2, bow }] = verbs {
        if let Some(([cx, cy], radius, start_deg, end_deg)) =
            arc_params_from_endpoints_and_bow(*p1, *p2, *bow)
        {
            dxf.push_str(&format!(
                "0\nARC\n100\nAcDbEntity\n8\n{layer}\n62\n{aci}\n420\n{truecolor}\n100\nAcDbCircle\n10\n{}\n20\n{}\n30\n0\n40\n{}\n100\nAcDbArc\n50\n{}\n51\n{}\n",
                format_clean_f64(cx),
                format_clean_f64(cy),
                format_clean_f64(radius),
                format_clean_f64(start_deg),
                format_clean_f64(end_deg),
            ));
            return;
        }
    }

    // General multi-subpath -> one or more `LWPOLYLINE` (or `CIRCLE`) entities
    let mut current_poly: Vec<PolyVertex> = Vec::new();
    let mut current_closed = false;

    let flush_poly = |buf: &mut String, verts: &mut Vec<PolyVertex>, closed: &mut bool| {
        if verts.len() >= 2 {
            // Detect implicit closure when last vertex coincides with first vertex
            if !*closed && verts.len() >= 3 {
                let first = verts[0];
                let last = verts[verts.len() - 1];
                if (first.x - last.x).hypot(first.y - last.y) < 1e-6 && last.bulge.abs() < 1e-9 {
                    verts.pop();
                    *closed = true;
                }
            }
            buf.push_str(&format!(
                "0\nLWPOLYLINE\n100\nAcDbEntity\n8\n{layer}\n62\n{aci}\n420\n{truecolor}\n100\nAcDbPolyline\n90\n{}\n70\n{}\n",
                verts.len(),
                if *closed { 1 } else { 0 }
            ));
            for v in verts.iter() {
                buf.push_str(&format!(
                    "10\n{}\n20\n{}\n",
                    format_clean_f64(v.x),
                    format_clean_f64(v.y)
                ));
                if v.bulge.abs() > 1e-9 {
                    buf.push_str(&format!("42\n{}\n", format_clean_f64(v.bulge)));
                }
            }
        }
        verts.clear();
        *closed = false;
    };

    for verb in verbs {
        match *verb {
            EvaluatedPathVerb::MoveTo(pt) => {
                flush_poly(dxf, &mut current_poly, &mut current_closed);
                current_poly.push(PolyVertex {
                    x: pt[0],
                    y: pt[1],
                    bulge: 0.0,
                });
            }
            EvaluatedPathVerb::LineTo(pt) => {
                current_poly.push(PolyVertex {
                    x: pt[0],
                    y: pt[1],
                    bulge: 0.0,
                });
            }
            EvaluatedPathVerb::ArcTo { to, bow } => {
                if let Some(prev) = current_poly.last_mut() {
                    let chord = (to[0] - prev.x).hypot(to[1] - prev.y);
                    if chord > 1e-9 && bow.abs() > 1e-9 {
                        prev.bulge = (2.0 * bow) / chord;
                    }
                }
                current_poly.push(PolyVertex {
                    x: to[0],
                    y: to[1],
                    bulge: 0.0,
                });
            }
            EvaluatedPathVerb::QuadTo { ctrl, to } => {
                let start = current_poly
                    .last()
                    .map(|v| [v.x, v.y])
                    .unwrap_or(ctrl);
                for s in 1..=8 {
                    let t = s as f64 / 8.0;
                    let mt = 1.0 - t;
                    let x = mt * mt * start[0] + 2.0 * mt * t * ctrl[0] + t * t * to[0];
                    let y = mt * mt * start[1] + 2.0 * mt * t * ctrl[1] + t * t * to[1];
                    current_poly.push(PolyVertex { x, y, bulge: 0.0 });
                }
            }
            EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                let start = current_poly
                    .last()
                    .map(|v| [v.x, v.y])
                    .unwrap_or(ctrl1);
                for s in 1..=10 {
                    let t = s as f64 / 10.0;
                    let mt = 1.0 - t;
                    let x = mt * mt * mt * start[0]
                        + 3.0 * mt * mt * t * ctrl1[0]
                        + 3.0 * mt * t * t * ctrl2[0]
                        + t * t * t * to[0];
                    let y = mt * mt * mt * start[1]
                        + 3.0 * mt * mt * t * ctrl1[1]
                        + 3.0 * mt * t * t * ctrl2[1]
                        + t * t * t * to[1];
                    current_poly.push(PolyVertex { x, y, bulge: 0.0 });
                }
            }
            EvaluatedPathVerb::Ellipse {
                center,
                rx,
                ry,
                angle_rad,
            } => {
                flush_poly(dxf, &mut current_poly, &mut current_closed);
                emit_visual_path_to_dxf(
                    dxf,
                    &[EvaluatedPathVerb::Ellipse {
                        center,
                        rx,
                        ry,
                        angle_rad,
                    }],
                    layer,
                    aci,
                    truecolor,
                );
            }
            EvaluatedPathVerb::Close => {
                current_closed = true;
                flush_poly(dxf, &mut current_poly, &mut current_closed);
            }
        }
    }
    flush_poly(dxf, &mut current_poly, &mut current_closed);
}
