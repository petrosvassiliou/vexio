//! Canonical Visio Document, Page, Master, Stencil, Shape, Style, Layer, Theme,
//! Markup, and Lossless OpenXML Opaque Bag intermediate representation (IR).

use crate::sections::ShapeSheet;
use crate::units::RgbaColor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeKind {
    Shape2D,
    Connector1D,
    Group,
    Guide,
    ForeignObject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GlueEndpoint {
    pub connector_shape_id: u64,
    /// `BeginX` or `EndX`
    pub from_cell: String,
    pub target_shape_id: u64,
    /// E.g., `Connections.X1` for static point glue, or `PinX` for dynamic shape-to-shape walking glue.
    pub to_cell: String,
    pub is_dynamic: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForeignMediaData {
    pub media_type: String, // "PNG", "JPEG", "SVG", "EMF", "OLE", "CAD"
    pub rel_id: Option<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisioShape {
    pub id: u64,
    pub name: String,
    pub name_u: String,
    pub kind: ShapeKind,
    pub master_id: Option<u32>,
    pub master_shape_id: Option<u64>,
    pub line_style_id: Option<u32>,
    pub fill_style_id: Option<u32>,
    pub text_style_id: Option<u32>,
    pub text: String,
    pub sheet: ShapeSheet,
    /// Sub-shapes when `kind == ShapeKind::Group` or Data Graphic callouts.
    pub children: Vec<VisioShape>,
    /// Layer indices assigned to this shape (`LayerMember`).
    pub layer_membership: Vec<u32>,
    /// Optional embedded raster/SVG/CAD/OLE payload.
    pub foreign_data: Option<ForeignMediaData>,
    /// Unrecognized XML elements/attributes preserved verbatim for lossless VSDX round-trip.
    pub opaque_xml_fragments: Vec<String>,
    /// Bound external data record key `(recordset_id, row_id)`.
    pub bound_data_row: Option<(u32, u64)>,
}

impl VisioShape {
    pub fn new_rect(id: u64, name: impl Into<String>, x: f64, y: f64, w: f64, h: f64) -> Self {
        let n = name.into();
        Self {
            id,
            name: n.clone(),
            name_u: n,
            kind: ShapeKind::Shape2D,
            master_id: None,
            master_shape_id: None,
            line_style_id: None,
            fill_style_id: None,
            text_style_id: None,
            text: String::new(),
            sheet: ShapeSheet::new_2d(x, y, w, h),
            children: Vec::new(),
            layer_membership: Vec::new(),
            foreign_data: None,
            opaque_xml_fragments: Vec::new(),
            bound_data_row: None,
        }
    }

    pub fn new_connector(
        id: u64,
        name: impl Into<String>,
        bx: f64,
        by: f64,
        ex: f64,
        ey: f64,
    ) -> Self {
        let n = name.into();
        Self {
            id,
            name: n.clone(),
            name_u: n,
            kind: ShapeKind::Connector1D,
            master_id: None,
            master_shape_id: None,
            line_style_id: None,
            fill_style_id: None,
            text_style_id: None,
            text: String::new(),
            sheet: ShapeSheet::new_1d_connector(bx, by, ex, ey),
            children: Vec::new(),
            layer_membership: Vec::new(),
            foreign_data: None,
            opaque_xml_fragments: Vec::new(),
            bound_data_row: None,
        }
    }

    pub fn find_shape(&self, id: u64) -> Option<&VisioShape> {
        if self.id == id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_shape(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_shape_mut(&mut self, id: u64) -> Option<&mut VisioShape> {
        if self.id == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find_shape_mut(id) {
                return Some(found);
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerDef {
    pub ix: u32,
    pub name: String,
    pub name_u: String,
    pub visible: bool,
    pub print: bool,
    pub active: bool,
    pub lock: bool,
    pub snap: bool,
    pub glue: bool,
    pub color_override: Option<RgbaColor>,
    pub transparency: f32,
}

impl LayerDef {
    pub fn new(ix: u32, name: impl Into<String>) -> Self {
        let n = name.into();
        Self {
            ix,
            name: n.clone(),
            name_u: n,
            visible: true,
            print: true,
            active: false,
            lock: false,
            snap: true,
            glue: true,
            color_override: None,
            transparency: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommentAnnotation {
    pub id: u32,
    pub author: String,
    pub initials: String,
    pub timestamp_iso: String,
    pub page_id: u32,
    pub shape_id: Option<u64>,
    pub x_in: f64,
    pub y_in: f64,
    pub text: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkupInkPoint {
    pub x_in: f64,
    pub y_in: f64,
    pub pressure: f32,
    pub tilt_x: f32,
    pub tilt_y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MarkupInkStroke {
    pub id: u64,
    pub color: RgbaColor,
    pub width_pt: f32,
    pub highlighter: bool,
    pub points: Vec<MarkupInkPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisioPage {
    pub id: u32,
    pub name: String,
    pub name_u: String,
    pub is_background: bool,
    pub back_page_id: Option<u32>,
    pub view_scale: f64,
    pub view_center_x: f64,
    pub view_center_y: f64,
    /// Page-level ShapeSheet (`PageWidth`, `PageHeight`, `PageScale`, `DrawingScale`, `RouteStyle`, etc.).
    pub page_sheet: ShapeSheet,
    pub layers: Vec<LayerDef>,
    pub shapes: Vec<VisioShape>,
    pub connects: Vec<GlueEndpoint>,
    pub ink_strokes: Vec<MarkupInkStroke>,
    pub opaque_xml_fragments: Vec<String>,
}

impl VisioPage {
    pub fn new(id: u32, name: impl Into<String>, width_in: f64, height_in: f64) -> Self {
        let n = name.into();
        let mut page_sheet = ShapeSheet::new();
        page_sheet.set_formula(
            "PageWidth",
            format!("{} in", crate::units::format_clean_f64(width_in)),
            Some(crate::units::Unit::Inch),
        );
        page_sheet.set_formula(
            "PageHeight",
            format!("{} in", crate::units::format_clean_f64(height_in)),
            Some(crate::units::Unit::Inch),
        );
        page_sheet.set_formula("PageScale", "1 in", Some(crate::units::Unit::Inch));
        page_sheet.set_formula("DrawingScale", "1 in", Some(crate::units::Unit::Inch));
        page_sheet.set_formula("RouteStyle", "1", Some(crate::units::Unit::Number));
        page_sheet.set_formula("PageLineJumpCode", "1", Some(crate::units::Unit::Number));
        page_sheet.set_formula("PageLineJumpStyle", "0", Some(crate::units::Unit::Number));
        page_sheet.set_formula("XGridSpacing", "0.25 in", Some(crate::units::Unit::Inch));
        page_sheet.set_formula("YGridSpacing", "0.25 in", Some(crate::units::Unit::Inch));
        Self {
            id,
            name: n.clone(),
            name_u: n,
            is_background: false,
            back_page_id: None,
            view_scale: 1.0,
            view_center_x: width_in * 0.5,
            view_center_y: height_in * 0.5,
            page_sheet,
            layers: vec![
                LayerDef::new(0, "Flowchart"),
                LayerDef::new(1, "Connector"),
                LayerDef::new(2, "Annotations"),
            ],
            shapes: Vec::new(),
            connects: Vec::new(),
            ink_strokes: Vec::new(),
            opaque_xml_fragments: Vec::new(),
        }
    }

    pub fn width_in(&self) -> f64 {
        self.page_sheet.get_f64("PageWidth").unwrap_or(8.5)
    }

    pub fn height_in(&self) -> f64 {
        self.page_sheet.get_f64("PageHeight").unwrap_or(11.0)
    }

    pub fn find_shape(&self, id: u64) -> Option<&VisioShape> {
        for s in &self.shapes {
            if let Some(found) = s.find_shape(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_shape_mut(&mut self, id: u64) -> Option<&mut VisioShape> {
        for s in &mut self.shapes {
            if let Some(found) = s.find_shape_mut(id) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_shape_by_name(&self, name: &str) -> Option<&VisioShape> {
        self.shapes.iter().find(|s| {
            s.name.eq_ignore_ascii_case(name) || s.name_u.eq_ignore_ascii_case(name)
        })
    }

    pub fn next_shape_id(&self) -> u64 {
        fn max_id(s: &VisioShape) -> u64 {
            s.children.iter().map(max_id).fold(s.id, u64::max)
        }
        self.shapes.iter().map(max_id).max().unwrap_or(0) + 1
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisioMaster {
    pub id: u32,
    pub name: String,
    pub name_u: String,
    pub base_id: String,
    pub unique_id: String,
    pub prompt: String,
    pub icon_bytes: Option<Vec<u8>>,
    pub page_sheet: ShapeSheet,
    pub shapes: Vec<VisioShape>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StencilPack {
    pub title: String,
    pub category: String,
    pub masters: Vec<VisioMaster>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyleSheetDef {
    pub id: u32,
    pub name: String,
    pub name_u: String,
    pub line_parent: Option<u32>,
    pub fill_parent: Option<u32>,
    pub text_parent: Option<u32>,
    pub sheet: ShapeSheet,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeDef {
    pub id: u32,
    pub name: String,
    pub accent_colors: [RgbaColor; 6],
    pub dark_color: RgbaColor,
    pub light_color: RgbaColor,
    pub background_color: RgbaColor,
    pub variant_index: u8,
    pub heading_font: String,
    pub body_font: String,
}

impl Default for ThemeDef {
    fn default() -> Self {
        Self {
            id: 1,
            name: "Vexio Office Classic".into(),
            accent_colors: [
                RgbaColor { r: 37, g: 99, b: 235, a: 255 },
                RgbaColor { r: 16, g: 185, b: 129, a: 255 },
                RgbaColor { r: 245, g: 158, b: 11, a: 255 },
                RgbaColor { r: 239, g: 68, b: 68, a: 255 },
                RgbaColor { r: 139, g: 92, b: 246, a: 255 },
                RgbaColor { r: 6, g: 182, b: 212, a: 255 },
            ],
            dark_color: RgbaColor { r: 15, g: 23, b: 42, a: 255 },
            light_color: RgbaColor::WHITE,
            background_color: RgbaColor::WHITE,
            variant_index: 0,
            heading_font: "Segoe UI".into(),
            body_font: "Segoe UI".into(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HeaderFooter {
    pub header_left: String,
    pub header_center: String,
    pub header_right: String,
    pub footer_left: String,
    pub footer_center: String,
    pub footer_right: String,
    pub header_margin_in: f64,
    pub footer_margin_in: f64,
    pub font_name: String,
    pub font_size_pt: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DocumentProperties {
    pub title: String,
    pub subject: String,
    pub creator: String,
    pub company: String,
    pub description: String,
    pub category: String,
    pub keywords: String,
    pub created_iso: String,
    pub modified_iso: String,
    pub custom_properties: BTreeMap<String, String>,
}

/// Preserves unknown OpenXML parts (`vbaProject.bin`, customXML, digital signatures, media, etc.)
/// byte-for-byte across read -> edit -> write cycles.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OpaqueOpcBag {
    /// Archive path (e.g. `visio/vbaproject.bin`) -> raw uncompressed bytes.
    pub parts: BTreeMap<String, Vec<u8>>,
    /// Extra content-type overrides `(part_name, content_type)`.
    pub content_type_overrides: BTreeMap<String, String>,
    /// Extra package/document relationships preserved verbatim `(source_rels_path, xml_lines)`.
    pub extra_relationships: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisioDocument {
    pub properties: DocumentProperties,
    pub doc_sheet: ShapeSheet,
    pub styles: Vec<StyleSheetDef>,
    pub themes: Vec<ThemeDef>,
    pub active_theme_id: Option<u32>,
    pub header_footer: HeaderFooter,
    pub masters: Vec<VisioMaster>,
    pub pages: Vec<VisioPage>,
    pub comments: Vec<CommentAnnotation>,
    pub opaque_bag: OpaqueOpcBag,
    pub macro_enabled: bool,
}

impl Default for VisioDocument {
    fn default() -> Self {
        Self::new_blank("Drawing1")
    }
}

impl VisioDocument {
    pub fn new_blank(title: impl Into<String>) -> Self {
        let mut doc_sheet = ShapeSheet::new();
        doc_sheet.set_formula("DocLangID", "1033", Some(crate::units::Unit::Number));
        doc_sheet.set_formula("OutputFormat", "0", Some(crate::units::Unit::Number));
        let mut props = DocumentProperties::default();
        props.title = title.into();
        props.creator = "Vexio".into();
        let default_style = StyleSheetDef {
            id: 0,
            name: "Normal".into(),
            name_u: "Normal".into(),
            line_parent: None,
            fill_parent: None,
            text_parent: None,
            sheet: ShapeSheet::new_2d(0.0, 0.0, 1.0, 1.0),
        };
        Self {
            properties: props,
            doc_sheet,
            styles: vec![default_style],
            themes: vec![ThemeDef::default()],
            active_theme_id: Some(1),
            header_footer: HeaderFooter {
                header_margin_in: 0.25,
                footer_margin_in: 0.25,
                font_name: "Segoe UI".into(),
                font_size_pt: 9.0,
                ..HeaderFooter::default()
            },
            masters: Vec::new(),
            pages: vec![VisioPage::new(1, "Page-1", 11.0, 8.5)],
            comments: Vec::new(),
            opaque_bag: OpaqueOpcBag::default(),
            macro_enabled: false,
        }
    }

    pub fn page(&self, id: u32) -> Option<&VisioPage> {
        self.pages.iter().find(|p| p.id == id)
    }

    pub fn page_mut(&mut self, id: u32) -> Option<&mut VisioPage> {
        self.pages.iter_mut().find(|p| p.id == id)
    }

    pub fn page_by_name(&self, name: &str) -> Option<&VisioPage> {
        self.pages.iter().find(|p| {
            p.name.eq_ignore_ascii_case(name) || p.name_u.eq_ignore_ascii_case(name)
        })
    }

    pub fn master(&self, id: u32) -> Option<&VisioMaster> {
        self.masters.iter().find(|m| m.id == id)
    }

    /// Instantiate a `VisioMaster` onto `page_id` at `(pin_x, pin_y)` in inches,
    /// establishing 3-tier ShapeSheet cell inheritance (`Shape` -> `Master` -> `Style`).
    pub fn drop_master(&mut self, page_id: u32, master_id: u32, pin_x: f64, pin_y: f64) -> Option<u64> {
        let master = self.master(master_id)?.clone();
        let template = master.shapes.first()?.clone();
        let page = self.page_mut(page_id)?;
        let new_id = page.next_shape_id();
        let mut instance = template;
        instance.id = new_id;
        instance.name = format!("{}.{}", master.name, new_id);
        instance.name_u = format!("{}.{}", master.name_u, new_id);
        instance.master_id = Some(master_id);
        instance.master_shape_id = master.shapes.first().map(|s| s.id);
        instance.sheet.set_formula(
            "PinX",
            format!("{} in", crate::units::format_clean_f64(pin_x)),
            Some(crate::units::Unit::Inch),
        );
        instance.sheet.set_formula(
            "PinY",
            format!("{} in", crate::units::format_clean_f64(pin_y)),
            Some(crate::units::Unit::Inch),
        );
        page.shapes.push(instance);
        Some(new_id)
    }
}
