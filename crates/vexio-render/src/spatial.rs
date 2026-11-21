//! Dynamic 2D R*-tree spatial index (`rstar::RTree<SpatialEntry>`) for `O(log N)`
//! viewport frustum culling, point hit-testing, and magnetic snap/glue queries
//! across 50,000+ shapes in canonical Visio page inches.

use rstar::{Envelope, PointDistance, RTree, RTreeObject, AABB};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use vexio_shapesheet::{EvaluatedPathVerb, EvaluatedShapeVisual, ShapeKind};

/// World-space bounding-box record stored inside the R*-tree, in canonical Visio inches.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SpatialEntry {
    pub shape_id: u64,
    pub min_in: [f64; 2],
    pub max_in: [f64; 2],
    /// Deterministic painter's-order index (`0..N`) preserving shape stacking order.
    pub z_order: u32,
}

impl SpatialEntry {
    /// Construct a sanitized `SpatialEntry` ensuring finite ordered `min_in <= max_in` corners.
    pub fn new(shape_id: u64, min_in: [f64; 2], max_in: [f64; 2], z_order: u32) -> Self {
        let (x0, x1) = sanitize_interval(min_in[0], max_in[0]);
        let (y0, y1) = sanitize_interval(min_in[1], max_in[1]);
        Self {
            shape_id,
            min_in: [x0, y0],
            max_in: [x1, y1],
            z_order,
        }
    }

    /// Build a `SpatialEntry` directly from an evaluated `EvaluatedShapeVisual`,
    /// expanding by half the stroke weight so thick strokes and connectors hit-test accurately.
    pub fn from_visual(visual: &EvaluatedShapeVisual, z_order: u32) -> Self {
        let pad = (visual.stroke_weight_in.max(0.0) * 0.5).max(1e-4);
        Self::new(
            visual.shape_id,
            [visual.aabb_min[0] - pad, visual.aabb_min[1] - pad],
            [visual.aabb_max[0] + pad, visual.aabb_max[1] + pad],
            z_order,
        )
    }

    #[inline]
    pub fn center_in(&self) -> [f64; 2] {
        [
            (self.min_in[0] + self.max_in[0]) * 0.5,
            (self.min_in[1] + self.max_in[1]) * 0.5,
        ]
    }

    #[inline]
    pub fn size_in(&self) -> [f64; 2] {
        [
            (self.max_in[0] - self.min_in[0]).max(0.0),
            (self.max_in[1] - self.min_in[1]).max(0.0),
        ]
    }

    /// Standard Visio magnetic snap/connection points on this bounding box:
    /// 4 cardinal midpoints (W, E, S, N), center pin, and 4 corners, plus perimeter clamp.
    pub fn candidate_snap_points(&self, query: [f64; 2]) -> [[f64; 2]; 10] {
        let [cx, cy] = self.center_in();
        let [x0, y0] = self.min_in;
        let [x1, y1] = self.max_in;
        let clamped_x = query[0].clamp(x0, x1);
        let clamped_y = query[1].clamp(y0, y1);
        // Project interior points to nearest perimeter edge for edge-glue snapping.
        let perim = if query[0] > x0 && query[0] < x1 && query[1] > y0 && query[1] < y1 {
            let dl = (query[0] - x0).abs();
            let dr = (x1 - query[0]).abs();
            let db = (query[1] - y0).abs();
            let dt = (y1 - query[1]).abs();
            let m = dl.min(dr).min(db).min(dt);
            if (m - dl).abs() < 1e-12 {
                [x0, query[1]]
            } else if (m - dr).abs() < 1e-12 {
                [x1, query[1]]
            } else if (m - db).abs() < 1e-12 {
                [query[0], y0]
            } else {
                [query[0], y1]
            }
        } else {
            [clamped_x, clamped_y]
        };

        [
            [x0, cy], // West midpoint
            [x1, cy], // East midpoint
            [cx, y0], // South midpoint
            [cx, y1], // North midpoint
            [cx, cy], // Center pin
            [x0, y0], // SW corner
            [x1, y0], // SE corner
            [x1, y1], // NE corner
            [x0, y1], // NW corner
            perim,    // Nearest perimeter point
        ]
    }
}

