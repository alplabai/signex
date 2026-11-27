# Dialogs.

# Autosave recovery prompt, the recovery copy's row: how long ago the copy
# was written and when. $age is an already-worded duration, such as
# "5 minutes ago"; $time is a clock time, such as "14:26" or "Nov 16 17:40".
dialog-autosave-recovery-autosaved-age = autosaved { $age } · { $time }

## Tools ▸ Assign Footprints (#2248): the project's placed parts and the
## footprints their library rows offer.

dialog-assign-footprints-title = Assign Footprints
# $count is how many parts are listed (a whole number).
dialog-assign-footprints-subtitle =
    { $count ->
        [one] { $count } part
       *[other] { $count } parts
    }
dialog-assign-footprints-intro = Each part can take only the footprints its own library row offers. Apply changes the open sheets as one undo step; nothing is saved until you save.
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
# Reason pill of a part with no library link (a hand-made part).
dialog-assign-footprints-no-link = No library link
# Placeholder of the filter field above the table.
dialog-assign-footprints-filter = Filter designator, value or row
# Filter chips above the table; each shows its count beside it.
dialog-assign-footprints-chip-all = All
dialog-assign-footprints-chip-changed = Changed
dialog-assign-footprints-chip-no-alternates = No alternates
# The chips and the filter field list no part.
dialog-assign-footprints-filtered-empty = No part matches the filter.
# The ticked bar: $count is how many parts are ticked (a whole number).
dialog-assign-footprints-ticked =
    { $count ->
        [one] { $count } ticked
       *[other] { $count } ticked
    }
dialog-assign-footprints-clear-ticks = Clear ticks
# The footprint preview column on the right.
dialog-assign-footprints-preview-title = Footprint preview
dialog-assign-footprints-preview-expand = Expand preview
dialog-assign-footprints-preview-collapse = Narrow preview
dialog-assign-footprints-preview-caption = Click a row to preview its footprint. A change draws the current and the new footprint side by side at the same scale. Read-only here; pads are edited in the footprint editor.
# The previewed part: $designator and $value as the part holds them.
dialog-assign-footprints-preview-part = { $designator } · { $value }
# A fact the preview cannot state.
dialog-assign-footprints-preview-none = —
dialog-assign-footprints-preview-loading = Reading the footprint…
# $why is the system's reason, in English.
dialog-assign-footprints-preview-failed = The footprint could not be read: { $why }
dialog-assign-footprints-preview-unlinked = This footprint is not in an open library, so it cannot be drawn.
# Tags under the two footprints of a change: the current one and the new one.
dialog-assign-footprints-preview-now = NOW
dialog-assign-footprints-preview-new = NEW
# Layer legend chips under the preview.
dialog-assign-footprints-legend-top-copper = Top copper
dialog-assign-footprints-legend-silkscreen = Silkscreen
dialog-assign-footprints-legend-courtyard = Courtyard
# Facts under the preview.
dialog-assign-footprints-fact-pads = Pads
dialog-assign-footprints-fact-pad-one = Pad 1 (mm)
dialog-assign-footprints-fact-courtyard = Courtyard (mm)
dialog-assign-footprints-fact-pin-map = Pin map
# A size in mm: $w by $h, or the diameter $d of a round pad.
dialog-assign-footprints-size-rect = { $w } × { $h }
dialog-assign-footprints-size-round = Ø { $d }
# Every pin keeps its pad. $pairs lists "1 → 1, 2 → 2".
dialog-assign-footprints-pin-map-matches = { $pairs } · matches
# Pins whose pad number is not on the new footprint; $numbers is a list.
dialog-assign-footprints-pin-map-missing = Pads { $numbers } are not on the new footprint
# Between the current and the new value of a fact.
dialog-assign-footprints-arrow = →

dialog-annotate-matching-unavailable = Parameter matching is not available yet.
dialog-annotate-sheet-options-unavailable = Start cannot be changed yet. Add Suffix is hidden until suffixes work.

