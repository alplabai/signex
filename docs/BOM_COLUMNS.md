# BOM columns and templates

The columns a bill of materials lists, the computed ones among them, and the
BOM template file that keeps a set of columns (#1988).

## Computed columns

A computed column has a name, its header, and an `=` expression over the
line's fields and the project variables, evaluated with the one expression
engine (`signex-expr`, see `docs/architecture/signex-expr.md`):

```text
Power   = VCC^2 / Value
Cost    = Quantity * UnitPrice
Half    = Rated / 2
```

A name in the expression is looked up in this order, the first that knows it
wins; a name matches exactly, then ignoring ASCII case:

1. The line's own columns: `Quantity` and `Qty` (the line's quantity),
   `FittedQty`, and on a line with a cached price `UnitPrice`, `BuyQty`,
   `ExtendedPrice` and `Stock`.
2. The part's fields: its user fields (`MPN`, `Rated`, ...), `Value`,
   `Footprint`, `Name`, `Description`, `LibRef` and `Designator`. `Value` is
   the value the sheet shows, so an `=` value is read evaluated (#1986).
3. The variables of the sheet the part is placed on (#1987).
4. The project variables (the `.snxprj` `parameters`, with the sheet's
   title block over them, as `${NAME}` reads them).

The cell shows the result with its derived unit (`2.5mW`, `330Ω`) and is
written as that text in CSV, XLSX and HTML.

A grouped line is evaluated once for each of its parts, each with the
variables of its own sheet. When every part gives the same result the cell
shows it; when they differ the cell says so: "Members differ" in the dialog,
`(members differ)` in an exported file. Two equal resistors on sheets with
different `VCC` are one line whose power differs.

A cell whose expression fails (an unknown name, a unit mismatch, an unpriced
line read for `UnitPrice`) is empty. The BOM dialog writes one Messages line
for each such cell when the column is ticked and when the BOM is exported,
naming the line by its designators, the column, the part it failed for on a
line of several parts, and the error:

```text
BOM line 3 (R1, R2): column Power for R2: unknown parameter 'VCC'
```

`bom_lines` (`docs/CLI.md`, "BOM lines") lists the same failures as its
`issues` instead.

## BOM templates

A BOM template keeps a set of columns in order, computed ones included: the
column preset of BOM Studio (mockup row 113). It is TOML with the token
`snxbomt/1` (`signex_types::bom_template`; the policy row is in
`docs/FORMAT_VERSIONS.md`):

```toml
format = "snxbomt/1"

[[columns]]
column = "designator"

[[columns]]
field = "MPN"

[[columns]]
name = "Power"
expression = "= VCC^2 / Value"
```

Each column is one of:

- `column`: a standard column by key: `name`, `designator`, `lib_ref`,
  `reference`, `qty`, `value`, `footprint`, `description`, `unit_price`,
  `buy_qty`, `extended_price`, `currency`, `stock`, `distributor`,
  `price_age`, `alternates` (a line's alternate parts, #2938);
- `field`: a part field by name;
- `name` with `expression`: a computed column.

A template with no token or a newer one, an unknown key, a column that is
none or several of these, a blank key or field name, a computed column
without a name, an expression that does not start with `=` or does not
parse, or two computed columns of one name is refused, naming the column
(from 1). Signex writes a template one way only, so a template it wrote,
read and written again is byte-identical.

## Implementation notes

These are the implementation's choices, open for the maintainer to confirm
(none is a recorded decision yet):

- The line's own columns are looked up before the part's fields, so a part
  field named `Quantity` does not change what `Quantity` means.
- A grouped line reads the line's `Quantity`, not 1 per part.
- A part whose units sit on several sheets reads the variables of the sheet
  of its first unit.
- On a grouped line, the first part that fails makes the cell an error, even
  when the other parts evaluate.
- A computed column does not read another computed column.
- The template file is its own format (`snxbomt/1`); where templates are
  kept (the project, a library, the config folder) and how the dialog
  saves and loads one are not built yet.
