---
trigger: always_on
---

# Core Rust Engine Rules (`crates/*`)

1. **Zero UI Coupling:** Crates under `crates/` (`vexio-shapesheet`, `vexio-vsdx`, `vexio-vsd-legacy`, `vexio-cad`, `vexio-render`, `vexio-core`) must compile cleanly across `x86_64`, `aarch64`, and `wasm32-unknown-unknown` without linking platform UI toolkits. Platform surface pointers enter strictly via narrow FFI boundary modules (`vexio-core/src/ffi/*`).
2. **Absolute Ban on Placeholder Code:** Never introduce `todo!()`, `unimplemented!()`, `panic!("not implemented")`, empty stub functions, or mock return values. Every public API, parser branch, formula builtin, and export target must execute real logic.
3. **Determinism & Reproducibility:**
   - Use `BTreeMap` / `BTreeSet` (or deterministic insertion-ordered maps) whenever serializing XML attributes, OPC relationships, ShapeSheet rows, or command journals so round-trip output is byte-deterministic.
   - Handle `NaN`, `-0.0`, and division-by-zero deterministically using Visio error values (`#DIV/0!`, `#VALUE!`, `#NAME?`, `#REF!`) rather than panicking.
4. **Memory & FFI Safety:**
   - Restrict `unsafe` blocks strictly to `vexio-core/src/ffi/` and `vexio-render` raw surface attachment.
   - Every `unsafe` block must carry an explicit `// SAFETY:` invariant comment verifying pointer validity, alignment, lifetime, and thread ownership.
   - Catch panics (`std::panic::catch_unwind`) at every `extern "C"` and JNI entry point so Rust never unwinds across foreign stack frames.
5. **Performance Budget:**
   - Spatial queries (pan/zoom frustum cull, hit-test, dynamic glue target lookup) must use the R*-tree index (`O(log N)`) to sustain **120 FPS (< 8.33 ms/frame)** on 50,000+ shapes.
   - Reactive ShapeSheet updates must traverse only dirty topological subgraphs rather than re-evaluating unaffected shapes.
