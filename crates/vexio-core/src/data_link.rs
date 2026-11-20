//! Two-Way External Data Linking (`.xlsx`, `.csv`, `.json`, SQL/SQLite),
//! Data Graphics (Icon Sets, Data Bars, Text Callouts, Color by Value),
//! Live Page Legends, and BOM/Inventory Shape Reports.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};
use thiserror::Error;
use vexio_shapesheet::{
    ReactiveEngine, RgbaColor, ShapeDataRow, ShapeDataType, ShapeKind, Unit, VisioDocument,
    VisioShape,
};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

#[derive(Debug, Error)]
pub enum DataLinkError {
    #[error("ZIP/XLSX error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Missing key column '{0}' in recordset")]
    MissingKeyColumn(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRecordset {
    pub id: u32,
    pub name: String,
    pub key_column: String,
    pub columns: Vec<String>,
    pub rows: Vec<BTreeMap<String, String>>,
}

impl DataRecordset {
    /// Parse RFC-4180 CSV into a `DataRecordset`.
    pub fn from_csv(id: u32, name: impl Into<String>, key_col: impl Into<String>, csv: &str) -> Result<Self, DataLinkError> {
        let mut lines = parse_csv_rows(csv);
        let headers = if lines.is_empty() {
            Vec::new()
        } else {
            lines.remove(0)
        };
        let key_column = key_col.into();
        if !headers.is_empty() && !headers.iter().any(|h| h.eq_ignore_ascii_case(&key_column)) {
            return Err(DataLinkError::MissingKeyColumn(key_column));
        }
        let mut rows = Vec::new();
        for vals in lines {
            if vals.iter().all(|s| s.trim().is_empty()) {
                continue;
            }
            let mut row = BTreeMap::new();
            for (i, h) in headers.iter().enumerate() {
                row.insert(h.clone(), vals.get(i).cloned().unwrap_or_default());
            }
            rows.push(row);
        }
        Ok(Self {
            id,
            name: name.into(),
            key_column,
            columns: headers,
            rows,
        })
    }

    pub fn to_csv(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.columns.iter().map(|c| escape_csv(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
        for r in &self.rows {
            let vals: Vec<String> = self
                .columns
                .iter()
                .map(|c| escape_csv(r.get(c).map(|s| s.as_str()).unwrap_or("")))
                .collect();
            out.push_str(&vals.join(","));
            out.push('\n');
        }
        out
    }

    /// Parse JSON array of objects into a `DataRecordset`.
    pub fn from_json(id: u32, name: impl Into<String>, key_col: impl Into<String>, json: &str) -> Result<Self, DataLinkError> {
        let parsed: Vec<BTreeMap<String, serde_json::Value>> = serde_json::from_str(json)?;
        let mut cols = Vec::new();
        let mut rows = Vec::new();
        for obj in parsed {
            let mut r = BTreeMap::new();
            for (k, v) in obj {
                if !cols.contains(&k) {
                    cols.push(k.clone());
                }
                let s = match v {
                    serde_json::Value::String(st) => st,
                    other => other.to_string(),
                };
                r.insert(k, s);
            }
            rows.push(r);
        }
        Ok(Self {
            id,
            name: name.into(),
            key_column: key_col.into(),
            columns: cols,
            rows,
        })
    }

    /// Native pure-Rust OpenXML `.xlsx` workbook reader (works 100% offline on Desktop, Mobile, and ChromeOS WASM!).
    pub fn from_xlsx_bytes(
        id: u32,
        name: impl Into<String>,
        key_col: impl Into<String>,
        xlsx_bytes: &[u8],
    ) -> Result<Self, DataLinkError> {
        let mut zip = ZipArchive::new(Cursor::new(xlsx_bytes))?;
        let mut shared_strings = Vec::new();
        if let Ok(mut ss_file) = zip.by_name("xl/sharedStrings.xml") {
            let mut xml = String::new();
            ss_file.read_to_string(&mut xml)?;
            shared_strings = parse_xlsx_shared_strings(&xml);
        }
        let mut sheet_xml = String::new();
        zip.by_name("xl/worksheets/sheet1.xml")?
            .read_to_string(&mut sheet_xml)?;
        let mut raw_rows = parse_xlsx_sheet(&sheet_xml, &shared_strings);
        let columns = if raw_rows.is_empty() {
            Vec::new()
        } else {
            raw_rows.remove(0)
        };
        let mut rows = Vec::new();
        for rvals in raw_rows {
            let mut map = BTreeMap::new();
            for (i, col) in columns.iter().enumerate() {
                map.insert(col.clone(), rvals.get(i).cloned().unwrap_or_default());
            }
            rows.push(map);
        }
        Ok(Self {
            id,
            name: name.into(),
            key_column: key_col.into(),
            columns,
            rows,
        })
    }

    /// Native pure-Rust OpenXML `.xlsx` workbook writer for two-way sync and BOM Shape Reports!
    pub fn to_xlsx_bytes(&self) -> Result<Vec<u8>, DataLinkError> {
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut zw = ZipWriter::new(&mut cursor);
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

            zw.start_file("[Content_Types].xml", opts)?;
            zw.write_all(
                b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                  <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
                  <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
                  <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
                  <Override PartName=\"/xl/workbook.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml\"/>\
                  <Override PartName=\"/xl/worksheets/sheet1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml\"/>\
                  </Types>",
            )?;

            zw.start_file("_rels/.rels", opts)?;
            zw.write_all(
                b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                  <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
                  <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"xl/workbook.xml\"/>\
                  </Relationships>",
            )?;

            zw.start_file("xl/workbook.xml", opts)?;
            zw.write_all(
                b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                  <workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\
                  <sheets><sheet name=\"Sheet1\" sheetId=\"1\" r:id=\"rId1\"/></sheets></workbook>",
            )?;

            zw.start_file("xl/_rels/workbook.xml.rels", opts)?;
            zw.write_all(
                b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                  <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
                  <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet\" Target=\"worksheets/sheet1.xml\"/>\
                  </Relationships>",
            )?;

            let mut sxml = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
                 <worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>",
            );
            // Header row
            sxml.push_str("<row r=\"1\">");
            for (ci, col) in self.columns.iter().enumerate() {
                let cref = format!("{}1", xlsx_col_letters(ci));
                let _ = write!(
                    sxml,
                    "<c r=\"{cref}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
                    vexio_vsdx::opc::xml_escape(col)
                );
            }
            sxml.push_str("</row>");
            for (ri, row) in self.rows.iter().enumerate() {
                let rnum = ri + 2;
                let _ = write!(sxml, "<row r=\"{rnum}\">");
                for (ci, col) in self.columns.iter().enumerate() {
                    let cref = format!("{}{rnum}", xlsx_col_letters(ci));
                    let val = row.get(col).map(|s| s.as_str()).unwrap_or("");
                    if val.parse::<f64>().is_ok() {
                        let _ = write!(sxml, "<c r=\"{cref}\"><v>{val}</v></c>");
                    } else {
                        let _ = write!(
                            sxml,
                            "<c r=\"{cref}\" t=\"inlineStr\"><is><t>{}</t></is></c>",
                            vexio_vsdx::opc::xml_escape(val)
                        );
                    }
                }
                sxml.push_str("</row>");
            }
            sxml.push_str("</sheetData></worksheet>");
            zw.start_file("xl/worksheets/sheet1.xml", opts)?;
            zw.write_all(sxml.as_bytes())?;
            zw.finish()?;
        }
        Ok(cursor.into_inner())
    }
}

/// Bind rows from `recordset` onto shapes on `page_id` matching `shape_prop_or_text` == `recordset.key_column`,
/// populating `Prop.<Column>` Shape Data rows automatically.
pub fn bind_recordset_to_page(
    doc: &mut VisioDocument,
    page_id: u32,
    recordset: &DataRecordset,
    match_prop_name: Option<&str>,
) -> usize {
    let mut linked_count = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        for sh in &mut page.shapes {
            if sh.kind != ShapeKind::Shape2D {
                continue;
            }
            let candidate_key = if let Some(pname) = match_prop_name {
                sh.sheet
                    .get_cell(&format!("Prop.{pname}"))
                    .and_then(|c| c.value.as_str_coerced().ok())
                    .unwrap_or_default()
            } else {
                sh.text.trim().to_string()
            };
            if candidate_key.is_empty() {
                continue;
            }
            if let Some((row_idx, row_map)) = recordset.rows.iter().enumerate().find(|(_, r)| {
                r.get(&recordset.key_column)
                    .map(|v| v.eq_ignore_ascii_case(&candidate_key))
                    .unwrap_or(false)
            }) {
                sh.bound_data_row = Some((recordset.id, row_idx as u64));
                for (col, val) in row_map {
                    let is_num = val.parse::<f64>().is_ok();
                    sh.sheet.set_shape_data(ShapeDataRow {
                        name: sanitize_prop_id(col),
                        label: col.clone(),
                        prompt: format!("Linked from {}", recordset.name),
                        data_type: if is_num {
                            ShapeDataType::Number
                        } else {
                            ShapeDataType::String
                        },
                        format: String::new(),
                        value: if is_num {
                            val.clone()
                        } else {
                            format!("\"{}\"", val.replace('"', "\"\""))
                        },
                        sort_key: "1".into(),
                        invisible: false,
                        ask: false,
                        lang_id: 1033,
                    });
                }
                linked_count += 1;
            }
        }
    }
    ReactiveEngine::recalculate_document(doc);
    linked_count
}

/// Two-way writeback: push edited `Prop.*` values from bound Visio shapes back into `recordset`!
pub fn sync_shapes_back_to_recordset(
    doc: &VisioDocument,
    page_id: u32,
    recordset: &mut DataRecordset,
) -> usize {
    let Some(page) = doc.page(page_id) else {
        return 0;
    };
    let mut updated = 0usize;
    for sh in &page.shapes {
        let Some((rs_id, row_idx)) = sh.bound_data_row else {
            continue;
        };
        if rs_id != recordset.id {
            continue;
        }
        let Some(row) = recordset.rows.get_mut(row_idx as usize) else {
            continue;
        };
        for col in &recordset.columns {
            let pid = sanitize_prop_id(col);
            if let Some(cell) = sh.sheet.get_cell(&format!("Prop.{pid}")) {
                if let Ok(s) = cell.value.as_str_coerced() {
                    let clean = s.trim_matches('"').to_string();
                    if row.get(col) != Some(&clean) {
                        row.insert(col.clone(), clean);
                        updated += 1;
                    }
                }
            }
        }
    }
    updated
}

// ============================================================================
// Data Graphics, Legends & BOM Shape Reports
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataGraphicItem {
    TextCallout {
        prop_name: String,
        prefix: String,
    },
    DataBar {
        prop_name: String,
        min_val: f64,
        max_val: f64,
        bar_color: RgbaColor,
    },
    IconSet {
        prop_name: String,
        green_min: f64,
        amber_min: f64,
    },
    ColorByValue {
        prop_name: String,
        palette: BTreeMap<String, RgbaColor>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataGraphicSpec {
    pub name: String,
    pub items: Vec<DataGraphicItem>,
}

/// Apply `DataGraphicSpec` to all shapes on `page_id` that carry the referenced `Prop.*` cells,
/// generating sub-shape badges/bars and Color-by-Value fills.
pub fn apply_data_graphics(doc: &mut VisioDocument, page_id: u32, spec: &DataGraphicSpec) -> usize {
    ReactiveEngine::recalculate_document(doc);
    let mut modified = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        let mut next_sub_id = page.next_shape_id() + 10_000;
        for sh in &mut page.shapes {
            if sh.kind != ShapeKind::Shape2D {
                continue;
            }
            // Remove prior Data Graphic sub-shapes before re-applying
            sh.children
                .retain(|c| !c.name_u.starts_with("DG_SubShape"));
            let w = sh.sheet.get_f64("Width").unwrap_or(2.0);
            let h = sh.sheet.get_f64("Height").unwrap_or(1.0);
            let mut touched = false;

            for item in &spec.items {
                match item {
                    DataGraphicItem::ColorByValue { prop_name, palette } => {
                        if let Some(c) = sh.sheet.get_cell(&format!("Prop.{}", sanitize_prop_id(prop_name))) {
                            if let Ok(val_s) = c.value.as_str_coerced() {
                                if let Some(col) = palette.get(val_s.trim_matches('"')) {
                                    sh.sheet.set_formula(
                                        "FillForegnd",
                                        format!("=RGB({},{},{})", col.r, col.g, col.b),
                                        Some(Unit::Color),
                                    );
                                    touched = true;
                                }
                            }
                        }
                    }
                    DataGraphicItem::TextCallout { prop_name, prefix } => {
                        if let Some(c) = sh.sheet.get_cell(&format!("Prop.{}", sanitize_prop_id(prop_name))) {
                            if let Ok(val_s) = c.value.as_str_coerced() {
                                let mut badge = VisioShape::new_rect(
                                    next_sub_id,
                                    format!("DG_SubShape.{next_sub_id}"),
                                    w * 0.5,
                                    h + 0.18,
                                    w * 0.9,
                                    0.28,
                                );
                                next_sub_id += 1;
                                badge.text = format!("{prefix}{val_s}");
                                badge
                                    .sheet
                                    .set_formula("FillForegnd", "=RGB(241,245,249)", Some(Unit::Color));
                                sh.children.push(badge);
                                touched = true;
                            }
                        }
                    }
                    DataGraphicItem::DataBar {
                        prop_name,
                        min_val,
                        max_val,
                        bar_color,
                    } => {
                        if let Some(v) = sh.sheet.get_f64(&format!("Prop.{}", sanitize_prop_id(prop_name))) {
                            let ratio = ((v - *min_val) / (*max_val - *min_val).max(1e-9)).clamp(0.05, 1.0);
                            let bar_w = (w * 0.85) * ratio;
                            let mut bar = VisioShape::new_rect(
                                next_sub_id,
                                format!("DG_SubShape.{next_sub_id}"),
                                (w * 0.075) + bar_w * 0.5,
                                0.12,
                                bar_w,
                                0.14,
                            );
                            next_sub_id += 1;
                            bar.sheet.set_formula(
                                "FillForegnd",
                                format!("=RGB({},{},{})", bar_color.r, bar_color.g, bar_color.b),
                                Some(Unit::Color),
                            );
                            sh.children.push(bar);
                            touched = true;
                        }
                    }
                    DataGraphicItem::IconSet {
                        prop_name,
                        green_min,
                        amber_min,
                    } => {
                        if let Some(v) = sh.sheet.get_f64(&format!("Prop.{}", sanitize_prop_id(prop_name))) {
                            let (col, sym) = if v >= *green_min {
                                (RgbaColor { r: 22, g: 163, b: 74, a: 255 }, "OK")
                            } else if v >= *amber_min {
                                (RgbaColor { r: 217, g: 119, b: 6, a: 255 }, "WARN")
                            } else {
                                (RgbaColor { r: 220, g: 38, b: 38, a: 255 }, "CRIT")
                            };
                            let mut icon = VisioShape::new_rect(
                                next_sub_id,
                                format!("DG_SubShape.{next_sub_id}"),
                                w - 0.18,
                                h - 0.18,
                                0.32,
                                0.24,
                            );
                            next_sub_id += 1;
                            icon.text = sym.into();
                            icon.sheet.set_formula(
                                "FillForegnd",
                                format!("=RGB({},{},{})", col.r, col.g, col.b),
                                Some(Unit::Color),
                            );
                            sh.children.push(icon);
                            touched = true;
                        }
                    }
                }
            }
            if touched {
                modified += 1;
            }
        }
    }
    ReactiveEngine::recalculate_document(doc);
    modified
}

/// Insert a live Page Legend summarizing Data Graphic categories and counts on `page_id`.
pub fn insert_page_legend(
    doc: &mut VisioDocument,
    page_id: u32,
    prop_name: &str,
    pin_x: f64,
    pin_y: f64,
) -> Option<u64> {
    let page = doc.page_mut(page_id)?;
    let key = format!("Prop.{}", sanitize_prop_id(prop_name));
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for sh in &page.shapes {
        if let Some(c) = sh.sheet.get_cell(&key) {
            if let Ok(v) = c.value.as_str_coerced() {
                *counts.entry(v.trim_matches('"').to_string()).or_insert(0) += 1;
            }
        }
    }
    let lid = page.next_shape_id();
    let mut lines = vec![format!("Legend — {prop_name}")];
    for (val, cnt) in &counts {
        lines.push(format!("• {val}: {cnt}"));
    }
    let h = 0.4 + 0.25 * counts.len() as f64;
    let mut legend = VisioShape::new_rect(lid, "Legend", pin_x, pin_y, 2.2, h.max(0.8));
    legend.text = lines.join("\n");
    legend
        .sheet
        .set_user_cell("msvStructureType", "\"Legend\"", "Page Legend");
    page.shapes.push(legend);
    ReactiveEngine::recalculate_document(doc);
    Some(lid)
}

/// Generate a BOM / Inventory Shape Report across all pages of `doc` for the requested Shape Data columns!
pub fn generate_bom_shape_report(doc: &VisioDocument, columns: &[&str]) -> DataRecordset {
    let mut all_cols = vec!["ShapeID".to_string(), "Page".to_string(), "Text".to_string()];
    for &c in columns {
        all_cols.push(c.to_string());
    }
    let mut rows = Vec::new();
    for page in &doc.pages {
        if page.is_background {
            continue;
        }
        for sh in &page.shapes {
            if sh.kind != ShapeKind::Shape2D || sh.name_u == "Legend" {
                continue;
            }
            let mut r = BTreeMap::new();
            r.insert("ShapeID".into(), sh.id.to_string());
            r.insert("Page".into(), page.name.clone());
            r.insert("Text".into(), sh.text.replace('\n', " "));
            for &col in columns {
                let val = sh
                    .sheet
                    .get_cell(&format!("Prop.{}", sanitize_prop_id(col)))
                    .or_else(|| sh.sheet.get_cell(col))
                    .and_then(|c| c.value.as_str_coerced().ok())
                    .unwrap_or_default();
                r.insert(col.to_string(), val.trim_matches('"').to_string());
            }
            rows.push(r);
        }
    }
    DataRecordset {
        id: 999,
        name: "BOM Inventory Report".into(),
        key_column: "ShapeID".into(),
        columns: all_cols,
        rows,
    }
}

fn sanitize_prop_id(col: &str) -> String {
    col.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn parse_csv_rows(csv: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut cur_row = Vec::new();
    let mut cur_field = String::new();
    let mut in_quotes = false;
    let mut chars = csv.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cur_field.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur_field.push(ch);
            }
        } else {
            match ch {
                '"' => in_quotes = true,
                ',' => {
                    cur_row.push(cur_field.trim().to_string());
                    cur_field.clear();
                }
                '\r' => {}
                '\n' => {
                    cur_row.push(cur_field.trim().to_string());
                    cur_field.clear();
                    rows.push(std::mem::take(&mut cur_row));
                }
                _ => cur_field.push(ch),
            }
        }
    }
    if !cur_field.is_empty() || !cur_row.is_empty() {
        cur_row.push(cur_field.trim().to_string());
        rows.push(cur_row);
    }
    rows
}

