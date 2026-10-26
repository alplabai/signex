# Font family in `.snxsch` (#510)

A schematic stores the font *family name* a text item asks for. Whether that
family is installed is decided at render time on the machine that opens the
file, and the stored name is never rewritten.

## Stored fields

| Where | Field | On disk |
|---|---|---|
| Sheet default | `SchematicSheet.font_family` | `font_family = "..."` under `[extras.sheet]` |
| Symbol reference / value text | `TextProp.font_family` | `ref_font_family` / `val_font_family` in `[extras.symbols.<uuid>]` |
| Text note | `TextNote.font_family` | `[extras.sheet.note_font_families]`, note uuid to family |
| Net / global / hierarchical label | `Label.font_family` | optional `font_family` column of the `[sheets.labels]` TSV block |

Every field is `Option<String>`. `None` writes nothing, so a file that sets no
family is byte-identical to what a build without the field wrote (pinned
against the output of trunk before the field). A blank name is not stored.

## Format decision

No format-token bump: the token stays `snxsch/1`, and a build that predates
the field loses nothing when it opens and re-saves the file.

Each family is stored where an older build already keeps unknown data:

- `[extras.sheet]` and `[extras.symbols.<uuid>]` preserve unknown keys, so the
  sheet default, the symbol-field families and the note table survive.
- The label column is not in `SchLabelRow::columns()`. It is a foreign TSV
  column, which older builds keep per row and write back.
- Families are deliberately **not** nested inside `ref_text`, `val_text` or the
  `text_notes` array. Those are typed tables an older build rebuilds from the
  keys it knows, dropping the rest.

`fixtures/font_family/` holds a file this build writes and the same file after
an older build (trunk at `2dcb6169`) parsed and re-saved it; the tests assert
every family comes back from the re-saved file.

Reader: an entry in the note table whose note is gone (an older build deleted
it) matches nothing, loads without a message, and is not written again. A
symbol's families live in the symbol's own table, so they go with the symbol.

## Resolution

Item family, then the sheet default, then the bundled Iosevka. The first one
named is the only one considered: an item whose own family is missing draws in
Iosevka, not in the sheet default, so the substitution is the same bundled
face on every machine.

"Installed" means a system font or one of the faces bundled with the app
(Iosevka, Roboto). The check is a cached catalog built by a background scan
at startup; until it lands only the bundled families count as installed.

The CPU canvas replay and the GPU text pipeline read the resolved family from
the same `TextItem::family` through `TextItem::family_or`.

## Export

PDF and SVG export can only outline the faces compiled into the app. A text
that names Roboto or Iosevka exports in that face; any other family, installed
or not, exports in Iosevka. Text that names no family keeps the face it had
before this field existed. Exporting an installed non-bundled family therefore
differs from the canvas until project-embedded fonts exist.

## Not covered

PCB board text, and carrying font files in the project (`fonts/` with a
content hash and licence note), are separate work.
