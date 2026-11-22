//! TrueType / OpenType proportional glyph shaper, pair-kerning engine, Lyon Bezier outline
//! tessellator, OS system font resolver, and vector PDF contour emitter (`ttf-parser`).
//!
//! Guarantees 100% offline deterministic rendering across `x86_64`, `aarch64`, and
//! `wasm32-unknown-unknown` using bundled Bitstream Vera / DejaVu TrueType faces
//! (`DejaVuSans`, `DejaVuSans-Bold`, `DejaVuSerif`, `DejaVuSansMono`) while seamlessly
//! upgrading to installed OS TrueType/OpenType fonts (`Calibri`, `Segoe UI`, `SF Pro`,
//! `Helvetica Neue`, `Arial`, `Roboto`, `Times New Roman`, `Consolas`, etc.) or runtime-registered
//! custom `.ttf`/`.otf` blobs when available.

use crate::tessellate::{GpuVertex, PRIM_FLAG_TEXT_GLYPH};
use crate::typography::FontFamilyClass;
use lyon_path::math::point;
use lyon_path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin,
    StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use std::collections::BTreeMap;
use std::sync::{OnceLock, RwLock};
use ttf_parser::{Face, GlyphId, OutlineBuilder};
use vexio_shapesheet::RgbaColor;

static DEJAVU_SANS_REG_DEFLATE: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSans.ttf.deflate");
static DEJAVU_SANS_BOLD_DEFLATE: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf.deflate");
static DEJAVU_SERIF_REG_DEFLATE: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSerif.ttf.deflate");
static DEJAVU_MONO_REG_DEFLATE: &[u8] =
    include_bytes!("../assets/fonts/DejaVuSansMono.ttf.deflate");

static DEJAVU_SANS_REG: OnceLock<Vec<u8>> = OnceLock::new();
static DEJAVU_SANS_BOLD: OnceLock<Vec<u8>> = OnceLock::new();
static DEJAVU_SERIF_REG: OnceLock<Vec<u8>> = OnceLock::new();
static DEJAVU_MONO_REG: OnceLock<Vec<u8>> = OnceLock::new();

fn inflate_bundled_font(deflated: &[u8]) -> Vec<u8> {
    miniz_oxide::inflate::decompress_to_vec(deflated).unwrap_or_default()
}

fn bundled_sans_regular() -> &'static [u8] {
    DEJAVU_SANS_REG.get_or_init(|| inflate_bundled_font(DEJAVU_SANS_REG_DEFLATE))
}

fn bundled_sans_bold() -> &'static [u8] {
    DEJAVU_SANS_BOLD.get_or_init(|| inflate_bundled_font(DEJAVU_SANS_BOLD_DEFLATE))
}

fn bundled_serif_regular() -> &'static [u8] {
    DEJAVU_SERIF_REG.get_or_init(|| inflate_bundled_font(DEJAVU_SERIF_REG_DEFLATE))
}

fn bundled_mono_regular() -> &'static [u8] {
    DEJAVU_MONO_REG.get_or_init(|| inflate_bundled_font(DEJAVU_MONO_REG_DEFLATE))
}

#[derive(Clone)]
struct RegisteredFontEntry {
    bytes: std::sync::Arc<Vec<u8>>,
    face_index: u32,
    is_bold: bool,
    is_italic: bool,
}

#[derive(Default)]
struct FontRegistry {
    /// Lowercase normalized family key -> registered or discovered font variants.
    families: BTreeMap<String, Vec<RegisteredFontEntry>>,
    /// Tracks families we have already probed on the host OS filesystem.
    #[cfg(not(target_arch = "wasm32"))]
    probed_os_families: std::collections::BTreeSet<String>,
}

static GLOBAL_FONT_REGISTRY: OnceLock<RwLock<FontRegistry>> = OnceLock::new();

fn font_registry() -> &'static RwLock<FontRegistry> {
    GLOBAL_FONT_REGISTRY.get_or_init(|| RwLock::new(FontRegistry::default()))
}

fn normalize_family_key(name: &str) -> String {
    name.trim()
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_lowercase()
}

