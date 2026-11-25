# Properties panel of a board with nothing selected (#2353).

# Placeholder of the field that filters the panel's rows.
pcb-board-search = Search
pcb-board-search-empty = Nothing matches the search.
# Tabs under the title.
pcb-board-tab-general = General
pcb-board-tab-parameters = Parameters
pcb-board-tab-health = Health Check
pcb-board-parameters-unavailable = Boards hold no parameters yet.
# Badge on the Health Check tab: how many design and electrical rule findings there are.
pcb-board-health-badge = { $count }

# The header's Selection Filter button: kinds on out of all, and its tooltip
# naming the kinds turned off ($off, a comma list).
pcb-board-filter-count = { $on }/{ $total }
pcb-board-filter-tip-all = Selection filter: everything is pickable
pcb-board-filter-tip-off = Selection filter: { $off } off

# Selection Filter: which objects a click or a box picks on the board.
pcb-board-section-filter = Selection Filter
pcb-board-filter-all-on = All - On
pcb-board-filter-unavailable = The board editor cannot pick these objects yet.
pcb-board-filter-presets = Presets
pcb-board-filter-preset-add = Add Preset
pcb-board-filter-preset-add-tip = Save the kinds that are on as a preset.
pcb-board-filter-preset-full = At most { $limit } presets; delete one to add another.
pcb-board-filter-preset-apply-tip = Turn on exactly the kinds this preset holds and edit it with the chips; click again to stop editing.
pcb-board-filter-preset-name = Preset name
pcb-board-kind-components = Components
pcb-board-kind-bodies = 3D Bodies
pcb-board-kind-keepouts = Keepouts
pcb-board-kind-tracks = Tracks
pcb-board-kind-arcs = Arcs
pcb-board-kind-pads = Pads
pcb-board-kind-vias = Vias
pcb-board-kind-regions = Regions
pcb-board-kind-polygons = Polygons
pcb-board-kind-fills = Fills
pcb-board-kind-texts = Texts
pcb-board-kind-rooms = Rooms
pcb-board-kind-other = Other

# Snap Options: what a carried footprint snaps to (#2353, #2374).
pcb-board-section-snap = Snap Options
# The row of the Grids, Guides and Axes checkboxes.
pcb-board-snap-to = Snap to
pcb-board-snap-grids = Grids
pcb-board-snap-guides = Guides
pcb-board-snap-axes = Axes
pcb-board-snapping = Snapping
pcb-board-snapping-all = All Layers
pcb-board-snapping-current = Current Layer
pcb-board-snapping-off = Off
pcb-board-snap-objects = Objects for snapping
pcb-board-snap-track-vertices = Track/Arc Vertices
pcb-board-snap-track-lines = Track/Arc Lines
pcb-board-snap-arc-centers = Arc Centers
pcb-board-snap-intersections = Intersections
pcb-board-snap-pad-centers = Pad Centers
pcb-board-snap-pad-vertices = Pad Vertices
pcb-board-snap-pad-edges = Pad Edges
pcb-board-snap-via-centers = Via Centers
pcb-board-snap-regions = Regions/Polygons/Fills
pcb-board-snap-board-shape = Board Shape
pcb-board-snap-footprint-origins = Footprint Origins
pcb-board-snap-body-points = 3D Body Snap Points
pcb-board-snap-texts = Texts
pcb-board-snap-object-unavailable = Boards hold no 3D bodies yet.
pcb-board-snap-distance = Distance
pcb-board-axis-range = Axis Range

# The board illustration's size row (#2523): the size comes from the board
# shape, read-only here. $width and $height are numbers from Signex's own
# formatter, $unit their unit.
pcb-board-size-readout = Size { $width } × { $height } { $unit }
# The same for an outline that is not a rectangle: its extent.
pcb-board-size-bounding = Bounding box { $width } × { $height } { $unit }
pcb-board-size-source = · from board shape
pcb-board-edit-shape = Edit board shape…
pcb-board-edit-shape-tip = Board size follows the board outline. Edit the shape in Board sketch.
# Callouts on the illustration; $value is a length with its unit.
pcb-board-callout-width = { $value } · from board shape
pcb-board-callout-snap = Snap { $value }

