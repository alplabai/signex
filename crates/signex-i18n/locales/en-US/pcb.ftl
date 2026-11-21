# The board view: the Layers panel and the layer shown in the status bar.

# Title of the Layers panel.
pcb-layers-title = Layers
# Shown in the Layers panel while neither a board nor a footprint is in view.
pcb-layers-empty = Open a board or a footprint to see its layers.
# Button that shows every hidden layer again.
pcb-layers-show-all = Show all layers
# Right-click menu of a layer tab under the board: hide that layer, or show
# it and hide every other layer. $layer is the layer's name.
pcb-layer-tab-hide = Hide { $layer }
pcb-layer-tab-show-only = Show only this layer
# The layer's colour, saved with the board: opens the Layers panel's
# colour picker for the layer; greyed with the reason for a layer whose
# colour the board cannot change yet.
pcb-layer-tab-colour = Layer colour...
pcb-layer-tab-colour-unavailable = This layer's colour cannot be changed yet.
# Give the layer, or every layer, the board palette colour again.
pcb-layer-tab-colour-reset = Reset layer colour
pcb-layer-tab-colour-reset-all = Reset all layer colours
# Edit ▸ Origin ▸ Set waits for a click on the board.
pcb-origin-set-hint = Origin: click on the board to set the origin there. Esc cancels
# Edit ▸ Origin ▸ Snap to Outline Corner before the pointer was on the board.
pcb-origin-no-pointer = Origin: point at the board first; the origin snaps to the outline corner nearest the pointer
# Highlight one layer: every other layer is drawn dimmed. $layer is its name.
pcb-layer-tab-highlight = Highlight { $layer }
# Flyouts of the layer tab menu: one row per kind of layer.
pcb-layer-tab-hide-layers = Hide layers
pcb-layer-tab-show-layers = Show layers
# A row of those flyouts. $kind is the kind's token.
pcb-layer-kind = { $kind ->
    [signal] Signal layers
    [plane] Plane layers
    [overlay] Silkscreen layers
    [mask] Mask layers
    [paste] Paste layers
    [mechanical] Mechanical layers
   *[all] All layers
}
# Rows that open board editors; greyed with the reason while the editor is
# not built.
# Opens the layer stack manager (#2401).
pcb-layer-tab-layer-stack = Layer stack...
pcb-layer-tab-drill-symbols = Drill symbols...
pcb-layer-tab-drill-symbols-unavailable = Drill symbols cannot be set yet.
pcb-layer-tab-drawing-order = Layer drawing order...
pcb-layer-tab-drawing-order-unavailable = The layer drawing order cannot be changed yet.
# How the layer tabs name their layers, and their numbers.
pcb-layer-tab-names-short = Short tab names
pcb-layer-tab-names-medium = Medium tab names
pcb-layer-tab-names-long = Long tab names
pcb-layer-tab-show-numbers = Show layer numbers
# View the board from below: mirrored, bottom layers on top.
pcb-layer-tab-flip = Flip board view
# A layer tab with its number in front. $number is the layer's number,
# $name its name.
pcb-layer-tab-numbered = { $number } { $name }
# Short layer tab names. $layer is the layer's name in the board file.
pcb-layer-short = { $layer ->
    [TopCopper] TL
    [BottomCopper] BL
    [EdgeCuts] OL
    [Margin] MG
    [TopSilkscreen] TO
    [BottomSilkscreen] BO
    [TopSolderMask] TS
    [BottomSolderMask] BS
    [TopPaste] TP
    [BottomPaste] BP
    [TopAdhesive] TA
    [BottomAdhesive] BA
    [TopFab] TF
    [BottomFab] BF
    [TopAssembly] TAS
    [BottomAssembly] BAS
    [TopCourtyard] TC
    [BottomCourtyard] BC
    [Top3DBody] T3D
    [Bottom3DBody] B3D
    [TopKeepout] TK
    [BottomKeepout] BK
    [TopFiducial] TFD
    [BottomFiducial] BFD
    [TopDesignator] TD
    [BottomDesignator] BD
    [Dimensions] DIM
    [Notes] NT
    [DrillDrawing] DD
    [Eco1] E1
    [Eco2] E2
   *[other] { $layer }
}
pcb-layer-short-inner = M{ $index }
pcb-layer-short-mechanical = Mech{ $index }
# Medium layer tab names of copper and mechanical layers; every other layer
# keeps its long name.
pcb-layer-medium-top = Top
pcb-layer-medium-bottom = Bottom
pcb-layer-medium-inner = Mid { $index }
pcb-layer-medium-mechanical = Mech { $index }
# The layer set button at the left end of the layer tabs, and its menu.
pcb-layer-set-tooltip = Layer set: show a set of layers
# The button's label. $set is the set's token.
pcb-layer-set-short = { $set ->
    [signal] Signal
    [plane] Plane
    [non_signal] Non-signal
    [mechanical] Mechanical
   *[all] All
}
# A built-in set's row in the menu. $set is the set's token.
pcb-layer-set = { $set ->
    [signal] Signal Layers
    [plane] Plane Layers
    [non_signal] Non-signal Layers
    [mechanical] Mechanical Layers
   *[all] All Layers
}
pcb-layer-set-save = Save current as set...
pcb-layer-set-manage = Manage sets...
pcb-layer-set-none-saved = No saved sets yet.
pcb-layer-set-name-placeholder = Set name
pcb-layer-set-save-button = Save
pcb-layer-set-delete = Delete
# Status bar pill with the active layer; the layer's name follows it.
pcb-status-layer = Layer:
# Status bar readout of how far the board view is turned on screen; the
# angle in degrees follows it.
pcb-status-view-angle = View:
# Tooltip of that readout.
pcb-status-view-angle-tip = Board view rotation. Click to type an angle.
# Placeholder of the angle field the readout opens.
pcb-status-view-angle-placeholder = Degrees
# Tooltip of the PCB Active Bar button that turns the view a quarter turn.
pcb-view-rotate-tooltip = Rotate View 90° Clockwise
# Tooltip of the PCB Active Bar's Move button (#2352).
pcb-bar-move-tooltip = Move (right-click for the move and transform menu)
# Tooltip of the PCB Active Bar's Selection Filter button (#2373).
pcb-bar-filter-tooltip = Selection Filter (click for the object kinds a click or box picks)
# Tooltip of the PCB Active Bar's Align button (#2373).
pcb-bar-align-tooltip = Align (click for the align and distribute menu)
# Tooltips of the PCB Active Bar's greyed buttons: tools the board editor
# cannot run yet (#2352).
pcb-bar-component-unavailable = Place Component: not available on boards yet
# Rows of the Place menu on a board (#2360). The ones the board editor cannot
# run yet are greyed, with the tooltip above as the reason.
pcb-tool-component = Component
pcb-tool-pad = Pad
pcb-tool-via = Via
pcb-tool-route = Interactive Routing
pcb-tool-track = Track
pcb-tool-arc = Arc
pcb-tool-fill = Fill
pcb-tool-region = Solid Region
pcb-tool-polygon-pour = Polygon Pour
pcb-tool-string = String
pcb-tool-dimension = Dimension
pcb-tool-keepout = Keepout
# Tooltips of the PCB Active Bar's live placement tools (#2373).
pcb-bar-via-tooltip = Place Via
pcb-bar-track-tooltip = Place Track (Space flips the corner, Shift+Space changes the corner mode)
pcb-bar-fill-tooltip = Place Fill
pcb-bar-region-tooltip = Place Solid Region (right-click to close)
pcb-bar-keepout-tooltip = Place Keepout (right-click to close)
pcb-bar-pour-tooltip = Place Polygon Pour (right-click to close)
pcb-bar-pad-tooltip = Place Pad
pcb-bar-arc-tooltip = Place Arc (by centre, by edge or any angle: Cycle Arc Mode in the palette)
pcb-bar-string-tooltip = Place String
pcb-bar-dimension-tooltip = Place Dimension
# What the Messages panel says when a board placement tool is armed (#2373).
pcb-tool-hint-via = Via: click to place a via on the net under it; Esc or a right-click ends the tool
pcb-tool-hint-track = Track: click to start a run on the active copper layer (a pad not on it starts with a via), then click each corner; change the layer to add a via. Click the run's end, Esc or a right-click to end the run
pcb-tool-hint-fill = Fill: click two opposite corners of solid copper on the active copper layer; Esc or a right-click ends the tool
pcb-tool-hint-region = Solid Region: click each corner, then the first corner, a double-click or a right-click to close it; Esc drops the shape, then ends the tool
pcb-tool-hint-keepout = Keepout: on a copper or keep-out layer, click each corner, then the first corner, a double-click or a right-click to close it. It keeps all copper off that copper layer; Esc drops the shape, then ends the tool
pcb-tool-hint-pour = Polygon Pour: click each corner, then the first corner, a double-click or a right-click to close it. It fills with the net set in Properties (Tab), else the net under its first corner on copper, and keeps clear of other nets' copper; Esc drops the shape, then ends the tool
# A Track, Fill or Solid Region click while the active layer is not copper;
# $tool is the tool's name.
pcb-tool-needs-copper = { $tool }: make a copper layer active to draw on it
# A Fill or Solid Region closed with no area (its corners on one line).
pcb-tool-no-area = { $tool }: the shape has no area, so nothing was added
# A Keepout click while the active layer is neither copper nor a keep-out layer.
pcb-tool-keepout-needs-layer = Keepout: make a copper layer or a keep-out layer active to draw on it
# The text a string placed with the String tool reads until it is edited.
pcb-tool-string-text = String
# A String click while the active layer takes no text.
pcb-tool-string-needs-text-layer = String: make a silkscreen, mechanical or documentation layer active to place a string on it
# The free pad, arc, string and dimension tools (#2373): what the Messages
# panel says when one is armed, and when a click adds nothing.
pcb-tool-hint-pad = Pad: click to place a through-hole pad on the net under it; Tab edits its shape, size and hole. Esc or a right-click ends the tool
pcb-tool-hint-arc = Arc: { $mode }. Esc drops the arc being drawn, then ends the tool
pcb-tool-hint-string = String: click to place a string on the active silkscreen, mechanical or documentation layer; Tab edits its text and height. Esc or a right-click ends the tool
pcb-tool-hint-dimension = Dimension: click the two points to measure, then where the dimension line goes. Esc drops the dimension being drawn, then ends the tool
pcb-tool-no-arc = Arc: those points make no arc, so nothing was added
pcb-tool-no-dimension = Dimension: its two points are the same, so nothing was added
# How the Arc tool's clicks make an arc; `cycle_arc_mode` steps through them.
pcb-arc-mode-centre = click the centre, the start, then where the arc ends (it runs counter-clockwise)
pcb-arc-mode-edge = click the two ends of a quarter circle
pcb-arc-mode-any = click the start, the end, then a point the arc bends through
# The short name of each Arc mode, as Properties shows it while placing.
pcb-arc-mode-centre-name = By Centre
pcb-arc-mode-edge-name = By Edge
pcb-arc-mode-any-name = Any Angle
# Pour Cutout and Slice Pour (#2216): their Place rows, Active Bar tooltips,
# Messages hints, and the line said when nothing was cut; $tool is the
# tool's name.
pcb-tool-pour-cutout = Pour Cutout
pcb-tool-slice-pour = Slice Pour
pcb-bar-pour-cutout-tooltip = Place Pour Cutout (right-click to close)
pcb-tool-hint-pour-cutout = Pour Cutout: click each corner, then the first corner, a double-click or a right-click to close it. The area is cut out of every polygon pour it overlaps on the active copper layer, as one step; Esc drops the shape, then ends the tool
pcb-bar-slice-pour-tooltip = Slice Pour
pcb-tool-hint-slice-pour = Slice Pour: click two points on a line across a polygon pour on the active copper layer; each pour the line meets is cut in two along it, as one step. Esc drops the first point, then ends the tool
pcb-tool-pour-missed = { $tool }: no polygon pour on the active copper layer is cut there, so nothing changed
# Repour Selected and Repour All (#2216): what the fill came to.
pcb-pour-repoured = Repour: { $pours ->
    [one] 1 polygon pour
   *[other] { $pours } polygon pours
} filled, { $pieces } areas of copper, { $islands } islands removed
pcb-pour-none = Repour: the board has no polygon pour
pcb-pour-none-selected = Repour Selected: select a polygon pour first
# Interactive Routing (#2389): its Active Bar tooltip and Messages hint, the
# lines a leg it cannot route writes, and the mode `cycle_route_mode` picks.
pcb-bar-route-tooltip = Interactive Routing (Space flips the corner, Shift+Space changes the corner mode)
pcb-tool-hint-route = Interactive Routing: click a pad or track end to start on the active copper layer (a pad not on it starts with a via), then click each point; each leg keeps the rule clearance from other nets. Change the layer to add a via. Click the route's end, Esc or a right-click to finish it as one step
pcb-route-blocked = Interactive Routing: no way to that point keeps the clearance the rules ask, so nothing was added
pcb-route-via-blocked = Interactive Routing: a via there would not keep the clearance the rules ask, so nothing was added
pcb-route-unrepresentable = Interactive Routing: a point or the track width is outside what the board geometry can hold
# $why says what is wrong with the rules (the setup clearance, a net class).
pcb-route-no-rules = Interactive Routing: the board's rules are not usable: { $why }
pcb-route-mode-walkaround = Interactive Routing: walk around other nets' copper
pcb-route-mode-shove = Interactive Routing: push other nets' tracks aside, walk around the rest
pcb-route-board-changed = Interactive Routing: a track the route pushed was changed or removed since, so nothing was added
# The board changed under a route being drawn (an undo, an edit elsewhere)
# and the route no longer fits it; $why says how.
pcb-route-stale = Interactive Routing: the board changed and the route no longer fits it ({ $why }), so nothing was added; Esc discards the route
pcb-route-not-placed = Interactive Routing: the board changed and the route no longer fits it ({ $why }), so nothing was placed; undo or redo back to place it, or press Esc again to discard it
pcb-route-discarded = Interactive Routing: the route no longer fits the board ({ $why }) and was discarded; the board is unchanged
pcb-route-too-sharp = Routing: a corner turns too sharply for the tracks to follow it side by side, so nothing was added
# The routing tools that follow Interactive Routing (#2389): Place rows,
# Active Bar tooltips and Messages hints.
pcb-tool-diff-pair = Differential Pair Routing
pcb-tool-quick-route = Quick Route
pcb-tool-multi-route = Multi-Route
pcb-bar-diff-pair-tooltip = Differential Pair Routing (Space flips the corner, Shift+Space changes the corner mode)
pcb-bar-quick-route-tooltip = Quick Route (straight to the point, other nets are not avoided)
pcb-bar-multi-route-tooltip = Multi-Route (select copper of the nets first)
pcb-tool-hint-diff-pair = Differential Pair Routing: click copper of a pair's net to start both tracks there, then click each point; the pair keeps its coupled width and gap and walks around other nets. Click the route's end, Esc or a right-click to finish it as one step
pcb-tool-hint-quick-route = Quick Route: click a pad or track end to start, then click the target; the track goes straight there at the rule width, without avoiding other nets, as one step
pcb-tool-hint-multi-route = Multi-Route: select copper of the nets to route, click near their ends to start, then click each point; the nets run side by side around other nets. Click the route's end, Esc or a right-click to finish it as one step
# Why a routing tool did nothing: $tool is its name, $why one of the
# reasons below ($net is a net's name).
pcb-route-refused = { $tool }: { $why }
pcb-route-no-net = click on copper of a net to start
pcb-route-no-pair = { $net } is in no differential pair; name its nets with _P and _N (or + and -), or give their net class a pair gap
pcb-route-member-no-copper = { $net } has no copper on the active layer to start from
pcb-route-bus-needs-nets = select copper of two or more nets first
# A leg aimed at a pad of another net; $pad names the pad (U1.3),
# $pad_net its net, $net the route's net.
pcb-route-end-other-net = { $pad } is on net { $pad_net }, the route is on net { $net }
# A pad of no net a route ended on took the route's net; Update PCB lists
# it as a net set on the board.
pcb-route-pad-joined = { $pad } joined net { $net }
pcb-route-join-gone = a pad the route joins is gone, or its net is no longer in the board's net table
# A Polygon Pour placed with no net; $tool is its name.
pcb-pour-no-net = { $tool } has no net; set it in Properties
# A pour the fill left empty; $pour names it, $why is one of the reasons
# below.
pcb-pour-not-filled = { $pour }: not filled: { $why }
pcb-pour-unfilled-no-layer = it is on no copper layer
pcb-pour-unfilled-outline = its outline has fewer than three corners the board geometry can hold
pcb-pour-unfilled-clearance = its clearance is not a length the board geometry can hold
# The leg on the pointer found no way; the straight leg drawn shows where
# it was aimed.
pcb-route-aim-blocked = the preview found no way to the pointer that keeps the clearance the rules ask; a click searches further and says if there is none
# The status bar while Tab paused a board tool; $tool is its name.
pcb-tool-paused = { $tool }: paused, edit in Properties; Enter or Tab goes on placing, Esc goes on with the old values
# Length tuning from the PCB Active Bar's Tune group, the palette or an
# agent (#2389). $what is the command's name, $why the reason.
pcb-tune-needs-tracks = { $what }: select a track of each net to tune
pcb-tune-refused = { $what }: { $why }
pcb-tune-done = { $what }: { $count } net(s) brought to { $length } mm
# A net a multi-net tuning left as it was; $net its name, $why the reason.
pcb-tune-skipped = { $what }: { $net } left as it is: { $why }

