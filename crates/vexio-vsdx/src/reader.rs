//! Streaming OpenXML Visio reader (`.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, `.vstm`).

use crate::opc::{RawOpcArchive, VsdxError, VsdxPackageKind, is_managed_opc_part};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::collections::BTreeMap;
use vexio_shapesheet::{
    ActionRow, CommentAnnotation, ConnectionPointRow, ControlRow, ForeignMediaData, GeometryRow,
    GeometryRowKind, GeometrySection, GlueEndpoint, GradientStopRow, HyperlinkRow, LayerDef,
    ReactiveEngine, RgbaColor, ShapeDataRow, ShapeDataType, ShapeKind, ShapeSheet, StencilPack,
    StyleSheetDef, TextFieldRow, Unit, VisioDocument, VisioMaster, VisioPage, VisioShape,
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

    // 4. Parse `visio/theme/theme*.xml` if present, then `visio/document.xml` (`Colors`, `FaceNames`, `Styles`, `DocumentSheet`, `HeaderFooter`)
    parse_theme_parts(&archive, &mut doc);
    let mut doc_tables = DocParseTables::with_visio_defaults();
    if let Some(doc_xml) = archive.get_str("visio/document.xml")? {
        parse_document_xml(&doc_xml, &mut doc, &mut doc_tables)?;
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

    // 5. Parse Masters (`visio/masters/masters.xml` + `visio/masters/masterN.xml` + per-master `.rels`)
    let master_rels = archive
        .get_str("visio/masters/_rels/masters.xml.rels")?
        .map(|s| parse_rels(&s))
        .unwrap_or_default();
    if let Some(masters_xml) = archive.get_str("visio/masters/masters.xml")? {
        parse_masters_index(&masters_xml, &master_rels, &archive, &doc_tables, &mut doc)?;
    }

    // 6. Parse Pages (`visio/pages/pages.xml` + `visio/pages/pageN.xml` + per-page `.rels`)
    let page_rels = archive
        .get_str("visio/pages/_rels/pages.xml.rels")?
        .map(|s| parse_rels(&s))
        .unwrap_or_default();
    if let Some(pages_xml) = archive.get_str("visio/pages/pages.xml")? {
        parse_pages_index(&pages_xml, &page_rels, &archive, &doc_tables, &mut doc)?;
    }
    if doc.pages.is_empty() && !matches!(pkg_kind, VsdxPackageKind::Stencil | VsdxPackageKind::MacroStencil) {
        doc.pages.push(VisioPage::new(1, "Page-1", 11.0, 8.5));
    }

    // 7. Parse Comments (`visio/comments.xml`)
    if let Some(comments_xml) = archive.get_str("visio/comments.xml")? {
        parse_comments_xml(&comments_xml, &mut doc);
    }

    // 8. Hydrate Master group children, sparse Geometry overrides, and internal Master Sheet.N! references
    hydrate_master_instances(&mut doc);

    // 8b. Restore External Data Recordsets, Page Data Graphics & Shape Row Bindings if present
    if let Ok(Some(data_json)) = archive.get_str("visio/data/vexio_data_state.json") {
        if let Ok(state) = serde_json::from_str::<crate::writer::VexioVsdxDataState>(&data_json) {
            doc.data_recordsets = state.recordsets;
            for (pid, cfg) in state.page_data_graphics {
                if let Some(p) = doc.page_mut(pid) {
                    p.data_graphic_config = Some(cfg);
                }
            }
            for (key, binding) in state.shape_bindings {
                if let Some((pid_s, sid_s)) = key.split_once(':') {
                    if let (Ok(pid), Ok(sid)) = (pid_s.parse::<u32>(), sid_s.parse::<u64>()) {
                        if let Some(sh) = doc.page_mut(pid).and_then(|p| p.find_shape_mut(sid)) {
                            sh.bound_data_row = Some(binding);
                        }
                    }
                }
            }
        }
    }

    // 9. Recalculate reactive ShapeSheet DAG across all sheets
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

#[derive(Debug, Clone, Default)]
struct DocParseTables {
    colors: BTreeMap<u32, RgbaColor>,
    face_names: BTreeMap<u32, String>,
}

impl DocParseTables {
    fn with_visio_defaults() -> Self {
        let mut colors = BTreeMap::new();
        for ix in 0..24 {
            if let Some(c) = vexio_shapesheet::visio_standard_palette_color(ix) {
                colors.insert(ix, c);
            }
        }
        let mut face_names = BTreeMap::new();
        face_names.insert(0, "Segoe UI".into());
        face_names.insert(1, "Calibri".into());
        face_names.insert(2, "Arial".into());
        Self { colors, face_names }
    }

    fn resolve_color_token(&self, v: &str) -> Option<RgbaColor> {
        let t = v.trim().trim_matches('"');
        if let Some(c) = RgbaColor::from_hex(t) {
            return Some(c);
        }
        if let Ok(ix) = t.parse::<u32>() {
            return self.colors.get(&ix).copied();
        }
        None
    }
}

fn parse_theme_parts(archive: &RawOpcArchive, doc: &mut VisioDocument) {
    let mut parsed_themes = Vec::new();
    for (path, bytes) in &archive.files {
        let lower = path.to_ascii_lowercase();
        if lower.starts_with("visio/theme/theme") && lower.ends_with(".xml") {
            let Ok(xml) = std::str::from_utf8(bytes) else {
                continue;
            };
            let mut theme = vexio_shapesheet::ThemeDef::default();
            theme.id = (parsed_themes.len() as u32) + 1;
            let mut reader = Reader::from_str(xml);
            reader.config_mut().trim_text(true);
            let mut buf = Vec::new();
            let mut slot = String::new();
            let mut font_role = String::new();
            loop {
                match reader.read_event_into(&mut buf) {
                    Ok(Event::Start(e)) => {
                        let ln = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                        match ln.as_str() {
                            "theme" => {
                                if let Some(n) = attr_str(&e, b"name") {
                                    theme.name = n;
                                }
                            }
                            "dk1" | "lt1" | "dk2" | "lt2" | "accent1" | "accent2" | "accent3"
                            | "accent4" | "accent5" | "accent6" => {
                                slot = ln;
                            }
                            "majorfont" | "minorfont" => {
                                font_role = ln;
                            }
                            "latin" if !font_role.is_empty() => {
                                if let Some(tf) = attr_str(&e, b"typeface").filter(|s| !s.is_empty()) {
                                    if font_role == "majorfont" {
                                        theme.heading_font = tf;
                                    } else {
                                        theme.body_font = tf;
                                    }
                                }
                            }
                            "srgbclr" | "sysclr" if !slot.is_empty() => {
                                let hex = attr_str(&e, b"val").or_else(|| attr_str(&e, b"lastClr"));
                                if let Some(c) = hex.and_then(|h| RgbaColor::from_hex(&format!("#{h}"))) {
                                    assign_theme_slot(&mut theme, &slot, c);
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(Event::Empty(e)) => {
                        let ln = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                        if (ln == "srgbclr" || ln == "sysclr") && !slot.is_empty() {
                            let hex = attr_str(&e, b"val").or_else(|| attr_str(&e, b"lastClr"));
                            if let Some(c) = hex.and_then(|h| RgbaColor::from_hex(&format!("#{h}"))) {
                                assign_theme_slot(&mut theme, &slot, c);
                            }
                        } else if ln == "latin" && !font_role.is_empty() {
                            if let Some(tf) = attr_str(&e, b"typeface").filter(|s| !s.is_empty()) {
                                if font_role == "majorfont" {
                                    theme.heading_font = tf;
                                } else {
                                    theme.body_font = tf;
                                }
                            }
                        }
                    }
                    Ok(Event::End(e)) => {
                        let ln = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                        if ln == slot {
                            slot.clear();
                        } else if ln == font_role {
                            font_role.clear();
                        }
                    }
                    Ok(Event::Eof) | Err(_) => break,
                    _ => {}
                }
                buf.clear();
            }
            parsed_themes.push(theme);
        }
    }
    if !parsed_themes.is_empty() {
        doc.themes = parsed_themes;
        doc.active_theme_id = Some(1);
    }
}

fn assign_theme_slot(theme: &mut vexio_shapesheet::ThemeDef, slot: &str, color: RgbaColor) {
    match slot {
        "dk1" => theme.dark_color = color,
        "lt1" => {
            theme.light_color = color;
            theme.background_color = color;
        }
        "accent1" => theme.accent_colors[0] = color,
        "accent2" => theme.accent_colors[1] = color,
        "accent3" => theme.accent_colors[2] = color,
        "accent4" => theme.accent_colors[3] = color,
        "accent5" => theme.accent_colors[4] = color,
        "accent6" => theme.accent_colors[5] = color,
        _ => {}
    }
}

fn parse_document_xml(
    xml: &str,
    doc: &mut VisioDocument,
    tables: &mut DocParseTables,
) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut in_header_footer = false;
    let mut hf_tag = String::new();

    // First scan `<ColorEntry>` and `<FaceName>` so StyleSheets & DocumentSheet resolve them cleanly.
    {
        let mut pre_reader = Reader::from_str(xml);
        pre_reader.config_mut().trim_text(true);
        let mut pbuf = Vec::new();
        loop {
            match pre_reader.read_event_into(&mut pbuf) {
                Ok(Event::Empty(e) | Event::Start(e)) => match e.local_name().as_ref() {
                    b"ColorEntry" => {
                        if let (Some(ix), Some(rgb)) = (
                            attr_str(&e, b"IX").and_then(|s| s.parse::<u32>().ok()),
                            attr_str(&e, b"RGB").and_then(|s| RgbaColor::from_hex(&s)),
                        ) {
                            tables.colors.insert(ix, rgb);
                        }
                    }
                    b"FaceName" => {
                        if let (Some(id), Some(name)) = (
                            attr_str(&e, b"ID").and_then(|s| s.parse::<u32>().ok()),
                            attr_str(&e, b"Name").or_else(|| attr_str(&e, b"NameU")),
                        ) {
                            tables.face_names.insert(id, name);
                        }
                    }
                    _ => {}
                },
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
            pbuf.clear();
        }
    }

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
                    parse_sheet_body(&mut reader, b"StyleSheet", &mut sheet, &mut layers, tables)?;
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
                    parse_sheet_body(&mut reader, b"DocumentSheet", &mut doc.doc_sheet, &mut layers, tables)?;
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

fn rels_path_for_part(part_path: &str) -> (String, String) {
    if let Some((dir, fname)) = part_path.rsplit_once('/') {
        (dir.to_string(), format!("{dir}/_rels/{fname}.rels"))
    } else {
        (String::new(), format!("_rels/{part_path}.rels"))
    }
}

fn parse_masters_index(
    xml: &str,
    rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    tables: &DocParseTables,
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
                    let (part_dir, rpath) = rels_path_for_part(&part_path);
                    let part_rels = archive
                        .get_str(&rpath)?
                        .map(|s| parse_rels(&s))
                        .unwrap_or_default();
                    parse_master_part(&m_xml, &part_dir, &part_rels, archive, tables, &mut m)?;
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
                    parse_sheet_body(&mut reader, b"PageSheet", &mut m.page_sheet, &mut dummy_layers, tables)?;
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
                        let (part_dir, rpath) = rels_path_for_part(&full_path);
                        let part_rels = archive
                            .get_str(&rpath)?
                            .map(|s| parse_rels(&s))
                            .unwrap_or_default();
                        parse_master_part(&m_xml, &part_dir, &part_rels, archive, tables, &mut m)?;
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

fn parse_master_part(
    xml: &str,
    part_dir: &str,
    part_rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    tables: &DocParseTables,
    master: &mut VisioMaster,
) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) if e.local_name().as_ref() == b"Shape" => {
                let mut shape = parse_shape(&mut reader, &e, part_dir, part_rels, archive, tables)?;
                finalize_geometry_defaults_recursive(&mut shape);
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
    tables: &DocParseTables,
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
                    parse_sheet_body(&mut reader, b"PageSheet", &mut p.page_sheet, &mut p.layers, tables)?;
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
                        let (part_dir, rpath) = rels_path_for_part(&full_path);
                        let part_rels = archive
                            .get_str(&rpath)?
                            .map(|s| parse_rels(&s))
                            .unwrap_or_default();
                        parse_page_part(&p_xml, &part_dir, &part_rels, archive, tables, &mut page)?;
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

fn parse_page_part(
    xml: &str,
    part_dir: &str,
    part_rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    tables: &DocParseTables,
    page: &mut VisioPage,
) -> Result<(), VsdxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"PageSheet" => {
                    parse_sheet_body(&mut reader, b"PageSheet", &mut page.page_sheet, &mut page.layers, tables)?;
                }
                b"Shape" => {
                    let shape = parse_shape(&mut reader, &e, part_dir, part_rels, archive, tables)?;
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
    let to_part = attr_str(e, b"ToPart").and_then(|s| s.parse::<u32>().ok());
    let mut to_cell = attr_str(e, b"ToCell").unwrap_or_else(|| "PinX".into());
    if (to_cell.eq_ignore_ascii_case("PinX") || to_cell.is_empty()) && to_part.is_some_and(|p| p >= 100) {
        let port_ix = to_part.unwrap() - 99;
        to_cell = format!("Connections.X{port_ix}");
    }
    let is_dynamic = to_cell.eq_ignore_ascii_case("PinX") || to_cell.eq_ignore_ascii_case("PinY");
    GlueEndpoint {
        connector_shape_id: from_sheet,
        from_cell,
        target_shape_id: to_sheet,
        to_cell,
        is_dynamic,
    }
}

fn parse_shape(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    part_dir: &str,
    part_rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    tables: &DocParseTables,
) -> Result<VisioShape, VsdxError> {
    parse_shape_bounded(reader, start, part_dir, part_rels, archive, tables, 0)
}

fn parse_shape_bounded(
    reader: &mut Reader<&[u8]>,
    start: &BytesStart<'_>,
    part_dir: &str,
    part_rels: &BTreeMap<String, String>,
    archive: &RawOpcArchive,
    tables: &DocParseTables,
    depth: u8,
) -> Result<VisioShape, VsdxError> {
    let id = attr_str(start, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
    let name = attr_str(start, b"Name").unwrap_or_else(|| format!("Sheet.{id}"));
    let name_u = attr_str(start, b"NameU").unwrap_or_else(|| name.clone());
    if depth > 32 {
        skip_to_end(reader, b"Shape")?;
        return Ok(VisioShape::new_rect(id, name, 0.0, 0.0, 1.0, 1.0));
    }
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
    let mut has_explicit_text = false;
    let mut foreign_data = None;
    let mut opaque_xml_fragments = Vec::new();
    let mut dummy_layers = Vec::new();

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Cell" => {
                    parse_cell_elem(reader, &e, false, &mut sheet, tables)?;
                }
                b"Section" => {
                    parse_section_elem(reader, &e, &mut sheet, &mut dummy_layers, tables)?;
                }
                b"Text" => {
                    has_explicit_text = true;
                    text = read_text_element(reader)?;
                }
                b"Shapes" => {
                    // Container for group child `<Shape>` elements
                }
                b"Shape" => {
                    if attr_str(&e, b"Del").as_deref() == Some("1") {
                        if let Some(msid) = attr_str(&e, b"MasterShape").and_then(|s| s.parse().ok()) {
                            let mut tomb = VisioShape::new_rect(u64::MAX, "__del__", 0.0, 0.0, 0.0, 0.0);
                            tomb.master_shape_id = Some(msid);
                            children.push(tomb);
                        }
                        skip_to_end(reader, b"Shape")?;
                    } else {
                        children.push(parse_shape_bounded(
                            reader,
                            &e,
                            part_dir,
                            part_rels,
                            archive,
                            tables,
                            depth + 1,
                        )?);
                    }
                }
                b"ForeignData" => {
                    let ftype = attr_str(&e, b"ForeignType").unwrap_or_else(|| "Bitmap".into());
                    let mut ctype = attr_str(&e, b"CompressionType").unwrap_or_else(|| ftype.clone());
                    let (rel_id, raw_b64) = read_foreign_data_body(reader)?;
                    let mut decoded = if !raw_b64.trim().is_empty() {
                        use base64::Engine;
                        base64::engine::general_purpose::STANDARD
                            .decode(raw_b64.trim())
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    if decoded.is_empty() {
                        if let Some(rid) = &rel_id {
                            if let Some(rel_target) = part_rels.get(rid) {
                                let media_path = resolve_visio_subpath(part_dir, rel_target);
                                let resolved_bytes = archive.files.get(&media_path).cloned().or_else(|| {
                                    archive
                                        .files
                                        .iter()
                                        .find(|(k, _)| k.eq_ignore_ascii_case(&media_path))
                                        .map(|(_, v)| v.clone())
                                });
                                if let Some(bytes) = resolved_bytes {
                                    ctype = infer_foreign_media_type(&media_path, &ctype, &bytes);
                                    decoded = bytes;
                                }
                            }
                        }
                    } else {
                        ctype = infer_foreign_media_type("", &ctype, &decoded);
                    }
                    if kind == ShapeKind::Shape2D && !decoded.is_empty() && sheet.geometries.is_empty() {
                        kind = ShapeKind::ForeignObject;
                    }
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
                    parse_cell_elem(reader, &e, true, &mut sheet, tables)?;
                }
                b"Text" => {
                    has_explicit_text = true;
                }
                b"Shape" => {
                    if attr_str(&e, b"Del").as_deref() == Some("1") {
                        if let Some(msid) = attr_str(&e, b"MasterShape").and_then(|s| s.parse().ok()) {
                            let mut tomb = VisioShape::new_rect(u64::MAX, "__del__", 0.0, 0.0, 0.0, 0.0);
                            tomb.master_shape_id = Some(msid);
                            children.push(tomb);
                        }
                    }
                }
                _ => {}
            },
            Event::End(e) if e.local_name().as_ref() == b"Shape" => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if has_explicit_text
        && text.trim().is_empty()
        && (master_id.is_some() || master_shape_id.is_some())
    {
        opaque_xml_fragments.push("<Text/>".to_string());
    }

    if kind == ShapeKind::Shape2D && !children.is_empty() {
        kind = ShapeKind::Group;
    } else if kind == ShapeKind::Shape2D
        && sheet.get_cell("BeginX").is_some()
        && sheet.get_cell("EndX").is_some()
    {
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

fn infer_foreign_media_type(path: &str, fallback: &str, bytes: &[u8]) -> String {
    let lp = path.to_ascii_lowercase();
    if lp.ends_with(".svg") || bytes.starts_with(b"<svg") || (bytes.starts_with(b"<?xml") && bytes.windows(4).any(|w| w == b"<svg")) {
        "SVG".into()
    } else if lp.ends_with(".png") || bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "PNG".into()
    } else if lp.ends_with(".jpg") || lp.ends_with(".jpeg") || bytes.starts_with(b"\xFF\xD8\xFF") {
        "JPEG".into()
    } else if lp.ends_with(".bmp") || bytes.starts_with(b"BM") {
        "BMP".into()
    } else if lp.ends_with(".emf") || (bytes.len() >= 44 && &bytes[40..44] == b" EMF") {
        "EMF".into()
    } else if lp.ends_with(".wmf") || bytes.starts_with(&[0xD7, 0xCD, 0xC6, 0x9A]) {
        "WMF".into()
    } else {
        fallback.to_string()
    }
}

fn parse_sheet_body(
    reader: &mut Reader<&[u8]>,
    end_tag: &[u8],
    sheet: &mut ShapeSheet,
    layers: &mut Vec<LayerDef>,
    tables: &DocParseTables,
) -> Result<(), VsdxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Cell" => parse_cell_elem(reader, &e, false, sheet, tables)?,
                b"Section" => parse_section_elem(reader, &e, sheet, layers, tables)?,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                parse_cell_elem(reader, &e, true, sheet, tables)?;
            }
            Event::End(e) if e.local_name().as_ref() == end_tag => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn is_color_cell_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "fillforegnd" | "fillbkgnd" | "linecolor" | "char.color" | "shdwforegnd" | "pagecolor"
    ) || lower.ends_with(".fillforegnd")
        || lower.ends_with(".linecolor")
}

/// Resolve Visio's 1-based `TextBkgnd` value token:
/// - `"0"` (or `<= 0`) -> `Ok(None)` (transparent / no text block background)
/// - `"#RRGGBB"` -> `Ok(Some(RgbaColor))`
/// - `"1".."254"` -> `Ok(Some(Colors[k - 1]))`
fn resolve_text_bkgnd_token(v: &str, tables: &DocParseTables) -> Option<Option<RgbaColor>> {
    let t = v.trim().trim_matches('"').trim();
    if t.is_empty() {
        return None;
    }
    if let Some(c) = RgbaColor::from_hex(t) {
        return Some(Some(c));
    }
    if let Ok(num) = t.parse::<f64>() {
        if !num.is_finite() {
            return None;
        }
        let k = num.round() as i64;
        if k <= 0 {
            return Some(None);
        }
        let idx0 = (k - 1) as u32;
        return tables
            .colors
            .get(&idx0)
            .copied()
            .or_else(|| vexio_shapesheet::visio_standard_palette_color(idx0))
            .map(Some);
    }
    None
}

fn parse_cell_elem(
    reader: &mut Reader<&[u8]>,
    e: &BytesStart<'_>,
    is_empty: bool,
    sheet: &mut ShapeSheet,
    tables: &DocParseTables,
) -> Result<(), VsdxError> {
    let Some(name) = attr_str(e, b"N") else {
        if !is_empty {
            skip_to_end(reader, b"Cell")?;
        }
        return Ok(());
    };
    let deleted = attr_str(e, b"Del").map(|d| d == "1").unwrap_or(false);
    let val = attr_str(e, b"V");
    let unit_tok = attr_str(e, b"U");
    let formula_attr = attr_str(e, b"F");
    let is_text_bkgnd = name.eq_ignore_ascii_case("TextBkgnd");
    let is_color = is_color_cell_name(&name);
    let unit = unit_tok
        .as_deref()
        .and_then(Unit::from_token)
        .or_else(|| if is_color { Some(Unit::Color) } else { None });

    // Crucial `[MS-VSDX]` rule: `<Cell N="..." F="Inh"/>` without `V` means inherit from Master/Style!
    if !deleted && val.is_none() && matches!(formula_attr.as_deref(), Some("Inh") | None) {
        if !is_empty {
            skip_to_end(reader, b"Cell")?;
        }
        return Ok(());
    }

    let formula = match formula_attr.as_deref() {
        Some("Inh") | Some("No Formula") | None => {
            if let Some(v) = &val {
                if is_text_bkgnd {
                    match resolve_text_bkgnd_token(v, tables) {
                        Some(None) => "0".into(),
                        Some(Some(c)) => format!("=RGB({},{},{})", c.r, c.g, c.b),
                        None => format_vsdx_value_as_formula(v, unit),
                    }
                } else if is_color {
                    if let Some(c) = tables.resolve_color_token(v) {
                        format!("=RGB({},{},{})", c.r, c.g, c.b)
                    } else {
                        format_vsdx_value_as_formula(v, unit)
                    }
                } else {
                    format_vsdx_value_as_formula(v, unit)
                }
            } else {
                "0".into()
            }
        }
        Some(f) => {
            if f.starts_with('=') {
                f.to_string()
            } else {
                format!("={f}")
            }
        }
    };
    sheet.set_formula(name.clone(), formula, unit);
    if let Some(cell) = sheet.get_cell_mut(&name) {
        cell.deleted = deleted;
        if let Some(v_str) = val {
            let parsed_cv = if is_text_bkgnd {
                match resolve_text_bkgnd_token(&v_str, tables) {
                    Some(None) => Some(vexio_shapesheet::CellValue::number(0.0)),
                    Some(Some(c)) => Some(vexio_shapesheet::CellValue::Color(c)),
                    None => None,
                }
            } else if is_color {
                tables
                    .resolve_color_token(&v_str)
                    .map(vexio_shapesheet::CellValue::Color)
            } else if let Some(c) = RgbaColor::from_hex(&v_str) {
                Some(vexio_shapesheet::CellValue::Color(c))
            } else if matches!(unit, Some(Unit::String)) {
                Some(vexio_shapesheet::CellValue::Str(v_str))
            } else if matches!(unit, Some(Unit::Boolean)) {
                Some(vexio_shapesheet::CellValue::Bool(
                    v_str == "1" || v_str.eq_ignore_ascii_case("true"),
                ))
            } else if let Ok(v_num) = v_str.parse::<f64>() {
                let u = unit.or(cell.unit_hint).unwrap_or(Unit::Number);
                Some(vexio_shapesheet::CellValue::Num {
                    internal: v_num,
                    unit: u,
                })
            } else {
                None
            };
            if let Some(cv) = parsed_cv {
                cell.value = cv.clone();
                cell.vsdx_cached_value = Some(cv);
            }
        }
    }

    if !is_empty {
        skip_to_end(reader, b"Cell")?;
    }
    Ok(())
}

fn format_vsdx_value_as_formula(v: &str, unit: Option<Unit>) -> String {
    let trimmed = v.trim();
    let upper = trimmed.to_ascii_uppercase();
    if upper.starts_with("NURBS(") || upper.starts_with("POLYLINE(") {
        return format!("={trimmed}");
    }
    if trimmed.starts_with('#') && RgbaColor::from_hex(trimmed).is_some() {
        let c = RgbaColor::from_hex(trimmed).unwrap();
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
    tables: &DocParseTables,
) -> Result<(), VsdxError> {
    let sec_name = attr_str(start, b"N").unwrap_or_default();
    let sec_ix_opt = attr_str(start, b"IX").and_then(|s| s.parse::<u32>().ok());
    let deleted = attr_str(start, b"Del").map(|d| d == "1").unwrap_or(false);

    match sec_name.as_str() {
        "Geometry" => {
            // In OpenXML `.vsdx`, `<Section N="Geometry" IX="0">` is 0-based (`IX=0` -> `Geometry1`, `IX=1` -> `Geometry2`).
            let geom_idx = match sec_ix_opt {
                Some(ix0) => ix0 + 1,
                None => (sheet.geometries.len() as u32) + 1,
            };
            let mut geom = GeometrySection::new(geom_idx);
            geom.no_fill = "Inh".into();
            geom.no_line = "Inh".into();
            geom.no_show = "Inh".into();
            geom.no_snap = "Inh".into();
            geom.no_quick_drag = "Inh".into();
            geom.deleted = deleted;
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                        let cname = attr_str(&e, b"N").unwrap_or_default();
                        if let Some(val) = cell_formula_from_attrs_opt(&e, Some(Unit::Boolean), tables) {
                            match cname.as_str() {
                                "NoFill" => {
                                    geom.no_fill = val;
                                    geom.explicit_flags |= 1;
                                }
                                "NoLine" => {
                                    geom.no_line = val;
                                    geom.explicit_flags |= 2;
                                }
                                "NoShow" => {
                                    geom.no_show = val;
                                    geom.explicit_flags |= 4;
                                }
                                "NoSnap" => geom.no_snap = val,
                                "NoQuickDrag" => geom.no_quick_drag = val,
                                _ => {}
                            }
                        }
                    }
                    Event::Start(e) if e.local_name().as_ref() == b"Cell" => {
                        let cname = attr_str(&e, b"N").unwrap_or_default();
                        if let Some(val) = cell_formula_from_attrs_opt(&e, Some(Unit::Boolean), tables) {
                            match cname.as_str() {
                                "NoFill" => {
                                    geom.no_fill = val;
                                    geom.explicit_flags |= 1;
                                }
                                "NoLine" => {
                                    geom.no_line = val;
                                    geom.explicit_flags |= 2;
                                }
                                "NoShow" => {
                                    geom.no_show = val;
                                    geom.explicit_flags |= 4;
                                }
                                "NoSnap" => geom.no_snap = val,
                                "NoQuickDrag" => geom.no_quick_drag = val,
                                _ => {}
                            }
                        }
                        skip_to_end(reader, b"Cell")?;
                    }
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        if let Some(row) = parse_geometry_row(reader, &e)? {
                            geom.rows.push(row);
                        }
                    }
                    Event::Empty(e) if e.local_name().as_ref() == b"Row" => {
                        let row_type = attr_str(&e, b"T").unwrap_or_else(|| "LineTo".into());
                        let ix = attr_str(&e, b"IX").and_then(|s| s.parse().ok()).unwrap_or(1);
                        let rdel = attr_str(&e, b"Del").map(|d| d == "1").unwrap_or(false);
                        if let Some(kind) = GeometryRowKind::from_vsdx_tag(&row_type) {
                            geom.rows.push(GeometryRow {
                                ix,
                                kind,
                                x: "Inh".into(),
                                y: "Inh".into(),
                                a: None,
                                b: None,
                                c: None,
                                d: None,
                                e: None,
                                deleted: rdel,
                            });
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
                        let next_n = sheet.actions.len() + 1;
                        let rname = attr_str(&e, b"N")
                            .or_else(|| attr_str(&e, b"IX").map(|ix| format!("Row_{ix}")))
                            .unwrap_or_else(|| format!("Row_{next_n}"));
                        let cells = read_row_cells(reader)?;
                        let menu_raw = cells.get("Menu").cloned().unwrap_or_default();
                        let menu_val = if menu_raw.trim().starts_with('=') {
                            menu_raw
                        } else {
                            menu_raw.trim_matches('"').to_string()
                        };
                        sheet.add_action(ActionRow {
                            name: rname,
                            action: cells.get("Action").cloned().unwrap_or_default(),
                            menu: menu_val,
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
                        parse_character_row_into_sheet(reader, sheet, tables)?;
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "Paragraph" => {
            let mut buf = Vec::new();
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let cells = read_row_cells_opt(reader, tables)?;
                        if let Some(ha) = cells.get("HorzAlign") {
                            sheet.set_formula("Para.HorzAlign", ha.clone(), Some(Unit::Number));
                        }
                        if let Some(bul) = cells.get("Bullet") {
                            sheet.set_formula("Para.Bullet", bul.clone(), Some(Unit::Number));
                        }
                    }
                    Event::End(e) if e.local_name().as_ref() == b"Section" => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }
        "FillGradient" => {
            let mut buf = Vec::new();
            let mut seq = 1u32;
            loop {
                match reader.read_event_into(&mut buf)? {
                    Event::Start(e) if e.local_name().as_ref() == b"Row" => {
                        let raw_ix = attr_str(&e, b"IX").and_then(|s| s.parse::<u32>().ok());
                        let ix = raw_ix.map(|n| n + 1).unwrap_or(seq);
                        seq = seq.max(ix + 1);
                        let cells = read_row_cells_opt(reader, tables)?;
                        let pos = cells
                            .get("GradientStopPosition")
                            .or_else(|| cells.get("Position"))
                            .cloned()
                            .unwrap_or_else(|| "0".into());
                        let col = cells
                            .get("GradientStopColor")
                            .or_else(|| cells.get("Color"))
                            .cloned()
                            .unwrap_or_else(|| "=RGB(255,255,255)".into());
                        let tr = cells
                            .get("GradientStopColorTrans")
                            .or_else(|| cells.get("ColorTrans"))
                            .cloned()
                            .unwrap_or_else(|| "0".into());
                        sheet.add_fill_gradient_stop(GradientStopRow {
                            ix,
                            position: pos,
                            color: col,
                            color_trans: tr,
                        });
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

fn parse_character_row_into_sheet(
    reader: &mut Reader<&[u8]>,
    sheet: &mut ShapeSheet,
    tables: &DocParseTables,
) -> Result<(), VsdxError> {
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                apply_character_cell_attrs(&e, sheet, tables);
            }
            Event::Start(e) if e.local_name().as_ref() == b"Cell" => {
                apply_character_cell_attrs(&e, sheet, tables);
                skip_to_end(reader, b"Cell")?;
            }
            Event::End(e) if e.local_name().as_ref() == b"Row" => break,
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    Ok(())
}

fn apply_character_cell_attrs(e: &BytesStart<'_>, sheet: &mut ShapeSheet, tables: &DocParseTables) {
    let Some(cname) = attr_str(e, b"N") else {
        return;
    };
    let val = attr_str(e, b"V");
    let f_attr = attr_str(e, b"F");
    if val.is_none() && matches!(f_attr.as_deref(), Some("Inh") | None) {
        return;
    }
    match cname.as_str() {
        "Size" => {
            if let Some(formula) = cell_formula_from_attrs_opt(e, Some(Unit::Point), tables) {
                sheet.set_formula("Char.Size", formula, Some(Unit::Point));
                if let (Some(vs), Some(cell)) = (&val, sheet.get_cell_mut("Char.Size")) {
                    if let Ok(v_in) = vs.parse::<f64>() {
                        let cv = vexio_shapesheet::CellValue::Num {
                            internal: v_in,
                            unit: Unit::Point,
                        };
                        cell.value = cv.clone();
                        cell.vsdx_cached_value = Some(cv);
                    }
                }
            }
        }
        "Color" => {
            let resolved_col = val.as_deref().and_then(|v| tables.resolve_color_token(v));
            let formula = match f_attr.as_deref() {
                Some("Inh") | Some("No Formula") | None => {
                    if let Some(c) = resolved_col {
                        format!("=RGB({},{},{})", c.r, c.g, c.b)
                    } else {
                        return;
                    }
                }
                Some(f) => {
                    if f.starts_with('=') {
                        f.to_string()
                    } else {
                        format!("={f}")
                    }
                }
            };
            sheet.set_formula("Char.Color", formula, Some(Unit::Color));
            if let (Some(c), Some(cell)) = (resolved_col, sheet.get_cell_mut("Char.Color")) {
                let cv = vexio_shapesheet::CellValue::Color(c);
                cell.value = cv.clone();
                cell.vsdx_cached_value = Some(cv);
            }
        }
        "Style" => {
            // In `[MS-VSDX]`, `Char.Style` bit `8` (`0x08`) is `SmallCaps` (NOT strikethrough!).
            if let Some(vs) = &val {
                if let Ok(raw_bits) = vs.parse::<f64>() {
                    let bits = raw_bits.round() as u32;
                    let clean_bits = bits & 0x07;
                    sheet.set_formula("Char.Style", clean_bits.to_string(), Some(Unit::Number));
                    if (bits & 0x08) != 0 {
                        sheet.set_formula("Char.SmallCaps", "TRUE", Some(Unit::Boolean));
                    }
                    return;
                }
            }
            if let Some(formula) = cell_formula_from_attrs_opt(e, Some(Unit::Number), tables) {
                sheet.set_formula("Char.Style", formula, Some(Unit::Number));
            }
        }
        "Strikethru" | "DoubleStrikethrough" => {
            if let Some(vs) = &val {
                let on = vs == "1" || vs.eq_ignore_ascii_case("true");
                sheet.set_formula(
                    "Char.Strikethru",
                    if on { "TRUE" } else { "FALSE" },
                    Some(Unit::Boolean),
                );
            }
        }
        "Font" => {
            if let Some(vs) = &val {
                let resolved_font = if let Ok(fid) = vs.trim().parse::<u32>() {
                    tables
                        .face_names
                        .get(&fid)
                        .cloned()
                        .unwrap_or_else(|| "Segoe UI".into())
                } else {
                    vs.trim_matches('"').to_string()
                };
                if !resolved_font.is_empty() {
                    let quoted = format!("\"{}\"", resolved_font.replace('"', "\"\""));
                    sheet.set_formula("Char.Font", quoted, Some(Unit::String));
                    if let Some(cell) = sheet.get_cell_mut("Char.Font") {
                        let cv = vexio_shapesheet::CellValue::Str(resolved_font);
                        cell.value = cv.clone();
                        cell.vsdx_cached_value = Some(cv);
                    }
                }
            }
        }
        _ => {}
    }
}

fn parse_geometry_row(
    reader: &mut Reader<&[u8]>,
    row_start: &BytesStart<'_>,
) -> Result<Option<GeometryRow>, VsdxError> {
    let row_type = attr_str(row_start, b"T").unwrap_or_else(|| "LineTo".into());
    let ix = attr_str(row_start, b"IX").and_then(|s| s.parse().ok()).unwrap_or(1);
    let deleted = attr_str(row_start, b"Del").map(|d| d == "1").unwrap_or(false);
    let Some(kind) = GeometryRowKind::from_vsdx_tag(&row_type) else {
        skip_to_end(reader, b"Row")?;
        return Ok(None);
    };
    let default_xy_unit = if matches!(
        kind,
        GeometryRowKind::RelMoveTo
            | GeometryRowKind::RelLineTo
            | GeometryRowKind::RelCubBezTo
            | GeometryRowKind::RelQuadBezTo
            | GeometryRowKind::RelEllipticalArcTo
    ) {
        Some(Unit::Number)
    } else {
        Some(Unit::Inch)
    };
    let cells = read_geometry_row_cells(reader, kind, default_xy_unit)?;
    Ok(Some(GeometryRow {
        ix,
        kind,
        x: cells.get("X").cloned().unwrap_or_else(|| "Inh".into()),
        y: cells.get("Y").cloned().unwrap_or_else(|| "Inh".into()),
        a: cells.get("A").cloned(),
        b: cells.get("B").cloned(),
        c: cells.get("C").cloned(),
        d: cells.get("D").cloned(),
        e: cells.get("E").cloned(),
        deleted,
    }))
}

fn geometry_row_cell_default_unit(
    kind: GeometryRowKind,
    cell_name: &str,
    default_xy_unit: Option<Unit>,
) -> Option<Unit> {
    match cell_name {
        "X" | "Y" => default_xy_unit,
        "A" => match kind {
            GeometryRowKind::ArcTo
            | GeometryRowKind::Ellipse
            | GeometryRowKind::EllipticalArcTo => Some(Unit::Inch),
            GeometryRowKind::RelEllipticalArcTo
            | GeometryRowKind::RelQuadBezTo
            | GeometryRowKind::RelCubBezTo => Some(Unit::Number),
            _ => None,
        },
        "B" => match kind {
            GeometryRowKind::Ellipse | GeometryRowKind::EllipticalArcTo => Some(Unit::Inch),
            GeometryRowKind::RelEllipticalArcTo
            | GeometryRowKind::RelQuadBezTo
            | GeometryRowKind::RelCubBezTo => Some(Unit::Number),
            _ => None,
        },
        "C" => match kind {
            GeometryRowKind::Ellipse => Some(Unit::Inch),
            GeometryRowKind::EllipticalArcTo | GeometryRowKind::RelEllipticalArcTo => {
                Some(Unit::Radian)
            }
            GeometryRowKind::RelCubBezTo => Some(Unit::Number),
            _ => None,
        },
        "D" => match kind {
            GeometryRowKind::Ellipse => Some(Unit::Inch),
            GeometryRowKind::EllipticalArcTo
            | GeometryRowKind::RelEllipticalArcTo
            | GeometryRowKind::RelCubBezTo => Some(Unit::Number),
            _ => None,
        },
        _ => None,
    }
}

fn read_geometry_row_cells(
    reader: &mut Reader<&[u8]>,
    kind: GeometryRowKind,
    default_xy_unit: Option<Unit>,
) -> Result<BTreeMap<String, String>, VsdxError> {
    let dummy_tables = DocParseTables::default();
    let mut map = BTreeMap::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                if let Some(n) = attr_str(&e, b"N") {
                    let hint = geometry_row_cell_default_unit(kind, &n, default_xy_unit);
                    if let Some(f) = cell_formula_from_attrs_opt(&e, hint, &dummy_tables) {
                        map.insert(n, f);
                    }
                }
            }
            Event::Start(e) if e.local_name().as_ref() == b"Cell" => {
                if let Some(n) = attr_str(&e, b"N") {
                    let hint = geometry_row_cell_default_unit(kind, &n, default_xy_unit);
                    if let Some(f) = cell_formula_from_attrs_opt(&e, hint, &dummy_tables) {
                        map.insert(n, f);
                    }
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

fn read_row_cells(reader: &mut Reader<&[u8]>) -> Result<BTreeMap<String, String>, VsdxError> {
    let dummy = DocParseTables::default();
    let mut map = BTreeMap::new();
    for (k, v) in read_row_cells_opt(reader, &dummy)? {
        map.insert(k, v);
    }
    Ok(map)
}

fn read_row_cells_opt(
    reader: &mut Reader<&[u8]>,
    tables: &DocParseTables,
) -> Result<BTreeMap<String, String>, VsdxError> {
    let mut map = BTreeMap::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Empty(e) if e.local_name().as_ref() == b"Cell" => {
                if let Some(n) = attr_str(&e, b"N") {
                    if let Some(f) = cell_formula_from_attrs_opt(&e, None, tables) {
                        map.insert(n, f);
                    }
                }
            }
            Event::Start(e) if e.local_name().as_ref() == b"Cell" => {
                if let Some(n) = attr_str(&e, b"N") {
                    if let Some(f) = cell_formula_from_attrs_opt(&e, None, tables) {
                        map.insert(n, f);
                    }
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

fn cell_formula_from_attrs_opt(
    e: &BytesStart<'_>,
    fallback_unit: Option<Unit>,
    tables: &DocParseTables,
) -> Option<String> {
    let val = attr_str(e, b"V");
    let unit = attr_str(e, b"U").as_deref().and_then(Unit::from_token).or(fallback_unit);
    match attr_str(e, b"F").as_deref() {
        Some("Inh") | None => val.as_deref().map(|v| {
            if unit == Some(Unit::Color) {
                if let Some(c) = tables.resolve_color_token(v) {
                    return format!("=RGB({},{},{})", c.r, c.g, c.b);
                }
            }
            format_vsdx_value_as_formula(v, unit)
        }),
        Some("No Formula") => Some(
            val.as_deref()
                .map(|v| format_vsdx_value_as_formula(v, unit))
                .unwrap_or_else(|| "0".into()),
        ),
        Some(f) => {
            // If `F` is a theme expression (`THEMEVAL` etc.) inside a structured row where we only store formula string,
            // prefer the exact Visio-computed `V` literal when present so row coordinates/booleans never fail!
            let f_up = f.to_ascii_uppercase();
            if f_up.contains("THEME") {
                if let Some(v) = &val {
                    return Some(format_vsdx_value_as_formula(v, unit));
                }
            }
            Some(if f.starts_with('=') {
                f.to_string()
            } else {
                format!("={f}")
            })
        }
    }
}

fn finalize_geometry_defaults_recursive(shape: &mut VisioShape) {
    for g in &mut shape.sheet.geometries {
        if g.no_fill.eq_ignore_ascii_case("Inh") {
            g.no_fill = "FALSE".into();
        }
        if g.no_line.eq_ignore_ascii_case("Inh") {
            g.no_line = "FALSE".into();
        }
        if g.no_show.eq_ignore_ascii_case("Inh") {
            g.no_show = "FALSE".into();
        }
        if g.no_snap.eq_ignore_ascii_case("Inh") {
            g.no_snap = "FALSE".into();
        }
        if g.no_quick_drag.eq_ignore_ascii_case("Inh") {
            g.no_quick_drag = "FALSE".into();
        }
        for r in &mut g.rows {
            if r.x.eq_ignore_ascii_case("Inh") {
                r.x = "0 in".into();
            }
            if r.y.eq_ignore_ascii_case("Inh") {
                r.y = "0 in".into();
            }
        }
    }
    shape.sheet.sync_all_sections_to_cells();
    for c in &mut shape.children {
        finalize_geometry_defaults_recursive(c);
    }
}

/// Materializes Master group children, merges sparse `<Section N="Geometry">` overrides,
/// propagates `master_id` / `master_shape_id`, and remaps internal Master `Sheet.<msid>!`
/// references to the instantiated page shape IDs across all pages.
fn hydrate_master_instances(doc: &mut VisioDocument) {
    let masters_snapshot = doc.masters.clone();
    for page in &mut doc.pages {
        let mut next_id = page.next_shape_id();
        for top_shape in &mut page.shapes {
            hydrate_shape_against_masters(
                top_shape,
                None,
                &masters_snapshot,
                &mut next_id,
                0,
            );
            finalize_geometry_defaults_recursive(top_shape);
        }
    }
}

fn hydrate_shape_against_masters(
    shape: &mut VisioShape,
    enclosing_master_id: Option<u32>,
    masters: &[VisioMaster],
    next_id: &mut u64,
    depth: u8,
) {
    if depth > 32 {
        return;
    }
    let effective_mid = shape.master_id.or(enclosing_master_id);
    if shape.master_id.is_none() && shape.master_shape_id.is_some() {
        shape.master_id = effective_mid;
    }

    let master_shape_opt = effective_mid.and_then(|mid| {
        let m = masters.iter().find(|m| m.id == mid)?;
        if let Some(msid) = shape.master_shape_id {
            m.shapes.iter().find_map(|s| s.find_shape(msid))
        } else if shape.master_id.is_some() {
            m.shapes.first()
        } else {
            None
        }
    });

    if let Some(ms) = master_shape_opt {
        if shape.master_shape_id.is_none() {
            shape.master_shape_id = Some(ms.id);
        }
        // Inherit shape kind (`Group` / `ForeignObject` / `Connector1D`), style IDs, and `foreign_data`
        if shape.kind == ShapeKind::Shape2D && ms.kind != ShapeKind::Shape2D {
            shape.kind = ms.kind;
        }
        if shape.line_style_id.is_none() {
            shape.line_style_id = ms.line_style_id;
        }
        if shape.fill_style_id.is_none() {
            shape.fill_style_id = ms.fill_style_id;
        }
        if shape.text_style_id.is_none() {
            shape.text_style_id = ms.text_style_id;
        }
        if shape
            .foreign_data
            .as_ref()
            .map(|fd| fd.data.is_empty())
            .unwrap_or(true)
        {
            if let Some(mfd) = &ms.foreign_data {
                if !mfd.data.is_empty() {
                    shape.foreign_data = Some(mfd.clone());
                }
            }
        }
        let eff_hide_text = shape
            .sheet
            .get_bool("HideText")
            .or_else(|| ms.sheet.get_bool("HideText"))
            .unwrap_or(false);
        let explicit_empty_text = shape
            .opaque_xml_fragments
            .iter()
            .any(|f| f.trim() == "<Text/>");
        if shape.text.is_empty() && !ms.text.is_empty() && !eff_hide_text && !explicit_empty_text {
            shape.text = ms.text.clone();
        }

        // Merge Master `GeometrySection`s with sparse local overrides!
        merge_master_geometries(&ms.sheet.geometries, &mut shape.sheet);

        // Merge Master `user_cells`, `controls`, `connection_points`, and `shape_data` when not overridden locally
        for (uk, urow) in &ms.sheet.user_cells {
            if !shape.sheet.user_cells.contains_key(uk) {
                shape.sheet.user_cells.insert(uk.clone(), urow.clone());
            }
        }
        if shape.sheet.controls.is_empty() && !ms.sheet.controls.is_empty() {
            shape.sheet.controls = ms.sheet.controls.clone();
        }
        if shape.sheet.connection_points.is_empty() && !ms.sheet.connection_points.is_empty() {
            shape.sheet.connection_points = ms.sheet.connection_points.clone();
        }
        for (pk, prow) in &ms.sheet.shape_data {
            if !shape.sheet.shape_data.contains_key(pk) {
                shape.sheet.shape_data.insert(pk.clone(), prow.clone());
            }
        }

        // Merge / instantiate Master `children` for Group masters!
        if !ms.children.is_empty() {
            shape.kind = ShapeKind::Group;
            for mchild in &ms.children {
                let exists = shape
                    .children
                    .iter()
                    .any(|c| c.master_shape_id == Some(mchild.id));
                if !exists {
                    let cloned = clone_master_subtree_with_fresh_ids(
                        mchild,
                        effective_mid,
                        next_id,
                        0,
                    );
                    shape.children.push(cloned);
                }
            }
            shape.children.retain(|c| c.name != "__del__");
        } else {
            shape.children.retain(|c| c.name != "__del__");
        }
    }

    for child in &mut shape.children {
        hydrate_shape_against_masters(child, effective_mid, masters, next_id, depth + 1);
    }

    // If `shape` is the root of a Master instance (`shape.master_id.is_some()` and `enclosing_master_id.is_none()`),
    // build `msid -> instance_id` map across the subtree, copy missing Master cells with remapped `Sheet.<msid>!`,
    // and scale literal group child coordinates if the group instance was resized!
    if shape.master_id.is_some() && enclosing_master_id.is_none() {
        let mut id_map = BTreeMap::new();
        collect_msid_to_instance_id_map(shape, &mut id_map);
        if let Some(ms) = master_shape_opt {
            apply_master_cell_inheritance_and_remap_recursive(shape, ms, masters, &id_map, 0);
        }
    }
}

fn clone_master_subtree_with_fresh_ids(
    mshape: &VisioShape,
    master_id: Option<u32>,
    next_id: &mut u64,
    depth: u8,
) -> VisioShape {
    let my_id = *next_id;
    *next_id += 1;
    let mut cloned = mshape.clone();
    cloned.id = my_id;
    cloned.master_id = master_id;
    cloned.master_shape_id = Some(mshape.id);
    if depth > 32 {
        cloned.children.clear();
        return cloned;
    }
    cloned.children = mshape
        .children
        .iter()
        .map(|c| clone_master_subtree_with_fresh_ids(c, master_id, next_id, depth + 1))
        .collect();
    cloned
}

fn collect_msid_to_instance_id_map(node: &VisioShape, map: &mut BTreeMap<u64, u64>) {
    if let Some(msid) = node.master_shape_id {
        map.insert(msid, node.id);
    }
    for c in &node.children {
        collect_msid_to_instance_id_map(c, map);
    }
}

fn apply_master_cell_inheritance_and_remap_recursive(
    inst: &mut VisioShape,
    mshape: &VisioShape,
    masters: &[VisioMaster],
    id_map: &BTreeMap<u64, u64>,
    depth: u8,
) {
    if depth > 32 {
        return;
    }
    // Copy non-overridden Master cells onto `inst` with `Sheet.<msid>!` rewritten to `Sheet.<inst_id>!`
    for (ck, mcell) in &mshape.sheet.cells {
        if mcell.deleted {
            continue;
        }
        if !inst.sheet.cells.contains_key(ck) {
            let mut inherited = mcell.clone();
            let rewritten = rewrite_master_sheet_refs(&inherited.formula, id_map);
            if rewritten != inherited.formula {
                let hint = inherited.unit_hint;
                let cached = inherited.vsdx_cached_value.clone();
                inherited = vexio_shapesheet::Cell::from_formula(inherited.name, rewritten, hint);
                inherited.vsdx_cached_value = cached;
            }
            inst.sheet.cells.insert(ck.clone(), inherited);
        } else if let Some(existing) = inst.sheet.cells.get_mut(ck) {
            let rewritten = rewrite_master_sheet_refs(&existing.formula, id_map);
            if rewritten != existing.formula {
                let hint = existing.unit_hint;
                let cached = existing.vsdx_cached_value.clone();
                let nm = existing.name.clone();
                *existing = vexio_shapesheet::Cell::from_formula(nm, rewritten, hint);
                existing.vsdx_cached_value = cached;
            }
        }
    }
    // Also rewrite `Sheet.<msid>!` inside Geometry rows and User cells!
    for geom in &mut inst.sheet.geometries {
        geom.no_fill = rewrite_master_sheet_refs(&geom.no_fill, id_map);
        geom.no_line = rewrite_master_sheet_refs(&geom.no_line, id_map);
        geom.no_show = rewrite_master_sheet_refs(&geom.no_show, id_map);
        for r in &mut geom.rows {
            r.x = rewrite_master_sheet_refs(&r.x, id_map);
            r.y = rewrite_master_sheet_refs(&r.y, id_map);
            if let Some(a) = &mut r.a {
                *a = rewrite_master_sheet_refs(a, id_map);
            }
            if let Some(b) = &mut r.b {
                *b = rewrite_master_sheet_refs(b, id_map);
            }
            if let Some(c) = &mut r.c {
                *c = rewrite_master_sheet_refs(c, id_map);
            }
            if let Some(d) = &mut r.d {
                *d = rewrite_master_sheet_refs(d, id_map);
            }
        }
    }
    for u in inst.sheet.user_cells.values_mut() {
        u.value = rewrite_master_sheet_refs(&u.value, id_map);
    }

    for child in &mut inst.children {
        let child_ms = child
            .master_shape_id
            .and_then(|msid| mshape.find_shape(msid))
            .or_else(|| {
                let mid = child.master_id?;
                let m = masters.iter().find(|m| m.id == mid)?;
                let msid = child.master_shape_id?;
                m.shapes.iter().find_map(|s| s.find_shape(msid))
            });
        if let Some(cms) = child_ms {
            apply_master_cell_inheritance_and_remap_recursive(child, cms, masters, id_map, depth + 1);
        }
    }
}

fn rewrite_master_sheet_refs(formula: &str, id_map: &BTreeMap<u64, u64>) -> String {
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
                    if let Ok(msid) = formula[i + 6..j].parse::<u64>() {
                        if let Some(&new_id) = id_map.get(&msid) {
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

fn merge_master_geometries(master_geoms: &[GeometrySection], inst_sheet: &mut ShapeSheet) {
    if master_geoms.is_empty() {
        return;
    }
    if inst_sheet.geometries.is_empty() {
        inst_sheet.geometries = master_geoms.to_vec();
        return;
    }
    let local_overrides = std::mem::take(&mut inst_sheet.geometries);
    let mut merged: Vec<GeometrySection> = master_geoms.to_vec();

    for ov in local_overrides {
        if let Some(target) = merged.iter_mut().find(|g| g.index == ov.index) {
            if ov.deleted {
                target.deleted = true;
                continue;
            }
            if !ov.no_fill.eq_ignore_ascii_case("Inh") {
                target.no_fill = ov.no_fill;
            }
            if !ov.no_line.eq_ignore_ascii_case("Inh") {
                target.no_line = ov.no_line;
            }
            if !ov.no_show.eq_ignore_ascii_case("Inh") {
                target.no_show = ov.no_show;
            }
            if !ov.no_snap.eq_ignore_ascii_case("Inh") {
                target.no_snap = ov.no_snap;
            }
            if !ov.no_quick_drag.eq_ignore_ascii_case("Inh") {
                target.no_quick_drag = ov.no_quick_drag;
            }
            for ov_row in ov.rows {
                if let Some(trow) = target.rows.iter_mut().find(|r| r.ix == ov_row.ix) {
                    if ov_row.deleted {
                        trow.deleted = true;
                        continue;
                    }
                    trow.kind = ov_row.kind;
                    if !ov_row.x.eq_ignore_ascii_case("Inh") {
                        trow.x = ov_row.x;
                    }
                    if !ov_row.y.eq_ignore_ascii_case("Inh") {
                        trow.y = ov_row.y;
                    }
                    if ov_row.a.is_some() {
                        trow.a = ov_row.a;
                    }
                    if ov_row.b.is_some() {
                        trow.b = ov_row.b;
                    }
                    if ov_row.c.is_some() {
                        trow.c = ov_row.c;
                    }
                    if ov_row.d.is_some() {
                        trow.d = ov_row.d;
                    }
                    if ov_row.e.is_some() {
                        trow.e = ov_row.e;
                    }
                } else if !ov_row.deleted {
                    target.rows.push(ov_row);
                }
            }
            target.rows.sort_by_key(|r| r.ix);
        } else if !ov.deleted {
            merged.push(ov);
        }
    }
    merged.sort_by_key(|g| g.index);
    inst_sheet.geometries = merged;
}

fn read_text_element(reader: &mut Reader<&[u8]>) -> Result<String, VsdxError> {
    let mut out = String::new();
    let mut buf = Vec::new();
    let mut depth = 1usize;
    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                if e.local_name().as_ref() == b"pp" && !out.is_empty() && !out.ends_with('\n') {
                    // Preserve paragraph breaks if Visio emitted `<pp IX="..."/>` between paragraphs without newline
                }
                depth += 1;
            }
            Event::End(e) => {
                depth = depth.saturating_sub(1);
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
                depth = depth.saturating_sub(1);
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
                depth = depth.saturating_sub(1);
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
                depth = depth.saturating_sub(1);
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

