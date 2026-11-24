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
# Kind chips at the top of the Library panel (mockup row 119), with their tooltips.
library-panel-kind-symbols = Symbols
library-panel-kind-footprints = Footprints
library-panel-kind-parts = Parts
library-panel-show-symbols = Show symbols
library-panel-show-footprints = Show footprints
library-panel-show-parts = Show parts
# Tooltip of a kind chip while nothing is open to browse.
library-panel-open-first = Open a library first
# Breadcrumb under the chips: "<library> › <file> · <count>". $library is a
# library name; the file name sits between this and the count.
library-panel-crumb-library = { $library } ›
# $count is a whole number.
library-panel-crumb-symbols = · { $count ->
        [one] { $count } symbol
       *[other] { $count } symbols
    }
library-panel-crumb-footprints = · { $count ->
        [one] { $count } footprint
       *[other] { $count } footprints
    }
library-panel-crumb-parts = · { $count ->
        [one] { $count } part
       *[other] { $count } parts
    }
library-panel-no-library = No library open
library-panel-no-symbol-file = No symbol file open
library-panel-no-footprint-file = No footprint file open
library-panel-filter-symbols = Filter symbols
library-panel-filter-footprints = Filter footprints
library-panel-filter-parts = Filter parts
# Toolbar of the Library panel.
library-panel-place = Place
library-panel-add = Add
library-panel-edit = Edit
library-panel-delete = Delete
# Tooltip on the disabled Delete of the Parts kind: the delete confirmation
# runs in the library's Library Browser tab.
library-panel-delete-needs-browser = Open the library in the Library Browser to delete a row
# Tooltips on the disabled Place of the Library panel: why it cannot act.
library-panel-place-no-row = Select a footprint in the list to place it
library-panel-place-no-board = Open a board to place this footprint
library-panel-place-several-boards = Several boards are open; keep one open to place this footprint
library-panel-place-read-only-board = The open board is read-only in this version of Signex
library-panel-place-board-in-sketch = Leave sketch mode on the board to place this footprint
library-panel-place-unsaved = Save this footprint to place it
library-panel-place-not-in-library = Only a footprint saved in an open library can be placed
library-panel-place-not-first = Only the first footprint of a file can be placed
library-panel-place-undocked = Dock this footprint's tab to place it
library-panel-place-symbols = Placing a symbol from here is not available yet; use the Parts list
# Section under the symbol list. $symbol is the active symbol's name.
library-panel-parts-of = Parts of { $symbol }
# Pad count of a footprint row. $count is a whole number.
library-panel-pads = { $count ->
        [one] { $count } pad
       *[other] { $count } pads
    }
# A bare count beside a table name. $count is a whole number.
library-panel-count = { $count }
library-panel-no-matches = Nothing matches the filter.
# Empty state of the Library panel.
library-panel-empty-title = No libraries open
library-panel-empty-body = Open a *.snxlib to browse its parts, symbols and footprints here.
# Empty-state button and the title-bar folder button's tooltip.
library-panel-open-library-action = Open Library…

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
library-recovery-remove-from-project = Remove from project
library-recovery-locate = Locate…
library-recovery-restore-remote = Restore from remote
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
# Line under the page heading.
library-distributors-intro = Accounts used to fetch prices, stock and part data. Keys and tokens are stored in the OS keyring.
# Status pill of the DigiKey card while the browser sign-in runs.
library-digikey-connecting = Connecting…
# $account is the signed-in DigiKey account.
library-digikey-connected-as = Connected as { $account }
library-digikey-not-connected = Not connected
library-digikey-help = OAuth2: opens your browser, then stores the refresh token in the OS keyring.
# Button of a connected DigiKey card: forgets the account and its stored token.
library-digikey-disconnect = Disconnect
# $error is the system's reason, in English.
library-digikey-disconnect-failed = Could not disconnect: { $error }. The token is still stored.
# The Messages line for the same failure; $why is the system's reason, in English.
library-digikey-disconnect-failed-log = DigiKey: disconnecting the account failed, so its token is still stored: { $why }
# Status or Messages reason when a distributor settings worker (sign-in, keyring, key test) stopped on a panic; $error is the runtime's own text, in English.
library-worker-panicked = worker thread panicked: { $error }
# Status pill when no DigiKey client id was set when Signex started.
library-digikey-not-configured = Not configured
# Keep the two environment variable names exactly as written.
library-digikey-not-configured-help = The DigiKey client id is not configured, so Connect cannot start. Set SIGNEX_DIGIKEY_CLIENT_ID and SIGNEX_DIGIKEY_CLIENT_SECRET, then start { -brand-name } again.
library-mouser-api-key = API Key
library-mouser-key-placeholder = Paste API key…
# Status pills of the Mouser card.
library-mouser-not-tested = Not tested
library-mouser-testing = Testing…
library-mouser-verified = Key verified
library-mouser-test-failed = Test failed
library-no-key-required = No key required
library-no-key-help = Public catalogue lookups. Nothing to set up.
library-order-of-preference = Order of Preference
# The place of a distributor in the preference list (1, 2, …).
library-order-rank = { $rank }
# A distributor that is none of the named ones.
library-distributor-other = Other
library-order-help = The first matching adapter is tried first when you paste a distributor URL into the Supply tab.
# $error is the system's reason, in English.
library-order-save-failed = Could not save the order: { $error }

