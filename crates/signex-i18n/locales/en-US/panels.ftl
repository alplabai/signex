# Panels, the dock, the status bar, the tab bar, the active bar and the Start
# screen (#2225).

## ERC panel

# One-letter severity pill of an ERC row: error.
panels-erc-severity-error = E
# One-letter severity pill of an ERC row: warning.
panels-erc-severity-warning = W
# One-letter severity pill of an ERC row: information.
panels-erc-severity-info = I

## Messages panel. The column headings and level pills are shown in
## capitals, so they are written in capitals here.

panels-messages-column-class = CLASS
# The document a message is about.
panels-messages-column-document = DOCUMENT
# The code of the part of Signex that logged the message.
panels-messages-column-source = SOURCE
panels-messages-column-message = MESSAGE
panels-messages-column-time = TIME

# Shown when nothing has been logged.
panels-messages-empty = No runtime messages yet

# Hint under the empty state; the environment variables that turn on verbose
# logging follow it on their own line, in mono.
panels-messages-verbose-hint = Set one of these for verbose output:

# A cell with no value (messages carry no document or time yet).
panels-messages-no-value = —

# Level pills of a message row.
panels-messages-level-error = ERROR
panels-messages-level-warning = WARN
panels-messages-level-info = INFO
panels-messages-level-debug = DEBUG
panels-messages-level-trace = TRACE

## Components panel

# Placeholder of the components search field; 100n and 0.1u are example
# values the value-aware search finds (mockup row 52).
panels-components-filter = Search, try 100n or 0.1u

# Ghost button that opens the Add library menu.
panels-components-add = Add

# A library count on a source chip, or the component count of a library row.
# $count is a whole number.
panels-components-count = { $count }

# Source chip: every library.
panels-components-source-all = All
# Source chip and Add menu entry: the open project's libraries.
panels-components-source-project = Project
# Source chip and Add menu entry: libraries opened for this session.
panels-components-source-installed = Installed
# Source chip and Add menu entry: libraries kept for every session.
panels-components-source-global = Global
# Source chip: the library that ships with Signex.
panels-components-source-stock = Stock

# Source badge of a library row, in capitals.
panels-components-badge-project = PROJECT
panels-components-badge-installed = INSTALLED
panels-components-badge-global = GLOBAL
panels-components-badge-stock = STOCK

# Title of the empty-list card: nothing is mounted.
panels-components-nothing-mounted = No libraries mounted
# Empty Project section: no project is open.
panels-components-empty-no-project = No project libraries. Open a project to populate this section.
# Empty Project section: the open project declares no library. The path
# names the project tree's context menu entries.
panels-components-empty-none-declared = This project has no libraries yet. Right-click the project, then Add New to Project > Component Library.
# Empty Project section: the project's libraries could not be mounted.
panels-components-empty-not-mounted = This project's libraries are not mounted. See the project tree.
# Empty Installed section.
panels-components-empty-installed = No session libraries. Click "+ Add Library…" to mount one.
# Empty Global section.
panels-components-empty-global = No global libraries. Click "+ Add Library…" or promote an installed library.
# Empty Stock section: the bundled library is missing.
panels-components-empty-stock = Stock library not found. Reinstall Signex to restore the bundled parts.

# Section labels of the selected component's details, in capitals.
panels-components-details-symbol = SYMBOL
panels-components-details-footprint = FOOTPRINT
panels-components-details-parameters = PARAMETERS

## PCB Nets panel. The column headings are shown in capitals, so they are
## written in capitals here.

# Shown when no board is open.
panels-pcb-nets-no-board = Open a `.snxpcb` to see its nets here.
panels-pcb-nets-column-net = NET
# How many pads the net connects.
panels-pcb-nets-column-nodes = NODES
# The routed length of the net, in millimetres.
panels-pcb-nets-column-length = LENGTH
# The net class a net belongs to.
panels-pcb-nets-column-class = CLASS
# Class cell of a net in no class.
panels-pcb-nets-no-class = —
# The PCB panel's mode dropdown.
panels-pcb-mode-nets = Nets
panels-pcb-mode-net-classes = Net Classes
# Placeholder of the field that filters the net list by name.
panels-pcb-nets-filter = Filter nets
# Chips over the net list; the others are net class names.
panels-pcb-nets-chip-all = All
panels-pcb-nets-chip-unrouted = Unrouted
# The chip and the filter leave no net in the list.
panels-pcb-nets-no-match = No nets match this filter.
# Detail of the selected net. $count is a whole number.
panels-pcb-nets-detail-pads = Pads { $count }
# One pad: its part's designator and its number.
panels-pcb-nets-detail-pad = { $part }-{ $pad }
# Where a drilled or multi-layer pad is.
panels-pcb-nets-detail-multi = Multi
panels-pcb-nets-detail-tracks = Tracks and vias
panels-pcb-nets-detail-track-count =
    { $count ->
        [one] { $count } track
       *[other] { $count } tracks
    }
# One layer's row: the layer's name and its track count.
panels-pcb-nets-detail-layer-tracks = { $layer } · { $tracks }
panels-pcb-nets-detail-vias = Vias
# A length in millimetres; $length is already formatted.
panels-pcb-nets-detail-mm = { $length } mm
# Shown when the board has no nets.
panels-pcb-nets-empty = This board has no nets.
# Length cell of a net with nothing routed.
panels-pcb-nets-no-length = —
# Nodes cell. $count is a whole number.
panels-pcb-nets-nodes = { $count }
# Footer: the board's net and pad counts. Both are whole numbers.
panels-pcb-nets-summary =
    { $nets ->
        [one] { $nets } net
       *[other] { $nets } nets
    } · { $pads ->
        [one] { $pads } pad
       *[other] { $pads } pads
    }

## Custom Selection Filters (Properties panel, and the active bar's Filter
## menu).

# Shown when no filter preset exists.
panels-filters-no-presets = No presets yet. Click + to define one.
# Button: create the first preset.
panels-filters-add-filter = + Add Filter
# Button beside the preset tabs: add a preset.
panels-filters-add-preset = +
# Placeholder of the preset name field.
panels-filters-preset-name = Preset name
# Button: delete the shown preset.
panels-filters-delete = Delete
# Tab of a preset the user has not named. $number is its position, from 1.
panels-filters-unnamed = Filter { $number }

# Object kinds a selection filter can include.
panels-filter-components = Components
panels-filter-wires = Wires
panels-filter-buses = Buses
panels-filter-sheet-symbols = Sheet Symbols
panels-filter-sheet-entries = Sheet Entries
panels-filter-net-labels = Net Labels
panels-filter-parameters = Parameters
panels-filter-ports = Ports
panels-filter-power-ports = Power Ports
panels-filter-texts = Texts
panels-filter-drawing-objects = Drawing Objects
panels-filter-other = Other

## Properties panel

# Title of the panel when nothing is selected: the document's own options.
panels-properties-document-options = Document Options
# Subsection label: the measurement units.
panels-properties-units = Units
# Subsection label: the 3 × 3 text anchor picker.
panels-properties-justification = Justification

## Dock

# Count badge on a dock tab (for example the ERC panel's open violations).
# $count is a whole number.
panels-dock-tab-count = { $count }

# Panel names: dock tabs, panel titles and floating panel title bars.
panels-kind-projects = Projects
panels-kind-components = Components
panels-kind-navigator = Navigator
panels-kind-properties = Properties
panels-kind-filter = Filter
# Electrical rule check.
panels-kind-erc = ERC
panels-kind-messages = Messages
# Design rule check.
panels-kind-drc = DRC
panels-kind-layer-stack = Layer Stack
panels-kind-net-classes = Net Classes
panels-kind-variants = Variants
# Schematic filter.
panels-kind-sch-filter = SCH Filter
# Schematic object list.
panels-kind-object-list = Object List
# Bill of materials studio.
panels-kind-bom-studio = BOM Studio
panels-kind-favorites = Favorites
panels-kind-snippets = Snippets
panels-kind-todo = To-Do
panels-kind-wiki = Wiki
panels-kind-output-jobs = Export Jobs
panels-kind-library = Library
# Schematic symbol library.
panels-kind-sch-library = SCH Library
panels-kind-footprint-library = Footprint Library
# The board's net list.
panels-kind-pcb-nets = PCB
panels-kind-layers = Layers
# Version history.
panels-kind-history = History

