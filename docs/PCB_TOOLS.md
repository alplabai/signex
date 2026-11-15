# Board placement tools

How the PCB editor's placement tools behave (#2373). The table of tools,
live and greyed, is `crates/signex-app/src/app/pcb_tools.rs`
(`BoardPlaceTool::state`); the Place menu on a board and the PCB Active Bar
are both built from it (see `MENU_BAR.md`, "Place on a board").

## Common behaviour

- **Arming.** Each live tool has a catalog command (`place_board_*`). Its
  Place row, its PCB Active Bar button, a key bound to it and the command
  palette all run that command, which arms the tool on the board in view of
  the window it ran in. Arming ends a Move, a paste or a part on the cursor
  and a Properties net pick; starting any of those ends the tool.
- **Points.** A clicked point lands on an object point within the snap
  distance (the Snap Options), else on the board's grid
  (`PcbSetup::grid_size`, else the PCB default) when grid snapping is on.
- **Preview.** What the next click adds follows the pointer, drawn by the
  board renderer in its layer's colour and dimmed, like a part on the
  cursor. Nothing reaches the board until a click.
- **Undo.** Each click that adds something is one board undo step
  (`BoardCommand::AddItems`, built by `signex_engine::pcb::draw`).
- **Ending.** Esc or a right-click drops the shape being drawn; with no
  shape in progress it ends the tool. A right-click that ends a tool does
  not open the board menu. A tab switch leaves the tool behind on its board.
- **Nets.** A via or track takes the net of the copper it starts on
  (`pcb_geom::net_at`, integer nm), or no net over bare board.

## Via

`place_board_via` (default profile: `P V` on a board). Each click places a
through via (top to bottom copper) with the board's via size
(`PcbSetup::via_diameter` and `via_drill`, else 0.6 mm and 0.3 mm).

## Track

`place_board_track` (default profile: `P T` on a board). A run is drawn on
the active copper layer; with a non-copper layer active the tool says so and
draws nothing.

- The first click starts the run on the net of the copper under it. A run
  started over bare board takes the net of the copper its next point lands
  on.
