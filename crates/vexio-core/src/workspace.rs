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
    ConnectionPointRow, ExternalDataRecordset, GeometryRow, GeometryRowKind, GeometrySection,
    GlueEndpoint, ReactiveEngine, RgbaColor, ShapeKind, StencilPack, Unit, UserCellRow,
    VisioDocument, VisioPage, VisioShape, format_clean_f64,
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
    #[error("Data Linking error: {0}")]
    DataLink(#[from] crate::data_link::DataLinkError),
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

fn format_baked_cell_formula(val: &vexio_shapesheet::CellValue) -> String {
    match val {
        vexio_shapesheet::CellValue::Num { internal, unit } => match unit {
            Unit::Inch
            | Unit::Millimeter
            | Unit::Centimeter
            | Unit::Meter
            | Unit::Foot
            | Unit::Point
            | Unit::Pica => format!("{} in", format_clean_f64(*internal)),
            Unit::Radian | Unit::Degree => format!("{} rad", format_clean_f64(*internal)),
            _ => format_clean_f64(*internal),
        },
        vexio_shapesheet::CellValue::Bool(b) => {
            if *b {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        vexio_shapesheet::CellValue::Color(c) => format!("RGB({},{},{})", c.r, c.g, c.b),
        vexio_shapesheet::CellValue::Str(s) => format!("\"{}\"", s.replace('"', "\"\"")),
        vexio_shapesheet::CellValue::Point { x, y } => {
            format!("PNT({} in, {} in)", format_clean_f64(*x), format_clean_f64(*y))
        }
        _ => "0".into(),
    }
}

fn bake_parent_group_refs_in_subtree(node: &mut VisioShape, parent_gid: u64) {
    let sheet_needle = format!("sheet.{parent_gid}!");
    let mut baked_map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for (k, cell) in &mut node.sheet.cells {
        let low = cell.formula.to_ascii_lowercase();
        if low.contains(&sheet_needle) || low.contains("theparent!") {
            let baked = format_baked_cell_formula(&cell.value);
            let nm = cell.name.clone();
            let hint = cell.unit_hint;
            let val_keep = cell.value.clone();
            *cell = vexio_shapesheet::Cell::from_formula(nm, baked.clone(), hint);
            cell.value = val_keep;
            baked_map.insert(k.clone(), baked);
        }
    }
    if !baked_map.is_empty() {
        for geom in &mut node.sheet.geometries {
            let prefix = format!("geometry{}", geom.index);
            if let Some(v) = baked_map.get(&format!("{prefix}.nofill")) {
                geom.no_fill = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("{prefix}.noline")) {
                geom.no_line = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("{prefix}.noshow")) {
                geom.no_show = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("{prefix}.nosnap")) {
                geom.no_snap = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("{prefix}.noquickdrag")) {
                geom.no_quick_drag = v.clone();
            }
            for row in &mut geom.rows {
                let ix = row.ix;
                if let Some(v) = baked_map.get(&format!("{prefix}.x{ix}")) {
                    row.x = v.clone();
                }
                if let Some(v) = baked_map.get(&format!("{prefix}.y{ix}")) {
                    row.y = v.clone();
                }
                if let Some(v) = baked_map.get(&format!("{prefix}.a{ix}")) {
                    row.a = Some(v.clone());
                }
                if let Some(v) = baked_map.get(&format!("{prefix}.b{ix}")) {
                    row.b = Some(v.clone());
                }
                if let Some(v) = baked_map.get(&format!("{prefix}.c{ix}")) {
                    row.c = Some(v.clone());
                }
                if let Some(v) = baked_map.get(&format!("{prefix}.d{ix}")) {
                    row.d = Some(v.clone());
                }
                if let Some(v) = baked_map.get(&format!("{prefix}.e{ix}")) {
                    row.e = Some(v.clone());
                }
            }
        }
        for urow in node.sheet.user_cells.values_mut() {
            let k = format!("user.{}", urow.name.to_ascii_lowercase());
            if let Some(v) = baked_map.get(&k) {
                urow.value = v.clone();
            }
        }
        for crow in &mut node.sheet.controls {
            let n = crow.name.to_ascii_lowercase();
            if let Some(v) = baked_map.get(&format!("controls.{n}.x")) {
                crow.x = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("controls.{n}.y")) {
                crow.y = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("controls.{n}.xdyn")) {
                crow.x_dyn = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("controls.{n}.ydyn")) {
                crow.y_dyn = v.clone();
            }
        }
        for cp in &mut node.sheet.connection_points {
            let ix = cp.ix;
            if let Some(v) = baked_map.get(&format!("connections.x{ix}")) {
                cp.x = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("connections.y{ix}")) {
                cp.y = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("connections.dirx{ix}")) {
                cp.dir_x = v.clone();
            }
            if let Some(v) = baked_map.get(&format!("connections.diry{ix}")) {
                cp.dir_y = v.clone();
            }
        }
        for prow in node.sheet.shape_data.values_mut() {
            let k = format!("prop.{}", prow.name.to_ascii_lowercase());
            if let Some(v) = baked_map.get(&k) {
                prow.value = v.clone();
            }
        }
    }
    for child in &mut node.children {
        bake_parent_group_refs_in_subtree(child, parent_gid);
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
            "mmd" | "mermaid" | "puml" | "plantuml" | "uml" | "dot" | "gv" => {
                let title = match clean_ext.as_str() {
                    "puml" | "plantuml" | "uml" => "PlantUML Diagram",
                    "dot" | "gv" => "Graphviz DOT Diagram",
                    _ => "Mermaid Diagram",
                };
                let mut doc = VisioDocument::new_blank(title);
                let text = String::from_utf8_lossy(bytes);
                let fmt = crate::diagram_text_interop::DiagramMarkupFormat::from_slug(&clean_ext);
                crate::diagram_text_interop::import_diagram_markup(&mut doc, 1, &text, fmt)
                    .map_err(CoreError::UnsupportedExtension)?;
                self.doc = doc;
            }
            _ => return Err(CoreError::UnsupportedExtension(clean_ext)),
        }
        self.journal = CommandJournal::new();
        self.sync_document_masters_to_stencil_shelf();
        Ok(())
    }

    pub(crate) fn sync_document_masters_to_stencil_shelf(&mut self) {
        if self.doc.masters.is_empty() {
            return;
        }
        let my_masters: Vec<vexio_shapesheet::VisioMaster> = self
            .doc
            .masters
            .iter()
            .filter(|m| m.prompt.starts_with("My Shapes:") || m.base_id.starts_with("{MYSHAPE-"))
            .cloned()
            .collect();
        if !my_masters.is_empty() {
            let styles = self.doc.styles.clone();
            let themes = self.doc.themes.clone();
            self.register_custom_masters_preserving_ports("My Shapes", my_masters, &styles, &themes);
        }
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
            "pptx" => {
                let snippets = crate::enterprise_plan2::collect_document_slide_snippets(&self.doc);
                crate::enterprise_plan2::export_presentation_pptx(&self.doc, &snippets)
                    .map_err(|e| CoreError::UnsupportedExtension(e.to_string()))
            }
            "mmd" | "mermaid" | "puml" | "plantuml" | "uml" | "dot" | "gv" => {
                let fmt = crate::diagram_text_interop::DiagramMarkupFormat::from_slug(&clean_ext);
                crate::diagram_text_interop::export_diagram_markup(&self.doc, pid, fmt)
                    .map(String::into_bytes)
                    .map_err(CoreError::UnsupportedExtension)
            }
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
        let matched_master = self
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
            .cloned();

        let resolved_doc_master_id = if let Some(ref mm) = matched_master {
            if let Some(existing_dm) = self
                .doc
                .masters
                .iter()
                .find(|dm| dm.name.eq_ignore_ascii_case(&mm.name) || (!mm.unique_id.is_empty() && dm.unique_id == mm.unique_id))
            {
                Some(existing_dm.id)
            } else if category.eq_ignore_ascii_case("My Shapes") || mm.base_id.starts_with("{MYSHAPE-") {
                let next_mid = self.doc.masters.iter().map(|m| m.id).max().unwrap_or(0) + 1;
                let mut dm = mm.clone();
                dm.id = next_mid;
                self.doc.masters.push(dm);
                Some(next_mid)
            } else {
                None
            }
        } else {
            None
        };

        let template = matched_master.and_then(|m| m.shapes.first().cloned());

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
        if let Some(mid) = resolved_doc_master_id {
            shape.master_id = Some(mid);
        }
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

    /// Slide an orthogonal segment (`seg_index`, 0-based along routed polyline) of a 1D connector
    /// to `coord_in` (X in inches for a vertical segment, Y in inches for a horizontal segment),
    /// preserving all user-authored multi-bend orthogonal waypoints in `User.ManualOrthoPoints`
    /// and inserting orthogonal dogleg stubs when dragging an endpoint-adjacent segment!
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
        if poly.len() < 2 {
            return Ok(false);
        }
        let idx = seg_index.min(poly.len() - 2);
        let axis = if (poly[idx][0] - poly[idx + 1][0]).abs()
            <= (poly[idx][1] - poly[idx + 1][1]).abs()
        {
            1
        } else {
            2
        };
        let new_poly = crate::routing::slide_orthogonal_segment(&poly, seg_index, coord_in);
        let serialized_pts = crate::routing::format_manual_ortho_points(&new_poly);

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
                            sh.sheet.set_user_cell(
                                "ManualOrthoPoints",
                                serialized_pts.clone(),
                                "User-adjusted multi-bend orthogonal waypoints",
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

    /// Clear any manual orthogonal segment overrides on `shape_id` and restore automatic obstacle-avoiding A* routing.
    pub fn reset_connector_routing(
        &mut self,
        page_id: u32,
        shape_id: u64,
    ) -> Result<bool, CoreError> {
        let mut ok = false;
        self.journal
            .record_mutation(&mut self.doc, "Reset Connector Routing", |doc| {
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
                            sh.sheet
                                .set_formula("User.MidAxis", "0", Some(Unit::Number));
                            sh.sheet.user_cells.remove("manualorthopoints");
                            sh.sheet.set_formula(
                                "User.ManualOrthoPoints",
                                "\"\"",
                                Some(Unit::String),
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

    /// Ensure `(page_id, shape_id)` has a draggable yellow diamond `Controls.Row_1` handle (undoable).
    pub fn ensure_control_handle(
        &mut self,
        page_id: u32,
        shape_id: u64,
    ) -> Result<bool, CoreError> {
        let mut ok = false;
        self.journal
            .record_mutation(&mut self.doc, "Add Control Handle", |doc| {
                ok = crate::boolean_ops::ensure_default_control_handle(doc, page_id, shape_id);
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
                            if let Some(raw_pts) = sh
                                .sheet
                                .get_cell("User.ManualOrthoPoints")
                                .and_then(|c| c.value.as_str_coerced().ok())
                            {
                                let mut mpts = crate::routing::parse_manual_ortho_points(&raw_pts);
                                if mpts.len() >= 2 {
                                    for p in &mut mpts {
                                        p[0] += eff_dx;
                                        p[1] += eff_dy;
                                    }
                                    sh.sheet.set_user_cell(
                                        "ManualOrthoPoints",
                                        crate::routing::format_manual_ortho_points(&mpts),
                                        "User-adjusted multi-bend orthogonal waypoints",
                                    );
                                }
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

    /// Group 2+ shapes on `page_id` into a new `ShapeKind::Group` container (`⌘G` / `Ctrl+G`),
    /// resolving any descendant IDs to top-level page shapes, converting child coordinates into
    /// proportional group-local formulas (`=Sheet.<gid>!Width*r`) so 8-handle group resize scales
    /// all members smoothly, and preserving undo/redo & external dynamic connectors.
    pub fn group_shapes(&mut self, page_id: u32, shape_ids: &[u64]) -> Result<u64, CoreError> {
        let top_id_set: std::collections::BTreeSet<u64> = {
            let Some(page) = self.doc.page(page_id) else {
                return Ok(0);
            };
            shape_ids
                .iter()
                .filter_map(|&id| page.top_level_shape_id(id))
                .collect()
        };
        if top_id_set.len() < 2 {
            return Ok(0);
        }

        ReactiveEngine::recalculate_document(&mut self.doc);
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let vis_by_id: std::collections::BTreeMap<u64, &vexio_shapesheet::EvaluatedShapeVisual> =
            visuals.iter().map(|v| (v.shape_id, v)).collect();

        let mut g_min_x = f64::INFINITY;
        let mut g_min_y = f64::INFINITY;
        let mut g_max_x = f64::NEG_INFINITY;
        let mut g_max_y = f64::NEG_INFINITY;
        let mut matched_count = 0usize;

        if let Some(page) = self.doc.page(page_id) {
            for top_sh in &page.shapes {
                if !top_id_set.contains(&top_sh.id) {
                    continue;
                }
                let mut sub_ids = std::collections::BTreeSet::new();
                vexio_shapesheet::VisioPage::collect_subtree_ids(top_sh, &mut sub_ids);
                let mut hit_any = false;
                for sid in sub_ids {
                    if let Some(v) = vis_by_id.get(&sid) {
                        let (bmn, bmx) = if v.kind == ShapeKind::Connector1D {
                            (v.aabb_min, v.aabb_max)
                        } else {
                            v.transform.world_aabb()
                        };
                        if bmn[0].is_finite() && bmx[0].is_finite() {
                            g_min_x = g_min_x.min(bmn[0]);
                            g_min_y = g_min_y.min(bmn[1]);
                            g_max_x = g_max_x.max(bmx[0]);
                            g_max_y = g_max_y.max(bmx[1]);
                            hit_any = true;
                        }
                    }
                }
                if hit_any {
                    matched_count += 1;
                }
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
                    if top_id_set.contains(&sh.id) {
                        if insert_pos.is_none() {
                            insert_pos = Some(remaining.len().max(idx.min(remaining.len())));
                        }
                        if sh.kind == ShapeKind::Connector1D {
                            let bx = sh.sheet.get_f64("BeginX").unwrap_or(g_min_x) - g_min_x;
                            let by = sh.sheet.get_f64("BeginY").unwrap_or(g_min_y) - g_min_y;
                            let ex = sh.sheet.get_f64("EndX").unwrap_or(g_min_x + 1.0) - g_min_x;
                            let ey = sh.sheet.get_f64("EndY").unwrap_or(g_min_y) - g_min_y;
                            let rbx = bx / gw;
                            let rby = by / gh;
                            let rex = ex / gw;
                            let rey = ey / gh;
                            sh.sheet.set_formula(
                                "BeginX",
                                format!("=Sheet.{gid}!Width*{}", format_clean_f64(rbx)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "BeginY",
                                format!("=Sheet.{gid}!Height*{}", format_clean_f64(rby)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "EndX",
                                format!("=Sheet.{gid}!Width*{}", format_clean_f64(rex)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "EndY",
                                format!("=Sheet.{gid}!Height*{}", format_clean_f64(rey)),
                                Some(Unit::Inch),
                            );
                            let cpx = (sh.sheet.get_f64("PinX").unwrap_or(g_min_x) - g_min_x) / gw;
                            let cpy = (sh.sheet.get_f64("PinY").unwrap_or(g_min_y) - g_min_y) / gh;
                            sh.sheet.set_formula(
                                "PinX",
                                format!("=Sheet.{gid}!Width*{}", format_clean_f64(cpx)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "PinY",
                                format!("=Sheet.{gid}!Height*{}", format_clean_f64(cpy)),
                                Some(Unit::Inch),
                            );
                        } else {
                            let px = sh.sheet.get_f64("PinX").unwrap_or(g_cx) - g_min_x;
                            let py = sh.sheet.get_f64("PinY").unwrap_or(g_cy) - g_min_y;
                            let cw = sh.sheet.get_f64("Width").unwrap_or(1.0).abs().max(0.02);
                            let ch = sh.sheet.get_f64("Height").unwrap_or(1.0).abs().max(0.02);
                            let clx = sh.sheet.get_f64("LocPinX").unwrap_or(cw * 0.5);
                            let cly = sh.sheet.get_f64("LocPinY").unwrap_or(ch * 0.5);
                            let rx = px / gw;
                            let ry = py / gh;
                            let rw = cw / gw;
                            let rh = ch / gh;
                            let rlx = clx / cw;
                            let rly = cly / ch;
                            sh.sheet.set_formula(
                                "PinX",
                                format!("=Sheet.{gid}!Width*{}", format_clean_f64(rx)),
                                Some(Unit::Inch),
                            );
                            sh.sheet.set_formula(
                                "PinY",
                                format!("=Sheet.{gid}!Height*{}", format_clean_f64(ry)),
                                Some(Unit::Inch),
                            );
                            if !sh.sheet.get_cell("Width").is_some_and(|c| c.is_guarded()) {
                                sh.sheet.set_formula(
                                    "Width",
                                    format!("=Sheet.{gid}!Width*{}", format_clean_f64(rw)),
                                    Some(Unit::Inch),
                                );
                                sh.sheet.set_formula(
                                    "LocPinX",
                                    format!("=Width*{}", format_clean_f64(rlx)),
                                    Some(Unit::Inch),
                                );
                            }
                            if !sh.sheet.get_cell("Height").is_some_and(|c| c.is_guarded()) {
                                sh.sheet.set_formula(
                                    "Height",
                                    format!("=Sheet.{gid}!Height*{}", format_clean_f64(rh)),
                                    Some(Unit::Inch),
                                );
                                sh.sheet.set_formula(
                                    "LocPinY",
                                    format!("=Height*{}", format_clean_f64(rly)),
                                    Some(Unit::Inch),
                                );
                            }
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
                grp.sheet.set_formula("SelectMode", "1", Some(Unit::Number));
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

                let pos = insert_pos.unwrap_or(remaining.len()).min(remaining.len());
                remaining.insert(pos, grp);
                page.shapes = remaining;
                created_gid = gid;
                ReactiveEngine::recalculate_document(doc);
                route_page_connectors(doc, page_id);
            });
        Ok(created_gid)
    }

    /// Ungroup one or more `ShapeKind::Group` shapes (`⇧⌘G` / `Ctrl+Shift+G`), resolving descendant
    /// IDs to their enclosing group, baking parent-group ShapeSheet references (`Sheet.N!` / `TheParent!`),
    /// preserving any group-root own geometry/text, and restoring member shapes to exact world coordinates.
    pub fn ungroup_shapes(
        &mut self,
        page_id: u32,
        group_ids: &[u64],
    ) -> Result<Vec<u64>, CoreError> {
        let top_id_set: std::collections::BTreeSet<u64> = {
            let Some(page) = self.doc.page(page_id) else {
                return Ok(Vec::new());
            };
            group_ids
                .iter()
                .filter_map(|&id| page.top_level_shape_id(id))
                .collect()
        };
        if top_id_set.is_empty() {
            return Ok(Vec::new());
        }

        ReactiveEngine::recalculate_document(&mut self.doc);
        let pre_visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let vis_by_id: std::collections::BTreeMap<u64, vexio_shapesheet::EvaluatedShapeVisual> =
            pre_visuals.into_iter().map(|v| (v.shape_id, v)).collect();

        let mut restored_ids = Vec::new();
        self.journal
            .record_mutation(&mut self.doc, "Ungroup Shapes", |doc| {
                let Some(page) = doc.page_mut(page_id) else {
                    return;
                };

                let mut used_ids = std::collections::BTreeSet::new();
                for existing in &page.shapes {
                    if !top_id_set.contains(&existing.id)
                        || existing.children.is_empty()
                    {
                        vexio_shapesheet::VisioPage::collect_subtree_ids(existing, &mut used_ids);
                    }
                }
                let mut next_alloc_id = page.next_shape_id().max(
                    used_ids.iter().copied().max().unwrap_or(0) + 1,
                );

                let mut out_shapes = Vec::with_capacity(page.shapes.len() + 4);
                let mut any_ungrouped = false;

                for sh in page.shapes.drain(..) {
                    if top_id_set.contains(&sh.id)
                        && (sh.kind == ShapeKind::Group || !sh.children.is_empty())
                        && !sh.children.is_empty()
                    {
                        any_ungrouped = true;
                        let parent_gid = sh.id;
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
                        let g_flip_x = sh.sheet.get_bool("FlipX").unwrap_or(false);
                        let g_flip_y = sh.sheet.get_bool("FlipY").unwrap_or(false);
                        let g_tf = vexio_shapesheet::ShapeTransform2D {
                            pin_x: gpx,
                            pin_y: gpy,
                            width: gw,
                            height: gh,
                            loc_pin_x: glpx,
                            loc_pin_y: glpy,
                            angle_rad: gang,
                            flip_x: g_flip_x,
                            flip_y: g_flip_y,
                        };

                        let is_flag_true = |s: &str| {
                            let t = s.trim();
                            t.eq_ignore_ascii_case("true") || t == "1"
                        };
                        let has_own_visible_geom = sh.sheet.geometries.iter().any(|g| {
                            !is_flag_true(&g.no_show)
                                && (!is_flag_true(&g.no_fill) || !is_flag_true(&g.no_line))
                        });
                        let has_own_foreign = sh
                            .foreign_data
                            .as_ref()
                            .is_some_and(|fd| !fd.data.is_empty());
                        let has_own_text = !sh.text.trim().is_empty();

                        if has_own_visible_geom || has_own_foreign || has_own_text {
                            let mut base_sh = sh.clone();
                            base_sh.children.clear();
                            base_sh.kind = if has_own_foreign {
                                ShapeKind::ForeignObject
                            } else {
                                ShapeKind::Shape2D
                            };
                            if used_ids.contains(&base_sh.id) {
                                base_sh.id = next_alloc_id;
                                next_alloc_id += 1;
                            }
                            used_ids.insert(base_sh.id);
                            restored_ids.push(base_sh.id);
                            out_shapes.push(base_sh);
                        }

                        let parent_master_id = sh.master_id;
                        let parent_line_style = sh.line_style_id;
                        let parent_fill_style = sh.fill_style_id;
                        let parent_text_style = sh.text_style_id;
                        let first_orig_child_id = sh.children.first().map(|c| c.id);
                        let mut first_restored_child_id: Option<u64> = None;

                        for mut child in sh.children {
                            let orig_child_id = child.id;
                            if child.master_id.is_none() && child.master_shape_id.is_some() {
                                child.master_id = parent_master_id;
                            }
                            if child.line_style_id.is_none() {
                                child.line_style_id = parent_line_style;
                            }
                            if child.fill_style_id.is_none() {
                                child.fill_style_id = parent_fill_style;
                            }
                            if child.text_style_id.is_none() {
                                child.text_style_id = parent_text_style;
                            }

                            let bx_uses_parent = child
                                .sheet
                                .get_cell("BeginX")
                                .is_some_and(|c| c.formula.to_ascii_lowercase().contains("sheet."));
                            bake_parent_group_refs_in_subtree(&mut child, parent_gid);

                            let mut child_sub_ids = std::collections::BTreeSet::new();
                            vexio_shapesheet::VisioPage::collect_subtree_ids(&child, &mut child_sub_ids);
                            if child_sub_ids.iter().any(|id| used_ids.contains(id)) {
                                remap_dropped_shape_subtree(&mut child, &mut next_alloc_id);
                            }
                            vexio_shapesheet::VisioPage::collect_subtree_ids(&child, &mut used_ids);
                            next_alloc_id = next_alloc_id.max(
                                used_ids.iter().copied().max().unwrap_or(0) + 1,
                            );

                            let cid = child.id;
                            if orig_child_id != cid {
                                for c in &mut page.connects {
                                    if c.target_shape_id == orig_child_id {
                                        c.target_shape_id = cid;
                                    }
                                    if c.connector_shape_id == orig_child_id {
                                        c.connector_shape_id = cid;
                                    }
                                }
                            }
                            if Some(orig_child_id) == first_orig_child_id {
                                first_restored_child_id = Some(cid);
                            }

                            if child.kind == ShapeKind::Connector1D {
                                let eff_sx = if bx_uses_parent { 1.0 } else { sx };
                                let eff_sy = if bx_uses_parent { 1.0 } else { sy };
                                let cbx = child.sheet.get_f64("BeginX").unwrap_or(0.0) * eff_sx;
                                let cby = child.sheet.get_f64("BeginY").unwrap_or(0.0) * eff_sy;
                                let cex = child.sheet.get_f64("EndX").unwrap_or(1.0) * eff_sx;
                                let cey = child.sheet.get_f64("EndY").unwrap_or(0.0) * eff_sy;
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
                                    format!("{} in", format_clean_f64(0.5 * (wbx + wex))),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "PinY",
                                    format!("{} in", format_clean_f64(0.5 * (wby + wey))),
                                    Some(Unit::Inch),
                                );
                            } else if let Some(ev) = vis_by_id.get(&orig_child_id) {
                                let wtf = &ev.transform;
                                child.sheet.set_formula(
                                    "PinX",
                                    format!("{} in", format_clean_f64(wtf.pin_x)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "PinY",
                                    format!("{} in", format_clean_f64(wtf.pin_y)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "Width",
                                    format!("{} in", format_clean_f64(wtf.width.abs().max(0.01))),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "Height",
                                    format!("{} in", format_clean_f64(wtf.height.abs().max(0.01))),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "LocPinX",
                                    format!("{} in", format_clean_f64(wtf.loc_pin_x)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "LocPinY",
                                    format!("{} in", format_clean_f64(wtf.loc_pin_y)),
                                    Some(Unit::Inch),
                                );
                                child.sheet.set_formula(
                                    "Angle",
                                    format!("{} rad", format_clean_f64(wtf.angle_rad)),
                                    Some(Unit::Radian),
                                );
                                child.sheet.set_formula(
                                    "FlipX",
                                    if wtf.flip_x { "TRUE" } else { "FALSE" },
                                    Some(Unit::Boolean),
                                );
                                child.sheet.set_formula(
                                    "FlipY",
                                    if wtf.flip_y { "TRUE" } else { "FALSE" },
                                    Some(Unit::Boolean),
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
                            }
                            restored_ids.push(cid);
                            out_shapes.push(child);
                        }

                        if let Some(fcid) = first_restored_child_id {
                            for c in &mut page.connects {
                                if c.target_shape_id == parent_gid {
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
                    ReactiveEngine::recalculate_document(doc);
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
        let remaining_foreground = self
            .doc
            .pages
            .iter()
            .filter(|p| p.id != page_id && !p.is_background)
            .count();
        if remaining_foreground == 0 {
            return false;
        }
        let mut removed = false;
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Delete Page", |doc| {
                let before = doc.pages.len();
                doc.pages.retain(|p| p.id != page_id);
                for p in &mut doc.pages {
                    if p.back_page_id == Some(page_id) {
                        p.back_page_id = None;
                    }
                }
                doc.comments.retain(|c| c.page_id != page_id);
                removed = doc.pages.len() < before;
            });
        removed
    }

    pub fn set_page_background(
        &mut self,
        page_id: u32,
        is_background: bool,
        back_page_id: u32,
        page_color_formula: &str,
    ) -> bool {
        if self.doc.page(page_id).is_none() {
            return false;
        }
        if is_background {
            let other_foreground = self
                .doc
                .pages
                .iter()
                .filter(|p| p.id != page_id && !p.is_background)
                .count();
            if other_foreground == 0 {
                return false;
            }
        }
        let clean_color = page_color_formula.trim();
        let _ = self
            .journal
            .record_mutation(&mut self.doc, "Page Background", |doc| {
                let valid_back = if !is_background
                    && back_page_id > 0
                    && back_page_id != page_id
                    && doc.page(back_page_id).is_some()
                {
                    Some(back_page_id)
                } else {
                    None
                };
                if let Some(p) = doc.page_mut(page_id) {
                    p.is_background = is_background;
                    p.back_page_id = valid_back;
                    if clean_color.eq_ignore_ascii_case("none")
                        || clean_color.eq_ignore_ascii_case("default")
                    {
                        p.page_sheet.cells.remove("PageColor");
                    } else if !clean_color.is_empty() {
                        let formula = if clean_color.starts_with('#') && clean_color.len() == 7 {
                            if let (Ok(r), Ok(g), Ok(b)) = (
                                u8::from_str_radix(&clean_color[1..3], 16),
                                u8::from_str_radix(&clean_color[3..5], 16),
                                u8::from_str_radix(&clean_color[5..7], 16),
                            ) {
                                format!("RGB({r},{g},{b})")
                            } else {
                                clean_color.to_string()
                            }
                        } else {
                            clean_color.to_string()
                        };
                        p.page_sheet
                            .set_formula("PageColor", formula, Some(Unit::Color));
                    }
                }
                if is_background {
                    for p in &mut doc.pages {
                        if p.is_background && p.back_page_id.is_some() {
                            p.back_page_id = None;
                        }
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            });
        true
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

    /// Resolve a Microsoft Visio Typography / Heading preset (`title`, `subtitle`, `h1`, `h2`, `h3`,
    /// `section_banner`, `callout_title`, `body`, `caption`) into `(size_pt, char_style, text_rgb, align, default_w, default_h, default_label)`.
    fn resolve_heading_preset_spec(
        preset_slug: &str,
    ) -> (f64, u8, &'static str, u8, f64, f64, &'static str) {
        match preset_slug.trim().to_ascii_lowercase().as_str() {
            "title" => (24.0, 1, "15,23,42", 0, 4.8, 0.62, "Diagram Title"),
            "subtitle" => (14.0, 0, "71,85,105", 0, 4.6, 0.44, "Subtitle or overview"),
            "h1" | "heading1" => (18.0, 1, "15,23,42", 0, 3.8, 0.50, "Heading 1"),
            "h2" | "heading2" => (14.0, 1, "30,41,59", 0, 3.2, 0.42, "Heading 2"),
            "h3" | "heading3" => (12.0, 1, "29,78,216", 0, 2.8, 0.36, "HEADING 3"),
            "section_banner" | "banner" => (13.0, 1, "255,255,255", 0, 4.4, 0.48, "Section Header"),
            "callout_title" => (13.0, 1, "30,58,138", 0, 3.6, 0.46, "Key Section"),
            "caption" => (9.5, 2, "100,116,139", 0, 3.0, 0.34, "Figure caption or note"),
            _ => (11.0, 0, "15,23,42", 0, 3.4, 0.46, "Body text"),
        }
    }

    /// Apply a Microsoft Visio Text Heading / Title typography preset to one or more existing shapes
    /// as a single atomic Undo/Redo batch.
    pub fn apply_text_heading_preset(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        preset_slug: &str,
    ) -> Result<usize, CoreError> {
        let norm = preset_slug.trim().to_ascii_lowercase();
        let (size_pt, char_style, text_rgb, horiz_align, _, _, _) =
            Self::resolve_heading_preset_spec(&norm);

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
            let mut set_cell = |cname: &str, new_f: String| {
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

            set_cell("Char.Size", format!("{} pt", format_clean_f64(size_pt)));
            set_cell("Char.Style", char_style.to_string());
            set_cell("Char.Color", format!("=RGB({text_rgb})"));
            set_cell("Para.HorzAlign", horiz_align.to_string());

            if matches!(norm.as_str(), "section_banner" | "banner") {
                set_cell("FillPattern", "1".into());
                set_cell("FillForegnd", "=RGB(30,64,175)".into());
                set_cell("LinePattern", "1".into());
                set_cell("LineColor", "=RGB(30,58,138)".into());
                set_cell("Rounding", "0.08 in".into());
            } else if norm == "callout_title" {
                set_cell("FillPattern", "1".into());
                set_cell("FillForegnd", "=RGB(239,246,255)".into());
                set_cell("LinePattern", "1".into());
                set_cell("LineColor", "=RGB(37,99,235)".into());
                set_cell("LineWeight", "1.25 pt".into());
                set_cell("Rounding", "0.06 in".into());
            }

            if cmds.len() > before_len {
                touched += 1;
            }
        }

        if !cmds.is_empty() {
            self.journal.execute(
                &mut self.doc,
                DocCommand::Batch {
                    label: format!("Apply Heading Style ({preset_slug})"),
                    commands: cmds,
                },
            )?;
        }
        Ok(touched)
    }

    /// Insert a standalone Microsoft Visio Title, Subtitle, Heading (`H1`–`H3`), Section Banner,
    /// or Caption block onto `page_id` (using smart page-header coordinates when `x_in <= 0 && y_in <= 0`).
    pub fn insert_text_heading(
        &mut self,
        page_id: u32,
        preset_slug: &str,
        custom_text: Option<&str>,
        x_in: f64,
        y_in: f64,
    ) -> Result<u64, CoreError> {
        let norm = preset_slug.trim().to_ascii_lowercase();
        let (size_pt, char_style, text_rgb, horiz_align, def_w, def_h, def_label) =
            Self::resolve_heading_preset_spec(&norm);

        let (page_w, page_h, sid) = {
            let page = self
                .doc
                .page(page_id)
                .ok_or(CoreError::Journal(JournalError::PageNotFound(page_id)))?;
            let pw = page.page_sheet.get_f64("PageWidth").unwrap_or(11.6929).max(3.0);
            let ph = page.page_sheet.get_f64("PageHeight").unwrap_or(8.2677).max(3.0);
            (pw, ph, page.next_shape_id())
        };

        let w = def_w.min((page_w - 0.8).max(1.8));
        let h = def_h;
        let (px, py) = if x_in > 0.0 || y_in > 0.0 {
            (x_in, y_in)
        } else {
            match norm.as_str() {
                "title" => (0.55 + w * 0.5, (page_h - 0.65).max(h * 0.6)),
                "subtitle" => (0.55 + w * 0.5, (page_h - 1.14).max(h * 0.6)),
                _ => (page_w * 0.5, page_h * 0.62),
            }
        };

        let shape_name = match norm.as_str() {
            "title" => format!("Title.{sid}"),
            "subtitle" => format!("Subtitle.{sid}"),
            "h1" | "heading1" => format!("Heading1.{sid}"),
            "h2" | "heading2" => format!("Heading2.{sid}"),
            "h3" | "heading3" => format!("Heading3.{sid}"),
            "section_banner" | "banner" => format!("SectionBanner.{sid}"),
            "callout_title" => format!("CalloutHeader.{sid}"),
            "caption" => format!("Caption.{sid}"),
            _ => format!("BodyText.{sid}"),
        };

        let mut sh = VisioShape::new_rect(sid, &shape_name, px, py, w, h);
        sh.text = custom_text
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(def_label)
            .to_string();

        sh.sheet
            .set_formula("Char.Size", format!("{} pt", format_clean_f64(size_pt)), Some(Unit::Point));
        sh.sheet
            .set_formula("Char.Style", char_style.to_string(), None);
        sh.sheet
            .set_formula("Char.Color", format!("=RGB({text_rgb})"), Some(Unit::Color));
        sh.sheet
            .set_formula("Para.HorzAlign", horiz_align.to_string(), None);

        match norm.as_str() {
            "section_banner" | "banner" => {
                sh.sheet.set_formula("FillPattern", "1", None);
                sh.sheet
                    .set_formula("FillForegnd", "=RGB(30,64,175)", Some(Unit::Color));
                sh.sheet.set_formula("LinePattern", "1", None);
                sh.sheet
                    .set_formula("LineColor", "=RGB(30,58,138)", Some(Unit::Color));
                sh.sheet
                    .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
            }
            "callout_title" => {
                sh.sheet.set_formula("FillPattern", "1", None);
                sh.sheet
                    .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
                sh.sheet.set_formula("LinePattern", "1", None);
                sh.sheet
                    .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
                sh.sheet
                    .set_formula("LineWeight", "1.25 pt", Some(Unit::Point));
                sh.sheet
                    .set_formula("Rounding", "0.06 in", Some(Unit::Inch));
            }
            "h1" | "heading1" => {
                // Frameless fill + crisp bottom accent underline rule (`Geometry1` open bottom line)
                let mut g = vexio_shapesheet::GeometrySection::new(1);
                g.no_fill = "TRUE".into();
                g.rows = vec![
                    vexio_shapesheet::GeometryRow::move_to(1, "0 in", "0 in"),
                    vexio_shapesheet::GeometryRow::line_to(2, "=Width", "0 in"),
                ];
                sh.sheet.geometries = vec![g];
                sh.sheet.sync_all_sections_to_cells();
                sh.sheet.set_formula("FillPattern", "0", None);
                sh.sheet.set_formula("LinePattern", "1", None);
                sh.sheet
                    .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
                sh.sheet
                    .set_formula("LineWeight", "1.75 pt", Some(Unit::Point));
            }
            _ => {
                // Pure frameless Visio text heading / title block
                if let Some(g) = sh.sheet.geometries.first_mut() {
                    g.no_fill = "TRUE".into();
                }
                sh.sheet.sync_all_sections_to_cells();
                sh.sheet.set_formula("FillPattern", "0", None);
                sh.sheet.set_formula("LinePattern", "0", None);
            }
        }

        self.journal.execute(
            &mut self.doc,
            DocCommand::AddShape {
                page_id,
                shape: sh,
            },
        )?;
        Ok(sid)
    }

    /// Apply or remove a Microsoft Visio `Design -> Borders & Titles` page style in an undoable transaction.
    pub fn apply_border_and_title(
        &mut self,
        page_id: u32,
        style_slug: &str,
        title_text: &str,
        subtitle_text: &str,
        author: &str,
        date_or_rev: &str,
    ) -> Result<u32, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let mut bg_pid = 0u32;
        self.journal
            .record_mutation(&mut self.doc, "Borders & Titles", |doc| {
                bg_pid = crate::enterprise_plan2::apply_border_and_title_style(
                    doc,
                    page_id,
                    style_slug,
                    title_text,
                    subtitle_text,
                    author,
                    date_or_rev,
                )
                .unwrap_or(0);
            });
        Ok(bg_pid)
    }

    // =========================================================================
    // v0.5.0-beta: Diagram-as-Code (Mermaid / PlantUML / Graphviz DOT) & Slide Snippets
    // =========================================================================

    /// Parse Mermaid (`.mmd`), PlantUML (`.puml`), or Graphviz DOT (`.dot`) markup and materialize
    /// real Visio shapes, containers, and routed connectors onto `page_id` in an undoable transaction.
    pub fn import_diagram_markup(
        &mut self,
        page_id: u32,
        markup: &str,
        format_slug: &str,
    ) -> Result<crate::diagram_text_interop::MarkupImportStats, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let fmt = crate::diagram_text_interop::DiagramMarkupFormat::from_slug(format_slug);
        let mut res: Result<crate::diagram_text_interop::MarkupImportStats, String> =
            Err("not executed".into());
        self.journal
            .record_mutation(&mut self.doc, "Import Diagram Markup", |doc| {
                res = crate::diagram_text_interop::import_diagram_markup(doc, page_id, markup, fmt);
            });
        res.map_err(CoreError::UnsupportedExtension)
    }

    /// Export `page_id` into clean, round-trippable Mermaid, PlantUML, or Graphviz DOT markup.
    pub fn export_diagram_markup(
        &self,
        page_id: u32,
        format_slug: &str,
    ) -> Result<String, CoreError> {
        let fmt = crate::diagram_text_interop::DiagramMarkupFormat::from_slug(format_slug);
        crate::diagram_text_interop::export_diagram_markup(&self.doc, page_id, fmt)
            .map_err(CoreError::UnsupportedExtension)
    }

    /// Return all persisted Presentation Mode `SlideSnippetFrame`s across all foreground pages.
    pub fn list_slide_snippets(&self) -> Vec<crate::enterprise_plan2::SlideSnippetFrame> {
        crate::enterprise_plan2::collect_document_slide_snippets(&self.doc)
    }

    /// Return all persisted Presentation Mode `SlideSnippetFrame`s serialized as JSON.
    pub fn list_slide_snippets_json(&self) -> String {
        serde_json::to_string(&self.list_slide_snippets()).unwrap_or_else(|_| "[]".into())
    }

    /// Add a Presentation Mode `SlideSnippetFrame` on `page_id` in an undoable transaction.
    #[allow(clippy::too_many_arguments)]
    pub fn add_slide_snippet(
        &mut self,
        page_id: u32,
        title: &str,
        notes: &str,
        x_in: f64,
        y_in: f64,
        width_in: f64,
        height_in: f64,
    ) -> Result<u32, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let clean_title = if title.trim().is_empty() {
            format!("Slide {}", self.list_slide_snippets().len() + 1)
        } else {
            title.trim().to_string()
        };
        let clean_notes = notes.trim().to_string();
        let mut created_id = 0u32;
        self.journal
            .record_mutation(&mut self.doc, "Add Slide Snippet", |doc| {
                let next_id = crate::enterprise_plan2::collect_document_slide_snippets(doc)
                    .iter()
                    .map(|s| s.id)
                    .max()
                    .unwrap_or(0)
                    + 1;
                if let Some(page) = doc.page_mut(page_id) {
                    let pw = page.width_in().max(1.0);
                    let ph = page.height_in().max(1.0);
                    let mut list = crate::enterprise_plan2::read_page_slide_snippets(page);
                    let w = if width_in.is_finite() && width_in > 0.25 {
                        width_in
                    } else {
                        pw
                    };
                    let h = if height_in.is_finite() && height_in > 0.25 {
                        height_in
                    } else {
                        ph
                    };
                    let x = if x_in.is_finite() { x_in.max(0.0) } else { 0.0 };
                    let y = if y_in.is_finite() { y_in.max(0.0) } else { 0.0 };
                    list.push(crate::enterprise_plan2::SlideSnippetFrame {
                        id: next_id,
                        page_id,
                        title: clean_title.clone(),
                        notes: clean_notes.clone(),
                        x_in: x,
                        y_in: y,
                        width_in: w,
                        height_in: h,
                    });
                    crate::enterprise_plan2::write_page_slide_snippets(page, &list);
                    created_id = next_id;
                }
            });
        Ok(created_id)
    }

    /// Delete a Presentation Mode `SlideSnippetFrame` by `snippet_id` (or `0` to clear all on `page_id`).
    pub fn delete_slide_snippet(
        &mut self,
        page_id: u32,
        snippet_id: u32,
    ) -> Result<bool, CoreError> {
        let mut removed = false;
        self.journal
            .record_mutation(&mut self.doc, "Delete Slide Snippet", |doc| {
                if snippet_id == 0 {
                    if let Some(page) = doc.page_mut(page_id) {
                        let had = !crate::enterprise_plan2::read_page_slide_snippets(page).is_empty();
                        crate::enterprise_plan2::write_page_slide_snippets(page, &[]);
                        removed = had;
                    }
                } else {
                    for page in &mut doc.pages {
                        if page_id != 0 && page.id != page_id {
                            continue;
                        }
                        let mut list = crate::enterprise_plan2::read_page_slide_snippets(page);
                        let before = list.len();
                        list.retain(|s| s.id != snippet_id);
                        if list.len() != before {
                            crate::enterprise_plan2::write_page_slide_snippets(page, &list);
                            removed = true;
                        }
                    }
                }
            });
        Ok(removed)
    }

    /// Auto-generate ordered Presentation Mode `SlideSnippetFrame`s on `page_id` from Containers or spatial clusters!
    pub fn auto_generate_slide_snippets(
        &mut self,
        page_id: u32,
    ) -> Result<Vec<crate::enterprise_plan2::SlideSnippetFrame>, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let mut generated = Vec::new();
        self.journal
            .record_mutation(&mut self.doc, "Auto-Generate Slide Snippets", |doc| {
                let mut base_id = doc
                    .pages
                    .iter()
                    .filter(|p| !p.is_background && p.id != page_id)
                    .flat_map(crate::enterprise_plan2::read_page_slide_snippets)
                    .map(|s| s.id)
                    .max()
                    .unwrap_or(0)
                    + 1;
                let mut frames =
                    crate::enterprise_plan2::auto_generate_page_slide_snippets(doc, page_id);
                for f in &mut frames {
                    f.id = base_id;
                    base_id += 1;
                }
                if let Some(page) = doc.page_mut(page_id) {
                    crate::enterprise_plan2::write_page_slide_snippets(page, &frames);
                }
                generated = frames;
            });
        Ok(generated)
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
                                matches!(
                                    cand.kind,
                                    ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
                                ) && !crate::containers::is_swimlane_shape(cand)
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
                    || !matches!(
                        cand.kind,
                        ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
                    )
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

    /// Rename a shape (`s.name` and `s.name_u`) with deterministic Undo/Redo journaling.
    pub fn rename_shape(
        &mut self,
        page_id: u32,
        shape_id: u64,
        new_name: &str,
    ) -> Result<bool, CoreError> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Ok(false);
        }
        let exists = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .is_some();
        if !exists {
            return Ok(false);
        }
        let owned = trimmed.to_string();
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Rename Shape #{shape_id}"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    if let Some(sh) = p.find_shape_mut(shape_id) {
                        sh.name = owned.clone();
                        sh.name_u = owned.clone();
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(true)
    }

    /// Create or update a Visio Shape Data (`Prop.<row_name>`) row and synchronize its ShapeSheet cells.
    pub fn upsert_shape_data(
        &mut self,
        page_id: u32,
        shape_id: u64,
        row_name: &str,
        label: &str,
        data_type: u8,
        value_input: &str,
    ) -> Result<bool, CoreError> {
        let raw_key = row_name
            .trim()
            .strip_prefix("Prop.")
            .or_else(|| row_name.trim().strip_prefix("prop."))
            .unwrap_or_else(|| row_name.trim());
        let clean_name: String = raw_key
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .trim_matches('_')
            .to_string();
        let final_name = if clean_name.is_empty() {
            "Property1".to_string()
        } else if clean_name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            format!("P_{clean_name}")
        } else {
            clean_name
        };
        let final_label = if label.trim().is_empty() {
            final_name.clone()
        } else {
            label.trim().to_string()
        };
        let dtype = match data_type {
            1 => vexio_shapesheet::ShapeDataType::FixedList,
            2 => vexio_shapesheet::ShapeDataType::Number,
            3 => vexio_shapesheet::ShapeDataType::Boolean,
            4 => vexio_shapesheet::ShapeDataType::VariableList,
            5 => vexio_shapesheet::ShapeDataType::DateTime,
            6 => vexio_shapesheet::ShapeDataType::Duration,
            7 => vexio_shapesheet::ShapeDataType::Currency,
            _ => vexio_shapesheet::ShapeDataType::String,
        };
        let val_trimmed = value_input.trim();
        let formatted_formula = if val_trimmed.starts_with('=') {
            val_trimmed.to_string()
        } else {
            match dtype {
                vexio_shapesheet::ShapeDataType::Boolean => {
                    if matches!(
                        val_trimmed.to_ascii_lowercase().as_str(),
                        "true" | "1" | "yes" | "ja" | "on"
                    ) {
                        "TRUE".to_string()
                    } else {
                        "FALSE".to_string()
                    }
                }
                vexio_shapesheet::ShapeDataType::Number
                | vexio_shapesheet::ShapeDataType::Currency
                | vexio_shapesheet::ShapeDataType::Duration => {
                    if val_trimmed.is_empty() {
                        "0".to_string()
                    } else if val_trimmed.parse::<f64>().is_ok() {
                        val_trimmed.to_string()
                    } else {
                        // Preserve unit-suffixed numeric literals such as `12 mm` or `45 deg` if parseable, else `"0"`.
                        if vexio_shapesheet::parse_formula(val_trimmed).is_ok() {
                            val_trimmed.to_string()
                        } else {
                            "0".to_string()
                        }
                    }
                }
                vexio_shapesheet::ShapeDataType::String
                | vexio_shapesheet::ShapeDataType::FixedList
                | vexio_shapesheet::ShapeDataType::VariableList
                | vexio_shapesheet::ShapeDataType::DateTime => {
                    if val_trimmed.len() >= 2
                        && val_trimmed.starts_with('"')
                        && val_trimmed.ends_with('"')
                    {
                        val_trimmed.to_string()
                    } else {
                        format!("\"{}\"", val_trimmed.replace('"', "\"\""))
                    }
                }
            }
        };

        let exists = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .is_some();
        if !exists {
            return Ok(false);
        }

        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Set Shape Data Prop.{final_name}"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    if let Some(sh) = p.find_shape_mut(shape_id) {
                        let lower_key = final_name.to_ascii_lowercase();
                        let existing = sh.sheet.shape_data.get(&lower_key).cloned();
                        let row = vexio_shapesheet::ShapeDataRow {
                            name: existing
                                .as_ref()
                                .map(|r| r.name.clone())
                                .unwrap_or_else(|| final_name.clone()),
                            label: final_label.clone(),
                            prompt: existing.as_ref().map(|r| r.prompt.clone()).unwrap_or_default(),
                            data_type: dtype,
                            format: existing.as_ref().map(|r| r.format.clone()).unwrap_or_default(),
                            value: formatted_formula.clone(),
                            sort_key: existing
                                .as_ref()
                                .map(|r| r.sort_key.clone())
                                .unwrap_or_default(),
                            invisible: false,
                            ask: existing.as_ref().is_some_and(|r| r.ask),
                            lang_id: existing.as_ref().map(|r| r.lang_id).unwrap_or(1033),
                        };
                        sh.sheet.set_shape_data(row);
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(true)
    }

    /// Delete a Shape Data (`Prop.<row_name>`) row and its associated ShapeSheet cells from a shape.
    pub fn delete_shape_data(
        &mut self,
        page_id: u32,
        shape_id: u64,
        row_name: &str,
    ) -> Result<bool, CoreError> {
        let raw_key = row_name
            .trim()
            .strip_prefix("Prop.")
            .or_else(|| row_name.trim().strip_prefix("prop."))
            .unwrap_or_else(|| row_name.trim())
            .to_ascii_lowercase();
        if raw_key.is_empty() {
            return Ok(false);
        }
        let has_row = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(shape_id))
            .is_some_and(|s| s.sheet.shape_data.contains_key(&raw_key));
        if !has_row {
            return Ok(false);
        }
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Delete Shape Data Prop.{raw_key}"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    if let Some(sh) = p.find_shape_mut(shape_id) {
                        sh.sheet.shape_data.remove(&raw_key);
                        sh.sheet.cells.remove(&format!("prop.{raw_key}"));
                        sh.sheet.cells.remove(&format!("prop.{raw_key}.value"));
                        sh.sheet.cells.remove(&format!("prop.{raw_key}.label"));
                        sh.sheet.cells.remove(&format!("prop.{raw_key}.format"));
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(true)
    }

    /// Add a new Layer to `page_id` (or return the existing layer index if case-insensitively matched).
    pub fn add_layer(&mut self, page_id: u32, name: &str) -> Result<u32, CoreError> {
        let Some(page_ref) = self.doc.page(page_id) else {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        };
        let fallback_num = page_ref.layers.len() + 1;
        let clean = if name.trim().is_empty() {
            format!("Layer {fallback_num}")
        } else {
            name.trim().to_string()
        };
        if let Some(existing) = page_ref
            .layers
            .iter()
            .find(|l| l.name.eq_ignore_ascii_case(&clean))
        {
            return Ok(existing.ix);
        }
        let next_ix = page_ref.layers.iter().map(|l| l.ix + 1).max().unwrap_or(0);
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Add Layer '{clean}'"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    p.layers
                        .push(vexio_shapesheet::LayerDef::new(next_ix, clean.clone()));
                }
            },
        );
        Ok(next_ix)
    }

    /// Update Visio Layer flags (`Visible`, `Lock`, `Print`, `Snap` & `Glue`) with Undo/Redo support.
    pub fn set_layer_flags(
        &mut self,
        page_id: u32,
        layer_ix: u32,
        visible: bool,
        lock: bool,
        print: bool,
        snap_glue: bool,
    ) -> Result<bool, CoreError> {
        let exists = self
            .doc
            .page(page_id)
            .is_some_and(|p| p.layers.iter().any(|l| l.ix == layer_ix));
        if !exists {
            return Ok(false);
        }
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Update Layer #{layer_ix}"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    if let Some(layer) = p.layers.iter_mut().find(|l| l.ix == layer_ix) {
                        layer.visible = visible;
                        layer.lock = lock;
                        layer.print = print;
                        layer.snap = snap_glue;
                        layer.glue = snap_glue;
                    }
                }
            },
        );
        Ok(true)
    }

    /// Assign or unassign a shape to/from a page Layer (`layer_ix`), keeping `LayerMember` synchronized for `.vsdx` round-trip.
    pub fn set_shape_layer_membership(
        &mut self,
        page_id: u32,
        shape_id: u64,
        layer_ix: u32,
        assigned: bool,
    ) -> Result<bool, CoreError> {
        let valid = self.doc.page(page_id).is_some_and(|p| {
            p.layers.iter().any(|l| l.ix == layer_ix) && p.find_shape(shape_id).is_some()
        });
        if !valid {
            return Ok(false);
        }
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Set Shape #{shape_id} Layer #{layer_ix}"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    if let Some(sh) = p.find_shape_mut(shape_id) {
                        if assigned {
                            if !sh.layer_membership.contains(&layer_ix) {
                                sh.layer_membership.push(layer_ix);
                                sh.layer_membership.sort_unstable();
                            }
                        } else {
                            sh.layer_membership.retain(|&ix| ix != layer_ix);
                        }
                        if sh.layer_membership.is_empty() {
                            sh.sheet.cells.remove("layermember");
                        } else {
                            let joined: Vec<String> =
                                sh.layer_membership.iter().map(u32::to_string).collect();
                            sh.sheet.set_formula(
                                "LayerMember",
                                format!("\"{}\"", joined.join(";")),
                                Some(Unit::String),
                            );
                        }
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(true)
    }

    // =========================================================================
    // Pillar #4 — Live Data Graphics, Legends & Two-Way External Data Linking
    // =========================================================================

    /// Return the active page's complete Data Graphics configuration, linked External Data
    /// recordsets, row-to-shape bindings, and discovered `Prop.*` field catalog.
    pub fn get_page_data_link_state(&self, page_id: u32) -> PageDataLinkStateDto {
        let resolved_pid = self
            .doc
            .page(page_id)
            .map(|p| p.id)
            .or_else(|| {
                self.doc
                    .pages
                    .iter()
                    .find(|p| !p.is_background)
                    .or_else(|| self.doc.pages.first())
                    .map(|p| p.id)
            })
            .unwrap_or(page_id);

        let mut legend_visible = false;
        let mut legend_field = String::from("Status");
        let mut available_props = std::collections::BTreeSet::new();
        let mut bindings_by_rs: std::collections::BTreeMap<u32, std::collections::BTreeMap<usize, Vec<u64>>> =
            std::collections::BTreeMap::new();

        let page_cfg = if let Some(page) = self.doc.page(resolved_pid) {
            for sh in &page.shapes {
                if sh
                    .sheet
                    .get_str("User.msvStructureType")
                    .is_some_and(|s| s.trim().trim_matches('"').eq_ignore_ascii_case("Legend"))
                {
                    legend_visible = true;
                    if let Some(lp) = sh.sheet.get_str("User.vexioLegendProp") {
                        let clean = lp.trim().trim_matches('"').trim();
                        if !clean.is_empty() {
                            legend_field = clean.to_string();
                        }
                    }
                    continue;
                }
                if let Some((rs_id, row_idx)) = sh.bound_data_row {
                    bindings_by_rs
                        .entry(rs_id)
                        .or_default()
                        .entry(row_idx as usize)
                        .or_default()
                        .push(sh.id);
                }
                for row in sh.sheet.shape_data.values() {
                    if !row.invisible {
                        available_props.insert(row.name.clone());
                    }
                }
            }
            page.data_graphic_config.clone().unwrap_or_default()
        } else {
            vexio_shapesheet::PageDataGraphicConfig::default()
        };

        for rs in &self.doc.data_recordsets {
            for col in &rs.columns {
                available_props.insert(crate::data_link::sanitize_prop_id(col));
            }
        }

        if legend_field == "Status" && !page_cfg.color_by_value_field.trim().is_empty() {
            legend_field = page_cfg.color_by_value_field.clone();
        }

        let recordsets = self
            .doc
            .data_recordsets
            .iter()
            .map(|rs| ExternalRecordsetDto {
                id: rs.id,
                name: rs.name.clone(),
                source_kind: rs.source_kind.clone(),
                source_name: rs.source_name.clone(),
                key_column: rs.key_column.clone(),
                columns: rs.columns.clone(),
                rows: rs.rows.clone(),
                bound_shape_ids: bindings_by_rs.remove(&rs.id).unwrap_or_default(),
            })
            .collect();

        PageDataLinkStateDto {
            data_graphics: DataGraphicConfigDto {
                enabled: page_cfg.enabled,
                icon_set_field: page_cfg.icon_set_field,
                icon_green_min: page_cfg.icon_green_min,
                icon_amber_min: page_cfg.icon_amber_min,
                data_bar_field: page_cfg.data_bar_field,
                data_bar_min: page_cfg.data_bar_min,
                data_bar_max: page_cfg.data_bar_max,
                data_bar_color_hex: page_cfg.data_bar_color.to_hex_rgb(),
                text_callout_field: page_cfg.text_callout_field,
                text_callout_prefix: page_cfg.text_callout_prefix,
                color_by_value_enabled: page_cfg.color_by_value_enabled,
                color_by_value_field: page_cfg.color_by_value_field,
                legend_visible,
                legend_field,
            },
            recordsets,
            available_prop_fields: available_props.into_iter().collect(),
        }
    }

    pub fn get_page_data_link_state_json(&self, page_id: u32) -> String {
        serde_json::to_string(&self.get_page_data_link_state(page_id))
            .unwrap_or_else(|_| "{}".into())
    }

    /// Configure live Data Graphics (`Icon Sets`, `Data Bars`, `Text Callouts`, `Color by Value`)
    /// and optional Page Legend on `page_id` inside an undoable transaction.
    pub fn set_page_data_graphics(
        &mut self,
        page_id: u32,
        dto: DataGraphicConfigDto,
    ) -> Result<usize, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Ok(0);
        }
        let bar_color = RgbaColor::from_hex(&dto.data_bar_color_hex).unwrap_or(RgbaColor {
            r: 37,
            g: 99,
            b: 235,
            a: 255,
        });
        let existing_palette = self
            .doc
            .page(page_id)
            .and_then(|p| p.data_graphic_config.as_ref())
            .map(|c| c.color_palette.clone())
            .unwrap_or_default();

        let new_cfg = vexio_shapesheet::PageDataGraphicConfig {
            enabled: dto.enabled,
            icon_set_field: dto.icon_set_field.trim().to_string(),
            icon_green_min: if dto.icon_green_min.is_finite() {
                dto.icon_green_min
            } else {
                75.0
            },
            icon_amber_min: if dto.icon_amber_min.is_finite() {
                dto.icon_amber_min
            } else {
                40.0
            },
            data_bar_field: dto.data_bar_field.trim().to_string(),
            data_bar_min: if dto.data_bar_min.is_finite() {
                dto.data_bar_min
            } else {
                0.0
            },
            data_bar_max: if dto.data_bar_max.is_finite() && dto.data_bar_max > dto.data_bar_min {
                dto.data_bar_max
            } else {
                100.0
            },
            data_bar_color: bar_color,
            text_callout_field: dto.text_callout_field.trim().to_string(),
            text_callout_prefix: dto.text_callout_prefix.clone(),
            color_by_value_enabled: dto.color_by_value_enabled,
            color_by_value_field: if dto.color_by_value_field.trim().is_empty() {
                dto.icon_set_field.trim().to_string()
            } else {
                dto.color_by_value_field.trim().to_string()
            },
            color_palette: existing_palette,
        };

        let legend_prop = if !dto.legend_field.trim().is_empty() {
            dto.legend_field.trim().to_string()
        } else if !new_cfg.color_by_value_field.is_empty() {
            new_cfg.color_by_value_field.clone()
        } else if !new_cfg.icon_set_field.is_empty() {
            new_cfg.icon_set_field.clone()
        } else {
            "Status".to_string()
        };
        let want_legend = dto.legend_visible;

        let _ = self.journal.record_mutation(
            &mut self.doc,
            "Configure Data Graphics",
            |doc| {
                let (pw, ph) = doc
                    .page(page_id)
                    .map(|p| (p.width_in(), p.height_in()))
                    .unwrap_or((11.0, 8.5));
                if let Some(p) = doc.page_mut(page_id) {
                    p.data_graphic_config = Some(new_cfg.clone());
                    if !want_legend {
                        p.shapes.retain(|sh| {
                            !sh.sheet
                                .get_str("User.msvStructureType")
                                .is_some_and(|s| s.trim().trim_matches('"').eq_ignore_ascii_case("Legend"))
                        });
                    }
                }
                if want_legend {
                    let _ = crate::data_link::insert_page_legend(
                        doc,
                        page_id,
                        &legend_prop,
                        (pw - 1.45).max(1.5),
                        (ph - 1.20).max(1.2),
                    );
                } else {
                    ReactiveEngine::recalculate_document(doc);
                }
            },
        );

        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        Ok(visuals.iter().filter(|v| v.data_graphics.is_some()).count())
    }

    pub fn set_page_data_graphics_json(
        &mut self,
        page_id: u32,
        config_json: &str,
    ) -> Result<usize, CoreError> {
        let dto: DataGraphicConfigDto = serde_json::from_str(config_json)
            .map_err(|e| CoreError::DataLink(crate::data_link::DataLinkError::Json(e)))?;
        self.set_page_data_graphics(page_id, dto)
    }

    /// Import an External Data Recordset (`.xlsx`, `.csv`, `.sqlite`/`.db`, `.json`), store it in
    /// `doc.data_recordsets`, optionally auto-bind matching shapes on `page_id`, and optionally
    /// enable live Data Graphics. Returns `(recordset_id, bound_shape_count)`.
    #[allow(clippy::too_many_arguments)]
    pub fn import_external_recordset(
        &mut self,
        page_id: u32,
        file_name: &str,
        bytes: &[u8],
        key_col_hint: &str,
        table_hint: &str,
        auto_bind: bool,
        auto_enable_graphics: bool,
    ) -> Result<(u32, usize), CoreError> {
        let clean_title = clean_stencil_stem(if file_name.trim().is_empty() {
            "External Data"
        } else {
            file_name
        });
        let next_id = self
            .doc
            .data_recordsets
            .iter()
            .find(|r| r.source_name.eq_ignore_ascii_case(file_name) && !file_name.is_empty())
            .map(|r| r.id)
            .unwrap_or_else(|| {
                self.doc
                    .data_recordsets
                    .iter()
                    .map(|r| r.id)
                    .max()
                    .unwrap_or(0)
                    + 1
            });

        let (parsed_rs, source_kind) = crate::data_link::DataRecordset::from_auto_bytes(
            next_id,
            clean_title,
            file_name,
            bytes,
            key_col_hint,
            table_hint,
        )?;
        let ext_rs = parsed_rs.to_external(source_kind, file_name);
        let inferred_cfg = crate::data_link::infer_data_graphic_config_from_recordset(&parsed_rs);

        let mut linked = 0usize;
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Import External Data '{}'", ext_rs.name),
            |doc| {
                if let Some(existing) = doc.data_recordsets.iter_mut().find(|r| r.id == next_id) {
                    *existing = ext_rs.clone();
                } else {
                    doc.data_recordsets.push(ext_rs.clone());
                }
                if auto_bind {
                    linked = crate::data_link::refresh_recordset_bindings_on_page(
                        doc, page_id, &parsed_rs,
                    );
                }
                if auto_enable_graphics {
                    if let Some(p) = doc.page_mut(page_id) {
                        let needs_seed = p
                            .data_graphic_config
                            .as_ref()
                            .is_none_or(|c| !c.enabled);
                        if needs_seed {
                            p.data_graphic_config = Some(inferred_cfg.clone());
                        }
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );

        Ok((next_id, linked))
    }

    /// Bind a specific row from `recordset_id` onto `shape_ids` on `page_id` (or stamp a new
    /// data-bound shape onto the page when `shape_ids` is empty, matching Visio row drag-to-canvas!).
    pub fn bind_external_row_to_shapes(
        &mut self,
        page_id: u32,
        recordset_id: u32,
        row_index: usize,
        shape_ids: &[u64],
    ) -> Result<usize, CoreError> {
        let Some(ext_rs) = self
            .doc
            .data_recordsets
            .iter()
            .find(|r| r.id == recordset_id)
            .cloned()
        else {
            return Ok(0);
        };
        let rs = crate::data_link::DataRecordset::from_external(&ext_rs);
        let Some(row_map) = rs.rows.get(row_index).cloned() else {
            return Ok(0);
        };

        let mut bound = 0usize;
        let targets: Vec<u64> = shape_ids.iter().copied().filter(|&id| id > 0).collect();
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Bind Row #{} from '{}'", row_index + 1, rs.name),
            |doc| {
                if targets.is_empty() {
                    if let Some(page) = doc.page_mut(page_id) {
                        let new_id = page.next_shape_id();
                        let label = row_map
                            .get(&rs.key_column)
                            .cloned()
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| format!("Row {}", row_index + 1));
                        let offset = (row_index % 6) as f64 * 0.35;
                        let mut sh = VisioShape::new_rect(
                            new_id,
                            format!("DataShape.{new_id}"),
                            3.0 + offset,
                            5.2 - offset * 0.6,
                            1.85,
                            0.95,
                        );
                        sh.text = label;
                        sh.sheet
                            .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
                        sh.sheet
                            .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
                        sh.sheet
                            .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
                        page.shapes.push(sh);
                        bound = crate::data_link::bind_recordset_row_to_shapes(
                            doc,
                            page_id,
                            &rs,
                            row_index,
                            &[new_id],
                        );
                    }
                } else {
                    bound = crate::data_link::bind_recordset_row_to_shapes(
                        doc, page_id, &rs, row_index, &targets,
                    );
                }
            },
        );
        Ok(bound)
    }

    /// Re-bind / refresh one (`recordset_id > 0`) or all (`recordset_id == 0`) External Data
    /// recordsets on `page_id`, optionally ingesting updated file bytes from disk.
    pub fn refresh_external_recordset(
        &mut self,
        page_id: u32,
        recordset_id: u32,
        new_bytes: Option<&[u8]>,
        format_hint: &str,
    ) -> Result<usize, CoreError> {
        let mut updated_rs_opt: Option<(u32, ExternalDataRecordset, crate::data_link::DataRecordset)> =
            None;
        if let Some(bytes) = new_bytes.filter(|b| !b.is_empty()) {
            let existing = self
                .doc
                .data_recordsets
                .iter()
                .find(|r| recordset_id == 0 || r.id == recordset_id)
                .cloned();
            if let Some(ex) = existing {
                let eff_hint = if format_hint.trim().is_empty() {
                    if !ex.source_name.is_empty() {
                        ex.source_name.as_str()
                    } else {
                        ex.source_kind.as_str()
                    }
                } else {
                    format_hint
                };
                let (parsed, kind) = crate::data_link::DataRecordset::from_auto_bytes(
                    ex.id,
                    &ex.name,
                    eff_hint,
                    bytes,
                    &ex.key_column,
                    "",
                )?;
                let ext = parsed.to_external(kind, &ex.source_name);
                updated_rs_opt = Some((ex.id, ext, parsed));
            }
        }

        let mut total_refreshed = 0usize;
        let _ = self.journal.record_mutation(
            &mut self.doc,
            "Refresh External Data",
            |doc| {
                if let Some((id, ext_rs, parsed)) = &updated_rs_opt {
                    if let Some(slot) = doc.data_recordsets.iter_mut().find(|r| r.id == *id) {
                        *slot = ext_rs.clone();
                    }
                    total_refreshed +=
                        crate::data_link::refresh_recordset_bindings_on_page(doc, page_id, parsed);
                } else {
                    let snapshots: Vec<crate::data_link::DataRecordset> = doc
                        .data_recordsets
                        .iter()
                        .filter(|r| recordset_id == 0 || r.id == recordset_id)
                        .map(crate::data_link::DataRecordset::from_external)
                        .collect();
                    for rs in &snapshots {
                        total_refreshed +=
                            crate::data_link::refresh_recordset_bindings_on_page(doc, page_id, rs);
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(total_refreshed)
    }

    /// Push edited `Prop.*` Shape Data values from shapes across all pages back into `recordset_id`
    /// (two-way writeback) and serialize the updated recordset to `.xlsx`, `.csv`, `.sqlite`, or `.json`.
    pub fn sync_and_export_recordset(
        &mut self,
        _page_id: u32,
        recordset_id: u32,
        format_ext: &str,
    ) -> Result<Vec<u8>, CoreError> {
        let target_idx = if recordset_id == 0 {
            if self.doc.data_recordsets.is_empty() {
                None
            } else {
                Some(0)
            }
        } else {
            self.doc
                .data_recordsets
                .iter()
                .position(|r| r.id == recordset_id)
        };

        let Some(idx) = target_idx else {
            // If no external recordset was imported yet, export the full document BOM Shape Report in the requested format!
            return self.export_bom_report_bytes(format_ext);
        };

        let mut working_rs =
            crate::data_link::DataRecordset::from_external(&self.doc.data_recordsets[idx]);
        let page_ids: Vec<u32> = self.doc.pages.iter().map(|p| p.id).collect();
        let mut changed_cells = 0usize;
        for pid in page_ids {
            changed_cells +=
                crate::data_link::sync_shapes_back_to_recordset(&self.doc, pid, &mut working_rs);
        }

        if changed_cells > 0 {
            let updated_rows = working_rs.rows.clone();
            let rs_id = working_rs.id;
            let _ = self.journal.record_mutation(
                &mut self.doc,
                "Two-Way Data Writeback",
                |doc| {
                    if let Some(slot) = doc.data_recordsets.iter_mut().find(|r| r.id == rs_id) {
                        slot.rows = updated_rows.clone();
                    }
                },
            );
        }

        let eff_fmt = if format_ext.trim().is_empty() {
            let sk = self.doc.data_recordsets[idx].source_kind.as_str();
            if sk.is_empty() { "xlsx" } else { sk }
        } else {
            format_ext
        };
        Ok(working_rs.export_bytes(eff_fmt, &working_rs.name)?)
    }

    pub fn delete_external_recordset(&mut self, recordset_id: u32) -> Result<bool, CoreError> {
        if !self.doc.data_recordsets.iter().any(|r| r.id == recordset_id) {
            return Ok(false);
        }
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Remove External Recordset #{recordset_id}"),
            |doc| {
                doc.data_recordsets.retain(|r| r.id != recordset_id);
                for p in &mut doc.pages {
                    for sh in &mut p.shapes {
                        if sh.bound_data_row.is_some_and(|(id, _)| id == recordset_id) {
                            sh.bound_data_row = None;
                        }
                    }
                }
            },
        );
        Ok(true)
    }

    /// Insert (or update existing) live Diagram Legend on `page_id` summarizing `prop_name` categories & color swatches.
    pub fn insert_or_update_data_legend(
        &mut self,
        page_id: u32,
        prop_name: &str,
        pin_x: Option<f64>,
        pin_y: Option<f64>,
    ) -> Result<u64, CoreError> {
        let Some(page_ref) = self.doc.page(page_id) else {
            return Ok(0);
        };
        let (pw, ph) = (page_ref.width_in(), page_ref.height_in());
        let resolved_prop = if !prop_name.trim().is_empty() {
            prop_name.trim().to_string()
        } else if let Some(cfg) = &page_ref.data_graphic_config {
            if !cfg.color_by_value_field.trim().is_empty() {
                cfg.color_by_value_field.clone()
            } else if !cfg.icon_set_field.trim().is_empty() {
                cfg.icon_set_field.clone()
            } else {
                "Status".to_string()
            }
        } else {
            "Status".to_string()
        };
        let px = pin_x
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or_else(|| (pw - 1.45).max(1.5));
        let py = pin_y
            .filter(|v| v.is_finite() && *v > 0.0)
            .unwrap_or_else(|| (ph - 1.20).max(1.2));

        let mut lid = 0u64;
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Insert Legend ({resolved_prop})"),
            |doc| {
                lid = crate::data_link::insert_page_legend(doc, page_id, &resolved_prop, px, py)
                    .unwrap_or(0);
            },
        );
        Ok(lid)
    }

    /// Export a complete BOM / Inventory Shape Report across all foreground pages in `.xlsx`, `.csv`, `.json`, or `.sqlite`.
    pub fn export_bom_report_bytes(&self, format_ext: &str) -> Result<Vec<u8>, CoreError> {
        let report = crate::data_link::generate_bom_shape_report(&self.doc, &[]);
        Ok(report.export_bytes(format_ext, "bom_shape_report")?)
    }

    // ========================================================================
    // v0.6.0-beta: VBA Macro IDE, Sub-Shape Vertices, Comments/Ink, Hyperlinks,
    //              Parameterized Enterprise Domain Wizards & CAD Layer Tools
    // ========================================================================

    pub fn run_vba_macro_with_selection(
        &mut self,
        selection: &[u64],
        script: &str,
    ) -> Result<String, CoreError> {
        let mut exec_res: Option<Result<crate::automation::AutomationExecutionReport, crate::automation::AutomationError>> = None;
        let _ = self.journal.record_mutation(&mut self.doc, "Run VBA Macro", |doc| {
            exec_res = Some(crate::automation::execute_vba_macro_with_selection(
                doc,
                script,
                selection,
                None,
                crate::automation::SandboxLimits::default(),
            ));
        });
        let report = exec_res
            .unwrap_or_else(|| {
                Err(crate::automation::AutomationError::Syntax {
                    line: 1,
                    message: "empty macro invocation".into(),
                })
            })?;
        serde_json::to_string(&report)
            .map_err(|e| CoreError::UnsupportedExtension(e.to_string()))
    }

    pub fn list_shape_vertices_json(&self, page_id: u32, shape_id: u64) -> String {
        let verts = crate::boolean_ops::list_shape_vertices(&self.doc, page_id, shape_id);
        serde_json::to_string(&verts).unwrap_or_else(|_| "[]".into())
    }

    pub fn move_shape_vertex(
        &mut self,
        page_id: u32,
        shape_id: u64,
        geom_index: u32,
        row_ix: u32,
        page_x_in: f64,
        page_y_in: f64,
    ) -> Result<bool, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let mut ok = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Move Shape Vertex", |doc| {
            ok = crate::boolean_ops::move_shape_vertex(
                doc,
                page_id,
                shape_id,
                geom_index,
                row_ix,
                page_x_in,
                page_y_in,
            );
        });
        Ok(ok)
    }

    pub fn insert_shape_vertex(
        &mut self,
        page_id: u32,
        shape_id: u64,
        geom_index: u32,
        after_row_ix: u32,
        page_x_in: f64,
        page_y_in: f64,
    ) -> Result<bool, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let mut ok = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Insert Shape Vertex", |doc| {
            ok = crate::boolean_ops::insert_shape_vertex(
                doc,
                page_id,
                shape_id,
                geom_index,
                after_row_ix,
                page_x_in,
                page_y_in,
            );
        });
        Ok(ok)
    }

    pub fn delete_shape_vertex(
        &mut self,
        page_id: u32,
        shape_id: u64,
        geom_index: u32,
        row_ix: u32,
    ) -> Result<bool, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let mut ok = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Delete Shape Vertex", |doc| {
            ok = crate::boolean_ops::delete_shape_vertex(doc, page_id, shape_id, geom_index, row_ix);
        });
        Ok(ok)
    }

    pub fn draw_freeform_curve_json(
        &mut self,
        page_id: u32,
        points_json: &str,
        closed: bool,
    ) -> Result<u64, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let pts: Vec<[f64; 2]> = serde_json::from_str(points_json)
            .map_err(|e| CoreError::UnsupportedExtension(format!("invalid freeform JSON: {e}")))?;
        let mut sid = 0u64;
        let _ = self.journal.record_mutation(&mut self.doc, "Draw Freeform Curve", |doc| {
            sid = crate::boolean_ops::draw_freeform_curve(doc, page_id, &pts, closed).unwrap_or(0);
        });
        if sid == 0 {
            return Err(CoreError::UnsupportedExtension(
                "freeform curve requires at least 2 points".into(),
            ));
        }
        Ok(sid)
    }

    pub fn list_comments_json(&self, page_id: u32) -> String {
        let filtered: Vec<&vexio_shapesheet::CommentAnnotation> = self
            .doc
            .comments
            .iter()
            .filter(|c| page_id == 0 || c.page_id == page_id)
            .collect();
        serde_json::to_string(&filtered).unwrap_or_else(|_| "[]".into())
    }

    pub fn add_comment(
        &mut self,
        page_id: u32,
        shape_id: u64,
        author: &str,
        text: &str,
        x_in: f64,
        y_in: f64,
    ) -> Result<u32, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let clean_text = text.trim();
        if clean_text.is_empty() {
            return Err(CoreError::UnsupportedExtension("comment text cannot be empty".into()));
        }
        let clean_author = if author.trim().is_empty() {
            "Reviewer"
        } else {
            author.trim()
        };
        let initials: String = clean_author
            .split_whitespace()
            .filter_map(|w| w.chars().next())
            .take(2)
            .collect::<String>()
            .to_uppercase();
        let mut created_id = 0u32;
        let _ = self.journal.record_mutation(&mut self.doc, "Add Review Comment", |doc| {
            let next_id = doc.comments.iter().map(|c| c.id).max().unwrap_or(0) + 1;
            let (rx, ry) = if x_in.is_finite() && y_in.is_finite() && (x_in > 0.0 || y_in > 0.0) {
                (x_in, y_in)
            } else if shape_id > 0 {
                doc.page(page_id)
                    .and_then(|p| p.find_shape(shape_id))
                    .map(|s| {
                        (
                            s.sheet.get_f64("PinX").unwrap_or(2.0),
                            s.sheet.get_f64("PinY").unwrap_or(2.0),
                        )
                    })
                    .unwrap_or((1.5, 1.5))
            } else {
                (1.5, 1.5)
            };
            doc.comments.push(vexio_shapesheet::CommentAnnotation {
                id: next_id,
                author: clean_author.to_string(),
                initials: if initials.is_empty() { "RV".into() } else { initials.clone() },
                timestamp_iso: "2026-11-27T00:00:00Z".into(),
                page_id,
                shape_id: if shape_id == 0 { None } else { Some(shape_id) },
                x_in: rx,
                y_in: ry,
                text: clean_text.to_string(),
                resolved: false,
            });
            created_id = next_id;
        });
        Ok(created_id)
    }

    pub fn set_comment_resolved(&mut self, comment_id: u32, resolved: bool) -> Result<bool, CoreError> {
        if !self.doc.comments.iter().any(|c| c.id == comment_id) {
            return Ok(false);
        }
        let mut updated = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Resolve Review Comment", |doc| {
            if let Some(c) = doc.comments.iter_mut().find(|c| c.id == comment_id) {
                c.resolved = resolved;
                updated = true;
            }
        });
        Ok(updated)
    }

    pub fn delete_comment(&mut self, comment_id: u32) -> Result<bool, CoreError> {
        if !self.doc.comments.iter().any(|c| c.id == comment_id) {
            return Ok(false);
        }
        let mut removed = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Delete Review Comment", |doc| {
            let before = doc.comments.len();
            doc.comments.retain(|c| c.id != comment_id);
            removed = doc.comments.len() < before;
        });
        Ok(removed)
    }

    pub fn clear_page_ink(&mut self, page_id: u32) -> Result<usize, CoreError> {
        let Some(p) = self.doc.page(page_id) else {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        };
        if p.ink_strokes.is_empty() {
            return Ok(0);
        }
        let mut count = 0usize;
        let _ = self.journal.record_mutation(&mut self.doc, "Clear Page Ink", |doc| {
            if let Some(pm) = doc.page_mut(page_id) {
                count = pm.ink_strokes.len();
                pm.ink_strokes.clear();
            }
        });
        Ok(count)
    }

    pub fn erase_ink_at(
        &mut self,
        page_id: u32,
        x_in: f64,
        y_in: f64,
        radius_in: f64,
    ) -> Result<usize, CoreError> {
        let Some(p) = self.doc.page(page_id) else {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        };
        let r = radius_in.max(0.05);
        let hit_any = p.ink_strokes.iter().any(|st| {
            st.points
                .iter()
                .any(|pt| (pt.x_in - x_in).hypot(pt.y_in - y_in) <= r)
        });
        if !hit_any {
            return Ok(0);
        }
        let mut removed = 0usize;
        let _ = self.journal.record_mutation(&mut self.doc, "Erase Markup Ink", |doc| {
            if let Some(pm) = doc.page_mut(page_id) {
                let before = pm.ink_strokes.len();
                pm.ink_strokes.retain(|st| {
                    !st.points
                        .iter()
                        .any(|pt| (pt.x_in - x_in).hypot(pt.y_in - y_in) <= r)
                });
                removed = before.saturating_sub(pm.ink_strokes.len());
            }
        });
        Ok(removed)
    }

    pub fn set_shape_hyperlink_and_screentip(
        &mut self,
        page_id: u32,
        shape_id: u64,
        description: &str,
        address: &str,
        sub_address: &str,
        screentip: &str,
    ) -> Result<bool, CoreError> {
        let Some(page) = self.doc.page(page_id) else {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        };
        if page.find_shape(shape_id).is_none() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }
        let mut ok = false;
        let _ = self.journal.record_mutation(
            &mut self.doc,
            "Set Shape Hyperlink & ScreenTip",
            |doc| {
                if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                    // Replace existing primary/non-subprocess links cleanly so editing in UI updates in place
                    sh.sheet.hyperlinks.clear();
                    sh.sheet
                        .cells
                        .retain(|k, _| !k.starts_with("hyperlink."));
                }
                if !address.trim().is_empty() || !sub_address.trim().is_empty() {
                    let desc = if description.trim().is_empty() {
                        if !sub_address.trim().is_empty() {
                            sub_address.trim()
                        } else {
                            address.trim()
                        }
                    } else {
                        description.trim()
                    };
                    let _ = crate::enterprise_plan2::add_shape_hyperlink(
                        doc,
                        page_id,
                        shape_id,
                        desc,
                        address.trim(),
                        sub_address.trim(),
                        true,
                    );
                }
                ok = crate::enterprise_plan2::set_shape_screentip(
                    doc,
                    page_id,
                    shape_id,
                    screentip.trim(),
                );
            },
        );
        Ok(ok)
    }

    pub fn run_domain_wizard(
        &mut self,
        page_id: u32,
        wizard_slug: &str,
        spec_text: &str,
    ) -> Result<String, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let slug = wizard_slug.trim().to_ascii_lowercase();
        if slug == "sql_forward" || slug == "erd_to_sql" {
            let ddl = crate::domains::forward_engineer_sql_ddl(&self.doc, page_id);
            let res = serde_json::json!({
                "wizard": "sql_forward",
                "createdCount": 0,
                "exportedText": ddl,
                "summary": format!("Forward-engineered SQL DDL ({} bytes)", ddl.len())
            });
            return Ok(res.to_string());
        }

        let mut created_count = 0usize;
        let mut summary = String::new();
        let mut parse_err: Option<String> = None;

        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Domain Wizard ({slug})"),
            |doc| match slug.as_str() {
                "sql_reverse" | "sql_to_erd" | "sql" => {
                    let tables = crate::domains::reverse_engineer_sql_ddl(doc, page_id, spec_text, true);
                    created_count = tables.len();
                    summary = format!("Reverse-engineered {} Crow's Foot ERD table(s)", created_count);
                }
                "org_chart" | "org" => {
                    created_count = crate::domains::build_org_chart_from_csv(doc, page_id, spec_text);
                    summary = format!("Generated Organization Chart with {} role card(s)", created_count);
                }
                "gantt" => {
                    let tasks = parse_gantt_wizard_spec(spec_text);
                    if tasks.is_empty() {
                        parse_err = Some("Gantt spec requires at least 1 task row (ID,Name,StartDay,DurationDays,PercentComplete,DependsOn)".into());
                    } else {
                        created_count = crate::domains::build_gantt_chart(doc, page_id, &tasks);
                        summary = format!("Generated Gantt Chart with {} scheduled task(s)", created_count);
                    }
                }
                "timeline" => {
                    let (title, total_days, milestones) = parse_timeline_wizard_spec(spec_text);
                    if milestones.is_empty() {
                        parse_err = Some("Timeline spec requires at least 1 milestone row (Day,Label,Major)".into());
                    } else {
                        created_count = crate::domains::build_project_timeline(
                            doc,
                            page_id,
                            &title,
                            total_days,
                            &milestones,
                        );
                        summary = format!("Generated Project Timeline '{}' ({} milestone(s))", title, created_count);
                    }
                }
                "pert" | "cpm" => {
                    let tasks = parse_pert_wizard_spec(spec_text);
                    if tasks.is_empty() {
                        parse_err = Some("PERT/CPM spec requires at least 1 task row (ID,Name,DurationDays,Predecessors)".into());
                    } else {
                        let computed = crate::domains::build_pert_critical_path_diagram(doc, page_id, &tasks);
                        created_count = computed.len();
                        let crit_names: Vec<String> = computed
                            .iter()
                            .filter(|n| n.is_critical)
                            .map(|n| n.id.clone())
                            .collect();
                        let project_days = computed.iter().map(|n| n.early_finish).fold(0.0, f64::max);
                        summary = format!(
                            "Generated PERT/CPM ({} node(s), duration {:.1}d, critical path: {})",
                            created_count,
                            project_days,
                            crit_names.join(" -> ")
                        );
                    }
                }
                "rack" | "rack_42u" => {
                    let devices = parse_rack_wizard_spec(spec_text);
                    let rack_id = if let Some(page) = doc.page_mut(page_id) {
                        let rid = page.next_shape_id();
                        let mut rack = VisioShape::new_rect(rid, "Rack42U", 5.0, 4.3, 2.4, 7.35);
                        rack.text = "42U EIA-310 Cabinet".into();
                        rack.sheet.set_formula("FillForegnd", "=RGB(226,232,240)", Some(Unit::Color));
                        rack.sheet.set_formula("VerticalAlign", "0", Some(Unit::Number));
                        page.shapes.push(rack);
                        rid
                    } else {
                        0
                    };
                    if rack_id > 0 {
                        created_count = 1;
                        for (name, u_start, u_h) in devices {
                            if crate::domains::mount_rack_equipment(doc, page_id, rack_id, &name, u_start, u_h).is_some() {
                                created_count += 1;
                            }
                        }
                        summary = format!("Built 42U Rack Cabinet with {} mounted appliance(s)", created_count.saturating_sub(1));
                    }
                }
                "arch_room" | "room" => {
                    let rooms = parse_room_wizard_spec(spec_text);
                    for (idx, (name, px, py, w_in, h_in)) in rooms.into_iter().enumerate() {
                        let rx = if px > 0.0 { px } else { 3.0 + (idx % 3) as f64 * 2.8 };
                        let ry = if py > 0.0 { py } else { 5.2 - (idx / 3) as f64 * 2.4 };
                        if crate::domains::create_architectural_room_space(doc, page_id, &name, rx, ry, w_in, h_in).is_some() {
                            created_count += 1;
                        }
                    }
                    summary = format!("Created {} Architectural Room space(s) with reactive AreaSqFt", created_count);
                }
                other => {
                    parse_err = Some(format!("unsupported domain wizard '{other}'"));
                }
            },
        );

        if let Some(err_msg) = parse_err {
            let _ = self.journal.undo(&mut self.doc);
            return Err(CoreError::UnsupportedExtension(err_msg));
        }
        let res = serde_json::json!({
            "wizard": slug,
            "createdCount": created_count,
            "exportedText": "",
            "summary": summary
        });
        Ok(res.to_string())
    }

    pub fn apply_cad_scale(
        &mut self,
        page_id: u32,
        scale_slug: &str,
        locked: bool,
    ) -> Result<f64, CoreError> {
        if self.doc.page(page_id).is_none() {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        }
        let preset = match scale_slug.trim().to_ascii_lowercase().as_str() {
            "1/4_in_ft" | "quarter_inch" | "1:48" | "imperial_quarter" => {
                vexio_cad::CadScalePreset::ArchitecturalQuarterInchPerFoot
            }
            "1/8_in_ft" | "eighth_inch" | "1:96" | "imperial_eighth" => {
                vexio_cad::CadScalePreset::ArchitecturalEighthInchPerFoot
            }
            "1:50" | "metric_50" => vexio_cad::CadScalePreset::Metric1To50,
            "1:100" | "metric_100" => vexio_cad::CadScalePreset::Metric1To100,
            "1:200" | "metric_200" => vexio_cad::CadScalePreset::Metric1To200,
            "1:1" | "unscaled" => vexio_cad::CadScalePreset::Custom {
                page_in: 1.0,
                drawing_in: 1.0,
            },
            custom => {
                if let Some((a, b)) = custom.split_once(':') {
                    let pa = a.trim().parse::<f64>().unwrap_or(1.0).max(1e-6);
                    let pb = b.trim().parse::<f64>().unwrap_or(1.0).max(1e-6);
                    vexio_cad::CadScalePreset::Custom {
                        page_in: pa,
                        drawing_in: pb,
                    }
                } else {
                    vexio_cad::CadScalePreset::Metric1To100
                }
            }
        };
        let ratio = preset.ratio();
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Apply CAD Scale ({scale_slug})"),
            |doc| {
                if let Some(p) = doc.page_mut(page_id) {
                    vexio_cad::apply_cad_scale_lock(p, preset, locked);
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(ratio)
    }

    pub fn convert_cad_layers(
        &mut self,
        page_id: u32,
        layer_name_or_all: &str,
    ) -> Result<usize, CoreError> {
        let Some(p) = self.doc.page(page_id) else {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        };
        let trimmed = layer_name_or_all.trim();
        let layer_names: Vec<String> = if trimmed.is_empty()
            || trimmed == "*"
            || trimmed.eq_ignore_ascii_case("all")
        {
            p.layers.iter().map(|l| l.name.clone()).collect()
        } else {
            vec![trimmed.to_string()]
        };
        let mut total = 0usize;
        let _ = self.journal.record_mutation(
            &mut self.doc,
            "Convert CAD Layers to Visio Shapes",
            |doc| {
                if let Some(pm) = doc.page_mut(page_id) {
                    for lname in &layer_names {
                        total += vexio_cad::convert_cad_layer_to_visio_shapes(pm, lname).len();
                    }
                }
                ReactiveEngine::recalculate_document(doc);
            },
        );
        Ok(total)
    }

    // =========================================================================
    // Milestone 2 (`v0.7.0-beta`): Smart-Shape `Actions.*` Menus, Connection
    // Point Tool (`Connections.*`), and `"My Shapes"` Custom `.vssx` Stencils
    // =========================================================================

    pub fn list_shapesheet_functions_json() -> String {
        serde_json::to_string(&vexio_shapesheet::list_supported_shapesheet_functions())
            .unwrap_or_else(|_| "[]".into())
    }

    pub fn list_shape_actions(&self, page_id: u32, shape_id: u64) -> Vec<ShapeActionItemDto> {
        let Some(sh) = self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)) else {
            return Vec::new();
        };
        collect_shape_action_items(&self.doc, sh)
    }

    pub fn list_shape_actions_json(&self, page_id: u32, shape_id: u64) -> String {
        serde_json::to_string(&self.list_shape_actions(page_id, shape_id))
            .unwrap_or_else(|_| "[]".into())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_shape_action(
        &mut self,
        page_id: u32,
        shape_id: u64,
        row_name: &str,
        menu: &str,
        action_formula: &str,
        checked_formula: &str,
        disabled_formula: &str,
        begin_group: bool,
    ) -> Result<bool, CoreError> {
        if self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)).is_none() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }
        let clean_row = sanitize_action_row_name(row_name);
        let menu_expr = normalize_string_or_formula_expr(menu, &clean_row);
        let act_expr = normalize_formula_expr_or_zero(action_formula);
        let chk_expr = normalize_bool_formula_expr(checked_formula);
        let dis_expr = normalize_bool_formula_expr(disabled_formula);

        let _ = self.journal.record_mutation(&mut self.doc, "Upsert Smart Action", |doc| {
            let inherited = doc
                .page(page_id)
                .and_then(|p| p.find_shape(shape_id))
                .and_then(|s| s.master_id)
                .and_then(|mid| doc.masters.iter().find(|m| m.id == mid))
                .and_then(|m| m.shapes.first())
                .map(|ms| ms.sheet.actions.clone())
                .unwrap_or_default();
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                if sh.sheet.actions.is_empty() && !inherited.is_empty() {
                    for mr in inherited {
                        sh.sheet.add_action(mr);
                    }
                }
                sh.sheet.add_action(vexio_shapesheet::ActionRow {
                    name: clean_row.clone(),
                    action: act_expr.clone(),
                    menu: menu_expr.clone(),
                    tag_name: clean_row.clone(),
                    checked: chk_expr.clone(),
                    disabled: dis_expr.clone(),
                    invisible: "FALSE".into(),
                    begin_group,
                });
            }
            ReactiveEngine::recalculate_document(doc);
        });
        Ok(true)
    }

    pub fn delete_shape_action(
        &mut self,
        page_id: u32,
        shape_id: u64,
        row_name: &str,
    ) -> Result<bool, CoreError> {
        let target = row_name.trim();
        if target.is_empty() {
            return Ok(false);
        }
        if self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)).is_none() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }
        let mut removed = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Delete Smart Action", |doc| {
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                let before = sh.sheet.actions.len();
                sh.sheet.actions.retain(|a| !a.name.eq_ignore_ascii_case(target));
                removed = sh.sheet.actions.len() < before;
                let prefix = format!("actions.{}.", target.to_ascii_lowercase());
                let bare = format!("actions.{}", target.to_ascii_lowercase());
                sh.sheet
                    .cells
                    .retain(|k, _| k != &bare && !k.starts_with(&prefix));
            }
            if removed {
                ReactiveEngine::recalculate_document(doc);
            }
        });
        Ok(removed)
    }

    pub fn execute_shape_action(
        &mut self,
        page_id: u32,
        shape_id: u64,
        row_name: &str,
    ) -> Result<String, CoreError> {
        let items = self.list_shape_actions(page_id, shape_id);
        let target_name = row_name.trim();
        let Some(act_item) = items
            .iter()
            .find(|a| a.name.eq_ignore_ascii_case(target_name) || a.menu.eq_ignore_ascii_case(target_name))
            .cloned()
        else {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        };
        if act_item.disabled {
            let dto = ShapeActionExecResultDto {
                executed: false,
                row_name: act_item.name,
                menu: act_item.menu,
                target_page_id: None,
                open_text_editor: false,
                macro_ran: None,
                summary: "Action is currently disabled".into(),
            };
            return Ok(serde_json::to_string(&dto).unwrap_or_else(|_| "{}".into()));
        }

        let raw_expr = act_item.action_formula.trim().to_string();
        let eval_formula = if raw_expr.starts_with('=') {
            raw_expr.clone()
        } else {
            format!("={raw_expr}")
        };

        let mut action_token = String::new();
        let _ = self.journal.record_mutation(
            &mut self.doc,
            format!("Action: {}", act_item.menu),
            |doc| {
                let res_val = ReactiveEngine::execute_oneshot_action_formula(
                    doc,
                    page_id,
                    shape_id,
                    &eval_formula,
                );
                action_token = res_val.as_str_coerced().unwrap_or_default();
                route_page_connectors(doc, page_id);
            },
        );

        let mut target_page_id = None;
        let mut open_text_editor = false;
        let mut macro_ran = None;

        if let Some(rest) = action_token.strip_prefix("__ACTION:GOTOPAGE:") {
            let page_target = rest.trim();
            target_page_id = self
                .doc
                .pages
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(page_target))
                .map(|p| p.id)
                .or_else(|| page_target.parse::<u32>().ok().filter(|&id| self.doc.page(id).is_some()));
        } else if action_token.starts_with("__ACTION:OPENTEXTWIN")
            || action_token.starts_with("__ACTION:DOCMD:1022")
        {
            open_text_editor = true;
        } else if let Some(rest) = action_token
            .strip_prefix("__ACTION:RUNMACRO:")
            .or_else(|| action_token.strip_prefix("__ACTION:CALLTHIS:"))
        {
            let proc_name = rest.split(':').next().unwrap_or(rest).trim().to_string();
            if !proc_name.is_empty() {
                macro_ran = Some(proc_name);
            }
        }

        let summary = if let Some(pid_dest) = target_page_id {
            format!("Navigated to page #{pid_dest}")
        } else if open_text_editor {
            format!("Opened text editor via {}", act_item.menu)
        } else if let Some(ref mname) = macro_ran {
            format!("Triggered macro '{mname}'")
        } else {
            format!("Executed '{}'", act_item.menu)
        };

        let dto = ShapeActionExecResultDto {
            executed: true,
            row_name: act_item.name,
            menu: act_item.menu,
            target_page_id,
            open_text_editor,
            macro_ran,
            summary,
        };
        Ok(serde_json::to_string(&dto).unwrap_or_else(|_| "{}".into()))
    }

    pub fn list_connection_points(&self, page_id: u32, shape_id: u64) -> Vec<ConnectionPointItemDto> {
        let Some(sh) = self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)) else {
            return Vec::new();
        };
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let vis_opt = visuals.iter().find(|v| v.shape_id == shape_id);
        collect_connection_point_items(&self.doc, sh, vis_opt)
    }

    pub fn list_connection_points_json(&self, page_id: u32, shape_id: u64) -> String {
        serde_json::to_string(&self.list_connection_points(page_id, shape_id))
            .unwrap_or_else(|_| "[]".into())
    }

    pub fn add_connection_point(
        &mut self,
        page_id: u32,
        shape_id: u64,
        page_x_in: f64,
        page_y_in: f64,
        point_type: u8,
    ) -> Result<u32, CoreError> {
        if self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)).is_none() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let Some(vis) = visuals.iter().find(|v| v.shape_id == shape_id) else {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        };
        let (x_expr, y_expr, dir_x_str, dir_y_str) =
            compute_local_port_formulas(&vis.transform, page_x_in, page_y_in);
        let pt_type = point_type.min(2);
        let mut created_ix = 1u32;

        let _ = self.journal.record_mutation(&mut self.doc, "Add Connection Point", |doc| {
            materialize_inherited_connection_points_if_needed(doc, page_id, shape_id);
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                let next_ix = sh
                    .sheet
                    .connection_points
                    .iter()
                    .map(|cp| cp.ix)
                    .max()
                    .unwrap_or(0)
                    + 1;
                created_ix = next_ix;
                sh.sheet.add_connection_point(ConnectionPointRow {
                    name: format!("X{next_ix}"),
                    ix: next_ix,
                    x: x_expr.clone(),
                    y: y_expr.clone(),
                    dir_x: dir_x_str.clone(),
                    dir_y: dir_y_str.clone(),
                    point_type: pt_type,
                });
            }
            ReactiveEngine::recalculate_document(doc);
            route_page_connectors(doc, page_id);
        });
        Ok(created_ix)
    }

    pub fn move_connection_point(
        &mut self,
        page_id: u32,
        shape_id: u64,
        ix: u32,
        page_x_in: f64,
        page_y_in: f64,
    ) -> Result<bool, CoreError> {
        if self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)).is_none() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }
        let visuals = ReactiveEngine::evaluate_page_visuals(&self.doc, page_id);
        let Some(vis) = visuals.iter().find(|v| v.shape_id == shape_id) else {
            return Ok(false);
        };
        let (x_expr, y_expr, dir_x_str, dir_y_str) =
            compute_local_port_formulas(&vis.transform, page_x_in, page_y_in);
        let mut updated = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Move Connection Point", |doc| {
            materialize_inherited_connection_points_if_needed(doc, page_id, shape_id);
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                if let Some(cp) = sh.sheet.connection_points.iter_mut().find(|c| c.ix == ix) {
                    cp.x = x_expr.clone();
                    cp.y = y_expr.clone();
                    cp.dir_x = dir_x_str.clone();
                    cp.dir_y = dir_y_str.clone();
                    updated = true;
                }
                if updated {
                    sh.sheet.sync_all_sections_to_cells();
                }
            }
            if updated {
                ReactiveEngine::recalculate_document(doc);
                route_page_connectors(doc, page_id);
            }
        });
        Ok(updated)
    }

    pub fn delete_connection_point(
        &mut self,
        page_id: u32,
        shape_id: u64,
        ix: u32,
    ) -> Result<bool, CoreError> {
        if self.doc.page(page_id).and_then(|p| p.find_shape(shape_id)).is_none() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(shape_id)));
        }
        let mut removed = false;
        let _ = self.journal.record_mutation(&mut self.doc, "Delete Connection Point", |doc| {
            materialize_inherited_connection_points_if_needed(doc, page_id, shape_id);
            if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                let before = sh.sheet.connection_points.len();
                sh.sheet.connection_points.retain(|c| c.ix != ix);
                removed = sh.sheet.connection_points.len() < before;
                if removed {
                    for prefix in ["connections.x", "connections.y", "connections.dirx", "connections.diry", "connections.type"] {
                        sh.sheet.cells.remove(&format!("{prefix}{ix}"));
                    }
                    sh.sheet.sync_all_sections_to_cells();
                }
            }
            if removed {
                ReactiveEngine::recalculate_document(doc);
                route_page_connectors(doc, page_id);
            }
        });
        Ok(removed)
    }

    /// Add one or more selected canvas shapes into a custom stencil pack (defaulting to `"My Shapes"`),
    /// generating vector SVG + PNG thumbnails and linking instances via `master_id`.
    pub fn add_selection_to_custom_stencil(
        &mut self,
        page_id: u32,
        shape_ids: &[u64],
        stencil_title: &str,
        master_name: &str,
    ) -> Result<String, CoreError> {
        let pack_title = if stencil_title.trim().is_empty() {
            "My Shapes".to_string()
        } else {
            stencil_title.trim().to_string()
        };
        let Some(page) = self.doc.page(page_id) else {
            return Err(CoreError::Journal(JournalError::PageNotFound(page_id)));
        };
        let selected_shapes: Vec<VisioShape> = shape_ids
            .iter()
            .filter_map(|&sid| page.find_shape(sid).cloned())
            .collect();
        if selected_shapes.is_empty() {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(
                shape_ids.first().copied().unwrap_or(0),
            )));
        };

        let tpl_shape = build_master_template_from_shapes(&self.doc, page_id, &selected_shapes);
        let base_label = if !master_name.trim().is_empty() {
            master_name.trim().to_string()
        } else if !selected_shapes[0].text.trim().is_empty() {
            selected_shapes[0]
                .text
                .lines()
                .next()
                .unwrap_or("Custom Shape")
                .trim()
                .chars()
                .take(36)
                .collect()
        } else {
            selected_shapes[0]
                .name
                .split('.')
                .next()
                .unwrap_or("Custom Shape")
                .trim()
                .to_string()
        };

        let existing_names: Vec<String> = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(&pack_title))
            .map(|p| p.masters.iter().map(|m| m.name.clone()).collect())
            .unwrap_or_default();
        let mut final_name = if base_label.is_empty() {
            "Custom Shape".to_string()
        } else {
            base_label
        };
        if existing_names.iter().any(|n| n.eq_ignore_ascii_case(&final_name)) {
            let mut suffix = 2u32;
            loop {
                let cand = format!("{final_name} {suffix}");
                if !existing_names.iter().any(|n| n.eq_ignore_ascii_case(&cand)) {
                    final_name = cand;
                    break;
                }
                suffix += 1;
            }
        }

        let next_mid = self.doc.masters.iter().map(|m| m.id).max().unwrap_or(0) + 1;
        let new_master = vexio_shapesheet::VisioMaster {
            id: next_mid,
            name: final_name.clone(),
            name_u: final_name.clone(),
            base_id: format!("{{MYSHAPE-BASE-{next_mid:05}}}"),
            unique_id: format!("{{MYSHAPE-UNIQ-{next_mid:05}}}"),
            prompt: format!("{pack_title}: {final_name}"),
            icon_bytes: None,
            page_sheet: vexio_shapesheet::ShapeSheet::new(),
            shapes: vec![tpl_shape],
        };

        let single_sid = if selected_shapes.len() == 1 {
            Some(selected_shapes[0].id)
        } else {
            None
        };
        let master_for_doc = new_master.clone();
        let _ = self.journal.record_mutation(&mut self.doc, "Add to My Shapes", |doc| {
            doc.masters.push(master_for_doc.clone());
            if let Some(sid) = single_sid {
                if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(sid)) {
                    sh.master_id = Some(next_mid);
                }
            }
        });

        let mut pack_masters: Vec<vexio_shapesheet::VisioMaster> = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(&pack_title))
            .map(|p| p.masters.clone())
            .unwrap_or_default();
        pack_masters.push(new_master);

        let styles = self.doc.styles.clone();
        let themes = self.doc.themes.clone();
        let previews =
            self.register_custom_masters_preserving_ports(&pack_title, pack_masters, &styles, &themes);
        let created_preview = previews.iter().find(|p| p.name == final_name).cloned();

        let resp = serde_json::json!({
            "masterId": next_mid,
            "category": pack_title,
            "name": final_name,
            "masterCount": previews.len(),
            "preview": created_preview,
            "previews": previews,
        });
        Ok(serde_json::to_string(&resp).unwrap_or_else(|_| "{}".into()))
    }

    pub fn rename_custom_master(
        &mut self,
        stencil_title: &str,
        old_name: &str,
        new_name: &str,
    ) -> Result<bool, CoreError> {
        let pack_title = if stencil_title.trim().is_empty() {
            "My Shapes"
        } else {
            stencil_title.trim()
        };
        let clean_new = new_name.trim();
        if clean_new.is_empty() || old_name.trim().is_empty() {
            return Ok(false);
        }
        let Some(pack_masters) = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(pack_title))
            .map(|p| p.masters.clone())
        else {
            return Ok(false);
        };
        let mut updated_masters = pack_masters;
        let mut found_uid = None;
        for m in &mut updated_masters {
            if m.name.eq_ignore_ascii_case(old_name.trim()) {
                found_uid = Some(m.unique_id.clone());
                m.name = clean_new.to_string();
                m.name_u = clean_new.to_string();
                m.prompt = format!("{pack_title}: {clean_new}");
                break;
            }
        }
        let Some(uid) = found_uid else {
            return Ok(false);
        };
        for dm in &mut self.doc.masters {
            if (!uid.is_empty() && dm.unique_id == uid) || dm.name.eq_ignore_ascii_case(old_name.trim()) {
                dm.name = clean_new.to_string();
                dm.name_u = clean_new.to_string();
                dm.prompt = format!("{pack_title}: {clean_new}");
            }
        }
        let styles = self.doc.styles.clone();
        let themes = self.doc.themes.clone();
        self.register_custom_masters_preserving_ports(pack_title, updated_masters, &styles, &themes);
        Ok(true)
    }

    pub fn delete_custom_master(
        &mut self,
        stencil_title: &str,
        master_name: &str,
    ) -> Result<bool, CoreError> {
        let pack_title = if stencil_title.trim().is_empty() {
            "My Shapes"
        } else {
            stencil_title.trim()
        };
        let target = master_name.trim();
        if target.is_empty() {
            return Ok(false);
        }
        let Some(pack_masters) = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(pack_title))
            .map(|p| p.masters.clone())
        else {
            return Ok(false);
        };
        let before = pack_masters.len();
        let mut removed_uids = Vec::new();
        let remaining: Vec<vexio_shapesheet::VisioMaster> = pack_masters
            .into_iter()
            .filter(|m| {
                if m.name.eq_ignore_ascii_case(target) {
                    removed_uids.push(m.unique_id.clone());
                    false
                } else {
                    true
                }
            })
            .collect();
        if remaining.len() == before {
            return Ok(false);
        }
        let mut removed_mids = Vec::new();
        self.doc.masters.retain(|dm| {
            let match_rm = (!dm.unique_id.is_empty() && removed_uids.contains(&dm.unique_id))
                || (dm.name.eq_ignore_ascii_case(target) && dm.prompt.starts_with(pack_title));
            if match_rm {
                removed_mids.push(dm.id);
                false
            } else {
                true
            }
        });
        for p in &mut self.doc.pages {
            for sh in &mut p.shapes {
                if sh.master_id.is_some_and(|mid| removed_mids.contains(&mid)) {
                    sh.master_id = None;
                }
            }
        }
        if remaining.is_empty() {
            self.builtin_stencils
                .retain(|p| !p.title.eq_ignore_ascii_case(pack_title));
            self.imported_stencil_previews.remove(pack_title);
        } else {
            let styles = self.doc.styles.clone();
            let themes = self.doc.themes.clone();
            self.register_custom_masters_preserving_ports(pack_title, remaining, &styles, &themes);
        }
        Ok(true)
    }

    /// Update an existing Custom Master's geometry, smart actions, connection points, and visual styles
    /// from `source_shape_id`, propagating changes across all inheriting instances on all pages!
    pub fn update_custom_master_from_shape(
        &mut self,
        page_id: u32,
        source_shape_id: u64,
        stencil_title: &str,
        master_name: &str,
    ) -> Result<u32, CoreError> {
        let pack_title = if stencil_title.trim().is_empty() {
            "My Shapes"
        } else {
            stencil_title.trim()
        };
        let Some(src_shape) = self
            .doc
            .page(page_id)
            .and_then(|p| p.find_shape(source_shape_id))
            .cloned()
        else {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(source_shape_id)));
        };

        let resolved_name = if !master_name.trim().is_empty() {
            master_name.trim().to_string()
        } else if let Some(mname) = resolve_shape_master_name(&self.doc, &src_shape) {
            mname
        } else {
            return Err(CoreError::Journal(JournalError::ShapeNotFound(source_shape_id)));
        };

        let new_tpl = build_master_template_from_shapes(&self.doc, page_id, &[src_shape.clone()]);
        let target_mid = self
            .doc
            .masters
            .iter()
            .find(|m| Some(m.id) == src_shape.master_id || m.name.eq_ignore_ascii_case(&resolved_name))
            .map(|m| m.id)
            .unwrap_or_else(|| self.doc.masters.iter().map(|m| m.id).max().unwrap_or(0) + 1);

        let mut updated_instances = 0u32;
        let tpl_for_doc = new_tpl.clone();
        let name_for_doc = resolved_name.clone();
        let pack_for_doc = pack_title.to_string();

        let _ = self.journal.record_mutation(&mut self.doc, "Update Master & Instances", |doc| {
            if let Some(dm) = doc.masters.iter_mut().find(|m| m.id == target_mid) {
                dm.shapes = vec![tpl_for_doc.clone()];
            } else {
                doc.masters.push(vexio_shapesheet::VisioMaster {
                    id: target_mid,
                    name: name_for_doc.clone(),
                    name_u: name_for_doc.clone(),
                    base_id: format!("{{MYSHAPE-BASE-{target_mid:05}}}"),
                    unique_id: format!("{{MYSHAPE-UNIQ-{target_mid:05}}}"),
                    prompt: format!("{pack_for_doc}: {name_for_doc}"),
                    icon_bytes: None,
                    page_sheet: vexio_shapesheet::ShapeSheet::new(),
                    shapes: vec![tpl_for_doc.clone()],
                });
            }

            if let Some(src_mut) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(source_shape_id)) {
                src_mut.master_id = Some(target_mid);
            }

            let all_pids: Vec<u32> = doc.pages.iter().map(|p| p.id).collect();
            for pid_iter in &all_pids {
                if let Some(pm) = doc.page_mut(*pid_iter) {
                    for sh in &mut pm.shapes {
                        if (*pid_iter == page_id && sh.id == source_shape_id)
                            || sh.master_id != Some(target_mid)
                        {
                            continue;
                        }
                        propagate_master_template_to_instance(&tpl_for_doc, sh);
                        updated_instances += 1;
                    }
                }
            }
            ReactiveEngine::recalculate_document(doc);
            for pid_iter in all_pids {
                route_page_connectors(doc, pid_iter);
            }
        });

        let mut pack_masters: Vec<vexio_shapesheet::VisioMaster> = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(pack_title))
            .map(|p| p.masters.clone())
            .unwrap_or_default();
        if let Some(pm) = pack_masters
            .iter_mut()
            .find(|m| m.name.eq_ignore_ascii_case(&resolved_name) || m.id == target_mid)
        {
            pm.shapes = vec![new_tpl];
        } else if let Some(dm) = self.doc.masters.iter().find(|m| m.id == target_mid) {
            pack_masters.push(dm.clone());
        }
        if !pack_masters.is_empty() {
            let styles = self.doc.styles.clone();
            let themes = self.doc.themes.clone();
            self.register_custom_masters_preserving_ports(pack_title, pack_masters, &styles, &themes);
        }

        Ok(updated_instances)
    }

    /// Export `"My Shapes"` (or any named stencil pack) as a valid standalone OpenXML `.vssx` stencil package.
    pub fn export_custom_stencil_vssx(&self, stencil_title: &str) -> Result<Vec<u8>, CoreError> {
        let pack_title = if stencil_title.trim().is_empty() {
            "My Shapes"
        } else {
            stencil_title.trim()
        };
        let mut masters: Vec<vexio_shapesheet::VisioMaster> = self
            .builtin_stencils
            .iter()
            .find(|p| p.title.eq_ignore_ascii_case(pack_title))
            .map(|p| p.masters.clone())
            .unwrap_or_default();
        if masters.is_empty() && pack_title.eq_ignore_ascii_case("My Shapes") {
            masters = self.doc.masters.clone();
        }
        let mut stencil_doc = VisioDocument::new_blank(pack_title);
        stencil_doc.properties.title = pack_title.to_string();
        stencil_doc.styles = self.doc.styles.clone();
        stencil_doc.themes = self.doc.themes.clone();
        for (idx, mut m) in masters.into_iter().enumerate() {
            m.id = (idx as u32) + 1;
            stencil_doc.masters.push(m);
        }
        Ok(write_vsdx(&stencil_doc, VsdxPackageKind::Stencil)?)
    }

    fn register_custom_masters_preserving_ports(
        &mut self,
        pack_title: &str,
        masters: Vec<vexio_shapesheet::VisioMaster>,
        imported_styles: &[vexio_shapesheet::StyleSheetDef],
        imported_themes: &[vexio_shapesheet::ThemeDef],
    ) -> Vec<crate::domains::StencilMasterPreview> {
        let preview_w_in = 1.50;
        let preview_h_in = 1.00;
        let mut mini_doc = VisioDocument::new_blank("CustomStencilPreview");
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
}

