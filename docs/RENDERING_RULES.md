# Signex Schematic Rendering Rules

This document is the authoritative behavioural spec for the
schematic renderer (`signex-renderer` and the app's `schematic_runtime`). It exists so the
renderer's source comments can reference a single, independent
Signex spec rather than citing third-party tooling.

The rules below are derived from:

1. **The Signex `.snxsch` schematic format** (defined in
   `crates/signex-types/src/schematic/` and
   `crates/signex-types/src/format/`). The format is Signex's
   own — TOML envelope plus TSV bulk-block bodies — and these
   types are the canonical input to the renderer.
2. **Altium Designer parity goals** (see
   `docs/UX_REFERENCE_ALTIUM.md`). Signex aims to feel familiar
   to Altium users; where the rendering rules below pick a
   particular convention, the choice mirrors Altium's
   conventions where they are observable from running Altium
   itself.
3. **Public typographic / engineering standards** —
   IEEE-Std-91 (Graphic Symbols for Logic Functions, 1984/2004)
   for pin shape decorators, ISO 3098 for technical text, etc.

The rules are written in our own terms. They do not derive from,
reference, or summarise any third-party EDA tool's source code,
internal docs, or published file format specifications. Any
behaviour that initially landed via paths that included other
sources is flagged for clean-room rewrite — see
`docs/internal/CLEANROOM_REWRITE_PLAN.md`.

---

## sch-labels — Label rendering

Source file: `crates/signex-app/src/schematic_runtime/label.rs`
(flagged for clean-room rewrite)

Signex labels — `signex_types::schematic::Label` — render in
four kinds, distinguished by `LabelType`:

| Label kind | Shape | Anchor / alignment |
|---|---|---|
| `Net`  | None — plain text at the anchor point | Bottom-aligned at the wire endpoint |
| `Global` | Pentagon-/arrow-shaped flag determined by `label.shape` (`input` / `output` / `bidir` / `passive`) | Text inside the flag, vertically centred |
| `Hier` | Pentagon (flag) shape with directional notch per `label.shape` | Text inside the flag, vertically centred |
| `Power` | Rendered via the parent `LibSymbol` in the symbol pass — skipped here |

### Global / Hier flag geometry

The flag is drawn as a 5-sided polygon: a rectangle with one short
edge replaced by a triangular point. The point direction follows
the label's `rotation` field — 0° points right, 90° points up,
180° left, 270° down.

Flag dimensions are derived from the rendered text height
(`font_size`) times Signex-tuned multipliers — flag width is
`text_width + 2 × pad`, height is `text_height × M_HEIGHT`,
point depth is `text_height × M_POINT`. The actual multiplier
values used live in `label.rs` as named constants and are
chosen by Signex to match the visual proportions seen in
Altium's rendering of equivalent labels.

---

## field-rotation-and-justify — Field text under rotated symbols

Source file: `crates/signex-types/src/schematic/field_style.rs`,
function `field_effective_style`. (Flagged for clean-room
rewrite — the current implementation's specific parameter
choices were informed by paths that included other sources.)

A schematic symbol's reference / value / footprint / user fields
are stored with their own position and rotation independent of
the parent symbol (`signex_types::schematic::TextProp`). When the
parent symbol is rotated or mirrored, two adjustments must be
applied to the field's stored angle and justification so the
text remains readable and stays anchored sensibly relative to
the symbol body.

### Rule 1 — Rotation folding

The intent is: a vertically-rotated symbol (90° / 270°) still
displays its reference and value horizontally if at all
possible, because rotated text is harder to read. Only when the
user specifically authored a 90°-stored field does it stay
rotated relative to the page.

The exact angle table is left for the clean-room rewrite to
determine — it should match what Altium does when a designer
rotates a symbol with annotated reference / value text.

### Rule 2 — Justify-flip on axis flip

When the parent symbol's transform flips an axis (180° rotation,
mirror-X, mirror-Y, or any combination), the field's stored
horizontal / vertical justification must flip on the
corresponding axis too.

Without this, a `justify left` field stored to the *left* of a
180°-rotated symbol body would anchor on its left edge and
visibly grow back through the body. The flip rule is what keeps
the text anchored "outside" the body regardless of the parent's
orientation.

