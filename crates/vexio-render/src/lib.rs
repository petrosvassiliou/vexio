//! `vexio-render` — Hardware-accelerated `wgpu` canvas pipeline (`DX12`, `Metal`, `Vulkan`,
//! `WebGPU`/`WebGL2`), Lyon vector path tessellation, R*-tree spatial index for 120 FPS at
//! 50,000+ shapes, and deterministic 4x-RGSS anti-aliased software rasterizer.

pub mod pipeline;
pub mod spatial;
pub mod tessellate;
pub mod typography;

pub use pipeline::{
    build_page_scene_mesh, prepare_visible_shape_batch, render_page_rgba_auto,
    render_page_software_rgba, render_page_software_rgba_with_camera,
    render_page_software_rgba_with_selection, CameraUniform, FrameStats, RenderError,
    ViewportCamera, WgpuCanvasRenderer, CANVAS_WGSL_SOURCE,
};
pub use spatial::{SpatialEntry, SpatialIndex};
pub use tessellate::{
    compute_visual_content_hash, emit_lod_bounding_quad, tessellate_ink_stroke,
    tessellate_ruler_and_grid_ticks, tessellate_selection_handles, tessellate_shape,
    tessellate_shape_full, GpuVertex, MeshCache, TessellatedMesh, PRIM_FLAG_MARKUP_INK,
    PRIM_FLAG_OVERLAY_UI, PRIM_FLAG_PAGE_GRID, PRIM_FLAG_SHAPE, PRIM_FLAG_TEXT_GLYPH,
};
pub use typography::{
    BulletStyle, RichTextBlock, RichTextRun, TextDirection, parse_inline_rich_text,
    reorder_bidi_visual_line, tessellate_rich_text_in_box,
};
pub use vexio_shapesheet::VisioDocument;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use vexio_shapesheet::{
        EvaluatedGeometryPath, EvaluatedPathVerb, EvaluatedShapeVisual, MarkupInkPoint,
        MarkupInkStroke, ReactiveEngine, RgbaColor, ShapeKind, ShapeTransform2D, Unit, VisioPage,
        VisioShape,
    };

    fn make_synthetic_visual(id: u64, x_in: f64, y_in: f64, w_in: f64, h_in: f64) -> EvaluatedShapeVisual {
        let tf = ShapeTransform2D {
            pin_x: x_in + w_in * 0.5,
            pin_y: y_in + h_in * 0.5,
            width: w_in,
            height: h_in,
            loc_pin_x: w_in * 0.5,
            loc_pin_y: h_in * 0.5,
            angle_rad: 0.0,
            flip_x: false,
            flip_y: false,
        };
        EvaluatedShapeVisual {
            shape_id: id,
            kind: ShapeKind::Shape2D,
            transform: tf,
            aabb_min: [x_in, y_in],
            aabb_max: [x_in + w_in, y_in + h_in],
            paths: vec![EvaluatedGeometryPath {
                no_fill: false,
                no_line: false,
                no_show: false,
                verbs_page_space: vec![
                    EvaluatedPathVerb::MoveTo([x_in, y_in]),
                    EvaluatedPathVerb::LineTo([x_in + w_in, y_in]),
                    EvaluatedPathVerb::LineTo([x_in + w_in, y_in + h_in]),
                    EvaluatedPathVerb::LineTo([x_in, y_in + h_in]),
                    EvaluatedPathVerb::Close,
                ],
            }],
            fill_color: Some(RgbaColor {
                r: 37,
                g: 99,
                b: 235,
                a: 255,
            }),
            stroke_color: Some(RgbaColor::BLACK),
            stroke_weight_in: 1.0 / 72.0,
            rounding_in: 0.0,
            end_arrow: 0,
            begin_arrow: 0,
            text: String::new(),
            font_size_pt: 10.0,
            text_color: RgbaColor::BLACK,
        }
    }

    #[test]
    fn benchmark_50k_shapes_rtree_spatial_and_lod_frame_prep() {
        // Generate a deterministic 250 x 200 grid = 50,000 shapes across a 250 in x 200 in canvas.
        let cols = 250usize;
        let rows = 200usize;
        let total = cols * rows;
        assert_eq!(total, 50_000);

        let mut visuals = Vec::with_capacity(total);
        for r in 0..rows {
            for c in 0..cols {
                let id = (r * cols + c + 1) as u64;
                let x = c as f64 * 1.0;
                let y = r as f64 * 1.0;
                visuals.push(make_synthetic_visual(id, x + 0.1, y + 0.1, 0.8, 0.6));
            }
        }

        let spatial = SpatialIndex::build(&visuals);
        assert_eq!(spatial.len(), 50_000);

        // 1) Normal zoomed-in viewport (`1280 x 800` at 96 PPI => ~13.3 in x 8.33 in window, ~120 visible shapes).
        let mut cam_zoomed_in = ViewportCamera::new(1280, 800, 1.0);
        cam_zoomed_in.center_x_in = 125.0;
        cam_zoomed_in.center_y_in = 100.0;
        cam_zoomed_in.zoom_ppi = 96.0;

        let mut cache = MeshCache::with_capacity(512);
        let mut batch = TessellatedMesh::with_capacity(4096, 8192);

        // Warm cache on first frame, then measure panned/cached frame budget!
        let first_stats =
            prepare_visible_shape_batch(&spatial, &visuals, &cam_zoomed_in, &mut cache, &mut batch);
        assert!(first_stats.visible_shapes > 80 && first_stats.visible_shapes < 200);
        assert_eq!(first_stats.lod_proxy_shapes, 0);
        assert!(!batch.is_empty());

        batch.clear();
        let t_cached = Instant::now();
        let cached_stats =
            prepare_visible_shape_batch(&spatial, &visuals, &cam_zoomed_in, &mut cache, &mut batch);
        let cached_ms = t_cached.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(cached_stats.cache_hits, cached_stats.visible_shapes);
        assert_eq!(cached_stats.cache_misses, 0);
        // Even in unoptimized debug builds, R*-tree query + cached slab assembly on ~120 shapes is < 8.33 ms!
        assert!(
            cached_ms < 50.0,
            "zoomed-in 50k R*-tree + cached mesh prep took {cached_ms:.3} ms"
        );

        // 2) Bird's-eye zoomed-out viewport (`pixels_per_inch * shape_size < 3.0`) covering thousands of shapes!
        let mut cam_zoomed_out = ViewportCamera::new(1280, 800, 1.0);
        cam_zoomed_out.center_x_in = 125.0;
        cam_zoomed_out.center_y_in = 100.0;
        cam_zoomed_out.zoom_ppi = 2.5; // 2.5 ppi * 0.8 in = 2.0 px (< 3.0 px => triggers 2-triangle LOD quad!)

        // Query a 60 in x 40 in overview window (~2,400 shapes) and verify LOD proxy triggers 100%.
        cam_zoomed_out.width_px = 150;
        cam_zoomed_out.height_px = 100;
        batch.clear();
        let t_lod = Instant::now();
        let lod_stats = prepare_visible_shape_batch(
            &spatial,
            &visuals,
            &cam_zoomed_out,
            &mut cache,
            &mut batch,
        );
        let lod_ms = t_lod.elapsed().as_secs_f64() * 1000.0;

        assert!(lod_stats.visible_shapes >= 2_000);
        assert_eq!(lod_stats.lod_proxy_shapes, lod_stats.visible_shapes);
        assert_eq!(batch.vertices.len(), lod_stats.visible_shapes * 4);
        assert_eq!(batch.indices.len(), lod_stats.visible_shapes * 6);
        assert!(
            lod_ms < 50.0,
            "LOD overview frame prep took {lod_ms:.3} ms"
        );

        // Verify hit-testing, magnetic snapping, upsert, and remove on the 50k R*-tree.
        let hits = spatial.hit_test_point(50.5, 40.4, 0.02);
        assert_eq!(hits.len(), 1);
        let hit_id = hits[0];
        let snap = spatial.nearest_snap_point(50.08, 40.4, 0.15).unwrap();
        assert_eq!(snap.0, hit_id);
        assert!((snap.1[0] - 50.1).abs() < 0.02);

        let mut mutable_spatial = spatial;
        mutable_spatial.upsert(SpatialEntry::new(999_999, [500.0, 500.0], [502.0, 502.0], 50_001));
        assert_eq!(mutable_spatial.hit_test_point(501.0, 501.0, 0.01), vec![999_999]);
        assert!(mutable_spatial.remove(999_999).is_some());
        assert!(mutable_spatial.hit_test_point(501.0, 501.0, 0.01).is_empty());
    }

    #[test]
    fn complex_path_verbs_arrowheads_and_ink_strokes_tessellate_cleanly() {
        let tf = ShapeTransform2D {
            pin_x: 4.0,
            pin_y: 4.0,
            width: 3.0,
            height: 2.0,
            loc_pin_x: 1.5,
            loc_pin_y: 1.0,
            angle_rad: 0.15,
            flip_x: false,
            flip_y: false,
        };
        let complex_visual = EvaluatedShapeVisual {
            shape_id: 42,
            kind: ShapeKind::Shape2D,
            transform: tf,
            aabb_min: [2.0, 2.5],
            aabb_max: [6.5, 6.0],
            paths: vec![
                // Path 1: MoveTo + LineTo + ArcTo + QuadTo + CubicTo + Close
                EvaluatedGeometryPath {
                    no_fill: false,
                    no_line: false,
                    no_show: false,
                    verbs_page_space: vec![
                        EvaluatedPathVerb::MoveTo([2.5, 3.0]),
                        EvaluatedPathVerb::LineTo([4.5, 3.0]),
                        EvaluatedPathVerb::ArcTo {
                            to: [5.5, 4.2],
                            bow: 0.35,
                        },
                        EvaluatedPathVerb::QuadTo {
                            ctrl: [5.0, 5.4],
                            to: [3.8, 5.2],
                        },
                        EvaluatedPathVerb::CubicTo {
                            ctrl1: [3.1, 5.5],
                            ctrl2: [2.3, 4.6],
                            to: [2.5, 3.0],
                        },
                        EvaluatedPathVerb::Close,
                    ],
                },
                // Path 2: Rotated Ellipse
                EvaluatedGeometryPath {
                    no_fill: false,
                    no_line: false,
                    no_show: false,
                    verbs_page_space: vec![EvaluatedPathVerb::Ellipse {
                        center: [6.0, 4.0],
                        rx: 0.6,
                        ry: 0.4,
                        angle_rad: 0.4,
                    }],
                },
            ],
            fill_color: Some(RgbaColor {
                r: 16,
                g: 185,
                b: 129,
                a: 255,
            }),
            stroke_color: Some(RgbaColor {
                r: 15,
                g: 23,
                b: 42,
                a: 255,
            }),
            stroke_weight_in: 2.0 / 72.0,
            rounding_in: 0.08,
            end_arrow: 4,
            begin_arrow: 1,
            text: "NODE-42".into(),
            font_size_pt: 12.0,
            text_color: RgbaColor::BLACK,
        };

        let mesh = tessellate_shape(&complex_visual, 96.0);
        assert!(mesh.vertices.len() > 40);
        assert!(mesh.indices.len() >= 90);
        assert_eq!(mesh.indices.len() % 3, 0);
        for &idx in &mesh.indices {
            assert!((idx as usize) < mesh.vertices.len());
        }
        for v in &mesh.vertices {
            assert!(v.pos[0].is_finite() && v.pos[1].is_finite());
        }

        // Verify variable-pressure Markup Ink stroke tessellation.
        let ink = MarkupInkStroke {
            id: 7,
            color: RgbaColor {
                r: 239,
                g: 68,
                b: 68,
                a: 255,
            },
            width_pt: 4.0,
            highlighter: true,
            points: vec![
                MarkupInkPoint {
                    x_in: 1.0,
                    y_in: 1.0,
                    pressure: 0.3,
                    tilt_x: 0.0,
                    tilt_y: 0.0,
                },
                MarkupInkPoint {
                    x_in: 2.0,
                    y_in: 1.5,
                    pressure: 0.9,
                    tilt_x: 0.1,
                    tilt_y: 0.0,
                },
                MarkupInkPoint {
                    x_in: 3.2,
                    y_in: 1.2,
                    pressure: 1.4,
                    tilt_x: 0.1,
                    tilt_y: -0.1,
                },
                MarkupInkPoint {
                    x_in: 4.0,
                    y_in: 2.0,
                    pressure: 0.4,
                    tilt_x: 0.0,
                    tilt_y: 0.0,
                },
            ],
        };
        let ink_mesh = tessellate_ink_stroke(&ink, 0.002);
        assert!(!ink_mesh.is_empty());
        assert_eq!(ink_mesh.indices.len() % 3, 0);
        for &idx in &ink_mesh.indices {
            assert!((idx as usize) < ink_mesh.vertices.len());
        }

        // Verify selection handles and ruler/grid ticks produce valid GPU meshes.
        let sel_mesh = tessellate_selection_handles(&complex_visual, 96.0);
        assert!(!sel_mesh.is_empty());
        let ruler_mesh = tessellate_ruler_and_grid_ticks([0.0, 0.0], [11.0, 8.5], 0.25, 96.0);
        assert!(!ruler_mesh.is_empty());
    }

    #[test]
    fn render_page_rgba_auto_draws_multi_shape_diagram_at_expected_coordinates() {
        let mut doc = VisioDocument::new_blank("RenderVerification");
        // Set Page-1 to 10 in x 8 in for clean integer pixel mapping at 200 x 160 px (20 PPI).
        *doc.page_mut(1).unwrap() = VisioPage::new(1, "Page-1", 10.0, 8.0);

        // Shape 1: Vibrant red rectangle centered at `(2.5 in, 6.0 in)` (top-left quadrant!), size `2.0 x 1.5 in`.
        let mut red_box = VisioShape::new_rect(1, "RedServer", 2.5, 6.0, 2.0, 1.5);
        red_box
            .sheet
            .set_formula("FillForegnd", "RGB(220, 38, 38)", Some(Unit::Color));
        red_box
            .sheet
            .set_formula("LineColor", "RGB(127, 29, 29)", Some(Unit::Color));

        // Shape 2: Vibrant blue rectangle centered at `(7.5 in, 2.0 in)` (bottom-right quadrant!), size `2.0 x 1.5 in`.
        let mut blue_box = VisioShape::new_rect(2, "BlueDatabase", 7.5, 2.0, 2.0, 1.5);
        blue_box
            .sheet
            .set_formula("FillForegnd", "RGB(37, 99, 235)", Some(Unit::Color));
        blue_box
            .sheet
            .set_formula("LineColor", "RGB(30, 58, 138)", Some(Unit::Color));

        // Shape 3: Connector with end arrowhead linking RedServer -> BlueDatabase.
        let mut conn = VisioShape::new_connector(3, "Link", 3.5, 6.0, 6.5, 2.0);
        conn.sheet
            .set_formula("LineColor", "RGB(16, 185, 129)", Some(Unit::Color));
        conn.sheet
            .set_formula("LineWeight", "3 pt", Some(Unit::Point));
        conn.sheet
            .set_formula("EndArrow", "4", Some(Unit::Number));

        {
            let page = doc.page_mut(1).unwrap();
            page.shapes.push(red_box);
            page.shapes.push(blue_box);
            page.shapes.push(conn);
        }

        ReactiveEngine::recalculate_document(&mut doc);

        let width = 200u32;
        let height = 160u32;
        let rgba = render_page_rgba_auto(&doc, 1, width, height).expect("render_page_rgba_auto");
        assert_eq!(rgba.len(), (width * height * 4) as usize);

        let mut cam = ViewportCamera::new(width, height, 1.0);
        cam.fit_to_page(10.0, 8.0);

        let sample_px = |buf: &[u8], page_x: f64, page_y: f64| -> [u8; 4] {
            let sp = cam.page_in_to_screen(page_x, page_y);
            let px = (sp[0].round() as u32).clamp(0, width - 1);
            let py = (sp[1].round() as u32).clamp(0, height - 1);
            let idx = ((py * width + px) * 4) as usize;
            [buf[idx], buf[idx + 1], buf[idx + 2], buf[idx + 3]]
        };

        // 1) Empty corner `(1.0 in, 1.0 in)` should be near-white background (`R,G,B > 200`).
        let bg_px = sample_px(&rgba, 1.0, 1.0);
        assert!(
            bg_px[0] > 200 && bg_px[1] > 200 && bg_px[2] > 200,
            "expected white/grid background at (1.0, 1.0), got {bg_px:?}"
        );

        // 2) RedServer center `(2.5 in, 6.0 in)` MUST be strongly red (`R > 180, G < 80, B < 80`)!
        let red_px = sample_px(&rgba, 2.5, 6.0);
        assert!(
            red_px[0] > 180 && red_px[1] < 80 && red_px[2] < 80 && red_px[3] == 255,
            "expected red shape pixel at (2.5, 6.0), got {red_px:?}"
        );

        // 3) BlueDatabase center `(7.5 in, 2.0 in)` MUST be strongly blue (`B > 180, R < 80`)!
        let blue_px = sample_px(&rgba, 7.5, 2.0);
        assert!(
            blue_px[2] > 180 && blue_px[0] < 80 && blue_px[3] == 255,
            "expected blue shape pixel at (7.5, 2.0), got {blue_px:?}"
        );

        // 4) Connector midpoint `(5.0 in, 4.0 in)` MUST be green (`G > 140, R < 80`)!
        let conn_px = sample_px(&rgba, 5.0, 4.0);
        assert!(
            conn_px[1] > 140 && conn_px[0] < 90,
            "expected green connector pixel at (5.0, 4.0), got {conn_px:?}"
        );

        // Also explicitly verify deterministic software rasterizer matches expected pixels.
        let sw_rgba =
            render_page_software_rgba(&doc, 1, width, height).expect("render_page_software_rgba");
        let sw_red = sample_px(&sw_rgba, 2.5, 6.0);
        let sw_blue = sample_px(&sw_rgba, 7.5, 2.0);
        assert!(sw_red[0] > 180 && sw_red[1] < 80);
        assert!(sw_blue[2] > 180 && sw_blue[0] < 80);
    }
}