/// Register raw `.ttf` / `.otf` / `.ttc` font bytes at runtime (used by native shells and
/// ChromeOS PWA Local Font Access API `FontData.blob()`).
/// Returns the number of valid font faces indexed from `bytes`.
pub fn register_font_bytes(family_hint: &str, bytes: Vec<u8>) -> usize {
    if bytes.len() < 12 {
        return 0;
    }
    let count = ttf_parser::fonts_in_collection(&bytes).unwrap_or(1).min(16);
    let shared = std::sync::Arc::new(bytes);
    let mut added = 0usize;

    let Ok(mut reg) = font_registry().write() else {
        return 0;
    };

    for idx in 0..count {
        let Ok(face) = Face::parse(&shared, idx) else {
            continue;
        };
        let is_bold = face.is_bold();
        let is_italic = face.is_italic();
        let entry = RegisteredFontEntry {
            bytes: std::sync::Arc::clone(&shared),
            face_index: idx,
            is_bold,
            is_italic,
        };

        let mut keys = std::collections::BTreeSet::new();
        let hint_key = normalize_family_key(family_hint);
        if !hint_key.is_empty() {
            keys.insert(hint_key);
        }
        for name_rec in face.names() {
            if matches!(name_rec.name_id, 1 | 4 | 16) {
                if let Some(s) = name_rec.to_string() {
                    let k = normalize_family_key(&s);
                    if !k.is_empty() {
                        keys.insert(k);
                    }
                }
            }
        }
        for k in keys {
            reg.families.entry(k).or_default().push(entry.clone());
        }
        added += 1;
    }
    added
}

