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

   **Stroke ends.** A solid scene line (a placed wire or bus segment) is
   stroked with round ends, `renderer_scene_canvas::solid_line_stroke`, so two
   segments meeting at a bend close the corner even where the snap moves
   their ends apart. The wire and bus routing preview strokes its segments
   with the same function, so a route draws its corners as the placed one
   will (`wire_corner_tests::the_routing_preview_joins_its_corners_like_a_placed_segment`).
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
sized to the stock library: every style's stem is one grid step (1.27 mm;
the rising styles had 2.54 mm until Caner's decision of 2026-11-21), so the
head sits close to the wire, and the
heads (bar half width 1.016 mm, ground bars 2.54 / 1.524 / 0.508 mm) sit in
proportion. The connection point is the pin tip at the symbol origin and
never moves; a sheet's own saved library entry keeps the size it was placed
with.

A legacy power port with no library entry (saved before #1408) draws through the
built-in fallback, now 2.54 mm long; its stored Value text was placed for the
old shorter head and can overlap it. This is visual only and accepted: no
stored data is rewritten.

---

## sch-junction-colour — Junction colour

A junction dot is drawn in the colour of the wires it joins. Every wire that
ends on the dot or passes through it votes with the colour it is drawn in (its
own colour, else the per-net colour, else the net class colour, else the
default). The colour with strictly more votes than any other wins. When two
colours tie, or the default wins, the dot keeps its own rule: its net class
colour, with a per-net colour over that, else the theme's junction colour.

Examples: a T of three green wires draws a green dot; two green wires and one
default wire draw a green dot; one green, one red and one default wire draw the
dot's own colour. A per-net colour given to the wires colours the dot as well.

The canvas, the SVG page (and so PDF and print) and the drag ghost read the one
function, `signex_types::schematic::junction_wire_colors`. During a drag the
moving items are drawn apart from the rest, so a dot that moves while its
wires stay (or the other way round) shows its own rule until the drop.

---

## sch-junction-places — Where a junction dot stands

Every dot is a stored junction; the canvas draws what the sheet holds and
derives none at draw time. The engine mints an automatic dot (`minted`) where
a wire ends inside another wire (a T), where three or more wire ends meet,
where a pin end lies inside a wire, where two wire ends meet on a pin end (a
part's or a power port's), and at the foot of a directive's stem on a wire (a
Net Parameters flag, a Differential Pair with a class). It drops the dot once
that meeting is gone. A net label on a wire gets no dot. Decisions: the
2026-11-18, 2026-11-19 and 2026-11-21 junction rows of `docs/DECISIONS.md`.

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

Three things follow from the widget drawing in its own clip layer:

- A sibling drawn over the canvas area after it, outside a `Stack`, would
  render under the canvas. None exists: the badge and every overlay are
  `Stack` children, which iced draws in layers of their own.
- On vello_cpu (the software fallback) the damage diff of text ignores the group
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

A solid line is stroked with round ends, so a track draws as the capsule the
GPU line shader, hit-testing and DRC take it to be and a bend of any angle is
filled (#2391; `pcb_canvas::tessellate::track_cap_tests`).

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
frame at the live camera, within 24/255 per channel and at most 4 edge
pixels, after reused pans, turned, at fractional display scales).

## pcb-footprint-text — Designator and comment text on the board (#2368)

A footprint's `Reference` and `Value` properties are drawn; other
properties are parameters and are not. Where each one is drawn comes from
`pcb_geom::footprint_texts` (#2434), the one answer the canvas, the board
print, the fabrication outputs and the selection index read: anchor, turn,
mirror, layer, height and justification. A property's position and rotation
are footprint-local, placed by `pcb_geom::property_text_placement` with the
footprint's own transform (mirror on the bottom side, then turn, then move),
so the text follows a move, a turn and a flip. The angle composes as a pad's
does: the footprint's turn plus the text's own, minus it on the bottom side.

- **No position:** the designator sits `PCB_FP_TEXT_OFFSET_MM` above the
  footprint origin, the comment that far below it.
- **No layer:** the overlay of the footprint's side
  (`pcb_geom::property_text_layer`); an unknown size is
  `PCB_DEFAULT_TEXT_SIZE_MM`.
- **Justification:** the property's `justify` (left, center, right; centred
  when not saved) says which end of the line sits on the anchor, in the
  text's own frame. A mirrored text is justified before its mirror, so a
  left-justified text on the bottom side runs from its anchor towards -x,
  as its strokes do. Each drawer lays the line with its own glyph widths:
  the canvas by the font's alignment, the board print by centring its line
  on `pcb_geom::justified_centre`, the fabrication outputs by the stroke
  font.
