# `.snxsch` text note format (#1114)

A text note is plain or rich. A plain note is the note of mockup row 47,
drawn through the Signex markup. A rich note (mockup row 203) holds Typst
source as its text. Either way the text is the note's source: it is stored as
text in the note's own sheet, next to its position, and switching the format
never changes it. No rendered output is ever stored.

## On disk

| What | Where it lives |
|---|---|
| The text (plain text or Typst source) | `text` of the note's row in `[[extras.sheet.text_notes]]`, as before |
| The format | `[extras.sheet.note_formats]`, note uuid to format word |

**`note_formats`.** A table in `[extras.sheet]` from note uuid to a format
word. Only a rich note is written, as `"typst"`; a plain note has no entry,
and the table is omitted when no note is rich. A sheet with only plain notes
therefore re-saves byte-identical, and the format token stays `snxsch/1.1`.

The format is kept beside the notes rather than as a key in the typed
`text_notes` array, for the reason the note font family and the lock are
(`snxsch-font-family.md`, `snxsch-object-lock.md`): a build that predates it
drops unknown keys nested in that array on re-save, but keeps unknown keys of
the extras table.

The source goes through the TOML string escaping of the `text` key, so
newlines, tabs, `#`, `$`, quotes and backslashes round-trip exactly
(`format::note_format_tests`).

## Reading

- An entry whose note no longer exists matches nothing and is not written
  back.
- A word this build does not know (from a newer build) reads as plain. The
  entry is kept and written back while its note exists and stays plain;
  choosing a format for that note replaces it.

## Older builds

An older build keeps `note_formats` as an unknown key and writes it back, so
the format survives a round trip through it. It draws the note as plain text.

## Editing

The Format row of a text note in Properties (Plain / Rich text) and the
catalog command `set_text_note_format` set it, one undo step each, through
`Command::UpdateTextNoteProps`. A locked note refuses the change. A new note
starts plain.

## Not yet built

Compiling and drawing the Typst source (a pinned Typst version, vendored
packages, bundled fonts, the hash-keyed compile cache, vector drawing on the
canvas and in SVG and PDF) and the in-place editor of mockup row 203 are the
rest of #1114. Until they land, a rich note draws as a plain note does.
