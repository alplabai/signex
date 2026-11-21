# Footprint sketch

## Bake ownership

A sketch bakes into footprint fields that can also be authored without it:
a `.snxfpt` written by hand or by an import, silk drawn in Pads mode. An
empty bake must therefore not clear every field. The sketch **owns** a
kind when either holds:

- an entity of the sketch is tagged for it (`courtyard`, `mask_opening`,
  `mask_exclude`, `paste_aperture`, `pour`, `keepout`, `board_cutout`,
  `v_score` own the kind of the same name), or
- an earlier bake produced it and recorded it in `baked_kinds`.

A non-empty bake replaces the field and records the kind. An empty bake
clears a field only when the sketch owns the kind, reports the clear in the
Messages panel, and drops the record so geometry drawn afterwards is safe.
Pads are always sketch-owned once the sketch has an entity, and are
recorded as `pads` so that deleting every entity clears them too. A sketch
that never baked has an empty record and owns nothing: entering Sketch mode
on a footprint with literal pads leaves them alone.

Legacy gap: a footprint saved by an older build whose tagged entity was
already deleted has no record and keeps that geometry (it cannot be told
from imported geometry; there is no migration). The same holds for pads: if
the first edit in a newer build deletes every entity of a sketch baked by an
older build, the sketch-baked pads are kept, as they were before.

### Silk is owned entry by entry

Silk (`silk_f`, `silk_b`) shares its layers with hand-drawn entries (Place
String, text frames, the Pads-mode line, arc and polygon tools), so it is
not owned as a whole field. Every entry a bake makes carries the id of the
sketch entity it came from (`sketch_entity`); an entry without one is
hand-drawn. Every bake, empty or not, and the clear after every entity is
deleted, rebuild each silk layer the same way: the bake's output first,
then every unmarked entry in its existing order. An entry is removed
because it carries a marker, never by looking its entity up, so an entry
whose entity is gone goes too. The merge lives in
`signex_bake::silk_ownership`; the editor moves the Properties selection
with its entry.

A baked entry belongs to the sketch: it can be selected, Properties shows
it read-only ("From sketch: edit in Sketch mode"), and Pads-mode edits and
deletes of it are refused with a Messages line, since the next bake would
undo them.

Silk is no longer recorded in `baked_kinds`. The `silk_f` / `silk_b`
tokens stay reserved; a bake drops one an older build wrote.

