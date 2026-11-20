//! Visio Plan 2 Enterprise Capabilities:
//! - Subprocess Drill-Down Pages, Multi-Hyperlinks & ScreenTips
//! - ISO 7200 / ANSI Engineering Title-Block Background Pages & Header/Footer Designer
//! - Presentation Mode `SlideSnippetFrame`s + OpenXML `.pptx` PowerPoint Exporter
//! - OpenXML `.docx` Word Process Specification & Inventory Exporter
//! - Visual `.vsdx` Diagram Compare (`compare_documents`) with Color-Coded SVG Diff Overlay.

use crate::export_print::{encode_rgba_to_png, export_page_svg};
use crate::routing::route_page_connectors;
use crate::validation::{DiagramRuleSet, validate_document};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::{Cursor, Write};
use vexio_render::{ViewportCamera, render_page_rgba_auto, render_page_software_rgba_with_camera};
use vexio_shapesheet::{
    GeometryRow, GeometryRowKind, GeometrySection, GlueEndpoint, HyperlinkRow, ReactiveEngine,
    ShapeDataRow, ShapeDataType, ShapeKind, Unit, VisioDocument, VisioPage, VisioShape,
    format_clean_f64,
};
use vexio_vsdx::opc::xml_escape;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

// ============================================================================
// 1. Subprocess Drill-Down, Multi-Hyperlinks & ScreenTips
// ============================================================================