## Status bar and document tabs

# Button that opens the list of panels.
panels-status-bar-panels = Panels

# Marker on the tab of a document that cannot be edited.
panels-tab-read-only = read-only

# Three-letter document kind code at the start of a document tab.
panels-tab-kind-schematic = SCH
panels-tab-kind-pcb = PCB
panels-tab-kind-symbol = SYM
panels-tab-kind-footprint = FPT
panels-tab-kind-library = LIB
panels-tab-kind-component = CMP
panels-tab-kind-project = PRJ

## Empty-state cards

# Selection Filter panel: its title while the panel has no backend.
panels-selection-filter-title = Selection Filter
# Selection Filter panel: the line under the title.
panels-selection-filter-body = All object types enabled

# Tooltip of every Start screen control that has no backend yet.
panels-coming-soon = Coming soon

## Start screen.

start-new-project = New project
# Button that opens a project; the ellipsis says a file dialog follows.
start-open = Open…
start-home = Home
start-projects = Projects
start-libraries = Libraries
# A planned part search; the row has no backend yet.
start-parts-search = Parts search
# Pill on a navigation row that opens nothing yet.
start-soon = soon
start-open-sample = Open Sample Project
# The row shows the key bound to the shortcut list beside this label.
start-keyboard-shortcuts = Keyboard Shortcuts
# Count at the right of a navigation row. $count is a whole number.
start-count = { $count }
# The key bound to a navigation row's action, at its right. $key is the
# key's label, such as F1.
start-row-key = { $key }
# Loaded libraries card: how many libraries are loaded and how many
# components they hold. Both are whole numbers.
start-libraries-summary =
    { $libraries } loaded · { $components ->
        [one] { $components } component
       *[other] { $components } components
    }
start-hero-title = What are you building?
start-hero-body = Start a new project, open one you have, or begin from the reference design below.
# Section headings, shown in capitals.
start-reference-design = START FROM A REFERENCE DESIGN
start-recent-projects = RECENT PROJECTS
# The bundled sample project card.
start-sample-title = Sample project
start-sample-body = Open the bundled example project and look around.
# Recent projects list when it is empty.
start-no-recent-title = No recent projects
start-no-recent-body = Projects you open appear here.
# Section heading above the pinned projects, shown in capitals.
start-pinned-projects = PINNED PROJECTS
# Project row actions (hover button and right-click menu): keep the
# project on the Start screen, take it off, or drop a pinned project that
# is no longer on disk.
start-pin = Pin
start-unpin = Unpin
start-remove = Remove
# Right-click menu row that opens the project.
start-open-project = Open
# Shown on a pinned project whose file is no longer on disk.
start-project-missing = Missing on disk
# Messages panel warning when a pinned project is clicked but its file is
# gone. $path is the project file's path.
start-pin-missing-warning = { $path } is no longer on disk; the pin is kept.
# Messages panel warning when the start-up check of the pinned projects
# could not run. $reason is the error text.
start-pin-check-failed = Could not check the pinned projects on disk: { $reason }
# Project tile of a project with no cached preview yet; the tile is small,
# so keep it short.
start-thumbnail-none = No preview yet
# Tooltips of the project tile.
start-thumbnail-board-hint = Board preview (cached on save)
start-thumbnail-sheet-hint = Root sheet preview (no board yet)
start-thumbnail-none-hint = Preview is written the next time the project is saved
start-thumbnail-missing-hint = Project file not found
# Messages panel warning when a project preview could not be saved. $project
# is the project file's path, $reason the error text.
start-thumbnail-write-failed = Could not save the preview of { $project }: { $reason }
# Messages panel warning when the cached project previews could not be read
# at start-up. $reason is the error text.
start-thumbnail-read-failed = Could not read the cached project previews: { $reason }
# Link at the right of RECENT PROJECTS that opens the Projects page.
# $count is how many projects that page lists, a whole number.
start-show-all = Show all { $count } ›
# When a project was last opened, in the right column of its row.
# $time is the local clock time, hours and minutes.
start-opened-today = today { $time }
start-opened-yesterday = yesterday
# An earlier day. $date is the date in the user's locale.
start-opened-date = { $date }
# Shown when the time is not known, or the project is missing on disk.
start-opened-unknown = —
# Projects page: the line under the title, the filter field's
# placeholder, and the switch. $count is a whole number.
start-projects-body = Every project you have opened, newest first.
start-projects-filter = Filter by name or folder
start-scope-all = All { $count }
start-scope-pinned = Pinned { $count }
start-scope-missing = Missing { $count }
# Empty states of a filtered list. $query is the text in the filter field.
start-projects-no-match = No projects match “{ $query }”
start-filter-no-match-body = Check the spelling, or clear the filter.
start-clear-filter = Clear filter
# Projects page with the Pinned or Missing switch on and nothing to list.
start-projects-none-pinned = No pinned projects
start-projects-none-pinned-body = Pin a project from its row on Home or from its right-click menu.
start-projects-none-missing = No missing projects
start-projects-none-missing-body = Every pinned project is on disk.
# Libraries page: the line under the title, the toolbar and the table.
start-libraries-body = Libraries loaded for every project. A project can add its own in Project Options.
start-libraries-filter = Filter libraries
start-add-library = Add library…
start-open-library-browser = Open Library Browser
# Column headings of the libraries table, shown in capitals.
start-libraries-column-library = LIBRARY
start-libraries-column-scope = SCOPE
start-libraries-column-components = COMPONENTS
# Caption under the library that comes with the install.
start-libraries-stock-caption = ships with Signex · read-only
start-libraries-no-match = No libraries match “{ $query }”
start-libraries-none = No libraries loaded
start-libraries-none-body = Add a library to use its parts in every project.
# A project is open but no document is shown.
start-hint-title = No document selected
start-hint-body = Choose a schematic or PCB from the project tree, or open one here.
# The open project has no schematic sheet and no board yet.
start-hint-empty-title = This project has no documents yet
start-hint-empty-body = Start with a schematic sheet; add the PCB once the schematic has parts.
start-hint-new-sheet = New sheet
start-hint-new-pcb = New PCB
start-hint-project-options = Project Options…

## Projects panel

# Project card counts, joined by " · ". $count is a whole number.
panels-projects-sheets =
    { $count ->
        [one] { $count } sheet
       *[other] { $count } sheets
    }
panels-projects-boards =
    { $count ->
        [one] { $count } board
       *[other] { $count } boards
    }
panels-projects-libraries =
    { $count ->
        [one] { $count } library
       *[other] { $count } libraries
    }
# Pill on a project card when the project is under version control (git is
# the tool's name).
panels-projects-git = git
# Placeholder of the project file filter.
panels-projects-filter = Filter files
# Tooltip of the gear button on a project card; it opens that project's
# options in a tab.
panels-projects-options = Project Options…
# Pill behind a file the project lists but the disk does not have.
panels-projects-pill-missing = missing
# Pill behind a file that exists but could not be read.
panels-projects-pill-unreadable = unreadable
# Tooltip of the clear (x) button inside the file filter.
panels-projects-clear-filter = Clear filter
# Card shown when the file filter matches no file. $filter is the text the
# user typed.
panels-projects-no-match-title = No files match "{ $filter }"
panels-projects-no-match-body = Clear the filter to see every file.
# Buttons of the Projects panel when no project is open; the ellipsis says a
# dialog follows.
panels-projects-new-project = New Project…
panels-projects-open = Open…
# Section heading above the recently opened projects when no project is
# open, shown in capitals.
panels-projects-recent = Recent projects

