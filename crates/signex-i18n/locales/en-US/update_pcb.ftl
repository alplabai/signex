# Update PCB from schematic (#1312) and New PCB in a project.

# Design menu rows: the board a schematic updates, the project a board
# imports from, or neither when the window shows no project document.
update-pcb-menu-update = Update PCB Document
update-pcb-menu-update-board = Update PCB Document { $board }
update-pcb-menu-import = Import Changes From { $project }
# Projects tree, project row.
update-pcb-tree-row = Update PCB Document

# The dialog.
update-pcb-title = Update PCB from Schematic
update-pcb-subtitle = { $project } to { $board }
update-pcb-computing = Comparing the schematic with the board...
update-pcb-opening = Opening { $board }...
update-pcb-nothing = The board is in line with the schematic. Nothing to change.
update-pcb-failed = The schematic could not be compared with the board: { $why }
update-pcb-stale = The schematic or the board changed after this list was made.
update-pcb-refresh = Refresh
update-pcb-parked = { $board } is closed with unsaved edits. Reopen it to update it.
update-pcb-reopen = Reopen
update-pcb-read-only = { $board } opened read-only (a newer format), so it cannot be updated.
update-pcb-no-board = This project has no board yet.
update-pcb-create-pcb = Create PCB
update-pcb-board-missing = The board { $board } could not be opened.
update-pcb-no-schematic = This project has no schematic to update the board from.
update-pcb-not-in-project = This document is not part of a project.

# Summary tiles (the count is drawn above the word): click one to list only
# its rows. The Add, Change and Remove tiles use the action words below.
update-pcb-tile-problems = Problems
update-pcb-tile-hint = Click a tile to list only those rows.
update-pcb-filter-placeholder = Filter designator or net
update-pcb-clear-filter = Clear filter
update-pcb-filtered-empty = No row matches the filter.

# The Problems card above the list. $count blocked rows hold back $parts parts.
update-pcb-problems-heading = { $count ->
        [one] 1 problem holds
       *[other] { $count } problems hold
    } back { $parts ->
        [one] 1 part
       *[other] { $parts } parts
    }
update-pcb-problems-hint = Fix them here, or execute without them

# The list: action pills, groups and their tick count.
update-pcb-action-add = Add
update-pcb-action-change = Change
update-pcb-action-remove = Remove
update-pcb-table-components = Components
update-pcb-table-nets = Nets
update-pcb-table-pad-nets = Pad nets
update-pcb-fold-open = ▾
update-pcb-fold-closed = ▸
update-pcb-group-count = { $on } of { $open }

# A row's Detail column.
update-pcb-detail-from-to = { $from } → { $to }
update-pcb-detail-other-version = { $from } → { $to } (another library version)
update-pcb-detail-locked = { $from } → { $to } (locked)
update-pcb-detail-designator = Designator { $from } → { $to }
update-pcb-detail-value = Value { $from } → { $to }
update-pcb-detail-link = Link to its part
update-pcb-detail-net = Net { $net }

# A row's status icon (its words are the tooltip); a problem in short, as
# the Problems card's reason pill.
update-pcb-status-ok = ✓
update-pcb-status-ok-tooltip = Validate: this change applies to the board.
update-pcb-problem-no-footprint = No footprint
update-pcb-problem-not-annotated = Not annotated
update-pcb-problem-duplicate = Duplicate { $designator }
update-pcb-problem-pads = Pads would lose a net
update-pcb-problem-unplaceable = Cannot be placed
update-pcb-problem-not-read = Not read back

# A whole row, as its tooltip.
update-pcb-row-add = Add { $designator } { $footprint }
update-pcb-row-remove = Remove { $designator }
update-pcb-row-change = Change { $designator }: { $from } to { $to }
update-pcb-row-change-version = Change { $designator }: { $from } to { $to } (another library version)
update-pcb-row-designator = Rename { $from } to { $to }
update-pcb-row-value = Value of { $designator }: { $from } to { $to }
update-pcb-row-link = Link { $designator } to its part
update-pcb-row-add-net = Add net { $net }
update-pcb-row-remove-net = Remove net { $net }
update-pcb-row-rename-net = Rename net { $from } to { $to }
update-pcb-row-pad-net = Pad { $designator }.{ $pad }: { $from } to { $to }
update-pcb-row-pad-net-locked = Pad { $designator }.{ $pad }: { $from } to { $to } (locked)
update-pcb-no-net = (no net)
update-pcb-row-skipped = Skipped: a row it needs is not ticked.

# Why a row cannot be ticked, and the links that fix it.
update-pcb-blocked-missing-footprint = No footprint: { $why }
update-pcb-blocked-annotate = Not annotated.
update-pcb-blocked-duplicate = Designator { $designator } is used twice.
update-pcb-blocked-pads = Fewer pads numbered { $pad }; a net or a lock would be lost.
update-pcb-blocked-unplaceable = Cannot be placed: { $why }
update-pcb-blocked-not-read = The part could not be read back from its sheet.
update-pcb-fix-annotate = Annotate...
update-pcb-fix-cross-probe = Cross Probe: show the part on its sheet
update-pcb-fix-cross-probe-icon = ⌖
update-pcb-fix-choose-footprint = Choose footprint...
update-pcb-fix-sheet-closed = Update PCB: the sheet holding the part is not open, so its footprint was not set. Open the sheet and choose again.
update-pcb-fix-not-set = Update PCB: the part's footprint was not set (the part is locked or the sheet is read-only).

