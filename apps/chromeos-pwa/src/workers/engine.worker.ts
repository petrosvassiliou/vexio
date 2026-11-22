/**
 * Dedicated Rust WASM SIMD + SharedArrayBuffer + WebGPU / OffscreenCanvas Engine Worker.
 */

/// <reference lib="webworker" />

export interface ControlHandleDto {
  name: string;
  page_x_in: number;
  page_y_in: number;
  prompt: string;
  can_glue: boolean;
}

export interface EditableVertexDto {
  geom_index: number;
  row_ix: number;
  kind: string;
  page_x_in: number;
  page_y_in: number;
}

export interface EngineShapeSummary {
  id: number;
  name: string;
  kind: string;
  text: string;
  pinX: number;
  pinY: number;
  txtX?: number;
  txtY?: number;
  width: number;
  height: number;
  angleDeg: number;
  flipX?: boolean;
  flipY?: boolean;
  fontFamily?: string;
  fontSizePt?: number;
  charStyle?: number;
  textColor?: string;
  textBgColor?: string | null;
  horizAlign?: number;
  vertAlign?: number;
  bulletStyle?: number;
  beginX?: number | null;
  beginY?: number | null;
  endX?: number | null;
  endY?: number | null;
  pathPoints?: Array<[number, number]>;
  ports?: Array<[number, number]>;
  portCells?: string[];
  routeStyle?: number;
  beginArrow?: number;
  endArrow?: number;
  lineWeightPt?: number;
  linePattern?: number;
  subprocessPage: string | null;
  screentip: string | null;
  controlHandles: ControlHandleDto[];
  cells: Array<{ name: string; formula: string; value: string; guarded: boolean }>;
}

export interface EngineLayerInfo {
  ix: number;
  name: string;
  visible: boolean;
  print: boolean;
  lock: boolean;
  colorOverride: string | null;
}

export interface EngineCommentInfo {
  id: number;
  author: string;
  text: string;
  pageId: number;
  shapeId: number | null;
  resolved: boolean;
}

export interface SmartGuideLine {
  axis: 'x' | 'y';
  posIn: number;
  fromIn: number;
  toIn: number;
  kind: 'center' | 'edge' | 'equal_spacing';
}

export interface SmartGuidesResult {
  snappedX: number;
  snappedY: number;
  guides: SmartGuideLine[];
}

export interface EngineSnapshot {
  title: string;
  activePageId: number;
  pageWidthIn: number;
  pageHeightIn: number;
  pages: Array<{
    id: number;
    name: string;
    widthIn: number;
    heightIn: number;
    shapeCount: number;
    isBackground: boolean;
  }>;
  layers: EngineLayerInfo[];
  comments: EngineCommentInfo[];
  shapes: EngineShapeSummary[];
  svgPreview: string;
  canUndo: boolean;
  canRedo: boolean;
  wgpuSurfaceActive: boolean;
  validationIssues: Array<{
    id: number;
    code: string;
    severity: string;
    category: string;
    description: string;
    shapeId: number | null;
  }>;
  crossOriginIsolated: boolean;
  webgpuAvailable: boolean;
}

interface WasmExports {
  default: (moduleOrPath?: string | URL | { module_or_path?: string | URL }) => Promise<unknown>;
  WasmVexioSession: new (title: string) => WasmSessionInstance;
}

