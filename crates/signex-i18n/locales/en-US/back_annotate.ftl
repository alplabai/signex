# Back Annotate (#1312): Tools > Annotation > Back Annotate Schematics and
# Design > Update Schematics. Designators, values and footprint names are
# shown as they are.

back-annotate-title = Back Annotate Schematics
# Under the title. $board is the board file, $project the project file.
back-annotate-subtitle = From { $board } to the schematics of { $project }
back-annotate-intro = Carries the board's designators and values to the schematic. Ticked rows are applied as one undo step; a sheet that is not open is changed in memory and marked unsaved. Nothing is saved until you save.
# The Design menu row on a board; $project is the project file.
back-annotate-menu-update-in = Update Schematics in { $project }
# The same row when no project is known for the document in view.
back-annotate-menu-update = Update Schematics
back-annotate-not-in-project = The document in view is not part of a project.
back-annotate-no-board = This project has no board to back-annotate from.
# $board is the board file.
back-annotate-opening = Opening { $board }…
back-annotate-computing = Comparing the board with the schematic…
# $why is the reason.
back-annotate-failed = The design could not be read: { $why }
back-annotate-nothing = The schematic already matches the board.
back-annotate-summary = { $ticked } of { $total } changes ticked
back-annotate-group-designators = Designators
back-annotate-group-values = Values
back-annotate-group-footprints = Footprints
# Next to a group's checkbox.
back-annotate-group-count = { $on } of { $open }
back-annotate-row-designator = Rename { $from } to { $to }
back-annotate-row-value = { $reference }: value { $from } to { $to }
back-annotate-row-footprint = { $reference }: the board has { $board }, the schematic { $schematic }
back-annotate-block-needs-annotation = The schematic part is not annotated; annotate it first.
# $designator is the shared schematic designator.
back-annotate-block-duplicate = Another schematic part is also { $designator }.
back-annotate-block-board-not-annotated = The board designator is not annotated.
back-annotate-block-shared-sheet = This sheet is placed more than once, so a value would change every placement; it is not carried back.
back-annotate-block-footprint = Changing a part's footprint from the board is not supported yet (#2248).
back-annotate-notes-heading = Notes
back-annotate-note-unreadable = A sheet could not be read; the parts on it are left alone.
back-annotate-note-unlinked = { $reference } is not linked to a schematic part.
back-annotate-note-part-gone = { $reference } is linked to a part that is no longer in the schematic.
back-annotate-note-part-not-read = { $designator } could not be read back and is left alone.
back-annotate-note-shared = After this, { $designator } is on { $count } parts.
# $reference is a second board footprint linked to the part $designator.
back-annotate-note-extra-link = { $reference } is linked to { $designator } too; only the first footprint linked to a part is carried back.
back-annotate-stale = The board or a sheet changed since the list was made. Refresh it.
back-annotate-refresh = Refresh
back-annotate-valid = Every ticked change applies.
back-annotate-checking = Checking the sheets that are not open…
back-annotate-close = Close
back-annotate-validate = Validate
back-annotate-execute = Execute
# $path is a sheet file.
back-annotate-disk-changed = { $path } changed on disk since the list was made. Refresh it.
back-annotate-busy = { $path } is being saved or moved.
back-annotate-read-only = { $path } opened read-only (a newer format), so it cannot be changed.
back-annotate-not-loaded = { $path } could not be loaded.
back-annotate-not-changed = { $path } refused the change.
back-annotate-refused = { $path } refused the change: { $why }
back-annotate-no-file = A part's sheet could not be found. Refresh the list.
# Messages panel line after Execute.
back-annotate-done = Back Annotate: { $count ->
    [one] 1 change
   *[other] { $count } changes
} on { $sheets ->
    [one] 1 sheet
   *[other] { $sheets } sheets
}; save the sheets to keep them.
