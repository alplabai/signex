# Properties panel while components are selected on a board (#2354).

pcb-component-title = Component
# The selection summary beside the title.
pcb-component-count = { $count ->
    [one] 1 component
   *[other] { $count } components
}
# Placeholder of the field that filters the panel's rows by name.
pcb-component-search = Search properties

# Section titles.
pcb-component-section-location = Location
pcb-component-section-properties = Properties
pcb-component-section-footprint = Footprint
pcb-component-section-swapping = Swapping Options
pcb-component-section-schematic = Schematic Reference Information

# Location rows; numbers are millimetres and degrees.
pcb-component-x = X (mm)
pcb-component-y = Y (mm)
pcb-component-rotation = Rotation (°)
pcb-component-locked = Locked
pcb-component-locked-tip = A locked component cannot be moved, turned, flipped or edited until it is unlocked.

# Properties rows.
pcb-component-layer = Layer
pcb-component-layer-top = Top Layer
pcb-component-layer-bottom = Bottom Layer
pcb-component-comment = Comment
# Tooltips of the eye beside Designator and Comment.
pcb-component-text-shown = Shown on the board; click to hide
pcb-component-text-hidden = Hidden on the board; click to show
pcb-component-text-missing = This footprint has no text for this on the board.
pcb-component-area = Area
# $mm2 is an area in square millimetres.
pcb-component-area-value = { $mm2 } mm²
pcb-component-description = Description
pcb-component-type = Type
pcb-component-design-item-id = Design Item ID
pcb-component-source = Source
pcb-component-height = Height
pcb-component-body-opacity = 3D Body Opacity
pcb-component-lock-primitives = Lock Primitives
pcb-component-lock-strings = Lock Strings

# Footprint rows.
pcb-component-footprint-name = Footprint Name
# The button beside the footprint name that picks another footprint (#2366).
pcb-component-footprint-pick = …
pcb-component-footprint-pick-tip = Change the footprint: pick a library footprint for this component
# $library is the id of a library that is not open.
pcb-component-library-not-open = { $library } (not open)
pcb-component-library = Library
pcb-component-no-library = Not from a library

# Swapping Options rows.
pcb-component-pin-swap = Enable Pin Swapping
pcb-component-part-swap = Enable Part Swapping

# Schematic Reference Information rows.
pcb-component-sch-designator = Designator
pcb-component-sch-path = Hierarchical Path
pcb-component-channel-offset = Channel Offset
pcb-component-not-linked = Not linked to a schematic part

# Tooltips of rows that cannot be edited yet.
pcb-component-not-stored = The board does not store this yet.
pcb-component-needs-schematic = This is read from the schematic, which the board panel cannot read yet.
pcb-component-one-at-a-time = Edit this on one component at a time.
pcb-component-no-description = This component has no Description parameter.
pcb-component-read-only-tip = This board opened read-only; it cannot be edited.

# Parameters tab.
pcb-component-parameters-none = No parameters

# Changing the footprint of placed components (#2366).
# The undo step and the name in a refusal.
pcb-component-footprint-swap = Change Footprint
# The footprint picker's group of the footprints the component's own library part offers.
pcb-component-picker-own-footprints = This component's footprints ({ $count })
# An old pad whose number the new footprint lacks. $designator is the
# component, $pad the pad number, $net the net the pad was on.
pcb-component-swap-pad-net-dropped = { $designator }: pad { $pad } has no match on the new footprint and was removed with its net { $net }
pcb-component-swap-pad-dropped = { $designator }: pad { $pad } has no match on the new footprint and was removed
