//! Sandboxed Macro / Automation Scripting Engine with Visio VBA Object Model Compatibility.
//!
//! Executes deterministic Visio automation scripts (`Sub ... End Sub`, `Dim`, `Set`,
//! `For i = a To b ... Next`, `ActiveDocument.Pages.Add`, `ActivePage.DrawRectangle`,
//! `ActivePage.Drop`, `shp.Text = ...`, `shp.Cells("...").FormulaU = ...`, `shp.AutoConnect`)
//! inside strict CPU instruction and object-count quotas with zero host OS/network access.

use crate::routing::route_page_connectors;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;
use vexio_shapesheet::{GlueEndpoint, ReactiveEngine, Unit, VisioDocument, VisioPage, VisioShape};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutomationError {
    #[error("Sandbox instruction quota ({max_steps}) exceeded — infinite loop terminated safely")]
    QuotaExceeded { max_steps: usize },
    #[error("Syntax error on line {line}: {message}")]
    Syntax { line: usize, message: String },
    #[error("Runtime error: {0}")]
    Runtime(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SandboxLimits {
    pub max_instructions: usize,
    pub max_created_shapes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            max_instructions: 25_000,
            max_created_shapes: 5_000,
        }
    }
}

#[derive(Debug, Clone)]
enum VbaValue {
    Num(f64),
    Str(String),
    PageRef(u32),
    ShapeRef { page_id: u32, shape_id: u64 },
}