## Graphite views (#2225)
dialog-annotate-order-heading = Order of Processing
dialog-annotate-order-up-then-across = Up Then Across
dialog-annotate-order-across-then-down = Across Then Down
dialog-annotate-order-down-then-across = Down Then Across
dialog-annotate-order-across-then-up = Across Then Up
dialog-annotate-process-location = Process Location of
dialog-annotate-process-location-designator = Designator
dialog-annotate-replace-sub-parts = Replace Sub-Parts
dialog-annotate-replace-sub-parts-off = Off
dialog-annotate-matching-heading = Matching Options
dialog-annotate-complete-packages = Complete Existing Packages
dialog-annotate-complete-packages-none = None
dialog-annotate-sheets-heading = Schematic Sheets To Annotate
dialog-annotate-sheets-empty = No schematic tab open.
dialog-annotate-sheets-col-sheet = Schematic Sheet
dialog-annotate-sheets-col-start = Start
dialog-annotate-changes-heading = Proposed Change List
dialog-annotate-col-lock = Lock
dialog-annotate-col-current = Current
dialog-annotate-col-proposed = Proposed
dialog-annotate-col-location = Location
dialog-annotate-loading = Loading project sheets…
dialog-annotate-load-failed = Could not load the project's sheets - see the Messages panel.
dialog-annotate-nothing-to-number = No symbols need annotation — every reference has a number.
# Banner when no designator changes. $total is a whole number.
dialog-annotate-summary-nothing = Nothing to number: all { $total } designators are set. Reset All or Reset & Renumber renumbers from scratch.
# Bold lead of the change-list banner. Both are whole numbers.
dialog-annotate-change-count = { $changes } of { $total } designators will change
dialog-annotate-reprefix-unavailable = Re-prefix to standard is unavailable: this project follows no designator standard. Pick one in Project Options.
dialog-annotate-reprefix-note = Re-prefix to standard uses { $standard } (Project Options) and works on the whole project after a confirm list. Locked parts, hand-edited prefixes and parts without a library link are left alone.
dialog-annotate-running = Running…
dialog-version-control-track-heading = Track in repository
dialog-version-control-lfs = Track binary 3D models via Git LFS
dialog-version-control-lfs-help = Routes *.step / *.stp / *.wrl / *.iges through Git LFS. Requires git lfs locally.
dialog-version-control-scanning = Reading the project folder…
dialog-grid-step = Step
dialog-grid-step-unit = mm
dialog-grid-fine = Fine display
dialog-grid-coarse = Coarse display
dialog-grid-lines = Lines
dialog-grid-dots = Dots
dialog-grid-hidden = Hidden
dialog-grid-multiplier = Multiplier
# Grid Properties (mockup row 142): the note beside the title, the step's scope and the footer.
dialog-grid-note = Cartesian · mm
dialog-grid-both-axes = both axes
dialog-grid-footer = One grid per footprint editor
# One multiplier segment. $multiplier is 1, 2, 5 or 10.
dialog-grid-multiplier-option = { $multiplier }
# The coarse grid spacing, step × multiplier. $value is the number in millimetres.
dialog-grid-coarse-value = coarse = { $value }
# Notes on the grid preview.
dialog-grid-too-dense = Fine grid too dense to draw at this size
dialog-grid-no-step = No grid: the step is not a number above zero
dialog-grid-label-refused = Not a length: type a number in mm, or press Esc
# Labels on the grid preview, before the value. $multiplier is 1, 2, 5 or 10.
dialog-grid-preview-step = step
dialog-grid-preview-coarse = { $multiplier }× =
# Tooltips of the preview's expand button.
dialog-grid-expand = Expand preview
dialog-grid-collapse = Collapse preview
# Move Selection By (footprint editor): the offset along X and Y, in millimetres.
dialog-move-by-title = Move Selection By
dialog-move-by-x = X (mm)
dialog-move-by-y = Y (mm)
# Align Selection (footprint editor): one choice per axis. "Space" makes the
# gaps between the centres equal; "None" leaves that axis alone.
dialog-align-title = Align Selection
dialog-align-horizontal = Horizontal
dialog-align-vertical = Vertical
dialog-align-left = Left
dialog-align-center = Center
dialog-align-right = Right
dialog-align-top = Top
dialog-align-bottom = Bottom
dialog-align-space = Space
dialog-align-none = None
dialog-selection-filter-help = Only Pads filters the canvas today. The other kinds are stored for later.
dialog-selection-filter-pads = Pads
dialog-selection-filter-tracks = Tracks
dialog-selection-filter-arcs = Arcs
dialog-selection-filter-pours = Pours
dialog-selection-filter-bodies-3d = 3D Bodies
dialog-selection-filter-keepouts = Keepouts
dialog-selection-filter-cutouts = Cutouts
dialog-selection-filter-texts = Texts

