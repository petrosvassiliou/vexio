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
            let mut mesh = tessellate_svg_foreign(svg_str, world_tf, tol_in);
            if !mesh.is_empty() {
                fit_foreign_mesh_to_transform(&mut mesh, world_tf);
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
        let mut mesh = tessellate_emf_foreign(&media.data, world_tf, tol_in);
        if !mesh.is_empty() {
            fit_foreign_mesh_to_transform(&mut mesh, world_tf);
            return mesh;
        }
    }

    let is_placeable_wmf = media.data.len() >= 22 && media.data[0..4] == [0xD7, 0xCD, 0xC6, 0x9A];
    let is_std_wmf = media.data.len() >= 18
        && matches!(u16::from_le_bytes([media.data[0], media.data[1]]), 1 | 2)
        && u16::from_le_bytes([media.data[2], media.data[3]]) == 9;
    if mt.contains("WMF") || is_placeable_wmf || is_std_wmf {
        let mut mesh = tessellate_wmf_foreign(&media.data, world_tf, tol_in);
        if !mesh.is_empty() {
            fit_foreign_mesh_to_transform(&mut mesh, world_tf);
            return mesh;
        }
    }

    TessellatedMesh::new()
}

/// Ensure vector `ForeignData` (`SVG` / `EMF` / `EMF+` / `WMF`) whose internal coordinate space
/// exceeds its declared header `viewBox` / `rclBounds` fits cleanly inside `world_tf` (`[0..width] x [0..height]`).
fn fit_foreign_mesh_to_transform(mesh: &mut TessellatedMesh, world_tf: &ShapeTransform2D) {
    if mesh.vertices.is_empty() {
        return;
    }
    let w = world_tf.width.abs();
    let h = world_tf.height.abs();
    if w < 1e-6 || h < 1e-6 {
        return;
    }
    let mut min_lx = f64::INFINITY;
    let mut min_ly = f64::INFINITY;
    let mut max_lx = f64::NEG_INFINITY;
    let mut max_ly = f64::NEG_INFINITY;
    for v in &mesh.vertices {
        let (lx, ly) = world_tf.parent_to_local(v.pos[0] as f64, v.pos[1] as f64);
        if lx.is_finite() && ly.is_finite() {
            min_lx = min_lx.min(lx);
            min_ly = min_ly.min(ly);
            max_lx = max_lx.max(lx);
            max_ly = max_ly.max(ly);
        }
    }
    if !min_lx.is_finite() || !max_lx.is_finite() {
        return;
    }
    let span_x = (max_lx - min_lx).max(1e-6);
    let span_y = (max_ly - min_ly).max(1e-6);
    let overflows = min_lx < -w * 0.25
        || max_lx > w * 1.25
        || min_ly < -h * 0.25
        || max_ly > h * 1.25;
    if !overflows {
        return;
    }
    let scale = (w / span_x).min(h / span_y);
    let pad_x = (w - span_x * scale) * 0.5;
    let pad_y = (h - span_y * scale) * 0.5;
    for v in &mut mesh.vertices {
        let (lx, ly) = world_tf.parent_to_local(v.pos[0] as f64, v.pos[1] as f64);
        let fit_lx = pad_x + (lx - min_lx) * scale;
        let fit_ly = pad_y + (ly - min_ly) * scale;
        let (wx, wy) = world_tf.local_to_parent(fit_lx, fit_ly);
        v.pos = [wx as f32, wy as f32];
    }
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
    xform: Affine2D,
}

impl Default for SvgStyleState {
    fn default() -> Self {
        Self {
            fill: Some(RgbaColor::BLACK),
            stroke: None,
            stroke_width: 1.0,
            opacity: 1.0,
            xform: Affine2D::default(),
        }
    }
}

#[derive(Default)]
struct SvgDocumentDefs {
    gradients: std::collections::BTreeMap<String, RgbaColor>,
    css_classes: std::collections::BTreeMap<String, String>,
}

fn is_svg_non_rendering_container(tag: &str) -> bool {
    matches!(
        tag,
        "defs"
            | "clippath"
            | "mask"
            | "symbol"
            | "marker"
            | "pattern"
            | "lineargradient"
            | "radialgradient"
            | "style"
            | "title"
            | "desc"
            | "metadata"
    )
}

