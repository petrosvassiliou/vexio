//! Enterprise Visio Pro Domain Engines & Built-in IP-Clean Stencil Factory.
//!
//! Covers 100% of Microsoft Visio Plan 2 / Professional domains:
//! - Flowcharts & Cross-Functional Swimlanes
//! - BPMN 2.0 & UML 2.5
//! - Org Charts (with hierarchical table importer)
//! - Network & 42U Rack Diagrams (1U = 1.75" slot snapping)
//! - Database / ERD (Crow's Foot & IDEF1X + SQL DDL Reverse & Forward Engineering)
//! - Electrical / P&ID & Floor Plans (with reactive Architectural Area calculation)
//! - Project Management: PERT Critical Path Method (CPM), Gantt, Timelines & Value Stream Mapping (VSM)
//! - UI Wireframes

use crate::autolayout::{LayoutAlgorithm, LayoutOptions, apply_auto_layout};
use crate::routing::route_page_connectors;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vexio_shapesheet::{
    ConnectionPointRow, GeometryRow, GeometrySection, GlueEndpoint, ReactiveEngine, ShapeDataRow,
    ShapeDataType, ShapeSheet, StencilPack, Unit, VisioDocument, VisioMaster, VisioShape,
    format_clean_f64,
};

// ============================================================================
// 1. Database / ERD Reverse & Forward SQL Schema Engineering
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErdColumn {
    pub name: String,
    pub sql_type: String,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub references_table: Option<(String, String)>, // (target_table, target_column)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErdTable {
    pub name: String,
    pub columns: Vec<ErdColumn>,
}

/// Reverse-engineer ANSI/PostgreSQL/SQLite/SQL Server `CREATE TABLE` DDL into an ERD diagram page
/// using Crow's Foot (`EndArrow = 27`) or IDEF1X notation!
pub fn reverse_engineer_sql_ddl(doc: &mut VisioDocument, page_id: u32, sql_ddl: &str, crows_foot: bool) -> Vec<ErdTable> {
    let tables = parse_sql_ddl(sql_ddl);
    let mut table_shape_ids: BTreeMap<String, u64> = BTreeMap::new();

    if let Some(page) = doc.page_mut(page_id) {
        for (idx, tbl) in tables.iter().enumerate() {
            let sid = page.next_shape_id();
            let row_h = 0.25 * (tbl.columns.len() as f64 + 1.5);
            let mut sh = VisioShape::new_rect(
                sid,
                tbl.name.clone(),
                2.5 + (idx % 3) as f64 * 3.2,
                7.0 - (idx / 3) as f64 * 2.8,
                2.4,
                row_h.max(1.0),
            );
            let mut label_lines = vec![format!("[{}]", tbl.name)];
            let mut encoded_cols = Vec::new();
            for col in &tbl.columns {
                let pk_tag = if col.is_primary_key { "PK " } else if col.references_table.is_some() { "FK " } else { "   " };
                let null_tag = if col.is_nullable { "NULL" } else { "NOT NULL" };
                label_lines.push(format!("{pk_tag}{} : {} ({null_tag})", col.name, col.sql_type));
                encoded_cols.push(format!(
                    "{}:{}:{}:{}:{}",
                    col.name,
                    col.sql_type,
                    if col.is_primary_key { "1" } else { "0" },
                    if col.is_nullable { "1" } else { "0" },
                    col.references_table
                        .as_ref()
                        .map(|(t, c)| format!("{t}.{c}"))
                        .unwrap_or_default()
                ));
            }
            sh.text = label_lines.join("\n");
            sh.sheet
                .set_user_cell("DomainRole", "\"ErdEntity\"", "ERD Entity");
            sh.sheet.set_user_cell(
                "ErdColumns",
                format!("\"{}\"", encoded_cols.join(";")),
                "Serialized ERD Columns",
            );
            sh.sheet
                .set_formula("FillForegnd", "=RGB(239,246,255)", Some(Unit::Color));
            page.shapes.push(sh);
            table_shape_ids.insert(tbl.name.to_ascii_lowercase(), sid);
        }

        for tbl in &tables {
            let Some(&child_sid) = table_shape_ids.get(&tbl.name.to_ascii_lowercase()) else {
                continue;
            };
            for col in &tbl.columns {
                if let Some((ref parent_tbl, ref parent_col)) = col.references_table {
                    if let Some(&parent_sid) = table_shape_ids.get(&parent_tbl.to_ascii_lowercase()) {
                        let cid = page.next_shape_id();
                        let mut conn = VisioShape::new_connector(
                            cid,
                            format!("FK_{}_{}", tbl.name, col.name),
                            1.0,
                            1.0,
                            2.0,
                            2.0,
                        );
                        conn.text = format!("{} -> {}", col.name, parent_col);
                        conn.sheet.set_formula(
                            "EndArrow",
                            if crows_foot { "27" } else { "5" },
                            Some(Unit::Number),
                        );
                        conn.sheet.set_user_cell(
                            "ErdFkSpec",
                            format!("\"{}.{}>{}.{}\"", tbl.name, col.name, parent_tbl, parent_col),
                            "Foreign Key Spec",
                        );
                        page.shapes.push(conn);
                        page.connects.push(GlueEndpoint {
                            connector_shape_id: cid,
                            from_cell: "BeginX".into(),
                            target_shape_id: parent_sid,
                            to_cell: "PinX".into(),
                            is_dynamic: true,
                        });
                        page.connects.push(GlueEndpoint {
                            connector_shape_id: cid,
                            from_cell: "EndX".into(),
                            target_shape_id: child_sid,
                            to_cell: "PinX".into(),
                            is_dynamic: true,
                        });
                    }
                }
            }
        }
    }

    route_page_connectors(doc, page_id);
    tables
}

/// Forward-engineer an ERD page in `VisioDocument` back into executable SQL `CREATE TABLE` DDL!
pub fn forward_engineer_sql_ddl(doc: &VisioDocument, page_id: u32) -> String {
    let Some(page) = doc.page(page_id) else {
        return String::new();
    };
    let mut ddl = String::new();
    for sh in &page.shapes {
        let Some(cols_cell) = sh.sheet.get_cell("User.ErdColumns") else {
            continue;
        };
        let Ok(raw) = cols_cell.value.as_str_coerced() else {
            continue;
        };
        let raw = raw.trim_matches('"');
        ddl.push_str(&format!("CREATE TABLE {} (\n", sh.name));
        let mut lines = Vec::new();
        let mut pks = Vec::new();
        let mut fks = Vec::new();
        for item in raw.split(';').filter(|s| !s.is_empty()) {
            let parts: Vec<&str> = item.split(':').collect();
            if parts.len() >= 5 {
                let cname = parts[0];
                let ctype = parts[1];
                let is_pk = parts[2] == "1";
                let nullable = parts[3] == "1";
                let fk_ref = parts[4];
                lines.push(format!(
                    "    {} {}{}",
                    cname,
                    ctype,
                    if nullable { "" } else { " NOT NULL" }
                ));
                if is_pk {
                    pks.push(cname.to_string());
                }
                if let Some((rt, rc)) = fk_ref.split_once('.') {
                    fks.push(format!("    FOREIGN KEY ({cname}) REFERENCES {rt}({rc})"));
                }
            }
        }
        if !pks.is_empty() {
            lines.push(format!("    PRIMARY KEY ({})", pks.join(", ")));
        }
        lines.extend(fks);
        ddl.push_str(&lines.join(",\n"));
        ddl.push_str("\n);\n\n");
    }
    ddl
}

fn parse_sql_ddl(sql: &str) -> Vec<ErdTable> {
    let mut tables = Vec::new();
    let mut rest = sql;
    while let Some(pos) = rest.to_ascii_uppercase().find("CREATE TABLE") {
        rest = &rest[pos + "CREATE TABLE".len()..];
        let Some(open_p) = rest.find('(') else {
            break;
        };
        let tbl_header = rest[..open_p]
            .trim()
            .trim_start_matches("IF NOT EXISTS")
            .trim()
            .trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']');
        let tbl_name = tbl_header
            .split('.')
            .next_back()
            .unwrap_or(tbl_header)
            .trim()
            .to_string();

        // Find matching ')'
        let mut depth = 0usize;
        let mut close_idx = None;
        for (i, ch) in rest[open_p..].char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        close_idx = Some(open_p + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end_p) = close_idx else {
            break;
        };
        let body = &rest[open_p + 1..end_p];
        rest = &rest[end_p + 1..];

        let mut columns: Vec<ErdColumn> = Vec::new();
        for raw_item in split_top_level_commas(body) {
            let line = raw_item.trim();
            if line.is_empty() {
                continue;
            }
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("PRIMARY KEY") {
                if let (Some(a), Some(b)) = (line.find('('), line.find(')')) {
                    for pk_col in line[a + 1..b].split(',') {
                        let pk_clean = pk_col.trim().trim_matches('"');
                        if let Some(c) = columns.iter_mut().find(|c| c.name.eq_ignore_ascii_case(pk_clean)) {
                            c.is_primary_key = true;
                            c.is_nullable = false;
                        }
                    }
                }
                continue;
            }
            if upper.starts_with("FOREIGN KEY") || upper.starts_with("CONSTRAINT") {
                if let Some(fk_idx) = upper.find("FOREIGN KEY") {
                    let after_fk = &line[fk_idx + "FOREIGN KEY".len()..];
                    if let (Some(a), Some(b), Some(ref_kw)) = (
                        after_fk.find('('),
                        after_fk.find(')'),
                        after_fk.to_ascii_uppercase().find("REFERENCES"),
                    ) {
                        let local_col = after_fk[a + 1..b].trim().trim_matches('"');
                        let target_spec = after_fk[ref_kw + "REFERENCES".len()..].trim();
                        if let (Some(ta), Some(tb)) = (target_spec.find('('), target_spec.find(')')) {
                            let target_tbl = target_spec[..ta].trim().trim_matches('"').to_string();
                            let target_col = target_spec[ta + 1..tb].trim().trim_matches('"').to_string();
                            if let Some(c) = columns
                                .iter_mut()
                                .find(|c| c.name.eq_ignore_ascii_case(local_col))
                            {
                                c.references_table = Some((target_tbl, target_col));
                            }
                        }
                    }
                }
                continue;
            }

            let tokens: Vec<&str> = line.split_whitespace().collect();
            if tokens.len() >= 2 {
                let col_name = tokens[0].trim_matches(|c| c == '"' || c == '`' || c == '[' || c == ']').to_string();
                let sql_type = tokens[1].to_string();
                let is_pk = upper.contains("PRIMARY KEY");
                let is_nullable = !is_pk && !upper.contains("NOT NULL");
                let mut references_table = None;
                if let Some(rpos) = upper.find("REFERENCES") {
                    let target_spec = line[rpos + "REFERENCES".len()..].trim();
                    if let (Some(ta), Some(tb)) = (target_spec.find('('), target_spec.find(')')) {
                        references_table = Some((
                            target_spec[..ta].trim().trim_matches('"').to_string(),
                            target_spec[ta + 1..tb].trim().trim_matches('"').to_string(),
                        ));
                    }
                }
                columns.push(ErdColumn {
                    name: col_name,
                    sql_type,
                    is_primary_key: is_pk,
                    is_nullable,
                    references_table,
                });
            }
        }
        tables.push(ErdTable {
            name: tbl_name,
            columns,
        });
    }
    tables
}

fn split_top_level_commas(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

// ============================================================================
// 2. PERT Critical Path Method (CPM), Gantt, Org Chart, Rack & Floor Plan
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PertTaskInput {
    pub id: String,
    pub name: String,
    pub duration_days: f64,
    pub predecessors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PertComputedNode {
    pub id: String,
    pub shape_id: u64,
    pub early_start: f64,
    pub early_finish: f64,
    pub late_start: f64,
    pub late_finish: f64,
    pub slack: f64,
    pub is_critical: bool,
}

/// Compute PERT Critical Path Method (forward + backward pass) and populate a Visio PERT chart page,
/// highlighting Critical Path nodes and connectors automatically!
pub fn build_pert_critical_path_diagram(
    doc: &mut VisioDocument,
    page_id: u32,
    tasks: &[PertTaskInput],
) -> Vec<PertComputedNode> {
    let mut es: BTreeMap<String, f64> = BTreeMap::new();
    let mut ef: BTreeMap<String, f64> = BTreeMap::new();
    let dur_map: BTreeMap<String, f64> = tasks
        .iter()
        .map(|t| (t.id.clone(), t.duration_days))
        .collect();

    // Forward pass (multi-pass relaxation over DAG)
    for _ in 0..tasks.len() {
        for t in tasks {
            let max_pred_ef = t
                .predecessors
                .iter()
                .filter_map(|p| ef.get(p).copied())
                .fold(0.0, f64::max);
            es.insert(t.id.clone(), max_pred_ef);
            ef.insert(t.id.clone(), max_pred_ef + t.duration_days);
        }
    }
    let project_duration = ef.values().copied().fold(0.0, f64::max);

    // Backward pass
    let mut lf: BTreeMap<String, f64> = tasks
        .iter()
        .map(|t| (t.id.clone(), project_duration))
        .collect();
    let mut ls: BTreeMap<String, f64> = BTreeMap::new();
    for _ in 0..tasks.len() {
        for t in tasks.iter().rev() {
            let min_succ_ls = tasks
                .iter()
                .filter(|succ| succ.predecessors.contains(&t.id))
                .filter_map(|succ| ls.get(&succ.id).copied())
                .fold(project_duration, f64::min);
            lf.insert(t.id.clone(), min_succ_ls);
            ls.insert(
                t.id.clone(),
                min_succ_ls - dur_map.get(&t.id).copied().unwrap_or(0.0),
            );
        }
    }

    let mut out = Vec::new();
    let mut id_map: BTreeMap<String, u64> = BTreeMap::new();

    if let Some(page) = doc.page_mut(page_id) {
        for t in tasks {
            let sid = page.next_shape_id();
            let e_s = es[&t.id];
            let e_f = ef[&t.id];
            let l_s = ls[&t.id];
            let l_f = lf[&t.id];
            let slack = (l_s - e_s).max(0.0);
            let is_critical = slack < 1e-6;

            let mut sh = VisioShape::new_rect(sid, format!("PERT.{}", t.id), 2.0, 4.0, 2.2, 1.1);
            sh.text = format!(
                "{}: {} ({}d)\nES:{} EF:{} | LS:{} LF:{}\nSlack: {}d",
                t.id,
                t.name,
                format_clean_f64(t.duration_days),
                format_clean_f64(e_s),
                format_clean_f64(e_f),
                format_clean_f64(l_s),
                format_clean_f64(l_f),
                format_clean_f64(slack)
            );
            sh.sheet.set_formula(
                "LineColor",
                if is_critical {
                    "=RGB(220,38,38)"
                } else {
                    "=RGB(37,99,235)"
                },
                Some(Unit::Color),
            );
            sh.sheet.set_formula(
                "LineWeight",
                if is_critical { "2.25 pt" } else { "1 pt" },
                Some(Unit::Point),
            );
            page.shapes.push(sh);
            id_map.insert(t.id.clone(), sid);
            out.push(PertComputedNode {
                id: t.id.clone(),
                shape_id: sid,
                early_start: e_s,
                early_finish: e_f,
                late_start: l_s,
                late_finish: l_f,
                slack,
                is_critical,
            });
        }

        for t in tasks {
            let dst_sid = id_map[&t.id];
            for pred in &t.predecessors {
                if let Some(&src_sid) = id_map.get(pred) {
                    let cid = page.next_shape_id();
                    let conn = VisioShape::new_connector(cid, "Dep", 0.0, 0.0, 1.0, 0.0);
                    page.shapes.push(conn);
                    page.connects.push(GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "BeginX".into(),
                        target_shape_id: src_sid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    });
                    page.connects.push(GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "EndX".into(),
                        target_shape_id: dst_sid,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    });
                }
            }
        }
    }

    apply_auto_layout(
        doc,
        page_id,
        LayoutOptions {
            algorithm: LayoutAlgorithm::HierarchicalLeftToRight,
            layer_spacing_in: 3.0,
            node_spacing_in: 1.6,
            margin_x_in: 1.5,
            margin_y_in: 1.5,
        },
    );
    out
}

/// Mount a rack appliance at exact EIA-310 19-inch rack U-position (`1U..42U`) inside a Rack frame shape.
pub fn mount_rack_equipment(
    doc: &mut VisioDocument,
    page_id: u32,
    rack_shape_id: u64,
    device_name: &str,
    u_start_1_to_42: u32,
    u_height: u32,
) -> Option<u64> {
    ReactiveEngine::recalculate_document(doc);
    let page = doc.page_mut(page_id)?;
    let rack = page.find_shape(rack_shape_id)?;
    let rx = rack.sheet.get_f64("PinX").unwrap_or(4.0);
    let ry = rack.sheet.get_f64("PinY").unwrap_or(5.0);
    let rw = rack.sheet.get_f64("Width").unwrap_or(2.2);
    let rh = rack.sheet.get_f64("Height").unwrap_or(7.35);
    let u_unit_in = rh / 42.0;
    let bottom_y = ry - rh * 0.5;
    let dev_h = u_height.max(1) as f64 * u_unit_in;
    let dev_cy = bottom_y + ((u_start_1_to_42.saturating_sub(1)) as f64 * u_unit_in) + dev_h * 0.5;

    let sid = page.next_shape_id();
    let mut dev = VisioShape::new_rect(sid, device_name, rx, dev_cy, rw * 0.9, dev_h);
    dev.text = format!("{device_name} ({u_height}U @ U{u_start_1_to_42})");
    dev.sheet.set_shape_data(ShapeDataRow {
        name: "RackU".into(),
        label: "Start U".into(),
        prompt: "EIA-310 U Position".into(),
        data_type: ShapeDataType::Number,
        format: "0".into(),
        value: u_start_1_to_42.to_string(),
        sort_key: "1".into(),
        invisible: false,
        ask: false,
        lang_id: 1033,
    });
    page.shapes.push(dev);
    ReactiveEngine::recalculate_document(doc);
    Some(sid)
}

/// Create an Architectural Room Space shape whose `Prop.AreaSqFt` updates reactively whenever resized!
pub fn create_architectural_room_space(
    doc: &mut VisioDocument,
    page_id: u32,
    room_name: &str,
    pin_x_in: f64,
    pin_y_in: f64,
    width_in: f64,
    height_in: f64,
) -> Option<u64> {
    let page = doc.page_mut(page_id)?;
    let sid = page.next_shape_id();
    let mut room = VisioShape::new_rect(sid, room_name, pin_x_in, pin_y_in, width_in, height_in);
    room.text = room_name.into();
    // Area in sq ft taking Page `DrawingScale / PageScale` into account!
    room.sheet.set_shape_data(ShapeDataRow {
        name: "AreaSqFt".into(),
        label: "Area (sq ft)".into(),
        prompt: "Live Architectural Floor Area".into(),
        data_type: ShapeDataType::Number,
        format: "0.00".into(),
        value: "=(Width * (ThePage!DrawingScale / ThePage!PageScale) / 12) * (Height * (ThePage!DrawingScale / ThePage!PageScale) / 12)".into(),
        sort_key: "1".into(),
        invisible: false,
        ask: false,
        lang_id: 1033,
    });
    page.shapes.push(room);
    ReactiveEngine::recalculate_document(doc);
    Some(sid)
}