/// Create (or link an existing) drill-down Subprocess `VisioPage` for `task_shape_id` on `parent_page_id`.
///
/// - Populates a newly created subprocess page with `Start Event -> Step -> End Event` plus a
///   `"← Return to Parent Page"` breadcrumb shape carrying a default `HyperlinkRow` back to `parent_page_id`.
/// - Marks `task_shape_id` with `User.IsSubprocess = TRUE`, a visual `[+]` subprocess badge child shape,
///   and a default `HyperlinkRow` (`sub_address = subprocess_page_name`).
pub fn create_or_link_subprocess_page(
    doc: &mut VisioDocument,
    parent_page_id: u32,
    task_shape_id: u64,
    subprocess_name: &str,
) -> Option<u32> {
    let parent_page = doc.page(parent_page_id)?;
    parent_page.find_shape(task_shape_id)?;
    let parent_page_name = parent_page.name.clone();
    let clean_sub_name = if subprocess_name.trim().is_empty() {
        format!("Subprocess-{task_shape_id}")
    } else {
        subprocess_name.trim().to_string()
    };

    let (sub_page_id, actual_sub_name, created_new) =
        if let Some(existing) = doc.page_by_name(&clean_sub_name) {
            (existing.id, existing.name.clone(), false)
        } else {
            let next_pid = doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
            let page = VisioPage::new(next_pid, clean_sub_name.clone(), 11.0, 8.5);
            doc.pages.push(page);
            (next_pid, clean_sub_name, true)
        };

    if created_new {
        if let Some(sub_page) = doc.page_mut(sub_page_id) {
            // 1. Breadcrumb navigation shape linking back to parent page
            let bc_id = sub_page.next_shape_id();
            let mut breadcrumb =
                VisioShape::new_rect(bc_id, "ReturnBreadcrumb", 2.0, 7.85, 2.7, 0.45);
            breadcrumb.text = "← Return to Parent Page".into();
            breadcrumb
                .sheet
                .set_formula("Rounding", "0.12 in", Some(Unit::Inch));
            breadcrumb
                .sheet
                .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
            breadcrumb
                .sheet
                .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
            breadcrumb
                .sheet
                .set_formula("Char.Color", "=RGB(29,78,216)", Some(Unit::Color));
            breadcrumb.sheet.set_user_cell(
                "DomainRole",
                "\"container\"",
                "Navigation Breadcrumb Chrome",
            );
            breadcrumb.sheet.set_user_cell(
                "msvStructureType",
                "\"Container\"",
                "Navigation Chrome",
            );
            breadcrumb.sheet.set_user_cell(
                "ParentPageId",
                parent_page_id.to_string(),
                "Parent Page ID",
            );
            push_hyperlink_row(
                &mut breadcrumb,
                &format!("Return to {parent_page_name}"),
                "",
                &parent_page_name,
                true,
            );
            sub_page.shapes.push(breadcrumb);

            // 2. BPMN Start Event (circle)
            let start_id = sub_page.next_shape_id();
            let mut start_ev = VisioShape::new_rect(start_id, "Start Event", 2.2, 4.25, 0.6, 0.6);
            start_ev.text = "Start".into();
            start_ev.sheet.add_geometry(circle_geometry(1));
            start_ev
                .sheet
                .set_formula("FillForegnd", "=RGB(220,252,231)", Some(Unit::Color));
            start_ev
                .sheet
                .set_formula("LineColor", "=RGB(22,163,74)", Some(Unit::Color));
            start_ev
                .sheet
                .set_user_cell("DomainRole", "\"start\"", "BPMN Start Event");
            sub_page.shapes.push(start_ev);

            // 3. BPMN Subprocess Step Task
            let step_id = sub_page.next_shape_id();
            let mut step_task = VisioShape::new_rect(step_id, "Step.1", 5.5, 4.25, 2.0, 1.0);
            step_task.text = format!("{actual_sub_name} — Step 1");
            step_task
                .sheet
                .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
            step_task
                .sheet
                .set_formula("FillForegnd", "=RGB(248,250,252)", Some(Unit::Color));
            step_task
                .sheet
                .set_user_cell("DomainRole", "\"task\"", "BPMN Task");
            sub_page.shapes.push(step_task);

            // 4. BPMN End Event (thick-bordered circle)
            let end_id = sub_page.next_shape_id();
            let mut end_ev = VisioShape::new_rect(end_id, "End Event", 8.8, 4.25, 0.6, 0.6);
            end_ev.text = "End".into();
            end_ev.sheet.add_geometry(circle_geometry(1));
            end_ev
                .sheet
                .set_formula("FillForegnd", "=RGB(254,226,226)", Some(Unit::Color));
            end_ev
                .sheet
                .set_formula("LineColor", "=RGB(220,38,38)", Some(Unit::Color));
            end_ev
                .sheet
                .set_formula("LineWeight", "2.25 pt", Some(Unit::Point));
            end_ev
                .sheet
                .set_user_cell("DomainRole", "\"end\"", "BPMN End Event");
            sub_page.shapes.push(end_ev);

            // 5. Sequence Flows: Start -> Step -> End
            let c1_id = sub_page.next_shape_id();
            let c1 = VisioShape::new_connector(c1_id, "SequenceFlow.1", 2.5, 4.25, 4.5, 4.25);
            sub_page.shapes.push(c1);
            let c2_id = sub_page.next_shape_id();
            let c2 = VisioShape::new_connector(c2_id, "SequenceFlow.2", 6.5, 4.25, 8.5, 4.25);
            sub_page.shapes.push(c2);

            sub_page.connects.extend([
                GlueEndpoint {
                    connector_shape_id: c1_id,
                    from_cell: "BeginX".into(),
                    target_shape_id: start_id,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
                GlueEndpoint {
                    connector_shape_id: c1_id,
                    from_cell: "EndX".into(),
                    target_shape_id: step_id,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
                GlueEndpoint {
                    connector_shape_id: c2_id,
                    from_cell: "BeginX".into(),
                    target_shape_id: step_id,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
                GlueEndpoint {
                    connector_shape_id: c2_id,
                    from_cell: "EndX".into(),
                    target_shape_id: end_id,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                },
            ]);
        }
        route_page_connectors(doc, sub_page_id);
    }

    // Attach default drill-down HyperlinkRow + `User.IsSubprocess = TRUE` + `[+]` badge onto `task_shape_id`
    if let Some(parent) = doc.page_mut(parent_page_id) {
        let badge_id = parent.next_shape_id() + 20_000;
        if let Some(task) = parent.find_shape_mut(task_shape_id) {
            task.sheet
                .set_user_cell("IsSubprocess", "TRUE", "Collapsed Subprocess Marker");
            task.sheet.set_user_cell(
                "SubprocessPage",
                format!("\"{}\"", actual_sub_name.replace('"', "\"\"")),
                "Target Subprocess Page",
            );
            push_hyperlink_row(
                task,
                &format!("Drill down to {actual_sub_name}"),
                "",
                &actual_sub_name,
                true,
            );

            if !task
                .children
                .iter()
                .any(|c| c.name_u.starts_with("SubprocessBadge"))
            {
                let tw = task.sheet.get_f64("Width").unwrap_or(1.6);
                let mut badge =
                    VisioShape::new_rect(badge_id, "SubprocessBadge", tw * 0.5, 0.14, 0.22, 0.22);
                badge.text = "+".into();
                badge
                    .sheet
                    .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
                badge
                    .sheet
                    .set_formula("LineColor", "=RGB(37,99,235)", Some(Unit::Color));
                task.children.push(badge);
            }
        }
    }

    ReactiveEngine::recalculate_document(doc);
    Some(sub_page_id)
}

/// Add a `HyperlinkRow` (supporting multiple links per shape) and synchronize `Hyperlink.*` cells.
pub fn add_shape_hyperlink(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    description: &str,
    address: &str,
    sub_address: &str,
    is_default: bool,
) -> bool {
    let Some(shape) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) else {
        return false;
    };
    push_hyperlink_row(shape, description, address, sub_address, is_default);
    ReactiveEngine::recalculate_document(doc);
    true
}

/// Store a Visio ScreenTip on `shape_id` in both Miscellaneous `Comment` and `User.ScreenTip`.
pub fn set_shape_screentip(
    doc: &mut VisioDocument,
    page_id: u32,
    shape_id: u64,
    screentip: &str,
) -> bool {
    let Some(shape) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) else {
        return false;
    };
    let quoted = format!("\"{}\"", screentip.replace('"', "\"\""));
    shape
        .sheet
        .set_formula("Comment", quoted.clone(), Some(Unit::String));
    shape
        .sheet
        .set_user_cell("ScreenTip", quoted, "Shape ScreenTip");
    ReactiveEngine::recalculate_document(doc);
    true
}

fn push_hyperlink_row(
    shape: &mut VisioShape,
    description: &str,
    address: &str,
    sub_address: &str,
    is_default: bool,
) {
    if is_default {
        for existing in &mut shape.sheet.hyperlinks {
            existing.default = false;
        }
    }
    let idx = shape.sheet.hyperlinks.len() + 1;
    let row_name = format!("Row_{idx}");
    shape.sheet.hyperlinks.push(HyperlinkRow {
        name: row_name.clone(),
        description: description.to_string(),
        address: address.to_string(),
        sub_address: sub_address.to_string(),
        new_window: false,
        default: is_default,
    });
    shape.sheet.set_formula(
        format!("Hyperlink.{row_name}.Description"),
        format!("\"{}\"", description.replace('"', "\"\"")),
        Some(Unit::String),
    );
    shape.sheet.set_formula(
        format!("Hyperlink.{row_name}.Address"),
        format!("\"{}\"", address.replace('"', "\"\"")),
        Some(Unit::String),
    );
    shape.sheet.set_formula(
        format!("Hyperlink.{row_name}.SubAddress"),
        format!("\"{}\"", sub_address.replace('"', "\"\"")),
        Some(Unit::String),
    );
    shape.sheet.set_formula(
        format!("Hyperlink.{row_name}.Default"),
        if is_default { "TRUE" } else { "FALSE" },
        Some(Unit::Boolean),
    );
}

fn circle_geometry(index: u32) -> GeometrySection {
    let mut g = GeometrySection::new(index);
    g.rows = vec![GeometryRow {
        ix: 1,
        kind: GeometryRowKind::Ellipse,
        x: "=Width*0.5".into(),
        y: "=Height*0.5".into(),
        a: Some("=Width*1".into()),
        b: Some("=Height*0.5".into()),
        c: Some("=Width*0.5".into()),
        d: Some("=Height*1".into()),
        e: None,
        deleted: false,
    }];
    g
}

// ============================================================================
// 2. Background Title-Block Page & Header/Footer Border Designer
// ============================================================================

/// Create or update an ISO 7200 / ANSI Y14.1 engineering border and bottom-right title-block
/// background page (`is_background = true`), assign `target_page.back_page_id = Some(bg_id)`,
/// and populate `doc.header_footer`.
pub fn apply_iso_title_block_background(
    doc: &mut VisioDocument,
    target_page_id: u32,
    project_code: &str,
    author: &str,
    revision: &str,
) -> Option<u32> {
    let (w_in, h_in, target_page_name) = {
        let tp = doc.page(target_page_id)?;
        (tp.width_in(), tp.height_in(), tp.name.clone())
    };
    let doc_title = if doc.properties.title.trim().is_empty() {
        "Controlled Engineering Diagram".to_string()
    } else {
        doc.properties.title.clone()
    };

    let bg_id = if let Some(existing_bg) = doc
        .pages
        .iter()
        .find(|p| p.is_background && p.name.eq_ignore_ascii_case("ISO Title Block"))
    {
        existing_bg.id
    } else {
        let next_id = doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
        let mut bg = VisioPage::new(next_id, "ISO Title Block", w_in, h_in);
        bg.is_background = true;
        doc.pages.push(bg);
        next_id
    };

    if let Some(bg_page) = doc.page_mut(bg_id) {
        bg_page.is_background = true;
        bg_page.page_sheet.set_formula(
            "PageWidth",
            format!("{} in", format_clean_f64(w_in)),
            Some(Unit::Inch),
        );
        bg_page.page_sheet.set_formula(
            "PageHeight",
            format!("{} in", format_clean_f64(h_in)),
            Some(Unit::Inch),
        );
        bg_page.shapes.clear();

        // Outer sheet frame
        let margin = 0.38;
        let frame_w = (w_in - margin * 2.0).max(2.0);
        let frame_h = (h_in - margin * 2.0).max(2.0);
        let mut outer = VisioShape::new_rect(1, "EngineeringBorderOuter", w_in * 0.5, h_in * 0.5, frame_w, frame_h);
        if let Some(g) = outer.sheet.geometries.first_mut() {
            g.no_fill = "TRUE".into();
        }
        outer.sheet.sync_all_sections_to_cells();
        outer
            .sheet
            .set_formula("LineWeight", "1.5 pt", Some(Unit::Point));
        outer
            .sheet
            .set_formula("LineColor", "=RGB(15,23,42)", Some(Unit::Color));
        bg_page.shapes.push(outer);

        // Inner zone border
        let inner_margin = 0.48;
        let inner_w = (w_in - inner_margin * 2.0).max(1.8);
        let inner_h = (h_in - inner_margin * 2.0).max(1.8);
        let mut inner =
            VisioShape::new_rect(2, "EngineeringBorderInner", w_in * 0.5, h_in * 0.5, inner_w, inner_h);
        if let Some(g) = inner.sheet.geometries.first_mut() {
            g.no_fill = "TRUE".into();
        }
        inner.sheet.sync_all_sections_to_cells();
        inner
            .sheet
            .set_formula("LineWeight", "0.75 pt", Some(Unit::Point));
        inner
            .sheet
            .set_formula("LineColor", "=RGB(71,85,105)", Some(Unit::Color));
        bg_page.shapes.push(inner);

        // Bottom-right ISO 7200 / ANSI Title Block grid
        let tb_w = 3.8_f64.min(inner_w * 0.85);
        let tb_h = 1.2_f64.min(inner_h * 0.45);
        let right_x = w_in - inner_margin;
        let bottom_y = inner_margin;

        let mut tb_header = VisioShape::new_rect(
            3,
            "TitleBlockHeader",
            right_x - tb_w * 0.5,
            bottom_y + tb_h - 0.22,
            tb_w,
            0.44,
        );
        tb_header.text = format!("{doc_title} — [{project_code}]");
        tb_header
            .sheet
            .set_formula("FillForegnd", "=RGB(15,23,42)", Some(Unit::Color));
        tb_header
            .sheet
            .set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        for (k, v) in [
            ("ProjectCode", project_code),
            ("Author", author),
            ("Revision", revision),
        ] {
            tb_header.sheet.set_shape_data(ShapeDataRow {
                name: k.into(),
                label: k.into(),
                prompt: format!("ISO Title Block {k}"),
                data_type: ShapeDataType::String,
                format: String::new(),
                value: format!("\"{}\"", v.replace('"', "\"\"")),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
        }
        bg_page.shapes.push(tb_header);

        let body_h = (tb_h - 0.44).max(0.4);
        let mut cell_left = VisioShape::new_rect(
            4,
            "TitleBlockAuthorCell",
            right_x - tb_w * 0.75,
            bottom_y + body_h * 0.5,
            tb_w * 0.5,
            body_h,
        );
        cell_left.text = format!("PROJECT: {project_code}\nDRAWN BY: {author}");
        cell_left
            .sheet
            .set_formula("FillForegnd", "=RGB(248,250,252)", Some(Unit::Color));
        bg_page.shapes.push(cell_left);

        let mut cell_right = VisioShape::new_rect(
            5,
            "TitleBlockRevisionCell",
            right_x - tb_w * 0.25,
            bottom_y + body_h * 0.5,
            tb_w * 0.5,
            body_h,
        );
        cell_right.text = format!("REV: {revision}\nSTD: ISO 7200 / ANSI Y14.1");
        cell_right
            .sheet
            .set_formula("FillForegnd", "=RGB(241,245,249)", Some(Unit::Color));
        bg_page.shapes.push(cell_right);
    }

    if let Some(target_page) = doc.page_mut(target_page_id) {
        target_page.back_page_id = Some(bg_id);
    }

    doc.header_footer.header_left = format!("Project: {project_code}");
    doc.header_footer.header_center = doc_title;
    doc.header_footer.header_right = format!("Rev {revision}");
    doc.header_footer.footer_left = format!("Author: {author}");
    doc.header_footer.footer_center = "ISO 7200 / ANSI Controlled Drawing".into();
    doc.header_footer.footer_right = format!("Sheet: {target_page_name}");
    if doc.header_footer.font_name.is_empty() {
        doc.header_footer.font_name = "Segoe UI".into();
    }
    if doc.header_footer.font_size_pt <= 0.0 {
        doc.header_footer.font_size_pt = 9.0;
    }

    ReactiveEngine::recalculate_document(doc);
    Some(bg_id)
}

// ============================================================================
// 3. Presentation `SlideSnippetFrame` + OpenXML `.pptx` & `.docx` Exporters
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SlideSnippetFrame {
    pub id: u32,
    pub page_id: u32,
    pub title: String,
    pub notes: String,
    pub x_in: f64,
    pub y_in: f64,
    pub width_in: f64,
    pub height_in: f64,
}

/// Export `snippets` (or one slide per foreground `VisioPage` when `snippets` is empty) into a
/// standards-compliant OpenXML PresentationML `.pptx` package containing high-DPI rendered
/// diagram PNG media, title text boxes, caption callouts, and speaker notes slides.
pub fn export_presentation_pptx(
    doc: &VisioDocument,
    snippets: &[SlideSnippetFrame],
) -> Result<Vec<u8>, std::io::Error> {
    let auto_snippets: Vec<SlideSnippetFrame>;
    let active_snippets: &[SlideSnippetFrame] = if snippets.is_empty() {
        let mut generated = Vec::new();
        for (idx, p) in doc.pages.iter().filter(|p| !p.is_background).enumerate() {
            generated.push(SlideSnippetFrame {
                id: (idx + 1) as u32,
                page_id: p.id,
                title: format!("{} — {}", doc.properties.title, p.name),
                notes: format!(
                    "Page '{}' ({} shapes, {} connections).",
                    p.name,
                    p.shapes.len(),
                    p.connects.len()
                ),
                x_in: 0.0,
                y_in: 0.0,
                width_in: p.width_in(),
                height_in: p.height_in(),
            });
        }
        if generated.is_empty() {
            generated.push(SlideSnippetFrame {
                id: 1,
                page_id: 1,
                title: doc.properties.title.clone(),
                notes: "Empty Visio presentation deck.".into(),
                x_in: 0.0,
                y_in: 0.0,
                width_in: 11.0,
                height_in: 8.5,
            });
        }
        auto_snippets = generated;
        &auto_snippets
    } else {
        snippets
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zw = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // 1. [Content_Types].xml
        let mut ct = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
             <Default Extension=\"png\" ContentType=\"image/png\"/>\
             <Override PartName=\"/ppt/presentation.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml\"/>\
             <Override PartName=\"/ppt/slideMasters/slideMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml\"/>\
             <Override PartName=\"/ppt/slideLayouts/slideLayout1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml\"/>\
             <Override PartName=\"/ppt/theme/theme1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>",
        );
        for i in 1..=active_snippets.len() {
            let _ = write!(
                ct,
                "<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>\
                 <Override PartName=\"/ppt/notesSlides/notesSlide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml\"/>"
            );
        }
        ct.push_str("</Types>");
        write_zip_part(&mut zw, "[Content_Types].xml", ct.as_bytes(), opts)?;

        // 2. _rels/.rels
        write_zip_part(
            &mut zw,
            "_rels/.rels",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
              <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"ppt/presentation.xml\"/>\
              </Relationships>",
            opts,
        )?;

        // 3. ppt/presentation.xml & ppt/_rels/presentation.xml.rels
        let mut pres_xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <p:presentation xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                             xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
                             xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">\
             <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
             <p:sldIdLst>",
        );
        let mut pres_rels = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
             <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"slideMasters/slideMaster1.xml\"/>\
             <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"theme/theme1.xml\"/>",
        );
        for i in 1..=active_snippets.len() {
            let rid = i + 2;
            let sld_id = 255 + i;
            let _ = write!(pres_xml, "<p:sldId id=\"{sld_id}\" r:id=\"rId{rid}\"/>");
            let _ = write!(
                pres_rels,
                "<Relationship Id=\"rId{rid}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"slides/slide{i}.xml\"/>"
            );
        }
        pres_xml.push_str(
            "</p:sldIdLst>\
             <p:sldSz cx=\"12192000\" cy=\"6858000\"/>\
             <p:notesSz cx=\"6858000\" cy=\"9144000\"/>\
             </p:presentation>",
        );
        pres_rels.push_str("</Relationships>");
        write_zip_part(&mut zw, "ppt/presentation.xml", pres_xml.as_bytes(), opts)?;
        write_zip_part(
            &mut zw,
            "ppt/_rels/presentation.xml.rels",
            pres_rels.as_bytes(),
            opts,
        )?;

        // 4. Slide Master, Slide Layout & Theme
        write_zip_part(
            &mut zw,
            "ppt/slideMasters/slideMaster1.xml",
            PPTX_SLIDE_MASTER_XML.as_bytes(),
            opts,
        )?;
        write_zip_part(
            &mut zw,
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
              <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>\
              <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme\" Target=\"../theme/theme1.xml\"/>\
              </Relationships>",
            opts,
        )?;
        write_zip_part(
            &mut zw,
            "ppt/slideLayouts/slideLayout1.xml",
            PPTX_SLIDE_LAYOUT_XML.as_bytes(),
            opts,
        )?;
        write_zip_part(
            &mut zw,
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
              <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster\" Target=\"../slideMasters/slideMaster1.xml\"/>\
              </Relationships>",
            opts,
        )?;
        write_zip_part(&mut zw, "ppt/theme/theme1.xml", PPTX_THEME_XML.as_bytes(), opts)?;

        // 5. Render each slide's PNG + Slide XML + NotesSlide XML
        for (idx, snip) in active_snippets.iter().enumerate() {
            let slide_num = idx + 1;
            let png_bytes = render_snippet_png(doc, snip);
            write_zip_part(
                &mut zw,
                &format!("ppt/media/slide{slide_num}.png"),
                &png_bytes,
                opts,
            )?;

            let slide_xml = build_pptx_slide_xml(&snip.title, &snip.notes);
            write_zip_part(
                &mut zw,
                &format!("ppt/slides/slide{slide_num}.xml"),
                slide_xml.as_bytes(),
                opts,
            )?;

            let slide_rels = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
                 <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
                 <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout\" Target=\"../slideLayouts/slideLayout1.xml\"/>\
                 <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"../media/slide{slide_num}.png\"/>\
                 <Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/notesSlide\" Target=\"../notesSlides/notesSlide{slide_num}.xml\"/>\
                 </Relationships>"
            );
            write_zip_part(
                &mut zw,
                &format!("ppt/slides/_rels/slide{slide_num}.xml.rels"),
                slide_rels.as_bytes(),
                opts,
            )?;

            let notes_xml = build_pptx_notes_slide_xml(&snip.notes);
            write_zip_part(
                &mut zw,
                &format!("ppt/notesSlides/notesSlide{slide_num}.xml"),
                notes_xml.as_bytes(),
                opts,
            )?;

            let notes_rels = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
                 <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
                 <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide\" Target=\"../slides/slide{slide_num}.xml\"/>\
                 </Relationships>"
            );
            write_zip_part(
                &mut zw,
                &format!("ppt/notesSlides/_rels/notesSlide{slide_num}.xml.rels"),
                notes_rels.as_bytes(),
                opts,
            )?;
        }

        zw.finish().map_err(std::io::Error::other)?;
    }

    Ok(cursor.into_inner())
}

