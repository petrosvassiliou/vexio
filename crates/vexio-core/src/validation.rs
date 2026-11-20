//! Built-in Diagram Rule Validation Engine: **BPMN 2.0**, **UML 2.5**, and **SharePoint / Workflow**.
//!
//! Replicates Microsoft Visio Professional's `Validation.Validate()` engine (`Issues` collection).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use vexio_shapesheet::{ShapeKind, VisioDocument, VisioPage, VisioShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagramRuleSet {
    Bpmn20,
    Uml25,
    SharePointWorkflow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub id: u32,
    pub ruleset: DiagramRuleSet,
    pub rule_code: String,
    pub severity: ValidationSeverity,
    pub category: String,
    pub description: String,
    pub page_id: u32,
    pub shape_id: Option<u64>,
    pub ignored: bool,
}

pub fn validate_document(doc: &VisioDocument, ruleset: DiagramRuleSet) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut next_id = 1u32;
    for page in &doc.pages {
        if page.is_background {
            continue;
        }
        match ruleset {
            DiagramRuleSet::Bpmn20 => validate_bpmn_page(page, &mut next_id, &mut issues),
            DiagramRuleSet::Uml25 => validate_uml_page(page, &mut next_id, &mut issues),
            DiagramRuleSet::SharePointWorkflow => {
                validate_workflow_page(page, &mut next_id, &mut issues)
            }
        }
    }
    issues
}

