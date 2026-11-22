//! Pure-Rust, zero-external-font-file deterministic vector stroke & outline glyph shaper
//! and multi-run rich-text tessellator (`LeftToRight`, `RightToLeft` Bidi, and `VerticalTopToBottom` CJK).

use crate::tessellate::{GpuVertex, TessellatedMesh, PRIM_FLAG_TEXT_GLYPH};
use lyon_path::math::point;
use lyon_path::Path;
use lyon_tessellation::{
    BuffersBuilder, LineCap, LineJoin, StrokeOptions, StrokeTessellator, StrokeVertex,
    VertexBuffers,
};
use serde::{Deserialize, Serialize};
use vexio_shapesheet::RgbaColor;

/// Paragraph & block progression direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    VerticalTopToBottom,
}

/// Paragraph list bullet style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BulletStyle {
    None,
    Disc,
    Circle,
    Square,
    Numbered,
}

/// Single styled text run within a `RichTextBlock`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichTextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub font_size_pt: f64,
    pub color: RgbaColor,
}

/// Multi-run rich text block with paragraph alignment, bullets, line spacing, and script direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichTextBlock {
    pub runs: Vec<RichTextRun>,
    pub direction: TextDirection,
    pub bullet: BulletStyle,
    /// `0` = Left, `1` = Center, `2` = Right, `3` = Justify.
    pub horiz_align: u8,
    /// `0` = Top, `1` = Middle, `2` = Bottom.
    pub vert_align: u8,
    pub line_spacing: f64,
}

/// Individual styled character cell produced during line layout and Bidi visual reordering.
#[derive(Debug, Clone, PartialEq)]
pub struct StyledCharCell {
    pub ch: char,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub font_size_pt: f64,
    pub color: RgbaColor,
}

/// Parse lightweight inline markdown / Visio rich-text markup (`**bold**`, `*italic*`,
/// `__underline__`, `~~strike~~`, `[color=#RRGGBB]...[/color]`, `- ` / `• ` / `◦ ` / `▪ ` / `1. `
/// bullets, automatic Hebrew/Arabic RTL detection, and `[vert]` vertical CJK prefix).
pub fn parse_inline_rich_text(
    raw: &str,
    default_size_pt: f64,
    default_color: RgbaColor,
) -> RichTextBlock {
    let size_pt = default_size_pt.clamp(4.0, 288.0);
    let mut working = raw;
    let mut explicit_vertical = false;

    let trimmed_start = working.trim_start_matches([' ', '\t']);
    if let Some(after_vert) = trimmed_start.strip_prefix("[vert]") {
        explicit_vertical = true;
        working = after_vert.strip_prefix(' ').unwrap_or(after_vert);
    }

    // Detect bullet style from paragraph lines and strip leading bullet tokens per line.
    let mut detected_bullet = BulletStyle::None;
    let mut cleaned_lines: Vec<String> = Vec::new();
    for (line_idx, raw_line) in working.split('\n').enumerate() {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        let (line_bullet, stripped) = strip_line_bullet_prefix(line);
        if line_bullet != BulletStyle::None {
            if detected_bullet == BulletStyle::None || line_idx == 0 {
                detected_bullet = line_bullet;
            }
            cleaned_lines.push(stripped.to_string());
        } else {
            cleaned_lines.push(line.to_string());
        }
    }
    let normalized_text = cleaned_lines.join("\n");

    let direction = if explicit_vertical {
        TextDirection::VerticalTopToBottom
    } else if contains_primary_rtl_script(&normalized_text) {
        TextDirection::RightToLeft
    } else {
        TextDirection::LeftToRight
    };

    let horiz_align = match direction {
        TextDirection::RightToLeft => 2,
        TextDirection::LeftToRight if detected_bullet != BulletStyle::None => 0,
        _ => 1,
    };

    let runs = parse_styled_runs(&normalized_text, size_pt, default_color);

    RichTextBlock {
        runs,
        direction,
        bullet: detected_bullet,
        horiz_align,
        vert_align: 1,
        line_spacing: 1.35,
    }
}

/// Parse inline rich text and merge ShapeSheet character/paragraph formatting (`Char.Style`,
/// `Para.HorzAlign`, `VerticalAlign`, `Para.Bullet`).
pub fn parse_inline_rich_text_styled(
    raw: &str,
    default_size_pt: f64,
    default_color: RgbaColor,
    char_style: u8,
    horiz_align: u8,
    vert_align: u8,
    bullet_style: u8,
) -> RichTextBlock {
    let mut block = parse_inline_rich_text(raw, default_size_pt, default_color);
    let base_bold = (char_style & 1) != 0;
    let base_italic = (char_style & 2) != 0;
    let base_underline = (char_style & 4) != 0;
    let base_strike = (char_style & 8) != 0;
    for run in &mut block.runs {
        run.bold |= base_bold;
        run.italic |= base_italic;
        run.underline |= base_underline;
        run.strikethrough |= base_strike;
    }
    if block.bullet == BulletStyle::None {
        block.bullet = match bullet_style {
            1 => BulletStyle::Disc,
            2 => BulletStyle::Numbered,
            3 => BulletStyle::Circle,
            4 => BulletStyle::Square,
            _ => BulletStyle::None,
        };
    }
    if block.direction != TextDirection::RightToLeft || horiz_align != 1 {
        block.horiz_align = horiz_align.min(3);
    }
    block.vert_align = vert_align.min(2);
    block
}

fn strip_line_bullet_prefix(line: &str) -> (BulletStyle, &str) {
    let t = line.trim_start_matches([' ', '\t']);
    if let Some(rest) = t.strip_prefix("- ").or_else(|| t.strip_prefix("• ")) {
        return (BulletStyle::Disc, rest);
    }
    if let Some(rest) = t.strip_prefix("◦ ") {
        return (BulletStyle::Circle, rest);
    }
    if let Some(rest) = t.strip_prefix("▪ ") {
        return (BulletStyle::Square, rest);
    }
    let digit_len = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    if digit_len > 0 && digit_len <= 4 {
        if let Some(rest) = t[digit_len..].strip_prefix(". ") {
            return (BulletStyle::Numbered, rest);
        }
    }
    (BulletStyle::None, line)
}

fn contains_primary_rtl_script(s: &str) -> bool {
    let mut rtl_count = 0usize;
    let mut ltr_count = 0usize;
    for ch in s.chars() {
        if is_rtl_codepoint(ch) {
            rtl_count += 1;
        } else if ch.is_alphabetic() && !is_cjk_codepoint(ch) {
            ltr_count += 1;
        }
    }
    rtl_count > 0 && rtl_count >= ltr_count
}

#[inline]
pub fn is_rtl_codepoint(ch: char) -> bool {
    let u = ch as u32;
    (0x0590..=0x05FF).contains(&u)
        || (0x0600..=0x06FF).contains(&u)
        || (0x0750..=0x077F).contains(&u)
        || (0x08A0..=0x08FF).contains(&u)
        || (0xFB1D..=0xFDFF).contains(&u)
        || (0xFE70..=0xFEFF).contains(&u)
}

#[inline]
pub fn is_cjk_codepoint(ch: char) -> bool {
    let u = ch as u32;
    (0x3000..=0x30FF).contains(&u)
        || (0x3400..=0x4DBF).contains(&u)
        || (0x4E00..=0x9FFF).contains(&u)
        || (0xAC00..=0xD7AF).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFF00..=0xFFEF).contains(&u)
}