## Net Classes section of the Properties panel

# Button: add a net class.
panels-net-classes-add = Add Class
# Tooltip of the undo button of the net class list.
panels-net-classes-undo = Undo
# Tooltip of the redo button of the net class list.
panels-net-classes-redo = Redo
# Button: move the selected class up (earlier classes win).
panels-net-classes-move-up = Move Up
# Button: move the selected class down.
panels-net-classes-move-down = Move Down
# Button: delete the selected class.
panels-net-classes-delete = Delete

## Properties panel: a placed part and a component row

# Header of the Properties panel for a placed library part.
panels-properties-component = Component
# Button: drop the sheet symbol's own colours and use the defaults.
panels-properties-reset-style = Reset to Default

# Fields of a component row's sheet in the Properties panel.
panels-component-sheet-internal-pn = Internal PN
# Manufacturer part number.
panels-component-sheet-mpn = MPN
panels-component-sheet-manufacturer = Manufacturer
panels-component-sheet-lifecycle = Lifecycle
panels-component-sheet-version = Version
panels-component-sheet-library = Library
panels-component-sheet-table = Table
panels-component-sheet-updated = Updated
# Shown in place of an empty field.
panels-component-sheet-no-value = —
# Section titles of a component row's sheet; shown upper-cased.
panels-component-sheet-section-component = Component
panels-component-sheet-section-storage = Storage

## Projects panel footer summary

# Caption: electrical rule check results.
panels-summary-erc = ERC
# Caption: design rule check results.
panels-summary-drc = DRC
# Caption: how long ago the last git commit was made.
panels-summary-last-commit = Last commit
# The check has not run on this project yet.
panels-summary-not-run = Not run
# DRC result: violations in the last run. $count is a whole number.
panels-summary-drc-violations = { $count ->
        [one] { $count } violation
       *[other] { $count } violations
    }
# ERC result counts. Both are whole numbers.
panels-summary-erc-counts =
    { $errors ->
        [one] { $errors } error
       *[other] { $errors } errors
    } · { $warnings } warn
# Age of the last commit, under a minute.
panels-summary-age-now = just now
# Age in whole minutes, hours or days.
panels-summary-age-minutes = { $count } min ago
panels-summary-age-hours = { $count } h ago
panels-summary-age-days = { $count } d ago

## Footprint Library panel buttons

# Place the selected footprint on the board (not wired yet, shown disabled).
panels-footprint-library-place = Place
panels-footprint-library-add = Add
panels-footprint-library-delete = Delete
panels-footprint-library-edit = Edit

## Net Inspector panel (#1269): every net of the active project. The column
## headings are shown in capitals, so they are written in capitals here.

panels-kind-net-inspector = Net Inspector
# Placeholder of the box that filters the list by net or class name.
panels-net-inspector-filter = Filter nets
# Shown when the active document is not a schematic of a project.
panels-net-inspector-no-project = Open a schematic to see its project's nets here.
# Shown while the project's connectivity is being read.
panels-net-inspector-reading = Reading the project's nets…
# Shown when the project's connectivity could not be read.
panels-net-inspector-unavailable = The project's nets could not be read; Messages says why.
# Shown when no net matches the filter.
panels-net-inspector-empty = No net matches the filter.
panels-net-inspector-column-net = NET
# The net class the net belongs to.
panels-net-inspector-column-class = CLASS
# How many component pins the net connects.
panels-net-inspector-column-pins = PINS
# Pins cell. $count is a whole number.
panels-net-inspector-pins = { $count }
# Class cell of a net in no net class.
panels-net-inspector-no-class = —
# What a member of an expanded net is.
panels-net-inspector-kind-pin = Pin
panels-net-inspector-kind-label = Label
panels-net-inspector-kind-port = Power port
panels-net-inspector-kind-sheet-entry = Sheet entry
# Footer: how many nets the filter shows of the project's total. Whole numbers.
panels-net-inspector-summary =
    { $shown } of { $total ->
        [one] { $total } net
       *[other] { $total } nets
    }

## Messages panel: short source names
panels-messages-source-app = App
panels-messages-source-tasks = Tasks
panels-messages-source-erc = ERC
panels-messages-source-drc = DRC
panels-messages-source-library = Library
panels-messages-source-preferences = Preferences
panels-messages-source-sketch = Sketch
panels-messages-source-project = Project
panels-messages-source-gerber-viewer = Gerber Viewer
panels-messages-source-graphics = Graphics
panels-messages-source-interface = Interface
panels-messages-source-system = System

## Graphite views (#2225)
panels-status-bar-grid = Grid { $grid }
panels-status-bar-grid-off = Grid OFF
panels-status-bar-snap-on = Snap on
panels-status-bar-snap-off = Snap off
# The active tool's name alone; the live hint follows it.
panels-status-bar-tool = { $tool }
panels-status-bar-importing = Importing { $name }… ✕
# The cursor position. $value is the coordinate as a number; the unit is
# written once after Y.
panels-status-bar-x = X { $value }
panels-status-bar-y = Y { $value }
# The zoom level; $percent is a whole number, already written out.
panels-status-bar-zoom = { $percent }%
# Shown while saved documents are being committed to the project history;
# $count is how many commits are still running.
panels-status-bar-saving = { $count ->
        [one] Saving…
       *[other] Saving { $count }…
    }
# The canvas selection when it holds one kind of object; $kind is one of the
# panels-status-bar-selected-* texts below.
panels-status-bar-selection-one-kind = Sel: { $kind }
# The canvas selection when it holds several kinds; $count is the total and
# $kinds the panels-status-bar-selected-* texts, separated by commas.
panels-status-bar-selection-kinds = Sel: { $count } ({ $kinds })
# The selection while a click has picked a pin of the selected part (mockup
# row 50); $selection is one of the two texts above and $pin the pin number.
panels-status-bar-selection-pin = { $selection } · pin { $pin }
panels-status-bar-selected-components = { $count ->
        [one] { $count } component
       *[other] { $count } components
    }
panels-status-bar-selected-wires = { $count ->
        [one] { $count } wire
       *[other] { $count } wires
    }
panels-status-bar-selected-labels = { $count ->
        [one] { $count } label
       *[other] { $count } labels
    }
panels-status-bar-selected-shapes = { $count ->
        [one] { $count } shape
       *[other] { $count } shapes
    }