// ============================================================================
// 3. Org Chart Wizard, Gantt Chart Builder & Project Timeline Builder
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GanttTaskInput {
    pub id: String,
    pub name: String,
    pub start_day: f64,
    pub duration_days: f64,
    pub percent_complete: f64,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineMilestone {
    pub day: f64,
    pub label: String,
    pub is_major: bool,
}

/// Parse `Name,Title,Manager,Department` CSV rows and construct a complete role-styled
/// hierarchical Visio Organization Chart with `Prop.*` metadata and orthogonal connectors.
pub fn build_org_chart_from_csv(doc: &mut VisioDocument, page_id: u32, csv_text: &str) -> usize {
    let Ok(rs) = crate::data_link::DataRecordset::from_csv(50, "OrgRoster", "Name", csv_text) else {
        return 0;
    };
    let mut id_by_name: BTreeMap<String, u64> = BTreeMap::new();
    let mut manager_links: Vec<(String, u64)> = Vec::new();

    if let Some(page) = doc.page_mut(page_id) {
        for r in &rs.rows {
            let name = r.get("Name").cloned().unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let title = r.get("Title").cloned().unwrap_or_else(|| "Member".into());
            let dept = r.get("Department").cloned().unwrap_or_else(|| "Engineering".into());
            let mgr = r.get("Manager").cloned().unwrap_or_default();

            let sid = page.next_shape_id();
            let mut card = VisioShape::new_rect(sid, name.clone(), 4.0, 5.0, 2.1, 0.95);
            card.text = format!("{name}\n{title}\n[{dept}]");
            card.sheet.set_formula("Rounding", "0.08 in", Some(Unit::Inch));

            let fill_formula = if mgr.trim().is_empty() || title.to_ascii_lowercase().contains("ceo") || title.to_ascii_lowercase().contains("chief") {
                "=RGB(191,219,254)"
            } else if title.to_ascii_lowercase().contains("vp") || title.to_ascii_lowercase().contains("director") {
                "=RGB(209,250,229)"
            } else if title.to_ascii_lowercase().contains("manager") || title.to_ascii_lowercase().contains("lead") {
                "=RGB(224,231,255)"
            } else {
                "=RGB(241,245,249)"
            };
            card.sheet.set_formula("FillForegnd", fill_formula, Some(Unit::Color));

            for (k, v) in [("Title", &title), ("Department", &dept), ("Manager", &mgr)] {
                card.sheet.set_shape_data(ShapeDataRow {
                    name: k.into(),
                    label: k.into(),
                    prompt: format!("Org Chart {k}"),
                    data_type: ShapeDataType::String,
                    format: String::new(),
                    value: format!("\"{}\"", v.replace('"', "\"\"")),
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }

            page.shapes.push(card);
            id_by_name.insert(name.to_ascii_lowercase(), sid);
            if !mgr.trim().is_empty() {
                manager_links.push((mgr.to_ascii_lowercase(), sid));
            }
        }

        for (mgr_lower, report_sid) in manager_links {
            if let Some(&mgr_sid) = id_by_name.get(&mgr_lower) {
                let cid = page.next_shape_id();
                let mut conn = VisioShape::new_connector(cid, "ReportsTo", 0.0, 0.0, 1.0, 0.0);
                conn.sheet.set_formula("EndArrow", "0", Some(Unit::Number));
                page.shapes.push(conn);
                page.connects.push(GlueEndpoint {
                    connector_shape_id: cid,
                    from_cell: "BeginX".into(),
                    target_shape_id: mgr_sid,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                });
                page.connects.push(GlueEndpoint {
                    connector_shape_id: cid,
                    from_cell: "EndX".into(),
                    target_shape_id: report_sid,
                    to_cell: "PinX".into(),
                    is_dynamic: true,
                });
            }
        }
    }

    apply_auto_layout(
        doc,
        page_id,
        LayoutOptions {
            algorithm: LayoutAlgorithm::CompactTree,
            layer_spacing_in: 1.7,
            node_spacing_in: 2.5,
            margin_x_in: 1.4,
            margin_y_in: 1.2,
        },
    );
    id_by_name.len()
}

/// Construct a complete Visio Gantt Schedule Chart with task table rows, proportional timeline bars,
/// inner completion `%` progress bars, and finish-to-start dependency connectors.
pub fn build_gantt_chart(doc: &mut VisioDocument, page_id: u32, tasks: &[GanttTaskInput]) -> usize {
    if tasks.is_empty() {
        return 0;
    }
    let total_days = tasks
        .iter()
        .map(|t| t.start_day + t.duration_days)
        .fold(10.0, f64::max);

    let left_col_x = 0.6;
    let left_col_w = 2.6;
    let chart_x0 = left_col_x + left_col_w + 0.15;
    let chart_w = 6.9;
    let top_y = 7.4;
    let row_h = 0.55;

    let mut bar_ids: BTreeMap<String, u64> = BTreeMap::new();

    if let Some(page) = doc.page_mut(page_id) {
        // Header Band
        let hdr_id = page.next_shape_id();
        let mut hdr = VisioShape::new_rect(
            hdr_id,
            "GanttHeader",
            left_col_x + (left_col_w + 0.15 + chart_w) * 0.5,
            top_y + 0.42,
            left_col_w + 0.15 + chart_w,
            0.42,
        );
        hdr.text = format!("Gantt Schedule — Horizon: {} Days", format_clean_f64(total_days));
        hdr.sheet.set_formula("FillForegnd", "=RGB(30,41,59)", Some(Unit::Color));
        hdr.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        page.shapes.push(hdr);

        for (idx, task) in tasks.iter().enumerate() {
            let row_cy = top_y - (idx as f64) * row_h;

            // Left Task Table Cell
            let lbl_id = page.next_shape_id();
            let mut lbl = VisioShape::new_rect(
                lbl_id,
                format!("TaskRow.{}", task.id),
                left_col_x + left_col_w * 0.5,
                row_cy,
                left_col_w,
                row_h * 0.88,
            );
            lbl.text = format!(
                "{} ({}d • {:.0}%)",
                task.name,
                format_clean_f64(task.duration_days),
                task.percent_complete
            );
            lbl.sheet.set_formula("FillForegnd", "=RGB(248,250,252)", Some(Unit::Color));
            page.shapes.push(lbl);

            // Proportional Gantt Bar
            let bar_x0 = chart_x0 + (task.start_day / total_days) * chart_w;
            let bar_w = ((task.duration_days / total_days) * chart_w).max(0.2);
            let bar_id = page.next_shape_id();
            let mut bar = VisioShape::new_rect(
                bar_id,
                format!("GanttBar.{}", task.id),
                bar_x0 + bar_w * 0.5,
                row_cy,
                bar_w,
                row_h * 0.52,
            );
            bar.sheet.set_formula("FillForegnd", "=RGB(147,197,253)", Some(Unit::Color));
            bar.sheet.set_formula("Rounding", "0.05 in", Some(Unit::Inch));

            // Inner Progress Fill Sub-Shape
            let pct = (task.percent_complete / 100.0).clamp(0.0, 1.0);
            if pct > 0.02 {
                let prog_w = (bar_w * pct).max(0.08);
                let mut prog = VisioShape::new_rect(
                    bar_id + 5000,
                    format!("GanttProgress.{}", task.id),
                    prog_w * 0.5,
                    row_h * 0.26,
                    prog_w,
                    row_h * 0.52,
                );
                prog.sheet.set_formula("FillForegnd", "=RGB(29,78,216)", Some(Unit::Color));
                bar.children.push(prog);
            }

            page.shapes.push(bar);
            bar_ids.insert(task.id.clone(), bar_id);
        }

        // Dependency Connectors
        for task in tasks {
            let Some(&dst_id) = bar_ids.get(&task.id) else {
                continue;
            };
            for dep in &task.depends_on {
                if let Some(&src_id) = bar_ids.get(dep) {
                    let cid = page.next_shape_id();
                    let conn = VisioShape::new_connector(cid, "GanttDep", 0.0, 0.0, 1.0, 0.0);
                    page.shapes.push(conn);
                    page.connects.push(GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "BeginX".into(),
                        target_shape_id: src_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    });
                    page.connects.push(GlueEndpoint {
                        connector_shape_id: cid,
                        from_cell: "EndX".into(),
                        target_shape_id: dst_id,
                        to_cell: "PinX".into(),
                        is_dynamic: true,
                    });
                }
            }
        }
    }

    route_page_connectors(doc, page_id);
    tasks.len()
}

/// Construct a horizontal Visio Executive Project Timeline with milestone diamonds and alternating callouts.
pub fn build_project_timeline(
    doc: &mut VisioDocument,
    page_id: u32,
    title: &str,
    total_days: f64,
    milestones: &[TimelineMilestone],
) -> usize {
    let spine_x0 = 1.2;
    let spine_w = 8.6;
    let spine_y = 4.25;
    let span = total_days.max(1.0);

    if let Some(page) = doc.page_mut(page_id) {
        let spine_id = page.next_shape_id();
        let mut spine = VisioShape::new_rect(
            spine_id,
            "TimelineSpine",
            spine_x0 + spine_w * 0.5,
            spine_y,
            spine_w,
            0.34,
        );
        spine.text = title.into();
        spine.sheet.set_formula("Rounding", "0.17 in", Some(Unit::Inch));
        spine.sheet.set_formula("FillForegnd", "=RGB(30,58,138)", Some(Unit::Color));
        spine.sheet.set_formula("Char.Color", "=RGB(255,255,255)", Some(Unit::Color));
        page.shapes.push(spine);

        for (i, m) in milestones.iter().enumerate() {
            let mx = spine_x0 + (m.day.clamp(0.0, span) / span) * spine_w;
            let diamond_id = page.next_shape_id();
            let sz = if m.is_major { 0.42 } else { 0.3 };
            let mut diamond = VisioShape::new_rect(diamond_id, format!("Milestone.{i}"), mx, spine_y, sz, sz);
            let mut g = GeometrySection::new(1);
            g.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "0 in"),
                GeometryRow::line_to(2, "=Width", "=Height*0.5"),
                GeometryRow::line_to(3, "=Width*0.5", "=Height"),
                GeometryRow::line_to(4, "0 in", "=Height*0.5"),
                GeometryRow::line_to(5, "=Width*0.5", "0 in"),
            ];
            diamond.sheet.add_geometry(g);
            diamond.sheet.set_formula(
                "FillForegnd",
                if m.is_major {
                    "=RGB(16,185,129)"
                } else {
                    "=RGB(56,189,248)"
                },
                Some(Unit::Color),
            );
            page.shapes.push(diamond);

            let label_cy = if i % 2 == 0 {
                spine_y + 1.35
            } else {
                spine_y - 1.35
            };
            let card_id = page.next_shape_id();
            let mut card = VisioShape::new_rect(card_id, format!("MilestoneCard.{i}"), mx, label_cy, 1.75, 0.65);
            card.text = format!("Day {}: {}", format_clean_f64(m.day), m.label);
            card.sheet.set_formula("FillForegnd", "=RGB(248,250,252)", Some(Unit::Color));
            page.shapes.push(card);

            let cid = page.next_shape_id();
            let mut leader = VisioShape::new_connector(cid, "MilestoneLeader", mx, spine_y, mx, label_cy);
            leader.sheet.set_formula("EndArrow", "0", Some(Unit::Number));
            page.shapes.push(leader);
            page.connects.push(GlueEndpoint {
                connector_shape_id: cid,
                from_cell: "BeginX".into(),
                target_shape_id: diamond_id,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
            page.connects.push(GlueEndpoint {
                connector_shape_id: cid,
                from_cell: "EndX".into(),
                target_shape_id: card_id,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
        }
    }

    route_page_connectors(doc, page_id);
    milestones.len()
}

/// Instantiate one of Vexio's 8 built-in Visio Backstage Starter Templates!
pub fn instantiate_starter_template(template_id: &str) -> VisioDocument {
    match template_id.trim().to_ascii_lowercase().as_str() {
        "flowchart" => {
            let mut sess = crate::workspace::DocumentSession::new_blank("Cross-Functional Flowchart");
            let s1 = sess.drop_builtin_shape(1, "Basic Flowchart", "Start/End", 2.0, 6.2).unwrap();
            let _ = sess.set_shape_text(1, s1, "Receive Request");
            let s2 = sess.drop_builtin_shape(1, "Basic Flowchart", "Process", 4.6, 6.2).unwrap();
            let _ = sess.set_shape_text(1, s2, "Validate Payload");
            let s3 = sess.drop_builtin_shape(1, "Basic Flowchart", "Decision", 7.2, 6.2).unwrap();
            let _ = sess.set_shape_text(1, s3, "Approved?");
            let s4 = sess.drop_builtin_shape(1, "Basic Flowchart", "Process", 7.2, 3.6).unwrap();
            let _ = sess.set_shape_text(1, s4, "Provision Service");
            let s5 = sess.drop_builtin_shape(1, "Basic Flowchart", "Start/End", 9.6, 3.6).unwrap();
            let _ = sess.set_shape_text(1, s5, "Complete");
            let _ = sess.connect_shapes(1, s1, s2);
            let _ = sess.connect_shapes(1, s2, s3);
            if let Ok(cy) = sess.connect_shapes(1, s3, s4) {
                let _ = sess.set_shape_text(1, cy, "Yes");
            }
            let _ = sess.connect_shapes(1, s4, s5);
            sess.doc
        }
        "bpmn_fulfillment" | "bpmn" | "bpmn2" => {
            let mut sess = crate::workspace::DocumentSession::new_blank("BPMN 2.0 Order Fulfillment");
            let s1 = sess.drop_builtin_shape(1, "BPMN 2.0", "Start Event", 1.5, 4.5).unwrap();
            let s2 = sess.drop_builtin_shape(1, "BPMN 2.0", "Task", 3.6, 4.5).unwrap();
            let _ = sess.set_shape_text(1, s2, "Verify Payment");
            let s3 = sess.drop_builtin_shape(1, "BPMN 2.0", "Exclusive Gateway", 5.8, 4.5).unwrap();
            let _ = sess.set_shape_text(1, s3, "Authorized?");
            let s4 = sess.drop_builtin_shape(1, "BPMN 2.0", "Task", 7.9, 5.6).unwrap();
            let _ = sess.set_shape_text(1, s4, "Dispatch Shipment");
            let s5 = sess.drop_builtin_shape(1, "BPMN 2.0", "End Event", 9.8, 4.5).unwrap();
            let _ = sess.connect_shapes(1, s1, s2);
            let _ = sess.connect_shapes(1, s2, s3);
            if let Ok(c_yes) = sess.connect_shapes(1, s3, s4) {
                let _ = sess.set_shape_text(1, c_yes, "Yes");
            }
            if let Ok(c_no) = sess.connect_shapes(1, s3, s5) {
                let _ = sess.set_shape_text(1, c_no, "Declined");
            }
            let _ = sess.connect_shapes(1, s4, s5);
            sess.doc
        }
        "cloud_microservices" | "network" => {
            let mut sess = crate::workspace::DocumentSession::new_blank("Cloud Kubernetes Architecture");
            let lb = sess
                .drop_builtin_shape(1, "Cloud Architecture (AWS / GCP / Azure)", "Edge CDN / Load Balancer", 2.0, 4.25)
                .unwrap();
            let k8s = sess
                .drop_builtin_shape(1, "Cloud Architecture (AWS / GCP / Azure)", "Kubernetes Cluster", 5.2, 5.4)
                .unwrap();
            let fn_s = sess
                .drop_builtin_shape(1, "Cloud Architecture (AWS / GCP / Azure)", "Serverless Function", 5.2, 3.0)
                .unwrap();
            let db = sess
                .drop_builtin_shape(1, "Cloud Architecture (AWS / GCP / Azure)", "Managed SQL / Spanner", 8.6, 4.25)
                .unwrap();
            let _ = sess.connect_shapes(1, lb, k8s);
            let _ = sess.connect_shapes(1, lb, fn_s);
            let _ = sess.connect_shapes(1, k8s, db);
            let _ = sess.connect_shapes(1, fn_s, db);
            let _ = crate::containers::create_container(&mut sess.doc, 1, "Production VPC (Multi-Zone)", &[k8s, fn_s, db], 0.4);
            sess.doc
        }
        "sql_ecommerce_erd" | "uml" | "erd" => {
            let mut doc = VisioDocument::new_blank("E-Commerce Crow's Foot ERD");
            reverse_engineer_sql_ddl(
                &mut doc,
                1,
                "CREATE TABLE customers (id INT PRIMARY KEY, email VARCHAR NOT NULL, tier VARCHAR); \
                 CREATE TABLE products (id INT PRIMARY KEY, sku VARCHAR NOT NULL, price_cents INT NOT NULL); \
                 CREATE TABLE orders (id INT PRIMARY KEY, customer_id INT NOT NULL REFERENCES customers(id), status VARCHAR NOT NULL); \
                 CREATE TABLE order_items (id INT PRIMARY KEY, order_id INT NOT NULL REFERENCES orders(id), product_id INT NOT NULL REFERENCES products(id), qty INT NOT NULL);",
                true,
            );
            doc
        }
        "executive_org_chart" | "orgchart" => {
            let mut doc = VisioDocument::new_blank("Executive Organization Chart");
            build_org_chart_from_csv(
                &mut doc,
                1,
                "Name,Title,Manager,Department\n\
                 Elena Rostova,Chief Executive Officer,,Executive\n\
                 Marcus Vance,VP Systems Engineering,Elena Rostova,Engineering\n\
                 Priya Nair,VP Product & Design,Elena Rostova,Product\n\
                 Liam O'Connor,Rust Graphics Lead,Marcus Vance,Core Engine\n\
                 Sofia Lindqvist,Platform UI Lead,Priya Nair,Design Systems",
            );
            doc
        }
        "release_gantt" | "gantt" => {
            let mut doc = VisioDocument::new_blank("Product Launch Gantt Schedule");
            build_gantt_chart(
                &mut doc,
                1,
                &[
                    GanttTaskInput { id: "1".into(), name: "ShapeSheet DAG".into(), start_day: 0.0, duration_days: 5.0, percent_complete: 100.0, depends_on: vec![] },
                    GanttTaskInput { id: "2".into(), name: "VSDX & CAD Codecs".into(), start_day: 4.0, duration_days: 6.0, percent_complete: 100.0, depends_on: vec!["1".into()] },
                    GanttTaskInput { id: "3".into(), name: "WGPU 120FPS Canvas".into(), start_day: 9.0, duration_days: 5.0, percent_complete: 85.0, depends_on: vec!["2".into()] },
                    GanttTaskInput { id: "4".into(), name: "Multi-OS GA Release".into(), start_day: 14.0, duration_days: 4.0, percent_complete: 50.0, depends_on: vec!["3".into()] },
                ],
            );
            doc
        }
        "project_timeline" | "timeline" => {
            let mut doc = VisioDocument::new_blank("Annual Architecture Roadmap");
            build_project_timeline(
                &mut doc,
                1,
                "Vexio Enterprise Roadmap (Q1–Q4)",
                120.0,
                &[
                    TimelineMilestone { day: 15.0, label: "Core Alpha".into(), is_major: true },
                    TimelineMilestone { day: 45.0, label: "CAD & VSD Parity".into(), is_major: false },
                    TimelineMilestone { day: 80.0, label: "Native Desktop Beta".into(), is_major: true },
                    TimelineMilestone { day: 115.0, label: "Global GA".into(), is_major: true },
                ],
            );
            doc
        }
        "datacenter_floorplan" | "floorplan" | "cad_floorplan" => {
            let mut doc = VisioDocument::new_blank("Data Center Architectural Floor Plan");
            if let Some(p) = doc.page_mut(1) {
                p.page_sheet.set_formula("PageScale", "0.25 in", Some(Unit::Inch));
                p.page_sheet.set_formula("DrawingScale", "12 in", Some(Unit::Inch));
            }
            let _ = create_architectural_room_space(&mut doc, 1, "Cold Aisle Pod A", 3.2, 4.5, 3.5, 3.0);
            let _ = create_architectural_room_space(&mut doc, 1, "UPS Power Room", 7.8, 4.5, 2.5, 3.0);
            if let Some(p) = doc.page_mut(1) {
                let rid = p.next_shape_id();
                p.shapes.push(VisioShape::new_rect(rid, "Rack-01", 3.2, 4.5, 1.4, 2.4));
                let _ = mount_rack_equipment(&mut doc, 1, rid, "Spine Switch", 38, 4);
            }
            ReactiveEngine::recalculate_document(&mut doc);
            doc
        }
        _ => VisioDocument::new_blank("Blank Drawing"),
    }
}

// ============================================================================
// 4. Complete 13-Pack High-Definition Built-In IP-Clean Stencil Library (126+ Masters)
// ============================================================================

pub fn build_all_builtin_stencils() -> Vec<StencilPack> {
    vec![
        make_stencil_pack(
            "Basic Shapes",
            "Basic Shapes",
            &[
                ("Rectangle", "Rectangle", 1.6, 1.0, "basic_rect"),
                ("Rounded Rectangle", "Rectangle", 1.6, 1.0, "basic_round_rect"),
                ("Snip-Corner Rectangle", "Rectangle", 1.6, 1.0, "basic_snip_rect"),
                ("Beveled Frame", "Rectangle", 1.6, 1.05, "basic_bevel_frame"),
                ("Square", "Rectangle", 1.1, 1.1, "basic_square"),
                ("Circle", "Circle", 1.0, 1.0, "basic_circle"),
                ("Ellipse", "Circle", 1.6, 1.0, "basic_ellipse"),
                ("Semi-Circle", "Rectangle", 1.4, 0.7, "basic_semicircle"),
                ("Quarter Pie Sector", "Rectangle", 1.2, 1.2, "basic_pie"),
                ("Triangle", "Triangle", 1.2, 1.04, "basic_triangle"),
                ("Right Triangle", "RightTriangle", 1.15, 1.15, "basic_right_triangle"),
                ("Inverted Triangle", "Rectangle", 1.2, 1.04, "basic_inv_triangle"),
                ("Diamond", "Diamond", 1.2, 1.2, "basic_diamond"),
                ("Parallelogram", "Parallelogram", 1.55, 0.95, "basic_parallelogram"),
                ("Trapezoid", "Trapezoid", 1.55, 0.95, "basic_trapezoid"),
                ("Pentagon", "Pentagon", 1.15, 1.1, "basic_pentagon"),
                ("Hexagon", "Hexagon", 1.24, 1.08, "basic_hexagon"),
                ("Heptagon", "Octagon", 1.2, 1.2, "basic_heptagon"),
                ("Octagon", "Octagon", 1.15, 1.15, "basic_octagon"),
                ("Decagon", "Circle", 1.2, 1.2, "basic_decagon"),
                ("Star (4-Point)", "Diamond", 1.2, 1.2, "basic_star4"),
                ("Star (5-Point)", "Star5", 1.2, 1.15, "basic_star"),
                ("Star (6-Point)", "Hexagon", 1.2, 1.2, "basic_star6"),
                ("Star (8-Point)", "Octagon", 1.2, 1.2, "basic_star8"),
                ("Cross / Plus", "Cross", 1.1, 1.1, "basic_cross"),
                ("Cylinder / Can", "Rectangle", 1.15, 1.35, "db_cluster"),
                ("3D Isometric Cube", "Cube3D", 1.25, 1.25, "basic_cube"),
                ("Ring / Frame", "RingFrame", 1.2, 1.2, "basic_ring"),
            ],
        ),
        make_stencil_pack(
            "Basic Flowchart",
            "Flowchart",
            &[
                ("Process", "Rectangle", 1.5, 0.9, "Process"),
                ("Decision", "Diamond", 1.4, 1.0, "Decision"),
                ("Start/End", "Pill", 1.5, 0.6, "Terminator"),
                ("Data (I/O)", "Parallelogram", 1.5, 0.85, "Data"),
                ("Subprocess", "Rectangle", 1.6, 0.95, "Subprocess"),
                ("Document", "DocumentWave", 1.5, 1.0, "flow_document"),
                ("Multi-Document", "DocumentWave", 1.6, 1.05, "flow_multidoc"),
                ("Database", "Rectangle", 1.15, 1.30, "db_cluster"),
                ("Manual Input", "ManualInput", 1.5, 0.9, "flow_manual_input"),
                ("Manual Operation", "Trapezoid", 1.55, 0.85, "flow_manual_op"),
                ("Preparation", "Hexagon", 1.6, 0.85, "flow_prep"),
                ("Delay", "DelayD", 1.4, 0.85, "flow_delay"),
                ("On-Page Reference", "Circle", 0.65, 0.65, "flow_onpage"),
                ("Off-Page Reference", "Pentagon", 0.95, 0.95, "flow_offpage"),
                ("Internal Storage", "Rectangle", 1.45, 0.95, "flow_internal_store"),
                ("Sequential Data (Tape)", "Circle", 1.05, 1.05, "flow_seq_tape"),
                ("Direct Data (Drum)", "Pill", 1.55, 0.90, "flow_direct_drum"),
                ("Stored Data", "Pill", 1.50, 0.85, "flow_stored_data"),
                ("Punched Card", "Rectangle", 1.50, 0.90, "flow_card"),
                ("Paper Tape", "DocumentWave", 1.55, 0.85, "flow_paper_tape"),
                ("Display", "Hexagon", 1.55, 0.85, "flow_display"),
                ("Merge", "Triangle", 1.15, 1.00, "flow_merge"),
                ("Or Junction", "Circle", 0.75, 0.75, "flow_or_junc"),
                ("Summing Junction", "Circle", 0.75, 0.75, "flow_sum_junc"),
            ],
        ),
        make_stencil_pack(
            "Microsoft 365 & Office",
            "Microsoft 365",
            &[
                ("Microsoft 365 Tenant", "Rectangle", 2.40, 1.50, "m365_tenant"),
                ("Microsoft Teams", "Rectangle", 1.05, 1.05, "m365_teams"),
                ("Exchange Online", "Rectangle", 1.05, 1.05, "m365_exchange"),
                ("SharePoint Online", "Circle", 1.05, 1.05, "m365_sharepoint"),
                ("OneDrive for Business", "Pill", 1.30, 0.90, "m365_onedrive"),
                ("Outlook Mail Flow", "Rectangle", 1.05, 1.05, "m365_outlook"),
                ("Word Document", "FoldedNote", 1.05, 1.05, "office_word"),
                ("Excel Workbook", "FoldedNote", 1.05, 1.05, "office_excel"),
                ("PowerPoint Deck", "Rectangle", 1.05, 1.05, "office_powerpoint"),
                ("OneNote Notebook", "Rectangle", 1.05, 1.05, "office_onenote"),
                ("Microsoft Visio", "Rectangle", 1.05, 1.05, "office_visio"),
                ("Microsoft Project", "Rectangle", 1.05, 1.05, "office_project"),
                ("Microsoft Access DB", "Rectangle", 1.05, 1.05, "office_access"),
                ("Microsoft Publisher", "FoldedNote", 1.05, 1.05, "office_publisher"),
                ("Microsoft Copilot", "Hexagon", 1.05, 1.05, "m365_copilot"),
                ("Microsoft Loop", "Rectangle", 1.05, 1.05, "m365_loop"),
                ("Microsoft Forms", "Rectangle", 1.05, 1.05, "m365_forms"),
                ("Microsoft Planner", "Rectangle", 1.05, 1.05, "m365_planner"),
                ("Microsoft To Do", "Rectangle", 1.05, 1.05, "m365_todo"),
                ("Microsoft Stream", "Pill", 1.15, 0.95, "m365_stream"),
                ("Microsoft Whiteboard", "Rectangle", 1.15, 1.00, "m365_whiteboard"),
                ("Viva Engage", "Circle", 1.05, 1.05, "m365_viva"),
                ("Viva Insights", "Hexagon", 1.05, 1.05, "m365_viva_insights"),
                ("Microsoft Intune", "Rectangle", 1.05, 1.05, "m365_intune"),
                ("Defender for M365", "Pentagon", 1.05, 1.05, "m365_defender"),
                ("Microsoft Purview", "Hexagon", 1.05, 1.05, "m365_purview"),
            ],
        ),
        make_stencil_pack(
            "Microsoft Servers & Active Directory",
            "Microsoft Servers",
            &[
                ("Windows Server", "Rectangle", 1.10, 1.30, "ms_win_server"),
                ("Windows Server Core", "Rectangle", 1.10, 1.30, "ms_server_core"),
                ("Active Directory DC", "Triangle", 1.10, 1.30, "ms_ad_dc"),
                ("Read-Only DC (RODC)", "Triangle", 1.10, 1.30, "ms_rodc"),
                ("Microsoft Entra ID", "Diamond", 1.05, 1.05, "ms_entra_id"),
                ("Entra Connect Sync", "Hexagon", 1.05, 1.05, "ms_entra_connect"),
                ("AD Forest / Domain Trust", "Pill", 1.60, 1.00, "ms_ad_trust"),
                ("Organizational Unit (OU)", "Rectangle", 1.15, 1.00, "ms_ad_ou"),
                ("Group Policy (GPO)", "FoldedNote", 1.05, 1.10, "ms_gpo"),
                ("AD Certificate Services (PKI)", "Pentagon", 1.10, 1.30, "ms_ad_cs"),
                ("AD Federation Services (ADFS)", "Diamond", 1.10, 1.30, "ms_adfs"),
                ("DNS / DHCP Server", "Rectangle", 1.10, 1.30, "ms_dns_dhcp"),
                ("Windows DNS Server", "Rectangle", 1.10, 1.30, "ms_win_dns"),
                ("Windows DHCP Server", "Rectangle", 1.10, 1.30, "ms_win_dhcp"),
                ("IIS Web Server", "Rectangle", 1.10, 1.30, "ms_iis"),
                ("SQL Server Cluster", "Rectangle", 1.10, 1.30, "ms_sql_server"),
                ("SQL Reporting (SSRS)", "FoldedNote", 1.10, 1.30, "ms_ssrs"),
                ("SQL Integration (SSIS)", "Hexagon", 1.10, 1.30, "ms_ssis"),
                ("Exchange Server", "Rectangle", 1.10, 1.30, "ms_exchange_srv"),
                ("SharePoint Server", "Rectangle", 1.10, 1.30, "ms_sharepoint_srv"),
                ("Hyper-V Host", "Cube3D", 1.10, 1.30, "ms_hyperv"),
                ("Failover Cluster", "Rectangle", 1.25, 1.30, "ms_failover_cluster"),
                ("System Center (MECM / SCOM)", "Hexagon", 1.10, 1.30, "ms_system_center"),
                ("WSUS Patch Server", "Rectangle", 1.10, 1.30, "ms_wsus"),
                ("Windows 11 Workstation", "Rectangle", 1.10, 1.05, "ms_win11_client"),
            ],
        ),
        make_stencil_pack(
            "Microsoft Azure & Hybrid Cloud",
            "Microsoft Azure",
            &[
                ("Azure Subscription / RG", "Rectangle", 2.40, 1.50, "az_resource_group"),
                ("Management Group", "Rectangle", 2.40, 1.50, "az_mgmt_group"),
                ("Azure Virtual Machine", "Rectangle", 1.05, 1.05, "az_vm"),
                ("VM Scale Set (VMSS)", "Rectangle", 1.05, 1.05, "az_vmss"),
                ("Azure Kubernetes (AKS)", "Hexagon", 1.05, 1.05, "az_aks"),
                ("Azure Container Apps", "Rectangle", 1.05, 1.05, "az_container_apps"),
                ("Azure App Service", "Circle", 1.05, 1.05, "az_app_service"),
                ("Azure Functions", "Diamond", 1.05, 1.05, "az_functions"),
                ("Azure Logic Apps", "Rectangle", 1.05, 1.05, "az_logic_apps"),
                ("API Management (APIM)", "Octagon", 1.05, 1.05, "az_apim"),
                ("Azure SQL / Cosmos DB", "Rectangle", 1.05, 1.05, "az_sql_cosmos"),
                ("Azure SQL Database", "Rectangle", 1.05, 1.05, "az_sql_db"),
                ("Azure Storage Account", "Rectangle", 1.05, 1.05, "az_storage"),
                ("Azure Data Lake Gen2", "Parallelogram", 1.05, 1.05, "az_datalake"),
                ("Azure Virtual Network (VNet)", "Pill", 2.40, 1.50, "az_vnet"),
                ("Network Security Group (NSG)", "Pentagon", 1.05, 1.05, "az_nsg"),
                ("Azure Firewall", "Rectangle", 1.45, 0.95, "az_firewall"),
                ("Azure Front Door / App GW", "Octagon", 1.05, 1.05, "az_front_door"),
                ("Application Gateway (WAF)", "Hexagon", 1.05, 1.05, "az_app_gw"),
                ("Azure Load Balancer", "Diamond", 1.05, 1.05, "az_lb"),
                ("Azure Bastion / NAT GW", "Pentagon", 1.05, 1.05, "az_bastion"),
                ("Azure VPN Gateway", "Diamond", 1.05, 1.05, "az_vpn_gw"),
                ("Azure Key Vault", "Circle", 1.05, 1.05, "az_key_vault"),
                ("Azure Service Bus / Event Hub", "Rectangle", 1.05, 1.05, "az_service_bus"),
                ("Azure Event Hub", "Pill", 1.05, 1.05, "az_event_hub"),
                ("Azure Monitor / Log Analytics", "Circle", 1.05, 1.05, "az_monitor"),
                ("Azure ExpressRoute / VPN", "Diamond", 1.05, 1.05, "az_expressroute"),
                ("Azure Arc (Hybrid)", "Pill", 1.05, 1.05, "az_arc"),
                ("Azure AI Foundry / OpenAI", "Hexagon", 1.05, 1.05, "az_openai"),
            ],
        ),
        make_stencil_pack(
            "Microsoft Power Platform & Dynamics 365",
            "Power Platform",
            &[
                ("Power BI Dashboard", "Rectangle", 1.05, 1.05, "pp_powerbi"),
                ("Power BI Semantic Model", "Hexagon", 1.05, 1.05, "pp_pbi_model"),
                ("Power BI Paginated Report", "FoldedNote", 1.05, 1.05, "pp_pbi_paginated"),
                ("Power Automate Flow", "Chevron", 1.05, 1.05, "pp_automate"),
                ("Power Automate Desktop (RPA)", "Rectangle", 1.05, 1.05, "pp_rpa"),
                ("Power Apps", "Diamond", 1.05, 1.05, "pp_powerapps"),
                ("Power Apps Model-Driven", "Rectangle", 1.05, 1.05, "pp_model_app"),
                ("Microsoft Dataverse", "Rectangle", 1.05, 1.05, "pp_dataverse"),
                ("AI Builder Model", "Hexagon", 1.05, 1.05, "pp_ai_builder"),
                ("Copilot Studio Agent", "CalloutBubble", 1.05, 1.05, "pp_copilot_studio"),
                ("Power Pages Portal", "Rectangle", 1.05, 1.05, "pp_power_pages"),
                ("Custom API Connector", "Pill", 1.05, 1.05, "pp_connector"),
                ("On-Premises Data Gateway", "Hexagon", 1.05, 1.05, "pp_gateway"),
                ("Managed Environment", "Rectangle", 2.40, 1.50, "pp_managed_env"),
                ("Dynamics 365 Sales / CRM", "Rectangle", 1.05, 1.05, "d365_crm"),
                ("Dynamics 365 Customer Service", "CalloutBubble", 1.05, 1.05, "d365_cs"),
                ("Dynamics 365 Field Service", "Hexagon", 1.05, 1.05, "d365_fs"),
                ("Dynamics 365 Finance & ERP", "Rectangle", 1.05, 1.05, "d365_erp"),
                ("Dynamics 365 Business Central", "Diamond", 1.05, 1.05, "d365_bc"),
                ("Microsoft Fabric Lakehouse", "Parallelogram", 1.05, 1.05, "ms_fabric"),
            ],
        ),
        make_stencil_pack(
            "Servers (Standard Roles)",
            "Servers",
            &[
                ("Tower Server", "Rectangle", 1.10, 1.30, "srv_tower"),
                ("Rackmount Server (1U / 2U)", "Rectangle", 1.85, 0.50, "srv_rack"),
                ("Rackmount Server (4U)", "Rectangle", 1.85, 0.85, "srv_rack_4u"),
                ("Blade Server Chassis", "Rectangle", 1.70, 1.15, "srv_blade"),
                ("Blade Server Module", "Rectangle", 0.70, 1.35, "srv_blade_mod"),
                ("Mainframe Host", "Rectangle", 1.50, 1.45, "srv_mainframe"),
                ("Microserver / Edge Node", "Rectangle", 1.15, 0.90, "srv_edge"),
                ("Web Server", "Rectangle", 1.10, 1.30, "srv_web"),
                ("Database Server", "Rectangle", 1.10, 1.30, "srv_db"),
                ("Application Server", "Rectangle", 1.10, 1.30, "srv_app"),
                ("File Server", "Rectangle", 1.10, 1.30, "srv_file"),
                ("Print Server", "Rectangle", 1.10, 1.30, "srv_print"),
                ("Mail / SMTP Server", "Rectangle", 1.10, 1.30, "srv_mail"),
                ("Directory / LDAP Server", "Rectangle", 1.10, 1.30, "srv_ldap"),
                ("DNS Server", "Rectangle", 1.10, 1.30, "srv_dns"),
                ("DHCP Server", "Rectangle", 1.10, 1.30, "srv_dhcp"),
                ("NTP Time Server", "Rectangle", 1.10, 1.30, "srv_ntp"),
                ("Proxy / Caching Server", "Rectangle", 1.10, 1.30, "srv_proxy"),
                ("Firewall / Bastion Server", "Rectangle", 1.10, 1.30, "srv_bastion"),
                ("FTP / SFTP Transfer Server", "Rectangle", 1.10, 1.30, "srv_ftp"),
                ("Syslog / SIEM Server", "Rectangle", 1.10, 1.30, "srv_siem"),
                ("Monitoring / NMS Server", "Rectangle", 1.10, 1.30, "srv_nms"),
                ("PKI Certificate Server", "Rectangle", 1.10, 1.30, "srv_pki"),
                ("Terminal / RDS Server", "Rectangle", 1.10, 1.30, "srv_rds"),
                ("Virtualization / Hypervisor Host", "Rectangle", 1.15, 1.30, "srv_hypervisor"),
                ("Backup & Storage Server", "Rectangle", 1.10, 1.30, "srv_backup"),
            ],
        ),
        make_stencil_pack(
            "Network & Rack Equipment",
            "Network",
            &[
                ("Enterprise Firewall", "Rectangle", 1.50, 0.95, "firewall"),
                ("Next-Gen Firewall (NGFW)", "Rectangle", 1.50, 0.95, "net_ngfw"),
                ("Web Application Firewall (WAF)", "Octagon", 1.45, 0.95, "net_waf"),
                ("IDS / IPS Security Appliance", "Hexagon", 1.45, 0.95, "net_ids_ips"),
                ("DDoS Mitigation Appliance", "Pentagon", 1.45, 0.95, "net_ddos"),
                ("VPN Concentrator / Gateway", "Rectangle", 1.55, 0.85, "net_vpn_gw"),
                ("NAC / RADIUS Appliance", "Hexagon", 1.45, 0.95, "net_nac"),
                ("Core Switch (1U)", "Rectangle", 1.85, 0.52, "switch_1u"),
                ("Layer 2 Access Switch", "Rectangle", 1.85, 0.52, "net_l2_switch"),
                ("Layer 3 Multilayer Switch", "Rectangle", 1.15, 1.15, "net_l3_switch"),
                ("Chassis Core Switch", "Rectangle", 1.50, 1.35, "net_chassis_sw"),
                ("PoE+ Managed Switch", "Rectangle", 1.85, 0.52, "net_poe_switch"),
                ("Fiber / SAN Switch", "Rectangle", 1.85, 0.52, "net_fiber_switch"),
                ("Industrial DIN Switch", "Rectangle", 0.85, 1.30, "net_din_switch"),
                ("Core Router", "Circle", 1.20, 0.85, "net_router"),
                ("Edge / Border Router (BGP)", "Circle", 1.20, 0.85, "net_edge_router"),
                ("Branch ISR Router", "Rectangle", 1.20, 0.85, "net_isr_router"),
                ("SD-WAN Edge Gateway", "Pill", 1.55, 0.85, "net_sdwan"),
                ("Load Balancer", "Hexagon", 1.45, 0.90, "net_lb"),
                ("Reverse Proxy / API Gateway", "Diamond", 1.45, 0.90, "net_rev_proxy"),
                ("Wi-Fi Access Point", "Circle", 1.05, 1.05, "net_wifi"),
                ("Outdoor Mesh AP", "Hexagon", 1.05, 1.05, "net_mesh_ap"),
                ("Wireless LAN Controller (WLC)", "Rectangle", 1.55, 0.85, "net_wlc"),
                ("Patch Panel (48-Port)", "Rectangle", 1.85, 0.48, "net_patch_panel"),
                ("Optical ODF Splice Tray", "Rectangle", 1.65, 0.75, "net_odf"),
                ("Cable / Fiber Modem (ONT)", "Rectangle", 1.45, 0.80, "net_modem"),
                ("Media Converter (Fiber/RJ45)", "Rectangle", 1.35, 0.78, "net_media_conv"),
                ("NAS / SAN Disk Array", "Rectangle", 1.45, 1.15, "net_nas_san"),
                ("Tape Library Autoloader", "Rectangle", 1.55, 0.95, "net_tape_lib"),
                ("UPS Battery Backup", "Rectangle", 1.05, 1.30, "net_ups"),
                ("Rack PDU Power Strip", "Rectangle", 1.85, 0.46, "net_pdu"),
                ("42U Standard Rack", "Rectangle", 1.40, 2.60, "rack_42u"),
                ("Cloud VPC", "Pill", 2.60, 1.70, "cloud_vpc"),
                ("Database Cluster", "Rectangle", 1.40, 1.15, "db_cluster"),
                ("Workstation / Laptop", "Rectangle", 1.25, 1.00, "net_laptop"),
                ("VoIP Desk Phone", "Rectangle", 1.15, 1.05, "net_voip"),
                ("IP Security Camera", "Circle", 1.05, 1.05, "net_ip_cam"),
                ("Network Laser Printer", "Rectangle", 1.20, 1.05, "net_printer"),
            ],
        ),
        make_stencil_pack(
            "Cloud Architecture (AWS / GCP / Azure)",
            "Cloud",
            &[
                ("Compute VM", "Rectangle", 1.05, 1.05, "cloud_vm"),
                ("Auto-Scaling Group", "Rectangle", 1.05, 1.05, "cloud_asg"),
                ("Spot Compute Instance", "Diamond", 1.05, 1.05, "cloud_spot"),
                ("Kubernetes Cluster", "Hexagon", 1.05, 1.05, "cloud_k8s"),
                ("Managed Container Pod", "Hexagon", 1.05, 1.05, "cloud_pod"),
                ("Serverless Function", "Hexagon", 1.05, 1.05, "cloud_fn"),
                ("Batch Compute Worker", "Rectangle", 1.05, 1.05, "cloud_batch"),
                ("Object Storage Bucket", "Parallelogram", 1.05, 1.05, "cloud_bucket"),
                ("Block Volume (SSD)", "Rectangle", 1.05, 1.05, "cloud_ebs"),
                ("Cold Archive Vault", "Pentagon", 1.05, 1.05, "cloud_glacier"),
                ("Managed SQL / Spanner", "Pill", 1.05, 1.05, "cloud_sql"),
                ("NoSQL Key-Value DB", "Hexagon", 1.05, 1.05, "cloud_nosql"),
                ("In-Memory Cache (Redis)", "Diamond", 1.05, 1.05, "cloud_cache"),
                ("Cloud Data Warehouse", "Octagon", 1.05, 1.05, "cloud_dwh"),
                ("Edge CDN / Load Balancer", "Diamond", 1.05, 1.05, "cloud_lb"),
                ("Cloud NAT Gateway", "Circle", 1.05, 1.05, "cloud_nat"),
                ("Cloud DNS Zone", "Circle", 1.05, 1.05, "cloud_dns"),
                ("Pub/Sub Event Bus", "Pill", 1.05, 1.05, "cloud_pubsub"),
                ("Workflow Orchestrator", "Rectangle", 1.05, 1.05, "cloud_workflow"),
                ("API Gateway", "Octagon", 1.05, 1.05, "cloud_apigw"),
                ("Secret Manager / KMS", "Circle", 1.05, 1.05, "cloud_kms"),
                ("IAM Security Shield", "Pentagon", 1.05, 1.05, "cloud_iam"),
            ],
        ),
        make_stencil_pack(
            "Arrows & Block Arrows",
            "Arrows",
            &[
                ("Right Block Arrow", "RightArrow", 1.7, 0.85, "basic_arrow_right"),
                ("Left Block Arrow", "LeftArrow", 1.7, 0.85, "basic_arrow_left"),
                ("Up Block Arrow", "UpArrow", 0.9, 1.55, "basic_arrow_up"),
                ("Down Block Arrow", "DownArrow", 0.9, 1.55, "basic_arrow_down"),
                ("Bidirectional Arrow", "BiArrow", 1.9, 0.8, "basic_arrow_bi"),
                ("Vertical Bi-Arrow", "Rectangle", 0.85, 1.75, "arrow_vert_bi"),
                ("Quad 4-Way Arrow", "QuadArrow", 1.4, 1.4, "basic_arrow_quad"),
                ("Chevron", "Chevron", 1.5, 0.85, "basic_chevron"),
                ("Double Chevron", "Chevron", 1.75, 0.85, "arrow_dbl_chevron"),
                ("Striped Right Arrow", "RightArrow", 1.8, 0.85, "arrow_striped"),
                ("Bent 90° Arrow", "BentArrow", 1.45, 1.35, "basic_arrow_bent"),
                ("Bent Left-Up Arrow", "BentArrow", 1.45, 1.35, "arrow_bent_lu"),
                ("U-Turn Arrow", "UTurnArrow", 1.45, 1.3, "basic_arrow_uturn"),
                ("Notched Signal Arrow", "NotchedArrow", 1.75, 0.85, "basic_arrow_notched"),
                ("Pentagon Tag Arrow", "RightArrow", 1.65, 0.80, "arrow_tag"),
                ("Split Fork Arrow", "Rectangle", 1.75, 1.25, "arrow_fork"),
                ("Merge Join Arrow", "Rectangle", 1.75, 1.25, "arrow_join"),
                ("Circular Loop Arrow", "RingFrame", 1.25, 1.25, "arrow_loop"),
                ("Curved Sweep Arrow", "Pill", 1.65, 0.95, "arrow_sweep"),
                ("Callout Block Arrow", "RightArrow", 1.95, 1.00, "arrow_callout"),
            ],
        ),
        make_stencil_pack(
            "Callouts, Notes & Containers",
            "Callouts",
            &[
                ("Callout Bubble", "CalloutBubble", 1.7, 1.1, "basic_callout"),
                ("Oval Callout", "Circle", 1.65, 1.05, "callout_oval"),
                ("Cloud Thought Bubble", "Pill", 1.75, 1.10, "callout_cloud"),
                ("Leader Line Callout", "Rectangle", 1.65, 0.95, "callout_leader"),
                ("Folded Note", "FoldedNote", 1.4, 1.2, "basic_note"),
                ("Sticky Memo Card", "Rectangle", 1.35, 1.30, "callout_sticky"),
                ("Pinned Note", "Rectangle", 1.35, 1.25, "callout_pinned"),
                ("Section Container", "Rectangle", 2.8, 1.8, "callout_container"),
                ("Dashed Region Group", "Rectangle", 2.6, 1.65, "callout_dashed_group"),
                ("Swimlane Header Card", "Rectangle", 2.2, 0.75, "callout_lane_hdr"),
                ("Curly Brace Left", "CurlyBraceLeft", 0.55, 1.5, "callout_brace_l"),
                ("Curly Brace Right", "CurlyBraceRight", 0.55, 1.5, "callout_brace_r"),
                ("Square Bracket Left", "Rectangle", 0.45, 1.5, "callout_sq_l"),
                ("Square Bracket Right", "Rectangle", 0.45, 1.5, "callout_sq_r"),
                ("Banner Ribbon", "BannerRibbon", 2.0, 0.75, "callout_banner"),
                ("Corner Tag Ribbon", "Parallelogram", 1.45, 0.65, "callout_tag"),
                ("Status Badge", "Pill", 1.25, 0.48, "callout_badge"),
                ("Warning Alert Box", "Rectangle", 1.85, 0.85, "callout_warning"),
                ("Numbered Step Badge", "Circle", 0.65, 0.65, "callout_step"),
                ("Title Header", "Rectangle", 2.6, 0.65, "callout_header"),
            ],
        ),
        make_stencil_pack(
            "Cross-Functional Swimlanes",
            "Swimlanes",
            &[
                ("Horizontal Swimlane", "Rectangle", 5.5, 1.5, "swimlane_horiz"),
                ("Vertical Swimlane", "Rectangle", 1.8, 4.2, "swimlane_vert"),
                ("Pool Header Frame", "Rectangle", 5.8, 2.2, "bpmn_pool"),
                ("Dual-Lane Pool", "Rectangle", 5.6, 2.6, "swim_dual_pool"),
                ("Three-Lane Pool", "Rectangle", 5.6, 3.6, "swim_triple_pool"),
                ("Phase Divider", "Rectangle", 0.35, 3.2, "swimlane_phase"),
                ("Milestone Phase Line", "Rectangle", 0.30, 3.2, "swim_milestone_line"),
                ("Handoff Node", "Chevron", 1.5, 0.8, "swimlane_handoff"),
                ("Cross-Team Escalation", "UpArrow", 0.95, 1.35, "swim_escalation"),
                ("Approval Gate", "Diamond", 1.2, 0.95, "Decision"),
                ("SLA Timer Gate", "Circle", 0.80, 0.80, "swim_sla_timer"),
                ("Shared Service Lane", "Rectangle", 5.2, 1.35, "swim_shared"),
                ("External Partner Lane", "Rectangle", 5.2, 1.35, "swim_external"),
                ("Automated System Lane", "Rectangle", 5.2, 1.35, "swim_system"),
                ("Customer Touchpoint", "Hexagon", 1.45, 0.85, "swim_touchpoint"),
                ("Audit / Control Check", "Pentagon", 1.10, 1.10, "swim_audit"),
                ("Bottleneck Marker", "Octagon", 1.00, 1.00, "swim_bottleneck"),
                ("Swimlane Summary Card", "Rectangle", 1.95, 1.05, "swim_summary"),
            ],
        ),
        make_stencil_pack(
            "BPMN 2.0",
            "Business Process",
            &[
                ("Start Event", "Circle", 0.5, 0.5, "start"),
                ("Message Start Event", "Circle", 0.55, 0.55, "bpmn_msg_start"),
                ("Timer Event", "Circle", 0.55, 0.55, "bpmn_timer"),
                ("Signal Start Event", "Circle", 0.55, 0.55, "bpmn_signal_start"),
                ("Conditional Event", "Circle", 0.55, 0.55, "bpmn_cond_event"),
                ("Intermediate Event", "Circle", 0.5, 0.5, "intermediate"),
                ("Catching Message Event", "Circle", 0.55, 0.55, "bpmn_catch_msg"),
                ("Throwing Message Event", "Circle", 0.55, 0.55, "bpmn_throw_msg"),
                ("Error Boundary Event", "Circle", 0.55, 0.55, "bpmn_error_ev"),
                ("Escalation Event", "Circle", 0.55, 0.55, "bpmn_esc_ev"),
                ("End Event", "Circle", 0.5, 0.5, "end"),
                ("Terminate End Event", "Circle", 0.55, 0.55, "bpmn_term_end"),
                ("Task", "Rectangle", 1.6, 0.9, "task"),
                ("User Task", "Rectangle", 1.65, 0.95, "bpmn_user_task"),
                ("Service Task", "Rectangle", 1.65, 0.95, "bpmn_service_task"),
                ("Send Task", "Rectangle", 1.65, 0.95, "bpmn_send_task"),
                ("Receive Task", "Rectangle", 1.65, 0.95, "bpmn_recv_task"),
                ("Script Task", "Rectangle", 1.65, 0.95, "bpmn_script_task"),
                ("Business Rule Task", "Rectangle", 1.65, 0.95, "bpmn_rule_task"),
                ("Manual Task", "Rectangle", 1.65, 0.95, "bpmn_manual_task"),
                ("Collapsed Subprocess", "Rectangle", 1.7, 1.0, "bpmn_subprocess"),
                ("Call Activity", "Rectangle", 1.7, 1.0, "bpmn_call_act"),
                ("Exclusive Gateway", "Diamond", 0.75, 0.75, "exclusive_gateway"),
                ("Parallel Gateway", "Diamond", 0.75, 0.75, "parallel_gateway"),
                ("Inclusive Gateway", "Diamond", 0.75, 0.75, "inclusive_gateway"),
                ("Event-Based Gateway", "Diamond", 0.80, 0.80, "bpmn_event_gw"),
                ("Data Object", "FoldedNote", 0.85, 1.05, "bpmn_data_obj"),
                ("Data Store", "Rectangle", 1.15, 1.05, "db_cluster"),
                ("Participant Pool", "Rectangle", 5.5, 1.8, "bpmn_pool"),
            ],
        ),
        make_stencil_pack(
            "UML 2.5",
            "Software & Database",
            &[
                ("Class", "Rectangle", 2.0, 1.4, "uml_class"),
                ("Abstract Class", "Rectangle", 2.0, 1.35, "uml_abstract"),
                ("Interface", "Rectangle", 2.0, 1.1, "uml_interface"),
                ("Enumeration", "Rectangle", 1.85, 1.2, "uml_enum"),
                ("Component", "Rectangle", 1.8, 1.0, "uml_component"),
                ("Package", "UmlPackage", 1.9, 1.25, "uml_package"),
                ("Subsystem", "UmlPackage", 2.1, 1.35, "uml_subsystem"),
                ("Artifact", "FoldedNote", 1.45, 1.1, "uml_artifact"),
                ("Use Case", "Circle", 1.65, 0.95, "uml_usecase"),
                ("Actor", "Rectangle", 0.85, 1.45, "uml_actor"),
                ("System Boundary", "Rectangle", 2.8, 2.0, "uml_boundary"),
                ("State", "Pill", 1.6, 0.8, "uml_state"),
                ("Composite State", "Pill", 2.2, 1.25, "uml_comp_state"),
                ("Activity Initial", "Circle", 0.42, 0.42, "uml_initial"),
                ("Activity Final", "Circle", 0.48, 0.48, "uml_final"),
                ("Flow Final", "Circle", 0.48, 0.48, "uml_flow_final"),
                ("Fork / Join Bar", "Rectangle", 1.8, 0.16, "uml_fork_join"),
                ("Actor / Lifeline", "Rectangle", 1.2, 2.5, "uml_lifeline"),
                ("Activation Bar", "Rectangle", 0.28, 1.35, "uml_activation"),
                ("Combined Fragment", "Rectangle", 2.5, 1.5, "uml_fragment"),
                ("Deployment Node (3D)", "Cube3D", 1.8, 1.3, "uml_node"),
                ("UML Note", "FoldedNote", 1.5, 1.0, "uml_note"),
            ],
        ),
        make_stencil_pack(
            "Database Crow's Foot & IDEF1X",
            "Database",
            &[
                ("Strong Entity", "Rectangle", 2.2, 1.3, "erd_strong"),
                ("Weak / Associative Entity", "Pill", 2.2, 1.3, "erd_weak"),
                ("Lookup / Reference Table", "Rectangle", 2.0, 1.15, "erd_lookup"),
                ("Junction / Bridge Table", "Diamond", 1.65, 1.10, "erd_junction"),
                ("Temporal History Table", "Rectangle", 2.1, 1.25, "erd_temporal"),
                ("Audit Log Table", "FoldedNote", 1.85, 1.20, "erd_audit"),
                ("Materialized View", "Parallelogram", 2.1, 1.1, "erd_view"),
                ("Standard SQL View", "Parallelogram", 2.0, 1.0, "erd_sql_view"),
                ("Partitioned Table", "Rectangle", 2.2, 1.35, "erd_partitioned"),
                ("External Foreign Table", "Rectangle", 2.1, 1.2, "erd_external"),
                ("PK Attribute", "Rectangle", 1.8, 0.38, "erd_pk"),
                ("FK Attribute", "Rectangle", 1.8, 0.38, "erd_fk"),
                ("Unique Key Attribute", "Rectangle", 1.8, 0.38, "erd_uk"),
                ("Nullable Column", "Pill", 1.7, 0.42, "erd_nullable"),
                ("JSON / Document Column", "Hexagon", 1.65, 0.65, "erd_json"),
                ("Vector Embedding Column", "Octagon", 1.65, 0.75, "erd_vector"),
                ("Index / Trigger", "Hexagon", 1.5, 0.65, "erd_index"),
                ("Stored Procedure", "Rectangle", 1.75, 0.85, "erd_proc"),
                ("Sequence Generator", "Circle", 0.95, 0.95, "erd_seq"),
                ("Schema Container", "Rectangle", 2.8, 1.85, "erd_schema"),
            ],
        ),
        make_stencil_pack(
            "Organization Chart",
            "Business",
            &[
                ("Executive", "Rectangle", 2.1, 0.95, "org_exec"),
                ("Board of Directors", "Rectangle", 2.25, 1.0, "org_board"),
                ("C-Suite Officer (CXO)", "Rectangle", 2.05, 0.95, "org_cxo"),
                ("VP / Director", "Rectangle", 2.0, 0.9, "org_vp"),
                ("Senior Manager", "Rectangle", 1.95, 0.88, "org_sr_mgr"),
                ("Manager", "Rectangle", 1.9, 0.85, "org_mgr"),
                ("Team Lead", "Rectangle", 1.85, 0.85, "org_lead"),
                ("Principal / Staff IC", "Rectangle", 1.85, 0.85, "org_staff"),
                ("Individual Contributor", "Rectangle", 1.8, 0.8, "org_ic"),
                ("Junior / Associate", "Rectangle", 1.75, 0.78, "org_assoc"),
                ("Intern / Trainee", "Pill", 1.65, 0.72, "org_intern"),
                ("Consultant / Contractor", "Rectangle", 1.8, 0.8, "org_consultant"),
                ("Vendor / Partner Team", "Hexagon", 1.85, 0.88, "org_vendor"),
                ("Executive Assistant", "Pill", 1.7, 0.75, "org_assistant"),
                ("Matrix Dotted Role", "Rectangle", 1.8, 0.8, "org_matrix"),
                ("Open Vacancy", "Rectangle", 1.8, 0.8, "org_vacancy"),
                ("Agile Squad / Pod", "Rectangle", 2.3, 1.25, "org_squad"),
                ("Department Header", "BannerRibbon", 2.3, 0.75, "org_dept"),
                ("Regional Office", "Rectangle", 1.95, 1.05, "org_office"),
                ("Advisory Council", "Pill", 2.0, 0.85, "org_council"),
            ],
        ),
        make_stencil_pack(
            "Floor Plan & Architecture",
            "Engineering & Architecture",
            &[
                ("Exterior Wall", "Rectangle", 4.0, 0.2, "wall"),
                ("Interior Partition", "Rectangle", 3.0, 0.12, "wall_inner"),
                ("Glass Curtain Wall", "Rectangle", 3.2, 0.18, "arch_glass_wall"),
                ("Structural Column", "Rectangle", 0.55, 0.55, "arch_column"),
                ("Single Door (90°)", "Rectangle", 0.9, 0.9, "door"),
                ("Double French Door", "Rectangle", 1.6, 0.85, "door_double"),
                ("Sliding Pocket Door", "Rectangle", 1.5, 0.28, "arch_sliding_door"),
                ("Revolving Entrance", "Circle", 1.25, 1.25, "arch_revolving"),
                ("Casement Window", "Rectangle", 1.5, 0.22, "arch_window"),
                ("Bay Window", "Trapezoid", 1.75, 0.55, "arch_bay_win"),
                ("Staircase Flight", "Rectangle", 1.8, 0.95, "arch_stairs"),
                ("Spiral Staircase", "Circle", 1.20, 1.20, "arch_spiral"),
                ("Elevator Shaft", "Rectangle", 1.2, 1.2, "arch_elevator"),
                ("Conference Table", "Pill", 2.2, 1.0, "arch_table"),
                ("Round Collaborator Table", "Circle", 1.25, 1.25, "arch_round_tbl"),
                ("Office Desk Workstation", "Rectangle", 1.6, 0.9, "arch_desk"),
                ("L-Shaped Corner Desk", "Rectangle", 1.65, 1.35, "arch_l_desk"),
                ("Ergonomic Task Chair", "Circle", 0.65, 0.65, "arch_chair"),
                ("Reception Counter", "Pill", 2.1, 0.85, "arch_reception"),
                ("Filing Cabinet", "Rectangle", 0.85, 1.05, "arch_cabinet"),
                ("Restroom Stall", "Rectangle", 1.15, 1.25, "arch_restroom"),
                ("Kitchenette Counter", "Rectangle", 1.85, 0.85, "arch_kitchen"),
                ("Emergency Exit Sign", "Rectangle", 0.95, 0.55, "arch_exit"),
                ("Server Closet", "Rectangle", 1.45, 1.25, "arch_closet"),
            ],
        ),
        make_stencil_pack(
            "Electrical & Electronic (IEEE / IEC)",
            "Electrical",
            &[
                ("Resistor (IEEE)", "Zigzag", 1.4, 0.45, "ieee_resistor"),
                ("Variable Potentiometer", "Zigzag", 1.4, 0.65, "ieee_pot"),
                ("Capacitor", "Rectangle", 0.9, 0.7, "ieee_capacitor"),
                ("Polarized Capacitor", "Rectangle", 0.95, 0.7, "ieee_pol_cap"),
                ("Inductor", "Pill", 1.3, 0.45, "ieee_inductor"),
                ("Step-Down Transformer", "Rectangle", 1.3, 1.1, "ieee_transformer"),
                ("PN Diode", "Rectangle", 1.1, 0.65, "ieee_diode"),
                ("Zener Diode", "Rectangle", 1.1, 0.65, "ieee_zener"),
                ("LED Emitting Diode", "Rectangle", 1.15, 0.75, "ieee_led"),
                ("Bridge Rectifier", "Diamond", 1.15, 1.15, "ieee_bridge"),
                ("NPN Transistor", "Circle", 1.05, 1.05, "ieee_npn"),
                ("N-Channel MOSFET", "Circle", 1.05, 1.05, "ieee_mosfet"),
                ("Op-Amp", "Triangle", 1.2, 1.0, "ieee_opamp"),
                ("Logic AND Gate", "DelayD", 1.25, 0.85, "ieee_and"),
                ("Logic OR Gate", "Chevron", 1.30, 0.85, "ieee_or"),
                ("Logic NOT Inverter", "Triangle", 1.10, 0.80, "ieee_not"),
                ("Crystal Oscillator", "Rectangle", 1.05, 0.75, "ieee_xtal"),
                ("Fuse / Breaker", "Rectangle", 1.15, 0.50, "ieee_fuse"),
                ("Relay Coil", "Rectangle", 1.15, 0.85, "ieee_relay"),
                ("Single-Pole Switch", "Rectangle", 1.1, 0.5, "ieee_switch"),
                ("Pushbutton Switch", "Rectangle", 1.1, 0.6, "ieee_pushbtn"),
                ("DC Battery", "Rectangle", 0.95, 0.7, "ieee_battery"),
                ("Earth Ground", "Triangle", 0.7, 0.55, "ieee_ground"),
                ("AC Voltage Source", "Circle", 0.85, 0.85, "ieee_ac_source"),
            ],
        ),
        make_stencil_pack(
            "P&ID Process Engineering",
            "Process Engineering",
            &[
                ("Centrifugal Pump", "Circle", 0.9, 0.9, "pid_pump"),
                ("Positive Displacement Pump", "Circle", 0.95, 0.95, "pid_pd_pump"),
                ("Compressor / Blower", "Trapezoid", 1.15, 0.95, "pid_compressor"),
                ("Turbine", "Trapezoid", 1.15, 0.95, "pid_turbine"),
                ("Control Valve", "Diamond", 0.7, 0.45, "pid_valve"),
                ("Gate Valve", "Diamond", 0.80, 0.50, "pid_gate_valve"),
                ("Globe Valve", "Diamond", 0.85, 0.55, "pid_globe_valve"),
                ("Ball Valve", "Diamond", 0.85, 0.55, "pid_ball_valve"),
                ("Check Valve", "Rectangle", 0.8, 0.5, "pid_check_valve"),
                ("Pressure Relief Valve", "Rectangle", 0.85, 0.95, "pid_prv"),
                ("3-Way Valve", "Triangle", 0.90, 0.75, "pid_3way"),
                ("Heat Exchanger", "Circle", 1.05, 1.05, "pid_heatex"),
                ("Shell & Tube Condenser", "Pill", 1.65, 0.85, "pid_condenser"),
                ("Cooling Tower", "Trapezoid", 1.35, 1.55, "pid_cooling_twr"),
                ("Fired Boiler", "Rectangle", 1.35, 1.50, "pid_boiler"),
                ("Vertical Storage Tank", "Rectangle", 1.2, 1.6, "pid_tank"),
                ("Cone-Roof Tank", "Pentagon", 1.30, 1.55, "pid_cone_tank"),
                ("Mixing Agitator Tank", "Rectangle", 1.25, 1.60, "pid_mixer"),
                ("Pressure Vessel", "Pill", 1.8, 0.9, "pid_vessel"),
                ("Distillation Column", "Pill", 0.95, 2.20, "pid_column"),
                ("Cyclone Separator", "Triangle", 1.05, 1.45, "pid_cyclone"),
                ("Inline Flow Meter", "Circle", 0.8, 0.8, "pid_flowmeter"),
            ],
        ),
        make_stencil_pack(
            "UI & Application Wireframes",
            "Wireframe",
            &[
                ("Browser / Desktop Frame", "Rectangle", 4.2, 2.8, "wf_window"),
                ("Mobile Phone Frame", "Rectangle", 1.5, 2.8, "wf_mobile"),
                ("Tablet Landscape Frame", "Rectangle", 3.2, 2.2, "wf_tablet"),
                ("Top App Bar", "Rectangle", 4.0, 0.45, "wf_appbar"),
                ("Bottom Tab Bar", "Rectangle", 2.8, 0.50, "wf_tabbar"),
                ("Navigation Rail", "Rectangle", 0.75, 2.4, "wf_navrail"),
                ("Breadcrumb Bar", "Rectangle", 2.6, 0.40, "wf_breadcrumb"),
                ("Hero Banner", "Rectangle", 3.4, 1.35, "wf_hero"),
                ("KPI Metric Card", "Rectangle", 1.6, 1.0, "wf_card"),
                ("Data Grid Table", "Rectangle", 3.0, 1.6, "wf_table"),
                ("Kanban Column", "Rectangle", 1.55, 2.2, "wf_kanban"),
                ("Primary Action Button", "Pill", 1.3, 0.42, "wf_button"),
                ("Secondary Outline Button", "Pill", 1.35, 0.42, "wf_sec_btn"),
                ("Segmented Control", "Pill", 2.0, 0.44, "wf_segmented"),
                ("Search Input", "Pill", 2.2, 0.42, "wf_input"),
                ("Text Input Field", "Rectangle", 2.1, 0.48, "wf_textfield"),
                ("Dropdown Select", "Rectangle", 1.9, 0.46, "wf_dropdown"),
                ("Checkbox & Radio Group", "Rectangle", 1.65, 1.05, "wf_checks"),
                ("Slider Control", "Pill", 2.0, 0.36, "wf_slider"),
                ("Toggle Switch", "Pill", 0.85, 0.42, "wf_toggle"),
                ("Toast Banner", "Pill", 2.3, 0.50, "wf_toast"),
                ("Modal Dialog", "Rectangle", 2.4, 1.5, "wf_modal"),
            ],
        ),
        make_stencil_pack(
            "Project Schedule (Gantt & Timeline)",
            "Schedule",
            &[
                ("Timeline Spine", "Pill", 5.0, 0.35, "sched_spine"),
                ("Milestone Diamond", "Diamond", 0.45, 0.45, "sched_milestone"),
                ("Major Release Star", "Star5", 0.65, 0.65, "sched_star"),
                ("Phase Interval Bar", "Pill", 2.4, 0.4, "sched_interval"),
                ("Gantt Task Bar", "Rectangle", 2.0, 0.35, "sched_gantt_bar"),
                ("Critical Path Bar", "Rectangle", 2.2, 0.38, "sched_critical"),
                ("Float / Slack Bar", "Rectangle", 1.6, 0.30, "sched_slack"),
                ("Summary Rollup Bar", "Rectangle", 3.0, 0.25, "sched_rollup"),
                ("Sprint Box", "Rectangle", 1.75, 0.85, "sched_sprint"),
                ("PI Planning Block", "Rectangle", 2.4, 1.10, "sched_pi"),
                ("Checkpoint Flag", "Rectangle", 0.85, 0.95, "sched_flag"),
                ("Go / No-Go Gate", "Diamond", 0.85, 0.85, "sched_gate"),
                ("Code Freeze Window", "Rectangle", 1.8, 0.65, "sched_freeze"),
                ("Today Marker Line", "Rectangle", 0.22, 2.4, "sched_today"),
                ("Resource Pill", "Pill", 1.35, 0.42, "sched_resource"),
                ("Risk Flag", "Pentagon", 0.95, 0.85, "sched_risk"),
                ("Deliverable Package", "Cube3D", 0.95, 0.95, "sched_deliverable"),
                ("Dependency Callout", "CalloutBubble", 1.45, 0.80, "sched_dep"),
                ("Status Header", "BannerRibbon", 2.4, 0.60, "sched_header"),
                ("Swimlane Track", "Rectangle", 4.5, 0.85, "sched_track"),
            ],
        ),
        make_stencil_pack(
            "Value Stream Mapping (Lean VSM)",
            "Lean Manufacturing",
            &[
                ("Customer / Supplier", "Rectangle", 1.6, 1.1, "vsm_customer"),
                ("Dedicated Process", "Rectangle", 1.8, 1.1, "vsm_dedicated"),
                ("Shared Process", "Rectangle", 1.8, 1.1, "vsm_shared"),
                ("U-Shaped Workcell", "Rectangle", 1.45, 1.20, "vsm_workcell"),
                ("VSM Data Box", "Rectangle", 1.65, 1.25, "vsm_databox"),
                ("Inventory Triangle", "Triangle", 0.85, 0.8, "vsm_inventory"),
                ("Safety Buffer", "Rectangle", 0.95, 1.15, "vsm_buffer"),
                ("Supermarket", "Rectangle", 1.0, 1.2, "vsm_supermarket"),
                ("Withdrawal Kanban", "FoldedNote", 1.10, 0.85, "vsm_w_kanban"),
                ("Production Kanban", "FoldedNote", 1.10, 0.85, "vsm_p_kanban"),
                ("Signal Kanban", "Triangle", 1.05, 0.90, "vsm_sig_kanban"),
                ("Kanban Post", "Rectangle", 0.75, 1.15, "vsm_post"),
                ("FIFO Lane", "Rectangle", 1.8, 0.55, "vsm_fifo"),
                ("Sequenced Pull Ball", "Circle", 0.75, 0.75, "vsm_pull_ball"),
                ("Heijunka Box", "Rectangle", 1.65, 1.00, "vsm_heijunka"),
                ("Kaizen Burst", "Hexagon", 1.3, 0.85, "vsm_kaizen"),
                ("Operator Symbol", "Circle", 0.80, 0.80, "vsm_operator"),
                ("Truck Shipment", "Rectangle", 1.5, 0.9, "vsm_truck"),
                ("Air / Rail Freight", "Parallelogram", 1.65, 0.85, "vsm_freight"),
                ("Lead-Time Ladder", "Rectangle", 2.6, 0.65, "vsm_ladder"),
            ],
        ),
    ]
}

fn attach_standard_cardinal_ports(sh: &mut VisioShape) {
    if !sh.sheet.connection_points.is_empty() {
        return;
    }
    for (ix, name, x, y, dx, dy) in [
        (1u32, "West", "0 in", "=Height*0.5", "-1 in", "0 in"),
        (2u32, "East", "=Width", "=Height*0.5", "1 in", "0 in"),
        (3u32, "South", "=Width*0.5", "0 in", "0 in", "-1 in"),
        (4u32, "North", "=Width*0.5", "=Height", "0 in", "1 in"),
    ] {
        sh.sheet.add_connection_point(ConnectionPointRow {
            name: name.into(),
            ix,
            x: x.into(),
            y: y.into(),
            dir_x: dx.into(),
            dir_y: dy.into(),
            point_type: 0,
        });
    }
}

fn make_stencil_pack(
    title: &str,
    category: &str,
    specs: &[(&str, &str, f64, f64, &str)],
) -> StencilPack {
    let mut masters = Vec::new();
    for (i, &(name, geom_kind, w, h, role)) in specs.iter().enumerate() {
        let mid = (i + 1) as u32;
        let mut sh = VisioShape::new_rect(1, name, w * 0.5, h * 0.5, w, h);
        sh.sheet
            .set_user_cell("DomainRole", format!("\"{role}\""), "Visio Domain Role");
        match geom_kind {
            "Diamond" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.5", "0 in"),
                    GeometryRow::line_to(2, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(3, "=Width*0.5", "=Height"),
                    GeometryRow::line_to(4, "0 in", "=Height*0.5"),
                    GeometryRow::line_to(5, "=Width*0.5", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Circle" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![GeometryRow {
                    ix: 1,
                    kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                    x: "=Width*0.5".into(),
                    y: "=Height*0.5".into(),
                    a: Some("=Width".into()),
                    b: Some("=Height*0.5".into()),
                    c: Some("=Width*0.5".into()),
                    d: Some("=Height".into()),
                    e: None,
                    deleted: false,
                }];
                sh.sheet.add_geometry(g);
            }
            "Pill" => {
                sh.sheet
                    .set_formula("Rounding", "=MIN(Width,Height)*0.5", Some(Unit::Inch));
            }
            "Parallelogram" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.16", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.84", "=Height"),
                    GeometryRow::line_to(4, "0 in", "=Height"),
                    GeometryRow::line_to(5, "=Width*0.16", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Hexagon" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.22", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.78", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(4, "=Width*0.78", "=Height"),
                    GeometryRow::line_to(5, "=Width*0.22", "=Height"),
                    GeometryRow::line_to(6, "0 in", "=Height*0.5"),
                    GeometryRow::line_to(7, "=Width*0.22", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Pentagon" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.5", "=Height"),
                    GeometryRow::line_to(2, "=Width", "=Height*0.62"),
                    GeometryRow::line_to(3, "=Width*0.81", "0 in"),
                    GeometryRow::line_to(4, "=Width*0.19", "0 in"),
                    GeometryRow::line_to(5, "0 in", "=Height*0.62"),
                    GeometryRow::line_to(6, "=Width*0.5", "=Height"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Octagon" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.29", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.71", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.29"),
                    GeometryRow::line_to(4, "=Width", "=Height*0.71"),
                    GeometryRow::line_to(5, "=Width*0.71", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.29", "=Height"),
                    GeometryRow::line_to(7, "0 in", "=Height*0.71"),
                    GeometryRow::line_to(8, "0 in", "=Height*0.29"),
                    GeometryRow::line_to(9, "=Width*0.29", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Triangle" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.5", "=Height"),
                    GeometryRow::line_to(4, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "RightTriangle" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "0 in", "=Height"),
                    GeometryRow::line_to(4, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Star5" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.50", "=Height"),
                    GeometryRow::line_to(2, "=Width*0.62", "=Height*0.62"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.62"),
                    GeometryRow::line_to(4, "=Width*0.69", "=Height*0.38"),
                    GeometryRow::line_to(5, "=Width*0.81", "0 in"),
                    GeometryRow::line_to(6, "=Width*0.50", "=Height*0.24"),
                    GeometryRow::line_to(7, "=Width*0.19", "0 in"),
                    GeometryRow::line_to(8, "=Width*0.31", "=Height*0.38"),
                    GeometryRow::line_to(9, "0 in", "=Height*0.62"),
                    GeometryRow::line_to(10, "=Width*0.38", "=Height*0.62"),
                    GeometryRow::line_to(11, "=Width*0.50", "=Height"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Cross" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.35", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.65", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.65", "=Height*0.35"),
                    GeometryRow::line_to(4, "=Width", "=Height*0.35"),
                    GeometryRow::line_to(5, "=Width", "=Height*0.65"),
                    GeometryRow::line_to(6, "=Width*0.65", "=Height*0.65"),
                    GeometryRow::line_to(7, "=Width*0.65", "=Height"),
                    GeometryRow::line_to(8, "=Width*0.35", "=Height"),
                    GeometryRow::line_to(9, "=Width*0.35", "=Height*0.65"),
                    GeometryRow::line_to(10, "0 in", "=Height*0.65"),
                    GeometryRow::line_to(11, "0 in", "=Height*0.35"),
                    GeometryRow::line_to(12, "=Width*0.35", "=Height*0.35"),
                    GeometryRow::line_to(13, "=Width*0.35", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "RightArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.26"),
                    GeometryRow::line_to(2, "=Width*0.62", "=Height*0.26"),
                    GeometryRow::line_to(3, "=Width*0.62", "0 in"),
                    GeometryRow::line_to(4, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(5, "=Width*0.62", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.62", "=Height*0.74"),
                    GeometryRow::line_to(7, "0 in", "=Height*0.74"),
                    GeometryRow::line_to(8, "0 in", "=Height*0.26"),
                ];
                sh.sheet.add_geometry(g);
            }
            "LeftArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width", "=Height*0.26"),
                    GeometryRow::line_to(2, "=Width*0.38", "=Height*0.26"),
                    GeometryRow::line_to(3, "=Width*0.38", "0 in"),
                    GeometryRow::line_to(4, "0 in", "=Height*0.5"),
                    GeometryRow::line_to(5, "=Width*0.38", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.38", "=Height*0.74"),
                    GeometryRow::line_to(7, "=Width", "=Height*0.74"),
                    GeometryRow::line_to(8, "=Width", "=Height*0.26"),
                ];
                sh.sheet.add_geometry(g);
            }
            "BiArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                    GeometryRow::line_to(2, "=Width*0.26", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.26", "=Height*0.28"),
                    GeometryRow::line_to(4, "=Width*0.74", "=Height*0.28"),
                    GeometryRow::line_to(5, "=Width*0.74", "0 in"),
                    GeometryRow::line_to(6, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(7, "=Width*0.74", "=Height"),
                    GeometryRow::line_to(8, "=Width*0.74", "=Height*0.72"),
                    GeometryRow::line_to(9, "=Width*0.26", "=Height*0.72"),
                    GeometryRow::line_to(10, "=Width*0.26", "=Height"),
                    GeometryRow::line_to(11, "0 in", "=Height*0.5"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Chevron" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.76", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(4, "=Width*0.76", "=Height"),
                    GeometryRow::line_to(5, "0 in", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.24", "=Height*0.5"),
                    GeometryRow::line_to(7, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "DocumentWave" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.14"),
                    GeometryRow::arc_to(2, "=Width*0.5", "=Height*0.14", "=MIN(Width,Height)*-0.08"),
                    GeometryRow::arc_to(3, "=Width", "=Height*0.14", "=MIN(Width,Height)*0.08"),
                    GeometryRow::line_to(4, "=Width", "=Height"),
                    GeometryRow::line_to(5, "0 in", "=Height"),
                    GeometryRow::line_to(6, "0 in", "=Height*0.14"),
                ];
                sh.sheet.add_geometry(g);
            }
            "ManualInput" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height"),
                    GeometryRow::line_to(4, "0 in", "=Height*0.74"),
                    GeometryRow::line_to(5, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "DelayD" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.62", "0 in"),
                    GeometryRow::arc_to(3, "=Width*0.62", "=Height", "=MIN(Width,Height)*0.32"),
                    GeometryRow::line_to(4, "0 in", "=Height"),
                    GeometryRow::line_to(5, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "FoldedNote" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.78"),
                    GeometryRow::line_to(4, "=Width*0.78", "=Height"),
                    GeometryRow::line_to(5, "0 in", "=Height"),
                    GeometryRow::line_to(6, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
                let mut fold = GeometrySection::new(2);
                fold.no_fill = "TRUE".into();
                fold.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.78", "=Height"),
                    GeometryRow::line_to(2, "=Width*0.78", "=Height*0.78"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.78"),
                ];
                sh.sheet.add_geometry(fold);
            }
            "CalloutBubble" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.24"),
                    GeometryRow::line_to(2, "=Width*0.24", "=Height*0.24"),
                    GeometryRow::line_to(3, "=Width*0.14", "0 in"),
                    GeometryRow::line_to(4, "=Width*0.44", "=Height*0.24"),
                    GeometryRow::line_to(5, "=Width", "=Height*0.24"),
                    GeometryRow::line_to(6, "=Width", "=Height"),
                    GeometryRow::line_to(7, "0 in", "=Height"),
                    GeometryRow::line_to(8, "0 in", "=Height*0.24"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Cube3D" => {
                let mut g1 = GeometrySection::new(1);
                g1.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.76", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.24"),
                    GeometryRow::line_to(4, "=Width", "=Height"),
                    GeometryRow::line_to(5, "=Width*0.24", "=Height"),
                    GeometryRow::line_to(6, "0 in", "=Height*0.76"),
                    GeometryRow::line_to(7, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g1);
                let mut seams = GeometrySection::new(2);
                seams.no_fill = "TRUE".into();
                seams.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                    GeometryRow::line_to(2, "=Width*0.76", "=Height*0.76"),
                    GeometryRow::line_to(3, "=Width", "=Height"),
                    GeometryRow::move_to(4, "=Width*0.76", "=Height*0.76"),
                    GeometryRow::line_to(5, "=Width*0.76", "0 in"),
                ];
                sh.sheet.add_geometry(seams);
            }
            "UmlPackage" => {
                let mut g1 = GeometrySection::new(1);
                g1.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width", "=Height*0.78"),
                    GeometryRow::line_to(4, "=Width*0.44", "=Height*0.78"),
                    GeometryRow::line_to(5, "=Width*0.38", "=Height"),
                    GeometryRow::line_to(6, "0 in", "=Height"),
                    GeometryRow::line_to(7, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g1);
                let mut tab_line = GeometrySection::new(2);
                tab_line.no_fill = "TRUE".into();
                tab_line.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.78"),
                    GeometryRow::line_to(2, "=Width*0.44", "=Height*0.78"),
                ];
                sh.sheet.add_geometry(tab_line);
            }
            "Zigzag" => {
                let mut g = GeometrySection::new(1);
                g.no_fill = "TRUE".into();
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                    GeometryRow::line_to(2, "=Width*0.15", "=Height*0.5"),
                    GeometryRow::line_to(3, "=Width*0.27", "=Height"),
                    GeometryRow::line_to(4, "=Width*0.42", "0 in"),
                    GeometryRow::line_to(5, "=Width*0.58", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.73", "0 in"),
                    GeometryRow::line_to(7, "=Width*0.85", "=Height*0.5"),
                    GeometryRow::line_to(8, "=Width", "=Height*0.5"),
                ];
                sh.sheet.add_geometry(g);
            }
            "Trapezoid" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.80", "=Height"),
                    GeometryRow::line_to(4, "=Width*0.20", "=Height"),
                    GeometryRow::line_to(5, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "RingFrame" => {
                let mut g1 = GeometrySection::new(1);
                g1.rows = vec![GeometryRow {
                    ix: 1,
                    kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                    x: "=Width*0.5".into(),
                    y: "=Height*0.5".into(),
                    a: Some("=Width".into()),
                    b: Some("=Height*0.5".into()),
                    c: Some("=Width*0.5".into()),
                    d: Some("=Height".into()),
                    e: None,
                    deleted: false,
                }];
                sh.sheet.add_geometry(g1);
                let mut inner = GeometrySection::new(2);
                inner.rows = vec![GeometryRow {
                    ix: 1,
                    kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                    x: "=Width*0.5".into(),
                    y: "=Height*0.5".into(),
                    a: Some("=Width*0.76".into()),
                    b: Some("=Height*0.5".into()),
                    c: Some("=Width*0.5".into()),
                    d: Some("=Height*0.76".into()),
                    e: None,
                    deleted: false,
                }];
                sh.sheet.add_geometry(inner);
            }
            "UpArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.26", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.74", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.74", "=Height*0.62"),
                    GeometryRow::line_to(4, "=Width", "=Height*0.62"),
                    GeometryRow::line_to(5, "=Width*0.5", "=Height"),
                    GeometryRow::line_to(6, "0 in", "=Height*0.62"),
                    GeometryRow::line_to(7, "=Width*0.26", "=Height*0.62"),
                    GeometryRow::line_to(8, "=Width*0.26", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "DownArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.26", "=Height"),
                    GeometryRow::line_to(2, "=Width*0.74", "=Height"),
                    GeometryRow::line_to(3, "=Width*0.74", "=Height*0.38"),
                    GeometryRow::line_to(4, "=Width", "=Height*0.38"),
                    GeometryRow::line_to(5, "=Width*0.5", "0 in"),
                    GeometryRow::line_to(6, "0 in", "=Height*0.38"),
                    GeometryRow::line_to(7, "=Width*0.26", "=Height*0.38"),
                    GeometryRow::line_to(8, "=Width*0.26", "=Height"),
                ];
                sh.sheet.add_geometry(g);
            }
            "QuadArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.5", "=Height"),
                    GeometryRow::line_to(2, "=Width*0.68", "=Height*0.76"),
                    GeometryRow::line_to(3, "=Width*0.58", "=Height*0.76"),
                    GeometryRow::line_to(4, "=Width*0.58", "=Height*0.58"),
                    GeometryRow::line_to(5, "=Width*0.76", "=Height*0.58"),
                    GeometryRow::line_to(6, "=Width*0.76", "=Height*0.68"),
                    GeometryRow::line_to(7, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(8, "=Width*0.76", "=Height*0.32"),
                    GeometryRow::line_to(9, "=Width*0.76", "=Height*0.42"),
                    GeometryRow::line_to(10, "=Width*0.58", "=Height*0.42"),
                    GeometryRow::line_to(11, "=Width*0.58", "=Height*0.24"),
                    GeometryRow::line_to(12, "=Width*0.68", "=Height*0.24"),
                    GeometryRow::line_to(13, "=Width*0.5", "0 in"),
                    GeometryRow::line_to(14, "=Width*0.32", "=Height*0.24"),
                    GeometryRow::line_to(15, "=Width*0.42", "=Height*0.24"),
                    GeometryRow::line_to(16, "=Width*0.42", "=Height*0.42"),
                    GeometryRow::line_to(17, "=Width*0.24", "=Height*0.42"),
                    GeometryRow::line_to(18, "=Width*0.24", "=Height*0.32"),
                    GeometryRow::line_to(19, "0 in", "=Height*0.5"),
                    GeometryRow::line_to(20, "=Width*0.24", "=Height*0.68"),
                    GeometryRow::line_to(21, "=Width*0.24", "=Height*0.58"),
                    GeometryRow::line_to(22, "=Width*0.42", "=Height*0.58"),
                    GeometryRow::line_to(23, "=Width*0.42", "=Height*0.76"),
                    GeometryRow::line_to(24, "=Width*0.32", "=Height*0.76"),
                    GeometryRow::line_to(25, "=Width*0.5", "=Height"),
                ];
                sh.sheet.add_geometry(g);
            }
            "BentArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.28", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.28", "=Height*0.56"),
                    GeometryRow::line_to(4, "=Width*0.66", "=Height*0.56"),
                    GeometryRow::line_to(5, "=Width*0.66", "=Height*0.36"),
                    GeometryRow::line_to(6, "=Width", "=Height*0.68"),
                    GeometryRow::line_to(7, "=Width*0.66", "=Height"),
                    GeometryRow::line_to(8, "=Width*0.66", "=Height*0.80"),
                    GeometryRow::line_to(9, "0 in", "=Height*0.80"),
                    GeometryRow::line_to(10, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "UTurnArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width*0.26", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.26", "=Height*0.68"),
                    GeometryRow::line_to(4, "=Width*0.64", "=Height*0.68"),
                    GeometryRow::line_to(5, "=Width*0.64", "=Height*0.36"),
                    GeometryRow::line_to(6, "=Width*0.48", "=Height*0.36"),
                    GeometryRow::line_to(7, "=Width*0.76", "0 in"),
                    GeometryRow::line_to(8, "=Width", "=Height*0.36"),
                    GeometryRow::line_to(9, "=Width*0.88", "=Height*0.36"),
                    GeometryRow::line_to(10, "=Width*0.88", "=Height*0.94"),
                    GeometryRow::line_to(11, "0 in", "=Height*0.94"),
                    GeometryRow::line_to(12, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            "NotchedArrow" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "=Height*0.22"),
                    GeometryRow::line_to(2, "=Width*0.66", "=Height*0.22"),
                    GeometryRow::line_to(3, "=Width*0.66", "0 in"),
                    GeometryRow::line_to(4, "=Width", "=Height*0.5"),
                    GeometryRow::line_to(5, "=Width*0.66", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.66", "=Height*0.78"),
                    GeometryRow::line_to(7, "0 in", "=Height*0.78"),
                    GeometryRow::line_to(8, "=Width*0.18", "=Height*0.5"),
                    GeometryRow::line_to(9, "0 in", "=Height*0.22"),
                ];
                sh.sheet.add_geometry(g);
            }
            "CurlyBraceLeft" => {
                let mut g = GeometrySection::new(1);
                g.no_fill = "TRUE".into();
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.85", "=Height"),
                    GeometryRow::arc_to(2, "=Width*0.45", "=Height*0.84", "=MIN(Width,Height)*0.05"),
                    GeometryRow::line_to(3, "=Width*0.45", "=Height*0.58"),
                    GeometryRow::arc_to(4, "=Width*0.10", "=Height*0.50", "=MIN(Width,Height)*-0.05"),
                    GeometryRow::arc_to(5, "=Width*0.45", "=Height*0.42", "=MIN(Width,Height)*-0.05"),
                    GeometryRow::line_to(6, "=Width*0.45", "=Height*0.16"),
                    GeometryRow::arc_to(7, "=Width*0.85", "0 in", "=MIN(Width,Height)*0.05"),
                ];
                sh.sheet.add_geometry(g);
            }
            "CurlyBraceRight" => {
                let mut g = GeometrySection::new(1);
                g.no_fill = "TRUE".into();
                g.rows = vec![
                    GeometryRow::move_to(1, "=Width*0.15", "=Height"),
                    GeometryRow::arc_to(2, "=Width*0.55", "=Height*0.84", "=MIN(Width,Height)*-0.05"),
                    GeometryRow::line_to(3, "=Width*0.55", "=Height*0.58"),
                    GeometryRow::arc_to(4, "=Width*0.90", "=Height*0.50", "=MIN(Width,Height)*0.05"),
                    GeometryRow::arc_to(5, "=Width*0.55", "=Height*0.42", "=MIN(Width,Height)*0.05"),
                    GeometryRow::line_to(6, "=Width*0.55", "=Height*0.16"),
                    GeometryRow::arc_to(7, "=Width*0.15", "0 in", "=MIN(Width,Height)*-0.05"),
                ];
                sh.sheet.add_geometry(g);
            }
            "BannerRibbon" => {
                let mut g = GeometrySection::new(1);
                g.rows = vec![
                    GeometryRow::move_to(1, "0 in", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.88", "=Height*0.5"),
                    GeometryRow::line_to(4, "=Width", "=Height"),
                    GeometryRow::line_to(5, "0 in", "=Height"),
                    GeometryRow::line_to(6, "=Width*0.12", "=Height*0.5"),
                    GeometryRow::line_to(7, "0 in", "0 in"),
                ];
                sh.sheet.add_geometry(g);
            }
            _ => {}
        }
        apply_domain_master_styling_and_details(&mut sh, category, role, geom_kind);
        attach_standard_cardinal_ports(&mut sh);

        masters.push(VisioMaster {
            id: mid,
            name: name.into(),
            name_u: name.into(),
            base_id: format!("{{VEXIO-{category}-{mid:04}}}"),
            unique_id: format!("{{VEXIO-U-{category}-{mid:04}}}"),
            prompt: format!("{title}: {name}"),
            icon_bytes: None,
            page_sheet: ShapeSheet::new(),
            shapes: vec![sh],
        });
    }
    StencilPack {
        title: title.into(),
        category: category.into(),
        masters,
    }
}

