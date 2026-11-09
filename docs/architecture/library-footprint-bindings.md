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

## Choosing a footprint (#1234)

The Components panel details view lists the default and the alternates; the pick lives in `ComponentsPanelState::footprint_pick` and Place builds the part with it. A part built with an alternate records it in its value stamp as `footprint_ref = "<library uuid>:<footprint uuid>"` (`[extras.symbols.<uuid>.library_stamp]`); the key is absent for the row's default, so older files load as the default and a build that does not know the key keeps it as an unknown key. Library drift reads the footprint name of that binding, not the row default. Placing from the library picker still uses the row default. Properties changes it on a placed part (a dropdown above the footprint text field) with `Command::SetSymbolFootprintBinding` (one undo step). A project-level preferred package is not part of this; `pick_binding` stays unused until it is.

## Board footprints (#663, #1312)

`resolve_part_footprint(row, chosen, lookup)` (`signex-library::part_footprint`) turns a part into a board footprint. `chosen` is the binding the part was placed with (`None` is the default); a binding the row does not offer is refused (`NotOffered`). `lookup` reads the library footprint by address and the binding's pinned version (empty = latest); a pinned version the library does not hold is `VersionMismatch`, and a footprint whose identity would not read back from the `lib_ref` cell (an empty version) is refused, as the save would be. The pin map is the row's `pin_map_overrides`; for an alternate its own overrides take priority pin by pin. The result is a `ResolvedFootprint`: the board footprint with `library_ref` set (ADR-0005 §13), its `FootprintSourceRef` and the `PinPadMap`, which `signex_net::board::SymbolPadKey::with_pin_maps` reads so a pin lands on its mapped pad. The key stores a part's map under every unit of the `DesignPart`, so a terminal finds it whichever unit it sits on and whatever units the footprint link lists; a mapped pad is trimmed like a pin, and a blank one links nothing.

`board_footprint` reads what the footprint stores and never runs the sketch solver:

- Pads: `Smd` and `Fiducial` are surface pads, `Tht` and `Castellated` plated holes, `NptHole` a non-plated hole, `ConnectorPad` a connector pad. `Round`, `Rect`, `Oval` and `RoundRect` (ratio 0..=0.5) map to the board's shapes; `Chamfered`, `Custom`, a slotted drill and a copper offset are refused. Size, position, rotation and drill diameter are copied as stored, in the same footprint-local frame. Refused when not the default: a tented side, paste turned off on a side, no thermal relief, a top or bottom pad feature, test-point flags and a non-default electrical type. Solder mask and paste margins (global and per side) are the one thing not carried: the board model has no field for them yet, so the board's own rules apply.
- Layers: a board token (`TopCopper`) reads as itself; `F.`/`B.`/`*.` `Cu`, `Mask`, `Paste`, `F.`/`B.` `SilkS`, `Fab`, `CrtYd`, `Adhes`, `Edge.Cuts` and `In<n>.Cu` read by a fixed table. Any other token refuses the footprint (`Unconvertible`).
- Graphics: `silk_f`, `silk_b`, `fab_f`, `fab_b` go to the top and bottom silkscreen and fab layers, the courtyard polygon to `TopCourtyard`. A framed text is refused, since the board text has no frame.
- Pours, keepouts, cutouts, v-scores, standalone mask or paste shapes, a 3D body other than the default, a STEP model and a non-zero anchor refuse the footprint: dropping them would place something else than the library holds.
- The board footprint is top side at the origin with empty Reference and Value texts, and it and its pads get fresh uuids.
