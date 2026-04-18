# KiCad Worksheet Summary

Official source: KiCad Developer Docs, Work Sheet File Format.

## File Header

```scheme
(kicad_wks
  (version YYYYMMDD)
  (generator "your-tool")
  ...
)
```

- File extension: `.kicad_wks`
- Third-party tools must not use `pl_editor` as the generator.

## Precision

- Worksheet coordinates use micrometer internal resolution.
- Maximum effective precision is 3 decimal places in millimeters.

## Setup Section

```scheme
(setup
  (textsize WIDTH HEIGHT)
  (linewidth WIDTH)
  (textlinewidth WIDTH)
  (left_margin DISTANCE)
  (right_margin DISTANCE)
  (top_margin DISTANCE)
  (bottom_margin DISTANCE)
)
```

## Drawing Objects

Supported top-level worksheet drawing objects include:

- `tbtext`
- `line`
- `rect`
- `polygon`
- `bitmap`

Objects are ordered as added to the worksheet.

## Repetition and Incrementing

Most worksheet objects support:

- `repeat COUNT`
- `incrx DISTANCE`
- `incry DISTANCE`

Corner anchors can be:

- `ltcorner`
- `lbcorner`
- `rbcorner`
- `rtcorner`

## Bitmap Notes

- Worksheet images use `bitmap`, not the common `image` token.
- Embedded PNG payload is stored under `pngdata`.
- Raw data chunks are hex-encoded and split across `data` tokens when needed.

## High-Value Pitfalls

- Worksheet object coordinates and increment anchors follow page-corner semantics, not schematic object semantics.
- Repeated objects depend on the chosen corner token, so preserving that token matters for round-trip correctness.
