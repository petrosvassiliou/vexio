# Vexio

**Vexio** is an enterprise-grade, **1:1 Microsoft Visio Professional / Plan 2** functional parity diagramming, CAD interop, and reactive data-visualization workstation suite.

Every parser, reactive ShapeSheet formula, layout router, CAD translator, validation rule, and GPU tessellation pass lives inside a **single, deterministic, zero-UI-dependency Rust workspace (`crates/*`)**, delivered through **strictly zero-wrapper native OS workstation shells** (`apps/windows`, `apps/macos`, `apps/android-googlebook`) and a **Firebase-powered, 100% unauthenticated offline-first ChromeOS PWA** (`apps/chromeos-pwa` + `firebase/`).

---

## Download Installers (`v0.1.0-alpha`)

All compiled native installers are published on the [**`v0.1.0-alpha` Releases Page**](https://github.com/petrosvassiliou/vexio/releases/tag/v0.1.0-alpha):

| Platform | Architecture | Direct Installer Download |
| :--- | :--- | :--- |
| **Windows 11 Native** (`WinUI 3 + DirectX 12`) | `x64` | [**`Vexio-Windows-v0.1.0-alpha-x64.msix`**](https://github.com/petrosvassiliou/vexio/releases/download/v0.1.0-alpha/Vexio-Windows-v0.1.0-alpha-x64.msix) |
| **Windows 11 Native** (`WinUI 3 + DirectX 12`) | `ARM64` | [**`Vexio-Windows-v0.1.0-alpha-arm64.msix`**](https://github.com/petrosvassiliou/vexio/releases/download/v0.1.0-alpha/Vexio-Windows-v0.1.0-alpha-arm64.msix) |
| **macOS 15+ Native** (`Swift 6 Liquid Glass + Metal`) | `Universal2` (`Apple Silicon + Intel`) | [**`Vexio-v0.1.0-alpha-Universal2.dmg`**](https://github.com/petrosvassiliou/vexio/releases/download/v0.1.0-alpha/Vexio-v0.1.0-alpha-Universal2.dmg) |
| **Android & Googlebook OS** (`Compose M3E + Vulkan`) | `arm64-v8a` + `x86_64` APK | [**`Vexio-Android-Googlebook-v0.1.0-alpha.apk`**](https://github.com/petrosvassiliou/vexio/releases/download/v0.1.0-alpha/Vexio-Android-Googlebook-v0.1.0-alpha.apk) |
| **Android & Googlebook OS** (`Play App Bundle`) | `arm64-v8a` + `x86_64` AAB | [**`Vexio-Android-Googlebook-v0.1.0-alpha.aab`**](https://github.com/petrosvassiliou/vexio/releases/download/v0.1.0-alpha/Vexio-Android-Googlebook-v0.1.0-alpha.aab) |
| **ChromeOS Offline-First PWA** | Live Web + Offline Bundle | [**Launch Live App**](https://vexio-chromeos-suite.web.app) · [**`Vexio-ChromeOS-PWA-v0.1.0-alpha.zip`**](https://github.com/petrosvassiliou/vexio/releases/download/v0.1.0-alpha/Vexio-ChromeOS-PWA-v0.1.0-alpha.zip) |

---

## Architectural Invariants

1. **Zero Stubs, Deterministic Core:** Every Rust module executes complete, deterministic algorithms (`BTreeMap`/`BTreeSet` canonical serialization, Visio error propagation `#DIV/0!`, `#VALUE!`, `#NAME?`, `#REF!`) backed by automated unit and lossless round-trip test suites.
2. **Strict Zero-Wrapper Native Mandate:** Electron, Tauri webviews, Capacitor, Cordova, React Native, and Flutter are strictly prohibited. Each operating system binds directly to native platform UI controls and low-level GPU swapchains (`DirectX 12`, `Metal`, `Vulkan`, `WebGPU`).
3. **100% Offline-First ChromeOS Workstation PWA:** Deployed via **Firebase Hosting** (never an Isolated Web App `.swbn` bundle) with `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` headers unlocking `SharedArrayBuffer` for multi-threaded Rust WASM SIMD and WebGPU throughput. Runs 100% offline even when unauthenticated via Service Worker precaching, OPFS `FileSystemSyncAccessHandle` WAL journals, Window Controls Overlay, and File System Access API + ChromeOS `file_handlers`, with optional non-blocking Firebase v11 cloud sync.
4. **120 FPS Spatial & Topological Performance Budget:** Viewport frustum culling, hit-testing, and dynamic connector walking glue execute in `O(log N)` via an `rstar` R*-tree spatial index, sustaining **120 FPS (< 8.33 ms/frame)** at 50,000+ shapes while incremental ShapeSheet DAG invalidation recomputes only dirty dependent cells.

---

## Monorepo Architecture

```text
vexio/
├── crates/
│   ├── vexio-shapesheet/       # Unit-aware cell/section store, formula AST, 220+ functions & reactive DAG
│   ├── vexio-vsdx/             # Lossless OpenXML OPC codec (.vsdx, .vssx, .vstx, .vsdm, .vssm, .vstm)
│   ├── vexio-vsd-legacy/       # Legacy OLE2/CFB (.vsd, .vss, .vst), DatadiagramML (.vdx) & .vdw codecs
│   ├── vexio-cad/              # Offline .dwg/.dxf parser & .dxf exporter with CAD layer/scale locking
│   ├── vexio-render/           # wgpu pipeline (DX12, Metal, Vulkan, WebGPU/WebGL2), lyon & R*-tree index
│   └── vexio-core/             # ACID WAL, orthogonal/curved routing, BPMN/UML rules, Data Linking & FFI
│       └── include/vexio.h     # Canonical C-ABI header consumed by C++20 (WinUI 3) & Swift 6 (macOS)
├── apps/
│   ├── windows/                # Native WinUI 3 / Windows App SDK 1.6 (C++20 / CppWinRT) + DX12 + MSIX
│   ├── macos/                  # Native Swift 6 + AppKit/SwiftUI (Apple Liquid Glass) + Metal + Quick Look
│   ├── android-googlebook/     # Native Kotlin + Compose Material 3 Expressive + Adaptive + Vulkan JNI
│   └── chromeos-pwa/           # Offline-first TypeScript/Vite PWA + WASM SIMD Worker + OPFS WAL + Firebase
├── firebase/                   # firebase.json (COOP/COEP), firestore.rules, firestore.indexes.json, storage.rules
├── stencils/                   # Deterministic IP-clean OpenXML .vssx stencil packs
├── tests/                      # End-to-end Firebase Hosting / Offline / Security Rules verification suite
└── .agents/rules/              # Architectural invariants & platform HIG enforcement contracts
```

### 6-Crate Rust Engine (`crates/*`)

| Crate | Responsibilities |
| :--- | :--- |
| **`vexio-shapesheet`** | Canonical Visio unit system (`in`, `rad`, `days` + display units `mm`, `cm`, `m`, `ft`, `pt`, `pica`, `deg`, `%`), recursive-descent formula parser, 220+ ShapeSheet builtins, `GUARD(expr)` / `SETF` semantics, cross-shape/cross-page references (`Sheet.N!Cell`, `Pages[Page]!Sheet.N!Cell`, `TheDoc!Cell`, `ThePage!Cell`), and reactive topological DAG (`DEPENDSON`, `GRAVITY`, `INTERSECTX/Y`, `PAR`, `PNT`, `LOCTOPAR`, `PARTOLOC`). |
| **`vexio-vsdx`** | Lossless OpenXML OPC reader/writer for `.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, and `.vstm` packages, preserving unknown XML nodes, custom relationships, media streams, themes/variants, masters, background pages, headers/footers, and `vbaProject.bin` across round-trip edits. |
| **`vexio-vsd-legacy`** | Legacy OLE2 Compound File Binary (`.vsd`, `.vss`, `.vst`) stream parser/encoder plus DatadiagramML XML (`.vdx`) and Visio Web Drawing (`.vdw`) codecs mapped losslessly into the unified ShapeSheet IR. |
| **`vexio-cad`** | 100% offline `.dwg` and `.dxf` parser and `.dxf` exporter with CAD layer visibility/lock/color mapping, architectural scale locking, and lossless CAD-to-ShapeSheet geometry conversion. |
| **`vexio-render`** | Multi-backend `wgpu` renderer (`DX12`, `Metal`, `Vulkan`, `WebGPU`/`WebGL2`), `lyon` vector stroke/fill/line-jump tessellation, rich multi-run text layout, and `rstar` R*-tree spatial indexing for 50,000+ shape viewports. |
| **`vexio-core`** | Transactional command journal & Undo/Redo WAL, A* orthogonal/curved/straight dynamic connector routing with obstacle avoidance & configurable line jumps, containers/lists/swimlanes, auto-layouts, **BPMN 2.0 / UML 2.5 / SharePoint** structural validators, two-way Data Linking (`.xlsx`, `.csv`, SQLite, JSON, REST), Data Graphics, Legends, BOM Shape Reports, Markup Ink, sandboxed VBA-compatible Automation engine, vector-accurate exports (`PDF/A`, `SVG`, `EPS`, `DXF`, `HTML`, raster, multi-page tiled printing with registration marks), built-in `.vssx` stencil packs, and platform FFI surfaces. |

### 4 Native & Offline-First Platform Shells (`apps/*` + `firebase/`)

| Target | Directory | UI & Graphics Stack | Packaging & Deployment |
| :--- | :--- | :--- | :--- |
| **Windows 11 Workstation** | `apps/windows` | WinUI 3 / Windows App SDK 1.6 (`C++20` / `CppWinRT`), Mica Alt chrome, contextual ribbon, ShapeSheet docked editor, `ISwapChainPanelNative` **DirectX 12** swapchain | `x64` & `ARM64` signed **MSIX** |
| **macOS 15+ Workstation** | `apps/macos` | **Swift 6** + SwiftUI/AppKit (**Apple Liquid Glass** HIG), `CAMetalLayer` **Metal** viewport, Continuity/Handoff, native **`VexioQuickLookExtension`** (`.vsdx`, `.vssx`, `.vstx`, `.vsd`) | Universal2 (`arm64` + `x86_64`) **`.dmg`** |
| **Android & Googlebook OS** | `apps/android-googlebook` | **100% Native Kotlin** + Jetpack Compose **Material 3 Expressive** + **Compose Adaptive** multi-pane scaffolds + **Navigation 3** + USI low-latency stylus prediction + **Vulkan** NDK | Multi-ABI (`arm64-v8a`, `x86_64`) **APK / AAB** |
| **ChromeOS Workstation PWA** | `apps/chromeos-pwa` + `firebase/` | TypeScript + Vite, Window Controls Overlay, ChromeOS `file_handlers` + File System Access API, OPFS `FileSystemSyncAccessHandle` WAL, Dedicated Worker **Rust WASM SIMD** + **WebGPU**, modular **Firebase SDK v11** (`Auth`, `Firestore` Multi-Tab IndexedDB Offline Persistence, `Storage`) | **Firebase Hosting** (`COOP`/`COEP` enabled) |

---

## FFI Surface Contracts

Platform UI toolkits never leak into `crates/*`. All host interaction crosses strictly bounded FFI modules under `crates/vexio-core/src/ffi/`:

1. **C-ABI (`ffi::c_abi` & `crates/vexio-core/include/vexio.h`):**
   - Consumed directly by Windows C++20 (`apps/windows/src/VexioInterop.h`) and Swift 6 (`apps/macos/Sources/VexioCoreFFI`).
   - Exposes opaque handles (`VexioEngine`, `VexioDoc`, `VexioViewport`), status codes (`VexioStatus`), borrowed input slices (`VexioByteSlice`), and Rust-owned return buffers (`VexioOwnedBuffer` freed via `vexio_buffer_free`).
   - Attaches native GPU surfaces via `vexio_viewport_create_dx12_swapchain` (`IDXGISwapChain3` / `ISwapChainPanelNative`) and `vexio_viewport_create_metal_layer` (`CAMetalLayer*`).
   - Every `extern "C"` function wraps execution in `std::panic::catch_unwind`, translating panics into `VEXIO_ERR_INTERNAL_PANIC` without unwinding across foreign stack frames.
2. **Android NDK / JNI (`ffi::jni_bridge`):**
   - Compiled via `cargo-ndk` into `libvexio_core.so` / `libvexio_jni.so` (`arm64-v8a` and `x86_64`) and loaded by `com.vexio.core.NativeBridge`.
   - Provides panic-isolated `Java_com_vexio_core_NativeBridge_*` entry points for document open/save, ShapeSheet inspection/mutation, pointer/stylus dispatch, rule validation, and Vulkan `ANativeWindow` surface rendering.
3. **WebAssembly SIMD (`ffi::wasm_api`):**
   - Compiled for `wasm32-unknown-unknown` with `RUSTFLAGS="-C target-feature=+simd128"` and bound via `wasm-bindgen` (`WasmVexioSession`).
   - Executes off the main thread inside `apps/chromeos-pwa/src/workers/engine.worker.ts` alongside `opfs-journal.worker.ts` (`FileSystemSyncAccessHandle` crash-safe WAL) and `OffscreenCanvas` WebGPU/2D presentation.

---

## Build, Test & Verification Commands

### 1. Rust Core Workspace Gate
Zero warnings and 100% passing unit, formula, CAD, routing, validation, and OpenXML/OLE2 round-trip tests are mandatory:
```bash
RUSTFLAGS="-D warnings" cargo test --workspace
```

### 2. Regenerate Deterministic Built-In `.vssx` Stencil Packs (12 Domain Libraries)
Exports all 12 IP-clean `.vssx` stencil packs (`basic_flowchart`, `bpmn_2_0`, `uml_2_5`, `network_rack_equipment`, `floor_plan_p_id_vsm`, `cloud_architecture_aws_gcp_azure`, `electrical_electronic_ieee_iec`, `ui_application_wireframes`, `organization_chart`, `project_schedule_gantt_timeline`, `value_stream_mapping_lean_vsm`, `database_crow_s_foot_idef1x`) to `stencils/` and `apps/chromeos-pwa/public/stencils/`:
```bash
cargo run -p vexio-core --example export_builtin_stencils
```

### 3. 50,000-Shape 120 FPS & Multi-Format Corpus Regression Suite
Runs the 50,000-shape R\*-tree viewport/LOD frame-budget benchmark (`< 8.33 ms`) alongside lossless round-trip verification across `.vsdx`, `.vsd` (`v5`/`v11`), `.vdx`, `.dwg` (`AC1015` & `AC1032` R2018/R2024 Reed-Solomon + LZ77), `.dxf`, `.emf`, `.sqlite`, `.pptx`, `.docx`, Power Automate, Power BI, and 3-peer HLC CRDT convergence:
```bash
cargo test -p vexio-core --test corpus_and_50k_bench --release
```

### 4. Rust WASM SIMD + Code-Split ChromeOS Offline-First PWA Build
```bash
rustup target add wasm32-unknown-unknown

RUSTFLAGS="-C target-feature=+simd128" \
  cargo build --target wasm32-unknown-unknown --release -p vexio-core

wasm-bindgen \
  --target web \
  --out-dir apps/chromeos-pwa/public/wasm \
  target/wasm32-unknown-unknown/release/vexio_core.wasm

npm --prefix apps/chromeos-pwa ci
npm --prefix apps/chromeos-pwa run build
```

### 5. Local Firebase Emulator Suite & 6-Stage Offline Security Verification
Validates `COOP: same-origin` / `COEP: require-corp` headers, `application/wasm` MIME + immutable caching, `< 500 KB` code-split JS chunks, ChromeOS `window-controls-overlay` & `file_handlers` manifest, 100% unauthenticated offline WASM SIMD Plan 2 execution, HLC CRDT convergence, and Firestore/Auth per-UID security rules:
```bash
cd firebase
npm ci
./node_modules/.bin/firebase emulators:exec \
  --project demo-vexio-chromeos \
  --only auth,firestore,hosting,storage \
  "node ../tests/verify_offline_firebase_pwa.mjs"
```

### 6. Live Production Firebase `.web.app` Provisioning & Auto-Deploy
Link a live Firebase project and configure GitHub Actions repository variables (`FIREBASE_PROJECT_ID`, `VITE_FIREBASE_*`) + `FIREBASE_SERVICE_ACCOUNT_VEXIO_CHROMEOS_SUITE` secret in one step:
```bash
./firebase/scripts/provision-firebase-ci.sh <your-firebase-project-id> petrosvassiliou/vexio
```

### 7. Native Workstation Builds (`v0.1.0`)
- **Windows (`apps/windows` — WinUI 3 + DirectX 12 + MSIX):**
  ```powershell
  cargo build --release -p vexio-core --target x86_64-pc-windows-msvc
  cmake -S apps/windows -B apps/windows/build -G "Visual Studio 17 2022" -A x64 `
    -DVEXIO_AUTO_BUILD_RUST_CORE=OFF `
    -DVEXIO_RUST_TARGET_DIR="$PWD/target/x86_64-pc-windows-msvc"
  cmake --build apps/windows/build --config Release
  ```
- **macOS (`apps/macos` — Swift 6 + Liquid Glass + Metal + Quick Look):**
  ```bash
  cargo build --release -p vexio-core --target aarch64-apple-darwin
  cargo build --release -p vexio-core --target x86_64-apple-darwin
  mkdir -p target/universal/release
  lipo -create -output target/universal/release/libvexio_core.a \
    target/aarch64-apple-darwin/release/libvexio_core.a \
    target/x86_64-apple-darwin/release/libvexio_core.a
  swift build --package-path apps/macos -c release --arch arm64 --arch x86_64
  ```
- **Android & Googlebook OS (`apps/android-googlebook` — Kotlin + Compose M3E + Vulkan NDK):**
  ```bash
  cargo ndk --platform 29 -t arm64-v8a -t x86_64 \
    -o apps/android-googlebook/app/src/main/jniLibs \
    build --release -p vexio-core
  gradle -p apps/android-googlebook assembleRelease bundleRelease
  ```

---

## CI/CD Pipelines (`.github/workflows/`)

| Workflow | Trigger | Runner | Pipeline Scope |
| :--- | :--- | :--- | :--- |
| [`ci-rust-wasm.yml`](.github/workflows/ci-rust-wasm.yml) | `push`, `pull_request` | `ubuntu-latest` | `RUSTFLAGS="-D warnings" cargo test --workspace` (`54` suites + 50k benchmark), `.vssx` stencil export, `wasm32` `+simd128` build, and code-split ChromeOS PWA bundle |
| [`release-windows-msix.yml`](.github/workflows/release-windows-msix.yml) | `v*` tags (`v0.1.0`), `workflow_dispatch` | `windows-latest` | `x86_64` & `aarch64` MSVC `vexio-core`, Windows App SDK 1.6 + DirectX 12 CMake build, and MSIX GitHub Release packaging |
| [`release-macos-dmg.yml`](.github/workflows/release-macos-dmg.yml) | `v*` tags (`v0.1.0`), `workflow_dispatch` | `macos-15` | Universal2 `lipo` `libvexio_core.a`, Swift 6 `VexioMacApp` + `VexioQuickLookExtension`, and `.dmg` GitHub Release packaging |
| [`release-android-googlebook.yml`](.github/workflows/release-android-googlebook.yml) | `v*` tags (`v0.1.0`), `workflow_dispatch` | `ubuntu-latest` | Android NDK + `cargo-ndk` (`arm64-v8a`, `x86_64`) `libvexio_core.so` / `libvexio_jni.so`, Gradle `assembleRelease` & `bundleRelease` APK/AAB |
| [`deploy-firebase-chromeos-pwa.yml`](.github/workflows/deploy-firebase-chromeos-pwa.yml) | `main` push, `v*` tags, `workflow_dispatch` | `ubuntu-latest` | WASM SIMD + 12 `.vssx` stencils + 6-stage Firebase Emulator verification + Release `.zip` + optional live Firebase Hosting (`COOP`/`COEP`) & Rules deployment |

