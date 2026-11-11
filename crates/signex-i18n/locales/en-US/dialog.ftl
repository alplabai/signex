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

dialog-annotate-matching-unavailable = Parameter matching is not available yet.
dialog-annotate-sheet-options-unavailable = Start and Add Suffix cannot be changed yet.

## Graphite views (#2225)
dialog-annotate-current-sheet = Current Sheet
dialog-annotate-order-heading = Order of Processing
dialog-annotate-order-up-then-across = Up Then Across
dialog-annotate-order-across-then-down = Across Then Down
dialog-annotate-order-down-then-across = Down Then Across
dialog-annotate-order-across-then-up = Across Then Up
dialog-annotate-process-location = Process Location of
dialog-annotate-process-location-designator = Designator
dialog-annotate-replace-sub-parts = Replace Sub-Parts
dialog-annotate-replace-sub-parts-off = Off
dialog-annotate-col-component-parameter = Component Parameter
dialog-annotate-col-strictly = Strictly
dialog-annotate-no-custom-parameters = No custom parameters on any symbol yet.
dialog-annotate-matching-heading = Matching Options
dialog-annotate-complete-packages = Complete Existing Packages
dialog-annotate-complete-packages-none = None
dialog-annotate-sheets-heading = Schematic Sheets To Annotate
dialog-annotate-sheets-empty = No schematic tab open.
dialog-annotate-sheets-col-sheet = Schematic Sheet
dialog-annotate-sheets-col-start = Start
dialog-annotate-sheets-col-suffix = Add Suffix
dialog-annotate-changes-heading = Proposed Change List
dialog-annotate-col-lock = Lock
dialog-annotate-col-current = Current
dialog-annotate-col-proposed = Proposed
dialog-annotate-col-location = Location
dialog-annotate-loading = Loading project sheets…
dialog-annotate-load-failed = Could not load the project's sheets - see the Messages panel.
dialog-annotate-nothing-to-number = No symbols need annotation — every reference has a number.
dialog-annotate-summary-nothing = Annotation has nothing to do — all { $total } symbols on '{ $sheet }' already carry a designator. Use Reset All or Reset & Renumber below to renumber from scratch.
dialog-annotate-summary-changes = Annotation will assign new designators to { $changes } of { $total } symbols on '{ $sheet }'. The list updates live. Click Accept Changes to apply, or Reset All to clear every number first.
dialog-annotate-summary-heading = Annotation Summary
dialog-annotate-reprefix-unavailable = Re-prefix to standard is unavailable: this project follows no designator standard. Pick one in Project Options.
dialog-annotate-reprefix-note = Re-prefix to standard re-issues the prefix of every placed part under { $standard } and renumbers them, across the whole project, as one undo step, after you confirm a list of the changes. Sheets that are not open are loaded unsaved; nothing is written until you save. Locked parts, hand-edited prefixes and parts without a library link are left alone and listed in Messages. PCB references are not updated.
dialog-annotate-running = Running…
dialog-version-control-track-heading = Track in repository
dialog-version-control-lfs = Track binary 3D models via Git LFS
dialog-version-control-lfs-help = Routes `*.step / *.stp / *.wrl / *.iges` through Git LFS. Requires `git lfs` locally.
dialog-version-control-scanning = Reading the project folder…
dialog-grid-step = Step
dialog-grid-step-unit = mm, both axes
dialog-grid-fine = Fine
dialog-grid-coarse = Coarse
dialog-grid-lines = Lines
dialog-grid-dots = Dots
dialog-grid-hidden = Hidden
dialog-grid-multiplier = Multiplier
dialog-grid-help = The step applies to both axes (one grid step per footprint editor). Display: Lines / Dots / Hidden. Multiplier sets the coarse-grid stride (5× / 10× are typical Altium defaults).
dialog-selection-filter-help = Toggle which kinds the canvas hit-test will accept. Pads is the only kind functionally wired today; the rest store flags for forward compatibility.
dialog-selection-filter-pads = Pads
dialog-selection-filter-tracks = Tracks
dialog-selection-filter-arcs = Arcs
dialog-selection-filter-pours = Pours
dialog-selection-filter-bodies-3d = 3D Bodies
dialog-selection-filter-keepouts = Keepouts
dialog-selection-filter-cutouts = Cutouts
dialog-selection-filter-texts = Texts