fn render_snippet_png(doc: &VisioDocument, snip: &SlideSnippetFrame) -> Vec<u8> {
    let crop_w = snip.width_in.max(0.5);
    let crop_h = snip.height_in.max(0.5);
    let render_w: u32 = 640;
    let render_h: u32 = ((render_w as f64 * (crop_h / crop_w)).round() as u32).clamp(200, 540);

    let is_full_page = doc
        .page(snip.page_id)
        .map(|p| {
            snip.x_in.abs() < 1e-4
                && snip.y_in.abs() < 1e-4
                && (snip.width_in - p.width_in()).abs() < 1e-3
                && (snip.height_in - p.height_in()).abs() < 1e-3
        })
        .unwrap_or(false);

    let rgba = if is_full_page {
        render_page_rgba_auto(doc, snip.page_id, render_w, render_h)
            .unwrap_or_else(|_| vec![255u8; (render_w as usize) * (render_h as usize) * 4])
    } else {
        let mut cam = ViewportCamera::new(render_w, render_h, 1.0);
        cam.center_x_in = snip.x_in + crop_w * 0.5;
        cam.center_y_in = snip.y_in + crop_h * 0.5;
        cam.zoom_ppi = ((render_w as f64) / crop_w)
            .min((render_h as f64) / crop_h)
            .clamp(1.0, 2400.0);
        render_page_software_rgba_with_camera(doc, snip.page_id, &cam, false)
            .unwrap_or_else(|_| vec![255u8; (render_w as usize) * (render_h as usize) * 4])
    };

    encode_rgba_to_png(render_w, render_h, &rgba, 150)
}

