//! Vector path tessellation (`lyon_path` + `lyon_tessellation`), level-of-detail (LOD)
//! sub-3px proxy quads, selection/ruler/ink geometry builders, and content-addressed `MeshCache`.

use lyon_path::math::point;
use lyon_path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin,
    StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_4, PI, TAU};
use vexio_shapesheet::{
    EvaluatedGeometryPath, EvaluatedPathVerb, EvaluatedShapeVisual, MarkupInkStroke, RgbaColor,
    ShapeKind,
};

/// Primitive kind constants stored in `GpuVertex::flags[0]`.
pub const PRIM_FLAG_PAGE_GRID: f32 = 0.0;
pub const PRIM_FLAG_SHAPE: f32 = 1.0;
pub const PRIM_FLAG_OVERLAY_UI: f32 = 2.0;
pub const PRIM_FLAG_MARKUP_INK: f32 = 3.0;
pub const PRIM_FLAG_TEXT_GLYPH: f32 = 4.0;

/// GPU-ready vertex layout (`32 bytes`, 16-byte aligned friendly) consumed by `canvas.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
    pub flags: [f32; 2],
}

impl GpuVertex {
    #[inline]
    pub const fn new(pos: [f32; 2], color: [f32; 4], flags: [f32; 2]) -> Self {
        Self { pos, color, flags }
    }
}

/// Indexed triangle list produced by CPU Lyon tessellation or LOD proxy generation.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TessellatedMesh {
    pub vertices: Vec<GpuVertex>,
    pub indices: Vec<u32>,
}

impl TessellatedMesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(vertex_cap: usize, index_cap: usize) -> Self {
        Self {
            vertices: Vec::with_capacity(vertex_cap),
            indices: Vec::with_capacity(index_cap),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty() || self.vertices.is_empty()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
    }

    /// Append another `TessellatedMesh`, offsetting its indices by the current vertex count.
    pub fn append(&mut self, other: &TessellatedMesh) {
        if other.is_empty() {
            return;
        }
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.reserve(other.indices.len());
        for &idx in &other.indices {
            self.indices.push(base + idx);
        }
    }

    /// Emit a fast 2-triangle axis-aligned quad `(4 vertices, 6 indices)` in page inches.
    #[inline]
    pub fn push_quad(&mut self, min: [f32; 2], max: [f32; 2], color: [f32; 4], flags: [f32; 2]) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            GpuVertex::new([min[0], min[1]], color, flags),
            GpuVertex::new([max[0], min[1]], color, flags),
            GpuVertex::new([max[0], max[1]], color, flags),
            GpuVertex::new([min[0], max[1]], color, flags),
        ]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// Emit a 2-triangle quad given arbitrary 4 corners `[p0, p1, p2, p3]` in page inches.
    #[inline]
    pub fn push_quad_points(
        &mut self,
        pts: [[f32; 2]; 4],
        color: [f32; 4],
        flags: [f32; 2],
    ) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            GpuVertex::new(pts[0], color, flags),
            GpuVertex::new(pts[1], color, flags),
            GpuVertex::new(pts[2], color, flags),
            GpuVertex::new(pts[3], color, flags),
        ]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
}

#[derive(Clone, Debug)]
struct CachedMeshEntry {
    content_hash: u64,
    mesh: TessellatedMesh,
}

/// Content-addressed mesh cache keyed by `(shape_id, content_hash)` so panning and zooming
/// reuse cached vertex/index slabs with zero CPU re-tessellation.
#[derive(Clone, Debug, Default)]
pub struct MeshCache {
    entries: HashMap<u64, CachedMeshEntry>,
    hits: usize,
    misses: usize,
}

impl MeshCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(cap),
            hits: 0,
            misses: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[inline]
    pub fn hits(&self) -> usize {
        self.hits
    }

    #[inline]
    pub fn misses(&self) -> usize {
        self.misses
    }

    pub fn reset_counters(&mut self) {
        self.hits = 0;
        self.misses = 0;
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.reset_counters();
    }

    pub fn invalidate(&mut self, shape_id: u64) -> bool {
        self.entries.remove(&shape_id).is_some()
    }

    /// Look up a cached mesh if `(shape_id, content_hash)` matches.
    pub fn get(&self, shape_id: u64, content_hash: u64) -> Option<&TessellatedMesh> {
        self.entries
            .get(&shape_id)
            .filter(|e| e.content_hash == content_hash)
            .map(|e| &e.mesh)
    }

    /// Retrieve cached `TessellatedMesh` for `visual` or tessellate & store on cache miss.
    pub fn get_or_tessellate(
        &mut self,
        visual: &EvaluatedShapeVisual,
        tolerance_in: f32,
    ) -> &TessellatedMesh {
        let hash = compute_visual_content_hash(visual);
        let hit = self
            .entries
            .get(&visual.shape_id)
            .is_some_and(|e| e.content_hash == hash);
        if hit {
            self.hits += 1;
        } else {
            self.misses += 1;
            let mesh = tessellate_shape_full(visual, tolerance_in);
            self.entries.insert(
                visual.shape_id,
                CachedMeshEntry {
                    content_hash: hash,
                    mesh,
                },
            );
        }
        &self.entries[&visual.shape_id].mesh
    }

    /// Append `visual` into `batch_out`, automatically switching to a fast 2-triangle LOD
    /// bounding quad whenever `pixels_per_inch * shape_size < 3.0` so zoomed-out scenes
    /// with 50,000+ shapes stay well below `< 8.33 ms` (120 FPS).
    /// Returns `true` if the fast LOD proxy path was taken.
    pub fn append_shape_with_lod(
        &mut self,
        visual: &EvaluatedShapeVisual,
        pixels_per_inch: f64,
        batch_out: &mut TessellatedMesh,
    ) -> bool {
        let ppi = pixels_per_inch.max(0.001);
        let w = (visual.aabb_max[0] - visual.aabb_min[0]).abs();
        let h = (visual.aabb_max[1] - visual.aabb_min[1]).abs();
        let shape_size = w.max(h);

        if ppi * shape_size < 3.0 {
            emit_lod_bounding_quad(visual, ppi, batch_out);
            return true;
        }

        // Tolerance scales gently with typical high-zoom precision while remaining zoom-invariant
        // in the cache so normal panning/zooming never invalidates cached shape geometry.
        let cached = self.get_or_tessellate(visual, 0.0035);
        batch_out.append(cached);
        false
    }
}

/// Emit a fast 2-triangle bounding quad for sub-3px zoomed-out shapes (`pixels_per_inch * shape_size < 3.0`).
#[inline]
pub fn emit_lod_bounding_quad(
    visual: &EvaluatedShapeVisual,
    pixels_per_inch: f64,
    out: &mut TessellatedMesh,
) {
    let color_rgba = visual
        .fill_color
        .or(visual.stroke_color)
        .unwrap_or(RgbaColor {
            r: 71,
            g: 85,
            b: 105,
            a: 255,
        })
        .to_linear_f32();
    let min_px_in = (1.0 / pixels_per_inch.max(1e-4)) as f32;
    let mut x0 = visual.aabb_min[0] as f32;
    let mut y0 = visual.aabb_min[1] as f32;
    let mut x1 = visual.aabb_max[0] as f32;
    let mut y1 = visual.aabb_max[1] as f32;
    if !x0.is_finite() || !x1.is_finite() || !y0.is_finite() || !y1.is_finite() {
        return;
    }
    if x1 - x0 < min_px_in {
        let cx = 0.5 * (x0 + x1);
        x0 = cx - 0.5 * min_px_in;
        x1 = cx + 0.5 * min_px_in;
    }
    if y1 - y0 < min_px_in {
        let cy = 0.5 * (y0 + y1);
        y0 = cy - 0.5 * min_px_in;
        y1 = cy + 0.5 * min_px_in;
    }
    out.push_quad([x0, y0], [x1, y1], color_rgba, [PRIM_FLAG_SHAPE, 0.0]);
}

/// Tessellate a single `EvaluatedShapeVisual` respecting LOD (`pixels_per_inch * shape_size < 3.0`).
pub fn tessellate_shape(visual: &EvaluatedShapeVisual, pixels_per_inch: f64) -> TessellatedMesh {
    let ppi = pixels_per_inch.max(0.001);
    let w = (visual.aabb_max[0] - visual.aabb_min[0]).abs();
    let h = (visual.aabb_max[1] - visual.aabb_min[1]).abs();
    if ppi * w.max(h) < 3.0 {
        let mut out = TessellatedMesh::with_capacity(4, 6);
        emit_lod_bounding_quad(visual, ppi, &mut out);
        return out;
    }
    let tol = (0.35 / ppi).clamp(0.0005, 0.02) as f32;
    tessellate_shape_full(visual, tol)
}