# Layer names in the Layers panel, the layer strip and the status bar, as
# Altium names them. A mechanical layer the user named shows that name instead.
pcb-layer-top-copper = Top Layer
pcb-layer-bottom-copper = Bottom Layer
# An inner copper layer; $index counts from the top, starting at 1.
pcb-layer-inner-copper = Mid-Layer { $index }
# A mechanical layer with no name of its own; $index is its number, 1 to 32.
pcb-layer-mechanical = Mechanical { $index }
# The board edge: the outline and the rims of its openings.
pcb-layer-board-outline = Board Outline
pcb-layer-margin = Margin
pcb-layer-silkscreen-top = Top Silkscreen
pcb-layer-silkscreen-bottom = Bottom Silkscreen
pcb-layer-solder-mask-top = Top Solder
pcb-layer-solder-mask-bottom = Bottom Solder
pcb-layer-paste-top = Top Paste
pcb-layer-paste-bottom = Bottom Paste
pcb-layer-adhesive-top = Top Adhesive
pcb-layer-adhesive-bottom = Bottom Adhesive
pcb-layer-fab-top = Top Fab
pcb-layer-fab-bottom = Bottom Fab
pcb-layer-assembly-top = Top Assembly
pcb-layer-assembly-bottom = Bottom Assembly
pcb-layer-courtyard-top = Top Courtyard
pcb-layer-courtyard-bottom = Bottom Courtyard
pcb-layer-body-3d-top = Top 3D Body
pcb-layer-body-3d-bottom = Bottom 3D Body
pcb-layer-keepout-top = Top Keep-Out Layer
pcb-layer-keepout-bottom = Bottom Keep-Out Layer
pcb-layer-fiducial-top = Top Fiducial
pcb-layer-fiducial-bottom = Bottom Fiducial
pcb-layer-designator-top = Top Designator
pcb-layer-designator-bottom = Bottom Designator
pcb-layer-dimensions = Dimensions
pcb-layer-notes = Notes
pcb-layer-drill-drawing = Drill Drawing
pcb-layer-eco-1 = ECO 1
pcb-layer-eco-2 = ECO 2
# Vias and through-hole pads.
pcb-layer-multi-layer = Multi-Layer
# Tooltip of a layer's eye in the Layers panel.
pcb-layers-hide = Hide layer
pcb-layers-show = Show layer