## Copy to Library (mockup row 134)

# Intro title. $label is the name of the stock symbol or footprint.
library-stock-copy-intro-title = Copy { $label } to a library
# Intro text under the title.
library-stock-copy-intro-text = The stock library is read-only. The copy opens for editing; the stock original is not changed.
# The chosen library can no longer take the copy. $path is its file.
library-stock-copy-not-writable = { $path } is not open and writable; pick another library.

## Close Library (mockup row 136)

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

## Library recovery (mockup row 138)

# Footer button: close the dialog and decide later (Git Repository Missing:
# the library opens read-only).
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
library-recovery-missing-caption = Skip for now opens the project anyway: parts from { $library } keep their embedded copies, and a "Library Missing" chip stays in the status bar until the library is located.
# Status bar chip while a skipped library of an open project is still gone; a
# click reopens its dialog. $count is how many are missing.
library-recovery-missing-chip = { $count ->
        [one] Library Missing
       *[other] Library Missing ({ $count })
    }
# Git Repository Missing: dialog title, intro, banners and caption.
library-recovery-git-title = Git Repository Missing
library-recovery-git-intro-title = The library's history is missing
library-recovery-git-intro-text = Version control is on for this library, but its .git folder is gone.
# $error is the system's reason, in English.
library-recovery-git-reinit-failed = Re-init failed: { $error }
library-recovery-git-reinit-warning = Re-init starts a new history: all earlier commits are lost.
library-recovery-git-caption = Skip for now opens the library read-only until its history is restored.
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

## Edit Component (mockup row 130)
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
# The lifecycle states of a component row, as its State dropdown and the
# Place Component State column show them.
library-lifecycle-draft = Draft
library-lifecycle-in-review = In review
library-lifecycle-released = Released
library-lifecycle-deprecated = Deprecated
library-lifecycle-obsolete = Obsolete
# A state written by a newer version of Signex.
library-lifecycle-unknown = Unknown
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
# Heading over the SI prefixes in a measurement's menu (mockup row 139).
library-edit-row-prefix = Prefix

## Unit menu captions (mockup row 139)
# Muted caption of the "—" entry: the value carries no unit.
library-unit-name-none = no unit
# Muted caption of the Ω entry.
library-unit-name-ohm = ohm
# Muted caption of the F entry.
library-unit-name-farad = farad
# Muted caption of the H entry.
library-unit-name-henry = henry
# Muted caption of the V entry.
library-unit-name-volt = volt
# Muted caption of the A entry.
library-unit-name-ampere = ampere
# Muted caption of the W entry.
library-unit-name-watt = watt
# Muted caption of the Hz entry.
library-unit-name-hertz = hertz

## Delete Component (mockup row 137)
# Dialog title.
library-delete-row-title = Delete Component?
# Intro title; $pn is the row's part number.
library-delete-row-intro-title = Delete { $pn }?
# Intro text; $pn is the row's part number, $table its table.
library-delete-row-intro = Delete row { $pn } from { $table }? This cannot be undone from the UI.
# Caption under the intro text.
library-delete-row-caption = Placed parts keep their embedded copy.