/// Full-precision Lyon tessellation of an `EvaluatedShapeVisual`, including:
/// - `MoveTo`, `LineTo`, `QuadTo`, `CubicTo`, `ArcTo` circular arcs, `Ellipse`, `Close`
/// - Corner fillet rounding (`rounding_in`)
/// - `begin_arrow` and `end_arrow` arrowheads
/// - Vector stroke glyph rendering for non-empty `visual.text`
pub fn tessellate_shape_full(
    visual: &EvaluatedShapeVisual,
    tolerance_in: f32,
) -> TessellatedMesh {
    let tol = tolerance_in.clamp(0.0002, 0.05);
    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    let stroke_w = (visual.stroke_weight_in.max(1.0 / 144.0)) as f32;
    let fill_rgba = visual.fill_color.map(RgbaColor::to_linear_f32);
    let stroke_rgba = visual.stroke_color.map(RgbaColor::to_linear_f32);

    let mut first_tangent: Option<([f64; 2], [f64; 2])> = None;
    let mut last_tangent: Option<([f64; 2], [f64; 2])> = None;

    if visual.paths.is_empty() && visual.kind != ShapeKind::Guide {
        // Fallback synthesized rectangle from ShapeTransform2D when no explicit Geometry section exists.
        let tf = &visual.transform;
        let c0: [f64; 2] = tf.local_to_parent(0.0, 0.0).into();
        let c1: [f64; 2] = tf.local_to_parent(tf.width, 0.0).into();
        let c2: [f64; 2] = tf.local_to_parent(tf.width, tf.height).into();
        let c3: [f64; 2] = tf.local_to_parent(0.0, tf.height).into();
        let synth = EvaluatedGeometryPath {
            no_fill: visual.kind == ShapeKind::Connector1D,
            no_line: false,
            no_show: false,
            fill_color: None,
            stroke_color: None,
            stroke_weight_in: None,
            verbs_page_space: vec![
                EvaluatedPathVerb::MoveTo(c0),
                EvaluatedPathVerb::LineTo(c1),
                EvaluatedPathVerb::LineTo(c2),
                EvaluatedPathVerb::LineTo(c3),
                EvaluatedPathVerb::Close,
            ],
        };
        tessellate_geometry_section(
            &synth,
            visual.rounding_in,
            fill_rgba,
            stroke_rgba,
            stroke_w,
            tol,
            &mut fill_tess,
            &mut stroke_tess,
            &mut lyon_buf,
            &mut first_tangent,
            &mut last_tangent,
        );
    } else {
        for geom in &visual.paths {
            if geom.no_show || geom.verbs_page_space.is_empty() {
                continue;
            }
            let g_fill = geom.fill_color.map(RgbaColor::to_linear_f32).or(fill_rgba);
            let g_stroke = geom.stroke_color.map(RgbaColor::to_linear_f32).or(stroke_rgba);
            let g_sw = geom
                .stroke_weight_in
                .map(|w| w.max(1.0 / 144.0) as f32)
                .unwrap_or(stroke_w);
            tessellate_geometry_section(
                geom,
                visual.rounding_in,
                g_fill,
                g_stroke,
                g_sw,
                tol,
                &mut fill_tess,
                &mut stroke_tess,
                &mut lyon_buf,
                &mut first_tangent,
                &mut last_tangent,
            );
        }
    }

    // Arrowheads at Begin and End of paths when configured (`begin_arrow > 0` or `end_arrow > 0`).
    if let Some(sc) = stroke_rgba {
        if visual.begin_arrow > 0 {
            if let Some((tip, next_pt)) = first_tangent {
                // Arrow points backward toward `tip` from `next_pt`.
                tessellate_arrowhead(
                    visual.begin_arrow,
                    next_pt,
                    tip,
                    stroke_w,
                    sc,
                    tol,
                    &mut fill_tess,
                    &mut stroke_tess,
                    &mut lyon_buf,
                );
            }
        }
        if visual.end_arrow > 0 {
            if let Some((prev_pt, tip)) = last_tangent {
                tessellate_arrowhead(
                    visual.end_arrow,
                    prev_pt,
                    tip,
                    stroke_w,
                    sc,
                    tol,
                    &mut fill_tess,
                    &mut stroke_tess,
                    &mut lyon_buf,
                );
            }
        }
    }

    // Crisp vector stroke text for shape labels (routing multi-run rich text, multi-line,
    // bullets, RTL Bidi, and vertical CJK blocks through `crate::typography`).
    let mut out_mesh = TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    };
    let trimmed_txt = visual.text.trim();
    if !trimmed_txt.is_empty() {
        if let Some(bg) = visual.text_bg_color {
            let bg_rgba = bg.to_linear_f32();
            let hw = (visual.transform.width.max(0.4) * 0.46) as f32;
            let hh = ((visual.font_size_pt / 72.0) * 0.85 * (trimmed_txt.lines().count().max(1) as f64)).min(visual.transform.height.max(0.35) * 0.48) as f32;
            let cx = visual.txt_pos[0] as f32;
            let cy = visual.txt_pos[1] as f32;
            out_mesh.push_quad([cx - hw, cy - hh], [cx + hw, cy + hh], bg_rgba, [PRIM_FLAG_SHAPE, 0.0]);
        }
        let needs_rich = !trimmed_txt.is_ascii()
            || trimmed_txt.contains('\n')
            || trimmed_txt.contains('*')
            || trimmed_txt.contains("__")
            || trimmed_txt.contains("~~")
            || trimmed_txt.contains('[')
            || trimmed_txt.starts_with("- ")
            || visual.char_style != 0
            || visual.horiz_align != 1
            || visual.vert_align != 1
            || visual.bullet_style != 0;
        if needs_rich {
            let block = crate::typography::parse_inline_rich_text_styled(
                trimmed_txt,
                visual.font_size_pt,
                visual.text_color,
                visual.char_style,
                visual.horiz_align,
                visual.vert_align,
                visual.bullet_style,
            );
            let rich_mesh = crate::typography::tessellate_rich_text_in_box(
                &block,
                visual.txt_pos[0],
                visual.txt_pos[1],
                visual.transform.width.max(0.8),
                visual.transform.height.max(0.4),
                visual.transform.angle_rad,
            );
            out_mesh.append(&rich_mesh);
        } else {
            let mut txt_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
            tessellate_vector_label(visual, tol, &mut stroke_tess, &mut txt_buf);
            out_mesh.append(&TessellatedMesh {
                vertices: txt_buf.vertices,
                indices: txt_buf.indices,
            });
        }
    }

    out_mesh
}

