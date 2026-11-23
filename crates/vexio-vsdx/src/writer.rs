//! Lossless OpenXML Visio serializer (`.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, `.vstm`).

use crate::opc::{
    NS_RELS_DOC, NS_RELS_PKG, NS_VISIO_MAIN, RawOpcArchive, VsdxError, VsdxPackageKind, xml_escape,
};
use std::fmt::Write as _;
use vexio_shapesheet::{
    LayerDef, ShapeDataType, ShapeKind, ShapeSheet, VisioDocument, VisioMaster, VisioPage,
    VisioShape, format_clean_f64,
};

pub fn write_vsdx(doc: &VisioDocument, mut kind: VsdxPackageKind) -> Result<Vec<u8>, VsdxError> {
    if doc.macro_enabled && !kind.is_macro_enabled() {
        kind = match kind {
            VsdxPackageKind::Drawing => VsdxPackageKind::MacroDrawing,
            VsdxPackageKind::Stencil => VsdxPackageKind::MacroStencil,
            VsdxPackageKind::Template => VsdxPackageKind::MacroTemplate,
            other => other,
        };
    }

    let mut archive = RawOpcArchive::default();

    // 1. Re-insert all opaque parts first (`vbaProject.bin`, customXml, media, embeddings) for byte-exact preservation.
    for (path, bytes) in &doc.opaque_bag.parts {
        archive.files.insert(path.clone(), bytes.clone());
    }

    let has_vba = archive.files.contains_key("visio/vbaproject.bin")
        || archive.files.contains_key("visio/vbaProject.bin");

    // 2. `[Content_Types].xml`
    archive.files.insert(
        "[Content_Types].xml".into(),
        build_content_types_xml(doc, kind, has_vba).into_bytes(),
    );

    // 3. `_rels/.rels`
    archive.files.insert(
        "_rels/.rels".into(),
        build_root_rels_xml(doc).into_bytes(),
    );

    // 4. `docProps/core.xml`, `docProps/app.xml`, and `docProps/custom.xml`
    archive
        .files
        .insert("docProps/core.xml".into(), build_core_props_xml(doc).into_bytes());
    archive
        .files
        .insert("docProps/app.xml".into(), build_app_props_xml(doc).into_bytes());
    if !doc.properties.custom_properties.is_empty() {
        archive.files.insert(
            "docProps/custom.xml".into(),
            build_custom_props_xml(doc).into_bytes(),
        );
    }

    // 5. `visio/document.xml` + `visio/_rels/document.xml.rels`
    archive
        .files
        .insert("visio/document.xml".into(), build_document_xml(doc).into_bytes());
    archive.files.insert(
        "visio/_rels/document.xml.rels".into(),
        build_document_rels_xml(doc, has_vba).into_bytes(),
    );
    archive
        .files
        .insert("visio/windows.xml".into(), build_windows_xml(doc).into_bytes());

    // 6. Masters (`visio/masters/masters.xml` + `masterN.xml`)
    if !doc.masters.is_empty() {
        archive.files.insert(
            "visio/masters/masters.xml".into(),
            build_masters_index_xml(&doc.masters).into_bytes(),
        );
        archive.files.insert(
            "visio/masters/_rels/masters.xml.rels".into(),
            build_masters_rels_xml(&doc.masters).into_bytes(),
        );
        for m in &doc.masters {
            archive.files.insert(
                format!("visio/masters/master{}.xml", m.id),
                build_master_part_xml(m).into_bytes(),
            );
        }
    }

    // 7. Pages (`visio/pages/pages.xml` + `pageN.xml`)
    if !doc.pages.is_empty() {
        archive.files.insert(
            "visio/pages/pages.xml".into(),
            build_pages_index_xml(&doc.pages).into_bytes(),
        );
        archive.files.insert(
            "visio/pages/_rels/pages.xml.rels".into(),
            build_pages_rels_xml(&doc.pages).into_bytes(),
        );
        for p in &doc.pages {
            archive.files.insert(
                format!("visio/pages/page{}.xml", p.id),
                build_page_part_xml(p).into_bytes(),
            );
        }
    }

    // 8. Comments (`visio/comments.xml`)
    if !doc.comments.is_empty() {
        archive
            .files
            .insert("visio/comments.xml".into(), build_comments_xml(doc).into_bytes());
    }

    archive.to_bytes()
}