fn build_pptx_slide_xml(title: &str, notes: &str) -> String {
    let esc_title = xml_escape(title);
    let esc_notes = xml_escape(notes);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <p:sld xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
                xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">\
           <p:cSld>\
             <p:spTree>\
               <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
               <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
               <p:sp>\
                 <p:nvSpPr><p:cNvPr id=\"2\" name=\"SlideTitle\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr><a:xfrm><a:off x=\"457200\" y=\"228600\"/><a:ext cx=\"11277600\" cy=\"640080\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"2400\" b=\"1\"/><a:t>{esc_title}</a:t></a:r></a:p></p:txBody>\
               </p:sp>\
               <p:pic>\
                 <p:nvPicPr><p:cNvPr id=\"3\" name=\"DiagramCanvas\"/><p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
                 <p:blipFill><a:blip r:embed=\"rId2\"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>\
                 <p:spPr><a:xfrm><a:off x=\"914400\" y=\"960120\"/><a:ext cx=\"10363200\" cy=\"5029200\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
               </p:pic>\
               <p:sp>\
                 <p:nvSpPr><p:cNvPr id=\"4\" name=\"SpeakerNotesSummary\"/><p:cNvSpPr txBox=\"1\"/><p:nvPr/></p:nvSpPr>\
                 <p:spPr><a:xfrm><a:off x=\"457200\" y=\"6126480\"/><a:ext cx=\"11277600\" cy=\"548640\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></p:spPr>\
                 <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"1200\"/><a:t>{esc_notes}</a:t></a:r></a:p></p:txBody>\
               </p:sp>\
             </p:spTree>\
           </p:cSld>\
         </p:sld>"
    )
}

fn build_pptx_notes_slide_xml(notes: &str) -> String {
    let esc_notes = xml_escape(notes);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <p:notes xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
                  xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
                  xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">\
           <p:cSld>\
             <p:spTree>\
               <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
               <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
               <p:sp>\
                 <p:nvSpPr><p:cNvPr id=\"2\" name=\"NotesBody\"/><p:cNvSpPr/><p:nvPr><p:ph type=\"body\" idx=\"1\"/></p:nvPr></p:nvSpPr>\
                 <p:spPr/>\
                 <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"1200\"/><a:t>{esc_notes}</a:t></a:r></a:p></p:txBody>\
               </p:sp>\
             </p:spTree>\
           </p:cSld>\
         </p:notes>"
    )
}

/// Export a complete OpenXML WordprocessingML `.docx` Process Specification Document containing:
/// - Document title & author/header metadata
/// - Page summaries
/// - Structured `<w:tbl>` step-by-step inventory tables (`ID`, `Shape Name`, `Text`, `Swimlane/Function`,
///   `Outgoing Connections`, `Prop.*` attributes)
/// - Automated BPMN 2.0 Validation Summary.
pub fn export_word_process_spec_docx(doc: &VisioDocument) -> Result<Vec<u8>, std::io::Error> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zw = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        write_zip_part(
            &mut zw,
            "[Content_Types].xml",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
              <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
              <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
              <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
              <Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
              </Types>",
            opts,
        )?;

        write_zip_part(
            &mut zw,
            "_rels/.rels",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
              <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
              </Relationships>",
            opts,
        )?;

        write_zip_part(
            &mut zw,
            "word/_rels/document.xml.rels",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
              <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
              <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
              </Relationships>",
            opts,
        )?;

        write_zip_part(&mut zw, "word/styles.xml", DOCX_STYLES_XML.as_bytes(), opts)?;

        let mut body = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
             <w:body>",
        );

        let doc_title = if doc.properties.title.trim().is_empty() {
            "Visio Process Specification"
        } else {
            doc.properties.title.as_str()
        };
        append_docx_para(&mut body, Some("Title"), doc_title);
        append_docx_para(
            &mut body,
            None,
            &format!(
                "Author: {} | Foreground Pages: {}",
                if doc.properties.creator.is_empty() {
                    "Vexio"
                } else {
                    &doc.properties.creator
                },
                doc.pages.iter().filter(|p| !p.is_background).count()
            ),
        );

        // Per-page sections & `<w:tbl>` step inventory
        for page in doc.pages.iter().filter(|p| !p.is_background) {
            append_docx_para(
                &mut body,
                Some("Heading1"),
                &format!(
                    "Page {}: {} ({}\" x {}\")",
                    page.id,
                    page.name,
                    format_clean_f64(page.width_in()),
                    format_clean_f64(page.height_in())
                ),
            );

            let outgoing = build_page_outgoing_descriptions(page);
            let step_shapes: Vec<&VisioShape> = page
                .shapes
                .iter()
                .filter(|s| {
                    s.kind == ShapeKind::Shape2D
                        && s.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) < 0.5
                })
                .collect();

            append_docx_para(
                &mut body,
                None,
                &format!(
                    "Page Summary: {} process shapes, {} glued connector endpoints.",
                    step_shapes.len(),
                    page.connects.len()
                ),
            );

            body.push_str(
                "<w:tbl>\
                 <w:tblPr><w:tblStyle w:val=\"TableGrid\"/><w:tblW w:w=\"9360\" w:type=\"dxa\"/></w:tblPr>",
            );
            append_docx_table_row(
                &mut body,
                &[
                    "ID",
                    "Shape Name",
                    "Text",
                    "Swimlane / Function",
                    "Outgoing Connections",
                    "Prop.* Attributes",
                ],
                true,
            );

            for sh in step_shapes {
                let func = sh
                    .sheet
                    .get_cell("Prop.Function")
                    .or_else(|| sh.sheet.get_cell("Prop.Pool"))
                    .and_then(|c| c.value.as_str_coerced().ok())
                    .map(|s| s.trim_matches('"').to_string())
                    .unwrap_or_else(|| "—".into());

                let out_str = outgoing
                    .get(&sh.id)
                    .map(|v| v.join("; "))
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "—".into());

                let mut props_list = Vec::new();
                for row in sh.sheet.shape_data.values() {
                    let clean_val = sh
                        .sheet
                        .get_cell(&format!("Prop.{}", row.name))
                        .and_then(|c| c.value.as_str_coerced().ok())
                        .unwrap_or_else(|| row.value.clone());
                    props_list.push(format!("{}={}", row.label, clean_val.trim_matches('"')));
                }
                let props_str = if props_list.is_empty() {
                    "—".into()
                } else {
                    props_list.join(", ")
                };

                append_docx_table_row(
                    &mut body,
                    &[
                        &sh.id.to_string(),
                        &sh.name,
                        &sh.text.replace('\n', " "),
                        &func,
                        &out_str,
                        &props_str,
                    ],
                    false,
                );
            }
            body.push_str("</w:tbl>");
        }

        // Validation Summary Section
        append_docx_para(&mut body, Some("Heading1"), "Validation Summary (BPMN 2.0)");
        let issues = validate_document(doc, DiagramRuleSet::Bpmn20);
        if issues.is_empty() {
            append_docx_para(
                &mut body,
                None,
                "Status: PASSED — 0 structural validation issues detected.",
            );
        } else {
            append_docx_para(
                &mut body,
                None,
                &format!("Status: {} validation issue(s) detected:", issues.len()),
            );
            for iss in &issues {
                append_docx_para(
                    &mut body,
                    None,
                    &format!(
                        "[{}] ({:?}, Page {}, Shape {:?}): {}",
                        iss.rule_code, iss.severity, iss.page_id, iss.shape_id, iss.description
                    ),
                );
            }
        }

        body.push_str(
            "<w:sectPr><w:pgSz w:w=\"12240\" w:h=\"15840\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\"/></w:sectPr>\
             </w:body></w:document>",
        );
        write_zip_part(&mut zw, "word/document.xml", body.as_bytes(), opts)?;
        zw.finish().map_err(std::io::Error::other)?;
    }

    Ok(cursor.into_inner())
}

