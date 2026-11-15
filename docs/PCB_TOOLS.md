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
- **Not yet.** Differential pairs (the net classes' `diff_pair_gap` and
  `diff_pair_width`), arcs, blind and buried vias, board-edge clearance,
  routing several layers in one search, and a route kept when its board's
  tab closes mid-route.

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
