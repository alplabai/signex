# Library Browser tab and the Library panel (#2225).

## Header

# Placeholder of the browser's search field.
library-browser-search = Search Internal PN, MPN, value, parameter

# Primary button of the header: add a component row to the active table.
library-browser-add-component = Add Component

# Caption under the library name: how many component rows the library holds
# across all its tables, and how many tables. Both are whole numbers.
library-browser-summary =
    { $components ->
        [one] { $components } component
       *[other] { $components } components
    } · { $tables ->
        [one] { $tables } table
       *[other] { $tables } tables
    }

# Caption in front of the lifecycle filter pills.
library-browser-show = Show

# Count line right of the filter pills: rows the search and filters leave
# in the active table, of all its rows. Both are whole numbers.
library-browser-shown = { $shown } of { $total } shown
# The same, with the column the grid is sorted by.
library-browser-shown-sorted = { $shown } of { $total } shown · sorted by { $column }

# Status bar on a Library Browser tab: the library file and its row count.
library-status-summary = { $library } · { $components ->
        [one] { $components } component
       *[other] { $components } components
    }
# The same with a row selected: its Internal PN.
library-status-selected = { $library } · 1 selected · { $pn }
# Status bar on a component editor tab: the Internal PN and its lifecycle.
library-status-component = { $pn } · { $lifecycle }

# Lifecycle filter pill: Released, In Review and Draft rows.
library-browser-filter-active = Active + Preferred

# Lifecycle filter pill: Released rows only.
library-browser-filter-preferred = Preferred Only

# Lifecycle filter pill: the default set plus Deprecated rows.
library-browser-filter-deprecated = Incl. Deprecated

# Lifecycle filter pill: every row, Obsolete ones included.
library-browser-filter-all = All (incl. Obsolete)

## Grid

# Header of the column of lifecycle pills (Released, Draft, …).
library-browser-lifecycle = Lifecycle

## Left column: the libraries tree and the table list

# Section heading over the mounted libraries, shown in capitals.
library-browser-libraries = LIBRARIES

# Pill beside a library that cannot be edited (the stock library).
library-browser-read-only = read-only

# Row count beside a library or a table. $count is a whole number.
library-browser-row-count = { $count }

# Section heading over the library's tables, shown in capitals.
library-browser-tables = TABLES

# Check-mark button that applies a table rename.
library-browser-rename-confirm = ✓

# Button that creates the table named in the add-table form.
library-browser-create-table = Create

# Button that closes the add-table form without creating a table.
library-browser-cancel = Cancel

# Sidebar footer when saved library files are not in git yet. $count is the
# number of files (a whole number); $reason is the cause reported by git.
library-browser-uncommitted =
    { $count ->
        [one] Uncommitted changes: { $count } saved file not in version control. { $reason }
       *[other] Uncommitted changes: { $count } saved files not in version control. { $reason }
    }

# Button in that footer: retry the git commit.
library-browser-commit-now = Commit now

# The same button while the commit runs.
library-browser-committing = Committing…

## Preview pane of the selected row. The field captions are shown in capitals,
## so they are written in capitals here.

# Shown in place of an empty field value.
library-browser-no-value = —

# Revision field of a released row. $version is the row's version text.
library-browser-revision-released = { $version } · released

# Caption of the component class field.
library-preview-class = CLASS

# Caption of the manufacturer field.
library-preview-manufacturer = MANUFACTURER

# Caption of the manufacturer part number field.
library-preview-mpn = MPN

# Caption of the revision field.
library-preview-revision = REVISION

# Caption of the bound schematic symbol field.
library-preview-symbol = SYMBOL

# Caption of the bound footprint field.
library-preview-footprint = FOOTPRINT

# Caption of the last-updated date field.
library-preview-updated = UPDATED

## Library panel

# Button on a library row of the Library panel: open its browser tab.
library-panel-open = Open

## Copy to Library dialog

# Note when the library the last copy went to cannot take this one. $path is
# that library's folder.
library-stock-copy-previous-gone = The library used before ({ $path }) is no longer open or writable.

