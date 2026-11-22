//! Hardware-accelerated `wgpu` rendering pipeline (`DX12`, `Metal`, `Vulkan`, `WebGPU`/`WebGL2`),
//! `ViewportCamera` coordinate transforms, offscreen RGBA readback, and deterministic 4x-RGSS
//! anti-aliased software raster fallback for headless sandboxes, Quick Look previews, and CI.

use crate::spatial::SpatialIndex;
use crate::tessellate::{
    tessellate_ink_stroke, tessellate_ruler_and_grid_ticks, tessellate_selection_handles,
    GpuVertex, MeshCache, TessellatedMesh, PRIM_FLAG_PAGE_GRID, PRIM_FLAG_SHAPE,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vexio_shapesheet::{
    EvaluatedShapeVisual, ReactiveEngine, RgbaColor, VisioDocument, VisioPage,
};

/// WGSL shader source embedded at compile time.
pub const CANVAS_WGSL_SOURCE: &str = include_str!("shaders/canvas.wgsl");

/// GPU uniform block matching `struct CameraUniform` in `src/shaders/canvas.wgsl` (`80 bytes`, 16-byte aligned).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub viewport_px: [f32; 2],
    pub zoom_ppi: f32,
    pub grid_spacing_in: f32,
}

/// Errors emitted by hardware or software rendering operations.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("no compatible wgpu graphics adapter found")]
    NoAdapter,
    #[error("failed to request wgpu device: {0}")]
    RequestDevice(String),
    #[error("wgpu surface error: {0}")]
    Surface(String),
    #[error("page id {0} not found in VisioDocument")]
    PageNotFound(u32),
    #[error("invalid render target dimensions {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },
    #[error("failed to map offscreen readback buffer: {0}")]
    BufferReadback(String),
}

/// Per-frame telemetry for spatial frustum culling, LOD proxy usage, `MeshCache` efficiency, and CPU prep budget.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct FrameStats {
    pub total_shapes: usize,
    pub visible_shapes: usize,
    pub lod_proxy_shapes: usize,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub vertex_count: usize,
    pub index_count: usize,
    pub cpu_prep_time_ms: f64,
}

/// 2D pan/zoom camera mapping canonical Visio page inches (`+X` right, `+Y` up, bottom-left origin)
/// to physical viewport pixels (`+X` right, `+Y` down, top-left origin) and WebGPU NDC clip space.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportCamera {
    pub center_x_in: f64,
    pub center_y_in: f64,
    /// Logical pixels per page inch (`96.0` = 100% zoom at standard 96 PPI).
    pub zoom_ppi: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi_scale: f64,
}

impl Default for ViewportCamera {
    fn default() -> Self {
        Self {
            center_x_in: 5.5,
            center_y_in: 4.25,
            zoom_ppi: 96.0,
            width_px: 1280,
            height_px: 800,
            dpi_scale: 1.0,
        }
    }
}

impl ViewportCamera {
    pub fn new(width_px: u32, height_px: u32, dpi_scale: f64) -> Self {
        Self {
            width_px: width_px.max(1),
            height_px: height_px.max(1),
            dpi_scale: sanitize_positive(dpi_scale, 1.0),
            ..Self::default()
        }
    }

    /// Physical device pixels per page inch (`zoom_ppi * dpi_scale`).
    #[inline]
    pub fn effective_ppi(&self) -> f64 {
        sanitize_positive(self.zoom_ppi, 96.0) * sanitize_positive(self.dpi_scale, 1.0)
    }

    /// Convert physical screen coordinates `(screen_x_px, screen_y_px)` (top-left origin, `+Y` down)
    /// into canonical Visio page coordinates `(x_in, y_in)` (bottom-left origin, `+Y` up).
    #[inline]
    pub fn screen_to_page_in(&self, screen_x_px: f64, screen_y_px: f64) -> [f64; 2] {
        let ppi = self.effective_ppi();
        let half_w = (self.width_px.max(1) as f64) * 0.5;
        let half_h = (self.height_px.max(1) as f64) * 0.5;
        let x_in = self.center_x_in + (screen_x_px - half_w) / ppi;
        let y_in = self.center_y_in - (screen_y_px - half_h) / ppi;
        [x_in, y_in]
    }

    /// Convert canonical Visio page coordinates `(page_x_in, page_y_in)` into physical screen pixels.
    #[inline]
    pub fn page_in_to_screen(&self, page_x_in: f64, page_y_in: f64) -> [f64; 2] {
        let ppi = self.effective_ppi();
        let half_w = (self.width_px.max(1) as f64) * 0.5;
        let half_h = (self.height_px.max(1) as f64) * 0.5;
        let sx = half_w + (page_x_in - self.center_x_in) * ppi;
        let sy = half_h - (page_y_in - self.center_y_in) * ppi;
        [sx, sy]
    }

    /// Compute the world-space AABB `([min_x_in, min_y_in], [max_x_in, max_y_in])` currently visible.
    #[inline]
    pub fn visible_aabb_in(&self) -> ([f64; 2], [f64; 2]) {
        let top_left = self.screen_to_page_in(0.0, 0.0);
        let bottom_right =
            self.screen_to_page_in(self.width_px.max(1) as f64, self.height_px.max(1) as f64);
        let min_x = top_left[0].min(bottom_right[0]);
        let max_x = top_left[0].max(bottom_right[0]);
        let min_y = top_left[1].min(bottom_right[1]);
        let max_y = top_left[1].max(bottom_right[1]);
        ([min_x, min_y], [max_x, max_y])
    }

    /// Center and zoom so `[0..page_width_in] x [0..page_height_in]` fits cleanly inside the viewport.
    pub fn fit_to_page(&mut self, page_width_in: f64, page_height_in: f64) {
        self.fit_to_page_with_margin(page_width_in, page_height_in, 0.0);
    }

    /// Fit page into viewport with an optional fractional margin (`0.05` = 5% border around page).
    pub fn fit_to_page_with_margin(
        &mut self,
        page_width_in: f64,
        page_height_in: f64,
        margin_fraction: f64,
    ) {
        let pw = sanitize_positive(page_width_in, 8.5);
        let ph = sanitize_positive(page_height_in, 11.0);
        let margin = margin_fraction.clamp(0.0, 0.45);
        let avail_w = (self.width_px.max(1) as f64) * (1.0 - 2.0 * margin);
        let avail_h = (self.height_px.max(1) as f64) * (1.0 - 2.0 * margin);
        let dpi = sanitize_positive(self.dpi_scale, 1.0);

        self.center_x_in = pw * 0.5;
        self.center_y_in = ph * 0.5;
        let ppi_x = (avail_w / dpi) / pw;
        let ppi_y = (avail_h / dpi) / ph;
        self.zoom_ppi = ppi_x.min(ppi_y).clamp(1.0, 25_000.0);
    }

