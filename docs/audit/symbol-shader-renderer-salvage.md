# Symbol Editor — Scene/Shader Renderer Salvage Plan

> **Date:** 2026-07-05
> **Branch:** `claude/symbol-editor-salvage`
> **Source of truth:** `feature/v0.13` / `feature/v0.13-symbol`

## TL;DR

`feature/v0.13` routes the **symbol editor** through the **unified scene
renderer** (`Symbol → SchematicSnapshot → SchematicRenderer::build_scene
→ signex_gfx::Scene → draw`). `dev` never carried this — dev's symbol
editor draws directly with `iced::canvas` primitives. This plan brings
the scene-unified rendering onto dev.

**Important framing:** at the app layer this is **not** live WGSL.
Both dev's `pcb_canvas` and v0.13's symbol path build a CPU-side `Scene`
and tessellate it with `iced` `frame.fill`/`frame.stroke`. The WGSL
pipelines in `signex-gfx` exist but are not the 2D-canvas app-layer path
today. So the win is **render unification/consistency** (symbol matches
schematic/PCB stroke/zoom/arc/text behaviour) + **the pin text Y-flip
fixes** + being **ready for the eventual Scene→WGSL cutover** — *not*
GPU perf today.

## Status

| Piece | State |
|-------|-------|
| **Option B — pin text align (Y-flip)** | ✅ done, live (`draw_pin` + `state::PinTextGeometry`), 4 tests |
| **A · `Symbol → SchematicSnapshot` mapper** | ✅ done, tested, **not wired** (`snapshot::build_symbol_snapshot`), 8 tests |
| A · scene drawers for `scene.arcs` / `scene.texts` | ⏳ TODO (visual) |
| A · swap `SymbolCanvas::draw` to scene path | ⏳ TODO (visual) |
| A · visual verification in the running app | ⏳ TODO (needs a human) |

## What dev already has (verified)

- `signex_renderer::schematic::{SchematicSnapshot, WireInput,
  JunctionInput, ArcInput, PolygonInput, TextInput, OverlayInputs}` —
  the full input API. ✅
- `ViewRenderer::build_scene(&snapshot, &theme, DirtyFlags, &mut Scene)`
  implemented for `SchematicRenderer`. ✅
- `signex_gfx::scene::Scene` with `lines, circles, arcs, polygons,
  texts` (+ `overlay_*`, `erc_marker_*`) buckets. ✅ (arcs + texts
  buckets exist — build_scene can populate them)
- `ResolvedTheme::from_canvas_colors(theme::canvas_colors(ThemeId::Signex))`,
  `signex_types::schematic::{HAlign, VAlign}`, `crate::canvas::Camera`. ✅
- A `draw_scene(frame, &scene, &camera, bounds)` pattern in
  `pcb_canvas.rs` — **but only draws `lines`/`circles`/`polygons`
  (+overlays); no arcs, no texts.** ⚠️ private to that file.

## What dev lacks (the remaining work)

### 1. Scene drawers for `scene.arcs` and `scene.texts` — TODO (visual)

Nobody on dev draws `scene.arcs` or `scene.texts` yet (pcb `draw_scene`
skips them). Two options:

- **Preferred:** wait for / align with the schematic Scene cutover
  (`docs/renderer-phase-notes`, milestone-f). When the schematic starts
  drawing `scene.arcs`/`scene.texts`, reuse those drawers for the symbol
  editor — don't write a parallel pair that the cutover would supersede.
- **If proceeding solo:** add `draw_arcs` + `draw_texts` next to
  `pcb_canvas::draw_scene` (or a shared `scene_draw` module):
  - `draw_arcs`: for each `signex_gfx::…::Arc`, build an `iced`
    `canvas::path::Arc` — **negate the sweep angles for the Y-flip**
    (same fix as `symbol/canvas.rs` arc rendering; world Y-up vs canvas
    Y-down).
  - `draw_texts`: for each `TextItem`, `frame.with_save` → translate to
    the screen anchor → `rotate(Radians(rotation))` → `fill_text`, with
    `h_align`/`v_align` → iced alignment. This is the fiddly part;
    verify placement visually.

### 2. Swap `SymbolCanvas::draw` to the scene path — TODO (visual)

Replace the per-primitive draw loop in `symbol/canvas.rs::draw` with:

```rust
let colors = snapshot::SymbolSnapshotColors { body, selected, pin, text };
let snap = snapshot::build_symbol_snapshot(self.symbol, &self.selected,
    self.active_part, &colors, scale);
let mut scene = signex_gfx::scene::Scene::default();
signex_renderer::schematic::SchematicRenderer::build_scene(
    &snap,
    &ResolvedTheme::from_canvas_colors(canvas_colors(ThemeId::Signex)),
    DirtyFlags::LINES | DirtyFlags::CIRCLES | DirtyFlags::ARCS
        | DirtyFlags::POLYGONS | DirtyFlags::TEXT,
    &mut scene,
);
draw_scene(frame, &scene, &camera, bounds); // incl. arcs + texts
```

**Keep as-is** (they are editor chrome, not symbol geometry): the grid
dots, the origin crosshair, the resize handles, the rubber-band box, and
the tool-hint label. Only the symbol-primitive drawing moves to the
scene path. `draw_pin`'s text (Option B) is subsumed by the snapshot's
`pin_texts` once the scene path is live — delete `draw_pin`'s text block
then, or keep `draw_pin` for the interaction-only bits.

### 3. Not needed on dev (v0.13 used these; we don't)

- `anchor2d`/`rotation2d` utils + `PinRenderGeometry` — reimplemented as
  `state::PinTextGeometry` (text rotation/side) + `state::pin_body_delta`
  (tip/body-end) using dev's plain trig. No util port required.
- `renderer_scene_canvas::draw_scene_with_world_to_screen` — dev's
  `pcb_canvas::draw_scene` is the equivalent (extend it for arcs/texts).

## Effort (remaining)

| Task | Size |
|------|------|
| `draw_arcs` + `draw_texts` scene drawers (text = fiddly) | M–L (~3–5 h) |
| `SymbolCanvas::draw` swap, keep overlays | M (~2–3 h) |
| Visual verification pass (running app) | required, human |

**~1 day of code + a visual pass** — best done *after* / *with* the
schematic Scene cutover so the arc/text drawers are shared, not
duplicated.

## Recommendation

The tested, renderer-independent core (the snapshot mapper + pin text
geometry) is **done and safe**. Hold the arc/text drawers + the `draw`
swap until the schematic Scene cutover (milestone-f) lands the canonical
`scene.arcs`/`scene.texts` drawing — then the symbol editor joins a
working scene path instead of pioneering (and later reconciling) its own.
