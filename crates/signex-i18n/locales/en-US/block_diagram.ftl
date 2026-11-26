## The block diagram tab (mockup row 171, #2796): the toolbar, the canvas
## footer, Properties, and why an edit, a save or a block_* command did
## not run. $file is a file name, $id an object id as the file holds it.

block-diagram-tab-kind = BLK
block-diagram-not-open = This block diagram is no longer open.
block-diagram-tool-select = Select
block-diagram-tool-connect = Connect
block-diagram-add-block = Add Block
block-diagram-add-block-tip = Add a block right of the others.
block-diagram-add-port = Add Port
block-diagram-add-port-tip = Add a port on the left edge of the selected block.
block-diagram-group = Group
block-diagram-group-tip = Put the selected blocks in a new group.
block-diagram-delete = Delete
block-diagram-delete-tip = Delete what is selected. A block takes its ports and connections with it.
block-diagram-undo = Undo
block-diagram-undo-tip = Undo the last change to this diagram.
block-diagram-redo = Redo
block-diagram-redo-tip = Redo the change undone last.
block-diagram-save = Save
block-diagram-save-tip = Save this diagram.
block-diagram-fit = Fit
block-diagram-fit-tip = Fit the whole diagram in view.
block-diagram-needs-one-block = Select one block first.
block-diagram-needs-blocks = Select one or more blocks first.
block-diagram-needs-selection = Select something first.
block-diagram-nothing-to-undo = Nothing to undo in this diagram.
block-diagram-nothing-to-redo = Nothing to redo in this diagram.
block-diagram-nothing-to-save = This diagram has no unsaved changes.
block-diagram-read-only = Read-only
block-diagram-read-only-tip = This diagram is from a newer Signex: it opens read-only.
block-diagram-connect-pick-end = Pick the block or port to connect to.
block-diagram-footer = { $blocks ->
    [one] 1 block
   *[other] { $blocks } blocks
} · { $connections ->
    [one] 1 connection
   *[other] { $connections } connections
}
block-diagram-footer-note = Double-click a block to open its sheet. Moving blocks only changes this file.
block-diagram-footer-read-only = Read only: this file is from a newer Signex and is never changed.
## The default names of what the toolbar adds; the person renames them.
block-diagram-new-block = Block { $number }
block-diagram-new-port = Port { $number }
block-diagram-new-group = Group { $number }

# block-diagram-properties
block-diagram-properties = Properties
block-diagram-prop-nothing = Select a block, a port, a connection or a group to see its properties.
block-diagram-prop-several = { $count } objects selected.
block-diagram-prop-name = Name
block-diagram-prop-function = Function
block-diagram-prop-note = Note
block-diagram-prop-shape = Shape
block-diagram-prop-x = X (mm)
block-diagram-prop-y = Y (mm)
block-diagram-prop-width = Width (mm)
block-diagram-prop-height = Height (mm)
block-diagram-prop-group = Group
block-diagram-prop-rails = Rails
block-diagram-prop-specs = Key specs
block-diagram-prop-color = Color
block-diagram-prop-sheet = Sheet
block-diagram-prop-sheet-state = Link
block-diagram-prop-ports = Ports
block-diagram-prop-no-ports = No ports yet.
block-diagram-prop-block = Block
block-diagram-prop-blocks = Blocks
block-diagram-prop-side = Side
block-diagram-prop-direction = Direction
block-diagram-prop-from = From
block-diagram-prop-to = To
block-diagram-prop-interface = Interface
block-diagram-prop-interface-name = Interface name
block-diagram-prop-label = Label
block-diagram-prop-derived = Made from
block-diagram-port-line = { $name } ({ $direction })
block-diagram-group-blocks = { $count ->
    [one] 1 block
   *[other] { $count } blocks
}
block-diagram-group-none = No group
block-diagram-sheet-none = Not linked to a sheet
block-diagram-sheet-linked = Linked
block-diagram-sheet-unlinked = The sheet left the project
block-diagram-shape-rectangle = Rectangle
block-diagram-shape-rounded = Rounded rectangle
block-diagram-shape-ellipse = Ellipse
block-diagram-shape-hexagon = Hexagon
block-diagram-side-left = Left
block-diagram-side-right = Right
block-diagram-side-top = Top
block-diagram-side-bottom = Bottom
block-diagram-direction-input = in
block-diagram-direction-output = out
block-diagram-direction-both = in and out
block-diagram-interface-signal = Signal
block-diagram-interface-bus = Bus
block-diagram-interface-power = Power
block-diagram-interface-analog = Analog
block-diagram-interface-spi = SPI
block-diagram-interface-i2c = I2C
block-diagram-interface-uart = UART
block-diagram-interface-usb = USB
block-diagram-interface-lvds = LVDS
block-diagram-interface-other = Other…

