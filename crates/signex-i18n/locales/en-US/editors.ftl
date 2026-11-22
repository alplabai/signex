# Library editors: the symbol and footprint editors and their panels (#2225).

## Pins tab of the symbol editor's Properties panel: a list of the open
## symbol's pins. The column headings are shown in capitals, so they are
## written in capitals here.

# Line above the list. $pins, $hidden and $parts are whole numbers: the
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

# Pin table column heading: the pin's electrical type.
editors-pins-column-electrical = ELECTRICAL TYPE

# Pin table column heading: which part of a multi-part symbol the pin belongs to.
editors-pins-column-part = PART

# Pin table column heading: whether the pin is hidden.
editors-pins-column-hidden = HIDDEN

# Pin table column heading: the pin length in millimetres.
editors-pins-column-length = LENGTH (mm)

# Pin table column heading: the direction the pin points.
editors-pins-column-orientation = ORIENTATION

# Placeholder of the pin table's name field.
editors-pins-name-placeholder = name

# Toggle in the pin table's Hidden column: the pin is hidden.
editors-pins-hidden = Hidden

# Toggle in the pin table's Hidden column: the pin is shown.
editors-pins-shown = Shown

## Pin electrical types (the Pins tab, Pin properties and the Pin Map tab).

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
editors-layer-top-overlay = Top Silkscreen
# Silkscreen of the bottom side.
editors-layer-bottom-overlay = Bottom Silkscreen
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
# The Template row for a pad with no named template.
editors-pad-template-custom = (custom)
# The name of a template only this pad uses.
editors-pad-template-name = Name
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

## Footprint Properties body: header and sections (mockup row 124)
# Header with nothing selected.
editors-fp-props-title = Footprint
# Status line under it. $pads is a whole number.
editors-fp-props-status = { $pads ->
        [one] { $pads } pad
       *[other] { $pads } pads
    }
# Status line once the sketch was solved. $state names the solve state.
editors-fp-props-status-solved = { $pads ->
        [one] { $pads } pad
       *[other] { $pads } pads
    } · { $state }