- Each later click adds the legs from the run's end to the point, bent by
  the corner rule wires use (#2372): Space flips the bend, Shift+Space steps
  the corner mode (90°, 45° start, 45° end, any angle). The rule is shared
  with wires and buses, and kept across runs.
- Width: `PcbSetup::trace_width`, else 0.25 mm. Net-class and rule widths
  are not read yet.
- Making another copper layer active mid-run (a layer tab, `+`, `-`, `*`)
  puts a via at the run's end in the same undo step as the next legs, and
  the run goes on on the new layer.
- A click on the run's end, Esc or a right-click ends the run; the next Esc
  or right-click ends the tool.
- No walk-around, shove or ratsnest following: that is Interactive Routing,
  still greyed.

## Fill and Solid Region

`place_board_fill` (`P F`) and `place_board_region` (`P R`). Both add solid
copper on the active copper layer, stored as a zone (`fill_type = "solid"`)
on the net of the copper under its first point, or no net. With a
non-copper layer active the tool says so and draws nothing.

- **Fill:** the first click is one corner, the second the opposite corner of
  the rectangle.
- **Solid Region:** each click adds a corner. A click on the first corner,
  or a right-click once there are three, closes it; Esc drops it.
- A shape whose corners are on one line has no area (checked in integer nm)
  and is not added; the Messages panel says so.

## Keepout

`place_board_keepout` (no default key). Drawn like a Solid Region, on the
active layer when it is a copper layer or a keep-out layer (Top or Bottom
Keep-Out Layer); on any other layer the tool says so and draws nothing.

It is stored as a zone with `fill_type = "keepout"` and no net, on the layer
it was drawn on, and drawn as a rule area. It keeps all copper (tracks, vias,
pads, pours) off one copper layer: the layer itself for a copper layer, the
side's outer copper for a keep-out layer (`zone_keepout::keepout_copper`).
The design rule check reads it through `Zone::as_keepout` exactly as it reads
a board-sketch keepout forbidding all copper, so a violation names the
keepout zone's id.

## String

Still greyed, with no catalog command or key yet. A string the canvas
cannot draw or pick would still be saved with the board and could not be
selected or deleted, so the row goes live only once board text is drawn
(#2387) and picked (a Text kind in `pcb_geom::ItemRef`).

The tool behind the row is in. Each click places a string
(`PcbBoard::texts`) on the active layer at the PCB default text size
(`PCB_DEFAULT_TEXT_SIZE_MM`) and unturned, reading the catalog's new-string
text (`pcb-tool-string-text`, "String"); the string carries no net. Only the
two overlay layers take a string (`PcbLayer::is_overlay`): on any other
layer a click places nothing and the Messages panel says to make an overlay
layer active, and nothing follows the pointer. Copper text would be copper
that the copper list, the design rule check and the ratsnest never read.

## Arc

Still greyed. The model is in: `signex_types::pcb_arc::TrackArc`, a copper
track along the circle through its start, a point on it (`mid`, which picks
the way it bends) and its end, with width, layer and net like a track.
`TrackArc::sweep` gives the circle and the signed turn (counter-clockwise
positive), `length_mm` the centre-line length and `points` the polyline
whose chords each stay within a given error of the arc (at most 360
chords), which is how drawing, hit-testing, the design rule check and the
ratsnest are to read it. Three points on one line are a straight track.

Still to come before the tool is live: the board list
(`PcbBoard::arcs`) and its `.snxpcb` rows, a `BoardItem` kind so it is
added and undone as one step, the renderer, the pick index, the copper list
the ratsnest and the design rule check share (`pcb_geom::collect_copper`),
and the three-click tool (start, end, then a point on the arc).

## Dimension

Still greyed. The model is in: `signex_types::pcb_dimension::LinearDimension`
measures the straight distance between two points, its dimension line
standing `offset` mm off them (positive: a quarter turn counter-clockwise
from start to end), on the Dimensions layer at the PCB default line width
and text height with two decimals. `LinearDimension::drawing` gives what it
draws: the two extension lines (running half a text height past the
dimension line), the dimension line, an arrow at each end (one text height
long) and the value's centre, a text height off the line, turned to read
along it and never upside down (text running straight down reads bottom to
top).

Still to come before the tool is live: the board list and its `.snxpcb`
section, a `BoardItem` kind, the renderer, and the tool (two clicks for the
points, a third for the offset).

## Pad

Still greyed. A pad placed on its own needs a board-level pad list next to
the footprints' pads, read by the copper list, the net table, the design rule
check, the ratsnest and the pick index; none of that exists yet.

## Selection Filter

The PCB Active Bar's first button. A click (or right-click) opens a
dropdown with `All - On` and a row per object kind, checked when on. A row
toggles its kind and the dropdown stays open; a click outside closes it. It
edits the same filter as the board Properties panel's chips (#2353),
through the same `PcbBoardMsg` messages, so the two always show the same
state. Kinds the board editor cannot pick yet are greyed with why, as their
chips are.

## Align

The PCB Active Bar's Align button opens a dropdown of the Align rows (Align
Left, Right, Horizontal Centers, Distribute Horizontally, Top, Bottom,
Vertical Centers, Distribute Vertically, Align To Grid). They run the same
commands as on a sheet; on a board they line the selected footprints up by
their origins, as a sheet lines symbols up by their anchors
(`signex_engine::pcb::align`), as one undo step. A selected pad stands for
its footprint. Two footprints are needed (three to distribute; one for Align
To Grid, which snaps each origin to the board's grid). A locked footprint
refuses the whole run and the Messages panel says which.

## Active Bar tool groups {#bar-groups}

Between Align and Board Sketch the PCB Active Bar has one button per tool
group (#2392), from the table in `crates/signex-app/src/app/pcb_bar_groups.rs`:

| Group | Rows |
|---|---|
| Components | Component, Change Footprint |
| Route | Interactive Routing |
| Via / Pad | Via, Pad |
| Copper areas | Polygon Pour, Solid Region, Fill, Keepout |
| Copper drawing | Track, Arc |
| Dimensions | Dimension |
| Text | String |

Every board placement tool is a row of exactly one group, so the bar and
Place on a board list the same tools; Change Footprint is the Edit and board
canvas menu row of #2366.

- **Button.** It shows and runs the row its group ran last; until then, the
  first row that can run. A click runs it as its menu row does.
- **Dropdown.** A right-click on a group with more than one row opens its
  rows. A live row runs its command and is greyed by the same gate as its
  menu row; a tool the board editor cannot run yet is greyed and its tooltip
  is the reason Place gives ("Place Pad: not available on boards yet").
- **No live row.** A group whose rows are all greyed is one greyed button
  with that reason as its tooltip and no dropdown.
- **Last used.** A tool armed from the group's dropdown, Place, a key or the
  palette becomes the group's button. Each group's last row is kept in
  `prefs.json` under `pcb_bar_last_tools` (group key to command id), written
  off the update thread, so it outlives the session. A stored command that is
  not a live row of its group reads as never used.
