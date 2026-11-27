//! Zero-Copy Android & Googlebook OS JNI Bridge (`com.vexio.core.NativeBridge`).
//!
//! Uses direct `ByteBuffer` memory views (`get_direct_buffer_address`) for zero-copy
//! `.vsdx`/`.vsd`/`.dwg` loading, USI Active Stylus motion batches, and RGBA previews.

#![cfg(not(target_arch = "wasm32"))]

use crate::ffi::c_abi::{
    NativeCanvasTool, VexioDoc, VexioEngine, VexioPointerEvent, VexioViewport,
    build_native_state_json, build_stencils_catalog_json,
};
use crate::journal::JournalError;
use crate::workspace::{CoreError, DocumentSession};
use jni::JNIEnv;
use jni::objects::{JByteArray, JByteBuffer, JClass, JLongArray, JObject, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint, jlong, jlongArray, jstring, JNI_FALSE, JNI_TRUE};
use std::panic::{AssertUnwindSafe, catch_unwind};

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeInitEngine(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    catch_unwind(|| Box::into_raw(Box::new(VexioEngine { initialized: true })) as jlong).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDestroyEngine(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    engine_ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if engine_ptr != 0 {
            // SAFETY: `engine_ptr` was allocated via `Box::into_raw` in `nativeInitEngine`.
            drop(unsafe { Box::from_raw(engine_ptr as *mut VexioEngine) });
        }
    }));
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeCreateBlankDocument<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    title: JString<'local>,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        let t: String = env
            .get_string(&title)
            .map(|s| s.into())
            .unwrap_or_else(|_| "Untitled".into());
        Box::into_raw(Box::new(VexioDoc {
            session: DocumentSession::new_blank(&t),
        })) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDestroyDocument(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr != 0 {
            // SAFETY: `doc_ptr` was allocated via `Box::into_raw` in `nativeCreateBlankDocument` or `nativeOpenDocument`.
            drop(unsafe { Box::from_raw(doc_ptr as *mut VexioDoc) });
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeOpenDocument<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine_ptr: jlong,
    direct_bytes: JByteBuffer<'local>,
    ext: JString<'local>,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if engine_ptr == 0 {
            return 0;
        }
        let Ok(ptr) = env.get_direct_buffer_address(&direct_bytes) else {
            return 0;
        };
        let Ok(cap) = env.get_direct_buffer_capacity(&direct_bytes) else {
            return 0;
        };
        let Ok(ext_str) = env.get_string(&ext).map(String::from) else {
            return 0;
        };
        // SAFETY: `ptr` and `cap` come from a valid JVM `DirectByteBuffer` pinned for this JNI call.
        let slice = unsafe { std::slice::from_raw_parts(ptr, cap) };
        let mut session = DocumentSession::new_blank("Imported");
        if session.open_bytes(slice, &ext_str).is_ok() {
            Box::into_raw(Box::new(VexioDoc { session })) as jlong
        } else {
            0
        }
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSaveDocument<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    ext: JString<'local>,
) -> JByteArray<'local> {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JByteArray::default();
        }
        // SAFETY: `doc_ptr` created by `nativeCreateBlankDocument` or `nativeOpenDocument`.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let Ok(ext_s) = env.get_string(&ext).map(String::from) else {
            return JByteArray::default();
        };
        match doc.session.save_format(&ext_s) {
            Ok(bytes) => env.byte_array_from_slice(&bytes).unwrap_or_default(),
            Err(_) => JByteArray::default(),
        }
    }))
    .unwrap_or_default()
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAttachVulkanSurface<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine_ptr: jlong,
    doc_ptr: jlong,
    surface: JObject<'local>,
    width: jint,
    height: jint,
    density: jfloat,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if engine_ptr == 0 || doc_ptr == 0 {
            return 0;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let w = width.max(1) as u32;
        let h = height.max(1) as u32;
        let dpi = density.max(0.5);
        #[allow(unused_mut)]
        let mut vp = VexioViewport::new_base(&doc.session.doc, u32::MAX, w, h, dpi);

        #[cfg(target_os = "android")]
        {
            if !surface.is_null() {
                #[link(name = "android")]
                extern "C" {
                    fn ANativeWindow_fromSurface(
                        env: *mut jni::sys::JNIEnv,
                        surface: jni::sys::jobject,
                    ) -> *mut std::ffi::c_void;
                    fn ANativeWindow_release(window: *mut std::ffi::c_void);
                }
                // SAFETY: Valid JNIEnv raw pointer and non-null `android.view.Surface` jobject.
                let native_win =
                    unsafe { ANativeWindow_fromSurface(env.get_native_interface(), surface.as_raw()) };
                if !native_win.is_null() {
                    // SAFETY: `native_win` was acquired via `ANativeWindow_fromSurface` and is retained on `vp`
                    // (or released immediately if `wgpu` surface creation fails).
                    match unsafe {
                        vexio_render::WgpuCanvasRenderer::new_android_native_window(
                            native_win, w, h, dpi,
                        )
                    } {
                        Ok(mut renderer) => {
                            renderer.camera = vp.camera;
                            let _ = renderer.render_surface_frame(&doc.session.doc, vp.page_id);
                            vp.gpu_renderer = Some(renderer);
                            vp.android_native_window = native_win;
                        }
                        Err(_) => {
                            // SAFETY: Release acquired `ANativeWindow*` when falling back to software viewport.
                            unsafe { ANativeWindow_release(native_win) };
                        }
                    }
                }
            }
        }
        #[cfg(not(target_os = "android"))]
        let _ = (&env, &surface);

        Box::into_raw(Box::new(vp)) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDestroyViewport(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr != 0 {
            // SAFETY: `viewport_ptr` created by `nativeAttachVulkanSurface`.
            drop(unsafe { Box::from_raw(viewport_ptr as *mut VexioViewport) });
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeResizeViewport(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    width: jint,
    height: jint,
    density: jfloat,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr != 0 {
            // SAFETY: `viewport_ptr` created in `nativeAttachVulkanSurface`.
            let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
            let w = width.max(1) as u32;
            let h = height.max(1) as u32;
            let dpi = density.max(0.5) as f64;
            vp.camera.width_px = w;
            vp.camera.height_px = h;
            vp.camera.dpi_scale = dpi;
            if let Some(r) = &mut vp.gpu_renderer {
                r.resize_surface(w, h, dpi);
            }
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetViewportTool(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    tool_code: jint,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr != 0 {
            // SAFETY: Valid `VexioViewport` pointer.
            let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
            vp.active_tool = NativeCanvasTool::from_code(tool_code.max(0) as u32);
            vp.drag_state = None;
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetViewportPage(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    page_id: jint,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr != 0 && doc_ptr != 0 {
            // SAFETY: Valid `VexioViewport` and `VexioDoc` pointers.
            let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
            let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
            let requested = if page_id < 0 { u32::MAX } else { page_id as u32 };
            let target_page = doc
                .session
                .doc
                .page(requested)
                .or_else(|| doc.session.doc.pages.iter().find(|p| !p.is_background))
                .or_else(|| doc.session.doc.pages.first());
            let resolved_id = target_page.map(|p| p.id).unwrap_or(requested);
            vp.page_id = resolved_id;
            vp.selected_ids.clear();
            if let Some(p) = target_page {
                vp.camera.fit_to_page(p.width_in(), p.height_in());
            }
            vp.present_if_attached(&doc.session.doc);
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetViewportZoom(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    zoom_scale: jdouble,
    pan_x_in: jdouble,
    pan_y_in: jdouble,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr != 0 && doc_ptr != 0 {
            // SAFETY: Valid `VexioViewport` and `VexioDoc` pointers.
            let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
            let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
            if zoom_scale <= 0.0 {
                if let Some(p) = doc.session.doc.page(vp.page_id) {
                    vp.camera.fit_to_page(p.width_in(), p.height_in());
                }
            } else {
                vp.camera.zoom_ppi = 96.0 * zoom_scale.clamp(0.1, 16.0);
                if pan_x_in.is_finite() && pan_y_in.is_finite() && (pan_x_in != 0.0 || pan_y_in != 0.0) {
                    vp.camera.center_x_in = pan_x_in;
                    vp.camera.center_y_in = pan_y_in;
                }
            }
            vp.present_if_attached(&doc.session.doc);
        }
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRenderViewportFrame(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 || doc_ptr == 0 {
            return JNI_FALSE;
        }
        // SAFETY: Valid `VexioViewport` and `VexioDoc` pointers.
        let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        if vp.present_if_attached(&doc.session.doc) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRenderViewportRgba<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    out_direct_rgba: JByteBuffer<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 || doc_ptr == 0 {
            return 1;
        }
        let Ok(dst_ptr) = env.get_direct_buffer_address(&out_direct_rgba) else {
            return 1;
        };
        let Ok(cap) = env.get_direct_buffer_capacity(&out_direct_rgba) else {
            return 1;
        };
        // SAFETY: Valid `VexioViewport` and `VexioDoc` pointers.
        let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let needed = (vp.camera.width_px as usize) * (vp.camera.height_px as usize) * 4;
        if needed == 0 || cap < needed {
            return 1;
        }
        let Ok(rgba) = vexio_render::render_page_software_rgba_with_overlay(
            &doc.session.doc,
            vp.page_id,
            &vp.camera,
            true,
            &vp.selected_ids,
            Some(&vp.overlay),
        ) else {
            return 5;
        };
        // SAFETY: `dst_ptr` has `cap >= needed == rgba.len()`.
        unsafe {
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), dst_ptr, needed);
        }
        0
    }))
    .unwrap_or(99)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetPrimarySelection(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 {
            return 0;
        }
        // SAFETY: Valid `VexioViewport` pointer.
        let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
        vp.selected_ids.first().copied().unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativePushMotionBatch<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    direct_batch: JByteBuffer<'local>,
    count: jint,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 || doc_ptr == 0 || count <= 0 {
            return 0; // VexioCursorKind.ARROW
        }
        let Ok(ptr) = env.get_direct_buffer_address(&direct_batch) else {
            return 0;
        };
        let Ok(cap) = env.get_direct_buffer_capacity(&direct_batch) else {
            return 0;
        };
        if cap < 32 {
            return 0;
        }
        // SAFETY: `ptr` has `cap` bytes in the direct ByteBuffer, and `viewport_ptr`/`doc_ptr` are valid handles.
        let buf = unsafe { std::slice::from_raw_parts(ptr, cap) };
        let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };

        let max_samples = (cap / 32).min(count as usize);
        let mut last_c_cursor = 0u32;
        for idx in 0..max_samples {
            let off = idx * 32;
            let x_px = f32::from_le_bytes(buf[off..off + 4].try_into().unwrap()) as f64;
            let y_px = f32::from_le_bytes(buf[off + 4..off + 8].try_into().unwrap()) as f64;
            let pressure = f32::from_le_bytes(buf[off + 8..off + 12].try_into().unwrap());
            let tilt_rad = f32::from_le_bytes(buf[off + 12..off + 16].try_into().unwrap());
            let orient_rad = f32::from_le_bytes(buf[off + 16..off + 20].try_into().unwrap());
            let flags = u32::from_le_bytes(buf[off + 20..off + 24].try_into().unwrap());
            let ts_us = u64::from_le_bytes(buf[off + 24..off + 32].try_into().unwrap());

            let action = flags & 0xFF;
            let tool_type = (flags >> 8) & 0x0F;
            let kind = match action {
                0 => 1, // ACTION_DOWN -> Down
                2 | 3 => 2, // ACTION_UP / CANCEL -> Up
                5 => 3, // ACTION_SCROLL -> Wheel
                1 | 4 | 6 => {
                    if tool_type == 2 {
                        4 // StylusMove
                    } else {
                        0 // Move
                    }
                }
                _ => 0,
            };
            let mut buttons = 0u32;
            if (flags & (1 << 12)) != 0 || action == 0 || action == 1 {
                buttons |= 1;
            }
            if (flags & (1 << 13)) != 0 {
                buttons |= 2;
            }
            if (flags & (1 << 14)) != 0 {
                buttons |= 4;
            }
            let mut modifiers = 0u32;
            if (flags & (1 << 20)) != 0 {
                modifiers |= 1;
            }
            if (flags & (1 << 21)) != 0 {
                modifiers |= 2;
            }
            if (flags & (1 << 22)) != 0 {
                modifiers |= 4;
            }
            if (flags & (1 << 23)) != 0 {
                modifiers |= 8;
            }

            let ev = VexioPointerEvent {
                kind,
                buttons,
                modifiers,
                x_px,
                y_px,
                pressure,
                tilt_x_deg: (tilt_rad * orient_rad.cos()).to_degrees(),
                tilt_y_deg: if action == 5 {
                    tilt_rad
                } else {
                    (tilt_rad * orient_rad.sin()).to_degrees()
                },
                timestamp_us: ts_us,
            };
            last_c_cursor = vp.dispatch_pointer_internal(&mut doc.session, ev);
        }

        // Map internal C cursor code to `VexioCursorKind` in `NativeBridge.kt`:
        // 0=ARROW, 1=CROSSHAIR, 2=MOVE_ALL, 3=RESIZE_NS, 4=RESIZE_EW, 5=RESIZE_NWSE, 6=RESIZE_NESW, 7=TEXT_IBEAM, 8=GRAB_HAND, 9=GRABBING_HAND
        match last_c_cursor {
            1 => 2,  // MOVE_ALL
            2 => 1,  // CROSSHAIR
            3 => {
                if vp.drag_state.is_some() {
                    9 // GRABBING_HAND
                } else {
                    8 // GRAB_HAND
                }
            }
            5 => 3,  // RESIZE_NS
            6 => 4,  // RESIZE_EW
            7 => 7,  // TEXT_IBEAM
            11 => 5, // RESIZE_NWSE
            12 => 6, // RESIZE_NESW
            13 => {
                if vp.drag_state.is_some() {
                    9 // GRABBING_HAND (active rotation drag)
                } else {
                    8 // GRAB_HAND (hovering ↻ Rotate button)
                }
            }
            _ => 0, // ARROW
        }
    }))
    .unwrap_or(0)
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeTransformShape(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    pin_x_in: jdouble,
    pin_y_in: jdouble,
    width_in: jdouble,
    height_in: jdouble,
    angle_deg: jdouble,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` created by `nativeCreateBlankDocument` or `nativeOpenDocument`.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .transform_shape(
                page_id.max(0) as u32,
                shape_id as u64,
                pin_x_in,
                pin_y_in,
                width_in,
                height_in,
                angle_deg,
            )
            .is_ok()
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetCellFormula<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    cell_name: JString<'local>,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return env.new_string("").unwrap().into_raw();
        }
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let cname: String = env
            .get_string(&cell_name)
            .map(Into::into)
            .unwrap_or_default();
        let formula = doc
            .session
            .doc
            .page(page_id as u32)
            .and_then(|p| p.find_shape(shape_id as u64))
            .and_then(|s| s.sheet.get_cell(&cname))
            .map(|c| c.formula.clone())
            .unwrap_or_default();
        env.new_string(formula).unwrap().into_raw()
    }))
    .unwrap_or_else(|_| std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetCellFormula<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    cell_name: JString<'local>,
    formula: JString<'local>,
    force_setf: jboolean,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return -1;
        }
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let cname: String = env.get_string(&cell_name).map(Into::into).unwrap_or_default();
        let fstr: String = env.get_string(&formula).map(Into::into).unwrap_or_default();
        match doc.session.set_cell_formula(
            page_id as u32,
            shape_id as u64,
            &cname,
            &fstr,
            force_setf == JNI_TRUE,
        ) {
            Ok(()) => 0,                                                       // FormulaCommitStatus.OK
            Err(CoreError::Journal(JournalError::CellGuarded(_))) => 1,        // GUARDED_CELL_PROTECTED
            Err(_) => 2,                                                       // SYNTAX_ERROR
        }
    }))
    .unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeListStencilsJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return env.new_string("{}").unwrap().into_raw();
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let json = build_stencils_catalog_json(Some(&doc.session));
        env.new_string(json).unwrap().into_raw()
    }))
    .unwrap_or_else(|_| std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeLoadStarterTemplate<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    template_id: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        let Ok(tid) = env.get_string(&template_id).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session.load_starter_template(&tid);
        JNI_TRUE
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetStateJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    active_page_id: jint,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return env.new_string("{}").unwrap().into_raw();
        }
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let requested = if active_page_id < 0 {
            u32::MAX
        } else {
            active_page_id as u32
        };
        let json = build_native_state_json(&doc.session, requested);
        env.new_string(json).unwrap().into_raw()
    }))
    .unwrap_or_else(|_| std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDropStencil<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    category: JString<'local>,
    master_name: JString<'local>,
    x_in: jdouble,
    y_in: jdouble,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let cat: String = env.get_string(&category).map(Into::into).unwrap_or_default();
        let Ok(master) = env.get_string(&master_name).map(String::from) else {
            return 0;
        };
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .drop_builtin_shape(page_id.max(0) as u32, &cat, &master, x_in, y_in)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDrawPrimitive<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    kind: JString<'local>,
    x1_in: jdouble,
    y1_in: jdouble,
    x2_in: jdouble,
    y2_in: jdouble,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(k) = env.get_string(&kind).map(String::from) else {
            return 0;
        };
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .draw_primitive_2d(page_id.max(0) as u32, &k, x1_in, y1_in, x2_in, y2_in)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeConnectShapes(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    from_shape_id: jlong,
    to_shape_id: jlong,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || from_shape_id <= 0 || to_shape_id <= 0 {
            return 0;
        }
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .connect_shapes(page_id.max(0) as u32, from_shape_id as u64, to_shape_id as u64)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDeleteShapes<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .delete_shapes(page_id.max(0) as u32, &ids)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDuplicateShapes<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    dx_in: jdouble,
    dy_in: jdouble,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .duplicate_shapes(page_id.max(0) as u32, &ids, dx_in, dy_in)
            .ok()
            .and_then(|v| v.first().copied())
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeReorderShapes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    mode: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        let mode_str: String = env.get_string(&mode).map(Into::into).unwrap_or_else(|_| "front".into());
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .reorder_shapes(page_id.max(0) as u32, &ids, &mode_str)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetShapeText<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    text: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(txt) = env.get_string(&text).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_shape_text(page_id.max(0) as u32, shape_id as u64, &txt)
            .is_ok()
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAddPage<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    name: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(nm) = env.get_string(&name).map(String::from) else {
            return 0;
        };
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session.add_page(&nm).unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAutoLayout<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    algo: JString<'local>,
) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return;
        }
        let alg: String = env.get_string(&algo).map(Into::into).unwrap_or_else(|_| "tb".into());
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session.layout(page_id.max(0) as u32, &alg);
    }));
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeValidateJson<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    ruleset: JString<'local>,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return env.new_string("[]").unwrap().into_raw();
        }
        let rs: String = env.get_string(&ruleset).map(Into::into).unwrap_or_else(|_| "bpmn".into());
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let issues = doc.session.validate(&rs);
        let json = serde_json::to_string(&issues).unwrap_or_else(|_| "[]".into());
        env.new_string(json).unwrap().into_raw()
    }))
    .unwrap_or_else(|_| std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeUndo(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc.session.journal.undo(&mut doc.session.doc).unwrap_or(false) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRedo(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        // SAFETY: Valid `VexioDoc` pointer.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc.session.journal.redo(&mut doc.session.doc).unwrap_or(false) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRenderPreviewRgba<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    width: jint,
    height: jint,
    out_direct_rgba: JByteBuffer<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || width <= 0 || height <= 0 {
            return 1;
        }
        let Ok(dst_ptr) = env.get_direct_buffer_address(&out_direct_rgba) else {
            return 1;
        };
        let Ok(cap) = env.get_direct_buffer_capacity(&out_direct_rgba) else {
            return 1;
        };
        let needed = (width as usize) * (height as usize) * 4;
        if cap < needed {
            return 1;
        }
        // SAFETY: Valid `VexioDoc` pointer and verified `DirectByteBuffer` capacity.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let Ok(rgba) = vexio_render::render_page_rgba_auto(
            &doc.session.doc,
            page_id as u32,
            width as u32,
            height as u32,
        ) else {
            return 5;
        };
        // SAFETY: `dst_ptr` has `cap >= needed == rgba.len()`.
        unsafe {
            std::ptr::copy_nonoverlapping(rgba.as_ptr(), dst_ptr, needed);
        }
        0
    }))
    .unwrap_or(99)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeBooleanOp<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    op: JString<'local>,
    offset_in: jdouble,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        let Ok(op_str) = env.get_string(&op).map(String::from) else {
            return 0;
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
            _ => return 0,
        };
        // SAFETY: `doc_ptr` verified non-zero handle allocated by `nativeCreateBlankDocument` or `nativeOpenDocument`.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut created_len = 0usize;
        let _ = doc.session.journal.record_mutation(
            &mut doc.session.doc,
            format!("Boolean {op_str}"),
            |doc_mut| {
                created_len = crate::boolean_ops::apply_boolean_operation(
                    doc_mut,
                    page_id as u32,
                    &ids,
                    kind,
                )
                .len();
            },
        );
        created_len as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeApplyTheme<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    theme_slug: JString<'local>,
    variant_index: jint,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(slug) = env.get_string(&theme_slug).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut count = 0usize;
        let _ = doc
            .session
            .journal
            .record_mutation(&mut doc.session.doc, "Apply Theme", |doc_mut| {
                count = crate::themes_styles::apply_document_theme_variant(
                    doc_mut,
                    &slug,
                    variant_index.clamp(0, 3) as u8,
                );
            });
        count as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeApplyVisualPreset<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    preset: JString<'local>,
) -> jboolean {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(preset_str) = env.get_string(&preset).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let spec = crate::ffi::c_abi::visual_preset_spec_from_slug(&preset_str, shape_id as u64);
        let mut applied = false;
        let _ = doc.session.journal.record_mutation(
            &mut doc.session.doc,
            "Apply Visual Preset",
            |doc_mut| {
                applied = crate::themes_styles::apply_visual_effects_to_shape(
                    doc_mut,
                    page_id as u32,
                    shape_id as u64,
                    &spec,
                );
            },
        );
        if applied { JNI_TRUE } else { JNI_FALSE }
    }));
    res.unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeCreateSubprocess<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    sub_name: JString<'local>,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 {
            return 0;
        }
        let name: String = env.get_string(&sub_name).map(Into::into).unwrap_or_default();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut pid = 0u32;
        let _ = doc.session.journal.record_mutation(
            &mut doc.session.doc,
            "Create Subprocess",
            |doc_mut| {
                pid = crate::enterprise_plan2::create_or_link_subprocess_page(
                    doc_mut,
                    page_id as u32,
                    shape_id as u64,
                    &name,
                )
                .unwrap_or(0);
            },
        );
        pid as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeCompareDocuments<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    base_doc_ptr: jlong,
    rev_doc_ptr: jlong,
) -> jstring {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if base_doc_ptr == 0 || rev_doc_ptr == 0 {
            return env.new_string("{}").unwrap().into_raw();
        }
        // SAFETY: Both document pointers verified non-zero handles.
        let base = unsafe { &*(base_doc_ptr as *const VexioDoc) };
        let rev = unsafe { &*(rev_doc_ptr as *const VexioDoc) };
        let rep = crate::enterprise_plan2::compare_documents(&base.session.doc, &rev.session.doc);
        let json = serde_json::to_string(&rep).unwrap_or_else(|_| "{}".into());
        env.new_string(json).unwrap().into_raw()
    }));
    res.unwrap_or_else(|_| std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeApplyCrdtJson<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    replica_id: JString<'local>,
    ops_json: JString<'local>,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(rep) = env.get_string(&replica_id).map(String::from) else {
            return 0;
        };
        let Ok(ops) = env.get_string(&ops_json).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut crdt = crate::crdt::CrdtDocumentState::new(&rep);
        crdt.apply_batch_json(&mut doc.session.doc, &ops).unwrap_or(0) as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeImportDataVisualizer<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    csv_or_xlsx: JByteArray<'local>,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(bytes) = env.convert_byte_array(&csv_or_xlsx) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut count = 0usize;
        let _ = doc.session.journal.record_mutation(
            &mut doc.session.doc,
            "Import Data Visualizer",
            |doc_mut| {
                count = crate::plan2_connectors::import_data_visualizer_csv_or_xlsx(
                    doc_mut,
                    page_id as u32,
                    &bytes,
                )
                .unwrap_or(0);
            },
        );
        count as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeImportCloudDiscovery<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    discovery_json: JString<'local>,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(json_str) = env.get_string(&discovery_json).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut count = 0usize;
        let _ = doc.session.journal.record_mutation(
            &mut doc.session.doc,
            "Import Cloud Discovery",
            |doc_mut| {
                count = crate::plan2_connectors::import_cloud_discovery_json(
                    doc_mut,
                    page_id as u32,
                    &json_str,
                )
                .unwrap_or(0);
            },
        );
        count as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeImportEntraOrLdif<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    text: JString<'local>,
) -> jint {
    let res = catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(txt) = env.get_string(&text).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let mut count = 0usize;
        let _ = doc.session.journal.record_mutation(
            &mut doc.session.doc,
            "Import Directory Org Chart",
            |doc_mut| {
                count = crate::plan2_connectors::import_entra_or_ldif_org_chart(
                    doc_mut,
                    page_id as u32,
                    &txt,
                )
                .unwrap_or(0);
            },
        );
        count as jint
    }));
    res.unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetViewportTool(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
) -> jint {
    if viewport_ptr == 0 {
        return 0;
    }
    // SAFETY: `viewport_ptr` created by `nativeAttachVulkanSurface`.
    let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
    vp.active_tool as jint
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativePopCreatedTextShape(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
) -> jlong {
    if viewport_ptr == 0 {
        return 0;
    }
    // SAFETY: `viewport_ptr` created by `nativeAttachVulkanSurface`.
    let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
    vp.last_created_text_shape_id.take().unwrap_or(0) as jlong
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeHitTestShape(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    x_px: jdouble,
    y_px: jdouble,
) -> jlong {
    if viewport_ptr == 0 || doc_ptr == 0 {
        return 0;
    }
    // SAFETY: Non-null `VexioViewport` and `VexioDoc` handles managed by `NativeBridge`.
    unsafe {
        crate::ffi::c_abi::vexio_viewport_hit_test_shape(
            viewport_ptr as *const VexioViewport,
            doc_ptr as *const VexioDoc,
            x_px,
            y_px,
        ) as jlong
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetShapeScreenBoundsJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    shape_id: jlong,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 || doc_ptr == 0 || shape_id <= 0 {
            return env
                .new_string("{}")
                .map(|s| s.into_raw())
                .unwrap_or(std::ptr::null_mut());
        }
        let (mut x, mut y, mut w, mut h) = (0.0, 0.0, 0.0, 0.0);
        // SAFETY: Non-null handles and valid stack output pointers.
        let status = unsafe {
            crate::ffi::c_abi::vexio_viewport_get_shape_screen_bounds(
                viewport_ptr as *const VexioViewport,
                doc_ptr as *const VexioDoc,
                shape_id as u64,
                &mut x,
                &mut y,
                &mut w,
                &mut h,
            )
        };
        let json = if status == crate::ffi::c_abi::VexioStatus::Ok {
            format!("{{\"x\":{x:.2},\"y\":{y:.2},\"w\":{w:.2},\"h\":{h:.2}}}")
        } else {
            "{}".to_string()
        };
        env.new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeImportStencilBytes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    file_name: JString<'local>,
    bytes: JByteArray<'local>,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return std::ptr::null_mut();
        }
        let Ok(fname) = env.get_string(&file_name).map(String::from) else {
            return std::ptr::null_mut();
        };
        let Ok(raw) = env.convert_byte_array(&bytes) else {
            return std::ptr::null_mut();
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let json = match doc.session.import_stencil_archive_packs(&fname, &raw) {
            Ok(packs) => {
                let first_title = packs
                    .first()
                    .map(|(t, _)| t.clone())
                    .unwrap_or_else(|| "Imported Visio Stencil".into());
                let mut all_masters = Vec::new();
                let mut packs_json = Vec::with_capacity(packs.len());
                for (pt, pv) in packs {
                    packs_json.push(serde_json::json!({
                        "packTitle": pt,
                        "masters": pv,
                    }));
                    all_masters.extend(pv);
                }
                serde_json::json!({
                    "packTitle": first_title,
                    "masters": all_masters,
                    "packs": packs_json,
                })
                .to_string()
            }
            Err(_) => "{}".to_string(),
        };
        env.new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeListOfficialStencilCatalogJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
) -> jstring {
    catch_unwind(AssertUnwindSafe(|| {
        let catalog = crate::domains::list_official_stencil_catalog();
        let json = serde_json::to_string(&catalog).unwrap_or_else(|_| "[]".into());
        env.new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetShapeTextStyleJson<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    style_json: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len.max(0) as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let Ok(json_s) = env.get_string(&style_json).map(String::from) else {
            return 0;
        };
        let Ok(patch) = serde_json::from_str::<crate::workspace::TextStylePatch>(&json_s) else {
            return 0;
        };
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .apply_text_style_patch(page_id.max(0) as u32, &ids, &patch)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRotateFlipShapes<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    delta_angle_deg: jdouble,
    toggle_flip_x: jboolean,
    toggle_flip_y: jboolean,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len.max(0) as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .rotate_or_flip_shapes(
                page_id.max(0) as u32,
                &ids,
                delta_angle_deg,
                toggle_flip_x == JNI_TRUE,
                toggle_flip_y == JNI_TRUE,
            )
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetPageSetup<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    width_in: jdouble,
    height_in: jdouble,
    unit_slug: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        let unit_s = env
            .get_string(&unit_slug)
            .map(String::from)
            .unwrap_or_else(|_| "mm".into());
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_page_setup(page_id.max(0) as u32, width_in, height_in, &unit_s)
            .is_ok()
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetPagePreset<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    preset_slug: JString<'local>,
    landscape: jboolean,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        let Ok(slug) = env.get_string(&preset_slug).map(String::from) else {
            return JNI_FALSE;
        };
        let Some((w_in, h_in, unit)) =
            crate::workspace::resolve_page_preset_dimensions_in(&slug, landscape == JNI_TRUE)
        else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_page_setup(page_id.max(0) as u32, w_in, h_in, unit)
            .is_ok()
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetDocumentTitle<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    title: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        let Ok(t) = env.get_string(&title).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session.set_document_title(&t);
        JNI_TRUE
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRenamePage<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    new_name: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        let Ok(name_s) = env.get_string(&new_name).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc.session.rename_page(page_id.max(0) as u32, &name_s) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRegisterFontBytes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    font_bytes: JByteArray<'local>,
    family_override: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        let Ok(bytes) = env.convert_byte_array(&font_bytes) else {
            return JNI_FALSE;
        };
        let fam_opt = if family_override.is_null() {
            None
        } else {
            env.get_string(&family_override)
                .map(String::from)
                .ok()
                .filter(|s| !s.trim().is_empty())
        };
        if DocumentSession::register_font_bytes(&bytes, fam_opt.as_deref()) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAlignShapes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    mode: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        let Ok(mode_s) = env.get_string(&mode).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .align_shapes(page_id.max(0) as u32, &ids, &mode_s)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDistributeShapes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    axis: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        let Ok(axis_s) = env.get_string(&axis).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .distribute_shapes(page_id.max(0) as u32, &ids, &axis_s)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGroupShapes<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(len) = env.get_array_length(&shape_ids) else {
            return 0;
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .group_shapes(page_id.max(0) as u32, &ids)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeUngroupShapes<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    group_ids: JLongArray<'local>,
) -> jlongArray {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return std::ptr::null_mut();
        }
        let Ok(len) = env.get_array_length(&group_ids) else {
            return std::ptr::null_mut();
        };
        let mut raw_ids = vec![0i64; len as usize];
        if env.get_long_array_region(&group_ids, 0, &mut raw_ids).is_err() {
            return std::ptr::null_mut();
        }
        let gids: Vec<u64> = raw_ids.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let restored = doc
            .session
            .ungroup_shapes(page_id.max(0) as u32, &gids)
            .unwrap_or_default();
        let i64_vec: Vec<i64> = restored.into_iter().map(|id| id as i64).collect();
        let Ok(out_arr) = env.new_long_array(i64_vec.len() as i32) else {
            return std::ptr::null_mut();
        };
        let _ = env.set_long_array_region(&out_arr, 0, &i64_vec);
        out_arr.into_raw()
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDragConnectorSegment(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    seg_index: jint,
    coord_in: jdouble,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 || !coord_in.is_finite() {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .drag_connector_segment(
                page_id.max(0) as u32,
                shape_id as u64,
                seg_index.max(0) as usize,
                coord_in,
            )
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetSmartGuides(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
    enabled: jboolean,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 {
            return JNI_FALSE;
        }
        // SAFETY: `viewport_ptr` verified non-zero handle.
        let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
        vp.smart_guides_enabled = enabled == JNI_TRUE;
        if !vp.smart_guides_enabled {
            vp.overlay.smart_guides.clear();
        }
        JNI_TRUE
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetSmartGuides(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    viewport_ptr: jlong,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 {
            return JNI_FALSE;
        }
        // SAFETY: `viewport_ptr` verified non-zero handle.
        let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
        if vp.smart_guides_enabled {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetSelectionIds<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    viewport_ptr: jlong,
) -> jlongArray {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: `viewport_ptr` verified non-zero handle.
        let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
        let i64_vec: Vec<i64> = vp.selected_ids.iter().map(|&id| id as i64).collect();
        let Ok(out_arr) = env.new_long_array(i64_vec.len() as i32) else {
            return std::ptr::null_mut();
        };
        let _ = env.set_long_array_region(&out_arr, 0, &i64_vec);
        out_arr.into_raw()
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetSelectionIds<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    viewport_ptr: jlong,
    doc_ptr: jlong,
    shape_ids: JLongArray<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 {
            return JNI_FALSE;
        }
        let ids: Vec<u64> = if shape_ids.is_null() {
            Vec::new()
        } else {
            let Ok(len) = env.get_array_length(&shape_ids) else {
                return JNI_FALSE;
            };
            let mut raw = vec![0i64; len as usize];
            if env.get_long_array_region(&shape_ids, 0, &mut raw).is_err() {
                return JNI_FALSE;
            }
            raw.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect()
        };
        // SAFETY: `viewport_ptr` verified non-zero handle.
        let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
        if doc_ptr != 0 {
            // SAFETY: `doc_ptr` verified non-zero handle.
            let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
            if let Some(page) = doc.session.doc.page(vp.page_id) {
                let mut normalized = Vec::new();
                for id in ids {
                    if let Some(top_id) = page.top_level_shape_id(id) {
                        if !normalized.contains(&top_id) {
                            normalized.push(top_id);
                        }
                    }
                }
                vp.selected_ids = normalized;
            } else {
                vp.selected_ids = ids;
            }
            vp.present_if_attached(&doc.session.doc);
        } else {
            vp.selected_ids = ids;
        }
        JNI_TRUE
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeViewportScreenToPageIn<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    viewport_ptr: jlong,
    x_px: jdouble,
    y_px: jdouble,
) -> jni::sys::jdoubleArray {
    catch_unwind(AssertUnwindSafe(|| {
        if viewport_ptr == 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: `viewport_ptr` verified non-zero handle.
        let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
        let pt = if x_px.is_finite() && y_px.is_finite() && (x_px >= 0.0 || y_px >= 0.0) {
            vp.camera.screen_to_page_in(x_px, y_px)
        } else {
            [vp.camera.center_x_in, vp.camera.center_y_in]
        };
        let Ok(out_arr) = env.new_double_array(2) else {
            return std::ptr::null_mut();
        };
        let _ = env.set_double_array_region(&out_arr, 0, &pt);
        out_arr.into_raw()
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeInsertContainer<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    title: JString<'local>,
    member_ids: JLongArray<'local>,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return 0;
        }
        let title_str: String = env
            .get_string(&title)
            .map(Into::into)
            .unwrap_or_else(|_| "Container".to_string());
        let ids: Vec<u64> = if member_ids.is_null() {
            Vec::new()
        } else {
            let Ok(len) = env.get_array_length(&member_ids) else {
                return 0;
            };
            let mut raw = vec![0i64; len as usize];
            if env.get_long_array_region(&member_ids, 0, &mut raw).is_err() {
                return 0;
            }
            raw.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect()
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .insert_container(page_id as u32, &title_str, &ids)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeInsertSwimlanes<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    lanes_semicolon: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return 0;
        }
        let lanes_str: String = env
            .get_string(&lanes_semicolon)
            .map(Into::into)
            .unwrap_or_default();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .insert_swimlanes(page_id as u32, &lanes_str)
            .map(|v| v.len() as jint)
            .unwrap_or(0)
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAttachCallout<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    target_shape_id: jlong,
    text: JString<'local>,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return 0;
        }
        let text_str: String = env
            .get_string(&text)
            .map(Into::into)
            .unwrap_or_else(|_| "Callout Note".to_string());
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let tid = if target_shape_id > 0 {
            target_shape_id as u64
        } else {
            0
        };
        doc.session
            .insert_callout(page_id as u32, tid, &text_str)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAutoConnect<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    source_shape_id: jlong,
    dir: JString<'local>,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || source_shape_id <= 0 {
            return 0;
        }
        let dir_str: String = env
            .get_string(&dir)
            .map(Into::into)
            .unwrap_or_else(|_| "east".to_string());
        let Some(ac_dir) = vexio_render::AutoConnectDir::from_slug(&dir_str) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .auto_connect_shape(page_id as u32, source_shape_id as u64, ac_dir)
            .map(|(tid, _)| tid as jlong)
            .unwrap_or(0)
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDragControlHandle<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    row_name: JString<'local>,
    page_x_in: jdouble,
    page_y_in: jdouble,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(r_str) = env.get_string(&row_name) else {
            return JNI_FALSE;
        };
        let row_rust: String = r_str.into();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .drag_control_handle(
                page_id as u32,
                shape_id as u64,
                &row_rust,
                page_x_in,
                page_y_in,
            )
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRenameShape<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    new_name: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(name_str) = env.get_string(&new_name).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .rename_shape(page_id as u32, shape_id as u64, &name_str)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeUpsertShapeData<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    row_name: JString<'local>,
    label: JString<'local>,
    data_type: jint,
    value_str: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(r_name) = env.get_string(&row_name).map(String::from) else {
            return JNI_FALSE;
        };
        let lbl = env.get_string(&label).map(String::from).unwrap_or_default();
        let val = env
            .get_string(&value_str)
            .map(String::from)
            .unwrap_or_default();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .upsert_shape_data(
                page_id as u32,
                shape_id as u64,
                &r_name,
                &lbl,
                data_type.clamp(0, 7) as u8,
                &val,
            )
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDeleteShapeData<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    row_name: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(r_name) = env.get_string(&row_name).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .delete_shape_data(page_id as u32, shape_id as u64, &r_name)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeAddLayer<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    layer_name: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return -1;
        }
        let l_name = env
            .get_string(&layer_name)
            .map(String::from)
            .unwrap_or_default();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .add_layer(page_id as u32, &l_name)
            .map(|ix| ix as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetLayerFlags(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    layer_ix: jint,
    visible: jboolean,
    lock_flag: jboolean,
    print_flag: jboolean,
    snap_glue: jboolean,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || layer_ix < 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_layer_flags(
                page_id as u32,
                layer_ix as u32,
                visible != JNI_FALSE,
                lock_flag != JNI_FALSE,
                print_flag != JNI_FALSE,
                snap_glue != JNI_FALSE,
            )
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetShapeLayerMembership(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    layer_ix: jint,
    assigned: jboolean,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 || layer_ix < 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_shape_layer_membership(
                page_id as u32,
                shape_id as u64,
                layer_ix as u32,
                assigned != JNI_FALSE,
            )
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

// ============================================================================
// Pillar #4 JNI: Live Data Graphics, Legends & Two-Way External Data Linking
// ============================================================================

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeGetDataLinkStateJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
) -> jni::sys::jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let json = doc.session.get_page_data_link_state_json(page_id as u32);
        env.new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetPageDataGraphicsJson<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    config_json: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || config_json.is_null() {
            return -1;
        }
        let Ok(cfg_str) = env.get_string(&config_json).map(String::from) else {
            return -1;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .set_page_data_graphics_json(page_id as u32, &cfg_str)
            .map(|c| c as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeImportExternalRecordset<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    file_name: JString<'local>,
    raw_bytes: JByteArray<'local>,
    key_col_hint: JString<'local>,
    table_hint: JString<'local>,
    auto_bind: jboolean,
    auto_enable_graphics: jboolean,
) -> jni::sys::jintArray {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || raw_bytes.is_null() {
            return std::ptr::null_mut();
        }
        let fname = if file_name.is_null() {
            "external_data.csv".into()
        } else {
            env.get_string(&file_name)
                .map(String::from)
                .unwrap_or_else(|_| "external_data.csv".into())
        };
        let key_col = if key_col_hint.is_null() {
            String::new()
        } else {
            env.get_string(&key_col_hint)
                .map(String::from)
                .unwrap_or_default()
        };
        let tbl = if table_hint.is_null() {
            String::new()
        } else {
            env.get_string(&table_hint)
                .map(String::from)
                .unwrap_or_default()
        };
        let Ok(bytes) = env.convert_byte_array(&raw_bytes) else {
            return std::ptr::null_mut();
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let Ok((rs_id, linked)) = doc.session.import_external_recordset(
            page_id as u32,
            &fname,
            &bytes,
            &key_col,
            &tbl,
            auto_bind != JNI_FALSE,
            auto_enable_graphics != JNI_FALSE,
        ) else {
            return std::ptr::null_mut();
        };
        let pair = [rs_id as i32, linked as i32];
        let Ok(arr) = env.new_int_array(2) else {
            return std::ptr::null_mut();
        };
        let _ = env.set_int_array_region(&arr, 0, &pair);
        arr.into_raw()
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeBindExternalRow<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    recordset_id: jint,
    row_index: jint,
    shape_ids: JLongArray<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || recordset_id <= 0 || row_index < 0 {
            return 0;
        }
        let ids: Vec<u64> = if shape_ids.is_null() {
            Vec::new()
        } else {
            let Ok(len) = env.get_array_length(&shape_ids) else {
                return 0;
            };
            let mut raw = vec![0i64; len as usize];
            if env.get_long_array_region(&shape_ids, 0, &mut raw).is_err() {
                return 0;
            }
            raw.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect()
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .bind_external_row_to_shapes(
                page_id as u32,
                recordset_id as u32,
                row_index as usize,
                &ids,
            )
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRefreshExternalRecordset<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    recordset_id: jint,
    optional_new_bytes: JByteArray<'local>,
    format_hint: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return 0;
        }
        let fmt = if format_hint.is_null() {
            String::new()
        } else {
            env.get_string(&format_hint)
                .map(String::from)
                .unwrap_or_default()
        };
        let bytes_opt = if optional_new_bytes.is_null() {
            None
        } else {
            env.convert_byte_array(&optional_new_bytes)
                .ok()
                .filter(|b| !b.is_empty())
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .refresh_external_recordset(
                page_id as u32,
                recordset_id.max(0) as u32,
                bytes_opt.as_deref(),
                &fmt,
            )
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSyncAndExportRecordset<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    recordset_id: jint,
    format_ext: JString<'local>,
) -> jni::sys::jbyteArray {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return std::ptr::null_mut();
        }
        let fmt = if format_ext.is_null() {
            "xlsx".into()
        } else {
            env.get_string(&format_ext)
                .map(String::from)
                .unwrap_or_else(|_| "xlsx".into())
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let Ok(bytes) = doc.session.sync_and_export_recordset(
            page_id.max(1) as u32,
            recordset_id.max(0) as u32,
            &fmt,
        ) else {
            return std::ptr::null_mut();
        };
        env.byte_array_from_slice(&bytes)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDeleteExternalRecordset(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    recordset_id: jint,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || recordset_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .delete_external_recordset(recordset_id as u32)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeInsertDataLegend<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    prop_name: JString<'local>,
    pin_x_in: jdouble,
    pin_y_in: jdouble,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return 0;
        }
        let prop = if prop_name.is_null() {
            String::new()
        } else {
            env.get_string(&prop_name)
                .map(String::from)
                .unwrap_or_default()
        };
        let px = if pin_x_in > 0.0 { Some(pin_x_in) } else { None };
        let py = if pin_y_in > 0.0 { Some(pin_y_in) } else { None };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .insert_or_update_data_legend(page_id as u32, &prop, px, py)
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeExportBomReport<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    format_ext: JString<'local>,
) -> jni::sys::jbyteArray {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return std::ptr::null_mut();
        }
        let fmt = if format_ext.is_null() {
            "xlsx".into()
        } else {
            env.get_string(&format_ext)
                .map(String::from)
                .unwrap_or_else(|_| "xlsx".into())
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let Ok(bytes) = doc.session.export_bom_report_bytes(&fmt) else {
            return std::ptr::null_mut();
        };
        env.byte_array_from_slice(&bytes)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDuplicatePage(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return 0;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .duplicate_page(page_id as u32)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDeletePage(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc.session.delete_page(page_id as u32) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetPageBackground<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    is_background: jboolean,
    back_page_id: jint,
    page_color_formula: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return JNI_FALSE;
        }
        let color_s = if page_color_formula.is_null() {
            String::new()
        } else {
            env.get_string(&page_color_formula)
                .map(String::from)
                .unwrap_or_default()
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc.session.set_page_background(
            page_id as u32,
            is_background != JNI_FALSE,
            back_page_id.max(0) as u32,
            &color_s,
        ) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeApplyHeadingPreset<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: JLongArray<'local>,
    preset_slug: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_ids.is_null() || preset_slug.is_null() {
            return 0;
        }
        let Ok(preset_s) = env.get_string(&preset_slug).map(String::from) else {
            return 0;
        };
        let len = env.get_array_length(&shape_ids).unwrap_or(0).max(0) as usize;
        let mut raw_ids = vec![0i64; len];
        if len > 0 && env.get_long_array_region(&shape_ids, 0, &mut raw_ids).is_err() {
            return 0;
        }
        let ids: Vec<u64> = raw_ids.into_iter().filter(|&v| v > 0).map(|v| v as u64).collect();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .apply_text_heading_preset(page_id as u32, &ids, &preset_s)
            .unwrap_or(0) as jint
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeInsertHeading<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    preset_slug: JString<'local>,
    custom_text: JString<'local>,
    x_in: jdouble,
    y_in: jdouble,
) -> jlong {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || preset_slug.is_null() {
            return 0;
        }
        let Ok(preset_s) = env.get_string(&preset_slug).map(String::from) else {
            return 0;
        };
        let text_opt = if custom_text.is_null() {
            None
        } else {
            env.get_string(&custom_text).map(String::from).ok()
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .insert_text_heading(
                page_id as u32,
                &preset_s,
                text_opt.as_deref(),
                x_in,
                y_in,
            )
            .unwrap_or(0) as jlong
    }))
    .unwrap_or(0)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeApplyBorderAndTitle<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    style_slug: JString<'local>,
    title: JString<'local>,
    subtitle: JString<'local>,
    author: JString<'local>,
    date_or_rev: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || style_slug.is_null() {
            return -1;
        }
        let read_s = |env: &mut JNIEnv<'local>, js: &JString<'local>| -> String {
            if js.is_null() {
                String::new()
            } else {
                env.get_string(js).map(String::from).unwrap_or_default()
            }
        };
        let style_s = read_s(&mut env, &style_slug);
        let title_s = read_s(&mut env, &title);
        let sub_s = read_s(&mut env, &subtitle);
        let auth_s = read_s(&mut env, &author);
        let date_s = read_s(&mut env, &date_or_rev);
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .apply_border_and_title(
                page_id as u32,
                &style_s,
                &title_s,
                &sub_s,
                &auth_s,
                &date_s,
            )
            .map(|id| id as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

// ============================================================================
// v0.5.0-beta JNI: Mermaid / PlantUML / Graphviz DOT, Slide Snippets & Multi-Bend Routing
// ============================================================================

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeImportMarkup<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    markup: JString<'local>,
    format_slug: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || markup.is_null() {
            return -1;
        }
        let Ok(markup_s) = env.get_string(&markup).map(String::from) else {
            return -1;
        };
        let fmt_s = if format_slug.is_null() {
            "auto".into()
        } else {
            env.get_string(&format_slug)
                .map(String::from)
                .unwrap_or_else(|_| "auto".into())
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .import_diagram_markup(page_id as u32, &markup_s, &fmt_s)
            .map(|st| st.nodes_created as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeExportMarkup<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    format_slug: JString<'local>,
) -> jni::sys::jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return std::ptr::null_mut();
        }
        let fmt_s = if format_slug.is_null() {
            "mermaid".into()
        } else {
            env.get_string(&format_slug)
                .map(String::from)
                .unwrap_or_else(|_| "mermaid".into())
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let Ok(out) = doc.session.export_diagram_markup(page_id as u32, &fmt_s) else {
            return std::ptr::null_mut();
        };
        env.new_string(out)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeListSlideSnippetsJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
) -> jni::sys::jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let json = doc.session.list_slide_snippets_json();
        env.new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeAddSlideSnippet<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    title: JString<'local>,
    notes: JString<'local>,
    x_in: jdouble,
    y_in: jdouble,
    width_in: jdouble,
    height_in: jdouble,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return -1;
        }
        let title_s = if title.is_null() {
            String::new()
        } else {
            env.get_string(&title).map(String::from).unwrap_or_default()
        };
        let notes_s = if notes.is_null() {
            String::new()
        } else {
            env.get_string(&notes).map(String::from).unwrap_or_default()
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .add_slide_snippet(
                page_id as u32,
                &title_s,
                &notes_s,
                x_in,
                y_in,
                width_in,
                height_in,
            )
            .map(|id| id as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeDeleteSlideSnippet(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    snippet_id: jint,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .delete_slide_snippet(page_id.max(0) as u32, snippet_id.max(0) as u32)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeAutoSlideSnippets(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return -1;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .auto_generate_slide_snippets(page_id as u32)
            .map(|v| v.len() as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeResetConnectorRouting(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .reset_connector_routing(page_id as u32, shape_id as u64)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeEnsureControlHandle(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .ensure_control_handle(page_id as u32, shape_id as u64)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

// ============================================================================
// v0.6.0-beta JNI: VBA Macro IDE, Sub-Shape Vertices, Review Comments & Ink,
//                  Hyperlinks/ScreenTips, Enterprise Domain Wizards & CAD Tools
// ============================================================================

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeRunVbaMacro<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    selection_ids: jni::objects::JLongArray<'local>,
    script: JString<'local>,
) -> jni::sys::jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || script.is_null() {
            return std::ptr::null_mut();
        }
        let Ok(script_s) = env.get_string(&script).map(String::from) else {
            return std::ptr::null_mut();
        };
        let mut sel_u64 = Vec::new();
        if !selection_ids.is_null() {
            if let Ok(len) = env.get_array_length(&selection_ids) {
                let mut buf = vec![0i64; len as usize];
                if env.get_long_array_region(&selection_ids, 0, &mut buf).is_ok() {
                    sel_u64 = buf.into_iter().filter(|&x| x > 0).map(|x| x as u64).collect();
                }
            }
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let out_json = match doc.session.run_vba_macro_with_selection(&sel_u64, &script_s) {
            Ok(json) => json,
            Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
        };
        env.new_string(out_json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeListCommentsJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
) -> jni::sys::jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return std::ptr::null_mut();
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
        let json = doc.session.list_comments_json(page_id.max(0) as u32);
        env.new_string(json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeAddComment<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    author: JString<'local>,
    text: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || text.is_null() {
            return -1;
        }
        let author_s = if author.is_null() {
            String::new()
        } else {
            env.get_string(&author).map(String::from).unwrap_or_default()
        };
        let Ok(text_s) = env.get_string(&text).map(String::from) else {
            return -1;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .add_comment(
                page_id as u32,
                shape_id.max(0) as u64,
                &author_s,
                &text_s,
                0.0,
                0.0,
            )
            .map(|id| id as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeResolveComment(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    comment_id: jint,
    resolved: jboolean,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || comment_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_comment_resolved(comment_id as u32, resolved == JNI_TRUE)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeDeleteComment(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    comment_id: jint,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || comment_id <= 0 {
            return JNI_FALSE;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .delete_comment(comment_id as u32)
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeClearPageInk(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
    page_id: jint,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return -1;
        }
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .clear_page_ink(page_id as u32)
            .map(|c| c as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeSetHyperlinkAndScreentip<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_id: jlong,
    description: JString<'local>,
    address: JString<'local>,
    sub_address: JString<'local>,
    screentip: JString<'local>,
) -> jboolean {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let read_s = |env: &mut JNIEnv<'local>, js: &JString<'local>| -> String {
            if js.is_null() {
                String::new()
            } else {
                env.get_string(js).map(String::from).unwrap_or_default()
            }
        };
        let desc_s = read_s(&mut env, &description);
        let addr_s = read_s(&mut env, &address);
        let sub_s = read_s(&mut env, &sub_address);
        let tip_s = read_s(&mut env, &screentip);
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        if doc
            .session
            .set_shape_hyperlink_and_screentip(
                page_id as u32,
                shape_id as u64,
                &desc_s,
                &addr_s,
                &sub_s,
                &tip_s,
            )
            .unwrap_or(false)
        {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
    }))
    .unwrap_or(JNI_FALSE)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeRunDomainWizard<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    wizard_slug: JString<'local>,
    spec: JString<'local>,
) -> jni::sys::jstring {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || wizard_slug.is_null() {
            return std::ptr::null_mut();
        }
        let Ok(slug_s) = env.get_string(&wizard_slug).map(String::from) else {
            return std::ptr::null_mut();
        };
        let spec_s = if spec.is_null() {
            String::new()
        } else {
            env.get_string(&spec).map(String::from).unwrap_or_default()
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let out_json = match doc.session.run_domain_wizard(page_id as u32, &slug_s, &spec_s) {
            Ok(json) => json,
            Err(e) => serde_json::json!({ "error": e.to_string() }).to_string(),
        };
        env.new_string(out_json)
            .map(|s| s.into_raw())
            .unwrap_or(std::ptr::null_mut())
    }))
    .unwrap_or(std::ptr::null_mut())
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeApplyCadScale<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    scale_slug: JString<'local>,
    locked: jboolean,
) -> jdouble {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 || scale_slug.is_null() {
            return -1.0;
        }
        let Ok(slug_s) = env.get_string(&scale_slug).map(String::from) else {
            return -1.0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .apply_cad_scale(page_id as u32, &slug_s, locked == JNI_TRUE)
            .unwrap_or(-1.0)
    }))
    .unwrap_or(-1.0)
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeConvertCadLayers<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    layer_name_or_all: JString<'local>,
) -> jint {
    catch_unwind(AssertUnwindSafe(|| {
        if doc_ptr == 0 || page_id <= 0 {
            return -1;
        }
        let layer_s = if layer_name_or_all.is_null() {
            "*".to_string()
        } else {
            env.get_string(&layer_name_or_all)
                .map(String::from)
                .unwrap_or_else(|_| "*".into())
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        doc.session
            .convert_cad_layers(page_id as u32, &layer_s)
            .map(|c| c as jint)
            .unwrap_or(-1)
    }))
    .unwrap_or(-1)
}
