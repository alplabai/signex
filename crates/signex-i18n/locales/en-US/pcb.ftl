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