struct PageGraph<'a> {
    shapes_by_id: BTreeMap<u64, &'a VisioShape>,
    outgoing: BTreeMap<u64, Vec<(u64, &'a VisioShape)>>, // node -> Vec<(target_node, connector_shape)>
    incoming: BTreeMap<u64, Vec<(u64, &'a VisioShape)>>, // node -> Vec<(source_node, connector_shape)>
    dangling_connectors: Vec<&'a VisioShape>,
}

impl<'a> PageGraph<'a> {
    fn from_page(page: &'a VisioPage) -> Self {
        let mut shapes_by_id = BTreeMap::new();
        let mut outgoing: BTreeMap<u64, Vec<(u64, &'a VisioShape)>> = BTreeMap::new();
        let mut incoming: BTreeMap<u64, Vec<(u64, &'a VisioShape)>> = BTreeMap::new();
        for s in &page.shapes {
            if s.kind == ShapeKind::Shape2D
                && s.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) < 0.5
                && role_str(s) != "container"
                && role_str(s) != "pool"
            {
                shapes_by_id.insert(s.id, s);
                outgoing.insert(s.id, Vec::new());
                incoming.insert(s.id, Vec::new());
            }
        }

        let mut beg_map: BTreeMap<u64, u64> = BTreeMap::new();
        let mut end_map: BTreeMap<u64, u64> = BTreeMap::new();
        for c in &page.connects {
            if c.from_cell.to_ascii_lowercase().starts_with("begin") {
                beg_map.insert(c.connector_shape_id, c.target_shape_id);
            } else {
                end_map.insert(c.connector_shape_id, c.target_shape_id);
            }
        }

        let mut dangling_connectors = Vec::new();
        for s in &page.shapes {
            if s.kind == ShapeKind::Connector1D {
                match (beg_map.get(&s.id), end_map.get(&s.id)) {
                    (Some(&u), Some(&v))
                        if shapes_by_id.contains_key(&u) && shapes_by_id.contains_key(&v) =>
                    {
                        outgoing.entry(u).or_default().push((v, s));
                        incoming.entry(v).or_default().push((u, s));
                    }
                    _ => dangling_connectors.push(s),
                }
            }
        }

        Self {
            shapes_by_id,
            outgoing,
            incoming,
            dangling_connectors,
        }
    }
}

fn role_str(shape: &VisioShape) -> String {
    if let Some(c) = shape
        .sheet
        .get_cell("Prop.BpmnType")
        .or_else(|| shape.sheet.get_cell("Prop.UmlType"))
        .or_else(|| shape.sheet.get_cell("Prop.WorkflowRole"))
        .or_else(|| shape.sheet.get_cell("User.DomainRole"))
    {
        if let Ok(s) = c.value.as_str_coerced() {
            return s.trim_matches('"').to_ascii_lowercase();
        }
    }
    shape.name_u.to_ascii_lowercase()
}

fn pool_of(shape: &VisioShape) -> Option<String> {
    shape
        .sheet
        .get_cell("Prop.Pool")
        .or_else(|| shape.sheet.get_cell("Prop.Function"))
        .and_then(|c| c.value.as_str_coerced().ok())
        .map(|s| s.trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
}

fn validate_bpmn_page(page: &VisioPage, next_id: &mut u32, out: &mut Vec<ValidationIssue>) {
    let g = PageGraph::from_page(page);
    if g.shapes_by_id.is_empty() && g.dangling_connectors.is_empty() {
        return;
    }

    let mut start_events = Vec::new();
    let mut end_events = Vec::new();

    for (&id, &sh) in &g.shapes_by_id {
        let r = role_str(sh);
        let in_cnt = g.incoming.get(&id).map(|v| v.len()).unwrap_or(0);
        let out_cnt = g.outgoing.get(&id).map(|v| v.len()).unwrap_or(0);

        if r.contains("start") {
            start_events.push(id);
            if in_cnt > 0 {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Bpmn20,
                    "BPMN-001",
                    ValidationSeverity::Error,
                    "Start Event",
                    "Start Event must not have any incoming Sequence Flows.",
                    page.id,
                    Some(id),
                );
            }
            if out_cnt == 0 {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Bpmn20,
                    "BPMN-002",
                    ValidationSeverity::Error,
                    "Start Event",
                    "Start Event must have at least one outgoing Sequence Flow.",
                    page.id,
                    Some(id),
                );
            }
        } else if r.contains("end") {
            end_events.push(id);
            if out_cnt > 0 {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Bpmn20,
                    "BPMN-003",
                    ValidationSeverity::Error,
                    "End Event",
                    "End Event must not have any outgoing Sequence Flows.",
                    page.id,
                    Some(id),
                );
            }
            if in_cnt == 0 {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Bpmn20,
                    "BPMN-004",
                    ValidationSeverity::Error,
                    "End Event",
                    "End Event must have at least one incoming Sequence Flow.",
                    page.id,
                    Some(id),
                );
            }
        } else if r.contains("gateway") {
            if in_cnt <= 1 && out_cnt <= 1 {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Bpmn20,
                    "BPMN-005",
                    ValidationSeverity::Warning,
                    "Gateway",
                    "Gateway neither splits (outgoing >= 2) nor merges (incoming >= 2) flows.",
                    page.id,
                    Some(id),
                );
            }
            if r.contains("exclusive") && out_cnt >= 2 {
                let unlabeled = g
                    .outgoing
                    .get(&id)
                    .unwrap()
                    .iter()
                    .any(|(_, conn)| conn.text.trim().is_empty());
                if unlabeled {
                    push_issue(
                        out,
                        next_id,
                        DiagramRuleSet::Bpmn20,
                        "BPMN-006",
                        ValidationSeverity::Warning,
                        "Gateway",
                        "Exclusive Gateway outgoing conditional Sequence Flows should have condition labels.",
                        page.id,
                        Some(id),
                    );
                }
            }
        } else {
            // Task / Activity / Sub-Process
            if in_cnt == 0 || out_cnt == 0 {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Bpmn20,
                    "BPMN-007",
                    ValidationSeverity::Error,
                    "Activity",
                    "BPMN Task/Activity must have both incoming and outgoing Sequence Flows.",
                    page.id,
                    Some(id),
                );
            }
        }

        // Cross-Pool Sequence Flow vs Intra-Pool Message Flow check
        if let Some(nbrs) = g.outgoing.get(&id) {
            for &(dst_id, conn) in nbrs {
                let dst_sh = g.shapes_by_id[&dst_id];
                let is_msg_flow = role_str(conn).contains("message");
                if let (Some(p1), Some(p2)) = (pool_of(sh), pool_of(dst_sh)) {
                    if p1 != p2 && !is_msg_flow {
                        push_issue(
                            out,
                            next_id,
                            DiagramRuleSet::Bpmn20,
                            "BPMN-008",
                            ValidationSeverity::Error,
                            "Connecting Object",
                            "Sequence Flow cannot cross Pool boundaries; use a Message Flow instead.",
                            page.id,
                            Some(conn.id),
                        );
                    } else if p1 == p2 && is_msg_flow {
                        push_issue(
                            out,
                            next_id,
                            DiagramRuleSet::Bpmn20,
                            "BPMN-009",
                            ValidationSeverity::Error,
                            "Connecting Object",
                            "Message Flow is not allowed between elements within the same Pool.",
                            page.id,
                            Some(conn.id),
                        );
                    }
                }
            }
        }
    }

    if start_events.is_empty() {
        push_issue(
            out,
            next_id,
            DiagramRuleSet::Bpmn20,
            "BPMN-010",
            ValidationSeverity::Error,
            "Process",
            "BPMN Process diagram is missing a Start Event.",
            page.id,
            None,
        );
    }
    if end_events.is_empty() {
        push_issue(
            out,
            next_id,
            DiagramRuleSet::Bpmn20,
            "BPMN-011",
            ValidationSeverity::Error,
            "Process",
            "BPMN Process diagram is missing an End Event.",
            page.id,
            None,
        );
    }
    for conn in g.dangling_connectors {
        push_issue(
            out,
            next_id,
            DiagramRuleSet::Bpmn20,
            "BPMN-012",
            ValidationSeverity::Error,
            "Connecting Object",
            "Connector is not glued to valid BPMN source and target shapes at both endpoints.",
            page.id,
            Some(conn.id),
        );
    }
}

