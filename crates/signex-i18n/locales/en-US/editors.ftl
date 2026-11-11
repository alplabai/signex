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
editors-symbol-status-hint-selected = Del removes · drag to move
editors-symbol-status-hint-idle = Ctrl+Z undo · Ctrl+Y redo · scroll zooms · right-drag pans · Home fits
editors-symbol-copy-again = Copy Again

## Graphite views (#2225)
editors-component-read-only-footer = Read-only library - this component cannot be changed.
editors-component-unsaved-marker = • unsaved` (U+2022)
editors-component-reload-confirm = Discard your edits and reload?
