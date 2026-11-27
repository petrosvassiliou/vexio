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
VexioStatus vexio_last_error_message(VexioOwnedBuffer* out_utf8);

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
VexioStatus vexio_doc_transform_shape(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, double pin_x_in, double pin_y_in, double width_in, double height_in, double angle_deg);
VexioStatus vexio_doc_import_stencil_bytes(VexioDoc* doc, const char* file_name_utf8, VexioByteSlice bytes, VexioOwnedBuffer* out_json);
VexioStatus vexio_list_official_stencil_catalog_json(VexioOwnedBuffer* out_json);
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

/* Viewport Inline Editing, Active Tool Sync & Screen Bounds */
uint32_t    vexio_viewport_get_active_tool(const VexioViewport* vp);
uint64_t    vexio_viewport_pop_created_text_shape(VexioViewport* vp);
uint64_t    vexio_viewport_hit_test_shape(const VexioViewport* vp, const VexioDoc* doc, double x_px, double y_px);
VexioStatus vexio_viewport_get_shape_screen_bounds(const VexioViewport* vp, const VexioDoc* doc, uint64_t shape_id, double* out_x_px, double* out_y_px, double* out_w_px, double* out_h_px);
VexioStatus vexio_viewport_screen_to_page(const VexioViewport* vp, double x_px, double y_px, double* out_x_in, double* out_y_in);

/* Rich Text Formatting, Stencil Rotate/Flip, ISO Metric Page Setup & Rename */
VexioStatus vexio_doc_set_shape_text_style_json(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* style_json_utf8);
VexioStatus vexio_doc_rotate_flip_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, double delta_angle_deg, bool toggle_flip_x, bool toggle_flip_y);
VexioStatus vexio_doc_set_page_setup(VexioDoc* doc, uint32_t page_id, double width_in, double height_in, const char* unit_slug_utf8);
VexioStatus vexio_doc_set_page_preset(VexioDoc* doc, uint32_t page_id, const char* preset_slug_utf8, bool landscape);
VexioStatus vexio_doc_set_title(VexioDoc* doc, const char* title_utf8);
VexioStatus vexio_doc_rename_page(VexioDoc* doc, uint32_t page_id, const char* new_name_utf8);

/* v0.2.0-alpha: Custom TrueType/OpenType Fonts, Align/Distribute, Group/Ungroup, Smart Guides & 1D Segment Drag */
VexioStatus vexio_register_font_bytes(VexioByteSlice font_bytes, const char* family_override_utf8);
VexioStatus vexio_doc_align_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* mode_utf8, uint32_t* out_count);
VexioStatus vexio_doc_distribute_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* axis_utf8, uint32_t* out_count);
VexioStatus vexio_doc_group_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, uint64_t* out_group_id);
VexioStatus vexio_doc_ungroup_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* group_ids_ptr, size_t group_ids_len, VexioOwnedBuffer* out_ids_json);
VexioStatus vexio_doc_drag_connector_segment(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t seg_index, double coord_in);
VexioStatus vexio_viewport_set_smart_guides(VexioViewport* vp, bool enabled);
bool        vexio_viewport_get_smart_guides(const VexioViewport* vp);
VexioStatus vexio_viewport_get_selection_json(const VexioViewport* vp, VexioOwnedBuffer* out_json);
VexioStatus vexio_viewport_set_selection(VexioViewport* vp, const VexioDoc* doc, const uint64_t* shape_ids_ptr, size_t shape_ids_len);

/* v0.3.0-alpha: Interactive Containers, Swimlanes, Callouts, Yellow Control Handles & AutoConnect */
VexioStatus vexio_doc_insert_container(VexioDoc* doc, uint32_t page_id, const char* title_utf8, const uint64_t* member_ids_ptr, size_t member_ids_len, uint64_t* out_container_id);
VexioStatus vexio_doc_insert_swimlanes(VexioDoc* doc, uint32_t page_id, const char* lanes_semicolon_utf8, uint32_t* out_lane_count);
VexioStatus vexio_doc_attach_callout(VexioDoc* doc, uint32_t page_id, uint64_t target_shape_id, const char* text_utf8, uint64_t* out_callout_id);
VexioStatus vexio_doc_auto_connect(VexioDoc* doc, uint32_t page_id, uint64_t source_shape_id, const char* dir_utf8, uint64_t* out_target_shape_id, uint64_t* out_conn_id);
VexioStatus vexio_doc_drag_control_handle(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* row_name_utf8, double page_x_in, double page_y_in);

