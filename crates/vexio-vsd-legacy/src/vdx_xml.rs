//! DatadiagramML (`.vdx` Visio 2003–2010 XML) and Visio Web Drawing (`.vdw`) parser & writer.

use crate::vsd_cfb::LegacyVisioError;
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use std::fmt::Write as _;
use std::io::{Cursor, Read};
use vexio_shapesheet::{
    GeometryRow, GeometrySection, GlueEndpoint, ReactiveEngine, ShapeKind, Unit, VisioDocument,
    VisioPage, VisioShape, format_clean_f64,
};
use zip::ZipArchive;

pub fn read_vdx_xml(xml: &str) -> Result<VisioDocument, LegacyVisioError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut doc = VisioDocument::new_blank("VDX Drawing");
    doc.pages.clear();

    let mut cur_page: Option<VisioPage> = None;
    let mut cur_shape: Option<VisioShape> = None;
    let mut cur_geom: Option<GeometrySection> = None;
    let mut cur_row: Option<GeometryRow> = None;
    let mut active_leaf_tag = String::new();
    let mut active_formula: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                match tag.as_str() {
                    "Title" => active_leaf_tag = "DocTitle".into(),
                    "Page" => {
                        let id = attr(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
                        let name = attr(&e, b"Name").unwrap_or_else(|| format!("Page-{id}"));
                        cur_page = Some(VisioPage::new(id, name, 11.0, 8.5));
                    }
                    "Shape" => {
                        let id = attr(&e, b"ID").and_then(|s| s.parse().ok()).unwrap_or(1);
                        let name = attr(&e, b"Name").unwrap_or_else(|| format!("Sheet.{id}"));
                        cur_shape = Some(VisioShape::new_rect(id, name, 1.0, 1.0, 2.0, 1.0));
                        if let Some(s) = cur_shape.as_mut() {
                            s.sheet.geometries.clear();
                        }
                    }
                    "Geom" => {
                        let ix = attr(&e, b"IX").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0) + 1;
                        cur_geom = Some(GeometrySection::new(ix));
                    }
                    "MoveTo" | "LineTo" | "ArcTo" => {
                        let ix = attr(&e, b"IX").and_then(|s| s.parse().ok()).unwrap_or(1);
                        let row = match tag.as_str() {
                            "MoveTo" => GeometryRow::move_to(ix, "0 in", "0 in"),
                            "ArcTo" => GeometryRow::arc_to(ix, "0 in", "0 in", "0 in"),
                            _ => GeometryRow::line_to(ix, "0 in", "0 in"),
                        };
                        cur_row = Some(row);
                    }
                    "Connect" => {
                        if let Some(p) = cur_page.as_mut() {
                            p.connects.push(parse_vdx_connect(&e));
                        }
                    }
                    other => {
                        active_formula = attr(&e, b"F");
                        active_leaf_tag = other.to_string();
                    }
                }
            }
            Event::Empty(e) if e.local_name().as_ref() == b"Connect" => {
                if let Some(p) = cur_page.as_mut() {
                    p.connects.push(parse_vdx_connect(&e));
                }
            }
            Event::Text(t) => {
                let text_val = t.unescape().map(|c| c.into_owned()).unwrap_or_default();
                if active_leaf_tag == "DocTitle" {
                    doc.properties.title = text_val;
                } else if let Some(row) = cur_row.as_mut() {
                    let expr_str = active_formula
                        .as_deref()
                        .filter(|f| *f != "Inh" && *f != "No Formula")
                        .map(|f| format!("={f}"))
                        .unwrap_or_else(|| format!("{text_val} in"));
                    match active_leaf_tag.as_str() {
                        "X" => row.x = expr_str,
                        "Y" => row.y = expr_str,
                        "A" => row.a = Some(expr_str),
                        _ => {}
                    }
                } else if let Some(sh) = cur_shape.as_mut() {
                    if active_leaf_tag == "Text" {
                        sh.text.push_str(&text_val);
                    } else if matches!(
                        active_leaf_tag.as_str(),
                        "PinX"
                            | "PinY"
                            | "Width"
                            | "Height"
                            | "LocPinX"
                            | "LocPinY"
                            | "BeginX"
                            | "BeginY"
                            | "EndX"
                            | "EndY"
                            | "Angle"
                            | "FillForegnd"
                            | "LineColor"
                            | "LineWeight"
                    ) {
                        let unit = match active_leaf_tag.as_str() {
                            "Angle" => Unit::Radian,
                            "FillForegnd" | "LineColor" => Unit::Color,
                            _ => Unit::Inch,
                        };
                        let f = active_formula
                            .as_deref()
                            .filter(|f| *f != "Inh" && *f != "No Formula")
                            .map(|f| format!("={f}"))
                            .unwrap_or_else(|| {
                                if unit == Unit::Color && text_val.starts_with('#') {
                                    if let Some(c) = vexio_shapesheet::RgbaColor::from_hex(&text_val) {
                                        return format!("=RGB({},{},{})", c.r, c.g, c.b);
                                    }
                                }
                                format!("{text_val}{}", unit.suffix())
                            });
                        sh.sheet.set_formula(active_leaf_tag.clone(), f, Some(unit));
                        if active_leaf_tag == "BeginX" {
                            sh.kind = ShapeKind::Connector1D;
                        }
                    }
                } else if let Some(p) = cur_page.as_mut() {
                    if active_leaf_tag == "PageWidth" || active_leaf_tag == "PageHeight" {
                        p.page_sheet.set_formula(
                            active_leaf_tag.clone(),
                            format!("{text_val} in"),
                            Some(Unit::Inch),
                        );
                    }
                }
            }
            Event::End(e) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                match tag.as_str() {
                    "MoveTo" | "LineTo" | "ArcTo" => {
                        if let (Some(g), Some(r)) = (cur_geom.as_mut(), cur_row.take()) {
                            g.rows.push(r);
                        }
                    }
                    "Geom" => {
                        if let (Some(sh), Some(g)) = (cur_shape.as_mut(), cur_geom.take()) {
                            sh.sheet.add_geometry(g);
                        }
                    }
                    "Shape" => {
                        if let (Some(p), Some(mut sh)) = (cur_page.as_mut(), cur_shape.take()) {
                            if sh.sheet.geometries.is_empty() && sh.kind == ShapeKind::Shape2D {
                                sh.sheet.add_geometry(GeometrySection::rectangle(1));
                            }
                            p.shapes.push(sh);
                        }
                    }
                    "Page" => {
                        if let Some(p) = cur_page.take() {
                            doc.pages.push(p);
                        }
                    }
                    _ => {
                        active_leaf_tag.clear();
                        active_formula = None;
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if doc.pages.is_empty() {
        doc.pages.push(VisioPage::new(1, "Page-1", 11.0, 8.5));
    }
    ReactiveEngine::recalculate_document(&mut doc);
    Ok(doc)
}

pub fn write_vdx_xml(doc: &VisioDocument) -> String {
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <VisioDocument xmlns=\"http://schemas.microsoft.com/visio/2003/core\">\n",
    );
    let _ = writeln!(
        s,
        "<DocumentProperties><Title>{}</Title></DocumentProperties>",
        vexio_vsdx::opc::xml_escape(&doc.properties.title)
    );
    s.push_str("<Pages>\n");
    for p in &doc.pages {
        let _ = writeln!(
            s,
            "<Page ID=\"{}\" Name=\"{}\">",
            p.id,
            vexio_vsdx::opc::xml_escape(&p.name)
        );
        let _ = writeln!(
            s,
            "<PageSheet><PageProps><PageWidth>{}</PageWidth><PageHeight>{}</PageHeight></PageProps></PageSheet>",
            format_clean_f64(p.width_in()),
            format_clean_f64(p.height_in())
        );
        s.push_str("<Shapes>\n");
        for sh in &p.shapes {
            let ss = &sh.sheet;
            let _ = writeln!(
                s,
                "<Shape ID=\"{}\" Name=\"{}\" Type=\"Shape\">",
                sh.id,
                vexio_vsdx::opc::xml_escape(&sh.name)
            );
            let _ = writeln!(
                s,
                "<XForm><PinX>{}</PinX><PinY>{}</PinY><Width>{}</Width><Height>{}</Height><Angle>{}</Angle></XForm>",
                format_clean_f64(ss.get_f64("PinX").unwrap_or(0.0)),
                format_clean_f64(ss.get_f64("PinY").unwrap_or(0.0)),
                format_clean_f64(ss.get_f64("Width").unwrap_or(1.0)),
                format_clean_f64(ss.get_f64("Height").unwrap_or(1.0)),
                format_clean_f64(ss.get_f64("Angle").unwrap_or(0.0)),
            );
            if sh.kind == ShapeKind::Connector1D {
                let _ = writeln!(
                    s,
                    "<XForm1D><BeginX>{}</BeginX><BeginY>{}</BeginY><EndX>{}</EndX><EndY>{}</EndY></XForm1D>",
                    format_clean_f64(ss.get_f64("BeginX").unwrap_or(0.0)),
                    format_clean_f64(ss.get_f64("BeginY").unwrap_or(0.0)),
                    format_clean_f64(ss.get_f64("EndX").unwrap_or(1.0)),
                    format_clean_f64(ss.get_f64("EndY").unwrap_or(1.0)),
                );
            }
            if !sh.text.is_empty() {
                let _ = writeln!(s, "<Text>{}</Text>", vexio_vsdx::opc::xml_escape(&sh.text));
            }
            s.push_str("</Shape>\n");
        }
        s.push_str("</Shapes>\n");
        if !p.connects.is_empty() {
            s.push_str("<Connects>\n");
            for c in &p.connects {
                let _ = writeln!(
                    s,
                    "<Connect FromSheet=\"{}\" FromCell=\"{}\" ToSheet=\"{}\" ToCell=\"{}\"/>",
                    c.connector_shape_id,
                    vexio_vsdx::opc::xml_escape(&c.from_cell),
                    c.target_shape_id,
                    vexio_vsdx::opc::xml_escape(&c.to_cell)
                );
            }
            s.push_str("</Connects>\n");
        }
        s.push_str("</Page>\n");
    }
    s.push_str("</Pages>\n</VisioDocument>");
    s
}

