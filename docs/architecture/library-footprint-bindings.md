# Alternate footprints — overloaded `footprint_ref` cell

Issue #1234. Status: codec in `signex-library::footprint_binding`, wired into `ComponentRow.alternate_footprints` (skipped while empty, so old rows keep their bytes and hash). `ROW_SCHEMA` 5 marks the field (2 = alternate symbols, 3 = labelled documents, 4 = description). The library server refuses a row write from a client below the current schema (code `row_schema_too_old`), and a client refuses to write to a server that reports a lower `row_schema`, so neither side drops alternates. As a last line, a client that sees a server echo different alternates **reports** the error. **Do not downgrade the library server once rows carry alternates**: a downgraded server has no schema guard for the field and drops it silently on the next write.

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

## Write paths

Every write path runs `ensure_footprint_bindings_valid` through `validate_row_for_write` (alternates need a default, no footprint bound twice): the TSV and `.snxlib` codec, the database client's insert and update, and the server's create and update (400 on failure). Nothing is merged server-side: an older client is refused, never patched up.

The cascade (`cascade.rs`) finds a footprint bound only as an alternate and moves that alternate's own `version` pin when it has one; an alternate with no version follows latest and has no pin to move. `dangling_row_refs` reports each missing alternate under `RefSlot::AlternateFootprint`.

## Not yet

`diff.rs` still reads only `footprint_ref`: a change to alternates alone is not shown in a row diff. It has no production caller yet. (`where_used.rs` lists alternate footprints as uses.)
