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
library-stock-copy-no-writable = No writable library is open. Create a new one to copy into.
