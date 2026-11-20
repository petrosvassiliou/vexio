//! Unit-aware value system and Visio ShapeSheet canonical conversions.
//!
//! Microsoft Visio evaluates lengths internally in inches (`in`), angles in
//! radians (`rad`), percentages as ratios (`0.5` = `50%`), and elapsed times in
//! days (`eday`), while preserving each cell's display unit for formatting and
//! round-trip serialization.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Unit {
    Number,
    Inch,
    Millimeter,
    Centimeter,
    Meter,
    Kilometer,
    Foot,
    Yard,
    Mile,
    Point,
    Pica,
    Didot,
    Cicero,
    Degree,
    Radian,
    ArcMinute,
    ArcSecond,
    Percent,
    Currency,
    ElapsedSecond,
    ElapsedMinute,
    ElapsedHour,
    ElapsedDay,
    ElapsedWeek,
    Boolean,
    String,
    Color,
    Point2D,
    Guid,
}

impl Unit {
    /// Parse a Visio unit token (case-insensitive), as used in ShapeSheet literals and VSDX `Unit` attributes.
    pub fn from_token(tok: &str) -> Option<Self> {
        match tok.trim().to_ascii_lowercase().as_str() {
            "num" | "none" | "" => Some(Self::Number),
            "in" | "inch" | "inches" | "\"" | "dl" | "du" => Some(Self::Inch),
            "mm" | "millimeter" | "millimeters" => Some(Self::Millimeter),
            "cm" | "centimeter" | "centimeters" => Some(Self::Centimeter),
            "m" | "meter" | "meters" => Some(Self::Meter),
            "km" | "kilometer" | "kilometers" => Some(Self::Kilometer),
            "ft" | "foot" | "feet" | "'" => Some(Self::Foot),
            "yd" | "yard" | "yards" => Some(Self::Yard),
            "mi" | "mile" | "miles" => Some(Self::Mile),
            "pt" | "point" | "points" => Some(Self::Point),
            "p" | "pi" | "pica" | "picas" => Some(Self::Pica),
            "d" | "didot" | "didots" => Some(Self::Didot),
            "c" | "cicero" | "ciceros" => Some(Self::Cicero),
            "deg" | "degree" | "degrees" | "da" => Some(Self::Degree),
            "rad" | "radian" | "radians" => Some(Self::Radian),
            "min" | "arcmin" => Some(Self::ArcMinute),
            "sec" | "arcsec" => Some(Self::ArcSecond),
            "%" | "pct" | "percent" => Some(Self::Percent),
            "cy" | "currency" => Some(Self::Currency),
            "es" | "esec" => Some(Self::ElapsedSecond),
            "em" | "emin" => Some(Self::ElapsedMinute),
            "eh" | "ehr" => Some(Self::ElapsedHour),
            "ed" | "eday" => Some(Self::ElapsedDay),
            "ew" | "ewk" => Some(Self::ElapsedWeek),
            "bool" | "boolean" => Some(Self::Boolean),
            "str" | "string" => Some(Self::String),
            "rgb" | "color" => Some(Self::Color),
            "pnt" => Some(Self::Point2D),
            "guid" => Some(Self::Guid),
            _ => None,
        }
    }

