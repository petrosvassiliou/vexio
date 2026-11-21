//! C-ABI FFI Implementation (`crates/vexio-core/include/vexio.h`) for Windows WinUI 3,
//! macOS Swift / Quick Look, and native C++ consumers.

use crate::journal::JournalError;
use crate::workspace::{CoreError, DocumentSession};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

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

pub struct VexioEngine {
    pub initialized: bool,
}

pub struct VexioDoc {
    pub session: DocumentSession,
}

pub struct VexioViewport {
    pub page_id: u32,
    pub camera: vexio_render::ViewportCamera,
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
    let res = catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    if engine.is_null() || doc.is_null() || out_viewport.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    let vp = VexioViewport {
        page_id,
        camera: vexio_render::ViewportCamera::new(width_px.max(1), height_px.max(1), dpi_scale.max(0.5) as f64),
    };
    // SAFETY: `out_viewport` verified non-null.
    unsafe { *out_viewport = Box::into_raw(Box::new(vp)) };
    VexioStatus::Ok
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
    if dxgi_swapchain_panel_unk.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    // SAFETY: Delegates after non-null check on `dxgi_swapchain_panel_unk`.
    unsafe {
        vexio_viewport_create_headless(
            engine,
            doc,
            1,
            width_px,
            height_px,
            dpi_scale,
            out_viewport,
        )
    }
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
    if ca_metal_layer_ptr.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    // SAFETY: Delegates after non-null check on `ca_metal_layer_ptr`.
    unsafe {
        vexio_viewport_create_headless(
            engine,
            doc,
            1,
            width_px,
            height_px,
            dpi_scale,
            out_viewport,
        )
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_destroy(vp: *mut VexioViewport) {
    if !vp.is_null() {
        // SAFETY: Allocated by `Box::into_raw`.
        drop(unsafe { Box::from_raw(vp) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn vexio_viewport_dispatch_pointer(
    vp: *mut VexioViewport,
    doc: *mut VexioDoc,
    ev: *const VexioPointerEvent,
    out_cursor_kind: *mut u32,
) -> VexioStatus {
    if vp.is_null() || doc.is_null() || ev.is_null() {
        return VexioStatus::ErrInvalidArg;
    }
    // SAFETY: `vp`, `doc`, and `ev` verified non-null.
    let vp_ref = unsafe { &*vp };
    let doc_ref = unsafe { &*doc };
    let ev_val = unsafe { *ev };
    let page_pt = vp_ref.camera.screen_to_page_in(ev_val.x_px, ev_val.y_px);
    let visuals = vexio_shapesheet::ReactiveEngine::evaluate_page_visuals(&doc_ref.session.doc, vp_ref.page_id);
    let index = vexio_render::SpatialIndex::build(&visuals);
    let hits = index.hit_test_point(page_pt[0], page_pt[1], 0.08);
    // Cursor codes: 0=Arrow, 1=Move(SizeAll), 2=Crosshair, 3=HandGrab
    let cursor = if (ev_val.buttons & 4) != 0 {
        3
    } else if !hits.is_empty() {
        1
    } else {
        0
    };
    if !out_cursor_kind.is_null() {
        // SAFETY: `out_cursor_kind` checked non-null.
        unsafe { ptr::write(out_cursor_kind, cursor) };
    }
    VexioStatus::Ok
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
        crate::boolean_ops::apply_boolean_operation(&mut d.session.doc, page_id, ids, kind);
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
        crate::themes_styles::apply_document_theme_variant(&mut d.session.doc, slug, variant_index);
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
        let Some(pid) = crate::enterprise_plan2::create_or_link_subprocess_page(
            &mut d.session.doc,
            page_id,
            shape_id,
            name,
        ) else {
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
        if crate::themes_styles::apply_visual_effects_to_shape(
            &mut d.session.doc,
            page_id,
            shape_id,
            &spec,
        ) {
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
        match crate::plan2_connectors::import_data_visualizer_csv_or_xlsx(
            &mut d.session.doc,
            page_id,
            bytes,
        ) {
            Ok(count) => {
                if !out_step_count.is_null() {
                    // SAFETY: `out_step_count` checked non-null.
                    unsafe { *out_step_count = count as u32 };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrIoCorruptFormat,
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
        match crate::plan2_connectors::import_cloud_discovery_json(
            &mut d.session.doc,
            page_id,
            json_str,
        ) {
            Ok(count) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` checked non-null.
                    unsafe { *out_count = count as u32 };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrIoCorruptFormat,
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
        match crate::plan2_connectors::import_entra_or_ldif_org_chart(
            &mut d.session.doc,
            page_id,
            text_str,
        ) {
            Ok(count) => {
                if !out_count.is_null() {
                    // SAFETY: `out_count` checked non-null.
                    unsafe { *out_count = count as u32 };
                }
                VexioStatus::Ok
            }
            Err(_) => VexioStatus::ErrIoCorruptFormat,
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
}


