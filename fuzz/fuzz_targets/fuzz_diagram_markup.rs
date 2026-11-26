#![no_main]

use libfuzzer_sys::fuzz_target;
use vexio_core::diagram_text_interop::{
    DiagramMarkupFormat, export_diagram_markup, import_diagram_markup,
};
use vexio_shapesheet::VisioDocument;

fuzz_target!(|data: &[u8]| {
    if data.len() > 32 * 1024 {
        return;
    }
    let Ok(markup) = std::str::from_utf8(data) else {
        return;
    };
    for fmt in [
        DiagramMarkupFormat::Auto,
        DiagramMarkupFormat::Mermaid,
        DiagramMarkupFormat::PlantUml,
        DiagramMarkupFormat::GraphvizDot,
    ] {
        let mut doc = VisioDocument::new_blank("FuzzMarkup");
        if import_diagram_markup(&mut doc, 1, markup, fmt).is_ok() {
            let _ = export_diagram_markup(&doc, 1, fmt);
        }
    }
});
