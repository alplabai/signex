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
# History graph rows. $author is the commit author's name; $age is one of the
# panels-history-ago-* texts.
panels-history-byline = { $author } · { $age }
# Caption of the working-tree row above the commits.
panels-history-working-tree-caption = Working tree · now
panels-history-commit = Commit…
# Pill on the commit the working tree is checked out at (the git term).
panels-history-head = HEAD
# Status letter of a changed file in the commit detail strip.
panels-history-file-modified = M
panels-history-file-added = A
panels-history-file-deleted = D
# Commit detail strip actions.
panels-history-show-differences = Show differences…
panels-history-show-on-board = Show on board / sheet
panels-history-more = More: new branch here, tag, copy SHA
panels-history-new-branch-here = New branch here…
panels-history-tag = Tag…
panels-history-copy-sha = Copy SHA
panels-history-branch-name = Branch name
panels-history-tag-name = Tag name
panels-history-create = Create
# Panel header: how many commits are listed and how many branches hold work
# the current branch lacks. $count is a whole number.
panels-history-commits-one = { $count } commit
panels-history-commits-many = { $count } commits
panels-history-branches-ahead-one = { $count } branch ahead
panels-history-branches-ahead-many = { $count } branches ahead
# $commits and $branches are the two texts above.
panels-history-counts = { $commits } · { $branches }
panels-history-refresh = Refresh history
# Filter chips and search.
panels-history-all-branches = All branches
panels-history-this-file = This file only
panels-history-search = Search commits
panels-history-search-placeholder = Subject, author or SHA
panels-history-no-matches = No commits match these filters.
# The branch chip when HEAD is on a commit and not on a branch.
panels-history-detached = detached
# Branch switcher.
panels-history-branch-switch = Switch branch
panels-history-branch-find = Find or create a branch
panels-history-branch-local = Local branches
panels-history-branch-remote = Remote
panels-history-branch-current = current
panels-history-branch-up-to-date = up to date
panels-history-branch-ahead = { $count } ahead
panels-history-branch-behind = { $count } behind
panels-history-branch-ahead-behind = { $ahead } ahead · { $behind } behind
# Error card when a switch was refused; nothing on disk changed.
panels-history-switch-failed = Branch not switched
# Error card when a new branch or tag could not be created.
panels-history-ref-failed = Branch or tag not created

## ERC panel: per-finding waivers
panels-erc-waive = Waive
panels-erc-waiver-cancel = Cancel
# The status of one stored waiver.
panels-erc-waiver-status-one = waived
# $count is a whole number above one.
panels-erc-waiver-status-many = waives { $count } findings
panels-erc-waiver-status-lapsed = LAPSED: an involved object changed
panels-erc-waiver-status-unmatched = matches no finding
panels-erc-waiver-status-incomplete = ignored: no reason

## Properties of a multi-selection
panels-multi-title = Multi-Selection
# $count is a whole number. The English keeps "objects" for every count.
panels-multi-objects-selected = { $count } objects selected
panels-multi-value = Value
panels-multi-footprint = Footprint
# Do not place: the part is left off the board.
panels-multi-dnp = DNP
panels-multi-in-bom = In BOM
panels-multi-on-board = On board
panels-multi-justify = Justify
panels-multi-justify-left = Left
panels-multi-justify-center = Center
panels-multi-justify-right = Right
panels-multi-rotation = Rotation

## Projects and Navigator panels
panels-projects-none-open = No project open
# The menu path that opens a project.
panels-projects-open-hint = File > Open to begin
# Branches of a project in the Projects tree.
panels-projects-group-schematics = Schematics
panels-projects-group-pcb = PCB
panels-projects-group-notebook = Notebook
panels-projects-group-bom = Bill of Materials
panels-projects-group-outputs = Export Jobs
panels-projects-group-libraries = Libraries
panels-projects-group-settings = Settings
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
# Navigator panel (Graphite mockup #54): the filter box, the no-project card and the hint.
panels-navigator-filter = Filter sheets
panels-navigator-no-project-body = Open a project to see its sheet hierarchy.
panels-navigator-open-project = Open Project…
panels-navigator-hint = Click highlights · double-click opens the sheet.
# Shown when no sheet name or file name contains the filter text.
panels-navigator-no-match = No sheet matches the filter.
# $filter is the filter text, $total a whole number.
panels-navigator-no-match-body = Nothing named { $filter } in { $total ->
        [one] { $total } sheet
       *[other] { $total } sheets
    }.
panels-navigator-clear-filter = Clear filter
# The sheet count beside the Sheets heading, and a row's sheet number. Whole numbers.
panels-navigator-sheet-count = { $count }
panels-navigator-sheet-number = { $number }

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
panels-stub-favorites = Favorite components
panels-stub-snippets = Reusable schematic snippets
panels-stub-todo = Task and issue tracking
panels-stub-wiki = Project documentation wiki
panels-stub-library = Open a library from the Projects panel to browse it.

# history-show-differences
## History panel: the lines a commit changed in one file (mockup row 39)
panels-history-diff-close = Close
panels-history-diff-loading = Reading the differences…
# $reason is why reading failed, as the version-control tool words it.
panels-history-diff-failed = Differences could not be read: { $reason }
# Lines added and removed; each count is a whole number.
panels-history-diff-counts = +{ $added } −{ $removed }
panels-history-diff-binary = Not a text file, so there are no lines to compare.
panels-history-diff-no-lines = This commit changed no lines of this file.
# $count is how many lines of the differences are listed.
panels-history-diff-truncated = Only the first { $count } lines are shown.
# Where a run of changes starts, in the file before and after the commit.
panels-history-diff-hunk = Line { $old } before, line { $new } after
# Marks in front of an added and a removed line.
panels-history-diff-added-sign = +
panels-history-diff-removed-sign = −