# Selected objects that are none of the kinds above.
panels-status-bar-selected-other = { $count } other
# A drag in progress that carries the wires along. $key is the modifier key
# that flips it (Alt).
panels-status-bar-drag-with-wires = Dragging with wires ({ $key }: plain move)
# The same, once the drag wiring mode (cycle_wiring_mode) is not automatic:
# how a corner inserted in a following wire leaves its fixed end.
panels-status-bar-drag-wires-horizontal-first = Dragging with wires, horizontal first ({ $key }: plain move)
panels-status-bar-drag-wires-vertical-first = Dragging with wires, vertical first ({ $key }: plain move)
panels-status-bar-drag-wires-straight = Dragging with wires, straight ({ $key }: plain move)
# The 45° modes: a 45° leg leaves the fixed end first, or reaches the pin last.
panels-status-bar-drag-wires-45-start = Dragging with wires, 45° first ({ $key }: plain move)
panels-status-bar-drag-wires-45-end = Dragging with wires, 45° last ({ $key }: plain move)
# A drag in progress that leaves the wires. $key is the modifier key.
panels-status-bar-move-without-wires = Moving without wires ({ $key }: drag with wires)
# Measure tool prompts.
panels-status-bar-measure-first = Click the first point
panels-status-bar-measure-second = Click the second point
# The Grid & Snap popover a right-click on the Grid read-out opens.
panels-grid-snap-title = Grid & Snap
# The label of a grid's spacing field.
panels-grid-snap-pitch = Pitch
# A pitch callout on the drawing; $pitch is the spacing with its unit symbol.
panels-grid-snap-pitch-value = { $pitch }
panels-grid-snap-foot = The same settings as Properties ▸ Document Options ▸ General; a change here shows there.
# Group captions of the status bar's panel list.
panels-list-group-project = Project
panels-list-group-inspect = Inspect
panels-list-group-verify = Verify
panels-list-group-library = Library
panels-list-group-pcb = PCB
# The installed extensions' panels, in the panel list and under View; not
# shown when no extension is installed.
panels-list-group-extensions = Extensions
# Where an open panel is, at the right of its row in the panel list.
panels-list-region-left = left
panels-list-region-right = right
panels-list-region-bottom = bottom
panels-list-region-floating = floating
# A panel detached into a window of its own.
panels-list-region-window = window
# The PCB panel's row in the panel list, named for its first mode.
panels-list-row-nets = Nets
# Foot of the panel list: where the tool documents went.
panels-list-foot = Layer Stack, Rules and other tool documents open as tabs, from menus or the command palette.
panels-erc-running = Running…
panels-erc-no-findings-left = No findings left since the last run
panels-erc-empty-hint = Run ERC to populate project-wide diagnostics
panels-erc-add-no-connect = Add No-Connect
panels-erc-waive-net = Waive net
panels-erc-restore = Restore
panels-properties-needs-schematic = Properties are available when a schematic is active
panels-properties-open-project = Open a project
panels-properties-nothing-selected = Nothing selected
panels-properties-one-object-selected = 1 object selected
panels-properties-tab-general = General
panels-properties-tab-parameters = Parameters
panels-properties-template-defaults = Template: using A-series defaults
panels-canvas-font-title = Canvas Font Settings
panels-canvas-font-close = Close
panels-canvas-font-family = Family
panels-canvas-font-size = Size
panels-canvas-font-style = Style
panels-canvas-font-bold = Bold
panels-canvas-font-italic = Italic
panels-canvas-font-hint = Applies immediately to canvas text rendering.
panels-shape-fill = Fill
panels-shape-fill-none = None
panels-shape-fill-outline = Outline
panels-shape-fill-background = Background
panels-net-classes-no-project = Open a project to define net classes.
panels-net-classes-empty = No net classes. A net in no class draws as before.
panels-net-classes-order-hint = The first matching class wins; reorder to change it.
panels-net-classes-name-hint = Class name
panels-net-classes-color-hint = #RRGGBB, empty = theme
panels-net-classes-width-hint = empty = wire's own
panels-net-classes-nets-hint = GND, +3V3
panels-net-classes-patterns-hint = VCC*, ?SDA
# Section title in Document Options; shown upper-cased.
panels-net-classes-section = Net Classes
# Row labels of the selected net class.
panels-net-classes-name = Name
panels-net-classes-color = Color
panels-net-classes-width = Width (mm)
panels-net-classes-line = Line
# Line pick-list entries. Default follows the project's line style.
panels-net-classes-line-default = Default
# Net Class pick-list entry of a net label: in no class.
panels-net-classes-no-class = (none)
panels-net-classes-line-solid = Solid
panels-net-classes-line-dashed = Dashed
panels-net-classes-line-dotted = Dotted
panels-net-classes-nets = Nets
panels-net-classes-patterns = Patterns
panels-child-sheet-name = Name
panels-child-sheet-file = File

## ERC panel counts and waivers. $count is a whole number.
panels-erc-count-errors = { $count } errors
# A net waiver: the rule id, the net name and how many findings it hid in the
# newest run.
panels-erc-waiver-row = { $rule }  net { $net }  ({ $hidden ->
        [0] matches no finding
        [one] { $hidden } finding hidden
       *[other] { $hidden } findings hidden
    })

## Components panel
panels-components-dismiss = Dismiss
# Rows of one library beyond the ones shown. $count is a whole number.
panels-components-more-rows = { $count } more — refine search
panels-components-no-matches-in-library = (no matches in this library)

## SCH Library panel
panels-sch-library-empty = Open a `.snxsym` to see its symbols here. Right-click a library node in the project tree and pick `Add New ▸ Symbol` to create one.
# Stands in for the file name of a symbol file not saved yet.
panels-sch-library-untitled-file = <untitled>
# $library is the library name, $file the symbol file; $count is a whole number.
panels-sch-library-breadcrumb-in-library = { $library }  ›  { $file }  ({ $count ->
        [one] { $count } symbol
       *[other] { $count } symbols
    })
panels-sch-library-breadcrumb = { $file }  ({ $count ->
        [one] { $count } symbol
       *[other] { $count } symbols
    })
panels-sch-library-column-design-item-id = Design Item ID
panels-sch-library-column-description = Description
panels-sch-library-part-shared = Part 0 (shared)
panels-sch-library-part = Part { $part }
panels-sch-library-place = Place
panels-sch-library-add = Add
panels-sch-library-delete = Delete
panels-sch-library-edit = Edit
panels-sch-library-delete-warning-note = Deleting it leaves placed parts on their embedded copy; the library symbol will be missing.
panels-sch-library-delete-anyway = Delete anyway
panels-sch-library-delete-cancel = Cancel

## Footprint Library panel
panels-footprint-library-empty = Open a `.snxfpt` to see its footprints here. Right-click a `.snxlib` (or a project) in the project tree and pick `Add New ▸ Footprint Library` to create one.
# Stands in for a file name that cannot be read.
panels-footprint-library-unknown-file = <unknown>
# $file is the footprint name; $count is a whole number.
panels-footprint-library-breadcrumb = { $file }  ({ $count ->
        [one] { $count } footprint
       *[other] { $count } footprints
    })
# Column headings of the Footprint Library list, in the section-label case.
panels-footprint-library-column-name = FOOTPRINT
panels-footprint-library-column-pads = PADS

## Library Browser row in the Properties panel
panels-library-row-title = Library Row
panels-library-row-unnamed = (unnamed row)
# $table, $class and $lifecycle come from the row; $row is a short row id.
panels-library-row-summary = table: { $table }  ·  class: { $class }  ·  { $lifecycle }  ·  row { $row }
panels-library-row-symbol = Symbol
panels-library-row-footprint = Footprint
panels-library-row-pick-symbol = Pick Symbol…
panels-library-row-pick-footprint = Pick Footprint…
# Caption under the row's part number (mockup row 127). $table is the table name; $row
# is the row's place in its table, or a short row id.
panels-library-row-heading = Library Row · { $table } · row { $row }
# After the lifecycle in the row's pill line. $version is the row's version.
panels-library-row-version = · v{ $version }
panels-library-row-section-bindings = Bindings
# Button of a bound Symbol or Footprint card.
panels-library-row-pick = Pick…

## Drawing properties (lines, rectangles, circles, arcs, polygons)
panels-drawing-color = Color
panels-drawing-placement = Placement
panels-drawing-placement-behind = Behind circuit
panels-drawing-placement-above = Above circuit
# Stroke width of a closed shape.
panels-drawing-border = Border
# Colour tile: the theme's own colour.
panels-drawing-color-auto = Auto

## Net Classes table (#2417): column titles and cells.

panels-net-classes-column-name = Name
panels-net-classes-column-color = Color
panels-net-classes-column-width = Width
panels-net-classes-column-members = Members
# Colour cell of a class with no colour of its own: the theme's wire colour.
panels-net-classes-theme-color = Theme
# Width cell of a class with a width; $width is in millimetres.
panels-net-classes-width-cell = { $width } mm
# Width cell of a class with no width: each wire keeps its own.
panels-net-classes-wire-width = Wire's own
panels-net-classes-no-members = None
# SCH Filter panel (#2417, Graphite mockup #56), over the kind toggles.
panels-sch-filter-hint = What a click, a box and the hover pick can select.

