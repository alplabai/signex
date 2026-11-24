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
# Radio cards: a title, then what the choice does.
dialog-remove-exclude = Exclude from project
dialog-remove-exclude-outside = Library is shared or lives outside this project's folder. It will be excluded from the project and is not deleted from disk.
dialog-remove-exclude-inside = The file stays on disk, but the project no longer lists it.
# The project folder is a Git repository.
dialog-remove-exclude-inside-git = The file stays on disk and in Git, but the project no longer lists it.
dialog-remove-delete = Delete file
dialog-remove-delete-package = The library package is deleted whole, with everything inside it, and the library is closed.
dialog-remove-delete-library = The library file is permanently deleted and the library closed. Its symbol, footprint, simulation and datasheet files are deleted too when the folder is the library's own; in a folder that also holds another library or a project or design file, only the library file goes. Other files are left.
dialog-remove-delete-file = The file is removed from the project and permanently deleted.
# Footer buttons that carry out the selected card.
dialog-remove-action-exclude = Exclude
dialog-remove-action-delete = Delete file
# $name is the document's name in the project tree.
dialog-remove-question = Remove "{ $name }"?
dialog-remove-question-detail = Choose how to remove it.
# $parents lists the sheets that also place this one.
dialog-remove-placed-by = It is also placed by { $parents }, so it cannot be deleted from here.
# $reason says why the project could not be read, in English.
dialog-remove-placement-unknown = It cannot be deleted from here: whether another sheet places it is not known yet ({ $reason }). You can exclude it from the project.
# $count is how many sheet symbols (a whole number); $symbols names them.
dialog-remove-dangling =
    { $count ->
        [one] Deleting it leaves { $count } sheet symbol pointing at a missing file: { $symbols }. ERC and netlist export report it until it is removed.
       *[other] Deleting it leaves { $count } sheet symbols pointing at a missing file: { $symbols }. ERC and netlist export report them until they are removed.
    }
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
# The project settings (its .snxprj) are counted apart, by the -settings
# messages below.
dialog-close-project-detail =
    { $kind ->
        [items]
            { $count ->
                [one] '{ $project }' has { $count } unsaved item.
               *[other] '{ $project }' has { $count } unsaved items.
            }
       *[documents]
            { $count ->
                [one] '{ $project }' has { $count } unsaved document.
               *[other] '{ $project }' has { $count } unsaved documents.
            }
    }
dialog-close-project-detail-settings =
    { $kind ->
        [items]
            { $count ->
                [one] '{ $project }' has { $count } unsaved item and unsaved project settings.
               *[other] '{ $project }' has { $count } unsaved items and unsaved project settings.
            }
       *[documents]
            { $count ->
                [one] '{ $project }' has { $count } unsaved document and unsaved project settings.
               *[other] '{ $project }' has { $count } unsaved documents and unsaved project settings.
            }
    }
# $project is the project's name.
dialog-close-project-detail-settings-only = '{ $project }' has unsaved project settings.
# $count is how many documents stay open (a whole number); $names lists
# them. Another open project uses them, or that cannot be ruled out.
dialog-close-project-kept =
    { $count ->
        [one] { $names } stays open because another open project may use it.
       *[other] { $names } stay open because other open projects may use them.
    }
# What each unsaved row is, beside its name.
dialog-close-project-kind-schematic = Schematic
dialog-close-project-kind-pcb = PCB
dialog-close-project-kind-settings = Project settings
dialog-close-project-kind-library = Library
dialog-close-project-kind-symbol = Symbol
dialog-close-project-kind-footprint = Footprint
dialog-close-project-kind-component = Component
dialog-close-project-kind-file = File
# macOS says "Quit", other systems "Exit": each wording has its own message.
dialog-quit-unsaved-title = Quit { -brand-name } — Unsaved Edits
dialog-exit-unsaved-title = Exit { -brand-name } — Unsaved Edits
dialog-quit-question = Save changes before quitting?
dialog-exit-question = Save changes before exiting?
# The fact line under the question. $unsaved, $projects and $libraries are
# the three messages below, already worded; $scope says which of them the
# list holds: "both", "projects", "libraries" or "none" (files outside any
# project only).
dialog-quit-summary =
    { $scope ->
        [both] { $unsaved } in { $projects } and { $libraries }.
        [projects] { $unsaved } in { $projects }.
        [libraries] { $unsaved } in { $libraries }.
       *[none] { $unsaved }.
    }
