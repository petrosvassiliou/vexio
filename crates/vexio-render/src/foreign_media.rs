//! Offline pure-Rust vector & raster `ForeignData` media renderer (`SVG`, `PNG`, `BMP`, `EMF`).
//!
//! Converts embedded `.vsdx` `<ForeignData>` payloads (such as Azure / Microsoft 365 / Entra
//! stencil SVG icons, PNG/BMP company logos, and classic Visio EMF metafiles) directly into
//! vertex-colored `TessellatedMesh` geometry in world page inches (`100%` WASM + Native compatible).

use crate::tessellate::{GpuVertex, TessellatedMesh, PRIM_FLAG_SHAPE};
use lyon_path::math::point;
use lyon_path::Path;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule, FillTessellator, FillVertex, LineCap, LineJoin,
    StrokeOptions, StrokeTessellator, StrokeVertex, VertexBuffers,
};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use vexio_shapesheet::{ForeignMediaData, RgbaColor, ShapeTransform2D};

/// Tessellate an embedded `ForeignMediaData` payload into `world_tf` (`[0..width] x [0..height]` local space).
pub fn tessellate_foreign_media(
    media: &ForeignMediaData,
    world_tf: &ShapeTransform2D,
    tol_in: f32,
) -> TessellatedMesh {
    if media.data.is_empty()
        || !world_tf.pin_x.is_finite()
        || !world_tf.pin_y.is_finite()
        || !(world_tf.width.is_finite() && world_tf.width > 1e-6)
        || !(world_tf.height.is_finite() && world_tf.height > 1e-6)
        || !world_tf.loc_pin_x.is_finite()
        || !world_tf.loc_pin_y.is_finite()
        || !world_tf.angle_rad.is_finite()
    {
        return TessellatedMesh::new();
    }
    let mt = media.media_type.to_ascii_uppercase();
    if mt.contains("SVG")
        || media.data.starts_with(b"<svg")
        || (media.data.starts_with(b"<?xml") && media.data.windows(4).any(|w| w == b"<svg"))
    {
        if let Ok(svg_str) = std::str::from_utf8(&media.data) {
            let mesh = tessellate_svg_foreign(svg_str, world_tf, tol_in);
            if !mesh.is_empty() {
                return mesh;
            }
        }
    }

    if mt.contains("PNG") || media.data.starts_with(b"\x89PNG\r\n\x1a\n") {
        if let Some((w, h, rgba)) = decode_png_rgba8(&media.data) {
            return emit_rgba_grid_in_transform(w, h, &rgba, world_tf);
        }
    }

    if mt.contains("BMP") || media.data.starts_with(b"BM") {
        if let Some((w, h, rgba)) = decode_bmp_rgba8(&media.data) {
            return emit_rgba_grid_in_transform(w, h, &rgba, world_tf);
        }
    }

    if mt.contains("EMF") || (media.data.len() >= 44 && &media.data[40..44] == b" EMF") {
        let mesh = tessellate_emf_foreign(&media.data, world_tf, tol_in);
        if !mesh.is_empty() {
            return mesh;
        }
    }

    TessellatedMesh::new()
}

// ============================================================================
// 1. Embedded SVG Vector Icon Tessellator
// ============================================================================

#[derive(Clone, Debug)]
struct SvgStyleState {
    fill: Option<RgbaColor>,
    stroke: Option<RgbaColor>,
    stroke_width: f64,
    opacity: f64,
    tx: f64,
    ty: f64,
    sx: f64,
    sy: f64,
}

impl Default for SvgStyleState {
    fn default() -> Self {
        Self {
            fill: Some(RgbaColor::BLACK),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            tx: 0.0,
            ty: 0.0,
            sx: 1.0,
            sy: 1.0,
        }
    }
}

fn tessellate_svg_foreign(svg: &str, world_tf: &ShapeTransform2D, tol_in: f32) -> TessellatedMesh {
    let mut reader = Reader::from_str(svg);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut vb_x = 0.0f64;
    let mut vb_y = 0.0f64;
    let mut vb_w = 64.0f64;
    let mut vb_h = 64.0f64;
    let mut got_vb = false;

    let mut stack: Vec<SvgStyleState> = vec![SvgStyleState::default()];
    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if tag == "svg" && !got_vb {
                    parse_svg_root_viewbox(&e, &mut vb_x, &mut vb_y, &mut vb_w, &mut vb_h);
                    got_vb = true;
                }
                let next_state = derive_svg_state(stack.last().unwrap(), &e);
                emit_svg_shape_elem(
                    &tag,
                    &e,
                    &next_state,
                    (vb_x, vb_y, vb_w.max(1e-3), vb_h.max(1e-3)),
                    world_tf,
                    tol_in,
                    &mut fill_tess,
                    &mut stroke_tess,
                    &mut lyon_buf,
                );
                stack.push(next_state);
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if tag == "svg" && !got_vb {
                    parse_svg_root_viewbox(&e, &mut vb_x, &mut vb_y, &mut vb_w, &mut vb_h);
                    got_vb = true;
                }
                let st = derive_svg_state(stack.last().unwrap(), &e);
                emit_svg_shape_elem(
                    &tag,
                    &e,
                    &st,
                    (vb_x, vb_y, vb_w.max(1e-3), vb_h.max(1e-3)),
                    world_tf,
                    tol_in,
                    &mut fill_tess,
                    &mut stroke_tess,
                    &mut lyon_buf,
                );
            }
            Ok(Event::End(_)) => {
                if stack.len() > 1 {
                    stack.pop();
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    }
}

