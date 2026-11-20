//! Sandboxed Macro / Automation Scripting Engine with Visio VBA Object Model Compatibility.
//!
//! Executes deterministic Visio automation scripts:
//! - `Sub ... End Sub`, `If ... Then / Else / End If`, `Dim`, `Set`
//! - `For i = a To b [Step s] ... Next`
//! - `For Each shp In ActivePage.Shapes ... Next` & `For Each shp In ActiveWindow.Selection ... Next`
//! - `ActiveDocument.Pages.Add`, `ActivePage.DrawRectangle`, `ActivePage.Drop`, `shp.AutoConnect`
//! - `ActiveWindow.Zoom = ...`, `shp.Text = ...`, `shp.Cells("...").FormulaU = ...`
//! - `UserForm` declarations (`Begin UserForm <Name> ... End`) & control bindings (`TextBox`, `ComboBox`, `CheckBox`, `CommandButton`)
//! - Document Event Sinks (`Document_ShapeAdded`, `Document_CellChanged`) via `fire_vba_document_event`
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VbaUserFormControl {
    pub kind: String, // "TextBox" | "ComboBox" | "CheckBox" | "CommandButton" | "Label"
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VbaUserFormDef {
    pub name: String,
    pub caption: String,
    pub shown: bool,
    pub controls: Vec<VbaUserFormControl>,
}

#[derive(Debug, Clone)]
pub enum VbaValue {
    Num(f64),
    Bool(bool),
    Str(String),
    PageRef(u32),
    ShapeRef { page_id: u32, shape_id: u64 },
}

impl VbaValue {
    fn as_f64(&self) -> f64 {
        match self {
            Self::Num(n) => *n,
            Self::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            Self::Str(s) => s.parse().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    fn as_bool(&self) -> bool {
        match self {
            Self::Bool(b) => *b,
            Self::Num(n) => n.abs() > 1e-12,
            Self::Str(s) => {
                let u = s.trim().to_ascii_uppercase();
                u == "TRUE" || u == "1" || u == "YES"
            }
            _ => true,
        }
    }

    fn as_string(&self) -> String {
        match self {
            Self::Num(n) => vexio_shapesheet::format_clean_f64(*n),
            Self::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
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
    pub window_zoom: Option<f64>,
    pub user_forms: Vec<VbaUserFormDef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VbaDocumentEvent<'a> {
    ShapeAdded { page_id: u32, shape_id: u64 },
    CellChanged { page_id: u32, shape_id: u64, cell_name: &'a str },
}

pub fn execute_vba_macro(
    doc: &mut VisioDocument,
    script: &str,
    limits: SandboxLimits,
) -> Result<AutomationExecutionReport, AutomationError> {
    execute_vba_macro_with_selection(doc, script, &[], None, limits)
}

/// Fire a Visio VBA Document Event Sink (`Document_ShapeAdded` or `Document_CellChanged`)
/// against a macro module without running unrelated top-level procedures.
pub fn fire_vba_document_event(
    doc: &mut VisioDocument,
    module_script: &str,
    event: VbaDocumentEvent<'_>,
    limits: SandboxLimits,
) -> Result<AutomationExecutionReport, AutomationError> {
    let target_sub = match event {
        VbaDocumentEvent::ShapeAdded { .. } => "document_shapeadded",
        VbaDocumentEvent::CellChanged { .. } => "document_cellchanged",
    };
    let mut sub_body = Vec::new();
    let mut inside_target = false;
    let mut bound_param_name = String::from("shape");

    for raw in module_script.lines() {
        let code = strip_vba_comment(raw).trim();
        let lower = code.to_ascii_lowercase();
        if !inside_target {
            if (lower.starts_with("sub ") || lower.starts_with("private sub ") || lower.starts_with("public sub "))
                && lower.contains(target_sub)
            {
                inside_target = true;
                if let (Some(lp), Some(rp)) = (code.find('('), code.find(')')) {
                    let param_spec = code[lp + 1..rp].trim();
                    let tokens: Vec<&str> = param_spec.split_whitespace().collect();
                    for (idx, tok) in tokens.iter().enumerate() {
                        if tok.eq_ignore_ascii_case("As") && idx > 0 {
                            bound_param_name = tokens[idx - 1].to_ascii_lowercase();
                            break;
                        } else if idx == 0 && !tok.eq_ignore_ascii_case("ByVal") && !tok.eq_ignore_ascii_case("ByRef") {
                            bound_param_name = tok.to_ascii_lowercase();
                        }
                    }
                }
                sub_body.push(code.to_string());
            }
        } else {
            sub_body.push(code.to_string());
            if lower == "end sub" {
                break;
            }
        }
    }

    if sub_body.is_empty() {
        return Ok(AutomationExecutionReport::default());
    }

    let mut initial_vars = BTreeMap::new();
    match event {
        VbaDocumentEvent::ShapeAdded { page_id, shape_id } => {
            initial_vars.insert(bound_param_name, VbaValue::ShapeRef { page_id, shape_id });
        }
        VbaDocumentEvent::CellChanged {
            page_id,
            shape_id,
            cell_name,
        } => {
            initial_vars.insert("shape".into(), VbaValue::ShapeRef { page_id, shape_id });
            initial_vars.insert(bound_param_name, VbaValue::Str(cell_name.to_string()));
        }
    }

    execute_vba_macro_with_selection(doc, &sub_body.join("\n"), &[], Some(initial_vars), limits)
}

pub fn execute_vba_macro_with_selection(
    doc: &mut VisioDocument,
    script: &str,
    selected_shape_ids: &[u64],
    seed_vars: Option<BTreeMap<String, VbaValue>>,
    limits: SandboxLimits,
) -> Result<AutomationExecutionReport, AutomationError> {
    let mut forms: BTreeMap<String, VbaUserFormDef> = BTreeMap::new();
    let mut raw_lines: Vec<(usize, String)> = Vec::new();

    // Pre-pass: extract declarative `Begin UserForm <Name> ... End` blocks cleanly
    let mut in_form: Option<VbaUserFormDef> = None;
    for (idx, raw) in script.lines().enumerate() {
        let code = strip_vba_comment(raw).trim().to_string();
        if code.is_empty() {
            continue;
        }
        if let Some(ref mut f) = in_form {
            if code.eq_ignore_ascii_case("End") || code.eq_ignore_ascii_case("End UserForm") {
                let done = f.clone();
                forms.insert(done.name.to_ascii_lowercase(), done);
                in_form = None;
            } else if let Some(ctrl_rest) = strip_prefix_ci(&code, "Control ") {
                // `Control TextBox txtHost = "SRV-01"`
                let (decl, init_val) = if let Some((d, v)) = ctrl_rest.split_once('=') {
                    (d.trim(), v.trim().trim_matches('"').to_string())
                } else {
                    (ctrl_rest.trim(), String::new())
                };
                let parts: Vec<&str> = decl.split_whitespace().collect();
                if parts.len() >= 2 {
                    f.controls.push(VbaUserFormControl {
                        kind: parts[0].to_string(),
                        name: parts[1].to_string(),
                        value: init_val,
                    });
                }
            } else if let Some((lhs, rhs)) = code.split_once('=') {
                if lhs.trim().eq_ignore_ascii_case("Caption") {
                    f.caption = rhs.trim().trim_matches('"').to_string();
                }
            }
            continue;
        }
        if let Some(form_name) = strip_prefix_ci(&code, "Begin UserForm ") {
            let name = form_name.trim().to_string();
            in_form = Some(VbaUserFormDef {
                caption: name.clone(),
                name,
                shown: false,
                controls: Vec::new(),
            });
            continue;
        }
        raw_lines.push((idx + 1, code));
    }

    let mut active_page_id = doc.pages.first().map(|p| p.id).unwrap_or(1);
    let mut vars: BTreeMap<String, VbaValue> = seed_vars.unwrap_or_default();
    let mut report = AutomationExecutionReport::default();

    #[derive(Clone)]
    enum LoopFrame {
        Numeric {
            var_name: String,
            end_val: f64,
            step_val: f64,
            body_pc: usize,
        },
        ForEach {
            var_name: String,
            items: Vec<VbaValue>,
            index: usize,
            body_pc: usize,
        },
    }

    let mut loop_stack: Vec<LoopFrame> = Vec::new();
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
            || upper == "END IF"
        {
            pc += 1;
            continue;
        }

        // Skip automatically over event handler subs during normal macro execution unless seeded
        if (upper.starts_with("SUB DOCUMENT_") || upper.starts_with("PRIVATE SUB DOCUMENT_"))
            && pc > 0
        {
            pc += 1;
            while pc < raw_lines.len() && !raw_lines[pc].1.eq_ignore_ascii_case("End Sub") {
                pc += 1;
            }
            pc += 1;
            continue;
        }

        if let Some(rest) = strip_prefix_ci(stmt, "Debug.Print") {
            let v = eval_vba_expr(rest.trim(), &vars, doc, &forms);
            report.log_output.push(v.as_string());
            pc += 1;
            continue;
        }

        // `UserForm1.Show`
        if upper.ends_with(".SHOW") {
            let fname = stmt[..stmt.len() - 5].trim().to_ascii_lowercase();
            let entry = forms.entry(fname.clone()).or_insert_with(|| VbaUserFormDef {
                name: fname,
                caption: "UserForm".into(),
                shown: true,
                controls: Vec::new(),
            });
            entry.shown = true;
            pc += 1;
            continue;
        }

        // `If <cond> Then [single-line or block]`
        if let Some(after_if) = strip_prefix_ci(stmt, "If ") {
            if let Some((cond_s, after_then)) = split_once_ci(after_if, " Then") {
                let cond_true = eval_vba_condition(cond_s.trim(), &vars, doc, &forms);
                let inline_tail = after_then.trim();
                if !inline_tail.is_empty() {
                    if cond_true {
                        let mut sub_lines = raw_lines.clone();
                        sub_lines[pc] = (line_no, inline_tail.to_string());
                        // Execute inline statement on next iteration by replacing in-place copy:
                        execute_single_inline_stmt(
                            inline_tail,
                            line_no,
                            doc,
                            &mut active_page_id,
                            &mut vars,
                            &mut forms,
                            &mut report,
                            limits,
                        )?;
                    }
                    pc += 1;
                    continue;
                }
                if !cond_true {
                    // Jump to matching `Else` or `End If`
                    let mut depth = 1usize;
                    pc += 1;
                    while pc < raw_lines.len() && depth > 0 {
                        let u = raw_lines[pc].1.to_ascii_uppercase();
                        if u.starts_with("IF ") && u.ends_with(" THEN") {
                            depth += 1;
                        } else if u == "END IF" {
                            depth -= 1;
                            if depth == 0 {
                                pc += 1;
                                break;
                            }
                        } else if u == "ELSE" && depth == 1 {
                            pc += 1;
                            break;
                        }
                        pc += 1;
                    }
                    continue;
                }
                pc += 1;
                continue;
            }
        }

        if upper == "ELSE" {
            // Truthy branch finished executing; skip directly to matching `End If`
            let mut depth = 1usize;
            pc += 1;
            while pc < raw_lines.len() && depth > 0 {
                let u = raw_lines[pc].1.to_ascii_uppercase();
                if u.starts_with("IF ") && u.ends_with(" THEN") {
                    depth += 1;
                } else if u == "END IF" {
                    depth -= 1;
                }
                pc += 1;
            }
            continue;
        }

        // `For Each shp In ActivePage.Shapes` | `For Each shp In ActiveWindow.Selection`
        if let Some(rest) = strip_prefix_ci(stmt, "For Each ") {
            let Some((vpart, col_part)) = split_once_ci(rest, " In ") else {
                return Err(AutomationError::Syntax {
                    line: line_no,
                    message: "Expected 'In' in For Each loop".into(),
                });
            };
            let vname = vpart.trim().to_ascii_lowercase();
            let col_norm = col_part.trim().to_ascii_lowercase();
            let items: Vec<VbaValue> = if col_norm == "activewindow.selection" || col_norm == "selection" {
                let ids: Vec<u64> = if selected_shape_ids.is_empty() {
                    doc.page(active_page_id)
                        .map(|p| p.shapes.iter().map(|s| s.id).collect())
                        .unwrap_or_default()
                } else {
                    selected_shape_ids.to_vec()
                };
                ids.into_iter()
                    .map(|shape_id| VbaValue::ShapeRef {
                        page_id: active_page_id,
                        shape_id,
                    })
                    .collect()
            } else if col_norm == "activepage.shapes" {
                doc.page(active_page_id)
                    .map(|p| {
                        p.shapes
                            .iter()
                            .map(|s| VbaValue::ShapeRef {
                                page_id: active_page_id,
                                shape_id: s.id,
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else if col_norm == "activedocument.pages" {
                doc.pages.iter().map(|p| VbaValue::PageRef(p.id)).collect()
            } else {
                Vec::new()
            };

            if items.is_empty() {
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

            vars.insert(vname.clone(), items[0].clone());
            loop_stack.push(LoopFrame::ForEach {
                var_name: vname,
                items,
                index: 0,
                body_pc: pc + 1,
            });
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
                (e, eval_vba_expr(st.trim(), &vars, doc, &forms).as_f64())
            } else {
                (after_to, 1.0)
            };
            let init_v = eval_vba_expr(start_s.trim(), &vars, doc, &forms).as_f64();
            let end_v = eval_vba_expr(end_s.trim(), &vars, doc, &forms).as_f64();
            vars.insert(vname.clone(), VbaValue::Num(init_v));
            if (step_val >= 0.0 && init_v > end_v) || (step_val < 0.0 && init_v < end_v) {
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
            loop_stack.push(LoopFrame::Numeric {
                var_name: vname,
                end_val: end_v,
                step_val,
                body_pc: pc + 1,
            });
            pc += 1;
            continue;
        }

        if upper == "NEXT" || upper.starts_with("NEXT ") {
            let Some(frame) = loop_stack.pop() else {
                return Err(AutomationError::Syntax {
                    line: line_no,
                    message: "Unexpected Next without For".into(),
                });
            };
            match frame {
                LoopFrame::Numeric {
                    var_name,
                    end_val,
                    step_val,
                    body_pc,
                } => {
                    let cur = vars.get(&var_name).map(|v| v.as_f64()).unwrap_or(0.0) + step_val;
                    vars.insert(var_name.clone(), VbaValue::Num(cur));
                    let keep_going = if step_val >= 0.0 {
                        cur <= end_val + 1e-9
                    } else {
                        cur >= end_val - 1e-9
                    };
                    if keep_going {
                        loop_stack.push(LoopFrame::Numeric {
                            var_name,
                            end_val,
                            step_val,
                            body_pc,
                        });
                        pc = body_pc;
                    } else {
                        pc += 1;
                    }
                }
                LoopFrame::ForEach {
                    var_name,
                    items,
                    index,
                    body_pc,
                } => {
                    let next_idx = index + 1;
                    if next_idx < items.len() {
                        vars.insert(var_name.clone(), items[next_idx].clone());
                        loop_stack.push(LoopFrame::ForEach {
                            var_name,
                            items,
                            index: next_idx,
                            body_pc,
                        });
                        pc = body_pc;
                    } else {
                        pc += 1;
                    }
                }
            }
            continue;
        }

        execute_single_inline_stmt(
            stmt,
            line_no,
            doc,
            &mut active_page_id,
            &mut vars,
            &mut forms,
            &mut report,
            limits,
        )?;
        pc += 1;
    }

    report.user_forms = forms.into_values().collect();
    route_page_connectors(doc, active_page_id);
    ReactiveEngine::recalculate_document(doc);
    Ok(report)
}

#[allow(clippy::too_many_arguments)]
fn execute_single_inline_stmt(
    stmt: &str,
    line_no: usize,
    doc: &mut VisioDocument,
    active_page_id: &mut u32,
    vars: &mut BTreeMap<String, VbaValue>,
    forms: &mut BTreeMap<String, VbaUserFormDef>,
    report: &mut AutomationExecutionReport,
    limits: SandboxLimits,
) -> Result<(), AutomationError> {
    let upper = stmt.to_ascii_uppercase();

    if let Some(rest) = strip_prefix_ci(stmt, "Debug.Print") {
        let v = eval_vba_expr(rest.trim(), vars, doc, forms);
        report.log_output.push(v.as_string());
        return Ok(());
    }

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
            *active_page_id = next_pid;
            vars.insert(var_name, VbaValue::PageRef(next_pid));
        } else if let Some(args_s) = extract_method_call_ci(rhs, "DrawRectangle") {
            if report.shapes_created >= limits.max_created_shapes {
                return Err(AutomationError::Runtime("Max created shapes quota exceeded".into()));
            }
            let nums = eval_arg_list_f64(args_s, vars, doc, forms);
            if nums.len() >= 4 {
                let (x1, y1, x2, y2) = (nums[0], nums[1], nums[2], nums[3]);
                let w = (x2 - x1).abs().max(0.1);
                let h = (y2 - y1).abs().max(0.1);
                let cx = (x1 + x2) * 0.5;
                let cy = (y1 + y2) * 0.5;
                let page = doc
                    .page_mut(*active_page_id)
                    .ok_or_else(|| AutomationError::Runtime("No active page".into()))?;
                let sid = page.next_shape_id();
                page.shapes
                    .push(VisioShape::new_rect(sid, format!("Sheet.{sid}"), cx, cy, w, h));
                report.shapes_created += 1;
                vars.insert(
                    var_name,
                    VbaValue::ShapeRef {
                        page_id: *active_page_id,
                        shape_id: sid,
                    },
                );
            }
        } else if let Some(args_s) = extract_method_call_ci(rhs, "Drop") {
            let parts: Vec<&str> = args_s.split(',').collect();
            if parts.len() >= 3 {
                let m_val = eval_vba_expr(parts[0].trim(), vars, doc, forms);
                let px = eval_vba_expr(parts[1].trim(), vars, doc, forms).as_f64();
                let py = eval_vba_expr(parts[2].trim(), vars, doc, forms).as_f64();
                let mid = m_val.as_f64() as u32;
                let sid = if let Some(s) = doc.drop_master(*active_page_id, mid, px, py) {
                    s
                } else {
                    let page = doc.page_mut(*active_page_id).unwrap();
                    let s = page.next_shape_id();
                    page.shapes
                        .push(VisioShape::new_rect(s, format!("Dropped.{s}"), px, py, 1.5, 1.0));
                    s
                };
                report.shapes_created += 1;
                vars.insert(
                    var_name,
                    VbaValue::ShapeRef {
                        page_id: *active_page_id,
                        shape_id: sid,
                    },
                );
            }
        }
        return Ok(());
    }

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
        return Ok(());
    }

    if let Some((lhs_raw, rhs_raw)) = stmt.split_once('=') {
        let lhs = lhs_raw.trim();
        let rhs_val = eval_vba_expr(rhs_raw.trim(), vars, doc, forms);

        if lhs.eq_ignore_ascii_case("ActiveWindow.Zoom") {
            let z = rhs_val.as_f64().clamp(0.1, 10.0);
            report.window_zoom = Some(z);
            if let Some(p) = doc.page_mut(*active_page_id) {
                p.view_scale = z;
            }
            return Ok(());
        }

        // `UserForm1.txtHost.Text = "SRV-99"` or `UserForm1.Caption = "..."`
        let dot_parts: Vec<&str> = lhs.split('.').map(str::trim).collect();
        if dot_parts.len() >= 2 {
            let root_key = dot_parts[0].to_ascii_lowercase();
            if let Some(form) = forms.get_mut(&root_key) {
                if dot_parts.len() == 2 && dot_parts[1].eq_ignore_ascii_case("Caption") {
                    form.caption = rhs_val.as_string();
                    return Ok(());
                } else if dot_parts.len() >= 3 {
                    let ctrl_name = dot_parts[1];
                    if let Some(ctrl) = form
                        .controls
                        .iter_mut()
                        .find(|c| c.name.eq_ignore_ascii_case(ctrl_name))
                    {
                        ctrl.value = rhs_val.as_string();
                    } else {
                        form.controls.push(VbaUserFormControl {
                            kind: "TextBox".into(),
                            name: ctrl_name.to_string(),
                            value: rhs_val.as_string(),
                        });
                    }
                    return Ok(());
                }
            }
        }

        if let Some((obj_var, after_cells)) = split_once_ci(lhs, ".Cells(") {
            if let Some(end_q) = after_cells.find(')') {
                let cell_expr = &after_cells[..end_q];
                let cell_name = eval_vba_expr(cell_expr.trim(), vars, doc, forms).as_string();
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
                _ => {}
            }
        } else {
            vars.insert(lhs.to_ascii_lowercase(), rhs_val);
        }
        return Ok(());
    }

    Err(AutomationError::Syntax {
        line: line_no,
        message: format!("Unsupported VBA statement: {stmt}"),
    })
}

fn eval_vba_condition(
    cond: &str,
    vars: &BTreeMap<String, VbaValue>,
    doc: &VisioDocument,
    forms: &BTreeMap<String, VbaUserFormDef>,
) -> bool {
    for op in ["<>", "<=", ">=", "=", "<", ">"] {
        if let Some((l, r)) = split_once_ci(cond, op) {
            let lv = eval_vba_expr(l.trim(), vars, doc, forms);
            let rv = eval_vba_expr(r.trim(), vars, doc, forms);
            let both_numeric = matches!(lv, VbaValue::Num(_)) || matches!(rv, VbaValue::Num(_));
            return match op {
                "<>" => {
                    if both_numeric {
                        (lv.as_f64() - rv.as_f64()).abs() > 1e-9
                    } else {
                        lv.as_string() != rv.as_string()
                    }
                }
                "<=" => lv.as_f64() <= rv.as_f64() + 1e-9,
                ">=" => lv.as_f64() + 1e-9 >= rv.as_f64(),
                "=" => {
                    if both_numeric {
                        (lv.as_f64() - rv.as_f64()).abs() <= 1e-9
                    } else {
                        lv.as_string() == rv.as_string()
                    }
                }
                "<" => lv.as_f64() < rv.as_f64() - 1e-9,
                ">" => lv.as_f64() > rv.as_f64() + 1e-9,
                _ => false,
            };
        }
    }
    eval_vba_expr(cond, vars, doc, forms).as_bool()
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

fn eval_arg_list_f64(
    args_s: &str,
    vars: &BTreeMap<String, VbaValue>,
    doc: &VisioDocument,
    forms: &BTreeMap<String, VbaUserFormDef>,
) -> Vec<f64> {
    args_s
        .split(',')
        .map(|a| eval_vba_expr(a.trim(), vars, doc, forms).as_f64())
        .collect()
}

fn eval_vba_expr(
    expr: &str,
    vars: &BTreeMap<String, VbaValue>,
    doc: &VisioDocument,
    forms: &BTreeMap<String, VbaUserFormDef>,
) -> VbaValue {
    let s = expr.trim();
    if s.eq_ignore_ascii_case("True") {
        return VbaValue::Bool(true);
    }
    if s.eq_ignore_ascii_case("False") {
        return VbaValue::Bool(false);
    }
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        return VbaValue::Str(s[1..s.len() - 1].replace("\"\"", "\""));
    }
    if let Some((a, b)) = split_top_level_op(s, '&') {
        return VbaValue::Str(format!(
            "{}{}",
            eval_vba_expr(a, vars, doc, forms).as_string(),
            eval_vba_expr(b, vars, doc, forms).as_string()
        ));
    }
    if let Some((a, b)) = split_top_level_op(s, '+') {
        return VbaValue::Num(
            eval_vba_expr(a, vars, doc, forms).as_f64() + eval_vba_expr(b, vars, doc, forms).as_f64(),
        );
    }
    if let Some((a, b)) = split_top_level_op(s, '-') {
        if !a.trim().is_empty() {
            return VbaValue::Num(
                eval_vba_expr(a, vars, doc, forms).as_f64() - eval_vba_expr(b, vars, doc, forms).as_f64(),
            );
        }
    }
    if let Some((a, b)) = split_top_level_op(s, '*') {
        return VbaValue::Num(
            eval_vba_expr(a, vars, doc, forms).as_f64() * eval_vba_expr(b, vars, doc, forms).as_f64(),
        );
    }
    if let Some((a, b)) = split_top_level_op(s, '/') {
        let denom = eval_vba_expr(b, vars, doc, forms).as_f64();
        return VbaValue::Num(if denom.abs() < 1e-12 {
            0.0
        } else {
            eval_vba_expr(a, vars, doc, forms).as_f64() / denom
        });
    }
    if let Ok(n) = s.parse::<f64>() {
        return VbaValue::Num(n);
    }
    if let Some(v) = vars.get(&s.to_ascii_lowercase()) {
        return v.clone();
    }
    // Check `shp.Text`, `shp.Name`, or `UserForm1.txtHost.Text`
    let dot_parts: Vec<&str> = s.split('.').map(str::trim).collect();
    if dot_parts.len() >= 2 {
        let root_l = dot_parts[0].to_ascii_lowercase();
        if let Some(VbaValue::ShapeRef { page_id, shape_id }) = vars.get(&root_l) {
            if let Some(sh) = doc.page(*page_id).and_then(|p| p.find_shape(*shape_id)) {
                match dot_parts[1].to_ascii_lowercase().as_str() {
                    "text" => return VbaValue::Str(sh.text.clone()),
                    "name" => return VbaValue::Str(sh.name.clone()),
                    "id" => return VbaValue::Num(sh.id as f64),
                    _ => {}
                }
            }
        }
        if let Some(form) = forms.get(&root_l) {
            if dot_parts.len() >= 3 {
                if let Some(ctrl) = form
                    .controls
                    .iter()
                    .find(|c| c.name.eq_ignore_ascii_case(dot_parts[1]))
                {
                    return VbaValue::Str(ctrl.value.clone());
                }
            }
        }
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
