# Properties panel: the Library rows of a placed library part (#2209).

# Button beside "Update from library": update only the selected part, leaving
# the other instances of the library symbol on the version they have.
properties-library-only-this-one = Only this one

# Tooltip of the "Only this one" button.
properties-library-only-this-one-tip = Updates only this part; the others keep their version

# Note beside the Update buttons: how many placed instances draw with an older
# version of the library symbol. $count is a whole number of instances.
properties-library-older-instances =
    { $count ->
        [one] Library: { $count } instance uses an older version
       *[other] Library: { $count } instances use an older version
    }

# Note beside the Update buttons when the library symbol changed but the sheet
# cannot say how many instances are behind.
properties-library-out-of-date = Library: out of date

# Shown when the placed part's library symbol no longer exists in its library.
properties-library-symbol-missing = Library: symbol missing (sheet keeps its embedded copy)

# Button: replace the part's embedded drawing with the library's current one.
properties-library-update = Update from library

# Tooltip of the "Update from library" button.
properties-library-update-tip = Updates every instance of this part on the sheet

## Section titles (shown upper-cased) (#2059)

properties-section-general = General
properties-section-graphical = Graphical
properties-section-style = Style
properties-section-state = State
properties-section-basic = Basic Properties
properties-section-text = Text Properties
properties-section-footprint = Footprint
properties-section-page-options = Page Options
properties-section-selection-filters = Custom Selection Filters

## Row labels

properties-position = Position
properties-size = Size
properties-name = Name
properties-net-name = Net Name
properties-port-name = Port Name
properties-sheet-name = Sheet Name
properties-text = Text
properties-value = Value
properties-designator = Designator
properties-description = Description
properties-datasheet = Datasheet
properties-font = Font
# Font pick-list entry: no family of its own, the sheet default applies.
properties-font-default = Default
properties-font-size = Font Size
properties-width-mm = Width (mm)
properties-net-class = Net Class
properties-style = Style
properties-locked = Locked
properties-mirror-x = Mirror X
properties-mirror-y = Mirror Y
# Do Not Place.
properties-dnp = DNP
properties-vertices = Vertices
# Which field of a part is selected: its Reference or its Value.
properties-field = Field
properties-field-reference = Reference
properties-field-value = Value
properties-visible = Visible
properties-fields-autoplaced = Fields Autoplaced
properties-justify-h = Justify H
properties-justify-v = Justify V
properties-text-size = Text Size
properties-yes = Yes
properties-no = No

## Pins of a placed part

properties-net = Net
properties-hidden = Hidden
# The clicked pin's own section. $pin is the pin's designator.
properties-pin-title = Pin { $pin }
properties-pins-column-pin = Pin
properties-pins-column-name = Name
properties-pins-column-type = Type
properties-pins-column-net = Net

## Library rows of a placed part

properties-symbol = Symbol
properties-footprint = Footprint
properties-library = Library
properties-action-open-datasheet = Open Datasheet
properties-action-edit-symbol = Edit Symbol
properties-action-change-component = Change Component…
# The part's body: the normal drawing or the alternate (De Morgan) one.
properties-body = Body
properties-body-normal = Normal
properties-body-alternate = Alternate (De Morgan)
properties-body-alternate-missing = Alternate (not in symbol; drawn as Normal)
properties-body-no-alternate-tip = This library symbol has no alternate (De Morgan) body

## Footprint card

# $n is the number of pads, for the plural form; $count is the same number
# as text, written without grouping.
properties-footprint-pads =
    { $n ->
        [one] { $count } pad
       *[other] { $count } pads
    }
# $pitch is the pad pitch in mm; $pads is the pad count line above.
properties-footprint-pitch-pads = { $pitch } mm pitch, { $pads }
properties-footprint-3d-body = 3D body attached
properties-footprint-reading = Reading footprint…
# $reason is the library reader's reason, in English.
properties-footprint-unreadable = Footprint could not be read: { $reason }
properties-footprint-unlinked = Not linked to a library footprint

## Power port