# Board Information: totals read from the board, as stat tiles.
pcb-board-section-info = Board Information
# Tooltip of the Width and Height tiles.
pcb-board-info-from-shape = From the board shape
pcb-board-info-width = Width
pcb-board-info-height = Height
pcb-board-info-thickness = Thickness
pcb-board-info-no-outline = No board outline
pcb-board-info-area = Area
pcb-board-info-components-area = Parts Area
pcb-board-info-density = Density
pcb-board-info-components = Components
pcb-board-info-top = Top
pcb-board-info-bottom = Bottom
pcb-board-info-select-tip = Select these components
pcb-board-info-layers = Layers
pcb-board-info-signal = Signal
pcb-board-info-nets = Nets
pcb-board-info-unrouted-nets = Unrouted nets
# Tooltip of the Unrouted nets tile above zero.
pcb-board-info-unrouted-tip = { $count ->
    [one] 1 net has connections that are not routed yet
   *[other] { $count } nets have connections that are not routed yet
}
# Nets whose ratsnest lines are hidden.
pcb-board-info-hidden-nets = Hidden nets
# Shown as the unrouted count before the ratsnest is worked out.
pcb-board-info-not-counted = Not counted yet
# The closed fold under the tiles.
pcb-board-info-more = Layers · Primitives & Others
pcb-board-info-fills = Fills
pcb-board-info-regions = Regions
pcb-board-info-keepouts = Keepouts
pcb-board-info-pads = Pads
pcb-board-info-strings = Strings
pcb-board-info-tracks = Tracks
pcb-board-info-vias = Vias
pcb-board-info-polygons = Polygons
pcb-board-info-holes = Holes
pcb-board-info-drc = DRC Violations
pcb-board-info-drc-not-run = Not run
pcb-board-info-drc-tip = Show the design rule check results
pcb-board-reports = Reports
# A count in the Board Information rows.
pcb-board-count = { $count }
# A length or area from Signex's own formatter, unit included.
pcb-board-measure = { $value }
# Shown in a table cell the board cannot fill yet.
pcb-board-unset = —
pcb-board-percent = { $value } %

# Grid Manager: the board's snap grid.
pcb-board-section-grids = Grid Manager
pcb-board-grid-name = Name
pcb-board-grid-global = Global Snap Grid
pcb-board-grid-step = Step
pcb-board-grid-visible = Visible Grid
pcb-board-grid-fast = Fast Grids
pcb-board-grid-no-setup = This board has no setup to hold a grid step.
pcb-board-grid-invalid = Type a length above zero.
# $why says why the grid step was not set.
pcb-board-grid-refused = Grid step not set: { $why }
# The undo step's name.
pcb-board-grid-undo = Grid step
pcb-board-delete = Delete
# A cell of the Global Board Snap Grid row: it applies to everything.
# A point in board coordinates; $x and $y come from Signex's own formatter.
pcb-board-point = { $x }, { $y }
pcb-board-swatch-tip = Click for the next color
pcb-board-grid-origin-x = Origin X
pcb-board-grid-origin-y = Origin Y
# The name a new grid gets; $number counts the board's grids.
pcb-board-grid-new-name = Grid { $number }
# The undo step of a guide, grid, sheet or naming edit (#2374).
pcb-board-options-undo = Board options
# The undo step of a layout plan change (#2942).
pcb-board-layout-plan-undo = Layout plan
# $why says why a board option was not changed.
pcb-board-options-refused = Board option not changed: { $why }

# Guide Manager: lines on the board that snapping can follow.
pcb-board-section-guides = Guide Manager
pcb-board-guide-enabled = Enabled
pcb-board-guide-x = X
pcb-board-guide-y = Y
pcb-board-guides-empty = No guides — add one
pcb-board-guide-vertical = Vertical
pcb-board-guide-horizontal = Horizontal
pcb-board-guide-plus45 = +45°
pcb-board-guide-minus45 = -45°
# Said in Messages after Place is picked.
pcb-board-guide-place-hint = Place guide: click on the board (Esc to cancel)

# Other board settings.
pcb-board-section-other = Other
pcb-board-units = Units
pcb-board-unit-mm = mm
pcb-board-unit-mils = mils
pcb-board-polygon-naming = Pour Names
# How a polygon pour is named: net name, layer number (LXX) or board layer
# name, and pour number (PXXX).
pcb-board-designator-display = Designators
pcb-board-designator-physical = Physical
pcb-board-designator-logical = Logical
pcb-board-sheet = Sheet Settings
pcb-board-sheet-width = Width
pcb-board-sheet-height = Height
pcb-board-sheet-from-layer = Size From Board
# The board's materials and finish live in the Layer Stack (#2523).
pcb-board-materials = Board materials
pcb-board-materials-link = Board materials & finish → Open Layer Stack
pcb-board-materials-tip = Solder mask, silkscreen, surface finish, copper weights and materials live in the Layer Stack.

# Grid and Guide Manager cards (#2386).
pcb-board-more = ⋯
pcb-board-more-tip = More actions
pcb-board-edit = Edit
pcb-board-move-up = Move Up
pcb-board-move-down = Move Down
pcb-board-grid-parts = Parts
pcb-board-grid-other = Other
pcb-board-grid-add = + Add Grid
pcb-board-grid-fixed-tip = The board's own grid applies to everything no other grid claims.
# A grid card's second line: its origin, and for the board's own grid what
# it applies to.
pcb-board-grid-origin = Origin
pcb-board-grid-applies-all = applies to all
pcb-board-guide-add = + Add Guide
pcb-board-guide-place-on-board = Place on Board
pcb-board-guide-glyph-vertical = │
pcb-board-guide-glyph-horizontal = ─
pcb-board-guide-glyph-plus45 = ╱
pcb-board-guide-glyph-minus45 = ╲

# Health Check tab: one card per check.
pcb-board-health-drc = Design rules
pcb-board-health-erc = Electrical rules
pcb-board-health-erc-source = From the project's schematic check
pcb-board-health-run-drc = Run DRC
pcb-board-health-show-results = Show results
pcb-board-health-not-run = Not run
pcb-board-health-findings = { $count ->
    [0] No findings
    [one] 1 finding
   *[other] { $count } findings
}