interface WasmSessionInstance {
  attach_offscreen_canvas(
    canvas: OffscreenCanvas,
    width: number,
    height: number,
    dpi: number
  ): Promise<boolean>;
  resize_offscreen_canvas(width: number, height: number, dpi: number): void;
  render_offscreen_frame(page_id: number, selected_ids: Uint32Array): boolean;
  list_builtin_stencils_json(): string;
  list_builtin_stencil_previews_json(): string;
  list_builtin_pack_order_json(): string;
  import_stencil_bytes(file_name: string, bytes: Uint8Array): string;
  load_starter_template(template_id: string): void;
  open_bytes(bytes: Uint8Array, ext: string): void;
  save_format(ext: string): Uint8Array;
  save_page_format?(page_id: number, ext: string): Uint8Array;
  drop_builtin_shape_on_page(
    page_id: number,
    category: string,
    master_name: string,
    x_in: number,
    y_in: number
  ): number;
  hit_test_page(page_id: number, x_in: number, y_in: number): number;
  query_box_shapes(page_id: number, x1: number, y1: number, x2: number, y2: number): Uint32Array;
  compute_smart_guides(
    page_id: number,
    moving_shape_id: number,
    cand_x: number,
    cand_y: number,
    width_in: number,
    height_in: number
  ): string;
  move_shape(page_id: number, shape_id: number, new_x: number, new_y: number): boolean;
  move_shapes_delta(page_id: number, shape_ids: Uint32Array, dx_in: number, dy_in: number): number;
  transform_shape(
    page_id: number,
    shape_id: number,
    pin_x: number,
    pin_y: number,
    width_in: number,
    height_in: number,
    angle_deg: number
  ): boolean;
  align_shapes(page_id: number, shape_ids: Uint32Array, mode: string): number;
  reorder_shapes(page_id: number, shape_ids: Uint32Array, mode: string): number;
  draw_rectangle(page_id: number, x1: number, y1: number, x2: number, y2: number): number;
  draw_primitive_2d(page_id: number, kind: string, x1: number, y1: number, x2: number, y2: number): number;
  draw_line(page_id: number, x1: number, y1: number, x2: number, y2: number): number;
  draw_arc(page_id: number, x1: number, y1: number, x2: number, y2: number, bow_in: number): number;
  connect_points_or_shapes(
    page_id: number,
    x1: number,
    y1: number,
    x2: number,
    y2: number,
    from_shape_id: number,
    to_shape_id: number
  ): number;
  move_connector_endpoint(
    page_id: number,
    shape_id: number,
    is_begin: boolean,
    x_in: number,
    y_in: number,
    target_shape_id: number
  ): boolean;
  duplicate_shapes(page_id: number, shape_ids: Uint32Array, dx_in: number, dy_in: number): Uint32Array;
  delete_shape(page_id: number, shape_id: number): boolean;
  add_ink_stroke(
    page_id: number,
    color_hex: string,
    width_pt: number,
    highlighter: boolean,
    points_json: string
  ): number;
  connect_shapes(page_id: number, from_id: number, to_id: number): number;
  set_cell_formula(
    page_id: number,
    shape_id: number,
    cell_name: string,
    formula: string,
    force_setf: boolean
  ): boolean;
  set_shape_text(page_id: number, shape_id: number, text: string): void;
  set_document_title(title: string): void;
  apply_text_style_json(page_id: number, shape_ids: Uint32Array, style_json: string): number;
  rotate_flip_shapes(
    page_id: number,
    shape_ids: Uint32Array,
    delta_angle_deg: number,
    toggle_flip_x: boolean,
    toggle_flip_y: boolean
  ): number;
  set_page_preset(page_id: number, preset_slug: string, landscape: boolean): boolean;
  set_page_setup_with_unit(
    page_id: number,
    width_in: number,
    height_in: number,
    unit_slug: string
  ): boolean;
  set_page_dimensions(page_id: number, width_in: number, height_in: number, jump_style: number): void;
  insert_container(page_id: number, title: string, member_ids: Uint32Array): number;
  insert_callout(page_id: number, target_shape_id: number, text: string): number;
  insert_swimlanes(page_id: number, lanes_semicolon: string): number;
  insert_architectural_room(page_id: number, room_name: string): number;
  insert_rack_with_equipment(page_id: number): number;
  import_org_chart_csv(page_id: number, csv_text: string): number;
  generate_gantt_demo(page_id: number): number;
  generate_timeline_demo(page_id: number): number;
  reverse_engineer_sql(page_id: number, sql_ddl: string): number;
  forward_engineer_sql(page_id: number): string;
  generate_pert_demo(page_id: number): number;
  link_csv_and_apply_data_graphics(page_id: number, csv_text: string, key_col: string): number;
  export_bom_xlsx(): Uint8Array;
  update_layer(
    page_id: number,
    layer_ix: number,
    visible: boolean,
    lock: boolean,
    color_hex: string
  ): void;
  add_comment(page_id: number, shape_id: number, author: string, text: string): number;
  apply_auto_layout(page_id: number, algo: string): void;
  validate_diagram(ruleset: string): string;
  run_vba_macro(script: string): string;
  run_vba_macro_with_selection(script: string, selected_ids: Uint32Array): string;
  add_page(name: string): number;
  rename_page(page_id: number, new_name: string): boolean;
  duplicate_page(page_id: number): number;
  delete_page(page_id: number): boolean;
  undo(): boolean;
  redo(): boolean;
  export_wal(): Uint8Array;
  replay_wal(bytes: Uint8Array): void;
  get_state_json(active_page_id: number): string;

