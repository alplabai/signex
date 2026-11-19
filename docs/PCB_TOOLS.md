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

Board tools draw on the active layer (maintainer 2026-11-18). While one is
armed the status bar says how to use it, that Tab paused it, that it needs
another kind of layer, or why the last click added nothing; Messages keeps
the same lines. Arming a tool with a form brings Properties to the front on
what the next click places; the canvas keeps the keyboard, so a Properties
window of its own is left where it is.

- A run started on netted copper that does not reach the active copper
  layer (a surface pad on Top with an inner layer active) takes that
  copper's net and starts with a via there (the via of a layer change, in
  the same undo step as the first legs). A pad that reaches the active
  layer (through-hole) starts directly. A leg ending on such a pad ends
  with a via there, in the same step.
- The first click starts the run on the net of the copper under it. A run
  started over bare board takes the net of the copper its next point lands
  on.
- Each later click adds the legs from the run's end to the point, bent by
  the corner rule wires use (#2372): Space flips the bend, Shift+Space steps
  the corner mode (90°, 45° start, 45° end, any angle). The rule is shared
  with wires and buses, and kept across runs.
- Width: the track width of the net's class (the first class of the
  project's table that holds the net, #2400) when it sets one, else
  `PcbSetup::trace_width`, else 0.25 mm. Written rule widths
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
  net of the copper its next point lands on. Copper that does not reach the
  active layer (a surface pad on Top with an inner layer active) gives the
  route its net and a via there with the first leg, as for a Track run
  (`signex_engine::pcb::route::copper_under`). Quick Route starts the same
  way.
- **Ends.** A leg aimed at a pad of another net is refused; the status bar
  and Messages name the pad and both nets ("U1.1 is on net B, the route is
  on net A"). A route that ends on a pad of no net connects: when it lands
  the pad joins the route's net, locked as a connection made on the board,
  in the same undo step; the status bar says "U1.1 joined net A", and
  Update PCB lists the pad as keeping a net set on the board
  (`signex_engine::pcb::route::pad_end`, `join_pads`). A leg ending on a
  pad that does not reach its layer (a Top pad while routing on an inner
  layer) ends with a via there, checked like a layer change's via. Track
  runs end on such a pad with a via too.
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
  still routes on the click. A leg with no way, or aimed at a pad it may
  not end on, is drawn straight to the pointer in the DRC error colour
  instead of vanishing, and the status bar says why. The route on the cursor is drawn at the full
  colour of its layer, never dimmed.
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

### Router soak and bench

`signex-engine`'s `tests/routing/` measures the router against its quality
bar (#2482, #2483) on a synthetic board, the realistic corpus boards
(`signex-types/tests/corpus/pcb/real/`) and the routing corpus
(`testdata/routing-corpus/*.snxpcb`, net classes from the `.snxprj` beside
each board). The routing corpus (#2481) is 50 third-party boards under
their own licences, listed in `testdata/routing-corpus/MANIFEST.tsv` and
checked by `scripts/check_routing_corpus.sh`; its README says what the
licences allow. The boards are stored in Git LFS: in a clone without the
LFS objects each board is a pointer file, and the tests skip it with a
message (`git lfs pull` fetches them). With
`SIGNEX_REQUIRE_ROUTING_CORPUS=1` the load check fails on a pointer or an
empty corpus instead.

- **Soak.** A seeded driver applies random actions through `BoardEngine`:
  routes and shoves (one or two legs from a ratsnest line or a pad, landed
  as one step as the tool lands them), track and footprint moves, deletes,
  undo and redo. After every action: nothing panicked, the engine's history
  matches the steps taken, undo and redo give back the board exactly as it
  was, ids are unique, tracks are finite, on copper and on known nets, no
  zero-length track appeared, a refused route changed nothing, and a
  just-routed draft fits the board it was routed on. A failure prints its
  seed, action index and the command that replays it. A routed track laid
  over a same-net track already there (same layer, on one line, sharing a
  stretch) is counted in the run's report (`overlapped_tracks`), not
  failed, until the router defect #2492 is fixed. Not checked yet (#2482):
  dangling track ends and connectivity against the net list. The soak's own
  edits make both on purpose (a stub routed into open board, a track moved
  off its pads or across another net), so these checks must first tell an
  edit's effect from a defect.
- **Bench.** One-leg routes, half of them shoves: the rounds each shove
  took (`pcb_geom::route::shove_run`). A shove converged when it settled
  within 8 rounds: it cleared the way, or found a push that cannot be made
  (a pad in the way, a crossing track), after which the router walks
  around. One that ran out of rounds did not; the report also counts the
  shoves that cleared the way. Then the time from click to
  landed step, and the DRC delta: every design rule check finding a route
  adds, other than an unrouted link, names the route's own tracks or vias.
- **Runs.** `cargo test -p signex-engine --test routing` runs a short soak
  and bench with every test run (DRC delta and convergence asserted) on the
  fixed boards only: routing a corpus board in the debug test profile takes
  too long for per-PR CI. Per PR the routing corpus is checked by loading
  alone (`tests/routing/corpus_check.rs`): every board parses, names only
  layers this build knows (copper items on copper), and no track end lands
  on another net's pad once the pads are placed. The scheduled `routing-soak` workflow runs the ignored
  `soak_long` (10 000 actions on every board) and `bench_thresholds` (shoves converged within 8 rounds on 95%
  of actions; median latency at most 16 ms and p95 at most 33 ms on the
  median board by copper count) in release; a failed night opens or
  comments on the issue "Nightly routing soak is failing". `SIGNEX_SOAK_SEED`,
  `SIGNEX_SOAK_ACTIONS` and `SIGNEX_SOAK_BOARD` pick the seed, the length
  and one board (looked up in the whole corpus, also by the short runs).
  `routing-soak` checks out the LFS objects. `test · workspace` fetches
  them from a `.git/lfs` cache keyed on the boards' object ids, and sets
  `SIGNEX_REQUIRE_ROUTING_CORPUS=1`, on pushes, the merge queue and on a PR
  that changes the corpus, `signex-types`, `pcb-geom` or the routing tests.

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
  more. A net in no pair, no selection or a net with no copper on any
  layer starts nothing, and the Messages panel says why.
- **Start.** Each net starts at its own copper nearest the first click (a
  pad centre, a track end or a via); the route's centre line starts in the
  middle of those points. A net with no copper on the active layer starts
  from its copper on the topmost layer that has some, with a via there
  (maintainer 2026-11-18); the vias land with the tracks.
- **Width and gap.** The width is the first net's route width (as
  Interactive Routing's). A pair takes its class's differential-pair width
  and gap where they are set; the gap between neighbours is never under the
  clearance the rules ask between any two of the nets, which is also a
  bus's gap. The pitch carries 4 nm more, so corners rounded to whole nm
  never bring two members under that clearance.
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
on the net set in Properties (Tab), else the net of the copper under its
first corner on copper, or no net. With a non-copper layer active the tool
says so and draws nothing.

- **Fill:** the first click is one corner, the second the opposite corner of
  the rectangle.
- **Solid Region:** each click adds a corner. A click on the first corner
  (or within half a grid step of it), a double-click, or a right-click once
  there are three, closes it; Esc drops it. Polygon Pour and Keepout close
  the same way.
- A shape whose corners are on one line has no area (checked in integer nm)
  and is not added; the Messages panel says so.

## Keepout

`place_board_keepout` (`P K` in the default profile, `K` in Classic, on a
board). Drawn like a Solid Region, on the
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
active copper layer, on the net set in Properties (Tab), else the net of the
copper under its first corner on copper, or no net; a pour placed with no
net says so in the status bar and Messages. A pour with no fill (on no
copper layer, an outline or clearance the geometry cannot hold) says why in
Messages when placed and in its Properties (`pcb_geom::Unfilled`). With a
non-copper layer active the tool says so and draws nothing. It is
stored as a zone with `fill_type = "pour"` and `clearance = 0`, which follows
the board's clearance (`Zone::pour_clearance_mm`: the zone's own clearance
when above 0, else `PcbSetup::clearance`, else 0.2 mm).

A pour's outline only bounds its copper. The copper, its fill, is computed by
`pcb_geom::zone_fills`, in integer nm:

- the outline less the pour's own holes;
- less every pad (free pads #2373 included), track, free copper arc and via
  on the pour's layer whose net is not the pour's, at its size on that layer
  (#2401), grown by the clearance (copper of no net counts as another net,
  and a pour of no net clears all copper);
- less every solid copper zone (Fill, Solid Region) of another net on the
  layer, grown by the clearance;
- less every keepout zone on the layer and every board-sketch keepout that
  forbids copper or pours on it, with no clearance;
- less the fills of the other nets' pours on the layer that filled before it,
  grown by its clearance (pour-to-pour clearance);
- inside the board outline less its through cutouts, when the board has an
  outline;
- with every part narrower than the zone's `min_thickness` removed, when it
  is set;
- with every island (separate piece) smaller than `min_island_area` (mm²)
  removed, when it is above 0, and, with `remove_dead_islands`, every island
  that touches no pad, free pad, track, free copper arc, via or solid copper
  zone of the pour's net (a
  pour of no net keeps them). The fill counts the islands it removed.

Pours fill in priority order (#2216): a higher `priority` first, equal
priorities in board order. A pour of the same net may overlap an earlier one;
one of another net keeps its clearance from it. So of two overlapping pours
of different nets, the higher priority keeps the overlap.

Copper of the pour's own net is filled over: a pad joins the pour solidly, with
no thermal relief spokes (`thermal_relief`, `thermal_gap` and `thermal_width`
are kept but not read yet). Not modelled yet: copper text and footprint copper
graphics are not cleared.

### Picking and editing a pour

A placed pour is picked inside its outline (`PickKind::Polygons`, the
Polygons chip of the selection filter) and opens the area panel: net, layer,
clearance, minimum width, Priority (a whole number from 0), Remove Islands
Below (mm²), Remove Dead Copper, and the copper its fill holds (area and
pieces), read from the fills the app holds. Each edit is one undo step.
`min_island_area` and `remove_dead_islands` are written to the file only when
set.

### Pour Cutout and Slice Pour

Both work on the polygon pours of the active copper layer and are one undo
step each (`signex_engine::pcb::pour_edit`, geometry in
`pcb_geom::{cut_area, slice_area}`):

- **Pour Cutout** (`place_board_pour_cutout`) is drawn like a Solid Region.
  Closed, its area is cut out of every pour it overlaps: a hole when inside,
  a notch at an edge, or several pieces.
- **Slice Pour** (`place_board_slice_pour`) takes two clicks; every pour the
  segment between them meets is cut in two along the line through them.

The largest piece keeps the pour (its uuid and settings); every other piece
becomes a new pour with the same settings, and a pour the cutout covers whole
is removed. When nothing is cut, Messages says so and the board is unchanged.

### Repour

`repour_selected` and `repour_all` (Tools, the Copper Areas group of the
Active Bar, the palette) fill the board in view again at once and say in
Messages how many pours, pieces of copper and removed islands they came to;
Repour Selected reports the selected pours. Fills depend on each other, so
the whole board is filled either way. Nothing in the document changes, so
there is no undo step. `repour_all document=<board>` answers an agent or the
CLI with each pour's `zone`, `net`, `net_name`, `layer`, `priority`,
`filled`, `pieces`, `area_mm2` and `islands_removed`.

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

`place_board_string` (no default key). Each click places a string
(`PcbBoard::texts`) on the active layer at the PCB default text size
(`PCB_DEFAULT_TEXT_SIZE_MM`) and unturned, reading the catalog's new-string
text (`pcb-tool-string-text`, "String"), or the text, height and turn set in
its placement form; the string carries no net. A layer takes a string when
`PcbLayer::takes_text` holds: the two overlay layers, the mechanical layers
and the documentation layers (Dimensions among them). On any other layer a
click places nothing and the Messages panel names the layers that do.
Copper text would be copper that the copper list, the design rule check and
the ratsnest never read. The canvas draws it (#2387); a click or box picks
it (`pcb_geom::ItemRef::Text`, the selection filter's Texts kind).

## Arc

`place_board_arc` (no default key) draws an arc on the active layer, as one
undo step per arc. `cycle_arc_mode` (palette, no default key) steps how the
clicks make it (`signex_engine::pcb::draw::ArcMode`); the Messages panel and
the placement form name the mode:

- **By centre** (the default): the centre, then the start, which sets the
  radius, then a point whose direction from the centre ends the arc. The arc
  runs counter-clockwise on screen from start to end, any angle up to a full
  turn.
- **By edge**: the two ends of a quarter circle.
- **Any angle**: the start, the end, then any point the arc bends through.

Points that make no arc (a zero radius, three points on one line) add
nothing and say so. The model is `signex_types::pcb_arc::TrackArc` (start,
a point on the arc, end, width, layer, net), kept in `PcbBoard::free.arcs`.
On a copper layer an arc is copper of the net under its start (or the net
set in the form): the copper list, the ratsnest, the design rule check (as a
track: clearance, width rules and keepouts) and fabrication outputs read it
along its chords (`pcb_geom::ARC_CHORD_ERROR_MM`, 5 µm). Its chords are one
copper: the ratsnest joins them, and DRC reports one finding per pair of
items, the worst of their chords'. On any other layer it is a drawing
with no net. Width follows `PcbSetup::trace_width` unless the form sets it.

The Arc panel shows Net Information and Net (copper only), Layer (any board
layer; moving off copper drops the net), Width, Centre, Radius, Start and End
Angle (read from the board origin with Y up, counter-clockwise from start to
end) and the length; each edit is one undo step
(`primitive_edit::ArcEdit`, `BoardCommand::ReplaceItems`).

## Dimension

`place_board_dimension` (no default key) places a linear dimension: two
clicks for the points it measures, a third for where its line stands (the
offset is how far the third point stands off the measured line). It goes on
the active layer when that takes text, else on the Dimensions layer. The
model is `signex_types::pcb_dimension::LinearDimension`, kept in
`PcbBoard::free.dimensions`: two extension lines, the dimension line, an
arrow at each end and the value (`value_text`, to `precision` decimals, at
most 6), drawn by the board renderer and picked by its strokes and the box
of its value. Two equal points add nothing and say so.

The Dimension panel shows Start, End, Offset, Line Width, Text Height,
Precision, the value and Layer (layers that take text); each edit is one
undo step. Angular, radial, diameter, leader, baseline and centre-mark
dimensions have no model yet and are not offered.

## Graphics {#graphics}

Place ▸ Graphics on a board and the PCB Active Bar's Graphics group (#2424):
`place_board_line`, `place_board_graphic_arc`, `place_board_circle`,
`place_board_rectangle` and `place_board_text_frame` (no default keys). Each
draws a board drawing (`PcbBoard::graphics`, kinds in
`signex_types::pcb_graphic`) on the active layer, built by
`signex_engine::pcb::graphic`, one undo step each.

- **Layer.** Any non-copper layer but the two keep-out layers, whose areas
  the design rule check reads as rules: overlay, solder mask, paste, fab,
  assembly, courtyard, mechanical, documentation, the board outline. On
  another layer a click adds nothing and the Messages panel says so.
- **Line.** The first click starts a run; each later click adds a line from
  the run's end, any angle. A click on the run's end, Esc or a right-click
  ends the run.
- **Arc.** The centre, then the start (its distance from the centre is the
  radius), then the end, moved onto the circle along its angle. The arc runs
  from the start the way of increasing angle in the model's axes, clockwise
  on screen; an end on the start's angle is a full turn. Until the end the
  pointer draws the full circle.
- **Full Circle.** The centre, then a point on the circle.
- **Rectangle, Text Frame.** Two opposite corners; a box with no area adds
  nothing and the Messages panel says so. A text frame reads the text and
  height set in its form, else the catalog's new-frame text ("Text") and the
  PCB default height; its text wraps inside the box (`docs/RENDERING_RULES.md`,
  "pcb-text-frame").
- **Form.** Properties shows the drawing placed next (#2393): Layer (the
  active layer), Width (`PCB_DEFAULT_GRAPHIC_WIDTH_MM` until set), and for a
  text frame its text and text height. Tab pauses placing to edit them.
- **Placed drawings.** A drawing is picked on its stroke (a text frame also
  inside its box) under the selection filter's Other kind (Texts for a text
  frame); Properties edits its layer, width, corners (line, rectangle, text
  frame) or centre and radius (arc, circle), and a text frame's text, where
  `\n` is a line break, and text height, one undo step per field
  (`signex_engine::pcb::primitive_edit::GraphicEdit`). Delete removes it.
- **Output.** Saved in `.snxpcb` (`[[extras.board.graphics]]`, each with its
  `uuid`; a text frame's `text` and `font_size`), drawn by the board
  renderer, printed by Board Print (#2402), and drawn into the fabrication
  layer's Gerber (#2388) when the layer is one (overlay, mask, paste, the
  board outline); a text frame's text waits there for the stroke font, as
  board texts do.
- **Agents.** `add_board_graphic` (`document`, `kind`: `line`, `arc`,
  `circle`, `rectangle` or `text_frame`; `start` and `end`, an arc's or
  circle's centre `at`; optional `layer` and `text`) and `set_board_graphic`
  (`document`, `id`, `field`, `value`) take points and X/Y from the board
  origin with Y up, are one undo step each and answer with the drawing as
  JSON.

## Pad

`place_board_pad` (no default key) places a free pad, one per click: a pad
with no footprint, kept in `PcbBoard::free.pads` at its board position. By
default it is a round through-hole pad 1.5 mm across with a 0.8 mm hole on
the top and bottom copper, numbered `1`, on the net of the copper under it.
Code that reads footprint pads reads a free pad in the board's own frame
(`signex_types::pcb_free::free_pad_frame`: a footprint at the origin, not
turned), so the copper list, ratsnest, design rule check, pick index,
renderer, drill file and netlist test points treat it as any pad. A click
selects it and the pad panel edits it, each edit one undo step.

## Placement form (Tab) {#placement-form}

While a tool is armed, Properties shows the object it places next (#2393),
with the rows of the placed object's panel: Via (Diameter, Hole Size, Net),
Track and Interactive Routing (Width, Net), the areas, Pad (Designator,
Shape, X/Y Size, Hole Size, Rotation, Net), Arc (Placement mode, Net, Layer,
Width), String (Text, Height, Rotation, Layer) and Dimension (Line Width,
Text Height, Precision, Layer). The location and an arc's shape come from
the clicks, so the form leaves them out. Tab (`placement_properties`) pauses
placing and focuses the form's main field (a via's Diameter, a track's or
arc's Width, a pour's Clearance, a pad's X-Size, a string's Text, a
dimension's Line Width); Enter or Tab again goes on with
the values; Esc puts back the values from before the pause and keeps the
tool armed. A value is checked as an edit of a placed object is (a size not
above zero, a hole wider than the pad, a precision that is not a whole number
from 0 to 6 are refused and say why). A Layer pick moves the active layer.
The values stay the tool's for the session (`BoardToolDefaults`).

## Saving free primitives

Free arcs, pads and dimensions are saved in the optional `[free]` section of
the `.snxpcb` (`signex_types::format::board_free`), lengths in integer nm.
A board without any has no section and re-saves byte-identical. Each item
has its own uuid in the board's one id space; a duplicate is repaired on
load like any other board object.

## Selection Filter

The PCB Active Bar's first button. A click (or right-click) opens a
dropdown with `All - On` and a row per object kind, checked when on. A row
toggles its kind and the dropdown stays open; a click outside closes it. It
edits the same filter as the board Properties panel's chips (#2353),
through the same `PcbBoardMsg` messages, so the two always show the same
state. Kinds the board editor cannot pick yet are greyed with why, as their
chips are. Since #2373 Arcs picks free arcs, Texts strings, Pads free pads
as well as footprint pads, and Other dimensions (`PickKind::of`).

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
| Copper areas | Polygon Pour, Pour Cutout, Slice Pour, Solid Region, Fill, Keepout, Repour Selected, Repour All |
| Copper drawing | Track, Arc |
| Dimensions | Dimension |
| Text | String |
| Graphics | Line, Arc, Full Circle, Rectangle, Text Frame |

Every board placement tool is a row of exactly one group, so the bar and
Place on a board (its Graphics submenu for the Graphics group) list the same
tools; Change Footprint is the Edit and board canvas menu row of #2366, and
the Tune rows are the length tuning commands of #2389 (palette, no menu row).

- **Button.** It shows and runs the row its group ran last; until then, the
  first row that can run. A click runs it as its menu row does. It is drawn
  selected while one of its group's tools is armed on the board in the main
  window, as a schematic bar button is, and while its dropdown is open.
- **Dropdown.** A right-click on a group with more than one row opens its
  rows. A live row runs its command and is greyed by the same gate as its
  menu row; a tool the board editor cannot run yet is greyed and its tooltip
  is the reason Place gives ("Place Component: not available on boards
  yet").
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

## Moving a designator or comment on its own

A press on a component's designator or comment drags that text alone
(#2368); the component and its tracks stay. The text follows the pointer
without snapping and the release is one undo step. A locked component, or
one that locks its strings, keeps its text where it is and Messages says
why. A click selects the text; a box or Select All takes the component, not
its text. The text keeps its place relative to the component, so it still
follows a later move, turn or flip. Agents use `set_footprint_text
document=<board> designator=<ref> text=designator|comment field=<field>
value=<v>` with `field` one of `x`, `y` (from the board origin, Y up),
`rotation`, `height` and `justify` (`left`, `center`, `right`). Rules:
`docs/RENDERING_RULES.md`, "pcb-footprint-text".

## Layer tabs

The strip under the board has a tab for every layer the Layers panel shows
that is not hidden, in the panel's order: the copper top to bottom, each
component pair's top then bottom layer, Board Outline, Margin, Dimensions,
Notes, ECO 1 and 2, Drill Drawing, the board's mechanical layers, then
Multi-Layer while it is shown (2026-11-18). Hiding a layer, by its eye in the
panel or its tab's Hide row, takes its tab away; Show layers ▸ or the eye
brings it back. The strip scrolls sideways when the tabs do not fit. A tab's
swatch is the layer's colour on this board.

## Layer tab menu

A right-click on a layer tab under the board opens its menu (#2379, #2394):

- Hide <layer>, Highlight <layer> (every other layer is drawn dimmed;
  picking it again ends the highlight).
- Layer colour..., Reset layer colour and Reset all layer colours
  (2026-11-18). Layer colour... opens the Layers panel on Layers & Colors with
  that layer's colours open under its row; it is greyed for the keep-out and
  mechanical layers, which have no colour row yet. A colour is saved with the
  board (`[board_options.layer_colors]`, `docs/PCB_BOARD_OPTIONS.md`), one
  undo step; the resets are greyed while there is nothing to reset.
- Show only this layer and Show all layers. Show only this layer makes the
  layer active and hides every other layer the Layers panel lists; the board
  outline and the Multi-Layer row keep their state, as in single-layer mode.
- Hide layers ▸ and Show layers ▸: per kind (signal, plane, silkscreen, mask
  (solder mask and adhesive), paste, mechanical (board outline,
  documentation and mechanical layers)) a row for the whole kind, then the
  layers of that kind the row would change, one by one: the shown ones under
  Hide, the hidden ones under Show. All layers ends the list.
- Layer stack... runs `board_layer_stack` (#2401) from the window the menu
  was opened in and is greyed exactly when that command is. Drill symbols...
  and Layer drawing order... are greyed with the reason as their hover text:
  neither editor exists yet.
- Short, Medium and Long tab names and Show layer numbers. Short names are
  `TL`, `BL`, `M1` (Mid-Layer 1), `TO` (Top Silkscreen), `TS` (Top Solder)
  and so on; Medium names are `Top`, `Bottom`, `Mid 1`, and a technical layer
  keeps its long name. A number goes in front: a copper layer's place in the
  stack (1 is the top), a mechanical layer's index; technical layers have
  none.
- Flip board view: the board seen from below. The view is mirrored left to
  right (the turn of the view, #2316, still applies on top), board text
  reads mirrored, a box selection keeps its left-to-right and right-to-left
  meaning on screen, and within one emphasis the copper nearer the bottom is
  drawn over the copper nearer the top. The active or highlighted layer still
  draws last.

The rows change the view of the board in the window the menu was opened in;
only the colour rows change the board file. The tab names are per user
(`prefs.json`, `pcb_layer_strip`); the flip is the board view's, for the
session.

## Board canvas menu

A right-click on the board opens a menu that follows what is under the
pointer (2026-11-18). On an empty area with nothing selected: Find, Place ▸,
Undo, Redo, Paste, Paste Special, Select All, Board Sketch, the reuse block
Place rows and Toggle Ratsnest. On an object, or with a selection, Find
Similar Objects and the rows that act on it follow: with a selection Align ▸,
Cut, Copy, Duplicate, Move Selection, the turns and mirrors, Save as Reuse
Block, Delete, Board Shape and Keepout from Selection; Change Footprint, Swap
Pins, Swap Gates and Arrange in Box only with a footprint selected; Lock,
Unlock, Gloss Selected Routes and Add Teardrops only with a track or via
selected; Select Net and Highlight Net only when the selection or the object
under the pointer has a net; Cross Probe and Properties. Each row runs its
catalog command and is live by that command's own rule.

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
View menu rows; `edit_layer_color` opens the colour picker of the active
layer and `reset_layer_colors` gives every layer the palette colour again
(`set_layer_color` sets one, saved with the board). See `docs/COMMANDS.md`.

## Board view configuration

The Layers panel configures the board view (#2363). It has a search field,
which keeps the rows whose name holds the text, and two tabs.

**Layers & Colors**, top to bottom:

- Layer Sets: the set dropdown (the built-in sets, then the saved ones), a
  name field with Save, and Delete for the saved set in view. They do what
  the layer set button does.
- Active Layer: the same choice as the layer tabs.
- View From Bottom Side: the flip of the layer tab menu.
- Import... and Export...: greyed until a view can be saved to a file.
- All Layers: its eye hides every layer, or shows them all while one is
  hidden. Used On lists only the layers the board has objects on.
- Signal and Plane Layers: the copper layers, each with its number in the
  stack and its type.
- Component Layer Pairs: Top | Layer | Bottom, an eye and a colour chip per
  side. The rows map onto the named technical layers: 3D Body, Assembly,
  Component Outline (the fabrication layers), Courtyard, Designator,
  Silkscreen, Paste, Solder (solder mask), Adhesive, Fiducial and
  Keep-Out. Coating, Component Center, Tenting and Value have no layer and
  are greyed.
- Other Technical Layers: Board Outline, Margin, Dimensions, Notes, ECO 1
  and ECO 2. Other Layers: Drill Drawing, Drill Guide (no layer yet,
  greyed) and Multi-Layer (vias and through-hole pads).
- System Colors: the eyes of Connection Lines (the ratsnest) and the Origin
  Marker work; the other rows and the system colours are greyed.

A click on a layer's name makes it the active layer. A click on a chip opens
the colours under the row: twelve presets, Custom... and the palette colour
again. The colour is an override on top of the one board palette for this
board view only; the canvas (the Board Outline colour draws the board edge
too), the chip and the board texts draw it. The Keep-Out chips are greyed:
the board's keepout regions keep the palette colour.

**View Options**: the configuration preset (greyed), 3D Off/On, Single Layer
Mode Off/On (Mono is greyed) and Show Grid. Object Visibility lists All
Objects (Mixed while the rows differ) and one row per kind of object, each
with an eye, Draft (greyed) and a Transparency slider. Pads, Vias, Arcs,
Tracks, Polygons, Regions, Fills, Texts, Dimensions and Keepouts can be
hidden; a hidden kind is neither drawn nor picked. Texts covers footprint
texts, board strings and text frames, box and all. Pads, Vias, Arcs (copper
arcs; Tracks never fades them), Tracks, Polygons, Regions and Fills can be
drawn transparent. Mask and Dim Settings and the Additional Options chips
are greyed until the view can draw them.

All of this is the board view's state, for the session; nothing touches the
board file. Saving the view, presets and every greyed control are tracked in
#2480. Every control has a catalog command that names its board with
`document`: `set_layer_visible`, `set_active_layer`, `set_layer_color`
(without `color`, the palette colour again), `show_used_layers_only`,
`set_object_visible` (`kind=all` for All Objects) and
`set_object_transparency` (`value` in percent), next to the layer strip's
commands above.

## Reading a board

Measure, Find and Replace, Find Similar Objects and the nets work on a board
as on a sheet (#2403). The numbers and matches come from the domain crates
(`pcb_geom::edge_gap`, `pcb_geom::net_reports`,
`signex_engine::pcb::{find, similar}`); the app keeps the view in step.

- **Measure Distance** (`measure_distance`, `Ctrl+M`): the first click marks a
  point, snapped like a placement tool's, and the object under it; the ruler
  follows the pointer to the second click. The Messages panel reads the
  distance with dx and dy in the display unit (X right, Y up, as the
  coordinate readouts count; measuring upward gives a positive dy) and, when
  both clicks landed on
  an object, the gap between them edge to edge (copper, footprint body or
  area outline). The ruler stays until the next first click; Esc drops the
  first point, then ends Measure.
- **Find and Replace** (`find`, `find_and_replace`): with a board in view the
  dialog searches its designators, values, footprint names and net names,
  without case. A result selects what it names (a footprint, or the net's
  tracks, vias and pads) and frames it. Replace writes designators and values
  as one undo step; net and footprint results are left as they are, and the
  Messages panel says how many.
- **Find Similar Objects** (`find_similar_objects`): the seed is the object
  under the cursor, else the one selected; the dialog lists its properties
  (kind, value, footprint, designator prefix, layer, width, size, drill, pad
  shape, net) and OK selects every object of the board that matches the ones
  set to Same.
- **Select Net** and **Highlight Net** (`select_board_net`,
  `highlight_board_net`): the net `net=<name>` names, else the net of the
  selection or of what is under the cursor. Select puts its tracks, vias,
  pads and areas in the selection; Highlight draws its copper bright until
  it is run again for that net or Esc is pressed. Neither changes the
  board. A name the board does not have is refused.
- **Net Inspector**: lists the board's nets (see `MENU_BAR.md`, "Tools ▸ Net
  Inspector").

Agents and the CLI read the same answers with `measure_board`,
`query_board_nets` and `find_similar_items` (`CLI.md`, "Board queries").

## Board Tools

Seven Tools rows edit the board in view (#2408, #2409); the menu side is in
`MENU_BAR.md`, "Tools: board edits". Each builds its edit in
`signex_engine::pcb::board_tools` from geometry in `pcb_geom` (integer nm)
and runs it as one board undo step. The same command with operands acts on
the board an agent or the CLI names by `document`, on items named by uuid
(`id`, `ids`) or by point (`start`, `end`), and answers with what it did:
the uuids added, removed and changed, and how many places it left out.
Copper keeps the clearance the board's setup, its project's net classes and
the written DRC rules ask for between two nets: the rules a check of the
board applies (`board_design_rules`), read per net pair
(`DrcRules::net_pair_clearance`, a class-pair rule included; a rule for one
object pair takes no part, since the tool knows only the nets), measured
between pads (free pads #2373 included), tracks, free copper arcs and vias,
each at its size on the layer (#2401), as the design rule check measures it,
so the result adds
no clearance or short finding. Fills, Solid Regions and teardrops of other
nets are kept clear too, though the check does not look at them; pours are
not obstacles, since a pour's fill clears new copper of other nets. Two
items on no net count as different nets here, the safe side. A board setup
clearance the geometry cannot hold refuses the copper tools (stitching,
teardrops, gloss) instead of guessing a value. The work runs on the update thread,
like Align. A refusal says why in Messages and changes nothing.

### Via Stitching

`stitch_vias` fills an area with vias of one net on a square grid. From the
menu the area is each selected Fill or Solid Region, or, with nothing
selected, every polygon pour that has a net, and the net is the area's; a
selection with no area in it is refused; the grid step is the smallest
multiple of the board's snap grid that is at least 2 mm. With operands the
area is a zone (`id`) or a rectangle (`start`, `end`), `net` names the net by
name (needed for a rectangle or a zone with no net) and `pitch` the step in
mm. A pour is stitched over its computed fill, any other zone over its
outline less its holes.

A via goes on a grid point (a multiple of the step, so a second run lands on
the same points) when its whole disc lies inside the area, inside the board
outline and clear of the board sketch's cutouts and mounting holes by the
board clearance, clear of copper of every other net on any layer (no net
counts as another) by the clearance, clear of pads and vias of its own net,
and outside every keepout that forbids vias. Vias take the board's via size
(`PcbSetup` diameter and drill). A step smaller than the via is refused, and
so is an area where no via fits. Stitching an area again adds nothing.

### Teardrops

`add_teardrops` adds a teardrop where a track enters a pad or via of its own
net on the track's layer: a straight-sided taper along the track's line,
from a base across the track at the point nearest the pad's centre out to
the track's width, one radius past where the track leaves the pad or at the
track's far end when the track is shorter. The base reaches 0.9 of the way
to the pad's (or via's) inscribed edge on the nearer side, so a track that
enters off centre or at an angle keeps both sides on it. It is
a zone with `fill_type = "teardrop"` on the track's net and layer, solid
copper like a Solid Region. A track as wide as the pad gets none. A teardrop
that would come closer than the clearance to another net's copper, or meet a
keepout that forbids routing, is left out and counted in Messages. Adding
removes the old teardrops of the same nets first, so running it twice leaves
the same board. `remove_teardrops` removes them. From the menu both act on
the nets of the selection, or every net with nothing selected; with operands
on the net `net` names, or every net.

Teardrops are not tied to their track: moving or deleting the track leaves
the teardrop where it was until Teardrops runs again.

### Gloss

`gloss_selected_routes` shortens the selected tracks. Where exactly two
selected tracks of one net, layer and width meet at a corner that no other
track end, pad or via shares, the two become one straight track between
their far ends, when that track keeps the clearance to other nets' copper
and stays out of keepouts that forbid routing, and still touches every
copper of its net that touched either track (a branch ending mid-track, a
via, a pad, an area); otherwise the corner stays. It repeats until no corner
can go, trying corners in position order. The first track keeps its uuid
and takes the new end; the second is removed. Route ends never move, so
nothing a route connects comes loose. Nothing to cut is refused. It does not
read the interactive router and makes no 45° bends; it only removes corners.

### Board Shape from Selection

`board_shape_from_selection` makes the selected lines the board outline when
they close one loop end to end (ends join at the same nm point, in any order
and direction). The lines stay. Open lines, a branch and several loops are
refused, saying where the path opens. While the board sketch owns the outline
the row is refused: the outline is changed in Board Sketch, whose backdrop
shows the outline set here. Only tracks are lines on a board today; board
arcs and graphics lines cannot be selected yet.

### Keepout from Selection

`keepout_from_selection` turns a selected Fill, Solid Region or pour into a
keepout over its outline on its layer, keeping its uuid (its holes and net
go). With no area selected, selected lines on no net that close one loop on
one layer are replaced by a keepout over that loop; lines that carry a net
are routed copper and are refused. The layer rule is the Keepout
tool's: a copper layer keeps that copper clear, a keep-out layer its side's
outer copper; other layers are refused, and so is an area that already is a
keepout.

### Arrange in Box

`arrange_in_box` arms a box for the selected unlocked components; two clicks
on the board (on the non-components grid) set its opposite corners. The
components are packed inside it in rows from its top-left corner, tallest
first, each origin on the components grid counted from that grid's origin,
0.5 mm apart and from the box
edge, by their bounds as placed (turned and mirrored). Components that are
not selected are not avoided, even inside the box. A box too small is refused
and nothing moves. Esc drops the box. With operands `ids` names the
components and `start` and `end` the box.

## Pin and gate swap

Swap Pins and Swap Gates (#2410) trade connections inside one component. On
the board menu they are live with two pads of one unlocked component
selected: Swap Pins trades the nets of those two pads, Swap Gates trades the
gates they belong to. As commands with operands, `swap_pins document=<board>
designator=<ref> pins=<a>,<b>` and `swap_gates document=<board>
designator=<ref> units=<a>,<b>` do the same, are on the agent channel, and
answer with the pads whose nets changed (`pads`: pad, from, to) and the sheet
that followed.

Which pins and gates may trade is the part's own rule, read from two
parameters of its placed units: `Pin Swap Groups` lists groups of pin
numbers and `Gate Swap Groups` groups of unit numbers, groups split by `;`
and members by `,` (`1,2;5,6,7`: pins 1 and 2 trade, and any two of 5, 6 and
7). A part without the parameter allows no swap. The component's own Pin
Swapping (pins) or Part Swapping (gates) switch in its Component view must
be on as well.

The board and the schematic change as one undo step, the board step and the
sheet step sharing one undo group, as a footprint swap does (#2366):

- A pin swap trades the nets of every pad of each number
  (`signex_engine::pcb::pad_swap`) and moves what ends on one pin's tip on the
  sheet (wire ends, labels, junctions, no-connect marks) to the other pin's
  tip, and back (`signex_engine::part_swap::rewire_pins`).
- A gate swap pairs each pin of one gate with the pin of the other drawn at
  the same place, trades those pad nets, and trades the two placed units'
  unit numbers, so every wire stays where it is and each net moves to the
  other gate's pin.

It is refused, with nothing changed and the reason in Messages, for pins or
gates outside one swap group, a locked component, a pad whose net is locked,
a part with no schematic link or whose sheet is closed or read-only, a pin
tip that meets the middle of a wire or another pin, a rewired wire that would
then cross a pin or a wire end and join nets, gates that do not draw the same
pins in the same places, a part whose gates are on more than one sheet, and
for Swap Gates on two pads of one gate. Pads that already share their nets
change nothing and say so.

## Design reuse blocks

A reuse block (#2410) is a placed and routed group saved once and placed
again on another channel of the same circuit, with its tracks and vias.

- **Save**: Save as Reuse Block on the board menu (live with board objects
  selected) saves the selection's footprints, tracks and vias as a block named
  after its designators (`R1-U1`). `save_board_reuse_block document=<board>
  name=<name> ids=<list>` names the block and lists designators or footprint,
  track and via ids. Zones, drawings and texts are not carried, and every
  footprint must be linked to a schematic part. The block is re-based so its
  first designator sits at the origin.
- **Storage**: per project, one file per block,
  `<project folder>/reuse-blocks/<name>.snxpcb`, a board file holding the block
  alone, written off the update thread. A name is letters, digits, spaces,
  `-`, `_` and `.` inside, at most 64 characters, and not a name the system
  keeps for a device (`CON`, `NUL`, `COM1`, ... with or without a dot after
  it). Saving a name again replaces that block; a name that differs from a
  block's only in case is refused, since a case-blind file system would
  overwrite that block's file. The case check needs the project's blocks
  on disk, so a save before they are read reads them first (the menu saves
  once they land; with operands it answers once they land), and a save is
  refused when the folder cannot be listed. A folder that could not be
  listed is listed again on the next save or place, and when a board menu
  next opens. A save is refused while the project folder is being renamed,
  and a project folder rename waits while a block file is being written.
  One block file is written one save at a time: a save while its file is
  being written waits for that write to land, and a later save replaces a
  waiting one, so the file holds the last save. A waiting save whose project
  folder is being renamed, or was renamed, when its turn comes is not
  written, and Messages says so.
  If the file cannot be written the block can still be placed until Signex
  closes, and Messages says so.
- **Place**: the board menu lists one Place Reuse Block "<name>" row per
  block of the project, live with one unlocked component selected: the
  anchor. The project's blocks are read once, when a board menu first opens.
  The row closes the menu and acts on the board of the window it was in.
  `place_board_reuse_block document=<board> name=<name> designator=<ref>
  [degrees=<turn>]` does the same; while the project's blocks are not read
  yet it reads them first and answers when they land. Without operands from
  the palette or a key it does nothing and says what it needs.

Placing finds each block footprint's counterpart through the schematic: a
part of a sheet placed more than once keeps its unit ids in every placement,
so the counterpart is the footprint linked to the same units on the anchor's
sheet placement. The anchor keeps its place; the other counterparts are moved,
turned and put on the block's side around it, the whole turned by `degrees`,
and the block's tracks and vias are added with new ids on the nets their pads
reach (`signex_engine::pcb::reuse_block`). It is one undo step. It is refused
for an anchor that is not part of the block's circuit, a block footprint
with no counterpart on the anchor's channel, a locked counterpart, and a
block net whose pads reach two different board nets. Updating placed copies
when a block changes is not built yet.