fn parse_gantt_wizard_spec(spec: &str) -> Vec<crate::domains::GanttTaskInput> {
    let trimmed = spec.trim();
    if trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Vec<crate::domains::GanttTaskInput>>(trimmed) {
            return v;
        }
    }
    let mut out = Vec::new();
    for (idx, raw_line) in trimmed.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if idx == 0
            && cols
                .first()
                .is_some_and(|c| c.eq_ignore_ascii_case("id") || c.eq_ignore_ascii_case("task"))
        {
            continue;
        }
        if cols.len() < 4 {
            continue;
        }
        let id = cols[0].to_string();
        let name = cols[1].to_string();
        let start_day = cols[2].parse::<f64>().unwrap_or(0.0).max(0.0);
        let duration_days = cols[3].parse::<f64>().unwrap_or(1.0).max(0.25);
        let percent_complete = cols
            .get(4)
            .and_then(|s| s.trim_end_matches('%').parse::<f64>().ok())
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);
        let depends_on = cols
            .get(5)
            .map(|d| {
                d.split([';', '|'])
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        out.push(crate::domains::GanttTaskInput {
            id,
            name,
            start_day,
            duration_days,
            percent_complete,
            depends_on,
        });
    }
    out
}

fn parse_timeline_wizard_spec(spec: &str) -> (String, f64, Vec<crate::domains::TimelineMilestone>) {
    let mut title = "Project Release Timeline".to_string();
    let mut explicit_horizon: Option<f64> = None;
    let mut milestones = Vec::new();
    for (idx, raw_line) in spec.trim().lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            for seg in rest.split('|') {
                if let Some((k, v)) = seg.split_once(':') {
                    let key = k.trim().to_ascii_lowercase();
                    if key == "title" && !v.trim().is_empty() {
                        title = v.trim().to_string();
                    } else if key == "horizon" || key == "days" {
                        explicit_horizon = v.trim().parse::<f64>().ok();
                    }
                }
            }
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if idx == 0 && cols.first().is_some_and(|c| c.eq_ignore_ascii_case("day")) {
            continue;
        }
        if cols.len() < 2 {
            continue;
        }
        let Ok(day) = cols[0].parse::<f64>() else {
            continue;
        };
        let label = cols[1].to_string();
        let is_major = cols
            .get(2)
            .map(|s| matches!(s.to_ascii_lowercase().as_str(), "true" | "1" | "yes" | "major"))
            .unwrap_or(true);
        milestones.push(crate::domains::TimelineMilestone {
            day: day.max(0.0),
            label,
            is_major,
        });
    }
    let max_day = milestones.iter().map(|m| m.day).fold(30.0, f64::max);
    let horizon = explicit_horizon.unwrap_or((max_day * 1.1).max(10.0));
    (title, horizon, milestones)
}

