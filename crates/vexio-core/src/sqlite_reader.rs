//! Pure-Rust, Zero-C-Dependency SQLite 3 (`"SQLite format 3\0"`) Binary B-Tree Codec.
//!
//! Compiles 100% cleanly on `x86_64`, `aarch64`, and `wasm32-unknown-unknown` without `libsqlite3`:
//! - Big-endian 64-bit Huffman-style SQLite varint reader & writer (including 9-byte full 64-bit form)
//! - Complete SQLite record serial-type decoder & encoder (`0` NULL, `1..6` signed 8/16/24/32/48/64-bit
//!   big-endian integers, `7` IEEE-754 float64, `8` integer `0`, `9` integer `1`, `>=12` even BLOB,
//!   `>=13` odd UTF-8/UTF-16 TEXT)
//! - Page 1 (`100`-byte database header) + `sqlite_schema` B-tree traversal (`0x0D` table leaf &
//!   `0x05` table interior pages + chained overflow pages) + `CREATE TABLE` DDL parser
//! - Standards-compliant `4096`-byte page SQLite 3 database writer (`write_recordset_to_sqlite_bytes`)
//!   compatible with both `read_sqlite_table_to_recordset` and the official `sqlite3` CLI.

use crate::data_link::DataRecordset;
use std::collections::{BTreeMap, BTreeSet};
use vexio_shapesheet::format_clean_f64;

const SQLITE3_HEADER_MAGIC: &[u8; 16] = b"SQLite format 3\0";
const BTREE_TABLE_INTERIOR: u8 = 0x05;
const BTREE_TABLE_LEAF: u8 = 0x0D;
const DEFAULT_WRITE_PAGE_SIZE: usize = 4096;

// ============================================================================
// 1. SQLite Big-Endian 64-bit Huffman-Style Varint Reader & Writer
// ============================================================================

/// Reads a SQLite big-endian variable-length integer (`1..=9` bytes), returning `(value, bytes_consumed)`.
pub fn read_sqlite_varint(buf: &[u8]) -> Result<(u64, usize), String> {
    if buf.is_empty() {
        return Err("Unexpected EOF reading SQLite varint".into());
    }
    let mut acc = 0u64;
    for (idx, &b) in buf.iter().take(8).enumerate() {
        acc = (acc << 7) | ((b & 0x7F) as u64);
        if (b & 0x80) == 0 {
            return Ok((acc, idx + 1));
        }
    }
    if buf.len() < 9 {
        return Err("Truncated 9-byte SQLite varint".into());
    }
    // 9th byte contributes all 8 bits
    acc = (acc << 8) | (buf[8] as u64);
    Ok((acc, 9))
}

fn sqlite_varint_len(val: u64) -> usize {
    if val <= 0x7F {
        1
    } else if val <= 0x3FFF {
        2
    } else if val <= 0x001F_FFFF {
        3
    } else if val <= 0x0FFF_FFFF {
        4
    } else if val <= 0x07_FFFF_FFFF {
        5
    } else if val <= 0x03FF_FFFF_FFFF {
        6
    } else if val <= 0x0001_FFFF_FFFF_FFFF {
        7
    } else if val <= 0x00FF_FFFF_FFFF_FFFF {
        8
    } else {
        9
    }
}

/// Appends `val` encoded as a SQLite big-endian variable-length integer (`1..=9` bytes) to `out`.
pub fn write_sqlite_varint(out: &mut Vec<u8>, val: u64) {
    if val <= 0x7F {
        out.push(val as u8);
        return;
    }
    if val > 0x00FF_FFFF_FFFF_FFFF {
        // 9-byte representation: top 56 bits in 8 x 7-bit continuation bytes, bottom 8 bits in byte 9
        let hi56 = val >> 8;
        for shift in (0..8).rev() {
            let septet = ((hi56 >> (shift * 7)) & 0x7F) as u8;
            out.push(septet | 0x80);
        }
        out.push((val & 0xFF) as u8);
        return;
    }
    let n = sqlite_varint_len(val);
    for idx in (0..n).rev() {
        let septet = ((val >> (idx * 7)) & 0x7F) as u8;
        if idx > 0 {
            out.push(septet | 0x80);
        } else {
            out.push(septet);
        }
    }
}

// ============================================================================
// 2. Database Header, Record Serial Types & B-Tree Page Traversal
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SqliteTextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

#[derive(Debug, Clone, Copy)]
struct SqliteDbConfig {
    page_size: usize,
    usable_size: usize,
    text_encoding: SqliteTextEncoding,
}

#[derive(Debug, Clone, PartialEq)]
enum SqliteValue {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqliteValue {
    fn to_recordset_string(&self) -> String {
        match self {
            Self::Null => String::new(),
            Self::Int(i) => i.to_string(),
            Self::Float(f) => {
                if f.is_finite() {
                    format_clean_f64(*f)
                } else {
                    String::new()
                }
            }
            Self::Text(s) => s.clone(),
            Self::Blob(bytes) => {
                let mut hex = String::with_capacity(2 + bytes.len() * 2);
                hex.push_str("0x");
                for b in bytes {
                    hex.push_str(&format!("{b:02X}"));
                }
                hex
            }
        }
    }

    fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Text(s) => s.trim().parse::<i64>().ok(),
            _ => None,
        }
    }
}