## Graphite views (#2225)
library-browser-table-name-placeholder = table_name
library-browser-new-table-name-placeholder = new_table_name
library-browser-add-class = + Class
library-browser-no-matching-rows = No matching rows.
library-browser-no-symbol-bound = No symbol bound
library-browser-no-footprint-bound = No footprint bound
library-range-filter-add = Add range filter…
library-range-filter-and = and
library-range-filter-number-placeholder = number
library-range-filter-enter-number = Enter a number to apply the filter.
library-range-filter-excluded = { $count } excluded: not a measurement in { $unit }.
library-preview-header-manufacturer = Manufacturer
library-preview-not-set = Not set
library-preview-no-description = No description
library-preview-pad-placeholder = pad number…
library-preview-override-pin = Override pin { $pin } →
# Footer of the component editor. $sheets and $projects are whole numbers.
library-preview-where-used = Used by { $sheets ->
        [one] { $sheets } sheet
       *[other] { $sheets } sheets
    } in { $projects ->
        [one] { $projects } project
       *[other] { $projects } projects
    }.
library-stock-copy-no-writable = No writable library is open. Create a new one to copy into.

## Library panel (left dock)
library-panel-search = Search libraries…
library-panel-none-open = No libraries open. Use File ▸ Library ▸ Open Library… to open a *.snxlib/.
library-panel-open-library = + Open Library…
library-panel-no-tables = No tables in this library.
# Row-count pill of a library. $count is a whole number.
library-panel-rows = { $count } rows

## Library Browser row in the Properties panel: what the row binds
library-row-no-symbol = No symbol bound
# $pins is a whole number.
library-row-symbol-pins = Symbol bound — { $pins ->
        [one] { $pins } pin
       *[other] { $pins } pins
    }
library-row-symbol-absent = Symbol unresolved (UUID not in mounted libraries)
# $error is the reason the library gave.
library-row-symbol-failed = Symbol could not be read from its library — { $error }
library-row-symbol-reading = Reading symbol…
# $summary is one of the symbol lines above; $count is a whole number.
library-row-alternate-symbols = { $summary } (+{ $count ->
        [one] { $count } alternate symbol
       *[other] { $count } alternate symbols
    })
library-row-no-footprint = No footprint bound
library-row-footprint-bound = Footprint bound
library-row-footprint-absent = Footprint unresolved (UUID not in mounted libraries)
# $error is the reason the library gave.
library-row-footprint-failed = Footprint could not be read from its library — { $error }
library-row-footprint-reading = Reading footprint…

## SCH Library panel: Delete of a symbol that is still in use. $name is the symbol name.
library-symbol-delete-checking = Checking whether symbol "{ $name }" is in use
library-symbol-delete-may-be-in-use = Symbol "{ $name }" may still be in use
library-symbol-delete-in-use = Symbol "{ $name }" is still in use

## Library dialog buttons (#2225)
library-create-options-create = Create Library
# Edit Component: the row changed on disk while it was being edited.
library-edit-row-reload = Reload
library-edit-row-keep-editing = Keep editing
library-edit-row-reload-saved = Reload saved version
# Change Component: go on to the pin preview.
library-picker-next = Next
library-primitive-picker-browse = Browse filesystem…
library-primitive-picker-open-library = Open library
library-primitive-picker-dont-copy = Don't copy
library-recovery-remove-from-project = Remove from project
library-recovery-locate = Locate…
library-recovery-restore-remote = Restore from remote
library-recovery-skip-read-only = Skip (read-only)
library-recovery-reinit = Re-init (lose history)
library-recovery-remove-row = Remove row
library-recovery-rebind = Re-bind…
library-distributor-connect-oauth = Connect via OAuth
library-distributor-test = Test
library-stock-copy-new-library = New Library…
library-updates-skip-all = Skip All
# $count is how many components are ticked (a whole number).
library-updates-apply = Update Selected Components ({ $count })
# Change Component: confirm; "anyway" when the change loses connections or data.
library-change-part-confirm = Change
library-change-part-confirm-anyway = Change anyway