fn build_content_types_xml(doc: &VisioDocument, kind: VsdxPackageKind, has_vba: bool) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\n\
         <Default Extension=\"png\" ContentType=\"image/png\"/>\n\
         <Default Extension=\"jpg\" ContentType=\"image/jpeg\"/>\n\
         <Default Extension=\"bin\" ContentType=\"application/vnd.ms-office.vbaProject\"/>\n",
    );
    let _ = writeln!(
        s,
        "<Override PartName=\"/visio/document.xml\" ContentType=\"{}\"/>",
        kind.main_content_type()
    );
    s.push_str("<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\n");
    s.push_str("<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\n");
    if !doc.properties.custom_properties.is_empty() {
        s.push_str("<Override PartName=\"/docProps/custom.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.custom-properties+xml\"/>\n");
    }
    s.push_str("<Override PartName=\"/visio/windows.xml\" ContentType=\"application/vnd.ms-visio.windows+xml\"/>\n");
    if !doc.masters.is_empty() {
        s.push_str("<Override PartName=\"/visio/masters/masters.xml\" ContentType=\"application/vnd.ms-visio.masters+xml\"/>\n");
        for m in &doc.masters {
            let _ = writeln!(
                s,
                "<Override PartName=\"/visio/masters/master{}.xml\" ContentType=\"application/vnd.ms-visio.master+xml\"/>",
                m.id
            );
        }
    }
    if !doc.pages.is_empty() {
        s.push_str("<Override PartName=\"/visio/pages/pages.xml\" ContentType=\"application/vnd.ms-visio.pages+xml\"/>\n");
        for p in &doc.pages {
            let _ = writeln!(
                s,
                "<Override PartName=\"/visio/pages/page{}.xml\" ContentType=\"application/vnd.ms-visio.page+xml\"/>",
                p.id
            );
        }
    }
    if !doc.comments.is_empty() {
        s.push_str("<Override PartName=\"/visio/comments.xml\" ContentType=\"application/vnd.ms-visio.comments+xml\"/>\n");
    }
    if has_vba {
        s.push_str("<Override PartName=\"/visio/vbaproject.bin\" ContentType=\"application/vnd.ms-office.vbaProject\"/>\n");
    }
    for (part, ct) in &doc.opaque_bag.content_type_overrides {
        let _ = writeln!(
            s,
            "<Override PartName=\"{}\" ContentType=\"{}\"/>",
            xml_escape(part),
            xml_escape(ct)
        );
    }
    s.push_str("</Types>");
    s
}

fn build_root_rels_xml(doc: &VisioDocument) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{NS_RELS_PKG}\">\n\
         <Relationship Id=\"rId1\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/document\" Target=\"visio/document.xml\"/>\n\
         <Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\n\
         <Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>\n"
    );
    if !doc.properties.custom_properties.is_empty() {
        s.push_str("<Relationship Id=\"rId4\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/custom-properties\" Target=\"docProps/custom.xml\"/>\n");
    }
    s.push_str("</Relationships>");
    s
}

fn build_core_props_xml(doc: &VisioDocument) -> String {
    let p = &doc.properties;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
         xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:dcterms=\"http://purl.org/dc/terms/\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n\
         <dc:title>{}</dc:title>\n\
         <dc:subject>{}</dc:subject>\n\
         <dc:creator>{}</dc:creator>\n\
         <dc:description>{}</dc:description>\n\
         <cp:category>{}</cp:category>\n\
         <cp:keywords>{}</cp:keywords>\n\
         </cp:coreProperties>",
        xml_escape(&p.title),
        xml_escape(&p.subject),
        xml_escape(&p.creator),
        xml_escape(&p.description),
        xml_escape(&p.category),
        xml_escape(&p.keywords),
    )
}

fn build_app_props_xml(doc: &VisioDocument) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">\n\
         <Application>Vexio</Application>\n\
         <Company>{}</Company>\n\
         </Properties>",
        xml_escape(&doc.properties.company)
    )
}

fn build_custom_props_xml(doc: &VisioDocument) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/custom-properties\" \
         xmlns:vt=\"http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes\">\n",
    );
    for (idx, (k, v)) in doc.properties.custom_properties.iter().enumerate() {
        let pid = idx + 2;
        let _ = writeln!(
            s,
            "<property fmtid=\"{{D5CDD505-2E9C-101B-9397-08002B2CF9AE}}\" pid=\"{pid}\" name=\"{}\"><vt:lpwstr>{}</vt:lpwstr></property>",
            xml_escape(k),
            xml_escape(v)
        );
    }
    s.push_str("</Properties>");
    s
}

