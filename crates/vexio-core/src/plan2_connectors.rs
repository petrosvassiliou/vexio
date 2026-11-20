//! Microsoft Visio Plan 2 Cloud, Process & Directory Connectors:
//! - Bidirectional Excel Data Visualizer (`import_data_visualizer_csv_or_xlsx` & `export_data_visualizer_xlsx`)
//! - Microsoft Power Automate / Azure Logic Apps Workflow Exporter (`export_power_automate_flow_json`)
//! - Microsoft Power BI Visio Visual Mapping Exporter (`export_power_bi_visual_json`)
//! - Cloud Infrastructure Discovery Importer (`import_cloud_discovery_json` for AWS / Azure / GCP / Terraform)
//! - Microsoft Entra ID (Graph `/users` JSON) & RFC-2849 LDAP `.ldif` Org Chart Importer (`import_entra_or_ldif_org_chart`).

use crate::autolayout::{LayoutAlgorithm, LayoutOptions, apply_auto_layout};
use crate::containers::{
    build_cross_functional_swimlanes, create_container, sync_swimlane_shape_metadata,
};
use crate::data_link::{DataLinkError, DataRecordset};
use crate::domains::build_all_builtin_stencils;
use crate::routing::route_page_connectors;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use vexio_shapesheet::{
    EvaluatedPathVerb, GlueEndpoint, ReactiveEngine, ShapeDataRow, ShapeDataType, ShapeKind,
    StencilPack, Unit, VisioDocument, VisioShape, format_clean_f64,
};

// ============================================================================
// 1. Bidirectional Excel / CSV Data Visualizer
// ============================================================================

struct DataVisStep {
    step_id: String,
    description: String,
    next_ids: Vec<String>,
    connector_labels: Vec<String>,
    shape_type: String,
    function_lane: String,
    phase: String,
}

