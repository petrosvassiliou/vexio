//! Unified `DocumentSession` orchestrating file I/O (`.vsdx`, `.vsdm`, `.vssx`, `.vstx`,
//! `.vsd`, `.vdx`, `.vdw`, `.dwg`, `.dxf`, `.pdf`, `.svg`, `.eps`, `.png`, `.html`),
//! transactions, routing, stencils, layouts, validation, and automation across all FFI targets.

use crate::autolayout::{LayoutAlgorithm, LayoutOptions, apply_auto_layout};
use crate::automation::{AutomationExecutionReport, SandboxLimits, execute_vba_macro};
use crate::domains::build_all_builtin_stencils;
use crate::export_print::{
    encode_rgba_to_png, export_document_html_report, export_document_pdf, export_page_eps,
    export_page_svg,
};
use crate::journal::{CommandJournal, DocCommand, JournalError};
use crate::routing::route_page_connectors;
use crate::validation::{DiagramRuleSet, ValidationIssue, validate_document};
use thiserror::Error;
use vexio_shapesheet::{
    GlueEndpoint, ReactiveEngine, StencilPack, VisioDocument, VisioPage, VisioShape,
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

pub struct DocumentSession {
    pub doc: VisioDocument,
    pub journal: CommandJournal,
    pub builtin_stencils: Vec<StencilPack>,
}

impl DocumentSession {
    pub fn new_blank(title: &str) -> Self {
        let mut doc = VisioDocument::new_blank(title);
        ReactiveEngine::recalculate_document(&mut doc);
        Self {
            doc,
            journal: CommandJournal::new(),
            builtin_stencils: build_all_builtin_stencils(),
        }
    }

    pub fn open_bytes(&mut self, bytes: &[u8], ext: &str) -> Result<(), CoreError> {
        let clean_ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        match clean_ext.as_str() {
            "vsdx" | "vsdm" | "vssx" | "vssm" | "vstx" | "vstm" => {
                let (doc, _) = read_vsdx(bytes)?;
                self.doc = doc;
            }
            "vsd" | "vss" | "vst" | "vdx" | "vdw" => {
                self.doc = vexio_vsd_legacy::read_legacy_auto(bytes, &clean_ext)?;
            }
            "dxf" => {
                let mut doc = VisioDocument::new_blank("CAD DXF Drawing");
                if let Some(page) = doc.page_mut(1) {
                    vexio_cad::import_dxf_to_page(page, bytes)?;
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
        let clean_ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
        let first_pid = self.doc.pages.first().map(|p| p.id).unwrap_or(1);
        match clean_ext.as_str() {
            "vsdx" | "vsdm" | "vssx" | "vssm" | "vstx" | "vstm" => {
                let kind = VsdxPackageKind::from_extension(&clean_ext);
                Ok(write_vsdx(&self.doc, kind)?)
            }
            "vsd" | "vss" | "vst" => Ok(vexio_vsd_legacy::write_vsd_cfb(&self.doc)?),
            "vdx" => Ok(vexio_vsd_legacy::write_vdx_xml(&self.doc).into_bytes()),
            "dxf" => Ok(vexio_cad::export_page_to_dxf(&self.doc, first_pid).into_bytes()),
            "dwg" => Ok(vexio_cad::encode_dwg_r2000_from_page(&self.doc, first_pid)?),
            "pdf" => Ok(export_document_pdf(&self.doc)),
            "svg" => Ok(export_page_svg(&self.doc, first_pid).into_bytes()),
            "eps" => Ok(export_page_eps(&self.doc, first_pid).into_bytes()),
            "html" => Ok(export_document_html_report(&self.doc).into_bytes()),
            "png" => {
                let rgba = vexio_render::render_page_rgba_auto(&self.doc, first_pid, 1056, 816)
                    .unwrap_or_else(|_| vec![255u8; 1056 * 816 * 4]);
                Ok(encode_rgba_to_png(1056, 816, &rgba, 300))
            }
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
            .and_then(|m| m.shapes.first())
            .cloned();

        let next_id = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);

        let mut shape = template.unwrap_or_else(|| {
            VisioShape::new_rect(next_id, master_name, x_in, y_in, 1.6, 0.9)
        });
        shape.id = next_id;
        shape.name = format!("{master_name}.{next_id}");
        shape.name_u = shape.name.clone();
        if shape.text.is_empty() {
            shape.text = master_name.to_string();
        }
        shape.sheet.set_formula(
            "PinX",
            format!("{} in", vexio_shapesheet::format_clean_f64(x_in)),
            Some(vexio_shapesheet::Unit::Inch),
        );
        shape.sheet.set_formula(
            "PinY",
            format!("{} in", vexio_shapesheet::format_clean_f64(y_in)),
            Some(vexio_shapesheet::Unit::Inch),
        );

        self.journal
            .execute(&mut self.doc, DocCommand::AddShape { page_id, shape })?;
        Ok(next_id)
    }

    pub fn connect_shapes(
        &mut self,
        page_id: u32,
        from_id: u64,
        to_id: u64,
    ) -> Result<u64, CoreError> {
        let cid = self
            .doc
            .page(page_id)
            .map(|p| p.next_shape_id())
            .unwrap_or(1);
        let conn = VisioShape::new_connector(cid, format!("Dynamic connector.{cid}"), 0.0, 0.0, 1.0, 0.0);
        let batch = DocCommand::Batch {
            label: "Connect Shapes".into(),
            commands: vec![
                DocCommand::AddShape {
                    page_id,
                    shape: conn,
                },
                DocCommand::AddConnect {
                    page_id,
                    connect: GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "BeginX".into(),
                        target_shape_id: from_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                },
                DocCommand::AddConnect {
                    page_id,
                    connect: GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "EndX".into(),
                        target_shape_id: to_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    },
                },
            ],
        };
        self.journal.execute(&mut self.doc, batch)?;
        route_page_connectors(&mut self.doc, page_id);
        Ok(cid)
    }

    pub fn set_cell_formula(
        &mut self,
        page_id: u32,
        shape_id: u64,
        cell_name: &str,
        new_formula: &str,
        force_setf: bool,
    ) -> Result<(), CoreError> {
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

    pub fn add_page(&mut self, name: &str) -> Result<u32, CoreError> {
        let next_id = self.doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let page = VisioPage::new(next_id, name, 11.0, 8.5);
        self.journal
            .execute(&mut self.doc, DocCommand::AddPage { page })?;
        Ok(next_id)
    }

    pub fn layout(&mut self, page_id: u32, key: &str) {
        let algorithm = match key.to_ascii_lowercase().as_str() {
            "lr" | "lefttoright" => LayoutAlgorithm::HierarchicalLeftToRight,
            "bt" | "bottomtotop" => LayoutAlgorithm::HierarchicalBottomToTop,
            "rl" | "righttoleft" => LayoutAlgorithm::HierarchicalRightToLeft,
            "radial" => LayoutAlgorithm::Radial,
            "tree" | "compacttree" => LayoutAlgorithm::CompactTree,
            "circular" => LayoutAlgorithm::Circular,
            _ => LayoutAlgorithm::HierarchicalTopToBottom,
        };
        apply_auto_layout(
            &mut self.doc,
            page_id,
            LayoutOptions {
                algorithm,
                ..LayoutOptions::default()
            },
        );
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
        Ok(execute_vba_macro(
            &mut self.doc,
            script,
            SandboxLimits::default(),
        )?)
    }
}