  // Phase 7 Complete Parity Endpoints
  set_replica_id(replica_id: string): void;
  flush_crdt_ops_json(now_ms: number): string;
  apply_remote_crdt_ops_json(ops_json: string): number;
  apply_boolean_op(page_id: number, shape_ids: Uint32Array, op: string, offset_in: number): Uint32Array;
  list_shape_vertices_json(page_id: number, shape_id: number): string;
  move_shape_vertex(
    page_id: number,
    shape_id: number,
    geom_index: number,
    row_ix: number,
    x_in: number,
    y_in: number
  ): boolean;
  draw_freeform_curve(page_id: number, points_json: string, closed: boolean): number;
  ensure_control_handle(page_id: number, shape_id: number): boolean;
  move_control_handle(page_id: number, shape_id: number, row_name: string, x_in: number, y_in: number): boolean;
  apply_theme_variant(theme_slug: string, variant_index: number): number;
  apply_shape_visual_preset(page_id: number, shape_id: number, preset: string): boolean;
  insert_cropped_image(
    page_id: number,
    media_type: string,
    image_bytes: Uint8Array,
    x_in: number,
    y_in: number,
    crop_in: number,
    brightness: number,
    contrast: number
  ): number;
  create_subprocess_drilldown(page_id: number, shape_id: number, sub_name: string): number;
  set_hyperlink_and_screentip(
    page_id: number,
    shape_id: number,
    description: string,
    address: string,
    sub_address: string,
    screentip: string
  ): boolean;
  apply_title_block_border(page_id: number, project_code: string, author: string, revision: string): number;
  compare_with_vsdx_bytes(baseline_vsdx: Uint8Array): string;
  import_data_visualizer(page_id: number, bytes: Uint8Array): number;
  export_data_visualizer_workbook(page_id: number): Uint8Array;
  import_cloud_discovery(page_id: number, discovery_json: string): number;
  import_entra_or_ldif(page_id: number, text: string): number;
  import_sqlite_recordset(page_id: number, sqlite_bytes: Uint8Array, table_name: string, key_col: string): number;
  register_font_bytes(bytes: Uint8Array, family_override: string): boolean;
  distribute_shapes(page_id: number, shape_ids: Uint32Array, axis: string): number;
  group_shapes(page_id: number, shape_ids: Uint32Array): number;
  ungroup_shapes(page_id: number, group_ids: Uint32Array): Uint32Array;
  move_connector_endpoint_with_port(
    page_id: number,
    shape_id: number,
    is_begin: boolean,
    x_in: number,
    y_in: number,
    target_shape_id: number,
    target_port_cell: string
  ): boolean;
  drag_connector_segment(page_id: number, shape_id: number, seg_index: number, coord_in: number): boolean;
}

let wasmSession: WasmSessionInstance | null = null;
let activePageId = 1;
let selectedShapeIds: number[] = [];
let sharedInputRing: Int32Array | null = null;
let webgpuSupported = false;
let latestIssues: EngineSnapshot['validationIssues'] = [];

declare const __VEXIO_WASM_HASH__: string | undefined;
const WASM_CACHE_TAG =
  typeof __VEXIO_WASM_HASH__ === 'string' && __VEXIO_WASM_HASH__.length > 0
    ? __VEXIO_WASM_HASH__
    : '20261121v525';

async function ensureWasmLoaded(): Promise<WasmSessionInstance> {
  if (wasmSession) return wasmSession;

  if ('gpu' in navigator) {
    try {
      // @ts-expect-error WebGPU navigator.gpu in WorkerGlobalScope
      const adapter = await navigator.gpu.requestAdapter();
      webgpuSupported = Boolean(adapter);
    } catch {
      webgpuSupported = false;
    }
  }

  const wasmJsUrl = new URL(`/wasm/vexio_core.js?v=${WASM_CACHE_TAG}`, self.location.origin).href;
  const wasmBinUrl = new URL(`/wasm/vexio_core_bg.wasm?v=${WASM_CACHE_TAG}`, self.location.origin).href;
  const mod = (await import(/* @vite-ignore */ wasmJsUrl)) as WasmExports;
  await mod.default({ module_or_path: wasmBinUrl });
  wasmSession = new mod.WasmVexioSession('Untitled Drawing');
  return wasmSession;
}

function buildSnapshot(session: WasmSessionInstance): EngineSnapshot {
  const raw = JSON.parse(session.get_state_json(activePageId));
  if (typeof raw.activePageId === 'number') {
    activePageId = raw.activePageId;
  }
  if (raw.wgpuSurfaceActive) {
    session.render_offscreen_frame(activePageId, new Uint32Array(selectedShapeIds));
  }
  return {
    ...raw,
    validationIssues: latestIssues,
    crossOriginIsolated: Boolean(self.crossOriginIsolated),
    webgpuAvailable: webgpuSupported,
  };
}