#[cfg(not(target_arch = "wasm32"))]
fn candidate_os_font_filenames(family_key: &str, bold: bool, italic: bool) -> Vec<&'static str> {
    match (family_key, bold, italic) {
        ("calibri", true, true) => vec!["calibriz.ttf", "Carlito-BoldItalic.ttf"],
        ("calibri", true, false) => vec!["calibrib.ttf", "Carlito-Bold.ttf", "calibri.ttf"],
        ("calibri", false, true) => vec!["calibrii.ttf", "Carlito-Italic.ttf", "calibri.ttf"],
        ("calibri", false, false) => vec!["calibri.ttf", "Carlito-Regular.ttf"],
        ("segoe ui", true, true) => vec!["segoeuiz.ttf", "seguisbi.ttf"],
        ("segoe ui", true, false) => vec!["segoeuib.ttf", "seguisb.ttf", "segoeui.ttf"],
        ("segoe ui", false, true) => vec!["segoeuii.ttf", "segoeui.ttf"],
        ("segoe ui", false, false) => vec!["segoeui.ttf"],
        ("arial" | "helvetica" | "helvetica neue" | "sans-serif", true, true) => vec![
            "arialbi.ttf",
            "Arial Bold Italic.ttf",
            "LiberationSans-BoldItalic.ttf",
        ],
        ("arial" | "helvetica" | "helvetica neue" | "sans-serif", true, false) => vec![
            "arialbd.ttf",
            "Arial Bold.ttf",
            "HelveticaNeue.ttc",
            "LiberationSans-Bold.ttf",
            "Roboto-Bold.ttf",
        ],
        ("arial" | "helvetica" | "helvetica neue" | "sans-serif", false, true) => vec![
            "ariali.ttf",
            "Arial Italic.ttf",
            "LiberationSans-Italic.ttf",
            "Roboto-Italic.ttf",
        ],
        ("arial" | "helvetica" | "helvetica neue" | "sans-serif", false, false) => vec![
            "arial.ttf",
            "Arial.ttf",
            "Helvetica.ttc",
            "HelveticaNeue.ttc",
            "LiberationSans-Regular.ttf",
            "Roboto-Regular.ttf",
        ],
        ("sf pro" | "sf pro text" | "sf pro display" | ".applesystemuifont", true, _) => {
            vec!["SFNS.ttf", "SF-Pro-Text-Bold.otf", "HelveticaNeue.ttc"]
        }
        ("sf pro" | "sf pro text" | "sf pro display" | ".applesystemuifont", false, _) => {
            vec!["SFNS.ttf", "SF-Pro-Text-Regular.otf", "HelveticaNeue.ttc"]
        }
        ("roboto" | "inter", true, true) => vec!["Roboto-BoldItalic.ttf", "Inter-BoldItalic.otf"],
        ("roboto" | "inter", true, false) => vec!["Roboto-Bold.ttf", "Inter-Bold.otf"],
        ("roboto" | "inter", false, true) => vec!["Roboto-Italic.ttf", "Inter-Italic.otf"],
        ("roboto" | "inter", false, false) => vec!["Roboto-Regular.ttf", "Inter-Regular.otf"],
        ("times new roman" | "times" | "georgia" | "cambria", true, _) => vec![
            "timesbd.ttf",
            "Times New Roman Bold.ttf",
            "georgiab.ttf",
            "LiberationSerif-Bold.ttf",
            "DejaVuSerif-Bold.ttf",
        ],
        ("times new roman" | "times" | "georgia" | "cambria", false, _) => vec![
            "times.ttf",
            "Times New Roman.ttf",
            "georgia.ttf",
            "LiberationSerif-Regular.ttf",
            "DejaVuSerif.ttf",
        ],
        ("consolas" | "courier new" | "menlo" | "sf mono" | "jetbrains mono", true, _) => vec![
            "consolab.ttf",
            "courbd.ttf",
            "Menlo.ttc",
            "DejaVuSansMono-Bold.ttf",
        ],
        ("consolas" | "courier new" | "menlo" | "sf mono" | "jetbrains mono", false, _) => vec![
            "consola.ttf",
            "cour.ttf",
            "Menlo.ttc",
            "Courier New.ttf",
            "RobotoMono-Regular.ttf",
            "DejaVuSansMono.ttf",
        ],
        _ => Vec::new(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn probe_os_font_family(family_key: &str) {
    if family_key.is_empty() {
        return;
    }
    {
        let Ok(reg) = font_registry().read() else {
            return;
        };
        if reg.probed_os_families.contains(family_key) {
            return;
        }
    }
    if let Ok(mut reg) = font_registry().write() {
        if !reg.probed_os_families.insert(family_key.to_string()) {
            return;
        }
    }

    let search_dirs = [
        "/System/Library/Fonts",
        "/System/Library/Fonts/Supplemental",
        "/Library/Fonts",
        "C:\\Windows\\Fonts",
        "/system/fonts",
        "/usr/share/fonts/truetype/dejavu",
        "/usr/share/fonts/truetype/liberation",
        "/usr/share/fonts/truetype/crosextra",
        "/usr/share/fonts/truetype",
    ];

    for (b, it) in [(false, false), (true, false), (false, true), (true, true)] {
        for fname in candidate_os_font_filenames(family_key, b, it) {
            for dir in &search_dirs {
                let path = std::path::Path::new(dir).join(fname);
                if let Ok(bytes) = std::fs::read(&path) {
                    if register_font_bytes(family_key, bytes) > 0 {
                        break;
                    }
                }
            }
        }
    }
}

/// Resolved font binary + face metadata ready for `ttf_parser::Face::parse`.
pub struct ResolvedFontData {
    bytes_arc: Option<std::sync::Arc<Vec<u8>>>,
    bytes_static: Option<&'static [u8]>,
    pub face_index: u32,
    pub native_bold: bool,
    pub native_italic: bool,
}

impl ResolvedFontData {
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        if let Some(s) = self.bytes_static {
            s
        } else if let Some(a) = &self.bytes_arc {
            a.as_slice()
        } else {
            &[]
        }
    }

    #[inline]
    pub fn parse_face(&self) -> Option<Face<'_>> {
        Face::parse(self.as_bytes(), self.face_index).ok()
    }
}