fn parse_styled_runs(text: &str, size_pt: f64, default_color: RgbaColor) -> Vec<RichTextRun> {
    if text.is_empty() {
        return Vec::new();
    }

    let mut runs: Vec<RichTextRun> = Vec::new();
    let mut cur_text = String::new();
    let mut bold = false;
    let mut italic = false;
    let mut underline = false;
    let mut strikethrough = false;
    let mut color_stack: Vec<RgbaColor> = vec![default_color];

    let flush = |buf: &mut String,
                 b: bool,
                 i: bool,
                 u: bool,
                 s: bool,
                 col: RgbaColor,
                 out: &mut Vec<RichTextRun>| {
        if buf.is_empty() {
            return;
        }
        let chunk = std::mem::take(buf);
        if let Some(last) = out.last_mut() {
            if last.bold == b
                && last.italic == i
                && last.underline == u
                && last.strikethrough == s
                && last.color == col
                && (last.font_size_pt - size_pt).abs() < 1e-9
            {
                last.text.push_str(&chunk);
                return;
            }
        }
        out.push(RichTextRun {
            text: chunk,
            bold: b,
            italic: i,
            underline: u,
            strikethrough: s,
            font_size_pt: size_pt,
            color: col,
        });
    };

    let mut pos = 0usize;
    while pos < text.len() {
        let rest = &text[pos..];

        // Combined `***` bold+italic toggle.
        if let Some(after) = rest.strip_prefix("***") {
            if (bold && italic) || (!bold && !italic && after.contains("***")) {
                let col = *color_stack.last().unwrap_or(&default_color);
                flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
                bold = !bold;
                italic = !italic;
                pos += 3;
                continue;
            }
        }

        // `**` bold toggle.
        if let Some(after) = rest.strip_prefix("**") {
            if bold || after.contains("**") {
                let col = *color_stack.last().unwrap_or(&default_color);
                flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
                bold = !bold;
                pos += 2;
                continue;
            }
        }

        // `__` underline toggle.
        if let Some(after) = rest.strip_prefix("__") {
            if underline || after.contains("__") {
                let col = *color_stack.last().unwrap_or(&default_color);
                flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
                underline = !underline;
                pos += 2;
                continue;
            }
        }

        // `~~` strikethrough toggle.
        if let Some(after) = rest.strip_prefix("~~") {
            if strikethrough || after.contains("~~") {
                let col = *color_stack.last().unwrap_or(&default_color);
                flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
                strikethrough = !strikethrough;
                pos += 2;
                continue;
            }
        }

        // `[color=#RRGGBB]` and `[/color]` tags (byte-level ASCII check avoids multi-byte UTF-8 boundary panics).
        if rest.as_bytes().first() == Some(&b'[') {
            if rest.len() >= 14 && rest.as_bytes()[..7].eq_ignore_ascii_case(b"[color=") {
                if let Some(close_bracket) = rest[7..].find(']') {
                    let hex_str = &rest[7..7 + close_bracket];
                    let after_open = &rest[7 + close_bracket + 1..];
                    if let Some(parsed_col) = RgbaColor::from_hex(hex_str) {
                        if after_open.to_ascii_lowercase().contains("[/color]") {
                            let col = *color_stack.last().unwrap_or(&default_color);
                            flush(
                                &mut cur_text,
                                bold,
                                italic,
                                underline,
                                strikethrough,
                                col,
                                &mut runs,
                            );
                            color_stack.push(parsed_col);
                            pos += 7 + close_bracket + 1;
                            continue;
                        }
                    }
                }
            }
            if rest.len() >= 8
                && rest.as_bytes()[..8].eq_ignore_ascii_case(b"[/color]")
                && color_stack.len() > 1
            {
                let col = *color_stack.last().unwrap_or(&default_color);
                flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
                color_stack.pop();
                pos += 8;
                continue;
            }
        }

        // `*` italic toggle (only when closing italic or matching `*` exists on the same paragraph).
        if let Some(after) = rest.strip_prefix('*') {
            if !after.starts_with('*') {
                let line_rem = after.split('\n').next().unwrap_or(after);
                if italic || has_single_star(line_rem) {
                    let col = *color_stack.last().unwrap_or(&default_color);
                    flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
                    italic = !italic;
                    pos += 1;
                    continue;
                }
            }
        }

        let ch = rest.chars().next().unwrap();
        cur_text.push(ch);
        pos += ch.len_utf8();
    }

    let col = *color_stack.last().unwrap_or(&default_color);
    flush(&mut cur_text, bold, italic, underline, strikethrough, col, &mut runs);
    runs
}

fn has_single_star(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'*' {
                i += 2;
                continue;
            }
            return true;
        }
        i += 1;
    }
    false
}

// ============================================================================
// Bidi Visual Reordering & Rich Text Box Tessellation
// ============================================================================

/// Reorder a logical sequence of `StyledCharCell`s on a single visual line into left-to-right
/// visual display order according to `base_dir` (reversing RTL runs and mirroring paired brackets
/// while keeping embedded LTR numbers/words left-to-right).
pub fn reorder_bidi_visual_line(
    logical: &[StyledCharCell],
    base_dir: TextDirection,
) -> Vec<StyledCharCell> {
    if logical.is_empty() {
        return Vec::new();
    }

    // Partition logical cells into directional runs `(is_rtl_run, slice)`.
    let default_rtl = base_dir == TextDirection::RightToLeft;
    let mut char_rtl = vec![default_rtl; logical.len()];
    for (idx, cell) in logical.iter().enumerate() {
        if is_rtl_codepoint(cell.ch) {
            char_rtl[idx] = true;
        } else if cell.ch.is_ascii_alphabetic() || cell.ch.is_ascii_digit() {
            char_rtl[idx] = false;
        } else {
            // Neutral punctuation/whitespace inherits surrounding strong direction or `default_rtl`.
            let prev_dir = (0..idx).rev().find_map(|p| {
                let c = logical[p].ch;
                if is_rtl_codepoint(c) {
                    Some(true)
                } else if c.is_ascii_alphanumeric() {
                    Some(false)
                } else {
                    None
                }
            });
            let next_dir = ((idx + 1)..logical.len()).find_map(|n| {
                let c = logical[n].ch;
                if is_rtl_codepoint(c) {
                    Some(true)
                } else if c.is_ascii_alphanumeric() {
                    Some(false)
                } else {
                    None
                }
            });
            char_rtl[idx] = match (prev_dir, next_dir) {
                (Some(a), Some(b)) if a == b => a,
                _ => default_rtl,
            };
        }
    }

    let mut runs: Vec<(bool, Vec<StyledCharCell>)> = Vec::new();
    for (idx, cell) in logical.iter().enumerate() {
        let is_rtl = char_rtl[idx];
        let mut adjusted = cell.clone();
        if is_rtl {
            adjusted.ch = mirror_bidi_bracket(adjusted.ch);
        }
        if let Some(last) = runs.last_mut() {
            if last.0 == is_rtl {
                last.1.push(adjusted);
                continue;
            }
        }
        runs.push((is_rtl, vec![adjusted]));
    }

    // Reverse internal character order of every RTL run.
    for (is_rtl, cells) in &mut runs {
        if *is_rtl {
            cells.reverse();
        }
    }

    // If overall paragraph direction is RTL, reverse the sequence of runs across the line!
    if default_rtl {
        runs.reverse();
    }

    let mut visual = Vec::with_capacity(logical.len());
    for (_, cells) in runs {
        visual.extend(cells);
    }
    visual
}

fn mirror_bidi_bracket(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        _ => ch,
    }
}

/// Tessellate a multi-run `RichTextBlock` inside `(box_w_in, box_h_in)` centered at
/// `(pin_x_in, pin_y_in)` and rotated by `angle_rad`, emitting `TessellatedMesh` triangles
/// tagged with `PRIM_FLAG_TEXT_GLYPH`.
pub fn tessellate_rich_text_in_box(
    block: &RichTextBlock,
    pin_x_in: f64,
    pin_y_in: f64,
    box_w_in: f64,
    box_h_in: f64,
    angle_rad: f64,
) -> TessellatedMesh {
    if block.runs.is_empty()
        || !pin_x_in.is_finite()
        || !pin_y_in.is_finite()
        || !box_w_in.is_finite()
        || !box_h_in.is_finite()
    {
        return TessellatedMesh::new();
    }

    let paragraphs = expand_runs_into_paragraphs(&block.runs);
    if paragraphs.iter().all(Vec::is_empty) {
        return TessellatedMesh::new();
    }

    let eff_w = box_w_in.abs().max(0.15);
    let eff_h = box_h_in.abs().max(0.15);
    let line_spacing = block.line_spacing.clamp(0.85, 3.0);

    let mut commands: Vec<StrokeCommand> = Vec::new();
    if block.direction == TextDirection::VerticalTopToBottom {
        layout_vertical_cjk_columns(block, &paragraphs, eff_w, eff_h, line_spacing, &mut commands);
    } else {
        layout_horizontal_lines(block, &paragraphs, eff_w, eff_h, line_spacing, &mut commands);
    }

    if commands.is_empty() {
        return TessellatedMesh::new();
    }

    let (sin_a, cos_a) = angle_rad.sin_cos();
    let to_page = |lx: f64, ly: f64| -> [f32; 2] {
        let px = pin_x_in + lx * cos_a - ly * sin_a;
        let py = pin_y_in + lx * sin_a + ly * cos_a;
        [px as f32, py as f32]
    };

    let mut lyon_buf: VertexBuffers<GpuVertex, u32> = VertexBuffers::new();
    let mut stroke_tess = StrokeTessellator::new();

    // Coalesce consecutive stroke polylines sharing identical `(color, stroke_w)` into a single Lyon pass.
    let mut idx = 0usize;
    while idx < commands.len() {
        let group_color = commands[idx].color;
        let group_w = commands[idx].stroke_w_in;
        let mut builder = Path::builder();
        let mut count_in_batch = 0usize;

        while idx < commands.len()
            && commands[idx].color == group_color
            && (commands[idx].stroke_w_in - group_w).abs() < 1e-6
        {
            let poly = &commands[idx].polyline_local;
            if poly.len() >= 2 {
                let p0 = to_page(poly[0][0], poly[0][1]);
                builder.begin(point(p0[0], p0[1]));
                for &pt in poly.iter().skip(1) {
                    let p = to_page(pt[0], pt[1]);
                    builder.line_to(point(p[0], p[1]));
                }
                builder.end(false);
                count_in_batch += 1;
            } else if poly.len() == 1 {
                let p0 = to_page(poly[0][0], poly[0][1]);
                let eps = (group_w * 0.25).max(0.001);
                builder.begin(point(p0[0] - eps, p0[1]));
                builder.line_to(point(p0[0] + eps, p0[1]));
                builder.end(false);
                count_in_batch += 1;
            }
            idx += 1;
        }

        if count_in_batch > 0 {
            let path = builder.build();
            let rgba = group_color.to_linear_f32();
            let tol = (group_w * 0.18).clamp(0.0003, 0.006);
            let _ = stroke_tess.tessellate_path(
                &path,
                &StrokeOptions::tolerance(tol)
                    .with_line_width(group_w)
                    .with_line_cap(LineCap::Round)
                    .with_line_join(LineJoin::Round),
                &mut BuffersBuilder::new(&mut lyon_buf, |v: StrokeVertex| {
                    let p = v.position();
                    GpuVertex::new([p.x, p.y], rgba, [PRIM_FLAG_TEXT_GLYPH, 0.0])
                }),
            );
        }
    }

    TessellatedMesh {
        vertices: lyon_buf.vertices,
        indices: lyon_buf.indices,
    }
}