## Preferences ▸ Library ▸ Distributor APIs
library-digikey-connecting = Connecting…
# $account is the signed-in DigiKey account.
library-digikey-connected-as = Connected as { $account }
library-digikey-not-connected = Not connected
library-digikey-help = OAuth2 PKCE flow — opens your browser, then stores the refresh token in the OS keyring. Set SIGNEX_DIGIKEY_CLIENT_ID + SIGNEX_DIGIKEY_CLIENT_SECRET first.
library-mouser-api-key = API Key
library-mouser-not-tested = Not tested
library-no-key-required = (no key required)
library-order-of-preference = Order of Preference
# One row of the preference list: $rank is its place (1, 2, …) and $name the
# distributor.
library-order-row = { $rank }.  { $name }
# A distributor that is none of the named ones.
library-distributor-other = Other
library-order-help = Saved to <config_dir>/signex/distributors.toml. The first matching adapter is tried first when you paste a distributor URL into the Supply tab.
# $error is the system's reason, in English.
library-order-save-failed = Could not save the order: { $error }

## Copy to Library (#134)

# Intro title. $label is the name of the stock symbol or footprint.
library-stock-copy-intro-title = Copy { $label } to a library
# Intro text under the title.
library-stock-copy-intro-text = The stock library is read-only. The copy opens for editing; the stock original is not changed.
# Primary button: copy into the selected library. $library is its name.
library-stock-copy-to = Copy to { $library }
# Primary button while no library is selected.
library-stock-copy-copy = Copy
# The chosen library can no longer take the copy. $path is its file.
library-stock-copy-not-writable = { $path } is not open and writable; pick another library.

## Close Library (#136)

# Intro title of the prompt shown when a library with unsaved drafts closes.
library-close-intro-title = Save the drafts before closing?
# $library is the library's name, $count how many drafts are unsaved.
library-close-intro-text = '{ $library }' has { $count ->
        [one] { $count } unsaved draft
       *[other] { $count } unsaved drafts
    }.
# Kind of a draft that is a table row. $table is the table's name.
library-close-kind-row = { $table } · row
# Kind of a draft that is a file (a symbol or footprint).
library-close-kind-file = file

## Library recovery (#138)

# Footer button: close the dialog and decide later.
library-recovery-skip-for-now = Skip for now
# Keys of the facts box.
library-recovery-key-library = Library
library-recovery-key-expected = Expected
library-recovery-key-remote = Remote
library-recovery-key-row = Row
library-recovery-key-missing-uuid = Missing UUID
# Library Missing: dialog title, intro and the caption under the facts.
library-recovery-missing-title = Library Missing
library-recovery-missing-intro-title = The library file is missing
library-recovery-missing-intro-text = The project refers to a library that is not on disk.
# $library is the missing library's name.
library-recovery-missing-caption = Skip for now opens the project anyway: parts from { $library } keep their embedded copies.
# Git Repository Missing: dialog title, intro, banners and caption.
library-recovery-git-title = Git Repository Missing
library-recovery-git-intro-title = The library's history is missing
library-recovery-git-intro-text = Version control is on for this library, but its .git folder is gone.
# $error is the system's reason, in English.
library-recovery-git-reinit-failed = Re-init failed: { $error }
library-recovery-git-reinit-warning = Re-init starts a new history: all earlier commits are lost.
library-recovery-git-caption = Skip for now leaves the library closed until its history is restored.
# Broken binding: $kind is symbol, footprint, sim or other; $table is the
# table that holds the row.
library-recovery-binding-title = Broken { $kind ->
        [symbol] Symbol
        [footprint] Footprint
        [sim] Sim Model
       *[other] Primitive
    } Binding
library-recovery-binding-intro-title = The { $kind ->
        [symbol] Symbol
        [footprint] Footprint
        [sim] Sim Model
       *[other] Primitive
    } is missing
library-recovery-binding-intro-text = A row in '{ $table }' references a { $kind ->
        [symbol] Symbol
        [footprint] Footprint
        [sim] Sim Model
       *[other] Primitive
    } that no longer exists on disk.
