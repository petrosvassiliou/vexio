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
        match &self.get_cell(name)?.value {
            CellValue::Color(c) => Some(*c),
            CellValue::Str(s) => RgbaColor::from_hex(s),
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
        let users: Vec<UserCellRow> = self.user_cells.values().cloned().collect();
        for u in &users {
            self.sync_user_row_to_cells(u);
        }
        let props: Vec<ShapeDataRow> = self.shape_data.values().cloned().collect();
        for p in &props {
            self.sync_shape_data_to_cells(p);
        }
    }

    fn sync_geometry_to_cells(&mut self, sec: &GeometrySection) {
        let prefix = format!("Geometry{}", sec.index);
        self.set_formula(format!("{prefix}.NoFill"), sec.no_fill.clone(), Some(Unit::Boolean));
        self.set_formula(format!("{prefix}.NoLine"), sec.no_line.clone(), Some(Unit::Boolean));
        self.set_formula(format!("{prefix}.NoShow"), sec.no_show.clone(), Some(Unit::Boolean));
        self.set_formula(format!("{prefix}.NoSnap"), sec.no_snap.clone(), Some(Unit::Boolean));
        self.set_formula(format!("{prefix}.NoQuickDrag"), sec.no_quick_drag.clone(), Some(Unit::Boolean));
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
            self.set_formula(format!("{prefix}.X{ix}"), row.x.clone(), Some(x_unit));
            self.set_formula(format!("{prefix}.Y{ix}"), row.y.clone(), Some(x_unit));
            if let Some(a) = &row.a {
                self.set_formula(format!("{prefix}.A{ix}"), a.clone(), None);
            }
            if let Some(b) = &row.b {
                self.set_formula(format!("{prefix}.B{ix}"), b.clone(), None);
            }
            if let Some(c) = &row.c {
                self.set_formula(format!("{prefix}.C{ix}"), c.clone(), None);
            }
            if let Some(d) = &row.d {
                self.set_formula(format!("{prefix}.D{ix}"), d.clone(), None);
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
        "angle" | "txtangle" => Some(Unit::Radian),
        "lineweight" | "char.size" => Some(Unit::Point),
        "flipx" | "flipy" | "lockwidth" | "lockheight" | "lockaspect" | "lockmovex"
        | "lockmovey" | "lockrotate" | "lockdelete" | "lockselect" | "lockformat"
        | "locktextedit" => Some(Unit::Boolean),
        "fillforegnd" | "fillbkgnd" | "linecolor" | "char.color" | "shdwforegnd" => {
            Some(Unit::Color)
        }
        k if k.ends_with(".fillforegnd") || k.ends_with(".linecolor") => Some(Unit::Color),
        k if k.ends_with(".lineweight") => Some(Unit::Point),
        _ => None,
    }
}
