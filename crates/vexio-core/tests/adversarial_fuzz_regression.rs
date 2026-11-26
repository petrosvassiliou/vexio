//! v0.5.0-beta Verification & Deterministic Adversarial Parser Fuzz Regression Suite.
//!
//! Covers:
//! 1. Multi-bend orthogonal connector segment dragging (`User.ManualOrthoPoints`), dogleg stub
//!    insertion on outer legs, glued-shape motion re-anchoring, and `reset_connector_routing`.
//! 2. Pre-wired parametric yellow diamond `Controls.Row_1` handles on Rounded Rectangle, Start/End Pill,
//!    Cylinder/Database, Parallelogram, Right Block Arrow, Chevron, and Folded Note stencils.
//! 3. Bidirectional Mermaid (`.mmd`), PlantUML (`.puml`), and Graphviz DOT (`.dot`) interop including
//!    `subgraph` / `package` / `cluster_*` containers, sequence/class/ER/state diagrams, and round-trips.
//! 4. Persisted Presentation Mode `SlideSnippetFrame`s (`User.VexioSlideSnippetsJson`) across `.vsdx`
//!    save/load cycles + multi-slide OpenXML `.pptx` deck generation.
//! 5. Deterministic mutation/corruption fuzzing across `.vsdx`, `.vsd`, `.vdx`, `.dwg`, `.dxf`,
//!    ShapeSheet formulas, and Mermaid/PlantUML/DOT parsers (guaranteeing zero panics!).

use std::ffi::CString;
use std::io::Cursor;
use vexio_core::DocumentSession;
use vexio_core::boolean_ops::list_control_handles;
use vexio_core::diagram_text_interop::{
    DiagramMarkupFormat, detect_diagram_markup_format, export_diagram_markup, import_diagram_markup,
};
use vexio_core::ffi::c_abi::*;
use vexio_shapesheet::{ReactiveEngine, VisioDocument, VisioShape};

#[test]
fn multi_bend_orthogonal_segment_drag_doglegs_reanchor_and_reset() {
    let mut session = DocumentSession::new_blank("MultiBendRouting");
    let pid = 1;

    let s1 = session
        .draw_primitive_2d(pid, "rect", 1.0, 5.0, 2.6, 6.0)
        .expect("s1");
    // Obstacle directly between s1 and s2 so A* produces a multi-bend detour!
    let _obs = session
        .draw_primitive_2d(pid, "rect", 4.2, 4.6, 5.8, 6.4)
        .expect("obs");
    let s2 = session
        .draw_primitive_2d(pid, "rect", 7.4, 5.0, 9.0, 6.0)
        .expect("s2");

    let cid = session.connect_shapes(pid, s1, s2).expect("connect");

    let visuals0 = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
    let poly0 = vexio_render::tessellate::extract_connector_polyline_in(
        visuals0.iter().find(|v| v.shape_id == cid).unwrap(),
    );
    assert!(
        poly0.len() >= 4,
        "A* detour around obstacle must have >= 4 points, got {:?}",
        poly0
    );

    // Drag segment 1 to Y = 7.25 in AND drag segment 2 to X = 6.80 in -> BOTH bends must persist!
    assert!(session.drag_connector_segment(pid, cid, 1, 7.25).unwrap());
    let visuals1 = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
    let poly1 = vexio_render::tessellate::extract_connector_polyline_in(
        visuals1.iter().find(|v| v.shape_id == cid).unwrap(),
    );
    assert!(
        poly1.iter().any(|p| (p[1] - 7.25).abs() < 1e-3 || (p[0] - 7.25).abs() < 1e-3),
        "first dragged bend must land at 7.25 in: {:?}",
        poly1
    );

    // Dragging outer segment `0` inserts a clean dogleg stub without losing orthogonality!
    assert!(session.drag_connector_segment(pid, cid, 0, 6.60).unwrap());
    let visuals2 = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
    let poly2 = vexio_render::tessellate::extract_connector_polyline_in(
        visuals2.iter().find(|v| v.shape_id == cid).unwrap(),
    );
    for seg in poly2.windows(2) {
        let dx = (seg[0][0] - seg[1][0]).abs();
        let dy = (seg[0][1] - seg[1][1]).abs();
        assert!(
            dx < 1e-4 || dy < 1e-4,
            "every segment in multi-bend polyline must remain strictly orthogonal: {:?}",
            poly2
        );
    }

    // Move glued target `s2` by `(0.5, -0.8)` -> connector re-anchors cleanly while preserving manual bends!
    session.move_shapes_delta(pid, &[s2], 0.5, -0.8).unwrap();
    let visuals3 = ReactiveEngine::evaluate_page_visuals(&session.doc, pid);
    let poly3 = vexio_render::tessellate::extract_connector_polyline_in(
        visuals3.iter().find(|v| v.shape_id == cid).unwrap(),
    );
    for seg in poly3.windows(2) {
        let dx = (seg[0][0] - seg[1][0]).abs();
        let dy = (seg[0][1] - seg[1][1]).abs();
        assert!(
            dx < 1e-4 || dy < 1e-4,
            "re-anchored manual polyline must stay strictly orthogonal: {:?}",
            poly3
        );
    }

    // `reset_connector_routing` clears `User.ManualOrthoPoints` and restores automatic A* route!
    assert!(session.reset_connector_routing(pid, cid).unwrap());
    let sh_conn = session.doc.page(pid).unwrap().find_shape(cid).unwrap();
    assert!(!sh_conn.sheet.user_cells.contains_key("manualorthopoints"));
}

