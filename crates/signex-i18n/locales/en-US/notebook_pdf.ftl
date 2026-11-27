## Export Design Notebook PDF (mockup row 202): the dialog, its done card and its notices.

notebook-pdf-title = Export Design Notebook PDF
# The header's second line: the notebook file and its document number.
notebook-pdf-subtitle = { $file } · { $doc }
notebook-pdf-subtitle-rev = { $file } · { $doc } rev { $rev }
notebook-pdf-reading = Reading the notebook…
notebook-pdf-unreadable = The notebook could not be read: { $why }
notebook-pdf-compile-errors = { $count ->
    [one] The notebook has 1 error; fix it to export. { $first }
   *[other] The notebook has { $count } errors; fix them to export. First: { $first }
}
notebook-pdf-stale-banner = { $count ->
    [one] 1 stale reference ({ $names }). It prints as plain text with a warning mark in the margin.
   *[other] { $count } stale references ({ $names }). They print as plain text with a warning mark in the margin.
}
notebook-pdf-review = Review…
notebook-pdf-pages = Pages
notebook-pdf-pages-all = All { $count }
notebook-pdf-pages-current = Current page
notebook-pdf-pages-range = Range
# Placeholder of the Range field.
notebook-pdf-range-placeholder = 3–7
notebook-pdf-paper = Paper
# The first entry of the Paper list; $paper is the document's paper, "A4 portrait".
notebook-pdf-paper-document = From the document ({ $paper })
notebook-pdf-paper-a4 = A4
notebook-pdf-paper-letter = Letter
notebook-pdf-paper-other = Custom
notebook-pdf-portrait = { $paper } portrait
notebook-pdf-landscape = { $paper } landscape
notebook-pdf-paper-caption = Letter re-flows the pages
notebook-pdf-variant = Variant
notebook-pdf-variant-caption = values and designators resolve as in { $variant }
notebook-pdf-include = Include
notebook-pdf-title-block = Title block and revision
notebook-pdf-contents = Contents
notebook-pdf-bookmarks = PDF bookmarks
notebook-pdf-bookmarks-detail = sections, figures and tables
notebook-pdf-python-source = Python cell source
notebook-pdf-python-source-detail = results and plots always print
notebook-pdf-annotations = Annotation blocks
notebook-pdf-annotations-caption = as in Output Jobs
notebook-pdf-stale = Stale references
notebook-pdf-stale-warn = Warn
notebook-pdf-stale-fail = Fail the export
notebook-pdf-stale-caption = stale = renamed or deleted after it was written
notebook-pdf-save-to = Save to
notebook-pdf-browse = Browse…
notebook-pdf-reproducible = Reproducible: the same notebook and project give the same PDF, byte for byte. No creation time and no random ids are written; the revision date comes from the title block.
notebook-pdf-add-output-job = Add as Output Job output…
# Hover note while Export Jobs is not a document tab.
notebook-pdf-add-output-job-later = Export Jobs does not open as a tab yet, so there is no job to add this output to.
notebook-pdf-cancel = Cancel
notebook-pdf-export = Export PDF
notebook-pdf-export-anyway = Export anyway
notebook-pdf-exporting = Exporting…
notebook-pdf-done-title = Notebook PDF exported
notebook-pdf-done-facts = { $pages ->
    [one] 1 page
   *[other] { $pages } pages
} · { $paper } · { $size } · { $references } references, { $stale } stale · variant { $variant }
notebook-pdf-done-bookmarks = Bookmarks written: { $sections } sections, { $subsections } subsections, { $figures } figures, { $tables } tables
notebook-pdf-done-no-bookmarks = No bookmarks written
notebook-pdf-done-checksum = Checksum sha256 { $sum }
notebook-pdf-done-same = Checksum sha256 { $sum } · same as the last export
notebook-pdf-done-changed = Checksum sha256 { $sum } · differs from the last export
notebook-pdf-show-in-folder = Show in folder
notebook-pdf-close = Close
notebook-pdf-open-pdf = Open PDF
notebook-pdf-no-project = Open a project to export its Design Notebook.
notebook-pdf-stale-refused = { $count ->
    [one] 1 stale reference ({ $names }), and the stale rule is Fail the export.
   *[other] { $count } stale references ({ $names }), and the stale rule is Fail the export.
}
notebook-pdf-no-path = Choose where to save the PDF.
notebook-pdf-bad-range = The page range names no page of the notebook.
notebook-pdf-failed = The notebook PDF was not exported: { $why }
notebook-pdf-review-line = Stale reference in the Design Notebook: { $name }
notebook-pdf-open-failed = The notebook PDF did not open: { $why }
