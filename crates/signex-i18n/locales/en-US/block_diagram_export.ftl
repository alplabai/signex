# Block diagram export (#2796, mockup row 171): the SVG and PDF files of a
# `.snxblk` diagram, from the window or `export_block_diagram`.

# The title block's second line on the printed page.
block-diagram-export-page-label = Block diagram
block-diagram-export-open-title = Pick the Block Diagram to Export
block-diagram-export-save-title = Export Block Diagram
block-diagram-export-filter-diagram = Block Diagram (.snxblk)
block-diagram-export-filter-svg = SVG (.svg)
block-diagram-export-filter-pdf = PDF (.pdf)
# Messages panel, after the file is written. $file is the written file.
block-diagram-export-written = Wrote { $file }: { $blocks } blocks, { $connections } connections
# Why nothing was written. $document is the diagram as named, $file the
# file to write, $kind the format as given, $reason what went wrong.
block-diagram-export-no-document = Name the block diagram (.snxblk) with the operand `document`.
block-diagram-export-not-a-diagram = { $document } is not a block diagram (.snxblk).
block-diagram-export-no-path = Name the file to write with the operand `path`.
block-diagram-export-bad-kind = `{ $kind }` is not a format to export (svg or pdf).
block-diagram-export-unknown-format = { $file } does not end in .svg or .pdf: name the format with `kind`.
block-diagram-export-unknown-extension = { $file } does not end in .svg or .pdf: pick one of the two.
block-diagram-export-same-file = { $file } is the block diagram itself: pick another file to write.
block-diagram-export-failed = Could not export { $document }: { $reason }