    /// Pan camera by `(dx_px, dy_px)` physical screen pixels.
    pub fn pan_by_pixels(&mut self, dx_px: f64, dy_px: f64) {
        let ppi = self.effective_ppi();
        self.center_x_in -= dx_px / ppi;
        self.center_y_in += dy_px / ppi;
    }

    /// Zoom around a stationary screen cursor anchor `(anchor_x_px, anchor_y_px)`.
    pub fn zoom_around_screen_point(&mut self, zoom_factor: f64, anchor_x_px: f64, anchor_y_px: f64) {
        let before = self.screen_to_page_in(anchor_x_px, anchor_y_px);
        let factor = sanitize_positive(zoom_factor, 1.0);
        self.zoom_ppi = (self.zoom_ppi * factor).clamp(1.0, 25_000.0);
        let after = self.screen_to_page_in(anchor_x_px, anchor_y_px);
        self.center_x_in += before[0] - after[0];
        self.center_y_in += before[1] - after[1];
    }

    /// Build the column-major orthographic `CameraUniform` consumed by `canvas.wgsl`.
    pub fn to_uniform(&self, grid_spacing_in: f64) -> CameraUniform {
        let w = (self.width_px.max(1)) as f64;
        let h = (self.height_px.max(1)) as f64;
        let ppi = self.effective_ppi();

        // Maps page_in -> NDC [-1, +1] (+X right, +Y up in both Visio page space and WebGPU NDC!).
        let sx = (2.0 * ppi / w) as f32;
        let sy = (2.0 * ppi / h) as f32;
        let tx = (-self.center_x_in * 2.0 * ppi / w) as f32;
        let ty = (-self.center_y_in * 2.0 * ppi / h) as f32;

        CameraUniform {
            view_proj: [
                [sx, 0.0, 0.0, 0.0],
                [0.0, sy, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [tx, ty, 0.0, 1.0],
            ],
            viewport_px: [w as f32, h as f32],
            zoom_ppi: ppi as f32,
            grid_spacing_in: if grid_spacing_in.is_finite() && grid_spacing_in > 0.0 {
                grid_spacing_in as f32
            } else {
                0.0
            },
        }
    }
}

/// High-throughput frame preparation over a pre-built `SpatialIndex` and slice of `EvaluatedShapeVisual`s.
/// Executes `O(log N)` R*-tree viewport culling and assembles visible meshes using `MeshCache` +
/// sub-3px 2-triangle LOD proxy quads to sustain `< 8.33 ms` (120 FPS) on 50,000+ shapes.
pub fn prepare_visible_shape_batch(
    spatial: &SpatialIndex,
    visuals: &[EvaluatedShapeVisual],
    camera: &ViewportCamera,
    mesh_cache: &mut MeshCache,
    batch_out: &mut TessellatedMesh,
) -> FrameStats {
    let t0 = monotonic_now_ms();
    mesh_cache.reset_counters();

    let (vmin, vmax) = camera.visible_aabb_in();
    let visible_entries = spatial.query_viewport_entries(vmin, vmax);
    let ppi = camera.effective_ppi();

    // Reserve vertex/index buffers up-front so 50k LOD quads assemble without reallocations.
    batch_out.vertices.reserve(visible_entries.len() * 4);
    batch_out.indices.reserve(visible_entries.len() * 6);

    // Fast O(1) index lookup per visible entry when `visuals` matches `z_order` from `SpatialIndex::build(visuals)`,
    // building a fallback lookup map lazily only if `visuals` was mutated out-of-line.
    let mut fallback_map: Option<BTreeMap<u64, &EvaluatedShapeVisual>> = None;
    let mut lod_proxy_shapes = 0usize;
    let mut rendered = 0usize;

    for entry in &visible_entries {
        let vis_opt = visuals
            .get(entry.z_order as usize)
            .filter(|v| v.shape_id == entry.shape_id)
            .or_else(|| {
                let map = fallback_map.get_or_insert_with(|| {
                    visuals.iter().map(|v| (v.shape_id, v)).collect()
                });
                map.get(&entry.shape_id).copied()
            });
        if let Some(vis) = vis_opt {
            rendered += 1;
            if mesh_cache.append_shape_with_lod(vis, ppi, batch_out) {
                lod_proxy_shapes += 1;
            }
        }
    }

    let elapsed = (monotonic_now_ms() - t0).max(0.0);
    FrameStats {
        total_shapes: spatial.len(),
        visible_shapes: rendered,
        lod_proxy_shapes,
        cache_hits: mesh_cache.hits(),
        cache_misses: mesh_cache.misses(),
        vertex_count: batch_out.vertices.len(),
        index_count: batch_out.indices.len(),
        cpu_prep_time_ms: elapsed,
    }
}

/// Assemble the complete vector scene mesh for `page_id` in `doc` (page sheet + grid quad,
/// optional background page, R*-tree culled foreground shapes, markup ink strokes, selection & rulers).
pub fn build_page_scene_mesh(
    doc: &VisioDocument,
    page_id: u32,
    camera: &ViewportCamera,
    mesh_cache: &mut MeshCache,
    show_grid: bool,
    show_rulers: bool,
    selected_shapes: &[u64],
) -> Result<(TessellatedMesh, FrameStats, f64), RenderError> {
    let t0 = monotonic_now_ms();
    let page = doc
        .page(page_id)
        .or_else(|| doc.pages.iter().find(|p| !p.is_background))
        .or_else(|| doc.pages.first())
        .ok_or(RenderError::PageNotFound(page_id))?;
    let resolved_page_id = page.id;

    let page_w = page.width_in();
    let page_h = page.height_in();
    let grid_spacing = if show_grid {
        page_grid_spacing_in(page)
    } else {
        0.0
    };

    let mut scene = TessellatedMesh::with_capacity(512, 1024);

    // Subtle drop-shadow behind page sheet when zoomed out enough to see canvaspasteboard.
    let inv_ppi = (1.0 / camera.effective_ppi().max(1.0)) as f32;
    let shadow_off = 4.0 * inv_ppi;
    scene.push_quad(
        [shadow_off, -shadow_off],
        [page_w as f32 + shadow_off, page_h as f32 - shadow_off],
        [0.08, 0.10, 0.14, 0.16],
        [PRIM_FLAG_SHAPE, 0.0],
    );

    // Page sheet quad tagged with `PRIM_FLAG_PAGE_GRID` so `canvas.wgsl` & software rasterizer
    // render crisp anti-aliased Visio grid lines directly on the sheet.
    let page_bg = active_page_background_color(doc, resolved_page_id).to_linear_f32();
    scene.push_quad(
        [0.0, 0.0],
        [page_w as f32, page_h as f32],
        page_bg,
        [
            if show_grid {
                PRIM_FLAG_PAGE_GRID
            } else {
                PRIM_FLAG_SHAPE
            },
            0.0,
        ],
    );

    // Crisp 1px page sheet outline.
    emit_rect_outline_quads(
        &mut scene,
        [0.0, 0.0],
        [page_w as f32, page_h as f32],
        inv_ppi.max(0.004),
        [0.58, 0.64, 0.72, 1.0],
    );

    // Render attached background page (`back_page_id`) underneath foreground shapes.
    if let Some(back_id) = page.back_page_id {
        if back_id != resolved_page_id {
            if let Some(back_page) = doc.page(back_id) {
                let back_visuals = ReactiveEngine::evaluate_page_visuals(doc, back_id);
                let back_spatial = SpatialIndex::build(&back_visuals);
                let _ = prepare_visible_shape_batch(
                    &back_spatial,
                    &back_visuals,
                    camera,
                    mesh_cache,
                    &mut scene,
                );
                let ink_tol = (0.35 / camera.effective_ppi()).clamp(0.0004, 0.01) as f32;
                for stroke in &back_page.ink_strokes {
                    scene.append(&tessellate_ink_stroke(stroke, ink_tol));
                }
            }
        }
    }

    // Evaluate and spatially cull foreground shapes.
    let visuals = ReactiveEngine::evaluate_page_visuals(doc, resolved_page_id);
    let spatial = SpatialIndex::build(&visuals);
    let mut stats =
        prepare_visible_shape_batch(&spatial, &visuals, camera, mesh_cache, &mut scene);

    // Foreground Markup Ink strokes.
    let ink_tol = (0.35 / camera.effective_ppi()).clamp(0.0004, 0.01) as f32;
    for stroke in &page.ink_strokes {
        scene.append(&tessellate_ink_stroke(stroke, ink_tol));
    }

    // Selection handles for active selection.
    if !selected_shapes.is_empty() {
        for vis in &visuals {
            if selected_shapes.contains(&vis.shape_id) {
                scene.append(&tessellate_selection_handles(vis, camera.effective_ppi()));
            }
        }
    }

    // Optional interactive rulers along top/left viewport edges.
    if show_rulers {
        let (vmin, vmax) = camera.visible_aabb_in();
        scene.append(&tessellate_ruler_and_grid_ticks(
            vmin,
            vmax,
            grid_spacing.max(0.25),
            camera.effective_ppi(),
        ));
    }

    stats.vertex_count = scene.vertices.len();
    stats.index_count = scene.indices.len();
    stats.cpu_prep_time_ms = (monotonic_now_ms() - t0).max(0.0);

    Ok((scene, stats, grid_spacing))
}

struct AttachedSurface {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    msaa_view: Option<wgpu::TextureView>,
}

/// Hardware-accelerated `wgpu` canvas renderer supporting both swapchain surfaces (`Surface<'static>`)
/// and headless offscreen RGBA rendering with configurable `1x` or `4x` MSAA.
pub struct WgpuCanvasRenderer {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    camera_bind_group_layout: wgpu::BindGroupLayout,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    pipeline_layout: wgpu::PipelineLayout,
    pipelines: BTreeMap<(u32, bool), (wgpu::TextureFormat, wgpu::RenderPipeline)>,
    sample_count: u32,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity_bytes: u64,
    index_buffer: wgpu::Buffer,
    index_capacity_bytes: u64,
    attached_surface: Option<AttachedSurface>,
    pub camera: ViewportCamera,
    pub mesh_cache: MeshCache,
    pub show_grid: bool,
    pub show_rulers: bool,
    pub selected_shapes: Vec<u64>,
    pub pasteboard_color: RgbaColor,
}

impl WgpuCanvasRenderer {
    /// Asynchronously initialize `wgpu::Instance` (`Backends::all()`), `Adapter`, `Device`, `Queue`,
    /// `canvas.wgsl` shader module, `BindGroupLayout`, and dynamic GPU geometry buffers.
    pub async fn new_async(sample_count: u32) -> Result<Self, RenderError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        Self::init_with_instance_and_surface(instance, None, 1280, 800, sample_count).await
    }