editors-fp-props-section-footprint = Footprint
editors-fp-props-name = Name
editors-fp-props-description = Description
editors-fp-props-designator = Designator
# Beside the designator: it is the default for placed parts.
editors-fp-props-designator-default = default
# The component type (Standard, Mechanical…).
editors-fp-props-type = Type
editors-fp-props-height = Height (mm)
editors-fp-props-version = Version
editors-fp-props-section-courtyard = Courtyard & Silkscreen
# Enable header: the courtyard follows the pads.
editors-fp-props-courtyard = Courtyard (generated)
# Size of the courtyard. $width and $height are millimetre numbers.
editors-fp-props-courtyard-size = { $width } × { $height } mm
# How far the generated courtyard grows past the pads.
editors-fp-props-expansion = Expansion
# A length in millimetres. $value is a number.
editors-fp-props-length-mm = { $value } mm
# Enable header: a silkscreen outline generated on the part's body.
editors-fp-props-silk-outline = Silkscreen outline
# Note at the right of the Silkscreen outline header.
editors-fp-props-silk-clipped = clipped at pads
# Stroke width of the generated silkscreen outline.
editors-fp-props-line-width = Line width
# How far the silkscreen outline stays from the pads.
editors-fp-props-pad-clearance = Pad clearance
# Checkbox text of the Pin 1 marker row.
editors-fp-props-pin-one-dot = Dot beside pad 1
# Size of the part's body. $width and $height are millimetre numbers.
editors-fp-props-assembly-body = body { $width } × { $height }
# Note when a generated outline has no body to follow.
editors-fp-props-no-body = needs a 3D body or fab outline
# Leader label of the courtyard in the illustration.
editors-fp-props-illustration-courtyard = Courtyard
editors-fp-props-illustration-silkscreen = Silkscreen
editors-fp-props-illustration-assembly = Assembly outline
editors-fp-props-illustration-pin-one = Pin 1 marker
editors-fp-props-section-snap = Snap Options
editors-fp-props-snapping = Snapping
editors-fp-props-snap-all-layers = All Layers
editors-fp-props-snap-current-layer = Current Layer
editors-fp-props-snap-off = Off
editors-fp-props-snap-grids = Grids
editors-fp-props-snap-guides = Guides
editors-fp-props-snap-axes = Axes
editors-fp-props-snap-objects = Objects for snapping
editors-fp-props-object-track-vertices = Track Vertices
editors-fp-props-object-track-lines = Track Lines
editors-fp-props-object-arc-centers = Arc Centers
editors-fp-props-object-intersections = Intersections
editors-fp-props-object-pad-centers = Pad Centers
editors-fp-props-object-pad-vertices = Pad Vertices
editors-fp-props-object-pad-edges = Pad Edges
editors-fp-props-object-via-centers = Via Centers
editors-fp-props-object-texts = Texts
editors-fp-props-object-regions = Regions
editors-fp-props-object-footprint-origins = Footprint Origins
editors-fp-props-object-body-points = 3D Body Snap Points
editors-fp-props-snap-distance = Snap Distance
editors-fp-props-axis-snap-range = Axis Snap Range
# Beside Axis Snap Range while snapping to axes is off.
editors-fp-props-axes-off = Axes off
# The active grid's step, where the Grid Manager is not shown.
editors-fp-props-grid-step-row = Grid step (mm)
# Note in the snap illustration while snapping is off.
editors-fp-props-illustration-snapping-off = Snapping off
editors-fp-props-section-grids = Grid Manager
# Header button that adds a grid.
editors-fp-props-grid-add = + Add
editors-fp-props-grid-name = Name
editors-fp-props-grid-step = Step
editors-fp-props-grid-display = Display
editors-fp-props-grid-lines = Lines
editors-fp-props-grid-dots = Dots
editors-fp-props-grid-hidden = Hidden
# Opens the Grid Properties dialog of the active grid.
editors-fp-props-grid-properties = Properties…
editors-fp-props-delete = Delete
editors-fp-props-grid-none = No grids
editors-fp-props-section-solve = Sketch solve
# Pill of a fully constrained sketch: no degree of freedom left.
editors-fp-props-dof-zero = DOF 0
editors-fp-props-entities = Entities
editors-fp-props-constraints = Constraints
editors-fp-props-iterations = Iterations
editors-fp-props-residual = Residual
editors-fp-props-elapsed = Elapsed
# A solve time. $ms is a whole number of milliseconds.
editors-fp-props-elapsed-value = { $ms } ms
editors-fp-props-over-constrained = Over-constrained
editors-fp-props-not-solved = Not solved yet
editors-fp-props-redundant = Redundant
# A constraint that restates earlier ones. $kind names the constraint.
editors-fp-props-redundant-row = { $kind } — already implied by earlier constraints
editors-fp-props-conflicts = Conflicts (worst first)
# More rows than the list shows. $count is a whole number.
editors-fp-props-more = … +{ $count } more
# A count in a section header pill. $count is a whole number.
editors-fp-props-count = { $count }
editors-fp-props-section-parameters = Parameters
editors-fp-props-parameters-none = None yet: add one through an expression.
editors-fp-props-parameter-placeholder = expression…
editors-fp-props-section-warnings = Solve warnings
editors-fp-props-warnings-none = None
editors-fp-props-open-sketch = Open the sketch (Edit Sketch in the active bar) to see the constraints on the canvas.
# Hint at the bottom of the panel, by editor mode.
editors-fp-props-hint-pads = Click a pad to edit its properties.
editors-fp-props-hint-sketch = Click a sketch entity (Point, Line, Arc or Circle) to edit it.
editors-fp-props-hint-view3d = 3D View: use the 3D preview pane to inspect the body.
# A sketch entity kind. $kind is the entity kind's name in the code.
editors-fp-props-entity-kind = { $kind ->
        [Point] Point
        [Line] Line
        [Arc] Arc
        [Circle] Circle
       *[other] Entity
    }
# Header of a selected sketch entity. $kind is its localized kind.
editors-fp-props-entity-heading = Sketch entity · { $kind }
# Status line of a selected sketch entity. $constraints is a whole number.
editors-fp-props-entity-status = { $constraints ->
        [one] { $constraints } constraint attached
       *[other] { $constraints } constraints attached
    }