properties-power-style-bar = Bar
properties-power-style-arrow = Arrow
properties-power-style-wave = Wave
properties-power-style-circle = Circle
properties-power-style-power-ground = Power Ground
properties-power-style-signal-ground = Signal Ground
properties-power-style-earth = Earth
properties-power-style-chassis = Chassis
properties-physical-name = Physical Name
properties-power-net = Power Net
properties-high-speed = High Speed
properties-diff-pair = Differential Pair
# Differential Pair pick-list entry: the net is in no pair.
properties-diff-pair-none = None

## Parameter tables

properties-tab-all = All
properties-tab-parameters = Parameters
properties-tab-rules = Rules
properties-tab-classes = Classes
# Button that opens the add menu; the triangle marks the menu.
properties-net-params-add = Add ▾
properties-net-no-parameters = No Parameters

## Wire style and colours

properties-wire-width = Width
# Under a multi-selection's width presets: any width, typed in the panel's unit.
properties-wire-width-exact = Exact Width
properties-wire-width-smallest = Smallest
properties-wire-width-small = Small
properties-wire-width-medium = Medium
properties-wire-width-large = Large
properties-wire-color = Color
# Colour swatch caption: no colour of its own.
properties-color-default = Default
properties-color-presets = Preset Colours
properties-color-custom = Custom…
properties-color-reset = Reset to Default

## Document Options

# Unit toggle: millimetres.
properties-unit-mm = mm
# Unit toggle: thousandths of an inch.
properties-unit-mils = mils
properties-visible-grid = Visible Grid
properties-snap-grid = Snap Grid
properties-snap-to-hotspots = Snap to Hotspots
properties-canvas-font = Canvas Font
properties-sheet-color = Sheet Color
properties-sheet-color-default = Default
properties-sheet-color-black = Black
properties-sheet-color-white = White
properties-sheet-color-dark-gray = Dark Gray
properties-sheet-color-light-gray = Light Gray
properties-sheet-color-cream = Cream
# A sheet colour from the file that is no preset. $hex is its code as
# #rrggbbaa (lowercase, eight digits).
properties-sheet-color-custom = Custom ({ $hex })
properties-format-template = Template
properties-format-standard = Standard
properties-format-custom = Custom
properties-paper = Paper
properties-orientation = Orientation
properties-orientation-landscape = Landscape
properties-orientation-portrait = Portrait
properties-show-border = Show Border
properties-show-title-block = Show Title Block
properties-origin = Origin
properties-origin-upper-left = Upper Left
properties-origin-lower-left = Lower Left
properties-default-font = Default Font

## Title block fields

properties-title-block-title = Title
properties-title-block-document-number = Document No.
properties-title-block-revision = Revision
properties-title-block-date = Date
properties-title-block-company = Company
properties-title-block-drawn-by = Drawn By
properties-title-block-checked-by = Checked By
properties-title-block-approved-by = Approved By
properties-title-block-hint = Press Enter to apply a field.
properties-sheet-variable-remove = Remove
# What a variable's `=` expression evaluates to. $value is the result, such
# as 1.65V.
properties-sheet-variable-shows = Shows { $value }
# Why a variable's `=` expression does not evaluate. $error is the reason.
properties-sheet-variable-error = Error: { $error }
# The read-only rows of Document Options, Parameters: special strings of the
# sheet that nothing in the panel edits.
properties-parameter-document-name = Document Name
properties-parameter-sheet-name = Sheet Name
properties-parameter-sheet-number = Sheet Number
properties-parameter-sheet-count = Sheet Count
properties-parameter-project-name = Project Name
properties-parameter-current-time = Current Time
properties-parameter-current-date = Current Date
properties-parameter-time = Time
properties-parameter-document-full-path = Document Full Path
properties-parameter-modified-date = Modified Date
properties-title-block-author = Author
properties-title-block-engineer = Engineer
properties-title-block-organization = Organization
properties-title-block-address-1 = Address 1
properties-title-block-address-2 = Address 2
properties-title-block-address-3 = Address 3
properties-title-block-address-4 = Address 4
# The sheet leaves the Date field empty, so it shows today's date and
# stores nothing. $field is the field's name.
properties-title-block-today = { $field } (today)
# What a part value's `=` expression evaluates to. $value is the result,
# such as 20kΩ.
properties-value-shows = Shows { $value }
# Why a part value's `=` expression does not evaluate. $error is the reason.
properties-value-error = Error: { $error }