library-recovery-binding-caption = Skip for now keeps the row as it is until it is re-bound.

## Edit Component (#130)
# Dialog title.
library-edit-row-title = Edit Component
# Header note: $pn is the row's part number, $library the library, $table the table.
library-edit-row-note = { $pn } · { $library } / { $table }
# Section heading, written in capitals.
library-edit-row-identity = IDENTITY
# Section heading, written in capitals.
library-edit-row-bindings = BINDINGS
# Section heading, written in capitals.
library-edit-row-parameters = PARAMETERS
# Field label, also the field's placeholder.
library-edit-row-internal-pn = Internal PN
# Field label: the component class.
library-edit-row-class = Class
# Placeholder of the Class dropdown.
library-edit-row-class-placeholder = Select class…
# Field label: the lifecycle state.
library-edit-row-state = State
# Placeholder of the State dropdown.
library-edit-row-state-placeholder = Select lifecycle…
# Field label, also the field's placeholder.
library-edit-row-manufacturer = Manufacturer
# Field label: manufacturer part number; also the field's placeholder.
library-edit-row-mpn = MPN
# Field label.
library-edit-row-description = Description
# Placeholder of the Description field: a sample description.
library-edit-row-description-placeholder = e.g. 100 nF 50 V X7R 0603
# Field label.
library-edit-row-datasheet = Datasheet
# Placeholder of the Datasheet field: a sample address.
library-edit-row-datasheet-placeholder = https://example.com/datasheet.pdf
# A datasheet stored in the library, shown read-only; $file is its file name.
library-edit-row-datasheet-pinned = Pinned PDF: { $file }
# Field label.
library-edit-row-tags = Tags (comma-separated)
# Placeholder of the Tags field: sample tags.
library-edit-row-tags-placeholder = e.g. low-noise, RoHS, qualified
# Error under the Internal PN field; $pn is the typed part number, $table the table of the row that has it.
library-edit-row-pn-conflict = Internal PN “{ $pn }” is already used in { $table }.
# Footer summary of what holds Save; $count is a whole number, $fields the field names.
library-edit-row-problems = { $count ->
        [one] { $count } problem
       *[other] { $count } problems
    }: { $fields }
# Binding card title.
library-edit-row-symbol-ref = Symbol ref
# Binding card title.
library-edit-row-footprint-ref = Footprint ref
# Binding card text when the row binds no symbol.
library-edit-row-symbol-unbound = Unbound — pick a symbol
# Binding card text when the row binds no footprint.
library-edit-row-footprint-unbound = Unbound (optional)
# What a binding resolves to: $file is the file, $name the symbol or footprint.
library-edit-row-binding = { $file } · { $name }
# A binding no open library lists; $id is the short id it names.
library-edit-row-binding-missing = Not in the open libraries ({ $id })
# Inline confirm before the draft is discarded.
library-edit-row-reload-confirm = Discard your edits and reload?
# Button that adds an empty parameter row.
library-edit-row-add-parameter = + Add Parameter
# Parameters table column heading.
library-edit-row-key = Key
# Parameters table column heading.
library-edit-row-value = Value
# Parameters table column heading.
library-edit-row-unit = Unit
# Placeholder of a parameter's number field.
library-edit-row-value-placeholder = value
# The parameters table when it is empty.
library-edit-row-no-parameters = No parameters yet.
# Pill after a parameter that comes from the class template.
library-edit-row-template-pill = template
# Caption under the parameters table; $class is the class name.
library-edit-row-template-caption = Template parameters come from the { $class } class and cannot be removed.
# Heading over the template parameters the row has no value for yet.
library-edit-row-template-offer = Template parameters
# Button that adds a template parameter; $name is the parameter.
library-edit-row-add-slot = + { $name }
# Button that adds a template parameter; $name is the parameter, $unit its unit symbol.
library-edit-row-add-slot-unit = + { $name } ({ $unit })
# Why a stored value breaks its template rule; $reason is the rule, in English.
library-edit-row-slot-problem = ✗ { $reason }
# Tooltip of a unit this version does not know, which is kept unchanged.
library-edit-row-unit-kept = Kept as written

