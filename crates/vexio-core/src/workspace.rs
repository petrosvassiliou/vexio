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

fn scale_physical_master_subtree(node: &mut VisioShape, scale: f64, is_root: bool) {
    let scale_cell_if_literal = |sheet: &mut vexio_shapesheet::ShapeSheet, key: &str| {
        if let Some(cell) = sheet.get_cell(key) {
            let f_low = cell.formula.to_ascii_lowercase();
            if !f_low.contains("width") && !f_low.contains("height") && !f_low.contains("sheet.") {
                if let Some(v) = sheet.get_f64(key) {
                    if v.is_finite() {
                        sheet.set_formula(
                            key,
                            format!("{} in", format_clean_f64(v * scale)),
                            Some(Unit::Inch),
                        );
                    }
                }
            }
        }
    };
    scale_cell_if_literal(&mut node.sheet, "Width");
    scale_cell_if_literal(&mut node.sheet, "Height");
    if !is_root {
        scale_cell_if_literal(&mut node.sheet, "PinX");
        scale_cell_if_literal(&mut node.sheet, "PinY");
        scale_cell_if_literal(&mut node.sheet, "LocPinX");
        scale_cell_if_literal(&mut node.sheet, "LocPinY");
    }
    let geom_keys: Vec<String> = node
        .sheet
        .cells
        .keys()
        .filter(|k| {
            k.starts_with("Geometry")
                && (k.contains(".X") || k.contains(".Y"))
                && !k.ends_with("Type")
        })
        .cloned()
        .collect();
    for gk in geom_keys {
        scale_cell_if_literal(&mut node.sheet, &gk);
    }
    for child in &mut node.children {
        scale_physical_master_subtree(child, scale, false);
    }
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
        let hide_txt = shape.sheet.get_bool("HideText").unwrap_or(false);
        if shape.text.is_empty() && shape.children.is_empty() && !hide_txt {
            shape.text = master_name.to_string();
            if shape.sheet.get_cell("TxtPinY").is_none() {
                crate::visio_artwork::apply_visio_below_icon_caption(&mut shape);
            }
        }
        let is_unscaled_page = self.doc.page(page_id).is_some_and(|p| {
            let ps = p.page_sheet.get_f64("PageScale").unwrap_or(1.0).abs().max(1e-6);
            let ds = p.page_sheet.get_f64("DrawingScale").unwrap_or(1.0).abs().max(1e-6);
            ((ds / ps) - 1.0).abs() < 1e-3
        });
        if is_unscaled_page
            && !crate::containers::is_container_shape(&shape)
            && !crate::containers::is_swimlane_shape(&shape)
        {
            let orig_w = shape.sheet.get_f64("Width").unwrap_or(1.5).abs();
            let orig_h = shape.sheet.get_f64("Height").unwrap_or(1.0).abs();
            if orig_w > 11.5 || orig_h > 11.5 {
                let fit_scale = (2.60 / orig_w.max(0.1)).min(2.60 / orig_h.max(0.1));
                scale_physical_master_subtree(&mut shape, fit_scale, true);
            }
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

        let has_containers_or_lanes = self.doc.page(page_id).is_some_and(|p| {
            p.shapes.iter().any(|s| {
                crate::containers::is_container_shape(s) || crate::containers::is_swimlane_shape(s)
            })
        });
        if has_containers_or_lanes
            && !crate::containers::is_container_shape(&shape)
            && !crate::containers::is_swimlane_shape(&shape)
        {
            self.journal
                .record_mutation(&mut self.doc, "Drop Stencil Shape", |doc| {
                    if let Some(p) = doc.page_mut(page_id) {
                        p.shapes.push(shape.clone());
                    }
                    ReactiveEngine::recalculate_document(doc);
                    crate::containers::reconcile_containers_and_swimlanes_after_move(
                        doc,
                        page_id,
                        &[assigned_root_id],
                    );
                });
        } else {
            self.journal
                .execute(&mut self.doc, DocCommand::AddShape { page_id, shape })?;
        }
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
    /// Unpacks and imports a single stencil file (`.vssx`, `.vssm`, `.vss`, `.vsdx`, `.vsd`, `.vdx`, `.svg`, `.png`)
    /// OR a multi-file `.zip` stencil/icon archive (`Azure_Public_Service_Icons_V25.zip`,
    /// `2024-microsoft-365-content-icons.zip`, `Visio-Cisco-Stencils-US-V1.zip`, `NetEquip.zip`),
    /// returning one `(pack_title, previews)` entry per discovered stencil pack.
    pub fn import_stencil_archive_packs(
        &mut self,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<Vec<(String, Vec<crate::domains::StencilMasterPreview>)>, CoreError> {
        let ext = file_name
            .rsplit('.')
            .next()
            .unwrap_or("vssx")
            .trim()
            .to_ascii_lowercase();

        if ext == "svg" || ext == "png" {
            let stem = clean_stencil_stem(file_name);
            let pack_title = "Imported Vector Icons".to_string();
            let master = build_foreign_icon_master(1, &stem, &pack_title, &ext, bytes);
            let previews = self.register_imported_masters_pack(&pack_title, vec![master], &[], &[]);
            return Ok(vec![(pack_title, previews)]);
        }

        if ext == "zip" {
            return self.import_zip_stencil_bundle(file_name, bytes);
        }

        self.import_single_stencil_doc_pack(file_name, &ext, bytes)
            .map(|res| vec![res])
    }

    pub fn import_stencil_package(
        &mut self,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<(String, Vec<crate::domains::StencilMasterPreview>), CoreError> {
        let mut packs = self.import_stencil_archive_packs(file_name, bytes)?;
        if packs.is_empty() {
            return Err(CoreError::UnsupportedExtension(file_name.to_string()));
        }
        if packs.len() == 1 {
            Ok(packs.remove(0))
        } else {
            let first_title = packs[0].0.clone();
            let mut combined = Vec::new();
            for (_, mut p) in packs {
                combined.append(&mut p);
            }
            Ok((first_title, combined))
        }
    }

    fn import_zip_stencil_bundle(
        &mut self,
        zip_file_name: &str,
        bytes: &[u8],
    ) -> Result<Vec<(String, Vec<crate::domains::StencilMasterPreview>)>, CoreError> {
        use std::io::{Cursor, Read};
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|e| CoreError::Vsdx(vexio_vsdx::VsdxError::Zip(e)))?;

        // Check whether this `.zip` is actually an OpenXML `.vssx` / `.vsdx` renamed to `.zip`!
        let has_visio_opc = (0..archive.len()).any(|i| {
            archive
                .by_index(i)
                .map(|f| f.name().eq_ignore_ascii_case("visio/document.xml"))
                .unwrap_or(false)
        });
        if has_visio_opc {
            let fallback_name = format!("{}.vssx", clean_stencil_stem(zip_file_name));
            let single = self.import_single_stencil_doc_pack(&fallback_name, "vssx", bytes)?;
            return Ok(vec![single]);
        }

        let mut entry_names = Vec::with_capacity(archive.len());
        for i in 0..archive.len() {
            if let Ok(f) = archive.by_index(i) {
                if !f.is_dir() && !f.name().contains("__MACOSX") {
                    entry_names.push((i, f.name().replace('\\', "/")));
                }
            }
        }

        // In Microsoft's `2024-microsoft-365-content-icons.zip`, each color family has 3 subfolders:
        // `Dark Blue Icon`, `Light Blue Icon`, and `Grey & Blue Icon` (the canonical 2-tone icon!).
        // If `Grey &` folders exist, skip duplicate monochrome `Dark ` / `Light ` variants!
        let has_two_tone_grey = entry_names
            .iter()
            .any(|(_, p)| p.contains("Grey &") || p.contains("Grey and"));

        let mut results: Vec<(String, Vec<crate::domains::StencilMasterPreview>)> = Vec::new();
        let mut svg_groups: std::collections::BTreeMap<String, Vec<(String, String, Vec<u8>)>> =
            std::collections::BTreeMap::new();
        let zip_stem = clean_stencil_stem(zip_file_name);

        for (idx, norm_path) in entry_names {
            let file_only = norm_path.rsplit('/').next().unwrap_or(&norm_path);
            if file_only.starts_with('.') {
                continue;
            }
            let item_ext = file_only
                .rsplit('.')
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();

            if matches!(
                item_ext.as_str(),
                "vssx" | "vssm" | "vstx" | "vsdx" | "vss" | "vsd" | "vst" | "vdx"
            ) {
                let mut buf = Vec::new();
                if let Ok(mut zf) = archive.by_index(idx) {
                    if zf.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                        if let Ok(pack) = self.import_single_stencil_doc_pack(file_only, &item_ext, &buf) {
                            if !pack.1.is_empty() {
                                results.push(pack);
                            }
                        }
                    }
                }
            } else if item_ext == "svg" {
                if has_two_tone_grey
                    && (norm_path.contains("Dark Blue Icon")
                        || norm_path.contains("Light Blue Icon")
                        || norm_path.contains("Dark Purple Icon")
                        || norm_path.contains("Light Purple Icon"))
                {
                    continue;
                }
                let mut buf = Vec::new();
                if let Ok(mut zf) = archive.by_index(idx) {
                    if zf.read_to_end(&mut buf).is_ok() && !buf.is_empty() {
                        let cat_title = derive_svg_zip_category(&zip_stem, &norm_path);
                        let icon_name = clean_icon_display_name(file_only);
                        svg_groups
                            .entry(cat_title)
                            .or_default()
                            .push((icon_name, item_ext, buf));
                    }
                }
            }
        }

        for (cat_title, mut icons) in svg_groups {
            icons.sort_by(|a, b| a.0.cmp(&b.0));
            icons.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
            let mut masters = Vec::with_capacity(icons.len());
            for (i, (icon_name, item_ext, data)) in icons.into_iter().enumerate() {
                masters.push(build_foreign_icon_master(
                    (i + 1) as u32,
                    &icon_name,
                    &cat_title,
                    &item_ext,
                    &data,
                ));
            }
            if !masters.is_empty() {
                let previews = self.register_imported_masters_pack(&cat_title, masters, &[], &[]);
                results.push((cat_title, previews));
            }
        }

        Ok(results)
    }

    fn import_single_stencil_doc_pack(
        &mut self,
        file_name: &str,
        ext: &str,
        bytes: &[u8],
    ) -> Result<(String, Vec<crate::domains::StencilMasterPreview>), CoreError> {
        let imported_doc = match ext {
            "vss" | "vsd" | "vst" | "vdx" | "vdw" => {
                vexio_vsd_legacy::read_legacy_auto(bytes, ext)?
            }
            _ => {
                let (d, _) = read_vsdx(bytes)?;
                d
            }
        };

        let pack_title = clean_stencil_stem(file_name);
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
            masters.retain(|m| !m.shapes.is_empty());
            if masters.iter().any(|m| !m.name.starts_with("More Shapes")) {
                masters.retain(|m| !m.name.starts_with("More Shapes"));
            }
            if masters
                .iter()
                .any(|m| m.shapes.first().is_some_and(|s| s.kind != ShapeKind::Connector1D))
            {
                masters.retain(|m| m.shapes.first().is_some_and(|s| s.kind != ShapeKind::Connector1D));
            }
        }

        let previews = self.register_imported_masters_pack(
            &pack_title,
            masters,
            &imported_styles,
            &imported_themes,
        );
        Ok((pack_title, previews))
    }

    fn register_imported_masters_pack(
        &mut self,
        pack_title: &str,
        mut masters: Vec<vexio_shapesheet::VisioMaster>,
        imported_styles: &[vexio_shapesheet::StyleSheetDef],
        imported_themes: &[vexio_shapesheet::ThemeDef],
    ) -> Vec<crate::domains::StencilMasterPreview> {
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
            mini_doc.styles = imported_styles.to_vec();
        }
        if !imported_themes.is_empty() {
            mini_doc.themes = imported_themes.to_vec();
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
                    sh.text.clear();
                    clear_subtree_preview_text(&mut sh);
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
            let png_data_url = crate::domains::render_stencil_preview_png_data_url(&mini_doc, 1);
            previews.push(crate::domains::StencilMasterPreview {
                name: m.name.clone(),
                prompt: m.prompt.clone(),
                category: pack_title.to_string(),
                width_in: orig_w,
                height_in: orig_h,
                fill_formula,
                line_formula,
                svg,
                png_data_url,
            });
        }

        let new_pack = StencilPack {
            title: pack_title.to_string(),
            category: pack_title.to_string(),
            masters,
        };
        if let Some(existing) = self
            .builtin_stencils
            .iter_mut()
            .find(|p| p.title.eq_ignore_ascii_case(pack_title))
        {
            *existing = new_pack;
        } else {
            self.builtin_stencils.insert(0, new_pack);
        }

        self.imported_stencil_previews
            .insert(pack_title.to_string(), previews.clone());

        previews
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
                sh.sheet.add_control(vexio_shapesheet::ControlRow {
                    name: "Row_1".into(),
                    x: "=Width*0.16".into(),
                    y: "=Height".into(),
                    x_dyn: "=Controls.Row_1.X".into(),
                    y_dyn: "=Controls.Row_1.Y".into(),
                    x_con: "0".into(),
                    y_con: "0".into(),
                    can_glue: false,
                    prompt: "Drag Yellow Handle to Adjust Corner Rounding".into(),
                });
                sh.sheet.set_formula(
                    "Rounding",
                    "=BOUND(Controls.Row_1.X, 0, FALSE, 0.02 in, MIN(Width, Height)*0.48)",
                    Some(Unit::Inch),
                );
                let r_expr = "BOUND(Controls.Row_1.X, 0, FALSE, 0.02 in, MIN(Width, Height)*0.48)";
                let bow_expr = format!("=-0.41421356 * ({r_expr})");
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, format!("={r_expr}"), "0 in"),
                    GeometryRow::line_to(2, format!("=Width - ({r_expr})"), "0 in"),
                    GeometryRow::arc_to(3, "Width", format!("={r_expr}"), &bow_expr),
                    GeometryRow::line_to(4, "Width", format!("=Height - ({r_expr})")),
                    GeometryRow::arc_to(5, format!("=Width - ({r_expr})"), "Height", &bow_expr),
                    GeometryRow::line_to(6, format!("={r_expr}"), "Height"),
                    GeometryRow::arc_to(7, "0 in", format!("=Height - ({r_expr})"), &bow_expr),
                    GeometryRow::line_to(8, "0 in", format!("={r_expr}")),
                    GeometryRow::arc_to(9, format!("={r_expr}"), "0 in", &bow_expr),
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

    /// Move a 1D connector or line endpoint (`Begin` or `End`), optionally gluing dynamically to `target_shape_id`.
    pub fn move_connector_endpoint(
        &mut self,
        page_id: u32,
        shape_id: u64,
        is_begin: bool,
        x_in: f64,
        y_in: f64,
        target_shape_id: Option<u64>,
    ) -> Result<bool, CoreError> {
        self.move_connector_endpoint_with_port(
            page_id,
            shape_id,
            is_begin,
            x_in,
            y_in,
            target_shape_id,
            None,
        )
    }

    /// Move a 1D connector endpoint (`Begin` or `End`), supporting both static port glue (`Connections.X{ix}`)
    /// when `target_port_cell` is provided and dynamic shape walking glue (`PinX`) when `None`.
    #[allow(clippy::too_many_arguments)]
    pub fn move_connector_endpoint_with_port(
        &mut self,
        page_id: u32,
        shape_id: u64,
        is_begin: bool,
        x_in: f64,
        y_in: f64,
        target_shape_id: Option<u64>,
        target_port_cell: Option<&str>,
    ) -> Result<bool, CoreError> {
        let mut ok = false;
        let port_owned = target_port_cell
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("PinX"))
            .map(str::to_string);
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
                    sh.sheet
                        .set_formula("User.Preserve1DGeometry", "FALSE", Some(Unit::Boolean));
                    ok = true;
                }
                if ok {
                    page.connects.retain(|c| {
                        !(c.connector_shape_id == shape_id
                            && c.from_cell.eq_ignore_ascii_case(from_cell))
                    });
                    if let Some(tid) = target_shape_id.filter(|&id| id > 0 && id != shape_id) {
                        if let Some(ref pcell) = port_owned {
                            page.connects.push(GlueEndpoint {
                                connector_shape_id: shape_id,
                                from_cell: from_cell.into(),
                                target_shape_id: tid,
                                to_cell: pcell.clone(),
                                is_dynamic: false,
                            });
                        } else {
                            page.connects.push(GlueEndpoint {
                                connector_shape_id: shape_id,
                                from_cell: from_cell.into(),
                                target_shape_id: tid,
                                to_cell: "PinX".into(),
                                is_dynamic: true,
                            });
                        }
                    }
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(ok)
    }

    /// Slide an interior orthogonal segment (`seg_index`, 0-based along routed polyline) of a 1D connector
    /// to `coord_in` (X in inches for a vertical segment, Y in inches for a horizontal segment).
    pub fn drag_connector_segment(
        &mut self,
        page_id: u32,
        shape_id: u64,
        seg_index: usize,
        coord_in: f64,
    ) -> Result<bool, CoreError> {
        if !coord_in.is_finite() {
            return Ok(false);
        }
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let Some(vis) = visuals.iter().find(|v| v.shape_id == shape_id) else {
            return Ok(false);
        };
        if vis.kind != ShapeKind::Connector1D {
            return Ok(false);
        }
        let poly = vexio_render::tessellate::extract_connector_polyline_in(vis);
        let axis = if poly.len() >= 2 {
            let idx = if seg_index + 1 < poly.len() {
                seg_index
            } else {
                poly.len() / 2
            };
            let idx = idx.min(poly.len() - 2);
            let p0 = poly[idx];
            let p1 = poly[idx + 1];
            if (p0[0] - p1[0]).abs() <= (p0[1] - p1[1]).abs() {
                1 // Vertical segment -> slide X coordinate
            } else {
                2 // Horizontal segment -> slide Y coordinate
            }
        } else {
            1
        };

        let mut ok = false;
        self.journal
            .record_mutation(&mut self.doc, "Slide Connector Segment", |doc| {
                if let Some(page) = doc.page_mut(page_id) {
                    if let Some(sh) = page.find_shape_mut(shape_id) {
                        if sh.kind == ShapeKind::Connector1D {
                            sh.sheet
                                .set_formula("ShapeRouteStyle", "16", Some(Unit::Number));
                            sh.sheet.set_formula(
                                "User.Preserve1DGeometry",
                                "FALSE",
                                Some(Unit::Boolean),
                            );
                            sh.sheet.set_formula(
                                "User.MidAxis",
                                format!("{axis}"),
                                Some(Unit::Number),
                            );
                            sh.sheet.set_formula(
                                "User.MidCoord",
                                format!("{} in", format_clean_f64(coord_in)),
                                Some(Unit::Inch),
                            );
                            ok = true;
                        }
                    }
                }
                if ok {
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(ok)
    }

    /// Translate one or more selected shapes by `(dx_in, dy_in)` in a single undoable transaction,
    /// moving enclosed Container/Swimlane members synchronously, reconciling drop-into/out-of
    /// Container membership & Swimlane metadata, and re-routing attached connectors.
    pub fn move_shapes_delta(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        dx_in: f64,
        dy_in: f64,
    ) -> Result<usize, CoreError> {
        if shape_ids.is_empty() || (dx_in.abs() < 1e-7 && dy_in.abs() < 1e-7) {
            return Ok(0);
        }
        let expanded_ids = crate::containers::expand_move_set_with_container_and_swimlane_members(
            &self.doc,
            page_id,
            shape_ids,
        );
        let id_set: std::collections::BTreeSet<u64> = expanded_ids.into_iter().collect();
        let root_ids: Vec<u64> = shape_ids.to_vec();
        let mut moved = 0usize;
        self.journal
            .record_mutation(&mut self.doc, "Move Shapes", |doc| {
                {
                    let Some(page) = doc.page_mut(page_id) else {
                        return;
                    };
                    let connects_snap = page.connects.clone();
                    for sh in &mut page.shapes {
                        if !id_set.contains(&sh.id) {
                            continue;
                        }
                        let lock_x = sh.sheet.get_bool("LockMoveX").unwrap_or(false);
                        let lock_y = sh.sheet.get_bool("LockMoveY").unwrap_or(false);
                        let eff_dx = if lock_x { 0.0 } else { dx_in };
                        let eff_dy = if lock_y { 0.0 } else { dy_in };
                        if eff_dx.abs() < 1e-7 && eff_dy.abs() < 1e-7 {
                            continue;
                        }
                        if sh.kind == ShapeKind::Connector1D {
                            let beg_glued = connects_snap.iter().any(|c| {
                                c.connector_shape_id == sh.id
                                    && c.from_cell.to_ascii_lowercase().starts_with("begin")
                            });
                            let end_glued = connects_snap.iter().any(|c| {
                                c.connector_shape_id == sh.id
                                    && !c.from_cell.to_ascii_lowercase().starts_with("begin")
                            });
                            if !beg_glued {
                                let bx = sh.sheet.get_f64("BeginX").unwrap_or(0.0) + eff_dx;
                                let by = sh.sheet.get_f64("BeginY").unwrap_or(0.0) + eff_dy;
                                let _ = sh.sheet.try_set_formula_respecting_guard(
                                    "BeginX",
                                    format!("{} in", format_clean_f64(bx)),
                                    false,
                                );
                                let _ = sh.sheet.try_set_formula_respecting_guard(
                                    "BeginY",
                                    format!("{} in", format_clean_f64(by)),
                                    false,
                                );
                            }
                            if !end_glued {
                                let ex = sh.sheet.get_f64("EndX").unwrap_or(1.0) + eff_dx;
                                let ey = sh.sheet.get_f64("EndY").unwrap_or(0.0) + eff_dy;
                                let _ = sh.sheet.try_set_formula_respecting_guard(
                                    "EndX",
                                    format!("{} in", format_clean_f64(ex)),
                                    false,
                                );
                                let _ = sh.sheet.try_set_formula_respecting_guard(
                                    "EndY",
                                    format!("{} in", format_clean_f64(ey)),
                                    false,
                                );
                            }
                            if let Some(mc) = sh.sheet.get_f64("User.MidCoord") {
                                let axis = sh
                                    .sheet
                                    .get_f64("User.MidAxis")
                                    .map(|v| v.round() as i32)
                                    .unwrap_or(0);
                                let delta = if axis == 1 {
                                    eff_dx
                                } else if axis == 2 {
                                    eff_dy
                                } else {
                                    0.0
                                };
                                if delta.abs() > 1e-7 {
                                    sh.sheet.set_formula(
                                        "User.MidCoord",
                                        format!("{} in", format_clean_f64(mc + delta)),
                                        Some(Unit::Inch),
                                    );
                                }
                            }
                            moved += 1;
                        } else {
                            let px = sh.sheet.get_f64("PinX").unwrap_or(0.0) + eff_dx;
                            let py = sh.sheet.get_f64("PinY").unwrap_or(0.0) + eff_dy;
                            let mut changed = false;
                            if eff_dx.abs() > 1e-7
                                && sh
                                    .sheet
                                    .try_set_formula_respecting_guard(
                                        "PinX",
                                        format!("{} in", format_clean_f64(px)),
                                        false,
                                    )
                                    .is_ok()
                            {
                                changed = true;
                            }
                            if eff_dy.abs() > 1e-7
                                && sh
                                    .sheet
                                    .try_set_formula_respecting_guard(
                                        "PinY",
                                        format!("{} in", format_clean_f64(py)),
                                        false,
                                    )
                                    .is_ok()
                            {
                                changed = true;
                            }
                            if changed {
                                moved += 1;
                            }
                        }
                    }
                }
                if moved > 0 {
                    ReactiveEngine::recalculate_document(doc);
                    crate::containers::reconcile_containers_and_swimlanes_after_move(
                        doc,
                        page_id,
                        &root_ids,
                    );
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(moved)
    }

    /// Align selected 2D/Group shapes (`left`, `center`, `right`, `top`, `middle`, `bottom`).
    /// When 2+ shapes are selected, aligns to their union bounding box; when 1 shape is selected, aligns to the page.
    pub fn align_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        mode: &str,
    ) -> Result<usize, CoreError> {
        let Some(page_ref) = self.doc.page(page_id) else {
            return Ok(0);
        };
        let page_w = page_ref.width_in();
        let page_h = page_ref.height_in();
        let id_set: std::collections::BTreeSet<u64> = shape_ids.iter().copied().collect();
        if id_set.is_empty() {
            return Ok(0);
        }
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let selected_vis: Vec<&vexio_shapesheet::EvaluatedShapeVisual> = visuals
            .iter()
            .filter(|v| id_set.contains(&v.shape_id) && v.kind != ShapeKind::Connector1D)
            .collect();
        if selected_vis.is_empty() {
            return Ok(0);
        }

        let (ref_min_x, ref_min_y, ref_max_x, ref_max_y) = if selected_vis.len() == 1 {
            (0.0, 0.0, page_w, page_h)
        } else {
            let mut mnx = f64::INFINITY;
            let mut mny = f64::INFINITY;
            let mut mxx = f64::NEG_INFINITY;
            let mut mxy = f64::NEG_INFINITY;
            for v in &selected_vis {
                let (bmn, bmx) = v.transform.world_aabb();
                mnx = mnx.min(bmn[0]);
                mny = mny.min(bmn[1]);
                mxx = mxx.max(bmx[0]);
                mxy = mxy.max(bmx[1]);
            }
            (mnx, mny, mxx, mxy)
        };

        let m = mode.trim().to_ascii_lowercase();
        let mut deltas: Vec<(u64, f64, f64)> = Vec::with_capacity(selected_vis.len());
        for v in &selected_vis {
            let (bmn, bmx) = v.transform.world_aabb();
            let (dx, dy) = match m.as_str() {
                "left" | "l" => (ref_min_x - bmn[0], 0.0),
                "center" | "centre" | "c" | "hcenter" => (
                    (ref_min_x + ref_max_x) * 0.5 - (bmn[0] + bmx[0]) * 0.5,
                    0.0,
                ),
                "right" | "r" => (ref_max_x - bmx[0], 0.0),
                "bottom" | "b" => (0.0, ref_min_y - bmn[1]),
                "middle" | "mid" | "m" | "vcenter" => (
                    0.0,
                    (ref_min_y + ref_max_y) * 0.5 - (bmn[1] + bmx[1]) * 0.5,
                ),
                "top" | "t" => (0.0, ref_max_y - bmx[1]),
                _ => return Ok(0),
            };
            if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
                deltas.push((v.shape_id, dx, dy));
            }
        }
        if deltas.is_empty() {
            return Ok(selected_vis.len());
        }
        let stationary_aligned = selected_vis.len().saturating_sub(deltas.len());

        let mut updated = 0usize;
        self.journal
            .record_mutation(&mut self.doc, "Align Shapes", |doc| {
                if let Some(page) = doc.page_mut(page_id) {
                    for &(sid, dx, dy) in &deltas {
                        if let Some(sh) = page.find_shape_mut(sid) {
                            let lock_x = sh.sheet.get_bool("LockMoveX").unwrap_or(false);
                            let lock_y = sh.sheet.get_bool("LockMoveY").unwrap_or(false);
                            let mut changed = false;
                            if dx.abs() > 1e-6 && !lock_x {
                                let px = sh.sheet.get_f64("PinX").unwrap_or(0.0) + dx;
                                if sh
                                    .sheet
                                    .try_set_formula_respecting_guard(
                                        "PinX",
                                        format!("{} in", format_clean_f64(px)),
                                        false,
                                    )
                                    .is_ok()
                                {
                                    changed = true;
                                }
                            }
                            if dy.abs() > 1e-6 && !lock_y {
                                let py = sh.sheet.get_f64("PinY").unwrap_or(0.0) + dy;
                                if sh
                                    .sheet
                                    .try_set_formula_respecting_guard(
                                        "PinY",
                                        format!("{} in", format_clean_f64(py)),
                                        false,
                                    )
                                    .is_ok()
                                {
                                    changed = true;
                                }
                            }
                            if changed {
                                updated += 1;
                            }
                        }
                    }
                }
                if updated > 0 {
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(if updated > 0 {
            stationary_aligned + updated
        } else {
            0
        })
    }

    /// Distribute 3+ selected 2D/Group shapes with equal bounding-box gap spacing along `horizontal` or `vertical` axis.
    pub fn distribute_shapes(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        axis: &str,
    ) -> Result<usize, CoreError> {
        let id_set: std::collections::BTreeSet<u64> = shape_ids.iter().copied().collect();
        if id_set.len() < 3 {
            return Ok(0);
        }
        let is_horiz = matches!(
            axis.trim().to_ascii_lowercase().as_str(),
            "horizontal" | "horiz" | "h" | "x"
        );
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let mut items: Vec<(u64, f64, f64)> = visuals
            .iter()
            .filter(|v| id_set.contains(&v.shape_id) && v.kind != ShapeKind::Connector1D)
            .map(|v| {
                let (bmn, bmx) = v.transform.world_aabb();
                if is_horiz {
                    (v.shape_id, bmn[0], bmx[0])
                } else {
                    (v.shape_id, bmn[1], bmx[1])
                }
            })
            .collect();
        if items.len() < 3 {
            return Ok(0);
        }

        items.sort_by(|a, b| {
            let ca = (a.1 + a.2) * 0.5;
            let cb = (b.1 + b.2) * 0.5;
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });

        let outer_min = items.iter().map(|i| i.1).fold(f64::INFINITY, f64::min);
        let outer_max = items.iter().map(|i| i.2).fold(f64::NEG_INFINITY, f64::max);
        let sum_spans: f64 = items.iter().map(|i| (i.2 - i.1).max(0.0)).sum();
        let gap = (outer_max - outer_min - sum_spans) / ((items.len() - 1) as f64);

        let mut cursor = outer_min;
        let mut shifts: Vec<(u64, f64)> = Vec::with_capacity(items.len());
        for (idx, &(sid, lo, hi)) in items.iter().enumerate() {
            let span = (hi - lo).max(0.0);
            if idx > 0 && idx + 1 < items.len() {
                let delta = cursor - lo;
                if delta.abs() > 1e-6 {
                    shifts.push((sid, delta));
                }
            }
            cursor += span + gap;
        }
        if shifts.is_empty() {
            return Ok(items.len());
        }
        let stationary_distributed = items.len().saturating_sub(shifts.len());

        let mut updated = 0usize;
        self.journal
            .record_mutation(&mut self.doc, "Distribute Shapes", |doc| {
                if let Some(page) = doc.page_mut(page_id) {
                    for &(sid, delta) in &shifts {
                        if let Some(sh) = page.find_shape_mut(sid) {
                            if is_horiz {
                                if !sh.sheet.get_bool("LockMoveX").unwrap_or(false) {
                                    let px = sh.sheet.get_f64("PinX").unwrap_or(0.0) + delta;
                                    if sh
                                        .sheet
                                        .try_set_formula_respecting_guard(
                                            "PinX",
                                            format!("{} in", format_clean_f64(px)),
                                            false,
                                        )
                                        .is_ok()
                                    {
                                        updated += 1;
                                    }
                                }
                            } else if !sh.sheet.get_bool("LockMoveY").unwrap_or(false) {
                                let py = sh.sheet.get_f64("PinY").unwrap_or(0.0) + delta;
                                if sh
                                    .sheet
                                    .try_set_formula_respecting_guard(
                                        "PinY",
                                        format!("{} in", format_clean_f64(py)),
                                        false,
                                    )
                                    .is_ok()
                                {
                                    updated += 1;
                                }
                            }
                        }
                    }
                }
                if updated > 0 {
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(if updated > 0 {
            stationary_distributed + updated
        } else {
            0
        })
    }

    /// Group 2+ top-level shapes on `page_id` into a new `ShapeKind::Group` container (`⌘G` / `Ctrl+G`),
    /// converting child coordinates into group-local space and preserving undo/redo & connectors.
    pub fn group_shapes(&mut self, page_id: u32, shape_ids: &[u64]) -> Result<u64, CoreError> {
        let id_set: std::collections::BTreeSet<u64> = shape_ids.iter().copied().collect();
        if id_set.len() < 2 {
            return Ok(0);
        }
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let mut g_min_x = f64::INFINITY;
        let mut g_min_y = f64::INFINITY;
        let mut g_max_x = f64::NEG_INFINITY;
        let mut g_max_y = f64::NEG_INFINITY;
        let mut matched_count = 0usize;
        for v in &visuals {
            if id_set.contains(&v.shape_id) {
                let (bmn, bmx) = if v.kind == ShapeKind::Connector1D {
                    (v.aabb_min, v.aabb_max)
                } else {
                    v.transform.world_aabb()
                };
                g_min_x = g_min_x.min(bmn[0]);
                g_min_y = g_min_y.min(bmn[1]);
                g_max_x = g_max_x.max(bmx[0]);
                g_max_y = g_max_y.max(bmx[1]);
                matched_count += 1;
            }
        }
        if matched_count < 2 || !g_min_x.is_finite() || !g_max_x.is_finite() {
            return Ok(0);
        }

        let gw = (g_max_x - g_min_x).max(0.1);
        let gh = (g_max_y - g_min_y).max(0.1);
        let g_cx = (g_min_x + g_max_x) * 0.5;
        let g_cy = (g_min_y + g_max_y) * 0.5;

        let mut created_gid = 0u64;
        self.journal
            .record_mutation(&mut self.doc, "Group Shapes", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                let gid = page.next_shape_id();
                let mut children = Vec::new();
                let mut insert_pos: Option<usize> = None;
                let mut remaining = Vec::with_capacity(page.shapes.len());

                for (idx, mut sh) in page.shapes.drain(..).enumerate() {
                    if id_set.contains(&sh.id) {
                        if insert_pos.is_none() {
                            insert_pos = Some(remaining.len().max(idx.min(remaining.len())));
                        }
                        if sh.kind == ShapeKind::Connector1D {
                            let bx = sh.sheet.get_f64("BeginX").unwrap_or(g_min_x) - g_min_x;
                            let by = sh.sheet.get_f64("BeginY").unwrap_or(g_min_y) - g_min_y;
                            let ex = sh.sheet.get_f64("EndX").unwrap_or(g_min_x + 1.0) - g_min_x;
                            let ey = sh.sheet.get_f64("EndY").unwrap_or(g_min_y) - g_min_y;
                            sh.sheet.set_formula(
                                "BeginX",
                                format!("{} in", format_clean_f64(bx)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "BeginY",
                                format!("{} in", format_clean_f64(by)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "EndX",
                                format!("{} in", format_clean_f64(ex)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "EndY",
                                format!("{} in", format_clean_f64(ey)),
                                Some(Unit::Inch),
                            );
                            let cpx = sh.sheet.get_f64("PinX").unwrap_or(g_min_x) - g_min_x;
                            let cpy = sh.sheet.get_f64("PinY").unwrap_or(g_min_y) - g_min_y;
                            sh.sheet.set_formula(
                                "PinX",
                                format!("{} in", format_clean_f64(cpx)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "PinY",
                                format!("{} in", format_clean_f64(cpy)),
                                Some(Unit::Inch),
                            );
                        } else {
                            let px = sh.sheet.get_f64("PinX").unwrap_or(g_cx) - g_min_x;
                            let py = sh.sheet.get_f64("PinY").unwrap_or(g_cy) - g_min_y;
                            sh.sheet.set_formula(
                                "PinX",
                                format!("{} in", format_clean_f64(px)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "PinY",
                                format!("{} in", format_clean_f64(py)),
                                Some(Unit::Inch),
                            );
                        }
                        children.push(sh);
                    } else {
                        remaining.push(sh);
                    }
                }

                if children.len() < 2 {
                    remaining.extend(children);
                    page.shapes = remaining;
                    return;
                }

                let mut grp = VisioShape::new_rect(gid, format!("Group.{gid}"), g_cx, g_cy, gw, gh);
                grp.kind = ShapeKind::Group;
                grp.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
                grp.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
                grp.sheet
                    .set_formula("User.OrigWidth", format!("{} in", format_clean_f64(gw)), Some(Unit::Inch));
                grp.sheet
                    .set_formula("User.OrigHeight", format!("{} in", format_clean_f64(gh)), Some(Unit::Inch));
                let mut g_sec = GeometrySection::rectangle(1);
                g_sec.no_fill = "TRUE".into();
                g_sec.no_line = "TRUE".into();
                g_sec.no_show = "TRUE".into();
                grp.sheet.add_geometry(g_sec);
                attach_cardinal_ports_to_shape(&mut grp);
                grp.children = children;

                // Re-target external dynamic connectors pointing at grouped member shapes to the group container
                for c in &mut page.connects {
                    if !id_set.contains(&c.connector_shape_id) && id_set.contains(&c.target_shape_id)
                    {
                        c.target_shape_id = gid;
                        c.to_cell = "PinX".into();
                        c.is_dynamic = true;
                    }
                }

                let pos = insert_pos.unwrap_or(remaining.len()).min(remaining.len());
                remaining.insert(pos, grp);
                page.shapes = remaining;
                created_gid = gid;
                route_page_connectors(doc, page_id);
            });
        Ok(created_gid)
    }

    /// Ungroup one or more `ShapeKind::Group` shapes (`⇧⌘G` / `Ctrl+Shift+G`), restoring member
    /// shapes back to page-space coordinates at the group's Z-order position.
    pub fn ungroup_shapes(
        &mut self,
        page_id: u32,
        group_ids: &[u64],
    ) -> Result<Vec<u64>, CoreError> {
        let id_set: std::collections::BTreeSet<u64> = group_ids.iter().copied().collect();
        if id_set.is_empty() {
            return Ok(Vec::new());
        }
        let mut restored_ids = Vec::new();
        self.journal
            .record_mutation(&mut self.doc, "Ungroup Shapes", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };
                let mut used_ids: std::collections::BTreeSet<u64> = page
                    .shapes
                    .iter()
                    .filter(|e| !id_set.contains(&e.id))
                    .map(|e| e.id)
                    .collect();
                let mut out_shapes = Vec::with_capacity(page.shapes.len() + 4);
                let mut any_ungrouped = false;

                for sh in page.shapes.drain(..) {
                    if id_set.contains(&sh.id)
                        && (sh.kind == ShapeKind::Group || !sh.children.is_empty())
                        && !sh.children.is_empty()
                    {
                        any_ungrouped = true;
                        let gw = sh.sheet.get_f64("Width").unwrap_or(1.0);
                        let gh = sh.sheet.get_f64("Height").unwrap_or(1.0);
                        let orig_gw = sh.sheet.get_f64("User.OrigWidth").unwrap_or(gw).max(1e-4);
                        let orig_gh = sh.sheet.get_f64("User.OrigHeight").unwrap_or(gh).max(1e-4);
                        let sx = (gw / orig_gw).clamp(0.05, 50.0);
                        let sy = (gh / orig_gh).clamp(0.05, 50.0);
                        let gpx = sh.sheet.get_f64("PinX").unwrap_or(0.0);
                        let gpy = sh.sheet.get_f64("PinY").unwrap_or(0.0);
                        let glpx = sh.sheet.get_f64("LocPinX").unwrap_or(gw * 0.5);
                        let glpy = sh.sheet.get_f64("LocPinY").unwrap_or(gh * 0.5);
                        let gang = sh.sheet.get_f64("Angle").unwrap_or(0.0);
                        let g_tf = vexio_shapesheet::ShapeTransform2D {
                            pin_x: gpx,
                            pin_y: gpy,
                            width: gw,
                            height: gh,
                            loc_pin_x: glpx,
                            loc_pin_y: glpy,
                            angle_rad: gang,
                            flip_x: sh.sheet.get_bool("FlipX").unwrap_or(false),
                            flip_y: sh.sheet.get_bool("FlipY").unwrap_or(false),
                        };

                        let first_child_id = sh.children.first().map(|c| c.id);
                        for mut child in sh.children {
                            let cid = if used_ids.contains(&child.id) {
                                let nid = used_ids.iter().copied().max().unwrap_or(sh.id) + 1;
                                child.id = nid;
                                nid
                            } else {
                                child.id
                            };
                            used_ids.insert(cid);

                            if child.kind == ShapeKind::Connector1D {
                                let cbx = child.sheet.get_f64("BeginX").unwrap_or(0.0) * sx;
                                let cby = child.sheet.get_f64("BeginY").unwrap_or(0.0) * sy;
                                let cex = child.sheet.get_f64("EndX").unwrap_or(1.0) * sx;
                                let cey = child.sheet.get_f64("EndY").unwrap_or(0.0) * sy;
                                let (wbx, wby) = g_tf.local_to_parent(cbx, cby);
                                let (wex, wey) = g_tf.local_to_parent(cex, cey);
                                child.sheet.set_formula(
                                    "BeginX",
                                    format!("{} in", format_clean_f64(wbx)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "BeginY",
                                    format!("{} in", format_clean_f64(wby)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "EndX",
                                    format!("{} in", format_clean_f64(wex)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "EndY",
                                    format!("{} in", format_clean_f64(wey)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "PinX",
                                    format!("{} in", format_clean_f64(wbx)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "PinY",
                                    format!("{} in", format_clean_f64(wby)),
                                    Some(Unit::Inch),
                                );
                            } else {
                                let cpx = child.sheet.get_f64("PinX").unwrap_or(0.0) * sx;
                                let cpy = child.sheet.get_f64("PinY").unwrap_or(0.0) * sy;
                                let (wpx, wpy) = g_tf.local_to_parent(cpx, cpy);
                                child.sheet.set_formula(
                                    "PinX",
                                    format!("{} in", format_clean_f64(wpx)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "PinY",
                                    format!("{} in", format_clean_f64(wpy)),
                                    Some(Unit::Inch),
                                );
                                if (sx - 1.0).abs() > 1e-4 {
                                    if let Some(cw) = child.sheet.get_f64("Width") {
                                        let new_w = cw * sx;
                                        child.sheet.set_formula(
                                            "Width",
                                            format!("{} in", format_clean_f64(new_w)),
                                            Some(Unit::Inch),
                                        );
                                        child.sheet.set_formula(
                                            "LocPinX",
                                            format!("{} in", format_clean_f64(new_w * 0.5)),
                                            Some(Unit::Inch),
                                        );
                                    }
                                }
                                if (sy - 1.0).abs() > 1e-4 {
                                    if let Some(ch_h) = child.sheet.get_f64("Height") {
                                        let new_h = ch_h * sy;
                                        child.sheet.set_formula(
                                            "Height",
                                            format!("{} in", format_clean_f64(new_h)),
                                            Some(Unit::Inch),
                                        );
                                        child.sheet.set_formula(
                                            "LocPinY",
                                            format!("{} in", format_clean_f64(new_h * 0.5)),
                                            Some(Unit::Inch),
                                        );
                                    }
                                }
                                if gang.abs() > 1e-6 {
                                    let cang = child.sheet.get_f64("Angle").unwrap_or(0.0) + gang;
                                    child.sheet.set_formula(
                                        "Angle",
                                        format!("{} rad", format_clean_f64(cang)),
                                        Some(Unit::Radian),
                                    );
                                }
                            }
                            restored_ids.push(cid);
                            out_shapes.push(child);
                        }

                        if let Some(fcid) = first_child_id {
                            for c in &mut page.connects {
                                if c.target_shape_id == sh.id {
                                    c.target_shape_id = fcid;
                                }
                            }
                        }
                    } else {
                        out_shapes.push(sh);
                    }
                }

                page.shapes = out_shapes;
                if any_ungrouped {
                    route_page_connectors(doc, page_id);
                }
            });
        Ok(restored_ids)
    }

    /// Compute Visio Dynamic Grid Smart Alignment & Equal-Spacing Guides for a candidate moving/resizing
    /// bounding box `cand_min..cand_max` (excluding `moving_ids`).
    /// Returns `([snap_dx_in, snap_dy_in], guide_lines)`.
    pub fn compute_smart_guides(
        &self,
        page_id: u32,
        moving_ids: &[u64],
        cand_min: [f64; 2],
        cand_max: [f64; 2],
        snap_tol_in: f64,
    ) -> ([f64; 2], Vec<vexio_render::SmartGuideLine>) {
        let tol = snap_tol_in.clamp(0.005, 0.5);
        let Some(page) = self.doc.page(page_id) else {
            return ([0.0, 0.0], Vec::new());
        };
        let page_w = page.width_in();
        let page_h = page.height_in();
        let moving_set: std::collections::BTreeSet<u64> = moving_ids.iter().copied().collect();
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);

        let others: Vec<([f64; 2], [f64; 2])> = visuals
            .iter()
            .filter(|v| !moving_set.contains(&v.shape_id) && v.kind != ShapeKind::Connector1D)
            .map(|v| v.transform.world_aabb())
            .collect();

        let cand_w = (cand_max[0] - cand_min[0]).max(0.0);
        let cand_h = (cand_max[1] - cand_min[1]).max(0.0);
        let cand_cx = (cand_min[0] + cand_max[0]) * 0.5;
        let cand_cy = (cand_min[1] + cand_max[1]) * 0.5;

        let mut best_dx: Option<(f64, f64)> = None; // (weighted_score, signed_dx)
        let mut best_dy: Option<(f64, f64)> = None; // (weighted_score, signed_dy)

        let mut consider_dx = |dx: f64, weight: f64| {
            let ad = dx.abs();
            let score = ad * weight;
            if ad <= tol && best_dx.is_none_or(|(best_sc, _)| score < best_sc - 1e-7) {
                best_dx = Some((score, dx));
            }
        };
        let mut consider_dy = |dy: f64, weight: f64| {
            let ad = dy.abs();
            let score = ad * weight;
            if ad <= tol && best_dy.is_none_or(|(best_sc, _)| score < best_sc - 1e-7) {
                best_dy = Some((score, dy));
            }
        };

        // 1. Page Center X & Y
        consider_dx(page_w * 0.5 - cand_cx, 0.5);
        consider_dy(page_h * 0.5 - cand_cy, 0.5);

        // 2. Stationary Shape Edges & Centers (center-to-center weighted 0.35 per Visio Dynamic Grid)
        for &(omn, omx) in &others {
            let ocx = (omn[0] + omx[0]) * 0.5;
            let ocy = (omn[1] + omx[1]) * 0.5;
            consider_dx(ocx - cand_cx, 0.35);
            consider_dy(ocy - cand_cy, 0.35);

            for &ox in &[omn[0], ocx, omx[0]] {
                for &cx in &[cand_min[0], cand_cx, cand_max[0]] {
                    consider_dx(ox - cx, 1.0);
                }
            }
            for &oy in &[omn[1], ocy, omx[1]] {
                for &cy in &[cand_min[1], cand_cy, cand_max[1]] {
                    consider_dy(oy - cy, 1.0);
                }
            }
        }

        // 3. Equal-Spacing Gaps across 3+ Shapes (Horizontal & Vertical)
        let mut row_peers: Vec<([f64; 2], [f64; 2])> = others
            .iter()
            .copied()
            .filter(|&(omn, omx)| {
                let oy_mid = (omn[1] + omx[1]) * 0.5;
                (oy_mid - cand_cy).abs() <= (cand_h + (omx[1] - omn[1])).max(1.5)
            })
            .collect();
        row_peers.sort_by(|a, b| {
            a.0[0]
                .partial_cmp(&b.0[0])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for i in 0..row_peers.len() {
            for j in (i + 1)..row_peers.len() {
                let (amn, amx) = row_peers[i];
                let (bmn, bmx) = row_peers[j];
                let gap = bmn[0] - amx[0];
                if gap > 0.05 {
                    consider_dx((bmx[0] + gap) - cand_min[0], 0.6);
                    consider_dx((amn[0] - gap) - cand_max[0], 0.6);
                }
                if bmn[0] - amx[0] > cand_w + 0.1 {
                    consider_dx((amx[0] + bmn[0]) * 0.5 - cand_cx, 0.6);
                }
            }
        }

        let mut col_peers: Vec<([f64; 2], [f64; 2])> = others
            .iter()
            .copied()
            .filter(|&(omn, omx)| {
                let ox_mid = (omn[0] + omx[0]) * 0.5;
                (ox_mid - cand_cx).abs() <= (cand_w + (omx[0] - omn[0])).max(1.5)
            })
            .collect();
        col_peers.sort_by(|a, b| {
            a.0[1]
                .partial_cmp(&b.0[1])
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        for i in 0..col_peers.len() {
            for j in (i + 1)..col_peers.len() {
                let (amn, amx) = col_peers[i];
                let (bmn, bmx) = col_peers[j];
                let gap = bmn[1] - amx[1];
                if gap > 0.05 {
                    consider_dy((bmx[1] + gap) - cand_min[1], 0.6);
                    consider_dy((amn[1] - gap) - cand_max[1], 0.6);
                }
                if bmn[1] - amx[1] > cand_h + 0.1 {
                    consider_dy((amx[1] + bmn[1]) * 0.5 - cand_cy, 0.6);
                }
            }
        }

        let snap_dx = best_dx.map(|(_, d)| d).unwrap_or(0.0);
        let snap_dy = best_dy.map(|(_, d)| d).unwrap_or(0.0);
        let s_min = [cand_min[0] + snap_dx, cand_min[1] + snap_dy];
        let s_max = [cand_max[0] + snap_dx, cand_max[1] + snap_dy];
        let s_cx = (s_min[0] + s_max[0]) * 0.5;
        let s_cy = (s_min[1] + s_max[1]) * 0.5;

        let mut guides = Vec::new();
        let eps = 1e-4;

        if best_dx.is_some() {
            if (s_cx - page_w * 0.5).abs() <= eps {
                guides.push(vexio_render::SmartGuideLine {
                    is_vertical: true,
                    coord_in: page_w * 0.5,
                    span_min_in: 0.0,
                    span_max_in: page_h,
                    is_equal_spacing: false,
                });
            }
            for &(omn, omx) in &others {
                let oxs = [omn[0], (omn[0] + omx[0]) * 0.5, omx[0]];
                for &ox in &oxs {
                    if [s_min[0], s_cx, s_max[0]]
                        .iter()
                        .any(|&sx| (sx - ox).abs() <= eps)
                    {
                        let y_lo = s_min[1].min(omn[1]) - 0.2;
                        let y_hi = s_max[1].max(omx[1]) + 0.2;
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: ox,
                            span_min_in: y_lo,
                            span_max_in: y_hi,
                            is_equal_spacing: false,
                        });
                    }
                }
            }
            for i in 0..row_peers.len() {
                for j in (i + 1)..row_peers.len() {
                    let (amn, amx) = row_peers[i];
                    let (bmn, bmx) = row_peers[j];
                    let gap_ab = bmn[0] - amx[0];
                    let mid_y = s_cy;
                    if gap_ab > 0.05 && ((s_min[0] - bmx[0]) - gap_ab).abs() <= eps {
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: mid_y,
                            span_min_in: amx[0],
                            span_max_in: bmn[0],
                            is_equal_spacing: true,
                        });
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: mid_y,
                            span_min_in: bmx[0],
                            span_max_in: s_min[0],
                            is_equal_spacing: true,
                        });
                    } else if gap_ab > 0.05 && ((amn[0] - s_max[0]) - gap_ab).abs() <= eps {
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: mid_y,
                            span_min_in: s_max[0],
                            span_max_in: amn[0],
                            is_equal_spacing: true,
                        });
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: mid_y,
                            span_min_in: amx[0],
                            span_max_in: bmn[0],
                            is_equal_spacing: true,
                        });
                    } else if s_min[0] > amx[0] + 0.02
                        && bmn[0] > s_max[0] + 0.02
                        && ((s_min[0] - amx[0]) - (bmn[0] - s_max[0])).abs() <= eps
                    {
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: mid_y,
                            span_min_in: amx[0],
                            span_max_in: s_min[0],
                            is_equal_spacing: true,
                        });
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: mid_y,
                            span_min_in: s_max[0],
                            span_max_in: bmn[0],
                            is_equal_spacing: true,
                        });
                    }
                }
            }
        }

        if best_dy.is_some() {
            if (s_cy - page_h * 0.5).abs() <= eps {
                guides.push(vexio_render::SmartGuideLine {
                    is_vertical: false,
                    coord_in: page_h * 0.5,
                    span_min_in: 0.0,
                    span_max_in: page_w,
                    is_equal_spacing: false,
                });
            }
            for &(omn, omx) in &others {
                let oys = [omn[1], (omn[1] + omx[1]) * 0.5, omx[1]];
                for &oy in &oys {
                    if [s_min[1], s_cy, s_max[1]]
                        .iter()
                        .any(|&sy| (sy - oy).abs() <= eps)
                    {
                        let x_lo = s_min[0].min(omn[0]) - 0.2;
                        let x_hi = s_max[0].max(omx[0]) + 0.2;
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: false,
                            coord_in: oy,
                            span_min_in: x_lo,
                            span_max_in: x_hi,
                            is_equal_spacing: false,
                        });
                    }
                }
            }
            for i in 0..col_peers.len() {
                for j in (i + 1)..col_peers.len() {
                    let (amn, amx) = col_peers[i];
                    let (bmn, bmx) = col_peers[j];
                    let gap_ab = bmn[1] - amx[1];
                    let mid_x = s_cx;
                    if gap_ab > 0.05 && ((s_min[1] - bmx[1]) - gap_ab).abs() <= eps {
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: mid_x,
                            span_min_in: amx[1],
                            span_max_in: bmn[1],
                            is_equal_spacing: true,
                        });
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: mid_x,
                            span_min_in: bmx[1],
                            span_max_in: s_min[1],
                            is_equal_spacing: true,
                        });
                    } else if gap_ab > 0.05 && ((amn[1] - s_max[1]) - gap_ab).abs() <= eps {
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: mid_x,
                            span_min_in: s_max[1],
                            span_max_in: amn[1],
                            is_equal_spacing: true,
                        });
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: mid_x,
                            span_min_in: amx[1],
                            span_max_in: bmn[1],
                            is_equal_spacing: true,
                        });
                    } else if s_min[1] > amx[1] + 0.02
                        && bmn[1] > s_max[1] + 0.02
                        && ((s_min[1] - amx[1]) - (bmn[1] - s_max[1])).abs() <= eps
                    {
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: mid_x,
                            span_min_in: amx[1],
                            span_max_in: s_min[1],
                            is_equal_spacing: true,
                        });
                        guides.push(vexio_render::SmartGuideLine {
                            is_vertical: true,
                            coord_in: mid_x,
                            span_min_in: s_max[1],
                            span_max_in: bmn[1],
                            is_equal_spacing: true,
                        });
                    }
                }
            }
        }

        ([snap_dx, snap_dy], guides)
    }

    /// Locate a static connection port (`Some("Connections.X{ix}")`) or dynamic shape body (`None`)
    /// near `(x_in, y_in)` on `page_id`, returning `(target_shape_id, optional_port_cell, snap_pos_in, all_shape_ports_in)`.
    pub fn find_nearest_connection_port(
        &self,
        page_id: u32,
        exclude_shape_id: u64,
        x_in: f64,
        y_in: f64,
        tol_in: f64,
    ) -> Option<(u64, Option<String>, [f64; 2], Vec<[f64; 2]>)> {
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let mut best_port: Option<(f64, u64, String, [f64; 2], Vec<[f64; 2]>)> = None;
        let mut hovered_body: Option<(u64, [f64; 2], Vec<[f64; 2]>)> = None;

        for v in visuals.iter().rev() {
            if v.shape_id == exclude_shape_id || v.kind == ShapeKind::Connector1D {
                continue;
            }
            let all_pts: Vec<[f64; 2]> =
                v.connection_points.iter().map(|cp| cp.pos_page_in).collect();
            for cp in &v.connection_points {
                let d = (cp.pos_page_in[0] - x_in).hypot(cp.pos_page_in[1] - y_in);
                if d <= tol_in && best_port.as_ref().is_none_or(|b| d < b.0) {
                    best_port = Some((
                        d,
                        v.shape_id,
                        cp.cell_name.clone(),
                        cp.pos_page_in,
                        all_pts.clone(),
                    ));
                }
            }
            if hovered_body.is_none()
                && x_in >= v.aabb_min[0] - tol_in * 0.4
                && x_in <= v.aabb_max[0] + tol_in * 0.4
                && y_in >= v.aabb_min[1] - tol_in * 0.4
                && y_in <= v.aabb_max[1] + tol_in * 0.4
            {
                hovered_body = Some((
                    v.shape_id,
                    [v.transform.pin_x, v.transform.pin_y],
                    all_pts,
                ));
            }
        }

        if let Some((_, sid, cell, pos, pts)) = best_port {
            Some((sid, Some(cell), pos, pts))
        } else {
            hovered_body.map(|(sid, pos, pts)| (sid, None, pos, pts))
        }
    }

    /// Register custom TrueType/OpenType font bytes (`.ttf`/`.otf`) into the process-wide `vexio-render`
    /// shaper registry (used for ChromeOS Local Font Access API `.blob()` streaming and user fonts).
    pub fn register_font_bytes(bytes: &[u8], family_override: Option<&str>) -> bool {
        vexio_render::register_font_bytes(family_override.unwrap_or(""), bytes.to_vec()) > 0
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
                    push_cell(&mut cmds, "TextBkgnd", "0".to_string());
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

    // =========================================================================
    // v0.3.0-alpha Pillar #3: Containers, Swimlanes, Callouts, Controls & AutoConnect
    // =========================================================================

    /// Insert a Visio Container enclosing `member_ids` (or a standalone interactive Container if `member_ids` is empty).
    pub fn insert_container(
        &mut self,
        page_id: u32,
        title: &str,
        member_ids: &[u64],
    ) -> Result<u64, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let clean_title = if title.trim().is_empty() {
            "Container"
        } else {
            title.trim()
        };
        let filtered: Vec<u64> = if let Some(page) = self.doc.page(page_id) {
            member_ids
                .iter()
                .copied()
                .filter(|&id| {
                    page.find_shape(id).is_some_and(|sh| {
                        sh.kind != ShapeKind::Connector1D
                            && !crate::containers::is_swimlane_shape(sh)
                            && !crate::containers::is_container_shape(sh)
                    })
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut created_id = 0u64;
        self.journal
            .record_mutation(&mut self.doc, "Insert Container", |doc| {
                created_id =
                    crate::containers::create_container(doc, page_id, clean_title, &filtered, 0.35)
                        .unwrap_or(0);
            });
        if created_id == 0 {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        Ok(created_id)
    }

    /// Build a Cross-Functional Swimlane pool (or append a contiguous lane below existing swimlanes).
    pub fn insert_swimlanes(
        &mut self,
        page_id: u32,
        lanes_semicolon: &str,
    ) -> Result<Vec<u64>, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let parsed: Vec<&str> = lanes_semicolon
            .split(';')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let mut created = Vec::new();
        self.journal
            .record_mutation(&mut self.doc, "Insert Cross-Functional Swimlanes", |doc| {
                created = crate::containers::insert_or_append_swimlanes(doc, page_id, &parsed);
            });
        Ok(created)
    }

    /// Attach a reactive Visio Callout annotation tethered to `target_shape_id` via `Controls.Row_1`
    /// (or place a standalone Callout if `target_shape_id == 0`).
    pub fn insert_callout(
        &mut self,
        page_id: u32,
        target_shape_id: u64,
        text: &str,
    ) -> Result<u64, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let valid_target = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(target_shape_id))
            .map(|s| s.id)
            .unwrap_or(0);
        let clean_text = if text.trim().is_empty() {
            "Callout Note"
        } else {
            text.trim()
        };
        let mut created_id = 0u64;
        self.journal
            .record_mutation(&mut self.doc, "Attach Callout", |doc| {
                created_id = crate::containers::attach_callout(
                    doc,
                    page_id,
                    valid_target,
                    clean_text,
                    1.55,
                    1.05,
                )
                .unwrap_or(0);
            });
        if created_id == 0 {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        Ok(created_id)
    }

    /// Drag a yellow diamond `Controls.*` handle in page inches with Undo/Redo journaling.
    /// When dragging `Row_1` of a Callout (`msvStructureType == "Callout"`) onto another 2D shape,
    /// automatically re-tethers the Callout's reactive leader formula (`=Sheet.N!PinX - PinX + LocPinX`)!
    pub fn drag_control_handle(
        &mut self,
        page_id: u32,
        shape_id: u64,
        row_name: &str,
        page_x_in: f64,
        page_y_in: f64,
    ) -> Result<bool, CoreError> {
        if !page_x_in.is_finite() || !page_y_in.is_finite() {
            return Ok(false);
        }
        if self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .is_none()
        {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }

        let clean_row = row_name
            .trim()
            .strip_prefix("Controls.")
            .or_else(|| row_name.trim().strip_prefix("controls."))
            .unwrap_or(row_name.trim())
            .to_string();

        let is_callout_leader = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .is_some_and(|sh| {
                crate::containers::is_callout_shape(sh) && clean_row.eq_ignore_ascii_case("Row_1")
            });

        let retether_target = if is_callout_leader {
            let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
            let index = vexio_render::SpatialIndex::build(&visuals);
            index
                .hit_test_visuals(&visuals, page_x_in, page_y_in, 0.08)
                .into_iter()
                .find(|&cand_id| {
                    cand_id != shape_id
                        && self
                            .doc
                            .page(page_id)
                            .and_then(|p| p.find_shape(cand_id))
                            .is_some_and(|cand| {
                                matches!(cand.kind, ShapeKind::Shape2D | ShapeKind::Group)
                                    && !crate::containers::is_swimlane_shape(cand)
                                    && !crate::containers::is_callout_shape(cand)
                            })
                })
        } else {
            None
        };

        let mut ok = false;
        self.journal
            .record_mutation(&mut self.doc, "Adjust Control Handle", |doc| {
                if let Some(tid) = retether_target {
                    if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id))
                    {
                        let fx = format!("=Sheet.{tid}!PinX - PinX + LocPinX");
                        let fy = format!("=Sheet.{tid}!PinY - PinY + LocPinY");
                        sh.sheet.set_user_cell(
                            "msvCalloutTargetSheet",
                            format!("{tid}"),
                            "Target Shape ID",
                        );
                        sh.sheet
                            .set_formula("Controls.Row_1", fx.clone(), Some(Unit::Inch));
                        sh.sheet
                            .set_formula("Controls.Row_1.X", fx.clone(), Some(Unit::Inch));
                        sh.sheet
                            .set_formula("Controls.Row_1.Y", fy.clone(), Some(Unit::Inch));
                        sh.sheet
                            .set_formula("Controls.Row_1.XDyn", fx, Some(Unit::Inch));
                        sh.sheet
                            .set_formula("Controls.Row_1.YDyn", fy, Some(Unit::Inch));
                        ok = true;
                    }
                    ReactiveEngine::recalculate_document(doc);
                } else {
                    ok = crate::boolean_ops::move_control_handle(
                        doc,
                        page_id,
                        shape_id,
                        &clean_row,
                        page_x_in,
                        page_y_in,
                    );
                }
            });
        Ok(ok)
    }

    /// Visio One-Click AutoConnect (`North`, `East`, `South`, `West`):
    /// - If an existing non-container 2D shape already sits aligned within `[0.15 in .. 2.35 in]`
    ///   in `dir`, wires a dynamic routed connector directly to that neighbor!
    /// - Otherwise, stamps a clean connected step shape `1.15 in` beyond `source_shape_id`'s edge in `dir`,
    ///   wires an orthogonal `Connector1D` port-to-port, reconciles Container/Swimlane membership & bounds,
    ///   and returns `(target_shape_id, connector_shape_id)`.
    pub fn auto_connect_shape(
        &mut self,
        page_id: u32,
        source_shape_id: u64,
        dir: vexio_render::AutoConnectDir,
    ) -> Result<(u64, u64), CoreError> {
        let Some(src_snapshot) = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(source_shape_id))
            .cloned()
        else {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(
                source_shape_id,
            )));
        };

        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let src_vis = visuals.iter().find(|v| v.shape_id == source_shape_id);
        let src_px = src_snapshot.sheet.get_f64("PinX").unwrap_or(3.0);
        let src_py = src_snapshot.sheet.get_f64("PinY").unwrap_or(4.0);
        let src_w = src_snapshot.sheet.get_f64("Width").unwrap_or(1.6).abs().max(0.4);
        let src_h = src_snapshot.sheet.get_f64("Height").unwrap_or(0.95).abs().max(0.4);
        let (src_min, src_max) = src_vis
            .map(|v| (v.aabb_min, v.aabb_max))
            .unwrap_or(([src_px - src_w * 0.5, src_py - src_h * 0.5], [src_px + src_w * 0.5, src_py + src_h * 0.5]));

        // 1. Check for an existing aligned 2D neighbor in `dir` within `[0.12 in .. 2.35 in]`
        let mut best_neighbor: Option<(f64, u64)> = None;
        if let Some(page) = self.doc.page(page_id) {
            for cand in &page.shapes {
                if cand.id == source_shape_id
                    || !matches!(cand.kind, ShapeKind::Shape2D | ShapeKind::Group)
                    || crate::containers::is_container_shape(cand)
                    || crate::containers::is_swimlane_shape(cand)
                    || crate::containers::is_callout_shape(cand)
                {
                    continue;
                }
                let cpx = cand.sheet.get_f64("PinX").unwrap_or(0.0);
                let cpy = cand.sheet.get_f64("PinY").unwrap_or(0.0);
                let cw = cand.sheet.get_f64("Width").unwrap_or(1.0).abs();
                let ch = cand.sheet.get_f64("Height").unwrap_or(1.0).abs();
                let cmn = [cpx - cw * 0.5, cpy - ch * 0.5];
                let cmx = [cpx + cw * 0.5, cpy + ch * 0.5];

                let (gap, lat_off, max_lat) = match dir {
                    vexio_render::AutoConnectDir::East => (
                        cmn[0] - src_max[0],
                        (cpy - src_py).abs(),
                        src_h.max(ch) * 0.7,
                    ),
                    vexio_render::AutoConnectDir::West => (
                        src_min[0] - cmx[0],
                        (cpy - src_py).abs(),
                        src_h.max(ch) * 0.7,
                    ),
                    vexio_render::AutoConnectDir::North => (
                        cmn[1] - src_max[1],
                        (cpx - src_px).abs(),
                        src_w.max(cw) * 0.7,
                    ),
                    vexio_render::AutoConnectDir::South => (
                        src_min[1] - cmx[1],
                        (cpx - src_px).abs(),
                        src_w.max(cw) * 0.7,
                    ),
                };

                if (0.12..=2.35).contains(&gap) && lat_off <= max_lat {
                    let already_linked = page.connects.iter().any(|c1| {
                        c1.target_shape_id == source_shape_id
                            && page.connects.iter().any(|c2| {
                                c2.connector_shape_id == c1.connector_shape_id
                                    && c2.target_shape_id == cand.id
                            })
                    });
                    if !already_linked {
                        let score = gap + lat_off * 0.5;
                        if best_neighbor.is_none_or(|(b, _)| score < b) {
                            best_neighbor = Some((score, cand.id));
                        }
                    }
                }
            }
        }

        if let Some((_, existing_id)) = best_neighbor {
            let cid = self.connect_shapes(page_id, source_shape_id, existing_id)?;
            return Ok((existing_id, cid));
        }

        // 2. Stamp a new connected step shape `1.15 in` beyond `source_shape_id` in `dir`!
        let clone_source = !crate::containers::is_container_shape(&src_snapshot)
            && !crate::containers::is_swimlane_shape(&src_snapshot)
            && !crate::containers::is_callout_shape(&src_snapshot)
            && src_w <= 3.2
            && src_h <= 2.4;
        let new_w = if clone_source { src_w.clamp(0.75, 2.4) } else { 1.6 };
        let new_h = if clone_source { src_h.clamp(0.55, 1.5) } else { 0.95 };
        let gap_in = 1.15;

        let (mut dst_x, mut dst_y) = match dir {
            vexio_render::AutoConnectDir::East => (src_max[0] + gap_in + new_w * 0.5, src_py),
            vexio_render::AutoConnectDir::West => (src_min[0] - gap_in - new_w * 0.5, src_py),
            vexio_render::AutoConnectDir::North => (src_px, src_max[1] + gap_in + new_h * 0.5),
            vexio_render::AutoConnectDir::South => (src_px, src_min[1] - gap_in - new_h * 0.5),
        };
        dst_x = dst_x.max(new_w * 0.5 + 0.25);
        dst_y = dst_y.max(new_h * 0.5 + 0.25);

        // Pre-discover any Container enclosing `source_shape_id` so the AutoConnected step automatically joins it!
        let source_containers: Vec<u64> = self
            .doc
            .page(page_id)
            .map(|p| {
                p.shapes
                    .iter()
                    .filter(|s| {
                        crate::containers::is_container_shape(s)
                            && crate::containers::parse_container_members(s)
                                .contains(&source_shape_id)
                    })
                    .map(|s| s.id)
                    .collect()
            })
            .unwrap_or_default();

        let mut out_pair = (0u64, 0u64);
        self.journal
            .record_mutation(&mut self.doc, "AutoConnect Shape", |doc| {
                // Auto-expand page width/height if stamping past top/right margins
                if let Some(page) = doc.page_mut(page_id) {
                    let req_pw = dst_x + new_w * 0.5 + 0.6;
                    let req_ph = dst_y + new_h * 0.5 + 0.6;
                    if req_pw > page.width_in() {
                        page.page_sheet.set_formula(
                            "PageWidth",
                            format!("{} in", format_clean_f64(req_pw)),
                            Some(Unit::Inch),
                        );
                    }
                    if req_ph > page.height_in() {
                        page.page_sheet.set_formula(
                            "PageHeight",
                            format!("{} in", format_clean_f64(req_ph)),
                            Some(Unit::Inch),
                        );
                    }
                }

                let (new_sid, conn_id) = {
                    let Some(page) = doc.page_mut(page_id) else {
                        return;
                    };
                    let mut alloc_id = page.next_shape_id();
                    let step_num = page
                        .shapes
                        .iter()
                        .filter(|s| {
                            s.kind == ShapeKind::Shape2D
                                && !crate::containers::is_container_shape(s)
                                && !crate::containers::is_swimlane_shape(s)
                        })
                        .count()
                        + 1;

                    let mut new_sh = if clone_source && src_snapshot.children.is_empty() {
                        let mut c = src_snapshot.clone();
                        let root_id = remap_dropped_shape_subtree(&mut c, &mut alloc_id);
                        c.name = format!("Process.{root_id}");
                        c.name_u = c.name.clone();
                        c.text = format!("Step {step_num}");
                        c.sheet.set_formula(
                            "Width",
                            format!("{} in", format_clean_f64(new_w)),
                            Some(Unit::Inch),
                        );
                        c.sheet.set_formula(
                            "Height",
                            format!("{} in", format_clean_f64(new_h)),
                            Some(Unit::Inch),
                        );
                        c
                    } else {
                        let sid = alloc_id;
                        alloc_id += 1;
                        let mut s = VisioShape::new_rect(
                            sid,
                            format!("Process.{sid}"),
                            dst_x,
                            dst_y,
                            new_w,
                            new_h,
                        );
                        s.text = format!("Step {step_num}");
                        s.sheet
                            .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
                        s.sheet
                            .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
                        s.sheet
                            .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
                        s.sheet
                            .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
                        s
                    };
                    new_sh.sheet.set_formula(
                        "PinX",
                        format!("{} in", format_clean_f64(dst_x)),
                        Some(Unit::Inch),
                    );
                    new_sh.sheet.set_formula(
                        "PinY",
                        format!("{} in", format_clean_f64(dst_y)),
                        Some(Unit::Inch),
                    );
                    attach_cardinal_ports_to_shape(&mut new_sh);
                    let new_sid = new_sh.id;
                    page.shapes.push(new_sh);

                    let conn_id = alloc_id.max(page.next_shape_id());
                    let mut conn = VisioShape::new_connector(
                        conn_id,
                        format!("Dynamic connector.{conn_id}"),
                        src_px,
                        src_py,
                        dst_x,
                        dst_y,
                    );
                    conn.sheet
                        .set_formula("EndArrow", "4", Some(Unit::Number));
                    conn.sheet
                        .set_formula("LineColor", "=RGB(51,65,85)", Some(Unit::Color));
                    conn.sheet
                        .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
                    page.shapes.push(conn);

                    page.connects.push(GlueEndpoint {
                        connector_shape_id: conn_id,
                        from_cell: "BeginX".into(),
                        target_shape_id: source_shape_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    });
                    page.connects.push(GlueEndpoint {
                        connector_shape_id: conn_id,
                        from_cell: "EndX".into(),
                        target_shape_id: new_sid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    });

                    // Inherit source Container membership if `source_shape_id` belonged to a Container
                    for &cid in &source_containers {
                        if let Some(cont_sh) = page.find_shape_mut(cid) {
                            let mut members = crate::containers::parse_container_members(cont_sh);
                            if !members.contains(&new_sid) {
                                members.push(new_sid);
                                crate::containers::set_container_members(cont_sh, &members);
                            }
                        }
                    }

                    (new_sid, conn_id)
                };

                ReactiveEngine::recalculate_document(doc);
                for &cid in &source_containers {
                    crate::containers::refit_container(
                        doc,
                        page_id,
                        cid,
                        0.35,
                        crate::containers::ContainerResizeMode::ExpandOnly,
                    );
                }
                crate::containers::reconcile_containers_and_swimlanes_after_move(
                    doc,
                    page_id,
                    &[new_sid],
                );
                route_page_connectors(doc, page_id);
                out_pair = (new_sid, conn_id);
            });

        Ok(out_pair)
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

