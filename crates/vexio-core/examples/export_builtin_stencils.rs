//! Generates Vexio's built-in IP-clean OpenXML `.vssx` Stencil Packs onto disk
//! for native platforms (`/home/antigravity/stencils/`) and ChromeOS PWA offline precaching
//! (`/home/antigravity/apps/chromeos-pwa/public/stencils/`).

use std::fs;
use std::path::Path;
use vexio_core::domains::build_all_builtin_stencils;
use vexio_shapesheet::VisioDocument;
use vexio_vsdx::{VsdxPackageKind, write_vsdx};

fn slugify(title: &str) -> String {
    title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn main() {
    let dirs = [
        Path::new("/home/antigravity/stencils"),
        Path::new("/home/antigravity/apps/chromeos-pwa/public/stencils"),
    ];
    for d in dirs {
        fs::create_dir_all(d).expect("create stencil output dir");
    }

    for pack in build_all_builtin_stencils() {
        let mut doc = VisioDocument::new_blank(&pack.title);
        doc.properties.category = pack.category.clone();
        doc.pages.clear();
        doc.masters = pack.masters;
        let bytes = write_vsdx(&doc, VsdxPackageKind::Stencil).expect("encode .vssx stencil");
        let file_name = format!("{}.vssx", slugify(&pack.title));
        for d in dirs {
            fs::write(d.join(&file_name), &bytes).expect("write .vssx file");
        }
        println!("Exported {} ({} bytes)", file_name, bytes.len());
    }
}