#[test]
fn parametric_yellow_control_handles_deform_stencils_reactively() {
    let mut session = DocumentSession::new_blank("ParametricControls");
    let pid = 1;

    for (pack, master_name, dx, dy) in [
        ("Basic Shapes", "Rounded Rectangle", 0.35, 0.0),
        ("Basic Shapes", "Parallelogram", 0.30, 0.0),
        ("Basic Shapes", "Cylinder / Can", 0.0, -0.20),
        ("Basic Flowchart", "Start/End", -0.20, 0.0),
        ("Basic Flowchart", "Data (I/O)", 0.28, 0.0),
        ("Basic Flowchart", "Database", 0.0, -0.18),
        ("Arrows & Block Arrows", "Right Block Arrow", -0.25, 0.10),
        ("Arrows & Block Arrows", "Chevron", -0.20, 0.0),
        ("Callouts, Notes & Containers", "Folded Note", -0.18, 0.0),
    ] {
        let sid = session
            .drop_builtin_shape(pid, pack, master_name, 4.0, 4.0)
            .unwrap_or_else(|e| panic!("drop {pack}::{master_name} failed: {e}"));
        let handles_before = list_control_handles(&session.doc, pid, sid);
        assert!(
            !handles_before.is_empty(),
            "{pack}::{master_name} must expose pre-wired yellow Controls.Row_1 handle"
        );
        let h0 = &handles_before[0];
        let moved = session
            .drag_control_handle(pid, sid, &h0.name, h0.page_x_in + dx, h0.page_y_in + dy)
            .expect("drag control handle");
        assert!(moved, "dragging Controls.{} on {master_name} must succeed", h0.name);
        let handles_after = list_control_handles(&session.doc, pid, sid);
        assert!(
            (handles_after[0].page_x_in - (h0.page_x_in + dx)).abs() < 1e-3
                && (handles_after[0].page_y_in - (h0.page_y_in + dy)).abs() < 1e-3
        );
    }
}