# Notes under the rows.
update-pcb-notes-heading = Notes
update-pcb-note-unreadable-sheet = A sheet could not be read; nothing linked under it is removed.
update-pcb-note-pad-loses-lock = { $designator }.{ $pad } is locked and not on the new footprint; its lock goes with it.
update-pcb-note-pad-loses-net = { $designator }.{ $pad } is on net { $net } and not on the new footprint; the pad and its net go.
update-pcb-note-part-not-read = Part { $designator } could not be read back from its sheet; its footprint is left as it is.
update-pcb-note-footprint-unresolved = The footprint of { $designator } could not be resolved ({ $why }); its pads keep their nets.
update-pcb-note-kept-unlinked = { $designator } is not linked to a part and is kept.
update-pcb-note-extra-link = { $reference } is linked to { $designator } too, which another footprint holds; removing it is listed but not ticked.
update-pcb-note-net = { $text }

# Validate, and the footer buttons.
update-pcb-valid = Validate: every ticked change applies to the board.
update-pcb-invalid = Validate: { $why }
update-pcb-close = Close
update-pcb-report = Report...
update-pcb-validate = Validate
update-pcb-execute = Execute
update-pcb-execute-count = { $count ->
        [one] Execute 1 change
       *[other] Execute { $count } changes
    }

# Messages panel lines.
update-pcb-done = Update PCB: { $count ->
        [one] 1 change applied to { $board }.
       *[other] { $count } changes applied to { $board }.
    }
update-pcb-refused = Update PCB did not run: { $why }
# The command run from operands (#2470): it needs one of the two forms.
update-pcb-command-mode = `update_pcb_from_schematic` lists its changes in the Update PCB dialog; run it with `apply=true` to execute the default ticks, or with `dry_run=true` to list them and change nothing
# The dialog is open, or another run from operands has not finished.
update-pcb-busy = Update PCB is already open or running; close its dialog or wait for the run to finish
# Run with no `document` when the active project has no board ($case
# `active-has-none`; $boards: the other open projects' boards) or no project
# is active among several ($case `no-active`; $boards: every open project's
# board). $boards lists their paths, comma-separated.
update-pcb-which-board = `update_pcb_from_schematic` needs `document=` to choose a board: { $case ->
    [active-has-none] the active project has no board
   *[no-active] no project is active
}; the boards of the open projects are { $boards }
update-pcb-library-not-opened = Update PCB: library { $library } could not be opened ({ $why }); its parts have no footprint.
update-pcb-report-saved = Update PCB report saved to { $path }.
update-pcb-report-failed = The Update PCB report could not be written: { $why }

# New PCB in a project (File > New > PCB, Projects > Add New > PCB).
new-pcb-title = New PCB
new-pcb-has-board = The project already has a board, { $board }. A project holds one board.
new-pcb-exists = A file named { $file } is already in the project folder.
new-pcb-check-failed = Could not check whether { $file } is already in the project folder: { $why }
new-pcb-no-project = Open a project to add a PCB to it.
new-pcb-created = Created { $board } in { $project }. It is written when you save it.

## Update PCB: the rules the schematic's Net Parameters directives ask for (#1489).

update-pcb-schematic-rules = Rules from the schematic
# Execute rewrites a rule of that name edited in the Rules tab: the schematic wins.
update-pcb-rule-changed = differs · schematic wins
update-pcb-rule-added = new rule
update-pcb-rule-unchanged = up to date
# The rule as the Rules tab has it now. $text is a line of rule text.
update-pcb-rule-was = now: { $text }
update-pcb-rules-note = Execute writes these into the project's design rules, marked "from schematic" in the Rules tab.
# Messages: a Net Parameters directive with no class whose net is not a board net
# by that name (it sits on a placed or repeated sheet). $net is the sheet's net name.
update-pcb-rule-net-not-on-board = Update PCB: no rule for net { $net }: that name is not a board net (a net on a placed or repeated sheet takes another name on the board). Give its Net Parameters directive a net class to make a rule.
# Messages: two scopes give one rule name. $rule is the rule, $kept and $dropped the class or net names.
update-pcb-rule-name-clash = Update PCB: { $kept } and { $dropped } both make rule { $rule }; it is kept for { $kept } and nothing is made for { $dropped }. Rename one of them.
# The one summary line of the net classes Execute puts into the board's net
# classes from the Net Blanket and Net Parameters directives (exit criterion 3).
update-pcb-schematic-classes = { $count ->
    [one] 1 net class from the schematic
   *[other] { $count } net classes from the schematic
}

# The change-list rows and group headers Update PCB and Back Annotate share
# (#2533). Status icons; their words are the row's tooltip.
change-row-icon-skipped = –
change-row-icon-blocked = ⊘
change-row-icon-info = ⓘ
change-row-icon-warn = ⚠
change-row-status-ticked = Ticked: Execute applies this change.
change-row-status-unticked = Not ticked: Execute leaves this change out.
# A group header's split of its rows: added, changed and removed.
change-row-split-add = +{ $count }
change-row-split-change = ~{ $count }
change-row-split-remove = −{ $count }
change-row-split-separator = ·
# The Notes group's count.
change-row-notes-count = { $count }

# mockup rows 81 and 82
# The footer after a passing Validate, beside Report... (mockup row 81).
update-pcb-validated = Validated: every ticked change applies to the board.