/// Resolve the best matching TrueType/OpenType font face for `(family_name, class, bold, italic)`.
pub fn resolve_font_data(
    family_name: &str,
    family_class: FontFamilyClass,
    want_bold: bool,
    want_italic: bool,
) -> ResolvedFontData {
    let key = normalize_family_key(family_name);
    #[cfg(not(target_arch = "wasm32"))]
    if !key.is_empty() {
        probe_os_font_family(&key);
    }

    if !key.is_empty() {
        if let Ok(reg) = font_registry().read() {
            if let Some(variants) = reg.families.get(&key) {
                if let Some(best) = variants.iter().max_by_key(|v| {
                    let mut score = 0i32;
                    if v.is_bold == want_bold {
                        score += 4;
                    }
                    if v.is_italic == want_italic {
                        score += 2;
                    }
                    score
                }) {
                    return ResolvedFontData {
                        bytes_arc: Some(std::sync::Arc::clone(&best.bytes)),
                        bytes_static: None,
                        face_index: best.face_index,
                        native_bold: best.is_bold,
                        native_italic: best.is_italic,
                    };
                }
            }
        }
    }

    match family_class {
        FontFamilyClass::Monospace => ResolvedFontData {
            bytes_arc: None,
            bytes_static: Some(bundled_mono_regular()),
            face_index: 0,
            native_bold: false,
            native_italic: false,
        },
        FontFamilyClass::Serif => ResolvedFontData {
            bytes_arc: None,
            bytes_static: Some(bundled_serif_regular()),
            face_index: 0,
            native_bold: false,
            native_italic: false,
        },
        FontFamilyClass::SansSerif => {
            if want_bold {
                ResolvedFontData {
                    bytes_arc: None,
                    bytes_static: Some(bundled_sans_bold()),
                    face_index: 0,
                    native_bold: true,
                    native_italic: false,
                }
            } else {
                ResolvedFontData {
                    bytes_arc: None,
                    bytes_static: Some(bundled_sans_regular()),
                    face_index: 0,
                    native_bold: false,
                    native_italic: false,
                }
            }
        }
    }
}

/// Fallback bundled universal Unicode Sans face (`DejaVuSans` / `DejaVuSans-Bold`) for scripts
/// (Greek, Cyrillic, Hebrew, Arabic, arrows/math) missing from a narrower OS font.
pub fn bundled_unicode_fallback(want_bold: bool) -> ResolvedFontData {
    if want_bold {
        ResolvedFontData {
            bytes_arc: None,
            bytes_static: Some(bundled_sans_bold()),
            face_index: 0,
            native_bold: true,
            native_italic: false,
        }
    } else {
        ResolvedFontData {
            bytes_arc: None,
            bytes_static: Some(bundled_sans_regular()),
            face_index: 0,
            native_bold: false,
            native_italic: false,
        }
    }
}

/// Look up TrueType `kern` table horizontal pair adjustment in font units.
#[inline]
pub fn lookup_pair_kerning_units(face: &Face<'_>, prev_gid: GlyphId, cur_gid: GlyphId) -> i16 {
    let Some(kern) = face.tables().kern else {
        return 0;
    };
    let mut total: i32 = 0;
    for sub in kern.subtables {
        if sub.horizontal && !sub.has_cross_stream {
            if let Some(k) = sub.glyphs_kerning(prev_gid, cur_gid) {
                total += k as i32;
            }
        }
    }
    total.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Compute proportional character advance in inches `(advance_in, glyph_id)` using `face`,
/// returning `None` if `ch` is not covered by `face` (`GlyphId(0)` `.notdef`).
#[inline]
pub fn glyph_advance_in(
    face: &Face<'_>,
    ch: char,
    prev_gid: Option<GlyphId>,
    font_size_pt: f64,
    synthetic_bold: bool,
) -> Option<(f64, GlyphId)> {
    let gid = face.glyph_index(ch)?;
    if gid.0 == 0 && !ch.is_whitespace() {
        return None;
    }
    let upem = (face.units_per_em().max(64)) as f64;
    let font_in = font_size_pt.clamp(4.0, 288.0) / 72.0;
    let raw_adv = face.glyph_hor_advance(gid).unwrap_or((upem * 0.55) as u16) as f64;
    let kern = prev_gid
        .map(|pg| lookup_pair_kerning_units(face, pg, gid) as f64)
        .unwrap_or(0.0);
    let bold_pad = if synthetic_bold { upem * 0.035 } else { 0.0 };
    let adv_in = ((raw_adv + kern + bold_pad).max(upem * 0.12) / upem) * font_in;
    Some((adv_in, gid))
}

/// Measure total proportional + pair-kerned advance width in inches for a single text run.
pub fn measure_text_width_in(
    text: &str,
    font_family: &str,
    font_size_pt: f64,
    bold: bool,
    italic: bool,
) -> f64 {
    if text.is_empty() {
        return 0.0;
    }
    let fam_class = FontFamilyClass::from_family_name(font_family);
    let primary = resolve_font_data(font_family, fam_class, bold, italic);
    let fallback = bundled_unicode_fallback(bold);
    let pri_face = primary.parse_face();
    let fb_face = fallback.parse_face();
    let syn_bold_pri = bold && !primary.native_bold;
    let syn_bold_fb = bold && !fallback.native_bold;
    let font_in = font_size_pt.clamp(4.0, 288.0) / 72.0;

    let mut prev_pri: Option<GlyphId> = None;
    let mut prev_fb: Option<GlyphId> = None;
    let mut total_w = 0.0f64;

    for ch in text.chars() {
        if let Some(ref pf) = pri_face {
            if let Some((adv, gid)) = glyph_advance_in(pf, ch, prev_pri, font_size_pt, syn_bold_pri)
            {
                total_w += adv;
                prev_pri = Some(gid);
                prev_fb = None;
                continue;
            }
        }
        if let Some(ref fbf) = fb_face {
            if let Some((adv, gid)) = glyph_advance_in(fbf, ch, prev_fb, font_size_pt, syn_bold_fb)
            {
                total_w += adv;
                prev_fb = Some(gid);
                prev_pri = None;
                continue;
            }
        }
        total_w += font_in * 0.62;
        prev_pri = None;
        prev_fb = None;
    }
    total_w
}

struct LyonGlyphBuilder<'a> {
    builder: &'a mut lyon_path::path::Builder,
    origin_local_x: f64,
    baseline_local_y: f64,
    scale: f64,
    italic_shear: f64,
    pin_x_in: f64,
    pin_y_in: f64,
    sin_a: f64,
    cos_a: f64,
    subpath_open: bool,
    emitted_any: bool,
}

