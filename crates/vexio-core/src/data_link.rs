//! Two-Way External Data Linking (`.xlsx`, `.csv`, `.json`, SQL/SQLite),
//! Data Graphics (Icon Sets, Data Bars, Text Callouts, Color by Value),
//! Live Page Legends, and BOM/Inventory Shape Reports.

use quick_xml::Reader;
use quick_xml::events::Event;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};
use thiserror::Error;
use vexio_shapesheet::{
    ExternalDataRecordset, PageDataGraphicConfig, ReactiveEngine, RgbaColor, ShapeDataRow,
    ShapeDataType, ShapeKind, Unit, VisioDocument, VisioShape, resolve_shape_prop_cell,
    sanitize_prop_identifier,
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
    #[error("SQLite error: {0}")]
    Sqlite(String),
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
    pub fn from_external(ext: &ExternalDataRecordset) -> Self {
        Self {
            id: ext.id,
            name: ext.name.clone(),
            key_column: ext.key_column.clone(),
            columns: ext.columns.clone(),
            rows: ext.rows.clone(),
        }
    }

    pub fn to_external(
        &self,
        source_kind: impl Into<String>,
        source_name: impl Into<String>,
    ) -> ExternalDataRecordset {
        ExternalDataRecordset {
            id: self.id,
            name: self.name.clone(),
            source_kind: source_kind.into(),
            source_name: source_name.into(),
            key_column: self.key_column.clone(),
            columns: self.columns.clone(),
            rows: self.rows.clone(),
        }
    }

    /// Parse RFC-4180 CSV (or TSV / semicolon-separated text) into a `DataRecordset`.
    pub fn from_csv(
        id: u32,
        name: impl Into<String>,
        key_col: impl Into<String>,
        csv: &str,
    ) -> Result<Self, DataLinkError> {
        let stripped = csv.strip_prefix('\u{FEFF}').unwrap_or(csv);
        let delim = detect_csv_delimiter(stripped);
        let mut lines = parse_delimited_rows(stripped, delim);
        let headers = if lines.is_empty() {
            Vec::new()
        } else {
            lines.remove(0)
        };
        let requested_key = key_col.into();
        let key_column = if requested_key.trim().is_empty() {
            pick_best_key_column(&headers, "")
        } else if !headers.is_empty() {
            if let Some(matched) = headers
                .iter()
                .find(|h| h.eq_ignore_ascii_case(requested_key.trim()))
            {
                matched.clone()
            } else {
                return Err(DataLinkError::MissingKeyColumn(requested_key));
            }
        } else {
            requested_key
        };

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
        out.push_str(
            &self
                .columns
                .iter()
                .map(|c| escape_csv(c))
                .collect::<Vec<_>>()
                .join(","),
        );
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

    /// Parse JSON array of objects (`[{...}]` or `{"rows":[...]}` / `{"data":[...]}`) into a `DataRecordset`.
    pub fn from_json(
        id: u32,
        name: impl Into<String>,
        key_col: impl Into<String>,
        json: &str,
    ) -> Result<Self, DataLinkError> {
        let stripped = json.strip_prefix('\u{FEFF}').unwrap_or(json).trim();
        let root_val: serde_json::Value = serde_json::from_str(stripped)?;
        let arr: Vec<BTreeMap<String, serde_json::Value>> = match root_val {
            serde_json::Value::Array(items) => {
                let mut out = Vec::new();
                for item in items {
                    if let serde_json::Value::Object(map) = item {
                        out.push(map.into_iter().collect());
                    }
                }
                out
            }
            serde_json::Value::Object(mut obj) => {
                let mut extracted = Vec::new();
                for wrapper_key in ["rows", "data", "items", "records"] {
                    if let Some(serde_json::Value::Array(items)) = obj.remove(wrapper_key) {
                        for item in items {
                            if let serde_json::Value::Object(map) = item {
                                extracted.push(map.into_iter().collect());
                            }
                        }
                        break;
                    }
                }
                extracted
            }
            _ => Vec::new(),
        };

        let mut cols = Vec::new();
        let mut rows = Vec::new();
        for obj in arr {
            let mut r = BTreeMap::new();
            for (k, v) in obj {
                if !cols.contains(&k) {
                    cols.push(k.clone());
                }
                let s = match v {
                    serde_json::Value::Null => String::new(),
                    serde_json::Value::String(st) => st,
                    serde_json::Value::Bool(b) => {
                        if b {
                            "true".into()
                        } else {
                            "false".into()
                        }
                    }
                    other => other.to_string(),
                };
                r.insert(k, s);
            }
            rows.push(r);
        }
        let requested_key = key_col.into();
        let key_column = pick_best_key_column(&cols, &requested_key);
        Ok(Self {
            id,
            name: name.into(),
            key_column,
            columns: cols,
            rows,
        })
    }

    pub fn to_json_pretty(&self) -> Result<String, DataLinkError> {
        let mut out_rows: Vec<BTreeMap<String, serde_json::Value>> =
            Vec::with_capacity(self.rows.len());
        for r in &self.rows {
            let mut map = BTreeMap::new();
            for col in &self.columns {
                let raw = r.get(col).cloned().unwrap_or_default();
                let val = if let Ok(i) = raw.parse::<i64>() {
                    serde_json::Value::Number(i.into())
                } else if let Ok(f) = raw.parse::<f64>() {
                    serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::String(raw))
                } else if raw.eq_ignore_ascii_case("true") {
                    serde_json::Value::Bool(true)
                } else if raw.eq_ignore_ascii_case("false") {
                    serde_json::Value::Bool(false)
                } else {
                    serde_json::Value::String(raw)
                };
                map.insert(col.clone(), val);
            }
            out_rows.push(map);
        }
        Ok(serde_json::to_string_pretty(&out_rows)?)
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
        let read_sheet1 = {
            if let Ok(mut f) = zip.by_name("xl/worksheets/sheet1.xml") {
                f.read_to_string(&mut sheet_xml)?;
                true
            } else {
                false
            }
        };
        if !read_sheet1 {
            // Fallback to first `xl/worksheets/*.xml` entry in the workbook
            let mut first_sheet_name: Option<String> = None;
            for idx in 0..zip.len() {
                if let Ok(zf) = zip.by_index(idx) {
                    let zname = zf.name().to_string();
                    if zname.starts_with("xl/worksheets/") && zname.ends_with(".xml") {
                        first_sheet_name = Some(zname);
                        break;
                    }
                }
            }
            let target = first_sheet_name.unwrap_or_else(|| "xl/worksheets/sheet1.xml".into());
            zip.by_name(&target)?.read_to_string(&mut sheet_xml)?;
        }
        let mut raw_rows = parse_xlsx_sheet(&sheet_xml, &shared_strings);
        let columns = if raw_rows.is_empty() {
            Vec::new()
        } else {
            raw_rows.remove(0)
        };
        let mut rows = Vec::new();
        for rvals in raw_rows {
            if rvals.iter().all(|s| s.trim().is_empty()) {
                continue;
            }
            let mut map = BTreeMap::new();
            for (i, col) in columns.iter().enumerate() {
                map.insert(col.clone(), rvals.get(i).cloned().unwrap_or_default());
            }
            rows.push(map);
        }
        let requested_key = key_col.into();
        let key_column = pick_best_key_column(&columns, &requested_key);
        Ok(Self {
            id,
            name: name.into(),
            key_column,
            columns,
            rows,
        })
    }

    /// Auto-detect `.xlsx`, `.sqlite`/`.db`, `.json`, or `.csv`/`.tsv` from raw bytes + hint,
    /// returning `(recordset, canonical_source_kind)`.
    pub fn from_auto_bytes(
        id: u32,
        name: impl Into<String>,
        format_hint: &str,
        bytes: &[u8],
        key_col_hint: &str,
        table_hint: &str,
    ) -> Result<(Self, &'static str), DataLinkError> {
        let name_str = name.into();
        let ext = format_hint
            .rsplit_once('.')
            .map(|(_, e)| e)
            .unwrap_or(format_hint)
            .trim()
            .to_ascii_lowercase();

        if bytes.starts_with(b"SQLite format 3\0")
            || matches!(ext.as_str(), "sqlite" | "sqlite3" | "db")
        {
            let tables =
                crate::sqlite_reader::list_sqlite_tables(bytes).map_err(DataLinkError::Sqlite)?;
            let chosen_table = if !table_hint.trim().is_empty()
                && tables.iter().any(|t| t.eq_ignore_ascii_case(table_hint.trim()))
            {
                table_hint.trim().to_string()
            } else {
                tables
                    .first()
                    .cloned()
                    .ok_or_else(|| DataLinkError::Sqlite("SQLite database contains no tables".into()))?
            };
            let mut rs = crate::sqlite_reader::read_sqlite_table_to_recordset(
                id,
                bytes,
                &chosen_table,
                "",
            )
            .map_err(DataLinkError::Sqlite)?;
            rs.name = if name_str.trim().is_empty() {
                chosen_table
            } else {
                name_str
            };
            rs.key_column = pick_best_key_column(&rs.columns, key_col_hint);
            return Ok((rs, "sqlite"));
        }

        if bytes.starts_with(b"PK\x03\x04") || ext == "xlsx" {
            let rs = Self::from_xlsx_bytes(id, name_str, key_col_hint, bytes)?;
            return Ok((rs, "xlsx"));
        }

        let text = String::from_utf8_lossy(bytes);
        let trimmed = text.strip_prefix('\u{FEFF}').unwrap_or(&text).trim();
        if ext == "json" || trimmed.starts_with('[') || trimmed.starts_with('{') {
            let rs = Self::from_json(id, name_str, key_col_hint, trimmed)?;
            return Ok((rs, "json"));
        }

        // For interactive import, if `key_col_hint` doesn't exist in this CSV, gracefully fall back to `pick_best_key_column`
        let rs = match Self::from_csv(id, name_str.clone(), key_col_hint, trimmed) {
            Ok(parsed) => parsed,
            Err(DataLinkError::MissingKeyColumn(_)) => Self::from_csv(id, name_str, "", trimmed)?,
            Err(other) => return Err(other),
        };
        Ok((rs, "csv"))
    }

    /// Export `DataRecordset` back out to `.xlsx`, `.sqlite`, `.json`, or `.csv` bytes for two-way writeback!
    pub fn export_bytes(&self, format_ext: &str, table_name: &str) -> Result<Vec<u8>, DataLinkError> {
        let ext = format_ext
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        match ext.as_str() {
            "xlsx" => self.to_xlsx_bytes(),
            "sqlite" | "sqlite3" | "db" => {
                let tbl = if table_name.trim().is_empty() {
                    sanitize_prop_id(&self.name)
                } else {
                    sanitize_prop_id(table_name)
                };
                let eff_tbl = if tbl.is_empty() { "vexio_records" } else { &tbl };
                Ok(crate::sqlite_reader::write_recordset_to_sqlite_bytes(
                    self, eff_tbl,
                ))
            }
            "json" => Ok(self.to_json_pretty()?.into_bytes()),
            _ => Ok(self.to_csv().into_bytes()),
        }
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

fn is_bindable_diagram_shape(sh: &VisioShape) -> bool {
    if !matches!(
        sh.kind,
        ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
    ) {
        return false;
    }
    if sh.name_u == "Legend" || sh.name_u.starts_with("Legend.") {
        return false;
    }
    if sh.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5 {
        return false;
    }
    if let Some(st) = sh.sheet.get_str("User.msvStructureType") {
        let clean = st.trim().trim_matches('"');
        if clean.eq_ignore_ascii_case("Container")
            || clean.eq_ignore_ascii_case("List")
            || clean.eq_ignore_ascii_case("Callout")
            || clean.eq_ignore_ascii_case("Legend")
        {
            return false;
        }
    }
    true
}

fn apply_recordset_row_to_shape(
    sh: &mut VisioShape,
    recordset_id: u32,
    recordset_name: &str,
    row_idx: usize,
    row_map: &BTreeMap<String, String>,
) {
    sh.bound_data_row = Some((recordset_id, row_idx as u64));
    for (col, val) in row_map {
        let trimmed = val.trim();
        let is_num = !trimmed.is_empty() && trimmed.parse::<f64>().is_ok();
        let is_bool = trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false");
        let (data_type, formula_val) = if is_num {
            (ShapeDataType::Number, trimmed.to_string())
        } else if is_bool {
            (
                ShapeDataType::Boolean,
                if trimmed.eq_ignore_ascii_case("true") {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                },
            )
        } else {
            (
                ShapeDataType::String,
                format!("\"{}\"", val.replace('"', "\"\"")),
            )
        };
        sh.sheet.set_shape_data(ShapeDataRow {
            name: sanitize_prop_id(col),
            label: col.clone(),
            prompt: format!("Linked from {recordset_name}"),
            data_type,
            format: String::new(),
            value: formula_val,
            sort_key: "1".into(),
            invisible: false,
            ask: false,
            lang_id: 1033,
        });
    }
}

/// Bind rows from `recordset` onto shapes on `page_id` matching `match_prop_name` (or shape text / name)
/// == `recordset.key_column`, populating `Prop.<Column>` Shape Data rows automatically.
pub fn bind_recordset_to_page(
    doc: &mut VisioDocument,
    page_id: u32,
    recordset: &DataRecordset,
    match_prop_name: Option<&str>,
) -> usize {
    let mut linked_count = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        for sh in &mut page.shapes {
            if !is_bindable_diagram_shape(sh) {
                continue;
            }
            let mut candidates: Vec<String> = Vec::new();
            if let Some(pname) = match_prop_name.filter(|s| !s.trim().is_empty()) {
                if let Some((_, val)) = resolve_shape_prop_cell(&sh.sheet, None, pname) {
                    candidates.push(val);
                }
            } else {
                let full_txt = sh.text.trim().to_string();
                if !full_txt.is_empty() {
                    candidates.push(full_txt.clone());
                    if let Some(first_line) = full_txt.lines().next() {
                        let fl = first_line.trim().to_string();
                        if !fl.is_empty() && fl != full_txt {
                            candidates.push(fl);
                        }
                    }
                }
                if !sh.name.trim().is_empty() {
                    candidates.push(sh.name.trim().to_string());
                }
                if let Some((_, existing_key)) =
                    resolve_shape_prop_cell(&sh.sheet, None, &recordset.key_column)
                {
                    candidates.push(existing_key);
                }
            }
            if candidates.is_empty() {
                continue;
            }
            if let Some((row_idx, row_map)) = recordset.rows.iter().enumerate().find(|(_, r)| {
                r.get(&recordset.key_column)
                    .map(|v| {
                        let target = v.trim();
                        !target.is_empty()
                            && candidates.iter().any(|c| c.eq_ignore_ascii_case(target))
                    })
                    .unwrap_or(false)
            }) {
                apply_recordset_row_to_shape(sh, recordset.id, &recordset.name, row_idx, row_map);
                linked_count += 1;
            }
        }
    }
    ReactiveEngine::recalculate_document(doc);
    linked_count
}

/// Bind a specific `row_index` from `recordset` directly onto `shape_ids` on `page_id`.
pub fn bind_recordset_row_to_shapes(
    doc: &mut VisioDocument,
    page_id: u32,
    recordset: &DataRecordset,
    row_index: usize,
    shape_ids: &[u64],
) -> usize {
    let Some(row_map) = recordset.rows.get(row_index) else {
        return 0;
    };
    let id_set: BTreeSet<u64> = shape_ids.iter().copied().collect();
    let mut count = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        for sid in id_set {
            if let Some(sh) = page.find_shape_mut(sid) {
                apply_recordset_row_to_shape(sh, recordset.id, &recordset.name, row_index, row_map);
                count += 1;
            }
        }
    }
    if count > 0 {
        ReactiveEngine::recalculate_document(doc);
    }
    count
}

