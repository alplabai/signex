# Library editors: the symbol and footprint editors and their panels (#2225).

## Symbol Pins panel: a table of the open symbol's pins. The column headings
## are shown in capitals, so they are written in capitals here.

# Shown when no symbol editor is open.
editors-pins-no-symbol = Open a `.snxsym` to see its pins here.

# Line above the table. $pins, $hidden and $parts are whole numbers: the
# symbol's pins, how many of them are hidden, and its number of parts.
editors-pins-summary =
    { $pins ->
        [one] { $pins } pin
       *[other] { $pins } pins
    } · { $hidden } hidden · { $parts ->
        [one] { $parts } part
       *[other] { $parts } parts
    }

# Column heading: the pin number.
editors-pins-column-number = #

# Column heading: the pin name.
editors-pins-column-name = NAME

# Column heading: the pin's electrical type.
editors-pins-column-electrical = ELECTRICAL TYPE

# Column heading: which part of a multi-part symbol the pin belongs to.
editors-pins-column-part = PART

# Column heading: whether the pin is hidden.
editors-pins-column-hidden = HIDDEN

# Column heading: the pin length in millimetres.
editors-pins-column-length = LENGTH (mm)

# Column heading: the direction the pin points.
editors-pins-column-orientation = ORIENTATION

# Placeholder of the pin name field.
editors-pins-name-placeholder = name

# Toggle in the Hidden column: the pin is hidden.
editors-pins-hidden = Hidden

# Toggle in the Hidden column: the pin is shown.
editors-pins-shown = Shown

## Pin electrical types (the Pins panel and the Pin Map tab).

editors-pin-type-input = Input
editors-pin-type-output = Output
editors-pin-type-bidirectional = Bidirectional
# A tri-state output (can be high impedance).
editors-pin-type-three-statable = Tri-state
editors-pin-type-passive = Passive
editors-pin-type-power-input = Power input
editors-pin-type-power-output = Power output
# The ground reference pin.
editors-pin-type-ground-reference = Ground
# Open-drain or open-collector output, active low.
editors-pin-type-open-drain-low = Open drain (low)
# Open-drain or open-emitter output, active high.
editors-pin-type-open-drain-high = Open drain (high)
# A member of a differential pair.
editors-pin-type-differential = Differential
editors-pin-type-clock = Clock
# The pin must stay unconnected.
editors-pin-type-do-not-connect = Do not connect
# The author has not set the pin's type yet.
editors-pin-type-unclassified = Unclassified

## Pin orientations: the direction a pin points from its body.

editors-pin-orientation-up = Up
editors-pin-orientation-down = Down
editors-pin-orientation-left = Left
editors-pin-orientation-right = Right

## Footprint Layers panel.

# Shown when no footprint editor is open.
editors-layers-no-footprint = Open a `.snxfpt` to see its layers here.

# Button: make every layer visible.
editors-layers-show-all = Show all

# Button: show only the top-side layers.
editors-layers-top-side-only = Top side only

# Group heading, in capitals: the layers on the top side of the board.
editors-layers-side-top = TOP SIDE

# Group heading, in capitals: the layers on the bottom side of the board.
editors-layers-side-bottom = BOTTOM SIDE

# Group heading, in capitals: layers of the whole board (the outline).
editors-layers-side-board = BOARD

# Eye button glyph of a visible layer (a filled circle).
editors-layers-eye-shown = ◉

# Eye button glyph of a hidden layer (an empty circle).
editors-layers-eye-hidden = ○

## Footprint layer names.

editors-layer-top = Top Layer
editors-layer-bottom = Bottom Layer
# Silkscreen of the top side.
editors-layer-top-overlay = Top Overlay
# Silkscreen of the bottom side.
editors-layer-bottom-overlay = Bottom Overlay
# Assembly drawing of the top side.
editors-layer-top-assembly = Top Assembly
# Assembly drawing of the bottom side.
editors-layer-bottom-assembly = Bottom Assembly
# The board outline layer.
editors-layer-keep-out = Keep-Out

## Component editor (a library row opened as a document).

# Header note while the row has edits that are not saved.
editors-component-unsaved = • Unsaved changes

