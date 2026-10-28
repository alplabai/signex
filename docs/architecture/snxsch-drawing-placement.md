# Drawing placement in `.snxsch` (#1583)

A schematic drawing (`SchDrawing`: line, rectangle, circle, arc, polyline)
paints behind the circuit by default. A drawing can instead be placed above
the circuit: over symbols, wires, pins and pads, under text.

## Stored field

| Where | Field | On disk |
|---|---|---|
| Sheet | `SchematicSheet.drawing_placements` | `[extras.sheet.drawing_placements]`, drawing uuid to `"above"` |

Only `"above"` is written. No entry means behind, so a sheet that never chose
a placement is byte-identical to what a build without the field wrote
(pinned by `format::drawing_placement_tests`). An entry for a drawing that no
longer exists, a key that is not a uuid, or a value other than `"above"` is
ignored on load and gone after the next save.

## Format decision

No format-token bump: the token stays `snxsch/1`. The placements live in their
own table beside the drawings, not inside the typed `drawings` array, for the
reason the font families do (see `snxsch-font-family.md`): a build that
predates the field drops unknown keys nested in a typed array on re-save, but
keeps unknown keys of an `[extras.sheet]` sub-table.

## Order

The order among drawings is the order of the `drawings` array, first painted
first, with the behind and above drawings each keeping their relative order.
Arrange (Bring to Front, Send to Back, Bring Forward, Send Backward) reorders
that array through `Command::ArrangeDrawings`; the Properties placement row
goes through `Command::SetDrawingPlacement`. Both are one undo step.
