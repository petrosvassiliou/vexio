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
    TwoSides,
    ThreeSides,
    FourSides,
    FiveSides,
}

impl LineJumpStyle {
    pub fn from_visio_code(code: i32) -> Self {
        match code {
            1 => Self::Gap,
            2 => Self::Square,
            3 => Self::TwoSides,
            4 => Self::ThreeSides,
            5 => Self::FourSides,
            6 => Self::FiveSides,
            _ => Self::Arc,
        }
    }
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
    let visuals = ReactiveEngine::evaluate_page_visuals(doc, page_id);
    let world_transforms: BTreeMap<u64, vexio_shapesheet::ShapeTransform2D> =
        visuals.iter().map(|v| (v.shape_id, v.transform)).collect();
    let masters_snapshot = doc.masters.clone();
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    let jump_code = page.page_sheet.get_f64("PageLineJumpCode").unwrap_or(1.0).round() as i32;
    let page_style_code = page.page_sheet.get_f64("PageLineJumpStyle").unwrap_or(0.0).round() as i32;
    let jump_factor_x = page
        .page_sheet
        .get_f64("PageLineJumpFactorX")
        .unwrap_or(0.666)
        .clamp(0.15, 3.0);
    let jump_factor_y = page
        .page_sheet
        .get_f64("PageLineJumpFactorY")
        .unwrap_or(0.666)
        .clamp(0.15, 3.0);
    let jump_rx = (0.08 * (jump_factor_x / 0.666)).clamp(0.03, 0.35);
    let jump_ry = (0.08 * (jump_factor_y / 0.666)).clamp(0.03, 0.35);