fn parse_pert_wizard_spec(spec: &str) -> Vec<crate::domains::PertTaskInput> {
    let trimmed = spec.trim();
    if trimmed.starts_with('[') {
        if let Ok(v) = serde_json::from_str::<Vec<crate::domains::PertTaskInput>>(trimmed) {
            return v;
        }
    }
    let mut out = Vec::new();
    for (idx, raw_line) in trimmed.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if idx == 0 && cols.first().is_some_and(|c| c.eq_ignore_ascii_case("id")) {
            continue;
        }
        if cols.len() < 3 {
            continue;
        }
        let id = cols[0].to_string();
        let name = cols[1].to_string();
        let duration_days = cols[2].parse::<f64>().unwrap_or(1.0).max(0.1);
        let predecessors = cols
            .get(3)
            .map(|p| {
                p.split([';', '|'])
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        out.push(crate::domains::PertTaskInput {
            id,
            name,
            duration_days,
            predecessors,
        });
    }
    out
}

fn parse_rack_wizard_spec(spec: &str) -> Vec<(String, u32, u32)> {
    let mut out = Vec::new();
    for (idx, raw_line) in spec.trim().lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if idx == 0
            && cols
                .first()
                .is_some_and(|c| c.eq_ignore_ascii_case("device") || c.eq_ignore_ascii_case("name"))
        {
            continue;
        }
        if cols.len() < 3 {
            continue;
        }
        let name = cols[0].to_string();
        let u_start = cols[1]
            .trim_start_matches(['U', 'u'])
            .parse::<u32>()
            .unwrap_or(1)
            .clamp(1, 42);
        let u_height = cols[2]
            .trim_end_matches(['U', 'u'])
            .parse::<u32>()
            .unwrap_or(1)
            .clamp(1, 42);
        out.push((name, u_start, u_height));
    }
    out
}