# The same, with the entity's constraint state. $state names it.
editors-fp-props-entity-status-dof = { $constraints ->
        [one] { $constraints } constraint attached
       *[other] { $constraints } constraints attached
    } · { $state }
editors-fp-props-section-entity = Entity
editors-fp-props-kind = Kind
editors-fp-props-role = Role
editors-fp-props-construction = Construction
editors-fp-props-yes = Yes
editors-fp-props-no = No
editors-fp-props-attached = Attached constraints
editors-fp-props-banner-under = Under-constrained: degrees of freedom are left.
editors-fp-props-banner-full = Fully constrained: no degree of freedom is left.
editors-fp-props-banner-over = Over-constrained: a constraint conflicts with the others.
# A silk graphic kind. $kind is the graphic kind's name in the code.
editors-fp-props-silk-kind = { $kind ->
        [Line] Line
        [Text] Text
        [Rectangle] Rectangle
        [Circle] Circle
        [Arc] Arc
       *[other] Polygon
    }
# Header of a selected silk graphic. $kind is its localized kind.
editors-fp-props-silk-heading = Silk graphic · { $kind }
editors-fp-props-section-silk = Silk graphic
editors-fp-props-index = Index
# The pad stack check card. $count is a number of sketch pads.
editors-fp-props-pad-stack-check = Pad stack check
editors-fp-props-pad-stack-offer = { $count ->
        [one] The sketch pad has paste disabled and direct connect, the stack older builds wrote. It produces no paste stencil opening. Enable paste and thermal relief on it? Undo reverts it; nothing is saved until you save.
       *[other] All { $count } sketch pads have paste disabled and direct connect, the stack older builds wrote. They produce no paste stencil openings. Enable paste and thermal relief on them? Undo reverts it; nothing is saved until you save.
    }
editors-fp-props-pad-stack-restore = { $count ->
        [one] Enable paste and thermal relief on 1 pad
       *[other] Enable paste and thermal relief on { $count } pads
    }
editors-fp-props-pad-stack-keep = Keep as is
# $pads lists the pad numbers.
editors-fp-props-pad-stack-no-paste = { $count ->
        [one] Warning: 1 SMD pad has paste disabled on its copper side and gets no stencil opening: { $pads }
       *[other] Warning: { $count } SMD pads have paste disabled on their copper side and get no stencil opening: { $pads }
    }
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

## Symbol editor Properties panel (mockup rows 122 and 123): the Symbol and Pins tabs with
## nothing selected, Pin properties and graphic properties. Section titles are
## shown in capitals by the panel.

# Header title and tab with nothing selected.
editors-symprops-title = Symbol
editors-symprops-tab-symbol = Symbol
editors-symprops-tab-pins = Pins
# Header pill and Part stepper: the shared part, drawn on every part.
editors-symprops-part-shared = Part 0 (shared)
# Header pill and Part stepper. $part is the part number, $letter its letter (A for part 1).
editors-symprops-part-lettered = Part { $part } ({ $letter })
# A part past Z, and a part chip of the Pins tab. $part is the part number.
editors-symprops-part-numbered = Part { $part }
# Caption under the title. $file is the .snxsym file name, $symbol the symbol name.
editors-symprops-file-symbol = { $file } · { $symbol }
editors-symprops-section-general = General
editors-symprops-symbol-name = Symbol name
editors-symprops-uuid = UUID
# The number of graphics (lines, shapes, text) the symbol holds.
editors-symprops-graphics = Graphics
editors-symprops-section-parts = Parts
editors-symprops-part = Part
# Whether the selected items belong to both bodies or only the body shown.
editors-symprops-applies-to = Applies to
# Part stepper buttons.
editors-symprops-previous-part = ‹
editors-symprops-next-part = ›
editors-symprops-previous-part-tip = Previous part
editors-symprops-next-part-tip = Next part
editors-symprops-add-part-tip = Add part
editors-symprops-remove-part-tip = Remove part
editors-symprops-section-graphical = Graphical
editors-symprops-mirrored = Mirrored
editors-symprops-local-colors = Local Colors
editors-symprops-fills = Fills
editors-symprops-lines = Lines
editors-symprops-pins = Pins
# A local colour that is not set: the symbol uses the sheet colours.
editors-symprops-inherit = Inherit
editors-symprops-section-fields = Fields
editors-symprops-reference = Reference
editors-symprops-value = Value
editors-symprops-value-bound = Bound to the host Component's value at place-time.
editors-symprops-reference-bound = Bound to the host Component's designator at place-time.
# Header title while the Reference or Value field is selected on the canvas.
editors-symprops-reference-field = Reference field
editors-symprops-value-field = Value field
editors-symprops-select-hint = Click on the canvas to select a pin or a graphic, or use the Pins tab.

