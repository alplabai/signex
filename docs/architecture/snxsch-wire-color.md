# `.snxsch` wire colour and width presets

A wire may carry its own colour. `None` draws in the theme's wire colour.

## On disk

An optional `color` column of `[sheets.wires]`. It is not a known column: like
`locked` it rides the foreign-column slot.

- A coloured row writes the colour as `#rrggbbaa` (lower-case hex, alpha last).
- Every other row of the block writes an empty cell, which reads as no colour.
- The header appears only when at least one wire is coloured.
- On read, `#RRGGBB` and `#RRGGBBAA` are accepted. Any other non-empty value is
  a read error that names the block, the row and the value; it is not kept as a
  foreign cell.

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