fn parse_room_wizard_spec(spec: &str) -> Vec<(String, f64, f64, f64, f64)> {
    let mut out = Vec::new();
    for (idx, raw_line) in spec.trim().lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let cols: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if idx == 0
            && cols
                .first()
                .is_some_and(|c| c.eq_ignore_ascii_case("room") || c.eq_ignore_ascii_case("name"))
        {
            continue;
        }
        if cols.len() >= 5 {
            let name = cols[0].to_string();
            let px = cols[1].parse::<f64>().unwrap_or(0.0);
            let py = cols[2].parse::<f64>().unwrap_or(0.0);
            let w = cols[3].parse::<f64>().unwrap_or(2.5).max(0.4);
            let h = cols[4].parse::<f64>().unwrap_or(2.0).max(0.4);
            out.push((name, px, py, w, h));
        } else if cols.len() >= 3 {
            let name = cols[0].to_string();
            let w = cols[1].parse::<f64>().unwrap_or(2.5).max(0.4);
            let h = cols[2].parse::<f64>().unwrap_or(2.0).max(0.4);
            out.push((name, 0.0, 0.0, w, h));
        }
    }
    if out.is_empty() && !spec.trim().is_empty() {
        out.push((spec.trim().lines().next().unwrap_or("Conference Room").trim().to_string(), 4.5, 4.2, 2.8, 2.1));
    }
    out
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataGraphicConfigDto {
    pub enabled: bool,
    #[serde(default)]
    pub icon_set_field: String,
    #[serde(default = "default_dto_green_min")]
    pub icon_green_min: f64,
    #[serde(default = "default_dto_amber_min")]
    pub icon_amber_min: f64,
    #[serde(default)]
    pub data_bar_field: String,
    #[serde(default)]
    pub data_bar_min: f64,
    #[serde(default = "default_dto_bar_max")]
    pub data_bar_max: f64,
    #[serde(default = "default_dto_bar_color_hex")]
    pub data_bar_color_hex: String,
    #[serde(default)]
    pub text_callout_field: String,
    #[serde(default)]
    pub text_callout_prefix: String,
    #[serde(default)]
    pub color_by_value_enabled: bool,
    #[serde(default)]
    pub color_by_value_field: String,
    #[serde(default)]
    pub legend_visible: bool,
    #[serde(default)]
    pub legend_field: String,
}