- **Shown:** not hidden, not empty, and its layer visible in the board view
  (`PcbLayerView::is_visible`, so single-layer mode shows the active layer's
  text only). Colour: `layer_style::board_layer_colour` of its layer.
- **Size:** true size (cap height in mm through `mm_per_em`), never clamped
  to a readable minimum; a text under half a pixel is not drawn.
- **Picked and moved on its own:** the selection index holds each shown
  text as `ItemRef::FootprintText`, on the box the stroke font fills
  (`pcb_geom::text_box`). A click picks it over its part's body; a box or
  Select All takes the footprint instead. A press on it drags the text
  alone, following the pointer without snapping, and the release is one
  board undo step (`footprint_text_edit`, a `SetFootprintProperty`); a
  locked footprint refuses. The drag preview is the text's box.
  `set_footprint_text` does the same for an agent.

The texts travel in `PcbSnapshot::texts`, not in the `Scene`: the GPU glyph
atlas can neither mirror nor follow a turned view. `pcb_canvas::BoardTextLayer`
paints them on a canvas layer between the board content and the highlight
layer on both mounts, so the CPU and GPU mounts draw the same text. The
layer is retained until the camera or the scene generation changes. A
mirrored or turned text is drawn under a frame transform, which iced
tessellates as outlines; an upright one stays a text primitive. During a
drag the carried footprints' texts are painted by the highlight layer with
the rest of the drag preview.

Checks: `pcb::footprint_text::tests` (place, flip, eye, layer and
single-layer visibility, justification), `pcb_geom::text::tests` (the
bottom-side mirror case among them), `pcb_canvas::board_text::tests`,
`signex_engine::pcb::footprint_text_edit::tests`,
`command::footprint_text_operand_tests` (drag and agent move), regression
`pcb_component_properties::{the_canvas_draws_the_designator_until_the_eye_hides_it, a_flip_puts_the_designator_mirrored_on_the_bottom_overlay}`.

## pcb-stroke-font — Board text as strokes (#2368)

Where a text must be strokes (a Gerber image), it is written in the board
stroke font, `pcb_geom::text_strokes`: each glyph a few polylines on a grid
8 units to the cap height, 4 wide, the next glyph 6 units on (so a line is
`0.75 × height` per glyph, less the last glyph's gap). Lower case reaches 5
units, descenders 2 below the baseline. The cap height is centred on the
anchor. The pen is `TEXT_STROKE_EM` (0.15) of the height. A glyph the font
lacks is drawn as a box, never left out. Printable ASCII, `µ`, `Ω`, `°` and
`±` have glyphs. The font is Signex's own drawing.

Checks: `pcb_geom::stroke_font::tests`.

## pcb-graphics — Footprint and board drawings on the board (#2387)

A footprint's drawings (`Footprint.graphics`: silkscreen, fab, assembly,
courtyard, 3D body outline) are footprint-local and are placed by
`pcb_geom::footprint_graphic_placement` with the footprint's own transform,
as its pads are. The board's own drawings (`PcbBoard.graphics`) and texts
(`PcbBoard.texts`) are in board coordinates already
(`board_graphic_placement`, `board_text_placement`).

- **Kinds:** `line`, `rect` (four corners, so it turns with the footprint),
  `poly` (a closed outline), `circle`, `arc`, `text`. Another kind, or one
  missing the points it needs, is not drawn; nor is a value `pcb-geom`
  refuses.
- **Circle:** about its centre through its rim (`end`, else `start`); the
  saved `radius` only when it has neither. The design rule check's copper
  and the fabrication outputs take the same radius (`pcb_geom::circle_radius`),
  as do a footprint's courtyard box, its staging extent and the bake preview.