/// Refresh all shapes already bound to `recordset.id` on `page_id` AND bind any newly matching shapes.
pub fn refresh_recordset_bindings_on_page(
    doc: &mut VisioDocument,
    page_id: u32,
    recordset: &DataRecordset,
) -> usize {
    let mut refreshed = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        for sh in &mut page.shapes {
            if !is_bindable_diagram_shape(sh) {
                continue;
            }
            if let Some((rs_id, row_idx)) = sh.bound_data_row {
                if rs_id == recordset.id {
                    if let Some(row_map) = recordset.rows.get(row_idx as usize) {
                        apply_recordset_row_to_shape(
                            sh,
                            recordset.id,
                            &recordset.name,
                            row_idx as usize,
                            row_map,
                        );
                        refreshed += 1;
                        continue;
                    }
                }
            }
        }
    }
    let newly_bound = bind_recordset_to_page(doc, page_id, recordset, None);
    refreshed.max(newly_bound)
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
            if let Some((_, clean_val)) = resolve_shape_prop_cell(&sh.sheet, None, col) {
                if row.get(col) != Some(&clean_val) {
                    row.insert(col.clone(), clean_val);
                    updated += 1;
                }
            }
        }
    }
    updated
}

/// Auto-seed a sensible `PageDataGraphicConfig` from the columns and sample values of `recordset`.
pub fn infer_data_graphic_config_from_recordset(recordset: &DataRecordset) -> PageDataGraphicConfig {
    let mut cfg = PageDataGraphicConfig {
        enabled: true,
        icon_set_field: String::new(),
        icon_green_min: 75.0,
        icon_amber_min: 40.0,
        data_bar_field: String::new(),
        data_bar_min: 0.0,
        data_bar_max: 100.0,
        data_bar_color: RgbaColor {
            r: 37,
            g: 99,
            b: 235,
            a: 255,
        },
        text_callout_field: String::new(),
        text_callout_prefix: String::new(),
        color_by_value_enabled: true,
        color_by_value_field: String::new(),
        color_palette: BTreeMap::new(),
    };

    let mut numeric_cols: Vec<(String, f64, f64)> = Vec::new();
    let mut status_col: Option<String> = None;
    let mut text_col: Option<String> = None;

    for col in &recordset.columns {
        if col.eq_ignore_ascii_case(&recordset.key_column) {
            continue;
        }
        let cl = col.to_ascii_lowercase();
        let mut nums = Vec::new();
        for r in &recordset.rows {
            if let Some(v) = r.get(col) {
                if let Some(n) = vexio_shapesheet::parse_prop_numeric_value(v) {
                    nums.push(n);
                }
            }
        }
        if !nums.is_empty() && nums.len() * 2 >= recordset.rows.len().max(1) {
            let min_v = nums.iter().copied().fold(f64::INFINITY, f64::min);
            let max_v = nums.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            numeric_cols.push((col.clone(), min_v, max_v));
            continue;
        }

        if status_col.is_none()
            && (cl.contains("status")
                || cl.contains("state")
                || cl.contains("health")
                || cl.contains("risk")
                || cl.contains("phase")
                || cl.contains("priority"))
        {
            status_col = Some(col.clone());
        } else if text_col.is_none() {
            text_col = Some(col.clone());
        }
    }

    if let Some((ncol, min_v, max_v)) = numeric_cols.first() {
        cfg.data_bar_field = ncol.clone();
        if *min_v >= 0.0 && *max_v <= 100.0 {
            cfg.data_bar_min = 0.0;
            cfg.data_bar_max = 100.0;
        } else {
            cfg.data_bar_min = min_v.min(0.0);
            cfg.data_bar_max = (*max_v * 1.1).max(1.0);
        }
    }

    if let Some(scol) = status_col {
        cfg.icon_set_field = scol.clone();
        cfg.color_by_value_field = scol.clone();
        if let Some(tcol) = text_col {
            cfg.text_callout_field = tcol;
        } else {
            cfg.text_callout_field = scol;
        }
    } else if let Some(tcol) = text_col {
        cfg.color_by_value_field = tcol.clone();
        cfg.text_callout_field = tcol;
        if let Some((ncol, min_v, max_v)) = numeric_cols.first() {
            cfg.icon_set_field = ncol.clone();
            let span = (*max_v - *min_v).max(1.0);
            cfg.icon_green_min = *min_v + span * 0.70;
            cfg.icon_amber_min = *min_v + span * 0.35;
        }
    } else if let Some((ncol, min_v, max_v)) = numeric_cols.first() {
        cfg.icon_set_field = ncol.clone();
        let span = (*max_v - *min_v).max(1.0);
        cfg.icon_green_min = *min_v + span * 0.70;
        cfg.icon_amber_min = *min_v + span * 0.35;
    }

    cfg
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
/// activating live `PageDataGraphicConfig` rendering (Icon Sets, Data Bars, Text Callouts, Color by Value).
pub fn apply_data_graphics(doc: &mut VisioDocument, page_id: u32, spec: &DataGraphicSpec) -> usize {
    ReactiveEngine::recalculate_document(doc);
    let mut cfg = PageDataGraphicConfig {
        enabled: true,
        icon_set_field: String::new(),
        icon_green_min: 75.0,
        icon_amber_min: 40.0,
        data_bar_field: String::new(),
        data_bar_min: 0.0,
        data_bar_max: 100.0,
        data_bar_color: RgbaColor {
            r: 37,
            g: 99,
            b: 235,
            a: 255,
        },
        text_callout_field: String::new(),
        text_callout_prefix: String::new(),
        color_by_value_enabled: false,
        color_by_value_field: String::new(),
        color_palette: BTreeMap::new(),
    };

    for item in &spec.items {
        match item {
            DataGraphicItem::ColorByValue { prop_name, palette } => {
                cfg.color_by_value_enabled = true;
                cfg.color_by_value_field = prop_name.clone();
                cfg.color_palette = palette.clone();
            }
            DataGraphicItem::TextCallout { prop_name, prefix } => {
                cfg.text_callout_field = prop_name.clone();
                cfg.text_callout_prefix = prefix.clone();
            }
            DataGraphicItem::DataBar {
                prop_name,
                min_val,
                max_val,
                bar_color,
            } => {
                cfg.data_bar_field = prop_name.clone();
                cfg.data_bar_min = *min_val;
                cfg.data_bar_max = *max_val;
                cfg.data_bar_color = *bar_color;
            }
            DataGraphicItem::IconSet {
                prop_name,
                green_min,
                amber_min,
            } => {
                cfg.icon_set_field = prop_name.clone();
                cfg.icon_green_min = *green_min;
                cfg.icon_amber_min = *amber_min;
            }
        }
    }

    let mut modified = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        page.data_graphic_config = Some(cfg.clone());
        for sh in &mut page.shapes {
            if !is_bindable_diagram_shape(sh) {
                continue;
            }
            sh.children
                .retain(|c| !c.name_u.starts_with("DG_SubShape"));
            let mut touched = false;
            for item in &spec.items {
                match item {
                    DataGraphicItem::ColorByValue { prop_name, palette } => {
                        if let Some((_, val_s)) =
                            resolve_shape_prop_cell(&sh.sheet, None, prop_name)
                        {
                            let col = vexio_shapesheet::resolve_categorical_data_color(
                                &val_s,
                                if palette.is_empty() {
                                    None
                                } else {
                                    Some(palette)
                                },
                            );
                            if !palette.is_empty() {
                                sh.sheet.set_formula(
                                    "FillForegnd",
                                    format!("=RGB({},{},{})", col.r, col.g, col.b),
                                    Some(Unit::Color),
                                );
                            }
                            touched = true;
                        }
                    }
                    DataGraphicItem::TextCallout { prop_name, .. }
                    | DataGraphicItem::IconSet { prop_name, .. } => {
                        if resolve_shape_prop_cell(&sh.sheet, None, prop_name).is_some() {
                            touched = true;
                        }
                    }
                    DataGraphicItem::DataBar { prop_name, .. } => {
                        if let Some((_, val_s)) =
                            resolve_shape_prop_cell(&sh.sheet, None, prop_name)
                        {
                            if vexio_shapesheet::parse_prop_numeric_value(&val_s).is_some() {
                                touched = true;
                            }
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

/// Insert (or update existing) live Page Legend summarizing Data Graphic categories, color swatches, and counts on `page_id`.
pub fn insert_page_legend(
    doc: &mut VisioDocument,
    page_id: u32,
    prop_name: &str,
    pin_x: f64,
    pin_y: f64,
) -> Option<u64> {
    let clean_prop = prop_name.trim().trim_matches('"');
    if clean_prop.is_empty() {
        return None;
    }
    let lid = {
        let page = doc.page_mut(page_id)?;
        if let Some(existing) = page.shapes.iter_mut().find(|sh| {
            sh.sheet
                .get_str("User.msvStructureType")
                .is_some_and(|s| s.trim().trim_matches('"').eq_ignore_ascii_case("Legend"))
        }) {
            existing.sheet.set_user_cell(
                "vexioLegendProp",
                format!("\"{}\"", clean_prop.replace('"', "")),
                "Live Legend Property",
            );
            existing.id
        } else {
            let new_id = page.next_shape_id();
            let mut legend = VisioShape::new_rect(new_id, "Legend", pin_x, pin_y, 2.35, 1.0);
            legend
                .sheet
                .set_user_cell("msvStructureType", "\"Legend\"", "Page Legend");
            legend.sheet.set_user_cell(
                "vexioLegendProp",
                format!("\"{}\"", clean_prop.replace('"', "")),
                "Live Legend Property",
            );
            page.shapes.push(legend);
            new_id
        }
    };
    ReactiveEngine::recalculate_document(doc);
    Some(lid)
}

/// Generate a BOM / Inventory Shape Report across all foreground pages of `doc`.
/// When `columns` is empty, automatically discovers every `Prop.*` column across the document!
pub fn generate_bom_shape_report(doc: &VisioDocument, columns: &[&str]) -> DataRecordset {
    let effective_cols: Vec<String> = if columns.is_empty() {
        let mut discovered = BTreeSet::new();
        for page in &doc.pages {
            if page.is_background {
                continue;
            }
            for sh in &page.shapes {
                if !is_bindable_diagram_shape(sh) {
                    continue;
                }
                for row in sh.sheet.shape_data.values() {
                    if !row.invisible {
                        let key = if row.label.trim().is_empty() {
                            row.name.clone()
                        } else {
                            row.label.clone()
                        };
                        discovered.insert(key);
                    }
                }
            }
        }
        discovered.into_iter().collect()
    } else {
        columns.iter().map(|&c| c.to_string()).collect()
    };

    let mut all_cols = vec![
        "ShapeID".to_string(),
        "Page".to_string(),
        "Name".to_string(),
        "Text".to_string(),
    ];
    for c in &effective_cols {
        if !all_cols.iter().any(|existing| existing.eq_ignore_ascii_case(c)) {
            all_cols.push(c.clone());
        }
    }

    let mut rows = Vec::new();
    for page in &doc.pages {
        if page.is_background {
            continue;
        }
        for sh in &page.shapes {
            if !is_bindable_diagram_shape(sh) {
                continue;
            }
            let ms_sheet = sh.master_id.and_then(|mid| {
                doc.masters
                    .iter()
                    .find(|m| m.id == mid)
                    .and_then(|m| m.shapes.first())
                    .map(|ms| &ms.sheet)
            });
            let mut r = BTreeMap::new();
            r.insert("ShapeID".into(), sh.id.to_string());
            r.insert("Page".into(), page.name.clone());
            r.insert("Name".into(), sh.name.clone());
            r.insert("Text".into(), sh.text.replace('\n', " "));
            for col in &effective_cols {
                let val = resolve_shape_prop_cell(&sh.sheet, ms_sheet, col)
                    .map(|(_, v)| v)
                    .or_else(|| {
                        sh.sheet
                            .get_cell(col)
                            .and_then(|c| c.value.as_str_coerced().ok())
                            .map(|s| s.trim_matches('"').to_string())
                    })
                    .unwrap_or_default();
                r.insert(col.clone(), val);
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

pub fn sanitize_prop_id(col: &str) -> String {
    sanitize_prop_identifier(col)
}

fn pick_best_key_column(columns: &[String], requested: &str) -> String {
    let req = requested.trim();
    if !req.is_empty() {
        if let Some(found) = columns.iter().find(|c| c.eq_ignore_ascii_case(req)) {
            return found.clone();
        }
    }
    for candidate in [
        "Label", "Name", "Title", "Host", "Hostname", "ID", "Key", "AssetId", "ServerID",
        "Component", "Step", "Process Step ID",
    ] {
        if let Some(found) = columns.iter().find(|c| c.eq_ignore_ascii_case(candidate)) {
            return found.clone();
        }
    }
    columns.first().cloned().unwrap_or_else(|| "ID".into())
}

fn detect_csv_delimiter(text: &str) -> char {
    let first_line = text.lines().next().unwrap_or("");
    let commas = first_line.chars().filter(|&c| c == ',').count();
    let semis = first_line.chars().filter(|&c| c == ';').count();
    let tabs = first_line.chars().filter(|&c| c == '\t').count();
    if tabs > commas && tabs >= semis {
        '\t'
    } else if semis > commas {
        ';'
    } else {
        ','
    }
}

fn parse_delimited_rows(csv: &str, delim: char) -> Vec<Vec<String>> {
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
                c if c == delim => {
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
                    } else if cell_type == "b" {
                        if cur_val.trim() == "1" {
                            "true".into()
                        } else {
                            "false".into()
                        }
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
