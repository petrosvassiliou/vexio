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
VexioStatus vexio_viewport_dispatch_pointer(VexioViewport* vp, VexioDoc* doc, const VexioPointerEvent* ev, uint32_t* out_cursor_kind);
VexioStatus vexio_render_preview_rgba(const VexioDoc* doc, uint32_t page_id, uint32_t width_px, uint32_t height_px, VexioOwnedBuffer* out_rgba);

/* Deterministic Command Journal Undo / Redo */
VexioStatus vexio_doc_undo(VexioDoc* doc);
VexioStatus vexio_doc_redo(VexioDoc* doc);

#ifdef __cplusplus
}
#endif
