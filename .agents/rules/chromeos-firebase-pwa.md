---
trigger: model_decision
description: "Apply when creating, modifying, or reviewing ChromeOS PWA code under apps/chromeos-pwa, Rust WASM bindings, Service Worker / OPFS storage, or Firebase configs under firebase/."
---

# ChromeOS Firebase Offline-First PWA Rules (`apps/chromeos-pwa` & `firebase/`)

1. **Firebase Hosting Only (No Isolated Web App / `.swbn`):**
   - Host and deliver via **Firebase Hosting** (`firebase/firebase.json`). Never package as an Isolated Web App (`.swbn`) or use IWA-only `isolated-app://` schemes.
   - `firebase.json` must serve `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp`, and `Cross-Origin-Resource-Policy: same-origin` on all routes so `self.crossOriginIsolated === true` and `SharedArrayBuffer` multi-threaded Rust WASM SIMD + WebGPU run at peak speed.
   - Serve `**/*.wasm` with `Content-Type: application/wasm` and `Cache-Control: public, max-age=31536000, immutable`.
2. **100% Guaranteed Offline-First Execution (Even Unauthenticated):**
   - App startup, stencil browsing, `.vsdx`/`.vsd`/`.dwg` editing, validation, CAD import, and export must work **100% offline with zero network connectivity and without signing into Firebase Auth**.
   - Precache 100% of the app shell, WASM SIMD binaries, WGSL shaders, validation engines, and core `.vssx` stencil/template packs on Service Worker install (Workbox versioned manifest + background delta updates when online).
   - Use **Origin Private File System (OPFS)** with synchronous `FileSystemSyncAccessHandle` inside a dedicated Web Worker for zero-latency offline swap files, undo/redo WAL journals, crash-proof autosave recovery, and cached user stencil packs.
3. **Modular Firebase SDK v11+ Optional Cloud Sync:**
   - Integrate modular Firebase v11+ (`firebase/app`, `firebase/auth`, `firebase/firestore`, `firebase/storage`).
   - Initialize Cloud Firestore with `persistentLocalCache({ tabManager: persistentMultipleTabManager() })` for IndexedDB multi-tab offline persistence of user preferences, custom stencil metadata, and optional cloud documents.
4. **Native ChromeOS Workstation Integration:**
   - Declare `"display_override": ["window-controls-overlay", "standalone"]` in `manifest.webmanifest`, merging Vexio's Material 3 Expressive document tab strip and Quick Access controls directly into the native ChromeOS window titlebar (`env(titlebar-area-*)`).
   - Register `.vsdx`, `.vsdm`, `.vssx`, `.vssm`, `.vstx`, `.vstm`, `.vsd`, `.vss`, `.vst`, `.vdx`, `.vdw`, `.dwg`, and `.dxf` in `file_handlers` (`window.launchQueue.setConsumer`) paired with the **File System Access API** (`FileSystemFileHandle.createWritable()`) for direct in-place disk open/save from ChromeOS Files.
   - Integrate the **Local Font Access API** (`window.queryLocalFonts`) and **Async Clipboard API** (`navigator.clipboard`) with graceful permission handling.
