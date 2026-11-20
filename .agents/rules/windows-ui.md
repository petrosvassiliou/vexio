---
trigger: model_decision
description: "Apply when creating, modifying, or reviewing Windows native application code under apps/windows or Windows C-ABI / DirectX 12 SwapChain bindings."
---

# Windows Native Target Rules (`apps/windows` — WinUI 3 + DirectX 12)

1. **Zero Webviews:** Build strictly with **WinUI 3 / Windows App SDK** (C++20 / CppWinRT) and **DirectX 12** (`ISwapChainPanelNative` bound to `vexio_viewport_create_dx12_swapchain` via `vexio.h`). Never use `WebView2`, Electron, or Tauri.
2. **1:1 Visio Muscle Memory & Command Hierarchy:**
   - Implement the classic multi-tabbed Ribbon (`File` Backstage, `Home`, `Insert`, `Design`, `Data`, `Process`, `Review`, `View`, `Developer`) + Quick Access Toolbar with full Visio keyboard shortcut/KeyTip parity (`Ctrl+1` Pointer, `Ctrl+2` Text, `Ctrl+3` Connector, `Shift+F4` Page Setup, `Alt+F9` VBA/Automation, `Shift+F5` ShapeSheet).
   - Use clean, original vector Segoe/SVG iconography to remain 100% IP-clean.
3. **Required Dockable Workstation Panes:**
   - Collapsible left `Shapes` / Stencils pane with instant stencil search and master drag-onto-canvas.
   - Dockable right Task Panes: `Shape Data`, `Size & Position`, `Format Shape`, `Pan & Zoom`, `External Data`, and `Layer Properties`.
   - Bottom Page Tab strip (`Insert Page`, drag reorder, rename, `All` popup, background page badge), horizontal/vertical Ruler & Grid overlays, Status Bar with Zoom Slider / Fit-to-Window / Macro & Validation indicators, and a dockable bottom **ShapeSheet Editor** split-view with live formula bar and section collapsible groups.