#[allow(clippy::too_many_arguments)]
fn tessellate_geometry_section(
    geom: &EvaluatedGeometryPath,
    rounding_in: f64,
    fill_rgba: Option<[f32; 4]>,
    stroke_rgba: Option<[f32; 4]>,
    stroke_w: f32,
    tol: f32,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
    first_tangent: &mut Option<([f64; 2], [f64; 2])>,
    last_tangent: &mut Option<([f64; 2], [f64; 2])>,
) {
    let path = build_lyon_path_from_verbs(
        &geom.verbs_page_space,
        rounding_in,
        first_tangent,
        last_tangent,
    );

    if !geom.no_fill {
        if let Some(fc) = fill_rgba {
            let _ = fill_tess.tessellate_path(
                &path,
                &FillOptions::tolerance(tol).with_fill_rule(FillRule::EvenOdd),
                &mut BuffersBuilder::new(lyon_buf, |vertex: FillVertex| {
                    let p = vertex.position();
                    GpuVertex::new([p.x, p.y], fc, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    }

    if !geom.no_line {
        if let Some(sc) = stroke_rgba {
            let _ = stroke_tess.tessellate_path(
                &path,
                &StrokeOptions::tolerance(tol)
                    .with_line_width(stroke_w)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round),
                &mut BuffersBuilder::new(lyon_buf, |vertex: StrokeVertex| {
                    let p = vertex.position();
                    GpuVertex::new([p.x, p.y], sc, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    }
}

#[derive(Clone, Debug)]
enum SubpathSeg {
    Line([f64; 2]),
    Quad { ctrl: [f64; 2], to: [f64; 2] },
    Cubic {
        ctrl1: [f64; 2],
        ctrl2: [f64; 2],
        to: [f64; 2],
    },
    Arc { to: [f64; 2], bow: f64 },
}

impl SubpathSeg {
    fn end_pt(&self) -> [f64; 2] {
        match self {
            Self::Line(p) => *p,
            Self::Quad { to, .. } => *to,
            Self::Cubic { to, .. } => *to,
            Self::Arc { to, .. } => *to,
        }
    }
}

fn build_lyon_path_from_verbs(
    verbs: &[EvaluatedPathVerb],
    rounding_in: f64,
    first_tangent: &mut Option<([f64; 2], [f64; 2])>,
    last_tangent: &mut Option<([f64; 2], [f64; 2])>,
) -> Path {
    let mut builder = Path::builder();
    let mut cur_start: Option<[f64; 2]> = None;
    let mut cur_segs: Vec<SubpathSeg> = Vec::new();

    let flush_subpath = |builder: &mut lyon_path::path::Builder,
                         start_opt: &mut Option<[f64; 2]>,
                         segs: &mut Vec<SubpathSeg>,
                         explicit_close: bool,
                         first_tan: &mut Option<([f64; 2], [f64; 2])>,
                         last_tan: &mut Option<([f64; 2], [f64; 2])>| {
        let Some(start) = start_opt.take() else {
            segs.clear();
            return;
        };
        if segs.is_empty() {
            return;
        }
        emit_subpath_into_builder(
            builder,
            start,
            segs,
            explicit_close,
            rounding_in,
            first_tan,
            last_tan,
        );
        segs.clear();
    };

    for verb in verbs {
        match *verb {
            EvaluatedPathVerb::MoveTo(pt) => {
                if !is_finite_pt(pt) {
                    continue;
                }
                flush_subpath(
                    &mut builder,
                    &mut cur_start,
                    &mut cur_segs,
                    false,
                    first_tangent,
                    last_tangent,
                );
                cur_start = Some(pt);
            }
            EvaluatedPathVerb::LineTo(pt) => {
                if !is_finite_pt(pt) {
                    continue;
                }
                if cur_start.is_none() {
                    cur_start = Some(pt);
                } else {
                    cur_segs.push(SubpathSeg::Line(pt));
                }
            }
            EvaluatedPathVerb::QuadTo { ctrl, to } => {
                if !is_finite_pt(ctrl) || !is_finite_pt(to) {
                    continue;
                }
                if cur_start.is_none() {
                    cur_start = Some(to);
                } else {
                    cur_segs.push(SubpathSeg::Quad { ctrl, to });
                }
            }
            EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                if !is_finite_pt(ctrl1) || !is_finite_pt(ctrl2) || !is_finite_pt(to) {
                    continue;
                }
                if cur_start.is_none() {
                    cur_start = Some(to);
                } else {
                    cur_segs.push(SubpathSeg::Cubic { ctrl1, ctrl2, to });
                }
            }
            EvaluatedPathVerb::ArcTo { to, bow } => {
                if !is_finite_pt(to) || !bow.is_finite() {
                    continue;
                }
                if cur_start.is_none() {
                    cur_start = Some(to);
                } else {
                    cur_segs.push(SubpathSeg::Arc { to, bow });
                }
            }
            EvaluatedPathVerb::Ellipse {
                center,
                rx,
                ry,
                angle_rad,
            } => {
                flush_subpath(
                    &mut builder,
                    &mut cur_start,
                    &mut cur_segs,
                    false,
                    first_tangent,
                    last_tangent,
                );
                if is_finite_pt(center) && rx.is_finite() && ry.is_finite() && angle_rad.is_finite()
                {
                    emit_rotated_ellipse(&mut builder, center, rx.abs(), ry.abs(), angle_rad);
                }
            }
            EvaluatedPathVerb::Close => {
                flush_subpath(
                    &mut builder,
                    &mut cur_start,
                    &mut cur_segs,
                    true,
                    first_tangent,
                    last_tangent,
                );
            }
        }
    }

    flush_subpath(
        &mut builder,
        &mut cur_start,
        &mut cur_segs,
        false,
        first_tangent,
        last_tangent,
    );

    builder.build()
}

fn emit_subpath_into_builder(
    builder: &mut lyon_path::path::Builder,
    start: [f64; 2],
    segs: &[SubpathSeg],
    explicit_close: bool,
    rounding_in: f64,
    first_tangent: &mut Option<([f64; 2], [f64; 2])>,
    last_tangent: &mut Option<([f64; 2], [f64; 2])>,
) {
    let last_end = segs.last().map(SubpathSeg::end_pt).unwrap_or(start);
    let closes_at_start = explicit_close || dist_2d(start, last_end) < 1e-5;

    // Record endpoint tangents for BeginArrow / EndArrow placement.
    if first_tangent.is_none() {
        if let Some(first_seg) = segs.first() {
            let dir_pt = match *first_seg {
                SubpathSeg::Line(p) => p,
                SubpathSeg::Quad { ctrl, to } => {
                    if dist_2d(start, ctrl) > 1e-6 {
                        ctrl
                    } else {
                        to
                    }
                }
                SubpathSeg::Cubic { ctrl1, ctrl2, to } => {
                    if dist_2d(start, ctrl1) > 1e-6 {
                        ctrl1
                    } else if dist_2d(start, ctrl2) > 1e-6 {
                        ctrl2
                    } else {
                        to
                    }
                }
                SubpathSeg::Arc { to, bow } => arc_initial_tangent_point(start, to, bow),
            };
            if dist_2d(start, dir_pt) > 1e-6 {
                *first_tangent = Some((start, dir_pt));
            }
        }
    }

    if let Some(last_seg) = segs.last() {
        let prev_pt = if segs.len() >= 2 {
            segs[segs.len() - 2].end_pt()
        } else {
            start
        };
        let end_pt = last_seg.end_pt();
        let incoming_pt = match *last_seg {
            SubpathSeg::Line(_) => prev_pt,
            SubpathSeg::Quad { ctrl, .. } => {
                if dist_2d(ctrl, end_pt) > 1e-6 {
                    ctrl
                } else {
                    prev_pt
                }
            }
            SubpathSeg::Cubic { ctrl1, ctrl2, .. } => {
                if dist_2d(ctrl2, end_pt) > 1e-6 {
                    ctrl2
                } else if dist_2d(ctrl1, end_pt) > 1e-6 {
                    ctrl1
                } else {
                    prev_pt
                }
            }
            SubpathSeg::Arc { to, bow } => arc_terminal_tangent_point(prev_pt, to, bow),
        };
        if dist_2d(incoming_pt, end_pt) > 1e-6 {
            *last_tangent = Some((incoming_pt, end_pt));
        }
    }

    // Pure polyline corner rounding (`Rounding > 0 in` in Visio Line Format section).
    let all_lines = segs.iter().all(|s| matches!(s, SubpathSeg::Line(_)));
    if rounding_in > 1e-5 && all_lines && segs.len() >= 2 {
        let mut pts = Vec::with_capacity(segs.len() + 1);
        pts.push(start);
        for s in segs {
            if let SubpathSeg::Line(p) = *s {
                if dist_2d(*pts.last().unwrap(), p) > 1e-6 {
                    pts.push(p);
                }
            }
        }
        if closes_at_start && pts.len() >= 2 && dist_2d(*pts.first().unwrap(), *pts.last().unwrap()) < 1e-5 {
            pts.pop();
        }
        if pts.len() >= 3 || ( !closes_at_start && pts.len() >= 3 ) {
            emit_rounded_polyline(builder, &pts, closes_at_start, rounding_in);
            return;
        }
    }

    builder.begin(point(start[0] as f32, start[1] as f32));
    let mut cur = start;
    for (idx, seg) in segs.iter().enumerate() {
        let is_closing_dup =
            closes_at_start && explicit_close && idx + 1 == segs.len() && dist_2d(seg.end_pt(), start) < 1e-5;
        if is_closing_dup {
            break;
        }
        match *seg {
            SubpathSeg::Line(to) => {
                if dist_2d(cur, to) > 1e-7 {
                    builder.line_to(point(to[0] as f32, to[1] as f32));
                    cur = to;
                }
            }
            SubpathSeg::Quad { ctrl, to } => {
                builder.quadratic_bezier_to(
                    point(ctrl[0] as f32, ctrl[1] as f32),
                    point(to[0] as f32, to[1] as f32),
                );
                cur = to;
            }
            SubpathSeg::Cubic { ctrl1, ctrl2, to } => {
                builder.cubic_bezier_to(
                    point(ctrl1[0] as f32, ctrl1[1] as f32),
                    point(ctrl2[0] as f32, ctrl2[1] as f32),
                    point(to[0] as f32, to[1] as f32),
                );
                cur = to;
            }
            SubpathSeg::Arc { to, bow } => {
                emit_circular_arc_sagitta(builder, cur, to, bow);
                cur = to;
            }
        }
    }
    builder.end(closes_at_start);
}

/// Emit a Visio `ArcTo(to, bow)` circular arc (where `bow` is perpendicular sagitta from chord midpoint)
/// decomposed into exact cubic Bezier arc segments (`<= 45 deg` each).
fn emit_circular_arc_sagitta(
    builder: &mut lyon_path::path::Builder,
    p0: [f64; 2],
    p1: [f64; 2],
    bow: f64,
) {
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-7 {
        return;
    }
    if bow.abs() < 1e-7 {
        builder.line_to(point(p1[0] as f32, p1[1] as f32));
        return;
    }

    let ux = dx / chord;
    let uy = dy / chord;
    let nx = -uy;
    let ny = ux;

    let mx = (p0[0] + p1[0]) * 0.5;
    let my = (p0[1] + p1[1]) * 0.5;
    let arc_mx = mx + bow * nx;
    let arc_my = my + bow * ny;

    let half_c = chord * 0.5;
    let radius = (half_c * half_c + bow * bow) / (2.0 * bow.abs());
    let center_offset = bow - radius * bow.signum();
    let cx = mx + center_offset * nx;
    let cy = my + center_offset * ny;

    let a0 = (p0[1] - cy).atan2(p0[0] - cx);
    let amid = (arc_my - cy).atan2(arc_mx - cx);
    let a1 = (p1[1] - cy).atan2(p1[0] - cx);

    let ccw_end = (a1 - a0).rem_euclid(TAU);
    let ccw_mid = (amid - a0).rem_euclid(TAU);
    let sweep = if ccw_mid <= ccw_end + 1e-9 {
        ccw_end
    } else {
        ccw_end - TAU
    };

    let n_segs = ((sweep.abs() / FRAC_PI_4).ceil() as usize).clamp(1, 32);
    let dt = sweep / (n_segs as f64);
    let k = (4.0 / 3.0) * (dt * 0.25).tan();

    for i in 0..n_segs {
        let t0 = a0 + (i as f64) * dt;
        let t1 = if i + 1 == n_segs { a0 + sweep } else { t0 + dt };
        let (s0, c0) = t0.sin_cos();
        let (s1, c1) = t1.sin_cos();
        let sx = cx + radius * c0;
        let sy = cy + radius * s0;
        let ex = if i + 1 == n_segs {
            p1[0]
        } else {
            cx + radius * c1
        };
        let ey = if i + 1 == n_segs {
            p1[1]
        } else {
            cy + radius * s1
        };
        let ctrl1 = [sx - k * radius * s0, sy + k * radius * c0];
        let ctrl2 = [ex + k * radius * s1, ey - k * radius * c1];
        builder.cubic_bezier_to(
            point(ctrl1[0] as f32, ctrl1[1] as f32),
            point(ctrl2[0] as f32, ctrl2[1] as f32),
            point(ex as f32, ey as f32),
        );
    }
}

fn arc_initial_tangent_point(p0: [f64; 2], p1: [f64; 2], bow: f64) -> [f64; 2] {
    if bow.abs() < 1e-7 {
        return p1;
    }
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-7 {
        return p1;
    }
    let ux = dx / chord;
    let uy = dy / chord;
    let nx = -uy;
    let ny = ux;
    [p0[0] + ux * 0.1 + nx * bow * 0.4, p0[1] + uy * 0.1 + ny * bow * 0.4]
}

fn arc_terminal_tangent_point(p0: [f64; 2], p1: [f64; 2], bow: f64) -> [f64; 2] {
    if bow.abs() < 1e-7 {
        return p0;
    }
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let chord = (dx * dx + dy * dy).sqrt();
    if chord < 1e-7 {
        return p0;
    }
    let ux = dx / chord;
    let uy = dy / chord;
    let nx = -uy;
    let ny = ux;
    [p1[0] - ux * 0.1 + nx * bow * 0.4, p1[1] - uy * 0.1 + ny * bow * 0.4]
}

/// Emit a closed rotated ellipse using 4 exact kappa-weighted cubic Bezier quadrants.
fn emit_rotated_ellipse(
    builder: &mut lyon_path::path::Builder,
    center: [f64; 2],
    rx: f64,
    ry: f64,
    angle_rad: f64,
) {
    if rx < 1e-7 || ry < 1e-7 {
        return;
    }
    const KAPPA: f64 = 0.552_284_749_830_793_6;
    let (sa, ca) = angle_rad.sin_cos();
    let map_pt = |lx: f64, ly: f64| -> lyon_path::math::Point {
        let wx = center[0] + lx * ca - ly * sa;
        let wy = center[1] + lx * sa + ly * ca;
        point(wx as f32, wy as f32)
    };
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;

    builder.begin(map_pt(rx, 0.0));
    builder.cubic_bezier_to(map_pt(rx, ky), map_pt(kx, ry), map_pt(0.0, ry));
    builder.cubic_bezier_to(map_pt(-kx, ry), map_pt(-rx, ky), map_pt(-rx, 0.0));
    builder.cubic_bezier_to(map_pt(-rx, -ky), map_pt(-kx, -ry), map_pt(0.0, -ry));
    builder.cubic_bezier_to(map_pt(kx, -ry), map_pt(rx, -ky), map_pt(rx, 0.0));
    builder.end(true);
}

/// Fillet polyline/polygon corners according to Visio's `Rounding` cell.
fn emit_rounded_polyline(
    builder: &mut lyon_path::path::Builder,
    pts: &[[f64; 2]],
    closed: bool,
    rounding_in: f64,
) {
    let n = pts.len();
    if n < 2 {
        return;
    }
    let mut started = false;

    if closed {
        for i in 0..n {
            let prev = pts[(i + n - 1) % n];
            let curr = pts[i];
            let next = pts[(i + 1) % n];
            let (t_in, t_out, used_fillet) = compute_corner_fillet(prev, curr, next, rounding_in);
            if !started {
                builder.begin(point(t_in[0] as f32, t_in[1] as f32));
                started = true;
            } else {
                builder.line_to(point(t_in[0] as f32, t_in[1] as f32));
            }
            if used_fillet {
                // Smooth quadratic/circular fillet around corner `curr`.
                let ctrl = [
                    curr[0] * 0.55 + (t_in[0] + t_out[0]) * 0.225,
                    curr[1] * 0.55 + (t_in[1] + t_out[1]) * 0.225,
                ];
                builder.quadratic_bezier_to(
                    point(ctrl[0] as f32, ctrl[1] as f32),
                    point(t_out[0] as f32, t_out[1] as f32),
                );
            }
        }
        if started {
            builder.end(true);
        }
    } else {
        builder.begin(point(pts[0][0] as f32, pts[0][1] as f32));
        for i in 1..(n - 1) {
            let (t_in, t_out, used_fillet) =
                compute_corner_fillet(pts[i - 1], pts[i], pts[i + 1], rounding_in);
            builder.line_to(point(t_in[0] as f32, t_in[1] as f32));
            if used_fillet {
                let ctrl = [
                    pts[i][0] * 0.55 + (t_in[0] + t_out[0]) * 0.225,
                    pts[i][1] * 0.55 + (t_in[1] + t_out[1]) * 0.225,
                ];
                builder.quadratic_bezier_to(
                    point(ctrl[0] as f32, ctrl[1] as f32),
                    point(t_out[0] as f32, t_out[1] as f32),
                );
            }
        }
        builder.line_to(point(pts[n - 1][0] as f32, pts[n - 1][1] as f32));
        builder.end(false);
    }
}

fn compute_corner_fillet(
    prev: [f64; 2],
    curr: [f64; 2],
    next: [f64; 2],
    radius: f64,
) -> ([f64; 2], [f64; 2], bool) {
    let v_in = [prev[0] - curr[0], prev[1] - curr[1]];
    let v_out = [next[0] - curr[0], next[1] - curr[1]];
    let len_in = (v_in[0] * v_in[0] + v_in[1] * v_in[1]).sqrt();
    let len_out = (v_out[0] * v_out[0] + v_out[1] * v_out[1]).sqrt();
    if len_in < 1e-6 || len_out < 1e-6 {
        return (curr, curr, false);
    }
    let u_in = [v_in[0] / len_in, v_in[1] / len_in];
    let u_out = [v_out[0] / len_out, v_out[1] / len_out];
    let cos_theta = (u_in[0] * u_out[0] + u_in[1] * u_out[1]).clamp(-1.0, 1.0);
    let theta = cos_theta.acos();
    if !(1e-3..(PI - 1e-3)).contains(&theta) {
        return (curr, curr, false);
    }
    let half_tan = (theta * 0.5).tan();
    if half_tan.abs() < 1e-6 {
        return (curr, curr, false);
    }
    let trim = (radius / half_tan).min(len_in * 0.48).min(len_out * 0.48);
    if trim < 1e-6 {
        return (curr, curr, false);
    }
    let t_in = [curr[0] + u_in[0] * trim, curr[1] + u_in[1] * trim];
    let t_out = [curr[0] + u_out[0] * trim, curr[1] + u_out[1] * trim];
    (t_in, t_out, true)
}

#[allow(clippy::too_many_arguments)]
fn tessellate_arrowhead(
    arrow_code: u8,
    from_pt: [f64; 2],
    tip_pt: [f64; 2],
    stroke_w: f32,
    color: [f32; 4],
    tol: f32,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
) {
    let dx = tip_pt[0] - from_pt[0];
    let dy = tip_pt[1] - from_pt[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return;
    }
    let ux = dx / len;
    let uy = dy / len;
    let nx = -uy;
    let ny = ux;

    let arrow_len = ((stroke_w as f64) * 6.5).clamp(0.09, 0.32);
    let arrow_half_w = arrow_len * 0.52;

    match arrow_code {
        1 => {
            // Visio Code 1: Open V-chevron arrowhead.
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let left = [bx + nx * arrow_half_w, by + ny * arrow_half_w];
            let right = [bx - nx * arrow_half_w, by - ny * arrow_half_w];
            let mut b = Path::builder();
            b.begin(point(left[0] as f32, left[1] as f32));
            b.line_to(point(tip_pt[0] as f32, tip_pt[1] as f32));
            b.line_to(point(right[0] as f32, right[1] as f32));
            b.end(false);
            let p = b.build();
            let _ = stroke_tess.tessellate_path(
                &p,
                &StrokeOptions::tolerance(tol)
                    .with_line_width(stroke_w)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Miter),
                &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
                    let pt = v.position();
                    GpuVertex::new([pt.x, pt.y], color, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
        10 | 11 | 20 => {
            // Circular dot marker at endpoint.
            let r = arrow_len * 0.42;
            let mut b = Path::builder();
            emit_rotated_ellipse(&mut b, tip_pt, r, r, 0.0);
            let p = b.build();
            let _ = fill_tess.tessellate_path(
                &p,
                &FillOptions::tolerance(tol),
                &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                    let pt = v.position();
                    GpuVertex::new([pt.x, pt.y], color, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
        5 | 12 => {
            // Diamond arrowhead (UML composition / aggregation).
            let mid = [tip_pt[0] - ux * (arrow_len * 0.6), tip_pt[1] - uy * (arrow_len * 0.6)];
            let tail = [tip_pt[0] - ux * (arrow_len * 1.2), tip_pt[1] - uy * (arrow_len * 1.2)];
            let left = [mid[0] + nx * arrow_half_w, mid[1] + ny * arrow_half_w];
            let right = [mid[0] - nx * arrow_half_w, mid[1] - ny * arrow_half_w];
            let mut b = Path::builder();
            b.begin(point(tip_pt[0] as f32, tip_pt[1] as f32));
            b.line_to(point(left[0] as f32, left[1] as f32));
            b.line_to(point(tail[0] as f32, tail[1] as f32));
            b.line_to(point(right[0] as f32, right[1] as f32));
            b.end(true);
            let p = b.build();
            let _ = fill_tess.tessellate_path(
                &p,
                &FillOptions::tolerance(tol),
                &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                    let pt = v.position();
                    GpuVertex::new([pt.x, pt.y], color, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
        _ => {
            // Classic filled Visio stealth/triangle arrowhead (Codes 2, 3, 4, 13, etc.).
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let notch = if arrow_code == 4 || arrow_code == 13 {
                [
                    tip_pt[0] - ux * (arrow_len * 0.78),
                    tip_pt[1] - uy * (arrow_len * 0.78),
                ]
            } else {
                [bx, by]
            };
            let left = [bx + nx * arrow_half_w, by + ny * arrow_half_w];
            let right = [bx - nx * arrow_half_w, by - ny * arrow_half_w];
            let mut b = Path::builder();
            b.begin(point(tip_pt[0] as f32, tip_pt[1] as f32));
            b.line_to(point(left[0] as f32, left[1] as f32));
            b.line_to(point(notch[0] as f32, notch[1] as f32));
            b.line_to(point(right[0] as f32, right[1] as f32));
            b.end(true);
            let p = b.build();
            let _ = fill_tess.tessellate_path(
                &p,
                &FillOptions::tolerance(tol),
                &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                    let pt = v.position();
                    GpuVertex::new([pt.x, pt.y], color, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    }
}

/// Tessellate variable-pressure `MarkupInkStroke` (pen or translucent highlighter) using Lyon.
pub fn tessellate_ink_stroke(stroke: &MarkupInkStroke, tolerance_in: f32) -> TessellatedMesh {
    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    if stroke.points.is_empty() {
        return TessellatedMesh::default();
    }

    let tol = tolerance_in.clamp(0.0003, 0.02);
    let mut fill_tess = FillTessellator::new();

    let mut color = stroke.color.to_linear_f32();
    if stroke.highlighter {
        color[3] = (color[3] * 0.42).clamp(0.08, 0.65);
    }

    let base_half_w = ((stroke.width_pt.max(0.5) as f64) / 72.0)
        * 0.5
        * if stroke.highlighter { 2.4 } else { 1.0 };

    // Deduplicate near-coincident sample points while preserving pressure profile.
    let mut samples: Vec<([f64; 2], f64)> = Vec::with_capacity(stroke.points.len());
    for pt in &stroke.points {
        if !pt.x_in.is_finite() || !pt.y_in.is_finite() {
            continue;
        }
        let pressure = if pt.pressure.is_finite() {
            (pt.pressure as f64).clamp(0.15, 2.0)
        } else {
            1.0
        };
        let radius = (base_half_w * pressure).max(0.002);
        let pos = [pt.x_in, pt.y_in];
        if let Some((prev_pos, prev_r)) = samples.last_mut() {
            if dist_2d(*prev_pos, pos) < 1e-5 {
                *prev_r = prev_r.max(radius);
                continue;
            }
        }
        samples.push((pos, radius));
    }

    if samples.is_empty() {
        return TessellatedMesh::default();
    }

    // Single-tap dot: emit circular ink dab.
    if samples.len() == 1 {
        let (center, r) = samples[0];
        let mut b = Path::builder();
        emit_rotated_ellipse(&mut b, center, r, r, 0.0);
        let path = b.build();
        let _ = fill_tess.tessellate_path(
            &path,
            &FillOptions::tolerance(tol),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                let p = v.position();
                GpuVertex::new([p.x, p.y], color, [PRIM_FLAG_MARKUP_INK, 0.0])
            }),
        );
        return TessellatedMesh {
            vertices: lyon_buf.vertices,
            indices: lyon_buf.indices,
        };
    }

    // Smooth midpoint subdivision when >= 3 points for fluid stylus curves with interpolated pressure.
    let refined = if samples.len() >= 3 {
        let mut out = Vec::with_capacity(samples.len() * 2);
        out.push(samples[0]);
        for i in 0..(samples.len() - 1) {
            let (p0, r0) = samples[i];
            let (p1, r1) = samples[i + 1];
            let mid = [(p0[0] + p1[0]) * 0.5, (p0[1] + p1[1]) * 0.5];
            let mid_r = (r0 + r1) * 0.5;
            if i > 0 {
                let (prev, pr) = samples[i - 1];
                let q = [
                    0.75 * p0[0] + 0.125 * (prev[0] + p1[0]),
                    0.75 * p0[1] + 0.125 * (prev[1] + p1[1]),
                ];
                let qr = 0.75 * r0 + 0.125 * (pr + r1);
                out.push((q, qr));
            }
            out.push((mid, mid_r));
        }
        out.push(*samples.last().unwrap());
        out
    } else {
        samples
    };

    // Build variable-width left & right offset rails + semicircular caps and fill via Lyon!
    let n = refined.len();
    let mut left_rail = Vec::with_capacity(n);
    let mut right_rail = Vec::with_capacity(n);
    let mut normals = Vec::with_capacity(n);

    for i in 0..n {
        let tang = if i == 0 {
            [refined[1].0[0] - refined[0].0[0], refined[1].0[1] - refined[0].0[1]]
        } else if i + 1 == n {
            [
                refined[n - 1].0[0] - refined[n - 2].0[0],
                refined[n - 1].0[1] - refined[n - 2].0[1],
            ]
        } else {
            [
                refined[i + 1].0[0] - refined[i - 1].0[0],
                refined[i + 1].0[1] - refined[i - 1].0[1],
            ]
        };
        let t_len = (tang[0] * tang[0] + tang[1] * tang[1]).sqrt().max(1e-9);
        let nx = -tang[1] / t_len;
        let ny = tang[0] / t_len;
        let (pos, r) = refined[i];
        normals.push([nx, ny]);
        left_rail.push([pos[0] + nx * r, pos[1] + ny * r]);
        right_rail.push([pos[0] - nx * r, pos[1] - ny * r]);
    }

    let mut b = Path::builder();
    b.begin(point(left_rail[0][0] as f32, left_rail[0][1] as f32));
    for pt in left_rail.iter().skip(1) {
        b.line_to(point(pt[0] as f32, pt[1] as f32));
    }

    // Semicircular end cap around `refined[n - 1]`.
    let (end_c, end_r) = refined[n - 1];
    let end_n = normals[n - 1];
    let end_ang = end_n[1].atan2(end_n[0]);
    for step in 1..=6 {
        let a = end_ang - (PI * (step as f64) / 6.0);
        let (sa, ca) = a.sin_cos();
        b.line_to(point(
            (end_c[0] + end_r * ca) as f32,
            (end_c[1] + end_r * sa) as f32,
        ));
    }

    for pt in right_rail.iter().rev().skip(1) {
        b.line_to(point(pt[0] as f32, pt[1] as f32));
    }

    // Semicircular start cap around `refined[0]`.
    let (start_c, start_r) = refined[0];
    let start_n = normals[0];
    let start_ang = (-start_n[1]).atan2(-start_n[0]);
    for step in 1..6 {
        let a = start_ang - (PI * (step as f64) / 6.0);
        let (sa, ca) = a.sin_cos();
        b.line_to(point(
            (start_c[0] + start_r * ca) as f32,
            (start_c[1] + start_r * sa) as f32,
        ));
    }
    b.end(true);

    let outline = b.build();
    let _ = fill_tess.tessellate_path(
        &outline,
        &FillOptions::tolerance(tol).with_fill_rule(FillRule::NonZero),
        &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], color, [PRIM_FLAG_MARKUP_INK, 0.0])
        }),
    );

    TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    }
}

/// Tessellate Visio interactive selection frame, 8 resize handles, and top rotation handle
/// in page-space inches scaled consistently for the current `zoom_ppi`.
pub fn tessellate_selection_handles(
    visual: &EvaluatedShapeVisual,
    zoom_ppi: f64,
) -> TessellatedMesh {
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = 1.0 / ppi;
    let stroke_w = (1.5 * inv_ppi) as f32;
    let handle_half = (3.5 * inv_ppi) as f32;
    let rot_stem_len = 18.0 * inv_ppi;
    let rot_radius = 4.0 * inv_ppi;

    let accent = [0.145, 0.388, 0.922, 1.0]; // Vexio / Visio selection blue (#2563EB)
    let white = [1.0, 1.0, 1.0, 1.0];
    let tol = (0.35 * inv_ppi).clamp(0.0002, 0.02) as f32;

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    let tf = &visual.transform;
    let c00: [f64; 2] = tf.local_to_parent(0.0, 0.0).into();
    let c10: [f64; 2] = tf.local_to_parent(tf.width, 0.0).into();
    let c11: [f64; 2] = tf.local_to_parent(tf.width, tf.height).into();
    let c01: [f64; 2] = tf.local_to_parent(0.0, tf.height).into();
    let top_mid = [(c01[0] + c11[0]) * 0.5, (c01[1] + c11[1]) * 0.5];
    let bot_mid = [(c00[0] + c10[0]) * 0.5, (c00[1] + c10[1]) * 0.5];
    let left_mid = [(c00[0] + c01[0]) * 0.5, (c00[1] + c01[1]) * 0.5];
    let right_mid = [(c10[0] + c11[0]) * 0.5, (c10[1] + c11[1]) * 0.5];

    // Upward normal in rotated shape frame for the rotation handle stem.
    let up_pt: [f64; 2] = tf.local_to_parent(tf.width * 0.5, tf.height + rot_stem_len).into();
    let mut frame_builder = Path::builder();
    frame_builder.begin(point(c00[0] as f32, c00[1] as f32));
    frame_builder.line_to(point(c10[0] as f32, c10[1] as f32));
    frame_builder.line_to(point(c11[0] as f32, c11[1] as f32));
    frame_builder.line_to(point(c01[0] as f32, c01[1] as f32));
    frame_builder.end(true);
    frame_builder.begin(point(top_mid[0] as f32, top_mid[1] as f32));
    frame_builder.line_to(point(up_pt[0] as f32, up_pt[1] as f32));
    frame_builder.end(false);
    let frame_path = frame_builder.build();

    let _ = stroke_tess.tessellate_path(
        &frame_path,
        &StrokeOptions::tolerance(tol).with_line_width(stroke_w),
        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], accent, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );

    let mut mesh = TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    };

    // 8 square resize handles (4 corners + 4 midpoints) with white fill & blue border.
    let handles = [c00, c10, c11, c01, top_mid, bot_mid, left_mid, right_mid];
    let outer = handle_half + stroke_w;
    for h in handles {
        let hx = h[0] as f32;
        let hy = h[1] as f32;
        mesh.push_quad(
            [hx - outer, hy - outer],
            [hx + outer, hy + outer],
            accent,
            [PRIM_FLAG_OVERLAY_UI, 0.0],
        );
        mesh.push_quad(
            [hx - handle_half, hy - handle_half],
            [hx + handle_half, hy + handle_half],
            white,
            [PRIM_FLAG_OVERLAY_UI, 0.0],
        );
    }

    // Circular rotation handle knob at `up_pt`.
    let mut knob_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut kb = Path::builder();
    emit_rotated_ellipse(&mut kb, up_pt, rot_radius, rot_radius, 0.0);
    let knob_path = kb.build();
    let _ = fill_tess.tessellate_path(
        &knob_path,
        &FillOptions::tolerance(tol),
        &mut BuffersBuilder::new(&mut knob_buf, |v: FillVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], white, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );
    let _ = stroke_tess.tessellate_path(
        &knob_path,
        &StrokeOptions::tolerance(tol).with_line_width(stroke_w),
        &mut BuffersBuilder::new(&mut knob_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], accent, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );
    mesh.append(&TessellatedMesh {
        vertices: knob_buf.vertices,
        indices: knob_buf.indices,
    });

    mesh
}

/// Tessellate major & minor Visio ruler ticks along the page origin/viewport edges
/// plus optional explicit vector grid lines.
pub fn tessellate_ruler_and_grid_ticks(
    min_in: [f64; 2],
    max_in: [f64; 2],
    grid_spacing_in: f64,
    zoom_ppi: f64,
) -> TessellatedMesh {
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = 1.0 / ppi;
    let spacing = grid_spacing_in.max(0.0625);
    let mut step = spacing;
    while step * ppi < 8.0 {
        step *= 2.0;
    }

    let line_half = (0.65 * inv_ppi) as f32;
    let ruler_band = (18.0 * inv_ppi) as f32;
    let major_tick = (9.0 * inv_ppi) as f32;
    let minor_tick = (4.5 * inv_ppi) as f32;

    let band_bg = [0.965, 0.973, 0.985, 0.94];
    let tick_col = [0.392, 0.455, 0.545, 1.0];

    let x0 = min_in[0].min(max_in[0]);
    let x1 = min_in[0].max(max_in[0]);
    let y0 = min_in[1].min(max_in[1]);
    let y1 = min_in[1].max(max_in[1]);

    let mut mesh = TessellatedMesh::new();
    // Top horizontal ruler strip & left vertical ruler strip in viewport page coordinates.
    mesh.push_quad(
        [x0 as f32, y1 as f32 - ruler_band],
        [x1 as f32, y1 as f32],
        band_bg,
        [PRIM_FLAG_OVERLAY_UI, 0.0],
    );
    mesh.push_quad(
        [x0 as f32, y0 as f32],
        [x0 as f32 + ruler_band, y1 as f32],
        band_bg,
        [PRIM_FLAG_OVERLAY_UI, 0.0],
    );

    let start_ix = (x0 / step).floor() as i64;
    let end_ix = (x1 / step).ceil() as i64;
    for ix in start_ix..=end_ix.min(start_ix + 2048) {
        let x = (ix as f64 * step) as f32;
        let is_major = ix.rem_euclid(4) == 0;
        let tick_len = if is_major { major_tick } else { minor_tick };
        mesh.push_quad(
            [x - line_half, y1 as f32 - ruler_band],
            [x + line_half, y1 as f32 - ruler_band + tick_len],
            tick_col,
            [PRIM_FLAG_OVERLAY_UI, 0.0],
        );
    }

    let start_iy = (y0 / step).floor() as i64;
    let end_iy = (y1 / step).ceil() as i64;
    for iy in start_iy..=end_iy.min(start_iy + 2048) {
        let y = (iy as f64 * step) as f32;
        let is_major = iy.rem_euclid(4) == 0;
        let tick_len = if is_major { major_tick } else { minor_tick };
        mesh.push_quad(
            [x0 as f32 + ruler_band - tick_len, y - line_half],
            [x0 as f32 + ruler_band, y + line_half],
            tick_col,
            [PRIM_FLAG_OVERLAY_UI, 0.0],
        );
    }

    mesh
}

/// Built-in crisp Hershey-style vector stroke glyph renderer so shape labels render cleanly
/// on GPU and software targets without external OS font dependencies.
fn tessellate_vector_label(
    visual: &EvaluatedShapeVisual,
    tol: f32,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
) {
    let text = visual.text.trim();
    if text.is_empty() {
        return;
    }
    let cap_h = ((visual.font_size_pt.clamp(5.0, 96.0)) / 72.0) * 0.72;
    let glyph_w = cap_h * 0.58;
    let advance = glyph_w * 1.28;
    let stroke_w = (cap_h * 0.12).clamp(0.004, 0.08) as f32;
    let text_rgba = visual.text_color.to_linear_f32();

    let cx = visual.txt_pos[0];
    let cy = visual.txt_pos[1];

    let lines: Vec<&str> = text.lines().take(6).collect();
    if lines.is_empty() {
        return;
    }
    let line_h = cap_h * 1.45;
    let total_h = (lines.len() as f64 - 1.0) * line_h + cap_h;
    let top_baseline = cy + total_h * 0.5 - cap_h;

    let mut b = Path::builder();
    let mut any_stroke = false;

    for (line_idx, line) in lines.iter().enumerate() {
        let chars: Vec<char> = line.chars().take(64).collect();
        if chars.is_empty() {
            continue;
        }
        let line_w = (chars.len() as f64 - 1.0) * advance + glyph_w;
        let start_x = cx - line_w * 0.5;
        let base_y = top_baseline - (line_idx as f64) * line_h;

        for (ci, &ch) in chars.iter().enumerate() {
            let gx = start_x + (ci as f64) * advance;
            for seg in glyph_strokes(ch) {
                any_stroke = true;
                let p0 = [gx + (seg[0] as f64) * glyph_w, base_y + (seg[1] as f64) * cap_h];
                let p1 = [gx + (seg[2] as f64) * glyph_w, base_y + (seg[3] as f64) * cap_h];
                b.begin(point(p0[0] as f32, p0[1] as f32));
                b.line_to(point(p1[0] as f32, p1[1] as f32));
                b.end(false);
            }
        }
    }

    if !any_stroke {
        return;
    }
    let path = b.build();
    let _ = stroke_tess.tessellate_path(
        &path,
        &StrokeOptions::tolerance(tol)
            .with_line_width(stroke_w)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round),
        &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], text_rgba, [PRIM_FLAG_TEXT_GLYPH, 0.0])
        }),
    );
}

/// Normalized `[x0, y0, x1, y1]` vector segments in `[0..1] x [0..1]` (`+Y` up) for ASCII glyphs.
fn glyph_strokes(ch: char) -> &'static [[f32; 4]] {
    match ch.to_ascii_uppercase() {
        'A' => &[
            [0.0, 0.0, 0.5, 1.0],
            [0.5, 1.0, 1.0, 0.0],
            [0.2, 0.38, 0.8, 0.38],
        ],
        'B' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.78, 1.0],
            [0.78, 1.0, 0.95, 0.75],
            [0.95, 0.75, 0.78, 0.52],
            [0.0, 0.52, 0.78, 0.52],
            [0.78, 0.52, 1.0, 0.25],
            [1.0, 0.25, 0.78, 0.0],
            [0.78, 0.0, 0.0, 0.0],
        ],
        'C' => &[
            [0.95, 0.85, 0.65, 1.0],
            [0.65, 1.0, 0.25, 1.0],
            [0.25, 1.0, 0.0, 0.75],
            [0.0, 0.75, 0.0, 0.25],
            [0.0, 0.25, 0.25, 0.0],
            [0.25, 0.0, 0.65, 0.0],
            [0.65, 0.0, 0.95, 0.15],
        ],
        'D' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.65, 1.0],
            [0.65, 1.0, 1.0, 0.72],
            [1.0, 0.72, 1.0, 0.28],
            [1.0, 0.28, 0.65, 0.0],
            [0.65, 0.0, 0.0, 0.0],
        ],
        'E' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.95, 1.0],
            [0.0, 0.5, 0.78, 0.5],
            [0.0, 0.0, 0.95, 0.0],
        ],
        'F' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.95, 1.0],
            [0.0, 0.5, 0.75, 0.5],
        ],
        'G' => &[
            [0.95, 0.85, 0.65, 1.0],
            [0.65, 1.0, 0.25, 1.0],
            [0.25, 1.0, 0.0, 0.75],
            [0.0, 0.75, 0.0, 0.25],
            [0.0, 0.25, 0.25, 0.0],
            [0.25, 0.0, 0.95, 0.0],
            [0.95, 0.0, 0.95, 0.45],
            [0.55, 0.45, 0.95, 0.45],
        ],
        'H' => &[
            [0.0, 0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0, 1.0],
            [0.0, 0.5, 1.0, 0.5],
        ],
        'I' => &[
            [0.2, 1.0, 0.8, 1.0],
            [0.5, 0.0, 0.5, 1.0],
            [0.2, 0.0, 0.8, 0.0],
        ],
        'J' => &[
            [0.25, 1.0, 0.95, 1.0],
            [0.75, 1.0, 0.75, 0.22],
            [0.75, 0.22, 0.5, 0.0],
            [0.5, 0.0, 0.2, 0.0],
            [0.2, 0.0, 0.0, 0.22],
        ],
        'K' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.95, 1.0, 0.0, 0.48],
            [0.28, 0.62, 0.98, 0.0],
        ],
        'L' => &[[0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.95, 0.0]],
        'M' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.5, 0.35],
            [0.5, 0.35, 1.0, 1.0],
            [1.0, 1.0, 1.0, 0.0],
        ],
        'N' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 1.0, 0.0],
            [1.0, 0.0, 1.0, 1.0],
        ],
        'O' | '0' => &[
            [0.25, 1.0, 0.75, 1.0],
            [0.75, 1.0, 1.0, 0.75],
            [1.0, 0.75, 1.0, 0.25],
            [1.0, 0.25, 0.75, 0.0],
            [0.75, 0.0, 0.25, 0.0],
            [0.25, 0.0, 0.0, 0.25],
            [0.0, 0.25, 0.0, 0.75],
            [0.0, 0.75, 0.25, 1.0],
        ],
        'P' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.78, 1.0],
            [0.78, 1.0, 1.0, 0.75],
            [1.0, 0.75, 0.78, 0.5],
            [0.78, 0.5, 0.0, 0.5],
        ],
        'Q' => &[
            [0.25, 1.0, 0.75, 1.0],
            [0.75, 1.0, 1.0, 0.75],
            [1.0, 0.75, 1.0, 0.25],
            [1.0, 0.25, 0.75, 0.0],
            [0.75, 0.0, 0.25, 0.0],
            [0.25, 0.0, 0.0, 0.25],
            [0.0, 0.25, 0.0, 0.75],
            [0.0, 0.75, 0.25, 1.0],
            [0.58, 0.28, 1.0, -0.08],
        ],
        'R' => &[
            [0.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.78, 1.0],
            [0.78, 1.0, 1.0, 0.75],
            [1.0, 0.75, 0.78, 0.5],
            [0.78, 0.5, 0.0, 0.5],
            [0.52, 0.5, 1.0, 0.0],
        ],
        'S' | '5' => &[
            [0.95, 0.85, 0.7, 1.0],
            [0.7, 1.0, 0.2, 1.0],
            [0.2, 1.0, 0.0, 0.75],
            [0.0, 0.75, 0.2, 0.52],
            [0.2, 0.52, 0.8, 0.48],
            [0.8, 0.48, 1.0, 0.25],
            [1.0, 0.25, 0.75, 0.0],
            [0.75, 0.0, 0.2, 0.0],
            [0.2, 0.0, 0.0, 0.15],
        ],
        'T' => &[[0.0, 1.0, 1.0, 1.0], [0.5, 0.0, 0.5, 1.0]],
        'U' => &[
            [0.0, 1.0, 0.0, 0.22],
            [0.0, 0.22, 0.25, 0.0],
            [0.25, 0.0, 0.75, 0.0],
            [0.75, 0.0, 1.0, 0.22],
            [1.0, 0.22, 1.0, 1.0],
        ],
        'V' => &[[0.0, 1.0, 0.5, 0.0], [0.5, 0.0, 1.0, 1.0]],
        'W' => &[
            [0.0, 1.0, 0.22, 0.0],
            [0.22, 0.0, 0.5, 0.62],
            [0.5, 0.62, 0.78, 0.0],
            [0.78, 0.0, 1.0, 1.0],
        ],
        'X' => &[[0.0, 0.0, 1.0, 1.0], [0.0, 1.0, 1.0, 0.0]],
        'Y' => &[
            [0.0, 1.0, 0.5, 0.48],
            [1.0, 1.0, 0.5, 0.48],
            [0.5, 0.48, 0.5, 0.0],
        ],
        'Z' | '2' => &[
            [0.0, 1.0, 1.0, 1.0],
            [1.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        '1' => &[
            [0.2, 0.78, 0.5, 1.0],
            [0.5, 1.0, 0.5, 0.0],
            [0.18, 0.0, 0.82, 0.0],
        ],
        '3' => &[
            [0.05, 0.88, 0.3, 1.0],
            [0.3, 1.0, 0.8, 1.0],
            [0.8, 1.0, 0.98, 0.75],
            [0.98, 0.75, 0.7, 0.52],
            [0.35, 0.52, 0.7, 0.52],
            [0.7, 0.52, 1.0, 0.25],
            [1.0, 0.25, 0.75, 0.0],
            [0.75, 0.0, 0.15, 0.0],
        ],
        '4' => &[
            [0.75, 0.0, 0.75, 1.0],
            [0.75, 1.0, 0.0, 0.32],
            [0.0, 0.32, 1.0, 0.32],
        ],
        '6' => &[
            [0.85, 1.0, 0.25, 1.0],
            [0.25, 1.0, 0.0, 0.65],
            [0.0, 0.65, 0.0, 0.22],
            [0.0, 0.22, 0.25, 0.0],
            [0.25, 0.0, 0.8, 0.0],
            [0.8, 0.0, 1.0, 0.28],
            [1.0, 0.28, 0.78, 0.52],
            [0.78, 0.52, 0.0, 0.52],
        ],
        '7' => &[[0.0, 1.0, 1.0, 1.0], [1.0, 1.0, 0.35, 0.0]],
        '8' => &[
            [0.2, 1.0, 0.8, 1.0],
            [0.8, 1.0, 0.98, 0.75],
            [0.98, 0.75, 0.8, 0.52],
            [0.8, 0.52, 0.2, 0.52],
            [0.2, 0.52, 0.02, 0.75],
            [0.02, 0.75, 0.2, 1.0],
            [0.8, 0.52, 1.0, 0.25],
            [1.0, 0.25, 0.8, 0.0],
            [0.8, 0.0, 0.2, 0.0],
            [0.2, 0.0, 0.0, 0.25],
            [0.0, 0.25, 0.2, 0.52],
        ],
        '9' => &[
            [1.0, 0.48, 0.22, 0.48],
            [0.22, 0.48, 0.0, 0.72],
            [0.0, 0.72, 0.22, 1.0],
            [0.22, 1.0, 0.78, 1.0],
            [0.78, 1.0, 1.0, 0.72],
            [1.0, 0.72, 1.0, 0.22],
            [1.0, 0.22, 0.75, 0.0],
            [0.75, 0.0, 0.2, 0.0],
        ],
        '-' => &[[0.12, 0.5, 0.88, 0.5]],
        '+' => &[[0.12, 0.5, 0.88, 0.5], [0.5, 0.15, 0.5, 0.85]],
        '_' => &[[0.0, 0.0, 1.0, 0.0]],
        '.' => &[[0.44, 0.0, 0.56, 0.0], [0.5, 0.0, 0.5, 0.08]],
        ':' => &[[0.5, 0.18, 0.5, 0.26], [0.5, 0.72, 0.5, 0.80]],
        '/' => &[[0.08, 0.0, 0.92, 1.0]],
        '(' => &[
            [0.72, 1.0, 0.35, 0.72],
            [0.35, 0.72, 0.35, 0.28],
            [0.35, 0.28, 0.72, 0.0],
        ],
        ')' => &[
            [0.28, 1.0, 0.65, 0.72],
            [0.65, 0.72, 0.65, 0.28],
            [0.65, 0.28, 0.28, 0.0],
        ],
        ' ' => &[],
        _ => &[
            [0.15, 0.15, 0.85, 0.15],
            [0.85, 0.15, 0.85, 0.85],
            [0.85, 0.85, 0.15, 0.85],
            [0.15, 0.85, 0.15, 0.15],
        ],
    }
}

