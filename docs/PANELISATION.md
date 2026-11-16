# Panelisation (#2410)

A board can be arrayed into a manufacturing panel: copies of the board with
gaps between them, waste rails, fiducials, tooling holes, and mouse bites or
V-score lines to break the copies apart after assembly. This is the panel
slice of the #2410 design note on the DXF import branch
(`docs/BOARD_IMPORT_AND_REUSE.md`, "Panelisation"); where this slice differs
from that note, it says so below.

## Model

The panel is board data: `BoardOptions::panel` (`signex_types::pcb_panel`),
`None` for a board made on its own. The board file is the panel's source, so
the copies are never stored; they are derived from the board's outline and
items whenever they are needed. This replaces the note's "panel document
that names the source board by path and revision": a panel kept on the board
cannot go stale against its source, and needs no second document kind. A
panel of several different boards is not covered by this slice.

`BoardPanel` holds:

| Setting (operand name) | Meaning | Default |
|---|---|---|
| `columns`, `rows` | copies along x and y, 1 to 50 each | 2, 2 |
| `spacing_x`, `spacing_y` | gap between copies, and between a copy and a rail, mm | 2, 2 |
| `rail_top`, `rail_bottom`, `rail_left`, `rail_right` | rail widths, mm; 0 leaves a side without a rail | 5, 5, 0, 0 |
| `fiducial_diameter` | copper dot of each fiducial, mm; 0 places none | 1 |
| `tooling_hole_diameter` | non-plated tooling hole, mm; 0 places none | 2 |
| `separation` | `mouse_bites` or `v_score` | `mouse_bites` |
| `bite_hole_diameter`, `bite_pitch` | perforation holes along a tab, mm | 0.5, 0.8 |
| `tab_width`, `tabs_per_edge` | tab length along a copy's edge, mm; tabs on each edge, 1 to 20 | 3, 1 |

The bite settings are kept while V-scores are chosen, so switching back loses
nothing. Rules every panel keeps (`BoardPanel::check`): counts in range,
lengths finite and not negative; with mouse bites, the bite settings above 0,
the pitch above the hole diameter, and both gaps above 0. A panel that breaks
one is refused by the board engine and on load.

## File

The `.snxpcb` stores the panel in an optional `[board_options.panel]` table,
lengths as integer nm like every other geometry, the separation as its token:

```toml
[board_options.panel]
columns = 3
rows = 1
spacing_x = 0
spacing_y = 1500000
rail_top = 5000000
...
separation = "v_score"
```

Written only when the board has a panel, so a board without one re-saves
byte-identical. A table that breaks a rule fails the load with the reason.

## Geometry

`pcb_geom::panel::layout` lays a panel out on the board's outline, in integer
nm, y down as in the board model. The first copy is the board where it is;
the others step by the outline's bounding box plus the gap. The frame is the
copies with a gap and a rail on each side that has a rail.

- **Fiducials and tooling holes** sit on the top and bottom rails, centred
  across the rail: a tooling hole half a rail width in from each end, a
  fiducial a rail width and a half in, on the left of the top rail and at both
  ends of the bottom one, so the three never look the same turned.
- **Mouse bites**: `tabs_per_edge` tabs of `tab_width`, spread evenly along
  each side of the board's box that faces a neighbour or a rail (none on the
  panel's own edge). Each tab carries a centred row of holes `bite_pitch`
  apart along the copy's edge. The routed edges are each copy's outline and
  each rail's inner edge, open where a tab crosses, plus the tab's two sides
  across the gap. Tabs that do not fit an edge are refused.
- **V-scores**: a straight line across the whole frame along every copy edge
  that is not the panel's own edge; copies that touch share one line.
- Tabs and score lines follow the outline's bounding box. On an outline that
  is not a rectangle the Properties panel says so: a tab may not reach the
  board there.

A board without an outline cannot be panelised.

## Commands

| Command | Operands | Does |
|---|---|---|
| `panelize_board` | `document` | the default panel; a board with one keeps it |
| `remove_board_panel` | `document` | the board on its own again; refused without a panel |
| `set_board_panel` | `document`, `field`, `value` | one setting: a whole number, a length in mm, or `mouse_bites` / `v_score` |
| `board_panel` | `document` | query: the panel and what it comes to |

Each edit is `BoardCommand::SetBoardPanel`, one undo step labelled "Panel",
refused before it runs when the panel does not lay out on the board. With
operands an edit answers with the same JSON as `board_panel` plus `changed`:

```json
{
  "document": "/project/board.snxpcb",
  "revision": 7,
  "panel": { "columns": 2, "rows": 2, "spacing_x": 2.0, "...": "...", "separation": "mouse_bites" },
  "layout": { "width_mm": 102.0, "height_mm": 96.0, "copies": 4, "tabs": 8,
              "bite_holes": 48, "v_scores": 0, "fiducials": 3,
              "tooling_holes": 4, "rectangular": true },
  "changed": true
}
```

`panel` is `null` for a board without one; `layout` is `{ "error": ... }` for
a panel that no longer lays out (the outline changed after it was set). The
four commands join the agent channel (they name their board); the edits are
refused inside `apply_edits`, which holds one sheet's steps.

Without operands, Tools ▸ Panelize Board and Tools ▸ Remove Panel, the
palette and a bound key act on the board in view. `set_board_panel` has no
operand-free form; a person edits the panel in the Panel section of the
board's Properties (nothing selected): each setting typed and applied with
Enter in the panel's unit, Mouse Bites | V-Score as a segmented control, the
panel's size, copies and tabs or score lines under them, and Remove Panel.
A board opened read-only shows the settings without editing them.

## Preview

The board view draws the panel behind the board: the frame, the rails and
the other copies in the board area's colour, the routed edges and score
lines in the board edge colour, and the fiducials, tooling and bite holes as
rings. The copies show the board's shape, not its content.

## Output

File ▸ Fabrication Outputs on a board with a panel shows a "Whole panel" box,
ticked by default. Generate then builds `pcb_geom::panel::panel_board`, the
panel as one board, and writes it through the same writers as any board
(`docs/FABRICATION_OUTPUTS.md`):

- every copy's footprints, tracks, vias, zones, drawings, texts and sketch
  items, moved to its place; the first copy keeps the board's uuids, the
  others get their own;
- each copy's nets are its own (`GND/2` is copy 2's `GND`, numbered past the
  copy before), so the test netlist does not join the copies; designators
  repeat per copy, as pick-and-place of a panel expects;
- the frame is the profile, the routed edges are drawings on the profile
  layer, the fiducials are fixed graphical footprints (no BOM or
  pick-and-place row), the tooling and bite holes are non-plated holes, and
  the score lines go on the V-cut image.

Unticked, the board alone is written. The board is never edited by an output
run.

## Not in this slice

A separate panel document of several boards, per-copy rotation, rail
designators or text, an exact board-shaped route (tabs follow the bounding
box), fiducials or holes placed by hand, and drawing the copies' content in
the preview.
