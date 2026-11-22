//! Unified `DocumentSession` orchestrating file I/O (`.vsdx`, `.vsdm`, `.vssx`, `.vstx`,
//! `.vsd`, `.vdx`, `.vdw`, `.dwg`, `.dxf`, `.pdf`, `.svg`, `.eps`, `.png`, `.html`),
//! transactions, routing, stencils, layouts, validation, and automation across all FFI targets.

use crate::autolayout::{LayoutAlgorithm, LayoutOptions, apply_auto_layout};
use crate::automation::{AutomationExecutionReport, SandboxLimits, execute_vba_macro};
use crate::domains::{build_all_builtin_stencils, instantiate_starter_template};
use crate::export_print::{
    export_document_html_report, export_document_pdf, export_page_eps, export_page_jpeg,
    export_page_png, export_page_svg,
};
use crate::journal::{CommandJournal, DocCommand, JournalError};
use crate::routing::route_page_connectors;
use crate::validation::{DiagramRuleSet, ValidationIssue, validate_document};
use thiserror::Error;
use vexio_shapesheet::{
    ConnectionPointRow, GeometryRow, GeometryRowKind, GeometrySection, GlueEndpoint,
    ReactiveEngine, ShapeKind, StencilPack, Unit, UserCellRow, VisioDocument, VisioPage,
    VisioShape, format_clean_f64,
};
use vexio_vsdx::{VsdxPackageKind, read_vsdx, write_vsdx};

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("VSDX OpenXML error: {0}")]
    Vsdx(#[from] vexio_vsdx::VsdxError),
    #[error("Legacy VSD/VDX error: {0}")]
    Legacy(#[from] vexio_vsd_legacy::LegacyVisioError),
    #[error("CAD DWG/DXF error: {0}")]
    Cad(#[from] vexio_cad::CadError),
    #[error("Journal / Command error: {0}")]
    Journal(#[from] JournalError),
    #[error("Automation error: {0}")]
    Automation(#[from] crate::automation::AutomationError),
    #[error("Unsupported file extension: {0}")]
    UnsupportedExtension(String),
}

fn attach_cardinal_ports_to_shape(sh: &mut VisioShape) {
    if !sh.sheet.connection_points.is_empty() {
        return;
    }
    sh.sheet.connection_points = vec![
        ConnectionPointRow {
            name: "X1".into(),
            ix: 1,
            x: "0 in".into(),
            y: "Height*0.5".into(),
            dir_x: "-1".into(),
            dir_y: "0".into(),
            point_type: 2,
        },
        ConnectionPointRow {
            name: "X2".into(),
            ix: 2,
            x: "Width".into(),
            y: "Height*0.5".into(),
            dir_x: "1".into(),
            dir_y: "0".into(),
            point_type: 2,
        },
        ConnectionPointRow {
            name: "X3".into(),
            ix: 3,
            x: "Width*0.5".into(),
            y: "0 in".into(),
            dir_x: "0".into(),
            dir_y: "-1".into(),
            point_type: 2,
        },
        ConnectionPointRow {
            name: "X4".into(),
            ix: 4,
            x: "Width*0.5".into(),
            y: "Height".into(),
            dir_x: "0".into(),
            dir_y: "1".into(),
            point_type: 2,
        },
    ];
    sh.sheet.sync_all_sections_to_cells();
}

fn remap_dropped_shape_subtree(root: &mut VisioShape, alloc_id: &mut u64) -> u64 {
    fn assign_ids(node: &mut VisioShape, alloc_id: &mut u64, id_map: &mut std::collections::BTreeMap<u64, u64>, depth: u8) {
        let old_id = node.id;
        let new_id = *alloc_id;
        *alloc_id += 1;
        node.id = new_id;
        id_map.insert(old_id, new_id);
        if depth > 32 {
            node.children.clear();
            return;
        }
        for c in &mut node.children {
            assign_ids(c, alloc_id, id_map, depth + 1);
        }
    }
    fn rewrite_formula(formula: &str, id_map: &std::collections::BTreeMap<u64, u64>) -> String {
        if id_map.is_empty() || !formula.to_ascii_lowercase().contains("sheet.") {
            return formula.to_string();
        }
        let bytes = formula.as_bytes();
        let mut out = String::with_capacity(formula.len() + 8);
        let mut i = 0usize;
        while i < bytes.len() {
            if i + 6 < bytes.len() && bytes[i..i + 6].eq_ignore_ascii_case(b"sheet.") {
                let prev_is_ident = i > 0
                    && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_' || bytes[i - 1] == b'.');
                if !prev_is_ident {
                    let mut j = i + 6;
                    while j < bytes.len() && bytes[j].is_ascii_digit() {
                        j += 1;
                    }
                    if j > i + 6 && j < bytes.len() && bytes[j] == b'!' {
                        if let Ok(old_id) = formula[i + 6..j].parse::<u64>() {
                            if let Some(&new_id) = id_map.get(&old_id) {
                                out.push_str(&formula[i..i + 6]);
                                out.push_str(&new_id.to_string());
                                out.push('!');
                                i = j + 1;
                                continue;
                            }
                        }
                    }
                }
            }
            let Some(ch) = formula.get(i..).and_then(|s| s.chars().next()) else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }
    fn apply_rewrite(node: &mut VisioShape, id_map: &std::collections::BTreeMap<u64, u64>) {
        for cell in node.sheet.cells.values_mut() {
            let rewritten = rewrite_formula(&cell.formula, id_map);
            if rewritten != cell.formula {
                let hint = cell.unit_hint;
                let cached = cell.vsdx_cached_value.clone();
                let nm = cell.name.clone();
                *cell = vexio_shapesheet::Cell::from_formula(nm, rewritten, hint);
                cell.vsdx_cached_value = cached;
            }
        }
        for geom in &mut node.sheet.geometries {
            geom.no_fill = rewrite_formula(&geom.no_fill, id_map);
            geom.no_line = rewrite_formula(&geom.no_line, id_map);
            geom.no_show = rewrite_formula(&geom.no_show, id_map);
            for r in &mut geom.rows {
                r.x = rewrite_formula(&r.x, id_map);
                r.y = rewrite_formula(&r.y, id_map);
                if let Some(a) = &mut r.a {
                    *a = rewrite_formula(a, id_map);
                }
                if let Some(b) = &mut r.b {
                    *b = rewrite_formula(b, id_map);
                }
                if let Some(c) = &mut r.c {
                    *c = rewrite_formula(c, id_map);
                }
                if let Some(d) = &mut r.d {
                    *d = rewrite_formula(d, id_map);
                }
            }
        }
        node.sheet.sync_all_sections_to_cells();
        for c in &mut node.children {
            apply_rewrite(c, id_map);
        }
    }

    let mut id_map = std::collections::BTreeMap::new();
    assign_ids(root, alloc_id, &mut id_map, 0);
    if !root.children.is_empty() {
        apply_rewrite(root, &id_map);
    }
    root.id
}

pub struct DocumentSession {
    pub doc: VisioDocument,
    pub journal: CommandJournal,
    pub builtin_stencils: Vec<StencilPack>,
    pub imported_stencil_previews: std::collections::BTreeMap<String, Vec<crate::domains::StencilMasterPreview>>,
}

impl DocumentSession {
    pub fn new_blank(title: &str) -> Self {
        let mut doc = VisioDocument::new_blank(title);
        ReactiveEngine::recalculate_document(&mut doc);
        Self {
            doc,
            journal: CommandJournal::new(),
            builtin_stencils: build_all_builtin_stencils(),
            imported_stencil_previews: std::collections::BTreeMap::new(),
        }
    }

    pub fn load_starter_template(&mut self, template_id: &str) {
        self.doc = instantiate_starter_template(template_id);
        self.journal = CommandJournal::new();
    }

    pub fn open_bytes(&mut self, bytes: &[u8], ext: &str) -> Result<(), CoreError> {
        let clean_ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean_ext.as_str() {
            "vsdx" | "vsdm" | "vssx" | "vssm" | "vstx" | "vstm" => {
                let (doc, _) = read_vsdx(bytes)?;
                self.doc = doc;
            }
            "vsd" | "vss" | "vst" | "vdx" | "vdw" | "emf" | "wmf" => {
                self.doc = vexio_vsd_legacy::read_legacy_auto(bytes, &clean_ext)?;
            }
            "ldif" => {
                let mut doc = VisioDocument::new_blank("Imported Directory Org Chart");
                let text = String::from_utf8_lossy(bytes);
                crate::plan2_connectors::import_entra_or_ldif_org_chart(&mut doc, 1, &text)
                    .map_err(|e| CoreError::UnsupportedExtension(e.to_string()))?;
                self.doc = doc;
            }
            "dxf" => {
                let mut doc = VisioDocument::new_blank("CAD DXF Drawing");
                if let Some(page) = doc.page_mut(1) {
                    vexio_cad::import_dxf_to_page(page, bytes)?;
                }
                if let Ok(dxf_str) = std::str::from_utf8(bytes) {
                    let _ = vexio_cad::parse_dxf_extended_entities(dxf_str, &mut doc, 1);
                }
                ReactiveEngine::recalculate_document(&mut doc);
                self.doc = doc;
            }
            "dwg" => {
                let mut doc = VisioDocument::new_blank("CAD DWG Drawing");
                if let Some(page) = doc.page_mut(1) {
                    vexio_cad::import_dwg_to_page(page, bytes)?;
                }
                ReactiveEngine::recalculate_document(&mut doc);
                self.doc = doc;
            }
            _ => return Err(CoreError::UnsupportedExtension(clean_ext)),
        }
        self.journal = CommandJournal::new();
        Ok(())
    }

    pub fn save_format(&self, ext: &str) -> Result<Vec<u8>, CoreError> {
        let first_pid = self
            .doc
            .pages
            .iter()
            .find(|p| !p.is_background)
            .or_else(|| self.doc.pages.first())
            .map(|p| p.id)
            .unwrap_or(1);
        self.save_page_format(first_pid, ext)
    }

    pub fn save_page_format(&self, page_id: u32, ext: &str) -> Result<Vec<u8>, CoreError> {
        let clean_ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        let pid = if self.doc.page(page_id).is_some() {
            page_id
        } else {
            self.doc.pages.first().map(|p| p.id).unwrap_or(1)
        };
        match clean_ext.as_str() {
            "vsdx" | "vsdm" | "vssx" | "vssm" | "vstx" | "vstm" => {
                let kind = VsdxPackageKind::from_extension(&clean_ext);
                Ok(write_vsdx(&self.doc, kind)?)
            }
            "vsd" | "vss" | "vst" => Ok(vexio_vsd_legacy::write_vsd_cfb(&self.doc)?),
            "vdx" => Ok(vexio_vsd_legacy::write_vdx_xml(&self.doc).into_bytes()),
            "dxf" => Ok(vexio_cad::export_page_to_dxf(&self.doc, pid).into_bytes()),
            "dwg" => Ok(vexio_cad::encode_dwg_r2000_from_page(&self.doc, pid)?),
            "pdf" => Ok(export_document_pdf(&self.doc)),
            "svg" => Ok(export_page_svg(&self.doc, pid).into_bytes()),
            "eps" => Ok(export_page_eps(&self.doc, pid).into_bytes()),
            "html" => Ok(export_document_html_report(&self.doc).into_bytes()),
            "pptx" => crate::enterprise_plan2::export_presentation_pptx(&self.doc, &[])
                .map_err(|e| CoreError::UnsupportedExtension(e.to_string())),
            "docx" => crate::enterprise_plan2::export_word_process_spec_docx(&self.doc)
                .map_err(|e| CoreError::UnsupportedExtension(e.to_string())),
            "powerautomate" | "flow.json" => crate::plan2_connectors::export_power_automate_flow_json(
                &self.doc, pid,
            )
            .map(String::into_bytes)
            .map_err(|e| CoreError::UnsupportedExtension(e.to_string())),
            "powerbi" | "pbi.json" => crate::plan2_connectors::export_power_bi_visual_json(
                &self.doc, pid,
            )
            .map(String::into_bytes)
            .map_err(|e| CoreError::UnsupportedExtension(e.to_string())),
            "png" => Ok(export_page_png(&self.doc, pid, 192)),
            "jpg" | "jpeg" => Ok(export_page_jpeg(&self.doc, pid, 192, 92)),
            _ => Err(CoreError::UnsupportedExtension(clean_ext)),
        }
    }

    pub fn drop_builtin_shape(
        &mut self,
        page_id: u32,
        category: &str,
        master_name: &str,
        x_in: f64,
        y_in: f64,
    ) -> Result<u64, CoreError> {
        let template = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(category) || p.category.eq_ignore_ascii_case(category))
            .and_then(|p| {
                p.masters
                    .iter()
                    .find(|m| m.name.eq_ignore_ascii_case(master_name))
            })
            .or_else(|| {
                self.builtin_stencils.iter().find_map(|p| {
                    p.masters
                        .iter()
                        .find(|m| m.name.eq_ignore_ascii_case(master_name))
                })
            })
            .and_then(|m| m.shapes.first())
            .cloned();

        let next_id = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);

        let mut alloc_id = next_id;
        let mut shape = template.unwrap_or_else(|| {
            let mut fallback = VisioShape::new_rect(alloc_id, master_name, x_in, y_in, 1.6, 0.9);
            attach_cardinal_ports_to_shape(&mut fallback);
            fallback
        });
        let assigned_root_id = remap_dropped_shape_subtree(&mut shape, &mut alloc_id);
        shape.name = format!("{master_name}.{assigned_root_id}");
        shape.name_u = shape.name.clone();
        if shape.text.is_empty() && shape.children.is_empty() {
            shape.text = master_name.to_string();
        }
        shape.sheet.set_formula(
            "PinX",
            format!("{} in", format_clean_f64(x_in)),
            Some(Unit::Inch),
        );
        shape.sheet.set_formula(
            "PinY",
            format!("{} in", format_clean_f64(y_in)),
            Some(Unit::Inch),
        );

        self.journal
            .execute(&mut self.doc, DocCommand::AddShape { page_id, shape })?;
        Ok(assigned_root_id)
    }

    /// Interactive 8-Handle Resize + Rotation transaction shared across WASM, C-ABI (WinUI 3 / Swift), and JNI.
    #[allow(clippy::too_many_arguments)]
    pub fn transform_shape(
        &mut self,
        page_id: u32,
        shape_id: u64,
        pin_x_in: f64,
        pin_y_in: f64,
        mut width_in: f64,
        mut height_in: f64,
        angle_deg: f64,
    ) -> Result<(), CoreError> {
        let Some(sh) = self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)) else {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
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
                shape_id,
                cell_name: cname.into(),
                old_formula: old_f,
                new_formula: new_f,
                force_setf: false,
            });
        }
        for (loc_cell, prop_expr) in [("LocPinX", "=Width*0.5"), ("LocPinY", "=Height*0.5")] {
            if let Some(c) = sh.sheet.get_cell(loc_cell) {
                let up = c.formula.to_ascii_uppercase();
                if !up.contains("WIDTH") && !up.contains("HEIGHT") && !c.is_guarded() {
                    cmds.push(DocCommand::SetCellFormula {
                        page_id,
                        shape_id,
                        cell_name: loc_cell.into(),
                        old_formula: c.formula.clone(),
                        new_formula: prop_expr.into(),
                        force_setf: false,
                    });
                }
            }
        }
        if !sh.sheet.get_bool("LockRotate").unwrap_or(false) {
            let old_a = sh
                .sheet
                .get_cell("Angle")
                .map(|c| c.formula.clone())
                .unwrap_or_else(|| "0 deg".into());
            cmds.push(DocCommand::SetCellFormula {
                page_id,
                shape_id,
                cell_name: "Angle".into(),
                old_formula: old_a,
                new_formula: format!("{} deg", format_clean_f64(angle_deg)),
                force_setf: false,
            });
        }

        self.journal.execute(
            &mut self.doc,
            DocCommand::Batch {
                label: "Transform Shape".into(),
                commands: cmds,
            },
        )?;
        route_page_connectors(&mut self.doc, page_id);
        Ok(())
    }

    /// Import an external Microsoft Visio `.vssx`, `.vssm`, `.vss`, `.vsdx`, or `.vdx` stencil package
    /// directly into the active session's stencil catalog with lossless geometry, `ForeignData` SVGs/PNGs,
    /// proportional resizing, and rendered SVG thumbnails!
    pub fn import_stencil_package(
        &mut self,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<(String, Vec<crate::domains::StencilMasterPreview>), CoreError> {
        let ext = file_name
            .rsplit('.')
            .next()
            .unwrap_or("vssx")
            .trim()
            .to_ascii_lowercase();
        let imported_doc = match ext.as_str() {
            "vss" | "vsd" | "vst" | "vdx" | "vdw" => {
                vexio_vsd_legacy::read_legacy_auto(bytes, &ext)?
            }
            _ => {
                let (d, _) = read_vsdx(bytes)?;
                d
            }
        };

        let stem = file_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(file_name)
            .split('.')
            .next()
            .unwrap_or("Imported Stencil")
            .trim();
        let pack_title = if stem.is_empty() {
            "Imported Visio Stencil".to_string()
        } else {
            stem.replace(['_', '-'], " ")
        };

        let imported_styles = imported_doc.styles.clone();
        let imported_themes = imported_doc.themes.clone();
        let mut masters = imported_doc.masters;
        if masters.is_empty() {
            let source_page = imported_doc
                .pages
                .iter()
                .find(|p| !p.is_background && !p.shapes.is_empty())
                .or_else(|| imported_doc.pages.iter().find(|p| !p.is_background))
                .or_else(|| imported_doc.pages.first());
            if let Some(first_page) = source_page {
                for (idx, sh) in first_page.shapes.iter().enumerate() {
                    if sh.kind == ShapeKind::Connector1D || sh.kind == ShapeKind::Guide {
                        continue;
                    }
                    let m_name = if !sh.text.trim().is_empty() {
                        sh.text.lines().next().unwrap_or(&sh.name).trim().to_string()
                    } else {
                        sh.name.clone()
                    };
                    masters.push(vexio_shapesheet::VisioMaster {
                        id: (idx + 1) as u32,
                        name: m_name.clone(),
                        name_u: m_name,
                        base_id: format!("{{IMPORTED-{:04}}}", idx + 1),
                        unique_id: format!("{{IMPORTED-U-{:04}}}", idx + 1),
                        prompt: pack_title.clone(),
                        icon_bytes: None,
                        page_sheet: vexio_shapesheet::ShapeSheet::new(),
                        shapes: vec![sh.clone()],
                    });
                }
            }
        } else {
            // Retain non-empty masters; if a drawing `.vsdx` has both 2D/Group masters and pure 1D connector masters,
            // prefer 2D/Group/Foreign masters when present so Stencil Navigator shows placeable shapes cleanly.
            masters.retain(|m| !m.shapes.is_empty());
            if masters
                .iter()
                .any(|m| m.shapes.first().is_some_and(|s| s.kind != ShapeKind::Connector1D))
            {
                masters.retain(|m| m.shapes.first().is_some_and(|s| s.kind != ShapeKind::Connector1D));
            }
        }

        // Normalize imported masters for smooth center-pinned 8-handle resizing, rotation, and cardinal ports
        for m in &mut masters {
            for s in &mut m.shapes {
                s.sheet.set_formula("LocPinX", "=Width*0.5", Some(Unit::Inch));
                s.sheet.set_formula("LocPinY", "=Height*0.5", Some(Unit::Inch));
                s.sheet.set_formula("LockWidth", "FALSE", Some(Unit::Boolean));
                s.sheet.set_formula("LockHeight", "FALSE", Some(Unit::Boolean));
                s.sheet.set_formula("LockAspect", "FALSE", Some(Unit::Boolean));
                s.sheet.set_formula("LockRotate", "FALSE", Some(Unit::Boolean));
                attach_cardinal_ports_to_shape(s);
            }
        }

        let preview_w_in = 1.50;
        let preview_h_in = 1.00;
        let mut mini_doc = VisioDocument::new_blank("ImportedStencilPreview");
        if !imported_styles.is_empty() {
            mini_doc.styles = imported_styles;
        }
        if !imported_themes.is_empty() {
            mini_doc.themes = imported_themes;
        }
        if let Some(page) = mini_doc.page_mut(1) {
            page.page_sheet.set_formula("PageWidth", "1.5 in", Some(Unit::Inch));
            page.page_sheet.set_formula("PageHeight", "1 in", Some(Unit::Inch));
            page.page_sheet
                .set_formula("PageColor", "=RGB(255,255,255)", Some(Unit::Color));
        }

        let mut previews = Vec::with_capacity(masters.len());
        for m in &masters {
            let mut orig_w = 1.5;
            let mut orig_h = 1.0;
            let mut fill_formula = "=RGB(239,246,255)".to_string();
            let mut line_formula = "=RGB(37,99,235)".to_string();
            if let Some(page) = mini_doc.page_mut(1) {
                page.shapes.clear();
                if let Some(tpl) = m.shapes.first() {
                    let mut sh = tpl.clone();
                    // Keep `sh.id = tpl.id` so Group Master sub-shapes never collide with `sh.id`
                    // and internal `Sheet.<root_id>!` formulas inside `sh.children` continue to resolve!
                    sh.text.clear();
                    orig_w = sh
                        .sheet
                        .get_f64("Width")
                        .filter(|v| v.is_finite() && v.abs() > 0.05)
                        .map(f64::abs)
                        .unwrap_or(1.5)
                        .max(0.15);
                    orig_h = sh
                        .sheet
                        .get_f64("Height")
                        .filter(|v| v.is_finite() && v.abs() > 0.05)
                        .map(f64::abs)
                        .unwrap_or(1.0)
                        .max(0.15);
                    if let Some(fc) = sh.sheet.get_cell("FillForegnd") {
                        fill_formula = fc.formula.clone();
                    }
                    if let Some(lc) = sh.sheet.get_cell("LineColor") {
                        line_formula = lc.formula.clone();
                    }
                    let scale = (1.24 / orig_w).min(0.80 / orig_h);
                    sh.sheet.set_formula(
                        "Width",
                        format!("{} in", format_clean_f64((orig_w * scale).max(0.04))),
                        Some(Unit::Inch),
                    );
                    sh.sheet.set_formula(
                        "Height",
                        format!("{} in", format_clean_f64((orig_h * scale).max(0.04))),
                        Some(Unit::Inch),
                    );
                    sh.sheet.set_formula(
                        "PinX",
                        format!("{} in", format_clean_f64(preview_w_in * 0.5)),
                        Some(Unit::Inch),
                    );
                    sh.sheet.set_formula(
                        "PinY",
                        format!("{} in", format_clean_f64(preview_h_in * 0.5)),
                        Some(Unit::Inch),
                    );
                    page.shapes.push(sh);
                }
            }
            ReactiveEngine::recalculate_document(&mut mini_doc);
            let svg = export_page_svg(&mini_doc, 1);
            previews.push(crate::domains::StencilMasterPreview {
                name: m.name.clone(),
                prompt: m.prompt.clone(),
                category: pack_title.clone(),
                width_in: orig_w,
                height_in: orig_h,
                fill_formula,
                line_formula,
                svg,
            });
        }

        let new_pack = StencilPack {
            title: pack_title.clone(),
            category: pack_title.clone(),
            masters,
        };
        if let Some(existing) = self
            .builtin_stencils
            .iter_mut()
            .find(|p| p.title.eq_ignore_ascii_case(&pack_title))
        {
            *existing = new_pack;
        } else {
            self.builtin_stencils.insert(0, new_pack);
        }

        self.imported_stencil_previews
            .insert(pack_title.clone(), previews.clone());

        Ok((pack_title, previews))
    }

    /// Draw a straight 1D Line segment (`EndArrow = 0`, `ShapeRouteStyle = 1`).
    pub fn draw_line(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
    ) -> Result<u64, CoreError> {
        let sid = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);
        let (ex, ey) = if (x2_in - x1_in).hypot(y2_in - y1_in) < 0.05 {
            (x1_in + 1.5, y1_in)
        } else {
            (x2_in, y2_in)
        };
        let mut line = VisioShape::new_connector(sid, format!("Line.{sid}"), x1_in, y1_in, ex, ey);
        line.sheet
            .set_formula("ShapeRouteStyle", "1", Some(Unit::Number));
        line.sheet.set_formula("EndArrow", "0", Some(Unit::Number));
        line.sheet
            .set_formula("BeginArrow", "0", Some(Unit::Number));
        line.sheet
            .set_formula("LineWeight", "1.5 pt", Some(Unit::Point));
        line.sheet
            .set_formula("LineColor", "=RGB(30,41,59)", Some(Unit::Color));
        self.journal.execute(
            &mut self.doc,
            DocCommand::AddShape {
                page_id,
                shape: line,
            },
        )?;
        route_page_connectors(&mut self.doc, page_id);
        Ok(sid)
    }

    /// Draw a curved 1D Arc (`ArcTo` with configurable `bow_in`, preserved across connector re-routing).
    pub fn draw_arc(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
        bow_in: f64,
    ) -> Result<u64, CoreError> {
        let sid = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);
        let (ex, ey) = if (x2_in - x1_in).hypot(y2_in - y1_in) < 0.05 {
            (x1_in + 1.8, y1_in + 0.6)
        } else {
            (x2_in, y2_in)
        };
        let chord = (ex - x1_in).hypot(ey - y1_in).max(0.2);
        let eff_bow = if bow_in.abs() < 0.02 {
            chord * 0.28
        } else {
            bow_in
        };
        let mut arc = VisioShape::new_connector(sid, format!("Arc.{sid}"), x1_in, y1_in, ex, ey);
        arc.sheet
            .set_formula("ShapeRouteStyle", "2", Some(Unit::Number));
        arc.sheet.set_formula("EndArrow", "0", Some(Unit::Number));
        arc.sheet
            .set_formula("BeginArrow", "0", Some(Unit::Number));
        arc.sheet
            .set_formula("LineWeight", "1.5 pt", Some(Unit::Point));
        arc.sheet
            .set_formula("LineColor", "=RGB(30,41,59)", Some(Unit::Color));
        arc.sheet.user_cells.insert(
            "preserve1dgeometry".into(),
            UserCellRow {
                name: "Preserve1DGeometry".into(),
                value: "TRUE".into(),
                prompt: "Preserve custom 1D ArcTo geometry".into(),
            },
        );
        let mut geom = GeometrySection::new(1);
        geom.no_fill = "TRUE".into();
        geom.rows = vec![
            GeometryRow::move_to(1, "0 in", "0 in"),
            GeometryRow::arc_to(
                2,
                "Width",
                "0 in",
                format!("{} in", format_clean_f64(eff_bow)),
            ),
        ];
        arc.sheet.add_geometry(geom);
        arc.sheet.sync_all_sections_to_cells();
        self.journal.execute(
            &mut self.doc,
            DocCommand::AddShape {
                page_id,
                shape: arc,
            },
        )?;
        Ok(sid)
    }

    /// Draw a basic 2D geometric shape or standalone Text Block (`rect`, `round_rect`, `ellipse`,
    /// `diamond`, `triangle`, `hexagon`, `star`, `text`) with cardinal connection ports.
    pub fn draw_primitive_2d(
        &mut self,
        page_id: u32,
        kind: &str,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
    ) -> Result<u64, CoreError> {
        let k = kind.trim().to_ascii_lowercase();
        if k == "line" {
            return self.draw_line(page_id, x1_in, y1_in, x2_in, y2_in);
        }
        if k == "arc" {
            return self.draw_arc(page_id, x1_in, y1_in, x2_in, y2_in, 0.0);
        }
        if k == "connector" || k == "connect" {
            return self.connect_points_or_shapes(page_id, x1_in, y1_in, x2_in, y2_in, None, None);
        }

        let span_x = (x2_in - x1_in).abs();
        let span_y = (y2_in - y1_in).abs();
        let is_click = span_x < 0.08 && span_y < 0.08;
        let (w, h) = if is_click {
            match k.as_str() {
                "text" => (1.8, 0.55),
                "ellipse" | "circle" | "diamond" | "triangle" | "star" | "hexagon" => (1.3, 1.3),
                _ => (1.6, 1.0),
            }
        } else {
            (span_x.max(0.35), span_y.max(0.28))
        };
        let cx = (x1_in + x2_in) * 0.5;
        let cy = (y1_in + y2_in) * 0.5;
        let sid = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);

        let label_prefix = match k.as_str() {
            "round_rect" | "rounded_rect" | "rounded" => "Rounded Rectangle",
            "ellipse" | "circle" | "oval" => "Ellipse",
            "diamond" => "Diamond",
            "triangle" => "Triangle",
            "hexagon" => "Hexagon",
            "star" => "Star",
            "text" => "Text",
            _ => "Rectangle",
        };
        let mut sh = VisioShape::new_rect(sid, format!("{label_prefix}.{sid}"), cx, cy, w, h);

        sh.sheet
            .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
        sh.sheet
            .set_formula("FillBkgnd", "=RGB(219,234,254)", Some(Unit::Color));
        sh.sheet
            .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
        sh.sheet
            .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
        sh.sheet
            .set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));
        sh.sheet
            .set_formula("Char.Size", "10 pt", Some(Unit::Point));

        match k.as_str() {
            "round_rect" | "rounded_rect" | "rounded" => {
                sh.sheet
                    .set_formula("Rounding", "0.14 in", Some(Unit::Inch));
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "Width*0.12", "0 in"),
                    GeometryRow::line_to(2, "Width*0.88", "0 in"),
                    GeometryRow::arc_to(3, "Width", "Height*0.12", "-0.04 in"),
                    GeometryRow::line_to(4, "Width", "Height*0.88"),
                    GeometryRow::arc_to(5, "Width*0.88", "Height", "-0.04 in"),
                    GeometryRow::line_to(6, "Width*0.12", "Height"),
                    GeometryRow::arc_to(7, "0 in", "Height*0.88", "-0.04 in"),
                    GeometryRow::line_to(8, "0 in", "Height*0.12"),
                    GeometryRow::arc_to(9, "Width*0.12", "0 in", "-0.04 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "ellipse" | "circle" | "oval" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![GeometryRow {
                    ix: 1,
                    kind: GeometryRowKind::Ellipse,
                    x: "Width*0.5".into(),
                    y: "Height*0.5".into(),
                    a: Some("Width".into()),
                    b: Some("Height*0.5".into()),
                    c: Some("Width*0.5".into()),
                    d: Some("Height".into()),
                    e: None,
                    deleted: false,
                }];
                sh.sheet.add_geometry(g);
            }
            "diamond" => {
                sh.sheet
                    .set_formula("FillForegnd", "=RGB(254,243,199)", Some(Unit::Color));
                sh.sheet
                    .set_formula("LineColor", "=RGB(217,119,6)", Some(Unit::Color));
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "Width*0.5", "0 in"),
                    GeometryRow::line_to(2, "Width", "Height*0.5"),
                    GeometryRow::line_to(3, "Width*0.5", "Height"),
                    GeometryRow::line_to(4, "0 in", "Height*0.5"),
                    GeometryRow::line_to(5, "Width*0.5", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "triangle" => {
                sh.sheet
                    .set_formula("FillForegnd", "=RGB(220,252,231)", Some(Unit::Color));
                sh.sheet
                    .set_formula("LineColor", "=RGB(22,163,74)", Some(Unit::Color));
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "Width", "0 in"),
                    GeometryRow::line_to(3, "Width*0.5", "Height"),
                    GeometryRow::line_to(4, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "hexagon" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "Width*0.22", "0 in"),
                    GeometryRow::line_to(2, "Width*0.78", "0 in"),
                    GeometryRow::line_to(3, "Width", "Height*0.5"),
                    GeometryRow::line_to(4, "Width*0.78", "Height"),
                    GeometryRow::line_to(5, "Width*0.22", "Height"),
                    GeometryRow::line_to(6, "0 in", "Height*0.5"),
                    GeometryRow::line_to(7, "Width*0.22", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "star" => {
                sh.sheet
                    .set_formula("FillForegnd", "=RGB(254,249,195)", Some(Unit::Color));
                sh.sheet
                    .set_formula("LineColor", "=RGB(202,138,4)", Some(Unit::Color));
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "Width*0.5", "Height"),
                    GeometryRow::line_to(2, "Width*0.62", "Height*0.62"),
                    GeometryRow::line_to(3, "Width", "Height*0.62"),
                    GeometryRow::line_to(4, "Width*0.69", "Height*0.38"),
                    GeometryRow::line_to(5, "Width*0.81", "0 in"),
                    GeometryRow::line_to(6, "Width*0.5", "Height*0.24"),
                    GeometryRow::line_to(7, "Width*0.19", "0 in"),
                    GeometryRow::line_to(8, "Width*0.31", "Height*0.38"),
                    GeometryRow::line_to(9, "0 in", "Height*0.62"),
                    GeometryRow::line_to(10, "Width*0.38", "Height*0.62"),
                    GeometryRow::line_to(11, "Width*0.5", "Height"),
                ];
                sh.sheet.add_geometry(g);
            }
            "text" => {
                sh.text = "Text".into();
                sh.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
                sh.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
                sh.sheet
                    .set_formula("Char.Size", "11 pt", Some(Unit::Point));
                let mut g = GeometrySection::rectangle(1);
                g.no_fill = "TRUE".into();
                g.no_line = "TRUE".into();
                sh.sheet.add_geometry(g);
            }
            _ => {}
        }

        attach_cardinal_ports_to_shape(&mut sh);
        self.journal
            .execute(&mut self.doc, DocCommand::AddShape { page_id, shape: sh })?;
        Ok(sid)
    }

    /// Create a 1D Dynamic Connector between optional source/target shapes and/or page coordinates.
    #[allow(clippy::too_many_arguments)]
    pub fn connect_points_or_shapes(
        &mut self,
        page_id: u32,
        x1_in: f64,
        y1_in: f64,
        x2_in: f64,
        y2_in: f64,
        from_shape_id: Option<u64>,
        to_shape_id: Option<u64>,
    ) -> Result<u64, CoreError> {
        let cid = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);
        let (ex, ey) = if from_shape_id.is_none()
            && to_shape_id.is_none()
            && (x2_in - x1_in).hypot(y2_in - y1_in) < 0.05
        {
            (x1_in + 1.5, y1_in)
        } else {
            (x2_in, y2_in)
        };
        let conn = VisioShape::new_connector(
            cid,
            format!("Dynamic connector.{cid}"),
            x1_in,
            y1_in,
            ex,
            ey,
        );
        let mut commands = vec![DocCommand::AddShape {
            page_id,
            shape: conn,
        }];
        if let Some(fid) = from_shape_id.filter(|&id| id > 0) {
            commands.push(DocCommand::AddConnect {
                page_id,
                connect: GlueEndpoint {
                    connector_shape_id: cid,
                    from_cell: "BeginX".into(),
                    target_shape_id: fid,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
            });
        }
        if let Some(tid) = to_shape_id.filter(|&id| id > 0 && Some(id) != from_shape_id) {
            commands.push(DocCommand::AddConnect {
                page_id,
                connect: GlueEndpoint {
                    connector_shape_id: cid,
                    from_cell: "EndX".into(),
                    target_shape_id: tid,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
            });
        }
        self.journal.execute(
            &mut self.doc,
            DocCommand::Batch {
                label: "Draw Connector".into(),
                commands,
            },
        )?;
        route_page_connectors(&mut self.doc, page_id);
        Ok(cid)
    }

    pub fn connect_shapes(
        &mut self,
        page_id: u32,
        from_id: u64,
        to_id: u64,
    ) -> Result<u64, CoreError> {
        self.connect_points_or_shapes(page_id, 0.0, 0.0, 1.0, 0.0, Some(from_id), Some(to_id))
    }

    /// Move a 1D connector or line endpoint (`Begin` or `End`), optionally gluing or ungluing to `target_shape_id`.
    pub fn move_connector_endpoint(
        &mut self,
        page_id: u32,
        shape_id: u64,
        is_begin: bool,
        x_in: f64,
        y_in: f64,
        target_shape_id: Option<u64>,
    ) -> Result<bool, CoreError> {
        let mut ok = false;
        self.journal
            .record_mutation(&mut self.doc, "Move 1D Endpoint", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                let from_cell = if is_begin { "BeginX" } else { "EndX" };
                let (cx_cell, cy_cell) = if is_begin {
                    ("BeginX", "BeginY")
                } else {
                    ("EndX", "EndY")
                };
                if let Some(sh) = page.find_shape_mut(shape_id) {
                    if sh.kind != ShapeKind::Connector1D {
                        return;
                    }
                    let _ = sh.sheet.try_set_formula_respecting_guard(
                        cx_cell,
                        format!("{} in", format_clean_f64(x_in)),
                        false,
                    );
                    let _ = sh.sheet.try_set_formula_respecting_guard(
                        cy_cell,
                        format!("{} in", format_clean_f64(y_in)),
                        false,
                    );
                    ok = true;
                }
                if ok {
                    page.connects.retain(|c| {
                        !(c.connector_shape_id == shape_id
                            && c.from_cell.eq_ignore_ascii_case(from_cell))
                    });
                    if let Some(tid) = target_shape_id.filter(|&id| id > 0 && id != shape_id) {
                        page.connects.push(GlueEndpoint {
                            connector_shape_id: shape_id,
                            from_cell: from_cell.into(),
                            target_shape_id: tid,
                            to_cell: "PinX".into(),
                            is_dynamic: true,
                        });
                    }
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(ok)
    }

    pub fn delete_shapes(&mut self, page_id: u32, shape_ids: &[u64]) -> Result<usize, CoreError> {
        let Some(page) = self.doc.page(page_id) else {
            return Ok(0);
        };
        let mut commands = Vec::new();
        for &sid in shape_ids {
            let Some(shape) = page.find_shape(sid).cloned() else {
                continue;
            };
            if shape.sheet.get_bool("LockDelete").unwrap_or(false) {
                continue;
            }
            let removed_connects = page
                .connects
                .iter()
                .filter(|c| c.connector_shape_id == sid || c.target_shape_id == sid)
                .cloned()
                .collect();
            commands.push(DocCommand::DeleteShape {
                page_id,
                shape,
                removed_connects,
            });
        }
        let deleted = commands.len();
        if deleted > 0 {
            self.journal.execute(
                &mut self.doc,
                DocCommand::Batch {
                    label: "Delete Shapes".into(),
                    commands,
                },
            )?;
        }
        Ok(deleted)
    }

    pub fn duplicate_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        dx_in: f64,
        dy_in: f64,
    ) -> Result<Vec<u64>, CoreError> {
        let mut created = Vec::new();
        self.journal
            .record_mutation(&mut self.doc, "Duplicate Shapes", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                for &sid in shape_ids {
                    let Some(orig) = page.find_shape(sid).cloned() else {
                        continue;
                    };
                    let new_id = page.next_shape_id();
                    let mut clone_sh = orig;
                    clone_sh.id = new_id;
                    let base = clone_sh
                        .name
                        .split('.')
                        .next()
                        .unwrap_or("Shape")
                        .to_string();
                    clone_sh.name = format!("{base}.{new_id}");
                    clone_sh.name_u = clone_sh.name.clone();
                    if clone_sh.kind == ShapeKind::Connector1D {
                        let bx = clone_sh.sheet.get_f64("BeginX").unwrap_or(0.0) + dx_in;
                        let by = clone_sh.sheet.get_f64("BeginY").unwrap_or(0.0) + dy_in;
                        let ex = clone_sh.sheet.get_f64("EndX").unwrap_or(1.0) + dx_in;
                        let ey = clone_sh.sheet.get_f64("EndY").unwrap_or(0.0) + dy_in;
                        clone_sh.sheet.set_formula(
                            "BeginX",
                            format!("{} in", format_clean_f64(bx)),
                            Some(Unit::Inch),
                        );
                        clone_sh.sheet.set_formula(
                            "BeginY",
                            format!("{} in", format_clean_f64(by)),
                            Some(Unit::Inch),
                        );
                        clone_sh.sheet.set_formula(
                            "EndX",
                            format!("{} in", format_clean_f64(ex)),
                            Some(Unit::Inch),
                        );
                        clone_sh.sheet.set_formula(
                            "EndY",
                            format!("{} in", format_clean_f64(ey)),
                            Some(Unit::Inch),
                        );
                    } else {
                        let px = clone_sh.sheet.get_f64("PinX").unwrap_or(0.0) + dx_in;
                        let py = clone_sh.sheet.get_f64("PinY").unwrap_or(0.0) + dy_in;
                        clone_sh.sheet.set_formula(
                            "PinX",
                            format!("{} in", format_clean_f64(px)),
                            Some(Unit::Inch),
                        );
                        clone_sh.sheet.set_formula(
                            "PinY",
                            format!("{} in", format_clean_f64(py)),
                            Some(Unit::Inch),
                        );
                    }
                    page.shapes.push(clone_sh);
                    created.push(new_id);
                }
            });
        Ok(created)
    }

    pub fn set_cell_formula(
        &mut self,
        page_id: u32,
        shape_id: u64,
        cell_name: &str,
        new_formula: &str,
        force_setf: bool,
    ) -> Result<(), CoreError> {
        if shape_id == 0 {
            let Some(page_check) = self.doc.page(page_id) else {
                return Err(CoreError::Journal(crate::journal::JournalError::PageNotFound(
                    page_id,
                )));
            };
            if !force_setf
                && page_check
                    .page_sheet
                    .get_cell(cell_name)
                    .is_some_and(|c| c.is_guarded())
            {
                return Err(CoreError::Journal(crate::journal::JournalError::CellGuarded(
                    cell_name.to_string(),
                )));
            }
            let cname = cell_name.to_string();
            let fstr = new_formula.to_string();
            let _ = self.journal.record_mutation(
                &mut self.doc,
                format!("Set PageSheet {cname}"),
                |doc| {
                    if let Some(p) = doc.page_mut(page_id) {
                        let _ = p.page_sheet.try_set_formula_respecting_guard(
                            &cname,
                            fstr.clone(),
                            force_setf,
                        );
                    }
                    ReactiveEngine::recalculate_document(doc);
                },
            );
            route_page_connectors(&mut self.doc, page_id);
            return Ok(());
        }

        let old_formula = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .and_then(|s| s.sheet.get_cell(cell_name))
            .map(|c| c.formula.clone())
            .unwrap_or_else(|| "0".into());

        self.journal.execute(
            &mut self.doc,
            DocCommand::SetCellFormula {
                page_id,
                shape_id,
                cell_name: cell_name.to_string(),
                old_formula,
                new_formula: new_formula.to_string(),
                force_setf,
            },
        )?;
        route_page_connectors(&mut self.doc, page_id);
        Ok(())
    }

    pub fn set_shape_text(
        &mut self,
        page_id: u32,
        shape_id: u64,
        text: &str,
    ) -> Result<(), CoreError> {
        let old_text = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .map(|s| s.text.clone())
            .unwrap_or_default();
        self.journal.execute(
            &mut self.doc,
            DocCommand::SetShapeText {
                page_id,
                shape_id,
                old_text,
                new_text: text.to_string(),
            },
        )?;
        Ok(())
    }

    pub fn reorder_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        mode: &str,
    ) -> Result<usize, CoreError> {
        if shape_ids.is_empty() {
            return Ok(0);
        }
        let mut moved = 0usize;
        let act = mode.trim().to_ascii_lowercase();
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Reorder Shapes Z-Order", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                match act.as_str() {
                    "front" | "bring_to_front" => {
                        let mut picked = Vec::new();
                        page.shapes.retain(|s| {
                            if shape_ids.contains(&s.id) {
                                picked.push(s.clone());
                                false
                            } else {
                                true
                            }
                        });
                        moved = picked.len();
                        page.shapes.extend(picked);
                    }
                    "back" | "send_to_back" => {
                        let mut picked = Vec::new();
                        page.shapes.retain(|s| {
                            if shape_ids.contains(&s.id) {
                                picked.push(s.clone());
                                false
                            } else {
                                true
                            }
                        });
                        moved = picked.len();
                        picked.append(&mut page.shapes);
                        page.shapes = picked;
                    }
                    "forward" | "bring_forward" => {
                        for i in (0..page.shapes.len().saturating_sub(1)).rev() {
                            if shape_ids.contains(&page.shapes[i].id)
                                && !shape_ids.contains(&page.shapes[i + 1].id)
                            {
                                page.shapes.swap(i, i + 1);
                                moved += 1;
                            }
                        }
                    }
                    "backward" | "send_backward" => {
                        for i in 1..page.shapes.len() {
                            if shape_ids.contains(&page.shapes[i].id)
                                && !shape_ids.contains(&page.shapes[i - 1].id)
                            {
                                page.shapes.swap(i - 1, i);
                                moved += 1;
                            }
                        }
                    }
                    _ => {}
                }
            });
        Ok(moved)
    }

    pub fn add_page(&mut self, name: &str) -> Result<u32, CoreError> {
        let next_id = self.doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let ref_page = self
            .doc
            .pages
            .iter()
            .find(|p| !p.is_background)
            .or_else(|| self.doc.pages.first());
        let (w_in, h_in, unit_opt) = ref_page
            .map(|p| {
                (
                    p.width_in(),
                    p.height_in(),
                    p.page_sheet.get_str("User.DisplayUnit"),
                )
            })
            .unwrap_or((297.0 / 25.4, 210.0 / 25.4, Some("mm".to_string())));
        let mut page = VisioPage::new(next_id, name, w_in, h_in);
        if let Some(u) = unit_opt {
            page.page_sheet.set_formula(
                "User.DisplayUnit",
                format!("\"{}\"", u.replace('"', "")),
                Some(Unit::String),
            );
        }
        self.journal
            .execute(&mut self.doc, DocCommand::AddPage { page })?;
        Ok(next_id)
    }

    pub fn rename_page(&mut self, page_id: u32, new_name: &str) -> bool {
        let clean = new_name.trim();
        if clean.is_empty() || self.doc.page(page_id).is_none() {
            return false;
        }
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Rename Page", |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    p.name = clean.to_string();
                }
            });
        true
    }

    pub fn duplicate_page(&mut self, page_id: u32) -> Result<u32, CoreError> {
        let Some(mut cloned) = self.doc.page(page_id).cloned() else {
            return self.add_page("Page Copy");
        };
        let next_id = self.doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        cloned.id = next_id;
        cloned.name = format!("{} (Copy)", cloned.name);
        self.journal
            .execute(&mut self.doc, DocCommand::AddPage { page: cloned })?;
        Ok(next_id)
    }

    pub fn delete_page(&mut self, page_id: u32) -> bool {
        if self.doc.pages.len() <= 1 || self.doc.page(page_id).is_none() {
            return false;
        }
        let mut removed = false;
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Delete Page", |doc| {
                let before = doc.pages.len();
                doc.pages.retain(|p| p.id != page_id);
                doc.comments.retain(|c| c.page_id != page_id);
                removed = doc.pages.len() < before;
            });
        removed
    }

    pub fn layout(&mut self, page_id: u32, key: &str) {
        let algorithm = match key.trim().to_ascii_lowercase().as_str() {
            "lr" | "lefttoright" | "hierarchical_lr" | "swimlane_lr" => {
                LayoutAlgorithm::HierarchicalLeftToRight
            }
            "bt" | "bottomtotop" | "hierarchical_bt" => LayoutAlgorithm::HierarchicalBottomToTop,
            "rl" | "righttoleft" | "hierarchical_rl" => LayoutAlgorithm::HierarchicalRightToLeft,
            "radial" => LayoutAlgorithm::Radial,
            "tree" | "compacttree" | "compact_tree" => LayoutAlgorithm::CompactTree,
            "circular" | "force_directed" | "compact_orthogonal" | "grid" => {
                LayoutAlgorithm::Circular
            }
            _ => LayoutAlgorithm::HierarchicalTopToBottom,
        };
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Auto-Layout", |doc| {
                apply_auto_layout(
                    doc,
                    page_id,
                    LayoutOptions {
                        algorithm,
                        ..LayoutOptions::default()
                    },
                );
            });
    }

    pub fn validate(&self, key: &str) -> Vec<ValidationIssue> {
        let rs = match key.to_ascii_lowercase().as_str() {
            "uml" | "uml25" => DiagramRuleSet::Uml25,
            "workflow" | "sharepoint" => DiagramRuleSet::SharePointWorkflow,
            _ => DiagramRuleSet::Bpmn20,
        };
        validate_document(&self.doc, rs)
    }

    pub fn run_macro(&mut self, script: &str) -> Result<AutomationExecutionReport, CoreError> {
        let mut report = None;
        let mut err = None;
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Run VBA Macro", |doc| {
                match execute_vba_macro(doc, script, SandboxLimits::default()) {
                    Ok(r) => report = Some(r),
                    Err(e) => err = Some(e),
                }
            });
        if let Some(e) = err {
            return Err(CoreError::Automation(e));
        }
        Ok(report.unwrap())
    }

    /// Rename the active document title cleanly.
    pub fn set_document_title(&mut self, title: &str) {
        let trimmed = title.trim();
        if !trimmed.is_empty() {
            self.doc.properties.title = trimmed.to_string();
        }
    }

    /// Apply standard character & paragraph formatting (`Font`, `Size`, `B/I/U/S`, `Color`,
    /// `Highlight`, `HorzAlign`, `VerticalAlign`, `Bullet`) across one or more shapes as a single Undo/Redo batch.
    pub fn apply_text_style_patch(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        patch: &TextStylePatch,
    ) -> Result<usize, CoreError> {
        let Some(page) = self.doc.page(page_id) else {
            return Ok(0);
        };
        let mut cmds: Vec<DocCommand> = Vec::new();
        let mut touched = 0usize;

        for &sid in shape_ids {
            let Some(sh) = page.find_shape(sid) else {
                continue;
            };
            let before_len = cmds.len();
            let push_cell = |cmds: &mut Vec<DocCommand>, cname: &str, new_f: String| {
                let existing = sh.sheet.get_cell(cname);
                if existing.is_some_and(|c| c.formula == new_f) {
                    return;
                }
                let old_f = existing
                    .map(|c| c.formula.clone())
                    .unwrap_or_else(|| "0".into());
                cmds.push(DocCommand::SetCellFormula {
                    page_id,
                    shape_id: sid,
                    cell_name: cname.to_string(),
                    old_formula: old_f,
                    new_formula: new_f,
                    force_setf: false,
                });
            };

            if let Some(font) = &patch.font_family {
                let clean = font.trim().replace('"', "");
                if !clean.is_empty() {
                    push_cell(&mut cmds, "Char.Font", format!("\"{clean}\""));
                }
            }
            if let Some(sz) = patch.font_size_pt {
                let clamped = sz.clamp(6.0, 144.0);
                push_cell(&mut cmds, "Char.Size", format!("{} pt", format_clean_f64(clamped)));
            } else if let Some(delta) = patch.size_delta_pt {
                let cur_pt = sh
                    .sheet
                    .get_f64("Char.Size")
                    .map(|inches| inches * 72.0)
                    .unwrap_or(10.0);
                let next_pt = (cur_pt + delta).clamp(6.0, 144.0);
                push_cell(&mut cmds, "Char.Size", format!("{} pt", format_clean_f64(next_pt)));
            }
            if let Some(explicit_style) = patch.char_style {
                push_cell(&mut cmds, "Char.Style", (explicit_style & 0x0F).to_string());
            } else if let Some(mask) = patch.toggle_style_mask {
                let cur = sh.sheet.get_f64("Char.Style").unwrap_or(0.0).round() as u8;
                let next = (cur ^ mask) & 0x0F;
                push_cell(&mut cmds, "Char.Style", next.to_string());
            } else if patch.bold.is_some()
                || patch.italic.is_some()
                || patch.underline.is_some()
                || patch.strikethrough.is_some()
            {
                let mut next = (sh.sheet.get_f64("Char.Style").unwrap_or(0.0).round() as u8) & 0x0F;
                if let Some(b) = patch.bold {
                    if b { next |= 1; } else { next &= !1; }
                }
                if let Some(i) = patch.italic {
                    if i { next |= 2; } else { next &= !2; }
                }
                if let Some(u) = patch.underline {
                    if u { next |= 4; } else { next &= !4; }
                }
                if let Some(s) = patch.strikethrough {
                    if s { next |= 8; } else { next &= !8; }
                }
                push_cell(&mut cmds, "Char.Style", next.to_string());
            }
            if let Some(col_str) = &patch.text_color {
                if let Some(c) = vexio_shapesheet::RgbaColor::from_hex(col_str) {
                    push_cell(
                        &mut cmds,
                        "Char.Color",
                        format!("=RGB({},{},{})", c.r, c.g, c.b),
                    );
                } else if col_str.trim().starts_with("=RGB(") || col_str.trim().starts_with("RGB(") {
                    let f = if col_str.trim().starts_with('=') {
                        col_str.trim().to_string()
                    } else {
                        format!("={}", col_str.trim())
                    };
                    push_cell(&mut cmds, "Char.Color", f);
                }
            }
            if let Some(bg_str) = &patch.text_bg_color {
                let bgt = bg_str.trim();
                if bgt.is_empty() || bgt.eq_ignore_ascii_case("none") || bgt.eq_ignore_ascii_case("transparent") {
                    push_cell(&mut cmds, "TextBkgndTrans", "1".to_string());
                } else if let Some(c) = vexio_shapesheet::RgbaColor::from_hex(bgt) {
                    push_cell(
                        &mut cmds,
                        "TextBkgnd",
                        format!("=RGB({},{},{})", c.r, c.g, c.b),
                    );
                    push_cell(&mut cmds, "TextBkgndTrans", "0".to_string());
                }
            }
            if let Some(ha) = patch.horiz_align {
                push_cell(&mut cmds, "Para.HorzAlign", ha.min(3).to_string());
            }
            if let Some(va) = patch.vert_align {
                push_cell(&mut cmds, "VerticalAlign", va.min(2).to_string());
            }
            if let Some(bul) = patch.bullet_style {
                push_cell(&mut cmds, "Para.Bullet", bul.min(4).to_string());
            }
            if cmds.len() > before_len {
                touched += 1;
            }
        }

        if !cmds.is_empty() {
            self.journal.execute(
                &mut self.doc,
                DocCommand::Batch {
                    label: "Format Text".into(),
                    commands: cmds,
                },
            )?;
        }
        Ok(touched)
    }

    /// Rotate (`delta_angle_deg`, e.g. `+90` left / `-90` right in Visio's CCW angle convention)
    /// and/or flip (`FlipX`, `FlipY`) all selected stencils and shapes as a single undoable operation.
    pub fn rotate_or_flip_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        delta_angle_deg: f64,
        toggle_flip_x: bool,
        toggle_flip_y: bool,
    ) -> Result<usize, CoreError> {
        let Some(page) = self.doc.page(page_id) else {
            return Ok(0);
        };
        let mut cmds: Vec<DocCommand> = Vec::new();
        let mut touched = 0usize;

        for &sid in shape_ids {
            let Some(sh) = page.find_shape(sid) else {
                continue;
            };
            let before_len = cmds.len();
            if sh.kind == ShapeKind::Connector1D {
                if delta_angle_deg.abs() > 1e-6 {
                    let bx = sh.sheet.get_f64("BeginX").unwrap_or(0.0);
                    let by = sh.sheet.get_f64("BeginY").unwrap_or(0.0);
                    let ex = sh.sheet.get_f64("EndX").unwrap_or(1.0);
                    let ey = sh.sheet.get_f64("EndY").unwrap_or(0.0);
                    let mx = (bx + ex) * 0.5;
                    let my = (by + ey) * 0.5;
                    let (s, c) = delta_angle_deg.to_radians().sin_cos();
                    let nbx = mx + (bx - mx) * c - (by - my) * s;
                    let nby = my + (bx - mx) * s + (by - my) * c;
                    let nex = mx + (ex - mx) * c - (ey - my) * s;
                    let ney = my + (ex - mx) * s + (ey - my) * c;
                    for (cname, val) in [("BeginX", nbx), ("BeginY", nby), ("EndX", nex), ("EndY", ney)] {
                        let old_f = sh
                            .sheet
                            .get_cell(cname)
                            .map(|cl| cl.formula.clone())
                            .unwrap_or_else(|| "0 in".into());
                        cmds.push(DocCommand::SetCellFormula {
                            page_id,
                            shape_id: sid,
                            cell_name: cname.into(),
                            old_formula: old_f,
                            new_formula: format!("{} in", format_clean_f64(val)),
                            force_setf: false,
                        });
                    }
                }
            } else {
                if delta_angle_deg.abs() > 1e-6 && !sh.sheet.get_bool("LockRotate").unwrap_or(false) {
                    let cur_deg = sh.sheet.get_f64("Angle").unwrap_or(0.0).to_degrees();
                    let mut next_deg = (cur_deg + delta_angle_deg) % 360.0;
                    if next_deg > 180.0 {
                        next_deg -= 360.0;
                    } else if next_deg <= -180.0 {
                        next_deg += 360.0;
                    }
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
                        new_formula: format!("{} deg", format_clean_f64(next_deg)),
                        force_setf: false,
                    });
                }
                if toggle_flip_x {
                    let cur = sh.sheet.get_bool("FlipX").unwrap_or(false);
                    let old_f = sh
                        .sheet
                        .get_cell("FlipX")
                        .map(|c| c.formula.clone())
                        .unwrap_or_else(|| "FALSE".into());
                    cmds.push(DocCommand::SetCellFormula {
                        page_id,
                        shape_id: sid,
                        cell_name: "FlipX".into(),
                        old_formula: old_f,
                        new_formula: if cur { "FALSE".into() } else { "TRUE".into() },
                        force_setf: false,
                    });
                }
                if toggle_flip_y {
                    let cur = sh.sheet.get_bool("FlipY").unwrap_or(false);
                    let old_f = sh
                        .sheet
                        .get_cell("FlipY")
                        .map(|c| c.formula.clone())
                        .unwrap_or_else(|| "FALSE".into());
                    cmds.push(DocCommand::SetCellFormula {
                        page_id,
                        shape_id: sid,
                        cell_name: "FlipY".into(),
                        old_formula: old_f,
                        new_formula: if cur { "FALSE".into() } else { "TRUE".into() },
                        force_setf: false,
                    });
                }
            }
            if cmds.len() > before_len {
                touched += 1;
            }
        }

        if !cmds.is_empty() {
            self.journal.execute(
                &mut self.doc,
                DocCommand::Batch {
                    label: "Rotate / Flip Shapes".into(),
                    commands: cmds,
                },
            )?;
            route_page_connectors(&mut self.doc, page_id);
        }
        Ok(touched)
    }

    /// Update page dimensions (`A0`–`A5`, `Letter`, `Legal`, `Tabloid`, or custom `width_in × height_in`)
    /// and page display unit (`mm`, `cm`, `m`, `in`, `ft`, `pt`) with Metric-aligned grid spacing.
    pub fn set_page_setup(
        &mut self,
        page_id: u32,
        width_in: f64,
        height_in: f64,
        unit_slug: &str,
    ) -> Result<(), CoreError> {
        let w = width_in.clamp(1.0, 200.0);
        let h = height_in.clamp(1.0, 200.0);
        let u = normalize_display_unit_slug(unit_slug);
        let is_metric = matches!(u, "mm" | "cm" | "m");

        self.journal
            .record_mutation(&mut self.doc, "Page Setup", |doc| {
                if let Some(page) = doc.page_mut(page_id) {
                    if is_metric {
                        let w_mm = w * 25.4;
                        let h_mm = h * 25.4;
                        page.page_sheet.set_formula(
                            "PageWidth",
                            format!("{} mm", format_clean_f64(w_mm)),
                            Some(Unit::Millimeter),
                        );
                        page.page_sheet.set_formula(
                            "PageHeight",
                            format!("{} mm", format_clean_f64(h_mm)),
                            Some(Unit::Millimeter),
                        );
                        page.page_sheet
                            .set_formula("PageScale", "1 mm", Some(Unit::Millimeter));
                        page.page_sheet
                            .set_formula("DrawingScale", "1 mm", Some(Unit::Millimeter));
                        page.page_sheet
                            .set_formula("XGridSpacing", "5 mm", Some(Unit::Millimeter));
                        page.page_sheet
                            .set_formula("YGridSpacing", "5 mm", Some(Unit::Millimeter));
                    } else {
                        page.page_sheet.set_formula(
                            "PageWidth",
                            format!("{} in", format_clean_f64(w)),
                            Some(Unit::Inch),
                        );
                        page.page_sheet.set_formula(
                            "PageHeight",
                            format!("{} in", format_clean_f64(h)),
                            Some(Unit::Inch),
                        );
                        page.page_sheet
                            .set_formula("PageScale", "1 in", Some(Unit::Inch));
                        page.page_sheet
                            .set_formula("DrawingScale", "1 in", Some(Unit::Inch));
                        page.page_sheet
                            .set_formula("XGridSpacing", "0.25 in", Some(Unit::Inch));
                        page.page_sheet
                            .set_formula("YGridSpacing", "0.25 in", Some(Unit::Inch));
                    }
                    page.page_sheet
                        .set_user_cell("DisplayUnit", format!("\"{u}\""), "Preferred Ruler & Inspector Unit");
                }
                route_page_connectors(doc, page_id);
            });
        Ok(())
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextStylePatch {
    #[serde(alias = "font_family")]
    pub font_family: Option<String>,
    #[serde(alias = "font_size_pt")]
    pub font_size_pt: Option<f64>,
    #[serde(alias = "size_delta_pt")]
    pub size_delta_pt: Option<f64>,
    #[serde(alias = "char_style")]
    pub char_style: Option<u8>,
    #[serde(alias = "toggle_style_mask")]
    pub toggle_style_mask: Option<u8>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    #[serde(alias = "strike")]
    pub strikethrough: Option<bool>,
    #[serde(alias = "colorHex", alias = "text_color", alias = "text_color_hex")]
    pub text_color: Option<String>,
    #[serde(alias = "bgColorHex", alias = "text_bg_color", alias = "text_bg_hex")]
    pub text_bg_color: Option<String>,
    #[serde(alias = "horiz_align")]
    pub horiz_align: Option<u8>,
    #[serde(alias = "vert_align")]
    pub vert_align: Option<u8>,
    #[serde(alias = "bullet", alias = "bullet_style")]
    pub bullet_style: Option<u8>,
}

pub fn normalize_display_unit_slug(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "cm" | "centimeter" | "centimeters" => "cm",
        "m" | "meter" | "meters" => "m",
        "in" | "inch" | "inches" | "imperial" | "us" => "in",
        "ft" | "foot" | "feet" => "ft",
        "pt" | "point" | "points" => "pt",
        _ => "mm",
    }
}

