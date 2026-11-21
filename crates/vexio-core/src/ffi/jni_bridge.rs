//! Zero-Copy Android & Googlebook OS JNI Bridge (`com.vexio.core.NativeBridge`).
//!
//! Uses direct `ByteBuffer` memory views (`get_direct_buffer_address`) for zero-copy
//! `.vsdx`/`.vsd`/`.dwg` loading, USI Active Stylus motion batches, and RGBA previews.

#![cfg(not(target_arch = "wasm32"))]

use crate::ffi::c_abi::{VexioDoc, VexioEngine, VexioViewport};
use crate::workspace::DocumentSession;
use jni::JNIEnv;
use jni::objects::{JByteArray, JByteBuffer, JClass, JObject, JString};
use jni::sys::{jboolean, jfloat, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeInitEngine(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    Box::into_raw(Box::new(VexioEngine { initialized: true })) as jlong
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeDestroyEngine(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    engine_ptr: jlong,
) {
    if engine_ptr != 0 {
        // SAFETY: `engine_ptr` was allocated via `Box::into_raw` in `nativeInitEngine`.
        drop(unsafe { Box::from_raw(engine_ptr as *mut VexioEngine) });
    }
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeCreateBlankDocument<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    title: JString<'local>,
) -> jlong {
    let t: String = env
        .get_string(&title)
        .map(|s| s.into())
        .unwrap_or_else(|_| "Untitled".into());
    Box::into_raw(Box::new(VexioDoc {
        session: DocumentSession::new_blank(&t),
    })) as jlong
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeOpenDocument<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine_ptr: jlong,
    direct_bytes: JByteBuffer<'local>,
    ext: JString<'local>,
) -> jlong {
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
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeSaveDocument<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    ext: JString<'local>,
) -> JByteArray<'local> {
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
}

#[no_mangle]
pub extern "system" fn Java_com_vexio_core_NativeBridge_nativeAttachVulkanSurface<'local>(
    _env: JNIEnv<'local>,
    _class: JClass<'local>,
    engine_ptr: jlong,
    doc_ptr: jlong,
    _surface: JObject<'local>,
    width: jint,
    height: jint,
    density: jfloat,
) -> jlong {
    if engine_ptr == 0 || doc_ptr == 0 {
        return 0;
    }
    let vp = VexioViewport {
        page_id: 1,
        camera: vexio_render::ViewportCamera::new(
            width.max(1) as u32,
            height.max(1) as u32,
            density.max(0.5) as f64,
        ),
    };
    Box::into_raw(Box::new(vp)) as jlong
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
    if viewport_ptr != 0 {
        // SAFETY: `viewport_ptr` created in `nativeAttachVulkanSurface`.
        let vp = unsafe { &mut *(viewport_ptr as *mut VexioViewport) };
        vp.camera.width_px = width.max(1) as u32;
        vp.camera.height_px = height.max(1) as u32;
        vp.camera.dpi_scale = density.max(0.5) as f64;
    }
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
    if viewport_ptr == 0 || doc_ptr == 0 || count <= 0 {
        return 1000; // PointerIcon.TYPE_ARROW
    }
    let Ok(ptr) = env.get_direct_buffer_address(&direct_batch) else {
        return 1000;
    };
    let Ok(cap) = env.get_direct_buffer_capacity(&direct_batch) else {
        return 1000;
    };
    // Each packed stylus/motion sample is 32 bytes (`MotionBatchFormat`):
    // [x: f32, y: f32, pressure: f32, tiltRad: f32, orientationRad: f32, packedFlags: u32, timestampMicros: i64]
    if cap < 32 {
        return 1000;
    }
    // SAFETY: `ptr` has at least 32 bytes in the direct ByteBuffer.
    let buf = unsafe { std::slice::from_raw_parts(ptr, cap) };
    let last_off = ((count as usize - 1) * 32).min(cap - 32);
    let x_px = f32::from_le_bytes(buf[last_off..last_off + 4].try_into().unwrap()) as f64;
    let y_px = f32::from_le_bytes(buf[last_off + 4..last_off + 8].try_into().unwrap()) as f64;

    // SAFETY: `viewport_ptr` and `doc_ptr` verified non-zero handles.
    let vp = unsafe { &*(viewport_ptr as *const VexioViewport) };
    let doc = unsafe { &*(doc_ptr as *const VexioDoc) };
    let page_pt = vp.camera.screen_to_page_in(x_px, y_px);
    let visuals = vexio_shapesheet::ReactiveEngine::evaluate_page_visuals(&doc.session.doc, vp.page_id);
    let index = vexio_render::SpatialIndex::build(&visuals);
    if !index
        .hit_test_point(page_pt[0], page_pt[1], 0.1)
        .is_empty()
    {
        1013 // Android PointerIcon.TYPE_ALL_SCROLL (Move)
    } else {
        1000 // Android PointerIcon.TYPE_ARROW
    }
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
    if doc_ptr == 0 {
        return 1;
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
        Ok(()) => 0,
        Err(_) => 2,
    }
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeUndo(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
) -> jboolean {
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
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeRedo(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    doc_ptr: jlong,
) -> jboolean {
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
    let Ok(rgba) =
        vexio_render::render_page_rgba_auto(&doc.session.doc, page_id as u32, width as u32, height as u32)
    else {
        return 5;
    };
    // SAFETY: `dst_ptr` has `cap >= needed == rgba.len()`.
    unsafe {
        std::ptr::copy_nonoverlapping(rgba.as_ptr(), dst_ptr, needed);
    }
    0
}

#[no_mangle]
pub unsafe extern "system" fn Java_com_vexio_core_NativeBridge_nativeBooleanOp<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    doc_ptr: jlong,
    page_id: jint,
    shape_ids: jni::objects::JLongArray<'local>,
    op: JString<'local>,
    offset_in: jni::sys::jdouble,
) -> jint {
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
        let created = crate::boolean_ops::apply_boolean_operation(
            &mut doc.session.doc,
            page_id as u32,
            &ids,
            kind,
        );
        created.len() as jint
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(slug) = env.get_string(&theme_slug).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        crate::themes_styles::apply_document_theme_variant(
            &mut doc.session.doc,
            &slug,
            variant_index.clamp(0, 3) as u8,
        ) as jint
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 {
            return JNI_FALSE;
        }
        let Ok(preset_str) = env.get_string(&preset).map(String::from) else {
            return JNI_FALSE;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        let spec = crate::ffi::c_abi::visual_preset_spec_from_slug(&preset_str, shape_id as u64);
        if crate::themes_styles::apply_visual_effects_to_shape(
            &mut doc.session.doc,
            page_id as u32,
            shape_id as u64,
            &spec,
        ) {
            JNI_TRUE
        } else {
            JNI_FALSE
        }
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if doc_ptr == 0 || shape_id <= 0 {
            return 0;
        }
        let name: String = env.get_string(&sub_name).map(Into::into).unwrap_or_default();
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        crate::enterprise_plan2::create_or_link_subprocess_page(
            &mut doc.session.doc,
            page_id as u32,
            shape_id as u64,
            &name,
        )
        .unwrap_or(0) as jint
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(bytes) = env.convert_byte_array(&csv_or_xlsx) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        crate::plan2_connectors::import_data_visualizer_csv_or_xlsx(
            &mut doc.session.doc,
            page_id as u32,
            &bytes,
        )
        .unwrap_or(0) as jint
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(json_str) = env.get_string(&discovery_json).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        crate::plan2_connectors::import_cloud_discovery_json(
            &mut doc.session.doc,
            page_id as u32,
            &json_str,
        )
        .unwrap_or(0) as jint
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
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if doc_ptr == 0 {
            return 0;
        }
        let Ok(txt) = env.get_string(&text).map(String::from) else {
            return 0;
        };
        // SAFETY: `doc_ptr` verified non-zero handle.
        let doc = unsafe { &mut *(doc_ptr as *mut VexioDoc) };
        crate::plan2_connectors::import_entra_or_ldif_org_chart(
            &mut doc.session.doc,
            page_id as u32,
            &txt,
        )
        .unwrap_or(0) as jint
    }));
    res.unwrap_or(0)
}