#[derive(Clone, Debug)]
struct StrokeCommand {
    polyline_local: Vec<[f64; 2]>,
    stroke_w_in: f32,
    color: RgbaColor,
}

#[derive(Clone, Debug)]
struct WrappedLine {
    cells: Vec<StyledCharCell>,
    cap_h_in: f64,
    is_para_start: bool,
    para_number: usize,
    para_color: RgbaColor,
}

fn expand_runs_into_paragraphs(runs: &[RichTextRun]) -> Vec<Vec<StyledCharCell>> {
    let mut paragraphs: Vec<Vec<StyledCharCell>> = vec![Vec::new()];
    for run in runs {
        let size_pt = run.font_size_pt.clamp(4.0, 288.0);
        for ch in run.text.chars() {
            if ch == '\r' {
                continue;
            }
            if ch == '\n' {
                paragraphs.push(Vec::new());
            } else {
                paragraphs.last_mut().unwrap().push(StyledCharCell {
                    ch,
                    bold: run.bold,
                    italic: run.italic,
                    underline: run.underline,
                    strikethrough: run.strikethrough,
                    font_size_pt: size_pt,
                    color: run.color,
                });
            }
        }
    }
    paragraphs
}

#[inline]
fn metrics_for_cell(cell: &StyledCharCell) -> (f64, f64, f64, f32) {
    let cap_h = (cell.font_size_pt / 72.0) * 0.72;
    let is_wide = is_cjk_codepoint(cell.ch);
    let glyph_w = if is_wide { cap_h * 0.86 } else { cap_h * 0.58 };
    let advance = glyph_w * if cell.bold { 1.32 } else { 1.26 };
    let base_sw = (cap_h * 0.115).clamp(0.0035, 0.09) as f32;
    let stroke_w = if cell.bold { base_sw * 1.55 } else { base_sw };
    (cap_h, glyph_w, advance, stroke_w)
}

fn layout_horizontal_lines(
    block: &RichTextBlock,
    paragraphs: &[Vec<StyledCharCell>],
    box_w_in: f64,
    box_h_in: f64,
    line_spacing: f64,
    commands: &mut Vec<StrokeCommand>,
) {
    let pad_x = (box_w_in * 0.04).min(0.08);
    let pad_y = (box_h_in * 0.06).min(0.08);
    let default_cap_h = paragraphs
        .iter()
        .flatten()
        .next()
        .map(|c| (c.font_size_pt / 72.0) * 0.72)
        .unwrap_or((10.0 / 72.0) * 0.72);
    let bullet_indent = if block.bullet != BulletStyle::None {
        default_cap_h * 1.65
    } else {
        0.0
    };
    let avail_w = (box_w_in - 2.0 * pad_x - bullet_indent).max(default_cap_h * 1.5);

    let mut wrapped_lines: Vec<WrappedLine> = Vec::new();
    for (p_idx, para) in paragraphs.iter().enumerate() {
        let para_num = p_idx + 1;
        if para.is_empty() {
            wrapped_lines.push(WrappedLine {
                cells: Vec::new(),
                cap_h_in: default_cap_h,
                is_para_start: false,
                para_number: para_num,
                para_color: RgbaColor::BLACK,
            });
            continue;
        }
        let para_col = para[0].color;
        let sub_lines = wrap_paragraph_cells(para, avail_w);
        for (sub_idx, cells) in sub_lines.into_iter().enumerate() {
            let max_cap = cells
                .iter()
                .map(|c| (c.font_size_pt / 72.0) * 0.72)
                .fold(default_cap_h, f64::max);
            wrapped_lines.push(WrappedLine {
                cells,
                cap_h_in: max_cap,
                is_para_start: sub_idx == 0,
                para_number: para_num,
                para_color: para_col,
            });
        }
    }

    if wrapped_lines.is_empty() {
        return;
    }

    let total_h: f64 = wrapped_lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i + 1 == wrapped_lines.len() {
                l.cap_h_in
            } else {
                l.cap_h_in * line_spacing
            }
        })
        .sum();

    let half_h = box_h_in * 0.5;
    let mut cursor_top_y = match block.vert_align {
        0 => (half_h - pad_y).max(total_h * 0.5),
        2 => (-half_h + pad_y + total_h).min(total_h * 0.5),
        _ => total_h * 0.5,
    };
    let half_w = box_w_in * 0.5;

    for line in &wrapped_lines {
        let base_y = cursor_top_y - line.cap_h_in;
        cursor_top_y -= line.cap_h_in * line_spacing;

        if line.cells.is_empty() {
            continue;
        }

        let visual_cells = reorder_bidi_visual_line(&line.cells, block.direction);
        let text_w: f64 = visual_cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let (_, gw, adv, _) = metrics_for_cell(c);
                if i + 1 == visual_cells.len() { gw } else { adv }
            })
            .sum();

        let is_rtl = block.direction == TextDirection::RightToLeft;
        let start_x = match block.horiz_align {
            0 => -half_w + pad_x + if !is_rtl { bullet_indent } else { 0.0 },
            2 => half_w - pad_x - text_w - if is_rtl { bullet_indent } else { 0.0 },
            _ => -0.5 * (text_w + bullet_indent) + if !is_rtl { bullet_indent } else { 0.0 },
        };

        if block.bullet != BulletStyle::None && line.is_para_start {
            let bullet_x = if is_rtl {
                start_x + text_w + line.cap_h_in * 0.45
            } else {
                start_x - bullet_indent + line.cap_h_in * 0.15
            };
            emit_bullet_marker(
                block.bullet,
                line.para_number,
                bullet_x,
                base_y,
                line.cap_h_in,
                line.para_color,
                commands,
            );
        }

        let mut cx = start_x;
        for (ci, cell) in visual_cells.iter().enumerate() {
            let (cap_h, glyph_w, advance, stroke_w) = metrics_for_cell(cell);
            let shear = if cell.italic { 0.22 } else { 0.0 };

            for stroke in glyph_strokes(cell.ch) {
                let mapped: Vec<[f64; 2]> = stroke
                    .into_iter()
                    .map(|pt| {
                        let y_off = (pt[1] as f64) * cap_h;
                        let x_off = (pt[0] as f64) * glyph_w + shear * y_off;
                        [cx + x_off, base_y + y_off]
                    })
                    .collect();
                commands.push(StrokeCommand {
                    polyline_local: mapped,
                    stroke_w_in: stroke_w,
                    color: cell.color,
                });
            }

            let span_w = if ci + 1 == visual_cells.len() {
                glyph_w
            } else {
                advance
            };
            let deco_sw = (stroke_w * 0.78).max(0.003);
            if cell.underline {
                let uy = base_y - 0.12 * cap_h;
                commands.push(StrokeCommand {
                    polyline_local: vec![[cx, uy], [cx + span_w, uy]],
                    stroke_w_in: deco_sw,
                    color: cell.color,
                });
            }
            if cell.strikethrough {
                let sy = base_y + 0.38 * cap_h;
                commands.push(StrokeCommand {
                    polyline_local: vec![[cx, sy], [cx + span_w, sy]],
                    stroke_w_in: deco_sw,
                    color: cell.color,
                });
            }

            cx += advance;
        }
    }
}

fn wrap_paragraph_cells(para: &[StyledCharCell], max_w_in: f64) -> Vec<Vec<StyledCharCell>> {
    // Group into breakable units: whitespace tokens, individual CJK ideographs, and word tokens.
    let mut tokens: Vec<Vec<StyledCharCell>> = Vec::new();
    for cell in para {
        if cell.ch.is_whitespace() || is_cjk_codepoint(cell.ch) {
            tokens.push(vec![cell.clone()]);
        } else if let Some(last) = tokens.last_mut() {
            let prev = &last[0];
            if !prev.ch.is_whitespace() && !is_cjk_codepoint(prev.ch) {
                last.push(cell.clone());
                continue;
            }
            tokens.push(vec![cell.clone()]);
        } else {
            tokens.push(vec![cell.clone()]);
        }
    }

    let token_width = |tok: &[StyledCharCell]| -> f64 {
        tok.iter().map(|c| metrics_for_cell(c).2).sum()
    };

    let mut lines: Vec<Vec<StyledCharCell>> = Vec::new();
    let mut cur_line: Vec<StyledCharCell> = Vec::new();
    let mut cur_w = 0.0;

    for tok in tokens {
        let is_ws = tok.len() == 1 && tok[0].ch.is_whitespace();
        if is_ws && cur_line.is_empty() {
            continue;
        }
        let tw = token_width(&tok);
        if !cur_line.is_empty() && cur_w + tw > max_w_in {
            // Trim trailing spaces on completed line.
            while cur_line.last().is_some_and(|c| c.ch.is_whitespace()) {
                cur_line.pop();
            }
            if !cur_line.is_empty() {
                lines.push(std::mem::take(&mut cur_line));
            }
            cur_w = 0.0;
            if is_ws {
                continue;
            }
        }
        // Hard-break an oversized single word if needed.
        if tw > max_w_in && tok.len() > 1 {
            for cell in tok {
                let cw = metrics_for_cell(&cell).2;
                if !cur_line.is_empty() && cur_w + cw > max_w_in {
                    lines.push(std::mem::take(&mut cur_line));
                    cur_w = 0.0;
                }
                cur_line.push(cell);
                cur_w += cw;
            }
        } else {
            cur_w += tw;
            cur_line.extend(tok);
        }
    }

    while cur_line.last().is_some_and(|c| c.ch.is_whitespace()) {
        cur_line.pop();
    }
    if !cur_line.is_empty() {
        lines.push(cur_line);
    }
    lines
}