dialog-quit-unsaved-count =
    { $kind ->
        [items]
            { $count ->
                [one] { $count } unsaved item
               *[other] { $count } unsaved items
            }
       *[documents]
            { $count ->
                [one] { $count } unsaved document
               *[other] { $count } unsaved documents
            }
    }
# $count is how many projects hold unsaved documents (a whole number).
dialog-quit-project-count =
    { $count ->
        [one] { $count } project
       *[other] { $count } projects
    }
# $count is how many libraries hold unsaved drafts or rows (a whole number).
dialog-quit-library-count =
    { $count ->
        [one] { $count } library
       *[other] { $count } libraries
    }
# Group headings of the unsaved list: files outside every open project, and
# the library drafts and Component Preview rows.
dialog-quit-group-other = Other files
dialog-quit-group-libraries = Libraries
dialog-quit-list-changed = The unsaved list changed; review and confirm again.

## Autosave recovery prompt

# $name is the document's file name or part number.
dialog-recovery-found = { -brand-name } found a recovery copy of { $name } that differs from the saved file.
# $name is the document's file name or part number.
dialog-recovery-older = { $name } was saved after this copy was written. Restore replaces that newer saved work in the editor with the older copy.
# $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-recovery-restore-hint = Restore loads the copy as unsaved edits; nothing is written to the file until you save. Restoring replaces the open document and cannot be undone with { $undo }.
dialog-recovery-later-hint = Discard deletes the copy. Decide later keeps it for the next time this file is opened, and pauses autosave for this document for the rest of the session.
# The two versions the prompt lists, and the mark on the newer one.
dialog-recovery-copy = Recovery copy
dialog-recovery-saved = Saved file
dialog-recovery-newer = newer
# $path is the saved file's path; $time is when it was last written, such
# as "13:58" or "Nov 16 17:40".
dialog-recovery-saved-time = { $path } · { $time }
# Tooltip of the header's close button, which keeps the copy.
dialog-recovery-close-tooltip = Decide later (Esc)

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
dialog-rules-severity-heading = Severity per rule
dialog-rules-matrix-heading = Pin Connection Matrix
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
dialog-rename-new-name = New name
# $name is the new file name another file in the folder already has.
dialog-rename-name-taken = A file named { $name } already exists.
# The sheet symbols that place the file being renamed; the rename re-points
# them. $count is how many there are; $symbol and $sheet name the first symbol
# and the sheet it sits on; $symbols lists every symbol as "name (sheet)".
dialog-rename-placed-by =
    { $count ->
        [one] Sheet symbol { $symbol } on { $sheet } points at this file and is updated too.
       *[other] Sheet symbols { $symbols } point at this file and are updated too.
    }
# $name is the project's name.
dialog-version-control-project = Project: { $name }
# $name is the library's name.
dialog-version-control-library = Library: { $name }
# Shown under the folder's path, which is drawn on its own line above. $every
# is "on" while Commit every save is on.
dialog-version-control-intro-project =
    { $every ->
        [on] Creates a Git repository in this folder and commits the ticked files as the first version. Every save after that is committed, including library edits inside the project's .snxlib folders.
       *[off] Creates a Git repository in this folder and commits the ticked files as the first version. Saves are not committed while Commit every save is off.
    }