# Properties of a placed object (#2418). $unit is the panel's length unit (mm, mil, in, um).
panels-placed-location = Location
panels-placed-properties = Properties
panels-placed-rotation = Rotation
# Shown above the disabled rows of a locked object; $reason is the lock's own words.
panels-placed-locked = Locked: { $reason }
panels-placed-parameters = Parameters ({ $count })
panels-placed-parameters-none = Parameters (none)
panels-placed-no-parameters = No custom parameters
# Read-only facts of a wire, bus or sheet entry in Properties (#2418).
panels-placed-end = End
panels-placed-length = Length
panels-placed-name = Name
panels-placed-direction = Direction
panels-placed-sheet = Sheet
panels-placed-sheet-file = Sheet File
# The UI name of a Blanket directive: a drawn region whose parameters apply
# to every net inside it.
panels-directive-blanket = Net Blanket
panels-directive-net-class = Net class

## Net Inspector panel (Graphite mockup #58): the By Net / By Component modes, the part
## list, the empty-state cards and the footer. Column headings are shown in
## capitals, so they are written in capitals here.

# Segments of the mode control; $count is how many rows each list has.
panels-net-inspector-by-net = By Net { $count }
panels-net-inspector-by-component = By Component { $count }
# Placeholder of the filter box in By Component mode.
panels-net-inspector-filter-parts = Filter parts
panels-net-inspector-column-part = PART
panels-net-inspector-column-value-footprint = VALUE · FOOTPRINT
# Follows a part row's value in the same cell: the part's footprint.
panels-net-inspector-footprint-suffix = · { $footprint }
# Sub-header of an expanded part, muted, in sentence case.
panels-net-inspector-column-pin = Pin
panels-net-inspector-column-pin-net = Net
panels-net-inspector-column-pin-name = Pin name
panels-net-inspector-column-pin-sheet = Sheet
# Net cell of a pin on no net.
panels-net-inspector-unconnected = — unconnected
# The row under a long expanded list; clicking it lists the rest. $count is a whole number.
panels-net-inspector-more-pins =
    { $count ->
        [one] + { $count } more pin
       *[other] + { $count } more pins
    }
# The same row under an expanded net, whose list holds labels, ports and
# sheet entries as well as pins. $count is a whole number.
panels-net-inspector-more-members = + { $count } more
# Footer in By Component mode. Whole numbers.
panels-net-inspector-parts-summary =
    { $shown } of { $total ->
        [one] { $total } part
       *[other] { $total } parts
    }
panels-net-inspector-footer-hint = Click a row to go to it
# Empty-state card titles.
panels-net-inspector-no-project-title = No nets to show
panels-net-inspector-reading-title = Reading nets
panels-net-inspector-unavailable-title = Nets unavailable
panels-net-inspector-open-messages = Open Messages
# Under "No net matches the filter."; $filter is the filter text, $total a whole number.
panels-net-inspector-empty-body = Nothing named { $filter } in { $total ->
        [one] { $total } net
       *[other] { $total } nets
    }.
panels-net-inspector-empty-parts = No part matches the filter.
panels-net-inspector-empty-parts-body = Nothing named { $filter } in { $total ->
        [one] { $total } part
       *[other] { $total } parts
    }.
panels-net-inspector-clear-filter = Clear filter
# Under a no-match title while the filter is empty: the project lists nothing.
panels-net-inspector-empty-none = The project has nothing to list here yet.

## Components panel rows, chips, Recently placed and empty states (#2535)

# Body of the "No libraries mounted" card.
panels-components-nothing-mounted-body = Use “+ Add” or open a project.
# Buttons of the "No libraries mounted" card.
panels-components-add-library = Add Library…
panels-components-open-project = Open Project…
# Caption at the right of the source chips. $count is a whole number.
panels-components-library-count = { $count ->
        [one] { $count } library
       *[other] { $count } libraries
    }
# Library header count while a filter is active. $shown and $total are whole numbers.
panels-components-shown-of-total = { $shown } of { $total }
# No part matches the filter. $filter is the typed text.
panels-components-no-match = No components match “{ $filter }”
# Button that empties the filter.
panels-components-clear-filter = Clear
# Inline button on a hovered row, a recent part and the details title row.
panels-components-place = Place
# Switch next to Add: rows with a symbol thumbnail and a description.
panels-components-previews = Previews
panels-components-previews-tooltip = Rows with symbol preview (remembered as a preference)
# Section title above the parts placed from this panel, in capitals.
panels-components-recent-title = RECENTLY PLACED
# How often a recent part was placed this session. $count is a whole number.
panels-components-recent-uses = ×{ $count }
# Hint card before anything was placed from the panel.
panels-components-recent-empty = Parts you place from this panel show here.
# Second line of the details title: the description, then the library.
panels-components-details-subtitle = { $description } · { $library }
# Second line of the drag card while a row is dragged toward the canvas.
panels-components-drag-release = release to place

# Filter (Graphite mockup #55) and SCH Filter (Graphite mockup #56) panels: the kind toggles.
# All / None cells over the kind toggles.
panels-filter-all = All
panels-filter-none = None
# How many kinds are on; $on and $total are counts.
panels-filter-readout = { $on } of { $total } kinds
# Filter panel header: the editor it follows, then what it does.
panels-filter-editor-schematic = Schematic
panels-filter-editor-board = Board
panels-filter-follows = follows the active editor
panels-filter-counts-sheet = Counts are objects on this sheet.
panels-filter-counts-board = Counts are objects on this board.
# SCH Filter's Presets section.
panels-filter-presets = Presets
panels-filter-save-current = Save current
panels-filter-save-current-tip = Save the current kinds as a preset
# Save current at the limit; $limit is the most presets kept.
panels-filter-preset-limit = At most { $limit } presets: delete one to save another
panels-filter-preset-name-label = Name
panels-filter-no-presets = No presets yet. Save current keeps the kinds that are on.
# A preset and the kinds it turns on; $name is the preset's, $kinds a list of kind names.
panels-filter-preset-kinds = { $name }: { $kinds }
panels-filter-preset-no-kinds = { $name }: no kinds
# Shown while a preset is selected for editing: each kind chip click changes it; $name is the preset's.
panels-filter-preset-editing = Editing { $name }

## Object List panel (#1259): every object of the sheet or the project, listed
## and selectable. The panel title is panels-kind-object-list.

# Placeholder of the box that filters the list by name, value or text.
panels-object-list-filter = Filter objects
# Scope switch: only the sheet in view.
panels-object-list-scope-sheet = This sheet
# Scope switch: every sheet of the project.
panels-object-list-scope-project = Project
# Kind chip that shows every kind.
panels-object-list-chip-all = All
# Chip that opens the other kinds' chips.
panels-object-list-chip-more = More ▾
# The same chip while the other kinds are shown.
panels-object-list-chip-fewer = Fewer ▴
# Kind chips, one per kind of object.
panels-object-list-kind-part = Parts
panels-object-list-kind-label = Labels
panels-object-list-kind-wire = Wires
panels-object-list-kind-power-port = Power ports
panels-object-list-kind-port = Ports
panels-object-list-kind-sheet-symbol = Sheet symbols
panels-object-list-kind-sheet-entry = Sheet entries
panels-object-list-kind-text = Texts
panels-object-list-kind-junction = Junctions
panels-object-list-kind-bus = Buses
panels-object-list-kind-drawing = Drawing objects
panels-object-list-kind-directive = Directives
# One-letter kind marks in the first column. C is a placed part (component).
panels-object-list-letter-part = C
panels-object-list-letter-label = L
panels-object-list-letter-wire = W
panels-object-list-letter-power-port = P
panels-object-list-letter-port = O
panels-object-list-letter-sheet-symbol = S
panels-object-list-letter-sheet-entry = E
panels-object-list-letter-text = T
panels-object-list-letter-junction = J
panels-object-list-letter-bus = B
panels-object-list-letter-drawing = D
panels-object-list-letter-directive = X
# Column headings.
panels-object-list-column-name = Name
panels-object-list-column-value = Value / text
panels-object-list-column-sheet = Sheet
# Name column of an object that has no name of its own.
panels-object-list-noun-wire = Wire
panels-object-list-noun-text = Text
panels-object-list-noun-junction = Junction
panels-object-list-noun-bus = Bus
panels-object-list-noun-drawing = Drawing
panels-object-list-noun-blanket = Blanket
panels-object-list-noun-compile-mask = Compile mask
panels-object-list-noun-diff-pair = Differential pair
panels-object-list-noun-net-parameters = Net parameters
# Value column words.
panels-object-list-value-net-label = net label
panels-object-list-value-power-port = power port
panels-object-list-value-port = port
panels-object-list-value-sheet-entry = sheet entry
# Value column of a wire on a known net: the net, then the wire's length.
panels-object-list-value-wire = { $net }, { $length }
# The row under the drawn rows, while more rows match.
panels-object-list-more-rows = … { $count } more
# Caption under the list.
panels-object-list-hint = Click selects on the canvas, double-click zooms to it.
# Shown when no object matches the filter or the chips.
panels-object-list-empty = No object matches.
# Shown while the project's sheets are being read.
panels-object-list-reading = Reading the project's sheets…
# Empty state: no schematic in view (none open, or a board tab).
panels-object-list-no-schematic = Open a schematic to list its objects.
# Footer: the objects in scope and how many of them are selected.
panels-object-list-footer =
    { $total ->
        [one] { $total } object
       *[other] { $total } objects
    } · { $selected } selected