fn attr_val(e: &BytesStart<'_>, key: &[u8]) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.as_ref() == key || a.key.local_name().as_ref() == key {
            if let Ok(v) = a.unescape_value() {
                return Some(v.into_owned());
            }
        }
    }
    None
}

fn parse_svg_num(s: &str) -> Option<f64> {
    let trimmed = s
        .trim()
        .trim_end_matches("px")
        .trim_end_matches("pt")
        .trim_end_matches('%')
        .trim();
    trimmed.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn parse_svg_root_viewbox(
    e: &BytesStart<'_>,
    vb_x: &mut f64,
    vb_y: &mut f64,
    vb_w: &mut f64,
    vb_h: &mut f64,
) {
    if let Some(vb) = attr_val(e, b"viewBox") {
        let nums: Vec<f64> = vb
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();
        if nums.len() == 4 && nums[2] > 0.0 && nums[3] > 0.0 {
            *vb_x = nums[0];
            *vb_y = nums[1];
            *vb_w = nums[2];
            *vb_h = nums[3];
            return;
        }
    }
    if let Some(w) = attr_val(e, b"width").and_then(|s| parse_svg_num(&s)) {
        if w > 0.0 {
            *vb_w = w;
        }
    }
    if let Some(h) = attr_val(e, b"height").and_then(|s| parse_svg_num(&s)) {
        if h > 0.0 {
            *vb_h = h;
        }
    }
}

fn parse_svg_paint(raw: &str) -> Option<Option<RgbaColor>> {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("none") || t.eq_ignore_ascii_case("transparent") {
        return Some(None);
    }
    if let Some(c) = RgbaColor::from_hex(t) {
        return Some(Some(c));
    }
    if let Some(inner) = t.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<u8> = inner
            .split(',')
            .filter_map(|p| p.trim().parse::<f64>().ok().map(|v| v.clamp(0.0, 255.0) as u8))
            .collect();
        if parts.len() == 3 {
            return Some(Some(RgbaColor {
                r: parts[0],
                g: parts[1],
                b: parts[2],
                a: 255,
            }));
        }
    }
    let named = match t.to_ascii_lowercase().as_str() {
        "black" => Some((0, 0, 0)),
        "white" => Some((255, 255, 255)),
        "red" => Some((220, 38, 38)),
        "blue" => Some((0, 120, 212)),
        "green" => Some((22, 163, 74)),
        "orange" => Some((234, 88, 12)),
        "purple" => Some((124, 58, 237)),
        "gray" | "grey" => Some((115, 115, 115)),
        "cyan" | "teal" => Some((8, 145, 178)),
        "yellow" | "gold" => Some((234, 179, 8)),
        _ => None,
    };
    named.map(|(r, g, b)| Some(RgbaColor { r, g, b, a: 255 }))
}

fn derive_svg_state(parent: &SvgStyleState, e: &BytesStart<'_>) -> SvgStyleState {
    let mut st = parent.clone();
    if let Some(style_str) = attr_val(e, b"style") {
        for decl in style_str.split(';') {
            let Some((k, v)) = decl.split_once(':') else {
                continue;
            };
            apply_svg_prop(&mut st, k.trim(), v.trim());
        }
    }
    for key in [b"fill".as_slice(), b"stroke", b"stroke-width", b"opacity", b"fill-opacity"] {
        if let Some(val) = attr_val(e, key) {
            let ks = std::str::from_utf8(key).unwrap_or("");
            apply_svg_prop(&mut st, ks, &val);
        }
    }
    if let Some(tf) = attr_val(e, b"transform") {
        let lower = tf.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("translate(").and_then(|s| s.strip_suffix(')')) {
            let nums: Vec<f64> = rest
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse::<f64>().ok())
                .collect();
            if let Some(&dx) = nums.first() {
                st.tx += dx * st.sx;
                st.ty += nums.get(1).copied().unwrap_or(0.0) * st.sy;
            }
        } else if let Some(rest) = lower.strip_prefix("scale(").and_then(|s| s.strip_suffix(')')) {
            let nums: Vec<f64> = rest
                .split(|c: char| c.is_whitespace() || c == ',')
                .filter(|t| !t.is_empty())
                .filter_map(|t| t.parse::<f64>().ok())
                .collect();
            if let Some(&sx) = nums.first() {
                st.sx *= sx;
                st.sy *= nums.get(1).copied().unwrap_or(sx);
            }
        }
    }
    st
}