## Shared Properties rows (#2523)

# One step of the Rotation control. $degrees is 0, 90, 180 or 270.
properties-rotation-step = { $degrees }°
# The first point of a wire or a bus, an X / Y pair.
properties-start = Start
# Axis tag inside a coordinate field.
properties-axis-x = X
properties-axis-y = Y
# Width / height tag inside a size field.
properties-axis-width = W
properties-axis-height = H
# The unit shown at the right of a length field. $unit is the unit symbol
# (mm, mil, in, um); symbols are not translated.
properties-unit-suffix = { $unit }
# A Font Size entry. $size is the size in points.
properties-font-size-pt = { $size } pt
# Tooltips of the text style buttons.
properties-style-bold = Bold
properties-style-italic = Italic
properties-style-underline = Underline
properties-style-strike = Strike
# The text style buttons: one letter each, drawn in its own style.
properties-style-bold-glyph = B
properties-style-italic-glyph = I
properties-style-underline-glyph = U
properties-style-strike-glyph = T

## Document Options (#2531)

properties-format = Format
properties-zones = Zones
# Short axis name of the zones counted down the sides (A, B, C...).
properties-zones-v-short = V
# Short axis name of the zones counted along the top (1, 2, 3...).
properties-zones-h-short = H
# Under the zone counts when a typed count was over the limit. $max is the
# most zones a border has, $used the count stored instead.
properties-zones-clamped = Out of range (0 to { $max }): { $used } is used.
# Short name of the paper width field.
properties-paper-w-short = W
# Short name of the paper height field.
properties-paper-h-short = H
# A paper size in the Paper list. $name is the size (A4), $width and
# $height its millimetres at the sheet's orientation.
properties-paper-choice = { $name } · { $width } × { $height } mm
# Callouts of the Grid and snap drawing. $pitch is the grid pitch with its
# unit, such as 2.54 mm.
properties-illustration-visible-grid = Visible Grid { $pitch }
properties-illustration-snap-grid = Snap Grid { $pitch }
properties-illustration-hotspot = Hotspot
properties-illustration-origin = Origin
# Callouts of the Sheet drawing. $size is the paper edge in millimetres.
properties-illustration-paper-width = W { $size } mm
properties-illustration-paper-height = H { $size } mm
properties-parameters-filter = Filter parameters
properties-parameters-group-title-block = Title block
properties-parameters-group-sheet-variables = Sheet variables
properties-parameters-group-built-in = Built-in
properties-parameters-read-only = read-only
properties-parameters-read-only-tip = Read-only: set by Signex
# Pill on a title-block row whose value comes from the project.
properties-parameters-from-project = project
properties-parameters-from-project-tip = Inherited from the project; type to override
properties-parameters-not-set = Not set
properties-parameters-no-rules = No rules
properties-parameters-no-rules-body = This sheet has no rules.
properties-parameters-no-match = No parameter matches the filter.
properties-parameters-footer = A sheet variable replaces the project variable of the same name. A value starting with = is an expression.
properties-sheet-variable-add-row = Add variable
# Result pill of a sheet variable whose expression does not evaluate.
properties-sheet-variable-unresolved = ?

## Pre-placement form (Tab while a placement tool is armed, #2532)

# Header. $tool is the placement tool's name ("Net Label", "Wire").
properties-pre-placement-title = Place { $tool }
# Caption under the header, in pieces around the key caps:
# "Applies to the next object placed · Enter OK · Esc back to placing".
properties-pre-placement-applies = Applies to the next object placed
properties-pre-placement-separator = ·
properties-pre-placement-key-enter = Enter
properties-pre-placement-key-esc = Esc
properties-pre-placement-back-to-placing = back to placing
# Row label: where a wire starts (its end is "End").
properties-pre-placement-start = Start
# Under the wire illustration.
properties-pre-placement-length-derived = Length is derived from Start and End
# Card under Location of a tool with nothing else to set.
properties-pre-placement-no-options = No other options
properties-pre-placement-no-options-hint = Set the position above, or click on the sheet to place.
# The anchor the justification grid marks.
properties-anchor-top-left = Top left
properties-anchor-top = Top
properties-anchor-top-right = Top right
properties-anchor-left = Left
properties-anchor-center = Center
properties-anchor-right = Right
properties-anchor-bottom-left = Bottom left
properties-anchor-bottom = Bottom
properties-anchor-bottom-right = Bottom right

