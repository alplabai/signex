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

# The header's summary of the Selection Filter. $first is the first object kind
# that is on; $more counts the other kinds that are on.
pcb-board-filter-summary = { $first } (and { $more } more)
pcb-board-filter-summary-none = No objects

# Selection Filter: which objects a click or a box picks on the board.
pcb-board-section-filter = Selection Filter
pcb-board-filter-all-on = All - On
pcb-board-filter-unavailable = The board editor cannot pick these objects yet.
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
pcb-board-snap-distance = Snap Distance
# $factor is a multiple of the snap grid, such as 0.5.
pcb-board-snap-distance-value = { $factor }x Snap Grid
pcb-board-axis-range = Axis Snap Range

# Board Information: totals read from the board.
pcb-board-section-info = Board Information
pcb-board-info-size = Board Size
pcb-board-info-horizontal = Horizontal
pcb-board-info-vertical = Vertical
pcb-board-info-area = Area
pcb-board-info-components-area = Components Area
pcb-board-info-density = Density
pcb-board-info-components = Components
pcb-board-info-total = Total
pcb-board-info-top = Top
pcb-board-info-bottom = Bottom
pcb-board-info-select-tip = Select these components
pcb-board-info-layers = Layers
pcb-board-info-signal = Signal
pcb-board-info-nets = Nets
pcb-board-info-unrouted = Unrouted
pcb-board-info-hidden = Hidden
# Shown as the unrouted count before the ratsnest is worked out.
pcb-board-info-not-counted = Not counted yet
pcb-board-info-primitives = Primitives & Others
pcb-board-info-arcs = Arcs
pcb-board-info-fills = Fills
pcb-board-info-pads = Pads
pcb-board-info-strings = Strings
pcb-board-info-tracks = Tracks
pcb-board-info-vias = Vias
pcb-board-info-polygons = Polygons
pcb-board-info-holes = Pad/Via Holes
pcb-board-info-drc = DRC Violations
pcb-board-info-drc-not-run = Not run
pcb-board-info-drc-tip = Show the design rule check results
pcb-board-info-not-held = The board does not hold these objects yet.
pcb-board-reports = Reports
pcb-board-reports-unavailable = No board report exists yet.
# A count in the Board Information rows.
pcb-board-count = { $count }
# A length or area from Signex's own formatter, unit included.
pcb-board-measure = { $value }
# Shown for a value the board cannot hold yet.
pcb-board-not-available = Not available
# Shown in a table cell the board cannot fill yet.
pcb-board-unset = —
pcb-board-percent = { $value } %

# Grid Manager: the board's snap grid.
pcb-board-section-grids = Grid Manager
pcb-board-grid-priority = Priority
pcb-board-grid-name = Name
pcb-board-grid-color = Color
pcb-board-grid-origin = Origin
pcb-board-grid-comp = Comp
pcb-board-grid-non-comp = Non Comp
pcb-board-grid-global = Global Board Snap Grid
pcb-board-grid-step = Step
pcb-board-grid-visible = Visible Grid
pcb-board-grid-fast = Fast Grids
pcb-board-grid-no-setup = This board has no setup to hold a grid step.
pcb-board-grid-invalid = Type a length above zero.
# $why says why the grid step was not set.
pcb-board-grid-refused = Grid step not set: { $why }
# The undo step's name.
pcb-board-grid-undo = Grid step
pcb-board-add = Add
pcb-board-properties = Properties
pcb-board-delete = Delete
pcb-board-place = Place
# A cell of the Global Board Snap Grid row: it applies to everything.
pcb-board-grid-always = Always
# A point in board coordinates; $x and $y come from Signex's own formatter.
pcb-board-point = { $x }, { $y }
pcb-board-swatch-tip = Click for the next color
pcb-board-grid-origin-x = Origin X
pcb-board-grid-origin-y = Origin Y
pcb-board-select-row-first = Select a row first.
# The name a new grid gets; $number counts the board's grids.
pcb-board-grid-new-name = Grid { $number }
# The undo step of a guide, grid, sheet or naming edit (#2374).
pcb-board-options-undo = Board options
# $why says why a board option was not changed.
pcb-board-options-refused = Board option not changed: { $why }

# Guide Manager: lines on the board that snapping can follow.
pcb-board-section-guides = Guide Manager
pcb-board-guide-enabled = Enabled
pcb-board-guide-x = X
pcb-board-guide-y = Y
pcb-board-guides-empty = No guides
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
pcb-board-polygon-naming = Polygon Naming Scheme
# How a polygon pour is named: net name, layer number (LXX) or board layer
# name, and pour number (PXXX).
pcb-board-naming-net-layer = NET NAME_LXX_PXXX
pcb-board-naming-layer-net = LXX_NET NAME_PXXX
pcb-board-naming-board-layer-net = Board Layer_NET NAME_PXXX
pcb-board-naming-net-board-layer = NET NAME_Board Layer_PXXX
pcb-board-designator-display = Designator Display
pcb-board-designator-physical = Physical
pcb-board-designator-logical = Logical
pcb-board-sheet = Sheet Settings
pcb-board-sheet-width = Width
pcb-board-sheet-height = Height
pcb-board-sheet-from-layer = Get Size From Sheet Layer
pcb-board-sheet-from-layer-tip = The sheet is the box around the board shape while Get Size From Sheet Layer is on.

# Health Check tab.
pcb-board-health-drc = Design rules
pcb-board-health-erc = Electrical rules
pcb-board-health-run-drc = Run DRC
pcb-board-health-not-run = Not run
pcb-board-health-findings = { $count ->
    [0] No findings
    [one] 1 finding
   *[other] { $count } findings
}
