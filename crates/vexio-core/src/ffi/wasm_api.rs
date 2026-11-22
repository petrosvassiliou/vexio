//! `wasm-bindgen` FFI surface for Target D: Firebase Offline-First ChromeOS PWA.
//!
//! Exposes 100% of `vexio-core` & `vexio-render` to the ChromeOS Web Worker engine:
//! - Hardware-accelerated `wgpu::SurfaceTarget::OffscreenCanvas` WebGPU / WebGL2 viewport
//! - R*-tree spatial point & marquee lasso hit-testing, 8-handle resize/rotate, batch multi-move,
//!   Align/Distribute, and Dynamic Grid Smart Alignment Snap Guides
//! - Pressure- & tilt-sensitive ChromeOS USI Active Stylus Markup Ink
//! - 12 Built-in Domain Stencil Packs, 8 Backstage Starter Templates, Org Chart CSV Wizard,
//!   Gantt Chart Builder, Project Timeline Builder, SQL ERD Studio, PERT CPM, Two-Way Data Graphics,
//!   BOM `.xlsx` Reports, Layer Manager, Collaboration Comments, and Sandboxed VBA Automation.

#![cfg(target_arch = "wasm32")]

use crate::boolean_ops::{
    BooleanOpKind, ControlHandleInfo, apply_boolean_operation, draw_freeform_curve,
    ensure_default_control_handle, list_control_handles, list_shape_vertices, move_control_handle,
    move_shape_vertex,
};
use crate::containers::{attach_callout, build_cross_functional_swimlanes, create_container};
use crate::crdt::CrdtDocumentState;
use crate::data_link::{
    DataGraphicItem, DataGraphicSpec, DataRecordset, apply_data_graphics, bind_recordset_to_page,
    generate_bom_shape_report, insert_page_legend,
};
use crate::domains::{
    GanttTaskInput, PertTaskInput, TimelineMilestone, build_gantt_chart, build_org_chart_from_csv,
    build_pert_critical_path_diagram, build_project_timeline, create_architectural_room_space,
    forward_engineer_sql_ddl, instantiate_starter_template, mount_rack_equipment,
    reverse_engineer_sql_ddl,
};
use crate::enterprise_plan2::{
    add_shape_hyperlink, apply_iso_title_block_background, compare_documents,
    create_or_link_subprocess_page, set_shape_screentip,
};
use crate::export_print::export_page_svg;
use crate::journal::{CommandJournal, DocCommand};
use crate::plan2_connectors::{
    export_data_visualizer_xlsx, import_cloud_discovery_json, import_data_visualizer_csv_or_xlsx,
    import_entra_or_ldif_org_chart,
};
use crate::routing::route_page_connectors;
use crate::sqlite_reader::{list_sqlite_tables, read_sqlite_table_to_recordset};
use crate::themes_styles::{
    BevelExtrusionSpec, DropShadowSpec, GradientDirection, GradientFillSpec, HatchFillSpec,
    HatchPatternKind, ShapeVisualStyleSpec, SketchOutlineSpec, apply_document_theme_variant,
    apply_visual_effects_to_shape,
};
use crate::workspace::DocumentSession;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vexio_shapesheet::{
    CommentAnnotation, EvaluatedPathVerb, EvaluatedShapeVisual, ForeignMediaData, MarkupInkPoint,
    MarkupInkStroke, ReactiveEngine, RgbaColor, ShapeKind, Unit, VisioDocument, VisioShape,
    format_clean_f64,
};
use wasm_bindgen::prelude::*;