## Place Component picker (mockup row 129)
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

## Primitive picker (mockup row 131)
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
# Link without copying found the library open but without the symbol; nothing
# is bound. $library is the library, $name the symbol.
library-primitive-picker-not-listed = { $library } does not list { $name }; nothing was linked.
# Footer button: binds the row highlighted in the list.
library-primitive-picker-pick = Pick
# The preview column before a row is highlighted.
library-primitive-picker-preview-empty = Click a row to preview it.
# Under a symbol's preview.
library-primitive-picker-preview-facts = { $pins ->
        [one] { $pins } pin
       *[other] { $pins } pins
    } · { $parts ->
        [one] { $parts } part
       *[other] { $parts } parts
    }

## Document Options dialog of a library (mockup row 132)
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

## New Library dialog (mockup row 133)
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
# Refusal banner when the chosen path already holds a library. $file is its
# file name.
library-create-options-exists = A library already exists in this folder: { $file }.
# Tooltip of the disabled Create Library button while that refusal shows.
library-create-options-exists-tip = A library already exists in this folder. Choose another location first.
# Refusal banner when the chosen file name cannot be used on every system.
# $file is the file name.
library-create-options-bad-name = { $file } cannot be a library file name. It must end in .snxlib, must not use \ / : * ? " < > | or end in a dot or space, and must not be a name Windows reserves, such as CON.
# Refusal banner when the project already holds this library, not yet created.
# $file is its file name.
library-create-options-pending = { $file } is already waiting to be created. Save the project to create it, or choose another location.
# Tooltip of the disabled Create Library button while either refusal shows.
library-create-options-path-tip = Choose another location first.
# Refusal banner action: open the library already at the chosen path.
library-create-options-open-existing = Open existing library

## Library Updates (mockup row 135)
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

## Library Browser tab, Graphite (mockup row 118)

# Shown in place of the tab when its library is not mounted. $path is the
# library folder.
library-browser-not-mounted = Library not mounted: { $path }
# Hint under that line.
library-browser-reopen-hint = Re-open the library through File ▸ Library ▸ Open Library… or via the project tree.
# Header of the narrow column that marks a row without a bound symbol.
library-browser-column-sym = Sym
# Tooltip of that column header.
library-browser-column-sym-tip = Symbol bound
# Header of the narrow column that marks a row without a bound footprint.
library-browser-column-fpt = Fpt
# Tooltip of that column header.
library-browser-column-fpt-tip = Footprint bound
# Tooltip of the red mark of a row whose bound symbol cannot be found.
library-browser-symbol-missing = The bound symbol is missing
# Tooltip of the red mark of a row whose bound footprint cannot be found.
library-browser-footprint-missing = The bound footprint is missing
# Preview pane button: edit the selected row's fields.
library-browser-edit-details = Edit Details
# Preview pane button: bind a symbol to the selected row.
library-browser-pick-symbol = Pick Symbol…
# Preview pane button: bind a footprint to the selected row.
library-browser-pick-footprint = Pick Footprint…
# Tooltip of the preview pane's ⋯ button.
library-browser-more-actions = More actions
# Item of the ⋯ menu: delete the selected row (asks first).
library-browser-delete-selected = Delete Selected
# Note in the preview pane of a library that cannot be edited.
library-browser-read-only-note = This library ships with Signex and is not editable.
# A preview tile while its drawing is read.
library-browser-preview-loading = Loading…
# A preview tile whose drawing could not be read.
library-browser-preview-unreadable = Could not be read