# Properties panel of a board with nothing selected. The title is followed by
# the board's name.
pcb-properties-title = Board
# Section header over the board's rows.
pcb-properties-section = Properties
pcb-properties-size = Size
# Outline extent; $width and $height are millimetre numbers.
pcb-properties-size-value = { $width } × { $height } mm
# Shown as the size when the board has no outline yet.
pcb-properties-no-outline = No outline
pcb-properties-thickness = Thickness
# Properties panel while board items are selected; no per-item view exists yet.
# $board is the board document's name.
pcb-properties-selection = { $count ->
    [one] 1 item selected on { $board }
   *[other] { $count } items selected on { $board }
}
pcb-properties-selection-hint = Press Esc to clear the selection and see the board's properties.

# Properties panel while one footprint is selected on a board (#1312).
pcb-footprint-designator = Designator
pcb-footprint-value = Value
# Shown as the footprint name when the footprint has none.
pcb-footprint-no-name = Not named
pcb-footprint-locked = This footprint is locked, so its designator and value cannot be changed.
pcb-footprint-read-only = This board opened read-only (a newer format), so it cannot be edited.
# $designator is the footprint's designator; $count the other footprints with it.
pcb-footprint-duplicate = { $count ->
    [one] { $designator } is also on 1 other footprint.
   *[other] { $designator } is also on { $count } other footprints.
}
# Under the fields while typed text is not applied yet.
pcb-footprint-enter-hint = Press Enter to apply the change.

