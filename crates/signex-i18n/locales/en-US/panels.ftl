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
# The two tabs (#1274): the project's nets, or its parts with the net on each pin.
panels-net-inspector-tab-nets = By Net
panels-net-inspector-tab-components = By Component
# Placeholder of the box that filters the parts by designator, value or footprint.
panels-net-inspector-filter-parts = Filter parts
# Shown when no part matches the filter.
panels-net-inspector-empty-parts = No part matches the filter.
# The part's designator.
panels-net-inspector-column-part = PART
panels-net-inspector-column-value = VALUE
panels-net-inspector-column-footprint = FOOTPRINT
# A part's designator, value or footprint cell when it is blank.
panels-net-inspector-blank = —
# Footer of the By Component tab: how many parts the filter shows of the
# project's total. Whole numbers.
panels-net-inspector-parts-summary =
    { $shown } of { $total ->
        [one] { $total } part
       *[other] { $total } parts
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
# The cursor position. $value is the coordinate as a number; $unit is the
# unit symbol (mm, mil, in, µm).
panels-status-bar-x = X { $value } { $unit }
panels-status-bar-y = Y { $value } { $unit }
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
# A drag in progress that leaves the wires. $key is the modifier key.
panels-status-bar-move-without-wires = Moving without wires ({ $key }: drag with wires)
# Measure tool prompts.
panels-status-bar-measure-first = Click the first point
panels-status-bar-measure-second = Click the second point
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
panels-net-classes-color = Colour
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
panels-erc-count-warnings = { $count } warnings
panels-erc-count-info = { $count } info
panels-erc-waived = { $count } waived
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
panels-net-classes-column-color = Colour
panels-net-classes-column-width = Width
panels-net-classes-column-members = Members
# Colour cell of a class with no colour of its own: the theme's wire colour.
panels-net-classes-theme-color = Theme
# Width cell of a class with a width; $width is in millimetres.
panels-net-classes-width-cell = { $width } mm
# Width cell of a class with no width: each wire keeps its own.
panels-net-classes-wire-width = Wire's own
panels-net-classes-no-members = None
# SCH Filter panel (#2417), over the chip grid.
panels-sch-filter-hint = Kinds a click, a box and the hover pick can select. Presets are edited in Properties.

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
# Properties section of a selected Blanket directive: a drawn region whose
# parameters apply to every net inside it.
panels-directive-blanket = Blanket
panels-directive-net-class = Net Class
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
# Stands for a net nothing names.
panels-messages-unnamed-net = an unnamed net
# The end of a list cut after its first few entries.
panels-messages-list-more = { $names } and { $count } more