    /// Synchronously initialize a headless `WgpuCanvasRenderer` using `pollster::block_on`.
    pub fn new_headless(sample_count: u32) -> Result<Self, RenderError> {
        pollster::block_on(Self::new_async(sample_count))
    }

    /// Initialize `WgpuCanvasRenderer` bound to an existing native/web `wgpu::Surface<'static>`.
    pub async fn new_with_surface_async(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        sample_count: u32,
    ) -> Result<Self, RenderError> {
        Self::init_with_instance_and_surface(instance, Some(surface), width, height, sample_count)
            .await
    }

    /// Bind a DirectX 12 WinUI 3 `ISwapChainPanelNative` (`IUnknown*`) directly to `wgpu`.
    ///
    /// # Safety
    /// `panel_unk` must be a valid, non-null COM pointer to a WinUI 3 `SwapChainPanel` outliving the surface.
    #[cfg(target_os = "windows")]
    pub unsafe fn new_dx12_swapchain_panel(
        panel_unk: *mut std::ffi::c_void,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<Self, RenderError> {
        let non_null = std::ptr::NonNull::new(panel_unk)
            .ok_or_else(|| RenderError::Surface("null SwapChainPanel COM pointer".into()))?;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12 | wgpu::Backends::VULKAN,
            ..Default::default()
        });
        // SAFETY: Caller guarantees `non_null` is a valid WinUI 3 `SwapChainPanel` COM object.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::SwapChainPanel(
                non_null.as_ptr(),
            ))
        }
        .map_err(|e| RenderError::Surface(e.to_string()))?;
        let mut r = pollster::block_on(Self::new_with_surface_async(
            instance,
            surface,
            width.max(1),
            height.max(1),
            4,
        ))?;
        r.camera.dpi_scale = sanitize_positive(dpi_scale as f64, 1.0);
        Ok(r)
    }

    /// Bind a macOS AppKit/SwiftUI `CAMetalLayer*` directly to `wgpu` Metal backend.
    ///
    /// # Safety
    /// `layer_ptr` must be a valid, non-null `CAMetalLayer` Objective-C object outliving the surface.
    #[cfg(target_os = "macos")]
    pub unsafe fn new_metal_layer(
        layer_ptr: *mut std::ffi::c_void,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<Self, RenderError> {
        if layer_ptr.is_null() {
            return Err(RenderError::Surface("null CAMetalLayer pointer".into()));
        }
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..Default::default()
        });
        // SAFETY: Caller guarantees `layer_ptr` is a valid retained `CAMetalLayer` instance.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_ptr))
        }
        .map_err(|e| RenderError::Surface(e.to_string()))?;
        let mut r = pollster::block_on(Self::new_with_surface_async(
            instance,
            surface,
            width.max(1),
            height.max(1),
            4,
        ))?;
        r.camera.dpi_scale = sanitize_positive(dpi_scale as f64, 1.0);
        Ok(r)
    }

    /// Bind an Android NDK `ANativeWindow*` directly to `wgpu` Vulkan/GLES backend.
    ///
    /// # Safety
    /// `a_native_window` must be a valid, acquired `ANativeWindow*` outliving the surface.
    #[cfg(target_os = "android")]
    pub unsafe fn new_android_native_window(
        a_native_window: *mut std::ffi::c_void,
        width: u32,
        height: u32,
        dpi_scale: f32,
    ) -> Result<Self, RenderError> {
        let non_null = std::ptr::NonNull::new(a_native_window)
            .ok_or_else(|| RenderError::Surface("null ANativeWindow pointer".into()))?;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });
        let win_handle = wgpu::rwh::AndroidNdkWindowHandle::new(non_null);
        let disp_handle = wgpu::rwh::AndroidDisplayHandle::new();
        // SAFETY: Caller guarantees `a_native_window` was acquired via `ANativeWindow_fromSurface` and remains valid.
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: wgpu::rwh::RawDisplayHandle::Android(disp_handle),
                raw_window_handle: wgpu::rwh::RawWindowHandle::AndroidNdk(win_handle),
            })
        }
        .map_err(|e| RenderError::Surface(e.to_string()))?;
        let mut r = pollster::block_on(Self::new_with_surface_async(
            instance,
            surface,
            width.max(1),
            height.max(1),
            4,
        ))?;
        r.camera.dpi_scale = sanitize_positive(dpi_scale as f64, 1.0);
        Ok(r)
    }

    async fn init_with_instance_and_surface(
        instance: wgpu::Instance,
        compatible_surface: Option<wgpu::Surface<'static>>,
        width: u32,
        height: u32,
        requested_samples: u32,
    ) -> Result<Self, RenderError> {
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: compatible_surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
        {
            Some(a) => a,
            None => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: compatible_surface.as_ref(),
                    force_fallback_adapter: true,
                })
                .await
                .ok_or(RenderError::NoAdapter)?,
        };

        let required_limits =
            wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits());
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("vexio-render-device"),
                    required_features: wgpu::Features::empty(),
                    required_limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .map_err(|e| RenderError::RequestDevice(e.to_string()))?;

        let sample_count = if requested_samples >= 4 { 4 } else { 1 };

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vexio-canvas-wgsl"),
            source: wgpu::ShaderSource::Wgsl(CANVAS_WGSL_SOURCE.into()),
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("vexio-camera-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            std::mem::size_of::<CameraUniform>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let camera = ViewportCamera::new(width.max(1), height.max(1), 1.0);
        let initial_uniform = camera.to_uniform(0.25);
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vexio-camera-ubo"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&camera_buffer, 0, bytemuck::bytes_of(&initial_uniform));

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("vexio-camera-bg"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("vexio-canvas-pipeline-layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let initial_vb_bytes = 64 * 1024;
        let initial_ib_bytes = 64 * 1024;
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vexio-canvas-vb"),
            size: initial_vb_bytes,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vexio-canvas-ib"),
            size: initial_ib_bytes,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut renderer = Self {
            instance,
            adapter,
            device,
            queue,
            shader,
            camera_bind_group_layout,
            camera_buffer,
            camera_bind_group,
            pipeline_layout,
            pipelines: BTreeMap::new(),
            sample_count,
            vertex_buffer,
            vertex_capacity_bytes: initial_vb_bytes,
            index_buffer,
            index_capacity_bytes: initial_ib_bytes,
            attached_surface: None,
            camera,
            mesh_cache: MeshCache::new(),
            show_grid: true,
            show_rulers: false,
            selected_shapes: Vec::new(),
            pasteboard_color: RgbaColor {
                r: 226,
                g: 232,
                b: 240,
                a: 255,
            },
        };

        // Pre-build standard offscreen Rgba8Unorm pipeline (`sample_count` + `1x` fallback).
        renderer.ensure_pipeline(wgpu::TextureFormat::Rgba8Unorm, sample_count);

        if let Some(surface) = compatible_surface {
            renderer.attach_surface(surface, width.max(1), height.max(1))?;
        }

        Ok(renderer)
    }

    #[inline]
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    #[inline]
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    #[inline]
    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    #[inline]
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    #[inline]
    pub fn sample_count(&self) -> u32 {
        self.sample_count
    }

    pub fn set_sample_count(&mut self, samples: u32) {
        let next = if samples >= 4 { 4 } else { 1 };
        if self.sample_count != next {
            self.sample_count = next;
            if let Some(surf) = &self.attached_surface {
                let fmt = surf.config.format;
                let w = surf.config.width;
                let h = surf.config.height;
                let effective = self.effective_samples_for_format(fmt, next);
                let msaa_view = if effective > 1 {
                    Some(create_msaa_view(&self.device, fmt, w, h, effective))
                } else {
                    None
                };
                if let Some(s) = &mut self.attached_surface {
                    s.msaa_view = msaa_view;
                }
            }
        }
    }

    /// Configure and attach a swapchain `wgpu::Surface<'static>` for interactive rendering.
    pub fn attach_surface(
        &mut self,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        let w = width.max(1);
        let h = height.max(1);
        let caps = surface.get_capabilities(&self.adapter);
        if caps.formats.is_empty() {
            return Err(RenderError::Surface(
                "surface reported no supported formats for adapter".into(),
            ));
        }
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        let alpha_mode = caps
            .alpha_modes
            .first()
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: w,
            height: h,
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&self.device, &config);

        let eff_samples = self.effective_samples_for_format(format, self.sample_count);
        self.ensure_pipeline(format, eff_samples);
        let msaa_view = if eff_samples > 1 {
            Some(create_msaa_view(&self.device, format, w, h, eff_samples))
        } else {
            None
        };

        self.camera.width_px = w;
        self.camera.height_px = h;
        self.attached_surface = Some(AttachedSurface {
            surface,
            config,
            msaa_view,
        });
        Ok(())
    }

    /// Resize the attached swapchain surface and update `ViewportCamera` dimensions.
    pub fn resize_surface(&mut self, width: u32, height: u32, dpi_scale: f64) {
        let w = width.max(1);
        let h = height.max(1);
        self.camera.width_px = w;
        self.camera.height_px = h;
        self.camera.dpi_scale = sanitize_positive(dpi_scale, 1.0);

        if let Some(att) = &mut self.attached_surface {
            att.config.width = w;
            att.config.height = h;
            att.surface.configure(&self.device, &att.config);
            let fmt = att.config.format;
            let eff_samples = if self.sample_count >= 4 {
                let flags = self.adapter.get_texture_format_features(fmt).flags;
                if flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4)
                    && flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
                {
                    4
                } else {
                    1
                }
            } else {
                1
            };
            att.msaa_view = if eff_samples > 1 {
                Some(create_msaa_view(&self.device, fmt, w, h, eff_samples))
            } else {
                None
            };
        }
    }

    /// Render `page_id` directly onto the attached swapchain `Surface<'static>` and present.
    pub fn render_surface_frame(
        &mut self,
        doc: &VisioDocument,
        page_id: u32,
    ) -> Result<FrameStats, RenderError> {
        let (mesh, stats, grid_spacing) = build_page_scene_mesh(
            doc,
            page_id,
            &self.camera,
            &mut self.mesh_cache,
            self.show_grid,
            self.show_rulers,
            &self.selected_shapes,
        )?;

        let Some(att_fmt) = self.attached_surface.as_ref().map(|a| a.config.format) else {
            return Err(RenderError::Surface("no surface attached".into()));
        };
        let eff_samples = self.effective_samples_for_format(att_fmt, self.sample_count);
        self.ensure_pipeline(att_fmt, eff_samples);
        self.upload_mesh_and_uniform(&mesh, grid_spacing);

        let att = self
            .attached_surface
            .as_ref()
            .ok_or_else(|| RenderError::Surface("no surface attached".into()))?;
        let frame = att
            .surface
            .get_current_texture()
            .map_err(|e| RenderError::Surface(e.to_string()))?;
        let target_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let pipeline = self.lookup_pipeline(att_fmt, eff_samples);
        let clear = to_wgpu_color(self.pasteboard_color);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vexio-surface-encoder"),
            });

        {
            let (view, resolve_target) = if let Some(msaa) = &att.msaa_view {
                (msaa, Some(&target_view))
            } else {
                (&target_view, None)
            };
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vexio-surface-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            if !mesh.is_empty() {
                let vb_bytes = (mesh.vertices.len() * std::mem::size_of::<GpuVertex>()) as u64;
                let ib_bytes = (mesh.indices.len() * std::mem::size_of::<u32>()) as u64;
                rpass.set_vertex_buffer(0, self.vertex_buffer.slice(0..vb_bytes));
                rpass.set_index_buffer(
                    self.index_buffer.slice(0..ib_bytes),
                    wgpu::IndexFormat::Uint32,
                );
                rpass.draw_indexed(0..(mesh.indices.len() as u32), 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
        Ok(stats)
    }

    /// Render `page_id` offscreen at `width x height` via hardware `wgpu` (with MSAA resolve when supported)
    /// and read back tight `RGBA8` pixel bytes (`width * height * 4`).
    pub fn render_page_offscreen_rgba(
        &mut self,
        doc: &VisioDocument,
        page_id: u32,
        width: u32,
        height: u32,
    ) -> Result<Vec<u8>, RenderError> {
        if width == 0 || height == 0 {
            return Err(RenderError::InvalidDimensions { width, height });
        }
        let page = doc.page(page_id).ok_or(RenderError::PageNotFound(page_id))?;

        let prev_camera = self.camera;
        let mut offscreen_cam = ViewportCamera::new(width, height, 1.0);
        offscreen_cam.fit_to_page(page.width_in(), page.height_in());
        self.camera = offscreen_cam;

        let scene_res = build_page_scene_mesh(
            doc,
            page_id,
            &self.camera,
            &mut self.mesh_cache,
            self.show_grid,
            false,
            &[],
        );
        let (mesh, _stats, grid_spacing) = match scene_res {
            Ok(v) => v,
            Err(e) => {
                self.camera = prev_camera;
                return Err(e);
            }
        };

        let format = wgpu::TextureFormat::Rgba8Unorm;
        let eff_samples = self.effective_samples_for_format(format, self.sample_count);
        self.ensure_pipeline(format, eff_samples);
        self.upload_mesh_and_uniform(&mesh, grid_spacing);
        self.camera = prev_camera;

        let resolve_tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vexio-offscreen-rgba"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let resolve_view = resolve_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let msaa_view = if eff_samples > 1 {
            Some(create_msaa_view(
                &self.device,
                format,
                width,
                height,
                eff_samples,
            ))
        } else {
            None
        };

        let unpadded_bytes_per_row = width * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let readback_size = (padded_bytes_per_row as u64) * (height as u64);

        let readback_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vexio-offscreen-readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let pipeline = self.lookup_pipeline(format, eff_samples);
        let page_bg = active_page_background_color(doc, page_id);
        let clear = to_wgpu_color(page_bg);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vexio-offscreen-encoder"),
            });

        {
            let (view, resolve_target) = if let Some(msaa) = &msaa_view {
                (msaa, Some(&resolve_view))
            } else {
                (&resolve_view, None)
            };
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("vexio-offscreen-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rpass.set_pipeline(pipeline);
            rpass.set_bind_group(0, &self.camera_bind_group, &[]);
            if !mesh.is_empty() {
                let vb_bytes = (mesh.vertices.len() * std::mem::size_of::<GpuVertex>()) as u64;
                let ib_bytes = (mesh.indices.len() * std::mem::size_of::<u32>()) as u64;
                rpass.set_vertex_buffer(0, self.vertex_buffer.slice(0..vb_bytes));
                rpass.set_index_buffer(
                    self.index_buffer.slice(0..ib_bytes),
                    wgpu::IndexFormat::Uint32,
                );
                rpass.draw_indexed(0..(mesh.indices.len() as u32), 0, 0..1);
            }
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &resolve_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback_buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        let slice = readback_buf.slice(..);
        let mapped_flag = std::sync::Arc::new(std::sync::Mutex::new(None));
        let flag_clone = std::sync::Arc::clone(&mapped_flag);
        slice.map_async(wgpu::MapMode::Read, move |res| {
            if let Ok(mut guard) = flag_clone.lock() {
                *guard = Some(res);
            }
        });
        self.device.poll(wgpu::Maintain::Wait);

        let map_status = mapped_flag
            .lock()
            .ok()
            .and_then(|mut g| g.take())
            .ok_or_else(|| RenderError::BufferReadback("map_async callback did not complete".into()))?;
        map_status.map_err(|e| RenderError::BufferReadback(e.to_string()))?;

        let mapped = slice.get_mapped_range();
        let mut rgba = vec![0u8; (unpadded_bytes_per_row * height) as usize];
        for row in 0..height {
            let src_start = (row * padded_bytes_per_row) as usize;
            let src_end = src_start + (unpadded_bytes_per_row as usize);
            let dst_start = (row * unpadded_bytes_per_row) as usize;
            let dst_end = dst_start + (unpadded_bytes_per_row as usize);
            rgba[dst_start..dst_end].copy_from_slice(&mapped[src_start..src_end]);
        }
        drop(mapped);
        readback_buf.unmap();

        Ok(rgba)
    }

    fn effective_samples_for_format(&self, format: wgpu::TextureFormat, desired: u32) -> u32 {
        if desired < 4 {
            return 1;
        }
        let flags = self.adapter.get_texture_format_features(format).flags;
        if flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_X4)
            && flags.contains(wgpu::TextureFormatFeatureFlags::MULTISAMPLE_RESOLVE)
        {
            4
        } else {
            1
        }
    }

    fn ensure_pipeline(&mut self, format: wgpu::TextureFormat, sample_count: u32) {
        let key = (sample_count, format.is_srgb());
        if self
            .pipelines
            .get(&key)
            .is_some_and(|(fmt, _)| *fmt == format)
        {
            return;
        }

        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<GpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 2,
                },
            ],
        };

        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("vexio-canvas-pipeline"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_layout],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: sample_count,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            });

        self.pipelines.insert(key, (format, pipeline));
    }

    fn lookup_pipeline(&self, format: wgpu::TextureFormat, sample_count: u32) -> &wgpu::RenderPipeline {
        let key = (sample_count, format.is_srgb());
        &self.pipelines[&key].1
    }

    fn upload_mesh_and_uniform(&mut self, mesh: &TessellatedMesh, grid_spacing_in: f64) {
        let uniform = self.camera.to_uniform(grid_spacing_in);
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));

        if mesh.is_empty() {
            return;
        }
        let vb_bytes = (mesh.vertices.len() * std::mem::size_of::<GpuVertex>()) as u64;
        if vb_bytes > self.vertex_capacity_bytes {
            let new_cap = vb_bytes.next_power_of_two().max(64 * 1024);
            self.vertex_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vexio-canvas-vb"),
                size: new_cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity_bytes = new_cap;
        }
        self.queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(&mesh.vertices),
        );

        let ib_bytes = (mesh.indices.len() * std::mem::size_of::<u32>()) as u64;
        if ib_bytes > self.index_capacity_bytes {
            let new_cap = ib_bytes.next_power_of_two().max(64 * 1024);
            self.index_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("vexio-canvas-ib"),
                size: new_cap,
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_capacity_bytes = new_cap;
        }
        self.queue
            .write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&mesh.indices));
    }
}

