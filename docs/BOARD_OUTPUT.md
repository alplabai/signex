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
  Mechanical (the outline, margin, mechanical and documentation layers), and
  the two assembly drawings, Assembly Drawing, Top and Assembly Drawing,
  Bottom (see "Assembly drawings" below). A layer the build does not know
  prints with no set.
- **Pages**: one page per chosen set, in that order, or every chosen layer
  set on one composite page (drawn mechanical, inner, bottom, top, so the top
  side is on top). An assembly drawing is always a page of its own, after the
  composite page.
- **Mirror the bottom side**: a page of the bottom set or of the bottom
  assembly drawing is drawn mirrored left to right, as seen from below. A
  composite page is never mirrored.
- **Scale**: Fit to Page fills the space inside the 10 mm margins and above
  the title block; 1:1 prints the board at its size, centred (a board larger
  than the page is cut at the page edge; the page is flagged and the export
  warns which pages run off). The board's extent is its outline's,
  or the default board area when it has none.
- **Ink**: each set in its colour on white paper, or everything black.
- **Show**: the board outline on every page, drill holes (pad and via holes
  drawn open), and the title block (the board name, the page's sets, the
  scale as `1:1`, `1:2.5` or `2:1`, and `n / N`).

Drawn per set: a polygon pour as its computed fill (clear of other nets'
copper, as on the canvas) with its outline stroked, a keepout or other rule
area as its outline only, any other zone filled (with its usable holes);
tracks with round ends at their width, pads and vias on the set's copper,
footprint and board drawings and texts on the set's layers. Texts are
placed and turned, not mirrored. Lines thinner than 0.25 pt print at 0.25 pt.

Defaults: Top, Bottom and Outline and Mechanical on their own pages, the
bottom mirrored, fitted, colour, outline, drill holes and title block on, A4
landscape. The dialog keeps its options for the next opening in the session.
Export PDF asks for the file and writes it off the UI thread, crash-safe.

Headless and CLI: `export_board_pdf document=<board> path=<file>` with the
optional `layers` (comma-separated set ids `top`, `bottom`, `inner`,
`mechanical`, `assembly_top`, `assembly_bottom`), `pages` (`per_set`,
`composite`), `scale` (`fit`, `one_to_one`) and the switches `monochrome`,
`mirror_bottom`, `outline`, `drill_marks`, `title_block`; anything left out
is the default above. The answer lists each page's sets, mirror, scale,
printed size in mm and whether the board runs off the page, and the export's
warnings. CLI only (`export` capability).

Not yet: footprint designator and comment texts on the layer set pages, a
raster preview of the pages, a system printer, and a page size other than A4
landscape in the dialog (the options carry one).

## Assembly drawings

One builder, `signex_output::assembly_drawing(board, side)`, makes the top or
the bottom assembly drawing as a list of marks in board mm; the board PDF
draws it as a page and `signex_output::assembly_svg` writes it as SVG, so the
two never differ.

- **Content**, in drawing order: the Dimensions layer (board drawings,
  texts and linear dimensions), the side's Fab, Assembly and Fiducial layers
  (footprint and board drawings and texts), the side's Designator layer, the
  designator of each part on that side, then the board outline and the Edge
  Cuts drawings (the board's and the footprints', such as a slot) on top.
  The designator is the part's own designator text when
  it sits on the side's Designator layer (left out when hidden), else the
  designator centred on the part's origin, turned with the part and kept
  upright, 1 mm high. Silkscreen, copper, courtyard and the other side are
  left out.
- **Inks**: outline black, fab grey, assembly blue, fiducial green,
  designators red, dimensions purple; black in monochrome.
- **Extent**: every mark, the outline and Edge Cuts drawings included, else
  the default board area; a text counts with its estimated box, not only its
  centre. An assembly page is framed on it, so a dimension or a part body
  past the board edge stays on the page and a board drawn only with Edge
  Cuts lines still fills it.
- **PDF**: the sets Assembly Drawing, Top (`assembly_top`) and Assembly
  Drawing, Bottom (`assembly_bottom`) of Board Print / PDF, with the page's
  scale, ink, mirror and title block options.
