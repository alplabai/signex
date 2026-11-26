## The Design Notebook tab (#2795, mockup row 195): the project's notebook
## as Typst source beside its compiled pages, the side panel, the insert
## toolbar, the Problems strip, the status footer and the empty state.

notebook-tab-title = Design Notebook
notebook-tab-kind = NBK
notebook-tab-reading = Reading the notebook…
notebook-tab-read-failed = The notebook could not be read: { $why }

## Toolbar
notebook-tab-view-source = Source
notebook-tab-view-split = Split
notebook-tab-view-preview = Preview
notebook-tab-insert-reference = Insert reference
notebook-tab-insert-calculation = Calculation
notebook-tab-insert-measurement = Measurement
notebook-tab-insert-figure = Figure
notebook-tab-insert-block = Block
notebook-tab-block-note = Note
notebook-tab-block-issue = Issue
notebook-tab-block-decision = Decision
notebook-tab-export-pdf = Export PDF…
notebook-tab-export-pdf-tip = Export the notebook as a PDF, starting from the page in view
notebook-tab-save = Save
# $key is the shortcut as this system writes it (Ctrl+S, or ⌘S on macOS).
notebook-tab-save-tip = Save the notebook ({ $key })

## Side panel
notebook-tab-outline = Outline
notebook-tab-pages = Pages
notebook-tab-references = References
notebook-tab-outline-sections = Sections
notebook-tab-outline-figures = Figures and tables
notebook-tab-outline-blocks = Blocks
notebook-tab-outline-empty = No sections yet.
# $kind is figure or table; $number counts each kind on its own.
notebook-tab-figure-row = { $kind ->
    [table] Table
   *[figure] Figure
} { $number } · { $caption }
notebook-tab-pages-empty = No pages yet.
notebook-tab-page-row = Page { $number }
notebook-tab-references-empty = No references yet.
notebook-tab-kind-component = part
notebook-tab-kind-pin = pin
notebook-tab-kind-net = net

## Editor
# $file is the notebook file's name inside notebook/.
notebook-tab-editor-file = { $file } · Typst
notebook-tab-recompiles = Recompiles as you type
notebook-tab-errors = { $count ->
    [one] 1 error
   *[other] { $count } errors
}
# The header between the +++ lines, folded to one line.
notebook-tab-header-line = +++ header (TOML) · { $count ->
    [one] 1 reference
   *[other] { $count } references
} · managed by Signex +++
notebook-tab-go-to-error = Go to error
notebook-tab-problems = Problems
notebook-tab-last-good = last good compile { $time }
notebook-tab-no-problems = No problems.
# $file is the file inside notebook/, then the line and column of the problem.
notebook-tab-problem-at = { $file } { $line }:{ $column }
notebook-tab-problems-more = and { $count } more

## Reference picker
notebook-tab-picker-placeholder = Find a part or net
notebook-tab-picker-count = { $shown } of { $count }
# $call is what Enter writes, such as #component("R7").
notebook-tab-picker-hint = Enter inserts { $call } · ↑↓ choose · Esc closes

## Preview
notebook-tab-preview-page = page { $page } of { $count }
notebook-tab-previous-page = Previous page
notebook-tab-next-page = Next page
notebook-tab-compiling = Compiling…
notebook-tab-preview-none = The preview shows here once the notebook compiles.
notebook-tab-failed-banner = Compile failed at line { $line }. The preview shows the last good compile ({ $time }) until the error is fixed.

## Status footer
# The whole status line: the compile, the references, the blocks and the save state.
notebook-tab-status = { $compile } · { $references } · { $blocks } · { $saved }
notebook-tab-status-compiled = Compiled in { $ms } ms · { $pages ->
    [one] 1 page
   *[other] { $pages } pages
}
notebook-tab-status-failed = Compile failed · { $errors ->
    [one] 1 error
   *[other] { $errors } errors
} · preview from { $time }
notebook-tab-status-references = { $count ->
    [one] 1 reference
   *[other] { $count } references
}
notebook-tab-status-blocks = { $calculations ->
    [one] 1 calculation
   *[other] { $calculations } calculations
} · { $measurements ->
    [one] 1 measurement
   *[other] { $measurements } measurements
} · { $issues ->
    [one] 1 issue
   *[other] { $issues } issues
}
notebook-tab-status-unsaved = Unsaved, written on Save
notebook-tab-status-saved = Saved
notebook-tab-status-saved-at = Saved { $time }
notebook-tab-status-saving = Saving…
notebook-tab-status-save-failed = Save failed: { $why }
notebook-tab-status-read-only = Read-only: a newer Signex wrote this notebook
notebook-tab-status-cursor = Ln { $line }, Col { $column } · Typst · UTF-8
notebook-tab-status-none = No notebook