fn collect_svg_defs_and_styles(svg: &str) -> SvgDocumentDefs {
    let mut defs = SvgDocumentDefs::default();
    let mut reader = Reader::from_str(svg);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut cur_grad_id: Option<String> = None;
    let mut cur_grad_stops: Vec<RgbaColor> = Vec::new();
    let mut grad_links: Vec<(String, String)> = Vec::new();
    let mut in_style = false;
    let mut style_buf = String::new();

    let empty_grads = std::collections::BTreeMap::new();
    let extract_stop_color = |e: &BytesStart<'_>| -> Option<RgbaColor> {
        let mut col: Option<RgbaColor> = None;
        let mut op = 1.0f64;
        if let Some(sc) = attr_val(e, b"stop-color") {
            if let Some(Some(c)) = parse_svg_paint(&sc, &empty_grads) {
                col = Some(c);
            }
        }
        if let Some(so) = attr_val(e, b"stop-opacity").and_then(|s| parse_svg_num(&s)) {
            op = so.clamp(0.0, 1.0);
        }
        if let Some(st_s) = attr_val(e, b"style") {
            for decl in st_s.split(';') {
                if let Some((k, v)) = decl.split_once(':') {
                    match k.trim().to_ascii_lowercase().as_str() {
                        "stop-color" => {
                            if let Some(Some(c)) = parse_svg_paint(v.trim(), &empty_grads) {
                                col = Some(c);
                            }
                        }
                        "stop-opacity" => {
                            if let Some(so) = parse_svg_num(v.trim()) {
                                op = so.clamp(0.0, 1.0);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        col.map(|mut c| {
            c.a = ((c.a as f64) * op).round().clamp(0.0, 255.0) as u8;
            c
        })
    };

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if tag == "lineargradient" || tag == "radialgradient" {
                    cur_grad_id = attr_val(&e, b"id");
                    cur_grad_stops.clear();
                    if let (Some(gid), Some(href)) = (
                        &cur_grad_id,
                        attr_val(&e, b"href").or_else(|| attr_val(&e, b"xlink:href")),
                    ) {
                        if let Some(target) = href.trim().strip_prefix('#') {
                            grad_links.push((gid.clone(), target.to_string()));
                        }
                    }
                } else if tag == "stop" && cur_grad_id.is_some() {
                    if let Some(c) = extract_stop_color(&e) {
                        cur_grad_stops.push(c);
                    }
                } else if tag == "style" {
                    in_style = true;
                    style_buf.clear();
                }
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if (tag == "lineargradient" || tag == "radialgradient") && cur_grad_id.is_none() {
                    if let (Some(gid), Some(href)) = (
                        attr_val(&e, b"id"),
                        attr_val(&e, b"href").or_else(|| attr_val(&e, b"xlink:href")),
                    ) {
                        if let Some(target) = href.trim().strip_prefix('#') {
                            grad_links.push((gid, target.to_string()));
                        }
                    }
                } else if tag == "stop" && cur_grad_id.is_some() {
                    if let Some(c) = extract_stop_color(&e) {
                        cur_grad_stops.push(c);
                    }
                }
            }
            Ok(Event::Text(t)) if in_style => {
                if let Ok(s) = t.unescape() {
                    style_buf.push_str(&s);
                }
            }
            Ok(Event::CData(cd)) if in_style => {
                style_buf.push_str(&String::from_utf8_lossy(cd.as_ref()));
            }
            Ok(Event::End(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if tag == "lineargradient" || tag == "radialgradient" {
                    if let Some(gid) = cur_grad_id.take() {
                        if !cur_grad_stops.is_empty() {
                            let n = cur_grad_stops.len() as u32;
                            let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
                            for c in &cur_grad_stops {
                                r += c.r as u32;
                                g += c.g as u32;
                                b += c.b as u32;
                                a += c.a as u32;
                            }
                            defs.gradients.insert(
                                gid,
                                RgbaColor {
                                    r: (r / n) as u8,
                                    g: (g / n) as u8,
                                    b: (b / n) as u8,
                                    a: (a / n) as u8,
                                },
                            );
                        }
                        cur_grad_stops.clear();
                    }
                } else if tag == "style" && in_style {
                    in_style = false;
                    parse_svg_css_rules(&style_buf, &mut defs.css_classes);
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    for (from_id, to_id) in grad_links {
        if !defs.gradients.contains_key(&from_id) {
            if let Some(&col) = defs.gradients.get(&to_id) {
                defs.gradients.insert(from_id, col);
            }
        }
    }
    defs
}

fn parse_svg_css_rules(css: &str, out: &mut std::collections::BTreeMap<String, String>) {
    for rule in css.split('}') {
        let Some((selectors, body)) = rule.split_once('{') else {
            continue;
        };
        let clean_body = body.trim();
        if clean_body.is_empty() {
            continue;
        }
        for sel in selectors.split(',') {
            if let Some(cls) = sel.trim().strip_prefix('.') {
                let entry = out.entry(cls.trim().to_string()).or_default();
                if !entry.is_empty() && !entry.ends_with(';') {
                    entry.push(';');
                }
                entry.push_str(clean_body);
            }
        }
    }
}

fn tessellate_svg_foreign(svg: &str, world_tf: &ShapeTransform2D, tol_in: f32) -> TessellatedMesh {
    let defs = collect_svg_defs_and_styles(svg);
    let mut reader = Reader::from_str(svg);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    let mut vb_x = 0.0f64;
    let mut vb_y = 0.0f64;
    let mut vb_w = 64.0f64;
    let mut vb_h = 64.0f64;
    let mut got_vb = false;
    let mut skip_depth = 0usize;

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
                if is_svg_non_rendering_container(&tag) || skip_depth > 0 {
                    skip_depth += 1;
                } else {
                    let next_state = derive_svg_state(stack.last().unwrap(), &e, &defs);
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
            }
            Ok(Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.local_name().as_ref()).to_ascii_lowercase();
                if tag == "svg" && !got_vb {
                    parse_svg_root_viewbox(&e, &mut vb_x, &mut vb_y, &mut vb_w, &mut vb_h);
                    got_vb = true;
                }
                if skip_depth == 0 && !is_svg_non_rendering_container(&tag) {
                    let st = derive_svg_state(stack.last().unwrap(), &e, &defs);
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
            }
            Ok(Event::End(_)) => {
                if skip_depth > 0 {
                    skip_depth -= 1;
                } else if stack.len() > 1 {
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
    let raw = s.trim();
    if raw.ends_with('%') {
        let pct = raw.trim_end_matches('%').trim().parse::<f64>().ok()?;
        return (pct * 0.01).is_finite().then_some(pct * 0.01);
    }
    let trimmed = raw
        .trim_end_matches("px")
        .trim_end_matches("pt")
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
    if let Some(ws) = attr_val(e, b"width") {
        if !ws.trim().ends_with('%') {
            if let Some(w) = parse_svg_num(&ws).filter(|&w| w > 0.0) {
                *vb_w = w;
            }
        }
    }
    if let Some(hs) = attr_val(e, b"height") {
        if !hs.trim().ends_with('%') {
            if let Some(h) = parse_svg_num(&hs).filter(|&h| h > 0.0) {
                *vb_h = h;
            }
        }
    }
}

fn parse_svg_paint(
    raw: &str,
    gradients: &std::collections::BTreeMap<String, RgbaColor>,
) -> Option<Option<RgbaColor>> {
    let t = raw.trim();
    if t.eq_ignore_ascii_case("none") || t.eq_ignore_ascii_case("transparent") {
        return Some(None);
    }
    if let Some(rest) = t.strip_prefix("url(").or_else(|| t.strip_prefix("URL(")) {
        if let Some(inner) = rest.strip_suffix(')') {
            let gid = inner
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .trim_start_matches('#');
            if let Some(&col) = gradients.get(gid) {
                return Some(Some(col));
            }
            // Fallback Azure blue if an unrecognized external gradient ID is referenced
            return Some(Some(RgbaColor {
                r: 0,
                g: 120,
                b: 212,
                a: 255,
            }));
        }
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

fn parse_svg_transform_list(tf_str: &str) -> Affine2D {
    let mut acc = Affine2D::default();
    let mut rest = tf_str.trim();
    while let Some(open_idx) = rest.find('(') {
        let fn_name = rest[..open_idx].trim().to_ascii_lowercase();
        let after_open = &rest[open_idx + 1..];
        let Some(close_idx) = after_open.find(')') else {
            break;
        };
        let args_str = &after_open[..close_idx];
        let nums: Vec<f64> = args_str
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect();
        let op = match fn_name.as_str() {
            "matrix" if nums.len() >= 6 => Affine2D {
                m11: nums[0],
                m12: nums[1],
                m21: nums[2],
                m22: nums[3],
                dx: nums[4],
                dy: nums[5],
            },
            "translate" if !nums.is_empty() => Affine2D {
                dx: nums[0],
                dy: nums.get(1).copied().unwrap_or(0.0),
                ..Affine2D::default()
            },
            "scale" if !nums.is_empty() => {
                let sx = nums[0];
                let sy = nums.get(1).copied().unwrap_or(sx);
                Affine2D {
                    m11: sx,
                    m22: sy,
                    ..Affine2D::default()
                }
            }
            "rotate" if !nums.is_empty() => {
                let (s, c) = nums[0].to_radians().sin_cos();
                let rot = Affine2D {
                    m11: c,
                    m12: s,
                    m21: -s,
                    m22: c,
                    dx: 0.0,
                    dy: 0.0,
                };
                if nums.len() >= 3 {
                    let cx = nums[1];
                    let cy = nums[2];
                    let t1 = Affine2D {
                        dx: cx,
                        dy: cy,
                        ..Affine2D::default()
                    };
                    let t2 = Affine2D {
                        dx: -cx,
                        dy: -cy,
                        ..Affine2D::default()
                    };
                    t2.multiply(rot).multiply(t1)
                } else {
                    rot
                }
            }
            _ => Affine2D::default(),
        };
        // SVG transform lists apply right-to-left to column vectors (`M_parent * M_op * p`),
        // which in row-vector `Affine2D::apply` (`p * M`) is `op.multiply(acc)`.
        acc = op.multiply(acc);
        rest = after_open[close_idx + 1..].trim().trim_start_matches(',').trim();
    }
    acc
}

fn derive_svg_state(
    parent: &SvgStyleState,
    e: &BytesStart<'_>,
    defs: &SvgDocumentDefs,
) -> SvgStyleState {
    let mut st = parent.clone();
    if let Some(cls_attr) = attr_val(e, b"class") {
        for cls in cls_attr.split_whitespace() {
            if let Some(css_body) = defs.css_classes.get(cls) {
                for decl in css_body.split(';') {
                    if let Some((k, v)) = decl.split_once(':') {
                        apply_svg_prop(&mut st, k.trim(), v.trim(), &defs.gradients);
                    }
                }
            }
        }
    }
    for key in [
        b"fill".as_slice(),
        b"stroke",
        b"stroke-width",
        b"opacity",
        b"fill-opacity",
    ] {
        if let Some(val) = attr_val(e, key) {
            let ks = std::str::from_utf8(key).unwrap_or("");
            apply_svg_prop(&mut st, ks, &val, &defs.gradients);
        }
    }
    if let Some(style_str) = attr_val(e, b"style") {
        for decl in style_str.split(';') {
            let Some((k, v)) = decl.split_once(':') else {
                continue;
            };
            apply_svg_prop(&mut st, k.trim(), v.trim(), &defs.gradients);
        }
    }
    if let Some(tf) = attr_val(e, b"transform") {
        let local_xf = parse_svg_transform_list(&tf);
        st.xform = local_xf.multiply(st.xform);
    }
    st
}

fn apply_svg_prop(
    st: &mut SvgStyleState,
    k: &str,
    v: &str,
    gradients: &std::collections::BTreeMap<String, RgbaColor>,
) {
    match k.to_ascii_lowercase().as_str() {
        "fill" => {
            if let Some(p) = parse_svg_paint(v, gradients) {
                st.fill = p;
            }
        }
        "stroke" => {
            if let Some(p) = parse_svg_paint(v, gradients) {
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
        if !sx.is_finite()
            || !sy.is_finite()
            || !vb.2.is_finite()
            || !vb.3.is_finite()
            || vb.2.abs() < 1e-9
            || vb.3.abs() < 1e-9
        {
            return fallback_pt;
        }
        let (tx, ty) = st.xform.apply(sx, sy);
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
            let xf_scale = st.xform.m11.hypot(st.xform.m12).max(0.1);
            let raw_sw = st.stroke_width * xf_scale * scale_in;
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

fn skip_svg_sep(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && (bytes[*i].is_ascii_whitespace() || bytes[*i] == b',') {
        *i += 1;
    }
}

fn read_svg_number(d: &str, i: &mut usize) -> Option<f64> {
    let bytes = d.as_bytes();
    skip_svg_sep(bytes, i);
    if *i >= bytes.len() {
        return None;
    }
    let start = *i;
    if bytes[*i] == b'+' || bytes[*i] == b'-' {
        *i += 1;
    }
    let mut saw_digit = false;
    let mut saw_dot = false;
    while *i < bytes.len() {
        let ch = bytes[*i];
        if ch.is_ascii_digit() {
            saw_digit = true;
            *i += 1;
        } else if ch == b'.' && !saw_dot {
            saw_dot = true;
            *i += 1;
        } else {
            break;
        }
    }
    if !saw_digit {
        *i = start;
        return None;
    }
    if *i < bytes.len() && (bytes[*i] == b'e' || bytes[*i] == b'E') {
        let exp_pos = *i;
        *i += 1;
        if *i < bytes.len() && (bytes[*i] == b'+' || bytes[*i] == b'-') {
            *i += 1;
        }
        let exp_digits_start = *i;
        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == exp_digits_start {
            *i = exp_pos;
        }
    }
    d[start..*i].parse::<f64>().ok().filter(|v| v.is_finite())
}

/// W3C SVG `<path>` arc flag reader: consumes a single `'0'` or `'1'` character,
/// cleanly handling concatenated minified flags such as `a8 8 0 10-16 0` or `0118.5`.
fn read_svg_arc_flag(d: &str, i: &mut usize) -> Option<bool> {
    let bytes = d.as_bytes();
    skip_svg_sep(bytes, i);
    match bytes.get(*i).copied() {
        Some(b'0') => {
            *i += 1;
            Some(false)
        }
        Some(b'1') => {
            *i += 1;
            Some(true)
        }
        _ => None,
    }
}

fn parse_svg_path_d<F>(d: &str, map_svg: &F, b: &mut lyon_path::path::Builder) -> bool
where
    F: Fn(f64, f64) -> lyon_path::math::Point,
{
    let bytes = d.as_bytes();
    let mut i = 0usize;
    let mut cmd = 'M';
    let mut cur = [0.0f64, 0.0f64];
    let mut sub_start = [0.0f64, 0.0f64];
    let mut last_ctrl = [0.0f64, 0.0f64];
    let mut in_sub = false;
    let mut any = false;

    loop {
        skip_svg_sep(bytes, &mut i);
        if i >= bytes.len() {
            break;
        }
        if bytes[i].is_ascii_alphabetic() {
            cmd = bytes[i] as char;
            i += 1;
        }
        match cmd {
            'M' | 'm' => {
                let (Some(x), Some(y)) = (read_svg_number(d, &mut i), read_svg_number(d, &mut i))
                else {
                    break;
                };
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
                let (Some(x), Some(y)) = (read_svg_number(d, &mut i), read_svg_number(d, &mut i))
                else {
                    break;
                };
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
                let Some(x) = read_svg_number(d, &mut i) else {
                    break;
                };
                cur[0] = if cmd == 'h' { cur[0] + x } else { x };
                last_ctrl = cur;
                if in_sub {
                    b.line_to(map_svg(cur[0], cur[1]));
                }
            }
            'V' | 'v' => {
                let Some(y) = read_svg_number(d, &mut i) else {
                    break;
                };
                cur[1] = if cmd == 'v' { cur[1] + y } else { y };
                last_ctrl = cur;
                if in_sub {
                    b.line_to(map_svg(cur[0], cur[1]));
                }
            }
            'C' | 'c' => {
                let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
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
                    b.cubic_bezier_to(
                        map_svg(c1[0], c1[1]),
                        map_svg(c2[0], c2[1]),
                        map_svg(end[0], end[1]),
                    );
                }
                last_ctrl = c2;
                cur = end;
            }
            'S' | 's' => {
                let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
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
                    b.cubic_bezier_to(
                        map_svg(c1[0], c1[1]),
                        map_svg(c2[0], c2[1]),
                        map_svg(end[0], end[1]),
                    );
                }
                last_ctrl = c2;
                cur = end;
            }
            'Q' | 'q' => {
                let (Some(cx), Some(cy), Some(x), Some(y)) = (
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
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
                let (Some(x), Some(y)) =
                    (read_svg_number(d, &mut i), read_svg_number(d, &mut i))
                else {
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
                let (
                    Some(rx),
                    Some(ry),
                    Some(rot_deg),
                    Some(large_arc),
                    Some(sweep),
                    Some(x),
                    Some(y),
                ) = (
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_arc_flag(d, &mut i),
                    read_svg_arc_flag(d, &mut i),
                    read_svg_number(d, &mut i),
                    read_svg_number(d, &mut i),
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
                        large_arc,
                        sweep,
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
                cmd = 'M';
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
    if width <= 0
        || height_signed == 0
        || !matches!(bpp, 16 | 24 | 32)
        || !matches!(compression, 0 | 3)
    {
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
            let out = &mut rgba[(dst_y * w + x) * 4..(dst_y * w + x + 1) * 4];
            if bytes_pp == 2 {
                // 16-bit RGB555 (`0RRRRRGGGGGBBBBB`)
                let v = u16::from_le_bytes([p[0], p[1]]);
                let r5 = ((v >> 10) & 0x1F) as u8;
                let g5 = ((v >> 5) & 0x1F) as u8;
                let b5 = (v & 0x1F) as u8;
                out[0] = (r5 << 3) | (r5 >> 2);
                out[1] = (g5 << 3) | (g5 >> 2);
                out[2] = (b5 << 3) | (b5 >> 2);
                out[3] = 255;
            } else {
                let a = if bytes_pp == 4 { p[3] } else { 255 };
                out[0] = p[2];
                out[1] = p[1];
                out[2] = p[0];
                out[3] = a;
            }
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
// 3. Enhanced Metafile (EMF), EMF+ & Windows Metafile (WMF) Vector Playback
// ============================================================================

#[derive(Clone, Copy, Debug)]
struct Affine2D {
    m11: f64,
    m12: f64,
    m21: f64,
    m22: f64,
    dx: f64,
    dy: f64,
}

impl Default for Affine2D {
    fn default() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            dx: 0.0,
            dy: 0.0,
        }
    }
}

impl Affine2D {
    #[inline]
    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.m11 + y * self.m21 + self.dx,
            x * self.m12 + y * self.m22 + self.dy,
        )
    }

    #[inline]
    fn multiply(self, rhs: Self) -> Self {
        Self {
            m11: self.m11 * rhs.m11 + self.m12 * rhs.m21,
            m12: self.m11 * rhs.m12 + self.m12 * rhs.m22,
            m21: self.m21 * rhs.m11 + self.m22 * rhs.m21,
            m22: self.m21 * rhs.m12 + self.m22 * rhs.m22,
            dx: self.dx * rhs.m11 + self.dy * rhs.m21 + rhs.dx,
            dy: self.dx * rhs.m12 + self.dy * rhs.m22 + rhs.dy,
        }
    }
}

#[derive(Clone, Debug)]
struct EmfDcState {
    stroke: Option<(RgbaColor, f32)>,
    fill: Option<RgbaColor>,
    fill_rule: FillRule,
    cur_pt: [f64; 2],
    win_org: [f64; 2],
    win_ext: [f64; 2],
    vp_org: [f64; 2],
    vp_ext: [f64; 2],
    has_win_vp: bool,
    xform: Affine2D,
}

#[derive(Clone, Debug)]
enum EmfGdiObject {
    Pen(Option<(RgbaColor, f32)>),
    Brush(Option<RgbaColor>),
}

#[derive(Clone, Debug)]
enum EmfPlusObj {
    Brush(RgbaColor),
    Pen(RgbaColor, f32),
    Path(Vec<(u8, [f64; 2])>),
}

fn parse_argb_u32(argb: u32) -> RgbaColor {
    RgbaColor {
        a: ((argb >> 24) & 0xFF) as u8,
        r: ((argb >> 16) & 0xFF) as u8,
        g: ((argb >> 8) & 0xFF) as u8,
        b: (argb & 0xFF) as u8,
    }
}

fn tessellate_emf_foreign(bytes: &[u8], world_tf: &ShapeTransform2D, tol_in: f32) -> TessellatedMesh {
    if bytes.len() < 88 {
        return TessellatedMesh::new();
    }
    let rd_i32_at = |b: &[u8], off: usize| -> i32 {
        i32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    };
    let rd_u32_at = |b: &[u8], off: usize| -> u32 {
        u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    };
    let rd_i16_at = |b: &[u8], off: usize| -> i16 { i16::from_le_bytes([b[off], b[off + 1]]) };
    let rd_f32_at = |b: &[u8], off: usize| -> f32 {
        f32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    };

    // Header `rclBounds` (device units `[8..24]`) and `rclFrame` (0.01mm `[24..40]`).
    let mut min_x = rd_i32_at(bytes, 8) as f64;
    let mut min_y = rd_i32_at(bytes, 12) as f64;
    let mut max_x = rd_i32_at(bytes, 16) as f64;
    let mut max_y = rd_i32_at(bytes, 20) as f64;
    if (max_x - min_x).abs() < 1.0 || (max_y - min_y).abs() < 1.0 {
        let fx0 = rd_i32_at(bytes, 24) as f64;
        let fy0 = rd_i32_at(bytes, 28) as f64;
        let fx1 = rd_i32_at(bytes, 32) as f64;
        let fy1 = rd_i32_at(bytes, 36) as f64;
        if (fx1 - fx0).abs() >= 1.0 && (fy1 - fy0).abs() >= 1.0 {
            min_x = fx0;
            min_y = fy0;
            max_x = fx1;
            max_y = fy1;
        } else {
            min_x = 0.0;
            min_y = 0.0;
            max_x = 1000.0;
            max_y = 1000.0;
        }
    }
    let span_x = (max_x - min_x).abs().max(1.0);
    let span_y = (max_y - min_y).abs().max(1.0);
    let base_sw = (world_tf.width.min(world_tf.height) * 0.020).clamp(0.006, 0.055) as f32;

    let map_dev_to_page = |dx: f64, dy: f64, dc: &EmfDcState| -> lyon_path::math::Point {
        let (wx0, wy0) = dc.xform.apply(dx, dy);
        let (dev_x, dev_y) = if dc.has_win_vp
            && dc.win_ext[0].abs() > 1e-6
            && dc.win_ext[1].abs() > 1e-6
        {
            (
                (wx0 - dc.win_org[0]) * (dc.vp_ext[0] / dc.win_ext[0]) + dc.vp_org[0],
                (wy0 - dc.win_org[1]) * (dc.vp_ext[1] / dc.win_ext[1]) + dc.vp_org[1],
            )
        } else {
            (wx0, wy0)
        };
        let nx = (dev_x - min_x) / span_x;
        let ny = 1.0 - (dev_y - min_y) / span_y;
        let (px, py) = world_tf.local_to_parent(nx * world_tf.width, ny * world_tf.height);
        point(px as f32, py as f32)
    };

    let map_plus_to_page = |px_in: f64, py_in: f64, xform: &Affine2D| -> lyon_path::math::Point {
        let (tx, ty) = xform.apply(px_in, py_in);
        let nx = (tx - min_x) / span_x;
        let ny = 1.0 - (ty - min_y) / span_y;
        let (wx, wy) = world_tf.local_to_parent(nx * world_tf.width, ny * world_tf.height);
        point(wx as f32, wy as f32)
    };

    let mut objects: std::collections::BTreeMap<u32, EmfGdiObject> = std::collections::BTreeMap::new();
    let mut plus_objects: std::collections::BTreeMap<u8, EmfPlusObj> = std::collections::BTreeMap::new();
    let mut plus_xform = Affine2D::default();

    let mut dc = EmfDcState {
        stroke: Some((RgbaColor::BLACK, base_sw)),
        fill: Some(RgbaColor {
            r: 0,
            g: 120,
            b: 212,
            a: 255,
        }),
        fill_rule: FillRule::EvenOdd,
        cur_pt: [min_x, min_y],
        win_org: [min_x, min_y],
        win_ext: [span_x, span_y],
        vp_org: [min_x, min_y],
        vp_ext: [span_x, span_y],
        has_win_vp: false,
        xform: Affine2D::default(),
    };
    let mut dc_stack: Vec<EmfDcState> = Vec::new();

    // Active GDI path bracket (`EMR_BEGINPATH .. EMR_ENDPATH`)
    let mut in_path = false;
    let mut active_path_builder: Option<lyon_path::path::Builder> = None;
    let mut path_sub_open = false;
    let mut built_gdi_path: Option<Path> = None;

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut plus_lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();
    let mut extra_meshes: Vec<TessellatedMesh> = Vec::new();
    let mut has_emf_plus_png = false;

    let emit_path_paint = |p: &Path,
                           do_fill: bool,
                           do_stroke: bool,
                           dc_st: &EmfDcState,
                           ft: &mut FillTessellator,
                           st: &mut StrokeTessellator,
                           buf: &mut VertexBuffers<GpuVertex, u32>| {
        if do_fill {
            if let Some(fc) = dc_st.fill {
                let rgba = fc.to_linear_f32();
                let _ = ft.tessellate_path(
                    p,
                    &FillOptions::tolerance(tol_in).with_fill_rule(dc_st.fill_rule),
                    &mut BuffersBuilder::new(buf, |v: FillVertex| {
                        let pt = v.position();
                        GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            }
        }
        if do_stroke {
            if let Some((sc, pw)) = dc_st.stroke {
                let rgba = sc.to_linear_f32();
                let _ = st.tessellate_path(
                    p,
                    &StrokeOptions::tolerance(tol_in)
                        .with_line_width(pw)
                        .with_line_cap(LineCap::Round)
                        .with_line_join(LineJoin::Round),
                    &mut BuffersBuilder::new(buf, |v: StrokeVertex| {
                        let pt = v.position();
                        GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                    }),
                );
            }
        }
    };

    let mut pos = 0usize;
    while pos + 8 <= bytes.len() {
        let rec_type = rd_u32_at(bytes, pos);
        let rec_size = rd_u32_at(bytes, pos + 4) as usize;
        if rec_size < 8 || pos + rec_size > bytes.len() {
            break;
        }
        let rec = &bytes[pos..pos + rec_size];
        match rec_type {
            14 => break, // EMR_EOF
            9 if rec.len() >= 16 => {
                // EMR_SETWINDOWEXTEX
                dc.win_ext = [rd_i32_at(rec, 8) as f64, rd_i32_at(rec, 12) as f64];
                dc.has_win_vp = true;
            }
            10 if rec.len() >= 16 => {
                // EMR_SETWINDOWORGEX
                dc.win_org = [rd_i32_at(rec, 8) as f64, rd_i32_at(rec, 12) as f64];
                dc.has_win_vp = true;
            }
            11 if rec.len() >= 16 => {
                // EMR_SETVIEWPORTEXTEX
                dc.vp_ext = [rd_i32_at(rec, 8) as f64, rd_i32_at(rec, 12) as f64];
                dc.has_win_vp = true;
            }
            12 if rec.len() >= 16 => {
                // EMR_SETVIEWPORTORGEX
                dc.vp_org = [rd_i32_at(rec, 8) as f64, rd_i32_at(rec, 12) as f64];
                dc.has_win_vp = true;
            }
            19 if rec.len() >= 12 => {
                // EMR_SETPOLYFILLMODE (1 = ALTERNATE/EvenOdd, 2 = WINDING/NonZero)
                dc.fill_rule = if rd_u32_at(rec, 8) == 2 {
                    FillRule::NonZero
                } else {
                    FillRule::EvenOdd
                };
            }
            33 => {
                // EMR_SAVEDC
                if dc_stack.len() < 64 {
                    dc_stack.push(dc.clone());
                }
            }
            34 if rec.len() >= 12 => {
                // EMR_RESTOREDC
                let rel = rd_i32_at(rec, 8);
                let pops = if rel < 0 { (-rel) as usize } else { 1 };
                for _ in 0..pops {
                    if let Some(saved) = dc_stack.pop() {
                        dc = saved;
                    }
                }
            }
            35 | 36 if rec.len() >= 32 => {
                // EMR_SETWORLDTRANSFORM (35) / EMR_MODIFYWORLDTRANSFORM (36)
                let xf = Affine2D {
                    m11: rd_f32_at(rec, 8) as f64,
                    m12: rd_f32_at(rec, 12) as f64,
                    m21: rd_f32_at(rec, 16) as f64,
                    m22: rd_f32_at(rec, 20) as f64,
                    dx: rd_f32_at(rec, 24) as f64,
                    dy: rd_f32_at(rec, 28) as f64,
                };
                if rec_type == 35 {
                    if xf.m11.is_finite() && xf.m22.is_finite() {
                        dc.xform = xf;
                    }
                } else if rec.len() >= 36 {
                    let mode = rd_u32_at(rec, 32);
                    match mode {
                        1 => dc.xform = Affine2D::default(), // MWT_IDENTITY
                        2 => dc.xform = xf.multiply(dc.xform), // MWT_LEFTMULTIPLY
                        3 => dc.xform = dc.xform.multiply(xf), // MWT_RIGHTMULTIPLY
                        4 => dc.xform = xf,
                        _ => {}
                    }
                }
            }
            27 if rec.len() >= 16 => {
                // EMR_MOVETOEX
                dc.cur_pt = [rd_i32_at(rec, 8) as f64, rd_i32_at(rec, 12) as f64];
                if in_path {
                    if let Some(pb) = &mut active_path_builder {
                        if path_sub_open {
                            pb.end(false);
                        }
                        pb.begin(map_dev_to_page(dc.cur_pt[0], dc.cur_pt[1], &dc));
                        path_sub_open = true;
                    }
                }
            }
            37 if rec.len() >= 12 => {
                // EMR_SELECTOBJECT
                let ih = rd_u32_at(rec, 8);
                if (ih & 0x8000_0000) != 0 {
                    match ih & 0x7FFF_FFFF {
                        0 => dc.fill = Some(RgbaColor::WHITE),
                        1 => dc.fill = Some(RgbaColor { r: 192, g: 192, b: 192, a: 255 }),
                        2 | 3 => dc.fill = Some(RgbaColor { r: 128, g: 128, b: 128, a: 255 }),
                        4 => dc.fill = Some(RgbaColor::BLACK),
                        5 => dc.fill = None, // NULL_BRUSH
                        6 => dc.stroke = Some((RgbaColor::WHITE, base_sw)),
                        7 => dc.stroke = Some((RgbaColor::BLACK, base_sw)),
                        8 => dc.stroke = None, // NULL_PEN
                        _ => {}
                    }
                } else if let Some(obj) = objects.get(&ih) {
                    match obj {
                        EmfGdiObject::Pen(p) => dc.stroke = *p,
                        EmfGdiObject::Brush(b) => dc.fill = *b,
                    }
                }
            }
            38 if rec.len() >= 28 => {
                // EMR_CREATEPEN
                let ih = rd_u32_at(rec, 8);
                let style = rd_u32_at(rec, 12);
                let raw_w = rd_i32_at(rec, 16).unsigned_abs() as f64;
                let pw = if raw_w > 0.0 {
                    ((raw_w / span_x) * world_tf.width).clamp(0.005, 0.12) as f32
                } else {
                    base_sw
                };
                let pen = if (style & 0xFF) == 5 {
                    None
                } else {
                    Some((
                        RgbaColor {
                            r: rec[24],
                            g: rec[25],
                            b: rec[26],
                            a: 255,
                        },
                        pw,
                    ))
                };
                objects.insert(ih, EmfGdiObject::Pen(pen));
            }
            95 if rec.len() >= 52 => {
                // EMR_EXTCREATEPEN (used by Illustrator, Visio, Cisco/HPE vendor stencils!)
                let ih = rd_u32_at(rec, 8);
                let pen_style = rd_u32_at(rec, 28);
                let raw_w = rd_u32_at(rec, 32) as f64;
                let brush_style = rd_u32_at(rec, 36);
                let pw = if raw_w > 0.0 {
                    ((raw_w / span_x) * world_tf.width).clamp(0.005, 0.12) as f32
                } else {
                    base_sw
                };
                let pen = if (pen_style & 0x0F) == 5 || brush_style == 1 {
                    None
                } else {
                    Some((
                        RgbaColor {
                            r: rec[40],
                            g: rec[41],
                            b: rec[42],
                            a: 255,
                        },
                        pw,
                    ))
                };
                objects.insert(ih, EmfGdiObject::Pen(pen));
            }
            39 if rec.len() >= 24 => {
                // EMR_CREATEBRUSHINDIRECT
                let ih = rd_u32_at(rec, 8);
                let style = rd_u32_at(rec, 12);
                let br = if style == 1 {
                    None
                } else {
                    Some(RgbaColor {
                        r: rec[16],
                        g: rec[17],
                        b: rec[18],
                        a: 255,
                    })
                };
                objects.insert(ih, EmfGdiObject::Brush(br));
            }
            40 if rec.len() >= 12 => {
                // EMR_DELETEOBJECT
                objects.remove(&rd_u32_at(rec, 8));
            }
            59 => {
                // EMR_BEGINPATH
                in_path = true;
                active_path_builder = Some(Path::builder());
                path_sub_open = false;
            }
            60 => {
                // EMR_ENDPATH
                in_path = false;
                if let Some(mut pb) = active_path_builder.take() {
                    if path_sub_open {
                        pb.end(false);
                        path_sub_open = false;
                    }
                    built_gdi_path = Some(pb.build());
                }
            }
            61 => {
                // EMR_CLOSEFIGURE
                if in_path && path_sub_open {
                    if let Some(pb) = &mut active_path_builder {
                        pb.end(true);
                        path_sub_open = false;
                    }
                }
            }
            62 | 63 | 64 => {
                // EMR_FILLPATH (62), EMR_STROKEANDFILLPATH (63), EMR_STROKEPATH (64)
                if let Some(gp) = built_gdi_path.take() {
                    emit_path_paint(
                        &gp,
                        rec_type == 62 || rec_type == 63,
                        rec_type == 63 || rec_type == 64,
                        &dc,
                        &mut fill_tess,
                        &mut stroke_tess,
                        &mut lyon_buf,
                    );
                }
            }
            54 if rec.len() >= 16 => {
                // EMR_LINETO
                let nx = rd_i32_at(rec, 8) as f64;
                let ny = rd_i32_at(rec, 12) as f64;
                if in_path {
                    if let Some(pb) = &mut active_path_builder {
                        if !path_sub_open {
                            pb.begin(map_dev_to_page(dc.cur_pt[0], dc.cur_pt[1], &dc));
                            path_sub_open = true;
                        }
                        pb.line_to(map_dev_to_page(nx, ny, &dc));
                    }
                } else {
                    let mut b = Path::builder();
                    b.begin(map_dev_to_page(dc.cur_pt[0], dc.cur_pt[1], &dc));
                    b.line_to(map_dev_to_page(nx, ny, &dc));
                    b.end(false);
                    emit_path_paint(
                        &b.build(),
                        false,
                        true,
                        &dc,
                        &mut fill_tess,
                        &mut stroke_tess,
                        &mut lyon_buf,
                    );
                }
                dc.cur_pt = [nx, ny];
            }
            42 | 43 | 44 if rec.len() >= 24 => {
                // EMR_ELLIPSE (42), EMR_RECTANGLE (43), EMR_ROUNDRECT (44)
                let l = rd_i32_at(rec, 8) as f64;
                let t = rd_i32_at(rec, 12) as f64;
                let r = rd_i32_at(rec, 16) as f64;
                let bt = rd_i32_at(rec, 20) as f64;
                let append_fig = |pb: &mut lyon_path::path::Builder| {
                    if rec_type == 43 || rec_type == 44 {
                        pb.begin(map_dev_to_page(l, t, &dc));
                        pb.line_to(map_dev_to_page(r, t, &dc));
                        pb.line_to(map_dev_to_page(r, bt, &dc));
                        pb.line_to(map_dev_to_page(l, bt, &dc));
                        pb.end(true);
                    } else {
                        let cx = (l + r) * 0.5;
                        let cy = (t + bt) * 0.5;
                        let rx = (r - l).abs() * 0.5;
                        let ry = (bt - t).abs() * 0.5;
                        pb.begin(map_dev_to_page(cx + rx, cy, &dc));
                        for s in 1..24 {
                            let a = std::f64::consts::TAU * (s as f64 / 24.0);
                            pb.line_to(map_dev_to_page(cx + rx * a.cos(), cy + ry * a.sin(), &dc));
                        }
                        pb.end(true);
                    }
                };
                if in_path {
                    if let Some(pb) = &mut active_path_builder {
                        if path_sub_open {
                            pb.end(false);
                            path_sub_open = false;
                        }
                        append_fig(pb);
                    }
                } else {
                    let mut b = Path::builder();
                    append_fig(&mut b);
                    emit_path_paint(
                        &b.build(),
                        true,
                        true,
                        &dc,
                        &mut fill_tess,
                        &mut stroke_tess,
                        &mut lyon_buf,
                    );
                }
            }
            2 | 3 | 4 | 5 | 6 | 85 | 86 | 87 | 88 | 89 if rec.len() >= 28 => {
                // Polyline / Polygon / PolyBezier / PolyBezierTo / PolylineTo (32-bit & 16-bit)
                let count = rd_u32_at(rec, 24) as usize;
                let is_16 = rec_type >= 85;
                let step = if is_16 { 4 } else { 8 };
                let is_bezier = matches!(rec_type, 2 | 5 | 85 | 88);
                let is_to = matches!(rec_type, 5 | 6 | 88 | 89);
                let is_closed = matches!(rec_type, 3 | 86);
                let needed = count.checked_mul(step).and_then(|n| n.checked_add(28));
                if count >= 1 && needed.is_some_and(|end| end <= rec.len()) {
                    let read_pt = |idx: usize| -> [f64; 2] {
                        let off = 28 + idx * step;
                        if is_16 {
                            [rd_i16_at(rec, off) as f64, rd_i16_at(rec, off + 2) as f64]
                        } else {
                            [rd_i32_at(rec, off) as f64, rd_i32_at(rec, off + 4) as f64]
                        }
                    };
                    if in_path {
                        if let Some(pb) = &mut active_path_builder {
                            let mut start_idx = 0usize;
                            if is_to {
                                if !path_sub_open {
                                    pb.begin(map_dev_to_page(dc.cur_pt[0], dc.cur_pt[1], &dc));
                                    path_sub_open = true;
                                }
                            } else {
                                if path_sub_open {
                                    pb.end(false);
                                }
                                let p0 = read_pt(0);
                                pb.begin(map_dev_to_page(p0[0], p0[1], &dc));
                                path_sub_open = true;
                                start_idx = 1;
                            }
                            if is_bezier {
                                let mut i = start_idx;
                                while i + 2 < count {
                                    let c1 = read_pt(i);
                                    let c2 = read_pt(i + 1);
                                    let p = read_pt(i + 2);
                                    pb.cubic_bezier_to(
                                        map_dev_to_page(c1[0], c1[1], &dc),
                                        map_dev_to_page(c2[0], c2[1], &dc),
                                        map_dev_to_page(p[0], p[1], &dc),
                                    );
                                    dc.cur_pt = p;
                                    i += 3;
                                }
                            } else {
                                for i in start_idx..count {
                                    let p = read_pt(i);
                                    pb.line_to(map_dev_to_page(p[0], p[1], &dc));
                                    dc.cur_pt = p;
                                }
                            }
                            if is_closed && path_sub_open {
                                pb.end(true);
                                path_sub_open = false;
                            }
                        }
                    } else {
                        let mut b = Path::builder();
                        let mut start_idx = 0usize;
                        if is_to {
                            b.begin(map_dev_to_page(dc.cur_pt[0], dc.cur_pt[1], &dc));
                        } else {
                            let p0 = read_pt(0);
                            b.begin(map_dev_to_page(p0[0], p0[1], &dc));
                            start_idx = 1;
                        }
                        if is_bezier {
                            let mut i = start_idx;
                            while i + 2 < count {
                                let c1 = read_pt(i);
                                let c2 = read_pt(i + 1);
                                let p = read_pt(i + 2);
                                b.cubic_bezier_to(
                                    map_dev_to_page(c1[0], c1[1], &dc),
                                    map_dev_to_page(c2[0], c2[1], &dc),
                                    map_dev_to_page(p[0], p[1], &dc),
                                );
                                dc.cur_pt = p;
                                i += 3;
                            }
                        } else {
                            for i in start_idx..count {
                                let p = read_pt(i);
                                b.line_to(map_dev_to_page(p[0], p[1], &dc));
                                dc.cur_pt = p;
                            }
                        }
                        b.end(is_closed);
                        emit_path_paint(
                            &b.build(),
                            is_closed,
                            true,
                            &dc,
                            &mut fill_tess,
                            &mut stroke_tess,
                            &mut lyon_buf,
                        );
                    }
                }
            }
            7 | 8 | 90 | 91 if rec.len() >= 32 => {
                // EMR_POLYPOLYLINE (7), EMR_POLYPOLYGON (8), EMR_POLYPOLYLINE16 (90), EMR_POLYPOLYGON16 (91)
                let n_polys = rd_u32_at(rec, 24) as usize;
                let total_pts = rd_u32_at(rec, 28) as usize;
                let is_16 = rec_type >= 90;
                let is_closed = rec_type == 8 || rec_type == 91;
                let counts_end = 32usize.saturating_add(n_polys.saturating_mul(4));
                let step = if is_16 { 4 } else { 8 };
                let pts_end = counts_end.saturating_add(total_pts.saturating_mul(step));
                if n_polys > 0 && n_polys <= 4096 && pts_end <= rec.len() {
                    let mut b = Path::builder();
                    let mut pt_cursor = 0usize;
                    for p_idx in 0..n_polys {
                        let cnt = rd_u32_at(rec, 32 + p_idx * 4) as usize;
                        if cnt >= 2 && pt_cursor + cnt <= total_pts {
                            for j in 0..cnt {
                                let off = counts_end + (pt_cursor + j) * step;
                                let (px, py) = if is_16 {
                                    (rd_i16_at(rec, off) as f64, rd_i16_at(rec, off + 2) as f64)
                                } else {
                                    (rd_i32_at(rec, off) as f64, rd_i32_at(rec, off + 4) as f64)
                                };
                                if j == 0 {
                                    b.begin(map_dev_to_page(px, py, &dc));
                                } else {
                                    b.line_to(map_dev_to_page(px, py, &dc));
                                }
                            }
                            b.end(is_closed);
                        }
                        pt_cursor += cnt;
                    }
                    emit_path_paint(
                        &b.build(),
                        is_closed,
                        true,
                        &dc,
                        &mut fill_tess,
                        &mut stroke_tess,
                        &mut lyon_buf,
                    );
                }
            }
            81 if rec.len() >= 80 && !has_emf_plus_png => {
                // EMR_STRETCHDIBITS (embedded DIB raster inside EMF stencil)
                let off_bmi = rd_u32_at(rec, 48) as usize;
                let cb_bmi = rd_u32_at(rec, 52) as usize;
                let off_bits = rd_u32_at(rec, 56) as usize;
                let cb_bits = rd_u32_at(rec, 60) as usize;
                if off_bmi >= 80
                    && cb_bmi >= 40
                    && off_bmi.saturating_add(cb_bmi) <= rec.len()
                    && off_bits.saturating_add(cb_bits) <= rec.len()
                    && cb_bits > 0
                {
                    // Reconstruct a minimal 14-byte BMP file header + BITMAPINFO + bits for `decode_bmp_rgba8`.
                    let total_bmp = 14usize + cb_bmi + cb_bits;
                    let mut bmp = Vec::with_capacity(total_bmp);
                    bmp.extend_from_slice(b"BM");
                    bmp.extend_from_slice(&(total_bmp as u32).to_le_bytes());
                    bmp.extend_from_slice(&0u32.to_le_bytes());
                    bmp.extend_from_slice(&((14 + cb_bmi) as u32).to_le_bytes());
                    bmp.extend_from_slice(&rec[off_bmi..off_bmi + cb_bmi]);
                    bmp.extend_from_slice(&rec[off_bits..off_bits + cb_bits]);
                    if let Some((bw, bh, rgba)) = decode_bmp_rgba8(&bmp) {
                        extra_meshes.push(emit_rgba_grid_in_transform(bw, bh, &rgba, world_tf));
                    }
                }
            }
            70 if rec.len() >= 20 => {
                // EMR_COMMENT: inspect for `EMF+` dual/pure vector & embedded PNG records (`0x2B464D45` = `b"EMF+"`).
                let cb_data = rd_u32_at(rec, 8) as usize;
                if 12 + cb_data <= rec.len() && cb_data >= 8 && &rec[12..16] == b"EMF+" {
                    let plus_buf = &rec[16..12 + cb_data];
                    playback_emf_plus_stream(
                        plus_buf,
                        base_sw,
                        tol_in,
                        &mut plus_objects,
                        &mut plus_xform,
                        &map_plus_to_page,
                        &mut fill_tess,
                        &mut stroke_tess,
                        &mut plus_lyon_buf,
                    );
                    if let Some(png_off) = plus_buf
                        .windows(8)
                        .position(|w| w == b"\x89PNG\r\n\x1a\n")
                    {
                        if let Some((pw, ph, prgba)) = decode_png_rgba8(&plus_buf[png_off..]) {
                            // Prefer 32-bit alpha EMF+ PNG over 16-bit GDI StretchDIBits fallback!
                            has_emf_plus_png = true;
                            extra_meshes.clear();
                            extra_meshes.push(emit_rgba_grid_in_transform(pw, ph, &prgba, world_tf));
                        }
                    }
                }
            }
            _ => {}
        }
        pos += rec_size;
    }

    let mut out = if !plus_lyon_buf.indices.is_empty() {
        TessellatedMesh {
            vertices: plus_lyon_buf.vertices,
            indices: plus_lyon_buf.indices,
        }
    } else {
        TessellatedMesh {
            vertices: lyon_buf.vertices,
            indices: lyon_buf.indices,
        }
    };
    for em in extra_meshes {
        out.append(&em);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn playback_emf_plus_stream<F>(
    stream: &[u8],
    base_sw: f32,
    tol_in: f32,
    objs: &mut std::collections::BTreeMap<u8, EmfPlusObj>,
    xform: &mut Affine2D,
    map_pt: &F,
    fill_tess: &mut FillTessellator,
    stroke_tess: &mut StrokeTessellator,
    lyon_buf: &mut VertexBuffers<GpuVertex, u32>,
) where
    F: Fn(f64, f64, &Affine2D) -> lyon_path::math::Point,
{
    let rd_u16 = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let rd_i16 = |b: &[u8], o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
    let rd_u32 = |b: &[u8], o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
    let rd_f32 = |b: &[u8], o: usize| f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);

    let mut off = 0usize;
    while off + 12 <= stream.len() {
        let rtype = rd_u16(stream, off);
        let flags = rd_u16(stream, off + 2);
        let rsize = rd_u32(stream, off + 4) as usize;
        let dsize = rd_u32(stream, off + 8) as usize;
        if rsize < 12 || off + rsize > stream.len() {
            break;
        }
        let data = &stream[off + 12..(off + 12 + dsize.min(rsize - 12))];
        match rtype {
            0x4008 => {
                // EmfPlusObject
                let obj_id = (flags & 0xFF) as u8;
                let obj_type = (flags >> 8) & 0x3F;
                match obj_type {
                    1 if data.len() >= 12 => {
                        // Brush: `[version:4][type:4]` (`0`=SolidColor `[argb:4]`, `4`=LinearGradient `[flags:4][wrap:4][rect:16][start_argb:4]...`)
                        let btype = rd_u32(data, 4);
                        if btype == 0 {
                            objs.insert(obj_id, EmfPlusObj::Brush(parse_argb_u32(rd_u32(data, 8))));
                        } else if btype == 4 && data.len() >= 40 {
                            let c0 = parse_argb_u32(rd_u32(data, 32));
                            let c1 = parse_argb_u32(rd_u32(data, 36));
                            objs.insert(
                                obj_id,
                                EmfPlusObj::Brush(RgbaColor {
                                    r: ((c0.r as u16 + c1.r as u16) / 2) as u8,
                                    g: ((c0.g as u16 + c1.g as u16) / 2) as u8,
                                    b: ((c0.b as u16 + c1.b as u16) / 2) as u8,
                                    a: ((c0.a as u16 + c1.a as u16) / 2) as u8,
                                }),
                            );
                        }
                    }
                    2 if data.len() >= 28 => {
                        // Pen: `[ver:4][type:4][penFlags:4][unit:4][width:f32][brushVer:4][brushType:4][argb:4]`
                        let pen_flags = rd_u32(data, 8);
                        // Count optional float fields set in `penFlags` before embedded Brush!
                        let mut opt_off = 20usize;
                        for &(bit, bytes_len) in &[
                            (0x0001u32, 24usize), // Transform
                            (0x0002, 4),          // StartCap
                            (0x0004, 4),          // EndCap
                            (0x0008, 4),          // Join
                            (0x0010, 4),          // MiterLimit
                            (0x0020, 4),          // LineStyle
                            (0x0040, 4),          // DashedLineCap
                            (0x0080, 4),          // DashedLineOffset
                        ] {
                            if (pen_flags & bit) != 0 {
                                opt_off += bytes_len;
                            }
                        }
                        if (pen_flags & 0x0100) != 0 && opt_off + 4 <= data.len() {
                            let dash_cnt = rd_u32(data, opt_off) as usize;
                            opt_off += 4 + dash_cnt.saturating_mul(4);
                        }
                        let raw_w = rd_f32(data, 16);
                        let pw = if raw_w.is_finite() && raw_w > 0.0 {
                            (base_sw * raw_w.clamp(0.5, 4.0)).clamp(0.005, 0.10)
                        } else {
                            base_sw
                        };
                        let col = if opt_off + 12 <= data.len() {
                            parse_argb_u32(rd_u32(data, opt_off + 8))
                        } else {
                            RgbaColor::BLACK
                        };
                        objs.insert(obj_id, EmfPlusObj::Pen(col, pw));
                    }
                    3 if data.len() >= 12 => {
                        // Path: `[ver:4][count:u32][pathFlags:u32][points...][types...]`
                        let cnt = rd_u32(data, 4) as usize;
                        let pflags = rd_u32(data, 8);
                        let compressed = (pflags & 0x4000) != 0;
                        let pt_stride = if compressed { 4 } else { 8 };
                        let pts_end = 12usize.saturating_add(cnt.saturating_mul(pt_stride));
                        if cnt > 0 && cnt <= 65_536 && pts_end + cnt <= data.len() {
                            let mut path_pts = Vec::with_capacity(cnt);
                            for i in 0..cnt {
                                let po = 12 + i * pt_stride;
                                let (px, py) = if compressed {
                                    (rd_i16(data, po) as f64, rd_i16(data, po + 2) as f64)
                                } else {
                                    (rd_f32(data, po) as f64, rd_f32(data, po + 4) as f64)
                                };
                                let ptype = data[pts_end + i];
                                path_pts.push((ptype, [px, py]));
                            }
                            objs.insert(obj_id, EmfPlusObj::Path(path_pts));
                        }
                    }
                    _ => {}
                }
            }
            0x402A if data.len() >= 24 => {
                // EmfPlusSetWorldTransform
                *xform = Affine2D {
                    m11: rd_f32(data, 0) as f64,
                    m12: rd_f32(data, 4) as f64,
                    m21: rd_f32(data, 8) as f64,
                    m22: rd_f32(data, 12) as f64,
                    dx: rd_f32(data, 16) as f64,
                    dy: rd_f32(data, 20) as f64,
                };
            }
            0x402B => *xform = Affine2D::default(), // EmfPlusResetWorldTransform
            0x400A | 0x400B | 0x400E | 0x400F => {
                // FillRects (0x400A), DrawRects (0x400B), FillEllipse (0x400E), DrawEllipse (0x400F)
                let is_fill = rtype == 0x400A || rtype == 0x400E;
                let is_ellipse = rtype == 0x400E || rtype == 0x400F;
                let compressed = (flags & 0x4000) != 0;
                let inline_brush = (flags & 0x8000) != 0;

                let (fill_col, stroke_col_w, mut cursor) = if is_fill {
                    if inline_brush && data.len() >= 4 {
                        (Some(parse_argb_u32(rd_u32(data, 0))), None, 4usize)
                    } else if let Some(EmfPlusObj::Brush(c)) = objs.get(&((flags & 0xFF) as u8)) {
                        (Some(*c), None, 0usize)
                    } else {
                        (None, None, 0usize)
                    }
                } else if let Some(EmfPlusObj::Pen(c, w)) = objs.get(&((flags & 0xFF) as u8)) {
                    (None, Some((*c, *w)), 0usize)
                } else {
                    (None, None, 0usize)
                };

                let count = if is_ellipse {
                    1usize
                } else if cursor + 4 <= data.len() {
                    let c = rd_u32(data, cursor) as usize;
                    cursor += 4;
                    c
                } else {
                    0
                };
                let stride = if compressed { 8 } else { 16 };
                for _ in 0..count.min(4096) {
                    if cursor + stride > data.len() {
                        break;
                    }
                    let (rx, ry, rw, rh) = if compressed {
                        (
                            rd_i16(data, cursor) as f64,
                            rd_i16(data, cursor + 2) as f64,
                            rd_i16(data, cursor + 4) as f64,
                            rd_i16(data, cursor + 6) as f64,
                        )
                    } else {
                        (
                            rd_f32(data, cursor) as f64,
                            rd_f32(data, cursor + 4) as f64,
                            rd_f32(data, cursor + 8) as f64,
                            rd_f32(data, cursor + 12) as f64,
                        )
                    };
                    cursor += stride;
                    let mut b = Path::builder();
                    if is_ellipse {
                        let cx = rx + rw * 0.5;
                        let cy = ry + rh * 0.5;
                        b.begin(map_pt(cx + rw * 0.5, cy, xform));
                        for s in 1..24 {
                            let a = std::f64::consts::TAU * (s as f64 / 24.0);
                            b.line_to(map_pt(cx + rw * 0.5 * a.cos(), cy + rh * 0.5 * a.sin(), xform));
                        }
                        b.end(true);
                    } else {
                        b.begin(map_pt(rx, ry, xform));
                        b.line_to(map_pt(rx + rw, ry, xform));
                        b.line_to(map_pt(rx + rw, ry + rh, xform));
                        b.line_to(map_pt(rx, ry + rh, xform));
                        b.end(true);
                    }
                    let p = b.build();
                    if let Some(fc) = fill_col {
                        let rgba = fc.to_linear_f32();
                        let _ = fill_tess.tessellate_path(
                            &p,
                            &FillOptions::tolerance(tol_in),
                            &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                                let pt = v.position();
                                GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                            }),
                        );
                    }
                    if let Some((sc, pw)) = stroke_col_w {
                        let rgba = sc.to_linear_f32();
                        let _ = stroke_tess.tessellate_path(
                            &p,
                            &StrokeOptions::tolerance(tol_in).with_line_width(pw),
                            &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
                                let pt = v.position();
                                GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                            }),
                        );
                    }
                }
            }
            0x400C | 0x400D => {
                // EmfPlusFillPolygon (0x400C) / EmfPlusDrawLines (0x400D)
                let is_fill = rtype == 0x400C;
                let compressed = (flags & 0x4000) != 0;
                let inline_brush = (flags & 0x8000) != 0;
                let closed = is_fill || (flags & 0x2000) != 0;
                let mut cursor = 0usize;
                let fill_col = if is_fill {
                    if inline_brush && data.len() >= 4 {
                        cursor = 4;
                        Some(parse_argb_u32(rd_u32(data, 0)))
                    } else if let Some(EmfPlusObj::Brush(c)) = objs.get(&((flags & 0xFF) as u8)) {
                        Some(*c)
                    } else {
                        None
                    }
                } else {
                    None
                };
                let stroke_col_w = if !is_fill {
                    if let Some(EmfPlusObj::Pen(c, w)) = objs.get(&((flags & 0xFF) as u8)) {
                        Some((*c, *w))
                    } else {
                        None
                    }
                } else {
                    None
                };
                if cursor + 4 <= data.len() {
                    let cnt = rd_u32(data, cursor) as usize;
                    cursor += 4;
                    let stride = if compressed { 4 } else { 8 };
                    if cnt >= 2 && cursor + cnt.saturating_mul(stride) <= data.len() {
                        let mut b = Path::builder();
                        for i in 0..cnt {
                            let po = cursor + i * stride;
                            let (px, py) = if compressed {
                                (rd_i16(data, po) as f64, rd_i16(data, po + 2) as f64)
                            } else {
                                (rd_f32(data, po) as f64, rd_f32(data, po + 4) as f64)
                            };
                            if i == 0 {
                                b.begin(map_pt(px, py, xform));
                            } else {
                                b.line_to(map_pt(px, py, xform));
                            }
                        }
                        b.end(closed);
                        let p = b.build();
                        if let Some(fc) = fill_col {
                            let rgba = fc.to_linear_f32();
                            let _ = fill_tess.tessellate_path(
                                &p,
                                &FillOptions::tolerance(tol_in),
                                &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                                    let pt = v.position();
                                    GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                                }),
                            );
                        }
                        if let Some((sc, pw)) = stroke_col_w {
                            let rgba = sc.to_linear_f32();
                            let _ = stroke_tess.tessellate_path(
                                &p,
                                &StrokeOptions::tolerance(tol_in).with_line_width(pw),
                                &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
                                    let pt = v.position();
                                    GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                                }),
                            );
                        }
                    }
                }
            }
            0x4014 | 0x4015 => {
                // EmfPlusFillPath (0x4014) / EmfPlusDrawPath (0x4015)
                let path_id = (flags & 0xFF) as u8;
                let path_pts_opt = match objs.get(&path_id) {
                    Some(EmfPlusObj::Path(pts)) => Some(pts.clone()),
                    _ => None,
                };
                if let Some(pts) = path_pts_opt {
                    let mut b = Path::builder();
                    let mut sub_open = false;
                    let mut i = 0usize;
                    while i < pts.len() {
                        let (pt_type, p) = pts[i];
                        let kind = pt_type & 0x07;
                        let close_after = (pt_type & 0x80) != 0;
                        match kind {
                            0 => {
                                if sub_open {
                                    b.end(false);
                                }
                                b.begin(map_pt(p[0], p[1], xform));
                                sub_open = true;
                                i += 1;
                            }
                            3 if i + 2 < pts.len() => {
                                if !sub_open {
                                    b.begin(map_pt(p[0], p[1], xform));
                                    sub_open = true;
                                }
                                let p2 = pts[i + 1].1;
                                let (t3, p3) = pts[i + 2];
                                b.cubic_bezier_to(
                                    map_pt(p[0], p[1], xform),
                                    map_pt(p2[0], p2[1], xform),
                                    map_pt(p3[0], p3[1], xform),
                                );
                                if (t3 & 0x80) != 0 && sub_open {
                                    b.end(true);
                                    sub_open = false;
                                }
                                i += 3;
                                continue;
                            }
                            _ => {
                                if !sub_open {
                                    b.begin(map_pt(p[0], p[1], xform));
                                    sub_open = true;
                                } else {
                                    b.line_to(map_pt(p[0], p[1], xform));
                                }
                                i += 1;
                            }
                        }
                        if close_after && sub_open {
                            b.end(true);
                            sub_open = false;
                        }
                    }
                    if sub_open {
                        b.end(false);
                    }
                    let built = b.build();
                    if rtype == 0x4014 {
                        let fill_col = if (flags & 0x8000) != 0 && data.len() >= 4 {
                            Some(parse_argb_u32(rd_u32(data, 0)))
                        } else if data.len() >= 4 {
                            match objs.get(&(rd_u32(data, 0) as u8)) {
                                Some(EmfPlusObj::Brush(c)) => Some(*c),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        if let Some(fc) = fill_col {
                            let rgba = fc.to_linear_f32();
                            let _ = fill_tess.tessellate_path(
                                &built,
                                &FillOptions::tolerance(tol_in).with_fill_rule(FillRule::EvenOdd),
                                &mut BuffersBuilder::new(lyon_buf, |v: FillVertex| {
                                    let pt = v.position();
                                    GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                                }),
                            );
                        }
                    } else if data.len() >= 4 {
                        if let Some(EmfPlusObj::Pen(sc, pw)) = objs.get(&(rd_u32(data, 0) as u8)) {
                            let rgba = sc.to_linear_f32();
                            let _ = stroke_tess.tessellate_path(
                                &built,
                                &StrokeOptions::tolerance(tol_in)
                                    .with_line_width(*pw)
                                    .with_line_cap(LineCap::Round)
                                    .with_line_join(LineJoin::Round),
                                &mut BuffersBuilder::new(lyon_buf, |v: StrokeVertex| {
                                    let pt = v.position();
                                    GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                                }),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
        off += rsize;
    }
}

/// 16-bit Windows Metafile (`WMF`, including 22-byte Aldus Placeable WMF header `0x9AC6CDD7`) vector playback.
fn tessellate_wmf_foreign(bytes: &[u8], world_tf: &ShapeTransform2D, tol_in: f32) -> TessellatedMesh {
    let rd_i16 = |b: &[u8], o: usize| i16::from_le_bytes([b[o], b[o + 1]]);
    let rd_u16 = |b: &[u8], o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
    let rd_u32 = |b: &[u8], o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);

    let mut win_org = [0.0f64, 0.0f64];
    let mut win_ext = [1000.0f64, 1000.0f64];
    let mut hdr_off = 0usize;

    if bytes.len() >= 40 && bytes[0..4] == [0xD7, 0xCD, 0xC6, 0x9A] {
        let l = rd_i16(bytes, 6) as f64;
        let t = rd_i16(bytes, 8) as f64;
        let r = rd_i16(bytes, 10) as f64;
        let b = rd_i16(bytes, 12) as f64;
        if (r - l).abs() > 1.0 && (b - t).abs() > 1.0 {
            win_org = [l, t];
            win_ext = [r - l, b - t];
        }
        hdr_off = 22;
    }
    if hdr_off + 18 > bytes.len() {
        return TessellatedMesh::new();
    }

    let base_sw = (world_tf.width.min(world_tf.height) * 0.022).clamp(0.006, 0.055) as f32;
    let mut cur_stroke: Option<(RgbaColor, f32)> = Some((RgbaColor::BLACK, base_sw));
    let mut cur_fill: Option<RgbaColor> = Some(RgbaColor {
        r: 0,
        g: 120,
        b: 212,
        a: 255,
    });
    let mut cur_pt = win_org;
    let mut gdi_table: Vec<Option<EmfGdiObject>> = Vec::new();

    let alloc_slot = |tbl: &mut Vec<Option<EmfGdiObject>>, obj: EmfGdiObject| {
        if let Some(slot) = tbl.iter_mut().find(|s| s.is_none()) {
            *slot = Some(obj);
        } else if tbl.len() < 1024 {
            tbl.push(Some(obj));
        }
    };

    let map_wmf = |wx: f64, wy: f64, org: [f64; 2], ext: [f64; 2]| -> lyon_path::math::Point {
        let sx = if ext[0].abs() > 1e-6 { ext[0] } else { 1000.0 };
        let sy = if ext[1].abs() > 1e-6 { ext[1] } else { 1000.0 };
        let nx = (wx - org[0]) / sx;
        let ny = 1.0 - (wy - org[1]) / sy;
        let (px, py) = world_tf.local_to_parent(nx * world_tf.width, ny * world_tf.height);
        point(px as f32, py as f32)
    };

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut fill_tess = FillTessellator::new();
    let mut stroke_tess = StrokeTessellator::new();

    let mut pos = hdr_off + 18;
    while pos + 6 <= bytes.len() {
        let size_words = rd_u32(bytes, pos) as usize;
        let func = rd_u16(bytes, pos + 4);
        let rec_bytes = size_words.saturating_mul(2);
        if func == 0 || rec_bytes < 6 || pos + rec_bytes > bytes.len() {
            break;
        }
        let rec = &bytes[pos..pos + rec_bytes];
        match func {
            0x020B if rec.len() >= 10 => {
                // META_SETWINDOWORG (`y`, `x`)
                win_org = [rd_i16(rec, 8) as f64, rd_i16(rec, 6) as f64];
            }
            0x020C if rec.len() >= 10 => {
                // META_SETWINDOWEXT (`y`, `x`)
                win_ext = [rd_i16(rec, 8) as f64, rd_i16(rec, 6) as f64];
            }
            0x02FA if rec.len() >= 16 => {
                // META_CREATEPENINDIRECT
                let style = rd_u16(rec, 6);
                let pen = if (style & 0x0F) == 5 {
                    None
                } else {
                    Some((
                        RgbaColor {
                            r: rec[12],
                            g: rec[13],
                            b: rec[14],
                            a: 255,
                        },
                        base_sw,
                    ))
                };
                alloc_slot(&mut gdi_table, EmfGdiObject::Pen(pen));
            }
            0x02FC if rec.len() >= 14 => {
                // META_CREATEBRUSHINDIRECT
                let style = rd_u16(rec, 6);
                let br = if style == 1 {
                    None
                } else {
                    Some(RgbaColor {
                        r: rec[8],
                        g: rec[9],
                        b: rec[10],
                        a: 255,
                    })
                };
                alloc_slot(&mut gdi_table, EmfGdiObject::Brush(br));
            }
            0x012D if rec.len() >= 8 => {
                // META_SELECTOBJECT
                let idx = rd_u16(rec, 6) as usize;
                if let Some(Some(obj)) = gdi_table.get(idx) {
                    match obj {
                        EmfGdiObject::Pen(p) => cur_stroke = *p,
                        EmfGdiObject::Brush(b) => cur_fill = *b,
                    }
                }
            }
            0x01F0 if rec.len() >= 8 => {
                // META_DELETEOBJECT
                let idx = rd_u16(rec, 6) as usize;
                if let Some(slot) = gdi_table.get_mut(idx) {
                    *slot = None;
                }
            }
            0x0214 if rec.len() >= 10 => {
                // META_MOVETO (`y`, `x`)
                cur_pt = [rd_i16(rec, 8) as f64, rd_i16(rec, 6) as f64];
            }
            0x0213 if rec.len() >= 10 => {
                // META_LINETO (`y`, `x`)
                let nx = rd_i16(rec, 8) as f64;
                let ny = rd_i16(rec, 6) as f64;
                if let Some((sc, pw)) = cur_stroke {
                    let mut b = Path::builder();
                    b.begin(map_wmf(cur_pt[0], cur_pt[1], win_org, win_ext));
                    b.line_to(map_wmf(nx, ny, win_org, win_ext));
                    b.end(false);
                    let rgba = sc.to_linear_f32();
                    let _ = stroke_tess.tessellate_path(
                        &b.build(),
                        &StrokeOptions::tolerance(tol_in).with_line_width(pw),
                        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                            let p = v.position();
                            GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                        }),
                    );
                }
                cur_pt = [nx, ny];
            }
            0x041B | 0x0418 if rec.len() >= 14 => {
                // META_RECTANGLE (0x041B) / META_ELLIPSE (0x0418) (`bottom`, `right`, `top`, `left`)
                let bt = rd_i16(rec, 6) as f64;
                let r = rd_i16(rec, 8) as f64;
                let t = rd_i16(rec, 10) as f64;
                let l = rd_i16(rec, 12) as f64;
                let mut b = Path::builder();
                if func == 0x041B {
                    b.begin(map_wmf(l, t, win_org, win_ext));
                    b.line_to(map_wmf(r, t, win_org, win_ext));
                    b.line_to(map_wmf(r, bt, win_org, win_ext));
                    b.line_to(map_wmf(l, bt, win_org, win_ext));
                    b.end(true);
                } else {
                    let cx = (l + r) * 0.5;
                    let cy = (t + bt) * 0.5;
                    let rx = (r - l).abs() * 0.5;
                    let ry = (bt - t).abs() * 0.5;
                    b.begin(map_wmf(cx + rx, cy, win_org, win_ext));
                    for s in 1..24 {
                        let a = std::f64::consts::TAU * (s as f64 / 24.0);
                        b.line_to(map_wmf(cx + rx * a.cos(), cy + ry * a.sin(), win_org, win_ext));
                    }
                    b.end(true);
                }
                let p = b.build();
                if let Some(fc) = cur_fill {
                    let rgba = fc.to_linear_f32();
                    let _ = fill_tess.tessellate_path(
                        &p,
                        &FillOptions::tolerance(tol_in),
                        &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                            let pt = v.position();
                            GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                        }),
                    );
                }
                if let Some((sc, pw)) = cur_stroke {
                    let rgba = sc.to_linear_f32();
                    let _ = stroke_tess.tessellate_path(
                        &p,
                        &StrokeOptions::tolerance(tol_in).with_line_width(pw),
                        &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                            let pt = v.position();
                            GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                        }),
                    );
                }
            }
            0x0324 | 0x0325 if rec.len() >= 12 => {
                // META_POLYGON (0x0324) / META_POLYLINE (0x0325)
                let cnt = rd_u16(rec, 6) as usize;
                let is_closed = func == 0x0324;
                if cnt >= 2 && 8 + cnt.saturating_mul(4) <= rec.len() {
                    let mut b = Path::builder();
                    for i in 0..cnt {
                        let px = rd_i16(rec, 8 + i * 4) as f64;
                        let py = rd_i16(rec, 10 + i * 4) as f64;
                        if i == 0 {
                            b.begin(map_wmf(px, py, win_org, win_ext));
                        } else {
                            b.line_to(map_wmf(px, py, win_org, win_ext));
                        }
                    }
                    b.end(is_closed);
                    let p = b.build();
                    if is_closed {
                        if let Some(fc) = cur_fill {
                            let rgba = fc.to_linear_f32();
                            let _ = fill_tess.tessellate_path(
                                &p,
                                &FillOptions::tolerance(tol_in).with_fill_rule(FillRule::EvenOdd),
                                &mut BuffersBuilder::new(&mut lyon_buf, |v: FillVertex| {
                                    let pt = v.position();
                                    GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                                }),
                            );
                        }
                    }
                    if let Some((sc, pw)) = cur_stroke {
                        let rgba = sc.to_linear_f32();
                        let _ = stroke_tess.tessellate_path(
                            &p,
                            &StrokeOptions::tolerance(tol_in).with_line_width(pw),
                            &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                                let pt = v.position();
                                GpuVertex::new([pt.x, pt.y], rgba, [PRIM_FLAG_SHAPE, 0.0])
                            }),
                        );
                    }
                }
            }
            _ => {}
        }
        pos += rec_bytes;
    }

    TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    }
}
