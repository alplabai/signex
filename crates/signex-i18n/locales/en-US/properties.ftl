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
# The muted line beside a selected wire's title. $net is the net's name.
properties-wire-net-line = net { $net }
# Button beside a wire's read-only Net Name: goes to what names the net.
properties-net-go-to-label = Go to Label
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
# The Alternate segment of the Body control; its tooltip has the full name.
properties-body-alternate-short = Alternate
# A power port's typed name could not be applied because the port is gone.
properties-port-name-not-applied = Properties: the name typed for a power port on { $sheet } was not applied, the port is no longer there. The name was: { $name }
# Renaming a power port renames every port of that name in the project.
properties-rail-renamed = Properties: renamed the power rail { $old } to { $new } on { $sheets } sheets; save to keep it.
properties-rail-rename-waits = Properties: the power rail name { $name } is applied once the project's sheets are read, so this save does not include it. Save again to keep it.
properties-rail-rename-refused = Properties: the power rail { $old } was not renamed to { $new } on any sheet: { $why }
properties-rail-why-empty = a rail needs a name.
properties-rail-why-port-gone = the port changed or is no longer there.
properties-rail-why-stale = the project's sheets kept changing while they were read. Try again.
properties-rail-why-read-failed = the sheets could not be read ({ $path }).
properties-rail-why-unreadable = { $path } could not be read, so it may hold a port of this rail.
properties-rail-why-shared = { $path } is also part of another open project, whose rail would split.
properties-rail-why-busy = { $path } is being saved or moved.
properties-rail-why-read-only = { $path } opens read-only.
properties-rail-why-not-loaded = { $path } could not be loaded.
properties-rail-why-not-changed = { $path } refused the change; a locked port holds the rail, or the sheet could not be edited.

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
properties-color-presets = Preset Colors
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

# Bezier and dimension tools (#1267): the hint bar at the canvas's bottom-left,
# after the tool's name.
properties-annotation-hint-bezier-start = click the start point
properties-annotation-hint-bezier-c1 = click control point C1
properties-annotation-hint-bezier-c2 = click control point C2
properties-annotation-hint-bezier-end = click the end point · Esc cancels
properties-annotation-hint-dimension-first = click the first point
properties-annotation-hint-dimension-second = click the second point · Tab edits value · Space flips side
properties-annotation-hint-dimension-line = click to place the dimension line · Tab edits value · Space flips side · Esc cancels
properties-annotation-section-points = Points
properties-annotation-section-line = Line
properties-annotation-mm = mm
properties-annotation-x = X
properties-annotation-y = Y
properties-annotation-width = Width
properties-annotation-style = Style
properties-annotation-style-solid = Solid
properties-annotation-style-dash = Dash
properties-annotation-style-dot = Dot
properties-annotation-colour = Color
# A drawing with no colour of its own takes the sheet's drawing colour.
properties-annotation-colour-sheet = Sheet drawing color
properties-annotation-colour-own = Own color
properties-annotation-ends = Ends
properties-annotation-ends-none = None
properties-annotation-ends-start = Start
properties-annotation-ends-end = End
properties-annotation-ends-both = Both
properties-bezier-title = Bezier
properties-bezier-caption = cubic · 4 points
properties-bezier-start = Start
properties-bezier-control-1 = Control 1
properties-bezier-control-2 = Control 2
properties-bezier-end = End
# The control points' names on the illustration.
properties-bezier-c1 = C1
properties-bezier-c2 = C2
properties-bezier-note = The drawing above only illustrates; the fields edit.
properties-dimension-linear = Linear dimension
properties-dimension-aligned = Aligned dimension
properties-dimension-tool-caption = placing · tool properties
properties-dimension-section-measure = Measure
properties-dimension-section-text = Text
properties-dimension-from = From
properties-dimension-to = To
properties-dimension-length = Length
properties-dimension-direction = Direction
properties-dimension-horizontal = Horizontal
properties-dimension-vertical = Vertical
properties-dimension-aligned-direction = Aligned
properties-dimension-units = Units
properties-dimension-unit-mm = mm
properties-dimension-unit-mil = mil
properties-dimension-unit-in = in
properties-dimension-precision = Precision
# The precision choices show how a value is written.
properties-dimension-precision-0 = 0
properties-dimension-precision-1 = 0.0
properties-dimension-precision-2 = 0.00
properties-dimension-precision-3 = 0.000
properties-dimension-arrows = Arrows
properties-dimension-arrows-inside = Filled, inside
properties-dimension-arrows-outside = Filled, outside
properties-dimension-arrows-open = Open
properties-dimension-override = Override text
properties-dimension-override-text = Text
properties-table-title = Table
properties-table-caption =
    { $rows ->
        [one] 1 row
       *[other] { $rows } rows
    } · { $columns ->
        [one] 1 column
       *[other] { $columns } columns
    }
