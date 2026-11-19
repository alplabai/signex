# Properties panel while tracks, vias or pads are selected on a board (#2355).

## Header: the kind, how many are selected, and a field that filters the rows.
pcb-prim-track-title = Track
pcb-prim-via-title = Via
pcb-prim-pad-title = Pad
pcb-prim-track-count = { $count ->
    [one] 1 Track
   *[other] { $count } Tracks
}
pcb-prim-via-count = { $count ->
    [one] 1 Via
   *[other] { $count } Vias
}
pcb-prim-pad-count = { $count ->
    [one] 1 Pad
   *[other] { $count } Pads
}
pcb-prim-search = Search

## Section headings.
pcb-prim-net-information = Net Information
pcb-prim-location = Location
pcb-prim-properties = Properties
pcb-prim-definition = Definition
pcb-prim-via-stack = Via Stack
pcb-prim-pad-stack = Pad Stack
pcb-prim-paste-mask = Paste Mask Expansion
pcb-prim-solder-mask = Solder Mask Expansion
pcb-prim-via-types = Via Types & Features
pcb-prim-testpoint = Testpoint

## Net Information.
pcb-prim-net = Net
pcb-prim-class = Class
pcb-prim-delay = Delay
pcb-prim-length = Length
# The two columns of the Delay and Length rows.
pcb-prim-selected = Selected
pcb-prim-total = Total
pcb-prim-show-more = Show More
pcb-prim-show-less = Show Less
# Show More rows: how much of the net is on the board.
pcb-prim-net-tracks = Tracks on net
pcb-prim-net-vias = Vias on net
pcb-prim-net-pads = Pads on net
pcb-prim-no-net = No Net
pcb-prim-no-class = No Class
pcb-prim-not-applicable = N/A
# A length in millimetres; $value is already formatted.
pcb-prim-mm = { $value } mm
# A rotation in degrees; $value is already formatted.
pcb-prim-degrees = { $value }°
pcb-prim-net-link-tip = Select every track, via and pad of this net
pcb-prim-class-link-tip = Select every net of this class on the board
pcb-prim-delay-unknown = The delay needs the dielectric of the board's stackup, which the board does not record yet.

## Shared rows.
pcb-prim-x = X
pcb-prim-y = Y
pcb-prim-lock = Lock
pcb-prim-lock-tip = A locked track or via stays where it is: move, delete and edits are refused until it is unlocked.
pcb-prim-item-locked = This track or via is locked, so it cannot be changed. Clear Lock to edit it.
pcb-prim-lock-nothing = Select a track or via to lock or unlock; a footprint is locked in its Component properties.
pcb-prim-pad-lock-tip = A pad is locked with its footprint.
pcb-prim-area-lock-unsupported = Fills, regions, pours and keepouts cannot be locked yet; the board does not store a lock for them.
pcb-prim-layer = Layer
pcb-prim-width = Width
pcb-prim-start = Start (X/Y)
pcb-prim-end = End (X/Y)
pcb-prim-pick-net = Pick a net from the board
pcb-prim-picking-net = Click copper on the board to take its net. Esc cancels.
pcb-prim-rule = Rule
pcb-prim-manual = Manual
pcb-prim-expansion = Expansion
pcb-prim-mask-unsupported = The board does not store a mask expansion per pad yet.
pcb-prim-top = Top
pcb-prim-bottom = Bottom
pcb-prim-fabrication = Fabrication
pcb-prim-assembly = Assembly
pcb-prim-testpoint-unsupported = The board does not store testpoint flags yet.
pcb-prim-show-preview = Show preview
pcb-prim-preview-unsupported = A stack preview is not available yet.
pcb-prim-stack-simple = Simple
pcb-prim-stack-top-middle-bottom = Top-Middle-Bottom
pcb-prim-stack-full = Full Stack
pcb-prim-stack-tip = Simple is one size on every layer; Top-Middle-Bottom and Full Stack size each copper layer.
pcb-prim-middle = Middle
pcb-prim-template = Template
pcb-prim-library = Library
pcb-prim-library-local = <Local>
pcb-prim-template-unsupported = Templates are not supported yet.
pcb-prim-propagation-delay = Propagation Delay
pcb-prim-read-only = This board opened read-only (a newer format), so it cannot be edited.
pcb-prim-enter-hint = Press Enter to apply a typed value.

