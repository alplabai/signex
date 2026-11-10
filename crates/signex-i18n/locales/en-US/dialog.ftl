# Dialogs.

# Autosave recovery card: how long ago the recovered copy was written.
# $age is an already-worded duration, such as "5 minutes ago".
dialog-autosave-recovery-autosaved-age = autosaved { $age }

## Tools ▸ Assign Footprints (#2248): the project's placed parts and the
## footprints their library rows offer.

dialog-assign-footprints-title = Assign Footprints
# $count is how many parts are listed (a whole number).
dialog-assign-footprints-subtitle =
    { $count ->
        [one] { $count } part
       *[other] { $count } parts
    }
dialog-assign-footprints-intro = Each part can take only the footprints its own library row offers. Tick several parts to set them at once. Apply changes the open sheets as one undo step; nothing is saved until you save.
dialog-assign-footprints-reading = Reading the project's sheets…
# $why is the system's reason, in English.
dialog-assign-footprints-read-failed = The sheets could not be read: { $why }
dialog-assign-footprints-no-parts = The project has no placed parts.
dialog-assign-footprints-col-designator = Designator
dialog-assign-footprints-col-value = Value
# The library row (part number) the part was placed from.
dialog-assign-footprints-col-row = Library row
dialog-assign-footprints-col-footprint = Footprint
# Label before the footprint list that sets every ticked part.
dialog-assign-footprints-set-ticked = Set ticked parts to
# Shown when the ticked parts' rows offer no footprint in common.
dialog-assign-footprints-no-shared = The ticked parts' rows offer no footprint in common.
# Link that ticks every part placed from the same library row.
dialog-assign-footprints-same-row = Tick all from this row
# Library row cell of a part with no library row.
dialog-assign-footprints-no-row = —
# Why a part cannot be changed here.
dialog-assign-footprints-sheet-closed = Sheet not open
dialog-assign-footprints-read-only = Read-only sheet
dialog-assign-footprints-row-not-open = Library row not open
dialog-assign-footprints-no-alternates = No alternates
# Note under the table when some parts sit on sheets that are not open.
dialog-assign-footprints-closed-note = Parts on sheets that are not open are listed only; open a sheet to change its parts.
# $count is how many parts Apply changes (a whole number).
dialog-assign-footprints-changes =
    { $count ->
        [one] { $count } part changes
       *[other] { $count } parts change
    }
# Why Apply changed nothing. $path is a sheet file; $part is a designator.
dialog-assign-footprints-err-closed = { $path } was closed after the list was read. Nothing was changed; open the dialog again.
dialog-assign-footprints-err-read-only = { $path } is read-only (written by a newer Signex). Nothing was changed.
dialog-assign-footprints-err-refused = { $part } can no longer take the picked footprint. Nothing was changed; open the dialog again.
dialog-assign-footprints-err-not-changed = { $path } refused the change. Nothing was changed.
dialog-assign-footprints-err-stale = The footprint of { $part } changed after the list was read. Nothing was changed; open the dialog again.
dialog-assign-footprints-cancel = Cancel
dialog-assign-footprints-apply = Apply