fn apply_svg_prop(st: &mut SvgStyleState, k: &str, v: &str) {
    match k.to_ascii_lowercase().as_str() {
        "fill" => {
            if let Some(p) = parse_svg_paint(v) {
                st.fill = p;
            }
        }
        "stroke" => {
            if let Some(p) = parse_svg_paint(v) {
                st.stroke = p;
            }
        }
        "stroke-width" => {
            if let Some(w) = parse_svg_num(v) {
                st.stroke_width = w.max(0.1);
            }
        }
        "opacity" | "fill-opacity" => {
            if let Some(op) = parse_svg_num(v) {
                st.opacity = (st.opacity * op).clamp(0.0, 1.0);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_svg_shape_elem(
    tag: &str,
    e: &BytesStart<'_>,
    st: &SvgStyleState,
    vb: (f64, f64, f64, f64),
    world_tf: &ShapeTransform2D,
    tol_in: f32,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
) {
    let fallback_pt = point(world_tf.pin_x as f32, world_tf.pin_y as f32);
    let map_svg = |sx: f64, sy: f64| -> lyon_path::math::Point {
        if !sx.is_finite() || !sy.is_finite() || !vb.2.is_finite() || !vb.3.is_finite() || vb.2.abs() < 1e-9 || vb.3.abs() < 1e-9 {
            return fallback_pt;
        }
        let tx = st.tx + sx * st.sx;
        let ty = st.ty + sy * st.sy;
        let nx = (tx - vb.0) / vb.2;
        // SVG Y points down (`0` = top, `vb.3` = bottom); Visio local Y points up (`height` = top, `0` = bottom)!
        let ny = 1.0 - (ty - vb.1) / vb.3;
        let (wx, wy) = world_tf.local_to_parent(nx * world_tf.width, ny * world_tf.height);
        let fx = wx as f32;
        let fy = wy as f32;
        if fx.is_finite() && fy.is_finite() {
            point(fx, fy)
        } else {
            fallback_pt
        }
    };

    let mut b = Path::builder();
    let mut has_geom = false;

    match tag {
        "rect" => {
            let x = attr_val(e, b"x").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let y = attr_val(e, b"y").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let w = attr_val(e, b"width").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let h = attr_val(e, b"height").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 {
                b.begin(map_svg(x, y));
                b.line_to(map_svg(x + w, y));
                b.line_to(map_svg(x + w, y + h));
                b.line_to(map_svg(x, y + h));
                b.end(true);
                has_geom = true;
            }
        }
        "circle" | "ellipse" => {
            let cx = attr_val(e, b"cx").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let cy = attr_val(e, b"cy").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let (rx, ry) = if tag == "circle" {
                let r = attr_val(e, b"r").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
                (r, r)
            } else {
                (
                    attr_val(e, b"rx").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0),
                    attr_val(e, b"ry").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0),
                )
            };
            if rx.is_finite() && ry.is_finite() && rx > 0.0 && ry > 0.0 {
                const STEPS: usize = 24;
                b.begin(map_svg(cx + rx, cy));
                for i in 1..STEPS {
                    let ang = std::f64::consts::TAU * (i as f64 / STEPS as f64);
                    b.line_to(map_svg(cx + rx * ang.cos(), cy + ry * ang.sin()));
                }
                b.end(true);
                has_geom = true;
            }
        }
        "line" => {
            let x1 = attr_val(e, b"x1").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let y1 = attr_val(e, b"y1").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let x2 = attr_val(e, b"x2").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            let y2 = attr_val(e, b"y2").and_then(|s| parse_svg_num(&s)).unwrap_or(0.0);
            b.begin(map_svg(x1, y1));
            b.line_to(map_svg(x2, y2));
            b.end(false);
            has_geom = true;
        }
        "polygon" | "polyline" => {
            if let Some(pts_str) = attr_val(e, b"points") {
                let nums: Vec<f64> = pts_str
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|t| !t.is_empty())
                    .filter_map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()))
                    .collect();
                if nums.len() >= 4 {
                    b.begin(map_svg(nums[0], nums[1]));
                    for pair in nums[2..].chunks_exact(2) {
                        b.line_to(map_svg(pair[0], pair[1]));
                    }
                    b.end(tag == "polygon");
                    has_geom = true;
                }
            }
        }
        "path" => {
            if let Some(d) = attr_val(e, b"d") {
                has_geom = parse_svg_path_d(&d, &map_svg, &mut b);
            }
        }
        _ => {}
    }

    if !has_geom {
        return;
    }
    let path = b.build();

    if tag != "line" && tag != "polyline" {
        if let Some(mut fc) = st.fill {
            fc.a = ((fc.a as f64) * st.opacity).round().clamp(0.0, 255.0) as u8;
            if fc.a > 0 {
                let rgba = fc.to_linear_f32();
                let _ = fill_tess.tessellate_path(
                    &path,
                    &FillOptions::tolerance(tol_in).with_fill_rule(FillRule::EvenOdd),
                    &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                        let p = v.position();
                        GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            }
        }
    }

    if let Some(mut sc) = st.stroke {
        sc.a = ((sc.a as f64) * st.opacity).round().clamp(0.0, 255.0) as u8;
        if sc.a > 0 {
            let rgba = sc.to_linear_f32();
            let scale_in = (world_tf.width / vb.2.max(1e-6))
                .min(world_tf.height / vb.3.max(1e-6))
                .max(0.002);
            let raw_sw = st.stroke_width * st.sx.abs().max(0.5) * scale_in;
            let sw = if raw_sw.is_finite() {
                (raw_sw as f32).clamp(0.006, 0.25)
            } else {
                0.015
            };
            let _ = stroke_tess.tessellate_path(
                &path,
                &StrokeOptions::tolerance(tol_in)
                    .with_line_width(sw)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round),
                &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
                    let p = v.position();
                    GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                }),
            );
        }
    }
}

fn tokenize_svg_path(d: &str) -> Vec<Result<char, f64>> {
    let mut tokens = Vec::new();
    let bytes = d.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if c.is_ascii_whitespace() || c == ',' {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() {
            tokens.push(Ok(c));
            i += 1;
            continue;
        }
        let start = i;
        if c == '+' || c == '-' {
            i += 1;
        }
        let mut saw_dot = false;
        while i < bytes.len() {
            let ch = bytes[i] as char;
            if ch.is_ascii_digit() {
                i += 1;
            } else if ch == '.' && !saw_dot {
                saw_dot = true;
                i += 1;
            } else if (ch == 'e' || ch == 'E') && i > start {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                    i += 1;
                }
            } else {
                break;
            }
        }
        if i > start {
            if let Ok(num) = d[start..i].parse::<f64>() {
                tokens.push(Err(num));
            }
        } else {
            i += 1;
        }
    }
    tokens
}

