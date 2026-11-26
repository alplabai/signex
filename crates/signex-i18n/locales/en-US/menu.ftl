# Menu bar. The top-level titles come first; submenu rows follow as the menus
# are migrated.

# Title of the first menu of the menu bar (the document commands).
menu-file = File

## Graphite views (#2225)
menu-bar-search-placeholder = Search commands, parts, nets, sheets
menu-bar-light-theme = Light theme
menu-bar-dark-theme = Dark theme
# Tooltip of the search button a narrow window shows instead of the search
# field: it opens the command palette. The second form names its shortcut;
# $key is the key chord as the keymap writes it, for example Ctrl+K.
menu-bar-search-button = Search
menu-bar-search-button-key = Search ({ $key })

## Menu bar root titles (#2057)
# Titles of the other nine menus of the menu bar, in bar order after File.
menu-edit = Edit
menu-view = View
menu-project = Project
menu-place = Place
menu-design = Design
menu-tools = Tools
menu-reports = Reports
menu-window = Window
# View ▸ Window ▸ Layouts (mockup row 166): the submenu, its two caption rows, and the
# Reset row of an edited built-in; { $name } is the built-in's name.
menu-window-layouts = Layouts
menu-window-layouts-builtin = BUILT-IN
menu-window-layouts-yours = YOUR LAYOUTS
menu-window-layouts-reset = Reset “{ $name }”
menu-help = Help

## File menu (#2057)
# Submenu titles of the File menu.
menu-file-new = New
menu-file-open-recent = Open Recent
menu-file-library = Library
menu-file-import = Import
menu-file-export = Export
# Rows of the File menu that are not commands.
menu-file-save-all = Save All
menu-file-import-altium = Altium Project...
menu-file-exit = Exit
menu-file-clear-recent = Clear Recent
menu-file-open-library = Open Library...
menu-file-place-component = Place Component...
menu-file-export-pdf = PDF…
# File ▸ Export row of the Bill of Materials export dialog (mockup row 112).
menu-file-export-bom = Export BOM…

## Edit menu (#2057)
# Submenu titles of the Edit menu.
menu-edit-move = Move
menu-edit-arrange = Arrange
# Edit ▸ Origin: where the board counts its coordinates from.
menu-edit-origin = Origin

## Place menu (#2057)
# Rows of the Place menu that are not commands.
menu-place-text = Text
menu-place-no-connect = No Connect
# Submenu titles of the Place menu on a sheet (#2415), grouped like the
# Active Bar: ports, sheet symbols and harnesses; directives; drawing tools.
menu-place-ports = Ports & Connectors
menu-place-directives = Directives
menu-place-graphics = Graphics
# Design ▸ Rooms on a board (#2457).
menu-design-rooms = Rooms

## Project menu (#2057)
# Rows of the Project menu that are not commands.
menu-project-close = Close Project
# The Design Notebook submenu (mockup rows 195 and 201).
menu-project-design-notebook = Design Notebook

## Tools ▸ Reports (#2057)
# Output rows of the Tools ▸ Reports submenu: the bill of materials, the netlist and the ERC report.
menu-reports-bom = Bill of Materials…
menu-reports-netlist = Netlist (.NET)...
menu-reports-erc = ERC Report…

## Tools menu (#2057)
# Submenu title of the Tools menu, over the annotation rows.
menu-tools-annotation = Annotation
# Annotation rows that are not commands.
menu-tools-annotate = Annotate Schematics...
menu-tools-reset-designators = Reset Schematic Designators...
menu-tools-reset-duplicate-designators = Reset Duplicate Schematic Designators...
# Rows of the Tools menu that are not commands.
menu-tools-up-hierarchy = Up Hierarchy
menu-tools-new-part = New Part
menu-tools-remove-part = Remove Part
menu-tools-document-options = Document Options...
menu-tools-passive-calculator = Passive Network Calculator...

## Help menu (#2057)
# Rows of the Help menu that are not commands.
menu-help-about = About { -brand-name }
menu-help-documentation = Documentation
menu-help-report-issue = Report an Issue
# chrome-copy-help
# Help menu row (mockup row 14): opens the first-run card again.
menu-help-welcome-tips = Show Welcome Tips

## Native app menu and menu chrome (#2057)
# The product name: a term, so a message names it as { -brand-name }.
# Fluent does not isolate a term reference, so it adds no bidi marks and
# a native menu label stays plain.
-brand-name = Signex
# Title of the app menu of the macOS menu bar (AppKit shows the process name regardless).
menu-app-title = { -brand-name }
# Tooltip of a menu row for a feature that is planned but not built yet.
menu-planned-tooltip = Planned — not available yet
# Pill on a menu row for a feature that is planned but not built yet.
menu-planned-badge = planned