/// Deterministic 4x-RGSS (Rotated Grid Super-Sampling) anti-aliased software rasterizer.
/// Renders the exact same Lyon-tessellated `TessellatedMesh` scene and procedural Visio page grid
/// as `WgpuCanvasRenderer`, guaranteeing crisp thumbnails, Quick Look previews, and CI renders
/// even in sandboxed environments without a hardware GPU adapter.
pub fn render_page_software_rgba(
    doc: &VisioDocument,
    page_id: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::InvalidDimensions { width, height });
    }
    let page = doc.page(page_id).ok_or(RenderError::PageNotFound(page_id))?;
    let mut camera = ViewportCamera::new(width, height, 1.0);
    camera.fit_to_page(page.width_in(), page.height_in());
    render_page_software_rgba_with_camera(doc, page_id, &camera, true)
}

/// Deterministic 4x-RGSS software rasterizer with caller-supplied `ViewportCamera`.
pub fn render_page_software_rgba_with_camera(
    doc: &VisioDocument,
    page_id: u32,
    camera: &ViewportCamera,
    show_grid: bool,
) -> Result<Vec<u8>, RenderError> {
    render_page_software_rgba_with_selection(doc, page_id, camera, show_grid, &[])
}

/// Deterministic 4x-RGSS software rasterizer with caller-supplied `ViewportCamera` and `selected_shapes`.
pub fn render_page_software_rgba_with_selection(
    doc: &VisioDocument,
    page_id: u32,
    camera: &ViewportCamera,
    show_grid: bool,
    selected_shapes: &[u64],
) -> Result<Vec<u8>, RenderError> {
    let width = camera.width_px;
    let height = camera.height_px;
    if width == 0 || height == 0 {
        return Err(RenderError::InvalidDimensions { width, height });
    }

    let mut cache = MeshCache::new();
    let (mesh, _stats, grid_spacing_in) = build_page_scene_mesh(
        doc,
        page_id,
        camera,
        &mut cache,
        show_grid,
        false,
        selected_shapes,
    )?;

    let pasteboard = RgbaColor {
        r: 226,
        g: 232,
        b: 240,
        a: 255,
    };
    let clear = if selected_shapes.is_empty() && !show_grid {
        active_page_background_color(doc, page_id)
    } else {
        pasteboard
    };
    let mut fb = vec![0u8; (width as usize) * (height as usize) * 4];
    for px in fb.chunks_exact_mut(4) {
        px[0] = clear.r;
        px[1] = clear.g;
        px[2] = clear.b;
        px[3] = clear.a;
    }

    rasterize_mesh_rgss4(&mesh, camera, grid_spacing_in as f32, &mut fb);
    Ok(fb)
}