fn build_page_outgoing_descriptions(page: &VisioPage) -> BTreeMap<u64, Vec<String>> {
    let mut beg: BTreeMap<u64, u64> = BTreeMap::new();
    let mut end: BTreeMap<u64, u64> = BTreeMap::new();
    for c in &page.connects {
        if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            beg.insert(c.connector_shape_id, c.target_shape_id);
        } else {
            end.insert(c.connector_shape_id, c.target_shape_id);
        }
    }
    let mut out: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for s in &page.shapes {
        if s.kind == ShapeKind::Connector1D {
            if let (Some(&u), Some(&v)) = (beg.get(&s.id), end.get(&s.id)) {
                let target_label = page
                    .find_shape(v)
                    .map(|t| {
                        if t.text.trim().is_empty() {
                            t.name.clone()
                        } else {
                            t.text.lines().next().unwrap_or(&t.name).to_string()
                        }
                    })
                    .unwrap_or_else(|| format!("Sheet.{v}"));
                let desc = if s.text.trim().is_empty() {
                    format!("-> {target_label}")
                } else {
                    format!("-[{}]-> {target_label}", s.text.trim())
                };
                out.entry(u).or_default().push(desc);
            }
        }
    }
    out
}

fn append_docx_para(out: &mut String, style: Option<&str>, text: &str) {
    out.push_str("<w:p>");
    if let Some(st) = style {
        let _ = write!(out, "<w:pPr><w:pStyle w:val=\"{st}\"/></w:pPr>");
    }
    let _ = write!(
        out,
        "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        xml_escape(text)
    );
}

fn append_docx_table_row(out: &mut String, cols: &[&str], is_header: bool) {
    out.push_str("<w:tr>");
    for &c in cols {
        out.push_str("<w:tc><w:p><w:r>");
        if is_header {
            out.push_str("<w:rPr><w:b/></w:rPr>");
        }
        let _ = write!(
            out,
            "<w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>",
            xml_escape(c)
        );
    }
    out.push_str("</w:tr>");
}

fn write_zip_part<W: Write + std::io::Seek>(
    zw: &mut ZipWriter<W>,
    path: &str,
    bytes: &[u8],
    opts: SimpleFileOptions,
) -> Result<(), std::io::Error> {
    zw.start_file(path, opts).map_err(std::io::Error::other)?;
    zw.write_all(bytes)?;
    Ok(())
}

