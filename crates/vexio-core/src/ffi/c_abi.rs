//! C-ABI FFI Implementation (`crates/vexio-core/include/vexio.h`) for Windows WinUI 3,
//! macOS Swift / Quick Look, and native C++ consumers.

use crate::boolean_ops::{draw_freeform_curve, list_control_handles};
use crate::domains::list_builtin_stencil_previews;
use crate::export_print::export_page_svg;
use crate::journal::{DocCommand, JournalError};
use crate::routing::route_page_connectors;
use crate::workspace::{CoreError, DocumentSession};
use serde::Serialize;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use vexio_shapesheet::{
    MarkupInkPoint, MarkupInkStroke, ReactiveEngine, RgbaColor, ShapeKind, VisioDocument,
    format_clean_f64,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VexioStatus {
    Ok = 0,
    ErrInvalidArg = 1,
    ErrCellGuarded = 2,
    ErrFormulaSyntax = 3,
    ErrIoCorruptFormat = 4,
    ErrGpuSurfaceLost = 5,
    ErrInternalPanic = 99,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VexioByteSlice {
    pub ptr: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VexioOwnedBuffer {
    pub ptr: *mut u8,
    pub len: usize,
    pub cap: usize,
}

impl VexioOwnedBuffer {
    pub fn from_vec(mut v: Vec<u8>) -> Self {
        let len = v.len();
        let cap = v.capacity();
        let ptr = v.as_mut_ptr();
        std::mem::forget(v);
        Self { ptr, len, cap }
    }
}

thread_local! {
    static LAST_FFI_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

fn set_last_ffi_error(msg: impl Into<String>) {
    let s = msg.into();
    LAST_FFI_ERROR.with(|slot| {
        *slot.borrow_mut() = s;
    });
}

fn panic_payload_to_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unexpected non-string panic payload".to_string()
    }
}

fn catch_ffi_status<F>(f: F) -> VexioStatus
where
    F: FnOnce() -> VexioStatus,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(status) => status,
        Err(payload) => {
            set_last_ffi_error(format!("Rust panic: {}", panic_payload_to_string(&*payload)));
            VexioStatus::ErrInternalPanic
        }
    }
}

