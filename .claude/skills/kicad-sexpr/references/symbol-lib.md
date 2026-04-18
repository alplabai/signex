# KiCad Symbol Library Summary

Official source: KiCad Developer Docs, Symbol Library File Format and shared schematic/symbol syntax.

## File Header

```scheme
(kicad_symbol_lib
  (version YYYYMMDD)
  (generator "your-tool")
  ...
)
```

- File extension: `.kicad_sym`
- Third-party tools must not use `kicad_symbol_editor` as the generator.

## Parent Symbol Rules

A top-level parent symbol uses:

```scheme
(symbol "LIBRARY_ID"
  [(extends "PARENT_ID")]
  [(pin_numbers hide)]
  [(pin_names [(offset OFFSET)] hide)]
  (in_bom yes|no)
  (on_board yes|no)
  PROPERTIES...
  GRAPHICS...
  PINS...
  UNITS...
)
```

## Mandatory Parent Properties

These properties belong on the parent symbol and must have these ids:

- `Reference` -> `id 0`
- `Value` -> `id 1`
- `Footprint` -> `id 2`
- `Datasheet` -> `id 3`

`Footprint` and `Datasheet` may be empty.

## Reserved Property Keys

Do not emit user-defined properties with these keys:

- `ki_keywords`
- `ki_description`
- `ki_locked`
- `ki_fp_filters`

## Child Unit Rules

- Child unit ids have the form `NAME_UNIT_STYLE`.
- `UNIT=0` means common to all units.
- Only body styles `1` and `2` are valid.
- Child unit symbols cannot define symbol properties.
- `unit_name` is only valid on child unit symbols.

## Pin Rules

Valid electrical types:

- `input`
- `output`
- `bidirectional`
- `tri_state`
- `passive`
- `free`
- `unspecified`
- `power_in`
- `power_out`
- `open_collector`
- `open_emitter`
- `no_connect`

Valid graphic styles:

- `line`
- `inverted`
- `clock`
- `inverted_clock`
- `input_low`
- `clock_low`
- `output_low`
- `edge_clock_high`
- `non_logic`

## Practical Notes

- If `pin_names` exists without an explicit offset, KiCad defaults to `0.508 mm`.
- Pin rotations are limited to `0`, `90`, `180`, and `270`.
- For third-party round-trip safety, preserve parent properties even if your renderer does not use them.