# Shown under the library folder's path, which is drawn on its own line above.
dialog-version-control-intro-library = Creates a Git repository in this folder and commits the ticked files as the first version. Later edits to the library are committed too.
# $ticked rows of $total are ticked; the second form leads into "Select all".
dialog-version-control-ticked = { $ticked } of { $total } ticked
dialog-version-control-ticked-more = { $ticked } of { $total } ticked ·
dialog-version-control-select-all = Select all
dialog-version-control-commit-message = First commit message
# $name is the project's or library's name.
dialog-version-control-commit-message-default = Initial version of { $name }
dialog-version-control-commit-every-save = Commit every save
dialog-version-control-commit-every-save-help = Each save of a project file is committed. Turned off, saves are not committed; the repository and its history stay as they are.
# What a row of the file list is.
dialog-version-control-kind-project = Project
dialog-version-control-kind-schematic = Schematic
dialog-version-control-kind-pcb = PCB
dialog-version-control-kind-library = Library
dialog-version-control-kind-config = Config
dialog-version-control-kind-components = Components
dialog-version-control-kind-folder = Folder
dialog-selection-filter-title = Selection Filter
dialog-selection-filter-intro = Selectable kinds
# Subtitle beside the title of the Custom Selection Filter dialog.
dialog-selection-filter-subtitle = Customize
# Buttons that turn every kind on or off.
dialog-selection-filter-all = All
dialog-selection-filter-none = None
# Pill on a kind the canvas does not filter by yet.
dialog-selection-filter-not-wired = not wired
# Name the filter is saved under as a preset.
dialog-selection-filter-preset-name = Preset name
# Footer count. $count and $total are whole numbers.
dialog-selection-filter-selectable = { $count } of { $total } selectable
# Messages: Apply at the preset limit. $name is the typed preset name, $limit a whole number.
dialog-selection-filter-preset-full = Filter applied; preset { $name } was not saved: the { $limit } preset slots are full.
# Messages: Apply with the kinds of a preset that already exists. $name is that preset.
dialog-selection-filter-preset-exists = Filter applied; no preset added: { $name } already holds these kinds.

## Move Selection
# Header subtitle. $count is how many objects are selected, a whole number.
dialog-move-selection-count = { $count ->
    [one] { $count } item selected
   *[other] { $count } items selected
}
# The offset along X and Y; the field shows the unit as a suffix.
dialog-move-selection-dx = ΔX
dialog-move-selection-dy = ΔY

## Net colours
dialog-net-color-no-nets = No net labels on the active sheet.
dialog-net-color-custom-rgb = Custom RGB
# One letter per colour channel: red, green, blue.
dialog-net-color-channel-r = R
dialog-net-color-channel-g = G
dialog-net-color-channel-b = B
dialog-net-color-quick-pick = Quick Pick

## Rule editor, grouped list and matrix legend (#1289)
# Filter chip: show only the rules that differ from the shipped profile.
dialog-rules-changed-only = Changed only
dialog-rules-filter-placeholder = Filter rules
dialog-rules-no-match = No rule matches the filter.
# Column heads of the rule table on the Rule Profiles page.
dialog-rules-column-rule = Rule
dialog-rules-column-severity = Severity
# Subheads of the rule list.
dialog-rules-group-pins = Pins
dialog-rules-group-nets = Nets
dialog-rules-group-labels = Labels
dialog-rules-group-buses = Buses
dialog-rules-group-hierarchy = Hierarchy
dialog-rules-group-parts = Parts
# Pin connection matrix legend.
dialog-rules-legend-error = Error
dialog-rules-legend-warning = Warning
dialog-rules-legend-info = Info
dialog-rules-legend-off = No report
dialog-rules-legend-changed = Changed
dialog-rules-legend-changed-rule = Rule differs
dialog-rules-matrix-caption = Pin type conflict is on/off only: its findings take their severity from this matrix.