    /// Canonical VSDX OpenXML `U` attribute string.
    pub fn vsdx_code(self) -> &'static str {
        match self {
            Self::Number => "NUM",
            Self::Inch => "IN",
            Self::Millimeter => "MM",
            Self::Centimeter => "CM",
            Self::Meter => "M",
            Self::Kilometer => "KM",
            Self::Foot => "FT",
            Self::Yard => "YD",
            Self::Mile => "MI",
            Self::Point => "PT",
            Self::Pica => "PICA",
            Self::Didot => "D",
            Self::Cicero => "C",
            Self::Degree => "DEG",
            Self::Radian => "RAD",
            Self::ArcMinute => "MIN",
            Self::ArcSecond => "SEC",
            Self::Percent => "PCT",
            Self::Currency => "CY",
            Self::ElapsedSecond => "ES",
            Self::ElapsedMinute => "EM",
            Self::ElapsedHour => "EH",
            Self::ElapsedDay => "ED",
            Self::ElapsedWeek => "EW",
            Self::Boolean => "BOOL",
            Self::String => "STR",
            Self::Color => "RGB",
            Self::Point2D => "PNT",
            Self::Guid => "GUID",
        }
    }

    /// Convert a user-facing numeric literal in `self` unit into Visio's internal scalar:
    /// - Lengths -> inches
    /// - Angles -> radians
    /// - Elapsed -> days
    /// - Percent -> fraction (0.01 per %)
    pub fn to_internal(self, display_val: f64) -> f64 {
        match self {
            Self::Number | Self::Inch | Self::Radian | Self::Currency | Self::ElapsedDay => {
                display_val
            }
            Self::Millimeter => display_val / 25.4,
            Self::Centimeter => display_val / 2.54,
            Self::Meter => display_val / 0.0254,
            Self::Kilometer => display_val / 0.000_025_4,
            Self::Foot => display_val * 12.0,
            Self::Yard => display_val * 36.0,
            Self::Mile => display_val * 63_360.0,
            Self::Point => display_val / 72.0,
            Self::Pica => display_val / 6.0,
            Self::Didot => display_val * 0.014_803_149_606_299_213,
            Self::Cicero => display_val * (12.0 * 0.014_803_149_606_299_213),
            Self::Degree => display_val.to_radians(),
            Self::ArcMinute => (display_val / 60.0).to_radians(),
            Self::ArcSecond => (display_val / 3600.0).to_radians(),
            Self::Percent => display_val * 0.01,
            Self::ElapsedSecond => display_val / 86_400.0,
            Self::ElapsedMinute => display_val / 1_440.0,
            Self::ElapsedHour => display_val / 24.0,
            Self::ElapsedWeek => display_val * 7.0,
            Self::Boolean => {
                if display_val != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::String | Self::Color | Self::Point2D | Self::Guid => display_val,
        }
    }

    /// Convert an internal Visio scalar back into `self` display unit.
    pub fn from_internal(self, internal_val: f64) -> f64 {
        match self {
            Self::Number | Self::Inch | Self::Radian | Self::Currency | Self::ElapsedDay => {
                internal_val
            }
            Self::Millimeter => internal_val * 25.4,
            Self::Centimeter => internal_val * 2.54,
            Self::Meter => internal_val * 0.0254,
            Self::Kilometer => internal_val * 0.000_025_4,
            Self::Foot => internal_val / 12.0,
            Self::Yard => internal_val / 36.0,
            Self::Mile => internal_val / 63_360.0,
            Self::Point => internal_val * 72.0,
            Self::Pica => internal_val * 6.0,
            Self::Didot => internal_val / 0.014_803_149_606_299_213,
            Self::Cicero => internal_val / (12.0 * 0.014_803_149_606_299_213),
            Self::Degree => internal_val.to_degrees(),
            Self::ArcMinute => internal_val.to_degrees() * 60.0,
            Self::ArcSecond => internal_val.to_degrees() * 3600.0,
            Self::Percent => internal_val * 100.0,
            Self::ElapsedSecond => internal_val * 86_400.0,
            Self::ElapsedMinute => internal_val * 1_440.0,
            Self::ElapsedHour => internal_val * 24.0,
            Self::ElapsedWeek => internal_val / 7.0,
            Self::Boolean => {
                if internal_val != 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Self::String | Self::Color | Self::Point2D | Self::Guid => internal_val,
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Self::Number | Self::Currency => "",
            Self::Inch => " in",
            Self::Millimeter => " mm",
            Self::Centimeter => " cm",
            Self::Meter => " m",
            Self::Kilometer => " km",
            Self::Foot => " ft",
            Self::Yard => " yd",
            Self::Mile => " mi",
            Self::Point => " pt",
            Self::Pica => " pica",
            Self::Didot => " d",
            Self::Cicero => " c",
            Self::Degree => " deg",
            Self::Radian => " rad",
            Self::ArcMinute => " min",
            Self::ArcSecond => " sec",
            Self::Percent => "%",
            Self::ElapsedSecond => " es",
            Self::ElapsedMinute => " em",
            Self::ElapsedHour => " eh",
            Self::ElapsedDay => " ed",
            Self::ElapsedWeek => " ew",
            Self::Boolean | Self::String | Self::Color | Self::Point2D | Self::Guid => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellError {
    DivByZero,
    Value,
    Name,
    Ref,
    NotAvailable,
    Num,
}

impl CellError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DivByZero => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Name => "#NAME?",
            Self::Ref => "#REF!",
            Self::NotAvailable => "#N/A",
            Self::Num => "#NUM!",
        }
    }
}

impl fmt::Display for CellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0, a: 255 };
    pub const WHITE: Self = Self { r: 255, g: 255, b: 255, a: 255 };
    pub const TRANSPARENT: Self = Self { r: 0, g: 0, b: 0, a: 0 };

    pub fn from_hex(hex: &str) -> Option<Self> {
        let s = hex.trim().trim_start_matches('#');
        match s.len() {
            6 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                Some(Self { r, g, b, a: 255 })
            }
            8 => {
                let r = u8::from_str_radix(&s[0..2], 16).ok()?;
                let g = u8::from_str_radix(&s[2..4], 16).ok()?;
                let b = u8::from_str_radix(&s[4..6], 16).ok()?;
                let a = u8::from_str_radix(&s[6..8], 16).ok()?;
                Some(Self { r, g, b, a })
            }
            _ => None,
        }
    }

    pub fn to_hex_rgb(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn to_linear_f32(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub fn from_hsl(h_240: f64, s_240: f64, l_240: f64) -> Self {
        // Visio HSL uses [0..240] domain for Hue, Saturation, Luminance.
        let h = (h_240.clamp(0.0, 240.0) / 240.0) * 360.0;
        let s = s_240.clamp(0.0, 240.0) / 240.0;
        let l = l_240.clamp(0.0, 240.0) / 240.0;
        if s == 0.0 {
            let v = (l * 255.0).round() as u8;
            return Self { r: v, g: v, b: v, a: 255 };
        }
        let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = l - c / 2.0;
        let (r1, g1, b1) = match h as u32 {
            0..=59 => (c, x, 0.0),
            60..=119 => (x, c, 0.0),
            120..=179 => (0.0, c, x),
            180..=239 => (0.0, x, c),
            240..=299 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        Self {
            r: ((r1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
            g: ((g1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
            b: ((b1 + m) * 255.0).round().clamp(0.0, 255.0) as u8,
            a: 255,
        }
    }

    pub fn to_visio_hsl(self) -> (f64, f64, f64) {
        let r = self.r as f64 / 255.0;
        let g = self.g as f64 / 255.0;
        let b = self.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) * 0.5;
        let d = max - min;
        if d.abs() < 1e-12 {
            return (0.0, 0.0, (l * 240.0).round());
        }
        let s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };
        let mut h = if (max - r).abs() < 1e-12 {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() < 1e-12 {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h /= 6.0;
        ((h * 240.0).round(), (s * 240.0).round(), (l * 240.0).round())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    Num { internal: f64, unit: Unit },
    Bool(bool),
    Str(String),
    Color(RgbaColor),
    Point { x: f64, y: f64 },
    Ref(String),
    Err(CellError),
}

impl Default for CellValue {
    fn default() -> Self {
        Self::Num {
            internal: 0.0,
            unit: Unit::Number,
        }
    }
}

impl CellValue {
    pub fn inches(v: f64) -> Self {
        Self::Num {
            internal: normalize_f64(v),
            unit: Unit::Inch,
        }
    }

    pub fn number(v: f64) -> Self {
        Self::Num {
            internal: normalize_f64(v),
            unit: Unit::Number,
        }
    }

    pub fn with_unit(display_val: f64, unit: Unit) -> Self {
        Self::Num {
            internal: normalize_f64(unit.to_internal(display_val)),
            unit,
        }
    }

    pub fn radians(rad: f64) -> Self {
        Self::Num {
            internal: normalize_f64(rad),
            unit: Unit::Radian,
        }
    }

    pub fn degrees(deg: f64) -> Self {
        Self::Num {
            internal: normalize_f64(deg.to_radians()),
            unit: Unit::Degree,
        }
    }

    /// Coerce value to Visio's internal scalar (`f64`), propagating `CellError`.
    pub fn as_internal_f64(&self) -> Result<f64, CellError> {
        match self {
            Self::Num { internal, .. } => {
                if internal.is_nan() || internal.is_infinite() {
                    Err(CellError::Num)
                } else {
                    Ok(*internal)
                }
            }
            Self::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Self::Str(s) => {
                if let Some((v, u)) = parse_scalar_with_unit(s) {
                    Ok(u.to_internal(v))
                } else {
                    Err(CellError::Value)
                }
            }
            Self::Color(c) => Ok(((c.r as u32) << 16 | (c.g as u32) << 8 | (c.b as u32)) as f64),
            Self::Point { x, .. } => Ok(*x),
            Self::Ref(_) => Err(CellError::Value),
            Self::Err(e) => Err(*e),
        }
    }

    pub fn as_bool(&self) -> Result<bool, CellError> {
        match self {
            Self::Bool(b) => Ok(*b),
            Self::Num { internal, .. } => Ok(internal.abs() > 1e-12),
            Self::Str(s) => match s.trim().to_ascii_uppercase().as_str() {
                "TRUE" | "1" => Ok(true),
                "FALSE" | "0" => Ok(false),
                _ => Err(CellError::Value),
            },
            Self::Err(e) => Err(*e),
            _ => Err(CellError::Value),
        }
    }

    pub fn as_str_coerced(&self) -> Result<String, CellError> {
        match self {
            Self::Str(s) => Ok(s.clone()),
            Self::Bool(b) => Ok(if *b { "TRUE".into() } else { "FALSE".into() }),
            Self::Num { internal, unit } => {
                let d = normalize_f64(unit.from_internal(*internal));
                if (d - d.round()).abs() < 1e-9 && d.abs() < 1e15 {
                    Ok(format!("{}{}", d.round() as i64, unit.suffix()))
                } else {
                    let s = format!("{:.6}", d);
                    let trimmed = s.trim_end_matches('0').trim_end_matches('.');
                    Ok(format!("{}{}", trimmed, unit.suffix()))
                }
            }
            Self::Color(c) => Ok(c.to_hex_rgb()),
            Self::Point { x, y } => Ok(format!("PNT({:.4} in, {:.4} in)", x, y)),
            Self::Ref(r) => Ok(r.clone()),
            Self::Err(e) => Err(*e),
        }
    }

    /// Format scalar as stored in VSDX `<Cell V="...">` attribute (internal scalar without suffix).
    pub fn to_vsdx_val_string(&self) -> String {
        match self {
            Self::Num { internal, .. } => format_clean_f64(*internal),
            Self::Bool(b) => if *b { "1".into() } else { "0".into() },
            Self::Str(s) => s.clone(),
            Self::Color(c) => c.to_hex_rgb(),
            Self::Point { x, y } => format!("PNT({},{})", format_clean_f64(*x), format_clean_f64(*y)),
            Self::Ref(r) => r.clone(),
            Self::Err(e) => e.as_str().into(),
        }
    }

    pub fn unit(&self) -> Unit {
        match self {
            Self::Num { unit, .. } => *unit,
            Self::Bool(_) => Unit::Boolean,
            Self::Str(_) => Unit::String,
            Self::Color(_) => Unit::Color,
            Self::Point { .. } => Unit::Point2D,
            Self::Ref(_) | Self::Err(_) => Unit::Number,
        }
    }
}

pub fn normalize_f64(v: f64) -> f64 {
    if v == 0.0 || v.abs() < 1e-15 {
        0.0
    } else {
        v
    }
}

pub fn format_clean_f64(v: f64) -> String {
    let v = normalize_f64(v);
    if (v - v.round()).abs() < 1e-11 && v.abs() < 1e15 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{:.10}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Parse a literal such as `"25.4 mm"`, `"1.5 in"`, `"45 deg"`, `"50%"`, or `"3.14"`.
pub fn parse_scalar_with_unit(input: &str) -> Option<(f64, Unit)> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(hex) = s.strip_prefix('#') {
        if RgbaColor::from_hex(hex).is_some() {
            return None;
        }
    }
    // Find boundary between numeric prefix (including scientific notation) and unit suffix.
    let bytes = s.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let mut saw_digit = false;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        saw_digit = true;
        i += 1;
    }
    if i < bytes.len() && bytes[i] == b'.' {
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            saw_digit = true;
            i += 1;
        }
    }
    if !saw_digit {
        return None;
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let save = i;
        i += 1;
        if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            // It wasn't scientific notation (e.g., "10em" elapsed minutes!).
            i = save;
        }
    }
    let num_part = &s[..i];
    let unit_part = s[i..].trim();
    let val: f64 = num_part.parse().ok()?;
    if unit_part.is_empty() {
        Some((val, Unit::Number))
    } else {
        let unit = Unit::from_token(unit_part)?;
        Some((val, unit))
    }
}
