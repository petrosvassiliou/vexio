//! Dynamic Connector Routing, Static/Dynamic Glue, Obstacle Avoidance & Line Jumps.
//!
//! Implements Microsoft Visio's connector engine:
//! - Point-to-point static connection point glue & shape-to-shape dynamic walking glue
//! - Orthogonal weighted A* visibility-grid routing avoiding 2D shape obstacles
//! - Straight and smooth curved (Catmull-Rom -> Cubic Bezier) routing
//! - Real-time connector-connector intersection detection with Arc, Gap, and Square line jumps.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, BTreeMap, BTreeSet};
use vexio_shapesheet::{
    GeometryRow, GeometryRowKind, GeometrySection, ReactiveEngine, ShapeKind, Unit, VisioDocument,
    VisioPage, format_clean_f64,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectorRouteMode {
    Orthogonal,
    Straight,
    Curved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineJumpStyle {
    None,
    Arc,
    Gap,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ObstacleRect {
    pub shape_id: u64,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl ObstacleRect {
    pub fn padded(&self, pad: f64) -> Self {
        Self {
            shape_id: self.shape_id,
            min_x: self.min_x - pad,
            min_y: self.min_y - pad,
            max_x: self.max_x + pad,
            max_y: self.max_y + pad,
        }
    }

    pub fn strictly_contains(&self, x: f64, y: f64) -> bool {
        x > self.min_x + 1e-6
            && x < self.max_x - 1e-6
            && y > self.min_y + 1e-6
            && y < self.max_y - 1e-6
    }

    pub fn intersects_open_segment(&self, a: [f64; 2], b: [f64; 2]) -> bool {
        // Orthogonal or general segment check against open interior of obstacle rectangle.
        let steps = 8;
        for i in 1..steps {
            let t = i as f64 / steps as f64;
            let px = a[0] + (b[0] - a[0]) * t;
            let py = a[1] + (b[1] - a[1]) * t;
            if self.strictly_contains(px, py) {
                return true;
            }
        }
        false
    }
}

/// Re-resolve all glued endpoints, route all 1D connectors around obstacles, and apply line jumps on `page_id`.
pub fn route_page_connectors(doc: &mut VisioDocument, page_id: u32) {
    ReactiveEngine::recalculate_document(doc);
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    let jump_code = page.page_sheet.get_f64("PageLineJumpCode").unwrap_or(1.0) as i32;
    let jump_style = match page.page_sheet.get_f64("PageLineJumpStyle").unwrap_or(0.0) as i32 {
        _ if jump_code == 0 => LineJumpStyle::None,
        1 => LineJumpStyle::Gap,
        2 => LineJumpStyle::Square,
        _ => LineJumpStyle::Arc,
    };

    // 1. Collect 2D obstacle AABBs
    let mut obstacles = Vec::new();
    let mut shape_centers: BTreeMap<u64, ([f64; 2], [f64; 2])> = BTreeMap::new();
    for s in &page.shapes {
        if s.kind == ShapeKind::Shape2D || s.kind == ShapeKind::Group {
            let px = s.sheet.get_f64("PinX").unwrap_or(0.0);
            let py = s.sheet.get_f64("PinY").unwrap_or(0.0);
            let w = s.sheet.get_f64("Width").unwrap_or(1.0);
            let h = s.sheet.get_f64("Height").unwrap_or(1.0);
            let lpx = s.sheet.get_f64("LocPinX").unwrap_or(w * 0.5);
            let lpy = s.sheet.get_f64("LocPinY").unwrap_or(h * 0.5);
            let min_x = px - lpx;
            let min_y = py - lpy;
            let max_x = min_x + w;
            let max_y = min_y + h;
            obstacles.push(ObstacleRect {
                shape_id: s.id,
                min_x,
                min_y,
                max_x,
                max_y,
            });
            shape_centers.insert(s.id, ([px, py], [w, h]));
        }
    }

    // 2. Resolve static and dynamic glue endpoints for all `Connect` records
    let connects = page.connects.clone();
    let mut conn_endpoints: BTreeMap<u64, (Option<(u64, [f64; 2])>, Option<(u64, [f64; 2])>)> =
        BTreeMap::new();

    for c in &connects {
        let entry = conn_endpoints
            .entry(c.connector_shape_id)
            .or_insert((None, None));
        let target_pt = resolve_glue_point(page, c.target_shape_id, &c.to_cell, c.is_dynamic);
        if let Some(pt) = target_pt {
            if c.from_cell.to_ascii_lowercase().starts_with("begin") {
                entry.0 = Some((c.target_shape_id, pt));
            } else {
                entry.1 = Some((c.target_shape_id, pt));
            }
        }
    }

    // Refine dynamic walking glue so source & target pick facing cardinal boundary ports!
    for (conn_id, (beg_opt, end_opt)) in &mut conn_endpoints {
        let cur_conn = page.find_shape(*conn_id);
        let fallback_beg = cur_conn
            .map(|s| {
                [
                    s.sheet.get_f64("BeginX").unwrap_or(0.0),
                    s.sheet.get_f64("BeginY").unwrap_or(0.0),
                ]
            })
            .unwrap_or([0.0, 0.0]);
        let fallback_end = cur_conn
            .map(|s| {
                [
                    s.sheet.get_f64("EndX").unwrap_or(1.0),
                    s.sheet.get_f64("EndY").unwrap_or(1.0),
                ]
            })
            .unwrap_or([1.0, 1.0]);

        let other_for_beg = end_opt.map(|(_, p)| p).unwrap_or(fallback_end);
        let other_for_end = beg_opt.map(|(_, p)| p).unwrap_or(fallback_beg);

        if let Some((tid, pt)) = beg_opt {
            if let Some(c_rec) = connects.iter().find(|c| {
                c.connector_shape_id == *conn_id
                    && c.from_cell.to_ascii_lowercase().starts_with("begin")
            }) {
                if c_rec.is_dynamic {
                    if let Some((center, wh)) = shape_centers.get(tid) {
                        *pt = cardinal_boundary_port(*center, *wh, other_for_beg);
                    }
                }
            }
        }
        if let Some((tid, pt)) = end_opt {
            if let Some(c_rec) = connects.iter().find(|c| {
                c.connector_shape_id == *conn_id
                    && !c.from_cell.to_ascii_lowercase().starts_with("begin")
            }) {
                if c_rec.is_dynamic {
                    if let Some((center, wh)) = shape_centers.get(tid) {
                        *pt = cardinal_boundary_port(*center, *wh, other_for_end);
                    }
                }
            }
        }
    }

    // 3. Route each 1D connector and store world-space polylines for line-jump calculation
    let margin = 0.25; // 0.25 in obstacle clearance matching Visio default
    let mut routed_polylines: Vec<(u64, ConnectorRouteMode, Vec<[f64; 2]>)> = Vec::new();

    for s in &mut page.shapes {
        if s.kind != ShapeKind::Connector1D {
            continue;
        }
        let mut bx = s.sheet.get_f64("BeginX").unwrap_or(0.0);
        let mut by = s.sheet.get_f64("BeginY").unwrap_or(0.0);
        let mut ex = s.sheet.get_f64("EndX").unwrap_or(1.0);
        let mut ey = s.sheet.get_f64("EndY").unwrap_or(1.0);
        let mut ignore_ids = BTreeSet::new();

        if let Some((b_glue, e_glue)) = conn_endpoints.get(&s.id) {
            if let Some((tid, p)) = b_glue {
                bx = p[0];
                by = p[1];
                ignore_ids.insert(*tid);
                s.sheet.set_formula(
                    "BeginX",
                    format!("{} in", format_clean_f64(bx)),
                    Some(Unit::Inch),
                );
                s.sheet.set_formula(
                    "BeginY",
                    format!("{} in", format_clean_f64(by)),
                    Some(Unit::Inch),
                );
            }
            if let Some((tid, p)) = e_glue {
                ex = p[0];
                ey = p[1];
                ignore_ids.insert(*tid);
                s.sheet.set_formula(
                    "EndX",
                    format!("{} in", format_clean_f64(ex)),
                    Some(Unit::Inch),
                );
                s.sheet.set_formula(
                    "EndY",
                    format!("{} in", format_clean_f64(ey)),
                    Some(Unit::Inch),
                );
            }
        }

        if s.sheet.get_bool("User.Preserve1DGeometry").unwrap_or(false) {
            continue;
        }

        let style_code = s.sheet.get_f64("ShapeRouteStyle").unwrap_or(16.0) as i32;
        let mode = match style_code {
            0 | 1 | 2 => ConnectorRouteMode::Straight,
            6 | 7 => ConnectorRouteMode::Curved,
            _ => ConnectorRouteMode::Orthogonal,
        };

        let active_obs: Vec<ObstacleRect> = obstacles
            .iter()
            .filter(|o| !ignore_ids.contains(&o.shape_id))
            .map(|o| o.padded(margin))
            .collect();

        let poly = match mode {
            ConnectorRouteMode::Straight => vec![[bx, by], [ex, ey]],
            ConnectorRouteMode::Curved => vec![[bx, by], [ex, ey]],
            ConnectorRouteMode::Orthogonal => {
                orthogonal_astar_route([bx, by], [ex, ey], &active_obs)
            }
        };
        routed_polylines.push((s.id, mode, poly));
    }

    // 4. Write routed Geometry1 (including horizontal connector Line Jumps over vertical segments!)
    let vertical_segments = collect_vertical_segments(&routed_polylines);
    for (conn_id, mode, poly) in routed_polylines {
        let Some(s) = page.find_shape_mut(conn_id) else {
            continue;
        };
        let bx = poly.first().map(|p| p[0]).unwrap_or(0.0);
        let by = poly.first().map(|p| p[1]).unwrap_or(0.0);
        let ex = poly.last().map(|p| p[0]).unwrap_or(1.0);
        let ey = poly.last().map(|p| p[1]).unwrap_or(0.0);

        // Position 1D connector in page coordinates with Angle=0, LocPin=(0,0), Pin=(bx,by)
        // when orthogonal/curved multi-segment path is generated so local geometry = `(pt - begin)`.
        let dx_total = (ex - bx).abs().max(0.01);
        let dy_total = (ey - by).abs().max(0.01);
        s.sheet
            .set_formula("PinX", format!("{} in", format_clean_f64(bx)), Some(Unit::Inch));
        s.sheet
            .set_formula("PinY", format!("{} in", format_clean_f64(by)), Some(Unit::Inch));
        s.sheet
            .set_formula("LocPinX", "0 in", Some(Unit::Inch));
        s.sheet
            .set_formula("LocPinY", "0 in", Some(Unit::Inch));
        s.sheet
            .set_formula("Width", format!("{} in", format_clean_f64(dx_total)), Some(Unit::Inch));
        s.sheet
            .set_formula("Height", format!("{} in", format_clean_f64(dy_total)), Some(Unit::Inch));
        s.sheet
            .set_formula("Angle", "0 rad", Some(Unit::Radian));

        let mut geom = GeometrySection::new(1);
        geom.no_fill = "TRUE".into();

        if mode == ConnectorRouteMode::Curved && poly.len() == 2 {
            let mx = (ex - bx) * 0.5;
            let my = (ey - by) * 0.5;
            geom.rows.push(GeometryRow::move_to(1, "0 in", "0 in"));
            geom.rows.push(GeometryRow {
                ix: 2,
                kind: GeometryRowKind::ArcTo,
                x: format!("{} in", format_clean_f64(mx)),
                y: format!("{} in", format_clean_f64(my)),
                a: Some("0.25 in".into()),
                b: None,
                c: None,
                d: None,
                e: None,
                deleted: false,
            });
            geom.rows.push(GeometryRow {
                ix: 3,
                kind: GeometryRowKind::ArcTo,
                x: format!("{} in", format_clean_f64(ex - bx)),
                y: format!("{} in", format_clean_f64(ey - by)),
                a: Some("-0.25 in".into()),
                b: None,
                c: None,
                d: None,
                e: None,
                deleted: false,
            });
        } else {
            let mut ix = 1u32;
            geom.rows.push(GeometryRow::move_to(ix, "0 in", "0 in"));
            ix += 1;

            for seg in poly.windows(2) {
                let p0 = seg[0];
                let p1 = seg[1];
                let is_horiz = (p0[1] - p1[1]).abs() < 1e-6 && (p1[0] - p0[0]).abs() > 0.25;
                if is_horiz && jump_style != LineJumpStyle::None {
                    let mut crossings = find_crossings_on_horizontal(
                        conn_id,
                        p0[0],
                        p1[0],
                        p0[1],
                        &vertical_segments,
                    );
                    if p1[0] < p0[0] {
                        crossings.reverse();
                    }
                    let dir = if p1[0] >= p0[0] { 1.0 } else { -1.0 };
                    let r = 0.08; // 0.08 in jump radius
                    for cx in crossings {
                        let pre_x = cx - dir * r;
                        let post_x = cx + dir * r;
                        geom.rows.push(GeometryRow::line_to(
                            ix,
                            format!("{} in", format_clean_f64(pre_x - bx)),
                            format!("{} in", format_clean_f64(p0[1] - by)),
                        ));
                        ix += 1;
                        match jump_style {
                            LineJumpStyle::Arc => {
                                geom.rows.push(GeometryRow::arc_to(
                                    ix,
                                    format!("{} in", format_clean_f64(post_x - bx)),
                                    format!("{} in", format_clean_f64(p0[1] - by)),
                                    format!("{} in", format_clean_f64(r * dir)),
                                ));
                                ix += 1;
                            }
                            LineJumpStyle::Gap => {
                                geom.rows.push(GeometryRow::move_to(
                                    ix,
                                    format!("{} in", format_clean_f64(post_x - bx)),
                                    format!("{} in", format_clean_f64(p0[1] - by)),
                                ));
                                ix += 1;
                            }
                            LineJumpStyle::Square => {
                                geom.rows.push(GeometryRow::line_to(
                                    ix,
                                    format!("{} in", format_clean_f64(pre_x - bx)),
                                    format!("{} in", format_clean_f64(p0[1] + r - by)),
                                ));
                                ix += 1;
                                geom.rows.push(GeometryRow::line_to(
                                    ix,
                                    format!("{} in", format_clean_f64(post_x - bx)),
                                    format!("{} in", format_clean_f64(p0[1] + r - by)),
                                ));
                                ix += 1;
                                geom.rows.push(GeometryRow::line_to(
                                    ix,
                                    format!("{} in", format_clean_f64(post_x - bx)),
                                    format!("{} in", format_clean_f64(p0[1] - by)),
                                ));
                                ix += 1;
                            }
                            LineJumpStyle::None => {}
                        }
                    }
                }
                geom.rows.push(GeometryRow::line_to(
                    ix,
                    format!("{} in", format_clean_f64(p1[0] - bx)),
                    format!("{} in", format_clean_f64(p1[1] - by)),
                ));
                ix += 1;
            }
        }
        s.sheet.add_geometry(geom);
    }

    ReactiveEngine::recalculate_document(doc);
}

fn resolve_glue_point(
    page: &VisioPage,
    target_id: u64,
    to_cell: &str,
    is_dynamic: bool,
) -> Option<[f64; 2]> {
    let sh = page.find_shape(target_id)?;
    let px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
    let py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
    let w = sh.sheet.get_f64("Width").unwrap_or(1.0);
    let h = sh.sheet.get_f64("Height").unwrap_or(1.0);
    let lpx = sh.sheet.get_f64("LocPinX").unwrap_or(w * 0.5);
    let lpy = sh.sheet.get_f64("LocPinY").unwrap_or(h * 0.5);

    if !is_dynamic {
        // Check explicit ConnectionPoint cell (`Connections.X1` -> `(Connections.X1, Connections.Y1)`)
        let lower = to_cell.to_ascii_lowercase();
        if let Some(suffix) = lower.strip_prefix("connections.x") {
            let cx = sh.sheet.get_f64(&format!("Connections.X{suffix}"))?;
            let cy = sh.sheet.get_f64(&format!("Connections.Y{suffix}"))?;
            return Some([px - lpx + cx, py - lpy + cy]);
        }
    }
    Some([px, py])
}

fn cardinal_boundary_port(center: [f64; 2], wh: [f64; 2], toward: [f64; 2]) -> [f64; 2] {
    let dx = toward[0] - center[0];
    let dy = toward[1] - center[1];
    let hw = wh[0] * 0.5;
    let hh = wh[1] * 0.5;
    if dx.abs() * hh >= dy.abs() * hw {
        [center[0] + if dx >= 0.0 { hw } else { -hw }, center[1]]
    } else {
        [center[0], center[1] + if dy >= 0.0 { hh } else { -hh }]
    }
}

/// Non-uniform visibility-grid weighted A* orthogonal router with bend penalty.
pub fn orthogonal_astar_route(
    start: [f64; 2],
    goal: [f64; 2],
    obstacles: &[ObstacleRect],
) -> Vec<[f64; 2]> {
    // Fast path: straight horizontal or vertical clear line
    if ((start[0] - goal[0]).abs() < 1e-6 || (start[1] - goal[1]).abs() < 1e-6)
        && obstacles.iter().all(|o| !o.intersects_open_segment(start, goal))
    {
        return vec![start, goal];
    }

    // Build Hanan visibility grid from start, goal, midpoints, and obstacle boundaries
    let mut xs = vec![start[0], goal[0], (start[0] + goal[0]) * 0.5];
    let mut ys = vec![start[1], goal[1], (start[1] + goal[1]) * 0.5];
    for o in obstacles {
        xs.push(o.min_x);
        xs.push(o.max_x);
        ys.push(o.min_y);
        ys.push(o.max_y);
    }
    dedup_sorted_f64(&mut xs);
    dedup_sorted_f64(&mut ys);

    let sx = nearest_idx(&xs, start[0]);
    let sy = nearest_idx(&ys, start[1]);
    let gx = nearest_idx(&xs, goal[0]);
    let gy = nearest_idx(&ys, goal[1]);

    #[derive(Clone, Copy, PartialEq)]
    struct Node {
        f_cost: f64,
        g_cost: f64,
        ix: usize,
        iy: usize,
        dir: u8, // 0=Start, 1=Horiz, 2=Vert
    }
    impl Eq for Node {}
    impl Ord for Node {
        fn cmp(&self, other: &Self) -> Ordering {
            other
                .f_cost
                .partial_cmp(&self.f_cost)
                .unwrap_or(Ordering::Equal)
        }
    }
    impl PartialOrd for Node {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut open = BinaryHeap::new();
    let mut best_g: BTreeMap<(usize, usize, u8), f64> = BTreeMap::new();
    let mut came_from: BTreeMap<(usize, usize, u8), (usize, usize, u8)> = BTreeMap::new();

    let h0 = (xs[sx] - xs[gx]).abs() + (ys[sy] - ys[gy]).abs();
    open.push(Node {
        f_cost: h0,
        g_cost: 0.0,
        ix: sx,
        iy: sy,
        dir: 0,
    });
    best_g.insert((sx, sy, 0), 0.0);

    let bend_penalty = 0.35; // 0.35 in equivalent penalty per 90-degree bend
    let mut goal_state = None;

    while let Some(cur) = open.pop() {
        if cur.ix == gx && cur.iy == gy {
            goal_state = Some((cur.ix, cur.iy, cur.dir));
            break;
        }
        if let Some(&known) = best_g.get(&(cur.ix, cur.iy, cur.dir)) {
            if cur.g_cost > known + 1e-9 {
                continue;
            }
        }

        let neighbors: [(isize, isize, u8); 4] = [
            (-1, 0, 1),
            (1, 0, 1),
            (0, -1, 2),
            (0, 1, 2),
        ];

        for (dx, dy, ndir) in neighbors {
            let nx = cur.ix as isize + dx;
            let ny = cur.iy as isize + dy;
            if nx < 0 || ny < 0 || nx as usize >= xs.len() || ny as usize >= ys.len() {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let p_a = [xs[cur.ix], ys[cur.iy]];
            let p_b = [xs[nx], ys[ny]];

            let blocked = !(nx == gx && ny == gy)
                && obstacles
                    .iter()
                    .any(|o| o.strictly_contains(p_b[0], p_b[1]) || o.intersects_open_segment(p_a, p_b));
            if blocked {
                continue;
            }

            let dist = (p_a[0] - p_b[0]).abs() + (p_a[1] - p_b[1]).abs();
            let turn = if cur.dir != 0 && cur.dir != ndir {
                bend_penalty
            } else {
                0.0
            };
            let ng = cur.g_cost + dist + turn;
            let state = (nx, ny, ndir);
            if ng + 1e-9 < *best_g.get(&state).unwrap_or(&f64::INFINITY) {
                best_g.insert(state, ng);
                came_from.insert(state, (cur.ix, cur.iy, cur.dir));
                let h = (p_b[0] - xs[gx]).abs() + (p_b[1] - ys[gy]).abs();
                open.push(Node {
                    f_cost: ng + h,
                    g_cost: ng,
                    ix: nx,
                    iy: ny,
                    dir: ndir,
                });
            }
        }
    }

    if let Some(mut st) = goal_state {
        let mut pts = vec![[xs[st.0], ys[st.1]]];
        while let Some(&prev) = came_from.get(&st) {
            pts.push([xs[prev.0], ys[prev.1]]);
            st = prev;
        }
        pts.reverse();
        simplify_collinear(&pts)
    } else {
        // Fallback L-bend if enclosed
        let mid_x = (start[0] + goal[0]) * 0.5;
        simplify_collinear(&[start, [mid_x, start[1]], [mid_x, goal[1]], goal])
    }
}

fn dedup_sorted_f64(v: &mut Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    v.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
}

fn nearest_idx(sorted: &[f64], target: f64) -> usize {
    sorted
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            ((*a - target).abs())
                .partial_cmp(&((*b - target).abs()))
                .unwrap_or(Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn simplify_collinear(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let mut out = vec![pts[0]];
    for i in 1..pts.len() - 1 {
        let prev = *out.last().unwrap();
        let cur = pts[i];
        let next = pts[i + 1];
        let same_x = (prev[0] - cur[0]).abs() < 1e-6 && (cur[0] - next[0]).abs() < 1e-6;
        let same_y = (prev[1] - cur[1]).abs() < 1e-6 && (cur[1] - next[1]).abs() < 1e-6;
        if !same_x && !same_y {
            out.push(cur);
        }
    }
    out.push(*pts.last().unwrap());
    out
}

fn collect_vertical_segments(
    polylines: &[(u64, ConnectorRouteMode, Vec<[f64; 2]>)],
) -> Vec<(u64, f64, f64, f64)> {
    let mut out = Vec::new();
    for (id, _, pts) in polylines {
        for seg in pts.windows(2) {
            if (seg[0][0] - seg[1][0]).abs() < 1e-6 && (seg[0][1] - seg[1][1]).abs() > 1e-4 {
                let y_min = seg[0][1].min(seg[1][1]);
                let y_max = seg[0][1].max(seg[1][1]);
                out.push((*id, seg[0][0], y_min, y_max));
            }
        }
    }
    out
}

fn find_crossings_on_horizontal(
    own_id: u64,
    x0: f64,
    x1: f64,
    y: f64,
    verticals: &[(u64, f64, f64, f64)],
) -> Vec<f64> {
    let min_x = x0.min(x1) + 0.12;
    let max_x = x0.max(x1) - 0.12;
    if min_x >= max_x {
        return Vec::new();
    }
    let mut xs = Vec::new();
    for &(vid, vx, vy0, vy1) in verticals {
        if vid != own_id && vx > min_x && vx < max_x && y > vy0 + 0.05 && y < vy1 - 0.05 {
            xs.push(vx);
        }
    }
    dedup_sorted_f64(&mut xs);
    xs
}