## ERC panel (#85): Run ERC, the count tiles, findings grouped by rule and
## the Waived group. The Run ERC key cap comes from the keymap.
panels-erc-run-button = Run ERC
panels-erc-clear-tooltip = Clear the ERC results
panels-erc-stale-banner = Results may be out of date: the sheet changed since the last run.
# Captions under the count tiles; $count is a whole number.
panels-erc-tile-errors =
    { $count ->
        [one] error
       *[other] errors
    }
panels-erc-tile-warnings =
    { $count ->
        [one] warning
       *[other] warnings
    }
panels-erc-tile-info = info
panels-erc-clean = No problems found.
# A clean run: what it checked and when. $time is the clock time, HH:MM.
panels-erc-clean-detail = { $sheets ->
        [one] 1 sheet
       *[other] { $sheets } sheets
    }, { $symbols ->
        [one] 1 symbol
       *[other] { $symbols } symbols
    } and { $nets ->
        [one] 1 net
       *[other] { $nets } nets
    } checked at { $time }.
# The second line of a finding: its sheet and position.
panels-erc-finding-place = { $sheet } · { $x }, { $y }
# The one row action that selects and frames a finding (AV-13).
panels-erc-show = Show
panels-erc-waived-group = Waived
# A net waiver in the Waived group: the rule id and the net.
panels-erc-net-waiver-title = { $rule } · net { $net }
# Who waived a finding and when, as they wrote it.
panels-erc-waived-by = by { $author } · { $date }

## The DRC waiver form: one required reason (mockup row 88).
panels-waiver-needs-reason = Write a reason to waive it.

## Messages panel (#86): level chips, source and document filters.
panels-messages-level-all = All
panels-messages-all-sources = All sources
panels-messages-all-documents = All documents
panels-messages-filter-hint = Filter messages
panels-messages-copy-tooltip = Copy the shown messages
panels-messages-clear-tooltip = Clear the messages
panels-messages-none-match = No messages match the filters.
# find-messages
# A Messages row's action: open the sheet the line names and select the object.
panels-messages-show = Show
# A schematic Net Inspector net row's hover action: draw the net bright on every sheet, or no longer.
panels-net-inspector-highlight = Highlight
# Logged when a Net Inspector row's Highlight names a net the project no longer has; $net is its name.
panels-net-inspector-net-gone = Highlight another net; net { $net } is no longer in the project.
# Tab + compact dock panel pair (#2523): the panel header's link that opens
# the same view as a document tab.
panels-paired-open-as-tab = Open as tab
# Title of the length-matching tab and its compact dock panel.
panels-paired-length-matching = Length Matching
# Compact group header caption: the group's member count, a whole number.
panels-compact-group-members =
    { $count ->
        [one] { $count } member
       *[other] { $count } members
    }
# Compact group header caption: the target length and its tolerance, both
# already formatted in the display unit.
panels-compact-group-band = { $target } ± { $tolerance }
# Status pill: how many members are outside the target band (whole number).
panels-compact-group-out-of-band = { $count } out of band
# Status pill: every member is inside the target band.
panels-compact-group-all-in-band = all in band
# Member caption: the member is one half of the differential pair $pair.
panels-compact-group-pair = pair { $pair }
# Member caption: the member is an xSignal (a net path through series parts).
panels-compact-group-xsignal = xSignal

## Messages lines for the notices an edit raises (signex_engine::EngineNotice).
## Pin names ("R7 pin 1") and net names are passed in as they are.

# A dragged wire could not keep what joined it at one point.
panels-messages-dragged-wire-left-one = could not keep { $name } on the dragged wire
# The same at several points; $names lists the first few.
panels-messages-dragged-wire-left-many = could not keep { $count } connections on the dragged wire: { $names }
# A grip drag of a wire end could not keep the wires attached at the end,
# which was at ($x, $y) mm.
panels-messages-grip-end-left = could not keep the wires at ({ $x }, { $y }) on the dragged end
# "Keep wires" held a pin on its net instead of joining it to the wire of
# another net its end now lies in.
panels-messages-held-pins-one = { $name } kept its net and was not joined to the wire it now ends on
panels-messages-held-pins-many = { $count } pins kept their nets and were not joined to the wires they end on: { $names }
# A pin end got no junction dot because the dot would merge two nets.
panels-messages-pin-dot-refused-one = could not join { $pin } to the wires it ends on: they carry two nets, { $through } and { $other }
# The same for several pins; each entry of $names is "R1 pin 2 (A, B)".
panels-messages-pin-dot-refused-many = could not join { $count } pins to wires of two nets they end on: { $names }
# One entry of that list: a pin and the two nets.
panels-messages-pin-dot-refused-entry = { $pin } ({ $through }, { $other })
# "Keep wires" left a connected pin as a plain move: keeping it connected
# would cross another net.
panels-messages-pins-not-kept-one = could not keep { $name } connected without crossing another net
# The same for several pins; $names lists the first few.
panels-messages-pins-not-kept-many = could not keep { $count } pins connected without crossing another net: { $names }
# Stands for a net nothing names.
panels-messages-unnamed-net = an unnamed net
# The end of a list cut after its first few entries.
panels-messages-list-more = { $names } and { $count } more

## Multi-channel sheets and the Compiled Design panel (#1279, #1308).

panels-kind-compiled-design = Compiled Design
# Tooltip of the panel's title-row button and the out-of-date banner's button.
panels-compiled-design-recompile = Recompile
# The panel's mode switch.
panels-compiled-design-mode-sheets = Sheets
panels-compiled-design-mode-nets = Nets
panels-compiled-design-find = Find sheet, channel or net
panels-compiled-design-column-sheet = Sheet / channel
panels-compiled-design-column-parts = Parts
panels-compiled-design-column-nets = Nets
panels-compiled-design-column-net = Net
panels-compiled-design-column-sheets = Sheets
# The out-of-date banner. $count is the number of changed sheets, $time the
# time of the last compile (12:41).
panels-compiled-design-stale =
    { $count ->
        [one] 1 sheet changed since { $time }. Counts may be wrong.
       *[other] { $count } sheets changed since { $time }. Counts may be wrong.
    }