# block-diagram-files
block-diagram-open-failed = { $file } could not be opened as a block diagram.
block-diagram-save-failed = { $file } was not saved.
block-diagram-save-exists = { $file } was not saved: a file of that name is already in the folder, and a new diagram does not replace it. Close this diagram and open that file, or rename that file first.

# block-diagram-refused
block-diagram-refused-no-document = Name the block diagram with the operand `document`.
block-diagram-refused-not-a-diagram = { $name } is a { $kind }, not a block diagram.
block-diagram-refused-not-open = { $file } is not an open block diagram; open it first.
block-diagram-refused-read-only = { $file } is from a newer Signex ({ $token }) and opens read-only.
block-diagram-refused-saving = { $file } is already being saved.
block-diagram-refused-no-project = Open a project first: a new block diagram goes in its folder.
block-diagram-refused-already-open = { $file } is already open.
block-diagram-refused-bad-name = `{ $name }` is not a file name for a block diagram.
block-diagram-refused-needs = This command needs the operand `{ $operand }`.
block-diagram-refused-bad-id = `{ $operand }` takes an id, not `{ $text }`.
block-diagram-refused-pin-point = `{ $operand }` takes a point in mm, not a pin.
block-diagram-refused-bad-shape = `{ $text }` is not a shape: use rectangle, rounded_rectangle, ellipse or hexagon.
block-diagram-refused-bad-side = `{ $text }` is not a side: use left, right, top or bottom.
block-diagram-refused-bad-direction = `{ $text }` is not a direction: use input, output or bidirectional.
block-diagram-refused-bad-clear = `clear` cannot clear `{ $text }`.
block-diagram-refused-bad-spec = `{ $text }` is not a key spec: write name=value.
block-diagram-refused-bad-length = `{ $text }` is not a length in mm.
block-diagram-refused-no-end = { $text } is neither a block nor a port of this diagram.
block-diagram-no-block = The diagram has no block { $id }.
block-diagram-no-port = The diagram has no port { $id }.
block-diagram-no-connection = The diagram has no connection { $id }.
block-diagram-no-group = The diagram has no group { $id }.
block-diagram-fault-duplicate-id = The id { $id } would be used twice.
block-diagram-fault-block-name = A block needs a name ({ $id }).
block-diagram-fault-block-size = A block's width and height must be numbers above zero ({ $id }).
block-diagram-fault-block-position = A block's position must be a number ({ $id }).
block-diagram-fault-missing-group = Block { $id } names group { $other }, which the diagram does not have.
block-diagram-fault-sheet-path = Block { $id } links to a sheet with no path.
block-diagram-fault-port-name = A port needs a name ({ $id }).
block-diagram-fault-port-offset = A port's offset must be a number at or above zero ({ $id }).
block-diagram-fault-group-name = A group needs a name ({ $id }).
block-diagram-fault-missing-block = Connection { $id } ends on block { $other }, which the diagram does not have.
block-diagram-fault-missing-port = Connection { $id } ends on port { $other }, which its block does not have.
block-diagram-fault-closed = A connection cannot start and end at the same place ({ $id }).
block-diagram-fault-interface = A connection's interface needs a name ({ $id }).

# block-diagram-files, continued. $file is the diagram's file name, $reason
# the reader's own words.
block-diagram-read-for-command-failed = { $file } could not be read as a block diagram: { $reason }
block-diagram-save-changed = { $file } was not saved: the file changed on disk since it was opened or saved. Close the diagram and open it again to see that version; your edits stay here until then.
block-diagram-refused-unwritable = { $file } was not saved: { $reason }

