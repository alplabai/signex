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

# Hint beside the empty state: the environment variables that turn on
# verbose logging. Keep the variable names as they are.
panels-messages-verbose-hint = Set RUST_LOG=debug or SIGNEX_LOG=debug for verbose output

# A cell with no value (messages carry no document or time yet).
panels-messages-no-value = —

# Level pills of a message row.
panels-messages-level-error = ERROR
panels-messages-level-warning = WARN
panels-messages-level-info = INFO
panels-messages-level-debug = DEBUG
panels-messages-level-trace = TRACE

## Components panel

# Placeholder of the components filter field.
panels-components-filter = Filter components…

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

# Empty list: nothing is mounted.
panels-components-nothing-mounted = No libraries mounted. Use "+ Add" or open a project.
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
# Subsection label of Page Options: paper size and format.
panels-properties-formatting = Formatting and Size
# Subsection label of Page Options: the sheet border and its zone grid.
panels-properties-border-zones = Border and Zones
# Subsection label of Page Options: the document font.
panels-properties-text = Text
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
# The Signex AI assistant; Signex is the product name.
panels-kind-signex-ai = Signex AI
# Design rule check.
panels-kind-drc = DRC
panels-kind-layer-stack = Layer Stack
panels-kind-net-classes = Net Classes
panels-kind-variants = Variants
# Schematic filter.
panels-kind-sch-filter = SCH Filter
# Schematic object list.
panels-kind-sch-list = SCH List
# Bill of materials studio.
panels-kind-bom-studio = BOM Studio
panels-kind-favorites = Favorites
panels-kind-snippets = Snippets
panels-kind-todo = To-Do
panels-kind-wiki = Wiki
panels-kind-output-jobs = Output Jobs
panels-kind-library = Library
# Schematic symbol library.
panels-kind-sch-library = SCH Library
panels-kind-footprint-library = Footprint Library
# The footprint editor's layer list.
panels-kind-footprint-layers = Layers
# The symbol editor's pin table.
panels-kind-symbol-pins = Pins
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

## Empty-state cards and the AI Design shell

# Selection Filter panel: its title while the panel has no backend.
panels-selection-filter-title = Selection Filter
# Selection Filter panel: the line under the title.
panels-selection-filter-body = All object types enabled

# Tooltip of every Signex AI and Start screen control that has no backend
# yet. "AI Design" is the name of that coming feature.
panels-coming-soon = Coming in AI Design

# Signex AI panel, Assistant mode: empty-state title and line.
panels-signal-assistant-title = Signex AI is not available yet
panels-signal-assistant-body = Ask about this sheet and review proposed changes before anything is applied.
# Signex AI panel, Generator mode: empty-state title and line.
panels-signal-generator-title = The Generator is not available yet
panels-signal-generator-body = Describe a board and approve each stage before anything is drawn.
# Placeholder of the (disabled) ask field.
panels-signal-ask = Ask about this sheet, or describe a change
# Send button glyph of the ask field.
panels-signal-send = →

## Start screen. "Signal" is the name of the AI design assistant.

start-new-project = New project
# Button that opens a project; the ellipsis says a file dialog follows.
start-open = Open…
start-home = Home
start-projects = Projects
start-libraries = Libraries
# A planned part search; the row has no backend yet.
start-parts-search = Parts search
start-open-sample = Open Sample Project
# F1 is the key that opens the shortcut list.
start-keyboard-shortcuts = Keyboard Shortcuts (F1)
# Count at the right of a navigation row. $count is a whole number.
start-count = { $count }
# Loaded libraries card: how many libraries are loaded and how many
# components they hold. Both are whole numbers.
start-libraries-summary =
    { $libraries } loaded · { $components ->
        [one] { $components } component
       *[other] { $components } components
    }
start-hero-title = What are you building?
start-hero-body = Describe the board. Signal drafts the specification, architecture and parts, and you approve each stage before anything is drawn.
# Placeholder of the (disabled) specification box.
start-spec-placeholder = Describe the board: what it does, its size, layers and budget.
# Specification box buttons (disabled until the AI backend exists).
start-spec-attach = Attach
start-spec-import = Import
start-spec-constraints = Constraints
start-spec-generate = Generate design
# Specification box mode switch.
start-spec-generator = Generator
start-spec-assistant = Assistant
start-spec-libraries-first = Signal uses your loaded libraries first
# Section headings, shown in capitals.
start-reference-design = START FROM A REFERENCE DESIGN
start-recent-projects = RECENT PROJECTS
# The bundled sample project card.
start-sample-title = Sample project
start-sample-body = Open the bundled example project and look around.
# Recent projects list when it is empty.
start-no-recent-title = No recent projects
start-no-recent-body = Projects you open appear here.

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
# Shown when the filter matches no file.
panels-projects-no-match = No files match

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

## Projects panel footer summary

# Caption: electrical rule check results.
panels-summary-erc = ERC
# Caption: design rule check results.
panels-summary-drc = DRC
# Caption: how long ago the last git commit was made.
panels-summary-last-commit = Last commit
# A value that is not known yet.
panels-summary-no-value = —
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
panels-messages-source-graphics = Graphics
panels-messages-source-interface = Interface
panels-messages-source-system = System

## Graphite views (#2225)
panels-status-bar-grid = Grid { $grid }
panels-status-bar-grid-off = Grid OFF
panels-status-bar-snap-on = Snap on
panels-status-bar-snap-off = Snap off
panels-status-bar-tool = Tool: { $tool }
panels-status-bar-importing = Importing { $name }… ✕
panels-erc-run = Run ERC (F8)
panels-erc-clear = Clear
panels-erc-running = Running…
panels-erc-stale = Results may be out of date: the sheet changed since the last run. Run ERC again.
panels-erc-no-findings-left = No findings left since the last run
panels-erc-no-diagnostics = No ERC diagnostics yet
panels-erc-empty-hint = Run ERC to populate project-wide diagnostics
panels-erc-results-title = ERC Diagnostic Results
panels-erc-add-no-connect = Add No-Connect
panels-erc-show-on-canvas = Show on Canvas
panels-erc-waive-net = Waive net
panels-erc-restore = Restore
panels-properties-needs-schematic = Properties are available when a schematic is active
panels-properties-open-project = Open a project
panels-properties-nothing-selected = Nothing selected
panels-properties-one-object-selected = 1 object selected
panels-properties-no-placement-options = Click to place. No per-instance options.
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
panels-child-sheet-name = Name
panels-child-sheet-file = File
panels-child-sheet-line-width = Line Width (mm)