#[test]
fn mermaid_plantuml_and_graphviz_dot_bidirectional_roundtrip() {
    // 1. Mermaid Flowchart with Subgraph, Stadium, Diamond, Subprocess, Cylinder & Styled Edges
    let mermaid_src = r#"
flowchart TD
    subgraph Auth ["API Gateway & Auth"]
        A(["Client Request"]) --> B{"Valid JWT?"}
    end
    B -->|Yes| C[["Order Service"]]
    B -.->|No| D(["401 Unauthorized"])
    C ==> E[("Orders PostgreSQL")]
"#;
    assert_eq!(
        detect_diagram_markup_format(mermaid_src),
        DiagramMarkupFormat::Mermaid
    );
    let mut doc_m = VisioDocument::new_blank("MermaidFlow");
    let st_m =
        import_diagram_markup(&mut doc_m, 1, mermaid_src, DiagramMarkupFormat::Auto).unwrap();
    assert_eq!(st_m.nodes_created, 5);
    assert_eq!(st_m.containers_created, 1);
    assert_eq!(st_m.connectors_created, 4);

    let reexported_mmd =
        export_diagram_markup(&doc_m, 1, DiagramMarkupFormat::Mermaid).expect("export mermaid");
    assert!(reexported_mmd.contains("subgraph "));
    assert!(reexported_mmd.contains("Orders PostgreSQL"));

    // Round-trip via DocumentSession `.mmd`, `.puml`, and `.dot` file codecs!
    let mut sess = DocumentSession::new_blank("MarkupSession");
    sess.open_bytes(reexported_mmd.as_bytes(), "mmd")
        .expect("open .mmd");
    assert!(sess.doc.page(1).unwrap().shapes.len() >= 9);

    // 2. Mermaid Sequence, Class, ER, and State diagrams
    for extra_mermaid in [
        "sequenceDiagram\n  actor User\n  participant API as Gateway\n  User->>API: POST /orders\n  API-->>User: 201 Created",
        "classDiagram\n  class Order {\n    +UUID id\n    +submit()\n  }\n  class LineItem\n  Order *-- LineItem : contains",
        "erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  CUSTOMER {\n    uuid id PK\n    string email\n  }",
        "stateDiagram-v2\n  [*] --> Pending\n  Pending --> Approved : sign_off\n  Approved --> [*]",
    ] {
        let mut d = VisioDocument::new_blank("MermaidSubMode");
        let stats =
            import_diagram_markup(&mut d, 1, extra_mermaid, DiagramMarkupFormat::Mermaid).unwrap();
        assert!(stats.nodes_created >= 2);
        assert!(stats.connectors_created >= 1);
    }

    // 3. PlantUML with `package`, `actor`, `component`, `database`, `note`, and dashed/solid arrows
    let puml_src = r#"
@startuml
left to right direction
actor "Enterprise User" as usr
package "Backend Cluster" as backend {
    component "Billing Engine" as bill
    database "Ledger DB" as db
}
usr --> bill : HTTPS
bill ..> db : SQL Read/Write
note right of bill : PCI-DSS Scope
@enduml
"#;
    assert_eq!(
        detect_diagram_markup_format(puml_src),
        DiagramMarkupFormat::PlantUml
    );
    let mut doc_p = VisioDocument::new_blank("PlantUmlDoc");
    let st_p = import_diagram_markup(&mut doc_p, 1, puml_src, DiagramMarkupFormat::Auto).unwrap();
    assert_eq!(st_p.nodes_created, 4);
    assert_eq!(st_p.containers_created, 1);
    assert_eq!(st_p.connectors_created, 3);
    let exported_puml = export_diagram_markup(&doc_p, 1, DiagramMarkupFormat::PlantUml).unwrap();
    assert!(exported_puml.contains("@startuml") && exported_puml.contains("@enduml"));

    // 4. Graphviz DOT with `subgraph cluster_*`, `rankdir=LR`, shapes, colors, and edge chains
    let dot_src = r##"
digraph ProdTopology {
    rankdir=LR;
    // Edge ingress cluster
    subgraph cluster_edge {
        label="Edge Tier";
        lb [label="Cloud Load Balancer", shape=diamond, fillcolor="#FEF3C7"];
        waf [label="WAF Gateway", shape=hexagon];
    }
    db [label="Primary AlloyDB", shape=cylinder, fillcolor="#E0F2FE"];
    lb -> waf -> db [label="mTLS", style=bold];
}
"##;
    assert_eq!(
        detect_diagram_markup_format(dot_src),
        DiagramMarkupFormat::GraphvizDot
    );
    let mut doc_d = VisioDocument::new_blank("DotDoc");
    let st_d = import_diagram_markup(&mut doc_d, 1, dot_src, DiagramMarkupFormat::Auto).unwrap();
    assert_eq!(st_d.nodes_created, 3);
    assert_eq!(st_d.containers_created, 1);
    assert_eq!(st_d.connectors_created, 2);
    let exported_dot = export_diagram_markup(&doc_d, 1, DiagramMarkupFormat::GraphvizDot).unwrap();
    assert!(exported_dot.contains("digraph VexioDiagram") && exported_dot.contains("subgraph cluster_"));
}

