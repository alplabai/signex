# Board output: reports and print

What a board gives out besides its fabrication files: four report tables
(#2404) and a printed or PDF drawing of its layers (#2402). Both read the
board as it is in memory; neither changes it, so there is no undo step and no
dirty mark. Fabrication outputs (Gerber, drill) are a separate feature.

The domain code is in `signex-output`: `board_report` (the tables) and
`pdf::board` (the drawing). The app slice is `crates/signex-app/src/app/board_output/`
and its dialogs `crates/signex-app/src/app/view/dialogs/board_output/`.

## Reports

| Report | Columns | Rows |
|---|---|---|
| BOM (`bom`) | designators, quantity, value, footprint, description, type | One per value + footprint + description + type, designators in natural order (`R1, R2, R10`), lines ordered by their first designator. A type the BOM leaves out (`ComponentType::in_bom`, #2365: Standard (No BOM), Mechanical, Graphical, Net Tie) is not listed. |
| Netlist (`netlist`) | net, node count, nodes | Every net of the board's net table and every net a pad names, net 0 left out, in natural name order; nodes are `designator.pad`. |
| Net lengths (`net_length`) | net, length (mm), tracks, vias | The same nets; the length is every track on the net (`pcb_geom::net_routed_length_mm`), summed in integer nm. |
| Components (`components`) | designator, footprint, value, x, y (mm), rotation (degrees), side | Every footprint in designator order; X and Y count from the board origin with Y up (`BoardOrigin::to_user`), as the board panel shows them. |

Cells are typed (`ReportCell`): text, a count, millimetres, degrees, a side,
a component type, or missing. The dialog shows lengths in the user's unit
(mm or mil, as the board panel), sides and types in words. JSON carries
numbers as numbers and the column ids below. CSV is RFC 4180 with a UTF-8
byte-order mark and CRLF line ends, the BOM export's writer (quoting, and a
cell a spreadsheet would run as a formula is neutralised); the header row is
the column ids: `designators`, `quantity`, `value`, `footprint`,
`description`, `type`, `net`, `node_count`, `nodes`, `length_mm`, `tracks`,
`vias`, `designator`, `x_mm`, `y_mm`, `rotation_deg`, `side`. Millimetres are
written to the nm, degrees to a thousandth, trailing zeros dropped.

Surfaces: Reports ▸ Board Bill of Materials..., Board Netlist..., Net
Lengths..., Component Report... (`board_report_bom`, `board_report_netlist`,
`board_report_net_length`, `board_report_components`), the Board panel's
Reports button (the BOM), the palette; Board Reports has a tab per table and
Export CSV (`export_board_report`: the table in view, the BOM when the dialog
is closed). A table is made from the board when its tab is picked; Export CSV
makes it again from the board as it is then.

Agents and the CLI: `board_report_<kind> document=<board>` answers with
`{kind, columns, rows, document}`; it is on the agent channel.
`export_board_report document=<board> kind=<kind> path=<file>` writes the CSV
and answers with the path and row count; it takes a path, so it is CLI only
(`export` capability). The board must be open.

## Print and PDF

On a board, File ▸ Print (Ctrl+P), File ▸ Export ▸ PDF and
`export_board_pdf` open Board Print / PDF.

- **Layer sets** (`BoardLayerSet`): Top Side (top copper and every top-side
  technical layer), Bottom Side (the same below), Inner Copper, Outline and
  Mechanical (the outline, margin, mechanical and documentation layers). A
  layer the build does not know prints with no set.
- **Pages**: one page per chosen set, in that order, or every chosen set on
  one composite page (drawn mechanical, inner, bottom, top, so the top side
  is on top).
- **Mirror the bottom side**: a page of the bottom set alone is drawn
  mirrored left to right, as seen from below. A composite page is never
  mirrored.
- **Scale**: Fit to Page fills the space inside the 10 mm margins and above
  the title block; 1:1 prints the board at its size, centred (a board larger
  than the page is cut at the page edge). The board's extent is its outline's,
  or the default board area when it has none.
- **Ink**: each set in its colour on white paper, or everything black.
- **Show**: the board outline on every page, drill holes (pad and via holes
  drawn open), and the title block (the board name, the page's sets, the
  scale as `1:1`, `1:2.5` or `2:1`, and `n / N`).

Drawn per set: zones filled (with their usable holes), tracks with round
ends at their width, pads and vias on the set's copper, footprint and board
drawings and texts on the set's layers. Texts are placed and turned, not
mirrored. Lines thinner than 0.25 pt print at 0.25 pt.

Defaults: Top, Bottom and Outline and Mechanical on their own pages, the
bottom mirrored, fitted, colour, outline, drill holes and title block on, A4
landscape. The dialog keeps its options for the next opening in the session.
Export PDF asks for the file and writes it off the UI thread, crash-safe.

Headless and CLI: `export_board_pdf document=<board> path=<file>` with the
optional `layers` (comma-separated set ids `top`, `bottom`, `inner`,
`mechanical`), `pages` (`per_set`, `composite`), `scale` (`fit`,
`one_to_one`) and the switches `monochrome`, `mirror_bottom`, `outline`,
`drill_marks`, `title_block`; anything left out is the default above. The
answer lists each page's sets, mirror, scale and printed size in mm. CLI only
(`export` capability).

Not yet: a raster preview of the pages, a system printer, and a page size
other than A4 landscape in the dialog (the options carry one).
