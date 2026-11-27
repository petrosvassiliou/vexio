//! Structured ShapeSheet sections and canonical cell storage.
//!
//! Every Visio object (`DocumentSheet`, `PageSheet`, `StyleSheet`, `Master`, `Shape`)
//! owns a `ShapeSheet`. Structured sections (`Geometry[1..N]`, `ConnectionPoints`,
//! `Controls`, `Actions`, `User`, `ShapeData` / `Prop`, `TextFields`, `Character`,
//! `Paragraph`, `Layers`) synchronize bidirectionally with the cell map so both
//! reactive formula references (`Geometry1.X2`, `Controls.Row_1.Y`, `Prop.Cost`)
//! and file-format codecs operate on a single source of truth.

use crate::parser::{Expr, canonical_cell_name, parse_formula};
use crate::units::{CellError, CellValue, RgbaColor, Unit};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub name: String,
    pub formula: String,
    pub ast: Option<Expr>,
    pub value: CellValue,
    #[serde(default)]
    pub vsdx_cached_value: Option<CellValue>,
    pub unit_hint: Option<Unit>,
    pub deleted: bool,
}

impl Cell {
    pub fn from_formula(name: impl Into<String>, formula: impl Into<String>, unit_hint: Option<Unit>) -> Self {
        let name = name.into();
        let formula = formula.into();
        let ast = parse_formula(&formula).ok();
        let value = match &ast {
            Some(Expr::Number { value, unit }) => {
                let effective_unit = if *unit == Unit::Number {
                    unit_hint.unwrap_or(Unit::Number)
                } else {
                    *unit
                };
                CellValue::with_unit(*value, effective_unit)
            }
            Some(Expr::Bool(b)) => CellValue::Bool(*b),
            Some(Expr::Str(s)) => CellValue::Str(s.clone()),
            Some(Expr::Call { name: fn_name, args })
                if fn_name.eq_ignore_ascii_case("RGB") && args.len() == 3 =>
            {
                if let (
                    Expr::Number { value: r, .. },
                    Expr::Number { value: g, .. },
                    Expr::Number { value: b, .. },
                ) = (&args[0], &args[1], &args[2])
                {
                    CellValue::Color(RgbaColor {
                        r: r.round().clamp(0.0, 255.0) as u8,
                        g: g.round().clamp(0.0, 255.0) as u8,
                        b: b.round().clamp(0.0, 255.0) as u8,
                        a: 255,
                    })
                } else {
                    CellValue::Num {
                        internal: 0.0,
                        unit: unit_hint.unwrap_or(Unit::Number),
                    }
                }
            }
            None if !formula.starts_with('=') => CellValue::Str(formula.clone()),
            None => CellValue::Err(CellError::Name),
            _ => CellValue::Num {
                internal: 0.0,
                unit: unit_hint.unwrap_or(Unit::Number),
            },
        };
        Self {
            name,
            formula,
            ast,
            value,
            vsdx_cached_value: None,
            unit_hint,
            deleted: false,
        }
    }

    pub fn from_literal_internal(name: impl Into<String>, internal: f64, unit: Unit) -> Self {
        let name = name.into();
        let display = unit.from_internal(internal);
        let formula = format!("{}{}", crate::units::format_clean_f64(display), unit.suffix());
        let ast = parse_formula(&formula).ok();
        Self {
            name,
            formula,
            ast,
            value: CellValue::Num { internal, unit },
            vsdx_cached_value: None,
            unit_hint: Some(unit),
            deleted: false,
        }
    }

