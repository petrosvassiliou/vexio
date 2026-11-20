//! Visio 6 Themes × 4 Variants Matrix (`modern`, `professional`, `vibrant`, `blueprint`,
//! `monochrome`, `hand_drawn`) and Structured Shape Visual Effects API (Linear/Radial Gradients,
//! Vector Hatches, Soft Drop Shadows, 3D Bevels/Extrusions, and Sketchy Hand-Drawn Outlines).

use serde::{Deserialize, Serialize};
use vexio_shapesheet::{
    ReactiveEngine, RgbaColor, ShapeKind, ShapeSheet, ThemeDef, Unit, VisioDocument, VisioShape,
    format_clean_f64,
};

/// Canonical Visio built-in theme identifiers (`6 themes × 4 variants = 24 palettes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ThemePresetId {
    Modern,
    Professional,
    Vibrant,
    Blueprint,
    Monochrome,
    HandDrawn,
}

impl ThemePresetId {
    pub const ALL: [Self; 6] = [
        Self::Modern,
        Self::Professional,
        Self::Vibrant,
        Self::Blueprint,
        Self::Monochrome,
        Self::HandDrawn,
    ];

    pub const fn id(self) -> u32 {
        match self {
            Self::Modern => 1,
            Self::Professional => 2,
            Self::Vibrant => 3,
            Self::Blueprint => 4,
            Self::Monochrome => 5,
            Self::HandDrawn => 6,
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Modern => "modern",
            Self::Professional => "professional",
            Self::Vibrant => "vibrant",
            Self::Blueprint => "blueprint",
            Self::Monochrome => "monochrome",
            Self::HandDrawn => "hand_drawn",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Modern => "Modern",
            Self::Professional => "Professional",
            Self::Vibrant => "Vibrant",
            Self::Blueprint => "Blueprint",
            Self::Monochrome => "Monochrome",
            Self::HandDrawn => "Hand Drawn",
        }
    }

