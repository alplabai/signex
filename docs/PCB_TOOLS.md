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
- Track goes where the clicks are, through anything; Interactive Routing
  (below) keeps the clearance.

## Interactive Routing

`place_board_route` (no default key; Place, the PCB Active Bar). A route is
drawn on the active copper layer like a Track run, but each leg keeps the
clearance the design rules ask from other nets' copper (#2389).

- **Start.** The first click starts the route on the net of the copper under
  it (a pad or a track end), or no net over bare board, which then takes the
  net of the copper its next point lands on.
- **Rules.** The route reads the rules the Design Rule Check of this board
  reads (`DrcRules`, the same project layers and net classes, compiled once
  when the route starts): the clearance between each pair of items
  (`DrcRules::clearance_between`) and the width, the net class's track
  width, else `PcbSetup::trace_width`, held within the written rules'
  minimum and maximum width on the layer. The search for copper near a
  leg looks as far as the widest clearance any rule asks
  (`DrcRules::widest_clearance`), so a written rule between two object
  kinds (tracks and pads, say) is found as the check finds it.
- **Legs.** Each later click routes a leg from the route's end to the point
  with `signex_engine::pcb::route::route_leg`. The corner rule of #2372
  draws it when it is clear (Space and Shift+Space bend it); else the leg
  with the bend flipped; else an A* search on a lattice of the board's grid
  (coarser for a long leg) finds a way round in the corner mode's directions
  (90°: axis-aligned; 45°: axis and diagonal, each corner turning 45° at
  most; any angle: pulled taut) and joins the point with the rule's legs.
  With no way within the search budget the click adds nothing and the
  Messages panel says so.
- **Preview.** The leg on the pointer is routed once per change of what it
  depends on (the pointer's grid point, a leg, the board revision, the
  corner rule, the mode, the layer), not per repaint, with a smaller search
  budget than a click (`PREVIEW_BUDGET`); a leg the preview gives up on
  still routes on the click.