// ============================================================================
// 4. Visual `.vsdx` Diagram Compare (`compare_documents`)
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShapeDiffKind {
    Added,
    Removed,
    MovedOrResized,
    TextChanged,
    CellsModified(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramDiffEntry {
    pub page_name: String,
    pub shape_id: u64,
    pub shape_name: String,
    pub kind: ShapeDiffKind,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramCompareReport {
    pub entries: Vec<DiagramDiffEntry>,
    pub added_count: usize,
    pub removed_count: usize,
    pub modified_count: usize,
    pub diff_overlay_svg: String,
}

/// Compare two `VisioDocument` revisions (`baseline` vs `revised`) across pages, shapes,
/// transforms, text, cells, and connector bindings, producing a structured change list and
/// a color-coded SVG overlay (`#10b981` Added, `#f59e0b` Modified/Moved, `#ef4444` Removed).
pub fn compare_documents(
    baseline: &VisioDocument,
    revised: &VisioDocument,
) -> DiagramCompareReport {
    let base_pages: BTreeMap<u32, &VisioPage> =
        baseline.pages.iter().map(|p| (p.id, p)).collect();
    let rev_pages: BTreeMap<u32, &VisioPage> =
        revised.pages.iter().map(|p| (p.id, p)).collect();
    let all_pids: BTreeSet<u32> = base_pages
        .keys()
        .chain(rev_pages.keys())
        .copied()
        .collect();

    let mut entries = Vec::new();
    let mut added_count = 0usize;
    let mut removed_count = 0usize;
    let mut modified_count = 0usize;

    for pid in all_pids {
        match (base_pages.get(&pid), rev_pages.get(&pid)) {
            (None, Some(&rp)) => {
                for sh in &rp.shapes {
                    added_count += 1;
                    entries.push(DiagramDiffEntry {
                        page_name: rp.name.clone(),
                        shape_id: sh.id,
                        shape_name: sh.name.clone(),
                        kind: ShapeDiffKind::Added,
                        summary: format!(
                            "Added shape '{}' (#{}) on new page '{}'",
                            sh.name, sh.id, rp.name
                        ),
                    });
                }
            }
            (Some(&bp), None) => {
                for sh in &bp.shapes {
                    removed_count += 1;
                    entries.push(DiagramDiffEntry {
                        page_name: bp.name.clone(),
                        shape_id: sh.id,
                        shape_name: sh.name.clone(),
                        kind: ShapeDiffKind::Removed,
                        summary: format!(
                            "Removed shape '{}' (#{}) from deleted page '{}'",
                            sh.name, sh.id, bp.name
                        ),
                    });
                }
            }
            (Some(&bp), Some(&rp)) => {
                let b_shapes: BTreeMap<u64, &VisioShape> =
                    bp.shapes.iter().map(|s| (s.id, s)).collect();
                let r_shapes: BTreeMap<u64, &VisioShape> =
                    rp.shapes.iter().map(|s| (s.id, s)).collect();
                let b_glue = connector_glue_map(bp);
                let r_glue = connector_glue_map(rp);

                let all_sids: BTreeSet<u64> = b_shapes
                    .keys()
                    .chain(r_shapes.keys())
                    .copied()
                    .collect();

                for sid in all_sids {
                    match (b_shapes.get(&sid), r_shapes.get(&sid)) {
                        (None, Some(&rsh)) => {
                            added_count += 1;
                            entries.push(DiagramDiffEntry {
                                page_name: rp.name.clone(),
                                shape_id: sid,
                                shape_name: rsh.name.clone(),
                                kind: ShapeDiffKind::Added,
                                summary: format!("Added shape '{}' (#{})", rsh.name, sid),
                            });
                        }
                        (Some(&bsh), None) => {
                            removed_count += 1;
                            entries.push(DiagramDiffEntry {
                                page_name: rp.name.clone(),
                                shape_id: sid,
                                shape_name: bsh.name.clone(),
                                kind: ShapeDiffKind::Removed,
                                summary: format!("Removed shape '{}' (#{})", bsh.name, sid),
                            });
                        }
                        (Some(&bsh), Some(&rsh)) => {
                            let moved_or_resized = has_transform_delta(bsh, rsh);
                            let text_changed = bsh.text != rsh.text;
                            let non_transform_cells = diff_non_transform_cells(
                                bsh,
                                rsh,
                                b_glue.get(&sid),
                                r_glue.get(&sid),
                            );

                            if !moved_or_resized && !text_changed && non_transform_cells.is_empty()
                            {
                                continue;
                            }

                            modified_count += 1;
                            let (kind, summary) = if !non_transform_cells.is_empty()
                                && !moved_or_resized
                                && !text_changed
                            {
                                let joined = non_transform_cells.join(", ");
                                (
                                    ShapeDiffKind::CellsModified(non_transform_cells),
                                    format!(
                                        "Modified cells [{}] on '{}' (#{})",
                                        joined, rsh.name, sid
                                    ),
                                )
                            } else if text_changed && !moved_or_resized {
                                (
                                    ShapeDiffKind::TextChanged,
                                    format!(
                                        "Changed text '{}' -> '{}' on '{}' (#{})",
                                        bsh.text, rsh.text, rsh.name, sid
                                    ),
                                )
                            } else {
                                (
                                    ShapeDiffKind::MovedOrResized,
                                    format!(
                                        "Moved or resized '{}' (#{})",
                                        rsh.name, sid
                                    ),
                                )
                            };

                            entries.push(DiagramDiffEntry {
                                page_name: rp.name.clone(),
                                shape_id: sid,
                                shape_name: rsh.name.clone(),
                                kind,
                                summary,
                            });
                        }
                        (None, None) => {}
                    }
                }
            }
            (None, None) => {}
        }
    }

    let diff_overlay_svg = build_diff_overlay_svg(baseline, revised, &entries);

    DiagramCompareReport {
        entries,
        added_count,
        removed_count,
        modified_count,
        diff_overlay_svg,
    }
}

fn has_transform_delta(a: &VisioShape, b: &VisioShape) -> bool {
    const KEYS: [&str; 9] = [
        "PinX", "PinY", "Width", "Height", "Angle", "BeginX", "BeginY", "EndX", "EndY",
    ];
    for k in KEYS {
        match (a.sheet.get_f64(k), b.sheet.get_f64(k)) {
            (Some(va), Some(vb)) if (va - vb).abs() > 1e-4 => return true,
            (Some(_), None) | (None, Some(_)) => return true,
            _ => {}
        }
    }
    false
}

fn is_transform_cell_key(k: &str) -> bool {
    matches!(
        k,
        "pinx"
            | "piny"
            | "width"
            | "height"
            | "angle"
            | "beginx"
            | "beginy"
            | "endx"
            | "endy"
            | "locpinx"
            | "locpiny"
            | "txtpinx"
            | "txtpiny"
            | "txtwidth"
            | "txtheight"
            | "txtlocpinx"
            | "txtlocpiny"
    )
}

fn diff_non_transform_cells(
    bsh: &VisioShape,
    rsh: &VisioShape,
    b_glue: Option<&(Option<u64>, Option<u64>)>,
    r_glue: Option<&(Option<u64>, Option<u64>)>,
) -> Vec<String> {
    let mut keys: BTreeSet<&str> = BTreeSet::new();
    for k in bsh.sheet.cells.keys() {
        keys.insert(k.as_str());
    }
    for k in rsh.sheet.cells.keys() {
        keys.insert(k.as_str());
    }

    let mut changed = Vec::new();
    for k in keys {
        if is_transform_cell_key(k) {
            continue;
        }
        if rsh.kind == ShapeKind::Connector1D && k.starts_with("geometry1.") {
            continue;
        }
        let bf = bsh.sheet.cells.get(k).map(|c| c.formula.as_str());
        let rf = rsh.sheet.cells.get(k).map(|c| c.formula.as_str());
        if bf != rf {
            changed.push(k.to_string());
        }
    }

    if b_glue != r_glue {
        changed.push("connect.endpoints".into());
    }
    changed
}

fn connector_glue_map(page: &VisioPage) -> BTreeMap<u64, (Option<u64>, Option<u64>)> {
    let mut out: BTreeMap<u64, (Option<u64>, Option<u64>)> = BTreeMap::new();
    for c in &page.connects {
        let entry = out.entry(c.connector_shape_id).or_insert((None, None));
        if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            entry.0 = Some(c.target_shape_id);
        } else {
            entry.1 = Some(c.target_shape_id);
        }
    }
    out
}

fn build_diff_overlay_svg(
    baseline: &VisioDocument,
    revised: &VisioDocument,
    entries: &[DiagramDiffEntry],
) -> String {
    let primary_rev = revised
        .pages
        .iter()
        .find(|p| !p.is_background)
        .or_else(|| revised.pages.first());
    let primary_base = baseline
        .pages
        .iter()
        .find(|p| !p.is_background)
        .or_else(|| baseline.pages.first());

    let (w_in, h_in, base_svg_inner) = if let Some(rp) = primary_rev {
        let raw_svg = export_page_svg(revised, rp.id);
        let stripped = raw_svg
            .find('>')
            .and_then(|start| raw_svg.rfind("</svg>").map(|end| &raw_svg[start + 1..end]))
            .unwrap_or("")
            .to_string();
        (rp.width_in(), rp.height_in(), stripped)
    } else if let Some(bp) = primary_base {
        (bp.width_in(), bp.height_in(), String::new())
    } else {
        (11.0, 8.5, String::new())
    };

    let ppi = 96.0;
    let w_px = w_in * ppi;
    let h_px = h_in * ppi;

    let mut svg = String::new();
    let _ = writeln!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w_px:.2}\" height=\"{h_px:.2}\" viewBox=\"0 0 {w_px:.2} {h_px:.2}\">"
    );
    svg.push_str(&base_svg_inner);
    svg.push_str("<g id=\"vexio-diff-overlay\">\n");

    let target_page_name = primary_rev
        .map(|p| p.name.as_str())
        .or_else(|| primary_base.map(|p| p.name.as_str()))
        .unwrap_or("");

    for entry in entries {
        if !target_page_name.is_empty() && entry.page_name != target_page_name {
            continue;
        }
        match &entry.kind {
            ShapeDiffKind::Added => {
                if let Some(sh) = primary_rev.and_then(|p| p.find_shape(entry.shape_id)) {
                    let [x, y, w, h] = shape_svg_box(sh, h_in, ppi);
                    let _ = writeln!(
                        svg,
                        "<rect data-diff=\"added\" data-shape-id=\"{}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#10b981\" fill-opacity=\"0.16\" stroke=\"#10b981\" stroke-width=\"2.5\" rx=\"4\"/>",
                        entry.shape_id
                    );
                }
            }
            ShapeDiffKind::Removed => {
                if let Some(sh) = primary_base.and_then(|p| p.find_shape(entry.shape_id)) {
                    let [x, y, w, h] = shape_svg_box(sh, h_in, ppi);
                    let _ = writeln!(
                        svg,
                        "<rect data-diff=\"removed\" data-shape-id=\"{}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#ef4444\" fill-opacity=\"0.14\" stroke=\"#ef4444\" stroke-width=\"2.5\" stroke-dasharray=\"6,4\" rx=\"4\"/>",
                        entry.shape_id
                    );
                }
            }
            ShapeDiffKind::MovedOrResized
            | ShapeDiffKind::TextChanged
            | ShapeDiffKind::CellsModified(_) => {
                if let Some(sh) = primary_rev.and_then(|p| p.find_shape(entry.shape_id)) {
                    let [x, y, w, h] = shape_svg_box(sh, h_in, ppi);
                    let _ = writeln!(
                        svg,
                        "<rect data-diff=\"modified\" data-shape-id=\"{}\" x=\"{x:.2}\" y=\"{y:.2}\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#f59e0b\" fill-opacity=\"0.16\" stroke=\"#f59e0b\" stroke-width=\"2.5\" rx=\"4\"/>",
                        entry.shape_id
                    );
                }
            }
        }
    }

    svg.push_str("</g>\n</svg>");
    svg
}