# Tools > Re-Annotate Board (#1312): the Positional Re-Annotate dialog.
pcb-reannotate-title = Positional Re-Annotate
# Under the title. $board is the board document's name.
pcb-reannotate-subtitle = { $board }
pcb-reannotate-intro = Numbers the board's parts by where they sit, from 1 within each prefix. Prefixes stay; locked footprints keep their designators.
# The walk order and its two choices (each card's hint is below them).
pcb-reannotate-order = Order
pcb-reannotate-order-columns = By X, then by Y
pcb-reannotate-order-rows = By Y, then by X
# Where bottom-side parts fall in the numbering.
pcb-reannotate-bottom = Bottom side
pcb-reannotate-bottom-after = After the top side
pcb-reannotate-bottom-with = With the top side
# Column headings of the rename list.
pcb-reannotate-col-from = Designator
pcb-reannotate-col-to = New designator
# $renamed parts get a new designator, $unchanged already have theirs, $kept are left alone.
pcb-reannotate-summary = { $renamed ->
    [one] 1 part renamed
   *[other] { $renamed } parts renamed
}, { $unchanged } already in order, { $kept } kept.
pcb-reannotate-nothing = Every part is already numbered in this order.
# $designator is the kept footprint's designator.
pcb-reannotate-kept-locked = { $designator } is locked and keeps its designator.
pcb-reannotate-kept-no-prefix = { $designator } is not a prefix and a number, so it is kept.
pcb-reannotate-kept-empty = A footprint with no designator is kept.
pcb-reannotate-read-only = This board opened read-only (a newer format), so it cannot be re-annotated.
pcb-reannotate-stale = The board changed, so the list was made again. Check it, then Execute again.
# $why is the reason the board gave.
pcb-reannotate-refused = Nothing was changed: { $why }
pcb-reannotate-cancel = Cancel
pcb-reannotate-execute = Execute
# Under the Designator field and in the Re-Annotate dialog: the board's
# designators are not carried to the schematic by Update PCB.
pcb-designator-update-pcb-note = Update PCB renames parts back to the schematic's designators; use Back Annotate to keep them.
# $designator is the kept footprint's designator.
pcb-reannotate-kept-unlinked = { $designator } is not linked to a schematic part, so it keeps its designator.
# Messages panel lines. $what is the field (Designator or Value), $why the reason.
pcb-footprint-refused = { $what }: { $why }; nothing changed
pcb-footprint-board-gone = { $what }: the board is no longer open
# $designator is the new designator, $count the other footprints with it.
pcb-footprint-shared-notice = Designator: { $designator } is also on { $count ->
    [one] 1 other footprint
   *[other] { $count } other footprints
} of this board
pcb-reannotate-no-pcb = Re-Annotate: no PCB is in view
pcb-reannotate-board-gone = Re-Annotate: the board is no longer open
# $count renamed footprints on the board file $board.
pcb-reannotate-done = Re-Annotate: { $count ->
    [one] 1 footprint renamed
   *[other] { $count } footprints renamed
} on { $board }; save the board to keep them.