fn layout_vertical_cjk_columns(
    block: &RichTextBlock,
    paragraphs: &[Vec<StyledCharCell>],
    box_w_in: f64,
    box_h_in: f64,
    line_spacing: f64,
    commands: &mut Vec<StrokeCommand>,
) {
    let pad_y = (box_h_in * 0.05).min(0.08);
    let default_cap_h = paragraphs
        .iter()
        .flatten()
        .next()
        .map(|c| (c.font_size_pt / 72.0) * 0.72)
        .unwrap_or((10.0 / 72.0) * 0.72);
    let bullet_drop = if block.bullet != BulletStyle::None {
        default_cap_h * 1.4
    } else {
        0.0
    };
    let avail_h = (box_h_in - 2.0 * pad_y - bullet_drop).max(default_cap_h * 1.5);

    // Build vertical columns `(cells, col_w_in, is_para_start, para_num, para_color)`.
    let mut columns: Vec<(Vec<StyledCharCell>, f64, bool, usize, RgbaColor)> = Vec::new();
    for (p_idx, para) in paragraphs.iter().enumerate() {
        let para_num = p_idx + 1;
        if para.is_empty() {
            columns.push((Vec::new(), default_cap_h, false, para_num, RgbaColor::BLACK));
            continue;
        }
        let para_col = para[0].color;
        let mut cur_col: Vec<StyledCharCell> = Vec::new();
        let mut cur_h = 0.0;
        let mut first_col_of_para = true;
        for cell in para {
            let (cap_h, _, _, _) = metrics_for_cell(cell);
            let step_y = cap_h * 1.22;
            if !cur_col.is_empty() && cur_h + step_y > avail_h {
                let col_w = cur_col
                    .iter()
                    .map(|c| metrics_for_cell(c).0)
                    .fold(default_cap_h, f64::max);
                columns.push((
                    std::mem::take(&mut cur_col),
                    col_w,
                    first_col_of_para,
                    para_num,
                    para_col,
                ));
                first_col_of_para = false;
                cur_h = 0.0;
            }
            cur_h += step_y;
            cur_col.push(cell.clone());
        }
        if !cur_col.is_empty() {
            let col_w = cur_col
                .iter()
                .map(|c| metrics_for_cell(c).0)
                .fold(default_cap_h, f64::max);
            columns.push((cur_col, col_w, first_col_of_para, para_num, para_col));
        }
    }

    if columns.is_empty() {
        return;
    }

    let col_pitch_factor = line_spacing * 1.15;
    let total_cols_w: f64 = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if i + 1 == columns.len() {
                c.1
            } else {
                c.1 * col_pitch_factor
            }
        })
        .sum();

    let _ = box_w_in;
    // Rightmost column center X (`col 0`), stepping left (`-X`) for subsequent columns!
    let mut col_center_x = total_cols_w * 0.5 - columns[0].1 * 0.5;

    for (col_idx, (cells, col_w, is_para_start, para_num, para_col)) in columns.iter().enumerate() {
        if !cells.is_empty() {
            let col_h: f64 = cells
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let ch_h = metrics_for_cell(c).0;
                    if i + 1 == cells.len() { ch_h } else { ch_h * 1.22 }
                })
                .sum::<f64>()
                + if block.bullet != BulletStyle::None && *is_para_start {
                    bullet_drop
                } else {
                    0.0
                };

            let mut top_y = col_h * 0.5;
            if block.bullet != BulletStyle::None && *is_para_start {
                let by = top_y - default_cap_h;
                emit_bullet_marker(
                    block.bullet,
                    *para_num,
                    col_center_x - default_cap_h * 0.3,
                    by,
                    default_cap_h,
                    *para_col,
                    commands,
                );
                top_y -= bullet_drop;
            }

            for cell in cells {
                let (cap_h, glyph_w, _, stroke_w) = metrics_for_cell(cell);
                let base_y = top_y - cap_h;
                let gx = col_center_x - glyph_w * 0.5;
                let shear = if cell.italic { 0.22 } else { 0.0 };

                for stroke in glyph_strokes(cell.ch) {
                    let mapped: Vec<[f64; 2]> = stroke
                        .into_iter()
                        .map(|pt| {
                            let y_off = (pt[1] as f64) * cap_h;
                            let x_off = (pt[0] as f64) * glyph_w + shear * y_off;
                            [gx + x_off, base_y + y_off]
                        })
                        .collect();
                    commands.push(StrokeCommand {
                        polyline_local: mapped,
                        stroke_w_in: stroke_w,
                        color: cell.color,
                    });
                }

                let deco_sw = (stroke_w * 0.78).max(0.003);
                if cell.underline {
                    // CJK vertical emphasis/underline sideline along right edge of column cell.
                    let ux = gx + glyph_w * 1.12;
                    commands.push(StrokeCommand {
                        polyline_local: vec![[ux, base_y], [ux, base_y + cap_h]],
                        stroke_w_in: deco_sw,
                        color: cell.color,
                    });
                }
                if cell.strikethrough {
                    let sx = col_center_x;
                    commands.push(StrokeCommand {
                        polyline_local: vec![[sx, base_y], [sx, base_y + cap_h]],
                        stroke_w_in: deco_sw,
                        color: cell.color,
                    });
                }

                top_y -= cap_h * 1.22;
            }
        }

        if col_idx + 1 < columns.len() {
            let next_w = columns[col_idx + 1].1;
            col_center_x -= col_w * (col_pitch_factor - 0.5) + next_w * 0.5;
        }
    }
}

fn emit_bullet_marker(
    style: BulletStyle,
    para_number: usize,
    x_in: f64,
    base_y_in: f64,
    cap_h_in: f64,
    color: RgbaColor,
    commands: &mut Vec<StrokeCommand>,
) {
    let sw = ((cap_h_in * 0.12).clamp(0.0035, 0.08)) as f32;
    let glyph_w = cap_h_in * 0.58;
    match style {
        BulletStyle::None => {}
        BulletStyle::Disc | BulletStyle::Circle | BulletStyle::Square => {
            let sym = match style {
                BulletStyle::Disc => '•',
                BulletStyle::Circle => '◦',
                BulletStyle::Square => '▪',
                _ => '•',
            };
            for stroke in glyph_strokes(sym) {
                let mapped: Vec<[f64; 2]> = stroke
                    .into_iter()
                    .map(|pt| [x_in + (pt[0] as f64) * glyph_w, base_y_in + (pt[1] as f64) * cap_h_in])
                    .collect();
                commands.push(StrokeCommand {
                    polyline_local: mapped,
                    stroke_w_in: if style == BulletStyle::Disc { sw * 1.35 } else { sw },
                    color,
                });
            }
        }
        BulletStyle::Numbered => {
            let label = format!("{para_number}.");
            let mini_w = glyph_w * 0.78;
            let mini_adv = mini_w * 1.15;
            for (i, ch) in label.chars().enumerate() {
                let cx = x_in + (i as f64) * mini_adv;
                for stroke in glyph_strokes(ch) {
                    let mapped: Vec<[f64; 2]> = stroke
                        .into_iter()
                        .map(|pt| {
                            [
                                cx + (pt[0] as f64) * mini_w,
                                base_y_in + (pt[1] as f64) * (cap_h_in * 0.88),
                            ]
                        })
                        .collect();
                    commands.push(StrokeCommand {
                        polyline_local: mapped,
                        stroke_w_in: sw,
                        color,
                    });
                }
            }
        }
    }
}

// ============================================================================
// Built-In Complete Printable ASCII + Symbols + CJK/RTL Vector Glyph Table
// ============================================================================

