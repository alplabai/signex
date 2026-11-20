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
duplicate. They are listed under "Redundant" in the footprint editor's
DOF / Last solve section. Conflicts are reported separately as
over-constrained.

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

## Modify tools

Sketch ▸ Modify lists, on both hosts: **Fillet, Chamfer, Trim, Extend**,
then under "Needs a selection" **Mirror, Offset, Linear Pattern,
Rectangular Pattern, Circular Pattern**, then **Make Pad from Profile**
(greyed on a board). The rows under "Needs a selection" grey out while
nothing is selected.

- **Chamfer** — click the first Line, then the second; they must share an
  endpoint. Both are cut back from the corner by the chamfer distance and a
  new Line joins the cut ends, which it shares with the two Lines. The
  distance is the dimension input, else 0.5 mm; a value that is not a
  positive length is refused and nothing changes, as for Fillet. A distance
  that reaches the far end of a Line, or two Lines that run in one
  direction, are refused with a warning. The new Line and points take the
  first Line's linetype. The corner Point is left in place, as Fillet leaves
  it.
- **Extend** — click near one end of a Line. That end moves to the nearest
  Line, Circle or Arc ahead of it and a constraint keeps it there (Point on
  Line, Point on Arc, or zero distance to a Circle). With nothing ahead the
  Line is left as it is and a warning says so. Only a free end extends: an
  end that another entity shares, or that a constraint, a pattern or a pad
  names, is refused rather than torn off.
- **Mirror** — with a Line selected, each click mirrors the entity under it
  about that Line and keeps the copy linked with Symmetric constraints. With
  no Line selected, a click on a Line of the Centerline linetype takes it as
  the mirror line (it becomes the selection); the next clicks mirror about
  it. Mirror is under "Needs a selection", so it is armed with something
  selected.
- **Linear Pattern** — click the source. The pattern repeats it 4 times
  along X, at the dimension input as the pitch, else 5 mm; count and pitch
  are edited in the Pattern section. Rectangular Pattern (2 × 2 at 5 mm) and
  Circular Pattern (4 around a new centre) work as before.

Chamfer, Extend and Mirror's centerline pick take an existing Line by its
stroke (0.30 mm), so their clicks place no Point. Each tool shows a ghost
while armed: the Line a click would take, where an extended end would stop,
the cursor's mirror image, or where the pattern's instances would land.

On a board the same rows are offered. A board bake still bakes one instance
of a pattern; baking every instance waits for #2517.

The geometry of Chamfer and Extend is `signex_sketch::geom::line_edit`.

## Pads and the sketch

A pad made in the sketch keeps the id of the entity that drives it
(`EditorPad::sketch_entity_id`).

- **Edit in Sketch** on a pad switches to sketch mode with that entity
  selected and focused (`FootprintEditorState::pad_link`). The canvas draws
  the driving profile in the selection colour: the entity, the Lines of the
  pad outline, and the closed loop it lies in. A floating link at the
  canvas's bottom centre reads **◂ Back to pad N** and returns to the pads
  view with that pad selected.
- With a pad's driving entity selected in sketch mode and no focus, the
  link reads **Jump to pad ▸** and does the same.
- The focus ends when the mode changes or the selection moves to another
  entity. The link changes the mode and the selection only, so it records
  no undo step and does not mark the document changed. A board sketch has no pads
  and shows no link.

Not done yet: the outline of the pattern instance a pad came from (pads do
not record their instance yet), and a "Driven by sketch" section in the pad
Properties.