fn escape_csv(val: &str) -> String {
    if val.contains(',') || val.contains('"') || val.contains('\n') {
        format!("\"{}\"", val.replace('"', "\"\""))
    } else {
        val.to_string()
    }
}

fn xlsx_col_letters(mut idx: usize) -> String {
    let mut out = Vec::new();
    loop {
        out.push((b'A' + (idx % 26) as u8) as char);
        if idx < 26 {
            break;
        }
        idx = idx / 26 - 1;
    }
    out.into_iter().rev().collect()
}

fn xlsx_col_index(cell_ref: &str) -> usize {
    let mut idx = 0usize;
    let mut saw = false;
    for b in cell_ref.bytes() {
        if b.is_ascii_uppercase() {
            idx = idx * 26 + ((b - b'A') as usize + 1);
            saw = true;
        } else {
            break;
        }
    }
    if saw { idx - 1 } else { 0 }
}

fn parse_xlsx_shared_strings(xml: &str) -> Vec<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut in_si = false;
    let mut in_t = false;
    let mut cur = String::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"si" => {
                    in_si = true;
                    cur.clear();
                }
                b"t" if in_si => in_t = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_t => {
                if let Ok(s) = t.unescape() {
                    cur.push_str(&s);
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"t" => in_t = false,
                b"si" => {
                    in_si = false;
                    out.push(cur.clone());
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn parse_xlsx_sheet(xml: &str, shared: &[String]) -> Vec<Vec<String>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut rows = Vec::new();
    let mut cur_row: Vec<String> = Vec::new();
    let mut cur_col_idx = 0usize;
    let mut cell_type = String::new();
    let mut in_val = false;
    let mut cur_val = String::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"row" => cur_row.clear(),
                b"c" => {
                    cell_type.clear();
                    cur_val.clear();
                    for a in e.attributes().flatten() {
                        if a.key.local_name().as_ref() == b"r" {
                            if let Ok(r) = a.unescape_value() {
                                cur_col_idx = xlsx_col_index(&r);
                            }
                        } else if a.key.local_name().as_ref() == b"t" {
                            if let Ok(t) = a.unescape_value() {
                                cell_type = t.into_owned();
                            }
                        }
                    }
                }
                b"v" | b"t" => in_val = true,
                _ => {}
            },
            Ok(Event::Text(t)) if in_val => {
                if let Ok(s) = t.unescape() {
                    cur_val.push_str(&s);
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"v" | b"t" => in_val = false,
                b"c" => {
                    while cur_row.len() <= cur_col_idx {
                        cur_row.push(String::new());
                    }
                    let resolved = if cell_type == "s" {
                        cur_val
                            .trim()
                            .parse::<usize>()
                            .ok()
                            .and_then(|idx| shared.get(idx).cloned())
                            .unwrap_or_default()
                    } else {
                        cur_val.clone()
                    };
                    cur_row[cur_col_idx] = resolved;
                }
                b"row" => rows.push(std::mem::take(&mut cur_row)),
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    rows
}