## Context menus (#2057)
# Rows of the right-click menus. A row that runs a command shows the command's
# own catalog text, so only the rows with no command are here.
# Canvas menu: submenu launchers, and rows for features that are planned, not built yet.
ctx-part-actions = Part Actions
ctx-sheet-actions = Sheet Actions
ctx-references = References
ctx-align = Align
# Board menu: runs `compare_board_with_schematic` under a shorter name (#2462).
ctx-compare-with-schematic = Compare with Schematic
ctx-unions = Unions
ctx-snippets = Snippets
ctx-comment = Comment...
ctx-pin-mapping = Pin Mapping...
ctx-supplier-links = Supplier Links...
# Tooltips of those planned rows: what each will do, and that it is not built yet.
ctx-references-planned = Planned: list where the selected objects are used across the project
ctx-unions-planned = Planned: group the selected objects so they move and select as one
ctx-pin-mapping-planned = Planned: map the selected part's symbol pins to its footprint pads
ctx-supplier-links-planned = Planned: link the selected part to supplier part numbers and prices
# Tooltip of the greyed Assign Net Class row: where a net is given a class today.
ctx-assign-net-class-elsewhere = Pick the class in the Net Class row of the Properties panel
# Document-tab menu. { $title } is the name of the document of the tab.
ctx-tab-close = Close { $title }
ctx-tab-close-others = Close All Other Documents
ctx-tab-open-in-new-window = Open In New Window
ctx-tab-close-right = Close Tabs to the Right
ctx-tab-copy-path = Copy Path
ctx-tab-reveal-in-projects = Reveal in Projects
# Tab strip overflow: the button at the strip's end ({ $count } is how many tabs
# do not fit) and the caption of the menu that lists them.
ctx-tab-overflow-button = +{ $count }
ctx-tab-overflow-caption = Hidden tabs
# Panel-tab menu: where the panel goes.
ctx-dock-close = Close
ctx-dock-float = Undock / Float
ctx-dock-left = Dock Left
ctx-dock-right = Dock Right
ctx-dock-bottom = Dock Bottom
# Button and tooltip in the title strip of a panel's own window: the panel goes
# back into the dock.
ctx-dock-back = Dock back
# Menu of a component row of the Components panel (the verb "place").
ctx-components-row-place = Place
ctx-components-row-edit = Edit Component
ctx-components-row-copy-part-number = Copy Part Number
# Grid sizes of the footprint editor's grid picker. The unit is part of the text.
ctx-grid-1-mil = 1 Mil
ctx-grid-5-mil = 5 Mil
ctx-grid-10-mil = 10 Mil
ctx-grid-20-mil = 20 Mil
ctx-grid-25-mil = 25 Mil
ctx-grid-50-mil = 50 Mil
ctx-grid-100-mil = 100 Mil
ctx-grid-0-025-mm = 0.025 mm
ctx-grid-0-100-mm = 0.100 mm
ctx-grid-0-250-mm = 0.250 mm
ctx-grid-0-500-mm = 0.500 mm
ctx-grid-1-000-mm = 1.000 mm
ctx-grid-2-500-mm = 2.500 mm
# Power ports of the canvas Place submenu (they have no command of their own).
ctx-place-power-gnd = GND Power Port
ctx-place-power-vcc = VCC Power Port
ctx-place-power-plus5 = +5 Power Port
ctx-place-power-plus12 = +12 Power Port
# Add New to Project submenu of the project tree: the kind of document to create.
ctx-add-schematic = Schematic
ctx-add-component-library = Component Library
ctx-add-symbol-library = Symbol Library
ctx-add-pcb = PCB
ctx-add-pcb-library = PCB Library
ctx-add-output-job = Export Job
ctx-add-design-notebook = Design Notebook
ctx-add-constraint-file = Constraint File
ctx-add-vhdl-file = VHDL File
# Project tree menu.
ctx-tree-refresh-pricing = Refresh All Pricing
ctx-tree-no-pricing-client = No enabled distributor returns prices yet
ctx-tree-library-not-found = Library not found
ctx-tree-make-available-online = Make Project Available Online...
ctx-tree-validate-project = Validate Project
ctx-tree-add-new = Add New to Project
ctx-tree-add-existing = Add Existing to Project...
ctx-tree-rename = Rename...
ctx-tree-close-documents = Close Project Documents
ctx-tree-explore = Explore
ctx-tree-variants = Variant Manager
ctx-tree-packager = Project Packager...
ctx-tree-releaser = Project Releaser...
ctx-tree-share = Share...
ctx-tree-enable-version-control = Enable Version Control...
ctx-tree-collapse = Collapse
ctx-tree-expand = Expand
ctx-tree-refresh = Refresh
ctx-tree-remove-library = Remove from Project...
ctx-tree-open = Open
ctx-tree-remove = Remove from Project...
ctx-tree-show-differences = Show Differences...
ctx-tree-expand-all = Expand All
ctx-tree-collapse-all = Collapse All
# The project root's launcher of the planned-feature rows.
ctx-tree-planned = Planned
# The library node's Add New submenu: the kind of item to create.
ctx-tree-library-add-new = Add New
ctx-tree-library-add-component = Component
ctx-tree-library-add-symbol-library = Symbol Library
ctx-tree-library-add-footprint-library = Footprint Library
# Shapes, in the symbol and footprint editor menus.
ctx-shape-rectangle = Rectangle
ctx-shape-circle = Circle
ctx-shape-arc = Arc
ctx-shape-elliptical-arc = Elliptical Arc
ctx-shape-bezier = Bezier
ctx-shape-round-rectangle = Round Rectangle
ctx-shape-pie-chart = Pie Chart
ctx-deselect-all = Deselect All
ctx-fit-to-window = Fit to Window
# Symbol editor menu.
ctx-symbol-join-polygon = Join into Polygon
ctx-symbol-pin = Pin
ctx-symbol-line = Line
ctx-symbol-text = Text
ctx-symbol-polygon = Polygon
# Footprint editor menu.
ctx-fp-pad = Pad
ctx-fp-track = Track
ctx-fp-polygon-region = Polygon (Region)
ctx-fp-string-text = String (Text)
ctx-fp-selection = Selection
# Header of the menu on a pad. { $number } is the pad number.
ctx-fp-pad-header = Pad { $number }
ctx-fp-pad-actions = Pad Actions
ctx-fp-rotate-90 = Rotate 90°
ctx-fp-flip-layer = Flip Layer
ctx-fp-custom-pad = Custom Pad from Outline...
ctx-fp-thermal-points = Thermal Connection Points...
# Header of the menu on a silkscreen graphic. { $kind } is one of the kinds below.
ctx-fp-silk-header = { $kind } (silk)
ctx-fp-silk-unknown = Silk graphic { $index }
ctx-fp-silk-string = String
ctx-fp-silk-region = Region