impl<'a> LyonGlyphBuilder<'a> {
    #[inline]
    fn map_pt(&self, fx: f32, fy: f32) -> lyon_path::math::Point {
        let gx = (fx as f64) * self.scale;
        let gy = (fy as f64) * self.scale;
        let lx = self.origin_local_x + gx + gy * self.italic_shear;
        let ly = self.baseline_local_y + gy;
        let px = self.pin_x_in + lx * self.cos_a - ly * self.sin_a;
        let py = self.pin_y_in + lx * self.sin_a + ly * self.cos_a;
        point(px as f32, py as f32)
    }
}

impl OutlineBuilder for LyonGlyphBuilder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.subpath_open {
            self.builder.end(true);
        }
        self.builder.begin(self.map_pt(x, y));
        self.subpath_open = true;
        self.emitted_any = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if !self.subpath_open {
            self.move_to(x, y);
            return;
        }
        self.builder.line_to(self.map_pt(x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        if !self.subpath_open {
            self.move_to(x, y);
            return;
        }
        self.builder
            .quadratic_bezier_to(self.map_pt(x1, y1), self.map_pt(x, y));
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        if !self.subpath_open {
            self.move_to(x, y);
            return;
        }
        self.builder.cubic_bezier_to(
            self.map_pt(x1, y1),
            self.map_pt(x2, y2),
            self.map_pt(x, y),
        );
    }

    fn close(&mut self) {
        if self.subpath_open {
            self.builder.end(true);
            self.subpath_open = false;
        }
    }
}

/// Append a single TrueType/OpenType glyph outline into `builder`, returning `true` if the glyph
/// exists in `face` (including valid whitespace glyphs, which return `true` with zero contours).
#[allow(clippy::too_many_arguments)]
pub fn append_glyph_to_lyon_builder(
    face: &Face<'_>,
    gid: GlyphId,
    ch: char,
    origin_local_x: f64,
    baseline_local_y: f64,
    font_size_pt: f64,
    synthetic_italic: bool,
    pin_x_in: f64,
    pin_y_in: f64,
    sin_a: f64,
    cos_a: f64,
    builder: &mut lyon_path::path::Builder,
) -> bool {
    if ch.is_whitespace() {
        return true;
    }
    if gid.0 == 0 {
        return false;
    }
    let upem = (face.units_per_em().max(64)) as f64;
    let font_in = font_size_pt.clamp(4.0, 288.0) / 72.0;
    let scale = font_in / upem;
    let italic_shear = if synthetic_italic { 0.20 } else { 0.0 };

    let mut gb = LyonGlyphBuilder {
        builder,
        origin_local_x,
        baseline_local_y,
        scale,
        italic_shear,
        pin_x_in,
        pin_y_in,
        sin_a,
        cos_a,
        subpath_open: false,
        emitted_any: false,
    };
    let has_bbox = face.outline_glyph(gid, &mut gb).is_some();
    if gb.subpath_open {
        gb.builder.end(true);
    }
    has_bbox || gb.emitted_any
}

