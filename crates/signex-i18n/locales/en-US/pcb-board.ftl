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

# Snap Options: what a carried footprint snaps to.
pcb-board-section-snap = Snap Options
pcb-board-snap-grids = Grids
pcb-board-snap-guides = Guides
pcb-board-snap-axes = Axes
pcb-board-snap-guides-unavailable = Boards hold no guides yet.
pcb-board-snap-axes-unavailable = Axis snapping is not available yet.
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
pcb-board-snap-object-unavailable = The board editor does not snap to these points yet.
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
pcb-board-grid-fast-unavailable = Fast grids are not available yet.
pcb-board-grid-more-unavailable = Boards hold one grid; more grids, their colours and origins cannot be stored yet.
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

# Guide Manager: lines on the board that snapping can follow.
pcb-board-section-guides = Guide Manager
pcb-board-guide-enabled = Enabled
pcb-board-guide-x = X
pcb-board-guide-y = Y
pcb-board-guides-empty = No guides

# Other board settings.
pcb-board-section-other = Other
pcb-board-units = Units
pcb-board-unit-mm = mm
pcb-board-unit-mils = mils
pcb-board-polygon-naming = Polygon Naming Scheme
pcb-board-polygon-naming-default = NET NAME_LXX_PXXX
pcb-board-polygon-naming-unavailable = Boards do not name polygons yet.
pcb-board-designator-display = Designator Display
pcb-board-designator-physical = Physical
pcb-board-designator-unavailable = Each part has one designator; logical designators need multi-channel design.
pcb-board-sheet = Sheet Settings
pcb-board-sheet-width = Width
pcb-board-sheet-height = Height
pcb-board-sheet-from-layer = Get Size From Sheet Layer
pcb-board-sheet-unavailable = Boards hold no sheet yet.

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
