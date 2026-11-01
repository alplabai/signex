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

Source: `crates/signex-types/src/schematic/stroke.rs`, applied by the CPU
replay (`renderer_scene_canvas::SceneDrawOptions::stroke_px`), the GPU
surface floor (`scene_shader::SchematicSurface`) and the Symbol Editor.

1. **Every stroke is a world width in mm.** Wires use their own or their net
   class width, symbol bodies their library width, pins the symbol's library
   width (the most common width among its stroked graphics), buses 0.45 mm,
   the no-connect X and No-ERC diamond 0.2 mm, global / hierarchical label
   markers and child sheet frames 0.15 mm. A stored width of `0` and
   anything thinner draw at 0.15 mm on screen. These widths do not change
   with zoom.
2. **Screen width is `mm * scale`**, so the ratio between any two kinds is the
   same at every zoom. Before #1858 each kind had its own curve (a per-kind
   px width times zoom, clamped), and pins ignored their library width, so
   the wire : pin : body weights changed from one zoom step to the next.
3. **One floor.** A stroke thinner than one device pixel draws at one device
   pixel, the same for every kind and on both the CPU and GPU paths. It is a
   whole pixel because the CPU canvas has no anti-aliasing: body outlines,
   circles and diagonals are not snapped, and at 0.76 px (a 0.254 mm body at
   100 %) whole edges of a rectangle missed every pixel centre and vanished.
4. **Then the whole-pixel snap** (#1674, `scene_line_snap.rs`) rounds
   axis-aligned hairlines under 4 device pixels to whole device pixels.
5. **Display scale.** The floor and the snap are defined in device pixels.
   The CPU canvas does not receive the window's scale factor yet, so both
   still run with one device pixel per logical pixel
   (`UNSCALED_DEVICE_PX_PER_LOGICAL_PX`) and the GPU path matches it.
6. **Output** (PDF, SVG, print) uses mm widths only, with no pixel floor and
   no snap. Output keeps each item's stored width (a width of `0` gets the
   exporter's default); the 0.15 mm minimum of rule 1 is the screen rule.

The mm values for markers, labels and sheet frames were picked so the
drawing looks the same as before at 400 % zoom after the snap. Selection
outlines, handles and axes are screen-constant chrome, not strokes of the
drawing, and stay a fixed pixel width.

---

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
