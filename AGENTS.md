# AGENTS.md — Vexio Master Workspace Constitution & Architectural Invariants

## 1. Mission
**Vexio** is a 1:1 Microsoft Visio Professional / Plan 2 functional parity suite built atop a single, deterministic, zero-UI-dependency **Rust** core (`crates/*`), delivered through strictly native platform shells (`apps/windows`, `apps/macos`, `apps/android-googlebook`) and a **Firebase-powered, 100% offline-first ChromeOS PWA** (`apps/chromeos-pwa` + `firebase/`).

## 2. Non-Negotiable Directives
1. **Zero Stubs / Zero Watered-Down Features:** Never commit `todo!()`, `unimplemented!()`, placeholder match arms, fake mock parsers, or stripped-down formula sets. Every module must implement real, deterministic algorithms backed by automated unit and round-trip tests.
2. **Zero-Wrapper Native Mandate:**
   - **Windows (`apps/windows`):** Native WinUI 3 / Windows App SDK (C++20 / CppWinRT) + DirectX 12 `ISwapChainPanelNative` over `vexio.h` C-ABI.
   - **macOS (`apps/macos`):** Native Swift 6 + SwiftUI/AppKit (Apple Liquid Glass HIG) + Metal `CAMetalLayer` + Quick Look extensions.
   - **Android & Googlebook OS (`apps/android-googlebook`):** 100% Native Kotlin + Jetpack Compose Material 3 Expressive + Compose Adaptive / Navigation 3 + Vulkan NDK/JNI.
   - **Prohibited Frameworks:** Electron, Tauri webviews, Capacitor, Cordova, React Native, Flutter, or stretched phone layouts on desktop/tablet are strictly forbidden.
3. **Firebase Offline-First ChromeOS PWA (`apps/chromeos-pwa` & `firebase/`):**
   - Deployed via **Firebase Hosting** (NEVER an Isolated Web App / `.swbn` bundle).
   - Must serve `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` headers to unlock `SharedArrayBuffer` for multi-threaded Rust WASM SIMD and maximum WebGPU throughput.
   - Must run **100% offline even when unauthenticated**, using Workbox Service Worker precaching, Origin Private File System (`FileSystemSyncAccessHandle`) WAL/swap journals, Window Controls Overlay (`display_override: ["window-controls-overlay", "standalone"]`), and ChromeOS Launch Queue (`file_handlers`) + File System Access API for direct local disk `.vsdx`/`.vsd`/`.dwg` editing.
   - Modular **Firebase SDK v11+** (`Auth`, `Firestore` with IndexedDB Multi-Tab Offline Persistence, `Cloud Storage`) provides optional seamless cloud sync and workspace backup without ever blocking offline startup or file operations.

## 3. Monorepo Map
- `crates/vexio-shapesheet`: Cell/section store, unit-aware values, formula parser, 220+ ShapeSheet functions, and reactive topological DAG (`GUARD`, `SETF`, `DEPENDSON`, `PAR`, `PNT`, `INTERSECTX/Y`, `GRAVITY`).
- `crates/vexio-vsdx`: Lossless OpenXML OPC reader/writer (`.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, `.vstm`) preserving unknown parts, relationships, themes, masters, background pages, and `vbaProject.bin`.
- `crates/vexio-vsd-legacy`: Legacy OLE2 Compound File Binary reader/writer (`.vsd`, `.vss`, `.vst`) + DatadiagramML XML (`.vdx`) & Web Drawing (`.vdw`) codecs.
- `crates/vexio-cad`: Native offline `.dwg` / `.dxf` parser and `.dxf` exporter with CAD layer mapping, architectural scale locking, and CAD-to-ShapeSheet geometry conversion.
- `crates/vexio-render`: `wgpu` rendering pipeline (DX12, Metal, Vulkan, WebGPU/WebGL2), vector path tessellation, rich text layout, and R*-tree spatial index for 120 FPS at 50,000+ shapes.
- `crates/vexio-core`: Document transactions, Undo/Redo WAL, dynamic orthogonal/curved connector routing with obstacle avoidance & line jumps, containers/lists/swimlanes, auto-layouts, BPMN 2.0 / UML 2.5 / SharePoint validation, two-way Data Linking, Data Graphics, Legends, BOM Shape Reports, Markup Ink, sandboxed VBA-compatible Automation engine, PDF/A/SVG/EPS/DXF/HTML/raster/tiled-print exports, and C-ABI/Swift/JNI/WASM FFI surfaces.
- `apps/*`: Native OS shells (`windows`, `macos`, `android-googlebook`) and `chromeos-pwa`.
- `firebase/`: `firebase.json`, `firestore.rules`, `firestore.indexes.json`, `storage.rules`.

## 4. Mandatory Rule Modules (`.agents/rules/`)
- `.agents/rules/core-rust.md` (`always_on`)
- `.agents/rules/visio-parity.md` (`always_on`)
- `.agents/rules/windows-ui.md` (`model_decision`)
- `.agents/rules/macos-liquid.md` (`model_decision`)
- `.agents/rules/googlebook-m3e.md` (`model_decision`)
- `.agents/rules/chromeos-firebase-pwa.md` (`model_decision`)

## 5. Verification Gates
- Run `cargo test --workspace` after every Rust core change; zero failing tests or compiler warnings allowed.
- Run WASM build (`wasm32-unknown-unknown` with SIMD) and Vite/Workbox bundle checks after any `apps/chromeos-pwa` or `ffi::wasm_api` change.
- Verify COOP/COEP headers, `.wasm` MIME types, Firestore/Storage security rules, and 100% unauthenticated offline load against the local Firebase Emulator Suite.
