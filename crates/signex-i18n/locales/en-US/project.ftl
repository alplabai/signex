# The Project tab (Project setup section): settings of the project itself.

# Agent journals group (#2253).
project-agent-journals-title = Agent journals
project-agent-journals-keep-in-git = Keep agent journals in git
project-agent-journals-keep-in-git-help = Keeps .signex/agent-*.jsonl in git. Turned off, Signex lists them in its managed .gitignore block when the project is saved; a journal that is already committed stays tracked.
project-agent-journals-summarise = Summarise agent changes in the commit message
project-agent-journals-summarise-help = Saved with the project; it takes effect once commit messages list agent changes, which a later version adds.
project-agent-journals-lines = Journal size (lines)
project-agent-journals-lines-help = Lines kept per journal. Older lines are dropped from the file; they stay in git history only while the journals are kept in git, otherwise they are discarded.
project-agent-journals-lines-invalid = Enter a whole number from { $min } to { $max }; the stored value is unchanged.

# The Project tab's section list (#1202, #1689-#1693).
project-section-general = General
project-section-parameters = Parameters
project-section-sheets = Sheets
project-section-variants = Variants
project-section-libraries = Libraries
project-section-erc-annotation = ERC and annotation
project-section-version-control = Version control
project-section-schematic-defaults = Schematic defaults
project-section-setup = Project setup
# The section list's group headings (mockup row 29).
project-section-group-project = PROJECT
project-section-group-rules = RULES & DEFAULTS
project-section-group-tracking = TRACKING

# General section (mockup row 29).
project-general-help = The project's name, description and the designator standard new parts follow.
project-general-name = Name
project-general-rename = Rename…
project-general-description = Description
project-general-designator-standard = Designator standard
project-general-library-designator = Library designator
# $standards is the picker's choices, joined with a middle dot.
project-general-designator-hint = { $standards }
project-general-designator-newer = Set by a newer Signex version — kept unchanged
project-general-files = Files
project-general-directory = Directory
project-general-root-schematic = Root schematic
project-general-pcb-file = PCB file
project-general-libraries = Libraries
project-general-show-in-libraries = Show in Libraries

# Project setup section (mockup row 37).
project-setup-help = The board rules and sheet defaults the project was created with. New PCBs and schematics start from these.
project-setup-pcb-rules = PCB rules
project-setup-schematic = Schematic
project-setup-your-profiles = YOUR PROFILES

# Row actions shared by the sections' tables.
project-move-up = Move up
project-move-down = Move down

# Parameters section (mockup row 30): the filter, the Title block and Variables groups,
# and renaming a variable in place.
project-parameters-hint = Every sheet inherits these; a sheet's own Title Block value or sheet variable overrides one. A value starting with = is an expression, such as = VCC / 2. Press Enter in a value to check it.
project-parameters-filter = Filter parameters
project-parameters-totals = { $title_block } title block · { $variables ->
    [one] 1 variable
   *[other] { $variables } variables
}
project-parameters-group-title-block = TITLE BLOCK
project-parameters-group-variables = VARIABLES
project-parameters-count = { $count }
project-parameters-sheets-override = sheets override
project-parameters-not-set = Not set
project-parameters-remove = Remove
project-parameters-add = Add
project-parameters-field-title = Title
project-parameters-field-document-number = Document No.
project-parameters-field-rev = Revision
project-parameters-field-date = Date
project-parameters-field-company = Company
project-parameters-field-drawn-by = Drawn By
project-parameters-field-checked-by = Checked By
project-parameters-field-approved-by = Approved By
project-parameters-field-author = Author
project-parameters-field-engineer = Engineer
project-parameters-field-organization = Organization
project-parameters-field-address-1 = Address 1
project-parameters-field-address-2 = Address 2
project-parameters-field-address-3 = Address 3
project-parameters-field-address-4 = Address 4
project-parameter-name-empty = Enter a parameter name.
project-parameter-name-invalid = A parameter name is letters, digits and _, and does not start with a digit.
project-parameter-name-title-block = { $name } is a title block field; choose another name.
project-parameter-name-taken = The project already has a parameter named { $name }.
project-parameter-rename-references = Sheet text that uses { $name } as a special string, and sheet values named { $name }, keep the old name: they stop showing this value. Press Enter to rename anyway.
# What a project variable's `=` expression evaluates to. $value is the
# result, such as 1.65V.
project-parameter-shows = Shows { $value }
# Why a project variable's `=` expression does not evaluate. $error is the
# reason.
project-parameter-error = Error: { $error }