# Tab of the component editor: identity, symbol and footprint.
editors-component-tab-general = General
# Tab of the component editor: the row's parameters.
editors-component-tab-parameters = Parameters
# Tab of the component editor: manufacturer parts and distributor listings.
editors-component-tab-supply = Supply
# Tab of the component editor: the datasheets.
editors-component-tab-datasheet = Datasheet
# Tab of the component editor: the SPICE model.
editors-component-tab-simulation = Simulation
# Tab of the component editor: which footprint pad each symbol pin uses.
editors-component-tab-pin-map = Pin Map
# Tab of the component editor: alternate symbol drawings.
editors-component-tab-alternates = Alternates

## Component editor section headings and table columns. They are shown in
## capitals, so they are written in capitals here.

editors-component-symbol-heading = SYMBOL
editors-component-footprint-heading = FOOTPRINT
# Heading of the pin-to-pad table.
editors-component-pin-to-pad-heading = PIN TO PAD
editors-component-parameters-heading = PARAMETERS
# Heading of the form that adds a parameter the class template does not list.
editors-component-add-parameter-heading = ADD CUSTOM PARAMETER
# Heading of the SPICE model section.
editors-component-model-heading = MODEL
# Heading of the table that maps symbol pins to SPICE nodes.
editors-component-pin-node-heading = PIN / NODE MAP
editors-component-parts-heading = MANUFACTURER PARTS
editors-component-listings-heading = DISTRIBUTOR LISTINGS
editors-component-datasheets-heading = DATASHEETS
editors-component-alternates-heading = ALTERNATE SYMBOLS

editors-component-column-pin = PIN
editors-component-column-name = NAME
# The pin's electrical type.
editors-component-column-type = TYPE
# The footprint pad number.
editors-component-column-pad = PAD
# How the pad was found for the pin.
editors-component-column-match = MATCH
editors-component-column-label = LABEL
editors-component-column-symbol = SYMBOL
# Whether a parameter comes from the class template or is custom.
editors-component-column-scope = SCOPE
editors-component-column-value = VALUE
editors-component-column-unit = UNIT
editors-component-column-status = STATUS
# The SPICE netlist node a pin connects to.
editors-component-column-spice-node = SPICE NODE
# Primary part or alternate.
editors-component-column-role = ROLE
editors-component-column-manufacturer = MANUFACTURER
# Manufacturer part number.
editors-component-column-mpn = MPN
editors-component-column-notes = NOTES
editors-component-column-distributor = DISTRIBUTOR
# The distributor's stock-keeping unit number.
editors-component-column-sku = SKU
editors-component-column-link = LINK

# Pin Map Match cell when no footprint is loaded to match against.
editors-component-no-match = —

## Symbol editor body and part card.

# Caption in front of the Normal / Alternate body switch.
editors-symbol-body = Body
# Body switch: the normal drawing.
editors-symbol-body-normal = Normal
# Body switch: the alternate drawing (for example a De Morgan form).
editors-symbol-body-alternate = Alternate
# Button: the selection belongs to both bodies.
editors-symbol-body-both = Both
# Button: the selection belongs to the shown body only.
editors-symbol-body-this-only = This only
# Button: go to the previous part.
editors-symbol-previous-part = ←
# Button: go to the next part.
editors-symbol-next-part = →
# Button: add a part.
editors-symbol-add-part = +
# Button: remove the shown part.
editors-symbol-remove-part = −
# The shown part of a multi-part symbol. $part and $parts are whole numbers.
editors-symbol-part-of = Part { $part } / { $parts }
# Save button.
editors-symbol-save = Save
# Save button while there are unsaved edits.
editors-symbol-save-dirty = Save *

## Footprint editor canvas chips.

# The footprint has no sketch.
editors-sketch-none = No sketch
# The sketch has not been solved yet.
editors-sketch-not-solved = Sketch not solved
# The sketch has more constraints than it can satisfy.
editors-sketch-over-constrained = Over-constrained
# The sketch still has free degrees of freedom.
editors-sketch-under-constrained = Under-constrained
# The sketch is fully constrained: no degree of freedom (DOF) is left.
editors-sketch-fully-constrained = Fully constrained · DOF 0
# The footprint's name and its pad count. $pads is a whole number.
editors-footprint-name-pads =
    { $pads ->
        [one] { $name } · { $pads } pad
       *[other] { $name } · { $pads } pads
    }

## Symbol editor status bar segment
editors-symbol-status-part-pins = Part { $part } of { $parts } · { $pins ->
        [one] { $pins } pin
       *[other] { $pins } pins
    }