## Placed part, Graphite layout (mockup row 42)

# The free-text footprint field under the footprint pick list.
properties-override = Override
# Note under the Override field.
properties-override-note = A name typed here wins over the pick
# Between the library name and the library id on the Library row.
properties-library-separator = ·
# The header button that opens the placed part's action menu.
properties-part-more = ⋯
properties-part-more-tip = More actions
# Menu entry: put the part's library id on the clipboard.
properties-action-copy-library-id = Copy Library ID
# Callout of the part illustration. $x and $y are the position as the
# Location fields show it.
properties-part-illustration-position = Position { $x }, { $y }
# Corner note of the part illustration when the part is mirrored both ways.
properties-part-illustration-mirror-xy = Mirror X · Mirror Y

## Pin of a placed part (mockup row 50)

# Caption under the pin illustration.
properties-pin-read-only-note = Read-only on a placed part · Edit in Symbol Editor changes it
properties-pin-edit-in-symbol-editor = Edit in Symbol Editor
# Opens the Net Inspector on the pin's net.
properties-pin-show-net = Show net
properties-pin-type = Type
properties-pin-length = Length
# $length is the pin length in mm.
properties-pin-length-value = { $length } mm
properties-pin-orientation = Orientation
properties-pin-orientation-right = Right
properties-pin-orientation-up = Up
properties-pin-orientation-left = Left
properties-pin-orientation-down = Down
# Leader of the pin illustration. $type is the pin's electrical type.
properties-pin-illustration-type = Type · { $type }
# Callout of the pin illustration. $length is the pin length in mm.
properties-pin-illustration-length = Length { $length } mm
# Tooltip of a Pins table row whose pin is hidden on the sheet.
properties-pin-hidden-tip = Hidden on the sheet
# The count pill of the Pins section header. $count is the number of pins.
properties-pins-count = { $count }
# Title of the Pins section; its count shows in a pill beside it.
properties-section-pins = Pins

## Single-selection headers and rows of wires, labels, ports and notes (#2523, g1)

properties-kind-wire = Wire
properties-kind-bus = Bus
properties-kind-net-label = Net Label
properties-kind-global-port = Global Port
properties-kind-hierarchical-port = Hierarchical Port
properties-kind-power-port = Power Port
properties-kind-text-note = Text Note
# Caption under a wire's or bus's header: Length is read-only.
properties-length-derived = Length is derived from Start and End
# A wire's or bus's end point: an X / Y pair row, as Start is.
properties-end = End
# Illustration callout at a wire's or bus's start point.
properties-callout-start = Start
# Short power-port style names under the glyph tiles; the tooltip has the full name.
properties-power-style-short-power-ground = Pwr GND
properties-power-style-short-signal-ground = Sig GND
# Tooltip on High Speed and Differential Pair while Power Net is on.
properties-power-net-excludes = A power net is neither high speed nor one side of a differential pair.
properties-justify-top = Top
properties-justify-middle = Middle
properties-justify-bottom = Bottom
# A power port's net, and the parameters, rules and classes on it.
properties-section-net = Net
properties-section-net-params = Net Parameters
properties-text-size-unit = Text Size ({ $unit })
# One step of a 4-step Rotation control, e.g. 90°.
properties-degrees-short = { $degrees }°

