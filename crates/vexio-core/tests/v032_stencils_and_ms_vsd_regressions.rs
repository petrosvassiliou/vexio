use std::io::{Cursor, Write};
use vexio_core::domains::{list_builtin_stencil_previews, list_official_stencil_catalog};
use vexio_core::export_print::export_page_png;
use vexio_core::DocumentSession;

#[test]
fn builtin_m365_ad_azure_and_rack_stencils_produce_retina_png_previews_and_below_icon_captions() {
    let previews = list_builtin_stencil_previews();

    let required_checks = [
        ("Microsoft 365 & Office", "Exchange Online"),
        ("Microsoft 365 & Office", "Exchange Online Protection (EOP)"),
        ("Microsoft 365 & Office", "Microsoft Purview"),
        ("Microsoft 365 & Office", "Microsoft Teams"),
        ("Microsoft 365 & Office", "SharePoint Online"),
        ("Microsoft Servers & Active Directory", "Active Directory DC"),
        ("Microsoft Servers & Active Directory", "Exchange Server"),
        ("Microsoft Servers & Active Directory", "Entra Connect Sync"),
        ("Microsoft Azure & Hybrid Cloud", "Azure Sentinel"),
        ("Network & Rack Equipment", "Cisco Catalyst 48-Port Switch (1U)"),
        ("Network & Rack Equipment", "Panduit 24-Port Patch Panel (1U)"),
        ("Network & Rack Equipment", "APC Smart-UPS Rackmount (2U)"),
    ];

    let mut sess = DocumentSession::new_blank("StencilParityCheck");
    for (idx, (cat, master)) in required_checks.iter().enumerate() {
        let cat_list = previews
            .get(*cat)
            .unwrap_or_else(|| panic!("missing builtin category {cat}"));
        let item = cat_list
            .iter()
            .find(|m| m.name == *master)
            .unwrap_or_else(|| panic!("missing builtin master {cat}::{master}"));
        assert!(
            item.png_data_url.starts_with("data:image/png;base64,"),
            "{cat}::{master} must carry a Retina transparent PNG data URL"
        );
        assert!(
            item.png_data_url.len() > 180,
            "{cat}::{master} PNG data URL unexpectedly tiny"
        );

        let sid = sess
            .drop_builtin_shape(1, cat, master, 1.5 + (idx as f64) * 0.7, 4.5)
            .expect("drop_builtin_shape must succeed");
        let page = sess.doc.page(1).unwrap();
        let shape = page.shapes.iter().find(|s| s.id == sid).unwrap();
        assert!(
            shape.foreign_data.is_some(),
            "{cat}::{master} should carry crisp SVG vector artwork"
        );
        let txt_pin_y = shape.sheet.get_f64("TxtPinY").unwrap_or(1.0);
        assert!(
            txt_pin_y < 0.0,
            "{cat}::{master} caption TxtPinY ({txt_pin_y}) should sit below the icon box"
        );
    }

    let png = export_page_png(&sess.doc, 1, 96);
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(png.len() > 2_000);
}

#[test]
fn multi_pack_svg_zip_archive_imports_clean_categories_dedupes_monochrome_and_renders_png_previews() {
    let mut zip_bytes = Vec::new();
    {
        let cursor = Cursor::new(&mut zip_bytes);
        let mut zip = zip::ZipWriter::new(cursor);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file(
            "Azure_Public_Service_Icons/Icons/networking/10061-icon-service-Virtual-Networks.svg",
            opts,
        )
        .unwrap();
        zip.write_all(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 18 18"><circle cx="9" cy="9" r="7" fill="#0078D4"/><circle cx="9" cy="9" r="3" fill="#50E6FF"/></svg>"##,
        )
        .unwrap();

        // Both Dark Blue monochrome and Grey & Blue two-tone folder variants (as in 2024-microsoft-365-content-icons.zip)
        zip.start_file("icons/Dark Blue Icon/Exchange.svg", opts).unwrap();
        zip.write_all(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><rect x="6" y="6" width="36" height="36" fill="#003366"/></svg>"##,
        )
        .unwrap();

        zip.start_file("icons/Grey & Blue Icon/Exchange.svg", opts).unwrap();
        zip.write_all(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><rect x="6" y="6" width="36" height="36" rx="6" fill="#0078D4"/></svg>"##,
        )
        .unwrap();

        zip.finish().unwrap();
    }

    let mut sess = DocumentSession::new_blank("ZipArchiveTest");
    let packs = sess
        .import_stencil_archive_packs("Azure_Public_Service_Icons_V25.zip", &zip_bytes)
        .expect("ZIP SVG stencil archive must import cleanly");

    assert!(packs.len() >= 2, "expected separate category packs from folder hierarchy");
    let net_pack = packs
        .iter()
        .find(|(title, _)| title.to_ascii_lowercase().contains("networking"))
        .expect("networking subfolder pack must exist");
    assert_eq!(net_pack.1.len(), 1);
    assert_eq!(net_pack.1[0].name, "Virtual Networks");
    assert!(net_pack.1[0]
        .png_data_url
        .starts_with("data:image/png;base64,"));

    let icons_pack = packs
        .iter()
        .find(|(_, items)| items.iter().any(|m| m.name == "Exchange"))
        .expect("deduplicated Exchange master must exist");
    assert_eq!(
        icons_pack
            .1
            .iter()
            .filter(|m| m.name.contains("Exchange"))
            .count(),
        1,
        "Black_ monochrome duplicate must be filtered when Grey & Blue two-tone icon exists"
    );

    let dropped_id = sess
        .drop_builtin_shape(1, &net_pack.0, "Virtual Networks", 3.0, 4.0)
        .expect("dropping imported ZIP SVG master must succeed");
    let sh = sess
        .doc
        .page(1)
        .unwrap()
        .shapes
        .iter()
        .find(|s| s.id == dropped_id)
        .unwrap();
    assert!(sh.sheet.get_f64("TxtPinY").unwrap_or(1.0) < 0.0);
}

#[test]
fn official_stencil_catalog_exposes_verified_https_entries() {
    let catalog = list_official_stencil_catalog();
    assert!(catalog.len() >= 10);
    for entry in &catalog {
        assert!(!entry.id.is_empty());
        assert!(!entry.title.is_empty());
        assert!(!entry.category_group.is_empty());
        assert!(!entry.description.is_empty());
        assert!(
            entry.url.starts_with("https://"),
            "catalog entry {} must use HTTPS: {}",
            entry.id,
            entry.url
        );
        assert!(!entry.file_name.is_empty());
        assert!(entry.approx_size_kb > 0);
    }
}