fn build_document_rels_xml(doc: &VisioDocument, has_vba: bool) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{NS_RELS_PKG}\">\n\
         <Relationship Id=\"rIdWindows\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/windows\" Target=\"windows.xml\"/>\n"
    );
    if !doc.pages.is_empty() {
        s.push_str("<Relationship Id=\"rIdPages\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/pages\" Target=\"pages/pages.xml\"/>\n");
    }
    if !doc.masters.is_empty() {
        s.push_str("<Relationship Id=\"rIdMasters\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/masters\" Target=\"masters/masters.xml\"/>\n");
    }
    if !doc.comments.is_empty() {
        s.push_str("<Relationship Id=\"rIdComments\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/comments\" Target=\"comments.xml\"/>\n");
    }
    if has_vba {
        let target = if doc.opaque_bag.parts.contains_key("visio/vbaProject.bin") {
            "vbaProject.bin"
        } else {
            "vbaproject.bin"
        };
        let _ = writeln!(
            s,
            "<Relationship Id=\"rIdVba\" Type=\"http://schemas.microsoft.com/office/2006/relationships/vbaProject\" Target=\"{target}\"/>"
        );
    }
    s.push_str("</Relationships>");
    s
}

fn build_document_xml(doc: &VisioDocument) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <VisioDocument xmlns=\"{NS_VISIO_MAIN}\" xmlns:r=\"{NS_RELS_DOC}\">\n"
    );
    // StyleSheets
    if !doc.styles.is_empty() {
        s.push_str("<StyleSheets>\n");
        for st in &doc.styles {
            let mut attrs = format!(
                "ID=\"{}\" Name=\"{}\" NameU=\"{}\"",
                st.id,
                xml_escape(&st.name),
                xml_escape(&st.name_u)
            );
            if let Some(lp) = st.line_parent {
                let _ = write!(attrs, " LineStyle=\"{lp}\"");
            }
            if let Some(fp) = st.fill_parent {
                let _ = write!(attrs, " FillStyle=\"{fp}\"");
            }
            if let Some(tp) = st.text_parent {
                let _ = write!(attrs, " TextStyle=\"{tp}\"");
            }
            let _ = writeln!(s, "<StyleSheet {attrs}>");
            write_sheet_contents(&mut s, &st.sheet, &[]);
            s.push_str("</StyleSheet>\n");
        }
        s.push_str("</StyleSheets>\n");
    }
    // DocumentSheet
    s.push_str("<DocumentSheet Name=\"TheDoc\" NameU=\"TheDoc\">\n");
    write_sheet_contents(&mut s, &doc.doc_sheet, &[]);
    s.push_str("</DocumentSheet>\n");

    // HeaderFooter
    let hf = &doc.header_footer;
    let _ = writeln!(
        s,
        "<HeaderFooter HeaderMargin=\"{}\" FooterMargin=\"{}\">",
        format_clean_f64(hf.header_margin_in),
        format_clean_f64(hf.footer_margin_in)
    );
    if !hf.header_left.is_empty() {
        let _ = writeln!(s, "<HeaderLeft>{}</HeaderLeft>", xml_escape(&hf.header_left));
    }
    if !hf.header_center.is_empty() {
        let _ = writeln!(s, "<HeaderCenter>{}</HeaderCenter>", xml_escape(&hf.header_center));
    }
    if !hf.header_right.is_empty() {
        let _ = writeln!(s, "<HeaderRight>{}</HeaderRight>", xml_escape(&hf.header_right));
    }
    if !hf.footer_left.is_empty() {
        let _ = writeln!(s, "<FooterLeft>{}</FooterLeft>", xml_escape(&hf.footer_left));
    }
    if !hf.footer_center.is_empty() {
        let _ = writeln!(s, "<FooterCenter>{}</FooterCenter>", xml_escape(&hf.footer_center));
    }
    if !hf.footer_right.is_empty() {
        let _ = writeln!(s, "<FooterRight>{}</FooterRight>", xml_escape(&hf.footer_right));
    }
    s.push_str("</HeaderFooter>\n");
    s.push_str("</VisioDocument>");
    s
}

fn build_windows_xml(doc: &VisioDocument) -> String {
    let first_page = doc.pages.first().map(|p| p.id).unwrap_or(1);
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Windows xmlns=\"{NS_VISIO_MAIN}\" ClientWidth=\"1440\" ClientHeight=\"900\">\n\
         <Window ID=\"0\" WindowType=\"Drawing\" WindowState=\"1073741824\" Page=\"{first_page}\"/>\n\
         </Windows>"
    )
}

fn build_masters_index_xml(masters: &[VisioMaster]) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Masters xmlns=\"{NS_VISIO_MAIN}\" xmlns:r=\"{NS_RELS_DOC}\">\n"
    );
    for m in masters {
        let _ = writeln!(
            s,
            "<Master ID=\"{}\" Name=\"{}\" NameU=\"{}\" BaseID=\"{}\" UniqueID=\"{}\" Prompt=\"{}\">",
            m.id,
            xml_escape(&m.name),
            xml_escape(&m.name_u),
            xml_escape(&m.base_id),
            xml_escape(&m.unique_id),
            xml_escape(&m.prompt)
        );
        s.push_str("<PageSheet>\n");
        write_sheet_contents(&mut s, &m.page_sheet, &[]);
        s.push_str("</PageSheet>\n");
        let _ = writeln!(s, "<Rel r:id=\"rIdMaster{}\"/>\n</Master>", m.id);
    }
    s.push_str("</Masters>");
    s
}