fn apply_domain_master_styling_and_details(
    sh: &mut VisioShape,
    category: &str,
    role: &str,
    geom_kind: &str,
) {
    // 0. Authentic 1:1 Multi-Color Visio Stencil Artwork for Microsoft, Servers, Network & Cloud
    if crate::visio_artwork::apply_visio_1to1_icon_artwork(sh, category, role) {
        return;
    }

    // 1. Curated Crisp Visio Light-Mode Domain Palette
    let (fill_rgb, stroke_rgb, weight_pt) = match (category, role) {
        ("Basic Shapes", "basic_star") => ("254,249,195", "202,138,4", 1.65),
        ("Basic Shapes", _) => ("239,246,255", "37,99,235", 1.6),
        ("Arrows", _) => ("219,234,254", "29,78,216", 1.65),
        ("Callouts", "basic_callout") | ("Callouts", "basic_note") => ("254,249,195", "202,138,4", 1.65),
        ("Callouts", "callout_badge") => ("220,252,231", "22,163,74", 1.6),
        ("Callouts", _) => ("248,250,252", "71,85,105", 1.6),
        ("Swimlanes", "swimlane_handoff") => ("219,234,254", "29,78,216", 1.6),
        ("Swimlanes", _) => ("248,250,252", "71,85,105", 1.65),
        ("Process Engineering", _) => ("240,249,255", "3,105,161", 1.65),
        (_, "start")
        | (_, "bpmn_msg_start")
        | (_, "bpmn_signal_start")
        | (_, "bpmn_cond_event") => ("236,253,245", "5,150,105", 1.75),
        (_, "intermediate")
        | (_, "bpmn_timer")
        | (_, "bpmn_catch_msg")
        | (_, "bpmn_throw_msg")
        | (_, "bpmn_error_ev")
        | (_, "bpmn_esc_ev") => ("255,251,235", "217,119,6", 1.65),
        (_, "end") | (_, "bpmn_term_end") => ("254,242,242", "220,38,38", 2.85),
        (_, "exclusive_gateway")
        | (_, "parallel_gateway")
        | (_, "inclusive_gateway")
        | (_, "bpmn_event_gw") => ("254,249,195", "202,138,4", 1.75),
        (_, "task")
        | (_, "bpmn_user_task")
        | (_, "bpmn_service_task")
        | (_, "bpmn_send_task")
        | (_, "bpmn_recv_task")
        | (_, "bpmn_script_task")
        | (_, "bpmn_rule_task")
        | (_, "bpmn_manual_task")
        | (_, "bpmn_subprocess") => ("239,246,255", "37,99,235", 1.6),
        (_, "bpmn_call_act") => ("239,246,255", "29,78,216", 2.6),
        (_, "bpmn_pool") => ("248,250,252", "71,85,105", 1.65),
        ("Flowchart", "Decision") => ("254,243,199", "217,119,6", 1.6),
        ("Flowchart", "Terminator") => ("236,253,245", "13,148,136", 1.6),
        ("Flowchart", "Data") => ("240,249,255", "2,132,199", 1.6),
        ("Flowchart", "Subprocess") => ("238,242,255", "79,70,229", 1.6),
        ("Flowchart", _) => ("239,246,255", "37,99,235", 1.6),
        ("Software & Database", "uml_initial") => ("15,23,42", "15,23,42", 1.5),
        ("Software & Database", "uml_note") => ("254,252,232", "161,98,7", 1.5),
        ("Software & Database", _) => ("245,243,255", "109,40,217", 1.55),
        ("Engineering & Architecture", _) => ("248,250,252", "51,65,85", 1.65),
        ("Electrical", _) => ("255,255,255", "15,23,42", 1.85),
        ("Wireframe", "wf_button") => ("219,234,254", "29,78,216", 1.6),
        ("Wireframe", _) => ("248,250,252", "71,85,105", 1.5),
        ("Business", "org_exec") => ("238,242,255", "49,46,129", 1.75),
        ("Business", "org_vacancy") | ("Business", "org_consultant") => ("255,255,255", "100,116,139", 1.5),
        ("Business", _) => ("240,249,255", "30,64,175", 1.55),
        ("Schedule", "sched_milestone") | ("Schedule", "sched_flag") => ("209,250,229", "5,150,105", 1.75),
        ("Schedule", "sched_rollup") => ("226,232,240", "30,41,59", 1.75),
        ("Schedule", _) => ("219,234,254", "37,99,235", 1.6),
        ("Lean Manufacturing", "vsm_kaizen") => ("254,249,195", "202,138,4", 1.7),
        ("Lean Manufacturing", "vsm_inventory") => ("255,247,237", "217,119,6", 1.65),
        ("Lean Manufacturing", _) => ("241,245,249", "51,65,85", 1.6),
        ("Database", _) => ("239,246,255", "29,78,216", 1.6),
        _ => ("239,246,255", "37,99,235", 1.6),
    };

    sh.sheet
        .set_formula("FillForegnd", format!("=RGB({fill_rgb})"), Some(Unit::Color));
    sh.sheet
        .set_formula("LineColor", format!("=RGB({stroke_rgb})"), Some(Unit::Color));
    sh.sheet.set_formula(
        "LineWeight",
        format!("{} pt", format_clean_f64(weight_pt)),
        Some(Unit::Point),
    );
    sh.sheet
        .set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));

    // Visio 1:1 External Below-Symbol Caption (`TxtPinY = -0.15 in`) for Events, Gateways, Actors & Schematics
    if matches!(category, "Electrical" | "Process Engineering")
        || matches!(
            role,
            "start"
                | "bpmn_msg_start"
                | "bpmn_timer"
                | "bpmn_signal_start"
                | "bpmn_cond_event"
                | "intermediate"
                | "bpmn_catch_msg"
                | "bpmn_throw_msg"
                | "bpmn_error_ev"
                | "bpmn_esc_ev"
                | "end"
                | "bpmn_term_end"
                | "exclusive_gateway"
                | "parallel_gateway"
                | "inclusive_gateway"
                | "bpmn_event_gw"
                | "bpmn_data_obj"
                | "uml_actor"
                | "uml_initial"
                | "uml_final"
                | "uml_flow_final"
                | "flow_onpage"
                | "flow_or_junc"
                | "flow_sum_junc"
                | "sched_milestone"
                | "sched_star"
                | "sched_gate"
        )
    {
        sh.sheet
            .set_formula("TxtPinX", "=Width*0.5", Some(Unit::Inch));
        sh.sheet
            .set_formula("TxtPinY", "-0.15 in", Some(Unit::Inch));
        sh.sheet
            .set_formula("Char.Size", "9 pt", Some(Unit::Point));
    }

    if geom_kind == "Rectangle"
        && matches!(
            role,
            "basic_round_rect"
                | "Process"
                | "task"
                | "bpmn_user_task"
                | "bpmn_service_task"
                | "bpmn_send_task"
                | "bpmn_recv_task"
                | "bpmn_script_task"
                | "bpmn_rule_task"
                | "bpmn_manual_task"
                | "bpmn_subprocess"
                | "bpmn_call_act"
                | "wf_card"
                | "wf_mobile"
                | "wf_modal"
                | "org_exec"
                | "org_board"
                | "org_cxo"
                | "org_vp"
                | "org_sr_mgr"
                | "org_mgr"
                | "org_lead"
                | "org_staff"
                | "org_ic"
                | "org_assoc"
                | "org_consultant"
        )
    {
        sh.sheet.set_formula(
            "Rounding",
            if role == "basic_round_rect" || role == "wf_mobile" {
                "=MIN(Width,Height)*0.16"
            } else {
                "=MIN(Width,Height)*0.10"
            },
            Some(Unit::Inch),
        );
    }
    if matches!(
        role,
        "org_vacancy" | "org_consultant" | "org_matrix" | "vsm_shared" | "uml_boundary" | "callout_dashed_group"
    ) {
        sh.sheet.set_formula("LinePattern", "2", Some(Unit::Number));
    }

    // 2. Domain-Authentic Interior Vector Details (`Geometry2` / Custom `Geometry1`)
    let mut detail = GeometrySection::new(2);
    detail.no_fill = "TRUE".into();

    match role {
        "Subprocess" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.13", "0 in"),
                GeometryRow::line_to(2, "=Width*0.13", "=Height"),
                GeometryRow::move_to(3, "=Width*0.87", "0 in"),
                GeometryRow::line_to(4, "=Width*0.87", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "flow_multidoc" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.08", "=Height"),
                GeometryRow::line_to(2, "=Width*0.08", "=Height*1.08"),
                GeometryRow::line_to(3, "=Width*1.06", "=Height*1.08"),
                GeometryRow::line_to(4, "=Width*1.06", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_msg_start" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.34"),
                GeometryRow::line_to(2, "=Width*0.76", "=Height*0.34"),
                GeometryRow::line_to(3, "=Width*0.76", "=Height*0.66"),
                GeometryRow::line_to(4, "=Width*0.24", "=Height*0.66"),
                GeometryRow::line_to(5, "=Width*0.24", "=Height*0.34"),
                GeometryRow::move_to(6, "=Width*0.24", "=Height*0.66"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.48"),
                GeometryRow::line_to(8, "=Width*0.76", "=Height*0.66"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_timer" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.5", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.5", "=Height*0.5"),
                GeometryRow::line_to(4, "=Width*0.72", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_user_task" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.08", "=Height*0.62"),
                GeometryRow::arc_to(2, "=Width*0.22", "=Height*0.62", "=MIN(Width,Height)*-0.06"),
                GeometryRow::move_to(3, "=Width*0.11", "=Height*0.76"),
                GeometryRow::arc_to(4, "=Width*0.19", "=Height*0.76", "=MIN(Width,Height)*0.04"),
                GeometryRow::arc_to(5, "=Width*0.11", "=Height*0.76", "=MIN(Width,Height)*0.04"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_service_task" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.62"),
                GeometryRow::line_to(2, "=Width*0.14", "=Height*0.86"),
                GeometryRow::move_to(3, "=Width*0.07", "=Height*0.74"),
                GeometryRow::line_to(4, "=Width*0.21", "=Height*0.74"),
                GeometryRow::move_to(5, "=Width*0.09", "=Height*0.66"),
                GeometryRow::line_to(6, "=Width*0.19", "=Height*0.82"),
                GeometryRow::move_to(7, "=Width*0.09", "=Height*0.82"),
                GeometryRow::line_to(8, "=Width*0.19", "=Height*0.66"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_subprocess" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.42", "=Height*0.06"),
                GeometryRow::line_to(2, "=Width*0.58", "=Height*0.06"),
                GeometryRow::line_to(3, "=Width*0.58", "=Height*0.28"),
                GeometryRow::line_to(4, "=Width*0.42", "=Height*0.28"),
                GeometryRow::line_to(5, "=Width*0.42", "=Height*0.06"),
                GeometryRow::move_to(6, "=Width*0.50", "=Height*0.10"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.24"),
                GeometryRow::move_to(8, "=Width*0.45", "=Height*0.17"),
                GeometryRow::line_to(9, "=Width*0.55", "=Height*0.17"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_pool" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.10", "0 in"),
                GeometryRow::line_to(2, "=Width*0.10", "=Height"),
                GeometryRow::move_to(3, "=Width*0.10", "=Height*0.5"),
                GeometryRow::line_to(4, "=Width", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "intermediate" | "uml_final" => {
            detail.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.5".into(),
                a: Some("=Width*0.82".into()),
                b: Some("=Height*0.5".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height*0.82".into()),
                e: None,
                deleted: false,
            }];
            sh.sheet.add_geometry(detail);
        }
        "inclusive_gateway" => {
            detail.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.5".into(),
                a: Some("=Width*0.72".into()),
                b: Some("=Height*0.5".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height*0.72".into()),
                e: None,
                deleted: false,
            }];
            sh.sheet.add_geometry(detail);
        }
        "exclusive_gateway" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.36", "=Height*0.36"),
                GeometryRow::line_to(2, "=Width*0.64", "=Height*0.64"),
                GeometryRow::move_to(3, "=Width*0.64", "=Height*0.36"),
                GeometryRow::line_to(4, "=Width*0.36", "=Height*0.64"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "parallel_gateway" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height*0.28"),
                GeometryRow::line_to(2, "=Width*0.5", "=Height*0.72"),
                GeometryRow::move_to(3, "=Width*0.28", "=Height*0.5"),
                GeometryRow::line_to(4, "=Width*0.72", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "uml_class" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.72"),
                GeometryRow::line_to(2, "=Width", "=Height*0.72"),
                GeometryRow::move_to(3, "0 in", "=Height*0.36"),
                GeometryRow::line_to(4, "=Width", "=Height*0.36"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "uml_interface" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.65"),
                GeometryRow::line_to(2, "=Width", "=Height*0.65"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "uml_component" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.78", "=Height*0.82"),
                GeometryRow::line_to(2, "=Width*0.92", "=Height*0.82"),
                GeometryRow::line_to(3, "=Width*0.92", "=Height*0.64"),
                GeometryRow::line_to(4, "=Width*0.78", "=Height*0.64"),
                GeometryRow::line_to(5, "=Width*0.78", "=Height*0.82"),
                GeometryRow::move_to(6, "=Width*0.73", "=Height*0.78"),
                GeometryRow::line_to(7, "=Width*0.82", "=Height*0.78"),
                GeometryRow::move_to(8, "=Width*0.73", "=Height*0.68"),
                GeometryRow::line_to(9, "=Width*0.82", "=Height*0.68"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "uml_lifeline" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.1", "=Height*0.74"),
                GeometryRow::line_to(2, "=Width*0.9", "=Height*0.74"),
                GeometryRow::line_to(3, "=Width*0.9", "=Height"),
                GeometryRow::line_to(4, "=Width*0.1", "=Height"),
                GeometryRow::line_to(5, "=Width*0.1", "=Height*0.74"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height*0.74"),
                GeometryRow::line_to(2, "=Width*0.5", "0 in"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "rack_42u" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.12", "0 in"),
                GeometryRow::line_to(2, "=Width*0.12", "=Height"),
                GeometryRow::move_to(3, "=Width*0.88", "0 in"),
                GeometryRow::line_to(4, "=Width*0.88", "=Height"),
                GeometryRow::move_to(5, "=Width*0.12", "=Height*0.78"),
                GeometryRow::line_to(6, "=Width*0.88", "=Height*0.78"),
                GeometryRow::move_to(7, "=Width*0.12", "=Height*0.56"),
                GeometryRow::line_to(8, "=Width*0.88", "=Height*0.56"),
                GeometryRow::move_to(9, "=Width*0.12", "=Height*0.34"),
                GeometryRow::line_to(10, "=Width*0.88", "=Height*0.34"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_router" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.44", "=Height*0.5"),
                GeometryRow::move_to(3, "=Width*0.56", "=Height*0.5"),
                GeometryRow::line_to(4, "=Width*0.78", "=Height*0.5"),
                GeometryRow::move_to(5, "=Width*0.5", "=Height*0.22"),
                GeometryRow::line_to(6, "=Width*0.5", "=Height*0.44"),
                GeometryRow::move_to(7, "=Width*0.5", "=Height*0.56"),
                GeometryRow::line_to(8, "=Width*0.5", "=Height*0.78"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_lb" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.48", "=Height*0.5"),
                GeometryRow::line_to(3, "=Width*0.76", "=Height*0.74"),
                GeometryRow::move_to(4, "=Width*0.48", "=Height*0.5"),
                GeometryRow::line_to(5, "=Width*0.76", "=Height*0.5"),
                GeometryRow::move_to(6, "=Width*0.48", "=Height*0.5"),
                GeometryRow::line_to(7, "=Width*0.76", "=Height*0.26"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_wifi" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.26", "=Height*0.56"),
                GeometryRow::arc_to(2, "=Width*0.74", "=Height*0.56", "=MIN(Width,Height)*0.14"),
                GeometryRow::move_to(3, "=Width*0.35", "=Height*0.42"),
                GeometryRow::arc_to(4, "=Width*0.65", "=Height*0.42", "=MIN(Width,Height)*0.09"),
                GeometryRow::move_to(5, "=Width*0.47", "=Height*0.26"),
                GeometryRow::line_to(6, "=Width*0.53", "=Height*0.26"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_laptop" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.24"),
                GeometryRow::line_to(2, "=Width*0.88", "=Height*0.24"),
                GeometryRow::line_to(3, "=Width*0.88", "=Height*0.92"),
                GeometryRow::line_to(4, "=Width*0.12", "=Height*0.92"),
                GeometryRow::line_to(5, "=Width*0.12", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.10"),
                GeometryRow::line_to(2, "=Width", "=Height*0.10"),
                GeometryRow::line_to(3, "=Width*0.88", "=Height*0.24"),
                GeometryRow::line_to(4, "=Width*0.12", "=Height*0.24"),
                GeometryRow::line_to(5, "0 in", "=Height*0.10"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "switch_1u" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.15", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.42", "=Height*0.5"),
                GeometryRow::move_to(3, "=Width*0.58", "=Height*0.3"),
                GeometryRow::line_to(4, "=Width*0.85", "=Height*0.7"),
                GeometryRow::move_to(5, "=Width*0.58", "=Height*0.7"),
                GeometryRow::line_to(6, "=Width*0.85", "=Height*0.3"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "firewall" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width", "=Height*0.5"),
                GeometryRow::move_to(3, "=Width*0.33", "=Height*0.5"),
                GeometryRow::line_to(4, "=Width*0.33", "=Height"),
                GeometryRow::move_to(5, "=Width*0.67", "=Height*0.5"),
                GeometryRow::line_to(6, "=Width*0.67", "=Height"),
                GeometryRow::move_to(7, "=Width*0.5", "0 in"),
                GeometryRow::line_to(8, "=Width*0.5", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "db_cluster" | "cloud_sql" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.16"),
                GeometryRow::line_to(2, "=Width", "=Height*0.16"),
                GeometryRow::line_to(3, "=Width", "=Height*0.84"),
                GeometryRow::line_to(4, "0 in", "=Height*0.84"),
                GeometryRow::line_to(5, "0 in", "=Height*0.16"),
            ];
            sh.sheet.add_geometry(g1);
            let mut cap = GeometrySection::new(2);
            cap.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.84".into(),
                a: Some("=Width".into()),
                b: Some("=Height*0.84".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height".into()),
                e: None,
                deleted: false,
            }];
            sh.sheet.add_geometry(cap);
            let mut base_arc = GeometrySection::new(3);
            base_arc.no_fill = "TRUE".into();
            base_arc.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.52"),
                GeometryRow::line_to(2, "=Width", "=Height*0.52"),
            ];
            sh.sheet.add_geometry(base_arc);
        }
        "door" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "0 in", "=Height"),
                GeometryRow::arc_to(3, "=Width", "0 in", "=MIN(Width,Height)*0.18"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "door_double" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "0 in", "=Height"),
                GeometryRow::arc_to(3, "=Width*0.5", "0 in", "=MIN(Width,Height)*0.14"),
                GeometryRow::move_to(4, "=Width", "0 in"),
                GeometryRow::line_to(5, "=Width", "=Height"),
                GeometryRow::arc_to(6, "=Width*0.5", "0 in", "=MIN(Width,Height)*-0.14"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "arch_window" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width", "=Height*0.5"),
                GeometryRow::move_to(3, "=Width*0.33", "0 in"),
                GeometryRow::line_to(4, "=Width*0.33", "=Height"),
                GeometryRow::move_to(5, "=Width*0.67", "0 in"),
                GeometryRow::line_to(6, "=Width*0.67", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "arch_stairs" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.20", "0 in"),
                GeometryRow::line_to(2, "=Width*0.20", "=Height"),
                GeometryRow::move_to(3, "=Width*0.40", "0 in"),
                GeometryRow::line_to(4, "=Width*0.40", "=Height"),
                GeometryRow::move_to(5, "=Width*0.60", "0 in"),
                GeometryRow::line_to(6, "=Width*0.60", "=Height"),
                GeometryRow::move_to(7, "=Width*0.80", "0 in"),
                GeometryRow::line_to(8, "=Width*0.80", "=Height"),
                GeometryRow::move_to(9, "=Width*0.10", "=Height*0.5"),
                GeometryRow::line_to(10, "=Width*0.90", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pid_pump" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height"),
                GeometryRow::line_to(2, "=Width", "=Height"),
                GeometryRow::move_to(3, "=Width*0.2", "0 in"),
                GeometryRow::line_to(4, "=Width*0.8", "0 in"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pid_heatex" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.25", "=Height*0.5"),
                GeometryRow::line_to(3, "=Width*0.40", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.60", "=Height*0.22"),
                GeometryRow::line_to(5, "=Width*0.75", "=Height*0.5"),
                GeometryRow::line_to(6, "=Width", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pid_valve" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "0 in", "=Height*0.75"),
                GeometryRow::line_to(3, "=Width*0.5", "=Height*0.38"),
                GeometryRow::line_to(4, "=Width", "=Height*0.75"),
                GeometryRow::line_to(5, "=Width", "0 in"),
                GeometryRow::line_to(6, "=Width*0.5", "=Height*0.38"),
                GeometryRow::line_to(7, "0 in", "0 in"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height*0.38"),
                GeometryRow::line_to(2, "=Width*0.5", "=Height"),
                GeometryRow::move_to(3, "=Width*0.28", "=Height"),
                GeometryRow::line_to(4, "=Width*0.72", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "cloud_k8s" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height*0.2"),
                GeometryRow::line_to(2, "=Width*0.5", "=Height*0.8"),
                GeometryRow::move_to(3, "=Width*0.24", "=Height*0.35"),
                GeometryRow::line_to(4, "=Width*0.76", "=Height*0.65"),
                GeometryRow::move_to(5, "=Width*0.24", "=Height*0.65"),
                GeometryRow::line_to(6, "=Width*0.76", "=Height*0.35"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "cloud_fn" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.55", "=Height*0.8"),
                GeometryRow::line_to(2, "=Width*0.36", "=Height*0.48"),
                GeometryRow::line_to(3, "=Width*0.54", "=Height*0.48"),
                GeometryRow::line_to(4, "=Width*0.42", "=Height*0.2"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "cloud_apigw" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.30", "=Height*0.25"),
                GeometryRow::line_to(2, "=Width*0.30", "=Height*0.75"),
                GeometryRow::move_to(3, "=Width*0.70", "=Height*0.25"),
                GeometryRow::line_to(4, "=Width*0.70", "=Height*0.75"),
                GeometryRow::move_to(5, "=Width*0.30", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width*0.70", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ieee_capacitor" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.42", "=Height*0.5"),
                GeometryRow::move_to(3, "=Width*0.42", "=Height*0.12"),
                GeometryRow::line_to(4, "=Width*0.42", "=Height*0.88"),
                GeometryRow::move_to(5, "=Width*0.58", "=Height*0.12"),
                GeometryRow::line_to(6, "=Width*0.58", "=Height*0.88"),
                GeometryRow::move_to(7, "=Width*0.58", "=Height*0.5"),
                GeometryRow::line_to(8, "=Width", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "ieee_diode" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.28", "=Height*0.14"),
                GeometryRow::line_to(2, "=Width*0.70", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.28", "=Height*0.86"),
                GeometryRow::line_to(4, "=Width*0.28", "=Height*0.14"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.28", "=Height*0.50"),
                GeometryRow::move_to(3, "=Width*0.70", "=Height*0.14"),
                GeometryRow::line_to(4, "=Width*0.70", "=Height*0.86"),
                GeometryRow::move_to(5, "=Width*0.70", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ieee_npn" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.10", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.38", "=Height*0.50"),
                GeometryRow::move_to(3, "=Width*0.38", "=Height*0.24"),
                GeometryRow::line_to(4, "=Width*0.38", "=Height*0.76"),
                GeometryRow::move_to(5, "=Width*0.38", "=Height*0.62"),
                GeometryRow::line_to(6, "=Width*0.76", "=Height*0.86"),
                GeometryRow::move_to(7, "=Width*0.38", "=Height*0.38"),
                GeometryRow::line_to(8, "=Width*0.76", "=Height*0.14"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ieee_switch" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.35"),
                GeometryRow::line_to(2, "=Width*0.30", "=Height*0.35"),
                GeometryRow::line_to(3, "=Width*0.68", "=Height*0.88"),
                GeometryRow::move_to(4, "=Width*0.70", "=Height*0.35"),
                GeometryRow::line_to(5, "=Width", "=Height*0.35"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "ieee_battery" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.34", "=Height*0.50"),
                GeometryRow::move_to(3, "=Width*0.34", "=Height*0.10"),
                GeometryRow::line_to(4, "=Width*0.34", "=Height*0.90"),
                GeometryRow::move_to(5, "=Width*0.46", "=Height*0.28"),
                GeometryRow::line_to(6, "=Width*0.46", "=Height*0.72"),
                GeometryRow::move_to(7, "=Width*0.58", "=Height*0.10"),
                GeometryRow::line_to(8, "=Width*0.58", "=Height*0.90"),
                GeometryRow::move_to(9, "=Width*0.70", "=Height*0.28"),
                GeometryRow::line_to(10, "=Width*0.70", "=Height*0.72"),
                GeometryRow::move_to(11, "=Width*0.70", "=Height*0.50"),
                GeometryRow::line_to(12, "=Width", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "ieee_inductor" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.35"),
                GeometryRow::line_to(2, "=Width*0.14", "=Height*0.35"),
                GeometryRow::arc_to(3, "=Width*0.32", "=Height*0.35", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(4, "=Width*0.50", "=Height*0.35", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(5, "=Width*0.68", "=Height*0.35", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(6, "=Width*0.86", "=Height*0.35", "=MIN(Width,Height)*0.14"),
                GeometryRow::line_to(7, "=Width", "=Height*0.35"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "ieee_ground" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height"),
                GeometryRow::line_to(2, "=Width*0.5", "=Height*0.48"),
                GeometryRow::move_to(3, "=Width*0.1", "=Height*0.48"),
                GeometryRow::line_to(4, "=Width*0.9", "=Height*0.48"),
                GeometryRow::move_to(5, "=Width*0.24", "=Height*0.26"),
                GeometryRow::line_to(6, "=Width*0.76", "=Height*0.26"),
                GeometryRow::move_to(7, "=Width*0.38", "=Height*0.06"),
                GeometryRow::line_to(8, "=Width*0.62", "=Height*0.06"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "ieee_opamp" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.15", "=Height*0.08"),
                GeometryRow::line_to(2, "=Width*0.88", "=Height*0.5"),
                GeometryRow::line_to(3, "=Width*0.15", "=Height*0.92"),
                GeometryRow::line_to(4, "=Width*0.15", "=Height*0.08"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.68"),
                GeometryRow::line_to(2, "=Width*0.15", "=Height*0.68"),
                GeometryRow::move_to(3, "0 in", "=Height*0.32"),
                GeometryRow::line_to(4, "=Width*0.15", "=Height*0.32"),
                GeometryRow::move_to(5, "=Width*0.88", "=Height*0.5"),
                GeometryRow::line_to(6, "=Width", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ieee_ac_source" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.5"),
                GeometryRow::arc_to(2, "=Width*0.50", "=Height*0.5", "=MIN(Width,Height)*0.12"),
                GeometryRow::arc_to(3, "=Width*0.78", "=Height*0.5", "=MIN(Width,Height)*-0.12"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_window" | "wf_modal" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.82"),
                GeometryRow::line_to(2, "=Width", "=Height*0.82"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_mobile" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.88"),
                GeometryRow::line_to(2, "=Width", "=Height*0.88"),
                GeometryRow::move_to(3, "0 in", "=Height*0.10"),
                GeometryRow::line_to(4, "=Width", "=Height*0.10"),
                GeometryRow::move_to(5, "=Width*0.36", "=Height*0.05"),
                GeometryRow::line_to(6, "=Width*0.64", "=Height*0.05"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_table" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.72"),
                GeometryRow::line_to(2, "=Width", "=Height*0.72"),
                GeometryRow::move_to(3, "0 in", "=Height*0.38"),
                GeometryRow::line_to(4, "=Width", "=Height*0.38"),
                GeometryRow::move_to(5, "=Width*0.34", "0 in"),
                GeometryRow::line_to(6, "=Width*0.34", "=Height"),
                GeometryRow::move_to(7, "=Width*0.68", "0 in"),
                GeometryRow::line_to(8, "=Width*0.68", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "org_exec" | "org_vp" | "org_mgr" | "org_ic" | "org_consultant" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width", "=Height*0.76"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "sched_rollup" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.1"),
                GeometryRow::line_to(2, "=Width*0.08", "=Height*0.55"),
                GeometryRow::line_to(3, "=Width*0.92", "=Height*0.55"),
                GeometryRow::line_to(4, "=Width", "=Height*0.1"),
                GeometryRow::line_to(5, "=Width", "=Height"),
                GeometryRow::line_to(6, "0 in", "=Height"),
                GeometryRow::line_to(7, "0 in", "=Height*0.1"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "sched_flag" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.44"),
                GeometryRow::line_to(2, "=Width*0.92", "=Height*0.68"),
                GeometryRow::line_to(3, "=Width*0.18", "=Height*0.94"),
                GeometryRow::line_to(4, "=Width*0.18", "=Height*0.44"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "0 in"),
                GeometryRow::line_to(2, "=Width*0.18", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "vsm_customer" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "=Width", "0 in"),
                GeometryRow::line_to(3, "=Width", "=Height"),
                GeometryRow::line_to(4, "=Width*0.67", "=Height*0.76"),
                GeometryRow::line_to(5, "=Width*0.67", "=Height"),
                GeometryRow::line_to(6, "=Width*0.33", "=Height*0.76"),
                GeometryRow::line_to(7, "=Width*0.33", "=Height"),
                GeometryRow::line_to(8, "0 in", "=Height*0.76"),
                GeometryRow::line_to(9, "0 in", "0 in"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "vsm_supermarket" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height"),
                GeometryRow::line_to(2, "=Width", "=Height"),
                GeometryRow::line_to(3, "=Width", "0 in"),
                GeometryRow::line_to(4, "0 in", "0 in"),
                GeometryRow::move_to(5, "0 in", "=Height*0.67"),
                GeometryRow::line_to(6, "=Width", "=Height*0.67"),
                GeometryRow::move_to(7, "0 in", "=Height*0.33"),
                GeometryRow::line_to(8, "=Width", "=Height*0.33"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "vsm_fifo" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.88", "=Height*0.5"),
                GeometryRow::line_to(3, "=Width*0.74", "=Height*0.78"),
                GeometryRow::move_to(4, "=Width*0.88", "=Height*0.5"),
                GeometryRow::line_to(5, "=Width*0.74", "=Height*0.22"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "vsm_truck" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.22"),
                GeometryRow::line_to(2, "=Width*0.68", "=Height*0.22"),
                GeometryRow::line_to(3, "=Width*0.68", "=Height*0.92"),
                GeometryRow::line_to(4, "0 in", "=Height*0.92"),
                GeometryRow::line_to(5, "0 in", "=Height*0.22"),
                GeometryRow::move_to(6, "=Width*0.68", "=Height*0.22"),
                GeometryRow::line_to(7, "=Width", "=Height*0.22"),
                GeometryRow::line_to(8, "=Width", "=Height*0.56"),
                GeometryRow::line_to(9, "=Width*0.84", "=Height*0.76"),
                GeometryRow::line_to(10, "=Width*0.68", "=Height*0.76"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.14"),
                GeometryRow::arc_to(2, "=Width*0.34", "=Height*0.14", "=MIN(Width,Height)*0.09"),
                GeometryRow::arc_to(3, "=Width*0.16", "=Height*0.14", "=MIN(Width,Height)*0.09"),
                GeometryRow::move_to(4, "=Width*0.74", "=Height*0.14"),
                GeometryRow::arc_to(5, "=Width*0.92", "=Height*0.14", "=MIN(Width,Height)*0.09"),
                GeometryRow::arc_to(6, "=Width*0.74", "=Height*0.14", "=MIN(Width,Height)*0.09"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ieee_transformer" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                // Left primary winding + leads
                GeometryRow::move_to(1, "0 in", "=Height*0.85"),
                GeometryRow::line_to(2, "=Width*0.28", "=Height*0.85"),
                GeometryRow::arc_to(3, "=Width*0.28", "=Height*0.62", "=MIN(Width,Height)*-0.12"),
                GeometryRow::arc_to(4, "=Width*0.28", "=Height*0.38", "=MIN(Width,Height)*-0.12"),
                GeometryRow::arc_to(5, "=Width*0.28", "=Height*0.15", "=MIN(Width,Height)*-0.12"),
                GeometryRow::line_to(6, "0 in", "=Height*0.15"),
                // Core bars
                GeometryRow::move_to(7, "=Width*0.46", "=Height*0.14"),
                GeometryRow::line_to(8, "=Width*0.46", "=Height*0.86"),
                GeometryRow::move_to(9, "=Width*0.54", "=Height*0.14"),
                GeometryRow::line_to(10, "=Width*0.54", "=Height*0.86"),
                // Right secondary winding + leads
                GeometryRow::move_to(11, "=Width", "=Height*0.85"),
                GeometryRow::line_to(12, "=Width*0.72", "=Height*0.85"),
                GeometryRow::arc_to(13, "=Width*0.72", "=Height*0.62", "=MIN(Width,Height)*0.12"),
                GeometryRow::arc_to(14, "=Width*0.72", "=Height*0.38", "=MIN(Width,Height)*0.12"),
                GeometryRow::arc_to(15, "=Width*0.72", "=Height*0.15", "=MIN(Width,Height)*0.12"),
                GeometryRow::line_to(16, "=Width", "=Height*0.15"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "wall" | "wall_inner" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "cloud_vm" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.22"),
                GeometryRow::line_to(2, "=Width*0.78", "=Height*0.22"),
                GeometryRow::line_to(3, "=Width*0.78", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.22", "=Height*0.78"),
                GeometryRow::line_to(5, "=Width*0.22", "=Height*0.22"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_card" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.28"),
                GeometryRow::line_to(2, "=Width*0.38", "=Height*0.52"),
                GeometryRow::line_to(3, "=Width*0.58", "=Height*0.38"),
                GeometryRow::line_to(4, "=Width*0.86", "=Height*0.74"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_appbar" | "wf_input" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.24", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_navrail" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.25", "=Height*0.8"),
                GeometryRow::line_to(2, "=Width*0.75", "=Height*0.8"),
                GeometryRow::move_to(3, "=Width*0.25", "=Height*0.55"),
                GeometryRow::line_to(4, "=Width*0.75", "=Height*0.55"),
                GeometryRow::move_to(5, "=Width*0.25", "=Height*0.3"),
                GeometryRow::line_to(6, "=Width*0.75", "=Height*0.3"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "vsm_inventory" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.5", "=Height*0.2"),
                GeometryRow::line_to(2, "=Width*0.5", "=Height*0.58"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "vsm_kaizen" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.05", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.24", "=Height*0.68"),
                GeometryRow::line_to(3, "=Width*0.20", "=Height*0.95"),
                GeometryRow::line_to(4, "=Width*0.46", "=Height*0.78"),
                GeometryRow::line_to(5, "=Width*0.64", "=Height*0.98"),
                GeometryRow::line_to(6, "=Width*0.72", "=Height*0.72"),
                GeometryRow::line_to(7, "=Width*0.96", "=Height*0.64"),
                GeometryRow::line_to(8, "=Width*0.78", "=Height*0.42"),
                GeometryRow::line_to(9, "=Width*0.92", "=Height*0.14"),
                GeometryRow::line_to(10, "=Width*0.60", "=Height*0.24"),
                GeometryRow::line_to(11, "=Width*0.40", "=Height*0.04"),
                GeometryRow::line_to(12, "=Width*0.28", "=Height*0.30"),
                GeometryRow::line_to(13, "=Width*0.05", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "vsm_process" | "vsm_dedicated" | "vsm_shared" | "erd_strong" | "erd_weak" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.7"),
                GeometryRow::line_to(2, "=Width", "=Height*0.7"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "erd_pk" | "erd_fk" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.22", "0 in"),
                GeometryRow::line_to(2, "=Width*0.22", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "callout_container" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.80"),
                GeometryRow::line_to(2, "=Width", "=Height*0.80"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "callout_header" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.18"),
                GeometryRow::line_to(2, "=Width", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "swimlane_horiz" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.14", "0 in"),
                GeometryRow::line_to(2, "=Width*0.14", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "swimlane_vert" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.84"),
                GeometryRow::line_to(2, "=Width", "=Height*0.84"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "swimlane_phase" => {
            sh.sheet.set_formula("LinePattern", "2", Some(Unit::Number));
        }
        "arch_desk" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "0 in"),
                GeometryRow::line_to(2, "=Width*0.18", "=Height*0.28"),
                GeometryRow::line_to(3, "=Width*0.82", "=Height*0.28"),
                GeometryRow::line_to(4, "=Width*0.82", "0 in"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "arch_elevator" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "=Width", "=Height"),
                GeometryRow::move_to(3, "0 in", "=Height"),
                GeometryRow::line_to(4, "=Width", "0 in"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pid_check_valve" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "0 in", "=Height"),
                GeometryRow::line_to(3, "=Width", "0 in"),
                GeometryRow::line_to(4, "=Width", "=Height"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "pid_tank" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.84"),
                GeometryRow::arc_to(2, "=Width", "=Height*0.84", "=MIN(Width,Height)*-0.12"),
                GeometryRow::move_to(3, "0 in", "=Height*0.16"),
                GeometryRow::arc_to(4, "=Width", "=Height*0.16", "=MIN(Width,Height)*0.10"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pid_vessel" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "0 in"),
                GeometryRow::line_to(2, "=Width*0.18", "=Height"),
                GeometryRow::move_to(3, "=Width*0.82", "0 in"),
                GeometryRow::line_to(4, "=Width*0.82", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pid_flowmeter" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width", "=Height*0.5"),
                GeometryRow::move_to(3, "=Width*0.32", "=Height*0.72"),
                GeometryRow::arc_to(4, "=Width*0.68", "=Height*0.72", "=MIN(Width,Height)*0.06"),
                GeometryRow::move_to(5, "=Width*0.32", "=Height*0.28"),
                GeometryRow::arc_to(6, "=Width*0.68", "=Height*0.28", "=MIN(Width,Height)*-0.06"),
            ];
            sh.sheet.add_geometry(detail);
        }
        // --- Microsoft 365 & Office (14 high-definition multi-geometry vector masters) ---
        "m365_tenant" => {
            detail.rows = vec![
                // Top tenant banner divider + 4-quadrant window emblem
                GeometryRow::move_to(1, "0 in", "=Height*0.74"),
                GeometryRow::line_to(2, "=Width", "=Height*0.74"),
                GeometryRow::move_to(3, "=Width*0.38", "=Height*0.16"),
                GeometryRow::line_to(4, "=Width*0.62", "=Height*0.16"),
                GeometryRow::line_to(5, "=Width*0.62", "=Height*0.58"),
                GeometryRow::line_to(6, "=Width*0.38", "=Height*0.58"),
                GeometryRow::line_to(7, "=Width*0.38", "=Height*0.16"),
                GeometryRow::move_to(8, "=Width*0.50", "=Height*0.16"),
                GeometryRow::line_to(9, "=Width*0.50", "=Height*0.58"),
                GeometryRow::move_to(10, "=Width*0.38", "=Height*0.37"),
                GeometryRow::line_to(11, "=Width*0.62", "=Height*0.37"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_teams" => {
            detail.rows = vec![
                // Left badge with 'T' + dual collaborator silhouettes
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.28"),
                GeometryRow::line_to(2, "=Width*0.44", "=Height*0.28"),
                GeometryRow::line_to(3, "=Width*0.44", "=Height*0.72"),
                GeometryRow::line_to(4, "=Width*0.14", "=Height*0.72"),
                GeometryRow::line_to(5, "=Width*0.14", "=Height*0.28"),
                GeometryRow::move_to(6, "=Width*0.21", "=Height*0.60"),
                GeometryRow::line_to(7, "=Width*0.37", "=Height*0.60"),
                GeometryRow::move_to(8, "=Width*0.29", "=Height*0.60"),
                GeometryRow::line_to(9, "=Width*0.29", "=Height*0.38"),
                // Primary & secondary user heads/shoulders
                GeometryRow::move_to(10, "=Width*0.52", "=Height*0.24"),
                GeometryRow::arc_to(11, "=Width*0.84", "=Height*0.24", "=MIN(Width,Height)*-0.12"),
                GeometryRow::move_to(12, "=Width*0.60", "=Height*0.66"),
                GeometryRow::arc_to(13, "=Width*0.76", "=Height*0.66", "=MIN(Width,Height)*0.08"),
                GeometryRow::arc_to(14, "=Width*0.60", "=Height*0.66", "=MIN(Width,Height)*0.08"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_exchange" => {
            detail.rows = vec![
                // Diamond routing shield + internal mail chevron
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.12"),
                GeometryRow::line_to(2, "=Width*0.84", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.50", "=Height*0.88"),
                GeometryRow::line_to(4, "=Width*0.16", "=Height*0.50"),
                GeometryRow::line_to(5, "=Width*0.50", "=Height*0.12"),
                GeometryRow::move_to(6, "=Width*0.32", "=Height*0.38"),
                GeometryRow::line_to(7, "=Width*0.68", "=Height*0.38"),
                GeometryRow::line_to(8, "=Width*0.68", "=Height*0.62"),
                GeometryRow::line_to(9, "=Width*0.32", "=Height*0.62"),
                GeometryRow::line_to(10, "=Width*0.32", "=Height*0.38"),
                GeometryRow::move_to(11, "=Width*0.32", "=Height*0.62"),
                GeometryRow::line_to(12, "=Width*0.50", "=Height*0.48"),
                GeometryRow::line_to(13, "=Width*0.68", "=Height*0.62"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_sharepoint" => {
            detail.rows = vec![
                // Overlapping SharePoint portal circles
                GeometryRow::move_to(1, "=Width*0.20", "=Height*0.62"),
                GeometryRow::arc_to(2, "=Width*0.60", "=Height*0.62", "=MIN(Width,Height)*0.18"),
                GeometryRow::arc_to(3, "=Width*0.20", "=Height*0.62", "=MIN(Width,Height)*0.18"),
                GeometryRow::move_to(4, "=Width*0.44", "=Height*0.38"),
                GeometryRow::arc_to(5, "=Width*0.80", "=Height*0.38", "=MIN(Width,Height)*0.16"),
                GeometryRow::arc_to(6, "=Width*0.44", "=Height*0.38", "=MIN(Width,Height)*0.16"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_onedrive" => {
            detail.rows = vec![
                // Dual-cloud contour silhouette inside OneDrive pill
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.28"),
                GeometryRow::line_to(2, "=Width*0.82", "=Height*0.28"),
                GeometryRow::arc_to(3, "=Width*0.70", "=Height*0.62", "=MIN(Width,Height)*0.10"),
                GeometryRow::arc_to(4, "=Width*0.40", "=Height*0.68", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(5, "=Width*0.18", "=Height*0.28", "=MIN(Width,Height)*0.11"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_outlook" => {
            detail.rows = vec![
                // Mail envelope + calendar sidebar
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.20"),
                GeometryRow::line_to(2, "=Width*0.86", "=Height*0.20"),
                GeometryRow::line_to(3, "=Width*0.86", "=Height*0.76"),
                GeometryRow::line_to(4, "=Width*0.14", "=Height*0.76"),
                GeometryRow::line_to(5, "=Width*0.14", "=Height*0.20"),
                GeometryRow::move_to(6, "=Width*0.14", "=Height*0.76"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.44"),
                GeometryRow::line_to(8, "=Width*0.86", "=Height*0.76"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "office_word" => {
            detail.rows = vec![
                // 'W' monogram + ruled document lines
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.78"),
                GeometryRow::line_to(2, "=Width*0.25", "=Height*0.54"),
                GeometryRow::line_to(3, "=Width*0.32", "=Height*0.70"),
                GeometryRow::line_to(4, "=Width*0.39", "=Height*0.54"),
                GeometryRow::line_to(5, "=Width*0.46", "=Height*0.78"),
                GeometryRow::move_to(6, "=Width*0.18", "=Height*0.38"),
                GeometryRow::line_to(7, "=Width*0.82", "=Height*0.38"),
                GeometryRow::move_to(8, "=Width*0.18", "=Height*0.24"),
                GeometryRow::line_to(9, "=Width*0.82", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "office_excel" => {
            detail.rows = vec![
                // 'X' emblem + 2x3 spreadsheet cell grid
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.78"),
                GeometryRow::line_to(2, "=Width*0.38", "=Height*0.56"),
                GeometryRow::move_to(3, "=Width*0.38", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.18", "=Height*0.56"),
                GeometryRow::move_to(5, "=Width*0.18", "=Height*0.44"),
                GeometryRow::line_to(6, "=Width*0.82", "=Height*0.44"),
                GeometryRow::line_to(7, "=Width*0.82", "=Height*0.16"),
                GeometryRow::line_to(8, "=Width*0.18", "=Height*0.16"),
                GeometryRow::line_to(9, "=Width*0.18", "=Height*0.44"),
                GeometryRow::move_to(10, "=Width*0.18", "=Height*0.30"),
                GeometryRow::line_to(11, "=Width*0.82", "=Height*0.30"),
                GeometryRow::move_to(12, "=Width*0.50", "=Height*0.16"),
                GeometryRow::line_to(13, "=Width*0.50", "=Height*0.44"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "office_powerpoint" => {
            detail.rows = vec![
                // Slide header + circular pie chart + bullet bars
                GeometryRow::move_to(1, "0 in", "=Height*0.78"),
                GeometryRow::line_to(2, "=Width", "=Height*0.78"),
                GeometryRow::move_to(3, "=Width*0.18", "=Height*0.44"),
                GeometryRow::arc_to(4, "=Width*0.46", "=Height*0.44", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(5, "=Width*0.18", "=Height*0.44", "=MIN(Width,Height)*0.14"),
                GeometryRow::move_to(6, "=Width*0.32", "=Height*0.44"),
                GeometryRow::line_to(7, "=Width*0.32", "=Height*0.62"),
                GeometryRow::move_to(8, "=Width*0.32", "=Height*0.44"),
                GeometryRow::line_to(9, "=Width*0.46", "=Height*0.44"),
                GeometryRow::move_to(10, "=Width*0.56", "=Height*0.54"),
                GeometryRow::line_to(11, "=Width*0.86", "=Height*0.54"),
                GeometryRow::move_to(12, "=Width*0.56", "=Height*0.34"),
                GeometryRow::line_to(13, "=Width*0.82", "=Height*0.34"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "office_onenote" => {
            detail.rows = vec![
                // Left binder spine + right section tabs + 'N' monogram
                GeometryRow::move_to(1, "=Width*0.20", "0 in"),
                GeometryRow::line_to(2, "=Width*0.20", "=Height"),
                GeometryRow::move_to(3, "=Width*0.86", "=Height*0.75"),
                GeometryRow::line_to(4, "=Width", "=Height*0.75"),
                GeometryRow::move_to(5, "=Width*0.86", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width", "=Height*0.50"),
                GeometryRow::move_to(7, "=Width*0.86", "=Height*0.25"),
                GeometryRow::line_to(8, "=Width", "=Height*0.25"),
                GeometryRow::move_to(9, "=Width*0.38", "=Height*0.32"),
                GeometryRow::line_to(10, "=Width*0.38", "=Height*0.68"),
                GeometryRow::line_to(11, "=Width*0.66", "=Height*0.32"),
                GeometryRow::line_to(12, "=Width*0.66", "=Height*0.68"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_copilot" => {
            detail.rows = vec![
                // Intertwined dual-ribbon infinity loop
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.50"),
                GeometryRow::arc_to(2, "=Width*0.50", "=Height*0.50", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(3, "=Width*0.78", "=Height*0.50", "=MIN(Width,Height)*-0.14"),
                GeometryRow::arc_to(4, "=Width*0.50", "=Height*0.50", "=MIN(Width,Height)*-0.14"),
                GeometryRow::arc_to(5, "=Width*0.22", "=Height*0.50", "=MIN(Width,Height)*0.14"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_loop" => {
            detail.rows = vec![
                // Tear-drop modular Loop ring anchored bottom-left
                GeometryRow::move_to(1, "=Width*0.26", "=Height*0.22"),
                GeometryRow::line_to(2, "=Width*0.52", "=Height*0.22"),
                GeometryRow::arc_to(3, "=Width*0.52", "=Height*0.78", "=MIN(Width,Height)*0.18"),
                GeometryRow::arc_to(4, "=Width*0.26", "=Height*0.50", "=MIN(Width,Height)*0.12"),
                GeometryRow::line_to(5, "=Width*0.26", "=Height*0.22"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_intune" => {
            detail.rows = vec![
                // Managed laptop screen + overlapping mobile handset
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.30"),
                GeometryRow::line_to(2, "=Width*0.66", "=Height*0.30"),
                GeometryRow::line_to(3, "=Width*0.66", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.14", "=Height*0.78"),
                GeometryRow::line_to(5, "=Width*0.14", "=Height*0.30"),
                GeometryRow::move_to(6, "=Width*0.62", "=Height*0.18"),
                GeometryRow::line_to(7, "=Width*0.86", "=Height*0.18"),
                GeometryRow::line_to(8, "=Width*0.86", "=Height*0.66"),
                GeometryRow::line_to(9, "=Width*0.62", "=Height*0.66"),
                GeometryRow::line_to(10, "=Width*0.62", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "m365_defender" => {
            // Classic pointed-bottom security shield + 4-quadrant seam
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.50", "0 in"),
                GeometryRow::line_to(2, "=Width*0.92", "=Height*0.36"),
                GeometryRow::line_to(3, "=Width*0.92", "=Height*0.84"),
                GeometryRow::line_to(4, "=Width*0.50", "=Height"),
                GeometryRow::line_to(5, "=Width*0.08", "=Height*0.84"),
                GeometryRow::line_to(6, "=Width*0.08", "=Height*0.36"),
                GeometryRow::line_to(7, "=Width*0.50", "0 in"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.50", "0 in"),
                GeometryRow::line_to(2, "=Width*0.50", "=Height"),
                GeometryRow::move_to(3, "=Width*0.08", "=Height*0.52"),
                GeometryRow::line_to(4, "=Width*0.92", "=Height*0.52"),
            ];
            sh.sheet.add_geometry(detail);
        }
        // --- Microsoft Servers & Active Directory (14 high-definition vector masters) ---
        "ms_win_server" => {
            detail.rows = vec![
                // 3 horizontal server drive bays + front LEDs + 2x2 OS emblem
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.82"),
                GeometryRow::line_to(2, "=Width*0.84", "=Height*0.82"),
                GeometryRow::move_to(3, "=Width*0.16", "=Height*0.68"),
                GeometryRow::line_to(4, "=Width*0.84", "=Height*0.68"),
                GeometryRow::move_to(5, "=Width*0.16", "=Height*0.54"),
                GeometryRow::line_to(6, "=Width*0.84", "=Height*0.54"),
                GeometryRow::move_to(7, "=Width*0.34", "=Height*0.16"),
                GeometryRow::line_to(8, "=Width*0.66", "=Height*0.16"),
                GeometryRow::line_to(9, "=Width*0.66", "=Height*0.42"),
                GeometryRow::line_to(10, "=Width*0.34", "=Height*0.42"),
                GeometryRow::line_to(11, "=Width*0.34", "=Height*0.16"),
                GeometryRow::move_to(12, "=Width*0.50", "=Height*0.16"),
                GeometryRow::line_to(13, "=Width*0.50", "=Height*0.42"),
                GeometryRow::move_to(14, "=Width*0.34", "=Height*0.29"),
                GeometryRow::line_to(15, "=Width*0.66", "=Height*0.29"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_ad_dc" => {
            detail.rows = vec![
                // Hierarchical Active Directory tree lines inside pyramid
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.72"),
                GeometryRow::line_to(2, "=Width*0.50", "=Height*0.42"),
                GeometryRow::move_to(3, "=Width*0.26", "=Height*0.42"),
                GeometryRow::line_to(4, "=Width*0.74", "=Height*0.42"),
                GeometryRow::move_to(5, "=Width*0.26", "=Height*0.42"),
                GeometryRow::line_to(6, "=Width*0.26", "=Height*0.18"),
                GeometryRow::move_to(7, "=Width*0.50", "=Height*0.42"),
                GeometryRow::line_to(8, "=Width*0.50", "=Height*0.18"),
                GeometryRow::move_to(9, "=Width*0.74", "=Height*0.42"),
                GeometryRow::line_to(10, "=Width*0.74", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_entra_id" => {
            detail.rows = vec![
                // Faceted Entra identity prism rays
                GeometryRow::move_to(1, "=Width*0.50", "=Height"),
                GeometryRow::line_to(2, "=Width*0.50", "=Height*0.38"),
                GeometryRow::line_to(3, "0 in", "=Height*0.50"),
                GeometryRow::move_to(4, "=Width*0.50", "=Height*0.38"),
                GeometryRow::line_to(5, "=Width", "=Height*0.50"),
                GeometryRow::move_to(6, "=Width*0.50", "=Height*0.38"),
                GeometryRow::line_to(7, "=Width*0.50", "0 in"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_ad_trust" => {
            detail.rows = vec![
                // Left & right directory triangles linked by bidirectional trust arrow
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.26"),
                GeometryRow::line_to(2, "=Width*0.34", "=Height*0.26"),
                GeometryRow::line_to(3, "=Width*0.23", "=Height*0.74"),
                GeometryRow::line_to(4, "=Width*0.12", "=Height*0.26"),
                GeometryRow::move_to(5, "=Width*0.66", "=Height*0.26"),
                GeometryRow::line_to(6, "=Width*0.88", "=Height*0.26"),
                GeometryRow::line_to(7, "=Width*0.77", "=Height*0.74"),
                GeometryRow::line_to(8, "=Width*0.66", "=Height*0.26"),
                GeometryRow::move_to(9, "=Width*0.36", "=Height*0.50"),
                GeometryRow::line_to(10, "=Width*0.64", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_gpo" => {
            detail.rows = vec![
                // Policy checkmarks & rule bars
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.62"),
                GeometryRow::line_to(2, "=Width*0.26", "=Height*0.54"),
                GeometryRow::line_to(3, "=Width*0.36", "=Height*0.68"),
                GeometryRow::move_to(4, "=Width*0.44", "=Height*0.60"),
                GeometryRow::line_to(5, "=Width*0.80", "=Height*0.60"),
                GeometryRow::move_to(6, "=Width*0.18", "=Height*0.34"),
                GeometryRow::line_to(7, "=Width*0.26", "=Height*0.26"),
                GeometryRow::line_to(8, "=Width*0.36", "=Height*0.40"),
                GeometryRow::move_to(9, "=Width*0.44", "=Height*0.32"),
                GeometryRow::line_to(10, "=Width*0.80", "=Height*0.32"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_dns_dhcp" => {
            detail.rows = vec![
                // Server slot + bidirectional IP lookup arrows
                GeometryRow::move_to(1, "0 in", "=Height*0.68"),
                GeometryRow::line_to(2, "=Width", "=Height*0.68"),
                GeometryRow::move_to(3, "=Width*0.20", "=Height*0.44"),
                GeometryRow::line_to(4, "=Width*0.80", "=Height*0.44"),
                GeometryRow::line_to(5, "=Width*0.68", "=Height*0.54"),
                GeometryRow::move_to(6, "=Width*0.80", "=Height*0.24"),
                GeometryRow::line_to(7, "=Width*0.20", "=Height*0.24"),
                GeometryRow::line_to(8, "=Width*0.32", "=Height*0.14"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_iis" => {
            detail.rows = vec![
                // Web globe circle + equator & meridian
                GeometryRow::move_to(1, "=Width*0.28", "=Height*0.50"),
                GeometryRow::arc_to(2, "=Width*0.72", "=Height*0.50", "=MIN(Width,Height)*0.22"),
                GeometryRow::arc_to(3, "=Width*0.28", "=Height*0.50", "=MIN(Width,Height)*0.22"),
                GeometryRow::move_to(4, "=Width*0.28", "=Height*0.50"),
                GeometryRow::line_to(5, "=Width*0.72", "=Height*0.50"),
                GeometryRow::move_to(6, "=Width*0.50", "=Height*0.18"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.82"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_sql_server" => {
            // SQL cylinder body + top ellipse cap + dual data-tier rings
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.14"),
                GeometryRow::line_to(2, "=Width", "=Height*0.14"),
                GeometryRow::line_to(3, "=Width", "=Height*0.84"),
                GeometryRow::line_to(4, "0 in", "=Height*0.84"),
                GeometryRow::line_to(5, "0 in", "=Height*0.14"),
            ];
            sh.sheet.add_geometry(g1);
            let mut cap = GeometrySection::new(2);
            cap.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.84".into(),
                a: Some("=Width".into()),
                b: Some("=Height*0.84".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height".into()),
                e: None,
                deleted: false,
            }];
            sh.sheet.add_geometry(cap);
            let mut rings = GeometrySection::new(3);
            rings.no_fill = "TRUE".into();
            rings.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.58"),
                GeometryRow::arc_to(2, "=Width", "=Height*0.58", "=MIN(Width,Height)*-0.08"),
                GeometryRow::move_to(3, "0 in", "=Height*0.36"),
                GeometryRow::arc_to(4, "=Width", "=Height*0.36", "=MIN(Width,Height)*-0.08"),
            ];
            sh.sheet.add_geometry(rings);
        }
        "ms_exchange_srv" => {
            detail.rows = vec![
                // Server rack header + front mail routing envelope
                GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.20", "=Height*0.20"),
                GeometryRow::line_to(4, "=Width*0.80", "=Height*0.20"),
                GeometryRow::line_to(5, "=Width*0.80", "=Height*0.58"),
                GeometryRow::line_to(6, "=Width*0.20", "=Height*0.58"),
                GeometryRow::line_to(7, "=Width*0.20", "=Height*0.20"),
                GeometryRow::move_to(8, "=Width*0.20", "=Height*0.58"),
                GeometryRow::line_to(9, "=Width*0.50", "=Height*0.36"),
                GeometryRow::line_to(10, "=Width*0.80", "=Height*0.58"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_sharepoint_srv" => {
            detail.rows = vec![
                // Server header + interconnected intranet farm nodes
                GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.26", "=Height*0.38"),
                GeometryRow::arc_to(4, "=Width*0.56", "=Height*0.38", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(5, "=Width*0.26", "=Height*0.38", "=MIN(Width,Height)*0.14"),
                GeometryRow::move_to(6, "=Width*0.46", "=Height*0.38"),
                GeometryRow::arc_to(7, "=Width*0.76", "=Height*0.38", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(8, "=Width*0.46", "=Height*0.38", "=MIN(Width,Height)*0.14"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_hyperv" => {
            // Add virtualized VM guest slices onto Cube3D face (`Geometry3` since Cube3D uses 1 & 2)
            let mut slices = GeometrySection::new(3);
            slices.no_fill = "TRUE".into();
            slices.rows = vec![
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.64", "=Height*0.50"),
                GeometryRow::move_to(3, "=Width*0.12", "=Height*0.26"),
                GeometryRow::line_to(4, "=Width*0.64", "=Height*0.26"),
            ];
            sh.sheet.add_geometry(slices);
        }
        "ms_failover_cluster" => {
            detail.rows = vec![
                // Dual HA server blades + central heartbeat pulse
                GeometryRow::move_to(1, "=Width*0.42", "0 in"),
                GeometryRow::line_to(2, "=Width*0.42", "=Height"),
                GeometryRow::move_to(3, "=Width*0.58", "0 in"),
                GeometryRow::line_to(4, "=Width*0.58", "=Height"),
                GeometryRow::move_to(5, "=Width*0.20", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width*0.45", "=Height*0.50"),
                GeometryRow::line_to(7, "=Width*0.48", "=Height*0.72"),
                GeometryRow::line_to(8, "=Width*0.52", "=Height*0.28"),
                GeometryRow::line_to(9, "=Width*0.55", "=Height*0.50"),
                GeometryRow::line_to(10, "=Width*0.80", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_system_center" => {
            detail.rows = vec![
                // Fleet telemetry waveform inside hexagon
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.36", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.45", "=Height*0.76"),
                GeometryRow::line_to(4, "=Width*0.55", "=Height*0.24"),
                GeometryRow::line_to(5, "=Width*0.64", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width*0.82", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_win11_client" => {
            // Monitor bezel + stand neck/base + centered Windows 11 taskbar
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.08", "=Height*0.24"),
                GeometryRow::line_to(2, "=Width*0.92", "=Height*0.24"),
                GeometryRow::line_to(3, "=Width*0.92", "=Height*0.94"),
                GeometryRow::line_to(4, "=Width*0.08", "=Height*0.94"),
                GeometryRow::line_to(5, "=Width*0.08", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.24"),
                GeometryRow::line_to(2, "=Width*0.50", "=Height*0.08"),
                GeometryRow::move_to(3, "=Width*0.30", "=Height*0.08"),
                GeometryRow::line_to(4, "=Width*0.70", "=Height*0.08"),
                GeometryRow::move_to(5, "=Width*0.34", "=Height*0.34"),
                GeometryRow::line_to(6, "=Width*0.66", "=Height*0.34"),
            ];
            sh.sheet.add_geometry(detail);
        }
        // --- Microsoft Azure & Hybrid Cloud (14 high-definition vector masters) ---
        "az_resource_group" => {
            detail.rows = vec![
                // Resource group header line + corner cube badge
                GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.08", "=Height*0.82"),
                GeometryRow::line_to(4, "=Width*0.20", "=Height*0.82"),
                GeometryRow::line_to(5, "=Width*0.20", "=Height*0.94"),
                GeometryRow::line_to(6, "=Width*0.08", "=Height*0.94"),
                GeometryRow::line_to(7, "=Width*0.08", "=Height*0.82"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_vm" => {
            detail.rows = vec![
                // Inner compute cube + VM monitor pedestal
                GeometryRow::move_to(1, "=Width*0.32", "=Height*0.34"),
                GeometryRow::line_to(2, "=Width*0.68", "=Height*0.34"),
                GeometryRow::line_to(3, "=Width*0.68", "=Height*0.76"),
                GeometryRow::line_to(4, "=Width*0.32", "=Height*0.76"),
                GeometryRow::line_to(5, "=Width*0.32", "=Height*0.34"),
                GeometryRow::move_to(6, "=Width*0.36", "=Height*0.16"),
                GeometryRow::line_to(7, "=Width*0.64", "=Height*0.16"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_aks" => {
            detail.rows = vec![
                // AKS 6-spoke container helm + central hub
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.16"),
                GeometryRow::line_to(2, "=Width*0.50", "=Height*0.84"),
                GeometryRow::move_to(3, "=Width*0.22", "=Height*0.32"),
                GeometryRow::line_to(4, "=Width*0.78", "=Height*0.68"),
                GeometryRow::move_to(5, "=Width*0.22", "=Height*0.68"),
                GeometryRow::line_to(6, "=Width*0.78", "=Height*0.32"),
                GeometryRow::move_to(7, "=Width*0.38", "=Height*0.50"),
                GeometryRow::arc_to(8, "=Width*0.62", "=Height*0.50", "=MIN(Width,Height)*0.11"),
                GeometryRow::arc_to(9, "=Width*0.38", "=Height*0.50", "=MIN(Width,Height)*0.11"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_app_service" => {
            detail.rows = vec![
                // Criss-crossing orbital web paths across App Service sphere
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.34"),
                GeometryRow::arc_to(2, "=Width*0.86", "=Height*0.66", "=MIN(Width,Height)*0.16"),
                GeometryRow::move_to(3, "=Width*0.14", "=Height*0.66"),
                GeometryRow::arc_to(4, "=Width*0.86", "=Height*0.34", "=MIN(Width,Height)*-0.16"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_functions" => {
            detail.rows = vec![
                // Serverless lightning bolt + '< >' code brackets
                GeometryRow::move_to(1, "=Width*0.54", "=Height*0.78"),
                GeometryRow::line_to(2, "=Width*0.40", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.56", "=Height*0.50"),
                GeometryRow::line_to(4, "=Width*0.44", "=Height*0.22"),
                GeometryRow::move_to(5, "=Width*0.30", "=Height*0.62"),
                GeometryRow::line_to(6, "=Width*0.20", "=Height*0.50"),
                GeometryRow::line_to(7, "=Width*0.30", "=Height*0.38"),
                GeometryRow::move_to(8, "=Width*0.70", "=Height*0.62"),
                GeometryRow::line_to(9, "=Width*0.80", "=Height*0.50"),
                GeometryRow::line_to(10, "=Width*0.70", "=Height*0.38"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_sql_cosmos" => {
            // Cylinder + planetary Cosmos DB ring
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.16"),
                GeometryRow::line_to(2, "=Width*0.86", "=Height*0.16"),
                GeometryRow::line_to(3, "=Width*0.86", "=Height*0.84"),
                GeometryRow::line_to(4, "=Width*0.14", "=Height*0.84"),
                GeometryRow::line_to(5, "=Width*0.14", "=Height*0.16"),
            ];
            sh.sheet.add_geometry(g1);
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.84"),
                GeometryRow::arc_to(2, "=Width*0.86", "=Height*0.84", "=MIN(Width,Height)*0.10"),
                GeometryRow::arc_to(3, "=Width*0.14", "=Height*0.84", "=MIN(Width,Height)*0.10"),
                GeometryRow::move_to(4, "0 in", "=Height*0.38"),
                GeometryRow::arc_to(5, "=Width", "=Height*0.62", "=MIN(Width,Height)*-0.14"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_storage" => {
            detail.rows = vec![
                // 3 stacked Azure Storage tiers (Blob / File / Table)
                GeometryRow::move_to(1, "0 in", "=Height*0.68"),
                GeometryRow::line_to(2, "=Width", "=Height*0.68"),
                GeometryRow::move_to(3, "0 in", "=Height*0.36"),
                GeometryRow::line_to(4, "=Width", "=Height*0.36"),
                GeometryRow::move_to(5, "=Width*0.36", "=Height*0.84"),
                GeometryRow::line_to(6, "=Width*0.64", "=Height*0.84"),
                GeometryRow::move_to(7, "=Width*0.36", "=Height*0.52"),
                GeometryRow::line_to(8, "=Width*0.64", "=Height*0.52"),
                GeometryRow::move_to(9, "=Width*0.36", "=Height*0.18"),
                GeometryRow::line_to(10, "=Width*0.64", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_vnet" => {
            detail.rows = vec![
                // '< ... >' VNet address brackets + 3 subnet nodes
                GeometryRow::move_to(1, "=Width*0.28", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width*0.14", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.28", "=Height*0.24"),
                GeometryRow::move_to(4, "=Width*0.72", "=Height*0.76"),
                GeometryRow::line_to(5, "=Width*0.86", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width*0.72", "=Height*0.24"),
                GeometryRow::move_to(7, "=Width*0.36", "=Height*0.50"),
                GeometryRow::line_to(8, "=Width*0.64", "=Height*0.50"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_front_door" => {
            detail.rows = vec![
                // Global ingress 1 -> 3 fan-out routing
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.46", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.78", "=Height*0.76"),
                GeometryRow::move_to(4, "=Width*0.46", "=Height*0.50"),
                GeometryRow::line_to(5, "=Width*0.82", "=Height*0.50"),
                GeometryRow::move_to(6, "=Width*0.46", "=Height*0.50"),
                GeometryRow::line_to(7, "=Width*0.78", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_key_vault" => {
            detail.rows = vec![
                // Inner vault ring + central keyhole
                GeometryRow::move_to(1, "=Width*0.34", "=Height*0.58"),
                GeometryRow::arc_to(2, "=Width*0.66", "=Height*0.58", "=MIN(Width,Height)*0.14"),
                GeometryRow::arc_to(3, "=Width*0.34", "=Height*0.58", "=MIN(Width,Height)*0.14"),
                GeometryRow::move_to(4, "=Width*0.44", "=Height*0.44"),
                GeometryRow::line_to(5, "=Width*0.40", "=Height*0.20"),
                GeometryRow::line_to(6, "=Width*0.60", "=Height*0.20"),
                GeometryRow::line_to(7, "=Width*0.56", "=Height*0.44"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_service_bus" => {
            detail.rows = vec![
                // Top/bottom bus rails + streaming message lanes
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width*0.88", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.18", "=Height*0.50"),
                GeometryRow::line_to(4, "=Width*0.82", "=Height*0.50"),
                GeometryRow::line_to(5, "=Width*0.70", "=Height*0.62"),
                GeometryRow::move_to(6, "=Width*0.12", "=Height*0.24"),
                GeometryRow::line_to(7, "=Width*0.88", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_expressroute" => {
            detail.rows = vec![
                // Dedicated private triangle circuit between 3 peering sites
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.74"),
                GeometryRow::line_to(2, "=Width*0.28", "=Height*0.32"),
                GeometryRow::line_to(3, "=Width*0.72", "=Height*0.32"),
                GeometryRow::line_to(4, "=Width*0.50", "=Height*0.74"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_arc" => {
            detail.rows = vec![
                // Sweeping hybrid Azure Arc over on-prem horizon
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.28"),
                GeometryRow::arc_to(2, "=Width*0.84", "=Height*0.28", "=MIN(Width,Height)*0.24"),
                GeometryRow::move_to(3, "=Width*0.28", "=Height*0.28"),
                GeometryRow::arc_to(4, "=Width*0.72", "=Height*0.28", "=MIN(Width,Height)*0.14"),
                GeometryRow::move_to(5, "=Width*0.14", "=Height*0.28"),
                GeometryRow::line_to(6, "=Width*0.86", "=Height*0.28"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "az_openai" => {
            detail.rows = vec![
                // Neural star-mesh inside Azure AI Foundry hexagon
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.82"),
                GeometryRow::line_to(2, "=Width*0.24", "=Height*0.34"),
                GeometryRow::line_to(3, "=Width*0.76", "=Height*0.34"),
                GeometryRow::line_to(4, "=Width*0.50", "=Height*0.82"),
                GeometryRow::move_to(5, "=Width*0.50", "=Height*0.18"),
                GeometryRow::line_to(6, "=Width*0.24", "=Height*0.66"),
                GeometryRow::line_to(7, "=Width*0.76", "=Height*0.66"),
                GeometryRow::line_to(8, "=Width*0.50", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }
        // --- Microsoft Power Platform & Dynamics 365 (10 high-definition vector masters) ---
        "pp_powerbi" => {
            detail.rows = vec![
                // 3 ascending Power BI column bars
                GeometryRow::move_to(1, "=Width*0.20", "=Height*0.16"),
                GeometryRow::line_to(2, "=Width*0.20", "=Height*0.44"),
                GeometryRow::line_to(3, "=Width*0.36", "=Height*0.44"),
                GeometryRow::line_to(4, "=Width*0.36", "=Height*0.16"),
                GeometryRow::move_to(5, "=Width*0.44", "=Height*0.16"),
                GeometryRow::line_to(6, "=Width*0.44", "=Height*0.64"),
                GeometryRow::line_to(7, "=Width*0.60", "=Height*0.64"),
                GeometryRow::line_to(8, "=Width*0.60", "=Height*0.16"),
                GeometryRow::move_to(9, "=Width*0.68", "=Height*0.16"),
                GeometryRow::line_to(10, "=Width*0.68", "=Height*0.84"),
                GeometryRow::line_to(11, "=Width*0.84", "=Height*0.84"),
                GeometryRow::line_to(12, "=Width*0.84", "=Height*0.16"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pp_automate" => {
            detail.rows = vec![
                // Nested automation flow chevron
                GeometryRow::move_to(1, "=Width*0.34", "=Height*0.22"),
                GeometryRow::line_to(2, "=Width*0.64", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.34", "=Height*0.78"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pp_powerapps" => {
            detail.rows = vec![
                // Nested layered app diamond
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.22"),
                GeometryRow::line_to(2, "=Width*0.78", "=Height*0.50"),
                GeometryRow::line_to(3, "=Width*0.50", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.22", "=Height*0.50"),
                GeometryRow::line_to(5, "=Width*0.50", "=Height*0.22"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pp_dataverse" => {
            detail.rows = vec![
                // 3 relational Dataverse record rows with primary-key divider
                GeometryRow::move_to(1, "0 in", "=Height*0.67"),
                GeometryRow::line_to(2, "=Width", "=Height*0.67"),
                GeometryRow::move_to(3, "0 in", "=Height*0.34"),
                GeometryRow::line_to(4, "=Width", "=Height*0.34"),
                GeometryRow::move_to(5, "=Width*0.26", "0 in"),
                GeometryRow::line_to(6, "=Width*0.26", "=Height"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pp_copilot_studio" => {
            detail.rows = vec![
                // Conversational bot visor & eyes inside CalloutBubble
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.48"),
                GeometryRow::line_to(2, "=Width*0.76", "=Height*0.48"),
                GeometryRow::line_to(3, "=Width*0.76", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.24", "=Height*0.78"),
                GeometryRow::line_to(5, "=Width*0.24", "=Height*0.48"),
                GeometryRow::move_to(6, "=Width*0.38", "=Height*0.63"),
                GeometryRow::line_to(7, "=Width*0.44", "=Height*0.63"),
                GeometryRow::move_to(8, "=Width*0.56", "=Height*0.63"),
                GeometryRow::line_to(9, "=Width*0.62", "=Height*0.63"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pp_power_pages" => {
            detail.rows = vec![
                // Portal browser bar + 2-column content layout
                GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.36", "0 in"),
                GeometryRow::line_to(4, "=Width*0.36", "=Height*0.76"),
                GeometryRow::move_to(5, "=Width*0.48", "=Height*0.52"),
                GeometryRow::line_to(6, "=Width*0.86", "=Height*0.52"),
                GeometryRow::move_to(7, "=Width*0.48", "=Height*0.28"),
                GeometryRow::line_to(8, "=Width*0.86", "=Height*0.28"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "pp_gateway" => {
            detail.rows = vec![
                // On-premises data gateway bidirectional bridge
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.60"),
                GeometryRow::line_to(2, "=Width*0.78", "=Height*0.60"),
                GeometryRow::line_to(3, "=Width*0.66", "=Height*0.74"),
                GeometryRow::move_to(4, "=Width*0.78", "=Height*0.40"),
                GeometryRow::line_to(5, "=Width*0.22", "=Height*0.40"),
                GeometryRow::line_to(6, "=Width*0.34", "=Height*0.26"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "d365_crm" => {
            detail.rows = vec![
                // Dynamics 365 CRM pipeline growth trend line
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.24"),
                GeometryRow::line_to(2, "=Width*0.40", "=Height*0.48"),
                GeometryRow::line_to(3, "=Width*0.58", "=Height*0.36"),
                GeometryRow::line_to(4, "=Width*0.84", "=Height*0.76"),
                GeometryRow::move_to(5, "=Width*0.68", "=Height*0.76"),
                GeometryRow::line_to(6, "=Width*0.84", "=Height*0.76"),
                GeometryRow::line_to(7, "=Width*0.84", "=Height*0.58"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "d365_erp" => {
            detail.rows = vec![
                // Dynamics 365 Finance & Supply Chain modular blocks
                GeometryRow::move_to(1, "=Width*0.50", "0 in"),
                GeometryRow::line_to(2, "=Width*0.50", "=Height"),
                GeometryRow::move_to(3, "0 in", "=Height*0.50"),
                GeometryRow::line_to(4, "=Width", "=Height*0.50"),
                GeometryRow::move_to(5, "=Width*0.34", "=Height*0.34"),
                GeometryRow::line_to(6, "=Width*0.66", "=Height*0.34"),
                GeometryRow::line_to(7, "=Width*0.66", "=Height*0.66"),
                GeometryRow::line_to(8, "=Width*0.34", "=Height*0.66"),
                GeometryRow::line_to(9, "=Width*0.34", "=Height*0.34"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "ms_fabric" => {
            detail.rows = vec![
                // OneLake delta-lakehouse strata inside Fabric parallelogram
                GeometryRow::move_to(1, "=Width*0.06", "=Height*0.66"),
                GeometryRow::line_to(2, "=Width*0.90", "=Height*0.66"),
                GeometryRow::move_to(3, "=Width*0.11", "=Height*0.34"),
                GeometryRow::line_to(4, "=Width*0.95", "=Height*0.34"),
            ];
            sh.sheet.add_geometry(detail);
        }
        _ => append_extended_role_vector_details(sh, category, role, detail),
    }
}

fn append_extended_role_vector_details(
    sh: &mut VisioShape,
    _category: &str,
    role: &str,
    mut detail: GeometrySection,
) {
    match role {
        // --- Standard Servers (All 26 Visio Server Roles: Chassis + Role Emblem) ---
        "srv_tower" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.84"),
                GeometryRow::line_to(2, "=Width*0.82", "=Height*0.84"),
                GeometryRow::move_to(3, "=Width*0.18", "=Height*0.70"),
                GeometryRow::line_to(4, "=Width*0.82", "=Height*0.70"),
                GeometryRow::move_to(5, "=Width*0.18", "=Height*0.56"),
                GeometryRow::line_to(6, "=Width*0.82", "=Height*0.56"),
                GeometryRow::move_to(7, "=Width*0.24", "=Height*0.16"),
                GeometryRow::line_to(8, "=Width*0.24", "=Height*0.42"),
                GeometryRow::move_to(9, "=Width*0.50", "=Height*0.16"),
                GeometryRow::line_to(10, "=Width*0.50", "=Height*0.42"),
                GeometryRow::move_to(11, "=Width*0.76", "=Height*0.16"),
                GeometryRow::line_to(12, "=Width*0.76", "=Height*0.42"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "srv_rack" | "srv_rack_4u" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.08", "0 in"),
                GeometryRow::line_to(2, "=Width*0.08", "=Height"),
                GeometryRow::move_to(3, "=Width*0.92", "0 in"),
                GeometryRow::line_to(4, "=Width*0.92", "=Height"),
                GeometryRow::move_to(5, "=Width*0.16", "=Height*0.50"),
                GeometryRow::line_to(6, "=Width*0.46", "=Height*0.50"),
                GeometryRow::move_to(7, "=Width*0.54", "=Height*0.32"),
                GeometryRow::line_to(8, "=Width*0.84", "=Height*0.32"),
                GeometryRow::move_to(9, "=Width*0.54", "=Height*0.68"),
                GeometryRow::line_to(10, "=Width*0.84", "=Height*0.68"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "srv_blade" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.78"),
                GeometryRow::line_to(2, "=Width", "=Height*0.78"),
                GeometryRow::move_to(3, "=Width*0.20", "0 in"),
                GeometryRow::line_to(4, "=Width*0.20", "=Height*0.78"),
                GeometryRow::move_to(5, "=Width*0.40", "0 in"),
                GeometryRow::line_to(6, "=Width*0.40", "=Height*0.78"),
                GeometryRow::move_to(7, "=Width*0.60", "0 in"),
                GeometryRow::line_to(8, "=Width*0.60", "=Height*0.78"),
                GeometryRow::move_to(9, "=Width*0.80", "0 in"),
                GeometryRow::line_to(10, "=Width*0.80", "=Height*0.78"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "srv_blade_mod" | "srv_edge" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.78"),
                GeometryRow::line_to(2, "=Width*0.82", "=Height*0.78"),
                GeometryRow::move_to(3, "=Width*0.28", "=Height*0.24"),
                GeometryRow::line_to(4, "=Width*0.72", "=Height*0.24"),
                GeometryRow::line_to(5, "=Width*0.72", "=Height*0.62"),
                GeometryRow::line_to(6, "=Width*0.28", "=Height*0.62"),
                GeometryRow::line_to(7, "=Width*0.28", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "srv_mainframe" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.33", "0 in"),
                GeometryRow::line_to(2, "=Width*0.33", "=Height"),
                GeometryRow::move_to(3, "=Width*0.67", "0 in"),
                GeometryRow::line_to(4, "=Width*0.67", "=Height"),
                GeometryRow::move_to(5, "=Width*0.08", "=Height*0.76"),
                GeometryRow::line_to(6, "=Width*0.92", "=Height*0.76"),
                GeometryRow::move_to(7, "=Width*0.08", "=Height*0.28"),
                GeometryRow::line_to(8, "=Width*0.92", "=Height*0.28"),
            ];
            sh.sheet.add_geometry(detail);
        }
        r if r.starts_with("srv_") => {
            fn push_seg(rows: &mut Vec<GeometryRow>, x1: &str, y1: &str, x2: &str, y2: &str) {
                let ix = (rows.len() + 1) as u32;
                rows.push(GeometryRow::move_to(ix, x1, y1));
                rows.push(GeometryRow::line_to(ix + 1, x2, y2));
            }
            fn push_circle_arcs(rows: &mut Vec<GeometryRow>, x1: &str, x2: &str, y: &str, bow: &str) {
                let ix = (rows.len() + 1) as u32;
                rows.push(GeometryRow::move_to(ix, x1, y));
                rows.push(GeometryRow::arc_to(ix + 1, x2, y, bow));
                rows.push(GeometryRow::arc_to(ix + 2, x1, y, bow));
            }
            // Standard Visio Server top drive-bay bezel + distinct role emblem in lower 65%
            let mut rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.16", "=Height*0.88"),
                GeometryRow::line_to(4, "=Width*0.58", "=Height*0.88"),
                GeometryRow::move_to(5, "=Width*0.72", "=Height*0.88"),
                GeometryRow::line_to(6, "=Width*0.84", "=Height*0.88"),
            ];
            match r {
                "srv_web" => {
                    push_circle_arcs(&mut rows, "=Width*0.28", "=Width*0.72", "=Height*0.38", "0.20 in");
                    push_seg(&mut rows, "=Width*0.28", "=Height*0.38", "=Width*0.72", "=Height*0.38");
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.14", "=Width*0.50", "=Height*0.62");
                }
                "srv_db" => {
                    push_seg(&mut rows, "=Width*0.26", "=Height*0.18", "=Width*0.26", "=Height*0.56");
                    push_seg(&mut rows, "=Width*0.74", "=Height*0.18", "=Width*0.74", "=Height*0.56");
                    push_circle_arcs(&mut rows, "=Width*0.26", "=Width*0.74", "=Height*0.56", "0.08 in");
                    let ix = (rows.len() + 1) as u32;
                    rows.push(GeometryRow::move_to(ix, "=Width*0.26", "=Height*0.18"));
                    rows.push(GeometryRow::arc_to(ix + 1, "=Width*0.74", "=Height*0.18", "=MIN(Width,Height)*-0.08"));
                }
                "srv_app" | "srv_rds" => {
                    push_seg(&mut rows, "=Width*0.22", "=Height*0.18", "=Width*0.78", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.78", "=Height*0.18", "=Width*0.78", "=Height*0.60");
                    push_seg(&mut rows, "=Width*0.78", "=Height*0.60", "=Width*0.22", "=Height*0.60");
                    push_seg(&mut rows, "=Width*0.22", "=Height*0.60", "=Width*0.22", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.22", "=Height*0.48", "=Width*0.78", "=Height*0.48");
                }
                "srv_file" => {
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.18", "=Width*0.80", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.80", "=Height*0.18", "=Width*0.80", "=Height*0.52");
                    push_seg(&mut rows, "=Width*0.80", "=Height*0.52", "=Width*0.48", "=Height*0.52");
                    push_seg(&mut rows, "=Width*0.48", "=Height*0.52", "=Width*0.40", "=Height*0.62");
                    push_seg(&mut rows, "=Width*0.40", "=Height*0.62", "=Width*0.20", "=Height*0.62");
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.62", "=Width*0.20", "=Height*0.18");
                }
                "srv_print" => {
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.26", "=Width*0.80", "=Height*0.26");
                    push_seg(&mut rows, "=Width*0.80", "=Height*0.26", "=Width*0.80", "=Height*0.50");
                    push_seg(&mut rows, "=Width*0.80", "=Height*0.50", "=Width*0.20", "=Height*0.50");
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.50", "=Width*0.20", "=Height*0.26");
                    push_seg(&mut rows, "=Width*0.32", "=Height*0.50", "=Width*0.68", "=Height*0.62");
                    push_seg(&mut rows, "=Width*0.32", "=Height*0.14", "=Width*0.68", "=Height*0.14");
                }
                "srv_mail" => {
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.18", "=Width*0.80", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.80", "=Height*0.18", "=Width*0.80", "=Height*0.58");
                    push_seg(&mut rows, "=Width*0.80", "=Height*0.58", "=Width*0.20", "=Height*0.58");
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.58", "=Width*0.20", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.20", "=Height*0.58", "=Width*0.50", "=Height*0.36");
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.36", "=Width*0.80", "=Height*0.58");
                }
                "srv_ldap" => {
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.62", "=Width*0.50", "=Height*0.38");
                    push_seg(&mut rows, "=Width*0.24", "=Height*0.38", "=Width*0.76", "=Height*0.38");
                    push_seg(&mut rows, "=Width*0.24", "=Height*0.38", "=Width*0.24", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.38", "=Width*0.50", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.76", "=Height*0.38", "=Width*0.76", "=Height*0.18");
                }
                "srv_dns" | "srv_ntp" => {
                    push_circle_arcs(&mut rows, "=Width*0.30", "=Width*0.70", "=Height*0.38", "0.18 in");
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.38", "=Width*0.50", "=Height*0.56");
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.38", "=Width*0.64", "=Height*0.32");
                }
                "srv_dhcp" | "srv_ftp" => {
                    push_seg(&mut rows, "=Width*0.34", "=Height*0.18", "=Width*0.34", "=Height*0.60");
                    push_seg(&mut rows, "=Width*0.24", "=Height*0.48", "=Width*0.34", "=Height*0.60");
                    push_seg(&mut rows, "=Width*0.44", "=Height*0.48", "=Width*0.34", "=Height*0.60");
                    push_seg(&mut rows, "=Width*0.66", "=Height*0.60", "=Width*0.66", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.56", "=Height*0.30", "=Width*0.66", "=Height*0.18");
                    push_seg(&mut rows, "=Width*0.76", "=Height*0.30", "=Width*0.66", "=Height*0.18");
                }
                "srv_proxy" | "srv_pki" => {
                    push_seg(&mut rows, "=Width*0.50", "=Height*0.62", "=Width*0.76", "=Height*0.48");
                    push_seg(&mut rows, "=Width*0.76", "=Height*0.48", "=Width*0.68", "=Height*0.16");
                    push_seg(&mut rows, "=Width*0.68", "=Height*0.16", "=Width*0.32", "=Height*0.16");
                    push_seg(&mut rows, "=Width*0.32", "=Height*0.16", "=Width*0.24", "=Height*0.48");
                    push_seg(&mut rows, "=Width*0.24", "=Height*0.48", "=Width*0.50", "=Height*0.62");
                }
                "srv_bastion" => {
                    push_seg(&mut rows, "=Width*0.16", "=Height*0.50", "=Width*0.84", "=Height*0.50");
                    push_seg(&mut rows, "=Width*0.16", "=Height*0.30", "=Width*0.84", "=Height*0.30");
                    push_seg(&mut rows, "=Width*0.38", "=Height*0.50", "=Width*0.38", "=Height*0.66");
                    push_seg(&mut rows, "=Width*0.62", "=Height*0.30", "=Width*0.62", "=Height*0.50");
                    push_seg(&mut rows, "=Width*0.38", "=Height*0.14", "=Width*0.38", "=Height*0.30");
                }
                "srv_siem" | "srv_nms" => {
                    push_seg(&mut rows, "=Width*0.16", "=Height*0.38", "=Width*0.36", "=Height*0.38");
                    push_seg(&mut rows, "=Width*0.36", "=Height*0.38", "=Width*0.46", "=Height*0.62");
                    push_seg(&mut rows, "=Width*0.46", "=Height*0.62", "=Width*0.56", "=Height*0.16");
                    push_seg(&mut rows, "=Width*0.56", "=Height*0.16", "=Width*0.66", "=Height*0.38");
                    push_seg(&mut rows, "=Width*0.66", "=Height*0.38", "=Width*0.84", "=Height*0.38");
                }
                "srv_hypervisor" => {
                    push_seg(&mut rows, "=Width*0.18", "=Height*0.20", "=Width*0.46", "=Height*0.20");
                    push_seg(&mut rows, "=Width*0.46", "=Height*0.20", "=Width*0.46", "=Height*0.58");
                    push_seg(&mut rows, "=Width*0.46", "=Height*0.58", "=Width*0.18", "=Height*0.58");
                    push_seg(&mut rows, "=Width*0.18", "=Height*0.58", "=Width*0.18", "=Height*0.20");
                    push_seg(&mut rows, "=Width*0.54", "=Height*0.20", "=Width*0.82", "=Height*0.20");
                    push_seg(&mut rows, "=Width*0.82", "=Height*0.20", "=Width*0.82", "=Height*0.58");
                    push_seg(&mut rows, "=Width*0.82", "=Height*0.58", "=Width*0.54", "=Height*0.58");
                    push_seg(&mut rows, "=Width*0.54", "=Height*0.58", "=Width*0.54", "=Height*0.20");
                }
                _ => {
                    push_circle_arcs(&mut rows, "=Width*0.22", "=Width*0.46", "=Height*0.38", "0.11 in");
                    push_circle_arcs(&mut rows, "=Width*0.54", "=Width*0.78", "=Height*0.38", "0.11 in");
                }
            }
            detail.rows = rows;
            sh.sheet.add_geometry(detail);
        }

        // --- Network, Firewalls, Switches, Routers & Peripherals (All 38 Network Shapes) ---
        "net_ngfw" | "az_firewall" => {
            detail.rows = vec![
                // Brick firewall courses + central inspection shield
                GeometryRow::move_to(1, "0 in", "=Height*0.66"),
                GeometryRow::line_to(2, "=Width", "=Height*0.66"),
                GeometryRow::move_to(3, "0 in", "=Height*0.33"),
                GeometryRow::line_to(4, "=Width", "=Height*0.33"),
                GeometryRow::move_to(5, "=Width*0.28", "=Height*0.66"),
                GeometryRow::line_to(6, "=Width*0.28", "=Height"),
                GeometryRow::move_to(7, "=Width*0.72", "=Height*0.66"),
                GeometryRow::line_to(8, "=Width*0.72", "=Height"),
                GeometryRow::move_to(9, "=Width*0.50", "=Height*0.80"),
                GeometryRow::line_to(10, "=Width*0.66", "=Height*0.52"),
                GeometryRow::line_to(11, "=Width*0.50", "=Height*0.18"),
                GeometryRow::line_to(12, "=Width*0.34", "=Height*0.52"),
                GeometryRow::line_to(13, "=Width*0.50", "=Height*0.80"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_waf" | "az_app_gw" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.50"),
                GeometryRow::arc_to(2, "=Width*0.76", "=Height*0.50", "=MIN(Width,Height)*0.22"),
                GeometryRow::arc_to(3, "=Width*0.24", "=Height*0.50", "=MIN(Width,Height)*0.22"),
                GeometryRow::move_to(4, "=Width*0.16", "=Height*0.28"),
                GeometryRow::line_to(5, "=Width*0.84", "=Height*0.28"),
                GeometryRow::move_to(6, "=Width*0.50", "=Height*0.28"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.78"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_ids_ips" | "net_ddos" | "net_nac" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.18", "=Height*0.50"),
                GeometryRow::arc_to(2, "=Width*0.82", "=Height*0.50", "=MIN(Width,Height)*0.18"),
                GeometryRow::arc_to(3, "=Width*0.18", "=Height*0.50", "=MIN(Width,Height)*0.18"),
                GeometryRow::move_to(4, "=Width*0.40", "=Height*0.50"),
                GeometryRow::arc_to(5, "=Width*0.60", "=Height*0.50", "=MIN(Width,Height)*0.09"),
                GeometryRow::arc_to(6, "=Width*0.40", "=Height*0.50", "=MIN(Width,Height)*0.09"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_vpn_gw" | "az_vpn_gw" | "az_bastion" => {
            detail.rows = vec![
                // Tunnel cylinder + central padlock
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.26"),
                GeometryRow::line_to(2, "=Width*0.86", "=Height*0.26"),
                GeometryRow::move_to(3, "=Width*0.14", "=Height*0.74"),
                GeometryRow::line_to(4, "=Width*0.86", "=Height*0.74"),
                GeometryRow::move_to(5, "=Width*0.38", "=Height*0.30"),
                GeometryRow::line_to(6, "=Width*0.62", "=Height*0.30"),
                GeometryRow::line_to(7, "=Width*0.62", "=Height*0.56"),
                GeometryRow::line_to(8, "=Width*0.38", "=Height*0.56"),
                GeometryRow::line_to(9, "=Width*0.38", "=Height*0.30"),
                GeometryRow::move_to(10, "=Width*0.42", "=Height*0.56"),
                GeometryRow::arc_to(11, "=Width*0.58", "=Height*0.56", "=MIN(Width,Height)*0.10"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_l2_switch" | "net_poe_switch" | "net_fiber_switch" | "net_din_switch" => {
            detail.rows = vec![
                // Opposing Visio L2 switch horizontal arrows + front RJ45/SFP port bank
                GeometryRow::move_to(1, "=Width*0.14", "=Height*0.70"),
                GeometryRow::line_to(2, "=Width*0.48", "=Height*0.70"),
                GeometryRow::line_to(3, "=Width*0.40", "=Height*0.84"),
                GeometryRow::move_to(4, "=Width*0.86", "=Height*0.70"),
                GeometryRow::line_to(5, "=Width*0.52", "=Height*0.70"),
                GeometryRow::line_to(6, "=Width*0.60", "=Height*0.56"),
                GeometryRow::move_to(7, "=Width*0.14", "=Height*0.26"),
                GeometryRow::line_to(8, "=Width*0.86", "=Height*0.26"),
                GeometryRow::move_to(9, "=Width*0.32", "=Height*0.14"),
                GeometryRow::line_to(10, "=Width*0.32", "=Height*0.38"),
                GeometryRow::move_to(11, "=Width*0.50", "=Height*0.14"),
                GeometryRow::line_to(12, "=Width*0.50", "=Height*0.38"),
                GeometryRow::move_to(13, "=Width*0.68", "=Height*0.14"),
                GeometryRow::line_to(14, "=Width*0.68", "=Height*0.38"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_l3_switch" | "net_chassis_sw" => {
            detail.rows = vec![
                // Classic Visio Layer-3 Multilayer Switch: central hub + 8 radial switching rays
                GeometryRow::move_to(1, "=Width*0.34", "=Height*0.50"),
                GeometryRow::arc_to(2, "=Width*0.66", "=Height*0.50", "=MIN(Width,Height)*0.16"),
                GeometryRow::arc_to(3, "=Width*0.34", "=Height*0.50", "=MIN(Width,Height)*0.16"),
                GeometryRow::move_to(4, "=Width*0.50", "=Height*0.12"),
                GeometryRow::line_to(5, "=Width*0.50", "=Height*0.88"),
                GeometryRow::move_to(6, "=Width*0.12", "=Height*0.50"),
                GeometryRow::line_to(7, "=Width*0.88", "=Height*0.50"),
                GeometryRow::move_to(8, "=Width*0.20", "=Height*0.20"),
                GeometryRow::line_to(9, "=Width*0.80", "=Height*0.80"),
                GeometryRow::move_to(10, "=Width*0.20", "=Height*0.80"),
                GeometryRow::line_to(11, "=Width*0.80", "=Height*0.20"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_edge_router" | "net_isr_router" | "net_sdwan" | "net_rev_proxy" => {
            detail.rows = vec![
                // 4-way inward/outward routing crosshairs
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.84", "=Height*0.50"),
                GeometryRow::move_to(3, "=Width*0.50", "=Height*0.16"),
                GeometryRow::line_to(4, "=Width*0.50", "=Height*0.84"),
                GeometryRow::move_to(5, "=Width*0.32", "=Height*0.64"),
                GeometryRow::line_to(6, "=Width*0.16", "=Height*0.50"),
                GeometryRow::line_to(7, "=Width*0.32", "=Height*0.36"),
                GeometryRow::move_to(8, "=Width*0.68", "=Height*0.64"),
                GeometryRow::line_to(9, "=Width*0.84", "=Height*0.50"),
                GeometryRow::line_to(10, "=Width*0.68", "=Height*0.36"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_mesh_ap" | "net_wlc" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.20", "=Height*0.62"),
                GeometryRow::arc_to(2, "=Width*0.80", "=Height*0.62", "=MIN(Width,Height)*0.18"),
                GeometryRow::move_to(3, "=Width*0.32", "=Height*0.44"),
                GeometryRow::arc_to(4, "=Width*0.68", "=Height*0.44", "=MIN(Width,Height)*0.10"),
                GeometryRow::move_to(5, "=Width*0.18", "=Height*0.22"),
                GeometryRow::line_to(6, "=Width*0.82", "=Height*0.22"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_patch_panel" | "net_odf" | "net_pdu" | "net_modem" | "net_media_conv" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.08", "=Height*0.50"),
                GeometryRow::line_to(2, "=Width*0.92", "=Height*0.50"),
                GeometryRow::move_to(3, "=Width*0.22", "=Height*0.20"),
                GeometryRow::line_to(4, "=Width*0.22", "=Height*0.80"),
                GeometryRow::move_to(5, "=Width*0.38", "=Height*0.20"),
                GeometryRow::line_to(6, "=Width*0.38", "=Height*0.80"),
                GeometryRow::move_to(7, "=Width*0.54", "=Height*0.20"),
                GeometryRow::line_to(8, "=Width*0.54", "=Height*0.80"),
                GeometryRow::move_to(9, "=Width*0.70", "=Height*0.20"),
                GeometryRow::line_to(10, "=Width*0.70", "=Height*0.80"),
                GeometryRow::move_to(11, "=Width*0.84", "=Height*0.20"),
                GeometryRow::line_to(12, "=Width*0.84", "=Height*0.80"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_nas_san" | "net_tape_lib" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.25", "=Height*0.14"),
                GeometryRow::line_to(2, "=Width*0.25", "=Height*0.86"),
                GeometryRow::move_to(3, "=Width*0.50", "=Height*0.14"),
                GeometryRow::line_to(4, "=Width*0.50", "=Height*0.86"),
                GeometryRow::move_to(5, "=Width*0.75", "=Height*0.14"),
                GeometryRow::line_to(6, "=Width*0.75", "=Height*0.86"),
                GeometryRow::move_to(7, "=Width*0.10", "=Height*0.72"),
                GeometryRow::line_to(8, "=Width*0.90", "=Height*0.72"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_ups" => {
            detail.rows = vec![
                // UPS lightning bolt + battery level bars
                GeometryRow::move_to(1, "=Width*0.56", "=Height*0.84"),
                GeometryRow::line_to(2, "=Width*0.36", "=Height*0.52"),
                GeometryRow::line_to(3, "=Width*0.58", "=Height*0.52"),
                GeometryRow::line_to(4, "=Width*0.42", "=Height*0.22"),
                GeometryRow::move_to(5, "=Width*0.20", "=Height*0.12"),
                GeometryRow::line_to(6, "=Width*0.80", "=Height*0.12"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "cloud_vpc" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.12", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width*0.88", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.22", "=Height*0.36"),
                GeometryRow::arc_to(4, "=Width*0.78", "=Height*0.36", "=MIN(Width,Height)*0.22"),
                GeometryRow::line_to(5, "=Width*0.22", "=Height*0.36"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "net_voip" | "net_ip_cam" | "net_printer" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.32"),
                GeometryRow::line_to(2, "=Width*0.76", "=Height*0.32"),
                GeometryRow::line_to(3, "=Width*0.76", "=Height*0.72"),
                GeometryRow::line_to(4, "=Width*0.24", "=Height*0.72"),
                GeometryRow::line_to(5, "=Width*0.24", "=Height*0.32"),
                GeometryRow::move_to(6, "=Width*0.36", "=Height*0.18"),
                GeometryRow::line_to(7, "=Width*0.64", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }

        // --- Additional Microsoft 365, Servers/AD, Azure & Power Platform Masters ---
        "office_visio" => {
            detail.rows = vec![
                // Connected Visio diamond -> rectangle diagram inside card
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.66"),
                GeometryRow::line_to(2, "=Width*0.38", "=Height*0.80"),
                GeometryRow::line_to(3, "=Width*0.52", "=Height*0.66"),
                GeometryRow::line_to(4, "=Width*0.38", "=Height*0.52"),
                GeometryRow::line_to(5, "=Width*0.24", "=Height*0.66"),
                GeometryRow::move_to(6, "=Width*0.52", "=Height*0.66"),
                GeometryRow::line_to(7, "=Width*0.78", "=Height*0.66"),
                GeometryRow::line_to(8, "=Width*0.78", "=Height*0.32"),
                GeometryRow::move_to(9, "=Width*0.58", "=Height*0.18"),
                GeometryRow::line_to(10, "=Width*0.86", "=Height*0.18"),
                GeometryRow::line_to(11, "=Width*0.86", "=Height*0.36"),
                GeometryRow::line_to(12, "=Width*0.58", "=Height*0.36"),
                GeometryRow::line_to(13, "=Width*0.58", "=Height*0.18"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "office_project" | "m365_planner" => {
            detail.rows = vec![
                // Cascade Gantt bars inside Microsoft Project / Planner card
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width*0.54", "=Height*0.76"),
                GeometryRow::move_to(3, "=Width*0.30", "=Height*0.50"),
                GeometryRow::line_to(4, "=Width*0.72", "=Height*0.50"),
                GeometryRow::move_to(5, "=Width*0.46", "=Height*0.24"),
                GeometryRow::line_to(6, "=Width*0.86", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "office_access" | "az_sql_db" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.16", "=Height*0.76"),
                GeometryRow::arc_to(2, "=Width*0.84", "=Height*0.76", "=MIN(Width,Height)*0.10"),
                GeometryRow::arc_to(3, "=Width*0.16", "=Height*0.76", "=MIN(Width,Height)*0.10"),
                GeometryRow::move_to(4, "=Width*0.16", "=Height*0.48"),
                GeometryRow::line_to(5, "=Width*0.84", "=Height*0.48"),
                GeometryRow::move_to(6, "=Width*0.16", "=Height*0.24"),
                GeometryRow::line_to(7, "=Width*0.84", "=Height*0.24"),
            ];
            sh.sheet.add_geometry(detail);
        }
        r if r.starts_with("office_")
            || r.starts_with("m365_")
            || r.starts_with("ms_")
            || r.starts_with("az_")
            || r.starts_with("pp_")
            || r.starts_with("d365_") =>
        {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.22", "=Height*0.70"),
                GeometryRow::line_to(2, "=Width*0.78", "=Height*0.70"),
                GeometryRow::move_to(3, "=Width*0.22", "=Height*0.50"),
                GeometryRow::line_to(4, "=Width*0.78", "=Height*0.50"),
                GeometryRow::move_to(5, "=Width*0.34", "=Height*0.26"),
                GeometryRow::line_to(6, "=Width*0.66", "=Height*0.26"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.82"),
                GeometryRow::line_to(8, "=Width*0.34", "=Height*0.26"),
            ];
            sh.sheet.add_geometry(detail);
        }

        // --- Extended Domain Shapes Across Basic, Flowchart, BPMN, UML, DB, Floor Plan, Electrical, P&ID, Wireframe, Schedule & VSM ---
        "basic_snip_rect" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "=Width", "0 in"),
                GeometryRow::line_to(3, "=Width", "=Height*0.76"),
                GeometryRow::line_to(4, "=Width*0.82", "=Height"),
                GeometryRow::line_to(5, "0 in", "=Height"),
                GeometryRow::line_to(6, "0 in", "0 in"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "basic_semicircle" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.15"),
                GeometryRow::arc_to(2, "=Width", "=Height*0.15", "=MIN(Width,Height)*0.42"),
                GeometryRow::line_to(3, "0 in", "=Height*0.15"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "basic_pie" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "=Width*0.15", "=Height*0.15"),
                GeometryRow::line_to(2, "=Width*0.90", "=Height*0.15"),
                GeometryRow::arc_to(3, "=Width*0.15", "=Height*0.90", "=MIN(Width,Height)*-0.26"),
                GeometryRow::line_to(4, "=Width*0.15", "=Height*0.15"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "basic_inv_triangle" => {
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height"),
                GeometryRow::line_to(2, "=Width", "=Height"),
                GeometryRow::line_to(3, "=Width*0.50", "0 in"),
                GeometryRow::line_to(4, "0 in", "=Height"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "bpmn_signal_start" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width*0.76", "=Height*0.28"),
                GeometryRow::line_to(3, "=Width*0.24", "=Height*0.28"),
                GeometryRow::line_to(4, "=Width*0.50", "=Height*0.76"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_cond_event" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.30", "=Height*0.26"),
                GeometryRow::line_to(2, "=Width*0.70", "=Height*0.26"),
                GeometryRow::line_to(3, "=Width*0.70", "=Height*0.74"),
                GeometryRow::line_to(4, "=Width*0.30", "=Height*0.74"),
                GeometryRow::line_to(5, "=Width*0.30", "=Height*0.26"),
                GeometryRow::move_to(6, "=Width*0.36", "=Height*0.58"),
                GeometryRow::line_to(7, "=Width*0.64", "=Height*0.58"),
                GeometryRow::move_to(8, "=Width*0.36", "=Height*0.42"),
                GeometryRow::line_to(9, "=Width*0.64", "=Height*0.42"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_catch_msg" | "bpmn_throw_msg" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.34"),
                GeometryRow::line_to(2, "=Width*0.76", "=Height*0.34"),
                GeometryRow::line_to(3, "=Width*0.76", "=Height*0.66"),
                GeometryRow::line_to(4, "=Width*0.24", "=Height*0.66"),
                GeometryRow::line_to(5, "=Width*0.24", "=Height*0.34"),
                GeometryRow::move_to(6, "=Width*0.24", "=Height*0.66"),
                GeometryRow::line_to(7, "=Width*0.50", "=Height*0.48"),
                GeometryRow::line_to(8, "=Width*0.76", "=Height*0.66"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_error_ev" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.28", "=Height*0.26"),
                GeometryRow::line_to(2, "=Width*0.46", "=Height*0.72"),
                GeometryRow::line_to(3, "=Width*0.56", "=Height*0.44"),
                GeometryRow::line_to(4, "=Width*0.72", "=Height*0.74"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_esc_ev" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.50", "=Height*0.76"),
                GeometryRow::line_to(2, "=Width*0.74", "=Height*0.26"),
                GeometryRow::line_to(3, "=Width*0.50", "=Height*0.44"),
                GeometryRow::line_to(4, "=Width*0.26", "=Height*0.26"),
                GeometryRow::line_to(5, "=Width*0.50", "=Height*0.76"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_term_end" => {
            detail.no_fill = "FALSE".into();
            detail.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.5".into(),
                a: Some("=Width*0.78".into()),
                b: Some("=Height*0.5".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height*0.78".into()),
                e: None,
                deleted: false,
            }];
            sh.sheet.add_geometry(detail);
            sh.sheet
                .set_formula("Geometry2.FillForegnd", "=RGB(220,38,38)", Some(Unit::Color));
        }
        "bpmn_event_gw" => {
            detail.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.5".into(),
                a: Some("=Width*0.74".into()),
                b: Some("=Height*0.5".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height*0.74".into()),
                e: None,
                deleted: false,
            }];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_send_task" | "bpmn_recv_task" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.07", "=Height*0.66"),
                GeometryRow::line_to(2, "=Width*0.23", "=Height*0.66"),
                GeometryRow::line_to(3, "=Width*0.23", "=Height*0.88"),
                GeometryRow::line_to(4, "=Width*0.07", "=Height*0.88"),
                GeometryRow::line_to(5, "=Width*0.07", "=Height*0.66"),
                GeometryRow::move_to(6, "=Width*0.07", "=Height*0.88"),
                GeometryRow::line_to(7, "=Width*0.15", "=Height*0.75"),
                GeometryRow::line_to(8, "=Width*0.23", "=Height*0.88"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "bpmn_script_task" | "bpmn_rule_task" | "bpmn_manual_task" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.07", "=Height*0.88"),
                GeometryRow::line_to(2, "=Width*0.22", "=Height*0.88"),
                GeometryRow::move_to(3, "=Width*0.07", "=Height*0.78"),
                GeometryRow::line_to(4, "=Width*0.22", "=Height*0.78"),
                GeometryRow::move_to(5, "=Width*0.07", "=Height*0.68"),
                GeometryRow::line_to(6, "=Width*0.18", "=Height*0.68"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "uml_flow_final" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "=Width*0.28", "=Height*0.28"),
                GeometryRow::line_to(2, "=Width*0.72", "=Height*0.72"),
                GeometryRow::move_to(3, "=Width*0.72", "=Height*0.28"),
                GeometryRow::line_to(4, "=Width*0.28", "=Height*0.72"),
            ];
            sh.sheet.add_geometry(detail);
        }
        _ => {
            // Pure canonical Visio shapes (`Start Event`, `End Event`, `Task`, `Process`, `Decision`,
            // `Rectangle`, `Circle`, `Diamond`, etc.) remain clean without spurious interior lines!
        }
    }
}

/// Deterministic vector SVG preview & master metadata descriptor for UI stencil galleries across all 4 OS shells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StencilMasterPreview {
    pub name: String,
    pub prompt: String,
    pub category: String,
    pub width_in: f64,
    pub height_in: f64,
    pub fill_formula: String,
    pub line_formula: String,
    pub svg: String,
}

/// Render every built-in stencil master across all 22 Visio, Microsoft, Server & Network categories into a crisp,
/// proportional standalone SVG (`1.50 in × 1.00 in` mini-sheet evaluated through `ReactiveEngine` + `export_page_svg`).
pub fn list_builtin_stencil_previews() -> BTreeMap<String, Vec<StencilMasterPreview>> {
    let packs = build_all_builtin_stencils();
    let mut out: BTreeMap<String, Vec<StencilMasterPreview>> = BTreeMap::new();
    let preview_w_in = 1.50;
    let preview_h_in = 1.00;
    let max_box_w = 1.24;
    let max_box_h = 0.80;

    let mut mini_doc = VisioDocument::new_blank("StencilPreview");
    if let Some(page) = mini_doc.page_mut(1) {
        page.page_sheet.set_formula(
            "PageWidth",
            format!("{} in", format_clean_f64(preview_w_in)),
            Some(Unit::Inch),
        );
        page.page_sheet.set_formula(
            "PageHeight",
            format!("{} in", format_clean_f64(preview_h_in)),
            Some(Unit::Inch),
        );
        page.page_sheet
            .set_formula("PageColor", "=RGB(255,255,255)", Some(Unit::Color));
    }

    for pack in packs {
        let mut items = Vec::with_capacity(pack.masters.len());
        for master in &pack.masters {
            let mut orig_w = 1.5;
            let mut orig_h = 1.0;
            let mut fill_formula = "=RGB(239,246,255)".to_string();
            let mut line_formula = "=RGB(37,99,235)".to_string();

            if let Some(page) = mini_doc.page_mut(1) {
                page.shapes.clear();
                if let Some(tpl) = master.shapes.first() {
                    let mut sh = tpl.clone();
                    sh.id = 1;
                    sh.text.clear(); // Pure crisp vector symbol preview without overlapping mini-text
                    orig_w = sh.sheet.get_f64("Width").unwrap_or(1.5).max(0.15);
                    orig_h = sh.sheet.get_f64("Height").unwrap_or(1.0).max(0.15);
                    if let Some(fc) = sh.sheet.get_cell("FillForegnd") {
                        fill_formula = fc.formula.clone();
                    }
                    if let Some(lc) = sh.sheet.get_cell("LineColor") {
                        line_formula = lc.formula.clone();
                    }
                    let scale: f64 = (max_box_w / orig_w).min(max_box_h / orig_h);
                    let fit_w = (orig_w * scale).max(0.04);
                    let fit_h = (orig_h * scale).max(0.04);

                    sh.sheet.set_formula(
                        "Width",
                        format!("{} in", format_clean_f64(fit_w)),
                        Some(Unit::Inch),
                    );
                    sh.sheet.set_formula(
                        "Height",
                        format!("{} in", format_clean_f64(fit_h)),
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
            let svg = crate::export_print::export_page_svg(&mini_doc, 1);
            items.push(StencilMasterPreview {
                name: master.name.clone(),
                prompt: master.prompt.clone(),
                category: pack.category.clone(),
                width_in: orig_w,
                height_in: orig_h,
                fill_formula,
                line_formula,
                svg,
            });
        }
        out.insert(pack.title, items);
    }
    out
}

#[cfg(test)]
mod stencil_preview_tests {
    use super::*;

    #[test]
    fn all_builtin_stencil_masters_generate_distinct_vector_svg_previews() {
        let previews = list_builtin_stencil_previews();
        assert_eq!(previews.len(), 22, "expected 22 stencil categories");
        let total: usize = previews.values().map(|v| v.len()).sum();
        assert!(
            total >= 500,
            "expected >= 500 high-definition masters across 22 packs, got {total}"
        );
        for (pack_name, masters) in &previews {
            assert!(
                masters.len() >= 18,
                "every stencil category must have >= 18 rich stencils; {pack_name} only had {}",
                masters.len()
            );
            for m in masters {
                assert!(m.svg.contains("<svg"), "missing <svg> in {}:{}", pack_name, m.name);
                let grp_idx = m
                    .svg
                    .find("data-shape-id=")
                    .unwrap_or_else(|| panic!("missing data-shape-id in {}:{}", pack_name, m.name));
                let shape_body = &m.svg[grp_idx..];
                let path_count = shape_body.matches("<path ").count();
                assert!(
                    path_count >= 1,
                    "missing native-compatible <path> in {}:{} (macOS/Android/Windows require <path>)",
                    pack_name,
                    m.name
                );
                assert!(
                    !shape_body.contains("<svg "),
                    "unexpected nested <svg> inside {}:{} that breaks native stencil renderers",
                    pack_name,
                    m.name
                );
                if pack_name.starts_with("Microsoft")
                    || pack_name.starts_with("Servers")
                    || pack_name.starts_with("Network")
                {
                    assert!(
                        path_count >= 3,
                        "Stencil {}:{} must have rich multi-path 1:1 Visio detail (got {path_count})",
                        pack_name,
                        m.name
                    );
                }
            }
        }
    }
}