fn parse_svg_path_d<F>(d: &str, map_svg: &F, b: &mut lyon_path::path::Builder) -> bool
where
    F: Fn(f64, f64) -> lyon_path::math::Point,
{
    let tokens = tokenize_svg_path(d);
    let mut idx = 0usize;
    let mut cmd = 'M';
    let mut cur = [0.0f64, 0.0f64];
    let mut sub_start = [0.0f64, 0.0f64];
    let mut last_ctrl = [0.0f64, 0.0f64];
    let mut in_sub = false;
    let mut any = false;

    let next_num = |idx: &mut usize| -> Option<f64> {
        match tokens.get(*idx) {
            Some(Err(v)) => {
                *idx += 1;
                Some(*v)
            }
            _ => None,
        }
    };

    while idx < tokens.len() {
        if let Ok(c) = tokens[idx] {
            cmd = c;
            idx += 1;
        }
        match cmd {
            'M' | 'm' => {
                let Some(x) = next_num(&mut idx) else { break };
                let Some(y) = next_num(&mut idx) else { break };
                if in_sub {
                    b.end(false);
                }
                cur = if cmd == 'm' { [cur[0] + x, cur[1] + y] } else { [x, y] };
                sub_start = cur;
                last_ctrl = cur;
                b.begin(map_svg(cur[0], cur[1]));
                in_sub = true;
                any = true;
                cmd = if cmd == 'm' { 'l' } else { 'L' };
            }
            'L' | 'l' => {
                let Some(x) = next_num(&mut idx) else { break };
                let Some(y) = next_num(&mut idx) else { break };
                cur = if cmd == 'l' { [cur[0] + x, cur[1] + y] } else { [x, y] };
                last_ctrl = cur;
                if !in_sub {
                    b.begin(map_svg(cur[0], cur[1]));
                    sub_start = cur;
                    in_sub = true;
                } else {
                    b.line_to(map_svg(cur[0], cur[1]));
                }
                any = true;
            }
            'H' | 'h' => {
                let Some(x) = next_num(&mut idx) else { break };
                cur[0] = if cmd == 'h' { cur[0] + x } else { x };
                last_ctrl = cur;
                if in_sub {
                    b.line_to(map_svg(cur[0], cur[1]));
                }
            }
            'V' | 'v' => {
                let Some(y) = next_num(&mut idx) else { break };
                cur[1] = if cmd == 'v' { cur[1] + y } else { y };
                last_ctrl = cur;
                if in_sub {
                    b.line_to(map_svg(cur[0], cur[1]));
                }
            }
            'C' | 'c' => {
                let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let (c1, c2, end) = if cmd == 'c' {
                    (
                        [cur[0] + x1, cur[1] + y1],
                        [cur[0] + x2, cur[1] + y2],
                        [cur[0] + x, cur[1] + y],
                    )
                } else {
                    ([x1, y1], [x2, y2], [x, y])
                };
                if in_sub {
                    b.cubic_bezier_to(map_svg(c1[0], c1[1]), map_svg(c2[0], c2[1]), map_svg(end[0], end[1]));
                }
                last_ctrl = c2;
                cur = end;
            }
            'S' | 's' => {
                let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let c1 = [2.0 * cur[0] - last_ctrl[0], 2.0 * cur[1] - last_ctrl[1]];
                let (c2, end) = if cmd == 's' {
                    ([cur[0] + x2, cur[1] + y2], [cur[0] + x, cur[1] + y])
                } else {
                    ([x2, y2], [x, y])
                };
                if in_sub {
                    b.cubic_bezier_to(map_svg(c1[0], c1[1]), map_svg(c2[0], c2[1]), map_svg(end[0], end[1]));
                }
                last_ctrl = c2;
                cur = end;
            }
            'Q' | 'q' => {
                let (Some(cx), Some(cy), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let (ctrl, end) = if cmd == 'q' {
                    ([cur[0] + cx, cur[1] + cy], [cur[0] + x, cur[1] + y])
                } else {
                    ([cx, cy], [x, y])
                };
                if in_sub {
                    b.quadratic_bezier_to(map_svg(ctrl[0], ctrl[1]), map_svg(end[0], end[1]));
                }
                last_ctrl = ctrl;
                cur = end;
            }
            'T' | 't' => {
                let (Some(x), Some(y)) = (next_num(&mut idx), next_num(&mut idx)) else {
                    break;
                };
                let ctrl = [2.0 * cur[0] - last_ctrl[0], 2.0 * cur[1] - last_ctrl[1]];
                let end = if cmd == 't' { [cur[0] + x, cur[1] + y] } else { [x, y] };
                if in_sub {
                    b.quadratic_bezier_to(map_svg(ctrl[0], ctrl[1]), map_svg(end[0], end[1]));
                }
                last_ctrl = ctrl;
                cur = end;
            }
            'A' | 'a' => {
                let (Some(rx), Some(ry), Some(rot_deg), Some(large_f), Some(sweep_f), Some(x), Some(y)) = (
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                    next_num(&mut idx),
                ) else {
                    break;
                };
                let end = if cmd == 'a' { [cur[0] + x, cur[1] + y] } else { [x, y] };
                if in_sub {
                    emit_svg_arc_segments(
                        cur,
                        end,
                        rx.abs(),
                        ry.abs(),
                        rot_deg.to_radians(),
                        large_f.abs() > 0.5,
                        sweep_f.abs() > 0.5,
                        map_svg,
                        b,
                    );
                }
                last_ctrl = end;
                cur = end;
            }
            'Z' | 'z' => {
                if in_sub {
                    b.end(true);
                    in_sub = false;
                }
                cur = sub_start;
                last_ctrl = cur;
            }
            _ => break,
        }
    }
    if in_sub {
        b.end(false);
    }
    any
}

#[allow(clippy::too_many_arguments)]
fn emit_svg_arc_segments<F>(
    p0: [f64; 2],
    p1: [f64; 2],
    mut rx: f64,
    mut ry: f64,
    phi: f64,
    large_arc: bool,
    sweep: bool,
    map_svg: &F,
    b: &mut lyon_path::path::Builder,
) where
    F: Fn(f64, f64) -> lyon_path::math::Point,
{
    if rx < 1e-6 || ry < 1e-6 || (p0[0] - p1[0]).hypot(p0[1] - p1[1]) < 1e-6 {
        b.line_to(map_svg(p1[0], p1[1]));
        return;
    }
    let (sin_p, cos_p) = phi.sin_cos();
    let dx2 = (p0[0] - p1[0]) * 0.5;
    let dy2 = (p0[1] - p1[1]) * 0.5;
    let x1p = cos_p * dx2 + sin_p * dy2;
    let y1p = -sin_p * dx2 + cos_p * dy2;

    let lam = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lam > 1.0 {
        let s = lam.sqrt();
        rx *= s;
        ry *= s;
    }
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let num = (rx2 * ry2 - rx2 * y1p * y1p - ry2 * x1p * x1p).max(0.0);
    let den = (rx2 * y1p * y1p + ry2 * x1p * x1p).max(1e-12);
    let sign = if large_arc == sweep { -1.0 } else { 1.0 };
    let coef = sign * (num / den).sqrt();
    let cxp = coef * (rx * y1p / ry);
    let cyp = coef * (-ry * x1p / rx);
    let cx = cos_p * cxp - sin_p * cyp + (p0[0] + p1[0]) * 0.5;
    let cy = sin_p * cxp + cos_p * cyp + (p0[1] + p1[1]) * 0.5;

    let vec_ang = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let det = ux * vy - uy * vx;
        det.atan2(dot)
    };
    let ux = (x1p - cxp) / rx;
    let uy = (y1p - cyp) / ry;
    let vx = (-x1p - cxp) / rx;
    let vy = (-y1p - cyp) / ry;
    let theta1 = vec_ang(1.0, 0.0, ux, uy);
    let mut dtheta = vec_ang(ux, uy, vx, vy);
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }

    let steps = ((dtheta.abs() / (std::f64::consts::PI / 8.0)).ceil() as usize).clamp(2, 24);
    for s in 1..=steps {
        let t = theta1 + dtheta * (s as f64 / steps as f64);
        let (st, ct) = t.sin_cos();
        let px = cx + cos_p * rx * ct - sin_p * ry * st;
        let py = cy + sin_p * rx * ct + cos_p * ry * st;
        b.line_to(map_svg(px, py));
    }
}