fn shape_svg_box(sh: &VisioShape, page_h_in: f64, ppi: f64) -> [f64; 4] {
    if sh.kind == ShapeKind::Connector1D {
        let bx = sh.sheet.get_f64("BeginX").unwrap_or(0.0);
        let by = sh.sheet.get_f64("BeginY").unwrap_or(0.0);
        let ex = sh.sheet.get_f64("EndX").unwrap_or(1.0);
        let ey = sh.sheet.get_f64("EndY").unwrap_or(1.0);
        let min_x = bx.min(ex) * ppi - 6.0;
        let max_x = bx.max(ex) * ppi + 6.0;
        let min_y = (page_h_in - by.max(ey)) * ppi - 6.0;
        let max_y = (page_h_in - by.min(ey)) * ppi + 6.0;
        [min_x, min_y, (max_x - min_x).max(12.0), (max_y - min_y).max(12.0)]
    } else {
        let px = sh.sheet.get_f64("PinX").unwrap_or(2.0);
        let py = sh.sheet.get_f64("PinY").unwrap_or(2.0);
        let w = sh.sheet.get_f64("Width").unwrap_or(1.5).max(0.2);
        let h = sh.sheet.get_f64("Height").unwrap_or(0.9).max(0.2);
        let pad = 4.0;
        let x = (px - w * 0.5) * ppi - pad;
        let y = (page_h_in - (py + h * 0.5)) * ppi - pad;
        [x, y, w * ppi + pad * 2.0, h * ppi + pad * 2.0]
    }
}

// ============================================================================
// Minimal OpenXML Static Master/Theme/Styles Templates
// ============================================================================

const PPTX_SLIDE_MASTER_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<p:sldMaster xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
             xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
             xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\">\
  <p:cSld>\
    <p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
    <p:spTree>\
      <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
      <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
    </p:spTree>\
  </p:cSld>\
  <p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\
  <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
</p:sldMaster>";

const PPTX_SLIDE_LAYOUT_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<p:sldLayout xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
             xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
             xmlns:p=\"http://schemas.openxmlformats.org/presentationml/2006/main\" type=\"blank\" preserve=\"1\">\
  <p:cSld name=\"Blank\">\
    <p:spTree>\
      <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
      <p:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"0\" cy=\"0\"/><a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"0\" cy=\"0\"/></a:xfrm></p:grpSpPr>\
    </p:spTree>\
  </p:cSld>\
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>\
</p:sldLayout>";

const PPTX_THEME_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<a:theme xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" name=\"Vexio Office Theme\">\
  <a:themeElements>\
    <a:clrScheme name=\"Vexio\">\
      <a:dk1><a:srgbClr val=\"0F172A\"/></a:dk1>\
      <a:lt1><a:srgbClr val=\"FFFFFF\"/></a:lt1>\
      <a:dk2><a:srgbClr val=\"1E293B\"/></a:dk2>\
      <a:lt2><a:srgbClr val=\"F8FAFC\"/></a:lt2>\
      <a:accent1><a:srgbClr val=\"2563EB\"/></a:accent1>\
      <a:accent2><a:srgbClr val=\"10B981\"/></a:accent2>\
      <a:accent3><a:srgbClr val=\"F59E0B\"/></a:accent3>\
      <a:accent4><a:srgbClr val=\"EF4444\"/></a:accent4>\
      <a:accent5><a:srgbClr val=\"8B5CF6\"/></a:accent5>\
      <a:accent6><a:srgbClr val=\"06B6D4\"/></a:accent6>\
      <a:hlink><a:srgbClr val=\"2563EB\"/></a:hlink>\
      <a:folHlink><a:srgbClr val=\"7C3AED\"/></a:folHlink>\
    </a:clrScheme>\
    <a:fontScheme name=\"VexioFonts\">\
      <a:majorFont><a:latin typeface=\"Segoe UI\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:majorFont>\
      <a:minorFont><a:latin typeface=\"Segoe UI\"/><a:ea typeface=\"\"/><a:cs typeface=\"\"/></a:minorFont>\
    </a:fontScheme>\
    <a:fmtScheme name=\"VexioFmt\">\
      <a:fillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:fillStyleLst>\
      <a:lnStyleLst><a:ln w=\"9525\"><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:ln></a:lnStyleLst>\
      <a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>\
      <a:bgFillStyleLst><a:solidFill><a:schemeClr val=\"phClr\"/></a:solidFill></a:bgFillStyleLst>\
    </a:fmtScheme>\
  </a:themeElements>\
</a:theme>";