## Via.
pcb-prim-via-name = Name
# A via's span over the copper layers, counted from the top: "Thru 1:4".
pcb-prim-via-span-through = Thru { $from }:{ $to }
pcb-prim-via-span-blind = Blind { $from }:{ $to }
pcb-prim-via-span-micro = Micro { $from }:{ $to }
pcb-prim-span-from = From
pcb-prim-span-to = To
pcb-prim-diameter = Diameter
pcb-prim-thermal-relief = Thermal Relief
pcb-prim-direct = Direct
pcb-prim-relief-unsupported = The board does not store a thermal relief per item yet.
pcb-prim-hole-size = Hole Size
pcb-prim-tolerance = Tolerance
pcb-prim-tolerance-unsupported = The board does not store hole tolerances yet.
pcb-prim-tented = Tented
pcb-prim-from-hole-edge = From Hole Edge
pcb-prim-ipc-via-type = IPC 4761 Via Type
pcb-prim-none = None
pcb-prim-via-type-unsupported = The board does not store an IPC 4761 via type yet.

## Pad.
pcb-prim-designator = Designator
pcb-prim-electrical-type = Electrical Type
pcb-prim-electrical-load = Load
pcb-prim-electrical-source = Source
pcb-prim-electrical-terminator = Terminator
pcb-prim-electrical-unsupported = The board does not store a pad's electrical type yet.
pcb-prim-pin-package-length = Pin Package Length
pcb-prim-pin-length-unsupported = The board does not store a pin package length yet.
pcb-prim-jumper = Jumper
pcb-prim-jumper-unsupported = The board does not store jumper ids yet.
pcb-prim-rotation = Rotation
pcb-prim-pad-layer-unsupported = Moving a pad to another layer is not supported here yet.
pcb-prim-copper = Copper
pcb-prim-paste = Paste
pcb-prim-solder = Solder
pcb-prim-x-size = X-Size
pcb-prim-y-size = Y-Size
pcb-prim-shape = Shape
pcb-prim-relief = Relief
pcb-prim-percent = %
pcb-prim-enabled = Enabled
pcb-prim-shape-round = Round
pcb-prim-shape-rect = Rectangle
pcb-prim-shape-roundrect = Rounded Rectangle
pcb-prim-shape-oval = Oval
pcb-prim-shape-trapezoid = Trapezoid
pcb-prim-shape-custom = Custom
pcb-prim-no-hole = No hole
pcb-prim-pad-locked = The footprint of this pad is locked, so the pad cannot be changed.
pcb-prim-tented-tip = Mask covers the via on this side: no opening.
pcb-prim-from-hole-edge-tip = Measure the expansion from the hole edge instead of the via copper.

## Messages panel notices.
# $what is the field's name; $why the reason.
pcb-prim-refused = { $what } was not changed: { $why }
pcb-prim-not-a-number = "{ $text }" is not a length or angle.
pcb-prim-board-gone = The board is no longer open, so { $what } was not changed.
pcb-prim-no-net-there = There is no netted copper there; the net was not changed.
# Length tuning (#2389): the track field that takes a target length for the
# track's net, and why it refuses a selection of more than one track.
pcb-prim-tune-length = Tune Net To
pcb-prim-tune-one-track = select one track to tune
pcb-prim-not-positive = the value must be above zero

## While a placement tool is armed (#2393): the object about to be placed.
pcb-prim-placing = Next to place
pcb-prim-placing-hint = Press Tab to pause placing and change these values for the next placements.
pcb-prim-placing-paused = Placing is paused. Enter or Tab goes on placing with these values; Esc puts the previous values back.

## Areas (#2393): fills, solid regions, polygon pours and keepouts.
pcb-prim-fill-title = Fill
pcb-prim-region-title = Solid Region
pcb-prim-pour-title = Polygon Pour
pcb-prim-keepout-title = Keepout
pcb-prim-area-title = Areas
pcb-prim-area-count = { $count ->
    [one] 1 Area
   *[other] { $count } Areas
}
pcb-prim-pour-settings = Pour
# Clearance 0 follows the board's clearance.
pcb-prim-clearance = Clearance
pcb-prim-min-thickness = Minimum Width
pcb-prim-keepout-kinds = Keeps Out
pcb-prim-keepout-all-copper = All copper
pcb-prim-keepout-kinds-unsupported = A board keepout keeps out all copper; other kinds are not stored yet.
# A pour's fill rules (#2216): a higher priority fills first and the others
# keep clear of it; islands below the area (mm²) are dropped, and with Remove
# Dead Copper those touching no copper of the pour's net.
pcb-prim-priority = Priority
pcb-prim-min-island-area = Remove Islands Below (mm²)
pcb-prim-remove-dead-islands = Remove Dead Copper
# What the pour's fill came to: its copper area and pieces.
pcb-prim-pour-copper = Copper
pcb-prim-pour-copper-value = { $area } mm² in { $pieces ->
    [one] 1 piece
   *[other] { $pieces } pieces
}
pcb-prim-pour-copper-tip = Computed from the board; Repour fills it again.