## Delete Component (#137)
# Dialog title.
library-delete-row-title = Delete Component?
# Intro title; $pn is the row's part number.
library-delete-row-intro-title = Delete { $pn }?
# Intro text; $pn is the row's part number, $table its table.
library-delete-row-intro = Delete row { $pn } from { $table }? This cannot be undone from the UI.
# Caption under the intro text.
library-delete-row-caption = Placed parts keep their embedded copy.

## Place Component picker (#129)
# Dialog title of the picker when it places a part.
library-picker-title-place = Place Component
# Dialog title of the picker when it swaps a placed part for another.
library-picker-title-change = Change Component
# Dialog title of the picker when it chooses a datasheet to open.
library-picker-title-datasheet = Open Datasheet
# Header note: how many open libraries the listed parts come from, and how many parts.
library-picker-scope = { $libraries ->
        [one] { $libraries } library
       *[other] { $libraries } libraries
    } · { $parts ->
        [one] { $parts } part
       *[other] { $parts } parts
    }
# Placeholder of the search field.
library-picker-search = Search internal PN / MPN / description…
# Column head: the library's own part number.
library-picker-column-internal-pn = Internal PN
# Column head: the part's lifecycle state (Draft, Released, …).
library-picker-column-state = State
# Column head: the manufacturer part number.
library-picker-column-mpn = MPN
# Column head: the part's description.
library-picker-column-description = Description
# Shown in place of the list when no part matches.
library-picker-empty = No components match. Open a library or refine the filter.
# Footer hint: Enter in the search field places the selected part.
library-picker-hint-place = Enter to place
# Footer hint, Change Component: Enter goes on to the change preview.
library-picker-hint-next = Enter for Next

## Primitive picker (#131)
# Dialog titles, by the kind of primitive picked.
library-primitive-picker-title-symbol = Pick Symbol
library-primitive-picker-title-footprint = Pick Footprint
library-primitive-picker-title-sim = Pick Sim Model
library-primitive-picker-title-primitive = Pick Primitive
# Header note: the component row the pick is for. $pn is its internal part number.
library-primitive-picker-for = for { $pn }
# Placeholder of the filter field.
library-primitive-picker-filter = Filter…
# One listed primitive: $file is the file that holds it, $name the primitive's name.
library-primitive-picker-row = { $file } · { $name }
# Group head: the footprints the component's own library part offers.
library-primitive-picker-group-suggested = This component's footprints
# Group head: symbols of a library that is not open. $library is its name.
library-primitive-picker-group-not-open = In library { $library } (not open)
# Group head: symbols of files that belong to no library.
library-primitive-picker-group-no-library = Not in a library
# Shown in place of the list when no library is open.
library-primitive-picker-no-libraries = No libraries open. Use Browse filesystem… to pick a primitive directly.
# Shown in place of the list when nothing matches the filter.
library-primitive-picker-no-match = No primitives match. Only primitives saved inside an open `.snxlib` are listed: save the symbol into `<lib>.snxlib/symbols/`, or use Browse filesystem…
# The library the row belongs to, when its name is not known.
library-primitive-picker-this-library = this library
# Title of the confirm shown before a symbol is copied or linked.
library-primitive-picker-copy-title = Copy into library?
# The symbol is in no library. $name is the symbol, $library the row's library.
library-primitive-picker-copy-loose = Copy { $name } into library { $library }?
# What the copy does. $library is the row's library.
library-primitive-picker-copy-loose-detail = The original file stays as it is. The row then points at the copy inside { $library }.
# The symbol belongs to a library that is not open. $name is the symbol, $library that library.
library-primitive-picker-copy-held = { $name } is in library { $library }, which is not open.
# Caption under the question. $library is the library that holds the symbol.
library-primitive-picker-link-caption = A link breaks if { $library } is not open.
# Button: open the other library and bind the row to its symbol, copying nothing.
library-primitive-picker-link = Link without copying
# Button: copy the symbol into the row's library. $library is that library.
library-primitive-picker-copy-into = Copy into { $library }
# The copy button while the copy runs.
library-primitive-picker-copying = Copying…
# Shown while the library that holds the symbol is being opened.
library-primitive-picker-opening = Opening library…

