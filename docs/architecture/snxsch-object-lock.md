# `.snxsch` object lock (#1573)

Wires, labels, text notes and drawings can be locked, as symbols can. A locked
object refuses move, rotate, mirror, delete and property edits (enforced in
`crates/signex-engine/src/lock.rs`) until it is unlocked.

## On disk

| Object | Where the lock lives |
|---|---|
| Wire (`[sheets.wires]`) | optional `locked` column |
| Label (`[sheets.labels]`) | optional `locked` column |
| Text note, drawing | uuid in `locked_objects` of `[extras.sheet]` |

**The `locked` column.** It is not a known column of the block: it rides the
foreign-column path (like the label `font_family` column). A locked row writes
`true`; every other row of the block writes an empty cell. The header appears
only when at least one row is locked, so a sheet that locks nothing re-saves
byte-identical. On read, `true` locks and an empty cell does not; any other
value (`TRUE`, `1`) is not read as a lock and stays a foreign cell.

**`locked_objects`.** A list of uuid strings in `[extras.sheet]`, omitted when
empty. The lock of a note or drawing is kept beside the objects rather than as
a key inside the typed `text_notes` / `drawings` arrays: a build that predates
the lock drops unknown keys nested in those arrays on re-save, but keeps
unknown keys of the extras table. An id naming an object that no longer exists
matches nothing on read and is not written back.

## Older builds

An older build reads a `locked` column as an unknown foreign column and writes
it back, and keeps `locked_objects` as an unknown key, so a lock survives a
round trip through it. It does not enforce the lock.

## Copy and paste

The clipboard carries the whole object, so a pasted copy of a locked object is
locked too.

## Merge and rubber band

Merging collinear wires never folds a locked wire away, and a wire the rubber
band adds is created unlocked. A move that would drag a locked wire along (a
symbol, sheet symbol or sheet pin with a locked wire on its pin, with wires
following) is refused or skipped.

## Commands the lock leaves open

On purpose, as for symbols:

- `SetSymbolLocked` and `SetItemLocked`: the way out of the lock.
- `SetTextFontFamily` on a locked label or text note: the font family is a
  display choice, as it is for a locked symbol's text.
- `ReorderObjects`, `ArrangeDrawings` and `SetDrawingPlacement` on a locked
  drawing: they change only paint order (front/back, above/behind the
  circuit), never geometry or content. `ReorderObjects` also applies to
  wires, labels and symbols, and wire order decides which wire survives a
  merge and which one `wire_to_break_at` picks.

Everything that moves, turns, mirrors, deletes or edits the content of a locked
object is refused (or skipped, for a group edit) by `Engine::execute` and
`Engine::without_locked`.

## Placement and the rubber band

A paste ghost is turned on a scratch engine that ignores locks, and the result
keeps every lock. Pasted wires are placed as copied
(`Command::PlaceWireSegmentAsIs`), so a split in the copy is not folded back.
The rubber band never slides a locked elbow leg (it bends on a new leg
instead) and never re-cuts a locked wire when it clears an overlap; an overlap
with a locked wire stays as it is.
