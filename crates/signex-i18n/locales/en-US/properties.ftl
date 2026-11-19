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
properties-section-font = Font
properties-section-basic = Basic Properties
properties-section-text = Text Properties
properties-section-footprint = Footprint
# The net a power port names.
properties-section-net-general = General (Net)
properties-section-net-parameters = Parameters (Net)
properties-section-parameters = Parameters
properties-section-page-options = Page Options
properties-section-selection-filters = Custom Selection Filters

## Row labels

# The X and Y of a position.
properties-xy = (X/Y)
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

properties-electrical-type = Electrical Type
properties-net = Net
properties-hidden = Hidden
# The clicked pin's own section. $pin is the pin's designator.
properties-pin-title = Pin { $pin }
# $count is how many pins the part has.
properties-pins-title = Pins ({ $count })
properties-pins-column-pin = Pin
properties-pins-column-name = Name
properties-pins-column-type = Type
properties-pins-column-net = Net
properties-pins-column-hidden = Hidden

## Library rows of a placed part

properties-symbol = Symbol
properties-footprint = Footprint
# The free-text footprint field under a footprint pick list.
properties-footprint-override = Footprint override
properties-library = Library
properties-library-id = Library ID
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
properties-net-params-name = Name
properties-net-params-value = Value
# Button that opens the add menu; the triangle marks the menu.
properties-net-params-add = Add ▾
properties-net-no-parameters = No Parameters
properties-net-no-rules = No Rules
properties-net-no-classes = No Classes

## Wire style and colours

properties-wire-width = Width
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
properties-border-colour = Border Colour
properties-fill-colour = Fill Colour

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
properties-paper-width = Width
properties-paper-height = Height
# $width and $height are the page size in whole mm.
properties-paper-dimensions = Width: { $width }mm  Height: { $height }mm
properties-orientation = Orientation
properties-orientation-landscape = Landscape
properties-orientation-portrait = Portrait
properties-show-border = Show Border
properties-show-title-block = Show Title Block
# Number of border zones down the page.
properties-zones-vertical = Vertical
# Number of border zones across the page.
properties-zones-horizontal = Horizontal
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
# The sheet leaves the field empty, so the project's value shows. $field is
# the field's name.
properties-title-block-from-project = { $field } (project)
# The sheet's own value replaces the project's. $field is the field's name.
properties-title-block-overrides-project = { $field } (overrides project)
properties-title-block-hint = Press Enter to apply a field.
properties-section-sheet-variables = Sheet Variables
properties-sheet-variable-add = Add
properties-sheet-variable-remove = Remove
# What a variable's `=` expression evaluates to. $value is the result, such
# as 1.65V.
properties-sheet-variable-shows = Shows { $value }
# Why a variable's `=` expression does not evaluate. $error is the reason.
properties-sheet-variable-error = Error: { $error }
properties-sheet-variables-hint = A sheet variable replaces the project variable of the same name. A value starting with = is an expression. Press Enter to apply.
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

## c13 sch-drawing-tables
# Bezier and dimension tools (#1267): the hint bar at the canvas's bottom-left,
# after the tool's name.
properties-annotation-hint-bezier-start = click the start point
properties-annotation-hint-bezier-c1 = click control point C1
properties-annotation-hint-bezier-c2 = click control point C2
properties-annotation-hint-bezier-end = click the end point · Esc cancels
properties-annotation-hint-dimension-first = click the first point
properties-annotation-hint-dimension-second = click the second point
properties-annotation-hint-dimension-line = click to place the dimension line · Esc cancels
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
properties-annotation-colour = Colour
# A drawing with no colour of its own takes the sheet's drawing colour.
properties-annotation-colour-sheet = Sheet drawing colour
properties-annotation-colour-own = Own colour
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
properties-table-header-colour = Colour
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

## c13 multi-channel-compiled

# The sheet symbol's designator row (it was "Name").
properties-sheet-symbol-designator = Designator
properties-sheet-symbol-title = Sheet Symbol
# Caption under the title: the sheet file, then whether it is repeated.
properties-sheet-symbol-repeated = { $file } · repeated ×{ $count }
properties-sheet-symbol-one-sheet = { $file } · one sheet
properties-sheet-symbol-section = Sheet
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