# Pins tab.
editors-symprops-pin-filter = Filter by number or name
editors-symprops-add-pin = + Pin
# Pins tab: switch the compact list to the pin table (name and hidden
# edited in place for every listed pin), and back.
editors-symprops-pin-table-open = Edit pins in table…
editors-symprops-pin-table-close = Back to the pin list
editors-symprops-add-pin-tip = Add a pin to the current part
# Part chip: every pin.
editors-symprops-chip-all = All
# After the pin summary, in the error colour. $count is the number of pins with a
# duplicate or empty number or no name.
editors-symprops-pins-issues = · { $count } with issues
# Column headings, in capitals: the electrical type and the part number.
editors-symprops-column-type = TYPE
editors-symprops-column-part = PT
editors-symprops-no-match = No pins match
editors-symprops-no-match-hint = Clear the filter or pick another part chip.
editors-symprops-no-pins = No pins yet
editors-symprops-no-pins-hint = Add one with + Pin.
# A pin row whose pin has no name.
editors-symprops-no-name = no name
# A pin row whose pin has no number.
editors-symprops-no-number = ?

# Pin properties.
editors-symprops-back-to-pins = ‹ Pins
editors-symprops-back-to-pins-tip = Clear the selection and show the Pins tab
# Header title. $number is the pin number.
editors-symprops-pin-title = Pin { $number }
# Caption under a pin or graphic title. $symbol is the symbol name, $part the part number.
editors-symprops-object-context = { $symbol } · Part { $part }
editors-symprops-section-pin = Pin
editors-symprops-designator-name = Designator / Name
# Placeholder of the pin number field.
editors-symprops-designator-placeholder = number
# Placeholder of the pin name field.
editors-symprops-name-placeholder = name
editors-symprops-electrical = Electrical
editors-symprops-description = Description
editors-symprops-description-placeholder = free text
editors-symprops-function = Function
editors-symprops-function-placeholder = alt-names, comma-separated
editors-symprops-part-number = Part Number
# Unit hint beside the Part Number field.
editors-symprops-part-number-unit = 0 = shared
editors-symprops-section-location = Location
editors-symprops-x-y = X / Y
editors-symprops-length = Length
editors-symprops-orientation = Orientation
# Millimetres, beside a length field.
editors-symprops-unit-mm = mm
editors-symprops-section-visibility = Visibility
editors-symprops-name-visible = Name visible
editors-symprops-designator-visible = Designator visible
editors-symprops-hidden = Hidden (Pin Hide)
editors-symprops-locked = Locked
editors-symprops-section-symbols = Symbols
editors-symprops-inside = Inside
editors-symprops-inside-edge = Inside Edge
editors-symprops-outside-edge = Outside Edge
editors-symprops-outside = Outside

## Pin symbols: the glyphs a pin can carry inside, on the edge of, or outside
## the symbol body.

editors-pin-symbol-none = None
editors-pin-symbol-dot = Dot (active-low bubble)
editors-pin-symbol-clock-edge = Clock edge
editors-pin-symbol-active-low-input = Active-low input
editors-pin-symbol-active-low-output = Active-low output
editors-pin-symbol-schmitt-trigger = Schmitt trigger
editors-pin-symbol-analog = Analog (≈)
editors-pin-symbol-digital = Digital (square wave)
editors-pin-symbol-shift-right = Shift-right (▷)
editors-pin-symbol-shift-left = Shift-left (◁)
editors-pin-symbol-pi = Pi (π)
editors-pin-symbol-sigma = Sigma (Σ)
editors-pin-symbol-open-collector = Open collector
editors-pin-symbol-open-emitter = Open emitter
editors-pin-symbol-hi-z = Hi-Z (tri-state)