    pub fn from_slug(slug: &str) -> Option<Self> {
        let norm = slug
            .trim()
            .to_ascii_lowercase()
            .replace(['-', ' '], "_");
        match norm.as_str() {
            "modern" => Some(Self::Modern),
            "professional" => Some(Self::Professional),
            "vibrant" => Some(Self::Vibrant),
            "blueprint" => Some(Self::Blueprint),
            "monochrome" => Some(Self::Monochrome),
            "hand_drawn" | "handdrawn" | "sketch" => Some(Self::HandDrawn),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct ThemeVariantTokens {
    theme_def: ThemeDef,
    shape_fill_foregnd: RgbaColor,
    shape_fill_bkgnd: RgbaColor,
    shape_fill_pattern: u8,
    shape_gradient_angle_deg: f64,
    shape_stroke: RgbaColor,
    connector_stroke: RgbaColor,
    shape_text: RgbaColor,
    sketch_enabled: bool,
    sketch_amount: f64,
}

const fn rgb(r: u8, g: u8, b: u8) -> RgbaColor {
    RgbaColor { r, g, b, a: 255 }
}

fn build_theme_variant_tokens(preset: ThemePresetId, variant_index: u8) -> ThemeVariantTokens {
    let v = variant_index.min(3);
    match preset {
        ThemePresetId::Modern => {
            let accents = [
                rgb(37, 99, 235),
                rgb(16, 185, 129),
                rgb(245, 158, 11),
                rgb(239, 68, 68),
                rgb(139, 92, 246),
                rgb(6, 182, 212),
            ];
            let (bg, fill_fg, fill_bg, stroke, text, pat) = match v {
                0 => (
                    rgb(255, 255, 255),
                    rgb(239, 246, 255),
                    rgb(219, 234, 254),
                    rgb(37, 99, 235),
                    rgb(15, 23, 42),
                    28,
                ),
                1 => (
                    rgb(248, 250, 252),
                    rgb(37, 99, 235),
                    rgb(29, 78, 216),
                    rgb(30, 64, 175),
                    rgb(255, 255, 255),
                    28,
                ),
                2 => (
                    rgb(240, 253, 250),
                    rgb(209, 250, 229),
                    rgb(167, 243, 208),
                    rgb(5, 150, 105),
                    rgb(6, 78, 59),
                    25,
                ),
                _ => (
                    rgb(15, 23, 42),
                    rgb(30, 41, 59),
                    rgb(51, 65, 85),
                    rgb(96, 165, 250),
                    rgb(248, 250, 252),
                    28,
                ),
            };
            ThemeVariantTokens {
                theme_def: ThemeDef {
                    id: preset.id(),
                    name: preset.display_name().into(),
                    accent_colors: accents,
                    dark_color: rgb(15, 23, 42),
                    light_color: rgb(255, 255, 255),
                    background_color: bg,
                    variant_index: v,
                    heading_font: "Segoe UI Semibold".into(),
                    body_font: "Segoe UI".into(),
                },
                shape_fill_foregnd: fill_fg,
                shape_fill_bkgnd: fill_bg,
                shape_fill_pattern: pat,
                shape_gradient_angle_deg: 90.0,
                shape_stroke: stroke,
                connector_stroke: stroke,
                shape_text: text,
                sketch_enabled: false,
                sketch_amount: 0.0,
            }
        }
        ThemePresetId::Professional => {
            let accents = [
                rgb(30, 58, 138),
                rgb(15, 118, 110),
                rgb(180, 83, 9),
                rgb(153, 27, 27),
                rgb(88, 28, 135),
                rgb(51, 65, 85),
            ];
            let (bg, fill_fg, fill_bg, stroke, text, pat) = match v {
                0 => (
                    rgb(255, 255, 255),
                    rgb(241, 245, 249),
                    rgb(226, 232, 240),
                    rgb(30, 58, 138),
                    rgb(15, 23, 42),
                    1,
                ),
                1 => (
                    rgb(248, 250, 252),
                    rgb(30, 58, 138),
                    rgb(23, 37, 84),
                    rgb(15, 23, 42),
                    rgb(255, 255, 255),
                    28,
                ),
                2 => (
                    rgb(250, 250, 249),
                    rgb(226, 242, 240),
                    rgb(204, 251, 241),
                    rgb(15, 118, 110),
                    rgb(19, 78, 74),
                    1,
                ),
                _ => (
                    rgb(23, 37, 84),
                    rgb(30, 58, 138),
                    rgb(30, 64, 175),
                    rgb(191, 219, 254),
                    rgb(248, 250, 252),
                    28,
                ),
            };
            ThemeVariantTokens {
                theme_def: ThemeDef {
                    id: preset.id(),
                    name: preset.display_name().into(),
                    accent_colors: accents,
                    dark_color: rgb(15, 23, 42),
                    light_color: rgb(255, 255, 255),
                    background_color: bg,
                    variant_index: v,
                    heading_font: "Calibri".into(),
                    body_font: "Calibri".into(),
                },
                shape_fill_foregnd: fill_fg,
                shape_fill_bkgnd: fill_bg,
                shape_fill_pattern: pat,
                shape_gradient_angle_deg: 90.0,
                shape_stroke: stroke,
                connector_stroke: stroke,
                shape_text: text,
                sketch_enabled: false,
                sketch_amount: 0.0,
            }
        }
        ThemePresetId::Vibrant => {
            let accents = [
                rgb(225, 29, 72),
                rgb(249, 115, 22),
                rgb(234, 179, 8),
                rgb(16, 185, 129),
                rgb(99, 102, 241),
                rgb(217, 70, 239),
            ];
            let (bg, fill_fg, fill_bg, stroke, text, pat) = match v {
                0 => (
                    rgb(255, 251, 235),
                    rgb(225, 29, 72),
                    rgb(244, 63, 94),
                    rgb(159, 18, 57),
                    rgb(255, 255, 255),
                    30,
                ),
                1 => (
                    rgb(255, 255, 255),
                    rgb(249, 115, 22),
                    rgb(251, 146, 60),
                    rgb(194, 65, 12),
                    rgb(255, 255, 255),
                    30,
                ),
                2 => (
                    rgb(245, 243, 255),
                    rgb(99, 102, 241),
                    rgb(139, 92, 246),
                    rgb(67, 56, 202),
                    rgb(255, 255, 255),
                    30,
                ),
                _ => (
                    rgb(24, 24, 27),
                    rgb(217, 70, 239),
                    rgb(168, 85, 247),
                    rgb(240, 171, 252),
                    rgb(255, 255, 255),
                    36,
                ),
            };
            ThemeVariantTokens {
                theme_def: ThemeDef {
                    id: preset.id(),
                    name: preset.display_name().into(),
                    accent_colors: accents,
                    dark_color: rgb(24, 24, 27),
                    light_color: rgb(255, 255, 255),
                    background_color: bg,
                    variant_index: v,
                    heading_font: "Trebuchet MS".into(),
                    body_font: "Segoe UI".into(),
                },
                shape_fill_foregnd: fill_fg,
                shape_fill_bkgnd: fill_bg,
                shape_fill_pattern: pat,
                shape_gradient_angle_deg: 45.0,
                shape_stroke: stroke,
                connector_stroke: stroke,
                shape_text: text,
                sketch_enabled: false,
                sketch_amount: 0.0,
            }
        }
        ThemePresetId::Blueprint => {
            let accents = [
                rgb(56, 189, 248),
                rgb(125, 211, 252),
                rgb(34, 211, 238),
                rgb(250, 204, 21),
                rgb(148, 163, 184),
                rgb(255, 255, 255),
            ];
            let (bg, fill_fg, fill_bg, stroke, text) = match v {
                0 => (
                    rgb(12, 45, 87),
                    rgb(19, 64, 116),
                    rgb(12, 45, 87),
                    rgb(186, 230, 253),
                    rgb(240, 249, 255),
                ),
                1 => (
                    rgb(7, 29, 58),
                    rgb(15, 52, 96),
                    rgb(7, 29, 58),
                    rgb(56, 189, 248),
                    rgb(224, 242, 254),
                ),
                2 => (
                    rgb(240, 249, 255),
                    rgb(224, 242, 254),
                    rgb(186, 230, 253),
                    rgb(3, 105, 161),
                    rgb(12, 74, 110),
                ),
                _ => (
                    rgb(6, 38, 45),
                    rgb(13, 62, 72),
                    rgb(6, 38, 45),
                    rgb(103, 232, 249),
                    rgb(236, 254, 255),
                ),
            };
            ThemeVariantTokens {
                theme_def: ThemeDef {
                    id: preset.id(),
                    name: preset.display_name().into(),
                    accent_colors: accents,
                    dark_color: rgb(7, 29, 58),
                    light_color: rgb(240, 249, 255),
                    background_color: bg,
                    variant_index: v,
                    heading_font: "Consolas".into(),
                    body_font: "Consolas".into(),
                },
                shape_fill_foregnd: fill_fg,
                shape_fill_bkgnd: fill_bg,
                shape_fill_pattern: 1,
                shape_gradient_angle_deg: 0.0,
                shape_stroke: stroke,
                connector_stroke: stroke,
                shape_text: text,
                sketch_enabled: false,
                sketch_amount: 0.0,
            }
        }
        ThemePresetId::Monochrome => {
            let accents = [
                rgb(17, 24, 39),
                rgb(55, 65, 81),
                rgb(107, 114, 128),
                rgb(156, 163, 175),
                rgb(209, 213, 219),
                rgb(243, 244, 246),
            ];
            let (bg, fill_fg, fill_bg, stroke, text) = match v {
                0 => (
                    rgb(255, 255, 255),
                    rgb(249, 250, 251),
                    rgb(243, 244, 246),
                    rgb(17, 24, 39),
                    rgb(17, 24, 39),
                ),
                1 => (
                    rgb(243, 244, 246),
                    rgb(229, 231, 235),
                    rgb(209, 213, 219),
                    rgb(31, 41, 55),
                    rgb(17, 24, 39),
                ),
                2 => (
                    rgb(255, 255, 255),
                    rgb(31, 41, 55),
                    rgb(17, 24, 39),
                    rgb(17, 24, 39),
                    rgb(249, 250, 251),
                ),
                _ => (
                    rgb(17, 24, 39),
                    rgb(31, 41, 55),
                    rgb(55, 65, 81),
                    rgb(229, 231, 235),
                    rgb(249, 250, 251),
                ),
            };
            ThemeVariantTokens {
                theme_def: ThemeDef {
                    id: preset.id(),
                    name: preset.display_name().into(),
                    accent_colors: accents,
                    dark_color: rgb(17, 24, 39),
                    light_color: rgb(255, 255, 255),
                    background_color: bg,
                    variant_index: v,
                    heading_font: "Inter".into(),
                    body_font: "Inter".into(),
                },
                shape_fill_foregnd: fill_fg,
                shape_fill_bkgnd: fill_bg,
                shape_fill_pattern: 1,
                shape_gradient_angle_deg: 0.0,
                shape_stroke: stroke,
                connector_stroke: stroke,
                shape_text: text,
                sketch_enabled: false,
                sketch_amount: 0.0,
            }
        }
        ThemePresetId::HandDrawn => {
            let accents = [
                rgb(180, 83, 9),
                rgb(37, 99, 235),
                rgb(22, 101, 52),
                rgb(185, 28, 28),
                rgb(109, 40, 217),
                rgb(71, 85, 105),
            ];
            let (bg, fill_fg, fill_bg, stroke, text, amount) = match v {
                0 => (
                    rgb(254, 252, 232),
                    rgb(254, 249, 195),
                    rgb(253, 230, 138),
                    rgb(68, 64, 60),
                    rgb(41, 37, 36),
                    0.35,
                ),
                1 => (
                    rgb(255, 255, 255),
                    rgb(224, 242, 254),
                    rgb(186, 230, 253),
                    rgb(30, 58, 138),
                    rgb(30, 41, 59),
                    0.45,
                ),
                2 => (
                    rgb(245, 245, 244),
                    rgb(220, 252, 231),
                    rgb(187, 247, 208),
                    rgb(20, 83, 45),
                    rgb(20, 83, 45),
                    0.40,
                ),
                _ => (
                    rgb(41, 37, 36),
                    rgb(68, 64, 60),
                    rgb(87, 83, 78),
                    rgb(253, 230, 138),
                    rgb(250, 250, 249),
                    0.50,
                ),
            };
            ThemeVariantTokens {
                theme_def: ThemeDef {
                    id: preset.id(),
                    name: preset.display_name().into(),
                    accent_colors: accents,
                    dark_color: rgb(41, 37, 36),
                    light_color: rgb(254, 252, 232),
                    background_color: bg,
                    variant_index: v,
                    heading_font: "Segoe Print".into(),
                    body_font: "Segoe Print".into(),
                },
                shape_fill_foregnd: fill_fg,
                shape_fill_bkgnd: fill_bg,
                shape_fill_pattern: 1,
                shape_gradient_angle_deg: 0.0,
                shape_stroke: stroke,
                connector_stroke: stroke,
                shape_text: text,
                sketch_enabled: true,
                sketch_amount: amount,
            }
        }
    }
}

/// Return all 6 built-in Visio themes (`modern`, `professional`, `vibrant`, `blueprint`,
/// `monochrome`, `hand_drawn`) at default variant `0`.
pub fn list_builtin_themes() -> Vec<ThemeDef> {
    ThemePresetId::ALL
        .into_iter()
        .map(|preset| build_theme_variant_tokens(preset, 0).theme_def)
        .collect()
}

/// Return the specific `ThemeDef` for `(preset, variant_index)` where `variant_index` is `0..=3`.
pub fn get_theme_variant_def(preset: ThemePresetId, variant_index: u8) -> ThemeDef {
    build_theme_variant_tokens(preset, variant_index).theme_def
}

/// Apply `(theme_slug, variant_index)` (`0..=3`) across `doc`, updating `doc.themes`,
/// `doc.active_theme_id`, `PageColor` on all pages, and harmonizing unguarded shape cells
/// (`FillForegnd`, `FillBkgnd`, `FillPattern`, `LineColor`, `Char.Color`, `Char.Font`,
/// and `LineSketchEnabled = TRUE` when `theme_slug == "hand_drawn"`).
/// Returns the number of shapes updated.
pub fn apply_document_theme_variant(
    doc: &mut VisioDocument,
    theme_slug: &str,
    variant_index: u8,
) -> usize {
    let Some(preset) = ThemePresetId::from_slug(theme_slug) else {
        return 0;
    };
    let tokens = build_theme_variant_tokens(preset, variant_index);

    if let Some(existing) = doc.themes.iter_mut().find(|t| t.id == tokens.theme_def.id) {
        *existing = tokens.theme_def.clone();
    } else {
        doc.themes.push(tokens.theme_def.clone());
    }
    doc.active_theme_id = Some(tokens.theme_def.id);
    doc.header_footer.font_name = tokens.theme_def.body_font.clone();

    let _ = set_cell_respecting_guard(
        &mut doc.doc_sheet,
        "ThemeIndex",
        format!("{}", tokens.theme_def.id),
        Unit::Number,
    );
    let _ = set_cell_respecting_guard(
        &mut doc.doc_sheet,
        "VariationColorIndex",
        format!("{}", tokens.theme_def.variant_index),
        Unit::Number,
    );

    let bg_formula = color_formula(tokens.theme_def.background_color);
    let mut updated_shapes = 0usize;

    for page in &mut doc.pages {
        let _ = set_cell_respecting_guard(
            &mut page.page_sheet,
            "PageColor",
            bg_formula.clone(),
            Unit::Color,
        );
        for shape in &mut page.shapes {
            updated_shapes += harmonize_shape_tree(shape, &tokens);
        }
    }

    ReactiveEngine::recalculate_document(doc);
    updated_shapes
}

fn harmonize_shape_tree(shape: &mut VisioShape, tokens: &ThemeVariantTokens) -> usize {
    if matches!(shape.kind, ShapeKind::Guide | ShapeKind::ForeignObject) {
        return 0;
    }
    let mut count = 0usize;
    let lock_format = shape.sheet.get_bool("LockFormat").unwrap_or(false);
    if !lock_format {
        let font_quoted = format!("\"{}\"", tokens.theme_def.body_font.replace('"', "\"\""));
        if shape.kind == ShapeKind::Connector1D {
            let _ = set_cell_respecting_guard(
                &mut shape.sheet,
                "LineColor",
                color_formula(tokens.connector_stroke),
                Unit::Color,
            );
        } else {
            let cur_pat = shape.sheet.get_f64("FillPattern").unwrap_or(1.0) as i32;
            if cur_pat != 0 {
                let _ = set_cell_respecting_guard(
                    &mut shape.sheet,
                    "FillForegnd",
                    color_formula(tokens.shape_fill_foregnd),
                    Unit::Color,
                );
                let _ = set_cell_respecting_guard(
                    &mut shape.sheet,
                    "FillBkgnd",
                    color_formula(tokens.shape_fill_bkgnd),
                    Unit::Color,
                );
                let _ = set_cell_respecting_guard(
                    &mut shape.sheet,
                    "FillPattern",
                    format!("{}", tokens.shape_fill_pattern),
                    Unit::Number,
                );
                if tokens.shape_fill_pattern >= 25 {
                    let _ = set_cell_respecting_guard(
                        &mut shape.sheet,
                        "FillGradientAngle",
                        format!("{} deg", format_clean_f64(tokens.shape_gradient_angle_deg)),
                        Unit::Degree,
                    );
                }
            }
            let _ = set_cell_respecting_guard(
                &mut shape.sheet,
                "LineColor",
                color_formula(tokens.shape_stroke),
                Unit::Color,
            );
        }

        let _ = set_cell_respecting_guard(
            &mut shape.sheet,
            "Char.Color",
            color_formula(tokens.shape_text),
            Unit::Color,
        );
        let _ = set_cell_respecting_guard(&mut shape.sheet, "Char.Font", font_quoted, Unit::String);

        let sketch_bool = if tokens.sketch_enabled { "TRUE" } else { "FALSE" };
        let _ = set_cell_respecting_guard(
            &mut shape.sheet,
            "LineSketchEnabled",
            sketch_bool.to_string(),
            Unit::Boolean,
        );
        if tokens.sketch_enabled {
            let deterministic_seed = (shape.id.wrapping_mul(2654435761) & 0xFFFF) as u32;
            let _ = set_cell_respecting_guard(
                &mut shape.sheet,
                "LineSketchSeed",
                format!("{deterministic_seed}"),
                Unit::Number,
            );
            let _ = set_cell_respecting_guard(
                &mut shape.sheet,
                "LineSketchAmount",
                format_clean_f64(tokens.sketch_amount),
                Unit::Number,
            );
        }
        count += 1;
    }

    for child in &mut shape.children {
        count += harmonize_shape_tree(child, tokens);
    }
    count
}

// ============================================================================
// Gradients, Vector Hatches, Drop Shadows, 3D Bevels & Sketchy Outlines API
// ============================================================================

/// Canonical Visio gradient fill pattern codes (`FillPattern`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GradientDirection {
    /// `FillPattern = 25` (Linear horizontal left-to-right).
    LinearHorizontal,
    /// `FillPattern = 28` (Linear vertical top-to-bottom).
    LinearVertical,
    /// `FillPattern = 30` (Linear diagonal).
    LinearDiagonal,
    /// `FillPattern = 36` (Radial from center).
    RadialCenter,
}

impl GradientDirection {
    pub const fn fill_pattern_code(self) -> u8 {
        match self {
            Self::LinearHorizontal => 25,
            Self::LinearVertical => 28,
            Self::LinearDiagonal => 30,
            Self::RadialCenter => 36,
        }
    }
}

/// Linear or radial gradient fill specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GradientFillSpec {
    pub direction: GradientDirection,
    pub foregnd: RgbaColor,
    pub bkgnd: RgbaColor,
    pub angle_deg: f64,
}

/// Canonical Visio vector hatch fill pattern codes (`FillPattern = 2..=12`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HatchPatternKind {
    Horizontal = 2,
    Vertical = 3,
    UpwardDiagonal = 4,
    DownwardDiagonal = 5,
    Crosshatch = 6,
    DiagonalCrosshatch = 7,
    SparseDots = 8,
    MediumDots = 9,
    DenseDots = 10,
    Trellis = 11,
    GridDots = 12,
}

