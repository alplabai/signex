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

### Generated features {#generated-features}

Silkscreen outline, Pad clearance, Pin 1 marker and Assembly outline (mockup
row 124, #2834) are switches and values on the footprint, edited in the
Courtyard & Silkscreen section as one undo step each. All off at the
defaults (line width 0.12 mm, clearance 0.20 mm, marker on), which is not
written, so such a file keeps its lower token; anything else writes
`snxfpt/1.3`.

The courtyard's Line width (mockup row 124, #2850, default 0.05 mm) is stored
with these values as `generated.courtyard_line_width_mm` and follows the same
token rule. Placing the footprint strokes the board courtyard at that width,
whichever way the courtyard was drawn. The field sits under the generated
courtyard switch and is greyed while that switch is off.

The pin 1 dot sits on pad 1's outer side, or on the first other side clear of
every pad by the clearance; with no clear side there is no dot. While the
silkscreen outline and the marker are on and no dot is placed, for that reason
or because no pad is numbered 1, the section shows a note saying so.

`signex_bake::bake_generated` rebuilds the strokes after a setting edit, a
pad write-back (saves included), a 3D body mint and a sketch bake. They are
stored in `generated.silk` and `generated.assembly`, apart from `silk_f` and
`fab_f`: a re-bake never touches hand-drawn entries or their selection, and a
stroke cannot be selected or edited by hand. Placing the footprint copies them
to the board's top silkscreen and fab layers.

- **Body:** the 3D body outline's extent, else the extent of the hand-drawn
  front fab graphics. With neither, no outline is generated and the section
  says what is missing.
- **Silkscreen outline:** the body rectangle, with the part of each edge
  within the clearance plus half the line width of a pad's turned extent
  removed.
- **Pin 1 marker:** a solid dot on the outer side of the pad numbered 1, the
  clearance away from its copper. No such pad, no dot.
- **Assembly outline:** the body rectangle on the front fab layer, at the
  stock parts' 0.1 mm line.

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

- **Pour.** Net, Priority and Fill, then **Corners** (Sharp, Rounded,
  Chamfer) and **Corner size (mm)**, the rows of a board pour (#2598): the
  model is the board pour's, used as is (`PourAttr::corners` and
  `PourAttr::corner_size`, as `Zone::corners` and `Zone::corner_size`,
  #2578). A new pour is Sharp. Corner size takes a number from 0; other
  text is kept in the editor's field buffers, outlined in red, without
  touching the pour, and the field is greyed at 0 while Sharp. The bake
  shapes the pour's convex corners with the rule a board pour fills with
  (`pcb_geom::shaped_outline_mm`, `docs/PCB_TOOLS.md`, "Corners and hatched
  fill"), so the canvas and the baked `pours` show the shaped outline;
  Sharp, or a size of 0, bakes the outline as drawn. A board sketch pour
  shows the same rows and bakes them into its board zone
  (`docs/PCB_SKETCH.md`). A pour with corners other than Sharp saves as
  `snxfpt/1.5`, or `snxpcb/1.13` on a board (`docs/FORMAT_VERSIONS.md`).
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
- **Pattern.** The kind segment shows the pattern's kind and switches it
  (see [Switching a pattern's kind](#switching-a-patterns-kind)). Start
  row / col and the letter skip apply to BGA numbering and grey otherwise.

The pattern illustration draws `signex_bake::array::preview_array`: the
same walk the bake runs (`signex-bake/src/array/walk.rs`) gives each
instance its offset, number and dropped flag, so the drawing shows what the
bake produces. Dropped instances are drawn dashed. When a count, spacing or
mask does not evaluate, or the pattern has more than 256 instances, the
drawing shows the source pad only, with a note.

## Switching a pattern's kind {#switching-a-patterns-kind}

The Pattern form's kind segment (Linear, Grid, Polar) switches the selected
pattern to another kind (#2596, DECISIONS 2026-11-27). The reshaping is
`signex_bake::array::switch_pattern_kind`; the segment and the centre pick
are `updates/sketch/pattern_kind.rs`.

- **Count.** The instance count carries over. A linear or polar pattern
  becomes one grid row (`ny = 1`), or one column (`nx = 1`) when it steps
  along Y only. A grid becomes a linear or polar pattern of `nx × ny`
  instances: the side that is not 1 as written, else the product (a number
  when both sides resolve, `(nx) * (ny)` when they do not). A polar pattern
  takes the count over a full 360° sweep.
- **Spacing.** dX and dY carry over between Linear and Grid as written. A
  grid of several rows becomes a line along its row (dY 0), a one-column
  grid a line down its column (dX 0). Polar has no spacing, so a polar
  pattern switched away gets a 5 mm step along X, the Rectangular Pattern
  default. The polar centre Point stays in the sketch, unless the switch
  to Polar placed it (the centre click minted it) and nothing else uses it:
  no entity, constraint, pattern or mirror names it and it has no role.
  Such a centre goes in the same undo step. Which centres the switch
  minted is known for the session only (`switch_minted_centres` in the
  editor state); after a reload a centre counts as drawn and stays.
- **Centre.** Linear and Grid switch at once. Polar first asks for a centre
  through the Re-pick flow: the form shows "Click a Point… (Esc to
  cancel)", and the next click on the canvas, under any tool, picks or
  places the centre and switches the pattern. Esc, or the pattern's own
  kind segment, cancels and leaves the pattern as it was.
- **Instances.** The suppression list and the pattern's per-pad overrides
  follow their instance in the row-by-row walk: cell `(i, j)` of a grid
  `nx` wide is instance `j × nx + i` of a line. An entry with no place in
  the new kind (past the count, past the last row such as a mirrored
  instance, or in a later row of a grid whose width does not resolve) is
  dropped, and a Messages line names the dropped instances. A pattern
  that repeats another pattern keys its overrides by (outer slot, inner
  instance), the outer slot already numbered in walk order, so they keep
  their key; only one whose outer slot is past the count is dropped. The mask
  expression carries over as written (the bake warns where it does not
  fit) and the numbering scheme is kept; the numbers follow the new walk.
- **Undo.** The switch is one undo step; for Polar it is the step of the
  click, centre Point included.

## Switching modes {#switching-modes}

The footprint editor bar floats at the canvas's top right, just under the
Active Bar (Graphite mockup row 187). At its left a two-state segment reads
**Footprint | Sketch**: Footprint is the pads view, Sketch opens the
sketcher. Beside it the **3D** toggle shows the body in 3D; it is lit while
3D is on, and a second click goes back to the mode 3D was entered from
(Footprint or Sketch). In 3D the segment of that mode stays lit, and a
click on a segment leaves 3D for that mode. Leaving Sketch this way does
what leaving it always does: the pad link, the canvas value edits and the
bake preview close. In sketch mode the bar goes on with the sketch info
caption and **Parameters** (see [Parameters](#parameters)). The keys `1`
(Sketch), `2` (Footprint) and `3` (3D) switch the same way. A board sketch
has no Footprint / Sketch switch.

The switch replaces the Sketch · Pads · 3D segment; "Pads" now reads
"Footprint". Decided by Caner on 2026-11-20 (`docs/DECISIONS.md`).

## Leaving sketch mode {#leaving-sketch-mode}

The footprint editor shares the board sketch's tools and its way out
(#2518; the board side and the host lists are in
[PCB_SKETCH.md](PCB_SKETCH.md#hosts)). In sketch mode the Active Bar is the
sketch bar alone, Select | Create ▾ | Modify ▾ | constraints | dimension
input | Linetype | Snap ▾, and the mode bar at the canvas's bottom left
reads **Bake preview · Discard changes · Bake**. The constraint strip always
shows all 19 buttons; one the selection cannot take is greyed and does nothing.
The bar fits and centres on the canvas between the docks. When the canvas is
too narrow for it, the strip alone scrolls sideways, by a thin scrollbar under
its buttons or Shift+wheel. A bar still wider than the canvas (a narrow tile)
stays whole and passes over the docks, kept inside the window, so every other
control stays in reach:

- **Bake** keeps the edits and goes back to the pads view. The footprint
  bakes on every solve, so nothing more is written. An open profile stops
  it: the editor stays in sketch mode, selects the open ends and opens the
  bake preview. The check reads the sketch's topology, so it needs no solve.
  A sketch that cannot be solved stops Bake too, and the preview says why.
- **Discard changes** goes back to the pads view without the edits. Sketch
  mode keeps the footprint as it found it (`sketch_mode::exit`); when the
  footprint differs from that, Discard changes asks first, then restores
  it in one undo step, so Undo brings the edits back. Without edits it
  leaves at once. It never writes to disk.
- **Bake preview** shows a count per footprint target (pads, silk,
  courtyard, mask, paste, pours, keepouts, cutouts, V-scores); "Back to
  sketch" closes it. While it is open the canvas draws the result (Graphite mockup row 190):
  a veil in the ground colour, then the baked pads, silk and courtyard as
  Pads mode draws them and the mask openings, paste apertures, pours,
  keepouts and cutouts in their layer colours (the board palette's
  `board_layer_colour`), with the sketch faint on top. The pads, silk and
  courtyard are the editor's own (the footprint bakes on every solve); the
  other fields are baked from the same solve when the canvas redraws
  (`canvas/draw/sketch/bake_overlay.rs`). A legend at the canvas's top
  right names the layers the preview draws on, with their colours: Top,
  Bottom, Paste, Bottom paste, Mask, Bottom mask, Silk, Bottom silk,
  Courtyard and Bottom courtyard, each only when something is drawn on it,
  so a layer hidden in the layer visibility is not listed (Caner,
  2026-11-27, mockup row 190). The card is in Properties, not
  over the canvas: while the preview is open Properties shows the Bake
  panel, whatever is selected: the Roles section, the card, then Back to
  sketch and Bake (mockup row 190). The card also lists the last
  bake's warnings, the features it left out; they name no entity, so a
  click on them does nothing. One gap is one line, naming the object and
  its place, and the canvas rings it. Bake stays clickable with an error:
  the click stays in sketch mode and shows the open ends. Opening the
  preview, a refused Bake included, brings Properties to the front.

After **Bake**, Properties shows the outcome: `Baked · N pads`, what was
written, a **Pads ↔ Sketch** section, the Roles and Bake result sections
closed below it, and Edit in Sketch (mockup row 190). Pads ↔ Sketch says
what the links are and "No pad selected." until a pad is clicked; clicking
one pad keeps the panel and shows "Pad N ← source" with Show in Sketch,
which opens that source in Sketch mode with Back to pad (mockup row 192).
A non-pad object, a multi-selection, an edit or an empty click ends the panel,
and it does not come back on Undo or Redo.

The Linetype control is a three-way segment, Normal / Construction /
Centerline; it replaces the click-to-cycle pill.

## Roles {#roles}

With nothing selected, Properties in a footprint sketch has a **Roles**
section: one row per closed profile with its role, then Solder mask (from the
pad rule, no entity), the centerlines (they never bake), and Bake preview and
Bake (mockup row 190). A role picked on a row applies to the whole profile as
one undo step.

A closed profile of lines and arcs can take the **Pad** role: a centre Point
carries the pad, and its custom shape is traced from the loop. Picking Pad on
any edge of the loop does the same, and an edge of that loop reads Pad. Another
role on the loop removes the pad. A Point stays a simple pad.

## Parameters {#parameters}

A sketch's parameters are named values its expressions read: dimensions,
pattern counts and spacings, pad sizes and the other bake attributes
(mockup row 189). They are kept per footprint (or per board sketch); there are no
family tables shared between footprints.

**The sketch info bar** sits at the canvas's top right in sketch mode, on
the editor bar after the 3D toggle ([Switching modes](#switching-modes)),
clear of the DOF chips; a board floats it alone: "Footprint sketch ·
plane Top · grid 0.05 mm" (a board reads "Board shape · …"), and the
**Parameters** button, which opens or closes the table.

Under the table's rows, a live bake preview (Caner, 2026-11-24) draws the
edges that bake, a footprint's baked pads and the driving dimensions, from
the current solve without solving again; it redraws on each keystroke.
While an expression does not read, it keeps the last good bake, marked,
and hides the pads, since their angles cannot be read, saying so. When a
solve fails (the canvas then shows the sketch unsolved), the preview keeps
the last solve that succeeded, dimmed, under "The sketch does not solve:
this is the last good bake.", with its pads hidden until the sketch solves
(Caner, 2026-11-27, #2856). The next successful solve replaces it; the
copy lives in the editor tab and goes with it. Between an edit and the solve
that lands for it (a drag solves off the UI thread), the preview hides
the pads, saying "Pads are hidden until the sketch solves.": the last
solve's numbers would be read with the table's new units (Caner,
2026-11-27, #2859). Live checks sits beside it,
parameter errors first.

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
- The row's **⋯** menu has **Rename…** and **Delete** (Caner 2026-11-20).
  Rename changes the name and every reference to it, whole names only
  (`pitch` never touches `pitch2`), as one undo step; a name that is taken,
  reserved (`pi`, the array indices `i` and `j`) or not a plain identifier
  is refused with the reason. Delete of an unused parameter is immediate.
  Delete of a used one asks first, naming its uses: **Replace with value**
  puts the parameter's value (for example `0.65mm`) in place of each
  reference and deletes it, as one undo step; **Cancel** keeps it.
- **Esc** closes the open prompt, rename or row menu, then the table.

**Make parameter** on a dimension (the constraint menu, mockup row 188) creates
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

While a pattern is selected, each of its instance slots draws a ring; a
click suppresses that instance (one undo step) and a second click brings it
back. The suppression list is the one the Properties pattern form ticks,
kept by one helper on the array type (`ArrayKind::set_instance_suppressed`).
A linear pattern keeps its list as a circular one does, as `(i, 0)` entries
(#2688); its form has the Mask field and per-instance checkboxes of a
circular one, and a masked instance keeps its number (numbering follows the
index). A
footprint with a linear suppression is saved as `snxfpt/1.5` and a board
sketch with one as `snxpcb/1.13` (`docs/FORMAT_VERSIONS.md`). The pattern form's
Numbering Mask (mockup row 126, #2845) edits the same list by pad number
(`4,5`, `A2,B4`; commas or spaces): it shows the numbers of the suppressed
instances, and a typed list replaces the list in one undo step
(`signex_bake::array::instances_numbered`, `ArrayKind::set_suppressed_instances`).
A number the pattern does not have is refused and the text stays in the field.
The source's own slot is
not offered: it is the source pad. The canvas draws no ring on it, and the
Mask refuses the source pad's number the same way.

## Conflicts {#conflicts}

An over-constrained sketch draws no card over the drawing (Caner,
2026-11-20): a compact red badge next to the conflicting dimension, or the
first conflicting glyph, counts the conflicts and points to Properties
(`1 conflict · see Properties`). In Properties, each conflict row has a
Remove button that deletes that constraint in one undo step, then solves.

## Snap badges {#snap-badges}

While a drawing tool is armed, the cyan glyph at the cursor shows what the
next click lands, and a pill beside it names it (Graphite mockup row 188, "the
badge names the target"; `canvas/draw/sketch/snap.rs`):

| Snap | Glyph | Badge |
|---|---|---|
| A sketch point | ● | Coincident · pad 1 (the point is a pad's centre or one of its own points), else Coincident · point |
| Horizontal / Vertical inference | ─ / │ | Horizontal / Vertical |
| Angle | ◇ | Angle 45° (the slope as seen on screen, 0° to 179°) |
| Intersection | + | Intersection |
| A Line's midpoint | ◐ | Midpoint · line (or · pad 1 for a pad's Line) |
| A Circle's or Arc's centre | ⊙ | Centre · circle / arc (or · pad 1) |
| Guide | none (the guide is drawn) | Guide |
| A board feature (board sketch only) | ◻ | Pad · J1-1, Courtyard · J1, Board outline, … |
| A footprint object, or Axes lining up with one ([Object snap](#object-snap)) | none | none |
| Grid, or no snap | none | none |

The words come from the message catalog through the editor's canvas text,
so they follow the app language. The pill takes the Graphite Dark chrome
tokens (raised fill, strong line, text), since the board ground is Graphite
Dark in both themes; the glyph keeps its cyan. The Midpoint, Centre and
Intersection snaps land a constraint on the click (Caner, 2026-11-20,
decision 6); see [Snap inference](#snap-inference).

## Create tools {#create-tools}

Sketch ▸ Create lists, on both hosts: Line, Rectangle, Rounded Rectangle,
Circle, Arc, Tangent Arc, Edge Arc, Point, **Slot**, **Polygon** (Caner,
2026-11-20, item 1; mockup row 187). Each shape is placed by its last click as
one edit, so one undo step, and the tool goes back to its first click; Esc
drops a half-made shape. The geometry is `signex_sketch::geom::shapes`, the
same helpers the canvas ghost draws from.

- **Slot** — click one end's centre, then the other's, then a third click
  sets the half width (its distance from the line through the centres). A
  positive length in the dimension input is the centre distance: the second
  centre lands that far from the first, towards the cursor; an unreadable
  one is refused and nothing changes. The slot is two semicircular Arcs on
  the centres and two Lines joining their ends, plus a construction diameter
  across each end. Constraints: each centre is the Midpoint of its diameter,
  each diameter is Perpendicular to a side, the Arcs have Equal radius, and
  two dimensions: the centre distance (centre to centre) and the width
  (across the first end's diameter). The slot is then fully defined but for
  where it sits and which way it points.
  The ends are held tangent by the diameters, not by Tangent constraints: a
  Tangent constraint at a point the Line and the Arc share has a residual
  with its maximum there (the distance from a centre to a line through a
  point of the circle never exceeds the radius), so its Jacobian row is zero
  and it pins nothing; the slot would read as under-defined and the
  redundancy check could flag it.
- **Polygon** — click the centre, then the first vertex. The dimension input
  is the side count: empty is 6, a whole number from 3 to 64 is taken, and
  anything else is refused with a warning, nothing changes and the tool
  waits for the vertex click again. The polygon is its sides as Lines
  closing a loop, each vertex held on a construction Circle on the centre
  (zero distance to the Circle), and each side as long as the next.

Both bake as any closed profile does. The side count and centre distance
are read from the dimension input (as Chamfer and Linear Pattern read it);
the cursor's typed-value overlay has no field for them yet.

## Snap inference {#snap-inference}

While a drawing tool is armed (the Create rows above), three snaps land a
constraint on the click's Point, in the click's undo step (Caner,
2026-11-20, item 6; mockup row 188):

| Snap | Constraint |
|---|---|
| Midpoint of a Line | Midpoint (the Point is the Line's midpoint) |
| Centre of a Circle or Arc | Coincident with the centre Point (the click places its own Point) |
| Intersection of two curves | Point on Line for a Line, Point on Arc for an Arc, zero distance to a Circle, on each of the two |

Priority: a sketch Point first (a Point that is a Circle's or Arc's centre
reads as Centre), then the board, then the footprint objects the sketch does
not draw ([Object snap](#object-snap)), then Midpoint and Centre within the
Point snap's 8 px, then guides (while Guides is on), intersections, H / V,
angle, Axes and grid. Snap ▾ in
sketch mode has a **Sketch snaps** group (Line midpoints, Circle and arc
centres, on by default, not saved, no undo step); intersections keep their
Objects for snapping row. A Line within ±5° of an axis still gets its
Horizontal or Vertical (one threshold, `snap::AXIS_THRESHOLD_DEG`).

**Ctrl while drawing** (Cmd on macOS) turns inference off for that click:
the cursor skips the Midpoint, Centre, Intersection, H / V and angle snaps
(point reuse, the board, guides and grid stay), and the click lands no
inferred constraint, the Line's auto H / V included. The code is
`snap::snap_cursor_with` and `snap::click_inference`; the canvas sends
`SketchToolClickInferred` / `SketchPlacePointInferred` only when one applies.

## Object snap {#object-snap}

The footprint canvas snaps to its own objects within Snap Distance, each
kind behind its row of Objects for snapping (#2726; `snap_objects/`). In
Pads mode every kind counts; in Sketch mode only pads without a sketch
backing and silk the sketch did not bake, since the sketch's own Points
snap first. The point is placed only; no constraint lands.

| Switch | Targets |
|---|---|
| Footprint Origins | (0, 0), Pads mode only |
| Pad Centers | each pad's centre |
| Pad Vertices | the corners of Rect, Chamfered, Trapezoid and Custom pads, turned with the pad |
| Pad Edges | the nearest point of any pad's outline (round corners in 8 chords per quarter turn) |
| Track Vertices | silk line ends, rectangle and polygon corners |
| Track/Arc Lines | the nearest point of a silk line, rectangle or polygon side, circle or arc (within its sweep) |
| Arc Centers | silk circle and arc centres |
| Texts | a silk string's anchor |
| Regions | a filled rectangle's or polygon's corners and the nearest point of its edges |
| 3D Body Snap Points | the box the 3D preview draws (`preview3d::body_bbox`) seen from above: its four corners, its centre (the vertical axis) and the nearest point of its two horizontal axes, Pads mode only |

Via Centers has no target of its own: a footprint via is a pad. **Axes**
(off by default) lines the pointer up with a fixed target (not the nearest
point of a line or edge) within Axis Snap Range: the nearest whose x is
within Snap Distance of the pointer's x gives the x, the same for y, and an
axis nothing lines up on takes the grid. It runs after the angle snap and
before the grid. **Guides** (on by default) gates the guide snap. The Snap
Options are session state, never saved, as on a board.

A drag of a pad that is part of a multi-pad selection moves the whole
selection by the grabbed pad's snapped offset, and the carried pads are no
snap targets; Alt drags the grabbed pad alone. A press on such a pad keeps
the selection; releasing it without a drag selects that pad alone. Place
Pad, Place Via and Place Hole over an existing pad place a new one there
(stacked on the pad's centre when Pad Centers is on); Shift grabs the
existing pad instead.

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
- **Mirror** — opens an options card (mockup row 191; Caner, 2026-11-24).
  The selection becomes its Objects and a selected centerline its Mirror
  line. Clicks pick: a centerline under the cursor fills an empty line
  first; with Mirror line focused, a click takes any Line; other clicks add
  or remove objects. Keep linked (on each time the card opens) keeps every
  copy symmetric to its source; off makes free copies. A selected pattern is
  always mirrored live, and takes the place of its source in the Objects:
  the pattern's mirror already holds the source's image, so a pick of the
  source is ignored while its pattern is an object. Selecting an entity
  drops the selected pattern, so the card never takes one picked before the
  current selection (#2859). OK writes one undo step and returns to Select;
  Cancel writes nothing. The ghost draws the line, the picks and the copies
  OK will write. Mirror is under "Needs a selection", so it is armed with
  something selected.
- **Linear Pattern** — click the source. The pattern repeats it 4 times
  along X, at the dimension input as the pitch, else 5 mm; count and pitch
  are edited in the Pattern section. Rectangular Pattern (2 × 2 at 5 mm) and
  Circular Pattern (4 around a new centre) work as before.

Chamfer, Extend and Mirror's centerline pick take an existing Line by its
stroke (0.30 mm), so their clicks place no Point. Each tool shows a ghost
while armed: the Line a click would take, where an extended end would stop,
the Mirror card's copies, or where the pattern's instances would land.

On a board the same rows are offered, and a board bake writes every instance
([PCB_SKETCH.md](PCB_SKETCH.md#patterned-board-items)).

The geometry of Chamfer and Extend is `signex_sketch::geom::line_edit`.

## Pads and the sketch

A pad made in the sketch keeps the id of the entity that drives it
(`EditorPad::sketch_entity_id`).

- **Edit in Sketch** on a pad switches to sketch mode with that entity
  selected and focused (`FootprintEditorState::pad_link`). The pad header
  carries it inline on the "N of M pads" line (mockup row 125). In Footprint
  mode a selected pad with sketch geometry behind it also gets a floating
  **Edit in Sketch ▸** link at the canvas's bottom left
  (`FootprintEditorState::edit_in_sketch_offer`, mockup row 192).
- The sketch canvas draws the driving profile in the selection colour: the
  entity, the Lines of the pad outline, and the closed loop it lies in. It
  also outlines the pad that entity drives, with the same halo
  (`FootprintEditorState::pad_link_highlight`). A floating link reads
  **◂ Back to pad N** and returns to the pads view with that pad selected.
- With a pad's driving entity selected in sketch mode and no focus, the
  link reads **Jump to pad ▸** and does the same; the profile and the pad
  outline are highlighted then too.
- The focus ends when the mode changes or the selection moves to another
  entity. The links change the mode and the selection only, so they record
  no undo step and do not mark the document changed. A board sketch has no
  pads and shows no link.

- Edit in Sketch on a pattern instance focuses the pattern's source, found
  by `signex_bake::instance_at`. The source's pad is outlined solid and the
  instance dashed (mockup row 192). Back to pad returns to the instance.

- A patterned pad's Properties show the **Pad attributes ↔ sketch** table
  under the header (mockup row 192, item D1). A caption names the source
  (a pad point or a profile of N edges) and the pattern instance. Each row
  is one attribute and its scope: **Numbering** for the designator,
  **Pattern source** for an attribute every instance shares, **This pad**
  for one the instance's override holds (`PadInstance::overrides`, from
  `SketchData::override_for`). The stack is stored whole in an override, so
  paste, mask, corner radius and thermal relief count as overridden only
  where they differ from the source pad. The net is not listed: the board
  netlist assigns it. A pad no pattern places shows no section here; the
  former Source / Instance / Size block is gone (mockup row 125).

Not done yet: locking the pad's Size fields to "from sketch"; the table's
Pad N and Sketch value columns; the "drives pad N" label and the source
pad's corner handles on the sketch canvas; placing the floating Edit in
Sketch link next to the selected pad.

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
- **A live mirror.** With a pattern among the Mirror card's objects, OK
  adds a `PatternMirror { id, pattern, line }` to
  `SketchData::pattern_mirrors` instead of copying geometry, as one undo
  step. Every instance of the pattern is reflected about the solved Line on
  each bake, so the mirror follows the pattern: change the count and the
  mirror changes with it. Mirrored pads keep the pattern's numbering scheme
  and continue it (a row numbered 1 to 4 mirrors as 5 to 8); a pad's
  rotation is reflected too. Pad offsets and the shape itself are not
  reflected (a trapezoid keeps its side). Other objects are copied as
  geometry.
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

Every pattern bakes through one instance walk, `signex_bake::array::links`:
a plain array, a linked one, one with overrides and a mirror alike (#2689).
The canvas's instance rings of a selected pattern come from the same walk
(`signex_bake::pattern_slots`): each ring sits where the bake puts the pad,
with the per-instance spacing and the sweep's written unit; a slot the
suppression list holds back draws as suppressed, and a slot the mask drops
has no ring.

## Per-pad overrides {#per-pad-overrides}

Decision 9 (Caner, 2026-11-20, mockup row 192, item D1). Editing a Pads-mode
attribute of a pad that is an instance of a pattern (not the pattern's
source pad) asks, each time, **This pad / All pads / Cancel**; no answer is
remembered. The edit shows in the Properties panel while the card waits; a
change of size or position, which an override does not hold, does not ask.
The held value lives only in the card (`PadScopePrompt`), never in the
editor's pad list, so a Save, Save All or autosave while the card waits
writes the pad as it was, and the card stays open. The canvas draws the pad
as it was until the answer.

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
Caner 2026-11-20, mockup row 187 and mockup row 188). The constraint menu offers
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

A closed loop with a role shows the role's name just right of its
top-right corner, outside the loop, in the role's colour (Caner,
2026-11-24, mockup row 190); a pad's loop shows none. Courtyard fills in
the Courtyard Top colour; the other fills are unchanged (Caner,
2026-11-24). A selected loop with a role keeps its role's colour,
stronger, so the selection does not hide the role (mockup row 190). Jump
to pad and Back to pad sit just above the mode bar, left-aligned, also
while the bake preview is open, because the Bake preview card is in
Properties (Caner, 2026-11-24, mockup row 190).

A mirrored pattern draws a glyph by its mirror line, `⇔` and the mirrored
pattern's glyph (`⇔ L×4`). A pattern glyph click with Select selects the
pattern and its source and opens the pattern form. A mirror glyph click
selects it, and its menu has only
Delete mirror (one undo step; the pattern and the line stay). A pattern
whose source is another pattern names it: `P×4 of L×4`.

## Close gap {#close-gap}

A footprint bakes past an open profile (a courtyard, mask, paste, pour,
keepout or cutout loop that does not close) and leaves that shape out. When
the two nearest open ends are at most **0.05 mm** apart (the one heal
tolerance, `signex_sketch::heal::HEAL_TOLERANCE_MM`, shared with the board
sketch and the DXF import), the bake preview card's "Why it does not bake"
names the gap and the tolerance and offers **Close gap**; a wider gap says
it is wider than the tolerance and offers no button. Close gap joins the two
end points into one, re-solves and bakes, as one undo step. Same rule on a
board sketch: `PCB_SKETCH.md`, "Close gap".