// ============================================================================
// 2. Pure-Rust PNG & BMP Decoder + Run-Coalesced Color Grid Tessellator
// ============================================================================

fn decode_png_rgba8(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.len() < 33 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return None;
    }
    let mut pos = 8usize;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut interlace = 0u8;
    let mut palette: Vec<[u8; 3]> = Vec::new();
    let mut trns: Vec<u8> = Vec::new();
    let mut idat = Vec::new();

    while pos + 12 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]]) as usize;
        let ctype = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = data_start.checked_add(len)?;
        if data_end + 4 > bytes.len() {
            break;
        }
        let chunk = &bytes[data_start..data_end];
        match ctype {
            b"IHDR" if chunk.len() >= 13 => {
                width = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                height = u32::from_be_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                bit_depth = chunk[8];
                color_type = chunk[9];
                interlace = chunk[12];
            }
            b"PLTE" => {
                for rgb in chunk.chunks_exact(3) {
                    palette.push([rgb[0], rgb[1], rgb[2]]);
                }
            }
            b"tRNS" => {
                trns.extend_from_slice(chunk);
            }
            b"IDAT" => {
                idat.extend_from_slice(chunk);
            }
            b"IEND" => break,
            _ => {}
        }
        pos = data_end + 4;
    }

    if width == 0 || height == 0 || width > 4096 || height > 4096 || interlace != 0 {
        return None;
    }
    let channels: usize = match color_type {
        0 => 1,
        2 => 3,
        3 => 1,
        4 => 2,
        6 => 4,
        _ => return None,
    };
    if !matches!(bit_depth, 8 | 16) {
        return None;
    }
    let bytes_per_sample = (bit_depth / 8) as usize;
    let bpp = channels * bytes_per_sample;
    let stride = (width as usize).checked_mul(bpp)?;
    let row_bytes = stride.checked_add(1)?;
    let expected_raw_len = (height as usize).checked_mul(row_bytes)?;
    let raw = miniz_oxide::inflate::decompress_to_vec_zlib(&idat).ok()?;
    if raw.len() < expected_raw_len {
        return None;
    }

    let mut prev_row = vec![0u8; stride];
    let mut cur_row = vec![0u8; stride];
    let mut rgba = Vec::with_capacity((width as usize) * (height as usize) * 4);

    for y in 0..(height as usize) {
        let row_off = y * row_bytes;
        let filter = raw[row_off];
        let src = &raw[row_off + 1..row_off + 1 + stride];
        for i in 0..stride {
            let a = if i >= bpp { cur_row[i - bpp] } else { 0 };
            let b = prev_row[i];
            let c = if i >= bpp { prev_row[i - bpp] } else { 0 };
            cur_row[i] = match filter {
                0 => src[i],
                1 => src[i].wrapping_add(a),
                2 => src[i].wrapping_add(b),
                3 => src[i].wrapping_add((((a as u16) + (b as u16)) / 2) as u8),
                4 => src[i].wrapping_add(paeth_predictor(a, b, c)),
                _ => src[i],
            };
        }
        for x in 0..(width as usize) {
            let px = &cur_row[x * bpp..(x + 1) * bpp];
            let (r, g, b_ch, a_ch) = match (color_type, bytes_per_sample) {
                (6, 1) => (px[0], px[1], px[2], px[3]),
                (6, 2) => (px[0], px[2], px[4], px[6]),
                (2, 1) => (px[0], px[1], px[2], 255),
                (2, 2) => (px[0], px[2], px[4], 255),
                (4, 1) => (px[0], px[0], px[0], px[1]),
                (4, 2) => (px[0], px[0], px[0], px[2]),
                (0, _) => (px[0], px[0], px[0], 255),
                (3, _) => {
                    let idx = px[0] as usize;
                    let rgb = palette.get(idx).copied().unwrap_or([0, 0, 0]);
                    let alpha = trns.get(idx).copied().unwrap_or(255);
                    (rgb[0], rgb[1], rgb[2], alpha)
                }
                _ => (0, 0, 0, 0),
            };
            rgba.extend_from_slice(&[r, g, b_ch, a_ch]);
        }
        std::mem::swap(&mut prev_row, &mut cur_row);
    }

    Some((width, height, rgba))
}

