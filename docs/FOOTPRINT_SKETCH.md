# Footprint sketch

## Bake ownership

A sketch bakes into footprint fields that can also be authored without it:
a `.snxfpt` written by hand or by an import, silk drawn in Pads mode. An
empty bake must therefore not clear every field. The sketch **owns** a
kind when either holds:

- an entity of the sketch is tagged for it (`silk` on `TopSilk` owns
  `silk_f`, on `BottomSilk` owns `silk_b`; `courtyard`, `mask_opening`,
  `mask_exclude`, `paste_aperture`, `pour`, `keepout`, `board_cutout`,
  `v_score` own the kind of the same name), or
- an earlier bake produced it and recorded it in `baked_kinds`.

A non-empty bake replaces the field and records the kind. An empty bake
clears a field only when the sketch owns the kind, reports the clear in the
Messages panel, and drops the record so geometry drawn afterwards is safe.
Pads are always sketch-owned once the sketch has an entity.

A non-empty silk bake still replaces the whole of `silk_f` / `silk_b`,
hand-drawn entries included: baked entries are not marked by origin.

### File format

`sketch.baked_kinds` is a list of strings, written only when non-empty and
read with a default, so a file the sketch never baked saves byte-identical
and an older file loads. Tokens: `silk_f`, `silk_b`, `courtyard`,
`mask_openings`, `mask_excludes`, `paste_apertures`, `pours`, `keepouts`,
`cutouts`, `v_scores`. A token is never renamed or reused for another
meaning; a token this build does not know is kept as read.

## Redundant constraints

`FullSolveOutput.redundant_constraints` names consistent constraints whose
every Jacobian row is implied by the other constraints (and the arc
closures). They are listed under "Redundant" in the footprint editor's
DOF / Last solve section. Conflicts are reported separately as
over-constrained.