## Help ▸ About Signex (#2262)
# The product name beside the mark.
dialog-about-name = { -brand-name }
dialog-about-tagline = AI-first electronics design automation
# Keys of the version block; the values are the version, the licence name
# and the operating system, shown as they are.
dialog-about-version = Version
dialog-about-license = License
dialog-about-os = OS
# Links to the user guide and the issue tracker, opened in the browser.
dialog-about-documentation = Documentation
dialog-about-report-issue = Report an Issue
# Footer button: copies the version block for a bug report.
dialog-about-copy-version = Copy version info

## Help ▸ Keyboard Shortcuts (#2262)
# Header subtitle. $profile is the keyboard-shortcut profile's name; $scope
# is the editor the sheet lists (Schematic, PCB or All).
dialog-shortcuts-subtitle = { $profile } profile · { $scope }
dialog-shortcuts-filter-placeholder = Filter by action or key
# The editor switch: which editor's shortcuts are listed.
dialog-shortcuts-scope-schematic = Schematic
dialog-shortcuts-scope-pcb = PCB
dialog-shortcuts-scope-all = All
# Under the list while a filter is typed. $shown and $total are whole numbers.
dialog-shortcuts-count-filtered =
    { $shown ->
        [one] { $shown } of { $total } shortcuts matches. Clear the filter to see all categories.
       *[other] { $shown } of { $total } shortcuts match. Clear the filter to see all categories.
    }
# Under the list with no filter. $total is a whole number; $scope as above.
dialog-shortcuts-count-all =
    { $total ->
        [one] { $total } shortcut in { $scope }.
       *[other] { $total } shortcuts in { $scope }.
    }
# Nothing matches. $query is the typed filter; $scope as above.
dialog-shortcuts-no-match = No shortcut matches “{ $query }” in { $scope }.
# Before a button that switches the list to All.
dialog-shortcuts-try = Try
# Nothing matches while All is already shown.
dialog-shortcuts-no-match-all = Check the spelling, or clear the filter.
# Footer note. $key is the key that opens and closes the sheet (F1).
dialog-shortcuts-key-hint = { $key } opens and closes this sheet
# Footer button: closes the sheet and opens the keyboard-shortcut page of
# Preferences.
dialog-shortcuts-edit-in-preferences = Edit in Preferences…

## First-run card (#2262)
dialog-tour-pan = Pan
# The pan gesture: drag with the right mouse button.
dialog-tour-pan-gesture = Right-drag
dialog-tour-zoom = Zoom
# The zoom gesture when the mouse wheel zooms.
dialog-tour-zoom-wheel = Wheel
# The zoom gesture when the mouse wheel scrolls. $modifier is the key held
# with the wheel (Ctrl, or Cmd on macOS).
dialog-tour-zoom-modified-wheel = { $modifier }+Wheel
dialog-tour-find-command = Find any command
dialog-tour-all-shortcuts = All keyboard shortcuts
# Footer note before the "change" link. $mode is the Mouse wheel setting.
dialog-tour-wheel-set =
    Wheel set to { $mode ->
        [scroll] Scroll
       *[zoom] Zoom
    } ·
# Link that opens the Mouse wheel setting in Preferences.
dialog-tour-wheel-change = change
# Footer button: closes the card for good, like its ✕.
dialog-tour-got-it = Got it