fn build_masters_rels_xml(masters: &[VisioMaster]) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{NS_RELS_PKG}\">\n"
    );
    for m in masters {
        let _ = writeln!(
            s,
            "<Relationship Id=\"rIdMaster{}\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/master\" Target=\"master{}.xml\"/>",
            m.id, m.id
        );
    }
    s.push_str("</Relationships>");
    s
}

fn build_master_part_xml(master: &VisioMaster) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <MasterContents xmlns=\"{NS_VISIO_MAIN}\" xmlns:r=\"{NS_RELS_DOC}\">\n\
         <Shapes>\n"
    );
    for sh in &master.shapes {
        write_shape_xml(&mut s, sh);
    }
    s.push_str("</Shapes>\n</MasterContents>");
    s
}

fn build_pages_index_xml(pages: &[VisioPage]) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Pages xmlns=\"{NS_VISIO_MAIN}\" xmlns:r=\"{NS_RELS_DOC}\">\n"
    );
    for p in pages {
        let mut attrs = format!(
            "ID=\"{}\" Name=\"{}\" NameU=\"{}\" ViewScale=\"{}\" ViewCenterX=\"{}\" ViewCenterY=\"{}\"",
            p.id,
            xml_escape(&p.name),
            xml_escape(&p.name_u),
            format_clean_f64(p.view_scale),
            format_clean_f64(p.view_center_x),
            format_clean_f64(p.view_center_y),
        );
        if p.is_background {
            attrs.push_str(" Background=\"1\"");
        }
        if let Some(bp) = p.back_page_id {
            let _ = write!(attrs, " BackPage=\"{bp}\"");
        }
        let _ = writeln!(s, "<Page {attrs}>");
        s.push_str("<PageSheet>\n");
        write_sheet_contents(&mut s, &p.page_sheet, &p.layers);
        s.push_str("</PageSheet>\n");
        let _ = writeln!(s, "<Rel r:id=\"rIdPage{}\"/>\n</Page>", p.id);
    }
    s.push_str("</Pages>");
    s
}

fn build_pages_rels_xml(pages: &[VisioPage]) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"{NS_RELS_PKG}\">\n"
    );
    for p in pages {
        let _ = writeln!(
            s,
            "<Relationship Id=\"rIdPage{}\" Type=\"http://schemas.microsoft.com/visio/2010/relationships/page\" Target=\"page{}.xml\"/>",
            p.id, p.id
        );
    }
    s.push_str("</Relationships>");
    s
}

fn build_page_part_xml(page: &VisioPage) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <PageContents xmlns=\"{NS_VISIO_MAIN}\" xmlns:r=\"{NS_RELS_DOC}\">\n"
    );
    if !page.shapes.is_empty() {
        s.push_str("<Shapes>\n");
        for sh in &page.shapes {
            write_shape_xml(&mut s, sh);
        }
        s.push_str("</Shapes>\n");
    }
    if !page.connects.is_empty() {
        s.push_str("<Connects>\n");
        for c in &page.connects {
            let _ = writeln!(
                s,
                "<Connect FromSheet=\"{}\" FromCell=\"{}\" ToSheet=\"{}\" ToCell=\"{}\"/>",
                c.connector_shape_id,
                xml_escape(&c.from_cell),
                c.target_shape_id,
                xml_escape(&c.to_cell)
            );
        }
        s.push_str("</Connects>\n");
    }
    s.push_str("</PageContents>");
    s
}

