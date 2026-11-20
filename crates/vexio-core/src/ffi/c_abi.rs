//! C-ABI FFI Implementation (`crates/vexio-core/include/vexio.h`) for Windows WinUI 3,
//! macOS Swift / Quick Look, and native C++ consumers.

use crate::journal::JournalError;
use crate::workspace::{CoreError, DocumentSession};
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::catch_unwind;
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
