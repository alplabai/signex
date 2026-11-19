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