# Tooltip of the amber dot on a changed row.
panels-compiled-design-stale-row = Changed since the last compile ({ $time })
panels-compiled-design-empty-title = Not compiled yet
panels-compiled-design-empty-body = Compile the project to see repeated sheets as channels, with part and net counts per sheet.
panels-compiled-design-compile = Compile project
panels-compiled-design-empty-note = Nothing is written to disk; the result lives in memory until the project closes.
panels-compiled-design-compiling = Compiling…
panels-compiled-design-no-project = Open a sheet of a project to compile it.
panels-compiled-design-no-match = No sheet, channel or net matches.
# The footer after a compile.
panels-compiled-design-footer = { $parts } parts · { $nets } nets · { $sheets } sheets ({ $channels } channels)
# The footer before the first compile. $files is the number of sheet files.
panels-compiled-design-footer-none = { $files } sheet files · not compiled
# The strip under a repeated sheet's canvas: the editable sheet, then one tab
# per channel.
panels-channel-strip-editor = Editor
# The selected channel tab's count; $count is its parts.
panels-channel-strip-parts = { $count } parts
# The count of a channel tab that is not selected.
panels-channel-strip-count = { $count }
# The caption of a channel card in the compiled view.
panels-channel-card-caption = { $parts } parts · { $nets } nets
# Shown in the compiled view before the project is compiled.
panels-channel-view-not-compiled = Compile the project to fill the channels with their parts and nets.
# A channel card's fit in the active variant (#2662). $variant is its name.
# A click on it leaves every part of the channel out of the variant, or lets
# the sheets decide again.
panels-channel-card-fitted = { $variant }: Fitted
panels-channel-card-not-fitted = { $variant }: Not fitted
# Some of the channel's parts are not fitted; $count is how many.
panels-channel-card-partly-fitted = { $variant }: { $count } not fitted
# The Messages line when an edit is tried in a channel tab. $channel is CH1.
panels-channel-read-only = Channel { $channel } is a compiled view; switch to Editor to edit
# The compile result in Messages.
panels-compiled-design-failed = Compile project: { $reason }

## Signal harnesses (#1280).

# Caption under the Harness Connector header: the sheet file and how many entries the connector has.
panels-harness-caption = { $sheet } · { $count ->
    [one] 1 entry
   *[other] { $count } entries
}
# Warn banner: the child sheet $far expects an entry this harness names differently.
panels-harness-mismatch = { $far } expects entry { $expects }; this harness has { $has }. One signal would not connect.
panels-harness-rename = Rename { $from } to { $to }
panels-harness-go-to = Go to { $sheet }
panels-harness-section = Harness
panels-harness-type = Type
panels-harness-definition = Definition
# A link that opens the harness definitions view.
panels-harness-definition-link = { $type } · { $count } entries · edit…
panels-harness-colour = Color
panels-harness-colour-default = harness default
panels-harness-placement = Placement
panels-harness-x = X
panels-harness-y = Y
panels-harness-width = Width
panels-harness-pitch = Entry pitch
panels-harness-entries = Entries
panels-harness-entry = Entry
panels-harness-net = Net on this sheet
panels-harness-expects-pill = { $far } expects { $expects }
# By a mismatched harness entry, in the Properties drawing and on the sheet: the entry name the far side expects in its place.
panels-harness-expects = expects { $expects }
# $example is a compiled net name such as SPI_BUS.SCK.
panels-harness-compiled = Compiled net names: { $example } … unless a net label names them.
panels-harness-start = Start
panels-harness-end = End
panels-harness-entry-type = Harness type
# The Harness type choice of a plain signal sheet entry.
panels-harness-no-type = None (signal)
panels-harness-defs-title = Harness definitions
panels-harness-defs-caption = project { $project } · { $count ->
    [one] 1 type
   *[other] { $count } types
}
# $entries is the entry count, nested types named, such as "2 + UART".
panels-harness-defs-uses = { $entries } entries · { $uses ->
    [one] 1 use
   *[other] { $uses } uses
}
# A type placed objects use that the project does not define (#3043); $entries as above, taken from its first connector.
panels-harness-defs-undefined-uses = { $entries } entries · { $uses ->
    [one] 1 use
   *[other] { $uses } uses
} · not defined
panels-harness-defs-name = Name
panels-harness-defs-description = Description
panels-harness-defs-kind = Kind
panels-harness-defs-signal = signal
panels-harness-defs-nested = harness · { $entries }
panels-harness-defs-nested-section = { $type } (nested)
panels-harness-defs-new = New harness type
panels-harness-defs-back = Back

## Properties of a Net Parameters directive (#1489): a flag on one wire that
## sets its net's class, diff-pair class and routing rules.

# Caption under the header: the net the flag sits on and the sheet file.
panels-np-caption = on { $net } · { $sheet }
# Caption when the flag sits on no wire.
panels-np-caption-no-net = on no wire · { $sheet }
panels-np-classes = Classes
panels-np-diff-pair-class = Diff-pair class
# A class dropdown entry: no class.
panels-np-none = None
# The diff-pair class dropdown while the net is not half of a pair.
panels-np-single-net = None (single net)
panels-np-rules = Rules
# Column headings of the rule list.
panels-np-rule = Rule
panels-np-value = Value
panels-np-width = Width
panels-np-clearance = Clearance
# Two inputs: the via diameter and the via hole.
panels-np-via = Via dia / hole
# The via on the flag and its illustration callout.
panels-np-via-short = Via
panels-np-current = Current
# Beside Current: the value is information only, no rule is made.
panels-np-info = info
panels-np-unit-mm = mm
# Unit of current: ampere.
panels-np-unit-a = A
panels-np-becomes-rules = Becomes PCB rules Width_{ $class } and Clearance_{ $class } on Update PCB.
panels-np-display = Display
panels-np-show-on-flag = Show parameters on the flag
# The first line of the flag on the sheet and in the illustration.
panels-np-flag-class = Net Class = { $class }
panels-np-flag-no-class = Net Parameters
panels-np-flag-hidden = parameters hidden on the flag
panels-np-current-info = current (info) { $current } A

## Properties of a Net Blanket (#1281).

# Caption under the header: the sheet file and the outline size.
panels-blanket-caption = { $sheet } · { $width } × { $height } mm
panels-blanket-outline = Outline
panels-blanket-xy = X / Y
panels-blanket-size = Width / Height
panels-blanket-directive = Directive
# The width of the tracks of the caught nets, not of the outline.
panels-blanket-track-width = Track width
# How copper pours treat the nets under this blanket.
panels-blanket-pour = Pour on nets
panels-blanket-pour-normal = Normal
panels-blanket-pour-keep-short = Keep short
# Section title with the number of nets the blanket catches.
panels-blanket-nets = Nets under the blanket · { $count }
# A net row whose wires only cross the outline, left out.
panels-blanket-edge-only = edge only
# A net row whose wires only cross the outline, included.
panels-blanket-edge-included = edge · included
panels-blanket-no-nets = No net is under the blanket.
panels-blanket-include-edge = Include nets that only cross the edge
panels-blanket-hover-hint = Hover a net row to highlight it on the sheet. Power nets (GND) are never caught.
# Note under the illustration.
panels-blanket-caught-note = accent = caught by the blanket
# Messages line: the Net Parameters tool was clicked away from a wire.
panels-np-off-wire = Net Parameters sit on a wire: click on a wire. Nothing was placed.

## Named unions (mockup row 158).

# The Unions dock panel and the Create Union popover (mockup row 158).
panels-kind-unions = Unions
panels-unions-empty-title = No schematic sheet in view
panels-unions-empty-body = Open a sheet to list its unions.
panels-unions-filter = Filter by name or #tag
panels-unions-create = Create from selection
panels-unions-this-sheet = This sheet
panels-unions-project = Project
panels-unions-tags = Tags
panels-unions-none = No unions here. Select objects and use Create Union to group them.
panels-unions-select-members = Select members
panels-unions-lock = Lock
panels-unions-unlock = Unlock
panels-unions-hide = Hide
panels-unions-show = Show
# The panel foot of a hidden union; $name is the union's name.
panels-unions-foot-hidden = { $name } is hidden
panels-unions-foot-hidden-note = Hidden on the canvas only. Show it to add, remove or rename members. Not an undo step.
panels-unions-foot-members = { $count ->
    [one] 1 member
   *[other] { $count } members
}
panels-unions-foot-wires = { $count ->
    [one] + 1 wire
   *[other] + { $count } wires
}
panels-unions-foot-others = { $count ->
    [one] + 1 other object
   *[other] + { $count } other objects
}
panels-unions-foot-add = Add selection
panels-unions-foot-remove = Remove selection
panels-unions-foot-break = Break union
# The popover title; $count is how many objects the union groups.
panels-unions-create-title = { $count ->
    [one] Create union · 1 object
   *[other] Create union · { $count } objects
}
panels-unions-create-name = Name
panels-unions-create-tags = Tags
panels-unions-create-add-tag = add tag
panels-unions-create-lock = Lock after creating
panels-unions-create-confirm = Create
# Removes a tag pill.
panels-unions-tag-remove = ×