fn parse_db_header(bytes: &[u8]) -> Result<SqliteDbConfig, String> {
    if bytes.len() < 100 {
        return Err("SQLite 3 file shorter than 100-byte database header".into());
    }
    if &bytes[0..16] != SQLITE3_HEADER_MAGIC {
        return Err("Invalid SQLite 3 file header magic (expected 'SQLite format 3\\0')".into());
    }
    let raw_ps = u16::from_be_bytes([bytes[16], bytes[17]]);
    let page_size = if raw_ps == 1 { 65_536 } else { raw_ps as usize };
    if !(512..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
        return Err(format!("Invalid SQLite page size {page_size}"));
    }
    let reserved = bytes[20] as usize;
    if reserved >= page_size || page_size - reserved < 480 {
        return Err(format!(
            "Invalid SQLite reserved space {reserved} for page size {page_size}"
        ));
    }
    let usable_size = page_size - reserved;
    let enc_code = u32::from_be_bytes([bytes[56], bytes[57], bytes[58], bytes[59]]);
    let text_encoding = match enc_code {
        2 => SqliteTextEncoding::Utf16Le,
        3 => SqliteTextEncoding::Utf16Be,
        _ => SqliteTextEncoding::Utf8,
    };
    Ok(SqliteDbConfig {
        page_size,
        usable_size,
        text_encoding,
    })
}

fn get_page_slice(db: &[u8], cfg: SqliteDbConfig, page_num: u32) -> Result<&[u8], String> {
    if page_num == 0 {
        return Err("Invalid SQLite page number 0 (pages are 1-indexed)".into());
    }
    let start = ((page_num as usize) - 1)
        .checked_mul(cfg.page_size)
        .ok_or_else(|| format!("Page offset overflow for page {page_num}"))?;
    let end = start + cfg.page_size;
    if end > db.len() {
        return Err(format!(
            "SQLite page {page_num} (bytes {start}..{end}) exceeds file length {}",
            db.len()
        ));
    }
    Ok(&db[start..start + cfg.usable_size])
}

fn assemble_leaf_cell_payload(
    db: &[u8],
    cfg: SqliteDbConfig,
    cell_slice: &[u8],
) -> Result<(i64, Vec<u8>), String> {
    let (payload_len_u64, n1) = read_sqlite_varint(cell_slice)?;
    let (rowid_u64, n2) = read_sqlite_varint(&cell_slice[n1..])?;
    let rowid = rowid_u64 as i64;
    let payload_len = payload_len_u64 as usize;
    let body_start = n1 + n2;

    let u = cfg.usable_size;
    let max_local = u - 35;
    if payload_len <= max_local {
        if body_start + payload_len > cell_slice.len() {
            return Err("Truncated local payload in SQLite table leaf cell".into());
        }
        return Ok((
            rowid,
            cell_slice[body_start..body_start + payload_len].to_vec(),
        ));
    }

    // Payload spills onto chained overflow pages (`[next_page: u32 BE, chunk...]`)
    let min_local = ((u - 12) * 32 / 255) - 23;
    let k = min_local + ((payload_len - min_local) % (u - 4));
    let local_len = if k <= max_local { k } else { min_local };
    if body_start + local_len + 4 > cell_slice.len() {
        return Err("Truncated overflow pointer in SQLite table leaf cell".into());
    }

    let mut full = Vec::with_capacity(payload_len);
    full.extend_from_slice(&cell_slice[body_start..body_start + local_len]);
    let ptr_pos = body_start + local_len;
    let mut ovf_page = u32::from_be_bytes([
        cell_slice[ptr_pos],
        cell_slice[ptr_pos + 1],
        cell_slice[ptr_pos + 2],
        cell_slice[ptr_pos + 3],
    ]);

    let mut visited_ovf = BTreeSet::new();
    while full.len() < payload_len && ovf_page != 0 {
        if !visited_ovf.insert(ovf_page) {
            return Err(format!("Cycle detected in SQLite overflow page {ovf_page}"));
        }
        let p_slice = get_page_slice(db, cfg, ovf_page)?;
        ovf_page = u32::from_be_bytes([p_slice[0], p_slice[1], p_slice[2], p_slice[3]]);
        let rem = payload_len - full.len();
        let take = rem.min(u - 4);
        full.extend_from_slice(&p_slice[4..4 + take]);
    }

    if full.len() < payload_len {
        return Err(format!(
            "Truncated SQLite overflow chain (got {} of {payload_len} bytes)",
            full.len()
        ));
    }
    Ok((rowid, full))
}