# A library part placed on a board from the Components panel (#2364).
# Messages line when its footprint is on the cursor; $part is the component's
# part number, $footprint the footprint's name.
pcb-place-part-hint = Place { $part } ({ $footprint }): click to place, Space to turn, Y to flip, Esc to stop
# Messages line when the part cannot go on a board; $reason says why.
pcb-place-part-refused = { $part } cannot be placed on a board: { $reason }. Nothing was placed.
# Messages line when the board was closed or switched away from while the
# part was being read.
pcb-place-part-board-gone = The board { $board } is no longer active; { $part } was not placed.

## The board 3D view (#2390) and its toolbar (mockup row 175).

# The toolbar's title.
pcb-3d-title = 3D View
# The quiet caption at the view's bottom right: how to move it.
pcb-3d-hint = Drag to turn, right-drag to slide, wheel to zoom, click to select
# The toolbar's 2D | 3D switch: back to the flat board, or the 3D view.
pcb-3d-flat = 2D
pcb-3d-solid = 3D
# Fit icon button tooltip.
pcb-3d-fit = Fit
# Viewpoints; each also names the view in the status pill. Bottom is
# reached by Flip.
pcb-3d-top = Top
pcb-3d-bottom = Bottom
pcb-3d-front = Front
pcb-3d-left = Left
pcb-3d-right = Right
pcb-3d-back = Back
# From above a front corner.
pcb-3d-iso = Iso
pcb-3d-reset = Reset camera
# Flip button: its label while the top shows, and while the bottom shows.
pcb-3d-flip = Flip
pcb-3d-flipped = Bottom
pcb-3d-flip-tooltip = Flip board
pcb-3d-flip-back-tooltip = Flip back to the top
# Explode button and its popover.
pcb-3d-explode = Explode
pcb-3d-explode-amount = Explode { $percent } %
pcb-3d-explode-close = Close Explode
pcb-3d-explode-readout = { $percent } %
pcb-3d-explode-assembled = Assembled
pcb-3d-explode-apart = Layers apart
pcb-3d-explode-layers = Separate copper and mask
pcb-3d-explode-parts = Lift components
# Lighting presets; each also picks the view's backdrop.
pcb-3d-lighting-studio = Studio
pcb-3d-lighting-workshop = Workshop
pcb-3d-lighting-outdoor = Outdoor
pcb-3d-lighting-dark = Dark
pcb-3d-lighting-tooltip = Next lighting preset
# The lighting toast after the preset's name: where it is in the cycle.
pcb-3d-lighting-position = · { $index } of { $count }
# Screenshot button and its menu.
pcb-3d-shot-tooltip = Copy a screenshot
pcb-3d-shot-copy = Copy to clipboard
pcb-3d-shot-save = Save PNG…
pcb-3d-shot-save-title = Save a Screenshot of the 3D View
pcb-3d-shot-filter = PNG image
# Messages lines about a screenshot.
pcb-3d-shot-copied = A screenshot of the 3D view was copied to the clipboard.
pcb-3d-shot-copy-failed = The screenshot of the 3D view could not be copied to the clipboard: { $why }
# Why the clipboard refused the screenshot ($why above).
pcb-3d-shot-clipboard-unavailable = no clipboard is available
pcb-3d-shot-clipboard-occupied = another program holds the clipboard
pcb-3d-shot-clipboard-format = the clipboard does not take images
pcb-3d-shot-clipboard-conversion = the image could not be converted for the clipboard
pcb-3d-shot-saved = A screenshot of the 3D view was saved to { $path }.
pcb-3d-shot-save-failed = The screenshot of the 3D view could not be saved to { $path }: { $why }
pcb-3d-shot-empty = The 3D view could not be captured: the window had nothing to show.
# View cube faces, top side up and turned over.
pcb-3d-cube-top = TOP
pcb-3d-cube-bottom = BOTTOM
pcb-3d-cube-left = LEFT
pcb-3d-cube-right = RIGHT
pcb-3d-cube-front = FRONT
# Status pill at the bottom left. $preset is a viewpoint name; $width,
# $height and $thickness are millimetres; $bodies counts the parts.
pcb-3d-status-view = { $preset } · { $width } × { $height } × { $thickness } mm · { $bodies ->
        [one] 1 body
       *[other] { $bodies } bodies
    }