## Messages lines of the unions, harness and compile slices.

panels-unions-msg-not-a-sheet = Create Union works on a schematic sheet.
panels-unions-msg-no-selection = Create Union: select the objects to group first.
panels-unions-msg-sheet-changed = Create Union closed: the active sheet changed.
# $reason is the engine's sentence saying why the union command was refused.
panels-unions-msg-refused = Unions: { $reason }.
panels-unions-msg-add-no-union = Add Selection to Union: pick a union in the Unions panel first.
panels-unions-msg-remove-no-members = Remove Selection from Union: select members of a union first.
panels-unions-msg-break-no-union = Break Union: select a union first.
panels-harness-msg-defs-no-project = Harness definitions not changed: the sheet is in no open project
# $why says why the project refused the edit.
panels-harness-msg-defs-refused = Harness definitions not changed: { $why }
# $file is the sheet, $why the engine's reason.
panels-harness-msg-rename-failed = Harness type not renamed on { $file }: { $why }
panels-harness-msg-entry-off-connector = Harness entry not placed: click on a harness connector
panels-compiled-design-msg-no-project = Compile project: open a sheet of a project first; a sheet outside a project has no compiled design.
# $why says why the project refused the edit.
panels-compiled-design-naming-refused = Channel naming not changed: { $why }
# A channel card's variant fit click was refused; $why says why.
panels-compiled-design-fit-refused = Channel fit not changed: { $why }
# The grey tag of a channel row the active variant leaves out entirely
# (mockup row 154); $variant is the variant's name.
panels-compiled-design-not-fitted = Not fitted ({ $variant })

# sch-panels-tidy
# The Canvas Font field (mockup row 40): $family is the font, $size its size in points.
panels-canvas-font-summary = { $family } · { $size } pt
# harness-connector-properties
panels-harness-add-entry = Add entry
# components-panel-cx
# Match summary under the Components search: $value is a displayed value
# such as 100 nF, $count the number of parts showing it.
panels-components-match-value = { $value } { $count }
# Match summary: $count more values than the line lists.
panels-components-match-more = +{ $count } more
# Match summary: $count parts the number matched by name or text only.
panels-components-match-by-name = name or text { $count }
# Match summary: the mark between two entries.
panels-components-match-separator = ·

# pcb-panel-group
# Empty state of the compact Length Matching dock panel (mockup row 107).
panels-length-matching-empty-title = No length-matching groups
panels-length-matching-empty-body = This board has no length-matching groups yet. Their members show here, with their lengths, as you route.

# pcb-nets-modes
## The PCB panel's Differential Pairs, xSignals, Polygons and Net Classes
## modes (mockup row 106).

panels-pcb-mode-diff-pairs = Differential Pairs
panels-pcb-mode-xsignals = xSignals
panels-pcb-mode-polygons = Polygons
# A mode dropdown entry: the mode's name and how many items it lists.
panels-pcb-mode-count = { $mode } · { $count }
# An unset rule: the board's own rule is in force.
panels-pcb-board-value = board
panels-pcb-pairs-empty = This board has no differential pairs. Name two nets with _P and _N (or + and -), or give a net class a pair gap and two nets.
# $skew is a length in mm, already formatted.
panels-pcb-pairs-skew = skew { $skew }
# A pair's half: its net and routed length in mm, already formatted.
panels-pcb-pairs-positive = P { $net } { $length }
panels-pcb-pairs-negative = N { $net } { $length }
# $gap and $width are lengths in mm or the word for the board's rule.
panels-pcb-pairs-rules = gap { $gap } · width { $width }
panels-pcb-pairs-summary =
    { $count ->
        [one] { $count } pair
       *[other] { $count } pairs
    }
panels-pcb-pairs-detail-positive = Positive
panels-pcb-pairs-detail-negative = Negative
panels-pcb-pairs-detail-gap-width = Gap / Width
panels-pcb-pairs-detail-routed = Routed
# Routed length of each half, in mm, already formatted.
panels-pcb-pairs-detail-routed-value = P { $positive } · N { $negative }
panels-pcb-pairs-detail-skew = Skew
# Two values of one detail row, as "0.175 / 0.265".
panels-pcb-pair-of-values = { $first } / { $second }
panels-pcb-xsignals-empty = This board has no xSignals yet.
panels-pcb-xsignals-summary =
    { $count ->
        [one] { $count } xSignal
       *[other] { $count } xSignals
    }
panels-pcb-pours-open-manager = Open Polygon Manager
panels-pcb-pours-manager-later = The Polygon Manager tab is not available yet.
# Chip over the pour list: the shelved pours.
panels-pcb-pours-chip-attention = Needs attention
panels-pcb-pours-poured = Poured
panels-pcb-pours-shelved = Shelved
panels-pcb-pours-no-net = No net
# A pour's layer and its pour priority, a whole number.
panels-pcb-pours-detail = { $layer } · priority { $priority }
panels-pcb-pours-empty = This board has no polygon pours.
panels-pcb-pours-no-match = No pours need attention.
panels-pcb-pours-summary =
    { $count ->
        [one] { $count } pour
       *[other] { $count } pours
    }
panels-pcb-classes-summary =
    { $count ->
        [one] { $count } class
       *[other] { $count } classes
    }
panels-pcb-classes-first-wins = The first matching class wins.
# A class's track width and clearance in mm, already formatted.
panels-pcb-classes-rule-summary = { $width } / { $clearance }
# The rules, then the via diameter and drill in mm, already formatted.
panels-pcb-classes-via-summary = { $rules } · via { $diameter }/{ $drill }
# An unset rule in a class's summary.
panels-pcb-classes-unset = —
panels-pcb-classes-nets =
    { $count ->
        [one] { $count } net
       *[other] { $count } nets
    }
# Member nets and patterns of a class; $nets is a whole number.
panels-pcb-classes-nets-patterns =
    { $nets } + { $patterns ->
        [one] { $patterns } pattern
       *[other] { $patterns } patterns
    }
panels-pcb-classes-rules = Rules · { $class }
panels-pcb-classes-width-clearance = Width / Clear.
panels-pcb-classes-via = Via
panels-pcb-classes-impedance = Impedance
panels-pcb-classes-impedance-tip = A net class stores no impedance yet.
panels-pcb-classes-members = Member nets
panels-pcb-classes-nodes =
    { $count ->
        [one] { $count } node
       *[other] { $count } nodes
    }
# A glob pattern's row: how many board nets it matches.
panels-pcb-classes-matches =
    { $count ->
        [one] { $count } match
       *[other] { $count } matches
    }
# The class detail's row of the diff-pair gap and width fields.
panels-pcb-classes-diff-pair = Diff pair
# quit-list-snxout-cap
# Kind code of an Export Job (.snxout) document, as on its tab and in the
# unsaved list of the Exit confirm.
panels-tab-kind-output-job = OUT
# The refusals of `set_dock_panel` (#2470), a command for agents and scripts.
panels-dock-command-unknown-panel = `panel` is one of { $panels }.
panels-dock-command-unknown-state = `state` is one of open, close, collapse, expand.
panels-dock-command-in-window = The { $panel } panel is in a window of its own; close that window.
panels-dock-command-not-docked = The { $panel } panel is not docked, so it has no region to { $state }.
