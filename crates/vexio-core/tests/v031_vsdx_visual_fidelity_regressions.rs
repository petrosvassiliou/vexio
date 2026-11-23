//! Regression suite for real-world enterprise `.vsdx` import visual fidelity:
//! 1. `TextBkgnd = 0` (`None` / transparent) vs 1-based palette `TextBkgnd = 2` (`#FFFFFF` White)
//! 2. `TxtLocPinX` / `TxtLocPinY` external below-icon captions + `StyleSheet`/`QuickStyle` font contrast
//! 3. Rotated/stretched `EllipticalArcTo` & `RelEllipticalArcTo` without circumcircle ballooning
//! 4. `NURBSTo` Cox-de Boor rational B-splines & `PolylineTo` multi-vertex evaluation
//! 5. Embedded `SVG` `<linearGradient>` `url(#id)`, `<defs>`/`<clipPath>` skipping, `matrix(...)` transforms, and compact `10`/`01` arc flags

use std::io::{Cursor, Write};
use vexio_render::tessellate_shape_full;
use vexio_shapesheet::{EvaluatedPathVerb, ReactiveEngine, RgbaColor};
use vexio_vsdx::read_vsdx;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn build_synthetic_enterprise_vsdx() -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default();

        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="svg" ContentType="image/svg+xml"/>
  <Override PartName="/visio/document.xml" ContentType="application/vnd.ms-visio.drawing.main+xml"/>
  <Override PartName="/visio/pages/pages.xml" ContentType="application/vnd.ms-visio.pages+xml"/>
  <Override PartName="/visio/pages/page1.xml" ContentType="application/vnd.ms-visio.page+xml"/>
</Types>"##,
        )
        .unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/_rels/document.xml.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/document.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<VisioDocument xmlns="http://schemas.microsoft.com/office/visio/2012/main">
  <Colors>
    <ColorEntry IX="0" RGB="#000000"/>
    <ColorEntry IX="1" RGB="#FFFFFF"/>
    <ColorEntry IX="2" RGB="#FF0000"/>
  </Colors>
  <StyleSheets>
    <StyleSheet ID="0" Name="Normal" NameU="Normal">
      <Cell N="TextBkgnd" V="0"/>
      <Cell N="TextBkgndTrans" V="0"/>
      <Section N="Character">
        <Row IX="0">
          <Cell N="Color" V="#ffffff" F="THEMEVAL()"/>
        </Row>
      </Section>
    </StyleSheet>
  </StyleSheets>
</VisioDocument>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/pages.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Pages xmlns="http://schemas.microsoft.com/office/visio/2012/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <Page ID="1" Name="Hybrid Architecture" NameU="Hybrid Architecture">
    <PageSheet>
      <Cell N="PageWidth" V="11" U="MM"/>
      <Cell N="PageHeight" V="8.5" U="MM"/>
      <Cell N="PageColor" V="#ffffff"/>
    </PageSheet>
    <Rel r:id="rId1"/>
  </Page>
