# Menu bar. The top-level titles come first; submenu rows follow as the menus
# are migrated.

# Title of the first menu of the menu bar (the document commands).
menu-file = File

## Graphite views (#2225)
menu-bar-search-placeholder = Search commands, parts, nets
menu-bar-light-theme = Light theme
menu-bar-dark-theme = Dark theme

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
# Greyed row shown in Open Recent when no project was opened yet.
menu-file-no-recent = No recent projects
menu-file-clear-recent = Clear Recent
menu-file-open-library = Open Library...
menu-file-place-component = Place Component...
menu-file-export-pdf = PDF…

## Edit menu (#2057)
# Submenu titles of the Edit menu.
menu-edit-move = Move
menu-edit-arrange = Arrange

## Place menu (#2057)
# Rows of the Place menu that are not commands.
menu-place-text = Text
menu-place-no-connect = No Connect

## Project menu (#2057)
# Rows of the Project menu that are not commands.
menu-project-close = Close Project

## Reports menu (#2057)
# Output rows of the Reports menu: the bill of materials, the netlist and the ERC report.
menu-reports-bom = Bill of Materials…
menu-reports-netlist = Netlist (Protel .NET)...
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
menu-help-sample-project = Open Sample Project

## Native app menu and menu chrome (#2057)
# The product name: a term, so a message names it as { -brand-name }.
# Fluent does not isolate a term reference, so it adds no bidi marks and
# a native menu label stays plain.
-brand-name = Signex
# Title of the app menu of the macOS menu bar (AppKit shows the process name regardless).
menu-app-title = { -brand-name }
# Tooltip of a menu row for a feature that is planned but not built yet.
menu-planned-tooltip = Planned — not available yet

## Context menus (#2057)
# Rows of the right-click menus. A row that runs a command shows the command's
# own catalog text, so only the rows with no command are here.
# Canvas menu: rows for features that are planned, not built yet.
ctx-clear-filter = Clear Filter
ctx-part-actions = Part Actions
ctx-sheet-actions = Sheet Actions
ctx-references = References
ctx-align = Align
ctx-unions = Unions
ctx-snippets = Snippets
ctx-comment = Comment...
ctx-pin-mapping = Pin Mapping...
ctx-supplier-links = Supplier Links...
# Document-tab menu. { $title } is the name of the document of the tab.
ctx-tab-close = Close { $title }
ctx-tab-close-others = Close All Other Documents
ctx-tab-open-in-new-window = Open In New Window
# Panel-tab menu: where the panel goes.
ctx-dock-close = Close
ctx-dock-float = Undock / Float
ctx-dock-left = Dock Left
ctx-dock-right = Dock Right
ctx-dock-bottom = Dock Bottom
# Menu of a component row of the Components panel (the verb "place").
ctx-components-row-place = Place
ctx-components-row-edit = Edit Component
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
ctx-add-output-job = Output Job
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
ctx-tree-variants = Variants...
ctx-tree-packager = Project Packager...
ctx-tree-releaser = Project Releaser...
ctx-tree-share = Share...
ctx-tree-enable-version-control = Enable Version Control...
ctx-tree-add-new-component = Add New ▸ Component
ctx-tree-add-new-symbol-library = Add New ▸ Symbol Library
ctx-tree-add-new-footprint-library = Add New ▸ Footprint Library
ctx-tree-collapse = Collapse
ctx-tree-expand = Expand
ctx-tree-refresh = Refresh
ctx-tree-remove-library = Remove from Project...
ctx-tree-open = Open
ctx-tree-remove = Remove from Project
ctx-tree-show-differences = Show Differences...
ctx-tree-expand-all = Expand All
ctx-tree-collapse-all = Collapse All
# Shapes, in the symbol and footprint editor menus.
ctx-shape-rectangle = Rectangle
ctx-shape-circle = Circle
ctx-shape-arc = Arc
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