# Sheet symbol Properties (mockup row 46). "Open Sheet" does what double-clicking the
# symbol does.
properties-sheet-open = Open Sheet
properties-sheet-browse = Browse…
# Title of the file dialog that picks the file a sheet symbol points at.
properties-sheet-browse-title = Choose the sheet file
properties-sheet-create = Create Sheet…
properties-sheet-missing-title = No file for this sheet yet
# $file is the file the sheet symbol points at, such as debug.snxsch.
properties-sheet-missing-detail = { $file } is not in the project folder.
properties-sheet-border = Border
properties-sheet-fill = Fill
properties-sheet-line-width = Line Width
properties-section-entries = Entries
properties-sheet-entries-note = Entries come from the child sheet
properties-sheet-no-entries = No entries yet
# Directions of a sheet entry.
properties-entry-input = Input
properties-entry-output = Output
properties-entry-bidirectional = Bidirectional
properties-entry-tri-state = Tri-state
properties-entry-passive = Passive
properties-entry-unspecified = Unspecified
# Unit and axis marks inside a paired field (X 50.80 mm).
properties-unit-degrees = °
properties-unit-pt = pt
properties-axis-w = W
properties-axis-h = H
properties-angle-start = Start
properties-angle-end = End
# Drawing shape Properties (mockup row 48).
properties-drawing-position = Position
properties-drawing-size = Size
properties-drawing-center = Center
properties-drawing-start = Start
properties-drawing-end = End
properties-drawing-radius = Radius
properties-drawing-angles = Angles
properties-drawing-width = Width
properties-section-appearance = Appearance
properties-fill-solid = Solid
properties-fill-none-tip = No fill: only the border is drawn.
properties-fill-outline-tip = Filled with the border colour.
properties-fill-solid-tip = Filled with the sheet background, so it hides what is behind it.
# Directive and No-ERC Properties (mockup row 49). Rule names come from the ERC
# engine in English.
properties-suppressed-rules = Suppressed rules
# Header count of the rules a directive silences.
properties-rules-off = { $count } off
# $count is how many rules the ERC engine has.
properties-all-rules = All rules · { $count } rules
properties-rule-filter = Filter rules
properties-all-rules-on = All rules is on: every rule is silenced here. Untick it to pick rules.
# $query is the text typed in the rule filter.
properties-rule-no-match = No rule matches “{ $query }”.
# Multi-selection Properties (mockup row 51): the kind chips and sections.
properties-kind-all = All
properties-kind-parts = Parts
properties-kind-labels = Labels
properties-kind-wires = Wires
# $count is how many labels a click sets.
properties-multi-mixed-both = Justify and Rotation are mixed; a click sets all { $count }.
properties-multi-mixed-justify = Justify is mixed; a click sets all { $count }.
properties-multi-mixed-rotation = Rotation is mixed; a click sets all { $count }.
# Footer of a multi-selection. $kinds lists the counts, such as
# "9 parts, 3 labels, 2 wires".
properties-multi-footer = { $count } selected: { $kinds }
properties-multi-footer-plain = { $count } selected
properties-nothing-shared-title = Nothing to edit together
properties-nothing-shared-body = Nothing in this selection has fields shared for editing. Selected: { $kinds }
properties-count-parts = { $count ->
    [one] 1 part
   *[other] { $count } parts
}
properties-count-part-fields = { $count ->
    [one] 1 part field
   *[other] { $count } part fields
}
properties-count-labels = { $count ->
    [one] 1 label
   *[other] { $count } labels
}
properties-count-wires = { $count ->
    [one] 1 wire
   *[other] { $count } wires
}
properties-count-buses = { $count ->
    [one] 1 bus
   *[other] { $count } buses
}
properties-count-bus-entries = { $count ->
    [one] 1 bus entry
   *[other] { $count } bus entries
}
properties-count-junctions = { $count ->
    [one] 1 junction
   *[other] { $count } junctions
}
properties-count-no-connects = { $count ->
    [one] 1 no-connect
   *[other] { $count } no-connects
}
properties-count-no-erc = { $count ->
    [one] 1 No-ERC directive
   *[other] { $count } No-ERC directives
}
properties-count-sheet-entries = { $count ->
    [one] 1 sheet entry
   *[other] { $count } sheet entries
}
properties-count-text-notes = { $count ->
    [one] 1 text note
   *[other] { $count } text notes
}
properties-count-sheet-symbols = { $count ->
    [one] 1 sheet symbol
   *[other] { $count } sheet symbols
}
properties-count-drawings = { $count ->
    [one] 1 drawing
   *[other] { $count } drawings
}
properties-count-directives = { $count ->
    [one] 1 directive
   *[other] { $count } directives
}