# Sheets section (#1689).
project-sheets-help = The project lists its sheets in this order. The Sheet N of M in a title block and the order of the project tree follow it. PDF export numbers pages by the sheet hierarchy instead; number child sheets with Tools ▸ Annotation ▸ Number Schematic Sheets.
project-sheets-col-number = Page
project-sheets-page = { $page }
project-sheets-col-name = Name
project-sheets-col-file = File
project-sheets-empty = This project lists no sheets.
project-sheets-defaults-title = New sheets
project-sheets-defaults-help = A sheet added to this project starts with this size, orientation and title block.

# Variants section (#1690).
project-variants-help = A variant decides which parts are fitted. The BOM and the PDF export use the active variant; Base fits every part.
project-variants-col-active = Active
project-variants-col-name = Name
project-variants-base = Base
project-variants-remove = Remove
project-variant-empty = Enter a variant name.
project-variant-too-long = A variant name is at most { $max } characters.
project-variant-invalid = A variant name cannot contain line breaks or control characters.
project-variant-reserved = Base is the variant every project has; choose another name.
project-variant-taken = The project already has a variant named { $name }.
project-variant-unknown = The project has no variant named { $name }.

# Variants section in the Variant Manager layout (#1293).
# The name `+` gives a new variant. $n counts from 1.
project-variants-new-name = Variant { $n }
project-variants-active-caption = Active variant
project-variants-filter = Filter parts (R5, U*, 10k)
project-variants-only-differ = Only rows that differ
project-variants-list-title = Variants
# How many variants the list holds, Base included.
project-variants-count = { $count }
# Pill on the active variant's row, card and column.
project-variants-active = active
project-variants-row-base = { $fitted } / { $total } fitted · reference
project-variants-row =
    { $changes ->
        [one] { $fitted } / { $total } fitted · 1 change
       *[other] { $fitted } / { $total } fitted · { $changes } changes
    }
project-variants-selected = Selected: { $name }
project-variants-rename = Rename
project-variants-delete = Delete
project-variants-base-note = Base is the reference: it cannot be renamed or deleted. Double-click a name to rename it in place.
project-variants-card-parts = Parts
project-variants-card-cost = Cost
project-variants-card-vs-base = vs Base
project-variants-card-fitted = { $fitted } / { $total }
# A card value with no source yet (the cost: no prices).
project-variants-no-value = —
project-variants-reference = reference
project-variants-changes =
    { $changes ->
        [one] 1 change
       *[other] { $changes } changes
    }
project-variants-col-part = Part
project-variants-col-value = Value · Footprint
# After a part's value in the matrix.
project-variants-footprint = · { $footprint }
project-variants-fitted = Fitted
project-variants-not-fitted = Not fitted
project-variants-legend = Cells that match Base stay quiet; only differences are coloured.
project-variants-hidden-rows =
    { $hidden ->
        [one] { $differ } of { $total } parts differ in at least one variant. 1 row is hidden by Only rows that differ.
       *[other] { $differ } of { $total } parts differ in at least one variant. { $hidden } rows are hidden by Only rows that differ.
    }
project-variants-no-match = No parts match the filter.
project-variants-no-parts = The project's sheets place no parts.
project-variants-status =
    { $parts ->
        [one] 1 part
       *[other] { $parts } parts
    } · { $variants } variants · { $cells ->
        [one] 1 cell differs
       *[other] { $cells } cells differ
    } from Base
project-variants-status-base =
    { $parts ->
        [one] 1 part · 1 variant
       *[other] { $parts } parts · 1 variant
    }
project-variants-reading = Reading the project's parts…
# $reason is the system's own text for why the read stopped.
project-variants-read-failed = The parts could not be read: { $reason }
project-variants-read-no-root = The parts could not be read: the project has no root sheet.
# $path is the root sheet's file path.
project-variants-read-root-unreadable = The parts could not be read: the root sheet { $path } could not be read.
project-variants-stale = The sheets changed since the parts were read.
project-variants-read-again = Read again
project-variants-empty-title = This project has one variant, Base
project-variants-empty-body = Base fits every part with the values on the schematic. A variant records only what differs from Base: parts not fitted, alternate parts and per-variant values.
project-variants-new-from-base = New variant from Base

