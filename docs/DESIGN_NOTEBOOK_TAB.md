# The Design Notebook tab

The project's Design Notebook as a document tab (#2795, mockup row 195):
the Typst source of `notebook/design.snxnbk` in a code editor beside its
compiled pages. The file format is in
[NOTEBOOK_ANNOTATIONS.md](NOTEBOOK_ANNOTATIONS.md) and
[FORMAT_VERSIONS.md](FORMAT_VERSIONS.md); the decisions are the 2026-11-25
Design Notebook rows of [DECISIONS.md](DECISIONS.md).

Code: `crates/signex-app/src/app/notebook_tab/` (state, tasks, view) over
`crates/signex-notebook` (file, outline, highlighting, insert text,
compile, page colours, templates).

## The tab

- One tab per project, titled `<project> — Design Notebook`, on the main
  window. It is not undocked and History does not restore it.
- The tab's state stays with the project while the project is loaded.
  Closing the tab keeps unsaved text: the tab shows it again when it
  reopens, and Save All and the close and quit prompts list and save it.
- With no notebook file the tab shows the empty state. Nothing is written
  to disk before Save.

## Layout

From the top: the toolbar, then the side panel, the editor and the
preview, then the status footer.

**Toolbar.** Source | Split | Preview. Insert reference, Calculation,
Measurement, Figure, Block (Note, Issue, Decision). At the end: Export PDF…
and Save. Export PDF… opens the PDF export dialog (`docs/NOTEBOOK_PDF.md`)
on the page in view; it is greyed until the notebook is saved, since the
export reads the file on disk. With no
notebook the view switch is greyed and the insert buttons are hidden. A
file a newer Signex wrote opens read-only: no insert buttons, Save greyed.

**Views.** The tab opens in Split. Source shows the editor alone, Preview
the pages alone. The view is kept per project while it is loaded.

**Side panel.** Outline, Pages or References.

- Outline: the sections, numbered as the report numbers them (`2.1`), then
  figures and tables (each numbered on its own), then the annotation blocks
  with their ids. It is read from the source, so it is there while the
  notebook does not compile; chapters show where they are included. A row
  in the notebook file moves the cursor to its line.
- Pages: a thumbnail of each page of the last good compile; a press shows
  that page in the preview.
- References: each part, pin and net the notebook names, once.

**Editor.** The header between the `+++` lines is one folded line that
Signex owns: `+++ header (TOML) · 31 references · managed by Signex +++`.
The body follows, with line numbers in the file's own numbering, Typst
colours (headings, functions, references, strings, numbers, math, labels,
comments, raw text) and a band on the cursor's line. A line with an error
has a red dot in the gutter and a red wash; a warning is amber. Under the
editor: the first error with its hints and Go to error, then the Problems
strip with every problem as `file line:column message`.

**Preview.** The page under the page navigator (`page 3 of 12`, Previous
page, Next page). The page follows the theme: under Graphite Dark it is
the theme's paper with its text as ink; a coloured mark keeps its colour.
The PDF always prints black on white. When a compile fails the preview
keeps the last good compile under a red banner: "Compile failed at line
56. The preview shows the last good compile (14:02:18) until the error is
fixed." with Go to error.

**Footer.** The compile result (`Compiled in 41 ms · 12 pages`, or
`Compile failed · 1 error · preview from 14:02:18`), the references, the
calculation, measurement and issue counts, the save state (`Unsaved,
written on Save`, `Saved 14:02`, read-only), and `Ln 48, Col 45 · Typst ·
UTF-8`.

**Empty state.** "This project has no Design Notebook yet", two template
cards (Design report: title block, contents, Introduction, Requirements,
Design, Bring-up, Decisions, Change log; Lab notebook: dated entries, no
fixed sections), Create from template, Generate from project… (the
Generate options, `docs/DESIGN_NOTEBOOK_GENERATE.md`) and Import .typ files…, which starts a notebook from the
picked files.

## Compiling

Each edit asks for a compile. One runs at a time, off the UI thread; an
edit made while it runs asks for one more compile when it lands, so the
preview is never more than one compile behind. The compile reads the text
as typed and the chapter files as read when the tab opened. `datetime.today()`
is today's date in the preview.