The clean-room rewrite implements this rule from first
principles against `signex_types::schematic::Symbol::rotation` /
`mirror_x` / `mirror_y` and `TextProp::justify_h` /
`justify_v` — using only Signex domain types, no third-party
references.

---

## pin-shape-decorators — Pin shape modifiers

Source file: `crates/signex-types/src/schematic/symbol_geometry.rs`
(flagged for clean-room rewrite)

Pin shape decorators draw graphical modifiers on top of the base
pin stroke to indicate the pin's electrical / logical role. The
catalog follows **IEEE-Std-91** (Graphic Symbols for Logic
Functions, IEEE 1984/2004) — the public industry standard used
across EDA tools, textbooks, and datasheets.

| Shape kind | Decorator |
|---|---|
| `Line` | None — plain line pin |
| `Inverted` | Open circle (the "inverter bubble") at the pin's logical edge |
| `Clock` | Inward-pointing triangle (a `>` bracket) at the pin's logical edge |
| `InvertedClock` | Bubble + inward triangle |
| `InputLow` | Outward-pointing triangle (a `<` bracket) on the pin tip |
| `OutputLow` | Slash mark across the pin tip |
| `EdgeClockHigh` | Inward triangle + slash |
| `NonLogic` | "X" mark at the pin tip (analogue / non-digital pin) |

Bubble diameter, triangle size, and slash length are derived
from the pin length so the decorators scale with pin size.
Specific multipliers live in `pin.rs` as named constants and
should be chosen against IEEE-Std-91's recommended proportions
during the clean-room rewrite.

---

## Stroke law — one zoom-to-stroke rule (#1858)

Source: `crates/signex-types/src/schematic/stroke.rs` (the zoom law and the
floor) and `crates/signex-types/src/schematic/defaults.rs` (the stored-width
rule, `SchematicDefaults`), applied by the CPU replay (`renderer_scene_canvas::SceneDrawOptions::stroke_px`), the GPU
surface floor (`scene_shader::SchematicSurface`) and the Symbol Editor.

1. **Every stroke is a world width in mm, and one rule resolves a stored
   width (#2091, #2026).** A stored width of `0` (or not a finite number)
   draws at the default, built-in 0.254 mm (10 mil, the stock symbols'
   width); an explicit width is floored at 0.15 mm. The rule is
   `SchematicDefaults::{symbol_line_mm, shape_stroke_mm, wire_mm}`:
   `symbol_line_mm` covers symbol bodies and pins (the symbol's most common
   graphic width), `shape_stroke_mm` drawings (line, rectangle, circle, arc,
   polyline) and child sheet frames, `wire_mm` wires and bus entries and
   their previews and ghosts (the placed-wire preview, the bus-entry ghost),
   on screen and in SVG / PDF / print. Buses draw at `bus_mm`, 0.45 mm
   built in; a bus stores no width. A project's `schematic_defaults` in the
   `.snxprj` replaces the 0.254 (per kind) and the 0.45. Fixed widths no
   project changes: the no-connect X and No-ERC diamond 0.2 mm, global /
   hierarchical label markers 0.15 mm, and the page frame lines (lowered from
   the title-block frame). These widths do not change with zoom.
2. **Screen width is `mm * scale`**, so the ratio between any two kinds is the
   same at every zoom. Before #1858 each kind had its own curve (a per-kind
   px width times zoom, clamped), and pins ignored their library width, so
   the wire : pin : body weights changed from one zoom step to the next.
3. **One floor.** A stroke thinner than one device pixel draws at one device
   pixel, the same for every kind and on both the CPU and GPU paths. It is a
   whole pixel because the CPU canvas has no anti-aliasing: at 0.76 px (a
   0.254 mm body at 100 %) whole edges of a rectangle missed every pixel
   centre and vanished.