/// Parse a Microsoft Visio Data Visualizer table from either `.xlsx` bytes or UTF-8 `.csv` bytes,
/// automatically constructing Cross-Functional Swimlanes (`Function`), Phase metadata, BPMN/Flowchart
/// shapes, labeled dynamic connectors (`Next Step ID` + `Connector Label`), and Left-to-Right layout.
pub fn import_data_visualizer_csv_or_xlsx(
    doc: &mut VisioDocument,
    page_id: u32,
    bytes: &[u8],
) -> Result<usize, DataLinkError> {
    let recordset = if bytes.starts_with(b"PK\x03\x04") {
        DataRecordset::from_xlsx_bytes(101, "DataVisualizer", "Process Step ID", bytes)?
    } else {
        let csv_str = std::str::from_utf8(bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let detected_key = detect_csv_step_id_header(csv_str);
        DataRecordset::from_csv(101, "DataVisualizer", detected_key, csv_str)?
    };

    let mut steps = Vec::new();
    let mut lane_order: Vec<String> = Vec::new();
    let mut phase_order: Vec<String> = Vec::new();

    for row in &recordset.rows {
        let step_id = get_ci(row, &["Process Step ID", "Step ID", "ID"]);
        if step_id.is_empty() {
            continue;
        }
        let description = get_ci(
            row,
            &["Process Step Description", "Description", "Label", "Step Name"],
        );
        let next_raw = get_ci(row, &["Next Step ID", "Next Step", "Next"]);
        let label_raw = get_ci(row, &["Connector Label", "LabelOnConnector", "Branch Label"]);
        let shape_type = {
            let st = get_ci(row, &["Shape Type", "Type", "BpmnType"]);
            if st.is_empty() {
                "Process".to_string()
            } else {
                st
            }
        };
        let function_lane = get_ci(row, &["Function", "Swimlane", "Department", "Role"]);
        let phase = get_ci(row, &["Phase", "Stage", "Milestone"]);

        if !function_lane.is_empty()
            && !lane_order
                .iter()
                .any(|l| l.eq_ignore_ascii_case(&function_lane))
        {
            lane_order.push(function_lane.clone());
        }
        if !phase.is_empty() && !phase_order.iter().any(|p| p.eq_ignore_ascii_case(&phase)) {
            phase_order.push(phase.clone());
        }

        let next_ids: Vec<String> = next_raw
            .split([',', ';'])
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let connector_labels: Vec<String> = if label_raw.is_empty() {
            Vec::new()
        } else {
            label_raw
                .split([',', ';'])
                .map(|s| s.trim().to_string())
                .collect()
        };

        steps.push(DataVisStep {
            step_id,
            description,
            next_ids,
            connector_labels,
            shape_type,
            function_lane,
            phase,
        });
    }

    if steps.is_empty() {
        return Ok(0);
    }

    let lane_h = 2.2;
    let total_w = ((steps.len() as f64) * 2.3 + 2.5).max(11.0);
    let total_h = ((lane_order.len().max(1) as f64) * lane_h + 1.8).max(8.5);

    if let Some(page) = doc.page_mut(page_id) {
        page.shapes.clear();
        page.connects.clear();
        page.page_sheet.set_formula(
            "PageWidth",
            format!("{} in", format_clean_f64(total_w)),
            Some(Unit::Inch),
        );
        page.page_sheet.set_formula(
            "PageHeight",
            format!("{} in", format_clean_f64(total_h)),
            Some(Unit::Inch),
        );
    }

    let top_y = total_h - 0.8;
    let lane_w = total_w - 1.4;
    if !lane_order.is_empty() {
        let lane_refs: Vec<&str> = lane_order.iter().map(String::as_str).collect();
        build_cross_functional_swimlanes(doc, page_id, &lane_refs, 0.7, top_y, lane_w, lane_h);
    }

    let stencils = build_all_builtin_stencils();
    let mut step_shape_ids: BTreeMap<String, u64> = BTreeMap::new();
    let mut shape_lane_idx: BTreeMap<u64, usize> = BTreeMap::new();

    if let Some(page) = doc.page_mut(page_id) {
        for (idx, step) in steps.iter().enumerate() {
            let sid = page.next_shape_id();
            let mut sh = instantiate_data_vis_shape(&stencils, sid, &step.shape_type);
            sh.text = if step.description.is_empty() {
                step.step_id.clone()
            } else {
                step.description.clone()
            };

            let init_x = 2.0 + (idx as f64) * 2.1;
            let l_idx = lane_order
                .iter()
                .position(|l| l.eq_ignore_ascii_case(&step.function_lane))
                .unwrap_or(0);
            let init_y = if lane_order.is_empty() {
                total_h * 0.5
            } else {
                top_y - (l_idx as f64 + 0.5) * lane_h
            };

            sh.sheet.set_formula(
                "PinX",
                format!("{} in", format_clean_f64(init_x)),
                Some(Unit::Inch),
            );
            sh.sheet.set_formula(
                "PinY",
                format!("{} in", format_clean_f64(init_y)),
                Some(Unit::Inch),
            );

            for (k, lbl, val) in [
                ("Process_Step_ID", "Process Step ID", step.step_id.as_str()),
                ("Shape_Type", "Shape Type", step.shape_type.as_str()),
                ("Function", "Function", step.function_lane.as_str()),
                ("Phase", "Phase", step.phase.as_str()),
            ] {
                sh.sheet.set_shape_data(ShapeDataRow {
                    name: k.into(),
                    label: lbl.into(),
                    prompt: format!("Data Visualizer {lbl}"),
                    data_type: ShapeDataType::String,
                    format: String::new(),
                    value: format!("\"{}\"", val.replace('"', "\"\"")),
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }

            if !lane_order.is_empty() {
                shape_lane_idx.insert(sid, l_idx);
            }
            page.shapes.push(sh);
            step_shape_ids.insert(step.step_id.to_ascii_lowercase(), sid);
        }

        // Wire Next Step ID connectors with corresponding Connector Labels
        for step in &steps {
            let Some(&src_sid) = step_shape_ids.get(&step.step_id.to_ascii_lowercase()) else {
                continue;
            };
            for (edge_idx, next_key) in step.next_ids.iter().enumerate() {
                if let Some(&dst_sid) = step_shape_ids.get(&next_key.to_ascii_lowercase()) {
                    let cid = page.next_shape_id();
                    let mut conn = VisioShape::new_connector(
                        cid,
                        format!("Flow.{}_{}", step.step_id, next_key),
                        0.0,
                        0.0,
                        1.0,
                        0.0,
                    );
                    if let Some(lbl) = step.connector_labels.get(edge_idx) {
                        conn.text = lbl.clone();
                    } else if step.connector_labels.len() == 1 {
                        conn.text = step.connector_labels[0].clone();
                    }
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

    // Run topological Left-to-Right layout for X rank positioning
    apply_auto_layout(
        doc,
        page_id,
        LayoutOptions {
            algorithm: LayoutAlgorithm::HierarchicalLeftToRight,
            layer_spacing_in: 2.35,
            node_spacing_in: 1.6,
            margin_x_in: 2.0,
            margin_y_in: 1.4,
        },
    );

    // Preserve Cross-Functional Swimlane Y-band containment while avoiding collisions inside a lane
    if !lane_order.is_empty() {
        if let Some(page) = doc.page_mut(page_id) {
            let mut occupied_per_lane: BTreeMap<(usize, i64), usize> = BTreeMap::new();
            for (&sid, &l_idx) in &shape_lane_idx {
                if let Some(sh) = page.find_shape_mut(sid) {
                    let cur_x = sh.sheet.get_f64("PinX").unwrap_or(2.0).clamp(1.6, total_w - 1.4);
                    let bucket = (cur_x * 2.0).round() as i64;
                    let slot = occupied_per_lane.entry((l_idx, bucket)).or_insert(0);
                    let y_jitter = if *slot == 0 {
                        0.0
                    } else if *slot % 2 == 1 {
                        0.42
                    } else {
                        -0.42
                    };
                    *slot += 1;
                    let lane_cy = top_y - (l_idx as f64 + 0.5) * lane_h + y_jitter;
                    sh.sheet.set_formula(
                        "PinX",
                        format!("{} in", format_clean_f64(cur_x)),
                        Some(Unit::Inch),
                    );
                    sh.sheet.set_formula(
                        "PinY",
                        format!("{} in", format_clean_f64(lane_cy)),
                        Some(Unit::Inch),
                    );
                }
            }
        }
        sync_swimlane_shape_metadata(doc, page_id);
        route_page_connectors(doc, page_id);
    }

    Ok(step_shape_ids.len())
}

/// Reverse-extract shapes, swimlane memberships, phases, and outgoing `GlueEndpoint` connectors
/// from `page_id` back into a canonical Microsoft Visio Data Visualizer `.xlsx` workbook!
pub fn export_data_visualizer_xlsx(
    doc: &VisioDocument,
    page_id: u32,
) -> Result<Vec<u8>, DataLinkError> {
    let columns = vec![
        "Process Step ID".to_string(),
        "Process Step Description".to_string(),
        "Next Step ID".to_string(),
        "Connector Label".to_string(),
        "Shape Type".to_string(),
        "Function".to_string(),
        "Phase".to_string(),
    ];

    let Some(page) = doc.page(page_id) else {
        let empty_rs = DataRecordset {
            id: 102,
            name: "DataVisualizerExport".into(),
            key_column: "Process Step ID".into(),
            columns,
            rows: Vec::new(),
        };
        return empty_rs.to_xlsx_bytes();
    };

    // Collect swimlane bounding boxes `(title, min_x, min_y, max_x, max_y)`
    let swimlanes: Vec<(String, f64, f64, f64, f64)> = page
        .shapes
        .iter()
        .filter(|s| s.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5)
        .map(|s| {
            let px = s.sheet.get_f64("PinX").unwrap_or(0.0);
            let py = s.sheet.get_f64("PinY").unwrap_or(0.0);
            let w = s.sheet.get_f64("Width").unwrap_or(1.0);
            let h = s.sheet.get_f64("Height").unwrap_or(1.0);
            (
                s.text.clone(),
                px - w * 0.5,
                py - h * 0.5,
                px + w * 0.5,
                py + h * 0.5,
            )
        })
        .collect();

    // Collect step 2D shapes (excluding swimlanes, containers, legends, callouts)
    let step_shapes: Vec<&VisioShape> = page
        .shapes
        .iter()
        .filter(|s| is_process_step_shape(s))
        .collect();

    let mut step_id_by_shape: BTreeMap<u64, String> = BTreeMap::new();
    for (idx, sh) in step_shapes.iter().enumerate() {
        let sid_str = sh
            .sheet
            .get_cell("Prop.Process_Step_ID")
            .or_else(|| sh.sheet.get_cell("Prop.StepID"))
            .and_then(|c| c.value.as_str_coerced().ok())
            .map(|s| s.trim_matches('"').trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("P{:03}", (idx + 1) * 10));
        step_id_by_shape.insert(sh.id, sid_str);
    }

    // Build outgoing edge list per shape `(dst_shape_id, connector_label)`
    let mut beg: BTreeMap<u64, u64> = BTreeMap::new();
    let mut end: BTreeMap<u64, u64> = BTreeMap::new();
    for c in &page.connects {
        if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            beg.insert(c.connector_shape_id, c.target_shape_id);
        } else {
            end.insert(c.connector_shape_id, c.target_shape_id);
        }
    }
    let mut outgoing: BTreeMap<u64, Vec<(u64, String)>> = BTreeMap::new();
    for s in &page.shapes {
        if s.kind == ShapeKind::Connector1D {
            if let (Some(&u), Some(&v)) = (beg.get(&s.id), end.get(&s.id)) {
                if step_id_by_shape.contains_key(&u) && step_id_by_shape.contains_key(&v) {
                    outgoing
                        .entry(u)
                        .or_default()
                        .push((v, s.text.trim().to_string()));
                }
            }
        }
    }

    let mut rows = Vec::new();
    for sh in step_shapes {
        let step_id = step_id_by_shape[&sh.id].clone();
        let edges = outgoing.remove(&sh.id).unwrap_or_default();
        let next_ids: Vec<String> = edges
            .iter()
            .filter_map(|(dst, _)| step_id_by_shape.get(dst).cloned())
            .collect();
        let has_any_label = edges.iter().any(|(_, lbl)| !lbl.is_empty());
        let conn_label_str = if has_any_label {
            edges
                .iter()
                .map(|(_, lbl)| lbl.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            String::new()
        };

        let px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
        let py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
        let geom_lane = swimlanes
            .iter()
            .find(|(_, x0, y0, x1, y1)| px >= *x0 && px <= *x1 && py >= *y0 && py <= *y1)
            .map(|(name, _, _, _, _)| name.clone());
        let function_val = geom_lane.unwrap_or_else(|| {
            read_prop_str(sh, "Function").unwrap_or_default()
        });
        let phase_val = read_prop_str(sh, "Phase").unwrap_or_default();
        let shape_type_val = infer_data_vis_shape_type(sh);

        let mut row = BTreeMap::new();
        row.insert("Process Step ID".into(), step_id);
        row.insert(
            "Process Step Description".into(),
            sh.text.replace('\n', " "),
        );
        row.insert("Next Step ID".into(), next_ids.join(", "));
        row.insert("Connector Label".into(), conn_label_str);
        row.insert("Shape Type".into(), shape_type_val);
        row.insert("Function".into(), function_val);
        row.insert("Phase".into(), phase_val);
        rows.push(row);
    }

    let rs = DataRecordset {
        id: 102,
        name: "DataVisualizerExport".into(),
        key_column: "Process Step ID".into(),
        columns,
        rows,
    };
    rs.to_xlsx_bytes()
}

fn detect_csv_step_id_header(csv: &str) -> String {
    let first_line = csv.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    for col in first_line.split(',') {
        let trimmed = col.trim().trim_matches('"');
        if trimmed.eq_ignore_ascii_case("Process Step ID")
            || trimmed.eq_ignore_ascii_case("Step ID")
            || trimmed.eq_ignore_ascii_case("ID")
        {
            return trimmed.to_string();
        }
    }
    first_line
        .split(',')
        .next()
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Process Step ID".into())
}

fn get_ci(row: &BTreeMap<String, String>, candidates: &[&str]) -> String {
    for &cand in candidates {
        for (k, v) in row {
            if k.trim().eq_ignore_ascii_case(cand) {
                return v.trim().to_string();
            }
        }
    }
    String::new()
}

fn instantiate_data_vis_shape(
    packs: &[StencilPack],
    sid: u64,
    shape_type: &str,
) -> VisioShape {
    let lower = shape_type.trim().to_ascii_lowercase();
    let (pack_name, master_name, role, canonical_type) = if lower.contains("start") {
        ("BPMN 2.0", "Start Event", "start", "Start")
    } else if lower.contains("end") {
        ("BPMN 2.0", "End Event", "end", "End")
    } else if lower.contains("decision") || lower.contains("gateway") {
        (
            "BPMN 2.0",
            "Exclusive Gateway",
            "exclusive_gateway",
            "Decision",
        )
    } else if lower.contains("sub") {
        ("Basic Flowchart", "Subprocess", "subprocess", "Subprocess")
    } else if lower.contains("data") || lower.contains("io") {
        ("Basic Flowchart", "Data (I/O)", "task", "Data")
    } else {
        ("BPMN 2.0", "Task", "task", "Process")
    };

    let mut sh = find_stencil_template(packs, pack_name, master_name)
        .unwrap_or_else(|| VisioShape::new_rect(sid, master_name, 2.0, 2.0, 1.6, 0.9));
    sh.id = sid;
    sh.name = format!("{canonical_type}.{sid}");
    sh.name_u = sh.name.clone();
    sh.sheet
        .set_user_cell("DomainRole", format!("\"{role}\""), "Process Role");
    if canonical_type == "Subprocess" {
        sh.sheet
            .set_user_cell("IsSubprocess", "TRUE", "Subprocess Step");
    }
    sh
}

fn infer_data_vis_shape_type(sh: &VisioShape) -> String {
    if let Some(explicit) = read_prop_str(sh, "Shape_Type").or_else(|| read_prop_str(sh, "ShapeType")) {
        if !explicit.is_empty() {
            return explicit;
        }
    }
    if sh.sheet.get_bool("User.IsSubprocess") == Some(true) {
        return "Subprocess".into();
    }
    let role = sh
        .sheet
        .get_cell("User.DomainRole")
        .or_else(|| sh.sheet.get_cell("Prop.BpmnType"))
        .and_then(|c| c.value.as_str_coerced().ok())
        .unwrap_or_else(|| sh.name_u.clone())
        .to_ascii_lowercase();

    if role.contains("start") {
        "Start".into()
    } else if role.contains("end") {
        "End".into()
    } else if role.contains("gateway") || role.contains("decision") {
        "Decision".into()
    } else if role.contains("sub") {
        "Subprocess".into()
    } else {
        "Process".into()
    }
}

fn is_process_step_shape(sh: &VisioShape) -> bool {
    if sh.kind != ShapeKind::Shape2D {
        return false;
    }
    if sh.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5 {
        return false;
    }
    if let Some(st) = sh
        .sheet
        .get_cell("User.msvStructureType")
        .and_then(|c| c.value.as_str_coerced().ok())
    {
        let clean = st.trim_matches('"');
        if matches!(clean, "Container" | "Callout" | "Legend") {
            return false;
        }
    }
    true
}

fn read_prop_str(sh: &VisioShape, prop: &str) -> Option<String> {
    sh.sheet
        .get_cell(&format!("Prop.{prop}"))
        .and_then(|c| c.value.as_str_coerced().ok())
        .map(|s| s.trim_matches('"').trim().to_string())
}

fn find_stencil_template(
    packs: &[StencilPack],
    pack_title: &str,
    master_name: &str,
) -> Option<VisioShape> {
    packs
        .iter()
        .find(|p| p.title.eq_ignore_ascii_case(pack_title) || p.category.eq_ignore_ascii_case(pack_title))
        .and_then(|p| {
            p.masters
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(master_name))
        })
        .and_then(|m| m.shapes.first())
        .cloned()
}

// ============================================================================
// 2. Microsoft Power Automate Flow Exporter (`workflowdefinition.json`)
// ============================================================================

/// Export a BPMN / Workflow `VisioPage` into an Azure Logic Apps / Microsoft Power Automate
/// Workflow Definition JSON (`2016-06-01/workflowdefinition.json#`), deriving `triggers` from
/// Start Events, `runAfter` topological edges, and `If` branches from Decision/Exclusive Gateways!
pub fn export_power_automate_flow_json(
    doc: &VisioDocument,
    page_id: u32,
) -> Result<String, serde_json::Error> {
    let Some(page) = doc.page(page_id) else {
        let empty = json!({
            "$schema": "https://schema.management.azure.com/providers/Microsoft.Logic/schemas/2016-06-01/workflowdefinition.json#",
            "contentVersion": "1.0.0.0",
            "parameters": {},
            "triggers": {
                "manual": { "type": "Request", "kind": "Button", "inputs": { "schema": {} } }
            },
            "actions": {},
            "outputs": {}
        });
        return serde_json::to_string_pretty(&empty);
    };

    let steps: BTreeMap<u64, &VisioShape> = page
        .shapes
        .iter()
        .filter(|s| is_process_step_shape(s))
        .map(|s| (s.id, s))
        .collect();

    let mut beg: BTreeMap<u64, u64> = BTreeMap::new();
    let mut end: BTreeMap<u64, u64> = BTreeMap::new();
    for c in &page.connects {
        if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            beg.insert(c.connector_shape_id, c.target_shape_id);
        } else {
            end.insert(c.connector_shape_id, c.target_shape_id);
        }
    }

    let mut outgoing: BTreeMap<u64, Vec<(u64, String)>> = BTreeMap::new();
    let mut incoming: BTreeMap<u64, Vec<u64>> = BTreeMap::new();
    for &sid in steps.keys() {
        outgoing.insert(sid, Vec::new());
        incoming.insert(sid, Vec::new());
    }
    for s in &page.shapes {
        if s.kind == ShapeKind::Connector1D {
            if let (Some(&u), Some(&v)) = (beg.get(&s.id), end.get(&s.id)) {
                if steps.contains_key(&u) && steps.contains_key(&v) && u != v {
                    outgoing.entry(u).or_default().push((v, s.text.trim().to_string()));
                    incoming.entry(v).or_default().push(u);
                }
            }
        }
    }

    // Identify Start Events (`triggers`) vs Workflow `actions`
    let mut start_ids: BTreeSet<u64> = steps
        .iter()
        .filter(|(_, s)| infer_data_vis_shape_type(s) == "Start")
        .map(|(&id, _)| id)
        .collect();

    if start_ids.is_empty() {
        if let Some((&root_id, _)) = steps
            .iter()
            .find(|(&id, _)| incoming.get(&id).map(Vec::is_empty).unwrap_or(true))
        {
            start_ids.insert(root_id);
        }
    }

    let mut action_names: BTreeMap<u64, String> = BTreeMap::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();
    for (&sid, &sh) in &steps {
        let base = sanitize_flow_identifier(if sh.text.trim().is_empty() {
            &sh.name
        } else {
            sh.text.trim()
        });
        let unique = if used_names.insert(base.clone()) {
            base
        } else {
            let suffixed = format!("{base}_{sid}");
            used_names.insert(suffixed.clone());
            suffixed
        };
        action_names.insert(sid, unique);
    }

    let mut triggers = JsonMap::new();
    if start_ids.is_empty() {
        triggers.insert(
            "manual".into(),
            json!({
                "type": "Request",
                "kind": "Button",
                "inputs": { "schema": {} }
            }),
        );
    } else {
        for &tid in &start_ids {
            let tname = action_names[&tid].clone();
            triggers.insert(
                tname,
                json!({
                    "type": "Request",
                    "kind": "Button",
                    "inputs": { "schema": {} },
                    "metadata": { "visioShapeId": tid }
                }),
            );
        }
    }

    // Detect single-parent branch children directly under Decision / Exclusive Gateways
    let mut nested_under_gateway: BTreeMap<u64, u64> = BTreeMap::new();
    let mut gateway_true_children: BTreeMap<u64, Vec<(u64, String)>> = BTreeMap::new();
    let mut gateway_false_children: BTreeMap<u64, Vec<(u64, String)>> = BTreeMap::new();

    for (&gid, &gsh) in &steps {
        if start_ids.contains(&gid) || infer_data_vis_shape_type(gsh) != "Decision" {
            continue;
        }
        let edges = outgoing.get(&gid).cloned().unwrap_or_default();
        for (idx, (child_id, cond_label)) in edges.into_iter().enumerate() {
            if start_ids.contains(&child_id) {
                continue;
            }
            let preds = incoming.get(&child_id).cloned().unwrap_or_default();
            let child_type = steps
                .get(&child_id)
                .map(|s| infer_data_vis_shape_type(s))
                .unwrap_or_default();
            // Nest pure exclusive branch steps (single predecessor == gateway, non-End merge)
            let is_positive = is_affirmative_branch(&cond_label, idx);
            if preds.len() == 1 && preds[0] == gid && child_type != "End" {
                nested_under_gateway.insert(child_id, gid);
                if is_positive {
                    gateway_true_children
                        .entry(gid)
                        .or_default()
                        .push((child_id, cond_label));
                } else {
                    gateway_false_children
                        .entry(gid)
                        .or_default()
                        .push((child_id, cond_label));
                }
            } else if is_positive {
                gateway_true_children
                    .entry(gid)
                    .or_default()
                    .push((child_id, cond_label));
            } else {
                gateway_false_children
                    .entry(gid)
                    .or_default()
                    .push((child_id, cond_label));
            }
        }
    }

    let mut actions = JsonMap::new();
    for (&sid, &sh) in &steps {
        if start_ids.contains(&sid) || nested_under_gateway.contains_key(&sid) {
            continue;
        }
        let act_name = action_names[&sid].clone();

        // Compute `runAfter` over top-level predecessor actions (lifting nested gateway children to their parent `If` gateway!)
        let mut run_after = JsonMap::new();
        if let Some(preds) = incoming.get(&sid) {
            for &p in preds {
                if start_ids.contains(&p) {
                    continue;
                }
                let effective_top = nested_under_gateway.get(&p).copied().unwrap_or(p);
                if effective_top != sid {
                    if let Some(pname) = action_names.get(&effective_top) {
                        run_after.insert(pname.clone(), json!(["Succeeded"]));
                    }
                }
            }
        }

        let stype = infer_data_vis_shape_type(sh);
        let act_json = if stype == "Decision" {
            let true_map = build_gateway_branch_actions(
                &gateway_true_children.get(&sid).cloned().unwrap_or_default(),
                &nested_under_gateway,
                &steps,
                &action_names,
                "TrueBranch",
            );
            let false_map = build_gateway_branch_actions(
                &gateway_false_children.get(&sid).cloned().unwrap_or_default(),
                &nested_under_gateway,
                &steps,
                &action_names,
                "ElseBranch",
            );
            json!({
                "type": "If",
                "runAfter": run_after,
                "expression": {
                    "and": [
                        {
                            "equals": [
                                format!("@triggerBody()?['{}']", act_name.to_ascii_lowercase()),
                                true
                            ]
                        }
                    ]
                },
                "actions": true_map,
                "else": {
                    "actions": false_map
                },
                "metadata": { "visioShapeId": sid }
            })
        } else if stype == "End" {
            json!({
                "type": "Response",
                "kind": "Http",
                "runAfter": run_after,
                "inputs": {
                    "statusCode": 200,
                    "body": { "completedAtStep": sh.text.trim() }
                },
                "metadata": { "visioShapeId": sid }
            })
        } else {
            json!({
                "type": "Compose",
                "runAfter": run_after,
                "inputs": {
                    "step": sh.text.trim(),
                    "function": read_prop_str(sh, "Function").unwrap_or_default()
                },
                "metadata": { "visioShapeId": sid }
            })
        };

        actions.insert(act_name, act_json);
    }

    let definition = json!({
        "$schema": "https://schema.management.azure.com/providers/Microsoft.Logic/schemas/2016-06-01/workflowdefinition.json#",
        "contentVersion": "1.0.0.0",
        "parameters": {},
        "triggers": triggers,
        "actions": actions,
        "outputs": {}
    });

    serde_json::to_string_pretty(&definition)
}

fn build_gateway_branch_actions(
    branch_targets: &[(u64, String)],
    nested_under_gateway: &BTreeMap<u64, u64>,
    steps: &BTreeMap<u64, &VisioShape>,
    action_names: &BTreeMap<u64, String>,
    fallback_prefix: &str,
) -> JsonMap<String, JsonValue> {
    let mut map = JsonMap::new();
    for (idx, (target_id, cond_lbl)) in branch_targets.iter().enumerate() {
        if nested_under_gateway.contains_key(target_id) {
            if let (Some(name), Some(sh)) = (action_names.get(target_id), steps.get(target_id)) {
                map.insert(
                    name.clone(),
                    json!({
                        "type": "Compose",
                        "runAfter": {},
                        "inputs": {
                            "step": sh.text.trim(),
                            "branchCondition": cond_lbl
                        },
                        "metadata": { "visioShapeId": *target_id }
                    }),
                );
            }
        } else {
            let fallback_route = format!("route_{idx}");
            let route_key = format!(
                "{}_{}",
                fallback_prefix,
                sanitize_flow_identifier(if cond_lbl.is_empty() {
                    &fallback_route
                } else {
                    cond_lbl
                })
            );
            map.insert(
                route_key,
                json!({
                    "type": "Compose",
                    "runAfter": {},
                    "inputs": {
                        "nextAction": action_names.get(target_id).cloned().unwrap_or_default(),
                        "branchCondition": cond_lbl
                    }
                }),
            );
        }
    }
    map
}

fn is_affirmative_branch(label: &str, index: usize) -> bool {
    let l = label.trim().to_ascii_lowercase();
    if l.contains("no") || l.contains("false") || l.contains("declin") || l.contains("reject") {
        return false;
    }
    if l.contains("yes") || l.contains("true") || l.contains("approv") || l.contains("ok") {
        return true;
    }
    index == 0
}

fn sanitize_flow_identifier(raw: &str) -> String {
    let mut s: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let trimmed = s.trim_matches('_');
    if trimmed.is_empty() {
        "Step".into()
    } else {
        trimmed.to_string()
    }
}

// ============================================================================
// 3. Power BI Visio Visual Mapping Exporter
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerBiMeasureBinding {
    pub prop_name: String,
    pub label: String,
    pub data_type: String,
    pub raw_value: String,
    pub numeric_value: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerBiShapeVisualEntry {
    pub shape_id: u64,
    pub join_key: String,
    pub name: String,
    pub text: String,
    pub bounding_box_in: [f64; 4],
    pub bounding_box_px: [f64; 4],
    pub fill_hex: Option<String>,
    pub stroke_hex: Option<String>,
    pub svg_paths: Vec<String>,
    pub measures: BTreeMap<String, PowerBiMeasureBinding>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PowerBiVisualMapping {
    pub schema_version: String,
    pub document_title: String,
    pub page_id: u32,
    pub page_name: String,
    pub width_px: f64,
    pub height_px: f64,
    pub shapes: Vec<PowerBiShapeVisualEntry>,
}

/// Export shape IDs, bounding boxes, vector SVG path commands, and bound `Prop.*` measures
/// as structured JSON ready for Power BI Visio Visual custom data binding.
pub fn export_power_bi_visual_json(
    doc: &VisioDocument,
    page_id: u32,
) -> Result<String, serde_json::Error> {
    let ppi = 96.0;
    let Some(page) = doc.page(page_id) else {
        let empty = PowerBiVisualMapping {
            schema_version: "1.0".into(),
            document_title: doc.properties.title.clone(),
            page_id,
            page_name: String::new(),
            width_px: 0.0,
            height_px: 0.0,
            shapes: Vec::new(),
        };
        return serde_json::to_string_pretty(&empty);
    };

    let w_in = page.width_in();
    let h_in = page.height_in();
    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    let mut shape_entries = Vec::new();

    for vis in visuals {
        let Some(sh) = page.find_shape(vis.shape_id) else {
            continue;
        };
        let to_svg_x = |x: f64| x * ppi;
        let to_svg_y = |y: f64| (h_in - y) * ppi;

        let mut svg_paths = Vec::new();
        for p in &vis.paths {
            if p.no_show || p.verbs_page_space.is_empty() {
                continue;
            }
            let mut d = String::new();
            for verb in &p.verbs_page_space {
                match verb {
                    EvaluatedPathVerb::MoveTo(pt) => {
                        let _ = write!(d, "M {:.2} {:.2} ", to_svg_x(pt[0]), to_svg_y(pt[1]));
                    }
                    EvaluatedPathVerb::LineTo(pt) | EvaluatedPathVerb::ArcTo { to: pt, .. } => {
                        let _ = write!(d, "L {:.2} {:.2} ", to_svg_x(pt[0]), to_svg_y(pt[1]));
                    }
                    EvaluatedPathVerb::QuadTo { ctrl, to } => {
                        let _ = write!(
                            d,
                            "Q {:.2} {:.2} {:.2} {:.2} ",
                            to_svg_x(ctrl[0]),
                            to_svg_y(ctrl[1]),
                            to_svg_x(to[0]),
                            to_svg_y(to[1])
                        );
                    }
                    EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                        let _ = write!(
                            d,
                            "C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                            to_svg_x(ctrl1[0]),
                            to_svg_y(ctrl1[1]),
                            to_svg_x(ctrl2[0]),
                            to_svg_y(ctrl2[1]),
                            to_svg_x(to[0]),
                            to_svg_y(to[1])
                        );
                    }
                    EvaluatedPathVerb::Ellipse { center, rx, ry, .. } => {
                        let cx = to_svg_x(center[0]);
                        let cy = to_svg_y(center[1]);
                        let rx_p = rx * ppi;
                        let ry_p = ry * ppi;
                        let _ = write!(
                            d,
                            "M {:.2} {:.2} A {:.2} {:.2} 0 1 0 {:.2} {:.2} A {:.2} {:.2} 0 1 0 {:.2} {:.2} Z ",
                            cx - rx_p,
                            cy,
                            rx_p,
                            ry_p,
                            cx + rx_p,
                            cy,
                            rx_p,
                            ry_p,
                            cx - rx_p,
                            cy
                        );
                    }
                    EvaluatedPathVerb::Close => d.push_str("Z "),
                }
            }
            if !d.trim().is_empty() {
                svg_paths.push(d.trim().to_string());
            }
        }

        let mut measures = BTreeMap::new();
        for row in sh.sheet.shape_data.values() {
            let cell_key = format!("Prop.{}", row.name);
            let raw_val = sh
                .sheet
                .get_cell(&cell_key)
                .and_then(|c| c.value.as_str_coerced().ok())
                .unwrap_or_else(|| row.value.clone())
                .trim_matches('"')
                .to_string();
            let num_val = sh
                .sheet
                .get_f64(&cell_key)
                .or_else(|| raw_val.parse::<f64>().ok());
            measures.insert(
                row.name.clone(),
                PowerBiMeasureBinding {
                    prop_name: row.name.clone(),
                    label: row.label.clone(),
                    data_type: format!("{:?}", row.data_type),
                    raw_value: raw_val,
                    numeric_value: num_val,
                },
            );
        }

        let join_key = read_prop_str(sh, "Process_Step_ID")
            .or_else(|| read_prop_str(sh, "ResourceId"))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if sh.text.trim().is_empty() {
                    sh.name.clone()
                } else {
                    sh.text.lines().next().unwrap_or(&sh.name).trim().to_string()
                }
            });

        let [min_x, min_y] = vis.aabb_min;
        let [max_x, max_y] = vis.aabb_max;
        let bbox_in = [min_x, min_y, (max_x - min_x).max(0.0), (max_y - min_y).max(0.0)];
        let bbox_px = [
            to_svg_x(min_x),
            to_svg_y(max_y),
            (max_x - min_x).max(0.0) * ppi,
            (max_y - min_y).max(0.0) * ppi,
        ];

        shape_entries.push(PowerBiShapeVisualEntry {
            shape_id: vis.shape_id,
            join_key,
            name: sh.name.clone(),
            text: sh.text.clone(),
            bounding_box_in: bbox_in,
            bounding_box_px: bbox_px,
            fill_hex: vis.fill_color.map(|c| c.to_hex_rgb()),
            stroke_hex: vis.stroke_color.map(|c| c.to_hex_rgb()),
            svg_paths,
            measures,
        });
    }

    let mapping = PowerBiVisualMapping {
        schema_version: "1.0".into(),
        document_title: doc.properties.title.clone(),
        page_id,
        page_name: page.name.clone(),
        width_px: w_in * ppi,
        height_px: h_in * ppi,
        shapes: shape_entries,
    };

    serde_json::to_string_pretty(&mapping)
}

// ============================================================================
// 4. Cloud Infrastructure Discovery Importer (AWS / Azure / GCP / Terraform)
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
struct CloudResourceSpec {
    id: String,
    #[serde(rename = "type", alias = "resource_type", alias = "kind", default)]
    resource_type: String,
    #[serde(default)]
    name: String,
    #[serde(default, alias = "vpc_or_region", alias = "subnet", alias = "resource_group")]
    vpc: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default, alias = "depends_on", alias = "targets")]
    connects_to: Vec<String>,
    #[serde(default)]
    tags: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum CloudDiscoveryEnvelope {
    Object {
        #[serde(default)]
        provider: Option<String>,
        resources: Vec<CloudResourceSpec>,
    },
    List(Vec<CloudResourceSpec>),
}

/// Import an AWS / Azure / GCP / Terraform JSON resource inventory onto `page_id`,
/// dropping Cloud Architecture stencil shapes, grouping resources inside VPC/Region
/// `Container` shapes, wiring `connects_to` edges, and running hierarchical layout.
pub fn import_cloud_discovery_json(
    doc: &mut VisioDocument,
    page_id: u32,
    discovery_json: &str,
) -> Result<usize, serde_json::Error> {
    let envelope: CloudDiscoveryEnvelope = serde_json::from_str(discovery_json)?;
    let (provider_opt, resources) = match envelope {
        CloudDiscoveryEnvelope::Object {
            provider,
            resources,
        } => (provider, resources),
        CloudDiscoveryEnvelope::List(resources) => (None, resources),
    };

    if resources.is_empty() {
        return Ok(0);
    }

    let stencils = build_all_builtin_stencils();
    let mut shape_id_by_res: BTreeMap<String, u64> = BTreeMap::new();
    let mut vpc_groups: BTreeMap<String, Vec<u64>> = BTreeMap::new();

    if let Some(page) = doc.page_mut(page_id) {
        let needed_w = ((resources.len() as f64) * 2.2 + 3.0).max(12.0);
        let needed_h = 9.0_f64.max(page.height_in());
        page.page_sheet.set_formula(
            "PageWidth",
            format!("{} in", format_clean_f64(needed_w)),
            Some(Unit::Inch),
        );
        page.page_sheet.set_formula(
            "PageHeight",
            format!("{} in", format_clean_f64(needed_h)),
            Some(Unit::Inch),
        );

        for (idx, res) in resources.iter().enumerate() {
            let sid = page.next_shape_id();
            let (pack_name, master_name) = map_cloud_resource_to_stencil(&res.resource_type);
            let mut sh = find_stencil_template(&stencils, pack_name, master_name)
                .unwrap_or_else(|| VisioShape::new_rect(sid, master_name, 3.0, 4.0, 1.6, 1.0));
            sh.id = sid;
            let display = if res.name.trim().is_empty() {
                res.id.clone()
            } else {
                res.name.clone()
            };
            sh.name = format!("{}.{}", sanitize_flow_identifier(&display), sid);
            sh.name_u = sh.name.clone();
            sh.text = format!("{display}\n({})", res.resource_type);

            sh.sheet.set_formula(
                "PinX",
                format!("{} in", format_clean_f64(2.0 + (idx as f64) * 2.0)),
                Some(Unit::Inch),
            );
            sh.sheet
                .set_formula("PinY", "4.5 in", Some(Unit::Inch));

            let vpc_label = match (&res.vpc, &res.region) {
                (Some(v), Some(r)) if !v.is_empty() && !r.is_empty() && v != r => {
                    format!("{v} ({r})")
                }
                (Some(v), _) if !v.is_empty() => v.clone(),
                (_, Some(r)) if !r.is_empty() => r.clone(),
                _ => String::new(),
            };

            let provider_str = provider_opt.as_deref().unwrap_or("cloud");
            for (k, lbl, val) in [
                ("ResourceId", "Resource ID", res.id.as_str()),
                ("ResourceType", "Resource Type", res.resource_type.as_str()),
                ("Provider", "Cloud Provider", provider_str),
                ("VpcRegion", "VPC / Region", vpc_label.as_str()),
            ] {
                sh.sheet.set_shape_data(ShapeDataRow {
                    name: k.into(),
                    label: lbl.into(),
                    prompt: lbl.into(),
                    data_type: ShapeDataType::String,
                    format: String::new(),
                    value: format!("\"{}\"", val.replace('"', "\"\"")),
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }

            for (tk, tv) in &res.tags {
                let prop_key = format!("Tag_{}", sanitize_flow_identifier(tk));
                sh.sheet.set_shape_data(ShapeDataRow {
                    name: prop_key,
                    label: tk.clone(),
                    prompt: "Cloud Resource Tag".into(),
                    data_type: ShapeDataType::String,
                    format: String::new(),
                    value: format!("\"{}\"", tv.replace('"', "\"\"")),
                    sort_key: "2".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }

            page.shapes.push(sh);
            shape_id_by_res.insert(res.id.clone(), sid);
            if !vpc_label.is_empty() {
                vpc_groups.entry(vpc_label).or_default().push(sid);
            }
        }

        for res in &resources {
            let Some(&src_sid) = shape_id_by_res.get(&res.id) else {
                continue;
            };
            for target_key in &res.connects_to {
                if let Some(&dst_sid) = shape_id_by_res.get(target_key) {
                    let cid = page.next_shape_id();
                    let conn = VisioShape::new_connector(
                        cid,
                        format!("CloudLink.{cid}"),
                        0.0,
                        0.0,
                        1.0,
                        0.0,
                    );
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

    // Auto-layout topological X flow first
    apply_auto_layout(
        doc,
        page_id,
        LayoutOptions {
            algorithm: LayoutAlgorithm::HierarchicalLeftToRight,
            layer_spacing_in: 2.6,
            node_spacing_in: 1.7,
            margin_x_in: 2.0,
            margin_y_in: 1.5,
        },
    );

    // Separate distinct VPC/Region horizontal bands cleanly and wrap each in a Visio Container!
    if !vpc_groups.is_empty() {
        let group_count = vpc_groups.len();
        if group_count > 1 {
            if let Some(page) = doc.page_mut(page_id) {
                let band_h = 2.6;
                let total_h = (group_count as f64 * band_h + 2.0).max(9.0);
                page.page_sheet.set_formula(
                    "PageHeight",
                    format!("{} in", format_clean_f64(total_h)),
                    Some(Unit::Inch),
                );
                for (g_idx, members) in vpc_groups.values().enumerate() {
                    let band_cy = total_h - 1.5 - (g_idx as f64) * band_h;
                    for (m_pos, &mid) in members.iter().enumerate() {
                        if let Some(sh) = page.find_shape_mut(mid) {
                            let y_off = if m_pos % 2 == 0 { 0.25 } else { -0.25 };
                            sh.sheet.set_formula(
                                "PinY",
                                format!("{} in", format_clean_f64(band_cy + y_off)),
                                Some(Unit::Inch),
                            );
                        }
                    }
                }
            }
        }

        for (vpc_name, member_ids) in &vpc_groups {
            let _ = create_container(doc, page_id, &format!("VPC / Region: {vpc_name}"), member_ids, 0.38);
        }
        route_page_connectors(doc, page_id);
    }

    Ok(shape_id_by_res.len())
}

fn map_cloud_resource_to_stencil(resource_type: &str) -> (&'static str, &'static str) {
    let t = resource_type.to_ascii_lowercase();
    if t.contains("eks")
        || t.contains("aks")
        || t.contains("gke")
        || t.contains("k8s")
        || t.contains("kubernetes")
    {
        ("Cloud Architecture (AWS / GCP / Azure)", "Kubernetes Cluster")
    } else if t.contains("lambda") || t.contains("function") || t.contains("cloudrun") {
        ("Cloud Architecture (AWS / GCP / Azure)", "Serverless Function")
    } else if t.contains("s3") || t.contains("bucket") || t.contains("blob") || t.contains("storage")
    {
        ("Cloud Architecture (AWS / GCP / Azure)", "Object Storage Bucket")
    } else if t.contains("rds")
        || t.contains("sql")
        || t.contains("spanner")
        || t.contains("postgres")
        || t.contains("cosmos")
        || t.contains("dynamo")
        || t.contains("db")
    {
        ("Cloud Architecture (AWS / GCP / Azure)", "Managed SQL / Spanner")
    } else if t.contains("alb")
        || t.contains("elb")
        || t.contains("cdn")
        || t.contains("cloudfront")
        || t.contains("frontdoor")
        || t.contains("gateway")
        || t.contains("lb")
        || t.contains("load_balancer")
    {
        ("Cloud Architecture (AWS / GCP / Azure)", "Edge CDN / Load Balancer")
    } else if t.contains("sns")
        || t.contains("sqs")
        || t.contains("pubsub")
        || t.contains("event")
        || t.contains("kafka")
        || t.contains("bus")
    {
        ("Cloud Architecture (AWS / GCP / Azure)", "Pub/Sub Event Bus")
    } else if t.contains("waf") || t.contains("firewall") || t.contains("security_group") {
        ("Network & Rack Equipment", "Enterprise Firewall")
    } else {
        ("Cloud Architecture (AWS / GCP / Azure)", "Compute VM")
    }
}

// ============================================================================
// 5. Entra ID (Microsoft Graph `/users` JSON) & LDAP `.ldif` Org Chart Importer
// ============================================================================

#[derive(Debug, Clone)]
struct DirectoryPerson {
    keys: Vec<String>,
    display_name: String,
    job_title: String,
    department: String,
    manager_ref: Option<String>,
    identity_uri: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum GraphManagerField {
    Text(String),
    Object {
        #[serde(default)]
        id: Option<String>,
        #[serde(rename = "displayName", default)]
        display_name: Option<String>,
        #[serde(rename = "userPrincipalName", default)]
        user_principal_name: Option<String>,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct GraphUserRecord {
    #[serde(default)]
    id: Option<String>,
    #[serde(rename = "userPrincipalName", default)]
    user_principal_name: Option<String>,
    #[serde(rename = "displayName", alias = "cn", alias = "name")]
    display_name: String,
    #[serde(rename = "jobTitle", alias = "title", default)]
    job_title: Option<String>,
    #[serde(default, alias = "ou")]
    department: Option<String>,
    #[serde(default)]
    manager: Option<GraphManagerField>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum GraphUsersEnvelope {
    Collection { value: Vec<GraphUserRecord> },
    Array(Vec<GraphUserRecord>),
}

/// Parse either Microsoft Entra ID Graph `/users` JSON (`displayName`, `jobTitle`, `department`,
/// `manager`) OR RFC-2849 LDAP `.ldif` (`dn:`, `cn:`, `title:`, `departmentNumber:`/`ou:`, `manager:`)
/// and construct a complete hierarchical Visio Organization Chart on `page_id`.
pub fn import_entra_or_ldif_org_chart(
    doc: &mut VisioDocument,
    page_id: u32,
    payload: &str,
) -> Result<usize, DataLinkError> {
    let trimmed = payload.trim_start();
    let people = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_entra_graph_users(trimmed)?
    } else {
        parse_ldif_users(payload)
    };

    if people.is_empty() {
        return Ok(0);
    }

    let stencils = build_all_builtin_stencils();
    let mut lookup_sid: BTreeMap<String, u64> = BTreeMap::new();
    let mut pending_mgr_edges: Vec<(String, u64)> = Vec::new();
    let mut created = 0usize;

    if let Some(page) = doc.page_mut(page_id) {
        let needed_w = ((people.len() as f64) * 2.2 + 2.0).max(11.0);
        page.page_sheet.set_formula(
            "PageWidth",
            format!("{} in", format_clean_f64(needed_w)),
            Some(Unit::Inch),
        );

        for person in &people {
            let sid = page.next_shape_id();
            let master_name = pick_org_master_name(&person.job_title, person.manager_ref.is_none());
            let mut card = find_stencil_template(&stencils, "Organization Chart", master_name)
                .unwrap_or_else(|| VisioShape::new_rect(sid, &person.display_name, 4.0, 5.0, 2.0, 0.9));
            card.id = sid;
            card.name = format!("{}.{}", sanitize_flow_identifier(&person.display_name), sid);
            card.name_u = card.name.clone();
            card.text = format!(
                "{}\n{}\n[{}]",
                person.display_name, person.job_title, person.department
            );
            card.sheet
                .set_formula("Rounding", "0.08 in", Some(Unit::Inch));
            let fill_formula = match master_name {
                "Executive" => "=RGB(191,219,254)",
                "VP / Director" => "=RGB(209,250,229)",
                "Manager" => "=RGB(224,231,255)",
                _ => "=RGB(241,245,249)",
            };
            card.sheet
                .set_formula("FillForegnd", fill_formula, Some(Unit::Color));

            let mgr_str = person.manager_ref.as_deref().unwrap_or("");
            for (k, lbl, val) in [
                ("Title", "Job Title", person.job_title.as_str()),
                ("Department", "Department", person.department.as_str()),
                ("Manager", "Manager", mgr_str),
                ("DirectoryId", "Directory Identity", person.identity_uri.as_str()),
            ] {
                card.sheet.set_shape_data(ShapeDataRow {
                    name: k.into(),
                    label: lbl.into(),
                    prompt: lbl.into(),
                    data_type: ShapeDataType::String,
                    format: String::new(),
                    value: format!("\"{}\"", val.replace('"', "\"\"")),
                    sort_key: "1".into(),
                    invisible: false,
                    ask: false,
                    lang_id: 1033,
                });
            }

            page.shapes.push(card);
            created += 1;

            for key in &person.keys {
                let norm = normalize_identity_key(key);
                if !norm.is_empty() {
                    lookup_sid.insert(norm, sid);
                }
            }
            if let Some(mref) = &person.manager_ref {
                let norm_mgr = normalize_identity_key(mref);
                if !norm_mgr.is_empty() {
                    pending_mgr_edges.push((norm_mgr, sid));
                }
            }
        }

        for (mgr_key, report_sid) in pending_mgr_edges {
            let resolved_mgr = lookup_sid.get(&mgr_key).copied().or_else(|| {
                extract_cn_from_dn(&mgr_key)
                    .and_then(|cn| lookup_sid.get(&normalize_identity_key(&cn)).copied())
            });
            if let Some(mgr_sid) = resolved_mgr {
                if mgr_sid != report_sid {
                    let cid = page.next_shape_id();
                    let mut conn =
                        VisioShape::new_connector(cid, format!("ReportsTo.{cid}"), 0.0, 0.0, 1.0, 0.0);
                    conn.sheet
                        .set_formula("EndArrow", "0", Some(Unit::Number));
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
    }

    apply_auto_layout(
        doc,
        page_id,
        LayoutOptions {
            algorithm: LayoutAlgorithm::CompactTree,
            layer_spacing_in: 1.75,
            node_spacing_in: 2.5,
            margin_x_in: 1.4,
            margin_y_in: 1.2,
        },
    );

    Ok(created)
}

fn parse_entra_graph_users(json_str: &str) -> Result<Vec<DirectoryPerson>, DataLinkError> {
    let env: GraphUsersEnvelope = serde_json::from_str(json_str)?;
    let records = match env {
        GraphUsersEnvelope::Collection { value } => value,
        GraphUsersEnvelope::Array(value) => value,
    };

    let mut out = Vec::new();
    for rec in records {
        if rec.display_name.trim().is_empty() {
            continue;
        }
        let mut keys = vec![rec.display_name.clone()];
        if let Some(id) = &rec.id {
            keys.push(id.clone());
        }
        if let Some(upn) = &rec.user_principal_name {
            keys.push(upn.clone());
        }
        let manager_ref = match rec.manager {
            Some(GraphManagerField::Text(s)) if !s.trim().is_empty() => Some(s),
            Some(GraphManagerField::Object {
                id,
                display_name,
                user_principal_name,
            }) => id
                .filter(|s| !s.trim().is_empty())
                .or_else(|| user_principal_name.filter(|s| !s.trim().is_empty()))
                .or_else(|| display_name.filter(|s| !s.trim().is_empty())),
            _ => None,
        };
        let identity_uri = rec
            .user_principal_name
            .or(rec.id)
            .unwrap_or_else(|| rec.display_name.clone());

        out.push(DirectoryPerson {
            keys,
            display_name: rec.display_name,
            job_title: rec.job_title.unwrap_or_else(|| "Member".into()),
            department: rec.department.unwrap_or_else(|| "Organization".into()),
            manager_ref,
            identity_uri,
        });
    }
    Ok(out)
}

fn parse_ldif_users(ldif: &str) -> Vec<DirectoryPerson> {
    // Step 1: Unfold RFC-2849 continuation lines (lines beginning with a single space ' ')
    let mut unfolded: Vec<String> = Vec::new();
    for raw_line in ldif.lines() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if let Some(cont) = line.strip_prefix(' ') {
            if let Some(last) = unfolded.last_mut() {
                last.push_str(cont);
                continue;
            }
        }
        unfolded.push(line.to_string());
    }

    // Step 2: Group attribute lines into entry blocks separated by blank lines
    let mut entries: Vec<BTreeMap<String, String>> = Vec::new();
    let mut cur: BTreeMap<String, String> = BTreeMap::new();

    for line in unfolded {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !cur.is_empty() {
                entries.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some((attr, b64_val)) = line.split_once("::") {
            let key = attr.trim().to_ascii_lowercase();
            if key == "version" {
                continue;
            }
            if let Ok(decoded_bytes) =
                base64::engine::general_purpose::STANDARD.decode(b64_val.trim())
            {
                if let Ok(utf8) = String::from_utf8(decoded_bytes) {
                    cur.entry(key).or_insert(utf8);
                }
            }
        } else if let Some((attr, plain_val)) = line.split_once(':') {
            let key = attr.trim().to_ascii_lowercase();
            if key == "version" && cur.is_empty() {
                continue;
            }
            cur.entry(key).or_insert_with(|| plain_val.trim().to_string());
        }
    }
    if !cur.is_empty() {
        entries.push(cur);
    }

    // Step 3: Project person / inetOrgPerson / organizationalPerson entries
    let mut people = Vec::new();
    for attrs in entries {
        let dn = attrs.get("dn").cloned().unwrap_or_default();
        let cn = attrs
            .get("displayname")
            .or_else(|| attrs.get("cn"))
            .cloned()
            .or_else(|| extract_cn_from_dn(&dn))
            .unwrap_or_default();
        if cn.is_empty() {
            continue;
        }
        // Skip pure organizationalUnit container entries that lack `cn`/`title` and have `objectclass: organizationalUnit`
        if let Some(oc) = attrs.get("objectclass") {
            if oc.eq_ignore_ascii_case("organizationalUnit") && !attrs.contains_key("cn") {
                continue;
            }
        }

        let title = attrs
            .get("title")
            .cloned()
            .unwrap_or_else(|| "Member".into());
        let department = attrs
            .get("departmentnumber")
            .or_else(|| attrs.get("department"))
            .or_else(|| attrs.get("ou"))
            .cloned()
            .or_else(|| extract_rdn_attr(&dn, "ou"))
            .unwrap_or_else(|| "Directory".into());
        let manager_ref = attrs
            .get("manager")
            .cloned()
            .filter(|m| !m.trim().is_empty());

        let mut keys = vec![cn.clone()];
        if !dn.is_empty() {
            keys.push(dn.clone());
        }
        if let Some(uid) = attrs.get("uid") {
            keys.push(uid.clone());
        }
        if let Some(mail) = attrs.get("mail") {
            keys.push(mail.clone());
        }

        people.push(DirectoryPerson {
            keys,
            display_name: cn.clone(),
            job_title: title,
            department,
            manager_ref,
            identity_uri: if dn.is_empty() { cn } else { dn },
        });
    }
    people
}

fn pick_org_master_name(job_title: &str, is_root: bool) -> &'static str {
    let lower = job_title.to_ascii_lowercase();
    if is_root || lower.contains("chief") || lower.contains("ceo") || lower.contains("president") {
        "Executive"
    } else if lower.contains("vp") || lower.contains("vice president") || lower.contains("director")
    {
        "VP / Director"
    } else if lower.contains("manager") || lower.contains("lead") || lower.contains("head") {
        "Manager"
    } else {
        "Individual Contributor"
    }
}

fn normalize_identity_key(raw: &str) -> String {
    raw.split(',')
        .map(|part| part.trim().to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(",")
}

fn extract_cn_from_dn(dn: &str) -> Option<String> {
    extract_rdn_attr(dn, "cn")
}

fn extract_rdn_attr(dn: &str, attr_prefix: &str) -> Option<String> {
    let target = format!("{}=", attr_prefix.to_ascii_lowercase());
    for part in dn.split(',') {
        let p = part.trim();
        if p.to_ascii_lowercase().starts_with(&target) {
            return Some(p[target.len()..].trim().to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bidirectional_data_visualizer_csv_to_diagram_to_xlsx_to_diagram_roundtrip() {
        let csv = "\
Process Step ID,Process Step Description,Next Step ID,Connector Label,Shape Type,Function,Phase
P100,Receive Purchase Request,P200,,Start,Requester,Intake
P200,Validate Budget,P300,,Process,Finance,Review
P300,Within Threshold?,\"P400, P500\",\"Yes, No\",Decision,Finance,Review
P400,Issue PO Subprocess,P600,,Subprocess,Procurement,Execution
P500,Escalate to CFO,P600,Approved,Process,Executive,Review
P600,Order Completed,,,End,Procurement,Closure
";

        let mut doc1 = VisioDocument::new_blank("Data Visualizer Import");
        let count1 = import_data_visualizer_csv_or_xlsx(&mut doc1, 1, csv.as_bytes()).unwrap();
        assert_eq!(count1, 6);

        let p1 = doc1.page(1).unwrap();
        let swimlane_count = p1
            .shapes
            .iter()
            .filter(|s| s.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5)
            .count();
        assert_eq!(swimlane_count, 4, "Expected 4 Function swimlanes");
        assert!(p1.shapes.iter().any(|s| s.text == "Yes"));
        assert!(p1.shapes.iter().any(|s| s.text == "No"));

        // Export back to Data Visualizer `.xlsx` workbook and re-import into a fresh document!
        let xlsx_bytes = export_data_visualizer_xlsx(&doc1, 1).unwrap();
        assert!(xlsx_bytes.starts_with(b"PK\x03\x04"));

        let mut doc2 = VisioDocument::new_blank("Data Visualizer Roundtrip");
        let count2 = import_data_visualizer_csv_or_xlsx(&mut doc2, 1, &xlsx_bytes).unwrap();
        assert_eq!(count2, 6);

        let p2 = doc2.page(1).unwrap();
        assert!(p2.shapes.iter().any(|s| s.text.contains("Validate Budget")));
        assert!(p2.shapes.iter().any(|s| s.text == "Yes"));
        assert!(p2.shapes.iter().any(|s| s.text == "No"));
    }

    #[test]
    fn power_automate_workflow_json_and_power_bi_visual_mapping_export() {
        let doc = crate::domains::instantiate_starter_template("bpmn_fulfillment");

        // 1. Power Automate Flow JSON
        let flow_json = export_power_automate_flow_json(&doc, 1).unwrap();
        let parsed: JsonValue = serde_json::from_str(&flow_json).unwrap();
        assert_eq!(
            parsed["$schema"],
            "https://schema.management.azure.com/providers/Microsoft.Logic/schemas/2016-06-01/workflowdefinition.json#"
        );
        assert!(parsed["triggers"].as_object().map(|m| !m.is_empty()).unwrap_or(false));

        let actions = parsed["actions"].as_object().unwrap();
        assert!(actions.contains_key("Verify_Payment"));
        assert_eq!(actions["Verify_Payment"]["runAfter"], json!({}));

        // Exclusive Gateway ("Authorized") must export as an `"If"` action with `runAfter: Verify_Payment`
        let gw = &actions["Authorized"];
        assert_eq!(gw["type"], "If");
        assert_eq!(gw["runAfter"]["Verify_Payment"], json!(["Succeeded"]));
        assert!(gw["actions"].as_object().unwrap().contains_key("Dispatch_Shipment"));
        assert!(!gw["else"]["actions"].as_object().unwrap().is_empty());

        // 2. Power BI Visio Visual JSON
        let mut bi_doc = doc.clone();
        bi_doc
            .page_mut(1)
            .unwrap()
            .shapes
            .iter_mut()
            .find(|s| s.text == "Verify Payment")
            .unwrap()
            .sheet
            .set_shape_data(ShapeDataRow {
                name: "LatencyMs".into(),
                label: "Latency (ms)".into(),
                prompt: "".into(),
                data_type: ShapeDataType::Number,
                format: "0".into(),
                value: "42.5".into(),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
        ReactiveEngine::recalculate_document(&mut bi_doc);

        let pbi_json = export_power_bi_visual_json(&bi_doc, 1).unwrap();
        let pbi: PowerBiVisualMapping = serde_json::from_str(&pbi_json).unwrap();
        assert!(!pbi.shapes.is_empty());
        let vp_entry = pbi
            .shapes
            .iter()
            .find(|s| s.text == "Verify Payment")
            .unwrap();
        assert!(!vp_entry.svg_paths.is_empty());
        assert_eq!(vp_entry.measures["LatencyMs"].numeric_value, Some(42.5));
    }

    #[test]
    fn cloud_discovery_vpc_topology_and_entra_json_plus_ldif_org_charts() {
        // 1. Cloud Infrastructure Discovery JSON
        let discovery_json = r#"{
            "provider": "aws",
            "resources": [
                { "id": "alb-1", "type": "aws_alb", "name": "Public Ingress ALB", "vpc": "vpc-edge-us-east-1", "connects_to": ["eks-1", "fn-1"] },
                { "id": "eks-1", "type": "aws_eks_cluster", "name": "Core API EKS", "vpc": "vpc-prod-us-east-1", "connects_to": ["rds-1"], "tags": { "env": "prod" } },
                { "id": "fn-1", "type": "aws_lambda_function", "name": "Billing Webhook", "vpc": "vpc-prod-us-east-1", "connects_to": ["rds-1"] },
                { "id": "rds-1", "type": "aws_rds_cluster", "name": "Aurora Postgres", "vpc": "vpc-prod-us-east-1", "connects_to": [] }
            ]
        }"#;

        let mut cloud_doc = VisioDocument::new_blank("Cloud Discovery");
        let res_count = import_cloud_discovery_json(&mut cloud_doc, 1, discovery_json).unwrap();
        assert_eq!(res_count, 4);

        let cloud_page = cloud_doc.page(1).unwrap();
        let container_count = cloud_page
            .shapes
            .iter()
            .filter(|s| {
                s.sheet
                    .get_cell("User.msvStructureType")
                    .and_then(|c| c.value.as_str_coerced().ok())
                    .map(|v| v.trim_matches('"') == "Container")
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(container_count, 2, "Expected 2 VPC Container shapes");
        assert!(cloud_page.connects.len() >= 8);

        // 2. Microsoft Entra ID `/users` JSON Org Chart
        let entra_json = r#"{
            "value": [
                { "id": "u-100", "userPrincipalName": "ada@vexio.io", "displayName": "Ada Lovelace", "jobTitle": "Chief Executive Officer", "department": "Executive", "manager": null },
                { "id": "u-200", "userPrincipalName": "alan@vexio.io", "displayName": "Alan Turing", "jobTitle": "VP Distributed Systems", "department": "Platform", "manager": { "id": "u-100", "displayName": "Ada Lovelace" } },
                { "id": "u-300", "userPrincipalName": "grace@vexio.io", "displayName": "Grace Hopper", "jobTitle": "Principal Compiler Lead", "department": "ShapeSheet", "manager": "u-200" }
            ]
        }"#;
        let mut entra_doc = VisioDocument::new_blank("Entra Org Chart");
        let entra_count = import_entra_or_ldif_org_chart(&mut entra_doc, 1, entra_json).unwrap();
        assert_eq!(entra_count, 3);
        assert_eq!(entra_doc.page(1).unwrap().connects.len(), 4);

        // 3. RFC-2849 LDAP `.ldif` Org Chart (including folded line + base64 `::` attribute!)
        let ldif_text = "\
version: 1
# Vexio Engineering Directory Export

dn: cn=Katherine Johnson,ou=Executive,dc=vexio,dc=io
objectClass: inetOrgPerson
cn: Katherine Johnson
title: Chief Scientist
departmentNumber: Executive

dn: cn=Margaret Hamilton,ou=FlightSoftware,dc=vexio,dc=io
objectClass: inetOrgPerson
cn: Margaret Hamilton
title:: VlAgRmxpZ2h0IFNvZnR3YXJl
ou: FlightSoftware
manager: cn=Katherine Johnson,ou=Executive,dc=vexio,
 dc=io

dn: cn= Barbara Liskov,ou=Distributed,dc=vexio,dc=io
objectClass: inetOrgPerson
cn: Barbara Liskov
title: Engineering Manager
departmentNumber: Distributed Systems
manager: cn=Margaret Hamilton,ou=FlightSoftware,dc=vexio,dc=io
";
        let mut ldif_doc = VisioDocument::new_blank("LDIF Org Chart");
        let ldif_count = import_entra_or_ldif_org_chart(&mut ldif_doc, 1, ldif_text).unwrap();
        assert_eq!(ldif_count, 3);
        let lp = ldif_doc.page(1).unwrap();
        assert_eq!(lp.connects.len(), 4);
        assert!(
            lp.shapes.iter().any(|s| s.text.contains("VP Flight Software")),
            "Base64 RFC-2849 `title::` attribute must decode cleanly"
        );
    }
}
