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
            _ => Self::Select,
        }
    }
}

#[derive(Debug, Clone)]
pub enum NativeDragState {
    Pan {
        last_x_px: f64,
        last_y_px: f64,
    },
    MoveShape {
        shape_id: u64,
        start_page: [f64; 2],
        orig_pin: [f64; 2],
        moved: bool,
    },
    DrawBoxOrLine {
        tool: NativeCanvasTool,
        start_page: [f64; 2],
        from_shape_id: Option<u64>,
    },
    DrawPath {
        tool: NativeCanvasTool,
        points: Vec<MarkupInkPoint>,
    },
}

pub struct VexioEngine {
    pub initialized: bool,
}

pub struct VexioDoc {
    pub session: DocumentSession,
}

pub struct VexioViewport {
    pub page_id: u32,
    pub camera: vexio_render::ViewportCamera,
    pub gpu_renderer: Option<vexio_render::WgpuCanvasRenderer>,
    pub active_tool: NativeCanvasTool,
    pub selected_ids: Vec<u64>,
    pub drag_state: Option<NativeDragState>,
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
        if let Some(page) = doc.page(page_id).or_else(|| doc.pages.first()) {
            camera.fit_to_page(page.width_in(), page.height_in());
        }
        Self {
            page_id,
            camera,
            gpu_renderer: None,
            active_tool: NativeCanvasTool::Select,
            selected_ids: Vec::new(),
            drag_state: None,
            android_native_window: ptr::null_mut(),
        }
    }

    pub fn present_if_attached(&mut self, doc: &VisioDocument) -> bool {
        let Some(r) = &mut self.gpu_renderer else {
            return false;
        };
        r.camera = self.camera;
        r.selected_shapes = self.selected_ids.clone();
        r.render_surface_frame(doc, self.page_id).is_ok()
    }

    /// Stateful cross-platform pointer / stylus / wheel dispatcher shared by C-ABI and JNI.
    /// Returns cursor code (`0=Arrow`, `1=Move`, `2=Crosshair`, `3=HandGrab`, `7=TextIBeam`).
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
        let visuals = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
        let index = vexio_render::SpatialIndex::build(&visuals);
        let top_hit = index
            .hit_test_visuals(&visuals, page_pt[0], page_pt[1], 0.09)
            .into_iter()
            .next();

        let is_middle_btn = (ev.buttons & 4) != 0;
        let is_shift = (ev.modifiers & 1) != 0;

        match ev.kind {
            1 => {
                // Pointer Down
                if is_middle_btn || self.active_tool == NativeCanvasTool::Pan {
                    self.drag_state = Some(NativeDragState::Pan {
                        last_x_px: ev.x_px,
                        last_y_px: ev.y_px,
                    });
                    return 3;
                }
                match self.active_tool {
                    NativeCanvasTool::Select => {
                        if let Some(sid) = top_hit {
                            if is_shift {
                                if let Some(pos) = self.selected_ids.iter().position(|&x| x == sid) {
                                    self.selected_ids.remove(pos);
                                } else {
                                    self.selected_ids.push(sid);
                                }
                            } else if !self.selected_ids.contains(&sid) {
                                self.selected_ids = vec![sid];
                            }
                            let orig_pin = session
                                .doc
                                .page(pid)
                                .and_then(|p| p.find_shape(sid))
                                .map(|s| {
                                    [
                                        s.sheet.get_f64("PinX").unwrap_or(page_pt[0]),
                                        s.sheet.get_f64("PinY").unwrap_or(page_pt[1]),
                                    ]
                                })
                                .unwrap_or(page_pt);
                            self.drag_state = Some(NativeDragState::MoveShape {
                                shape_id: sid,
                                start_page: page_pt,
                                orig_pin,
                                moved: false,
                            });
                            self.present_if_attached(&session.doc);
                            return 1;
                        } else {
                            self.selected_ids.clear();
                            self.drag_state = Some(NativeDragState::Pan {
                                last_x_px: ev.x_px,
                                last_y_px: ev.y_px,
                            });
                            self.present_if_attached(&session.doc);
                            return 0;
                        }
                    }
                    NativeCanvasTool::Freeform
                    | NativeCanvasTool::InkPen
                    | NativeCanvasTool::Highlighter => {
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
                    tool => {
                        self.drag_state = Some(NativeDragState::DrawBoxOrLine {
                            tool,
                            start_page: page_pt,
                            from_shape_id: top_hit,
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
                        NativeDragState::MoveShape {
                            shape_id,
                            start_page,
                            orig_pin,
                            moved,
                        } => {
                            let dx = page_pt[0] - start_page[0];
                            let dy = page_pt[1] - start_page[1];
                            if dx.hypot(dy) > 0.02 || *moved {
                                *moved = true;
                                let nx = orig_pin[0] + dx;
                                let ny = orig_pin[1] + dy;
                                if let Some(page) = session.doc.page_mut(pid) {
                                    if let Some(sh) = page.find_shape_mut(*shape_id) {
                                        if sh.kind == ShapeKind::Connector1D {
                                            let cur_px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
                                            let cur_py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
                                            let sdx = nx - cur_px;
                                            let sdy = ny - cur_py;
                                            let bx = sh.sheet.get_f64("BeginX").unwrap_or(0.0) + sdx;
                                            let by = sh.sheet.get_f64("BeginY").unwrap_or(0.0) + sdy;
                                            let ex = sh.sheet.get_f64("EndX").unwrap_or(1.0) + sdx;
                                            let ey = sh.sheet.get_f64("EndY").unwrap_or(0.0) + sdy;
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
                                        } else {
                                            let _ = sh.sheet.try_set_formula_respecting_guard(
                                                "PinX",
                                                format!("{} in", format_clean_f64(nx)),
                                                false,
                                            );
                                            let _ = sh.sheet.try_set_formula_respecting_guard(
                                                "PinY",
                                                format!("{} in", format_clean_f64(ny)),
                                                false,
                                            );
                                        }
                                    }
                                }
                                route_page_connectors(&mut session.doc, pid);
                                self.present_if_attached(&session.doc);
                            }
                            return 1;
                        }
                        NativeDragState::DrawPath { points, .. } => {
                            let should_push = points
                                .last()
                                .map(|p| {
                                    (p.x_in - page_pt[0]).hypot(p.y_in - page_pt[1]) >= 0.02
                                })
                                .unwrap_or(true);
                            if should_push {
                                points.push(MarkupInkPoint {
                                    x_in: page_pt[0],
                                    y_in: page_pt[1],
                                    pressure: ev.pressure.clamp(0.1, 1.0),
                                    tilt_x: ev.tilt_x_deg,
                                    tilt_y: ev.tilt_y_deg,
                                });
                            }
                            return 2;
                        }
                        NativeDragState::DrawBoxOrLine { tool, .. } => {
                            return if *tool == NativeCanvasTool::Text { 7 } else { 2 };
                        }
                    }
                }
            }
            2 => {
                // Pointer Up
                if let Some(state) = self.drag_state.take() {
                    match state {
                        NativeDragState::Pan { .. } => {}
                        NativeDragState::MoveShape {
                            shape_id,
                            orig_pin,
                            moved,
                            ..
                        } => {
                            if moved {
                                // Record clean undoable move from `orig_pin` to final `PinX/PinY`
                                if let Some(sh) =
                                    session.doc.page(pid).and_then(|p| p.find_shape(shape_id))
                                {
                                    if sh.kind == ShapeKind::Shape2D {
                                        let fx = sh.sheet.get_f64("PinX").unwrap_or(orig_pin[0]);
                                        let fy = sh.sheet.get_f64("PinY").unwrap_or(orig_pin[1]);
                                        let batch = DocCommand::Batch {
                                            label: "Move Shape".into(),
                                            commands: vec![
                                                DocCommand::SetCellFormula {
                                                    page_id: pid,
                                                    shape_id,
                                                    cell_name: "PinX".into(),
                                                    old_formula: format!(
                                                        "{} in",
                                                        format_clean_f64(orig_pin[0])
                                                    ),
                                                    new_formula: format!(
                                                        "{} in",
                                                        format_clean_f64(fx)
                                                    ),
                                                    force_setf: false,
                                                },
                                                DocCommand::SetCellFormula {
                                                    page_id: pid,
                                                    shape_id,
                                                    cell_name: "PinY".into(),
                                                    old_formula: format!(
                                                        "{} in",
                                                        format_clean_f64(orig_pin[1])
                                                    ),
                                                    new_formula: format!(
                                                        "{} in",
                                                        format_clean_f64(fy)
                                                    ),
                                                    force_setf: false,
                                                },
                                            ],
                                        };
                                        let _ = session.journal.execute(&mut session.doc, batch);
                                        route_page_connectors(&mut session.doc, pid);
                                    }
                                }
                            }
                        }
                        NativeDragState::DrawBoxOrLine {
                            tool,
                            start_page,
                            from_shape_id,
                        } => {
                            let created = match tool {
                                NativeCanvasTool::Line => session
                                    .draw_line(pid, start_page[0], start_page[1], page_pt[0], page_pt[1])
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
                                NativeCanvasTool::Connector => session
                                    .connect_points_or_shapes(
                                        pid,
                                        start_page[0],
                                        start_page[1],
                                        page_pt[0],
                                        page_pt[1],
                                        from_shape_id,
                                        top_hit,
                                    )
                                    .ok(),
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
                                            created_id = draw_freeform_curve(doc, pid, &raw_pts, false);
                                        },
                                    );
                                    if let Some(sid) = created_id {
                                        self.selected_ids = vec![sid];
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
    text: String,
    pin_x: f64,
    pin_y: f64,
    width: f64,
    height: f64,
    angle_deg: f64,
    control_handles_count: usize,
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeLayerDto {
    ix: u32,
    name: String,
    visible: bool,
    print: bool,
    lock: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeStateDto {
    title: String,
    active_page_id: u32,
    page_width_in: f64,
    page_height_in: f64,
    pages: Vec<NativePageDto>,
    layers: Vec<NativeLayerDto>,
    shapes: Vec<NativeShapeDto>,
    svg_preview: String,
    can_undo: bool,
    can_redo: bool,
}

pub(crate) fn build_native_state_json(session: &DocumentSession, active_page_id: u32) -> String {
    let pid = if session.doc.page(active_page_id).is_some() {
        active_page_id
    } else {
        session.doc.pages.first().map(|p| p.id).unwrap_or(1)
    };
    let active_page = session.doc.page(pid);
    let page_width_in = active_page.map(|p| p.width_in()).unwrap_or(11.0);
    let page_height_in = active_page.map(|p| p.height_in()).unwrap_or(8.5);

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
                    NativeShapeDto {
                        id: s.id,
                        name: s.name.clone(),
                        kind: format!("{:?}", s.kind),
                        text: s.text.clone(),
                        pin_x: s.sheet.get_f64("PinX").unwrap_or(0.0),
                        pin_y: s.sheet.get_f64("PinY").unwrap_or(0.0),
                        width: s.sheet.get_f64("Width").unwrap_or(1.0),
                        height: s.sheet.get_f64("Height").unwrap_or(1.0),
                        angle_deg: s.sheet.get_f64("Angle").unwrap_or(0.0).to_degrees(),
                        control_handles_count: list_control_handles(&session.doc, pid, s.id).len(),
                        cells,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let dto = NativeStateDto {
        title: session.doc.properties.title.clone(),
        active_page_id: pid,
        page_width_in,
        page_height_in,
        pages,
        layers,
        shapes,
        svg_preview: export_page_svg(&session.doc, pid),
        can_undo: session.journal.can_undo(),
        can_redo: session.journal.can_redo(),
    };
    serde_json::to_string(&dto).unwrap_or_else(|_| "{}".into())
}

pub(crate) fn build_stencils_catalog_json() -> String {
    let previews = list_builtin_stencil_previews();
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
    let res = catch_unwind(|| {
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
                // SAFETY: `out_doc` verified non-null above.
                unsafe {
                    *out_doc = Box::into_raw(Box::new(VexioDoc { session }));
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrIoCorruptFormat,
        }
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_save_format(
    doc: *mut VexioDoc,
    ext_utf8: *const c_char,
    out_buf: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
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
                // SAFETY: `out_buf` verified non-null.
                unsafe { *out_buf = VexioOwnedBuffer::from_vec(vec) };
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrIoCorruptFormat,
        }
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
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
pub unsafe extern "C" fn vexio_cell_get_formula(
    doc: *const VexioDoc,
    page_id: u32,
    shape_id: u64,
    cell_name: *const c_char,
    out_utf8: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if doc.is_null() || cell_name.is_null() || out_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let doc_ref = unsafe { &*doc };
        let Ok(cname) = unsafe { CStr::from_ptr(cell_name) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Some(f) = doc_ref
            .session
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .and_then(|s| s.sheet.get_cell(cname))
            .map(|c| c.formula.clone())
        else {
            return VexioStatus::ErrInvalidArg;
        };
        // SAFETY: `out_utf8` verified non-null.
        unsafe { *out_utf8 = VexioOwnedBuffer::from_vec(f.into_bytes()) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_cell_get_value_str(
    doc: *const VexioDoc,
    page_id: u32,
    shape_id: u64,
    cell_name: *const c_char,
    out_utf8: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if doc.is_null() || cell_name.is_null() || out_utf8.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: Pointers verified non-null.
        let doc_ref = unsafe { &*doc };
        let Ok(cname) = unsafe { CStr::from_ptr(cell_name) }.to_str() else {
            return VexioStatus::ErrInvalidArg;
        };
        let Some(val_str) = doc_ref
            .session
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .and_then(|s| s.sheet.get_cell(cname))
            .map(|c| {
                c.value
                    .as_str_coerced()
                    .unwrap_or_else(|e| e.as_str().to_string())
            })
        else {
            return VexioStatus::ErrInvalidArg;
        };
        // SAFETY: `out_utf8` verified non-null.
        unsafe { *out_utf8 = VexioOwnedBuffer::from_vec(val_str.into_bytes()) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
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
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() || doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_ref = unsafe { &*doc };
        vp_mut.page_id = page_id;
        vp_mut.selected_ids.clear();
        if let Some(p) = doc_ref.session.doc.page(page_id) {
            vp_mut.camera.fit_to_page(p.width_in(), p.height_in());
        }
        vp_mut.present_if_attached(&doc_ref.session.doc);
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_set_zoom(
    vp: *mut VexioViewport,
    doc: *const VexioDoc,
    zoom_scale: f64,
    pan_x_in: f64,
    pan_y_in: f64,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() || doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_ref = unsafe { &*doc };
        if zoom_scale <= 0.0 {
            if let Some(p) = doc_ref.session.doc.page(vp_mut.page_id) {
                vp_mut.camera.fit_to_page(p.width_in(), p.height_in());
            }
        } else {
            vp_mut.camera.zoom_ppi = 96.0 * zoom_scale.clamp(0.1, 16.0);
            if pan_x_in.is_finite() && pan_y_in.is_finite() && (pan_x_in != 0.0 || pan_y_in != 0.0) {
                vp_mut.camera.center_x_in = pan_x_in;
                vp_mut.camera.center_y_in = pan_y_in;
            }
        }
        vp_mut.present_if_attached(&doc_ref.session.doc);
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_render_frame(
    vp: *mut VexioViewport,
    doc: *const VexioDoc,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() || doc.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp` and `doc` verified non-null.
        let vp_mut = unsafe { &mut *vp };
        let doc_ref = unsafe { &*doc };
        vp_mut.present_if_attached(&doc_ref.session.doc);
        VexioStatus::Ok
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_render_rgba(
    vp: *const VexioViewport,
    doc: *const VexioDoc,
    out_rgba: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if vp.is_null() || doc.is_null() || out_rgba.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `vp`, `doc`, and `out_rgba` verified non-null.
        let vp_ref = unsafe { &*vp };
        let doc_ref = unsafe { &*doc };
        match vexio_render::render_page_software_rgba_with_selection(
            &doc_ref.session.doc,
            vp_ref.page_id,
            &vp_ref.camera,
            true,
            &vp_ref.selected_ids,
        ) {
            Ok(rgba) => {
                // SAFETY: `out_rgba` verified non-null.
                unsafe { *out_rgba = VexioOwnedBuffer::from_vec(rgba) };
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrGpuSurfaceLost,
        }
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
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
    let res = catch_unwind(AssertUnwindSafe(|| {
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
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_render_preview_rgba(
    doc: *const VexioDoc,
    page_id: u32,
    width_px: u32,
    height_px: u32,
    out_rgba: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
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
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_list_stencils_json(
    doc: *const VexioDoc,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        let json = build_stencils_catalog_json();
        // SAFETY: `out_json` verified non-null.
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_load_starter_template(
    doc: *mut VexioDoc,
    template_id_utf8: *const c_char,
) -> VexioStatus {
    let res = catch_unwind(AssertUnwindSafe(|| {
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
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
}

#[no_mangle]
pub unsafe extern "C" fn vexio_doc_get_state_json(
    doc: *const VexioDoc,
    active_page_id: u32,
    out_json: *mut VexioOwnedBuffer,
) -> VexioStatus {
    let res = catch_unwind(|| {
        if doc.is_null() || out_json.is_null() {
            return VexioStatus::ErrInvalidArg;
        }
        // SAFETY: `doc` and `out_json` verified non-null.
        let d = unsafe { &*doc };
        let json = build_native_state_json(&d.session, active_page_id);
        unsafe { *out_json = VexioOwnedBuffer::from_vec(json.into_bytes()) };
        VexioStatus::Ok
    });
    res.unwrap_or(VexioStatus::ErrInternalPanic)
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
    let res = catch_unwind(AssertUnwindSafe(|| {
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
    }));
    res.unwrap_or(VexioStatus::ErrInternalPanic)
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
}