impl HatchPatternKind {
    pub const fn fill_pattern_code(self) -> u8 {
        self as u8
    }
}

/// Vector hatch fill specification (`FillPattern` in `2..=12`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HatchFillSpec {
    pub kind: HatchPatternKind,
    pub foregnd: RgbaColor,
    pub bkgnd: RgbaColor,
}

/// Soft drop shadow specification (`ShdwPattern`, `ShdwForegnd`, `ShdwOffsetX/Y`, `ShdwBlur`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DropShadowSpec {
    pub enabled: bool,
    pub foregnd: RgbaColor,
    pub offset_x_in: f64,
    pub offset_y_in: f64,
    pub blur_pt: f64,
}

/// 3D bevel and extrusion specification (`BevelTopType` `1..=4`, `ExtrusionDepth`, `ExtrusionAngle`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BevelExtrusionSpec {
    /// `0` = None, `1` = Circle/Round, `2` = Relaxed Inset, `3` = Cross/Slope, `4` = Art Deco.
    pub bevel_top_type: u8,
    pub extrusion_depth_in: f64,
    pub extrusion_angle_deg: f64,
}

/// Sketchy / hand-drawn outline specification (`LineSketchEnabled`, `LineSketchSeed`, `LineSketchAmount`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SketchOutlineSpec {
    pub enabled: bool,
    pub seed: u32,
    pub amount: f64,
}

