# Design Notebook PDF export

Export Design Notebook PDF (mockup row 202, #2611) writes a project's Design
Notebook as a PDF file. It opens from File ▸ Export ▸ Design Notebook PDF…,
the notebook toolbar's Export PDF… and the command `export_notebook_pdf`; the
command line runs it as `signex-cli notebook export-pdf`. All of them run one
export, `signex_notebook::export::export_pdf`, which is also the export of the
Design Notebook PDF output of an Output Job.

The export rules live in the UI-free crate `signex-notebook`
(`crates/signex-notebook/src/export/`); the dialog and the command are the
slice `crates/signex-app/src/app/notebook_pdf/`, the view
`app/view/dialogs/notebook_pdf/`.

## What is exported

The notebook is the project's `notebook/design.snxnbk` (another file with the
command's `notebook` operand), with the chapter files it includes, as saved on
disk. Writing the file is the person's own action (Export), so it follows the
rule that nothing is written without one.

The PDF is reproducible: the same notebook and project give the same file,
byte for byte. No creation time and no random id is written; the PDF id is
hashed from the project name and the notebook path; `datetime.today()` answers
the revision date of the title block (`report.with(date: "YYYY-MM-DD")`) and
nothing else. Pages print black on white whatever the theme.

## Settings

| Setting | Values | Default |
|---|---|---|
| Pages | all, the page the notebook tab shows, a range (`3-7`) | all |
| Paper | from the document, A4, Letter (the pages flow again) | from the document |
| Variant | Base and the project's variants | Base |
| Title block and revision | on, off | on |
| Contents | on, off | on |
| PDF bookmarks | on, off | on |
| Python cell source | on, off (results and plots always print) | off |
| Annotation blocks | Note, Calculation, Measurement, Issue, Decision | all |
| Stale references | Warn, Fail the export | Warn |
| Save to | a path, relative to the project folder or not | `Project Outputs/notebook/<project>-<document>-rev<R>.pdf` |

The default file name takes the document number and revision from the title
block: project `buck-3v3`, `doc: "DN-0042"`, `rev: "B"` give
`buck-3v3-DN-0042-revB.pdf`. With no document number the notebook file's stem
stands in; with no revision the `-rev` part is left out; a character a file
name cannot hold becomes `_`.

## Python cells

The export runs the Python cells before it compiles, as Run all does: every
cell with no result or an out-of-date one runs, and the pages print each
cell's result; the source prints only with Python cell source on
(`docs/NOTEBOOK_CELLS.md`). Until the Python runtime is built the app runs
them with no runtime, so a cell prints its cached result, greyed, with the
slim-install note.

## Stale references

Before it exports, the dialog checks every part, pin and net the notebook
names against the design. A reference is stale when the design no longer has
it (renamed or deleted after it was written). The banner lists the stale
ones; Review… writes each to Messages. Under Warn the export runs (the button
reads Export anyway) and a stale reference prints as plain text with a mark
in the left margin; under Fail the export is refused.

The check reads the named project's design itself, on the export's worker:
its sheets (the open ones as edited), their designators, every pin by number
and by name, and the nets, so a project not in view is checked too and a pin
the part no longer has is stale. When the design cannot be read whole (a
sheet missing or unreadable, or no sheet at all) nothing is called stale, and
under Fail the export is refused instead of passing unchecked.

The file is a `.pdf`, never a Signex document (`.snx*`). The command's `path`
is replaced, like the default file, only with `overwrite=true`; in the dialog
the person picked it.

## Bookmarks

With PDF bookmarks on, the PDF lists every section heading, every subsection
heading under its section, and every figure and table that has a caption,
under the section it is in. Clicking a bookmark opens its page. The done card
and the command's answer count them (`sections`, `subsections`, `figures`,
`tables`), read from the notebook's source.

## What the export passes to the page

The export compiles with its settings as Typst `sys.inputs`, read by the
bundled `@signex/notebook` package; the preview passes none, so it reads as
before.

| Input | Meaning |
|---|---|
| `signex-export` | `true` while exporting |
| `signex-paper` | `a4` or `us-letter`; absent keeps the document's paper |
| `signex-title-block`, `signex-contents`, `signex-bookmarks` | `true` or `false` |
| `signex-python-source` | `true` prints the source of Python cells |
| `signex-hidden-kinds` | the annotation kinds left out, comma separated |
| `signex-variant` | the variant the references resolve in |
| `signex-stale` | the names of the stale references, one per line |

The title block, the contents, the paper and the figure bookmarks apply to a
notebook that uses the report layout (`#show: report.with(...)`).

## The done card

After the write, the card shows the file name, the pages, the paper, the
size, the references and how many are stale, the variant, the bookmarks
written and the start and end of the SHA-256 checksum, and says whether it is
the checksum of the last export of the same file this session. Show in folder
and Open PDF launch through the platform's file manager and viewer.

## The command and the command line

```
export_notebook_pdf document=<project or one of its documents> [path=<file>]
    [overwrite=true] [notebook=<file>] [pages=all|current|<n>|<n>-<m>] [paper=document|a4|letter]
    [variant=<name>] [annotations=all|none|<kind>,...] [stale=warn|fail]
    [title_block=<bool>] [contents=<bool>] [bookmarks=<bool>] [python_source=<bool>]
```

In a window the command opens the dialog. Run with its operands it exports
and answers once the file is written: the summary (`pages`, `page_count`,
`first_page`, `last_page`, `paper`, `bytes`, `references`, `stale`,
`variant`, `bookmarks`, `sha256`, `title`, `warnings`) with `path`,
`notebook` and `project_dir`. A stale reference under `stale=fail`, a notebook
that does not compile or a page selection that names no page is refused with
the reason. It needs the `export` capability.

```
signex-cli notebook export-pdf --project buck-3v3.snxprj [--out <file>] [--overwrite]
    [--notebook generated/design-report] [--pages ...] [--paper ...]
    [--variant ...] [--annotations ...] [--stale warn|fail] [--no-title-block]
    [--no-contents] [--no-bookmarks] [--python-source] [--json] [run options]
```

`notebook export-pdf` runs `export_notebook_pdf` with the export capability;
a notebook named without an extension is a `.snxnbk` file. See
[CLI.md](CLI.md), "Design Notebook".

## Output Jobs

Add as Output Job output… turns the dialog's settings into the
`design_notebook_pdf` output (`signex_notebook::export::NotebookPdfOutput`):
the notebook, enabled, and the settings above, the Current page kept as a
one-page range since a job has no current page. Export Jobs does not open as
a document tab yet, so the link is greyed with a note until it does.

## Not done yet

- The project PDF (the schematic export) does not carry the notebook's pages
  or their bookmarks yet (#2611); the notebook PDF does.
- The unsaved text of an open notebook tab is not exported; the file on disk
  is.