#[inline]
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let p = (a as i32) + (b as i32) - (c as i32);
    let pa = (p - a as i32).abs();
    let pb = (p - b as i32).abs();
    let pc = (p - c as i32).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn decode_bmp_rgba8(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if bytes.len() < 54 || &bytes[0..2] != b"BM" {
        return None;
    }
    let pixel_offset = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], bytes[13]]) as usize;
    let width = i32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
    let height_signed = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]);
    let bpp = u16::from_le_bytes([bytes[28], bytes[29]]);
    let compression = u32::from_le_bytes([bytes[30], bytes[31], bytes[32], bytes[33]]);
    if width <= 0 || height_signed == 0 || !matches!(bpp, 24 | 32) || compression != 0 {
        return None;
    }
    let w = width as usize;
    let h = height_signed.unsigned_abs() as usize;
    if w > 4096 || h > 4096 {
        return None;
    }
    let top_down = height_signed < 0;
    let bytes_pp = (bpp / 8) as usize;
    let row_stride = (w.checked_mul(bytes_pp)?.checked_add(3)?) & !3;
    let end_offset = pixel_offset.checked_add(row_stride.checked_mul(h)?)?;
    if end_offset > bytes.len() {
        return None;
    }
    let mut rgba = vec![0u8; w * h * 4];
    for dst_y in 0..h {
        let src_y = if top_down { dst_y } else { h - 1 - dst_y };
        let row = &bytes[pixel_offset + src_y * row_stride..pixel_offset + (src_y + 1) * row_stride];
        for x in 0..w {
            let p = &row[x * bytes_pp..(x + 1) * bytes_pp];
            let a = if bytes_pp == 4 { p[3] } else { 255 };
            let out = &mut rgba[(dst_y * w + x) * 4..(dst_y * w + x + 1) * 4];
            out[0] = p[2];
            out[1] = p[1];
            out[2] = p[0];
            out[3] = a;
        }
    }
    Some((w as u32, h as u32, rgba))
}

