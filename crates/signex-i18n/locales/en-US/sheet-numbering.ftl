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

# Apply was refused; $why says why.
dialog-sheet-numbering-apply-failed = Nothing was changed: { $why }

dialog-sheet-numbering-auto = Auto-number
dialog-sheet-numbering-cancel = Cancel
dialog-sheet-numbering-apply = Apply
