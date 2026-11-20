---
trigger: always_on
---

# 1:1 Microsoft Visio Functional Parity Rules

1. **Lossless File & Stencil Round-Trip:**
   - `.vsdx`, `.vssx`, `.vstx`, `.vsdm`, `.vssm`, `.vstm` OpenXML packages must preserve unrecognized XML elements, custom OPC relationships, embedded media, document properties, themes/variants, background pages, headers/footers, and `vbaProject.bin` streams losslessly across load -> edit -> save cycles.
   - Legacy `.vsd`, `.vss`, `.vst` (OLE2/CFB binary) and `.vdx`, `.vdw` files must parse cleanly into the unified `vexio-core` Document + ShapeSheet IR and round-trip without losing geometry, text, formatting, masters, or layer memberships.
2. **1:1 ShapeSheet Semantics:**
   - Maintain Visio's internal canonical unit (inches for length, radians for angles, days for elapsed/datetime) while preserving user display units (`mm`, `cm`, `m`, `in`, `ft`, `pt`, `pica`, `deg`, `rad`, `%`).
   - Enforce `GUARD(expr)`: interactive/value writes (`SetValue`) must not overwrite a guarded cell; only `SETF(GetRef(cell), formula)` or direct formula editing in the ShapeSheet UI may replace a `GUARD` expression.
   - Support cross-shape and cross-page references (`Sheet.N!Cell`, `Pages[PageName]!Sheet.N!Cell`, `TheDoc!Cell`, `ThePage!Cell`) and reactive functions (`DEPENDSON`, `GRAVITY`, `INTERSECTX`, `INTERSECTY`, `PAR`, `PNT`, `LOCTOPAR`, `PARTOLOC`, `SETF`, `SETATREF`, `SETATREFEXPR`, `EVALCELL`, logical, string, trig, and lookup functions).
3. **Complete Enterprise Diagramming & CAD Scope:**
   - Dynamic connectors must support orthogonal, straight, and curved routing with obstacle avoidance, configurable line jumps, point-to-point static glue, and shape-to-shape dynamic walking glue.
   - Built-in rule validators must enforce **BPMN 2.0**, **UML 2.5**, and **SharePoint/Workflow** structural rules with structured `ValidationIssue` diagnostics.
   - CAD interop (`vexio-cad`) must work 100% offline for `.dwg` and `.dxf` with layer visibility/lock/color mapping, architectural scale locking, and lossless CAD-to-ShapeSheet geometry conversion.
   - Support two-way Data Linking (`.xlsx`, `.csv`, SQLite, JSON, SQL/REST), Data Graphics (Icon Sets, Data Bars, Text Callouts, Color by Value), Legends, BOM Shape Reports, Layer Manager, Markup Ink/Comments, sandboxed VBA-compatible Automation scripting, and vector-accurate PDF/A, SVG, EPS, DXF, raster, HTML, and multi-page tiled printing with registration marks.
