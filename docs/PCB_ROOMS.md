# Rooms on a board

A room is a named area of a board that holds the parts of one schematic
sheet placement, or one channel of a sheet placed several times (#2457).
Rooms are drawn on the board, edited in Room Properties, made one by one with
Place ▸ Room or for the whole design with Design ▸ Rooms ▸ Create Rooms from
Sheets, and checked by the design rule `builtin::part_outside_room`.

## Model

`signex_types::pcb::room::Room`, kept in `PcbBoard::rooms`:

| Field | Meaning |
|---|---|
| `uuid`, `name` | Identity and the name drawn at its top-left. |
| `source` | The sheet placement (`InstancePath`) whose parts it holds, and for a channel its number from 1; `None` for a room drawn by hand. |
| `layer` | `top`, `bottom` or `both`: the side view that shows it. |
| `outline` | A rectangle (`x`, `y`, width, height) or a polygon of corners, board mm. |
| `colour` | `#RRGGBB`, from a fixed palette (`ROOM_PALETTE`) in creation order, stored in the room so it keeps its colour when other rooms come and go. |
| `keep_parts_inside`, `margin_mm` | The rule: a member part outside the outline shrunk by the margin is a DRC finding. |
| `locked` | A locked room refuses a move and a fit. |

**Membership.** A footprint is a member of a room when its schematic link
names a part on the room's sheet placement or a sheet below it
(`Room::holds`, the same "is under" rule Update PCB uses). A room without a
source holds no parts. The side of the board does not change membership; the
layer only says which side view shows the room.

## Edits

Every edit is one board command and one undo step
(`signex_engine::pcb::rooms`): `AddRoom`, `RemoveRoom`, `EditRoom` (name,
source, layer, shape and outline, colour, rule, margin, lock), `MoveRoom`,
`FitRoomToParts` and `PullPartsIntoRoom`. Each inverse restores absolute
values (`PlaceRoom` puts back the outline and every moved part's origin), so
undo is exact.

- **Move** moves the room so its box starts at the new X / Y and moves its
  unlocked member parts by the same distance. A locked room refuses it.
- **Fit room to parts** sets the outline to the rectangle round the members'
  bodies plus the margin. A locked room, or one without members, refuses it.
- **Pull parts into room** packs the unlocked members outside the room into
  its outline shrunk by the margin as Arrange in Box packs
  (`pcb_geom::pack_in_box`): in rows from its top-left, tallest first, each
  origin on the components grid, 0.5 mm apart and from the edge, and clear of
  the members already inside. Parts that do not fit go on in rows below the
  room. A polygon room is packed into the box round it.
- **Shape**: Rectangle ↔ Polygon. A rectangle becomes the polygon of its four
  corners; a polygon becomes the rectangle round it. Moving or adding polygon
  corners on the canvas is not built yet.

## Place ▸ Room

`place_board_room`, the row after Keepout in Place on a board and in the PCB
Active Bar's copper-area group. The first click is a corner; the room follows
the pointer, drawn like a selected room; the second click places it as one
undo step, named `Room N` (the lowest free number) in the next palette colour,
with the default 0.5 mm margin and the rule off. The tool then ends and the
new room is selected, so Properties shows it. Esc drops the first corner,
then ends the tool.

## Room Properties

With exactly one room selected, the Properties panel shows Room Properties:

- the header: a swatch in the room's colour, "Room", the name as an accent
  pill and the lock button;
- a line-art illustration (the room's shape, its member parts, outside ones
  in the error ink, and the margin outline while Keep parts inside is on)
  with Width, Height, X, Y, Layer and Margin callouts linked to their fields:
  hovering either lights the other. There are no handles on the drawing; the
  fields edit;
- **Room**: Name, From sheet (the sheet placements the board's parts are
  linked under, named by the room made for them or by a short id), Layer,
  Shape and Colour;
- **Position**: X / Y and Width / Height, mm. X / Y moves the parts too;
- **Parts**: "N inside" or, in the warning colour, "N outside"; a chip per
  member, an outside one marked "· outside"; Pull parts into room and Fit
  room to parts;
- **Rule**: Keep parts inside heads the section as its switch; with it off
  the caption and Margin grey out. A part outside its room adds a warning
  banner naming the part, the distance and the room, with a Pull into room
  link.

Without the rule, a part counts as outside only past the outline itself.

## Create Rooms from Sheets

`create_rooms_from_sheets` (Design ▸ Rooms), greyed without a board in view
that belongs to an open project. It reads the project's sheets off the update
thread and opens the dialog for the sheets placed directly under the root.

- **Rooms to create**: one row per sheet placed once, then the channels of
  each sheet placed N times, named `<sheet>_CH1`..`<sheet>_CHN`. Each row has
  a checkbox, its colour, its name, its sheet file and its part count.
- **Options**: Shape (Around parts, Even grid), Margin (0.500 mm), Layer,
  Move parts into their rooms (on), Add "Keep parts inside" rule (on),
  Replace existing rooms with the same name (off).
- **Preview**: the board outline on the board palette's dark ground, its
  parts, the planned rooms dashed, and the parts that will move drawn dashed
  where they are now.
- **Create N Rooms** runs the plan as one undo step; the footer counts the
  rooms and the parts that move.

The planner is pure (`plan_rooms_from_sheets`):

- *Around parts*: the box round the members already on the board (inside the
  board's outline box) plus the margin. With Move parts on, the members off
  the board are packed into it as Pull parts into room packs them, and the
  room grows over the rows they take. A sheet with no member on the board
  gets a grid cell.
- *Even grid*: the board's box split into one cell per room, rows of about
  the board's aspect.
- Colours follow the palette per sheet in order, after the rooms the board
  keeps; the channels of one sheet share its colour.
- A room of the same name already on the board stays and its sheet gets no
  new room, unless Replace is ticked: then the old room is removed and the new
  one takes its colour.
- With Move parts on, members outside their room are packed into it.

## Design rule

`builtin::part_outside_room` ("Part outside its room"), error by default and
overridable like the other built-in rules. Only rooms with Keep parts inside
are checked, so a board without such rooms gets the report it got before. A
member part whose body box (`pcb_geom::footprint_body`) is not inside the
room's outline shrunk by its margin is a finding; `measured_nm` is how far
the body reaches past that line, `required_nm` the margin, and the items are
the footprint and the room. Its check-report message names them:
`R2 is 1.2 mm outside room dcdc`.

## File format

Rooms are a top-level `[[rooms]]` array of the `.snxpcb`, written only when
the board has rooms, so a board without them saves byte-identical. Lengths
are integer nm like every other on-disk geometry.

```toml
[[rooms]]
uuid = "..."
name = "dcdc"
sheet = "/<placement uuid>"
layer = "top"
shape = "rectangle"
x = 20000000
y = 2000000
width = 24000000
height = 33000000
colour = "#A058C8"
keep_parts_inside = true
margin = 500000
locked = false
```

A polygon room has `shape = "polygon"` and `points = [[x, y], ...]` instead
of `x`, `y`, `width` and `height`; a channel room adds `channel = <n>`. A
room that cannot be read (an unknown layer or shape, a colour that is not
`#RRGGBB`, a rectangle without size, a polygon under three corners, a
negative margin, a bad sheet path) refuses the file, naming block `rooms` and
the room's row. Keys of a room this build does not know are kept and written
back.

A board with rooms writes the format token `snxpcb/1.6` (`snxpcb_token`, from
content), so a build that reads only up to `snxpcb/1.5` opens it read-only
and refuses every save rather than writing the board without its rooms. A
board without rooms keeps `snxpcb/1.4`, or `snxpcb/1.5` with layer colours,
byte for byte, and removing the last room writes the lower token again
(maintainer decision 2026-11-20, `docs/FORMAT_VERSIONS.md`).

## Not built yet

- Dragging, adding or removing a polygon room's corners on the canvas.
- Delete on a selected room (Properties edits it; Undo removes a room just
  placed).
- From sheet names placements by the room made for them or a short id, not by
  their sheet file.
- The Layers panel cannot hide rooms yet; the selection filter's Rooms chip
  turns their picking on and off.