- **SVG**: one file per side, `<board>-assembly-top.svg` and
  `<board>-assembly-bottom.svg`. One view-box unit is one board mm with a
  2 mm margin, so the file prints 1:1; numbers to the micrometre; texts are
  centred `<text>` elements in Roboto (a viewer falls back to sans-serif).
  The bottom file is mirrored when Mirror the Bottom Side is on.

Surfaces: Board Print / PDF's Export Assembly SVG, Reports ▸ Assembly Drawings
as SVG... and the palette's
Assembly Drawings as SVG (`export_assembly_svg`) ask for a folder and write
the drawings of the chosen assembly sets, both sides when none is chosen,
off the UI thread and crash-safe; a file already in the folder is replaced,
and its Messages line says "Replaced" instead of "Wrote". Headless and CLI:
`export_assembly_svg document=<board> path=<folder>` with the optional
`sides` (`top`, `bottom`; both when left out), `mirror_bottom` (on when left
out) and `monochrome`; the answer lists each file's side, path and mirror.
CLI only (`export` capability). Agents: `board_assembly_drawing
document=<board> side=<top|bottom>` with the same switches answers
`{document, side, mirrored, extent_mm, svg}` and writes nothing; in a window
it opens Board Print / PDF.

Golden tests: `signex_output::svg::board::tests` and
`signex_output::pdf::board::assembly_tests` draw the corpus board
`mixed-tht-smd` and compare with `crates/signex-output/testdata/assembly/`
(regenerate with `UPDATE_GOLDEN=1 cargo test -p signex-output assembly` when
the drawing is meant to change). The PDF goldens are the pages' content
streams with numbers rounded to 0.01 pt.

Not yet: free arcs and free pads on the drawing's layers.

## STEP export

On a board, File ▸ Export ▸ STEP 3D Model... and `export_board_step` write
the board and its parts as a STEP model (#2488) for MCAD tools. It reads the
board as it is in memory and changes nothing.

- **Board body**: the outline (or, with none, the box round the copper, as
  the 3D view frames it) extruded from z = 0 to the board thickness, which
  the board engine keeps equal to the layer stack's total. The cutouts,
  mounting holes, pad drills and via drills inside the outline are cut
  through; a hole that meets one cut before it is left out, as in the 3D
  view, and the export warns how many. The body is built on the MCAD kernel
  (`signex_mcad::board_body`) and written exactly: flat faces on a `PLANE`,
  hole walls on a `CYLINDRICAL_SURFACE`, straight edges on a `LINE`, hole
  rims on a `CIRCLE`. Each face is named after its persistent name (`Top`,
  `Bottom`, `Outline side 3`, `Hole 12 wall`, `Cut-out 0 wall`). A board
  that falls apart into pieces is one solid per piece.
- **Parts**: each footprint's body is the one the 3D view draws (its
  attached model, placed by the attachment's offset and turn, else its 3D
  Body, else a box over its pads), placed as the footprint is and stood on
  its face. A model is a triangle mesh, so a part is a faceted surface body
  (one `ADVANCED_FACE` per triangle), named by its designator. A model that
  cannot be read is replaced by the plain body, with a warning.
- **File**: ISO 10303-21, schema AP214 (`AUTOMOTIVE_DESIGN`, the default,
  read by nearly every MCAD tool) or AP242 (`schema=ap242`). One product,
  named after the board file, in millimetres; x right, y up (the board
  seen from above), z up out of the top face, the bottom face at z = 0. No
  colours and no copper.
- **Where it is written**: `signex-mcad` turns solids and meshes into bytes
  (`write_step`, no IO); `signex_renderer::board3d::board_step` gathers the
  board body and the parts; the app reads the parts' models and runs the
  kernel off the UI thread, then writes the file through the export write
  path (atomic, off the UI thread).

Headless and CLI: `export_board_step document=<board> path=<file>` with the
optional `schema` (`ap214`, `ap242`); the answer names the board, the file
and the schema. It takes a path, so it is CLI only (`export` capability).

Not yet: colours, copper and silkscreen on the faces, each part as its own
product in an assembly, and a part's exact STEP geometry when its model was
a STEP file (it is written from the 3D view's mesh).
