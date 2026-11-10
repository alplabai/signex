# Number Schematic Sheets dialog (#2247): Tools > Annotation > Number
# Schematic Sheets. Sheet numbers are not localised.

# Dialog title.
dialog-sheet-numbering-title = Number Schematic Sheets

# Under the title. $count is the number of pages, $project the project name.
dialog-sheet-numbering-subtitle =
    { $count ->
        [one] 1 page in { $project }
       *[other] { $count } pages in { $project }
    }

dialog-sheet-numbering-intro = Pages are numbered in hierarchy order. Type a number to give a page its own, or Auto-number to put every page back in order. Apply changes the open sheets; save them to keep the numbers.

# Column headings of the page table.
dialog-sheet-numbering-col-sheet = Sheet
dialog-sheet-numbering-col-file = File
dialog-sheet-numbering-col-number = Number

# Shown for a top-level page, which has no sheet symbol to hold a number.
dialog-sheet-numbering-top-level = top level

# While the project's sheets are read.
dialog-sheet-numbering-reading = Reading the project's sheets…

# The read failed; $why is the reason.
dialog-sheet-numbering-read-failed = The sheets could not be read: { $why }

dialog-sheet-numbering-no-pages = This project has no sheets to number.

# A typed number that is not a whole number from 1.
dialog-sheet-numbering-invalid = A sheet number is a whole number from 1.

# Two or more pages share $number. Warning only; Apply still works.
dialog-sheet-numbering-clash = More than one page is numbered { $number }.

# Numbers from 1 to the page count that no page has; $numbers is a list.
dialog-sheet-numbering-gap = No page is numbered { $numbers }.

# A page numbered above the page count $count.
dialog-sheet-numbering-out-of-range = Page { $page } is numbered { $number }, above the page count { $count }.

# The project's root sheet is absent or does not parse; $path is its file.
dialog-sheet-numbering-err-root-unreadable = The root sheet { $path } could not be read.

# Apply was refused: a sheet was opened, closed or had a sheet symbol changed
# while the dialog was open.
dialog-sheet-numbering-err-stale = Nothing was changed: the sheets changed while this dialog was open. Close it and open it again.

# Apply was refused: the closed sheet $path is being saved, renamed or moved.
dialog-sheet-numbering-err-busy = Nothing was changed: { $path } is being saved or moved. Try again once it finishes.

# Apply was refused: $path was written by a newer Signex and opens read-only.
dialog-sheet-numbering-err-read-only = Nothing was changed: { $path } is read-only (written by a newer Signex).

# Apply stopped: the closed sheet $path could not be loaded; every sheet was put back.
dialog-sheet-numbering-err-not-loaded = Nothing was changed: { $path } could not be loaded.

# Apply stopped: $path refused the change; every sheet was put back.
dialog-sheet-numbering-err-not-changed = Nothing was changed: { $path } could not be changed.

dialog-sheet-numbering-auto = Auto-number
dialog-sheet-numbering-cancel = Cancel
dialog-sheet-numbering-apply = Apply