# Libraries section (#1691).
project-libraries-help = The libraries this project mounts, in the order the project tree lists them. A read-only library cannot be edited from this project.
project-libraries-col-name = Name
project-libraries-col-location = Location
project-libraries-col-kind = Kind
project-libraries-col-state = State
project-libraries-kind-project = In the project
project-libraries-kind-shared = Shared
project-libraries-kind-global = Global
project-libraries-state-mounted = Mounted
project-libraries-state-read-only = Mounted, read-only
project-libraries-state-missing = Missing
project-libraries-state-not-mounted = Not mounted
# Button on a Missing library row: pick the file where it moved to.
project-libraries-locate = Locate…
project-libraries-empty = This project mounts no libraries. Right-click the project in the project tree, then Add New to Project ▸ Component Library.

# ERC and annotation section (#1692).
project-erc-annotation-help = Rules and annotation settings stored in this project travel with the project file. A project that stores none uses this machine's Preferences.
project-erc-title = ERC rules
project-erc-stored = This project stores its own ERC rules; they travel with the project file.
project-erc-machine = This project uses this machine's ERC rules (User Default). Store them in the project so they travel with it.
project-erc-unreadable = This project holds ERC rules from a newer version of Signex; they are kept and not applied.
project-erc-store = Store ERC rules in this project
project-erc-edit-hint = Change a rule's severity in Project ▸ Electrical Rules Check.
project-erc-rule-count = { $count }
project-erc-severity-error = Error
project-erc-severity-warning = Warning
project-erc-severity-info = Info
project-annotate-title = Annotation
project-annotate-undo = Annotate undo
project-annotate-undo-help = How many undo steps one annotate run of this project is. The Preferences default follows this machine's Preferences.
project-annotate-undo-default = Preferences default ({ $value })
project-annotate-undo-project = One step for the whole project
project-annotate-undo-per-sheet = Per sheet

# Version control section (#1693).
project-vcs-help = Version control keeps the project's design files in a Git repository in the project folder. Each commit is a point to compare against or restore from in the History panel.
project-vcs-repository = Repository
project-vcs-repository-present = A Git repository is in the project folder.
project-vcs-repository-absent = The project folder has no Git repository.
project-vcs-repository-checking = Checking the project folder…
project-vcs-enable = Enable Version Control…
project-vcs-auto-commit = Commit every save
project-vcs-auto-commit-help = Each save of a project file is committed to the project's repository. Turned off, saves are not committed; the repository and its history stay as they are.
project-vcs-auto-commit-needs-repo = Enable version control first: the project folder has no Git repository.
project-vcs-pending = Commits in progress: { $count }

# Graphite look (mockup rows 31 and 33).
project-sheets-root = root
project-sheets-missing = missing
project-sheets-add-schematic = Add New ▸ Schematic
project-sheets-preview = Preview
# $size is the sheet size name, such as A4 or ANSI B.
project-sheets-preview-landscape = { $size } landscape
project-sheets-preview-portrait = { $size } portrait
project-sheets-preview-mm = { $width } × { $height } mm
project-libraries-where = Add, remove and rename libraries in the project tree: right-click the project, then Add New to Project ▸ Component Library.

# ERC, version control look (mockup rows 34 and 35).
project-erc-source-machine = User Default
project-erc-source-project = Stored in project
project-erc-source-unreadable = Newer version
project-erc-by-severity = Rules by severity
project-erc-by-severity-caption = (how the { $total } rules are set, not ERC results)
project-erc-tile-error = rules set to Error
project-erc-tile-warning = rules set to Warning
project-erc-tile-info = rules set to Info
project-erc-tile-off = rules turned off
project-erc-open-dialog = Open Electrical Rules Check…
project-vcs-state-present = Present
project-vcs-state-absent = None
project-vcs-state-checking = Checking…
project-vcs-status = Status
project-vcs-open-history = Open History panel