fn write_shape_xml(out: &mut String, shape: &VisioShape) {
    let type_str = match shape.kind {
        ShapeKind::Group => "Group",
        ShapeKind::ForeignObject => "Foreign",
        ShapeKind::Guide => "Guide",
        ShapeKind::Shape2D | ShapeKind::Connector1D => "Shape",
    };
    let mut attrs = format!(
        "ID=\"{}\" Name=\"{}\" NameU=\"{}\" Type=\"{type_str}\"",
        shape.id,
        xml_escape(&shape.name),
        xml_escape(&shape.name_u),
    );
    if let Some(m) = shape.master_id {
        let _ = write!(attrs, " Master=\"{m}\"");
    }
    if let Some(ms) = shape.master_shape_id {
        let _ = write!(attrs, " MasterShape=\"{ms}\"");
    }
    if let Some(ls) = shape.line_style_id {
        let _ = write!(attrs, " LineStyle=\"{ls}\"");
    }
    if let Some(fs) = shape.fill_style_id {
        let _ = write!(attrs, " FillStyle=\"{fs}\"");
    }
    if let Some(ts) = shape.text_style_id {
        let _ = write!(attrs, " TextStyle=\"{ts}\"");
    }
    let _ = writeln!(out, "<Shape {attrs}>");

    // LayerMember cell if assigned
    if !shape.layer_membership.is_empty() && shape.sheet.get_cell("LayerMember").is_none() {
        let joined: Vec<String> = shape.layer_membership.iter().map(|i| i.to_string()).collect();
        let _ = writeln!(
            out,
            "<Cell N=\"LayerMember\" V=\"{}\" U=\"STR\"/>",
            joined.join(";")
        );
    }

    write_sheet_contents(out, &shape.sheet, &[]);

    if !shape.text.is_empty() {
        let _ = writeln!(out, "<Text>{}</Text>", xml_escape(&shape.text));
    }

    if let Some(fd) = &shape.foreign_data {
        let _ = writeln!(
            out,
            "<ForeignData ForeignType=\"Bitmap\" CompressionType=\"{}\">",
            xml_escape(&fd.media_type)
        );
        if let Some(rid) = &fd.rel_id {
            let _ = writeln!(out, "<Rel r:id=\"{}\"/>", xml_escape(rid));
        } else if !fd.data.is_empty() {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&fd.data);
            out.push_str(&encoded);
        }
        out.push_str("</ForeignData>\n");
    }

    for frag in &shape.opaque_xml_fragments {
        if !shape.text.is_empty() && frag.trim() == "<Text/>" {
            continue;
        }
        out.push_str(frag);
        out.push('\n');
    }

    if !shape.children.is_empty() {
        out.push_str("<Shapes>\n");
        for c in &shape.children {
            write_shape_xml(out, c);
        }
        out.push_str("</Shapes>\n");
    }

    out.push_str("</Shape>\n");
}

fn is_section_synthesized_cell(key_lower: &str) -> bool {
    key_lower.starts_with("geometry")
        || key_lower.starts_with("connections.")
        || key_lower.starts_with("controls.")
        || key_lower.starts_with("user.")
        || key_lower.starts_with("prop.")
        || key_lower.starts_with("fillgradient.")
}

