#![no_main]

use libfuzzer_sys::fuzz_target;
use vexio_shapesheet::{ReactiveEngine, Unit, VisioDocument, VisioShape};

fuzz_target!(|data: &[u8]| {
    if data.len() > 8192 {
        return;
    }
    let Ok(formula) = std::str::from_utf8(data) else {
        return;
    };
    let mut doc = VisioDocument::new_blank("FuzzFormula");
    if let Some(page) = doc.page_mut(1) {
        let mut sh = VisioShape::new_rect(1, "Fuzz.1", 2.0, 2.0, 1.5, 1.0);
        sh.sheet
            .set_formula("User.FuzzCell", format!("={formula}"), Some(Unit::Number));
        page.shapes.push(sh);
    }
    ReactiveEngine::recalculate_document(&mut doc);
    let _ = ReactiveEngine::evaluate_page_visuals(&doc, 1);
});