/// Read a `.vdw` Visio Web Drawing archive (ZIP package containing VSD/XML/XAML streams).
pub fn read_vdw(bytes: &[u8]) -> Result<VisioDocument, LegacyVisioError> {
    if let Ok((doc, _)) = vexio_vsdx::read_vsdx(bytes) {
        return Ok(doc);
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().to_ascii_lowercase();
        if name.ends_with(".vsd") || name.contains("data") {
            let mut raw = Vec::new();
            entry.read_to_end(&mut raw)?;
            if raw.starts_with(b"\xD0\xCF\x11\xE0") {
                return crate::vsd_cfb::read_vsd_cfb(&raw);
            }
        } else if name.ends_with(".vdx") || name.ends_with(".xml") {
            let mut s = String::new();
            if entry.read_to_string(&mut s).is_ok() && s.contains("<VisioDocument") {
                return read_vdx_xml(&s);
            }
        }
    }
    Err(LegacyVisioError::InvalidHeader(
        "Unrecognized .vdw Web Drawing archive structure".into(),
    ))
}

fn attr(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key || a.key.local_name().as_ref() == key {
            if let Ok(v) = a.unescape_value() {
                return Some(v.into_owned());
            }
        }
    }
    None
}

fn parse_vdx_connect(e: &BytesStart<'_>) -> GlueEndpoint {
    let from_sheet = attr(e, b"FromSheet").and_then(|s| s.parse().ok()).unwrap_or(0);
    let from_cell = attr(e, b"FromCell").unwrap_or_else(|| "BeginX".into());
    let to_sheet = attr(e, b"ToSheet").and_then(|s| s.parse().ok()).unwrap_or(0);
    let to_cell = attr(e, b"ToCell").unwrap_or_else(|| "PinX".into());
    let is_dynamic = to_cell.eq_ignore_ascii_case("PinX");
    GlueEndpoint {
        connector_shape_id: from_sheet,
        from_cell,
        target_shape_id: to_sheet,
        to_cell,
        is_dynamic,
    }
}