/* v0.3.8-alpha: 1:1 Microsoft Visio Object Inspector (Object Rename, Shape Data Prop.*, Layer Properties & Membership) */
VexioStatus vexio_doc_rename_shape(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* new_name_utf8);
VexioStatus vexio_doc_upsert_shape_data(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* row_name_utf8, const char* label_utf8, uint8_t data_type, const char* value_utf8);
VexioStatus vexio_doc_delete_shape_data(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* row_name_utf8);
VexioStatus vexio_doc_add_layer(VexioDoc* doc, uint32_t page_id, const char* layer_name_utf8, uint32_t* out_layer_ix);
VexioStatus vexio_doc_set_layer_flags(VexioDoc* doc, uint32_t page_id, uint32_t layer_ix, bool visible, bool lock_flag, bool print_flag, bool snap_glue);
VexioStatus vexio_doc_set_shape_layer_membership(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t layer_ix, bool assigned);

/* Pillar #4: Live Data Graphics, Auto-Updating Legends & Two-Way External Data Linking (.xlsx / .csv / .sqlite / .json) */
VexioStatus vexio_doc_get_data_link_state_json(const VexioDoc* doc, uint32_t page_id, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_set_page_data_graphics_json(VexioDoc* doc, uint32_t page_id, const char* config_json_utf8, uint32_t* out_decorated_count);
VexioStatus vexio_doc_import_external_recordset(VexioDoc* doc, uint32_t page_id, const char* file_name_utf8, VexioByteSlice data_bytes, const char* key_col_utf8, const char* table_name_utf8, bool auto_bind, bool auto_enable_graphics, uint32_t* out_recordset_id, uint32_t* out_linked_count);
VexioStatus vexio_doc_bind_external_row(VexioDoc* doc, uint32_t page_id, uint32_t recordset_id, uint32_t row_index, const uint64_t* shape_ids_ptr, size_t shape_ids_len, uint32_t* out_bound_count);
VexioStatus vexio_doc_refresh_external_recordset(VexioDoc* doc, uint32_t page_id, uint32_t recordset_id, VexioByteSlice optional_new_bytes, const char* format_hint_utf8, uint32_t* out_refreshed_count);
VexioStatus vexio_doc_sync_and_export_recordset(VexioDoc* doc, uint32_t page_id, uint32_t recordset_id, const char* format_ext_utf8, VexioOwnedBuffer* out_bytes);
VexioStatus vexio_doc_delete_external_recordset(VexioDoc* doc, uint32_t recordset_id);
VexioStatus vexio_doc_insert_data_legend(VexioDoc* doc, uint32_t page_id, const char* prop_name_utf8, double pin_x_in, double pin_y_in, uint64_t* out_legend_id);
VexioStatus vexio_doc_export_bom_report(const VexioDoc* doc, const char* format_ext_utf8, VexioOwnedBuffer* out_bytes);

/* v0.4.0-alpha: Page Duplicate, Delete & Background Page / Paper Color Management */
VexioStatus vexio_doc_duplicate_page(VexioDoc* doc, uint32_t page_id, uint32_t* out_new_page_id);
VexioStatus vexio_doc_delete_page(VexioDoc* doc, uint32_t page_id);
VexioStatus vexio_doc_set_page_background(VexioDoc* doc, uint32_t page_id, bool is_background, uint32_t back_page_id, const char* page_color_formula_utf8);

/* v0.4.2-alpha: Visio Text Headings, Titles & Design Borders/Titles Gallery */
VexioStatus vexio_doc_apply_heading_preset(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* preset_slug_utf8, uint32_t* out_modified_count);
VexioStatus vexio_doc_insert_heading(VexioDoc* doc, uint32_t page_id, const char* preset_slug_utf8, const char* custom_text_utf8, double x_in, double y_in, uint64_t* out_shape_id);
VexioStatus vexio_doc_apply_border_and_title(VexioDoc* doc, uint32_t page_id, const char* style_slug_utf8, const char* title_utf8, const char* subtitle_utf8, const char* author_utf8, const char* date_or_rev_utf8, uint32_t* out_bg_page_id);

/* v0.5.0-beta: Diagram-as-Code (Mermaid / PlantUML / Graphviz DOT), Presentation Slide Snippets & Multi-Bend Routing */
VexioStatus vexio_doc_import_markup(VexioDoc* doc, uint32_t page_id, const char* markup_utf8, const char* format_slug_utf8, uint32_t* out_nodes_created);
VexioStatus vexio_doc_export_markup(const VexioDoc* doc, uint32_t page_id, const char* format_slug_utf8, VexioOwnedBuffer* out_markup_utf8);
VexioStatus vexio_doc_list_slide_snippets_json(const VexioDoc* doc, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_add_slide_snippet(VexioDoc* doc, uint32_t page_id, const char* title_utf8, const char* notes_utf8, double x_in, double y_in, double width_in, double height_in, uint32_t* out_snippet_id);
VexioStatus vexio_doc_delete_slide_snippet(VexioDoc* doc, uint32_t page_id, uint32_t snippet_id);
VexioStatus vexio_doc_auto_slide_snippets(VexioDoc* doc, uint32_t page_id, uint32_t* out_count);
VexioStatus vexio_doc_reset_connector_routing(VexioDoc* doc, uint32_t page_id, uint64_t shape_id);
VexioStatus vexio_doc_ensure_control_handle(VexioDoc* doc, uint32_t page_id, uint64_t shape_id);

/* v0.6.0-beta: VBA Macro IDE, Vertex/Freeform Editing, Threaded Comments & Ink, Hyperlinks/ScreenTips, Domain Wizards & CAD Tools */
VexioStatus vexio_doc_run_vba_macro(VexioDoc* doc, const uint64_t* selection_ptr, size_t selection_len, const char* script_utf8, VexioOwnedBuffer* out_report_json);
VexioStatus vexio_doc_list_vertices_json(const VexioDoc* doc, uint32_t page_id, uint64_t shape_id, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_move_vertex(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t geom_index, uint32_t row_ix, double page_x_in, double page_y_in);
VexioStatus vexio_doc_insert_vertex(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t geom_index, uint32_t after_row_ix, double page_x_in, double page_y_in);
VexioStatus vexio_doc_delete_vertex(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t geom_index, uint32_t row_ix);
VexioStatus vexio_doc_draw_freeform_json(VexioDoc* doc, uint32_t page_id, const char* points_json_utf8, bool closed, uint64_t* out_shape_id);
VexioStatus vexio_doc_list_comments_json(const VexioDoc* doc, uint32_t page_id, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_add_comment(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* author_utf8, const char* text_utf8, double x_in, double y_in, uint32_t* out_comment_id);
VexioStatus vexio_doc_resolve_comment(VexioDoc* doc, uint32_t comment_id, bool resolved);
VexioStatus vexio_doc_delete_comment(VexioDoc* doc, uint32_t comment_id);
VexioStatus vexio_doc_clear_page_ink(VexioDoc* doc, uint32_t page_id, uint32_t* out_removed_count);
VexioStatus vexio_doc_set_hyperlink_and_screentip(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* description_utf8, const char* address_utf8, const char* sub_address_utf8, const char* screentip_utf8);
VexioStatus vexio_doc_run_domain_wizard(VexioDoc* doc, uint32_t page_id, const char* wizard_slug_utf8, const char* spec_utf8, VexioOwnedBuffer* out_result_json);
VexioStatus vexio_doc_apply_cad_scale(VexioDoc* doc, uint32_t page_id, const char* scale_slug_utf8, bool locked, double* out_ratio);
VexioStatus vexio_doc_convert_cad_layers(VexioDoc* doc, uint32_t page_id, const char* layer_name_or_all_utf8, uint32_t* out_converted_count);

/* v0.7.0-beta: 238-Function ShapeSheet Catalog, Smart-Shape Actions.*, Shift+Ctrl+1 Connection Points & "My Shapes" Custom .vssx Stencil Authoring */
VexioStatus vexio_list_shapesheet_functions_json(VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_list_shape_actions_json(const VexioDoc* doc, uint32_t page_id, uint64_t shape_id, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_upsert_shape_action(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* row_name_utf8, const char* menu_utf8, const char* action_formula_utf8, const char* checked_formula_utf8, const char* disabled_formula_utf8, bool begin_group);
VexioStatus vexio_doc_delete_shape_action(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* row_name_utf8);
VexioStatus vexio_doc_execute_shape_action(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, const char* row_name_utf8, VexioOwnedBuffer* out_result_json);
VexioStatus vexio_doc_list_connection_points_json(const VexioDoc* doc, uint32_t page_id, uint64_t shape_id, VexioOwnedBuffer* out_json);
VexioStatus vexio_doc_add_connection_point(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, double page_x_in, double page_y_in, uint8_t point_type, uint32_t* out_ix);
VexioStatus vexio_doc_move_connection_point(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t ix, double page_x_in, double page_y_in);
VexioStatus vexio_doc_delete_connection_point(VexioDoc* doc, uint32_t page_id, uint64_t shape_id, uint32_t ix);
VexioStatus vexio_doc_add_to_custom_stencil(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids_ptr, size_t shape_ids_len, const char* stencil_title_utf8, const char* master_name_utf8, VexioOwnedBuffer* out_result_json);
VexioStatus vexio_doc_rename_custom_master(VexioDoc* doc, const char* stencil_title_utf8, const char* old_name_utf8, const char* new_name_utf8);
VexioStatus vexio_doc_delete_custom_master(VexioDoc* doc, const char* stencil_title_utf8, const char* master_name_utf8);
VexioStatus vexio_doc_update_custom_master(VexioDoc* doc, uint32_t page_id, uint64_t source_shape_id, const char* stencil_title_utf8, const char* master_name_utf8, uint32_t* out_updated_count);
VexioStatus vexio_doc_export_custom_stencil_vssx(const VexioDoc* doc, const char* stencil_title_utf8, VexioOwnedBuffer* out_vssx_bytes);

#ifdef __cplusplus
}
#endif