editors-symbol-status-pin-selected = Pin { $number } { $name } selected
editors-symbol-status-items-selected = { $count } selected
editors-symbol-copy-again = Copy Again

## Graphite views (#2225)
editors-component-read-only-footer = Read-only library - this component cannot be changed.
editors-component-unsaved-marker = • unsaved
editors-component-reload-confirm = Discard your edits and reload?

## Component editor header (Graphite)
editors-component-header-version = v{ $version }
editors-component-header-location = { $library } / { $table }
editors-component-header-unparsed = { $count ->
    [one] { $count } field does not parse
   *[other] { $count } fields do not parse
}

## Footprint editor status bar (Graphite)
editors-footprint-status-pad-selected = Pad { $number } — { $width } × { $height } { $unit } @ ({ $x }, { $y })
editors-footprint-status-pad-count = { $pads ->
    [one] { $pads } pad
   *[other] { $pads } pads
}

## Footprint editor Properties panel
# Header while a pad is selected: its number, its kind (SMD, Through-hole…)
# and its place among the footprint's pads. $index and $pads are whole numbers.
editors-footprint-pad-heading = Pad { $number } · { $kind } · { $index } of { $pads ->
        [one] { $pads } pad
       *[other] { $pads } pads
    }
# Beside the header while several pads are selected. $count is a whole number.
editors-footprint-pads-selected = ({ $count } pads selected)
# Pad kinds, as the Pad Properties header names them.
editors-pad-kind-smd = SMD
editors-pad-kind-through-hole = Through-hole
editors-pad-kind-npt-hole = NPT hole
editors-pad-kind-connector = Connector
editors-pad-kind-castellated = Castellated
editors-pad-kind-fiducial = Fiducial
editors-pad-kind-unknown = Unknown
# Degrees of freedom of the selected sketch point.
editors-footprint-dof = DOF
editors-footprint-dof-fully-constrained = Fully constrained
editors-footprint-unlink-corner-radius = Unlink corner radius
editors-footprint-role-points-only = Pad role applies to Points only
# A silk graphic baked from the sketch: edited in Sketch mode only.
editors-footprint-silk-from-sketch = From sketch: edit in Sketch mode
# Pad placement form. TAB is the key that pauses placement.
editors-pad-placement-paused = PAUSED — TAB to resume placement
editors-pad-placement-pause-hint = TAB to pause placement and edit defaults
editors-pad-testpoint = Testpoint

## Pad Properties form rows (Pads mode and the sketch pad attributes)
editors-pad-designator = Designator
editors-pad-layer = Layer
editors-pad-electrical-type = Electrical Type
editors-pad-net = Net
editors-pad-rotation = Rotation (°)
editors-pad-locked = Locked
editors-pad-template = Template
# The library the pad template comes from.
editors-pad-template-library = Library
# Pad features of each board side.
editors-pad-top-side = Top Side
editors-pad-bottom-side = Bottom Side
# Testpoint flags: assembly and fabrication testpoints on each side.
editors-pad-top-assembly = Top Assembly
editors-pad-top-fab = Top Fab
editors-pad-bottom-assembly = Bottom Assembly
editors-pad-bottom-fab = Bottom Fab
editors-pad-shape = Shape
editors-pad-size-x = X-Size
editors-pad-size-y = Y-Size
editors-pad-corner-radius = Corner radius %
editors-pad-trapezoid-delta-x = Delta X
editors-pad-trapezoid-delta-y = Delta Y
# Thermal relief toggle of the Simple pad stack.
editors-pad-relief = Relief
# Read-only position of the selected pad, one row per axis.
editors-pad-position-x = X (mm)
editors-pad-position-y = Y (mm)
# Simple pad stack: the paste opening on this side is on. $layer is a layer name.
editors-pad-paste-enabled = { $layer } on
# Simple pad stack: the solder mask covers the pad. $layer is a layer name.
editors-pad-tented = { $layer } tented
editors-pad-hole-details = Hole Details
editors-pad-hole-tolerance-plus = Tolerance + (mm)
editors-pad-hole-tolerance-minus = Tolerance − (mm)
editors-pad-hole-rotation = Hole rotation (°)
editors-pad-copper-offset-x = Copper offset X (mm)
editors-pad-copper-offset-y = Copper offset Y (mm)
editors-pad-top-paste-enabled = Top paste enabled
editors-pad-bottom-paste-enabled = Bottom paste enabled
editors-pad-top-mask-tented = Top mask tented
editors-pad-bottom-mask-tented = Bottom mask tented
editors-pad-thermal-relief = Thermal relief