impl VbaValue {
    fn as_f64(&self) -> f64 {
        match self {
            Self::Num(n) => *n,
            Self::Str(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }
    fn as_string(&self) -> String {
        match self {
            Self::Num(n) => vexio_shapesheet::format_clean_f64(*n),
            Self::Str(s) => s.clone(),
            Self::PageRef(id) => format!("Page({id})"),
            Self::ShapeRef { shape_id, .. } => format!("Shape({shape_id})"),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AutomationExecutionReport {
    pub instructions_executed: usize,
    pub shapes_created: usize,
    pub log_output: Vec<String>,
}

pub fn execute_vba_macro(
    doc: &mut VisioDocument,
    script: &str,
    limits: SandboxLimits,
) -> Result<AutomationExecutionReport, AutomationError> {
    let raw_lines: Vec<(usize, String)> = script
        .lines()
        .enumerate()
        .map(|(i, l)| {
            // Strip VBA `'` single-quote comments outside double quotes
            let code = strip_vba_comment(l).trim().to_string();
            (i + 1, code)
        })
        .filter(|(_, l)| !l.is_empty())
        .collect();

    let mut active_page_id = doc.pages.first().map(|p| p.id).unwrap_or(1);
    let mut vars: BTreeMap<String, VbaValue> = BTreeMap::new();
    let mut report = AutomationExecutionReport::default();

    #[derive(Clone)]
    struct ForFrame {
        var_name: String,
        end_val: f64,
        step_val: f64,
        body_pc: usize,
    }
    let mut for_stack: Vec<ForFrame> = Vec::new();
    let mut pc = 0usize;

    while pc < raw_lines.len() {
        report.instructions_executed += 1;
        if report.instructions_executed > limits.max_instructions {
            return Err(AutomationError::QuotaExceeded {
                max_steps: limits.max_instructions,
            });
        }

        let (line_no, ref stmt) = raw_lines[pc];
        let upper = stmt.to_ascii_uppercase();

        if upper.starts_with("SUB ")
            || upper.starts_with("PUBLIC SUB ")
            || upper.starts_with("PRIVATE SUB ")
            || upper == "END SUB"
            || upper.starts_with("OPTION ")
            || upper.starts_with("DIM ")
        {
            pc += 1;
            continue;
        }

        if let Some(rest) = strip_prefix_ci(stmt, "Debug.Print") {
            let v = eval_vba_expr(rest.trim(), &vars);
            report.log_output.push(v.as_string());
            pc += 1;
            continue;
        }

        // `For i = 1 To 5 [Step 1]`
        if let Some(rest) = strip_prefix_ci(stmt, "For ") {
            let Some((var_part, range_part)) = rest.split_once('=') else {
                return Err(AutomationError::Syntax {
                    line: line_no,
                    message: "Expected '=' in For loop".into(),
                });
            };
            let vname = var_part.trim().to_ascii_lowercase();
            let Some((start_s, after_to)) = split_once_ci(range_part, " To ") else {
                return Err(AutomationError::Syntax {
                    line: line_no,
                    message: "Expected 'To' in For loop".into(),
                });
            };
            let (end_s, step_val) = if let Some((e, st)) = split_once_ci(after_to, " Step ") {
                (e, eval_vba_expr(st.trim(), &vars).as_f64())
            } else {
                (after_to, 1.0)
            };
            let init_v = eval_vba_expr(start_s.trim(), &vars).as_f64();
            let end_v = eval_vba_expr(end_s.trim(), &vars).as_f64();
            vars.insert(vname.clone(), VbaValue::Num(init_v));
            if (step_val >= 0.0 && init_v > end_v) || (step_val < 0.0 && init_v < end_v) {
                // Skip to matching `Next`
                let mut depth = 1usize;
                pc += 1;
                while pc < raw_lines.len() && depth > 0 {
                    let u = raw_lines[pc].1.to_ascii_uppercase();
                    if u.starts_with("FOR ") {
                        depth += 1;
                    } else if u == "NEXT" || u.starts_with("NEXT ") {
                        depth -= 1;
                    }
                    pc += 1;
                }
                continue;
            }
            for_stack.push(ForFrame {
                var_name: vname,
                end_val: end_v,
                step_val,
                body_pc: pc + 1,
            });
            pc += 1;
            continue;
        }

        if upper == "NEXT" || upper.starts_with("NEXT ") {
            let Some(frame) = for_stack.last().cloned() else {
                return Err(AutomationError::Syntax {
                    line: line_no,
                    message: "Unexpected Next without For".into(),
                });
            };
            let cur = vars
                .get(&frame.var_name)
                .map(|v| v.as_f64())
                .unwrap_or(0.0)
                + frame.step_val;
            vars.insert(frame.var_name.clone(), VbaValue::Num(cur));
            let keep_going = if frame.step_val >= 0.0 {
                cur <= frame.end_val + 1e-9
            } else {
                cur >= frame.end_val - 1e-9
            };
            if keep_going {
                pc = frame.body_pc;
            } else {
                for_stack.pop();
                pc += 1;
            }
            continue;
        }

        // `Set x = ActivePage.DrawRectangle(x1, y1, x2, y2)` or `ActiveDocument.Pages.Add` or `ActivePage.Drop(...)`
        if let Some(rest) = strip_prefix_ci(stmt, "Set ") {
            let Some((lhs, rhs)) = rest.split_once('=') else {
                return Err(AutomationError::Syntax {
                    line: line_no,
                    message: "Expected '=' after Set".into(),
                });
            };
            let var_name = lhs.trim().to_ascii_lowercase();
            let rhs = rhs.trim();
            if rhs.eq_ignore_ascii_case("ActiveDocument.Pages.Add")
                || rhs.eq_ignore_ascii_case("ActiveDocument.Pages.Add()")
            {
                let next_pid = doc.pages.iter().map(|p| p.id).max().unwrap_or(0) + 1;
                doc.pages
                    .push(VisioPage::new(next_pid, format!("Page-{next_pid}"), 11.0, 8.5));
                active_page_id = next_pid;
                vars.insert(var_name, VbaValue::PageRef(next_pid));
            } else if let Some(args_s) = extract_method_call_ci(rhs, "DrawRectangle") {
                if report.shapes_created >= limits.max_created_shapes {
                    return Err(AutomationError::Runtime("Max created shapes quota exceeded".into()));
                }
                let nums = eval_arg_list_f64(args_s, &vars);
                if nums.len() >= 4 {
                    let (x1, y1, x2, y2) = (nums[0], nums[1], nums[2], nums[3]);
                    let w = (x2 - x1).abs().max(0.1);
                    let h = (y2 - y1).abs().max(0.1);
                    let cx = (x1 + x2) * 0.5;
                    let cy = (y1 + y2) * 0.5;
                    let page = doc
                        .page_mut(active_page_id)
                        .ok_or_else(|| AutomationError::Runtime("No active page".into()))?;
                    let sid = page.next_shape_id();
                    page.shapes
                        .push(VisioShape::new_rect(sid, format!("Sheet.{sid}"), cx, cy, w, h));
                    report.shapes_created += 1;
                    vars.insert(
                        var_name,
                        VbaValue::ShapeRef {
                            page_id: active_page_id,
                            shape_id: sid,
                        },
                    );
                }
            } else if let Some(args_s) = extract_method_call_ci(rhs, "Drop") {
                let parts: Vec<&str> = args_s.split(',').collect();
                if parts.len() >= 3 {
                    let m_val = eval_vba_expr(parts[0].trim(), &vars);
                    let px = eval_vba_expr(parts[1].trim(), &vars).as_f64();
                    let py = eval_vba_expr(parts[2].trim(), &vars).as_f64();
                    let mid = m_val.as_f64() as u32;
                    let sid = if let Some(s) = doc.drop_master(active_page_id, mid, px, py) {
                        s
                    } else {
                        let page = doc.page_mut(active_page_id).unwrap();
                        let s = page.next_shape_id();
                        page.shapes
                            .push(VisioShape::new_rect(s, format!("Dropped.{s}"), px, py, 1.5, 1.0));
                        s
                    };
                    report.shapes_created += 1;
                    vars.insert(
                        var_name,
                        VbaValue::ShapeRef {
                            page_id: active_page_id,
                            shape_id: sid,
                        },
                    );
                }
            }
            pc += 1;
            continue;
        }

        // Method call: `shp1.AutoConnect shp2`
        if upper.contains(".AUTOCONNECT") {
            if let Some((obj_part, rest)) = split_once_ci(stmt, ".AutoConnect") {
                let target_arg = rest
                    .trim()
                    .trim_start_matches('(')
                    .trim_end_matches(')')
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .trim();
                let v1 = vars.get(&obj_part.trim().to_ascii_lowercase()).cloned();
                let v2 = vars.get(&target_arg.to_ascii_lowercase()).cloned();
                if let (
                    Some(VbaValue::ShapeRef {
                        page_id: p1,
                        shape_id: s1,
                    }),
                    Some(VbaValue::ShapeRef {
                        page_id: p2,
                        shape_id: s2,
                    }),
                ) = (v1, v2)
                {
                    if p1 == p2 {
                        if let Some(page) = doc.page_mut(p1) {
                            let cid = page.next_shape_id();
                            page.shapes
                                .push(VisioShape::new_connector(cid, "AutoConn", 0.0, 0.0, 1.0, 0.0));
                            page.connects.push(GlueEndpoint {
                                connector_shape_id: cid,
                                from_cell: "BeginX".into(),
                                target_shape_id: s1,
                                to_cell: "PinX".into(),
                                is_dynamic: true,
                            });
                            page.connects.push(GlueEndpoint {
                                connector_shape_id: cid,
                                from_cell: "EndX".into(),
                                target_shape_id: s2,
                                to_cell: "PinX".into(),
                                is_dynamic: true,
                            });
                            report.shapes_created += 1;
                        }
                    }
                }
            }
            pc += 1;
            continue;
        }

        // Property or Cell Assignment:
        // - `shp.Text = "Label"`
        // - `shp.Cells("Width").FormulaU = "3.5 in"`
        // - `pageVar.Name = "Architecture"`
        // - `x = x + 1`
        if let Some((lhs_raw, rhs_raw)) = stmt.split_once('=') {
            let lhs = lhs_raw.trim();
            let rhs_val = eval_vba_expr(rhs_raw.trim(), &vars);

            if let Some((obj_var, after_cells)) = split_once_ci(lhs, ".Cells(") {
                if let Some(end_q) = after_cells.find(')') {
                    let cell_expr = &after_cells[..end_q];
                    let cell_name = eval_vba_expr(cell_expr.trim(), &vars).as_string();
                    if let Some(VbaValue::ShapeRef { page_id, shape_id }) =
                        vars.get(&obj_var.trim().to_ascii_lowercase())
                    {
                        if let Some(sh) = doc.page_mut(*page_id).and_then(|p| p.find_shape_mut(*shape_id)) {
                            let f_str = rhs_val.as_string();
                            let normalized = if f_str.starts_with('=')
                                || vexio_shapesheet::parse_scalar_with_unit(&f_str).is_some()
                            {
                                f_str
                            } else {
                                format!("={f_str}")
                            };
                            sh.sheet.set_formula(cell_name, normalized, None);
                        }
                    }
                }
            } else if let Some((obj_var, prop)) = lhs.split_once('.') {
                let prop_l = prop.trim().to_ascii_lowercase();
                match vars.get(&obj_var.trim().to_ascii_lowercase()).cloned() {
                    Some(VbaValue::ShapeRef { page_id, shape_id }) => {
                        if let Some(sh) = doc.page_mut(page_id).and_then(|p| p.find_shape_mut(shape_id)) {
                            match prop_l.as_str() {
                                "text" => sh.text = rhs_val.as_string(),
                                "name" => sh.name = rhs_val.as_string(),
                                "nameu" => sh.name_u = rhs_val.as_string(),
                                _ => {
                                    sh.sheet.set_formula(prop.trim(), rhs_val.as_string(), Some(Unit::Inch));
                                }
                            }
                        }
                    }
                    Some(VbaValue::PageRef(pid)) => {
                        if let Some(p) = doc.page_mut(pid) {
                            if prop_l == "name" {
                                p.name = rhs_val.as_string();
                                p.name_u = p.name.clone();
                            }
                        }
                    }
                    None => {}
                    _ => {}
                }
            } else {
                vars.insert(lhs.to_ascii_lowercase(), rhs_val);
            }
            pc += 1;
            continue;
        }

        return Err(AutomationError::Syntax {
            line: line_no,
            message: format!("Unsupported VBA statement: {stmt}"),
        });
    }

    route_page_connectors(doc, active_page_id);
    ReactiveEngine::recalculate_document(doc);
    Ok(report)
}

fn strip_vba_comment(line: &str) -> &str {
    let mut in_q = false;
    for (i, ch) in line.char_indices() {
        if ch == '"' {
            in_q = !in_q;
        } else if ch == '\'' && !in_q {
            return &line[..i];
        }
    }
    line
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn split_once_ci<'a>(s: &'a str, needle: &str) -> Option<(&'a str, &'a str)> {
    let pos = s.to_ascii_lowercase().find(&needle.to_ascii_lowercase())?;
    Some((&s[..pos], &s[pos + needle.len()..]))
}

fn extract_method_call_ci<'a>(rhs: &'a str, method: &str) -> Option<&'a str> {
    let pat = format!(".{method}(");
    let pos = rhs.to_ascii_lowercase().find(&pat.to_ascii_lowercase())?;
    let after = &rhs[pos + pat.len()..];
    after.strip_suffix(')')
}

fn eval_arg_list_f64(args_s: &str, vars: &BTreeMap<String, VbaValue>) -> Vec<f64> {
    args_s
        .split(',')
        .map(|a| eval_vba_expr(a.trim(), vars).as_f64())
        .collect()
}

fn eval_vba_expr(expr: &str, vars: &BTreeMap<String, VbaValue>) -> VbaValue {
    let s = expr.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return VbaValue::Str(s[1..s.len() - 1].replace("\"\"", "\""));
    }
    // String concatenation `&`
    if let Some((a, b)) = split_top_level_op(s, '&') {
        return VbaValue::Str(format!(
            "{}{}",
            eval_vba_expr(a, vars).as_string(),
            eval_vba_expr(b, vars).as_string()
        ));
    }
    if let Some((a, b)) = split_top_level_op(s, '+') {
        return VbaValue::Num(eval_vba_expr(a, vars).as_f64() + eval_vba_expr(b, vars).as_f64());
    }
    if let Some((a, b)) = split_top_level_op(s, '-') {
        if !a.trim().is_empty() {
            return VbaValue::Num(eval_vba_expr(a, vars).as_f64() - eval_vba_expr(b, vars).as_f64());
        }
    }
    if let Some((a, b)) = split_top_level_op(s, '*') {
        return VbaValue::Num(eval_vba_expr(a, vars).as_f64() * eval_vba_expr(b, vars).as_f64());
    }
    if let Some((a, b)) = split_top_level_op(s, '/') {
        let denom = eval_vba_expr(b, vars).as_f64();
        return VbaValue::Num(if denom.abs() < 1e-12 {
            0.0
        } else {
            eval_vba_expr(a, vars).as_f64() / denom
        });
    }
    if let Ok(n) = s.parse::<f64>() {
        return VbaValue::Num(n);
    }
    if let Some(v) = vars.get(&s.to_ascii_lowercase()) {
        return v.clone();
    }
    VbaValue::Str(s.to_string())
}

fn split_top_level_op(s: &str, op: char) -> Option<(&str, &str)> {
    let mut in_q = false;
    let mut depth = 0usize;
    let mut last_pos = None;
    for (i, ch) in s.char_indices() {
        if ch == '"' {
            in_q = !in_q;
        } else if !in_q {
            match ch {
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                c if c == op && depth == 0 => last_pos = Some(i),
                _ => {}
            }
        }
    }
    let idx = last_pos?;
    Some((&s[..idx], &s[idx + op.len_utf8()..]))
}
