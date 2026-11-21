# The board counters of the status bar and their popovers (#2460).

## The counters in the bar
board-status-grid = Grid
board-status-grid-off = OFF
# { $value } is a grid pitch with its unit.
board-status-value = { $value }
board-status-routed = Routed
# { $routed } of { $total } connections are routed.
board-status-routed-count = { $routed }/{ $total }
board-status-routed-percent = { $percent }%
board-status-unrouted = Unrouted
board-status-drc = DRC
board-status-selected = Selected
# The number in a counter's badge.
board-status-count = { $count }
board-status-drc-just-now = · just now
board-status-drc-minutes-ago = · { $minutes } min ago
board-status-drc-not-run = · not run yet

## Popovers: shared
board-status-zoom = Zoom
# { $key } is a key or key sequence; { $next } and { $prev } too.
board-status-cycles = { $key } cycles
board-status-cycles-both = { $next } / { $prev } cycles

## Active layer popover
board-status-layer-title = Active layer
board-status-layers-panel = Layers panel

## The layer dropdown's group headings
board-status-layer-group-copper = Copper
board-status-layer-group-silkscreen = Silkscreen
board-status-layer-group-mask = Mask
board-status-layer-group-paste = Paste
board-status-layer-group-assembly = Assembly
board-status-layer-group-mechanical = Mechanical
board-status-layer-group-documentation = Documentation

## Grid popover
board-status-grid-title = Grid
# { $value } is a grid pitch in mm, without the unit.
board-status-grid-preset = { $value } mm
# { $mil } is the same pitch in mil.
board-status-grid-mil = ({ $mil } mil)
board-status-grid-custom = Custom…
board-status-grid-unit = mm
board-status-grid-custom-placeholder = 0.150
board-status-snap-to-grid = Snap to grid
board-status-grids = Grids…

## Unrouted connections popover
board-status-unrouted-title = Unrouted connections
board-status-net-connections = { $count ->
    [one] 1 connection
   *[other] { $count } connections
}
# A pad: { $designator } is its part, { $pad } its number.
board-status-pad = { $designator }-{ $pad }
board-status-pad-alone = { $pad }
# { $from } and { $to } are pads.
board-status-pair = { $from } → { $to }
board-status-pour-will-connect = · pour will connect
board-status-route-next = Route next
# { $key } is the key that arms Interactive Routing.
board-status-route-next-key = Route next ({ $key })
board-status-all-routed = Every connection is routed

## DRC popover
board-status-drc-title = DRC
# { $time } is a time of day, { $edits } the board's edits since.
board-status-drc-last-run = last run { $time } · { $edits ->
    [one] 1 edit since
   *[other] { $edits } edits since
}
board-status-drc-never = not run yet
board-status-drc-running = Checking the board…
board-status-drc-clean = No violations
# { $count } more violations are in the DRC panel.
board-status-drc-more = { $count } more in the DRC panel
# { $objects } how many objects, { $place } where and { $layer } the
# copper layer of a violation.
board-status-drc-detail = { $objects } · { $place }
board-status-drc-detail-layer = { $objects } · { $place } · { $layer }
board-status-run-drc = Run DRC
board-status-open-messages = Open Messages

## Selection popover
board-status-selection-title = Selection
board-status-selection-objects = { $count ->
    [one] 1 object
   *[other] { $count } objects
}
# { $value } and { $footprint } of a selected part.
board-status-part-caption = { $value } · { $footprint }
# { $count } more selected objects are not listed.
board-status-selection-more = { $count } more
board-status-properties = Properties
board-status-select-in-schematic = Select in schematic
board-status-clear = Clear
board-status-kind-component = Component
board-status-kind-body = 3D body
board-status-kind-keepout = Keepout
board-status-kind-track = Track
board-status-kind-arc = Arc
board-status-kind-pad = Pad
board-status-kind-via = Via
board-status-kind-region = Region
board-status-kind-polygon = Polygon pour
board-status-kind-fill = Fill
board-status-kind-text = Text
board-status-kind-room = Room
board-status-kind-other = Other object

## A footprint drag (Caner, 2026-11-21); { $key } is Ctrl, or Cmd on macOS.
board-status-drag-with-tracks = Dragging with tracks ({ $key }: leave the tracks)
board-status-drag-without-tracks = Moving without tracks (release { $key }: drag the tracks)
