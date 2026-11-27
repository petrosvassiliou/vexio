/* tslint:disable */
/* eslint-disable */

export class WasmVexioSession {
    free(): void;
    [Symbol.dispose](): void;
    add_comment(page_id: number, shape_id: number, author: string, text: string): number;
    add_connection_point(page_id: number, shape_id: number, page_x_in: number, page_y_in: number, point_type: number): number;
    add_ink_stroke(page_id: number, color_hex: string, width_pt: number, highlighter: boolean, points_json: string): number;
    add_layer(page_id: number, name: string): number;
    add_page(name: string): number;
    add_ruler_guide(page_id: number, is_vertical: boolean, coord_in: number): number;
    add_slide_snippet(page_id: number, title: string, notes: string, x_in: number, y_in: number, width_in: number, height_in: number): number;
    add_to_custom_stencil(page_id: number, shape_ids: Uint32Array | number[], stencil_title: string, master_name: string): string;
    /**
     * Multi-shape Align (`left`, `center`, `right`, `top`, `middle`, `bottom`) and
     * Distribute (`distribute_h`, `distribute_v`) matching Visio Home -> Position!
     */
    align_shapes(page_id: number, shape_ids: Uint32Array | number[], mode: string): number;
    apply_auto_layout(page_id: number, algo: string): void;
    apply_boolean_op(page_id: number, shape_ids: Uint32Array | number[], op: string, offset_in: number): Uint32Array;
    apply_border_and_title(page_id: number, style_slug: string, title: string, subtitle: string, author: string, date_or_rev: string): number;
    apply_cad_scale(page_id: number, scale_slug: string, locked: boolean): number;
    apply_format_painter(page_id: number, target_ids: Uint32Array | number[], format_json: string): number;
    apply_heading_preset(page_id: number, shape_ids: Uint32Array | number[], preset_slug: string): number;
    apply_remote_crdt_ops_json(ops_json: string): number;
    apply_shape_visual_preset(page_id: number, shape_id: number, preset: string): boolean;
    apply_text_style_json(page_id: number, shape_ids: Uint32Array | number[], style_json: string): number;
    apply_theme_variant(theme_slug: string, variant_index: number): number;
    apply_title_block_border(page_id: number, project_code: string, author: string, revision: string): number;
    attach_offscreen_canvas(canvas: OffscreenCanvas, width: number, height: number, dpi: number): Promise<boolean>;
    auto_connect(page_id: number, source_shape_id: number, dir: string): number;
    auto_generate_slide_snippets(page_id: number): number;
    bind_external_row(page_id: number, recordset_id: number, row_index: number, shape_ids: Uint32Array | number[]): number;
    clear_page_ink(page_id: number): number;
    compare_with_vsdx_bytes(baseline_vsdx: Uint8Array | number[]): string;
    /**
     * Compute Dynamic Grid Smart Alignment & Equal-Spacing Guides (center-X, center-Y, left/right/top/bottom edges,
     * page center, and equal gaps across 3+ shapes) within `0.10 in` magnetic threshold.
     */
    compute_smart_guides(page_id: number, moving_shape_id: number, cand_x_in: number, cand_y_in: number, width_in: number, height_in: number): string;
    connect_points_or_shapes(page_id: number, x1_in: number, y1_in: number, x2_in: number, y2_in: number, from_shape_id: number, to_shape_id: number): number;
    connect_shapes(page_id: number, from_id: number, to_id: number): number;
    convert_cad_layers(page_id: number, layer_name_or_all: string): number;
    copy_shape_format_json(page_id: number, shape_id: number): string;
    create_subprocess_drilldown(page_id: number, shape_id: number, sub_name: string): number;
    delete_comment(comment_id: number): boolean;
    delete_connection_point(page_id: number, shape_id: number, ix: number): boolean;
    delete_custom_master(stencil_title: string, master_name: string): boolean;
    delete_external_recordset(recordset_id: number): boolean;
    delete_page(page_id: number): boolean;
    delete_ruler_guide(page_id: number, guide_id: number): boolean;
    delete_shape(page_id: number, shape_id: number): boolean;
    delete_shape_action(page_id: number, shape_id: number, row_name: string): boolean;
    delete_shape_data(page_id: number, shape_id: number, row_name: string): boolean;
    delete_shape_vertex(page_id: number, shape_id: number, geom_index: number, row_ix: number): boolean;
    delete_slide_snippet(page_id: number, snippet_id: number): boolean;
    distribute_shapes(page_id: number, shape_ids: Uint32Array | number[], axis: string): number;
    drag_connector_segment(page_id: number, shape_id: number, seg_index: number, coord_in: number): boolean;
    draw_arc(page_id: number, x1_in: number, y1_in: number, x2_in: number, y2_in: number, bow_in: number): number;
    draw_freeform_curve(page_id: number, points_json: string, closed: boolean): number;
    draw_line(page_id: number, x1_in: number, y1_in: number, x2_in: number, y2_in: number): number;
    draw_primitive_2d(page_id: number, kind: string, x1_in: number, y1_in: number, x2_in: number, y2_in: number): number;
    draw_rectangle(page_id: number, x1_in: number, y1_in: number, x2_in: number, y2_in: number): number;
    drop_builtin_shape(category: string, master_name: string, x_in: number, y_in: number): number;
    drop_builtin_shape_on_page(page_id: number, category: string, master_name: string, x_in: number, y_in: number): number;
    duplicate_page(page_id: number): number;
    duplicate_shapes(page_id: number, shape_ids: Uint32Array | number[], dx_in: number, dy_in: number): Uint32Array;
    ensure_control_handle(page_id: number, shape_id: number): boolean;
    execute_shape_action(page_id: number, shape_id: number, row_name: string): string;
    export_bom_report_bytes(format_ext: string): Uint8Array;
    export_bom_xlsx(): Uint8Array;
    export_custom_stencil_vssx(stencil_title: string): Uint8Array;
    export_data_visualizer_workbook(page_id: number): Uint8Array;
    export_diagram_markup(page_id: number, format_slug: string): string;
    export_wal(): Uint8Array;
    find_in_document_json(active_page_id: number, query: string, match_case: boolean, whole_word: boolean, all_pages: boolean): string;
    flush_crdt_ops_json(now_ms: number): string;
    forward_engineer_sql(page_id: number): string;
    generate_gantt_demo(page_id: number): number;
    generate_pert_demo(page_id: number): number;
    generate_timeline_demo(page_id: number): number;
    get_page_data_link_state_json(page_id: number): string;
    get_snap_glue_json(page_id: number): string;
    get_state_json(active_page_id: number): string;
    get_text_block_overlay_json(page_id: number, shape_id: number, zoom_ppi: number): string;
    group_shapes(page_id: number, shape_ids: Uint32Array | number[]): number;
    hit_test_page(page_id: number, x_in: number, y_in: number): number;
    import_cloud_discovery(page_id: number, discovery_json: string): number;
    import_data_visualizer(page_id: number, bytes: Uint8Array | number[]): number;
    import_diagram_markup(page_id: number, markup: string, format_slug: string): string;
    import_entra_or_ldif(page_id: number, text: string): number;
    import_external_recordset_bytes(page_id: number, file_name: string, bytes: Uint8Array | number[], key_col_hint: string, table_hint: string, auto_bind: boolean, auto_enable_graphics: boolean): string;
    import_org_chart_csv(page_id: number, csv_text: string): number;
    import_sqlite_recordset(page_id: number, sqlite_bytes: Uint8Array | number[], table_name: string, key_col: string): number;
    /**
     * Import an external `.vssx`, `.vssm`, `.vss`, `.vsdx`, `.vdx`, `.svg`, or `.zip` Visio stencil archive
     * directly into the Stencil Navigator with lossless geometry, `ForeignData`, and Retina PNG/SVG thumbnails.
     */
    import_stencil_bytes(file_name: string, bytes: Uint8Array | number[]): string;
    insert_architectural_room(page_id: number, room_name: string): number;
    insert_callout(page_id: number, target_shape_id: number, text: string): number;
    insert_container(page_id: number, title: string, member_ids: Uint32Array | number[]): number;
    insert_cropped_image(page_id: number, media_type: string, image_bytes: Uint8Array | number[], x_in: number, y_in: number, crop_in: number, brightness: number, contrast: number): number;
    insert_data_legend(page_id: number, prop_name: string, pin_x_in: number, pin_y_in: number): number;
    insert_heading(page_id: number, preset_slug: string, custom_text: string, x_in: number, y_in: number): number;
    insert_rack_with_equipment(page_id: number): number;
    insert_shape_vertex(page_id: number, shape_id: number, geom_index: number, after_row_ix: number, x_in: number, y_in: number): boolean;
    insert_swimlanes(page_id: number, lanes_semicolon: string): number;
    insert_text_field(page_id: number, shape_id: number, preset_or_formula: string, format_spec: string, prefix_label: string): number;
    link_csv_and_apply_data_graphics(page_id: number, csv_text: string, key_col: string): number;
    list_arrowheads_json(): string;
    list_builtin_pack_order_json(): string;
    list_builtin_stencil_previews_json(): string;
    list_builtin_stencils_json(): string;
    list_connection_points_json(page_id: number, shape_id: number): string;
    /**
     * Returns the curated catalog of direct-HTTPS official & enterprise Visio stencil packages.
     */
    list_official_stencil_catalog_json(): string;
    list_ruler_guides_json(page_id: number): string;
    list_shape_actions_json(page_id: number, shape_id: number): string;
    list_shape_vertices_json(page_id: number, shape_id: number): string;
    list_shapesheet_functions_json(): string;
    list_slide_snippets_json(): string;
    list_text_fields_json(page_id: number, shape_id: number): string;
    load_starter_template(template_id: string): void;
    move_connection_point(page_id: number, shape_id: number, ix: number, page_x_in: number, page_y_in: number): boolean;
    move_connector_endpoint(page_id: number, shape_id: number, is_begin: boolean, x_in: number, y_in: number, target_shape_id: number): boolean;
    move_connector_endpoint_with_port(page_id: number, shape_id: number, is_begin: boolean, x_in: number, y_in: number, target_shape_id: number, target_port_cell: string): boolean;
    move_control_handle(page_id: number, shape_id: number, row_name: string, x_in: number, y_in: number): boolean;
    move_ruler_guide(page_id: number, guide_id: number, coord_in: number): boolean;
    move_shape(page_id: number, shape_id: number, new_pin_x_in: number, new_pin_y_in: number): boolean;
    move_shape_vertex(page_id: number, shape_id: number, geom_index: number, row_ix: number, x_in: number, y_in: number): boolean;
    /**
     * Batch-move multiple lasso-selected shapes atomically in a single Undo/Redo transaction.
     */
    move_shapes_delta(page_id: number, shape_ids: Uint32Array | number[], dx_in: number, dy_in: number): number;
    constructor(title: string);
    number_shapes_on_page(page_id: number, shape_ids: Uint32Array | number[], order_mode: string, start_num: number, interval: number, prefix: string, target_mode: string): number;
    open_bytes(bytes: Uint8Array | number[], ext: string): void;
    /**
     * Marquee Lasso Multi-Select resolving top-level page shapes & groups via Visio enclosure rules.
     */
    query_box_shapes(page_id: number, x1_in: number, y1_in: number, x2_in: number, y2_in: number): Uint32Array;
    redo(): boolean;
    refresh_external_recordset_bytes(page_id: number, recordset_id: number, new_bytes: Uint8Array | number[], format_hint: string): number;
    /**
     * Stream custom `.ttf` / `.otf` TrueType/OpenType bytes (e.g., from ChromeOS Local Font Access API `.blob()`)
     * directly into the `vexio-render` TrueType shaper & PDF contour engine.
     */
    register_font_bytes(bytes: Uint8Array | number[], family_override: string): boolean;
    rename_custom_master(stencil_title: string, old_name: string, new_name: string): boolean;
    rename_page(page_id: number, new_name: string): boolean;
    rename_shape(page_id: number, shape_id: number, new_name: string): boolean;
    render_offscreen_frame(page_id: number, selected_ids: Uint32Array | number[]): boolean;
    reorder_shapes(page_id: number, shape_ids: Uint32Array | number[], mode: string): number;
    replace_in_document(active_page_id: number, find_str: string, replace_str: string, match_case: boolean, whole_word: boolean, all_pages: boolean): number;
    replay_wal(bytes: Uint8Array | number[]): void;
    reset_connector_routing(page_id: number, shape_id: number): boolean;
    resize_offscreen_canvas(width: number, height: number, dpi: number): void;
    resolve_comment(comment_id: number, resolved: boolean): boolean;
    reverse_engineer_sql(page_id: number, sql_ddl: string): number;
    rotate_flip_shapes(page_id: number, shape_ids: Uint32Array | number[], delta_angle_deg: number, toggle_flip_x: boolean, toggle_flip_y: boolean): number;
    run_domain_wizard(page_id: number, wizard_slug: string, spec_text: string): string;
    run_vba_macro(script: string): string;
    run_vba_macro_with_selection(script: string, selected_ids: Uint32Array | number[]): string;
    save_format(ext: string): Uint8Array;
    save_page_format(page_id: number, ext: string): Uint8Array;
    set_cell_formula(page_id: number, shape_id: number, cell_name: string, formula: string, force_setf: boolean): boolean;
    set_connector_arrowheads(page_id: number, shape_ids: Uint32Array | number[], begin_arrow: number, end_arrow: number, arrow_size: number): number;
    set_document_title(title: string): void;
    set_hyperlink_and_screentip(page_id: number, shape_id: number, description: string, address: string, sub_address: string, screentip: string): boolean;
    set_layer_flags(page_id: number, layer_ix: number, visible: boolean, lock: boolean, print: boolean, snap_glue: boolean): boolean;
    set_page_background(page_id: number, is_background: boolean, back_page_id: number, page_color_formula: string): boolean;
    set_page_data_graphics_json(page_id: number, config_json: string): number;
    set_page_dimensions(page_id: number, width_in: number, height_in: number, jump_style: number): void;
    set_page_line_jumps(page_id: number, jump_code: number, jump_style: number, factor_x: number, factor_y: number): boolean;
    set_page_preset(page_id: number, preset_slug: string, landscape: boolean): boolean;
    set_page_setup_with_unit(page_id: number, width_in: number, height_in: number, unit_slug: string): boolean;
    set_replica_id(replica_id: string): void;
    set_shape_layer_membership(page_id: number, shape_id: number, layer_ix: number, assigned: boolean): boolean;
    set_shape_rich_runs_json(page_id: number, shape_id: number, runs_json: string): void;
    set_shape_text(page_id: number, shape_id: number, text: string): void;
    set_snap_glue(page_id: number, snap_enabled: boolean, glue_enabled: boolean, snap_targets_mask: number, glue_types_mask: number): boolean;
    set_text_block_transform(page_id: number, shape_id: number, txt_pin_x_in: number, txt_pin_y_in: number, txt_width_in: number, txt_height_in: number, txt_angle_deg: number): void;
    sync_and_export_recordset_bytes(page_id: number, recordset_id: number, format_ext: string): Uint8Array;
    /**
     * 8-Handle Interactive Resize + Top Rotation Knob transaction respecting `LockWidth`,
     * `LockHeight`, `LockAspect`, `LockRotate`, and `GUARD`.
     */
    transform_shape(page_id: number, shape_id: number, pin_x_in: number, pin_y_in: number, width_in: number, height_in: number, angle_deg: number): boolean;
    undo(): boolean;
    ungroup_shapes(page_id: number, group_ids: Uint32Array | number[]): Uint32Array;
    update_custom_master(page_id: number, source_shape_id: number, stencil_title: string, master_name: string): number;
    update_layer(page_id: number, layer_ix: number, visible: boolean, lock: boolean, color_hex: string): void;
    upsert_shape_action(page_id: number, shape_id: number, row_name: string, menu: string, action_formula: string, checked_formula: string, disabled_formula: string, begin_group: boolean): boolean;
    upsert_shape_data(page_id: number, shape_id: number, row_name: string, label: string, data_type: number, value_input: string): boolean;
    validate_diagram(ruleset: string): string;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmvexiosession_free: (a: number, b: number) => void;
    readonly vexio_buffer_free: (a: number) => void;
    readonly vexio_cell_get_formula: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_cell_get_value_str: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_cell_set_formula: (a: number, b: number, c: bigint, d: number, e: number, f: number) => number;
    readonly vexio_doc_add_comment: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_add_connection_point: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_add_layer: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_add_page: (a: number, b: number, c: number) => number;
    readonly vexio_doc_add_ruler_guide: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_add_slide_snippet: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly vexio_doc_add_to_custom_stencil: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_align_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_apply_border_and_title: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_apply_cad_scale: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_apply_crdt_json: (a: number, b: number, c: number) => number;
    readonly vexio_doc_apply_format_painter: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_apply_heading_preset: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_apply_theme: (a: number, b: number, c: number) => number;
    readonly vexio_doc_apply_visual_preset: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_attach_callout: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_doc_auto_connect: (a: number, b: number, c: bigint, d: number, e: number, f: number) => number;
    readonly vexio_doc_auto_layout: (a: number, b: number, c: number) => number;
    readonly vexio_doc_auto_slide_snippets: (a: number, b: number, c: number) => number;
    readonly vexio_doc_bind_external_row: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_boolean_op: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_clear_page_ink: (a: number, b: number, c: number) => number;
    readonly vexio_doc_compare: (a: number, b: number, c: number) => number;
    readonly vexio_doc_connect_shapes: (a: number, b: number, c: bigint, d: bigint, e: number) => number;
    readonly vexio_doc_convert_cad_layers: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_copy_shape_format_json: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_create_blank: (a: number) => number;
    readonly vexio_doc_create_subprocess: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_doc_delete_comment: (a: number, b: number) => number;
    readonly vexio_doc_delete_connection_point: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_delete_custom_master: (a: number, b: number, c: number) => number;
    readonly vexio_doc_delete_external_recordset: (a: number, b: number) => number;
    readonly vexio_doc_delete_page: (a: number, b: number) => number;
    readonly vexio_doc_delete_ruler_guide: (a: number, b: number, c: bigint) => number;
    readonly vexio_doc_delete_shape_action: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_delete_shape_data: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_delete_shapes: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_delete_slide_snippet: (a: number, b: number, c: number) => number;
    readonly vexio_doc_delete_vertex: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_doc_destroy: (a: number) => void;
    readonly vexio_doc_distribute_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_drag_connector_segment: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_doc_drag_control_handle: (a: number, b: number, c: bigint, d: number, e: number, f: number) => number;
    readonly vexio_doc_draw_freeform_json: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_draw_primitive: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_drop_stencil: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_duplicate_page: (a: number, b: number, c: number) => number;
    readonly vexio_doc_duplicate_shapes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_ensure_control_handle: (a: number, b: number, c: bigint) => number;
    readonly vexio_doc_execute_shape_action: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_doc_export_bom_report: (a: number, b: number, c: number) => number;
    readonly vexio_doc_export_custom_stencil_vssx: (a: number, b: number, c: number) => number;
    readonly vexio_doc_export_data_visualizer_xlsx: (a: number, b: number, c: number) => number;
    readonly vexio_doc_export_markup: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_find_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_flush_crdt_json: (a: number, b: number, c: number, d: bigint, e: number) => number;
    readonly vexio_doc_get_data_link_state_json: (a: number, b: number, c: number) => number;
    readonly vexio_doc_get_snap_glue_json: (a: number, b: number, c: number) => number;
    readonly vexio_doc_get_state_json: (a: number, b: number, c: number) => number;
    readonly vexio_doc_group_shapes: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_import_cloud_discovery: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_import_data_visualizer: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_import_entra_or_ldif: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_import_external_recordset: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly vexio_doc_import_markup: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_import_stencil_bytes: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_insert_container: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_insert_data_legend: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_insert_heading: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_insert_swimlanes: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_insert_text_field: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_insert_vertex: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_list_comments_json: (a: number, b: number, c: number) => number;
    readonly vexio_doc_list_connection_points_json: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_list_ruler_guides_json: (a: number, b: number, c: number) => number;
    readonly vexio_doc_list_shape_actions_json: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_list_slide_snippets_json: (a: number, b: number) => number;
    readonly vexio_doc_list_stencils_json: (a: number, b: number) => number;
    readonly vexio_doc_list_text_fields_json: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_list_vertices_json: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_load_starter_template: (a: number, b: number) => number;
    readonly vexio_doc_move_connection_point: (a: number, b: number, c: bigint, d: number, e: number, f: number) => number;
    readonly vexio_doc_move_ruler_guide: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_move_vertex: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_number_shapes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly vexio_doc_open_bytes: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_redo: (a: number) => number;
    readonly vexio_doc_refresh_external_recordset: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_rename_custom_master: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_rename_page: (a: number, b: number, c: number) => number;
    readonly vexio_doc_rename_shape: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_reorder_shapes: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_replace: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_reset_connector_routing: (a: number, b: number, c: bigint) => number;
    readonly vexio_doc_resolve_comment: (a: number, b: number, c: number) => number;
    readonly vexio_doc_rotate_flip_shapes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_run_domain_wizard: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_run_vba_macro: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_save_format: (a: number, b: number, c: number) => number;
    readonly vexio_doc_set_connector_arrowheads: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_set_hyperlink_and_screentip: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_set_layer_flags: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_set_page_background: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_set_page_data_graphics_json: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_set_page_line_jumps: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_set_page_preset: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_doc_set_page_setup: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_set_shape_layer_membership: (a: number, b: number, c: bigint, d: number, e: number) => number;
    readonly vexio_doc_set_shape_rich_runs_json: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_set_shape_text: (a: number, b: number, c: bigint, d: number) => number;
    readonly vexio_doc_set_shape_text_style_json: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_set_snap_glue: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly vexio_doc_set_text_block_transform: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_set_title: (a: number, b: number) => number;
    readonly vexio_doc_sync_and_export_recordset: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_transform_shape: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number) => number;
    readonly vexio_doc_undo: (a: number) => number;
    readonly vexio_doc_ungroup_shapes: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_doc_update_custom_master: (a: number, b: number, c: bigint, d: number, e: number, f: number) => number;
    readonly vexio_doc_upsert_shape_action: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly vexio_doc_upsert_shape_data: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_doc_validate_json: (a: number, b: number, c: number) => number;
    readonly vexio_engine_create: () => number;
    readonly vexio_engine_destroy: (a: number) => void;
    readonly vexio_last_error_message: (a: number) => number;
    readonly vexio_list_arrowheads_json: (a: number) => number;
    readonly vexio_list_official_stencil_catalog_json: (a: number) => number;
    readonly vexio_list_shapesheet_functions_json: (a: number) => number;
    readonly vexio_register_font_bytes: (a: number, b: number) => number;
    readonly vexio_render_preview_rgba: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_viewport_create_dx12_swapchain: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_viewport_create_headless: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly vexio_viewport_destroy: (a: number) => void;
    readonly vexio_viewport_dispatch_pointer: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_viewport_get_active_tool: (a: number) => number;
    readonly vexio_viewport_get_primary_selection: (a: number) => bigint;
    readonly vexio_viewport_get_selection_json: (a: number, b: number) => number;
    readonly vexio_viewport_get_shape_screen_bounds: (a: number, b: number, c: bigint, d: number, e: number, f: number, g: number) => number;
    readonly vexio_viewport_get_smart_guides: (a: number) => number;
    readonly vexio_viewport_hit_test_shape: (a: number, b: number, c: number, d: number) => bigint;
    readonly vexio_viewport_pop_created_text_shape: (a: number) => bigint;
    readonly vexio_viewport_render_frame: (a: number, b: number) => number;
    readonly vexio_viewport_render_rgba: (a: number, b: number, c: number) => number;
    readonly vexio_viewport_resize: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_viewport_screen_to_page: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly vexio_viewport_set_page: (a: number, b: number, c: number) => number;
    readonly vexio_viewport_set_selection: (a: number, b: number, c: number, d: number) => number;
    readonly vexio_viewport_set_smart_guides: (a: number, b: number) => number;
    readonly vexio_viewport_set_tool: (a: number, b: number) => number;
    readonly vexio_viewport_set_zoom: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_add_comment: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_add_connection_point: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_add_ink_stroke: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly wasmvexiosession_add_layer: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_add_page: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmvexiosession_add_ruler_guide: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_add_slide_snippet: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => [number, number, number];
    readonly wasmvexiosession_add_to_custom_stencil: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number, number];
    readonly wasmvexiosession_align_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_apply_auto_layout: (a: number, b: number, c: number, d: number) => void;
    readonly wasmvexiosession_apply_boolean_op: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly wasmvexiosession_apply_border_and_title: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => number;
    readonly wasmvexiosession_apply_cad_scale: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wasmvexiosession_apply_format_painter: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_apply_heading_preset: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_apply_remote_crdt_ops_json: (a: number, b: number, c: number) => [number, number, number];
    readonly wasmvexiosession_apply_shape_visual_preset: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_apply_text_style_json: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_apply_theme_variant: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_apply_title_block_border: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly wasmvexiosession_attach_offscreen_canvas: (a: number, b: any, c: number, d: number, e: number) => any;
    readonly wasmvexiosession_auto_connect: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_auto_generate_slide_snippets: (a: number, b: number) => [number, number, number];
    readonly wasmvexiosession_bind_external_row: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_clear_page_ink: (a: number, b: number) => number;
    readonly wasmvexiosession_compare_with_vsdx_bytes: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmvexiosession_compute_smart_guides: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly wasmvexiosession_connect_points_or_shapes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly wasmvexiosession_connect_shapes: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_convert_cad_layers: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_copy_shape_format_json: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmvexiosession_create_subprocess_drilldown: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_delete_comment: (a: number, b: number) => number;
    readonly wasmvexiosession_delete_connection_point: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_delete_custom_master: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_delete_external_recordset: (a: number, b: number) => number;
    readonly wasmvexiosession_delete_page: (a: number, b: number) => number;
    readonly wasmvexiosession_delete_ruler_guide: (a: number, b: number, c: number) => number;
    readonly wasmvexiosession_delete_shape: (a: number, b: number, c: number) => number;
    readonly wasmvexiosession_delete_shape_action: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_delete_shape_data: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_delete_shape_vertex: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_delete_slide_snippet: (a: number, b: number, c: number) => number;
    readonly wasmvexiosession_distribute_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_drag_connector_segment: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wasmvexiosession_draw_arc: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly wasmvexiosession_draw_freeform_curve: (a: number, b: number, c: number, d: number, e: number) => [number, number, number];
    readonly wasmvexiosession_draw_line: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_draw_primitive_2d: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly wasmvexiosession_draw_rectangle: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_drop_builtin_shape: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly wasmvexiosession_drop_builtin_shape_on_page: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly wasmvexiosession_duplicate_page: (a: number, b: number) => [number, number, number];
    readonly wasmvexiosession_duplicate_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly wasmvexiosession_ensure_control_handle: (a: number, b: number, c: number) => number;
    readonly wasmvexiosession_execute_shape_action: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly wasmvexiosession_export_bom_report_bytes: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmvexiosession_export_bom_xlsx: (a: number) => [number, number, number, number];
    readonly wasmvexiosession_export_custom_stencil_vssx: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmvexiosession_export_data_visualizer_workbook: (a: number, b: number) => [number, number, number, number];
    readonly wasmvexiosession_export_diagram_markup: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wasmvexiosession_export_wal: (a: number) => [number, number];
    readonly wasmvexiosession_find_in_document_json: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly wasmvexiosession_flush_crdt_ops_json: (a: number, b: number) => [number, number];
    readonly wasmvexiosession_forward_engineer_sql: (a: number, b: number) => [number, number];
    readonly wasmvexiosession_generate_gantt_demo: (a: number, b: number) => number;
    readonly wasmvexiosession_generate_pert_demo: (a: number, b: number) => number;
    readonly wasmvexiosession_generate_timeline_demo: (a: number, b: number) => number;
    readonly wasmvexiosession_get_page_data_link_state_json: (a: number, b: number) => [number, number];
    readonly wasmvexiosession_get_snap_glue_json: (a: number, b: number) => [number, number];
    readonly wasmvexiosession_get_state_json: (a: number, b: number) => [number, number];
    readonly wasmvexiosession_get_text_block_overlay_json: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmvexiosession_group_shapes: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_hit_test_page: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_import_cloud_discovery: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_import_data_visualizer: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_import_diagram_markup: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly wasmvexiosession_import_entra_or_ldif: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_import_external_recordset_bytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number, number];
    readonly wasmvexiosession_import_org_chart_csv: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_import_sqlite_recordset: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly wasmvexiosession_import_stencil_bytes: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly wasmvexiosession_insert_architectural_room: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_insert_callout: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_insert_container: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_insert_cropped_image: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => number;
    readonly wasmvexiosession_insert_data_legend: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_insert_heading: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number, number];
    readonly wasmvexiosession_insert_rack_with_equipment: (a: number, b: number) => number;
    readonly wasmvexiosession_insert_shape_vertex: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_insert_swimlanes: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_insert_text_field: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number];
    readonly wasmvexiosession_link_csv_and_apply_data_graphics: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly wasmvexiosession_list_arrowheads_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_builtin_pack_order_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_builtin_stencil_previews_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_builtin_stencils_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_connection_points_json: (a: number, b: number, c: number) => [number, number];
    readonly wasmvexiosession_list_official_stencil_catalog_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_ruler_guides_json: (a: number, b: number) => [number, number];
    readonly wasmvexiosession_list_shape_actions_json: (a: number, b: number, c: number) => [number, number];
    readonly wasmvexiosession_list_shape_vertices_json: (a: number, b: number, c: number) => [number, number];
    readonly wasmvexiosession_list_shapesheet_functions_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_slide_snippets_json: (a: number) => [number, number];
    readonly wasmvexiosession_list_text_fields_json: (a: number, b: number, c: number) => [number, number];
    readonly wasmvexiosession_load_starter_template: (a: number, b: number, c: number) => void;
    readonly wasmvexiosession_move_connection_point: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_move_connector_endpoint: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly wasmvexiosession_move_connector_endpoint_with_port: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => [number, number, number];
    readonly wasmvexiosession_move_control_handle: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_move_ruler_guide: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_move_shape: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_move_shape_vertex: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_move_shapes_delta: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_new: (a: number, b: number) => number;
    readonly wasmvexiosession_number_shapes_on_page: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number) => [number, number, number];
    readonly wasmvexiosession_open_bytes: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmvexiosession_query_box_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number];
    readonly wasmvexiosession_redo: (a: number) => number;
    readonly wasmvexiosession_refresh_external_recordset_bytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly wasmvexiosession_register_font_bytes: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_rename_custom_master: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_rename_page: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_rename_shape: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_render_offscreen_frame: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_reorder_shapes: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_replace_in_document: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number) => number;
    readonly wasmvexiosession_replay_wal: (a: number, b: number, c: number) => [number, number];
    readonly wasmvexiosession_reset_connector_routing: (a: number, b: number, c: number) => number;
    readonly wasmvexiosession_resize_offscreen_canvas: (a: number, b: number, c: number, d: number) => void;
    readonly wasmvexiosession_resolve_comment: (a: number, b: number, c: number) => number;
    readonly wasmvexiosession_reverse_engineer_sql: (a: number, b: number, c: number, d: number) => number;
    readonly wasmvexiosession_rotate_flip_shapes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_run_domain_wizard: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number, number];
    readonly wasmvexiosession_run_vba_macro: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmvexiosession_run_vba_macro_with_selection: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly wasmvexiosession_save_format: (a: number, b: number, c: number) => [number, number, number, number];
    readonly wasmvexiosession_save_page_format: (a: number, b: number, c: number, d: number) => [number, number, number, number];
    readonly wasmvexiosession_set_cell_formula: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly wasmvexiosession_set_connector_arrowheads: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_set_document_title: (a: number, b: number, c: number) => void;
    readonly wasmvexiosession_set_hyperlink_and_screentip: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number) => number;
    readonly wasmvexiosession_set_layer_flags: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasmvexiosession_set_page_background: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_set_page_data_graphics_json: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly wasmvexiosession_set_page_dimensions: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly wasmvexiosession_set_page_line_jumps: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_set_page_preset: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_set_page_setup_with_unit: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_set_replica_id: (a: number, b: number, c: number) => void;
    readonly wasmvexiosession_set_shape_layer_membership: (a: number, b: number, c: number, d: number, e: number) => number;
    readonly wasmvexiosession_set_shape_rich_runs_json: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmvexiosession_set_shape_text: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmvexiosession_set_snap_glue: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly wasmvexiosession_set_text_block_transform: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => [number, number];
    readonly wasmvexiosession_sync_and_export_recordset_bytes: (a: number, b: number, c: number, d: number, e: number) => [number, number, number, number];
    readonly wasmvexiosession_transform_shape: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => number;
    readonly wasmvexiosession_undo: (a: number) => number;
    readonly wasmvexiosession_ungroup_shapes: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmvexiosession_update_custom_master: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly wasmvexiosession_update_layer: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly wasmvexiosession_upsert_shape_action: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number, k: number, l: number, m: number, n: number) => number;
    readonly wasmvexiosession_upsert_shape_data: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => number;
    readonly wasmvexiosession_validate_diagram: (a: number, b: number, c: number) => [number, number];
    readonly vexio_viewport_create_metal_layer: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined___js_sys_f9f1820855ccb71a___Function_fn_wasm_bindgen_70d1b2bcb7a7720f___JsValue_____wasm_bindgen_70d1b2bcb7a7720f___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_70d1b2bcb7a7720f___convert__closures_____invoke___wasm_bindgen_70d1b2bcb7a7720f___JsValue__core_d941ad8722425d2e___result__Result_____wasm_bindgen_70d1b2bcb7a7720f___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
