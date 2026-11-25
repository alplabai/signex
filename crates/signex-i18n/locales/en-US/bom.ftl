# Bill of Materials export dialog (#2058).

bom-properties = Properties
bom-tab-general = General
bom-tab-columns = Columns
bom-tab-rules = Rules
bom-section-items = BOM Items
bom-section-export = Export Options
bom-section-rules = Validation Rules
bom-section-issues = Issues
bom-grouping-grouped = Grouped
bom-grouping-ungrouped = Ungrouped
bom-grouping-flat = Flat
bom-format = Format
# File format names.
bom-format-csv = CSV
bom-format-xlsx = XLSX
bom-format-html = HTML
# One option for the parts a BOM leaves out: DNP and not-fitted parts of the
# active variant, parts excluded from the BOM and parts not on the board.
bom-include-not-fitted = Include not-fitted (DNP), excluded-from-BOM and off-board parts
# Caption before the variant picker in the toolbar.
bom-variant = Variant
# The view with no variant applied.
bom-variant-base = Base
# Placeholder of the toolbar filter; filtering only hides lines on screen.
bom-filter-placeholder = Filter lines
# Toolbar chip that opens the Rules tab; $count is a whole number, at least 1.
bom-chip-warnings = { $count ->
    [one] { $count } warning
   *[other] { $count } warnings
}
# Toolbar chip that opens the Rules tab; $count is a whole number, at least 1.
bom-chip-errors = { $count ->
    [one] { $count } error
   *[other] { $count } errors
}
bom-columns-hint = Ticked columns are exported. Drag to reorder.
bom-issues-hint = Click an issue to select its line.
# Where an issue sits: $line is the line number in the table, $part the
# line's value (or name).
bom-issue-line = Line { $line } · { $part }
# Export button, named by the chosen format.
bom-export-csv = Export CSV…
bom-export-xlsx = Export XLSX…
bom-export-html = Export HTML…
# Column picker names.
bom-column-name = Name
bom-column-description = Description
bom-column-designator = Designator
bom-column-value = Value
bom-column-footprint = Footprint
bom-column-lib-ref = LibRef
# The legacy column that lists the designators, under its old name.
bom-column-reference = Reference
bom-column-qty = Qty
bom-column-unit-price = Unit Price
bom-column-buy-qty = Buy Qty
bom-column-ext-price = Ext. Price
bom-column-currency = Currency
bom-column-stock = Stock
bom-column-distributor = Distributor
bom-column-price-age = Price Age
bom-column-alternates = Alternates
bom-no-issues-found = No issues found.
# Issue list heading; $count is how many errors (a whole number).
bom-issues-errors = Errors ({ $count })
# Issue list heading; $count is how many warnings (a whole number).
bom-issues-warnings = Warnings ({ $count })
# Issue list heading; $count is how many notes (a whole number).
bom-issues-notes = Notes ({ $count })
# Footer: $visible lines of $total are shown (whole numbers; the English says
# "lines" for every count); $parts is how many parts the BOM lists (the sum
# of the Qty column).
bom-status-visible = { $visible } of { $total } lines · { $parts ->
    [one] { $parts } part
   *[other] { $parts } parts
}
# As above, followed by the price note $prices (one of the bom-price-*
# messages below).
bom-status-visible-prices = { $visible } of { $total } lines · { $parts ->
    [one] { $parts } part
   *[other] { $parts } parts
} · { $prices }
# Price note when no line has a cached price.
bom-price-none = no cached prices
# Price note: $sums is the total per currency, such as "2.00 EUR + 1.50 USD".
bom-price-total = Total { $sums }
# As above, when some lines have no cached price; $count is how many lines
# (a whole number, at least 1).
bom-price-total-unpriced = Total { $sums } · { $count ->
    [one] { $count } line without a cached price
   *[other] { $count } lines without a cached price
}
# Validation rule names on the Rules tab.
bom-rule-missing-footprint = Missing footprint
bom-rule-missing-mpn = Missing MPN
bom-rule-duplicate-designator = Duplicate designator
bom-rule-empty-or-zero-qty = Empty or zero quantity
bom-rule-unit-field-conflict = Conflicting unit fields
