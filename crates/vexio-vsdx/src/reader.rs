//! Streaming OpenXML Visio reader (`.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, `.vstm`).

use crate::opc::{RawOpcArchive, VsdxError, VsdxPackageKind, is_managed_opc_part};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::BTreeMap;
use vexio_shapesheet::{
    ActionRow, CommentAnnotation, ConnectionPointRow, ControlRow, ForeignMediaData, GeometryRow,
    GeometryRowKind, GeometrySection, GlueEndpoint, HyperlinkRow, LayerDef, ReactiveEngine,
    RgbaColor, ShapeDataRow, ShapeDataType, ShapeKind, ShapeSheet, StencilPack, StyleSheetDef,
    TextFieldRow, Unit, VisioDocument, VisioMaster, VisioPage, VisioShape,
};

pub fn read_vsdx(bytes: &[u8]) -> Result<(VisioDocument, VsdxPackageKind), VsdxError> {
    let archive = RawOpcArchive::from_bytes(bytes)?;
    let mut doc = VisioDocument::new_blank("Imported");
    doc.pages.clear();
    doc.styles.clear();
    doc.masters.clear();

    // 1. Inspect `[Content_Types].xml` to determine package kind and preserve custom overrides.
    let mut pkg_kind = VsdxPackageKind::Drawing;
    if let Some(ct_xml) = archive.get_str("[Content_Types].xml")? {
        parse_content_types(&ct_xml, &mut pkg_kind, &mut doc);
    }
    doc.macro_enabled = pkg_kind.is_macro_enabled()
        || archive.files.contains_key("visio/vbaproject.bin")
        || archive.files.contains_key("visio/vbaProject.bin");

    // 2. Preserve all unmanaged parts (vbaProject.bin, media, embeddings, customXml, signatures) verbatim!
    for (path, data) in &archive.files {
        if !is_managed_opc_part(path) {
            doc.opaque_bag.parts.insert(path.clone(), data.clone());
        }
    }

    // 3. Parse `docProps/core.xml` and `docProps/custom.xml`
    if let Some(core_xml) = archive.get_str("docProps/core.xml")? {
        parse_core_props(&core_xml, &mut doc);
    }
    if let Some(custom_xml) = archive.get_str("docProps/custom.xml")? {
        parse_custom_props(&custom_xml, &mut doc);
    }

    // 4. Parse `visio/document.xml` (Styles, DocumentSheet, HeaderFooter)
    if let Some(doc_xml) = archive.get_str("visio/document.xml")? {
        parse_document_xml(&doc_xml, &mut doc)?;
    }
    if doc.styles.is_empty() {
        doc.styles.push(StyleSheetDef {
            id: 0,
            name: "Normal".into(),
            name_u: "Normal".into(),
            line_parent: None,
            fill_parent: None,
            text_parent: None,
            sheet: ShapeSheet::new_2d(0.0, 0.0, 1.0, 1.0),
        });
    }

    // 5. Parse Masters (`visio/masters/masters.xml` + `visio/masters/masterN.xml`)
    let master_rels = archive
        .get_str("visio/masters/_rels/masters.xml.rels")?
        .map(|s| parse_rels(&s))
        .unwrap_or_default();
    if let Some(masters_xml) = archive.get_str("visio/masters/masters.xml")? {
        parse_masters_index(&masters_xml, &master_rels, &archive, &mut doc)?;
    }

    // 6. Parse Pages (`visio/pages/pages.xml` + `visio/pages/pageN.xml`)
    let page_rels = archive
        .get_str("visio/pages/_rels/pages.xml.rels")?
        .map(|s| parse_rels(&s))
        .unwrap_or_default();
    if let Some(pages_xml) = archive.get_str("visio/pages/pages.xml")? {
        parse_pages_index(&pages_xml, &page_rels, &archive, &mut doc)?;
    }
    if doc.pages.is_empty() && !matches!(pkg_kind, VsdxPackageKind::Stencil | VsdxPackageKind::MacroStencil) {
        doc.pages.push(VisioPage::new(1, "Page-1", 11.0, 8.5));
    }

    // 7. Parse Comments (`visio/comments.xml`)
    if let Some(comments_xml) = archive.get_str("visio/comments.xml")? {
        parse_comments_xml(&comments_xml, &mut doc);
    }

    // 8. Recalculate reactive ShapeSheet DAG across all sheets
    ReactiveEngine::recalculate_document(&mut doc);
    Ok((doc, pkg_kind))
}

pub fn read_stencil_pack(bytes: &[u8]) -> Result<StencilPack, VsdxError> {
    let (doc, _) = read_vsdx(bytes)?;
    Ok(StencilPack {
        title: if doc.properties.title.is_empty() {
            "Custom Stencil".into()
        } else {
            doc.properties.title
        },
        category: doc.properties.category,
        masters: doc.masters,
    })
}

fn attr_str(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key || a.key.local_name().as_ref() == key {
            if let Ok(v) = a.unescape_value() {
                return Some(v.into_owned());
            }
        }
    }
    None
}

