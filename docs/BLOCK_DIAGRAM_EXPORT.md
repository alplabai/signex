# Block diagram export (SVG and PDF)

A block diagram (`.snxblk`, `docs/architecture/snxblk-format.md`) is written
as an SVG file or a PDF page (#2796, mockup row 171). One builder,
`signex_output::block_diagram_drawing`, turns the diagram into marks in
diagram millimetres; `block_diagram_svg` and `block_diagram_pdf` write the
same marks, so the two files show the same picture.

The saved file is what is exported. Changes still open in a diagram tab are
not in the export until they are saved.

## What is drawn

In this order, so a later mark draws over an earlier one:

1. **Groups**: a frame 5 mm outside the blocks of the group, in the group's
   colour (grey when it has none), its name above the top-left corner. A
   group with no block draws nothing.
2. **Connections**: a right-angle line from its start to its end with a
   filled arrow head at the end (the signal flows from `from` to `to`). A
   connection that names a port starts or ends on that port; one that names
   none starts or ends in the middle of the block edge that faces the other
   block. Both ends leave along their edge's direction; when both leave
   sideways (or both up or down) the line turns halfway between them.
   Power is red and 0.5 mm wide, a bus blue and 0.8 mm wide, any other
   interface the line ink at 0.35 mm.
3. **Labels of connections**: over the middle of the longest level segment
   (the longest segment when none is level): the protocol name, then the
   label (`I2C SDA, SCL`). A plain signal, power, analog or bus connection
   shows its label alone; another interface shows its own name.
4. **Blocks**: the outline by shape (rectangle; rounded rectangle with a
   radius of 15 % of the shorter side; ellipse; hexagon with its points on
   the left and right sides), filled with 18 % of the block's colour on
   paper (a light grey when it has none) and stroked in the block's colour.
   The name sits in the middle, the function under it, the note under the
   block, one line per line of the note. Each port is a 1.6 mm square on
   its edge with its name inside the block beside it.

Text is one line: line breaks and runs of spaces read as one space (a note
keeps its lines). The extent of the drawing counts a text by an estimated
width (0.6 of its height per character), so a long label is not cut off.

**Monochrome** draws every mark in black on white: blocks are unfilled
(paper), the port squares and arrows black.

## SVG

One view-box unit is one diagram millimetre, with 5 mm of paper around the
drawing, so the file prints at 1:1. Text asks for Roboto and falls back to
the viewer's sans-serif. Numbers are written to the micrometre, so one
diagram always gives the same bytes.

## PDF

The pages `pages` asks for, in order, on the paper `paper` and
`orientation` name (A4 landscape by default). The diagram's page is the
drawing fitted inside a 10 mm margin, with the title block of the board PDF
under it (the diagram's file name, "Block diagram" in the person's
language, the scale and the page number) unless `title_block=false`. The
Power tree and Budget table pages are tables of the project's power tree
(`signex_output::BlockDiagramTable`), read by the run behind
`query_power_tree`; a table runs on over as many pages as its rows need. The fonts are the embedded, subset Roboto of every
Signex PDF. Characters no bundled font covers are listed in the answer and in
the Messages panel, as for the other PDFs.

## Running it

In a window, `export_block_diagram` (command palette) takes the diagram in
view when the active tab is a `.snxblk`, else asks for one, then asks for
the file: `<name>_blocks.svg` is offered, and the extension picked (`.svg`
or `.pdf`) decides the format. Any other extension is refused with a notice
and nothing is written. The file is written off the update thread,
crash-safely, and the Messages panel says what was written.

With operands (`docs/CLI.md`, "Block diagram export") it writes
`document` to `path` and answers once the file is written. It needs the
`export` capability.

The diagram tab's toolbar (mockup row 171: SVG, and PDF…) opens a popover
with the format, colours, title block and file, and its Export runs this
command through the translator; the written file is reported as the
dialogs' export is ([BLOCK_DIAGRAM.md](BLOCK_DIAGRAM.md), "Export popover").

## Output Jobs

The export is a catalog command with operands, so an Output Job runs it as
one of its outputs (`export_block_diagram document=<diagram> path=<file>
kind=pdf`), the same way it runs the other exports. Output Job documents
(`.snxout`) have no content yet (mockup row 167, #2852); the Output Job
entry is this command until they do.
