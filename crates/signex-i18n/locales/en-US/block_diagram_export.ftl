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

# The block diagram tab's SVG and PDF… buttons and their popover (mockup row
# 171, "Export PDF…"). The popover exports the saved file.
block-diagram-export-svg = SVG
block-diagram-export-svg-tip = Export the saved diagram as an SVG file
block-diagram-export-pdf = PDF…
block-diagram-export-pdf-tip = Export the saved diagram as a PDF page
block-diagram-export-pdf-format = PDF
block-diagram-export-heading = Export the saved diagram
block-diagram-export-format = Format
block-diagram-export-colours = Colors
block-diagram-export-colours-print = Print
block-diagram-export-colours-shown = As shown
block-diagram-export-paper = Paper
block-diagram-export-paper-a4 = A4
block-diagram-export-page = Page
block-diagram-export-title-block = Title block
block-diagram-export-file = File
block-diagram-export-saved-only = The saved file is exported; save first to include unsaved edits.
block-diagram-export-run = Export
block-diagram-export-pages = Pages
block-diagram-export-page-blocks = Block diagram
block-diagram-export-page-power = Power tree
block-diagram-export-page-budget = Budget table
block-diagram-export-paper-a3 = A3
block-diagram-export-paper-a2 = A2
block-diagram-export-paper-letter = Letter
block-diagram-export-paper-tabloid = Tabloid
block-diagram-export-landscape = Landscape
block-diagram-export-portrait = Portrait

# The Power tree and Budget table pages' columns and the role of each row.
block-diagram-export-col-item = Item
block-diagram-export-col-role = Role
block-diagram-export-col-voltage = Voltage
block-diagram-export-col-current = Current
block-diagram-export-col-limit = Limit
block-diagram-export-col-share = Share of rail
block-diagram-export-col-rail = Rail
block-diagram-export-col-source = Source
block-diagram-export-col-load = Load
block-diagram-export-col-headroom = Headroom
block-diagram-export-role-rail = Rail
block-diagram-export-role-source = Source
block-diagram-export-role-converter = Converter
block-diagram-export-role-load = Load
block-diagram-export-not-known = —

# Why the pages or paper asked for were refused. $page, $paper,
# $orientation and $variant are as given; $document is the diagram.
block-diagram-export-bad-page = `{ $page }` is not a page: use blocks, power_tree or budget.
block-diagram-export-no-pages = Name at least one page with `pages`.
block-diagram-export-svg-pages = An SVG holds the block diagram only; export a PDF for the power tree or budget pages.
block-diagram-export-bad-paper = `{ $paper }` is not a paper size: use a4, a3, a2, letter or tabloid.
block-diagram-export-bad-orientation = `{ $orientation }` is not an orientation: use landscape or portrait.
block-diagram-export-no-project = { $document } is not in the folder of an open project, so it has no power tree to print.
block-diagram-export-bad-variant = `{ $variant }` is not a variant of the project.
