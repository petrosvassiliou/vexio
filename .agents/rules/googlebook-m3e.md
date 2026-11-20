---
trigger: model_decision
description: "Apply when creating, modifying, or reviewing Android and Googlebook OS native code under apps/android-googlebook or JNI / Android NDK Vulkan bindings."
---

# Android & Googlebook OS Native Target Rules (`apps/android-googlebook`)

1. **100% Native Execution:** Compile directly to `arm64-v8a` and `x86_64` native libraries (`libvexio_jni.so`) paired with **100% Kotlin + Jetpack Compose** and **Vulkan NDK** (`ANativeWindow` surface + zero-copy Direct `ByteBuffer` JNI). Never use Android `WebView`, Capacitor, React Native, or Flutter.
2. **Material 3 Expressive & Compose Adaptive Workstation UX:**
   - Implement **Material 3 Expressive** motion springs, morphing floating/docked toolbars, rich dynamic color tokens, and high-contrast semantic hierarchy.
   - Use **Compose Adaptive / Navigation 3** multi-pane desktop workstation scaffolds that reflow smoothly without state loss across tablet, convertible, and ultra-wide external monitors.
3. **First-Class Googlebook OS Desktop Capabilities:**
   - **True Multi-Instance Windowing:** Support opening multiple Visio documents side-by-side in independent freeform desktop windows (`FLAG_ACTIVITY_NEW_DOCUMENT` | `FLAG_ACTIVITY_MULTIPLE_TASK`, `resizeableActivity="true"`).
   - **Hardware Pointer & Keyboard Parity:** Contextual hardware mouse cursors (`PointerIcon` for 8-way resize, crosshair, connector glue, text I-beam, pan hand), right-click context menus, scroll-wheel zoom, and full hardware keyboard shortcuts.
   - **Cross-Window Drag & Drop:** Drag shapes, masters, stencils, and `.vsdx`/`.vsd`/`.dwg` files directly between side-by-side Vexio windows and the OS Files app.
   - **USI / Active Stylus & Local I/O:** Low-latency stylus ink & pressure/tilt prediction with palm rejection, Storage Access Framework direct file persistence, and native `PrintManager` tiled/PDF printing.
