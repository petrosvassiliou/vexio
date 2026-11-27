/* @ts-self-types="./vexio_core.d.ts" */

export class WasmVexioSession {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        WasmVexioSessionFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_wasmvexiosession_free(ptr, 0);
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} author
     * @param {string} text
     * @returns {number}
     */
    add_comment(page_id, shape_id, author, text) {
        const val3 = +page_id;
        const val4 = +shape_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(author, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_add_comment(this.__wbg_ptr, val3, val4, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} page_x_in
     * @param {number} page_y_in
     * @param {number} point_type
     * @returns {number}
     */
    add_connection_point(page_id, shape_id, page_x_in, page_y_in, point_type) {
        const ret = wasm.wasmvexiosession_add_connection_point(this.__wbg_ptr, page_id, shape_id, page_x_in, page_y_in, point_type);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} color_hex
     * @param {number} width_pt
     * @param {boolean} highlighter
     * @param {string} points_json
     * @returns {number}
     */
    add_ink_stroke(page_id, color_hex, width_pt, highlighter, points_json) {
        const val3 = +page_id;
        const val4 = +width_pt;
        const val5 = +highlighter;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(color_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(points_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_add_ink_stroke(this.__wbg_ptr, val3, ptr0, len0, val4, val5, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} name
     * @returns {number}
     */
    add_layer(page_id, name) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_add_layer(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} anchor_shape_id
     * @param {string} topic_text
     * @param {boolean} as_sibling
     * @returns {number}
     */
    add_mind_map_topic(page_id, anchor_shape_id, topic_text, as_sibling) {
        const val2 = +page_id;
        const val3 = +anchor_shape_id;
        const val4 = +as_sibling;
        const ptr0 = passStringToWasm0(topic_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_add_mind_map_topic(this.__wbg_ptr, val2, val3, ptr0, len0, val4);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {string} name
     * @returns {number}
     */
    add_page(name) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_add_page(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {boolean} is_vertical
     * @param {number} coord_in
     * @returns {number}
     */
    add_ruler_guide(page_id, is_vertical, coord_in) {
        const ret = wasm.wasmvexiosession_add_ruler_guide(this.__wbg_ptr, page_id, is_vertical, coord_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} title
     * @param {string} notes
     * @param {number} x_in
     * @param {number} y_in
     * @param {number} width_in
     * @param {number} height_in
     * @returns {number}
     */
    add_slide_snippet(page_id, title, notes, x_in, y_in, width_in, height_in) {
        const val3 = +page_id;
        const val4 = +x_in;
        const val5 = +y_in;
        const val6 = +width_in;
        const val7 = +height_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(notes, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_add_slide_snippet(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4, val5, val6, val7);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} stencil_title
     * @param {string} master_name
     * @returns {string}
     */
    add_to_custom_stencil(page_id, shape_ids, stencil_title, master_name) {
        let deferred6_0;
        let deferred6_1;
        try {
            const val4 = +page_id;
            let ptr0, len0, ptr1, len1, ptr2, len2;
            try {
                ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(stencil_title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
                ptr2 = passStringToWasm0(master_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len2 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
                throw e;
            }
            const ret = wasm.wasmvexiosession_add_to_custom_stencil(this.__wbg_ptr, val4, ptr0, len0, ptr1, len1, ptr2, len2);
            var ptr5 = ret[0];
            var len5 = ret[1];
            if (ret[3]) {
                ptr5 = 0; len5 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred6_0 = ptr5;
            deferred6_1 = len5;
            return getStringFromWasm0(ptr5, len5);
        } finally {
            wasm.__wbindgen_free(deferred6_0, deferred6_1, 1);
        }
    }
    /**
     * Multi-shape Align (`left`, `center`, `right`, `top`, `middle`, `bottom`) and
     * Distribute (`distribute_h`, `distribute_v`) matching Visio Home -> Position!
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} mode
     * @returns {number}
     */
    align_shapes(page_id, shape_ids, mode) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_align_shapes(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} algo
     */
    apply_auto_layout(page_id, algo) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(algo, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmvexiosession_apply_auto_layout(this.__wbg_ptr, val2, ptr0, len0);
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} op
     * @param {number} offset_in
     * @returns {Uint32Array}
     */
    apply_boolean_op(page_id, shape_ids, op, offset_in) {
        const val3 = +page_id;
        const val4 = +offset_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(op, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_apply_boolean_op(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4);
        var v5 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v5;
    }
    /**
     * @param {number} page_id
     * @param {string} style_slug
     * @param {string} title
     * @param {string} subtitle
     * @param {string} author
     * @param {string} date_or_rev
     * @returns {number}
     */
    apply_border_and_title(page_id, style_slug, title, subtitle, author, date_or_rev) {
        const val6 = +page_id;
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4;
        try {
            ptr0 = passStringToWasm0(style_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(subtitle, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passStringToWasm0(author, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len3 = WASM_VECTOR_LEN;
            ptr4 = passStringToWasm0(date_or_rev, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len4 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr4) { wasm.__wbindgen_free(ptr4, len4 * 1, 1); }
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_apply_border_and_title(this.__wbg_ptr, val6, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} scale_slug
     * @param {boolean} locked
     * @returns {number}
     */
    apply_cad_scale(page_id, scale_slug, locked) {
        const val2 = +page_id;
        const val3 = +locked;
        const ptr0 = passStringToWasm0(scale_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_apply_cad_scale(this.__wbg_ptr, val2, ptr0, len0, val3);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0];
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} target_ids
     * @param {string} format_json
     * @returns {number}
     */
    apply_format_painter(page_id, target_ids, format_json) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(target_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(format_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_apply_format_painter(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} preset_slug
     * @returns {number}
     */
    apply_heading_preset(page_id, shape_ids, preset_slug) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(preset_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_apply_heading_preset(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {string} ops_json
     * @returns {number}
     */
    apply_remote_crdt_ops_json(ops_json) {
        const ptr0 = passStringToWasm0(ops_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_apply_remote_crdt_ops_json(this.__wbg_ptr, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} preset
     * @returns {boolean}
     */
    apply_shape_visual_preset(page_id, shape_id, preset) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(preset, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_apply_shape_visual_preset(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} style_json
     * @returns {number}
     */
    apply_text_style_json(page_id, shape_ids, style_json) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(style_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_apply_text_style_json(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {string} theme_slug
     * @param {number} variant_index
     * @returns {number}
     */
    apply_theme_variant(theme_slug, variant_index) {
        const val2 = +variant_index;
        const ptr0 = passStringToWasm0(theme_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_apply_theme_variant(this.__wbg_ptr, ptr0, len0, val2);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} project_code
     * @param {string} author
     * @param {string} revision
     * @returns {number}
     */
    apply_title_block_border(page_id, project_code, author, revision) {
        const val4 = +page_id;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(project_code, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(author, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(revision, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_apply_title_block_border(this.__wbg_ptr, val4, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret >>> 0;
    }
    /**
     * @param {OffscreenCanvas} canvas
     * @param {number} width
     * @param {number} height
     * @param {number} dpi
     * @returns {Promise<boolean>}
     */
    attach_offscreen_canvas(canvas, width, height, dpi) {
        const ret = wasm.wasmvexiosession_attach_offscreen_canvas(this.__wbg_ptr, canvas, width, height, dpi);
        return ret;
    }
    /**
     * @param {number} page_id
     * @param {number} source_shape_id
     * @param {string} dir
     * @returns {number}
     */
    auto_connect(page_id, source_shape_id, dir) {
        const val2 = +page_id;
        const val3 = +source_shape_id;
        const ptr0 = passStringToWasm0(dir, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_auto_connect(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    auto_generate_slide_snippets(page_id) {
        const ret = wasm.wasmvexiosession_auto_generate_slide_snippets(this.__wbg_ptr, page_id);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} recordset_id
     * @param {number} row_index
     * @param {Uint32Array | number[]} shape_ids
     * @returns {number}
     */
    bind_external_row(page_id, recordset_id, row_index, shape_ids) {
        const val2 = +page_id;
        const val3 = +recordset_id;
        const val4 = +row_index;
        const ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_bind_external_row(this.__wbg_ptr, val2, val3, val4, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    clear_page_ink(page_id) {
        const ret = wasm.wasmvexiosession_clear_page_ink(this.__wbg_ptr, page_id);
        return ret >>> 0;
    }
    /**
     * @param {Uint8Array | number[]} baseline_vsdx
     * @returns {string}
     */
    compare_with_vsdx_bytes(baseline_vsdx) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passArray8ToWasm0(baseline_vsdx, wasm.__wbindgen_malloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmvexiosession_compare_with_vsdx_bytes(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * Compute Dynamic Grid Smart Alignment & Equal-Spacing Guides (center-X, center-Y, left/right/top/bottom edges,
     * page center, and equal gaps across 3+ shapes) within `0.10 in` magnetic threshold.
     * @param {number} page_id
     * @param {number} moving_shape_id
     * @param {number} cand_x_in
     * @param {number} cand_y_in
     * @param {number} width_in
     * @param {number} height_in
     * @returns {string}
     */
    compute_smart_guides(page_id, moving_shape_id, cand_x_in, cand_y_in, width_in, height_in) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_compute_smart_guides(this.__wbg_ptr, page_id, moving_shape_id, cand_x_in, cand_y_in, width_in, height_in);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @param {number} from_shape_id
     * @param {number} to_shape_id
     * @returns {number}
     */
    connect_points_or_shapes(page_id, x1_in, y1_in, x2_in, y2_in, from_shape_id, to_shape_id) {
        const ret = wasm.wasmvexiosession_connect_points_or_shapes(this.__wbg_ptr, page_id, x1_in, y1_in, x2_in, y2_in, from_shape_id, to_shape_id);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} from_id
     * @param {number} to_id
     * @returns {number}
     */
    connect_shapes(page_id, from_id, to_id) {
        const ret = wasm.wasmvexiosession_connect_shapes(this.__wbg_ptr, page_id, from_id, to_id);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} layer_name_or_all
     * @returns {number}
     */
    convert_cad_layers(page_id, layer_name_or_all) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(layer_name_or_all, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_convert_cad_layers(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {string}
     */
    copy_shape_format_json(page_id, shape_id) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmvexiosession_copy_shape_format_json(this.__wbg_ptr, page_id, shape_id);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} sub_name
     * @returns {number}
     */
    create_subprocess_drilldown(page_id, shape_id, sub_name) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(sub_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_create_subprocess_drilldown(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} comment_id
     * @returns {boolean}
     */
    delete_comment(comment_id) {
        const ret = wasm.wasmvexiosession_delete_comment(this.__wbg_ptr, comment_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} ix
     * @returns {boolean}
     */
    delete_connection_point(page_id, shape_id, ix) {
        const ret = wasm.wasmvexiosession_delete_connection_point(this.__wbg_ptr, page_id, shape_id, ix);
        return ret !== 0;
    }
    /**
     * @param {string} stencil_title
     * @param {string} master_name
     * @returns {boolean}
     */
    delete_custom_master(stencil_title, master_name) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(stencil_title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(master_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_delete_custom_master(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * @param {number} recordset_id
     * @returns {boolean}
     */
    delete_external_recordset(recordset_id) {
        const ret = wasm.wasmvexiosession_delete_external_recordset(this.__wbg_ptr, recordset_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @returns {boolean}
     */
    delete_page(page_id) {
        const ret = wasm.wasmvexiosession_delete_page(this.__wbg_ptr, page_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} guide_id
     * @returns {boolean}
     */
    delete_ruler_guide(page_id, guide_id) {
        const ret = wasm.wasmvexiosession_delete_ruler_guide(this.__wbg_ptr, page_id, guide_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} row_name
     * @returns {boolean}
     */
    delete_shape_action(page_id, shape_id, row_name) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(row_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_delete_shape_action(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {boolean}
     */
    delete_shape(page_id, shape_id) {
        const ret = wasm.wasmvexiosession_delete_shape(this.__wbg_ptr, page_id, shape_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} row_name
     * @returns {boolean}
     */
    delete_shape_data(page_id, shape_id, row_name) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(row_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_delete_shape_data(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} geom_index
     * @param {number} row_ix
     * @returns {boolean}
     */
    delete_shape_vertex(page_id, shape_id, geom_index, row_ix) {
        const ret = wasm.wasmvexiosession_delete_shape_vertex(this.__wbg_ptr, page_id, shape_id, geom_index, row_ix);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} snippet_id
     * @returns {boolean}
     */
    delete_slide_snippet(page_id, snippet_id) {
        const ret = wasm.wasmvexiosession_delete_slide_snippet(this.__wbg_ptr, page_id, snippet_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} axis
     * @returns {number}
     */
    distribute_shapes(page_id, shape_ids, axis) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(axis, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_distribute_shapes(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} seg_index
     * @param {number} coord_in
     * @returns {boolean}
     */
    drag_connector_segment(page_id, shape_id, seg_index, coord_in) {
        const ret = wasm.wasmvexiosession_drag_connector_segment(this.__wbg_ptr, page_id, shape_id, seg_index, coord_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @param {number} bow_in
     * @returns {number}
     */
    draw_arc(page_id, x1_in, y1_in, x2_in, y2_in, bow_in) {
        const ret = wasm.wasmvexiosession_draw_arc(this.__wbg_ptr, page_id, x1_in, y1_in, x2_in, y2_in, bow_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @param {number} thickness_in
     * @returns {number}
     */
    draw_architectural_wall(page_id, x1_in, y1_in, x2_in, y2_in, thickness_in) {
        const ret = wasm.wasmvexiosession_draw_architectural_wall(this.__wbg_ptr, page_id, x1_in, y1_in, x2_in, y2_in, thickness_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} points_json
     * @param {boolean} closed
     * @returns {number}
     */
    draw_freeform_curve(page_id, points_json, closed) {
        const val2 = +page_id;
        const val3 = +closed;
        const ptr0 = passStringToWasm0(points_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_draw_freeform_curve(this.__wbg_ptr, val2, ptr0, len0, val3);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @returns {number}
     */
    draw_line(page_id, x1_in, y1_in, x2_in, y2_in) {
        const ret = wasm.wasmvexiosession_draw_line(this.__wbg_ptr, page_id, x1_in, y1_in, x2_in, y2_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} kind
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @returns {number}
     */
    draw_primitive_2d(page_id, kind, x1_in, y1_in, x2_in, y2_in) {
        const val2 = +page_id;
        const val3 = +x1_in;
        const val4 = +y1_in;
        const val5 = +x2_in;
        const val6 = +y2_in;
        const ptr0 = passStringToWasm0(kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_draw_primitive_2d(this.__wbg_ptr, val2, ptr0, len0, val3, val4, val5, val6);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @returns {number}
     */
    draw_rectangle(page_id, x1_in, y1_in, x2_in, y2_in) {
        const ret = wasm.wasmvexiosession_draw_rectangle(this.__wbg_ptr, page_id, x1_in, y1_in, x2_in, y2_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {string} category
     * @param {string} master_name
     * @param {number} x_in
     * @param {number} y_in
     * @returns {number}
     */
    drop_builtin_shape(category, master_name, x_in, y_in) {
        const val3 = +x_in;
        const val4 = +y_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(category, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(master_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_drop_builtin_shape(this.__wbg_ptr, ptr0, len0, ptr1, len1, val3, val4);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} category
     * @param {string} master_name
     * @param {number} x_in
     * @param {number} y_in
     * @returns {number}
     */
    drop_builtin_shape_on_page(page_id, category, master_name, x_in, y_in) {
        const val3 = +page_id;
        const val4 = +x_in;
        const val5 = +y_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(category, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(master_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_drop_builtin_shape_on_page(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4, val5);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    duplicate_page(page_id) {
        const ret = wasm.wasmvexiosession_duplicate_page(this.__wbg_ptr, page_id);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {number} dx_in
     * @param {number} dy_in
     * @returns {Uint32Array}
     */
    duplicate_shapes(page_id, shape_ids, dx_in, dy_in) {
        const val2 = +page_id;
        const val3 = +dx_in;
        const val4 = +dy_in;
        const ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_duplicate_shapes(this.__wbg_ptr, val2, ptr0, len0, val3, val4);
        var v5 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v5;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {boolean}
     */
    ensure_control_handle(page_id, shape_id) {
        const ret = wasm.wasmvexiosession_ensure_control_handle(this.__wbg_ptr, page_id, shape_id);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} row_name
     * @returns {string}
     */
    execute_shape_action(page_id, shape_id, row_name) {
        let deferred5_0;
        let deferred5_1;
        try {
            const val2 = +page_id;
            const val3 = +shape_id;
            const ptr0 = passStringToWasm0(row_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmvexiosession_execute_shape_action(this.__wbg_ptr, val2, val3, ptr0, len0);
            var ptr4 = ret[0];
            var len4 = ret[1];
            if (ret[3]) {
                ptr4 = 0; len4 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred5_0 = ptr4;
            deferred5_1 = len4;
            return getStringFromWasm0(ptr4, len4);
        } finally {
            wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
        }
    }
    /**
     * @param {string} format_ext
     * @returns {Uint8Array}
     */
    export_bom_report_bytes(format_ext) {
        const ptr0 = passStringToWasm0(format_ext, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_export_bom_report_bytes(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * @returns {Uint8Array}
     */
    export_bom_xlsx() {
        const ret = wasm.wasmvexiosession_export_bom_xlsx(this.__wbg_ptr);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @param {string} stencil_title
     * @returns {Uint8Array}
     */
    export_custom_stencil_vssx(stencil_title) {
        const ptr0 = passStringToWasm0(stencil_title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_export_custom_stencil_vssx(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * @param {number} page_id
     * @returns {Uint8Array}
     */
    export_data_visualizer_workbook(page_id) {
        const ret = wasm.wasmvexiosession_export_data_visualizer_workbook(this.__wbg_ptr, page_id);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @param {number} page_id
     * @param {string} format_slug
     * @returns {string}
     */
    export_diagram_markup(page_id, format_slug) {
        let deferred4_0;
        let deferred4_1;
        try {
            const val2 = +page_id;
            const ptr0 = passStringToWasm0(format_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmvexiosession_export_diagram_markup(this.__wbg_ptr, val2, ptr0, len0);
            var ptr3 = ret[0];
            var len3 = ret[1];
            if (ret[3]) {
                ptr3 = 0; len3 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred4_0 = ptr3;
            deferred4_1 = len3;
            return getStringFromWasm0(ptr3, len3);
        } finally {
            wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @returns {string}
     */
    export_mind_map_outline(page_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_export_mind_map_outline(this.__wbg_ptr, page_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {Uint8Array}
     */
    export_wal() {
        const ret = wasm.wasmvexiosession_export_wal(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * @param {number} active_page_id
     * @param {string} query
     * @param {boolean} match_case
     * @param {boolean} whole_word
     * @param {boolean} all_pages
     * @returns {string}
     */
    find_in_document_json(active_page_id, query, match_case, whole_word, all_pages) {
        let deferred6_0;
        let deferred6_1;
        try {
            const val2 = +active_page_id;
            const val3 = +match_case;
            const val4 = +whole_word;
            const val5 = +all_pages;
            const ptr0 = passStringToWasm0(query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmvexiosession_find_in_document_json(this.__wbg_ptr, val2, ptr0, len0, val3, val4, val5);
            deferred6_0 = ret[0];
            deferred6_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred6_0, deferred6_1, 1);
        }
    }
    /**
     * @param {number} now_ms
     * @returns {string}
     */
    flush_crdt_ops_json(now_ms) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_flush_crdt_ops_json(this.__wbg_ptr, now_ms);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @returns {string}
     */
    forward_engineer_sql(page_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_forward_engineer_sql(this.__wbg_ptr, page_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    generate_gantt_demo(page_id) {
        const ret = wasm.wasmvexiosession_generate_gantt_demo(this.__wbg_ptr, page_id);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    generate_pert_demo(page_id) {
        const ret = wasm.wasmvexiosession_generate_pert_demo(this.__wbg_ptr, page_id);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    generate_timeline_demo(page_id) {
        const ret = wasm.wasmvexiosession_generate_timeline_demo(this.__wbg_ptr, page_id);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {string}
     */
    get_page_data_link_state_json(page_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_get_page_data_link_state_json(this.__wbg_ptr, page_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @returns {string}
     */
    get_snap_glue_json(page_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_get_snap_glue_json(this.__wbg_ptr, page_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} active_page_id
     * @returns {string}
     */
    get_state_json(active_page_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_get_state_json(this.__wbg_ptr, active_page_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} zoom_ppi
     * @returns {string}
     */
    get_text_block_overlay_json(page_id, shape_id, zoom_ppi) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_get_text_block_overlay_json(this.__wbg_ptr, page_id, shape_id, zoom_ppi);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @returns {number}
     */
    group_shapes(page_id, shape_ids) {
        const val2 = +page_id;
        const ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_group_shapes(this.__wbg_ptr, val2, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} tolerance_in
     * @returns {number}
     */
    heal_wall_intersections(page_id, tolerance_in) {
        const ret = wasm.wasmvexiosession_heal_wall_intersections(this.__wbg_ptr, page_id, tolerance_in);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} x_in
     * @param {number} y_in
     * @returns {number}
     */
    hit_test_page(page_id, x_in, y_in) {
        const ret = wasm.wasmvexiosession_hit_test_page(this.__wbg_ptr, page_id, x_in, y_in);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} discovery_json
     * @returns {number}
     */
    import_cloud_discovery(page_id, discovery_json) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(discovery_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_import_cloud_discovery(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint8Array | number[]} bytes
     * @returns {number}
     */
    import_data_visualizer(page_id, bytes) {
        const val2 = +page_id;
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_import_data_visualizer(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} markup
     * @param {string} format_slug
     * @returns {string}
     */
    import_diagram_markup(page_id, markup, format_slug) {
        let deferred5_0;
        let deferred5_1;
        try {
            const val3 = +page_id;
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(markup, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(format_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.wasmvexiosession_import_diagram_markup(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
            var ptr4 = ret[0];
            var len4 = ret[1];
            if (ret[3]) {
                ptr4 = 0; len4 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred5_0 = ptr4;
            deferred5_1 = len4;
            return getStringFromWasm0(ptr4, len4);
        } finally {
            wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {string} text
     * @returns {number}
     */
    import_entra_or_ldif(page_id, text) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_import_entra_or_ldif(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} file_name
     * @param {Uint8Array | number[]} bytes
     * @param {string} key_col_hint
     * @param {string} table_hint
     * @param {boolean} auto_bind
     * @param {boolean} auto_enable_graphics
     * @returns {string}
     */
    import_external_recordset_bytes(page_id, file_name, bytes, key_col_hint, table_hint, auto_bind, auto_enable_graphics) {
        let deferred9_0;
        let deferred9_1;
        try {
            const val5 = +page_id;
            const val6 = +auto_bind;
            const val7 = +auto_enable_graphics;
            let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3;
            try {
                ptr0 = passStringToWasm0(file_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
                len1 = WASM_VECTOR_LEN;
                ptr2 = passStringToWasm0(key_col_hint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len2 = WASM_VECTOR_LEN;
                ptr3 = passStringToWasm0(table_hint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len3 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
                if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.wasmvexiosession_import_external_recordset_bytes(this.__wbg_ptr, val5, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, val6, val7);
            var ptr8 = ret[0];
            var len8 = ret[1];
            if (ret[3]) {
                ptr8 = 0; len8 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred9_0 = ptr8;
            deferred9_1 = len8;
            return getStringFromWasm0(ptr8, len8);
        } finally {
            wasm.__wbindgen_free(deferred9_0, deferred9_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {string} csv_text
     * @returns {number}
     */
    import_org_chart_csv(page_id, csv_text) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(csv_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_import_org_chart_csv(this.__wbg_ptr, val2, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint8Array | number[]} sqlite_bytes
     * @param {string} table_name
     * @param {string} key_col
     * @returns {number}
     */
    import_sqlite_recordset(page_id, sqlite_bytes, table_name, key_col) {
        const val4 = +page_id;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passArray8ToWasm0(sqlite_bytes, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(table_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(key_col, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_import_sqlite_recordset(this.__wbg_ptr, val4, ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Import an external `.vssx`, `.vssm`, `.vss`, `.vsdx`, `.vdx`, `.svg`, or `.zip` Visio stencil archive
     * directly into the Stencil Navigator with lossless geometry, `ForeignData`, and Retina PNG/SVG thumbnails.
     * @param {string} file_name
     * @param {Uint8Array | number[]} bytes
     * @returns {string}
     */
    import_stencil_bytes(file_name, bytes) {
        let deferred4_0;
        let deferred4_1;
        try {
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(file_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.wasmvexiosession_import_stencil_bytes(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var ptr3 = ret[0];
            var len3 = ret[1];
            if (ret[3]) {
                ptr3 = 0; len3 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred4_0 = ptr3;
            deferred4_1 = len3;
            return getStringFromWasm0(ptr3, len3);
        } finally {
            wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {string} room_name
     * @returns {number}
     */
    insert_architectural_room(page_id, room_name) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(room_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_insert_architectural_room(this.__wbg_ptr, val2, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} dim_kind
     * @param {number} target_shape_id
     * @param {number} second_shape_id
     * @param {number} offset_in
     * @param {string} unit_format
     * @returns {number}
     */
    insert_associative_dimension(page_id, dim_kind, target_shape_id, second_shape_id, offset_in, unit_format) {
        const val3 = +page_id;
        const val4 = +target_shape_id;
        const val5 = +second_shape_id;
        const val6 = +offset_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(dim_kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(unit_format, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_insert_associative_dimension(this.__wbg_ptr, val3, ptr0, len0, val4, val5, val6, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} target_shape_id
     * @param {string} text
     * @returns {number}
     */
    insert_callout(page_id, target_shape_id, text) {
        const val2 = +page_id;
        const val3 = +target_shape_id;
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_insert_callout(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} title
     * @param {Uint32Array | number[]} member_ids
     * @returns {number}
     */
    insert_container(page_id, title, member_ids) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passArray32ToWasm0(member_ids, wasm.__wbindgen_malloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 4, 4); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_insert_container(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} media_type
     * @param {Uint8Array | number[]} image_bytes
     * @param {number} x_in
     * @param {number} y_in
     * @param {number} crop_in
     * @param {number} brightness
     * @param {number} contrast
     * @returns {number}
     */
    insert_cropped_image(page_id, media_type, image_bytes, x_in, y_in, crop_in, brightness, contrast) {
        const val3 = +page_id;
        const val4 = +x_in;
        const val5 = +y_in;
        const val6 = +crop_in;
        const val7 = +brightness;
        const val8 = +contrast;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(media_type, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passArray8ToWasm0(image_bytes, wasm.__wbindgen_malloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_insert_cropped_image(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4, val5, val6, val7, val8);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} prop_name
     * @param {number} pin_x_in
     * @param {number} pin_y_in
     * @returns {number}
     */
    insert_data_legend(page_id, prop_name, pin_x_in, pin_y_in) {
        const val2 = +page_id;
        const val3 = +pin_x_in;
        const val4 = +pin_y_in;
        const ptr0 = passStringToWasm0(prop_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_insert_data_legend(this.__wbg_ptr, val2, ptr0, len0, val3, val4);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} preset_slug
     * @param {string} custom_text
     * @param {number} x_in
     * @param {number} y_in
     * @returns {number}
     */
    insert_heading(page_id, preset_slug, custom_text, x_in, y_in) {
        const val3 = +page_id;
        const val4 = +x_in;
        const val5 = +y_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(preset_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(custom_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_insert_heading(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4, val5);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @returns {number}
     */
    insert_rack_with_equipment(page_id) {
        const ret = wasm.wasmvexiosession_insert_rack_with_equipment(this.__wbg_ptr, page_id);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} geom_index
     * @param {number} after_row_ix
     * @param {number} x_in
     * @param {number} y_in
     * @returns {boolean}
     */
    insert_shape_vertex(page_id, shape_id, geom_index, after_row_ix, x_in, y_in) {
        const ret = wasm.wasmvexiosession_insert_shape_vertex(this.__wbg_ptr, page_id, shape_id, geom_index, after_row_ix, x_in, y_in);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {string} lanes_semicolon
     * @returns {number}
     */
    insert_swimlanes(page_id, lanes_semicolon) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(lanes_semicolon, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_insert_swimlanes(this.__wbg_ptr, val2, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} preset_or_formula
     * @param {string} format_spec
     * @param {string} prefix_label
     * @returns {number}
     */
    insert_text_field(page_id, shape_id, preset_or_formula, format_spec, prefix_label) {
        const val4 = +page_id;
        const val5 = +shape_id;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(preset_or_formula, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(format_spec, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(prefix_label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_insert_text_field(this.__wbg_ptr, val4, val5, ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} wall_shape_id
     * @param {string} opening_kind
     * @param {number} offset_along_in
     * @param {number} opening_width_in
     * @param {string} swing_dir
     * @returns {number}
     */
    insert_wall_opening(page_id, wall_shape_id, opening_kind, offset_along_in, opening_width_in, swing_dir) {
        const val3 = +page_id;
        const val4 = +wall_shape_id;
        const val5 = +offset_along_in;
        const val6 = +opening_width_in;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(opening_kind, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(swing_dir, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_insert_wall_opening(this.__wbg_ptr, val3, val4, ptr0, len0, val5, val6, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} csv_text
     * @param {string} key_col
     * @returns {number}
     */
    link_csv_and_apply_data_graphics(page_id, csv_text, key_col) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(csv_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(key_col, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_link_csv_and_apply_data_graphics(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @returns {string}
     */
    list_arrowheads_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_arrowheads_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    list_builtin_pack_order_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_builtin_pack_order_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    list_builtin_stencil_previews_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_builtin_stencil_previews_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    list_builtin_stencils_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_builtin_stencils_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {string}
     */
    list_connection_points_json(page_id, shape_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_connection_points_json(this.__wbg_ptr, page_id, shape_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Returns the curated catalog of direct-HTTPS official & enterprise Visio stencil packages.
     * @returns {string}
     */
    list_official_stencil_catalog_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_official_stencil_catalog_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @returns {string}
     */
    list_ruler_guides_json(page_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_ruler_guides_json(this.__wbg_ptr, page_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {string}
     */
    list_shape_actions_json(page_id, shape_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_shape_actions_json(this.__wbg_ptr, page_id, shape_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {string}
     */
    list_shape_vertices_json(page_id, shape_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_shape_vertices_json(this.__wbg_ptr, page_id, shape_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    list_shapesheet_functions_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_shapesheet_functions_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @returns {string}
     */
    list_slide_snippets_json() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_slide_snippets_json(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {string}
     */
    list_text_fields_json(page_id, shape_id) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.wasmvexiosession_list_text_fields_json(this.__wbg_ptr, page_id, shape_id);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * @param {string} template_id
     */
    load_starter_template(template_id) {
        const ptr0 = passStringToWasm0(template_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmvexiosession_load_starter_template(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} ix
     * @param {number} page_x_in
     * @param {number} page_y_in
     * @returns {boolean}
     */
    move_connection_point(page_id, shape_id, ix, page_x_in, page_y_in) {
        const ret = wasm.wasmvexiosession_move_connection_point(this.__wbg_ptr, page_id, shape_id, ix, page_x_in, page_y_in);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {boolean} is_begin
     * @param {number} x_in
     * @param {number} y_in
     * @param {number} target_shape_id
     * @returns {boolean}
     */
    move_connector_endpoint(page_id, shape_id, is_begin, x_in, y_in, target_shape_id) {
        const ret = wasm.wasmvexiosession_move_connector_endpoint(this.__wbg_ptr, page_id, shape_id, is_begin, x_in, y_in, target_shape_id);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {boolean} is_begin
     * @param {number} x_in
     * @param {number} y_in
     * @param {number} target_shape_id
     * @param {string} target_port_cell
     * @returns {boolean}
     */
    move_connector_endpoint_with_port(page_id, shape_id, is_begin, x_in, y_in, target_shape_id, target_port_cell) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const val4 = +is_begin;
        const val5 = +x_in;
        const val6 = +y_in;
        const val7 = +target_shape_id;
        const ptr0 = passStringToWasm0(target_port_cell, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_move_connector_endpoint_with_port(this.__wbg_ptr, val2, val3, val4, val5, val6, val7, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} row_name
     * @param {number} x_in
     * @param {number} y_in
     * @returns {boolean}
     */
    move_control_handle(page_id, shape_id, row_name, x_in, y_in) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const val4 = +x_in;
        const val5 = +y_in;
        const ptr0 = passStringToWasm0(row_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_move_control_handle(this.__wbg_ptr, val2, val3, ptr0, len0, val4, val5);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} guide_id
     * @param {number} coord_in
     * @returns {boolean}
     */
    move_ruler_guide(page_id, guide_id, coord_in) {
        const ret = wasm.wasmvexiosession_move_ruler_guide(this.__wbg_ptr, page_id, guide_id, coord_in);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} new_pin_x_in
     * @param {number} new_pin_y_in
     * @returns {boolean}
     */
    move_shape(page_id, shape_id, new_pin_x_in, new_pin_y_in) {
        const ret = wasm.wasmvexiosession_move_shape(this.__wbg_ptr, page_id, shape_id, new_pin_x_in, new_pin_y_in);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} geom_index
     * @param {number} row_ix
     * @param {number} x_in
     * @param {number} y_in
     * @returns {boolean}
     */
    move_shape_vertex(page_id, shape_id, geom_index, row_ix, x_in, y_in) {
        const ret = wasm.wasmvexiosession_move_shape_vertex(this.__wbg_ptr, page_id, shape_id, geom_index, row_ix, x_in, y_in);
        return ret !== 0;
    }
    /**
     * Batch-move multiple lasso-selected shapes atomically in a single Undo/Redo transaction.
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {number} dx_in
     * @param {number} dy_in
     * @returns {number}
     */
    move_shapes_delta(page_id, shape_ids, dx_in, dy_in) {
        const val2 = +page_id;
        const val3 = +dx_in;
        const val4 = +dy_in;
        const ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_move_shapes_delta(this.__wbg_ptr, val2, ptr0, len0, val3, val4);
        return ret >>> 0;
    }
    /**
     * @param {string} title
     */
    constructor(title) {
        const ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_new(ptr0, len0);
        this.__wbg_ptr = ret;
        WasmVexioSessionFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} order_mode
     * @param {number} start_num
     * @param {number} interval
     * @param {string} prefix
     * @param {string} target_mode
     * @returns {number}
     */
    number_shapes_on_page(page_id, shape_ids, order_mode, start_num, interval, prefix, target_mode) {
        const val5 = +page_id;
        const val6 = +start_num;
        const val7 = +interval;
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(order_mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(prefix, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passStringToWasm0(target_mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len3 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_number_shapes_on_page(this.__wbg_ptr, val5, ptr0, len0, ptr1, len1, val6, val7, ptr2, len2, ptr3, len3);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {Uint8Array | number[]} bytes
     * @param {string} ext
     */
    open_bytes(bytes, ext) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(ext, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_open_bytes(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Marquee Lasso Multi-Select resolving top-level page shapes & groups via Visio enclosure rules.
     * @param {number} page_id
     * @param {number} x1_in
     * @param {number} y1_in
     * @param {number} x2_in
     * @param {number} y2_in
     * @returns {Uint32Array}
     */
    query_box_shapes(page_id, x1_in, y1_in, x2_in, y2_in) {
        const ret = wasm.wasmvexiosession_query_box_shapes(this.__wbg_ptr, page_id, x1_in, y1_in, x2_in, y2_in);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * @param {number} page_id
     * @returns {string}
     */
    recalculate_vsm_and_fta_json(page_id) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.wasmvexiosession_recalculate_vsm_and_fta_json(this.__wbg_ptr, page_id);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {boolean}
     */
    redo() {
        const ret = wasm.wasmvexiosession_redo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} recordset_id
     * @param {Uint8Array | number[]} new_bytes
     * @param {string} format_hint
     * @returns {number}
     */
    refresh_external_recordset_bytes(page_id, recordset_id, new_bytes, format_hint) {
        const val3 = +page_id;
        const val4 = +recordset_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray8ToWasm0(new_bytes, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(format_hint, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_refresh_external_recordset_bytes(this.__wbg_ptr, val3, val4, ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * Stream custom `.ttf` / `.otf` TrueType/OpenType bytes (e.g., from ChromeOS Local Font Access API `.blob()`)
     * directly into the `vexio-render` TrueType shaper & PDF contour engine.
     * @param {Uint8Array | number[]} bytes
     * @param {string} family_override
     * @returns {boolean}
     */
    register_font_bytes(bytes, family_override) {
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(family_override, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_register_font_bytes(this.__wbg_ptr, ptr0, len0, ptr1, len1);
        return ret !== 0;
    }
    /**
     * @param {string} stencil_title
     * @param {string} old_name
     * @param {string} new_name
     * @returns {boolean}
     */
    rename_custom_master(stencil_title, old_name, new_name) {
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(stencil_title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(old_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(new_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_rename_custom_master(this.__wbg_ptr, ptr0, len0, ptr1, len1, ptr2, len2);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {string} new_name
     * @returns {boolean}
     */
    rename_page(page_id, new_name) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(new_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_rename_page(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} new_name
     * @returns {boolean}
     */
    rename_shape(page_id, shape_id, new_name) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(new_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_rename_shape(this.__wbg_ptr, val2, val3, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} selected_ids
     * @returns {boolean}
     */
    render_offscreen_frame(page_id, selected_ids) {
        const val2 = +page_id;
        const ptr0 = passArray32ToWasm0(selected_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_render_offscreen_frame(this.__wbg_ptr, val2, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {string} mode
     * @returns {number}
     */
    reorder_shapes(page_id, shape_ids, mode) {
        const val3 = +page_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(mode, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 4, 4); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_reorder_shapes(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
        return ret >>> 0;
    }
    /**
     * @param {number} active_page_id
     * @param {string} find_str
     * @param {string} replace_str
     * @param {boolean} match_case
     * @param {boolean} whole_word
     * @param {boolean} all_pages
     * @returns {number}
     */
    replace_in_document(active_page_id, find_str, replace_str, match_case, whole_word, all_pages) {
        const val3 = +active_page_id;
        const val4 = +match_case;
        const val5 = +whole_word;
        const val6 = +all_pages;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(find_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(replace_str, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_replace_in_document(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1, val4, val5, val6);
        return ret >>> 0;
    }
    /**
     * @param {Uint8Array | number[]} bytes
     */
    replay_wal(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_replay_wal(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @returns {boolean}
     */
    reset_connector_routing(page_id, shape_id) {
        const ret = wasm.wasmvexiosession_reset_connector_routing(this.__wbg_ptr, page_id, shape_id);
        return ret !== 0;
    }
    /**
     * @param {number} width
     * @param {number} height
     * @param {number} dpi
     */
    resize_offscreen_canvas(width, height, dpi) {
        wasm.wasmvexiosession_resize_offscreen_canvas(this.__wbg_ptr, width, height, dpi);
    }
    /**
     * @param {number} comment_id
     * @param {boolean} resolved
     * @returns {boolean}
     */
    resolve_comment(comment_id, resolved) {
        const ret = wasm.wasmvexiosession_resolve_comment(this.__wbg_ptr, comment_id, resolved);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {string} sql_ddl
     * @returns {number}
     */
    reverse_engineer_sql(page_id, sql_ddl) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(sql_ddl, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_reverse_engineer_sql(this.__wbg_ptr, val2, ptr0, len0);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {number} delta_angle_deg
     * @param {boolean} toggle_flip_x
     * @param {boolean} toggle_flip_y
     * @returns {number}
     */
    rotate_flip_shapes(page_id, shape_ids, delta_angle_deg, toggle_flip_x, toggle_flip_y) {
        const val2 = +page_id;
        const val3 = +delta_angle_deg;
        const val4 = +toggle_flip_x;
        const val5 = +toggle_flip_y;
        const ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_rotate_flip_shapes(this.__wbg_ptr, val2, ptr0, len0, val3, val4, val5);
        return ret >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {string} wizard_slug
     * @param {string} spec_text
     * @returns {string}
     */
    run_domain_wizard(page_id, wizard_slug, spec_text) {
        let deferred5_0;
        let deferred5_1;
        try {
            const val3 = +page_id;
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(wizard_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passStringToWasm0(spec_text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.wasmvexiosession_run_domain_wizard(this.__wbg_ptr, val3, ptr0, len0, ptr1, len1);
            var ptr4 = ret[0];
            var len4 = ret[1];
            if (ret[3]) {
                ptr4 = 0; len4 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred5_0 = ptr4;
            deferred5_1 = len4;
            return getStringFromWasm0(ptr4, len4);
        } finally {
            wasm.__wbindgen_free(deferred5_0, deferred5_1, 1);
        }
    }
    /**
     * @param {string} script
     * @returns {string}
     */
    run_vba_macro(script) {
        let deferred3_0;
        let deferred3_1;
        try {
            const ptr0 = passStringToWasm0(script, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmvexiosession_run_vba_macro(this.__wbg_ptr, ptr0, len0);
            var ptr2 = ret[0];
            var len2 = ret[1];
            if (ret[3]) {
                ptr2 = 0; len2 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred3_0 = ptr2;
            deferred3_1 = len2;
            return getStringFromWasm0(ptr2, len2);
        } finally {
            wasm.__wbindgen_free(deferred3_0, deferred3_1, 1);
        }
    }
    /**
     * @param {string} script
     * @param {Uint32Array | number[]} selected_ids
     * @returns {string}
     */
    run_vba_macro_with_selection(script, selected_ids) {
        let deferred4_0;
        let deferred4_1;
        try {
            let ptr0, len0, ptr1, len1;
            try {
                ptr0 = passStringToWasm0(script, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
                len0 = WASM_VECTOR_LEN;
                ptr1 = passArray32ToWasm0(selected_ids, wasm.__wbindgen_malloc);
                len1 = WASM_VECTOR_LEN;
            } catch (e) {
                if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 4, 4); }
                if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
                throw e;
            }
            const ret = wasm.wasmvexiosession_run_vba_macro_with_selection(this.__wbg_ptr, ptr0, len0, ptr1, len1);
            var ptr3 = ret[0];
            var len3 = ret[1];
            if (ret[3]) {
                ptr3 = 0; len3 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred4_0 = ptr3;
            deferred4_1 = len3;
            return getStringFromWasm0(ptr3, len3);
        } finally {
            wasm.__wbindgen_free(deferred4_0, deferred4_1, 1);
        }
    }
    /**
     * @param {string} ext
     * @returns {Uint8Array}
     */
    save_format(ext) {
        const ptr0 = passStringToWasm0(ext, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_save_format(this.__wbg_ptr, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v2 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v2;
    }
    /**
     * @param {number} page_id
     * @param {string} ext
     * @returns {Uint8Array}
     */
    save_page_format(page_id, ext) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(ext, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_save_page_format(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v3 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v3;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} cell_name
     * @param {string} formula
     * @param {boolean} force_setf
     * @returns {boolean}
     */
    set_cell_formula(page_id, shape_id, cell_name, formula, force_setf) {
        const val3 = +page_id;
        const val4 = +shape_id;
        const val5 = +force_setf;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(cell_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(formula, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_set_cell_formula(this.__wbg_ptr, val3, val4, ptr0, len0, ptr1, len1, val5);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} shape_ids
     * @param {number} begin_arrow
     * @param {number} end_arrow
     * @param {number} arrow_size
     * @returns {number}
     */
    set_connector_arrowheads(page_id, shape_ids, begin_arrow, end_arrow, arrow_size) {
        const val2 = +page_id;
        const val3 = +begin_arrow;
        const val4 = +end_arrow;
        const val5 = +arrow_size;
        const ptr0 = passArray32ToWasm0(shape_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_connector_arrowheads(this.__wbg_ptr, val2, ptr0, len0, val3, val4, val5);
        return ret >>> 0;
    }
    /**
     * @param {string} title
     */
    set_document_title(title) {
        const ptr0 = passStringToWasm0(title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmvexiosession_set_document_title(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} description
     * @param {string} address
     * @param {string} sub_address
     * @param {string} screentip
     * @returns {boolean}
     */
    set_hyperlink_and_screentip(page_id, shape_id, description, address, sub_address, screentip) {
        const val5 = +page_id;
        const val6 = +shape_id;
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3;
        try {
            ptr0 = passStringToWasm0(description, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(sub_address, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passStringToWasm0(screentip, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len3 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_set_hyperlink_and_screentip(this.__wbg_ptr, val5, val6, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} layer_ix
     * @param {boolean} visible
     * @param {boolean} lock
     * @param {boolean} print
     * @param {boolean} snap_glue
     * @returns {boolean}
     */
    set_layer_flags(page_id, layer_ix, visible, lock, print, snap_glue) {
        const ret = wasm.wasmvexiosession_set_layer_flags(this.__wbg_ptr, page_id, layer_ix, visible, lock, print, snap_glue);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {boolean} is_background
     * @param {number} back_page_id
     * @param {string} page_color_formula
     * @returns {boolean}
     */
    set_page_background(page_id, is_background, back_page_id, page_color_formula) {
        const val2 = +page_id;
        const val3 = +is_background;
        const val4 = +back_page_id;
        const ptr0 = passStringToWasm0(page_color_formula, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_page_background(this.__wbg_ptr, val2, val3, val4, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {string} config_json
     * @returns {number}
     */
    set_page_data_graphics_json(page_id, config_json) {
        const val2 = +page_id;
        const ptr0 = passStringToWasm0(config_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_page_data_graphics_json(this.__wbg_ptr, val2, ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} width_in
     * @param {number} height_in
     * @param {number} jump_style
     */
    set_page_dimensions(page_id, width_in, height_in, jump_style) {
        wasm.wasmvexiosession_set_page_dimensions(this.__wbg_ptr, page_id, width_in, height_in, jump_style);
    }
    /**
     * @param {number} page_id
     * @param {number} jump_code
     * @param {number} jump_style
     * @param {number} factor_x
     * @param {number} factor_y
     * @returns {boolean}
     */
    set_page_line_jumps(page_id, jump_code, jump_style, factor_x, factor_y) {
        const ret = wasm.wasmvexiosession_set_page_line_jumps(this.__wbg_ptr, page_id, jump_code, jump_style, factor_x, factor_y);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {string} preset_slug
     * @param {boolean} landscape
     * @returns {boolean}
     */
    set_page_preset(page_id, preset_slug, landscape) {
        const val2 = +page_id;
        const val3 = +landscape;
        const ptr0 = passStringToWasm0(preset_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_page_preset(this.__wbg_ptr, val2, ptr0, len0, val3);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} width_in
     * @param {number} height_in
     * @param {string} unit_slug
     * @returns {boolean}
     */
    set_page_setup_with_unit(page_id, width_in, height_in, unit_slug) {
        const val2 = +page_id;
        const val3 = +width_in;
        const val4 = +height_in;
        const ptr0 = passStringToWasm0(unit_slug, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_page_setup_with_unit(this.__wbg_ptr, val2, val3, val4, ptr0, len0);
        return ret !== 0;
    }
    /**
     * @param {string} replica_id
     */
    set_replica_id(replica_id) {
        const ptr0 = passStringToWasm0(replica_id, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmvexiosession_set_replica_id(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} layer_ix
     * @param {boolean} assigned
     * @returns {boolean}
     */
    set_shape_layer_membership(page_id, shape_id, layer_ix, assigned) {
        const ret = wasm.wasmvexiosession_set_shape_layer_membership(this.__wbg_ptr, page_id, shape_id, layer_ix, assigned);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} runs_json
     */
    set_shape_rich_runs_json(page_id, shape_id, runs_json) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(runs_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_shape_rich_runs_json(this.__wbg_ptr, val2, val3, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} text
     */
    set_shape_text(page_id, shape_id, text) {
        const val2 = +page_id;
        const val3 = +shape_id;
        const ptr0 = passStringToWasm0(text, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_set_shape_text(this.__wbg_ptr, val2, val3, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} page_id
     * @param {boolean} snap_enabled
     * @param {boolean} glue_enabled
     * @param {number} snap_targets_mask
     * @param {number} glue_types_mask
     * @returns {boolean}
     */
    set_snap_glue(page_id, snap_enabled, glue_enabled, snap_targets_mask, glue_types_mask) {
        const ret = wasm.wasmvexiosession_set_snap_glue(this.__wbg_ptr, page_id, snap_enabled, glue_enabled, snap_targets_mask, glue_types_mask);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} txt_pin_x_in
     * @param {number} txt_pin_y_in
     * @param {number} txt_width_in
     * @param {number} txt_height_in
     * @param {number} txt_angle_deg
     */
    set_text_block_transform(page_id, shape_id, txt_pin_x_in, txt_pin_y_in, txt_width_in, txt_height_in, txt_angle_deg) {
        const ret = wasm.wasmvexiosession_set_text_block_transform(this.__wbg_ptr, page_id, shape_id, txt_pin_x_in, txt_pin_y_in, txt_width_in, txt_height_in, txt_angle_deg);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @param {number} page_id
     * @param {number} recordset_id
     * @param {string} format_ext
     * @returns {Uint8Array}
     */
    sync_and_export_recordset_bytes(page_id, recordset_id, format_ext) {
        const val2 = +page_id;
        const val3 = +recordset_id;
        const ptr0 = passStringToWasm0(format_ext, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_sync_and_export_recordset_bytes(this.__wbg_ptr, val2, val3, ptr0, len0);
        if (ret[3]) {
            throw takeFromExternrefTable0(ret[2]);
        }
        var v4 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v4;
    }
    /**
     * 8-Handle Interactive Resize + Top Rotation Knob transaction respecting `LockWidth`,
     * `LockHeight`, `LockAspect`, `LockRotate`, and `GUARD`.
     * @param {number} page_id
     * @param {number} shape_id
     * @param {number} pin_x_in
     * @param {number} pin_y_in
     * @param {number} width_in
     * @param {number} height_in
     * @param {number} angle_deg
     * @returns {boolean}
     */
    transform_shape(page_id, shape_id, pin_x_in, pin_y_in, width_in, height_in, angle_deg) {
        const ret = wasm.wasmvexiosession_transform_shape(this.__wbg_ptr, page_id, shape_id, pin_x_in, pin_y_in, width_in, height_in, angle_deg);
        return ret !== 0;
    }
    /**
     * @returns {boolean}
     */
    undo() {
        const ret = wasm.wasmvexiosession_undo(this.__wbg_ptr);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {Uint32Array | number[]} group_ids
     * @returns {Uint32Array}
     */
    ungroup_shapes(page_id, group_ids) {
        const val2 = +page_id;
        const ptr0 = passArray32ToWasm0(group_ids, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.wasmvexiosession_ungroup_shapes(this.__wbg_ptr, val2, ptr0, len0);
        var v3 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v3;
    }
    /**
     * @param {number} page_id
     * @param {number} source_shape_id
     * @param {string} stencil_title
     * @param {string} master_name
     * @returns {number}
     */
    update_custom_master(page_id, source_shape_id, stencil_title, master_name) {
        const val3 = +page_id;
        const val4 = +source_shape_id;
        let ptr0, len0, ptr1, len1;
        try {
            ptr0 = passStringToWasm0(stencil_title, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(master_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_update_custom_master(this.__wbg_ptr, val3, val4, ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return ret[0] >>> 0;
    }
    /**
     * @param {number} page_id
     * @param {number} layer_ix
     * @param {boolean} visible
     * @param {boolean} lock
     * @param {string} color_hex
     */
    update_layer(page_id, layer_ix, visible, lock, color_hex) {
        const val2 = +page_id;
        const val3 = +layer_ix;
        const val4 = +visible;
        const val5 = +lock;
        const ptr0 = passStringToWasm0(color_hex, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.wasmvexiosession_update_layer(this.__wbg_ptr, val2, val3, val4, val5, ptr0, len0);
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} row_name
     * @param {string} menu
     * @param {string} action_formula
     * @param {string} checked_formula
     * @param {string} disabled_formula
     * @param {boolean} begin_group
     * @returns {boolean}
     */
    upsert_shape_action(page_id, shape_id, row_name, menu, action_formula, checked_formula, disabled_formula, begin_group) {
        const val6 = +page_id;
        const val7 = +shape_id;
        const val8 = +begin_group;
        let ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4;
        try {
            ptr0 = passStringToWasm0(row_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(menu, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(action_formula, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
            ptr3 = passStringToWasm0(checked_formula, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len3 = WASM_VECTOR_LEN;
            ptr4 = passStringToWasm0(disabled_formula, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len4 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr4) { wasm.__wbindgen_free(ptr4, len4 * 1, 1); }
            if (ptr3) { wasm.__wbindgen_free(ptr3, len3 * 1, 1); }
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_upsert_shape_action(this.__wbg_ptr, val6, val7, ptr0, len0, ptr1, len1, ptr2, len2, ptr3, len3, ptr4, len4, val8);
        return ret !== 0;
    }
    /**
     * @param {number} page_id
     * @param {number} shape_id
     * @param {string} row_name
     * @param {string} label
     * @param {number} data_type
     * @param {string} value_input
     * @returns {boolean}
     */
    upsert_shape_data(page_id, shape_id, row_name, label, data_type, value_input) {
        const val4 = +page_id;
        const val5 = +shape_id;
        const val6 = +data_type;
        let ptr0, len0, ptr1, len1, ptr2, len2;
        try {
            ptr0 = passStringToWasm0(row_name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len0 = WASM_VECTOR_LEN;
            ptr1 = passStringToWasm0(label, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len1 = WASM_VECTOR_LEN;
            ptr2 = passStringToWasm0(value_input, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            len2 = WASM_VECTOR_LEN;
        } catch (e) {
            if (ptr2) { wasm.__wbindgen_free(ptr2, len2 * 1, 1); }
            if (ptr1) { wasm.__wbindgen_free(ptr1, len1 * 1, 1); }
            if (ptr0) { wasm.__wbindgen_free(ptr0, len0 * 1, 1); }
            throw e;
        }
        const ret = wasm.wasmvexiosession_upsert_shape_data(this.__wbg_ptr, val4, val5, ptr0, len0, ptr1, len1, val6, ptr2, len2);
        return ret !== 0;
    }
    /**
     * @param {string} ruleset
     * @returns {string}
     */
    validate_diagram(ruleset) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(ruleset, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.wasmvexiosession_validate_diagram(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
}
if (Symbol.dispose) WasmVexioSession.prototype[Symbol.dispose] = WasmVexioSession.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg_Error_208446807d0d1e30: function(arg0, arg1) {
            const ret = Error(getStringFromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_Window_cf67da1fb12d51da: function(arg0) {
            const ret = arg0.Window;
            return ret;
        },
        __wbg_WorkerGlobalScope_6780b2e2ad7552b5: function(arg0) {
            const ret = arg0.WorkerGlobalScope;
            return ret;
        },
        __wbg___wbindgen_boolean_get_4f1baf9a00be3c74: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_f249a2a76caa9a55: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_741ff2bb612836f5: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_c22311b1cc228e3a: function(arg0) {
            const ret = arg0 === null;
            return ret;
        },
        __wbg___wbindgen_is_undefined_be5b34249ff1fc15: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_number_get_934b640d0889af78: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64((arg0 >>> 0) + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_7ce8eee7fd327545: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_09736879a3f67559: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_activeTexture_45b425a49df733ce: function(arg0, arg1) {
            arg0.activeTexture(arg1 >>> 0);
        },
        __wbg_activeTexture_89cef8c297ad0698: function(arg0, arg1) {
            arg0.activeTexture(arg1 >>> 0);
        },
        __wbg_attachShader_3ed301cc92db59f6: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_attachShader_817dcec9e79fd18f: function(arg0, arg1, arg2) {
            arg0.attachShader(arg1, arg2);
        },
        __wbg_beginQuery_f800947e639433e8: function(arg0, arg1, arg2) {
            arg0.beginQuery(arg1 >>> 0, arg2);
        },
        __wbg_beginRenderPass_48136d232ff2fb11: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.beginRenderPass(arg1);
            return ret;
        }, arguments); },
        __wbg_bindAttribLocation_40c41e6185822b52: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.bindAttribLocation(arg1, arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        },
        __wbg_bindAttribLocation_b51bf2d65ded8f70: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.bindAttribLocation(arg1, arg2 >>> 0, getStringFromWasm0(arg3, arg4));
        },
        __wbg_bindBufferRange_da5accbcd5d223ad: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.bindBufferRange(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5);
        },
        __wbg_bindBuffer_b3bb001fdbee089a: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindBuffer_cbc9075f76fffa51: function(arg0, arg1, arg2) {
            arg0.bindBuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindFramebuffer_79534f24b57b7b95: function(arg0, arg1, arg2) {
            arg0.bindFramebuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindFramebuffer_b245cd93a150dd96: function(arg0, arg1, arg2) {
            arg0.bindFramebuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindRenderbuffer_3458a69a6dc55057: function(arg0, arg1, arg2) {
            arg0.bindRenderbuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindRenderbuffer_612f27d9b3b97b9f: function(arg0, arg1, arg2) {
            arg0.bindRenderbuffer(arg1 >>> 0, arg2);
        },
        __wbg_bindSampler_0fb033a6ffd5d7b3: function(arg0, arg1, arg2) {
            arg0.bindSampler(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_6f822943b2790466: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_bindTexture_a23350e1b573ddfd: function(arg0, arg1, arg2) {
            arg0.bindTexture(arg1 >>> 0, arg2);
        },
        __wbg_bindVertexArrayOES_5161c1f5b1fea3f9: function(arg0, arg1) {
            arg0.bindVertexArrayOES(arg1);
        },
        __wbg_bindVertexArray_cc64c48bf67ab8d4: function(arg0, arg1) {
            arg0.bindVertexArray(arg1);
        },
        __wbg_blendColor_8631cab46394c93b: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendColor(arg1, arg2, arg3, arg4);
        },
        __wbg_blendColor_957c8ddad53063a2: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendColor(arg1, arg2, arg3, arg4);
        },
        __wbg_blendEquationSeparate_0dceb23863585a87: function(arg0, arg1, arg2) {
            arg0.blendEquationSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blendEquationSeparate_156fe040a9aebfd6: function(arg0, arg1, arg2) {
            arg0.blendEquationSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blendEquation_847eb360af3b2786: function(arg0, arg1) {
            arg0.blendEquation(arg1 >>> 0);
        },
        __wbg_blendEquation_cd72c0ac181cd88d: function(arg0, arg1) {
            arg0.blendEquation(arg1 >>> 0);
        },
        __wbg_blendFuncSeparate_6db2212299fd7f1e: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_blendFuncSeparate_eb9098d760f4a25c: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.blendFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_blendFunc_378a3be8ab6cb7d0: function(arg0, arg1, arg2) {
            arg0.blendFunc(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blendFunc_68b4cfb9bd98a13f: function(arg0, arg1, arg2) {
            arg0.blendFunc(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_blitFramebuffer_d17b7bddf5e86dbc: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.blitFramebuffer(arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0);
        },
        __wbg_bufferData_2fb8740d60478bf0: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_41b591a9f2d502ad: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_9305dba3ba9cba4e: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferData_9eb9c4c5053ac480: function(arg0, arg1, arg2, arg3) {
            arg0.bufferData(arg1 >>> 0, arg2, arg3 >>> 0);
        },
        __wbg_bufferSubData_8acfdcd4b9f7edeb: function(arg0, arg1, arg2, arg3) {
            arg0.bufferSubData(arg1 >>> 0, arg2, arg3);
        },
        __wbg_bufferSubData_9bec93d67e98757e: function(arg0, arg1, arg2, arg3) {
            arg0.bufferSubData(arg1 >>> 0, arg2, arg3);
        },
        __wbg_buffer_aca05a93c046db1a: function(arg0) {
            const ret = arg0.buffer;
            return ret;
        },
        __wbg_call_a410eb4b59ed5965: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_clearBufferfv_474b28c18db287a8: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearBufferfv(arg1 >>> 0, arg2, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_clearBufferiv_a89cfdf7da6f2819: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearBufferiv(arg1 >>> 0, arg2, getArrayI32FromWasm0(arg3, arg4));
        },
        __wbg_clearBufferuiv_e723936f302725de: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.clearBufferuiv(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4));
        },
        __wbg_clearDepth_3417be30229e823a: function(arg0, arg1) {
            arg0.clearDepth(arg1);
        },
        __wbg_clearDepth_3dc1f52f96f29526: function(arg0, arg1) {
            arg0.clearDepth(arg1);
        },
        __wbg_clearStencil_29f687031586f5e6: function(arg0, arg1) {
            arg0.clearStencil(arg1);
        },
        __wbg_clearStencil_59a8a925116b24ec: function(arg0, arg1) {
            arg0.clearStencil(arg1);
        },
        __wbg_clear_3ffa1ed794f92a04: function(arg0, arg1) {
            arg0.clear(arg1 >>> 0);
        },
        __wbg_clear_ccb3d618da90fea3: function(arg0, arg1) {
            arg0.clear(arg1 >>> 0);
        },
        __wbg_clientWaitSync_fee803cb72073198: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.clientWaitSync(arg1, arg2 >>> 0, arg3 >>> 0);
            return ret;
        },
        __wbg_colorMask_24c1b6c3ec53f8c8: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.colorMask(arg1 !== 0, arg2 !== 0, arg3 !== 0, arg4 !== 0);
        },
        __wbg_colorMask_cfc2d619716d01ed: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.colorMask(arg1 !== 0, arg2 !== 0, arg3 !== 0, arg4 !== 0);
        },
        __wbg_compileShader_11eb6754e9b15970: function(arg0, arg1) {
            arg0.compileShader(arg1);
        },
        __wbg_compileShader_54c3771819e6caba: function(arg0, arg1) {
            arg0.compileShader(arg1);
        },
        __wbg_compressedTexSubImage2D_8706ea3fb9e0180c: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.compressedTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8);
        },
        __wbg_compressedTexSubImage2D_b6a8807f715c2782: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.compressedTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8, arg9);
        },
        __wbg_compressedTexSubImage2D_d66b16b23793038b: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.compressedTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8);
        },
        __wbg_compressedTexSubImage3D_74b7bfbc9a7fc90f: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.compressedTexSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10, arg11);
        },
        __wbg_compressedTexSubImage3D_e1d361485803b713: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.compressedTexSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10);
        },
        __wbg_configure_ba34abd53780af93: function() { return handleError(function (arg0, arg1) {
            arg0.configure(arg1);
        }, arguments); },
        __wbg_copyBufferSubData_b10635982222cbc4: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.copyBufferSubData(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5);
        },
        __wbg_copyTexSubImage2D_0cc80bc35bca1d8e: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.copyTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8);
        },
        __wbg_copyTexSubImage2D_be50078e0f0575f1: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8) {
            arg0.copyTexSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8);
        },
        __wbg_copyTexSubImage3D_e35f615c7c8244ba: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.copyTexSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9);
        },
        __wbg_createBindGroupLayout_361b4129db214524: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBindGroupLayout(arg1);
            return ret;
        }, arguments); },
        __wbg_createBindGroup_3bccbd7517f0708e: function(arg0, arg1) {
            const ret = arg0.createBindGroup(arg1);
            return ret;
        },
        __wbg_createBuffer_2c27ad5f899d418d: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createBuffer_5af24c32f25d7e90: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createBuffer(arg1);
            return ret;
        }, arguments); },
        __wbg_createBuffer_ca6ca40e9fb13678: function(arg0) {
            const ret = arg0.createBuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createCommandEncoder_48a406baaa084912: function(arg0, arg1) {
            const ret = arg0.createCommandEncoder(arg1);
            return ret;
        },
        __wbg_createFramebuffer_cdf686778c61014d: function(arg0) {
            const ret = arg0.createFramebuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createFramebuffer_f52399034cb1a200: function(arg0) {
            const ret = arg0.createFramebuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createPipelineLayout_f668b6fbdf877ab3: function(arg0, arg1) {
            const ret = arg0.createPipelineLayout(arg1);
            return ret;
        },
        __wbg_createProgram_de0881c688a8e43b: function(arg0) {
            const ret = arg0.createProgram();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createProgram_ea784724f6f0074f: function(arg0) {
            const ret = arg0.createProgram();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createQuery_8f0481bdf42276b8: function(arg0) {
            const ret = arg0.createQuery();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createRenderPipeline_7001840beafefb73: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createRenderPipeline(arg1);
            return ret;
        }, arguments); },
        __wbg_createRenderbuffer_19c2eec619a37a15: function(arg0) {
            const ret = arg0.createRenderbuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createRenderbuffer_e45c14c918a72668: function(arg0) {
            const ret = arg0.createRenderbuffer();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createSampler_5bc030b64ed02c52: function(arg0) {
            const ret = arg0.createSampler();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createShaderModule_1b0812f3a4503221: function(arg0, arg1) {
            const ret = arg0.createShaderModule(arg1);
            return ret;
        },
        __wbg_createShader_9754a1e4ef374e84: function(arg0, arg1) {
            const ret = arg0.createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createShader_d2d90020273cef2b: function(arg0, arg1) {
            const ret = arg0.createShader(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createTexture_3287f26407473071: function(arg0) {
            const ret = arg0.createTexture();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createTexture_701072deb7c2b006: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createTexture(arg1);
            return ret;
        }, arguments); },
        __wbg_createTexture_eec9a1994aabeca7: function(arg0) {
            const ret = arg0.createTexture();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createVertexArrayOES_3eaf087ef22da1cb: function(arg0) {
            const ret = arg0.createVertexArrayOES();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createVertexArray_1293d2ffa8346be2: function(arg0) {
            const ret = arg0.createVertexArray();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_createView_49c3dfbbb459d385: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.createView(arg1);
            return ret;
        }, arguments); },
        __wbg_cullFace_208151a46885a1fa: function(arg0, arg1) {
            arg0.cullFace(arg1 >>> 0);
        },
        __wbg_cullFace_9eaca57652d7ed37: function(arg0, arg1) {
            arg0.cullFace(arg1 >>> 0);
        },
        __wbg_deleteBuffer_673584ba5a55669d: function(arg0, arg1) {
            arg0.deleteBuffer(arg1);
        },
        __wbg_deleteBuffer_ffa760fd3b0b5ef1: function(arg0, arg1) {
            arg0.deleteBuffer(arg1);
        },
        __wbg_deleteFramebuffer_0ec884f89db72a3e: function(arg0, arg1) {
            arg0.deleteFramebuffer(arg1);
        },
        __wbg_deleteFramebuffer_ffdb1bee4c92ae4c: function(arg0, arg1) {
            arg0.deleteFramebuffer(arg1);
        },
        __wbg_deleteProgram_8c38fff0f4a88dea: function(arg0, arg1) {
            arg0.deleteProgram(arg1);
        },
        __wbg_deleteProgram_c07d952a1b0c1cb6: function(arg0, arg1) {
            arg0.deleteProgram(arg1);
        },
        __wbg_deleteQuery_dfd665f6d8652e22: function(arg0, arg1) {
            arg0.deleteQuery(arg1);
        },
        __wbg_deleteRenderbuffer_3a6e1676ab926e0f: function(arg0, arg1) {
            arg0.deleteRenderbuffer(arg1);
        },
        __wbg_deleteRenderbuffer_895ff607251243a9: function(arg0, arg1) {
            arg0.deleteRenderbuffer(arg1);
        },
        __wbg_deleteSampler_2b73428131e403e1: function(arg0, arg1) {
            arg0.deleteSampler(arg1);
        },
        __wbg_deleteShader_320530dd636bc187: function(arg0, arg1) {
            arg0.deleteShader(arg1);
        },
        __wbg_deleteShader_ac74568b64a1c3c7: function(arg0, arg1) {
            arg0.deleteShader(arg1);
        },
        __wbg_deleteSync_eede535e5826f144: function(arg0, arg1) {
            arg0.deleteSync(arg1);
        },
        __wbg_deleteTexture_1653a9994a44215e: function(arg0, arg1) {
            arg0.deleteTexture(arg1);
        },
        __wbg_deleteTexture_89bd1d49ae4fb309: function(arg0, arg1) {
            arg0.deleteTexture(arg1);
        },
        __wbg_deleteVertexArrayOES_2214ecdf65be031d: function(arg0, arg1) {
            arg0.deleteVertexArrayOES(arg1);
        },
        __wbg_deleteVertexArray_2589b74b67ba20c8: function(arg0, arg1) {
            arg0.deleteVertexArray(arg1);
        },
        __wbg_depthFunc_04abd9f6ff980ba9: function(arg0, arg1) {
            arg0.depthFunc(arg1 >>> 0);
        },
        __wbg_depthFunc_584bfa0174e7628a: function(arg0, arg1) {
            arg0.depthFunc(arg1 >>> 0);
        },
        __wbg_depthMask_276856a569045897: function(arg0, arg1) {
            arg0.depthMask(arg1 !== 0);
        },
        __wbg_depthMask_2ba7e83178c3cec3: function(arg0, arg1) {
            arg0.depthMask(arg1 !== 0);
        },
        __wbg_depthRange_8b4b69a7c91ff92b: function(arg0, arg1, arg2) {
            arg0.depthRange(arg1, arg2);
        },
        __wbg_depthRange_942825e3232596de: function(arg0, arg1, arg2) {
            arg0.depthRange(arg1, arg2);
        },
        __wbg_disableVertexAttribArray_011898581287ef1e: function(arg0, arg1) {
            arg0.disableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_disableVertexAttribArray_5c66375eba552ee7: function(arg0, arg1) {
            arg0.disableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_disable_2ead6c11cefd0322: function(arg0, arg1) {
            arg0.disable(arg1 >>> 0);
        },
        __wbg_disable_8a9e81e6d64d046b: function(arg0, arg1) {
            arg0.disable(arg1 >>> 0);
        },
        __wbg_document_3b5c3071b43e97ee: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_drawArraysInstancedANGLE_635e10224985b842: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.drawArraysInstancedANGLE(arg1 >>> 0, arg2, arg3, arg4);
        },
        __wbg_drawArraysInstanced_762b36f2147c7a85: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.drawArraysInstanced(arg1 >>> 0, arg2, arg3, arg4);
        },
        __wbg_drawArrays_a9c18db2e94f4a82: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_drawArrays_fe5a51410bfabb5b: function(arg0, arg1, arg2, arg3) {
            arg0.drawArrays(arg1 >>> 0, arg2, arg3);
        },
        __wbg_drawBuffersWEBGL_49c672d0ddcdfe71: function(arg0, arg1) {
            arg0.drawBuffersWEBGL(arg1);
        },
        __wbg_drawBuffers_e296659ce384e721: function(arg0, arg1) {
            arg0.drawBuffers(arg1);
        },
        __wbg_drawElementsInstancedANGLE_470ce8b489eeafa5: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawElementsInstancedANGLE(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_drawElementsInstanced_3240c0c168d6af87: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawElementsInstanced(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_drawIndexed_fd47a285c65bc454: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.drawIndexed(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5 >>> 0);
        },
        __wbg_enableVertexAttribArray_d485706ade6bebf8: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_enableVertexAttribArray_eeecc6185af24227: function(arg0, arg1) {
            arg0.enableVertexAttribArray(arg1 >>> 0);
        },
        __wbg_enable_c3c00565c89d3175: function(arg0, arg1) {
            arg0.enable(arg1 >>> 0);
        },
        __wbg_enable_e955d456ef6b93f7: function(arg0, arg1) {
            arg0.enable(arg1 >>> 0);
        },
        __wbg_endQuery_f052a8f0f3589d3b: function(arg0, arg1) {
            arg0.endQuery(arg1 >>> 0);
        },
        __wbg_end_1300dc816a60c7ab: function(arg0) {
            arg0.end();
        },
        __wbg_features_75ae263739edbe15: function(arg0) {
            const ret = arg0.features;
            return ret;
        },
        __wbg_fenceSync_53f6f24f4bb43d24: function(arg0, arg1, arg2) {
            const ret = arg0.fenceSync(arg1 >>> 0, arg2 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_finish_2440fb64e53f7d5a: function(arg0, arg1) {
            const ret = arg0.finish(arg1);
            return ret;
        },
        __wbg_finish_4b40810f0b577bc2: function(arg0) {
            const ret = arg0.finish();
            return ret;
        },
        __wbg_framebufferRenderbuffer_1c89563506a7d0ac: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.framebufferRenderbuffer(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4);
        },
        __wbg_framebufferRenderbuffer_ffaba93222aea77c: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.framebufferRenderbuffer(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4);
        },
        __wbg_framebufferTexture2D_2c69ac0ce67faf7d: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTexture2D(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5);
        },
        __wbg_framebufferTexture2D_b6250a0c37eb48c2: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTexture2D(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4, arg5);
        },
        __wbg_framebufferTextureLayer_2f6e24e2dbf9b214: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.framebufferTextureLayer(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5);
        },
        __wbg_framebufferTextureMultiviewOVR_63308e847d8b8f6c: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.framebufferTextureMultiviewOVR(arg1 >>> 0, arg2 >>> 0, arg3, arg4, arg5, arg6);
        },
        __wbg_frontFace_124bb11a73cc0dd1: function(arg0, arg1) {
            arg0.frontFace(arg1 >>> 0);
        },
        __wbg_frontFace_8bbd77ca3b8f90a6: function(arg0, arg1) {
            arg0.frontFace(arg1 >>> 0);
        },
        __wbg_getBufferSubData_4a9f8244f0e59e63: function(arg0, arg1, arg2, arg3) {
            arg0.getBufferSubData(arg1 >>> 0, arg2, arg3);
        },
        __wbg_getContext_1938edb9a972f63d: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_9d32151c28a36ed6: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2), arg3);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_d93f904b714852ae: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getContext_f2ea37484e3f8ff4: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.getContext(getStringFromWasm0(arg1, arg2), arg3);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getCurrentTexture_344e53f8917b05e0: function() { return handleError(function (arg0) {
            const ret = arg0.getCurrentTexture();
            return ret;
        }, arguments); },
        __wbg_getExtension_30c10f49f67eb05d: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getExtension(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getIndexedParameter_5d1c39ef8e171053: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.getIndexedParameter(arg1 >>> 0, arg2 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getParameter_4c67158a3cef7a34: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.getParameter(arg1 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getParameter_7fbe79a0114e119f: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.getParameter(arg1 >>> 0);
            return ret;
        }, arguments); },
        __wbg_getPreferredCanvasFormat_1af5deca596e85de: function(arg0) {
            const ret = arg0.getPreferredCanvasFormat();
            return (__wbindgen_enum_GpuTextureFormat.indexOf(ret) + 1 || 96) - 1;
        },
        __wbg_getProgramInfoLog_01799fd7b07b7f3c: function(arg0, arg1, arg2) {
            const ret = arg1.getProgramInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getProgramInfoLog_92d61551e1ab39ea: function(arg0, arg1, arg2) {
            const ret = arg1.getProgramInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getProgramParameter_1e7562e7d66170c3: function(arg0, arg1, arg2) {
            const ret = arg0.getProgramParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getProgramParameter_3edd526f9e5f6c10: function(arg0, arg1, arg2) {
            const ret = arg0.getProgramParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getQueryParameter_0beb5dc153d8ca4f: function(arg0, arg1, arg2) {
            const ret = arg0.getQueryParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getShaderInfoLog_3cfe5b3e097d0f2d: function(arg0, arg1, arg2) {
            const ret = arg1.getShaderInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getShaderInfoLog_8758e8aea7960121: function(arg0, arg1, arg2) {
            const ret = arg1.getShaderInfoLog(arg2);
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getShaderParameter_a346f74d5ba41c4b: function(arg0, arg1, arg2) {
            const ret = arg0.getShaderParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getShaderParameter_fc64069fbc010d19: function(arg0, arg1, arg2) {
            const ret = arg0.getShaderParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getSupportedExtensions_f48b337512e8f8ce: function(arg0) {
            const ret = arg0.getSupportedExtensions();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getSupportedProfiles_b5cc35261935c0c9: function(arg0) {
            const ret = arg0.getSupportedProfiles();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getSyncParameter_8b7b36e9efd95806: function(arg0, arg1, arg2) {
            const ret = arg0.getSyncParameter(arg1, arg2 >>> 0);
            return ret;
        },
        __wbg_getUniformBlockIndex_0742ae35f926850a: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformBlockIndex(arg1, getStringFromWasm0(arg2, arg3));
            return ret;
        },
        __wbg_getUniformLocation_6352e0669c8f5393: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getUniformLocation_e0d11c86376cdd26: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.getUniformLocation(arg1, getStringFromWasm0(arg2, arg3));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_0c55869fe53c7d14: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_unchecked_e6325ad374eff861: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_gpu_e272e5d759d131d3: function(arg0) {
            const ret = arg0.gpu;
            return ret;
        },
        __wbg_has_dc80aa6186153231: function(arg0, arg1, arg2) {
            const ret = arg0.has(getStringFromWasm0(arg1, arg2));
            return ret;
        },
        __wbg_includes_40dae55ee37516e5: function(arg0, arg1, arg2) {
            const ret = arg0.includes(arg1, arg2);
            return ret;
        },
        __wbg_instanceof_GpuAdapter_675fb39e47f29cd5: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUAdapter;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_GpuCanvasContext_15eb056c751cc680: function(arg0) {
            let result;
            try {
                result = arg0 instanceof GPUCanvasContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlCanvasElement_89f280683e987b9e: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLCanvasElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WebGl2RenderingContext_de6feb460369d6c6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof WebGL2RenderingContext;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_5cf453f5639aa1a2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_invalidateFramebuffer_33206c632642a1bd: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.invalidateFramebuffer(arg1 >>> 0, arg2);
        }, arguments); },
        __wbg_is_2248ff2b575d60c7: function(arg0, arg1) {
            const ret = Object.is(arg0, arg1);
            return ret;
        },
        __wbg_label_10b65b51f15135dd: function(arg0, arg1) {
            const ret = arg1.label;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_length_b03a0fabb109b1e9: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_limits_abc8ed416bd58b7f: function(arg0) {
            const ret = arg0.limits;
            return ret;
        },
        __wbg_linkProgram_99848b73f1ebab5f: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_linkProgram_dee7cda98bacb12f: function(arg0, arg1) {
            arg0.linkProgram(arg1);
        },
        __wbg_maxBindGroups_e2c8e975081afa76: function(arg0) {
            const ret = arg0.maxBindGroups;
            return ret;
        },
        __wbg_maxBindingsPerBindGroup_275efff12ccdeb1b: function(arg0) {
            const ret = arg0.maxBindingsPerBindGroup;
            return ret;
        },
        __wbg_maxBufferSize_6f9f2d6fd534e8b1: function(arg0) {
            const ret = arg0.maxBufferSize;
            return ret;
        },
        __wbg_maxColorAttachmentBytesPerSample_c512ac4b3c492cd1: function(arg0) {
            const ret = arg0.maxColorAttachmentBytesPerSample;
            return ret;
        },
        __wbg_maxColorAttachments_c37605f8b6687bbe: function(arg0) {
            const ret = arg0.maxColorAttachments;
            return ret;
        },
        __wbg_maxComputeInvocationsPerWorkgroup_4032960962d67f18: function(arg0) {
            const ret = arg0.maxComputeInvocationsPerWorkgroup;
            return ret;
        },
        __wbg_maxComputeWorkgroupSizeX_908af20a8198e4e8: function(arg0) {
            const ret = arg0.maxComputeWorkgroupSizeX;
            return ret;
        },
        __wbg_maxComputeWorkgroupSizeY_b3652d8b236cdba6: function(arg0) {
            const ret = arg0.maxComputeWorkgroupSizeY;
            return ret;
        },
        __wbg_maxComputeWorkgroupSizeZ_2c01b3e7bd78da03: function(arg0) {
            const ret = arg0.maxComputeWorkgroupSizeZ;
            return ret;
        },
        __wbg_maxComputeWorkgroupStorageSize_633fa9cd024fd1c9: function(arg0) {
            const ret = arg0.maxComputeWorkgroupStorageSize;
            return ret;
        },
        __wbg_maxComputeWorkgroupsPerDimension_f1e98c4fad4c1feb: function(arg0) {
            const ret = arg0.maxComputeWorkgroupsPerDimension;
            return ret;
        },
        __wbg_maxDynamicStorageBuffersPerPipelineLayout_f7f56e8be0e19422: function(arg0) {
            const ret = arg0.maxDynamicStorageBuffersPerPipelineLayout;
            return ret;
        },
        __wbg_maxDynamicUniformBuffersPerPipelineLayout_e04d5ebda3927ec1: function(arg0) {
            const ret = arg0.maxDynamicUniformBuffersPerPipelineLayout;
            return ret;
        },
        __wbg_maxSampledTexturesPerShaderStage_45483cf4cab6346a: function(arg0) {
            const ret = arg0.maxSampledTexturesPerShaderStage;
            return ret;
        },
        __wbg_maxSamplersPerShaderStage_59d6588ff861d07f: function(arg0) {
            const ret = arg0.maxSamplersPerShaderStage;
            return ret;
        },
        __wbg_maxStorageBufferBindingSize_54653537934c0f52: function(arg0) {
            const ret = arg0.maxStorageBufferBindingSize;
            return ret;
        },
        __wbg_maxStorageBuffersPerShaderStage_4ff52d7a543e4117: function(arg0) {
            const ret = arg0.maxStorageBuffersPerShaderStage;
            return ret;
        },
        __wbg_maxStorageTexturesPerShaderStage_20cb9a73e9eed621: function(arg0) {
            const ret = arg0.maxStorageTexturesPerShaderStage;
            return ret;
        },
        __wbg_maxTextureArrayLayers_97c084cacc46d17c: function(arg0) {
            const ret = arg0.maxTextureArrayLayers;
            return ret;
        },
        __wbg_maxTextureDimension1D_c50a14fd8b8fb5db: function(arg0) {
            const ret = arg0.maxTextureDimension1D;
            return ret;
        },
        __wbg_maxTextureDimension2D_a6d9c5a4db4ae6eb: function(arg0) {
            const ret = arg0.maxTextureDimension2D;
            return ret;
        },
        __wbg_maxTextureDimension3D_8d3b6a7b668f1342: function(arg0) {
            const ret = arg0.maxTextureDimension3D;
            return ret;
        },
        __wbg_maxUniformBufferBindingSize_ee95074d61d7b6ee: function(arg0) {
            const ret = arg0.maxUniformBufferBindingSize;
            return ret;
        },
        __wbg_maxUniformBuffersPerShaderStage_5a25d41e64041465: function(arg0) {
            const ret = arg0.maxUniformBuffersPerShaderStage;
            return ret;
        },
        __wbg_maxVertexAttributes_e47b2eebe0faed59: function(arg0) {
            const ret = arg0.maxVertexAttributes;
            return ret;
        },
        __wbg_maxVertexBufferArrayStride_68a069a94ca089eb: function(arg0) {
            const ret = arg0.maxVertexBufferArrayStride;
            return ret;
        },
        __wbg_maxVertexBuffers_50cb1fd2668c2e81: function(arg0) {
            const ret = arg0.maxVertexBuffers;
            return ret;
        },
        __wbg_minStorageBufferOffsetAlignment_c0fb551a08637362: function(arg0) {
            const ret = arg0.minStorageBufferOffsetAlignment;
            return ret;
        },
        __wbg_minUniformBufferOffsetAlignment_d5684347e1f28ff7: function(arg0) {
            const ret = arg0.minUniformBufferOffsetAlignment;
            return ret;
        },
        __wbg_navigator_9641ab32fa612dd1: function(arg0) {
            const ret = arg0.navigator;
            return ret;
        },
        __wbg_navigator_c17f443a6fe5d38d: function(arg0) {
            const ret = arg0.navigator;
            return ret;
        },
        __wbg_new_37eb45aaf5eeb820: function() {
            const ret = new Array();
            return ret;
        },
        __wbg_new_cd4348290afae586: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_from_slice_b8daf545a41c6aa0: function(arg0, arg1) {
            const ret = new Uint8Array(getArrayU8FromWasm0(arg0, arg1));
            return ret;
        },
        __wbg_new_typed_e09da9dae6ee5935: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_now_043d245551b22f9c: function(arg0) {
            const ret = arg0.now();
            return ret;
        },
        __wbg_of_027eb15371aac932: function(arg0) {
            const ret = Array.of(arg0);
            return ret;
        },
        __wbg_performance_50e6a3f8dc80bbca: function(arg0) {
            const ret = arg0.performance;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_pixelStorei_43dc7945a0d4d49c: function(arg0, arg1, arg2) {
            arg0.pixelStorei(arg1 >>> 0, arg2);
        },
        __wbg_pixelStorei_6dd56b4da047c480: function(arg0, arg1, arg2) {
            arg0.pixelStorei(arg1 >>> 0, arg2);
        },
        __wbg_polygonOffset_41bc1f487e5f53fe: function(arg0, arg1, arg2) {
            arg0.polygonOffset(arg1, arg2);
        },
        __wbg_polygonOffset_acf166e60739dbfa: function(arg0, arg1, arg2) {
            arg0.polygonOffset(arg1, arg2);
        },
        __wbg_push_c9f4893fc209f768: function(arg0, arg1) {
            const ret = arg0.push(arg1);
            return ret;
        },
        __wbg_queryCounterEXT_d43bae4582de7efd: function(arg0, arg1, arg2) {
            arg0.queryCounterEXT(arg1, arg2 >>> 0);
        },
        __wbg_querySelectorAll_6c1bd81d448fb480: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelectorAll(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_querySelector_72201b7c62dc196b: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelector(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_queueMicrotask_1e8503a8a690f0a3: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_4a1a4d8dd332373b: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_queue_0539ef8a48f16a20: function(arg0) {
            const ret = arg0.queue;
            return ret;
        },
        __wbg_readBuffer_49d4b278b9838d40: function(arg0, arg1) {
            arg0.readBuffer(arg1 >>> 0);
        },
        __wbg_readPixels_22316d250d820aff: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7);
        }, arguments); },
        __wbg_readPixels_56a96c4d106afcbc: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7);
        }, arguments); },
        __wbg_readPixels_7c7b54a733248182: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7) {
            arg0.readPixels(arg1, arg2, arg3, arg4, arg5 >>> 0, arg6 >>> 0, arg7);
        }, arguments); },
        __wbg_renderbufferStorageMultisample_d5b01825cc08514b: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.renderbufferStorageMultisample(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_renderbufferStorage_0d080ee86520d3bd: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.renderbufferStorage(arg1 >>> 0, arg2 >>> 0, arg3, arg4);
        },
        __wbg_renderbufferStorage_ebe429f577dae949: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.renderbufferStorage(arg1 >>> 0, arg2 >>> 0, arg3, arg4);
        },
        __wbg_requestAdapter_245da40985c2fdc5: function(arg0, arg1) {
            const ret = arg0.requestAdapter(arg1);
            return ret;
        },
        __wbg_requestDevice_28434913a23418c4: function(arg0, arg1) {
            const ret = arg0.requestDevice(arg1);
            return ret;
        },
        __wbg_resolve_dbf78ab4942478b0: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_samplerParameterf_1fb6c6796e22f10f: function(arg0, arg1, arg2, arg3) {
            arg0.samplerParameterf(arg1, arg2 >>> 0, arg3);
        },
        __wbg_samplerParameteri_defcb3b230e696e7: function(arg0, arg1, arg2, arg3) {
            arg0.samplerParameteri(arg1, arg2 >>> 0, arg3);
        },
        __wbg_scissor_17956d9d23dfe025: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.scissor(arg1, arg2, arg3, arg4);
        },
        __wbg_scissor_bb7201f050f220ac: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.scissor(arg1, arg2, arg3, arg4);
        },
        __wbg_setBindGroup_8c6b890e83d98e74: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.setBindGroup(arg1 >>> 0, arg2, getArrayU32FromWasm0(arg3, arg4), arg5, arg6 >>> 0);
        }, arguments); },
        __wbg_setBindGroup_e7493a4d990b460a: function(arg0, arg1, arg2) {
            arg0.setBindGroup(arg1 >>> 0, arg2);
        },
        __wbg_setIndexBuffer_4fb98e6d19bb7f33: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.setIndexBuffer(arg1, __wbindgen_enum_GpuIndexFormat[arg2], arg3, arg4);
        },
        __wbg_setIndexBuffer_7fdf61bb296c38e7: function(arg0, arg1, arg2, arg3) {
            arg0.setIndexBuffer(arg1, __wbindgen_enum_GpuIndexFormat[arg2], arg3);
        },
        __wbg_setPipeline_323a115b52180cad: function(arg0, arg1) {
            arg0.setPipeline(arg1);
        },
        __wbg_setVertexBuffer_e32a2441b1f7bfa3: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3, arg4);
        },
        __wbg_setVertexBuffer_f90b4b03dde32ab1: function(arg0, arg1, arg2, arg3) {
            arg0.setVertexBuffer(arg1 >>> 0, arg2, arg3);
        },
        __wbg_set_2793479888329cb6: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = Reflect.set(arg0, arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_a_a2fe9b7fbde10ee2: function(arg0, arg1) {
            arg0.a = arg1;
        },
        __wbg_set_access_2061b1f73548a9eb: function(arg0, arg1) {
            arg0.access = __wbindgen_enum_GpuStorageTextureAccess[arg1];
        },
        __wbg_set_alpha_26de83866a2b6365: function(arg0, arg1) {
            arg0.alpha = arg1;
        },
        __wbg_set_alpha_mode_6ad30139e5f65145: function(arg0, arg1) {
            arg0.alphaMode = __wbindgen_enum_GpuCanvasAlphaMode[arg1];
        },
        __wbg_set_alpha_to_coverage_enabled_07625a7db710034d: function(arg0, arg1) {
            arg0.alphaToCoverageEnabled = arg1 !== 0;
        },
        __wbg_set_array_layer_count_2a33a637467b3dc5: function(arg0, arg1) {
            arg0.arrayLayerCount = arg1 >>> 0;
        },
        __wbg_set_array_stride_5658ff806f55d526: function(arg0, arg1) {
            arg0.arrayStride = arg1;
        },
        __wbg_set_aspect_a6e567de4d40bd2b: function(arg0, arg1) {
            arg0.aspect = __wbindgen_enum_GpuTextureAspect[arg1];
        },
        __wbg_set_attributes_7b8cf62c4e907fd6: function(arg0, arg1) {
            arg0.attributes = arg1;
        },
        __wbg_set_b_7924cf2425565e31: function(arg0, arg1) {
            arg0.b = arg1;
        },
        __wbg_set_base_array_layer_bd21f5ed0d70ab50: function(arg0, arg1) {
            arg0.baseArrayLayer = arg1 >>> 0;
        },
        __wbg_set_base_mip_level_a00ab03f906f95a5: function(arg0, arg1) {
            arg0.baseMipLevel = arg1 >>> 0;
        },
        __wbg_set_beginning_of_pass_write_index_e1a4f6003c04887d: function(arg0, arg1) {
            arg0.beginningOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_bind_group_layouts_0b6f0ad5823d8453: function(arg0, arg1) {
            arg0.bindGroupLayouts = arg1;
        },
        __wbg_set_binding_8cfbfa8e760f9b2c: function(arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        },
        __wbg_set_binding_ac34fbe91ef6a36c: function(arg0, arg1) {
            arg0.binding = arg1 >>> 0;
        },
        __wbg_set_blend_eb90aa676d095d87: function(arg0, arg1) {
            arg0.blend = arg1;
        },
        __wbg_set_buffer_0a8fd026232e87da: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_buffer_1a7450116c46ac9e: function(arg0, arg1) {
            arg0.buffer = arg1;
        },
        __wbg_set_buffers_359c884f45bdbfcf: function(arg0, arg1) {
            arg0.buffers = arg1;
        },
        __wbg_set_clear_value_51a250f26b225164: function(arg0, arg1) {
            arg0.clearValue = arg1;
        },
        __wbg_set_code_3d00af090aa70700: function(arg0, arg1, arg2) {
            arg0.code = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_color_8ce146e0d257f5f4: function(arg0, arg1) {
            arg0.color = arg1;
        },
        __wbg_set_color_attachments_0fcc0288331a58e8: function(arg0, arg1) {
            arg0.colorAttachments = arg1;
        },
        __wbg_set_compare_7f142d5ffd23d31b: function(arg0, arg1) {
            arg0.compare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_count_68f54de7ae4f98e4: function(arg0, arg1) {
            arg0.count = arg1 >>> 0;
        },
        __wbg_set_cull_mode_6115d51ba799f9ee: function(arg0, arg1) {
            arg0.cullMode = __wbindgen_enum_GpuCullMode[arg1];
        },
        __wbg_set_depth_bias_8acb13bf57c15d7a: function(arg0, arg1) {
            arg0.depthBias = arg1;
        },
        __wbg_set_depth_bias_clamp_38591040fa630418: function(arg0, arg1) {
            arg0.depthBiasClamp = arg1;
        },
        __wbg_set_depth_bias_slope_scale_6244c39ba59b4bc8: function(arg0, arg1) {
            arg0.depthBiasSlopeScale = arg1;
        },
        __wbg_set_depth_clear_value_2d2b8e052076757b: function(arg0, arg1) {
            arg0.depthClearValue = arg1;
        },
        __wbg_set_depth_compare_a8eaa5678a6e7593: function(arg0, arg1) {
            arg0.depthCompare = __wbindgen_enum_GpuCompareFunction[arg1];
        },
        __wbg_set_depth_fail_op_7d304c5796a1098e: function(arg0, arg1) {
            arg0.depthFailOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_depth_load_op_142f6288ff0d982f: function(arg0, arg1) {
            arg0.depthLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_depth_or_array_layers_bfe8d1c1368758ef: function(arg0, arg1) {
            arg0.depthOrArrayLayers = arg1 >>> 0;
        },
        __wbg_set_depth_read_only_fbc0ae97fcf4eefb: function(arg0, arg1) {
            arg0.depthReadOnly = arg1 !== 0;
        },
        __wbg_set_depth_stencil_9f037d75243795b7: function(arg0, arg1) {
            arg0.depthStencil = arg1;
        },
        __wbg_set_depth_stencil_attachment_d4fd688280f32a3d: function(arg0, arg1) {
            arg0.depthStencilAttachment = arg1;
        },
        __wbg_set_depth_store_op_677b5c0df7b87097: function(arg0, arg1) {
            arg0.depthStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_depth_write_enabled_11b8595f8526dea4: function(arg0, arg1) {
            arg0.depthWriteEnabled = arg1 !== 0;
        },
        __wbg_set_device_06d6d4585bfb7bfb: function(arg0, arg1) {
            arg0.device = arg1;
        },
        __wbg_set_dimension_50b9caabd6331d97: function(arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_dimension_55755ad7db8b9c4f: function(arg0, arg1) {
            arg0.dimension = __wbindgen_enum_GpuTextureDimension[arg1];
        },
        __wbg_set_dst_factor_247c0807ea5a5ad5: function(arg0, arg1) {
            arg0.dstFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        },
        __wbg_set_end_of_pass_write_index_2f58fa286c5efaa2: function(arg0, arg1) {
            arg0.endOfPassWriteIndex = arg1 >>> 0;
        },
        __wbg_set_entries_39d6a318f29b2989: function(arg0, arg1) {
            arg0.entries = arg1;
        },
        __wbg_set_entries_7111d9fbfec489a7: function(arg0, arg1) {
            arg0.entries = arg1;
        },
        __wbg_set_entry_point_05b199ba20484e3d: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_entry_point_6265d560b1a76650: function(arg0, arg1, arg2) {
            arg0.entryPoint = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_fail_op_5fcf06fcc904d8ef: function(arg0, arg1) {
            arg0.failOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_format_2f93e27676d0dddb: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuVertexFormat[arg1];
        },
        __wbg_set_format_5ca11d1f5fb17478: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_5f73f017a97b6244: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_7234e43a9ae271ac: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_b4823a74a8155a57: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_cdf1e4995d40224a: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_format_f5a01179c470dc07: function(arg0, arg1) {
            arg0.format = __wbindgen_enum_GpuTextureFormat[arg1];
        },
        __wbg_set_fragment_8e18f36eeeacbbe5: function(arg0, arg1) {
            arg0.fragment = arg1;
        },
        __wbg_set_front_face_255118ec222bd160: function(arg0, arg1) {
            arg0.frontFace = __wbindgen_enum_GpuFrontFace[arg1];
        },
        __wbg_set_g_bea00a94e7ce3fdb: function(arg0, arg1) {
            arg0.g = arg1;
        },
        __wbg_set_has_dynamic_offset_c320e720e48a7605: function(arg0, arg1) {
            arg0.hasDynamicOffset = arg1 !== 0;
        },
        __wbg_set_height_ae0610d318c3428c: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_height_ae24c2f7d580ae0a: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_height_c2d85b31cc89475f: function(arg0, arg1) {
            arg0.height = arg1 >>> 0;
        },
        __wbg_set_label_209d5fbd8b5ed1f0: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_28cd35adef389ec6: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_531708f49e285595: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_67ac6f8fb665b7c2: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_7dc554ae95ff1da5: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_b7ce58718573bbfa: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_ba49905c889557b6: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_be697f18a1980cb1: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_d1556ad0c6189415: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_e6d5698ac1ea5e79: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_f7cf6d51f72f98d5: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_label_fcedc76399351d67: function(arg0, arg1, arg2) {
            arg0.label = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_layout_29fbc08a816773bd: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_layout_724176d6cd95dd34: function(arg0, arg1) {
            arg0.layout = arg1;
        },
        __wbg_set_load_op_74ce6ff007ca9a1c: function(arg0, arg1) {
            arg0.loadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_mapped_at_creation_9de845439ad6f547: function(arg0, arg1) {
            arg0.mappedAtCreation = arg1 !== 0;
        },
        __wbg_set_mask_6d54863dfb1a561b: function(arg0, arg1) {
            arg0.mask = arg1 >>> 0;
        },
        __wbg_set_min_binding_size_9c38a3535e596945: function(arg0, arg1) {
            arg0.minBindingSize = arg1;
        },
        __wbg_set_mip_level_count_5be5dc9e911edda1: function(arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        },
        __wbg_set_mip_level_count_add44a42f51b21f7: function(arg0, arg1) {
            arg0.mipLevelCount = arg1 >>> 0;
        },
        __wbg_set_module_cef056655b7e27b1: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_module_d0707e6ea19db621: function(arg0, arg1) {
            arg0.module = arg1;
        },
        __wbg_set_multisample_63bc1410fb43c639: function(arg0, arg1) {
            arg0.multisample = arg1;
        },
        __wbg_set_multisampled_52f4b88580119f56: function(arg0, arg1) {
            arg0.multisampled = arg1 !== 0;
        },
        __wbg_set_offset_85286b3f7cf5b516: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_offset_ad470ed410a87932: function(arg0, arg1) {
            arg0.offset = arg1;
        },
        __wbg_set_operation_f787124637b0bfcf: function(arg0, arg1) {
            arg0.operation = __wbindgen_enum_GpuBlendOperation[arg1];
        },
        __wbg_set_pass_op_8e176cc1285b76eb: function(arg0, arg1) {
            arg0.passOp = __wbindgen_enum_GpuStencilOperation[arg1];
        },
        __wbg_set_power_preference_37966403f91dd49f: function(arg0, arg1) {
            arg0.powerPreference = __wbindgen_enum_GpuPowerPreference[arg1];
        },
        __wbg_set_primitive_903955dac70bf43a: function(arg0, arg1) {
            arg0.primitive = arg1;
        },
        __wbg_set_query_set_1ed2d9a5fb6254ed: function(arg0, arg1) {
            arg0.querySet = arg1;
        },
        __wbg_set_r_1df1e4e7089c8620: function(arg0, arg1) {
            arg0.r = arg1;
        },
        __wbg_set_required_features_7df3d73afd128d00: function(arg0, arg1) {
            arg0.requiredFeatures = arg1;
        },
        __wbg_set_resolve_target_d218ff9374745b78: function(arg0, arg1) {
            arg0.resolveTarget = arg1;
        },
        __wbg_set_resource_0d578a8b981e6b76: function(arg0, arg1) {
            arg0.resource = arg1;
        },
        __wbg_set_sample_count_c172b343cf3d0beb: function(arg0, arg1) {
            arg0.sampleCount = arg1 >>> 0;
        },
        __wbg_set_sample_type_7825e985cb9afbfb: function(arg0, arg1) {
            arg0.sampleType = __wbindgen_enum_GpuTextureSampleType[arg1];
        },
        __wbg_set_sampler_a7dc6d753003f3ba: function(arg0, arg1) {
            arg0.sampler = arg1;
        },
        __wbg_set_shader_location_bdd41af4f7a1ce14: function(arg0, arg1) {
            arg0.shaderLocation = arg1 >>> 0;
        },
        __wbg_set_size_9a9f87ca300f7378: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_size_cd7d631653ef3fd8: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_size_e6610cae1a99e379: function(arg0, arg1) {
            arg0.size = arg1;
        },
        __wbg_set_src_factor_1ecefc17b203ae2a: function(arg0, arg1) {
            arg0.srcFactor = __wbindgen_enum_GpuBlendFactor[arg1];
        },
        __wbg_set_stencil_back_9bf1e9c174e72029: function(arg0, arg1) {
            arg0.stencilBack = arg1;
        },
        __wbg_set_stencil_clear_value_3ca279d87d805eaf: function(arg0, arg1) {
            arg0.stencilClearValue = arg1 >>> 0;
        },
        __wbg_set_stencil_front_dff94b71535c2b51: function(arg0, arg1) {
            arg0.stencilFront = arg1;
        },
        __wbg_set_stencil_load_op_186b5d2dcbb2e4c4: function(arg0, arg1) {
            arg0.stencilLoadOp = __wbindgen_enum_GpuLoadOp[arg1];
        },
        __wbg_set_stencil_read_mask_d06407ece83bacf0: function(arg0, arg1) {
            arg0.stencilReadMask = arg1 >>> 0;
        },
        __wbg_set_stencil_read_only_069cf5994323455c: function(arg0, arg1) {
            arg0.stencilReadOnly = arg1 !== 0;
        },
        __wbg_set_stencil_store_op_1d9f3cddd04a45cd: function(arg0, arg1) {
            arg0.stencilStoreOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_stencil_write_mask_e356cc4a4e9ed7a5: function(arg0, arg1) {
            arg0.stencilWriteMask = arg1 >>> 0;
        },
        __wbg_set_step_mode_e89016b848f48dcf: function(arg0, arg1) {
            arg0.stepMode = __wbindgen_enum_GpuVertexStepMode[arg1];
        },
        __wbg_set_storage_texture_e3766fe10229186f: function(arg0, arg1) {
            arg0.storageTexture = arg1;
        },
        __wbg_set_store_op_3400132f94199c14: function(arg0, arg1) {
            arg0.storeOp = __wbindgen_enum_GpuStoreOp[arg1];
        },
        __wbg_set_strip_index_format_c87df8d86831127c: function(arg0, arg1) {
            arg0.stripIndexFormat = __wbindgen_enum_GpuIndexFormat[arg1];
        },
        __wbg_set_targets_818e8cf79855dba2: function(arg0, arg1) {
            arg0.targets = arg1;
        },
        __wbg_set_texture_d6c48b3b60ac8e22: function(arg0, arg1) {
            arg0.texture = arg1;
        },
        __wbg_set_timestamp_writes_2b88ccb5929e7ed0: function(arg0, arg1) {
            arg0.timestampWrites = arg1;
        },
        __wbg_set_topology_f8c64f2f9f18a167: function(arg0, arg1) {
            arg0.topology = __wbindgen_enum_GpuPrimitiveTopology[arg1];
        },
        __wbg_set_type_a4cc3b0194f88912: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuBufferBindingType[arg1];
        },
        __wbg_set_type_ff5cb00f71320dd0: function(arg0, arg1) {
            arg0.type = __wbindgen_enum_GpuSamplerBindingType[arg1];
        },
        __wbg_set_usage_5a616231546f7624: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_a599c7f524cb92f1: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_bb0af4b4f325377f: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_usage_c2ac6c72a4dafcb6: function(arg0, arg1) {
            arg0.usage = arg1 >>> 0;
        },
        __wbg_set_vertex_8646a39b6b4713eb: function(arg0, arg1) {
            arg0.vertex = arg1;
        },
        __wbg_set_view_2286a6551059eaf3: function(arg0, arg1) {
            arg0.view = arg1;
        },
        __wbg_set_view_8c7b97aec99cab68: function(arg0, arg1) {
            arg0.view = arg1;
        },
        __wbg_set_view_dimension_b16c83c333dfb847: function(arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_view_dimension_b56dc785ccf59835: function(arg0, arg1) {
            arg0.viewDimension = __wbindgen_enum_GpuTextureViewDimension[arg1];
        },
        __wbg_set_view_formats_649a861defdbe509: function(arg0, arg1) {
            arg0.viewFormats = arg1;
        },
        __wbg_set_view_formats_bd5c41cebc567c8e: function(arg0, arg1) {
            arg0.viewFormats = arg1;
        },
        __wbg_set_visibility_8ce4060216b4c58b: function(arg0, arg1) {
            arg0.visibility = arg1 >>> 0;
        },
        __wbg_set_width_5f89a551b5472887: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_width_ca327e026b4a017f: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_width_eb3001e1dcabb909: function(arg0, arg1) {
            arg0.width = arg1 >>> 0;
        },
        __wbg_set_write_mask_ee4cc241ef708271: function(arg0, arg1) {
            arg0.writeMask = arg1 >>> 0;
        },
        __wbg_shaderSource_519d0f56b613b2f9: function(arg0, arg1, arg2, arg3) {
            arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
        },
        __wbg_shaderSource_c4c06a3cbc2e7bf3: function(arg0, arg1, arg2, arg3) {
            arg0.shaderSource(arg1, getStringFromWasm0(arg2, arg3));
        },
        __wbg_static_accessor_GLOBAL_THIS_003ee833af9699fc: function() {
            const ret = typeof globalThis === 'undefined' ? undefined : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_e4293fb22cbe77da: function() {
            const ret = typeof global === 'undefined' ? undefined : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_53bb20457401453d: function() {
            const ret = typeof self === 'undefined' ? undefined : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_b5e0810bfecb7feb: function() {
            const ret = typeof window === 'undefined' ? undefined : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_stencilFuncSeparate_bd578fbb298ad0e6: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3, arg4 >>> 0);
        },
        __wbg_stencilFuncSeparate_c4e19ab2f6a233b9: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilFuncSeparate(arg1 >>> 0, arg2 >>> 0, arg3, arg4 >>> 0);
        },
        __wbg_stencilMaskSeparate_41c3f5591b508051: function(arg0, arg1, arg2) {
            arg0.stencilMaskSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_stencilMaskSeparate_b009d9ab29dc5926: function(arg0, arg1, arg2) {
            arg0.stencilMaskSeparate(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_stencilMask_1fe24adb96500c11: function(arg0, arg1) {
            arg0.stencilMask(arg1 >>> 0);
        },
        __wbg_stencilMask_6d5dbf91337de94e: function(arg0, arg1) {
            arg0.stencilMask(arg1 >>> 0);
        },
        __wbg_stencilOpSeparate_9a2068a0767d6507: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilOpSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_stencilOpSeparate_f4319497016868f8: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.stencilOpSeparate(arg1 >>> 0, arg2 >>> 0, arg3 >>> 0, arg4 >>> 0);
        },
        __wbg_submit_2521bdd9a232bca7: function(arg0, arg1) {
            arg0.submit(arg1);
        },
        __wbg_texImage2D_20b5002326193fe6: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texImage2D_5048d2cd8540da16: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texImage2D_84bdef619a1ea935: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texImage3D_5b7fe3781e1db22a: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.texImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8 >>> 0, arg9 >>> 0, arg10);
        }, arguments); },
        __wbg_texImage3D_abf2fdb0f8c4c1b6: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10) {
            arg0.texImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8 >>> 0, arg9 >>> 0, arg10);
        }, arguments); },
        __wbg_texParameteri_0afc5d515132fed2: function(arg0, arg1, arg2, arg3) {
            arg0.texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        },
        __wbg_texParameteri_e24e4fdc17153838: function(arg0, arg1, arg2, arg3) {
            arg0.texParameteri(arg1 >>> 0, arg2 >>> 0, arg3);
        },
        __wbg_texStorage2D_e0ebb5940fa1fc5e: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.texStorage2D(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_texStorage3D_a82e5be79cb278dc: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.texStorage3D(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5, arg6);
        },
        __wbg_texSubImage2D_0fed1a7412bc7a63: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_1434c994889108aa: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_4343af4633b8b8f8: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_4aef07460a86c81e: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_9c7e2d1250b1f95f: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_a26a8700e734449a: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_a271f112c3b60b18: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage2D_baff05c8a8de2e17: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9) {
            arg0.texSubImage2D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7 >>> 0, arg8 >>> 0, arg9);
        }, arguments); },
        __wbg_texSubImage3D_00e908f23d33228d: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_286489de7bf7239a: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_32223c3dd7dcdd78: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_5e8b27dc040faf8d: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_b1b631c3011a209e: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_e8f20815b70bc7cb: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_texSubImage3D_ed8982daddcccd1e: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9, arg10, arg11) {
            arg0.texSubImage3D(arg1 >>> 0, arg2, arg3, arg4, arg5, arg6, arg7, arg8, arg9 >>> 0, arg10 >>> 0, arg11);
        }, arguments); },
        __wbg_then_656f253a5dd7d8ea: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_e3f7f2d365708be8: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_uniform1f_0b36dc15cb2b4991: function(arg0, arg1, arg2) {
            arg0.uniform1f(arg1, arg2);
        },
        __wbg_uniform1f_fe417acc7c7a1dc5: function(arg0, arg1, arg2) {
            arg0.uniform1f(arg1, arg2);
        },
        __wbg_uniform1i_385ebc488d4d763f: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform1i_778a02a48ecdceed: function(arg0, arg1, arg2) {
            arg0.uniform1i(arg1, arg2);
        },
        __wbg_uniform1ui_79071fccd0396b88: function(arg0, arg1, arg2) {
            arg0.uniform1ui(arg1, arg2 >>> 0);
        },
        __wbg_uniform2fv_486658e6bcddfbaf: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2fv_71fe605c72879b61: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2iv_80c1acefe7258756: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2iv_d6efe574c69f5aed: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform2uiv_f9619772d6706c37: function(arg0, arg1, arg2, arg3) {
            arg0.uniform2uiv(arg1, getArrayU32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3fv_431207a9407b7d29: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3fv_58fa948c65cb5f6c: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3iv_11889262cf74698c: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3iv_e9d02a45a4149457: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform3uiv_b71a1165273e4af4: function(arg0, arg1, arg2, arg3) {
            arg0.uniform3uiv(arg1, getArrayU32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4f_14a3638afe93aa8f: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.uniform4f(arg1, arg2, arg3, arg4, arg5);
        },
        __wbg_uniform4f_5c31fb6dab645ad8: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.uniform4f(arg1, arg2, arg3, arg4, arg5);
        },
        __wbg_uniform4fv_a1415c11925ac651: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4fv_d3afea28181f5342: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4fv(arg1, getArrayF32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4iv_c9e8520342091b0a: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4iv_fa18eabf499f4df7: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4iv(arg1, getArrayI32FromWasm0(arg2, arg3));
        },
        __wbg_uniform4uiv_7a5b848aa1084b38: function(arg0, arg1, arg2, arg3) {
            arg0.uniform4uiv(arg1, getArrayU32FromWasm0(arg2, arg3));
        },
        __wbg_uniformBlockBinding_61c5a97721e0f36b: function(arg0, arg1, arg2, arg3) {
            arg0.uniformBlockBinding(arg1, arg2 >>> 0, arg3 >>> 0);
        },
        __wbg_uniformMatrix2fv_0a42d3c91d7cb169: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix2fv_8b3774cc1f2dd2af: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix2x3fv_f9e50997fac55ae7: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2x3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix2x4fv_080d208b4f9c72be: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix2x4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3fv_15b2b1f5505b8506: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3fv_df1958f8b617c8d6: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3x2fv_6730abc8ef13d7d3: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3x2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix3x4fv_3eb72e0d85856655: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix3x4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4fv_cd0ba72d1d780759: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4fv_edd2b51dbb1caffd: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4x2fv_b0c23910facd8e86: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4x2fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_uniformMatrix4x3fv_b6f9b241f392f245: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.uniformMatrix4x3fv(arg1, arg2 !== 0, getArrayF32FromWasm0(arg3, arg4));
        },
        __wbg_useProgram_d9c24a7d8632c4c1: function(arg0, arg1) {
            arg0.useProgram(arg1);
        },
        __wbg_useProgram_f6341ea2eeea1e5a: function(arg0, arg1) {
            arg0.useProgram(arg1);
        },
        __wbg_vertexAttribDivisorANGLE_5bec9a774ccd1cf8: function(arg0, arg1, arg2) {
            arg0.vertexAttribDivisorANGLE(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_vertexAttribDivisor_5b64fc697c0496e1: function(arg0, arg1, arg2) {
            arg0.vertexAttribDivisor(arg1 >>> 0, arg2 >>> 0);
        },
        __wbg_vertexAttribIPointer_a6240b64263fb63b: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.vertexAttribIPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4, arg5);
        },
        __wbg_vertexAttribPointer_39b706eb26c50057: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        },
        __wbg_vertexAttribPointer_9e0e4517b8d274bd: function(arg0, arg1, arg2, arg3, arg4, arg5, arg6) {
            arg0.vertexAttribPointer(arg1 >>> 0, arg2, arg3 >>> 0, arg4 !== 0, arg5, arg6);
        },
        __wbg_viewport_aaac6dd8cff1831a: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.viewport(arg1, arg2, arg3, arg4);
        },
        __wbg_viewport_cdae9e8b2f33343d: function(arg0, arg1, arg2, arg3, arg4) {
            arg0.viewport(arg1, arg2, arg3, arg4);
        },
        __wbg_writeBuffer_84ea7d33c38cd405: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4, arg5) {
            arg0.writeBuffer(arg1, arg2, arg3, arg4, arg5);
        }, arguments); },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Closure(Closure { owned: true, function: Function { arguments: [Externref], shim_idx: 2068, ret: Result(Unit), inner_ret: Some(Result(Unit)) }, mutable: true }) -> Externref`.
            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___wasm_bindgen_70d1b2bcb7a7720f___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_70d1b2bcb7a7720f___JsError___true_);
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0) {
            // Cast intrinsic for `F64 -> Externref`.
            const ret = arg0;
            return ret;
        },
        __wbindgen_generic_0000000000000003: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(F32)) -> NamedExternref("Float32Array")`.
            const ret = getArrayF32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000004: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(I16)) -> NamedExternref("Int16Array")`.
            const ret = getArrayI16FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000005: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(I32)) -> NamedExternref("Int32Array")`.
            const ret = getArrayI32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000006: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(I8)) -> NamedExternref("Int8Array")`.
            const ret = getArrayI8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000007: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U16)) -> NamedExternref("Uint16Array")`.
            const ret = getArrayU16FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000008: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U32)) -> NamedExternref("Uint32Array")`.
            const ret = getArrayU32FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_0000000000000009: function(arg0, arg1) {
            // Cast intrinsic for `Ref(Slice(U8)) -> NamedExternref("Uint8Array")`.
            const ret = getArrayU8FromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_generic_000000000000000a: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./vexio_core_bg.js": import0,
    };
}

function wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___wasm_bindgen_70d1b2bcb7a7720f___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_70d1b2bcb7a7720f___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___wasm_bindgen_70d1b2bcb7a7720f___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_70d1b2bcb7a7720f___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_GpuBlendFactor = ["zero", "one", "src", "one-minus-src", "src-alpha", "one-minus-src-alpha", "dst", "one-minus-dst", "dst-alpha", "one-minus-dst-alpha", "src-alpha-saturated", "constant", "one-minus-constant", "src1", "one-minus-src1", "src1-alpha", "one-minus-src1-alpha"];


const __wbindgen_enum_GpuBlendOperation = ["add", "subtract", "reverse-subtract", "min", "max"];


const __wbindgen_enum_GpuBufferBindingType = ["uniform", "storage", "read-only-storage"];


const __wbindgen_enum_GpuCanvasAlphaMode = ["opaque", "premultiplied"];


const __wbindgen_enum_GpuCompareFunction = ["never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"];


const __wbindgen_enum_GpuCullMode = ["none", "front", "back"];


const __wbindgen_enum_GpuFrontFace = ["ccw", "cw"];


const __wbindgen_enum_GpuIndexFormat = ["uint16", "uint32"];


const __wbindgen_enum_GpuLoadOp = ["load", "clear"];


const __wbindgen_enum_GpuPowerPreference = ["low-power", "high-performance"];


const __wbindgen_enum_GpuPrimitiveTopology = ["point-list", "line-list", "line-strip", "triangle-list", "triangle-strip"];


const __wbindgen_enum_GpuSamplerBindingType = ["filtering", "non-filtering", "comparison"];


const __wbindgen_enum_GpuStencilOperation = ["keep", "zero", "replace", "invert", "increment-clamp", "decrement-clamp", "increment-wrap", "decrement-wrap"];


const __wbindgen_enum_GpuStorageTextureAccess = ["write-only", "read-only", "read-write"];


const __wbindgen_enum_GpuStoreOp = ["store", "discard"];


const __wbindgen_enum_GpuTextureAspect = ["all", "stencil-only", "depth-only"];


const __wbindgen_enum_GpuTextureDimension = ["1d", "2d", "3d"];


const __wbindgen_enum_GpuTextureFormat = ["r8unorm", "r8snorm", "r8uint", "r8sint", "r16uint", "r16sint", "r16float", "rg8unorm", "rg8snorm", "rg8uint", "rg8sint", "r32uint", "r32sint", "r32float", "rg16uint", "rg16sint", "rg16float", "rgba8unorm", "rgba8unorm-srgb", "rgba8snorm", "rgba8uint", "rgba8sint", "bgra8unorm", "bgra8unorm-srgb", "rgb9e5ufloat", "rgb10a2uint", "rgb10a2unorm", "rg11b10ufloat", "rg32uint", "rg32sint", "rg32float", "rgba16uint", "rgba16sint", "rgba16float", "rgba32uint", "rgba32sint", "rgba32float", "stencil8", "depth16unorm", "depth24plus", "depth24plus-stencil8", "depth32float", "depth32float-stencil8", "bc1-rgba-unorm", "bc1-rgba-unorm-srgb", "bc2-rgba-unorm", "bc2-rgba-unorm-srgb", "bc3-rgba-unorm", "bc3-rgba-unorm-srgb", "bc4-r-unorm", "bc4-r-snorm", "bc5-rg-unorm", "bc5-rg-snorm", "bc6h-rgb-ufloat", "bc6h-rgb-float", "bc7-rgba-unorm", "bc7-rgba-unorm-srgb", "etc2-rgb8unorm", "etc2-rgb8unorm-srgb", "etc2-rgb8a1unorm", "etc2-rgb8a1unorm-srgb", "etc2-rgba8unorm", "etc2-rgba8unorm-srgb", "eac-r11unorm", "eac-r11snorm", "eac-rg11unorm", "eac-rg11snorm", "astc-4x4-unorm", "astc-4x4-unorm-srgb", "astc-5x4-unorm", "astc-5x4-unorm-srgb", "astc-5x5-unorm", "astc-5x5-unorm-srgb", "astc-6x5-unorm", "astc-6x5-unorm-srgb", "astc-6x6-unorm", "astc-6x6-unorm-srgb", "astc-8x5-unorm", "astc-8x5-unorm-srgb", "astc-8x6-unorm", "astc-8x6-unorm-srgb", "astc-8x8-unorm", "astc-8x8-unorm-srgb", "astc-10x5-unorm", "astc-10x5-unorm-srgb", "astc-10x6-unorm", "astc-10x6-unorm-srgb", "astc-10x8-unorm", "astc-10x8-unorm-srgb", "astc-10x10-unorm", "astc-10x10-unorm-srgb", "astc-12x10-unorm", "astc-12x10-unorm-srgb", "astc-12x12-unorm", "astc-12x12-unorm-srgb"];


const __wbindgen_enum_GpuTextureSampleType = ["float", "unfilterable-float", "depth", "sint", "uint"];


const __wbindgen_enum_GpuTextureViewDimension = ["1d", "2d", "2d-array", "cube", "cube-array", "3d"];


const __wbindgen_enum_GpuVertexFormat = ["uint8", "uint8x2", "uint8x4", "sint8", "sint8x2", "sint8x4", "unorm8", "unorm8x2", "unorm8x4", "snorm8", "snorm8x2", "snorm8x4", "uint16", "uint16x2", "uint16x4", "sint16", "sint16x2", "sint16x4", "unorm16", "unorm16x2", "unorm16x4", "snorm16", "snorm16x2", "snorm16x4", "float16", "float16x2", "float16x4", "float32", "float32x2", "float32x3", "float32x4", "uint32", "uint32x2", "uint32x3", "uint32x4", "sint32", "sint32x2", "sint32x3", "sint32x4", "unorm10-10-10-2", "unorm8x4-bgra"];


const __wbindgen_enum_GpuVertexStepMode = ["vertex", "instance"];
const WasmVexioSessionFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_wasmvexiosession_free(ptr, 1));

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    switch (typeof val) {
        // primitive types
        case 'number':
        case 'boolean':
        case 'undefined':
        return `${val}`;
        case 'string':
        return JSON.stringify(val);
        case 'bigint':
        return `${val}n`;
        case 'symbol':
        return val.description !== undefined
            ? `Symbol(${val.description})`
            : 'Symbol';
        case 'function':
        return typeof val.name === 'string' && val.name !== ''
            ? `Function(${val.name})`
            : 'Function';
        case 'object': {
            if (val === null) {
                return 'null';
            }
            // objects
            if (Array.isArray(val)) {
                return '[' + Array.from(val, debugString).join(', ') + ']';
            }
            // errors
            if (val instanceof Error) {
                return `${val.name}: ${val.message}\n${val.stack}`;
            }

            const toStringOutput = toString.call(val);
            // Test for built-in
            const className = /\[object ([^\]]+)\]/.exec(toStringOutput)?.[1];

            // Failed to match the standard '[object ClassName]'
            if (className === undefined) {
                return toStringOutput;
            }
            if (className === 'Object') {
                // we're a user defined class or Object
                // JSON.stringify avoids problems with cycles, and is generally much
                // easier than looping through ownProperties of `val`.
                try {
                    return 'Object(' + JSON.stringify(val) + ')';
                } catch (_) {
                    return 'Object';
                }
            }
            // TODO we could test for more things here, like `Set`s and `Map`s.
            return className;
        }
        // Per MDN: 'This list of values is exhaustive.'
        // https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Operators/typeof#description
    }
}

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayI16FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}

function getArrayI32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayI8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getInt8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

function getArrayU16FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint16ArrayMemory0().subarray(ptr / 2, ptr / 2 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

let cachedInt16ArrayMemory0 = null;
function getInt16ArrayMemory0() {
    if (cachedInt16ArrayMemory0 === null || cachedInt16ArrayMemory0.byteLength === 0) {
        cachedInt16ArrayMemory0 = new Int16Array(wasm.memory.buffer);
    }
    return cachedInt16ArrayMemory0;
}

let cachedInt32ArrayMemory0 = null;
function getInt32ArrayMemory0() {
    if (cachedInt32ArrayMemory0 === null || cachedInt32ArrayMemory0.byteLength === 0) {
        cachedInt32ArrayMemory0 = new Int32Array(wasm.memory.buffer);
    }
    return cachedInt32ArrayMemory0;
}

let cachedInt8ArrayMemory0 = null;
function getInt8ArrayMemory0() {
    if (cachedInt8ArrayMemory0 === null || cachedInt8ArrayMemory0.byteLength === 0) {
        cachedInt8ArrayMemory0 = new Int8Array(wasm.memory.buffer);
    }
    return cachedInt8ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint16ArrayMemory0 = null;
function getUint16ArrayMemory0() {
    if (cachedUint16ArrayMemory0 === null || cachedUint16ArrayMemory0.byteLength === 0) {
        cachedUint16ArrayMemory0 = new Uint16Array(wasm.memory.buffer);
    }
    return cachedUint16ArrayMemory0;
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {

        // First up with a closure we increment the internal reference
        // count. This ensures that the Rust closure environment won't
        // be deallocated while we're invoking it.
        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArray32ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 4, 4) >>> 0;
    getUint32ArrayMemory0().set(arg, ptr / 4);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedFloat32ArrayMemory0 = null;
    cachedInt16ArrayMemory0 = null;
    cachedInt32ArrayMemory0 = null;
    cachedInt8ArrayMemory0 = null;
    cachedUint16ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('vexio_core_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