- **Arc:** about its centre from its start to its end; with a middle point
  it runs the way that passes it, without one the way of increasing angle.
  The bottom-side mirror reverses it. Drawn as chords of at most 6 degrees.
- **Fill:** `fill = "solid"` draws a circle as a disc and a rectangle or
  polygon as an area; otherwise the outline. A drawing saved with no width
  takes `PCB_DEFAULT_GRAPHIC_WIDTH_MM`.
- **Layer:** drawn only on a layer this build knows and the view shows
  (`PcbLayerView::is_visible`, so single-layer mode draws the active layer's
  drawings only). Colour: `ResolvedTheme::board_layer_color`, which the
  Layers panel swatches read too. A drawing on a copper layer off the active
  copper layer is dimmed as copper is; other layers are not dimmed.
- **Text:** a footprint's `text` drawing and a board text join
  `PcbSnapshot::texts` and draw as the designator does (see
  "pcb-footprint-text"). A board text on a bottom-side layer is mirrored: it
  is read from below.

The strokes and areas travel in `PcbSnapshot::graphics` and are emitted
into the overlay buckets after the board features and before the ratsnest,
so both mounts draw them from the one scene. A drag preview builds the
carried footprints' snapshot through the same path, so their drawings move
with them. The library preview (`PcbSnapshot::from_footprint`) takes the pads
only and draws the drawings itself in the same palette (#2348).

Checks: `pcb_geom::graphic::tests`, `pcb::graphics::tests` (placement, flip,
eye, single-layer mode, dimming, board text), `pcb_canvas::drag_preview::tests`.

## pcb-text-frame — A board text frame (#2424)

A text frame is a board drawing (`graphic_type = "text_frame"`) with two
opposite corners (`start`, `end`), a text and a text height (`font_size`,
else `PCB_DEFAULT_TEXT_SIZE_MM`). `pcb_geom::board_graphic_placement` places
its box as a closed outline, drawn like a rectangle with the drawing's line
width. `pcb_geom::text_frame_lines` places its text:

- **Wrap:** at each `\n`, then between words, to the number of glyphs that
  fit the box less a margin of a quarter of the text height on each side; a
  glyph is taken as 0.6 of the text height wide, wider than the board's
  monospace font, so a line always fits. A word longer than a line is cut.
- **Place:** lines run down from the top of the box (model y grows down), a
  line pitch of 1.4 text heights, each line centred across the box, upright.
  Lines past the bottom of the box are not drawn; one line always is.
- **Side:** on a bottom-side layer each line is mirrored, read from below.

The lines join `PcbSnapshot::texts` and draw as a board text does; the board
print (#2402) prints the box and the lines in the same places; fabrication
outputs (#2388) draw the box on a fabrication layer and write the lines in
the board stroke font (#2368, "pcb-stroke-font"). That font is wider than
the wrap's 0.6, so a line longer than the room inside the margins
(`FrameLine::width_mm`) is condensed along its line to fit
(`pcb_geom::text_strokes_within`); its height is kept.

Checks: `pcb_geom::text_frame::tests`, `pcb::graphics::tests::a_text_frame_draws_its_box_and_its_lines`,
`pdf::board::tests::a_text_frame_prints_its_box_and_lines`,
`fab::draw::tests::a_text_frames_box_is_drawn_as_its_rectangle`,
`fab::tests::a_full_width_text_frame_line_stays_inside_its_box`.

## board-3d — Board 3D view (#2390)

View ▸ 3D View (`view_board_3d`, key `3` on a board) shows the board in view
in 3D in the main window; the same command, the toolbar's 2D segment or `3`
again shows it flat. Undocked board windows stay flat. It is view state like
pan and zoom: no board command, no undo step, no dirty mark; every toolbar
action below is view state too.

**Frame.** Millimetres, right-handed, z up out of the top face: a board
point `(x, y)` (y down, as the flat view draws it) is `(x, -y)`. The bottom
face is at z = 0, the top face at z = board thickness (the default
`PCB_DEFAULT_THICKNESS_MM` when the board has none). The Top viewpoint shows
the board as the flat view does; Bottom mirrors it left to right.

**Lowering** (`signex_renderer::board3d::build_board_scene`, one indexed
mesh, `signex_gfx::scene3d::Scene3d`):

- **Physical stack.** From the laminate out on each side: copper, solder
  mask, silkscreen. Heights come from the board's layer stack, or the stack
  derived from its copper list and thickness, scaled to the board thickness
  (`board3d::stack::StackHeights`). The outer mask surfaces are the board's
  two faces (z = 0 and z = thickness). A mask or outer copper row is drawn
  at least 0.005 mm thick so its order survives the depth buffer; nothing
  else is exaggerated. Silkscreen stands 0.015 mm off the mask.
- **Materials.** The assembled board uses physical colours
  (`board_3d_colors`), never the flat view's layer colours: green,
  slightly translucent mask, FR-4 laminate, copper, gold finish, white
  silkscreen. The board stores no mask colour or surface finish, so these
  are the defaults. The mask is drawn after the copper so it blends over it.
- **Board.** The laminate is the outline with every drill hole (pads, vias),
  cutout and mounting hole that lies inside it cut through; hidden layers do
  not close a hole. A board with no outline gets the box round its copper,
  1 mm wider each side. Edge and unplated hole walls are FR-4, plated drill
  walls take the finish; walls run the full thickness. A hole that meets one
  cut before it (two vias at one point, a pad drill on a mounting hole, a
  drill inside a cutout) is left out (`signex_gfx::ring_fill::disjoint_holes`,
  counted in `BoardScene3d::dropped_holes` and logged), so one such pair
  cannot make the fill refuse every hole. The fill with holes is the flat
  view's (`signex_gfx::ring_fill::triangulate_with_holes`).
- **Solder mask** covers the board on both faces, whatever the mask layers'
  eyes say. Openings come from the data: a pad's listed mask layers, or its
  type's default as the fabrication images read it (a pad stores no
  expansion of its own), a via per side unless tented (with its manual
  expansion, from the hole edge when set), a track with a manual mask
  expansion, and shapes and zones drawn on the mask layer. Mask over copper
  or a tented via is a lighter tint of the mask colour; a tent spans the
  drill.
- **Single-layer mode does not reach 3D.** It is an editing aid of the flat
  view; in 3D the copper of every shown layer and the parts of both faces
  stay. Only a layer's eye hides it (`PcbLayerView::is_hidden`). The active
  layer's per-layer size does not reach 3D either: a via or pad keeps its
  stored `diameter` or `size`.
- **Copper** follows the layer eyes, so a hidden layer is hidden in 3D too.
  Each layer is the union of its tracks, pads, vias and poured fills, clipped
  to the board and cut by every drill, at its stack height. Copper inside a
  mask opening takes the finish; the rest is under the mask. Inner copper
  sits inside the laminate, seen only in the explode and at drill walls.
- **Silkscreen** is the shown silkscreen drawings, texts and zones as areas,
  clipped to the board and kept out of mask openings and holes.
- **Parts.** One per footprint whose face's 3D Body layer is shown. The
  model is the library footprint's (`library_ref`), read off the update
  thread: its attached model file (`step/<hash>.step`; a `.wrl` or `.vrml`
  name reads VRML), in the file's units taken as mm, the model's y up the
  board, turned by the attachment's x, y, z angles and moved by its offset;
  else its 3D Body (outline, height, offset, colours; a neutral body colour
  when it has none). A footprint with no
  library footprint, or one that could not be read (Messages says why), is
  its body box (`pcb_geom::footprint_body`) extruded to its component height
  (1 mm when unset). Parts are placed by `pcb_geom::Placement`; a bottom part
  hangs below the bottom face, x mirrored and z flipped, which turns it over.
  The component's body opacity scales its alpha.

**Toolbar** (mockup rows 109 and 175): one floating toolbar at the top
centre of the view, always shown: the "3D View" title; 2D | 3D; Top, Iso,
Front, Left, Right, Back, Reset and Fit; Flip and Explode; the lighting
preset; the screenshot button and its menu chevron. The interaction hint is
a quiet caption at the bottom right, the status pill sits at the bottom
left, the view cube at the top right, under the toolbar's band so the two
never overlap at any view width. Under the cube, while the view is tall
enough, a legend card lists the stack from the laminate out in the scene's
own materials (mockup row 109). As decided (DECISIONS, the 2026-11-19
board 3D view toolbar row), the "3D View" title stays and Left and Right
are separate buttons (no single Side button with Shift+click).

**Camera** (`signex_gfx::camera3d::OrbitCamera`): perspective, orbiting a
target. Left drag turns it, right or middle drag or Shift left drag slides
it, the wheel zooms. A viewpoint looks from there on the side shown
(`OrbitCamera::set_preset_on`) and frames the board: Top, Iso (above a front
corner), Front, Left, Right and Back (the high-y edge). The active
viewpoint is tinted; a turn by hand clears it. Bottom is still a viewpoint
(`PresetPicked`), reached by Flip. Reset looks
from Iso on the side shown, framed; flip, explode and lighting stay. Fit and
View ▸ Fit All frame the board from where it is seen. Entering the view looks
from Iso on the top side (the tinted viewpoint), framed for an assumed shape;
the view's first redraw reports
its real size and frames it again, and a later redraw reports where the
view sits in the window (where a screenshot is cropped). A gesture never
frames it again, so the first drag or wheel step cannot jump. The view
follows a renamed board and ends when the board's tab closes.

**Flip** turns the board over: the side shown changes at once and the
camera turns to `OrbitCamera::flipped` over `FLIP_DURATION_MS`
(`OrbitCamera::tween`), driven by a frame subscription that exists only
while the turn runs. A gesture, a viewpoint, Reset or Fit during the turn
ends it at its target first. While flipped the button reads Bottom and is
tinted, the view cube reads BOTTOM, RIGHT, FRONT, and the status pill names
the bottom side's parts and the parts with through-hole pins.

**Popovers**: the Explode popover and the screenshot menu hang centred under
their own buttons. Esc closes an open one first (`board_3d_popover_escape`
in the Esc ladder), so it never reaches the tool reset behind the view.

**Explode** opens a popover under its button: a slider from Assembled
(0 %) to Layers apart (100 %) and two tick boxes, Separate copper and mask
and Lift components. Apart, the layers keep their stack order: from the
core out on each side, inner copper, outer copper, mask, silkscreen, then
the parts. The scene the view draws and picks with is
`BoardScene3d::exploded` of the assembled one (only heights move), cached by
the assembled scene's generation and the explode, so a click still selects
a lifted part. The button reads `Explode <n> %` and is tinted while its
popover is open or the board stands apart; the status pill says what stands
apart.

**Lighting**: four presets, Studio, Workshop, Outdoor and Dark
(`LightingPreset`); a click on the lighting button moves to the next and a
toast under the toolbar shows the four swatches and `<Name> · <i> of 4` for
about two seconds (a newer toast outlives an older timer). The mesh is lit
by the preset (`OrbitCamera::uniform_lit`). The backdrop is the preset's
vertical gradient (`styles::board_palette::LightingBackdrop`: Studio
#E8E8EA to #BFC2C7, Workshop #EADBC6 to #9C8770, Outdoor #BFD8F2 to #9DB38A,
Dark #2A2D33 to #0E0F12). It follows the preset only, the same in Graphite
Light and Dark, as the board palette does (DECISIONS, the 2026-11-19 board 3D
view toolbar row).

**Screenshot**: a click on the camera copies the view to the clipboard;
the chevron's menu has Copy to clipboard, Save PNG… and Save with
transparent backdrop…. The toolbar,
popovers, toasts, status pill, hint and view cube hide, the next frame is
captured from the main window (`iced::window::screenshot`) and cropped to
the view in physical pixels, and the chrome shows again. Save PNG… asks for
the file first (the file-picker port) and writes the PNG atomically
(temporary file renamed over it) off the update thread. Messages says
whether the copy or the save worked, and why one failed (the clipboard's
reason, or the write error).

**Transparent screenshot** (mockup row 175): Save with transparent
backdrop… captures the view twice, on two frames with the chrome hidden,
first over flat black and then over flat white in place of the lighting
backdrop. A pixel the same in both is opaque; where white came through, its
share is the backdrop's, so the coverage is one less that share and the
colour is the black pass divided by the coverage (straight alpha). The PNG
is written like Save PNG…; when the view changed size between the passes
nothing is written and Messages says why. The mesh pipeline draws without
multisampling, so edges come out hard; only a translucent surface with the
backdrop behind it comes out partly transparent. While a capture is
under way, Flip, the next lighting preset, a new screenshot and a drag,
wheel or click on the view are ignored,
and a save picked meanwhile is not written (Messages says so); a pass that
comes back after the view was left is dropped with a Messages line.

**Keys** (mockup row 175): on a board `H` is the next lighting preset and
`Mod+Shift+C` copies a screenshot, in both built-in profiles; on a flat
board both commands (`cycle_board_3d_lighting`, `copy_board_3d_screenshot`)
are greyed with the reason that they need the 3D view. `F` is Flip
(`flip_board_3d`) in the `board_3d` shortcut context, live only while the
main window shows the board in 3D, where it outranks the board's `F`
(Change Footprint). Flip does nothing while the board is flat. The chrome
names each key the active keymap binds, as the mockup draws them: the
Flip, lighting and screenshot tooltips end in the key ("Flip board (F)"),
the menu's Copy row shows its key as a cap, and the lighting toast ends in
"· press H for the next" with the key as a cap; an unbound command shows
no key.

**Model-unread toast**: a part whose model could not be read adds its
`pcb-3d-model-unread` line to Messages and also shows it as a dismissible
warn toast at the bottom left, above the status pill (with how many more
could not be read), as wide as its text up to 420 px at the default body
size; dismissing it leaves the Messages lines.

**Selection** is the flat view's: a click casts a ray and selects the
footprint of the nearest part it meets before the board (on the board or on
nothing it clears the selection), and selected footprints are washed in the
selection colour.

**GPU.** `signex_gfx::pipeline::mesh::MeshPipeline` through iced's shader
widget (`board_3d_shader`), in its own render pass with its own depth buffer
over the lighting preset's backdrop; flat shading from the face's slope,
both faces lit by the preset's light from the eye. The mesh uploads only
when its generation changes.

Checks: `signex_gfx::scene3d::tests`, `signex_gfx::ring_fill::tests`, `signex_gfx::camera3d::tests`,
`signex_renderer::board3d::tests`, regression `pcb_board_3d` and
`pcb_board_3d_toolbar`, `styles::board_palette::tests`,
`app::handlers::board_3d_screenshot::tests`.

## sch-annotations — Bezier, dimension and table drawings (#1267, #1300)

Three drawing kinds that are annotation only: no connectivity, no ERC, drawn
and exported like the other drawings and ordered with them (Behind / Above,
Arrange). Their geometry is computed once in `signex-types`, so the canvas,
the hit box, SVG and PDF place every line and cell at the same spot.

**Bezier.** `SchBezier::flatten` gives 32 straight segments (both ends exact);
the canvas strokes them in the line style (`LineSegment` dash and dot bits),
SVG writes one cubic path when solid and the flattened dash marks otherwise.
`arrow_heads` gives a filled head per end `ends` asks for, pointing along the
end tangent (towards the control point). With no own colour the curve takes
the sheet's drawing colour (DECISIONS 2026-11-19).

**Dimensions.** `SchDimension::geometry` holds the dimension line (offset from
the measured points along the dimension's normal: `(0, 1)` horizontal, `(1, 0)`
vertical, the axis turned a quarter when aligned), the two extension lines
(0.5 mm gap at the point, 1 mm past the line), the heads (filled inside,
filled outside with tails past the extension lines, or open) and the text
anchor: the line's middle, 0.5 mm out on the side away from the points, the
text turned to read left to right or bottom to top. The text is the measured
value in the dimension's unit and decimals with the unit symbol (`12.70 mm`),
or the override. A turn maps the line with the points, so a horizontal
dimension turned a quarter becomes a vertical one of the same length.

**Tables.** `SchTable::layout` places the cells: a monospace advance of 0.6 em
per character, a column as wide as its longest cell or title (three
characters at least) plus the padding on both sides, every row `text size +
2 × padding` high, the title line above the grid. Hidden columns are left
out. The header row is filled with the sheet's net-label colour mixed 18 %
over the paper (`table_header_fill`) when Header fill is on; All draws every
cell edge, Outer the outline and the header rule, None no lines. Cell text is
in the value colour, a bound pin table's net cell that names a power port's
net in the power colour. A table never turns or mirrors: its centre follows
the transform.

**While placing.** The Bezier tool draws the clicked points, the control
lines and the curve through the cursor; a dimension tool draws the dimension
the cursor would place with its value in an accent tag set off the line on
its outer side (so it clears text next to the line), and a hint bar at the
canvas's bottom-left names the tool and the next click.

Checks: `schematic::drawing_kinds_tests`, `signex_output::svg::annotation_drawing_tests`,
`annotation_drawing_tests` (engine), `app::sch_tables::tests`.

## sch-not-fitted — Not-fitted parts (#1294)

Decided by Caner, 2026-11-25: one drawing style for now, a cross over the
part and a greyed body, behind a user preference (nothing is stored in the
project). A style picker comes later. The rest of this section is
implementation, not his decision: the preference is Preferences ▸ Schematic ▸
Display ▸ "Cross out not-fitted parts", on by default, `prefs.json` key
`cross_out_not_fitted`.

**Which parts.** A part is not fitted when the BOM would call it so: the
project's variant settings (Variant Manager) for the variant, else the
sheets' fit fields for it, else the base fit fields, else not fitted when a
unit is marked DNP. One unit not fitted makes every unit of the part not
fitted, on every sheet: a part's units are grouped across sheets as the BOM
groups them, and its settings entry is found by the BOM's part link. A PDF,
its preview and print decide across the hierarchy the export walks (for a project,
every sheet placement the BOM counts), pages left out of the printout
included; a page the walk never reaches is decided on its own. The canvas
decides across the project's open sheets (a unit on a sheet that is not open is not
counted). A repeated sheet shown once for all its channels crosses a part
out only when every channel's copy is not fitted. Power ports are never
parts. The canvas reads the project's active variant, and so do an undocked
window and an unfocused tile pane showing a sheet; a PDF, its preview and
print read the printed variant. `signex_output::PlacedFits` decides for all
of them.

**How it draws.** The part's body graphics, pins, pin names and numbers, its
library text and its default body fill are greyed: each colour moved 60 %
of the way to the paper colour (`signex_types::schematic::greyed_ink`). A
cross runs corner to corner over the body box (`not_fitted_cross`: the body
graphics, no pins, no text) in the theme's error ink
(`CanvasColors::erc_error`; decided by Caner, 2026-11-25), not greyed, twice
as wide as an unset symbol line. The designator, the value and the other fields keep
their colours, so the part still reads. A symbol whose library entry is
missing keeps its crossed placeholder box, greyed and crossed on paper too.
In Black & White the greyed ink
prints black like every other ink; the cross still marks the part.

Checks: `schematic::not_fitted_tests` (types), `variant_fit::drawn_tests`,
`signex_output::svg::not_fitted::tests`, `schematic_runtime::not_fitted_tests`,
`app::runtime::not_fitted_tests`, `handlers::menu::export::tests::not_fitted`.

## Canvas cursor

Decided by Caner, 2026-11-21. One rule for the schematic, board, footprint
editor and symbol editor canvases (`canvas::cursor::tool_cursor`):

- **Idle selection tool:** the OS arrow (`mouse::Interaction::Idle`).
- **A tool armed:** the crosshair, while a placement, drawing or routing
  tool or Measure is active, or a part, power port, paste or Edit ▸ Move
  waits for clicks. On the board that is the canvas's clicks-only state.
- **Gestures keep their own cursors:** grabbing while panning, move while
  dragging the selection or a board footprint, the resize cursor over a
  handle.
- **Footprint Sketch mode:** the Select tool shows the arrow and draws no
  mark of its own except the line-hover resize arrow; a drawing tool hides
  the OS cursor and its reticle is the crosshair.

Checks: `canvas::cursor::tests`.

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

Licensed under the same FSL-1.1-ALv2 terms as the rest of `signex`
(see `LICENSE`). This document is original Signex prose and may
be reproduced under those terms.
