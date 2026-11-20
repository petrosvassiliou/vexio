//! Enterprise Auto-Layout Engine: Hierarchical (Sugiyama), Radial, Compact Tree,
//! Circular, and Flowchart Reflow.

use crate::routing::route_page_connectors;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use vexio_shapesheet::{ReactiveEngine, ShapeKind, VisioDocument, format_clean_f64};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayoutAlgorithm {
    HierarchicalTopToBottom,
    HierarchicalBottomToTop,
    HierarchicalLeftToRight,
    HierarchicalRightToLeft,
    Radial,
    CompactTree,
    Circular,
    Flowchart,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LayoutOptions {
    pub algorithm: LayoutAlgorithm,
    pub layer_spacing_in: f64,
    pub node_spacing_in: f64,
    pub margin_x_in: f64,
    pub margin_y_in: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            algorithm: LayoutAlgorithm::HierarchicalTopToBottom,
            layer_spacing_in: 1.8,
            node_spacing_in: 2.4,
            margin_x_in: 1.5,
            margin_y_in: 1.5,
        }
    }
}

pub fn apply_auto_layout(doc: &mut VisioDocument, page_id: u32, opts: LayoutOptions) {
    ReactiveEngine::recalculate_document(doc);
    let Some(page) = doc.page(page_id) else {
        return;
    };

    // Collect movable 2D shapes (excluding containers/swimlanes) and directed edges from `Connects`
    let nodes: Vec<u64> = page
        .shapes
        .iter()
        .filter(|s| {
            s.kind == ShapeKind::Shape2D
                && s.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) < 0.5
                && s.sheet
                    .get_cell("User.msvStructureType")
                    .and_then(|c| c.value.as_str_coerced().ok())
                    .map(|v| v != "Container")
                    .unwrap_or(true)
        })
        .map(|s| s.id)
        .collect();

    if nodes.is_empty() {
        return;
    }

    let node_set: BTreeSet<u64> = nodes.iter().copied().collect();
    let mut conn_src: BTreeMap<u64, u64> = BTreeMap::new();
    let mut conn_dst: BTreeMap<u64, u64> = BTreeMap::new();
    for c in &page.connects {
        if !node_set.contains(&c.target_shape_id) {
            continue;
        }
        if c.from_cell.to_ascii_lowercase().starts_with("begin") {
            conn_src.insert(c.connector_shape_id, c.target_shape_id);
        } else {
            conn_dst.insert(c.connector_shape_id, c.target_shape_id);
        }
    }

    let mut adj: BTreeMap<u64, Vec<u64>> = nodes.iter().map(|&id| (id, Vec::new())).collect();
    let mut indeg: BTreeMap<u64, usize> = nodes.iter().map(|&id| (id, 0)).collect();
    for (cid, &u) in &conn_src {
        if let Some(&v) = conn_dst.get(cid) {
            if u != v {
                adj.entry(u).or_default().push(v);
                *indeg.entry(v).or_insert(0) += 1;
            }
        }
    }

    let page_h = page.height_in();
    let page_w = page.width_in();

    let positions: BTreeMap<u64, [f64; 2]> = match opts.algorithm {
        LayoutAlgorithm::Circular => compute_circular(&nodes, page_w * 0.5, page_h * 0.5, opts),
        LayoutAlgorithm::Radial => compute_radial(&nodes, &adj, &indeg, page_w * 0.5, page_h * 0.5, opts),
        LayoutAlgorithm::CompactTree => compute_compact_tree(&nodes, &adj, &indeg, page_h, opts),
        LayoutAlgorithm::Flowchart
        | LayoutAlgorithm::HierarchicalTopToBottom
        | LayoutAlgorithm::HierarchicalBottomToTop
        | LayoutAlgorithm::HierarchicalLeftToRight
        | LayoutAlgorithm::HierarchicalRightToLeft => {
            compute_sugiyama(&nodes, &adj, &indeg, page_w, page_h, opts)
        }
    };

    if let Some(page_mut) = doc.page_mut(page_id) {
        for (sid, [x, y]) in positions {
            if let Some(sh) = page_mut.find_shape_mut(sid) {
                let _ = sh.sheet.try_set_formula_respecting_guard(
                    "PinX",
                    format!("{} in", format_clean_f64(x)),
                    false,
                );
                let _ = sh.sheet.try_set_formula_respecting_guard(
                    "PinY",
                    format!("{} in", format_clean_f64(y)),
                    false,
                );
            }
        }
    }

    route_page_connectors(doc, page_id);
}

