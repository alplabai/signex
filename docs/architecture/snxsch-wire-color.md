# `.snxsch` wire colour and width presets

A wire may carry its own colour. `None` draws in the theme's wire colour.

## On disk

An optional `color` column of `[sheets.wires]`. It is not a known column: like
`locked` it rides the foreign-column slot.

- A coloured row writes the colour as `#rrggbbaa` (lower-case hex, alpha last).
- Every other row of the block writes an empty cell, which reads as no colour.
- The header appears only when at least one wire is coloured.
- On read, `#RRGGBB` and `#RRGGBBAA` are accepted. Any other non-empty value is
  not an error: the colour is cosmetic, so the load succeeds, the wire draws
  uncoloured, and the cell stays a foreign cell that a save writes back
  unchanged. Each one is reported to Messages through the load-repairs path,
  naming the file, the wire's row and the value.

The format token was deliberately not bumped for this column. An older build
does not know it either: it keeps the cell through the foreign-column path and
writes it back, so a coloured wire survives a round trip through it (the wire
draws uncoloured there). A later build may widen the colour syntax the same
way without a bump, and this build then still opens the file.

## Which colour wins

Wire colour, then the per-net colour (Active Bar net colours), then the net
class colour, then the theme's wire colour. The canvas, SVG and PDF export all
resolve it this way (`schematic_runtime::wire_color`, `svg::wire_style::wire_rgb`).
Decided by the maintainer 2026-11-08 (docs/DECISIONS.md).

## Width presets

The Tab form and the wire Properties offer `Smallest / Small / Medium / Large`
(`WireWidthPreset`). They store `Wire::stroke_width`: Smallest 0.15 mm (the
render floor), Small 0 (unset, so it draws at the default 0.254 mm and a
project or net class width still applies; this is the default choice), Medium
0.5 mm, Large 1.0 mm. A stored width that matches none shows as the nearest
preset.