## Free arcs, strings and dimensions (#2373). A free pad uses the pad rows.
pcb-prim-arc-title = Arc
pcb-prim-arc-count = { $count ->
    [one] 1 Arc
   *[other] { $count } Arcs
}
pcb-prim-text-title = String
pcb-prim-text-count = { $count ->
    [one] 1 String
   *[other] { $count } Strings
}
pcb-prim-dimension-title = Dimension
pcb-prim-dimension-count = { $count ->
    [one] 1 Dimension
   *[other] { $count } Dimensions
}
pcb-prim-center = Centre (X/Y)
pcb-prim-radius = Radius
# Angles are read from the board origin with Y up; the arc runs
# counter-clockwise from the start angle to the end angle.
pcb-prim-start-angle = Start Angle
pcb-prim-end-angle = End Angle
pcb-prim-arc-mode = Placement
pcb-prim-arc-mode-tip = Cycle Arc Mode (the command palette) switches how the next arc is drawn.
pcb-prim-text = Text
pcb-prim-text-height = Text Height
pcb-prim-line-width = Line Width
pcb-prim-offset = Offset
pcb-prim-precision = Precision
pcb-prim-value = Value
pcb-prim-not-a-precision = the precision is a whole number of decimals from 0 to 6

## w2/zone-text-dim (#2523): the pour, string and dimension panels and
## their illustrations.
# The unit after an area field.
pcb-prim-unit-mm2 = mm²
pcb-prim-pour-fill-mode = Fill Mode
pcb-prim-pour-fill-solid = Solid
pcb-prim-pour-fill-hatched = Hatched
pcb-prim-pour-hatched-tip = Hatched fill is not supported yet.
pcb-prim-pour-thermal = Thermal Relief
pcb-prim-pour-thermal-tip = Pours join the pads of their net solidly; thermal relief spokes are not drawn yet.
pcb-prim-pour-spoke-gap = Spoke / Gap
pcb-prim-pour-corners = Corners
pcb-prim-pour-corner-sharp = Sharp
pcb-prim-pour-corner-rounded = Rounded
pcb-prim-pour-corner-chamfer = Chamfer
pcb-prim-pour-corner-size = Corner Size
pcb-prim-pour-corners-tip = Not stored yet: a pour's copper keeps sharp corners.
# The label of the island rule; the field says "below" and the unit is mm².
pcb-prim-pour-remove-islands = Remove Islands
pcb-prim-pour-below = below
pcb-prim-pour-shelved-value = shelved · no copper
pcb-prim-pour-repour = Repour
pcb-prim-pour-repour-shelved-tip = Unshelve the pour before repouring it
pcb-prim-pour-shelve = Shelve
pcb-prim-pour-shelve-tip = Keep the outline, show no copper
pcb-prim-pour-unshelve = Unshelve
# The button that opens the menu of board-wide pour commands.
pcb-prim-pour-menu = ⋯
pcb-prim-pour-menu-tip = All pours on the board
pcb-prim-pour-repour-all = Repour all
pcb-prim-pour-shelve-all = Shelve all
pcb-prim-pour-unshelve-all = Unshelve all
pcb-prim-pour-shelve-note = Shelve is one undo step and is saved with the board. Fields stay editable while shelved.
## Labels in the pour illustration.
pcb-prim-pour-drawing-spoke = Spoke Width
pcb-prim-pour-drawing-gap = Air Gap
pcb-prim-pour-drawing-direct = direct connect
pcb-prim-pour-drawing-other-net = other net
pcb-prim-pour-drawing-shelved = Shelved · outline kept, no copper
pcb-prim-pour-drawing-keepout = Keepout · all copper
# $net is the pour's net name, $layer its layer's name.
pcb-prim-pour-drawing-note = { $net } · { $layer } · solid
# A string's location; its two fields are marked X and Y.
pcb-prim-string-position = Position
pcb-prim-string-quick-rotation = Quick rotation
# A quick rotation step; $degrees is a whole number.
pcb-prim-string-rotation-step = { $degrees }°
pcb-prim-string-drawing-origin = Position X/Y
pcb-prim-dim-style = Style
# A decimals step of the Precision control.
pcb-prim-dim-precision-step = { $decimals }
# $layer is the layer's name, $height the text height in mm.
pcb-prim-dim-drawing-note = { $layer } · text { $height }
