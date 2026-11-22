# Vexio `v0.2.1-alpha` — Complete 520+ Googlebook Vector Stencils, `22 dp` `↻` Rotate Button & Unified Expressive Toolbar (`a2c717b`)

> [!IMPORTANT]
> **Release `v0.2.1-alpha`** (`versionCode = 21`) resolves all three Googlebook issues reported on `v0.2.0-alpha`—restoring all **520+ vector stencils across 22 packs**, introducing a high-contrast **`22 dp` circular `↻` Rotate button** that never shrinks on high-DPI screens, and removing the duplicate vertical tool rail—across [**GitHub Release `v0.2.1-alpha`**](https://github.com/petrosvassiliou/vexio/releases/tag/v0.2.1-alpha) and https://vexio-chromeos-suite.web.app.

---

## 1. Multi-OS Release Installers & Live Cloud Deployment (`v0.2.1-alpha`)

| Platform | Architecture | Installer / Live URL | Verified GitHub Actions Run |
| :--- | :--- | :--- | :--- |
| **Android & Googlebook OS Native** (`Compose M3E + Vulkan JNI`, `versionCode = 21`) | `arm64-v8a` + `x86_64` | [`Vexio-Android-Googlebook-v0.2.1-alpha.apk`](https://github.com/petrosvassiliou/vexio/releases/download/v0.2.1-alpha/Vexio-Android-Googlebook-v0.2.1-alpha.apk) / [`.aab`](https://github.com/petrosvassiliou/vexio/releases/download/v0.2.1-alpha/Vexio-Android-Googlebook-v0.2.1-alpha.aab) | [`45080737556`](https://github.com/petrosvassiliou/vexio/actions/runs/45080737556) (**Success**) |
| **ChromeOS Offline-First PWA** | WASM SIMD128 + 22 `.vssx` Packs | https://vexio-chromeos-suite.web.app & [`Vexio-ChromeOS-PWA-v0.2.1-alpha.zip`](https://github.com/petrosvassiliou/vexio/releases/download/v0.2.1-alpha/Vexio-ChromeOS-PWA-v0.2.1-alpha.zip) | [`45080735589`](https://github.com/petrosvassiliou/vexio/actions/runs/45080735589) & [`45080737540`](https://github.com/petrosvassiliou/vexio/actions/runs/45080737540) (**Success**) |
| **macOS 15+ Native** (`Swift 6 Liquid Glass + Metal + Quick Look`) | `Universal2` (`arm64` + `x86_64`) | [`Vexio-v0.2.1-alpha-Universal2.dmg`](https://github.com/petrosvassiliou/vexio/releases/download/v0.2.1-alpha/Vexio-v0.2.1-alpha-Universal2.dmg) | [`45080737576`](https://github.com/petrosvassiliou/vexio/actions/runs/45080737576) (**Success**) |
| **Windows 11 Native** (`WinUI 3 + DirectX 12 + DirectWrite`) | `x64` & `ARM64` | [`Vexio-Windows-v0.2.1-alpha-x64.msix`](https://github.com/petrosvassiliou/vexio/releases/download/v0.2.1-alpha/Vexio-Windows-v0.2.1-alpha-x64.msix) / [`arm64.msix`](https://github.com/petrosvassiliou/vexio/releases/download/v0.2.1-alpha/Vexio-Windows-v0.2.1-alpha-arm64.msix) | [`45080737536`](https://github.com/petrosvassiliou/vexio/actions/runs/45080737536) (**Success**) |

---

## 2. Root Causes & Engineering Fixes in `v0.2.1-alpha`

1. **Complete 520+ Vector Stencils Across All 22 Packs ([NativeBridge.kt](../../apps/android-googlebook/app/src/main/java/com/vexio/core/NativeBridge.kt#L1486-L1685), [GooglebookWorkstationScreen.kt](../../apps/android-googlebook/app/src/main/java/com/vexio/ui/workstation/GooglebookWorkstationScreen.kt#L2875-L2992)):**
   - **Root Cause:** In Kotlin, class properties and `init` blocks execute strictly top-to-bottom. `init { initializeEngineAndBlankDocument(...) }` ran *before* `private val lenientJson = Json { ignoreUnknownKeys = true }` was initialized 103 lines lower, throwing a `NullPointerException` inside `loadDynamicStencilsFromNative()` that was caught by `runCatching`, leaving `stencilMasters` on its 39-item fallback list instead of the 520+ JNI catalog.
   - **Fix:** Moved `lenientJson`, `viewportWidthPx`, `viewportHeightPx`, `directViewportRgbaBuffer`, and `syntheticGroups` above `init`, extracted testable `applyNativeStencilsJson(rawJson)`, updated canonical default `expandedCategories`, and upgraded `parseStencilSvgDoc` to parse dynamic `viewBox` dimensions plus `<path>`, `<polygon>`, `<polyline>`, `<rect>`, `<circle>`, `<ellipse>`, and `<line>` vector primitives.
2. **Unmistakable `22 dp` Circular `↻` Rotate Button Across All OSes ([pipeline.rs](../../crates/vexio-render/src/pipeline.rs#L388-L450), [tessellate.rs](../../crates/vexio-render/src/tessellate.rs#L1976-L2144), [c_abi.rs](../../crates/vexio-core/src/ffi/c_abi.rs#L221-L272), [jni_bridge.rs](../../crates/vexio-core/src/ffi/jni_bridge.rs#L364-L372), [main.ts](../../apps/chromeos-pwa/src/main.ts#L2890-L2895)):**
   - **Root Cause:** `build_page_scene_mesh_with_overlay` passed `camera.effective_ppi()` (`zoom_ppi * dpi_scale`) instead of logical `camera.zoom_ppi`, shrinking a `4 px` empty dot down to `1.6 dp` (`3.2 dp` diameter) on `2.5x` high-DPI Googlebook screens with no interior rotate icon, mismatched hit-testing offsets, and missing selection/touch refreshes on `ACTION_DOWN` / `ACTION_MOVE`.
   - **Fix:** Switched interactive UI handle/frame/overlay/ruler tessellation to `ui_ppi = camera.zoom_ppi.max(1.0)`, upgraded `tessellate_selection_handles` (and `.rotation-knob` in ChromeOS PWA) to render a `22 dp` white/blue circular button at `28 dp` stem offset containing a crisp `#1D4ED8` clockwise `↻` arc + directional arrowhead (`30 dp` hit target in `hit_test_selection_grips`), synced `nativeSetSelectionIds` on surface attach & stencil drop, and enabled full `render_page_software_rgba_with_overlay` refreshes across `ACTION_DOWN`, `ACTION_MOVE`, and `ACTION_UP`.
3. **Single Unified Expressive Toolbar ([GooglebookWorkstationScreen.kt](../../apps/android-googlebook/app/src/main/java/com/vexio/ui/workstation/GooglebookWorkstationScreen.kt#L596-L606)):**
   - Removed the redundant vertical `PersistentDrawingToolRail` so only the single Material 3 Expressive morphing toolbar (`ExpressiveMorphingToolbar`) displays the 16 drawing tools, reclaiming `50 dp` of horizontal canvas space.

---

## 3. Verification Summary

| Verification Gate | Coverage | Result |
| :--- | :--- | :--- |
| **Rust Workspace Suite** ([v020_typography_and_ergonomics.rs](../../crates/vexio-core/tests/v020_typography_and_ergonomics.rs#L515-L607), [c_abi.rs](../../crates/vexio-core/src/ffi/c_abi.rs#L4538-L4637)) | High-DPI (`2.5x`) `22 dp` `↻` Rotate button mesh & RGBA pixel verification, C-ABI hover (`13`) & `buttonState=0` touch-drag rotation, plus all 72 C-ABI functions & `.vsdx` round-trip tests | **`69 / 69` passed (`0` warnings)** |
| **Android & Googlebook JUnit + NDK Release** ([NativeBridgePacketTest.kt](../../apps/android-googlebook/app/src/test/java/com/vexio/core/NativeBridgePacketTest.kt#L670-L704)) | `VexioSessionViewModel` zero-NPE startup init, 22 canonical stencil packs, dynamic JSON stencil loading, JNI signature parity, and `arm64-v8a` + `x86_64` APK/AAB packaging | **Passed in Gradle CI** ([`45080737556`](https://github.com/petrosvassiliou/vexio/actions/runs/45080737556)) |
| **Firebase Emulator + Headless Chromium CDP Smoke** ([verify_offline_firebase_pwa.mjs](../../tests/verify_offline_firebase_pwa.mjs)) | COOP/COEP `SharedArrayBuffer`, WASM SIMD128, 45-entry Workbox precache, live `.rotation-knob svg` `↻` icon verification, and `0.2.1-alpha` update modal | **`6 / 6` + `11 / 11` passed (`0` console errors)** |
