# Annotations panel (#165): the annotation blocks of the project's Design
# Notebook files (notebook/*.snxnbk), and the Messages lines of their reading.

## Panel name (dock tab, title, View menu row and panel list)
panels-kind-annotations = Annotations

## Tools
annotations-search = Search annotations
annotations-refresh = Refresh
annotations-kind-note = Note
annotations-kind-calculation = Calculation
annotations-kind-measurement = Measurement
annotations-kind-issue = Issue
annotations-kind-decision = Decision
annotations-part-any = Part: any
annotations-status-any = Status: any
annotations-date-any = Any date
annotations-date-last-7 = Last 7 days
annotations-date-last-30 = Last 30 days
# Tooltip of the x on a filter field that has a value.
annotations-clear-filter = Clear this filter

## Statuses. Issue: Open / Resolved; Decision: Proposed / Accepted /
## Superseded; Measurement: Planned / Done; Calculation: Up to date / Out of
## date.
annotations-status-open = Open
annotations-status-resolved = Resolved
annotations-status-proposed = Proposed
annotations-status-accepted = Accepted
annotations-status-superseded = Superseded
annotations-status-planned = Planned
annotations-status-done = Done
annotations-status-current = Up to date
annotations-status-stale = Out of date

## Rows
# How many rows a file's header lists. A whole number.
annotations-file-count = { $count }
annotations-jump-to-schematic = Jump to schematic
# The button of an expanded row; $name is a part's designator.
annotations-jump-to = Jump to { $name }
# Tooltip of a reference to a part the design no longer has.
annotations-part-deleted = Part deleted from the design
# Above an expanded row's source: the file's name and the block's line.
annotations-source-at = { $file } · line { $line }

## Footer
annotations-footer-shown = { $shown } of { $total ->
        [one] { $total } annotation
       *[other] { $total } annotations
    }
annotations-footer-files = { $count ->
        [one] 1 file
       *[other] { $count } files
    }

## States
annotations-empty-title = No notebook in this project
annotations-empty-body = Annotations live in Design Notebook files under notebook/. Create one to write notes, calculations and measurements next to the design.
annotations-no-match = No annotations match these filters.
annotations-none-yet = The notebook files hold no annotation blocks yet.
annotations-no-project = Open a project to list the annotations of its Design Notebook.
annotations-reading = Reading the notebook files…
annotations-unavailable = The notebook files could not be read. The Messages panel says why.

## Messages lines. Each leads with the file and the line it is about.
annotations-read-failed = { $file }: not read: { $why }
annotations-problem-unclosed-front-matter = { $file } line { $line }: the +++ front matter is never closed; the file was read as Typst.
annotations-problem-missing-kind = { $file } line { $line }: an annotation without kind: was skipped.
annotations-problem-unknown-kind = { $file } line { $line }: kind "{ $kind }" is not note, calculation, measurement, issue or decision; the block was skipped.
annotations-problem-unknown-status = { $file } line { $line }: a { $kind } has no status "{ $status }"; it is listed without one.
annotations-problem-bad-date = { $file } line { $line }: date "{ $date }" is not YYYY-MM-DD; the block is listed undated.
annotations-problem-unknown-argument = { $file } line { $line }: annotation argument "{ $argument }" is not known and was ignored.
annotations-problem-not-text = { $file } line { $line }: { $argument }: needs a quoted value; it was ignored.
annotations-problem-missing-body = { $file } line { $line }: an annotation without a [ ] body right after it was skipped.
annotations-problem-unbalanced-block = { $file } line { $line }: this annotation is never closed; it and the rest of the file were skipped.
annotations-problem-unclosed = { $file } line { $line }: a { $opener } is never closed; the rest of the file was not read.
annotations-problem-bad-reference = { $file } line { $line }: { $reference } needs quoted names; the reference was left out.

## Jumps from a reference
annotations-jump-gone = { $name } is no longer in the design.
annotations-jump-design-unread = Some sheets of the design could not be read, so { $name } was not found. Open them to see why.
annotations-jump-net-reading = The project's nets are still being read. Click { $name } again in a moment.
annotations-jump-net-unknown = { $name } is not a net of the design.
annotations-jump-net-open-sheet = Open a sheet of this project, then click { $name } again.
