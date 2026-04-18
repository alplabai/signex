# KiCad Footprint Library Summary

Official source: KiCad Developer Docs, Footprint Library File Format and board-common S-expression syntax.

## File Header

```scheme
(footprint "NAME"
  (version YYYYMMDD)
  (generator "your-tool")
  ...
)
```

- File extension: `.kicad_mod`
- A `.kicad_mod` file contains exactly one footprint.
- Third-party tools must not use `pcbnew` as the generator.

## Core Footprint Structure

A footprint may contain:

- placement and layer data
- description and tags
- properties
- clearance and paste/mask settings
- attributes
- text and graphic items
- pads
- zones
- groups
- 3D model references

## Required Text Objects

At minimum, a footprint should define:

- `fp_text reference`
- `fp_text value`

## Pad Rules

Pad token shape:

```scheme
(pad "NUMBER" TYPE SHAPE
  (at X Y [ANGLE])
  (size X Y)
  [(drill ...)]
  (layers ...)
  ...
)
```

Valid pad types:

- `thru_hole`
- `smd`
- `connect`
- `np_thru_hole`

Valid pad shapes:

- `circle`
- `rect`
- `oval`
- `trapezoid`
- `roundrect`
- `custom`

## Attributes

Footprint `attr` supports:

- `smd`
- `through_hole`
- optional `board_only`
- optional `exclude_from_pos_files`
- optional `exclude_from_bom`

## Version-Sensitive Notes

- Before KiCad 7, many footprint graphics used direct `width` instead of `stroke`.
- Before KiCad 6, strings were only quoted when necessary.
- Earlier formats referred to a placed footprint as `module` instead of `footprint`.

## High-Value Pitfalls

- Use `uuid` for footprint objects, but board tracks and vias still use `tstamp`.
- `fp_*` graphic items are valid only inside a footprint definition.
- If a text box uses a non-cardinal angle, KiCad expects a `pts` rectangle instead of `start` and `end`.