## Silk graphic rows of the footprint Properties panel
editors-silk-from-x = From X (mm)
editors-silk-from-y = From Y (mm)
editors-silk-to-x = To X (mm)
editors-silk-to-y = To Y (mm)
# The text of a silk text graphic.
editors-silk-content = Content
editors-silk-position-x = Position X (mm)
editors-silk-position-y = Position Y (mm)
editors-silk-size = Size (mm)
editors-silk-stroke-width = Stroke width (mm)
editors-silk-filled = Filled
editors-silk-use-sketch = Use Sketch mode for parametric editing

## Pattern (array) rows of the footprint Properties panel
# Number of instances in the array.
editors-pattern-count = Count
# Expression that keeps or drops an instance.
editors-pattern-mask = Mask
# Total angle a polar array covers.
editors-pattern-sweep = Sweep
# Letters left out of grid row names, as IPC-7351 asks.
editors-pattern-skip-letters = Skip I/O/Q/S/X/Z (IPC-7351)
editors-pattern-start-row = Start row
editors-pattern-start-col = Start col

## Component editor empty states
editors-component-alternates-empty = No alternate drawings; the row uses its default symbol only.
editors-component-datasheets-empty = No datasheet on this component yet.
editors-component-parameters-empty = No parameters yet. Add a custom parameter below.
editors-component-parameters-no-template = No template for this class. Add custom parameters below.
editors-component-no-symbol-bound = No symbol bound
editors-component-no-footprint-bound = No footprint bound
editors-component-symbol-not-loaded = The symbol could not be loaded. It may be missing from the open libraries.
editors-component-footprint-not-loaded = The footprint could not be loaded. It may be missing from the open libraries.
editors-component-pin-map-pending = The pin map shows once the symbol is loaded.
editors-component-pin-map-no-pins = This symbol has no pins yet. Open the Symbol Editor to add some.
editors-component-model-empty = No SPICE model is bound to this component.
editors-component-pin-node-pending = Open the Symbol tab first to list the pins.
editors-component-pin-node-no-pins = The symbol has no pins yet.
editors-component-parts-empty = No alternates yet.
editors-component-listings-empty = No distributor listings yet.

## Schematic status bar while the Wire or Bus tool is active (#2372)
editors-wire-corner-horizontal-first = Corner: 90°, horizontal first
editors-wire-corner-vertical-first = Corner: 90°, vertical first
editors-wire-corner-diagonal-first = Corner: 45°, diagonal first
editors-wire-corner-diagonal-last = Corner: 45°, diagonal last
editors-wire-corner-any-angle = Corner: any angle