    // 1. Collect 2D obstacle AABBs and world-space shape centers (including group subshapes)
    let mut obstacles = Vec::new();
    let mut shape_centers: BTreeMap<u64, ([f64; 2], [f64; 2])> = BTreeMap::new();
    for (&sid, wtf) in &world_transforms {
        shape_centers.insert(
            sid,
            ([wtf.pin_x, wtf.pin_y], [wtf.width.abs(), wtf.height.abs()]),
        );
    }
    let mut top_of_id: BTreeMap<u64, u64> = BTreeMap::new();
    for s in &page.shapes {
        let mut sub_ids = BTreeSet::new();
        vexio_shapesheet::VisioPage::collect_subtree_ids(s, &mut sub_ids);
        for sid in sub_ids {
            top_of_id.insert(sid, s.id);
        }
        if matches!(s.kind, ShapeKind::Shape2D | ShapeKind::ForeignObject)
            || (s.kind == ShapeKind::Group && s.master_id.is_some())
        {
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
        } else if s.kind == ShapeKind::Group {
            for ch in &s.children {
                if matches!(
                    ch.kind,
                    ShapeKind::Shape2D | ShapeKind::Group | ShapeKind::ForeignObject
                ) {
                    if let Some(wtf) = world_transforms.get(&ch.id) {
                        let min_x = wtf.pin_x - wtf.loc_pin_x;
                        let min_y = wtf.pin_y - wtf.loc_pin_y;
                        obstacles.push(ObstacleRect {
                            shape_id: ch.id,
                            min_x,
                            min_y,
                            max_x: min_x + wtf.width.abs(),
                            max_y: min_y + wtf.height.abs(),
                        });
                    }
                }
            }
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
        let target_pt = resolve_glue_point(
            page,
            &masters_snapshot,
            c.target_shape_id,
            world_transforms.get(&c.target_shape_id).copied(),
            &c.to_cell,
            c.is_dynamic,
        );
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
                if let Some(&top_id) = top_of_id.get(tid) {
                    ignore_ids.insert(top_id);
                }
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
                if let Some(&top_id) = top_of_id.get(tid) {
                    ignore_ids.insert(top_id);
                }
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

        let manual_pts = s
            .sheet
            .get_cell("User.ManualOrthoPoints")
            .and_then(|c| c.value.as_str_coerced().ok())
            .map(|raw| parse_manual_ortho_points(&raw))
            .unwrap_or_default();

        let mid_axis = s
            .sheet
            .get_f64("User.MidAxis")
            .map(|v| v.round() as i32)
            .unwrap_or(0);
        let mid_coord = s.sheet.get_f64("User.MidCoord").filter(|v| v.is_finite());

        let poly = match mode {
            ConnectorRouteMode::Straight => vec![[bx, by], [ex, ey]],
            ConnectorRouteMode::Curved => vec![[bx, by], [ex, ey]],
            ConnectorRouteMode::Orthogonal => {
                if manual_pts.len() >= 2 {
                    reanchor_manual_ortho_polyline(&manual_pts, [bx, by], [ex, ey])
                } else if let Some(mc) = mid_coord {
                    if mid_axis == 1 {
                        // User-dragged vertical middle segment at `x = mc`
                        compact_orthogonal_polyline(&[[bx, by], [mc, by], [mc, ey], [ex, ey]])
                    } else if mid_axis == 2 {
                        // User-dragged horizontal middle segment at `y = mc`
                        compact_orthogonal_polyline(&[[bx, by], [bx, mc], [ex, mc], [ex, ey]])
                    } else {
                        orthogonal_astar_route([bx, by], [ex, ey], &active_obs)
                    }
                } else {
                    orthogonal_astar_route([bx, by], [ex, ey], &active_obs)
                }
            }
        };
        routed_polylines.push((s.id, mode, poly));
    }

    // 4. Write routed Geometry1 (including horizontal/vertical/Z-ordered Line Jumps across all 7 Visio jump styles!)
    let vertical_segments = collect_vertical_segments(&routed_polylines);
    let horizontal_segments = collect_horizontal_segments(&routed_polylines);
    let z_rank_by_id: BTreeMap<u64, usize> = routed_polylines
        .iter()
        .enumerate()
        .map(|(z, (cid, _, _))| (*cid, z))
        .collect();

    for (conn_id, mode, poly) in routed_polylines {
        let own_z = z_rank_by_id.get(&conn_id).copied().unwrap_or(0);
        let Some(s) = page.find_shape_mut(conn_id) else {
            continue;
        };
        // Per-connector `ConLineJumpCode`: `1` = Page Default, `2` = Never, `3` = Always (or `0` when explicitly set to `0` while `User.ConJumpExplicit == 1`).
        let conn_jump_code = s.sheet.get_f64("ConLineJumpCode").unwrap_or(1.0).round() as i32;
        let explicit_conn_off = conn_jump_code == 2
            || (conn_jump_code == 0
                && s.sheet.get_f64("User.ConJumpExplicit").unwrap_or(0.0) > 0.5);
        let eff_jump_code = if explicit_conn_off {
            0
        } else if conn_jump_code == 3 {
            if jump_code == 0 { 1 } else { jump_code }
        } else {
            jump_code
        };
        let conn_style_raw = s.sheet.get_f64("ConLineJumpStyle").unwrap_or(0.0).round() as i32;
        let explicit_style = s.sheet.get_f64("User.ConJumpStyleExplicit").unwrap_or(0.0) > 0.5;
        let jump_style = if eff_jump_code == 0 {
            LineJumpStyle::None
        } else if explicit_style || conn_style_raw > 0 {
            LineJumpStyle::from_visio_code(conn_style_raw)
        } else {
            LineJumpStyle::from_visio_code(page_style_code)
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
                let is_vert = (p0[0] - p1[0]).abs() < 1e-6 && (p1[1] - p0[1]).abs() > 0.25;

                if is_horiz && jump_style != LineJumpStyle::None && matches!(eff_jump_code, 1 | 3) {
                    let mut crossings = find_crossings_on_horizontal_filtered(
                        conn_id,
                        own_z,
                        eff_jump_code == 3,
                        p0[0],
                        p1[0],
                        p0[1],
                        &vertical_segments,
                        &z_rank_by_id,
                    );
                    if p1[0] < p0[0] {
                        crossings.reverse();
                    }
                    let dir = if p1[0] >= p0[0] { 1.0 } else { -1.0 };
                    for cx in crossings {
                        emit_line_jump_rows(
                            &mut geom.rows,
                            &mut ix,
                            jump_style,
                            [cx - dir * jump_rx, p0[1]],
                            [cx + dir * jump_rx, p0[1]],
                            [0.0, jump_ry],
                            jump_ry * dir,
                            [bx, by],
                        );
                    }
                } else if is_vert && jump_style != LineJumpStyle::None && matches!(eff_jump_code, 2 | 3) {
                    let mut crossings = find_crossings_on_vertical_filtered(
                        conn_id,
                        own_z,
                        eff_jump_code == 3,
                        p0[1],
                        p1[1],
                        p0[0],
                        &horizontal_segments,
                        &z_rank_by_id,
                    );
                    if p1[1] < p0[1] {
                        crossings.reverse();
                    }
                    let dir = if p1[1] >= p0[1] { 1.0 } else { -1.0 };
                    for cy in crossings {
                        emit_line_jump_rows(
                            &mut geom.rows,
                            &mut ix,
                            jump_style,
                            [p0[0], cy - dir * jump_ry],
                            [p0[0], cy + dir * jump_ry],
                            [-jump_rx, 0.0],
                            jump_rx * dir,
                            [bx, by],
                        );
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
        // Compute true arc-length midpoint along `poly` so 1D connector labels always sit
        // directly on the routed path (even when `EndX < BeginX`, `EndY < BeginY`, or L/U-routed!).
        let mut mid_page = [(bx + ex) * 0.5, (by + ey) * 0.5];
        if mode == ConnectorRouteMode::Curved && poly.len() == 2 {
            let cdx = ex - bx;
            let cdy = ey - by;
            let clen = cdx.hypot(cdy);
            if clen > 1e-6 {
                // First half of S-curve bows by +0.25 in; overall center sits near `(bx+ex)/2, (by+ey)/2`
                mid_page = [bx + cdx * 0.5, by + cdy * 0.5];
            }
        } else if poly.len() >= 2 {
            let total_len: f64 = poly
                .windows(2)
                .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                .sum();
            if total_len > 1e-6 {
                let half = total_len * 0.5;
                let mut acc = 0.0;
                for seg in poly.windows(2) {
                    let slen = (seg[1][0] - seg[0][0]).hypot(seg[1][1] - seg[0][1]);
                    if acc + slen >= half && slen > 1e-9 {
                        let t = ((half - acc) / slen).clamp(0.0, 1.0);
                        mid_page = [
                            seg[0][0] + (seg[1][0] - seg[0][0]) * t,
                            seg[0][1] + (seg[1][1] - seg[0][1]) * t,
                        ];
                        break;
                    }
                    acc += slen;
                }
            }
        }
        let _ = s.sheet.try_set_formula_respecting_guard(
            "TxtPinX",
            format!("{} in", format_clean_f64(mid_page[0] - bx)),
            false,
        );
        let _ = s.sheet.try_set_formula_respecting_guard(
            "TxtPinY",
            format!("{} in", format_clean_f64(mid_page[1] - by)),
            false,
        );
        if s.sheet.get_cell("TxtWidth").is_none() {
            s.sheet
                .set_formula("TxtWidth", "2.2 in".to_string(), Some(Unit::Inch));
        }
        if s.sheet.get_cell("TxtHeight").is_none() {
            s.sheet
                .set_formula("TxtHeight", "0.5 in".to_string(), Some(Unit::Inch));
        }

        s.sheet.add_geometry(geom);
    }

    ReactiveEngine::recalculate_document(doc);
}

pub fn compact_orthogonal_polyline(pts: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut out: Vec<[f64; 2]> = Vec::with_capacity(pts.len());
    for &p in pts {
        if let Some(&last) = out.last() {
            if (last[0] - p[0]).hypot(last[1] - p[1]) < 1e-5 {
                continue;
            }
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let collinear_x = (a[0] - b[0]).abs() < 1e-5 && (b[0] - p[0]).abs() < 1e-5;
            let collinear_y = (a[1] - b[1]).abs() < 1e-5 && (b[1] - p[1]).abs() < 1e-5;
            if collinear_x || collinear_y {
                out.pop();
            }
        }
        out.push(p);
    }
    if out.len() < 2 && !pts.is_empty() {
        vec![pts[0], *pts.last().unwrap_or(&pts[0])]
    } else {
        out
    }
}

/// Parse serialized `User.ManualOrthoPoints` (`"x0,y0;x1,y1;..."`) into page-space `[x, y]` vertices.
pub fn parse_manual_ortho_points(raw: &str) -> Vec<[f64; 2]> {
    let trimmed = raw.trim().trim_matches('"').trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut pts = Vec::new();
    for pair in trimmed.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((xs, ys)) = pair.split_once(',') {
            if let (Ok(x), Ok(y)) = (xs.trim().parse::<f64>(), ys.trim().parse::<f64>()) {
                if x.is_finite() && y.is_finite() {
                    pts.push([x, y]);
                }
            }
        }
    }
    pts
}

/// Format page-space `[x, y]` vertices as a quoted ShapeSheet string literal `"\"x0,y0;x1,y1;...\""`.
pub fn format_manual_ortho_points(pts: &[[f64; 2]]) -> String {
    let joined: Vec<String> = pts
        .iter()
        .map(|p| format!("{},{}", format_clean_f64(p[0]), format_clean_f64(p[1])))
        .collect();
    format!("\"{}\"", joined.join(";"))
}

/// Re-anchor a user-customized multi-bend orthogonal polyline to updated `(new_b, new_e)` endpoints
/// while keeping every segment strictly horizontal/vertical and preserving all interior bends.
pub fn reanchor_manual_ortho_polyline(
    pts: &[[f64; 2]],
    new_b: [f64; 2],
    new_e: [f64; 2],
) -> Vec<[f64; 2]> {
    let m = pts.len();
    if m >= 4 {
        let mut out = pts.to_vec();
        let start_horiz = (pts[0][1] - pts[1][1]).abs() <= (pts[0][0] - pts[1][0]).abs();
        out[0] = new_b;
        if start_horiz {
            out[1][1] = new_b[1];
        } else {
            out[1][0] = new_b[0];
        }
        let end_horiz =
            (pts[m - 2][1] - pts[m - 1][1]).abs() <= (pts[m - 2][0] - pts[m - 1][0]).abs();
        out[m - 1] = new_e;
        if end_horiz {
            out[m - 2][1] = new_e[1];
        } else {
            out[m - 2][0] = new_e[0];
        }
        compact_orthogonal_polyline(&out)
    } else if m == 3 {
        let start_horiz = (pts[0][1] - pts[1][1]).abs() <= (pts[0][0] - pts[1][0]).abs();
        if start_horiz {
            compact_orthogonal_polyline(&[new_b, [new_e[0], new_b[1]], new_e])
        } else {
            compact_orthogonal_polyline(&[new_b, [new_b[0], new_e[1]], new_e])
        }
    } else if (new_b[0] - new_e[0]).abs() < 1e-5 || (new_b[1] - new_e[1]).abs() < 1e-5 {
        vec![new_b, new_e]
    } else {
        let mid_x = (new_b[0] + new_e[0]) * 0.5;
        compact_orthogonal_polyline(&[new_b, [mid_x, new_b[1]], [mid_x, new_e[1]], new_e])
    }
}

/// Slide segment `seg_index` (`0 .. poly.len() - 1`) of an orthogonal polyline perpendicular to its
/// axis to `coord_in` (`X` for vertical segments, `Y` for horizontal segments), inserting clean
/// orthogonal dogleg stubs automatically when `seg_index` touches the `Begin` or `End` anchor!
pub fn slide_orthogonal_segment(
    poly: &[[f64; 2]],
    seg_index: usize,
    coord_in: f64,
) -> Vec<[f64; 2]> {
    if poly.len() < 2 || !coord_in.is_finite() {
        return poly.to_vec();
    }
    let n = poly.len();
    let s = seg_index.min(n - 2);
    let a = poly[s];
    let b = poly[s + 1];
    let is_vert = (a[0] - b[0]).abs() <= (a[1] - b[1]).abs();

    if n == 2 {
        let bx = poly[0][0];
        let by = poly[0][1];
        let ex = poly[1][0];
        let ey = poly[1][1];
        return if is_vert {
            compact_orthogonal_polyline(&[[bx, by], [coord_in, by], [coord_in, ey], [ex, ey]])
        } else {
            compact_orthogonal_polyline(&[[bx, by], [bx, coord_in], [ex, coord_in], [ex, ey]])
        };
    }

    if s == 0 {
        let mut out = Vec::with_capacity(n + 1);
        out.push(poly[0]);
        if is_vert {
            out.push([coord_in, poly[0][1]]);
            out.push([coord_in, poly[1][1]]);
        } else {
            out.push([poly[0][0], coord_in]);
            out.push([poly[1][0], coord_in]);
        }
        out.extend_from_slice(&poly[2..]);
        return compact_orthogonal_polyline(&out);
    }

    if s + 2 == n {
        let mut out = Vec::with_capacity(n + 1);
        out.extend_from_slice(&poly[..n - 2]);
        let end_pt = poly[n - 1];
        if is_vert {
            out.push([coord_in, poly[n - 2][1]]);
            out.push([coord_in, end_pt[1]]);
        } else {
            out.push([poly[n - 2][0], coord_in]);
            out.push([end_pt[0], coord_in]);
        }
        out.push(end_pt);
        return compact_orthogonal_polyline(&out);
    }

    let mut out = poly.to_vec();
    if is_vert {
        out[s][0] = coord_in;
        out[s + 1][0] = coord_in;
    } else {
        out[s][1] = coord_in;
        out[s + 1][1] = coord_in;
    }
    compact_orthogonal_polyline(&out)
}

fn resolve_glue_point(
    page: &VisioPage,
    masters: &[vexio_shapesheet::VisioMaster],
    target_id: u64,
    world_tf: Option<vexio_shapesheet::ShapeTransform2D>,
    to_cell: &str,
    is_dynamic: bool,
) -> Option<[f64; 2]> {
    let sh = page.find_shape(target_id)?;
    let master_shape = sh
        .master_id
        .and_then(|mid| masters.iter().find(|m| m.id == mid))
        .and_then(|m| {
            if let Some(msid) = sh.master_shape_id {
                m.shapes.iter().find(|s| s.id == msid).or_else(|| m.shapes.first())
            } else {
                m.shapes.first()
            }
        });
    let local_w = sh
        .sheet
        .get_f64("Width")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("Width")))
        .unwrap_or(1.0);
    let local_h = sh
        .sheet
        .get_f64("Height")
        .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64("Height")))
        .unwrap_or(1.0);
    let tf = world_tf.unwrap_or_else(|| {
        let px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
        let py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
        let lpx = sh.sheet.get_f64("LocPinX").unwrap_or(local_w * 0.5);
        let lpy = sh.sheet.get_f64("LocPinY").unwrap_or(local_h * 0.5);
        let ang = sh.sheet.get_f64("Angle").unwrap_or(0.0);
        let flip_x = sh.sheet.get_bool("FlipX").unwrap_or(false);
        let flip_y = sh.sheet.get_bool("FlipY").unwrap_or(false);
        vexio_shapesheet::ShapeTransform2D {
            pin_x: px,
            pin_y: py,
            width: local_w,
            height: local_h,
            loc_pin_x: lpx,
            loc_pin_y: lpy,
            angle_rad: ang,
            flip_x,
            flip_y,
        }
    });
    let w = tf.width;
    let h = tf.height;
    let px = tf.pin_x;
    let py = tf.pin_y;
    let sx = if local_w.abs() > 1e-6 { w / local_w } else { 1.0 };
    let sy = if local_h.abs() > 1e-6 { h / local_h } else { 1.0 };

    if !is_dynamic {
        // Check explicit ConnectionPoint cell (`Connections.X1` -> `(Connections.X1, Connections.Y1)`)
        let lower = to_cell.to_ascii_lowercase();
        let suffix_opt = lower
            .strip_prefix("connections.x")
            .or_else(|| lower.strip_prefix("connections.y"));
        if let Some(suffix) = suffix_opt {
            let xk = format!("Connections.X{suffix}");
            let yk = format!("Connections.Y{suffix}");
            let cx_opt = sh
                .sheet
                .get_f64(&xk)
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(&xk)));
            let cy_opt = sh
                .sheet
                .get_f64(&yk)
                .or_else(|| master_shape.and_then(|ms| ms.sheet.get_f64(&yk)));
            if let (Some(cx), Some(cy)) = (cx_opt, cy_opt) {
                let (wx, wy) = tf.local_to_parent(cx * sx, cy * sy);
                return Some([wx, wy]);
            }
            // Fallback cardinal ports `1..=4` matching `EvaluatedShapeVisual::connection_points`
            if let Ok(ix) = suffix.parse::<u32>() {
                let cardinal = match ix {
                    1 => Some((w * 0.5, 0.0)),
                    2 => Some((w * 0.5, h)),
                    3 => Some((0.0, h * 0.5)),
                    4 => Some((w, h * 0.5)),
                    _ => None,
                };
                if let Some((lx, ly)) = cardinal {
                    let (wx, wy) = tf.local_to_parent(lx, ly);
                    return Some([wx, wy]);
                }
            }
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
        let Some(&prev) = out.last() else {
            break;
        };
        let cur = pts[i];
        let next = pts[i + 1];
        let same_x = (prev[0] - cur[0]).abs() < 1e-6 && (cur[0] - next[0]).abs() < 1e-6;
        let same_y = (prev[1] - cur[1]).abs() < 1e-6 && (cur[1] - next[1]).abs() < 1e-6;
        if !same_x && !same_y {
            out.push(cur);
        }
    }
    if let Some(&last) = pts.last() {
        out.push(last);
    }
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