## Empty state
notebook-tab-empty-title = This project has no Design Notebook yet
notebook-tab-empty-body = Keep calculations, bench measurements, issues and decisions next to the design, written in Typst. Parts, pins and nets are references that follow renames and cross-probe to the schematic.
notebook-tab-template-report = Design report
notebook-tab-template-report-body = Title block, contents, Introduction, Requirements, Design, Bring-up, Decisions, Change log
notebook-tab-template-lab = Lab notebook
notebook-tab-template-lab-body = Dated entries, measurements and attachments, no fixed sections
notebook-tab-create = Create from template
notebook-tab-generate = Generate from project…
notebook-tab-generate-tip = Build a design report from the project into its own notebook file
notebook-tab-import = Import .typ files…
notebook-tab-import-title = Import Typst files
notebook-tab-import-filter = Typst files
notebook-tab-empty-note = Creates notebook/design.snxnbk on Save; nothing is written to disk before that.

## Commands: why a Design Notebook command (notebook_*, list_annotations,
## export_notebook_pdf, the measurement commands) is refused. $command is
## the command id, $operand an operand name, $value what was given.
# A reason another part of Signex gave (a document that cannot be found).
notebook-tab-cmd-because = { $why }
notebook-tab-cmd-project-closed = the project is no longer open
notebook-tab-cmd-not-open = the project's Design Notebook is not open: open it with Project ▸ Design Notebook
notebook-tab-cmd-read-only = the notebook is read-only: a newer Signex wrote it
notebook-tab-cmd-compile-stopped = the compile did not finish: { $why }
notebook-tab-cmd-read-stopped = the notebook read stopped: { $why }
notebook-tab-cmd-files-unreadable = the notebook files could not be read: { $why }
notebook-tab-cmd-needs-operand = `{ $command }` needs the operand `{ $operand }`
notebook-tab-cmd-not-in-project = `{ $command }` reads a project's notebook, and { $name } is not part of an open one
notebook-tab-cmd-needs-name = `kind={ $kind }` needs the operand `name`
notebook-tab-cmd-not-a-pin = { $value } is no pin: write it as `U3.12`
notebook-tab-cmd-bad-kind = `kind` is component, pin, net or one of { $kinds }
notebook-tab-cmd-bad-line = `line` is a line number of the notebook file, from 1
notebook-tab-cmd-bad-view = `view` is source, split or preview
notebook-tab-cmd-bad-panel = `panel` is outline, pages or references
notebook-tab-cmd-needs-target = `{ $command }` needs one of the operands `net`, `part` or `pin`
notebook-tab-cmd-one-target = `{ $command }` takes one of `net`, `part` or `pin`, not several
notebook-tab-cmd-bad-pin = `{ $command }`: `pin` is a part and a pin, `U3.3`, not { $value }
notebook-tab-cmd-bad-reading = `{ $command }`: { $value } is not a block id such as MEAS-005
notebook-tab-cmd-needs-file-or-id = `{ $command }` needs one of the operands `file` (an attachment) or `id` (a reading)
notebook-tab-cmd-bad-pages = `pages` is all, current, a page or a range such as 3-7, not { $value }
notebook-tab-cmd-bad-paper = `paper` is document, a4 or letter, not { $value }
notebook-tab-cmd-bad-stale = `stale` is warn or fail, not { $value }
notebook-tab-cmd-bad-annotations = `annotations` lists note, calculation, measurement, issue and decision, not { $value }
notebook-tab-cmd-bad-block-id = `id` is a block id such as DEC-002 or CALC-2.1, not { $value }
notebook-tab-cmd-bad-block-kind = `kind` is note, calculation, measurement, issue or decision, not { $value }
# $file is a chapter (notebook/chapters/power.snxnbk): the open tab holds the
# notebook file, so a change written to a chapter behind it is refused.
notebook-tab-cmd-chapter-held = { $file } is a chapter of the notebook open in its tab: close the tab, or edit the chapter by hand
# $what is "the attachment scope/a.csv".
notebook-tab-cmd-not-found = { $what } is not in the notebook any more
notebook-tab-cmd-in-chapter = { $what } is in the chapter { $file }: edit it there
notebook-tab-cmd-bad-target-ext = `path` is a { $ext } file, not { $value }
notebook-tab-cmd-snx-target = `path` names a Signex document ({ $value }): pick another file
notebook-tab-cmd-target-exists = { $value } exists: pass overwrite=true to replace it
# A new notebook's first Save finds a file at its path (written meanwhile).
notebook-tab-cmd-file-appeared = { $path } was written since this notebook was started: it is not replaced
