#pragma once
#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct VexioEngine   VexioEngine;
typedef struct VexioDoc      VexioDoc;
typedef struct VexioViewport VexioViewport;

typedef enum VexioStatus {
    VEXIO_OK                    = 0,
    VEXIO_ERR_INVALID_ARG       = 1,
    VEXIO_ERR_CELL_GUARDED      = 2,
    VEXIO_ERR_FORMULA_SYNTAX    = 3,
    VEXIO_ERR_IO_CORRUPT_FORMAT = 4,
    VEXIO_ERR_GPU_SURFACE_LOST  = 5,
    VEXIO_ERR_INTERNAL_PANIC    = 99,
} VexioStatus;

typedef struct VexioByteSlice {
    const uint8_t* ptr;
    size_t         len;
} VexioByteSlice;

typedef struct VexioOwnedBuffer {
    uint8_t* ptr;
    size_t   len;
    size_t   cap;
} VexioOwnedBuffer;

typedef struct VexioPointerEvent {
    uint32_t kind;        /* 0=Move, 1=Down, 2=Up, 3=Wheel, 4=StylusMove */
    uint32_t buttons;     /* Bitmask: Primary=1, Secondary=2, Middle=4, Eraser=32 */
    uint32_t modifiers;   /* Shift=1, Ctrl=2, Alt=4, Meta=8 */
    double   x_px;
    double   y_px;
    float    pressure;    /* [0.0, 1.0] */
    float    tilt_x_deg;
    float    tilt_y_deg;
    uint64_t timestamp_us;
} VexioPointerEvent;

/* Engine Lifecycle */
VexioEngine* vexio_engine_create(void);
void         vexio_engine_destroy(VexioEngine* engine);

/* Document Open / Create / Save (.vsdx, .vsdm, .vssx, .vstx, .vsd, .vdx, .dwg, .dxf, .pdf, .svg) */
VexioDoc*   vexio_doc_create_blank(const char* title_utf8);
VexioStatus vexio_doc_open_bytes(VexioEngine* engine, VexioByteSlice bytes, const char* ext_utf8, VexioDoc** out_doc);
VexioStatus vexio_doc_save_format(VexioDoc* doc, const char* ext_utf8, VexioOwnedBuffer* out_buf);
void        vexio_doc_destroy(VexioDoc* doc);
void        vexio_buffer_free(VexioOwnedBuffer buf);

/* ShapeSheet Cell Inspection & Reactive Formula Mutation */
VexioStatus vexio_cell_get_formula(const VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* cell_name, VexioOwnedBuffer* out_utf8);
VexioStatus vexio_cell_get_value_str(const VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* cell_name, VexioOwnedBuffer* out_utf8);
VexioStatus vexio_cell_set_formula(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* cell_name, const char* formula_utf8, bool force_setf);

/* Viewport, DX12 / Metal Surface Attachment, Input & Offscreen Preview */
VexioStatus vexio_viewport_create_headless(VexioEngine* engine, VexioDoc* doc, uint32_t page_id, uint32_t width_px, uint32_t height_px, float dpi_scale, VexioViewport** out_viewport);
VexioStatus vexio_viewport_create_dx12_swapchain(VexioEngine* engine, VexioDoc* doc, void* dxgi_swapchain_panel_unk, uint32_t width_px, uint32_t height_px, float dpi_scale, VexioViewport** out_viewport);
VexioStatus vexio_viewport_create_metal_layer(VexioEngine* engine, VexioDoc* doc, void* ca_metal_layer_ptr, uint32_t width_px, uint32_t height_px, float dpi_scale, VexioViewport** out_viewport);
void        vexio_viewport_destroy(VexioViewport* vp);
VexioStatus vexio_viewport_resize(VexioViewport* vp, uint32_t width_px, uint32_t height_px, float dpi_scale);
VexioStatus vexio_viewport_set_tool(VexioViewport* vp, uint32_t tool_code);
VexioStatus vexio_viewport_set_page(VexioViewport* vp, const VexioDoc* doc, uint32_t page_id);
VexioStatus vexio_viewport_set_zoom(VexioViewport* vp, const VexioDoc* doc, double zoom_scale, double pan_x_in, double pan_y_in);
VexioStatus vexio_viewport_render_frame(VexioViewport* vp, const VexioDoc* doc);
VexioStatus vexio_viewport_render_rgba(const VexioViewport* vp, const VexioDoc* doc, VexioOwnedBuffer* out_rgba);
uint64_t    vexio_viewport_get_primary_selection(const VexioViewport* vp);
VexioStatus vexio_viewport_dispatch_pointer(VexioViewport* vp, VexioDoc* doc, const VexioPointerEvent* ev, uint32_t* out_cursor_kind);
VexioStatus vexio_render_preview_rgba(const VexioDoc* doc, uint32_t page_id, uint32_t width_px, uint32_t height_px, VexioOwnedBuffer* out_rgba);