pcb-3d-status-size = { $width } × { $height } × { $thickness } mm · { $bodies ->
        [one] 1 body
       *[other] { $bodies } bodies
    }
# $apart is all, layers, parts or none: what the explode moves apart.
pcb-3d-status-explode = Explode { $percent } % · { $apart ->
        [all] core, copper, mask and parts apart
        [layers] core, copper and mask apart
        [parts] parts lifted off the board
       *[none] nothing set apart
    }
# While flipped: the parts on the bottom side, and the parts with
# through-hole pins, as lists of designators.
pcb-3d-status-bottom = Bottom side · { $parts }
pcb-3d-status-bottom-tht = Bottom side · { $parts } · through-hole pins of { $tht }
pcb-3d-status-no-parts = no parts
# A designator list cut short: the first few, and how many more.
pcb-3d-status-list-more = { $list } and { $count } more
# Messages line when a part's model could not be read; the part is drawn as
# a plain body. $source names the library footprint. It also shows as the
# view's warn toast.
pcb-3d-model-unread = The 3D model of { $source } could not be read: { $why }. It is drawn as a plain body.
# Under the toast when more parts could not be read.
pcb-3d-model-unread-more = { $count ->
        [one] One more part could not be read; Messages lists it.
       *[other] { $count } more parts could not be read; Messages lists them.
    }
