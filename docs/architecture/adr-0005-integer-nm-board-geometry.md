# ADR-0005 — Board geometry computes in integer nanometres; the model stays `f64` mm

- **Status:** Accepted (2026-11-07, maintainer decision on #2174)
- **Scope:** `pcb-geom` — the board geometry crate that connectivity, ratsnest, hit-test and later DRC read
- **Relates to:** #1058 (one `Distance` length type), #662 (one domain-to-drawable lowering), #394 (the unused `Coord`/`Vec2` pair deleted)
- **Tracking:** #2174

## Context

The board model (`signex_types::pcb`) holds every position and size as `f64` mm, like the schematic. On disk the `.snxpcb` TSV columns are `i64` nm, converted in `signex-types` `format/units.rs`. Nothing in memory is in nm today; #394 deleted the last in-memory nm type because nothing used it.

Board geometry asks questions that floating point answers badly:

- **Equality and order.** Two pads at "the same" point, a track end on a pad centre, a tie between two equal ratsnest edges. In `f64` mm, `3.81` loads back as `3.8099999999999996`, so the answer depends on whether the board came from a file or from an edit.
- **Orientation and intersection.** Whether a track crosses a marquee edge is the sign of a cross product. Near zero, an `f64` sign is noise.
- **Determinism.** A ratsnest or a DRC report that changes between two runs on the same board is a bug report. Integer arithmetic gives the same answer on every OS and every build.

## Decision

1. **`pcb-geom` computes in integer nm.** Coordinates and lengths are `i64` nm (`PointNm`, `RectNm`). Every product (squared distance, dot, cross, R-tree area) is `i128`, so none overflows.
2. **The model stays `f64` mm.** No board type changes, no file format changes. `pcb-geom` reads the model and converts at its boundary.
3. **One conversion, the file's.** `mm_to_nm` rounds with `signex_types::format::mm_as_stored_nm`, the rounding a save applies. Geometry built from a freshly edited board and from the same board saved and reopened is identical to the nm. `nm_to_mm` uses the loader's arithmetic.
4. **The boundary refuses, it does not clamp.** NaN, an infinity, or a length beyond 1 km (`MAX_ABS_NM`) is a `GeomError` naming the model field. The board index lists every item it could not convert (`BoardIndex::rejected`) instead of dropping it silently. The 1 km bound is what keeps every `i128` product in range; no board comes near it.
5. **Where `f64` still appears inside.** Two steps use `sin`/`cos` or a division and round back to the nearest nm: turning a point into a rotated pad's frame, and the foot of a perpendicular on a track. Quarter turns are exact. The error is under 1 nm, below what the file stores. Every comparison after that is integer.
6. **Hit-tests read the model, never the render output (#662).** The pad world transform (mirror for a bottom-side footprint, then rotate, then translate) is computed here from the footprint and pad fields.
7. **Footprint-local geometry is never mirrored in the model (maintainer decision, #2174).** A footprint's pads, graphics and properties keep their library-local coordinates. The side comes from the footprint's native layer token (`TopCopper` / `BottomCopper`, the `.snxpcb` names), parsed through `signex_types::pcb_layer::PcbLayer`; an unknown token or a layer with no side is the top. The mirror (local x negated) is applied only at world-transform time, in `pcb-geom`. No foreign-tool layer prefix is read anywhere.
8. **Flip.** `flip_footprint` swaps the sided layers of the footprint, its pads, its graphics and its properties (where a property has a layer, so a designator on `TopSilkscreen` goes to `BottomSilkscreen`) (`PcbLayer::flipped`, top <-> bottom; inner copper, mechanical layers and unknown tokens stay) and stores the rotation as `(-rotation) mod 360`. A host that must undo a flip exactly restores the saved rotation and layers rather than flipping again, since the mod is not bit-exact on every value. Local coordinates are untouched. This is the turn about the footprint's vertical axis: `R(-t)·M = M·R(t)` for the x mirror `M`, so each pad lands at the mirror image of where it was.

9. **Mechanical layers are board-wide (maintainer decision, 2026-11-08).** The fixed `User.1`..`User.6` layers are gone. `PcbLayer::Mechanical(n)`, `n` in 1..=32, has the on-disk token `Mechanical1`..`Mechanical32` (no sign, no leading zero). A name this build does not know, `User.N` included, stays a string and round-trips byte for byte. A mechanical layer does not flip. Its user-editable display name is `LayerDef::display_name`, saved as the optional `[stackup.display_names]` table (layer token -> display name, only for layers that have one, left out when there are none). It is keyed, not parallel to `layers`, so a build that reorders or drops layers cannot misalign it; a key naming no layer is dropped on load.
10. **A pad's net can be locked (maintainer decision, 2026-11-08).** `Pad::net_locked` is saved in the optional `net_locked` pad column (`true`, or an empty cell), written only when a pad is locked. A locked pad keeps its net number; a rename of that number in the net table follows (the pad takes the new name, number and lock unchanged). Net sync never otherwise changes a locked pad and reports it (`PadNetIssue::PadNetLocked`); a locked pad still counts when sync checks for a link shared by several footprints. Only a pad the schematic cannot speak for locks itself, when it holds a net: a pad with no pad number, or, on a footprint linked to a symbol, a pad whose number matches no pin of that symbol (`PadNotOnSymbol`). The sync's edit records the lock and its inverse restores the previous flag. `assign_pad_net` sets a net by hand and locks the pad; `set_pad_net_lock` locks or unlocks a pad and keeps its net. Both return an edit whose application returns the exact inverse.
11. **A footprint links to its schematic symbol by sheet path and symbol UUID, never by designator (maintainer decision, 2026-11-08).** `Footprint::schematic_link` is `SymbolLink { sheet: InstancePath, symbol: Uuid }`, the same identity a netlist `Terminal` carries (`occurrence`, `symbol`). It is saved in the optional `sch_link` footprint column as `<symbol uuid>@<sheet path>` (absent when no footprint is linked; an empty cell for an unlinked footprint in a block that has linked ones; a cell this build cannot read stays a foreign cell). Net sync keys a pad by (sheet path, symbol UUID, pad number) (`SymbolPadKey`). A footprint with no link (placed by hand, or on a board made before the link existed) is matched for that sync by its reference when exactly one schematic symbol carries that designator, and is reported with `PadNetIssue::FootprintNotLinked` either way: the designator match is a fallback for old boards, is not stored, and a re-annotation breaks it. A linked footprint whose symbol is not in the schematic is reported (`FootprintNotInSchematic`) and its pads are not locked.

## Rounding

Besides §5, a pad's half-width and half-height are `mm_to_nm(size) / 2`, which drops 0.5 nm when the size is an odd number of nm. That is below what the file stores.

## Open: the renderer must consume `pcb-geom`

`signex-renderer` (`pcb/emit.rs`, `pad_from_footprint`) still builds its own pad centre and size: it does not mirror a bottom-side pad and it floors a zero size at `PCB_DEFAULT_PAD_SIZE_MM`. Until it takes both from `pcb-geom`, a ratsnest end or a click on a bottom-side pad can land where nothing is drawn. **The renderer must consume `pcb-geom` before the first consumer of `pcb-geom` merges** (#662, one lowering); a "kept in step" pair is not allowed. This unit does not change the renderer.

## Ownership of item identity

`BoardIndex::build` lists a repeated `ItemRef` (two pads with one footprint and pad UUID) in `rejected()` as `GeomError::DuplicateItem`. `from_items` leaves uniqueness to its caller.

## Relation to #1058

#1058 proposes one `Distance` value type for the `f64` mm model: units, formatting and parsing for the editor and the property panels. This ADR does not replace it and does not conflict with it:

- `Distance` is the **model's** length: what a user types and sees.
- `PointNm` / `i64` nm is **board geometry's working unit**: what connectivity and DRC compare.

When `Distance` lands, `pcb-geom`'s boundary takes `Distance` instead of a raw `f64`, and the conversion stays `mm_as_stored_nm`. Nothing else in the crate changes.

## Consequences

- Board connectivity, ratsnest and DRC are deterministic and agree with the saved file.
- Each consumer converts once, at the index or query boundary. A consumer that needs mm back (a ratsnest line to draw) converts with `nm_to_mm`.
- An item with a non-finite or out-of-range coordinate cannot be hit-tested; the caller sees it in `rejected` and reports it.
- `pcb-geom` depends on `signex-types` only (plus `rstar` and `uuid`). It is a domain crate: no UI or GPU dependency (`scripts/check_arch_guard.sh`).