self.onmessage = async (ev: MessageEvent) => {
  const { id, cmd, payload } = ev.data;
  try {
    const session = await ensureWasmLoaded();

    switch (cmd) {
      case 'init': {
        if (payload?.sharedBuffer instanceof SharedArrayBuffer) {
          sharedInputRing = new Int32Array(payload.sharedBuffer);
          Atomics.store(sharedInputRing, 0, 1);
        }
        if (payload?.replicaId) {
          session.set_replica_id(String(payload.replicaId));
        }
        if (payload?.defaultPresetSlug && typeof session.set_page_preset === 'function') {
          session.set_page_preset(1, String(payload.defaultPresetSlug), payload.landscape !== false);
        }
        if (payload?.seedDemo) {
          const s1 = session.drop_builtin_shape_on_page(1, 'BPMN 2.0', 'Start Event', 1.4, 5.4);
          session.set_shape_text(1, s1, 'Order Received');
          const s2 = session.drop_builtin_shape_on_page(1, 'BPMN 2.0', 'Task', 3.9, 5.4);
          session.transform_shape(1, s2, 3.9, 5.4, 2.1, 0.9, 0);
          session.set_shape_text(1, s2, 'Validate Order & Inventory');
          const s3 = session.drop_builtin_shape_on_page(1, 'BPMN 2.0', 'Exclusive Gateway', 6.5, 5.4);
          session.transform_shape(1, s3, 6.5, 5.4, 0.85, 0.85, 0);
          session.set_shape_text(1, s3, 'Authorized?');
          const s4 = session.drop_builtin_shape_on_page(1, 'BPMN 2.0', 'End Event', 9.2, 5.4);
          session.set_shape_text(1, s4, 'Fulfilled');
          session.connect_shapes(1, s1, s2);
          session.connect_shapes(1, s2, s3);
          const c3 = session.connect_shapes(1, s3, s4);
          session.set_shape_text(1, c3, 'Yes');
          session.flush_crdt_ops_json(Date.now());
        }
        const stencils = JSON.parse(session.list_builtin_stencils_json());
        const stencilPreviews = JSON.parse(session.list_builtin_stencil_previews_json());
        const packOrder = JSON.parse(session.list_builtin_pack_order_json());
        self.postMessage({ id, ok: true, stencils, stencilPreviews, packOrder, snapshot: buildSnapshot(session) });
        break;
      }
      case 'importStencil': {
        const res = JSON.parse(
          session.import_stencil_bytes(
            String(payload.fileName ?? 'CustomStencil.vssx'),
            payload.bytes as Uint8Array
          )
        );
        self.postMessage({
          id,
          ok: true,
          packTitle: res.packTitle,
          masterNames: res.masterNames,
          previews: res.previews,
        });
        break;
      }
      case 'flushCrdtOps': {
        const crdtOpsJson = session.flush_crdt_ops_json(Date.now());
        self.postMessage({ id, ok: true, crdtOpsJson });
        break;
      }
      case 'applyRemoteCrdtOps': {
        const applied = session.apply_remote_crdt_ops_json(String(payload.crdtOpsJson ?? '[]'));
        self.postMessage({ id, ok: true, applied, snapshot: buildSnapshot(session) });
        break;
      }
      case 'attachOffscreenCanvas': {
        const attached = await session.attach_offscreen_canvas(
          payload.canvas as OffscreenCanvas,
          Number(payload.width ?? 1056),
          Number(payload.height ?? 816),
          Number(payload.dpi ?? 1)
        );
        self.postMessage({ id, ok: true, attached, snapshot: buildSnapshot(session) });
        break;
      }
      case 'resizeOffscreenCanvas': {
        session.resize_offscreen_canvas(
          Number(payload.width ?? 1056),
          Number(payload.height ?? 816),
          Number(payload.dpi ?? 1)
        );
        const rendered = session.render_offscreen_frame(activePageId, new Uint32Array(selectedShapeIds));
        self.postMessage({ id, ok: true, rendered });
        break;
      }
      case 'renderOffscreenFrame': {
        if (Array.isArray(payload?.selectedIds)) {
          selectedShapeIds = payload.selectedIds.map(Number);
        }
        const rendered = session.render_offscreen_frame(activePageId, new Uint32Array(selectedShapeIds));
        self.postMessage({ id, ok: true, rendered });
        break;
      }
      case 'listStencils': {
        const stencils = JSON.parse(session.list_builtin_stencils_json());
        self.postMessage({ id, ok: true, stencils });
        break;
      }
      case 'loadStarterTemplate': {
        const tplId = String(payload.templateId ?? 'blank');
        if (tplId === 'blank') {
          const curState = JSON.parse(session.get_state_json(activePageId));
          const keepPid = Number(curState.pages?.[0]?.id ?? curState.activePageId ?? 1);
          for (const pg of curState.pages ?? []) {
            if (Number(pg.id) !== keepPid) {
              session.delete_page(Number(pg.id));
            }
          }
          const pState = JSON.parse(session.get_state_json(keepPid));
          for (const s of pState.shapes ?? []) {
            session.delete_shape(keepPid, Number(s.id));
          }
          session.rename_page(keepPid, 'Page-1');
          activePageId = keepPid;
        } else {
          session.load_starter_template(tplId);
          const nextState = JSON.parse(session.get_state_json(0xffffffff));
          activePageId = Number(nextState.activePageId ?? 1);
        }
        latestIssues = [];
        selectedShapeIds = [];
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'openBytes': {
        session.open_bytes(payload.bytes, payload.ext);
        activePageId = 0xffffffff;
        latestIssues = [];
        selectedShapeIds = [];
        self.postMessage({ id, ok: true, snapshot: buildSnapshot(session) });
        break;
      }
      case 'saveFormat': {
        const bytes =
          typeof session.save_page_format === 'function'
            ? session.save_page_format(activePageId, payload.ext)
            : session.save_format(payload.ext);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, bytes, wal }, { transfer: [bytes.buffer, wal.buffer] });
        break;
      }
      case 'hitTest': {
        const hitId = session.hit_test_page(activePageId, payload.xIn, payload.yIn);
        self.postMessage({ id, ok: true, hitId });
        break;
      }
      case 'queryBoxShapes': {
        const ids = Array.from(
          session.query_box_shapes(activePageId, payload.x1, payload.y1, payload.x2, payload.y2)
        );
        selectedShapeIds = ids;
        session.render_offscreen_frame(activePageId, new Uint32Array(selectedShapeIds));
        self.postMessage({ id, ok: true, ids });
        break;
      }
      case 'computeSmartGuides': {
        const guides: SmartGuidesResult = JSON.parse(
          session.compute_smart_guides(
            activePageId,
            Number(payload.shapeId ?? 0),
            Number(payload.candX ?? 0),
            Number(payload.candY ?? 0),
            Number(payload.width ?? 1),
            Number(payload.height ?? 1)
          )
        );
        self.postMessage({ id, ok: true, ...guides });
        break;
      }
      case 'moveShape': {
        const moved = session.move_shape(activePageId, payload.shapeId, payload.xIn, payload.yIn);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, moved, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'moveShapesDelta': {
        const movedCount = session.move_shapes_delta(
          activePageId,
          new Uint32Array(payload.shapeIds ?? []),
          Number(payload.dxIn ?? 0),
          Number(payload.dyIn ?? 0)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, movedCount, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'transformShape': {
        const transformed = session.transform_shape(
          activePageId,
          Number(payload.shapeId),
          Number(payload.pinX),
          Number(payload.pinY),
          Number(payload.width),
          Number(payload.height),
          Number(payload.angleDeg ?? 0)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, transformed, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'alignShapes': {
        const alignedCount = session.align_shapes(
          activePageId,
          new Uint32Array(payload.shapeIds ?? []),
          String(payload.mode ?? 'left')
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, alignedCount, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'applyBooleanOp': {
        const createdIds = Array.from(
          session.apply_boolean_op(
            activePageId,
            new Uint32Array(payload.shapeIds ?? []),
            String(payload.op ?? 'union'),
            Number(payload.offsetIn ?? 0.2)
          )
        );
        selectedShapeIds = createdIds;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, createdIds, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'listShapeVertices': {
        const vertices: EditableVertexDto[] = JSON.parse(
          session.list_shape_vertices_json(activePageId, Number(payload.shapeId))
        );
        self.postMessage({ id, ok: true, vertices });
        break;
      }
      case 'moveShapeVertex': {
        const moved = session.move_shape_vertex(
          activePageId,
          Number(payload.shapeId),
          Number(payload.geomIndex),
          Number(payload.rowIx),
          Number(payload.xIn),
          Number(payload.yIn)
        );
        const vertices: EditableVertexDto[] = JSON.parse(
          session.list_shape_vertices_json(activePageId, Number(payload.shapeId))
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, moved, vertices, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'drawFreeformCurve': {
        const shapeId = session.draw_freeform_curve(
          activePageId,
          JSON.stringify(payload.points ?? []),
          Boolean(payload.closed)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'ensureControlHandle': {
        const okHandle = session.ensure_control_handle(activePageId, Number(payload.shapeId));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, okHandle, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'moveControlHandle': {
        const moved = session.move_control_handle(
          activePageId,
          Number(payload.shapeId),
          String(payload.rowName ?? 'Row_1'),
          Number(payload.xIn),
          Number(payload.yIn)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, moved, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'applyThemeVariant': {
        const count = session.apply_theme_variant(
          String(payload.themeSlug ?? 'modern'),
          Number(payload.variantIndex ?? 0)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'applyShapeVisualPreset': {
        for (const sid of payload.shapeIds ?? [payload.shapeId]) {
          if (sid) {
            session.apply_shape_visual_preset(activePageId, Number(sid), String(payload.preset));
          }
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'insertCroppedImage': {
        const shapeId = session.insert_cropped_image(
          activePageId,
          String(payload.mediaType ?? 'PNG'),
          payload.bytes as Uint8Array,
          Number(payload.xIn ?? 5.5),
          Number(payload.yIn ?? 4.25),
          Number(payload.cropIn ?? 0.08),
          Number(payload.brightness ?? 1.05),
          Number(payload.contrast ?? 1.08)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'createSubprocess': {
        const subPageId = session.create_subprocess_drilldown(
          activePageId,
          Number(payload.shapeId),
          String(payload.subName ?? '')
        );
        if (subPageId > 0 && payload.navigate) {
          activePageId = subPageId;
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, subPageId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setHyperlinkAndScreentip': {
        session.set_hyperlink_and_screentip(
          activePageId,
          Number(payload.shapeId),
          String(payload.description ?? 'Link'),
          String(payload.address ?? ''),
          String(payload.subAddress ?? ''),
          String(payload.screentip ?? '')
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'applyTitleBlockBorder': {
        const bgPageId = session.apply_title_block_border(
          activePageId,
          String(payload.projectCode ?? 'VEX-2026'),
          String(payload.author ?? 'Principal Architect'),
          String(payload.revision ?? 'Rev A')
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, bgPageId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'compareWithVsdx': {
        const report = JSON.parse(session.compare_with_vsdx_bytes(payload.bytes as Uint8Array));
        self.postMessage({ id, ok: true, report });
        break;
      }
      case 'importDataVisualizer': {
        const count = session.import_data_visualizer(activePageId, payload.bytes as Uint8Array);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'exportDataVisualizer': {
        const bytes = session.export_data_visualizer_workbook(activePageId);
        self.postMessage({ id, ok: true, bytes }, { transfer: [bytes.buffer] });
        break;
      }
      case 'importCloudDiscovery': {
        const count = session.import_cloud_discovery(activePageId, String(payload.json ?? ''));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'importEntraOrLdif': {
        const count = session.import_entra_or_ldif(activePageId, String(payload.text ?? ''));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'importSqliteRecordset': {
        const count = session.import_sqlite_recordset(
          activePageId,
          payload.bytes as Uint8Array,
          String(payload.tableName ?? ''),
          String(payload.keyCol ?? 'Label')
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'drawRect': {
        const shapeId = session.draw_rectangle(
          activePageId,
          payload.x1,
          payload.y1,
          payload.x2,
          payload.y2
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'drawPrimitive': {
        const shapeId = session.draw_primitive_2d(
          activePageId,
          String(payload.kind ?? 'rect'),
          Number(payload.x1),
          Number(payload.y1),
          Number(payload.x2),
          Number(payload.y2)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'drawLine': {
        const shapeId = session.draw_line(
          activePageId,
          Number(payload.x1),
          Number(payload.y1),
          Number(payload.x2),
          Number(payload.y2)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'drawArc': {
        const shapeId = session.draw_arc(
          activePageId,
          Number(payload.x1),
          Number(payload.y1),
          Number(payload.x2),
          Number(payload.y2),
          Number(payload.bowIn ?? 0.35)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'connectPointsOrShapes': {
        const connId = session.connect_points_or_shapes(
          activePageId,
          Number(payload.x1),
          Number(payload.y1),
          Number(payload.x2),
          Number(payload.y2),
          Number(payload.fromId ?? 0),
          Number(payload.toId ?? 0)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, connId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'moveConnectorEndpoint': {
        const moved = session.move_connector_endpoint(
          activePageId,
          Number(payload.shapeId),
          Boolean(payload.isBegin),
          Number(payload.xIn),
          Number(payload.yIn),
          Number(payload.targetShapeId ?? 0)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, moved, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'duplicateShapes': {
        const createdIds = Array.from(
          session.duplicate_shapes(
            activePageId,
            new Uint32Array(payload.shapeIds ?? []),
            Number(payload.dxIn ?? 0.35),
            Number(payload.dyIn ?? -0.35)
          )
        );
        selectedShapeIds = createdIds;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, createdIds, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'deleteShape': {
        const idsToDelete: number[] = Array.isArray(payload.shapeIds)
          ? payload.shapeIds
          : [payload.shapeId];
        let deleted = false;
        for (const sid of idsToDelete) {
          if (sid && session.delete_shape(activePageId, sid)) {
            deleted = true;
          }
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, deleted, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'addInkStroke': {
        const strokeId = session.add_ink_stroke(
          activePageId,
          payload.colorHex,
          payload.widthPt,
          Boolean(payload.highlighter),
          JSON.stringify(payload.points)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, strokeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'dropShape': {
        const shapeId = session.drop_builtin_shape_on_page(
          activePageId,
          payload.category,
          payload.masterName,
          payload.xIn,
          payload.yIn
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, shapeId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'connectShapes': {
        const connId = session.connect_shapes(activePageId, payload.fromId, payload.toId);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, connId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setCellFormula': {
        const applied = session.set_cell_formula(
          activePageId,
          payload.shapeId,
          payload.cellName,
          payload.formula,
          Boolean(payload.forceSetf)
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, applied, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setShapeText': {
        session.set_shape_text(activePageId, payload.shapeId, payload.text);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setDocumentTitle': {
        if (typeof session.set_document_title === 'function') {
          session.set_document_title(String(payload.title ?? 'Untitled.vsdx'));
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'applyTextStyle': {
        const ids = new Uint32Array((payload.shapeIds ?? []).map(Number));
        const styleJson =
          typeof payload.styleJson === 'string'
            ? payload.styleJson
            : JSON.stringify(payload.patch ?? {});
        const updated =
          typeof session.apply_text_style_json === 'function'
            ? session.apply_text_style_json(activePageId, ids, styleJson)
            : 0;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, updated, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'rotateFlipShapes': {
        const ids = new Uint32Array((payload.shapeIds ?? []).map(Number));
        const updated =
          typeof session.rotate_flip_shapes === 'function'
            ? session.rotate_flip_shapes(
                activePageId,
                ids,
                Number(payload.deltaAngleDeg ?? 0),
                Boolean(payload.toggleFlipX),
                Boolean(payload.toggleFlipY)
              )
            : 0;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, updated, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setPagePreset': {
        const applied =
          typeof session.set_page_preset === 'function'
            ? session.set_page_preset(
                activePageId,
                String(payload.presetSlug ?? 'a4'),
                payload.landscape !== false
              )
            : false;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, applied, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setPageSetupWithUnit': {
        const applied =
          typeof session.set_page_setup_with_unit === 'function'
            ? session.set_page_setup_with_unit(
                activePageId,
                Number(payload.widthIn ?? 11.6929),
                Number(payload.heightIn ?? 8.2677),
                String(payload.unitSlug ?? 'mm')
              )
            : false;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, applied, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'setPageDimensions': {
        session.set_page_dimensions(
          activePageId,
          payload.widthIn,
          payload.heightIn,
          payload.jumpStyle
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'insertContainer': {
        const cid = session.insert_container(
          activePageId,
          payload.title,
          new Uint32Array(payload.memberIds ?? [])
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, containerId: cid, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'insertCallout': {
        const cid = session.insert_callout(activePageId, payload.targetShapeId, payload.text);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, calloutId: cid, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'insertSwimlanes': {
        session.insert_swimlanes(activePageId, payload.lanes);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'insertRoom': {
        const roomId = session.insert_architectural_room(activePageId, payload.roomName);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, roomId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'insertRack': {
        const rackId = session.insert_rack_with_equipment(activePageId);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, rackId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'importOrgChartCsv': {
        const count = session.import_org_chart_csv(activePageId, String(payload.csv ?? ''));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'generateGanttDemo': {
        const count = session.generate_gantt_demo(activePageId);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'generateTimelineDemo': {
        const count = session.generate_timeline_demo(activePageId);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'reverseSql': {
        const count = session.reverse_engineer_sql(activePageId, payload.sql);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'forwardSql': {
        const sql = session.forward_engineer_sql(activePageId);
        self.postMessage({ id, ok: true, sql });
        break;
      }
      case 'generatePert': {
        const count = session.generate_pert_demo(activePageId);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, count, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'linkCsvDataGraphics': {
        const linked = session.link_csv_and_apply_data_graphics(
          activePageId,
          payload.csv,
          payload.keyCol
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, linked, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'exportBomXlsx': {
        const bytes = session.export_bom_xlsx();
        self.postMessage({ id, ok: true, bytes }, { transfer: [bytes.buffer] });
        break;
      }
      case 'updateLayer': {
        session.update_layer(
          activePageId,
          payload.ix,
          Boolean(payload.visible),
          Boolean(payload.lock),
          payload.colorHex ?? ''
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'addComment': {
        session.add_comment(activePageId, payload.shapeId ?? 0, payload.author, payload.text);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'autoLayout': {
        session.apply_auto_layout(activePageId, payload.algorithm);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'validate': {
        latestIssues = JSON.parse(session.validate_diagram(payload.ruleset));
        self.postMessage({ id, ok: true, snapshot: buildSnapshot(session) });
        break;
      }
      case 'runMacro': {
        const macroReport = JSON.parse(
          session.run_vba_macro_with_selection(
            payload.script,
            new Uint32Array(payload.selectedIds ?? selectedShapeIds)
          )
        );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, macroReport, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'selectPage': {
        if (payload.pageName) {
          const snap = JSON.parse(session.get_state_json(activePageId));
          const found = snap.pages.find(
            (p: { id: number; name: string }) =>
              p.name.toLowerCase() === String(payload.pageName).toLowerCase()
          );
          if (found) activePageId = found.id;
        } else {
          activePageId = payload.pageId;
        }
        self.postMessage({ id, ok: true, snapshot: buildSnapshot(session) });
        break;
      }
      case 'addPage': {
        activePageId = session.add_page(payload.name);
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'renamePage': {
        const okRename = session.rename_page(Number(payload.pageId ?? activePageId), String(payload.name ?? 'Page'));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, renamed: okRename, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'duplicatePage': {
        activePageId = session.duplicate_page(Number(payload.pageId ?? activePageId));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'deletePage': {
        const targetPid = Number(payload.pageId ?? activePageId);
        const deleted = session.delete_page(targetPid);
        const snapRaw = JSON.parse(session.get_state_json(activePageId));
        activePageId = snapRaw.activePageId;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, deleted, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'reorderShapes': {
        const ids = new Uint32Array((payload.shapeIds ?? []).map(Number));
        const moved = session.reorder_shapes(activePageId, ids, String(payload.mode ?? 'front'));
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, moved, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'batchSetCells': {
        const ids: number[] = (payload.shapeIds ?? []).map(Number);
        const entries: Array<{ cell: string; formula: string }> = payload.entries ?? [];
        let changed = 0;
        for (const sid of ids) {
          for (const entry of entries) {
            if (session.set_cell_formula(activePageId, sid, entry.cell, entry.formula, Boolean(payload.forceSetf))) {
              changed++;
            }
          }
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, changed, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'undo': {
        session.undo();
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'redo': {
        session.redo();
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'replayWal': {
        session.replay_wal(payload.bytes);
        self.postMessage({ id, ok: true, snapshot: buildSnapshot(session) });
        break;
      }
      case 'registerFontBytes': {
        const registered =
          typeof session.register_font_bytes === 'function'
            ? session.register_font_bytes(
                payload.bytes as Uint8Array,
                String(payload.familyOverride ?? '')
              )
            : false;
        self.postMessage({ id, ok: true, registered, snapshot: buildSnapshot(session) });
        break;
      }
      case 'distributeShapes': {
        const distributedCount =
          typeof session.distribute_shapes === 'function'
            ? session.distribute_shapes(
                activePageId,
                new Uint32Array(payload.shapeIds ?? []),
                String(payload.axis ?? 'horizontal')
              )
            : session.align_shapes(
                activePageId,
                new Uint32Array(payload.shapeIds ?? []),
                payload.axis === 'vertical' ? 'distribute_v' : 'distribute_h'
              );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, distributedCount, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'groupShapes': {
        const groupId =
          typeof session.group_shapes === 'function'
            ? session.group_shapes(activePageId, new Uint32Array(payload.shapeIds ?? []))
            : 0;
        if (groupId > 0) {
          selectedShapeIds = [groupId];
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, groupId, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'ungroupShapes': {
        const ungroupedIds =
          typeof session.ungroup_shapes === 'function'
            ? Array.from(
                session.ungroup_shapes(activePageId, new Uint32Array(payload.groupIds ?? []))
              )
            : [];
        if (ungroupedIds.length > 0) {
          selectedShapeIds = ungroupedIds;
        }
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, ungroupedIds, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'moveConnectorEndpointWithPort': {
        const moved =
          typeof session.move_connector_endpoint_with_port === 'function'
            ? session.move_connector_endpoint_with_port(
                activePageId,
                Number(payload.shapeId),
                Boolean(payload.isBegin),
                Number(payload.xIn),
                Number(payload.yIn),
                Number(payload.targetShapeId ?? 0),
                String(payload.targetPortCell ?? '')
              )
            : session.move_connector_endpoint(
                activePageId,
                Number(payload.shapeId),
                Boolean(payload.isBegin),
                Number(payload.xIn),
                Number(payload.yIn),
                Number(payload.targetShapeId ?? 0)
              );
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, moved, wal, snapshot: buildSnapshot(session) });
        break;
      }
      case 'dragConnectorSegment': {
        const dragged =
          typeof session.drag_connector_segment === 'function'
            ? session.drag_connector_segment(
                activePageId,
                Number(payload.shapeId),
                Number(payload.segIndex ?? 1),
                Number(payload.coordIn ?? 0)
              )
            : false;
        const wal = session.export_wal();
        self.postMessage({ id, ok: true, dragged, wal, snapshot: buildSnapshot(session) });
        break;
      }
    }
  } catch (err) {
    self.postMessage({
      id,
      ok: false,
      error: err instanceof Error ? err.message : String(err),
    });
  }
};