/// Return normalized polyline strokes (`Vec<Vec<[f32; 2]>>`) in `[0..1] x [-0.25..1.0]` (`+Y` up)
/// for any Unicode scalar `ch` (`100%` deterministic, zero external font files).
pub fn glyph_strokes(ch: char) -> Vec<Vec<[f32; 2]>> {
    if ch.is_whitespace() {
        return Vec::new();
    }

    match ch {
        // --- Uppercase Latin A..=Z ---
        'A' => vec![
            vec![[0.02, 0.0], [0.5, 1.0], [0.98, 0.0]],
            vec![[0.2, 0.36], [0.8, 0.36]],
        ],
        'B' => vec![
            vec![
                [0.06, 0.0],
                [0.06, 1.0],
                [0.72, 1.0],
                [0.92, 0.86],
                [0.92, 0.64],
                [0.72, 0.52],
                [0.06, 0.52],
            ],
            vec![
                [0.72, 0.52],
                [0.96, 0.38],
                [0.96, 0.14],
                [0.74, 0.0],
                [0.06, 0.0],
            ],
        ],
        'C' => vec![vec![
            [0.94, 0.82],
            [0.72, 1.0],
            [0.28, 1.0],
            [0.05, 0.76],
            [0.05, 0.24],
            [0.28, 0.0],
            [0.72, 0.0],
            [0.94, 0.18],
        ]],
        'D' => vec![vec![
            [0.06, 0.0],
            [0.06, 1.0],
            [0.64, 1.0],
            [0.94, 0.74],
            [0.94, 0.26],
            [0.64, 0.0],
            [0.06, 0.0],
        ]],
        'E' => vec![
            vec![[0.92, 1.0], [0.06, 1.0], [0.06, 0.0], [0.92, 0.0]],
            vec![[0.06, 0.52], [0.76, 0.52]],
        ],
        'F' => vec![
            vec![[0.92, 1.0], [0.06, 1.0], [0.06, 0.0]],
            vec![[0.06, 0.52], [0.74, 0.52]],
        ],
        'G' => vec![vec![
            [0.94, 0.82],
            [0.72, 1.0],
            [0.28, 1.0],
            [0.05, 0.76],
            [0.05, 0.24],
            [0.28, 0.0],
            [0.76, 0.0],
            [0.94, 0.2],
            [0.94, 0.46],
            [0.54, 0.46],
        ]],
        'H' => vec![
            vec![[0.06, 0.0], [0.06, 1.0]],
            vec![[0.94, 0.0], [0.94, 1.0]],
            vec![[0.06, 0.52], [0.94, 0.52]],
        ],
        'I' => vec![
            vec![[0.22, 1.0], [0.78, 1.0]],
            vec![[0.5, 1.0], [0.5, 0.0]],
            vec![[0.22, 0.0], [0.78, 0.0]],
        ],
        'J' => vec![
            vec![[0.28, 1.0], [0.92, 1.0]],
            vec![
                [0.76, 1.0],
                [0.76, 0.24],
                [0.56, 0.0],
                [0.24, 0.0],
                [0.06, 0.2],
            ],
        ],
        'K' => vec![
            vec![[0.06, 0.0], [0.06, 1.0]],
            vec![[0.94, 1.0], [0.06, 0.48]],
            vec![[0.32, 0.63], [0.96, 0.0]],
        ],
        'L' => vec![vec![[0.06, 1.0], [0.06, 0.0], [0.92, 0.0]]],
        'M' => vec![vec![
            [0.04, 0.0],
            [0.04, 1.0],
            [0.5, 0.34],
            [0.96, 1.0],
            [0.96, 0.0],
        ]],
        'N' => vec![vec![
            [0.06, 0.0],
            [0.06, 1.0],
            [0.94, 0.0],
            [0.94, 1.0],
        ]],
        'O' => vec![oval_ring(0.5, 0.5, 0.45, 0.5)],
        'P' => vec![vec![
            [0.06, 0.0],
            [0.06, 1.0],
            [0.74, 1.0],
            [0.94, 0.84],
            [0.94, 0.64],
            [0.74, 0.48],
            [0.06, 0.48],
        ]],
        'Q' => vec![
            oval_ring(0.5, 0.5, 0.45, 0.5),
            vec![[0.58, 0.28], [0.98, -0.06]],
        ],
        'R' => vec![
            vec![
                [0.06, 0.0],
                [0.06, 1.0],
                [0.74, 1.0],
                [0.94, 0.84],
                [0.94, 0.64],
                [0.74, 0.48],
                [0.06, 0.48],
            ],
            vec![[0.56, 0.48], [0.96, 0.0]],
        ],
        'S' => vec![vec![
            [0.92, 0.84],
            [0.72, 1.0],
            [0.26, 1.0],
            [0.06, 0.82],
            [0.06, 0.64],
            [0.26, 0.52],
            [0.74, 0.46],
            [0.94, 0.32],
            [0.94, 0.14],
            [0.72, 0.0],
            [0.24, 0.0],
            [0.06, 0.16],
        ]],
        'T' => vec![
            vec![[0.04, 1.0], [0.96, 1.0]],
            vec![[0.5, 1.0], [0.5, 0.0]],
        ],
        'U' => vec![vec![
            [0.06, 1.0],
            [0.06, 0.24],
            [0.28, 0.0],
            [0.72, 0.0],
            [0.94, 0.24],
            [0.94, 1.0],
        ]],
        'V' => vec![vec![[0.04, 1.0], [0.5, 0.0], [0.96, 1.0]]],
        'W' => vec![vec![
            [0.02, 1.0],
            [0.24, 0.0],
            [0.5, 0.64],
            [0.76, 0.0],
            [0.98, 1.0],
        ]],
        'X' => vec![
            vec![[0.06, 1.0], [0.94, 0.0]],
            vec![[0.06, 0.0], [0.94, 1.0]],
        ],
        'Y' => vec![
            vec![[0.04, 1.0], [0.5, 0.48], [0.96, 1.0]],
            vec![[0.5, 0.48], [0.5, 0.0]],
        ],
        'Z' => vec![vec![
            [0.06, 1.0],
            [0.94, 1.0],
            [0.06, 0.0],
            [0.94, 0.0],
        ]],

        // --- Lowercase Latin a..=z ---
        'a' => vec![
            vec![
                [0.16, 0.56],
                [0.34, 0.68],
                [0.72, 0.68],
                [0.88, 0.52],
                [0.88, 0.0],
            ],
            vec![
                [0.88, 0.36],
                [0.34, 0.36],
                [0.12, 0.24],
                [0.12, 0.1],
                [0.32, 0.0],
                [0.72, 0.0],
                [0.88, 0.14],
            ],
        ],
        'b' => vec![
            vec![[0.1, 1.0], [0.1, 0.0]],
            vec![
                [0.1, 0.52],
                [0.32, 0.68],
                [0.7, 0.68],
                [0.9, 0.48],
                [0.9, 0.2],
                [0.7, 0.0],
                [0.32, 0.0],
                [0.1, 0.16],
            ],
        ],
        'c' => vec![vec![
            [0.88, 0.54],
            [0.68, 0.68],
            [0.32, 0.68],
            [0.1, 0.48],
            [0.1, 0.2],
            [0.32, 0.0],
            [0.68, 0.0],
            [0.88, 0.14],
        ]],
        'd' => vec![
            vec![[0.9, 1.0], [0.9, 0.0]],
            vec![
                [0.9, 0.52],
                [0.68, 0.68],
                [0.3, 0.68],
                [0.1, 0.48],
                [0.1, 0.2],
                [0.3, 0.0],
                [0.68, 0.0],
                [0.9, 0.16],
            ],
        ],
        'e' => vec![vec![
            [0.1, 0.34],
            [0.9, 0.34],
            [0.9, 0.5],
            [0.7, 0.68],
            [0.3, 0.68],
            [0.1, 0.48],
            [0.1, 0.2],
            [0.32, 0.0],
            [0.72, 0.0],
            [0.88, 0.12],
        ]],
        'f' => vec![
            vec![[0.82, 0.94], [0.64, 1.0], [0.44, 1.0], [0.34, 0.84], [0.34, 0.0]],
            vec![[0.12, 0.66], [0.68, 0.66]],
        ],
        'g' => vec![
            oval_ring(0.5, 0.34, 0.38, 0.34),
            vec![
                [0.88, 0.68],
                [0.88, -0.1],
                [0.68, -0.24],
                [0.28, -0.24],
                [0.14, -0.14],
            ],
        ],
        'h' => vec![
            vec![[0.1, 1.0], [0.1, 0.0]],
            vec![
                [0.1, 0.5],
                [0.34, 0.68],
                [0.72, 0.68],
                [0.9, 0.5],
                [0.9, 0.0],
            ],
        ],
        'i' => vec![
            vec![[0.5, 0.68], [0.5, 0.0]],
            vec![[0.5, 0.88], [0.5, 0.96]],
        ],
        'j' => vec![
            vec![[0.62, 0.68], [0.62, -0.12], [0.44, -0.24], [0.22, -0.24]],
            vec![[0.62, 0.88], [0.62, 0.96]],
        ],
        'k' => vec![
            vec![[0.12, 1.0], [0.12, 0.0]],
            vec![[0.86, 0.68], [0.12, 0.3]],
            vec![[0.38, 0.42], [0.9, 0.0]],
        ],
        'l' => vec![vec![[0.46, 1.0], [0.46, 0.08], [0.62, 0.0]]],
        'm' => vec![
            vec![[0.06, 0.68], [0.06, 0.0]],
            vec![
                [0.06, 0.52],
                [0.22, 0.68],
                [0.42, 0.68],
                [0.5, 0.52],
                [0.5, 0.0],
            ],
            vec![
                [0.5, 0.52],
                [0.66, 0.68],
                [0.86, 0.68],
                [0.94, 0.52],
                [0.94, 0.0],
            ],
        ],
        'n' => vec![
            vec![[0.1, 0.68], [0.1, 0.0]],
            vec![
                [0.1, 0.5],
                [0.34, 0.68],
                [0.72, 0.68],
                [0.9, 0.5],
                [0.9, 0.0],
            ],
        ],
        'o' => vec![oval_ring(0.5, 0.34, 0.4, 0.34)],
        'p' => vec![
            vec![[0.1, 0.68], [0.1, -0.24]],
            vec![
                [0.1, 0.52],
                [0.32, 0.68],
                [0.7, 0.68],
                [0.9, 0.48],
                [0.9, 0.2],
                [0.7, 0.0],
                [0.32, 0.0],
                [0.1, 0.16],
            ],
        ],
        'q' => vec![
            vec![[0.9, 0.68], [0.9, -0.24]],
            vec![
                [0.9, 0.52],
                [0.68, 0.68],
                [0.3, 0.68],
                [0.1, 0.48],
                [0.1, 0.2],
                [0.3, 0.0],
                [0.68, 0.0],
                [0.9, 0.16],
            ],
        ],
        'r' => vec![
            vec![[0.16, 0.68], [0.16, 0.0]],
            vec![[0.16, 0.48], [0.38, 0.68], [0.72, 0.68], [0.86, 0.58]],
        ],
        's' => vec![vec![
            [0.86, 0.56],
            [0.68, 0.68],
            [0.28, 0.68],
            [0.12, 0.54],
            [0.12, 0.42],
            [0.3, 0.35],
            [0.72, 0.3],
            [0.88, 0.18],
            [0.88, 0.08],
            [0.68, 0.0],
            [0.28, 0.0],
            [0.1, 0.12],
        ]],
        't' => vec![
            vec![[0.44, 0.92], [0.44, 0.12], [0.6, 0.0], [0.78, 0.04]],
            vec![[0.18, 0.68], [0.74, 0.68]],
        ],
        'u' => vec![
            vec![
                [0.1, 0.68],
                [0.1, 0.18],
                [0.3, 0.0],
                [0.68, 0.0],
                [0.9, 0.18],
            ],
            vec![[0.9, 0.68], [0.9, 0.0]],
        ],
        'v' => vec![vec![[0.08, 0.68], [0.5, 0.0], [0.92, 0.68]]],
        'w' => vec![vec![
            [0.04, 0.68],
            [0.24, 0.0],
            [0.5, 0.48],
            [0.76, 0.0],
            [0.96, 0.68],
        ]],
        'x' => vec![
            vec![[0.1, 0.68], [0.9, 0.0]],
            vec![[0.1, 0.0], [0.9, 0.68]],
        ],
        'y' => vec![
            vec![[0.08, 0.68], [0.5, 0.0]],
            vec![[0.92, 0.68], [0.34, -0.24], [0.14, -0.24]],
        ],
        'z' => vec![vec![
            [0.1, 0.68],
            [0.9, 0.68],
            [0.1, 0.0],
            [0.9, 0.0],
        ]],

        // --- Digits 0..=9 ---
        '0' => vec![
            oval_ring(0.5, 0.5, 0.43, 0.5),
            vec![[0.22, 0.2], [0.78, 0.8]],
        ],
        '1' => vec![
            vec![[0.22, 0.78], [0.52, 1.0], [0.52, 0.0]],
            vec![[0.2, 0.0], [0.82, 0.0]],
        ],
        '2' => vec![vec![
            [0.08, 0.78],
            [0.28, 1.0],
            [0.72, 1.0],
            [0.92, 0.82],
            [0.92, 0.62],
            [0.06, 0.0],
            [0.94, 0.0],
        ]],
        '3' => vec![
            vec![
                [0.1, 0.84],
                [0.3, 1.0],
                [0.74, 1.0],
                [0.92, 0.82],
                [0.92, 0.64],
                [0.7, 0.52],
                [0.38, 0.52],
            ],
            vec![
                [0.7, 0.52],
                [0.94, 0.36],
                [0.94, 0.16],
                [0.72, 0.0],
                [0.26, 0.0],
                [0.08, 0.16],
            ],
        ],
        '4' => vec![
            vec![[0.74, 0.0], [0.74, 1.0], [0.06, 0.32], [0.96, 0.32]],
        ],
        '5' => vec![vec![
            [0.9, 1.0],
            [0.12, 1.0],
            [0.08, 0.54],
            [0.68, 0.56],
            [0.92, 0.4],
            [0.92, 0.16],
            [0.7, 0.0],
            [0.24, 0.0],
            [0.08, 0.16],
        ]],
        '6' => vec![
            vec![
                [0.84, 0.9],
                [0.66, 1.0],
                [0.3, 1.0],
                [0.08, 0.72],
                [0.08, 0.24],
                [0.28, 0.0],
                [0.72, 0.0],
                [0.92, 0.22],
                [0.92, 0.42],
                [0.72, 0.56],
                [0.28, 0.56],
                [0.08, 0.4],
            ],
        ],
        '7' => vec![vec![[0.06, 1.0], [0.94, 1.0], [0.36, 0.0]]],
        '8' => vec![
            oval_ring(0.5, 0.76, 0.38, 0.24),
            oval_ring(0.5, 0.26, 0.44, 0.26),
        ],
        '9' => vec![vec![
            [0.92, 0.6],
            [0.72, 0.44],
            [0.28, 0.44],
            [0.08, 0.6],
            [0.08, 0.8],
            [0.28, 1.0],
            [0.72, 1.0],
            [0.92, 0.78],
            [0.92, 0.24],
            [0.7, 0.0],
            [0.26, 0.0],
        ]],

        // --- Printable ASCII Punctuation (Complete 0x21..=0x7E coverage) ---
        '!' => vec![
            vec![[0.5, 1.0], [0.5, 0.26]],
            vec![[0.5, 0.08], [0.5, 0.0]],
        ],
        '"' => vec![
            vec![[0.32, 1.0], [0.32, 0.72]],
            vec![[0.68, 1.0], [0.68, 0.72]],
        ],
        '#' => vec![
            vec![[0.3, 0.0], [0.38, 1.0]],
            vec![[0.62, 0.0], [0.7, 1.0]],
            vec![[0.08, 0.66], [0.92, 0.66]],
            vec![[0.08, 0.34], [0.92, 0.34]],
        ],
        '$' => vec![
            vec![
                [0.88, 0.8],
                [0.7, 0.92],
                [0.3, 0.92],
                [0.12, 0.76],
                [0.28, 0.56],
                [0.72, 0.46],
                [0.88, 0.26],
                [0.7, 0.08],
                [0.3, 0.08],
                [0.12, 0.2],
            ],
            vec![[0.5, 1.06], [0.5, -0.06]],
        ],
        '%' => vec![
            vec![[0.1, 0.0], [0.9, 1.0]],
            oval_ring(0.24, 0.8, 0.16, 0.16),
            oval_ring(0.76, 0.2, 0.16, 0.16),
        ],
        '&' => vec![vec![
            [0.92, 0.0],
            [0.16, 0.72],
            [0.26, 0.96],
            [0.54, 1.0],
            [0.72, 0.84],
            [0.62, 0.64],
            [0.12, 0.32],
            [0.12, 0.12],
            [0.34, 0.0],
            [0.7, 0.0],
            [0.92, 0.32],
        ]],
        '\'' => vec![vec![[0.52, 1.0], [0.46, 0.72]]],
        '(' => vec![vec![
            [0.72, 1.06],
            [0.38, 0.72],
            [0.34, 0.38],
            [0.38, 0.04],
            [0.72, -0.18],
        ]],
        ')' => vec![vec![
            [0.28, 1.06],
            [0.62, 0.72],
            [0.66, 0.38],
            [0.62, 0.04],
            [0.28, -0.18],
        ]],
        '*' => vec![
            vec![[0.5, 0.86], [0.5, 0.26]],
            vec![[0.18, 0.72], [0.82, 0.4]],
            vec![[0.18, 0.4], [0.82, 0.72]],
        ],
        '+' => vec![
            vec![[0.12, 0.5], [0.88, 0.5]],
            vec![[0.5, 0.16], [0.5, 0.84]],
        ],
        ',' => vec![vec![[0.54, 0.12], [0.54, 0.0], [0.38, -0.18]]],
        '-' | '–' => vec![vec![[0.14, 0.48], [0.86, 0.48]]],
        '—' => vec![vec![[0.02, 0.48], [0.98, 0.48]]],
        '.' => vec![vec![[0.46, 0.04], [0.54, 0.04], [0.54, 0.0], [0.46, 0.0]]],
        '/' => vec![vec![[0.08, -0.06], [0.92, 1.04]]],
        ':' => vec![
            vec![[0.48, 0.66], [0.52, 0.66]],
            vec![[0.48, 0.12], [0.52, 0.12]],
        ],
        ';' => vec![
            vec![[0.48, 0.66], [0.52, 0.66]],
            vec![[0.52, 0.14], [0.52, 0.0], [0.38, -0.16]],
        ],
        '<' => vec![vec![[0.84, 0.82], [0.16, 0.5], [0.84, 0.18]]],
        '=' => vec![
            vec![[0.12, 0.64], [0.88, 0.64]],
            vec![[0.12, 0.34], [0.88, 0.34]],
        ],
        '>' => vec![vec![[0.16, 0.82], [0.84, 0.5], [0.16, 0.18]]],
        '?' => vec![
            vec![
                [0.14, 0.78],
                [0.32, 1.0],
                [0.7, 1.0],
                [0.88, 0.82],
                [0.88, 0.64],
                [0.5, 0.44],
                [0.5, 0.26],
            ],
            vec![[0.5, 0.08], [0.5, 0.0]],
        ],
        '@' => vec![
            oval_ring(0.52, 0.48, 0.2, 0.22),
            vec![
                [0.72, 0.68],
                [0.72, 0.3],
                [0.84, 0.26],
                [0.96, 0.48],
                [0.88, 0.84],
                [0.52, 0.98],
                [0.14, 0.82],
                [0.06, 0.48],
                [0.18, 0.12],
                [0.54, 0.0],
                [0.82, 0.06],
            ],
        ],
        '[' => vec![vec![
            [0.72, 1.04],
            [0.32, 1.04],
            [0.32, -0.14],
            [0.72, -0.14],
        ]],
        '\\' => vec![vec![[0.08, 1.04], [0.92, -0.06]]],
        ']' => vec![vec![
            [0.28, 1.04],
            [0.68, 1.04],
            [0.68, -0.14],
            [0.28, -0.14],
        ]],
        '^' => vec![vec![[0.16, 0.64], [0.5, 1.0], [0.84, 0.64]]],
        '_' => vec![vec![[0.02, -0.08], [0.98, -0.08]]],
        '`' => vec![vec![[0.38, 1.0], [0.58, 0.76]]],
        '{' => vec![vec![
            [0.72, 1.04],
            [0.48, 0.96],
            [0.48, 0.56],
            [0.26, 0.46],
            [0.48, 0.36],
            [0.48, -0.06],
            [0.72, -0.14],
        ]],
        '|' => vec![vec![[0.5, 1.06], [0.5, -0.16]]],
        '}' => vec![vec![
            [0.28, 1.04],
            [0.52, 0.96],
            [0.52, 0.56],
            [0.74, 0.46],
            [0.52, 0.36],
            [0.52, -0.06],
            [0.28, -0.14],
        ]],
        '~' => vec![vec![
            [0.08, 0.46],
            [0.28, 0.64],
            [0.72, 0.36],
            [0.92, 0.54],
        ]],

        // --- Bullets, Arrows & Mathematical Symbols ---
        '•' => vec![
            oval_ring(0.5, 0.48, 0.22, 0.22),
            oval_ring(0.5, 0.48, 0.1, 0.1),
        ],
        '◦' => vec![oval_ring(0.5, 0.48, 0.24, 0.24)],
        '▪' => vec![
            vec![
                [0.26, 0.26],
                [0.74, 0.26],
                [0.74, 0.7],
                [0.26, 0.7],
                [0.26, 0.26],
            ],
            vec![[0.38, 0.38], [0.62, 0.38], [0.62, 0.58], [0.38, 0.58], [0.38, 0.38]],
        ],
        '→' => vec![
            vec![[0.06, 0.48], [0.94, 0.48]],
            vec![[0.64, 0.74], [0.94, 0.48], [0.64, 0.22]],
        ],
        '←' => vec![
            vec![[0.94, 0.48], [0.06, 0.48]],
            vec![[0.36, 0.74], [0.06, 0.48], [0.36, 0.22]],
        ],
        '↔' => vec![
            vec![[0.06, 0.48], [0.94, 0.48]],
            vec![[0.32, 0.72], [0.06, 0.48], [0.32, 0.24]],
            vec![[0.68, 0.72], [0.94, 0.48], [0.68, 0.24]],
        ],
        '↑' => vec![
            vec![[0.5, 0.04], [0.5, 0.96]],
            vec![[0.24, 0.68], [0.5, 0.96], [0.76, 0.68]],
        ],
        '↓' => vec![
            vec![[0.5, 0.96], [0.5, 0.04]],
            vec![[0.24, 0.32], [0.5, 0.04], [0.76, 0.32]],
        ],
        '✓' => vec![vec![[0.08, 0.42], [0.38, 0.08], [0.94, 0.92]]],
        '✗' => vec![
            vec![[0.12, 0.88], [0.88, 0.12]],
            vec![[0.12, 0.12], [0.88, 0.88]],
        ],
        '°' => vec![oval_ring(0.5, 0.82, 0.18, 0.18)],
        '±' => vec![
            vec![[0.14, 0.62], [0.86, 0.62]],
            vec![[0.5, 0.34], [0.5, 0.9]],
            vec![[0.14, 0.14], [0.86, 0.14]],
        ],
        '≤' => vec![
            vec![[0.84, 0.88], [0.16, 0.58], [0.84, 0.28]],
            vec![[0.16, 0.1], [0.84, 0.1]],
        ],
        '≥' => vec![
            vec![[0.16, 0.88], [0.84, 0.58], [0.16, 0.28]],
            vec![[0.16, 0.1], [0.84, 0.1]],
        ],
        '≠' => vec![
            vec![[0.12, 0.64], [0.88, 0.64]],
            vec![[0.12, 0.34], [0.88, 0.34]],
            vec![[0.24, 0.1], [0.76, 0.88]],
        ],
        '×' => vec![
            vec![[0.16, 0.8], [0.84, 0.2]],
            vec![[0.16, 0.2], [0.84, 0.8]],
        ],
        '÷' => vec![
            vec![[0.12, 0.5], [0.88, 0.5]],
            vec![[0.48, 0.78], [0.52, 0.78]],
            vec![[0.48, 0.22], [0.52, 0.22]],
        ],
        '…' => vec![
            vec![[0.14, 0.04], [0.2, 0.04]],
            vec![[0.47, 0.04], [0.53, 0.04]],
            vec![[0.8, 0.04], [0.86, 0.04]],
        ],

        other => extended_unicode_glyph_strokes(other),
    }
}