## Graphic properties of the symbol editor.

editors-symprops-rectangle = Rectangle
editors-symprops-line = Line
editors-symprops-circle = Circle
editors-symprops-arc = Arc
editors-symprops-text = Text
editors-symprops-polygon = Polygon
editors-symprops-section-geometry = Geometry
editors-symprops-section-style = Style
editors-symprops-start-x-y = Start X / Y
editors-symprops-end-x-y = End X / Y
editors-symprops-width-height = Width / Height
editors-symprops-center-x-y = Center X / Y
editors-symprops-radius = Radius
editors-symprops-start-angle = Start °
editors-symprops-end-angle = End °
# Text height in millimetres.
editors-symprops-size = Size
editors-symprops-content = Content
# Placeholder of the text content field.
editors-symprops-content-placeholder = text
editors-symprops-vertices = Vertices
editors-symprops-stroke = Stroke (mm)
editors-symprops-fill = Fill
# Fill colour that is not set: the shape is drawn as an outline.
editors-symprops-fill-none = None
# Callout labels of the illustration above the fields.
editors-symprops-callout-width = Width
editors-symprops-callout-height = Height
editors-symprops-callout-start = Start
editors-symprops-callout-end = End
editors-symprops-callout-stroke = Stroke
## Component editor window, Graphite (mockup row 128)

# Tooltip of the header's close button.
editors-component-close = Close
# Small button beside the Symbol or Footprint caption: bind another one.
editors-component-change = Change…
# Small button beside the Symbol or Footprint caption: open it in its editor.
editors-component-open-primitive = Open
# Caption under the symbol tile: the file and the symbol in it.
editors-component-symbol-caption = { $file } · { $symbol }
# The same for a symbol of several parts. $part and $parts are whole numbers.
editors-component-symbol-part-caption = { $file } · { $symbol } · Part { $part } of { $parts }
# Caption under the footprint tile: the file and the footprint in it.
editors-component-footprint-caption = { $file } · { $footprint }
# Pill beside PIN TO PAD: pins that land on a pad, of all pins.
editors-component-pin-to-pad-matched = { $mapped } / { $total } matched
# Hint under the pin-to-pad table. $pin is the first pin's number.
editors-component-pin-to-pad-hint = Click a pad cell to override: "Override pin { $pin } → pad number…"
# Match cell of the pin-to-pad table: the pin has a pad of its number.
editors-component-match-ok = ok
# Match cell: the pin is overridden to another pad.
editors-component-match-override = override
# Match cell: the footprint has no pad for the pin.
editors-component-match-no-pad = no pad
# Footer button: replace the working copy with the stored row.
editors-component-reload-saved = Reload saved version
# Confirm button of that reload.
editors-component-reload = Reload
# Keep the working copy instead of reloading.
editors-component-keep-editing = Keep editing
# Shortcut hint beside Save.
editors-component-save-shortcut = Ctrl+S
# Footer button (not available yet): release the component.
editors-component-release = Release…
# Footer button: save and close the editor.
editors-component-save-and-close = Save & Close
# Footer button: close the editor (asks first when there are unsaved edits).
editors-component-cancel = Cancel
# Footer button: save the row.
editors-component-save = Save
# Tooltip of the disabled Save. $field is the parameter that does not parse.
editors-component-save-blocked = { $field } does not parse. Fix it to save.
# Footer summary of the fields that do not parse. $count is a whole number,
# $field the first such parameter and $reason why it was refused (English).
editors-component-footer-unparsed = { $count ->
    [one] { $field } does not parse: { $reason }
   *[other] { $count } fields do not parse. { $field }: { $reason }
}
# Read-only footer button: copy the component row into a writable library.
editors-component-copy-to-library = Copy to Library…
# Tooltip on the disabled Copy to Library… until a component row copy exists.
editors-component-copy-to-library-unavailable = Copying a component to another library is not available yet.

