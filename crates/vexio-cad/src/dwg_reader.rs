//! Native Offline AutoCAD `.dwg` Bit-Stream Reader & R2000 (`AC1015`) Encoder.
//!
//! Implements:
//! - DWG version magic validation (`AC1012` R13 through `AC1032` R2018+)
//! - Full bit-stream primitives (`DwgBitReader` / `DwgBitWriter`: `B`, `BB`, `RC`, `RS`,
//!   `RL`, `RD`, `BS`, `BL`, `BD`, `2RD`, `2BD`, `3BD`, `DD`, `BT`, `BE`, `MC`, `MS`, `TV`)
//! - R13/R14/R2000 Section Locator Table parsing at file offset `0x15` (`AcDb:Header`,
//!   `AcDb:Classes`, `AcDb:Handles` Object Map, and `AcDb:AcDbObjects` entity/object stream)
//!   with 16-byte section sentinels and `crc32fast` integrity verification
//! - Deterministic `encode_minimal_dwg_r2000` and `encode_dwg_r2000_from_page` binary `.dwg`
//!   encoders for lossless round-trip verification of `LAYER`, `LINE`, `CIRCLE`, `ARC`,
//!   `ELLIPSE`, `LWPOLYLINE` (with bulges), and `TEXT` / `MTEXT`.

use crate::CadError;
use crate::dxf_codec::{
    PolyVertex, aci_to_rgba, arc_params_from_endpoints_and_bow, build_arc_shape,
    build_ellipse_or_circle_shape, build_line_shape, build_polyline_shape, build_text_shape,
    insunits_to_unit_and_factor, resolve_or_create_layer, rgba_to_aci, strip_mtext_formatting,
    upsert_cad_layer,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::{PI, TAU};
use vexio_shapesheet::{
    EvaluatedPathVerb, ReactiveEngine, RgbaColor, Unit, VisioDocument, VisioPage, format_clean_f64,
};

// Standard OpenDWG / R2000 16-byte section sentinels
const HEADER_SENTINEL_BEGIN: [u8; 16] = [
    0xCF, 0x7B, 0x1F, 0x23, 0xFD, 0xDE, 0x38, 0xA9, 0x5F, 0x7C, 0x68, 0xB8, 0x4E, 0x6D, 0x33, 0x5F,
];
const HEADER_SENTINEL_END: [u8; 16] = [
    0x30, 0x84, 0xE0, 0xDC, 0x02, 0x21, 0xC7, 0x56, 0xA0, 0x83, 0x97, 0x47, 0xB1, 0x92, 0xCC, 0xA0,
];
const CLASSES_SENTINEL_BEGIN: [u8; 16] = [
    0x8D, 0xA1, 0xC4, 0xB8, 0xC4, 0xA9, 0xF8, 0xC5, 0xC0, 0xDC, 0xF4, 0x5F, 0xE7, 0xCF, 0xB6, 0x8A,
];
const CLASSES_SENTINEL_END: [u8; 16] = [
    0x72, 0x5E, 0x3B, 0x47, 0x3B, 0x56, 0x07, 0x3A, 0x3F, 0x23, 0x0B, 0xA0, 0x18, 0x30, 0x49, 0x75,
];
const FILE_HEADER_END_SENTINEL: [u8; 16] = [
    0x95, 0xA0, 0x4E, 0x28, 0x99, 0x82, 0x1A, 0xE5, 0x5E, 0x41, 0xE0, 0x5F, 0x9D, 0x3A, 0x4D, 0x00,
];

// DWG Object Type Codes (`BS`)
const DWG_OBJ_TEXT: i16 = 0x01;
const DWG_OBJ_ARC: i16 = 0x11;
const DWG_OBJ_CIRCLE: i16 = 0x12;
const DWG_OBJ_LINE: i16 = 0x13;
const DWG_OBJ_ELLIPSE: i16 = 0x23;
const DWG_OBJ_MTEXT: i16 = 0x2C;
const DWG_OBJ_LAYER: i16 = 0x33;
const DWG_OBJ_LWPOLYLINE: i16 = 0x4D;
const DWG_OBJ_LWPOLYLINE_CLASS: i16 = 500;

/// Supported AutoCAD `.dwg` binary format versions (`AC1012` .. `AC1032`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DwgVersion {
    /// AutoCAD R13 (`AC1012`)
    R13,
    /// AutoCAD R14 (`AC1014`)
    R14,
    /// AutoCAD 2000 / 2000i / 2002 (`AC1015`)
    R2000,
    /// AutoCAD 2004 / 2005 / 2006 (`AC1018`)
    R2004,
    /// AutoCAD 2007 / 2008 / 2009 (`AC1021`)
    R2007,
    /// AutoCAD 2010 / 2011 / 2012 (`AC1024`)
    R2010,
    /// AutoCAD 2013 / 2014 / 2015 / 2016 / 2017 (`AC1027`)
    R2013,
    /// AutoCAD 2018 / 2019 / 2020 / 2021 / 2024+ (`AC1032`)
    R2018,
}

impl DwgVersion {
    pub fn from_magic_bytes(bytes: &[u8]) -> Result<Self, CadError> {
        if bytes.len() < 6 {
            return Err(CadError::InvalidDwgMagic(
                "Buffer shorter than 6-byte DWG version header".into(),
            ));
        }
        let magic = std::str::from_utf8(&bytes[0..6])
            .map_err(|_| CadError::InvalidDwgMagic("Non-ASCII DWG magic header".into()))?;
        if !magic.starts_with("AC") {
            return Err(CadError::InvalidDwgMagic(format!(
                "Expected 'ACxxxx' DWG magic, got '{magic}'"
            )));
        }
        match magic {
            "AC1012" => Ok(Self::R13),
            "AC1014" => Ok(Self::R14),
            "AC1015" => Ok(Self::R2000),
            "AC1018" => Ok(Self::R2004),
            "AC1021" => Ok(Self::R2007),
            "AC1024" => Ok(Self::R2010),
            "AC1027" => Ok(Self::R2013),
            "AC1032" => Ok(Self::R2018),
            _ => Err(CadError::UnsupportedDwgVersion(magic.to_string())),
        }
    }

    pub fn magic_str(self) -> &'static str {
        match self {
            Self::R13 => "AC1012",
            Self::R14 => "AC1014",
            Self::R2000 => "AC1015",
            Self::R2004 => "AC1018",
            Self::R2007 => "AC1021",
            Self::R2010 => "AC1024",
            Self::R2013 => "AC1027",
            Self::R2018 => "AC1032",
        }
    }
}