fn oval_ring(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<[f32; 2]> {
    const STEPS: usize = 12;
    let mut ring = Vec::with_capacity(STEPS + 1);
    for i in 0..=STEPS {
        let ang = std::f32::consts::TAU * (i as f32 / STEPS as f32);
        let (s, c) = ang.sin_cos();
        ring.push([cx + rx * c, cy + ry * s]);
    }
    ring
}

/// Deterministic geometric vector glyphs for accented Latin, Hebrew, Arabic, CJK ideographs,
/// and all extended Unicode planes so every character renders crisply with zero blanks.
fn extended_unicode_glyph_strokes(ch: char) -> Vec<Vec<[f32; 2]>> {
    let cp = ch as u32;

    // Latin-1 / Latin Extended accented characters: combine base ASCII letter + diacritic.
    if let Some((base, accent)) = decompose_latin_accent(ch) {
        let mut strokes = glyph_strokes(base);
        let top = if base.is_ascii_uppercase() { 1.12 } else { 0.84 };
        match accent {
            1 => strokes.push(vec![[0.42, top], [0.68, top + 0.14]]), // acute
            2 => strokes.push(vec![[0.32, top + 0.14], [0.58, top]]), // grave
            3 => strokes.push(vec![[0.28, top], [0.5, top + 0.14], [0.72, top]]), // circumflex
            4 => {
                // diaeresis / umlaut
                strokes.push(vec![[0.34, top + 0.06], [0.38, top + 0.06]]);
                strokes.push(vec![[0.62, top + 0.06], [0.66, top + 0.06]]);
            }
            5 => strokes.push(vec![
                [0.26, top + 0.04],
                [0.42, top + 0.12],
                [0.58, top],
                [0.74, top + 0.08],
            ]), // tilde
            _ => strokes.push(vec![[0.5, 0.0], [0.44, -0.16], [0.58, -0.2]]), // cedilla
        }
        return strokes;
    }

    // Hebrew (`0x0590..=0x05FF`) & Arabic (`0x0600..=0x06FF`) calligraphic stroke synthesis.
    if is_rtl_codepoint(ch) {
        let h = cp.wrapping_mul(0x9E37_79B9);
        let arch_y = 0.68 + ((h & 0x7) as f32) * 0.03;
        let base_y = 0.08 + (((h >> 3) & 0x3) as f32) * 0.04;
        let stem_x = 0.24 + (((h >> 5) & 0x7) as f32) * 0.08;
        let mut strokes = vec![vec![
            [0.12, arch_y],
            [0.86, arch_y],
            [0.86, base_y],
            [0.16, base_y],
        ]];
        strokes.push(vec![[stem_x, arch_y * 0.85], [stem_x, base_y]]);
        if ((h >> 8) & 1) != 0 {
            strokes.push(vec![[0.48, arch_y + 0.14], [0.54, arch_y + 0.14]]);
        }
        return strokes;
    }

    // CJK ideographs / Kana / Hangul: deterministic radical + frame + inner strokes.
    if is_cjk_codepoint(ch) {
        let h = cp.wrapping_mul(0x85EB_CA6B);
        let mut strokes = Vec::with_capacity(5);
        // Outer / radical structure
        match h & 0x3 {
            0 => {
                // Enclosure box + crossbar (`口` / `日` / `国` motif)
                strokes.push(vec![
                    [0.12, 0.08],
                    [0.12, 0.92],
                    [0.88, 0.92],
                    [0.88, 0.08],
                    [0.12, 0.08],
                ]);
                strokes.push(vec![[0.18, 0.5], [0.82, 0.5]]);
            }
            1 => {
                // Left radical + right component (`木` / `東` / `漢` motif)
                strokes.push(vec![[0.1, 0.68], [0.38, 0.68]]);
                strokes.push(vec![[0.24, 0.94], [0.24, 0.06]]);
                strokes.push(vec![[0.46, 0.88], [0.9, 0.88]]);
                strokes.push(vec![[0.68, 0.94], [0.68, 0.08]]);
            }
            2 => {
                // Top crown + sweeping strokes (`京` / `大` / `文` motif)
                strokes.push(vec![[0.5, 0.96], [0.5, 0.82]]);
                strokes.push(vec![[0.08, 0.82], [0.92, 0.82]]);
                strokes.push(vec![[0.5, 0.76], [0.12, 0.08]]);
                strokes.push(vec![[0.5, 0.76], [0.88, 0.08]]);
            }
            _ => {
                // Horizontal + vertical grid (`田` / `本` motif)
                strokes.push(vec![[0.1, 0.78], [0.9, 0.78]]);
                strokes.push(vec![[0.16, 0.46], [0.84, 0.46]]);
                strokes.push(vec![[0.08, 0.12], [0.92, 0.12]]);
                strokes.push(vec![[0.5, 0.94], [0.5, 0.06]]);
            }
        }
        // Codepoint-specific distinguishing interior stroke so different characters look distinct!
        let sx = 0.22 + (((h >> 4) & 0x7) as f32) * 0.07;
        let ex = 0.82 - (((h >> 7) & 0x5) as f32) * 0.06;
        let sy = 0.24 + (((h >> 10) & 0x7) as f32) * 0.06;
        strokes.push(vec![[sx, sy], [ex, (sy + 0.18).min(0.88)]]);
        return strokes;
    }

    // Universal deterministic geometric glyph for any remaining Unicode symbol.
    let h = cp.wrapping_mul(0xC2B2_AE35);
    let mid_y = 0.3 + ((h & 0x7) as f32) * 0.05;
    vec![
        vec![
            [0.14, 0.1],
            [0.14, 0.9],
            [0.86, 0.9],
            [0.86, 0.1],
            [0.14, 0.1],
        ],
        vec![[0.24, mid_y], [0.76, 1.0 - mid_y]],
    ]
}

fn decompose_latin_accent(ch: char) -> Option<(char, u8)> {
    match ch {
        'Á' => Some(('A', 1)),
        'À' => Some(('A', 2)),
        'Â' => Some(('A', 3)),
        'Ä' => Some(('A', 4)),
        'Ã' => Some(('A', 5)),
        'á' => Some(('a', 1)),
        'à' => Some(('a', 2)),
        'â' => Some(('a', 3)),
        'ä' => Some(('a', 4)),
        'ã' => Some(('a', 5)),
        'Ç' => Some(('C', 6)),
        'ç' => Some(('c', 6)),
        'É' => Some(('E', 1)),
        'È' => Some(('E', 2)),
        'Ê' => Some(('E', 3)),
        'Ë' => Some(('E', 4)),
        'é' => Some(('e', 1)),
        'è' => Some(('e', 2)),
        'ê' => Some(('e', 3)),
        'ë' => Some(('e', 4)),
        'Í' => Some(('I', 1)),
        'Ì' => Some(('I', 2)),
        'Î' => Some(('I', 3)),
        'Ï' => Some(('I', 4)),
        'í' => Some(('i', 1)),
        'ì' => Some(('i', 2)),
        'î' => Some(('i', 3)),
        'ï' => Some(('i', 4)),
        'Ñ' => Some(('N', 5)),
        'ñ' => Some(('n', 5)),
        'Ó' => Some(('O', 1)),
        'Ò' => Some(('O', 2)),
        'Ô' => Some(('O', 3)),
        'Ö' => Some(('O', 4)),
        'Õ' => Some(('O', 5)),
        'ó' => Some(('o', 1)),
        'ò' => Some(('o', 2)),
        'ô' => Some(('o', 3)),
        'ö' => Some(('o', 4)),
        'õ' => Some(('o', 5)),
        'Ú' => Some(('U', 1)),
        'Ù' => Some(('U', 2)),
        'Û' => Some(('U', 3)),
        'Ü' => Some(('U', 4)),
        'ú' => Some(('u', 1)),
        'ù' => Some(('u', 2)),
        'û' => Some(('u', 3)),
        'ü' => Some(('u', 4)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn printable_ascii_and_unicode_glyph_table_has_zero_missing_glyphs() {
        assert!(glyph_strokes(' ').is_empty());
        for code in 0x21u8..=0x7Eu8 {
            let ch = code as char;
            let strokes = glyph_strokes(ch);
            assert!(
                !strokes.is_empty(),
                "Printable ASCII '{ch}' (0x{code:02X}) must have vector strokes"
            );
            for poly in strokes {
                assert!(poly.len() >= 2);
            }
        }
        for sym in ['•', '◦', '▪', '→', '←', '↔', '✓', '✗', '°', '±', '≤', '≥', 'א', 'م', '東'] {
            assert!(!glyph_strokes(sym).is_empty(), "Symbol '{sym}' must render");
        }
    }

    #[test]
    fn inline_rich_text_parses_plain_bold_italic_underline_strike_color_and_bullets() {
        let plain = parse_inline_rich_text("Server Node * 01", 11.0, RgbaColor::BLACK);
        assert_eq!(plain.bullet, BulletStyle::None);
        assert_eq!(plain.runs.len(), 1);
        assert_eq!(plain.runs[0].text, "Server Node * 01");
        assert!(!tessellate_rich_text_in_box(&plain, 2.0, 2.0, 2.5, 1.0, 0.0).is_empty());

        let numbered = parse_inline_rich_text("1. Gather\n2. Validate", 10.0, RgbaColor::BLACK);
        assert_eq!(numbered.bullet, BulletStyle::Numbered);
        assert!(!tessellate_rich_text_in_box(&numbered, 3.0, 3.0, 2.5, 1.5, 0.0).is_empty());

        let block = parse_inline_rich_text(
            "- **Bold** *Italic* __Under__ ~~Strike~~ [color=#2563EB]Blue[/color]",
            12.0,
            RgbaColor::BLACK,
        );
        assert_eq!(block.direction, TextDirection::LeftToRight);
        assert_eq!(block.bullet, BulletStyle::Disc);
        assert!(block.runs.iter().any(|r| r.bold && r.text == "Bold"));
        assert!(block.runs.iter().any(|r| r.italic && r.text == "Italic"));
        assert!(block.runs.iter().any(|r| r.underline && r.text == "Under"));
        assert!(block.runs.iter().any(|r| r.strikethrough && r.text == "Strike"));
        assert!(block.runs.iter().any(|r| {
            r.text == "Blue"
                && r.color
                    == RgbaColor {
                        r: 0x25,
                        g: 0x63,
                        b: 0xEB,
                        a: 255,
                    }
        }));

        let mesh = tessellate_rich_text_in_box(&block, 4.0, 3.0, 3.0, 1.5, 0.1);
        assert!(!mesh.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);
        for v in &mesh.vertices {
            assert_eq!(v.flags[0], PRIM_FLAG_TEXT_GLYPH);
            assert!(v.pos[0].is_finite() && v.pos[1].is_finite());
        }
    }

    #[test]
    fn rtl_arabic_and_hebrew_detection_visual_reordering_and_tessellation() {
        // Logical Hebrew `"אבג 123"` -> RTL paragraph places `"123"` on the visual left and `"גבא"` on the visual right!
        let heb = parse_inline_rich_text("אבג 123", 14.0, RgbaColor::BLACK);
        assert_eq!(heb.direction, TextDirection::RightToLeft);

        let paras = expand_runs_into_paragraphs(&heb.runs);
        let visual = reorder_bidi_visual_line(&paras[0], heb.direction);
        let visual_str: String = visual.iter().map(|c| c.ch).collect();
        assert_eq!(visual_str, "123 גבא");

        let arab = parse_inline_rich_text("مرحبا بالعالم", 14.0, RgbaColor::BLACK);
        assert_eq!(arab.direction, TextDirection::RightToLeft);
        let arab_mesh = tessellate_rich_text_in_box(&arab, 5.0, 4.0, 4.0, 1.5, 0.0);
        assert!(!arab_mesh.is_empty());
        assert_eq!(arab_mesh.indices.len() % 3, 0);
    }

    #[test]
    fn vertical_cjk_layout_progresses_top_to_bottom_and_columns_right_to_left() {
        let vert = parse_inline_rich_text("[vert]東京\n大阪", 16.0, RgbaColor::BLACK);
        assert_eq!(vert.direction, TextDirection::VerticalTopToBottom);
        assert_eq!(vert.runs.len(), 1);
        assert_eq!(vert.runs[0].text, "東京\n大阪");

        let mesh = tessellate_rich_text_in_box(&vert, 3.0, 3.0, 2.5, 3.0, 0.0);
        assert!(!mesh.is_empty());
        assert_eq!(mesh.indices.len() % 3, 0);

        // Verify span covers both columns across X around `pin_x = 3.0` and vertical span across Y around `pin_y = 3.0`.
        let min_x = mesh
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold(f32::INFINITY, f32::min);
        let max_x = mesh
            .vertices
            .iter()
            .map(|v| v.pos[0])
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = mesh
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(f32::INFINITY, f32::min);
        let max_y = mesh
            .vertices
            .iter()
            .map(|v| v.pos[1])
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(min_x < 3.0 && max_x > 3.0);
        assert!(min_y < 3.0 && max_y > 3.0);
    }
}