/// Tessellate a batched `lyon_path::Path` of TrueType glyph contours into `lyon_buf`.
pub fn tessellate_glyph_outline_batch(
    path: &Path,
    color: RgbaColor,
    font_size_pt: f64,
    synthetic_bold: bool,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
) {
    let font_in = (font_size_pt.clamp(4.0, 288.0) / 72.0) as f32;
    let tol = (font_in * 0.012).clamp(0.00018, 0.0025);
    let rgba = color.to_linear_f32();

    let _ = fill_tess.tessellate_path(
        path,
        &FillOptions::tolerance(tol).with_fill_rule(FillRule::NonZero),
        &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_TEXT_GLYPH, 0.0])
        }),
    );

    if synthetic_bold {
        let embolden_w = (font_in * 0.036).clamp(0.0012, 0.045);
        let _ = stroke_tess.tessellate_path(
            path,
            &StrokeOptions::tolerance(tol)
                .with_line_width(embolden_w)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round),
            &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
                let p = v.position();
                GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_TEXT_GLYPH, 0.0])
            }),
        );
    }
}

/// Vector contour command in page-space points (`1 in = 72 pt`) for crisp PDF glyph emission.
#[derive(Debug, Clone, Copy)]
pub enum PdfContourOp {
    MoveTo(f64, f64),
    LineTo(f64, f64),
    CubicTo(f64, f64, f64, f64, f64, f64),
    Close,
}

struct PdfGlyphOutlineCollector<'a> {
    ops: &'a mut Vec<PdfContourOp>,
    origin_local_x_in: f64,
    baseline_local_y_in: f64,
    scale_in: f64,
    italic_shear: f64,
    pin_x_in: f64,
    pin_y_in: f64,
    sin_a: f64,
    cos_a: f64,
    cur_pt: [f64; 2],
    open: bool,
}

impl<'a> PdfGlyphOutlineCollector<'a> {
    #[inline]
    fn map_pdf_pt(&self, fx: f32, fy: f32) -> [f64; 2] {
        let gx = (fx as f64) * self.scale_in;
        let gy = (fy as f64) * self.scale_in;
        let lx = self.origin_local_x_in + gx + gy * self.italic_shear;
        let ly = self.baseline_local_y_in + gy;
        let px_in = self.pin_x_in + lx * self.cos_a - ly * self.sin_a;
        let py_in = self.pin_y_in + lx * self.sin_a + ly * self.cos_a;
        [px_in * 72.0, py_in * 72.0]
    }
}

impl OutlineBuilder for PdfGlyphOutlineCollector<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        if self.open {
            self.ops.push(PdfContourOp::Close);
        }
        let p = self.map_pdf_pt(x, y);
        self.ops.push(PdfContourOp::MoveTo(p[0], p[1]));
        self.cur_pt = p;
        self.open = true;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map_pdf_pt(x, y);
        self.ops.push(PdfContourOp::LineTo(p[0], p[1]));
        self.cur_pt = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.cur_pt;
        let q = self.map_pdf_pt(x1, y1);
        let p2 = self.map_pdf_pt(x, y);
        // Exact degree elevation from quadratic Bezier `(p0, q, p2)` to cubic `(p0, c1, c2, p2)`!
        let c1 = [p0[0] + (2.0 / 3.0) * (q[0] - p0[0]), p0[1] + (2.0 / 3.0) * (q[1] - p0[1])];
        let c2 = [p2[0] + (2.0 / 3.0) * (q[0] - p2[0]), p2[1] + (2.0 / 3.0) * (q[1] - p2[1])];
        self.ops
            .push(PdfContourOp::CubicTo(c1[0], c1[1], c2[0], c2[1], p2[0], p2[1]));
        self.cur_pt = p2;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.map_pdf_pt(x1, y1);
        let c2 = self.map_pdf_pt(x2, y2);
        let p = self.map_pdf_pt(x, y);
        self.ops
            .push(PdfContourOp::CubicTo(c1[0], c1[1], c2[0], c2[1], p[0], p[1]));
        self.cur_pt = p;
    }

    fn close(&mut self) {
        if self.open {
            self.ops.push(PdfContourOp::Close);
            self.open = false;
        }
    }
}