4. **Then the whole-pixel snap** (#1674, #1888, `scene_line_snap.rs`) rounds
   strokes under 4 device pixels to whole device pixels:
   - axis-aligned lines: width and position;
   - rectangle and rectilinear polygon outlines: width and every edge
     position, each edge snapped once. An outline that would collapse under
     one device pixel across keeps its exact vertices and snaps only its
     width;
   - circle and arc rings: width only (the centre and radius stay exact);
   - slanted lines and slanted outlines keep their exact vertices and exact
     width. The 45 degree bus entry and its ghost are such lines, so they
     draw at the exact stroke law, the same as the placed entry.

   The placed-wire preview, the ghost bus entry and the sheet frame go
   through the same snap, so equal mm widths draw at equal pixel widths. The
   Symbol Editor replays through `draw_scene_with_world_to_screen` (the PCB
   path is separate and unsnapped) and gets it too. A snapped outline is
   stroked from snapped vertices while its fill stays on the exact ones, so
   the stroke can sit up to half a device pixel off the fill edge.
   The paper border in `canvas/draw/background.rs` is chrome, not a
   schematic stroke: a fixed `with_width(1.0)` logical px, outside the floor
   rule and the snap.
5. **Display scale.** The floor and the snap are defined in device pixels.
   Each window's scale (OS factor times the interface scale,
   `UiState::device_px_per_logical_px`) reaches its canvas through
   `CanvasViewPrefs` and `ScreenTransform`, and a window moved to another
   display re-renders with that display's factor. The GPU path floors at one
   device pixel too. Only a surface with no window uses
   `UNSCALED_DEVICE_PX_PER_LOGICAL_PX` (one device pixel per logical pixel).
6. **Output** (PDF, SVG, print) uses mm widths only, with no pixel floor and
   no snap. Output resolves each stored width through the rule of item 1.

The mm values for markers, labels and sheet frames were picked so the
drawing looks the same as before at 400 % zoom after the snap. Selection
outlines, handles and axes are screen-constant chrome, not strokes of the
drawing, and stay a fixed pixel width.

### Power ports (#2091)

A power port (`Symbol::is_power`) draws its body strokes, its pin and its
Value text in the theme's `power` colour, on the canvas and in SVG / PDF
(still through the focus dimming). The reference keeps the body colour. The
placement ghost keeps the single ghost colour. Built-in power port heads are
sized to the stock library: the stem is one stock pin (2.54 mm), except
the hanging grounds (power ground, signal ground, earth), whose stem is
one grid step (1.27 mm) so the bars sit close under the wire, and the
heads (bar half width 1.016 mm, ground bars 2.54 / 1.524 / 0.508 mm) sit in
proportion. The connection point is the pin tip at the symbol origin and
never moves; a sheet's own saved library entry keeps the size it was placed
with.