## Footprint Pad Properties (mockup row 125)
# Header, first line: the selected pad's number.
editors-fp-pad-title = Pad { $number }
# Header, second line: the pad's place among the footprint's pads. $index and $pads are whole numbers.
editors-fp-pad-place = { $index } of { $pads ->
        [one] { $pads } pad
       *[other] { $pads } pads
    }
# Header while a pad is being placed: the form edits the next pad.
editors-fp-pad-next-title = Next pad
# Link in the header that opens the pad's profile in Sketch mode.
editors-fp-pad-edit-in-sketch = Edit in Sketch ▸
# The Driven by sketch section of a baked pad (mockup row 192, item D5).
editors-fp-pad-driven-by = Driven by sketch
editors-fp-pad-driven-source = Source
editors-fp-pad-driven-point = Pad point
editors-fp-pad-driven-profile = Profile of { $edges } edges
editors-fp-pad-driven-instance = Instance
editors-fp-pad-driven-single = Single pad
editors-fp-pad-driven-pattern = { $kind ->
        [linear] Linear
        [grid] Grid
        [circular] Circular
       *[mirror] Mirror
    } pattern, instance { $place }
editors-fp-pad-driven-size = Size
editors-fp-pad-driven-size-value = { $x } × { $y } mm · from sketch
# Kind pill of a non-plated hole.
editors-fp-pad-pill-npth = NPTH
editors-fp-pad-section-properties = Properties
# The section of an SMD pad: it has one layer, so no pad stack.
editors-fp-pad-section-pad-top = Pad · Top Layer
editors-fp-pad-section-pad-bottom = Pad · Bottom Layer
editors-fp-pad-section-pad-stack = Pad Stack
editors-fp-pad-section-paste-solder = Paste & Solder
editors-fp-pad-section-features = Pad Features
# Pad stack modes of a through-hole pad.
editors-fp-pad-stack-simple = Simple
editors-fp-pad-stack-top-middle-bottom = Top-Middle-Bottom
editors-fp-pad-stack-full = Full Stack
# Layer chips under the illustration of a through-hole pad.
editors-fp-pad-layers = Layers
editors-fp-pad-layer-top = Top
editors-fp-pad-layer-inner = Inner
# Every inner copper layer, named by position: L1 is Top and Ln is Bottom.
editors-fp-pad-layer-inner-range = L2…Ln−1
editors-fp-pad-layer-bottom = Bottom
editors-fp-pad-size = Size X / Y
editors-fp-pad-position = X / Y (mm)
editors-fp-pad-copper-offset = Copper offset
# Hole shapes the pad model stores.
editors-fp-pad-hole-round = Round
editors-fp-pad-hole-slot = Slot
editors-fp-pad-hole-size = Size
editors-fp-pad-hole-width-length = Width / Length
editors-fp-pad-plated = Plated
editors-fp-pad-tolerance = Tolerance +/−
# Beside Thermal relief while it is off: the pad connects to a plane directly.
editors-fp-pad-thermal-off = off · direct connect
editors-fp-pad-air-gap = Air gap
editors-fp-pad-spoke-width = Spoke width
editors-fp-pad-spokes = Spokes
# Where a value comes from: the board's design rule, or a value typed for this pad.
editors-fp-pad-rule = Rule
editors-fp-pad-manual = Manual
# Tag in a value cell that the board's design rule decides.
editors-fp-pad-rule-tag = rule
editors-fp-pad-expansion = Expansion
# A pad's solder mask section; $layer is the solder mask layer's name from the Layers panel ("Top Solder").
editors-fp-pad-solder-mask-layer = { $layer } mask
# Beside a paste layer that is switched off.
editors-fp-pad-paste-off = off
# Beside a solder mask layer: an opening over the pad, or the mask covers it.
editors-fp-pad-mask-opening = opening
editors-fp-pad-mask-tented = tented
editors-fp-pad-no-through-hole-paste = No paste on through-hole pads and holes.
# Why a typed value was not taken. The field keeps the last good value.
editors-fp-pad-invalid-number = Enter a number
editors-fp-pad-invalid-positive = Must be greater than 0
editors-fp-pad-invalid-negative = Must be 0 or more
editors-fp-pad-invalid-no-copper = Leaves no copper with this trapezoid
editors-fp-pad-invalid-corner-radius = Must be 0 to 50
# Illustration labels. $value is a length in mm, already formatted.
editors-fp-pad-callout-title = Pad { $number } · { $layer }
editors-fp-pad-callout-size-x = Size X { $value }
editors-fp-pad-callout-size-y = Size Y { $value }
editors-fp-pad-callout-diameter = Size Ø { $value }
# $value is a whole-number percentage.
editors-fp-pad-callout-corner = Corner radius { $value }%
editors-fp-pad-callout-hole-plated = Hole, plated
editors-fp-pad-callout-slot-plated = Slot, plated
editors-fp-pad-callout-hole-npth = Hole, no copper
editors-fp-pad-callout-solder-mask = Solder mask
# $layer is a solder mask layer name, such as Top Solder mask.
editors-fp-pad-callout-tented = { $layer } off: tented
# Between the parts of the pad header's second line.
editors-fp-pad-separator = ·
# Placeholder of the next pad's designator: numbered automatically.
editors-fp-pad-auto = (auto)
# Placeholder of a pad net that is not set.
editors-fp-pad-unassigned = (unassigned)
## Footprint Properties role sub-forms: Pour, Keepout, Cutout, Pattern and
## Sketch pad (mockup row 126). Lower-case words are captions and drawing labels.
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
editors-role-pattern-sweep = Sweep
editors-role-pattern-sweep-row = Row by row
editors-role-pattern-sweep-column = Column by column
editors-role-pattern-sweep-dip = Down then up (DIP)
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

