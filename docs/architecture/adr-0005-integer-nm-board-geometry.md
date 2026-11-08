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
8. **Flip.** `flip_footprint` swaps the sided layers of the footprint, its pads and its graphics (`PcbLayer::flipped`, top <-> bottom; inner copper and unknown tokens stay) and sets rotation to `-rotation` mod 360. Local coordinates are untouched. This is the turn about the footprint's vertical axis: `R(-t)·M = M·R(t)` for the x mirror `M`, so each pad lands at the mirror image of where it was.

## Rounding

Besides §5, a pad's half-width and half-height are `mm_to_nm(size) / 2`, which drops 0.5 nm when the size is an odd number of nm. That is below what the file stores.

## The renderer consumes `pcb-geom` (#2190)

`signex-renderer` (`pcb/emit.rs`, `pad_from_footprint`) takes a pad's world centre from `pcb_geom::pad_world_center` and its rotation from `Placement`, so a bottom-side pad is drawn at its mirrored centre and a ratsnest end or a click lands on what is drawn (#662, one lowering). A pad `pcb-geom` refuses is not drawn; it is in `BoardIndex::rejected`.

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
