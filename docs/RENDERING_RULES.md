# Signex Schematic Rendering Rules

This document is the authoritative behavioural spec for the
`signex-render` crate's schematic renderer. It exists so the
renderer's source comments can reference a single, independent
spec instead of citing third-party implementations.

The rules below are derived from:

1. **Observation of how `.kicad_sch` files render** — i.e. opening
   files in any conforming reader, including non-Signex tools, and
   noting what users see on screen. The `.kicad_sch` format itself
   is a public file format documented at
   <https://dev-docs.kicad.org/en/file-formats/sexpr-schematic/>.
2. **Altium Designer parity goals** — Signex aims to feel
   familiar to Altium users, so where Altium and other readers
   diverge, the rules pick the Altium-shaped behaviour where
   feasible (see `docs/UX_REFERENCE_ALTIUM.md`).
3. **Public typographic / engineering standards** — IEEE-Std-91
   for pin shape decorators, ISO 3098 for technical text, etc.

The rules are written in our own terms and reference no third-party
implementation source code. Where a rule was inferred from
observed behaviour, that's noted.

---

## sch-labels — Label rendering

Source file: `crates/signex-render/src/schematic/label.rs`

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

Flag dimensions are derived from the rendered text height (`font_size`)
times empirically-tuned multipliers — the flag width is `text_width
+ 2 × pad`, height is `text_height × 1.6`, point depth is
`text_height × 0.5`. These multipliers live in `label.rs` as
named constants.

---

## field-rotation-and-justify — Field text under rotated symbols

Source file: `crates/signex-render/src/schematic/mod.rs`,
function `field_effective_style`.

A schematic symbol's reference / value / footprint / user fields
are stored with their own position and rotation independent of the
parent symbol. When the parent symbol is rotated or mirrored, two
adjustments must be applied to the field's stored angle and
justification so the text remains readable and stays anchored
sensibly relative to the symbol body.

### Rule 1 — Rotation folding

| Parent symbol rotation | Stored field angle | Drawn angle |
|---|---|---|
| 0° or 180° | any | stored angle, used directly |
| 90° or 270°, stored 0° | 0° | 90° |
| 90° or 270°, stored 90° | 90° | 0° |
| 90° or 270°, stored 180° | 180° | 90° |
| 90° or 270°, stored 270° | 270° | 0° |

The intent is: a vertically-rotated symbol (90° / 270°) still
displays its reference and value horizontally if at all possible,
because rotated text is harder to read. Only when the user
specifically authored a 90°-stored field does it stay rotated
relative to the page.

180° → 0° and 270° → 90° folding for stored field angles is a
readability optimisation: a "180°-rotated" reference looks
identical to a "0°-rotated" reference under the parent's flip,
and the latter is the conventional notation.

### Rule 2 — Justify-flip on axis flip

When the parent symbol's transform flips an axis (180° rotation,
mirror-X, mirror-Y, or any combination), the field's stored
horizontal / vertical justification must flip on the corresponding
axis too.

Concretely:

| Parent transform | H-justify flips? | V-justify flips? |
|---|---|---|
| 0° rotation, no mirror | no | no |
| 90° rotation | swap H ↔ V | swap H ↔ V |
| 180° rotation | yes | yes |
| 270° rotation | swap H ↔ V (with extra flip) | swap H ↔ V (with extra flip) |
| `mirror x` | yes (flipped Y axis) | no |
| `mirror y` | no | yes (flipped X axis) |

Without this, a `justify left` field stored to the *left* of a
180°-rotated symbol body would anchor on its left edge and visibly
grow back through the body. The flip rule is what keeps the
text anchored "outside" the body regardless of the parent's
orientation.

This behaviour is observable in any reader that opens a
`.kicad_sch` file — try authoring a symbol with reference
`U1` left-justified and rotating the symbol through 0°/90°/180°/270°
and you'll see the same behaviour we implement.

---

## pin-shape-decorators — Pin shape modifiers

Source file: `crates/signex-render/src/schematic/pin.rs`

Pin shape decorators draw graphical modifiers on top of the base
pin stroke to indicate the pin's electrical / logical role. The
catalog follows **IEEE-Std-91** (Graphic Symbols for Logic
Functions, IEEE 1984/2004) — the public standard used across EDA
tools and textbooks.

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

Bubble diameter, triangle size, and slash length are all derived
from the pin length so the decorators scale with pin size.
Specific multipliers live in `pin.rs` as named constants.

---

## What this document is *not*

- **It is not** a reverse-engineering of any third-party EDA tool's
  source code. The Signex codebase contains no copies of, and no
  structural ports of, any third-party renderer's source.
- **It is not** comprehensive — it only documents rendering rules
  that needed to be made explicit for the source comments to
  reference. Most renderer behaviour is self-documenting through
  the code itself.
- **It is not** versioned independently from the source — it lives
  alongside the renderer crate and updates in lock-step with code
  changes. Pin shape catalog gains a new shape → this doc gets a
  new row.

## License

Licensed under the same Apache-2.0 terms as the rest of `signex`
(see `LICENSE`). This document is original Signex prose and may
be reproduced under those terms.