/// Emit exact PDF cubic Bezier contour operations (`m`, `l`, `c`, `h`) in PDF points (`72 pt/in`)
/// for a single line of Unicode text centered/aligned at `(origin_local_x_in, baseline_local_y_in)`
/// inside a shape rotated by `angle_rad` around `(pin_x_in, pin_y_in)`.
#[allow(clippy::too_many_arguments)]
pub fn shape_line_to_pdf_contours(
    text: &str,
    font_family: &str,
    font_size_pt: f64,
    bold: bool,
    italic: bool,
    horiz_align: u8,
    box_w_in: f64,
    pad_in: f64,
    baseline_local_y_in: f64,
    pin_x_in: f64,
    pin_y_in: f64,
    angle_rad: f64,
) -> (Vec<PdfContourOp>, bool) {
    let fam_class = FontFamilyClass::from_family_name(font_family);
    let primary = resolve_font_data(font_family, fam_class, bold, italic);
    let fallback = bundled_unicode_fallback(bold);
    let Some(pri_face) = primary.parse_face() else {
        return (Vec::new(), false);
    };
    let fb_face = fallback.parse_face();
    let syn_bold_pri = bold && !primary.native_bold;
    let syn_italic_pri = italic && !primary.native_italic;

    // 1. Measure advances across the line.
    let mut positioned: Vec<(char, bool, GlyphId, f64)> = Vec::with_capacity(text.chars().count());
    let mut prev_pri: Option<GlyphId> = None;
    let mut prev_fb: Option<GlyphId> = None;
    let mut total_w = 0.0f64;

    for ch in text.chars() {
        if let Some((adv, gid)) = glyph_advance_in(&pri_face, ch, prev_pri, font_size_pt, syn_bold_pri) {
            positioned.push((ch, true, gid, adv));
            prev_pri = Some(gid);
            prev_fb = None;
            total_w += adv;
        } else if let Some(ref fbf) = fb_face {
            if let Some((adv, gid)) = glyph_advance_in(fbf, ch, prev_fb, font_size_pt, bold && !fallback.native_bold) {
                positioned.push((ch, false, gid, adv));
                prev_fb = Some(gid);
                prev_pri = None;
                total_w += adv;
            }
        }
    }

    let mut cur_x = match horiz_align {
        0 => -box_w_in * 0.5 + pad_in,
        2 => (box_w_in * 0.5 - pad_in - total_w).max(-box_w_in * 0.5 + pad_in),
        _ => -total_w * 0.5,
    };

    let (sin_a, cos_a) = angle_rad.sin_cos();
    let font_in = font_size_pt.clamp(4.0, 288.0) / 72.0;
    let mut ops = Vec::new();

    for (ch, use_pri, gid, adv) in positioned {
        if !ch.is_whitespace() && gid.0 != 0 {
            let (face_ref, italic_sh) = if use_pri {
                (&pri_face, if syn_italic_pri { 0.20 } else { 0.0 })
            } else if let Some(ref fbf) = fb_face {
                (fbf, if italic { 0.20 } else { 0.0 })
            } else {
                (&pri_face, 0.0)
            };
            let upem = (face_ref.units_per_em().max(64)) as f64;
            let mut collector = PdfGlyphOutlineCollector {
                ops: &mut ops,
                origin_local_x_in: cur_x,
                baseline_local_y_in,
                scale_in: font_in / upem,
                italic_shear: italic_sh,
                pin_x_in,
                pin_y_in,
                sin_a,
                cos_a,
                cur_pt: [0.0, 0.0],
                open: false,
            };
            let _ = face_ref.outline_glyph(gid, &mut collector);
            if collector.open {
                collector.ops.push(PdfContourOp::Close);
            }
        }
        cur_x += adv;
    }

    (ops, syn_bold_pri)
}
