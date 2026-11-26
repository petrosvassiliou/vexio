#![no_main]

use libfuzzer_sys::fuzz_target;
use vexio_cad::{import_dwg_to_page, import_dxf_to_page};
use vexio_shapesheet::VisioDocument;

fuzz_target!(|data: &[u8]| {
    if data.len() > 2 * 1024 * 1024 {
        return;
    }
    let mut doc_dwg = VisioDocument::new_blank("FuzzDwg");
    if let Some(page) = doc_dwg.page_mut(1) {
        let _ = import_dwg_to_page(page, data);
    }
    let mut doc_dxf = VisioDocument::new_blank("FuzzDxf");
    if let Some(page) = doc_dxf.page_mut(1) {
        let _ = import_dxf_to_page(page, data);
    }
});