/* High-Definition Built-In Stencil Library, Starter Templates, Authoring & Validation */
VexioStatus vexio_doc_list_stencils_json(const VexioDoc* doc, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_load_starter_template(VexioDoc* doc, const char* template_id_utf8);
VexioStatus vexio_doc_get_state_json(const VexioDoc* doc, uint32_t active_page_id, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_drop_stencil(VexioDoc* doc, uint32_t page_id, const char* category_utf8, const char* master_name_utf8, double x_in, double y_in, uint64_t* out_shape_id);
VexioStatus vexio_doc_draw_primitive(VexioDoc* doc, uint32_t page_id, const char* kind_utf8, double x1_in, double y1_in, double x2_in, double y2_in, uint64_t* out_shape_id);
VexioStatus vexio_doc_connect_shapes(VexioDoc* doc, uint32_t page_id, uint64_t from_shape_id, uint64_t to_shape_id, uint64_t* out_conn_id);
VexioStatus vexio_doc_delete_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, uint32_t* out_deleted_count);
VexioStatus vexio_doc_duplicate_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, double dx_in, double dy_in, uint64_t* out_first_id);
VexioStatus vexio_doc_reorder_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* mode_utf8);
VexioStatus vexio_doc_set_shape_text(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* text_utf8);
VexioStatus vexio_doc_add_page(VexioDoc* doc, const char* name_utf8, uint32_t* out_page_id);
VexioStatus vexio_doc_auto_layout(VexioDoc* doc, uint32_t page_id, const char* algo_utf8);
VexioStatus vexio_doc_validate_json(const VexioDoc* doc, const char* ruleset_utf8, VexioOwnedBuffer* out_json);

/* Deterministic Command Journal Undo / Redo */
VexioStatus vexio_doc_undo(VexioDoc* doc);
VexioStatus vexio_doc_redo(VexioDoc* doc);

/* Phase 7: Boolean Shape Ops, Themes/Variants, Visual Presets, Subprocess, Compare, Plan 2 Connectors & CRDT */
VexioStatus vexio_doc_boolean_op(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* op_utf8, double offset_in);
VexioStatus vexio_doc_apply_theme(VexioDoc* doc, const char* theme_slug_utf8, uint8_t variant_index);
VexioStatus vexio_doc_apply_visual_preset(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* preset_utf8);
VexioStatus vexio_doc_create_subprocess(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* sub_name_utf8, uint32_t* out_sub_page_id);
VexioStatus vexio_doc_compare(const VexioDoc* base_doc, const VexioDoc* rev_doc, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_apply_crdt_json(VexioDoc* doc, const char* replica_id_utf8, const char* ops_json_utf8);
VexioStatus vexio_doc_flush_crdt_json(const VexioDoc* prev_doc, const VexioDoc* curr_doc, const char* replica_id_utf8, uint64_t now_ms, VexioOwnedBuffer* out_ops_json);
VexioStatus vexio_doc_import_data_visualizer(VexioDoc* doc, uint32_t page_id, VexioByteSlice csv_or_xlsx, uint32_t* out_step_count);
VexioStatus vexio_doc_export_data_visualizer_xlsx(const VexioDoc* doc, uint32_t page_id, VexioOwnedBuffer* out_xlsx);
VexioStatus vexio_doc_import_cloud_discovery(VexioDoc* doc, uint32_t page_id, const char* discovery_json_utf8, uint32_t* out_count);
VexioStatus vexio_doc_import_entra_or_ldif(VexioDoc* doc, uint32_t page_id, const char* text_utf8, uint32_t* out_count);

#ifdef __cplusplus
}
#endif

