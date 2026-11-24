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
# A pour with no fill; $why says why (pcb-pour-unfilled-*).
pcb-prim-pour-unfilled = not filled: { $why }

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

## #2523: the panel shell, Net Information, the Track, Arc and Drawing
## panels and their illustrations.
# A field whose selected items differ.
pcb-prim-mixed = Mixed
# A value that cannot be shown yet.
pcb-prim-no-value = —
pcb-prim-unit-mm = mm
pcb-prim-unit-degrees = °
# The header lock of a selection where only some items are locked.
pcb-prim-locked-count = { $locked } of { $total } locked
pcb-prim-lock-unsupported = Arcs, strings, dimensions and drawings cannot be locked yet; the board does not store a lock for them.
pcb-prim-unlock = Unlock
pcb-prim-shelved-pill = Shelved
# The header pill of one pad: its component's designator and the pad's.
pcb-prim-pad-pill = { $component }-{ $pad }
# The closed group of rows the board cannot store yet.
pcb-prim-unstored = { $count ->
    [one] Not stored yet · 1 row
   *[other] Not stored yet · { $count } rows
}
# A closed Net Information header: the net and its class.
pcb-prim-net-summary = { $net } · { $class }
# After Show More: what else is on the net.
pcb-prim-net-counts = · { $tracks ->
    [one] 1 track
   *[other] { $tracks } tracks
}, { $vias ->
    [one] 1 via
   *[other] { $vias } vias
}, { $pads ->
    [one] 1 pad
   *[other] { $pads } pads
} on net
pcb-prim-ends = Ends
pcb-prim-ends-round = Round
pcb-prim-ends-square = Square
pcb-prim-ends-flat = Flat
pcb-prim-ends-copper-tip = Copper keeps round ends until square and flat ends are decided for copper (DRC and Gerber).
pcb-prim-ends-unstored-tip = Not stored yet: the board stores no end cap for a drawing, so its ends are round.
# Point rows; their two fields are marked X and Y.
pcb-prim-point-start = Start
pcb-prim-point-end = End
pcb-prim-centre = Centre
# An arc's start and end angle on one row.
pcb-prim-angles = Angles
## Labels in the illustrations.
# $value is already formatted.
pcb-prim-drawing-width-value = Width { $value }
pcb-prim-drawing-height = Height
# $layer is the layer's name.
pcb-prim-drawing-layer-ends = { $layer } · round ends
# An inner copper layer named by its place in the stack: L1 is the top.
pcb-prim-drawing-inner-layer = L{ $number }
# The net of the track about to be placed.
pcb-prim-drawing-next = { $net } · next
pcb-prim-drawing-cursor = cursor