## Symbol editor active bar
# Header of the Shapes menu.
editors-symbol-bar-shapes = Shapes
# Tooltip of the selection filter trigger; $kinds lists the filter categories.
editors-symbol-bar-filter-tooltip = Selection Filter: { $kinds } (left or right click for menu)
# A selection filter category; $kind is the category's name in the code.
editors-symbol-bar-filter-kind = { $kind ->
        [Pins] Pins
        [Drawings] Drawings
        [Texts] Texts
        [Designators] Designators
        [Values] Values
        [Parameters] Parameters
       *[other] Other
    }

## Sketch tools shared by the footprint and board sketch (#2518).

# Role picker: what a selected sketch entity becomes when the sketch bakes.
editors-sketch-role-unassigned = Unassigned
editors-sketch-role-pad = Pad
editors-sketch-role-silk-top = Silk · Top
editors-sketch-role-silk-bottom = Silk · Bottom
editors-sketch-role-courtyard = Courtyard
editors-sketch-role-keepout = Keepout
# A cutout through the board, drawn in a footprint.
editors-sketch-role-board-cutout = Board Cutout
# A cutout drawn on the board itself.
editors-sketch-role-cutout = Cutout
editors-sketch-role-mask-opening-top = Mask Opening · Top
editors-sketch-role-mask-opening-bottom = Mask Opening · Bottom
editors-sketch-role-mask-exclude-top = Mask Exclude · Top
editors-sketch-role-mask-exclude-bottom = Mask Exclude · Bottom
editors-sketch-role-pour-top = Pour · Top
editors-sketch-role-pour-bottom = Pour · Bottom
editors-sketch-role-paste-top = Paste · Top
editors-sketch-role-paste-bottom = Paste · Bottom
editors-sketch-role-board-outline = Board outline
editors-sketch-role-mounting-hole = Mounting hole
editors-sketch-role-v-score = V-score
# Messages, when a board role is picked for a shape it cannot take.
editors-sketch-role-does-not-fit = { $role } was not set: a board outline cannot be a point, a mounting hole needs a circle and a V-score a line.

# Active Bar tooltips.
editors-sketch-select = Select
editors-sketch-create = Create: Line, Rectangle, Rounded Rectangle, Circle, Arc, Tangent Arc, Edge Arc, Point
editors-sketch-modify = Modify: Fillet, Trim, Mirror, Offset, patterns, Make Pad from Profile
editors-sketch-snap = Snap options (left or right click for menu)
editors-sketch-linetype = Linetype of new entities: Normal, Construction or Centerline
# The Linetype segments: a solid line, a dashed line, a dash-dot line.
editors-sketch-linetype-normal = ━
editors-sketch-linetype-construction = ┅
editors-sketch-linetype-centerline = ━ · ━