# Branch, last commit and author (mockup rows 35 and 39).
project-vcs-branch = Branch
project-vcs-branch-count = { $count ->
    [one] 1 local branch
   *[other] { $count } local branches
}
project-vcs-branch-detached = Detached HEAD
project-vcs-branch-current = current
project-vcs-branch-ahead = { $count } ahead
project-vcs-branch-behind = { $count } behind
project-vcs-branch-ahead-behind = { $ahead } ahead · { $behind } behind
project-vcs-switcher-heading = Local branches
# Messages panel, after a branch switch.
project-vcs-switched = Switched to branch { $branch }. Clean documents were reloaded; documents with edits offer Reload.
project-vcs-switch-unsaved = { $file } has unsaved edits. Save or discard them before switching; Signex never saves for you.
project-vcs-switch-busy = A save, commit or restore is still running. Try again when it finishes.
project-vcs-switch-unknown = That branch no longer exists.
project-vcs-switch-no-commits = The repository has no commit yet, so there is nothing to switch from.
# $file is a path inside the repository.
project-vcs-switch-held = { $file } is open and differs on that branch. Close it, then switch.
project-vcs-switch-project-unreadable = { $file } is missing or unreadable on that branch, so the project could not reopen. Nothing was changed.
project-vcs-switch-lfs = The Git LFS content of { $file } on that branch is not in this repository. Fetch it with your Git tool, then switch.
# Button under the unsaved-edits notice, for a branch with another project file.
project-vcs-switch-save-and-switch = Save and switch
# Error card: the branch did switch, the project did not follow.
project-vcs-switch-reopen-title = Project not reopened
project-vcs-switch-reopen-failed = The branch was switched, but { $file } could not be reopened: { $why }. Close the project and open it again.
project-vcs-switch-reopen-held = The branch was switched, but { $file } has edits or work still running, so it stayed open with its old content. Save or discard them, then close the project and open it again.
project-vcs-switch-overwrite = Files changed since their last commit would be overwritten. Commit them first.
project-vcs-switch-failed = The branch was not switched: { $why }
# $file is a path inside the repository.
project-vcs-switch-unsafe-path = { $file } on that branch would be written outside the project's files, so the branch was not switched.
# $files is a comma-separated list of paths inside the repository.
project-vcs-switch-not-restored = The branch was not switched, and these files could not be put back: { $files }. Check them before your next commit.
project-vcs-last-commit = Last commit
# $sha is the short commit id, $age how long ago, as the History panel says it.
project-vcs-last-commit-line = { $sha } · { $subject } · { $age }
project-vcs-no-commits = No commits yet
project-vcs-read-failed = The repository could not be read: { $why }
project-vcs-author = Author
project-vcs-author-help = Commits are written as this person. Enter saves it to this repository's git config; left empty, your own git identity applies.
project-vcs-author-placeholder = Name <email>
project-vcs-author-invalid = Write the author as Name <email>.
project-vcs-author-failed = The author was not saved: { $why }

# Schematic defaults (mockup row 36).
project-defaults-help = Unset widths and sizes on every sheet of this project follow these. Leave a field empty for the built-in value.
project-defaults-lines = Lines
project-defaults-text-markers = Text and markers
project-defaults-wire-width = Wire width
project-defaults-bus-width = Bus width
project-defaults-symbol-line-width = Symbol line and pin width
project-defaults-shape-stroke = Shape stroke
project-defaults-text-size = Text size
project-defaults-junction-diameter = Junction diameter
project-defaults-unit-mm = mm
project-defaults-unit-pt = pt
project-defaults-font = Font
project-defaults-wire-line-style = Wire line style
project-defaults-shape-fill = New shape fill
project-defaults-built-in = Built-in ({ $value })
project-defaults-line-solid = Solid
project-defaults-line-dashed = Dashed
project-defaults-line-dotted = Dotted
project-defaults-fill-none = No fill
project-defaults-fill-outline = Outline colour
project-defaults-fill-background = Background colour
project-defaults-reset = Reset to built-in ({ $value })
# A typed value the field refuses: the stored default stays. $min and $max are
# millimetres.
project-defaults-refused = Not applied: enter { $min } to { $max } mm.
# The same for the text size. $min and $max are points.
project-defaults-refused-pt = Not applied: enter { $min } to { $max } pt.
project-defaults-caption = Grey values are the built-in defaults. An edited field has an accent border and a reset button; an invalid value has a red border, a tinted fill and a warning icon.
project-defaults-preview = Preview
project-defaults-preview-caption = schematic canvas · click a value to edit · empty = built-in
project-defaults-preview-wire = wire { $value }
project-defaults-preview-bus = bus { $value }
project-defaults-preview-line = line { $value }
project-defaults-preview-stroke = stroke { $value }
project-defaults-preview-text = text { $value } pt
project-defaults-preview-junction = ⌀ { $value }
# The larger preview window (mockup rows 31 and 36).
project-preview-expand = Expand preview
project-preview-close = Close
project-preview-hint-values = Click a value on the drawing to edit it · Tab moves to the next · Esc closes
project-preview-hint-sheet = Esc closes
# dialog-polish
# The result pill of a project variable's `=` expression that does not
# evaluate (mockup row 30); the reason is in the Error line under the row.
project-parameter-unresolved = ?