/// Resolve ISO 216 Metric (`A0`–`A5`) and ANSI Imperial (`Letter`, `Legal`, `Tabloid`) page presets
/// into canonical `(width_in, height_in, recommended_unit_slug)`.
pub fn resolve_page_preset_dimensions_in(
    preset_slug: &str,
    landscape: bool,
) -> Option<(f64, f64, &'static str)> {
    let (short_in, long_in, unit) = match preset_slug.trim().to_ascii_lowercase().as_str() {
        "a0" => (841.0 / 25.4, 1189.0 / 25.4, "mm"),
        "a1" => (594.0 / 25.4, 841.0 / 25.4, "mm"),
        "a2" => (420.0 / 25.4, 594.0 / 25.4, "mm"),
        "a3" => (297.0 / 25.4, 420.0 / 25.4, "mm"),
        "a4" => (210.0 / 25.4, 297.0 / 25.4, "mm"),
        "a5" => (148.0 / 25.4, 210.0 / 25.4, "mm"),
        "letter" | "us_letter" => (8.5, 11.0, "in"),
        "legal" | "us_legal" => (8.5, 14.0, "in"),
        "tabloid" | "ledger" | "ansi_b" => (11.0, 17.0, "in"),
        _ => return None,
    };
    if landscape {
        Some((long_in, short_in, unit))
    } else {
        Some((short_in, long_in, unit))
    }
}