# Create and Modify menus.
editors-sketch-menu-create = Create
editors-sketch-menu-modify = Modify
editors-sketch-menu-needs-selection = Needs a selection
editors-sketch-tool-line = Line
editors-sketch-tool-rectangle = Rectangle
editors-sketch-tool-rounded-rectangle = Rounded Rectangle
editors-sketch-tool-circle = Circle
editors-sketch-tool-arc = Arc
editors-sketch-tool-tangent-arc = Tangent Arc
editors-sketch-tool-edge-arc = Edge Arc
editors-sketch-tool-point = Point
editors-sketch-tool-fillet = Fillet
editors-sketch-tool-trim = Trim
editors-sketch-tool-mirror = Mirror
editors-sketch-tool-offset = Offset
editors-sketch-tool-rect-pattern = Rectangular Pattern
editors-sketch-tool-circular-pattern = Circular Pattern
editors-sketch-make-pad = Make Pad from Profile
# Why Make Pad from Profile is greyed in a board sketch.
editors-sketch-footprint-only = footprint only
editors-sketch-make-pad-board = A board has no pads: Make Pad from Profile works in the footprint editor.

# The sketch mode bar: the one way out of sketch mode on both hosts.
editors-sketch-mode-title = Sketch
editors-sketch-bake-preview = Bake preview
editors-sketch-back-to-sketch = Back to sketch
editors-sketch-discard-changes = Discard changes
editors-sketch-bake = Bake
editors-sketch-bake-tooltip = Bake the sketch into the footprint and leave sketch mode
# Asked by Discard changes when the sketch has edits.
editors-sketch-discard-confirm = Discard the changes made in this sketch?
editors-sketch-keep-editing = Keep editing
# The bake preview card.
editors-sketch-bake-result = Bake result
editors-sketch-bake-never = Construction and centerline entities never bake.
# The preview could not bake the sketch; $reason says why.
editors-sketch-bake-refused = The sketch does not bake: { $reason }
editors-sketch-target-pads = Pads
editors-sketch-target-silk-top = Top silk
editors-sketch-target-silk-bottom = Bottom silk
editors-sketch-target-courtyard = Courtyard
editors-sketch-target-mask-openings = Mask openings
editors-sketch-target-mask-excludes = Mask excludes
editors-sketch-target-paste-apertures = Paste apertures
editors-sketch-target-pours = Pours
editors-sketch-target-keepouts = Keepouts
editors-sketch-target-cutouts = Cutouts
editors-sketch-target-v-scores = V-scores
editors-sketch-target-board-outline = Outline
editors-sketch-target-mounting-holes = Holes
# A count in the bake preview; $count is a whole number.
editors-sketch-bake-count = { $count }
# The question after an edit of a patterned pad (mockup row 192, item D1); $designator is
# the pad's number.
editors-pad-scope-question = Pad { $designator } is one of a pattern's pads. Apply the change to:
editors-pad-scope-this-pad = This pad
editors-pad-scope-all-pads = All pads
editors-pad-scope-cancel = Cancel
# Sketch ▸ Create rows after Point (mockup row 187): two arcs joined by two lines,
# and a regular polygon.
editors-sketch-tool-slot = Slot
editors-sketch-tool-polygon = Polygon
# The board roles of decision 2 (2026-11-20), in the board's role picker.
editors-sketch-role-slot = Slot
editors-sketch-role-board-pour = Pour
editors-sketch-role-mechanical-layer = Mechanical layer
editors-sketch-role-component-anchor = Component anchor
# Bake preview rows of a board sketch.
editors-sketch-target-mechanical = Mechanical
editors-sketch-target-component-anchors = Anchors
# The footprint editor bar's mode switch (mockup row 187): a two-state segment and a
# 3D toggle beside it.
editors-mode-footprint = Footprint
editors-mode-sketch = Sketch
editors-mode-3d = 3D
editors-mode-3d-tooltip = Show the 3D body; click again to go back
