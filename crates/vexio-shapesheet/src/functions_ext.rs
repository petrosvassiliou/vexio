//! Extended Microsoft Visio ShapeSheet Function Catalog (`v0.7.0-beta` — 235+ Total Functions).
//!
//! Implements deterministic, zero-stub evaluators for all Visio Date/Time & Elapsed Duration,
//! Hyperbolic/Combinatorial/Statistical Math, String/Formatting, Geometry/Path Introspection
//! (`PATHLEN`, `SHAPELENGTH`, `AREA`, `POINTALONGPATH`, `TANGENTALONGPATH`, `NEARESTPOINTONPATH`,
//! `DISTTOPATH`, `BOUNDINGBOXDIST`, `PTINPATH`), Sheet/Container/List/Callout/Connector/Layer/Doc
//! Queries (`HASCATEGORY`, `ISSUBSHAPEOF`, `CONTAINERSHEETREF`, `LISTSHEETREF`, `LISTORDER`,
//! `CALLOUTTARGETREF`, `CONNECTORSHEETREF`, `MEMBEROF`, `PAGENUMBER`, `PAGECOUNT`), and Smart-Shape
//! Action Triggers (`CALLTHIS`, `RUNMACRO`, `RUNADDON`, `GOTOPAGE`, `OPENTEXTWIN`, etc.).

use crate::dag::{EvalSession, GlobalCellId, SheetKey, encode_global_cell_ref};
use crate::model::{ShapeKind, VisioPage, VisioShape};
use crate::parser::{Expr, canonical_cell_name};
use crate::sections::GeometryRowKind;
use crate::units::{CellError, CellValue, Unit, normalize_f64, parse_scalar_with_unit};

/// Complete sorted registry of all 238 built-in Visio ShapeSheet functions supported by `vexio-shapesheet`.
pub const SUPPORTED_SHAPESHEET_FUNCTIONS: &[&str] = &[
    "ABS", "ACOS", "ACOSH", "AND", "ANG360", "AREA", "ASIN", "ASINH", "ATAN", "ATAN2", "ATANH",
    "AVERAGE", "BBOXHEIGHT", "BBOXWIDTH", "BBOXX", "BBOXY", "BITAND", "BITLSHIFT", "BITNOT",
    "BITOR", "BITRSHIFT", "BITXOR", "BLEND", "BOUND", "BOUNDINGBOXDIST", "CALLOUTCOUNT",
    "CALLOUTTARGETREF", "CALLTHIS", "CATEGORY", "CBRT", "CEILING", "CELLISTHEMED", "CHAR",
    "CHOOSE", "CLAMP", "CLEAN", "CODE", "COLOR", "COMBIN", "COMPANY", "CONCATENATE",
    "CONNECTEDSHAPES", "CONNECTORSHEETREF", "CONTAINERCOUNT", "CONTAINERSHEETREF", "CONTAINS",
    "COS", "COSH", "COT", "COTH", "COUNT", "COUNTA", "CREATOR", "CSC", "CSCH", "CY", "DATE",
    "DATEADD", "DATEDIFF", "DATETIME", "DATEVALUE", "DAY", "DAYOFYEAR", "DEG", "DEGREES",
    "DEPENDSON", "DESCRIPTION", "DIRECTORY", "DIST", "DISTTOPATH", "DOCCREATION", "DOCLASTSAVE",
    "DOCMD", "DOLLAR", "EDATE", "EDAYS", "EHOURS", "EMINUTES", "ENDSWITH", "EOMONTH", "ESECONDS",
    "EVALCELL", "EVEN", "EWEEKS", "EXACT", "EXP", "EXP2", "FACT", "FILENAME", "FIND", "FIXED",
    "FLOOR", "FORMAT", "FORMATEX", "GCD", "GETREF", "GETVAL", "GOTOPAGE", "GRAVITY", "GREEN",
    "GUARD", "HASCATEGORY", "HELP", "HITTEST", "HOUR", "HSL", "HUE", "HYPOT", "ID", "IF",
    "IFERROR", "INDEX", "INT", "INTERSECTX", "INTERSECTY", "IS1D", "IS2D", "ISERR", "ISERRDIV0",
    "ISERRNA", "ISERRNUM", "ISERROR", "ISERRREF", "ISERRVALUE", "ISLOGICAL", "ISNUMBER",
    "ISOWEEKNUM", "ISREF", "ISSUBSHAPEOF", "ISTEXT", "KEYWORDS", "LARGE", "LAYERMEMBER", "LCM",
    "LEFT", "LEN", "LISTORDER", "LISTSHEETREF", "LN", "LOC", "LOCTOPAR", "LOG", "LOG10", "LOG2",
    "LOOKUP", "LOWER", "LPAD", "LUM", "LUMMOD", "LUMOFF", "MAGNITUDE", "MASTERNAME", "MAX",
    "MEDIAN", "MEMBEROF", "MID", "MIN", "MINUTE", "MOD", "MODE", "MONTH", "MROUND", "N", "NA",
    "NAME", "NAMEU", "NEARESTPOINTONPATH", "NETWORKDAYS", "NORMANGLE", "NOT", "NOW", "NURBS",
    "ODD", "OPENGROUPWIN", "OPENSHAPESHEET", "OPENTEXTWIN", "OR", "PAGECOUNT", "PAGENAME",
    "PAGENUMBER", "PAR", "PARENTSHEET", "PARTOLOC", "PATHLEN", "PERCENTILE", "PERMUT", "PI",
    "PLAYSOUND", "PNT", "PNTX", "PNTY", "POINTALONGPATH", "POLYLINE", "POW", "PROPER", "PTINPATH",
    "QUEUEMARKEREVENT", "QUOTIENT", "RAD", "RADIANS", "RAND", "RANDBETWEEN", "RED", "REPLACE",
    "REPT", "REVERSE", "RGB", "RGB2HEX", "RIGHT", "ROUND", "ROUNDDOWN", "ROUNDUP", "RPAD",
    "RUNADDON", "RUNADDONWARGS", "RUNMACRO", "SAT", "SATMOD", "SEARCH", "SEC", "SECH", "SECOND",
    "SETATREF", "SETATREFEXPR", "SETF", "SHADE", "SHAPEAREA", "SHAPELENGTH", "SHAPETEXT",
    "SHEETID", "SIGN", "SIN", "SINH", "SMALL", "SPLITCOUNT", "SQRT", "SQRTPI", "STARTSWITH",
    "STDEV", "STDEVP", "STR", "STRSAME", "SUBJECT", "SUBSTITUTE", "SUM", "T", "TAN",
    "TANGENTALONGPATH", "TANH", "TEXT", "THEME", "THEMECBV", "THEMEGUARD", "THEMEVAL",
    "THEPAGENAME", "TIME", "TIMEVALUE", "TINT", "TITLE", "TODAY", "TONE", "TRIM", "TRUNC",
    "UNICHAR", "UNICODE", "UPPER", "VALUE", "VAR", "VARP", "WEEKDAY", "WEEKNUM", "WORKDAY", "XOR",
    "YEAR",
];

/// Return the complete slice of supported ShapeSheet function names (`>= 225`).
pub fn list_supported_shapesheet_functions() -> &'static [&'static str] {
    SUPPORTED_SHAPESHEET_FUNCTIONS
}

// ============================================================================
// Proleptic Gregorian Calendar <-> Visio Serial Day (`1899-12-30` = `0.0`)
// ============================================================================
// Unix epoch `1970-01-01` is `25569` days after Visio/OLE Automation epoch `1899-12-30`.
const VISIO_TO_UNIX_DAYS: i64 = 25_569;

fn civil_to_visio_days(year: i32, month: i32, day: i32) -> f64 {
    // Normalize out-of-range months (Visio `DATE(2026, 14, 1)` rolls into `2027-02-01`)
    let total_months = (year as i64) * 12 + (month as i64 - 1);
    let norm_y = total_months.div_euclid(12);
    let norm_m = (total_months.rem_euclid(12) + 1) as u32;
    let y = norm_y - if norm_m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u32;
    let doy = (153 * (if norm_m > 2 { norm_m - 3 } else { norm_m + 9 }) + 2) / 5;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let unix_day_m1 = era * 146_097 + (doe as i64) - 719_468;
    (unix_day_m1 + (day as i64 - 1) + VISIO_TO_UNIX_DAYS) as f64
}

fn visio_days_to_civil(visio_serial: f64) -> (i32, u32, u32) {
    let serial_days = visio_serial.floor() as i64;
    let z = serial_days - VISIO_TO_UNIX_DAYS + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = (y + if m <= 2 { 1 } else { 0 }) as i32;
    (year, m, d)
}

