# PCB sketch: board geometry from the parametric sketch (#2234)

A board can carry a sketch, the same parametric sketch the footprint editor
uses (`signex_types::sketch::SketchData`, solved by `signex-sketch`). Entity
attributes say what each part of it becomes on the board. The sketch is the
source of truth; the bake turns it into board geometry, and one board
command stores both.

This page covers the domain half (storage, bake, engine command, DRC) and
the sketch-mode UI ([Sketch mode](#sketch-mode)).

## What a board sketch defines

| Entity attribute | Entity | Bakes to |
|---|---|---|
| `board_outline = true` | a loop of lines and arcs, or one circle | the board outline (`PcbBoard::outline`) |
| `keepout` (`KeepoutAttr`: layer, forbidden kinds) | a loop or a circle | a `PcbKeepout` |
| `board_cutout` (`BoardCutoutAttr`: edge radius, through) | a loop or a circle | a `PcbCutout`, an opening in the board |
| `mounting_hole` (`MountingHoleAttr`: drill, plated, net name) | a circle | a `PcbHole` at the circle's centre |
| `v_score` (`VScoreHintAttr`) | a line | a `PcbVScore` |

A loop needs only one tagged edge: the bake walks the closed profile through
it (`trace_closed_profile`), and edges tagged alike in one loop bake it once.
Arcs are tessellated with `ARC_SAMPLES` points, a circle with
`4 * ARC_SAMPLES` corners. Construction and centerline entities never bake
(`Entity::bake_skipped`), as in footprints.

A mounting hole's drill is its `drill_expr` (a length expression over the
sketch parameters), or the circle's diameter when unset. Its net is a net
**name**, turned into the board's net number at bake time, so renumbering the
net table does not detach it. A V-score keeps its depth as a fraction of the
board thickness, not in mm, so a thickness change needs no re-bake.

## The bake

`signex_bake::bake_board(&SketchData, &[NetDef]) -> Result<BakedBoard,
BoardBakeError>` is pure: it resolves the parameters, solves the sketch and
reads the solved positions. Every baked item takes the uuid of the entity it
came from (the first tagged edge of its loop, or the circle), so baking an
unchanged sketch again gives an equal result.

Unlike the footprint bake, which skips a broken feature with a warning, the
board bake refuses: a keepout or hole dropped without a word would pass DRC
unseen. `BoardBakeError` names the problem:

| Error | When |
|---|---|
| `Parameters` | the parameter table does not resolve (a cycle, a bad expression) |
| `Solve` | the sketch does not solve (`SolveError`: over-constrained, dangling reference, ...) |
| `OpenProfile` / `BranchingProfile` / `UntraceableProfile` | a tagged loop is not closed, branches, or cannot be walked; names the feature and the edge |
| `NotAnEdge` | a point carries an outline, keepout or cutout attribute |
| `MissingPoint` | a profile point has no solved position |
| `OutlineCount` | the outline tags more than one closed profile |
| `HoleNotACircle`, `HoleDrill`, `UnknownNet` | a hole on a non-circle, a drill not above 0, a net name the board lacks |
| `VScoreNotALine` | a V-score on an arc or circle |
| `Expression` | a cutout radius, drill or V-score depth that does not evaluate to a finite number |

**The outline is owned like a footprint's baked fields.** With an edge
tagged `board_outline`, the bake returns the profile. With none tagged, it
returns `None` (the board keeps its own outline), unless an earlier bake of
this sketch produced one (`baked_kinds` holds `board_outline`): then it
returns an empty outline, which clears the board's. `BoardSketch::from_bake`
records that token.

## The board command

`BoardCommand::SetBoardSketch { sketch, outline }` is the only way the sketch,
its keepouts, cutouts, holes and V-scores, or a sketch-owned outline change.
Build it with `BoardCommand::set_board_sketch(sketch, baked)`;
`BoardCommand::remove_board_sketch()` takes the sketch off and leaves the
outline as the last bake drew it, the board's own again.

- One undo step. Values move by `mem::replace`, so undo and redo restore the
  board bit for bit. Its inverse is another `SetBoardSketch`.
- Checked like every command (`check_batch`, all-or-nothing batches). A NaN
  or infinity anywhere in the sketch or its geometry is refused
  (`NonFiniteSketch`), as is a non-finite outline point.
- `SetBoardOutline` is refused (`OutlineOwnedBySketch`) while the sketch owns
  the outline.

The baked outline is written to `PcbBoard::outline`, so every reader of the
board area (the renderer, Update PCB staging, export) uses it with no change.

## In the `.snxpcb`

A new optional section, written only for a board that has a sketch. A board
without one writes no byte of it and re-saves byte-identical; a file written
before it loads unchanged. The section did not change the format token (`snxpcb/1.3` then): a build that
predates the section keeps it as a foreign top-level key and writes it back.

```toml
[board_sketch.sketch]
# SketchData in the serde form `.snxfpt` uses: planes, entities,
# constraints, arrays, parameters, baked_kinds. Lengths in f64 mm.

[[board_sketch.keepouts]]
uuid = "..."
boundary = [{ x = 10.0, y = 10.0 }, ...]   # f64 mm, closing vertex not repeated
layers = ["TopCopper"]                     # board-layer tokens

[board_sketch.keepouts.forbids]
no_routing = true
...

[[board_sketch.cutouts]]                    # uuid, boundary, edge_radius (mm), through
[[board_sketch.v_scores]]                   # uuid, from, to, depth_fraction, min_web (mm), side

[board_sketch.holes]
content = """
uuid  pos_x  pos_y  drill  plated  net
...   4000000  4000000  3200000  true  7
"""
```

- `keepouts`, `cutouts`, `v_scores` and `holes` are left out when empty.
- The holes are a TSV block like `[vias]`: positions and drill in integer nm,
  `plated` is `true` or `false`, `net` is a net number or empty. Unknown
  columns are kept per row.
- Polygons are TOML tables in f64 mm, like `[[zones]]` and the outline in
  `[extras.board]`. The baked outline stays in `[extras.board] outline`.
- Foreign keys inside `[board_sketch]` are kept (`BoardUnknown::board_sketch`).
- A non-finite number fails the save, naming its key path.

## DRC

`run_drc` reports a `Keepout` violation for each item inside a keepout that
forbids it; see `docs/architecture/pcb-drc.md`, "Keepouts".

## Sketch mode {#sketch-mode}

One command, `board_sketch` (Design ▸ Board Sketch, the PCB Active Bar's
Board Sketch button, the board canvas menu, the palette; `1` on a board in
both built-in profiles),
enters sketch mode on the board in view, or leaves it when it is on (as
Bake does). Only the main window's board can be in sketch mode, one board
at a time; entering on another board leaves the first one (committing it)
first.

**The editor is the footprint editor's sketch tool.** The mode hosts a
`FootprintDocument` that exists only in `BoardSketchSession`
(`app/state/board_sketch.rs`): its one footprint holds a working copy of
the board's sketch, keyed by the board's path. The footprint canvas, the
sketch Active Bar, the DOF chip and Properties row, the snap glyphs, the
solver and its off-thread solve all run unchanged on it.

### Hosts {#hosts}

One sketch editor serves two hosts, `SketchHost::Footprint` and
`SketchHost::Board` (`signex-types/src/sketch/host.rs`, #2518). The editor
asks `FootprintEditorState::sketch_host()` (a board sketch carries the
board backdrop) and nothing else. The host decides:

- **the role picker** (`SketchHost::roles`): a footprint keeps its fifteen
  roles; a board offers Board outline, Cutout, Keepout, V-score and
  Mounting hole, the targets `bake_board` writes today
  (`SketchHost::bake_targets`). A board role needs its shape: an outline
  an edge, a hole a circle, a V-score a line (`SketchRole::fits`); a pick
  on the wrong shape changes nothing;
- **the actions** (`SketchHost::offers`): Make Pad from Profile is greyed
  on a board, "footprint only";
- **the bake**: a footprint bakes on every solve; a board bakes only when
  Bake commits it.

Both hosts show the same Active Bar: Select | Create ▾ | Modify ▾ |
constraint strip | dimension input | Linetype (Normal / Construction /
Centerline segments) | Snap ▾. Create holds Point. The canvas draws the
board (outline, tracks, vias) faint under the sketch, on the dark board
ground. Properties adds a Board section: Board outline for an edge,
Mounting hole for a circle, and the hole's Drill (empty: the circle's
diameter), Plated and Net.

**Nothing touches the board or the disk while editing.** Undo and Redo
step the working copy while the mode is on. There is no auto-save.

### The way out {#way-out}

The mode bar is the same on both hosts: **Bake preview · Discard changes ·
Bake**.

- **Bake** (tooltip "Apply to board") commits a changed copy and ends the
  mode. A commit runs `bake_board` on the copy and, if it bakes, executes
  `BoardCommand::set_board_sketch` as one board step: undo, dirty and save
  are those of any board edit. If the bake (or the board command) refuses,
  the error text is shown on the notice card, in Messages and in the mode
  bar; the board is unchanged and the mode stays on.
- **Discard changes** ends the mode without committing. When the copy
  differs from what the board last took it asks first ("Keep editing" or
  "Discard changes"). It never writes.
- **Bake preview** opens a card with what Bake would write, a count per
  board target, or why the sketch does not bake; "Back to sketch" closes
  it. It writes nothing. While it is open the canvas draws the result
  (Graphite mockup row 190): a veil in the ground colour, then the baked outline over the
  board area, cutouts and mounting holes as openings, keepouts outlined
  and faintly filled, V-scores dashed, all in the theme slots of
  [Drawing the board](#drawing-the-board), with the sketch faint on top.
  The bake runs in `BoardSketchSession::refresh_preview` and the canvas
  reads it from `SketchExit::board_result`; a refused bake draws nothing.
  When the sketch does not bake, the card names why, and a click on the
  reason selects the entity the refusal names
  (`BoardBakeError::place`). The card also lists the failed
  [outline checks](#outline-checks), each selecting its place; they warn
  only.

A save, close, quit or undock still commits the copy and keeps the mode on
(`settle_board_sketch_of`), so a save never drops sketch work; the working
copy continues from what the board stored (with its `baked_kinds`).

**Esc** answers an open discard question with Keep editing, then drops an
armed sketch tool, a gesture in flight or an open menu (the footprint
editor's `ToolEscape`), then acts as Discard changes. Esc never applies the
sketch to the board.

**The mode ends with its board.** Closing the board's tab (parked or not),
a History restore or recovery copy read into it, or a failed reload ends
the mode (`Signex::end_board_sketch_of`). Uncommitted changes are dropped
and Messages names the board; the close prompt is not used, because it is
about saving the file, which never holds an uncommitted sketch. A reopened
board starts with a fresh copy. A session whose board is no longer open
(Save As renamed it) is dropped the same way when sketch mode next starts.

The DOF chip's second pill names the board (its file stem), with no pad
count.

The board sketch has the footprint sketch's info bar ("Board shape · plane
Top · grid …") and its Parameters button and table
([FOOTPRINT_SKETCH.md](FOOTPRINT_SKETCH.md#parameters)); on a board, Esc
with only the table open still acts as Discard changes, so close the
table with its Close button.

## Outline checks {#outline-checks}

With nothing selected, Properties shows an **Outline checks** section under
the Board summary (Graphite mockup row 179d). The checks run on the solve the editor already
has (no second solve) and walk the outline with the bake's own profile walk
(`signex_bake::board::outline_checks`), so they see what Bake would write.
They are read each time the panel is built, so every edit, Properties edit,
undo or solve shows them at once.

| Check | Fails with |
|---|---|
| Closed: one loop | each pair of open ends (the two edges and the gap between their ends, in mm), a fork, or the number of loops |
| No self-intersection | the two crossing edges and the point |
| Holes, cutouts and keepouts inside the outline | each opening that is not inside (a mounting hole by its drill; touching the edge counts as inside) |
| Inside corners: the router can cut them | each corner tighter than the router tool: a concave corner of the outline and every corner of a cutout or slot opening (radius 0 for a sharp corner, the cutout's edge radius when it names one, an arc's radius for a fillet), read `Inside corner R 0.00 < R 1.00 at (x, y)`; drilled mounting holes are skipped |

A check that cannot run says why: no outline drawn (the board keeps its
own), the outline not one closed loop yet, the sketch not solved, or (for
inside corners) the project's fab profile names no router tool. A failing
line names its place, and a click selects that entity (the first one, for a
pair).

The router tool diameter is the project's (`router_tool_diameter` in the
`.snxprj`), set when a fab tier or a fab profile naming one is applied
(`DRC_RULES.md`, "Fab profiles"). The board sketch session copies it from
the board's project on entry and after every edit; Properties shows it in a
**Fab profile** group (`Router tool Ø 2.00 · from profile`).

**Every outline check warns only; Bake still applies the sketch**
(`DECISIONS.md`, Caner 2026-11-20, decision 3).

## Close gap {#close-gap}

An open profile whose two nearest open ends are at most **0.05 mm** apart
(`signex_sketch::heal::HEAL_TOLERANCE_MM`, the one heal tolerance shared with
the DXF import, Caner 2026-11-20, decision 4) offers **Close gap**: on the open-outline
line of the Outline checks, and in the bake preview card when the bake
refuses an open profile. The line names the gap and the tolerance (`Gap 0.03
mm, closes under the 0.05 mm heal tolerance`); a wider gap says `Gap 0.12 mm
is wider than the 0.05 mm heal tolerance` and offers no button. Close gap
joins the two end points into one (every edge, constraint and pattern that
named the removed end names the kept one; a fixed end is kept), re-solves,
and is one undo step. The open ends are found by one helper on both hosts,
`signex_bake::open_ends`.

## Drawing the board {#drawing-the-board}

Outside sketch mode the PCB canvas draws the board's outline (the board
edge colour), its cutouts and mounting holes as openings (filled with the
canvas background, rimmed in the edge colour) and its keepouts as outlines
with a fainter 45° hatch (`signex-renderer` `pcb::board_features`, in the
overlay pass of both the CPU and the GPU path). The colours are the theme
slots `BoardEdge` (the canvas cursor ink), `BoardOpening` (the canvas
background) and `Keepout` (the DRC warning colour), so both Graphite themes
draw them in their own palette. V-scores are not drawn yet.

Copper (tracks, pads, vias, copper zones) is drawn in its layer's colour,
the one the Layers panel lists it with: the slots `TopCopper`,
`BottomCopper` and `LayerCycle0`..`LayerCycle5` (Mid-Layer `n` takes entry
`n % 6`), set from the board palette (`CanvasColors::board_layer_colors`).
A blind item takes its topmost layer's colour. Vias and through-hole pads
are drawn in `MultiLayer` (the Layers panel's Multi-Layer row, whose eye
shows or hides them), whatever the active layer. Copper on no layer this
build knows keeps its kind's slot. A through-hole pad's drill and a via's
drill are drawn as openings (`BoardOpening`) over their copper; a via whose
drill is as wide as the via, and a non-plated (NPTH) pad, draw as the
opening alone, with no copper. The board
area inside the outline is filled with `BoardArea` (the board palette's
`board_area`) under every zone and all copper; cutouts and mounting holes
stay open, since the overlay draws them over it. Decision: `DECISIONS.md`,
2026-11-11, #2262.

## Not done yet

- The board targets Slot, Pour, Room, Mechanical layer and Component
  anchor, and the Create rows Slot and Polygon (#2518): none is offered
  until it bakes. Modify offers Chamfer, Extend and Linear Pattern on both
  hosts ([FOOTPRINT_SKETCH.md](FOOTPRINT_SKETCH.md#modify-tools)); a board
  bake still bakes one instance of a pattern, and patterns bake their
  instances with #2517.
  Since 2026-11-20 the Create rows Slot and Polygon are offered on a board
  as on a footprint, and bake as any closed profile
  ([FOOTPRINT_SKETCH.md](FOOTPRINT_SKETCH.md#create-tools)).
- Drawing V-scores. (Exporting them is done: through cutouts go to the
  Gerber profile, mounting holes to the drill files and V-scores to their own
  Gerber image; see [FABRICATION_OUTPUTS.md](FABRICATION_OUTPUTS.md).)
- Sketch mode in an undocked board window.
- DRC: `no_pours` keepouts, holes in `no_drilling` keepouts, and
  copper-to-edge and copper-to-cutout clearance (the rules have no edge
  clearance yet).
- The ratsnest does not read plated holes: a hole with a net is not yet
  copper of that net. It reads no board area today, so the outline does not
  affect it.
- A corpus golden for a board with a sketch (`SIGNEX_BLESS_CORPUS=1`).
- Inside corners: the mockup's "Add fillets R 1.00" fix (not part of
  decision 3), and the Project tab's Project setup tier pick setting the
  router tool. Undoing a Presets apply puts the rules back but keeps the
  router tool it set (the rule history holds the rules only).

## Values, menus and patterns on the drawing

The board sketch shares the footprint sketch's values on the drawing, its
constraint and pattern menus, pattern glyphs, instance suppression and
conflict badge, on the same canvas and the same code
([FOOTPRINT_SKETCH.md](FOOTPRINT_SKETCH.md#values-on-the-drawing)).

## Snapping to the board {#snapping-to-the-board}

On a board sketch the cursor also snaps to the board drawn under the
sketch (#2518, Graphite mockup row 179): the outline's vertices and the
nearest point on an outline edge, placed pads' centres, drilled pads'
centres (plated and not), via centres, both ends of every track and the
corners of component courtyards (`snap_board.rs`). The backdrop carries
them (`BoardBackdrop::from_board`); a courtyard drawn as an arc or a
circle is not carried yet.

- **Point only.** A board-feature snap places the point there and lands
  no constraint. The point does not follow the feature when the board
  changes: references to board features are not decided yet.
- **Priority.** A sketch Point within reach still wins; the board comes
  next, before guides, intersections, the H / V and angle inference and
  the grid. Among board features a point (vertex, centre, track end,
  corner) wins over an outline edge, and a through-hole pad reads as a pad
  rather than its hole.
- **Reach.** The sketch Point snap's 8 px, in board mm at the current
  zoom.
- **Snap ▾.** The board sketch's Snap ▾ adds a Board features group: Snap
  to board features (one switch for all) and one row per kind (Board
  outline, Pad centres, Hole centres, Via centres, Track ends, Courtyard
  corners). All are on by default; the kind rows grey out while the switch
  for all is off. Like the other Snap ▾ rows they are the sketch editor's
  snap options, not board data: they record no undo step, are not saved,
  and start on again with the next sketch mode. The footprint
  sketch has no board and does not show the group.
- **Badge.** The badge names the feature: "Pad · J1-1", "Courtyard · J1",
  "Board outline", under a hollow square glyph (see
  [FOOTPRINT_SKETCH.md](FOOTPRINT_SKETCH.md#snap-badges)).

The backdrop also draws placed pads (top, bottom or Multi-Layer copper)
and component courtyards faint, in the board palette's colours, which are
the same in both Graphite themes.

## Pattern links on a board

A board sketch stores pattern sources, pattern mirrors and per-instance
overrides in `[board_sketch.sketch]` as a footprint sketch does
([FOOTPRINT_SKETCH.md](FOOTPRINT_SKETCH.md#pattern-links)), but only a
footprint bake expands them for now: a board bake still bakes one instance
per pattern until the instance walk of #2517, and ignores links and
mirrors. A board sketch holding any of them, or a reference dimension, is
saved as `snxpcb/1.7`, so an older build opens it read-only instead of
dropping them.

## Targets {#targets}

Decision 2 (Caner, 2026-11-20) adds four board targets to the role picker,
in the order of mockup row 179 without Room (rooms are not on the board yet): Board outline,
Cutout, Slot, Keepout, Pour, V-score, Mechanical layer, Component anchor,
Mounting hole (`SketchHost::Board.roles()`). The picker refuses a shape a
role cannot take (`SketchRole::fits`; `SketchRole::misfit` names the shape:
lines only, points only, circles only, edges).

| Role | Entity attribute | Entity | Bakes to |
|---|---|---|---|
| Slot | `board_cutout` + `board_slot` (`BoardSlotAttr`) | a closed loop | a `PcbCutout` with the stadium boundary, `edge_radius` = width / 2 |
| Pour | `pour` (`PourAttr`: copper layer, net name, priority, thermals) | a loop or a circle | a board `Zone`, `fill_type = "pour"` (`BakedKind::BoardPours`) |
| Mechanical layer | `mechanical` (`MechanicalAttr`: `MechanicalLayer`, 1-based) | a line, arc or circle | a `BoardGraphic` on `Mechanical<n>` |
| Component anchor | `component_anchor` (`ComponentAnchorAttr`: footprint uuid) | a point | the footprint's `position` |

A Slot profile is checked by `signex_bake::board::slot`: exactly four edges,
lines and arcs alternating, the lines parallel, of one length and twice the
arc radius apart, the arcs of one radius, tangent to both lines and bulging
outward. Anything else is `BoardBakeError::NotASlot` naming the seed edge. A
plain Cutout on the same loop is not checked.

The board pour is its own `BakedKind::BoardPours` (token `board_pours`), not
the footprint's `Pours`: a board bake answers for exactly
`SketchHost::Board.bake_targets()`, and `Pours` is a footprint field. A
pour's net is a name the board's net table must hold (`UnknownNet`), an
anchor's component a footprint the board holds (`UnknownComponent`; no
component picked is `AnchorWithoutComponent`).

Pours, mechanical drawings and anchors are not kept in `[board_sketch]`:
they are the board's own rows, baked by `signex_bake::board::bake_board_items`
(or `bake_board_with_items`, one solve for both). A zone takes the uuid of
its seed edge and a drawing that of its edge. The board command
`SetBoardSketchBake` (`BoardCommand::set_board_sketch_with_items`) does what
`SetBoardSketch` does and, in the same step, replaces every zone and drawing
whose uuid is an entity of the board sketch before or after, and moves each
anchored footprint that is not locked (Messages names a locked one). Its
inverse `RestoreBoardSketchBake` puts back the zone and drawing lists and the
old positions bit for bit. The app uses it only when the sketch owns one of
the three kinds now or did before (`SketchData::owns_board_items`), so a
sketch without them commits through `SetBoardSketch` as before.

The `.snxpcb` token is `snxpcb/1.7` while the board sketch carries any of
`board_slot`, `pour`, `mechanical` or `component_anchor`
(`SketchData::carries_board_targets`), else what it was (1.4, or 1.5 with
layer colours), byte for byte. Still to do: the drill and route output emits a
Slot cutout as a stadium cutout, not a routed slot.
