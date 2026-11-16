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
# Highlight one layer: every other layer is drawn dimmed. $layer is its name.
pcb-layer-tab-highlight = Highlight { $layer }
# Flyouts of the layer tab menu: one row per kind of layer.
pcb-layer-tab-hide-layers = Hide layers
pcb-layer-tab-show-layers = Show layers
# A row of those flyouts. $kind is the kind's token.
pcb-layer-kind = { $kind ->
    [signal] Signal layers
    [plane] Plane layers
    [overlay] Overlay layers
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
# Group headers of the Layers panel.
pcb-layers-signal = Signal
pcb-layers-plane = Plane
pcb-layers-mechanical = Mechanical
pcb-layers-overlay = Overlay
pcb-layers-mask = Mask
pcb-layers-drill = Drill
# Buttons at the top of a Layers panel group: show or hide all its layers.
pcb-layers-group-show-all = Show all
pcb-layers-group-hide-all = Hide all
# Toggle button of the Layers panel: draw only the active layer.
pcb-layers-single-layer = Single layer
# Tooltip of that button.
pcb-layers-single-layer-tip = Draw only the active layer, with the board outline and the vias and through-hole pads that reach it
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
pcb-bar-pad-unavailable = Place Pad: not available on boards yet
pcb-bar-arc-unavailable = Place Arc: not available on boards yet
pcb-bar-text-unavailable = Place String: not available on boards yet
pcb-bar-dimension-unavailable = Place Dimension: not available on boards yet
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
# What the Messages panel says when a board placement tool is armed (#2373).
pcb-tool-hint-via = Via: click to place a via on the net under it; Esc or a right-click ends the tool
pcb-tool-hint-track = Track: click to start a run on the active copper layer, then click each corner; change the layer to add a via. Click the run's end, Esc or a right-click to end the run
pcb-tool-hint-fill = Fill: click two opposite corners of solid copper on the active copper layer; Esc or a right-click ends the tool
pcb-tool-hint-region = Solid Region: click each corner, then the first corner or a right-click to close it; Esc drops the shape, then ends the tool
pcb-tool-hint-keepout = Keepout: on a copper or keep-out layer, click each corner, then the first corner or a right-click to close it. It keeps all copper off that copper layer; Esc drops the shape, then ends the tool
pcb-tool-hint-pour = Polygon Pour: click each corner, then the first corner or a right-click to close it. It fills with the net under the first corner and keeps clear of other nets' copper; Esc drops the shape, then ends the tool
# A Track, Fill or Solid Region click while the active layer is not copper;
# $tool is the tool's name.
pcb-tool-needs-copper = { $tool }: make a copper layer active to draw on it
# A Fill or Solid Region closed with no area (its corners on one line).
pcb-tool-no-area = { $tool }: the shape has no area, so nothing was added
# A Keepout click while the active layer is neither copper nor a keep-out layer.
pcb-tool-keepout-needs-layer = Keepout: make a copper layer or a keep-out layer active to draw on it
# The text a string placed with the String tool reads until it is edited.
pcb-tool-string-text = String
# A String click while the active layer is not an overlay layer.
pcb-tool-string-needs-overlay = String: make an overlay layer active to place a string on it
# Interactive Routing (#2389): its Active Bar tooltip and Messages hint, the
# lines a leg it cannot route writes, and the mode `cycle_route_mode` picks.
pcb-bar-route-tooltip = Interactive Routing (Space flips the corner, Shift+Space changes the corner mode)
pcb-tool-hint-route = Interactive Routing: click a pad or track end to start on the active copper layer, then click each point; each leg keeps the rule clearance from other nets. Change the layer to add a via. Click the route's end, Esc or a right-click to finish it as one step
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
pcb-layer-silkscreen-top = Top Overlay
pcb-layer-silkscreen-bottom = Bottom Overlay
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
pcb-reannotate-intro = Numbers the board's parts by where they sit, from 1 within each prefix (R1, R2, … and C1, C2, …). Prefixes stay, and locked footprints keep their designators. Execute renames the parts on the board as one undo step; save the board to keep them.
# The walk order and its two choices.
pcb-reannotate-order = Order
pcb-reannotate-order-columns = By X, then by Y (left to right; equal X top to bottom)
pcb-reannotate-order-rows = By Y, then by X (top to bottom; equal Y left to right)
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

## The board 3D view (#2390).

# The 3D view's bar: its name, how to move it, and its buttons.
pcb-3d-title = 3D View
pcb-3d-hint = Drag to turn, right-drag to slide, wheel to zoom, click to select
# Back to the flat board.
pcb-3d-flat = 2D
pcb-3d-fit = Fit
pcb-3d-top = Top
pcb-3d-bottom = Bottom
pcb-3d-front = Front
pcb-3d-left = Left
pcb-3d-right = Right
# From above a front corner.
pcb-3d-iso = Angled
# Messages line when a part's model could not be read; the part is drawn as
# a plain body. $source names the library footprint.
pcb-3d-model-unread = The 3D model of { $source } could not be read: { $why }. It is drawn as a plain body.

## The board Tools edits (#2408, #2409).

# Messages line when a Tools edit ran; $tool is its menu name, the counts
# are the items it added, removed and changed.
pcb-board-tool-done = { $tool }: { $added } added, { $removed } removed, { $changed } changed
# Messages line when a copper tool left places out; $count of them.
pcb-board-tool-skipped = { $tool }: { $count } left out where the copper would break the clearance or a keepout
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
