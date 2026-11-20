# Project thumbnails

The Start screen shows a small preview tile on each recent and pinned
project row. This page describes what the preview shows, when it is made,
and where it is stored. Decision: `docs/DECISIONS.md`, 2026-11-19, "Start
screen thumbnails".

## What the tile shows

The tile is 64 x 46 px at the default base size. It shows one of three
things:

| State | Tile |
|---|---|
| A saved preview exists | The line art, with a `PCB` or `SCH` badge in the bottom-right corner. |
| No preview yet | A sunken tile that says "No preview yet". The project was not saved since previews were added. |
| Project missing on disk | A dashed tile with a file glyph. A stale picture is never shown for a missing project. |

A project with a board shows the board on the board canvas ground, in the
board palette:

- the board outline;
- top and bottom copper: tracks, and surface pads as filled areas;
- through-hole pads and vias in the Multi-Layer colour;
- top and bottom silkscreen. The palette has one silkscreen colour, so the
  bottom face draws it darker.

Inner copper, zones and text are not drawn.

A project without a board shows its root sheet on the schematic paper of the
current theme: wires, symbol bodies and pins, and power ports in the power
colour (Graphite Light `#C0392B`, Graphite Dark `#E06C5C`). Text is not
drawn. The board art looks the same in both themes, because board views use
one palette. The sheet art follows the theme.

A project names one board (`pcb_file`), so "the first board in project
order" is that board. Projects with several boards, and a way to choose the
board, stay open in #2590 (Caner, 2026-11-20).

The art keeps at most 3000 line segments; a circle counts as 8. When a
board or sheet has more, the smallest primitives are dropped first.

## When the preview is made

The preview is made from the document in memory after a save succeeds:

- Saving the project's board makes the preview from that board.
- Saving any sheet of a project that has no board makes the preview from
  the root sheet, if the root sheet is open.
- Any other save leaves the preview as it is.

A change made to the project's files outside Signex does not redraw the
preview; the next save in Signex does. Refreshing on outside changes stays
open in #2590 (Caner, 2026-11-20).

The preview shows on the Start screen at once. The cache file is written on
a background worker. If the write fails, a warning goes to the Messages
panel and the preview still shows for the rest of the session.

At start-up, one background read loads the cached previews of the recent and
pinned projects.

## Cache location and format

Each project has one file:

```
<config root>/thumbnails/<hash>.json
```

`<config root>` is the per-user Signex configuration folder (see
`crates/signex-app/src/config_root.rs`). `<hash>` is the FNV-1a 64-bit hash
of the project path, written as 16 lowercase hex digits. The path is spelled
as the recent and pinned lists spell it. The hash is the same on every
platform and build.

The file is JSON:

```json
{
  "version": 1,
  "project": "/home/me/hw/dcdc/dcdc.snxprj",
  "thumbnail": {
    "kind": "board",
    "view_box": [1000.0, 750.0],
    "primitives": [
      { "ink": "board_outline", "filled": false,
        "geometry": { "kind": "polyline", "closed": true,
                      "points": [[0.0, 0.0], [1000.0, 0.0], [1000.0, 750.0], [0.0, 750.0]] } },
      { "ink": "multi_layer", "filled": true,
        "geometry": { "kind": "circle", "center": [500.0, 375.0], "radius": 7.5 } }
    ]
  }
}
```

- `kind` is `board` or `sheet`.
- `view_box` is the width and height of the art. The longer side is 1000.
  Coordinates are rounded to 0.1.
- `ink` is the colour role: `board_outline`, `top_copper`, `bottom_copper`,
  `multi_layer`, `top_silkscreen`, `bottom_silkscreen`, `wire`,
  `symbol_body` or `power_port`. The colour comes from the theme when the
  tile is drawn, not from the file.
- Primitives are listed in paint order.

The file is written atomically: a temporary file, fsync, then rename. The
following files read as "No preview yet" and never stop start-up:

- a file of another `version`;
- a file whose `project` is a different path (a hash collision);
- a file that does not parse.

The next save rewrites the file. A file that exists but cannot be read is
reported in Messages.

Deleting the `thumbnails` folder only removes the previews. Each project
gets its preview back on its next save.

## Code

- `crates/signex-app/src/project_thumbnail/snapshot.rs`: the art, built from
  the model.
- `crates/signex-app/src/project_thumbnail/cache.rs`: the file, its name and
  version.
- `crates/signex-app/src/project_thumbnail/tile.rs`: the tile.
- `crates/signex-app/src/app/handlers/document_files/save_async.rs`
  (`refresh_project_thumbnail`): the rebuild after a save.