fn collect_horizontal_segments(
    polylines: &[(u64, ConnectorRouteMode, Vec<[f64; 2]>)],
) -> Vec<(u64, f64, f64, f64)> {
    let mut out = Vec::new();
    for (id, _, pts) in polylines {
        for seg in pts.windows(2) {
            if (seg[0][1] - seg[1][1]).abs() < 1e-6 && (seg[0][0] - seg[1][0]).abs() > 1e-4 {
                let x_min = seg[0][0].min(seg[1][0]);
                let x_max = seg[0][0].max(seg[1][0]);
                out.push((*id, seg[0][1], x_min, x_max));
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn find_crossings_on_horizontal_filtered(
    own_id: u64,
    own_z: usize,
    only_higher_z: bool,
    x0: f64,
    x1: f64,
    y: f64,
    verticals: &[(u64, f64, f64, f64)],
    z_ranks: &BTreeMap<u64, usize>,
) -> Vec<f64> {
    let min_x = x0.min(x1) + 0.12;
    let max_x = x0.max(x1) - 0.12;
    if min_x >= max_x {
        return Vec::new();
    }
    let mut xs = Vec::new();
    for &(vid, vx, vy0, vy1) in verticals {
        if vid != own_id && vx > min_x && vx < max_x && y > vy0 + 0.05 && y < vy1 - 0.05 {
            if only_higher_z && own_z <= z_ranks.get(&vid).copied().unwrap_or(0) {
                continue;
            }
            xs.push(vx);
        }
    }
    dedup_sorted_f64(&mut xs);
    xs
}

#[allow(clippy::too_many_arguments)]
fn find_crossings_on_vertical_filtered(
    own_id: u64,
    own_z: usize,
    only_higher_z: bool,
    y0: f64,
    y1: f64,
    x: f64,
    horizontals: &[(u64, f64, f64, f64)],
    z_ranks: &BTreeMap<u64, usize>,
) -> Vec<f64> {
    let min_y = y0.min(y1) + 0.12;
    let max_y = y0.max(y1) - 0.12;
    if min_y >= max_y {
        return Vec::new();
    }
    let mut ys = Vec::new();
    for &(hid, hy, hx0, hx1) in horizontals {
        if hid != own_id && hy > min_y && hy < max_y && x > hx0 + 0.05 && x < hx1 - 0.05 {
            if only_higher_z && own_z <= z_ranks.get(&hid).copied().unwrap_or(0) {
                continue;
            }
            ys.push(hy);
        }
    }
    dedup_sorted_f64(&mut ys);
    ys
}

#[allow(clippy::too_many_arguments)]
fn emit_line_jump_rows(
    rows: &mut Vec<GeometryRow>,
    ix: &mut u32,
    style: LineJumpStyle,
    pre: [f64; 2],
    post: [f64; 2],
    ortho_off: [f64; 2],
    arc_bow: f64,
    origin: [f64; 2],
) {
    let fmt_pt = |pt: [f64; 2]| -> (String, String) {
        (
            format!("{} in", format_clean_f64(pt[0] - origin[0])),
            format!("{} in", format_clean_f64(pt[1] - origin[1])),
        )
    };
    let (pre_sx, pre_sy) = fmt_pt(pre);
    rows.push(GeometryRow::line_to(*ix, pre_sx, pre_sy));
    *ix += 1;

    match style {
        LineJumpStyle::None => {}
        LineJumpStyle::Arc => {
            let (post_sx, post_sy) = fmt_pt(post);
            rows.push(GeometryRow::arc_to(
                *ix,
                post_sx,
                post_sy,
                format!("{} in", format_clean_f64(arc_bow)),
            ));
            *ix += 1;
        }
        LineJumpStyle::Gap => {
            let (post_sx, post_sy) = fmt_pt(post);
            rows.push(GeometryRow::move_to(*ix, post_sx, post_sy));
            *ix += 1;
        }
        LineJumpStyle::Square => {
            for pt in [
                [pre[0] + ortho_off[0], pre[1] + ortho_off[1]],
                [post[0] + ortho_off[0], post[1] + ortho_off[1]],
                post,
            ] {
                let (sx, sy) = fmt_pt(pt);
                rows.push(GeometryRow::line_to(*ix, sx, sy));
                *ix += 1;
            }
        }
        LineJumpStyle::TwoSides => {
            let mid = [
                (pre[0] + post[0]) * 0.5 + ortho_off[0],
                (pre[1] + post[1]) * 0.5 + ortho_off[1],
            ];
            for pt in [mid, post] {
                let (sx, sy) = fmt_pt(pt);
                rows.push(GeometryRow::line_to(*ix, sx, sy));
                *ix += 1;
            }
        }
        LineJumpStyle::ThreeSides => {
            let p_a = [
                pre[0] + (post[0] - pre[0]) * 0.25 + ortho_off[0],
                pre[1] + (post[1] - pre[1]) * 0.25 + ortho_off[1],
            ];
            let p_b = [
                pre[0] + (post[0] - pre[0]) * 0.75 + ortho_off[0],
                pre[1] + (post[1] - pre[1]) * 0.75 + ortho_off[1],
            ];
            for pt in [p_a, p_b, post] {
                let (sx, sy) = fmt_pt(pt);
                rows.push(GeometryRow::line_to(*ix, sx, sy));
                *ix += 1;
            }
        }
        LineJumpStyle::FourSides | LineJumpStyle::FiveSides => {
            let segments = if style == LineJumpStyle::FourSides { 4 } else { 5 };
            for step in 1..=segments {
                let theta = std::f64::consts::PI * (step as f64) / (segments as f64);
                let u = (1.0 - theta.cos()) * 0.5;
                let h = theta.sin();
                let pt = [
                    pre[0] + (post[0] - pre[0]) * u + ortho_off[0] * h,
                    pre[1] + (post[1] - pre[1]) * u + ortho_off[1] * h,
                ];
                let (sx, sy) = fmt_pt(pt);
                rows.push(GeometryRow::line_to(*ix, sx, sy));
                *ix += 1;
            }
        }
    }
}