**Files from older builds.** A file written before the marker existed has
no marked silk. A layer is marked all or nothing: this build marks every
entry it bakes, and an older build drops every marker when it saves. So
when a footprint opens in the editor and its sketch tags a silk layer that
holds entries but no marked one, the editor solves the sketch as loaded,
before any edit, and bakes its silk once. A layer with a marker was baked
by this build and is not classified again. Each unmarked entry that
matches a bake output (same kind, stroke width and fill,
geometry equal at the file's nanometre resolution, `mm_as_stored_nm`) is
marked; each output is used at most once, and an entry that matches
nothing stays hand-drawn. So a legacy hand entry equal to a bake output at
that resolution, with no old baked entry consuming the output, is claimed as
baked and a later re-bake replaces it. The markers are written in memory only: the
document is not dirtied and a save treats the footprint as unchanged until
the user edits it; the save's merge compares footprints without the
markers (`Footprint::same_ignoring_silk_markers`), so they are never
reported as replacing another window's change. If the sketch does not
solve, nothing is marked or removed. Either that, or an entry and a bake
output both left unmatched in one layer, puts a Messages warning that the
next sketch edit may duplicate that silk. An older build that saves the
file again drops the markers, and the next open classifies it again.

### File format

`sketch.baked_kinds` is a list of strings, written only when non-empty and
read with a default, so a file the sketch never baked saves byte-identical
and an older file loads. Tokens: `silk_f`, `silk_b`, `courtyard`,
`mask_openings`, `mask_excludes`, `paste_apertures`, `pours`, `keepouts`,
`cutouts`, `v_scores`, `pads`. A token is never renamed or reused for another
meaning; a token this build does not know is kept as read. `silk_f` and
`silk_b` are reserved: this build no longer writes them.

A silk graphic (`[[footprints.silk_f]]`, `[[footprints.silk_b]]`) carries
an optional `sketch_entity = "<uuid>"`, written only when set and read with
a default. A file without markers loads, and saves byte-identical; an older
build reads a file with markers (it ignores the key) and drops them when it
saves. The format token stays `snxfpt/1` and `FOOTPRINT_SCHEMA_VERSION`
is unchanged.

## Redundant constraints

`FullSolveOutput.redundant_constraints` names consistent constraints whose
every Jacobian row is implied by the EARLIER constraints (and the arc
closures), so the verdict depends on constraint order and names the later
duplicate. They are listed under "Redundant" in the Sketch solve section
of the footprint editor's Properties panel. Conflicts are reported
separately as over-constrained.

## The sketch in the Properties panel

With nothing selected, the footprint Properties panel shows the Footprint,
Snap & Grid and sketch sections in one scroll, in any mode. The sketch
sections hold the solve state and the sketch parameters:

- **Sketch solve**: a pill in the header names the state (DOF 0 in green when
  fully constrained, Under-constrained in blue, Over-constrained in red, or
  No sketch / Sketch not solved), then the entity and constraint counts and
  the last solve's iterations, residual, time and over-constrained count,
  the redundant constraints and the conflicting ones. A conflict or redundant
  row selects its entity.
- **Parameters**: one editable expression per sketch parameter.
- **Solve warnings**: the notes of the last solve.

In Sketch mode the same sections also follow a selected sketch entity, under
its Entity section (kind, Role, position, construction, attached constraints
and a banner with the entity's constraint state) and Snap Options.

## Role sub-forms

When the selected sketch entity carries a role, Properties shows the
sub-form of each role it carries, one under the other: Pour, Keepout,
Cutout, Sketch pad, then Pattern (a pattern's source is also a sketch pad,
so both show). Each starts with a header (the object and a caption line),
then a line-art illustration, then its sections. The forms live in
`panels/footprint_editor_properties/subforms/`; the edits without a flat
`FpEditor*` message are `FpSubformMsg`, handled in
`app/handlers/dock/sch_library/footprint/role.rs`. Every edit is one undo
step, re-solves and re-bakes the sketch, and dirties the tab.

- **Keepout.** The **Layer** dropdown writes `KeepoutAttr::layer`. It offers
  Top Layer and Bottom Layer, plus the stored layer when a file names
  another one. A keepout on all layers or on the inner layers only cannot
  be stored yet (`KeepoutAttr::layer` holds one board layer).
- **Cutout.** **Limit depth (pocket)** is the inverse of
  `BoardCutoutAttr::through`: ticked writes `through = false`. A cutout has
  no stored depth, so the switch shows no Depth field.
- **Sketch pad.** The copper size comes from the sketch profile and is
  shown, not edited. Top Paste and Top Solder mask are enable sections: the
  mask switch is ticked while the mask opens (`mask_top_tented = false`).
  Each expansion is Rule (`paste_*_expr` / `mask_*_expr` empty) or Manual;
  a plain number is stored as `<n>mm`, an expression as written, and text
  that does not parse yet is kept in the editor's field buffers without
  touching the pad. A paste expression clears the paste percentage of that
  side. A pad without a drill shows the paste and mask of its own side
  (`PadAttr::side`; both sides for a pad on every side), which are the
  layers the bake gives it; a drilled pad shows both sides and the hole
  details.
- **Pattern.** The kind segment shows the pattern's kind; a pattern keeps
  the kind it was made with. Start row / col and the letter skip apply to
  BGA numbering and grey otherwise.

The pattern illustration draws `signex_bake::array::preview_array`: the
same walk the bake runs (`signex-bake/src/array/walk.rs`) gives each
instance its offset, number and dropped flag, so the drawing shows what the
bake produces. Dropped instances are drawn dashed. When a count, spacing or
mask does not evaluate, or the pattern has more than 256 instances, the
drawing shows the source pad only, with a note.

## Leaving sketch mode {#leaving-sketch-mode}

The footprint editor shares the board sketch's tools and its way out
(#2518; the board side and the host lists are in
[PCB_SKETCH.md](PCB_SKETCH.md#hosts)). In sketch mode the Active Bar is the
sketch bar alone, Select | Create ▾ | Modify ▾ | constraints | dimension
input | Linetype | Snap ▾, and the mode bar at the canvas's bottom left
reads **Bake preview · Discard changes · Bake**:

- **Bake** keeps the edits and goes back to the pads view. The footprint
  bakes on every solve, so nothing more is written.
- **Discard changes** goes back to the pads view without the edits. Sketch
  mode keeps the footprint as it found it (`sketch_mode::exit`); when the
  footprint differs from that, Discard changes asks first, then restores
  it in one undo step, so Undo brings the edits back. Without edits it
  leaves at once. It never writes to disk.
- **Bake preview** shows a count per footprint target (pads, silk,
  courtyard, mask, paste, pours, keepouts, cutouts, V-scores); "Back to
  sketch" closes it.

The Linetype control is a three-way segment, Normal / Construction /
Centerline; it replaces the click-to-cycle pill.
