//! Containers, Ordered Lists, Callouts & Cross-Functional Swimlane Systems.

use serde::{Deserialize, Serialize};
use vexio_shapesheet::{
    ReactiveEngine, RgbaColor, ShapeDataRow, ShapeDataType, Unit, VisioDocument, VisioShape,
    format_clean_f64,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerResizeMode {
    FitToContents,
    ExpandOnly,
    Locked,
}

/// Create a Visio Container shape enclosing `member_ids` with header band and live `User.msvStructureType = "Container"`.
pub fn create_container(
    doc: &mut VisioDocument,
    page_id: u32,
    title: &str,
    member_ids: &[u64],
    padding_in: f64,
) -> Option<u64> {
    let page = doc.page_mut(page_id)?;
    let next_id = page.next_shape_id();
    let mut container = VisioShape::new_rect(next_id, format!("Container.{next_id}"), 4.0, 4.0, 4.0, 3.0);
    container.text = title.into();
    container
        .sheet
        .set_user_cell("msvStructureType", "\"Container\"", "Structure Type");
    let members_str = member_ids
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(";");
    container
        .sheet
        .set_user_cell("msvContainerMembers", format!("\"{members_str}\""), "Member IDs");
    container
        .sheet
        .set_formula("FillForegnd", "=RGB(248,250,252)", Some(Unit::Color));
    container
        .sheet
        .set_formula("LineColor", "=RGB(100,116,139)", Some(Unit::Color));
    // Insert behind members (at index 0)
    page.shapes.insert(0, container);
    refit_container(doc, page_id, next_id, padding_in, ContainerResizeMode::FitToContents);
    Some(next_id)
}

pub fn refit_container(
    doc: &mut VisioDocument,
    page_id: u32,
    container_id: u64,
    padding_in: f64,
    mode: ContainerResizeMode,
) {
    if mode == ContainerResizeMode::Locked {
        return;
    }
    ReactiveEngine::recalculate_document(doc);
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    let member_ids: Vec<u64> = page
        .find_shape(container_id)
        .and_then(|c| c.sheet.get_cell("User.msvContainerMembers"))
        .and_then(|cell| cell.value.as_str_coerced().ok())
        .map(|s| {
            s.trim_matches('"')
                .split(';')
                .filter_map(|tok| tok.trim().parse::<u64>().ok())
                .collect()
        })
        .unwrap_or_default();

    if member_ids.is_empty() {
        return;
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for mid in member_ids {
        if let Some(m) = page.find_shape(mid) {
            let px = m.sheet.get_f64("PinX").unwrap_or(0.0);
            let py = m.sheet.get_f64("PinY").unwrap_or(0.0);
            let w = m.sheet.get_f64("Width").unwrap_or(1.0);
            let h = m.sheet.get_f64("Height").unwrap_or(1.0);
            min_x = min_x.min(px - w * 0.5);
            min_y = min_y.min(py - h * 0.5);
            max_x = max_x.max(px + w * 0.5);
            max_y = max_y.max(py + h * 0.5);
        }
    }
    if !min_x.is_finite() {
        return;
    }

    let header_extra = 0.4;
    let target_w = (max_x - min_x) + padding_in * 2.0;
    let target_h = (max_y - min_y) + padding_in * 2.0 + header_extra;
    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5 + header_extra * 0.5;

    if let Some(cont) = page.find_shape_mut(container_id) {
        let cur_w = cont.sheet.get_f64("Width").unwrap_or(0.0);
        let cur_h = cont.sheet.get_f64("Height").unwrap_or(0.0);
        let final_w = if mode == ContainerResizeMode::ExpandOnly {
            target_w.max(cur_w)
        } else {
            target_w
        };
        let final_h = if mode == ContainerResizeMode::ExpandOnly {
            target_h.max(cur_h)
        } else {
            target_h
        };
        cont.sheet.set_formula(
            "PinX",
            format!("{} in", format_clean_f64(cx)),
            Some(Unit::Inch),
        );
        cont.sheet.set_formula(
            "PinY",
            format!("{} in", format_clean_f64(cy)),
            Some(Unit::Inch),
        );
        cont.sheet.set_formula(
            "Width",
            format!("{} in", format_clean_f64(final_w)),
            Some(Unit::Inch),
        );
        cont.sheet.set_formula(
            "Height",
            format!("{} in", format_clean_f64(final_h)),
            Some(Unit::Inch),
        );
    }
    ReactiveEngine::recalculate_document(doc);
}

/// Attach a Visio Callout annotation tethered reactively to `target_shape_id` via `Controls.Row_1`.
pub fn attach_callout(
    doc: &mut VisioDocument,
    page_id: u32,
    target_shape_id: u64,
    text: &str,
    offset_dx_in: f64,
    offset_dy_in: f64,
) -> Option<u64> {
    let page = doc.page_mut(page_id)?;
    let target = page.find_shape(target_shape_id)?;
    let tx = target.sheet.get_f64("PinX").unwrap_or(2.0);
    let ty = target.sheet.get_f64("PinY").unwrap_or(2.0);
    let cid = page.next_shape_id();

    let mut callout = VisioShape::new_rect(
        cid,
        format!("Callout.{cid}"),
        tx + offset_dx_in,
        ty + offset_dy_in,
        1.8,
        0.75,
    );
    callout.text = text.into();
    callout
        .sheet
        .set_user_cell("msvStructureType", "\"Callout\"", "Structure Type");
    callout.sheet.set_user_cell(
        "msvCalloutTargetSheet",
        format!("{target_shape_id}"),
        "Target Shape ID",
    );
    callout
        .sheet
        .set_formula("FillForegnd", "=RGB(254,249,195)", Some(Unit::Color));
    callout.sheet.add_control(vexio_shapesheet::ControlRow {
        name: "Row_1".into(),
        x: format!("=Sheet.{target_shape_id}!PinX - PinX + LocPinX"),
        y: format!("=Sheet.{target_shape_id}!PinY - PinY + LocPinY"),
        x_dyn: "0 in".into(),
        y_dyn: "0 in".into(),
        x_con: "0".into(),
        y_con: "0".into(),
        can_glue: true,
        prompt: "Callout Leader Anchor".into(),
    });
    page.shapes.push(callout);
    ReactiveEngine::recalculate_document(doc);
    Some(cid)
}

/// Build a Cross-Functional Swimlane system (horizontal bands) and assign shape `Prop.Function` automatically.
pub fn build_cross_functional_swimlanes(
    doc: &mut VisioDocument,
    page_id: u32,
    lane_titles: &[&str],
    origin_x: f64,
    top_y: f64,
    lane_width: f64,
    lane_height: f64,
) -> Vec<u64> {
    let mut lane_ids = Vec::new();
    {
        let Some(page) = doc.page_mut(page_id) else {
            return lane_ids;
        };
        for (i, &title) in lane_titles.iter().enumerate() {
            let id = page.next_shape_id();
            let cy = top_y - (i as f64 + 0.5) * lane_height;
            let cx = origin_x + lane_width * 0.5;
            let mut lane = VisioShape::new_rect(id, format!("Swimlane.{id}"), cx, cy, lane_width, lane_height);
            lane.text = title.into();
            lane.sheet
                .set_user_cell("msvStructureType", "\"Container\"", "Swimlane Container");
            lane.sheet
                .set_user_cell("visSwimlane", "1", "Cross-Functional Swimlane");
            let bg = if i % 2 == 0 {
                RgbaColor { r: 248, g: 250, b: 252, a: 255 }
            } else {
                RgbaColor { r: 241, g: 245, b: 249, a: 255 }
            };
            lane.sheet.set_formula(
                "FillForegnd",
                format!("=RGB({},{},{})", bg.r, bg.g, bg.b),
                Some(Unit::Color),
            );
            page.shapes.insert(i, lane);
            lane_ids.push(id);
        }
    }
    sync_swimlane_shape_metadata(doc, page_id);
    lane_ids
}

/// Inspect all non-swimlane 2D shapes on `page_id` and populate `Prop.Function` based on enclosing swimlane band.
pub fn sync_swimlane_shape_metadata(doc: &mut VisioDocument, page_id: u32) {
    ReactiveEngine::recalculate_document(doc);
    let Some(page) = doc.page_mut(page_id) else {
        return;
    };
    let lanes: Vec<(String, f64, f64, f64, f64)> = page
        .shapes
        .iter()
        .filter(|s| s.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5)
        .map(|s| {
            let px = s.sheet.get_f64("PinX").unwrap_or(0.0);
            let py = s.sheet.get_f64("PinY").unwrap_or(0.0);
            let w = s.sheet.get_f64("Width").unwrap_or(1.0);
            let h = s.sheet.get_f64("Height").unwrap_or(1.0);
            (
                s.text.clone(),
                px - w * 0.5,
                py - h * 0.5,
                px + w * 0.5,
                py + h * 0.5,
            )
        })
        .collect();

    for sh in &mut page.shapes {
        if sh.kind != vexio_shapesheet::ShapeKind::Shape2D
            || sh.sheet.get_f64("User.visSwimlane").unwrap_or(0.0) > 0.5
        {
            continue;
        }
        let px = sh.sheet.get_f64("PinX").unwrap_or(0.0);
        let py = sh.sheet.get_f64("PinY").unwrap_or(0.0);
        if let Some((lane_name, _, _, _, _)) = lanes
            .iter()
            .find(|(_, x0, y0, x1, y1)| px >= *x0 && px <= *x1 && py >= *y0 && py <= *y1)
        {
            sh.sheet.set_shape_data(ShapeDataRow {
                name: "Function".into(),
                label: "Function".into(),
                prompt: "Assigned Swimlane Function".into(),
                data_type: ShapeDataType::String,
                format: String::new(),
                value: format!("\"{}\"", lane_name.replace('"', "\"\"")),
                sort_key: "1".into(),
                invisible: false,
                ask: false,
                lang_id: 1033,
            });
        }
    }
    ReactiveEngine::recalculate_document(doc);
}
