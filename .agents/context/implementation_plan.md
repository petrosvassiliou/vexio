# Vexio Workstation UX & Visio Ribbon Overhaul Plan (macOS + Cross-Platform Parity)

## Goal Description
Deliver a cohesive, 1:1 Microsoft Visio-grade workstation upgrade addressing all **14 user review points** starting from the macOS native app (`apps/macos`) and propagating every applicable capability across **Windows (`apps/windows`)**, **Android & Googlebook OS (`apps/android-googlebook`)**, **ChromeOS Offline-First PWA (`apps/chromeos-pwa`)**, and the shared **Rust core (`crates/*`)**:

1. **In-App Start / New & Open File Dialog** on launch (replacing macOS's external OS Finder modal).
2. **Double-Click Any Placed Stencil/Shape** to edit its text label inline right on the canvas.
3. **Standard Rich Text Formatting** (Font family, size, grow/shrink, Bold, Italic, Underline, Strikethrough, Text Color, Highlight/Background, Horizontal & Vertical Alignment, Bullets).
4. **Functional `Text` Tool (`[A]`)** creating clean text blocks and **immediately entering inline text editing** instead of placing an uneditable static `"TEXT"` box (`image.png`).
5. **Rotate & Flip Controls for All Stencils** (`Rotate Left 90° ↺`, `Rotate Right 90° ↻`, `Flip Horizontal`, `Flip Vertical`, custom angle, interactive rotation handle).
6. **ISO Metric Page Sizes (`A4`, `A3`, `A5`, `A2`, `A1`, `A0`, `Letter`, `Legal`, `Tabloid`) & Metric / OS-Standard Default Units** (`mm`/`cm` rulers, inspector, and grid when OS uses Metric or by default).
7. **6-Language Localization (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`/`EL`)** with OS auto-detection and live in-app switcher.
8. **Automatic Updates + Manual "Check for Updates…"** backed by GitHub Releases (`petrosvassiliou/vexio`) on desktop/mobile and Service Worker delta updates on ChromeOS PWA.
9. **Auto-Return Cursor to `Select` (`Pointer`) Mode** immediately after inserting a shape, connector, or text box.
10. **Import Visio Stencils & Compatible Files** (`.vssx`, `.vss`, `.vssm`, `.vsdx`, `.vstx`, `.vdx`) into the Stencil Navigator via button, menu, or drag-and-drop.
11. **Double-Click Top-Left File Name** (`image_2.png`) to select and rename the document inline.
12. **Chrome-Style Hover-to-Switch Across Open Top Menus** (`image_3.png`) so moving the pointer across sibling menus switches instantly without extra clicks.
13. **Clean Up the Cluttered Middle Top Bar** (`image_4.png` — recommendation below).
14. **Classic Collapsible Visio Ribbon Interface** (`Home`, `Insert`, `Design`, `Data`, `Process`, `Review`, `View`, `Developer`) housing text formatting, tools, shape styles, rotation/arrangement, and page setup.

---

## User Review Required

> [!IMPORTANT]
> ### Recommendation for Screenshot 4 (`Templates | Shape Ops | Routing | Auto-Layout | Rotate | Validate`) & Missing Visio Ribbon
> Currently on macOS (`image_4.png`), six unrelated workflow dropdowns sit in the middle of the window titlebar, crowding the header while leaving **zero room for Font/Paragraph formatting** (`Home` Ribbon).
>
> **Recommended Redistribution (matching original Microsoft Visio):**
> ```mermaid
> flowchart TD
>     Old["Current Middle Top Bar (Screenshot 4)<br/>Templates • Shape Ops • Routing • Auto-Layout • Rotate • Validate"]
>     TopBar["New Clean Window Titlebar<br/>Left: App Icon + File Button + Undo/Redo/Save + Editable Filename<br/>Center: Ribbon Tab Strip (Home | Insert | Design | Data | Process | Review | View | Developer)<br/>Right: Language (EN/DE/IT/FR/ES/GR) + Update Status + Units (mm) + Export + Panel Toggles"]
>     Backstage["In-App 'File' Backstage Dialog<br/>← Templates, New (A4/A3/Letter), Open, Recent, Import Stencil, Updates"]
>     HomeRib["'Home' Ribbon (Default Open)<br/>Clipboard • Font & Size & Color • Paragraph Align & Bullets • Tools • Shape Styles • Arrange (← Rotate 90° ↺/↻ & Flip)"]
>     DesignRib["'Design' Ribbon<br/>Page Size (A4, A3, A5...) • Orientation • Metric/OS Units • Themes • ← Connector Routing • ← Auto-Layout"]
>     ProcessRib["'Process' & 'Developer' Ribbons<br/>Process: ← Validate (BPMN 2.0 / UML 2.5) & Subprocess<br/>Developer: ← Shape Ops (Union/Combine/Fragment/Subtract) & ShapeSheet"]
>
>     Old --> TopBar
>     Old --> Backstage
>     Old --> HomeRib
>     Old --> DesignRib
>     Old --> ProcessRib
> ```
> - **Result:** The middle of the top bar becomes clean and balanced, `Templates` lives in the new **In-App `File` Start/Backstage Dialog**, `Rotate` + **Text Formatting** are always 1 click away on the **`Home` Ribbon**, `Page Size (A4/A3)` + `Routing` + `Auto-Layout` live together on **`Design`**, `Validate` lives on **`Process`**, and boolean `Shape Ops` lives on **`Developer`**.

> [!NOTE]
> ### Key Behavioral Decisions to Confirm (Defaults Proposed Below)
> 1. **Startup Experience (Remark #1):** Launching Vexio opens the main app window directly (never an external macOS Finder `NSOpenPanel`) showing the **In-App Start / Open Hub** overlay (Blank ISO `A4` / `A3` / `Letter`, Starter Templates, Recent Files, and **"Open File…"**), with a *"One-click Close / Start Blank A4"* action and a *"Start directly in blank drawing next time"* checkbox. Opening a `.vsdx`/`.vsd`/`.dwg` file from Finder/Explorer bypasses the hub and opens the diagram directly.
> 2. **Default Units & Page Size (Remark #6):** Query the OS locale measurement system on startup (`Locale.measurementSystem` / Win32 `LOCALE_IMEASURE` / Android `LocaleData` / `Intl.Locale`); default to **Metric (`mm`, `A4 297 × 210 mm Landscape`)** whenever the OS uses Metric (or UK/EU/GR/international) and fallback to **Metric (`mm`, `A4`)** if unspecified, while preserving canonical internal inches inside `.vsdx` ShapeSheets for 100% lossless Microsoft Visio compatibility.
> 3. **Post-Insertion Tool Reset (Remark #9):** Placing any shape, text box, or connector automatically switches the active tool back to **`Select` (`Pointer`)** on mouse-up. (Holding `Shift` while drawing optionally keeps the tool active for rapid multi-stamping, matching Visio/CAD conventions.)

---

## Proposed Changes

### 1. Rust Core Engine (`crates/vexio-shapesheet`, `crates/vexio-render`, `crates/vexio-core`)

#### [MODIFY] [dag.rs](../../crates/vexio-shapesheet/src/dag.rs) & [sections.rs](../../crates/vexio-shapesheet/src/sections.rs)
- Extend `EvaluatedShapeVisual` to evaluate full Visio text & paragraph ShapeSheet cells alongside `font_size_pt` and `text_color`:
  - `font_family: String` (`Char.Font`)
  - `char_style: u8` (`Char.Style` bitmask: `1 = Bold`, `2 = Italic`, `4 = Underline`, `8 = Strikethrough`)
  - `horiz_align: u8` (`Para.HorzAlign`: `0 = Left`, `1 = Center`, `2 = Right`, `3 = Justify`)
  - `vert_align: u8` (`VerticalAlign`: `0 = Top`, `1 = Middle`, `2 = Bottom`)
  - `bullet_style: u8` (`Para.Bullet`: `0 = None`, `1 = Disc`, `2 = Numbered`)
  - `text_bg_color: Option<RgbaColor>` (`TextBkgnd` / `TextBkgndTrans`)

#### [MODIFY] [tessellate.rs](../../crates/vexio-render/src/tessellate.rs) & [typography.rs](../../crates/vexio-render/src/typography.rs)
- Pass `EvaluatedShapeVisual`'s `char_style` (bold stroke weight, italic shear, underline, strikethrough), `horiz_align`, `vert_align`, `bullet_style`, and optional `text_bg_color` highlight quad directly into `tessellate_rich_text_in_box` and `compute_visual_content_hash` so live Metal / DX12 / Vulkan / WebGPU canvases and PNG/JPG exports render formatted text identically.
- Render crisp dashed/subtle selection outline when a borderless `Text` shape (`NoFill=1`, `NoLine=1`) is hovered or selected.

#### [MODIFY] [export_print.rs](../../crates/vexio-core/src/export_print.rs) & [workspace.rs](../../crates/vexio-core/src/workspace.rs)
- Update `draw_primitive_2d(..., "text", ...)` so the `Text` tool creates a true Visio **Text-Only Shape** (`NoFill = true`, `NoLine = true`, default `"Text"`, minimum sensible click-size `2.0 in × 0.45 in` if single-clicked without dragging).
- Add `DocumentSession::set_shape_text_style(...)` (journaled Undo/Redo batch updating `Char.Font`, `Char.Size`, `Char.Style`, `Char.Color`, `Para.HorzAlign`, `VerticalAlign`, `Para.Bullet`, `TextBkgnd`).
- Add `DocumentSession::rotate_or_flip_shapes(page_id, &shape_ids, delta_deg, flip_x, flip_y)` (journaled Undo/Redo batch supporting `±90°` rotation and horizontal/vertical mirroring on **any** stencil or shape, ensuring no built-in or imported stencil sets `LockRotate = 1`).
- Add `DocumentSession::set_page_setup(page_id, preset_or_dims, orientation, display_unit)` supporting ISO `A0`–`A5` (`A4 = 210×297 mm`, `A3 = 297×420 mm`, `A5 = 148×210 mm`), `Letter`, `Legal`, `Tabloid`, `Portrait`/`Landscape`, and setting `PageWidth`, `PageHeight`, `PageScale`, and `DrawingScale` with full Undo/Redo journaling.
- Emit `font-weight`, `font-style`, `text-decoration`, `text-anchor`, and vertical offset in SVG & PDF/A exports (`export_print.rs`) and round-trip `<Cell>` formatting in `crates/vexio-vsdx`.

#### [MODIFY] [vexio.h](../../crates/vexio-core/include/vexio.h), [c_abi.rs](../../crates/vexio-core/src/ffi/c_abi.rs), [jni_bridge.rs](../../crates/vexio-core/src/ffi/jni_bridge.rs), & [wasm_api.rs](../../crates/vexio-core/src/ffi/wasm_api.rs)
- **Auto-Reset Tool After Shape Creation (`c_abi.rs`):** In `VexioViewport::dispatch_pointer_internal` (`ev.kind == 2`), whenever `DrawBoxOrLine` or `Freeform` creates a shape (unless `Shift` modifier is held), automatically reset `self.active_tool = NativeCanvasTool::Select` and expose `vexio_viewport_get_active_tool(const VexioViewport* vp)` + `vexio_viewport_pop_created_text_shape(VexioViewport* vp)` so native shells immediately sync their toolbar button to `Pointer` and auto-open inline text editing when a `Text` box was just placed!
- **Inline Label Overlay Geometry & Formatting C-ABI / JNI / WASM:**
  - `vexio_viewport_hit_test_shape(const VexioViewport* vp, const VexioDoc* doc, double x_px, double y_px, uint64_t* out_shape_id)`
  - `vexio_viewport_get_shape_screen_bounds(const VexioViewport* vp, const VexioDoc* doc, uint64_t shape_id, double* out_x_px, double* out_y_px, double* out_w_px, double* out_h_px)`
  - `vexio_doc_set_shape_text_style_json(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids, size_t len, const char* style_json_utf8)`
  - `vexio_doc_rotate_flip_shapes(VexioDoc* doc, uint32_t page_id, const uint64_t* shape_ids, size_t len, double delta_angle_deg, bool toggle_flip_x, bool toggle_flip_y)`
  - `vexio_doc_set_page_setup(VexioDoc* doc, uint32_t page_id, double width_in, double height_in, const char* unit_slug_utf8)`
  - `vexio_doc_set_title(VexioDoc* doc, const char* title_utf8)`

---

### 2. Shared 6-Language Localization (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`) & Auto-Updater Specification
- Implement an identical localization catalog & OS locale resolver across all 4 targets supporting:
  - `en` (**English**), `de` (**Deutsch**), `it` (**Italiano**), `fr` (**Français**), `es` (**Español**), `el` (**Ελληνικά / GR**).
- Implement an async, offline-safe **Release Update Checker** querying `https://api.github.com/repos/petrosvassiliou/vexio/releases` (handling both stable releases and pre-releases like `v0.1.0-alpha`), comparing semantic versions, persisting `"Check for updates automatically"` + `"Last checked"` timestamp, and providing both silent startup banner notifications when a newer release exists and an interactive **"Check for Updates…"** dialog with release notes and direct platform installer download (`DMG` / `MSIX` / `APK`) or PWA Service Worker activation.

---

### 3. macOS Native App (`apps/macos` — Swift 6 + AppKit/SwiftUI Liquid Glass + Metal)

#### [NEW] [VexioLocalization.swift](../../apps/macos/Sources/VexioMacApp/VexioLocalization.swift) & [VexioUpdateManager.swift](../../apps/macos/Sources/VexioMacApp/VexioUpdateManager.swift)
- `VexioLocalization`: `@MainActor ObservableObject` localization store (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`) seeded from `Locale.preferredLanguages` + `Locale.current.measurementSystem` (defaulting to Metric `mm` and `A4` page size unless OS measurement system is `.us`).
- `VexioUpdateManager`: Native `URLSession` GitHub Releases updater with automatic background startup check toggle + manual `Check for Updates…` sheet showing version comparison, changelog, and one-click `arm64`/`x86_64` `.dmg` download & open.

#### [NEW] [VexioBackstageStartView.swift](../../apps/macos/Sources/VexioMacApp/VexioBackstageStartView.swift) & [VexioRibbonView.swift](../../apps/macos/Sources/VexioMacApp/VexioRibbonView.swift)
- **`VexioBackstageStartView` (Fixes Remark #1):** Full Liquid Glass in-window Start & `File` Backstage view with:
  - **New Diagram** presets (`Blank A4 Metric`, `Blank A3 Metric`, `Blank US Letter`, `BPMN 2.0`, `Network & Rack`, `Cloud Architecture`, `Org Chart`, `Floor Plan`),
  - **Recent Files** list (`NSDocumentController.shared.recentDocumentURLs`) + in-app **"Browse / Open File…"** & drag-and-drop `.vsdx/.vssx/.vsd/.dwg` zone,
  - **Import Visio Stencil (`.vssx` / `.vss`)**, **Language (`EN/DE/IT/FR/ES/GR`)**, **Default Units (`OS Standard / mm / cm / in`)**, and **Updates** section.
- **`VexioRibbonView` + `HoverSwitchMenuStrip` (Fixes Remarks #3, #5, #6, #12, #13, #14):**
  - Clean Top Bar + Chrome-style **hover-switching** across sibling dropdown menus (`HoverSwitchMenuCoordinator`: once any menu is open, pointer hover over another menu pill immediately switches the open popover).
  - Collapsible multi-tab **Visio Ribbon** (`Home`, `Insert`, `Design`, `Data`, `Process`, `Review`, `View`, `Developer`) with Liquid Glass styling:
    - `Home`: Clipboard | **Font Family, Font Size, `A▲`/`A▼`, `B`, `I`, `U`, `S`, Text Color, Text Highlight** | **Align Left/Center/Right, Top/Middle/Bottom, Bullets** | Tools | Shape Fill/Line | **Rotate `↺ 90°`, Rotate `↻ 90°`, Flip H, Flip V**, Order.
    - `Insert`: New Page, **Import Visio Stencil (`.vssx/.vss`)**, Text Box, Container, CAD.
    - `Design`: **Page Size (`A4`, `A3`, `A5`, `A2`, `Letter`, `Legal`)**, **Portrait/Landscape**, **Units (`mm`, `cm`, `m`, `in`)**, Themes, Connector Routing, Auto-Layout.
    - `Process` / `Review` / `Developer`: BPMN/UML Validation, Diagram Compare, Boolean **Shape Ops**, ShapeSheet toggle.

#### [MODIFY] [VexioMacApp.swift](../../apps/macos/Sources/VexioMacApp/VexioMacApp.swift), [VexioWorkspaceView.swift](../../apps/macos/Sources/VexioMacApp/VexioWorkspaceView.swift), [MetalDiagramCanvas.swift](../../apps/macos/Sources/VexioMacApp/MetalDiagramCanvas.swift), & [CoreBridge.swift](../../apps/macos/Sources/VexioMacApp/CoreBridge.swift)
- **Eliminate External OS Launch Dialog (`VexioMacApp.swift`):** Replace root `DocumentGroup` launch behavior with a native workstation `WindowGroup` + `AppDelegate` (`applicationShouldOpenUntitledFile -> false`, handling `application(_:open:)` and `⌘N` / `⌘O` / `⌘S` cleanly) so Vexio **always** launches directly into its own window with the in-app Start/Backstage view!
- **Double-Click Stencil Label Editing & Immediate `Text` Tool Editing (`MetalDiagramCanvas.swift` + `VexioWorkspaceView.swift`):**
  - Detect `event.clickCount == 2` in `MetalDiagramCanvas.mouseDown(with:)` to hit-test the clicked stencil/shape and open a floating **Inline Canvas Text Editor** positioned directly over the shape's projected screen bounds with existing label text pre-selected.
  - On `mouseUp(with:)`, synchronize `session.activeTool` from `vexio_viewport_get_active_tool` (returning cursor to `.pointer` after inserting any shape!), and if a new `Text` box was just created, immediately mount & focus the Inline Canvas Text Editor so typing works right away!
- **Double-Click Top-Left File Name to Rename (`VexioWorkspaceView.swift`):** Double-clicking `Untitled Diagram` (`image_2.png`) swaps in an auto-focused `SelectAllTextField` (`NSTextField` wrapper calling `selectText(nil)`) that commits on `Return` or focus loss and renames the document/file.
- **Import Visio Stencils (`VexioWorkspaceView.swift` + `CoreBridge.swift`):** Add `+ Import Stencil…` (`UTType` `.vssx`, `.vss`, `.vssm`, `.vsdx`, `.vstx`, `.vdx`) button + drag-and-drop on `VexioStencilNavigatorSidebar` wired to `vexio_doc_import_stencil_bytes`, plus reset `activeTool = .pointer` after stencil insertion.
- **Metric Rulers & Inspector Formatting (`CoreBridge.swift`):** Format Inspector `PinX`, `PinY`, `Width`, `Height` and page dimensions in `mm` / `cm` / `in` according to active `MeasurementUnitPreference`.

---

### 4. Windows Native App (`apps/windows` — WinUI 3 C++20 + DirectX 12)

#### [MODIFY] [MainWindow.xaml](../../apps/windows/src/MainWindow.xaml) & [MainWindow.xaml.cpp](../../apps/windows/src/MainWindow.xaml.cpp)
- Upgrade `File` Backstage flyout to a full **In-App Start / File Backstage Overlay** (New Blank `A4`/`A3`/`Letter`, Templates, Open, Recent, Import Stencil, Language `EN/DE/IT/FR/ES/GR`, Check for Updates).
- Add **Font & Paragraph Formatting** groups (`Font`, `Size`, `B/I/U/S`, `Text Color`, `Align L/C/R`, `Top/Mid/Bot`, `Bullets`) and **Rotate `90° ↺` / `90° ↻` / `Flip H` / `Flip V`** buttons to the `Home` Ribbon; add **ISO Page Presets (`A4`, `A3`, `A5`, `Letter`) + `Metric (mm)` / OS-Locale (`GetLocaleInfoEx` `LOCALE_IMEASURE`) Units** to `Design` Ribbon.
- Add **Inline Canvas `TextBox` Overlay** over `Dx12SwapChainPanel` triggered both on **double-clicking any placed stencil/shape** and **immediately after placing a `Text` tool object**, and auto-reset `PointerToolToggle` (`Select`) after inserting any shape.
- Wire **`+ Import Stencil (.vssx/.vss)`** in the left `Shapes` dock via `FileOpenPicker` -> `vexio_doc_import_stencil_bytes`.
- Make top-bar `DocumentTitleText` double-clickable to select all & rename inline, add `PointerEntered` **hover-to-switch** across open menu/ribbon dropdowns, and wire WinRT `HttpClient` automatic + manual GitHub Releases update checker & 6-language UI string dictionary.

---

### 5. Android & Googlebook OS Native App (`apps/android-googlebook` — Kotlin + Compose M3E + Vulkan JNI)

#### [MODIFY] [MainActivity.kt](../../apps/android-googlebook/app/src/main/java/com/vexio/MainActivity.kt), [NativeBridge.kt](../../apps/android-googlebook/app/src/main/java/com/vexio/core/NativeBridge.kt), & [GooglebookWorkstationScreen.kt](../../apps/android-googlebook/app/src/main/java/com/vexio/ui/workstation/GooglebookWorkstationScreen.kt)
- Add **In-App Start / File Hub**, collapsible **M3 Expressive Visio Ribbon** (`Home` with rich text formatting & `Rotate 90° ↺/↻` + `Flip`, `Insert` with `.vssx/.vss` SAF Stencil Import, `Design` with `A4`/`A3` ISO Metric page sizes & `LocaleData.getMeasurementSystem` default `mm` units, `Process`, `Developer`).
- Support **double-tap / double-click on canvas shapes** to edit labels inline in a positioned Compose `OutlinedTextField` overlay, **immediate inline text editing** upon placing a `Text` box, and **auto-resetting `activeTool` to `Select`** after shape insertion.
- Support **double-clicking the top-left document title** to select-all & rename inline, **pointer-hover menu switching** on Googlebook desktop trackpad/mouse, **6-language (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`)** Compose localization, and automatic/manual GitHub Releases `.apk` update checks.

---

### 6. ChromeOS Offline-First Firebase PWA (`apps/chromeos-pwa` — TypeScript + Rust WASM SIMD)

#### [MODIFY] [main.ts](../../apps/chromeos-pwa/src/main.ts) & [m3-expressive.css](../../apps/chromeos-pwa/src/styles/m3-expressive.css)
- Align the top header + **Visio Ribbon tabs (`Home`, `Insert`, `Design`, `Data`, `Process`, `Review`, `View`, `Developer`)** with the clean macOS/Windows layout (removing top-bar clutter, adding Chrome-style `pointerenter` **hover-to-switch** across open menus/tabs).
- Ensure **In-App File Backstage**, **double-click stencil inline text editing**, **immediate inline text entry + auto-return to `Pointer` tool** after inserting a `Text` box or shape, **full `Home` Ribbon text formatting** (`Font`, `Size`, `B/I/U/S`, `Color`, `Highlight`, `Align`, `V-Align`, `Bullets`), **Rotate `90° ↺/↻` & Flip buttons**, **ISO `A4`/`A3`/`A5` page presets with Metric (`mm`) OS-aware default**, **double-click top-left filename select-all rename**, **6-language (`EN`, `DE`, `IT`, `FR`, `ES`, `GR`)** dictionary, and **Auto + Manual Check for Updates** (combining Service Worker update detection + GitHub release version check when online, 100% non-blocking offline).

---

## Verification Plan

### Automated Tests
1. **Rust Core Workspace Suite (`-D warnings`):**
   - Run `RUSTFLAGS="-D warnings" cargo test --workspace` verifying:
     - New unit & `.vsdx` round-trip tests for `Char.Font`, `Char.Size`, `Char.Style` (`B/I/U/S`), `Char.Color`, `Para.HorzAlign`, `VerticalAlign`, `Para.Bullet`, and `TextBkgnd`.
     - Unit tests for `Text` tool borderless shape creation, `VexioViewport` post-insertion tool reset to `Select`, screen-bounds projection for inline label editing, `±90°` stencil rotation & `FlipX`/`FlipY` across all built-in stencil categories, and ISO `A4`/`A3`/`A5` Metric page setup (`297 × 210 mm` -> `11.6929 × 8.2677 in` canonical with `mm` scale metadata).
2. **ChromeOS PWA WASM SIMD + Workbox Build & Headless Browser E2E Verification:**
   - Build `wasm32-unknown-unknown` (`+simd128`) and `pnpm --dir apps/chromeos-pwa build` (`0` TypeScript/Vite warnings).
   - Verify offline load, Ribbon text formatting, `Text` tool immediate inline typing + auto-return to `Pointer`, double-click stencil label editing, `Rotate 90°`, `A4` Metric default rulers, `.vssx` stencil import, double-click filename rename, hover-switch menus, and `EN/DE/IT/FR/ES/GR` language switching.
3. **Multi-OS Native Compilation Gates (GitHub Actions Matrix on Push/Approval):**
   - Verify clean compilation across `apps/macos` (`swift build` + `.app`/`.dmg`), `apps/windows` (`cmake` + MSBuild WinUI 3 DX12), `apps/android-googlebook` (`gradle assembleRelease` Kotlin + NDK Vulkan), and `apps/chromeos-pwa` Firebase Hosting deployment.