/// Composite visual effects bundle applied atomically to a shape's ShapeSheet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapeVisualStyleSpec {
    pub gradient: Option<GradientFillSpec>,
    pub hatch: Option<HatchFillSpec>,
    pub shadow: Option<DropShadowSpec>,
    pub bevel: Option<BevelExtrusionSpec>,
    pub sketch: Option<SketchOutlineSpec>,
}

/// Apply structured visual effects (`gradient`, `hatch`, `shadow`, `bevel`, `sketch`) to
/// `(page_id, shape_id)` while strictly honoring `LockFormat` and per-cell `GUARD(...)`.
pub fn apply_visual_effects_to_shape(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    spec: &ShapeVisualStyleSpec,
) -> bool {
    let applied = {
        let Some(page) = doc.page_mut(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape_mut(shape_id) else {
            return false;
        };
        if shape.sheet.get_bool("LockFormat").unwrap_or(false) {
            return false;
        }
        let ss = &mut shape.sheet;

        if let Some(grad) = &spec.gradient {
            let _ = set_cell_respecting_guard(
                ss,
                "FillPattern",
                format!("{}", grad.direction.fill_pattern_code()),
                Unit::Number,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "FillForegnd",
                color_formula(grad.foregnd),
                Unit::Color,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "FillBkgnd",
                color_formula(grad.bkgnd),
                Unit::Color,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "FillGradientAngle",
                format!("{} deg", format_clean_f64(grad.angle_deg)),
                Unit::Degree,
            );
        }

        if let Some(hatch) = &spec.hatch {
            let _ = set_cell_respecting_guard(
                ss,
                "FillPattern",
                format!("{}", hatch.kind.fill_pattern_code()),
                Unit::Number,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "FillForegnd",
                color_formula(hatch.foregnd),
                Unit::Color,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "FillBkgnd",
                color_formula(hatch.bkgnd),
                Unit::Color,
            );
        }

        if let Some(shdw) = &spec.shadow {
            let pat = if shdw.enabled { "1" } else { "0" };
            let _ = set_cell_respecting_guard(ss, "ShdwPattern", pat.into(), Unit::Number);
            let _ = set_cell_respecting_guard(
                ss,
                "ShdwForegnd",
                color_formula(shdw.foregnd),
                Unit::Color,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "ShdwOffsetX",
                format!("{} in", format_clean_f64(shdw.offset_x_in)),
                Unit::Inch,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "ShdwOffsetY",
                format!("{} in", format_clean_f64(shdw.offset_y_in)),
                Unit::Inch,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "ShdwBlur",
                format!("{} pt", format_clean_f64(shdw.blur_pt.max(0.0))),
                Unit::Point,
            );
        }

        if let Some(bevel) = &spec.bevel {
            let clamped_type = bevel.bevel_top_type.min(4);
            let _ = set_cell_respecting_guard(
                ss,
                "BevelTopType",
                format!("{clamped_type}"),
                Unit::Number,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "ExtrusionDepth",
                format!("{} in", format_clean_f64(bevel.extrusion_depth_in.max(0.0))),
                Unit::Inch,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "ExtrusionAngle",
                format!("{} deg", format_clean_f64(bevel.extrusion_angle_deg)),
                Unit::Degree,
            );
        }

        if let Some(sketch) = &spec.sketch {
            let flag = if sketch.enabled { "TRUE" } else { "FALSE" };
            let _ = set_cell_respecting_guard(ss, "LineSketchEnabled", flag.into(), Unit::Boolean);
            let _ = set_cell_respecting_guard(
                ss,
                "LineSketchSeed",
                format!("{}", sketch.seed),
                Unit::Number,
            );
            let _ = set_cell_respecting_guard(
                ss,
                "LineSketchAmount",
                format_clean_f64(sketch.amount.clamp(0.0, 2.0)),
                Unit::Number,
            );
        }

        true
    };

    if applied {
        ReactiveEngine::recalculate_document(doc);
    }
    applied
}

fn color_formula(c: RgbaColor) -> String {
    format!("=RGB({},{},{})", c.r, c.g, c.b)
}

fn set_cell_respecting_guard(
    sheet: &mut ShapeSheet,
    name: &str,
    formula: String,
    unit: Unit,
) -> bool {
    if sheet
        .try_set_formula_respecting_guard(name, formula, false)
        .is_ok()
    {
        if let Some(cell) = sheet.get_cell_mut(name) {
            cell.unit_hint = Some(unit);
        }
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_themes_times_four_variants_harmonize_pages_and_preserve_guarded_cells() {
        let builtins = list_builtin_themes();
        assert_eq!(builtins.len(), 6);

        let mut doc = VisioDocument::new_blank("ThemesMatrixTest");
        {
            let page = doc.page_mut(1).unwrap();
            let s1 = VisioShape::new_rect(1, "NormalBox", 2.0, 3.0, 2.0, 1.5);
            let mut s2 = VisioShape::new_rect(2, "GuardedBox", 6.0, 3.0, 2.0, 1.5);
            s2.sheet
                .set_formula("FillForegnd", "=GUARD(RGB(255,0,0))", Some(Unit::Color));
            page.shapes.push(s1);
            page.shapes.push(s2);
        }
        ReactiveEngine::recalculate_document(&mut doc);

        // Verify every theme slug across all 4 variants (`0..=3`) produces distinct valid palettes.
        for slug in [
            "modern",
            "professional",
            "vibrant",
            "blueprint",
            "monochrome",
            "hand_drawn",
        ] {
            let mut seen_bgs = Vec::new();
            for v in 0..=3 {
                let updated = apply_document_theme_variant(&mut doc, slug, v);
                assert_eq!(updated, 2);
                let page_bg = doc
                    .page(1)
                    .unwrap()
                    .page_sheet
                    .get_color("PageColor")
                    .expect("PageColor set");
                seen_bgs.push(page_bg);

                // Guarded `FillForegnd` on `s2` MUST remain pure red `RGB(255, 0, 0)`!
                let guarded_fg = doc
                    .page(1)
                    .unwrap()
                    .find_shape(2)
                    .unwrap()
                    .sheet
                    .get_color("FillForegnd")
                    .unwrap();
                assert_eq!(guarded_fg, rgb(255, 0, 0));
            }
            // Ensure variants within a theme have distinct palette colors.
            assert!(
                seen_bgs.windows(2).any(|w| w[0] != w[1]),
                "Theme {slug} variants should not all have identical backgrounds"
            );
        }

        // Last applied theme was `hand_drawn` -> `LineSketchEnabled` MUST be TRUE on unguarded shapes!
        let s1 = doc.page(1).unwrap().find_shape(1).unwrap();
        assert_eq!(s1.sheet.get_bool("LineSketchEnabled"), Some(true));
        assert!(s1.sheet.get_f64("LineSketchAmount").unwrap_or(0.0) > 0.1);
    }

    #[test]
    fn gradients_hatches_shadows_bevels_and_sketchy_effects_persist_and_respect_guard() {
        let mut doc = VisioDocument::new_blank("EffectsTest");
        {
            let page = doc.page_mut(1).unwrap();
            let mut s = VisioShape::new_rect(1, "StyledCard", 4.0, 4.0, 3.0, 2.0);
            // Guard `ShdwOffsetX` to verify GUARD protection during visual effects application.
            s.sheet
                .set_formula("ShdwOffsetX", "=GUARD(0.25 in)", Some(Unit::Inch));
            page.shapes.push(s);
        }
        ReactiveEngine::recalculate_document(&mut doc);

        // 1. Apply Radial Gradient + Soft Drop Shadow + 3D Bevel + Sketchy Outline.
        let fx_grad = ShapeVisualStyleSpec {
            gradient: Some(GradientFillSpec {
                direction: GradientDirection::RadialCenter,
                foregnd: rgb(59, 130, 246),
                bkgnd: rgb(29, 78, 216),
                angle_deg: 35.0,
            }),
            hatch: None,
            shadow: Some(DropShadowSpec {
                enabled: true,
                foregnd: rgb(15, 23, 42),
                offset_x_in: 0.08,
                offset_y_in: -0.08,
                blur_pt: 6.0,
            }),
            bevel: Some(BevelExtrusionSpec {
                bevel_top_type: 2,
                extrusion_depth_in: 0.18,
                extrusion_angle_deg: 45.0,
            }),
            sketch: Some(SketchOutlineSpec {
                enabled: true,
                seed: 4242,
                amount: 0.42,
            }),
        };
        assert!(apply_visual_effects_to_shape(&mut doc, 1, 1, &fx_grad));

        let sh = doc.page(1).unwrap().find_shape(1).unwrap();
        assert_eq!(sh.sheet.get_f64("FillPattern"), Some(36.0));
        assert_eq!(sh.sheet.get_color("FillForegnd"), Some(rgb(59, 130, 246)));
        assert_eq!(sh.sheet.get_color("FillBkgnd"), Some(rgb(29, 78, 216)));
        assert!((sh.sheet.get_f64("FillGradientAngle").unwrap() - 35.0_f64.to_radians()).abs() < 1e-9);
        assert_eq!(sh.sheet.get_f64("ShdwPattern"), Some(1.0));
        // Guarded `ShdwOffsetX` MUST remain `0.25 in` while unguarded `ShdwOffsetY` updates to `-0.08 in`!
        assert!((sh.sheet.get_f64("ShdwOffsetX").unwrap() - 0.25).abs() < 1e-9);
        assert!((sh.sheet.get_f64("ShdwOffsetY").unwrap() - (-0.08)).abs() < 1e-9);
        assert!((sh.sheet.get_f64("ShdwBlur").unwrap() - (6.0 / 72.0)).abs() < 1e-9);
        assert_eq!(sh.sheet.get_f64("BevelTopType"), Some(2.0));
        assert!((sh.sheet.get_f64("ExtrusionDepth").unwrap() - 0.18).abs() < 1e-9);
        assert!((sh.sheet.get_f64("ExtrusionAngle").unwrap() - 45.0_f64.to_radians()).abs() < 1e-9);
        assert_eq!(sh.sheet.get_bool("LineSketchEnabled"), Some(true));
        assert_eq!(sh.sheet.get_f64("LineSketchSeed"), Some(4242.0));
        assert!((sh.sheet.get_f64("LineSketchAmount").unwrap() - 0.42).abs() < 1e-9);

        // 2. Apply Diagonal Crosshatch pattern (`FillPattern = 7`).
        let fx_hatch = ShapeVisualStyleSpec {
            hatch: Some(HatchFillSpec {
                kind: HatchPatternKind::DiagonalCrosshatch,
                foregnd: rgb(30, 41, 59),
                bkgnd: rgb(248, 250, 252),
            }),
            ..Default::default()
        };
        assert!(apply_visual_effects_to_shape(&mut doc, 1, 1, &fx_hatch));
        let sh_hatched = doc.page(1).unwrap().find_shape(1).unwrap();
        assert_eq!(sh_hatched.sheet.get_f64("FillPattern"), Some(7.0));
    }
}
