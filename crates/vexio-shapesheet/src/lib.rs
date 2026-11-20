//! `vexio-shapesheet` — 1:1 Microsoft Visio ShapeSheet Reactive Formula Engine & Document IR.

pub mod dag;
pub mod model;
pub mod parser;
pub mod sections;
pub mod units;

pub use dag::{
    EvaluatedGeometryPath, EvaluatedPathVerb, EvaluatedShapeVisual, GlobalCellId, PendingSetf,
    ReactiveEngine, ShapeTransform2D, SheetKey,
};
pub use model::{
    CommentAnnotation, DocumentProperties, ForeignMediaData, GlueEndpoint, HeaderFooter, LayerDef,
    MarkupInkPoint, MarkupInkStroke, OpaqueOpcBag, ShapeKind, StencilPack, StyleSheetDef, ThemeDef,
    VisioDocument, VisioMaster, VisioPage, VisioShape,
};
pub use parser::{
    BinaryOp, CellRefPath, Expr, FormulaParseError, SheetTarget, UnaryOp, canonical_cell_name,
    parse_formula,
};
pub use sections::{
    ActionRow, Cell, ConnectionPointRow, ControlRow, GeometryRow, GeometryRowKind, GeometrySection,
    HyperlinkRow, ShapeDataRow, ShapeDataType, ShapeSheet, TextFieldRow, UserCellRow,
};
pub use units::{CellError, CellValue, RgbaColor, Unit, format_clean_f64, parse_scalar_with_unit};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_conversions_match_visio_internal_standards() {
        let mm = CellValue::with_unit(25.4, Unit::Millimeter);
        assert!((mm.as_internal_f64().unwrap() - 1.0).abs() < 1e-12);

        let pt = CellValue::with_unit(72.0, Unit::Point);
        assert!((pt.as_internal_f64().unwrap() - 1.0).abs() < 1e-12);

        let deg = CellValue::with_unit(180.0, Unit::Degree);
        assert!((deg.as_internal_f64().unwrap() - std::f64::consts::PI).abs() < 1e-12);

        let pct = CellValue::with_unit(25.0, Unit::Percent);
        assert!((pct.as_internal_f64().unwrap() - 0.25).abs() < 1e-12);
    }

    #[test]
    fn guard_blocks_interactive_writes_and_setf_overrides() {
        let mut doc = VisioDocument::new_blank("GuardTest");
        let page = doc.page_mut(1).unwrap();
        let mut s = VisioShape::new_rect(1, "Box", 2.0, 3.0, 4.0, 2.0);
        s.sheet.set_formula("Width", "=GUARD(4 in)", Some(Unit::Inch));
        s.sheet.set_formula("Height", "=Width*0.5", Some(Unit::Inch));
        page.shapes.push(s);

        ReactiveEngine::recalculate_document(&mut doc);
        let shape = doc.page(1).unwrap().find_shape(1).unwrap();
        assert!((shape.sheet.get_f64("Width").unwrap() - 4.0).abs() < 1e-9);
        assert!((shape.sheet.get_f64("Height").unwrap() - 2.0).abs() < 1e-9);

        // Interactive UI drag (`force_setf = false`) MUST be blocked by GUARD!
        let err = doc
            .page_mut(1)
            .unwrap()
            .find_shape_mut(1)
            .unwrap()
            .sheet
            .try_set_formula_respecting_guard("Width", "10 in", false);
        assert_eq!(err, Err(CellError::Ref));

        // Reactive `SETF(GetRef(Width), "8 in")` inside an Action/User trigger MUST override GUARD!
        doc.page_mut(1)
            .unwrap()
            .find_shape_mut(1)
            .unwrap()
            .sheet
            .set_user_cell("TriggerResize", "=SETF(GetRef(Width), \"8 in\")", "Override");
        ReactiveEngine::recalculate_document(&mut doc);

        let shape = doc.page(1).unwrap().find_shape(1).unwrap();
        assert!((shape.sheet.get_f64("Width").unwrap() - 8.0).abs() < 1e-9);
        assert!((shape.sheet.get_f64("Height").unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn cross_shape_cross_page_par_pnt_intersect_and_gravity_formulas() {
        let mut doc = VisioDocument::new_blank("CrossRefTest");
        doc.pages.push(VisioPage::new(2, "MetricsPage", 11.0, 8.5));

        // Shape 1 on Page-1 with a Control handle at local (3 in, 2 in), Pin at (5, 5), Width=2, Height=2 (LocPin = 1, 1)
        let mut s1 = VisioShape::new_rect(1, "SourceNode", 5.0, 5.0, 2.0, 2.0);
        s1.sheet.add_control(ControlRow {
            name: "Row_1".into(),
            x: "3 in".into(),
            y: "2 in".into(),
            x_dyn: "3 in".into(),
            y_dyn: "2 in".into(),
            x_con: "0".into(),
            y_con: "0".into(),
            can_glue: true,
            prompt: "Handle".into(),
        });
        doc.page_mut(1).unwrap().shapes.push(s1);

        // Shape 2 on Page-1 tracks Shape 1's control handle via PAR(PNT(...)) + tests INTERSECTX/Y + GRAVITY
        let mut s2 = VisioShape::new_rect(2, "Follower", 1.0, 1.0, 2.0, 1.0);
        s2.sheet.set_formula(
            "PinX",
            "=PAR(PNT(Sheet.1!Controls.Row_1.X, Sheet.1!Controls.Row_1.Y))",
            Some(Unit::Inch),
        );
        s2.sheet.set_formula(
            "PinY",
            "=PAR(PNT(Sheet.1!Controls.Row_1.X, Sheet.1!Controls.Row_1.Y))",
            Some(Unit::Inch),
        );
        s2.sheet.set_user_cell(
            "CrossX",
            "=INTERSECTX(0 in, 0 in, 45 deg, 0 in, 4 in, 0 deg)",
            "Line intersection X",
        );
        s2.sheet.set_formula("Angle", "170 deg", Some(Unit::Degree));
        s2.sheet.set_formula("TxtAngle", "=GRAVITY(Angle, 0 deg, 180 deg)", Some(Unit::Degree));
        doc.page_mut(1).unwrap().shapes.push(s2);

        // Shape on Page 2 references `Pages[Page-1]!Sheet.2!PinX` and string/lookup formulas
        let mut s_p2 = VisioShape::new_rect(10, "SummaryCard", 2.0, 2.0, 3.0, 2.0);
        s_p2.sheet.set_user_cell("RemotePinX", "=Pages[Page-1]!Sheet.2!PinX", "Cross-page ref");
        s_p2.sheet.set_user_cell(
            "StatusLabel",
            "=INDEX(LOOKUP(\"Active\", \"Draft;Active;Archived\"), \"Red;Green;Gray\")",
            "Lookup+Index",
        );
        doc.page_mut(2).unwrap().shapes.push(s_p2);

        ReactiveEngine::recalculate_document(&mut doc);

        // Local (3,2) with LocPin (1,1) and Pin (5,5) -> Page (5 + (3-1), 5 + (2-1)) = (7, 6)!
        let follower = doc.page(1).unwrap().find_shape(2).unwrap();
        assert!((follower.sheet.get_f64("PinX").unwrap() - 7.0).abs() < 1e-9);
        assert!((follower.sheet.get_f64("PinY").unwrap() - 6.0).abs() < 1e-9);
        assert!((follower.sheet.get_f64("User.CrossX").unwrap() - 4.0).abs() < 1e-9);
        assert!((follower.sheet.get_f64("TxtAngle").unwrap() - std::f64::consts::PI).abs() < 1e-9);

        let card = doc.page(2).unwrap().find_shape(10).unwrap();
        assert!((card.sheet.get_f64("User.RemotePinX").unwrap() - 7.0).abs() < 1e-9);
        assert_eq!(
            card.sheet.get_cell("User.StatusLabel").unwrap().value,
            CellValue::Str("Green".into())
        );
    }

    #[test]
    fn cyclic_formulas_produce_ref_error_without_hanging() {
        let mut doc = VisioDocument::new_blank("CycleTest");
        let mut s = VisioShape::new_rect(1, "CycleBox", 1.0, 1.0, 2.0, 2.0);
        s.sheet.set_formula("Width", "=Height + 1 in", Some(Unit::Inch));
        s.sheet.set_formula("Height", "=Width + 1 in", Some(Unit::Inch));
        doc.page_mut(1).unwrap().shapes.push(s);

        ReactiveEngine::recalculate_document(&mut doc);
        let sh = doc.page(1).unwrap().find_shape(1).unwrap();
        assert_eq!(sh.sheet.get_cell("Width").unwrap().value, CellValue::Err(CellError::Ref));
    }

    #[test]
    fn master_inheritance_evaluates_in_instance_context() {
        let mut doc = VisioDocument::new_blank("MasterInheritance");
        let mut master_shape = VisioShape::new_rect(1, "Server", 0.0, 0.0, 2.0, 1.0);
        master_shape.sheet.set_user_cell("Aspect", "=Width / Height", "Aspect ratio");
        doc.masters.push(VisioMaster {
            id: 1,
            name: "RackServer".into(),
            name_u: "RackServer".into(),
            base_id: "{0001}".into(),
            unique_id: "{0002}".into(),
            prompt: "1U Rack Server".into(),
            icon_bytes: None,
            page_sheet: ShapeSheet::new(),
            shapes: vec![master_shape],
        });

        let inst_id = doc.drop_master(1, 1, 4.0, 4.0).unwrap();
        // Resize instance Width to 6 in while inheriting `User.Aspect = Width / Height` from Master!
        doc.page_mut(1)
            .unwrap()
            .find_shape_mut(inst_id)
            .unwrap()
            .sheet
            .set_formula("Width", "6 in", Some(Unit::Inch));
        // Remove local User.Aspect so it strictly inherits from Master:
        doc.page_mut(1)
            .unwrap()
            .find_shape_mut(inst_id)
            .unwrap()
            .sheet
            .cells
            .remove("user.aspect");

        ReactiveEngine::recalculate_document(&mut doc);
        let inst = doc.page(1).unwrap().find_shape(inst_id).unwrap();
        assert!((inst.sheet.get_f64("User.Aspect").unwrap() - 6.0).abs() < 1e-9);
    }
}