</Pages>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/_rels/pages.xml.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/_rels/page1.xml.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdSvg1" Type="http://schemas.microsoft.com/visio/2010/relationships/image" Target="../media/entra_badge.svg"/>
</Relationships>"##,
        )
        .unwrap();

        // Embedded Azure-style SVG with `<defs>`, `<linearGradient>`, `<clipPath>` (must NOT draw!),
        // `<style>` CSS classes, `matrix(0.05,...)` scaling, and minified `a8 8 0 10-16 0` arc flags.
        zip.start_file("visio/media/entra_badge.svg", opts).unwrap();
        zip.write_all(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 18 18" width="100%" height="100%">
  <defs>
    <clipPath id="clip0"><rect x="0" y="0" width="500" height="500"/></clipPath>
    <linearGradient id="azGrad" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#5ea0ef"/>
      <stop offset="100%" stop-color="#0078d4"/>
    </linearGradient>
    <style>.ring { fill: none; stroke: #32bedd; stroke-width: 1.5px; }</style>
  </defs>
  <g transform="translate(1, 1) matrix(0.05, 0, 0, 0.05, 0, 0)">
    <polygon points="160,10 310,280 10,280" fill="url(#azGrad)"/>
  </g>
  <path class="ring" d="M9 1a8 8 0 100 16A8 8 0 009 1z"/>
</svg>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/page1.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<PageContents xmlns="http://schemas.microsoft.com/office/visio/2012/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <Shapes>
    <!-- Shape 1: Azure icon with external below-icon caption (TxtPinY=0, TxtLocPinY=TxtHeight) and TextBkgnd=0 -->
    <Shape ID="1" NameU="ActiveDirectory" Type="Foreign" FillStyle="0" LineStyle="0" TextStyle="0">
      <Cell N="PinX" V="2.0" U="MM"/>
      <Cell N="PinY" V="6.0" U="MM"/>
      <Cell N="Width" V="0.6" U="MM"/>
      <Cell N="Height" V="0.6" U="MM"/>
      <Cell N="TxtPinX" V="0.3" F="Width*0.5"/>
      <Cell N="TxtPinY" V="0" U="MM"/>
      <Cell N="TxtWidth" V="1.4" U="MM"/>
      <Cell N="TxtHeight" V="0.3" U="MM"/>
      <Cell N="TxtLocPinX" V="0.7" F="TxtWidth*0.5"/>
      <Cell N="TxtLocPinY" V="0.3" F="TxtHeight"/>
      <Cell N="TextBkgnd" V="0"/>
      <Cell N="TextBkgndTrans" V="0"/>
      <Cell N="QuickStyleFillMatrix" V="100"/>
      <ForeignData ForeignType="SVG">
        <Rel r:id="rIdSvg1"/>
      </ForeignData>
      <Text>Active Directory</Text>
    </Shape>

    <!-- Shape 2: 1D Connector "Cloud Connect" with 1-based TextBkgnd=2 (White #FFFFFF) -->
    <Shape ID="2" NameU="CloudConnect" Type="Shape" FillStyle="0" LineStyle="0" TextStyle="0">
      <Cell N="BeginX" V="3.0"/>
      <Cell N="BeginY" V="7.0"/>
      <Cell N="EndX" V="7.0"/>
      <Cell N="EndY" V="7.0"/>
      <Cell N="PinX" V="5.0"/>
      <Cell N="PinY" V="7.0"/>
      <Cell N="Width" V="4.0"/>
      <Cell N="Height" V="0.0"/>
      <Cell N="TextBkgnd" V="2"/>
      <Cell N="TextBkgndTrans" V="0"/>
      <Section N="Geometry" IX="0">
        <Cell N="NoFill" V="1"/>
        <Row T="MoveTo" IX="1"><Cell N="X" V="0"/><Cell N="Y" V="0"/></Row>
        <Row T="LineTo" IX="2"><Cell N="X" V="4"/><Cell N="Y" V="0"/></Row>
      </Section>
      <Text>Cloud Connect</Text>
    </Shape>

    <!-- Shape 3: Stretched EllipticalArcTo + NURBSTo red badge (0.6 x 0.6 in) that previously blew up into a 4-inch balloon -->
    <Shape ID="3" NameU="RedShield" Type="Shape" FillStyle="0" LineStyle="0" TextStyle="0">
      <Cell N="PinX" V="5.0" U="MM"/>
      <Cell N="PinY" V="2.5" U="MM"/>
      <Cell N="Width" V="0.6" U="MM"/>
      <Cell N="Height" V="0.6" U="MM"/>
      <Cell N="FillForegnd" V="#cc0000"/>
      <Cell N="FillPattern" V="1"/>
      <Section N="Geometry" IX="0">
        <Cell N="NoFill" V="0"/>
        <Row T="MoveTo" IX="1"><Cell N="X" V="0.0" U="MM"/><Cell N="Y" V="0.3" U="MM"/></Row>
        <Row T="EllipticalArcTo" IX="2">
          <Cell N="X" V="0.6" U="MM"/>
          <Cell N="Y" V="0.3" U="MM"/>
          <Cell N="A" V="-0.02" U="MM"/>
          <Cell N="B" V="0.34" U="MM"/>
          <Cell N="C" V="0" U="DA"/>
          <Cell N="D" V="3.2"/>
        </Row>
        <Row T="NURBSTo" IX="3">
          <Cell N="X" V="0.0" U="MM"/>
          <Cell N="Y" V="0.3" U="MM"/>
          <Cell N="A" V="1"/>
          <Cell N="B" V="1"/>
          <Cell N="C" V="0"/>
          <Cell N="D" V="1"/>
          <Cell N="E" V="NURBS(1,3,0,0,0.85,0.05,0,1,0.5,0.0,0.5,1,0.15,0.05,1,1)"/>
        </Row>
      </Section>
    </Shape>
  </Shapes>
</PageContents>"##,
        )
        .unwrap();

        zip.finish().unwrap();
    }
    cursor.into_inner()
}