## Keys

On the tab: Ctrl+S saves the notebook, Ctrl+\ cycles Source, Split and
Preview, F8 moves to the next problem (wrapping), Ctrl+Shift+M inserts a
measurement. Typing `@` opens the reference picker instead of typing an
`@` (Typst reads `@` as a label reference); in the picker Up and Down move,
Enter inserts the stored call (`#component("R7")`), Esc closes it. Esc
also closes the Block menu.

## Inserts

- A reference is the markup call: `#component("R7")`, `#pin("U3", "12")`,
  `#net("+3V3")`.
- A block gets the next free id of its kind. A Calculation is numbered
  within the top-level section of the cursor (`CALC-2.1`, `CALC-2.2`); the
  other kinds are numbered straight through (`MEAS-010`, `ISS-005`,
  `DEC-001`, `NOTE-001`). An Issue starts `open`, a Decision `proposed`.
- A block or a figure starts on a line of its own.

The picker lists the parts, then the nets, of the design the project's
last notebook read built (mockup row 196), filtered as you type; `U3.`
lists the pins of U3. The read starts when the tab opens.

## Margin lane

Beside the preview (Split or Preview; opening it from Source switches to
Split), the lane shows the card of a reference picked in References, the
timeline of a net, part or pin opened from a card, a notebook badge or a
timeline tab (mockup row 199: its tabs, export CSV and Close), and the card
of each stale attachment: Re-measure… inserts a Measurement block at the
cursor, Mark still valid writes the current part values into the text (an
edit, written on Save), Show change cross-probes the part. References lists
the references resolved against the design with their uses, a stale one in
the warning colour, under "31 references · 3 stale"; the status footer adds
the measurement counts.

## State

The notebook document (the path, where its read stands, the file, the body
as typed, the header lines, the chapters, the save state) is a
`signex_notebook::NotebookDocument` in `DocumentState::notebooks`; the
editor, the view, the panels, the compile and preview, the picker and the
lane are the tab's state in `UiState::notebook_tabs`.

## Saving

Save writes the notebook file atomically (temporary file, sync, rename)
and makes the `notebook/` folder when it is missing. An unchanged header
is written back byte for byte. The save of the tab runs off the UI
thread; Save All and the close and quit prompts write on the calling
thread, as they do for every other document.

## Commands

Each names the project with `document` (the `.snxprj`, or any document of
the project). JSON shapes:

- `notebook_compile {document}` compiles the notebook (the text as typed
  when the tab holds it, else the file) and answers with `notebook` (the
  path), `exists`, `unsaved`, `compiled`, `compile_ms`, `pages` (number and
  size in points), `problems` (`severity`, `file`, `line`, `column`,
  `message`, `hints`), `errors`, `warnings`, `outline` (`sections`,
  `figures`, `blocks`), `references` and `blocks` (counts per kind). With
  no notebook file: `{"notebook": "...", "exists": false}`.
- `notebook_insert {document, kind, name?, line?}` writes what an insert
  button writes into the open tab: `kind` is `calculation`, `measurement`,
  `figure`, `note`, `issue` or `decision`, or `component`, `pin` or `net`
  with `name` (`R7`, `U3.12`, `+3V3`). `line` (from 1, in the file) moves
  the cursor there first. Answers `inserted`, `id` and `line`.
- `notebook_view {document, view?, panel?, next_problem?}` picks the view
  (`source`, `split`, `preview`) and the side panel (`outline`, `pages`,
  `references`), and with `next_problem=true` moves the cursor to the next
  problem. Answers `view`, `panel`, `page`, `cursor` and `problem`.

`notebook_insert` and `notebook_view` act on the open tab and are refused
while it is not open. None of the three is run from a key or a menu
without its operands.

## Not yet

- Page numbers in the outline, the preview following the cursor's page,
  and a click in the preview moving the cursor.
- Find (Ctrl+F) and folding per section in the editor.
- The inline peek is drawn under the editor, not under its line.