fn compute_sugiyama(
    nodes: &[u64],
    adj: &BTreeMap<u64, Vec<u64>>,
    indeg: &BTreeMap<u64, usize>,
    page_w: f64,
    page_h: f64,
    opts: LayoutOptions,
) -> BTreeMap<u64, [f64; 2]> {
    // 1. Cycle-safe rank assignment via Kahn BFS
    let mut rem_in = indeg.clone();
    let mut rank: BTreeMap<u64, usize> = BTreeMap::new();
    let mut q: VecDeque<u64> = nodes
        .iter()
        .copied()
        .filter(|id| *rem_in.get(id).unwrap_or(&0) == 0)
        .collect();
    if q.is_empty() {
        q.push_back(nodes[0]);
    }
    let mut visited = BTreeSet::new();
    while let Some(u) = q.pop_front() {
        if !visited.insert(u) {
            continue;
        }
        let ru = *rank.get(&u).unwrap_or(&0);
        rank.insert(u, ru);
        if let Some(nbrs) = adj.get(&u) {
            for &v in nbrs {
                if !visited.contains(&v) {
                    let entry = rank.entry(v).or_insert(0);
                    *entry = (*entry).max(ru + 1);
                    let deg = rem_in.entry(v).or_insert(1);
                    *deg = deg.saturating_sub(1);
                    if *deg == 0 {
                        q.push_back(v);
                    }
                }
            }
        }
        if q.is_empty() && visited.len() < nodes.len() {
            if let Some(&next_unvisited) = nodes.iter().find(|id| !visited.contains(id)) {
                q.push_back(next_unvisited);
            }
        }
    }

    // 2. Group into layers & run barycentric crossing-reduction pass
    let max_rank = rank.values().copied().max().unwrap_or(0);
    let mut layers: Vec<Vec<u64>> = vec![Vec::new(); max_rank + 1];
    for &id in nodes {
        layers[*rank.get(&id).unwrap_or(&0)].push(id);
    }
    for r in 1..=max_rank {
        let prev = layers[r - 1].clone();
        let pos_in_prev: BTreeMap<u64, f64> = prev
            .iter()
            .enumerate()
            .map(|(idx, &id)| (id, idx as f64))
            .collect();
        layers[r].sort_by(|a, b| {
            let bary = |target: u64| -> f64 {
                let mut sum = 0.0;
                let mut cnt = 0usize;
                for (&u, nbrs) in adj {
                    if nbrs.contains(&target) {
                        if let Some(&p) = pos_in_prev.get(&u) {
                            sum += p;
                            cnt += 1;
                        }
                    }
                }
                if cnt == 0 {
                    target as f64
                } else {
                    sum / cnt as f64
                }
            };
            bary(*a)
                .partial_cmp(&bary(*b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // 3. Assign coordinates per orientation
    let mut out = BTreeMap::new();
    for (r, layer) in layers.iter().enumerate() {
        let count = layer.len() as f64;
        for (i, &id) in layer.iter().enumerate() {
            let cross_offset = (i as f64 - (count - 1.0) * 0.5) * opts.node_spacing_in;
            let depth = r as f64 * opts.layer_spacing_in;
            let pt = match opts.algorithm {
                LayoutAlgorithm::HierarchicalBottomToTop => {
                    [page_w * 0.5 + cross_offset, opts.margin_y_in + depth]
                }
                LayoutAlgorithm::HierarchicalLeftToRight => {
                    [opts.margin_x_in + depth, page_h * 0.5 - cross_offset]
                }
                LayoutAlgorithm::HierarchicalRightToLeft => {
                    [page_w - opts.margin_x_in - depth, page_h * 0.5 - cross_offset]
                }
                _ => [page_w * 0.5 + cross_offset, page_h - opts.margin_y_in - depth],
            };
            out.insert(id, pt);
        }
    }
    out
}

fn compute_circular(
    nodes: &[u64],
    cx: f64,
    cy: f64,
    opts: LayoutOptions,
) -> BTreeMap<u64, [f64; 2]> {
    let mut out = BTreeMap::new();
    let n = nodes.len();
    if n == 1 {
        out.insert(nodes[0], [cx, cy]);
        return out;
    }
    let radius = ((n as f64 * opts.node_spacing_in) / std::f64::consts::TAU).max(2.0);
    for (i, &id) in nodes.iter().enumerate() {
        let theta = (i as f64 / n as f64) * std::f64::consts::TAU;
        out.insert(id, [cx + radius * theta.cos(), cy + radius * theta.sin()]);
    }
    out
}

fn compute_radial(
    nodes: &[u64],
    adj: &BTreeMap<u64, Vec<u64>>,
    indeg: &BTreeMap<u64, usize>,
    cx: f64,
    cy: f64,
    opts: LayoutOptions,
) -> BTreeMap<u64, [f64; 2]> {
    let root = nodes
        .iter()
        .copied()
        .find(|id| *indeg.get(id).unwrap_or(&0) == 0)
        .unwrap_or(nodes[0]);
    let mut out = BTreeMap::new();
    out.insert(root, [cx, cy]);

    let mut depth_map: BTreeMap<u64, usize> = BTreeMap::new();
    depth_map.insert(root, 0);
    let mut q = VecDeque::from([root]);
    while let Some(u) = q.pop_front() {
        let d = depth_map[&u];
        if let Some(nbrs) = adj.get(&u) {
            for &v in nbrs {
                if !depth_map.contains_key(&v) {
                    depth_map.insert(v, d + 1);
                    q.push_back(v);
                }
            }
        }
    }
    for &id in nodes {
        depth_map.entry(id).or_insert(1);
    }

    let max_d = depth_map.values().copied().max().unwrap_or(0);
    for d in 1..=max_d {
        let ring: Vec<u64> = nodes
            .iter()
            .copied()
            .filter(|id| depth_map.get(id) == Some(&d))
            .collect();
        let r = d as f64 * opts.layer_spacing_in;
        for (i, id) in ring.into_iter().enumerate() {
            let count = nodes.iter().filter(|nid| depth_map.get(nid) == Some(&d)).count();
            let ang = (i as f64 / count.max(1) as f64) * std::f64::consts::TAU;
            out.insert(id, [cx + r * ang.cos(), cy + r * ang.sin()]);
        }
    }
    out
}

fn compute_compact_tree(
    nodes: &[u64],
    adj: &BTreeMap<u64, Vec<u64>>,
    indeg: &BTreeMap<u64, usize>,
    page_h: f64,
    opts: LayoutOptions,
) -> BTreeMap<u64, [f64; 2]> {
    let roots: Vec<u64> = nodes
        .iter()
        .copied()
        .filter(|id| *indeg.get(id).unwrap_or(&0) == 0)
        .collect();
    let roots = if roots.is_empty() {
        vec![nodes[0]]
    } else {
        roots
    };

    let mut out = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut next_leaf_x = opts.margin_x_in;

    fn place_subtree(
        u: u64,
        depth: usize,
        adj: &BTreeMap<u64, Vec<u64>>,
        visited: &mut BTreeSet<u64>,
        next_leaf_x: &mut f64,
        page_h: f64,
        opts: LayoutOptions,
        out: &mut BTreeMap<u64, [f64; 2]>,
    ) -> f64 {
        visited.insert(u);
        let y = page_h - opts.margin_y_in - (depth as f64 * opts.layer_spacing_in);
        let children: Vec<u64> = adj
            .get(&u)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|c| !visited.contains(c))
            .collect();
        if children.is_empty() {
            let x = *next_leaf_x;
            *next_leaf_x += opts.node_spacing_in;
            out.insert(u, [x, y]);
            x
        } else {
            let mut child_xs = Vec::new();
            for c in children {
                child_xs.push(place_subtree(
                    c,
                    depth + 1,
                    adj,
                    visited,
                    next_leaf_x,
                    page_h,
                    opts,
                    out,
                ));
            }
            let x = (child_xs.first().unwrap() + child_xs.last().unwrap()) * 0.5;
            out.insert(u, [x, y]);
            x
        }
    }

    for r in roots {
        if !visited.contains(&r) {
            place_subtree(
                r,
                0,
                adj,
                &mut visited,
                &mut next_leaf_x,
                page_h,
                opts,
                &mut out,
            );
        }
    }
    for &id in nodes {
        if !out.contains_key(&id) {
            out.insert(id, [next_leaf_x, page_h - opts.margin_y_in]);
            next_leaf_x += opts.node_spacing_in;
        }
    }
    out
}
