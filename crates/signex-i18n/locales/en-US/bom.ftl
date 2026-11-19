# Bill of Materials preview (#2058).

bom-properties = Properties
bom-tab-general = General
bom-tab-columns = Columns
bom-tab-rules = Rules
bom-section-items = BOM Items
bom-section-export = Export Options
bom-section-columns = Columns
bom-section-rules = Validation Rules
bom-section-issues = Issues
bom-grouping = Grouping:
bom-grouping-grouped = Grouped
bom-grouping-ungrouped = Ungrouped
bom-grouping-flat = Flat
bom-format = Format:
# File format names.
bom-format-csv = CSV
bom-format-xlsx = XLSX
bom-format-html = HTML
bom-include-dnp = Include DNP
bom-include-not-fitted = Include Not Fitted
bom-variant = Variant:
# The view with no variant applied.
bom-variant-base = Base
# $variant is the variant's name, or the word for the base view.
bom-variant-is = Variant: { $variant }
# $variant is the variant's name, or the word for the base view.
bom-current-variant = Current variant: { $variant }
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
bom-no-issues = No BOM issues
bom-no-issues-found = No issues found.
# Issue list heading; $count is how many errors (a whole number).
bom-issues-errors = Errors ({ $count })
# Issue list heading; $count is how many warnings (a whole number).
bom-issues-warnings = Warnings ({ $count })
# Issue list heading; $count is how many notes (a whole number).
bom-issues-notes = Notes ({ $count })
# Footer: $visible and $total are line counts (whole numbers). The English
# says "lines" for every count.
bom-status-visible = { $visible } of { $total } lines visible
# As above, followed by the price note $prices (one of the bom-price-*
# messages below).
bom-status-visible-prices = { $visible } of { $total } lines visible · { $prices }
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
# Footer issue count, one part per severity. $n is how many (a whole number,
# at least 1), for the plural form; $count is the same number as text,
# written without grouping.
bom-issue-count-errors = { $n ->
    [one] { $count } error
   *[other] { $count } errors
}
# As above: $n picks the plural form; $count is the same number written
# without grouping. Write { $count }, not { $n }.
bom-issue-count-warnings = { $n ->
    [one] { $count } warning
   *[other] { $count } warnings
}
# As above: $n picks the plural form; $count is the same number written
# without grouping. Write { $count }, not { $n }.
bom-issue-count-notes = { $n ->
    [one] { $count } note
   *[other] { $count } notes
}
# Joins two parts of the footer issue count, such as "2 errors, 5 warnings".
# $first is the error or warning count, $second the count after it (it may
# itself be a joined pair).
bom-issue-count-pair = { $first }, { $second }