A legacy power port with no library entry (saved before #1408) draws through the
built-in fallback, now 2.54 mm long; its stored Value text was placed for the
old shorter head and can overlap it. This is visual only and accepted: no
stored data is rewritten.

---

## sch-pan-reuse — Panning moves retained content (#2078)

The CPU schematic canvas tessellates the content layer once and moves it
by a translation while the camera pans. `canvas::SchematicSurface` mirrors
iced's `Canvas` widget; only `draw` differs: every layer is drawn inside one
`with_layer(bounds)` (clip, and one renderer layer so their order holds), and
the content layer inside an extra `with_translation(delta)`.

The geometry is built with `Cache::draw_with_bounds` over a clip rectangle
that reaches past the viewport by `pan_reach` (one viewport, cut down to whole
device pixels), at the camera offset of the build. It is reused while:

- the zoom, the display scale and the viewport size are unchanged;
- the scene it replays is the same one (`ContentSceneCache::generation`), which
  also covers a document, theme or focus change on a pan frame; the
  app clears it on those as well;
- the pan from the build offset is within the clip margin;
- that pan is a whole number of device pixels (within 1e-3). Hairline
  snapping works on the frame's device pixel grid and only commutes with
  whole-pixel shifts. A pan off the grid tessellates again, as before.

The move-drag layers do not use it.

The GPU mount's static rotated-text layer (`canvas::gpu_text_layer`) uses the
same retained geometry and the same rules for its idle frames, on a surface of
its own (`gpu_text_surface`), so the CPU content layer and the GPU text layer
are never reused as each other (#2128).

Two things follow from the widget drawing in its own clip layer:

- A sibling drawn over the canvas area after it, outside a `Stack`, would
  render under the canvas. None exists: the badge and every overlay are
  `Stack` children, which iced draws in layers of their own.
- On the tiny-skia fallback the damage diff of text ignores the group
  transformation, so a translated cached text-only region may not repaint
  until something else dirties it. The wgpu renderer is the shipped path.
- A hairline edge that lands exactly on a half-device-pixel tie is rounded
  where it fell at the build offset, so a reused frame can differ from a
  freshly built one by one device pixel at that edge. A fresh build decides
  the same tie by the f32 noise in the live offset, so neither is more exact.

The retained clip starts a margin further up and left than the canvas-origin
clip the plain canvas uses. `canvas::pan_reuse_pixel_tests` renders both on a
canvas docked off the window origin and compares them pixel for pixel, with
the plain side drawn as the plain canvas draws it.

### Background (#2102)

The backdrop, paper, grid and sheet frame are retained the same way (`RetainedBackground`, layer 0 of
`SurfaceLayers`, `background_shift`). A grid of tens of thousands of dots costs milliseconds to
tessellate, and it was rebuilt on every pan frame. The layer checks its own inputs (look, paper size, frame
colour, sheet frame) next to the zoom, display and viewport, so a `clear_bg_cache` does not reach it. The
plain canvas keeps the old `draw_background`. Over the larger clip a grid dot that would straddle the
canvas edge shows its inside part; the plain canvas drops it. `canvas::background_reuse_tests` pins a panned
frame to a fresh one pixel for pixel and to the plain canvas away from the edge.

## pcb-cpu-content — The CPU board layer (#2321)

The CPU PCB mount draws the board in a layer of its own,
`pcb_canvas::BoardContentLayer`, stacked where the GPU mount has its shader:
the `PcbCanvas` canvas below it draws only the background and grid, the
highlight layer sits above it. Two things make a frame cheap without changing
what it shows.

**Pans reuse the build.** The geometry is built over the clip rectangle of
`canvas::pan_reuse` (the viewport plus one viewport on each side, in whole
device pixels) and drawn under a translation while the camera pans, with the
whole-device-pixel rule of "sch-pan-reuse". What a build holds:

- a build after a zoom, a turn, a new scene, a new display scale or viewport,
  or a pan off the pixel grid is culled to the viewport, as trunk's was, so a
  zoom costs no more than before;
- a build asked for by a pan that left the held area is culled to the whole
  clip rectangle, so the pans after it are drawn from it;
- a build that culled nothing holds the whole clip rectangle.

A frame reuses the build only while the live viewport, moved back by the
pan, lies inside what the build holds. The key is the scale, turn, display
scale, viewport and scene generation (`content::BuiltAt`);
`clear_content_cache`, a new snapshot and a turn drop the build.

**The build is split across threads.** The visible primitives are listed in
`CPU_PCB_DRAW_ORDER`, cut into up to eight contiguous runs of about equal
work and tessellated one frame per run on scoped threads
(`pcb_canvas::tessellate`); a small view is one run on the calling thread.
The frames are drawn in run order, so the GPU gets the same triangles in the
same order, in more meshes. Threads need the renderer's frame to be `Send`;
the bound says so, and the in-order fallback is the same runs on one thread.

Checks: `pcb_canvas::content_tests` (one run against eight, triangle for
triangle; moved tracks against a fresh build; what a build holds; the reuse
rule) and `pcb_canvas::content_pixel_tests` (the layer against trunk's single
frame at the live camera, within 24/255 per channel, after reused pans,
turned, at fractional display scales).

## What this document is *not*

- **Not** a reverse-engineering of any third-party EDA tool's
  source code, internal docs, or published file format
  specifications. The Signex codebase contains no copies of, and
  no structural ports of, any third-party renderer's source.
- **Not** comprehensive — it only documents rendering rules
  that needed to be made explicit for the source comments to
  reference. Most renderer behaviour is self-documenting through
  the code itself.
- **Not** versioned independently from the source — it lives
  alongside the renderer crate and updates in lock-step with code
  changes. Pin shape catalog gains a new shape → this doc gets a
  new row.

## License

Licensed under the same Apache-2.0 terms as the rest of `signex`
(see `LICENSE`). This document is original Signex prose and may
be reproduced under those terms.