/// Render `page_id` to an `RGBA8` buffer (`width * height * 4`), automatically using hardware `wgpu`
/// when an adapter is available and falling back seamlessly to deterministic 4x-RGSS software rendering.
pub fn render_page_rgba_auto(
    doc: &VisioDocument,
    page_id: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, RenderError> {
    if width == 0 || height == 0 {
        return Err(RenderError::InvalidDimensions { width, height });
    }
    if doc.page(page_id).is_none() {
        return Err(RenderError::PageNotFound(page_id));
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Ok(mut gpu) = WgpuCanvasRenderer::new_headless(4) {
            if let Ok(rgba) = gpu.render_page_offscreen_rgba(doc, page_id, width, height) {
                return Ok(rgba);
            }
        }
    }

    render_page_software_rgba(doc, page_id, width, height)
}

/// True 4x MSAA Rotated-Grid Super-Sampled (`RGSS`) triangle rasterizer + resolve pass
/// matching hardware `wgpu` 4x MSAA + `canvas.wgsl` semantics with zero shared-edge seam bleeding.
fn rasterize_mesh_rgss4(
    mesh: &TessellatedMesh,
    camera: &ViewportCamera,
    grid_spacing_in: f32,
    fb: &mut [u8],
) {
    let w = camera.width_px as i32;
    let h = camera.height_px as i32;
    if w <= 0 || h <= 0 || mesh.is_empty() {
        return;
    }
    let pixel_count = (w as usize) * (h as usize);
    let ppi = camera.effective_ppi() as f32;

    // Initialize 4x MSAA sample buffer (`4` packed `[u8; 4]` samples per pixel) from `fb` clear color.
    let init_px = [fb[0], fb[1], fb[2], fb[3]];
    let mut msaa_buf: Vec<[[u8; 4]; 4]> = vec![[init_px; 4]; pixel_count];

    // Pre-project all vertices to screen pixels once.
    let mut screen_pts: Vec<[f32; 2]> = Vec::with_capacity(mesh.vertices.len());
    for v in &mesh.vertices {
        let sp = camera.page_in_to_screen(v.pos[0] as f64, v.pos[1] as f64);
        screen_pts.push([sp[0] as f32, sp[1] as f32]);
    }

    // 4x Rotated Grid Super-Sampling offsets relative to pixel center `(px + 0.5, py + 0.5)`.
    const RGSS_OFFSETS: [[f32; 2]; 4] = [
        [-0.125, -0.375],
        [0.375, -0.125],
        [-0.375, 0.125],
        [0.125, 0.375],
    ];

    for tri in mesh.indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if i0 >= mesh.vertices.len() || i1 >= mesh.vertices.len() || i2 >= mesh.vertices.len() {
            continue;
        }
        let p0 = screen_pts[i0];
        let p1 = screen_pts[i1];
        let p2 = screen_pts[i2];

        let area = edge_fn(p0, p1, p2);
        if area.abs() < 1e-5 || !area.is_finite() {
            continue;
        }
        // Normalize winding to positive `edge_fn` area in screen space (`+X` right, `+Y` down).
        let (idx0, idx1, idx2, s0, s1, s2, inv_area) = if area > 0.0 {
            (i0, i1, i2, p0, p1, p2, 1.0 / area)
        } else {
            (i0, i2, i1, p0, p2, p1, -1.0 / area)
        };

        let min_x = (s0[0].min(s1[0]).min(s2[0]).floor() as i32).max(0);
        let max_x = (s0[0].max(s1[0]).max(s2[0]).ceil() as i32).min(w - 1);
        let min_y = (s0[1].min(s1[1]).min(s2[1]).floor() as i32).max(0);
        let max_y = (s0[1].max(s1[1]).max(s2[1]).ceil() as i32).min(h - 1);
        if min_x > max_x || min_y > max_y {
            continue;
        }

        let v0 = mesh.vertices[idx0];
        let v1 = mesh.vertices[idx1];
        let v2 = mesh.vertices[idx2];
        let uniform_color = v0.color == v1.color && v1.color == v2.color;
        let is_page_grid = v0.flags[0].abs() < 0.5;

        // Top-left shared-edge tie-break flags so adjacent Lyon triangles partition edge samples 1:1.
        let tl0 = is_top_left_edge(s1, s2);
        let tl1 = is_top_left_edge(s2, s0);
        let tl2 = is_top_left_edge(s0, s1);

        let band0 = (s2[1] - s1[1]).abs() + (s2[0] - s1[0]).abs();
        let band1 = (s0[1] - s2[1]).abs() + (s0[0] - s2[0]).abs();
        let band2 = (s1[1] - s0[1]).abs() + (s1[0] - s0[0]).abs();

        for py in min_y..=max_y {
            let cy = py as f32 + 0.5;
            let row_px_base = (py as usize) * (w as usize);
            for px in min_x..=max_x {
                let cx = px as f32 + 0.5;
                let center = [cx, cy];
                let e0 = edge_fn(s1, s2, center);
                let e1 = edge_fn(s2, s0, center);
                let e2 = edge_fn(s0, s1, center);

                if e0 < -band0 || e1 < -band1 || e2 < -band2 {
                    continue;
                }

                let sample_mask: u8 = if e0 > band0 && e1 > band1 && e2 > band2 {
                    0b1111
                } else {
                    let mut mask = 0u8;
                    for (si, off) in RGSS_OFFSETS.iter().enumerate() {
                        let sp = [cx + off[0], cy + off[1]];
                        let se0 = edge_fn(s1, s2, sp);
                        let se1 = edge_fn(s2, s0, sp);
                        let se2 = edge_fn(s0, s1, sp);
                        if (se0 > 0.0 || (se0 == 0.0 && tl0))
                            && (se1 > 0.0 || (se1 == 0.0 && tl1))
                            && (se2 > 0.0 || (se2 == 0.0 && tl2))
                        {
                            mask |= 1 << si;
                        }
                    }
                    if mask == 0 {
                        continue;
                    }
                    mask
                };

                let mut col = if uniform_color {
                    v0.color
                } else {
                    let w0 = (e0 * inv_area).clamp(0.0, 1.0);
                    let w1 = (e1 * inv_area).clamp(0.0, 1.0);
                    let w2 = (1.0 - w0 - w1).clamp(0.0, 1.0);
                    [
                        v0.color[0] * w0 + v1.color[0] * w1 + v2.color[0] * w2,
                        v0.color[1] * w0 + v1.color[1] * w1 + v2.color[1] * w2,
                        v0.color[2] * w0 + v1.color[2] * w1 + v2.color[2] * w2,
                        v0.color[3] * w0 + v1.color[3] * w1 + v2.color[3] * w2,
                    ]
                };

                if is_page_grid && grid_spacing_in > 0.0 {
                    let page_pt = camera.screen_to_page_in(cx as f64, cy as f64);
                    let rgb = software_grid_tint(
                        [col[0], col[1], col[2]],
                        [page_pt[0] as f32, page_pt[1] as f32],
                        grid_spacing_in,
                        ppi,
                    );
                    col[0] = rgb[0];
                    col[1] = rgb[1];
                    col[2] = rgb[2];
                }

                let src_a = col[3].clamp(0.0, 1.0);
                if src_a <= 1.0 / 512.0 {
                    continue;
                }

                let samples = &mut msaa_buf[row_px_base + (px as usize)];
                if src_a >= 0.999 {
                    let packed = [
                        (col[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                        (col[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                        (col[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                        255,
                    ];
                    if sample_mask == 0b1111 {
                        *samples = [packed; 4];
                    } else {
                        for (si, slot) in samples.iter_mut().enumerate() {
                            if (sample_mask & (1 << si)) != 0 {
                                *slot = packed;
                            }
                        }
                    }
                } else {
                    for (si, slot) in samples.iter_mut().enumerate() {
                        if (sample_mask & (1 << si)) == 0 {
                            continue;
                        }
                        let dst_r = slot[0] as f32 / 255.0;
                        let dst_g = slot[1] as f32 / 255.0;
                        let dst_b = slot[2] as f32 / 255.0;
                        let dst_a = slot[3] as f32 / 255.0;
                        let out_a = src_a + dst_a * (1.0 - src_a);
                        let inv_out = if out_a > 1e-6 { 1.0 / out_a } else { 0.0 };
                        let out_r = (col[0] * src_a + dst_r * dst_a * (1.0 - src_a)) * inv_out;
                        let out_g = (col[1] * src_a + dst_g * dst_a * (1.0 - src_a)) * inv_out;
                        let out_b = (col[2] * src_a + dst_b * dst_a * (1.0 - src_a)) * inv_out;
                        *slot = [
                            (out_r.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                            (out_g.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                            (out_b.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                            (out_a.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
                        ];
                    }
                }
            }
        }
    }

    // Resolve 4x MSAA sample buffer into tight RGBA8 output framebuffer.
    for (px_slots, out_px) in msaa_buf.iter().zip(fb.chunks_exact_mut(4)) {
        let r = (px_slots[0][0] as u32
            + px_slots[1][0] as u32
            + px_slots[2][0] as u32
            + px_slots[3][0] as u32
            + 2)
            >> 2;
        let g = (px_slots[0][1] as u32
            + px_slots[1][1] as u32
            + px_slots[2][1] as u32
            + px_slots[3][1] as u32
            + 2)
            >> 2;
        let b = (px_slots[0][2] as u32
            + px_slots[1][2] as u32
            + px_slots[2][2] as u32
            + px_slots[3][2] as u32
            + 2)
            >> 2;
        let a = (px_slots[0][3] as u32
            + px_slots[1][3] as u32
            + px_slots[2][3] as u32
            + px_slots[3][3] as u32
            + 2)
            >> 2;
        out_px[0] = r as u8;
        out_px[1] = g as u8;
        out_px[2] = b as u8;
        out_px[3] = a as u8;
    }
}

#[inline]
fn is_top_left_edge(a: [f32; 2], b: [f32; 2]) -> bool {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dy == 0.0 && dx < 0.0) || dy < 0.0
}

#[inline]
fn edge_fn(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> f32 {
    (p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])
}

#[inline]
fn smoothstep_f32(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// CPU mirror of `compute_grid_tint` in `canvas.wgsl`.
fn software_grid_tint(
    base_rgb: [f32; 3],
    page_pos_in: [f32; 2],
    grid_spacing_in: f32,
    zoom_ppi: f32,
) -> [f32; 3] {
    let spacing = grid_spacing_in.max(0.0001);
    let minor_px = spacing * zoom_ppi;
    let major_spacing = spacing * 4.0;
    let major_px = major_spacing * zoom_ppi;

    let mut rgb = base_rgb;

    if grid_spacing_in > 0.0 && minor_px >= 6.0 {
        let minor_fade = ((minor_px - 6.0) / 8.0).clamp(0.0, 1.0);
        let ux = (page_pos_in[0] / spacing - 0.5).rem_euclid(1.0) - 0.5;
        let uy = (page_pos_in[1] / spacing - 0.5).rem_euclid(1.0) - 0.5;
        let dist_minor_px = ux.abs().min(uy.abs()) * minor_px;
        let minor_alpha = (1.0 - smoothstep_f32(0.0, 1.05, dist_minor_px)) * 0.35 * minor_fade;
        let mc = [0.84, 0.88, 0.94];
        rgb = [
            rgb[0] + (mc[0] - rgb[0]) * minor_alpha,
            rgb[1] + (mc[1] - rgb[1]) * minor_alpha,
            rgb[2] + (mc[2] - rgb[2]) * minor_alpha,
        ];
    }

    if grid_spacing_in > 0.0 && major_px >= 8.0 {
        let major_fade = ((major_px - 8.0) / 10.0).clamp(0.0, 1.0);
        let ux = (page_pos_in[0] / major_spacing - 0.5).rem_euclid(1.0) - 0.5;
        let uy = (page_pos_in[1] / major_spacing - 0.5).rem_euclid(1.0) - 0.5;
        let dist_major_px = ux.abs().min(uy.abs()) * major_px;
        let major_alpha = (1.0 - smoothstep_f32(0.0, 1.15, dist_major_px)) * 0.55 * major_fade;
        let mc = [0.74, 0.80, 0.89];
        rgb = [
            rgb[0] + (mc[0] - rgb[0]) * major_alpha,
            rgb[1] + (mc[1] - rgb[1]) * major_alpha,
            rgb[2] + (mc[2] - rgb[2]) * major_alpha,
        ];
    }

    rgb
}

fn emit_rect_outline_quads(
    mesh: &mut TessellatedMesh,
    min: [f32; 2],
    max: [f32; 2],
    thickness: f32,
    color: [f32; 4],
) {
    let half = thickness * 0.5;
    let flags = [PRIM_FLAG_SHAPE, 0.0];
    mesh.push_quad(
        [min[0] - half, min[1] - half],
        [max[0] + half, min[1] + half],
        color,
        flags,
    );
    mesh.push_quad(
        [min[0] - half, max[1] - half],
        [max[0] + half, max[1] + half],
        color,
        flags,
    );
    mesh.push_quad(
        [min[0] - half, min[1] - half],
        [min[0] + half, max[1] + half],
        color,
        flags,
    );
    mesh.push_quad(
        [max[0] - half, min[1] - half],
        [max[0] + half, max[1] + half],
        color,
        flags,
    );
}

fn create_msaa_view(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sample_count: u32,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("vexio-msaa-attachment"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

fn page_grid_spacing_in(page: &VisioPage) -> f64 {
    page.page_sheet
        .get_f64("XGridSpacing")
        .or_else(|| page.page_sheet.get_f64("YGridSpacing"))
        .filter(|v| v.is_finite() && *v > 0.0)
        .unwrap_or(0.25)
}

fn active_page_background_color(doc: &VisioDocument, page_id: u32) -> RgbaColor {
    if let Some(page) = doc
        .page(page_id)
        .or_else(|| doc.pages.iter().find(|p| !p.is_background))
        .or_else(|| doc.pages.first())
    {
        if let Some(c) = page.page_sheet.get_color("PageColor") {
            return c;
        }
    }
    if let Some(tid) = doc.active_theme_id {
        if let Some(theme) = doc.themes.iter().find(|t| t.id == tid) {
            return theme.background_color;
        }
    }
    RgbaColor::WHITE
}

#[inline]
fn to_wgpu_color(c: RgbaColor) -> wgpu::Color {
    wgpu::Color {
        r: c.r as f64 / 255.0,
        g: c.g as f64 / 255.0,
        b: c.b as f64 / 255.0,
        a: c.a as f64 / 255.0,
    }
}

#[inline]
fn sanitize_positive(v: f64, fallback: f64) -> f64 {
    if v.is_finite() && v > 0.0 {
        v
    } else {
        fallback
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn monotonic_now_ms() -> f64 {
    use std::sync::OnceLock;
    use std::time::Instant;
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let start = EPOCH.get_or_init(Instant::now);
    start.elapsed().as_secs_f64() * 1000.0
}

#[cfg(target_arch = "wasm32")]
fn monotonic_now_ms() -> f64 {
    if let Some(win) = web_sys::window() {
        if let Some(perf) = win.performance() {
            return perf.now();
        }
    }
    0.0
}
