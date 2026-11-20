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

use crate::containers::{attach_callout, build_cross_functional_swimlanes, create_container};
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
use crate::export_print::export_page_svg;
use crate::journal::{CommandJournal, DocCommand};
use crate::routing::route_page_connectors;
use crate::workspace::DocumentSession;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vexio_shapesheet::{
    CommentAnnotation, MarkupInkPoint, MarkupInkStroke, ReactiveEngine, RgbaColor, ShapeKind, Unit,
    VisioShape, format_clean_f64,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmVexioSession {
    inner: DocumentSession,
    wgpu_renderer: Option<vexio_render::WgpuCanvasRenderer>,
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
        Self {
            inner: DocumentSession::new_blank(title),
            wgpu_renderer: None,
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
        if let Some(p) = self.inner.doc.pages.first() {
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
        if let Some(p) = self.inner.doc.page(page_id) {
            r.camera.fit_to_page(p.width_in(), p.height_in());
        }
        r.selected_shapes = selected_ids.iter().map(|&i| i as u64).collect();
        r.render_surface_frame(&self.inner.doc, page_id).is_ok()
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

    pub fn load_starter_template(&mut self, template_id: &str) {
        self.inner.doc = instantiate_starter_template(template_id);
        self.inner.journal = CommandJournal::new();
    }

    pub fn open_bytes(&mut self, bytes: &[u8], ext: &str) -> Result<(), JsValue> {
        self.inner
            .open_bytes(bytes, ext)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn save_format(&self, ext: &str) -> Result<Vec<u8>, JsValue> {
        self.inner
            .save_format(ext)
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
            .hit_test_point(x_in, y_in, 0.08)
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
        mut width_in: f64,
        mut height_in: f64,
        angle_deg: f64,
    ) -> bool {
        let sid = shape_id as u64;
        let Some(sh) = self.inner.doc.page(page_id).and_then(|p| p.find_shape(sid)) else {
            return false;
        };
        let cur_w = sh.sheet.get_f64("Width").unwrap_or(1.0);
        let cur_h = sh.sheet.get_f64("Height").unwrap_or(1.0);
        if sh.sheet.get_bool("LockWidth").unwrap_or(false) {
            width_in = cur_w;
        }
        if sh.sheet.get_bool("LockHeight").unwrap_or(false) {
            height_in = cur_h;
        }
        if sh.sheet.get_bool("LockAspect").unwrap_or(false) && cur_h > 1e-6 {
            let aspect = cur_w / cur_h;
            height_in = (width_in / aspect).max(0.15);
        }
        width_in = width_in.max(0.15);
        height_in = height_in.max(0.15);

        let mut cmds = Vec::new();
        for (cname, new_f) in [
            ("PinX", format!("{} in", format_clean_f64(pin_x_in))),
            ("PinY", format!("{} in", format_clean_f64(pin_y_in))),
            ("Width", format!("{} in", format_clean_f64(width_in))),
            ("Height", format!("{} in", format_clean_f64(height_in))),
        ] {
            let old_f = sh
                .sheet
                .get_cell(cname)
                .map(|c| c.formula.clone())
                .unwrap_or_else(|| "0 in".into());
            cmds.push(DocCommand::SetCellFormula {
                page_id,
                shape_id: sid,
                cell_name: cname.into(),
                old_formula: old_f,
                new_formula: new_f,
                force_setf: false,
            });
        }
        if !sh.sheet.get_bool("LockRotate").unwrap_or(false) {
            let old_a = sh
                .sheet
                .get_cell("Angle")
                .map(|c| c.formula.clone())
                .unwrap_or_else(|| "0 deg".into());
            cmds.push(DocCommand::SetCellFormula {
                page_id,
                shape_id: sid,
                cell_name: "Angle".into(),
                old_formula: old_a,
                new_formula: format!("{} deg", format_clean_f64(angle_deg)),
                force_setf: false,
            });
        }

        if self
            .inner
            .journal
            .execute(
                &mut self.inner.doc,
                DocCommand::Batch {
                    label: "Transform Shape".into(),
                    commands: cmds,
                },
            )
            .is_ok()
        {
            route_page_connectors(&mut self.inner.doc, page_id);
            true
        } else {
            false
        }
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
        let w = (x2_in - x1_in).abs().max(0.4);
        let h = (y2_in - y1_in).abs().max(0.3);
        let cx = (x1_in + x2_in) * 0.5;
        let cy = (y1_in + y2_in) * 0.5;
        let sid = self
            .inner
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);
        let mut sh = VisioShape::new_rect(sid, format!("Rectangle.{sid}"), cx, cy, w, h);
        sh.text = format!("Shape {sid}");
        self.inner
            .journal
            .execute(&mut self.inner.doc, DocCommand::AddShape { page_id, shape: sh })
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(sid as u32)
    }

    pub fn delete_shape(&mut self, page_id: u32, shape_id: u32) -> bool {
        let sid = shape_id as u64;
        let Some(page) = self.inner.doc.page(page_id) else {
            return false;
        };
        let Some(shape) = page.find_shape(sid).cloned() else {
            return false;
        };
        if shape.sheet.get_bool("LockDelete").unwrap_or(false) {
            return false;
        }
        let removed_connects = page
            .connects
            .iter()
            .filter(|c| c.connector_shape_id == sid || c.target_shape_id == sid)
            .cloned()
            .collect();
        self.inner
            .journal
            .execute(
                &mut self.inner.doc,
                DocCommand::DeleteShape {
                    page_id,
                    shape,
                    removed_connects,
                },
            )
            .is_ok()
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

    pub fn set_page_dimensions(&mut self, page_id: u32, width_in: f64, height_in: f64, jump_style: i32) {
        if let Some(page) = self.inner.doc.page_mut(page_id) {
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
                page.page_sheet.set_formula("PageLineJumpCode", "0", Some(Unit::Number));
            } else {
                page.page_sheet.set_formula("PageLineJumpCode", "1", Some(Unit::Number));
                page.page_sheet.set_formula(
                    "PageLineJumpStyle",
                    jump_style.to_string(),
                    Some(Unit::Number),
                );
            }
        }
        route_page_connectors(&mut self.inner.doc, page_id);
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
        create_container(&mut self.inner.doc, page_id, title, &ids, 0.35).unwrap_or(0) as u32
    }

    pub fn insert_callout(&mut self, page_id: u32, target_shape_id: u32, text: &str) -> u32 {
        attach_callout(
            &mut self.inner.doc,
            page_id,
            target_shape_id as u64,
            text,
            1.6,
            1.1,
        )
        .unwrap_or(0) as u32
    }

    pub fn insert_swimlanes(&mut self, page_id: u32, lanes_semicolon: &str) -> usize {
        let names: Vec<&str> = lanes_semicolon
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        build_cross_functional_swimlanes(
            &mut self.inner.doc,
            page_id,
            &names,
            0.75,
            7.75,
            9.5,
            2.1,
        )
        .len()
    }

    pub fn insert_architectural_room(&mut self, page_id: u32, room_name: &str) -> u32 {
        if let Some(p) = self.inner.doc.page_mut(page_id) {
            p.page_sheet.set_formula("PageScale", "0.25 in", Some(Unit::Inch));
            p.page_sheet.set_formula("DrawingScale", "12 in", Some(Unit::Inch));
        }
        create_architectural_room_space(&mut self.inner.doc, page_id, room_name, 5.5, 4.25, 3.0, 2.25)
            .unwrap_or(0) as u32
    }

    pub fn insert_rack_with_equipment(&mut self, page_id: u32) -> u32 {
        let Ok(rack_id) = self.drop_builtin_shape_on_page(
            page_id,
            "Network & Rack Equipment",
            "42U Standard Rack",
            5.5,
            4.25,
        ) else {
            return 0;
        };
        let _ = mount_rack_equipment(&mut self.inner.doc, page_id, rack_id as u64, "Core Switch", 40, 2);
        let _ = mount_rack_equipment(&mut self.inner.doc, page_id, rack_id as u64, "App Cluster", 28, 4);
        let _ = mount_rack_equipment(&mut self.inner.doc, page_id, rack_id as u64, "SAN Array", 12, 6);
        rack_id
    }

    pub fn import_org_chart_csv(&mut self, page_id: u32, csv_text: &str) -> usize {
        build_org_chart_from_csv(&mut self.inner.doc, page_id, csv_text)
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
        build_gantt_chart(&mut self.inner.doc, page_id, &tasks)
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
        build_project_timeline(
            &mut self.inner.doc,
            page_id,
            "Vexio Product Release Timeline",
            100.0,
            &milestones,
        )
    }

    pub fn reverse_engineer_sql(&mut self, page_id: u32, sql_ddl: &str) -> usize {
        reverse_engineer_sql_ddl(&mut self.inner.doc, page_id, sql_ddl, true).len()
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
        build_pert_critical_path_diagram(&mut self.inner.doc, page_id, &tasks).len()
    }

    pub fn link_csv_and_apply_data_graphics(&mut self, page_id: u32, csv_text: &str, key_col: &str) -> Result<usize, JsValue> {
        let rs = DataRecordset::from_csv(1, "External Telemetry", key_col, csv_text)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let linked = bind_recordset_to_page(&mut self.inner.doc, page_id, &rs, None);
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
        apply_data_graphics(&mut self.inner.doc, page_id, &spec);
        let _ = insert_page_legend(&mut self.inner.doc, page_id, "Status", 9.4, 1.4);
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
        if let Some(page) = self.inner.doc.page_mut(page_id) {
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
        ReactiveEngine::recalculate_document(&mut self.inner.doc);
    }

    pub fn add_comment(&mut self, page_id: u32, shape_id: u32, author: &str, text: &str) -> u32 {
        let next_id = (self.inner.doc.comments.len() as u32) + 1;
        self.inner.doc.comments.push(CommentAnnotation {
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

    pub fn add_page(&mut self, name: &str) -> Result<u32, JsValue> {
        self.inner
            .add_page(name)
            .map_err(|e| JsValue::from_str(&e.to_string()))
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
            self.inner.doc.pages.first().map(|p| p.id).unwrap_or(1)
        };
        let active_page = self.inner.doc.page(pid);
        let page_width_in = active_page.map(|p| p.width_in()).unwrap_or(11.0);
        let page_height_in = active_page.map(|p| p.height_in()).unwrap_or(8.5);

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
                            .take(28)
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
                            cells,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let state = WasmStateJson {
            title: self.inner.doc.properties.title.clone(),
            active_page_id: pid,
            page_width_in,
            page_height_in,
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
}
