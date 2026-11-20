# Bottom-up hierarchy (#1276): Create Sheet Symbol From Sheet and the
# Synchronize Sheet Entries and Ports dialog. Port and entry names are not
# localised.

# Title of the file dialog that picks the sheet to make a sheet symbol for.
sheet-sync-pick-title = Create Sheet Symbol From Sheet

# Dialog title.
sheet-sync-title = Synchronize Sheet Entries and Ports

# Under the title. $symbol is the sheet symbol's name, $file its sheet file.
sheet-sync-subtitle = { $symbol } · { $file }

sheet-sync-intro = Choose which side wins and check the changes, then Apply. Apply is one undo step. A sheet that is not open is changed in memory and stays unsaved until you save it.

# The two directions of the sync.
sheet-sync-to-symbol = Sheet to sheet symbol
sheet-sync-to-sheet = Sheet symbol to sheet

# Headings of the change lists; $count is how many names each holds.
sheet-sync-added = Added ({ $count })
sheet-sync-removed = Removed ({ $count })
sheet-sync-renamed = Renamed ({ $count })

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
