# Text inside the dialogs and modals (#2058, part 2).

## Header subtitles beside a dialog's title.
dialog-subtitle-annotate = Schematic Annotation Configuration
# ERC dialog with no project in view: the rules being edited are this
# machine's.
dialog-subtitle-erc-machine = Machine rules
dialog-subtitle-bom = BOM preview

## Remove from Project

dialog-remove-title = Remove from Project
dialog-remove-unsaved-title = Exclude from Project — Unsaved Edits
# Option cards; the arrow marks a choice.
dialog-remove-exclude = → Exclude from project
dialog-remove-exclude-outside = Library is shared or lives outside this project's folder. It will be excluded from the project and is not deleted from disk.
dialog-remove-exclude-inside = File will be excluded from project but left in local folder. Not recommended for projects stored in version control.
dialog-remove-delete = → Delete file
dialog-remove-delete-package = The library package is deleted whole, with everything inside it, and the library is closed.
dialog-remove-delete-library = The library file is permanently deleted and the library closed. Its symbol, footprint, simulation and datasheet files are deleted too when the folder is the library's own; in a folder that also holds another library or a project or design file, only the library file goes. Other files are left.
dialog-remove-delete-file = File will be removed from project and permanently deleted.
# $name is the document's name in the project tree.
dialog-remove-question = Remove "{ $name }"?
# $name is the document's name in the project tree.
dialog-remove-question-detail = Please choose how to remove file "{ $name }"
# $parents lists the sheets that also place this one.
dialog-remove-placed-by = It is also placed by { $parents }, so it cannot be deleted from here.
# $reason says why the project could not be read, in English.
dialog-remove-placement-unknown = It cannot be deleted from here: whether another sheet places it is not known yet ({ $reason }). You can exclude it from the project.
# $count is how many sheet symbols (a whole number); $symbols names them.
# The English keeps "symbol(s)" for every count.
dialog-remove-dangling = Deleting it leaves { $count } sheet symbol(s) pointing at a missing file: { $symbols }. ERC and netlist export will report them until they are removed.
# $symbols names the sheet symbols that place the file.
dialog-remove-still-placed = Excluding it does not take it out of the design: a sheet symbol still places it ({ $symbols }).
dialog-remove-not-a-page = It is not a page of this project, so there is nothing to exclude.
dialog-exclude-dirty-question = Save the unsaved edits first?

## Close Project and Quit with unsaved edits. In the details, $count is how
## many are unsaved (a whole number) and $kind is "documents" when all are
## files, "items" when library rows are among them; $project is the
## project's name.

dialog-close-project-unsaved-title = Close Project — Unsaved Edits
dialog-close-project-question = Save changes before closing?
dialog-close-project-detail =
    { $kind ->
        [items]
            { $count ->
                [one] '{ $project }' has { $count } unsaved item. What would you like to do?
               *[other] '{ $project }' has { $count } unsaved items. What would you like to do?
            }
       *[documents]
            { $count ->
                [one] '{ $project }' has { $count } unsaved document. What would you like to do?
               *[other] '{ $project }' has { $count } unsaved documents. What would you like to do?
            }
    }
# $names lists the documents kept open.
dialog-close-project-kept = Kept open, not provably this project's alone: { $names }
# macOS says "Quit", other systems "Exit": each wording has its own message.
dialog-quit-unsaved-title = Quit { -brand-name } — Unsaved Edits
dialog-exit-unsaved-title = Exit { -brand-name } — Unsaved Edits
dialog-quit-question = Save changes before quitting?
dialog-exit-question = Save changes before exiting?
dialog-quit-detail =
    { $kind ->
        [items]
            { $count ->
                [one] You have { $count } unsaved item. Save before quitting?
               *[other] You have { $count } unsaved items. Save before quitting?
            }
       *[documents]
            { $count ->
                [one] You have { $count } unsaved document. Save before quitting?
               *[other] You have { $count } unsaved documents. Save before quitting?
            }
    }
dialog-exit-detail =
    { $kind ->
        [items]
            { $count ->
                [one] You have { $count } unsaved item. Save before exiting?
               *[other] You have { $count } unsaved items. Save before exiting?
            }
       *[documents]
            { $count ->
                [one] You have { $count } unsaved document. Save before exiting?
               *[other] You have { $count } unsaved documents. Save before exiting?
            }
    }