pcb-3d-dismiss = Dismiss

## The board Tools edits (#2408, #2409).

# Messages line when a Tools edit ran; $tool is its menu name, the counts
# are the items it added, removed and changed.
pcb-board-tool-done = { $tool }: { $added } added, { $removed } removed, { $changed } changed
# Messages line when a copper tool left places or locked tracks out; $count of them.
pcb-board-tool-skipped = { $tool }: { $count } left out, locked or where the copper would break the clearance or a keepout
# Messages line when a Tools edit did nothing; $reason says why.
pcb-board-tool-refused = { $tool }: { $reason }; nothing changed
# Messages line when Arrange in Box is waiting for its corners.
pcb-arrange-box-hint = Arrange in Box: click two opposite corners of the box for the selected components; Esc ends it

## DXF import (#2410).
# The undo label of an import.
pcb-dxf-import-step = Import DXF
# Messages line. $count drawings, $file the file name, $layer the layer.
pcb-dxf-imported-layer = Imported { $count } drawings from { $file } onto { $layer }.
pcb-dxf-imported-outline = The board outline is now the { $count }-corner shape from { $file }.
# $count entities of kinds the import does not read (text, blocks, hatches).
pcb-dxf-skipped = { $count } entities of { $file } are not lines, arcs, circles or polylines and were skipped.
pcb-dxf-refused = { $file } was not imported: { $why }
# Before the file dialog opens; $why says the layer and why it cannot take a drawing.
pcb-dxf-layer-refused = No DXF file was asked for: { $why }