#[test]
fn enterprise_vsdx_textbkgnd_captions_elliptical_arcs_nurbs_and_svg_gradients_render_cleanly() {
    let vsdx_bytes = build_synthetic_enterprise_vsdx();
    let (mut doc, _kind) = read_vsdx(&vsdx_bytes).expect("parse enterprise .vsdx");
    ReactiveEngine::recalculate_document(&mut doc);

    let visuals = ReactiveEngine::evaluate_page_visuals(&doc, 1);
    assert_eq!(visuals.len(), 3);

    // 1. Shape 1 ("Active Directory"):
    // - `TextBkgnd = 0` must be `None` (no black box!)
    // - `TxtPinY = 0, TxtLocPinY = TxtHeight (0.3)` must place caption center below icon bottom (`6.0 - 0.3 - 0.15 = 5.55`)
    // - Inherited white StyleSheet `THEMEVAL()` font color must contrast-correct to dark text on white page!
    let ad = visuals.iter().find(|v| v.shape_id == 1).unwrap();
    assert_eq!(ad.text_bg_color, None, "TextBkgnd=0 must be transparent, never black");
    assert!(
        (ad.txt_pos[1] - 5.55).abs() < 1e-3,
        "Below-icon caption must hang cleanly below icon via TxtLocPinY, got y={}",
        ad.txt_pos[1]
    );
    let ad_text_lum =
        (0.2126 * ad.text_color.r as f64 + 0.7152 * ad.text_color.g as f64 + 0.0722 * ad.text_color.b as f64)
            / 255.0;
    assert!(
        ad_text_lum < 0.35,
        "External caption on white page must be dark/readable, got {:?}",
        ad.text_color
    );

    // Verify embedded SVG tessellation:
    // - `<clipPath><rect width="500".../></clipPath>` ignored
    // - `matrix(0.05, ...)` applied so vertices stay strictly within `[1.7..2.3] x [5.7..6.3]` (plus caption below)
    // - `url(#azGrad)` resolved to Azure blue/cyan (not black `[0,0,0]`)
    let ad_mesh = tessellate_shape_full(ad, 0.002);
    assert!(!ad_mesh.vertices.is_empty());
    let mut saw_azure_blue = false;
    for v in &ad_mesh.vertices {
        assert!(
            v.pos[0] >= 1.2 && v.pos[0] <= 2.8 && v.pos[1] >= 5.2 && v.pos[1] <= 6.5,
            "SVG vertex escaped shape/caption bounds: {:?}",
            v.pos
        );
        if v.color[2] > v.color[0] * 1.5 && v.color[2] > 0.2 {
            saw_azure_blue = true;
        }
    }
    assert!(saw_azure_blue, "SVG linearGradient url(#azGrad) must resolve to Azure blue");

    // 2. Shape 2 ("Cloud Connect" 1D connector):
    // - 1-based `TextBkgnd = 2` -> `Colors[1]` = `#FFFFFF` White pill!
    let cc = visuals.iter().find(|v| v.shape_id == 2).unwrap();
    assert_eq!(
        cc.text_bg_color,
        Some(RgbaColor::WHITE),
        "1-based TextBkgnd=2 must resolve to Colors[1] (#FFFFFF)"
    );

    // 3. Shape 3 ("RedShield" with stretched `EllipticalArcTo` + `NURBSTo`):
    // - AABB must remain tightly bounded around `(5.0, 2.5)` (`0.6 x 0.6 in`), NOT a 4-inch reflex balloon!
    // - `NURBSTo` must emit smooth multi-segment curve (> 8 vertices), not a single jagged LineTo!
    let shield = visuals.iter().find(|v| v.shape_id == 3).unwrap();
    let span_w = shield.aabb_max[0] - shield.aabb_min[0];
    let span_h = shield.aabb_max[1] - shield.aabb_min[1];
    assert!(
        span_w < 1.2 && span_h < 1.2,
        "EllipticalArcTo + NURBSTo must stay bounded near 0.6x0.6 in, got {span_w:.3} x {span_h:.3}"
    );
    let line_verb_count = shield.paths[0]
        .verbs_page_space
        .iter()
        .filter(|v| matches!(v, EvaluatedPathVerb::LineTo(_)))
        .count();
    assert!(
        line_verb_count >= 10,
        "NURBSTo must sample smooth Cox-de Boor curve points, got only {line_verb_count} LineTo verbs"
    );
}