# Place > Graphics > Dimension, and the Drawing Tools group header of the same name;
# on a board, Place > Dimension and its heading in the Place flyout (#2507).
menu-place-dimension = Dimension

## Active Bar Power menu and the Place flyout's power rows (Graphite
## mockup #61, Graphite mockup #62): the short port names; the full
## "Place … power port" text is the tooltip.

ctx-power-common = Common
ctx-power-styles = Styles
ctx-power-grounds = Grounds
ctx-power-gnd = GND
ctx-power-vcc = VCC
ctx-power-plus12 = +12
ctx-power-plus5 = +5
ctx-power-minus5 = -5
ctx-power-arrow = Arrow
ctx-power-wave = Wave
ctx-power-bar = Bar
ctx-power-circle = Circle
ctx-power-signal-ground = Signal Ground
ctx-power-earth = Earth
ctx-power-chassis = Chassis

## Sheet right-click menu (Graphite mockup #62)

# Muted first line with nothing selected; { $file } is the sheet's file name.
ctx-sheet-context = schematic sheet · { $file }
ctx-selection-count = { $count ->
    [one] 1 item selected
   *[other] { $count } items selected
}
# Muted first line with one part selected; { $selected } is ctx-selection-count.
ctx-selection-part = { $reference } · { $value } · { $selected }
# Greyed flyout gathering the features not built yet.
ctx-planned-group = Planned ({ $count })
ctx-place-group-wiring = Wiring
ctx-place-group-ports-power = Ports and power
ctx-place-group-parts-sheets = Parts and sheets
ctx-place-group-directives-text = Directives and text
# Greyed row of the Place flyout standing for the planned tools.
ctx-place-planned-tools = { $count ->
    [one] { $count } planned tool…
   *[other] { $count } planned tools…
}
# { $names } lists the planned tools.
ctx-place-planned-tools-tooltip = Planned: { $names }

## Grid picker (Graphite mockup #63)