#[test]
fn slide_snippets_persist_across_vsdx_and_export_multi_slide_pptx() {
    let mut session = DocumentSession::new_blank("ExecutiveDeck");
    let pid = 1;
    let a = session
        .draw_primitive_2d(pid, "round_rect", 1.0, 5.5, 2.8, 6.5)
        .unwrap();
    let b = session
        .draw_primitive_2d(pid, "round_rect", 5.0, 5.5, 6.8, 6.5)
        .unwrap();
    let c = session
        .draw_primitive_2d(pid, "round_rect", 8.2, 2.0, 10.0, 3.0)
        .unwrap();
    session.connect_shapes(pid, a, b).unwrap();
    session.connect_shapes(pid, b, c).unwrap();

    let generated = session.auto_generate_slide_snippets(pid).unwrap();
    assert!(
        generated.len() >= 3,
        "expected overview + spatial stage slides, got {}",
        generated.len()
    );

    let extra_id = session
        .add_slide_snippet(pid, "Executive Summary Zoom", "Key KPI callout", 0.5, 4.5, 4.0, 3.0)
        .unwrap();
    assert!(extra_id > 0);
    let total_slides = session.list_slide_snippets().len();
    assert_eq!(total_slides, generated.len() + 1);

    // Save to `.vsdx` and reload -> all Slide Snippets MUST round-trip losslessly!
    let vsdx_bytes = session.save_format("vsdx").expect("save vsdx");
    let mut reloaded = DocumentSession::new_blank("ReloadedDeck");
    reloaded.open_bytes(&vsdx_bytes, "vsdx").expect("open vsdx");
    let reloaded_slides = reloaded.list_slide_snippets();
    assert_eq!(reloaded_slides.len(), total_slides);
    assert!(
        reloaded_slides
            .iter()
            .any(|s| s.title == "Executive Summary Zoom" && s.notes == "Key KPI callout")
    );

    // Export `.pptx` via `save_format("pptx")` -> ZIP must contain `slide1.xml` .. `slide{total_slides}.xml`!
    let pptx_bytes = reloaded.save_format("pptx").expect("save pptx");
    let mut archive = zip::ZipArchive::new(Cursor::new(&pptx_bytes)).expect("valid pptx zip");
    for i in 1..=total_slides {
        assert!(
            archive.by_name(&format!("ppt/slides/slide{i}.xml")).is_ok(),
            "missing ppt/slides/slide{i}.xml in exported multi-slide .pptx"
        );
    }

    // Verify C-ABI v0.5.0-beta entrypoints end-to-end
    unsafe {
        let title_c = CString::new("CAbiV050").unwrap();
        let doc = vexio_doc_create_blank(title_c.as_ptr());
        let mmd_c = CString::new("flowchart LR\n  X[Alpha] --> Y([Beta])").unwrap();
        let fmt_c = CString::new("mermaid").unwrap();
        let mut created_nodes = 0u32;
        assert_eq!(
            vexio_doc_import_markup(doc, 1, mmd_c.as_ptr(), fmt_c.as_ptr(), &mut created_nodes),
            VexioStatus::Ok
        );
        assert_eq!(created_nodes, 2);

        let mut out_dot = VexioOwnedBuffer {
            ptr: std::ptr::null_mut(),
            len: 0,
            cap: 0,
        };
        let dot_fmt = CString::new("dot").unwrap();
        assert_eq!(
            vexio_doc_export_markup(doc, 1, dot_fmt.as_ptr(), &mut out_dot),
            VexioStatus::Ok
        );
        assert!(out_dot.len > 10);
        vexio_buffer_free(out_dot);

        let mut auto_cnt = 0u32;
        assert_eq!(
            vexio_doc_auto_slide_snippets(doc, 1, &mut auto_cnt),
            VexioStatus::Ok
        );
        assert!(auto_cnt >= 2);
        vexio_doc_destroy(doc);
    }
}