fn write_sheet_contents(out: &mut String, sheet: &ShapeSheet, layers: &[LayerDef]) {
    // 1. Top-level singleton cells
    for (k, cell) in &sheet.cells {
        if cell.deleted || is_section_synthesized_cell(k) {
            continue;
        }
        write_cell_tag(out, &cell.name, &cell.formula, Some(&cell.value));
    }

    // 2. Layer section (on PageSheet)
    if !layers.is_empty() {
        out.push_str("<Section N=\"Layer\">\n");
        for l in layers {
            let _ = writeln!(out, "<Row IX=\"{}\">", l.ix);
            let _ = writeln!(out, "<Cell N=\"Name\" V=\"{}\" U=\"STR\"/>", xml_escape(&l.name));
            let _ = writeln!(out, "<Cell N=\"NameUniv\" V=\"{}\" U=\"STR\"/>", xml_escape(&l.name_u));
            let _ = writeln!(out, "<Cell N=\"Visible\" V=\"{}\" U=\"BOOL\"/>", if l.visible { 1 } else { 0 });
            let _ = writeln!(out, "<Cell N=\"Print\" V=\"{}\" U=\"BOOL\"/>", if l.print { 1 } else { 0 });
            let _ = writeln!(out, "<Cell N=\"Active\" V=\"{}\" U=\"BOOL\"/>", if l.active { 1 } else { 0 });
            let _ = writeln!(out, "<Cell N=\"Lock\" V=\"{}\" U=\"BOOL\"/>", if l.lock { 1 } else { 0 });
            let _ = writeln!(out, "<Cell N=\"Snap\" V=\"{}\" U=\"BOOL\"/>", if l.snap { 1 } else { 0 });
            let _ = writeln!(out, "<Cell N=\"Glue\" V=\"{}\" U=\"BOOL\"/>", if l.glue { 1 } else { 0 });
            if let Some(c) = l.color_override {
                let _ = writeln!(out, "<Cell N=\"Color\" V=\"{}\" U=\"RGB\"/>", c.to_hex_rgb());
            }
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 3. User-defined cells (`User.*`)
    if !sheet.user_cells.is_empty() {
        out.push_str("<Section N=\"User\">\n");
        for u in sheet.user_cells.values() {
            let _ = writeln!(out, "<Row N=\"{}\">", xml_escape(&u.name));
            let resolved = sheet.get_cell(&format!("User.{}", u.name)).map(|c| &c.value);
            write_cell_tag(out, "Value", &u.value, resolved);
            let _ = writeln!(out, "<Cell N=\"Prompt\" V=\"{}\" U=\"STR\"/>", xml_escape(&u.prompt));
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 4. Shape Data (`Prop.*`)
    if !sheet.shape_data.is_empty() {
        out.push_str("<Section N=\"Property\">\n");
        for p in sheet.shape_data.values() {
            let _ = writeln!(out, "<Row N=\"{}\">", xml_escape(&p.name));
            let resolved = sheet.get_cell(&format!("Prop.{}", p.name)).map(|c| &c.value);
            write_cell_tag(out, "Value", &p.value, resolved);
            let _ = writeln!(out, "<Cell N=\"Label\" V=\"{}\" U=\"STR\"/>", xml_escape(&p.label));
            let _ = writeln!(out, "<Cell N=\"Prompt\" V=\"{}\" U=\"STR\"/>", xml_escape(&p.prompt));
            let _ = writeln!(out, "<Cell N=\"Format\" V=\"{}\" U=\"STR\"/>", xml_escape(&p.format));
            let dtype_num = match p.data_type {
                ShapeDataType::String => 0,
                ShapeDataType::FixedList => 1,
                ShapeDataType::Number => 2,
                ShapeDataType::Boolean => 3,
                ShapeDataType::VariableList => 4,
                ShapeDataType::DateTime => 5,
                ShapeDataType::Duration => 6,
                ShapeDataType::Currency => 7,
            };
            let _ = writeln!(out, "<Cell N=\"Type\" V=\"{dtype_num}\" U=\"NUM\"/>");
            let _ = writeln!(
                out,
                "<Cell N=\"Invisible\" V=\"{}\" U=\"BOOL\"/>",
                if p.invisible { 1 } else { 0 }
            );
            let _ = writeln!(
                out,
                "<Cell N=\"Ask\" V=\"{}\" U=\"BOOL\"/>",
                if p.ask { 1 } else { 0 }
            );
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 5. Connection Points
    if !sheet.connection_points.is_empty() {
        out.push_str("<Section N=\"Connection\">\n");
        for cp in &sheet.connection_points {
            if cp.name.is_empty() {
                let _ = writeln!(out, "<Row IX=\"{}\">", cp.ix);
            } else {
                let _ = writeln!(out, "<Row IX=\"{}\" N=\"{}\">", cp.ix, xml_escape(&cp.name));
            }
            write_cell_tag(
                out,
                "X",
                &cp.x,
                sheet.get_cell(&format!("Connections.X{}", cp.ix)).map(|c| &c.value),
            );
            write_cell_tag(
                out,
                "Y",
                &cp.y,
                sheet.get_cell(&format!("Connections.Y{}", cp.ix)).map(|c| &c.value),
            );
            write_cell_tag(out, "DirX", &cp.dir_x, None);
            write_cell_tag(out, "DirY", &cp.dir_y, None);
            let _ = writeln!(out, "<Cell N=\"Type\" V=\"{}\" U=\"NUM\"/>", cp.point_type);
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 6. Controls
    if !sheet.controls.is_empty() {
        out.push_str("<Section N=\"Controls\">\n");
        for (i, ctrl) in sheet.controls.iter().enumerate() {
            let _ = writeln!(out, "<Row IX=\"{}\" N=\"{}\">", i + 1, xml_escape(&ctrl.name));
            write_cell_tag(
                out,
                "X",
                &ctrl.x,
                sheet.get_cell(&format!("Controls.{}.X", ctrl.name)).map(|c| &c.value),
            );
            write_cell_tag(
                out,
                "Y",
                &ctrl.y,
                sheet.get_cell(&format!("Controls.{}.Y", ctrl.name)).map(|c| &c.value),
            );
            write_cell_tag(out, "XDyn", &ctrl.x_dyn, None);
            write_cell_tag(out, "YDyn", &ctrl.y_dyn, None);
            write_cell_tag(out, "XCon", &ctrl.x_con, None);
            write_cell_tag(out, "YCon", &ctrl.y_con, None);
            let _ = writeln!(
                out,
                "<Cell N=\"CanGlue\" V=\"{}\" U=\"BOOL\"/>",
                if ctrl.can_glue { 1 } else { 0 }
            );
            let _ = writeln!(out, "<Cell N=\"Prompt\" V=\"{}\" U=\"STR\"/>", xml_escape(&ctrl.prompt));
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 7. FillGradient Stops (OpenXML `IX` is 0-based; `GradientStopRow.ix` may be 0-based or 1-based)
    if !sheet.fill_gradient_stops.is_empty() {
        out.push_str("<Section N=\"FillGradient\">\n");
        let is_zero_based = sheet.fill_gradient_stops.iter().any(|s| s.ix == 0);
        for (seq_idx, stop) in sheet.fill_gradient_stops.iter().enumerate() {
            let zero_ix = if is_zero_based {
                stop.ix.max(seq_idx as u32)
            } else {
                stop.ix.saturating_sub(1).max(seq_idx as u32)
            };
            let _ = writeln!(out, "<Row IX=\"{zero_ix}\">");
            write_cell_tag(
                out,
                "GradientStopPosition",
                &stop.position,
                sheet
                    .get_cell(&format!("FillGradient.Stop{}.Position", stop.ix))
                    .map(|c| &c.value),
            );
            write_cell_tag(
                out,
                "GradientStopColor",
                &stop.color,
                sheet
                    .get_cell(&format!("FillGradient.Stop{}.Color", stop.ix))
                    .map(|c| &c.value),
            );
            write_cell_tag(
                out,
                "GradientStopColorTrans",
                &stop.color_trans,
                sheet
                    .get_cell(&format!("FillGradient.Stop{}.ColorTrans", stop.ix))
                    .map(|c| &c.value),
            );
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 7. Actions & Hyperlinks
    if !sheet.actions.is_empty() {
        out.push_str("<Section N=\"Action\">\n");
        for act in &sheet.actions {
            let _ = writeln!(out, "<Row N=\"{}\">", xml_escape(&act.name));
            write_cell_tag(out, "Action", &act.action, None);
            let _ = writeln!(out, "<Cell N=\"Menu\" V=\"{}\" U=\"STR\"/>", xml_escape(&act.menu));
            let _ = writeln!(out, "<Cell N=\"TagName\" V=\"{}\" U=\"STR\"/>", xml_escape(&act.tag_name));
            write_cell_tag(out, "Checked", &act.checked, None);
            write_cell_tag(out, "Disabled", &act.disabled, None);
            write_cell_tag(out, "Invisible", &act.invisible, None);
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }
    if !sheet.hyperlinks.is_empty() {
        out.push_str("<Section N=\"Hyperlink\">\n");
        for hl in &sheet.hyperlinks {
            let _ = writeln!(out, "<Row N=\"{}\">", xml_escape(&hl.name));
            let _ = writeln!(
                out,
                "<Cell N=\"Description\" V=\"{}\" U=\"STR\"/>",
                xml_escape(&hl.description)
            );
            let _ = writeln!(out, "<Cell N=\"Address\" V=\"{}\" U=\"STR\"/>", xml_escape(&hl.address));
            let _ = writeln!(
                out,
                "<Cell N=\"SubAddress\" V=\"{}\" U=\"STR\"/>",
                xml_escape(&hl.sub_address)
            );
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }

    // 8. Geometry[1..N]
    for geom in &sheet.geometries {
        let ix_zero_based = geom.index.saturating_sub(1);
        let del_attr = if geom.deleted { " Del=\"1\"" } else { "" };
        let _ = writeln!(out, "<Section N=\"Geometry\" IX=\"{ix_zero_based}\"{del_attr}>");
        let prefix = format!("Geometry{}", geom.index);
        write_cell_tag(out, "NoFill", &geom.no_fill, sheet.get_cell(&format!("{prefix}.NoFill")).map(|c| &c.value));
        write_cell_tag(out, "NoLine", &geom.no_line, sheet.get_cell(&format!("{prefix}.NoLine")).map(|c| &c.value));
        write_cell_tag(out, "NoShow", &geom.no_show, sheet.get_cell(&format!("{prefix}.NoShow")).map(|c| &c.value));
        write_cell_tag(out, "NoSnap", &geom.no_snap, sheet.get_cell(&format!("{prefix}.NoSnap")).map(|c| &c.value));
        for row in &geom.rows {
            let rdel = if row.deleted { " Del=\"1\"" } else { "" };
            let _ = writeln!(out, "<Row T=\"{}\" IX=\"{}\"{rdel}>", row.kind.vsdx_tag(), row.ix);
            write_cell_tag(
                out,
                "X",
                &row.x,
                sheet.get_cell(&format!("{prefix}.X{}", row.ix)).map(|c| &c.value),
            );
            write_cell_tag(
                out,
                "Y",
                &row.y,
                sheet.get_cell(&format!("{prefix}.Y{}", row.ix)).map(|c| &c.value),
            );
            if let Some(a) = &row.a {
                write_cell_tag(
                    out,
                    "A",
                    a,
                    sheet.get_cell(&format!("{prefix}.A{}", row.ix)).map(|c| &c.value),
                );
            }
            if let Some(b) = &row.b {
                write_cell_tag(
                    out,
                    "B",
                    b,
                    sheet.get_cell(&format!("{prefix}.B{}", row.ix)).map(|c| &c.value),
                );
            }
            if let Some(c) = &row.c {
                write_cell_tag(
                    out,
                    "C",
                    c,
                    sheet.get_cell(&format!("{prefix}.C{}", row.ix)).map(|c| &c.value),
                );
            }
            if let Some(d) = &row.d {
                write_cell_tag(
                    out,
                    "D",
                    d,
                    sheet.get_cell(&format!("{prefix}.D{}", row.ix)).map(|c| &c.value),
                );
            }
            if let Some(e) = &row.e {
                write_cell_tag(
                    out,
                    "E",
                    e,
                    sheet.get_cell(&format!("{prefix}.E{}", row.ix)).map(|c| &c.value),
                );
            }
            out.push_str("</Row>\n");
        }
        out.push_str("</Section>\n");
    }
}

fn write_cell_tag(
    out: &mut String,
    name: &str,
    formula: &str,
    val: Option<&vexio_shapesheet::CellValue>,
) {
    let trimmed_f = formula.trim();
    let f_attr = if let Some(expr) = trimmed_f.strip_prefix('=') {
        Some(expr)
    } else if vexio_shapesheet::parse_scalar_with_unit(trimmed_f).is_none()
        && !trimmed_f.eq_ignore_ascii_case("TRUE")
        && !trimmed_f.eq_ignore_ascii_case("FALSE")
        && !(trimmed_f.starts_with('"') && trimmed_f.ends_with('"'))
    {
        Some(trimmed_f)
    } else {
        None
    };

    if let Some(cv) = val {
        let v_str = xml_escape(&cv.to_vsdx_val_string());
        let u_str = cv.unit().vsdx_code();
        if let Some(f) = f_attr {
            let _ = writeln!(
                out,
                "<Cell N=\"{}\" V=\"{v_str}\" U=\"{u_str}\" F=\"{}\"/>",
                xml_escape(name),
                xml_escape(f)
            );
        } else {
            let _ = writeln!(
                out,
                "<Cell N=\"{}\" V=\"{v_str}\" U=\"{u_str}\"/>",
                xml_escape(name)
            );
        }
    } else if let Some(f) = f_attr {
        let _ = writeln!(
            out,
            "<Cell N=\"{}\" F=\"{}\"/>",
            xml_escape(name),
            xml_escape(f)
        );
    } else if let Some((scalar, unit)) = vexio_shapesheet::parse_scalar_with_unit(trimmed_f) {
        let internal = unit.to_internal(scalar);
        let _ = writeln!(
            out,
            "<Cell N=\"{}\" V=\"{}\" U=\"{}\"/>",
            xml_escape(name),
            format_clean_f64(internal),
            unit.vsdx_code()
        );
    } else if trimmed_f.eq_ignore_ascii_case("TRUE") || trimmed_f.eq_ignore_ascii_case("FALSE") {
        let b = if trimmed_f.eq_ignore_ascii_case("TRUE") { "1" } else { "0" };
        let _ = writeln!(out, "<Cell N=\"{}\" V=\"{b}\" U=\"BOOL\"/>", xml_escape(name));
    } else {
        let unquoted = trimmed_f.trim_matches('"').replace("\"\"", "\"");
        let _ = writeln!(
            out,
            "<Cell N=\"{}\" V=\"{}\" U=\"STR\"/>",
            xml_escape(name),
            xml_escape(&unquoted)
        );
    }
}

fn build_comments_xml(doc: &VisioDocument) -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Comments xmlns=\"{NS_VISIO_MAIN}\">\n\
         <Authors>\n"
    );
    for (idx, c) in doc.comments.iter().enumerate() {
        let _ = writeln!(
            s,
            "<AuthorEntry ID=\"{idx}\" Name=\"{}\" Initials=\"{}\"/>",
            xml_escape(&c.author),
            xml_escape(&c.initials)
        );
    }
    s.push_str("</Authors>\n<CommentList>\n");
    for (idx, c) in doc.comments.iter().enumerate() {
        let shape_attr = c
            .shape_id
            .map(|sid| format!(" ShapeID=\"{sid}\""))
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "<CommentEntry AuthorID=\"{idx}\" PageID=\"{}\"{shape_attr} Date=\"{}\" CommentID=\"{}\" Done=\"{}\">{}</CommentEntry>",
            c.page_id,
            xml_escape(&c.timestamp_iso),
            c.id,
            if c.resolved { 1 } else { 0 },
            xml_escape(&c.text)
        );
    }
    s.push_str("</CommentList>\n</Comments>");
    s
}