fn sample_visual_polyline(vis: &EvaluatedShapeVisual) -> Vec<[f64; 2]> {
    let mut pts: Vec<[f64; 2]> = Vec::new();
    for path in &vis.paths {
        if path.no_show {
            continue;
        }
        let mut cur = [0.0, 0.0];
        let mut has_cur = false;
        for verb in &path.verbs_page_space {
            match *verb {
                EvaluatedPathVerb::MoveTo(p) => {
                    pts.push(p);
                    cur = p;
                    has_cur = true;
                }
                EvaluatedPathVerb::LineTo(to) => {
                    pts.push(to);
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::QuadTo { ctrl, to } => {
                    if has_cur {
                        for step in 1..=8 {
                            let t = step as f64 / 8.0;
                            let mt = 1.0 - t;
                            pts.push([
                                mt * mt * cur[0] + 2.0 * mt * t * ctrl[0] + t * t * to[0],
                                mt * mt * cur[1] + 2.0 * mt * t * ctrl[1] + t * t * to[1],
                            ]);
                        }
                    } else {
                        pts.push(to);
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                    if has_cur {
                        for step in 1..=10 {
                            let t = step as f64 / 10.0;
                            let mt = 1.0 - t;
                            pts.push([
                                mt * mt * mt * cur[0]
                                    + 3.0 * mt * mt * t * ctrl1[0]
                                    + 3.0 * mt * t * t * ctrl2[0]
                                    + t * t * t * to[0],
                                mt * mt * mt * cur[1]
                                    + 3.0 * mt * mt * t * ctrl1[1]
                                    + 3.0 * mt * t * t * ctrl2[1]
                                    + t * t * t * to[1],
                            ]);
                        }
                    } else {
                        pts.push(to);
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::ArcTo { to, bow } => {
                    if has_cur {
                        let mx = (cur[0] + to[0]) * 0.5;
                        let my = (cur[1] + to[1]) * 0.5;
                        let dx = to[0] - cur[0];
                        let dy = to[1] - cur[1];
                        let chord = (dx * dx + dy * dy).sqrt();
                        let ctrl = if chord > 1e-9 {
                            let nx = -dy / chord;
                            let ny = dx / chord;
                            [mx + nx * bow * 2.0, my + ny * bow * 2.0]
                        } else {
                            [mx, my]
                        };
                        for step in 1..=10 {
                            let t = step as f64 / 10.0;
                            let mt = 1.0 - t;
                            pts.push([
                                mt * mt * cur[0] + 2.0 * mt * t * ctrl[0] + t * t * to[0],
                                mt * mt * cur[1] + 2.0 * mt * t * ctrl[1] + t * t * to[1],
                            ]);
                        }
                    } else {
                        pts.push(to);
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::Close | EvaluatedPathVerb::Ellipse { .. } => {}
            }
        }
    }
    pts
}

#[wasm_bindgen]
pub struct WasmVexioSession {
    inner: DocumentSession,
    wgpu_renderer: Option<vexio_render::WgpuCanvasRenderer>,
    crdt_state: CrdtDocumentState,
    crdt_prev_doc: VisioDocument,
}

#[derive(Serialize)]
struct WasmCellInfo {
    name: String,
    formula: String,
    value: String,
    guarded: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmShapeInfo {
    id: u64,
    name: String,
    kind: String,
    text: String,
    pin_x: f64,
    pin_y: f64,
    width: f64,
    height: f64,
    angle_deg: f64,
    flip_x: bool,
    flip_y: bool,
    font_family: String,
    font_size_pt: f64,
    char_style: u8,
    text_color: String,
    text_bg_color: Option<String>,
    horiz_align: u8,
    vert_align: u8,
    bullet_style: u8,
    begin_x: Option<f64>,
    begin_y: Option<f64>,
    end_x: Option<f64>,
    end_y: Option<f64>,
    path_points: Vec<[f64; 2]>,
    ports: Vec<[f64; 2]>,
    route_style: i32,
    begin_arrow: i32,
    end_arrow: i32,
    line_weight_pt: f64,
    line_pattern: i32,
    subprocess_page: Option<String>,
    screentip: Option<String>,
    control_handles: Vec<ControlHandleInfo>,
    cells: Vec<WasmCellInfo>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmPageInfo {
    id: u32,
    name: String,
    width_in: f64,
    height_in: f64,
    shape_count: usize,
    is_background: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmLayerInfo {
    ix: u32,
    name: String,
    visible: bool,
    print: bool,
    lock: bool,
    color_override: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmCommentInfo {
    id: u32,
    author: String,
    text: String,
    page_id: u32,
    shape_id: Option<u64>,
    resolved: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmStateJson {
    title: String,
    active_page_id: u32,
    page_width_in: f64,
    page_height_in: f64,
    display_unit: String,
    pages: Vec<WasmPageInfo>,
    layers: Vec<WasmLayerInfo>,
    comments: Vec<WasmCommentInfo>,
    shapes: Vec<WasmShapeInfo>,
    svg_preview: String,
    can_undo: bool,
    can_redo: bool,
    wgpu_surface_active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WasmIssueJson {
    id: u32,
    code: String,
    severity: String,
    category: String,
    description: String,
    shape_id: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SmartGuideLine {
    axis: &'static str, // "x" (vertical line at x=pos_in) or "y" (horizontal line at y=pos_in)
    pos_in: f64,
    from_in: f64,
    to_in: f64,
    kind: &'static str, // "center" | "edge"
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SmartGuidesResponse {
    snapped_x: f64,
    snapped_y: f64,
    guides: Vec<SmartGuideLine>,
}

#[derive(Deserialize)]
struct InkPointDto {
    x: f64,
    y: f64,
    pressure: Option<f32>,
    tilt_x: Option<f32>,
    tilt_y: Option<f32>,
}

#[wasm_bindgen]
impl WasmVexioSession {
    #[wasm_bindgen(constructor)]
    pub fn new(title: &str) -> Self {
        let inner = DocumentSession::new_blank(title);
        let crdt_prev_doc = inner.doc.clone();
        Self {
            inner,
            wgpu_renderer: None,
            crdt_state: CrdtDocumentState::new("wasm-peer"),
            crdt_prev_doc,
        }
    }

    // --- Live WebGPU / WebGL2 OffscreenCanvas Surface Binding ---

    pub async fn attach_offscreen_canvas(
        &mut self,
        canvas: web_sys::OffscreenCanvas,
        width: u32,
        height: u32,
        dpi: f64,
    ) -> bool {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });
        let Ok(surface) = instance.create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas))
        else {
            return false;
        };
        let Ok(mut renderer) = vexio_render::WgpuCanvasRenderer::new_with_surface_async(
            instance,
            surface,
            width.max(1),
            height.max(1),
            4,
        )
        .await
        else {
            return false;
        };
        renderer.camera.dpi_scale = dpi.max(0.5);
        if let Some(p) = self
            .inner
            .doc
            .pages
            .iter()
            .find(|p| !p.is_background)
            .or_else(|| self.inner.doc.pages.first())
        {
            renderer.camera.fit_to_page(p.width_in(), p.height_in());
            let _ = renderer.render_surface_frame(&self.inner.doc, p.id);
        }
        self.wgpu_renderer = Some(renderer);
        true
    }

    pub fn resize_offscreen_canvas(&mut self, width: u32, height: u32, dpi: f64) {
        if let Some(r) = &mut self.wgpu_renderer {
            r.resize_surface(width.max(1), height.max(1), dpi.max(0.5));
        }
    }

    pub fn render_offscreen_frame(&mut self, page_id: u32, selected_ids: &[u32]) -> bool {
        let Some(r) = &mut self.wgpu_renderer else {
            return false;
        };
        let resolved_pid = if self.inner.doc.page(page_id).is_some() {
            page_id
        } else {
            self.inner
                .doc
                .pages
                .iter()
                .find(|p| !p.is_background)
                .or_else(|| self.inner.doc.pages.first())
                .map(|p| p.id)
                .unwrap_or(page_id)
        };
        if let Some(p) = self.inner.doc.page(resolved_pid) {
            r.camera.fit_to_page(p.width_in(), p.height_in());
        }
        r.selected_shapes = selected_ids.iter().map(|&i| i as u64).collect();
        r.render_surface_frame(&self.inner.doc, resolved_pid).is_ok()
    }

    // --- Stencils & Backstage Templates ---

    pub fn list_builtin_stencils_json(&self) -> String {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for pack in &self.inner.builtin_stencils {
            map.insert(
                pack.title.clone(),
                pack.masters.iter().map(|m| m.name.clone()).collect(),
            );
        }
        serde_json::to_string(&map).unwrap_or_else(|_| "{}".into())
    }

    pub fn list_builtin_stencil_previews_json(&self) -> String {
        let mut previews = crate::domains::list_builtin_stencil_previews();
        for (cat, list) in &self.inner.imported_stencil_previews {
            previews.insert(cat.clone(), list.clone());
        }
        serde_json::to_string(&previews).unwrap_or_else(|_| "{}".into())
    }

    pub fn list_builtin_pack_order_json(&self) -> String {
        let order: Vec<String> = self
            .inner
            .builtin_stencils
            .iter()
            .map(|p| p.title.clone())
            .collect();
        serde_json::to_string(&order).unwrap_or_else(|_| "[]".into())
    }

    pub fn load_starter_template(&mut self, template_id: &str) {
        self.inner.doc = instantiate_starter_template(template_id);
        self.inner.journal = CommandJournal::new();
        self.crdt_prev_doc = self.inner.doc.clone();
    }

    pub fn open_bytes(&mut self, bytes: &[u8], ext: &str) -> Result<(), JsValue> {
        self.inner
            .open_bytes(bytes, ext)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.crdt_prev_doc = self.inner.doc.clone();
        Ok(())
    }

    pub fn save_format(&self, ext: &str) -> Result<Vec<u8>, JsValue> {
        self.inner
            .save_format(ext)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn save_page_format(&self, page_id: u32, ext: &str) -> Result<Vec<u8>, JsValue> {
        self.inner
            .save_page_format(page_id, ext)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn drop_builtin_shape(
        &mut self,
        category: &str,
        master_name: &str,
        x_in: f64,
        y_in: f64,
    ) -> Result<u32, JsValue> {
        let pid = self.inner.doc.pages.first().map(|p| p.id).unwrap_or(1);
        self.drop_builtin_shape_on_page(pid, category, master_name, x_in, y_in)
    }

    pub fn drop_builtin_shape_on_page(
        &mut self,
        page_id: u32,
        category: &str,
        master_name: &str,
        x_in: f64,
        y_in: f64,
    ) -> Result<u32, JsValue> {
        self.inner
            .drop_builtin_shape(page_id, category, master_name, x_in, y_in)
            .map(|id| id as u32)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // --- Spatial Hit-Testing, Marquee Lasso Multi-Select, 8-Handle Resize/Rotate & Smart Guides ---

    pub fn hit_test_page(&self, page_id: u32, x_in: f64, y_in: f64) -> u32 {
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.inner.doc, page_id);
        let index = vexio_render::SpatialIndex::build(&visuals);
        index
            .hit_test_visuals(&visuals, x_in, y_in, 0.08)
            .into_iter()
            .next()
            .unwrap_or(0) as u32
    }

    /// Marquee Lasso Multi-Select via `O(log N)` R*-tree viewport query.
    pub fn query_box_shapes(
        &self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
    ) -> Vec<u32> {
        let min_pt = [x1_in.min(x2_in), y1_in.min(y2_in)];
        let max_pt = [x1_in.max(x2_in), y1_in.max(y2_in)];
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.inner.doc, page_id);
        let index = vexio_render::SpatialIndex::build(&visuals);
        let Some(page) = self.inner.doc.page(page_id) else {
            return Vec::new();
        };
        index
            .query_viewport(min_pt, max_pt)
            .into_iter()
            .filter(|&id| {
                page.find_shape(id)
                    .map(|s| s.kind == ShapeKind::Shape2D)
                    .unwrap_or(false)
            })
            .map(|id| id as u32)
            .collect()
    }

    /// Compute Dynamic Grid Smart Alignment Guides (center-X, center-Y, left/right/top/bottom edges)
    /// against sibling 2D shapes on `page_id` within `0.10 in` magnetic threshold.
    pub fn compute_smart_guides(
        &self,
        page_id: u32,
        moving_shape_id: u32,
        cand_x_in: f64,
        cand_y_in: f64,
        width_in: f64,
        height_in: f64,
    ) -> String {
        let tol = 0.10;
        let mut snapped_x = cand_x_in;
        let mut snapped_y = cand_y_in;
        let mut guides = Vec::new();

        if let Some(page) = self.inner.doc.page(page_id) {
            let mut best_dx = tol;
            let mut best_dy = tol;
            let mut gx: Option<SmartGuideLine> = None;
            let mut gy: Option<SmartGuideLine> = None;

            let my_xs = [
                (cand_x_in, "center"),
                (cand_x_in - width_in * 0.5, "edge"),
                (cand_x_in + width_in * 0.5, "edge"),
            ];
            let my_ys = [
                (cand_y_in, "center"),
                (cand_y_in - height_in * 0.5, "edge"),
                (cand_y_in + height_in * 0.5, "edge"),
            ];

            for other in &page.shapes {
                if other.id == moving_shape_id as u64 || other.kind != ShapeKind::Shape2D {
                    continue;
                }
                let ox = other.sheet.get_f64("PinX").unwrap_or(0.0);
                let oy = other.sheet.get_f64("PinY").unwrap_or(0.0);
                let ow = other.sheet.get_f64("Width").unwrap_or(1.0);
                let oh = other.sheet.get_f64("Height").unwrap_or(1.0);

                let target_xs = [ox, ox - ow * 0.5, ox + ow * 0.5];
                for &(mx, mkind) in &my_xs {
                    for &tx in &target_xs {
                        let d = (tx - mx).abs();
                        if d < best_dx {
                            best_dx = d;
                            snapped_x = cand_x_in + (tx - mx);
                            gx = Some(SmartGuideLine {
                                axis: "x",
                                pos_in: tx,
                                from_in: (snapped_y - height_in * 0.5).min(oy - oh * 0.5) - 0.25,
                                to_in: (snapped_y + height_in * 0.5).max(oy + oh * 0.5) + 0.25,
                                kind: mkind,
                            });
                        }
                    }
                }

                let target_ys = [oy, oy - oh * 0.5, oy + oh * 0.5];
                for &(my, mkind) in &my_ys {
                    for &ty in &target_ys {
                        let d = (ty - my).abs();
                        if d < best_dy {
                            best_dy = d;
                            snapped_y = cand_y_in + (ty - my);
                            gy = Some(SmartGuideLine {
                                axis: "y",
                                pos_in: ty,
                                from_in: (snapped_x - width_in * 0.5).min(ox - ow * 0.5) - 0.25,
                                to_in: (snapped_x + width_in * 0.5).max(ox + ow * 0.5) + 0.25,
                                kind: mkind,
                            });
                        }
                    }
                }
            }

            if let Some(g) = gx {
                guides.push(g);
            }
            if let Some(g) = gy {
                guides.push(g);
            }
        }

        serde_json::to_string(&SmartGuidesResponse {
            snapped_x,
            snapped_y,
            guides,
        })
        .unwrap_or_else(|_| "{}".into())
    }

    pub fn move_shape(
        &mut self,
        page_id: u32,
        shape_id: u32,
        new_pin_x_in: f64,
        new_pin_y_in: f64,
    ) -> bool {
        let sid = shape_id as u64;
        let Some(sh) = self.inner.doc.page(page_id).and_then(|p| p.find_shape(sid)) else {
            return false;
        };
        if sh.sheet.get_bool("LockMoveX").unwrap_or(false)
            || sh.sheet.get_bool("LockMoveY").unwrap_or(false)
        {
            return false;
        }
        if sh.kind == ShapeKind::Connector1D {
            let cur_px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
            let cur_py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
            let dx = new_pin_x_in - cur_px;
            let dy = new_pin_y_in - cur_py;
            let bx = sh.sheet.get_f64("BeginX").unwrap_or(0.0) + dx;
            let by = sh.sheet.get_f64("BeginY").unwrap_or(0.0) + dy;
            let ex = sh.sheet.get_f64("EndX").unwrap_or(1.0) + dx;
            let ey = sh.sheet.get_f64("EndY").unwrap_or(0.0) + dy;
            let old_bx = sh.sheet.get_cell("BeginX").map(|c| c.formula.clone()).unwrap_or_default();
            let old_by = sh.sheet.get_cell("BeginY").map(|c| c.formula.clone()).unwrap_or_default();
            let old_ex = sh.sheet.get_cell("EndX").map(|c| c.formula.clone()).unwrap_or_default();
            let old_ey = sh.sheet.get_cell("EndY").map(|c| c.formula.clone()).unwrap_or_default();
            let batch = DocCommand::Batch {
                label: "Move 1D Shape".into(),
                commands: vec![
                    DocCommand::SetCellFormula {
                        page_id,
                        shape_id: sid,
                        cell_name: "BeginX".into(),
                        old_formula: old_bx,
                        new_formula: format!("{} in", format_clean_f64(bx)),
                        force_setf: false,
                    },
                    DocCommand::SetCellFormula {
                        page_id,
                        shape_id: sid,
                        cell_name: "BeginY".into(),
                        old_formula: old_by,
                        new_formula: format!("{} in", format_clean_f64(by)),
                        force_setf: false,
                    },
                    DocCommand::SetCellFormula {
                        page_id,
                        shape_id: sid,
                        cell_name: "EndX".into(),
                        old_formula: old_ex,
                        new_formula: format!("{} in", format_clean_f64(ex)),
                        force_setf: false,
                    },
                    DocCommand::SetCellFormula {
                        page_id,
                        shape_id: sid,
                        cell_name: "EndY".into(),
                        old_formula: old_ey,
                        new_formula: format!("{} in", format_clean_f64(ey)),
                        force_setf: false,
                    },
                ],
            };
            return self.inner.journal.execute(&mut self.inner.doc, batch).is_ok();
        }
        let old_x = sh
            .sheet
            .get_cell("PinX")
            .map(|c| c.formula.clone())
            .unwrap_or_else(|| "0 in".into());
        let old_y = sh
            .sheet
            .get_cell("PinY")
            .map(|c| c.formula.clone())
            .unwrap_or_else(|| "0 in".into());
        let batch = DocCommand::Batch {
            label: "Move Shape".into(),
            commands: vec![
                DocCommand::SetCellFormula {
                    page_id,
                    shape_id: sid,
                    cell_name: "PinX".into(),
                    old_formula: old_x,
                    new_formula: format!("{} in", format_clean_f64(new_pin_x_in)),
                    force_setf: false,
                },
                DocCommand::SetCellFormula {
                    page_id,
                    shape_id: sid,
                    cell_name: "PinY".into(),
                    old_formula: old_y,
                    new_formula: format!("{} in", format_clean_f64(new_pin_y_in)),
                    force_setf: false,
                },
            ],
        };
        if self.inner.journal.execute(&mut self.inner.doc, batch).is_ok() {
            route_page_connectors(&mut self.inner.doc, page_id);
            true
        } else {
            false
        }
    }

    /// Batch-move multiple lasso-selected shapes atomically in a single Undo/Redo transaction.
    pub fn move_shapes_delta(
        &mut self,
        page_id: u32,
        shape_ids: &[u32],
        dx_in: f64,
        dy_in: f64,
    ) -> usize {
        let Some(page) = self.inner.doc.page(page_id) else {
            return 0;
        };
        let mut cmds = Vec::new();
        for &id32 in shape_ids {
            let sid = id32 as u64;
            let Some(sh) = page.find_shape(sid) else {
                continue;
            };
            if sh.kind != ShapeKind::Shape2D
                || sh.sheet.get_bool("LockMoveX").unwrap_or(false)
                || sh.sheet.get_bool("LockMoveY").unwrap_or(false)
            {
                continue;
            }
            let cur_x = sh.sheet.get_f64("PinX").unwrap_or(0.0);
            let cur_y = sh.sheet.get_f64("PinY").unwrap_or(0.0);
            let old_x = sh.sheet.get_cell("PinX").map(|c| c.formula.clone()).unwrap_or_default();
            let old_y = sh.sheet.get_cell("PinY").map(|c| c.formula.clone()).unwrap_or_default();
            cmds.push(DocCommand::SetCellFormula {
                page_id,
                shape_id: sid,
                cell_name: "PinX".into(),
                old_formula: old_x,
                new_formula: format!("{} in", format_clean_f64(cur_x + dx_in)),
                force_setf: false,
            });
            cmds.push(DocCommand::SetCellFormula {
                page_id,
                shape_id: sid,
                cell_name: "PinY".into(),
                old_formula: old_y,
                new_formula: format!("{} in", format_clean_f64(cur_y + dy_in)),
                force_setf: false,
            });
        }
        let moved = cmds.len() / 2;
        if moved > 0
            && self
                .inner
                .journal
                .execute(
                    &mut self.inner.doc,
                    DocCommand::Batch {
                        label: "Batch Move Shapes".into(),
                        commands: cmds,
                    },
                )
                .is_ok()
        {
            route_page_connectors(&mut self.inner.doc, page_id);
            moved
        } else {
            0
        }
    }

    /// 8-Handle Interactive Resize + Top Rotation Knob transaction respecting `LockWidth`,
    /// `LockHeight`, `LockAspect`, `LockRotate`, and `GUARD`.
    #[allow(clippy::too_many_arguments)]
    pub fn transform_shape(
        &mut self,
        page_id: u32,
        shape_id: u32,
        pin_x_in: f64,
        pin_y_in: f64,
        width_in: f64,
        height_in: f64,
        angle_deg: f64,
    ) -> bool {
        self.inner
            .transform_shape(
                page_id,
                shape_id as u64,
                pin_x_in,
                pin_y_in,
                width_in,
                height_in,
                angle_deg,
            )
            .is_ok()
    }

    /// Import an external `.vssx`, `.vssm`, `.vss`, `.vsdx`, or `.vdx` Visio stencil file
    /// directly into the Stencil Navigator with lossless geometry, `ForeignData`, and SVG thumbnails.
    pub fn import_stencil_bytes(&mut self, file_name: &str, bytes: &[u8]) -> Result<String, JsValue> {
        let (pack_title, previews) = self
            .inner
            .import_stencil_package(file_name, bytes)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let master_names: Vec<String> = previews.iter().map(|p| p.name.clone()).collect();
        let resp = serde_json::json!({
            "packTitle": pack_title,
            "masterNames": master_names,
            "previews": previews,
        });
        Ok(serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into()))
    }

    /// Multi-shape Align (`left`, `center`, `right`, `top`, `middle`, `bottom`) and
    /// Distribute (`distribute_h`, `distribute_v`) matching Visio Home -> Position!
    pub fn align_shapes(&mut self, page_id: u32, shape_ids: &[u32], mode: &str) -> usize {
        if shape_ids.len() < 2 {
            return 0;
        }
        let Some(page) = self.inner.doc.page(page_id) else {
            return 0;
        };
        let mut items: Vec<(u64, f64, f64, f64, f64, String, String)> = shape_ids
            .iter()
            .filter_map(|&id32| {
                let sh = page.find_shape(id32 as u64)?;
                if sh.kind != ShapeKind::Shape2D {
                    return None;
                }
                Some((
                    sh.id,
                    sh.sheet.get_f64("PinX").unwrap_or(0.0),
                    sh.sheet.get_f64("PinY").unwrap_or(0.0),
                    sh.sheet.get_f64("Width").unwrap_or(1.0),
                    sh.sheet.get_f64("Height").unwrap_or(1.0),
                    sh.sheet.get_cell("PinX").map(|c| c.formula.clone()).unwrap_or_default(),
                    sh.sheet.get_cell("PinY").map(|c| c.formula.clone()).unwrap_or_default(),
                ))
            })
            .collect();

        if items.len() < 2 {
            return 0;
        }

        let min_left = items.iter().map(|i| i.1 - i.3 * 0.5).fold(f64::INFINITY, f64::min);
        let max_right = items.iter().map(|i| i.1 + i.3 * 0.5).fold(f64::NEG_INFINITY, f64::max);
        let min_bot = items.iter().map(|i| i.2 - i.4 * 0.5).fold(f64::INFINITY, f64::min);
        let max_top = items.iter().map(|i| i.2 + i.4 * 0.5).fold(f64::NEG_INFINITY, f64::max);
        let mid_x = (min_left + max_right) * 0.5;
        let mid_y = (min_bot + max_top) * 0.5;

        let mut cmds = Vec::new();
        match mode {
            "distribute_h" if items.len() >= 3 => {
                items.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
                let first_x = items.first().unwrap().1;
                let last_x = items.last().unwrap().1;
                let step = (last_x - first_x) / (items.len() - 1) as f64;
                for (idx, it) in items.iter().enumerate() {
                    let nx = first_x + idx as f64 * step;
                    cmds.push(DocCommand::SetCellFormula {
                        page_id,
                        shape_id: it.0,
                        cell_name: "PinX".into(),
                        old_formula: it.5.clone(),
                        new_formula: format!("{} in", format_clean_f64(nx)),
                        force_setf: false,
                    });
                }
            }
            "distribute_v" if items.len() >= 3 => {
                items.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
                let first_y = items.first().unwrap().2;
                let last_y = items.last().unwrap().2;
                let step = (last_y - first_y) / (items.len() - 1) as f64;
                for (idx, it) in items.iter().enumerate() {
                    let ny = first_y + idx as f64 * step;
                    cmds.push(DocCommand::SetCellFormula {
                        page_id,
                        shape_id: it.0,
                        cell_name: "PinY".into(),
                        old_formula: it.6.clone(),
                        new_formula: format!("{} in", format_clean_f64(ny)),
                        force_setf: false,
                    });
                }
            }
            _ => {
                for it in &items {
                    match mode {
                        "left" => cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: it.0,
                            cell_name: "PinX".into(),
                            old_formula: it.5.clone(),
                            new_formula: format!("{} in", format_clean_f64(min_left + it.3 * 0.5)),
                            force_setf: false,
                        }),
                        "center" => cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: it.0,
                            cell_name: "PinX".into(),
                            old_formula: it.5.clone(),
                            new_formula: format!("{} in", format_clean_f64(mid_x)),
                            force_setf: false,
                        }),
                        "right" => cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: it.0,
                            cell_name: "PinX".into(),
                            old_formula: it.5.clone(),
                            new_formula: format!("{} in", format_clean_f64(max_right - it.3 * 0.5)),
                            force_setf: false,
                        }),
                        "top" => cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: it.0,
                            cell_name: "PinY".into(),
                            old_formula: it.6.clone(),
                            new_formula: format!("{} in", format_clean_f64(max_top - it.4 * 0.5)),
                            force_setf: false,
                        }),
                        "middle" => cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: it.0,
                            cell_name: "PinY".into(),
                            old_formula: it.6.clone(),
                            new_formula: format!("{} in", format_clean_f64(mid_y)),
                            force_setf: false,
                        }),
                        "bottom" => cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: it.0,
                            cell_name: "PinY".into(),
                            old_formula: it.6.clone(),
                            new_formula: format!("{} in", format_clean_f64(min_bot + it.4 * 0.5)),
                            force_setf: false,
                        }),
                        _ => {}
                    }
                }
            }
        }

        let count = cmds.len();
        if count > 0
            && self
                .inner
                .journal
                .execute(
                    &mut self.inner.doc,
                    DocCommand::Batch {
                        label: format!("Align {mode}"),
                        commands: cmds,
                    },
                )
                .is_ok()
        {
            route_page_connectors(&mut self.inner.doc, page_id);
            count
        } else {
            0
        }
    }

    pub fn draw_rectangle(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
    ) -> Result<u32, JsValue> {
        self.draw_primitive_2d(page_id, "rect", x1_in, y1_in, x2_in, y2_in)
    }

    pub fn draw_primitive_2d(
        &mut self,
        page_id: u32,
        kind: &str,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
    ) -> Result<u32, JsValue> {
        self.inner
            .draw_primitive_2d(page_id, kind, x1_in, y1_in, x2_in, y2_in)
            .map(|id| id as u32)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn draw_line(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
    ) -> Result<u32, JsValue> {
        self.inner
            .draw_line(page_id, x1_in, y1_in, x2_in, y2_in)
            .map(|id| id as u32)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn draw_arc(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
        bow_in: f64,
    ) -> Result<u32, JsValue> {
        self.inner
            .draw_arc(page_id, x1_in, y1_in, x2_in, y2_in, bow_in)
            .map(|id| id as u32)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn connect_points_or_shapes(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
        from_shape_id: u32,
        to_shape_id: u32,
    ) -> Result<u32, JsValue> {
        self.inner
            .connect_points_or_shapes(
                page_id,
                x1_in,
                y1_in,
                x2_in,
                y2_in,
                if from_shape_id > 0 {
                    Some(from_shape_id as u64)
                } else {
                    None
                },
                if to_shape_id > 0 {
                    Some(to_shape_id as u64)
                } else {
                    None
                },
            )
            .map(|id| id as u32)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn move_connector_endpoint(
        &mut self,
        page_id: u32,
        shape_id: u32,
        is_begin: bool,
        x_in: f64,
        y_in: f64,
        target_shape_id: u32,
    ) -> Result<bool, JsValue> {
        self.inner
            .move_connector_endpoint(
                page_id,
                shape_id as u64,
                is_begin,
                x_in,
                y_in,
                if target_shape_id > 0 {
                    Some(target_shape_id as u64)
                } else {
                    None
                },
            )
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn duplicate_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u32],
        dx_in: f64,
        dy_in: f64,
    ) -> Vec<u32> {
        let ids64: Vec<u64> = shape_ids.iter().map(|&i| i as u64).collect();
        self.inner
            .duplicate_shapes(page_id, &ids64, dx_in, dy_in)
            .unwrap_or_default()
            .into_iter()
            .map(|id| id as u32)
            .collect()
    }

    pub fn delete_shape(&mut self, page_id: u32, shape_id: u32) -> bool {
        self.inner
            .delete_shapes(page_id, &[shape_id as u64])
            .map(|n| n > 0)
            .unwrap_or(false)
    }

    pub fn add_ink_stroke(
        &mut self,
        page_id: u32,
        color_hex: &str,
        width_pt: f32,
        highlighter: bool,
        points_json: &str,
    ) -> Result<u32, JsValue> {
        let dto: Vec<InkPointDto> =
            serde_json::from_str(points_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        if dto.len() < 2 {
            return Ok(0);
        }
        let stroke_id = self
            .inner
            .doc
            .page(page_id)
            .map(|p| (p.ink_strokes.len() as u64) + 1)
            .unwrap_or(1);
        let color = RgbaColor::from_hex(color_hex).unwrap_or(RgbaColor {
            r: 37,
            g: 99,
            b: 235,
            a: 255,
        });
        let stroke = MarkupInkStroke {
            id: stroke_id,
            color,
            width_pt,
            highlighter,
            points: dto
                .into_iter()
                .map(|p| MarkupInkPoint {
                    x_in: p.x,
                    y_in: p.y,
                    pressure: p.pressure.unwrap_or(0.5),
                    tilt_x: p.tilt_x.unwrap_or(0.0),
                    tilt_y: p.tilt_y.unwrap_or(0.0),
                })
                .collect(),
        };
        self.inner
            .journal
            .execute(&mut self.inner.doc, DocCommand::AddInkStroke { page_id, stroke })
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(stroke_id as u32)
    }

    pub fn connect_shapes(&mut self, page_id: u32, from_id: u32, to_id: u32) -> Result<u32, JsValue> {
        self.inner
            .connect_shapes(page_id, from_id as u64, to_id as u64)
            .map(|id| id as u32)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn set_cell_formula(
        &mut self,
        page_id: u32,
        shape_id: u32,
        cell_name: &str,
        formula: &str,
        force_setf: bool,
    ) -> bool {
        self.inner
            .set_cell_formula(page_id, shape_id as u64, cell_name, formula, force_setf)
            .is_ok()
    }

    pub fn set_shape_text(&mut self, page_id: u32, shape_id: u32, text: &str) -> Result<(), JsValue> {
        self.inner
            .set_shape_text(page_id, shape_id as u64, text)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn set_document_title(&mut self, title: &str) {
        self.inner.set_document_title(title);
    }

    pub fn apply_text_style_json(
        &mut self,
        page_id: u32,
        shape_ids: &[u32],
        style_json: &str,
    ) -> usize {
        let Ok(patch) = serde_json::from_str::<crate::workspace::TextStylePatch>(style_json) else {
            return 0;
        };
        let ids: Vec<u64> = shape_ids.iter().map(|&id| id as u64).collect();
        self.inner
            .apply_text_style_patch(page_id, &ids, &patch)
            .unwrap_or(0)
    }

    pub fn rotate_flip_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u32],
        delta_angle_deg: f64,
        toggle_flip_x: bool,
        toggle_flip_y: bool,
    ) -> usize {
        let ids: Vec<u64> = shape_ids.iter().map(|&id| id as u64).collect();
        self.inner
            .rotate_or_flip_shapes(page_id, &ids, delta_angle_deg, toggle_flip_x, toggle_flip_y)
            .unwrap_or(0)
    }

    pub fn set_page_preset(&mut self, page_id: u32, preset_slug: &str, landscape: bool) -> bool {
        let Some((w_in, h_in, unit)) =
            crate::workspace::resolve_page_preset_dimensions_in(preset_slug, landscape)
        else {
            return false;
        };
        self.inner.set_page_setup(page_id, w_in, h_in, unit).is_ok()
    }

    pub fn set_page_setup_with_unit(
        &mut self,
        page_id: u32,
        width_in: f64,
        height_in: f64,
        unit_slug: &str,
    ) -> bool {
        self.inner
            .set_page_setup(page_id, width_in, height_in, unit_slug)
            .is_ok()
    }

    pub fn set_page_dimensions(&mut self, page_id: u32, width_in: f64, height_in: f64, jump_style: i32) {
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Page Setup", |doc| {
                if let Some(page) = doc.page_mut(page_id) {
                    page.page_sheet.set_formula(
                        "PageWidth",
                        format!("{} in", format_clean_f64(width_in)),
                        Some(Unit::Inch),
                    );
                    page.page_sheet.set_formula(
                        "PageHeight",
                        format!("{} in", format_clean_f64(height_in)),
                        Some(Unit::Inch),
                    );
                    if jump_style < 0 {
                        page.page_sheet
                            .set_formula("PageLineJumpCode", "0", Some(Unit::Number));
                    } else {
                        page.page_sheet
                            .set_formula("PageLineJumpCode", "1", Some(Unit::Number));
                        page.page_sheet.set_formula(
                            "PageLineJumpStyle",
                            jump_style.to_string(),
                            Some(Unit::Number),
                        );
                    }
                }
                route_page_connectors(doc, page_id);
            });
    }

    // --- Enterprise Containers, Swimlanes, Callouts, FloorPlan, Rack, Org Chart, Gantt & Timeline ---

    pub fn insert_container(&mut self, page_id: u32, title: &str, member_ids: &[u32]) -> u32 {
        let ids: Vec<u64> = if member_ids.is_empty() {
            self.inner
                .doc
                .page(page_id)
                .map(|p| {
                    p.shapes
                        .iter()
                        .filter(|s| s.kind == ShapeKind::Shape2D)
                        .map(|s| s.id)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            member_ids.iter().map(|&i| i as u64).collect()
        };
        let mut created = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Insert Container", |doc| {
                created = create_container(doc, page_id, title, &ids, 0.35).unwrap_or(0) as u32;
            });
        created
    }

    pub fn insert_callout(&mut self, page_id: u32, target_shape_id: u32, text: &str) -> u32 {
        let mut created = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Insert Callout", |doc| {
                created =
                    attach_callout(doc, page_id, target_shape_id as u64, text, 1.6, 1.1).unwrap_or(0)
                        as u32;
            });
        created
    }

    pub fn insert_swimlanes(&mut self, page_id: u32, lanes_semicolon: &str) -> usize {
        let names: Vec<&str> = lanes_semicolon
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Insert Swimlanes", |doc| {
                count = build_cross_functional_swimlanes(doc, page_id, &names, 0.75, 7.75, 9.5, 2.1)
                    .len();
            });
        count
    }

    pub fn insert_architectural_room(&mut self, page_id: u32, room_name: &str) -> u32 {
        let mut created = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Insert Room Space", |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    p.page_sheet
                        .set_formula("PageScale", "0.25 in", Some(Unit::Inch));
                    p.page_sheet
                        .set_formula("DrawingScale", "12 in", Some(Unit::Inch));
                }
                created =
                    create_architectural_room_space(doc, page_id, room_name, 5.5, 4.25, 3.0, 2.25)
                        .unwrap_or(0) as u32;
            });
        created
    }

    pub fn insert_rack_with_equipment(&mut self, page_id: u32) -> u32 {
        let rack_template = self
            .inner
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case("Network & Rack Equipment"))
            .and_then(|p| {
                p.masters
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case("42U Standard Rack"))
            })
            .and_then(|m| m.shapes.first())
            .cloned();
        let mut rack_id = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Insert 42U Rack", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                let sid = page.next_shape_id();
                let mut rack = rack_template
                    .clone()
                    .unwrap_or_else(|| VisioShape::new_rect(sid, "42U Standard Rack", 5.5, 4.25, 1.8, 3.6));
                rack.id = sid;
                rack.name = format!("42U Standard Rack.{sid}");
                rack.name_u = rack.name.clone();
                rack.sheet.set_formula("PinX", "5.5 in", Some(Unit::Inch));
                rack.sheet.set_formula("PinY", "4.25 in", Some(Unit::Inch));
                page.shapes.push(rack);
                let _ = mount_rack_equipment(doc, page_id, sid, "Core Switch", 40, 2);
                let _ = mount_rack_equipment(doc, page_id, sid, "App Cluster", 28, 4);
                let _ = mount_rack_equipment(doc, page_id, sid, "SAN Array", 12, 6);
                rack_id = sid as u32;
            });
        rack_id
    }

    pub fn import_org_chart_csv(&mut self, page_id: u32, csv_text: &str) -> usize {
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Import Org Chart CSV", |doc| {
                count = build_org_chart_from_csv(doc, page_id, csv_text);
            });
        count
    }

    pub fn generate_gantt_demo(&mut self, page_id: u32) -> usize {
        let tasks = vec![
            GanttTaskInput {
                id: "1".into(),
                name: "ShapeSheet DAG Engine".into(),
                start_day: 0.0,
                duration_days: 5.0,
                percent_complete: 100.0,
                depends_on: vec![],
            },
            GanttTaskInput {
                id: "2".into(),
                name: "Lossless VSDX & CAD".into(),
                start_day: 4.0,
                duration_days: 6.0,
                percent_complete: 100.0,
                depends_on: vec!["1".into()],
            },
            GanttTaskInput {
                id: "3".into(),
                name: "120 FPS WGPU Viewport".into(),
                start_day: 9.0,
                duration_days: 5.0,
                percent_complete: 90.0,
                depends_on: vec!["2".into()],
            },
            GanttTaskInput {
                id: "4".into(),
                name: "Multi-OS & PWA Ship".into(),
                start_day: 13.0,
                duration_days: 5.0,
                percent_complete: 75.0,
                depends_on: vec!["3".into()],
            },
        ];
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Generate Gantt Chart", |doc| {
                count = build_gantt_chart(doc, page_id, &tasks);
            });
        count
    }

    pub fn generate_timeline_demo(&mut self, page_id: u32) -> usize {
        let milestones = vec![
            TimelineMilestone {
                day: 10.0,
                label: "Core Engine Lock".into(),
                is_major: true,
            },
            TimelineMilestone {
                day: 35.0,
                label: "CAD & VSD Parity".into(),
                is_major: false,
            },
            TimelineMilestone {
                day: 65.0,
                label: "Native Desktop Beta".into(),
                is_major: true,
            },
            TimelineMilestone {
                day: 95.0,
                label: "Enterprise GA".into(),
                is_major: true,
            },
        ];
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Generate Timeline", |doc| {
                count = build_project_timeline(
                    doc,
                    page_id,
                    "Vexio Product Release Timeline",
                    100.0,
                    &milestones,
                );
            });
        count
    }

    pub fn reverse_engineer_sql(&mut self, page_id: u32, sql_ddl: &str) -> usize {
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Reverse Engineer SQL DDL", |doc| {
                count = reverse_engineer_sql_ddl(doc, page_id, sql_ddl, true).len();
            });
        count
    }

    pub fn forward_engineer_sql(&self, page_id: u32) -> String {
        forward_engineer_sql_ddl(&self.inner.doc, page_id)
    }

    pub fn generate_pert_demo(&mut self, page_id: u32) -> usize {
        let tasks = vec![
            PertTaskInput {
                id: "T1".into(),
                name: "Architecture Spec".into(),
                duration_days: 3.0,
                predecessors: vec![],
            },
            PertTaskInput {
                id: "T2".into(),
                name: "Rust Core Engine".into(),
                duration_days: 6.0,
                predecessors: vec!["T1".into()],
            },
            PertTaskInput {
                id: "T3".into(),
                name: "Stencil Assets".into(),
                duration_days: 2.0,
                predecessors: vec!["T1".into()],
            },
            PertTaskInput {
                id: "T4".into(),
                name: "Multi-OS Ship".into(),
                duration_days: 4.0,
                predecessors: vec!["T2".into(), "T3".into()],
            },
        ];
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Generate PERT CPM", |doc| {
                count = build_pert_critical_path_diagram(doc, page_id, &tasks).len();
            });
        count
    }

    pub fn link_csv_and_apply_data_graphics(&mut self, page_id: u32, csv_text: &str, key_col: &str) -> Result<usize, JsValue> {
        let rs = DataRecordset::from_csv(1, "External Telemetry", key_col, csv_text)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut linked = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Link External Data & Graphics", |doc| {
                linked = bind_recordset_to_page(doc, page_id, &rs, None);
                let mut palette = BTreeMap::new();
                palette.insert("Healthy".into(), RgbaColor { r: 220, g: 252, b: 231, a: 255 });
                palette.insert("Warning".into(), RgbaColor { r: 254, g: 243, b: 199, a: 255 });
                palette.insert("Critical".into(), RgbaColor { r: 254, g: 226, b: 226, a: 255 });

                let spec = DataGraphicSpec {
                    name: "Live Telemetry".into(),
                    items: vec![
                        DataGraphicItem::ColorByValue {
                            prop_name: "Status".into(),
                            palette,
                        },
                        DataGraphicItem::DataBar {
                            prop_name: "Load".into(),
                            min_val: 0.0,
                            max_val: 100.0,
                            bar_color: RgbaColor { r: 37, g: 99, b: 235, a: 255 },
                        },
                        DataGraphicItem::IconSet {
                            prop_name: "Load".into(),
                            green_min: 75.0,
                            amber_min: 40.0,
                        },
                        DataGraphicItem::TextCallout {
                            prop_name: "Status".into(),
                            prefix: "Status: ".into(),
                        },
                    ],
                };
                apply_data_graphics(doc, page_id, &spec);
                let _ = insert_page_legend(doc, page_id, "Status", 9.4, 1.4);
            });
        Ok(linked)
    }

    pub fn export_bom_xlsx(&self) -> Result<Vec<u8>, JsValue> {
        let report = generate_bom_shape_report(
            &self.inner.doc,
            &["Status", "Load", "Function", "AreaSqFt", "RackU", "Title", "Department"],
        );
        report
            .to_xlsx_bytes()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn update_layer(
        &mut self,
        page_id: u32,
        layer_ix: u32,
        visible: bool,
        lock: bool,
        color_hex: &str,
    ) {
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Update Layer", |doc| {
                if let Some(page) = doc.page_mut(page_id) {
                    if let Some(l) = page.layers.iter_mut().find(|l| l.ix == layer_ix) {
                        l.visible = visible;
                        l.lock = lock;
                        l.color_override = if color_hex.trim().is_empty() {
                            None
                        } else {
                            RgbaColor::from_hex(color_hex)
                        };
                    }
                }
            });
    }

    pub fn add_comment(&mut self, page_id: u32, shape_id: u32, author: &str, text: &str) -> u32 {
        let mut next_id = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Add Comment", |doc| {
                next_id = (doc.comments.len() as u32) + 1;
                doc.comments.push(CommentAnnotation {
                    id: next_id,
                    author: author.into(),
                    initials: author
                        .split_whitespace()
                        .filter_map(|w| w.chars().next())
                        .take(2)
                        .collect::<String>()
                        .to_uppercase(),
                    timestamp_iso: "2026-11-20T18:00:00Z".into(),
                    page_id,
                    shape_id: if shape_id == 0 { None } else { Some(shape_id as u64) },
                    x_in: 1.0,
                    y_in: 1.0,
                    text: text.into(),
                    resolved: false,
                });
            });
        next_id
    }

    pub fn apply_auto_layout(&mut self, page_id: u32, algo: &str) {
        self.inner.layout(page_id, algo);
    }

    pub fn validate_diagram(&self, ruleset: &str) -> String {
        let issues: Vec<WasmIssueJson> = self
            .inner
            .validate(ruleset)
            .into_iter()
            .map(|i| WasmIssueJson {
                id: i.id,
                code: i.rule_code,
                severity: format!("{:?}", i.severity),
                category: i.category,
                description: i.description,
                shape_id: i.shape_id,
            })
            .collect();
        serde_json::to_string(&issues).unwrap_or_else(|_| "[]".into())
    }

    pub fn run_vba_macro(&mut self, script: &str) -> Result<String, JsValue> {
        let rep = self
            .inner
            .run_macro(script)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&rep).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn reorder_shapes(&mut self, page_id: u32, shape_ids: &[u32], mode: &str) -> usize {
        let ids64: Vec<u64> = shape_ids.iter().map(|&i| i as u64).collect();
        self.inner
            .reorder_shapes(page_id, &ids64, mode)
            .unwrap_or(0)
    }

    pub fn add_page(&mut self, name: &str) -> Result<u32, JsValue> {
        self.inner
            .add_page(name)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn rename_page(&mut self, page_id: u32, new_name: &str) -> bool {
        self.inner.rename_page(page_id, new_name)
    }

    pub fn duplicate_page(&mut self, page_id: u32) -> Result<u32, JsValue> {
        self.inner
            .duplicate_page(page_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn delete_page(&mut self, page_id: u32) -> bool {
        self.inner.delete_page(page_id)
    }

    pub fn undo(&mut self) -> bool {
        self.inner
            .journal
            .undo(&mut self.inner.doc)
            .unwrap_or(false)
    }

    pub fn redo(&mut self) -> bool {
        self.inner
            .journal
            .redo(&mut self.inner.doc)
            .unwrap_or(false)
    }

    pub fn export_wal(&self) -> Vec<u8> {
        self.inner.journal.encode_wal().unwrap_or_default()
    }

    pub fn replay_wal(&mut self, bytes: &[u8]) -> Result<(), JsValue> {
        let j = CommandJournal::replay_wal(bytes, &mut self.inner.doc)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.inner.journal = j;
        Ok(())
    }

    pub fn get_state_json(&self, active_page_id: u32) -> String {
        let pid = if self.inner.doc.page(active_page_id).is_some() {
            active_page_id
        } else {
            self.inner
                .doc
                .pages
                .iter()
                .find(|p| !p.is_background)
                .or_else(|| self.inner.doc.pages.first())
                .map(|p| p.id)
                .unwrap_or(0)
        };
        let active_page = self.inner.doc.page(pid);
        let page_width_in = active_page.map(|p| p.width_in()).unwrap_or(11.0);
        let page_height_in = active_page.map(|p| p.height_in()).unwrap_or(8.5);
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.inner.doc, pid);
        let vis_by_id: BTreeMap<u64, &EvaluatedShapeVisual> =
            visuals.iter().map(|v| (v.shape_id, v)).collect();

        let pages: Vec<WasmPageInfo> = self
            .inner
            .doc
            .pages
            .iter()
            .map(|p| WasmPageInfo {
                id: p.id,
                name: p.name.clone(),
                width_in: p.width_in(),
                height_in: p.height_in(),
                shape_count: p.shapes.len(),
                is_background: p.is_background,
            })
            .collect();

        let layers: Vec<WasmLayerInfo> = active_page
            .map(|p| {
                p.layers
                    .iter()
                    .map(|l| WasmLayerInfo {
                        ix: l.ix,
                        name: l.name.clone(),
                        visible: l.visible,
                        print: l.print,
                        lock: l.lock,
                        color_override: l.color_override.map(|c| c.to_hex_rgb()),
                    })
                    .collect()
            })
            .unwrap_or_default();

        let comments: Vec<WasmCommentInfo> = self
            .inner
            .doc
            .comments
            .iter()
            .filter(|c| c.page_id == pid)
            .map(|c| WasmCommentInfo {
                id: c.id,
                author: c.author.clone(),
                text: c.text.clone(),
                page_id: c.page_id,
                shape_id: c.shape_id,
                resolved: c.resolved,
            })
            .collect();

        let shapes: Vec<WasmShapeInfo> = active_page
            .map(|page| {
                page.shapes
                    .iter()
                    .map(|s| {
                        let cells: Vec<WasmCellInfo> = s
                            .sheet
                            .cells
                            .values()
                            .filter(|c| !c.name.starts_with("Geometry"))
                            .take(80)
                            .map(|c| WasmCellInfo {
                                name: c.name.clone(),
                                formula: c.formula.clone(),
                                value: c
                                    .value
                                    .as_str_coerced()
                                    .unwrap_or_else(|e| e.as_str().into()),
                                guarded: c.is_guarded(),
                            })
                            .collect();
                        let subprocess_page = s
                            .sheet
                            .hyperlinks
                            .iter()
                            .find(|h| !h.sub_address.is_empty())
                            .map(|h| h.sub_address.clone());
                        let screentip = s
                            .sheet
                            .get_cell("Comment")
                            .and_then(|c| c.value.as_str_coerced().ok())
                            .filter(|v| !v.trim().is_empty());
                        let control_handles = list_control_handles(&self.inner.doc, pid, s.id);
                        let is_1d = s.kind == ShapeKind::Connector1D;
                        let (begin_x, begin_y, end_x, end_y) = if is_1d {
                            (
                                s.sheet.get_f64("BeginX"),
                                s.sheet.get_f64("BeginY"),
                                s.sheet.get_f64("EndX"),
                                s.sheet.get_f64("EndY"),
                            )
                        } else {
                            (None, None, None, None)
                        };
                        let path_points = if is_1d {
                            vis_by_id
                                .get(&s.id)
                                .map(|v| sample_visual_polyline(v))
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        let ports = if s.kind == ShapeKind::Shape2D {
                            if let Some(v) = vis_by_id.get(&s.id) {
                                let w = v.transform.width;
                                let h = v.transform.height;
                                let (wx, wy) = v.transform.local_to_parent(0.0, h * 0.5);
                                let (ex, ey) = v.transform.local_to_parent(w, h * 0.5);
                                let (sx, sy) = v.transform.local_to_parent(w * 0.5, 0.0);
                                let (nx, ny) = v.transform.local_to_parent(w * 0.5, h);
                                vec![[wx, wy], [ex, ey], [sx, sy], [nx, ny]]
                            } else {
                                Vec::new()
                            }
                        } else {
                            Vec::new()
                        };
                        let route_style = s.sheet.get_f64("ShapeRouteStyle").unwrap_or(16.0).round() as i32;
                        let begin_arrow = s.sheet.get_f64("BeginArrow").unwrap_or(0.0).round() as i32;
                        let end_arrow = s.sheet.get_f64("EndArrow").unwrap_or(if is_1d { 4.0 } else { 0.0 }).round() as i32;
                        let line_weight_pt = (s.sheet.get_f64("LineWeight").unwrap_or(1.25 / 72.0) * 72.0 * 100.0).round() / 100.0;
                        let line_pattern = s.sheet.get_f64("LinePattern").unwrap_or(1.0).round() as i32;
                        let bg_trans = s.sheet.get_f64("TextBkgndTrans").unwrap_or(1.0);
                        let text_bg_color = if bg_trans < 0.99 {
                            s.sheet.get_color("TextBkgnd").map(|c| c.to_hex_rgb())
                        } else {
                            None
                        };
                        WasmShapeInfo {
                            id: s.id,
                            name: s.name.clone(),
                            kind: format!("{:?}", s.kind),
                            text: s.text.clone(),
                            pin_x: s.sheet.get_f64("PinX").unwrap_or(0.0),
                            pin_y: s.sheet.get_f64("PinY").unwrap_or(0.0),
                            width: s.sheet.get_f64("Width").unwrap_or(1.0),
                            height: s.sheet.get_f64("Height").unwrap_or(1.0),
                            angle_deg: s.sheet.get_f64("Angle").unwrap_or(0.0).to_degrees(),
                            flip_x: s.sheet.get_bool("FlipX").unwrap_or(false),
                            flip_y: s.sheet.get_bool("FlipY").unwrap_or(false),
                            font_family: s
                                .sheet
                                .get_str("Char.Font")
                                .filter(|f| !f.is_empty())
                                .unwrap_or_else(|| "Inter".to_string()),
                            font_size_pt: (s.sheet.get_f64("Char.Size").unwrap_or(10.0 / 72.0) * 72.0 * 10.0).round() / 10.0,
                            char_style: s.sheet.get_f64("Char.Style").unwrap_or(0.0).round().clamp(0.0, 15.0) as u8,
                            text_color: s
                                .sheet
                                .get_color("Char.Color")
                                .unwrap_or(vexio_shapesheet::RgbaColor::BLACK)
                                .to_hex_rgb(),
                            text_bg_color,
                            horiz_align: s.sheet.get_f64("Para.HorzAlign").unwrap_or(1.0).round().clamp(0.0, 3.0) as u8,
                            vert_align: s.sheet.get_f64("VerticalAlign").unwrap_or(1.0).round().clamp(0.0, 2.0) as u8,
                            bullet_style: s.sheet.get_f64("Para.Bullet").unwrap_or(0.0).round().clamp(0.0, 4.0) as u8,
                            begin_x,
                            begin_y,
                            end_x,
                            end_y,
                            path_points,
                            ports,
                            route_style,
                            begin_arrow,
                            end_arrow,
                            line_weight_pt,
                            line_pattern,
                            subprocess_page,
                            screentip,
                            control_handles,
                            cells,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let display_unit = active_page
            .and_then(|p| p.page_sheet.get_str("User.DisplayUnit"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "mm".to_string());

        let state = WasmStateJson {
            title: self.inner.doc.properties.title.clone(),
            active_page_id: pid,
            page_width_in,
            page_height_in,
            display_unit,
            pages,
            layers,
            comments,
            shapes,
            svg_preview: export_page_svg(&self.inner.doc, pid),
            can_undo: self.inner.journal.can_undo(),
            can_redo: self.inner.journal.can_redo(),
            wgpu_surface_active: self.wgpu_renderer.is_some(),
        };
        serde_json::to_string(&state).unwrap_or_else(|_| "{}".into())
    }

    // --- Phase 7: Per-Cell HLC CRDT Co-Editing Sync ---

    pub fn set_replica_id(&mut self, replica_id: &str) {
        self.crdt_state = CrdtDocumentState::new(replica_id);
        self.crdt_prev_doc = self.inner.doc.clone();
    }

    pub fn flush_crdt_ops_json(&mut self, now_ms: f64) -> String {
        let ops = self
            .crdt_state
            .diff_to_ops(&self.crdt_prev_doc, &self.inner.doc, now_ms.max(1.0) as u64);
        self.crdt_prev_doc = self.inner.doc.clone();
        serde_json::to_string(&ops).unwrap_or_else(|_| "[]".into())
    }

    pub fn apply_remote_crdt_ops_json(&mut self, ops_json: &str) -> Result<usize, JsValue> {
        let applied = self
            .crdt_state
            .apply_batch_json(&mut self.inner.doc, ops_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.crdt_prev_doc = self.inner.doc.clone();
        Ok(applied)
    }

    // --- Phase 7: 2D Boolean Shape Operations, Vertex/NURBS Editing & Yellow Control Handles ---

    pub fn apply_boolean_op(
        &mut self,
        page_id: u32,
        shape_ids: &[u32],
        op: &str,
        offset_in: f64,
    ) -> Vec<u32> {
        let ids64: Vec<u64> = shape_ids.iter().map(|&i| i as u64).collect();
        let kind = match op.to_ascii_lowercase().as_str() {
            "union" => BooleanOpKind::Union,
            "combine" => BooleanOpKind::Combine,
            "fragment" => BooleanOpKind::Fragment,
            "intersect" => BooleanOpKind::Intersect,
            "subtract" => BooleanOpKind::Subtract,
            "trim" => BooleanOpKind::Trim,
            "join" => BooleanOpKind::Join,
            "offset" => BooleanOpKind::Offset(offset_in),
            _ => return Vec::new(),
        };
        let mut created = Vec::new();
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, format!("Boolean {op}"), |doc| {
                created = apply_boolean_operation(doc, page_id, &ids64, kind)
                    .into_iter()
                    .map(|id| id as u32)
                    .collect();
            });
        created
    }

    pub fn list_shape_vertices_json(&self, page_id: u32, shape_id: u32) -> String {
        let verts = list_shape_vertices(&self.inner.doc, page_id, shape_id as u64);
        serde_json::to_string(&verts).unwrap_or_else(|_| "[]".into())
    }

    pub fn move_shape_vertex(
        &mut self,
        page_id: u32,
        shape_id: u32,
        geom_index: u32,
        row_ix: u32,
        x_in: f64,
        y_in: f64,
    ) -> bool {
        let mut ok = false;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Move Vertex", |doc| {
                ok = move_shape_vertex(doc, page_id, shape_id as u64, geom_index, row_ix, x_in, y_in);
            });
        ok
    }

    pub fn draw_freeform_curve(
        &mut self,
        page_id: u32,
        points_json: &str,
        closed: bool,
    ) -> Result<u32, JsValue> {
        let pts: Vec<[f64; 2]> =
            serde_json::from_str(points_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut sid = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Draw Freeform Curve", |doc| {
                sid = draw_freeform_curve(doc, page_id, &pts, closed).unwrap_or(0) as u32;
            });
        Ok(sid)
    }

    pub fn ensure_control_handle(&mut self, page_id: u32, shape_id: u32) -> bool {
        let mut ok = false;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Add Control Handle", |doc| {
                ok = ensure_default_control_handle(doc, page_id, shape_id as u64);
            });
        ok
    }

    pub fn move_control_handle(
        &mut self,
        page_id: u32,
        shape_id: u32,
        row_name: &str,
        x_in: f64,
        y_in: f64,
    ) -> bool {
        let mut ok = false;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Move Control Handle", |doc| {
                ok = move_control_handle(doc, page_id, shape_id as u64, row_name, x_in, y_in);
            });
        ok
    }

    // --- Phase 7: 6 Themes × 4 Variants, Gradients/Hatches/Shadows/3D Bevels/Sketchy & Cropped Images ---

    pub fn apply_theme_variant(&mut self, theme_slug: &str, variant_index: u8) -> usize {
        let mut count = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Apply Theme Variant", |doc| {
                count = apply_document_theme_variant(doc, theme_slug, variant_index);
            });
        count
    }

    pub fn apply_shape_visual_preset(&mut self, page_id: u32, shape_id: u32, preset: &str) -> bool {
        let sid = shape_id as u64;
        let spec = match preset {
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
            "hatch_cross" => ShapeVisualStyleSpec {
                hatch: Some(HatchFillSpec {
                    kind: HatchPatternKind::Crosshatch,
                    foregnd: RgbaColor { r: 100, g: 116, b: 139, a: 255 },
                    bkgnd: RgbaColor { r: 255, g: 255, b: 255, a: 255 },
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
                    seed: shape_id.max(1),
                    amount: 1.0,
                }),
                ..Default::default()
            },
            _ => ShapeVisualStyleSpec {
                shadow: Some(DropShadowSpec {
                    enabled: false,
                    foregnd: RgbaColor::BLACK,
                    offset_x_in: 0.0,
                    offset_y_in: 0.0,
                    blur_pt: 0.0,
                }),
                bevel: Some(BevelExtrusionSpec {
                    bevel_top_type: 0,
                    extrusion_depth_in: 0.0,
                    extrusion_angle_deg: 0.0,
                }),
                sketch: Some(SketchOutlineSpec {
                    enabled: false,
                    seed: 0,
                    amount: 0.0,
                }),
                ..Default::default()
            },
        };
        let mut ok = false;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Apply Visual Style", |doc| {
                ok = apply_visual_effects_to_shape(doc, page_id, sid, &spec);
            });
        ok
    }

    #[allow(clippy::too_many_arguments)]
    pub fn insert_cropped_image(
        &mut self,
        page_id: u32,
        media_type: &str,
        image_bytes: &[u8],
        x_in: f64,
        y_in: f64,
        crop_in: f64,
        brightness: f64,
        contrast: f64,
    ) -> u32 {
        let mut sid32 = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Insert Image", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                let sid = page.next_shape_id();
                let mut sh = VisioShape::new_rect(sid, format!("Picture.{sid}"), x_in, y_in, 2.4, 1.8);
                sh.kind = ShapeKind::ForeignObject;
                sh.foreign_data = Some(ForeignMediaData {
                    media_type: media_type.to_ascii_uppercase(),
                    rel_id: None,
                    data: image_bytes.to_vec(),
                });
                let c = format_clean_f64(crop_in.max(0.0));
                sh.sheet.set_formula("CropLeft", format!("{c} in"), Some(Unit::Inch));
                sh.sheet.set_formula("CropRight", format!("{c} in"), Some(Unit::Inch));
                sh.sheet.set_formula("CropTop", format!("{c} in"), Some(Unit::Inch));
                sh.sheet.set_formula("CropBottom", format!("{c} in"), Some(Unit::Inch));
                sh.sheet
                    .set_formula("Brightness", format_clean_f64(brightness), Some(Unit::Number));
                sh.sheet
                    .set_formula("Contrast", format_clean_f64(contrast), Some(Unit::Number));
                page.shapes.push(sh);
                sid32 = sid as u32;
            });
        sid32
    }

    // --- Phase 7: Subprocess Drill-Down, Hyperlinks/ScreenTips, Title Block, Compare & Plan 2 Connectors ---

    pub fn create_subprocess_drilldown(
        &mut self,
        page_id: u32,
        shape_id: u32,
        sub_name: &str,
    ) -> u32 {
        let mut pid = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Link Subprocess", |doc| {
                pid = create_or_link_subprocess_page(doc, page_id, shape_id as u64, sub_name)
                    .unwrap_or(0);
            });
        pid
    }

    pub fn set_hyperlink_and_screentip(
        &mut self,
        page_id: u32,
        shape_id: u32,
        description: &str,
        address: &str,
        sub_address: &str,
        screentip: &str,
    ) -> bool {
        let mut ok = false;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Set Hyperlink & ScreenTip", |doc| {
                if !address.trim().is_empty() || !sub_address.trim().is_empty() {
                    ok |= add_shape_hyperlink(
                        doc,
                        page_id,
                        shape_id as u64,
                        description,
                        address,
                        sub_address,
                        true,
                    );
                }
                if !screentip.trim().is_empty() {
                    ok |= set_shape_screentip(doc, page_id, shape_id as u64, screentip);
                }
            });
        ok
    }

    pub fn apply_title_block_border(
        &mut self,
        page_id: u32,
        project_code: &str,
        author: &str,
        revision: &str,
    ) -> u32 {
        let mut bg_pid = 0u32;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Apply ISO Title Block", |doc| {
                bg_pid =
                    apply_iso_title_block_background(doc, page_id, project_code, author, revision)
                        .unwrap_or(0);
            });
        bg_pid
    }

    pub fn compare_with_vsdx_bytes(&self, baseline_vsdx: &[u8]) -> Result<String, JsValue> {
        let (base_doc, _) =
            vexio_vsdx::read_vsdx(baseline_vsdx).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let report = compare_documents(&base_doc, &self.inner.doc);
        serde_json::to_string(&report).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn import_data_visualizer(&mut self, page_id: u32, bytes: &[u8]) -> Result<usize, JsValue> {
        let mut count = 0usize;
        let mut err = None;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Import Data Visualizer", |doc| {
                match import_data_visualizer_csv_or_xlsx(doc, page_id, bytes) {
                    Ok(c) => count = c,
                    Err(e) => err = Some(e.to_string()),
                }
            });
        if let Some(e) = err {
            return Err(JsValue::from_str(&e));
        }
        Ok(count)
    }

    pub fn export_data_visualizer_workbook(&self, page_id: u32) -> Result<Vec<u8>, JsValue> {
        export_data_visualizer_xlsx(&self.inner.doc, page_id)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn import_cloud_discovery(
        &mut self,
        page_id: u32,
        discovery_json: &str,
    ) -> Result<usize, JsValue> {
        let mut count = 0usize;
        let mut err = None;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Import Cloud Discovery", |doc| {
                match import_cloud_discovery_json(doc, page_id, discovery_json) {
                    Ok(c) => count = c,
                    Err(e) => err = Some(e.to_string()),
                }
            });
        if let Some(e) = err {
            return Err(JsValue::from_str(&e));
        }
        Ok(count)
    }

    pub fn import_entra_or_ldif(&mut self, page_id: u32, text: &str) -> Result<usize, JsValue> {
        let mut count = 0usize;
        let mut err = None;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Import Directory Org Chart", |doc| {
                match import_entra_or_ldif_org_chart(doc, page_id, text) {
                    Ok(c) => count = c,
                    Err(e) => err = Some(e.to_string()),
                }
            });
        if let Some(e) = err {
            return Err(JsValue::from_str(&e));
        }
        Ok(count)
    }

    pub fn import_sqlite_recordset(
        &mut self,
        page_id: u32,
        sqlite_bytes: &[u8],
        table_name: &str,
        key_col: &str,
    ) -> Result<usize, JsValue> {
        let tbl = if table_name.trim().is_empty() {
            list_sqlite_tables(sqlite_bytes)
                .map_err(|e| JsValue::from_str(&e))?
                .into_iter()
                .next()
                .ok_or_else(|| JsValue::from_str("SQLite database contains no user tables"))?
        } else {
            table_name.to_string()
        };
        let rs = read_sqlite_table_to_recordset(77, sqlite_bytes, &tbl, key_col)
            .map_err(|e| JsValue::from_str(&e))?;
        let mut linked = 0usize;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Import SQLite Recordset", |doc| {
                linked = bind_recordset_to_page(doc, page_id, &rs, None);
            });
        Ok(linked)
    }

    pub fn run_vba_macro_with_selection(
        &mut self,
        script: &str,
        selected_ids: &[u32],
    ) -> Result<String, JsValue> {
        let ids64: Vec<u64> = selected_ids.iter().map(|&i| i as u64).collect();
        let mut rep = None;
        let mut err = None;
        let _ = self
            .inner
            .journal
            .record_mutation(&mut self.inner.doc, "Run VBA Macro", |doc| {
                match crate::automation::execute_vba_macro_with_selection(
                    doc,
                    script,
                    &ids64,
                    None,
                    crate::automation::SandboxLimits::default(),
                ) {
                    Ok(r) => rep = Some(r),
                    Err(e) => err = Some(e.to_string()),
                }
            });
        if let Some(e) = err {
            return Err(JsValue::from_str(&e));
        }
        serde_json::to_string(&rep.unwrap()).map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
