## Board Reports (#2404), Board Print / PDF (#2402), the assembly drawings (#2486) and the board STEP export (#2488).

board-output-no-pcb = Board reports, board printing, assembly drawings and STEP export need an open PCB.
board-output-board-gone = { $board } is no longer open, so nothing was written.

board-reports-title = Board Reports
board-reports-subtitle = { $board }
board-reports-rows = { $count ->
    [one] 1 row
   *[other] { $count } rows
}
# The row count while the filter hides some rows.
board-reports-rows-filtered = { $shown } of { $count } rows
# Placeholder of the filter field (view only; Export CSV writes every row).
board-reports-filter = Filter rows
board-reports-empty = This report has no rows for the board.
# Shown when the filter hides every row.
board-reports-no-match = No row holds the filter text.
board-reports-export-csv = Export CSV…
board-reports-cancel = Cancel
board-reports-export-title = Export Board Report
# Footer caption of each report; $unit is the length unit shown, mm or mil.
board-reports-caption-bom = Grouped by value and footprint
board-reports-caption-netlist = One row per net
board-reports-caption-net-length = { $unit ->
    [mil] Lengths in mil · routed copper only
   *[mm] Lengths in mm · routed copper only
}
board-reports-caption-components = { $unit ->
    [mil] Positions in mil · rotation in degrees
   *[mm] Positions in mm · rotation in degrees
}

board-report-kind-bom = Bill of Materials
board-report-kind-netlist = Netlist
board-report-kind-net-length = Net Lengths
board-report-kind-components = Components

board-report-col-designators = Designators
board-report-col-quantity = Qty
board-report-col-value = Value
board-report-col-footprint = Footprint
board-report-col-description = Description
board-report-col-type = Type
board-report-col-net = Net
board-report-col-node-count = Pads
board-report-col-nodes = Pads on the Net
board-report-col-length = Routed Length
board-report-col-tracks = Tracks
board-report-col-vias = Vias
board-report-col-designator = Designator
board-report-col-x = X
board-report-col-y = Y
board-report-col-rotation = Rotation
board-report-col-side = Side
board-report-side-top = Top
board-report-side-bottom = Bottom
board-report-value = { $value }
board-report-degrees = { $value }°
board-report-missing = —

board-print-title = Board Print / PDF
board-print-subtitle = { $board }
board-print-sets = Layer Sets
# How many of the six layer sets are ticked.
board-print-sets-count = { $ticked } of { $total }
board-print-set-top = Top Side
board-print-set-bottom = Bottom Side
board-print-set-inner = Inner Copper
board-print-set-mechanical = Outline and Mechanical
board-print-set-assembly-top = Assembly Drawing, Top
board-print-set-assembly-bottom = Assembly Drawing, Bottom
board-print-layout = Layout
board-print-pages = Pages
board-print-pages-per-set = One Page per Set
board-print-pages-composite = All Sets on One Page
board-print-scale = Scale
board-print-scale-fit = Fit to Page
board-print-scale-one = 1:1
board-print-ink = Ink
board-print-ink-colour = Colour
board-print-ink-mono = Black
board-print-show = Show
board-print-mirror-bottom = Mirror the Bottom Side
# Hover note of Mirror the Bottom Side while no bottom set is ticked.
board-print-mirror-needs-bottom = Tick a bottom set to mirror it
board-print-outline = Board Outline
board-print-drill-marks = Drill Holes
board-print-title-block = Title Block
board-print-page-count = { $count ->
    [one] 1 page
   *[other] { $count } pages
}
board-print-no-sets = Pick at least one layer set to print.
# The page plan: one card per page the options print.
board-print-page-plan = Page plan
board-print-no-sets-hint = Tick a set on the left; each set becomes a page.
# The card's page number.
board-print-page-number = { $number }
# The card's title: the set names of the page, joined.
board-print-page-sets = { $sets }
# The card's layer line: the page's layers, joined.
board-print-page-layers = { $layers }
# The last card while some sets are not ticked.
board-print-add-set = + tick a set
board-print-not-printed = { $sets }: not printed
board-print-export = Export PDF…
board-print-export-svg = Export Assembly SVG…
board-print-svg-folder = Folder for the Assembly Drawings
board-print-cancel = Cancel
board-print-mirrored = mirrored
board-print-failed = The board PDF could not be made: { $why }
board-step-export-title = Export STEP 3D Model
board-step-failed = The board STEP model could not be made: { $why }
