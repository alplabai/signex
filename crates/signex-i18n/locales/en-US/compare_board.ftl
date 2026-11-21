# Compare Board with Schematic (#2462): Project ▸ Compare Board with
# Schematic…, the board right-click menu and the palette.

compare-board-title = Compare Board with Schematic
# Subtitle: the root sheet, the board and when they were compared (local time).
compare-board-subtitle = { $schematic } ↔ { $board } · compared { $time }
# Subtitle while the first comparison is read.
compare-board-subtitle-reading = { $schematic } ↔ { $board }
compare-board-again = Compare again

# Kind chips (filter the rows); the count follows each.
compare-board-all = All
compare-board-kind-missing = Missing on board
compare-board-kind-extra = Extra on board
compare-board-kind-nets = Net differences
compare-board-kind-footprints = Footprint mismatches
compare-board-kind-values = Value changes
# Group rows of the table, in capitals.
compare-board-group-missing = MISSING ON BOARD
compare-board-group-extra = EXTRA ON BOARD
compare-board-group-nets = NET DIFFERENCES
compare-board-group-footprints = FOOTPRINT MISMATCHES
compare-board-group-values = VALUE CHANGES
compare-board-group-notes = NOTES

# Before the segmented control that sets every shown row.
compare-board-set-shown = Set shown rows:
compare-board-update-pcb = Update PCB
compare-board-update-schematic = Update schematic
compare-board-skip = Skip

compare-board-column-object = Object
compare-board-column-schematic = Schematic
compare-board-column-board = Board
compare-board-column-jump = Jump
compare-board-column-action = Action

compare-board-not-in-schematic = not in schematic
compare-board-not-on-board = not on board
# A pad on no net.
compare-board-no-net = no net
# A part: its value and footprint.
compare-board-part = { $value } · { $footprint }
# One pin of a part.
compare-board-pin = { $part } pin { $pin }
# Jump links.
compare-board-jump-schematic = Schematic
compare-board-jump-board = Board

# What the chosen direction does to a row.
compare-board-action-add-to-board = Add to board
compare-board-action-link-footprint = Link footprint
compare-board-action-remove-from-board = Remove from board
compare-board-action-add-net = Add net
compare-board-action-remove-net = Remove net
compare-board-action-rename-net = Rename net
compare-board-action-move-pin = Move pin to net
compare-board-action-connect-pin = Connect pin
compare-board-action-disconnect-pin = Disconnect pin
compare-board-action-swap-footprint = Swap footprint
compare-board-action-take-footprint = Take board footprint
compare-board-action-set-value = Set value
compare-board-action-take-value = Take board value
compare-board-action-set-designator = Set designator
compare-board-action-take-designator = Take board designator
compare-board-action-skip = No change
# An Update PCB row that needs another row the user skipped.
compare-board-action-waits = Skipped: a row it needs is skipped
# Tooltips of a direction that cannot be chosen.
compare-board-unavailable-no-board-change = Update PCB has nothing to change for this row.
compare-board-unavailable-schematic-later = Taking this difference into the schematic is not available yet.
compare-board-unavailable-footprint-later = Taking the board's footprint into the schematic is not available yet.

compare-board-export = Export report…
# Title of the save dialog.
compare-board-export-title = Export the comparison report
# Footer caption: how many changes go each way.
compare-board-footer = { $to_pcb } to PCB · { $to_schematic } to schematic
compare-board-footer-skipped = { $to_pcb } to PCB · { $to_schematic } to schematic · { $skipped } skipped
compare-board-footer-shown = { $shown } of { $total } shown · { $to_pcb } to PCB · { $to_schematic } to schematic
compare-board-footer-shown-skipped = { $shown } of { $total } shown · { $to_pcb } to PCB · { $to_schematic } to schematic · { $skipped } skipped
compare-board-close = Close
compare-board-apply = Apply { $count ->
    [one] 1 change
   *[other] { $count } changes
}

compare-board-in-sync-title = Board and schematic match
compare-board-in-sync-body = { $parts } parts, { $nets } nets and { $footprints } footprints compared. No differences.
compare-board-reading = Comparing the board with the schematic…
compare-board-filtered-empty = No rows of this kind.
compare-board-stale = The board or a sheet changed since this comparison. Compare again.

# The confirmation before Apply when a chosen row deletes from the board.
compare-board-confirm-title = Apply { $count ->
    [one] 1 change
   *[other] { $count } changes
}?
compare-board-confirm-removes = Removes { $object } from the board
compare-board-confirm-removes-net = Removes net { $net } from the board
compare-board-confirm-cancel = Cancel
compare-board-confirm-apply = Apply

compare-board-not-in-project = The document in view is not part of a project.
compare-board-no-board = This project has no board to compare with.
compare-board-no-schematic = This project has no schematic to compare with.
compare-board-project-closed = The project is no longer open.
# Messages panel lines.
compare-board-failed = Compare Board with Schematic could not compare: { $why }
compare-board-refused = Compare Board with Schematic: nothing more was changed: { $why }
compare-board-disk-changed = { $path } changed on disk since the comparison. Compare again.
compare-board-schematic-refused = The board was updated, but the schematic was not: { $why }
compare-board-done = Compare Board with Schematic: { $pcb } changes to { $board }, { $schematic } to the schematic.
compare-board-library-not-opened = Compare Board with Schematic: library { $library } could not be opened ({ $why }); its parts have no footprint.
compare-board-report-saved = Comparison report saved to { $path }.
compare-board-report-failed = The comparison report could not be written: { $why }