fn parse_content_types(xml: &str, kind: &mut VsdxPackageKind, doc: &mut VisioDocument) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == b"Override" => {
                let part = attr_str(&e, b"PartName").unwrap_or_default();
                let ct = attr_str(&e, b"ContentType").unwrap_or_default();
                if part == "/visio/document.xml" {
                    *kind = VsdxPackageKind::from_main_content_type(&ct);
                } else if !is_managed_opc_part(&part) {
                    doc.opaque_bag.content_type_overrides.insert(part, ct);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_rels(xml: &str) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(target)) = (attr_str(&e, b"Id"), attr_str(&e, b"Target")) {
                    map.insert(id, target);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    map
}

fn parse_core_props(xml: &str, doc: &mut VisioDocument) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current_tag = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                current_tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
            }
            Ok(Event::Text(t)) => {
                if let Ok(val) = t.unescape() {
                    match current_tag.as_str() {
                        "title" => doc.properties.title = val.into_owned(),
                        "subject" => doc.properties.subject = val.into_owned(),
                        "creator" => doc.properties.creator = val.into_owned(),
                        "description" => doc.properties.description = val.into_owned(),
                        "category" => doc.properties.category = val.into_owned(),
                        "keywords" => doc.properties.keywords = val.into_owned(),
                        "created" => doc.properties.created_iso = val.into_owned(),
                        "modified" => doc.properties.modified_iso = val.into_owned(),
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => current_tag.clear(),
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_custom_props(xml: &str, doc: &mut VisioDocument) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut current_prop_name: Option<String> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"property" => {
                current_prop_name = attr_str(&e, b"name");
            }
            Ok(Event::Text(t)) => {
                if let (Some(name), Ok(val)) = (&current_prop_name, t.unescape()) {
                    doc.properties
                        .custom_properties
                        .insert(name.clone(), val.into_owned());
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"property" => {
                current_prop_name = None;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn parse_document_xml(xml: &str, doc: &mut VisioDocument) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut in_header_footer = false;
    let mut hf_tag = String::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"StyleSheet" => {
                    let id = attr_str(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let name = attr_str(&e, b"Name").unwrap_or_else(|| format!("Style.{id}"));
                    let name_u = attr_str(&e, b"NameU").unwrap_or_else(|| name.clone());
                    let line_parent = attr_str(&e, b"LineStyle").and_then(|s| s.parse().ok());
                    let fill_parent = attr_str(&e, b"FillStyle").and_then(|s| s.parse().ok());
                    let text_parent = attr_str(&e, b"TextStyle").and_then(|s| s.parse().ok());
                    let mut sheet = ShapeSheet::new();
                    let mut layers = Vec::new();
                    parse_sheet_body(&mut reader, b"StyleSheet", &mut sheet, &mut layers)?;
                    doc.styles.push(StyleSheetDef {
                        id,
                        name,
                        name_u,
                        line_parent,
                        fill_parent,
                        text_parent,
                        sheet,
                    });
                }
                b"DocumentSheet" => {
                    let mut layers = Vec::new();
                    parse_sheet_body(&mut reader, b"DocumentSheet", &mut doc.doc_sheet, &mut layers)?;
                }
                b"HeaderFooter" => {
                    in_header_footer = true;
                    if let Some(hm) = attr_str(&e, b"HeaderMargin").and_then(|s| s.parse().ok()) {
                        doc.header_footer.header_margin_in = hm;
                    }
                    if let Some(fm) = attr_str(&e, b"FooterMargin").and_then(|s| s.parse().ok()) {
                        doc.header_footer.footer_margin_in = fm;
                    }
                }
                other if in_header_footer => {
                    hf_tag = String::from_utf8_lossy(other).into_owned();
                }
                _ => {}
            },
            Event::Text(t) if in_header_footer && !hf_tag.is_empty() => {
                if let Ok(val) = t.unescape() {
                    match hf_tag.as_str() {
                        "HeaderLeft" => doc.header_footer.header_left = val.into_owned(),
                        "HeaderCenter" => doc.header_footer.header_center = val.into_owned(),
                        "HeaderRight" => doc.header_footer.header_right = val.into_owned(),
                        "FooterLeft" => doc.header_footer.footer_left = val.into_owned(),
                        "FooterCenter" => doc.header_footer.footer_center = val.into_owned(),
                        "FooterRight" => doc.header_footer.footer_right = val.into_owned(),
                        _ => {}
                    }
                }
            }
            Event::End(e) => {
                if e.local_name().as_ref() == b"HeaderFooter" {
                    in_header_footer = false;
                } else if in_header_footer {
                    hf_tag.clear();
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_masters_index(
    xml: &str,
    rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    doc: &mut VisioDocument,
) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut current_master: Option<VisioMaster> = None;
    let mut current_rel_id: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) if e.local_name().as_ref() == b"Master" => {
                let id = attr_str(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
                let name = attr_str(&e, b"Name").unwrap_or_else(|| format!("Master.{id}"));
                let name_u = attr_str(&e, b"NameU").unwrap_or_else(|| name.clone());
                current_master = Some(VisioMaster {
                    id,
                    name,
                    name_u,
                    base_id: attr_str(&e, b"BaseID").unwrap_or_default(),
                    unique_id: attr_str(&e, b"UniqueID").unwrap_or_default(),
                    prompt: attr_str(&e, b"Prompt").unwrap_or_default(),
                    icon_bytes: None,
                    page_sheet: ShapeSheet::new(),
                    shapes: Vec::new(),
                });
                current_rel_id = None;
            }
            Event::Empty(e) if e.local_name().as_ref() == b"Master" => {
                let id = attr_str(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
                let name = attr_str(&e, b"Name").unwrap_or_else(|| format!("Master.{id}"));
                let name_u = attr_str(&e, b"NameU").unwrap_or_else(|| name.clone());
                let mut m = VisioMaster {
                    id,
                    name,
                    name_u,
                    base_id: attr_str(&e, b"BaseID").unwrap_or_default(),
                    unique_id: attr_str(&e, b"UniqueID").unwrap_or_default(),
                    prompt: attr_str(&e, b"Prompt").unwrap_or_default(),
                    icon_bytes: None,
                    page_sheet: ShapeSheet::new(),
                    shapes: Vec::new(),
                };
                let part_path = format!("visio/masters/master{id}.xml");
                if let Some(m_xml) = archive.get_str(&part_path)? {
                    parse_master_part(&m_xml, &mut m)?;
                }
                doc.masters.push(m);
            }
            Event::Empty(e) | Event::Start(e)
                if current_master.is_some() && e.local_name().as_ref() == b"Rel" =>
            {
                current_rel_id = attr_str(&e, b"id");
            }
            Event::Start(e)
                if current_master.is_some() && e.local_name().as_ref() == b"PageSheet" =>
            {
                if let Some(m) = current_master.as_mut() {
                    let mut dummy_layers = Vec::new();
                    parse_sheet_body(&mut reader, b"PageSheet", &mut m.page_sheet, &mut dummy_layers)?;
                }
            }
            Event::End(e) if e.local_name().as_ref() == b"Master" => {
                if let Some(mut m) = current_master.take() {
                    let target = current_rel_id
                        .take()
                        .and_then(|rid| rels.get(&rid).cloned())
                        .unwrap_or_else(|| format!("master{}.xml", m.id));
                    let full_path = resolve_visio_subpath("visio/masters", &target);
                    if let Some(m_xml) = archive.get_str(&full_path)? {
                        parse_master_part(&m_xml, &mut m)?;
                    }
                    doc.masters.push(m);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_master_part(xml: &str, master: &mut VisioMaster) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) if e.local_name().as_ref() == b"Shape" => {
                let shape = parse_shape(&mut reader, &e)?;
                master.shapes.push(shape);
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_pages_index(
    xml: &str,
    rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    doc: &mut VisioDocument,
) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut current_page: Option<VisioPage> = None;
    let mut current_rel_id: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) if e.local_name().as_ref() == b"Page" => {
                let id = attr_str(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
                let name = attr_str(&e, b"Name").unwrap_or_else(|| format!("Page-{id}"));
                let name_u = attr_str(&e, b"NameU").unwrap_or_else(|| name.clone());
                let mut page = VisioPage::new(id, name, 11.0, 8.5);
                page.name_u = name_u;
                page.layers.clear();
                page.is_background = attr_str(&e, b"Background")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                page.back_page_id = attr_str(&e, b"BackPage").and_then(|s| s.parse().ok());
                if let Some(vs) = attr_str(&e, b"ViewScale").and_then(|s| s.parse().ok()) {
                    page.view_scale = vs;
                }
                if let Some(vx) = attr_str(&e, b"ViewCenterX").and_then(|s| s.parse().ok()) {
                    page.view_center_x = vx;
                }
                if let Some(vy) = attr_str(&e, b"ViewCenterY").and_then(|s| s.parse().ok()) {
                    page.view_center_y = vy;
                }
                current_page = Some(page);
                current_rel_id = None;
            }
            Event::Start(e)
                if current_page.is_some() && e.local_name().as_ref() == b"PageSheet" =>
            {
                if let Some(p) = current_page.as_mut() {
                    parse_sheet_body(&mut reader, b"PageSheet", &mut p.page_sheet, &mut p.layers)?;
                }
            }
            Event::Empty(e) | Event::Start(e)
                if current_page.is_some() && e.local_name().as_ref() == b"Rel" =>
            {
                current_rel_id = attr_str(&e, b"id");
            }
            Event::End(e) if e.local_name().as_ref() == b"Page" => {
                if let Some(mut page) = current_page.take() {
                    let target = current_rel_id
                        .take()
                        .and_then(|rid| rels.get(&rid).cloned())
                        .unwrap_or_else(|| format!("page{}.xml", page.id));
                    let full_path = resolve_visio_subpath("visio/pages", &target);
                    if let Some(p_xml) = archive.get_str(&full_path)? {
                        parse_page_part(&p_xml, &mut page)?;
                    }
                    doc.pages.push(page);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_page_part(xml: &str, page: &mut VisioPage) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"PageSheet" => {
                    parse_sheet_body(&mut reader, b"PageSheet", &mut page.page_sheet, &mut page.layers)?;
                }
                b"Shape" => {
                    let shape = parse_shape(&mut reader, &e)?;
                    page.shapes.push(shape);
                }
                b"Connect" => {
                    page.connects.push(parse_connect_elem(&e));
                }
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"Connect" => {
                page.connects.push(parse_connect_elem(&e));
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_connect_elem(e: &BytesStart<'_>) -> GlueEndpoint {
    let from_sheet = attr_str(e, b"FromSheet").and_then(|s| s.parse().ok()).unwrap_or(0);
    let from_cell = attr_str(e, b"FromCell").unwrap_or_else(|| "BeginX".into());
    let to_sheet = attr_str(e, b"ToSheet").and_then(|s| s.parse().ok()).unwrap_or(0);
    let to_cell = attr_str(e, b"ToCell").unwrap_or_else(|| "PinX".into());
    let is_dynamic = to_cell.eq_ignore_ascii_case("PinX") || to_cell.eq_ignore_ascii_case("PinY");
    GlueEndpoint {
        connector_shape_id: from_sheet,
        from_cell,
        target_shape_id: to_sheet,
        to_cell,
        is_dynamic,
    }
}

fn parse_shape(reader: &mut Reader<&[u8]>, start: &BytesStart<'_>) -> Result<VisioShape, VsdxError> {
    let id = attr_str(start, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
    let name = attr_str(start, b"Name").unwrap_or_else(|| format!("Sheet.{id}"));
    let name_u = attr_str(start, b"NameU").unwrap_or_else(|| name.clone());
    let type_str = attr_str(start, b"Type").unwrap_or_else(|| "Shape".into());
    let mut kind = match type_str.as_str() {
        "Group" => ShapeKind::Group,
        "Foreign" => ShapeKind::ForeignObject,
        "Guide" => ShapeKind::Guide,
        _ => ShapeKind::Shape2D,
    };
    let master_id = attr_str(start, b"Master").and_then(|s| s.parse().ok());
    let master_shape_id = attr_str(start, b"MasterShape").and_then(|s| s.parse().ok());
    let line_style_id = attr_str(start, b"LineStyle").and_then(|s| s.parse().ok());
    let fill_style_id = attr_str(start, b"FillStyle").and_then(|s| s.parse().ok());
    let text_style_id = attr_str(start, b"TextStyle").and_then(|s| s.parse().ok());

    let mut sheet = ShapeSheet::new();
    let mut children = Vec::new();
    let mut text = String::new();
    let mut foreign_data = None;
    let mut opaque_xml_fragments = Vec::new();
    let mut dummy_layers = Vec::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Cell" => {
                    parse_cell_elem(reader, &e, false, &mut sheet)?;
                }
                b"Section" => {
                    parse_section_elem(reader, &e, &mut sheet, &mut dummy_layers)?;
                }
                b"Text" => {
                    text = read_text_element(reader)?;
                }
                b"Shapes" => {
                    // Container for group child `<Shape>` elements
                }
                b"Shape" => {
                    children.push(parse_shape(reader, &e)?);
                }
                b"ForeignData" => {
                    let ftype = attr_str(&e, b"ForeignType").unwrap_or_else(|| "Bitmap".into());
                    let ctype = attr_str(&e, b"CompressionType").unwrap_or_else(|| ftype.clone());
                    let (rel_id, raw_b64) = read_foreign_data_body(reader)?;
                    let decoded = if !raw_b64.trim().is_empty() {
                        use base64::Engine;
                        base64::engine::general_purpose::STANDARD
                            .decode(raw_b64.trim())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    foreign_data = Some(ForeignMediaData {
                        media_type: ctype,
                        rel_id,
                        data: decoded,
                    });
                }
                other => {
                    // Preserve unknown XML subtree verbatim!
                    let tag_name = String::from_utf8_lossy(other).into_owned();
                    let raw = capture_raw_element(reader, &e, &tag_name)?;
                    opaque_xml_fragments.push(raw);
                }
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"Cell" => {
                    parse_cell_elem(reader, &e, true, &mut sheet)?;
                }
                _ => {}
            },
            Event::End(e) if e.local_name().as_ref() == b"Shape" => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if kind == ShapeKind::Shape2D && sheet.get_cell("BeginX").is_some() && sheet.get_cell("EndX").is_some() {
        kind = ShapeKind::Connector1D;
    }

    let mut layer_membership = Vec::new();
    if let Some(lm_cell) = sheet.get_cell("LayerMember") {
        if let Ok(s) = lm_cell.value.as_str_coerced() {
            for part in s.trim_matches('"').split(';') {
                if let Ok(ix) = part.trim().parse::<u32>() {
                    layer_membership.push(ix);
                }
            }
        }
    }

    Ok(VisioShape {
        id,
        name,
        name_u,
        kind,
        master_id,
        master_shape_id,
        line_style_id,
        fill_style_id,
        text_style_id,
        text,
        sheet,
        children,
        layer_membership,
        foreign_data,
        opaque_xml_fragments,
        bound_data_row: None,
    })
}

fn parse_sheet_body(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
    sheet: &mut ShapeSheet,
    layers: &mut Vec<LayerDef>,
) -> Result<(), VsdxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Cell" => parse_cell_elem(reader, &e, false, sheet)?,
                b"Section" => parse_section_elem(reader, &e, sheet, layers)?,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                parse_cell_elem(reader, &e, true, sheet)?;
            }
            Event::End(e) if e.local_name().as_ref() == end_tag => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_cell_elem(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart<'_>,
    is_empty: bool,
    sheet: &mut ShapeSheet,
) -> Result<(), VsdxError> {
    let Some(name) = attr_str(e, b"N") else {
        if !is_empty {
            skip_to_end(reader, b"Cell")?;
        }
        return Ok(());
    };
    let val = attr_str(e, b"V");
    let unit_tok = attr_str(e, b"U");
    let formula_attr = attr_str(e, b"F");
    let unit = unit_tok.as_deref().and_then(Unit::from_token);

    let formula = match formula_attr.as_deref() {
        Some("Inh") | None => {
            if let Some(v) = &val {
                format_vsdx_value_as_formula(v, unit)
            } else {
                "0".into()
            }
        }
        Some("No Formula") => val
            .as_deref()
            .map(|v| format_vsdx_value_as_formula(v, unit))
            .unwrap_or_else(|| "0".into()),
        Some(f) => {
            if f.starts_with('=') {
                f.to_string()
            } else {
                format!("={f}")
            }
        }
    };
    sheet.set_formula(name.clone(), formula, unit);
    // Also populate initial cached value from `V` attribute in case formula is `Inh` or literal
    if let (Some(v_str), Some(cell)) = (val, sheet.get_cell_mut(&name)) {
        if let Some(c) = RgbaColor::from_hex(&v_str) {
            cell.value = vexio_shapesheet::CellValue::Color(c);
        } else if matches!(unit, Some(Unit::String)) {
            cell.value = vexio_shapesheet::CellValue::Str(v_str);
        } else if matches!(unit, Some(Unit::Boolean)) {
            cell.value = vexio_shapesheet::CellValue::Bool(v_str == "1" || v_str.eq_ignore_ascii_case("true"));
        } else if let Ok(v_num) = v_str.parse::<f64>() {
            let u = unit.or(cell.unit_hint).unwrap_or(Unit::Number);
            cell.value = vexio_shapesheet::CellValue::Num {
                internal: v_num,
                unit: u,
            };
        }
    }

    if !is_empty {
        skip_to_end(reader, b"Cell")?;
    }
    Ok(())
}

fn format_vsdx_value_as_formula(v: &str, unit: Option<Unit>) -> String {
    if v.starts_with('#') && RgbaColor::from_hex(v).is_some() {
        let c = RgbaColor::from_hex(v).unwrap();
        return format!("=RGB({},{},{})", c.r, c.g, c.b);
    }
    match unit {
        Some(Unit::Boolean) => {
            if v == "1" || v.eq_ignore_ascii_case("true") {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        Some(Unit::String) => format!("\"{}\"", v.replace('"', "\"\"")),
        Some(u) => {
            if let Ok(internal) = v.parse::<f64>() {
                let disp = u.from_internal(internal);
                format!("{}{}", vexio_shapesheet::format_clean_f64(disp), u.suffix())
            } else {
                format!("\"{}\"", v.replace('"', "\"\""))
            }
        }
        None => {
            if v.parse::<f64>().is_ok() {
                v.to_string()
            } else {
                format!("\"{}\"", v.replace('"', "\"\""))
            }
        }
    }
}

fn parse_section_elem(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    sheet: &mut ShapeSheet,
    layers: &mut Vec<LayerDef>,
) -> Result<(), VsdxError> {
    let sec_name = attr_str(start, b"N").unwrap_or_default();
    let sec_ix = attr_str(start, b"IX").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let deleted = attr_str(start, b"Del").map(|d| d == "1").unwrap_or(false);

    match sec_name.as_str() {
        "Geometry" => {
            let geom_idx = if sec_ix == 0 {
                (sheet.geometries.len() as u32) + 1
            } else {
                sec_ix
            };
            let mut geom = GeometrySection::new(geom_idx);
            geom.deleted = deleted;
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Empty(e) | Event::Start(e) if e.local_name().as_ref() == b"Cell" => {
                        let cname = attr_str(&e, b"N").unwrap_or_default();
                        let val = cell_formula_from_attrs(&e);
                        match cname.as_str() {
                            "NoFill" => geom.no_fill = val,
                            "NoLine" => geom.no_line = val,
                            "NoShow" => geom.no_show = val,
                            "NoSnap" => geom.no_snap = val,
                            "NoQuickDrag" => geom.no_quick_drag = val,
                            _ => {}
                        }
                    }
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        if let Some(row) = parse_geometry_row(reader, &e)? {
                            geom.rows.push(row);
                        }
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
            sheet.add_geometry(geom);
        }
        "Layer" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let ix = attr_str(&e, b"IX").and_then(|s| s.parse().ok()).unwrap_or(0);
                        let cells = read_row_cells(reader)?;
                        let name = cells
                            .get("Name")
                            .cloned()
                            .unwrap_or_else(|| format!("Layer {ix}"))
                            .trim_matches('"')
                            .to_string();
                        let mut ldef = LayerDef::new(ix, name.clone());
                        ldef.name_u = cells
                            .get("NameUniv")
                            .map(|s| s.trim_matches('"').to_string())
                            .unwrap_or(name);
                        ldef.visible = cells.get("Visible").map(|v| v != "0" && v != "FALSE").unwrap_or(true);
                        ldef.print = cells.get("Print").map(|v| v != "0" && v != "FALSE").unwrap_or(true);
                        ldef.active = cells.get("Active").map(|v| v == "1" || v == "TRUE").unwrap_or(false);
                        ldef.lock = cells.get("Lock").map(|v| v == "1" || v == "TRUE").unwrap_or(false);
                        ldef.snap = cells.get("Snap").map(|v| v != "0" && v != "FALSE").unwrap_or(true);
                        ldef.glue = cells.get("Glue").map(|v| v != "0" && v != "FALSE").unwrap_or(true);
                        if let Some(col_s) = cells.get("Color") {
                            ldef.color_override = RgbaColor::from_hex(col_s.trim_matches('"'));
                        }
                        layers.push(ldef);
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Connection" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let ix = attr_str(&e, b"IX").and_then(|s| s.parse().ok()).unwrap_or(1);
                        let rname = attr_str(&e, b"N").unwrap_or_default();
                        let cells = read_row_cells(reader)?;
                        sheet.add_connection_point(ConnectionPointRow {
                            name: rname,
                            ix,
                            x: cells.get("X").cloned().unwrap_or_else(|| "0 in".into()),
                            y: cells.get("Y").cloned().unwrap_or_else(|| "0 in".into()),
                            dir_x: cells.get("DirX").cloned().unwrap_or_else(|| "0 in".into()),
                            dir_y: cells.get("DirY").cloned().unwrap_or_else(|| "0 in".into()),
                            point_type: cells.get("Type").and_then(|t| t.parse().ok()).unwrap_or(0),
                        });
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Controls" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let ix = attr_str(&e, b"IX").and_then(|s| s.parse().ok()).unwrap_or(1);
                        let rname = attr_str(&e, b"N").unwrap_or_else(|| format!("Row_{ix}"));
                        let cells = read_row_cells(reader)?;
                        let x = cells.get("X").cloned().unwrap_or_else(|| "0 in".into());
                        let y = cells.get("Y").cloned().unwrap_or_else(|| "0 in".into());
                        sheet.add_control(ControlRow {
                            name: rname,
                            x: x.clone(),
                            y: y.clone(),
                            x_dyn: cells.get("XDyn").cloned().unwrap_or(x),
                            y_dyn: cells.get("YDyn").cloned().unwrap_or(y),
                            x_con: cells.get("XCon").cloned().unwrap_or_else(|| "0".into()),
                            y_con: cells.get("YCon").cloned().unwrap_or_else(|| "0".into()),
                            can_glue: cells.get("CanGlue").map(|v| v == "1" || v == "TRUE").unwrap_or(false),
                            prompt: cells
                                .get("Prompt")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                        });
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "User" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let rname = attr_str(&e, b"N").unwrap_or_else(|| "Row_1".into());
                        let cells = read_row_cells(reader)?;
                        let val = cells.get("Value").cloned().unwrap_or_else(|| "0".into());
                        let prompt = cells
                            .get("Prompt")
                            .cloned()
                            .unwrap_or_default()
                            .trim_matches('"')
                            .to_string();
                        sheet.set_user_cell(rname, val, prompt);
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Property" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let rname = attr_str(&e, b"N").unwrap_or_else(|| "Row_1".into());
                        let cells = read_row_cells(reader)?;
                        let dtype_code = cells
                            .get("Type")
                            .and_then(|s| s.parse::<u8>().ok())
                            .unwrap_or(0);
                        let data_type = match dtype_code {
                            1 => ShapeDataType::FixedList,
                            2 => ShapeDataType::Number,
                            3 => ShapeDataType::Boolean,
                            4 => ShapeDataType::VariableList,
                            5 => ShapeDataType::DateTime,
                            6 => ShapeDataType::Duration,
                            7 => ShapeDataType::Currency,
                            _ => ShapeDataType::String,
                        };
                        sheet.set_shape_data(ShapeDataRow {
                            name: rname.clone(),
                            label: cells
                                .get("Label")
                                .cloned()
                                .unwrap_or(rname)
                                .trim_matches('"')
                                .to_string(),
                            prompt: cells
                                .get("Prompt")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            data_type,
                            format: cells
                                .get("Format")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            value: cells.get("Value").cloned().unwrap_or_else(|| "\"\"".into()),
                            sort_key: cells
                                .get("SortKey")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            invisible: cells
                                .get("Invisible")
                                .map(|v| v == "1" || v == "TRUE")
                                .unwrap_or(false),
                            ask: cells.get("Ask").map(|v| v == "1" || v == "TRUE").unwrap_or(false),
                            lang_id: 1033,
                        });
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Action" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let rname = attr_str(&e, b"N").unwrap_or_else(|| "Row_1".into());
                        let cells = read_row_cells(reader)?;
                        sheet.actions.push(ActionRow {
                            name: rname,
                            action: cells.get("Action").cloned().unwrap_or_default(),
                            menu: cells
                                .get("Menu")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            tag_name: cells
                                .get("TagName")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            checked: cells.get("Checked").cloned().unwrap_or_else(|| "FALSE".into()),
                            disabled: cells.get("Disabled").cloned().unwrap_or_else(|| "FALSE".into()),
                            invisible: cells.get("Invisible").cloned().unwrap_or_else(|| "FALSE".into()),
                            begin_group: cells
                                .get("BeginGroup")
                                .map(|v| v == "1" || v == "TRUE")
                                .unwrap_or(false),
                        });
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Hyperlink" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let rname = attr_str(&e, b"N").unwrap_or_else(|| "Row_1".into());
                        let cells = read_row_cells(reader)?;
                        sheet.hyperlinks.push(HyperlinkRow {
                            name: rname,
                            description: cells
                                .get("Description")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            address: cells
                                .get("Address")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            sub_address: cells
                                .get("SubAddress")
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string(),
                            new_window: cells
                                .get("NewWindow")
                                .map(|v| v == "1" || v == "TRUE")
                                .unwrap_or(false),
                            default: cells
                                .get("Default")
                                .map(|v| v == "1" || v == "TRUE")
                                .unwrap_or(false),
                        });
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Field" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let ix = attr_str(&e, b"IX").and_then(|s| s.parse().ok()).unwrap_or(0);
                        let cells = read_row_cells(reader)?;
                        sheet.text_fields.push(TextFieldRow {
                            ix,
                            value: cells.get("Value").cloned().unwrap_or_default(),
                            format: cells.get("Format").cloned().unwrap_or_default(),
                        });
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Character" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let cells = read_row_cells(reader)?;
                        if let Some(sz) = cells.get("Size") {
                            sheet.set_formula("Char.Size", sz.clone(), Some(Unit::Point));
                        }
                        if let Some(col) = cells.get("Color") {
                            sheet.set_formula("Char.Color", col.clone(), Some(Unit::Color));
                        }
                        if let Some(st) = cells.get("Style") {
                            sheet.set_formula("Char.Style", st.clone(), Some(Unit::Number));
                        }
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        _ => {
            skip_to_end(reader, b"Section")?;
        }
    }
    Ok(())
}

fn parse_geometry_row(
    reader: &mut Reader<&[u8]>,
    row_start: &BytesStart<'_>,
) -> Result<Option<GeometryRow>, VsdxError> {
    let row_type = attr_str(row_start, b"T").unwrap_or_else(|| "LineTo".into());
    let ix = attr_str(row_start, b"IX").and_then(|s| s.parse().ok()).unwrap_or(1);
    let deleted = attr_str(row_start, b"Del").map(|d| d == "1").unwrap_or(false);
    let cells = read_row_cells(reader)?;
    let Some(kind) = GeometryRowKind::from_vsdx_tag(&row_type) else {
        return Ok(None);
    };
    Ok(Some(GeometryRow {
        ix,
        kind,
        x: cells.get("X").cloned().unwrap_or_else(|| "0 in".into()),
        y: cells.get("Y").cloned().unwrap_or_else(|| "0 in".into()),
        a: cells.get("A").cloned(),
        b: cells.get("B").cloned(),
        c: cells.get("C").cloned(),
        d: cells.get("D").cloned(),
        e: cells.get("E").cloned(),
        deleted,
    }))
}

fn read_row_cells(reader: &mut Reader<&[u8]>) -> Result<BTreeMap<String, String>, VsdxError> {
    let mut map = BTreeMap::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                if let Some(n) = attr_str(&e, b"N") {
                    map.insert(n, cell_formula_from_attrs(&e));
                }
            }
            Event::Start(e) if e.local_name().as_ref() == b"Cell" => {
                if let Some(n) = attr_str(&e, b"N") {
                    map.insert(n, cell_formula_from_attrs(&e));
                }
                skip_to_end(reader, b"Cell")?;
            }
            Event::End(e) if e.local_name().as_ref() == b"Row" => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(map)
}

fn cell_formula_from_attrs(e: &BytesStart<'_>) -> String {
    let val = attr_str(e, b"V");
    let unit = attr_str(e, b"U").as_deref().and_then(Unit::from_token);
    match attr_str(e, b"F").as_deref() {
        Some("Inh") | Some("No Formula") | None => val
            .as_deref()
            .map(|v| format_vsdx_value_as_formula(v, unit))
            .unwrap_or_else(|| "0".into()),
        Some(f) => {
            if f.starts_with('=') {
                f.to_string()
            } else {
                format!("={f}")
            }
        }
    }
}

fn read_text_element(reader: &mut Reader<&[u8]>) -> Result<String, VsdxError> {
    let mut out = String::new();
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(_) => depth += 1,
            Event::End(e) => {
                depth -= 1;
                if depth == 0 && e.local_name().as_ref() == b"Text" {
                    break;
                }
            }
            Event::Text(t) => {
                if let Ok(s) = t.unescape() {
                    out.push_str(&s);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(out)
}

fn read_foreign_data_body(reader: &mut Reader<&[u8]>) -> Result<(Option<String>, String), VsdxError> {
    let mut rel_id = None;
    let mut b64 = String::new();
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) if e.local_name().as_ref() == b"Rel" => {
                rel_id = attr_str(&e, b"id");
            }
            Event::Start(e) => {
                if e.local_name().as_ref() == b"Rel" {
                    rel_id = attr_str(&e, b"id");
                }
                depth += 1;
            }
            Event::Text(t) => {
                if let Ok(s) = t.unescape() {
                    b64.push_str(&s);
                }
            }
            Event::End(e) => {
                depth -= 1;
                if depth == 0 && e.local_name().as_ref() == b"ForeignData" {
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok((rel_id, b64))
}

fn capture_raw_element(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    tag_name: &str,
) -> Result<String, VsdxError> {
    let mut xml = String::new();
    xml.push('<');
    xml.push_str(&String::from_utf8_lossy(start.as_ref()));
    xml.push('>');
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                depth += 1;
                xml.push('<');
                xml.push_str(&String::from_utf8_lossy(e.as_ref()));
                xml.push('>');
            }
            Event::Empty(e) => {
                xml.push('<');
                xml.push_str(&String::from_utf8_lossy(e.as_ref()));
                xml.push_str("/>");
            }
            Event::Text(t) => {
                xml.push_str(&String::from_utf8_lossy(t.as_ref()));
            }
            Event::End(e) => {
                depth -= 1;
                xml.push_str("</");
                xml.push_str(&String::from_utf8_lossy(e.as_ref()));
                xml.push('>');
                if depth == 0 && e.local_name().as_ref() == tag_name.as_bytes() {
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(xml)
}

fn skip_to_end(reader: &mut Reader<&[u8]>, tag: &[u8]) -> Result<(), VsdxError> {
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(_) => depth += 1,
            Event::End(e) => {
                depth -= 1;
                if depth == 0 && e.local_name().as_ref() == tag {
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn parse_comments_xml(xml: &str, doc: &mut VisioDocument) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut authors: BTreeMap<u32, (String, String)> = BTreeMap::new();
    let mut cur_comment: Option<CommentAnnotation> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e) | Event::Start(e)) if e.local_name().as_ref() == b"AuthorEntry" => {
                let id = attr_str(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(0);
                let name = attr_str(&e, b"Name").unwrap_or_else(|| "Reviewer".into());
                let initials = attr_str(&e, b"Initials").unwrap_or_else(|| "RV".into());
                authors.insert(id, (name, initials));
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"CommentEntry" => {
                let author_id = attr_str(&e, b"AuthorID").and_then(|s| s.parse().ok()).unwrap_or(0);
                let (author, initials) = authors
                    .get(&author_id)
                    .cloned()
                    .unwrap_or_else(|| ("Reviewer".into(), "RV".into()));
                cur_comment = Some(CommentAnnotation {
                    id: attr_str(&e, b"CommentID")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or((doc.comments.len() as u32) + 1),
                    author,
                    initials,
                    timestamp_iso: attr_str(&e, b"Date").unwrap_or_default(),
                    page_id: attr_str(&e, b"PageID").and_then(|s| s.parse().ok()).unwrap_or(1),
                    shape_id: attr_str(&e, b"ShapeID").and_then(|s| s.parse().ok()),
                    x_in: 1.0,
                    y_in: 1.0,
                    text: String::new(),
                    resolved: attr_str(&e, b"Done").map(|v| v == "1" || v == "true").unwrap_or(false),
                });
            }
            Ok(Event::Text(t)) if cur_comment.is_some() => {
                if let (Some(c), Ok(s)) = (cur_comment.as_mut(), t.unescape()) {
                    c.text.push_str(&s);
                }
            }
            Ok(Event::End(e)) if e.local_name().as_ref() == b"CommentEntry" => {
                if let Some(c) = cur_comment.take() {
                    doc.comments.push(c);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
}

fn resolve_visio_subpath(base_dir: &str, rel_target: &str) -> String {
    if let Some(stripped) = rel_target.strip_prefix('/') {
        return stripped.to_string();
    }
    let mut stack: Vec<&str> = base_dir.split('/').collect();
    for seg in rel_target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    stack.join("/")
}