# library-copy-recovery
# Copy to Library footer primary: copy the stock primitive into the selected library ($library is its name).
library-stock-copy-copy-to = Copy to { $library }
# library-browser-columns-and-row-style
# Tooltip of the column button at the right of the count line, and the
# heading of the chooser it opens.
library-browser-columns = Columns
# Tooltip of the Library panel button that shows one-line rows.
library-panel-rows-compact = Compact rows
# Tooltip of the Library panel button that shows rows with a preview thumbnail.
library-panel-rows-thumbnail = Rows with preview thumbnail
# range-filter-chip
# Chip of an applied range filter: column, operator (≥ ≤ =), number with its prefix, unit. "Value ≥ 1k Ω".
library-range-filter-chip = { $column } { $op } { $value } { $unit }
# Chip of an applied between filter: column, low, high, unit. "Resistance 1k – 100k Ω".
library-range-filter-chip-between = { $column } { $low } – { $high } { $unit }
# Tooltip of a range filter chip.
library-range-filter-chip-tip = Edit this filter
# Tooltip of the × on a range filter chip.
library-range-filter-remove-tip = Remove this filter
# Button that folds a finished range filter editor into its chip.
library-range-filter-done = Done

# symbol-delete-modal
# Title of the confirm a Delete of a symbol still in use opens (mockup row 137).
library-symbol-delete-title = Delete Symbol?
# picker-and-change-component
# Change Component header note: $reference is the part's designator, $sheet the sheet's file name.
library-change-component-note = { $reference } · { $sheet }
# Placeholder of Change Component's search field.
library-change-component-search = Search parts
# Filter chip: only parts drawn with the placed part's symbol.
library-change-component-same-symbol = Same symbol
# Filter chip: only parts with the placed part's value (its class's value parameter).
library-change-component-same-value = Same value
# Filter chip: only parts a distributor last reported in stock.
library-change-component-in-stock = In stock
# Tag of a listed part with as many pins as the placed part.
library-change-component-pins-same = same pins
# Tag of a listed part with another pin count; $row and $placed are whole numbers.
library-change-component-pins-differ = { $row ->
        [one] { $row } pin
       *[other] { $row } pins
    }, not { $placed }
# Tooltip of that tag; $reference is the placed part's designator.
library-change-component-pins-differ-tip = Pins do not match: { $row ->
        [one] { $row } pin
       *[other] { $row } pins
    }, { $reference } has { $placed }
# Card heading: the part placed now.
library-change-component-placed-now = Placed now
# Card heading: the picked part that replaces it.
library-change-component-replace-with = Replace with
# Shown beside the list until a part is picked; $reference is the placed part's designator.
library-change-component-pick-prompt = Pick a part in the list to compare it with { $reference }.
# Section heading of the Now / After table.
library-change-component-parameters = Parameters
# Column headings of the Now / After table.
library-change-component-column-parameter = Parameter
library-change-component-column-now = Now
library-change-component-column-after = After
library-change-component-column-keep = Keep now
# Keep now cell of a row the new part gives the same value.
library-change-component-same = same
# After cell of a user parameter the new part does not have.
library-change-component-not-in-part = not in part
# Caption after a user parameter's name.
library-change-component-user = user
# Names of the part-data rows of the table.
library-change-component-value = Value
library-change-component-footprint = Footprint
library-change-component-datasheet = Datasheet
# Section heading: what the change keeps.
library-change-component-kept = Kept
# What the change keeps; $matched and $total are whole numbers of pins.
library-change-component-kept-summary = Designator { $reference } · position and rotation · pins matched by number, { $matched } of { $total }
# Section heading of the scope choice.
library-change-component-apply-to = Apply to
# Scope choice: only the part the change started on.
library-change-component-scope-this = Only { $reference } on this sheet
# Primary button; $count is the whole number of parts changed.
library-change-component-apply = { $count ->
        [one] Change { $count } part
       *[other] Change { $count } parts
    }
# Primary button when the change loses connections or data.
library-change-component-apply-anyway = { $count ->
        [one] Change { $count } part anyway
       *[other] Change { $count } parts anyway
    }
# Footer caption of Change Component.
library-change-component-footer = One undo step. The PCB footprint changes on Update PCB.
# Change Component warnings; $unit is a whole number, $pins a comma-separated list of pin numbers.
library-change-component-warn-unit = The new part has no drawing for unit { $unit }.
library-change-component-warn-dangling = Connected pins with no match, their wires will dangle: { $pins }
library-change-component-warn-moved = Connected pins that sit elsewhere on the new part, their wires will no longer reach them: { $pins }
library-change-component-warn-gained = New pins landing on a wire they were not mapped to: { $pins }
# range-filter-chip glyph
# The × that removes an applied range filter chip.
library-range-filter-remove = ✕