fn default_dto_green_min() -> f64 {
    75.0
}
fn default_dto_amber_min() -> f64 {
    40.0
}
fn default_dto_bar_max() -> f64 {
    100.0
}
fn default_dto_bar_color_hex() -> String {
    "#2563eb".into()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRecordsetDto {
    pub id: u32,
    pub name: String,
    pub source_kind: String,
    pub source_name: String,
    pub key_column: String,
    pub columns: Vec<String>,
    pub rows: Vec<std::collections::BTreeMap<String, String>>,
    pub bound_shape_ids: std::collections::BTreeMap<usize, Vec<u64>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageDataLinkStateDto {
    pub data_graphics: DataGraphicConfigDto,
    pub recordsets: Vec<ExternalRecordsetDto>,
    pub available_prop_fields: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeActionItemDto {
    pub name: String,
    pub menu: String,
    pub tag_name: String,
    pub action_formula: String,
    pub checked_formula: String,
    pub disabled_formula: String,
    pub invisible_formula: String,
    pub checked: bool,
    pub disabled: bool,
    pub invisible: bool,
    pub begin_group: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeActionExecResultDto {
    pub executed: bool,
    pub row_name: String,
    pub menu: String,
    pub target_page_id: Option<u32>,
    pub open_text_editor: bool,
    pub macro_ran: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionPointItemDto {
    pub ix: u32,
    pub name: String,
    pub x_formula: String,
    pub y_formula: String,
    pub page_x_in: f64,
    pub page_y_in: f64,
    pub dir_x: f64,
    pub dir_y: f64,
    pub point_type: u8,
}

fn sanitize_action_row_name(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if cleaned.is_empty() {
        "Row_1".into()
    } else if cleaned.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("Action_{cleaned}")
    } else {
        cleaned
    }
}

fn normalize_string_or_formula_expr(raw: &str, fallback: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        return format!("\"{}\"", fallback.replace('"', "\"\""));
    }
    if t.starts_with('=') || (t.starts_with('"') && t.ends_with('"') && t.len() >= 2) {
        t.to_string()
    } else {
        format!("\"{}\"", t.replace('"', "\"\""))
    }
}

fn normalize_formula_expr_or_zero(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        "0".into()
    } else if t.starts_with('=') {
        t.to_string()
    } else {
        format!("={t}")
    }
}

fn normalize_bool_formula_expr(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() {
        "FALSE".into()
    } else if matches!(t.to_ascii_uppercase().as_str(), "0" | "1" | "TRUE" | "FALSE") || t.starts_with('=') {
        t.to_string()
    } else {
        format!("={t}")
    }
}

fn eval_action_bool_cell_or_literal(shape: &VisioShape, cell_key: &str, raw_formula: &str) -> bool {
    if let Some(b) = shape.sheet.get_bool(cell_key) {
        return b;
    }
    let up = raw_formula.trim().trim_start_matches('=').trim().to_ascii_uppercase();
    matches!(up.as_str(), "1" | "TRUE")
}

pub(crate) fn collect_shape_action_items(
    doc: &VisioDocument,
    shape: &VisioShape,
) -> Vec<ShapeActionItemDto> {
    let source_rows: &[vexio_shapesheet::ActionRow] = if !shape.sheet.actions.is_empty() {
        &shape.sheet.actions
    } else if let Some(mid) = shape.master_id {
        doc.masters
            .iter()
            .find(|m| m.id == mid)
            .and_then(|m| m.shapes.first())
            .map(|ms| ms.sheet.actions.as_slice())
            .unwrap_or(&[])
    } else {
        &[]
    };

    source_rows
        .iter()
        .filter_map(|row| {
            let prefix = format!("Actions.{}", row.name);
            let invisible = eval_action_bool_cell_or_literal(
                shape,
                &format!("{prefix}.Invisible"),
                &row.invisible,
            );
            if invisible {
                return None;
            }
            let menu_val = shape
                .sheet
                .get_cell(&format!("{prefix}.Menu"))
                .and_then(|c| c.value.as_str_coerced().ok())
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| {
                    let trimmed = row.menu.trim().trim_start_matches('=').trim().trim_matches('"');
                    if trimmed.is_empty() {
                        row.name.clone()
                    } else {
                        trimmed.replace("\"\"", "\"")
                    }
                });
            let checked =
                eval_action_bool_cell_or_literal(shape, &format!("{prefix}.Checked"), &row.checked);
            let disabled = eval_action_bool_cell_or_literal(
                shape,
                &format!("{prefix}.Disabled"),
                &row.disabled,
            );
            Some(ShapeActionItemDto {
                name: row.name.clone(),
                menu: menu_val,
                tag_name: row.tag_name.clone(),
                action_formula: row.action.clone(),
                checked_formula: row.checked.clone(),
                disabled_formula: row.disabled.clone(),
                invisible_formula: row.invisible.clone(),
                checked,
                disabled,
                invisible,
                begin_group: row.begin_group,
            })
        })
        .collect()
}

pub(crate) fn collect_connection_point_items(
    doc: &VisioDocument,
    shape: &VisioShape,
    vis_opt: Option<&vexio_shapesheet::EvaluatedShapeVisual>,
) -> Vec<ConnectionPointItemDto> {
    let source_rows: &[ConnectionPointRow] = if !shape.sheet.connection_points.is_empty() {
        &shape.sheet.connection_points
    } else if let Some(mid) = shape.master_id {
        doc.masters
            .iter()
            .find(|m| m.id == mid)
            .and_then(|m| m.shapes.first())
            .map(|ms| ms.sheet.connection_points.as_slice())
            .unwrap_or(&[])
    } else {
        &[]
    };
    source_rows
        .iter()
        .map(|cp| {
            let eval_port = vis_opt.and_then(|v| {
                v.connection_points
                    .iter()
                    .find(|ep| ep.ix == cp.ix)
            });
            let [px, py] = eval_port.map(|ep| ep.pos_page_in).unwrap_or_else(|| {
                let lx = shape.sheet.get_f64(&format!("Connections.X{}", cp.ix)).unwrap_or(0.0);
                let ly = shape.sheet.get_f64(&format!("Connections.Y{}", cp.ix)).unwrap_or(0.0);
                if let Some(v) = vis_opt {
                    v.transform.local_to_parent(lx, ly).into()
                } else {
                    [lx, ly]
                }
            });
            let dx = shape
                .sheet
                .get_f64(&format!("Connections.DirX{}", cp.ix))
                .or_else(|| cp.dir_x.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            let dy = shape
                .sheet
                .get_f64(&format!("Connections.DirY{}", cp.ix))
                .or_else(|| cp.dir_y.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            ConnectionPointItemDto {
                ix: cp.ix,
                name: cp.name.clone(),
                x_formula: cp.x.clone(),
                y_formula: cp.y.clone(),
                page_x_in: px,
                page_y_in: py,
                dir_x: dx,
                dir_y: dy,
                point_type: cp.point_type,
            }
        })
        .collect()
}

pub(crate) fn compute_local_port_formulas(
    tf: &vexio_shapesheet::ShapeTransform2D,
    page_x_in: f64,
    page_y_in: f64,
) -> (String, String, String, String) {
    let (lx, ly) = tf.parent_to_local(page_x_in, page_y_in);
    let w = tf.width.abs();
    let h = tf.height.abs();
    let x_expr = if w > 0.05 {
        let fx = ((lx / w) * 10000.0).round() / 10000.0;
        if fx.abs() < 1e-4 {
            "0 in".into()
        } else if (fx - 1.0).abs() < 1e-4 {
            "=Width".into()
        } else {
            format!("=Width*{}", format_clean_f64(fx))
        }
    } else {
        format!("{} in", format_clean_f64(lx))
    };
    let y_expr = if h > 0.05 {
        let fy = ((ly / h) * 10000.0).round() / 10000.0;
        if fy.abs() < 1e-4 {
            "0 in".into()
        } else if (fy - 1.0).abs() < 1e-4 {
            "=Height".into()
        } else {
            format!("=Height*{}", format_clean_f64(fy))
        }
    } else {
        format!("{} in", format_clean_f64(ly))
    };
    let vx = lx - w * 0.5;
    let vy = ly - h * 0.5;
    let len = vx.hypot(vy);
    let (dx, dy) = if len > 1e-4 {
        (vx / len, vy / len)
    } else {
        (0.0, 1.0)
    };
    (
        x_expr,
        y_expr,
        format_clean_f64((dx * 1000.0).round() / 1000.0),
        format_clean_f64((dy * 1000.0).round() / 1000.0),
    )
}

pub(crate) fn materialize_inherited_connection_points_if_needed(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
) {
    let inherited = doc
        .page(page_id)
        .and_then(|p| p.find_shape(shape_id))
        .filter(|s| s.sheet.connection_points.is_empty())
        .and_then(|s| s.master_id)
        .and_then(|mid| doc.masters.iter().find(|m| m.id == mid))
        .and_then(|m| m.shapes.first())
        .map(|ms| ms.sheet.connection_points.clone())
        .unwrap_or_default();
    if !inherited.is_empty() {
        if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
            for cp in inherited {
                sh.sheet.add_connection_point(cp);
            }
        }
    }
}

pub(crate) fn move_connection_point_preview(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    ix: u32,
    page_x_in: f64,
    page_y_in: f64,
) {
    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    let Some(vis) = visuals.iter().find(|v| v.shape_id == shape_id) else {
        return;
    };
    let (x_expr, y_expr, dx_s, dy_s) =
        compute_local_port_formulas(&vis.transform, page_x_in, page_y_in);
    materialize_inherited_connection_points_if_needed(doc, page_id, shape_id);
    if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
        if let Some(cp) = sh.sheet.connection_points.iter_mut().find(|c| c.ix == ix) {
            cp.x = x_expr;
            cp.y = y_expr;
            cp.dir_x = dx_s;
            cp.dir_y = dy_s;
        }
        sh.sheet.sync_all_sections_to_cells();
    }
    ReactiveEngine::recalculate_document(doc);
    route_page_connectors(doc, page_id);
}

fn build_master_template_from_shapes(
    doc: &VisioDocument,
    page_id: u32,
    shapes: &[VisioShape],
) -> VisioShape {
    if shapes.len() == 1 {
        let mut s = shapes[0].clone();
        s.id = 1;
        s.master_id = None;
        s.bound_data_row = None;
        let w = s.sheet.get_f64("Width").unwrap_or(1.5).abs().max(0.15);
        let h = s.sheet.get_f64("Height").unwrap_or(1.0).abs().max(0.15);
        s.sheet.set_formula("Width", format!("{} in", format_clean_f64(w)), Some(Unit::Inch));
        s.sheet.set_formula("Height", format!("{} in", format_clean_f64(h)), Some(Unit::Inch));
        s.sheet.set_formula("LocPinX", "=Width*0.5", Some(Unit::Inch));
        s.sheet.set_formula("LocPinY", "=Height*0.5", Some(Unit::Inch));
        s.sheet.set_formula("PinX", format!("{} in", format_clean_f64(w * 0.5)), Some(Unit::Inch));
        s.sheet.set_formula("PinY", format!("{} in", format_clean_f64(h * 0.5)), Some(Unit::Inch));
        attach_cardinal_ports_to_shape(&mut s);
        s.sheet.sync_all_sections_to_cells();
        return s;
    }

    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for sh in shapes {
        if let Some(v) = visuals.iter().find(|vv| vv.shape_id == sh.id) {
            min_x = min_x.min(v.aabb_min[0]);
            min_y = min_y.min(v.aabb_min[1]);
            max_x = max_x.max(v.aabb_max[0]);
            max_y = max_y.max(v.aabb_max[1]);
        }
    }
    if !min_x.is_finite() {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 2.0;
        max_y = 1.5;
    }
    let gw = (max_x - min_x).max(0.25);
    let gh = (max_y - min_y).max(0.25);
    let mut grp = VisioShape::new_rect(1, "CustomGroup", gw * 0.5, gh * 0.5, gw, gh);
    grp.kind = ShapeKind::Group;
    grp.text.clear();
    grp.sheet.geometries.clear();
    grp.sheet.set_formula("FillPattern", "0", Some(Unit::Number));
    grp.sheet.set_formula("LinePattern", "0", Some(Unit::Number));
    let mut next_cid = 2u64;
    for orig in shapes {
        let mut ch = orig.clone();
        ch.master_id = None;
        ch.bound_data_row = None;
        let px = ch.sheet.get_f64("PinX").unwrap_or(min_x) - min_x;
        let py = ch.sheet.get_f64("PinY").unwrap_or(min_y) - min_y;
        ch.sheet.set_formula("PinX", format!("{} in", format_clean_f64(px)), Some(Unit::Inch));
        ch.sheet.set_formula("PinY", format!("{} in", format_clean_f64(py)), Some(Unit::Inch));
        let _ = remap_dropped_shape_subtree(&mut ch, &mut next_cid);
        grp.children.push(ch);
    }
    attach_cardinal_ports_to_shape(&mut grp);
    grp.sheet.sync_all_sections_to_cells();
    grp
}

fn propagate_master_template_to_instance(tpl: &VisioShape, inst: &mut VisioShape) {
    let keep_px = inst.sheet.get_cell("PinX").map(|c| c.formula.clone());
    let keep_py = inst.sheet.get_cell("PinY").map(|c| c.formula.clone());
    let keep_w = inst.sheet.get_cell("Width").map(|c| c.formula.clone());
    let keep_h = inst.sheet.get_cell("Height").map(|c| c.formula.clone());
    let keep_ang = inst.sheet.get_cell("Angle").map(|c| c.formula.clone());
    let keep_props = inst.sheet.shape_data.clone();

    inst.kind = tpl.kind;
    inst.foreign_data = tpl.foreign_data.clone();
    inst.sheet.geometries = tpl.sheet.geometries.clone();
    inst.sheet.actions = tpl.sheet.actions.clone();
    inst.sheet.connection_points = tpl.sheet.connection_points.clone();
    inst.sheet.controls = tpl.sheet.controls.clone();
    for (uk, uv) in &tpl.sheet.user_cells {
        inst.sheet.user_cells.insert(uk.clone(), uv.clone());
    }
    for (pk, pv) in &tpl.sheet.shape_data {
        inst.sheet
            .shape_data
            .entry(pk.clone())
            .or_insert_with(|| pv.clone());
    }
    for (pk, pv) in keep_props {
        inst.sheet.shape_data.insert(pk, pv);
    }

    for style_cell in [
        "FillForegnd",
        "FillBkgnd",
        "FillPattern",
        "FillForegndTrans",
        "LineColor",
        "LineWeight",
        "LinePattern",
        "LineColorTrans",
        "Rounding",
        "Char.Font",
        "Char.Size",
        "Char.Style",
        "Char.Color",
    ] {
        if let Some(tc) = tpl.sheet.get_cell(style_cell) {
            let is_guarded = inst
                .sheet
                .get_cell(style_cell)
                .is_some_and(|c| c.is_guarded());
            if !is_guarded {
                inst.sheet
                    .set_formula(style_cell, tc.formula.clone(), tc.unit_hint);
            }
        }
    }

    if !tpl.children.is_empty() {
        let mut alloc = inst.id.saturating_mul(1000).max(10_000);
        inst.children = tpl
            .children
            .iter()
            .map(|c| {
                let mut cloned = c.clone();
                let _ = remap_dropped_shape_subtree(&mut cloned, &mut alloc);
                cloned
            })
            .collect();
    }

    if let Some(f) = keep_px {
        inst.sheet.set_formula("PinX", f, Some(Unit::Inch));
    }
    if let Some(f) = keep_py {
        inst.sheet.set_formula("PinY", f, Some(Unit::Inch));
    }
    if let Some(f) = keep_w {
        inst.sheet.set_formula("Width", f, Some(Unit::Inch));
    }
    if let Some(f) = keep_h {
        inst.sheet.set_formula("Height", f, Some(Unit::Inch));
    }
    if let Some(f) = keep_ang {
        inst.sheet.set_formula("Angle", f, Some(Unit::Degree));
    }
    inst.sheet.sync_all_sections_to_cells();
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeDataItemDto {
    pub name: String,
    pub label: String,
    pub prompt: String,
    pub data_type: u8,
    pub format: String,
    pub formula: String,
    pub value: String,
}

pub(crate) fn collect_shape_data_items(
    doc: &VisioDocument,
    shape: &VisioShape,
) -> Vec<ShapeDataItemDto> {
    let mut merged: std::collections::BTreeMap<String, vexio_shapesheet::ShapeDataRow> =
        std::collections::BTreeMap::new();
    if let Some(mid) = shape.master_id {
        if let Some(master) = doc.masters.iter().find(|m| m.id == mid) {
            if let Some(ms) = master.shapes.first() {
                for (k, row) in &ms.sheet.shape_data {
                    if !row.invisible {
                        merged.insert(k.clone(), row.clone());
                    }
                }
            }
        }
    }
    for (k, row) in &shape.sheet.shape_data {
        if row.invisible {
            merged.remove(k);
        } else {
            merged.insert(k.clone(), row.clone());
        }
    }
    merged
        .into_values()
        .map(|row| {
            let cell_key = format!("Prop.{}", row.name);
            let evaluated = shape
                .sheet
                .get_cell(&cell_key)
                .and_then(|c| c.value.as_str_coerced().ok())
                .unwrap_or_else(|| row.value.trim_matches('"').to_string());
            ShapeDataItemDto {
                name: row.name,
                label: row.label,
                prompt: row.prompt,
                data_type: row.data_type as u8,
                format: row.format,
                formula: row.value,
                value: evaluated,
            }
        })
        .collect()
}

pub(crate) fn resolve_shape_master_name(doc: &VisioDocument, shape: &VisioShape) -> Option<String> {
    let mid = shape.master_id?;
    doc.masters
        .iter()
        .find(|m| m.id == mid)
        .map(|m| {
            if !m.name.trim().is_empty() {
                m.name.clone()
            } else {
                m.name_u.clone()
            }
        })
        .filter(|n| !n.is_empty())
}

pub(crate) fn resolve_inspector_fill_hex(shape: &VisioShape) -> String {
    let fp = shape.sheet.get_f64("FillPattern").unwrap_or(1.0).round() as i32;
    if fp != 0 {
        if let Some(c) = shape.sheet.get_color("FillForegnd") {
            return c.to_hex_rgb();
        }
    }
    for child in &shape.children {
        let cfp = child.sheet.get_f64("FillPattern").unwrap_or(1.0).round() as i32;
        if cfp != 0 {
            if let Some(c) = child.sheet.get_color("FillForegnd") {
                return c.to_hex_rgb();
            }
        }
    }
    shape
        .sheet
        .get_color("FillForegnd")
        .map(|c| c.to_hex_rgb())
        .unwrap_or_else(|| "#DBEAFE".to_string())
}

pub(crate) fn resolve_inspector_line_hex(shape: &VisioShape) -> String {
    let lp = shape.sheet.get_f64("LinePattern").unwrap_or(1.0).round() as i32;
    if lp != 0 {
        if let Some(c) = shape.sheet.get_color("LineColor") {
            return c.to_hex_rgb();
        }
    }
    for child in &shape.children {
        let clp = child.sheet.get_f64("LinePattern").unwrap_or(1.0).round() as i32;
        if clp != 0 {
            if let Some(c) = child.sheet.get_color("LineColor") {
                return c.to_hex_rgb();
            }
        }
    }
    shape
        .sheet
        .get_color("LineColor")
        .map(|c| c.to_hex_rgb())
        .unwrap_or_else(|| "#1E3A8A".to_string())
}

pub(crate) fn count_layer_members_on_page(page: &VisioPage, layer_ix: u32) -> usize {
    fn count_in_shape(s: &VisioShape, ix: u32) -> usize {
        let here = usize::from(s.layer_membership.contains(&ix));
        here + s.children.iter().map(|c| count_in_shape(c, ix)).sum::<usize>()
    }
    page.shapes.iter().map(|s| count_in_shape(s, layer_ix)).sum()
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

#[cfg(test)]
mod headings_and_titles_tests {
    use super::*;

    #[test]
    fn visio_headings_titles_and_borders_roundtrip_and_undo() {
        let mut session = DocumentSession::new_blank("Architecture Overview");
        let page_id = session.doc.pages[0].id;

        // 1. Insert standalone Title, Subtitle, Heading 1, Section Banner, and Callout Header
        let title_sid = session
            .insert_text_heading(page_id, "title", Some("Cloud Architecture"), 0.0, 0.0)
            .expect("insert title");
        let subtitle_sid = session
            .insert_text_heading(page_id, "subtitle", Some("Q4 Production Blueprint"), 0.0, 0.0)
            .expect("insert subtitle");
        let h1_sid = session
            .insert_text_heading(page_id, "h1", Some("1. Core Services"), 3.0, 5.0)
            .expect("insert h1");
        let banner_sid = session
            .insert_text_heading(page_id, "section_banner", Some("DMZ Ingress"), 3.0, 4.0)
            .expect("insert banner");

        assert_ne!(title_sid, subtitle_sid);
        assert_ne!(h1_sid, banner_sid);

        // Verify Heading 1 has frameless fill + accent bottom underline geometry
        let h1_shape = session.doc.page(page_id).unwrap().find_shape(h1_sid).unwrap();
        assert_eq!(h1_shape.text, "1. Core Services");
        assert_eq!(h1_shape.sheet.get_f64("FillPattern").unwrap_or(1.0) as i32, 0);
        assert_eq!(h1_shape.sheet.get_f64("LinePattern").unwrap_or(0.0) as i32, 1);

        // 2. Apply typography preset onto existing shapes + Undo/Redo verification
        let updated = session
            .apply_text_heading_preset(page_id, &[subtitle_sid], "h1")
            .expect("apply h1 preset");
        assert_eq!(updated, 1);
        let sub_after = session.doc.page(page_id).unwrap().find_shape(subtitle_sid).unwrap();
        assert!((sub_after.sheet.get_f64("Char.Size").unwrap_or(0.0) - (18.0 / 72.0)).abs() < 1e-3);
        assert_eq!(sub_after.sheet.get_f64("Char.Style").unwrap_or(0.0) as i32, 1);

        assert!(session.journal.undo(&mut session.doc).expect("undo preset"));
        let sub_undone = session.doc.page(page_id).unwrap().find_shape(subtitle_sid).unwrap();
        assert!((sub_undone.sheet.get_f64("Char.Size").unwrap_or(0.0) - (14.0 / 72.0)).abs() < 1e-3);
        assert_eq!(sub_undone.sheet.get_f64("Char.Style").unwrap_or(1.0) as i32, 0);
        assert!(session.journal.redo(&mut session.doc).expect("redo preset"));

        // 3. Apply all Borders & Titles gallery variants + clean "none" removal
        for style in [
            "modern_banner",
            "executive_underline",
            "classic_frame",
            "pill_banner",
            "iso_engineering",
        ] {
            let bg_id = session
                .apply_border_and_title(
                    page_id,
                    style,
                    "Cloud Architecture",
                    "PRJ-2026",
                    "Platform Team",
                    "Rev 1.2",
                )
                .expect("apply border style");
            assert!(bg_id > 0, "style {style} must link background page");
            assert_eq!(session.doc.page(page_id).unwrap().back_page_id, Some(bg_id));
        }

        // Round-trip through lossless .vsdx bytes while background title page is linked
        let vsdx_bytes = session.save_format("vsdx").expect("save vsdx");
        let mut reloaded = DocumentSession::new_blank("Reloaded");
        reloaded.open_bytes(&vsdx_bytes, "vsdx").expect("reload vsdx");
        assert!(reloaded.doc.pages[0].back_page_id.is_some());

        // Detach via "none"
        let cleared = session
            .apply_border_and_title(page_id, "none", "", "", "", "")
            .expect("remove border and title");
        assert_eq!(cleared, 0);
        assert_eq!(session.doc.page(page_id).unwrap().back_page_id, None);
    }
}
