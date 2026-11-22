# Vexio — Antigravity Session Context & Local Mac Handoff

This folder preserves the Antigravity session artifacts transferred from the remote workspace at release **`v0.2.1-alpha`** (`a2c717b`), so any local Antigravity session on macOS can immediately pick up with full architectural context.

## Context Documents
- [`task.md`](./task.md) — Full Phase 1–6 roadmap & verification checklist through `v0.2.1-alpha`.
- [`walkthrough.md`](./walkthrough.md) — `v0.2.1-alpha` root-cause fixes, multi-OS installer links, and test verification summary.
- [`implementation_plan.md`](./implementation_plan.md) — Deep architectural design across `crates/*` and `apps/*`.
- [`platform_functionality_audit.md`](./platform_functionality_audit.md) — Feature-by-feature parity matrix across macOS, Windows, Android/Googlebook, and ChromeOS PWA.
- [`platform_parity_explorer.html`](./platform_parity_explorer.html) — Interactive parity explorer widget.

## Quick Prompt to Resume in Local Antigravity on Mac
> *"Read `AGENTS.md`, `.agents/context/task.md`, and `.agents/context/walkthrough.md` to get up to speed on `v0.2.1-alpha`, then let's continue."*

## Local Mac Native Build Quickstart (`apps/macos`)
```bash
# 1. Build static Rust FFI archive (libvexio_core.a)
cargo build --release -p vexio-core
mkdir -p target/universal/release
cp -f target/release/libvexio_core.a target/universal/release/libvexio_core.a
rm -f target/release/libvexio_core.dylib

# 2. Run Swift 6 + Metal workstation app (or `open apps/macos/Package.swift` in Xcode 16+)
swift run --package-path apps/macos -c release VexioMacApp
```
