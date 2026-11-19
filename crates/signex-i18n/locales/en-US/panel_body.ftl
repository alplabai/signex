# Text inside the panels: History, ERC waivers, Projects, multi-selection,
# Components and the planned panels (#2059).

## History panel
panels-history-no-active-file = Open a schematic, PCB, library, or primitive to see its history.
panels-history-no-repo = This file isn't under version control. Enable from the project's right-click menu.
panels-history-loading = Loading history…
# $reason is why reading the history failed, as the version-control tool
# words it.
panels-history-read-failed = History could not be read: { $reason }
panels-history-restore = Restore this version
# Shown on the restore button when the document has unsaved edits.
panels-history-discard-and-restore = Discard unsaved edits and restore
panels-history-cancel = Cancel
# A pseudo-entry above the commits while the document has unsaved changes.
panels-history-working-tree = Working tree
panels-history-uncommitted = Uncommitted changes
# How long ago a commit was made; $count is a whole number. The unit is
# abbreviated: s seconds, m minutes, h hours, d days, mo months, y years.
panels-history-ago-seconds = { $count }s ago
panels-history-ago-minutes = { $count }m ago
panels-history-ago-hours = { $count }h ago
panels-history-ago-days = { $count }d ago
panels-history-ago-months = { $count }mo ago
panels-history-ago-years = { $count }y ago

## ERC panel: per-finding waivers
panels-erc-waive = Waive
panels-erc-waiver-cancel = Cancel
# $count is how many findings the stored waivers hide, a whole number. The
# English keeps "findings" for every count.
panels-erc-findings-waived = { $count } findings waived
# The status of one stored waiver.
panels-erc-waiver-status-one = waived
# $count is a whole number above one.
panels-erc-waiver-status-many = waives { $count } findings
panels-erc-waiver-status-lapsed = LAPSED: an involved object changed
panels-erc-waiver-status-unmatched = matches no finding
panels-erc-waiver-status-incomplete = ignored: no reason or author
# One stored waiver, on two lines: $rule the rule id, $message the finding,
# $status one of the statuses above; then $reason, $author and $date as the
# user wrote them. Keep the double spaces and the indent of the second line.
panels-erc-violation-waiver-row =
    { $rule }  { $message }  ({ $status })
    {"  "}Reason: { $reason }  by { $author } { $date }

## Properties of a multi-selection
panels-multi-title = Multi-Selection
# $count is a whole number. The English keeps "objects" for every count.
panels-multi-objects-selected = { $count } objects selected
# Group headings; $count is how many objects of the kind are selected.
panels-multi-symbols = Symbols ({ $count })
panels-multi-labels = Labels ({ $count })
panels-multi-wires = Wires ({ $count })
panels-multi-value = Value
panels-multi-footprint = Footprint
# Do not place: the part is left off the board.
panels-multi-dnp = DNP
panels-multi-in-bom = In BOM
panels-multi-on-board = On board
# Label text size, in points.
panels-multi-size-pt = Size (pt)
panels-multi-width-mm = Width (mm)
# The state of a flag every selected object shares.
panels-multi-on = On
panels-multi-off = Off
panels-multi-justify = Justify
# The selected labels are justified differently.
panels-multi-justify-mixed = Justify <mixed>
panels-multi-justify-left = Left
panels-multi-justify-center = Center
panels-multi-justify-right = Right
panels-multi-rotation = Rotation
# The selected labels are turned differently.
panels-multi-rotation-mixed = Rotation <mixed>
panels-multi-nothing-shared = Nothing in this selection has fields shared for editing

## Projects and Navigator panels
panels-projects-none-open = No project open
# The menu path that opens a project.
panels-projects-open-hint = File > Open to begin
# Branches of a project in the Projects tree.
panels-projects-group-schematics = Schematics
panels-projects-group-pcb = PCB
panels-projects-group-bom = Bill of Materials
panels-projects-group-outputs = Output Jobs
panels-projects-group-libraries = Libraries
panels-projects-group-settings = Settings
# A tree row whose file is gone from disk. $name is the file name.
panels-projects-file-missing = { $name }  (missing)
# A tree row whose sheet could not be read. $name is the file name.
panels-projects-file-unreadable = { $name }  (unreadable)
panels-navigator-sheets = Sheets
panels-navigator-no-project = No project
# Navigator rows of a sheet that is not a plain page. $name is the sheet's name.
panels-navigator-missing = { $name } (missing)
panels-navigator-unreadable = { $name } (unreadable)
# The sheet was refused (it is not a sheet Signex can place).
panels-navigator-rejected = { $name } (rejected)
# The sheet places itself further down, so it is not opened again.
panels-navigator-cycle = { $name } (cycle)
# The hierarchy is too deep to show this sheet's children.
panels-navigator-not-expanded = { $name } (not expanded)

## Components panel: the details of the selected row
panels-components-details-select = Select a component to see its details.
panels-components-details-loading = Loading…
panels-components-details-no-symbol = No symbol.
panels-components-details-footprint-unreadable = Footprint could not be read.
panels-components-details-no-footprint = No footprint.
# Footprint pick-list entry of the row's own footprint. $name is its name.
panels-components-details-footprint-default = { $name } (default)
# Name of the row's footprint when it could not be read.
panels-components-details-footprint-unnamed = Default
panels-components-details-no-parameters = No parameters.

## Planned panels: what each will do, under its name and the planned note
panels-stub-variants = Design variant management
panels-stub-output-jobs = Manufacturing output config
panels-stub-bom-studio = Bill of Materials management
panels-stub-favorites = Favorite components and snippets
panels-stub-snippets = Reusable schematic snippets
panels-stub-todo = Task and issue tracking
panels-stub-wiki = Project documentation wiki
panels-stub-library = Open a library from the Projects panel to browse it.
