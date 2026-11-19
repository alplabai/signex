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