impl RTreeObject for SpatialEntry {
    type Envelope = AABB<[f64; 2]>;

    #[inline]
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(self.min_in, self.max_in)
    }
}

impl PointDistance for SpatialEntry {
    #[inline]
    fn distance_2(&self, point: &[f64; 2]) -> f64 {
        self.envelope().distance_2(point)
    }

    #[inline]
    fn contains_point(&self, point: &[f64; 2]) -> bool {
        self.envelope().contains_point(point)
    }
}

/// Dynamic 2D R*-tree spatial index supporting bulk-loading (`O(N log N)`), incremental
/// `upsert`/`remove`, viewport frustum culling, hit-testing, and magnetic snap lookup.
#[derive(Debug, Clone)]
pub struct SpatialIndex {
    tree: RTree<SpatialEntry>,
    entries_by_id: BTreeMap<u64, SpatialEntry>,
    next_z_order: u32,
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl SpatialIndex {
    pub fn new() -> Self {
        Self {
            tree: RTree::new(),
            entries_by_id: BTreeMap::new(),
            next_z_order: 0,
        }
    }

    /// Bulk-load an entire page's `EvaluatedShapeVisual` slice in `O(N log N)` using STR/R*-tree packing.
    pub fn build(visuals: &[EvaluatedShapeVisual]) -> Self {
        let mut entries = Vec::with_capacity(visuals.len());
        let mut entries_by_id = BTreeMap::new();
        for (idx, vis) in visuals.iter().enumerate() {
            let entry = SpatialEntry::from_visual(vis, idx as u32);
            entries_by_id.insert(entry.shape_id, entry);
            entries.push(entry);
        }
        // If duplicate shape_ids existed, keep the tree strictly 1:1 with `entries_by_id`.
        if entries_by_id.len() != entries.len() {
            entries = entries_by_id.values().copied().collect();
        }
        let next_z_order = visuals.len() as u32;
        Self {
            tree: RTree::bulk_load(entries),
            entries_by_id,
            next_z_order,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.entries_by_id.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries_by_id.is_empty()
    }

    #[inline]
    pub fn get(&self, shape_id: u64) -> Option<&SpatialEntry> {
        self.entries_by_id.get(&shape_id)
    }

    /// Insert or update a shape's world-space bounding box in the R*-tree.
    pub fn upsert(&mut self, mut entry: SpatialEntry) {
        let (x0, x1) = sanitize_interval(entry.min_in[0], entry.max_in[0]);
        let (y0, y1) = sanitize_interval(entry.min_in[1], entry.max_in[1]);
        entry.min_in = [x0, y0];
        entry.max_in = [x1, y1];

        if let Some(prev) = self.entries_by_id.remove(&entry.shape_id) {
            let _ = self.tree.remove(&prev);
            if entry.z_order == 0 && prev.z_order != 0 {
                entry.z_order = prev.z_order;
            }
        } else if entry.z_order == 0 && self.next_z_order > 0 {
            entry.z_order = self.next_z_order;
            self.next_z_order = self.next_z_order.saturating_add(1);
        } else {
            self.next_z_order = self.next_z_order.max(entry.z_order.saturating_add(1));
        }

        self.tree.insert(entry);
        self.entries_by_id.insert(entry.shape_id, entry);
    }

    /// Remove a shape from the R*-tree by `shape_id`.
    pub fn remove(&mut self, shape_id: u64) -> Option<SpatialEntry> {
        if let Some(prev) = self.entries_by_id.remove(&shape_id) {
            let _ = self.tree.remove(&prev);
            Some(prev)
        } else {
            None
        }
    }

    /// Query all shapes intersecting the viewport AABB `[min_in, max_in]` in inches,
    /// sorted in deterministic back-to-front painter's draw order `(z_order, shape_id)`.
    pub fn query_viewport(&self, min_in: [f64; 2], max_in: [f64; 2]) -> Vec<u64> {
        let mut hits = self.query_viewport_entries(min_in, max_in);
        hits.sort_unstable_by_key(|e| (e.z_order, e.shape_id));
        hits.into_iter().map(|e| e.shape_id).collect()
    }

    /// Query raw `SpatialEntry` records intersecting `[min_in, max_in]` sorted back-to-front.
    pub fn query_viewport_entries(&self, min_in: [f64; 2], max_in: [f64; 2]) -> Vec<SpatialEntry> {
        let (x0, x1) = sanitize_interval(min_in[0], max_in[0]);
        let (y0, y1) = sanitize_interval(min_in[1], max_in[1]);
        let envelope = AABB::from_corners([x0, y0], [x1, y1]);
        let mut hits: Vec<SpatialEntry> = self
            .tree
            .locate_in_envelope_intersecting(&envelope)
            .copied()
            .collect();
        hits.sort_unstable_by_key(|e| (e.z_order, e.shape_id));
        hits
    }

    /// Hit-test a page coordinate `(x_in, y_in)` within `tolerance_in` inches,
    /// returning matching `shape_id`s ordered front-to-back (topmost drawn shape first).
    pub fn hit_test_point(&self, x_in: f64, y_in: f64, tolerance_in: f64) -> Vec<u64> {
        let tol = if tolerance_in.is_finite() {
            tolerance_in.max(0.0)
        } else {
            0.0
        };
        let cx = if x_in.is_finite() { x_in } else { 0.0 };
        let cy = if y_in.is_finite() { y_in } else { 0.0 };
        let envelope = AABB::from_corners([cx - tol, cy - tol], [cx + tol, cy + tol]);

        let mut hits: Vec<SpatialEntry> = self
            .tree
            .locate_in_envelope_intersecting(&envelope)
            .copied()
            .collect();
        // Front-to-back order so clicking overlapping shapes selects the top shape first.
        hits.sort_unstable_by(|a, b| {
            b.z_order
                .cmp(&a.z_order)
                .then_with(|| b.shape_id.cmp(&a.shape_id))
        });
        hits.into_iter().map(|e| e.shape_id).collect()
    }

    /// Geometry-accurate hit-test using R*-tree broadphase (`O(log N)`) followed by narrowphase:
    /// - 1D connectors and open (`no_fill`) strokes require true point-to-polyline/curve distance `<= tol`,
    ///   preventing L/Z-connector AABBs from stealing clicks across the page.
    /// - Rotated 2D shapes test in local unrotated shape coordinates (`parent_to_local`).
    pub fn hit_test_visuals(
        &self,
        visuals: &[EvaluatedShapeVisual],
        x_in: f64,
        y_in: f64,
        tolerance_in: f64,
    ) -> Vec<u64> {
        let tol = if tolerance_in.is_finite() {
            tolerance_in.max(0.03)
        } else {
            0.06
        };
        let candidates = self.hit_test_point(x_in, y_in, tol);
        if candidates.is_empty() {
            return candidates;
        }
        let vis_by_id: BTreeMap<u64, &EvaluatedShapeVisual> =
            visuals.iter().map(|v| (v.shape_id, v)).collect();

        candidates
            .into_iter()
            .filter(|id| {
                let Some(vis) = vis_by_id.get(id) else {
                    return true;
                };
                visual_narrowphase_hit(vis, x_in, y_in, tol)
            })
            .collect()
    }

    /// Locate the nearest magnetic connection/snap point within `max_dist_in` inches of `(x_in, y_in)`.
    /// Uses `RTree::locate_within_distance` for `O(log N)` spatial pruning.
    pub fn nearest_snap_point(
        &self,
        x_in: f64,
        y_in: f64,
        max_dist_in: f64,
    ) -> Option<(u64, [f64; 2])> {
        if !x_in.is_finite() || !y_in.is_finite() || !max_dist_in.is_finite() || max_dist_in < 0.0 {
            return None;
        }
        let query = [x_in, y_in];
        let max_dist_sq = max_dist_in * max_dist_in;

        let mut best: Option<(f64, u64, [f64; 2])> = None;
        for entry in self.tree.locate_within_distance(query, max_dist_sq) {
            // Check explicit Visio connection points (cardinal midpoints, center, corners) first,
            // with a tiny preference for cardinal/center handles over arbitrary perimeter points.
            let candidates = entry.candidate_snap_points(query);
            for (idx, cand) in candidates.iter().enumerate() {
                let dx = cand[0] - x_in;
                let dy = cand[1] - y_in;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq <= max_dist_sq {
                    // Keypoint candidates (indices 0..9) win exact ties over arbitrary perimeter clamp (index 9).
                    let weighted_sq = if idx == 9 { dist_sq + 1e-12 } else { dist_sq };
                    let replace = match best {
                        None => true,
                        Some((best_sq, best_id, _)) => {
                            if (weighted_sq - best_sq).abs() > 1e-12 {
                                weighted_sq < best_sq
                            } else {
                                entry.shape_id < best_id
                            }
                        }
                    };
                    if replace {
                        best = Some((weighted_sq, entry.shape_id, *cand));
                    }
                }
            }
        }
        best.map(|(_, id, pt)| (id, pt))
    }
}

#[inline]
fn sanitize_interval(a: f64, b: f64) -> (f64, f64) {
    let fa = if a.is_finite() { a } else { 0.0 };
    let fb = if b.is_finite() { b } else { fa };
    if fa <= fb {
        (fa, fb)
    } else {
        (fb, fa)
    }
}

#[inline]
fn point_to_segment_dist_sq(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let vx = b[0] - a[0];
    let vy = b[1] - a[1];
    let len_sq = vx * vx + vy * vy;
    if len_sq <= 1e-12 {
        let dx = p[0] - a[0];
        let dy = p[1] - a[1];
        return dx * dx + dy * dy;
    }
    let t = (((p[0] - a[0]) * vx + (p[1] - a[1]) * vy) / len_sq).clamp(0.0, 1.0);
    let proj_x = a[0] + t * vx;
    let proj_y = a[1] + t * vy;
    let dx = p[0] - proj_x;
    let dy = p[1] - proj_y;
    dx * dx + dy * dy
}

fn min_dist_sq_to_visual_strokes(vis: &EvaluatedShapeVisual, pt: [f64; 2]) -> f64 {
    let mut best_sq = f64::INFINITY;
    for path in &vis.paths {
        if path.no_show {
            continue;
        }
        let mut cur = [0.0, 0.0];
        let mut start = [0.0, 0.0];
        let mut has_cur = false;
        for verb in &path.verbs_page_space {
            match *verb {
                EvaluatedPathVerb::MoveTo(p) => {
                    cur = p;
                    start = p;
                    has_cur = true;
                }
                EvaluatedPathVerb::LineTo(to) => {
                    if has_cur {
                        best_sq = best_sq.min(point_to_segment_dist_sq(pt, cur, to));
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::QuadTo { ctrl, to } => {
                    if has_cur {
                        let mut prev = cur;
                        for step in 1..=8 {
                            let t = step as f64 / 8.0;
                            let mt = 1.0 - t;
                            let nx = mt * mt * cur[0] + 2.0 * mt * t * ctrl[0] + t * t * to[0];
                            let ny = mt * mt * cur[1] + 2.0 * mt * t * ctrl[1] + t * t * to[1];
                            let next = [nx, ny];
                            best_sq = best_sq.min(point_to_segment_dist_sq(pt, prev, next));
                            prev = next;
                        }
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::CubicTo { ctrl1, ctrl2, to } => {
                    if has_cur {
                        let mut prev = cur;
                        for step in 1..=10 {
                            let t = step as f64 / 10.0;
                            let mt = 1.0 - t;
                            let nx = mt * mt * mt * cur[0]
                                + 3.0 * mt * mt * t * ctrl1[0]
                                + 3.0 * mt * t * t * ctrl2[0]
                                + t * t * t * to[0];
                            let ny = mt * mt * mt * cur[1]
                                + 3.0 * mt * mt * t * ctrl1[1]
                                + 3.0 * mt * t * t * ctrl2[1]
                                + t * t * t * to[1];
                            let next = [nx, ny];
                            best_sq = best_sq.min(point_to_segment_dist_sq(pt, prev, next));
                            prev = next;
                        }
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::ArcTo { to, bow } => {
                    if has_cur {
                        let mx = (cur[0] + to[0]) * 0.5;
                        let my = (cur[1] + to[1]) * 0.5;
                        let dx = to[0] - cur[0];
                        let dy = to[1] - cur[1];
                        let chord = (dx * dx + dy * dy).sqrt();
                        let ctrl = if chord > 1e-9 {
                            let nx = -dy / chord;
                            let ny = dx / chord;
                            [mx + nx * bow * 2.0, my + ny * bow * 2.0]
                        } else {
                            [mx, my]
                        };
                        let mut prev = cur;
                        for step in 1..=10 {
                            let t = step as f64 / 10.0;
                            let mt = 1.0 - t;
                            let nx = mt * mt * cur[0] + 2.0 * mt * t * ctrl[0] + t * t * to[0];
                            let ny = mt * mt * cur[1] + 2.0 * mt * t * ctrl[1] + t * t * to[1];
                            let next = [nx, ny];
                            best_sq = best_sq.min(point_to_segment_dist_sq(pt, prev, next));
                            prev = next;
                        }
                    }
                    cur = to;
                    has_cur = true;
                }
                EvaluatedPathVerb::Close => {
                    if has_cur {
                        best_sq = best_sq.min(point_to_segment_dist_sq(pt, cur, start));
                        cur = start;
                    }
                }
                EvaluatedPathVerb::Ellipse {
                    center,
                    rx,
                    ry,
                    angle_rad,
                } => {
                    let dx = pt[0] - center[0];
                    let dy = pt[1] - center[1];
                    let (s, c) = (-angle_rad).sin_cos();
                    let lx = (dx * c - dy * s) / rx.max(1e-4);
                    let ly = (dx * s + dy * c) / ry.max(1e-4);
                    let norm_r = (lx * lx + ly * ly).sqrt();
                    let approx_dist = ((norm_r - 1.0).abs()) * rx.min(ry).max(1e-4);
                    best_sq = best_sq.min(approx_dist * approx_dist);
                }
            }
        }
    }
    best_sq
}

fn visual_narrowphase_hit(vis: &EvaluatedShapeVisual, x_in: f64, y_in: f64, tol_in: f64) -> bool {
    let stroke_tol = (vis.stroke_weight_in.max(0.02) * 0.5 + tol_in).max(0.075);
    if vis.kind == ShapeKind::Connector1D {
        return min_dist_sq_to_visual_strokes(vis, [x_in, y_in]) <= stroke_tol * stroke_tol;
    }

    let (lx, ly) = vis.transform.parent_to_local(x_in, y_in);
    let w = vis.transform.width.max(0.0);
    let h = vis.transform.height.max(0.0);
    if lx >= -tol_in && lx <= w + tol_in && ly >= -tol_in && ly <= h + tol_in {
        return true;
    }

    // Fallback for freeform/boolean shapes whose evaluated path vertices extend beyond [0..w, 0..h]
    min_dist_sq_to_visual_strokes(vis, [x_in, y_in]) <= stroke_tol * stroke_tol
}

