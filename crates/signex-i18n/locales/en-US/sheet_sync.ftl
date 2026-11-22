# Bottom-up hierarchy (#1276): Create Sheet Symbol From Sheet and the
# Synchronize Sheet Entries and Ports dialog. Port and entry names are not
# localised.

# Title of the file dialog that picks the sheet to make a sheet symbol for.
sheet-sync-pick-title = Create Sheet Symbol From Sheet

# Dialog title.
sheet-sync-title = Synchronize Sheet Entries and Ports

# Under the title. $symbol is the sheet symbol's name, $file its sheet file.
sheet-sync-subtitle = { $symbol } · { $file }

# Under the change table.
sheet-sync-intro = Apply is one undo step. A sheet that is not open is changed in memory and stays unsaved until you save it.

# Heading over the two direction cards.
sheet-sync-which-side = Which side wins

# The two directions of the sync, each with what it moves where. $symbol is
# the sheet symbol's name, $file its sheet file.
sheet-sync-to-symbol = Sheet to sheet symbol
sheet-sync-to-sheet = Sheet symbol to sheet
sheet-sync-to-symbol-line = Ports on { $file } become entries on { $symbol }.
sheet-sync-to-sheet-line = Entries on { $symbol } become ports on { $file }.

# The drawing. $name is the sheet symbol or the sheet file it shows.
sheet-sync-preview = Preview
sheet-sync-preview-after-caption = { $name } after Apply
sheet-sync-preview-before-caption = { $name } as it is now
sheet-sync-preview-after = After
sheet-sync-preview-before = Before
sheet-sync-preview-expand = Expand preview
sheet-sync-preview-collapse = Shrink preview
# Over a renamed entry: its current name.
sheet-sync-preview-was = was { $name }
# The legend's note on the entry shapes.
sheet-sync-legend-shapes = Input points in, Output points out, Bidirectional and Tri-state point both ways, Passive is square

# The change table. $name is the sheet symbol or the sheet file it changes.
sheet-sync-changes-to = Changes to { $name }
sheet-sync-change-count = { $count ->
    [one] 1 change
   *[other] { $count } changes
}
sheet-sync-column-change = Change
sheet-sync-column-entry = Entry
sheet-sync-column-io = I/O type

# The I/O type of an entry or port.
sheet-sync-io-input = Input
sheet-sync-io-output = Output
sheet-sync-io-bidirectional = Bidirectional
sheet-sync-io-tri-state = Tri-state
sheet-sync-io-passive = Passive

# Apply with how many changes it makes.
sheet-sync-apply-count = Apply { $count }

# The Change pill of a row, and the legend's names.
sheet-sync-added = Added
sheet-sync-removed = Removed
sheet-sync-renamed = Renamed

# One renamed entry or port: its old name, then its new one.
sheet-sync-rename-row = { $from } → { $to }

sheet-sync-none = The sheet symbol and its sheet already agree.

# While the sheet is read.
sheet-sync-reading = Reading the sheet…

# The read failed; $why is the reason.
sheet-sync-err-read = The sheet could not be read: { $why }

sheet-sync-err-stale = The sheet, the sheet symbol or the open sheets changed. Close this dialog and open it again.

sheet-sync-err-changed = The sheets changed since the preview, which now shows the current changes. Check them, then Apply again.

# $path is the sheet file.
sheet-sync-err-busy = { $path } is being saved or moved. Try again when that is done.
sheet-sync-err-read-only = { $path } was written by a newer Signex and opens read-only.
sheet-sync-err-not-loaded = { $path } could not be loaded.
sheet-sync-err-not-changed = { $path } refused the change. The Messages panel says why.
sheet-sync-err-disk-changed = { $path } changed on disk after it was read. Close this dialog and open it again.

sheet-sync-cancel = Cancel
sheet-sync-apply = Apply