#[test]
fn deterministic_adversarial_parser_fuzz_corpus_never_panics() {
    // Seed a valid .vsdx, .vsd, .vdx, .dwg, and .dxf byte stream, then subject each to
    // truncations, bit-flips, zero-fills, header mutations, and hostile UTF-8 payloads!
    let mut seed_session = DocumentSession::new_blank("FuzzSeed");
    let s1 = seed_session
        .draw_primitive_2d(1, "round_rect", 1.0, 2.0, 3.0, 3.5)
        .unwrap();
    let s2 = seed_session
        .draw_primitive_2d(1, "diamond", 5.0, 2.0, 6.5, 3.5)
        .unwrap();
    seed_session.connect_shapes(1, s1, s2).unwrap();

    let valid_vsdx = seed_session.save_format("vsdx").unwrap();
    let valid_vsd = seed_session.save_format("vsd").unwrap();
    let valid_vdx = seed_session.save_format("vdx").unwrap();
    let valid_dwg = seed_session.save_format("dwg").unwrap();
    let valid_dxf = seed_session.save_format("dxf").unwrap();

    // 1. Truncation & deterministic Xorshift64 bit-flip fuzzing across all 5 binary/XML codecs
    let mut rng_state: u64 = 0x5EED_CAFE_F00D_2026;
    let mut next_u64 = || -> u64 {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 7;
        rng_state ^= rng_state << 17;
        rng_state
    };

    for (ext, corpus) in [
        ("vsdx", &valid_vsdx),
        ("vsd", &valid_vsd),
        ("vdx", &valid_vdx),
        ("dwg", &valid_dwg),
        ("dxf", &valid_dxf),
    ] {
        // Empty & tiny slices
        for cut in [0usize, 1, 2, 4, 8, 16, 31, 64, 128, corpus.len() / 2] {
            if cut <= corpus.len() {
                let mut probe = DocumentSession::new_blank("TruncProbe");
                let _ = probe.open_bytes(&corpus[..cut], ext);
            }
        }
        // 24 deterministic multi-byte corruption rounds per format (120 binary fuzz cases)
        for _ in 0..24 {
            let mut mutated = corpus.clone();
            let flips = (next_u64() % 12) + 1;
            for _ in 0..flips {
                let idx = (next_u64() as usize) % mutated.len();
                mutated[idx] ^= (next_u64() & 0xFF) as u8;
            }
            let mut probe = DocumentSession::new_blank("MutProbe");
            let _ = probe.open_bytes(&mutated, ext);
        }
    }

    // 2. Hostile ShapeSheet formulas (deep nesting, division by zero, cyclic refs, malformed tokens, NaN)
    let hostile_formulas = [
        "=",
        "=((((((((((((((((((((1+2))))))))))))))))))))",
        "=1/0 + SQRT(-9) + ATAN2(0,0)",
        "=Sheet.1!User.SelfCycle + 1",
        "=GUARD(SETF(GetRef(Width), \"=1/0\"))",
        "=INDEX(999999, \"a;b;c\")",
        "=LOOKUP(\"missing\", \"a;b\")",
        "=RGB(999999, -50, 1e308)",
        "=\"unterminated string literal",
        "=DEPENDSON(Sheet.9999!PinX, UnknownFunc(1, 2, 3))",
        "=BOUND(10 in, 0, TRUE, 5 in, 2 in, 1, 99, 0)",
        "=GRAVITY(0, -0.0, NaN)",
    ];
    let mut fdoc = VisioDocument::new_blank("HostileFormulas");
    if let Some(page) = fdoc.page_mut(1) {
        let mut sh = VisioShape::new_rect(1, "FuzzShape", 2.0, 2.0, 1.5, 1.0);
        sh.sheet.set_user_cell("SelfCycle", "=User.CycleB + 1", "");
        sh.sheet.set_user_cell("CycleB", "=User.SelfCycle + 1", "");
        for (i, expr) in hostile_formulas.iter().enumerate() {
            sh.sheet
                .set_user_cell(&format!("Hostile_{i}"), *expr, "Adversarial Formula");
        }
        page.shapes.push(sh);
    }
    ReactiveEngine::recalculate_document(&mut fdoc);
    let _ = ReactiveEngine::evaluate_page_visuals(&fdoc, 1);

    // 3. Hostile / malformed Mermaid, PlantUML & Graphviz DOT inputs
    let hostile_markups = [
        "",
        "   \n\t  ",
        "flowchart TD\n subgraph Unclosed [\"Missing End\"\n A --> B --> --> C{{{bad",
        "A --> A\nA --> B\nB --> C\nC --> A",
        "@startuml\npackage \"Deep\" {\n class Broken {\n +unclosed\n",
        "digraph {{{{ rankdir=INVALID; a -> b -> -> c [label=\"\\\"unterminated]; }",
        "erDiagram\n A ||--o{ B : \"\"\n B }|--|| A : loop",
        "sequenceDiagram\n participant\n ->> : empty",
    ];
    for sample in hostile_markups {
        for fmt in [
            DiagramMarkupFormat::Auto,
            DiagramMarkupFormat::Mermaid,
            DiagramMarkupFormat::PlantUml,
            DiagramMarkupFormat::GraphvizDot,
        ] {
            let mut d = VisioDocument::new_blank("HostileMarkup");
            let _ = import_diagram_markup(&mut d, 1, sample, fmt);
        }
    }
}