dialog-quit-list-changed = The unsaved list changed; review and confirm again.

## Autosave recovery prompt

# $name is the document's file name or part number; $age is an
# already-worded age, such as "5 minutes ago", in English.
dialog-recovery-found = { -brand-name } found a recovery copy of { $name } written { $age } that differs from the saved file.
# $name is the document's file name or part number.
dialog-recovery-older = { $name } was saved after this copy was written. Restore replaces that newer saved work in the editor with the older copy.
# $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-recovery-hint = Restore loads the copy as unsaved edits; nothing is written to the file until you save. Restoring replaces the open document and cannot be undone with { $undo }. Discard deletes the copy. Decide later keeps it for the next time this file is opened, and pauses autosave for this document for the rest of the session.

## Reset All Annotations confirm
dialog-annotate-reset-question = Reset every designator?
# $sheets and $symbols are whole numbers; $renumber is "yes" (Reset & Renumber)
# or "no" (Reset All).
dialog-annotate-reset-detail =
    Every reference designator on { $sheets ->
        [one] the ticked sheet
       *[other] the { $sheets } ticked sheets
    } ({ $symbols ->
        [one] { $symbols } symbol
       *[other] { $symbols } symbols
    }) is reset to '?'{ $renumber ->
        [yes] , then renumbered from 1.
       *[no] .
    }

## Re-prefix confirm
dialog-reprefix-question = Change designator prefixes?

# $count is how many more changes are not listed (a whole number).
dialog-reprefix-more = +{ $count } more

## ERC severity chips: one label under each count.
dialog-erc-chip-errors = Errors
dialog-erc-chip-warnings = Warnings
dialog-erc-chip-info = Info
dialog-erc-chip-off = Off

## Rule editor (ERC dialog and Preferences ▸ Rule Profiles)
dialog-rules-col-rule = Rule
dialog-rules-col-severity = Severity
# A rule that is only switched on or off; $rule is its name, in English.
dialog-rules-on-off = { $rule } (on/off; severity per matrix cell)
dialog-rules-severity-heading = Severity per rule
dialog-rules-severity-hint = Click a cell to change.
dialog-rules-matrix-heading = Pin Connection Matrix
dialog-rules-matrix-hint = Click a cell to cycle severity.
dialog-rules-on = On
dialog-rules-off = Off
# One-letter severities on the rule buttons and the matrix cells.
dialog-rules-error-short = E
dialog-rules-warning-short = W
dialog-rules-info-short = I

## Rename, Enable Version Control, Grid and Selection Filter dialogs
dialog-rename-project-title = Rename Project
dialog-rename-file-title = Rename File
# $name is the project's current name.
dialog-rename-project-prompt = Rename project "{ $name }"
# $name is the file's current name.
dialog-rename-file-prompt = Rename "{ $name }"
# $name is the project's name.
dialog-version-control-project = Project: { $name }
# $name is the library's name.
dialog-version-control-library = Library: { $name }
dialog-selection-filter-title = Selection Filter — Customize
dialog-selection-filter-intro = Selectable kinds

## Move Selection
# $count is how many objects are selected, a whole number. The English keeps
# "item(s)" for every count.
dialog-move-selection-count = { $count } item(s) selected
# The offset along X and Y, in millimetres.
dialog-move-selection-dx = ΔX (mm)
dialog-move-selection-dy = ΔY (mm)

## Net colours
dialog-net-color-pick-title = Pick Net Color
dialog-net-color-no-nets = No net labels on the active sheet.
dialog-net-color-custom-rgb = Custom RGB
# One letter per colour channel: red, green, blue.
dialog-net-color-channel-r = R
dialog-net-color-channel-g = G
dialog-net-color-channel-b = B
dialog-net-color-quick-pick = Quick Pick

