# Sheet page settings in `.snxsch` (#2034)

Document Options ▸ Sheet Color and Page Origin are stored per sheet, edited
through undoable engine commands (`SetSheetColor`, `SetPageOrigin`) and
saved with the sheet.

## Stored fields

| Field | On disk | Absent means |
|---|---|---|
| `SchematicSheet.page.color` | `color = "#rrggbbaa"` under `[extras.sheet.page]` | Default: the theme's paper colour, also after a theme switch |
| `SchematicSheet.page.origin` | `origin = "upper_left"` under `[extras.sheet.page]` | Lower Left |

Only values that differ from the default are written, and the table is left
out when both are at the default, so a sheet that never changed them, and
every file written before the table existed, loads and re-saves unchanged
(`format::sheet_page_tests`). Reading never stops a sheet from opening: a
colour that is not `#RRGGBB[AA]` or an origin this build does not know shows
as the default and is kept raw, written back unchanged until the user picks a
value (any pick, Default included, replaces it); keys this build does not know
are kept too. A `page` value that is not a table opens the sheet, is written
back, and gives way to a real table once a value is set. No format-token bump.

## Sheet colour

A colour fills the paper on the canvas and on every exported page (PDF, SVG,
print preview). Paper is opaque: a stored alpha is ignored on the canvas as in
export. Black & White export ignores the sheet colour and prints black ink on
white paper, mapped against the theme paper (#1342). The picker offers
Default plus the symbol editor's presets; the symbol editor's own Sheet Color
(default Cream) is a separate setting and is unchanged.

## Page origin

Display only. Stored coordinates are always measured from the upper-left
corner of the paper, Y down; no coordinate is rewritten when the origin
changes. With Lower Left, the shown Y is `paper_height_mm - y`, the paper
height coming from the sheet's paper size (Custom included). The status bar,
Properties ▸ Position (and Start / End / Center rows), the pre-placement X/Y
readout, the ERC panel's location label and typed drawing coordinates go
through one mapping, refreshed when the origin or the paper size changes (a
half-typed coordinate box is dropped then), `SchematicSheet::page_y`, which is its own
inverse. X never changes. A rectangle's Position row is its top-left corner
(smallest stored Y), with that Y counted from the chosen origin. The exported
ERC report's `y` column holds the same shown Y as the ERC panel.

## Known gaps

- Arc start and end angles in Properties are measured with stored Y (down),
  whatever the origin.
- ERC marker contrast is tuned against the theme paper, not a custom sheet
  colour.
- Text built in the engine and the rule checker still shows stored Y: the
  lock refusal messages (`signex-engine` `lock.rs`) and ERC rule message
  bodies (e.g. `signex-erc` `rules/multiple_names.rs`). The ERC panel's
  location label reads from the origin.
