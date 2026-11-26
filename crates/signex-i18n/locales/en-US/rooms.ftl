# Rooms on a board (#2457): Place ▸ Room, Room Properties, Create Rooms
# from Sheets and the DRC kind for a part outside its room.

## The design rule
rooms-drc-kind = Part outside its room
# { $measured } and { $margin } are mm.
rooms-drc-detail = { $measured } mm outside its room, margin { $margin } mm

## Place ▸ Room
rooms-tool = Room
rooms-bar-tooltip = Place Room (two corners)
rooms-tool-hint = Room: click two corners to draw a room, then edit it in Properties; Esc drops the first corner, then ends the tool
# The name a drawn room takes, before its number: "Room 1".
rooms-default-name = Room

## Undo labels and refusals
rooms-undo-place = Place room
rooms-undo-edit = Edit room
rooms-undo-move = Move room
rooms-undo-fit = Fit room to parts
rooms-undo-pull = Pull parts into room
rooms-undo-create = Create rooms from sheets
rooms-refused = Room not changed: { $why }
rooms-nothing-to-pull = Every part of room { $room } is already inside it.
rooms-number-invalid = Enter a length in mm.

## Room Properties
rooms-props-title = Room
rooms-props-lock = Lock room
rooms-props-unlock = Unlock room
rooms-section-room = Room
rooms-field-name = Name
rooms-field-sheet = From sheet
rooms-sheet-none = None
rooms-field-layer = Layer
rooms-layer-top = Top
rooms-layer-bottom = Bottom
rooms-layer-both = Both
# The illustration's layer callout for a room on both sides.
rooms-layer-top-bottom = Top + Bottom
rooms-field-shape = Shape
rooms-shape-rectangle = Rectangle
rooms-shape-polygon = Polygon
rooms-field-colour = Color
rooms-section-position = Position
rooms-field-xy = X / Y
rooms-field-size = Width / Height
rooms-unit-mm = mm
rooms-callout-width = Width
rooms-callout-height = Height
rooms-callout-x = X
rooms-callout-y = Y
rooms-callout-margin = Margin
rooms-section-parts = Parts
rooms-parts-inside = { $count } inside
rooms-parts-outside = { $count } outside
# A part chip of a part outside its room: "R2 · outside".
rooms-part-outside = { $part } · outside
rooms-pull = Pull parts into room
rooms-fit = Fit room to parts
rooms-section-rule = Rule
rooms-keep = Keep parts inside
rooms-keep-caption = A part outside its room, or inside the margin, is a DRC violation.
rooms-field-margin = Margin
rooms-outside-banner = { $part } is { $distance } mm outside room { $room }.
rooms-pull-link = Pull into room

## Create Rooms from Sheets
rooms-dialog-title = Create Rooms from Sheets
# { $project } is the project file; { $channels } one of the two lines below.
rooms-dialog-subtitle = { $project } · { $sheets } sheets · { $channels }
rooms-dialog-no-channels = no repeated channels
rooms-dialog-channels = { $count } repeated channels
rooms-dialog-to-create = Rooms to create
rooms-group-sheets = Sheets
rooms-group-channels = Channels
rooms-parts-pill = { $count } parts
rooms-channels-none = No repeated sheets. A sheet placed N times gets rooms named name_CH1…CHN.
rooms-dialog-options = Options
rooms-option-shape = Shape
rooms-around-parts = Around parts
rooms-even-grid = Even grid
rooms-option-margin = Margin
rooms-option-layer = Layer
rooms-move-parts = Move parts into their rooms
rooms-move-parts-desc = Parts still at the board edge after Update PCB are packed into their room.
rooms-add-keep = Add "Keep parts inside" rule
rooms-replace-same = Replace existing rooms with the same name
rooms-preview = Preview
rooms-preview-legend = dashed = new room
rooms-preview-caption = Parts that Update PCB left below the board move into their rooms on Create.
rooms-footer = { $rooms } rooms · { $parts } parts move · one undo step
rooms-cancel = Cancel
rooms-create = Create { $count } Rooms
rooms-no-project = Create Rooms from Sheets: the board in view is in no open project with a root sheet.
rooms-read-failed = Create Rooms from Sheets: the sheets could not be read: { $why }
rooms-no-sheets = Create Rooms from Sheets: no sheet is placed under the root sheet.
rooms-kept = Room { $room } already exists and was kept.

## Arrange in Room and Auto Place in Rooms
rooms-undo-arrange = Arrange in room
rooms-undo-auto-place = Auto place in rooms
rooms-arranged = Room { $room }: { $moved } parts arranged; { $outside } did not fit and went below it.
rooms-auto-placed = Auto Place: { $moved } parts placed in { $rooms } rooms; { $unplaced } did not fit and stayed.
rooms-select-one = Arrange in Room: select one room first.

## Refusals of the rooms commands run with operands
rooms-board-no-project = { $board } is in no open project with a root sheet.
rooms-no-rooms = The board has no rooms.
rooms-unknown-room = The board has no room { $room }.
rooms-name-taken = The board already has a room named { $room }.
rooms-bad-side = `side` is top, bottom or both, not { $side }.
rooms-bad-shape = `shape` is around_parts or even_grid, not { $shape }.
rooms-bad-sheet = `sheet` is a sheet placement written /<uuid>/.., not { $sheet }.
rooms-bad-corners = `corners` is three or more points <x>,<y>;<x>,<y>;.. in mm, not { $corners }.
rooms-outline-needed = Give the room's outline as `start` and `end`, or as `corners`, not both.
rooms-no-area = The room's outline has no area.
rooms-bad-weights = The weights were not read: { $why }