## Annotate Schematics (#75)
dialog-annotate-order-up-then-across-help = Columns bottom to top, then the next column to the right.
dialog-annotate-order-across-then-down-help = Rows left to right, then the next row down.
dialog-annotate-order-down-then-across-help = Columns top to bottom, then the next column to the right.
dialog-annotate-order-across-then-up-help = Rows left to right, then the next row up.
# Count beside the sheets heading. Both are whole numbers.
dialog-annotate-sheets-ticked = { $ticked } of { $total }
# Pill on the sheet the active tab shows.
dialog-annotate-sheet-current = current
# A sheet placed more than once: $n is the placement's ordinal (a whole number).
dialog-annotate-sheet-placement = placement { $n }
dialog-annotate-fixed-options = Fixed options
dialog-annotate-fixed-read-only = read-only
# The folded Fixed options group in one line.
dialog-annotate-fixed-digest = Designator · Sub-parts Off · Packages None · Matching not available yet
# Banner text after the bold count. $sheets is how many sheets are ticked (a
# whole number) and $sheet the name of the one; $undo is "project" or "sheet".
dialog-annotate-banner-scope =
    { $sheets ->
        [one] on { $sheet }.
       *[other] on { $sheets } sheets.
    } The list updates live; Accept Changes applies it as { $undo ->
        [sheet] one undo step per sheet.
       *[project] one undo step.
    }
# Change-list switch. $count is a whole number.
dialog-annotate-view-changes = Changes { $count }
dialog-annotate-view-all = All { $count }
dialog-annotate-filter-placeholder = Filter designators
dialog-annotate-col-sheet = Sheet
dialog-annotate-kept-locked = Kept (locked)
dialog-annotate-no-match = No row matches the filter.
dialog-annotate-no-changes = No designator changes. Pick All to see every row.
dialog-annotate-reset-all-ellipsis = Reset All…
dialog-annotate-reset-renumber-ellipsis = Reset & Renumber…
# Footer caption. $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-annotate-undo-caption = { $undo } undoes it on every sheet
dialog-annotate-undo-caption-per-sheet = { $undo } undoes it one sheet at a time

## Reset confirm (#76)
# The scope before the preview has counted it. $renumber is "yes" or "no".
dialog-annotate-reset-detail-loading =
    Every reference designator on the ticked sheets is reset to '?'{ $renumber ->
        [yes] , then renumbered from 1.
       *[no] .
    }
# $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-annotate-reset-one-step = One undo step: { $undo } restores every sheet.
dialog-annotate-reset-per-sheet = { $undo } undoes it one sheet at a time, as the Annotate undo setting asks.

## Re-prefix confirm (#77)
# Stat tile captions under a count (a whole number).
dialog-reprefix-tile-parts =
    { $count ->
        [one] part changes
       *[other] parts change
    }
dialog-reprefix-tile-sheets =
    { $count ->
        [one] sheet
       *[other] sheets
    }
dialog-reprefix-tile-standard = standard
# $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-reprefix-bullet-undo = One undo step: { $undo } on any changed sheet undoes it on all of them.
dialog-reprefix-bullet-unsaved = Sheets stay unsaved until you save. PCB references are not updated.
# $count is a whole number.
dialog-reprefix-bullet-by-symbol =
    { $count ->
        [one] { $count } part has no stored library row and was matched through its symbol.
       *[other] { $count } parts have no stored library row and were matched through their symbol.
    }
# Caption over a list the last row of which counts the rest. Both are whole numbers.
dialog-reprefix-caption-named = { $named } named changes of { $total }; the last row counts the rest.
# Caption over a list that names every change. $count is a whole number.
dialog-reprefix-caption-all =
    { $count ->
        [one] { $count } change.
       *[other] { $count } changes.
    }
dialog-reprefix-scroll = Scroll ↓
dialog-reprefix-col-part = Part
dialog-reprefix-col-prefix = Prefix
dialog-reprefix-col-new = New
dialog-reprefix-col-note = Note
dialog-reprefix-note-by-symbol = matched by symbol
dialog-reprefix-left-unchanged = Left unchanged
dialog-reprefix-listed-in-messages = listed in Messages
# Skip pills: $count is how many parts the reason leaves unchanged.
dialog-reprefix-skip-read-only = { $count } on a read-only sheet
dialog-reprefix-skip-locked = { $count } locked
dialog-reprefix-skip-no-link = { $count } without a library link
dialog-reprefix-skip-not-mounted = { $count } from a library not mounted
dialog-reprefix-skip-row-missing = { $count } with a deleted library row
dialog-reprefix-skip-ambiguous = { $count } whose library rows disagree
dialog-reprefix-skip-per-placement = { $count } with a per-placement designator
dialog-reprefix-skip-mixed-versions = { $count } with mixed symbol versions
dialog-reprefix-skip-hand-edited =
    { $count ->
        [one] { $count } hand-edited prefix
       *[other] { $count } hand-edited prefixes
    }