/// Deterministic 64-bit FNV-1a content hash of an `EvaluatedShapeVisual` for `MeshCache` invalidation.
pub fn compute_visual_content_hash(v: &EvaluatedShapeVisual) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h = OFFSET_BASIS;
    fn hash_u64(h: &mut u64, x: u64) {
        for b in x.to_le_bytes() {
            *h ^= b as u64;
            *h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn hash_f64(h: &mut u64, v: f64) {
        let norm = if v == 0.0 { 0.0 } else { v };
        hash_u64(h, norm.to_bits());
    }
    fn hash_rgba(h: &mut u64, c: Option<RgbaColor>) {
        match c {
            Some(c) => hash_u64(
                h,
                1u64 | ((c.r as u64) << 8)
                    | ((c.g as u64) << 16)
                    | ((c.b as u64) << 24)
                    | ((c.a as u64) << 32),
            ),
            None => hash_u64(h, 0),
        }
    }

    hash_u64(&mut h, v.shape_id);
    hash_f64(&mut h, v.transform.pin_x);
    hash_f64(&mut h, v.transform.pin_y);
    hash_f64(&mut h, v.transform.width);
    hash_f64(&mut h, v.transform.height);
    hash_f64(&mut h, v.transform.angle_rad);
    hash_rgba(&mut h, v.fill_color);
    hash_rgba(&mut h, v.stroke_color);
    hash_f64(&mut h, v.stroke_weight_in);
    hash_f64(&mut h, v.rounding_in);
    hash_u64(&mut h, (v.begin_arrow as u64) | ((v.end_arrow as u64) << 8));
    hash_f64(&mut h, v.font_size_pt);
    hash_f64(&mut h, v.txt_pos[0]);
    hash_f64(&mut h, v.txt_pos[1]);
    hash_rgba(&mut h, Some(v.text_color));
    hash_rgba(&mut h, v.text_bg_color);
    hash_u64(
        &mut h,
        (v.char_style as u64)
            | ((v.horiz_align as u64) << 8)
            | ((v.vert_align as u64) << 16)
            | ((v.bullet_style as u64) << 24)
            | ((v.transform.flip_x as u64) << 32)
            | ((v.transform.flip_y as u64) << 33),
    );
    for b in v.font_family.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    for b in v.text.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    hash_u64(&mut h, v.paths.len() as u64);
    for gp in &v.paths {
        let flags = (gp.no_fill as u64) | ((gp.no_line as u64) << 1) | ((gp.no_show as u64) << 2);
        hash_u64(&mut h, flags);
        hash_rgba(&mut h, gp.fill_color);
        hash_rgba(&mut h, gp.stroke_color);
        hash_f64(&mut h, gp.stroke_weight_in.unwrap_or(-1.0));
        hash_u64(&mut h, gp.verbs_page_space.len() as u64);
        for verb in &gp.verbs_page_space {
            match *verb {
                EvaluatedPathVerb::MoveTo(p) => {
                    hash_u64(&mut h, 1);
                    hash_f64(&mut h, p[0]);
                    hash_f64(&mut h, p[1]);
                }
                EvaluatedPathVerb::LineTo(p) => {
                    hash_u64(&mut h, 2);
                    hash_f64(&mut h, p[0]);
                    hash_f64(&mut h, p[1]);
                }
                EvaluatedPathVerb::QuadTo { ctrl, to } => {
                    hash_u64(&mut h, 3);
                    hash_f64(&mut h, ctrl[0]);
                    hash_f64(&mut h, ctrl[1]);
                    hash_f64(&mut h, to[0]);
                    hash_f64(&mut h, to[1]);
                }
                EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                    hash_u64(&mut h, 4);
                    hash_f64(&mut h, ctrl1[0]);
                    hash_f64(&mut h, ctrl1[1]);
                    hash_f64(&mut h, ctrl2[0]);
                    hash_f64(&mut h, ctrl2[1]);
                    hash_f64(&mut h, to[0]);
                    hash_f64(&mut h, to[1]);
                }
                EvaluatedPathVerb::ArcTo { to, bow } => {
                    hash_u64(&mut h, 5);
                    hash_f64(&mut h, to[0]);
                    hash_f64(&mut h, to[1]);
                    hash_f64(&mut h, bow);
                }
                EvaluatedPathVerb::Ellipse {
                    center,
                    rx,
                    ry,
                    angle_rad,
                } => {
                    hash_u64(&mut h, 6);
                    hash_f64(&mut h, center[0]);
                    hash_f64(&mut h, center[1]);
                    hash_f64(&mut h, rx);
                    hash_f64(&mut h, ry);
                    hash_f64(&mut h, angle_rad);
                }
                EvaluatedPathVerb::Close => {
                    hash_u64(&mut h, 7);
                }
            }
        }
    }
    h
}

#[inline]
fn is_finite_pt(p: [f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

#[inline]
fn dist_2d(a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}
