# Board import, panelisation, pin swap and design reuse (#2410)

Design notes for four board features. DXF import is built; the other three
are follow-ups and each note says what its first slice needs. Every one of
them follows the agent rule of #2410: a catalog command (`AppCommandId`)
with typed operands, the same path for the GUI, the CLI and the agent
socket, a JSON-serialisable result, and one undo step per run.

## DXF import

**Commands.** `import_dxf` puts a drawing on a board layer;
`import_dxf_outline` makes its largest closed shape the board outline.
File ▸ Import ▸ DXF to Current Layer... and DXF as Board Outline... run
them, as do the palette and any key a profile binds. Both are greyed
without a board in view.

- Without operands (menu, palette, key) the command opens the file dialog.
  `import_dxf` aims at the active layer of the board in view, read when the
  command runs, so switching layers while the dialog is open does not move
  the import.
- With operands: `import_dxf path=<file> [layer=<token>] [document=<board>]`
  and `import_dxf_outline path=<file> [document=<board>]`. `layer` is a
  layer token as saved (`TopSilkscreen`, `Mechanical1`, ...); left out it is
  the active layer. `document` names an open board; left out it is the
  board in view. A bad layer, a closed board or a document that is not a
  board is refused before anything is read. The command reports `Deferred`
  and is answered once the file is read: the summary (`graphics`,
  `outline_points`, `skipped`, `target`) or the refusal, as `Disabled`. The
  same lines go to the Messages panel. The agent socket refuses both
  commands, as it refuses every command with a `path` operand; the CLI and
  headless runs take the answer.
- A copper layer is refused, from the dialog and from operands, before any
  file is read: a drawing there would be copper that DRC, the ratsnest and
  connectivity never read.

**Reading.** The file is picked and read off the UI thread
(`DxfImportMsg::Picked`, `DxfImportMsg::Read`, the read in a blocking
task); a file over 64 MiB is not read, and one that is not UTF-8 is read
lossily. A drawing with a coordinate past the board file's nanometre range
is refused, so an import never leaves a board that cannot be saved. `signex_engine::pcb::dxf` parses it and builds the command;
nothing there does IO.

- Entities read: `LINE`, `ARC`, `CIRCLE`, `LWPOLYLINE` and `POLYLINE` with its
  `VERTEX` list, including bulged (arc) segments. Every other entity (text,
  blocks and their inserts, hatches, splines, dimensions) is skipped and
  counted in the summary.
- Units: the header's `$INSUNITS` (inches, feet, mm, cm, m, microinches,
  mils, microns, decimetres); a file without it, or unitless, is read as
  mm. Other units are refused.
- Placement: the file's origin is the board origin and its y grows up, so
  a point lands where the Properties panel shows the same coordinates. The
  y flip turns a counter-clockwise file arc clockwise in the model, so the
  board arc swaps its ends.

**Layer import.** Each entity becomes a board drawing (`PcbBoard::graphics`)
on the layer, at `PCB_DEFAULT_GRAPHIC_WIDTH_MM`: a line, an arc, a circle, a
closed straight polyline as one polygon, any other polyline as its lines
and arcs. `BoardCommand::AddGraphics` appends them; its inverse
`RemoveGraphics` takes exactly those drawings off the end again and refuses
if the list no longer ends with them.

**Outline import.** The closed shapes are closed polylines, circles, and
chains of lines, arcs and open polylines whose ends meet within 0.1 µm.
Arcs become chords of at most 6 degrees, as the board draws them. The
shape with the largest area becomes the outline through
`BoardCommand::SetBoardOutline`. A file without a closed shape is refused,
and so is a board whose outline the board sketch owns.

**Summary** (`DxfImportSummary`, serialisable): drawings added, outline
corners, entities skipped, and the target (`outline` or the layer token).

**Not yet.** Block inserts are not expanded, splines and ellipses are not
flattened, the stroke width is not read from the file's line weight, an
import is not offset or scaled on the way in, and the agent result is the
Messages line rather than a deferred answer carrying the summary.

## Panelisation (follow-up)

A panel is a document of its own that places copies of one board; the board
file is never edited by panelising it.

- **Model:** a `.snxpcb` with a `panel` section: the source board by path
  and revision, an array (columns, rows, spacing x/y, rotation per copy),
  rails (top/bottom and left/right widths, fiducials and tooling holes as
  drawn items on the rails), and the separation between copies: mouse bites
  (hole diameter, pitch, tab width and count per edge) or V-score lines
  (`BoardSketch` already bakes V-scores).
- **Geometry:** the panel outline, tabs, bites and score lines are derived in
  `pcb-geom` from the source outline and the array; the copies are drawn
  from the source board, not stored.
- **Commands:** `new_panel source=<board>` and `set_panel_array
  columns rows spacing ...`, one undo step each on the panel document.
- **Output:** fabrication output treats the panel as one board; a later
  slice teaches the output pipeline to repeat the source layers per copy.

## Pin and gate swap (follow-up)

Swapping two pins (or two units) of a component is a board edit and a
schematic edit that must stay one change.

- **Model:** the swap rule lives on the part: pins sharing a swap group
  (`pin_swap_group`) and units sharing a gate group may trade nets. The
  library part model (#663) carries the groups; a part without them allows
  no swap.
- **Board side:** a `BoardCommand::SwapPadNets { footprint, a, b }` trades
  the nets of two pads (inverse: itself). A gate swap trades the nets of
  every pad of the two units.
- **Back-annotation:** the schematic follows through the existing Back
  Annotate flow (#1312): the swap is recorded as a pending pin-net change
  and the schematic edit rewires the two pins' net labels or wires, one
  sheet undo step. The board and the sheet steps share an undo group so one
  undo takes both back.
- **Commands:** `swap_pins footprint a b` and `swap_gates footprint a b`,
  refused for pins or units outside one group, and for a locked pad net.

## Design reuse blocks (follow-up)

A reuse block is a placed and routed group saved once and placed again.

- **Model:** a block is a `.snxpcb` fragment (footprints, tracks, vias,
  zones, drawings) with its own origin, saved in a library as a block entry,
  and the schematic sheet it belongs to by reference.
- **Save:** `save_reuse_block` takes the selection, re-bases it on its
  origin and writes it through the library adapter (a Task).
- **Place:** `place_reuse_block block at rotation` copies the block in with
  new uuids (the clipboard path, `BoardClipboard`, already renumbers), maps
  each footprint to the matching part of the target sheet's channel by
  designator pattern, and maps nets through the parts' pins. Footprints
  without a match are refused, not placed loose. One undo step.
- **Follow-on:** updating every placed copy when the block changes is a
  later slice; it needs a link from each copy to its block revision.