## Document Options dialog of a library (#132)
# Header note: $name is the library's display name.
library-document-options-note = Library: { $name }
# Form label: the colour of the symbol and footprint editor sheets.
library-document-options-sheet-color = Sheet Color
# Form label of the grid visibility switch.
library-document-options-grid = Grid
# Beside the grid switch: the grid is drawn.
library-document-options-grid-visible = Visible
# Form label: the distance between grid points.
library-document-options-grid-spacing = Grid Spacing
# One grid spacing in the dropdown: $pitch is the number in millimetres,
# already formatted (three decimals).
library-document-options-grid-pitch = { $pitch } mm
# Caption beside the dropdown: $pitches lists the preset spacings,
# already formatted and separated.
library-document-options-grid-presets = { $pitches }
# Form label: the unit coordinates are shown in.
library-document-options-unit = Unit
# Unit segment: millimetres.
library-document-options-unit-mm = mm
# Unit segment: thousandths of an inch.
library-document-options-unit-mil = mil
# Unit segment: inches.
library-document-options-unit-inch = inch
# Unit segment: micrometres.
library-document-options-unit-um = µm

## New Library dialog (#133)
# Option card title: put the new library under version control.
library-create-options-git = Enable version control (Git)
# Option card help under the version-control title.
library-create-options-git-help = Initialise a Git repository at the library's parent directory and commit on every save. Off by default; can be enabled later.
# Option card title, indented under the version-control card.
library-create-options-lfs = Use Git LFS for binary 3D models
# Option card help under the Git LFS title; the file patterns and the
# git lfs command are not translated.
library-create-options-lfs-help = Tracks *.step, *.stp, *.wrl, *.iges via Git LFS. Requires git lfs installed locally.
# Refusal banner action: reopen the save-as picker to pick another place.
library-create-options-relocate = Choose another location…
# Tooltip of the disabled Create Library button while a refusal shows.
library-create-options-refused-tip = Choose another location, or turn off version control.

## Library Updates (#135)
# Message over the drift table. $count is how many placed components differ
# from their library rows (a whole number).
library-updates-summary =
    { $count ->
        [one] { $count } placed component drifted from the library since this schematic was last saved.
       *[other] { $count } placed components drifted from the library since this schematic was last saved.
    }
# Summary pill: $count components (a whole number) have a major version change.
library-updates-count-major = { $count } major
# Summary pill: $count components (a whole number) have a minor version change.
library-updates-count-minor = { $count } minor
# Summary pill: $count components (a whole number) have a patch version change.
library-updates-count-patch = { $count } patch
# Column heading of the drift table: the placed part (designator and Internal PN).
library-updates-column-part = Part
# Column heading: the library the part comes from.
library-updates-column-library = Library
# Column heading: the part's version and its library row's version now.
library-updates-column-version = Version
# Column heading: how large the version change is.
library-updates-column-change = Change
# Change pill: a major version change, which may break the placed part.
library-updates-change-major = ⚠ Major
# Change pill: a minor version change.
library-updates-change-minor = Minor
# Change pill: a patch version change.
library-updates-change-patch = Patch
# Version cell: $current is the placed part's version, $latest its library
# row's version now.
library-updates-version = { $current } → { $latest }
# Conflict line: caption before the value the placed part holds.
library-updates-conflict-yours = yours
# Conflict line: caption before the value the library row holds now.
library-updates-conflict-library-now = library now
# Conflict answer: keep the placed part's own value.
library-updates-keep-mine = Keep mine
# Conflict answer: take the library row's value.
library-updates-use-library = Use library value
# Conflict line: the field that differs, the part's footprint.
library-updates-field-footprint = Footprint
# Conflict line: the field that differs, the part's manufacturer.
library-updates-field-manufacturer = Manufacturer
# Conflict line: the field that differs, the manufacturer part number.
library-updates-field-mpn = MPN