## ERC dialog: which rules are being edited, and saving them as a profile
dialog-erc-scope-project = Editing the rules stored in this project.
dialog-erc-scope-newer-project = This project's rule set is from a newer Signex and is kept unchanged; editing the "User Default" rule profile (this machine's rules).
dialog-erc-scope-project-without-rules = Editing the "User Default" rule profile (this machine's rules); this project stores no rules.
dialog-erc-scope-user-default = Editing the "User Default" rule profile (this machine's rules).
dialog-erc-profile-save-title = Save the project's ERC rules as a profile
# $name is the profile name the user typed.
dialog-erc-profile-overwrite-note = A profile named "{ $name }" already exists. Overwriting replaces its rules and version; projects that took their rules from it will show the change as an update.
dialog-erc-profile-name = Name
dialog-erc-profile-name-placeholder = Profile name
dialog-erc-profile-version = Version
dialog-erc-profile-version-placeholder = Version
# A pending profile update whose rules already match; only the stored hash differs.
dialog-erc-profile-hash-only = Profile "{ $name }": rules already match; only the recorded hash is out of date.
# Heading above the list of rule changes a profile update applies. $count is a whole number.
dialog-erc-profile-update-heading = Update from profile "{ $name }" changes { $count } entries:

## Dialog buttons (#2225)
dialog-annotate-reprefix = Re-prefix to standard
dialog-annotate-reset-all = Reset All
dialog-annotate-reset-renumber = Reset & Renumber
dialog-annotate-accept = Accept Changes
# Re-prefix confirm button. $count is how many parts change (a whole number).
dialog-annotate-reprefix-parts =
    { $count ->
        [one] Re-prefix { $count } part
       *[other] Re-prefix { $count } parts
    }
# Autosave recovery: load the recovered copy as unsaved edits.
dialog-autosave-recovery-restore = Restore
dialog-autosave-recovery-later = Decide later
# Closing a project or quitting with unsaved documents.
dialog-unsaved-discard-all = Discard All
# The discard of a project written by a newer version, which drops only that
# project's unsaved settings (#2904). $projects lists the names, quoted.
dialog-unsaved-discard-read-only = Discard { $projects }
# Create Sheet: the file already exists; point the sheet symbol at it and open
# it instead of creating one.
dialog-create-child-sheet-open-existing = Use existing file
dialog-create-child-sheet-browse = Browse...
dialog-create-child-sheet-later = Later
dialog-erc-store-rules = Store ERC rules in this project
dialog-erc-save-as-profile = Save as profile
dialog-erc-update-from-profile = Update from profile
dialog-erc-run = Run ERC
dialog-erc-apply-update = Apply update
# Replace an existing file or profile.
dialog-overwrite = Overwrite
dialog-rename-submit = Rename
dialog-version-control-enable = Enable
dialog-version-control-enabling = Enabling…
dialog-net-color-use = Use Color
dialog-parameter-manager-add-column = Add column
# Column headings of the Parameter Manager table.
dialog-parameter-manager-column-reference = Reference
dialog-parameter-manager-column-value = Value
dialog-parameter-manager-column-footprint = Footprint
# Shown with All sheets: the sheet file a part is placed on.
dialog-parameter-manager-column-sheet = Sheet
# The header switch: list the active sheet, or every open sheet of its project.
dialog-parameter-manager-scope-active = Active sheet
dialog-parameter-manager-scope-all = All sheets
# The Parameter Manager footer and its no-match card.
dialog-parameter-manager-clear-filter = Clear filter
# Undoes every cell edit since the dialog opened; $count is how many.
dialog-parameter-manager-revert = Revert changes ({ $count })
dialog-parameter-manager-done = Done
# The file changed or vanished on disk since it was opened.
dialog-disk-conflict-reload = Reload from disk
dialog-disk-conflict-recreate = Re-create
# Writes the open document to a new file and leaves the changed one as it is.
dialog-disk-conflict-save-as = Save As…
dialog-netlist-export-anyway = Export anyway (incomplete)
dialog-print-preview-export-pdf = Export PDF
# Footer of Print Preview when no file is ticked for export.
dialog-print-preview-no-files = No files selected for export
dialog-netlist-incomplete-title = Netlist is incomplete
# Headline of the netlist-incomplete prompt. $count is how many pages are
# left out, a whole number.
dialog-netlist-incomplete-count =
    { $count ->
        [one] { $count } page is not in the netlist
       *[other] { $count } pages are not in the netlist
    }
dialog-netlist-incomplete-detail = The file would miss their components, and nets that merge through them would carry the wrong names. A PCB import cannot tell.
dialog-netlist-incomplete-anyway-note = If you export anyway, the file is still a valid netlist and the omitted pages are reported in the Messages panel.
# Column headings of the omitted-page list.
dialog-netlist-omitted-page = Omitted page
dialog-netlist-omitted-why = Why
# Why a page is left out: no file at its path.
dialog-netlist-omitted-file-missing = file missing
# $symbol is the name of the sheet symbol that places the missing file.
dialog-netlist-omitted-placed-by = file missing · placed by { $symbol }

## Shared by dialog footer buttons (#2225): one id per common action, so
## every dialog's Cancel, OK, Save … translates the same way.
dialog-button-cancel = Cancel
dialog-button-ok = OK
dialog-button-apply = Apply
dialog-button-close = Close
dialog-button-save = Save
dialog-button-save-all = Save All
dialog-button-discard = Discard
dialog-button-delete = Delete
dialog-button-create = Create
dialog-button-back = Back
dialog-button-copy = Copy
dialog-button-undo = Undo
dialog-button-redo = Redo
dialog-button-place = Place
dialog-button-move = Move
dialog-button-align = Align

## Create Sheet (#1275): the file of a sheet symbol that has none yet.
dialog-create-child-sheet-title = Create Sheet
# $name is the sheet symbol's name.
dialog-create-child-sheet-intro = Create a file for sheet '{ $name }'
# $name is the sheet symbol's name.
dialog-create-child-sheet-question = Sheet '{ $name }' has no file yet. Where should it be created?
# $folder is a folder path, relative to the project folder, ending in "/".
dialog-create-child-sheet-relative-to = Relative to { $folder }
dialog-create-child-sheet-relative-to-project = Relative to the project folder
dialog-create-child-sheet-linked-folder = The sheet must be inside the project folder (a linked folder leaves it)
# $symbol is the name of the other sheet symbol.
dialog-create-child-sheet-file-placed = A file with that name already exists, and sheet symbol '{ $symbol }' already points at it. A sheet file is placed by one sheet symbol; pick another name.
dialog-create-child-sheet-file-exists = A file with that name already exists. Use it, or pick another name.
dialog-create-child-sheet-symbol-gone = The sheet symbol is no longer there
# Label of the file path field.
dialog-create-child-sheet-file-label = File
dialog-create-child-sheet-file-note = The sheet symbol's File field is set to this path when the file is created.
# Footer hint beside the Later button.
dialog-create-child-sheet-later-hint = Later keeps the symbol without a file
# The existing file is already a page of the project. $file is its file name,
# $page its sheet number (a whole number), $name the sheet symbol's name.
dialog-create-child-sheet-reuse-page = { $file } is already sheet { $page }; using it makes { $name } a copy of it.
# The existing file is already placed by a sheet symbol of another sheet.
# $file is its file name, $name the sheet symbol's name.
dialog-create-child-sheet-reuse = { $file } is already a sheet of this project; using it makes { $name } a copy of it.
## Native title bar of a dialog shown in its own window: the name the OS
## window switcher shows.
dialog-title-annotate = Annotate
dialog-title-erc = Electrical Rules Check
dialog-title-annotate-reset = Reset All Annotations
dialog-title-reprefix = Re-prefix to Standard
dialog-title-move-selection = Move Selection
dialog-title-net-colors = Net Colors
dialog-title-parameters = Parameters
dialog-title-preferences = Preferences
dialog-title-find-replace = Find and Replace
dialog-title-rename = Rename
dialog-title-remove = Remove
dialog-title-export-pdf = Export PDF
dialog-title-bom = Bill of Materials
dialog-title-enable-version-control = Enable Version Control
dialog-title-grid-properties = Grid Properties
dialog-title-selection-filter = Custom Selection Filter
dialog-title-new-project = New Project
dialog-title-about = About { -brand-name }
dialog-title-keyboard-shortcuts = Keyboard Shortcuts
dialog-title-passive-calculator = Passive Network Calculator
dialog-title-first-run-tour = Welcome to { -brand-name }
# The error notice window carries only the product name.
dialog-title-error-notice = { -brand-name }
dialog-title-netlist-incomplete = Netlist is incomplete
dialog-title-autosave-recovery = Recover Unsaved Edits
dialog-title-close-project = Close Project
# macOS names leaving the app "Quit", other systems "Exit".
dialog-title-quit = Quit { -brand-name }
dialog-title-exit = Exit { -brand-name }
dialog-title-open-library = Open Library
dialog-title-edit-component = Edit Component Details
dialog-title-pick-primitive = Pick Primitive
dialog-title-document-options = Document Options
# The dialog that sets up a library about to be created (its version control).
dialog-title-library-options = New Library
dialog-title-copy-to-library = Copy to Library
dialog-title-library-updates = Library Updates Available
dialog-title-import-library = Import Library
dialog-title-close-library = Close Library
dialog-title-delete-component = Delete Component
dialog-title-library-recovery = Library Recovery
dialog-title-net-color-custom = Custom Net Color
# $path is the parent sheet's file.
dialog-create-child-sheet-not-focused = The sheet's File field was not updated: { $path } could not be focused

## Design ▸ Differential Pair Classes… (#1282).

dialog-dpc-title = Differential Pair Classes
# $project is the project name; $classes and $pairs are whole numbers.
dialog-dpc-subtitle = { $project } · { $classes ->
        [one] { $classes } class
       *[other] { $classes } classes
    } · { $pairs ->
        [one] { $pairs } pair
       *[other] { $pairs } pairs
    }
# Under a class name: how many pairs use it. $count is a whole number.
dialog-dpc-pairs = { $count ->
        [one] { $count } pair
       *[other] { $count } pairs
    }
# The button that opens a class row's menu.
dialog-dpc-menu = ⋯
dialog-dpc-rename = Rename
dialog-dpc-duplicate = Duplicate
dialog-dpc-delete = Delete
dialog-dpc-new-class = New class
dialog-dpc-pairs-from = Pairs come from Differential Pair directives on the schematic, or from Create Pair from Nets in the PCB panel.
dialog-dpc-class = Class
dialog-dpc-name = Name
dialog-dpc-impedance = Impedance
dialog-dpc-width-gap = Width / Gap
dialog-dpc-on-top = on Top, from the stack
dialog-dpc-max-skew = Max skew
dialog-dpc-max-uncoupled = Max uncoupled
# A limit the Rules tab owns and no rule sets yet.
dialog-dpc-not-set = —
dialog-dpc-edit-in-rules = Edit in Rules ▸ High Speed
dialog-dpc-unit-mm = mm
dialog-dpc-unit-ohm = Ω
dialog-dpc-unit-pct = %
dialog-dpc-plus-minus = ±
dialog-dpc-members = Members
dialog-dpc-col-pair = Pair
dialog-dpc-col-p = P net
dialog-dpc-col-n = N net
dialog-dpc-col-source = Source
# Where a pair comes from. $sheet is the sheet file.
dialog-dpc-source-directive = directive · { $sheet }
dialog-dpc-no-members = No pair on the open sheets uses this class.
dialog-dpc-pairing = Pairing
dialog-dpc-pair-by-suffix = Pair nets by suffix _P/_N, DP/DN, +/-
dialog-dpc-no-class = The project has no differential pair class. Add one with New class.
dialog-dpc-footer = Shown in PCB panel ▸ Differential Pairs mode.
dialog-dpc-cancel = Cancel
dialog-dpc-ok = OK
dialog-dpc-blank-name = Every class needs a name.
dialog-dpc-duplicate-name = Two classes have the same name.
# Callouts of the preview. $value is a length in mm, or —.
dialog-dpc-width-label = Width { $value } mm
dialog-dpc-gap-label = Gap { $value } mm
# The impedance and its tolerance, each a value of its own on the preview. $ohm and $pct are numbers, or —.
dialog-dpc-zdiff-value = Zdiff { $ohm } Ω
dialog-dpc-tolerance-value = ± { $pct } %
dialog-dpc-preview-hint = Top layer, to scale across the pair. Click a value to type it; drag the N trace handle to change the gap.
dialog-dpc-p-net = P net
dialog-dpc-n-net = N net
dialog-dpc-no-project = Differential Pair Classes: open a project first.
dialog-dpc-project-closed = Differential Pair Classes: the project was closed; nothing was applied.
# The banner, and the Messages line of a refused edit, while the project's schematic_design section is unreadable.
dialog-schematic-design-unreadable = This project's channel naming, harness definitions and differential pair classes cannot be read by this version of Signex. They stay unchanged in the project file and cannot be edited here.
# Why a class's name and Delete are greyed. $count is a whole number.
dialog-dpc-locked-by-pairs = { $count ->
        [one] { $count } pair uses this class: it keeps its name and cannot be deleted.
       *[other] { $count } pairs use this class: it keeps its name and cannot be deleted.
    }
dialog-dpc-locked-by-closed-sheets = Some sheets of the project are not open, so their pairs cannot be counted. Open every sheet to rename or delete a class.
# The name a new class starts with (a net-class style name, not a sentence).
dialog-dpc-new-class-name = DIFF

## Smart Paste (mockup row 155).

# The Smart Paste dialog (mockup row 155).
dialog-smart-paste-title = Smart Paste
# $parts lists the copied designators; $wires and $labels count the rest.
dialog-smart-paste-subtitle = Clipboard: { $parts } · { $wires ->
    [one] 1 wire
   *[other] { $wires } wires
} · { $labels ->
    [one] 1 net label
   *[other] { $labels } net labels
}
dialog-smart-paste-close = ×
dialog-smart-paste-place = Place
dialog-smart-paste-once = Once, with offset
dialog-smart-paste-array = Array
dialog-smart-paste-offset = Offset X / Y
dialog-smart-paste-copies = Copies
dialog-smart-paste-step = Step X / Y
dialog-smart-paste-mm = mm
dialog-smart-paste-designators = Designators
# $mapping is "R4 → R6, R5 → R7" (one copy) or "R4 → R6, R8; R5 → R7, R9" (several).
dialog-smart-paste-next-free = Next free ({ $mapping })
dialog-smart-paste-next-free-plain = Next free
dialog-smart-paste-keep-designators = Keep unless taken
dialog-smart-paste-net-names = Net names
dialog-smart-paste-keep-nets = Keep (pasted nets join the originals)
dialog-smart-paste-rewrite-nets = Rewrite with a pattern
dialog-smart-paste-pattern = Pattern
dialog-smart-paste-start = Start n at
dialog-smart-paste-net = Net
dialog-smart-paste-after = After paste
dialog-smart-paste-kept-power = kept · power net
# $net is the copied net's name.
dialog-smart-paste-joins = { $net } (joins original)
# $names lists the rewritten net names that already exist.
dialog-smart-paste-names-taken = Smart Paste stopped: { $names } already exists, so the nets would join. Change the pattern or Start n at.
# $sheets lists the project sheets that could not be read.
dialog-smart-paste-sheet-unread = Smart Paste stopped: { $sheets } could not be read, so its net names are unknown and a rewritten net could join one. Fix the sheet, or keep the net names.
dialog-smart-paste-preview = Preview
dialog-smart-paste-copy-count = { $count ->
    [one] 1 copy
   *[other] { $count } copies
}
dialog-smart-paste-drag-hint = Ghosts show where the copies land; drag the first copy to set the offset (1.27 mm grid).
dialog-smart-paste-footer = Paste places at the cursor; Esc cancels.
dialog-smart-paste-paste = Paste
dialog-smart-paste-paste-copies = Paste { $count } copies
dialog-smart-paste-nothing-copied = Smart Paste: nothing to paste yet; copy a selection first.
dialog-smart-paste-sheet-changed = Smart Paste closed: the active sheet changed.

## ERC dialog (#2413): where the rules come from, and the Profile menu
# Pill under the title: the dialog edits the rules stored in the project.
dialog-erc-source-project = Rules: this project
# Pill under the title: the dialog edits this machine's "User Default" rule profile.
dialog-erc-source-user-default = User Default
# Footer menu button holding Store / Save as profile / Update from profile.
dialog-erc-profile-menu = Profile

# project-shell
## Rename Project (mockup row 23): shown while the folder keeps its name.
dialog-rename-project-folder-kept = The project folder is not renamed.
## Enable Version Control (mockup row 26): asked only when git has no author.
dialog-version-control-author-note = Git has no author name and email on this computer. Commits are signed with these, stored in this repository's settings.
dialog-version-control-author-name = Your name
dialog-version-control-author-email = Your email
## Netlist incomplete (mockup row 28): why an existing page is left out.
dialog-netlist-omitted-unreadable = unreadable
dialog-netlist-omitted-not-a-file = unreadable (not a file)
dialog-netlist-omitted-too-large = unreadable (too large)
dialog-netlist-omitted-read-failed = unreadable (could not be read)
dialog-netlist-omitted-newer-format = unreadable (newer format)
dialog-netlist-omitted-not-a-sheet = unreadable (not a sheet file)
dialog-netlist-omitted-refused = not read (refused)
# $parent is the sheet that places this one again: its chain of sheet-symbol names from the root (U_power/U_loop), or the root sheet's path.
dialog-netlist-omitted-sheet-cycle = sheet cycle · re-entered from { $parent }
# $kept is the whole path of the file this one cannot be told from; only it is in the netlist.
dialog-netlist-omitted-key-collision = same path as { $kept } on this file system
# An annotate run from operands that changed nothing; $why is the reason in words.
dialog-annotate-command-refused = Annotate did not run: { $why }

## Generate design notebook (#2945, mockup row 201).

dialog-nb-generate-title = Generate design notebook
# $project is the project file name, $revision the project's revision.
dialog-nb-generate-subtitle = { $project } · rev { $revision }
# $project is the project file name.
dialog-nb-generate-subtitle-no-revision = { $project }
dialog-nb-generate-variant = Variant
dialog-nb-generate-template = Template
dialog-nb-generate-template-a4 = Design report (A4)
dialog-nb-generate-document = Document
dialog-nb-generate-revision = Revision
dialog-nb-generate-sections = Sections
# $picked and $total are whole numbers.
dialog-nb-generate-sections-count = { $picked } of { $total }
dialog-nb-generate-section-summary = Summary
dialog-nb-generate-section-requirements = Requirements and how each was met
# $count is the number of requirements the project lists.
dialog-nb-generate-section-requirements-hint = { $count ->
        [one] { $count } requirement, evidence from blocks
       *[other] { $count } requirements, evidence from blocks
    }
dialog-nb-generate-section-architecture = Architecture: block diagram
dialog-nb-generate-section-architecture-hint = from the sheet hierarchy and power nets
dialog-nb-generate-section-decisions = Key decisions
dialog-nb-generate-section-decisions-hint = accepted Decision blocks
dialog-nb-generate-section-bom = Bill of materials summary
dialog-nb-generate-section-bom-hint = parts, lines, cost per board
dialog-nb-generate-section-checks = ERC, DRC and requirement checks
dialog-nb-generate-section-simulation = Simulation results
dialog-nb-generate-section-simulation-hint = saved runs
dialog-nb-generate-section-outputs = Outputs produced
dialog-nb-generate-section-outputs-hint = last Export Job run
dialog-nb-generate-section-open-items = Open items
dialog-nb-generate-section-open-items-hint = open Issues, failing checks
dialog-nb-generate-section-changelog = Change log
dialog-nb-generate-section-changelog-hint = from project version history
dialog-nb-generate-checks = Checks
dialog-nb-generate-checks-run = Run ERC, DRC and requirement checks now
dialog-nb-generate-checks-last = Use the last results
# $path is the generated file inside the project folder.
dialog-nb-generate-note = Opens in the notebook tab unsaved. Save writes { $path }; your own notebook pages are never changed.
# $command is the command line that does the same.
dialog-nb-generate-same-as = Same as { $command }
dialog-nb-generate-cancel = Cancel
dialog-nb-generate-run = Generate
dialog-nb-generate-running = Generating…
dialog-nb-generate-no-project = Open a project to generate its design notebook.
# $pages is a whole number, $path the generated file inside the project folder.
dialog-nb-generate-done = Design report generated: { $pages ->
        [one] { $pages } page
       *[other] { $pages } pages
    }, unsaved. Save writes { $path }.
# $why says why nothing was generated.
dialog-nb-generate-failed = The design report was not generated: { $why }
