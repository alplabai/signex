# KiCad S-Expression Intro Summary

Official source: KiCad Developer Docs, S-Expression Format.

## Core Rules

- All tokens are lowercase.
- Token names cannot contain whitespace or special characters other than underscore.
- Strings are UTF-8 and double-quoted.
- Units are millimeters.
- Exponential notation is not used.
- Coordinates are relative to the containing object origin.

## Precision

- PCB and footprint precision: 6 decimal places max.
- Schematic and symbol precision: 4 decimal places max.
- Worksheet precision: 3 decimal places max.

## Common Tokens

- `at`: `(at X Y [ANGLE])`
- `pts`: `(pts (xy X Y) ...)`
- `stroke`: `(stroke (width W) (type TYPE) (color R G B A))`
- `effects`: font, justification, and optional hide flag
- `paper`: standard page size or custom width/height
- `title_block`: title, date, rev, company, comments 1..9
- `property`: `(property "KEY" "VALUE")`
- `uuid`: KiCad uses UUID v4 values
- `image`: embedded PNG payloads

## Generator Guidance

Third-party tools should not impersonate KiCad generators.

Reserved generator names include:

- `eeschema` for schematic files
- `kicad_symbol_editor` for symbol libraries
- `pcbnew` for board and footprint files
- `pl_editor` for worksheet files

## High-Value Pitfalls

- Symbol text angles are stored in tenths of a degree; most other angles are stored in degrees.
- Canonical layer names are always English even if the UI shows custom names.
- Wildcard layer forms such as `*.Cu` only apply to canonical layer names.