## Parameter Manager
# Window subtitle: the active sheet's file, its parts and parameter columns.
dialog-parameter-manager-subtitle = { $sheet } · { $parts ->
    [one] 1 part
   *[other] { $parts } parts
} · { $parameters ->
    [one] 1 parameter
   *[other] { $parameters } parameters
}
# Beside the filter box: the fields the filter searches.
dialog-parameter-manager-filter-hint = Reference, value or footprint
# An empty parameter cell.
dialog-parameter-manager-not-set = Not set
dialog-parameter-manager-remove-column = Remove column
dialog-parameter-manager-no-match-title = No part matches the filter
# $sheet is the sheet's file name, $query the typed filter.
dialog-parameter-manager-no-match-body = No reference, value or footprint on { $sheet } contains { $query }.
dialog-parameter-manager-no-schematic-title = No active schematic
dialog-parameter-manager-no-schematic-body = The Parameter Manager edits the parts of the active sheet. Open a schematic sheet, then open it again from Tools ▸ Parameter Manager.
dialog-parameter-manager-footer-hint = Edits apply as you type · each cell is one undo step (Ctrl+Z)
# Cells edited since the dialog opened whose value differs from before.
dialog-parameter-manager-changed = { $count ->
    [one] 1 cell changed
   *[other] { $count } cells changed
}
# Tooltip of Revert changes.
dialog-parameter-manager-revert-tip = { $count ->
    [one] Undo the cell edit as one step
   *[other] Undo all { $count } cell edits as one step
}

## Move Selection (#66)
# The primary button: moves the selection by the offsets.
dialog-move-selection-move = Move
# Under the preview. $grid is the grid step and $unit its unit; $x and $y
# are how many whole grid steps the offsets span.
dialog-move-selection-help = Positive X moves right, positive Y moves up. Grid { $grid } { $unit }: { $x } × { $y } steps.
# The same line when an offset is not a whole number of grid steps.
dialog-move-selection-help-off-grid = Positive X moves right, positive Y moves up. Grid { $grid } { $unit }: off grid.
# Caption of the framed preview.
dialog-move-selection-preview = Preview
# After the names drawn on the preview; $count is how many items are not named.
dialog-move-selection-preview-more = +{ $count }

## Net colour palette (#2528)
# Header subtitle. $count is how many nets the active sheet labels.
dialog-net-color-count = { $count ->
    [one] { $count } net on this sheet
   *[other] { $count } nets on this sheet
}
dialog-net-color-filter = Filter nets
# Footer link: removes every net colour of the active sheet in one undo step.
dialog-net-color-clear-all = Clear all colours
dialog-net-color-row-hint = Click a row to choose
# Row chooser: opens the custom colour picker for the row's net.
dialog-net-color-custom = Custom…
# Row chooser: removes the row's net colour.
dialog-net-color-clear = Clear
# Custom picker: the hex code field.
dialog-net-color-hex = Hex
# Custom picker preview: the net's colour now, and the pick.
dialog-net-color-old = Old
dialog-net-color-new = New
# Messages: Use Color was pressed after the active sheet changed. $net is the net name.
dialog-net-color-sheet-changed = Net colour not changed: the picker was opened for { $net } on another sheet.

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
       *[project] one undo step until you save.
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
dialog-annotate-undo-caption = { $undo } undoes it on every sheet until you save
dialog-annotate-undo-caption-per-sheet = { $undo } undoes it one sheet at a time

## Reset confirm (#76)
# The scope before the preview has counted it. $renumber is "yes" or "no".
dialog-annotate-reset-detail-loading =
    Every reference designator on the ticked sheets is reset to '?'{ $renumber ->
        [yes] , then renumbered from 1.
       *[no] .
    }
# $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-annotate-reset-one-step = One undo step: { $undo } restores every sheet until you save.
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
dialog-reprefix-bullet-undo = One undo step: { $undo } on any changed sheet undoes it on all of them until you save.
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

# net-color-clear-toast
# Net colour palette toast after Clear all colours. $count is how many
# labelled nets lost their colour (a whole number), as the palette counts
# its rows; colours on nets with no label are cleared too but not counted.
dialog-net-color-toast-cleared =
    { $count ->
        [one] Cleared { $count } net colour
       *[other] Cleared { $count } net colours
    }
# The same toast when the colours cleared were on nets with no label.
dialog-net-color-toast-cleared-unlabelled = Cleared the net colours of this sheet
# Toast after Clear in a row's chooser. $net is the net name.
dialog-net-color-toast-cleared-net = Cleared the colour of { $net }
# The toast's button. $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-net-color-toast-undo = Undo { $undo }