ctx-grid-imperial = Imperial
ctx-grid-metric = Metric
# A grid step in millimetres; { $mm } keeps '.' in every language.
ctx-grid-mm-caption = { $mm } mm
ctx-grid-properties = Grid Properties…
ctx-grid-preview = Grid preview
# { $step } is the ladder name of the step the preview shows.
ctx-grid-preview-active = { $step } · active
ctx-grid-preview-on-pitch = Pins on the 2.54 mm pitch land on this grid
ctx-grid-preview-off-pitch = Pins on the 2.54 mm pitch fall between grid points
# The canvas zoom pill (Graphite mockup #64). { $percent } is the zoom as a whole number.
zoom-pill-percent = { $percent }%
zoom-pill-fit = Fit
# Hover hint of a pill control: { $action } is the command's menu text,
# { $shortcut } its key in the active shortcut profile.
zoom-pill-hint = { $action } · { $shortcut }
zoom-pill-presets = Zoom Presets
# Rows of the menu the pill's percentage opens. { $percent } as above.
zoom-presets-percent = { $percent }%
zoom-presets-fit-selection = Fit Selection
# The orientation tile above the zoom pill (mockup row 194): its axis labels,
# then the caption row of a turned or flipped view. { $angle } is the view
# rotation in degrees, '.' as the decimal mark.
orientation-tile-x = +X
orientation-tile-y = +Y
orientation-tile-angle = { $angle }°
orientation-tile-bottom = Bottom
orientation-tile-bottom-angle = Bottom · { $angle }°
# Hover hint of the Home button left of the zoom pill, on a board and on a sheet.
home-view-hint-board = Home View · rotation 0°, Flip off, fit
home-view-hint-sheet = Home View · rotation 0°, fit
# Footprint editor menu on a pad: rows acting on the selection.
ctx-fp-move-by = Move Selection By…
ctx-fp-rotate-selection = Rotate Selection
ctx-fp-flip-selection = Flip Selection
ctx-fp-rect-pattern = Rectangular Pattern…
ctx-fp-edit-sketch = Edit Sketch
# Rows of the Align submenu on a pad. The first opens the Align Selection dialog.
ctx-fp-align-dialog = Align…
ctx-fp-align-left = Align Left
ctx-fp-align-right = Align Right
ctx-fp-align-hcenter = Align Horizontal Centers
ctx-fp-distribute-h = Distribute Horizontally
ctx-fp-align-top = Align Top
ctx-fp-align-bottom = Align Bottom
ctx-fp-align-vcenter = Align Vertical Centers
ctx-fp-distribute-v = Distribute Vertically
ctx-fp-align-to-grid = Align To Grid
# Symbol editor menu: rows acting on the selection.
ctx-symbol-rotate-selection = Rotate Selection
ctx-symbol-align-to-grid = Align To Grid
# in-variant-context-row
# Sheet menu row after Part Actions (mockup row 62); { $variant } is the
# project's active variant.
ctx-in-variant = In variant: { $variant }
# Header of the In variant flyout; { $part } is the selected part's designator.
ctx-in-variant-part = { $part } in { $variant }
# The picker and the popover of mockup rows 208 and 209.
ctx-in-variant-alternate-part = Alternate…
ctx-in-variant-edit-value = Edit value for this variant…
# Tooltip of the In variant rows that are not built yet: the fit on a sheet
# that is not repeated.
ctx-in-variant-planned = Planned: change a part's fit in one variant on a sheet that is not repeated. The Variant Manager sets it today.
ctx-in-variant-show-manager = Show in Variant Manager
ctx-in-variant-new = New variant…
# The In variant fit rows on a repeated sheet (#2662, mockup row 32). $fit
# is Fitted or Not fitted, $channel the channel shown (CH2).
ctx-in-variant-this-channel = { $fit } · This channel only ({ $channel })
ctx-in-variant-all-channels = { $fit } · All channels
# Tooltip of those rows before the project is compiled.
ctx-in-variant-compile-first = Compile the project to set this part's fit per channel. The Variant Manager sets it today.
# Tooltip of those rows while Base is the active variant.
ctx-in-variant-in-base = Base fits every part as the sheets say. Switch to a variant first.
# Tooltips of Alternate… and Edit value for this variant… when they cannot
# open: in Base, without one part selected, before a compile on a repeated
# sheet.
ctx-in-variant-edit-in-base = Base keeps each part as placed. Switch to a variant first.
ctx-in-variant-one-part = Select one part to change it in this variant.
ctx-in-variant-edit-compile-first = Compile the project to change this part per channel. The Variant Manager changes it today.
# project-tree-history-rows
# A document leaf's row that opens the History panel filtered to the file.
ctx-tree-show-history = Show History
