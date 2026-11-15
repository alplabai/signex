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
pcb-prim-lock-unsupported = Tracks and vias cannot be locked yet; the board does not store a lock for them.
pcb-prim-pad-lock-tip = A pad is locked with its footprint.
pcb-prim-layer = Layer
pcb-prim-width = Width
pcb-prim-start = Start (X/Y)
pcb-prim-end = End (X/Y)
pcb-prim-pick-net = Pick a net from the board
pcb-prim-picking-net = Click copper on the board to take its net. Esc cancels.
pcb-prim-rule = Rule
pcb-prim-manual = Manual
pcb-prim-expansion = Expansion
pcb-prim-mask-unsupported = The board does not store a mask expansion per item or a mask rule yet.
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
pcb-prim-stack-unsupported = The board stores one size for every layer; per-layer stacks are not supported yet.
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
pcb-prim-via-span-unsupported = Changing a via's span is not supported here yet.
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
pcb-prim-no-hole = No hole
pcb-prim-pad-locked = The footprint of this pad is locked, so the pad cannot be changed.

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