properties-table-section-source = Data source
properties-table-section-columns = Columns
properties-table-section-style = Style
properties-table-source = Source
properties-table-source-pins = Component pins
properties-table-source-typed = Typed / CSV
properties-table-source-sheets = Project sheets
properties-table-component = Component
properties-table-component-none = Pick a part
properties-table-sort = Sort
properties-table-sort-number = Pin number
properties-table-sort-name = Pin name
properties-table-refresh = Refresh
properties-table-refresh-on-compile = On compile
properties-table-refresh-off = Off
properties-table-csv = CSV
properties-table-csv-import = Import…
properties-table-csv-export = Export…
properties-table-channels = Channels
properties-table-per-channel = One row per channel
properties-table-per-sheet = One row per sheet
properties-table-text-size = Text size
properties-table-cell-padding = Cell padding
properties-table-borders = Borders
properties-table-borders-all = All
properties-table-borders-outer = Outer
properties-table-borders-none = None
properties-table-header-fill = Header fill
properties-table-header-colour = Color
properties-table-header-colour-note = sheet style · info tint
properties-table-source-missing = The part is not on this sheet any more; the rows stay as they were.
# A column checkbox whose column has no title yet. $index counts from 1.
properties-table-untitled-column = Column { $index }
properties-table-csv-import-title = Import table from CSV
properties-table-csv-export-title = Export table as CSV
properties-table-csv-import-failed = Table import: { $file } was not read: { $why }
properties-table-csv-exported = Table exported: { $file }
properties-table-csv-export-failed = Table export: { $file } was not written: { $why }
properties-table-toc-no-project = Add Table of Contents Sheet: the sheet belongs to no project
properties-table-toc-written = Contents sheet written: { $file }
properties-table-toc-exists = Add Table of Contents Sheet: { $file } is there already and was kept
properties-table-toc-failed = Add Table of Contents Sheet: { $file } was not written: { $why }
properties-table-toc-not-serialised = Add Table of Contents Sheet: the contents sheet could not be written as text
# The word after the project's name in the contents sheet's heading
# ("dcdc-buck · Contents"). No placeable: the heading is sheet text, so it
# must not carry the bidi isolation marks a placeable gets.
properties-table-toc-heading = Contents
# $file is the sheet the bound tables are on; $reason is why the edit was refused.
properties-table-refresh-failed = Table refresh failed on { $file }: { $reason }
# The $reason above when the sheet gave none.
properties-table-refresh-no-reason = the sheet refused the edit
# A CSV import or cell edit answered after its table left the sheet. $file is the sheet.
properties-table-csv-table-gone = Table import: the table is no longer on { $file }; nothing was changed

## Multi-channel sheets and the Compiled Design panel (#1279, #1308).

# The sheet symbol's designator row (it was "Name").
properties-sheet-symbol-designator = Designator
properties-sheet-symbol-title = Sheet Symbol
# Caption under the title: the sheet file, then whether it is repeated.
properties-sheet-symbol-repeated = { $file } · repeated ×{ $count }
properties-sheet-symbol-one-sheet = { $file } · one sheet
properties-sheet-symbol-width = Width
properties-sheet-symbol-height = Height
# The section whose header is the Repeat switch.
properties-sheet-symbol-repeat = Repeat
properties-sheet-symbol-repeat-tip = Repeat this sheet
properties-sheet-symbol-channel-name = Channel name
properties-sheet-symbol-from-to = From / To
# The channels a repeat compiles to. $names lists them (CH1, CH2, CH3, CH4).
properties-sheet-symbol-channels =
    { $count ->
        [one] 1 channel: { $names }
       *[other] { $count } channels: { $names }
    }
properties-sheet-symbol-channel-naming = Channel naming
properties-sheet-symbol-designators = Designators
properties-sheet-symbol-local-nets = Local nets
# The live preview of the naming formats: R5 → CH1_R5 · VOUT_3V3 → VOUT_3V3_CH1.
properties-sheet-symbol-naming-preview = { $designator } → { $channel-designator } · { $net } → { $channel-net }
properties-sheet-symbol-entries = Entries
properties-sheet-symbol-entry = Entry
properties-sheet-symbol-per-channel = Per channel
# A plain entry: every channel shares its net.
properties-sheet-symbol-shared = shared
# The pill of a Repeat(…) entry: one net per channel.
properties-sheet-symbol-repeat-pill = Repeat
# The naming format menus.
properties-channel-naming-designator-format = Designator format
properties-channel-naming-net-format = Local net format
properties-channel-naming-custom = Custom format…
# The token help line; each variable is a token as typed.
properties-channel-naming-tokens = Tokens: { $channel } (CH1), { $index } (1), { $alpha } (A), { $base } ({ $example }).
properties-channel-naming-error-empty = Type a format.
properties-channel-naming-error-whitespace = A format cannot contain spaces.
properties-channel-naming-error-unclosed = A token is not closed with a brace.
properties-channel-naming-error-unknown = Unknown token { $token }.
properties-channel-naming-error-no-channel = Add a channel token, or every channel gets the same name.
properties-channel-naming-error-no-base = Add the designator or net token.
# The Properties header of a channel tab. $channel is CH1.
properties-channel-title = Channel { $channel }
# $index is the channel's position, $count the number of channels.
properties-channel-caption = { $file } · { $index } of { $count } · read-only view
properties-channel-designator-map = Designator map
properties-channel-logical = Logical

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
# $down and $across are the sheet's zone counts.
properties-illustration-zones = Zones { $down } × { $across }
properties-illustration-title-block = Title Block
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
# Why Open Sheet is disabled, in Properties, the context menu and the log.
properties-sheet-open-missing = The sheet file is missing, so there is nothing to open.
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
properties-fill-outline-tip = Filled with the border color.
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
properties-kind-power-ports = Power Ports
# Shown with the Power Ports chip on: a port's value is its net name.
properties-multi-power-note = Power ports have no fields to edit together, because a port's name is its net.
properties-count-power-ports = { $count ->
    [one] 1 power port
   *[other] { $count } power ports
}
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
properties-count-harness-connectors = { $count ->
    [one] 1 harness connector
   *[other] { $count } harness connectors
}
properties-count-harness-entries = { $count ->
    [one] 1 harness entry
   *[other] { $count } harness entries
}
properties-count-harness-wires = { $count ->
    [one] 1 signal harness
   *[other] { $count } signal harnesses
}
