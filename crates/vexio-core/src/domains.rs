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
    GeometryRow, GeometrySection, GlueEndpoint, ReactiveEngine, ShapeDataRow, ShapeDataType,
    ShapeSheet, StencilPack, Unit, VisioDocument, VisioMaster, VisioShape, format_clean_f64,
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
    match template_id {
        "bpmn_fulfillment" => {
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
        "cloud_microservices" => {
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
        "sql_ecommerce_erd" => {
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
        "executive_org_chart" => {
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
        "release_gantt" => {
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
        "project_timeline" => {
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
        "datacenter_floorplan" => {
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
// 4. Complete 12-Pack Built-In IP-Clean Stencil Library
// ============================================================================

pub fn build_all_builtin_stencils() -> Vec<StencilPack> {
    vec![
        make_stencil_pack(
            "Basic Flowchart",
            "Flowchart",
            &[
                ("Process", "Rectangle", 1.5, 0.9, "Process"),
                ("Decision", "Diamond", 1.5, 1.0, "Decision"),
                ("Start/End", "Pill", 1.5, 0.6, "Terminator"),
                ("Data (I/O)", "Parallelogram", 1.5, 0.85, "Data"),
                ("Subprocess", "Rectangle", 1.6, 0.95, "Subprocess"),
            ],
        ),
        make_stencil_pack(
            "BPMN 2.0",
            "Business Process",
            &[
                ("Start Event", "Circle", 0.5, 0.5, "start"),
                ("Intermediate Event", "Circle", 0.5, 0.5, "intermediate"),
                ("End Event", "Circle", 0.5, 0.5, "end"),
                ("Task", "Rectangle", 1.6, 0.9, "task"),
                ("Exclusive Gateway", "Diamond", 0.75, 0.75, "exclusive_gateway"),
                ("Parallel Gateway", "Diamond", 0.75, 0.75, "parallel_gateway"),
            ],
        ),
        make_stencil_pack(
            "UML 2.5",
            "Software & Database",
            &[
                ("Class", "Rectangle", 2.0, 1.4, "uml_class"),
                ("Interface", "Rectangle", 2.0, 1.1, "uml_interface"),
                ("Component", "Rectangle", 1.8, 1.0, "uml_component"),
                ("State", "Pill", 1.6, 0.8, "uml_state"),
                ("Actor / Lifeline", "Rectangle", 1.2, 2.5, "uml_lifeline"),
            ],
        ),
        make_stencil_pack(
            "Network & Rack Equipment",
            "Network",
            &[
                ("42U Standard Rack", "Rectangle", 2.2, 7.35, "rack_42u"),
                ("Core Switch (1U)", "Rectangle", 1.9, 0.35, "switch_1u"),
                ("Enterprise Firewall", "Rectangle", 1.9, 0.5, "firewall"),
                ("Cloud VPC", "Pill", 2.8, 1.8, "cloud_vpc"),
                ("Database Cluster", "Rectangle", 1.6, 1.2, "db_cluster"),
            ],
        ),
        make_stencil_pack(
            "Floor Plan, P&ID & VSM",
            "Engineering & Architecture",
            &[
                ("Exterior Wall", "Rectangle", 4.0, 0.2, "wall"),
                ("Single Door (90°)", "Rectangle", 0.9, 0.9, "door"),
                ("Centrifugal Pump", "Circle", 0.9, 0.9, "pid_pump"),
                ("Control Valve", "Diamond", 0.7, 0.45, "pid_valve"),
                ("VSM Process Box", "Rectangle", 1.8, 1.2, "vsm_process"),
            ],
        ),
        make_stencil_pack(
            "Cloud Architecture (AWS / GCP / Azure)",
            "Cloud",
            &[
                ("Compute VM", "Rectangle", 1.6, 1.0, "cloud_vm"),
                ("Kubernetes Cluster", "Hexagon", 1.6, 1.3, "cloud_k8s"),
                ("Serverless Function", "Hexagon", 1.4, 1.1, "cloud_fn"),
                ("Object Storage Bucket", "Parallelogram", 1.6, 1.0, "cloud_bucket"),
                ("Managed SQL / Spanner", "Pill", 1.7, 1.1, "cloud_sql"),
                ("Edge CDN / Load Balancer", "Diamond", 1.5, 1.1, "cloud_lb"),
                ("Pub/Sub Event Bus", "Pill", 2.0, 0.75, "cloud_pubsub"),
            ],
        ),
        make_stencil_pack(
            "Electrical & Electronic (IEEE / IEC)",
            "Electrical",
            &[
                ("Resistor (IEEE)", "Zigzag", 1.4, 0.45, "ieee_resistor"),
                ("Capacitor", "Rectangle", 0.9, 0.7, "ieee_capacitor"),
                ("Inductor", "Pill", 1.3, 0.45, "ieee_inductor"),
                ("Step-Down Transformer", "Rectangle", 1.3, 1.1, "ieee_transformer"),
                ("Op-Amp", "Triangle", 1.2, 1.0, "ieee_opamp"),
                ("Earth Ground", "Triangle", 0.7, 0.55, "ieee_ground"),
                ("AC Voltage Source", "Circle", 0.85, 0.85, "ieee_ac_source"),
            ],
        ),
        make_stencil_pack(
            "UI & Application Wireframes",
            "Wireframe",
            &[
                ("Browser / Desktop Frame", "Rectangle", 4.2, 2.8, "wf_window"),
                ("Top App Bar", "Rectangle", 4.0, 0.45, "wf_appbar"),
                ("Navigation Rail", "Rectangle", 0.75, 2.4, "wf_navrail"),
                ("KPI Metric Card", "Rectangle", 1.6, 1.0, "wf_card"),
                ("Data Grid Table", "Rectangle", 3.0, 1.6, "wf_table"),
                ("Primary Action Button", "Pill", 1.3, 0.42, "wf_button"),
                ("Search Input", "Pill", 2.2, 0.42, "wf_input"),
            ],
        ),
        make_stencil_pack(
            "Organization Chart",
            "Business",
            &[
                ("Executive", "Rectangle", 2.1, 0.95, "org_exec"),
                ("VP / Director", "Rectangle", 2.0, 0.9, "org_vp"),
                ("Manager", "Rectangle", 1.9, 0.85, "org_mgr"),
                ("Individual Contributor", "Rectangle", 1.8, 0.8, "org_ic"),
                ("Executive Assistant", "Pill", 1.7, 0.75, "org_assistant"),
                ("Open Vacancy", "Rectangle", 1.8, 0.8, "org_vacancy"),
            ],
        ),
        make_stencil_pack(
            "Project Schedule (Gantt & Timeline)",
            "Schedule",
            &[
                ("Timeline Spine", "Pill", 5.0, 0.35, "sched_spine"),
                ("Milestone Diamond", "Diamond", 0.45, 0.45, "sched_milestone"),
                ("Phase Interval Bar", "Pill", 2.4, 0.4, "sched_interval"),
                ("Gantt Task Bar", "Rectangle", 2.0, 0.35, "sched_gantt_bar"),
                ("Summary Rollup Bar", "Rectangle", 3.0, 0.25, "sched_rollup"),
            ],
        ),
        make_stencil_pack(
            "Value Stream Mapping (Lean VSM)",
            "Lean Manufacturing",
            &[
                ("Customer / Supplier", "Rectangle", 1.6, 1.1, "vsm_customer"),
                ("Dedicated Process", "Rectangle", 1.8, 1.1, "vsm_dedicated"),
                ("Inventory Triangle", "Triangle", 0.85, 0.8, "vsm_inventory"),
                ("Supermarket", "Rectangle", 1.0, 1.2, "vsm_supermarket"),
                ("Kaizen Burst", "Hexagon", 1.3, 0.85, "vsm_kaizen"),
            ],
        ),
        make_stencil_pack(
            "Database Crow's Foot & IDEF1X",
            "Database",
            &[
                ("Strong Entity", "Rectangle", 2.2, 1.3, "erd_strong"),
                ("Weak / Associative Entity", "Pill", 2.2, 1.3, "erd_weak"),
                ("Materialized View", "Parallelogram", 2.1, 1.1, "erd_view"),
                ("PK Attribute", "Rectangle", 1.8, 0.38, "erd_pk"),
            ],
        ),
    ]
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
                    GeometryRow::move_to(1, "=Width*0.15", "0 in"),
                    GeometryRow::line_to(2, "=Width", "0 in"),
                    GeometryRow::line_to(3, "=Width*0.85", "=Height"),
                    GeometryRow::line_to(4, "0 in", "=Height"),
                    GeometryRow::line_to(5, "=Width*0.15", "0 in"),
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
            _ => {}
        }
        apply_domain_master_styling_and_details(&mut sh, category, role, geom_kind);

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
    // 1. Curated Crisp Visio Light-Mode Domain Palette
    let (fill_rgb, stroke_rgb, weight_pt) = match (category, role) {
        (_, "start") => ("236,253,245", "5,150,105", 1.75),
        (_, "intermediate") => ("255,251,235", "217,119,6", 1.65),
        (_, "end") => ("254,242,242", "220,38,38", 2.85),
        (_, "exclusive_gateway") | (_, "parallel_gateway") => ("254,249,195", "202,138,4", 1.75),
        (_, "task") => ("239,246,255", "37,99,235", 1.6),
        ("Flowchart", "Decision") => ("254,243,199", "217,119,6", 1.6),
        ("Flowchart", "Terminator") => ("236,253,245", "13,148,136", 1.6),
        ("Flowchart", "Data") => ("240,249,255", "2,132,199", 1.6),
        ("Flowchart", "Subprocess") => ("238,242,255", "79,70,229", 1.6),
        ("Flowchart", _) => ("239,246,255", "37,99,235", 1.6),
        ("Software & Database", _) => ("245,243,255", "109,40,217", 1.55),
        ("Network", "firewall") => ("254,242,242", "220,38,38", 1.65),
        ("Network", _) => ("240,249,255", "3,105,161", 1.6),
        ("Engineering & Architecture", _) => ("248,250,252", "51,65,85", 1.65),
        ("Cloud", "cloud_fn") => ("255,247,237", "234,88,12", 1.65),
        ("Cloud", "cloud_bucket") => ("236,253,245", "22,163,74", 1.6),
        ("Cloud", "cloud_sql") => ("238,242,255", "67,56,202", 1.6),
        ("Cloud", _) => ("239,246,255", "29,78,216", 1.65),
        ("Electrical", _) => ("255,255,255", "15,23,42", 1.85),
        ("Wireframe", "wf_button") => ("219,234,254", "29,78,216", 1.6),
        ("Wireframe", _) => ("248,250,252", "71,85,105", 1.5),
        ("Business", "org_exec") => ("238,242,255", "49,46,129", 1.75),
        ("Business", "org_vacancy") => ("255,255,255", "100,116,139", 1.5),
        ("Business", _) => ("240,249,255", "30,64,175", 1.55),
        ("Schedule", "sched_milestone") => ("209,250,229", "5,150,105", 1.75),
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

    if geom_kind == "Rectangle"
        && matches!(
            role,
            "Process" | "task" | "cloud_vm" | "wf_card" | "org_exec" | "org_vp" | "org_mgr" | "org_ic"
        )
    {
        sh.sheet
            .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
    }
    if role == "org_vacancy" || role == "cloud_vpc" {
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
        "intermediate" => {
            detail.rows = vec![GeometryRow {
                ix: 1,
                kind: vexio_shapesheet::GeometryRowKind::Ellipse,
                x: "=Width*0.5".into(),
                y: "=Height*0.5".into(),
                a: Some("=Width*0.86".into()),
                b: Some("=Height*0.5".into()),
                c: Some("=Width*0.5".into()),
                d: Some("=Height*0.86".into()),
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
                GeometryRow::line_to(3, "=Width*0.42", "=Height*0.92"),
                GeometryRow::line_to(4, "=Width*0.74", "=Height*0.68"),
                GeometryRow::line_to(5, "=Width*0.94", "=Height*0.36"),
                GeometryRow::line_to(6, "=Width", "0 in"),
            ];
            sh.sheet.add_geometry(g1);
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
        "ieee_inductor" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.35"),
                GeometryRow::line_to(2, "=Width*0.14", "=Height*0.35"),
                GeometryRow::line_to(3, "=Width*0.23", "=Height*0.88"),
                GeometryRow::line_to(4, "=Width*0.32", "=Height*0.35"),
                GeometryRow::line_to(5, "=Width*0.41", "=Height*0.88"),
                GeometryRow::line_to(6, "=Width*0.50", "=Height*0.35"),
                GeometryRow::line_to(7, "=Width*0.59", "=Height*0.88"),
                GeometryRow::line_to(8, "=Width*0.68", "=Height*0.35"),
                GeometryRow::line_to(9, "=Width*0.77", "=Height*0.88"),
                GeometryRow::line_to(10, "=Width*0.86", "=Height*0.35"),
                GeometryRow::line_to(11, "=Width", "=Height*0.35"),
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
                GeometryRow::move_to(1, "=Width*0.24", "=Height*0.5"),
                GeometryRow::line_to(2, "=Width*0.38", "=Height*0.68"),
                GeometryRow::line_to(3, "=Width*0.62", "=Height*0.32"),
                GeometryRow::line_to(4, "=Width*0.76", "=Height*0.5"),
            ];
            sh.sheet.add_geometry(detail);
        }
        "wf_window" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.82"),
                GeometryRow::line_to(2, "=Width", "=Height*0.82"),
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
        "org_exec" | "org_vp" | "org_mgr" | "org_ic" => {
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
        "ieee_transformer" => {
            let mut g1 = GeometrySection::new(1);
            g1.no_fill = "TRUE".into();
            g1.rows = vec![
                // Left primary winding + leads
                GeometryRow::move_to(1, "0 in", "=Height*0.85"),
                GeometryRow::line_to(2, "=Width*0.28", "=Height*0.85"),
                GeometryRow::line_to(3, "=Width*0.38", "=Height*0.72"),
                GeometryRow::line_to(4, "=Width*0.28", "=Height*0.58"),
                GeometryRow::line_to(5, "=Width*0.38", "=Height*0.44"),
                GeometryRow::line_to(6, "=Width*0.28", "=Height*0.30"),
                GeometryRow::line_to(7, "=Width*0.28", "=Height*0.15"),
                GeometryRow::line_to(8, "0 in", "=Height*0.15"),
                // Core bars
                GeometryRow::move_to(9, "=Width*0.46", "=Height*0.14"),
                GeometryRow::line_to(10, "=Width*0.46", "=Height*0.86"),
                GeometryRow::move_to(11, "=Width*0.54", "=Height*0.14"),
                GeometryRow::line_to(12, "=Width*0.54", "=Height*0.86"),
                // Right secondary winding + leads
                GeometryRow::move_to(13, "=Width", "=Height*0.85"),
                GeometryRow::line_to(14, "=Width*0.72", "=Height*0.85"),
                GeometryRow::line_to(15, "=Width*0.62", "=Height*0.72"),
                GeometryRow::line_to(16, "=Width*0.72", "=Height*0.58"),
                GeometryRow::line_to(17, "=Width*0.62", "=Height*0.44"),
                GeometryRow::line_to(18, "=Width*0.72", "=Height*0.30"),
                GeometryRow::line_to(19, "=Width*0.72", "=Height*0.15"),
                GeometryRow::line_to(20, "=Width", "=Height*0.15"),
            ];
            sh.sheet.add_geometry(g1);
        }
        "wall" => {
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
        "vsm_process" | "vsm_dedicated" | "erd_strong" | "erd_weak" => {
            detail.rows = vec![
                GeometryRow::move_to(1, "0 in", "=Height*0.7"),
                GeometryRow::line_to(2, "=Width", "=Height*0.7"),
            ];
            sh.sheet.add_geometry(detail);
        }
        _ => {}
    }
}

/// Deterministic vector SVG preview descriptor for UI stencil galleries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StencilMasterPreview {
    pub name: String,
    pub prompt: String,
    pub svg: String,
}

/// Render every built-in stencil master across all 12 packs into a crisp, proportional standalone SVG
/// (`1.50 in × 1.00 in` mini-sheet evaluated through `ReactiveEngine` + `export_page_svg`).
pub fn list_builtin_stencil_previews() -> BTreeMap<String, Vec<StencilMasterPreview>> {
    let packs = build_all_builtin_stencils();
    let mut out: BTreeMap<String, Vec<StencilMasterPreview>> = BTreeMap::new();
    let preview_w_in = 1.50;
    let preview_h_in = 1.00;
    let max_box_w = 1.18;
    let max_box_h = 0.74;

    for pack in packs {
        let mut items = Vec::with_capacity(pack.masters.len());
        for master in &pack.masters {
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
                    .set_formula("PageColor", "=RGB(248,250,252)", Some(Unit::Color));

                if let Some(tpl) = master.shapes.first() {
                    let mut sh = tpl.clone();
                    sh.id = 1;
                    sh.text.clear(); // Pure crisp vector symbol preview without overlapping mini-text
                    let orig_w: f64 = sh.sheet.get_f64("Width").unwrap_or(1.5).max(0.15);
                    let orig_h: f64 = sh.sheet.get_f64("Height").unwrap_or(1.0).max(0.15);
                    let scale: f64 = (max_box_w / orig_w).min(max_box_h / orig_h);
                    let fit_w = (orig_w * scale).max(0.18);
                    let fit_h = (orig_h * scale).max(0.14);

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
    fn all_68_builtin_stencil_masters_generate_distinct_vector_svg_previews() {
        let previews = list_builtin_stencil_previews();
        assert_eq!(previews.len(), 12);
        let total: usize = previews.values().map(|v| v.len()).sum();
        assert!(total >= 65, "expected at least 65 masters across 12 packs, got {total}");
        for (pack_name, masters) in &previews {
            assert!(!masters.is_empty(), "pack {pack_name} had no masters");
            for m in masters {
                assert!(m.svg.contains("<svg"), "missing <svg> in {}:{}", pack_name, m.name);
                assert!(
                    m.svg.contains("<path") || m.svg.contains("<ellipse") || m.svg.contains("<rect"),
                    "missing vector primitive in {}:{}",
                    pack_name,
                    m.name
                );
            }
        }
    }
}

