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
properties-section-title-block = Title Block
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