    pub fn is_guarded(&self) -> bool {
        self.ast
            .as_ref()
            .map(|e| e.is_top_level_guard())
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeometryRowKind {
    MoveTo,
    LineTo,
    ArcTo,
    InfiniteLine,
    Ellipse,
    EllipticalArcTo,
    SplineStart,
    SplineKnot,
    PolylineTo,
    NURBSTo,
    RelMoveTo,
    RelLineTo,
    RelCubBezTo,
    RelQuadBezTo,
    RelEllipticalArcTo,
}

impl GeometryRowKind {
    pub fn from_vsdx_tag(tag: &str) -> Option<Self> {
        match tag {
            "MoveTo" => Some(Self::MoveTo),
            "LineTo" => Some(Self::LineTo),
            "ArcTo" => Some(Self::ArcTo),
            "InfiniteLine" => Some(Self::InfiniteLine),
            "Ellipse" => Some(Self::Ellipse),
            "EllipticalArcTo" => Some(Self::EllipticalArcTo),
            "SplineStart" => Some(Self::SplineStart),
            "SplineKnot" => Some(Self::SplineKnot),
            "PolylineTo" => Some(Self::PolylineTo),
            "NURBSTo" => Some(Self::NURBSTo),
            "RelMoveTo" => Some(Self::RelMoveTo),
            "RelLineTo" => Some(Self::RelLineTo),
            "RelCubBezTo" => Some(Self::RelCubBezTo),
            "RelQuadBezTo" => Some(Self::RelQuadBezTo),
            "RelEllipticalArcTo" => Some(Self::RelEllipticalArcTo),
            _ => None,
        }
    }

    pub fn vsdx_tag(self) -> &'static str {
        match self {
            Self::MoveTo => "MoveTo",
            Self::LineTo => "LineTo",
            Self::ArcTo => "ArcTo",
            Self::InfiniteLine => "InfiniteLine",
            Self::Ellipse => "Ellipse",
            Self::EllipticalArcTo => "EllipticalArcTo",
            Self::SplineStart => "SplineStart",
            Self::SplineKnot => "SplineKnot",
            Self::PolylineTo => "PolylineTo",
            Self::NURBSTo => "NURBSTo",
            Self::RelMoveTo => "RelMoveTo",
            Self::RelLineTo => "RelLineTo",
            Self::RelCubBezTo => "RelCubBezTo",
            Self::RelQuadBezTo => "RelQuadBezTo",
            Self::RelEllipticalArcTo => "RelEllipticalArcTo",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryRow {
    pub ix: u32,
    pub kind: GeometryRowKind,
    pub x: String,
    pub y: String,
    pub a: Option<String>,
    pub b: Option<String>,
    pub c: Option<String>,
    pub d: Option<String>,
    pub e: Option<String>,
    pub deleted: bool,
}

impl GeometryRow {
    pub fn move_to(ix: u32, x: impl Into<String>, y: impl Into<String>) -> Self {
        Self {
            ix,
            kind: GeometryRowKind::MoveTo,
            x: x.into(),
            y: y.into(),
            a: None,
            b: None,
            c: None,
            d: None,
            e: None,
            deleted: false,
        }
    }

    pub fn line_to(ix: u32, x: impl Into<String>, y: impl Into<String>) -> Self {
        Self {
            ix,
            kind: GeometryRowKind::LineTo,
            x: x.into(),
            y: y.into(),
            a: None,
            b: None,
            c: None,
            d: None,
            e: None,
            deleted: false,
        }
    }

    pub fn arc_to(ix: u32, x: impl Into<String>, y: impl Into<String>, bow: impl Into<String>) -> Self {
        Self {
            ix,
            kind: GeometryRowKind::ArcTo,
            x: x.into(),
            y: y.into(),
            a: Some(bow.into()),
            b: None,
            c: None,
            d: None,
            e: None,
            deleted: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometrySection {
    pub index: u32, // 1-based (`Geometry1`, `Geometry2`, ...)
    pub no_fill: String,
    pub no_line: String,
    pub no_show: String,
    pub no_snap: String,
    pub no_quick_drag: String,
    #[serde(default)]
    pub explicit_flags: u8,
    pub rows: Vec<GeometryRow>,
    pub deleted: bool,
}

impl GeometrySection {
    pub fn new(index: u32) -> Self {
        Self {
            index,
            no_fill: "FALSE".into(),
            no_line: "FALSE".into(),
            no_show: "FALSE".into(),
            no_snap: "FALSE".into(),
            no_quick_drag: "FALSE".into(),
            explicit_flags: 0,
            rows: Vec::new(),
            deleted: false,
        }
    }

    pub fn rectangle(index: u32) -> Self {
        let mut sec = Self::new(index);
        sec.rows = vec![
            GeometryRow::move_to(1, "0 in", "0 in"),
            GeometryRow::line_to(2, "Width*1", "0 in"),
            GeometryRow::line_to(3, "Width*1", "Height*1"),
            GeometryRow::line_to(4, "0 in", "Height*1"),
            GeometryRow::line_to(5, "0 in", "0 in"),
        ];
        sec
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnectionPointRow {
    pub name: String,
    pub ix: u32,
    pub x: String,
    pub y: String,
    pub dir_x: String,
    pub dir_y: String,
    pub point_type: u8, // 0=Inward, 1=Outward, 2=Inward&Outward
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControlRow {
    pub name: String, // e.g., `Row_1`
    pub x: String,
    pub y: String,
    pub x_dyn: String,
    pub y_dyn: String,
    pub x_con: String,
    pub y_con: String,
    pub can_glue: bool,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionRow {
    pub name: String,
    pub action: String,
    pub menu: String,
    pub tag_name: String,
    pub checked: String,
    pub disabled: String,
    pub invisible: String,
    pub begin_group: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserCellRow {
    pub name: String, // without `User.`
    pub value: String,
    pub prompt: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeDataType {
    String = 0,
    FixedList = 1,
    Number = 2,
    Boolean = 3,
    VariableList = 4,
    DateTime = 5,
    Duration = 6,
    Currency = 7,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShapeDataRow {
    pub name: String, // without `Prop.`
    pub label: String,
    pub prompt: String,
    pub data_type: ShapeDataType,
    pub format: String,
    pub value: String,
    pub sort_key: String,
    pub invisible: bool,
    pub ask: bool,
    pub lang_id: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextFieldRow {
    pub ix: u32,
    pub value: String,
    pub format: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientStopRow {
    pub ix: u32,
    /// Stop position in `[0.0 .. 1.0]` (or `%`).
    pub position: String,
    pub color: String,
    pub color_trans: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyperlinkRow {
    pub name: String,
    pub description: String,
    pub address: String,
    pub sub_address: String,
    pub new_window: bool,
    pub default: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapeSheet {
    /// Canonical lowercase key -> Cell.
    pub cells: BTreeMap<String, Cell>,
    pub geometries: Vec<GeometrySection>,
    pub connection_points: Vec<ConnectionPointRow>,
    pub controls: Vec<ControlRow>,
    pub actions: Vec<ActionRow>,
    pub user_cells: BTreeMap<String, UserCellRow>,
    pub shape_data: BTreeMap<String, ShapeDataRow>,
    pub text_fields: Vec<TextFieldRow>,
    pub hyperlinks: Vec<HyperlinkRow>,
    #[serde(default)]
    pub fill_gradient_stops: Vec<GradientStopRow>,
}

impl ShapeSheet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a standard 2D Visio shape sheet initialized with Visio's canonical defaults.
    pub fn new_2d(pin_x_in: f64, pin_y_in: f64, width_in: f64, height_in: f64) -> Self {
        let mut ss = Self::new();
        ss.set_formula("PinX", format!("{} in", crate::units::format_clean_f64(pin_x_in)), Some(Unit::Inch));
        ss.set_formula("PinY", format!("{} in", crate::units::format_clean_f64(pin_y_in)), Some(Unit::Inch));
        ss.set_formula("Width", format!("{} in", crate::units::format_clean_f64(width_in)), Some(Unit::Inch));
        ss.set_formula("Height", format!("{} in", crate::units::format_clean_f64(height_in)), Some(Unit::Inch));
        ss.set_formula("LocPinX", "=Width*0.5", Some(Unit::Inch));
        ss.set_formula("LocPinY", "=Height*0.5", Some(Unit::Inch));
        ss.set_formula("Angle", "0 deg", Some(Unit::Degree));
        ss.set_formula("FlipX", "FALSE", Some(Unit::Boolean));
        ss.set_formula("FlipY", "FALSE", Some(Unit::Boolean));
        ss.set_formula("TxtPinX", "=Width*0.5", Some(Unit::Inch));
        ss.set_formula("TxtPinY", "=Height*0.5", Some(Unit::Inch));
        ss.set_formula("TxtWidth", "=Width*1", Some(Unit::Inch));
        ss.set_formula("TxtHeight", "=Height*1", Some(Unit::Inch));
        ss.set_formula("TxtLocPinX", "=TxtWidth*0.5", Some(Unit::Inch));
        ss.set_formula("TxtLocPinY", "=TxtHeight*0.5", Some(Unit::Inch));
        ss.set_formula("TxtAngle", "0 deg", Some(Unit::Degree));
        ss.set_formula("FillForegnd", "=RGB(255,255,255)", Some(Unit::Color));
        ss.set_formula("FillBkgnd", "=RGB(240,244,248)", Some(Unit::Color));
        ss.set_formula("FillPattern", "1", Some(Unit::Number));
        ss.set_formula("LineWeight", "1 pt", Some(Unit::Point));
        ss.set_formula("LineColor", "=RGB(30,41,59)", Some(Unit::Color));
        ss.set_formula("LinePattern", "1", Some(Unit::Number));
        ss.set_formula("Rounding", "0 in", Some(Unit::Inch));
        ss.set_formula("Char.Size", "10 pt", Some(Unit::Point));
        ss.set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));
        ss.add_geometry(GeometrySection::rectangle(1));
        ss
    }

    /// Create a standard 1D Dynamic Connector shape sheet.
    pub fn new_1d_connector(begin_x: f64, begin_y: f64, end_x: f64, end_y: f64) -> Self {
        let mut ss = Self::new();
        ss.set_formula("BeginX", format!("{} in", crate::units::format_clean_f64(begin_x)), Some(Unit::Inch));
        ss.set_formula("BeginY", format!("{} in", crate::units::format_clean_f64(begin_y)), Some(Unit::Inch));
        ss.set_formula("EndX", format!("{} in", crate::units::format_clean_f64(end_x)), Some(Unit::Inch));
        ss.set_formula("EndY", format!("{} in", crate::units::format_clean_f64(end_y)), Some(Unit::Inch));
        ss.set_formula("Width", "=SQRT((EndX-BeginX)^2+(EndY-BeginY)^2)", Some(Unit::Inch));
        ss.set_formula("Height", "0 in", Some(Unit::Inch));
        ss.set_formula("PinX", "=(BeginX+EndX)*0.5", Some(Unit::Inch));
        ss.set_formula("PinY", "=(BeginY+EndY)*0.5", Some(Unit::Inch));
        ss.set_formula("LocPinX", "=Width*0.5", Some(Unit::Inch));
        ss.set_formula("LocPinY", "0 in", Some(Unit::Inch));
        ss.set_formula("Angle", "=ATAN2(EndY-BeginY, EndX-BeginX)", Some(Unit::Radian));
        ss.set_formula("LineWeight", "1 pt", Some(Unit::Point));
        ss.set_formula("LineColor", "=RGB(30,41,59)", Some(Unit::Color));
        ss.set_formula("LinePattern", "1", Some(Unit::Number));
        ss.set_formula("EndArrow", "4", Some(Unit::Number));
        ss.set_formula("ShapeRouteStyle", "16", Some(Unit::Number)); // Right-angle orthogonal
        ss.set_formula("ConLineJumpCode", "1", Some(Unit::Number));
        ss.set_formula("ConLineJumpStyle", "0", Some(Unit::Number)); // Arc jump
        let mut geom = GeometrySection::new(1);
        geom.no_fill = "TRUE".into();
        geom.rows = vec![
            GeometryRow::move_to(1, "0 in", "0 in"),
            GeometryRow::line_to(2, "Width", "0 in"),
        ];
        ss.add_geometry(geom);
        ss
    }

    pub fn set_formula(&mut self, name: impl Into<String>, formula: impl Into<String>, unit_hint: Option<Unit>) {
        let name_str = name.into();
        let formula_str = formula.into();
        let key = canonical_cell_name(&name_str);
        self.sync_structured_row_from_cell_key(&key, &formula_str);
        let hint = unit_hint.or_else(|| default_unit_for_cell(&key));
        let cell = Cell::from_formula(name_str, formula_str, hint);
        self.cells.insert(key, cell);
    }

    /// Attempt to write an interactive value (`SetValue` semantics).
    /// Returns `Err(CellError::Ref)` if the cell is protected by `GUARD(...)` and `force_setf` is false.
    pub fn try_set_formula_respecting_guard(
        &mut self,
        name: &str,
        new_formula: impl Into<String>,
        force_setf: bool,
    ) -> Result<(), CellError> {
        let key = canonical_cell_name(name);
        if !force_setf {
            if let Some(existing) = self.cells.get(&key) {
                if existing.is_guarded() {
                    return Err(CellError::Ref);
                }
            }
        }
        let formula_str = new_formula.into();
        self.sync_structured_row_from_cell_key(&key, &formula_str);
        let orig_name = self
            .cells
            .get(&key)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| name.to_string());
        let hint = self
            .cells
            .get(&key)
            .and_then(|c| c.unit_hint)
            .or_else(|| default_unit_for_cell(&key));
        let cell = Cell::from_formula(orig_name, formula_str, hint);
        self.cells.insert(key, cell);
        Ok(())
    }

    fn sync_structured_row_from_cell_key(&mut self, key: &str, formula_str: &str) {
        if let Some(prop_rest) = key.strip_prefix("prop.") {
            if !prop_rest.contains('.') {
                if let Some(row) = self.shape_data.get_mut(prop_rest) {
                    row.value = formula_str.to_string();
                }
            }
        } else if let Some(user_rest) = key.strip_prefix("user.") {
            if !user_rest.contains('.') {
                if let Some(row) = self.user_cells.get_mut(user_rest) {
                    row.value = formula_str.to_string();
                }
            }
        } else if let Some(ctrl_rest) = key.strip_prefix("controls.") {
            let (row_key, sub) = match ctrl_rest.split_once('.') {
                Some((r, s)) => (r, s),
                None => (ctrl_rest, "x"),
            };
            if let Some(ctrl) = self
                .controls
                .iter_mut()
                .find(|c| c.name.eq_ignore_ascii_case(row_key))
            {
                match sub {
                    "x" => ctrl.x = formula_str.to_string(),
                    "y" => ctrl.y = formula_str.to_string(),
                    "xdyn" => ctrl.x_dyn = formula_str.to_string(),
                    "ydyn" => ctrl.y_dyn = formula_str.to_string(),
                    "xcon" => ctrl.x_con = formula_str.to_string(),
                    "ycon" => ctrl.y_con = formula_str.to_string(),
                    _ => {}
                }
            }
        } else if let Some(act_rest) = key.strip_prefix("actions.") {
            if let Some((row_key, sub)) = act_rest.split_once('.') {
                if let Some(act) = self
                    .actions
                    .iter_mut()
                    .find(|a| a.name.eq_ignore_ascii_case(row_key))
                {
                    match sub {
                        "action" => act.action = formula_str.to_string(),
                        "menu" => act.menu = formula_str.trim_matches('"').to_string(),
                        "tagname" => act.tag_name = formula_str.trim_matches('"').to_string(),
                        "checked" => act.checked = formula_str.to_string(),
                        "disabled" => act.disabled = formula_str.to_string(),
                        "invisible" => act.invisible = formula_str.to_string(),
                        "begingroup" => {
                            act.begin_group = formula_str.trim() == "1"
                                || formula_str.trim().eq_ignore_ascii_case("true");
                        }
                        _ => {}
                    }
                }
            }
        } else if let Some(conn_rest) = key.strip_prefix("connections.") {
            if let Some(ix_str) = conn_rest.strip_prefix('x') {
                if let Ok(ix) = ix_str.parse::<u32>() {
                    if let Some(cp) = self.connection_points.iter_mut().find(|c| c.ix == ix) {
                        cp.x = formula_str.to_string();
                    }
                }
            } else if let Some(ix_str) = conn_rest.strip_prefix('y') {
                if let Ok(ix) = ix_str.parse::<u32>() {
                    if let Some(cp) = self.connection_points.iter_mut().find(|c| c.ix == ix) {
                        cp.y = formula_str.to_string();
                    }
                }
            } else if let Some(ix_str) = conn_rest.strip_prefix("dirx") {
                if let Ok(ix) = ix_str.parse::<u32>() {
                    if let Some(cp) = self.connection_points.iter_mut().find(|c| c.ix == ix) {
                        cp.dir_x = formula_str.to_string();
                    }
                }
            } else if let Some(ix_str) = conn_rest.strip_prefix("diry") {
                if let Ok(ix) = ix_str.parse::<u32>() {
                    if let Some(cp) = self.connection_points.iter_mut().find(|c| c.ix == ix) {
                        cp.dir_y = formula_str.to_string();
                    }
                }
            } else if let Some((row_name, sub)) = conn_rest.split_once('.') {
                if let Some(cp) = self
                    .connection_points
                    .iter_mut()
                    .find(|c| c.name.eq_ignore_ascii_case(row_name))
                {
                    match sub {
                        "x" => cp.x = formula_str.to_string(),
                        "y" => cp.y = formula_str.to_string(),
                        "dirx" => cp.dir_x = formula_str.to_string(),
                        "diry" => cp.dir_y = formula_str.to_string(),
                        _ => {}
                    }
                }
            }
        }
    }

    pub fn get_cell(&self, name: &str) -> Option<&Cell> {
        self.cells.get(&canonical_cell_name(name))
    }

    pub fn get_cell_mut(&mut self, name: &str) -> Option<&mut Cell> {
        self.cells.get_mut(&canonical_cell_name(name))
    }

    pub fn get_f64(&self, name: &str) -> Option<f64> {
        self.get_cell(name)?.value.as_internal_f64().ok()
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        self.get_cell(name)?.value.as_bool().ok()
    }

    pub fn get_color(&self, name: &str) -> Option<RgbaColor> {
        let key = canonical_cell_name(name);
        let is_text_bkgnd = key == "textbkgnd";
        let cell = self.cells.get(&key)?;
        match &cell.value {
            CellValue::Color(c) => Some(*c),
            CellValue::Str(s) => {
                let t = s.trim().trim_matches('"').trim();
                if is_text_bkgnd
                    && (t == "0"
                        || t.eq_ignore_ascii_case("none")
                        || t.eq_ignore_ascii_case("transparent"))
                {
                    return None;
                }
                RgbaColor::from_hex(t).or_else(|| {
                    let idx = t.parse::<u32>().ok()?;
                    if is_text_bkgnd {
                        idx.checked_sub(1).and_then(visio_standard_palette_color)
                    } else {
                        visio_standard_palette_color(idx)
                    }
                })
            }
            CellValue::Num { internal, .. } if internal.is_finite() => {
                let rounded = internal.round() as i64;
                if is_text_bkgnd {
                    // Visio `[MS-VSDX]` rule: `TextBkgnd = 0` means None (transparent); `1..=254` maps to `Palette[k - 1]`.
                    if rounded <= 0 {
                        None
                    } else {
                        visio_standard_palette_color((rounded - 1) as u32)
                    }
                } else if rounded >= 0 {
                    visio_standard_palette_color(rounded as u32)
                } else {
                    None
                }
            }
            _ => match &cell.vsdx_cached_value {
                Some(CellValue::Color(c)) => Some(*c),
                _ => None,
            },
        }
    }

    pub fn get_str(&self, name: &str) -> Option<String> {
        match &self.get_cell(name)?.value {
            CellValue::Str(s) => Some(s.trim_matches('"').to_string()),
            _ => None,
        }
    }

    pub fn add_geometry(&mut self, sec: GeometrySection) {
        self.sync_geometry_to_cells(&sec);
        if let Some(existing) = self.geometries.iter_mut().find(|g| g.index == sec.index) {
            *existing = sec;
        } else {
            self.geometries.push(sec);
            self.geometries.sort_by_key(|g| g.index);
        }
    }

    pub fn sync_all_sections_to_cells(&mut self) {
        let geoms = self.geometries.clone();
        for g in &geoms {
            self.sync_geometry_to_cells(g);
        }
        let cps = self.connection_points.clone();
        for cp in &cps {
            self.sync_connection_point_to_cells(cp);
        }
        let ctrls = self.controls.clone();
        for c in &ctrls {
            self.sync_control_to_cells(c);
        }
        let acts = self.actions.clone();
        for a in &acts {
            self.sync_action_to_cells(a);
        }
        let users: Vec<UserCellRow> = self.user_cells.values().cloned().collect();
        for u in &users {
            self.sync_user_row_to_cells(u);
        }
        let props: Vec<ShapeDataRow> = self.shape_data.values().cloned().collect();
        for p in &props {
            self.sync_shape_data_to_cells(p);
        }
        let gstops = self.fill_gradient_stops.clone();
        for gs in &gstops {
            self.sync_gradient_stop_to_cells(gs);
        }
    }

    pub fn add_fill_gradient_stop(&mut self, stop: GradientStopRow) {
        self.sync_gradient_stop_to_cells(&stop);
        if let Some(ex) = self.fill_gradient_stops.iter_mut().find(|s| s.ix == stop.ix) {
            *ex = stop;
        } else {
            self.fill_gradient_stops.push(stop);
            self.fill_gradient_stops.sort_by_key(|s| s.ix);
        }
    }

    fn sync_gradient_stop_to_cells(&mut self, stop: &GradientStopRow) {
        let ix = stop.ix;
        if !stop.position.eq_ignore_ascii_case("Inh") && !stop.position.is_empty() {
            self.set_formula(
                format!("FillGradient.Stop{ix}.Position"),
                stop.position.clone(),
                Some(Unit::Number),
            );
        }
        if !stop.color.eq_ignore_ascii_case("Inh") && !stop.color.is_empty() {
            self.set_formula(
                format!("FillGradient.Stop{ix}.Color"),
                stop.color.clone(),
                Some(Unit::Color),
            );
        }
        if !stop.color_trans.eq_ignore_ascii_case("Inh") && !stop.color_trans.is_empty() {
            self.set_formula(
                format!("FillGradient.Stop{ix}.ColorTrans"),
                stop.color_trans.clone(),
                Some(Unit::Number),
            );
        }
    }

    fn sync_geometry_to_cells(&mut self, sec: &GeometrySection) {
        let prefix = format!("Geometry{}", sec.index);
        if !sec.no_fill.eq_ignore_ascii_case("Inh") {
            self.set_formula(format!("{prefix}.NoFill"), sec.no_fill.clone(), Some(Unit::Boolean));
        }
        if !sec.no_line.eq_ignore_ascii_case("Inh") {
            self.set_formula(format!("{prefix}.NoLine"), sec.no_line.clone(), Some(Unit::Boolean));
        }
        if !sec.no_show.eq_ignore_ascii_case("Inh") {
            self.set_formula(format!("{prefix}.NoShow"), sec.no_show.clone(), Some(Unit::Boolean));
        }
        if !sec.no_snap.eq_ignore_ascii_case("Inh") {
            self.set_formula(format!("{prefix}.NoSnap"), sec.no_snap.clone(), Some(Unit::Boolean));
        }
        if !sec.no_quick_drag.eq_ignore_ascii_case("Inh") {
            self.set_formula(format!("{prefix}.NoQuickDrag"), sec.no_quick_drag.clone(), Some(Unit::Boolean));
        }
        for row in &sec.rows {
            if row.deleted {
                continue;
            }
            let ix = row.ix;
            let x_unit = if matches!(
                row.kind,
                GeometryRowKind::RelMoveTo
                    | GeometryRowKind::RelLineTo
                    | GeometryRowKind::RelCubBezTo
                    | GeometryRowKind::RelQuadBezTo
                    | GeometryRowKind::RelEllipticalArcTo
            ) {
                Unit::Number
            } else {
                Unit::Inch
            };
            if !row.x.eq_ignore_ascii_case("Inh") {
                self.set_formula(format!("{prefix}.X{ix}"), row.x.clone(), Some(x_unit));
            }
            if !row.y.eq_ignore_ascii_case("Inh") {
                self.set_formula(format!("{prefix}.Y{ix}"), row.y.clone(), Some(x_unit));
            }
            let (a_unit, b_unit, c_unit, d_unit) = match row.kind {
                GeometryRowKind::ArcTo => (Some(Unit::Inch), None, None, None),
                GeometryRowKind::Ellipse => (
                    Some(Unit::Inch),
                    Some(Unit::Inch),
                    Some(Unit::Inch),
                    Some(Unit::Inch),
                ),
                GeometryRowKind::EllipticalArcTo => (
                    Some(Unit::Inch),
                    Some(Unit::Inch),
                    Some(Unit::Radian),
                    Some(Unit::Number),
                ),
                GeometryRowKind::RelEllipticalArcTo => (
                    Some(Unit::Number),
                    Some(Unit::Number),
                    Some(Unit::Radian),
                    Some(Unit::Number),
                ),
                GeometryRowKind::RelQuadBezTo => (Some(Unit::Number), Some(Unit::Number), None, None),
                GeometryRowKind::RelCubBezTo => (
                    Some(Unit::Number),
                    Some(Unit::Number),
                    Some(Unit::Number),
                    Some(Unit::Number),
                ),
                _ => (None, None, None, None),
            };
            if let Some(a) = &row.a {
                self.set_formula(format!("{prefix}.A{ix}"), a.clone(), a_unit);
            }
            if let Some(b) = &row.b {
                self.set_formula(format!("{prefix}.B{ix}"), b.clone(), b_unit);
            }
            if let Some(c) = &row.c {
                self.set_formula(format!("{prefix}.C{ix}"), c.clone(), c_unit);
            }
            if let Some(d) = &row.d {
                self.set_formula(format!("{prefix}.D{ix}"), d.clone(), d_unit);
            }
            if let Some(e) = &row.e {
                self.set_formula(format!("{prefix}.E{ix}"), e.clone(), None);
            }
        }
    }

    pub fn add_connection_point(&mut self, cp: ConnectionPointRow) {
        self.sync_connection_point_to_cells(&cp);
        self.connection_points.push(cp);
    }

    fn sync_connection_point_to_cells(&mut self, cp: &ConnectionPointRow) {
        let ix = cp.ix;
        self.set_formula(format!("Connections.X{ix}"), cp.x.clone(), Some(Unit::Inch));
        self.set_formula(format!("Connections.Y{ix}"), cp.y.clone(), Some(Unit::Inch));
        self.set_formula(format!("Connections.DirX{ix}"), cp.dir_x.clone(), Some(Unit::Inch));
        self.set_formula(format!("Connections.DirY{ix}"), cp.dir_y.clone(), Some(Unit::Inch));
        if !cp.name.is_empty() {
            self.set_formula(format!("Connections.{}.X", cp.name), cp.x.clone(), Some(Unit::Inch));
            self.set_formula(format!("Connections.{}.Y", cp.name), cp.y.clone(), Some(Unit::Inch));
        }
    }

    pub fn add_control(&mut self, ctrl: ControlRow) {
        self.sync_control_to_cells(&ctrl);
        self.controls.push(ctrl);
    }

    pub fn add_action(&mut self, act: ActionRow) {
        self.sync_action_to_cells(&act);
        if let Some(ex) = self
            .actions
            .iter_mut()
            .find(|a| a.name.eq_ignore_ascii_case(&act.name))
        {
            *ex = act;
        } else {
            self.actions.push(act);
        }
    }

    fn sync_action_to_cells(&mut self, act: &ActionRow) {
        let n = &act.name;
        let act_f = if act.action.trim().is_empty() {
            "0".to_string()
        } else {
            act.action.clone()
        };
        let menu_f = if act.menu.trim().starts_with('=') {
            act.menu.clone()
        } else {
            format!("\"{}\"", act.menu.trim_matches('"').replace('"', "\"\""))
        };
        let tag_f = format!("\"{}\"", act.tag_name.trim_matches('"').replace('"', "\"\""));
        let chk_f = if act.checked.trim().is_empty() { "FALSE".into() } else { act.checked.clone() };
        let dis_f = if act.disabled.trim().is_empty() { "FALSE".into() } else { act.disabled.clone() };
        let inv_f = if act.invisible.trim().is_empty() { "FALSE".into() } else { act.invisible.clone() };
        let bg_f = if act.begin_group { "TRUE" } else { "FALSE" };
        self.set_formula(format!("Actions.{n}.Action"), act_f, None);
        self.set_formula(format!("Actions.{n}.Menu"), menu_f, Some(Unit::String));
        self.set_formula(format!("Actions.{n}.TagName"), tag_f, Some(Unit::String));
        self.set_formula(format!("Actions.{n}.Checked"), chk_f, Some(Unit::Boolean));
        self.set_formula(format!("Actions.{n}.Disabled"), dis_f, Some(Unit::Boolean));
        self.set_formula(format!("Actions.{n}.Invisible"), inv_f, Some(Unit::Boolean));
        self.set_formula(format!("Actions.{n}.BeginGroup"), bg_f, Some(Unit::Boolean));
    }

    fn sync_control_to_cells(&mut self, ctrl: &ControlRow) {
        let n = &ctrl.name;
        // In Visio, `Controls.Row_1` evaluates to `Controls.Row_1.X`!
        self.set_formula(format!("Controls.{n}"), ctrl.x.clone(), Some(Unit::Inch));
        self.set_formula(format!("Controls.{n}.X"), ctrl.x.clone(), Some(Unit::Inch));
        self.set_formula(format!("Controls.{n}.Y"), ctrl.y.clone(), Some(Unit::Inch));
        self.set_formula(format!("Controls.{n}.XDyn"), ctrl.x_dyn.clone(), Some(Unit::Inch));
        self.set_formula(format!("Controls.{n}.YDyn"), ctrl.y_dyn.clone(), Some(Unit::Inch));
    }

    pub fn set_user_cell(&mut self, name: impl Into<String>, value_formula: impl Into<String>, prompt: impl Into<String>) {
        let name = name.into();
        let row = UserCellRow {
            name: name.clone(),
            value: value_formula.into(),
            prompt: prompt.into(),
        };
        self.sync_user_row_to_cells(&row);
        self.user_cells.insert(name.to_ascii_lowercase(), row);
    }

    fn sync_user_row_to_cells(&mut self, row: &UserCellRow) {
        self.set_formula(format!("User.{}", row.name), row.value.clone(), None);
        self.set_formula(format!("User.{}.Value", row.name), format!("=User.{}", row.name), None);
        self.set_formula(
            format!("User.{}.Prompt", row.name),
            format!("\"{}\"", row.prompt.replace('"', "\"\"")),
            Some(Unit::String),
        );
    }

    pub fn set_shape_data(&mut self, row: ShapeDataRow) {
        self.sync_shape_data_to_cells(&row);
        self.shape_data.insert(row.name.to_ascii_lowercase(), row);
    }

    fn sync_shape_data_to_cells(&mut self, row: &ShapeDataRow) {
        let unit = match row.data_type {
            ShapeDataType::Boolean => Some(Unit::Boolean),
            ShapeDataType::Currency => Some(Unit::Currency),
            ShapeDataType::String | ShapeDataType::FixedList | ShapeDataType::VariableList => Some(Unit::String),
            _ => None,
        };
        self.set_formula(format!("Prop.{}", row.name), row.value.clone(), unit);
        self.set_formula(format!("Prop.{}.Value", row.name), format!("=Prop.{}", row.name), unit);
        self.set_formula(
            format!("Prop.{}.Label", row.name),
            format!("\"{}\"", row.label.replace('"', "\"\"")),
            Some(Unit::String),
        );
        self.set_formula(
            format!("Prop.{}.Format", row.name),
            format!("\"{}\"", row.format.replace('"', "\"\"")),
            Some(Unit::String),
        );
    }
}

pub fn default_unit_for_cell(canonical_key: &str) -> Option<Unit> {
    match canonical_key {
        "pinx" | "piny" | "width" | "height" | "locpinx" | "locpiny" | "beginx" | "beginy"
        | "endx" | "endy" | "txtpinx" | "txtpiny" | "txtwidth" | "txtheight" | "txtlocpinx"
        | "txtlocpiny" | "pagewidth" | "pageheight" | "shdwoffsetx" | "shdwoffsety"
        | "rounding" | "leftmargin" | "rightmargin" | "topmargin" | "bottommargin"
        | "drawingscale" | "pagescale" | "xrulerorigin" | "yrulerorigin" | "xgridspacing"
        | "ygridspacing" => Some(Unit::Inch),
        "angle" | "txtangle" | "fillgradientangle" => Some(Unit::Radian),
        "lineweight" | "char.size" => Some(Unit::Point),
        "flipx" | "flipy" | "lockwidth" | "lockheight" | "lockaspect" | "lockmovex"
        | "lockmovey" | "lockrotate" | "lockdelete" | "lockselect" | "lockformat"
        | "locktextedit" | "char.strikethru" | "char.smallcaps" | "fillgradientenabled"
        | "hidetext" | "dontshow" => {
            Some(Unit::Boolean)
        }
        "fillforegnd" | "fillbkgnd" | "linecolor" | "char.color" | "shdwforegnd" => {
            Some(Unit::Color)
        }
        "char.font" => Some(Unit::String),
        "char.style"
        | "para.horzalign"
        | "verticalalign"
        | "para.bullet"
        | "textbkgndtrans"
        | "beginarrowsize"
        | "endarrowsize"
        | "fillgradientdir"
        | "quickstylefillmatrix"
        | "quickstylelinematrix"
        | "quickstylefontmatrix"
        | "quickstyleeffectsmatrix"
        | "quickstylefillcolor"
        | "quickstylelinecolor"
        | "quickstylefontcolor"
        | "quickstyleshadowcolor" => Some(Unit::Number),
        k if k.ends_with(".fillforegnd")
            || k.ends_with(".linecolor")
            || (k.starts_with("fillgradient.") && k.ends_with(".color")) =>
        {
            Some(Unit::Color)
        }
        k if k.ends_with(".lineweight") => Some(Unit::Point),
        k if k.starts_with("controls.")
            && (!k["controls.".len()..].contains('.')
                || k.ends_with(".x")
                || k.ends_with(".y")
                || k.ends_with(".xdyn")
                || k.ends_with(".ydyn")) =>
        {
            Some(Unit::Inch)
        }
        k if k.starts_with("connections.")
            && (k["connections.".len()..].starts_with('x')
                || k["connections.".len()..].starts_with('y')
                || k.ends_with(".x")
                || k.ends_with(".y")) =>
        {
            Some(Unit::Inch)
        }
        k if k.starts_with("actions.")
            && (k.ends_with(".checked")
                || k.ends_with(".disabled")
                || k.ends_with(".invisible")
                || k.ends_with(".begingroup")) =>
        {
            Some(Unit::Boolean)
        }
        k if k.starts_with("actions.") && (k.ends_with(".menu") || k.ends_with(".tagname")) => {
            Some(Unit::String)
        }
        _ => None,
    }
}

/// Standard 24-entry Microsoft Visio `[MS-VSDX]` color table (`IX=0..23`).
pub fn visio_standard_palette_color(idx: u32) -> Option<RgbaColor> {
    let (r, g, b) = match idx {
        0 => (0x00, 0x00, 0x00),
        1 => (0xFF, 0xFF, 0xFF),
        2 => (0xFF, 0x00, 0x00),
        3 => (0x00, 0xFF, 0x00),
        4 => (0x00, 0x00, 0xFF),
        5 => (0xFF, 0xFF, 0x00),
        6 => (0xFF, 0x00, 0xFF),
        7 => (0x00, 0xFF, 0xFF),
        8 => (0x80, 0x00, 0x00),
        9 => (0x00, 0x80, 0x00),
        10 => (0x00, 0x00, 0x80),
        11 => (0x80, 0x80, 0x00),
        12 => (0x80, 0x00, 0x80),
        13 => (0x00, 0x80, 0x80),
        14 => (0xC0, 0xC0, 0xC0),
        15 => (0x80, 0x80, 0x80),
        16 => (0x99, 0x99, 0xFF),
        17 => (0x99, 0x33, 0x66),
        18 => (0xFF, 0xFF, 0xCC),
        19 => (0xCC, 0xFF, 0xFF),
        20 => (0x66, 0x00, 0x66),
        21 => (0xFF, 0x80, 0x80),
        22 => (0x00, 0x66, 0xCC),
        23 => (0xCC, 0xCC, 0xFF),
        _ => return None,
    };
    Some(RgbaColor { r, g, b, a: 255 })
}