fn emit_rgba_grid_in_transform(
    src_w: u32,
    src_h: u32,
    rgba: &[u8],
    world_tf: &ShapeTransform2D,
) -> TessellatedMesh {
    const MAX_GRID: u32 = 56;
    let gw = src_w.min(MAX_GRID).max(1);
    let gh = src_h.min(MAX_GRID).max(1);
    let mut mesh = TessellatedMesh::new();

    let cell_w = world_tf.width / (gw as f64);
    let cell_h = world_tf.height / (gh as f64);

    let sample_box = |gx: u32, gy: u32| -> [u8; 4] {
        let x0 = ((gx as u64 * src_w as u64) / gw as u64) as u32;
        let x1 = (((gx + 1) as u64 * src_w as u64) / gw as u64).max((x0 + 1) as u64) as u32;
        let y0 = ((gy as u64 * src_h as u64) / gh as u64) as u32;
        let y1 = (((gy + 1) as u64 * src_h as u64) / gh as u64).max((y0 + 1) as u64) as u32;
        let (mut r_sum, mut g_sum, mut b_sum, mut a_sum, mut cnt) = (0u64, 0u64, 0u64, 0u64, 0u64);
        for sy in y0..y1.min(src_h) {
            for sx in x0..x1.min(src_w) {
                let off = ((sy * src_w + sx) as usize) * 4;
                let a = rgba[off + 3] as u64;
                r_sum += (rgba[off] as u64) * a;
                g_sum += (rgba[off + 1] as u64) * a;
                b_sum += (rgba[off + 2] as u64) * a;
                a_sum += a;
                cnt += 1;
            }
        }
        if cnt == 0 || a_sum == 0 {
            return [0, 0, 0, 0];
        }
        let avg_a = (a_sum / cnt).min(255) as u8;
        [
            (r_sum / a_sum).min(255) as u8,
            (g_sum / a_sum).min(255) as u8,
            (b_sum / a_sum).min(255) as u8,
            avg_a,
        ]
    };

    for gy in 0..gh {
        // Image row `0` is top -> `local_y = height - (gy + 1) * cell_h`!
        let ly0 = world_tf.height - ((gy + 1) as f64) * cell_h;
        let ly1 = world_tf.height - (gy as f64) * cell_h;
        let mut gx = 0u32;
        while gx < gw {
            let px = sample_box(gx, gy);
            if px[3] < 18 {
                gx += 1;
                continue;
            }
            // Quantize slightly (`& 0xF8`) to coalesce adjacent uniform icon pixels into wide horizontal quads!
            let key = [px[0] & 0xF8, px[1] & 0xF8, px[2] & 0xF8, px[3] & 0xF0];
            let start_gx = gx;
            gx += 1;
            while gx < gw {
                let nxt = sample_box(gx, gy);
                if nxt[3] >= 18
                    && [nxt[0] & 0xF8, nxt[1] & 0xF8, nxt[2] & 0xF8, nxt[3] & 0xF0] == key
                {
                    gx += 1;
                } else {
                    break;
                }
            }
            let lx0 = (start_gx as f64) * cell_w;
            let lx1 = (gx as f64) * cell_w;
            let p0 = world_tf.local_to_parent(lx0, ly0);
            let p1 = world_tf.local_to_parent(lx1, ly0);
            let p2 = world_tf.local_to_parent(lx1, ly1);
            let p3 = world_tf.local_to_parent(lx0, ly1);
            let col = RgbaColor {
                r: px[0],
                g: px[1],
                b: px[2],
                a: px[3],
            }
            .to_linear_f32();
            mesh.push_quad_points(
                [
                    [p0.0 as f32, p0.1 as f32],
                    [p1.0 as f32, p1.1 as f32],
                    [p2.0 as f32, p2.1 as f32],
                    [p3.0 as f32, p3.1 as f32],
                ],
                col,
                [PRIM_FLAG_SHAPE, 0.0],
            );
        }
    }

    mesh
}

// ============================================================================
// 3. Enhanced Metafile (EMF) Vector Stencil Record Tessellator
// ============================================================================

