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

# Row actions shared by the sections' tables.
project-move-up = Move up
project-move-down = Move down

# Sheets section (#1689).
project-sheets-help = The project lists its sheets in this order: the Sheet N of M a sheet shows in its title block and the order of the project tree follow it. PDF export numbers its pages by the sheet hierarchy instead; number child sheets with Tools ▸ Annotation ▸ Number Schematic Sheets.
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
project-variants-new = New variant name
project-variants-add = Add
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
project-variants-read-failed = The parts could not be read: { $reason }
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
project-libraries-empty = This project mounts no libraries. Right-click the project in the project tree, then Add New to Project ▸ Component Library.

# ERC and annotation section (#1692).
project-erc-annotation-help = Rules and annotation settings stored in this project travel with the project file. A project that stores none uses this machine's Preferences.
project-erc-title = ERC rules
project-erc-stored = This project stores its own ERC rules; they travel with the project file.
project-erc-machine = This project uses this machine's ERC rules (User Default). Store them in the project so they travel with it.
project-erc-unreadable = This project holds ERC rules from a newer version of Signex; they are kept and not applied.
project-erc-store = Store ERC rules in this project
project-erc-edit-hint = Change a rule's severity in Project ▸ Electrical Rules Check.
project-erc-col-rules = Rules
project-erc-col-severity = Severity
project-erc-rule-count = { $count }
project-erc-severity-error = Error
project-erc-severity-warning = Warning
project-erc-severity-info = Info
project-erc-severity-off = Off
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
project-vcs-auto-commit-help = Each save of a project file is committed to the project's repository. Turned off, saves are not committed and the repository stays.
project-vcs-auto-commit-needs-repo = Enable version control first: the project folder has no Git repository.
project-vcs-pending = Commits in progress: { $count }
