---
trigger: model_decision
description: "Apply when creating, modifying, or reviewing macOS native application code under apps/macos, Swift-Bridge bindings, Metal rendering, or Quick Look extensions."
---

# macOS Native Target Rules (`apps/macos` — Swift 6 + Liquid Glass + Metal)

1. **100% Native Apple Architecture:** Build strictly in **Swift 6** using **SwiftUI + AppKit (`NSDocument`)** and **Metal** (`CAMetalLayer` bound directly to `vexio-core` `wgpu` Metal backend). Never embed `WKWebView` for the app shell or canvas.
2. **Apple Liquid Glass Design Language & HIG:**
   - Implement translucent **Liquid Glass** toolbars, floating glass tool palettes, specular depth edge highlights, adaptive refraction over the live diagram canvas, and fluid spring physics.
   - Provide a left-hand **Vibrancy Stencil Navigator**, right-hand native **Inspector Sidebar** (Format, Shape Data, Geometry/ShapeSheet, Layers, Data Graphics), and bottom glass zoom/page pill controls.
3. **Deep macOS System Integration:**
   - Full global **macOS Menu Bar** command & shortcut integration (`Cmd`-mapped parity + Help search).
   - Native **Quick Look Thumbnail (`QLThumbnailProvider`)** and **Quick Look Preview (`QLPreviewProvider`)** App Extensions rendering `.vsdx`, `.vssx`, `.vstx`, and `.vsd` files offline via `vexio_render_preview_rgba`.
   - `NSUserActivity` **Continuity / Handoff** state serialization, Stage Manager multi-window responsiveness, and full multi-touch Trackpad support (`NSMagnificationGestureRecognizer` pinch-to-zoom, two-finger pan, `NSRotationGestureRecognizer`, and Force Touch pressure/quick-inspect).
