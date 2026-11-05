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
(`format::sheet_page_tests`). Reading is lenient like `[extras.sheet.frame]`:
a colour that is not `#RRGGBB[AA]` or an unknown origin reads as the default,
and keys this build does not know are kept on save. No format-token bump.

## Sheet colour

A colour fills the paper on the canvas and on every exported page (PDF, SVG,
print preview); Black & White export maps ink against it. The picker offers
Default plus the symbol editor's presets; the symbol editor's own Sheet Color
(default Cream) is a separate setting and is unchanged.

## Page origin

Display only. Stored coordinates are always measured from the upper-left
corner of the paper, Y down; no coordinate is rewritten when the origin
changes. With Lower Left, the shown Y is `paper_height_mm - y`, the paper
height coming from the sheet's paper size (Custom included). The status bar,
Properties ▸ Position (and Start / End / Center rows) and typed drawing
coordinates go through one mapping, `SchematicSheet::page_y`, which is its own
inverse. X never changes.
