//! Bidirectional Diagram-as-Code Interop: Mermaid, PlantUML & Graphviz DOT ⇄ Visio (`VisioDocument`).
//!
//! Provides deterministic, 100% offline, zero-stub parsers, hierarchical rank layout,
//! container/subgraph materialization, and clean round-trip exporters for:
//! - **Mermaid** (`.mmd`, `.mermaid`): `flowchart` / `graph` (`TD`/`TB`/`LR`/`RL`/`BT`), `subgraph ... end`,
//!   `sequenceDiagram`, `classDiagram`, `erDiagram`, and `stateDiagram` / `stateDiagram-v2`.
//! - **PlantUML** (`.puml`, `.plantuml`): `@startuml ... @enduml` participants, actors, classes,
//!   components, databases, queues, notes, `package` / `namespace` / `folder` containers, and styled edges.
//! - **Graphviz DOT** (`.dot`, `.gv`): `digraph` / `graph`, `rankdir`, `subgraph cluster_*` containers,
//!   node shapes/colors/labels, and multi-hop attributed edge chains.

use crate::containers::{
    create_container, is_callout_shape, is_container_shape, is_swimlane_shape,
    parse_container_members,
};
use crate::routing::route_page_connectors;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use vexio_shapesheet::{
    ConnectionPointRow, ControlRow, GeometryRow, GeometryRowKind, GeometrySection, GlueEndpoint,
    ReactiveEngine, ShapeKind, Unit, VisioDocument, VisioShape, format_clean_f64,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagramMarkupFormat {
    Auto,
    Mermaid,
    PlantUml,
    GraphvizDot,
}

impl DiagramMarkupFormat {
    pub fn from_slug(s: &str) -> Self {
        match s.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "mermaid" | "mmd" => Self::Mermaid,
            "plantuml" | "puml" | "uml" | "pu" => Self::PlantUml,
            "dot" | "gv" | "graphviz" => Self::GraphvizDot,
            _ => Self::Auto,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Auto | Self::Mermaid => "mermaid",
            Self::PlantUml => "plantuml",
            Self::GraphvizDot => "dot",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkupImportStats {
    pub format: String,
    pub nodes_created: usize,
    pub containers_created: usize,
    pub connectors_created: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutDir {
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkupNodeShape {
    Rect,
    RoundRect,
    Stadium,
    Subprocess,
    Cylinder,
    Circle,
    Diamond,
    Hexagon,
    Parallelogram,
    Trapezoid,
    Actor,
    ClassBox,
    EntityBox,
    Note,
}

impl MarkupNodeShape {
    fn tag(self) -> &'static str {
        match self {
            Self::Rect => "rect",
            Self::RoundRect => "round_rect",
            Self::Stadium => "stadium",
            Self::Subprocess => "subprocess",
            Self::Cylinder => "cylinder",
            Self::Circle => "circle",
            Self::Diamond => "diamond",
            Self::Hexagon => "hexagon",
            Self::Parallelogram => "parallelogram",
            Self::Trapezoid => "trapezoid",
            Self::Actor => "actor",
            Self::ClassBox => "class_box",
            Self::EntityBox => "entity_box",
            Self::Note => "note",
        }
    }

    fn from_tag(tag: &str) -> Option<Self> {
        match tag.trim().to_ascii_lowercase().as_str() {
            "rect" | "box" | "rectangle" => Some(Self::Rect),
            "round_rect" | "rounded" | "mrecord" => Some(Self::RoundRect),
            "stadium" | "pill" | "terminator" => Some(Self::Stadium),
            "subprocess" | "component" => Some(Self::Subprocess),
            "cylinder" | "database" | "db" => Some(Self::Cylinder),
            "circle" | "ellipse" | "oval" | "doublecircle" => Some(Self::Circle),
            "diamond" | "decision" => Some(Self::Diamond),
            "hexagon" | "prep" => Some(Self::Hexagon),
            "parallelogram" | "data" => Some(Self::Parallelogram),
            "trapezoid" => Some(Self::Trapezoid),
            "actor" => Some(Self::Actor),
            "class_box" | "record" => Some(Self::ClassBox),
            "entity_box" => Some(Self::EntityBox),
            "note" | "folded_note" => Some(Self::Note),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkupEdgeStyle {
    Solid,
    Dashed,
    Dotted,
    Thick,
}

#[derive(Debug, Clone)]
struct MarkupNode {
    id: String,
    label: String,
    shape: MarkupNodeShape,
    fill_rgb: Option<(u8, u8, u8)>,
    stroke_rgb: Option<(u8, u8, u8)>,
    container_id: Option<String>,
    order_idx: usize,
}

#[derive(Debug, Clone)]
struct MarkupContainer {
    id: String,
    title: String,
    member_node_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct MarkupEdge {
    from_id: String,
    to_id: String,
    label: String,
    style: MarkupEdgeStyle,
    begin_arrow: u32,
    end_arrow: u32,
}

#[derive(Debug, Clone)]
struct MarkupGraph {
    direction: LayoutDir,
    nodes: BTreeMap<String, MarkupNode>,
    containers: Vec<MarkupContainer>,
    edges: Vec<MarkupEdge>,
    next_order: usize,
}

impl MarkupGraph {
    fn new(direction: LayoutDir) -> Self {
        Self {
            direction,
            nodes: BTreeMap::new(),
            containers: Vec::new(),
            edges: Vec::new(),
            next_order: 0,
        }
    }

    fn upsert_node(
        &mut self,
        id: &str,
        label: Option<&str>,
        shape: Option<MarkupNodeShape>,
        container_id: Option<&str>,
    ) {
        let clean_id = sanitize_id_key(id);
        if clean_id.is_empty() {
            return;
        }
        if let Some(existing) = self.nodes.get_mut(&clean_id) {
            if let Some(lbl) = label.map(str::trim).filter(|s| !s.is_empty()) {
                existing.label = unescape_diagram_text(lbl);
            }
            if let Some(sh) = shape {
                existing.shape = sh;
            }
            if existing.container_id.is_none() {
                if let Some(cid) = container_id {
                    existing.container_id = Some(cid.to_string());
                }
            }
        } else {
            let order_idx = self.next_order;
            self.next_order += 1;
            let resolved_label = label
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(unescape_diagram_text)
                .unwrap_or_else(|| id.trim().trim_matches('"').to_string());
            self.nodes.insert(
                clean_id.clone(),
                MarkupNode {
                    id: clean_id.clone(),
                    label: resolved_label,
                    shape: shape.unwrap_or(MarkupNodeShape::Rect),
                    fill_rgb: None,
                    stroke_rgb: None,
                    container_id: container_id.map(str::to_string),
                    order_idx,
                },
            );
        }
        if let Some(cid) = container_id {
            if let Some(cont) = self.containers.iter_mut().find(|c| c.id == cid) {
                if !cont.member_node_ids.contains(&clean_id) {
                    cont.member_node_ids.push(clean_id);
                }
            }
        }
    }
}

/// Auto-detect whether `markup` is Mermaid, PlantUML, or Graphviz DOT.
pub fn detect_diagram_markup_format(markup: &str) -> DiagramMarkupFormat {
    let mut trimmed = markup.trim();
    // Strip optional YAML frontmatter `---\n...\n---`
    if let Some(after_open) = trimmed.strip_prefix("---") {
        if let Some(end_yaml) = after_open.find("\n---") {
            trimmed = after_open[end_yaml + 4..].trim();
        }
    }

    for raw_line in trimmed.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("%%")
            || line.starts_with("//")
            || line.starts_with('#')
            || line.starts_with('\'')
        {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("@startuml")
            || lower.starts_with("@startmindmap")
            || lower.starts_with("skinparam ")
            || lower.starts_with("actor ")
            || lower.starts_with("database ")
            || lower.starts_with("package ")
            || lower.starts_with("left to right direction")
            || lower.starts_with("top to bottom direction")
        {
            return DiagramMarkupFormat::PlantUml;
        }
        if lower.starts_with("strict digraph")
            || lower.starts_with("digraph")
            || lower.starts_with("strict graph")
            || (lower.starts_with("graph") && line.contains('{'))
        {
            return DiagramMarkupFormat::GraphvizDot;
        }
        if lower.starts_with("flowchart")
            || lower.starts_with("graph ")
            || lower.starts_with("sequencediagram")
            || lower.starts_with("classdiagram")
            || lower.starts_with("erdiagram")
            || lower.starts_with("statediagram")
        {
            return DiagramMarkupFormat::Mermaid;
        }
        if line.contains("->") && (line.contains('[') && line.contains('=')) || line.ends_with(';') {
            if trimmed.contains('{') && trimmed.contains('}') && !lower.contains("subgraph ") {
                return DiagramMarkupFormat::GraphvizDot;
            }
        }
        break;
    }
    if trimmed.contains("@enduml") {
        DiagramMarkupFormat::PlantUml
    } else if trimmed.contains("digraph ") || trimmed.contains("rankdir") {
        DiagramMarkupFormat::GraphvizDot
    } else {
        DiagramMarkupFormat::Mermaid
    }
}

/// Parse Mermaid, PlantUML, or Graphviz DOT diagram markup and materialize real Visio shapes,
/// containers, and dynamically routed connectors onto `(doc, page_id)`.
pub fn import_diagram_markup(
    doc: &mut VisioDocument,
    page_id: u32,
    markup: &str,
    format: DiagramMarkupFormat,
) -> Result<MarkupImportStats, String> {
    if markup.trim().is_empty() {
        return Err("Diagram markup is empty".into());
    }
    if doc.page(page_id).is_none() {
        return Err(format!("Page {page_id} does not exist"));
    }
    let resolved = match format {
        DiagramMarkupFormat::Auto => detect_diagram_markup_format(markup),
        other => other,
    };
    let graph = match resolved {
        DiagramMarkupFormat::Mermaid | DiagramMarkupFormat::Auto => parse_mermaid(markup)?,
        DiagramMarkupFormat::PlantUml => parse_plantuml(markup)?,
        DiagramMarkupFormat::GraphvizDot => parse_graphviz_dot(markup)?,
    };
    if graph.nodes.is_empty() {
        return Err(format!(
            "No diagram nodes found in {} markup",
            resolved.slug()
        ));
    }
    materialize_markup_graph(doc, page_id, &graph, resolved)
}

/// Export `(doc, page_id)` into clean, round-trippable Mermaid, PlantUML, or Graphviz DOT markup!
pub fn export_diagram_markup(
    doc: &VisioDocument,
    page_id: u32,
    format: DiagramMarkupFormat,
) -> Result<String, String> {
    let page = doc
        .page(page_id)
        .ok_or_else(|| format!("Page {page_id} not found"))?;
    let resolved = match format {
        DiagramMarkupFormat::Auto => DiagramMarkupFormat::Mermaid,
        other => other,
    };

    // Collect containers and their member shape IDs
    let mut container_shapes: Vec<(u64, String, Vec<u64>)> = Vec::new();
    let mut member_to_container: BTreeMap<u64, u64> = BTreeMap::new();
    for sh in &page.shapes {
        if is_container_shape(sh) && !is_swimlane_shape(sh) {
            let members = parse_container_members(sh);
            for &mid in &members {
                member_to_container.insert(mid, sh.id);
            }
            let title = if sh.text.trim().is_empty() {
                sh.name.clone()
            } else {
                sh.text.trim().to_string()
            };
            container_shapes.push((sh.id, title, members));
        }
    }

    // Collect foreground 2D/Group node shapes
    let mut node_ids: BTreeMap<u64, (String, String, MarkupNodeShape)> = BTreeMap::new();
    let mut used_keys: BTreeSet<String> = BTreeSet::new();
    for sh in &page.shapes {
        if !matches!(
            sh.kind,
            ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
        ) || is_container_shape(sh)
            || is_swimlane_shape(sh)
            || is_callout_shape(sh)
        {
            continue;
        }
        let preferred_id = sh
            .sheet
            .get_cell("User.MarkupNodeId")
            .and_then(|c| c.value.as_str_coerced().ok())
            .map(|s| sanitize_export_ident(s.trim_matches('"')))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("N{}", sh.id));
        let mut key = preferred_id.clone();
        let mut suffix = 2usize;
        while used_keys.contains(&key) {
            key = format!("{preferred_id}_{suffix}");
            suffix += 1;
        }
        used_keys.insert(key.clone());

        let label = if !sh.text.trim().is_empty() {
            sh.text.trim().to_string()
        } else {
            sh.name.trim().to_string()
        };
        let nshape = infer_shape_markup_kind(sh);
        node_ids.insert(sh.id, (key, label, nshape));
    }

    // Collect glued 1D connectors
    let mut edges: Vec<(u64, u64, String, MarkupEdgeStyle, bool, bool)> = Vec::new();
    for sh in &page.shapes {
        if sh.kind != ShapeKind::Connector1D {
            continue;
        }
        let mut from_sid = None;
        let mut to_sid = None;
        for c in &page.connects {
            if c.connector_shape_id == sh.id {
                if c.from_cell.to_ascii_lowercase().starts_with("begin") {
                    from_sid = Some(c.target_shape_id);
                } else {
                    to_sid = Some(c.target_shape_id);
                }
            }
        }
        if let (Some(f), Some(t)) = (from_sid, to_sid) {
            if node_ids.contains_key(&f) && node_ids.contains_key(&t) {
                let pat = sh.sheet.get_f64("LinePattern").unwrap_or(1.0).round() as i32;
                let lw_pt = sh.sheet.get_f64("LineWeight").unwrap_or(0.014) * 72.0;
                let style = if pat == 2 {
                    MarkupEdgeStyle::Dashed
                } else if pat == 3 {
                    MarkupEdgeStyle::Dotted
                } else if lw_pt >= 2.0 {
                    MarkupEdgeStyle::Thick
                } else {
                    MarkupEdgeStyle::Solid
                };
                let beg_arr = sh.sheet.get_f64("BeginArrow").unwrap_or(0.0) >= 1.0;
                let end_arr = sh.sheet.get_f64("EndArrow").unwrap_or(4.0) >= 1.0;
                edges.push((f, t, sh.text.trim().to_string(), style, beg_arr, end_arr));
            }
        }
    }

    match resolved {
        DiagramMarkupFormat::Mermaid | DiagramMarkupFormat::Auto => {
            let mut out = String::from("flowchart TD\n");
            for (cid, ctitle, members) in &container_shapes {
                let sg_id = format!("SG_{cid}");
                out.push_str(&format!(
                    "    subgraph {sg_id} [\"{}\"]\n",
                    escape_quotes(ctitle)
                ));
                for mid in members {
                    if let Some((key, lbl, nshape)) = node_ids.get(mid) {
                        out.push_str(&format!(
                            "        {}\n",
                            format_mermaid_node(key, lbl, *nshape)
                        ));
                    }
                }
                out.push_str("    end\n");
            }
            for (sid, (key, lbl, nshape)) in &node_ids {
                if !member_to_container.contains_key(sid) {
                    out.push_str(&format!("    {}\n", format_mermaid_node(key, lbl, *nshape)));
                }
            }
            for (f, t, lbl, style, beg_arr, end_arr) in &edges {
                let fk = &node_ids[f].0;
                let tk = &node_ids[t].0;
                let arrow = match (style, *beg_arr, *end_arr) {
                    (MarkupEdgeStyle::Dashed | MarkupEdgeStyle::Dotted, _, true) => "-.->",
                    (MarkupEdgeStyle::Dashed | MarkupEdgeStyle::Dotted, _, false) => "-.-",
                    (MarkupEdgeStyle::Thick, _, true) => "==>",
                    (MarkupEdgeStyle::Thick, _, false) => "===",
                    (_, true, true) => "<-->",
                    (_, false, false) => "---",
                    _ => "-->",
                };
                if lbl.is_empty() {
                    out.push_str(&format!("    {fk} {arrow} {tk}\n"));
                } else {
                    let clean_lbl = lbl.replace('\n', " ").replace('|', "/");
                    out.push_str(&format!("    {fk} {arrow}|{clean_lbl}| {tk}\n"));
                }
            }
            Ok(out)
        }
        DiagramMarkupFormat::PlantUml => {
            let mut out = String::from("@startuml\ntop to bottom direction\n");
            for (cid, ctitle, members) in &container_shapes {
                out.push_str(&format!(
                    "package \"{}\" as PKG_{cid} {{\n",
                    escape_quotes(ctitle)
                ));
                for mid in members {
                    if let Some((key, lbl, nshape)) = node_ids.get(mid) {
                        out.push_str(&format!(
                            "    {}\n",
                            format_plantuml_node(key, lbl, *nshape)
                        ));
                    }
                }
                out.push_str("}\n");
            }
            for (sid, (key, lbl, nshape)) in &node_ids {
                if !member_to_container.contains_key(sid) {
                    out.push_str(&format!("{}\n", format_plantuml_node(key, lbl, *nshape)));
                }
            }
            for (f, t, lbl, style, beg_arr, end_arr) in &edges {
                let fk = &node_ids[f].0;
                let tk = &node_ids[t].0;
                let arrow = match (style, *beg_arr, *end_arr) {
                    (MarkupEdgeStyle::Dashed | MarkupEdgeStyle::Dotted, _, true) => "..>",
                    (MarkupEdgeStyle::Dashed | MarkupEdgeStyle::Dotted, _, false) => "..",
                    (_, true, true) => "<-->",
                    (_, false, false) => "--",
                    _ => "-->",
                };
                if lbl.is_empty() {
                    out.push_str(&format!("{fk} {arrow} {tk}\n"));
                } else {
                    out.push_str(&format!(
                        "{fk} {arrow} {tk} : {}\n",
                        lbl.replace('\n', "\\n")
                    ));
                }
            }
            out.push_str("@enduml\n");
            Ok(out)
        }
        DiagramMarkupFormat::GraphvizDot => {
            let mut out = String::from("digraph VexioDiagram {\n    rankdir=TB;\n");
            for (cid, ctitle, members) in &container_shapes {
                out.push_str(&format!(
                    "    subgraph cluster_{cid} {{\n        label=\"{}\";\n",
                    escape_quotes(ctitle)
                ));
                for mid in members {
                    if let Some((key, lbl, nshape)) = node_ids.get(mid) {
                        out.push_str(&format!("        {};\n", format_dot_node(key, lbl, *nshape)));
                    }
                }
                out.push_str("    }\n");
            }
            for (sid, (key, lbl, nshape)) in &node_ids {
                if !member_to_container.contains_key(sid) {
                    out.push_str(&format!("    {};\n", format_dot_node(key, lbl, *nshape)));
                }
            }
            for (f, t, lbl, style, beg_arr, end_arr) in &edges {
                let fk = &node_ids[f].0;
                let tk = &node_ids[t].0;
                let mut attrs = Vec::new();
                if !lbl.is_empty() {
                    attrs.push(format!("label=\"{}\"", escape_quotes(lbl)));
                }
                match style {
                    MarkupEdgeStyle::Dashed => attrs.push("style=dashed".into()),
                    MarkupEdgeStyle::Dotted => attrs.push("style=dotted".into()),
                    MarkupEdgeStyle::Thick => attrs.push("style=bold".into()),
                    MarkupEdgeStyle::Solid => {}
                }
                if *beg_arr && *end_arr {
                    attrs.push("dir=both".into());
                } else if !*end_arr && !*beg_arr {
                    attrs.push("dir=none".into());
                }
                if attrs.is_empty() {
                    out.push_str(&format!("    {fk} -> {tk};\n"));
                } else {
                    out.push_str(&format!("    {fk} -> {tk} [{}];\n", attrs.join(", ")));
                }
            }
            out.push_str("}\n");
            Ok(out)
        }
    }
}

fn format_mermaid_node(key: &str, lbl: &str, shape: MarkupNodeShape) -> String {
    let q = escape_quotes(lbl).replace('\n', "<br/>");
    match shape {
        MarkupNodeShape::Rect | MarkupNodeShape::ClassBox | MarkupNodeShape::EntityBox => {
            format!("{key}[\"{q}\"]")
        }
        MarkupNodeShape::RoundRect => format!("{key}(\"{q}\")"),
        MarkupNodeShape::Stadium => format!("{key}([\"{q}\"])"),
        MarkupNodeShape::Subprocess => format!("{key}[[\"{q}\"]]"),
        MarkupNodeShape::Cylinder => format!("{key}[(\"{q}\")]"),
        MarkupNodeShape::Circle | MarkupNodeShape::Actor => format!("{key}((\"{q}\"))"),
        MarkupNodeShape::Diamond => format!("{key}{{\"{q}\"}}"),
        MarkupNodeShape::Hexagon => format!("{key}{{{{\"{q}\"}}}}"),
        MarkupNodeShape::Parallelogram | MarkupNodeShape::Note => format!("{key}[/\"{q}\"/]"),
        MarkupNodeShape::Trapezoid => format!("{key}[/\"{q}\"\\]"),
    }
}

fn format_plantuml_node(key: &str, lbl: &str, shape: MarkupNodeShape) -> String {
    let q = escape_quotes(lbl).replace('\n', "\\n");
    let kw = match shape {
        MarkupNodeShape::Cylinder => "database",
        MarkupNodeShape::Actor => "actor",
        MarkupNodeShape::Subprocess => "component",
        MarkupNodeShape::Circle => "usecase",
        MarkupNodeShape::Hexagon | MarkupNodeShape::Diamond => "hexagon",
        MarkupNodeShape::ClassBox => "class",
        MarkupNodeShape::EntityBox => "entity",
        MarkupNodeShape::Stadium | MarkupNodeShape::RoundRect => "card",
        MarkupNodeShape::Note => "file",
        _ => "rectangle",
    };
    format!("{kw} \"{q}\" as {key}")
}

fn format_dot_node(key: &str, lbl: &str, shape: MarkupNodeShape) -> String {
    let q = escape_quotes(lbl).replace('\n', "\\n");
    let sh = match shape {
        MarkupNodeShape::RoundRect | MarkupNodeShape::Stadium => "box, style=\"rounded\"",
        MarkupNodeShape::Subprocess => "component",
        MarkupNodeShape::Cylinder => "cylinder",
        MarkupNodeShape::Circle | MarkupNodeShape::Actor => "ellipse",
        MarkupNodeShape::Diamond => "diamond",
        MarkupNodeShape::Hexagon => "hexagon",
        MarkupNodeShape::Parallelogram => "parallelogram",
        MarkupNodeShape::Trapezoid => "trapezium",
        MarkupNodeShape::Note => "note",
        MarkupNodeShape::ClassBox | MarkupNodeShape::EntityBox => "record",
        MarkupNodeShape::Rect => "box",
    };
    format!("{key} [label=\"{q}\", shape={sh}]")
}

fn infer_shape_markup_kind(sh: &VisioShape) -> MarkupNodeShape {
    if let Some(tag) = sh
        .sheet
        .get_cell("User.MarkupNodeShape")
        .and_then(|c| c.value.as_str_coerced().ok())
    {
        if let Some(parsed) = MarkupNodeShape::from_tag(tag.trim_matches('"')) {
            return parsed;
        }
    }
    if let Some(role) = sh
        .sheet
        .get_cell("User.DomainRole")
        .and_then(|c| c.value.as_str_coerced().ok())
    {
        let r = role.trim_matches('"').to_ascii_lowercase();
        if r.contains("decision") || r.contains("gateway") || r.contains("diamond") {
            return MarkupNodeShape::Diamond;
        }
        if r.contains("terminator") || r == "start" || r == "end" || r.contains("pill") {
            return MarkupNodeShape::Stadium;
        }
        if r.contains("db") || r.contains("database") || r.contains("cylinder") {
            return MarkupNodeShape::Cylinder;
        }
        if r == "data" || r.contains("parallelogram") {
            return MarkupNodeShape::Parallelogram;
        }
        if r.contains("subprocess") {
            return MarkupNodeShape::Subprocess;
        }
        if r.contains("actor") {
            return MarkupNodeShape::Actor;
        }
        if r.contains("note") {
            return MarkupNodeShape::Note;
        }
    }
    if let Some(g1) = sh.sheet.geometries.first() {
        if g1.rows.iter().any(|r| r.kind == GeometryRowKind::Ellipse) {
            return MarkupNodeShape::Circle;
        }
        if g1.rows.len() == 5
            && g1
                .rows
                .first()
                .is_some_and(|r| r.x.contains("0.5") && r.y.contains('0'))
        {
            return MarkupNodeShape::Diamond;
        }
    }
    let rounding = sh.sheet.get_f64("Rounding").unwrap_or(0.0);
    let h = sh.sheet.get_f64("Height").unwrap_or(1.0).abs();
    if rounding >= h * 0.4 {
        MarkupNodeShape::Stadium
    } else if rounding > 0.02 {
        MarkupNodeShape::RoundRect
    } else {
        MarkupNodeShape::Rect
    }
}

// ============================================================================
// 1. Mermaid Parser (`flowchart`/`graph`, `sequenceDiagram`, `classDiagram`, `erDiagram`, `stateDiagram`)
// ============================================================================

fn parse_mermaid(markup: &str) -> Result<MarkupGraph, String> {
    let mut body = markup.trim();
    if let Some(after_open) = body.strip_prefix("---") {
        if let Some(end_yaml) = after_open.find("\n---") {
            body = after_open[end_yaml + 4..].trim();
        }
    }

    let mut clean_lines: Vec<String> = Vec::new();
    for raw in body.lines() {
        let mut line = raw.trim().to_string();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if let Some(idx) = line.find("%%") {
            line = line[..idx].trim().to_string();
            if line.is_empty() {
                continue;
            }
        }
        // Split semicolon-separated statements outside quotes
        for part in split_semicolons_outside_quotes(&line) {
            let p = part.trim();
            if !p.is_empty() {
                clean_lines.push(p.to_string());
            }
        }
    }

    let first_lower = clean_lines
        .first()
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if first_lower.starts_with("sequencediagram") {
        return parse_mermaid_sequence(&clean_lines[1..]);
    }
    if first_lower.starts_with("classdiagram") {
        return parse_mermaid_class(&clean_lines[1..]);
    }
    if first_lower.starts_with("erdiagram") {
        return parse_mermaid_er(&clean_lines[1..]);
    }
    if first_lower.starts_with("statediagram") {
        return parse_mermaid_state(&clean_lines[1..]);
    }

    let mut dir = LayoutDir::TopDown;
    let start_idx = if first_lower.starts_with("flowchart") || first_lower.starts_with("graph") {
        let tokens: Vec<&str> = clean_lines[0].split_whitespace().collect();
        if let Some(d) = tokens.get(1) {
            dir = match d.to_ascii_uppercase().as_str() {
                "LR" => LayoutDir::LeftRight,
                "RL" => LayoutDir::RightLeft,
                "BT" => LayoutDir::BottomUp,
                _ => LayoutDir::TopDown,
            };
        }
        1
    } else {
        0
    };

    let mut g = MarkupGraph::new(dir);
    let mut container_stack: Vec<String> = Vec::new();
    let mut class_defs: BTreeMap<String, (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>)> =
        BTreeMap::new();
    let mut pending_node_classes: Vec<(String, String)> = Vec::new();
    let mut direct_styles: Vec<(String, Option<(u8, u8, u8)>, Option<(u8, u8, u8)>)> = Vec::new();

    for line in &clean_lines[start_idx..] {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if lower == "end" {
            container_stack.pop();
            continue;
        }
        if lower.starts_with("direction ") {
            let d = trimmed[10..].trim().to_ascii_uppercase();
            if container_stack.is_empty() {
                g.direction = match d.as_str() {
                    "LR" => LayoutDir::LeftRight,
                    "RL" => LayoutDir::RightLeft,
                    "BT" => LayoutDir::BottomUp,
                    _ => LayoutDir::TopDown,
                };
            }
            continue;
        }
        if lower.starts_with("subgraph ") {
            let rest = trimmed[9..].trim();
            let (sg_id, sg_title) = parse_mermaid_subgraph_header(rest, g.containers.len() + 1);
            g.containers.push(MarkupContainer {
                id: sg_id.clone(),
                title: sg_title,
                member_node_ids: Vec::new(),
            });
            container_stack.push(sg_id);
            continue;
        }
        if lower.starts_with("classdef ") {
            let rest = trimmed[9..].trim();
            if let Some((cname, attrs)) = rest.split_once(char::is_whitespace) {
                let (fill, stroke) = parse_css_like_colors(attrs);
                class_defs.insert(cname.trim().to_string(), (fill, stroke));
            }
            continue;
        }
        if lower.starts_with("class ") {
            let rest = trimmed[6..].trim();
            if let Some((nodes_csv, cname)) = rest.rsplit_once(char::is_whitespace) {
                for nid in nodes_csv.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                    pending_node_classes.push((sanitize_id_key(nid), cname.trim().to_string()));
                }
            }
            continue;
        }
        if lower.starts_with("style ") {
            let rest = trimmed[6..].trim();
            if let Some((nid, attrs)) = rest.split_once(char::is_whitespace) {
                let (fill, stroke) = parse_css_like_colors(attrs);
                direct_styles.push((sanitize_id_key(nid), fill, stroke));
            }
            continue;
        }
        if lower.starts_with("click ") || lower.starts_with("linkstyle ") {
            continue;
        }

        let active_cont = container_stack.last().map(String::as_str);
        parse_mermaid_flowchart_statement(&mut g, trimmed, active_cont, &mut pending_node_classes);
    }

    for (nid, cname) in pending_node_classes {
        if let (Some(node), Some(&(fill, stroke))) = (g.nodes.get_mut(&nid), class_defs.get(&cname))
        {
            if fill.is_some() {
                node.fill_rgb = fill;
            }
            if stroke.is_some() {
                node.stroke_rgb = stroke;
            }
        }
    }
    for (nid, fill, stroke) in direct_styles {
        if let Some(node) = g.nodes.get_mut(&nid) {
            if fill.is_some() {
                node.fill_rgb = fill;
            }
            if stroke.is_some() {
                node.stroke_rgb = stroke;
            }
        }
    }

    Ok(g)
}

fn parse_mermaid_subgraph_header(rest: &str, idx: usize) -> (String, String) {
    if let Some(open_b) = rest.find('[') {
        if let Some(close_b) = rest.rfind(']') {
            let raw_id = rest[..open_b].trim();
            let id = if raw_id.is_empty() {
                format!("sg_{idx}")
            } else {
                sanitize_id_key(raw_id)
            };
            let title = rest[open_b + 1..close_b]
                .trim()
                .trim_matches('"')
                .to_string();
            return (id, title);
        }
    }
    let clean = rest.trim().trim_matches('"');
    let id = if clean.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !clean.is_empty() {
        clean.to_string()
    } else {
        format!("sg_{idx}")
    };
    (id, clean.to_string())
}

#[derive(Debug, Clone)]
struct ParsedMermaidArrow {
    pos: usize,
    end_pos: usize,
    label: String,
    style: MarkupEdgeStyle,
    begin_arrow: u32,
    end_arrow: u32,
}

fn find_next_mermaid_arrow(s: &str) -> Option<ParsedMermaidArrow> {
    let bytes = s.as_bytes();
    let mut in_quotes = false;
    let mut bracket_depth: i32 = 0;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if !in_quotes {
            match b {
                b'[' | b'(' | b'{' => {
                    bracket_depth += 1;
                    i += 1;
                    continue;
                }
                b']' | b')' | b'}' => {
                    bracket_depth = (bracket_depth - 1).max(0);
                    i += 1;
                    continue;
                }
                _ => {}
            }
            if bracket_depth == 0 {
                let tail = &s[i..];
                // Inline labeled arrows: `-- text -->`, `-. text .->`, `== text ==>`
                if let Some(after) = tail.strip_prefix("-- ") {
                    if let Some(end_rel) = after.find("-->") {
                        let lbl = after[..end_rel].trim().trim_matches('-').trim().to_string();
                        return Some(ParsedMermaidArrow {
                            pos: i,
                            end_pos: i + 3 + end_rel + 3,
                            label: lbl,
                            style: MarkupEdgeStyle::Solid,
                            begin_arrow: 0,
                            end_arrow: 4,
                        });
                    }
                }
                if let Some(after) = tail.strip_prefix("-. ") {
                    if let Some(end_rel) = after.find(".->") {
                        let lbl = after[..end_rel].trim().to_string();
                        return Some(ParsedMermaidArrow {
                            pos: i,
                            end_pos: i + 3 + end_rel + 3,
                            label: lbl,
                            style: MarkupEdgeStyle::Dashed,
                            begin_arrow: 0,
                            end_arrow: 4,
                        });
                    }
                }
                if let Some(after) = tail.strip_prefix("== ") {
                    if let Some(end_rel) = after.find("==>") {
                        let lbl = after[..end_rel].trim().to_string();
                        return Some(ParsedMermaidArrow {
                            pos: i,
                            end_pos: i + 3 + end_rel + 3,
                            label: lbl,
                            style: MarkupEdgeStyle::Thick,
                            begin_arrow: 0,
                            end_arrow: 4,
                        });
                    }
                }

                for &(pat, style, beg_a, end_a) in &[
                    ("<-->", MarkupEdgeStyle::Solid, 4u32, 4u32),
                    ("<-.->", MarkupEdgeStyle::Dashed, 4, 4),
                    ("<==>", MarkupEdgeStyle::Thick, 4, 4),
                    ("-.->", MarkupEdgeStyle::Dashed, 0, 4),
                    ("-..->", MarkupEdgeStyle::Dashed, 0, 4),
                    ("==>", MarkupEdgeStyle::Thick, 0, 4),
                    ("-->", MarkupEdgeStyle::Solid, 0, 4),
                    ("---", MarkupEdgeStyle::Solid, 0, 0),
                    ("-.-", MarkupEdgeStyle::Dashed, 0, 0),
                    ("===", MarkupEdgeStyle::Thick, 0, 0),
                ] {
                    if let Some(after_arrow) = tail.strip_prefix(pat) {
                        let trimmed_after = after_arrow.trim_start();
                        let ws_len = after_arrow.len() - trimmed_after.len();
                        if let Some(after_pipe) = trimmed_after.strip_prefix('|') {
                            if let Some(close_pipe) = after_pipe.find('|') {
                                let lbl = after_pipe[..close_pipe].trim().to_string();
                                return Some(ParsedMermaidArrow {
                                    pos: i,
                                    end_pos: i + pat.len() + ws_len + 1 + close_pipe + 1,
                                    label: lbl,
                                    style,
                                    begin_arrow: beg_a,
                                    end_arrow: end_a,
                                });
                            }
                        }
                        return Some(ParsedMermaidArrow {
                            pos: i,
                            end_pos: i + pat.len(),
                            label: String::new(),
                            style,
                            begin_arrow: beg_a,
                            end_arrow: end_a,
                        });
                    }
                }
            }
        }
        i += 1;
    }
    None
}

fn parse_mermaid_flowchart_statement(
    g: &mut MarkupGraph,
    stmt: &str,
    container_id: Option<&str>,
    pending_classes: &mut Vec<(String, String)>,
) {
    let mut segments: Vec<&str> = Vec::new();
    let mut arrows: Vec<ParsedMermaidArrow> = Vec::new();
    let mut rest = stmt;
    while let Some(arr) = find_next_mermaid_arrow(rest) {
        segments.push(rest[..arr.pos].trim());
        rest = rest[arr.end_pos..].trim();
        arrows.push(arr);
    }
    segments.push(rest.trim());

    let mut group_ids: Vec<Vec<String>> = Vec::with_capacity(segments.len());
    for seg in segments {
        let mut ids = Vec::new();
        for part in split_ampersands_outside_brackets(seg) {
            if let Some(nid) =
                parse_and_register_mermaid_node_token(g, part.trim(), container_id, pending_classes)
            {
                ids.push(nid);
            }
        }
        group_ids.push(ids);
    }

    for (idx, arr) in arrows.into_iter().enumerate() {
        let left = &group_ids[idx];
        let right = &group_ids[idx + 1];
        for f in left {
            for t in right {
                g.edges.push(MarkupEdge {
                    from_id: f.clone(),
                    to_id: t.clone(),
                    label: unescape_diagram_text(&arr.label),
                    style: arr.style,
                    begin_arrow: arr.begin_arrow,
                    end_arrow: arr.end_arrow,
                });
            }
        }
    }
}

fn parse_and_register_mermaid_node_token(
    g: &mut MarkupGraph,
    token: &str,
    container_id: Option<&str>,
    pending_classes: &mut Vec<(String, String)>,
) -> Option<String> {
    if token.is_empty() {
        return None;
    }
    let (token_main, opt_cls) = if let Some((lhs, cls)) = token.rsplit_once(":::") {
        (lhs.trim(), Some(cls.trim()))
    } else {
        (token.trim(), None)
    };

    // Find first opening delimiter `[`, `(`, `{`
    let delim_pos = token_main.find(['[', '(', '{']);
    let (raw_id, label_opt, shape_opt) = if let Some(pos) = delim_pos {
        let id_part = token_main[..pos].trim();
        let tail = token_main[pos..].trim();
        let (lbl, sh) = parse_mermaid_shape_delimiters(tail);
        (
            if id_part.is_empty() { tail } else { id_part },
            Some(lbl),
            Some(sh),
        )
    } else {
        (token_main, None, None)
    };

    let clean_id = sanitize_id_key(raw_id);
    if clean_id.is_empty() {
        return None;
    }
    g.upsert_node(&clean_id, label_opt.as_deref(), shape_opt, container_id);
    if let Some(cls) = opt_cls.filter(|s| !s.is_empty()) {
        pending_classes.push((clean_id.clone(), cls.to_string()));
    }
    Some(clean_id)
}

fn parse_mermaid_shape_delimiters(tail: &str) -> (String, MarkupNodeShape) {
    let strip_q = |s: &str| s.trim().trim_matches('"').trim().to_string();
    if let Some(inner) = tail.strip_prefix("([") .and_then(|s| s.strip_suffix("])")) {
        return (strip_q(inner), MarkupNodeShape::Stadium);
    }
    if let Some(inner) = tail.strip_prefix("[[").and_then(|s| s.strip_suffix("]]")) {
        return (strip_q(inner), MarkupNodeShape::Subprocess);
    }
    if let Some(inner) = tail.strip_prefix("[(").and_then(|s| s.strip_suffix(")]")) {
        return (strip_q(inner), MarkupNodeShape::Cylinder);
    }
    if let Some(inner) = tail.strip_prefix("(((").and_then(|s| s.strip_suffix(")))")) {
        return (strip_q(inner), MarkupNodeShape::Circle);
    }
    if let Some(inner) = tail.strip_prefix("((").and_then(|s| s.strip_suffix("))")) {
        return (strip_q(inner), MarkupNodeShape::Circle);
    }
    if let Some(inner) = tail.strip_prefix("{{").and_then(|s| s.strip_suffix("}}")) {
        return (strip_q(inner), MarkupNodeShape::Hexagon);
    }
    if let Some(inner) = tail.strip_prefix("[/").and_then(|s| s.strip_suffix("/]")) {
        return (strip_q(inner), MarkupNodeShape::Parallelogram);
    }
    if let Some(inner) = tail.strip_prefix("[/").and_then(|s| s.strip_suffix("\\]")) {
        return (strip_q(inner), MarkupNodeShape::Trapezoid);
    }
    if let Some(inner) = tail.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        return (strip_q(inner), MarkupNodeShape::Diamond);
    }
    if let Some(inner) = tail.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        return (strip_q(inner), MarkupNodeShape::RoundRect);
    }
    if let Some(inner) = tail.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return (strip_q(inner), MarkupNodeShape::Rect);
    }
    (strip_q(tail), MarkupNodeShape::Rect)
}

fn parse_mermaid_sequence(lines: &[String]) -> Result<MarkupGraph, String> {
    let mut g = MarkupGraph::new(LayoutDir::LeftRight);
    let mut active_box: Option<String> = None;
    for line in lines {
        let lower = line.to_ascii_lowercase();
        if lower == "end" {
            active_box = None;
            continue;
        }
        if lower.starts_with("box ") {
            let title = line[4..].trim().trim_matches('"').to_string();
            let cid = format!("box_{}", g.containers.len() + 1);
            g.containers.push(MarkupContainer {
                id: cid.clone(),
                title: if title.is_empty() { "Group".into() } else { title },
                member_node_ids: Vec::new(),
            });
            active_box = Some(cid);
            continue;
        }
        if lower.starts_with("participant ") || lower.starts_with("actor ") {
            let is_actor = lower.starts_with("actor ");
            let rest = if is_actor { &line[6..] } else { &line[12..] }.trim();
            let (id, lbl) = split_as_alias(rest, false);
            g.upsert_node(
                &id,
                Some(&lbl),
                Some(if is_actor {
                    MarkupNodeShape::Actor
                } else {
                    MarkupNodeShape::RoundRect
                }),
                active_box.as_deref(),
            );
            continue;
        }
        for &(pat, dashed) in &[
            ("-->>", true),
            ("->>", false),
            ("-->", true),
            ("->", false),
            ("--x", true),
            ("-x", false),
        ] {
            if let Some((lhs, rhs_full)) = line.split_once(pat) {
                let (rhs, msg) = rhs_full
                    .split_once(':')
                    .map(|(a, b)| (a.trim(), b.trim()))
                    .unwrap_or((rhs_full.trim(), ""));
                let from = sanitize_id_key(lhs.trim());
                let to = sanitize_id_key(rhs.trim_end_matches(['+', '-']));
                if !from.is_empty() && !to.is_empty() {
                    g.upsert_node(&from, None, Some(MarkupNodeShape::RoundRect), None);
                    g.upsert_node(&to, None, Some(MarkupNodeShape::RoundRect), None);
                    g.edges.push(MarkupEdge {
                        from_id: from,
                        to_id: to,
                        label: msg.to_string(),
                        style: if dashed {
                            MarkupEdgeStyle::Dashed
                        } else {
                            MarkupEdgeStyle::Solid
                        },
                        begin_arrow: 0,
                        end_arrow: 4,
                    });
                }
                break;
            }
        }
    }
    Ok(g)
}

fn parse_mermaid_class(lines: &[String]) -> Result<MarkupGraph, String> {
    let mut g = MarkupGraph::new(LayoutDir::TopDown);
    let mut active_class: Option<(String, Vec<String>)> = None;
    for line in lines {
        let trimmed = line.trim();
        if let Some((ref cname, ref mut members)) = active_class {
            if trimmed == "}" {
                let full_lbl = if members.is_empty() {
                    cname.clone()
                } else {
                    format!("{cname}\n{}", members.join("\n"))
                };
                let cid = cname.clone();
                active_class = None;
                g.upsert_node(&cid, Some(&full_lbl), Some(MarkupNodeShape::ClassBox), None);
            } else {
                members.push(trimmed.to_string());
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("class ") {
            if let Some(open_b) = rest.find('{') {
                let cname = sanitize_id_key(rest[..open_b].trim());
                active_class = Some((cname, Vec::new()));
            } else {
                let (id, lbl) = split_as_alias(rest, false);
                g.upsert_node(&id, Some(&lbl), Some(MarkupNodeShape::ClassBox), None);
            }
            continue;
        }
        let mut matched_rel = false;
        for &(pat, dashed, beg_a, end_a) in &[
            ("<|--", false, 14u32, 0u32),
            ("--|>", false, 0, 14),
            ("..|>", true, 0, 14),
            ("<|..", true, 14, 0),
            ("*--", false, 22, 0),
            ("--*", false, 0, 22),
            ("o--", false, 20, 0),
            ("--o", false, 0, 20),
            ("-->", false, 0, 4),
            ("..>", true, 0, 4),
            ("--", false, 0, 0),
        ] {
            if let Some((lhs, rhs_full)) = trimmed.split_once(pat) {
                let (rhs, lbl) = rhs_full
                    .split_once(':')
                    .map(|(a, b)| (a.trim(), b.trim()))
                    .unwrap_or((rhs_full.trim(), ""));
                let from = sanitize_id_key(lhs.split_whitespace().next().unwrap_or(lhs));
                let to = sanitize_id_key(rhs.split_whitespace().next_back().unwrap_or(rhs));
                if !from.is_empty() && !to.is_empty() {
                    g.upsert_node(&from, None, Some(MarkupNodeShape::ClassBox), None);
                    g.upsert_node(&to, None, Some(MarkupNodeShape::ClassBox), None);
                    g.edges.push(MarkupEdge {
                        from_id: from,
                        to_id: to,
                        label: lbl.trim_matches('"').to_string(),
                        style: if dashed {
                            MarkupEdgeStyle::Dashed
                        } else {
                            MarkupEdgeStyle::Solid
                        },
                        begin_arrow: beg_a,
                        end_arrow: end_a,
                    });
                    matched_rel = true;
                }
                break;
            }
        }
        if !matched_rel {
            if let Some((cname, member)) = trimmed.split_once(':') {
                let cid = sanitize_id_key(cname.trim());
                if !cid.is_empty() {
                    let prev = g
                        .nodes
                        .get(&cid)
                        .map(|n| n.label.clone())
                        .unwrap_or_else(|| cname.trim().to_string());
                    let updated = format!("{prev}\n{}", member.trim());
                    g.upsert_node(&cid, Some(&updated), Some(MarkupNodeShape::ClassBox), None);
                }
            }
        }
    }
    Ok(g)
}

fn parse_mermaid_er(lines: &[String]) -> Result<MarkupGraph, String> {
    let mut g = MarkupGraph::new(LayoutDir::LeftRight);
    let mut active_ent: Option<(String, Vec<String>)> = None;
    for line in lines {
        let trimmed = line.trim();
        if let Some((ref ename, ref mut cols)) = active_ent {
            if trimmed == "}" {
                let lbl = if cols.is_empty() {
                    ename.clone()
                } else {
                    format!("[{ename}]\n{}", cols.join("\n"))
                };
                let eid = ename.clone();
                active_ent = None;
                g.upsert_node(&eid, Some(&lbl), Some(MarkupNodeShape::EntityBox), None);
            } else {
                cols.push(trimmed.to_string());
            }
            continue;
        }
        if let Some(open_b) = trimmed.find('{') {
            if !trimmed.contains("--") && !trimmed.contains("..") {
                let ename = sanitize_id_key(trimmed[..open_b].trim());
                if !ename.is_empty() {
                    active_ent = Some((ename, Vec::new()));
                    continue;
                }
            }
        }
        if trimmed.contains("--") || trimmed.contains("..") {
            let (left_part, lbl_part) = trimmed
                .split_once(':')
                .map(|(a, b)| (a.trim(), b.trim().trim_matches('"')))
                .unwrap_or((trimmed, ""));
            let tokens: Vec<&str> = left_part.split_whitespace().collect();
            if tokens.len() >= 3 {
                let e1 = sanitize_id_key(tokens[0]);
                let rel_sym = tokens[1];
                let e2 = sanitize_id_key(tokens[2]);
                let dashed = rel_sym.contains("..");
                let end_crows = if rel_sym.ends_with('{') { 27 } else { 4 };
                g.upsert_node(&e1, None, Some(MarkupNodeShape::EntityBox), None);
                g.upsert_node(&e2, None, Some(MarkupNodeShape::EntityBox), None);
                g.edges.push(MarkupEdge {
                    from_id: e1,
                    to_id: e2,
                    label: lbl_part.to_string(),
                    style: if dashed {
                        MarkupEdgeStyle::Dashed
                    } else {
                        MarkupEdgeStyle::Solid
                    },
                    begin_arrow: 0,
                    end_arrow: end_crows,
                });
            }
        }
    }
    Ok(g)
}

fn parse_mermaid_state(lines: &[String]) -> Result<MarkupGraph, String> {
    let mut g = MarkupGraph::new(LayoutDir::TopDown);
    let mut container_stack: Vec<String> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "}" {
            container_stack.pop();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("state ") {
            if let Some(open_b) = rest.find('{') {
                let header = rest[..open_b].trim();
                let (sid, lbl) = split_as_alias(header, false);
                g.containers.push(MarkupContainer {
                    id: sid.clone(),
                    title: lbl,
                    member_node_ids: Vec::new(),
                });
                container_stack.push(sid);
            } else {
                let (sid, lbl) = split_as_alias(rest, false);
                g.upsert_node(
                    &sid,
                    Some(&lbl),
                    Some(MarkupNodeShape::RoundRect),
                    container_stack.last().map(String::as_str),
                );
            }
            continue;
        }
        if let Some((lhs, rhs_full)) = trimmed.split_once("-->") {
            let (rhs, lbl) = rhs_full
                .split_once(':')
                .map(|(a, b)| (a.trim(), b.trim()))
                .unwrap_or((rhs_full.trim(), ""));
            let from_raw = lhs.trim();
            let to_raw = rhs.trim();
            let from = if from_raw == "[*]" {
                g.upsert_node(
                    "StartState",
                    Some("Start"),
                    Some(MarkupNodeShape::Circle),
                    container_stack.last().map(String::as_str),
                );
                "StartState".to_string()
            } else {
                let id = sanitize_id_key(from_raw);
                g.upsert_node(
                    &id,
                    None,
                    Some(MarkupNodeShape::RoundRect),
                    container_stack.last().map(String::as_str),
                );
                id
            };
            let to = if to_raw == "[*]" {
                g.upsert_node(
                    "EndState",
                    Some("End"),
                    Some(MarkupNodeShape::Circle),
                    container_stack.last().map(String::as_str),
                );
                "EndState".to_string()
            } else {
                let id = sanitize_id_key(to_raw);
                g.upsert_node(
                    &id,
                    None,
                    Some(MarkupNodeShape::RoundRect),
                    container_stack.last().map(String::as_str),
                );
                id
            };
            g.edges.push(MarkupEdge {
                from_id: from,
                to_id: to,
                label: lbl.to_string(),
                style: MarkupEdgeStyle::Solid,
                begin_arrow: 0,
                end_arrow: 4,
            });
        }
    }
    Ok(g)
}

// ============================================================================
// 2. PlantUML Parser (`@startuml ... @enduml`)
// ============================================================================

fn parse_plantuml(markup: &str) -> Result<MarkupGraph, String> {
    let mut g = MarkupGraph::new(LayoutDir::TopDown);
    let mut container_stack: Vec<String> = Vec::new();
    let mut in_block_comment = false;
    let mut active_class: Option<(String, String, Vec<String>)> = None;

    for raw in markup.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('\'') {
            continue;
        }
        if trimmed.starts_with("/'") {
            if !trimmed.contains("'/") {
                in_block_comment = true;
            }
            continue;
        }
        if in_block_comment {
            if trimmed.contains("'/") {
                in_block_comment = false;
            }
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with("@start")
            || lower.starts_with("@end")
            || lower.starts_with("skinparam")
            || lower.starts_with("hide ")
            || lower.starts_with("scale ")
            || lower.starts_with("title ")
        {
            continue;
        }
        if lower == "left to right direction" {
            g.direction = LayoutDir::LeftRight;
            continue;
        }
        if lower == "top to bottom direction" {
            g.direction = LayoutDir::TopDown;
            continue;
        }

        if let Some((ref cid, ref clabel, ref mut members)) = active_class {
            if trimmed == "}" {
                let full = if members.is_empty() {
                    clabel.clone()
                } else {
                    format!("{clabel}\n{}", members.join("\n"))
                };
                let id_copy = cid.clone();
                active_class = None;
                g.upsert_node(
                    &id_copy,
                    Some(&full),
                    Some(MarkupNodeShape::ClassBox),
                    container_stack.last().map(String::as_str),
                );
            } else {
                members.push(trimmed.to_string());
            }
            continue;
        }

        if trimmed == "}" {
            container_stack.pop();
            continue;
        }

        // Container blocks: `package "Core" as pkg {`, `namespace`, `folder`, `frame`, `node "..." {`, `cloud "..." {`
        let cont_prefixes = [
            "package ",
            "namespace ",
            "folder ",
            "frame ",
            "cloud ",
            "together ",
        ];
        if let Some(pfx) = cont_prefixes.iter().find(|&&p| lower.starts_with(p)) {
            if let Some(open_b) = trimmed.find('{') {
                let hdr = trimmed[pfx.len()..open_b].trim();
                let (id, lbl) = split_as_alias(hdr, true);
                let cid = if id.is_empty() {
                    format!("pkg_{}", g.containers.len() + 1)
                } else {
                    id
                };
                g.containers.push(MarkupContainer {
                    id: cid.clone(),
                    title: lbl,
                    member_node_ids: Vec::new(),
                });
                container_stack.push(cid);
                continue;
            }
        }

        // Class / interface / enum with `{ ... }`
        for kw in ["class ", "interface ", "abstract class ", "enum ", "entity "] {
            if lower.starts_with(kw) && trimmed.ends_with('{') {
                let hdr = trimmed[kw.len()..trimmed.len() - 1].trim();
                let (id, lbl) = split_as_alias(hdr, true);
                active_class = Some((id, lbl, Vec::new()));
                break;
            }
        }
        if active_class.is_some() {
            continue;
        }

        // Note single-line: `note right of X : text` or `note "text" as N1`
        if lower.starts_with("note ") {
            let rest = trimmed[5..].trim();
            if let Some((lhs, note_txt)) = rest.split_once(':') {
                let target = lhs
                    .split_whitespace()
                    .next_back()
                    .map(sanitize_id_key)
                    .unwrap_or_default();
                let nid = format!("Note_{}", g.nodes.len() + 1);
                g.upsert_node(
                    &nid,
                    Some(note_txt.trim()),
                    Some(MarkupNodeShape::Note),
                    container_stack.last().map(String::as_str),
                );
                if !target.is_empty() && target != "of" {
                    g.upsert_node(
                        &target,
                        None,
                        None,
                        container_stack.last().map(String::as_str),
                    );
                    g.edges.push(MarkupEdge {
                        from_id: nid,
                        to_id: target,
                        label: String::new(),
                        style: MarkupEdgeStyle::Dashed,
                        begin_arrow: 0,
                        end_arrow: 0,
                    });
                }
            } else {
                let (id, lbl) = split_as_alias(rest, true);
                g.upsert_node(
                    &id,
                    Some(&lbl),
                    Some(MarkupNodeShape::Note),
                    container_stack.last().map(String::as_str),
                );
            }
            continue;
        }

        // Keyword node declarations without edges
        let node_keywords: &[(&str, MarkupNodeShape)] = &[
            ("actor ", MarkupNodeShape::Actor),
            ("participant ", MarkupNodeShape::RoundRect),
            ("database ", MarkupNodeShape::Cylinder),
            ("queue ", MarkupNodeShape::Cylinder),
            ("component ", MarkupNodeShape::Subprocess),
            ("rectangle ", MarkupNodeShape::Rect),
            ("card ", MarkupNodeShape::RoundRect),
            ("node ", MarkupNodeShape::Rect),
            ("storage ", MarkupNodeShape::Stadium),
            ("usecase ", MarkupNodeShape::Circle),
            ("hexagon ", MarkupNodeShape::Hexagon),
            ("collections ", MarkupNodeShape::Subprocess),
            ("file ", MarkupNodeShape::Note),
            ("class ", MarkupNodeShape::ClassBox),
            ("entity ", MarkupNodeShape::EntityBox),
            ("state ", MarkupNodeShape::RoundRect),
        ];
        let has_arrow = trimmed.contains("->")
            || trimmed.contains("<--")
            || trimmed.contains("..>")
            || trimmed.contains("<..")
            || trimmed.contains("*--")
            || trimmed.contains("o--")
            || trimmed.contains("<|--");

        if !has_arrow {
            let mut matched_kw = false;
            for &(kw, sh) in node_keywords {
                if lower.starts_with(kw) {
                    let rest = trimmed[kw.len()..].trim().trim_end_matches('{').trim();
                    let (id, lbl) = split_as_alias(rest, true);
                    g.upsert_node(
                        &id,
                        Some(&lbl),
                        Some(sh),
                        container_stack.last().map(String::as_str),
                    );
                    matched_kw = true;
                    break;
                }
            }
            if matched_kw {
                continue;
            }
        }

        // Edge lines (`A --> B : label`, `[Comp A] ..> [Comp B] : REST`, `User -> (Login)`)
        if let Some((from_tok, to_tok, lbl, style, beg_a, end_a)) = parse_plantuml_edge_line(trimmed)
        {
            let fid = register_plantuml_endpoint(
                &mut g,
                &from_tok,
                container_stack.last().map(String::as_str),
            );
            let tid = register_plantuml_endpoint(
                &mut g,
                &to_tok,
                container_stack.last().map(String::as_str),
            );
            if !fid.is_empty() && !tid.is_empty() {
                g.edges.push(MarkupEdge {
                    from_id: fid,
                    to_id: tid,
                    label: unescape_diagram_text(&lbl),
                    style,
                    begin_arrow: beg_a,
                    end_arrow: end_a,
                });
            }
        }
    }
    Ok(g)
}

fn parse_plantuml_edge_line(
    line: &str,
) -> Option<(String, String, String, MarkupEdgeStyle, u32, u32)> {
    let (main_part, label_part) = split_colon_outside_brackets_and_quotes(line);
    // Normalize styled PlantUML arrows like `-[dashed]->` or `-[#blue,bold]->` or `-down->`
    for &(pat, style, beg_a, end_a) in &[
        ("<-->", MarkupEdgeStyle::Solid, 4u32, 4u32),
        ("<..>", MarkupEdgeStyle::Dashed, 4, 4),
        ("<|--", MarkupEdgeStyle::Solid, 14, 0),
        ("--|>", MarkupEdgeStyle::Solid, 0, 14),
        ("..|>", MarkupEdgeStyle::Dashed, 0, 14),
        ("*--", MarkupEdgeStyle::Solid, 22, 0),
        ("o--", MarkupEdgeStyle::Solid, 20, 0),
        ("..>", MarkupEdgeStyle::Dashed, 0, 4),
        (".>", MarkupEdgeStyle::Dashed, 0, 4),
        ("<..", MarkupEdgeStyle::Dashed, 4, 0),
        ("-->", MarkupEdgeStyle::Solid, 0, 4),
        ("<--", MarkupEdgeStyle::Solid, 4, 0),
        ("->", MarkupEdgeStyle::Solid, 0, 4),
        ("<-", MarkupEdgeStyle::Solid, 4, 0),
        ("--", MarkupEdgeStyle::Solid, 0, 0),
        ("..", MarkupEdgeStyle::Dashed, 0, 0),
    ] {
        if let Some((lhs, rhs)) = split_outside_brackets_and_quotes(main_part, pat) {
            return Some((
                lhs.trim().to_string(),
                rhs.trim().to_string(),
                label_part.trim().to_string(),
                style,
                beg_a,
                end_a,
            ));
        }
    }
    // Check directional PlantUML arrow `-down->`, `-right->`, `-[dashed]->`
    if let Some(dash_pos) = main_part.find('-') {
        if let Some(gt_rel) = main_part[dash_pos + 1..].find("->") {
            let mid = &main_part[dash_pos + 1..dash_pos + 1 + gt_rel];
            if !mid.contains(' ') {
                let lhs = main_part[..dash_pos].trim();
                let rhs = main_part[dash_pos + 1 + gt_rel + 2..].trim();
                let style = if mid.contains("dashed") || mid.contains("dotted") {
                    MarkupEdgeStyle::Dashed
                } else if mid.contains("bold") || mid.contains("thickness") {
                    MarkupEdgeStyle::Thick
                } else {
                    MarkupEdgeStyle::Solid
                };
                return Some((
                    lhs.to_string(),
                    rhs.to_string(),
                    label_part.trim().to_string(),
                    style,
                    0,
                    4,
                ));
            }
        }
    }
    None
}

fn register_plantuml_endpoint(
    g: &mut MarkupGraph,
    tok: &str,
    container_id: Option<&str>,
) -> String {
    let t = tok.trim();
    if t == "[*]" {
        let id = "StateTerminal".to_string();
        g.upsert_node(&id, Some("●"), Some(MarkupNodeShape::Circle), container_id);
        return id;
    }
    if let Some(inner) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let id = sanitize_id_key(inner);
        g.upsert_node(
            &id,
            Some(inner.trim()),
            Some(MarkupNodeShape::Subprocess),
            container_id,
        );
        return id;
    }
    if let Some(inner) = t.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        let id = sanitize_id_key(inner);
        g.upsert_node(
            &id,
            Some(inner.trim()),
            Some(MarkupNodeShape::Circle),
            container_id,
        );
        return id;
    }
    if let Some(inner) = t.strip_prefix(':').and_then(|s| s.strip_suffix(':')) {
        let id = sanitize_id_key(inner);
        g.upsert_node(
            &id,
            Some(inner.trim()),
            Some(MarkupNodeShape::Actor),
            container_id,
        );
        return id;
    }
    let clean = t.trim_matches('"');
    let id = sanitize_id_key(clean);
    g.upsert_node(&id, Some(clean), None, container_id);
    id
}

// ============================================================================
// 3. Graphviz DOT Parser (`digraph` / `graph`)
// ============================================================================

fn parse_graphviz_dot(markup: &str) -> Result<MarkupGraph, String> {
    let stripped = strip_c_and_hash_comments(markup);
    let mut g = MarkupGraph::new(LayoutDir::TopDown);
    let is_undirected = {
        let l = stripped.trim_start().to_ascii_lowercase();
        (l.starts_with("graph ") || l.starts_with("strict graph")) && !l.contains("digraph")
    };

    // Find outer `{ ... }`
    let body = if let (Some(open_b), Some(close_b)) = (stripped.find('{'), stripped.rfind('}')) {
        if open_b < close_b {
            &stripped[open_b + 1..close_b]
        } else {
            stripped.as_str()
        }
    } else {
        stripped.as_str()
    };

    let mut default_shape = MarkupNodeShape::Rect;
    parse_dot_block(
        &mut g,
        body,
        None,
        is_undirected,
        &mut default_shape,
    );
    Ok(g)
}

fn parse_dot_block(
    g: &mut MarkupGraph,
    block: &str,
    container_id: Option<&str>,
    is_undirected: bool,
    default_shape: &mut MarkupNodeShape,
) {
    for stmt in split_dot_statements(block) {
        let trimmed = stmt.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();

        // Nested `subgraph cluster_foo { ... }`
        if lower.starts_with("subgraph") || trimmed.starts_with('{') {
            if let (Some(ob), Some(cb)) = (trimmed.find('{'), trimmed.rfind('}')) {
                if ob < cb {
                    let hdr = trimmed[..ob].trim();
                    let inner = &trimmed[ob + 1..cb];
                    let raw_name = hdr
                        .strip_prefix("subgraph")
                        .or_else(|| hdr.strip_prefix("SUBGRAPH"))
                        .map(str::trim)
                        .unwrap_or("")
                        .trim_matches('"');
                    let is_cluster = raw_name.to_ascii_lowercase().starts_with("cluster")
                        || inner.to_ascii_lowercase().contains("label");
                    if is_cluster {
                        let cid = if raw_name.is_empty() {
                            format!("cluster_{}", g.containers.len() + 1)
                        } else {
                            sanitize_id_key(raw_name)
                        };
                        let label_from_inner = extract_dot_top_label(inner).unwrap_or_else(|| {
                            raw_name
                                .strip_prefix("cluster_")
                                .unwrap_or(raw_name)
                                .to_string()
                        });
                        g.containers.push(MarkupContainer {
                            id: cid.clone(),
                            title: if label_from_inner.is_empty() {
                                cid.clone()
                            } else {
                                label_from_inner
                            },
                            member_node_ids: Vec::new(),
                        });
                        let mut sub_default = *default_shape;
                        parse_dot_block(g, inner, Some(&cid), is_undirected, &mut sub_default);
                    } else {
                        let mut sub_default = *default_shape;
                        parse_dot_block(g, inner, container_id, is_undirected, &mut sub_default);
                    }
                    continue;
                }
            }
        }

        // Graph-level `rankdir = LR` or `label = "..."`
        if !trimmed.contains("->") && !trimmed.contains("--") && !trimmed.contains('[') {
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_ascii_lowercase();
                let val = v.trim().trim_matches('"').to_ascii_uppercase();
                if key == "rankdir" && container_id.is_none() {
                    g.direction = match val.as_str() {
                        "LR" => LayoutDir::LeftRight,
                        "RL" => LayoutDir::RightLeft,
                        "BT" => LayoutDir::BottomUp,
                        _ => LayoutDir::TopDown,
                    };
                }
                continue;
            }
        }

        // Split trailing `[attr=val, ...]`
        let (head_expr, attr_map) = split_dot_trailing_attrs(trimmed);
        let head_lower = head_expr.trim().to_ascii_lowercase();
        if head_lower == "graph" || head_lower == "edge" {
            if let Some(rd) = attr_map.get("rankdir") {
                g.direction = match rd.to_ascii_uppercase().as_str() {
                    "LR" => LayoutDir::LeftRight,
                    "RL" => LayoutDir::RightLeft,
                    "BT" => LayoutDir::BottomUp,
                    _ => LayoutDir::TopDown,
                };
            }
            continue;
        }
        if head_lower == "node" {
            if let Some(sh_str) = attr_map.get("shape") {
                if let Some(parsed) = dot_shape_to_markup(sh_str, attr_map.get("style")) {
                    *default_shape = parsed;
                }
            }
            continue;
        }

        // Edge chain: `A -> B -> C` or `A -- B`
        let edge_op = if head_expr.contains("->") {
            Some("->")
        } else if head_expr.contains("--") {
            Some("--")
        } else {
            None
        };

        if let Some(op) = edge_op {
            let hops: Vec<&str> = split_outside_quotes_by_op(head_expr, op);
            let lbl = attr_map
                .get("label")
                .or_else(|| attr_map.get("xlabel"))
                .cloned()
                .unwrap_or_default();
            let style_attr = attr_map
                .get("style")
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let edge_style = if style_attr.contains("dashed") {
                MarkupEdgeStyle::Dashed
            } else if style_attr.contains("dotted") {
                MarkupEdgeStyle::Dotted
            } else if style_attr.contains("bold") {
                MarkupEdgeStyle::Thick
            } else {
                MarkupEdgeStyle::Solid
            };
            let dir_attr = attr_map
                .get("dir")
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let (beg_a, end_a) = if is_undirected || op == "--" || dir_attr == "none" {
                (0, 0)
            } else if dir_attr == "both" {
                (4, 4)
            } else if dir_attr == "back" {
                (4, 0)
            } else {
                (0, 4)
            };

            let mut hop_ids = Vec::new();
            for h in hops {
                // Strip optional DOT record port `:port`
                let node_ref = strip_dot_port(h.trim());
                let nid = sanitize_id_key(node_ref.trim_matches('"'));
                if !nid.is_empty() {
                    g.upsert_node(&nid, None, Some(*default_shape), container_id);
                    hop_ids.push(nid);
                }
            }
            for pair in hop_ids.windows(2) {
                g.edges.push(MarkupEdge {
                    from_id: pair[0].clone(),
                    to_id: pair[1].clone(),
                    label: unescape_diagram_text(&lbl),
                    style: edge_style,
                    begin_arrow: beg_a,
                    end_arrow: end_a,
                });
            }
            continue;
        }

        // Standalone node declaration: `node_id [label="...", shape=..., fillcolor="#..."]`
        let raw_node = strip_dot_port(head_expr.trim());
        let clean_unquoted = raw_node.trim_matches('"').trim();
        if clean_unquoted.is_empty() || clean_unquoted.contains('=') {
            continue;
        }
        let nid = sanitize_id_key(clean_unquoted);
        let lbl = attr_map
            .get("label")
            .map(|s| clean_dot_record_label(s))
            .unwrap_or_else(|| clean_unquoted.to_string());
        let sh = attr_map
            .get("shape")
            .and_then(|s| dot_shape_to_markup(s, attr_map.get("style")))
            .or_else(|| {
                if attr_map
                    .get("style")
                    .is_some_and(|st| st.to_ascii_lowercase().contains("rounded"))
                {
                    Some(MarkupNodeShape::RoundRect)
                } else {
                    Some(*default_shape)
                }
            });
        g.upsert_node(&nid, Some(&lbl), sh, container_id);
        if let Some(node) = g.nodes.get_mut(&nid) {
            if let Some(fc) = attr_map.get("fillcolor").and_then(|c| parse_hex_rgb(c)) {
                node.fill_rgb = Some(fc);
            }
            if let Some(sc) = attr_map.get("color").and_then(|c| parse_hex_rgb(c)) {
                node.stroke_rgb = Some(sc);
            }
        }
    }
}

fn dot_shape_to_markup(shape: &str, style: Option<&String>) -> Option<MarkupNodeShape> {
    let s = shape.trim().to_ascii_lowercase();
    let st = style.map(|v| v.to_ascii_lowercase()).unwrap_or_default();
    if s.contains("cylinder") {
        Some(MarkupNodeShape::Cylinder)
    } else if s.contains("diamond") {
        Some(MarkupNodeShape::Diamond)
    } else if s.contains("hexagon") {
        Some(MarkupNodeShape::Hexagon)
    } else if s.contains("parallelogram") {
        Some(MarkupNodeShape::Parallelogram)
    } else if s.contains("trapezium") || s.contains("trapezoid") {
        Some(MarkupNodeShape::Trapezoid)
    } else if s.contains("component") || s.contains("box3d") {
        Some(MarkupNodeShape::Subprocess)
    } else if s.contains("ellipse") || s.contains("circle") || s.contains("oval") {
        Some(MarkupNodeShape::Circle)
    } else if s.contains("note") || s.contains("tab") || s.contains("folder") {
        Some(MarkupNodeShape::Note)
    } else if s.contains("mrecord") {
        Some(MarkupNodeShape::RoundRect)
    } else if s.contains("record") {
        Some(MarkupNodeShape::ClassBox)
    } else if st.contains("rounded") {
        Some(MarkupNodeShape::RoundRect)
    } else {
        Some(MarkupNodeShape::Rect)
    }
}

// ============================================================================
// 4. Deterministic Layered Graph Layout & Visio ShapeSheet Materialization
// ============================================================================

fn materialize_markup_graph(
    doc: &mut VisioDocument,
    page_id: u32,
    graph: &MarkupGraph,
    resolved: DiagramMarkupFormat,
) -> Result<MarkupImportStats, String> {
    let n_count = graph.nodes.len();
    let mut node_keys: Vec<String> = graph.nodes.keys().cloned().collect();
    node_keys.sort_by_key(|k| graph.nodes[k].order_idx);

    // 1. Compute deterministic longest-path DAG ranks (bounded relaxation handles cycles cleanly!)
    let mut rank_map: BTreeMap<String, usize> =
        node_keys.iter().map(|k| (k.clone(), 0usize)).collect();
    for _ in 0..n_count.min(64) {
        let mut changed = false;
        for edge in &graph.edges {
            if edge.from_id == edge.to_id {
                continue;
            }
            let r_from = *rank_map.get(&edge.from_id).unwrap_or(&0);
            let r_to = *rank_map.get(&edge.to_id).unwrap_or(&0);
            let from_ord = graph
                .nodes
                .get(&edge.from_id)
                .map(|n| n.order_idx)
                .unwrap_or(0);
            let to_ord = graph
                .nodes
                .get(&edge.to_id)
                .map(|n| n.order_idx)
                .unwrap_or(0);
            // Break cycles deterministically using declaration order
            if (from_ord < to_ord || r_from == 0) && r_to < r_from + 1 && r_from + 1 < n_count {
                rank_map.insert(edge.to_id.clone(), r_from + 1);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // If zero edges exist, wrap into a tidy grid of up to 4 columns!
    if graph.edges.is_empty() && n_count > 4 {
        for (idx, k) in node_keys.iter().enumerate() {
            rank_map.insert(k.clone(), idx / 4);
        }
    }

    let max_rank = rank_map.values().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for k in &node_keys {
        let r = rank_map[k];
        layers[r].push(k.clone());
    }
    // Sort within each rank by `(container_id, order_idx)` so subgraph members stay contiguous!
    for layer in &mut layers {
        layer.sort_by(|a, b| {
            let na = &graph.nodes[a];
            let nb = &graph.nodes[b];
            na.container_id
                .cmp(&nb.container_id)
                .then_with(|| na.order_idx.cmp(&nb.order_idx))
        });
    }

    // 2. Size each node cleanly from its text lines & shape kind
    let mut node_sizes: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for (k, node) in &graph.nodes {
        let lines: Vec<&str> = node.label.lines().collect();
        let line_cnt = lines.len().max(1) as f64;
        let max_chars = lines
            .iter()
            .map(|l| l.chars().count())
            .max()
            .unwrap_or(8) as f64;
        let base_w = (0.75 + max_chars * 0.082).clamp(1.35, 3.20);
        let base_h = (0.55 + (line_cnt - 1.0) * 0.24).clamp(0.75, 2.80);
        let (w, h) = match node.shape {
            MarkupNodeShape::Circle => {
                let d = base_w.min(1.35).max(0.85);
                (d, d)
            }
            MarkupNodeShape::Diamond => (base_w.max(1.55), base_h.max(1.0)),
            MarkupNodeShape::Cylinder => (base_w.max(1.30), base_h.max(1.15)),
            MarkupNodeShape::Stadium => (base_w, base_h.clamp(0.65, 1.20)),
            MarkupNodeShape::ClassBox | MarkupNodeShape::EntityBox => {
                (base_w.max(1.95), base_h.max(0.95))
            }
            _ => (base_w, base_h),
        };
        node_sizes.insert(k.clone(), (w, h));
    }

    // 3. Compute coordinates in inches (auto-growing page dimensions if needed!)
    let rank_gap = 2.65;
    let sibling_gap = 2.55;
    let is_horizontal = matches!(graph.direction, LayoutDir::LeftRight | LayoutDir::RightLeft);
    let max_layer_len = layers.iter().map(Vec::len).max().unwrap_or(1);

    let req_w = if is_horizontal {
        ((max_rank + 1) as f64 * rank_gap + 2.4).max(11.0)
    } else {
        (max_layer_len as f64 * sibling_gap + 2.4).max(11.0)
    };
    let req_h = if is_horizontal {
        (max_layer_len as f64 * 1.85 + 2.2).max(8.5)
    } else {
        ((max_rank + 1) as f64 * 2.15 + 2.2).max(8.5)
    };

    if let Some(page) = doc.page_mut(page_id) {
        if req_w > page.width_in() {
            page.page_sheet.set_formula(
                "PageWidth",
                format!("{} in", format_clean_f64(req_w)),
                Some(Unit::Inch),
            );
        }
        if req_h > page.height_in() {
            page.page_sheet.set_formula(
                "PageHeight",
                format!("{} in", format_clean_f64(req_h)),
                Some(Unit::Inch),
            );
        }
    }

    let page_w = doc.page(page_id).map(|p| p.width_in()).unwrap_or(req_w);
    let page_h = doc.page(page_id).map(|p| p.height_in()).unwrap_or(req_h);

    let mut node_shape_ids: BTreeMap<String, u64> = BTreeMap::new();
    for (r_idx, layer) in layers.iter().enumerate() {
        let count = layer.len();
        for (pos_idx, key) in layer.iter().enumerate() {
            let node = &graph.nodes[key];
            let (w, h) = node_sizes[key];
            let (pin_x, pin_y) = match graph.direction {
                LayoutDir::TopDown => {
                    let total_span = (count.saturating_sub(1)) as f64 * sibling_gap;
                    let start_x = (page_w * 0.5 - total_span * 0.5).max(1.4);
                    let x = start_x + pos_idx as f64 * sibling_gap;
                    let y = (page_h - 1.35) - r_idx as f64 * 2.05;
                    (x, y.max(1.0))
                }
                LayoutDir::BottomUp => {
                    let total_span = (count.saturating_sub(1)) as f64 * sibling_gap;
                    let start_x = (page_w * 0.5 - total_span * 0.5).max(1.4);
                    let x = start_x + pos_idx as f64 * sibling_gap;
                    let y = 1.35 + r_idx as f64 * 2.05;
                    (x, y)
                }
                LayoutDir::LeftRight => {
                    let total_span = (count.saturating_sub(1)) as f64 * 1.85;
                    let start_y = (page_h * 0.5 + total_span * 0.5).min(page_h - 1.2);
                    let x = 1.55 + r_idx as f64 * rank_gap;
                    let y = (start_y - pos_idx as f64 * 1.85).max(1.0);
                    (x, y)
                }
                LayoutDir::RightLeft => {
                    let total_span = (count.saturating_sub(1)) as f64 * 1.85;
                    let start_y = (page_h * 0.5 + total_span * 0.5).min(page_h - 1.2);
                    let x = (page_w - 1.55 - r_idx as f64 * rank_gap).max(1.2);
                    let y = (start_y - pos_idx as f64 * 1.85).max(1.0);
                    (x, y)
                }
            };

            let sid = doc
                .page(page_id)
                .map(|p| p.next_shape_id())
                .unwrap_or(1);
            let sh = build_visio_node_shape(sid, node, pin_x, pin_y, w, h);
            if let Some(page) = doc.page_mut(page_id) {
                page.shapes.push(sh);
            }
            node_shape_ids.insert(key.clone(), sid);
        }
    }

    // 4. Materialize containers (`subgraph` / `package` / `cluster_*`) around their member shapes!
    let mut containers_created = 0usize;
    for cont in &graph.containers {
        let member_sids: Vec<u64> = cont
            .member_node_ids
            .iter()
            .filter_map(|nid| node_shape_ids.get(nid).copied())
            .collect();
        if !member_sids.is_empty() {
            if create_container(doc, page_id, &cont.title, &member_sids, 0.35).is_some() {
                containers_created += 1;
            }
        }
    }

    // 5. Materialize glued 1D connectors and route around obstacles!
    let mut connectors_created = 0usize;
    if let Some(page) = doc.page_mut(page_id) {
        for edge in &graph.edges {
            let Some(&from_sid) = node_shape_ids.get(&edge.from_id) else {
                continue;
            };
            let Some(&to_sid) = node_shape_ids.get(&edge.to_id) else {
                continue;
            };
            let cid = page.next_shape_id();
            let mut conn = VisioShape::new_connector(
                cid,
                format!("Connector.{cid}"),
                1.0,
                1.0,
                2.0,
                2.0,
            );
            conn.text = edge.label.clone();
            conn.sheet
                .set_formula("ShapeRouteStyle", "16", Some(Unit::Number));
            conn.sheet.set_formula(
                "BeginArrow",
                format!("{}", edge.begin_arrow),
                Some(Unit::Number),
            );
            conn.sheet.set_formula(
                "EndArrow",
                format!("{}", edge.end_arrow),
                Some(Unit::Number),
            );
            match edge.style {
                MarkupEdgeStyle::Solid => {
                    conn.sheet
                        .set_formula("LinePattern", "1", Some(Unit::Number));
                    conn.sheet
                        .set_formula("LineWeight", "1.35 pt", Some(Unit::Point));
                }
                MarkupEdgeStyle::Dashed => {
                    conn.sheet
                        .set_formula("LinePattern", "2", Some(Unit::Number));
                    conn.sheet
                        .set_formula("LineWeight", "1.35 pt", Some(Unit::Point));
                }
                MarkupEdgeStyle::Dotted => {
                    conn.sheet
                        .set_formula("LinePattern", "3", Some(Unit::Number));
                    conn.sheet
                        .set_formula("LineWeight", "1.35 pt", Some(Unit::Point));
                }
                MarkupEdgeStyle::Thick => {
                    conn.sheet
                        .set_formula("LinePattern", "1", Some(Unit::Number));
                    conn.sheet
                        .set_formula("LineWeight", "2.4 pt", Some(Unit::Point));
                }
            }
            conn.sheet
                .set_formula("LineColor", "=RGB(51,65,85)", Some(Unit::Color));
            page.shapes.push(conn);
            page.connects.push(GlueEndpoint {
                connector_shape_id: cid,
                from_cell: "BeginX".into(),
                target_shape_id: from_sid,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
            page.connects.push(GlueEndpoint {
                connector_shape_id: cid,
                from_cell: "EndX".into(),
                target_shape_id: to_sid,
                to_cell: "PinX".into(),
                is_dynamic: true,
            });
            connectors_created += 1;
        }
    }

    route_page_connectors(doc, page_id);
    ReactiveEngine::recalculate_document(doc);

    Ok(MarkupImportStats {
        format: resolved.slug().to_string(),
        nodes_created: node_shape_ids.len(),
        containers_created,
        connectors_created,
    })
}

fn build_visio_node_shape(
    sid: u64,
    node: &MarkupNode,
    pin_x: f64,
    pin_y: f64,
    w: f64,
    h: f64,
) -> VisioShape {
    let mut sh = VisioShape::new_rect(sid, format!("{}.{sid}", node.id), pin_x, pin_y, w, h);
    sh.text = node.label.clone();
    sh.sheet.set_user_cell(
        "MarkupNodeId",
        format!("\"{}\"", escape_quotes(&node.id)),
        "Diagram-as-Code Node Identifier",
    );
    sh.sheet.set_user_cell(
        "MarkupNodeShape",
        format!("\"{}\"", node.shape.tag()),
        "Diagram-as-Code Node Shape",
    );

    let (default_fill, default_stroke) = match node.shape {
        MarkupNodeShape::Diamond => ((254, 243, 199), (217, 119, 6)),
        MarkupNodeShape::Stadium => ((220, 252, 231), (22, 163, 74)),
        MarkupNodeShape::Cylinder | MarkupNodeShape::EntityBox => ((224, 242, 254), (2, 132, 199)),
        MarkupNodeShape::Note => ((254, 249, 195), (202, 138, 4)),
        MarkupNodeShape::Hexagon | MarkupNodeShape::Subprocess => ((243, 232, 255), (126, 34, 206)),
        _ => ((239, 246, 255), (37, 99, 235)),
    };
    let (fr, fg, fb) = node.fill_rgb.unwrap_or(default_fill);
    let (sr, sg, sb) = node.stroke_rgb.unwrap_or(default_stroke);
    sh.sheet.set_formula(
        "FillForegnd",
        format!("=RGB({fr},{fg},{fb})"),
        Some(Unit::Color),
    );
    sh.sheet.set_formula(
        "LineColor",
        format!("=RGB({sr},{sg},{sb})"),
        Some(Unit::Color),
    );
    sh.sheet
        .set_formula("LineWeight", "1.4 pt", Some(Unit::Point));
    sh.sheet
        .set_formula("Char.Color", "=RGB(15,23,42)", Some(Unit::Color));
    sh.sheet
        .set_formula("Char.Size", "10 pt", Some(Unit::Point));

    match node.shape {
        MarkupNodeShape::RoundRect => {
            sh.sheet.add_control(ControlRow {
                name: "Row_1".into(),
                x: "=Width*0.16".into(),
                y: "=Height".into(),
                x_dyn: "=Controls.Row_1.X".into(),
                y_dyn: "=Controls.Row_1.Y".into(),
                x_con: "0".into(),
                y_con: "0".into(),
                can_glue: false,
                prompt: "Adjust Corner Rounding".into(),
            });
            sh.sheet.set_formula(
                "Rounding",
                "=BOUND(Controls.Row_1.X, 0, FALSE, 0.02 in, MIN(Width,Height)*0.48)",
                Some(Unit::Inch),
            );
        }
        MarkupNodeShape::Stadium => {
            sh.sheet.add_control(ControlRow {
                name: "Row_1".into(),
                x: "=MIN(Width,Height)*0.5".into(),
                y: "=Height".into(),
                x_dyn: "=Controls.Row_1.X".into(),
                y_dyn: "=Controls.Row_1.Y".into(),
                x_con: "0".into(),
                y_con: "0".into(),
                can_glue: false,
                prompt: "Adjust Pill Rounding".into(),
            });
            sh.sheet.set_formula(
                "Rounding",
                "=BOUND(Controls.Row_1.X, 0, FALSE, 0 in, MIN(Width,Height)*0.5)",
                Some(Unit::Inch),
            );
        }
        MarkupNodeShape::Subprocess => {
            let mut bars = GeometrySection::new(2);
            bars.no_fill = "TRUE".into();
            bars.rows = vec![
                GeometryRow::move_to(1, "=Width*0.10", "0 in"),
                GeometryRow::line_to(2, "=Width*0.10", "=Height"),
                GeometryRow::move_to(3, "=Width*0.90", "0 in"),
                GeometryRow::line_to(4, "=Width*0.90", "=Height"),
            ];
            sh.sheet.add_geometry(bars);
        }
        MarkupNodeShape::Cylinder => {
            sh.sheet.add_control(ControlRow {
                name: "Row_1".into(),
                x: "=Width*0.5".into(),
                y: "=Height*0.84".into(),
                x_dyn: "=Controls.Row_1.X".into(),
                y_dyn: "=Controls.Row_1.Y".into(),
                x_con: "0".into(),
                y_con: "0".into(),
                can_glue: false,
                prompt: "Adjust Cylinder Cap Depth".into(),
            });
            let cap_h = "BOUND(Height - Controls.Row_1.Y, 0, FALSE, Height*0.06, Height*0.42)";
            let mut g1 = GeometrySection::new(1);
            g1.rows = vec![
                GeometryRow::move_to(1, "0 in", format!("={cap_h}")),
                GeometryRow::arc_to(2, "=Width", format!("={cap_h}"), format!("=-({cap_h})")),
                GeometryRow::line_to(3, "=Width", format!("=Height - ({cap_h})")),
                GeometryRow::arc_to(
                    4,
                    "0 in",
                    format!("=Height - ({cap_h})"),
                    format!("=-({cap_h})"),
                ),
                GeometryRow::line_to(5, "0 in", format!("={cap_h}")),
            ];
            sh.sheet.add_geometry(g1);
            let mut g2 = GeometrySection::new(2);
            g2.no_fill = "TRUE".into();
            g2.rows = vec![
                GeometryRow::move_to(1, "0 in", format!("=Height - ({cap_h})")),
                GeometryRow::arc_to(
                    2,
                    "=Width",
                    format!("=Height - ({cap_h})"),
                    format!("=-({cap_h})"),
                ),
            ];
            sh.sheet.add_geometry(g2);
        }
        MarkupNodeShape::Circle | MarkupNodeShape::Actor => {
            let mut g = GeometrySection::new(1);
            g.rows = vec![GeometryRow {
                ix: 1,
                kind: GeometryRowKind::Ellipse,
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
        MarkupNodeShape::Diamond => {
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
        MarkupNodeShape::Hexagon => {
            let mut g = GeometrySection::new(1);
            g.rows = vec![
                GeometryRow::move_to(1, "=Width*0.20", "0 in"),
                GeometryRow::line_to(2, "=Width*0.80", "0 in"),
                GeometryRow::line_to(3, "=Width", "=Height*0.5"),
                GeometryRow::line_to(4, "=Width*0.80", "=Height"),
                GeometryRow::line_to(5, "=Width*0.20", "=Height"),
                GeometryRow::line_to(6, "0 in", "=Height*0.5"),
                GeometryRow::line_to(7, "=Width*0.20", "0 in"),
            ];
            sh.sheet.add_geometry(g);
        }
        MarkupNodeShape::Parallelogram => {
            sh.sheet.add_control(ControlRow {
                name: "Row_1".into(),
                x: "=Width*0.16".into(),
                y: "0 in".into(),
                x_dyn: "=Controls.Row_1.X".into(),
                y_dyn: "=Controls.Row_1.Y".into(),
                x_con: "0".into(),
                y_con: "0".into(),
                can_glue: false,
                prompt: "Adjust Parallelogram Skew".into(),
            });
            let sk = "BOUND(Controls.Row_1.X, 0, FALSE, 0 in, Width*0.45)";
            let mut g = GeometrySection::new(1);
            g.rows = vec![
                GeometryRow::move_to(1, format!("={sk}"), "0 in"),
                GeometryRow::line_to(2, "=Width", "0 in"),
                GeometryRow::line_to(3, format!("=Width - ({sk})"), "=Height"),
                GeometryRow::line_to(4, "0 in", "=Height"),
                GeometryRow::line_to(5, format!("={sk}"), "0 in"),
            ];
            sh.sheet.add_geometry(g);
        }
        MarkupNodeShape::Trapezoid => {
            let mut g = GeometrySection::new(1);
            g.rows = vec![
                GeometryRow::move_to(1, "0 in", "0 in"),
                GeometryRow::line_to(2, "=Width", "0 in"),
                GeometryRow::line_to(3, "=Width*0.82", "=Height"),
                GeometryRow::line_to(4, "=Width*0.18", "=Height"),
                GeometryRow::line_to(5, "0 in", "0 in"),
            ];
            sh.sheet.add_geometry(g);
        }
        MarkupNodeShape::Note => {
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
        MarkupNodeShape::Rect | MarkupNodeShape::ClassBox | MarkupNodeShape::EntityBox => {}
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
    sh.sheet.sync_all_sections_to_cells();
    sh
}

// ============================================================================
// 5. Lexical & String Helpers
// ============================================================================

fn sanitize_id_key(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim();
    let mut out = String::with_capacity(trimmed.len());
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' {
            out.push(ch);
        } else if ch.is_whitespace() && !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn sanitize_export_ident(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else if ch == '-' || ch == '.' || ch == ' ' {
            out.push('_');
        }
    }
    let clean = out.trim_matches('_').to_string();
    if clean.is_empty() {
        String::new()
    } else if clean.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("N_{clean}")
    } else {
        clean
    }
}

fn unescape_diagram_text(s: &str) -> String {
    s.replace("<br/>", "\n")
        .replace("<br>", "\n")
        .replace("<br />", "\n")
        .replace("\\n", "\n")
        .replace("\\l", "\n")
        .replace("&quot;", "\"")
        .trim()
        .to_string()
}

fn escape_quotes(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn split_as_alias(rest: &str, plantuml_order: bool) -> (String, String) {
    // In PlantUML: `database "Orders DB" as db1` (label first, alias second)
    // In Mermaid sequenceDiagram: `participant db1 as Orders DB` (id first, label second)
    // Note: Mermaid stateDiagram has `state "Long Label" as s1` (quoted label first, id second)!
    let trimmed = rest.trim();
    if let Some(pos) = find_case_insensitive_as(trimmed) {
        let left = trimmed[..pos].trim();
        let right = trimmed[pos + 4..].trim();
        let left_quoted = left.starts_with('"') && left.ends_with('"');
        if plantuml_order || left_quoted {
            let id = sanitize_id_key(right);
            let lbl = left.trim_matches('"').trim_matches('[').trim_matches(']').to_string();
            return (id, lbl);
        } else {
            let id = sanitize_id_key(left);
            let lbl = right.trim_matches('"').to_string();
            return (id, lbl);
        }
    }
    let clean = trimmed
        .trim_matches('"')
        .trim_matches('[')
        .trim_matches(']')
        .trim_matches('(')
        .trim_matches(')')
        .trim();
    (sanitize_id_key(clean), clean.to_string())
}

fn find_case_insensitive_as(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut in_quotes = false;
    let mut i = 0usize;
    while i + 4 <= bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && s[i..].len() >= 4 {
            let chunk = &bytes[i..i + 4];
            if chunk[0].is_ascii_whitespace()
                && chunk[1].eq_ignore_ascii_case(&b'a')
                && chunk[2].eq_ignore_ascii_case(&b's')
                && chunk[3].is_ascii_whitespace()
            {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn split_semicolons_outside_quotes(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_quotes = false;
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ';' && !in_quotes {
            out.push(&s[start..i]);
            start = i + 1;
        }
    }
    out.push(&s[start..]);
    out
}

fn split_ampersands_outside_brackets(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_quotes = false;
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '[' | '(' | '{' if !in_quotes => depth += 1,
            ']' | ')' | '}' if !in_quotes => depth = (depth - 1).max(0),
            '&' if !in_quotes && depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

fn split_colon_outside_brackets_and_quotes(s: &str) -> (&str, &str) {
    let bytes = s.as_bytes();
    let mut in_quotes = false;
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            match b {
                b'[' | b'(' | b'{' => depth += 1,
                b']' | b')' | b'}' => depth = (depth - 1).max(0),
                b':' if depth == 0 => {
                    // Skip `:Actor:` PlantUML actor syntax (`:User: -> :Admin:`)
                    let rest = &s[i + 1..];
                    let next_colon = rest.find(':');
                    let next_space = rest.find(char::is_whitespace);
                    let is_actor_token = match (next_colon, next_space) {
                        (Some(c), Some(sp)) => c < sp,
                        (Some(_), None) => true,
                        _ => false,
                    };
                    if is_actor_token {
                        if let Some(c) = next_colon {
                            i += 1 + c + 1;
                            continue;
                        }
                    }
                    return (&s[..i], &s[i + 1..]);
                }
                _ => {}
            }
        }
        i += 1;
    }
    (s, "")
}

fn split_outside_brackets_and_quotes<'a>(s: &'a str, pat: &str) -> Option<(&'a str, &'a str)> {
    let bytes = s.as_bytes();
    let mut in_quotes = false;
    let mut depth = 0i32;
    let mut i = 0usize;
    while i + pat.len() <= bytes.len() {
        let b = bytes[i];
        if b == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes {
            match b {
                b'[' | b'(' | b'{' => depth += 1,
                b']' | b')' | b'}' => depth = (depth - 1).max(0),
                _ if depth == 0 && s[i..].starts_with(pat) => {
                    return Some((&s[..i], &s[i + pat.len()..]));
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}

fn strip_c_and_hash_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut in_quotes = false;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '"' && (i == 0 || chars[i - 1] != '\\') {
            in_quotes = !in_quotes;
            out.push(ch);
            i += 1;
            continue;
        }
        if !in_quotes {
            if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
                i += 2;
                while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                    i += 1;
                }
                i = (i + 2).min(chars.len());
                continue;
            }
            if ch == '#' && (i == 0 || chars[i - 1] == '\n') {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
        }
        out.push(ch);
        i += 1;
    }
    out
}

fn split_dot_statements(block: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_quotes = false;
    let mut brace_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut start = 0usize;
    let bytes = block.as_bytes();
    for (i, ch) in block.char_indices() {
        if ch == '"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_quotes = !in_quotes;
            continue;
        }
        if !in_quotes {
            match ch {
                '{' => brace_depth += 1,
                '}' => {
                    brace_depth = (brace_depth - 1).max(0);
                    if brace_depth == 0 && bracket_depth == 0 {
                        out.push(&block[start..=i]);
                        start = i + 1;
                    }
                }
                '[' => bracket_depth += 1,
                ']' => bracket_depth = (bracket_depth - 1).max(0),
                ';' | '\n' if brace_depth == 0 && bracket_depth == 0 => {
                    out.push(&block[start..i]);
                    start = i + 1;
                }
                _ => {}
            }
        }
    }
    if start < block.len() {
        out.push(&block[start..]);
    }
    out
}

fn extract_dot_top_label(inner: &str) -> Option<String> {
    for stmt in split_dot_statements(inner) {
        let t = stmt.trim();
        if !t.contains('{') && !t.contains("->") && !t.contains('[') {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case("label") {
                    return Some(v.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

fn split_dot_trailing_attrs(stmt: &str) -> (&str, BTreeMap<String, String>) {
    let mut attrs = BTreeMap::new();
    let mut in_quotes = false;
    let mut open_bracket = None;
    let bytes = stmt.as_bytes();
    for (i, ch) in stmt.char_indices() {
        if ch == '"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_quotes = !in_quotes;
        } else if !in_quotes && ch == '[' && open_bracket.is_none() {
            open_bracket = Some(i);
        }
    }
    let Some(ob) = open_bracket else {
        return (stmt, attrs);
    };
    let head = stmt[..ob].trim();
    let tail = stmt[ob..].trim();
    let inner = tail
        .strip_prefix('[')
        .unwrap_or(tail)
        .strip_suffix(']')
        .unwrap_or(tail);

    let mut cur_key = String::new();
    let mut cur_val = String::new();
    let mut reading_val = false;
    let mut q = false;
    let chars: Vec<char> = inner.chars().collect();
    let mut idx = 0usize;
    while idx < chars.len() {
        let ch = chars[idx];
        if ch == '"' && (idx == 0 || chars[idx - 1] != '\\') {
            q = !q;
            idx += 1;
            continue;
        }
        if !q {
            if ch == '=' && !reading_val {
                reading_val = true;
                idx += 1;
                continue;
            }
            if ch == ',' || ch == ';' {
                let k = cur_key.trim().to_ascii_lowercase();
                if !k.is_empty() {
                    attrs.insert(k, cur_val.trim().to_string());
                }
                cur_key.clear();
                cur_val.clear();
                reading_val = false;
                idx += 1;
                continue;
            }
        }
        if reading_val {
            cur_val.push(ch);
        } else {
            cur_key.push(ch);
        }
        idx += 1;
    }
    let k = cur_key.trim().to_ascii_lowercase();
    if !k.is_empty() {
        attrs.insert(k, cur_val.trim().to_string());
    }
    (head, attrs)
}

fn split_outside_quotes_by_op<'a>(s: &'a str, op: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut in_quotes = false;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'"' && (i == 0 || bytes[i - 1] != b'\\') {
            in_quotes = !in_quotes;
            i += 1;
            continue;
        }
        if !in_quotes && s[i..].starts_with(op) {
            out.push(&s[start..i]);
            i += op.len();
            start = i;
            continue;
        }
        i += 1;
    }
    out.push(&s[start..]);
    out
}

fn strip_dot_port(s: &str) -> &str {
    let mut in_quotes = false;
    for (i, ch) in s.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if ch == ':' && !in_quotes {
            return &s[..i];
        }
    }
    s
}

fn clean_dot_record_label(raw: &str) -> String {
    let s = raw.trim().trim_start_matches('{').trim_end_matches('}');
    let mut cleaned = String::with_capacity(s.len());
    let mut in_port = false;
    for ch in s.chars() {
        match ch {
            '<' => in_port = true,
            '>' if in_port => in_port = false,
            '|' if !in_port => cleaned.push('\n'),
            '{' | '}' if !in_port => {}
            _ if !in_port => cleaned.push(ch),
            _ => {}
        }
    }
    unescape_diagram_text(&cleaned)
}

fn parse_css_like_colors(attrs: &str) -> (Option<(u8, u8, u8)>, Option<(u8, u8, u8)>) {
    let mut fill = None;
    let mut stroke = None;
    for part in attrs.split(',') {
        if let Some((k, v)) = part.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            if key == "fill" {
                fill = parse_hex_rgb(v.trim());
            } else if key == "stroke" {
                stroke = parse_hex_rgb(v.trim());
            }
        }
    }
    (fill, stroke)
}

fn parse_hex_rgb(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_matches('"').trim_start_matches('#');
    if h.len() == 6 {
        let r = u8::from_str_radix(&h[0..2], 16).ok()?;
        let g = u8::from_str_radix(&h[2..4], 16).ok()?;
        let b = u8::from_str_radix(&h[4..6], 16).ok()?;
        Some((r, g, b))
    } else if h.len() == 3 {
        let r = u8::from_str_radix(&h[0..1], 16).ok()? * 17;
        let g = u8::from_str_radix(&h[1..2], 16).ok()? * 17;
        let b = u8::from_str_radix(&h[2..3], 16).ok()? * 17;
        Some((r, g, b))
    } else {
        None
    }
}