fn validate_uml_page(page: &VisioPage, next_id: &mut u32, out: &mut Vec<ValidationIssue>) {
    let g = PageGraph::from_page(page);
    for (&id, &sh) in &g.shapes_by_id {
        let r = role_str(sh);
        if (r.contains("class") || r.contains("interface")) && sh.text.trim().is_empty() {
            push_issue(
                out,
                next_id,
                DiagramRuleSet::Uml25,
                "UML-001",
                ValidationSeverity::Error,
                "Classifier",
                "UML Classifier (Class or Interface) must have a non-empty name.",
                page.id,
                Some(id),
            );
        }
        // Check inheritance cycle via outgoing generalization connectors
        let mut q = VecDeque::new();
        let mut seen = BTreeSet::new();
        if let Some(nbrs) = g.outgoing.get(&id) {
            for &(v, conn) in nbrs {
                if role_str(conn).contains("generalization") || role_str(conn).contains("inherit") {
                    q.push_back(v);
                }
            }
        }
        while let Some(cur) = q.pop_front() {
            if cur == id {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::Uml25,
                    "UML-002",
                    ValidationSeverity::Error,
                    "Generalization",
                    "Circular inheritance hierarchy detected on UML Classifier.",
                    page.id,
                    Some(id),
                );
                break;
            }
            if seen.insert(cur) {
                if let Some(nbrs) = g.outgoing.get(&cur) {
                    for &(v, conn) in nbrs {
                        if role_str(conn).contains("generalization")
                            || role_str(conn).contains("inherit")
                        {
                            q.push_back(v);
                        }
                    }
                }
            }
        }
    }

    for conn in g.dangling_connectors {
        push_issue(
            out,
            next_id,
            DiagramRuleSet::Uml25,
            "UML-003",
            ValidationSeverity::Error,
            "Relationship",
            "UML relationship or sequence message connector has an unglued endpoint.",
            page.id,
            Some(conn.id),
        );
    }
}

fn validate_workflow_page(page: &VisioPage, next_id: &mut u32, out: &mut Vec<ValidationIssue>) {
    let g = PageGraph::from_page(page);
    if g.shapes_by_id.is_empty() {
        return;
    }
    let starts: Vec<u64> = g
        .shapes_by_id
        .iter()
        .filter(|(_, s)| role_str(s).contains("start"))
        .map(|(&id, _)| id)
        .collect();
    if starts.len() != 1 {
        push_issue(
            out,
            next_id,
            DiagramRuleSet::SharePointWorkflow,
            "SPWF-001",
            ValidationSeverity::Error,
            "Workflow Entry",
            "SharePoint Workflow must contain exactly one Start shape.",
            page.id,
            starts.get(1).copied(),
        );
    }

    if let Some(&root) = starts.first() {
        let mut reachable = BTreeSet::new();
        let mut q = VecDeque::from([root]);
        while let Some(u) = q.pop_front() {
            if reachable.insert(u) {
                if let Some(nbrs) = g.outgoing.get(&u) {
                    for &(v, _) in nbrs {
                        q.push_back(v);
                    }
                }
            }
        }
        for (&id, _) in &g.shapes_by_id {
            if !reachable.contains(&id) {
                push_issue(
                    out,
                    next_id,
                    DiagramRuleSet::SharePointWorkflow,
                    "SPWF-002",
                    ValidationSeverity::Error,
                    "Reachability",
                    "Workflow step is unreachable from the workflow Start node.",
                    page.id,
                    Some(id),
                );
            }
        }
    }
    for conn in g.dangling_connectors {
        push_issue(
            out,
            next_id,
            DiagramRuleSet::SharePointWorkflow,
            "SPWF-003",
            ValidationSeverity::Error,
            "Transition",
            "Workflow transition connector is not connected at both ends.",
            page.id,
            Some(conn.id),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_issue(
    out: &mut Vec<ValidationIssue>,
    next_id: &mut u32,
    ruleset: DiagramRuleSet,
    code: &str,
    severity: ValidationSeverity,
    category: &str,
    description: &str,
    page_id: u32,
    shape_id: Option<u64>,
) {
    out.push(ValidationIssue {
        id: *next_id,
        ruleset,
        rule_code: code.into(),
        severity,
        category: category.into(),
        description: description.into(),
        page_id,
        shape_id,
        ignored: false,
    });
    *next_id += 1;
}
