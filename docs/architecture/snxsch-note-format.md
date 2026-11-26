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

## Frame and Size

A rich note's Frame and Size sections (mockup row 203) are kept in
`[extras.sheet.note_layouts]`, note uuid to a key table, beside the notes for
the reason the format is:

| Key | Value | Default (not written) |
|---|---|---|
| `frame` | `none`, `border` or `filled` | `none` |
| `border_mm` | outline width in mm | Auto: the sheet's line width |
| `padding_mm` | space inside the frame, mm | `0` |
| `size` | `fit_text`, `fixed_width` or `fixed_box` | `fit_text` |
| `width_mm` | box width of Fixed width and Fixed box | unset: 50.8 mm |
| `height_mm` | box height of Fixed box | unset: 25.4 mm |

An entry holds only what differs from the default, in key order, and a note
with nothing set has none, so the table is absent until a section is used.
A key this build does not know, or a word it does not read, is kept for its
note and written back in place; a length that is not a finite number at or
above zero reads as unset. The sections are kept when the note goes back to
plain, which ignores them (`format::note_layout_tests`).

## Drawing

A rich note compiles in the Design Notebook's Typst world
(`signex_notebook::note`): Typst 0.15.1, the fonts bundled with Typst, the
`@signex/notebook` package and nothing else. No file, package download or
system font is read, so a note draws the same on every machine; `#import` of
another package and `#read` or `#image` of a file are compile errors. The
compile lays the source out on a page as wide as the Size section's width
(less the padding), or as wide as its longest line for Fit text, at the
note's text size (its capitals, as every sheet text) and with its Justify H.
The result is lowered to vector paths in mm (glyph outlines and shapes); the
canvas, the SVG export and the PDF export all draw those paths. Typst's
default black draws in the note's ink, any other colour as written.

The box (the drawing plus the padding, or the set width and height) is placed
against the note's position by Justify H and V and turned with the note. A
Fixed box leaves out every glyph or shape that reaches past it and carries a
small corner mark.

**Cache.** A compile is keyed by a hash of the source, the width, the text
size, Justify H, the Typst release and the bundled package version. The app
compiles in a task every rich note of the open sheets whose key it has not
seen, once; an unchanged note never compiles again and the same text on many
sheets compiles once. The editor's typing compiles after a 150 ms pause.

**Errors.** A note that does not compile keeps its last good drawing on the
sheet with a red corner mark, and its first error goes to Messages once per
text, with Show on the note. A note that never compiled draws its source as
plain text. The SVG and PDF export compile each rich note as they write it
and print the source as text when it does not compile.

## The in-place editor

Double-click a rich note, choose Edit… beside its Text in Properties, or
choose a search hit inside its source: the editor opens over the sheet above
the note (below it when there is no room), drawn in Graphite Dark in both
themes (mockup row 203, decided as drawn). The source has line numbers and
the notebook editor's colours; the preview beside it is the note as it will
draw, recompiled as you type, the last good drawing kept under an error.
The toolbar wraps the selection in bold, italic, a heading, a list, math, a
link or a table (the empty markup when nothing is selected). Ctrl+Enter or
Apply makes the typed text the note's in one undo step; Esc or Cancel drops
it. A search hit opens the editor with the cursor on the match.

**Links.** Ctrl+click on a link of a rich note, on the sheet or in the
preview, opens it: an `http(s)` address in the browser, or a path relative to
the project folder in its viewer. A path that leaves the project folder or
names another scheme is refused. A plain click selects the note.

**Commands.** `set_text_note_frame`, `set_text_note_size`,
`apply_rich_note_edit`, `edit_rich_note`, `cancel_rich_note_edit`,
`wrap_rich_note_selection`, `open_rich_note_link` and `compile_rich_note`
(`docs/CLI.md`, "Rich text notes").

## Not yet built

A PDF link annotation for a note's link (the PDF draws the link text only),
images inside a note, a per-document record of the Typst release, and a
persistent cache across sessions.
