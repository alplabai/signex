# The board view: the Layers panel and the layer shown in the status bar.

# Title of the Layers panel.
pcb-layers-title = Layers
# Shown in the Layers panel while no board is open.
pcb-layers-empty = Open a PCB to see its layers.
# Button that shows every hidden layer again.
pcb-layers-show-all = Show all layers
# Group headers of the Layers panel.
pcb-layers-copper = Copper
pcb-layers-technical = Technical
pcb-layers-mechanical = Mechanical
# Status bar pill with the active copper layer; the layer's name follows it.
pcb-status-layer = Layer:

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
pcb-properties-layers = Layers
pcb-properties-layers-value = { $count ->
    [one] 1 copper layer
   *[other] { $count } copper layers
}
pcb-properties-thickness = Thickness
# $mm is a millimetre number.
pcb-properties-thickness-value = { $mm } mm
pcb-properties-nets = Nets
# Properties panel while board items are selected; no per-item view exists yet.
# $board is the board document's name.
pcb-properties-selection = { $count ->
    [one] 1 item selected on { $board }
   *[other] { $count } items selected on { $board }
}
pcb-properties-selection-hint = Press Esc to clear the selection and see the board's properties.

# Properties panel while one footprint is selected on a board (#1312). The
# title is followed by the footprint's designator.
pcb-footprint-title = Footprint
pcb-footprint-designator = Designator
pcb-footprint-value = Value
# The library footprint the board footprint was made from.
pcb-footprint-name = Footprint
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
pcb-reannotate-order-columns = By X, then Y: columns left to right, each top to bottom
pcb-reannotate-order-rows = By Y, then X: rows top to bottom, each left to right
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
