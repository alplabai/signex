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
notebook-tab-export-pdf-tip = Export PDF is not available yet
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
notebook-tab-picker-none = No part or net matches.
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
notebook-tab-generate-tip = Generate from project is not available yet
notebook-tab-import = Import .typ files…
notebook-tab-import-title = Import Typst files
notebook-tab-import-filter = Typst files
notebook-tab-empty-note = Creates notebook/design.snxnbk on Save; nothing is written to disk before that.