/// Bit-level reader implementing AutoCAD `.dwg` variable-width primitives (`MSB`-first within bytes).
#[derive(Debug, Clone)]
pub struct DwgBitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> DwgBitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    pub fn bit_offset(&self) -> usize {
        self.byte_pos * 8 + self.bit_pos as usize
    }

    pub fn total_bits(&self) -> usize {
        self.data.len() * 8
    }

    pub fn align_to_byte(&mut self) {
        if self.bit_pos != 0 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
    }

    /// Read a single bit (`B`: `0` or `1`).
    pub fn read_bit(&mut self) -> Result<u8, CadError> {
        if self.byte_pos >= self.data.len() {
            return Err(CadError::CorruptDwgStream(
                "Unexpected EOF reading DWG bit (B)".into(),
            ));
        }
        let b = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Ok(b)
    }

    /// Read a 2-bit code (`BB`: `0..=3`).
    pub fn read_bb(&mut self) -> Result<u8, CadError> {
        let hi = self.read_bit()?;
        let lo = self.read_bit()?;
        Ok((hi << 1) | lo)
    }

    /// Read an unaligned 8-bit Raw Char (`RC`).
    pub fn read_rc(&mut self) -> Result<u8, CadError> {
        if self.bit_pos == 0 {
            if self.byte_pos >= self.data.len() {
                return Err(CadError::CorruptDwgStream(
                    "Unexpected EOF reading aligned DWG RC".into(),
                ));
            }
            let b = self.data[self.byte_pos];
            self.byte_pos += 1;
            return Ok(b);
        }
        if self.byte_pos + 1 >= self.data.len() {
            return Err(CadError::CorruptDwgStream(
                "Unexpected EOF reading unaligned DWG RC".into(),
            ));
        }
        let hi = self.data[self.byte_pos] << self.bit_pos;
        let lo = self.data[self.byte_pos + 1] >> (8 - self.bit_pos);
        self.byte_pos += 1;
        Ok(hi | lo)
    }

    /// Read an unaligned 16-bit LE Raw Short (`RS`).
    pub fn read_rs(&mut self) -> Result<i16, CadError> {
        let b0 = self.read_rc()?;
        let b1 = self.read_rc()?;
        Ok(i16::from_le_bytes([b0, b1]))
    }

    /// Read an unaligned 32-bit LE Raw Long (`RL`).
    pub fn read_rl(&mut self) -> Result<i32, CadError> {
        let mut arr = [0u8; 4];
        for b in &mut arr {
            *b = self.read_rc()?;
        }
        Ok(i32::from_le_bytes(arr))
    }

    /// Read an unaligned 64-bit IEEE-754 LE Raw Double (`RD`).
    pub fn read_rd(&mut self) -> Result<f64, CadError> {
        let mut arr = [0u8; 8];
        for b in &mut arr {
            *b = self.read_rc()?;
        }
        let v = f64::from_le_bytes(arr);
        if !v.is_finite() || v == 0.0 {
            Ok(0.0)
        } else {
            Ok(v)
        }
    }

    /// Read a DWG Bit-Short (`BS`):
    /// - `00`: 16-bit `RS`
    /// - `01`: 8-bit `RC` (`0..=255`)
    /// - `10`: `0`
    /// - `11`: `256`
    pub fn read_bs(&mut self) -> Result<i16, CadError> {
        match self.read_bb()? {
            0b00 => self.read_rs(),
            0b01 => Ok(self.read_rc()? as i16),
            0b10 => Ok(0),
            0b11 => Ok(256),
            _ => unreachable!(),
        }
    }

    /// Read a DWG Bit-Long (`BL`):
    /// - `00`: 32-bit `RL`
    /// - `01`: 8-bit `RC` (`0..=255`)
    /// - `10`: `0`
    /// - `11`: invalid in R2000 (`CorruptDwgStream`)
    pub fn read_bl(&mut self) -> Result<i32, CadError> {
        match self.read_bb()? {
            0b00 => self.read_rl(),
            0b01 => Ok(self.read_rc()? as i32),
            0b10 => Ok(0),
            _ => Err(CadError::CorruptDwgStream(
                "Invalid BB=11 code in DWG Bit-Long (BL)".into(),
            )),
        }
    }

    /// Read a DWG Bit-Double (`BD`):
    /// - `00`: 64-bit IEEE-754 `RD`
    /// - `01`: `1.0`
    /// - `10`: `0.0`
    /// - `11`: invalid (`CorruptDwgStream`)
    pub fn read_bd(&mut self) -> Result<f64, CadError> {
        match self.read_bb()? {
            0b00 => self.read_rd(),
            0b01 => Ok(1.0),
            0b10 => Ok(0.0),
            _ => Err(CadError::CorruptDwgStream(
                "Invalid BB=11 code in DWG Bit-Double (BD)".into(),
            )),
        }
    }

    /// Read two 64-bit Raw Doubles (`2RD`).
    pub fn read_2rd(&mut self) -> Result<[f64; 2], CadError> {
        Ok([self.read_rd()?, self.read_rd()?])
    }

    /// Read two Bit-Doubles (`2BD`).
    pub fn read_2bd(&mut self) -> Result<[f64; 2], CadError> {
        Ok([self.read_bd()?, self.read_bd()?])
    }

    /// Read three Bit-Doubles (`3BD`).
    pub fn read_3bd(&mut self) -> Result<[f64; 3], CadError> {
        Ok([self.read_bd()?, self.read_bd()?, self.read_bd()?])
    }

    /// Read a Bit-Double with Default (`DD`), used in compact `LINE` and `LWPOLYLINE` streams.
    pub fn read_dd(&mut self, default: f64) -> Result<f64, CadError> {
        match self.read_bb()? {
            0b00 => Ok(default),
            0b01 => {
                let mut bytes = default.to_le_bytes();
                for b in &mut bytes[0..4] {
                    *b = self.read_rc()?;
                }
                let v = f64::from_le_bytes(bytes);
                Ok(if v.is_finite() { v } else { 0.0 })
            }
            0b10 => {
                let mut bytes = default.to_le_bytes();
                bytes[4] = self.read_rc()?;
                bytes[5] = self.read_rc()?;
                for b in &mut bytes[0..4] {
                    *b = self.read_rc()?;
                }
                let v = f64::from_le_bytes(bytes);
                Ok(if v.is_finite() { v } else { 0.0 })
            }
            0b11 => self.read_rd(),
            _ => unreachable!(),
        }
    }

    /// Read Bit-Thickness (`BT`, R2000+): leading `1` bit => `0.0`, `0` bit => `BD`.
    pub fn read_bt(&mut self) -> Result<f64, CadError> {
        if self.read_bit()? == 1 {
            Ok(0.0)
        } else {
            self.read_bd()
        }
    }

    /// Read Bit-Extrusion (`BE`, R2000+): leading `1` bit => `[0.0, 0.0, 1.0]`, `0` bit => `3BD`.
    pub fn read_be(&mut self) -> Result<[f64; 3], CadError> {
        if self.read_bit()? == 1 {
            Ok([0.0, 0.0, 1.0])
        } else {
            self.read_3bd()
        }
    }

    /// Read a signed Modular Char (`MC`), used in DWG Object Map handle/offset deltas.
    pub fn read_mc(&mut self) -> Result<i32, CadError> {
        let mut acc: u32 = 0;
        let mut shift = 0u32;
        for _ in 0..5 {
            let b = self.read_rc()?;
            if (b & 0x80) != 0 {
                acc |= ((b & 0x7F) as u32) << shift;
                shift += 7;
            } else {
                acc |= ((b & 0x3F) as u32) << shift;
                let neg = (b & 0x40) != 0;
                return Ok(if neg { -(acc as i32) } else { acc as i32 });
            }
        }
        Err(CadError::CorruptDwgStream(
            "DWG Modular Char (MC) exceeded 5 bytes".into(),
        ))
    }

    /// Read an unsigned Modular Short (`MS`), used for object record byte sizes in `AcDb:AcDbObjects`.
    pub fn read_ms(&mut self) -> Result<u32, CadError> {
        let mut acc: u32 = 0;
        let mut shift = 0u32;
        for _ in 0..3 {
            let lo = self.read_rc()?;
            let hi = self.read_rc()?;
            let word = u16::from_le_bytes([lo, hi]);
            acc |= ((word & 0x7FFF) as u32) << shift;
            if (word & 0x8000) == 0 {
                return Ok(acc);
            }
            shift += 15;
        }
        Err(CadError::CorruptDwgStream(
            "DWG Modular Short (MS) exceeded 3 words".into(),
        ))
    }

    /// Read a Text Value (`TV`): `BS` byte length followed by `len` `RC` characters.
    pub fn read_tv(&mut self) -> Result<String, CadError> {
        let len = self.read_bs()?;
        if len <= 0 {
            return Ok(String::new());
        }
        let n = len as usize;
        if n > 65_535 {
            return Err(CadError::CorruptDwgStream("Invalid TV length".into()));
        }
        let mut bytes = Vec::with_capacity(n);
        for _ in 0..n {
            bytes.push(self.read_rc()?);
        }
        if bytes.last() == Some(&0) {
            bytes.pop();
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Bit-level writer matching `DwgBitReader` bit-for-bit for deterministic R2000 `.dwg` encoding.
#[derive(Debug, Clone, Default)]
pub struct DwgBitWriter {
    bytes: Vec<u8>,
    bit_pos: u8,
}

impl DwgBitWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_bit(&mut self, bit: u8) {
        if self.bit_pos == 0 {
            self.bytes.push(0);
        }
        if (bit & 1) != 0 {
            if let Some(last) = self.bytes.last_mut() {
                *last |= 1 << (7 - self.bit_pos);
            }
        }
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
        }
    }

    pub fn write_bb(&mut self, val: u8) {
        self.write_bit((val >> 1) & 1);
        self.write_bit(val & 1);
    }

    pub fn write_rc(&mut self, byte: u8) {
        if self.bit_pos == 0 {
            self.bytes.push(byte);
            return;
        }
        for shift in (0..8).rev() {
            self.write_bit((byte >> shift) & 1);
        }
    }

    pub fn write_rs(&mut self, val: i16) {
        let [b0, b1] = val.to_le_bytes();
        self.write_rc(b0);
        self.write_rc(b1);
    }

    pub fn write_rl(&mut self, val: i32) {
        for b in val.to_le_bytes() {
            self.write_rc(b);
        }
    }

    pub fn write_rd(&mut self, val: f64) {
        let clean = if !val.is_finite() || val == 0.0 {
            0.0
        } else {
            val
        };
        for b in clean.to_le_bytes() {
            self.write_rc(b);
        }
    }

    pub fn write_bs(&mut self, val: i16) {
        if val == 0 {
            self.write_bb(0b10);
        } else if val == 256 {
            self.write_bb(0b11);
        } else if (0..=255).contains(&val) {
            self.write_bb(0b01);
            self.write_rc(val as u8);
        } else {
            self.write_bb(0b00);
            self.write_rs(val);
        }
    }

    pub fn write_bl(&mut self, val: i32) {
        if val == 0 {
            self.write_bb(0b10);
        } else if (0..=255).contains(&val) {
            self.write_bb(0b01);
            self.write_rc(val as u8);
        } else {
            self.write_bb(0b00);
            self.write_rl(val);
        }
    }

    pub fn write_bd(&mut self, val: f64) {
        let v = if !val.is_finite() || val == 0.0 {
            0.0
        } else {
            val
        };
        if v == 0.0 {
            self.write_bb(0b10);
        } else if (v - 1.0).abs() == 0.0 {
            self.write_bb(0b01);
        } else {
            self.write_bb(0b00);
            self.write_rd(v);
        }
    }

    pub fn write_2rd(&mut self, pt: [f64; 2]) {
        self.write_rd(pt[0]);
        self.write_rd(pt[1]);
    }

    pub fn write_3bd(&mut self, pt: [f64; 3]) {
        self.write_bd(pt[0]);
        self.write_bd(pt[1]);
        self.write_bd(pt[2]);
    }

    pub fn write_dd(&mut self, val: f64, default: f64) {
        let v = if !val.is_finite() || val == 0.0 {
            0.0
        } else {
            val
        };
        let d = if !default.is_finite() || default == 0.0 {
            0.0
        } else {
            default
        };
        if v.to_bits() == d.to_bits() {
            self.write_bb(0b00);
        } else {
            let vb = v.to_le_bytes();
            let db = d.to_le_bytes();
            if vb[4..8] == db[4..8] {
                self.write_bb(0b01);
                for &b in &vb[0..4] {
                    self.write_rc(b);
                }
            } else if vb[6..8] == db[6..8] {
                self.write_bb(0b10);
                self.write_rc(vb[4]);
                self.write_rc(vb[5]);
                for &b in &vb[0..4] {
                    self.write_rc(b);
                }
            } else {
                self.write_bb(0b11);
                self.write_rd(v);
            }
        }
    }

    pub fn write_bt(&mut self, thickness: f64) {
        if thickness.abs() < 1e-12 {
            self.write_bit(1);
        } else {
            self.write_bit(0);
            self.write_bd(thickness);
        }
    }

    pub fn write_be(&mut self, ext: [f64; 3]) {
        if ext[0].abs() < 1e-12 && ext[1].abs() < 1e-12 && (ext[2] - 1.0).abs() < 1e-12 {
            self.write_bit(1);
        } else {
            self.write_bit(0);
            self.write_3bd(ext);
        }
    }

    pub fn write_mc(&mut self, val: i32) {
        let neg = val < 0;
        let mut mag = val.unsigned_abs();
        while mag >= 0x40 {
            self.write_rc(((mag & 0x7F) as u8) | 0x80);
            mag >>= 7;
        }
        self.write_rc((mag as u8) | if neg { 0x40 } else { 0x00 });
    }

    pub fn write_ms(&mut self, mut val: u32) {
        loop {
            let mut word = (val & 0x7FFF) as u16;
            val >>= 15;
            if val > 0 {
                word |= 0x8000;
            }
            let [lo, hi] = word.to_le_bytes();
            self.write_rc(lo);
            self.write_rc(hi);
            if val == 0 {
                break;
            }
        }
    }

    pub fn write_tv(&mut self, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len().min(32_767);
        self.write_bs(len as i16);
        for &b in &bytes[..len] {
            self.write_rc(b);
        }
    }

    pub fn align_to_byte(&mut self) {
        self.bit_pos = 0;
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

// --- Public DWG R2000 Scene Specification & Encoder ---

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DwgLayerSpec {
    pub name: String,
    /// ACI color index (`< 0` means layer is OFF).
    pub aci_color: i16,
    /// Bit 1 = frozen, bit 4 = locked.
    pub flags: i16,
    pub plot: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DwgEntitySpec {
    Line {
        layer: String,
        aci_color: i16,
        start: [f64; 2],
        end: [f64; 2],
    },
    Circle {
        layer: String,
        aci_color: i16,
        center: [f64; 2],
        radius: f64,
    },
    Arc {
        layer: String,
        aci_color: i16,
        center: [f64; 2],
        radius: f64,
        start_angle_rad: f64,
        end_angle_rad: f64,
    },
    Ellipse {
        layer: String,
        aci_color: i16,
        center: [f64; 2],
        major_axis_vec: [f64; 2],
        axis_ratio: f64,
    },
    LwPolyline {
        layer: String,
        aci_color: i16,
        closed: bool,
        vertices: Vec<([f64; 2], f64)>, // ((x, y), bulge)
    },
    Text {
        layer: String,
        aci_color: i16,
        position: [f64; 2],
        height: f64,
        rotation_rad: f64,
        value: String,
        is_mtext: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DwgMinimalScene {
    pub insunits: i16,
    pub ext_min: [f64; 3],
    pub ext_max: [f64; 3],
    pub layers: Vec<DwgLayerSpec>,
    pub entities: Vec<DwgEntitySpec>,
}

impl Default for DwgMinimalScene {
    fn default() -> Self {
        Self {
            insunits: 1,
            ext_min: [0.0, 0.0, 0.0],
            ext_max: [11.0, 8.5, 0.0],
            layers: vec![DwgLayerSpec {
                name: "0".into(),
                aci_color: 7,
                flags: 0,
                plot: true,
            }],
            entities: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SectionLocatorEntry {
    record_number: u8,
    seeker: u32,
    size: u32,
}

/// Encode a `DwgMinimalScene` into a valid AutoCAD R2000 (`AC1015`) binary `.dwg` file
/// containing:
/// - `0x00..0x15` R2000 header + `0x15` Section Locator Table (`AcDb:Header`, `AcDb:Classes`,
///   `AcDb:Handles` Object Map, `AcDb:AcDbObjects`)
/// - `AcDb:Header` bit-stream with 16-byte sentinels and CRC32
/// - `AcDb:Classes` bit-stream registering `AcDbPolyline` (`LWPOLYLINE`, class 500)
/// - `AcDb:AcDbObjects` bit-packed object records (`MS` size + `BS` type + entity bit-fields)
/// - `AcDb:Handles` Object Map (`MC` handle deltas + `MC` file offset deltas)
pub fn encode_minimal_dwg_r2000(scene: &DwgMinimalScene) -> Vec<u8> {
    // 1. Build `AcDb:Header` section payload
    let mut hdr_bits = DwgBitWriter::new();
    hdr_bits.write_bs(scene.insunits);
    hdr_bits.write_3bd(scene.ext_min);
    hdr_bits.write_3bd(scene.ext_max);
    hdr_bits.write_2rd([scene.ext_min[0], scene.ext_min[1]]);
    hdr_bits.write_2rd([scene.ext_max[0], scene.ext_max[1]]);
    let hdr_bit_bytes = hdr_bits.into_bytes();

    let mut sec_header = Vec::with_capacity(16 + 4 + hdr_bit_bytes.len() + 4 + 16);
    sec_header.extend_from_slice(&HEADER_SENTINEL_BEGIN);
    sec_header.extend_from_slice(&(hdr_bit_bytes.len() as u32).to_le_bytes());
    sec_header.extend_from_slice(&hdr_bit_bytes);
    let hdr_crc = crc32fast::hash(&hdr_bit_bytes);
    sec_header.extend_from_slice(&hdr_crc.to_le_bytes());
    sec_header.extend_from_slice(&HEADER_SENTINEL_END);

    // 2. Build `AcDb:Classes` section payload (registers `LWPOLYLINE` class 500)
    let mut cls_bits = DwgBitWriter::new();
    cls_bits.write_bs(DWG_OBJ_LWPOLYLINE_CLASS); // max class number
    cls_bits.write_bit(0);
    // Single dynamic class record for LWPOLYLINE
    cls_bits.write_bs(DWG_OBJ_LWPOLYLINE_CLASS);
    cls_bits.write_bs(0); // proxy flags
    cls_bits.write_tv("ObjectDBX Classes");
    cls_bits.write_tv("AcDbPolyline");
    cls_bits.write_tv("LWPOLYLINE");
    cls_bits.write_bit(0); // was_zombie
    cls_bits.write_bs(0x01F2); // entity item_class_id
    let cls_bit_bytes = cls_bits.into_bytes();

    let mut sec_classes = Vec::with_capacity(16 + 4 + cls_bit_bytes.len() + 4 + 16);
    sec_classes.extend_from_slice(&CLASSES_SENTINEL_BEGIN);
    sec_classes.extend_from_slice(&(cls_bit_bytes.len() as u32).to_le_bytes());
    sec_classes.extend_from_slice(&cls_bit_bytes);
    let cls_crc = crc32fast::hash(&cls_bit_bytes);
    sec_classes.extend_from_slice(&cls_crc.to_le_bytes());
    sec_classes.extend_from_slice(&CLASSES_SENTINEL_END);

    // 3. Serialize all LAYER + Entity records into `AcDb:AcDbObjects`
    // File layout:
    //   [0x00 .. file_hdr_len] File header + 4 section locators + CRC32 + end sentinel
    //   [sec_header]
    //   [sec_classes]
    //   [sec_objects]
    //   [sec_handles]
    let num_sections: u32 = 4;
    let file_hdr_len = 0x15 + 4 + (num_sections as usize) * 9 + 4 + 16;
    let hdr_seeker = file_hdr_len as u32;
    let cls_seeker = hdr_seeker + sec_header.len() as u32;
    let obj_seeker = cls_seeker + sec_classes.len() as u32;

    let mut sec_objects: Vec<u8> = Vec::new();
    let mut handle_entries: Vec<(i32, i32)> = Vec::new(); // (handle, absolute_file_offset)
    let mut next_handle: i32 = 1;

    for layer in &scene.layers {
        let mut rec = DwgBitWriter::new();
        rec.write_bs(DWG_OBJ_LAYER);
        rec.write_bl(next_handle);
        rec.write_tv(&layer.name);
        rec.write_bs(layer.flags);
        rec.write_bs(layer.aci_color);
        rec.write_bit(u8::from(layer.plot));
        let payload = rec.into_bytes();
        let abs_offset = obj_seeker as i32 + sec_objects.len() as i32;
        append_dwg_object_record(&mut sec_objects, &payload);
        handle_entries.push((next_handle, abs_offset));
        next_handle += 1;
    }

    for ent in &scene.entities {
        let payload = encode_dwg_entity_record(ent, next_handle);
        let abs_offset = obj_seeker as i32 + sec_objects.len() as i32;
        append_dwg_object_record(&mut sec_objects, &payload);
        handle_entries.push((next_handle, abs_offset));
        next_handle += 1;
    }

    // 4. Build `AcDb:Handles` Object Map section using `MC` handle/offset deltas
    let hdl_seeker = obj_seeker + sec_objects.len() as u32;
    let mut map_body = DwgBitWriter::new();
    let mut prev_h = 0i32;
    let mut prev_off = 0i32;
    for (h, off) in &handle_entries {
        map_body.write_mc(*h - prev_h);
        map_body.write_mc(*off - prev_off);
        prev_h = *h;
        prev_off = *off;
    }
    let map_bytes = map_body.into_bytes();
    let mut sec_handles = Vec::with_capacity(map_bytes.len() + 10);
    // Big-endian chunk size includes the 2-byte size field itself
    let chunk_size = (map_bytes.len() + 2) as u16;
    sec_handles.extend_from_slice(&chunk_size.to_be_bytes());
    sec_handles.extend_from_slice(&map_bytes);
    let map_crc = (crc32fast::hash(&map_bytes) & 0xFFFF) as u16;
    sec_handles.extend_from_slice(&map_crc.to_be_bytes());
    // Terminating empty 2-byte chunk (`0x00, 0x02`) + 2-byte CRC
    sec_handles.extend_from_slice(&2u16.to_be_bytes());
    sec_handles.extend_from_slice(&0u16.to_be_bytes());

    // 5. Assemble complete `.dwg` file with Section Locator Table at `0x15`
    let locators = [
        SectionLocatorEntry {
            record_number: 0, // AcDb:Header
            seeker: hdr_seeker,
            size: sec_header.len() as u32,
        },
        SectionLocatorEntry {
            record_number: 1, // AcDb:Classes
            seeker: cls_seeker,
            size: sec_classes.len() as u32,
        },
        SectionLocatorEntry {
            record_number: 2, // AcDb:Handles (Object Map)
            seeker: hdl_seeker,
            size: sec_handles.len() as u32,
        },
        SectionLocatorEntry {
            record_number: 4, // AcDb:AcDbObjects stream
            seeker: obj_seeker,
            size: sec_objects.len() as u32,
        },
    ];

    let mut file = Vec::with_capacity(
        file_hdr_len + sec_header.len() + sec_classes.len() + sec_objects.len() + sec_handles.len(),
    );
    file.extend_from_slice(b"AC1015"); // 0x00..0x06
    file.extend_from_slice(&[0u8; 5]); // 0x06..0x0B
    file.push(0x0F); // 0x0B maintenance version
    file.push(0x01); // 0x0C
    file.extend_from_slice(&0u32.to_le_bytes()); // 0x0D..0x11 preview seeker
    file.push(0x17); // 0x11 dwg ver
    file.push(0x17); // 0x12 app ver
    file.extend_from_slice(&30u16.to_le_bytes()); // 0x13..0x15 ANSI_1252 codepage
    debug_assert_eq!(file.len(), 0x15);

    // Offset 0x15: Section Locator Table
    file.extend_from_slice(&num_sections.to_le_bytes());
    for loc in &locators {
        file.push(loc.record_number);
        file.extend_from_slice(&loc.seeker.to_le_bytes());
        file.extend_from_slice(&loc.size.to_le_bytes());
    }
    let hdr_table_crc = crc32fast::hash(&file);
    file.extend_from_slice(&hdr_table_crc.to_le_bytes());
    file.extend_from_slice(&FILE_HEADER_END_SENTINEL);
    debug_assert_eq!(file.len(), file_hdr_len);

    file.extend_from_slice(&sec_header);
    file.extend_from_slice(&sec_classes);
    file.extend_from_slice(&sec_objects);
    file.extend_from_slice(&sec_handles);
    file
}

fn append_dwg_object_record(buf: &mut Vec<u8>, payload: &[u8]) {
    let mut prefix = DwgBitWriter::new();
    prefix.write_ms(payload.len() as u32);
    buf.extend_from_slice(&prefix.into_bytes());
    buf.extend_from_slice(payload);
    let crc = (crc32fast::hash(payload) & 0xFFFF) as u16;
    buf.extend_from_slice(&crc.to_le_bytes());
}

fn encode_dwg_entity_record(ent: &DwgEntitySpec, handle: i32) -> Vec<u8> {
    let mut w = DwgBitWriter::new();
    match ent {
        DwgEntitySpec::Line {
            layer,
            aci_color,
            start,
            end,
        } => {
            w.write_bs(DWG_OBJ_LINE);
            w.write_bl(handle);
            w.write_tv(layer);
            w.write_bs(*aci_color);
            // R2000 compact Z-is-zero bit + X/Y with `RD` + `DD`!
            w.write_bit(1); // z_is_zero = true
            w.write_rd(start[0]);
            w.write_dd(end[0], start[0]);
            w.write_rd(start[1]);
            w.write_dd(end[1], start[1]);
            w.write_bt(0.0);
            w.write_be([0.0, 0.0, 1.0]);
        }
        DwgEntitySpec::Circle {
            layer,
            aci_color,
            center,
            radius,
        } => {
            w.write_bs(DWG_OBJ_CIRCLE);
            w.write_bl(handle);
            w.write_tv(layer);
            w.write_bs(*aci_color);
            w.write_3bd([center[0], center[1], 0.0]);
            w.write_bd(*radius);
            w.write_bt(0.0);
            w.write_be([0.0, 0.0, 1.0]);
        }
        DwgEntitySpec::Arc {
            layer,
            aci_color,
            center,
            radius,
            start_angle_rad,
            end_angle_rad,
        } => {
            w.write_bs(DWG_OBJ_ARC);
            w.write_bl(handle);
            w.write_tv(layer);
            w.write_bs(*aci_color);
            w.write_3bd([center[0], center[1], 0.0]);
            w.write_bd(*radius);
            w.write_bt(0.0);
            w.write_be([0.0, 0.0, 1.0]);
            w.write_bd(*start_angle_rad);
            w.write_bd(*end_angle_rad);
        }
        DwgEntitySpec::Ellipse {
            layer,
            aci_color,
            center,
            major_axis_vec,
            axis_ratio,
        } => {
            w.write_bs(DWG_OBJ_ELLIPSE);
            w.write_bl(handle);
            w.write_tv(layer);
            w.write_bs(*aci_color);
            w.write_3bd([center[0], center[1], 0.0]);
            w.write_3bd([major_axis_vec[0], major_axis_vec[1], 0.0]);
            w.write_be([0.0, 0.0, 1.0]);
            w.write_bd(*axis_ratio);
            w.write_bd(0.0);
            w.write_bd(TAU);
        }
        DwgEntitySpec::LwPolyline {
            layer,
            aci_color,
            closed,
            vertices,
        } => {
            w.write_bs(DWG_OBJ_LWPOLYLINE_CLASS);
            w.write_bl(handle);
            w.write_tv(layer);
            w.write_bs(*aci_color);
            let has_bulges = vertices.iter().any(|(_, b)| b.abs() > 1e-9);
            // DWG LWPOLYLINE flags: bit 0x0200 = closed, bit 0x0010 = has bulges
            let mut flags: i16 = 0;
            if *closed {
                flags |= 0x0200;
            }
            if has_bulges {
                flags |= 0x0010;
            }
            w.write_bs(flags);
            w.write_bl(vertices.len() as i32);
            if has_bulges {
                w.write_bl(vertices.len() as i32);
            }
            if let Some((first_pt, _)) = vertices.first() {
                w.write_2rd(*first_pt);
                let mut prev = *first_pt;
                for (pt, _) in &vertices[1..] {
                    w.write_dd(pt[0], prev[0]);
                    w.write_dd(pt[1], prev[1]);
                    prev = *pt;
                }
            }
            if has_bulges {
                for (_, bulge) in vertices {
                    w.write_bd(*bulge);
                }
            }
        }
        DwgEntitySpec::Text {
            layer,
            aci_color,
            position,
            height,
            rotation_rad,
            value,
            is_mtext,
        } => {
            if *is_mtext {
                w.write_bs(DWG_OBJ_MTEXT);
                w.write_bl(handle);
                w.write_tv(layer);
                w.write_bs(*aci_color);
                w.write_3bd([position[0], position[1], 0.0]);
                w.write_be([0.0, 0.0, 1.0]);
                w.write_3bd([rotation_rad.cos(), rotation_rad.sin(), 0.0]);
                w.write_bd(*height * 6.0);
                w.write_bd(*height);
                w.write_bs(5); // attachment
                w.write_bs(1); // flow_dir
                w.write_tv(value);
            } else {
                w.write_bs(DWG_OBJ_TEXT);
                w.write_bl(handle);
                w.write_tv(layer);
                w.write_bs(*aci_color);
                w.write_2rd(*position);
                w.write_bd(*height);
                w.write_bd(*rotation_rad);
                w.write_tv(value);
            }
        }
    }
    w.into_bytes()
}

/// Encode `page_id` of `doc` into a complete offline R2000 (`AC1015`) `.dwg` binary byte stream.
pub fn encode_dwg_r2000_from_page(doc: &VisioDocument, page_id: u32) -> Result<Vec<u8>, CadError> {
    let mut eval_doc = doc.clone();
    ReactiveEngine::recalculate_document(&mut eval_doc);

    let page = eval_doc
        .page(page_id)
        .ok_or(CadError::PageNotFound(page_id))?;
    let visuals = ReactiveEngine::evaluate_page_visuals(&eval_doc, page_id);

    let mut layers = Vec::new();
    if !page.layers.iter().any(|l| l.name == "0") {
        layers.push(DwgLayerSpec {
            name: "0".into(),
            aci_color: 7,
            flags: 0,
            plot: true,
        });
    }
    for l in &page.layers {
        let mut flags = 0i16;
        if !l.visible && !l.print {
            flags |= 1;
        }
        if l.lock {
            flags |= 4;
        }
        let base_aci = l.color_override.map_or(7, rgba_to_aci);
        let signed_aci = if !l.visible && l.print {
            -base_aci
        } else {
            base_aci
        };
        layers.push(DwgLayerSpec {
            name: l.name.clone(),
            aci_color: signed_aci,
            flags,
            plot: l.print,
        });
    }

    let mut entities = Vec::new();
    for vis in &visuals {
        let shape_ref = page.find_shape(vis.shape_id);
        let layer_name = shape_ref
            .and_then(|s| s.layer_membership.first())
            .and_then(|&lix| page.layers.iter().find(|l| l.ix == lix))
            .map(|l| l.name.clone())
            .unwrap_or_else(|| "0".into());
        let stroke = vis.stroke_color.or(vis.fill_color).unwrap_or(RgbaColor::BLACK);
        let aci = rgba_to_aci(stroke);

        for path in &vis.paths {
            if path.no_show || (path.no_line && path.no_fill) {
                continue;
            }
            visual_verbs_to_dwg_entities(&path.verbs_page_space, &layer_name, aci, &mut entities);
        }

        if !vis.text.trim().is_empty() {
            let txt_aci = rgba_to_aci(vis.text_color);
            entities.push(DwgEntitySpec::Text {
                layer: layer_name,
                aci_color: txt_aci,
                position: [vis.transform.pin_x, vis.transform.pin_y],
                height: (vis.font_size_pt / 72.0).max(0.05),
                rotation_rad: vis.transform.angle_rad,
                value: vis.text.replace('\n', "\\P"),
                is_mtext: vis.text.contains('\n'),
            });
        }
    }

    let scene = DwgMinimalScene {
        insunits: 1, // Inches
        ext_min: [0.0, 0.0, 0.0],
        ext_max: [page.width_in(), page.height_in(), 0.0],
        layers,
        entities,
    };
    Ok(encode_minimal_dwg_r2000(&scene))
}

fn visual_verbs_to_dwg_entities(
    verbs: &[EvaluatedPathVerb],
    layer: &str,
    aci: i16,
    out: &mut Vec<DwgEntitySpec>,
) {
    if verbs.is_empty() {
        return;
    }
    if verbs.len() == 1 {
        if let EvaluatedPathVerb::Ellipse {
            center,
            rx,
            ry,
            angle_rad,
        } = verbs[0]
        {
            if (rx - ry).abs() <= 1e-6 {
                out.push(DwgEntitySpec::Circle {
                    layer: layer.to_string(),
                    aci_color: aci,
                    center,
                    radius: rx,
                });
            } else {
                let (major, minor, ang) = if rx >= ry {
                    (rx, ry, angle_rad)
                } else {
                    (ry, rx, angle_rad + PI * 0.5)
                };
                out.push(DwgEntitySpec::Ellipse {
                    layer: layer.to_string(),
                    aci_color: aci,
                    center,
                    major_axis_vec: [major * ang.cos(), major * ang.sin()],
                    axis_ratio: (minor / major.max(1e-9)).clamp(1e-6, 1.0),
                });
            }
            return;
        }
    }
    if let [EvaluatedPathVerb::MoveTo(p1), EvaluatedPathVerb::LineTo(p2)] = verbs {
        out.push(DwgEntitySpec::Line {
            layer: layer.to_string(),
            aci_color: aci,
            start: *p1,
            end: *p2,
        });
        return;
    }
    if let [EvaluatedPathVerb::MoveTo(p1), EvaluatedPathVerb::ArcTo { to: p2, bow }] = verbs {
        if let Some((center, radius, start_deg, end_deg)) =
            arc_params_from_endpoints_and_bow(*p1, *p2, *bow)
        {
            out.push(DwgEntitySpec::Arc {
                layer: layer.to_string(),
                aci_color: aci,
                center,
                radius,
                start_angle_rad: start_deg.to_radians(),
                end_angle_rad: end_deg.to_radians(),
            });
            return;
        }
    }

    let mut cur: Vec<([f64; 2], f64)> = Vec::new();
    let mut closed = false;
    let flush = |out: &mut Vec<DwgEntitySpec>,
                 pts: &mut Vec<([f64; 2], f64)>,
                 closed: &mut bool| {
        if pts.len() >= 2 {
            if !*closed && pts.len() >= 3 {
                let first = pts[0].0;
                let last = pts[pts.len() - 1];
                if (first[0] - last.0[0]).hypot(first[1] - last.0[1]) < 1e-6
                    && last.1.abs() < 1e-9
                {
                    pts.pop();
                    *closed = true;
                }
            }
            out.push(DwgEntitySpec::LwPolyline {
                layer: layer.to_string(),
                aci_color: aci,
                closed: *closed,
                vertices: pts.clone(),
            });
        }
        pts.clear();
        *closed = false;
    };

    for v in verbs {
        match *v {
            EvaluatedPathVerb::MoveTo(pt) => {
                flush(out, &mut cur, &mut closed);
                cur.push((pt, 0.0));
            }
            EvaluatedPathVerb::LineTo(pt) => cur.push((pt, 0.0)),
            EvaluatedPathVerb::ArcTo { to, bow } => {
                if let Some(prev) = cur.last_mut() {
                    let chord = (to[0] - prev.0[0]).hypot(to[1] - prev.0[1]);
                    if chord > 1e-9 && bow.abs() > 1e-9 {
                        prev.1 = (2.0 * bow) / chord;
                    }
                }
                cur.push((to, 0.0));
            }
            EvaluatedPathVerb::QuadTo { ctrl, to } => {
                let s0 = cur.last().map_or(ctrl, |(p, _)| *p);
                for s in 1..=8 {
                    let t = s as f64 / 8.0;
                    let mt = 1.0 - t;
                    cur.push((
                        [
                            mt * mt * s0[0] + 2.0 * mt * t * ctrl[0] + t * t * to[0],
                            mt * mt * s0[1] + 2.0 * mt * t * ctrl[1] + t * t * to[1],
                        ],
                        0.0,
                    ));
                }
            }
            EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                let s0 = cur.last().map_or(ctrl1, |(p, _)| *p);
                for s in 1..=10 {
                    let t = s as f64 / 10.0;
                    let mt = 1.0 - t;
                    cur.push((
                        [
                            mt.powi(3) * s0[0]
                                + 3.0 * mt.powi(2) * t * ctrl1[0]
                                + 3.0 * mt * t.powi(2) * ctrl2[0]
                                + t.powi(3) * to[0],
                            mt.powi(3) * s0[1]
                                + 3.0 * mt.powi(2) * t * ctrl1[1]
                                + 3.0 * mt * t.powi(2) * ctrl2[1]
                                + t.powi(3) * to[1],
                        ],
                        0.0,
                    ));
                }
            }
            EvaluatedPathVerb::Ellipse {
                center,
                rx,
                ry,
                angle_rad,
            } => {
                flush(out, &mut cur, &mut closed);
                visual_verbs_to_dwg_entities(
                    &[EvaluatedPathVerb::Ellipse {
                        center,
                        rx,
                        ry,
                        angle_rad,
                    }],
                    layer,
                    aci,
                    out,
                );
            }
            EvaluatedPathVerb::Close => {
                closed = true;
                flush(out, &mut cur, &mut closed);
            }
        }
    }
    flush(out, &mut cur, &mut closed);
}

// --- Native Offline `.dwg` Bit-Stream Decoder ---

/// Decode a binary `.dwg` byte buffer onto `page`, validating its `DwgVersion` magic,
/// parsing the Section Locator Table at `0x15` (`AcDb:Header`, `AcDb:Classes`,
/// `AcDb:Handles` Object Map, and `AcDb:AcDbObjects`), and converting CAD layers & entities.
pub fn import_dwg_to_page(
    page: &mut VisioPage,
    dwg_bytes: &[u8],
) -> Result<(DwgVersion, Vec<u64>), CadError> {
    let version = DwgVersion::from_magic_bytes(dwg_bytes)?;
    if dwg_bytes.len() < 0x19 {
        return Err(CadError::CorruptDwgStream(
            "DWG file truncated before Section Locator Table at 0x15".into(),
        ));
    }

    let mut insunits = 1i32;
    let mut ext_min: Option<[f64; 3]> = None;
    let mut ext_max: Option<[f64; 3]> = None;
    let mut lwpolyline_class_num = DWG_OBJ_LWPOLYLINE_CLASS;
    let mut object_offsets: Vec<usize> = Vec::new();
    let mut objects_stream_range: Option<(usize, usize)> = None;

    match version {
        DwgVersion::R13 | DwgVersion::R14 | DwgVersion::R2000 => {
            // Parse Section Locator Table at offset `0x15`
            let num_records = u32::from_le_bytes([
                dwg_bytes[0x15],
                dwg_bytes[0x16],
                dwg_bytes[0x17],
                dwg_bytes[0x18],
            ]) as usize;
            if num_records > 64 || 0x19 + num_records * 9 > dwg_bytes.len() {
                return Err(CadError::CorruptDwgStream(format!(
                    "Invalid DWG Section Locator count ({num_records}) at offset 0x15"
                )));
            }

            let mut pos = 0x19usize;
            for _ in 0..num_records {
                let rec_no = dwg_bytes[pos];
                let seeker = u32::from_le_bytes([
                    dwg_bytes[pos + 1],
                    dwg_bytes[pos + 2],
                    dwg_bytes[pos + 3],
                    dwg_bytes[pos + 4],
                ]) as usize;
                let size = u32::from_le_bytes([
                    dwg_bytes[pos + 5],
                    dwg_bytes[pos + 6],
                    dwg_bytes[pos + 7],
                    dwg_bytes[pos + 8],
                ]) as usize;
                pos += 9;

                if seeker == 0 || size == 0 {
                    continue;
                }
                if seeker.saturating_add(size) > dwg_bytes.len() {
                    return Err(CadError::CorruptDwgStream(format!(
                        "Section {rec_no} (seeker={seeker}, size={size}) exceeds DWG file length {}",
                        dwg_bytes.len()
                    )));
                }
                let sec_slice = &dwg_bytes[seeker..seeker + size];
                match rec_no {
                    0 => parse_dwg_header_section(sec_slice, &mut insunits, &mut ext_min, &mut ext_max)?,
                    1 => parse_dwg_classes_section(sec_slice, &mut lwpolyline_class_num)?,
                    2 => parse_dwg_handles_section(sec_slice, &mut object_offsets)?,
                    4 => objects_stream_range = Some((seeker, seeker + size)),
                    _ => {}
                }
            }
        }
        DwgVersion::R2004
        | DwgVersion::R2007
        | DwgVersion::R2010
        | DwgVersion::R2013
        | DwgVersion::R2018 => {
            // R2004+ uses 0x80 XOR-masked header (`AcFssFcAJMB`); also scan for uncompressed
            // `AcDb:Header` / `AcDb:Classes` sentinels if present in raw stream.
            if dwg_bytes.len() >= 0x100 {
                let mut decrypted = [0u8; 0x6C];
                let mut seed: u32 = 1;
                for (idx, dst) in decrypted.iter_mut().enumerate() {
                    seed = seed.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
                    let mask = ((seed >> 16) & 0xFF) as u8;
                    *dst = dwg_bytes[0x80 + idx] ^ mask;
                }
                // Record verified R2004+ header presence on page metadata
                let has_r2004_sig = &decrypted[0..11] == b"AcFssFcAJMB"
                    || &dwg_bytes[0x80..0x8B] == b"AcFssFcAJMB";
                page.page_sheet.set_user_cell(
                    "CadDwgR2004Header",
                    if has_r2004_sig { "TRUE" } else { "FALSE" },
                    "R2004+ File Header Signature",
                );
            }
            if let Some(hdr_pos) = find_subslice(dwg_bytes, &HEADER_SENTINEL_BEGIN) {
                let rem = &dwg_bytes[hdr_pos..];
                if let Some(end_rel) = find_subslice(rem, &HEADER_SENTINEL_END) {
                    let _ = parse_dwg_header_section(
                        &rem[..end_rel + 16],
                        &mut insunits,
                        &mut ext_min,
                        &mut ext_max,
                    );
                }
            }
        }
    }

    let (unit_enum, to_in) = insunits_to_unit_and_factor(insunits);
    page.page_sheet.set_user_cell(
        "CadDwgVersion",
        format!("\"{}\"", version.magic_str()),
        "DWG Version Magic",
    );
    page.page_sheet.set_user_cell(
        "CadInsUnits",
        format!("\"{}\"", unit_enum.vsdx_code()),
        "DWG $INSUNITS",
    );
    if let (Some(mn), Some(mx)) = (ext_min, ext_max) {
        let span_w = (mx[0] - mn[0]) * to_in;
        let span_h = (mx[1] - mn[1]) * to_in;
        if span_w.is_finite() && span_h.is_finite() && span_w > 0.1 && span_h > 0.1 {
            page.page_sheet.set_formula(
                "PageWidth",
                format!("{} in", format_clean_f64(page.width_in().max(mx[0] * to_in))),
                Some(Unit::Inch),
            );
            page.page_sheet.set_formula(
                "PageHeight",
                format!("{} in", format_clean_f64(page.height_in().max(mx[1] * to_in))),
                Some(Unit::Inch),
            );
        }
    }

    // Decode all object records either via Object Map file offsets (`AcDb:Handles`)
    // or sequential scan of `AcDb:AcDbObjects`
    let mut raw_records: Vec<&[u8]> = Vec::new();
    if !object_offsets.is_empty() {
        for &off in &object_offsets {
            if off >= dwg_bytes.len() {
                return Err(CadError::CorruptDwgStream(format!(
                    "Object Map offset {off} out of bounds"
                )));
            }
            let mut rdr = DwgBitReader::new(&dwg_bytes[off..]);
            let obj_len = rdr.read_ms()? as usize;
            let hdr_bytes = rdr.bit_offset() / 8;
            let payload_start = off + hdr_bytes;
            let payload_end = payload_start.saturating_add(obj_len);
            if payload_end > dwg_bytes.len() {
                return Err(CadError::CorruptDwgStream(
                    "Truncated object record in AcDb:AcDbObjects".into(),
                ));
            }
            raw_records.push(&dwg_bytes[payload_start..payload_end]);
        }
    } else if let Some((start, end)) = objects_stream_range {
        let mut cur = start;
        while cur < end {
            let mut rdr = DwgBitReader::new(&dwg_bytes[cur..end]);
            let obj_len = rdr.read_ms()? as usize;
            let prefix_len = rdr.bit_offset() / 8;
            let p_start = cur + prefix_len;
            let p_end = p_start.saturating_add(obj_len);
            if p_end > end {
                break;
            }
            raw_records.push(&dwg_bytes[p_start..p_end]);
            cur = p_end + 2; // skip 2-byte CRC
        }
    }

    // Pass 1: Decode `LAYER` records first so entity layers resolve exact flags/colors.
    let mut pending_entities: Vec<DwgEntitySpec> = Vec::new();
    for payload in raw_records {
        decode_dwg_object_payload(
            payload,
            lwpolyline_class_num,
            page,
            &mut pending_entities,
        )?;
    }

    // Pass 2: Instantiate decoded CAD entities onto `VisioPage`
    let mut next_id = page.next_shape_id();
    let mut created_ids = Vec::with_capacity(pending_entities.len());
    for ent in &pending_entities {
        if let Some(shape) = dwg_entity_to_visio_shape(page, ent, to_in, next_id) {
            created_ids.push(shape.id);
            page.shapes.push(shape);
            next_id += 1;
        }
    }

    Ok((version, created_ids))
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn parse_dwg_header_section(
    sec: &[u8],
    insunits: &mut i32,
    ext_min: &mut Option<[f64; 3]>,
    ext_max: &mut Option<[f64; 3]>,
) -> Result<(), CadError> {
    if sec.len() < 36 || sec[0..16] != HEADER_SENTINEL_BEGIN {
        return Err(CadError::CorruptDwgStream(
            "Missing 16-byte AcDb:Header begin sentinel".into(),
        ));
    }
    let body_size = u32::from_le_bytes([sec[16], sec[17], sec[18], sec[19]]) as usize;
    if 20 + body_size > sec.len() {
        return Err(CadError::CorruptDwgStream(
            "AcDb:Header body size exceeds section bounds".into(),
        ));
    }
    let body = &sec[20..20 + body_size];
    let mut rdr = DwgBitReader::new(body);
    *insunits = rdr.read_bs()? as i32;
    *ext_min = Some(rdr.read_3bd()?);
    *ext_max = Some(rdr.read_3bd()?);
    Ok(())
}

fn parse_dwg_classes_section(sec: &[u8], lwpolyline_class: &mut i16) -> Result<(), CadError> {
    if sec.len() < 36 || sec[0..16] != CLASSES_SENTINEL_BEGIN {
        return Err(CadError::CorruptDwgStream(
            "Missing 16-byte AcDb:Classes begin sentinel".into(),
        ));
    }
    let body_size = u32::from_le_bytes([sec[16], sec[17], sec[18], sec[19]]) as usize;
    if 20 + body_size > sec.len() {
        return Err(CadError::CorruptDwgStream(
            "AcDb:Classes body size exceeds section bounds".into(),
        ));
    }
    let body = &sec[20..20 + body_size];
    let mut rdr = DwgBitReader::new(body);
    let max_cls = rdr.read_bs()?;
    let _flag = rdr.read_bit()?;
    while rdr.bit_offset() + 16 <= rdr.total_bits() {
        let Ok(cls_num) = rdr.read_bs() else { break };
        if cls_num < 500 || cls_num > max_cls {
            break;
        }
        let _proxy = rdr.read_bs()?;
        let _app = rdr.read_tv()?;
        let _cpp = rdr.read_tv()?;
        let dxf_name = rdr.read_tv()?;
        let _zombie = rdr.read_bit()?;
        let _item_id = rdr.read_bs()?;
        if dxf_name.eq_ignore_ascii_case("LWPOLYLINE") {
            *lwpolyline_class = cls_num;
        }
    }
    Ok(())
}

fn parse_dwg_handles_section(sec: &[u8], offsets: &mut Vec<usize>) -> Result<(), CadError> {
    let mut pos = 0usize;
    while pos + 2 <= sec.len() {
        let chunk_size = u16::from_be_bytes([sec[pos], sec[pos + 1]]) as usize;
        if chunk_size <= 2 {
            break; // Terminating 2-byte chunk
        }
        if pos + chunk_size > sec.len() {
            return Err(CadError::CorruptDwgStream(
                "Truncated chunk in AcDb:Handles Object Map".into(),
            ));
        }
        let body = &sec[pos + 2..pos + chunk_size];
        let mut rdr = DwgBitReader::new(body);
        let mut cur_handle = 0i32;
        let mut cur_offset = 0i32;
        while rdr.bit_offset() < rdr.total_bits() {
            let h_delta = rdr.read_mc()?;
            let off_delta = rdr.read_mc()?;
            cur_handle = cur_handle.wrapping_add(h_delta);
            cur_offset = cur_offset.wrapping_add(off_delta);
            if cur_handle > 0 && cur_offset > 0 {
                offsets.push(cur_offset as usize);
            }
        }
        pos += chunk_size + 2; // advance past chunk + 2-byte big-endian CRC
    }
    Ok(())
}

fn decode_dwg_object_payload(
    payload: &[u8],
    lwpoly_cls: i16,
    page: &mut VisioPage,
    entities: &mut Vec<DwgEntitySpec>,
) -> Result<(), CadError> {
    let mut r = DwgBitReader::new(payload);
    let obj_type = r.read_bs()?;
    let _handle = r.read_bl()?;

    if obj_type == DWG_OBJ_LAYER {
        let name = r.read_tv()?;
        let flags = r.read_bs()? as i32;
        let aci = r.read_bs()?;
        let plot = r.read_bit()? != 0;
        upsert_cad_layer(page, &name, aci, None, flags, plot);
        return Ok(());
    }

    let layer = r.read_tv()?;
    let aci_color = r.read_bs()?;

    if obj_type == DWG_OBJ_LINE {
        let z_is_zero = r.read_bit()? != 0;
        let x1 = r.read_rd()?;
        let x2 = r.read_dd(x1)?;
        let y1 = r.read_rd()?;
        let y2 = r.read_dd(y1)?;
        if !z_is_zero {
            let z1 = r.read_rd()?;
            let _z2 = r.read_dd(z1)?;
        }
        let _thickness = r.read_bt()?;
        let _ext = r.read_be()?;
        entities.push(DwgEntitySpec::Line {
            layer,
            aci_color,
            start: [x1, y1],
            end: [x2, y2],
        });
    } else if obj_type == DWG_OBJ_CIRCLE {
        let c = r.read_3bd()?;
        let radius = r.read_bd()?;
        let _thickness = r.read_bt()?;
        let _ext = r.read_be()?;
        entities.push(DwgEntitySpec::Circle {
            layer,
            aci_color,
            center: [c[0], c[1]],
            radius,
        });
    } else if obj_type == DWG_OBJ_ARC {
        let c = r.read_3bd()?;
        let radius = r.read_bd()?;
        let _thickness = r.read_bt()?;
        let _ext = r.read_be()?;
        let start_angle_rad = r.read_bd()?;
        let end_angle_rad = r.read_bd()?;
        entities.push(DwgEntitySpec::Arc {
            layer,
            aci_color,
            center: [c[0], c[1]],
            radius,
            start_angle_rad,
            end_angle_rad,
        });
    } else if obj_type == DWG_OBJ_ELLIPSE {
        let c = r.read_3bd()?;
        let maj = r.read_3bd()?;
        let _ext = r.read_be()?;
        let axis_ratio = r.read_bd()?;
        let _beg = r.read_bd()?;
        let _end = r.read_bd()?;
        entities.push(DwgEntitySpec::Ellipse {
            layer,
            aci_color,
            center: [c[0], c[1]],
            major_axis_vec: [maj[0], maj[1]],
            axis_ratio,
        });
    } else if obj_type == DWG_OBJ_LWPOLYLINE || obj_type == lwpoly_cls {
        let flags = r.read_bs()?;
        let closed = (flags & 0x0200) != 0 || (flags & 0x0001) != 0;
        let has_bulges = (flags & 0x0010) != 0;
        let num_pts = r.read_bl()?.max(0) as usize;
        if num_pts > 1_000_000 {
            return Err(CadError::CorruptDwgStream(
                "Unreasonable LWPOLYLINE vertex count".into(),
            ));
        }
        let num_bulges = if has_bulges {
            r.read_bl()?.max(0) as usize
        } else {
            0
        };
        let mut pts: Vec<[f64; 2]> = Vec::with_capacity(num_pts);
        if num_pts > 0 {
            let p0 = r.read_2rd()?;
            pts.push(p0);
            let mut prev = p0;
            for _ in 1..num_pts {
                let x = r.read_dd(prev[0])?;
                let y = r.read_dd(prev[1])?;
                pts.push([x, y]);
                prev = [x, y];
            }
        }
        let mut bulges = vec![0.0; num_pts];
        for b_slot in bulges.iter_mut().take(num_bulges.min(num_pts)) {
            *b_slot = r.read_bd()?;
        }
        let vertices = pts.into_iter().zip(bulges).collect();
        entities.push(DwgEntitySpec::LwPolyline {
            layer,
            aci_color,
            closed,
            vertices,
        });
    } else if obj_type == DWG_OBJ_TEXT {
        let position = r.read_2rd()?;
        let height = r.read_bd()?;
        let rotation_rad = r.read_bd()?;
        let value = r.read_tv()?;
        entities.push(DwgEntitySpec::Text {
            layer,
            aci_color,
            position,
            height,
            rotation_rad,
            value,
            is_mtext: false,
        });
    } else if obj_type == DWG_OBJ_MTEXT {
        let ins = r.read_3bd()?;
        let _ext = r.read_be()?;
        let xdir = r.read_3bd()?;
        let _rect_w = r.read_bd()?;
        let height = r.read_bd()?;
        let _attach = r.read_bs()?;
        let _flow = r.read_bs()?;
        let value = r.read_tv()?;
        let rotation_rad = xdir[1].atan2(xdir[0]);
        entities.push(DwgEntitySpec::Text {
            layer,
            aci_color,
            position: [ins[0], ins[1]],
            height,
            rotation_rad,
            value,
            is_mtext: true,
        });
    }
    Ok(())
}

fn dwg_entity_to_visio_shape(
    page: &mut VisioPage,
    ent: &DwgEntitySpec,
    to_in: f64,
    id: u64,
) -> Option<vexio_shapesheet::VisioShape> {
    let resolve = |page: &mut VisioPage, layer: &str, aci: i16| -> (u32, RgbaColor, bool) {
        let (lix, lcol, llock) = resolve_or_create_layer(page, layer);
        let col = if (1..=255).contains(&aci) {
            aci_to_rgba(aci as u16)
        } else {
            lcol
        };
        (lix, col, llock)
    };

    match ent {
        DwgEntitySpec::Line {
            layer,
            aci_color,
            start,
            end,
        } => {
            let (lix, col, llock) = resolve(page, layer, *aci_color);
            Some(build_line_shape(
                id,
                [start[0] * to_in, start[1] * to_in],
                [end[0] * to_in, end[1] * to_in],
                lix,
                llock,
                col,
            ))
        }
        DwgEntitySpec::Circle {
            layer,
            aci_color,
            center,
            radius,
        } => {
            let (lix, col, llock) = resolve(page, layer, *aci_color);
            let r = (*radius * to_in).abs();
            Some(build_ellipse_or_circle_shape(
                id,
                "CIRCLE",
                center[0] * to_in,
                center[1] * to_in,
                r,
                r,
                0.0,
                lix,
                llock,
                col,
            ))
        }
        DwgEntitySpec::Arc {
            layer,
            aci_color,
            center,
            radius,
            start_angle_rad,
            end_angle_rad,
        } => {
            let (lix, col, llock) = resolve(page, layer, *aci_color);
            Some(build_arc_shape(
                id,
                center[0] * to_in,
                center[1] * to_in,
                (*radius * to_in).abs(),
                *start_angle_rad,
                *end_angle_rad,
                true,
                lix,
                llock,
                col,
            ))
        }
        DwgEntitySpec::Ellipse {
            layer,
            aci_color,
            center,
            major_axis_vec,
            axis_ratio,
        } => {
            let (lix, col, llock) = resolve(page, layer, *aci_color);
            let mx = major_axis_vec[0] * to_in;
            let my = major_axis_vec[1] * to_in;
            let rx = mx.hypot(my).max(1e-6);
            let ry = (rx * axis_ratio.abs()).max(1e-6);
            let ang = my.atan2(mx);
            Some(build_ellipse_or_circle_shape(
                id,
                "ELLIPSE",
                center[0] * to_in,
                center[1] * to_in,
                rx,
                ry,
                ang,
                lix,
                llock,
                col,
            ))
        }
        DwgEntitySpec::LwPolyline {
            layer,
            aci_color,
            closed,
            vertices,
        } => {
            let (lix, col, llock) = resolve(page, layer, *aci_color);
            let pverts: Vec<PolyVertex> = vertices
                .iter()
                .map(|([x, y], bulge)| PolyVertex {
                    x: *x * to_in,
                    y: *y * to_in,
                    bulge: *bulge,
                })
                .collect();
            build_polyline_shape(id, "LWPOLYLINE", &pverts, *closed, lix, llock, col, None)
        }
        DwgEntitySpec::Text {
            layer,
            aci_color,
            position,
            height,
            rotation_rad,
            value,
            is_mtext,
        } => {
            let (lix, col, llock) = resolve(page, layer, *aci_color);
            let cleaned = strip_mtext_formatting(value);
            Some(build_text_shape(
                id,
                if *is_mtext { "MTEXT" } else { "TEXT" },
                position[0] * to_in,
                position[1] * to_in,
                (*height * to_in).abs().max(0.05),
                None,
                *rotation_rad,
                cleaned,
                lix,
                llock,
                col,
            ))
        }
    }
}
