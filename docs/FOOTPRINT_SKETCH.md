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
  sketch" closes it. While it is open the canvas draws the result (Graphite mockup #190):
  a veil in the ground colour, then the baked pads, silk and courtyard as
  Pads mode draws them and the mask openings, paste apertures, pours,
  keepouts and cutouts in their layer colours (the board palette's
  `board_layer_colour`), with the sketch faint on top. The pads, silk and
  courtyard are the editor's own (the footprint bakes on every solve); the
  other fields are baked from the same solve when the canvas redraws
  (`canvas/draw/sketch/bake_overlay.rs`). The card also lists the last
  bake's warnings, the features it left out; they name no entity, so a
  click on them does nothing.

The Linetype control is a three-way segment, Normal / Construction /
Centerline; it replaces the click-to-cycle pill.

## Parameters {#parameters}

A sketch's parameters are named values its expressions read: dimensions,
pattern counts and spacings, pad sizes and the other bake attributes
(#189). They are kept per footprint (or per board sketch); there are no
family tables shared between footprints.

**The sketch info bar** sits at the canvas's top right in sketch mode,
under the mode switcher and clear of the DOF chips: "Footprint sketch ·
plane Top · grid 0.05 mm" (a board reads "Board shape · …"), and the
**Parameters** button, which opens or closes the table.

**The table** is a card over the canvas with one row per parameter:

- **Name**, **Expression**, **Value**, **Unit** (mm, deg or count, the unit
  family the expression resolves to) and **Used by** (the parameters,
  dimensions, patterns, pads and other attributes whose expressions read
  it).
- **Add parameter** adds `param_1`, `param_2`, … (the first free name)
  holding `1mm`, as one undo step.
- Typing in Expression edits the parameter the same way the Properties
  list does: each keystroke re-solves and is one undo step. While the
  expression does not resolve (it does not parse, names an unknown
  parameter, or forms a cycle), the drawing keeps its last solve, the row
  is marked with the message, and Value shows the last good value. A row
  whose expression is fine but reads a broken parameter says which one.
- Typing a letter offers the parameter names that start with it, with
  their values; picking one completes the name. Nothing is offered after a
  number or a unit suffix (`0.5mm`).
- The row's **⋯** menu has **Rename…** and **Delete** (decision fp-07).
  Rename changes the name and every reference to it, whole names only
  (`pitch` never touches `pitch2`), as one undo step; a name that is taken,
  reserved (`pi`, the array indices `i` and `j`) or not a plain identifier
  is refused with the reason. Delete of an unused parameter is immediate.
  Delete of a used one asks first, naming its uses: **Replace with value**
  puts the parameter's value (for example `0.65mm`) in place of each
  reference and deletes it, as one undo step; **Cancel** keeps it.
- **Esc** closes the open prompt, rename or row menu, then the table.

**Make parameter** on a dimension (the constraint menu, m188) creates
`dim_N` (`angle_N` for an angle) holding the dimension's current value
with its unit, points the dimension at it, and opens the table on that
row, as one undo step. A dimension that already reads exactly one
parameter keeps it, and the table opens on that row.

The rules live in the domain: `signex_expr::rename` edits a source token
by token, and `signex_sketch::parameter_refs` walks every expression a
sketch holds (its patterns name every field, so a new field does not
compile until it is placed) for Used by, rename, delete and Make
parameter.

Not built yet: a Comment column (the parameter table stores no comment),
Import from another footprint, and the live-checks pane beside the
table. The compact Parameters list in Properties is unchanged.

## Values on the drawing {#values-on-the-drawing}

Every dimension constraint (Distance, Distance to line, Distance to circle,
Angle) draws its value where its letter glyph used to be, in a small label:
`name · value` when its target is exactly one parameter (`pitch · 0.65 mm`),
else the evaluated value with its unit (`0.70 mm`, `90°`); a reference
shows the value it measures ([Reference dimensions](#reference-dimensions)).
Constraints that are not dimensions draw their marks
([Canvas look](#canvas-look)). One layout
(`canvas/glyph_layout.rs`) places every glyph, value and pattern glyph; the
draw pass and the hit test both read it, so a click lands on what is
drawn.

With the Select tool, a click on a value opens it for editing on the
drawing, starting from its expression or number (degrees for an angle).
Enter writes it through the same path as every sketch edit: one undo step,
then a solve; a value that over-constrains the sketch is reverted and its
conflict named, leaving no undo step. A plain number is stored as a
number, anything else as an expression, kept only when it evaluates; text
that does not read keeps the field open with a red outline. Esc, or a click
elsewhere on the canvas, cancels. The dimension input in the Active Bar
stays.

The words the canvas draws (units, pattern letters, the conflict badge,
the snap badges) come from the app's language: the app renders them into
the editor state (`state/canvas_text.rs`) when the editor opens and after
a language change, because a canvas cannot read the catalog.

## Constraint menu {#constraint-menu}

A right-click on a constraint's glyph or value (Select tool) opens one
menu at the click: Edit value, Make parameter and Convert to reference
(dimensions only; a reference shows Make driving instead, see
[Reference dimensions](#reference-dimensions)), Suppress, Select
constrained entities, Show conflicts (only when the constraint is in a
conflict; it selects every entity of the conflict) and Delete. Make
parameter hands the dimension to the sketch parameters table, and is
greyed when the value already is a parameter. Suppress is greyed with a
tooltip that says why: the sketch file cannot store a suppressed
constraint yet (see `DECISIONS.md`). Esc closes the menu first. A right-click anywhere
else keeps its behaviour: it cancels a tool, pans, or opens the footprint
context menu.

## Patterns on the drawing {#patterns-on-the-drawing}

Each pattern draws a glyph by its source, like a constraint: `L×4`
(linear), `G×3×4` (rectangular, columns by rows), `P×8` (circular). A
click on it selects the pattern (a second click, or Esc, drops it); a
right-click opens its menu: one Edit row per value (count, or columns and
rows; X and Y spacing; the sweep of a circular pattern) and Delete pattern,
which keeps the source.

A selected pattern shows its values under its glyph (`count = rows · 4`,
`spacing X · 0.65 mm`); they edit on the drawing like a dimension and
write the pattern's expression as typed.

While a rectangular or circular pattern is selected, each of its instance
slots draws a ring; a click suppresses that instance (one undo step) and a
second click brings it back. The suppression list is the one the
Properties pattern form ticks, kept by one helper on the array type
(`ArrayKind::set_instance_suppressed`). A linear pattern has no
suppression list, so its instances do not toggle. The source's own slot is
not offered: it is the source pad.

## Conflicts {#conflicts}

An over-constrained sketch draws no card over the drawing (decision
fp-15): a compact red badge next to the conflicting dimension, or the
first conflicting glyph, counts the conflicts and points to Properties
(`1 conflict · see Properties`). In Properties, each conflict row has a
Remove button that deletes that constraint in one undo step, then solves.

## Snap badges {#snap-badges}

While a drawing tool is armed, the cyan glyph at the cursor shows what the
next click lands, and a pill beside it names it (Graphite mockup #188, "the
badge names the target"; `canvas/draw/sketch/snap.rs`):

| Snap | Glyph | Badge |
|---|---|---|
| A sketch point | ● | Coincident · pad 1 (the point is a pad's centre or one of its own points), else Coincident · point |
| Horizontal / Vertical inference | ─ / │ | Horizontal / Vertical |
| Angle | ◇ | Angle 45° (the slope as seen on screen, 0° to 179°) |
| Intersection | + | Intersection |
| Guide | none (the guide is drawn) | Guide |
| A board feature (board sketch only) | ◻ | Pad · J1-1, Courtyard · J1, Board outline, … |
| Grid, or no snap | none | none |

The words come from the message catalog through the editor's canvas text,
so they follow the app language. The pill takes the Graphite Dark chrome
tokens (raised fill, strong line, text), since the board ground is Graphite
Dark in both themes; the glyph keeps its cyan. Midpoint, Centre and the
other snaps the mockup lists, and whether they land constraints, are not
decided yet.

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

## Pattern links {#pattern-links}

Decision 10 (Caner, 2026-11-20, mockup row 191). A pattern may repeat
another pattern, and a mirror of a pattern stays a pattern.

- **A pattern of a pattern.** With a pattern selected (its glyph clicked),
  Linear, Rectangular or Circular Pattern makes the new array repeat every
  instance of the selected one: a linear pattern of a 4-way circular pattern
  bakes count × 4 pads. The link is `SketchData::pattern_sources`
  (`PatternSource { array, source }`); the array's own source entity is set
  to the innermost source, so entity references and the delete cascade
  work as for any array. Instance numbers run on in the outer pattern's
  scheme, the inner pattern's instances first: the outer slot `s`, inner
  instance `k` is number `s · inner slots + k` of the scheme (a grid counts
  its rows that many times wider). The inner pattern still bakes its own
  pads; the outer one bakes its other slots.
- **A live mirror.** With a pattern selected, a Mirror click on a Line (or
  with a Line selected) adds a `PatternMirror { id, pattern, line }` to
  `SketchData::pattern_mirrors` instead of copying geometry, as one undo
  step. Every instance of the pattern is reflected about the solved Line on
  each bake, so the mirror follows the pattern: change the count and the
  mirror changes with it. Mirrored pads keep the pattern's numbering scheme
  and continue it (a row numbered 1 to 4 mirrors as 5 to 8); a pad's
  rotation is reflected too. Pad offsets and the shape itself are not
  reflected (a trapezoid keeps its side). Without a pattern selected,
  Mirror copies geometry as before.
- **Chains and loops.** A mirror can be the source of another pattern or
  mirror. A link that would make a pattern repeat itself, directly or
  through other patterns and mirrors, is refused with a warning and leaves
  no undo step (`SketchData::set_pattern_source`, `add_pattern_mirror`); a
  loop in a damaged file is reported by the bake and that pattern bakes
  nothing. The instance limit (10 000 per pattern) counts every instance a
  chain expands to.
- **Delete.** Deleting the entity a chain starts from removes every
  pattern on it. Deleting a mirror's Line removes the mirror, and links
  onto a removed pattern are dropped; the array that named it stays and
  repeats its own source again.

The bake expands links in `signex_bake::array::links`; a plain array with
no link and no override still bakes through `bake_one_array`.

## Per-pad overrides {#per-pad-overrides}

Decision 9 (Caner, 2026-11-20, mockup row 192 D1). Editing a Pads-mode
attribute of a pad that is an instance of a pattern (not the pattern's
source pad) asks, each time, **This pad / All pads / Cancel**; no answer is
remembered. The edit shows in the Properties panel while the card waits; a
change of size or position, which an override does not hold, does not ask.

- **This pad** stores only the attributes the edit changed, as an override
  of that instance: `SketchData::pad_overrides`, keyed by `(pattern, i, j)`
  (`InstanceOverride`, `PadAttrOverride`: number, net, locked, electrical
  type, template, library, features, testpoint, hole tolerances and
  rotation, copper offsets, rotation, drill, mask and paste margins, shape,
  pad stack). The bake applies it to that instance's pad only. A second
  edit of the same pad merges into its override.
- **All pads** writes the changed attributes onto the pattern's source pad,
  so every instance changes.
- **Cancel**, or Esc, drops the edit.

Each answer that changes the sketch is one undo step and re-bakes. The key
of an instance (`signex_bake::instance_at`) is `(array, i, j)` for a plain
array, `(array, outer slot, inner instance)` for a pattern of a pattern and
`(mirror, mirrored instance, 0)` for a mirror. An override of a pattern
that is deleted goes with it.

## Reference dimensions in the file

Decision 5 (storage). `SketchData::driven_dimensions` lists the
constraints that are reference dimensions: they measure and never drive.
The list is pruned with its constraints. What they draw and how they
measure is in the sketch canvas look.

The four lists above (`driven_dimensions`, `pattern_sources`,
`pattern_mirrors`, `pad_overrides`) are absent from a file when empty;
any of them makes a save write `snxfpt/1.2`, which a 1.1 build refuses by
name instead of dropping them ([FORMAT_VERSIONS.md](FORMAT_VERSIONS.md)).

## Reference dimensions {#reference-dimensions}

A dimension (Distance, Distance to line, Distance to circle, Angle) can be
a reference: it measures the geometry and never drives it (decision 5,
Caner 2026-11-20, mockups #187 and #188). The constraint menu offers
**Convert to reference** on a driving dimension and **Make driving** on a
reference; a Properties conflict row of a dimension has a Convert to
reference button beside Remove. Each is one undo step, then a solve.
Converting the conflicting dimension clears the conflict. Make driving that
would over-constrain the sketch is reverted with its conflict named and
leaves no undo step, as a value written on the drawing is.

The sketch stores the references as a list of constraint ids
(`SketchData::driven_dimensions`, `signex-types/src/sketch/driven.rs`), not
as a flag on the constraint. Only a dimension can be a reference; an id in
the list that names anything else is ignored. The solver reads one list,
`SketchData::driving_constraints`, in every walk over the constraints: a
reference has no residual, no Jacobian row and no degree of freedom, and
is never named in a conflict or as redundant. A point only a reference
reads stays out of the solve and does not move.

On the drawing a reference shows the value it measures
(`signex_sketch::solver::measure::measured_value`: the dimension's own
residual with its target set to zero, so each kind keeps one formula),
never its stored target. Its label and its dimension line draw dashed in
the reference purple. Edit value and Make parameter are greyed on it ("A
reference dimension measures; make it driving to edit it"), and a click
on its value does not open an edit.

## Canvas look {#canvas-look}

Sketch mode paints the dark board ground on every theme (only the
Graphite themes ship; the white sketch canvas is gone), and the sketch
follows the mockups on it (decision 7, Caner 2026-11-20), from one palette
(`canvas/draw/sketch/sketch_ink.rs`, `SketchInk::BOARD`): fully
constrained geometry green `#7FD1A0`, under-constrained blue `#A9C2FF`,
over-constrained red `#F2A093`, construction lines and an unsolved sketch
grey `#8B8D93`, centerlines gold `#C9A13B`, selection `#E0865C`. Values sit
in pills (`#1D1F23`, border `#34363C`, text `#CFD1D6`) on dimension lines
in `#7C7F86` with an arrowhead at each end; a reference draws in
`#C9A0FF`. A conflict keeps the theme's danger colour.

Relations draw the mockup marks in small upright pills: H, V, ∥ parallel,
⊥ perpendicular, = equal length or radius, ◡ tangent, ⇔ symmetric, ⌖
fixed, ┃· point on line or arc, M midpoint; coincident is a dot. A
conflicting mark fills in the danger colour.

The cursor marks sketch mode draws in place of the OS cursor are light on
that ground.

A mirrored pattern draws a glyph by its mirror line, `⇔` and the mirrored
pattern's glyph (`⇔ L×4`); a click selects it, and its menu has only
Delete mirror (one undo step; the pattern and the line stay). A pattern
whose source is another pattern names it: `P×4 of L×4`.
