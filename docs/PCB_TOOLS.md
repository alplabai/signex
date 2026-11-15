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
