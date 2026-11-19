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
# $undo is the Undo shortcut, such as "Ctrl+Z".
dialog-annotate-reset-detail =
    Every reference designator will be reset to '?', then the sheet will be renumbered from 1.
    This cannot be undone through { $undo } alone — consider saving first.

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
dialog-grid-title = Cartesian Grid Editor [mm]
dialog-grid-intro = Grid step and display
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