const DOCX_STYLES_XML: &str = "\
<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:styles xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
  <w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\">\
    <w:name w:val=\"Normal\"/>\
    <w:rPr><w:rFonts w:ascii=\"Segoe UI\" w:hAnsi=\"Segoe UI\"/><w:sz w:val=\"20\"/></w:rPr>\
  </w:style>\
  <w:style w:type=\"paragraph\" w:styleId=\"Title\">\
    <w:name w:val=\"Title\"/>\
    <w:rPr><w:b/><w:sz w:val=\"36\"/><w:color w:val=\"0F172A\"/></w:rPr>\
  </w:style>\
  <w:style w:type=\"paragraph\" w:styleId=\"Heading1\">\
    <w:name w:val=\"heading 1\"/>\
    <w:rPr><w:b/><w:sz w:val=\"26\"/><w:color w:val=\"1D4ED8\"/></w:rPr>\
  </w:style>\
  <w:style w:type=\"table\" w:styleId=\"TableGrid\">\
    <w:name w:val=\"Table Grid\"/>\
    <w:tblPr>\
      <w:tblBorders>\
        <w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CBD5E1\"/>\
        <w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CBD5E1\"/>\
        <w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CBD5E1\"/>\
        <w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CBD5E1\"/>\
        <w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CBD5E1\"/>\
        <w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"CBD5E1\"/>\
      </w:tblBorders>\
    </w:tblPr>\
  </w:style>\
</w:styles>";

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use zip::ZipArchive;

    #[test]
    fn subprocess_drilldown_hyperlinks_screentips_and_iso_title_block() {
        let mut doc = VisioDocument::new_blank("Enterprise Order Pipeline");
        {
            let p = doc.page_mut(1).unwrap();
            let mut task = VisioShape::new_rect(10, "PaymentCheck", 4.5, 4.5, 2.0, 1.0);
            task.text = "Verify Fraud & KYC".into();
            p.shapes.push(task);
        }

        let sub_pid =
            create_or_link_subprocess_page(&mut doc, 1, 10, "KYC Subprocess").unwrap();
        assert_eq!(sub_pid, 2);

        // Parent shape has User.IsSubprocess = TRUE, [+] badge child, and default hyperlink to "KYC Subprocess"
        let parent_task = doc.page(1).unwrap().find_shape(10).unwrap();
        assert_eq!(parent_task.sheet.get_bool("User.IsSubprocess"), Some(true));
        assert!(parent_task
            .sheet
            .hyperlinks
            .iter()
            .any(|h| h.default && h.sub_address == "KYC Subprocess"));
        assert!(!parent_task.children.is_empty());

        // Subprocess page has Start -> Step -> End + "← Return to Parent Page" breadcrumb back to Page-1
        let sub_page = doc.page(sub_pid).unwrap();
        let breadcrumb = sub_page
            .shapes
            .iter()
            .find(|s| s.text == "← Return to Parent Page")
            .expect("Subprocess page must include Return breadcrumb");
        assert!(breadcrumb
            .sheet
            .hyperlinks
            .iter()
            .any(|h| h.sub_address == "Page-1"));

        // Additional hyperlink + ScreenTip
        assert!(add_shape_hyperlink(
            &mut doc,
            1,
            10,
            "Compliance Runbook",
            "https://runbooks.internal/kyc",
            "",
            false
        ));
        assert!(set_shape_screentip(
            &mut doc,
            1,
            10,
            "Automated AML/KYC verification SLA < 250ms"
        ));
        let updated_task = doc.page(1).unwrap().find_shape(10).unwrap();
        assert_eq!(updated_task.sheet.hyperlinks.len(), 2);
        assert_eq!(
            updated_task
                .sheet
                .get_cell("Comment")
                .unwrap()
                .value
                .as_str_coerced()
                .unwrap(),
            "Automated AML/KYC verification SLA < 250ms"
        );

        // ISO Title Block Background Page
        let bg_id =
            apply_iso_title_block_background(&mut doc, 1, "PRJ-2026-X1", "A. Lovelace", "B.2")
                .unwrap();
        assert!(doc.page(bg_id).unwrap().is_background);
        assert_eq!(doc.page(1).unwrap().back_page_id, Some(bg_id));
        assert!(doc.header_footer.header_left.contains("PRJ-2026-X1"));
        assert!(doc.header_footer.footer_left.contains("A. Lovelace"));
        assert!(doc.header_footer.header_right.contains("B.2"));
    }

    #[test]
    fn openxml_pptx_and_docx_exports_produce_valid_packages() {
        let mut doc = crate::domains::instantiate_starter_template("bpmn_fulfillment");
        let _ = apply_iso_title_block_background(&mut doc, 1, "ORD-900", "M. Vance", "1.0");

        // 1. Export .pptx with explicit SlideSnippetFrame
        let snippets = vec![SlideSnippetFrame {
            id: 1,
            page_id: 1,
            title: "Order Fulfillment Overview".into(),
            notes: "Walk stakeholders through payment gateway split.".into(),
            x_in: 0.5,
            y_in: 1.0,
            width_in: 10.0,
            height_in: 6.5,
        }];
        let pptx_bytes = export_presentation_pptx(&doc, &snippets).unwrap();
        assert!(pptx_bytes.starts_with(b"PK\x03\x04"));

        let mut pptx_zip = ZipArchive::new(Cursor::new(&pptx_bytes)).unwrap();
        assert!(pptx_zip.by_name("[Content_Types].xml").is_ok());
        assert!(pptx_zip.by_name("_rels/.rels").is_ok());
        assert!(pptx_zip.by_name("ppt/presentation.xml").is_ok());
        assert!(pptx_zip.by_name("ppt/slideMasters/slideMaster1.xml").is_ok());
        assert!(pptx_zip.by_name("ppt/slideLayouts/slideLayout1.xml").is_ok());

        let mut png_header = [0u8; 8];
        pptx_zip
            .by_name("ppt/media/slide1.png")
            .unwrap()
            .read_exact(&mut png_header)
            .unwrap();
        assert_eq!(&png_header, &[137, 80, 78, 71, 13, 10, 26, 10]);

        let mut slide1_xml = String::new();
        pptx_zip
            .by_name("ppt/slides/slide1.xml")
            .unwrap()
            .read_to_string(&mut slide1_xml)
            .unwrap();
        assert!(slide1_xml.contains("Order Fulfillment Overview"));
        assert!(slide1_xml.contains("Walk stakeholders through payment gateway split."));

        // Also verify empty snippets auto-generates 1 slide per foreground page (ignoring background page!)
        let auto_pptx = export_presentation_pptx(&doc, &[]).unwrap();
        let mut auto_zip = ZipArchive::new(Cursor::new(&auto_pptx)).unwrap();
        assert!(auto_zip.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(auto_zip.by_name("ppt/slides/slide2.xml").is_err());

        // 2. Export .docx Word Process Specification
        let docx_bytes = export_word_process_spec_docx(&doc).unwrap();
        assert!(docx_bytes.starts_with(b"PK\x03\x04"));
        let mut docx_zip = ZipArchive::new(Cursor::new(&docx_bytes)).unwrap();
        assert!(docx_zip.by_name("[Content_Types].xml").is_ok());
        assert!(docx_zip.by_name("word/styles.xml").is_ok());

        let mut word_xml = String::new();
        docx_zip
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut word_xml)
            .unwrap();
        assert!(word_xml.contains("<w:tbl>"));
        assert!(word_xml.contains("Verify Payment"));
        assert!(word_xml.contains("Dispatch Shipment"));
        assert!(word_xml.contains("Validation Summary (BPMN 2.0)"));
    }

    #[test]
    fn compare_documents_detects_added_removed_moved_text_and_cell_diffs_with_svg_overlay() {
        let mut baseline = VisioDocument::new_blank("Architecture v1");
        {
            let p = baseline.page_mut(1).unwrap();
            let mut s1 = VisioShape::new_rect(1, "Ingress", 2.0, 5.0, 1.8, 0.9);
            s1.text = "API Gateway v1".into();
            let mut s2 = VisioShape::new_rect(2, "Worker", 5.0, 5.0, 1.8, 0.9);
            s2.text = "Worker Pool".into();
            let mut s3 = VisioShape::new_rect(3, "Cache", 8.0, 5.0, 1.8, 0.9);
            s3.text = "Redis Cache".into();
            let mut s4 = VisioShape::new_rect(4, "LegacyMonolith", 5.0, 2.0, 2.0, 1.0);
            s4.text = "Deprecated Service".into();
            p.shapes.extend([s1, s2, s3, s4]);
        }
        ReactiveEngine::recalculate_document(&mut baseline);

        let mut revised = baseline.clone();
        {
            let p = revised.page_mut(1).unwrap();
            // Shape 1: TextChanged
            p.find_shape_mut(1).unwrap().text = "API Gateway v2 (mTLS)".into();
            // Shape 2: MovedOrResized
            p.find_shape_mut(2)
                .unwrap()
                .sheet
                .set_formula("PinY", "6.25 in", Some(Unit::Inch));
            // Shape 3: CellsModified (`fillforegnd`)
            p.find_shape_mut(3)
                .unwrap()
                .sheet
                .set_formula("FillForegnd", "=RGB(220,252,231)", Some(Unit::Color));
            // Shape 4: Removed
            p.shapes.retain(|s| s.id != 4);
            // Shape 5: Added
            let mut s5 = VisioShape::new_rect(5, "EventBus", 8.5, 2.5, 2.0, 0.9);
            s5.text = "Kafka Event Mesh".into();
            p.shapes.push(s5);
        }
        ReactiveEngine::recalculate_document(&mut revised);

        let report = compare_documents(&baseline, &revised);
        assert_eq!(report.added_count, 1);
        assert_eq!(report.removed_count, 1);
        assert_eq!(report.modified_count, 3);
        assert_eq!(report.entries.len(), 5);

        assert!(report
            .entries
            .iter()
            .any(|e| e.shape_id == 1 && e.kind == ShapeDiffKind::TextChanged));
        assert!(report
            .entries
            .iter()
            .any(|e| e.shape_id == 2 && e.kind == ShapeDiffKind::MovedOrResized));
        assert!(report.entries.iter().any(|e| {
            e.shape_id == 3
                && matches!(&e.kind, ShapeDiffKind::CellsModified(cols) if cols.contains(&"fillforegnd".to_string()))
        }));
        assert!(report
            .entries
            .iter()
            .any(|e| e.shape_id == 4 && e.kind == ShapeDiffKind::Removed));
        assert!(report
            .entries
            .iter()
            .any(|e| e.shape_id == 5 && e.kind == ShapeDiffKind::Added));

        assert!(report.diff_overlay_svg.starts_with("<svg"));
        assert!(report.diff_overlay_svg.contains("#10b981"));
        assert!(report.diff_overlay_svg.contains("#f59e0b"));
        assert!(report.diff_overlay_svg.contains("#ef4444"));
    }
}
