# Properties panel while components are selected on a board (#2354).

pcb-component-title = Component
# The selection summary beside the title.
pcb-component-count = { $count ->
    [one] 1 component
   *[other] { $count } components
}
# Placeholder of the field that filters the panel's rows by name.
pcb-component-search = Search properties
# The parameter count on the Parameters tab; $count is a number.
pcb-component-parameter-count = { $count }

# Section titles.
pcb-component-section-location = Location
pcb-component-section-properties = Properties
pcb-component-section-footprint = Footprint
pcb-component-section-swapping = Swapping Options
pcb-component-section-schematic = Schematic Reference Information

# Location rows; numbers are millimetres and degrees. X and Y name the
# undo steps; the Position row shows them as the short prefixes.
pcb-component-x = X (mm)
pcb-component-y = Y (mm)
pcb-component-position = Position
pcb-component-x-prefix = X
pcb-component-y-prefix = Y
pcb-component-rotation = Rotation
# Units shown after a field.
pcb-component-unit-mm = mm
pcb-component-unit-degrees = °
# A field whose value differs between the selected components.
pcb-component-mixed = Mixed
pcb-component-locked = Locked
pcb-component-locked-tip = A locked component cannot be moved, turned, flipped or edited until it is unlocked.

# Properties rows.
pcb-component-layer = Layer
pcb-component-layer-top = Top Layer
pcb-component-layer-bottom = Bottom Layer
pcb-component-comment = Comment
# The Comment field of several components whose comments differ.
pcb-component-comments-differ = Different comments
# The Designator field of several components: designators are edited one at a time.
pcb-component-one-at-a-time-short = One at a time
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
# The full name of the 3D Opacity row: its tooltip and its undo step.
pcb-component-body-opacity = 3D Body Opacity
pcb-component-opacity = 3D Opacity
# The undo steps of the two locks.
pcb-component-lock-primitives = Lock Primitives
pcb-component-lock-strings = Lock Strings
# The Lock row and its two checkboxes.
pcb-component-lock = Lock
pcb-component-lock-primitives-short = Primitives
pcb-component-lock-strings-short = Strings

# Footprint rows.
pcb-component-footprint-name = Footprint Name
# The button beside the footprint name that picks another footprint (#2366).
pcb-component-footprint-pick = Change…
pcb-component-footprint-pick-tip = Change the footprint: pick a library footprint for this component
# $library is the id of a library that is not open.
pcb-component-library-not-open = { $library } (not open)
pcb-component-library = Library
pcb-component-no-library = Not from a library

# Swapping Options rows.
pcb-component-pin-swap = Enable Pin Swapping
pcb-component-part-swap = Enable Part Swapping
# The Swapping Options header while both switches are off.
pcb-component-swapping-off = off

# Schematic Reference Information rows.
pcb-component-sch-designator = Designator
pcb-component-sch-path = Hier. Path
pcb-component-sch-path-tip = Hierarchical Path
pcb-component-channel-offset = Channel Offset
pcb-component-not-linked = Not linked to a schematic part
# The button of the designator mismatch note: runs Update PCB.
pcb-component-update-pcb = Update PCB

# The property illustration: leaders and the side note.
pcb-component-pin-one = Pin 1
pcb-component-courtyard = Courtyard
pcb-component-side-top = Top Layer
pcb-component-side-bottom = Bottom Layer · mirrored

# Tooltips of rows that cannot be edited yet.
pcb-component-one-at-a-time = Edit this on one component at a time.
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

# Placeholders of the fields of a parameter to add.
pcb-component-parameter-name = Name
pcb-component-parameter-value = Value
pcb-component-parameter-add = Add
# Undo step names; $key is the parameter's name.
pcb-component-parameter-added = Add parameter { $key }
pcb-component-parameter-removed = Remove parameter { $key }
pcb-component-parameter-remove-tip = Remove this parameter from the selected components

# Component types (#2365).
pcb-component-type-standard = Standard
pcb-component-type-standard-no-bom = Standard (No BOM)
pcb-component-type-mechanical = Mechanical
pcb-component-type-graphical = Graphical
pcb-component-type-net-tie = Net Tie (No BOM)
pcb-component-type-net-tie-in-bom = Net Tie (In BOM)
pcb-component-type-jumper = Jumper

# Tooltips of the component settings.
pcb-component-height-tip = The component's height above the board, in millimetres.
pcb-component-lock-primitives-tip = The pads and graphics of the component cannot be edited on their own.
pcb-component-lock-strings-tip = The texts of the component cannot be moved.

# Comment with several components selected; $count is how many.
pcb-component-comment-all = The selected components have different comments. Enter sets the typed comment on all { $count }.

# What the schematic says about the component (#2367).
pcb-component-sch-not-read = Open the schematic sheet to read this
pcb-component-source-not-open = No open library holds this part
# The note beside the Update PCB button. $schematic and $board are designators.
pcb-component-sch-mismatch = The schematic names this component { $schematic }, the board { $board }.
pcb-component-sch-mismatch-count = { $count ->
    [one] 1 component carries a designator the schematic does not.
   *[other] { $count } components carry a designator the schematic does not.
}
# The board footprint changed but the linked schematic part kept its own.
# $designator is the component.
pcb-component-swap-sheet-not-open = { $designator }: the board footprint changed, but the schematic was not updated because the part's sheet is not open
pcb-component-swap-sheet-read-only = { $designator }: the board footprint changed, but the schematic was not updated because the part's sheet is read-only