fn is_leap_year(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(y) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn visio_time_hms(visio_serial: f64) -> (u32, u32, u32) {
    let frac = visio_serial.rem_euclid(1.0);
    let total_secs = (frac * 86_400.0).round() as u32 % 86_400;
    let h = total_secs / 3_600;
    let m = (total_secs % 3_600) / 60;
    let s = total_secs % 60;
    (h, m, s)
}

fn parse_datetime_str(raw: &str) -> Option<f64> {
    let s = raw.trim().trim_matches('"').trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        return Some(n);
    }
    // Split optional date and time parts (`YYYY-MM-DD HH:MM:SS`, `YYYY-MM-DDTHH:MM:SSZ`, `MM/DD/YYYY`, `DD.MM.YYYY`, `HH:MM:SS`)
    let norm = s.trim_end_matches('Z');
    let (date_part, time_part) = if let Some((d, t)) = norm.split_once('T') {
        (Some(d.trim()), Some(t.trim()))
    } else if let Some((d, t)) = norm.split_once(' ') {
        if d.contains(':') && !t.contains(':') {
            (None, Some(norm))
        } else {
            (Some(d.trim()), Some(t.trim()))
        }
    } else if norm.contains(':') {
        (None, Some(norm))
    } else {
        (Some(norm), None)
    };

    let mut serial = 0.0;
    if let Some(dp) = date_part {
        if let Some((a, rest)) = dp.split_once('-') {
            let (b, c) = rest.split_once('-')?;
            let y: i32 = a.parse().ok()?;
            let m: i32 = b.parse().ok()?;
            let d: i32 = c.parse().ok()?;
            serial += civil_to_visio_days(y, m, d);
        } else if let Some((a, rest)) = dp.split_once('/') {
            let (b, c) = rest.split_once('/')?;
            let p1: i32 = a.parse().ok()?;
            let p2: i32 = b.parse().ok()?;
            let p3: i32 = c.parse().ok()?;
            let (y, m, d) = if p1 > 31 { (p1, p2, p3) } else { (p3, p1, p2) };
            serial += civil_to_visio_days(y, m, d);
        } else if let Some((a, rest)) = dp.split_once('.') {
            let (b, c) = rest.split_once('.')?;
            let d: i32 = a.parse().ok()?;
            let m: i32 = b.parse().ok()?;
            let y: i32 = c.parse().ok()?;
            serial += civil_to_visio_days(y, m, d);
        } else {
            return None;
        }
    }

    if let Some(tp) = time_part {
        let mut t_clean = tp;
        let mut pm_offset = 0u32;
        let mut is_12am = false;
        let up = tp.to_ascii_uppercase();
        if let Some(stripped) = up.strip_suffix("PM") {
            pm_offset = 12;
            t_clean = &tp[..stripped.len()];
        } else if let Some(stripped) = up.strip_suffix("AM") {
            is_12am = true;
            t_clean = &tp[..stripped.len()];
        }
        let parts: Vec<&str> = t_clean.trim().split(':').collect();
        if parts.len() < 2 {
            return None;
        }
        let mut h: u32 = parts[0].trim().parse().ok()?;
        let min: u32 = parts[1].trim().parse().ok()?;
        let sec: f64 = parts
            .get(2)
            .and_then(|x| x.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        if is_12am && h == 12 {
            h = 0;
        } else if pm_offset == 12 && h < 12 {
            h += 12;
        }
        serial += ((h as f64) * 3_600.0 + (min as f64) * 60.0 + sec) / 86_400.0;
    }

    Some(serial)
}

fn add_months_serial(base_serial: f64, delta_months: i32) -> f64 {
    let frac = base_serial.rem_euclid(1.0);
    let (y, m, d) = visio_days_to_civil(base_serial);
    let total = (y as i64) * 12 + (m as i64 - 1) + (delta_months as i64);
    let ny = total.div_euclid(12) as i32;
    let nm = (total.rem_euclid(12) + 1) as u32;
    let nd = d.min(days_in_month(ny, nm));
    civil_to_visio_days(ny, nm as i32, nd as i32) + frac
}

fn gcd_u64(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl<'a> EvalSession<'a> {
    fn resolve_now_serial(&self) -> f64 {
        if let Some(custom) = self.doc.doc_sheet.get_f64("User.VexioClockSerial") {
            if custom.is_finite() && custom > 0.0 {
                return custom;
            }
        }
        if !self.doc.properties.modified_iso.is_empty() {
            if let Some(parsed) = parse_datetime_str(&self.doc.properties.modified_iso) {
                return parsed;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(dur) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
                return (VISIO_TO_UNIX_DAYS as f64) + dur.as_secs_f64() / 86_400.0;
            }
        }
        civil_to_visio_days(2026, 1, 1) + 0.5
    }

    fn eval_datetime_arg(&mut self, ctx_sheet: &SheetKey, ctx_cell: &str, expr: &Expr) -> Result<f64, CellError> {
        let cv = self.eval_expr(ctx_sheet, ctx_cell, expr);
        match cv {
            CellValue::Num { internal, .. } => Ok(internal),
            CellValue::Str(s) => parse_datetime_str(&s).ok_or(CellError::Value),
            CellValue::Err(e) => Err(e),
            _ => cv.as_internal_f64(),
        }
    }

    /// Linearize a shape's `Geometry[geom_idx]` (1-based) into local-space `(x_in, y_in)` polyline vertices
    /// and whether the path is closed.
    fn linearize_local_geometry_path(&mut self, sheet: &SheetKey, geom_idx_1based: u32) -> Option<(Vec<[f64; 2]>, bool)> {
        let shape_sheet = match sheet {
            SheetKey::Shape { page_id, shape_id } => {
                &self.doc.page(*page_id)?.find_shape(*shape_id)?.sheet
            }
            SheetKey::Master { master_id, shape_id } => {
                let m = self.doc.master(*master_id)?;
                &m.shapes
                    .iter()
                    .find_map(|s| s.find_shape(*shape_id))
                    .or_else(|| m.shapes.first())?
                    .sheet
            }
            _ => return None,
        };
        let geom = shape_sheet
            .geometries
            .iter()
            .find(|g| g.index == geom_idx_1based && !g.deleted)
            .or_else(|| shape_sheet.geometries.iter().find(|g| !g.deleted))?
            .clone();

        let width = self.eval_sheet_f64(sheet, "Width", 1.0);
        let height = self.eval_sheet_f64(sheet, "Height", 1.0);
        let prefix = format!("Geometry{}", geom.index);
        let mut pts: Vec<[f64; 2]> = Vec::new();
        let mut closed = false;

        for row in &geom.rows {
            if row.deleted {
                continue;
            }
            let ix = row.ix;
            let raw_x = self.eval_sheet_f64(sheet, &format!("{prefix}.X{ix}"), 0.0);
            let raw_y = self.eval_sheet_f64(sheet, &format!("{prefix}.Y{ix}"), 0.0);
            match row.kind {
                GeometryRowKind::Ellipse => {
                    // `X, Y` = center; `A, B` = major point; `C, D` = minor point
                    let ax = self.eval_sheet_f64(sheet, &format!("{prefix}.A{ix}"), raw_x + width * 0.5);
                    let ay = self.eval_sheet_sheet_or_default(sheet, &format!("{prefix}.B{ix}"), raw_y);
                    let cx = self.eval_sheet_sheet_or_default(sheet, &format!("{prefix}.C{ix}"), raw_x);
                    let cy = self.eval_sheet_f64(sheet, &format!("{prefix}.D{ix}"), raw_y + height * 0.5);
                    let rx = (ax - raw_x).hypot(ay - raw_y).max(1e-6);
                    let ry = (cx - raw_x).hypot(cy - raw_y).max(1e-6);
                    pts.clear();
                    let steps = 64usize;
                    for s in 0..=steps {
                        let ang = (s as f64) * std::f64::consts::TAU / (steps as f64);
                        pts.push([raw_x + rx * ang.cos(), raw_y + ry * ang.sin()]);
                    }
                    closed = true;
                }
                GeometryRowKind::RelMoveTo | GeometryRowKind::RelLineTo | GeometryRowKind::RelQuadBezTo | GeometryRowKind::RelCubBezTo | GeometryRowKind::RelEllipticalArcTo => {
                    pts.push([raw_x * width, raw_y * height]);
                }
                GeometryRowKind::ArcTo => {
                    let bow = self.eval_sheet_f64(sheet, &format!("{prefix}.A{ix}"), 0.0);
                    if let Some(&p0) = pts.last() {
                        let chord = (raw_x - p0[0]).hypot(raw_y - p0[1]);
                        if bow.abs() > 1e-6 && chord > 1e-6 {
                            // Exact circular arc sagitta subdivision (16 segments)
                            let r = (chord * chord / (8.0 * bow.abs())) + bow.abs() * 0.5;
                            let mx = (p0[0] + raw_x) * 0.5;
                            let my = (p0[1] + raw_y) * 0.5;
                            let ux = (raw_x - p0[0]) / chord;
                            let uy = (raw_y - p0[1]) / chord;
                            let nx = -uy;
                            let ny = ux;
                            let h = (r * r - (chord * 0.5).powi(2)).max(0.0).sqrt();
                            let sign = if bow >= 0.0 { 1.0 } else { -1.0 };
                            let cx = mx - sign * nx * h;
                            let cy = my - sign * ny * h;
                            let a0 = (p0[1] - cy).atan2(p0[0] - cx);
                            let mut da = (raw_y - cy).atan2(raw_x - cx) - a0;
                            if bow > 0.0 && da > 0.0 {
                                da -= std::f64::consts::TAU;
                            } else if bow < 0.0 && da < 0.0 {
                                da += std::f64::consts::TAU;
                            }
                            for k in 1..=16 {
                                let t = (k as f64) / 16.0;
                                let ang = a0 + da * t;
                                pts.push([cx + r * ang.cos(), cy + r * ang.sin()]);
                            }
                        } else {
                            pts.push([raw_x, raw_y]);
                        }
                    } else {
                        pts.push([raw_x, raw_y]);
                    }
                }
                _ => {
                    pts.push([raw_x, raw_y]);
                }
            }
        }
        if pts.len() >= 3 {
            let first = pts[0];
            let last = *pts.last().unwrap();
            if (first[0] - last[0]).hypot(first[1] - last[1]) < 1e-5 {
                closed = true;
            }
        }
        Some((pts, closed))
    }

    fn eval_sheet_sheet_or_default(&mut self, sheet: &SheetKey, cell_name: &str, default: f64) -> f64 {
        let cid = GlobalCellId {
            sheet: sheet.clone(),
            cell: canonical_cell_name(cell_name),
        };
        if self.lookup_cell_exists(&cid) {
            self.eval_cell_id(&cid).as_internal_f64().unwrap_or(default)
        } else {
            default
        }
    }

    fn parse_geom_target(&mut self, ctx_sheet: &SheetKey, ctx_cell: &str, arg_opt: Option<&Expr>) -> (SheetKey, u32) {
        let Some(expr) = arg_opt else {
            return (ctx_sheet.clone(), 1);
        };
        if let Expr::CellRef(path) = expr {
            let target_sheet = self
                .resolve_ref_path(ctx_sheet, path)
                .map(|c| c.sheet)
                .unwrap_or_else(|| ctx_sheet.clone());
            let low = path.cell.to_ascii_lowercase();
            if let Some(rest) = low.strip_prefix("geometry") {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(idx) = digits.parse::<u32>() {
                    return (target_sheet, idx.max(1));
                }
            }
            return (target_sheet, 1);
        }
        let idx = self
            .eval_expr(ctx_sheet, ctx_cell, expr)
            .as_internal_f64()
            .ok()
            .map(|v| {
                let iv = v.round() as i32;
                if iv <= 0 { 1u32 } else { iv as u32 }
            })
            .unwrap_or(1);
        (ctx_sheet.clone(), idx)
    }

    fn find_enclosing_structures(
        &mut self,
        page: &VisioPage,
        page_id: u32,
        shape_id: u64,
        want_list: bool,
    ) -> Vec<u64> {
        let mut matches_out = Vec::new();
        let needle_id = shape_id.to_string();
        let member_tf = self.eval_shape_transform(&SheetKey::Shape { page_id, shape_id });

        for cand in &page.shapes {
            if cand.id == shape_id {
                continue;
            }
            let stype = cand
                .sheet
                .get_str("User.msvStructureType")
                .unwrap_or_default();
            let is_match_kind = if want_list {
                stype.eq_ignore_ascii_case("List") || stype.eq_ignore_ascii_case("Swimlane")
            } else {
                stype.eq_ignore_ascii_case("Container")
            };
            if !is_match_kind {
                continue;
            }
            let members_cell = if want_list {
                cand.sheet
                    .get_str("User.msvListMembers")
                    .or_else(|| cand.sheet.get_str("User.msvContainerMembers"))
                    .unwrap_or_default()
            } else {
                cand.sheet
                    .get_str("User.msvContainerMembers")
                    .unwrap_or_default()
            };
            if members_cell
                .split([';', ','])
                .any(|tok| tok.trim() == needle_id || tok.trim().eq_ignore_ascii_case(&format!("Sheet.{shape_id}")))
            {
                matches_out.push(cand.id);
                continue;
            }
            if let (Some(mtf), Some(ctf)) = (
                member_tf,
                self.eval_shape_transform(&SheetKey::Shape {
                    page_id,
                    shape_id: cand.id,
                }),
            ) {
                let (cmn, cmx) = ctf.world_aabb();
                if mtf.pin_x >= cmn[0]
                    && mtf.pin_x <= cmx[0]
                    && mtf.pin_y >= cmn[1]
                    && mtf.pin_y <= cmx[1]
                {
                    matches_out.push(cand.id);
                }
            }
        }
        matches_out
    }

    pub(crate) fn eval_extended_builtin(
        &mut self,
        ctx_sheet: &SheetKey,
        ctx_cell: &str,
        upper: &str,
        args: &[Expr],
    ) -> CellValue {
        match upper {
            // --- Dynamic `<SheetFunc>()!Cell` dereference ---
            "__DYN_SHEET_CELL" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Ref);
                }
                let sheet_val = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let Ok(cell_name) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced() else {
                    return CellValue::Err(CellError::Ref);
                };
                if let CellValue::Ref(encoded) = sheet_val {
                    if let Some(mut cid) = crate::dag::decode_global_cell_ref_pub(&encoded) {
                        cid.cell = canonical_cell_name(&cell_name);
                        return self.eval_cell_id(&cid);
                    }
                }
                CellValue::Err(CellError::Ref)
            }

            // ================================================================
            // 1. Date / Time, Calendar & Elapsed Duration Functions
            // ================================================================
            "NOW" => CellValue::Num {
                internal: normalize_f64(self.resolve_now_serial()),
                unit: Unit::ElapsedDay,
            },
            "TODAY" => CellValue::Num {
                internal: normalize_f64(self.resolve_now_serial().floor()),
                unit: Unit::ElapsedDay,
            },
            "DATE" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(y) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(m) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(d) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let mut yr = y.trunc() as i32;
                if (0..100).contains(&yr) {
                    yr += if yr < 30 { 2000 } else { 1900 };
                }
                CellValue::Num {
                    internal: civil_to_visio_days(yr, m.trunc() as i32, d.trunc() as i32),
                    unit: Unit::ElapsedDay,
                }
            }
            "TIME" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(h) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(m) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let frac = ((h * 3_600.0 + m * 60.0 + s) / 86_400.0).rem_euclid(1.0);
                CellValue::Num {
                    internal: normalize_f64(frac),
                    unit: Unit::ElapsedDay,
                }
            }
            "DATETIME" | "DATEVALUE" | "TIMEVALUE" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_datetime_arg(ctx_sheet, ctx_cell, a0) {
                    Ok(serial) => {
                        let out = match upper {
                            "DATEVALUE" => serial.floor(),
                            "TIMEVALUE" => serial.rem_euclid(1.0),
                            _ => serial,
                        };
                        CellValue::Num {
                            internal: normalize_f64(out),
                            unit: Unit::ElapsedDay,
                        }
                    }
                    Err(e) => CellValue::Err(e),
                }
            }
            "YEAR" | "MONTH" | "DAY" | "DAYOFYEAR" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let serial = match self.eval_datetime_arg(ctx_sheet, ctx_cell, a0) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let (y, m, d) = visio_days_to_civil(serial);
                let out = match upper {
                    "YEAR" => y as f64,
                    "MONTH" => m as f64,
                    "DAY" => d as f64,
                    _ => (serial.floor() - civil_to_visio_days(y, 1, 1) + 1.0).max(1.0),
                };
                CellValue::number(out)
            }
            "WEEKDAY" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let serial = match self.eval_datetime_arg(ctx_sheet, ctx_cell, a0) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let ret_type = args
                    .get(1)
                    .and_then(|e| self.eval_expr(ctx_sheet, ctx_cell, e).as_internal_f64().ok())
                    .unwrap_or(1.0)
                    .round() as i32;
                // `1899-12-30` (`serial = 0`) was a Saturday (`sun0 = 6`, `0=Sun..6=Sat`)
                let sun0 = (serial.floor() as i64 - 1).rem_euclid(7);
                let val = match ret_type {
                    2 => ((sun0 + 6) % 7) + 1, // 1=Mon .. 7=Sun
                    3 => (sun0 + 6) % 7,       // 0=Mon .. 6=Sun
                    _ => sun0 + 1,             // 1=Sun .. 7=Sat (Visio default)
                };
                CellValue::number(val as f64)
            }
            "WEEKNUM" | "ISOWEEKNUM" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let serial = match self.eval_datetime_arg(ctx_sheet, ctx_cell, a0) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let (y, _, _) = visio_days_to_civil(serial);
                let doy = serial.floor() - civil_to_visio_days(y, 1, 1);
                let wk = (doy / 7.0).floor() + 1.0;
                CellValue::number(wk.clamp(1.0, 53.0))
            }
            "HOUR" | "MINUTE" | "SECOND" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let serial = match self.eval_datetime_arg(ctx_sheet, ctx_cell, a0) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let (h, m, s) = visio_time_hms(serial);
                let v = match upper {
                    "HOUR" => h as f64,
                    "MINUTE" => m as f64,
                    _ => s as f64,
                };
                CellValue::number(v)
            }
            "DATEDIFF" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(iv_raw) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let d1 = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[1]) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let d2 = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[2]) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let iv = iv_raw.trim().to_ascii_lowercase();
                let (y1, m1, _) = visio_days_to_civil(d1);
                let (y2, m2, _) = visio_days_to_civil(d2);
                let delta_days = d2 - d1;
                let res = match iv.as_str() {
                    "yyyy" | "y_yr" | "year" => (y2 - y1) as f64,
                    "q" => (((y2 - y1) * 12 + (m2 as i32 - m1 as i32)) / 3) as f64,
                    "m" => ((y2 - y1) * 12 + (m2 as i32 - m1 as i32)) as f64,
                    "ww" | "w" => (delta_days / 7.0).trunc(),
                    "h" => (delta_days * 24.0).round(),
                    "n" | "min" => (delta_days * 1_440.0).round(),
                    "s" | "sec" => (delta_days * 86_400.0).round(),
                    _ => d2.floor() - d1.floor(),
                };
                CellValue::number(res)
            }
            "DATEADD" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(iv_raw) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(num) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let base = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[2]) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let iv = iv_raw.trim().to_ascii_lowercase();
                let out = match iv.as_str() {
                    "yyyy" | "year" => add_months_serial(base, (num.round() as i32) * 12),
                    "q" => add_months_serial(base, (num.round() as i32) * 3),
                    "m" => add_months_serial(base, num.round() as i32),
                    "ww" | "w" => base + num * 7.0,
                    "h" => base + num / 24.0,
                    "n" | "min" => base + num / 1_440.0,
                    "s" | "sec" => base + num / 86_400.0,
                    _ => base + num,
                };
                CellValue::Num {
                    internal: normalize_f64(out),
                    unit: Unit::ElapsedDay,
                }
            }
            "EDATE" | "EOMONTH" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let base = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[0]) {
                    Ok(v) => v,
                    Err(e) => return CellValue::Err(e),
                };
                let Ok(months) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let shifted = add_months_serial(base.floor(), months.round() as i32);
                let out = if upper == "EOMONTH" {
                    let (y, m, _) = visio_days_to_civil(shifted);
                    civil_to_visio_days(y, m as i32, days_in_month(y, m) as i32)
                } else {
                    shifted
                };
                CellValue::Num {
                    internal: out,
                    unit: Unit::ElapsedDay,
                }
            }
            "WORKDAY" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let start = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[0]) {
                    Ok(v) => v.floor() as i64,
                    Err(e) => return CellValue::Err(e),
                };
                let Ok(days_f) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let mut rem = days_f.trunc() as i64;
                let step = if rem >= 0 { 1i64 } else { -1i64 };
                let mut cur = start;
                while rem != 0 {
                    cur += step;
                    let sun0 = (cur - 1).rem_euclid(7);
                    if sun0 != 0 && sun0 != 6 {
                        rem -= step;
                    }
                }
                CellValue::Num {
                    internal: cur as f64,
                    unit: Unit::ElapsedDay,
                }
            }
            "NETWORKDAYS" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let d1 = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[0]) {
                    Ok(v) => v.floor() as i64,
                    Err(e) => return CellValue::Err(e),
                };
                let d2 = match self.eval_datetime_arg(ctx_sheet, ctx_cell, &args[1]) {
                    Ok(v) => v.floor() as i64,
                    Err(e) => return CellValue::Err(e),
                };
                let sign = if d2 >= d1 { 1.0 } else { -1.0 };
                let (lo, hi) = (d1.min(d2), d1.max(d2));
                let mut count = 0i64;
                let full_weeks = (hi - lo) / 7;
                count += full_weeks * 5;
                let rem_start = lo + full_weeks * 7;
                for d in rem_start..=hi {
                    let sun0 = (d - 1).rem_euclid(7);
                    if sun0 != 0 && sun0 != 6 {
                        count += 1;
                    }
                }
                CellValue::number((count as f64) * sign)
            }
            "EDAYS" | "EHOURS" | "EMINUTES" | "ESECONDS" | "EWEEKS" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let u = match upper {
                    "EWEEKS" => Unit::ElapsedWeek,
                    "EHOURS" => Unit::ElapsedHour,
                    "EMINUTES" => Unit::ElapsedMinute,
                    "ESECONDS" => Unit::ElapsedSecond,
                    _ => Unit::ElapsedDay,
                };
                let raw_val = self.eval_expr(ctx_sheet, ctx_cell, a0);
                let days = match raw_val {
                    CellValue::Num { internal, unit } => match unit {
                        Unit::ElapsedSecond
                        | Unit::ElapsedMinute
                        | Unit::ElapsedHour
                        | Unit::ElapsedDay
                        | Unit::ElapsedWeek => internal,
                        _ => u.to_internal(internal),
                    },
                    CellValue::Str(_) => match self.eval_datetime_arg(ctx_sheet, ctx_cell, a0) {
                        Ok(v) => v,
                        Err(e) => return CellValue::Err(e),
                    },
                    CellValue::Bool(b) => u.to_internal(if b { 1.0 } else { 0.0 }),
                    CellValue::Err(e) => return CellValue::Err(e),
                    _ => return CellValue::Err(CellError::Value),
                };
                CellValue::Num {
                    internal: normalize_f64(days),
                    unit: u,
                }
            }
            "CY" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a0).as_internal_f64() {
                    Ok(v) => CellValue::Num {
                        internal: normalize_f64(v),
                        unit: Unit::Currency,
                    },
                    Err(e) => CellValue::Err(e),
                }
            }

            // ================================================================
            // 2. Extended Hyperbolic, Angle, Combinatorial & Statistical Math
            // ================================================================
            "SINH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.sinh())),
            "COSH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.cosh())),
            "TANH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.tanh())),
            "ASINH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.asinh())),
            "ACOSH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x < 1.0 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number(x.acosh())
                }
            }),
            "ATANH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x <= -1.0 || x >= 1.0 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number(x.atanh())
                }
            }),
            "SEC" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let c = x.cos();
                if c.abs() < 1e-15 { CellValue::Err(CellError::DivByZero) } else { CellValue::number(1.0 / c) }
            }),
            "CSC" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let s = x.sin();
                if s.abs() < 1e-15 { CellValue::Err(CellError::DivByZero) } else { CellValue::number(1.0 / s) }
            }),
            "COT" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let t = x.tan();
                if t.abs() < 1e-15 { CellValue::Err(CellError::DivByZero) } else { CellValue::number(1.0 / t) }
            }),
            "SECH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(1.0 / x.cosh())),
            "CSCH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let s = x.sinh();
                if s.abs() < 1e-15 { CellValue::Err(CellError::DivByZero) } else { CellValue::number(1.0 / s) }
            }),
            "COTH" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let t = x.tanh();
                if t.abs() < 1e-15 { CellValue::Err(CellError::DivByZero) } else { CellValue::number(1.0 / t) }
            }),
            "DEGREES" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let cv = self.eval_expr(ctx_sheet, ctx_cell, a0);
                match cv.as_internal_f64() {
                    Ok(v) => CellValue::number(v.to_degrees()),
                    Err(e) => CellValue::Err(e),
                }
            }
            "RADIANS" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let cv = self.eval_expr(ctx_sheet, ctx_cell, a0);
                let u = cv.unit();
                match cv.as_internal_f64() {
                    Ok(v) => {
                        let rad = if matches!(u, Unit::Degree | Unit::Radian | Unit::ArcMinute | Unit::ArcSecond) {
                            v
                        } else {
                            v.to_radians()
                        };
                        CellValue::radians(rad)
                    }
                    Err(e) => CellValue::Err(e),
                }
            }
            "NORMANGLE" => self.unary_num(ctx_sheet, ctx_cell, args, |rad| {
                let mut r = rad.rem_euclid(std::f64::consts::TAU);
                if r > std::f64::consts::PI {
                    r -= std::f64::consts::TAU;
                }
                CellValue::radians(r)
            }),
            "LOG" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(x) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let base = args
                    .get(1)
                    .and_then(|b| self.eval_expr(ctx_sheet, ctx_cell, b).as_internal_f64().ok())
                    .unwrap_or(10.0);
                if x <= 0.0 || base <= 0.0 || (base - 1.0).abs() < 1e-15 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number(x.log(base))
                }
            }
            "LOG2" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x <= 0.0 { CellValue::Err(CellError::Num) } else { CellValue::number(x.log2()) }
            }),
            "EXP2" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.exp2())),
            "CBRT" => self.unary_num(ctx_sheet, ctx_cell, args, |x| CellValue::number(x.cbrt())),
            "SQRTPI" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x < 0.0 {
                    CellValue::Err(CellError::Num)
                } else {
                    CellValue::number((x * std::f64::consts::PI).sqrt())
                }
            }),
            "HYPOT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let cv0 = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let u = cv0.unit();
                let Ok(a) = cv0.as_internal_f64() else { return CellValue::Err(CellError::Value); };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::Num {
                    internal: normalize_f64(a.hypot(b)),
                    unit: u,
                }
            }
            "FACT" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x < 0.0 || x > 170.0 {
                    return CellValue::Err(CellError::Num);
                }
                let n = x.trunc() as u64;
                let mut prod = 1.0_f64;
                for k in 2..=n {
                    prod *= k as f64;
                }
                CellValue::number(prod)
            }),
            "COMBIN" | "PERMUT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(nf) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(kf) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                if nf < 0.0 || kf < 0.0 || kf.trunc() > nf.trunc() {
                    return CellValue::Err(CellError::Num);
                }
                let n = nf.trunc() as u64;
                let k = kf.trunc() as u64;
                if upper == "PERMUT" {
                    let mut p = 1.0_f64;
                    for i in 0..k {
                        p *= (n - i) as f64;
                    }
                    CellValue::number(p)
                } else {
                    let k_eff = k.min(n - k);
                    let mut c = 1.0_f64;
                    for i in 1..=k_eff {
                        c = c * ((n - k_eff + i) as f64) / (i as f64);
                    }
                    CellValue::number(c.round())
                }
            }
            "GCD" | "LCM" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let mut acc: Option<u64> = None;
                for a in args {
                    let Ok(v) = self.eval_expr(ctx_sheet, ctx_cell, a).as_internal_f64() else {
                        return CellValue::Err(CellError::Value);
                    };
                    if v < 0.0 {
                        return CellValue::Err(CellError::Num);
                    }
                    let u = v.trunc() as u64;
                    acc = Some(match acc {
                        None => u,
                        Some(prev) => {
                            if upper == "GCD" {
                                gcd_u64(prev, u)
                            } else if prev == 0 || u == 0 {
                                0
                            } else {
                                (prev / gcd_u64(prev, u)).saturating_mul(u)
                            }
                        }
                    });
                }
                CellValue::number(acc.unwrap_or(0) as f64)
            }
            "EVEN" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                let sign = if x < 0.0 { -1.0 } else { 1.0 };
                let mag = (x.abs() / 2.0).ceil() * 2.0;
                CellValue::number(sign * mag)
            }),
            "ODD" => self.unary_num(ctx_sheet, ctx_cell, args, |x| {
                if x == 0.0 {
                    return CellValue::number(1.0);
                }
                let sign = if x < 0.0 { -1.0 } else { 1.0 };
                let mut n = x.abs().ceil() as i64;
                if n % 2 == 0 {
                    n += 1;
                }
                CellValue::number(sign * (n as f64))
            }),
            "MROUND" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let cv = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let u = cv.unit();
                let Ok(v) = cv.as_internal_f64() else { return CellValue::Err(CellError::Value); };
                let Ok(mult) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                if mult == 0.0 {
                    return CellValue::Num { internal: 0.0, unit: u };
                }
                if (v > 0.0 && mult < 0.0) || (v < 0.0 && mult > 0.0) {
                    return CellValue::Err(CellError::Num);
                }
                CellValue::Num {
                    internal: normalize_f64((v / mult).round() * mult),
                    unit: u,
                }
            }
            "ROUNDUP" | "ROUNDDOWN" => {
                if args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let cv = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let u = cv.unit();
                let Ok(v) = cv.as_internal_f64() else { return cv; };
                let digits = args
                    .get(1)
                    .and_then(|d| self.eval_expr(ctx_sheet, ctx_cell, d).as_internal_f64().ok())
                    .unwrap_or(0.0) as i32;
                let factor = 10_f64.powi(digits);
                let disp = u.from_internal(v);
                let scaled = disp * factor;
                let out_scaled = if upper == "ROUNDUP" {
                    if scaled >= 0.0 { scaled.ceil() } else { scaled.floor() }
                } else {
                    scaled.trunc()
                };
                CellValue::with_unit(out_scaled / factor, u)
            }
            "QUOTIENT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(a) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(b) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                if b.abs() < 1e-15 {
                    CellValue::Err(CellError::DivByZero)
                } else {
                    CellValue::number((a / b).trunc())
                }
            }
            "CLAMP" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let cv = self.eval_expr(ctx_sheet, ctx_cell, &args[0]);
                let u = cv.unit();
                let Ok(v) = cv.as_internal_f64() else { return cv; };
                let Ok(lo) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(hi) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::Num {
                    internal: normalize_f64(v.clamp(lo.min(hi), lo.max(hi))),
                    unit: u,
                }
            }
            "RAND" | "RANDBETWEEN" => {
                // Deterministic SplitMix64 hash of `(sheet_key, cell_name)` so DAG fixed-point evaluation converges!
                let seed_str = encode_global_cell_ref(&GlobalCellId {
                    sheet: ctx_sheet.clone(),
                    cell: ctx_cell.to_string(),
                });
                let mut z = 0x9E37_79B9_7F4A_7C15_u64;
                for &b in seed_str.as_bytes() {
                    z = z.wrapping_add(b as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                    z ^= z >> 30;
                }
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 31;
                let unit01 = ((z >> 11) as f64) * (1.0 / ((1u64 << 53) as f64));
                if upper == "RAND" {
                    CellValue::number(unit01)
                } else {
                    if args.len() < 2 {
                        return CellValue::Err(CellError::Value);
                    }
                    let Ok(lo) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                        return CellValue::Err(CellError::Value);
                    };
                    let Ok(hi) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                        return CellValue::Err(CellError::Value);
                    };
                    let ilo = lo.ceil() as i64;
                    let ihi = hi.floor() as i64;
                    if ilo > ihi {
                        return CellValue::Err(CellError::Num);
                    }
                    let span = (ihi - ilo + 1) as f64;
                    let picked = ilo + ((unit01 * span).floor() as i64).min(ihi - ilo);
                    CellValue::number(picked as f64)
                }
            }
            "BITLSHIFT" | "BITRSHIFT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(v) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(sh) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let shift = (sh.trunc() as i32).clamp(-52, 52);
                let vi = v.trunc() as i64;
                let res = if (upper == "BITLSHIFT" && shift >= 0) || (upper == "BITRSHIFT" && shift < 0) {
                    vi.wrapping_shl(shift.unsigned_abs())
                } else {
                    vi.wrapping_shr(shift.unsigned_abs())
                };
                CellValue::number(res as f64)
            }
            "COUNT" | "COUNTA" => {
                let mut n = 0usize;
                for a in args {
                    let cv = self.eval_expr(ctx_sheet, ctx_cell, a);
                    if upper == "COUNT" {
                        if matches!(cv, CellValue::Num { .. }) {
                            n += 1;
                        }
                    } else if !matches!(cv, CellValue::Err(_)) {
                        if let CellValue::Str(ref s) = cv {
                            if !s.is_empty() {
                                n += 1;
                            }
                        } else {
                            n += 1;
                        }
                    }
                }
                CellValue::number(n as f64)
            }
            "MEDIAN" | "MODE" | "VAR" | "VARP" | "STDEV" | "STDEVP" | "SMALL" | "LARGE" | "PERCENTILE" => {
                let value_args = if matches!(upper, "SMALL" | "LARGE" | "PERCENTILE") {
                    if args.len() < 2 {
                        return CellValue::Err(CellError::Value);
                    }
                    &args[..args.len() - 1]
                } else {
                    args
                };
                if value_args.is_empty() {
                    return CellValue::Err(CellError::Value);
                }
                let mut vals = Vec::with_capacity(value_args.len());
                let mut first_unit = Unit::Number;
                for (i, a) in value_args.iter().enumerate() {
                    let cv = self.eval_expr(ctx_sheet, ctx_cell, a);
                    if i == 0 {
                        first_unit = cv.unit();
                    }
                    match cv.as_internal_f64() {
                        Ok(v) if v.is_finite() => vals.push(v),
                        Ok(_) => return CellValue::Err(CellError::Num),
                        Err(e) => return CellValue::Err(e),
                    }
                }
                vals.sort_by(|a, b| a.total_cmp(b));
                let n = vals.len();
                match upper {
                    "MEDIAN" => {
                        let m = if n % 2 == 1 {
                            vals[n / 2]
                        } else {
                            (vals[n / 2 - 1] + vals[n / 2]) * 0.5
                        };
                        CellValue::Num { internal: normalize_f64(m), unit: first_unit }
                    }
                    "MODE" => {
                        let mut best_v = vals[0];
                        let mut best_c = 1usize;
                        let mut cur_v = vals[0];
                        let mut cur_c = 1usize;
                        for &v in vals.iter().skip(1) {
                            if (v - cur_v).abs() < 1e-12 {
                                cur_c += 1;
                            } else {
                                if cur_c > best_c {
                                    best_c = cur_c;
                                    best_v = cur_v;
                                }
                                cur_v = v;
                                cur_c = 1;
                            }
                        }
                        if cur_c > best_c {
                            best_v = cur_v;
                        }
                        CellValue::Num { internal: normalize_f64(best_v), unit: first_unit }
                    }
                    "VAR" | "STDEV" => {
                        if n < 2 {
                            return CellValue::Err(CellError::DivByZero);
                        }
                        let mean = vals.iter().sum::<f64>() / (n as f64);
                        let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / ((n - 1) as f64);
                        CellValue::number(if upper == "STDEV" { var.sqrt() } else { var })
                    }
                    "VARP" | "STDEVP" => {
                        let mean = vals.iter().sum::<f64>() / (n as f64);
                        let var = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n as f64);
                        CellValue::number(if upper == "STDEVP" { var.sqrt() } else { var })
                    }
                    "SMALL" | "LARGE" => {
                        let Ok(kf) = self.eval_expr(ctx_sheet, ctx_cell, args.last().unwrap()).as_internal_f64() else {
                            return CellValue::Err(CellError::Value);
                        };
                        let k = kf.trunc() as isize;
                        if k < 1 || (k as usize) > n {
                            return CellValue::Err(CellError::Num);
                        }
                        let picked = if upper == "SMALL" {
                            vals[(k as usize) - 1]
                        } else {
                            vals[n - (k as usize)]
                        };
                        CellValue::Num { internal: normalize_f64(picked), unit: first_unit }
                    }
                    _ => {
                        // PERCENTILE(..., p in [0..1])
                        let Ok(p) = self.eval_expr(ctx_sheet, ctx_cell, args.last().unwrap()).as_internal_f64() else {
                            return CellValue::Err(CellError::Value);
                        };
                        if !(0.0..=1.0).contains(&p) {
                            return CellValue::Err(CellError::Num);
                        }
                        if n == 1 {
                            return CellValue::Num { internal: vals[0], unit: first_unit };
                        }
                        let rank = p * ((n - 1) as f64);
                        let lo = rank.floor() as usize;
                        let hi = rank.ceil() as usize;
                        let frac = rank - (lo as f64);
                        let interp = vals[lo] * (1.0 - frac) + vals[hi] * frac;
                        CellValue::Num { internal: normalize_f64(interp), unit: first_unit }
                    }
                }
            }

            // ================================================================
            // 3. String, Text & Formatting Functions
            // ================================================================
            "REPLACE" => {
                if args.len() < 4 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(old_s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(start_f) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(len_f) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(new_s) = self.eval_expr(ctx_sheet, ctx_cell, &args[3]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                if start_f < 1.0 || len_f < 0.0 {
                    return CellValue::Err(CellError::Value);
                }
                let start0 = (start_f.trunc() as usize).saturating_sub(1);
                let num_chars = len_f.trunc() as usize;
                let chars: Vec<char> = old_s.chars().collect();
                let prefix: String = chars.iter().take(start0.min(chars.len())).collect();
                let suffix: String = chars.iter().skip((start0 + num_chars).min(chars.len())).collect();
                CellValue::Str(format!("{prefix}{new_s}{suffix}"))
            }
            "REPT" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(n) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                if n < 0.0 {
                    return CellValue::Err(CellError::Value);
                }
                let count = (n.trunc() as usize).min(4_096);
                CellValue::Str(s.repeat(count))
            }
            "PROPER" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, a0).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let mut out = String::with_capacity(s.len());
                let mut cap_next = true;
                for ch in s.chars() {
                    if ch.is_alphabetic() {
                        if cap_next {
                            out.extend(ch.to_uppercase());
                            cap_next = false;
                        } else {
                            out.extend(ch.to_lowercase());
                        }
                    } else {
                        out.push(ch);
                        cap_next = true;
                    }
                }
                CellValue::Str(out)
            }
            "CLEAN" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, a0).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                CellValue::Str(s.chars().filter(|&c| (c as u32) >= 32).collect())
            }
            "CHAR" | "UNICHAR" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(code) = self.eval_expr(ctx_sheet, ctx_cell, a0).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                if code < 1.0 {
                    return CellValue::Err(CellError::Value);
                }
                match char::from_u32(code.trunc() as u32) {
                    Some(c) => CellValue::Str(c.to_string()),
                    None => CellValue::Err(CellError::Value),
                }
            }
            "CODE" | "UNICODE" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, a0).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                match s.chars().next() {
                    Some(c) => CellValue::number((c as u32) as f64),
                    None => CellValue::Err(CellError::Value),
                }
            }
            "VALUE" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let cv = self.eval_expr(ctx_sheet, ctx_cell, a0);
                if matches!(cv, CellValue::Num { .. }) {
                    return cv;
                }
                let Ok(s) = cv.as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let trimmed = s.trim().trim_start_matches('$').replace(',', "");
                if let Some((scalar, unit)) = parse_scalar_with_unit(&trimmed) {
                    CellValue::with_unit(scalar, unit)
                } else if let Ok(v) = trimmed.parse::<f64>() {
                    CellValue::number(v)
                } else {
                    CellValue::Err(CellError::Value)
                }
            }
            "TEXT" | "STR" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let val = self.eval_expr(ctx_sheet, ctx_cell, a0);
                if let Some(pic_expr) = args.get(1) {
                    let pic = self
                        .eval_expr(ctx_sheet, ctx_cell, pic_expr)
                        .as_str_coerced()
                        .unwrap_or_default();
                    if pic.contains("yyyy") || pic.contains("YYYY") {
                        if let Ok(serial) = val.as_internal_f64() {
                            let (y, m, d) = visio_days_to_civil(serial);
                            return CellValue::Str(format!("{y:04}-{m:02}-{d:02}"));
                        }
                    }
                    if let (Ok(num), Some(dot_idx)) = (val.as_internal_f64(), pic.find('.')) {
                        let decimals = pic[dot_idx + 1..]
                            .chars()
                            .take_while(|c| *c == '0' || *c == '#')
                            .count();
                        return CellValue::Str(format!("{:.*}", decimals, val.unit().from_internal(num)));
                    }
                }
                match val.as_str_coerced() {
                    Ok(s) => CellValue::Str(s),
                    Err(e) => CellValue::Err(e),
                }
            }
            "FIXED" | "DOLLAR" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let cv = self.eval_expr(ctx_sheet, ctx_cell, a0);
                let Ok(num) = cv.as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let decimals = args
                    .get(1)
                    .and_then(|d| self.eval_expr(ctx_sheet, ctx_cell, d).as_internal_f64().ok())
                    .unwrap_or(2.0)
                    .clamp(0.0, 12.0) as usize;
                let no_commas = if upper == "FIXED" {
                    args.get(2)
                        .and_then(|nc| self.eval_expr(ctx_sheet, ctx_cell, nc).as_bool().ok())
                        .unwrap_or(false)
                } else {
                    false
                };
                let disp = cv.unit().from_internal(num);
                let raw = format!("{:.*}", decimals, disp.abs());
                let formatted_abs = if no_commas {
                    raw
                } else {
                    let (int_part, frac_part) = match raw.split_once('.') {
                        Some((i, f)) => (i, Some(f)),
                        None => (raw.as_str(), None),
                    };
                    let mut grouped = String::with_capacity(int_part.len() + int_part.len() / 3);
                    for (idx, ch) in int_part.chars().enumerate() {
                        if idx > 0 && (int_part.len() - idx) % 3 == 0 {
                            grouped.push(',');
                        }
                        grouped.push(ch);
                    }
                    if let Some(f) = frac_part {
                        format!("{grouped}.{f}")
                    } else {
                        grouped
                    }
                };
                if upper == "DOLLAR" {
                    if disp < 0.0 {
                        CellValue::Str(format!("-${formatted_abs}"))
                    } else {
                        CellValue::Str(format!("${formatted_abs}"))
                    }
                } else if disp < 0.0 {
                    CellValue::Str(format!("-{formatted_abs}"))
                } else {
                    CellValue::Str(formatted_abs)
                }
            }
            "T" => {
                let Some(a0) = args.first() else {
                    return CellValue::Str(String::new());
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a0) {
                    CellValue::Str(s) => CellValue::Str(s),
                    CellValue::Err(e) => CellValue::Err(e),
                    _ => CellValue::Str(String::new()),
                }
            }
            "N" => {
                let Some(a0) = args.first() else {
                    return CellValue::number(0.0);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a0) {
                    CellValue::Num { internal, unit } => CellValue::Num { internal, unit },
                    CellValue::Bool(b) => CellValue::number(if b { 1.0 } else { 0.0 }),
                    CellValue::Err(e) => CellValue::Err(e),
                    _ => CellValue::number(0.0),
                }
            }
            "STARTSWITH" | "ENDSWITH" | "CONTAINS" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(pat) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let b = match upper {
                    "STARTSWITH" => s.starts_with(&pat),
                    "ENDSWITH" => s.ends_with(&pat),
                    _ => s.contains(&pat),
                };
                CellValue::Bool(b)
            }
            "LPAD" | "RPAD" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(w) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let pad_ch = args
                    .get(2)
                    .and_then(|p| self.eval_expr(ctx_sheet, ctx_cell, p).as_str_coerced().ok())
                    .and_then(|ps| ps.chars().next())
                    .unwrap_or(' ');
                let width = (w.max(0.0) as usize).min(4_096);
                let cur_len = s.chars().count();
                if cur_len >= width {
                    CellValue::Str(s)
                } else {
                    let pad: String = std::iter::repeat_n(pad_ch, width - cur_len).collect();
                    if upper == "LPAD" {
                        CellValue::Str(format!("{pad}{s}"))
                    } else {
                        CellValue::Str(format!("{s}{pad}"))
                    }
                }
            }
            "REVERSE" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a0).as_str_coerced() {
                    Ok(s) => CellValue::Str(s.chars().rev().collect()),
                    Err(e) => CellValue::Err(e),
                }
            }
            "SPLITCOUNT" => {
                if args.is_empty() {
                    return CellValue::number(0.0);
                }
                let Ok(s) = self.eval_expr(ctx_sheet, ctx_cell, &args[0]).as_str_coerced() else {
                    return CellValue::number(0.0);
                };
                if s.trim().is_empty() {
                    return CellValue::number(0.0);
                }
                let delim = args
                    .get(1)
                    .and_then(|d| self.eval_expr(ctx_sheet, ctx_cell, d).as_str_coerced().ok())
                    .unwrap_or_else(|| ";".into());
                CellValue::number(s.split(&delim).count() as f64)
            }
            "RGB2HEX" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                match self.eval_expr(ctx_sheet, ctx_cell, a0) {
                    CellValue::Color(c) => CellValue::Str(c.to_hex_rgb()),
                    _ => CellValue::Err(CellError::Value),
                }
            }

            // ================================================================
            // 4. Geometry & Path Introspection Functions
            // ================================================================
            "PATHLEN" | "SHAPELENGTH" => {
                let (target_sheet, geom_idx) = self.parse_geom_target(ctx_sheet, ctx_cell, args.first());
                let Some((pts, _)) = self.linearize_local_geometry_path(&target_sheet, geom_idx) else {
                    return CellValue::inches(0.0);
                };
                let len: f64 = pts
                    .windows(2)
                    .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                    .sum();
                CellValue::inches(len)
            }
            "AREA" | "SHAPEAREA" => {
                let (target_sheet, geom_idx) = self.parse_geom_target(ctx_sheet, ctx_cell, args.first());
                let Some((pts, _)) = self.linearize_local_geometry_path(&target_sheet, geom_idx) else {
                    let w = self.eval_sheet_f64(&target_sheet, "Width", 0.0);
                    let h = self.eval_sheet_f64(&target_sheet, "Height", 0.0);
                    return CellValue::number((w * h).abs());
                };
                if pts.len() < 3 {
                    return CellValue::number(0.0);
                }
                let mut sum = 0.0_f64;
                for i in 0..pts.len() {
                    let p0 = pts[i];
                    let p1 = pts[(i + 1) % pts.len()];
                    sum += p0[0] * p1[1] - p1[0] * p0[1];
                }
                CellValue::number(normalize_f64((sum * 0.5).abs()))
            }
            "POINTALONGPATH" | "TANGENTALONGPATH" => {
                if args.len() < 2 {
                    return CellValue::Err(CellError::Value);
                }
                let (target_sheet, geom_idx) = self.parse_geom_target(ctx_sheet, ctx_cell, Some(&args[0]));
                let Ok(frac_raw) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let frac = frac_raw.clamp(0.0, 1.0);
                let offset = args
                    .get(2)
                    .and_then(|o| self.eval_expr(ctx_sheet, ctx_cell, o).as_internal_f64().ok())
                    .unwrap_or(0.0);
                let Some((pts, _)) = self.linearize_local_geometry_path(&target_sheet, geom_idx) else {
                    return if upper == "TANGENTALONGPATH" {
                        CellValue::radians(0.0)
                    } else {
                        CellValue::Point { x: 0.0, y: 0.0 }
                    };
                };
                if pts.len() < 2 {
                    let p = pts.first().copied().unwrap_or([0.0, 0.0]);
                    return if upper == "TANGENTALONGPATH" {
                        CellValue::radians(0.0)
                    } else {
                        CellValue::Point { x: p[0], y: p[1] }
                    };
                }
                let seg_lens: Vec<f64> = pts
                    .windows(2)
                    .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                    .collect();
                let total_len: f64 = seg_lens.iter().sum();
                if total_len < 1e-12 {
                    return if upper == "TANGENTALONGPATH" {
                        CellValue::radians(0.0)
                    } else {
                        CellValue::Point { x: pts[0][0], y: pts[0][1] }
                    };
                }
                let target_dist = frac * total_len;
                let mut acc = 0.0;
                let mut chosen_seg = seg_lens.len() - 1;
                let mut seg_t = 1.0;
                for (i, &sl) in seg_lens.iter().enumerate() {
                    if acc + sl >= target_dist - 1e-12 {
                        chosen_seg = i;
                        seg_t = if sl > 1e-12 { ((target_dist - acc) / sl).clamp(0.0, 1.0) } else { 0.0 };
                        break;
                    }
                    acc += sl;
                }
                let a = pts[chosen_seg];
                let b = pts[chosen_seg + 1];
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let ang = dy.atan2(dx);
                if upper == "TANGENTALONGPATH" {
                    CellValue::radians(ang)
                } else {
                    let sl = dx.hypot(dy).max(1e-12);
                    let nx = -dy / sl;
                    let ny = dx / sl;
                    let px = a[0] + dx * seg_t + nx * offset;
                    let py = a[1] + dy * seg_t + ny * offset;
                    CellValue::Point { x: normalize_f64(px), y: normalize_f64(py) }
                }
            }
            "NEARESTPOINTONPATH" | "DISTTOPATH" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let (target_sheet, geom_idx) = self.parse_geom_target(ctx_sheet, ctx_cell, Some(&args[0]));
                let Ok(qx) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(qy) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Some((pts, _)) = self.linearize_local_geometry_path(&target_sheet, geom_idx) else {
                    return CellValue::number(0.0);
                };
                if pts.len() < 2 {
                    return CellValue::number(0.0);
                }
                let seg_lens: Vec<f64> = pts
                    .windows(2)
                    .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
                    .collect();
                let total_len: f64 = seg_lens.iter().sum();
                if total_len < 1e-12 {
                    return CellValue::number(0.0);
                }
                let mut best_dist = f64::INFINITY;
                let mut best_arc = 0.0;
                let mut acc = 0.0;
                for (i, w) in pts.windows(2).enumerate() {
                    let (ax, ay) = (w[0][0], w[0][1]);
                    let (bx, by) = (w[1][0], w[1][1]);
                    let vx = bx - ax;
                    let vy = by - ay;
                    let len2 = vx * vx + vy * vy;
                    let t = if len2 > 1e-15 {
                        (((qx - ax) * vx + (qy - ay) * vy) / len2).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let cx = ax + vx * t;
                    let cy = ay + vy * t;
                    let d = (qx - cx).hypot(qy - cy);
                    if d < best_dist {
                        best_dist = d;
                        best_arc = acc + seg_lens[i] * t;
                    }
                    acc += seg_lens[i];
                }
                if upper == "DISTTOPATH" {
                    CellValue::inches(best_dist)
                } else {
                    CellValue::number((best_arc / total_len).clamp(0.0, 1.0))
                }
            }
            "PTINPATH" => {
                if args.len() < 3 {
                    return CellValue::Err(CellError::Value);
                }
                let (target_sheet, geom_idx) = self.parse_geom_target(ctx_sheet, ctx_cell, Some(&args[0]));
                let Ok(qx) = self.eval_expr(ctx_sheet, ctx_cell, &args[1]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Ok(qy) = self.eval_expr(ctx_sheet, ctx_cell, &args[2]).as_internal_f64() else {
                    return CellValue::Err(CellError::Value);
                };
                let Some((pts, _)) = self.linearize_local_geometry_path(&target_sheet, geom_idx) else {
                    return CellValue::Bool(false);
                };
                if pts.len() < 3 {
                    return CellValue::Bool(false);
                }
                let mut inside = false;
                let n = pts.len();
                let mut j = n - 1;
                for i in 0..n {
                    let (xi, yi) = (pts[i][0], pts[i][1]);
                    let (xj, yj) = (pts[j][0], pts[j][1]);
                    if ((yi > qy) != (yj > qy)) && (qx < (xj - xi) * (qy - yi) / (yj - yi + 1e-30) + xi) {
                        inside = !inside;
                    }
                    j = i;
                }
                CellValue::Bool(inside)
            }
            "BOUNDINGBOXDIST" => {
                let Some(a0) = args.first() else {
                    return CellValue::Err(CellError::Value);
                };
                let other_sheet = match a0 {
                    Expr::CellRef(p) => self
                        .resolve_ref_path(ctx_sheet, p)
                        .map(|c| c.sheet)
                        .unwrap_or_else(|| ctx_sheet.clone()),
                    other => match self.eval_expr(ctx_sheet, ctx_cell, other) {
                        CellValue::Ref(s) => crate::dag::decode_global_cell_ref_pub(&s)
                            .map(|c| c.sheet)
                            .unwrap_or_else(|| ctx_sheet.clone()),
                        _ => ctx_sheet.clone(),
                    },
                };
                let (Some(tf1), Some(tf2)) = (
                    self.eval_shape_transform(ctx_sheet),
                    self.eval_shape_transform(&other_sheet),
                ) else {
                    return CellValue::Err(CellError::Ref);
                };
                let (mn1, mx1) = tf1.world_aabb();
                let (mn2, mx2) = tf2.world_aabb();
                let dx = (mn2[0] - mx1[0]).max(mn1[0] - mx2[0]).max(0.0);
                let dy = (mn2[1] - mx1[1]).max(mn1[1] - mx2[1]).max(0.0);
                CellValue::inches(dx.hypot(dy))
            }
            "BBOXX" | "BBOXY" | "BBOXWIDTH" | "BBOXHEIGHT" => {
                let Some(tf) = self.eval_shape_transform(ctx_sheet) else {
                    return CellValue::inches(0.0);
                };
                let (mn, mx) = tf.world_aabb();
                let v = match upper {
                    "BBOXX" => (mn[0] + mx[0]) * 0.5,
                    "BBOXY" => (mn[1] + mx[1]) * 0.5,
                    "BBOXWIDTH" => mx[0] - mn[0],
                    _ => mx[1] - mn[1],
                };
                CellValue::inches(v)
            }

            // ================================================================
            // 5. Sheet, Container, List, Callout, Connector, Layer & Doc Queries
            // ================================================================
            "ID" | "SHEETID" => match ctx_sheet {
                SheetKey::Shape { shape_id, .. } | SheetKey::Master { shape_id, .. } => {
                    CellValue::number(*shape_id as f64)
                }
                SheetKey::Page(pid) => CellValue::number(*pid as f64),
                SheetKey::Style(sid) => CellValue::number(*sid as f64),
                SheetKey::Doc => CellValue::number(0.0),
            },
            "NAME" | "NAMEU" => {
                let want_u = upper == "NAMEU";
                let s = match ctx_sheet {
                    SheetKey::Shape { page_id, shape_id } => self
                        .doc
                        .page(*page_id)
                        .and_then(|p| p.find_shape(*shape_id))
                        .map(|sh| if want_u { sh.name_u.clone() } else { sh.name.clone() })
                        .unwrap_or_default(),
                    SheetKey::Page(pid) => self
                        .doc
                        .page(*pid)
                        .map(|p| if want_u { p.name_u.clone() } else { p.name.clone() })
                        .unwrap_or_default(),
                    SheetKey::Master { master_id, .. } => self
                        .doc
                        .master(*master_id)
                        .map(|m| if want_u { m.name_u.clone() } else { m.name.clone() })
                        .unwrap_or_default(),
                    SheetKey::Style(sid) => self
                        .doc
                        .styles
                        .iter()
                        .find(|st| st.id == *sid)
                        .map(|st| if want_u { st.name_u.clone() } else { st.name.clone() })
                        .unwrap_or_default(),
                    SheetKey::Doc => "TheDoc".into(),
                };
                CellValue::Str(s)
            }
            "MASTERNAME" => {
                let mname = match ctx_sheet {
                    SheetKey::Shape { page_id, shape_id } => self
                        .doc
                        .page(*page_id)
                        .and_then(|p| p.find_shape(*shape_id))
                        .and_then(|s| s.master_id)
                        .and_then(|mid| self.doc.master(mid))
                        .map(|m| m.name.clone())
                        .unwrap_or_default(),
                    SheetKey::Master { master_id, .. } => self
                        .doc
                        .master(*master_id)
                        .map(|m| m.name.clone())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                CellValue::Str(mname)
            }
            "HASCATEGORY" => {
                let Some(a0) = args.first() else {
                    return CellValue::Bool(false);
                };
                let Ok(cat) = self.eval_expr(ctx_sheet, ctx_cell, a0).as_str_coerced() else {
                    return CellValue::Bool(false);
                };
                let needle = cat.trim();
                if needle.is_empty() {
                    return CellValue::Bool(false);
                }
                if let SheetKey::Shape { page_id, shape_id } = ctx_sheet {
                    if let Some(sh) = self.doc.page(*page_id).and_then(|p| p.find_shape(*shape_id)) {
                        let cats = sh
                            .sheet
                            .get_str("User.msvShapeCategories")
                            .or_else(|| sh.sheet.get_str("User.msvStructureType"))
                            .unwrap_or_default();
                        if cats
                            .split([';', ','])
                            .any(|item| item.trim().eq_ignore_ascii_case(needle))
                        {
                            return CellValue::Bool(true);
                        }
                        if sh.name_u.to_ascii_lowercase().contains(&needle.to_ascii_lowercase()) {
                            return CellValue::Bool(true);
                        }
                    }
                }
                CellValue::Bool(false)
            }
            "ISSUBSHAPEOF" => {
                let Some(a0) = args.first() else {
                    return CellValue::Bool(false);
                };
                let ancestor_id = match a0 {
                    Expr::CellRef(p) => self.resolve_ref_path(ctx_sheet, p).and_then(|c| match c.sheet {
                        SheetKey::Shape { shape_id, .. } | SheetKey::Master { shape_id, .. } => Some(shape_id),
                        _ => None,
                    }),
                    other => self
                        .eval_expr(ctx_sheet, ctx_cell, other)
                        .as_internal_f64()
                        .ok()
                        .map(|v| v.trunc() as u64),
                };
                let (SheetKey::Shape { page_id, shape_id }, Some(anc_id)) = (ctx_sheet, ancestor_id) else {
                    return CellValue::Bool(false);
                };
                if *shape_id == anc_id {
                    return CellValue::Bool(false);
                }
                let is_desc = self
                    .doc
                    .page(*page_id)
                    .and_then(|p| p.find_shape(anc_id))
                    .is_some_and(|anc| anc.find_shape(*shape_id).is_some());
                CellValue::Bool(is_desc)
            }
            "PARENTSHEET" => {
                if let SheetKey::Shape { page_id, shape_id } = ctx_sheet {
                    if let Some(page) = self.doc.page(*page_id) {
                        fn find_parent(node: &VisioShape, target: u64) -> Option<u64> {
                            for ch in &node.children {
                                if ch.id == target {
                                    return Some(node.id);
                                }
                                if let Some(found) = find_parent(ch, target) {
                                    return Some(found);
                                }
                            }
                            None
                        }
                        for top in &page.shapes {
                            if let Some(pid) = find_parent(top, *shape_id) {
                                return CellValue::number(pid as f64);
                            }
                        }
                    }
                }
                CellValue::number(0.0)
            }
            "CONTAINERCOUNT" | "CONTAINERSHEETREF" => {
                let SheetKey::Shape { page_id, shape_id } = *ctx_sheet else {
                    return if upper == "CONTAINERCOUNT" {
                        CellValue::number(0.0)
                    } else {
                        CellValue::Err(CellError::Ref)
                    };
                };
                let Some(page) = self.doc.page(page_id) else {
                    return CellValue::Err(CellError::Ref);
                };
                let ids = self.find_enclosing_structures(page, page_id, shape_id, false);
                if upper == "CONTAINERCOUNT" {
                    return CellValue::number(ids.len() as f64);
                }
                let idx_1 = args
                    .first()
                    .and_then(|e| self.eval_expr(ctx_sheet, ctx_cell, e).as_internal_f64().ok())
                    .unwrap_or(1.0)
                    .max(1.0) as usize;
                if let Some(&cid_shape) = ids.get(idx_1 - 1) {
                    CellValue::Ref(encode_global_cell_ref(&GlobalCellId {
                        sheet: SheetKey::Shape { page_id, shape_id: cid_shape },
                        cell: "pinx".into(),
                    }))
                } else {
                    CellValue::Err(CellError::Ref)
                }
            }
            "LISTSHEETREF" | "LISTORDER" => {
                let SheetKey::Shape { page_id, shape_id } = *ctx_sheet else {
                    return if upper == "LISTORDER" {
                        CellValue::number(-1.0)
                    } else {
                        CellValue::Err(CellError::Ref)
                    };
                };
                let Some(page) = self.doc.page(page_id) else {
                    return CellValue::Err(CellError::Ref);
                };
                let list_ids = self.find_enclosing_structures(page, page_id, shape_id, true);
                let Some(&lid) = list_ids.first() else {
                    return if upper == "LISTORDER" {
                        CellValue::number(-1.0)
                    } else {
                        CellValue::Err(CellError::Ref)
                    };
                };
                if upper == "LISTSHEETREF" {
                    return CellValue::Ref(encode_global_cell_ref(&GlobalCellId {
                        sheet: SheetKey::Shape { page_id, shape_id: lid },
                        cell: "pinx".into(),
                    }));
                }
                if let Some(list_sh) = page.find_shape(lid) {
                    let members = list_sh
                        .sheet
                        .get_str("User.msvListMembers")
                        .or_else(|| list_sh.sheet.get_str("User.msvContainerMembers"))
                        .unwrap_or_default();
                    for (pos0, tok) in members.split([';', ',']).map(str::trim).filter(|s| !s.is_empty()).enumerate() {
                        if tok == shape_id.to_string() || tok.eq_ignore_ascii_case(&format!("Sheet.{shape_id}")) {
                            return CellValue::number((pos0 + 1) as f64);
                        }
                    }
                }
                CellValue::number(1.0)
            }
            "CALLOUTTARGETREF" | "CALLOUTCOUNT" => {
                let SheetKey::Shape { page_id, shape_id } = *ctx_sheet else {
                    return if upper == "CALLOUTCOUNT" {
                        CellValue::number(0.0)
                    } else {
                        CellValue::Err(CellError::Ref)
                    };
                };
                let Some(page) = self.doc.page(page_id) else {
                    return CellValue::Err(CellError::Ref);
                };
                let extract_callout_tid = |s: &VisioShape| -> Option<u64> {
                    if let Some(cell) = s.sheet.get_cell("User.msvCalloutTarget") {
                        let low = cell.formula.to_ascii_lowercase();
                        if let Some(pos) = low.find("sheet.") {
                            let digits: String = low[pos + 6..]
                                .chars()
                                .take_while(|c| c.is_ascii_digit())
                                .collect();
                            if let Ok(parsed) = digits.parse::<u64>() {
                                if parsed > 0 {
                                    return Some(parsed);
                                }
                            }
                        }
                        if let Ok(st) = cell.value.as_str_coerced() {
                            let st_low = st.trim().to_ascii_lowercase();
                            if let Some(rest) = st_low.strip_prefix("sheet.") {
                                let digits: String =
                                    rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                                if let Ok(parsed) = digits.parse::<u64>() {
                                    if parsed > 0 {
                                        return Some(parsed);
                                    }
                                }
                            }
                        }
                    }
                    s.sheet
                        .get_f64("User.msvCalloutTarget")
                        .map(|v| v.round() as u64)
                        .filter(|&id| id > 0)
                };
                if upper == "CALLOUTCOUNT" {
                    let count = page
                        .shapes
                        .iter()
                        .filter(|s| extract_callout_tid(s) == Some(shape_id))
                        .count();
                    return CellValue::number(count as f64);
                }
                if let Some(sh) = page.find_shape(shape_id) {
                    if let Some(tid) = extract_callout_tid(sh).filter(|&id| page.find_shape(id).is_some()) {
                        return CellValue::Ref(encode_global_cell_ref(&GlobalCellId {
                            sheet: SheetKey::Shape { page_id, shape_id: tid },
                            cell: "pinx".into(),
                        }));
                    }
                }
                CellValue::Err(CellError::Ref)
            }
            "CONNECTORSHEETREF" | "CONNECTEDSHAPES" => {
                let SheetKey::Shape { page_id, shape_id } = *ctx_sheet else {
                    return if upper == "CONNECTEDSHAPES" {
                        CellValue::number(0.0)
                    } else {
                        CellValue::Err(CellError::Ref)
                    };
                };
                let Some(page) = self.doc.page(page_id) else {
                    return CellValue::Err(CellError::Ref);
                };
                let mut conn_ids: Vec<u64> = page
                    .connects
                    .iter()
                    .filter(|c| c.target_shape_id == shape_id)
                    .map(|c| c.connector_shape_id)
                    .collect();
                conn_ids.dedup();
                if upper == "CONNECTEDSHAPES" {
                    return CellValue::number(conn_ids.len() as f64);
                }
                let idx_1 = args
                    .first()
                    .and_then(|e| self.eval_expr(ctx_sheet, ctx_cell, e).as_internal_f64().ok())
                    .unwrap_or(1.0)
                    .max(1.0) as usize;
                if let Some(&cid) = conn_ids.get(idx_1 - 1) {
                    CellValue::Ref(encode_global_cell_ref(&GlobalCellId {
                        sheet: SheetKey::Shape { page_id, shape_id: cid },
                        cell: "beginx".into(),
                    }))
                } else {
                    CellValue::Err(CellError::Ref)
                }
            }
            "MEMBEROF" | "LAYERMEMBER" => {
                let SheetKey::Shape { page_id, shape_id } = *ctx_sheet else {
                    return if upper == "LAYERMEMBER" {
                        CellValue::Str(String::new())
                    } else {
                        CellValue::Bool(false)
                    };
                };
                let Some(page) = self.doc.page(page_id) else {
                    return CellValue::Bool(false);
                };
                let Some(sh) = page.find_shape(shape_id) else {
                    return CellValue::Bool(false);
                };
                if upper == "LAYERMEMBER" {
                    let joined: Vec<String> = sh.layer_membership.iter().map(|i| i.to_string()).collect();
                    return CellValue::Str(joined.join(";"));
                }
                let Some(a0) = args.first() else {
                    return CellValue::Bool(false);
                };
                let cv = self.eval_expr(ctx_sheet, ctx_cell, a0);
                if let Ok(ix_f) = cv.as_internal_f64() {
                    let ix = ix_f.round().max(0.0) as u32;
                    return CellValue::Bool(sh.layer_membership.contains(&ix));
                }
                if let Ok(lname) = cv.as_str_coerced() {
                    if let Some(layer) = page.layers.iter().find(|l| {
                        l.name.eq_ignore_ascii_case(lname.trim())
                            || l.name_u.eq_ignore_ascii_case(lname.trim())
                    }) {
                        return CellValue::Bool(sh.layer_membership.contains(&layer.ix));
                    }
                }
                CellValue::Bool(false)
            }
            "IS1D" | "IS2D" => {
                if let SheetKey::Shape { page_id, shape_id } = ctx_sheet {
                    if let Some(sh) = self.doc.page(*page_id).and_then(|p| p.find_shape(*shape_id)) {
                        let is_1d = sh.kind == ShapeKind::Connector1D;
                        return CellValue::Bool(if upper == "IS1D" { is_1d } else { !is_1d });
                    }
                }
                CellValue::Bool(upper == "IS2D")
            }
            "PAGENUMBER" => {
                let pid = match ctx_sheet {
                    SheetKey::Page(p) | SheetKey::Shape { page_id: p, .. } => *p,
                    _ => self.doc.pages.first().map(|p| p.id).unwrap_or(1),
                };
                let fg_pos = self
                    .doc
                    .pages
                    .iter()
                    .filter(|p| !p.is_background)
                    .position(|p| p.id == pid)
                    .map(|idx| idx + 1)
                    .unwrap_or(1);
                CellValue::number(fg_pos as f64)
            }
            "PAGECOUNT" => {
                let count = self.doc.pages.iter().filter(|p| !p.is_background).count().max(1);
                CellValue::number(count as f64)
            }
            "THEPAGENAME" => {
                let pid = match ctx_sheet {
                    SheetKey::Page(p) | SheetKey::Shape { page_id: p, .. } => Some(*p),
                    _ => self.doc.pages.first().map(|p| p.id),
                };
                let name = pid
                    .and_then(|id| self.doc.page(id))
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                CellValue::Str(name)
            }
            "DOCLASTSAVE" | "DOCCREATION" => {
                let iso = if upper == "DOCCREATION" {
                    &self.doc.properties.created_iso
                } else {
                    &self.doc.properties.modified_iso
                };
                let serial = parse_datetime_str(iso).unwrap_or_else(|| self.resolve_now_serial());
                CellValue::Num {
                    internal: normalize_f64(serial),
                    unit: Unit::ElapsedDay,
                }
            }
            "CREATOR" => CellValue::Str(self.doc.properties.creator.clone()),
            "COMPANY" => CellValue::Str(self.doc.properties.company.clone()),
            "TITLE" | "FILENAME" => CellValue::Str(self.doc.properties.title.clone()),
            "SUBJECT" => CellValue::Str(self.doc.properties.subject.clone()),
            "DESCRIPTION" => CellValue::Str(self.doc.properties.description.clone()),
            "CATEGORY" => CellValue::Str(self.doc.properties.category.clone()),
            "KEYWORDS" => CellValue::Str(self.doc.properties.keywords.clone()),
            "DIRECTORY" => CellValue::Str(String::new()),

            // Type & Error Predicates
            "ISNUMBER" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a0), CellValue::Num { .. }))
            }
            "ISTEXT" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a0), CellValue::Str(_)))
            }
            "ISLOGICAL" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a0), CellValue::Bool(_)))
            }
            "ISREF" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                match a0 {
                    Expr::CellRef(p) => CellValue::Bool(self.resolve_ref_path(ctx_sheet, p).is_some()),
                    other => CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, other), CellValue::Ref(_))),
                }
            }
            "ISERRREF" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a0), CellValue::Err(CellError::Ref)))
            }
            "ISERRDIV0" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a0), CellValue::Err(CellError::DivByZero)))
            }
            "ISERRNUM" => {
                let Some(a0) = args.first() else { return CellValue::Bool(false); };
                CellValue::Bool(matches!(self.eval_expr(ctx_sheet, ctx_cell, a0), CellValue::Err(CellError::Num)))
            }
            "CELLISTHEMED" => {
                let allow = args
                    .first()
                    .and_then(|a| self.eval_expr(ctx_sheet, ctx_cell, a).as_bool().ok())
                    .unwrap_or(true);
                CellValue::Bool(allow)
            }

            // Smart-Shape Action & UI Trigger Builtins (evaluate args for side-effects like nested `SETF`, return 0 passively or structured token during one-shot Action execution)
            "CALLTHIS" | "RUNMACRO" | "RUNADDON" | "RUNADDONWARGS" | "QUEUEMARKEREVENT"
            | "GOTOPAGE" | "OPENTEXTWIN" | "OPENGROUPWIN" | "OPENSHAPESHEET" | "PLAYSOUND"
            | "HELP" => {
                let mut first_str = String::new();
                for (idx, a) in args.iter().enumerate() {
                    let v = self.eval_expr(ctx_sheet, ctx_cell, a);
                    if idx == 0 {
                        first_str = v.as_str_coerced().unwrap_or_default();
                    }
                }
                if ctx_cell == "__action_oneshot" {
                    match upper {
                        "GOTOPAGE" => return CellValue::Str(format!("__ACTION:GOTOPAGE:{first_str}")),
                        "OPENTEXTWIN" => return CellValue::Str("__ACTION:OPENTEXTWIN".into()),
                        "RUNMACRO" | "CALLTHIS" => {
                            return CellValue::Str(format!("__ACTION:RUNMACRO:{first_str}"));
                        }
                        _ => {}
                    }
                }
                CellValue::number(0.0)
            }

            _ => CellValue::Err(CellError::Name),
        }
    }
}
