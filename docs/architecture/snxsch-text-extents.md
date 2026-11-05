# Stored text widths in `.snxsch` (#2029)

A schematic stores the width each text measured on the machine that saved it.
A machine without the text's font uses that width for hit boxes, selection
and label flags instead of the width of the substitute face. Where the font is
installed the live measurement always wins; the stored width is never read.

## Covered texts

Net, global and hierarchical labels, text notes, and the reference and value
text of a symbol. Not covered, each a known gap:

- Symbol-body text (text inside a library symbol's drawing): a follow-up.
- A child sheet's text: nothing uses its width.
- The height of a label flag is not stored; it follows the em of the face that
  draws.
- A hidden reference or value, and the reference of a power symbol (which
  draws none), are not measured on the canvas, so no record is kept for them:
  a save drops one that was there, and showing the text again measures it.
- A stored entry that cannot be read (a wrong type, a missing field, a bad
  key) is dropped on load with one Messages warning per file; the design opens.
- PDF and SVG export do not read stored widths. They size from the face they
  actually draw, as before.

## Placement and key

`[extras.sheet.text_extents]`, one table per text, keyed `"<uuid>:<field>"`
with the field `label`, `note`, `ref` or `value`:

```toml
[extras.sheet.text_extents."0190...:label"]
width_nm = 12345678
size_nm = 1270000
family = "Fira Mono"
text = "185edd1b28072179"
```

Integers only (nanometres). `family` is the family the text asked for (its own,
else the sheet default) and is left out when none was asked for. `size_nm` is
the text height measured at. `text` is 16 lowercase hex characters of the
SHA-256 of the exact UTF-8 measured. The width is the unrotated advance.

A key that is not `<uuid>:<field>` and a negative width match nothing and are
dropped on load.

## Staleness guard

A record is valid for a text only when its family, `size_nm` and text hash all
match what the text measures from now. On save, a record for a missing item or
from other inputs is dropped (like `note_font_families` entries of a deleted
note). A symbol's reference is measured per hierarchy instance: the file
records the symbol's own reference, and an instance whose designator differs
hashes differently and falls back.

## Which width wins

1. The requested face is installed (the resolved family equals the requested
   one, the bundled default counting for no family or `Iosevka`) and the
   measurement is final: the live width.
2. Otherwise a record that is still valid: its width.
3. Otherwise the substitute face's live width.

Before the installed-font scan has landed, a system family counts as missing.
The decision is `signex_types::schematic::decide_width_mm`, wired to the
canvas in `schematic_runtime/text_width.rs`. A stored width is never put in
the live cache (`TextWidths`), which stays derived data; and a text with a
stored record skips the cache while its face is missing, because the cache
is keyed by the resolved face, which is the default for every missing one.

## Writing

The record table is a field of `SchematicSheet` (`text_extents`), filled on
load and never changed by an edit or by the engine, so undo and redo cannot
lose it. A save refreshes it on the copy being encoded
(`Engine::serialize_with_extents`, `save_with_extents`): a live final
measurement in an installed face overwrites; a text without one keeps its
loaded record. A width the file cannot hold skips that entry and never fails
the save. The save paths that refresh: Save and Save As, Save All, and
autosave. Writers of a sheet with no open engine keep the valid loaded
records and measure nothing.

A sheet with no records writes no table, so its bytes are the ones it had
before.

## Format decision

No format-token bump: the token stays `snxsch/1`. The table is a nested table
under `[extras.sheet]`, which builds since #658 keep as an unknown key and
write back, so an older build that re-saves the file loses nothing. v0.15.0
drops it on re-save, as it does the font families (see `snxsch-font-family.md`).
