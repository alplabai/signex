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
5. **Where `f64` still appears inside.** Three steps use `sin`/`cos` or a division and round back to the nearest nm: turning a point into a rotated pad's frame, turning a pad's corners (a round-rect's inset rectangle included), and the foot of a perpendicular on a track. Quarter turns are exact. The error is under 1 nm, below what the file stores. A round-rect's corner arc adds no float to a test: copper is the rectangle inset by the radius plus that radius, so hit-test, overlap and window select all compare an exact integer squared distance with the radius, and the bounding box is the inset rectangle grown by the radius. The radius itself is `round(ratio x 2 x short half side)`, clamped to the short half side, so it is within 0.5 nm of the ratio's value. The one place a round-rect arc is a float polyline is drawing (`Shape::outline` / `Shape::ring`, rounded to the nm per vertex); there the chord under-reads the arc by `radius x (1 - cos(pi / (4 x steps)))`, about 75 nm for a 0.25 mm radius at 32 steps, and no test reads it. Every comparison after the three rounding steps is integer.
6. **Hit-tests read the model, never the render output (#662).** The pad world transform (mirror for a bottom-side footprint, then rotate, then translate) is computed here from the footprint and pad fields.
7. **Footprint-local geometry is never mirrored in the model (maintainer decision, #2174).** A footprint's pads, graphics and properties keep their library-local coordinates. The side comes from the footprint's native layer token (`TopCopper` / `BottomCopper`, the `.snxpcb` names), parsed through `signex_types::pcb_layer::PcbLayer`; an unknown token or a layer with no side is the top. The mirror (local x negated) is applied only at world-transform time, in `pcb-geom`. No foreign-tool layer prefix is read anywhere.
8. **Flip.** `flip_footprint` swaps the sided layers of the footprint, its pads, its graphics and its properties (where a property has a layer, so a designator on `TopSilkscreen` goes to `BottomSilkscreen`) (`PcbLayer::flipped`, top <-> bottom; inner copper, mechanical layers and unknown tokens stay) and stores the rotation as `(-rotation) mod 360`. A host that must undo a flip exactly restores the saved rotation and layers rather than flipping again, since the mod is not bit-exact on every value. Local coordinates are untouched. This is the turn about the footprint's vertical axis: `R(-t)·M = M·R(t)` for the x mirror `M`, so each pad lands at the mirror image of where it was.

9. **Mechanical layers are board-wide (maintainer decision, 2026-11-08).** The fixed `User.1`..`User.6` layers are gone. `PcbLayer::Mechanical(n)`, `n` in 1..=32, has the on-disk token `Mechanical1`..`Mechanical32` (no sign, no leading zero). A name this build does not know, `User.N` included, stays a string and round-trips byte for byte. A mechanical layer does not flip. Its user-editable display name is `LayerDef::display_name`, saved as the optional `[stackup.display_names]` table (layer token -> display name, only for layers that have one, left out when there are none). It is keyed, not parallel to `layers`, so a build that reorders or drops layers cannot misalign it; a key naming no layer is kept and written back (see §12).
10. **A pad's net can be locked (maintainer decision, 2026-11-08).** `Pad::net_locked` is saved in the optional `net_locked` pad column (`true`, or an empty cell), written only when a pad is locked. A locked pad keeps its net number; a rename of that number in the net table follows (the pad takes the new name, number and lock unchanged). Net sync never otherwise changes a locked pad and reports it (`PadNetIssue::PadNetLocked`); a locked pad still counts when sync checks for a link shared by several footprints. Only on a footprint that is in the schematic does a pad lock itself, when it holds a net: a pad with no pad number, or a pad whose number matches no pin of the footprint's part (`PadNotOnSymbol`). A footprint whose link matches nothing in the schematic is reported (`FootprintNotInSchematic`) and none of its pads is locked. The sync's edit records the lock and its inverse restores the previous flag. `assign_pad_net` sets a net by hand and locks the pad; `set_pad_net_lock` locks or unlocks a pad and keeps its net. Both return an edit whose application returns the exact inverse.
11. **A footprint links to its schematic part by sheet path and unit UUIDs, never by designator (maintainer decision, 2026-11-08).** `Footprint::schematic_link` is `SymbolLink { sheet: InstancePath, units: Vec<Uuid> }`: the sheet occurrence plus the UUID of every unit of the part, sorted, without repeats (`SymbolLink::new`). A multi-unit part (a dual op-amp) is several symbol instances that share a designator and a library symbol and differ in `unit` (`signex_types::schematic::group_into_parts`), each with its own UUID and its own terminals in the netlist (`Terminal::symbol`, `Terminal::occurrence`); the link must name the part, so it lists all of them. Unit UUIDs and the sheet path do not change when the schematic is re-annotated, which is why they, and not a designator, are stored. A unit added to the part later is not in the link: its pins are reported (`TerminalWithoutPad`) until the footprint is linked again.
    - Format: the optional `sch_link` footprint column, `<unit uuid>[+<unit uuid>...]@<sheet path>` with units ascending, for example `0192...a001@/` or `0192...a002+0192...a003@/<sheet uuid>`. It is a foreign column like `net_locked`: absent when no footprint is linked (old files re-save byte-identical), an empty cell for an unlinked footprint in a block that has linked ones. A cell this build cannot read, or that would not write back identically, stays a foreign cell.
    - Sync: `SymbolPadKey::for_board` keys a pad by (sheet path, the part's anchor unit, pad number), the anchor being the smallest unit UUID, and keys a terminal by the anchor of the link that lists its symbol. A pin of any unit therefore reaches the same pad, and no designator is read. A footprint with no link is reported (`PadNetIssue::FootprintNotLinked`) and left completely untouched: no net change, no lock. Two different links that list the same unit are not supported: the first wins and the other footprint is reported as not in the schematic.
    - Migration: `link_footprints_by_designator(board, sheets)` is the one place a designator links a footprint, and only on request. It groups each placed sheet's symbols into parts with `group_into_parts` and sets `schematic_link` on an unlinked footprint only when exactly one part carries its reference and no other footprint holds or claims that part. Ambiguous and unmatched footprints are listed in the report and left alone, linked footprints are never touched, and the returned `BoardLinkEdit` applies to an exact inverse.
12. **Stackup display names keep what this build does not own.** `[stackup.display_names]` entries whose layer is missing, and values that are not strings, are kept (`BoardUnknown::stackup_display_names`) and written back, so an older or newer build's data survives a save. A layer's own name wins over a kept entry of the same token. Layer tokens must be unique: with two layers of one token only the first reads the table.
13. **A footprint records the library footprint it was built from (maintainer decision, Caner, 2026-11-09, #1312).** `Footprint::library_ref` is a `FootprintSourceRef { library, footprint, version }` (`signex-types::part`), set by `signex-library::resolve_part_footprint` and read by Update PCB from schematic to tell a changed or newer library footprint from the one placed. `None` for a footprint drawn on the board or saved before the field.
    - Format: the optional `lib_ref` footprint column, `<library uuid>:<footprint uuid>@<version>`, uuids hyphenated lower-case, the version not empty. Like `sch_link` it is absent when no footprint carries one (old files re-save byte-identical), and an empty cell for a footprint without one in a block that has some. Unlike `sch_link`, a malformed cell is a load error naming the field and the value, not a foreign cell: a wrong library identity would update the footprint from the wrong part. A save refuses an identity that would not read back (an empty version), so the writer never makes a board the reader refuses.
    - Conversion: `signex-library::board_footprint` builds the board footprint from the library footprint's stored pads, silkscreen, fab and courtyard without running the sketch solver. Each layer token is read into a `PcbLayer` (a board token as itself, the library editor's tokens by a fixed table); an unknown token, a pad shape or drill the board cannot hold, pad settings the board has no field for (tenting, paste off, no thermal relief, features, test points, electrical type) and a footprint with pours, keepouts, cutouts, v-scores, standalone mask or paste shapes, a 3D body, a STEP model or an anchor are refused (`FootprintResolveError::Unconvertible`), never approximated.

## Rounding

Besides §5, a pad's half-width and half-height are `mm_to_nm(size) / 2`, which drops 0.5 nm when the size is an odd number of nm. That is below what the file stores.

## The renderer consumes `pcb-geom` (#2190)

`signex-renderer` (`pcb/emit.rs`, `pad_from_footprint`) takes a pad's world centre from `pcb_geom::pad_world_center` and its whole outline (shape, size and turn, the pad's own rotation composed with the footprint's and mirrored on the bottom side) from `pcb_geom::pad_shape`, not a rotation from `Placement`, so a bottom-side pad is drawn at its mirrored centre and a ratsnest end or a click lands on what is drawn (#662, one lowering). A pad `pcb-geom` refuses is not drawn; it is in `BoardIndex::rejected`.

### Drawn size

The renderer floors a pad's drawn size: a zero or negative side draws as `PCB_DEFAULT_PAD_SIZE_MM`, and no side is under 0.05 mm. This stays a renderer rule, not geometry: `pad_shape` keeps the model's size (a zero-size pad has no copper to hit), while the drawing needs something visible to select. The floor changes no centre or angle, so it cannot move a ratsnest end.

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