## Footprint Properties role sub-forms: Pour, Keepout, Cutout, Pattern and
## Sketch pad (#126). Lower-case words are captions and drawing labels.
# Header caption of a pour or keepout; $layer is a board layer name.
editors-role-region-caption = Region · { $layer }
editors-role-pour-title = Pour
editors-role-pour-section = Pour properties
# Placeholder of an empty net field.
editors-role-pour-no-net = (none)
# Drawing label of a pour with no net.
editors-role-pour-no-net-short = no net
editors-role-pour-priority = Priority
editors-role-pour-fill = Fill
editors-role-pour-fill-solid = Solid
editors-role-pour-fill-hatched = Hatched
# A pour drawn as its outline only.
editors-role-pour-fill-outline = Outline
# Drawing label; $fill is one of the fill names above.
editors-role-pour-fill-callout = Fill: { $fill }
editors-role-pour-priority-callout = Priority { $priority }
editors-role-keepout-title = Keepout
editors-role-keepout-kinds = Keepout kinds
editors-role-keepout-no-copper = No copper
editors-role-keepout-no-routing = No routing
editors-role-keepout-no-vias = No vias
editors-role-keepout-no-pours = No pours
editors-role-keepout-no-components = No components
editors-role-keepout-no-drilling = No drilling
editors-role-keepout-from-sketch = The region shape comes from the sketch (Role = Keepout).
editors-role-keepout-blocks-nothing = No kind ticked: the keepout blocks nothing
editors-role-cutout-title = Cutout
editors-role-cutout-caption = Board cutout
editors-role-cutout-section = Cutout properties
editors-role-cutout-edge-radius = Edge radius
# Placeholder of an empty edge radius: the corners stay sharp.
editors-role-cutout-sharp-placeholder = (sharp)
# Enable header: the cutout stops inside the board instead of going through.
editors-role-cutout-pocket = Limit depth (pocket)
editors-role-cutout-through = through the board
editors-role-cutout-pocket-short = pocket
# Drawing label of the board around a cutout.
editors-role-cutout-board = board
# Drawing callout; $radius is the edge radius expression as written.
editors-role-cutout-radius-callout = Edge radius { $radius }
editors-role-cutout-radius-sharp = Edge radius sharp
editors-role-pattern-title = Pattern
editors-role-pattern-instances = { $count ->
    [one] { $count } instance
   *[other] { $count } instances
}
# Header caption; $source, $first and $last are pad numbers.
editors-role-pattern-range = Pad { $source } → pads { $first }…{ $last }
editors-role-pattern-source = Pad { $source }
editors-role-pattern-kind-linear = Linear
editors-role-pattern-kind-grid = Grid
editors-role-pattern-kind-polar = Polar
editors-role-pattern-layout = Layout
# Two fields on one row: the instance count and the step along X.
editors-role-pattern-count-dx = Count / dX
editors-role-pattern-dy = dY
# Two fields on one row: columns and rows of a grid.
editors-role-pattern-nx-ny = nx / ny
editors-role-pattern-dx-dy = dX / dY
# Two fields on one row: the instance count and the angle a polar pattern covers.
editors-role-pattern-count-sweep = Count / Sweep
editors-role-pattern-centre = Centre
# The centre point of a polar pattern in mm.
editors-role-pattern-centre-value = { $x }, { $y }
editors-role-pattern-centre-lost = (centre lost)
editors-role-pattern-repick = Re-pick
editors-role-pattern-repicking = Click a Point… (Esc to cancel)
editors-role-pattern-instance-grid = Instances (untick one to skip it)
editors-role-pattern-numbering = Numbering
editors-role-pattern-scheme = Scheme
editors-role-pattern-scheme-linear = Linear (1, 2, 3, …)
editors-role-pattern-scheme-bga = BGA (A1, A2, …)
editors-role-pattern-scheme-explicit = Explicit list
editors-role-pattern-start-row-col = Start row / col
editors-role-pattern-delete = Delete pattern
# Drawing callouts; $value is a length in mm.
editors-role-pattern-dx-callout = dX { $value }
editors-role-pattern-dy-callout = dY { $value }
editors-role-pattern-legend = Source pad outlined · skipped instances dashed
editors-role-pattern-unresolved = The pattern does not evaluate: only the source pad is drawn
editors-role-pattern-too-many = { $count } instances: only the source pad is drawn
editors-role-sketch-pad-title = Sketch pad
# Header caption, followed by the Edit in Pads link; $kind is a pad kind name.
editors-role-sketch-pad-caption = Pad { $number } · { $kind } · from sketch profile ·
editors-role-sketch-pad-edit-in-pads = Edit in Pads ▸
editors-role-sketch-pad-attributes = Pad Attributes
# Section title; $layer is a copper layer name.
editors-role-sketch-pad-side-section = Pad · { $layer }
editors-role-sketch-pad-top-paste = Top Paste
editors-role-sketch-pad-bottom-paste = Bottom Paste
editors-role-sketch-pad-top-mask = Top Solder mask
editors-role-sketch-pad-bottom-mask = Bottom Solder mask
# State beside a switched-off paste header.
editors-role-sketch-pad-off = off
# State beside a solder mask header: open over the pad, or covering it.
editors-role-sketch-pad-opening = opening
editors-role-sketch-pad-tented = tented
editors-role-sketch-pad-expansion = Expansion
editors-role-sketch-pad-features = Pad Features
editors-role-sketch-pad-mask-off = Top Solder mask off: tented
# Drawing note: the copper size in mm, set by the sketch profile.
editors-role-sketch-pad-size = { $width } × { $height } (from the sketch)
# Drawing corner note; $layer is a copper layer name, $number the pad number.
editors-role-sketch-pad-corner = { $layer } · pad { $number }
