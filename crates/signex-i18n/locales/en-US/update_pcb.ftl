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
update-pcb-summary = { $ticked } of { $total } changes ticked
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

# Group headings and their tick count.
update-pcb-group-add-footprints = Add footprints
update-pcb-group-remove-footprints = Remove footprints
update-pcb-group-change-footprints = Change footprints
update-pcb-group-designators = Designators
update-pcb-group-values = Values
update-pcb-group-links = Links
update-pcb-group-nets = Nets
update-pcb-group-pad-nets = Pad nets
update-pcb-group-count = { $on } of { $open }

# One row each.
update-pcb-row-add = Add { $designator } ({ $value }) { $footprint }
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
update-pcb-fix-cross-probe = Cross Probe

# Notes under the rows.
update-pcb-notes-heading = Notes
update-pcb-note-unreadable-sheet = A sheet could not be read; nothing linked under it is removed.
update-pcb-note-pad-loses-lock = { $designator }.{ $pad } is locked and not on the new footprint; its lock goes with it.
update-pcb-note-pad-loses-net = { $designator }.{ $pad } is on net { $net } and not on the new footprint; the pad and its net go.
update-pcb-note-part-not-read = Part { $designator } could not be read back from its sheet; its footprint is left as it is.
update-pcb-note-footprint-unresolved = The footprint of { $designator } could not be resolved ({ $why }); its pads keep their nets.
update-pcb-note-kept-unlinked = { $designator } is not linked to a part and is kept.
update-pcb-note-extra-link = { $reference } is linked to { $designator } too, which another footprint holds; removing it is listed but not ticked.
update-pcb-note-net ={ $text }

# Validate, and the footer buttons.
update-pcb-valid = Validate: every ticked change applies to the board.
update-pcb-invalid = Validate: { $why }
update-pcb-close = Close
update-pcb-report = Report...
update-pcb-validate = Validate
update-pcb-execute = Execute

# Messages panel lines.
update-pcb-done = Update PCB: { $count ->
        [one] 1 change applied to { $board }.
       *[other] { $count } changes applied to { $board }.
    }
update-pcb-refused = Update PCB did not run: { $why }
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