fn decode_serial_value(
    serial_type: u64,
    body: &[u8],
    pos: &mut usize,
    enc: SqliteTextEncoding,
) -> Result<SqliteValue, String> {
    match serial_type {
        0 | 10 | 11 => Ok(SqliteValue::Null),
        8 => Ok(SqliteValue::Int(0)),
        9 => Ok(SqliteValue::Int(1)),
        1 => {
            if *pos + 1 > body.len() {
                return Err("Truncated 8-bit int in SQLite record".into());
            }
            let v = body[*pos] as i8 as i64;
            *pos += 1;
            Ok(SqliteValue::Int(v))
        }
        2 => {
            if *pos + 2 > body.len() {
                return Err("Truncated 16-bit int in SQLite record".into());
            }
            let v = i16::from_be_bytes([body[*pos], body[*pos + 1]]) as i64;
            *pos += 2;
            Ok(SqliteValue::Int(v))
        }
        3 => {
            if *pos + 3 > body.len() {
                return Err("Truncated 24-bit int in SQLite record".into());
            }
            let b0 = body[*pos] as i8 as i64;
            let b1 = body[*pos + 1] as i64;
            let b2 = body[*pos + 2] as i64;
            *pos += 3;
            Ok(SqliteValue::Int((b0 << 16) | (b1 << 8) | b2))
        }
        4 => {
            if *pos + 4 > body.len() {
                return Err("Truncated 32-bit int in SQLite record".into());
            }
            let v = i32::from_be_bytes([
                body[*pos],
                body[*pos + 1],
                body[*pos + 2],
                body[*pos + 3],
            ]) as i64;
            *pos += 4;
            Ok(SqliteValue::Int(v))
        }
        5 => {
            if *pos + 6 > body.len() {
                return Err("Truncated 48-bit int in SQLite record".into());
            }
            let hi = (body[*pos] as i8 as i64) << 40;
            let rest = ((body[*pos + 1] as i64) << 32)
                | ((body[*pos + 2] as i64) << 24)
                | ((body[*pos + 3] as i64) << 16)
                | ((body[*pos + 4] as i64) << 8)
                | (body[*pos + 5] as i64);
            *pos += 6;
            Ok(SqliteValue::Int(hi | rest))
        }
        6 => {
            if *pos + 8 > body.len() {
                return Err("Truncated 64-bit int in SQLite record".into());
            }
            let v = i64::from_be_bytes(body[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Ok(SqliteValue::Int(v))
        }
        7 => {
            if *pos + 8 > body.len() {
                return Err("Truncated IEEE-754 float64 in SQLite record".into());
            }
            let f = f64::from_be_bytes(body[*pos..*pos + 8].try_into().unwrap());
            *pos += 8;
            Ok(SqliteValue::Float(f))
        }
        n if n >= 12 && n % 2 == 0 => {
            let len = ((n - 12) / 2) as usize;
            if *pos + len > body.len() {
                return Err("Truncated BLOB in SQLite record".into());
            }
            let b = body[*pos..*pos + len].to_vec();
            *pos += len;
            Ok(SqliteValue::Blob(b))
        }
        n => {
            // `n >= 13 && n % 2 == 1`: TEXT
            let len = ((n - 13) / 2) as usize;
            if *pos + len > body.len() {
                return Err("Truncated TEXT in SQLite record".into());
            }
            let raw = &body[*pos..*pos + len];
            *pos += len;
            let text = match enc {
                SqliteTextEncoding::Utf8 => String::from_utf8_lossy(raw).into_owned(),
                SqliteTextEncoding::Utf16Le => {
                    let units: Vec<u16> = raw
                        .chunks_exact(2)
                        .map(|c| u16::from_le_bytes([c[0], c[1]]))
                        .collect();
                    String::from_utf16_lossy(&units)
                }
                SqliteTextEncoding::Utf16Be => {
                    let units: Vec<u16> = raw
                        .chunks_exact(2)
                        .map(|c| u16::from_be_bytes([c[0], c[1]]))
                        .collect();
                    String::from_utf16_lossy(&units)
                }
            };
            Ok(SqliteValue::Text(text))
        }
    }
}

fn decode_sqlite_record(payload: &[u8], enc: SqliteTextEncoding) -> Result<Vec<SqliteValue>, String> {
    if payload.is_empty() {
        return Ok(Vec::new());
    }
    let (hdr_len_u64, mut hdr_pos) = read_sqlite_varint(payload)?;
    let hdr_len = hdr_len_u64 as usize;
    if hdr_len > payload.len() {
        return Err(format!(
            "SQLite record header size ({hdr_len}) exceeds payload length ({})",
            payload.len()
        ));
    }
    let mut serial_types = Vec::new();
    while hdr_pos < hdr_len {
        let (st, used) = read_sqlite_varint(&payload[hdr_pos..hdr_len])?;
        serial_types.push(st);
        hdr_pos += used;
    }

    let body = &payload[hdr_len..];
    let mut body_pos = 0usize;
    let mut values = Vec::with_capacity(serial_types.len());
    for st in serial_types {
        values.push(decode_serial_value(st, body, &mut body_pos, enc)?);
    }
    Ok(values)
}

fn scan_table_btree(
    db: &[u8],
    cfg: SqliteDbConfig,
    page_num: u32,
    visited: &mut BTreeSet<u32>,
    rows_out: &mut Vec<(i64, Vec<SqliteValue>)>,
) -> Result<(), String> {
    if !visited.insert(page_num) {
        return Err(format!("Cycle detected in SQLite B-tree at page {page_num}"));
    }
    let page = get_page_slice(db, cfg, page_num)?;
    let hdr_off = if page_num == 1 { 100usize } else { 0usize };
    if hdr_off + 8 > page.len() {
        return Err(format!("Truncated B-tree header on page {page_num}"));
    }

    let page_type = page[hdr_off];
    let num_cells = u16::from_be_bytes([page[hdr_off + 3], page[hdr_off + 4]]) as usize;

    match page_type {
        BTREE_TABLE_LEAF => {
            let ptr_array_start = hdr_off + 8;
            if ptr_array_start + num_cells * 2 > page.len() {
                return Err(format!(
                    "Cell pointer array overflows leaf page {page_num}"
                ));
            }
            for i in 0..num_cells {
                let p_off = ptr_array_start + i * 2;
                let cell_off = u16::from_be_bytes([page[p_off], page[p_off + 1]]) as usize;
                if cell_off >= page.len() {
                    return Err(format!(
                        "Cell offset {cell_off} out of bounds on leaf page {page_num}"
                    ));
                }
                let (rowid, rec_bytes) = assemble_leaf_cell_payload(db, cfg, &page[cell_off..])?;
                let cols = decode_sqlite_record(&rec_bytes, cfg.text_encoding)?;
                rows_out.push((rowid, cols));
            }
            Ok(())
        }
        BTREE_TABLE_INTERIOR => {
            if hdr_off + 12 > page.len() {
                return Err(format!(
                    "Truncated interior B-tree header on page {page_num}"
                ));
            }
            let right_child = u32::from_be_bytes([
                page[hdr_off + 8],
                page[hdr_off + 9],
                page[hdr_off + 10],
                page[hdr_off + 11],
            ]);
            let ptr_array_start = hdr_off + 12;
            if ptr_array_start + num_cells * 2 > page.len() {
                return Err(format!(
                    "Cell pointer array overflows interior page {page_num}"
                ));
            }
            for i in 0..num_cells {
                let p_off = ptr_array_start + i * 2;
                let cell_off = u16::from_be_bytes([page[p_off], page[p_off + 1]]) as usize;
                if cell_off + 4 > page.len() {
                    return Err(format!(
                        "Interior cell offset {cell_off} out of bounds on page {page_num}"
                    ));
                }
                let left_child = u32::from_be_bytes([
                    page[cell_off],
                    page[cell_off + 1],
                    page[cell_off + 2],
                    page[cell_off + 3],
                ]);
                // Validate key varint presence
                let _ = read_sqlite_varint(&page[cell_off + 4..])?;
                scan_table_btree(db, cfg, left_child, visited, rows_out)?;
            }
            scan_table_btree(db, cfg, right_child, visited, rows_out)
        }
        other => Err(format!(
            "Unsupported SQLite B-tree page type 0x{other:02X} on page {page_num}"
        )),
    }
}

// ============================================================================
// 3. `sqlite_schema` Reader & `CREATE TABLE` DDL Column Extractor
// ============================================================================

#[derive(Debug, Clone)]
struct SchemaTableEntry {
    name: String,
    root_page: u32,
    sql: String,
}

fn read_schema_tables(db: &[u8], cfg: SqliteDbConfig) -> Result<Vec<SchemaTableEntry>, String> {
    let mut visited = BTreeSet::new();
    let mut raw_rows = Vec::new();
    scan_table_btree(db, cfg, 1, &mut visited, &mut raw_rows)?;

    let mut tables = Vec::new();
    for (_rowid, cols) in raw_rows {
        if cols.len() < 5 {
            continue;
        }
        let obj_type = cols[0].to_recordset_string();
        let name = cols[1].to_recordset_string();
        let tbl_name = cols[2].to_recordset_string();
        let root_page = cols[3].as_i64().unwrap_or(0);
        let sql = cols[4].to_recordset_string();
        if obj_type.eq_ignore_ascii_case("table") && root_page > 0 {
            let effective_name = if !name.is_empty() { name } else { tbl_name };
            if !effective_name.starts_with("sqlite_") {
                tables.push(SchemaTableEntry {
                    name: effective_name,
                    root_page: root_page as u32,
                    sql,
                });
            }
        }
    }
    Ok(tables)
}

#[derive(Debug, Clone)]
struct ParsedColumnDef {
    name: String,
    is_integer_primary_key: bool,
}

fn unquote_sql_ident(token: &str) -> String {
    let t = token.trim();
    if t.len() >= 2 {
        if (t.starts_with('"') && t.ends_with('"'))
            || (t.starts_with('`') && t.ends_with('`'))
            || (t.starts_with('\'') && t.ends_with('\''))
        {
            let inner = &t[1..t.len() - 1];
            let q = t.chars().next().unwrap();
            let doubled = format!("{q}{q}");
            return inner.replace(&doubled, &q.to_string());
        }
        if t.starts_with('[') && t.ends_with(']') {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn split_top_level_commas(body: &str) -> Vec<&str> {
    let mut items = Vec::new();
    let mut depth = 0usize;
    let mut in_quote: Option<char> = None;
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = body.char_indices().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let (byte_idx, ch) = chars[i];
        if let Some(q) = in_quote {
            if ch == q {
                if i + 1 < chars.len() && chars[i + 1].1 == q {
                    i += 2;
                    continue;
                }
                in_quote = None;
            } else if q == '[' && ch == ']' {
                in_quote = None;
            }
            i += 1;
            continue;
        }

        match ch {
            '"' | '\'' | '`' => in_quote = Some(ch),
            '[' => in_quote = Some('['),
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                items.push(&body[start..byte_idx]);
                start = byte_idx + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start <= body.len() {
        items.push(&body[start..]);
    }
    items
}

fn extract_create_table_columns(sql: &str) -> Vec<ParsedColumnDef> {
    // Find outermost `(` .. `)` of `CREATE TABLE ... (...)` while respecting quoted identifiers
    let mut in_quote: Option<char> = None;
    let mut open_paren: Option<usize> = None;
    let mut close_paren: Option<usize> = None;
    let mut depth = 0usize;

    for (idx, ch) in sql.char_indices() {
        if let Some(q) = in_quote {
            if (q == '[' && ch == ']') || (q != '[' && ch == q) {
                in_quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => in_quote = Some(ch),
            '[' => in_quote = Some('['),
            '(' => {
                if depth == 0 && open_paren.is_none() {
                    open_paren = Some(idx);
                }
                depth += 1;
            }
            ')' => {
                if depth == 1 && open_paren.is_some() && close_paren.is_none() {
                    close_paren = Some(idx);
                    break;
                }
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    let (Some(open_idx), Some(close_idx)) = (open_paren, close_paren) else {
        return Vec::new();
    };
    let without_rowid = sql[close_idx..]
        .to_ascii_uppercase()
        .contains("WITHOUT ROWID");
    let body = &sql[open_idx + 1..close_idx];

    let mut cols = Vec::new();
    for raw_clause in split_top_level_commas(body) {
        let clause = raw_clause.trim();
        if clause.is_empty() {
            continue;
        }

        // Extract first token (quoted or unquoted identifier)
        let (ident_raw, rest) = if let Some(first_ch) = clause.chars().next() {
            if matches!(first_ch, '"' | '`' | '\'') {
                let mut end_pos = first_ch.len_utf8();
                let bytes = clause.as_bytes();
                let q = first_ch as u8;
                while end_pos < bytes.len() {
                    if bytes[end_pos] == q {
                        if end_pos + 1 < bytes.len() && bytes[end_pos + 1] == q {
                            end_pos += 2;
                            continue;
                        }
                        end_pos += 1;
                        break;
                    }
                    end_pos += 1;
                }
                (&clause[..end_pos], clause[end_pos..].trim())
            } else if first_ch == '[' {
                if let Some(close_b) = clause.find(']') {
                    (&clause[..=close_b], clause[close_b + 1..].trim())
                } else {
                    (clause, "")
                }
            } else {
                let split_pos = clause
                    .find(|c: char| c.is_whitespace() || c == '(')
                    .unwrap_or(clause.len());
                (&clause[..split_pos], clause[split_pos..].trim())
            }
        } else {
            continue;
        };

        // Skip table-level constraints (`PRIMARY KEY (...)`, `UNIQUE (...)`, `CHECK (...)`, `FOREIGN KEY`, `CONSTRAINT`)
        let unquoted_first_upper = ident_raw.to_ascii_uppercase();
        if !ident_raw.starts_with('"')
            && !ident_raw.starts_with('`')
            && !ident_raw.starts_with('[')
            && matches!(
                unquoted_first_upper.as_str(),
                "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK" | "CONSTRAINT"
            )
        {
            continue;
        }

        let col_name = unquote_sql_ident(ident_raw);
        if col_name.is_empty() {
            continue;
        }

        let rest_upper: String = rest
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_uppercase();
        let is_int_pk = !without_rowid
            && rest_upper.starts_with("INTEGER PRIMARY KEY")
            && !rest_upper.starts_with("INTEGER PRIMARY KEY DESC");

        cols.push(ParsedColumnDef {
            name: col_name,
            is_integer_primary_key: is_int_pk,
        });
    }
    cols
}

// ============================================================================
// 4. Public SQLite 3 Reader & Writer API
// ============================================================================

/// Lists all user table names stored in `sqlite_schema` of a SQLite 3 binary database buffer.
pub fn list_sqlite_tables(sqlite_bytes: &[u8]) -> Result<Vec<String>, String> {
    let cfg = parse_db_header(sqlite_bytes)?;
    let entries = read_schema_tables(sqlite_bytes, cfg)?;
    Ok(entries.into_iter().map(|e| e.name).collect())
}

/// Reads `table_name` from `sqlite_bytes` into a Visio `DataRecordset` keyed on `key_col`.
pub fn read_sqlite_table_to_recordset(
    id: u32,
    sqlite_bytes: &[u8],
    table_name: &str,
    key_col: &str,
) -> Result<DataRecordset, String> {
    let cfg = parse_db_header(sqlite_bytes)?;
    let schema = read_schema_tables(sqlite_bytes, cfg)?;
    let target = schema
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(table_name.trim()))
        .ok_or_else(|| format!("SQLite table '{}' not found in sqlite_schema", table_name))?;

    let mut col_defs = extract_create_table_columns(&target.sql);

    let mut visited = BTreeSet::new();
    let mut btree_rows = Vec::new();
    scan_table_btree(sqlite_bytes, cfg, target.root_page, &mut visited, &mut btree_rows)?;

    if col_defs.is_empty() {
        if let Some((_, first_vals)) = btree_rows.first() {
            col_defs = (0..first_vals.len())
                .map(|idx| ParsedColumnDef {
                    name: format!("Col{}", idx + 1),
                    is_integer_primary_key: false,
                })
                .collect();
        }
    }

    let columns: Vec<String> = col_defs.iter().map(|c| c.name.clone()).collect();
    let canonical_key_col = if columns.is_empty() {
        key_col.to_string()
    } else if key_col.trim().is_empty() {
        columns[0].clone()
    } else {
        columns
            .iter()
            .find(|c| c.eq_ignore_ascii_case(key_col.trim()))
            .cloned()
            .ok_or_else(|| {
                format!(
                    "Key column '{}' not found in SQLite table '{}' (columns: {:?})",
                    key_col, target.name, columns
                )
            })?
    };

    let mut rows = Vec::with_capacity(btree_rows.len());
    for (rowid, vals) in btree_rows {
        let mut map = BTreeMap::new();
        for (idx, cdef) in col_defs.iter().enumerate() {
            let cell_val = vals.get(idx).unwrap_or(&SqliteValue::Null);
            let formatted = if cdef.is_integer_primary_key && *cell_val == SqliteValue::Null {
                rowid.to_string()
            } else {
                cell_val.to_recordset_string()
            };
            map.insert(cdef.name.clone(), formatted);
        }
        rows.push(map);
    }

    Ok(DataRecordset {
        id,
        name: target.name.clone(),
        key_column: canonical_key_col,
        columns,
        rows,
    })
}

fn encode_string_cell_optimal(val: Option<&String>, serial_out: &mut Vec<u64>, body_out: &mut Vec<u8>) {
    let Some(s) = val else {
        serial_out.push(0);
        return;
    };
    if s == "0" {
        serial_out.push(8);
        return;
    }
    if s == "1" {
        serial_out.push(9);
        return;
    }
    if let Ok(i) = s.parse::<i64>() {
        if i.to_string() == *s {
            if (-128..=127).contains(&i) {
                serial_out.push(1);
                body_out.push(i as i8 as u8);
            } else if (-32_768..=32_767).contains(&i) {
                serial_out.push(2);
                body_out.extend_from_slice(&(i as i16).to_be_bytes());
            } else if (-8_388_608..=8_388_607).contains(&i) {
                serial_out.push(3);
                body_out.extend_from_slice(&i.to_be_bytes()[5..8]);
            } else if (-2_147_483_648..=2_147_483_647).contains(&i) {
                serial_out.push(4);
                body_out.extend_from_slice(&(i as i32).to_be_bytes());
            } else if (-140_737_488_355_328..=140_737_488_355_327).contains(&i) {
                serial_out.push(5);
                body_out.extend_from_slice(&i.to_be_bytes()[2..8]);
            } else {
                serial_out.push(6);
                body_out.extend_from_slice(&i.to_be_bytes());
            }
            return;
        }
    }
    if let Ok(f) = s.parse::<f64>() {
        if f.is_finite() && format_clean_f64(f) == *s {
            serial_out.push(7);
            body_out.extend_from_slice(&f.to_be_bytes());
            return;
        }
    }
    let bytes = s.as_bytes();
    serial_out.push(13 + 2 * (bytes.len() as u64));
    body_out.extend_from_slice(bytes);
}

fn build_sqlite_record_payload(serials: &[u64], body: &[u8]) -> Vec<u8> {
    let mut serial_bytes = Vec::with_capacity(serials.len() * 2);
    for &st in serials {
        write_sqlite_varint(&mut serial_bytes, st);
    }
    let s_len = serial_bytes.len();
    let mut hdr_size = s_len + 1;
    while sqlite_varint_len(hdr_size as u64) + s_len != hdr_size {
        hdr_size = s_len + sqlite_varint_len(hdr_size as u64);
    }

    let mut rec = Vec::with_capacity(hdr_size + body.len());
    write_sqlite_varint(&mut rec, hdr_size as u64);
    rec.extend_from_slice(&serial_bytes);
    rec.extend_from_slice(body);
    rec
}

fn build_leaf_cell_with_overflow(
    rowid: u64,
    payload: &[u8],
    next_page_num: &mut u32,
    overflow_pages: &mut Vec<(u32, [u8; DEFAULT_WRITE_PAGE_SIZE])>,
) -> Vec<u8> {
    let u = DEFAULT_WRITE_PAGE_SIZE;
    let max_local = u - 35;
    let mut cell = Vec::new();
    write_sqlite_varint(&mut cell, payload.len() as u64);
    write_sqlite_varint(&mut cell, rowid);

    if payload.len() <= max_local {
        cell.extend_from_slice(payload);
        return cell;
    }

    let min_local = ((u - 12) * 32 / 255) - 23;
    let k = min_local + ((payload.len() - min_local) % (u - 4));
    let local_len = if k <= max_local { k } else { min_local };
    cell.extend_from_slice(&payload[..local_len]);

    let rem = &payload[local_len..];
    let chunks: Vec<&[u8]> = rem.chunks(u - 4).collect();
    let first_ovf = *next_page_num;
    *next_page_num += chunks.len() as u32;
    cell.extend_from_slice(&first_ovf.to_be_bytes());

    for (idx, chunk) in chunks.iter().enumerate() {
        let pno = first_ovf + (idx as u32);
        let next_pno = if idx + 1 < chunks.len() { pno + 1 } else { 0 };
        let mut page = [0u8; DEFAULT_WRITE_PAGE_SIZE];
        page[0..4].copy_from_slice(&next_pno.to_be_bytes());
        page[4..4 + chunk.len()].copy_from_slice(chunk);
        overflow_pages.push((pno, page));
    }
    cell
}

fn pack_table_leaf_page(cells: &[Vec<u8>], hdr_offset: usize) -> [u8; DEFAULT_WRITE_PAGE_SIZE] {
    let mut page = [0u8; DEFAULT_WRITE_PAGE_SIZE];
    let mut cursor = DEFAULT_WRITE_PAGE_SIZE;
    let mut ptrs = Vec::with_capacity(cells.len());

    for cell in cells {
        let slot_len = cell.len().max(4);
        cursor -= slot_len;
        page[cursor..cursor + cell.len()].copy_from_slice(cell);
        ptrs.push(cursor as u16);
    }

    page[hdr_offset] = BTREE_TABLE_LEAF;
    page[hdr_offset + 1..hdr_offset + 3].copy_from_slice(&0u16.to_be_bytes());
    page[hdr_offset + 3..hdr_offset + 5].copy_from_slice(&(cells.len() as u16).to_be_bytes());
    let content_off = if cursor == 65_536 { 0u16 } else { cursor as u16 };
    page[hdr_offset + 5..hdr_offset + 7].copy_from_slice(&content_off.to_be_bytes());
    page[hdr_offset + 7] = 0;

    let ptr_base = hdr_offset + 8;
    for (i, ptr) in ptrs.into_iter().enumerate() {
        let p = ptr_base + i * 2;
        page[p..p + 2].copy_from_slice(&ptr.to_be_bytes());
    }
    page
}

fn quote_sql_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// Serializes `rs` into a valid, standards-compliant `4096`-byte page SQLite 3 binary database file
/// (`"SQLite format 3\0"`, Page 1 `sqlite_schema` pointing to Page 2 table B-tree) readable by both
/// `read_sqlite_table_to_recordset` and the `sqlite3` CLI.
pub fn write_recordset_to_sqlite_bytes(rs: &DataRecordset, table_name: &str) -> Vec<u8> {
    let clean_table = if table_name.trim().is_empty() {
        if rs.name.trim().is_empty() {
            "Sheet1"
        } else {
            rs.name.trim()
        }
    } else {
        table_name.trim()
    };

    let col_defs_sql = if rs.columns.is_empty() {
        "\"Value\" TEXT".to_string()
    } else {
        rs.columns
            .iter()
            .map(|c| format!("{} TEXT", quote_sql_ident(c)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let create_sql = format!(
        "CREATE TABLE {} ({})",
        quote_sql_ident(clean_table),
        col_defs_sql
    );

    // Reserve Page 1 (`sqlite_schema`) and Page 2 (root of user table B-tree)
    let mut next_free_page = 3u32;
    let mut extra_pages: BTreeMap<u32, [u8; DEFAULT_WRITE_PAGE_SIZE]> = BTreeMap::new();

    let mut encoded_cells: Vec<(u64, Vec<u8>)> = Vec::with_capacity(rs.rows.len());
    for (idx, row) in rs.rows.iter().enumerate() {
        let rowid = (idx as u64) + 1;
        let mut serials = Vec::with_capacity(rs.columns.len().max(1));
        let mut body = Vec::new();
        if rs.columns.is_empty() {
            serials.push(0);
        } else {
            for col in &rs.columns {
                encode_string_cell_optimal(row.get(col), &mut serials, &mut body);
            }
        }
        let payload = build_sqlite_record_payload(&serials, &body);
        let mut ovf = Vec::new();
        let cell = build_leaf_cell_with_overflow(rowid, &payload, &mut next_free_page, &mut ovf);
        for (pno, pbuf) in ovf {
            extra_pages.insert(pno, pbuf);
        }
        encoded_cells.push((rowid, cell));
    }

    // Determine whether all cells fit directly into Page 2 as a single `0x0D` Table Leaf Page
    let single_leaf_bytes: usize = 8
        + encoded_cells.len() * 2
        + encoded_cells
            .iter()
            .map(|(_, c)| c.len().max(4))
            .sum::<usize>();

    let page2_buf = if single_leaf_bytes <= DEFAULT_WRITE_PAGE_SIZE {
        let only_cells: Vec<Vec<u8>> = encoded_cells.into_iter().map(|(_, c)| c).collect();
        pack_table_leaf_page(&only_cells, 0)
    } else {
        // Partition cells across leaf pages (`next_free_page..`) and make Page 2 a `0x05` Interior Root!
        let mut leaf_groups: Vec<(u32, u64, Vec<Vec<u8>>)> = Vec::new();
        let mut cur_group: Vec<Vec<u8>> = Vec::new();
        let mut cur_used = 8usize;
        let mut cur_max_rowid = 1u64;

        for (rowid, cell) in encoded_cells {
            let needed = 2 + cell.len().max(4);
            if !cur_group.is_empty() && cur_used + needed > DEFAULT_WRITE_PAGE_SIZE {
                let leaf_pno = next_free_page;
                next_free_page += 1;
                leaf_groups.push((leaf_pno, cur_max_rowid, std::mem::take(&mut cur_group)));
                cur_used = 8;
            }
            cur_used += needed;
            cur_max_rowid = rowid;
            cur_group.push(cell);
        }
        if !cur_group.is_empty() {
            let leaf_pno = next_free_page;
            next_free_page += 1;
            leaf_groups.push((leaf_pno, cur_max_rowid, cur_group));
        }

        for (pno, _, cells) in &leaf_groups {
            extra_pages.insert(*pno, pack_table_leaf_page(cells, 0));
        }

        let mut interior = [0u8; DEFAULT_WRITE_PAGE_SIZE];
        let right_child = leaf_groups.last().map_or(2, |(pno, _, _)| *pno);
        let interior_children = &leaf_groups[..leaf_groups.len().saturating_sub(1)];
        let mut cursor = DEFAULT_WRITE_PAGE_SIZE;
        let mut ptrs = Vec::with_capacity(interior_children.len());

        for &(left_pno, max_key, _) in interior_children {
            let mut icell = Vec::with_capacity(12);
            icell.extend_from_slice(&left_pno.to_be_bytes());
            write_sqlite_varint(&mut icell, max_key);
            let slot_len = icell.len().max(4);
            cursor -= slot_len;
            interior[cursor..cursor + icell.len()].copy_from_slice(&icell);
            ptrs.push(cursor as u16);
        }

        interior[0] = BTREE_TABLE_INTERIOR;
        interior[3..5].copy_from_slice(&(interior_children.len() as u16).to_be_bytes());
        interior[5..7].copy_from_slice(&(cursor as u16).to_be_bytes());
        interior[8..12].copy_from_slice(&right_child.to_be_bytes());
        for (i, ptr) in ptrs.into_iter().enumerate() {
            let p = 12 + i * 2;
            interior[p..p + 2].copy_from_slice(&ptr.to_be_bytes());
        }
        interior
    };

    let total_pages = (next_free_page - 1).max(2);

    // Build Page 1: 100-byte Database Header + `sqlite_schema` leaf B-tree (`hdr_offset = 100`)
    let schema_serials = [
        13 + 2 * 5u64,                        // "table"
        13 + 2 * (clean_table.len() as u64),  // name
        13 + 2 * (clean_table.len() as u64),  // tbl_name
        1u64,                                 // rootpage = 2 (8-bit signed int)
        13 + 2 * (create_sql.len() as u64),   // sql
    ];
    let mut schema_body =
        Vec::with_capacity(5 + clean_table.len() * 2 + 1 + create_sql.len());
    schema_body.extend_from_slice(b"table");
    schema_body.extend_from_slice(clean_table.as_bytes());
    schema_body.extend_from_slice(clean_table.as_bytes());
    schema_body.push(2u8); // rootpage = 2
    schema_body.extend_from_slice(create_sql.as_bytes());

    let schema_rec = build_sqlite_record_payload(&schema_serials, &schema_body);
    let mut schema_cell = Vec::with_capacity(4 + schema_rec.len());
    write_sqlite_varint(&mut schema_cell, schema_rec.len() as u64);
    write_sqlite_varint(&mut schema_cell, 1); // rowid = 1
    schema_cell.extend_from_slice(&schema_rec);

    let mut page1 = pack_table_leaf_page(&[schema_cell], 100);
    page1[0..16].copy_from_slice(SQLITE3_HEADER_MAGIC);
    page1[16..18].copy_from_slice(&(DEFAULT_WRITE_PAGE_SIZE as u16).to_be_bytes());
    page1[18] = 1; // write version
    page1[19] = 1; // read version
    page1[20] = 0; // reserved space
    page1[21] = 64; // max embedded payload fraction
    page1[22] = 32; // min embedded payload fraction
    page1[23] = 32; // leaf payload fraction
    page1[24..28].copy_from_slice(&1u32.to_be_bytes()); // file change counter
    page1[28..32].copy_from_slice(&total_pages.to_be_bytes()); // database size in pages
    page1[40..44].copy_from_slice(&1u32.to_be_bytes()); // schema cookie
    page1[44..48].copy_from_slice(&4u32.to_be_bytes()); // schema format number = 4
    page1[56..60].copy_from_slice(&1u32.to_be_bytes()); // UTF-8 encoding
    page1[92..96].copy_from_slice(&1u32.to_be_bytes()); // version-valid-for
    page1[96..100].copy_from_slice(&3_045_000u32.to_be_bytes()); // SQLite 3.45.0

    let mut db_bytes = Vec::with_capacity((total_pages as usize) * DEFAULT_WRITE_PAGE_SIZE);
    db_bytes.extend_from_slice(&page1);
    db_bytes.extend_from_slice(&page2_buf);
    for pno in 3..=total_pages {
        if let Some(p) = extra_pages.get(&pno) {
            db_bytes.extend_from_slice(p);
        } else {
            db_bytes.extend_from_slice(&[0u8; DEFAULT_WRITE_PAGE_SIZE]);
        }
    }
    db_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlite_varint_huffman_64bit_roundtrip() {
        for &val in &[
            0u64,
            1,
            127,
            128,
            16_383,
            16_384,
            2_097_151,
            268_435_455,
            0x00FF_FFFF_FFFF_FFFF,
            0x0100_0000_0000_0000,
            u64::MAX,
            (-1i64) as u64,
            (-987_654_321i64) as u64,
        ] {
            let mut buf = Vec::new();
            write_sqlite_varint(&mut buf, val);
            let (decoded, used) = read_sqlite_varint(&buf).unwrap();
            assert_eq!(used, buf.len());
            assert_eq!(decoded, val);
        }
    }

    #[test]
    fn recordset_to_sqlite3_binary_and_multitype_roundtrip() {
        let csv = "\
ServerID,Hostname,ActiveFlag,DisabledFlag,RackU,PowerKw,SerialCode,MemoryBytes,Notes
1,core-sw-01,1,0,42,3.75,00742,68719476736,Primary Spine
2,edge-fw-02,0,1,-12,1.25,00991,214748364800,DMZ Pair
3,db-node-03,1,0,1024,12.5,ABC-99,9007199254740992,UTF-8 Cluster — αβγ
";
        let orig_rs =
            DataRecordset::from_csv(7, "DatacenterInventory", "ServerID", csv).unwrap();

        let sqlite_bytes = write_recordset_to_sqlite_bytes(&orig_rs, "datacenter_nodes");
        assert_eq!(&sqlite_bytes[0..16], b"SQLite format 3\0");
        assert_eq!(sqlite_bytes.len() % 4096, 0);

        let tables = list_sqlite_tables(&sqlite_bytes).unwrap();
        assert_eq!(tables, vec!["datacenter_nodes".to_string()]);

        let restored = read_sqlite_table_to_recordset(
            7,
            &sqlite_bytes,
            "datacenter_nodes",
            "ServerID",
        )
        .unwrap();
        assert_eq!(restored.columns, orig_rs.columns);
        assert_eq!(restored.rows, orig_rs.rows);
    }

    #[test]
    fn multi_page_interior_and_leaf_btree_roundtrip() {
        let columns = vec![
            "AssetKey".to_string(),
            "Count24Bit".to_string(),
            "Payload".to_string(),
        ];
        let mut rows = Vec::new();
        // 150 rows x ~80 bytes > 4096 bytes -> exercises 0x05 Table Interior Root + multiple 0x0D Leaf Pages!
        for i in 1..=150 {
            let mut r = BTreeMap::new();
            r.insert("AssetKey".into(), format!("ASSET-{i:04}"));
            r.insert("Count24Bit".into(), (100_000 + i * 37).to_string());
            r.insert(
                "Payload".into(),
                format!("Telemetry description payload for industrial sensor node #{i}"),
            );
            rows.push(r);
        }
        let big_rs = DataRecordset {
            id: 42,
            name: "PlantTelemetry".into(),
            key_column: "AssetKey".into(),
            columns,
            rows,
        };

        let db = write_recordset_to_sqlite_bytes(&big_rs, "plant_telemetry");
        assert!(
            db.len() >= 4096 * 4,
            "Expected multi-page SQLite B-tree, got {} bytes",
            db.len()
        );
        // Page 2 byte 0 (offset 4096) MUST be 0x05 (Table Interior B-Tree Root)!
        assert_eq!(db[4096], 0x05);

        let decoded =
            read_sqlite_table_to_recordset(42, &db, "plant_telemetry", "AssetKey").unwrap();
        assert_eq!(decoded.rows.len(), 150);
        assert_eq!(decoded.rows[0], big_rs.rows[0]);
        assert_eq!(decoded.rows[149], big_rs.rows[149]);
    }

    #[test]
    fn sqlite_blob_64bit_int_null_and_integer_pk_rowid_alias() {
        // Test record decoder directly on all remaining serial types: 0 (NULL), 6 (i64), 16 (2-byte BLOB)
        let mut body = Vec::new();
        let big_i64: i64 = -576_460_752_303_423_488;
        body.extend_from_slice(&big_i64.to_be_bytes());
        body.extend_from_slice(&[0xDE, 0xAD]);
        let payload = build_sqlite_record_payload(&[0, 6, 16], &body);
        let vals = decode_sqlite_record(&payload, SqliteTextEncoding::Utf8).unwrap();
        assert_eq!(vals[0], SqliteValue::Null);
        assert_eq!(vals[1], SqliteValue::Int(big_i64));
        assert_eq!(vals[2].to_recordset_string(), "0xDEAD");

        // Verify CREATE TABLE parser recognizes `INTEGER PRIMARY KEY` rowid alias & skips table constraints
        let cols = extract_create_table_columns(
            "CREATE TABLE \"devices\" (id INTEGER PRIMARY KEY, \"Host Name\" VARCHAR(64) NOT NULL, CONSTRAINT uq_host UNIQUE (\"Host Name\"))",
        );
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].name, "id");
        assert!(cols[0].is_integer_primary_key);
        assert_eq!(cols[1].name, "Host Name");
        assert!(!cols[1].is_integer_primary_key);
    }
}