# block-diagram-views: the view switch and Re-sync (mockup row 171).
block-diagram-view-blocks = Block diagram
block-diagram-view-power = Power tree
block-diagram-resync = Re-sync
block-diagram-resync-tip = Rebuild blocks and connections from the sheet hierarchy. Your layout is kept, and the change is one undo step.

# block-diagram-power: the Power tree view (mockup row 171, #1313). Amounts
# come pre-formatted: $amps, $used and $limit in A, $volts in V, $share and
# $tolerance in percent.
block-diagram-power-reading = Reading the power tree…
block-diagram-power-unread = The power tree could not be read: { $reason }
block-diagram-power-empty = No power rails yet. Name the power nets (VIN_12V, VOUT_3V3) or give parts a `Rail Load <net>` parameter.
block-diagram-power-legend = A source's bar is the rail's load against its limit: green under 50 %, amber to 90 %, red above. A load's bar is its share of the rail: accent under 50 %, amber to 90 %, red above. Click a part to open its sheet.
block-diagram-power-arrow = →
block-diagram-power-source = Source
block-diagram-power-converter = Converter
block-diagram-power-no-source = No source
block-diagram-power-of-limit = { $used } A of { $limit } A · { $share } %
block-diagram-power-voltage = { $volts } V
block-diagram-power-voltage-tol = { $volts } V ±{ $tolerance } %
block-diagram-power-no-voltage = Voltage not known
block-diagram-power-total = { $amps } A max
block-diagram-power-loads = { $count ->
        [one] 1 load
       *[other] { $count } loads
    }
block-diagram-power-draw = { $amps } A
block-diagram-power-share = { $share } % of the rail
block-diagram-power-unknown-draw = Load not known
block-diagram-power-rails = Rails
block-diagram-power-rail = Rail
block-diagram-power-nominal = Nominal
block-diagram-power-limit = Limit
block-diagram-power-load = Load
block-diagram-power-headroom = Headroom
block-diagram-power-free = { $share } % free
block-diagram-power-not-known = —
block-diagram-power-warning = Note: { $text }
block-diagram-power-footer = { $rails } rails · { $converters } converters
block-diagram-power-footer-note = Read from the schematic for the variant in the Variant field. Switch views to read it again.
# The totals line. $active of $sources sources feed a load in the scenario;
# $rail has the least headroom, $headroom percent of its limit.
block-diagram-power-scenario = Scenario Max load · sources { $active } of { $sources } active · worst rail headroom { $headroom } % ({ $rail })
block-diagram-power-scenario-no-limit = Scenario Max load · sources { $active } of { $sources } active · no rail has a limit
block-diagram-variant = Variant
block-diagram-show-gnd = Show GND

# block-diagram-review: the Re-sync review side sheet (mockup row 210). It
# opens when a block would become unlinked; the added and renamed or moved
# blocks are applied already.
block-diagram-review-title = Re-sync from sheets
block-diagram-review-close-mark = ✕
block-diagram-review-compared = Compared with the sheet hierarchy of the project.
block-diagram-review-summary = { $added } added · { $moved } renamed or moved · { $unlinked } unlinked · { $unchanged } unchanged
block-diagram-review-applied-note = The added and the renamed or moved blocks are applied; Undo takes them back.
block-diagram-review-unlinked-group = Unlinked { $count }
block-diagram-review-unlink-on = Sheet deleted · the block stays with its layout and note, marked unlinked · its connections from the sheets are dropped
block-diagram-review-unlink-off = Off: the block stays as it is, linked to a sheet that no longer exists. The next Re-sync lists it again.
block-diagram-review-manual = Manual blocks are not part of Re-sync and stay as drawn.
block-diagram-review-selected = { $checked } of { $total } selected · one undo step
block-diagram-review-apply = Apply selected
block-diagram-review-nothing = Nothing to change
block-diagram-review-nothing-note = The diagram matches the sheet hierarchy. Unlinked and manual blocks are left as they are.
block-diagram-review-close = Close
block-diagram-review-applied = { $file }: { $blocks } blocks unlinked from the sheets, layout kept. Undo with Ctrl+Z.