fn clean_stencil_stem(file_name: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    let stem = base.rsplit_once('.').map(|(s, _)| s).unwrap_or(base).trim();
    let cleaned = stem
        .trim_start_matches("MIS ")
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        "Imported Visio Stencil".to_string()
    } else {
        cleaned
    }
}

fn clean_icon_display_name(file_name: &str) -> String {
    let stem = file_name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(file_name)
        .trim();
    // Strip Azure Architecture Center numeric prefix (`00028-icon-service-Batch-AI` -> `Batch AI`)
    let without_azure_prefix = if let Some((_, rest)) = stem.split_once("-icon-service-") {
        rest
    } else if let Some((_, rest)) = stem.split_once("-icon-") {
        rest
    } else {
        stem
    };
    let cleaned = without_azure_prefix
        .replace(['_', '-'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if cleaned.is_empty() {
        "Icon".to_string()
    } else {
        cleaned
    }
}

fn title_case_words(raw: &str) -> String {
    raw.split_whitespace()
        .map(|w| {
            if w.len() <= 3 && w.chars().all(|c| c.is_ascii_alphabetic())
                && matches!(w.to_ascii_lowercase().as_str(), "ai" | "ml" | "iot" | "web" | "sql" | "api" | "vm")
            {
                w.to_ascii_uppercase()
            } else {
                let mut chs = w.chars();
                match chs.next() {
                    Some(first) => format!("{}{}", first.to_uppercase(), chs.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn derive_svg_zip_category(zip_stem: &str, norm_path: &str) -> String {
    let parts: Vec<&str> = norm_path
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    if parts.len() <= 1 {
        return zip_stem.to_string();
    }
    let dir_parts = &parts[..parts.len() - 1];
    // Azure Icons: `Azure_Public_Service_Icons/Icons/networking/...` -> `Azure Icons — Networking`
    if norm_path.to_ascii_lowercase().contains("azure_public_service_icons") {
        if let Some(last_dir) = dir_parts.last() {
            return format!("Azure Icons — {}", title_case_words(&last_dir.replace(['_', '-'], " ")));
        }
    }
    // M365 Content Icons: `Microsoft Blue/48x48 Grey & Blue Icon/...` -> `M365 Icons — Microsoft Blue`
    let meaningful = dir_parts
        .iter()
        .rev()
        .find(|d| {
            let l = d.to_ascii_lowercase();
            !l.contains("48x48") && !l.contains("svg") && l != "icons"
        })
        .or_else(|| dir_parts.last())
        .copied()
        .unwrap_or(zip_stem);
    let nice = title_case_words(&meaningful.replace(['_', '-'], " "));
    if zip_stem.to_ascii_lowercase().contains("microsoft 365") || zip_stem.to_ascii_lowercase().contains("office") {
        format!("M365 Icons — {nice}")
    } else {
        nice
    }
}

fn build_foreign_icon_master(
    id: u32,
    name: &str,
    pack_title: &str,
    ext: &str,
    bytes: &[u8],
) -> vexio_shapesheet::VisioMaster {
    let mut sh = VisioShape::new_rect(1, name, 0.45, 0.45, 0.90, 0.90);
    sh.kind = ShapeKind::ForeignObject;
    sh.sheet.geometries.clear();
    let mut frame = vexio_shapesheet::GeometrySection::rectangle(1);
    frame.no_fill = "TRUE".into();
    frame.no_line = "TRUE".into();
    sh.sheet.add_geometry(frame);
    sh.sheet
        .set_formula("FillPattern", "0", Some(Unit::Number));
    sh.sheet
        .set_formula("LinePattern", "0", Some(Unit::Number));
    if ext.eq_ignore_ascii_case("svg") {
        sh.sheet
            .set_user_cell("ForeignSvgPrimary", "TRUE", "Primary SVG Icon");
    }
    sh.foreign_data = Some(vexio_shapesheet::ForeignMediaData {
        media_type: if ext.eq_ignore_ascii_case("png") {
            "PNG".into()
        } else {
            "SVG".into()
        },
        rel_id: None,
        data: bytes.to_vec(),
    });
    crate::visio_artwork::apply_visio_below_icon_caption(&mut sh);
    sh.sheet.sync_all_sections_to_cells();
    vexio_shapesheet::VisioMaster {
        id,
        name: name.to_string(),
        name_u: name.to_string(),
        base_id: format!("{{ICON-{id:05}}}"),
        unique_id: format!("{{ICON-U-{id:05}}}"),
        prompt: format!("{pack_title}: {name}"),
        icon_bytes: None,
        page_sheet: vexio_shapesheet::ShapeSheet::new(),
        shapes: vec![sh],
    }
}

fn clear_subtree_preview_text(sh: &mut VisioShape) {
    // Only clear external label subshapes that have no geometry (`FillPattern=0 && LinePattern=0`) so icon badges (`T`, `S`, `E`) remain intact!
    for child in &mut sh.children {
        let fp = child.sheet.get_f64("FillPattern").unwrap_or(1.0).round() as i32;
        let lp = child.sheet.get_f64("LinePattern").unwrap_or(1.0).round() as i32;
        if fp == 0 && lp == 0 && child.foreign_data.is_none() && child.children.is_empty() && child.text.len() > 2 {
            child.text.clear();
        }
        clear_subtree_preview_text(child);
    }
}
