# Board options (#2374)

The board Properties panel (nothing selected) edits these board-level
settings. They are document data: stored in `PcbBoard::options`
(`signex_types::pcb_options::BoardOptions`), saved in the `.snxpcb`, and
changed only through `BoardCommand::SetBoardOptions`, one undo step per edit.

| Setting | What it does |
|---|---|
| Guides | Endless lines through a point: vertical, horizontal, +45° (rising to the right) or -45°. Each has an enabled flag and a colour. Enabled guides are drawn across the view and are snap targets while Snap Options ▸ Guides is on. |
| Extra grids | Snap grids above the board's own step (`PcbSetup::grid_size`), in priority order (first = priority 1). Each has a name, step, origin, colour, and applies to components, non-components or both. The Global Snap Grid (priority 50, the first card of the Grid Manager, which lists the extra grids under it highest priority first) is the board's own step counted from the board origin and applies to everything no extra grid claims. Extra grids are drawn over the board area in their colour. |
| Sheet | The rectangle drawn behind the board area. With Size From Board on (the default) it is the box around the board shape: a board has no separate sheet layer. Turned off, the stored X, Y, Width and Height are used; turning it off keeps the current box, so nothing jumps. |
| Pour Names | How a polygon pour is named: `NET NAME_LXX_PXXX` (default), `LXX_NET NAME_PXXX`, `Board Layer_NET NAME_PXXX` or `NET NAME_Board Layer_PXXX`. `PolygonNaming::name` builds the name; pours are named by the pour tool (#2373). |
| Designators | Physical or Logical. Stored for multi-channel designs; boards have one designator per part today, so both show the same text. |

## Origin

The board origin (#2384), `BoardOptions::origin` (`signex_types::pcb_origin::BoardOrigin`), is the point a board shows as (0, 0). Shown coordinates run with X to the right and Y up the screen; the model's y grows down the screen, so `BoardOrigin::to_user` and `from_user` mirror Y about the origin. The status bar, the Component view's X and Y, the track, via and pad X/Y fields, and the Guide Manager, Grid Manager and Sheet coordinates read and type through it; the sheet's X and Y are its bottom-left corner.

A new board (`PcbDefaults::new_board`, the New PCB setup dialog included) has no outline yet, so its board area is `EMPTY_BOARD_AREA` (100 × 80 mm) and its origin is that area's bottom-left corner (`board_area_bottom_left`). A board saved without an origin keeps counting from the model's zero. The origin marker is drawn at the origin, and the board's own grid (drawn and snapped) counts from it.

Edit ▸ Origin moves it (2026-11-18): **Set** (`set_board_origin`) takes the next click on the board, on the non-component grid; **Reset** (`reset_board_origin`) puts it at the bottom-left of the outline, or of the board area without one; **Snap to Outline Corner** (`snap_board_origin_to_corner`) puts it at the outline or board-area corner nearest the pointer. Each is one `SetBoardOptions` undo step, marks the board changed and is written on Save; all three are greyed on a read-only board. This is how a board saved without an origin is given one.

Update PCB stages new parts to the right of the board; on a board with no outline and no parts it stages them inside the board area from its bottom-left corner, rows going up (`pcb_geom::stage_up`).

## Which grid snaps what

`placement::grid_for(board, GridUse)` is the one place that decides. A carried
footprint, a placed library part and a paste holding a footprint use
`GridUse::Components`; a paste of only tracks and vias, and a guide placed with
Place, use `GridUse::NonComponents`. The first extra grid that applies wins;
with none, the board's own step counted from the board origin. Extra grids have no extents: a
grid that applies to components governs every component carry on the board.

## Snapping order while footprints are carried

`pcb_canvas::carry_delta`, in order:

1. An object point within the snap distance (Distance × the board step)
   takes the grab point exactly onto it. Snapping Off skips this step and
   step 4.
2. Otherwise the nearest footprint origin lands on the components grid (with
   Grids on).
3. With Guides on, a crossing of two enabled guides within the snap distance
   takes the grab point onto it; a vertical guide fixes x, a horizontal one
   fixes y, a diagonal one takes the grab point onto its nearest point.
4. With Axes on, an axis no guide fixed lines up with the nearest object point
   that is within Axis Range of the pointer and within the snap distance
   on that axis.

The object kinds are the Objects for snapping list. Track/Arc Lines snap to the
nearest point of a track, a board line or a board circle; Intersections to
where two tracks or board lines near the pointer cross; Arc Centers to the
centres of board and footprint arcs and circles (All Layers only, as they sit
on no copper layer); Pad Vertices to the corners of rectangular pads; Pad Edges
to the nearest point of a pad's outline. 3D Body Snap Points stays disabled:
a board holds no 3D bodies.

## Settings kept for the session

Grids / Guides / Axes, Snapping, the object list, Distance and Axis Range
(default 10 mm) are editor settings in `UiState::pcb_snap`, as #2353
decided for the snap options. They are not written to the board.

## File format

`[board_options]`, written after `[board_sketch]` and only when the options
differ from the default, so a board without any re-saves byte-identical. The
section did not change the format token (it was `snxpcb/1.3` then). Lengths are integer nm, colours `#rrggbbaa`.

```toml
[board_options]
polygon_naming = "layer_net_pour"
designator_display = "logical"

[board_options.sheet]
from_board = false
x = -5000000
y = -5000000
width = 210000000
height = 297000000

[[board_options.guides]]
uuid = "…"
kind = "plus45"          # vertical | horizontal | plus45 | minus45
x = 1500000
y = -2250000
enabled = true
color = "#2ec4d8ff"

[[board_options.grids]]
uuid = "…"
name = "Fine"
step = 125000
origin_x = 500000
origin_y = 750000
color = "#e8c43aff"
components = true
non_components = false
```

The origin is `[board_options.origin]` with `x` and `y` in nm, written only when it is not the model's zero.

Layer colours (2026-11-18) are `[board_options.layer_colors]`: one key per
layer the user gave a colour, its `.snxpcb` token, to `#rrggbbaa`, sorted by
token and written only when some layer has one. A board with layer colours
is written as `snxpcb/1.5` (`docs/FORMAT_VERSIONS.md`); a token or colour
this build cannot read refuses the file. Layer colour... and the reset rows
of a layer tab, the Layers panel's colour chips and `set_layer_color` /
`reset_layer_colors` set them, each as one `SetBoardOptions` undo step.

Waived design-rule findings (#2399) are `[[board_options.drc_waivers]]`, one
table per waiver with `key` (`signex_drc::waiver_key`), `rule`, `note` and
`fingerprint`, written only when the board waives one. The DRC results panel
and the `waive_drc_violation` and `unwaive_drc_violation` commands set them,
each as one `SetBoardOptions` undo step. `fingerprint` (#2307, 32 hex digits,
`signex_drc::fingerprint`) is the geometry of the finding's objects when it
was waived; when they change the waiver lapses. A waiver saved before #2307
(26.11.0) has no `fingerprint` key and loads with an empty one; it lapses
once and is waived again. A board whose waivers all lack it is re-saved byte-identical.
`author` (AV-13) names who accepted the finding; the DRC panel's waiver
form asks for it (it starts as the user identity) and `waive_drc_violation`
takes it as `author=`. A waiver with no author writes no `author` key, so a
board saved before it re-saves byte-identical.
See `docs/architecture/pcb-drc.md`, "Waivers".

The panel (#2410) is `[board_options.panel]`, written only when the board has one; the panel edits run its own `SetBoardPanel` command, and the options edits here carry it unchanged. See [PANELISATION.md](PANELISATION.md).

A colour that is not `#rrggbb` or `#rrggbbaa` refuses the load; a length that
is not finite refuses the save.
