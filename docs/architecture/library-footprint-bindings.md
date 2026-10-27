# Alternate footprints — overloaded `footprint_ref` cell

Issue #1234. Status: codec in `signex-library::footprint_binding`, wired into `ComponentRow.alternate_footprints` (skipped while empty, so old rows keep their bytes and hash). `ROW_SCHEMA` 3 marks the field: an older server is refused rather than allowed to drop it. The row field is `alternate_footprints` (`alternates` already means alternate MPNs).

## Cell shapes

The component table's `footprint_ref` cell holds one of:

| Cell | Meaning |
|---|---|
| empty | no footprint |
| JSON object `{"library_id":..,"uuid":..}` | default footprint only; byte-identical to the pre-#1234 cell |
| JSON array `[default, alt1, ...]` | default plus alternates; every element is a `FootprintBinding` |

`FootprintBinding` is `{footprint, version?, pin_map_overrides?}`; empty `version` and `pin_map_overrides` are omitted, and unknown keys are rejected.

## Rules

- Element 0 is the default. It carries no `version` and no `pin_map_overrides`: those stay in the row's existing columns. A reader rejects element 0 if it has either.
- The object form is never extended with extra keys. `PrimitiveRef` does not deny unknown fields, so an older build would silently drop them; an array makes older builds fail loudly instead.
- Invariants, enforced on both encode and decode: no alternates without a default, no empty array, no one-element array (use the object form), no `PrimitiveRef` (library_id + uuid) bound twice.
- Encode and decode are a strict bijection on valid input. Leading whitespace is tolerated on read, as before.

## Selection

`mount_style()` on a footprint derives `Smd`, `ThroughHole`, `Mixed` or `Unknown` from its `Smd` and `Tht` pads only; other pad kinds (mounting hole, connector, castellated, fiducial) are ignored. `pick_binding(styles, preference)` takes, for each preferred style in order, the first binding with exactly that style. `Mixed`, `Unknown`, an empty preference or no match fall back to index 0, the default.