fn tessellate_emf_foreign(bytes: &[u8], world_tf: &ShapeTransform2D, tol_in: f32) -> TessellatedMesh {
    if bytes.len() < 88 {
        return TessellatedMesh::new();
    }
    let rd_i32 = |off: usize| -> i32 {
        i32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
    };
    let rd_u32 = |off: usize| -> u32 {
        u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
    };
    let rd_i16 = |off: usize| -> i16 { i16::from_le_bytes([bytes[off], bytes[off + 1]]) };

    let mut min_x = rd_i32(8) as f64;
    let mut min_y = rd_i32(12) as f64;
    let mut max_x = rd_i32(16) as f64;
    let mut max_y = rd_i32(20) as f64;
    if (max_x - min_x).abs() < 1.0 || (max_y - min_y).abs() < 1.0 {
        min_x = 0.0;
        min_y = 0.0;
        max_x = 1000.0;
        max_y = 1000.0;
    }
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);

    let map_emf = |ex: f64, ey: f64| -> lyon_path::math::Point {
        let nx = (ex - min_x) / span_x;
        let ny = 1.0 - (ey - min_y) / span_y;
        let (wx, wy) = world_tf.local_to_parent(nx * world_tf.width, ny * world_tf.height);
        point(wx as f32, wy as f32)
    };

    let mut pens: std::collections::BTreeMap<u32, Option<RgbaColor>> = std::collections::BTreeMap::new();
    let mut brushes: std::collections::BTreeMap<u32, Option<RgbaColor>> = std::collections::BTreeMap::new();
    let mut cur_stroke: Option<RgbaColor> = Some(RgbaColor::BLACK);
    let mut cur_fill: Option<RgbaColor> = Some(RgbaColor {
        r: 0,
        g: 120,
        b: 212,
        a: 255,
    });
    let mut cur_pt = [min_x, min_y];

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();
    let sw = (world_tf.width.min(world_tf.height) * 0.025).clamp(0.008, 0.06) as f32;

    let mut pos = 0usize;
    while pos + 8 <= bytes.len() {
        let rec_type = rd_u32(pos);
        let rec_size = rd_u32(pos + 4) as usize;
        if rec_size < 8 || pos + rec_size > bytes.len() {
            break;
        }
        let rec = &bytes[pos..pos + rec_size];
        match rec_type {
            14 => break, // EMR_EOF
            27 if rec.len() >= 16 => {
                cur_pt = [
                    i32::from_le_bytes([rec[8], rec[9], rec[10], rec[11]]) as f64,
                    i32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]) as f64,
                ];
            }
            37 if rec.len() >= 12 => {
                let ih = u32::from_le_bytes([rec[8], rec[9], rec[10], rec[11]]);
                if (ih & 0x8000_0000) != 0 {
                    match ih & 0x7FFF_FFFF {
                        0 => cur_fill = Some(RgbaColor::WHITE),
                        4 => cur_fill = Some(RgbaColor::BLACK),
                        5 => cur_fill = None, // NULL_BRUSH
                        6 => cur_stroke = Some(RgbaColor::WHITE),
                        7 => cur_stroke = Some(RgbaColor::BLACK),
                        8 => cur_stroke = None, // NULL_PEN
                        _ => {}
                    }
                } else if let Some(p) = pens.get(&ih) {
                    cur_stroke = *p;
                } else if let Some(br) = brushes.get(&ih) {
                    cur_fill = *br;
                }
            }
            38 if rec.len() >= 28 => {
                // EMR_CREATEPEN
                let ih = u32::from_le_bytes([rec[8], rec[9], rec[10], rec[11]]);
                let style = u32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]);
                let col = if (style & 0xFF) == 5 {
                    None
                } else {
                    Some(RgbaColor {
                        r: rec[24],
                        g: rec[25],
                        b: rec[26],
                        a: 255,
                    })
                };
                pens.insert(ih, col);
            }
            39 if rec.len() >= 24 => {
                // EMR_CREATEBRUSHINDIRECT
                let ih = u32::from_le_bytes([rec[8], rec[9], rec[10], rec[11]]);
                let style = u32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]);
                let col = if style == 1 {
                    None
                } else {
                    Some(RgbaColor {
                        r: rec[16],
                        g: rec[17],
                        b: rec[18],
                        a: 255,
                    })
                };
                brushes.insert(ih, col);
            }
            54 if rec.len() >= 16 => {
                // EMR_LINETO
                let nx = i32::from_le_bytes([rec[8], rec[9], rec[10], rec[11]]) as f64;
                let ny = i32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]) as f64;
                if let Some(sc) = cur_stroke {
                    let mut b = Path::builder();
                    b.begin(map_emf(cur_pt[0], cur_pt[1]));
                    b.line_to(map_emf(nx, ny));
                    b.end(false);
                    let path = b.build();
                    let rgba = sc.to_linear_f32();
                    let _ = stroke_tess.tessellate_path(
                        &path,
                        &StrokeOptions::tolerance(tol_in).with_line_width(sw),
                        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                            let p = v.position();
                            GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                        }),
                    );
                }
                cur_pt = [nx, ny];
            }
            42 | 43 if rec.len() >= 24 => {
                // EMR_ELLIPSE (42) / EMR_RECTANGLE (43)
                let l = i32::from_le_bytes([rec[8], rec[9], rec[10], rec[11]]) as f64;
                let t = i32::from_le_bytes([rec[12], rec[13], rec[14], rec[15]]) as f64;
                let r = i32::from_le_bytes([rec[16], rec[17], rec[18], rec[19]]) as f64;
                let bt = i32::from_le_bytes([rec[20], rec[21], rec[22], rec[23]]) as f64;
                let mut b = Path::builder();
                if rec_type == 43 {
                    b.begin(map_emf(l, t));
                    b.line_to(map_emf(r, t));
                    b.line_to(map_emf(r, bt));
                    b.line_to(map_emf(l, bt));
                    b.end(true);
                } else {
                    let cx = (l + r) * 0.5;
                    let cy = (t + bt) * 0.5;
                    let rx = (r - l).abs() * 0.5;
                    let ry = (bt - t).abs() * 0.5;
                    b.begin(map_emf(cx + rx, cy));
                    for s in 1..20 {
                        let a = std::f64::consts::TAU * (s as f64 / 20.0);
                        b.line_to(map_emf(cx + rx * a.cos(), cy + ry * a.sin()));
                    }
                    b.end(true);
                }
                let path = b.build();
                if let Some(fc) = cur_fill {
                    let rgba = fc.to_linear_f32();
                    let _ = fill_tess.tessellate_path(
                        &path,
                        &FillOptions::tolerance(tol_in),
                        &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                            let p = v.position();
                            GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                        }),
                    );
                }
                if let Some(sc) = cur_stroke {
                    let rgba = sc.to_linear_f32();
                    let _ = stroke_tess.tessellate_path(
                        &path,
                        &StrokeOptions::tolerance(tol_in).with_line_width(sw),
                        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                            let p = v.position();
                            GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                        }),
                    );
                }
            }
            3 | 4 | 86 | 87 if rec.len() >= 28 => {
                // EMR_POLYGON (3), EMR_POLYLINE (4), EMR_POLYGON16 (86), EMR_POLYLINE16 (87)
                let count = u32::from_le_bytes([rec[24], rec[25], rec[26], rec[27]]) as usize;
                let is_closed = rec_type == 3 || rec_type == 86;
                let is_16 = rec_type >= 85;
                let step = if is_16 { 4 } else { 8 };
                let needed = count.checked_mul(step).and_then(|n| n.checked_add(28));
                if count >= 2 && needed.is_some_and(|end| end <= rec.len()) {
                    let mut b = Path::builder();
                    for i in 0..count {
                        let off = pos + 28 + i * step;
                        let (px, py) = if is_16 {
                            (rd_i16(off) as f64, rd_i16(off + 2) as f64)
                        } else {
                            (rd_i32(off) as f64, rd_i32(off + 4) as f64)
                        };
                        if i == 0 {
                            b.begin(map_emf(px, py));
                        } else {
                            b.line_to(map_emf(px, py));
                        }
                    }
                    b.end(is_closed);
                    let path = b.build();
                    if is_closed {
                        if let Some(fc) = cur_fill {
                            let rgba = fc.to_linear_f32();
                            let _ = fill_tess.tessellate_path(
                                &path,
                                &FillOptions::tolerance(tol_in).with_fill_rule(FillRule::EvenOdd),
                                &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                                    let p = v.position();
                                    GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                                }),
                            );
                        }
                    }
                    if let Some(sc) = cur_stroke {
                        let rgba = sc.to_linear_f32();
                        let _ = stroke_tess.tessellate_path(
                            &path,
                            &StrokeOptions::tolerance(tol_in).with_line_width(sw),
                            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                                let p = v.position();
                                GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                            }),
                        );
                    }
                }
            }
            _ => {}
        }
        pos += rec_size;
    }

    TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    }
}