- **Push and shove.** `cycle_route_mode` (the palette; no default key)
  switches between walking around (the default) and pushing. In push mode
  the rule's legs go straight to the point and other nets' tracks in the
  way move square to themselves just far enough, carrying the tracks
  joined at their ends (whose far ends stay), and what they crowd is pushed
  in turn, a few rounds deep (`pcb_geom::route::shove`). A track that ends
  on a pad or via, one with same-net copper joined other than at a shared
  end (a branch ending on its side, an overlap: moving it would break that
  link), one the leg crosses, a pad or via in the way, or a move that would
  break a clearance (the leg's via included) or enter a keepout is not
  pushed: the leg walks around instead.
- **Vias.** Making another copper layer active mid-route puts a through via
  (the board's via size, grown to the written rules' minimum diameter and
  drill for the net) at the route's end with the next leg; a via that would
  not keep the clearance is refused with the leg.
- **Keepouts.** A leg stays out of a keepout that forbids tracks, a via out
  of one that forbids vias or drilling, as the Design Rule Check reads them.
- **One step.** The legs and the pushed tracks stay off the board, drawn on
  the cursor, until the route ends: a click on its end, Esc, a right-click,
  arming another tool (on any board or window), a Move, paste or part on
  the cursor, or ending this one. It then lands as one undo step
  (`RouteDraft::commands`: the vias and tracks added, the pushed tracks
  replaced), checked against the board revision it was routed on. The
  next Esc or right-click ends the tool.
- **A board that changed under the route.** An undo, or an edit from
  another window or panel, moves the board past the revision the route
  was routed on. The route is then checked again before its next leg and
  before it lands (`signex_engine::pcb::route::draft_fits`): every track
  it pushed must still be on the board as it was, and its tracks, vias and
  pushed tracks must keep the clearance from the board as it is now. A
  route that still fits goes on against the new revision. One that does
  not adds no leg and does not land: the Messages panel says why, the
  route stays drawn (undo or redo back to place it) and the next Esc
  discards it. When the tool ends with such a route (another tool, a Move,
  paste or part), the route is discarded with a Messages line; it is never
  dropped silently.
- **Not yet.** Arcs, blind and buried vias, board-edge clearance, routing
  several layers in one search, and a route kept when its board's tab
  closes mid-route.

## Differential Pair Routing and Multi-Route

`place_board_diff_pair` and `place_board_multi_route` (no default key;
Place, the PCB Active Bar's Route group). Several nets run side by side on
the active copper layer as one route (#2389).

- **Which nets.** Differential Pair Routing takes the pair of the net under
  the first click (`signex_engine::pcb::route::diff_pair_of`): the net
  named with the other ending of `_P`/`_N`, `_p`/`_n` or `+`/`-`, else,
  when the net's class sets a differential-pair gap and holds exactly two
  of the board's nets, the other one (the first by name is the positive).
  Multi-Route takes the nets of the selected tracks, pads and vias, two or
  more. A net in no pair, no selection or a net with no copper on the layer
  starts nothing, and the Messages panel says why.
- **Start.** Each net starts at its own copper nearest the first click (a
  pad centre, a track end or a via); the route's centre line starts in the
  middle of those points.
- **Width and gap.** The width is the first net's route width (as
  Interactive Routing's). A pair takes its class's differential-pair width
  and gap where they are set; the gap between neighbours is never under the
  clearance the rules ask between any two of the nets, which is also a
  bus's gap.
- **Legs.** Each later click routes the centre line to the point around
  other nets' copper (`route_bus_leg`): the search sees the whole bus as one
  wide track, half a pitch wider on each side so a right-angle bend's outer
  corner fits, and the route's own nets are no obstacle to it
  (`RouteWorld::with_kin`). Every member then follows the centre line at
  its offset (`pcb_geom::route::parallel_paths`, corners mitred), the
  member whose start lies leftmost of the first leg on the left, joined
  straight from its start. The member tracks are checked one by one against
  the board, their gaps to each other included (`draft_fits`); a leg that
  breaks a clearance, or a corner too sharp to follow side by side, adds
  nothing and Messages says why.
- **One step.** As for Interactive Routing: nothing lands until the route
  ends, then all members' tracks land as one undo step, checked against
  the board revision. The tracks stay on the layer the route started on; a
  via mid-route, push and shove, and joining the far ends to pads are not
  done yet.

## Quick Route

`place_board_quick_route` (no default key; Place, the Route group). The
first click starts on the net of the copper under it, at Interactive
Routing's width; the second click routes straight to the point, bent only
by the corner rule, and lands at once as one undo step. Other nets are not
avoided: the Design Rule Check reports what the track crosses. A copper
layer change between the clicks adds a via, as for a Track run.

## Length tuning

The track panel's **Tune Net To** field (#2389). Enter a length with one
track selected: the track becomes an accordion of square bumps in its
middle that brings its net's routed length to the target
(`signex_engine::pcb::route::tune_length`), one undo step. The bumps are
the fewest no taller than four pitches, a pitch being twice the track width
plus the board clearance, and never lower than half a pitch (a lower bump
would lay its copper over itself and add no real length); they go left of
the track when that side keeps the clearance from other nets, else right.
The panel's net length readout shows the result after Enter; a live
length readout while routing is not done yet. A net already as long, a
shortfall smaller than one lowest bump adds, a track too short for the
bumps, or no side clear is refused and the Messages panel says why.

The PCB Active Bar's **Tune** group runs the same accordion on nets:

- **Length Tuning** (`tune_net_length`) lengthens the nets of the selected
  tracks to the longest of them (`tune_nets`), each on its longest straight
  track that takes the bumps with a clear side, each tuned on the board as
  the ones before it left it. A net already at the length, or short by
  less than one bump adds, is left.
- **Differential Pair Length Tuning** (`tune_diff_pair`) lengthens the
  shorter net of the selected track's pair to the longer one's routed
  length (`tune_pair`).

Each is one undo step, and Messages says what was tuned or why nothing
was. An agent or the CLI names the board (`document`), the net (`net`;
else the selection, on the board in view only) and for Length Tuning the
target in mm (`value`); the answer lists each tuned net and its length.
Coupled (both nets at once) pair meanders are not done yet.

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

## Polygon Pour

`place_board_polygon_pour` (`P G`, #2216). Drawn like a Solid Region, on the
active copper layer, on the net of the copper under its first corner, or no
net. With a non-copper layer active the tool says so and draws nothing. It is
stored as a zone with `fill_type = "pour"` and `clearance = 0`, which follows
the board's clearance (`Zone::pour_clearance_mm`: the zone's own clearance
when above 0, else `PcbSetup::clearance`, else 0.2 mm).

A pour's outline only bounds its copper. The copper, its fill, is computed by
`pcb_geom::zone_fills`, in integer nm:

- the outline less the pour's own holes;
- less every pad, track and via on the pour's layer whose net is not the
  pour's, grown by the clearance (copper of no net counts as another net,
  and a pour of no net clears all copper);
- less every solid copper zone (Fill, Solid Region) of another net on the
  layer, grown by the clearance;
- less every keepout zone on the layer and every board-sketch keepout that
  forbids copper or pours on it, with no clearance;
- with every part narrower than the zone's `min_thickness` removed, when it
  is set.

Copper of the pour's own net is filled over: a pad joins the pour solidly, with
no thermal relief spokes (`thermal_relief`, `thermal_gap` and `thermal_width`
are kept but not read yet). Not modelled yet: pours of different nets that
overlap are not cleared from each other and both fill the overlap (zone
`priority` is not read); islands of fill that join nothing stay; the fill is
not clipped to the board outline; copper text and footprint copper graphics
are not cleared.

A round part of a grown obstacle is cut as a polygon whose chords lie on or
outside the true arc, so the fill never comes closer than the clearance. The
pieces are separate areas of copper: the ratsnest (`docs/architecture/pcb-ratsnest.md`)
joins pads through the piece they touch, so pads on two pieces of one pour get
a ratsnest line.

The fill is not saved: the file keeps the outline and the settings, and the
fill is recomputed from them. The app recomputes a board's fills with its
ratsnest, once per board revision (`PcbRatsnest::refresh`), and the canvas
draws each piece as copper and strokes the outline, on the GPU and the CPU
paths alike. A pour shown before its fill is computed (a preview on the
cursor) is drawn as its outline alone.

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
| Route | Interactive Routing, Differential Pair Routing, Quick Route, Multi-Route |
| Tune | Length Tuning, Differential Pair Length Tuning |
| Via / Pad | Via, Pad |
| Copper areas | Polygon Pour, Solid Region, Fill, Keepout |
| Copper drawing | Track, Arc |
| Dimensions | Dimension |
| Text | String |

Every board placement tool is a row of exactly one group, so the bar and
Place on a board list the same tools; Change Footprint is the Edit and board
canvas menu row of #2366, and the Tune rows are the length tuning commands
of #2389 (palette, no menu row).

- **Button.** It shows and runs the row its group ran last; until then, the
  first row that can run. A click runs it as its menu row does. It is drawn
  selected while one of its group's tools is armed on the board in the main
  window, as a schematic bar button is, and while its dropdown is open.
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

## Moving a component with its tracks

A drag or Edit ▸ Move of footprints takes the tracks connected to their pads
along (#2379). A track end follows when it lies on a moved pad's copper, on a
copper layer the pad reaches, and the track is on the pad's net: the contact
the ratsnest counts as connected (`pcb_geom::tracks_on_pads`). Only that end
moves; the other end stays, so the track stays joined to whatever it meets
there. The drop runs the footprint moves and the track edits as one batch
(`signex_engine::pcb::placement::move_commands`), one undo step. During the
drag the preview draws the carried footprints with those tracks at the
offset, and the ratsnest is read off the board as the drop would leave it,
so its lines stay on the carried pads. The tracks that follow are found once,
when the carry starts, and kept with the preview; each grid step only moves
them.

Only the drag and Edit ▸ Move take tracks along so far. Align, Distribute and
Align To Grid, the X and Y fields in Properties, and Turn and Mirror still
move the footprint alone and leave its tracks where they were. Turn and Mirror
need the track ends turned about the footprint, not shifted, so they need more
than the shift a move applies.

## Layer tab menu

A right-click on a layer tab under the board opens its menu (#2379, #2394):

- Hide <layer>, Highlight <layer> (every other layer is drawn dimmed;
  picking it again ends the highlight), Show only this layer and Show all
  layers. Show only this layer makes the layer active and hides every other
  layer the Layers panel lists; the board outline and the Multi-Layer row
  keep their state, as in single-layer mode.
- Hide layers ▸ and Show layers ▸, one row per kind: signal, plane, overlay,
  mask (solder mask and adhesive), paste, mechanical (board outline,
  documentation and mechanical layers) and all.
- Layer stack..., Drill symbols... and Layer drawing order... are greyed with
  the reason as their hover text: none of those editors exists yet (the layer
  stack editor is #2401).
- Short, Medium and Long tab names and Show layer numbers. Short names are
  `TL`, `BL`, `M1` (Mid-Layer 1), `TO`, `BS` and so on; Medium names are
  `Top`, `Bottom`, `Mid 1`, and a technical layer keeps its long name. A
  number goes in front: a copper layer's place in the stack (1 is the top),
  a mechanical layer's index; technical layers have none.
- Flip board view: the board seen from below. The view is mirrored left to
  right (the turn of the view, #2316, still applies on top), board text
  reads mirrored, a box selection keeps its left-to-right and right-to-left
  meaning on screen, and within one emphasis the copper nearer the bottom is
  drawn over the copper nearer the top. The active or highlighted layer still
  draws last.

The rows change the view of the board in the window the menu was opened in,
never the board file. The tab names are per user (`prefs.json`,
`pcb_layer_strip`); the flip is the board view's, for the session.

## Layer sets

The button at the left end of the layer tabs names the set the tabs list and
opens the set menu (#2394): All Layers, Signal Layers, Plane Layers,
Non-signal Layers (every layer but copper), Mechanical Layers, then the sets
the user saved, then Save current as set... and Manage sets.... Picking a set
shows its layers and hides every other listed layer (the board outline keeps
its state), and the tabs list only the set's layers. An active layer the set
hides moves into the set. All Layers and Show all layers list and show every
layer again.

Save current as set... turns the card into a name field; Enter or Save saves
the layers the view shows under that name (a set of the same name is
replaced) and shows it. Manage sets... lists the saved sets with Delete; it is
greyed while there are none. Saved sets are per user, in `prefs.json` under
`pcb_layer_strip`, and are offered on every board; a layer a board does not
have is left out when the set is shown there.

Every row is also a catalog command with typed operands: `show_layer_set`,
`save_layer_set`, `hide_layer`, `highlight_layer`, `hide_layer_group` and
`show_layer_group` name their board with `document` and are on the agent
channel; `delete_layer_set` and `set_layer_tab_names` change per-user
settings and name no board; `toggle_layer_numbers` and `flip_board_view` are
View menu rows. See `docs/COMMANDS.md`.