## Via (mockup row 96): the via type a via picks, protection after IPC-4761.
## The types are derived from the board's vias until the Layer Stack
## Manager stores them.
pcb-prim-via-type = Via Type
pcb-prim-via-type-tip = The board's via types, read from its vias until the Layer Stack stores them. Picking one sets the span, sizes and tenting in one step.
# A Via Type entry: its name ("Thru 1:4") and its spec.
pcb-prim-via-type-entry = { $name } · { $spec }
# "Through 1:4 · 0.6/0.3 · Type I-b"; $class is "Type I-b", "open" or "Custom".
pcb-prim-via-type-spec = { $kind } { $from }:{ $to } · { $diameter }/{ $drill } · { $class }
pcb-prim-via-kind-through = Through
pcb-prim-via-kind-blind = Blind
pcb-prim-via-kind-micro = Micro
# A drill pair over the copper layers, counted from the top: "Through 1–4".
pcb-prim-via-drill-pair = { $kind } { $from }–{ $to }
pcb-prim-edit-in-layer-stack = Edit in Layer Stack…
pcb-prim-define-via-types = Define blind/buried via types in Layer Stack…
pcb-prim-layer-stack-later = Via types will be edited in the Layer Stack Manager, which is not available yet.
# A small tag after a value that comes from somewhere else.
pcb-prim-from-type = from type
pcb-prim-from-drill = from drill
pcb-prim-span = Span
# The layers a via joins: "Top Layer → Bottom Layer".
pcb-prim-span-ends = { $from } → { $to }
pcb-prim-drill-pair = Drill Pair
pcb-prim-position = Position
pcb-prim-via-locked = This via is locked. Click the lock to edit it.
pcb-prim-unlock-via = Unlock
pcb-prim-override-note = Span and drill pair stay with the type; only Diameter and Drill change for this via.
pcb-prim-drill = Drill
pcb-prim-reset-to-type = Reset to the via type's value
pcb-prim-finished-hole = Finished Hole
pcb-prim-finished-hole-tip = The drill less twice the hole plating.
pcb-prim-microvia = Microvia
pcb-prim-microvia-stacked = Stacked
pcb-prim-microvia-staggered = Staggered
pcb-prim-microvia-unstored = Not stored yet: the board does not store whether a microvia is stacked or staggered.
pcb-prim-min-drill-warn = Drill { $drill } is below the board's minimum via drill { $min }.
pcb-prim-protection = Protection
pcb-prim-protection-type = Type
pcb-prim-protection-type-tip = Picks a via protection type and sets Fill, Cap and Tented in one step. Set by the via type here.
# An IPC-4761 type in a via type's spec: $code is "I-b", "VII".
pcb-prim-ipc-code = Type { $code }
pcb-prim-ipc-short-open = open
pcb-prim-ipc-open = None · open
pcb-prim-ipc-ia = Type I-a · tented one side
pcb-prim-ipc-ib = Type I-b · tented both sides
pcb-prim-ipc-ii = Type II · tented & covered (second coat not stored)
pcb-prim-ipc-iiia = Type III-a · plugged one side
pcb-prim-ipc-iiib = Type III-b · plugged both sides
pcb-prim-ipc-iva = Type IV-a · plugged & covered one side
pcb-prim-ipc-ivb = Type IV-b · plugged & covered both sides
pcb-prim-ipc-v = Type V · filled
pcb-prim-ipc-via = Type VI-a · filled & covered one side
pcb-prim-ipc-vib = Type VI-b · filled & covered both sides
pcb-prim-ipc-vii = Type VII · filled & capped
pcb-prim-ipc-custom = Custom
pcb-prim-fill = Fill
pcb-prim-fill-plug = Plug
pcb-prim-fill-resin = Resin
pcb-prim-fill-conductive = Cond.
pcb-prim-fill-cap-unstored = The board does not store a via's fill or cap yet.
pcb-prim-plug-from = Plug From
pcb-prim-both = Both
pcb-prim-cap = Cap
pcb-prim-capped = Capped
pcb-prim-no-mask-top = Top: this via does not reach Top Layer, so the side has no mask.
pcb-prim-no-mask-bottom = Bottom: this via does not reach Bottom Layer, so the side has no mask.
pcb-prim-protection-from-type = Protection comes from the via type.
pcb-prim-edit-via-types = Edit it in Layer Stack ▸ Via Types.
pcb-prim-testpoint-summary = Fabrication · Assembly

## Mask rows of tracks and vias (mockup rows 95 and 96).
pcb-prim-source = Source
pcb-prim-tenting-moved = Tented Top/Bottom moved to Protection above; a tented side has no opening.
pcb-prim-track-tent-unstored = Tracks do not store tenting yet.

## Via and pad illustrations (mockup rows 96 and 97).
pcb-prim-top-view = Top view
pcb-prim-side-view = Side view
# A copper layer counted from the top: L1 is Top, the last is Bottom.
pcb-prim-layer-position = L{ $n }
# The cross-section's header: the drill pair and the protection type.
pcb-prim-via-section-header = { $pair } · { $class }
pcb-prim-fill-summary = Fill: { $fill }
pcb-prim-fill-capped-summary = Fill: { $fill } · capped

## Pad (mockup row 97): the copper table, the hole and the mask switches.
pcb-prim-all-layers = All layers
pcb-prim-shape-all-layers-tip = The board stores one shape per pad: a shape picked in any row sets every layer.
pcb-prim-hole = Hole
pcb-prim-plated = Plated
pcb-prim-plated-tip = A plated hole connects the copper on every layer. Plating follows the pad type, which the footprint sets.
pcb-prim-solder-tented = Solder mask: Tented

## #2523: the pour, string and dimension panels and their illustrations.
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
# The caption under a dimension's Layer row.
pcb-prim-dim-layer-note = Any board layer, copper included. On copper a dimension is a drawing only: no copper, DRC or Gerber.
# m7/pcb-props-via-zone
# The closed Via Type field's sizes; $diameter and $drill are in mm.
pcb-prim-via-type-sizes = { $diameter }/{ $drill }
