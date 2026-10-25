# Font family in `.snxsch` (#510)

A schematic stores the font *family name* a text item asks for. Whether that
family is installed is decided at render time on the machine that opens the
file, and the stored name is never rewritten.

## Stored fields

| Where | Field | On disk |
|---|---|---|
| Sheet default | `SchematicSheet.font_family` | `font_family = "..."` under `[extras.sheet]` |
| Symbol reference / value text | `TextProp.font_family` | in the `ref_text` / `val_text` tables of `[extras.symbols.<uuid>]` |
| Text note | `TextNote.font_family` | in the `[extras.sheet]` `text_notes` array |
| Net / global / hierarchical label | `Label.font_family` | optional `font_family` column of the `[sheets.labels]` TSV block |

Every field is `Option<String>`. `None` writes nothing, so a file that sets no
family is byte-identical to what a build without the field wrote (the
round-trip corpus pins this). A blank name is not stored: clearing and `""`
are the same edit.

## Format decision

No format-token bump. The token stays `snxsch/1`:

- The TOML fields are additive with `#[serde(default)]`, and the TSV column is
  not part of `SchLabelRow::columns()`. It travels as a foreign column, which
  the reader already accepts, and is present only when some label sets a
  family. Labels that set none write an empty cell in that column.
- A build that predates the field still opens the file. It keeps the sheet
  default (`[extras.sheet]` preserves unknown keys) and the label families
  (foreign columns are written back). It does **not** know the nested
  `font_family` key of a symbol's reference/value text or of a text note, and
  drops those on re-save. That one-way loss is accepted for the preview window
  instead of making every older build read-only with a token bump; a build
  that has the field never loses anything.

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