## Pin and gate swap (#2410). Messages lines; $why is the reason from the
## board or the schematic, in English.
pcb-part-swap-refused = { $what } did nothing: { $why }
# A swap command's refusal from the board or the schematic, as its answer
# gives it; the line above already said it in Messages.
pcb-part-swap-why = { $why }
pcb-pins-swapped = { $designator }: pins { $a } and { $b } traded nets on the board and on its schematic
pcb-gates-swapped = { $designator }: gates { $a } and { $b } traded places on the board and on its schematic
pcb-part-swap-nothing = { $designator }: pins { $a } and { $b } already share their nets; nothing changed
pcb-gates-swap-nothing = { $designator }: gates { $a } and { $b } already share their nets; nothing changed

## Design reuse blocks (#2410).
pcb-reuse-block-saved = Reuse block { $name } saved to { $file }: { $footprints } footprints, { $tracks } tracks, { $vias } vias
pcb-reuse-block-not-written = Reuse block { $name } could not be written to { $file }: { $why }. It can be placed until Signex closes.
pcb-reuse-block-placed = Reuse block { $name } placed on the channel of { $anchor }: { $footprints } footprints, { $tracks } tracks, { $vias } vias
pcb-reuse-block-refused = { $what } did nothing: { $why }
pcb-reuse-blocks-unread = The project's reuse blocks could not be read: { $why }
# A board menu row: place the block called $name around the selected part.
pcb-reuse-block-place-row = Place Reuse Block "{ $name }"
# Positional Re-Annotate, Graphite layout (#2533): the hints under the
# Order cards, the preview heading and its toggle, the arrow between the
# designators, the footer note and Execute with the rename count.
pcb-reannotate-order-columns-hint = Left to right; equal X top to bottom
pcb-reannotate-order-rows-hint = Top to bottom; equal Y left to right
pcb-reannotate-preview = Preview
pcb-reannotate-show-in-order = Show already in order
pcb-reannotate-arrow = →
pcb-reannotate-footer-note = One undo step; save the board to keep it.
pcb-reannotate-execute-count = Execute { $count }