fn map_core_err(e: CoreError) -> VexioStatus {
    set_last_ffi_error(e.to_string());
    match e {
        CoreError::Journal(crate::journal::JournalError::CellGuarded(_)) => {
            VexioStatus::ErrCellGuarded
        }
        CoreError::Journal(_) => VexioStatus::ErrInvalidArg,
        _ => VexioStatus::ErrIoCorruptFormat,
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VexioPointerEvent {
    pub kind: u32,
    pub buttons: u32,
    pub modifiers: u32,
    pub x_px: f64,
    pub y_px: f64,
    pub pressure: f32,
    pub tilt_x_deg: f32,
    pub tilt_y_deg: f32,
    pub timestamp_us: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCanvasTool {
    Select = 0,
    Pan = 1,
    Connector = 2,
    Text = 3,
    Rectangle = 4,
    RoundedRect = 5,
    Ellipse = 6,
    Diamond = 7,
    Triangle = 8,
    Line = 9,
    Arc = 10,
    Freeform = 11,
    InkPen = 12,
    Highlighter = 13,
    Hexagon = 14,
    Star = 15,
    EditVertices = 16,
    InkEraser = 17,
}

impl NativeCanvasTool {
    pub fn from_code(code: u32) -> Self {
        match code {
            1 => Self::Pan,
            2 => Self::Connector,
            3 => Self::Text,
            4 => Self::Rectangle,
            5 => Self::RoundedRect,
            6 => Self::Ellipse,
            7 => Self::Diamond,
            8 => Self::Triangle,
            9 => Self::Line,
            10 => Self::Arc,
            11 => Self::Freeform,
            12 => Self::InkPen,
            13 => Self::Highlighter,
            14 => Self::Hexagon,
            15 => Self::Star,
            16 => Self::EditVertices,
            17 => Self::InkEraser,
            _ => Self::Select,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeResizeHandle {
    NW,
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
}

impl NativeResizeHandle {
    pub fn cursor_code(self) -> u32 {
        match self {
            Self::N | Self::S => 5,
            Self::E | Self::W => 6,
            Self::NW | Self::SE => 11,
            Self::NE | Self::SW => 12,
        }
    }

    pub fn compute_box(
        self,
        orig_pin: [f64; 2],
        orig_size: [f64; 2],
        dx: f64,
        dy: f64,
        lock_aspect: bool,
    ) -> ([f64; 2], [f64; 2]) {
        let mut left = orig_pin[0] - orig_size[0] * 0.5;
        let mut right = orig_pin[0] + orig_size[0] * 0.5;
        let mut bottom = orig_pin[1] - orig_size[1] * 0.5;
        let mut top = orig_pin[1] + orig_size[1] * 0.5;

        match self {
            Self::W | Self::NW | Self::SW => left = (left + dx).min(right - 0.15),
            Self::E | Self::NE | Self::SE => right = (right + dx).max(left + 0.15),
            _ => {}
        }
        match self {
            Self::S | Self::SW | Self::SE => bottom = (bottom + dy).min(top - 0.15),
            Self::N | Self::NW | Self::NE => top = (top + dy).max(bottom + 0.15),
            _ => {}
        }

        let mut w = (right - left).max(0.15);
        let mut h = (top - bottom).max(0.15);
        if lock_aspect && orig_size[1] > 1e-6 && matches!(self, Self::NW | Self::NE | Self::SE | Self::SW) {
            let aspect = orig_size[0] / orig_size[1];
            h = (w / aspect).max(0.15);
            if matches!(self, Self::NW | Self::NE) {
                top = bottom + h;
            } else {
                bottom = top - h;
            }
            w = (right - left).max(0.15);
        }
        ([(left + right) * 0.5, (bottom + top) * 0.5], [w, h])
    }
}

fn hit_test_selection_grips(
    visuals: &[vexio_shapesheet::EvaluatedShapeVisual],
    selected_id: u64,
    page_pt: [f64; 2],
    logical_ppi: f64,
) -> Option<Result<NativeResizeHandle, ()>> {
    let vis = visuals.iter().find(|v| v.shape_id == selected_id)?;
    if !matches!(
        vis.kind,
        ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
    ) {
        return None;
    }
    let tf = &vis.transform;
    let ppi = logical_ppi.max(1.0);

    // Exact match to `tessellate_selection_handles` (`rot_stem_len = 28.0 / ppi`, `rot_radius = 11.0 / ppi`)
    // using `vis.transform.local_to_parent(...)` so non-centered `LocPinX`/`LocPinY`, `FlipX`/`FlipY`,
    // master-inherited dimensions, and rotated `Group` stencils hit with 0.00 px discrepancy!
    let rot_stem_len = 28.0 / ppi;
    let up_pt: [f64; 2] = tf
        .local_to_parent(tf.width * 0.5, tf.height + rot_stem_len)
        .into();
    // Generous 18 dp (`36 dp` diameter) hit target around the `22 dp` circular `↻` button + stem!
    let rot_hit_radius = 18.0 / ppi;
    if (page_pt[0] - up_pt[0]).hypot(page_pt[1] - up_pt[1]) <= rot_hit_radius {
        return Some(Err(())); // Rotation button (`↻`)!
    }
    let (lx, ly) = tf.parent_to_local(page_pt[0], page_pt[1]);
    if (lx - tf.width * 0.5).abs() <= 11.0 / ppi
        && ly >= tf.height + 7.5 / ppi
        && ly <= tf.height + rot_stem_len + rot_hit_radius
    {
        return Some(Err(())); // Rotation stem directly beneath `↻`!
    }

    let c00: [f64; 2] = tf.local_to_parent(0.0, 0.0).into();
    let c10: [f64; 2] = tf.local_to_parent(tf.width, 0.0).into();
    let c11: [f64; 2] = tf.local_to_parent(tf.width, tf.height).into();
    let c01: [f64; 2] = tf.local_to_parent(0.0, tf.height).into();
    let top_mid = [(c01[0] + c11[0]) * 0.5, (c01[1] + c11[1]) * 0.5];
    let bot_mid = [(c00[0] + c10[0]) * 0.5, (c00[1] + c10[1]) * 0.5];
    let left_mid = [(c00[0] + c01[0]) * 0.5, (c00[1] + c01[1]) * 0.5];
    let right_mid = [(c10[0] + c11[0]) * 0.5, (c10[1] + c11[1]) * 0.5];

    let tol_in = 11.0 / ppi;
    let grips = [
        (NativeResizeHandle::SW, c00),
        (NativeResizeHandle::SE, c10),
        (NativeResizeHandle::NE, c11),
        (NativeResizeHandle::NW, c01),
        (NativeResizeHandle::N, top_mid),
        (NativeResizeHandle::S, bot_mid),
        (NativeResizeHandle::W, left_mid),
        (NativeResizeHandle::E, right_mid),
    ];
    for (dir, gp) in grips {
        if (page_pt[0] - gp[0]).hypot(page_pt[1] - gp[1]) <= tol_in {
            return Some(Ok(dir));
        }
    }
    None
}

#[derive(Debug, Clone, Copy)]
enum Connector1DGripHit {
    BeginEndpoint([f64; 2]),
    EndEndpoint([f64; 2]),
    InteriorSegment {
        seg_index: usize,
        is_vertical: bool,
        coord_in: f64,
    },
}

fn hit_test_connector_1d_grips(
    visuals: &[vexio_shapesheet::EvaluatedShapeVisual],
    shape_id: u64,
    page_pt: [f64; 2],
    eff_ppi: f64,
) -> Option<Connector1DGripHit> {
    let vis = visuals.iter().find(|v| v.shape_id == shape_id)?;
    if vis.kind != ShapeKind::Connector1D {
        return None;
    }
    let poly = vexio_render::tessellate::extract_connector_polyline_in(vis);
    if poly.len() < 2 {
        return None;
    }
    let tol_in = (9.5 / eff_ppi.max(12.0)).max(0.055);
    let p_beg = poly[0];
    let p_end = *poly.last().unwrap_or(&p_beg);
    if (page_pt[0] - p_beg[0]).hypot(page_pt[1] - p_beg[1]) <= tol_in {
        return Some(Connector1DGripHit::BeginEndpoint(p_beg));
    }
    if (page_pt[0] - p_end[0]).hypot(page_pt[1] - p_end[1]) <= tol_in {
        return Some(Connector1DGripHit::EndEndpoint(p_end));
    }
    if poly.len() >= 3 {
        let seg_count = poly.len() - 1;
        for seg_idx in 0..seg_count {
            let is_outer = seg_idx == 0 || seg_idx + 1 == seg_count;
            let min_seg_in = if is_outer {
                (28.0 / eff_ppi.max(12.0)).max(0.28)
            } else {
                (8.0 / eff_ppi.max(12.0)).max(0.08)
            };
            let a = poly[seg_idx];
            let b = poly[seg_idx + 1];
            let dx = (b[0] - a[0]).abs();
            let dy = (b[1] - a[1]).abs();
            let is_horiz = dy < 1e-3 && dx >= min_seg_in;
            let is_vert = dx < 1e-3 && dy >= min_seg_in;
            if !is_horiz && !is_vert {
                continue;
            }
            let mx = (a[0] + b[0]) * 0.5;
            let my = (a[1] + b[1]) * 0.5;
            if (page_pt[0] - mx).hypot(page_pt[1] - my) <= tol_in {
                return Some(Connector1DGripHit::InteriorSegment {
                    seg_index: seg_idx,
                    is_vertical: is_vert,
                    coord_in: if is_vert { mx } else { my },
                });
            }
        }
    }
    None
}

fn hit_test_control_handles(
    visuals: &[vexio_shapesheet::EvaluatedShapeVisual],
    shape_id: u64,
    page_pt: [f64; 2],
    logical_ppi: f64,
) -> Option<(String, [f64; 2])> {
    let vis = visuals.iter().find(|v| v.shape_id == shape_id)?;
    if vis.control_handles.is_empty() {
        return None;
    }
    let tol_in = (11.5 / logical_ppi.max(1.0)).max(0.055);
    for ch in &vis.control_handles {
        if (page_pt[0] - ch.pos_page_in[0]).hypot(page_pt[1] - ch.pos_page_in[1]) <= tol_in {
            return Some((ch.name.clone(), ch.pos_page_in));
        }
    }
    None
}

fn hit_test_autoconnect_triangles(
    visuals: &[vexio_shapesheet::EvaluatedShapeVisual],
    shape_id: u64,
    page_pt: [f64; 2],
    logical_ppi: f64,
) -> Option<(vexio_render::AutoConnectDir, [f64; 2])> {
    let vis = visuals.iter().find(|v| v.shape_id == shape_id)?;
    if !matches!(
        vis.kind,
        ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
    ) || vis.is_swimlane
    {
        return None;
    }
    let ppi = logical_ppi.max(1.0);
    let tol_in = (11.5 / ppi).max(0.055);
    for dir in vexio_render::AutoConnectDir::ALL {
        let (center, _) = vexio_render::autoconnect_arrow_center_in(&vis.transform, dir, ppi);
        if (page_pt[0] - center[0]).hypot(page_pt[1] - center[1]) <= tol_in {
            let edge_local = match dir {
                vexio_render::AutoConnectDir::North => (vis.transform.width * 0.5, vis.transform.height),
                vexio_render::AutoConnectDir::East => (vis.transform.width, vis.transform.height * 0.5),
                vexio_render::AutoConnectDir::South => (vis.transform.width * 0.5, 0.0),
                vexio_render::AutoConnectDir::West => (0.0, vis.transform.height * 0.5),
            };
            let port_start: [f64; 2] = vis.transform.local_to_parent(edge_local.0, edge_local.1).into();
            return Some((dir, port_start));
        }
    }
    None
}

fn translate_preview_shapes_in_doc(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_ids: &[u64],
    dx_in: f64,
    dy_in: f64,
) {
    if dx_in.abs() < 1e-8 && dy_in.abs() < 1e-8 {
        return;
    }
    let expanded = crate::containers::expand_move_set_with_container_and_swimlane_members(
        doc,
        page_id,
        shape_ids,
    );
    let id_set: std::collections::BTreeSet<u64> = expanded.into_iter().collect();
    if let Some(page) = doc.page_mut(page_id) {
        let connects_snap = page.connects.clone();
        for sh in &mut page.shapes {
            if !id_set.contains(&sh.id) {
                continue;
            }
            let eff_dx = if sh.sheet.get_bool("LockMoveX").unwrap_or(false) {
                0.0
            } else {
                dx_in
            };
            let eff_dy = if sh.sheet.get_bool("LockMoveY").unwrap_or(false) {
                0.0
            } else {
                dy_in
            };
            if eff_dx.abs() < 1e-8 && eff_dy.abs() < 1e-8 {
                continue;
            }
            if sh.kind == ShapeKind::Connector1D {
                let beg_glued = connects_snap.iter().any(|c| {
                    c.connector_shape_id == sh.id
                        && c.from_cell.to_ascii_lowercase().starts_with("begin")
                });
                let end_glued = connects_snap.iter().any(|c| {
                    c.connector_shape_id == sh.id
                        && !c.from_cell.to_ascii_lowercase().starts_with("begin")
                });
                if !beg_glued {
                    let bx = sh.sheet.get_f64("BeginX").unwrap_or(0.0) + eff_dx;
                    let by = sh.sheet.get_f64("BeginY").unwrap_or(0.0) + eff_dy;
                    let _ = sh.sheet.try_set_formula_respecting_guard(
                        "BeginX",
                        format!("{} in", format_clean_f64(bx)),
                        false,
                    );
                    let _ = sh.sheet.try_set_formula_respecting_guard(
                        "BeginY",
                        format!("{} in", format_clean_f64(by)),
                        false,
                    );
                }
                if !end_glued {
                    let ex = sh.sheet.get_f64("EndX").unwrap_or(1.0) + eff_dx;
                    let ey = sh.sheet.get_f64("EndY").unwrap_or(0.0) + eff_dy;
                    let _ = sh.sheet.try_set_formula_respecting_guard(
                        "EndX",
                        format!("{} in", format_clean_f64(ex)),
                        false,
                    );
                    let _ = sh.sheet.try_set_formula_respecting_guard(
                        "EndY",
                        format!("{} in", format_clean_f64(ey)),
                        false,
                    );
                }
                if let Some(raw_pts) = sh
                    .sheet
                    .get_cell("User.ManualOrthoPoints")
                    .and_then(|c| c.value.as_str_coerced().ok())
                {
                    let mut mpts = crate::routing::parse_manual_ortho_points(&raw_pts);
                    if mpts.len() >= 2 {
                        for p in &mut mpts {
                            p[0] += eff_dx;
                            p[1] += eff_dy;
                        }
                        sh.sheet.set_user_cell(
                            "ManualOrthoPoints",
                            crate::routing::format_manual_ortho_points(&mpts),
                            "User-adjusted multi-bend orthogonal waypoints",
                        );
                    }
                }
            } else {
                let px = sh.sheet.get_f64("PinX").unwrap_or(0.0) + eff_dx;
                let py = sh.sheet.get_f64("PinY").unwrap_or(0.0) + eff_dy;
                let _ = sh.sheet.try_set_formula_respecting_guard(
                    "PinX",
                    format!("{} in", format_clean_f64(px)),
                    false,
                );
                let _ = sh.sheet.try_set_formula_respecting_guard(
                    "PinY",
                    format!("{} in", format_clean_f64(py)),
                    false,
                );
            }
        }
    }
    route_page_connectors(doc, page_id);
}

#[derive(Debug, Clone)]
pub enum NativeDragState {
    Pan {
        last_x_px: f64,
        last_y_px: f64,
    },
    MarqueeSelect {
        start_page: [f64; 2],
        cur_page: [f64; 2],
        additive: bool,
        prior_ids: Vec<u64>,
    },
    MoveShapes {
        shape_ids: Vec<u64>,
        start_page: [f64; 2],
        applied_delta: [f64; 2],
        orig_union_min: [f64; 2],
        orig_union_max: [f64; 2],
        moved: bool,
    },
    ResizeShape {
        shape_id: u64,
        handle: NativeResizeHandle,
        start_page: [f64; 2],
        orig_pin: [f64; 2],
        orig_size: [f64; 2],
        orig_angle_deg: f64,
        cur_pin: [f64; 2],
        cur_size: [f64; 2],
    },
    RotateShape {
        shape_id: u64,
        start_page: [f64; 2],
        orig_pin: [f64; 2],
        orig_size: [f64; 2],
        orig_angle_deg: f64,
        cur_angle_deg: f64,
        dragged: bool,
    },
    DragConnectorEndpoint {
        shape_id: u64,
        is_begin: bool,
        cur_page: [f64; 2],
        target_shape_id: Option<u64>,
        target_port_cell: Option<String>,
    },
    DragConnectorSegment {
        shape_id: u64,
        seg_index: usize,
        is_vertical: bool,
        cur_coord_in: f64,
        base_poly: Vec<[f64; 2]>,
        orig_manual_pts: Option<String>,
    },
    DrawBoxOrLine {
        tool: NativeCanvasTool,
        start_page: [f64; 2],
        from_shape_id: Option<u64>,
        from_port_cell: Option<String>,
    },
    DrawPath {
        tool: NativeCanvasTool,
        points: Vec<MarkupInkPoint>,
    },
    DragControlHandle {
        shape_id: u64,
        row_name: String,
        orig_x: String,
        orig_y: String,
        orig_xdyn: String,
        orig_ydyn: String,
        cur_page: [f64; 2],
    },
    AutoConnectGesture {
        source_id: u64,
        dir: vexio_render::AutoConnectDir,
        start_page: [f64; 2],
        port_start_in: [f64; 2],
        cur_page: [f64; 2],
        dragged: bool,
        target_shape_id: Option<u64>,
        target_port_cell: Option<String>,
    },
    DragShapeVertex {
        shape_id: u64,
        geom_index: u32,
        row_ix: u32,
        orig_x: String,
        orig_y: String,
        cur_page: [f64; 2],
    },
}

pub struct VexioEngine {
    pub initialized: bool,
}

pub struct VexioDoc {
    pub session: DocumentSession,
}

pub(crate) fn fit_camera_to_diagram(
    camera: &mut vexio_render::ViewportCamera,
    doc: &VisioDocument,
    page_id: u32,
) {
    let page = doc
        .page(page_id)
        .or_else(|| doc.pages.iter().find(|p| !p.is_background))
        .or_else(|| doc.pages.first());
    let pw = page.map(|p| p.width_in()).unwrap_or(11.0).max(0.5);
    let ph = page.map(|p| p.height_in()).unwrap_or(8.5).max(0.5);

    let mut min_x = 0.0_f64;
    let mut min_y = 0.0_f64;
    let mut max_x = pw;
    let mut max_y = ph;

    if let Some(p) = page {
        let visuals = vexio_shapesheet::ReactiveEngine::evaluate_page_visuals(doc, p.id);
        for v in &visuals {
            if v.aabb_min[0].is_finite()
                && v.aabb_min[1].is_finite()
                && v.aabb_max[0].is_finite()
                && v.aabb_max[1].is_finite()
                && (v.aabb_max[0] - v.aabb_min[0]) < 500.0
                && (v.aabb_max[1] - v.aabb_min[1]) < 500.0
            {
                min_x = min_x.min(v.aabb_min[0]);
                min_y = min_y.min(v.aabb_min[1]);
                max_x = max_x.max(v.aabb_max[0]);
                max_y = max_y.max(v.aabb_max[1]);
            }
        }
    }

    let span_w = (max_x - min_x).max(0.5);
    let span_h = (max_y - min_y).max(0.5);
    let dpi = camera.dpi_scale.max(0.5);
    let logical_w = (camera.width_px.max(1) as f64) / dpi;
    let logical_h = (camera.height_px.max(1) as f64) / dpi;

    // Reserve clean breathing room plus clearance for floating left tool palette and bottom pills.
    let left_pad_pt = if logical_w > 420.0 { 88.0 } else { logical_w * 0.05 };
    let right_pad_pt = (logical_w * 0.025).clamp(22.0, 40.0).min(logical_w * 0.08);
    let top_pad_pt = (logical_h * 0.03).clamp(18.0, 34.0).min(logical_h * 0.08);
    let bottom_pad_pt = if logical_h > 320.0 { 50.0 } else { logical_h * 0.05 };

    let avail_w_pt = (logical_w - left_pad_pt - right_pad_pt).max(logical_w * 0.55).max(1.0);
    let avail_h_pt = (logical_h - top_pad_pt - bottom_pad_pt).max(logical_h * 0.55).max(1.0);
    let ppi = (avail_w_pt / span_w).min(avail_h_pt / span_h).clamp(1.0, 25_000.0);

    camera.zoom_ppi = ppi;
    // Shift camera center slightly left/down in page space so diagram sits centered in the unobstructed region!
    let horiz_bias_in = if logical_w > 420.0 {
        (left_pad_pt - right_pad_pt) * 0.5 / ppi
    } else {
        0.0
    };
    let vert_bias_in = if logical_h > 320.0 {
        (bottom_pad_pt - top_pad_pt) * 0.5 / ppi
    } else {
        0.0
    };
    camera.center_x_in = (min_x + max_x) * 0.5 - horiz_bias_in;
    camera.center_y_in = (min_y + max_y) * 0.5 - vert_bias_in;
}

pub struct VexioViewport {
    pub page_id: u32,
    pub camera: vexio_render::ViewportCamera,
    pub gpu_renderer: Option<vexio_render::WgpuCanvasRenderer>,
    pub active_tool: NativeCanvasTool,
    pub selected_ids: Vec<u64>,
    pub drag_state: Option<NativeDragState>,
    pub last_created_text_shape_id: Option<u64>,
    pub smart_guides_enabled: bool,
    pub pending_fit: bool,
    pub overlay: vexio_render::InteractiveCanvasOverlay,
    /// Retained Android `ANativeWindow*` released on `Drop` when created via `jni_bridge`.
    pub android_native_window: *mut std::ffi::c_void,
}

impl VexioViewport {
    pub fn new_base(doc: &VisioDocument, page_id: u32, width_px: u32, height_px: u32, dpi_scale: f32) -> Self {
        let mut camera = vexio_render::ViewportCamera::new(
            width_px.max(1),
            height_px.max(1),
            dpi_scale.max(0.5) as f64,
        );
        let resolved_page = doc
            .page(page_id)
            .or_else(|| doc.pages.iter().find(|p| !p.is_background))
            .or_else(|| doc.pages.first());
        let resolved_id = resolved_page.map(|p| p.id).unwrap_or(page_id);
        fit_camera_to_diagram(&mut camera, doc, resolved_id);
        let pending_fit = width_px <= 64 || height_px <= 64;
        Self {
            page_id: resolved_id,
            camera,
            gpu_renderer: None,
            active_tool: NativeCanvasTool::Select,
            selected_ids: Vec::new(),
            drag_state: None,
            last_created_text_shape_id: None,
            smart_guides_enabled: true,
            pending_fit,
            overlay: vexio_render::InteractiveCanvasOverlay::default(),
            android_native_window: ptr::null_mut(),
        }
    }

    pub fn sync_glued_ends(&mut self, doc: &VisioDocument) {
        self.overlay.glued_connector_ends.clear();
        if let Some(page) = doc.page(self.page_id) {
            for &sid in &self.selected_ids {
                let beg = page.connects.iter().any(|c| {
                    c.connector_shape_id == sid
                        && c.from_cell.to_ascii_lowercase().starts_with("begin")
                });
                let end = page.connects.iter().any(|c| {
                    c.connector_shape_id == sid
                        && !c.from_cell.to_ascii_lowercase().starts_with("begin")
                });
                if beg || end {
                    self.overlay.glued_connector_ends.insert(sid, (beg, end));
                }
            }
        }
    }

    pub fn clear_transient_overlay(&mut self) {
        self.overlay.marquee_rect_in = None;
        self.overlay.smart_guides.clear();
        self.overlay.highlighted_ports.clear();
        self.overlay.connector_preview_in = None;
        self.overlay.hovered_autoconnect = None;
        self.overlay.container_drop_target_in = None;
        self.overlay.live_stroke_points.clear();
        self.overlay.live_stroke_highlighter = false;
    }

    pub fn present_if_attached(&mut self, doc: &VisioDocument) -> bool {
        if self.pending_fit && self.camera.width_px > 64 && self.camera.height_px > 64 {
            fit_camera_to_diagram(&mut self.camera, doc, self.page_id);
            self.pending_fit = false;
        }
        self.sync_glued_ends(doc);
        if self.active_tool == NativeCanvasTool::EditVertices {
            if let Some(&sid) = self.selected_ids.first() {
                self.overlay.editable_vertices =
                    crate::boolean_ops::list_shape_vertices(doc, self.page_id, sid)
                        .into_iter()
                        .map(|v| [v.page_x_in, v.page_y_in])
                        .collect();
            } else {
                self.overlay.editable_vertices.clear();
            }
        } else {
            self.overlay.editable_vertices.clear();
        }
        let Some(r) = &mut self.gpu_renderer else {
            return false;
        };
        r.camera = self.camera;
        r.selected_shapes = self.selected_ids.clone();
        r.interactive_overlay = self.overlay.clone();
        r.render_surface_frame(doc, self.page_id).is_ok()
    }

    /// Stateful cross-platform pointer / stylus / wheel dispatcher shared by C-ABI and JNI.
    /// Returns cursor code (`0=Arrow`, `1=Move`, `2=Crosshair`, `3=HandGrab`, `5=ResizeNS`,
    /// `6=ResizeWE`, `7=TextIBeam`, `11=ResizeNWSE`, `12=ResizeNESW`, `13=Rotate`).
    pub fn dispatch_pointer_internal(
        &mut self,
        session: &mut DocumentSession,
        ev: VexioPointerEvent,
    ) -> u32 {
        let pid = self.page_id;
        // kind: 0=Move, 1=Down, 2=Up, 3=Wheel, 4=StylusMove
        if ev.kind == 3 {
            let delta = if ev.pressure.abs() > 1e-4 {
                ev.pressure as f64
            } else if ev.tilt_y_deg.abs() > 1e-4 {
                ev.tilt_y_deg as f64
            } else {
                1.0
            };
            let factor = if delta > 0.0 { 1.12 } else { 1.0 / 1.12 };
            self.camera.zoom_around_screen_point(factor, ev.x_px, ev.y_px);
            self.present_if_attached(&session.doc);
            return 0;
        }

        let page_pt = self.camera.screen_to_page_in(ev.x_px, ev.y_px);
        let eff_ppi = self.camera.effective_ppi();
        let logical_ppi = self.camera.zoom_ppi.max(1.0);
        let port_tol_in = (10.0 / logical_ppi.max(12.0)).clamp(0.06, 0.22);
        let snap_tol_in = (7.5 / eff_ppi.max(12.0)).clamp(0.03, 0.14);
        let visuals = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
        let index = vexio_render::SpatialIndex::build(&visuals);
        let raw_top_hit = index
            .hit_test_visuals(&visuals, page_pt[0], page_pt[1], 0.09)
            .into_iter()
            .next();
        let top_hit = raw_top_hit.map(|hid| {
            session
                .doc
                .page(pid)
                .and_then(|p| p.top_level_shape_id(hid))
                .unwrap_or(hid)
        });

        let is_middle_btn = (ev.buttons & 4) != 0;
        let is_shift = (ev.modifiers & 1) != 0;
        let is_cmd = (ev.modifiers & 8) != 0;
        let is_multi_toggle = is_shift || is_cmd;
        let is_alt = (ev.modifiers & 4) != 0;

        match ev.kind {
            1 => {
                // Pointer Down
                self.clear_transient_overlay();
                if is_middle_btn || self.active_tool == NativeCanvasTool::Pan {
                    self.drag_state = Some(NativeDragState::Pan {
                        last_x_px: ev.x_px,
                        last_y_px: ev.y_px,
                    });
                    return 3;
                }
                match self.active_tool {
                    NativeCanvasTool::Select => {
                        // 1. Check 1D Connector Begin/End glue knobs & interior orthogonal mid-segment diamonds!
                        if self.selected_ids.len() == 1 {
                            let sel_id = self.selected_ids[0];
                            if let Some(c_hit) =
                                hit_test_connector_1d_grips(&visuals, sel_id, page_pt, logical_ppi)
                            {
                                match c_hit {
                                    Connector1DGripHit::BeginEndpoint(pt) => {
                                        self.drag_state =
                                            Some(NativeDragState::DragConnectorEndpoint {
                                                shape_id: sel_id,
                                                is_begin: true,
                                                cur_page: pt,
                                                target_shape_id: None,
                                                target_port_cell: None,
                                            });
                                        return 2;
                                    }
                                    Connector1DGripHit::EndEndpoint(pt) => {
                                        self.drag_state =
                                            Some(NativeDragState::DragConnectorEndpoint {
                                                shape_id: sel_id,
                                                is_begin: false,
                                                cur_page: pt,
                                                target_shape_id: None,
                                                target_port_cell: None,
                                            });
                                        return 2;
                                    }
                                    Connector1DGripHit::InteriorSegment {
                                        seg_index,
                                        is_vertical,
                                        coord_in,
                                    } => {
                                        let base_poly = visuals
                                            .iter()
                                            .find(|v| v.shape_id == sel_id)
                                            .map(vexio_render::tessellate::extract_connector_polyline_in)
                                            .unwrap_or_default();
                                        let orig_manual_pts = session
                                            .doc
                                            .page(pid)
                                            .and_then(|p| p.find_shape(sel_id))
                                            .and_then(|sh| {
                                                sh.sheet
                                                    .user_cells
                                                    .get("manualorthopoints")
                                                    .map(|u| u.value.clone())
                                            });
                                        self.drag_state =
                                            Some(NativeDragState::DragConnectorSegment {
                                                shape_id: sel_id,
                                                seg_index,
                                                is_vertical,
                                                cur_coord_in: coord_in,
                                                base_poly,
                                                orig_manual_pts,
                                            });
                                        return if is_vertical { 6 } else { 5 };
                                    }
                                }
                            }
                        }

                        // 2a. Check 8-Handle Resize Grips & Top `↻` Rotation Button FIRST across all selected 2D/Group shapes!
                        for &cand_sid in self.selected_ids.iter().rev() {
                            if let Some(grip_hit) = hit_test_selection_grips(
                                &visuals,
                                cand_sid,
                                page_pt,
                                logical_ppi,
                            ) {
                                if let Some(vis) = visuals.iter().find(|v| v.shape_id == cand_sid) {
                                    let ox = vis.transform.pin_x;
                                    let oy = vis.transform.pin_y;
                                    let ow = vis.transform.width.max(0.1);
                                    let oh = vis.transform.height.max(0.1);
                                    let oa = vis.transform.angle_rad.to_degrees();
                                    match grip_hit {
                                        Ok(handle) => {
                                            self.drag_state = Some(NativeDragState::ResizeShape {
                                                shape_id: cand_sid,
                                                handle,
                                                start_page: page_pt,
                                                orig_pin: [ox, oy],
                                                orig_size: [ow, oh],
                                                orig_angle_deg: oa,
                                                cur_pin: [ox, oy],
                                                cur_size: [ow, oh],
                                            });
                                            return handle.cursor_code();
                                        }
                                        Err(()) => {
                                            self.drag_state = Some(NativeDragState::RotateShape {
                                                shape_id: cand_sid,
                                                start_page: page_pt,
                                                orig_pin: [ox, oy],
                                                orig_size: [ow, oh],
                                                orig_angle_deg: oa,
                                                cur_angle_deg: oa,
                                                dragged: false,
                                            });
                                            return 13;
                                        }
                                    }
                                }
                            }
                        }

                        // 2b. Check Yellow Diamond `Controls.*` Handles & 4 Directional AutoConnect Triangles!
                        if self.selected_ids.len() == 1 {
                            let primary_sid = self.selected_ids[0];
                            if let Some((row_name, ctrl_pt)) = hit_test_control_handles(
                                &visuals,
                                primary_sid,
                                page_pt,
                                logical_ppi,
                            ) {
                                if let Some(sh) =
                                    session.doc.page(pid).and_then(|p| p.find_shape(primary_sid))
                                {
                                    let ox = sh
                                        .sheet
                                        .get_cell(&format!("Controls.{row_name}.X"))
                                        .map(|c| c.formula.clone())
                                        .unwrap_or_else(|| "0 in".into());
                                    let oy = sh
                                        .sheet
                                        .get_cell(&format!("Controls.{row_name}.Y"))
                                        .map(|c| c.formula.clone())
                                        .unwrap_or_else(|| "0 in".into());
                                    let oxd = sh
                                        .sheet
                                        .get_cell(&format!("Controls.{row_name}.XDyn"))
                                        .map(|c| c.formula.clone())
                                        .unwrap_or_else(|| ox.clone());
                                    let oyd = sh
                                        .sheet
                                        .get_cell(&format!("Controls.{row_name}.YDyn"))
                                        .map(|c| c.formula.clone())
                                        .unwrap_or_else(|| oy.clone());
                                    self.drag_state = Some(NativeDragState::DragControlHandle {
                                        shape_id: primary_sid,
                                        row_name,
                                        orig_x: ox,
                                        orig_y: oy,
                                        orig_xdyn: oxd,
                                        orig_ydyn: oyd,
                                        cur_page: ctrl_pt,
                                    });
                                    return 2;
                                }
                            }
                            if let Some((ac_dir, port_start)) = hit_test_autoconnect_triangles(
                                &visuals,
                                primary_sid,
                                page_pt,
                                logical_ppi,
                            ) {
                                self.overlay.hovered_autoconnect = Some((primary_sid, ac_dir));
                                self.drag_state = Some(NativeDragState::AutoConnectGesture {
                                    source_id: primary_sid,
                                    dir: ac_dir,
                                    start_page: page_pt,
                                    port_start_in: port_start,
                                    cur_page: page_pt,
                                    dragged: false,
                                    target_shape_id: None,
                                    target_port_cell: None,
                                });
                                self.present_if_attached(&session.doc);
                                return 2;
                            }
                        }

                        // 3. Hit a shape -> select / multi-select & enter multi-shape MoveShapes
                        if let Some(sid) = top_hit {
                            if is_multi_toggle {
                                if let Some(pos) = self.selected_ids.iter().position(|&x| x == sid)
                                {
                                    self.selected_ids.remove(pos);
                                } else {
                                    self.selected_ids.push(sid);
                                }
                            } else if !self.selected_ids.contains(&sid) {
                                self.selected_ids = vec![sid];
                            }
                            self.sync_glued_ends(&session.doc);

                            if !self.selected_ids.is_empty() {
                                let mut sel_sub_set = std::collections::BTreeSet::new();
                                if let Some(page) = session.doc.page(pid) {
                                    for &sel_id in &self.selected_ids {
                                        if let Some(sh) = page.find_shape(sel_id) {
                                            vexio_shapesheet::VisioPage::collect_subtree_ids(
                                                sh,
                                                &mut sel_sub_set,
                                            );
                                        } else {
                                            sel_sub_set.insert(sel_id);
                                        }
                                    }
                                }
                                let mut u_min = [f64::INFINITY, f64::INFINITY];
                                let mut u_max = [f64::NEG_INFINITY, f64::NEG_INFINITY];
                                for v in &visuals {
                                    if sel_sub_set.contains(&v.shape_id) {
                                        u_min[0] = u_min[0].min(v.aabb_min[0]);
                                        u_min[1] = u_min[1].min(v.aabb_min[1]);
                                        u_max[0] = u_max[0].max(v.aabb_max[0]);
                                        u_max[1] = u_max[1].max(v.aabb_max[1]);
                                    }
                                }
                                if !u_min[0].is_finite() {
                                    u_min = [page_pt[0] - 0.5, page_pt[1] - 0.5];
                                    u_max = [page_pt[0] + 0.5, page_pt[1] + 0.5];
                                }
                                self.drag_state = Some(NativeDragState::MoveShapes {
                                    shape_ids: self.selected_ids.clone(),
                                    start_page: page_pt,
                                    applied_delta: [0.0, 0.0],
                                    orig_union_min: u_min,
                                    orig_union_max: u_max,
                                    moved: false,
                                });
                            }
                            self.present_if_attached(&session.doc);
                            return 1;
                        } else {
                            // 4. Empty canvas in Pointer mode -> Rubber-Band Marquee Multi-Select!
                            let prior = if is_multi_toggle {
                                self.selected_ids.clone()
                            } else {
                                self.selected_ids.clear();
                                Vec::new()
                            };
                            self.sync_glued_ends(&session.doc);
                            self.drag_state = Some(NativeDragState::MarqueeSelect {
                                start_page: page_pt,
                                cur_page: page_pt,
                                additive: is_multi_toggle,
                                prior_ids: prior,
                            });
                            self.present_if_attached(&session.doc);
                            return 0;
                        }
                    }
                    NativeCanvasTool::Freeform
                    | NativeCanvasTool::InkPen
                    | NativeCanvasTool::Highlighter => {
                        self.overlay.live_stroke_points = vec![page_pt];
                        self.overlay.live_stroke_highlighter =
                            self.active_tool == NativeCanvasTool::Highlighter;
                        self.drag_state = Some(NativeDragState::DrawPath {
                            tool: self.active_tool,
                            points: vec![MarkupInkPoint {
                                x_in: page_pt[0],
                                y_in: page_pt[1],
                                pressure: ev.pressure.clamp(0.1, 1.0),
                                tilt_x: ev.tilt_x_deg,
                                tilt_y: ev.tilt_y_deg,
                            }],
                        });
                        return 2;
                    }
                    NativeCanvasTool::InkEraser => {
                        let _ = session.erase_ink_at(
                            pid,
                            page_pt[0],
                            page_pt[1],
                            (14.0 / logical_ppi).max(0.10),
                        );
                        self.present_if_attached(&session.doc);
                        return 2;
                    }
                    NativeCanvasTool::EditVertices => {
                        let is_right_btn = (ev.buttons & 2) != 0;
                        if let Some(&sid) = self.selected_ids.first() {
                            let verts =
                                crate::boolean_ops::list_shape_vertices(&session.doc, pid, sid);
                            let v_tol = (10.5 / logical_ppi).max(0.06);
                            if let Some(hit_v) = verts.iter().find(|v| {
                                (v.page_x_in - page_pt[0]).hypot(v.page_y_in - page_pt[1]) <= v_tol
                            }) {
                                if is_right_btn || (is_alt && is_shift) {
                                    let _ = session.delete_shape_vertex(
                                        pid,
                                        sid,
                                        hit_v.geom_index,
                                        hit_v.row_ix,
                                    );
                                    self.present_if_attached(&session.doc);
                                    return 2;
                                }
                                let (ox, oy) = session
                                    .doc
                                    .page(pid)
                                    .and_then(|p| p.find_shape(sid))
                                    .and_then(|sh| {
                                        sh.sheet
                                            .geometries
                                            .iter()
                                            .find(|g| g.index == hit_v.geom_index && !g.deleted)
                                            .and_then(|g| {
                                                g.rows
                                                    .iter()
                                                    .find(|r| r.ix == hit_v.row_ix && !r.deleted)
                                                    .map(|r| (r.x.clone(), r.y.clone()))
                                            })
                                    })
                                    .unwrap_or_else(|| ("0 in".into(), "0 in".into()));
                                self.drag_state = Some(NativeDragState::DragShapeVertex {
                                    shape_id: sid,
                                    geom_index: hit_v.geom_index,
                                    row_ix: hit_v.row_ix,
                                    orig_x: ox,
                                    orig_y: oy,
                                    cur_page: [hit_v.page_x_in, hit_v.page_y_in],
                                });
                                return 2;
                            } else if is_alt && !verts.is_empty() {
                                if let Some(nearest) = verts.iter().min_by(|a, b| {
                                    let da = (a.page_x_in - page_pt[0])
                                        .hypot(a.page_y_in - page_pt[1]);
                                    let db = (b.page_x_in - page_pt[0])
                                        .hypot(b.page_y_in - page_pt[1]);
                                    da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                                }) {
                                    let _ = session.insert_shape_vertex(
                                        pid,
                                        sid,
                                        nearest.geom_index,
                                        nearest.row_ix,
                                        page_pt[0],
                                        page_pt[1],
                                    );
                                    self.present_if_attached(&session.doc);
                                    return 2;
                                }
                            }
                        }
                        if let Some(sid) = top_hit {
                            self.selected_ids = vec![sid];
                            self.present_if_attached(&session.doc);
                        }
                        return 2;
                    }
                    NativeCanvasTool::Connector => {
                        let port_hit = session.find_nearest_connection_port(
                            pid,
                            0,
                            page_pt[0],
                            page_pt[1],
                            port_tol_in,
                        );
                        let (from_sid, from_port, start_p) = if let Some((sid, pcell, spos, pts)) =
                            port_hit
                        {
                            let snap_active = pcell.is_some();
                            self.overlay.highlighted_ports = pts
                                .into_iter()
                                .map(|p| {
                                    (
                                        p,
                                        snap_active
                                            && (p[0] - spos[0]).hypot(p[1] - spos[1]) < 1e-4,
                                    )
                                })
                                .collect();
                            (Some(sid), pcell, spos)
                        } else {
                            (top_hit, None, page_pt)
                        };
                        self.overlay.connector_preview_in = Some((start_p, page_pt));
                        self.drag_state = Some(NativeDragState::DrawBoxOrLine {
                            tool: NativeCanvasTool::Connector,
                            start_page: start_p,
                            from_shape_id: from_sid,
                            from_port_cell: from_port,
                        });
                        self.present_if_attached(&session.doc);
                        return 2;
                    }
                    tool => {
                        self.drag_state = Some(NativeDragState::DrawBoxOrLine {
                            tool,
                            start_page: page_pt,
                            from_shape_id: top_hit,
                            from_port_cell: None,
                        });
                        return if tool == NativeCanvasTool::Text { 7 } else { 2 };
                    }
                }
            }
            0 | 4 => {
                // Move or StylusMove
                if let Some(state) = &mut self.drag_state {
                    match state {
                        NativeDragState::Pan {
                            last_x_px,
                            last_y_px,
                        } => {
                            let dx = ev.x_px - *last_x_px;
                            let dy = ev.y_px - *last_y_px;
                            *last_x_px = ev.x_px;
                            *last_y_px = ev.y_px;
                            self.camera.pan_by_pixels(dx, dy);
                            self.present_if_attached(&session.doc);
                            return 3;
                        }
                        NativeDragState::MarqueeSelect {
                            start_page,
                            cur_page,
                            additive,
                            prior_ids,
                        } => {
                            *cur_page = page_pt;
                            let min_q = [start_page[0].min(page_pt[0]), start_page[1].min(page_pt[1])];
                            let max_q = [start_page[0].max(page_pt[0]), start_page[1].max(page_pt[1])];
                            self.overlay.marquee_rect_in = Some((min_q, max_q));
                            let mut next_sel = if *additive {
                                prior_ids.clone()
                            } else {
                                Vec::new()
                            };
                            if (max_q[0] - min_q[0]).hypot(max_q[1] - min_q[1]) > 0.03 {
                                if let Some(page) = session.doc.page(pid) {
                                    let vis_map: std::collections::BTreeMap<
                                        u64,
                                        &vexio_shapesheet::EvaluatedShapeVisual,
                                    > = visuals.iter().map(|v| (v.shape_id, v)).collect();
                                    for top_sh in &page.shapes {
                                        let mut sub_ids = std::collections::BTreeSet::new();
                                        vexio_shapesheet::VisioPage::collect_subtree_ids(
                                            top_sh,
                                            &mut sub_ids,
                                        );
                                        let mut s_min = [f64::INFINITY, f64::INFINITY];
                                        let mut s_max = [f64::NEG_INFINITY, f64::NEG_INFINITY];
                                        for sid in sub_ids {
                                            if let Some(v) = vis_map.get(&sid) {
                                                if v.aabb_min[0].is_finite()
                                                    && v.aabb_max[0].is_finite()
                                                {
                                                    s_min[0] = s_min[0].min(v.aabb_min[0]);
                                                    s_min[1] = s_min[1].min(v.aabb_min[1]);
                                                    s_max[0] = s_max[0].max(v.aabb_max[0]);
                                                    s_max[1] = s_max[1].max(v.aabb_max[1]);
                                                }
                                            }
                                        }
                                        if !s_min[0].is_finite() {
                                            continue;
                                        }
                                        let matched = if is_alt {
                                            // Option/Alt + Marquee = Visio Intersecting Area Select
                                            s_max[0] >= min_q[0]
                                                && s_min[0] <= max_q[0]
                                                && s_max[1] >= min_q[1]
                                                && s_min[1] <= max_q[1]
                                        } else {
                                            // Default Visio Enclosure Marquee (ignores outer surrounding containers)
                                            s_min[0] >= min_q[0] - 0.02
                                                && s_min[1] >= min_q[1] - 0.02
                                                && s_max[0] <= max_q[0] + 0.02
                                                && s_max[1] <= max_q[1] + 0.02
                                        };
                                        if matched && !next_sel.contains(&top_sh.id) {
                                            next_sel.push(top_sh.id);
                                        }
                                    }
                                }
                            }
                            self.selected_ids = next_sel;
                            self.present_if_attached(&session.doc);
                            return 0;
                        }
                        NativeDragState::ResizeShape {
                            shape_id,
                            handle,
                            start_page,
                            orig_pin,
                            orig_size,
                            cur_pin,
                            cur_size,
                            ..
                        } => {
                            let sid = *shape_id;
                            let h_copy = *handle;
                            let dx = page_pt[0] - start_page[0];
                            let dy = page_pt[1] - start_page[1];
                            let (mut np, mut ns) =
                                h_copy.compute_box(*orig_pin, *orig_size, dx, dy, is_shift);

                            if self.smart_guides_enabled && !is_alt {
                                let c_min = [np[0] - ns[0] * 0.5, np[1] - ns[1] * 0.5];
                                let c_max = [np[0] + ns[0] * 0.5, np[1] + ns[1] * 0.5];
                                let (_snap_d, guides) = session.compute_smart_guides(
                                    pid,
                                    &[sid],
                                    c_min,
                                    c_max,
                                    snap_tol_in,
                                );
                                self.overlay.smart_guides = guides;
                            } else {
                                self.overlay.smart_guides.clear();
                            }
                            np[0] = (np[0] * 10000.0).round() / 10000.0;
                            np[1] = (np[1] * 10000.0).round() / 10000.0;
                            ns[0] = ns[0].max(0.1);
                            ns[1] = ns[1].max(0.1);
                            *cur_pin = np;
                            *cur_size = ns;
                            let code = h_copy.cursor_code();
                            if let Some(page) = session.doc.page_mut(pid) {
                                if let Some(sh) = page.find_shape_mut(sid) {
                                    let _ = sh.sheet.try_set_formula_respecting_guard(
                                        "PinX",
                                        format!("{} in", format_clean_f64(np[0])),
                                        false,
                                    );
                                    let _ = sh.sheet.try_set_formula_respecting_guard(
                                        "PinY",
                                        format!("{} in", format_clean_f64(np[1])),
                                        false,
                                    );
                                    let _ = sh.sheet.try_set_formula_respecting_guard(
                                        "Width",
                                        format!("{} in", format_clean_f64(ns[0])),
                                        false,
                                    );
                                    let _ = sh.sheet.try_set_formula_respecting_guard(
                                        "Height",
                                        format!("{} in", format_clean_f64(ns[1])),
                                        false,
                                    );
                                }
                            }
                            ReactiveEngine::recalculate_document(&mut session.doc);
                            route_page_connectors(&mut session.doc, pid);
                            self.present_if_attached(&session.doc);
                            return code;
                        }
                        NativeDragState::RotateShape {
                            shape_id,
                            start_page,
                            orig_pin,
                            cur_angle_deg,
                            dragged,
                            ..
                        } => {
                            if !*dragged
                                && (page_pt[0] - start_page[0]).hypot(page_pt[1] - start_page[1])
                                    >= (4.0 / eff_ppi.max(12.0)).max(0.025)
                            {
                                *dragged = true;
                            }
                            if *dragged {
                                let rad = (page_pt[1] - orig_pin[1]).atan2(page_pt[0] - orig_pin[0])
                                    - std::f64::consts::FRAC_PI_2;
                                let deg = if is_alt {
                                    rad.to_degrees()
                                } else {
                                    (rad.to_degrees() / 5.0).round() * 5.0
                                };
                                *cur_angle_deg = deg;
                                if let Some(page) = session.doc.page_mut(pid) {
                                    if let Some(sh) = page.find_shape_mut(*shape_id) {
                                        let _ = sh.sheet.try_set_formula_respecting_guard(
                                            "Angle",
                                            format!("{} deg", format_clean_f64(deg)),
                                            false,
                                        );
                                    }
                                }
                                ReactiveEngine::recalculate_document(&mut session.doc);
                                route_page_connectors(&mut session.doc, pid);
                                self.present_if_attached(&session.doc);
                            }
                            return 13;
                        }
                        NativeDragState::MoveShapes {
                            shape_ids,
                            start_page,
                            applied_delta,
                            orig_union_min,
                            orig_union_max,
                            moved,
                        } => {
                            let raw_dx = page_pt[0] - start_page[0];
                            let raw_dy = page_pt[1] - start_page[1];
                            if raw_dx.hypot(raw_dy) > 0.015 || *moved {
                                *moved = true;
                                let ids_copy = shape_ids.clone();
                                let prev_applied = *applied_delta;
                                let orig_min = *orig_union_min;
                                let orig_max = *orig_union_max;

                                // Temporarily revert `prev_applied` before computing Smart Guides against clean siblings
                                translate_preview_shapes_in_doc(
                                    &mut session.doc,
                                    pid,
                                    &ids_copy,
                                    -prev_applied[0],
                                    -prev_applied[1],
                                );

                                let (snap_d, guides) = if self.smart_guides_enabled && !is_alt {
                                    session.compute_smart_guides(
                                        pid,
                                        &ids_copy,
                                        [orig_min[0] + raw_dx, orig_min[1] + raw_dy],
                                        [orig_max[0] + raw_dx, orig_max[1] + raw_dy],
                                        snap_tol_in,
                                    )
                                } else {
                                    ([0.0, 0.0], Vec::new())
                                };

                                let next_dx = raw_dx + snap_d[0];
                                let next_dy = raw_dy + snap_d[1];
                                translate_preview_shapes_in_doc(
                                    &mut session.doc,
                                    pid,
                                    &ids_copy,
                                    next_dx,
                                    next_dy,
                                );

                                if let Some(NativeDragState::MoveShapes { applied_delta, .. }) =
                                    &mut self.drag_state
                                {
                                    *applied_delta = [next_dx, next_dy];
                                }
                                let cand_center = [
                                    0.5 * (orig_min[0] + orig_max[0]) + next_dx,
                                    0.5 * (orig_min[1] + orig_max[1]) + next_dy,
                                ];
                                self.overlay.container_drop_target_in =
                                    crate::containers::find_drop_target_container_or_swimlane(
                                        &session.doc,
                                        pid,
                                        &ids_copy,
                                        cand_center,
                                    )
                                    .map(|(_, mn, mx)| (mn, mx));
                                self.overlay.smart_guides = guides;
                                self.present_if_attached(&session.doc);
                            }
                            return 1;
                        }
                        NativeDragState::DragConnectorEndpoint {
                            shape_id,
                            is_begin,
                            cur_page,
                            target_shape_id,
                            target_port_cell,
                        } => {
                            let sid = *shape_id;
                            let beg = *is_begin;
                            let port_hit = session.find_nearest_connection_port(
                                pid,
                                sid,
                                page_pt[0],
                                page_pt[1],
                                port_tol_in,
                            );
                            if let Some((tid, pcell, spos, pts)) = port_hit {
                                *cur_page = spos;
                                *target_shape_id = Some(tid);
                                *target_port_cell = pcell.clone();
                                let snap_active = pcell.is_some();
                                self.overlay.highlighted_ports = pts
                                    .into_iter()
                                    .map(|p| {
                                        (
                                            p,
                                            snap_active
                                                && (p[0] - spos[0]).hypot(p[1] - spos[1]) < 1e-4,
                                        )
                                    })
                                    .collect();
                            } else {
                                *cur_page = page_pt;
                                *target_shape_id = None;
                                *target_port_cell = None;
                                self.overlay.highlighted_ports.clear();
                            }
                            // Live preview connector line from fixed opposite endpoint to `cur_page`
                            if let Some(vis) = visuals.iter().find(|v| v.shape_id == sid) {
                                let poly = vexio_render::tessellate::extract_connector_polyline_in(vis);
                                if poly.len() >= 2 {
                                    let anchor = if beg {
                                        *poly.last().unwrap_or(&poly[0])
                                    } else {
                                        poly[0]
                                    };
                                    let tip = *cur_page;
                                    self.overlay.connector_preview_in = Some(if beg {
                                        (tip, anchor)
                                    } else {
                                        (anchor, tip)
                                    });
                                }
                            }
                            self.present_if_attached(&session.doc);
                            return 2;
                        }
                        NativeDragState::DragConnectorSegment {
                            shape_id,
                            seg_index,
                            is_vertical,
                            cur_coord_in,
                            base_poly,
                            ..
                        } => {
                            let new_coord = if *is_vertical { page_pt[0] } else { page_pt[1] };
                            *cur_coord_in = new_coord;
                            let sid = *shape_id;
                            let sidx = *seg_index;
                            let vert = *is_vertical;
                            if base_poly.len() >= 2 {
                                let preview_poly =
                                    crate::routing::slide_orthogonal_segment(base_poly, sidx, new_coord);
                                let serialized = crate::routing::format_manual_ortho_points(&preview_poly);
                                if let Some(sh) =
                                    session.doc.page_mut(pid).and_then(|p| p.find_shape_mut(sid))
                                {
                                    sh.sheet.set_formula(
                                        "ShapeRouteStyle",
                                        "16",
                                        Some(vexio_shapesheet::Unit::Number),
                                    );
                                    sh.sheet.set_formula(
                                        "User.Preserve1DGeometry",
                                        "FALSE",
                                        Some(vexio_shapesheet::Unit::Boolean),
                                    );
                                    sh.sheet.set_formula(
                                        "User.MidAxis",
                                        "0",
                                        Some(vexio_shapesheet::Unit::Number),
                                    );
                                    sh.sheet.set_user_cell(
                                        "ManualOrthoPoints",
                                        serialized,
                                        "User-adjusted multi-bend orthogonal waypoints",
                                    );
                                }
                                route_page_connectors(&mut session.doc, pid);
                                self.present_if_attached(&session.doc);
                            }
                            return if vert { 6 } else { 5 };
                        }
                        NativeDragState::DrawPath { tool, points } => {
                            let should_push = points
                                .last()
                                .map(|p| (p.x_in - page_pt[0]).hypot(p.y_in - page_pt[1]) >= 0.02)
                                .unwrap_or(true);
                            if should_push {
                                points.push(MarkupInkPoint {
                                    x_in: page_pt[0],
                                    y_in: page_pt[1],
                                    pressure: ev.pressure.clamp(0.1, 1.0),
                                    tilt_x: ev.tilt_x_deg,
                                    tilt_y: ev.tilt_y_deg,
                                });
                                self.overlay.live_stroke_points =
                                    points.iter().map(|p| [p.x_in, p.y_in]).collect();
                                self.overlay.live_stroke_highlighter =
                                    *tool == NativeCanvasTool::Highlighter;
                                self.present_if_attached(&session.doc);
                            }
                            return 2;
                        }
                        NativeDragState::DragShapeVertex {
                            shape_id,
                            geom_index,
                            row_ix,
                            cur_page,
                            ..
                        } => {
                            *cur_page = page_pt;
                            let sid = *shape_id;
                            let gidx = *geom_index;
                            let rix = *row_ix;
                            let _ = crate::boolean_ops::move_shape_vertex(
                                &mut session.doc,
                                pid,
                                sid,
                                gidx,
                                rix,
                                page_pt[0],
                                page_pt[1],
                            );
                            self.present_if_attached(&session.doc);
                            return 2;
                        }
                        NativeDragState::DrawBoxOrLine {
                            tool,
                            start_page,
                            from_shape_id,
                            ..
                        } => {
                            let t_copy = *tool;
                            let sp = *start_page;
                            let f_sid = from_shape_id.unwrap_or(0);
                            if t_copy == NativeCanvasTool::Connector {
                                if let Some((_tid, pcell, spos, pts)) = session
                                    .find_nearest_connection_port(
                                        pid,
                                        f_sid,
                                        page_pt[0],
                                        page_pt[1],
                                        port_tol_in,
                                    )
                                {
                                    let snap_active = pcell.is_some();
                                    self.overlay.highlighted_ports = pts
                                        .into_iter()
                                        .map(|p| {
                                            (
                                                p,
                                                snap_active
                                                    && (p[0] - spos[0]).hypot(p[1] - spos[1])
                                                        < 1e-4,
                                            )
                                        })
                                        .collect();
                                    self.overlay.connector_preview_in = Some((sp, spos));
                                } else {
                                    self.overlay.highlighted_ports.clear();
                                    self.overlay.connector_preview_in = Some((sp, page_pt));
                                }
                                self.present_if_attached(&session.doc);
                            }
                            return if t_copy == NativeCanvasTool::Text {
                                7
                            } else {
                                2
                            };
                        }
                        NativeDragState::DragControlHandle {
                            shape_id,
                            row_name,
                            cur_page,
                            ..
                        } => {
                            *cur_page = page_pt;
                            let sid = *shape_id;
                            let rname = row_name.clone();
                            let _ = crate::boolean_ops::move_control_handle(
                                &mut session.doc,
                                pid,
                                sid,
                                &rname,
                                page_pt[0],
                                page_pt[1],
                            );
                            self.present_if_attached(&session.doc);
                            return 2;
                        }
                        NativeDragState::AutoConnectGesture {
                            source_id,
                            start_page,
                            port_start_in,
                            cur_page,
                            dragged,
                            target_shape_id,
                            target_port_cell,
                            ..
                        } => {
                            let sid = *source_id;
                            let pstart = *port_start_in;
                            if !*dragged
                                && (page_pt[0] - start_page[0]).hypot(page_pt[1] - start_page[1])
                                    > (6.0 / logical_ppi).max(0.04)
                            {
                                *dragged = true;
                            }
                            if *dragged {
                                if let Some((tid, pcell, spos, pts)) = session
                                    .find_nearest_connection_port(
                                        pid,
                                        sid,
                                        page_pt[0],
                                        page_pt[1],
                                        port_tol_in,
                                    )
                                {
                                    *cur_page = spos;
                                    *target_shape_id = Some(tid);
                                    *target_port_cell = pcell.clone();
                                    let snap_active = pcell.is_some();
                                    self.overlay.highlighted_ports = pts
                                        .into_iter()
                                        .map(|p| {
                                            (
                                                p,
                                                snap_active
                                                    && (p[0] - spos[0]).hypot(p[1] - spos[1])
                                                        < 1e-4,
                                            )
                                        })
                                        .collect();
                                    self.overlay.connector_preview_in = Some((pstart, spos));
                                } else {
                                    *cur_page = page_pt;
                                    *target_shape_id = None;
                                    *target_port_cell = None;
                                    self.overlay.highlighted_ports.clear();
                                    self.overlay.connector_preview_in = Some((pstart, page_pt));
                                }
                                self.present_if_attached(&session.doc);
                            }
                            return 2;
                        }
                    }
                } else if self.active_tool == NativeCanvasTool::Connector {
                    // Hovering in Connector mode highlights nearby shape ports (`×`)!
                    if let Some((_tid, pcell, spos, pts)) = session.find_nearest_connection_port(
                        pid,
                        0,
                        page_pt[0],
                        page_pt[1],
                        port_tol_in,
                    ) {
                        let snap_active = pcell.is_some();
                        self.overlay.highlighted_ports = pts
                            .into_iter()
                            .map(|p| {
                                (
                                    p,
                                    snap_active && (p[0] - spos[0]).hypot(p[1] - spos[1]) < 1e-4,
                                )
                            })
                            .collect();
                    } else {
                        self.overlay.highlighted_ports.clear();
                    }
                    self.present_if_attached(&session.doc);
                } else if self.active_tool == NativeCanvasTool::InkEraser
                    && (ev.buttons & 1) != 0
                {
                    let _ = session.erase_ink_at(
                        pid,
                        page_pt[0],
                        page_pt[1],
                        (14.0 / logical_ppi).max(0.10),
                    );
                    self.present_if_attached(&session.doc);
                    return 2;
                } else if self.active_tool == NativeCanvasTool::Select {
                    for &cand_sid in self.selected_ids.iter().rev() {
                        if let Some(grip_hit) =
                            hit_test_selection_grips(&visuals, cand_sid, page_pt, logical_ppi)
                        {
                            if self.overlay.hovered_autoconnect.is_some() {
                                self.overlay.hovered_autoconnect = None;
                                self.present_if_attached(&session.doc);
                            }
                            return match grip_hit {
                                Ok(handle) => handle.cursor_code(),
                                Err(()) => 13,
                            };
                        }
                    }
                    let next_hov = if self.selected_ids.len() == 1 {
                        let psid = self.selected_ids[0];
                        hit_test_autoconnect_triangles(&visuals, psid, page_pt, logical_ppi)
                            .map(|(dir, _)| (psid, dir))
                    } else {
                        None
                    };
                    if self.overlay.hovered_autoconnect != next_hov {
                        self.overlay.hovered_autoconnect = next_hov;
                        self.present_if_attached(&session.doc);
                    }
                    if next_hov.is_some() {
                        return 2;
                    }
                    if top_hit.is_some() {
                        return 1;
                    }
                }
            }
            2 => {
                // Pointer Up
                self.clear_transient_overlay();
                if let Some(state) = self.drag_state.take() {
                    match state {
                        NativeDragState::Pan { .. } | NativeDragState::MarqueeSelect { .. } => {}
                        NativeDragState::ResizeShape {
                            shape_id,
                            orig_pin,
                            orig_size,
                            orig_angle_deg,
                            cur_pin,
                            cur_size,
                            ..
                        } => {
                            if let Some(page) = session.doc.page_mut(pid) {
                                if let Some(sh) = page.find_shape_mut(shape_id) {
                                    sh.sheet.set_formula(
                                        "PinX",
                                        format!("{} in", format_clean_f64(orig_pin[0])),
                                        Some(vexio_shapesheet::Unit::Inch),
                                    );
                                    sh.sheet.set_formula(
                                        "PinY",
                                        format!("{} in", format_clean_f64(orig_pin[1])),
                                        Some(vexio_shapesheet::Unit::Inch),
                                    );
                                    sh.sheet.set_formula(
                                        "Width",
                                        format!("{} in", format_clean_f64(orig_size[0])),
                                        Some(vexio_shapesheet::Unit::Inch),
                                    );
                                    sh.sheet.set_formula(
                                        "Height",
                                        format!("{} in", format_clean_f64(orig_size[1])),
                                        Some(vexio_shapesheet::Unit::Inch),
                                    );
                                }
                            }
                            let _ = session.transform_shape(
                                pid,
                                shape_id,
                                cur_pin[0],
                                cur_pin[1],
                                cur_size[0],
                                cur_size[1],
                                orig_angle_deg,
                            );
                        }
                        NativeDragState::RotateShape {
                            shape_id,
                            orig_pin,
                            orig_size,
                            orig_angle_deg,
                            cur_angle_deg,
                            dragged,
                            ..
                        } => {
                            if dragged {
                                if let Some(page) = session.doc.page_mut(pid) {
                                    if let Some(sh) = page.find_shape_mut(shape_id) {
                                        sh.sheet.set_formula(
                                            "Angle",
                                            format!("{} deg", format_clean_f64(orig_angle_deg)),
                                            Some(vexio_shapesheet::Unit::Degree),
                                        );
                                    }
                                }
                                let _ = session.transform_shape(
                                    pid,
                                    shape_id,
                                    orig_pin[0],
                                    orig_pin[1],
                                    orig_size[0],
                                    orig_size[1],
                                    cur_angle_deg,
                                );
                            } else {
                                // Single click/tap on the `↻` Rotate button rotates 90° clockwise (`-90°` CCW)
                                // while keeping the shape selected!
                                let _ = session.rotate_or_flip_shapes(
                                    pid,
                                    &[shape_id],
                                    -90.0,
                                    false,
                                    false,
                                );
                            }
                        }
                        NativeDragState::MoveShapes {
                            shape_ids,
                            applied_delta,
                            moved,
                            ..
                        } => {
                            if moved
                                && (applied_delta[0].abs() > 1e-6 || applied_delta[1].abs() > 1e-6)
                            {
                                // Revert preview offset first so `move_shapes_delta` commits a clean Undo/Redo entry!
                                translate_preview_shapes_in_doc(
                                    &mut session.doc,
                                    pid,
                                    &shape_ids,
                                    -applied_delta[0],
                                    -applied_delta[1],
                                );
                                let _ = session.move_shapes_delta(
                                    pid,
                                    &shape_ids,
                                    applied_delta[0],
                                    applied_delta[1],
                                );
                            }
                        }
                        NativeDragState::DragConnectorEndpoint {
                            shape_id,
                            is_begin,
                            cur_page,
                            target_shape_id,
                            target_port_cell,
                        } => {
                            let _ = session.move_connector_endpoint_with_port(
                                pid,
                                shape_id,
                                is_begin,
                                cur_page[0],
                                cur_page[1],
                                target_shape_id,
                                target_port_cell.as_deref(),
                            );
                        }
                        NativeDragState::DragConnectorSegment {
                            shape_id,
                            seg_index,
                            is_vertical,
                            orig_manual_pts,
                            ..
                        } => {
                            // Restore pre-drag state first so `drag_connector_segment` records a clean Undo/Redo step!
                            if let Some(sh) =
                                session.doc.page_mut(pid).and_then(|p| p.find_shape_mut(shape_id))
                            {
                                if let Some(orig) = orig_manual_pts {
                                    sh.sheet.set_user_cell(
                                        "ManualOrthoPoints",
                                        orig,
                                        "User-adjusted multi-bend orthogonal waypoints",
                                    );
                                } else {
                                    sh.sheet.user_cells.remove("manualorthopoints");
                                    sh.sheet.set_formula(
                                        "User.ManualOrthoPoints",
                                        "\"\"",
                                        Some(vexio_shapesheet::Unit::String),
                                    );
                                }
                            }
                            route_page_connectors(&mut session.doc, pid);
                            let final_coord = if is_vertical { page_pt[0] } else { page_pt[1] };
                            let _ = session.drag_connector_segment(
                                pid,
                                shape_id,
                                seg_index,
                                final_coord,
                            );
                        }
                        NativeDragState::DrawBoxOrLine {
                            tool,
                            start_page,
                            from_shape_id,
                            from_port_cell,
                        } => {
                            let created = match tool {
                                NativeCanvasTool::Line => session
                                    .draw_line(
                                        pid,
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::Arc => session
                                    .draw_arc(
                                        pid,
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                        0.0,
                                    )
                                    .ok(),
                                NativeCanvasTool::Connector => {
                                    let to_hit = session.find_nearest_connection_port(
                                        pid,
                                        from_shape_id.unwrap_or(0),
                                        page_pt[0],
                                        page_pt[1],
                                        port_tol_in,
                                    );
                                    let (to_sid, to_port, end_p) =
                                        if let Some((tid, pcell, spos, _)) = to_hit {
                                            (Some(tid), pcell, spos)
                                        } else {
                                            (top_hit, None, page_pt)
                                        };
                                    let cid_opt = session
                                        .connect_points_or_shapes(
                                            pid,
                                            start_page[0],
                                            start_page[1],
                                            end_p[0],
                                            end_p[1],
                                            from_shape_id,
                                            to_sid,
                                        )
                                        .ok();
                                    if let Some(cid) = cid_opt {
                                        if let (Some(fid), Some(ref fport)) =
                                            (from_shape_id, &from_port_cell)
                                        {
                                            let _ = session.move_connector_endpoint_with_port(
                                                pid,
                                                cid,
                                                true,
                                                start_page[0],
                                                start_page[1],
                                                Some(fid),
                                                Some(fport.as_str()),
                                            );
                                        }
                                        if let (Some(tid), Some(ref tport)) = (to_sid, &to_port) {
                                            let _ = session.move_connector_endpoint_with_port(
                                                pid,
                                                cid,
                                                false,
                                                end_p[0],
                                                end_p[1],
                                                Some(tid),
                                                Some(tport.as_str()),
                                            );
                                        }
                                    }
                                    cid_opt
                                }
                                NativeCanvasTool::Text => session
                                    .draw_primitive_2d(
                                        pid,
                                        "text",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::RoundedRect => session
                                    .draw_primitive_2d(
                                        pid,
                                        "round_rect",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::Ellipse => session
                                    .draw_primitive_2d(
                                        pid,
                                        "ellipse",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::Diamond => session
                                    .draw_primitive_2d(
                                        pid,
                                        "diamond",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::Triangle => session
                                    .draw_primitive_2d(
                                        pid,
                                        "triangle",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::Hexagon => session
                                    .draw_primitive_2d(
                                        pid,
                                        "hexagon",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                NativeCanvasTool::Star => session
                                    .draw_primitive_2d(
                                        pid,
                                        "star",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                                _ => session
                                    .draw_primitive_2d(
                                        pid,
                                        "rect",
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                    )
                                    .ok(),
                            };
                            if let Some(sid) = created {
                                self.selected_ids = vec![sid];
                                if tool == NativeCanvasTool::Text {
                                    self.last_created_text_shape_id = Some(sid);
                                }
                                if !is_shift {
                                    self.active_tool = NativeCanvasTool::Select;
                                }
                            }
                        }
                        NativeDragState::DrawPath { tool, points } => {
                            if points.len() >= 2 {
                                if tool == NativeCanvasTool::Freeform {
                                    let raw_pts: Vec<[f64; 2]> =
                                        points.iter().map(|p| [p.x_in, p.y_in]).collect();
                                    let mut created_id = None;
                                    let _ = session.journal.record_mutation(
                                        &mut session.doc,
                                        "Draw Freeform Curve",
                                        |doc| {
                                            created_id =
                                                draw_freeform_curve(doc, pid, &raw_pts, false);
                                        },
                                    );
                                    if let Some(sid) = created_id {
                                        self.selected_ids = vec![sid];
                                        if !is_shift {
                                            self.active_tool = NativeCanvasTool::Select;
                                        }
                                    }
                                } else {
                                    let stroke_id = session
                                        .doc
                                        .page(pid)
                                        .map(|p| (p.ink_strokes.len() as u64) + 1)
                                        .unwrap_or(1);
                                    let highlighter = tool == NativeCanvasTool::Highlighter;
                                    let stroke = MarkupInkStroke {
                                        id: stroke_id,
                                        color: if highlighter {
                                            RgbaColor {
                                                r: 250,
                                                g: 204,
                                                b: 21,
                                                a: 110,
                                            }
                                        } else {
                                            RgbaColor {
                                                r: 37,
                                                g: 99,
                                                b: 235,
                                                a: 255,
                                            }
                                        },
                                        width_pt: if highlighter { 12.0 } else { 2.25 },
                                        highlighter,
                                        points,
                                    };
                                        let _ = session.journal.execute(
                                        &mut session.doc,
                                        DocCommand::AddInkStroke {
                                            page_id: pid,
                                            stroke,
                                        },
                                    );
                                }
                            }
                        }
                        NativeDragState::DragControlHandle {
                            shape_id,
                            row_name,
                            orig_x,
                            orig_y,
                            orig_xdyn,
                            orig_ydyn,
                            cur_page,
                        } => {
                            // Restore pre-drag formulas first so `drag_control_handle` records a clean Undo/Redo step!
                            if let Some(sh) =
                                session.doc.page_mut(pid).and_then(|p| p.find_shape_mut(shape_id))
                            {
                                sh.sheet.set_formula(
                                    format!("Controls.{row_name}"),
                                    orig_x.clone(),
                                    Some(vexio_shapesheet::Unit::Inch),
                                );
                                sh.sheet.set_formula(
                                    format!("Controls.{row_name}.X"),
                                    orig_x,
                                    Some(vexio_shapesheet::Unit::Inch),
                                );
                                sh.sheet.set_formula(
                                    format!("Controls.{row_name}.Y"),
                                    orig_y,
                                    Some(vexio_shapesheet::Unit::Inch),
                                );
                                sh.sheet.set_formula(
                                    format!("Controls.{row_name}.XDyn"),
                                    orig_xdyn,
                                    Some(vexio_shapesheet::Unit::Inch),
                                );
                                sh.sheet.set_formula(
                                    format!("Controls.{row_name}.YDyn"),
                                    orig_ydyn,
                                    Some(vexio_shapesheet::Unit::Inch),
                                );
                            }
                            ReactiveEngine::recalculate_document(&mut session.doc);
                            let _ = session.drag_control_handle(
                                pid,
                                shape_id,
                                &row_name,
                                cur_page[0],
                                cur_page[1],
                            );
                        }
                        NativeDragState::AutoConnectGesture {
                            source_id,
                            dir,
                            port_start_in,
                            cur_page,
                            dragged,
                            target_shape_id,
                            target_port_cell,
                            ..
                        } => {
                            if !dragged {
                                if let Ok((new_sid, _cid)) =
                                    session.auto_connect_shape(pid, source_id, dir)
                                {
                                    self.selected_ids = vec![new_sid];
                                }
                            } else {
                                let resolved_to = target_shape_id.or_else(|| {
                                    top_hit.filter(|&tid| tid != source_id)
                                });
                                if let Some(tid) = resolved_to {
                                    if let Ok(cid) = session.connect_points_or_shapes(
                                        pid,
                                        port_start_in[0],
                                        port_start_in[1],
                                        cur_page[0],
                                        cur_page[1],
                                        Some(source_id),
                                        Some(tid),
                                    ) {
                                        if let Some(ref tport) = target_port_cell {
                                            let _ = session.move_connector_endpoint_with_port(
                                                pid,
                                                cid,
                                                false,
                                                cur_page[0],
                                                cur_page[1],
                                                Some(tid),
                                                Some(tport.as_str()),
                                            );
                                        }
                                        self.selected_ids = vec![tid];
                                    }
                                } else {
                                    // Dragged onto empty space -> stamp a new shape at `cur_page` and connect!
                                    if let Ok(new_sid) = session.draw_primitive_2d(
                                        pid,
                                        "round_rect",
                                        cur_page[0] - 0.8,
                                        cur_page[1] - 0.45,
                                        cur_page[0] + 0.8,
                                        cur_page[1] + 0.45,
                                    ) {
                                        let _ = session.set_shape_text(pid, new_sid, "Process");
                                        let _ = session.connect_shapes(pid, source_id, new_sid);
                                        self.selected_ids = vec![new_sid];
                                    }
                                }
                            }
                        }
                        NativeDragState::DragShapeVertex {
                            shape_id,
                            geom_index,
                            row_ix,
                            orig_x,
                            orig_y,
                            cur_page,
                        } => {
                            if let Some(sh) =
                                session.doc.page_mut(pid).and_then(|p| p.find_shape_mut(shape_id))
                            {
                                if let Some(g) = sh
                                    .sheet
                                    .geometries
                                    .iter_mut()
                                    .find(|g| g.index == geom_index && !g.deleted)
                                {
                                    if let Some(r) =
                                        g.rows.iter_mut().find(|r| r.ix == row_ix && !r.deleted)
                                    {
                                        r.x = orig_x;
                                        r.y = orig_y;
                                    }
                                }
                                sh.sheet.sync_all_sections_to_cells();
                            }
                            ReactiveEngine::recalculate_document(&mut session.doc);
                            let _ = session.move_shape_vertex(
                                pid,
                                shape_id,
                                geom_index,
                                row_ix,
                                cur_page[0],
                                cur_page[1],
                            );
                        }
                    }
                    self.present_if_attached(&session.doc);
                }
            }
            _ => {}
        }

        match self.active_tool {
            NativeCanvasTool::Pan => 3,
            NativeCanvasTool::Text => 7,
            NativeCanvasTool::Select => {
                if self.selected_ids.len() == 1 {
                    let psid = self.selected_ids[0];
                    if let Some(c_hit) =
                        hit_test_connector_1d_grips(&visuals, psid, page_pt, logical_ppi)
                    {
                        return match c_hit {
                            Connector1DGripHit::BeginEndpoint(_)
                            | Connector1DGripHit::EndEndpoint(_) => 2,
                            Connector1DGripHit::InteriorSegment { is_vertical, .. } => {
                                if is_vertical {
                                    6
                                } else {
                                    5
                                }
                            }
                        };
                    }
                }
                for &cand_sid in self.selected_ids.iter().rev() {
                    if let Some(grip_hit) =
                        hit_test_selection_grips(&visuals, cand_sid, page_pt, logical_ppi)
                    {
                        return match grip_hit {
                            Ok(h) => h.cursor_code(),
                            Err(()) => 13,
                        };
                    }
                }
                if self.selected_ids.len() == 1 {
                    let psid = self.selected_ids[0];
                    if hit_test_control_handles(&visuals, psid, page_pt, logical_ppi).is_some()
                        || hit_test_autoconnect_triangles(&visuals, psid, page_pt, logical_ppi)
                            .is_some()
                    {
                        return 2;
                    }
                }
                if top_hit.is_some() {
                    1
                } else {
                    0
                }
            }
            _ => 2,
        }
    }
}

#[cfg(target_os = "android")]
impl Drop for VexioViewport {
    fn drop(&mut self) {
        // Drop GPU surface BEFORE releasing `ANativeWindow` so `VkSurfaceKHR` teardown sees a live window.
        self.gpu_renderer = None;
        if !self.android_native_window.is_null() {
            #[link(name = "android")]
            extern "C" {
                fn ANativeWindow_release(window: *mut std::ffi::c_void);
            }
            // SAFETY: `android_native_window` was acquired via `ANativeWindow_fromSurface` in `jni_bridge`.
            unsafe {
                ANativeWindow_release(self.android_native_window);
            }
            self.android_native_window = ptr::null_mut();
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeCellDto {
    name: String,
    formula: String,
    value: String,
    guarded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeShapeDto {
    id: u64,
    name: String,
    kind: String,
    is_group: bool,
    child_count: usize,
    master_name: Option<String>,
    layer_indices: Vec<u32>,
    text: String,
    pin_x: f64,
    pin_y: f64,
    loc_pin_x: f64,
    loc_pin_y: f64,
    width: f64,
    height: f64,
    angle_deg: f64,
    flip_x: bool,
    flip_y: bool,
    begin_x: Option<f64>,
    begin_y: Option<f64>,
    end_x: Option<f64>,
    end_y: Option<f64>,
    route_style: i32,
    fill_color: String,
    fill_bkgnd: String,
    fill_pattern: i32,
    fill_trans_pct: f64,
    rounding_in: f64,
    line_color: String,
    line_pattern: i32,
    line_weight_pt: f64,
    line_trans_pct: f64,
    begin_arrow: i32,
    end_arrow: i32,
    lock_width: bool,
    lock_height: bool,
    lock_aspect: bool,
    lock_move_x: bool,
    lock_move_y: bool,
    lock_rotate: bool,
    lock_delete: bool,
    hide_text: bool,
    font_family: String,
    font_size_pt: f64,
    char_style: u8,
    text_color: String,
    text_bg_color: Option<String>,
    horiz_align: u8,
    vert_align: u8,
    bullet_style: u8,
    control_handles_count: usize,
    screentip: Option<String>,
    hyperlink_address: Option<String>,
    hyperlink_sub_address: Option<String>,
    hyperlink_description: Option<String>,
    bound_recordset_row: Option<[u64; 2]>,
    shape_data: Vec<crate::workspace::ShapeDataItemDto>,
    cells: Vec<NativeCellDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativePageDto {
    id: u32,
    name: String,
    width_in: f64,
    height_in: f64,
    shape_count: usize,
    is_background: bool,
    back_page_id: Option<u32>,
    page_color_hex: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeLayerDto {
    ix: u32,
    name: String,
    visible: bool,
    print: bool,
    lock: bool,
    snap: bool,
    glue: bool,
    member_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeStateDto {
    title: String,
    active_page_id: u32,
    page_width_in: f64,
    page_height_in: f64,
    display_unit: String,
    pages: Vec<NativePageDto>,
    layers: Vec<NativeLayerDto>,
    comments: Vec<vexio_shapesheet::CommentAnnotation>,
    ink_stroke_count: usize,
    shapes: Vec<NativeShapeDto>,
    data_link_state: crate::workspace::PageDataLinkStateDto,
    slide_snippets: Vec<crate::enterprise_plan2::SlideSnippetFrame>,
    svg_preview: String,
    can_undo: bool,
    can_redo: bool,
    state_revision: u64,
}

pub(crate) fn build_native_state_json(session: &DocumentSession, active_page_id: u32) -> String {
    let pid = if session.doc.page(active_page_id).is_some() {
        active_page_id
    } else {
        session
            .doc
            .pages
            .iter()
            .find(|p| !p.is_background)
            .or_else(|| session.doc.pages.first())
            .map(|p| p.id)
            .unwrap_or(1)
    };
    let active_page = session.doc.page(pid);
    let page_width_in = active_page.map(|p| p.width_in()).unwrap_or(11.0);
    let page_height_in = active_page.map(|p| p.height_in()).unwrap_or(8.5);
    let display_unit = active_page
        .and_then(|p| p.page_sheet.get_str("User.DisplayUnit"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "mm".to_string());

    let pages = session
        .doc
        .pages
        .iter()
        .map(|p| NativePageDto {
            id: p.id,
            name: p.name.clone(),
            width_in: p.width_in(),
            height_in: p.height_in(),
            shape_count: p.shapes.len(),
            is_background: p.is_background,
            back_page_id: p.back_page_id,
            page_color_hex: p
                .page_sheet
                .get_color("PageColor")
                .map(|c| c.to_hex_rgb())
                .unwrap_or_else(|| "#FFFFFF".to_string()),
        })
        .collect();

    let layers = active_page
        .map(|p| {
            p.layers
                .iter()
                .map(|l| NativeLayerDto {
                    ix: l.ix,
                    name: l.name.clone(),
                    visible: l.visible,
                    print: l.print,
                    lock: l.lock,
                    snap: l.snap,
                    glue: l.glue,
                    member_count: crate::workspace::count_layer_members_on_page(p, l.ix),
                })
                .collect()
        })
        .unwrap_or_default();

    let shapes = active_page
        .map(|page| {
            page.shapes
                .iter()
                .map(|s| {
                    let cells = s
                        .sheet
                        .cells
                        .values()
                        .filter(|c| !c.name.starts_with("Geometry"))
                        .take(80)
                        .map(|c| NativeCellDto {
                            name: c.name.clone(),
                            formula: c.formula.clone(),
                            value: c
                                .value
                                .as_str_coerced()
                                .unwrap_or_else(|e| e.as_str().into()),
                            guarded: c.is_guarded(),
                        })
                        .collect();
                    let bg_trans = s.sheet.get_f64("TextBkgndTrans").unwrap_or(0.0);
                    let text_bg_color = if bg_trans < 0.995 {
                        s.sheet.get_color("TextBkgnd").map(|c| c.to_hex_rgb())
                    } else {
                        None
                    };
                    let is_1d = s.kind == ShapeKind::Connector1D;
                    let w_in = s.sheet.get_f64("Width").unwrap_or(1.0);
                    let h_in = s.sheet.get_f64("Height").unwrap_or(1.0);
                    NativeShapeDto {
                        id: s.id,
                        name: s.name.clone(),
                        kind: format!("{:?}", s.kind),
                        is_group: s.kind == ShapeKind::Group || !s.children.is_empty(),
                        child_count: s.children.len(),
                        master_name: crate::workspace::resolve_shape_master_name(&session.doc, s),
                        layer_indices: s.layer_membership.clone(),
                        text: s.text.clone(),
                        pin_x: s.sheet.get_f64("PinX").unwrap_or(0.0),
                        pin_y: s.sheet.get_f64("PinY").unwrap_or(0.0),
                        loc_pin_x: s.sheet.get_f64("LocPinX").unwrap_or(w_in * 0.5),
                        loc_pin_y: s.sheet.get_f64("LocPinY").unwrap_or(h_in * 0.5),
                        width: w_in,
                        height: h_in,
                        angle_deg: s.sheet.get_f64("Angle").unwrap_or(0.0).to_degrees(),
                        flip_x: s.sheet.get_bool("FlipX").unwrap_or(false),
                        flip_y: s.sheet.get_bool("FlipY").unwrap_or(false),
                        begin_x: if is_1d { s.sheet.get_f64("BeginX") } else { None },
                        begin_y: if is_1d { s.sheet.get_f64("BeginY") } else { None },
                        end_x: if is_1d { s.sheet.get_f64("EndX") } else { None },
                        end_y: if is_1d { s.sheet.get_f64("EndY") } else { None },
                        route_style: s.sheet.get_f64("ShapeRouteStyle").unwrap_or(16.0).round() as i32,
                        fill_color: crate::workspace::resolve_inspector_fill_hex(s),
                        fill_bkgnd: s
                            .sheet
                            .get_color("FillBkgnd")
                            .unwrap_or(RgbaColor::WHITE)
                            .to_hex_rgb(),
                        fill_pattern: s.sheet.get_f64("FillPattern").unwrap_or(1.0).round() as i32,
                        fill_trans_pct: (s.sheet.get_f64("FillForegndTrans").unwrap_or(0.0) * 100.0)
                            .round()
                            .clamp(0.0, 100.0),
                        rounding_in: s.sheet.get_f64("Rounding").unwrap_or(0.0),
                        line_color: crate::workspace::resolve_inspector_line_hex(s),
                        line_pattern: s.sheet.get_f64("LinePattern").unwrap_or(1.0).round() as i32,
                        line_weight_pt: (s.sheet.get_f64("LineWeight").unwrap_or(1.0 / 72.0) * 72.0 * 100.0).round() / 100.0,
                        line_trans_pct: (s.sheet.get_f64("LineColorTrans").unwrap_or(0.0) * 100.0)
                            .round()
                            .clamp(0.0, 100.0),
                        begin_arrow: s.sheet.get_f64("BeginArrow").unwrap_or(0.0).round() as i32,
                        end_arrow: s
                            .sheet
                            .get_f64("EndArrow")
                            .unwrap_or(if is_1d { 4.0 } else { 0.0 })
                            .round() as i32,
                        lock_width: s.sheet.get_bool("LockWidth").unwrap_or(false),
                        lock_height: s.sheet.get_bool("LockHeight").unwrap_or(false),
                        lock_aspect: s.sheet.get_bool("LockAspect").unwrap_or(false),
                        lock_move_x: s.sheet.get_bool("LockMoveX").unwrap_or(false),
                        lock_move_y: s.sheet.get_bool("LockMoveY").unwrap_or(false),
                        lock_rotate: s.sheet.get_bool("LockRotate").unwrap_or(false),
                        lock_delete: s.sheet.get_bool("LockDelete").unwrap_or(false),
                        hide_text: s.sheet.get_bool("HideText").unwrap_or(false),
                        font_family: s
                            .sheet
                            .get_str("Char.Font")
                            .filter(|f| !f.is_empty())
                            .unwrap_or_else(|| "Inter".to_string()),
                        font_size_pt: (s.sheet.get_f64("Char.Size").unwrap_or(10.0 / 72.0) * 72.0 * 10.0).round() / 10.0,
                        char_style: s.sheet.get_f64("Char.Style").unwrap_or(0.0).round().clamp(0.0, 15.0) as u8,
                        text_color: s
                            .sheet
                            .get_color("Char.Color")
                            .unwrap_or(RgbaColor::BLACK)
                            .to_hex_rgb(),
                        text_bg_color,
                        horiz_align: s.sheet.get_f64("Para.HorzAlign").unwrap_or(1.0).round().clamp(0.0, 3.0) as u8,
                        vert_align: s.sheet.get_f64("VerticalAlign").unwrap_or(1.0).round().clamp(0.0, 2.0) as u8,
                        bullet_style: s.sheet.get_f64("Para.Bullet").unwrap_or(0.0).round().clamp(0.0, 4.0) as u8,
                        control_handles_count: list_control_handles(&session.doc, pid, s.id).len(),
                        screentip: s
                            .sheet
                            .get_cell("Comment")
                            .and_then(|c| c.value.as_str_coerced().ok())
                            .filter(|v| !v.trim().is_empty()),
                        hyperlink_address: s
                            .sheet
                            .hyperlinks
                            .first()
                            .map(|h| h.address.clone())
                            .filter(|v| !v.trim().is_empty()),
                        hyperlink_sub_address: s
                            .sheet
                            .hyperlinks
                            .first()
                            .map(|h| h.sub_address.clone())
                            .filter(|v| !v.trim().is_empty()),
                        hyperlink_description: s
                            .sheet
                            .hyperlinks
                            .first()
                            .map(|h| h.description.clone())
                            .filter(|v| !v.trim().is_empty()),
                        bound_recordset_row: s.bound_data_row.map(|(rs_id, row_id)| [rs_id as u64, row_id]),
                        shape_data: crate::workspace::collect_shape_data_items(&session.doc, s),
                        cells,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let comments: Vec<vexio_shapesheet::CommentAnnotation> = session
        .doc
        .comments
        .iter()
        .filter(|c| c.page_id == pid)
        .cloned()
        .collect();
    let ink_stroke_count = active_page.map(|p| p.ink_strokes.len()).unwrap_or(0);

    let dto = NativeStateDto {
        title: session.doc.properties.title.clone(),
        active_page_id: pid,
        page_width_in,
        page_height_in,
        display_unit,
        pages,
        layers,
        comments,
        ink_stroke_count,
        shapes,
        data_link_state: session.get_page_data_link_state(pid),
        slide_snippets: session.list_slide_snippets(),
        svg_preview: export_page_svg(&session.doc, pid),
        can_undo: session.journal.can_undo(),
        can_redo: session.journal.can_redo(),
        state_revision: session.journal.state_revision(),
    };
    serde_json::to_string(&dto).unwrap_or_else(|_| "{}".into())
}

pub(crate) fn build_stencils_catalog_json(session: Option<&DocumentSession>) -> String {
    let mut previews = list_builtin_stencil_previews();
    if let Some(sess) = session {
        for (cat, list) in &sess.imported_stencil_previews {
            previews.insert(cat.clone(), list.clone());
        }
    }
    serde_json::to_string(&previews).unwrap_or_else(|_| "{}".into())
}

#[no_mangle]
pub extern "C" fn vexio_engine_create() -> *mut VexioEngine {
    Box::into_raw(Box::new(VexioEngine { initialized: true }))
}

#[no_mangle]
pub unsafe extern "C" fn vexio_engine_destroy(engine: *mut VexioEngine) {
    if !engine.is_null() {
        // SAFETY: `engine` was allocated via `Box::into_raw` in `vexio_engine_create` and is non-null.
        drop(unsafe { Box::from_raw(engine) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_create_blank(title_utf8: *const c_char) -> *mut VexioDoc {
    let title = if title_utf8.is_null() {
        "Drawing1"
    } else {
        // SAFETY: Caller passes a valid NUL-terminated UTF-8 C string or null.
        unsafe { CStr::from_ptr(title_utf8) }
            .to_str()
            .unwrap_or("Drawing1")
    };
    Box::into_raw(Box::new(VexioDoc {
        session: DocumentSession::new_blank(title),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_open_bytes(
    engine: *mut VexioEngine,
    bytes: VexioByteSlice,
    ext_utf8: *const c_char,
    out_doc: *mut *mut VexioDoc,
) -> VexioStatus {
    catch_ffi_status(|| {
        if engine.is_null() || out_doc.is_null() || ext_utf8.is_null() || (bytes.ptr.is_null() && bytes.len > 0) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Validated non-null pointers and byte length from C caller.
        let raw_slice = if bytes.len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) }
        };
        // SAFETY: `ext_utf8` verified non-null above.
        let Ok(ext) = unsafe { CStr::from_ptr(ext_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let mut session = DocumentSession::new_blank("Imported");
        match session.open_bytes(raw_slice, ext) {
            Ok(()) => {
                set_last_ffi_error("");
                // SAFETY: `out_doc` verified non-null above.
                unsafe {
                    *out_doc = Box::into_raw(Box::new(VexioDoc { session }));
                }
                VexioStatus::Ok
            }
            Err(e) => {
                set_last_ffi_error(e.to_string());
                VexioStatus::ErrIoCorruptFormat
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_save_format(
    doc: *mut VexioDoc,
    ext_utf8: *const c_char,
    out_buf: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || ext_utf8.is_null() || out_buf.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `ext_utf8` verified non-null.
        let doc_ref = unsafe { &*doc };
        let Ok(ext) = unsafe { CStr::from_ptr(ext_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match doc_ref.session.save_format(ext) {
            Ok(vec) => {
                set_last_ffi_error("");
                // SAFETY: `out_buf` verified non-null.
                unsafe { *out_buf = VexioOwnedBuffer::from_vec(vec) };
                VexioStatus::Ok
            }
            Err(e) => {
                set_last_ffi_error(e.to_string());
                VexioStatus::ErrIoCorruptFormat
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_destroy(doc: *mut VexioDoc) {
    if !doc.is_null() {
        // SAFETY: Allocated by `Box::into_raw` in `vexio_doc_create_blank` or `vexio_doc_open_bytes`.
        drop(unsafe { Box::from_raw(doc) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_buffer_free(buf: VexioOwnedBuffer) {
    if !buf.ptr.is_null() && buf.cap > 0 {
        // SAFETY: Reconstructs the `Vec<u8>` originally forgotten in `VexioOwnedBuffer::from_vec`.
        drop(unsafe { Vec::from_raw_parts(buf.ptr, buf.len, buf.cap) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_last_error_message(
    out_utf8: *mut VexioOwnedBuffer,
) -> VexioStatus {
    if out_utf8.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    let msg = LAST_FFI_ERROR.with(|slot| slot.borrow().clone());
    // SAFETY: `out_utf8` verified non-null above.
    unsafe { *out_utf8 = VexioOwnedBuffer::from_vec(msg.into_bytes()) };
    VexioStatus::Ok
}

#[no_mangle]
pub unsafe extern "C" fn vexio_cell_get_formula(
    doc: *const VexioDoc,
    page_id: u32,
    shape_id: u64,
    cell_name: *const c_char,
    out_utf8: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || cell_name.is_null() || out_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let doc_ref = unsafe { &*doc };
        let Ok(cname) = unsafe { CStr::from_ptr(cell_name) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let cell_opt = doc_ref.session.doc.page(page_id).and_then(|p| {
            if shape_id == 0 {
                p.page_sheet.get_cell(cname)
            } else {
                p.find_shape(shape_id).and_then(|s| s.sheet.get_cell(cname))
            }
        });
        let Some(f) = cell_opt.map(|c| c.formula.clone()) else {
            return VexioStatus::ErrInvalidArg;
        };
        // SAFETY: `out_utf8` verified non-null.
        unsafe { *out_utf8 = VexioOwnedBuffer::from_vec(f.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_cell_get_value_str(
    doc: *const VexioDoc,
    page_id: u32,
    shape_id: u64,
    cell_name: *const c_char,
    out_utf8: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || cell_name.is_null() || out_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let doc_ref = unsafe { &*doc };
        let Ok(cname) = unsafe { CStr::from_ptr(cell_name) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let cell_opt = doc_ref.session.doc.page(page_id).and_then(|p| {
            if shape_id == 0 {
                p.page_sheet.get_cell(cname)
            } else {
                p.find_shape(shape_id).and_then(|s| s.sheet.get_cell(cname))
            }
        });
        let Some(val_str) = cell_opt.map(|c| {
            c.value
                .as_str_coerced()
                .unwrap_or_else(|e| e.as_str().to_string())
        }) else {
            return VexioStatus::ErrInvalidArg;
        };
        // SAFETY: `out_utf8` verified non-null.
        unsafe { *out_utf8 = VexioOwnedBuffer::from_vec(val_str.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_cell_set_formula(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    cell_name: *const c_char,
    formula_utf8: *const c_char,
    force_setf: bool,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || cell_name.is_null() || formula_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let doc_mut = unsafe { &mut *doc };
        let Ok(cname) = unsafe { CStr::from_ptr(cell_name) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Ok(fstr) = unsafe { CStr::from_ptr(formula_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match doc_mut
            .session
            .set_cell_formula(page_id, shape_id, cname, fstr, force_setf)
        {
            Ok(()) => VexioStatus::Ok,
            Err(CoreError::Journal(JournalError::CellGuarded(_))) => VexioStatus::ErrCellGuarded,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_create_headless(
    engine: *mut VexioEngine,
    doc: *mut VexioDoc,
    page_id: u32,
    width_px: u32,
    height_px: u32,
    dpi_scale: f32,
    out_viewport: *mut *mut VexioViewport,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() || doc.is_null() || out_viewport.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_viewport` verified non-null.
        let doc_ref = unsafe { &*doc };
        let vp = VexioViewport::new_base(&doc_ref.session.doc, page_id, width_px, height_px, dpi_scale);
        unsafe { *out_viewport = Box::into_raw(Box::new(vp)) };
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_create_dx12_swapchain(
    engine: *mut VexioEngine,
    doc: *mut VexioDoc,
    dxgi_swapchain_panel_unk: *mut std::ffi::c_void,
    width_px: u32,
    height_px: u32,
    dpi_scale: f32,
    out_viewport: *mut *mut VexioViewport,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() || doc.is_null() || dxgi_swapchain_panel_unk.is_null() || out_viewport.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let doc_ref = unsafe { &*doc };
        #[allow(unused_mut)]
        let mut vp = VexioViewport::new_base(&doc_ref.session.doc, 1, width_px, height_px, dpi_scale);
        #[cfg(target_os = "windows")]
        {
            // SAFETY: Caller passes a valid WinUI 3 `SwapChainPanel` (`IUnknown*`) outliving `vp`.
            if let Ok(mut renderer) = unsafe {
                vexio_render::WgpuCanvasRenderer::new_dx12_swapchain_panel(
                    dxgi_swapchain_panel_unk,
                    width_px.max(1),
                    height_px.max(1),
                    dpi_scale,
                )
            } {
                renderer.camera = vp.camera;
                let _ = renderer.render_surface_frame(&doc_ref.session.doc, vp.page_id);
                vp.gpu_renderer = Some(renderer);
            }
        }
        // SAFETY: `out_viewport` verified non-null.
        unsafe { *out_viewport = Box::into_raw(Box::new(vp)) };
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_create_metal_layer(
    engine: *mut VexioEngine,
    doc: *mut VexioDoc,
    ca_metal_layer_ptr: *mut std::ffi::c_void,
    width_px: u32,
    height_px: u32,
    dpi_scale: f32,
    out_viewport: *mut *mut VexioViewport,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if engine.is_null() || doc.is_null() || ca_metal_layer_ptr.is_null() || out_viewport.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let doc_ref = unsafe { &*doc };
        #[allow(unused_mut)]
        let mut vp = VexioViewport::new_base(&doc_ref.session.doc, 1, width_px, height_px, dpi_scale);
        #[cfg(target_os = "macos")]
        {
            // SAFETY: Caller passes a valid retained `CAMetalLayer*` outliving `vp`.
            if let Ok(mut renderer) = unsafe {
                vexio_render::WgpuCanvasRenderer::new_metal_layer(
                    ca_metal_layer_ptr,
                    width_px.max(1),
                    height_px.max(1),
                    dpi_scale,
                )
            } {
                renderer.camera = vp.camera;
                let _ = renderer.render_surface_frame(&doc_ref.session.doc, vp.page_id);
                vp.gpu_renderer = Some(renderer);
            }
        }
        // SAFETY: `out_viewport` verified non-null.
        unsafe { *out_viewport = Box::into_raw(Box::new(vp)) };
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_destroy(vp: *mut VexioViewport) {
    if !vp.is_null() {
        // SAFETY: Allocated by `Box::into_raw`.
        drop(unsafe { Box::from_raw(vp) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_resize(
    vp: *mut VexioViewport,
    width_px: u32,
    height_px: u32,
    dpi_scale: f32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let w = width_px.max(1);
        let h = height_px.max(1);
        let dpi = dpi_scale.max(0.5) as f64;
        vp_mut.camera.width_px = w;
        vp_mut.camera.height_px = h;
        vp_mut.camera.dpi_scale = dpi;
        if let Some(r) = &mut vp_mut.gpu_renderer {
            r.resize_surface(w, h, dpi);
        }
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_set_tool(
    vp: *mut VexioViewport,
    tool_code: u32,
) -> VexioStatus {
    if vp.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    // SAFETY: `vp` verified non-null.
    let vp_mut = unsafe { &mut *vp };
    vp_mut.active_tool = NativeCanvasTool::from_code(tool_code);
    vp_mut.drag_state = None;
    VexioStatus::Ok
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_set_page(
    vp: *mut VexioViewport,
    doc: *const VexioDoc,
    page_id: u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_ref = unsafe { &*doc };
        let target_page = doc_ref
            .session
            .doc
            .page(page_id)
            .or_else(|| doc_ref.session.doc.pages.iter().find(|p| !p.is_background))
            .or_else(|| doc_ref.session.doc.pages.first());
        let resolved_id = target_page.map(|p| p.id).unwrap_or(page_id);
        vp_mut.page_id = resolved_id;
        vp_mut.selected_ids.clear();
        fit_camera_to_diagram(&mut vp_mut.camera, &doc_ref.session.doc, resolved_id);
        vp_mut.pending_fit = vp_mut.camera.width_px <= 64 || vp_mut.camera.height_px <= 64;
        vp_mut.present_if_attached(&doc_ref.session.doc);
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_set_zoom(
    vp: *mut VexioViewport,
    doc: *const VexioDoc,
    zoom_scale: f64,
    pan_x_in: f64,
    pan_y_in: f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_ref = unsafe { &*doc };
        if zoom_scale <= 0.0 {
            fit_camera_to_diagram(&mut vp_mut.camera, &doc_ref.session.doc, vp_mut.page_id);
            vp_mut.pending_fit = vp_mut.camera.width_px <= 64 || vp_mut.camera.height_px <= 64;
        } else {
            vp_mut.pending_fit = false;
            vp_mut.camera.zoom_ppi = 96.0 * zoom_scale.clamp(0.1, 16.0);
            if pan_x_in.is_finite() && pan_y_in.is_finite() && (pan_x_in != 0.0 || pan_y_in != 0.0) {
                vp_mut.camera.center_x_in = pan_x_in;
                vp_mut.camera.center_y_in = pan_x_in * 0.0 + pan_y_in;
            }
        }
        vp_mut.present_if_attached(&doc_ref.session.doc);
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_render_frame(
    vp: *mut VexioViewport,
    doc: *const VexioDoc,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_ref = unsafe { &*doc };
        vp_mut.present_if_attached(&doc_ref.session.doc);
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_render_rgba(
    vp: *const VexioViewport,
    doc: *const VexioDoc,
    out_rgba: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || doc.is_null() || out_rgba.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp`, `doc`, and `out_rgba` verified non-null.
        let vp_ref = unsafe { &*vp };
        let doc_ref = unsafe { &*doc };
        match vexio_render::render_page_software_rgba_with_overlay(
            &doc_ref.session.doc,
            vp_ref.page_id,
            &vp_ref.camera,
            true,
            &vp_ref.selected_ids,
            Some(&vp_ref.overlay),
        ) {
            Ok(rgba) => {
                // SAFETY: `out_rgba` verified non-null.
                unsafe { *out_rgba = VexioOwnedBuffer::from_vec(rgba) };
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrGpuSurfaceLost,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_get_primary_selection(vp: *const VexioViewport) -> u64 {
    if vp.is_null() {
        return 0;
    }
    // SAFETY: `vp` verified non-null.
    let vp_ref = unsafe { &*vp };
    vp_ref.selected_ids.first().copied().unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_dispatch_pointer(
    vp: *mut VexioViewport,
    doc: *mut VexioDoc,
    ev: *const VexioPointerEvent,
    out_cursor_kind: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || doc.is_null() || ev.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp`, `doc`, and `ev` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_mut = unsafe { &mut *doc };
        let ev_val = unsafe { *ev };
        let cursor = vp_mut.dispatch_pointer_internal(&mut doc_mut.session, ev_val);
        if !out_cursor_kind.is_null() {
            // SAFETY: `out_cursor_kind` checked non-null.
            unsafe { ptr::write(out_cursor_kind, cursor) };
        }
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_render_preview_rgba(
    doc: *const VexioDoc,
    page_id: u32,
    width_px: u32,
    height_px: u32,
    out_rgba: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_rgba.is_null() || width_px == 0 || height_px == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let doc_ref = unsafe { &*doc };
        match vexio_render::render_page_rgba_auto(
            &doc_ref.session.doc,
            page_id,
            width_px,
            height_px,
        ) {
            Ok(rgba) => {
                // SAFETY: `out_rgba` verified non-null.
                unsafe { *out_rgba = VexioOwnedBuffer::from_vec(rgba) };
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrGpuSurfaceLost,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_list_stencils_json(
    doc: *const VexioDoc,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let doc_ref = unsafe { &*doc };
        let json = build_stencils_catalog_json(Some(&doc_ref.session));
        // SAFETY: `out_json` verified non-null.
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_load_starter_template(
    doc: *mut VexioDoc,
    template_id_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || template_id_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `template_id_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(tid) = unsafe { CStr::from_ptr(template_id_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        d.session.load_starter_template(tid);
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_get_state_json(
    doc: *const VexioDoc,
    active_page_id: u32,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_json` verified non-null.
        let d = unsafe { &*doc };
        let json = build_native_state_json(&d.session, active_page_id);
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_drop_stencil(
    doc: *mut VexioDoc,
    page_id: u32,
    category_utf8: *const c_char,
    master_name_utf8: *const c_char,
    x_in: f64,
    y_in: f64,
    out_shape_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || master_name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let d = unsafe { &mut *doc };
        let cat = if category_utf8.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(category_utf8) }.to_str().unwrap_or("")
        };
        let Ok(master_name) = unsafe { CStr::from_ptr(master_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.drop_builtin_shape(page_id, cat, master_name, x_in, y_in) {
            Ok(sid) => {
                if !out_shape_id.is_null() {
                    // SAFETY: `out_shape_id` checked non-null.
                    unsafe { *out_shape_id = sid };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_draw_primitive(
    doc: *mut VexioDoc,
    page_id: u32,
    kind_utf8: *const c_char,
    x1_in: f64,
    y1_in: f64,
    x2_in: f64,
    y2_in: f64,
    out_shape_id: *mut u64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || kind_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(kind) = unsafe { CStr::from_ptr(kind_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.draw_primitive_2d(page_id, kind, x1_in, y1_in, x2_in, y2_in) {
            Ok(sid) => {
                if !out_shape_id.is_null() {
                    // SAFETY: `out_shape_id` checked non-null.
                    unsafe { *out_shape_id = sid };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_connect_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    from_shape_id: u64,
    to_shape_id: u64,
    out_conn_id: *mut u64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.connect_shapes(page_id, from_shape_id, to_shape_id) {
            Ok(cid) => {
                if !out_conn_id.is_null() {
                    // SAFETY: `out_conn_id` checked non-null.
                    unsafe { *out_conn_id = cid };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    out_deleted_count: *mut u32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || shape_ids_ptr.is_null() || shape_ids_len == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `shape_ids_ptr[..shape_ids_len]` verified non-null.
        let d = unsafe { &mut *doc };
        let ids = unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) };
        match d.session.delete_shapes(page_id, ids) {
            Ok(n) => {
                if !out_deleted_count.is_null() {
                    // SAFETY: `out_deleted_count` checked non-null.
                    unsafe { *out_deleted_count = n as u32 };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_duplicate_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    dx_in: f64,
    dy_in: f64,
    out_first_id: *mut u64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || shape_ids_ptr.is_null() || shape_ids_len == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `shape_ids_ptr[..shape_ids_len]` verified non-null.
        let d = unsafe { &mut *doc };
        let ids = unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) };
        match d.session.duplicate_shapes(page_id, ids, dx_in, dy_in) {
            Ok(created) => {
                if !out_first_id.is_null() {
                    // SAFETY: `out_first_id` checked non-null.
                    unsafe { *out_first_id = created.first().copied().unwrap_or(0) };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_reorder_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    mode_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || shape_ids_ptr.is_null() || shape_ids_len == 0 || mode_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc`, `shape_ids_ptr[..shape_ids_len]`, and `mode_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let ids = unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) };
        let Ok(mode) = unsafe { CStr::from_ptr(mode_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.reorder_shapes(page_id, ids, mode) {
            Ok(_) => VexioStatus::Ok,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_shape_text(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    text_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || text_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `text_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(txt) = unsafe { CStr::from_ptr(text_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.set_shape_text(page_id, shape_id, txt) {
            Ok(()) => VexioStatus::Ok,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_add_page(
    doc: *mut VexioDoc,
    name_utf8: *const c_char,
    out_page_id: *mut u32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `name_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(name) = unsafe { CStr::from_ptr(name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.add_page(name) {
            Ok(pid) => {
                if !out_page_id.is_null() {
                    // SAFETY: `out_page_id` checked non-null.
                    unsafe { *out_page_id = pid };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_auto_layout(
    doc: *mut VexioDoc,
    page_id: u32,
    algo_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || algo_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `algo_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(algo) = unsafe { CStr::from_ptr(algo_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        d.session.layout(page_id, algo);
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_validate_json(
    doc: *const VexioDoc,
    ruleset_utf8: *const c_char,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if doc.is_null() || ruleset_utf8.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let d = unsafe { &*doc };
        let Ok(rs) = unsafe { CStr::from_ptr(ruleset_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let issues = d.session.validate(rs);
        let json_bytes = serde_json::to_vec(&issues).unwrap_or_else(|_| b"[]".to_vec());
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json_bytes) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_transform_shape(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    pin_x_in: f64,
    pin_y_in: f64,
    width_in: f64,
    height_in: f64,
    angle_deg: f64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .transform_shape(page_id, shape_id, pin_x_in, pin_y_in, width_in, height_in, angle_deg)
        {
            Ok(()) => VexioStatus::Ok,
            Err(CoreError::Journal(JournalError::CellGuarded(_))) => VexioStatus::ErrCellGuarded,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_import_stencil_bytes(
    doc: *mut VexioDoc,
    file_name_utf8: *const c_char,
    bytes: VexioByteSlice,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || file_name_utf8.is_null() || bytes.ptr.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null; `bytes.ptr[..bytes.len]` provided by caller.
        let d = unsafe { &mut *doc };
        let Ok(fname) = unsafe { CStr::from_ptr(file_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let raw = unsafe { std::slice::from_raw_parts(bytes.ptr, bytes.len) };
        match d.session.import_stencil_archive_packs(fname, raw) {
            Ok(packs) => {
                set_last_ffi_error("");
                let first_title = packs
                    .first()
                    .map(|(t, _)| t.clone())
                    .unwrap_or_else(|| "Imported Visio Stencil".into());
                let mut all_previews = Vec::new();
                let mut packs_json = Vec::with_capacity(packs.len());
                for (pt, pv) in packs {
                    let p_names: Vec<String> = pv.iter().map(|p| p.name.clone()).collect();
                    packs_json.push(serde_json::json!({
                        "packTitle": pt,
                        "masterNames": p_names,
                        "masters": pv,
                        "previews": pv,
                    }));
                    all_previews.extend(pv);
                }
                let master_names: Vec<String> = all_previews.iter().map(|p| p.name.clone()).collect();
                let payload = serde_json::json!({
                    "packTitle": first_title,
                    "masterNames": master_names,
                    "masters": all_previews,
                    "previews": all_previews,
                    "packs": packs_json,
                });
                let json_bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| b"{}".to_vec());
                unsafe { *out_json = VexioOwnedBuffer::from_vec(json_bytes) };
                VexioStatus::Ok
            }
            Err(e) => {
                set_last_ffi_error(e.to_string());
                VexioStatus::ErrIoCorruptFormat
            }
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_list_official_stencil_catalog_json(
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let catalog = crate::domains::list_official_stencil_catalog();
        let json_bytes = serde_json::to_vec(&catalog).unwrap_or_else(|_| b"[]".to_vec());
        // SAFETY: `out_json` verified non-null above.
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json_bytes) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_undo(doc: *mut VexioDoc) -> VexioStatus {
    if doc.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    // SAFETY: `doc` checked non-null.
    let d = unsafe { &mut *doc };
    match d.session.journal.undo(&mut d.session.doc) {
        Ok(_) => VexioStatus::Ok,
        Err(_) => VexioStatus::ErrInvalidArg,
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_redo(doc: *mut VexioDoc) -> VexioStatus {
    if doc.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    // SAFETY: `doc` checked non-null.
    let d = unsafe { &mut *doc };
    match d.session.journal.redo(&mut d.session.doc) {
        Ok(_) => VexioStatus::Ok,
        Err(_) => VexioStatus::ErrInvalidArg,
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_boolean_op(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    op_utf8: *const c_char,
    offset_in: f64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || shape_ids_ptr.is_null() || shape_ids_len == 0 || op_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Caller guarantees `doc`, `shape_ids_ptr[..shape_ids_len]`, and `op_utf8` are valid.
        let d = unsafe { &mut *doc };
        let ids = unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) };
        let Ok(op_str) = unsafe { CStr::from_ptr(op_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let kind = match op_str.to_ascii_lowercase().as_str() {
            "union" => crate::boolean_ops::BooleanOpKind::Union,
            "combine" => crate::boolean_ops::BooleanOpKind::Combine,
            "fragment" => crate::boolean_ops::BooleanOpKind::Fragment,
            "intersect" => crate::boolean_ops::BooleanOpKind::Intersect,
            "subtract" => crate::boolean_ops::BooleanOpKind::Subtract,
            "trim" => crate::boolean_ops::BooleanOpKind::Trim,
            "join" => crate::boolean_ops::BooleanOpKind::Join,
            "offset" => crate::boolean_ops::BooleanOpKind::Offset(offset_in),
            _ => return VexioStatus::ErrInvalidArg,
        };
        let _ = d
            .session
            .journal
            .record_mutation(&mut d.session.doc, format!("Boolean {op_str}"), |doc_mut| {
                crate::boolean_ops::apply_boolean_operation(doc_mut, page_id, ids, kind);
            });
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_apply_theme(
    doc: *mut VexioDoc,
    theme_slug_utf8: *const c_char,
    variant_index: u8,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || theme_slug_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(slug) = unsafe { CStr::from_ptr(theme_slug_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let _ = d
            .session
            .journal
            .record_mutation(&mut d.session.doc, "Apply Theme", |doc_mut| {
                crate::themes_styles::apply_document_theme_variant(doc_mut, slug, variant_index);
            });
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_create_subprocess(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    sub_name_utf8: *const c_char,
    out_sub_page_id: *mut u32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || sub_name_utf8.is_null() || out_sub_page_id.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(name) = unsafe { CStr::from_ptr(sub_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let mut created_pid = None;
        let _ = d
            .session
            .journal
            .record_mutation(&mut d.session.doc, "Create Subprocess", |doc_mut| {
                created_pid = crate::enterprise_plan2::create_or_link_subprocess_page(
                    doc_mut, page_id, shape_id, name,
                );
            });
        let Some(pid) = created_pid else {
            return VexioStatus::ErrInvalidArg;
        };
        // SAFETY: `out_sub_page_id` verified non-null.
        unsafe { *out_sub_page_id = pid };
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_compare(
    base_doc: *const VexioDoc,
    rev_doc: *const VexioDoc,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if base_doc.is_null() || rev_doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let b = unsafe { &*base_doc };
        let r = unsafe { &*rev_doc };
        let report = crate::enterprise_plan2::compare_documents(&b.session.doc, &r.session.doc);
        let json_bytes = serde_json::to_vec(&report).unwrap_or_default();
        // SAFETY: `out_json` verified non-null.
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json_bytes) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_apply_crdt_json(
    doc: *mut VexioDoc,
    replica_id_utf8: *const c_char,
    ops_json_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || replica_id_utf8.is_null() || ops_json_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(rep_id) = unsafe { CStr::from_ptr(replica_id_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Ok(ops_json) = unsafe { CStr::from_ptr(ops_json_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let mut crdt = crate::crdt::CrdtDocumentState::new(rep_id);
        match crdt.apply_batch_json(&mut d.session.doc, ops_json) {
            Ok(_) => VexioStatus::Ok,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

pub(crate) fn visual_preset_spec_from_slug(
    preset: &str,
    shape_id: u64,
) -> crate::themes_styles::ShapeVisualStyleSpec {
    use crate::themes_styles::{
        BevelExtrusionSpec, DropShadowSpec, GradientDirection, GradientFillSpec, HatchFillSpec,
        HatchPatternKind, ShapeVisualStyleSpec, SketchOutlineSpec,
    };
    use vexio_shapesheet::RgbaColor;

    match preset {
        "grad_linear" => ShapeVisualStyleSpec {
            gradient: Some(GradientFillSpec {
                direction: GradientDirection::LinearDiagonal,
                foregnd: RgbaColor { r: 191, g: 219, b: 254, a: 255 },
                bkgnd: RgbaColor { r: 239, g: 246, b: 255, a: 255 },
                angle_deg: 45.0,
            }),
            ..Default::default()
        },
        "grad_radial" => ShapeVisualStyleSpec {
            gradient: Some(GradientFillSpec {
                direction: GradientDirection::RadialCenter,
                foregnd: RgbaColor { r: 167, g: 243, b: 208, a: 255 },
                bkgnd: RgbaColor { r: 240, g: 253, b: 244, a: 255 },
                angle_deg: 0.0,
            }),
            ..Default::default()
        },
        "hatch_diag" => ShapeVisualStyleSpec {
            hatch: Some(HatchFillSpec {
                kind: HatchPatternKind::UpwardDiagonal,
                foregnd: RgbaColor { r: 59, g: 130, b: 246, a: 255 },
                bkgnd: RgbaColor { r: 248, g: 250, b: 252, a: 255 },
            }),
            ..Default::default()
        },
        "shadow_soft" => ShapeVisualStyleSpec {
            shadow: Some(DropShadowSpec {
                enabled: true,
                foregnd: RgbaColor { r: 15, g: 23, b: 42, a: 255 },
                offset_x_in: 0.06,
                offset_y_in: -0.06,
                blur_pt: 4.5,
            }),
            ..Default::default()
        },
        "bevel_3d" => ShapeVisualStyleSpec {
            bevel: Some(BevelExtrusionSpec {
                bevel_top_type: 1,
                extrusion_depth_in: 0.14,
                extrusion_angle_deg: 35.0,
            }),
            ..Default::default()
        },
        "sketchy" => ShapeVisualStyleSpec {
            sketch: Some(SketchOutlineSpec {
                enabled: true,
                seed: (shape_id as u32).max(1),
                amount: 1.0,
            }),
            ..Default::default()
        },
        _ => ShapeVisualStyleSpec::default(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_apply_visual_preset(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    preset_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || preset_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `preset_utf8` verified non-null for the duration of this call.
        let d = unsafe { &mut *doc };
        // SAFETY: `preset_utf8` is a NUL-terminated UTF-8 string supplied by the native caller.
        let Ok(preset) = unsafe { CStr::from_ptr(preset_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let spec = visual_preset_spec_from_slug(preset, shape_id);
        let mut applied = false;
        let _ = d
            .session
            .journal
            .record_mutation(&mut d.session.doc, "Apply Visual Preset", |doc_mut| {
                applied =
                    crate::themes_styles::apply_visual_effects_to_shape(doc_mut, page_id, shape_id, &spec);
            });
        if applied {
            VexioStatus::Ok
        } else {
            VexioStatus::ErrInvalidArg
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_flush_crdt_json(
    prev_doc: *const VexioDoc,
    curr_doc: *const VexioDoc,
    replica_id_utf8: *const c_char,
    now_ms: u64,
    out_ops_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if prev_doc.is_null()
            || curr_doc.is_null()
            || replica_id_utf8.is_null()
            || out_ops_json.is_null()
        {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `prev_doc` and `curr_doc` verified non-null.
        let prev = unsafe { &*prev_doc };
        let curr = unsafe { &*curr_doc };
        // SAFETY: `replica_id_utf8` verified non-null NUL-terminated C string.
        let Ok(rep_id) = unsafe { CStr::from_ptr(replica_id_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let mut crdt = crate::crdt::CrdtDocumentState::new(rep_id);
        let ops = crdt.diff_to_ops(&prev.session.doc, &curr.session.doc, now_ms.max(1));
        let json_bytes = serde_json::to_vec(&ops).unwrap_or_else(|_| b"[]".to_vec());
        // SAFETY: `out_ops_json` verified non-null writable pointer.
        unsafe { *out_ops_json = VexioOwnedBuffer::from_vec(json_bytes) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_import_data_visualizer(
    doc: *mut VexioDoc,
    page_id: u32,
    csv_or_xlsx: VexioByteSlice,
    out_step_count: *mut u32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || csv_or_xlsx.ptr.is_null() || csv_or_xlsx.len == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `csv_or_xlsx.ptr[..len]` verified non-null and valid.
        let d = unsafe { &mut *doc };
        let bytes = unsafe { std::slice::from_raw_parts(csv_or_xlsx.ptr, csv_or_xlsx.len) };
        let mut imported = None;
        let _ = d.session.journal.record_mutation(
            &mut d.session.doc,
            "Import Data Visualizer",
            |doc_mut| {
                imported = crate::plan2_connectors::import_data_visualizer_csv_or_xlsx(
                    doc_mut, page_id, bytes,
                )
                .ok();
            },
        );
        match imported {
            Some(count) => {
                if !out_step_count.is_null() {
                    // SAFETY: `out_step_count` checked non-null.
                    unsafe { *out_step_count = count as u32 };
                }
                VexioStatus::Ok
            }
            None => VexioStatus::ErrIoCorruptFormat,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_export_data_visualizer_xlsx(
    doc: *const VexioDoc,
    page_id: u32,
    out_xlsx: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if doc.is_null() || out_xlsx.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &*doc };
        match crate::plan2_connectors::export_data_visualizer_xlsx(&d.session.doc, page_id) {
            Ok(xlsx) => {
                // SAFETY: `out_xlsx` verified non-null.
                unsafe { *out_xlsx = VexioOwnedBuffer::from_vec(xlsx) };
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrIoCorruptFormat,
        }
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_import_cloud_discovery(
    doc: *mut VexioDoc,
    page_id: u32,
    discovery_json_utf8: *const c_char,
    out_count: *mut u32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || discovery_json_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `discovery_json_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(json_str) = unsafe { CStr::from_ptr(discovery_json_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let mut imported = None;
        let _ = d.session.journal.record_mutation(
            &mut d.session.doc,
            "Import Cloud Discovery",
            |doc_mut| {
                imported =
                    crate::plan2_connectors::import_cloud_discovery_json(doc_mut, page_id, json_str)
                        .ok();
            },
        );
        match imported {
            Some(count) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` checked non-null.
                    unsafe { *out_count = count as u32 };
                }
                VexioStatus::Ok
            }
            None => VexioStatus::ErrIoCorruptFormat,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_import_entra_or_ldif(
    doc: *mut VexioDoc,
    page_id: u32,
    text_utf8: *const c_char,
    out_count: *mut u32,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || text_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `text_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(text_str) = unsafe { CStr::from_ptr(text_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let mut imported = None;
        let _ = d.session.journal.record_mutation(
            &mut d.session.doc,
            "Import Directory Org Chart",
            |doc_mut| {
                imported =
                    crate::plan2_connectors::import_entra_or_ldif_org_chart(doc_mut, page_id, text_str)
                        .ok();
            },
        );
        match imported {
            Some(count) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` checked non-null.
                    unsafe { *out_count = count as u32 };
                }
                VexioStatus::Ok
            }
            None => VexioStatus::ErrIoCorruptFormat,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_get_active_tool(vp: *const VexioViewport) -> u32 {
    if vp.is_null() {
        return 0;
    }
    // SAFETY: `vp` verified non-null.
    let vp_ref = unsafe { &*vp };
    vp_ref.active_tool as u32
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_pop_created_text_shape(vp: *mut VexioViewport) -> u64 {
    if vp.is_null() {
        return 0;
    }
    // SAFETY: `vp` verified non-null.
    let vp_mut = unsafe { &mut *vp };
    vp_mut.last_created_text_shape_id.take().unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_hit_test_shape(
    vp: *const VexioViewport,
    doc: *const VexioDoc,
    x_px: f64,
    y_px: f64,
) -> u64 {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() || doc.is_null() {
            return 0u64;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_ref = unsafe { &*vp };
        let doc_ref = unsafe { &*doc };
        let page_pt = vp_ref.camera.screen_to_page_in(x_px, y_px);
        let visuals = ReactiveEngine::evaluate_page_visuals(&doc_ref.session.doc, vp_ref.page_id);
        let index = vexio_render::SpatialIndex::build(&visuals);
        let resolve_top = |hid: u64| -> u64 {
            doc_ref
                .session
                .doc
                .page(vp_ref.page_id)
                .and_then(|p| p.top_level_shape_id(hid))
                .unwrap_or(hid)
        };
        if let Some(hit_id) = index
            .hit_test_visuals(&visuals, page_pt[0], page_pt[1], 0.10)
            .into_iter()
            .next()
        {
            return resolve_top(hit_id);
        }
        // Fallback: hit-test external/below-icon stencil text captions (`vis.txt_pos`) topmost-first
        for vis in visuals.iter().rev() {
            let cap_half_w = (vis.transform.width * 0.5 + 0.18).max(0.92);
            if (page_pt[0] - vis.txt_pos[0]).abs() <= cap_half_w
                && (page_pt[1] - vis.txt_pos[1]).abs() <= 0.24
            {
                return resolve_top(vis.shape_id);
            }
        }
        0
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_get_shape_screen_bounds(
    vp: *const VexioViewport,
    doc: *const VexioDoc,
    shape_id: u64,
    out_x_px: *mut f64,
    out_y_px: *mut f64,
    out_w_px: *mut f64,
    out_h_px: *mut f64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null()
            || doc.is_null()
            || out_x_px.is_null()
            || out_y_px.is_null()
            || out_w_px.is_null()
            || out_h_px.is_null()
        {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null above.
        let vp_ref = unsafe { &*vp };
        let doc_ref = unsafe { &*doc };
        let visuals = ReactiveEngine::evaluate_page_visuals(&doc_ref.session.doc, vp_ref.page_id);
        let Some(vis) = visuals.iter().find(|v| v.shape_id == shape_id) else {
            return VexioStatus::ErrInvalidArg;
        };
        let dpi = vp_ref.camera.dpi_scale.max(1.0);
        let tl = vp_ref.camera.page_in_to_screen(vis.aabb_min[0], vis.aabb_max[1]);
        let br = vp_ref.camera.page_in_to_screen(vis.aabb_max[0], vis.aabb_min[1]);
        let shape_w = (br[0] - tl[0]).abs();
        let shape_h = (br[1] - tl[1]).abs();

        // If the stencil's text block (`txt_pos`) sits outside/below the icon body (e.g. Microsoft 365,
        // Azure, Servers, Network, BPMN Events/Gateways), position the in-place text editor directly over `txt_pos`!
        let external_caption = vis.txt_pos[1] < vis.aabb_min[1] + 0.02
            || vis.txt_pos[1] > vis.aabb_max[1] - 0.02;
        let (min_x, min_y, w, h) = if external_caption {
            let tc = vp_ref.camera.page_in_to_screen(vis.txt_pos[0], vis.txt_pos[1]);
            let cap_w = (shape_w + 36.0 * dpi).max(148.0 * dpi);
            let cap_h = (26.0 * dpi).max(vis.font_size_pt * 2.1 * dpi);
            (tc[0] - cap_w * 0.5, tc[1] - cap_h * 0.5, cap_w, cap_h)
        } else {
            let box_w = shape_w.max(110.0 * dpi);
            let box_h = shape_h.max(26.0 * dpi);
            let mid_x = (tl[0] + br[0]) * 0.5;
            let mid_y = (tl[1] + br[1]) * 0.5;
            (mid_x - box_w * 0.5, mid_y - box_h * 0.5, box_w, box_h)
        };
        // SAFETY: Output pointers checked non-null.
        unsafe {
            *out_x_px = min_x;
            *out_y_px = min_y;
            *out_w_px = w;
            *out_h_px = h;
        }
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_shape_text_style_json(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    style_json_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null()
            || style_json_utf8.is_null()
            || (shape_ids_ptr.is_null() && shape_ids_len > 0)
        {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null above.
        let d = unsafe { &mut *doc };
        let ids = if shape_ids_len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        let Ok(json_s) = unsafe { CStr::from_ptr(style_json_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Ok(patch) = serde_json::from_str::<crate::workspace::TextStylePatch>(json_s) else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.apply_text_style_patch(page_id, ids, &patch) {
            Ok(_) => VexioStatus::Ok,
            Err(CoreError::Journal(JournalError::CellGuarded(_))) => VexioStatus::ErrCellGuarded,
            Err(_) => VexioStatus::ErrFormulaSyntax,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_rotate_flip_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    delta_angle_deg: f64,
    toggle_flip_x: bool,
    toggle_flip_y: bool,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || (shape_ids_ptr.is_null() && shape_ids_len > 0) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null above.
        let d = unsafe { &mut *doc };
        let ids = if shape_ids_len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        match d.session.rotate_or_flip_shapes(
            page_id,
            ids,
            delta_angle_deg,
            toggle_flip_x,
            toggle_flip_y,
        ) {
            Ok(_) => VexioStatus::Ok,
            Err(CoreError::Journal(JournalError::CellGuarded(_))) => VexioStatus::ErrCellGuarded,
            Err(_) => VexioStatus::ErrFormulaSyntax,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_page_setup(
    doc: *mut VexioDoc,
    page_id: u32,
    width_in: f64,
    height_in: f64,
    unit_slug_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || !width_in.is_finite() || !height_in.is_finite() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        let unit_s = if unit_slug_utf8.is_null() {
            "mm"
        } else {
            // SAFETY: `unit_slug_utf8` verified non-null.
            unsafe { CStr::from_ptr(unit_slug_utf8) }
                .to_str()
                .unwrap_or("mm")
        };
        match d.session.set_page_setup(page_id, width_in, height_in, unit_s) {
            Ok(()) => VexioStatus::Ok,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_page_preset(
    doc: *mut VexioDoc,
    page_id: u32,
    preset_slug_utf8: *const c_char,
    landscape: bool,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || preset_slug_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `preset_slug_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(slug) = unsafe { CStr::from_ptr(preset_slug_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Some((w_in, h_in, unit)) =
            crate::workspace::resolve_page_preset_dimensions_in(slug, landscape)
        else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.set_page_setup(page_id, w_in, h_in, unit) {
            Ok(()) => VexioStatus::Ok,
            Err(_) => VexioStatus::ErrInvalidArg,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_title(
    doc: *mut VexioDoc,
    title_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || title_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `title_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(title) = unsafe { CStr::from_ptr(title_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        d.session.set_document_title(title);
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_rename_page(
    doc: *mut VexioDoc,
    page_id: u32,
    new_name_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc.is_null() || new_name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `new_name_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(name) = unsafe { CStr::from_ptr(new_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        if d.session.rename_page(page_id, name) {
            VexioStatus::Ok
        } else {
            VexioStatus::ErrInvalidArg
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_duplicate_page(
    doc: *mut VexioDoc,
    page_id: u32,
    out_new_page_id: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(new_id) = d.session.duplicate_page(page_id) else {
            return VexioStatus::ErrInvalidArg;
        };
        if !out_new_page_id.is_null() {
            // SAFETY: `out_new_page_id` verified non-null.
            unsafe { *out_new_page_id = new_id };
        }
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_page(doc: *mut VexioDoc, page_id: u32) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        if d.session.delete_page(page_id) {
            VexioStatus::Ok
        } else {
            VexioStatus::ErrInvalidArg
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_page_background(
    doc: *mut VexioDoc,
    page_id: u32,
    is_background: bool,
    back_page_id: u32,
    page_color_formula_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let color_str = if page_color_formula_utf8.is_null() {
            ""
        } else {
            // SAFETY: `page_color_formula_utf8` verified non-null.
            let Ok(s) = unsafe { CStr::from_ptr(page_color_formula_utf8) }.to_str() else {
                return VexioStatus::ErrInvalidArg;
            };
            s
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        if d.session
            .set_page_background(page_id, is_background, back_page_id, color_str)
        {
            VexioStatus::Ok
        } else {
            VexioStatus::ErrInvalidArg
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_apply_heading_preset(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    preset_slug_utf8: *const c_char,
    out_modified_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || preset_slug_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let ids: &[u64] = if shape_ids_ptr.is_null() || shape_ids_len == 0 {
            &[]
        } else {
            // SAFETY: `shape_ids_ptr` verified non-null with length `shape_ids_len`.
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        // SAFETY: `preset_slug_utf8` verified non-null.
        let Ok(preset_str) = unsafe { CStr::from_ptr(preset_slug_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.apply_text_heading_preset(page_id, ids, preset_str) {
            Ok(n) => {
                if !out_modified_count.is_null() {
                    // SAFETY: `out_modified_count` verified non-null.
                    unsafe { *out_modified_count = n as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_insert_heading(
    doc: *mut VexioDoc,
    page_id: u32,
    preset_slug_utf8: *const c_char,
    custom_text_utf8: *const c_char,
    x_in: f64,
    y_in: f64,
    out_shape_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || preset_slug_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `preset_slug_utf8` verified non-null.
        let Ok(preset_str) = unsafe { CStr::from_ptr(preset_slug_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let custom_opt = if custom_text_utf8.is_null() {
            None
        } else {
            // SAFETY: `custom_text_utf8` verified non-null.
            unsafe { CStr::from_ptr(custom_text_utf8) }.to_str().ok()
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .insert_text_heading(page_id, preset_str, custom_opt, x_in, y_in)
        {
            Ok(sid) => {
                if !out_shape_id.is_null() {
                    // SAFETY: `out_shape_id` verified non-null.
                    unsafe { *out_shape_id = sid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_apply_border_and_title(
    doc: *mut VexioDoc,
    page_id: u32,
    style_slug_utf8: *const c_char,
    title_utf8: *const c_char,
    subtitle_utf8: *const c_char,
    author_utf8: *const c_char,
    date_or_rev_utf8: *const c_char,
    out_bg_page_id: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || style_slug_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let read_opt_str = |p: *const c_char| -> &str {
            if p.is_null() {
                ""
            } else {
                // SAFETY: `p` verified non-null.
                unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
            }
        };
        let style_s = read_opt_str(style_slug_utf8);
        let title_s = read_opt_str(title_utf8);
        let sub_s = read_opt_str(subtitle_utf8);
        let author_s = read_opt_str(author_utf8);
        let date_s = read_opt_str(date_or_rev_utf8);
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .apply_border_and_title(page_id, style_s, title_s, sub_s, author_s, date_s)
        {
            Ok(bg_pid) => {
                if !out_bg_page_id.is_null() {
                    // SAFETY: `out_bg_page_id` verified non-null.
                    unsafe { *out_bg_page_id = bg_pid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_import_markup(
    doc: *mut VexioDoc,
    page_id: u32,
    markup_utf8: *const c_char,
    format_slug_utf8: *const c_char,
    out_nodes_created: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || markup_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `markup_utf8` verified non-null.
        let Ok(markup_s) = unsafe { CStr::from_ptr(markup_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let fmt_s = if format_slug_utf8.is_null() {
            "auto"
        } else {
            // SAFETY: `format_slug_utf8` verified non-null.
            unsafe { CStr::from_ptr(format_slug_utf8) }
                .to_str()
                .unwrap_or("auto")
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.import_diagram_markup(page_id, markup_s, fmt_s) {
            Ok(stats) => {
                if !out_nodes_created.is_null() {
                    // SAFETY: `out_nodes_created` verified non-null.
                    unsafe { *out_nodes_created = stats.nodes_created as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_export_markup(
    doc: *const VexioDoc,
    page_id: u32,
    format_slug_utf8: *const c_char,
    out_markup_utf8: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_markup_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let fmt_s = if format_slug_utf8.is_null() {
            "mermaid"
        } else {
            // SAFETY: `format_slug_utf8` verified non-null.
            unsafe { CStr::from_ptr(format_slug_utf8) }
                .to_str()
                .unwrap_or("mermaid")
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &*doc };
        match d.session.export_diagram_markup(page_id, fmt_s) {
            Ok(text) => {
                // SAFETY: `out_markup_utf8` verified non-null.
                unsafe { *out_markup_utf8 = VexioOwnedBuffer::from_vec(text.into_bytes()) };
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_list_slide_snippets_json(
    doc: *const VexioDoc,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_json` verified non-null.
        let d = unsafe { &*doc };
        let json = d.session.list_slide_snippets_json();
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn vexio_doc_add_slide_snippet(
    doc: *mut VexioDoc,
    page_id: u32,
    title_utf8: *const c_char,
    notes_utf8: *const c_char,
    x_in: f64,
    y_in: f64,
    width_in: f64,
    height_in: f64,
    out_snippet_id: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let title_s = if title_utf8.is_null() {
            ""
        } else {
            // SAFETY: `title_utf8` verified non-null.
            unsafe { CStr::from_ptr(title_utf8) }.to_str().unwrap_or("")
        };
        let notes_s = if notes_utf8.is_null() {
            ""
        } else {
            // SAFETY: `notes_utf8` verified non-null.
            unsafe { CStr::from_ptr(notes_utf8) }.to_str().unwrap_or("")
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .add_slide_snippet(page_id, title_s, notes_s, x_in, y_in, width_in, height_in)
        {
            Ok(id) => {
                if !out_snippet_id.is_null() {
                    // SAFETY: `out_snippet_id` verified non-null.
                    unsafe { *out_snippet_id = id };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_slide_snippet(
    doc: *mut VexioDoc,
    page_id: u32,
    snippet_id: u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.delete_slide_snippet(page_id, snippet_id) {
            Ok(_) => VexioStatus::Ok,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_auto_slide_snippets(
    doc: *mut VexioDoc,
    page_id: u32,
    out_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.auto_generate_slide_snippets(page_id) {
            Ok(frames) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` verified non-null.
                    unsafe { *out_count = frames.len() as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_reset_connector_routing(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || shape_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.reset_connector_routing(page_id, shape_id) {
            Ok(_) => VexioStatus::Ok,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_ensure_control_handle(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || shape_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.ensure_control_handle(page_id, shape_id) {
            Ok(_) => VexioStatus::Ok,
            Err(e) => map_core_err(e),
        }
    })
}

// ============================================================================
// v0.6.0-beta C-ABI: VBA Macro IDE, Sub-Shape Vertices, Review Comments & Ink,
//                    Hyperlinks/ScreenTips, Enterprise Domain Wizards & CAD Tools
// ============================================================================

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_run_vba_macro(
    doc: *mut VexioDoc,
    selection_ptr: *const u64,
    selection_len: usize,
    script_utf8: *const c_char,
    out_report_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || script_utf8.is_null() || out_report_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc`, `script_utf8`, and `out_report_json` verified non-null; `selection_ptr` guarded by `selection_len`.
        let d = unsafe { &mut *doc };
        let sel: &[u64] = if selection_ptr.is_null() || selection_len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(selection_ptr, selection_len) }
        };
        let Ok(script) = unsafe { CStr::from_ptr(script_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.run_vba_macro_with_selection(sel, script) {
            Ok(json) => {
                // SAFETY: `out_report_json` verified non-null.
                unsafe { *out_report_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_list_vertices_json(
    doc: *const VexioDoc,
    page_id: u32,
    shape_id: u64,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_json` verified non-null.
        let d = unsafe { &*doc };
        let json = d.session.list_shape_vertices_json(page_id, shape_id);
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_move_vertex(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    geom_index: u32,
    row_ix: u32,
    page_x_in: f64,
    page_y_in: f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || shape_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.move_shape_vertex(page_id, shape_id, geom_index, row_ix, page_x_in, page_y_in) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_insert_vertex(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    geom_index: u32,
    after_row_ix: u32,
    page_x_in: f64,
    page_y_in: f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || shape_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.insert_shape_vertex(page_id, shape_id, geom_index, after_row_ix, page_x_in, page_y_in) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_vertex(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    geom_index: u32,
    row_ix: u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || shape_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.delete_shape_vertex(page_id, shape_id, geom_index, row_ix) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_draw_freeform_json(
    doc: *mut VexioDoc,
    page_id: u32,
    points_json_utf8: *const c_char,
    closed: bool,
    out_shape_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || points_json_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `points_json_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(pjson) = unsafe { CStr::from_ptr(points_json_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.draw_freeform_curve_json(page_id, pjson, closed) {
            Ok(sid) => {
                if !out_shape_id.is_null() {
                    // SAFETY: `out_shape_id` verified non-null.
                    unsafe { *out_shape_id = sid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_list_comments_json(
    doc: *const VexioDoc,
    page_id: u32,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_json` verified non-null.
        let d = unsafe { &*doc };
        let json = d.session.list_comments_json(page_id);
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_add_comment(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    author_utf8: *const c_char,
    text_utf8: *const c_char,
    x_in: f64,
    y_in: f64,
    out_comment_id: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || text_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `text_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let author = if author_utf8.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(author_utf8) }.to_str().unwrap_or("")
        };
        let Ok(text) = unsafe { CStr::from_ptr(text_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.add_comment(page_id, shape_id, author, text, x_in, y_in) {
            Ok(cid) => {
                if !out_comment_id.is_null() {
                    unsafe { *out_comment_id = cid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_resolve_comment(
    doc: *mut VexioDoc,
    comment_id: u32,
    resolved: bool,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || comment_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.set_comment_resolved(comment_id, resolved) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_comment(
    doc: *mut VexioDoc,
    comment_id: u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || comment_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.delete_comment(comment_id) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_clear_page_ink(
    doc: *mut VexioDoc,
    page_id: u32,
    out_removed_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.clear_page_ink(page_id) {
            Ok(cnt) => {
                if !out_removed_count.is_null() {
                    unsafe { *out_removed_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_hyperlink_and_screentip(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    description_utf8: *const c_char,
    address_utf8: *const c_char,
    sub_address_utf8: *const c_char,
    screentip_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || shape_id == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null; optional string pointers guarded for null.
        let d = unsafe { &mut *doc };
        let read_opt = |p: *const c_char| -> &str {
            if p.is_null() {
                ""
            } else {
                unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
            }
        };
        match d.session.set_shape_hyperlink_and_screentip(
            page_id,
            shape_id,
            read_opt(description_utf8),
            read_opt(address_utf8),
            read_opt(sub_address_utf8),
            read_opt(screentip_utf8),
        ) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_run_domain_wizard(
    doc: *mut VexioDoc,
    page_id: u32,
    wizard_slug_utf8: *const c_char,
    spec_utf8: *const c_char,
    out_result_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || wizard_slug_utf8.is_null() || out_result_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc`, `wizard_slug_utf8`, and `out_result_json` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(slug) = unsafe { CStr::from_ptr(wizard_slug_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let spec = if spec_utf8.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(spec_utf8) }.to_str().unwrap_or("")
        };
        match d.session.run_domain_wizard(page_id, slug, spec) {
            Ok(json) => {
                unsafe { *out_result_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_apply_cad_scale(
    doc: *mut VexioDoc,
    page_id: u32,
    scale_slug_utf8: *const c_char,
    locked: bool,
    out_ratio: *mut f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || scale_slug_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `scale_slug_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(slug) = unsafe { CStr::from_ptr(scale_slug_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.apply_cad_scale(page_id, slug, locked) {
            Ok(r) => {
                if !out_ratio.is_null() {
                    unsafe { *out_ratio = r };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_convert_cad_layers(
    doc: *mut VexioDoc,
    page_id: u32,
    layer_name_or_all_utf8: *const c_char,
    out_converted_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        let layer_s = if layer_name_or_all_utf8.is_null() {
            "*"
        } else {
            unsafe { CStr::from_ptr(layer_name_or_all_utf8) }
                .to_str()
                .unwrap_or("*")
        };
        match d.session.convert_cad_layers(page_id, layer_s) {
            Ok(cnt) => {
                if !out_converted_count.is_null() {
                    unsafe { *out_converted_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_register_font_bytes(
    font_bytes: VexioByteSlice,
    family_override_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if font_bytes.ptr.is_null() || font_bytes.len == 0 {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `font_bytes.ptr` verified non-null with positive `font_bytes.len`.
        let slice = unsafe { std::slice::from_raw_parts(font_bytes.ptr, font_bytes.len) };
        let fam_opt = if family_override_utf8.is_null() {
            None
        } else {
            // SAFETY: `family_override_utf8` verified non-null NUL-terminated C string.
            unsafe { CStr::from_ptr(family_override_utf8) }.to_str().ok()
        };
        if DocumentSession::register_font_bytes(slice, fam_opt) {
            VexioStatus::Ok
        } else {
            VexioStatus::ErrIoCorruptFormat
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_align_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    mode_utf8: *const c_char,
    out_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || mode_utf8.is_null() || (shape_ids_len > 0 && shape_ids_ptr.is_null()) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc`, `mode_utf8`, and `shape_ids_ptr` verified valid.
        let d = unsafe { &mut *doc };
        let Ok(mode) = unsafe { CStr::from_ptr(mode_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let ids: &[u64] = if shape_ids_len == 0 {
            &[]
        } else {
            // SAFETY: `shape_ids_ptr` is non-null for `shape_ids_len` `u64` elements.
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        match d.session.align_shapes(page_id, ids, mode) {
            Ok(cnt) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` verified non-null.
                    unsafe { *out_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_distribute_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    axis_utf8: *const c_char,
    out_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || axis_utf8.is_null() || (shape_ids_len > 0 && shape_ids_ptr.is_null()) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc`, `axis_utf8`, and `shape_ids_ptr` verified valid.
        let d = unsafe { &mut *doc };
        let Ok(axis) = unsafe { CStr::from_ptr(axis_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let ids: &[u64] = if shape_ids_len == 0 {
            &[]
        } else {
            // SAFETY: `shape_ids_ptr` verified non-null for `shape_ids_len` elements.
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        match d.session.distribute_shapes(page_id, ids, axis) {
            Ok(cnt) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` verified non-null.
                    unsafe { *out_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_group_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    out_group_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || (shape_ids_len > 0 && shape_ids_ptr.is_null()) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `shape_ids_ptr` verified valid.
        let d = unsafe { &mut *doc };
        let ids: &[u64] = if shape_ids_len == 0 {
            &[]
        } else {
            // SAFETY: `shape_ids_ptr` verified non-null for `shape_ids_len` elements.
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        match d.session.group_shapes(page_id, ids) {
            Ok(gid) => {
                if !out_group_id.is_null() {
                    // SAFETY: `out_group_id` verified non-null.
                    unsafe { *out_group_id = gid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_ungroup_shapes(
    doc: *mut VexioDoc,
    page_id: u32,
    group_ids_ptr: *const u64,
    group_ids_len: usize,
    out_ids_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || (group_ids_len > 0 && group_ids_ptr.is_null()) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `group_ids_ptr` verified valid.
        let d = unsafe { &mut *doc };
        let gids: &[u64] = if group_ids_len == 0 {
            &[]
        } else {
            // SAFETY: `group_ids_ptr` verified non-null for `group_ids_len` elements.
            unsafe { std::slice::from_raw_parts(group_ids_ptr, group_ids_len) }
        };
        match d.session.ungroup_shapes(page_id, gids) {
            Ok(restored) => {
                if !out_ids_json.is_null() {
                    let json = serde_json::to_vec(&restored).unwrap_or_else(|_| b"[]".to_vec());
                    // SAFETY: `out_ids_json` verified non-null.
                    unsafe { *out_ids_json = VexioOwnedBuffer::from_vec(json) };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_drag_connector_segment(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    seg_index: u32,
    coord_in: f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || !coord_in.is_finite() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .drag_connector_segment(page_id, shape_id, seg_index as usize, coord_in)
        {
            Ok(_) => VexioStatus::Ok,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_set_smart_guides(
    vp: *mut VexioViewport,
    enabled: bool,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        vp_mut.smart_guides_enabled = enabled;
        if !enabled {
            vp_mut.overlay.smart_guides.clear();
        }
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_get_smart_guides(vp: *const VexioViewport) -> bool {
    catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() {
            return false;
        }
        // SAFETY: `vp` verified non-null.
        unsafe { (*vp).smart_guides_enabled }
    }))
    .unwrap_or(false)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_get_selection_json(
    vp: *const VexioViewport,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `out_json` verified non-null.
        let vp_ref = unsafe { &*vp };
        let json = serde_json::to_vec(&vp_ref.selected_ids).unwrap_or_else(|_| b"[]".to_vec());
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_set_selection(
    vp: *mut VexioViewport,
    doc: *const VexioDoc,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || (shape_ids_len > 0 && shape_ids_ptr.is_null()) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `shape_ids_ptr` verified valid.
        let vp_mut = unsafe { &mut *vp };
        let ids: Vec<u64> = if shape_ids_len == 0 {
            Vec::new()
        } else {
            // SAFETY: `shape_ids_ptr` verified non-null for `shape_ids_len` elements.
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }.to_vec()
        };
        if !doc.is_null() {
            // SAFETY: `doc` verified non-null.
            let doc_ref = unsafe { &*doc };
            if let Some(page) = doc_ref.session.doc.page(vp_mut.page_id) {
                let mut normalized = Vec::new();
                for id in ids {
                    if let Some(top_id) = page.top_level_shape_id(id) {
                        if !normalized.contains(&top_id) {
                            normalized.push(top_id);
                        }
                    }
                }
                vp_mut.selected_ids = normalized;
            } else {
                vp_mut.selected_ids = ids;
            }
            vp_mut.present_if_attached(&doc_ref.session.doc);
        } else {
            vp_mut.selected_ids = ids;
        }
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_screen_to_page(
    vp: *const VexioViewport,
    x_px: f64,
    y_px: f64,
    out_x_in: *mut f64,
    out_y_in: *mut f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if vp.is_null() || out_x_in.is_null() || out_y_in.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp`, `out_x_in`, and `out_y_in` verified non-null.
        let vp_ref = unsafe { &*vp };
        let pt = if x_px.is_finite() && y_px.is_finite() && (x_px >= 0.0 || y_px >= 0.0) {
            vp_ref.camera.screen_to_page_in(x_px, y_px)
        } else {
            [vp_ref.camera.center_x_in, vp_ref.camera.center_y_in]
        };
        unsafe {
            *out_x_in = pt[0];
            *out_y_in = pt[1];
        }
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_insert_container(
    doc: *mut VexioDoc,
    page_id: u32,
    title_utf8: *const c_char,
    member_ids_ptr: *const u64,
    member_ids_len: usize,
    out_container_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || (member_ids_len > 0 && member_ids_ptr.is_null()) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and optional `member_ids_ptr` / `title_utf8` verified valid.
        let d = unsafe { &mut *doc };
        let title = if title_utf8.is_null() {
            "Container"
        } else {
            unsafe { CStr::from_ptr(title_utf8) }
                .to_str()
                .unwrap_or("Container")
        };
        let member_ids: &[u64] = if member_ids_len == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(member_ids_ptr, member_ids_len) }
        };
        match d.session.insert_container(page_id, title, member_ids) {
            Ok(cid) => {
                if !out_container_id.is_null() {
                    unsafe { *out_container_id = cid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_insert_swimlanes(
    doc: *mut VexioDoc,
    page_id: u32,
    lanes_semicolon_utf8: *const c_char,
    out_lane_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null; `lanes_semicolon_utf8` null-checked below.
        let d = unsafe { &mut *doc };
        let lanes = if lanes_semicolon_utf8.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(lanes_semicolon_utf8) }
                .to_str()
                .unwrap_or("")
        };
        match d.session.insert_swimlanes(page_id, lanes) {
            Ok(ids) => {
                if !out_lane_count.is_null() {
                    unsafe { *out_lane_count = ids.len() as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_attach_callout(
    doc: *mut VexioDoc,
    page_id: u32,
    target_shape_id: u64,
    text_utf8: *const c_char,
    out_callout_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null; `text_utf8` null-checked below.
        let d = unsafe { &mut *doc };
        let text = if text_utf8.is_null() {
            "Callout Note"
        } else {
            unsafe { CStr::from_ptr(text_utf8) }
                .to_str()
                .unwrap_or("Callout Note")
        };
        match d.session.insert_callout(page_id, target_shape_id, text) {
            Ok(cid) => {
                if !out_callout_id.is_null() {
                    unsafe { *out_callout_id = cid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_auto_connect(
    doc: *mut VexioDoc,
    page_id: u32,
    source_shape_id: u64,
    dir_utf8: *const c_char,
    out_target_shape_id: *mut u64,
    out_conn_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || dir_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `dir_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(dir_str) = unsafe { CStr::from_ptr(dir_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Some(dir) = vexio_render::AutoConnectDir::from_slug(dir_str) else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.auto_connect_shape(page_id, source_shape_id, dir) {
            Ok((tid, cid)) => {
                if !out_target_shape_id.is_null() {
                    unsafe { *out_target_shape_id = tid };
                }
                if !out_conn_id.is_null() {
                    unsafe { *out_conn_id = cid };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_drag_control_handle(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    row_name_utf8: *const c_char,
    page_x_in: f64,
    page_y_in: f64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || row_name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `row_name_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(row_str) = unsafe { CStr::from_ptr(row_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d
            .session
            .drag_control_handle(page_id, shape_id, row_str, page_x_in, page_y_in)
        {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_rename_shape(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    new_name_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || new_name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `new_name_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(name_str) = unsafe { CStr::from_ptr(new_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.rename_shape(page_id, shape_id, name_str) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_upsert_shape_data(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    row_name_utf8: *const c_char,
    label_utf8: *const c_char,
    data_type: u8,
    value_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || row_name_utf8.is_null() || value_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc`, `row_name_utf8`, and `value_utf8` verified non-null (`label_utf8` may be null).
        let d = unsafe { &mut *doc };
        let Ok(r_str) = unsafe { CStr::from_ptr(row_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let lbl_str = if label_utf8.is_null() {
            ""
        } else {
            unsafe { CStr::from_ptr(label_utf8) }
                .to_str()
                .unwrap_or("")
        };
        let Ok(val_str) = unsafe { CStr::from_ptr(value_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d
            .session
            .upsert_shape_data(page_id, shape_id, r_str, lbl_str, data_type, val_str)
        {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_shape_data(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    row_name_utf8: *const c_char,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || row_name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `row_name_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(r_str) = unsafe { CStr::from_ptr(row_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.delete_shape_data(page_id, shape_id, r_str) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_add_layer(
    doc: *mut VexioDoc,
    page_id: u32,
    layer_name_utf8: *const c_char,
    out_layer_ix: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || layer_name_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `layer_name_utf8` verified non-null.
        let d = unsafe { &mut *doc };
        let Ok(l_str) = unsafe { CStr::from_ptr(layer_name_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        match d.session.add_layer(page_id, l_str) {
            Ok(ix) => {
                if !out_layer_ix.is_null() {
                    // SAFETY: `out_layer_ix` verified non-null.
                    unsafe { *out_layer_ix = ix };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_layer_flags(
    doc: *mut VexioDoc,
    page_id: u32,
    layer_ix: u32,
    visible: bool,
    lock_flag: bool,
    print_flag: bool,
    snap_glue: bool,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .set_layer_flags(page_id, layer_ix, visible, lock_flag, print_flag, snap_glue)
        {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_shape_layer_membership(
    doc: *mut VexioDoc,
    page_id: u32,
    shape_id: u64,
    layer_ix: u32,
    assigned: bool,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .set_shape_layer_membership(page_id, shape_id, layer_ix, assigned)
        {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

// ============================================================================
// Pillar #4 C-ABI: Live Data Graphics, Legends & Two-Way External Data Linking
// ============================================================================

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_get_data_link_state_json(
    doc: *const VexioDoc,
    page_id: u32,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_json` verified non-null.
        let d = unsafe { &*doc };
        let json = d.session.get_page_data_link_state_json(page_id);
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_set_page_data_graphics_json(
    doc: *mut VexioDoc,
    page_id: u32,
    config_json_utf8: *const c_char,
    out_decorated_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || config_json_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `config_json_utf8` verified non-null NUL-terminated UTF-8 string; `doc` verified non-null.
        let Ok(cfg_str) = unsafe { CStr::from_ptr(config_json_utf8) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let d = unsafe { &mut *doc };
        match d.session.set_page_data_graphics_json(page_id, cfg_str) {
            Ok(cnt) => {
                if !out_decorated_count.is_null() {
                    // SAFETY: `out_decorated_count` verified non-null.
                    unsafe { *out_decorated_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_import_external_recordset(
    doc: *mut VexioDoc,
    page_id: u32,
    file_name_utf8: *const c_char,
    data_bytes: VexioByteSlice,
    key_col_utf8: *const c_char,
    table_name_utf8: *const c_char,
    auto_bind: bool,
    auto_enable_graphics: bool,
    out_recordset_id: *mut u32,
    out_linked_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || (data_bytes.ptr.is_null() && data_bytes.len > 0) {
            return VexioStatus::ErrInvalidArg;
        }
        let fname = if file_name_utf8.is_null() {
            "external_data.csv"
        } else {
            // SAFETY: `file_name_utf8` verified non-null.
            unsafe { CStr::from_ptr(file_name_utf8) }
                .to_str()
                .unwrap_or("external_data.csv")
        };
        let key_col = if key_col_utf8.is_null() {
            ""
        } else {
            // SAFETY: `key_col_utf8` verified non-null.
            unsafe { CStr::from_ptr(key_col_utf8) }
                .to_str()
                .unwrap_or("")
        };
        let table_name = if table_name_utf8.is_null() {
            ""
        } else {
            // SAFETY: `table_name_utf8` verified non-null.
            unsafe { CStr::from_ptr(table_name_utf8) }
                .to_str()
                .unwrap_or("")
        };
        // SAFETY: `data_bytes` verified non-null when `len > 0`.
        let slice = if data_bytes.len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(data_bytes.ptr, data_bytes.len) }
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.import_external_recordset(
            page_id,
            fname,
            slice,
            key_col,
            table_name,
            auto_bind,
            auto_enable_graphics,
        ) {
            Ok((rs_id, linked)) => {
                if !out_recordset_id.is_null() {
                    // SAFETY: `out_recordset_id` verified non-null.
                    unsafe { *out_recordset_id = rs_id };
                }
                if !out_linked_count.is_null() {
                    // SAFETY: `out_linked_count` verified non-null.
                    unsafe { *out_linked_count = linked as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_bind_external_row(
    doc: *mut VexioDoc,
    page_id: u32,
    recordset_id: u32,
    row_index: u32,
    shape_ids_ptr: *const u64,
    shape_ids_len: usize,
    out_bound_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || (shape_ids_ptr.is_null() && shape_ids_len > 0) {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `shape_ids_ptr` verified non-null when `shape_ids_len > 0`.
        let shape_ids = if shape_ids_len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(shape_ids_ptr, shape_ids_len) }
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .bind_external_row_to_shapes(page_id, recordset_id, row_index as usize, shape_ids)
        {
            Ok(cnt) => {
                if !out_bound_count.is_null() {
                    // SAFETY: `out_bound_count` verified non-null.
                    unsafe { *out_bound_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_refresh_external_recordset(
    doc: *mut VexioDoc,
    page_id: u32,
    recordset_id: u32,
    optional_new_bytes: VexioByteSlice,
    format_hint_utf8: *const c_char,
    out_refreshed_count: *mut u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || (optional_new_bytes.ptr.is_null() && optional_new_bytes.len > 0) {
            return VexioStatus::ErrInvalidArg;
        }
        let fmt_hint = if format_hint_utf8.is_null() {
            ""
        } else {
            // SAFETY: `format_hint_utf8` verified non-null.
            unsafe { CStr::from_ptr(format_hint_utf8) }
                .to_str()
                .unwrap_or("")
        };
        let new_bytes_opt = if optional_new_bytes.len == 0 || optional_new_bytes.ptr.is_null() {
            None
        } else {
            // SAFETY: `optional_new_bytes` verified non-null with `len > 0`.
            Some(unsafe { std::slice::from_raw_parts(optional_new_bytes.ptr, optional_new_bytes.len) })
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .refresh_external_recordset(page_id, recordset_id, new_bytes_opt, fmt_hint)
        {
            Ok(cnt) => {
                if !out_refreshed_count.is_null() {
                    // SAFETY: `out_refreshed_count` verified non-null.
                    unsafe { *out_refreshed_count = cnt as u32 };
                }
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_sync_and_export_recordset(
    doc: *mut VexioDoc,
    page_id: u32,
    recordset_id: u32,
    format_ext_utf8: *const c_char,
    out_bytes: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_bytes.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let fmt = if format_ext_utf8.is_null() {
            "xlsx"
        } else {
            // SAFETY: `format_ext_utf8` verified non-null.
            unsafe { CStr::from_ptr(format_ext_utf8) }
                .to_str()
                .unwrap_or("xlsx")
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.sync_and_export_recordset(page_id, recordset_id, fmt) {
            Ok(bytes) => {
                // SAFETY: `out_bytes` verified non-null.
                unsafe { *out_bytes = VexioOwnedBuffer::from_vec(bytes) };
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_delete_external_recordset(
    doc: *mut VexioDoc,
    recordset_id: u32,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d.session.delete_external_recordset(recordset_id) {
            Ok(true) => VexioStatus::Ok,
            Ok(false) => VexioStatus::ErrInvalidArg,
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_insert_data_legend(
    doc: *mut VexioDoc,
    page_id: u32,
    prop_name_utf8: *const c_char,
    pin_x_in: f64,
    pin_y_in: f64,
    out_legend_id: *mut u64,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let prop = if prop_name_utf8.is_null() {
            ""
        } else {
            // SAFETY: `prop_name_utf8` verified non-null.
            unsafe { CStr::from_ptr(prop_name_utf8) }
                .to_str()
                .unwrap_or("")
        };
        let px = if pin_x_in > 0.0 { Some(pin_x_in) } else { None };
        let py = if pin_y_in > 0.0 { Some(pin_y_in) } else { None };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &mut *doc };
        match d
            .session
            .insert_or_update_data_legend(page_id, prop, px, py)
        {
            Ok(lid) => {
                if !out_legend_id.is_null() {
                    // SAFETY: `out_legend_id` verified non-null.
                    unsafe { *out_legend_id = lid };
                }
                if lid > 0 {
                    VexioStatus::Ok
                } else {
                    VexioStatus::ErrInvalidArg
                }
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_export_bom_report(
    doc: *const VexioDoc,
    format_ext_utf8: *const c_char,
    out_bytes: *mut VexioOwnedBuffer,
) -> VexioStatus {
    catch_ffi_status(|| {
        if doc.is_null() || out_bytes.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let fmt = if format_ext_utf8.is_null() {
            "xlsx"
        } else {
            // SAFETY: `format_ext_utf8` verified non-null.
            unsafe { CStr::from_ptr(format_ext_utf8) }
                .to_str()
                .unwrap_or("xlsx")
        };
        // SAFETY: `doc` verified non-null.
        let d = unsafe { &*doc };
        match d.session.export_bom_report_bytes(fmt) {
            Ok(bytes) => {
                // SAFETY: `out_bytes` verified non-null.
                unsafe { *out_bytes = VexioOwnedBuffer::from_vec(bytes) };
                VexioStatus::Ok
            }
            Err(e) => map_core_err(e),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn c_abi_plan2_visual_presets_connectors_and_crdt_flush_roundtrip() {
        let title = CString::new("Native C-ABI v0.1").unwrap();
        // SAFETY: `title` is a valid NUL-terminated CString; returned pointer is freed via `vexio_doc_destroy`.
        unsafe {
            let base_doc = vexio_doc_create_blank(title.as_ptr());
            let work_doc = vexio_doc_create_blank(title.as_ptr());
            assert!(!base_doc.is_null() && !work_doc.is_null());

            let dv_csv = b"Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Owner\nA1,Ingest,A2,,Start,Ops,SRE\nA2,Deploy,,,End,Eng,Dev";
            let mut step_count = 0u32;
            assert_eq!(
                vexio_doc_import_data_visualizer(
                    work_doc,
                    1,
                    VexioByteSlice {
                        ptr: dv_csv.as_ptr(),
                        len: dv_csv.len(),
                    },
                    &mut step_count,
                ),
                VexioStatus::Ok
            );
            assert_eq!(step_count, 2);

            let mut xlsx_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_export_data_visualizer_xlsx(work_doc, 1, &mut xlsx_buf),
                VexioStatus::Ok
            );
            assert!(xlsx_buf.len > 200);
            vexio_buffer_free(xlsx_buf);

            let preset = CString::new("shadow_soft").unwrap();
            let first_sid = (*work_doc).session.doc.page(1).unwrap().shapes[0].id;
            assert_eq!(
                vexio_doc_apply_visual_preset(work_doc, 1, first_sid, preset.as_ptr()),
                VexioStatus::Ok
            );

            let replica = CString::new("winui-peer").unwrap();
            let mut ops_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_flush_crdt_json(base_doc, work_doc, replica.as_ptr(), 5_000, &mut ops_buf),
                VexioStatus::Ok
            );
            assert!(ops_buf.len > 10);
            vexio_buffer_free(ops_buf);

            vexio_doc_destroy(work_doc);
            vexio_doc_destroy(base_doc);
        }
    }

    #[test]
    fn c_abi_stencils_drawing_primitives_interactive_pointer_and_rgba_viewport() {
        let title = CString::new("Authoring Test").unwrap();
        // SAFETY: All pointers are created and destroyed within this test scope.
        unsafe {
            let eng = vexio_engine_create();
            let doc = vexio_doc_create_blank(title.as_ptr());
            let mut vp: *mut VexioViewport = ptr::null_mut();
            assert_eq!(
                vexio_viewport_create_headless(eng, doc, 1, 640, 480, 1.0, &mut vp),
                VexioStatus::Ok
            );

            let mut stencils_buf = VexioOwnedBuffer {
                ptr: ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_list_stencils_json(doc, &mut stencils_buf),
                VexioStatus::Ok
            );
            let stencils_str =
                std::str::from_utf8(std::slice::from_raw_parts(stencils_buf.ptr, stencils_buf.len))
                    .unwrap();
            assert!(stencils_str.contains("Basic Shapes"));
            assert!(stencils_str.contains("Arrows & Block Arrows"));
            assert!(stencils_str.contains("Callouts, Notes & Containers"));
            assert!(stencils_str.contains("Right Block Arrow"));
            vexio_buffer_free(stencils_buf);

            let cat = CString::new("Basic Shapes & Arrows").unwrap();
            let master = CString::new("Diamond").unwrap();
            let mut sid1 = 0u64;
            assert_eq!(
                vexio_doc_drop_stencil(doc, 1, cat.as_ptr(), master.as_ptr(), 3.0, 4.0, &mut sid1),
                VexioStatus::Ok
            );
            assert!(sid1 > 0);

            for kind in ["rect", "round_rect", "ellipse", "triangle", "line", "arc", "text"] {
                let ckind = CString::new(kind).unwrap();
                let mut sid = 0u64;
                assert_eq!(
                    vexio_doc_draw_primitive(doc, 1, ckind.as_ptr(), 4.5, 3.5, 6.2, 4.8, &mut sid),
                    VexioStatus::Ok
                );
                assert!(sid > 0);
            }

            // Test interactive tool drawing (`Rectangle` = 4) via pointer Down -> Up
            assert_eq!(vexio_viewport_set_tool(vp, 4), VexioStatus::Ok);
            let mut cursor = 0u32;
            let down = VexioPointerEvent {
                kind: 1,
                buttons: 1,
                modifiers: 0,
                x_px: 120.0,
                y_px: 140.0,
                pressure: 0.8,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 1000,
            };
            let up = VexioPointerEvent {
                kind: 2,
                buttons: 0,
                modifiers: 0,
                x_px: 260.0,
                y_px: 240.0,
                pressure: 0.0,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 2000,
            };
            assert_eq!(
                vexio_viewport_dispatch_pointer(vp, doc, &down, &mut cursor),
                VexioStatus::Ok
            );
            assert_eq!(
                vexio_viewport_dispatch_pointer(vp, doc, &up, &mut cursor),
                VexioStatus::Ok
            );
            assert!(vexio_viewport_get_primary_selection(vp) > 0);

            let mut rgba_buf = VexioOwnedBuffer {
                ptr: ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_viewport_render_rgba(vp, doc, &mut rgba_buf),
                VexioStatus::Ok
            );
            assert_eq!(rgba_buf.len, 640 * 480 * 4);
            vexio_buffer_free(rgba_buf);

            vexio_viewport_destroy(vp);
            vexio_doc_destroy(doc);
            vexio_engine_destroy(eng);
        }
    }

    #[test]
    fn c_abi_text_formatting_rotate_iso_page_and_tool_autoreset() {
        let title = CString::new("Untitled Diagram").unwrap();
        // SAFETY: All FFI pointers are owned and destroyed within this test scope.
        unsafe {
            let eng = vexio_engine_create();
            let doc = vexio_doc_create_blank(title.as_ptr());
            let mut vp: *mut VexioViewport = ptr::null_mut();
            assert_eq!(
                vexio_viewport_create_headless(eng, doc, 1, 800, 600, 1.0, &mut vp),
                VexioStatus::Ok
            );

            // 1. ISO A4 Landscape Metric page preset (`297 x 210 mm`)
            let a4 = CString::new("a4").unwrap();
            assert_eq!(
                vexio_doc_set_page_preset(doc, 1, a4.as_ptr(), true),
                VexioStatus::Ok
            );
            let p = (&*doc).session.doc.page(1).unwrap();
            assert!((p.width_in() - 297.0 / 25.4).abs() < 1e-4);
            assert!((p.height_in() - 210.0 / 25.4).abs() < 1e-4);

            // 2. Rename document inline
            let new_title = CString::new("Architecture Overview").unwrap();
            assert_eq!(
                vexio_doc_set_title(doc, new_title.as_ptr()),
                VexioStatus::Ok
            );
            assert_eq!((&*doc).session.doc.properties.title, "Architecture Overview");

            // 3. Activate `Text` tool (`3`), single-click canvas -> auto-resets tool to `Select` (`0`)
            //    and records `last_created_text_shape_id` for immediate inline editing!
            assert_eq!(vexio_viewport_set_tool(vp, 3), VexioStatus::Ok);
            assert_eq!(vexio_viewport_get_active_tool(vp), 3);
            let mut cursor = 0u32;
            let click_down = VexioPointerEvent {
                kind: 1,
                buttons: 1,
                modifiers: 0,
                x_px: 380.0,
                y_px: 280.0,
                pressure: 0.8,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 100,
            };
            let click_up = VexioPointerEvent {
                kind: 2,
                buttons: 0,
                modifiers: 0,
                x_px: 382.0,
                y_px: 282.0,
                pressure: 0.0,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 200,
            };
            assert_eq!(
                vexio_viewport_dispatch_pointer(vp, doc, &click_down, &mut cursor),
                VexioStatus::Ok
            );
            assert_eq!(
                vexio_viewport_dispatch_pointer(vp, doc, &click_up, &mut cursor),
                VexioStatus::Ok
            );
            assert_eq!(vexio_viewport_get_active_tool(vp), 0);
            let created_txt_id = vexio_viewport_pop_created_text_shape(vp);
            assert!(created_txt_id > 0);
            assert_eq!(vexio_viewport_pop_created_text_shape(vp), 0);

            // 4. Hit-test & screen bounds for double-click inline editing
            let hit = vexio_viewport_hit_test_shape(vp, doc, 381.0, 281.0);
            assert_eq!(hit, created_txt_id);
            let (mut bx, mut by, mut bw, mut bh) = (0.0, 0.0, 0.0, 0.0);
            assert_eq!(
                vexio_viewport_get_shape_screen_bounds(
                    vp,
                    doc,
                    created_txt_id,
                    &mut bx,
                    &mut by,
                    &mut bw,
                    &mut bh,
                ),
                VexioStatus::Ok
            );
            assert!(bw >= 48.0 && bh >= 26.0);

            // 5. Rich text formatting (`Bold + Italic + Underline`, `18 pt`, `#1D4ED8`, Left Align, Top Align, Disc Bullets)
            let style_json = CString::new(
                r##"{"fontFamily":"Georgia","fontSizePt":18.0,"charStyle":7,"textColor":"#1D4ED8","textBgColor":"#FEF08A","horizAlign":0,"vertAlign":0,"bulletStyle":1}"##,
            )
            .unwrap();
            let ids = [created_txt_id];
            assert_eq!(
                vexio_doc_set_shape_text_style_json(doc, 1, ids.as_ptr(), 1, style_json.as_ptr()),
                VexioStatus::Ok
            );
            let svg = export_page_svg(&(&*doc).session.doc, 1);
            assert!(svg.contains("font-weight=\"bold\""));
            assert!(svg.contains("font-style=\"italic\""));
            assert!(svg.contains("text-decoration=\"underline\""));
            assert!(svg.contains("text-anchor=\"start\""));

            // 6. Drop a Microsoft stencil and rotate 90° + FlipX
            let cat = CString::new("Microsoft Servers & Active Directory").unwrap();
            let master = CString::new("Windows Server").unwrap();
            let mut srv_id = 0u64;
            assert_eq!(
                vexio_doc_drop_stencil(doc, 1, cat.as_ptr(), master.as_ptr(), 4.0, 4.0, &mut srv_id),
                VexioStatus::Ok
            );
            assert!(srv_id > 0);
            let srv_ids = [srv_id];
            assert_eq!(
                vexio_doc_rotate_flip_shapes(doc, 1, srv_ids.as_ptr(), 1, -90.0, true, false),
                VexioStatus::Ok
            );
            let srv_sh = (&*doc).session.doc.page(1).unwrap().find_shape(srv_id).unwrap();
            assert!((srv_sh.sheet.get_f64("Angle").unwrap().to_degrees() + 90.0).abs() < 1e-4);
            assert!(srv_sh.sheet.get_bool("FlipX").unwrap());

            vexio_viewport_destroy(vp);
            vexio_doc_destroy(doc);
            vexio_engine_destroy(eng);
        }
    }

    #[test]
    fn c_abi_all_62_functions_and_16_tools_smoke_test() {
        // SAFETY: All pointers created via `vexio_engine_create`, `vexio_doc_create_blank`,
        // `vexio_viewport_create_headless`, and stack-owned `CString` / slices are valid,
        // non-null (except deliberate null-arg error-path checks), properly aligned, owned
        // on the current test thread, and freed via `vexio_buffer_free` / `*_destroy`.
        unsafe {
            let take_utf8 = |buf: VexioOwnedBuffer| -> String {
                let slice = if buf.ptr.is_null() || buf.len == 0 {
                    &[][..]
                } else {
                    std::slice::from_raw_parts(buf.ptr, buf.len)
                };
                let s = String::from_utf8_lossy(slice).into_owned();
                vexio_buffer_free(buf);
                s
            };
            let take_bytes = |buf: VexioOwnedBuffer| -> Vec<u8> {
                let v = if buf.ptr.is_null() || buf.len == 0 {
                    Vec::new()
                } else {
                    std::slice::from_raw_parts(buf.ptr, buf.len).to_vec()
                };
                vexio_buffer_free(buf);
                v
            };

            let eng = vexio_engine_create();
            assert!(!eng.is_null());

            // Deliberate invalid-surface checks for DX12/Metal null surface pointers + error message readback
            let title_c = CString::new("Full C-ABI Smoke").unwrap();
            let doc = vexio_doc_create_blank(title_c.as_ptr());
            assert!(!doc.is_null());

            let mut null_vp: *mut VexioViewport = ptr::null_mut();
            assert_ne!(
                vexio_viewport_create_dx12_swapchain(
                    eng,
                    doc,
                    ptr::null_mut(),
                    800,
                    600,
                    1.0,
                    &mut null_vp
                ),
                VexioStatus::Ok
            );
            assert_ne!(
                vexio_viewport_create_metal_layer(
                    eng,
                    doc,
                    ptr::null_mut(),
                    800,
                    600,
                    1.0,
                    &mut null_vp
                ),
                VexioStatus::Ok
            );

            // Corrupt open check to exercise `vexio_last_error_message`
            let bad_bytes = b"not-a-valid-vsdx-zip";
            let mut bad_doc: *mut VexioDoc = ptr::null_mut();
            let vsdx_ext = CString::new("vsdx").unwrap();
            assert_eq!(
                vexio_doc_open_bytes(
                    eng,
                    VexioByteSlice {
                        ptr: bad_bytes.as_ptr(),
                        len: bad_bytes.len(),
                    },
                    vsdx_ext.as_ptr(),
                    &mut bad_doc
                ),
                VexioStatus::ErrIoCorruptFormat
            );
            let mut err_buf = VexioOwnedBuffer {
                ptr: ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(vexio_last_error_message(&mut err_buf), VexioStatus::Ok);
            let err_msg = take_utf8(err_buf);
            assert!(!err_msg.is_empty(), "vexio_last_error_message must return diagnostic text");

            // Baseline clone for CRDT & Diagram Compare
            let base_doc = vexio_doc_create_blank(title_c.as_ptr());
            assert!(!base_doc.is_null());

            // List built-in stencils JSON
            let mut stencils_buf = VexioOwnedBuffer {
                ptr: ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(vexio_doc_list_stencils_json(doc, &mut stencils_buf), VexioStatus::Ok);
            let stencils_json = take_utf8(stencils_buf);
            assert!(stencils_json.contains("BPMN 2.0") && stencils_json.contains("Basic Shapes"));

            // Starter template + Page setup + Page preset + Add/Rename page + PageSheet (shape_id == 0) cells
            for slug in ["bpmn2", "cad_floorplan", "flowchart", "blank"] {
                let slug_c = CString::new(slug).unwrap();
                assert_eq!(vexio_doc_load_starter_template(doc, slug_c.as_ptr()), VexioStatus::Ok);
            }
            let a3_c = CString::new("a3").unwrap();
            assert_eq!(vexio_doc_set_page_preset(doc, 1, a3_c.as_ptr(), true), VexioStatus::Ok);
            let mm_c = CString::new("mm").unwrap();
            assert_eq!(vexio_doc_set_page_setup(doc, 1, 11.6929, 8.2677, mm_c.as_ptr()), VexioStatus::Ok);

            // PageSheet cell access via shape_id == 0
            let page_color_cell = CString::new("PageColor").unwrap();
            let page_color_val = CString::new("RGB(248,250,252)").unwrap();
            assert_eq!(
                vexio_cell_set_formula(doc, 1, 0, page_color_cell.as_ptr(), page_color_val.as_ptr(), true),
                VexioStatus::Ok
            );
            let mut ps_f_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                vexio_cell_get_formula(doc, 1, 0, page_color_cell.as_ptr(), &mut ps_f_buf),
                VexioStatus::Ok
            );
            assert!(take_utf8(ps_f_buf).contains("248"));
            let mut ps_v_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                vexio_cell_get_value_str(doc, 1, 0, page_color_cell.as_ptr(), &mut ps_v_buf),
                VexioStatus::Ok
            );
            let _ = take_utf8(ps_v_buf);

            // Headless viewport + resize + page + zoom + all 16 tools (0..=15) pointer dispatch!
            let mut vp: *mut VexioViewport = ptr::null_mut();
            assert_eq!(
                vexio_viewport_create_headless(eng, doc, 1, 1024, 768, 1.0, &mut vp),
                VexioStatus::Ok
            );
            assert!(!vp.is_null());
            assert_eq!(vexio_viewport_resize(vp, 1280, 800, 1.0), VexioStatus::Ok);
            assert_eq!(vexio_viewport_set_page(vp, doc, 1), VexioStatus::Ok);
            assert_eq!(vexio_viewport_set_zoom(vp, doc, 1.0, 5.8, 4.1), VexioStatus::Ok);

            for tool_code in 0u32..=15u32 {
                assert_eq!(vexio_viewport_set_tool(vp, tool_code), VexioStatus::Ok);
                assert_eq!(vexio_viewport_get_active_tool(vp), tool_code);
                let base_x = 180.0 + (tool_code as f64) * 42.0;
                let base_y = 220.0 + ((tool_code % 4) as f64) * 55.0;
                let mut cur = 0u32;
                let down = VexioPointerEvent {
                    kind: 1,
                    buttons: 1,
                    modifiers: 0,
                    x_px: base_x,
                    y_px: base_y,
                    pressure: 0.75,
                    tilt_x_deg: 0.0,
                    tilt_y_deg: 0.0,
                    timestamp_us: 1000 + (tool_code as u64) * 10,
                };
                let mv = VexioPointerEvent {
                    kind: 0,
                    buttons: 1,
                    modifiers: 0,
                    x_px: base_x + 36.0,
                    y_px: base_y + 28.0,
                    pressure: 0.75,
                    tilt_x_deg: 0.0,
                    tilt_y_deg: 0.0,
                    timestamp_us: 1005 + (tool_code as u64) * 10,
                };
                let up = VexioPointerEvent {
                    kind: 2,
                    buttons: 0,
                    modifiers: 0,
                    x_px: base_x + 36.0,
                    y_px: base_y + 28.0,
                    pressure: 0.0,
                    tilt_x_deg: 0.0,
                    tilt_y_deg: 0.0,
                    timestamp_us: 1009 + (tool_code as u64) * 10,
                };
                assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &down, &mut cur), VexioStatus::Ok);
                assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &mv, &mut cur), VexioStatus::Ok);
                assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &up, &mut cur), VexioStatus::Ok);
                let _ = vexio_viewport_pop_created_text_shape(vp);
            }

            // Wheel zoom via kind=4
            let mut wheel_cur = 0u32;
            let wheel_ev = VexioPointerEvent {
                kind: 4,
                buttons: 0,
                modifiers: 2, // Ctrl
                x_px: 640.0,
                y_px: 400.0,
                pressure: 0.0,
                tilt_x_deg: 0.0,
                tilt_y_deg: 120.0,
                timestamp_us: 5000,
            };
            assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &wheel_ev, &mut wheel_cur), VexioStatus::Ok);
            assert_eq!(vexio_viewport_render_frame(vp, doc), VexioStatus::Ok);

            let mut vp_rgba = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_viewport_render_rgba(vp, doc, &mut vp_rgba), VexioStatus::Ok);
            let vp_pixels = take_bytes(vp_rgba);
            assert_eq!(vp_pixels.len(), 1280 * 800 * 4);

            let mut prev_rgba = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_render_preview_rgba(doc, 1, 320, 200, &mut prev_rgba), VexioStatus::Ok);
            assert_eq!(take_bytes(prev_rgba).len(), 320 * 200 * 4);

            // Drop stencils, draw primitive, connect, transform, duplicate, reorder, text, boolean, visual preset, theme
            let cat_c = CString::new("BPMN 2.0").unwrap();
            let m1_c = CString::new("Start Event").unwrap();
            let m2_c = CString::new("Task").unwrap();
            let (mut s1, mut s2, mut s3, mut conn_id) = (0u64, 0u64, 0u64, 0u64);
            assert_eq!(vexio_doc_drop_stencil(doc, 1, cat_c.as_ptr(), m1_c.as_ptr(), 2.0, 5.0, &mut s1), VexioStatus::Ok);
            assert_eq!(vexio_doc_drop_stencil(doc, 1, cat_c.as_ptr(), m2_c.as_ptr(), 5.5, 5.0, &mut s2), VexioStatus::Ok);
            let rect_c = CString::new("rect").unwrap();
            assert_eq!(vexio_doc_draw_primitive(doc, 1, rect_c.as_ptr(), 5.0, 4.5, 7.0, 5.8, &mut s3), VexioStatus::Ok);
            assert_eq!(vexio_doc_connect_shapes(doc, 1, s1, s2, &mut conn_id), VexioStatus::Ok);
            assert!(conn_id > 0);

            let label_c = CString::new("Approve Architecture\nGröße & Έλεγχος").unwrap();
            assert_eq!(vexio_doc_set_shape_text(doc, 1, s2, label_c.as_ptr()), VexioStatus::Ok);
            assert_eq!(vexio_doc_transform_shape(doc, 1, s2, 5.5, 5.0, 2.2, 1.1, 15.0), VexioStatus::Ok);

            let preset_c = CString::new("shadow_soft").unwrap();
            assert_eq!(vexio_doc_apply_visual_preset(doc, 1, s2, preset_c.as_ptr()), VexioStatus::Ok);
            let theme_c = CString::new("modern").unwrap();
            assert_eq!(vexio_doc_apply_theme(doc, theme_c.as_ptr(), 0), VexioStatus::Ok);

            let mut dup_id = 0u64;
            let dup_src = [s2];
            assert_eq!(vexio_doc_duplicate_shapes(doc, 1, dup_src.as_ptr(), 1, 0.5, -0.5, &mut dup_id), VexioStatus::Ok);
            assert!(dup_id > 0);
            let front_c = CString::new("front").unwrap();
            assert_eq!(vexio_doc_reorder_shapes(doc, 1, dup_src.as_ptr(), 1, front_c.as_ptr()), VexioStatus::Ok);

            let bool_pair = [s2, s3];
            let union_c = CString::new("union").unwrap();
            assert_eq!(vexio_doc_boolean_op(doc, 1, bool_pair.as_ptr(), 2, union_c.as_ptr(), 0.0), VexioStatus::Ok);

            let mut del_cnt = 0u32;
            let del_ids = [dup_id];
            assert_eq!(vexio_doc_delete_shapes(doc, 1, del_ids.as_ptr(), 1, &mut del_cnt), VexioStatus::Ok);
            assert_eq!(del_cnt, 1);

            // Undo & Redo
            assert_eq!(vexio_doc_undo(doc), VexioStatus::Ok);
            assert_eq!(vexio_doc_redo(doc), VexioStatus::Ok);

            // Auto-layout & Validate JSON
            let lr_c = CString::new("hierarchical_lr").unwrap();
            assert_eq!(vexio_doc_auto_layout(doc, 1, lr_c.as_ptr()), VexioStatus::Ok);
            let bpmn_c = CString::new("bpmn").unwrap();
            let mut val_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_doc_validate_json(doc, bpmn_c.as_ptr(), &mut val_buf), VexioStatus::Ok);
            assert!(take_utf8(val_buf).starts_with('['));

            // Subprocess + Add/Rename Page + State JSON
            let sub_c = CString::new("Sub-Audit").unwrap();
            let mut sub_pid = 0u32;
            assert_eq!(vexio_doc_create_subprocess(doc, 1, s1, sub_c.as_ptr(), &mut sub_pid), VexioStatus::Ok);
            assert!(sub_pid > 1);
            let mut added_pid = 0u32;
            let p3_c = CString::new("Page-3").unwrap();
            assert_eq!(vexio_doc_add_page(doc, p3_c.as_ptr(), &mut added_pid), VexioStatus::Ok);
            let p3_ren = CString::new("Network Core").unwrap();
            assert_eq!(vexio_doc_rename_page(doc, added_pid, p3_ren.as_ptr()), VexioStatus::Ok);

            let mut state_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_doc_get_state_json(doc, 1, &mut state_buf), VexioStatus::Ok);
            let state_str = take_utf8(state_buf);
            assert!(state_str.contains("Network Core"));

            // Save .vsdx, reopen via vexio_doc_open_bytes, and import as stencil via vexio_doc_import_stencil_bytes!
            let mut saved_vsdx = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_doc_save_format(doc, vsdx_ext.as_ptr(), &mut saved_vsdx), VexioStatus::Ok);
            let vsdx_bytes = take_bytes(saved_vsdx);
            assert!(vsdx_bytes.starts_with(b"PK\x03\x04"));

            let mut reopened: *mut VexioDoc = ptr::null_mut();
            assert_eq!(
                vexio_doc_open_bytes(
                    eng,
                    VexioByteSlice { ptr: vsdx_bytes.as_ptr(), len: vsdx_bytes.len() },
                    vsdx_ext.as_ptr(),
                    &mut reopened
                ),
                VexioStatus::Ok
            );
            assert!(!reopened.is_null());

            let imp_name = CString::new("CustomPack.vsdx").unwrap();
            let mut imp_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                vexio_doc_import_stencil_bytes(
                    reopened,
                    imp_name.as_ptr(),
                    VexioByteSlice { ptr: vsdx_bytes.as_ptr(), len: vsdx_bytes.len() },
                    &mut imp_buf
                ),
                VexioStatus::Ok
            );
            let imp_json = take_utf8(imp_buf);
            assert!(imp_json.contains("\"masters\"") && imp_json.contains("\"masterNames\"") && imp_json.contains("\"previews\""));

            // CRDT flush & apply + Diagram Compare
            let rep_c = CString::new("smoke-peer").unwrap();
            let mut crdt_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                vexio_doc_flush_crdt_json(base_doc, doc, rep_c.as_ptr(), 1_760_000_000_000, &mut crdt_buf),
                VexioStatus::Ok
            );
            let crdt_ops = take_utf8(crdt_buf);
            let crdt_ops_c = CString::new(crdt_ops).unwrap();
            assert_eq!(vexio_doc_apply_crdt_json(reopened, rep_c.as_ptr(), crdt_ops_c.as_ptr()), VexioStatus::Ok);

            let mut cmp_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_doc_compare(base_doc, doc, &mut cmp_buf), VexioStatus::Ok);
            assert!(take_utf8(cmp_buf).contains("\"added_count\""));

            // Data Visualizer CSV -> XLSX, Cloud Discovery, Entra/LDIF Org Chart
            let dv_csv = b"Process Step ID,Process Step Description,Next Step ID,Shape Type,Function\nS1,Start,S2,Start,Ops\nS2,Finish,,End,Ops\n";
            let mut dv_cnt = 0u32;
            assert_eq!(
                vexio_doc_import_data_visualizer(
                    doc,
                    added_pid,
                    VexioByteSlice { ptr: dv_csv.as_ptr(), len: dv_csv.len() },
                    &mut dv_cnt
                ),
                VexioStatus::Ok
            );
            assert!(dv_cnt >= 2);
            let mut dv_xlsx = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_doc_export_data_visualizer_xlsx(doc, added_pid, &mut dv_xlsx), VexioStatus::Ok);
            assert!(take_bytes(dv_xlsx).starts_with(b"PK\x03\x04"));

            let cloud_c = CString::new(r#"{"resources":[{"id":"r1","name":"API","kind":"compute","vpc":"v1"}]}"#).unwrap();
            let mut cloud_cnt = 0u32;
            assert_eq!(vexio_doc_import_cloud_discovery(doc, added_pid, cloud_c.as_ptr(), &mut cloud_cnt), VexioStatus::Ok);
            assert!(cloud_cnt >= 1);

            let ldif_c = CString::new("dn: cn=Ada,dc=vexio\ncn: Ada\ntitle: CTO\n").unwrap();
            let mut ldif_cnt = 0u32;
            assert_eq!(vexio_doc_import_entra_or_ldif(doc, added_pid, ldif_c.as_ptr(), &mut ldif_cnt), VexioStatus::Ok);
            assert!(ldif_cnt >= 1);

            // v0.2.0 C-ABI Extensions (10 additional functions -> 72 total C-ABI functions):
            // 1) Runtime font registration (rejects invalid font slice deterministically without panicking)
            let bad_font = b"not-a-font-file";
            let fam_c = CString::new("CustomSans").unwrap();
            assert_eq!(
                vexio_register_font_bytes(
                    VexioByteSlice { ptr: bad_font.as_ptr(), len: bad_font.len() },
                    fam_c.as_ptr()
                ),
                VexioStatus::ErrIoCorruptFormat
            );

            // 2) Smart Guides toggle & readback + Multi-selection set/get JSON
            assert!(vexio_viewport_get_smart_guides(vp));
            assert_eq!(vexio_viewport_set_smart_guides(vp, false), VexioStatus::Ok);
            assert!(!vexio_viewport_get_smart_guides(vp));
            assert_eq!(vexio_viewport_set_smart_guides(vp, true), VexioStatus::Ok);
            assert!(vexio_viewport_get_smart_guides(vp));

            let (mut a1, mut a2, mut a3, mut c_ortho) = (0u64, 0u64, 0u64, 0u64);
            assert_eq!(vexio_doc_draw_primitive(doc, added_pid, rect_c.as_ptr(), 1.0, 2.0, 2.0, 3.0, &mut a1), VexioStatus::Ok);
            assert_eq!(vexio_doc_draw_primitive(doc, added_pid, rect_c.as_ptr(), 4.0, 2.4, 5.0, 3.4, &mut a2), VexioStatus::Ok);
            assert_eq!(vexio_doc_draw_primitive(doc, added_pid, rect_c.as_ptr(), 8.5, 2.8, 9.5, 3.8, &mut a3), VexioStatus::Ok);
            assert_eq!(vexio_doc_connect_shapes(doc, added_pid, a1, a2, &mut c_ortho), VexioStatus::Ok);

            let sel_trio = [a1, a2, a3];
            assert_eq!(vexio_viewport_set_selection(vp, doc, sel_trio.as_ptr(), sel_trio.len()), VexioStatus::Ok);
            let mut sel_json_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(vexio_viewport_get_selection_json(vp, &mut sel_json_buf), VexioStatus::Ok);
            let sel_json = take_utf8(sel_json_buf);
            assert!(sel_json.contains(&a1.to_string()) && sel_json.contains(&a3.to_string()));

            // 3) Align, Distribute, Drag Connector Segment, Group & Ungroup
            let middle_c = CString::new("middle").unwrap();
            let mut aligned_cnt = 0u32;
            assert_eq!(
                vexio_doc_align_shapes(doc, added_pid, sel_trio.as_ptr(), sel_trio.len(), middle_c.as_ptr(), &mut aligned_cnt),
                VexioStatus::Ok
            );
            assert!(aligned_cnt >= 1);

            let horiz_c = CString::new("horizontal").unwrap();
            let mut dist_cnt = 0u32;
            assert_eq!(
                vexio_doc_distribute_shapes(doc, added_pid, sel_trio.as_ptr(), sel_trio.len(), horiz_c.as_ptr(), &mut dist_cnt),
                VexioStatus::Ok
            );
            assert!(dist_cnt >= 1);

            assert_eq!(vexio_doc_drag_connector_segment(doc, added_pid, c_ortho, 1, 3.25), VexioStatus::Ok);

            let grp_pair = [a1, a2];
            let mut grp_id = 0u64;
            assert_eq!(
                vexio_doc_group_shapes(doc, added_pid, grp_pair.as_ptr(), grp_pair.len(), &mut grp_id),
                VexioStatus::Ok
            );
            assert!(grp_id > 0);

            let grp_arr = [grp_id];
            let mut ungrp_buf = VexioOwnedBuffer { ptr: ptr::null_mut(), len: 0, cap: 0 };
            assert_eq!(
                vexio_doc_ungroup_shapes(doc, added_pid, grp_arr.as_ptr(), grp_arr.len(), &mut ungrp_buf),
                VexioStatus::Ok
            );
            let ungrp_json = take_utf8(ungrp_buf);
            assert!(ungrp_json.contains(','), "ungroup must return JSON array of restored member IDs: {ungrp_json}");

            // 4) v0.2.1-alpha High-DPI (`dpi_scale = 2.5`) `22 dp` `↻` Rotate Button Hit-Test & Drag Rotation!
            assert_eq!(vexio_viewport_resize(vp, 2560, 1600, 2.5), VexioStatus::Ok);
            assert_eq!(vexio_viewport_set_page(vp, doc, added_pid), VexioStatus::Ok);
            assert_eq!(vexio_viewport_set_zoom(vp, doc, 1.0, 5.0, 3.0), VexioStatus::Ok);
            assert_eq!(vexio_viewport_set_tool(vp, 0), VexioStatus::Ok);

            let mut rot_sid = 0u64;
            assert_eq!(
                vexio_doc_draw_primitive(doc, added_pid, rect_c.as_ptr(), 4.0, 2.5, 6.0, 3.5, &mut rot_sid),
                VexioStatus::Ok
            );
            let single_sel = [rot_sid];
            assert_eq!(
                vexio_viewport_set_selection(vp, doc, single_sel.as_ptr(), single_sel.len()),
                VexioStatus::Ok
            );

            let logical_ppi = (*vp).camera.zoom_ppi.max(1.0);
            let [rot_btn_x_px, rot_btn_y_px] =
                (*vp).camera.page_in_to_screen(5.0, 3.5 + 28.0 / logical_ppi);
            let [drag_to_x_px, drag_to_y_px] = (*vp).camera.page_in_to_screen(6.5, 3.0);

            let mut hover_cur = 0u32;
            let hover_ev = VexioPointerEvent {
                kind: 0,
                buttons: 0,
                modifiers: 0,
                x_px: rot_btn_x_px,
                y_px: rot_btn_y_px,
                pressure: 0.0,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 9000,
            };
            assert_eq!(
                vexio_viewport_dispatch_pointer(vp, doc, &hover_ev, &mut hover_cur),
                VexioStatus::Ok
            );
            assert_eq!(
                hover_cur, 13,
                "hovering 22dp Rotate button on 2.5x high-DPI viewport must return rotate cursor 13"
            );

            let mut rot_cur = 0u32;
            let rot_down = VexioPointerEvent {
                kind: 1,
                buttons: 0,
                modifiers: 0,
                x_px: rot_btn_x_px,
                y_px: rot_btn_y_px,
                pressure: 0.8,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 9010,
            };
            let rot_move = VexioPointerEvent {
                kind: 0,
                buttons: 0,
                modifiers: 0,
                x_px: drag_to_x_px,
                y_px: drag_to_y_px,
                pressure: 0.8,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 9020,
            };
            let rot_up = VexioPointerEvent {
                kind: 2,
                buttons: 0,
                modifiers: 0,
                x_px: drag_to_x_px,
                y_px: drag_to_y_px,
                pressure: 0.0,
                tilt_x_deg: 0.0,
                tilt_y_deg: 0.0,
                timestamp_us: 9030,
            };
            assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &rot_down, &mut rot_cur), VexioStatus::Ok);
            assert_eq!(rot_cur, 13);
            assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &rot_move, &mut rot_cur), VexioStatus::Ok);
            assert_eq!(rot_cur, 13);
            assert_eq!(vexio_viewport_dispatch_pointer(vp, doc, &rot_up, &mut rot_cur), VexioStatus::Ok);

            let angle_rad = (*doc)
                .session
                .doc
                .page(added_pid)
                .unwrap()
                .find_shape(rot_sid)
                .unwrap()
                .sheet
                .get_f64("Angle")
                .unwrap();
            assert!(
                (angle_rad.to_degrees().abs() - 90.0).abs() < 1.0,
                "dragging 22dp Rotate button 90 degrees CW must rotate Angle to ~-90 deg, got {} deg",
                angle_rad.to_degrees()
            );

            // 5) v0.3.0-alpha Pillar #3 C-ABI Extensions (5 additional functions -> all 77 C-ABI functions in vexio.h!):
            let cont_title_c = CString::new("Smoke Container").unwrap();
            let mut smoke_cont_id = 0u64;
            assert_eq!(
                vexio_doc_insert_container(
                    doc,
                    added_pid,
                    cont_title_c.as_ptr(),
                    single_sel.as_ptr(),
                    single_sel.len(),
                    &mut smoke_cont_id
                ),
                VexioStatus::Ok
            );
            assert!(smoke_cont_id > 0);

            let empty_lanes_c = CString::new("").unwrap();
            let mut smoke_lanes_cnt = 0u32;
            assert_eq!(
                vexio_doc_insert_swimlanes(doc, added_pid, empty_lanes_c.as_ptr(), &mut smoke_lanes_cnt),
                VexioStatus::Ok
            );
            // `added_pid` already received a swimlane from `vexio_doc_import_data_visualizer` above,
            // so `insert_swimlanes("")` appends 1 contiguous swimlane band (`Lane 2`)!
            assert_eq!(smoke_lanes_cnt, 1);

            let callout_txt_c = CString::new("Smoke Callout").unwrap();
            let mut smoke_callout_id = 0u64;
            assert_eq!(
                vexio_doc_attach_callout(doc, added_pid, rot_sid, callout_txt_c.as_ptr(), &mut smoke_callout_id),
                VexioStatus::Ok
            );
            assert!(smoke_callout_id > 0);

            let row1_c = CString::new("Row_1").unwrap();
            assert_eq!(
                vexio_doc_drag_control_handle(doc, added_pid, smoke_callout_id, row1_c.as_ptr(), 4.5, 3.0),
                VexioStatus::Ok
            );

            let east_dir_c = CString::new("east").unwrap();
            let (mut ac_target_id, mut ac_conn_id) = (0u64, 0u64);
            assert_eq!(
                vexio_doc_auto_connect(doc, added_pid, rot_sid, east_dir_c.as_ptr(), &mut ac_target_id, &mut ac_conn_id),
                VexioStatus::Ok
            );
            assert!(ac_target_id > 0 && ac_conn_id > 0);

            // 6) v0.3.8-alpha Object Inspector C-ABI verification (Rename, Shape Data Prop.*, Layers & VSDX round-trip):
            let new_obj_name = CString::new("ExchangeEdge.01").unwrap();
            assert_eq!(
                vexio_doc_rename_shape(doc, added_pid, rot_sid, new_obj_name.as_ptr()),
                VexioStatus::Ok
            );
            let prop_row = CString::new("Environment").unwrap();
            let prop_lbl = CString::new("Deployment Tier").unwrap();
            let prop_val = CString::new("Production").unwrap();
            assert_eq!(
                vexio_doc_upsert_shape_data(
                    doc,
                    added_pid,
                    rot_sid,
                    prop_row.as_ptr(),
                    prop_lbl.as_ptr(),
                    0,
                    prop_val.as_ptr()
                ),
                VexioStatus::Ok
            );
            let prop_cost = CString::new("MonthlyCost").unwrap();
            let prop_cost_lbl = CString::new("Monthly Cost").unwrap();
            let prop_cost_val = CString::new("420.5").unwrap();
            assert_eq!(
                vexio_doc_upsert_shape_data(
                    doc,
                    added_pid,
                    rot_sid,
                    prop_cost.as_ptr(),
                    prop_cost_lbl.as_ptr(),
                    7,
                    prop_cost_val.as_ptr()
                ),
                VexioStatus::Ok
            );
            assert_eq!(
                vexio_doc_delete_shape_data(doc, added_pid, rot_sid, prop_cost.as_ptr()),
                VexioStatus::Ok
            );

            let sec_layer_c = CString::new("Security Perimeter").unwrap();
            let mut sec_layer_ix = 999u32;
            assert_eq!(
                vexio_doc_add_layer(doc, added_pid, sec_layer_c.as_ptr(), &mut sec_layer_ix),
                VexioStatus::Ok
            );
            assert!(sec_layer_ix < 100);
            assert_eq!(
                vexio_doc_set_layer_flags(doc, added_pid, sec_layer_ix, true, false, true, true),
                VexioStatus::Ok
            );
            assert_eq!(
                vexio_doc_set_shape_layer_membership(doc, added_pid, rot_sid, sec_layer_ix, true),
                VexioStatus::Ok
            );

            let mut insp_json_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_get_state_json(doc, added_pid, &mut insp_json_buf),
                VexioStatus::Ok
            );
            let insp_json_str = std::str::from_utf8(std::slice::from_raw_parts(
                insp_json_buf.ptr,
                insp_json_buf.len,
            ))
            .unwrap()
            .to_string();
            vexio_buffer_free(insp_json_buf);
            assert!(insp_json_str.contains("ExchangeEdge.01"));
            assert!(insp_json_str.contains("Deployment Tier"));
            assert!(insp_json_str.contains("Production"));
            assert!(!insp_json_str.contains("MonthlyCost"));
            assert!(insp_json_str.contains("Security Perimeter"));

            // Round-trip through `.vsdx` to confirm `Prop.Environment` and `LayerMember` survive load -> save -> load!
            let vsdx_ext = CString::new("vsdx").unwrap();
            let mut vsdx_out = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_save_format(doc, vsdx_ext.as_ptr(), &mut vsdx_out),
                VexioStatus::Ok
            );
            let mut rt_doc: *mut VexioDoc = std::ptr::null_mut();
            assert_eq!(
                vexio_doc_open_bytes(
                    eng,
                    VexioByteSlice {
                        ptr: vsdx_out.ptr,
                        len: vsdx_out.len,
                    },
                    vsdx_ext.as_ptr(),
                    &mut rt_doc,
                ),
                VexioStatus::Ok
            );
            vexio_buffer_free(vsdx_out);
            let rt_shape = (*rt_doc)
                .session
                .doc
                .page(added_pid)
                .unwrap()
                .find_shape(rot_sid)
                .unwrap();
            assert_eq!(rt_shape.name, "ExchangeEdge.01");
            assert!(rt_shape.layer_membership.contains(&sec_layer_ix));
            assert!(rt_shape.sheet.shape_data.contains_key("environment"));
            vexio_doc_destroy(rt_doc);

            // Pillar #4 C-ABI smoke verification across all 9 Data Linking & Data Graphics endpoints
            let csv_payload = b"Label,Status,CompletionPct,Owner\nExchangeEdge.01,Healthy,95,SecOps\n";
            let csv_fname = CString::new("edge_nodes.csv").unwrap();
            let key_hint = CString::new("Label").unwrap();
            let empty_c = CString::new("").unwrap();
            let mut out_rs_id: u32 = 0;
            let mut out_linked: u32 = 0;
            assert_eq!(
                vexio_doc_import_external_recordset(
                    doc,
                    added_pid,
                    csv_fname.as_ptr(),
                    VexioByteSlice {
                        ptr: csv_payload.as_ptr(),
                        len: csv_payload.len(),
                    },
                    key_hint.as_ptr(),
                    empty_c.as_ptr(),
                    true,
                    true,
                    &mut out_rs_id,
                    &mut out_linked,
                ),
                VexioStatus::Ok
            );
            assert!(out_rs_id >= 1);

            let bind_ids = [rot_sid];
            let mut bound_cnt: u32 = 0;
            assert_eq!(
                vexio_doc_bind_external_row(
                    doc,
                    added_pid,
                    out_rs_id,
                    0,
                    bind_ids.as_ptr(),
                    bind_ids.len(),
                    &mut bound_cnt,
                ),
                VexioStatus::Ok
            );
            assert_eq!(bound_cnt, 1);

            let mut legend_sid: u64 = 0;
            let status_prop = CString::new("Status").unwrap();
            assert_eq!(
                vexio_doc_insert_data_legend(
                    doc,
                    added_pid,
                    status_prop.as_ptr(),
                    8.5,
                    2.0,
                    &mut legend_sid,
                ),
                VexioStatus::Ok
            );
            assert!(legend_sid > 0);

            let mut dls_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_get_data_link_state_json(doc, added_pid, &mut dls_buf),
                VexioStatus::Ok
            );
            let dls_str =
                std::str::from_utf8(std::slice::from_raw_parts(dls_buf.ptr, dls_buf.len))
                    .unwrap()
                    .to_string();
            vexio_buffer_free(dls_buf);
            assert!(dls_str.contains("edge_nodes"));

            let mut refreshed_cnt: u32 = 0;
            assert_eq!(
                vexio_doc_refresh_external_recordset(
                    doc,
                    added_pid,
                    out_rs_id,
                    VexioByteSlice {
                        ptr: std::ptr::null(),
                        len: 0,
                    },
                    empty_c.as_ptr(),
                    &mut refreshed_cnt,
                ),
                VexioStatus::Ok
            );
            assert_eq!(refreshed_cnt, 1);

            let xlsx_ext = CString::new("xlsx").unwrap();
            let mut wb_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_sync_and_export_recordset(
                    doc,
                    added_pid,
                    out_rs_id,
                    xlsx_ext.as_ptr(),
                    &mut wb_buf,
                ),
                VexioStatus::Ok
            );
            assert!(wb_buf.len > 100);
            vexio_buffer_free(wb_buf);

            let mut bom_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_export_bom_report(doc, xlsx_ext.as_ptr(), &mut bom_buf),
                VexioStatus::Ok
            );
            assert!(bom_buf.len > 100);
            vexio_buffer_free(bom_buf);

            assert_eq!(
                vexio_doc_delete_external_recordset(doc, out_rs_id),
                VexioStatus::Ok
            );

            // v0.4.0-alpha: Duplicate Page, Background Page Assignment & Delete Page Guard
            let mut dup_pid: u32 = 0;
            assert_eq!(
                vexio_doc_duplicate_page(doc, added_pid, &mut dup_pid),
                VexioStatus::Ok
            );
            assert!(dup_pid > added_pid);

            let bg_hex = CString::new("#EFF6FF").unwrap();
            assert_eq!(
                vexio_doc_set_page_background(doc, dup_pid, true, 0, bg_hex.as_ptr()),
                VexioStatus::Ok
            );

            let fg_hex = CString::new("#FAF8F5").unwrap();
            assert_eq!(
                vexio_doc_set_page_background(doc, added_pid, false, dup_pid, fg_hex.as_ptr()),
                VexioStatus::Ok
            );

            let mut bg_state_buf = VexioOwnedBuffer {
                ptr: std::ptr::null_mut(),
                len: 0,
                cap: 0,
            };
            assert_eq!(
                vexio_doc_get_state_json(doc, added_pid, &mut bg_state_buf),
                VexioStatus::Ok
            );
            let bg_state_str = std::str::from_utf8(std::slice::from_raw_parts(
                bg_state_buf.ptr,
                bg_state_buf.len,
            ))
            .unwrap()
            .to_string();
            vexio_buffer_free(bg_state_buf);
            assert!(bg_state_str.contains("#FAF8F5"));
            assert!(bg_state_str.contains(&format!("\"backPageId\":{}", dup_pid)));

            assert_eq!(vexio_doc_delete_page(doc, dup_pid), VexioStatus::Ok);

            // v0.4.2-alpha: Visio Text Headings, Titles & Borders/Titles Gallery
            let h1_slug = CString::new("h1").unwrap();
            let h1_text = CString::new("System Architecture Overview").unwrap();
            let mut inserted_h1_id: u64 = 0;
            assert_eq!(
                vexio_doc_insert_heading(
                    doc,
                    added_pid,
                    h1_slug.as_ptr(),
                    h1_text.as_ptr(),
                    0.0,
                    0.0,
                    &mut inserted_h1_id
                ),
                VexioStatus::Ok
            );
            assert!(inserted_h1_id > 0);

            let banner_slug = CString::new("section_banner").unwrap();
            let mut modified_headings: u32 = 0;
            assert_eq!(
                vexio_doc_apply_heading_preset(
                    doc,
                    added_pid,
                    &inserted_h1_id,
                    1,
                    banner_slug.as_ptr(),
                    &mut modified_headings
                ),
                VexioStatus::Ok
            );
            assert_eq!(modified_headings, 1);

            let bt_style = CString::new("modern_banner").unwrap();
            let bt_title = CString::new("Global Cloud Network").unwrap();
            let bt_sub = CString::new("Phase 2 Topology").unwrap();
            let bt_author = CString::new("Architecture Team").unwrap();
            let bt_date = CString::new("2026-11-25").unwrap();
            let mut bt_bg_id: u32 = 0;
            assert_eq!(
                vexio_doc_apply_border_and_title(
                    doc,
                    added_pid,
                    bt_style.as_ptr(),
                    bt_title.as_ptr(),
                    bt_sub.as_ptr(),
                    bt_author.as_ptr(),
                    bt_date.as_ptr(),
                    &mut bt_bg_id
                ),
                VexioStatus::Ok
            );
            assert!(bt_bg_id > 0);

            let bt_none = CString::new("none").unwrap();
            let mut cleared_bg_id: u32 = 99;
            assert_eq!(
                vexio_doc_apply_border_and_title(
                    doc,
                    added_pid,
                    bt_none.as_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    &mut cleared_bg_id
                ),
                VexioStatus::Ok
            );
            assert_eq!(cleared_bg_id, 0);

            let remaining_fg: Vec<u32> = (*doc)
                .session
                .doc
                .pages
                .iter()
                .filter(|p| !p.is_background)
                .map(|p| p.id)
                .collect();
            for &pid in &remaining_fg[1..] {
                assert_eq!(vexio_doc_delete_page(doc, pid), VexioStatus::Ok);
            }
            // Deleting the last remaining foreground page must be rejected cleanly
            assert_eq!(
                vexio_doc_delete_page(doc, remaining_fg[0]),
                VexioStatus::ErrInvalidArg
            );

            let _ = vexio_viewport_get_primary_selection(vp);
            vexio_viewport_destroy(vp);
            vexio_doc_destroy(reopened);
            vexio_doc_destroy(base_doc);
            vexio_doc_destroy(doc);
            vexio_engine_destroy(eng);
        }
    }
}





