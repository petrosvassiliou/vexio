//! Vector path tessellation (`lyon_path` + `lyon_tessellation`), level-of-detail (LOD)
//! sub-3px proxy quads, selection/ruler/ink geometry builders, and content-addressed `MeshCache`.

use lyon_path::iterator::PathIterator;
use lyon_path::math::point;
use lyon_path::{Event, Path};
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin,
    StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::{FRAC_PI_4, PI, TAU};
use vexio_shapesheet::{
    DataGraphicIconKind, EvaluatedDataGraphicOverlay, EvaluatedGeometryPath, EvaluatedGradientStop,
    EvaluatedPathVerb, EvaluatedShapeVisual, MarkupInkStroke, RgbaColor, ShapeKind,
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
    let Some(base_col) = visual.fill_color.or(visual.stroke_color) else {
        // Pure transparent text boxes (`FillPattern=0`, `LinePattern=0`) must not draw a solid LOD rectangle.
        return;
    };
    let color_rgba = base_col.to_linear_f32();
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

/// Normalize text rotation angle so upside-down shapes (`cos(angle) < 0`) flip `+180°` to remain readable, matching Visio.
#[inline]
pub fn readable_text_angle_rad(raw_angle_rad: f64) -> f64 {
    if !raw_angle_rad.is_finite() {
        return 0.0;
    }
    if raw_angle_rad.cos() < -1e-4 {
        raw_angle_rad + std::f64::consts::PI
    } else {
        raw_angle_rad
    }
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
/// - Closed-only Visio fill rules (open figures never fill diagonally)
/// - Dashed/dotted `line_pattern >= 2` stroke tessellation
/// - Corner fillet rounding (`rounding_in`), `begin_arrow` and `end_arrow` arrowheads
/// - Embedded `<ForeignData>` vector/raster media (`SVG`, `PNG`, `BMP`, `EMF`)
/// - Full-Unicode OS-font-aware rich text shaper for non-empty `visual.text`
pub fn tessellate_shape_full(
    visual: &EvaluatedShapeVisual,
    tolerance_in: f32,
) -> TessellatedMesh {
    let tol = if tolerance_in.is_finite() {
        tolerance_in.clamp(0.0002, 0.05)
    } else {
        0.0035
    };
    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    let stroke_w = if visual.stroke_weight_in.is_finite() {
        (visual.stroke_weight_in.clamp(1.0 / 144.0, 2.0)) as f32
    } else {
        (1.0 / 72.0) as f32
    };
    let rounding = if visual.rounding_in.is_finite() && visual.rounding_in > 0.0 {
        visual.rounding_in.min(24.0)
    } else {
        0.0
    };
    let fill_rgba = visual.fill_color.map(RgbaColor::to_linear_f32);
    let stroke_rgba = visual.stroke_color.map(RgbaColor::to_linear_f32);
    let grad_spec = if visual.gradient_stops.len() >= 2 && visual.fill_color.is_some() {
        Some(GradientFillSpec {
            aabb_min: visual.aabb_min,
            aabb_max: visual.aabb_max,
            angle_rad: visual.fill_gradient_angle_rad,
            radial: visual.fill_gradient_radial,
            stops: &visual.gradient_stops,
        })
    } else {
        None
    };

    let mut first_tangent: Option<([f64; 2], [f64; 2])> = None;
    let mut last_tangent: Option<([f64; 2], [f64; 2])> = None;

    let shared_fill_count = visual
        .paths
        .iter()
        .filter(|g| !g.no_show && !g.no_fill && g.fill_color.is_none() && !g.verbs_page_space.is_empty())
        .count();
    let use_compound_fill = shared_fill_count >= 2 && (fill_rgba.is_some() || grad_spec.is_some());
    if use_compound_fill {
        let mut compound_verbs = Vec::new();
        for geom in &visual.paths {
            if !geom.no_show && !geom.no_fill && geom.fill_color.is_none() && !geom.verbs_page_space.is_empty() {
                compound_verbs.extend_from_slice(&geom.verbs_page_space);
            }
        }
        let compound_geom = EvaluatedGeometryPath {
            no_fill: false,
            no_line: true,
            no_show: false,
            verbs_page_space: compound_verbs,
            fill_color: None,
            stroke_color: None,
            stroke_weight_in: None,
        };
        let mut dummy_t0 = None;
        let mut dummy_t1 = None;
        tessellate_geometry_section(
            &compound_geom,
            rounding,
            0,
            fill_rgba,
            grad_spec.as_ref(),
            None,
            stroke_w,
            tol,
            &mut fill_tess,
            &mut stroke_tess,
            &mut lyon_buf,
            &mut dummy_t0,
            &mut dummy_t1,
        );
    }

    for geom in &visual.paths {
        if geom.no_show || geom.verbs_page_space.is_empty() {
            continue;
        }
        let is_shared_fill = !geom.no_fill && geom.fill_color.is_none();
        let g_fill = if use_compound_fill && is_shared_fill {
            None
        } else {
            geom.fill_color.map(RgbaColor::to_linear_f32).or(fill_rgba)
        };
        let g_grad = if (use_compound_fill && is_shared_fill) || geom.fill_color.is_some() {
            None
        } else {
            grad_spec.as_ref()
        };
        let g_stroke = geom.stroke_color.map(RgbaColor::to_linear_f32).or(stroke_rgba);
        let g_sw = geom
            .stroke_weight_in
            .filter(|w| w.is_finite())
            .map(|w| w.clamp(1.0 / 144.0, 2.0) as f32)
            .unwrap_or(stroke_w);
        tessellate_geometry_section(
            geom,
            rounding,
            visual.line_pattern,
            g_fill,
            g_grad,
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

    // Arrowheads at Begin and End of paths when configured (`begin_arrow > 0` or `end_arrow > 0`).
    if let Some(sc) = stroke_rgba {
        if visual.begin_arrow > 0 {
            if let Some((tip, next_pt)) = first_tangent {
                // Arrow points backward toward `tip` from `next_pt`.
                tessellate_arrowhead(
                    visual.begin_arrow,
                    visual.begin_arrow_size,
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
                    visual.end_arrow_size,
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

    let mut out_mesh = TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    };

    if let Some(media) = &visual.foreign_media {
        let foreign_mesh =
            crate::foreign_media::tessellate_foreign_media(media, &visual.transform, tol);
        out_mesh.append(&foreign_mesh);
    }

    // Crisp full-Unicode TrueType/OpenType vector text for shape labels.
    let trimmed_txt = visual.text.trim();
    if !trimmed_txt.is_empty() {
        let default_w = if visual.kind == ShapeKind::Connector1D {
            visual.transform.width.abs().max(2.2)
        } else {
            visual.transform.width.abs().max(0.45)
        };
        let default_h = if visual.kind == ShapeKind::Connector1D {
            visual.transform.height.abs().max(0.5)
        } else {
            visual.transform.height.abs().max(0.28)
        };
        let box_w = visual.txt_width.unwrap_or(default_w).abs().max(0.45);
        let box_h = visual.txt_height.unwrap_or(default_h).abs().max(0.28);
        let text_angle = if visual.txt_angle_rad.abs() > 1e-6 {
            visual.transform.angle_rad + visual.txt_angle_rad
        } else {
            readable_text_angle_rad(visual.transform.angle_rad)
        };

        let block = if !visual.evaluated_text_runs.is_empty() {
            crate::typography::build_rich_text_block_from_evaluated_runs(
                &visual.evaluated_text_runs,
                &visual.font_family,
                visual.horiz_align,
                visual.vert_align,
                visual.bullet_style,
            )
        } else {
            crate::typography::parse_inline_rich_text_styled_with_font(
                trimmed_txt,
                visual.font_size_pt,
                visual.text_color,
                visual.char_style,
                visual.horiz_align,
                visual.vert_align,
                visual.bullet_style,
                &visual.font_family,
            )
        };

        if let Some(bg) = visual.text_bg_color {
            let bg_rgba = bg.to_linear_f32();
            let (span_w, span_h) = crate::typography::measure_rich_text_bounds_in(&block, box_w, box_h);
            let pad = 0.05;
            let local_cx = match visual.horiz_align {
                0 => -box_w * 0.5 + pad + span_w * 0.5,
                2 => box_w * 0.5 - pad - span_w * 0.5,
                _ => 0.0,
            };
            let local_cy = match visual.vert_align {
                0 => box_h * 0.5 - pad - span_h * 0.5,
                2 => -box_h * 0.5 + pad + span_h * 0.5,
                _ => 0.0,
            };
            let hw = span_w * 0.5;
            let hh = span_h * 0.5;
            let (sa, ca) = text_angle.sin_cos();
            let rot_pt = |lx: f64, ly: f64| -> [f32; 2] {
                [
                    (visual.txt_pos[0] + lx * ca - ly * sa) as f32,
                    (visual.txt_pos[1] + lx * sa + ly * ca) as f32,
                ]
            };
            let c0 = rot_pt(local_cx - hw, local_cy - hh);
            let c1 = rot_pt(local_cx + hw, local_cy - hh);
            let c2 = rot_pt(local_cx + hw, local_cy + hh);
            let c3 = rot_pt(local_cx - hw, local_cy + hh);
            let base = out_mesh.vertices.len() as u32;
            for pos in [c0, c1, c2, c3] {
                out_mesh.vertices.push(GpuVertex {
                    pos,
                    color: bg_rgba,
                    flags: [PRIM_FLAG_SHAPE, 0.0],
                });
            }
            out_mesh
                .indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }

        let rich_mesh = crate::typography::tessellate_rich_text_in_box(
            &block,
            visual.txt_pos[0],
            visual.txt_pos[1],
            box_w,
            box_h,
            text_angle,
        );
        out_mesh.append(&rich_mesh);
    }

    if let Some(dg) = &visual.data_graphics {
        let dg_mesh = tessellate_data_graphics_overlay(dg, &visual.font_family, tol);
        out_mesh.append(&dg_mesh);
    }

    out_mesh
}

/// Tessellate Visio Plan 2 Data Graphics (`Icon Sets`, `Data Bars`, `Text Callouts`) into crisp GPU/PDF triangles.
pub fn tessellate_data_graphics_overlay(
    dg: &EvaluatedDataGraphicOverlay,
    font_family: &str,
    tol: f32,
) -> TessellatedMesh {
    let mut out = TessellatedMesh::new();
    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();
    let eff_tol = tol.clamp(0.0005, 0.01);

    let emit_disc = |buf: &mut VertexBuffers<GpuVertex, u32>,
                     ft: &mut FillTessellator,
                     cx: f32,
                     cy: f32,
                     r: f32,
                     rgba: [f32; 4]| {
        if r <= 1e-5 {
            return;
        }
        let mut pb = Path::builder();
        let segs = 20usize;
        for i in 0..segs {
            let ang = (i as f32) * (std::f32::consts::TAU / (segs as f32));
            let (sa, ca) = ang.sin_cos();
            let p = point(cx + r * ca, cy + r * sa);
            if i == 0 {
                pb.begin(p);
            } else {
                pb.line_to(p);
            }
        }
        pb.close();
        let path = pb.build();
        let _ = ft.tessellate_path(
            &path,
            &FillOptions::tolerance(eff_tol),
            &mut BuffersBuilder::new(buf, |v: FillVertex| {
                GpuVertex::new(v.position().to_array(), rgba, [PRIM_FLAG_SHAPE, 0.0])
            }),
        );
    };

    let emit_rounded_rect = |buf: &mut VertexBuffers<GpuVertex, u32>,
                             ft: &mut FillTessellator,
                             st: &mut StrokeTessellator,
                             min: [f32; 2],
                             max: [f32; 2],
                             radius: f32,
                             fill_opt: Option<[f32; 4]>,
                             stroke_opt: Option<([f32; 4], f32)>| {
        let w = (max[0] - min[0]).max(0.01);
        let h = (max[1] - min[1]).max(0.01);
        let r = radius.min(w * 0.48).min(h * 0.48).max(0.0);
        let mut pb = Path::builder();
        if r <= 1e-4 {
            pb.begin(point(min[0], min[1]));
            pb.line_to(point(max[0], min[1]));
            pb.line_to(point(max[0], max[1]));
            pb.line_to(point(min[0], max[1]));
            pb.close();
        } else {
            let k = 0.552_284_8 * r;
            pb.begin(point(min[0] + r, min[1]));
            pb.line_to(point(max[0] - r, min[1]));
            pb.cubic_bezier_to(
                point(max[0] - r + k, min[1]),
                point(max[0], min[1] + r - k),
                point(max[0], min[1] + r),
            );
            pb.line_to(point(max[0], max[1] - r));
            pb.cubic_bezier_to(
                point(max[0], max[1] - r + k),
                point(max[0] - r + k, max[1]),
                point(max[0] - r, max[1]),
            );
            pb.line_to(point(min[0] + r, max[1]));
            pb.cubic_bezier_to(
                point(min[0] + r - k, max[1]),
                point(min[0], max[1] - r + k),
                point(min[0], max[1] - r),
            );
            pb.line_to(point(min[0], min[1] + r));
            pb.cubic_bezier_to(
                point(min[0], min[1] + r - k),
                point(min[0] + r - k, min[1]),
                point(min[0] + r, min[1]),
            );
            pb.close();
        }
        let path = pb.build();
        if let Some(fc) = fill_opt {
            let _ = ft.tessellate_path(
                &path,
                &FillOptions::tolerance(eff_tol),
                &mut BuffersBuilder::new(buf, |v: FillVertex| {
                    GpuVertex::new(v.position().to_array(), fc, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
        if let Some((sc, sw)) = stroke_opt {
            let opts = StrokeOptions::tolerance(eff_tol)
                .with_line_width(sw)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round);
            let _ = st.tessellate_path(
                &path,
                &opts,
                &mut BuffersBuilder::new(buf, |v: StrokeVertex| {
                    GpuVertex::new(v.position().to_array(), sc, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    };

    // 1. Icon Set Vector Badge
    if let Some(badge) = &dg.icon_badge {
        let cx = badge.center_page_in[0] as f32;
        let cy = badge.center_page_in[1] as f32;
        let r = badge.radius_in.max(0.05) as f32;
        let white = RgbaColor::WHITE.to_linear_f32();
        let fill = badge.fill_color.to_linear_f32();

        emit_disc(&mut lyon_buf, &mut fill_tess, cx, cy, r + 0.018, white);
        emit_disc(&mut lyon_buf, &mut fill_tess, cx, cy, r, fill);

        let glyph_sw = (r * 0.24).max(0.015);
        let stroke_opts = StrokeOptions::tolerance(eff_tol)
            .with_line_width(glyph_sw)
            .with_line_cap(LineCap::Round)
            .with_line_join(LineJoin::Round);
        match badge.kind {
            DataGraphicIconKind::CheckOk => {
                let mut pb = Path::builder();
                pb.begin(point(cx - r * 0.42, cy - r * 0.02));
                pb.line_to(point(cx - r * 0.10, cy - r * 0.34));
                pb.line_to(point(cx + r * 0.45, cy + r * 0.32));
                pb.end(false);
                let _ = stroke_tess.tessellate_path(
                    &pb.build(),
                    &stroke_opts,
                    &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                        GpuVertex::new(v.position().to_array(), white, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            }
            DataGraphicIconKind::WarnTriangle => {
                let mut pb = Path::builder();
                pb.begin(point(cx, cy + r * 0.42));
                pb.line_to(point(cx, cy - r * 0.05));
                pb.end(false);
                let _ = stroke_tess.tessellate_path(
                    &pb.build(),
                    &stroke_opts,
                    &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                        GpuVertex::new(v.position().to_array(), white, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
                emit_disc(&mut lyon_buf, &mut fill_tess, cx, cy - r * 0.36, glyph_sw * 0.62, white);
            }
            DataGraphicIconKind::ErrorCross => {
                let mut pb = Path::builder();
                pb.begin(point(cx - r * 0.34, cy - r * 0.34));
                pb.line_to(point(cx + r * 0.34, cy + r * 0.34));
                pb.end(false);
                pb.begin(point(cx - r * 0.34, cy + r * 0.34));
                pb.line_to(point(cx + r * 0.34, cy - r * 0.34));
                pb.end(false);
                let _ = stroke_tess.tessellate_path(
                    &pb.build(),
                    &stroke_opts,
                    &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                        GpuVertex::new(v.position().to_array(), white, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            }
            DataGraphicIconKind::InfoCircle => {
                emit_disc(&mut lyon_buf, &mut fill_tess, cx, cy + r * 0.35, glyph_sw * 0.58, white);
                let mut pb = Path::builder();
                pb.begin(point(cx, cy + r * 0.08));
                pb.line_to(point(cx, cy - r * 0.38));
                pb.end(false);
                let _ = stroke_tess.tessellate_path(
                    &pb.build(),
                    &stroke_opts,
                    &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                        GpuVertex::new(v.position().to_array(), white, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            }
        }
    }

    // 2. Data Bar Progress Track & Fill
    if let Some(bar) = &dg.data_bar {
        let min = [bar.rect_min_in[0] as f32, bar.rect_min_in[1] as f32];
        let max = [bar.rect_max_in[0] as f32, bar.rect_max_in[1] as f32];
        let track_col = RgbaColor {
            r: 226,
            g: 232,
            b: 240,
            a: 255,
        }
        .to_linear_f32();
        let border_col = RgbaColor {
            r: 148,
            g: 163,
            b: 184,
            a: 255,
        }
        .to_linear_f32();
        emit_rounded_rect(
            &mut lyon_buf,
            &mut fill_tess,
            &mut stroke_tess,
            min,
            max,
            0.04,
            Some(track_col),
            Some((border_col, (0.65 / 72.0) as f32)),
        );
        let pad = 0.012f32;
        let inner_w = (max[0] - min[0] - pad * 2.0).max(0.0);
        let fill_w = (inner_w * (bar.ratio.clamp(0.0, 1.0) as f32)).max(if bar.ratio > 0.0 {
            0.025
        } else {
            0.0
        });
        if fill_w > 0.002 {
            emit_rounded_rect(
                &mut lyon_buf,
                &mut fill_tess,
                &mut stroke_tess,
                [min[0] + pad, min[1] + pad],
                [min[0] + pad + fill_w, max[1] - pad],
                0.03,
                Some(bar.bar_color.to_linear_f32()),
                None,
            );
        }
    }

    // 3. Text Callout Pill
    if let Some(callout) = &dg.text_callout {
        let min = [callout.rect_min_in[0] as f32, callout.rect_min_in[1] as f32];
        let max = [callout.rect_max_in[0] as f32, callout.rect_max_in[1] as f32];
        emit_rounded_rect(
            &mut lyon_buf,
            &mut fill_tess,
            &mut stroke_tess,
            min,
            max,
            0.055,
            Some(callout.bg_color.to_linear_f32()),
            Some((callout.border_color.to_linear_f32(), (0.75 / 72.0) as f32)),
        );
        // Accent pill indicator on leading edge
        emit_rounded_rect(
            &mut lyon_buf,
            &mut fill_tess,
            &mut stroke_tess,
            [min[0] + 0.025, min[1] + 0.035],
            [min[0] + 0.085, max[1] - 0.035],
            0.025,
            Some(callout.accent_color.to_linear_f32()),
            None,
        );
    }

    out.vertices = lyon_buf.vertices;
    out.indices = lyon_buf.indices;

    // Render crisp glyph labels for Data Bar & Text Callout on top of background shapes
    if let Some(bar) = &dg.data_bar {
        if !bar.value_label.is_empty() {
            let bw = (bar.rect_max_in[0] - bar.rect_min_in[0]).abs().max(0.35);
            let bh = (bar.rect_max_in[1] - bar.rect_min_in[1]).abs().max(0.12);
            let txt_col = if bar.ratio > 0.55 {
                RgbaColor::WHITE
            } else {
                RgbaColor {
                    r: 15,
                    g: 23,
                    b: 42,
                    a: 255,
                }
            };
            let block = crate::typography::parse_inline_rich_text_styled_with_font(
                &bar.value_label,
                7.2,
                txt_col,
                1,
                1,
                1,
                0,
                font_family,
            );
            let lbl_mesh = crate::typography::tessellate_rich_text_in_box(
                &block,
                0.5 * (bar.rect_min_in[0] + bar.rect_max_in[0]),
                0.5 * (bar.rect_min_in[1] + bar.rect_max_in[1]),
                bw,
                bh + 0.06,
                0.0,
            );
            out.append(&lbl_mesh);
        }
    }

    if let Some(callout) = &dg.text_callout {
        if !callout.text.is_empty() {
            let x0 = callout.rect_min_in[0] + 0.095;
            let x1 = callout.rect_max_in[0] - 0.03;
            let bw = (x1 - x0).max(0.4);
            let bh = (callout.rect_max_in[1] - callout.rect_min_in[1]).abs().max(0.16);
            let block = crate::typography::parse_inline_rich_text_styled_with_font(
                &callout.text,
                7.8,
                callout.text_color,
                1,
                1,
                1,
                0,
                font_family,
            );
            let callout_mesh = crate::typography::tessellate_rich_text_in_box(
                &block,
                0.5 * (x0 + x1),
                0.5 * (callout.rect_min_in[1] + callout.rect_max_in[1]),
                bw,
                bh + 0.06,
                0.0,
            );
            out.append(&callout_mesh);
        }
    }

    out
}

struct GradientFillSpec<'a> {
    aabb_min: [f64; 2],
    aabb_max: [f64; 2],
    angle_rad: f64,
    radial: bool,
    stops: &'a [EvaluatedGradientStop],
}

impl GradientFillSpec<'_> {
    fn sample_linear_rgba(&self, px: f32, py: f32) -> [f32; 4] {
        let cx = ((self.aabb_min[0] + self.aabb_max[0]) * 0.5) as f32;
        let cy = ((self.aabb_min[1] + self.aabb_max[1]) * 0.5) as f32;
        let hw = (((self.aabb_max[0] - self.aabb_min[0]).abs() * 0.5).max(1e-4)) as f32;
        let hh = (((self.aabb_max[1] - self.aabb_min[1]).abs() * 0.5).max(1e-4)) as f32;

        let t = if self.radial {
            let nx = (px - cx) / hw;
            let ny = (py - cy) / hh;
            ((nx * nx + ny * ny).sqrt() / std::f32::consts::SQRT_2).clamp(0.0, 1.0)
        } else {
            let (sa, ca) = (self.angle_rad as f32).sin_cos();
            let proj = (px - cx) * ca + (py - cy) * sa;
            let half_ext = (hw * ca.abs() + hh * sa.abs()).max(1e-4);
            ((proj / half_ext) * 0.5 + 0.5).clamp(0.0, 1.0)
        };

        if self.stops.is_empty() {
            return [1.0, 1.0, 1.0, 1.0];
        }
        if t <= self.stops[0].position {
            return self.stops[0].color.to_linear_f32();
        }
        let last = self.stops.last().unwrap();
        if t >= last.position {
            return last.color.to_linear_f32();
        }
        for w in self.stops.windows(2) {
            if t >= w[0].position && t <= w[1].position {
                let span = (w[1].position - w[0].position).max(1e-5);
                let u = ((t - w[0].position) / span).clamp(0.0, 1.0);
                let c0 = w[0].color.to_linear_f32();
                let c1 = w[1].color.to_linear_f32();
                return [
                    c0[0] + (c1[0] - c0[0]) * u,
                    c0[1] + (c1[1] - c0[1]) * u,
                    c0[2] + (c1[2] - c0[2]) * u,
                    c0[3] + (c1[3] - c0[3]) * u,
                ];
            }
        }
        last.color.to_linear_f32()
    }
}

fn subdivide_triangles_midpoint(
    verts: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    passes: usize,
) {
    for _ in 0..passes {
        if indices.len() / 3 > 512 {
            break;
        }
        let mut next_idx = Vec::with_capacity(indices.len() * 4);
        let mut edge_mid: BTreeMap<(u32, u32), u32> = BTreeMap::new();
        let mut midpoint_of = |a: u32, b: u32, vs: &mut Vec<[f32; 2]>| -> u32 {
            let key = if a <= b { (a, b) } else { (b, a) };
            if let Some(&m) = edge_mid.get(&key) {
                return m;
            }
            let pa = vs[a as usize];
            let pb = vs[b as usize];
            let mid_id = vs.len() as u32;
            vs.push([0.5 * (pa[0] + pb[0]), 0.5 * (pa[1] + pb[1])]);
            edge_mid.insert(key, mid_id);
            mid_id
        };
        for tri in indices.chunks_exact(3) {
            let (i0, i1, i2) = (tri[0], tri[1], tri[2]);
            let m01 = midpoint_of(i0, i1, verts);
            let m12 = midpoint_of(i1, i2, verts);
            let m20 = midpoint_of(i2, i0, verts);
            next_idx.extend_from_slice(&[
                i0, m01, m20,
                m01, i1, m12,
                m20, m12, i2,
                m01, m12, m20,
            ]);
        }
        *indices = next_idx;
    }
}

#[allow(clippy::too_many_arguments)]
fn tessellate_geometry_section(
    geom: &EvaluatedGeometryPath,
    rounding_in: f64,
    line_pattern: u8,
    fill_rgba: Option<[f32; 4]>,
    grad_spec: Option<&GradientFillSpec<'_>>,
    stroke_rgba: Option<[f32; 4]>,
    stroke_w: f32,
    tol: f32,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
    first_tangent: &mut Option<([f64; 2], [f64; 2])>,
    last_tangent: &mut Option<([f64; 2], [f64; 2])>,
) {
    let (fill_path, stroke_path) = build_lyon_paths_from_verbs(
        &geom.verbs_page_space,
        rounding_in,
        first_tangent,
        last_tangent,
    );

    // Visio ShapeSheet rule: only explicitly or geometrically CLOSED figures are filled.
    // Open figures (`MoveTo -> LineTo -> LineTo` where end != start) never fill across the diagonal!
    if !geom.no_fill {
        if let Some(grad) = grad_spec {
            let mut pos_buf: VertexBuffers<[f32; 2], u32> = VertexBuffers::new();
            let _ = fill_tess.tessellate_path(
                &fill_path,
                &FillOptions::tolerance(tol).with_fill_rule(FillRule::EvenOdd),
                &mut BuffersBuilder::new(&mut pos_buf, |vertex: FillVertex| {
                    let p = vertex.position();
                    [p.x, p.y]
                }),
            );
            if !pos_buf.indices.is_empty() {
                if grad.radial || grad.stops.len() >= 3 {
                    subdivide_triangles_midpoint(&mut pos_buf.vertices, &mut pos_buf.indices, 2);
                }
                let base = lyon_buf.vertices.len() as u32;
                for p in &pos_buf.vertices {
                    let col = grad.sample_linear_rgba(p[0], p[1]);
                    lyon_buf
                        .vertices
                        .push(GpuVertex::new(*p, col, [PRIM_FLAG_SHAPE, 0.0]));
                }
                for &idx in &pos_buf.indices {
                    lyon_buf.indices.push(base + idx);
                }
            }
        } else if let Some(fc) = fill_rgba {
            let _ = fill_tess.tessellate_path(
                &fill_path,
                &FillOptions::tolerance(tol).with_fill_rule(FillRule::EvenOdd),
                &mut BuffersBuilder::new(lyon_buf, |vertex: FillVertex| {
                    let p = vertex.position();
                    GpuVertex::new([p.x, p.y], fc, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    }

    if !geom.no_line && line_pattern != 0 {
        if let Some(sc) = stroke_rgba {
            if line_pattern >= 2 {
                let dashed = dash_lyon_path(&stroke_path, line_pattern, stroke_w, tol);
                let is_dotted = matches!(line_pattern, 3 | 10 | 17 | 23);
                let cap = if is_dotted {
                    LineCap::Round
                } else {
                    LineCap::Butt
                };
                let _ = stroke_tess.tessellate_path(
                    &dashed,
                    &StrokeOptions::tolerance(tol)
                        .with_line_width(stroke_w)
                        .with_line_cap(cap)
                        .with_line_join(LineJoin::Round),
                    &mut BuffersBuilder::new(lyon_buf, |vertex: StrokeVertex| {
                        let p = vertex.position();
                        GpuVertex::new([p.x, p.y], sc, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            } else {
                let _ = stroke_tess.tessellate_path(
                    &stroke_path,
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
}

fn dash_lyon_path(path: &Path, line_pattern: u8, stroke_w: f32, tol: f32) -> Path {
    let sw = stroke_w.max(1.0 / 96.0);
    let pat_buf: [f32; 6];
    let pattern: &[f32] = match line_pattern {
        2 | 7 | 9 | 16 => {
            pat_buf = [sw * 4.5, sw * 2.8, 0.0, 0.0, 0.0, 0.0];
            &pat_buf[..2]
        }
        3 | 10 | 17 | 23 => {
            pat_buf = [sw * 0.65, sw * 2.35, 0.0, 0.0, 0.0, 0.0];
            &pat_buf[..2]
        }
        4 | 11 | 18 => {
            pat_buf = [sw * 4.5, sw * 2.2, sw * 0.75, sw * 2.2, 0.0, 0.0];
            &pat_buf[..4]
        }
        5 | 12 | 19 => {
            pat_buf = [
                sw * 4.5,
                sw * 2.0,
                sw * 0.75,
                sw * 2.0,
                sw * 0.75,
                sw * 2.0,
            ];
            &pat_buf[..6]
        }
        _ => {
            pat_buf = [sw * 3.8, sw * 2.4, 0.0, 0.0, 0.0, 0.0];
            &pat_buf[..2]
        }
    };

    let mut out = Path::builder();
    let mut pat_idx = 0usize;
    let mut rem = pattern[0];
    let mut draw_on = true;
    let mut sub_open = false;

    let step_segment = |from: lyon_path::math::Point,
                        to: lyon_path::math::Point,
                        out: &mut lyon_path::path::Builder,
                        pat_idx: &mut usize,
                        rem: &mut f32,
                        draw_on: &mut bool,
                        sub_open: &mut bool| {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let seg_len = (dx * dx + dy * dy).sqrt();
        if seg_len <= 1e-6 || !seg_len.is_finite() {
            return;
        }
        let mut consumed = 0.0f32;
        while consumed < seg_len - 1e-6 {
            let take = (*rem).min(seg_len - consumed);
            let t0 = consumed / seg_len;
            let t1 = (consumed + take) / seg_len;
            let p0 = point(from.x + dx * t0, from.y + dy * t0);
            let p1 = point(from.x + dx * t1, from.y + dy * t1);
            if *draw_on {
                if !*sub_open {
                    out.begin(p0);
                    *sub_open = true;
                }
                out.line_to(p1);
            }
            consumed += take;
            *rem -= take;
            if *rem <= 1e-6 {
                if *draw_on && *sub_open {
                    out.end(false);
                    *sub_open = false;
                }
                *pat_idx = (*pat_idx + 1) % pattern.len();
                *rem = pattern[*pat_idx];
                *draw_on = (*pat_idx % 2) == 0;
            }
        }
    };

    for evt in path.iter().flattened(tol.max(0.001)) {
        match evt {
            Event::Begin { .. } => {
                if sub_open {
                    out.end(false);
                    sub_open = false;
                }
                pat_idx = 0;
                rem = pattern[0];
                draw_on = true;
            }
            Event::Line { from, to } => {
                step_segment(
                    from,
                    to,
                    &mut out,
                    &mut pat_idx,
                    &mut rem,
                    &mut draw_on,
                    &mut sub_open,
                );
            }
            Event::End { first, last, close } => {
                if close {
                    step_segment(
                        last,
                        first,
                        &mut out,
                        &mut pat_idx,
                        &mut rem,
                        &mut draw_on,
                        &mut sub_open,
                    );
                }
                if sub_open {
                    out.end(false);
                    sub_open = false;
                }
            }
            Event::Quadratic { .. } | Event::Cubic { .. } => {}
        }
    }
    if sub_open {
        out.end(false);
    }
    out.build()
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

fn build_lyon_paths_from_verbs(
    verbs: &[EvaluatedPathVerb],
    rounding_in: f64,
    first_tangent: &mut Option<([f64; 2], [f64; 2])>,
    last_tangent: &mut Option<([f64; 2], [f64; 2])>,
) -> (Path, Path) {
    let mut fill_builder = Path::builder();
    let mut stroke_builder = Path::builder();
    let mut cur_start: Option<[f64; 2]> = None;
    let mut cur_segs: Vec<SubpathSeg> = Vec::new();

    let flush_subpath = |fill_b: &mut lyon_path::path::Builder,
                         stroke_b: &mut lyon_path::path::Builder,
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
        let last_end = segs.last().map(SubpathSeg::end_pt).unwrap_or(start);
        let closes_at_start = explicit_close || dist_2d(start, last_end) < 1e-5;
        emit_subpath_into_builder(
            stroke_b,
            start,
            segs,
            explicit_close,
            rounding_in,
            first_tan,
            last_tan,
        );
        if closes_at_start {
            let mut dummy_first = None;
            let mut dummy_last = None;
            emit_subpath_into_builder(
                fill_b,
                start,
                segs,
                explicit_close,
                rounding_in,
                &mut dummy_first,
                &mut dummy_last,
            );
        }
        segs.clear();
    };

    for verb in verbs {
        match *verb {
            EvaluatedPathVerb::MoveTo(pt) => {
                if !is_finite_pt(pt) {
                    continue;
                }
                flush_subpath(
                    &mut fill_builder,
                    &mut stroke_builder,
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
                    &mut fill_builder,
                    &mut stroke_builder,
                    &mut cur_start,
                    &mut cur_segs,
                    false,
                    first_tangent,
                    last_tangent,
                );
                if is_finite_pt(center) && rx.is_finite() && ry.is_finite() && angle_rad.is_finite()
                {
                    emit_rotated_ellipse(
                        &mut stroke_builder,
                        center,
                        rx.abs(),
                        ry.abs(),
                        angle_rad,
                    );
                    emit_rotated_ellipse(
                        &mut fill_builder,
                        center,
                        rx.abs(),
                        ry.abs(),
                        angle_rad,
                    );
                }
            }
            EvaluatedPathVerb::Close => {
                flush_subpath(
                    &mut fill_builder,
                    &mut stroke_builder,
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
        &mut fill_builder,
        &mut stroke_builder,
        &mut cur_start,
        &mut cur_segs,
        false,
        first_tangent,
        last_tangent,
    );

    (fill_builder.build(), stroke_builder.build())
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

/// Descriptor entry for Visio's canonical 46-entry `BeginArrow` / `EndArrow` catalog (`0..=45`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisioArrowheadInfo {
    pub code: u8,
    pub name: &'static str,
    pub category: &'static str,
}

/// Complete catalog of all 46 canonical Visio arrowhead styles (`0..=45`).
pub fn list_visio_arrowheads() -> Vec<VisioArrowheadInfo> {
    vec![
        VisioArrowheadInfo { code: 0, name: "None", category: "Standard" },
        VisioArrowheadInfo { code: 1, name: "Open Arrow (60°)", category: "Standard" },
        VisioArrowheadInfo { code: 2, name: "Hollow Triangle", category: "UML / Standard" },
        VisioArrowheadInfo { code: 3, name: "Wide Open Arrow (90°)", category: "Standard" },
        VisioArrowheadInfo { code: 4, name: "Filled Triangle", category: "Standard" },
        VisioArrowheadInfo { code: 5, name: "Filled Stealth", category: "Standard" },
        VisioArrowheadInfo { code: 6, name: "Concave Base Arrow", category: "Standard" },
        VisioArrowheadInfo { code: 7, name: "Convex Base Arrow", category: "Standard" },
        VisioArrowheadInfo { code: 8, name: "Swallowtail Open", category: "Standard" },
        VisioArrowheadInfo { code: 9, name: "Oblique Tick (45°)", category: "Dimension" },
        VisioArrowheadInfo { code: 10, name: "Filled Dot", category: "Marker" },
        VisioArrowheadInfo { code: 11, name: "Filled Diamond (Composition)", category: "UML / ERD" },
        VisioArrowheadInfo { code: 12, name: "Bold Filled Triangle", category: "Standard" },
        VisioArrowheadInfo { code: 13, name: "Acute Filled Dart (30°)", category: "Standard" },
        VisioArrowheadInfo { code: 14, name: "Hollow Stealth", category: "UML / Standard" },
        VisioArrowheadInfo { code: 15, name: "Hollow Acute Triangle", category: "UML / Standard" },
        VisioArrowheadInfo { code: 16, name: "Hollow Concave Arrow", category: "UML / Standard" },
        VisioArrowheadInfo { code: 17, name: "Hollow Dart", category: "UML / Standard" },
        VisioArrowheadInfo { code: 18, name: "Hollow Wide Triangle", category: "UML / Standard" },
        VisioArrowheadInfo { code: 19, name: "Acute Open Arrow (30°)", category: "Standard" },
        VisioArrowheadInfo { code: 20, name: "Centered Filled Circle", category: "Marker" },
        VisioArrowheadInfo { code: 21, name: "Filled Square", category: "Marker" },
        VisioArrowheadInfo { code: 22, name: "Hollow Diamond (Aggregation)", category: "UML / ERD" },
        VisioArrowheadInfo { code: 23, name: "Perpendicular Stop Bar (|)", category: "Dimension / ERD" },
        VisioArrowheadInfo { code: 24, name: "Dimension Arrow + Bar (>|)", category: "Dimension" },
        VisioArrowheadInfo { code: 25, name: "Double Stop Bar (|| — One Only)", category: "ERD" },
        VisioArrowheadInfo { code: 26, name: "Trident Fork", category: "Engineering" },
        VisioArrowheadInfo { code: 27, name: "Crow's Foot — Many (<)", category: "ERD" },
        VisioArrowheadInfo { code: 28, name: "Crow's Foot — One & Many (|<)", category: "ERD" },
        VisioArrowheadInfo { code: 29, name: "Crow's Foot — Zero or Many (o<)", category: "ERD" },
        VisioArrowheadInfo { code: 30, name: "ERD Zero or One (o|)", category: "ERD" },
        VisioArrowheadInfo { code: 31, name: "Hollow Circle (Message Start)", category: "BPMN / Marker" },
        VisioArrowheadInfo { code: 32, name: "Hollow Circle + Tangent Bar", category: "ERD / BPMN" },
        VisioArrowheadInfo { code: 33, name: "Half Harpoon (Upper Barb)", category: "Engineering" },
        VisioArrowheadInfo { code: 34, name: "Half Harpoon (Lower Barb)", category: "Engineering" },
        VisioArrowheadInfo { code: 35, name: "Double Open Chevron (>>)", category: "Standard" },
        VisioArrowheadInfo { code: 36, name: "Double Filled Triangle", category: "Standard" },
        VisioArrowheadInfo { code: 37, name: "Double Hollow Triangle", category: "UML / Standard" },
        VisioArrowheadInfo { code: 38, name: "Hollow Square", category: "Marker" },
        VisioArrowheadInfo { code: 39, name: "Filled Triangle + Bar", category: "Dimension" },
        VisioArrowheadInfo { code: 40, name: "Hollow Triangle + Bar", category: "Dimension" },
        VisioArrowheadInfo { code: 41, name: "Circle with Cross", category: "BPMN / Engineering" },
        VisioArrowheadInfo { code: 42, name: "Diamond with Cross", category: "BPMN / Engineering" },
        VisioArrowheadInfo { code: 43, name: "Reverse Inward Filled Triangle", category: "Engineering" },
        VisioArrowheadInfo { code: 44, name: "Reverse Inward Open Chevron", category: "Engineering" },
        VisioArrowheadInfo { code: 45, name: "Fletched Triple Feather", category: "Engineering" },
    ]
}

/// Scale multiplier for Visio `BeginArrowSize` / `EndArrowSize` (`0=VerySmall .. 6=Colossal`).
#[inline]
pub fn visio_arrow_size_scale(size_code: u8) -> f64 {
    match size_code {
        0 => 0.60, // Very Small
        1 => 0.78, // Small
        2 => 1.00, // Medium (Visio Default)
        3 => 1.32, // Large
        4 => 1.68, // Extra Large
        5 => 2.15, // Jumbo
        _ => 2.75, // Colossal
    }
}

#[allow(clippy::too_many_arguments)]
fn tessellate_arrowhead(
    arrow_code: u8,
    size_code: u8,
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

    let size_mul = visio_arrow_size_scale(size_code);
    let arrow_len = (((stroke_w as f64) * 6.5).clamp(0.09, 0.32) * size_mul).clamp(0.055, 0.90);
    let arrow_half_w = arrow_len * 0.52;
    let white = [1.0f32, 1.0, 1.0, 1.0];

    let fill_poly = |pts: &[[f64; 2]],
                     col: [f32; 4],
                     ft: &mut FillTessellator,
                     buf: &mut VertexBuffers<GpuVertex, u32>| {
        if pts.len() < 3 {
            return;
        }
        let mut b = Path::builder();
        b.begin(point(pts[0][0] as f32, pts[0][1] as f32));
        for p in pts.iter().skip(1) {
            b.line_to(point(p[0] as f32, p[1] as f32));
        }
        b.end(true);
        let built = b.build();
        let _ = ft.tessellate_path(
            &built,
            &FillOptions::tolerance(tol),
            &mut BuffersBuilder::new(buf, |v: FillVertex| {
                let pt = v.position();
                GpuVertex::new([pt.x, pt.y], col, [PRIM_FLAG_SHAPE, 0.0])
            }),
        );
    };

    let stroke_poly = |pts: &[[f64; 2]],
                       closed: bool,
                       col: [f32; 4],
                       st: &mut StrokeTessellator,
                       buf: &mut VertexBuffers<GpuVertex, u32>| {
        if pts.len() < 2 {
            return;
        }
        let mut b = Path::builder();
        b.begin(point(pts[0][0] as f32, pts[0][1] as f32));
        for p in pts.iter().skip(1) {
            b.line_to(point(p[0] as f32, p[1] as f32));
        }
        b.end(closed);
        let built = b.build();
        let _ = st.tessellate_path(
            &built,
            &StrokeOptions::tolerance(tol)
                .with_line_width(stroke_w)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Miter),
            &mut BuffersBuilder::new(buf, |v: StrokeVertex| {
                let pt = v.position();
                GpuVertex::new([pt.x, pt.y], col, [PRIM_FLAG_SHAPE, 0.0])
            }),
        );
    };

    let emit_circle = |center: [f64; 2],
                       r: f64,
                       hollow: bool,
                       ft: &mut FillTessellator,
                       st: &mut StrokeTessellator,
                       buf: &mut VertexBuffers<GpuVertex, u32>| {
        let mut b = Path::builder();
        emit_rotated_ellipse(&mut b, center, r, r, 0.0);
        let p = b.build();
        let fc = if hollow { white } else { color };
        let _ = ft.tessellate_path(
            &p,
            &FillOptions::tolerance(tol),
            &mut BuffersBuilder::new(buf, |v: FillVertex| {
                let pt = v.position();
                GpuVertex::new([pt.x, pt.y], fc, [PRIM_FLAG_SHAPE, 0.0])
            }),
        );
        if hollow {
            let _ = st.tessellate_path(
                &p,
                &StrokeOptions::tolerance(tol).with_line_width(stroke_w),
                &mut BuffersBuilder::new(buf, |v: StrokeVertex| {
                    let pt = v.position();
                    GpuVertex::new([pt.x, pt.y], color, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    };

    let emit_perp_bar = |dist_back: f64,
                         half_span: f64,
                         st: &mut StrokeTessellator,
                         buf: &mut VertexBuffers<GpuVertex, u32>| {
        let c = [tip_pt[0] - ux * dist_back, tip_pt[1] - uy * dist_back];
        let p0 = [c[0] + nx * half_span, c[1] + ny * half_span];
        let p1 = [c[0] - nx * half_span, c[1] - ny * half_span];
        stroke_poly(&[p0, p1], false, color, st, buf);
    };

    match arrow_code {
        0 => {}
        1 | 3 | 19 => {
            // Open V-chevron arrowheads (`1`=60°, `3`=90° wide, `19`=30° acute).
            let hw = match arrow_code {
                3 => arrow_len * 0.85,
                19 => arrow_len * 0.34,
                _ => arrow_half_w,
            };
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let left = [bx + nx * hw, by + ny * hw];
            let right = [bx - nx * hw, by - ny * hw];
            stroke_poly(&[left, tip_pt, right], false, color, stroke_tess, lyon_buf);
        }
        2 | 14 | 15 | 16 | 17 | 18 => {
            // Hollow triangle / stealth / concave / acute / wide family (`2`, `14..=18`).
            let hw = match arrow_code {
                15 | 17 => arrow_len * 0.38,
                18 => arrow_len * 0.78,
                _ => arrow_half_w,
            };
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let left = [bx + nx * hw, by + ny * hw];
            let right = [bx - nx * hw, by - ny * hw];
            if arrow_code == 14 || arrow_code == 16 {
                let depth_mul = if arrow_code == 16 { 0.70 } else { 0.78 };
                let notch = [
                    tip_pt[0] - ux * (arrow_len * depth_mul),
                    tip_pt[1] - uy * (arrow_len * depth_mul),
                ];
                let poly = [tip_pt, left, notch, right];
                fill_poly(&poly, white, fill_tess, lyon_buf);
                stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
            } else {
                let poly = [tip_pt, left, right];
                fill_poly(&poly, white, fill_tess, lyon_buf);
                stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
            }
        }
        4 | 5 | 6 | 7 | 12 | 13 => {
            // Filled triangle / stealth / concave / convex / bold / acute dart (`4..=7`, `12`, `13`).
            let hw = match arrow_code {
                12 => arrow_len * 0.72,
                13 => arrow_len * 0.35,
                _ => arrow_half_w,
            };
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let left = [bx + nx * hw, by + ny * hw];
            let right = [bx - nx * hw, by - ny * hw];
            let mid_depth = match arrow_code {
                5 => 0.76,  // Stealth inward notch
                6 => 0.68,  // Deep concave inward notch
                7 => 1.18,  // Convex outward tail
                _ => 1.00,  // Flat base triangle
            };
            let mid = [
                tip_pt[0] - ux * (arrow_len * mid_depth),
                tip_pt[1] - uy * (arrow_len * mid_depth),
            ];
            fill_poly(&[tip_pt, left, mid, right], color, fill_tess, lyon_buf);
        }
        8 => {
            // Swallowtail / quill open arrowhead.
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let notch = [tip_pt[0] - ux * (arrow_len * 0.62), tip_pt[1] - uy * (arrow_len * 0.62)];
            let left = [bx + nx * arrow_half_w, by + ny * arrow_half_w];
            let right = [bx - nx * arrow_half_w, by - ny * arrow_half_w];
            stroke_poly(&[left, tip_pt, right, notch, left], false, color, stroke_tess, lyon_buf);
        }
        9 => {
            // Architectural 45° oblique slash tick.
            let d = arrow_half_w * 1.05;
            let p0 = [tip_pt[0] - (ux - nx) * d, tip_pt[1] - (uy - ny) * d];
            let p1 = [tip_pt[0] + (ux - nx) * d, tip_pt[1] + (uy - ny) * d];
            stroke_poly(&[p0, p1], false, color, stroke_tess, lyon_buf);
        }
        10 | 20 => {
            // Filled circular dot marker.
            let c = if arrow_code == 10 {
                [tip_pt[0] - ux * (arrow_len * 0.35), tip_pt[1] - uy * (arrow_len * 0.35)]
            } else {
                tip_pt
            };
            emit_circle(c, arrow_len * 0.40, false, fill_tess, stroke_tess, lyon_buf);
        }
        11 | 22 => {
            // Diamond (`11` = Filled Composition, `22` = Hollow Aggregation).
            let mid = [tip_pt[0] - ux * (arrow_len * 0.62), tip_pt[1] - uy * (arrow_len * 0.62)];
            let tail = [tip_pt[0] - ux * (arrow_len * 1.24), tip_pt[1] - uy * (arrow_len * 1.24)];
            let left = [mid[0] + nx * arrow_half_w, mid[1] + ny * arrow_half_w];
            let right = [mid[0] - nx * arrow_half_w, mid[1] - ny * arrow_half_w];
            let poly = [tip_pt, left, tail, right];
            if arrow_code == 22 {
                fill_poly(&poly, white, fill_tess, lyon_buf);
                stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
            } else {
                fill_poly(&poly, color, fill_tess, lyon_buf);
            }
        }
        21 | 38 => {
            // Square marker (`21` = Filled, `38` = Hollow).
            let r = arrow_len * 0.42;
            let c = [tip_pt[0] - ux * r, tip_pt[1] - uy * r];
            let poly = [
                [c[0] + ux * r + nx * r, c[1] + uy * r + ny * r],
                [c[0] - ux * r + nx * r, c[1] - uy * r + ny * r],
                [c[0] - ux * r - nx * r, c[1] - uy * r - ny * r],
                [c[0] + ux * r - nx * r, c[1] + uy * r - ny * r],
            ];
            if arrow_code == 38 {
                fill_poly(&poly, white, fill_tess, lyon_buf);
                stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
            } else {
                fill_poly(&poly, color, fill_tess, lyon_buf);
            }
        }
        23 => {
            // Perpendicular stop bar (`|`).
            emit_perp_bar(0.0, arrow_half_w * 1.05, stroke_tess, lyon_buf);
        }
        24 => {
            // Open dimension arrow + perpendicular stop bar (`>|`).
            emit_perp_bar(0.0, arrow_half_w * 1.05, stroke_tess, lyon_buf);
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let left = [bx + nx * arrow_half_w, by + ny * arrow_half_w];
            let right = [bx - nx * arrow_half_w, by - ny * arrow_half_w];
            stroke_poly(&[left, tip_pt, right], false, color, stroke_tess, lyon_buf);
        }
        25 => {
            // ERD One-and-Only-One double perpendicular bar (`||`).
            emit_perp_bar(arrow_len * 0.30, arrow_half_w * 0.90, stroke_tess, lyon_buf);
            emit_perp_bar(arrow_len * 0.72, arrow_half_w * 0.90, stroke_tess, lyon_buf);
        }
        26 => {
            // Three-pronged trident fork.
            let neck = [tip_pt[0] - ux * arrow_len, tip_pt[1] - uy * arrow_len];
            let top_l = [tip_pt[0] + nx * arrow_half_w, tip_pt[1] + ny * arrow_half_w];
            let top_r = [tip_pt[0] - nx * arrow_half_w, tip_pt[1] - ny * arrow_half_w];
            let mid_l = [neck[0] + nx * arrow_half_w, neck[1] + ny * arrow_half_w];
            let mid_r = [neck[0] - nx * arrow_half_w, neck[1] - ny * arrow_half_w];
            stroke_poly(&[top_l, mid_l, mid_r, top_r], false, color, stroke_tess, lyon_buf);
        }
        27 | 28 | 29 => {
            // Crow's Foot ERD arrowheads (`27`=Many, `28`=One-and-Many, `29`=Zero-or-Many).
            let neck = [tip_pt[0] - ux * arrow_len, tip_pt[1] - uy * arrow_len];
            let splay_l = [tip_pt[0] + nx * arrow_half_w, tip_pt[1] + ny * arrow_half_w];
            let splay_r = [tip_pt[0] - nx * arrow_half_w, tip_pt[1] - ny * arrow_half_w];
            stroke_poly(&[splay_l, neck, splay_r], false, color, stroke_tess, lyon_buf);
            if arrow_code == 28 {
                emit_perp_bar(arrow_len * 1.15, arrow_half_w * 0.80, stroke_tess, lyon_buf);
            } else if arrow_code == 29 {
                let ring_c = [
                    tip_pt[0] - ux * (arrow_len * 1.34),
                    tip_pt[1] - uy * (arrow_len * 1.34),
                ];
                emit_circle(ring_c, arrow_len * 0.28, true, fill_tess, stroke_tess, lyon_buf);
            }
        }
        30 => {
            // ERD Zero-or-One (`o|`).
            emit_perp_bar(arrow_len * 0.35, arrow_half_w * 0.88, stroke_tess, lyon_buf);
            let ring_c = [tip_pt[0] - ux * (arrow_len * 0.95), tip_pt[1] - uy * (arrow_len * 0.95)];
            emit_circle(ring_c, arrow_len * 0.30, true, fill_tess, stroke_tess, lyon_buf);
        }
        31 => {
            // Hollow circular marker (BPMN Message Start).
            let c = [tip_pt[0] - ux * (arrow_len * 0.40), tip_pt[1] - uy * (arrow_len * 0.40)];
            emit_circle(c, arrow_len * 0.40, true, fill_tess, stroke_tess, lyon_buf);
        }
        32 => {
            // Hollow circle + outer tangent bar.
            emit_perp_bar(0.0, arrow_half_w * 0.95, stroke_tess, lyon_buf);
            let c = [tip_pt[0] - ux * (arrow_len * 0.44), tip_pt[1] - uy * (arrow_len * 0.44)];
            emit_circle(c, arrow_len * 0.38, true, fill_tess, stroke_tess, lyon_buf);
        }
        33 | 34 => {
            // Half harpoon (`33` = upper barb, `34` = lower barb).
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let sign = if arrow_code == 33 { 1.0 } else { -1.0 };
            let barb = [bx + nx * arrow_half_w * sign, by + ny * arrow_half_w * sign];
            fill_poly(&[tip_pt, barb, [bx, by]], color, fill_tess, lyon_buf);
        }
        35 => {
            // Double open chevron (`>>`).
            for k in [0.0, 0.58] {
                let t = [tip_pt[0] - ux * (arrow_len * k), tip_pt[1] - uy * (arrow_len * k)];
                let b_pt = [t[0] - ux * (arrow_len * 0.75), t[1] - uy * (arrow_len * 0.75)];
                let left = [b_pt[0] + nx * (arrow_half_w * 0.85), b_pt[1] + ny * (arrow_half_w * 0.85)];
                let right = [b_pt[0] - nx * (arrow_half_w * 0.85), b_pt[1] - ny * (arrow_half_w * 0.85)];
                stroke_poly(&[left, t, right], false, color, stroke_tess, lyon_buf);
            }
        }
        36 | 37 => {
            // Double triangle (`36` = filled, `37` = hollow).
            for k in [0.0, 0.72] {
                let t = [tip_pt[0] - ux * (arrow_len * k), tip_pt[1] - uy * (arrow_len * k)];
                let b_pt = [t[0] - ux * (arrow_len * 0.74), t[1] - uy * (arrow_len * 0.74)];
                let left = [b_pt[0] + nx * (arrow_half_w * 0.85), b_pt[1] + ny * (arrow_half_w * 0.85)];
                let right = [b_pt[0] - nx * (arrow_half_w * 0.85), b_pt[1] - ny * (arrow_half_w * 0.85)];
                let poly = [t, left, right];
                if arrow_code == 37 {
                    fill_poly(&poly, white, fill_tess, lyon_buf);
                    stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
                } else {
                    fill_poly(&poly, color, fill_tess, lyon_buf);
                }
            }
        }
        39 | 40 => {
            // Triangle + perpendicular stop bar (`39` = filled, `40` = hollow).
            emit_perp_bar(0.0, arrow_half_w * 1.05, stroke_tess, lyon_buf);
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let poly = [
                tip_pt,
                [bx + nx * arrow_half_w, by + ny * arrow_half_w],
                [bx - nx * arrow_half_w, by - ny * arrow_half_w],
            ];
            if arrow_code == 40 {
                fill_poly(&poly, white, fill_tess, lyon_buf);
                stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
            } else {
                fill_poly(&poly, color, fill_tess, lyon_buf);
            }
        }
        41 => {
            // Hollow circle with internal cross (`⊕`).
            let r = arrow_len * 0.44;
            let c = [tip_pt[0] - ux * r, tip_pt[1] - uy * r];
            emit_circle(c, r, true, fill_tess, stroke_tess, lyon_buf);
            let d = r * 0.68;
            stroke_poly(
                &[[c[0] - (ux + nx) * d, c[1] - (uy + ny) * d], [c[0] + (ux + nx) * d, c[1] + (uy + ny) * d]],
                false,
                color,
                stroke_tess,
                lyon_buf,
            );
            stroke_poly(
                &[[c[0] - (ux - nx) * d, c[1] - (uy - ny) * d], [c[0] + (ux - nx) * d, c[1] + (uy - ny) * d]],
                false,
                color,
                stroke_tess,
                lyon_buf,
            );
        }
        42 => {
            // Hollow diamond with internal cross.
            let mid = [tip_pt[0] - ux * (arrow_len * 0.62), tip_pt[1] - uy * (arrow_len * 0.62)];
            let tail = [tip_pt[0] - ux * (arrow_len * 1.24), tip_pt[1] - uy * (arrow_len * 1.24)];
            let left = [mid[0] + nx * arrow_half_w, mid[1] + ny * arrow_half_w];
            let right = [mid[0] - nx * arrow_half_w, mid[1] - ny * arrow_half_w];
            let poly = [tip_pt, left, tail, right];
            fill_poly(&poly, white, fill_tess, lyon_buf);
            stroke_poly(&poly, true, color, stroke_tess, lyon_buf);
            stroke_poly(&[left, right], false, color, stroke_tess, lyon_buf);
            stroke_poly(&[tip_pt, tail], false, color, stroke_tess, lyon_buf);
        }
        43 | 44 => {
            // Reverse inward triangle / chevron (`43` = filled, `44` = open).
            let apex = [tip_pt[0] - ux * arrow_len, tip_pt[1] - uy * arrow_len];
            let left = [tip_pt[0] + nx * arrow_half_w, tip_pt[1] + ny * arrow_half_w];
            let right = [tip_pt[0] - nx * arrow_half_w, tip_pt[1] - ny * arrow_half_w];
            if arrow_code == 44 {
                stroke_poly(&[left, apex, right], false, color, stroke_tess, lyon_buf);
            } else {
                fill_poly(&[apex, left, right], color, fill_tess, lyon_buf);
            }
        }
        45 => {
            // Fletched triple feather tail.
            for k in [0.18, 0.52, 0.86] {
                let c = [tip_pt[0] - ux * (arrow_len * k), tip_pt[1] - uy * (arrow_len * k)];
                let back = [c[0] - ux * (arrow_len * 0.36), c[1] - uy * (arrow_len * 0.36)];
                let fl = [back[0] + nx * (arrow_half_w * 0.78), back[1] + ny * (arrow_half_w * 0.78)];
                let fr = [back[0] - nx * (arrow_half_w * 0.78), back[1] - ny * (arrow_half_w * 0.78)];
                stroke_poly(&[fl, c, fr], false, color, stroke_tess, lyon_buf);
            }
        }
        _ => {
            // Fallback filled stealth arrowhead for out-of-range custom codes.
            let bx = tip_pt[0] - ux * arrow_len;
            let by = tip_pt[1] - uy * arrow_len;
            let notch = [tip_pt[0] - ux * (arrow_len * 0.78), tip_pt[1] - uy * (arrow_len * 0.78)];
            let left = [bx + nx * arrow_half_w, by + ny * arrow_half_w];
            let right = [bx - nx * arrow_half_w, by - ny * arrow_half_w];
            fill_poly(&[tip_pt, left, notch, right], color, fill_tess, lyon_buf);
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

/// Extract clean deduplicated polyline vertices in page inches for a `Connector1D` shape.
pub fn extract_connector_polyline_in(visual: &EvaluatedShapeVisual) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for g in &visual.paths {
        if g.no_show {
            continue;
        }
        for v in &g.verbs_page_space {
            let next_pt = match *v {
                EvaluatedPathVerb::MoveTo(p)
                | EvaluatedPathVerb::LineTo(p)
                | EvaluatedPathVerb::ArcTo { to: p, .. }
                | EvaluatedPathVerb::QuadTo { to: p, .. }
                | EvaluatedPathVerb::CubicTo { to: p, .. } => Some(p),
                _ => None,
            };
            if let Some(p) = next_pt {
                if pts.last().is_none_or(|prev| dist_2d(*prev, p) > 1e-4) {
                    pts.push(p);
                }
            }
        }
    }
    pts
}

/// Tessellate Visio 1D Connector selection handles: green `Begin`/`End` glue knobs
/// (solid green when glued, white-center with green ring when unglued) plus cyan
/// interior orthogonal segment midpoint diamonds for manual route dragging!
pub fn tessellate_connector_1d_handles(
    visual: &EvaluatedShapeVisual,
    zoom_ppi: f64,
    begin_glued: bool,
    end_glued: bool,
) -> TessellatedMesh {
    let pts = extract_connector_polyline_in(visual);
    if pts.len() < 2 {
        return TessellatedMesh::new();
    }
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = 1.0 / ppi;
    let stroke_w = (1.6 * inv_ppi) as f32;
    let end_r = 4.6 * inv_ppi;
    let seg_r = (3.8 * inv_ppi) as f32;
    let tol = (0.3 * inv_ppi).clamp(0.0002, 0.02) as f32;

    let glue_green = [0.020, 0.580, 0.215, 1.0]; // Visio glue green (#16A34A in linear RGB)
    let cyan_seg = [0.015, 0.485, 0.850, 1.0];   // Visio segment diamond blue
    let white = [1.0, 1.0, 1.0, 1.0];

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    // Subtle selection highlight along the connector path
    let mut hl_b = Path::builder();
    hl_b.begin(point(pts[0][0] as f32, pts[0][1] as f32));
    for p in pts.iter().skip(1) {
        hl_b.line_to(point(p[0] as f32, p[1] as f32));
    }
    hl_b.end(false);
    let hl_path = hl_b.build();
    let _ = stroke_tess.tessellate_path(
        &hl_path,
        &StrokeOptions::tolerance(tol).with_line_width((1.2 * inv_ppi) as f32),
        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], [0.145, 0.388, 0.922, 0.65], [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );

    // Begin & End circular glue handles
    for (pt, is_glued) in [(pts[0], begin_glued), (*pts.last().unwrap(), end_glued)] {
        let mut b = Path::builder();
        emit_rotated_ellipse(&mut b, pt, end_r, end_r, 0.0);
        let p = b.build();
        let fill_col = if is_glued { glue_green } else { white };
        let ring_col = if is_glued { white } else { glue_green };
        let _ = fill_tess.tessellate_path(
            &p,
            &FillOptions::tolerance(tol),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], fill_col, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
        let _ = stroke_tess.tessellate_path(
            &p,
            &StrokeOptions::tolerance(tol).with_line_width(stroke_w),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], ring_col, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
    }

    let mut mesh = TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    };

    // Orthogonal segment midpoint diamond grips:
    // - All interior segments `1 .. pts.len() - 2` longer than `8 dp`
    // - Endpoint-adjacent segments (`0` and `pts.len() - 2`) on multi-segment polylines (`pts.len() >= 3`)
    //   longer than `28 dp` so dragging an L-bend or outer leg inserts a clean orthogonal dogleg!
    if pts.len() >= 3 {
        let seg_count = pts.len() - 1;
        for seg_idx in 0..seg_count {
            let is_outer = seg_idx == 0 || seg_idx + 1 == seg_count;
            let min_len = if is_outer {
                (28.0 * inv_ppi).max(0.28)
            } else {
                8.0 * inv_ppi
            };
            let a = pts[seg_idx];
            let b = pts[seg_idx + 1];
            let is_horiz = (a[1] - b[1]).abs() < 1e-3 && (a[0] - b[0]).abs() > min_len;
            let is_vert = (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() > min_len;
            if is_horiz || is_vert {
                let mx = ((a[0] + b[0]) * 0.5) as f32;
                let my = ((a[1] + b[1]) * 0.5) as f32;
                let r_out = seg_r + stroke_w;
                mesh.push_quad_points(
                    [
                        [mx, my + r_out],
                        [mx + r_out, my],
                        [mx, my - r_out],
                        [mx - r_out, my],
                    ],
                    cyan_seg,
                    [PRIM_FLAG_OVERLAY_UI, 0.0],
                );
                mesh.push_quad_points(
                    [
                        [mx, my + seg_r],
                        [mx + seg_r, my],
                        [mx, my - seg_r],
                        [mx - seg_r, my],
                    ],
                    white,
                    [PRIM_FLAG_OVERLAY_UI, 0.0],
                );
            }
        }
    }

    mesh
}

/// Tessellate Visio interactive selection frame, 8 resize handles, and top rotation handle
/// in page-space inches scaled consistently for the current `zoom_ppi`.
pub fn tessellate_selection_handles(
    visual: &EvaluatedShapeVisual,
    zoom_ppi: f64,
) -> TessellatedMesh {
    if visual.kind == ShapeKind::Connector1D {
        return tessellate_connector_1d_handles(visual, zoom_ppi, false, false);
    }
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = 1.0 / ppi;
    let stroke_w = (1.6 * inv_ppi) as f32;
    let handle_half = (4.5 * inv_ppi) as f32; // 9 dp crisp resize squares
    let rot_stem_len = 28.0 * inv_ppi; // 28 dp offset above top edge
    let rot_radius = 11.0 * inv_ppi; // 22 dp diameter unmistakable circular Rotate button!

    let accent = [0.145, 0.388, 0.922, 1.0]; // Vexio / Visio selection blue (#2563EB)
    let icon_blue = [0.114, 0.306, 0.847, 1.0]; // High-contrast #1D4ED8 interior ↻ icon
    let shadow_rgba = [0.06, 0.09, 0.16, 0.22];
    let white = [1.0, 1.0, 1.0, 1.0];
    let tol = (0.25 * inv_ppi).clamp(0.00015, 0.015) as f32;

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

    // Upward normal in rotated shape frame for the rotation button center (`up_pt`) and stem end (`stem_end`).
    let up_pt: [f64; 2] = tf
        .local_to_parent(tf.width * 0.5, tf.height + rot_stem_len)
        .into();
    let stem_end: [f64; 2] = tf
        .local_to_parent(tf.width * 0.5, tf.height + (rot_stem_len - rot_radius))
        .into();

    let mut frame_builder = Path::builder();
    frame_builder.begin(point(c00[0] as f32, c00[1] as f32));
    frame_builder.line_to(point(c10[0] as f32, c10[1] as f32));
    frame_builder.line_to(point(c11[0] as f32, c11[1] as f32));
    frame_builder.line_to(point(c01[0] as f32, c01[1] as f32));
    frame_builder.end(true);
    frame_builder.begin(point(top_mid[0] as f32, top_mid[1] as f32));
    frame_builder.line_to(point(stem_end[0] as f32, stem_end[1] as f32));
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

    // Prominent 22 dp circular Rotate button at `up_pt` with subtle drop-shadow, white badge disc,
    // crisp blue border, and interior clockwise `↻` circular arrow + triangular arrowhead!
    let mut knob_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let shadow_off = 1.4 * inv_ppi;
    let mut sb = Path::builder();
    emit_rotated_ellipse(
        &mut sb,
        [up_pt[0] + shadow_off * 0.4, up_pt[1] - shadow_off],
        rot_radius + 0.6 * inv_ppi,
        rot_radius + 0.6 * inv_ppi,
        0.0,
    );
    let shadow_path = sb.build();
    let _ = fill_tess.tessellate_path(
        &shadow_path,
        &FillOptions::tolerance(tol),
        &mut BuffersBuilder::new(&mut knob_buf, |v: FillVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], shadow_rgba, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );

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
        &StrokeOptions::tolerance(tol).with_line_width((1.8 * inv_ppi) as f32),
        &mut BuffersBuilder::new(&mut knob_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], accent, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );

    // Interior clockwise `↻` arc (`225°` -> `-50°`, sweeping 275° clockwise in `+Y`-up page space)
    let arc_r = 5.4 * inv_ppi;
    let start_deg = 225.0_f64;
    let end_deg = -50.0_f64;
    let arc_steps = 20usize;
    let mut ab = Path::builder();
    for step in 0..=arc_steps {
        let t = (step as f64) / (arc_steps as f64);
        let deg = start_deg + (end_deg - start_deg) * t;
        let rad = deg.to_radians();
        let px = (up_pt[0] + arc_r * rad.cos()) as f32;
        let py = (up_pt[1] + arc_r * rad.sin()) as f32;
        if step == 0 {
            ab.begin(point(px, py));
        } else {
            ab.line_to(point(px, py));
        }
    }
    ab.end(false);
    let arc_path = ab.build();
    let _ = stroke_tess.tessellate_path(
        &arc_path,
        &StrokeOptions::tolerance(tol)
            .with_line_width((1.85 * inv_ppi) as f32)
            .with_line_cap(LineCap::Round),
        &mut BuffersBuilder::new(&mut knob_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], icon_blue, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );

    // Directional triangular arrowhead at the clockwise end of the `↻` arc (`end_deg`)
    let end_rad = end_deg.to_radians();
    let end_pt = [
        up_pt[0] + arc_r * end_rad.cos(),
        up_pt[1] + arc_r * end_rad.sin(),
    ];
    // Clockwise unit tangent at `end_rad` is `(sin(theta), -cos(theta))`; outward normal is `(cos(theta), sin(theta))`
    let tx = end_rad.sin();
    let ty = -end_rad.cos();
    let nx = end_rad.cos();
    let ny = end_rad.sin();
    let head_fwd = 3.2 * inv_ppi;
    let head_back = 1.6 * inv_ppi;
    let head_wing = 3.2 * inv_ppi;
    let tip = [end_pt[0] + tx * head_fwd, end_pt[1] + ty * head_fwd];
    let wing_l = [
        end_pt[0] - tx * head_back + nx * head_wing,
        end_pt[1] - ty * head_back + ny * head_wing,
    ];
    let wing_r = [
        end_pt[0] - tx * head_back - nx * head_wing,
        end_pt[1] - ty * head_back - ny * head_wing,
    ];
    let mut hb = Path::builder();
    hb.begin(point(tip[0] as f32, tip[1] as f32));
    hb.line_to(point(wing_l[0] as f32, wing_l[1] as f32));
    hb.line_to(point(wing_r[0] as f32, wing_r[1] as f32));
    hb.end(true);
    let head_path = hb.build();
    let _ = fill_tess.tessellate_path(
        &head_path,
        &FillOptions::tolerance(tol),
        &mut BuffersBuilder::new(&mut knob_buf, |v: FillVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], icon_blue, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );

    mesh.append(&TessellatedMesh {
        vertices: knob_buf.vertices,
        indices: knob_buf.indices,
    });

    // Yellow diamond `Controls.*` handles (`#FACC15` gold fill + `#B45309` amber border + drop shadow).
    if !visual.control_handles.is_empty() {
        let ctrl_r = (6.2 * inv_ppi) as f32;
        let ctrl_outer = ctrl_r + (1.7 * inv_ppi) as f32;
        let sh_off = (1.1 * inv_ppi) as f32;
        let ctrl_shadow = [0.06, 0.09, 0.16, 0.24];
        let ctrl_border = [0.706, 0.325, 0.035, 1.0]; // #B45309 rich amber border
        let ctrl_fill = [0.980, 0.800, 0.082, 1.0]; // #FACC15 unmistakable Visio yellow diamond
        for ch in &visual.control_handles {
            let cx = ch.pos_page_in[0] as f32;
            let cy = ch.pos_page_in[1] as f32;
            mesh.push_quad_points(
                [
                    [cx + sh_off * 0.4, cy - sh_off + ctrl_outer],
                    [cx + sh_off * 0.4 + ctrl_outer, cy - sh_off],
                    [cx + sh_off * 0.4, cy - sh_off - ctrl_outer],
                    [cx + sh_off * 0.4 - ctrl_outer, cy - sh_off],
                ],
                ctrl_shadow,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
            mesh.push_quad_points(
                [
                    [cx, cy + ctrl_outer],
                    [cx + ctrl_outer, cy],
                    [cx, cy - ctrl_outer],
                    [cx - ctrl_outer, cy],
                ],
                ctrl_border,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
            mesh.push_quad_points(
                [
                    [cx, cy + ctrl_r],
                    [cx + ctrl_r, cy],
                    [cx, cy - ctrl_r],
                    [cx - ctrl_r, cy],
                ],
                ctrl_fill,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
        }
    }

    mesh
}

/// Cardinal Visio AutoConnect quick-stamp & link directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AutoConnectDir {
    North,
    East,
    South,
    West,
}

impl AutoConnectDir {
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    pub fn from_slug(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "n" | "north" | "up" | "top" => Some(Self::North),
            "e" | "east" | "right" => Some(Self::East),
            "s" | "south" | "down" | "bottom" => Some(Self::South),
            "w" | "west" | "left" => Some(Self::West),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::North => "north",
            Self::East => "east",
            Self::South => "south",
            Self::West => "west",
        }
    }
}

/// Compute the world/page-space center `(x_in, y_in)` and unit outward direction `(ux, uy)`
/// of a Visio AutoConnect triangle for `tf` at `zoom_ppi`.
/// `North` sits cleanly above the `28 dp` circular `↻` Rotate button (`50 dp` above top edge);
/// `East`, `South`, and `West` sit `22 dp` outside their respective midpoint resize handles.
pub fn autoconnect_arrow_center_in(
    tf: &vexio_shapesheet::ShapeTransform2D,
    dir: AutoConnectDir,
    zoom_ppi: f64,
) -> ([f64; 2], [f64; 2]) {
    let ppi = zoom_ppi.max(1.0);
    let side_off = 22.0 / ppi;
    let north_off = 50.0 / ppi;
    let (lx, ly, edge_lx, edge_ly) = match dir {
        AutoConnectDir::North => (tf.width * 0.5, tf.height + north_off, tf.width * 0.5, tf.height),
        AutoConnectDir::East => (tf.width + side_off, tf.height * 0.5, tf.width, tf.height * 0.5),
        AutoConnectDir::South => (tf.width * 0.5, -side_off, tf.width * 0.5, 0.0),
        AutoConnectDir::West => (-side_off, tf.height * 0.5, 0.0, tf.height * 0.5),
    };
    let center: [f64; 2] = tf.local_to_parent(lx, ly).into();
    let edge: [f64; 2] = tf.local_to_parent(edge_lx, edge_ly).into();
    let dx = center[0] - edge[0];
    let dy = center[1] - edge[1];
    let len = dx.hypot(dy);
    let unit = if len > 1e-9 {
        [dx / len, dy / len]
    } else {
        match dir {
            AutoConnectDir::North => [0.0, 1.0],
            AutoConnectDir::East => [1.0, 0.0],
            AutoConnectDir::South => [0.0, -1.0],
            AutoConnectDir::West => [-1.0, 0.0],
        }
    };
    (center, unit)
}

/// Tessellate the 4 outward-pointing Visio AutoConnect directional triangles around a selected 2D shape.
pub fn tessellate_autoconnect_triangles(
    visual: &EvaluatedShapeVisual,
    zoom_ppi: f64,
    hovered_dir: Option<AutoConnectDir>,
) -> TessellatedMesh {
    if !matches!(
        visual.kind,
        ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
    ) || visual.is_swimlane
    {
        return TessellatedMesh::new();
    }
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = 1.0 / ppi;
    let tol = (0.25 * inv_ppi).clamp(0.00015, 0.015) as f32;
    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    for dir in AutoConnectDir::ALL {
        let is_hovered = hovered_dir == Some(dir);
        let (c, u) = autoconnect_arrow_center_in(&visual.transform, dir, ppi);
        let scale = if is_hovered { 1.22 } else { 1.0 };
        let fwd = 5.6 * scale * inv_ppi;
        let back = 4.2 * scale * inv_ppi;
        let wing = 5.4 * scale * inv_ppi;
        let nx = -u[1];
        let ny = u[0];

        let tip = [c[0] + u[0] * fwd, c[1] + u[1] * fwd];
        let wl = [c[0] - u[0] * back + nx * wing, c[1] - u[1] * back + ny * wing];
        let wr = [c[0] - u[0] * back - nx * wing, c[1] - u[1] * back - ny * wing];

        let mut tb = Path::builder();
        tb.begin(point(tip[0] as f32, tip[1] as f32));
        tb.line_to(point(wl[0] as f32, wl[1] as f32));
        tb.line_to(point(wr[0] as f32, wr[1] as f32));
        tb.end(true);
        let tri_path = tb.build();

        let fill_col = if is_hovered {
            [0.145, 0.388, 0.922, 0.98] // Solid royal blue when hovered
        } else {
            [0.859, 0.918, 0.996, 0.94] // Crisp #DBEAFE Visio AutoConnect light blue
        };
        let border_col = [0.114, 0.306, 0.847, 1.0]; // #1D4ED8 border

        let _ = fill_tess.tessellate_path(
            &tri_path,
            &FillOptions::tolerance(tol),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                let p = v.position();
                GpuVertex::new([p.x, p.y], fill_col, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
        let _ = stroke_tess.tessellate_path(
            &tri_path,
            &StrokeOptions::tolerance(tol).with_line_width((1.35 * inv_ppi) as f32),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let p = v.position();
                GpuVertex::new([p.x, p.y], border_col, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
    }

    TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    }
}

/// Tessellate a dashed Visio multi-selection group bounding frame + 8 outer resize grips.
pub fn tessellate_multi_selection_frame(
    aabb_min: [f64; 2],
    aabb_max: [f64; 2],
    zoom_ppi: f64,
) -> TessellatedMesh {
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = (1.0 / ppi) as f32;
    let stroke_w = 1.4 * inv_ppi;
    let handle_half = 3.6 * inv_ppi;
    let accent = [0.055, 0.450, 0.910, 0.92];
    let white = [1.0, 1.0, 1.0, 1.0];

    let mut fb = Path::builder();
    let x0 = aabb_min[0] as f32;
    let y0 = aabb_min[1] as f32;
    let x1 = aabb_max[0] as f32;
    let y1 = aabb_max[1] as f32;
    fb.begin(point(x0, y0));
    fb.line_to(point(x1, y0));
    fb.line_to(point(x1, y1));
    fb.line_to(point(x0, y1));
    fb.end(true);
    let dashed = dash_lyon_path(&fb.build(), 2, stroke_w, (0.3 * inv_ppi).max(0.0004));

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut stroke_tess = StrokeTessellator::new();
    let _ = stroke_tess.tessellate_path(
        &dashed,
        &StrokeOptions::tolerance((0.3 * inv_ppi).max(0.0004)).with_line_width(stroke_w),
        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
            let p = v.position();
            GpuVertex::new([p.x, p.y], accent, [PRIM_FLAG_OVERLAY_UI, 0.0])
        }),
    );
    let mut mesh = TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    };

    let mx = 0.5 * (x0 + x1);
    let my = 0.5 * (y0 + y1);
    let outer = handle_half + stroke_w;
    for [hx, hy] in [
        [x0, y0],
        [x1, y0],
        [x1, y1],
        [x0, y1],
        [mx, y1],
        [mx, y0],
        [x0, my],
        [x1, my],
    ] {
        mesh.push_quad([hx - outer, hy - outer], [hx + outer, hy + outer], accent, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([hx - handle_half, hy - handle_half], [hx + handle_half, hy + handle_half], white, [PRIM_FLAG_OVERLAY_UI, 0.0]);
    }
    mesh
}

/// Visio Dynamic Grid Smart Guide line (edge/center alignment or equal-gap spacing indicator).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SmartGuideLine {
    /// `true` = vertical line at `x = coord_in` spanning `y in [span_min_in .. span_max_in]`;
    /// `false` = horizontal line at `y = coord_in` spanning `x in [span_min_in .. span_max_in]`.
    pub is_vertical: bool,
    pub coord_in: f64,
    pub span_min_in: f64,
    pub span_max_in: f64,
    /// When `true`, renders perpendicular Visio equal-spacing ticks at both ends of the gap segment.
    pub is_equal_spacing: bool,
}

/// Draggable horizontal or vertical Visio ruler guide (`ShapeKind::Guide`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RulerGuideOverlay {
    pub shape_id: u64,
    pub is_vertical: bool,
    pub coord_in: f64,
    pub span_min_in: f64,
    pub span_max_in: f64,
    pub selected: bool,
}

/// Interactive `Shift+Ctrl+4` Text Block manipulation frame (`TxtPinX/Y`, `TxtWidth/Height`, `TxtAngle`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EditableTextBlockOverlay {
    /// Four corners `[BL, BR, TR, TL]` in page inches.
    pub corners_page_in: [[f64; 2]; 4],
    pub top_mid_page_in: [f64; 2],
    pub rot_knob_page_in: [f64; 2],
}

/// Transient interactive canvas overlays rendered above shapes in `PRIM_FLAG_OVERLAY_UI`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InteractiveCanvasOverlay {
    /// Rubber-band marquee selection rectangle `(min_in, max_in)`.
    pub marquee_rect_in: Option<([f64; 2], [f64; 2])>,
    /// Live Dynamic Grid alignment and equal-spacing guides during shape move/resize.
    pub smart_guides: Vec<SmartGuideLine>,
    /// Connection points on hovered/targeted 2D shapes `(pos_page_in, is_active_snap_target)`.
    pub highlighted_ports: Vec<([f64; 2], bool)>,
    /// Live preview line `(start_in, current_in)` while drawing a new connector.
    pub connector_preview_in: Option<([f64; 2], [f64; 2])>,
    /// Per-connector `(begin_glued, end_glued)` state for selected `Connector1D` shapes.
    pub glued_connector_ends: BTreeMap<u64, (bool, bool)>,
    /// Hovered AutoConnect directional triangle `(shape_id, dir)` in Pointer mode.
    #[serde(default)]
    pub hovered_autoconnect: Option<(u64, AutoConnectDir)>,
    /// Target Container or Swimlane bounding box `(min_in, max_in)` illuminated while dragging shapes into it.
    #[serde(default)]
    pub container_drop_target_in: Option<([f64; 2], [f64; 2])>,
    /// Live stroke points `(x_in, y_in)` while actively drawing Freeform, Ink Pen, or Highlighter paths.
    #[serde(default)]
    pub live_stroke_points: Vec<[f64; 2]>,
    /// Whether `live_stroke_points` uses translucent highlighter styling.
    #[serde(default)]
    pub live_stroke_highlighter: bool,
    /// Editable geometry vertices `(x_in, y_in)` for the active shape in `EditVertices` mode.
    #[serde(default)]
    pub editable_vertices: Vec<[f64; 2]>,
    /// Editable connection point ports `(x_in, y_in)` in `ConnectionPoint` (`Shift+Ctrl+1`) mode.
    #[serde(default)]
    pub editable_connection_points: Vec<[f64; 2]>,
    /// Persistent/draggable Ruler Guides (`ShapeKind::Guide`).
    #[serde(default)]
    pub ruler_guides: Vec<RulerGuideOverlay>,
    /// Independent Text Block manipulation frame (`Shift+Ctrl+4` TextBlock tool).
    #[serde(default)]
    pub editable_text_block: Option<EditableTextBlockOverlay>,
}

/// Tessellate transient interactive overlays (rubber-band marquee, smart alignment & equal-spacing
/// guides, hovered connection point snap rings, and live connector preview).
pub fn tessellate_interactive_overlay(
    overlay: &InteractiveCanvasOverlay,
    zoom_ppi: f64,
) -> TessellatedMesh {
    let ppi = zoom_ppi.max(1.0);
    let inv_ppi = 1.0 / ppi;
    let tol = (0.3 * inv_ppi).clamp(0.0002, 0.02) as f32;
    let mut mesh = TessellatedMesh::new();

    // 0. Container / Swimlane Drop-Target Glow Border when dragging member shapes over it!
    if let Some((mn, mx)) = overlay.container_drop_target_in {
        let pad = (2.0 * inv_ppi) as f32;
        let x0 = (mn[0].min(mx[0]) as f32) - pad;
        let y0 = (mn[1].min(mx[1]) as f32) - pad;
        let x1 = (mn[0].max(mx[0]) as f32) + pad;
        let y1 = (mn[1].max(mx[1]) as f32) + pad;
        let glow_fill = [0.063, 0.725, 0.506, 0.09]; // Soft emerald container highlight
        let outer_glow = [0.063, 0.725, 0.506, 0.32];
        let crisp_ring = [0.020, 0.588, 0.412, 0.96]; // #059669 crisp Visio container target border
        let gw = (4.5 * inv_ppi) as f32;
        let bw = (2.1 * inv_ppi) as f32;
        mesh.push_quad([x0, y0], [x1, y1], glow_fill, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0 - gw, y0 - gw], [x1 + gw, y0 + gw], outer_glow, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0 - gw, y1 - gw], [x1 + gw, y1 + gw], outer_glow, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0 - gw, y0], [x0 + gw, y1], outer_glow, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x1 - gw, y0], [x1 + gw, y1], outer_glow, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0, y0], [x1, y0 + bw], crisp_ring, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0, y1 - bw], [x1, y1], crisp_ring, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0, y0], [x0 + bw, y1], crisp_ring, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x1 - bw, y0], [x1, y1], crisp_ring, [PRIM_FLAG_OVERLAY_UI, 0.0]);
    }

    // 1. Rubber-band Marquee Selection Box
    if let Some((mn, mx)) = overlay.marquee_rect_in {
        let x0 = mn[0].min(mx[0]) as f32;
        let y0 = mn[1].min(mx[1]) as f32;
        let x1 = mn[0].max(mx[0]) as f32;
        let y1 = mn[1].max(mx[1]) as f32;
        let fill = [0.145, 0.388, 0.922, 0.14];
        let border = [0.145, 0.388, 0.922, 0.92];
        let bw = (1.35 * inv_ppi) as f32;
        mesh.push_quad([x0, y0], [x1, y1], fill, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0, y0], [x1, y0 + bw], border, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0, y1 - bw], [x1, y1], border, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x0, y0], [x0 + bw, y1], border, [PRIM_FLAG_OVERLAY_UI, 0.0]);
        mesh.push_quad([x1 - bw, y0], [x1, y1], border, [PRIM_FLAG_OVERLAY_UI, 0.0]);
    }

    // 2. Dynamic Grid Smart Alignment & Equal-Spacing Guides
    let align_col = [0.012, 0.580, 0.250, 0.95]; // Crisp Visio Dynamic Grid green/emerald
    let space_col = [0.850, 0.120, 0.480, 0.95]; // Crisp magenta equal-spacing dimension bar
    let half_lw = (0.85 * inv_ppi) as f32;
    let cap_len = (4.5 * inv_ppi) as f32;

    for g in &overlay.smart_guides {
        let col = if g.is_equal_spacing { space_col } else { align_col };
        let s0 = g.span_min_in.min(g.span_max_in) as f32;
        let s1 = g.span_min_in.max(g.span_max_in) as f32;
        let c = g.coord_in as f32;
        if g.is_vertical {
            mesh.push_quad([c - half_lw, s0], [c + half_lw, s1], col, [PRIM_FLAG_OVERLAY_UI, 0.0]);
            if g.is_equal_spacing {
                mesh.push_quad([c - cap_len, s0 - half_lw], [c + cap_len, s0 + half_lw], col, [PRIM_FLAG_OVERLAY_UI, 0.0]);
                mesh.push_quad([c - cap_len, s1 - half_lw], [c + cap_len, s1 + half_lw], col, [PRIM_FLAG_OVERLAY_UI, 0.0]);
            }
        } else {
            mesh.push_quad([s0, c - half_lw], [s1, c + half_lw], col, [PRIM_FLAG_OVERLAY_UI, 0.0]);
            if g.is_equal_spacing {
                mesh.push_quad([s0 - half_lw, c - cap_len], [s0 + half_lw, c + cap_len], col, [PRIM_FLAG_OVERLAY_UI, 0.0]);
                mesh.push_quad([s1 - half_lw, c - cap_len], [s1 + half_lw, c + cap_len], col, [PRIM_FLAG_OVERLAY_UI, 0.0]);
            }
        }
    }

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    // 3. Live Connector Drag Preview Line
    if let Some((p0, p1)) = overlay.connector_preview_in {
        let mut b = Path::builder();
        b.begin(point(p0[0] as f32, p0[1] as f32));
        b.line_to(point(p1[0] as f32, p1[1] as f32));
        b.end(false);
        let sw = (1.6 * inv_ppi) as f32;
        let dashed = dash_lyon_path(&b.build(), 2, sw, tol);
        let _ = stroke_tess.tessellate_path(
            &dashed,
            &StrokeOptions::tolerance(tol).with_line_width(sw),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let p = v.position();
                GpuVertex::new([p.x, p.y], [0.020, 0.580, 0.215, 0.92], [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
    }

    // 4. Connection Point Snap Targets (`×` markers + glowing green ring on active snap port)
    for &(pt, snapped) in &overlay.highlighted_ports {
        if snapped {
            let r = 6.2 * inv_ppi;
            let mut b = Path::builder();
            emit_rotated_ellipse(&mut b, pt, r, r, 0.0);
            let p = b.build();
            let _ = fill_tess.tessellate_path(
                &p,
                &FillOptions::tolerance(tol),
                &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                    let pos = v.position();
                    GpuVertex::new([pos.x, pos.y], [0.020, 0.580, 0.215, 0.25], [PRIM_FLAG_OVERLAY_UI, 0.0])
                }),
            );
            let _ = stroke_tess.tessellate_path(
                &p,
                &StrokeOptions::tolerance(tol).with_line_width((2.0 * inv_ppi) as f32),
                &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                    let pos = v.position();
                    GpuVertex::new([pos.x, pos.y], [0.020, 0.580, 0.215, 1.0], [PRIM_FLAG_OVERLAY_UI, 0.0])
                }),
            );
        }
        // Crisp connection point `×` cross
        let d = 3.0 * inv_ppi;
        let mut xb = Path::builder();
        xb.begin(point((pt[0] - d) as f32, (pt[1] - d) as f32));
        xb.line_to(point((pt[0] + d) as f32, (pt[1] + d) as f32));
        xb.end(false);
        xb.begin(point((pt[0] - d) as f32, (pt[1] + d) as f32));
        xb.line_to(point((pt[0] + d) as f32, (pt[1] - d) as f32));
        xb.end(false);
        let xp = xb.build();
        let x_col = if snapped {
            [0.020, 0.580, 0.215, 1.0]
        } else {
            [0.145, 0.388, 0.922, 0.88]
        };
        let _ = stroke_tess.tessellate_path(
            &xp,
            &StrokeOptions::tolerance(tol).with_line_width((1.35 * inv_ppi) as f32),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], x_col, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
    }

    // 5. Live Freeform / Ink Pen / Highlighter Stroke Preview
    if overlay.live_stroke_points.len() >= 2 {
        let mut sb = Path::builder();
        sb.begin(point(
            overlay.live_stroke_points[0][0] as f32,
            overlay.live_stroke_points[0][1] as f32,
        ));
        for pt in &overlay.live_stroke_points[1..] {
            sb.line_to(point(pt[0] as f32, pt[1] as f32));
        }
        sb.end(false);
        let sp = sb.build();
        let (sw, scol) = if overlay.live_stroke_highlighter {
            ((12.0 * inv_ppi) as f32, [0.980, 0.800, 0.082, 0.45])
        } else {
            ((2.2 * inv_ppi) as f32, [0.145, 0.388, 0.922, 0.95])
        };
        let _ = stroke_tess.tessellate_path(
            &sp,
            &StrokeOptions::tolerance(tol).with_line_width(sw),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], scol, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
    }

    // 6. Sub-Shape Editable Geometry Vertex Grips (`EditVertices` mode)
    if !overlay.editable_vertices.is_empty() {
        let r_outer = (4.8 * inv_ppi) as f32;
        let r_inner = (3.1 * inv_ppi) as f32;
        let violet = [0.576, 0.200, 0.918, 0.98]; // #9333EA Visio Vertex Grip
        let white = [1.0, 1.0, 1.0, 1.0];
        for vpt in &overlay.editable_vertices {
            let cx = vpt[0] as f32;
            let cy = vpt[1] as f32;
            mesh.push_quad(
                [cx - r_outer, cy - r_outer],
                [cx + r_outer, cy + r_outer],
                violet,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
            mesh.push_quad(
                [cx - r_inner, cy - r_inner],
                [cx + r_inner, cy + r_inner],
                white,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
        }
    }

    // 7. Interactive Connection Point Authoring Grips (`Shift+Ctrl+1` ConnectionPoint tool)
    if !overlay.editable_connection_points.is_empty() {
        let halo_r = 5.4 * inv_ppi;
        let cross_d = 3.8 * inv_ppi;
        let magenta = [0.859, 0.153, 0.467, 1.0]; // #DB2777 Visio Connection Point Magenta
        let white_halo = [1.0, 1.0, 1.0, 0.94];
        for cpt in &overlay.editable_connection_points {
            let mut hb = Path::builder();
            emit_rotated_ellipse(&mut hb, *cpt, halo_r, halo_r, 0.0);
            let hp = hb.build();
            let _ = fill_tess.tessellate_path(
                &hp,
                &FillOptions::tolerance(tol),
                &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                    let pos = v.position();
                    GpuVertex::new([pos.x, pos.y], white_halo, [PRIM_FLAG_OVERLAY_UI, 0.0])
                }),
            );
            let _ = stroke_tess.tessellate_path(
                &hp,
                &StrokeOptions::tolerance(tol).with_line_width((1.4 * inv_ppi) as f32),
                &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                    let pos = v.position();
                    GpuVertex::new([pos.x, pos.y], magenta, [PRIM_FLAG_OVERLAY_UI, 0.0])
                }),
            );
            let mut xb = Path::builder();
            xb.begin(point((cpt[0] - cross_d) as f32, (cpt[1] - cross_d) as f32));
            xb.line_to(point((cpt[0] + cross_d) as f32, (cpt[1] + cross_d) as f32));
            xb.end(false);
            xb.begin(point((cpt[0] - cross_d) as f32, (cpt[1] + cross_d) as f32));
            xb.line_to(point((cpt[0] + cross_d) as f32, (cpt[1] - cross_d) as f32));
            xb.end(false);
            let xp = xb.build();
            let _ = stroke_tess.tessellate_path(
                &xp,
                &StrokeOptions::tolerance(tol).with_line_width((1.85 * inv_ppi) as f32),
                &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                    let pos = v.position();
                    GpuVertex::new([pos.x, pos.y], magenta, [PRIM_FLAG_OVERLAY_UI, 0.0])
                }),
            );
        }
    }

    // 8. Draggable Horizontal & Vertical Ruler Guides (`ShapeKind::Guide`)
    for rg in &overlay.ruler_guides {
        let g_col = if rg.selected {
            [0.976, 0.451, 0.086, 0.96] // #F97316 orange-amber when selected/dragging
        } else {
            [0.008, 0.518, 0.780, 0.90] // #0284C7 Visio cyan-blue ruler guide
        };
        let half_gw = ((if rg.selected { 1.35 } else { 0.85 }) * inv_ppi) as f32;
        let s0 = rg.span_min_in.min(rg.span_max_in) as f32;
        let s1 = rg.span_min_in.max(rg.span_max_in) as f32;
        let c = rg.coord_in as f32;
        if rg.is_vertical {
            mesh.push_quad(
                [c - half_gw, s0],
                [c + half_gw, s1],
                g_col,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
        } else {
            mesh.push_quad(
                [s0, c - half_gw],
                [s1, c + half_gw],
                g_col,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
        }
    }

    // 9. Independent `Shift+Ctrl+4` Text Block Frame (`TxtPinX/Y`, `TxtWidth/Height`, `TxtAngle`)
    if let Some(tb) = &overlay.editable_text_block {
        let emerald = [0.020, 0.588, 0.412, 0.98]; // #059669 crisp Visio Text Block emerald
        let white = [1.0, 1.0, 1.0, 1.0];
        let mut fb = Path::builder();
        fb.begin(point(tb.corners_page_in[0][0] as f32, tb.corners_page_in[0][1] as f32));
        for pt in &tb.corners_page_in[1..] {
            fb.line_to(point(pt[0] as f32, pt[1] as f32));
        }
        fb.end(true);
        fb.begin(point(tb.top_mid_page_in[0] as f32, tb.top_mid_page_in[1] as f32));
        fb.line_to(point(tb.rot_knob_page_in[0] as f32, tb.rot_knob_page_in[1] as f32));
        fb.end(false);
        let sw = (1.55 * inv_ppi) as f32;
        let dashed = dash_lyon_path(&fb.build(), 2, sw, tol);
        let _ = stroke_tess.tessellate_path(
            &dashed,
            &StrokeOptions::tolerance(tol).with_line_width(sw),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], emerald, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
        // 4 corner resize handles
        let r_out = (4.6 * inv_ppi) as f32;
        let r_in = (2.9 * inv_ppi) as f32;
        for c in &tb.corners_page_in {
            let cx = c[0] as f32;
            let cy = c[1] as f32;
            mesh.push_quad(
                [cx - r_out, cy - r_out],
                [cx + r_out, cy + r_out],
                emerald,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
            mesh.push_quad(
                [cx - r_in, cy - r_in],
                [cx + r_in, cy + r_in],
                white,
                [PRIM_FLAG_OVERLAY_UI, 0.0],
            );
        }
        // Top rotation knob circle
        let kr = 5.2 * inv_ppi;
        let mut kb = Path::builder();
        emit_rotated_ellipse(&mut kb, tb.rot_knob_page_in, kr, kr, 0.0);
        let kp = kb.build();
        let _ = fill_tess.tessellate_path(
            &kp,
            &FillOptions::tolerance(tol),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], white, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
        let _ = stroke_tess.tessellate_path(
            &kp,
            &StrokeOptions::tolerance(tol).with_line_width((1.8 * inv_ppi) as f32),
            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                let pos = v.position();
                GpuVertex::new([pos.x, pos.y], emerald, [PRIM_FLAG_OVERLAY_UI, 0.0])
            }),
        );
    }

    mesh.append(&TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
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
    hash_rgba(&mut h, v.fill_bkgnd);
    hash_u64(
        &mut h,
        (v.fill_pattern as u64) | ((v.fill_gradient_radial as u64) << 8),
    );
    hash_f64(&mut h, v.fill_gradient_angle_rad);
    hash_u64(&mut h, v.gradient_stops.len() as u64);
    for gs in &v.gradient_stops {
        hash_f64(&mut h, gs.position as f64);
        hash_rgba(&mut h, Some(gs.color));
    }
    hash_rgba(&mut h, v.stroke_color);
    hash_f64(&mut h, v.stroke_weight_in);
    hash_u64(&mut h, v.line_pattern as u64);
    hash_f64(&mut h, v.rounding_in);
    hash_u64(
        &mut h,
        (v.begin_arrow as u64)
            | ((v.end_arrow as u64) << 8)
            | ((v.begin_arrow_size as u64) << 16)
            | ((v.end_arrow_size as u64) << 24),
    );
    hash_f64(&mut h, v.font_size_pt);
    hash_f64(&mut h, v.txt_pos[0]);
    hash_f64(&mut h, v.txt_pos[1]);
    hash_f64(&mut h, v.txt_width.unwrap_or(-1.0));
    hash_f64(&mut h, v.txt_height.unwrap_or(-1.0));
    hash_f64(&mut h, v.txt_angle_rad);
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
    hash_u64(&mut h, v.evaluated_text_runs.len() as u64);
    for er in &v.evaluated_text_runs {
        hash_f64(&mut h, er.font_size_pt);
        hash_rgba(&mut h, Some(er.color));
        hash_u64(
            &mut h,
            (er.bold as u64)
                | ((er.italic as u64) << 1)
                | ((er.underline as u64) << 2)
                | ((er.strikethrough as u64) << 3)
                | ((er.superscript as u64) << 4)
                | ((er.subscript as u64) << 5)
                | ((er.horiz_align as u64) << 8)
                | ((er.bullet_style as u64) << 16),
        );
        for b in er.font_family.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        for b in er.text.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    if let Some(fm) = &v.foreign_media {
        for b in fm.media_type.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        hash_u64(&mut h, fm.data.len() as u64);
        let step = (fm.data.len() / 64).max(1);
        for b in fm.data.iter().step_by(step) {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    } else {
        hash_u64(&mut h, 0);
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
    if let Some(dg) = &v.data_graphics {
        hash_u64(&mut h, 0xD6_01);
        hash_rgba(&mut h, dg.color_by_value_applied);
        if let Some(b) = &dg.icon_badge {
            hash_u64(&mut h, (b.kind as u64) + 1);
            hash_f64(&mut h, b.center_page_in[0]);
            hash_f64(&mut h, b.center_page_in[1]);
            hash_f64(&mut h, b.radius_in);
            hash_rgba(&mut h, Some(b.fill_color));
        }
        if let Some(bar) = &dg.data_bar {
            hash_f64(&mut h, bar.rect_min_in[0]);
            hash_f64(&mut h, bar.rect_min_in[1]);
            hash_f64(&mut h, bar.rect_max_in[0]);
            hash_f64(&mut h, bar.rect_max_in[1]);
            hash_f64(&mut h, bar.ratio);
            hash_rgba(&mut h, Some(bar.bar_color));
            for b in bar.value_label.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
        if let Some(co) = &dg.text_callout {
            hash_f64(&mut h, co.rect_min_in[0]);
            hash_f64(&mut h, co.rect_min_in[1]);
            hash_f64(&mut h, co.rect_max_in[0]);
            hash_f64(&mut h, co.rect_max_in[1]);
            hash_rgba(&mut h, Some(co.accent_color));
            for b in co.text.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        }
    } else {
        hash_u64(&mut h, 0);
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