#[test]
fn master_explicit_empty_text_override_and_compound_evenodd_cutout_work() {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let opts = SimpleFileOptions::default();

        zip.start_file("[Content_Types].xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"##,
        )
        .unwrap();

        zip.start_file("_rels/.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/document" Target="visio/document.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/document.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<VisioDocument xmlns="http://schemas.microsoft.com/office/visio/2012/main">
  <StyleSheets><StyleSheet ID="0" NameU="No Style"/></StyleSheets>
</VisioDocument>"##,
        )
        .unwrap();

        zip.start_file("visio/_rels/document.xml.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/masters" Target="masters/masters.xml"/>
  <Relationship Id="rId2" Type="http://schemas.microsoft.com/visio/2010/relationships/pages" Target="pages/pages.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/masters/masters.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Masters xmlns="http://schemas.microsoft.com/office/visio/2012/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <Master ID="10" NameU="BadgeMaster"><Rel r:id="rId1"/></Master>
</Masters>"##,
        )
        .unwrap();

        zip.start_file("visio/masters/_rels/masters.xml.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/master" Target="master1.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/masters/master1.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<MasterContents xmlns="http://schemas.microsoft.com/office/visio/2012/main">
  <Shapes>
    <Shape ID="5" Type="Shape">
      <Cell N="Width" V="2"/><Cell N="Height" V="2"/>
      <Cell N="FillForegnd" V="#dc3c00"/><Cell N="FillPattern" V="1"/>
      <Cell N="LinePattern" V="0"/>
      <Cell N="HideText" V="1"/>
      <Section N="Geometry" IX="0">
        <Cell N="NoFill" V="0"/>
        <Row T="MoveTo" IX="1"><Cell N="X" V="0"/><Cell N="Y" V="0"/></Row>
        <Row T="LineTo" IX="2"><Cell N="X" V="2"/><Cell N="Y" V="0"/></Row>
        <Row T="LineTo" IX="3"><Cell N="X" V="2"/><Cell N="Y" V="2"/></Row>
        <Row T="LineTo" IX="4"><Cell N="X" V="0"/><Cell N="Y" V="2"/></Row>
        <Row T="LineTo" IX="5"><Cell N="X" V="0"/><Cell N="Y" V="0"/></Row>
      </Section>
      <Section N="Geometry" IX="1">
        <Cell N="NoFill" V="0"/>
        <Row T="MoveTo" IX="1"><Cell N="X" V="0.5"/><Cell N="Y" V="0.5"/></Row>
        <Row T="LineTo" IX="2"><Cell N="X" V="1.5"/><Cell N="Y" V="0.5"/></Row>
        <Row T="LineTo" IX="3"><Cell N="X" V="1.5"/><Cell N="Y" V="1.5"/></Row>
        <Row T="LineTo" IX="4"><Cell N="X" V="0.5"/><Cell N="Y" V="1.5"/></Row>
        <Row T="LineTo" IX="5"><Cell N="X" V="0.5"/><Cell N="Y" V="0.5"/></Row>
      </Section>
      <Text>Exchange</Text>
    </Shape>
  </Shapes>
</MasterContents>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/pages.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Pages xmlns="http://schemas.microsoft.com/office/visio/2012/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <Page ID="1" NameU="Page-1"><Rel r:id="rId1"/></Page>
</Pages>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/_rels/pages.xml.rels", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.microsoft.com/visio/2010/relationships/page" Target="page1.xml"/>
</Relationships>"##,
        )
        .unwrap();

        zip.start_file("visio/pages/page1.xml", opts).unwrap();
        zip.write_all(
            br##"<?xml version="1.0" encoding="UTF-8"?>
<PageContents xmlns="http://schemas.microsoft.com/office/visio/2012/main">
  <Shapes>
    <!-- Shape 10: HideText=0 WITH explicit empty <Text/> must NOT inherit "Exchange" -->
    <Shape ID="10" Master="10" Type="Shape">
      <Cell N="PinX" V="2"/><Cell N="PinY" V="2"/>
      <Cell N="HideText" V="0"/>
      <Text/>
    </Shape>
    <!-- Shape 11: HideText=0 WITHOUT <Text/> MUST inherit "Exchange" -->
    <Shape ID="11" Master="10" Type="Shape">
      <Cell N="PinX" V="5"/><Cell N="PinY" V="2"/>
      <Cell N="HideText" V="0"/>
    </Shape>
  </Shapes>
</PageContents>"##,
        )
        .unwrap();
        zip.finish().unwrap();
    }

    let (mut doc, _) = read_vsdx(&cursor.into_inner()).expect("parse master empty text override vsdx");
    ReactiveEngine::recalculate_document(&mut doc);
    let visuals = ReactiveEngine::evaluate_page_visuals(&doc, 1);

    let s10 = visuals.iter().find(|v| v.shape_id == 10).unwrap();
    assert_eq!(s10.text, "", "Explicit empty <Text/> must override Master text even when HideText=0");

    let s11 = visuals.iter().find(|v| v.shape_id == 11).unwrap();
    assert_eq!(s11.text, "Exchange", "Omitted <Text> with HideText=0 must inherit Master text");

    // Verify compound Even-Odd cutout: center `(2.0, 2.0)` lies inside inner `[1.5..2.5] x [1.5..2.5]` hole!
    let mesh10 = tessellate_shape_full(s10, 0.002);
    let point_in_tri = |p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]| -> bool {
        let cross = |u: [f32; 2], v: [f32; 2], w: [f32; 2]| (v[0] - u[0]) * (w[1] - u[1]) - (v[1] - u[1]) * (w[0] - u[0]);
        let d1 = cross(p, a, b);
        let d2 = cross(p, b, c);
        let d3 = cross(p, c, a);
        let has_neg = (d1 < -1e-5) || (d2 < -1e-5) || (d3 < -1e-5);
        let has_pos = (d1 > 1e-5) || (d2 > 1e-5) || (d3 > 1e-5);
        !(has_neg && has_pos)
    };
    let mut covered_center = false;
    let mut covered_rim = false;
    for tri in mesh10.indices.chunks_exact(3) {
        let a = mesh10.vertices[tri[0] as usize].pos;
        let b = mesh10.vertices[tri[1] as usize].pos;
        let c = mesh10.vertices[tri[2] as usize].pos;
        if point_in_tri([2.0, 2.0], a, b, c) {
            covered_center = true;
        }
        if point_in_tri([1.2, 2.0], a, b, c) {
            covered_rim = true;
        }
    }
    assert!(covered_rim, "Outer ring of compound Geometry must be filled");
    assert!(!covered_center, "Inner Geometry2 cutout hole must NOT be filled under Even-Odd rule");
}
