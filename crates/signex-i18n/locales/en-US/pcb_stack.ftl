# The Layer Stack manager of a board (#2401, mockup row 105): a document tab
# with one table row per layer of the physical stack, top to bottom.

pcb-stack-title = Layer Stack
# The three-letter kind code on the document tab.
pcb-stack-tab-kind = STK
pcb-stack-no-board-title = No board
pcb-stack-no-board = Open a board to see its layer stack.
# $value is already formatted; $unit is mm, µm or mil.
pcb-stack-total = Total thickness { $value } { $unit }
pcb-stack-copper-count = { $count ->
    [one] 1 copper layer
   *[other] { $count } copper layers
}
pcb-stack-row-count = { $count ->
    [one] 1 row
   *[other] { $count } rows
}
# $copper and $rows are the two counts above.
pcb-stack-footer = { $copper } · { $rows }
pcb-stack-footer-note = Changes apply to the board when you save the board. Undo works per edit.
pcb-stack-derived = Made from the board's layers; the first edit saves it with the board.
pcb-stack-read-only = This board opened read-only (a newer format), so its stack cannot be edited.
pcb-stack-enter-hint = Press Enter to apply a typed value.

## Sub-tabs.
pcb-stack-sub-stack = Stack
pcb-stack-sub-drills = Via Types / Drill Pairs

## Columns. $unit is mm, µm or mil.
pcb-stack-col-name = Name
pcb-stack-col-type = Type
pcb-stack-col-material = Material
pcb-stack-col-thickness = Thickness ({ $unit })
pcb-stack-col-er = Er
pcb-stack-col-weight = Weight (oz)
# A cell that does not apply to the row's kind.
pcb-stack-none = —

## Row kinds.
pcb-stack-kind-signal = Signal
pcb-stack-kind-plane = Plane
pcb-stack-kind-dielectric = Dielectric
pcb-stack-kind-solder-mask = Solder Mask
pcb-stack-kind-paste = Paste
pcb-stack-kind-overlay = Silkscreen

## Toolbar.
pcb-stack-stackup = Stackup
pcb-stack-add = Add
pcb-stack-remove = Remove
pcb-stack-move-up = Move Up
pcb-stack-move-down = Move Down
pcb-stack-select-tip = Select a row first.
pcb-stack-add-tip = The new layer goes below the selected row, between the outer copper layers.
pcb-stack-remove-tip = A copper layer takes a dielectric with it; a layer with items on it stays.
pcb-stack-presets = Presets
pcb-stack-presets-unavailable = Presets come with the stack templates.
pcb-stack-import = Import...
pcb-stack-import-unavailable = Importing a stackup is not available yet.
pcb-stack-export = Export
pcb-stack-export-unavailable = Exporting a stackup is not available yet.
pcb-stack-symmetric = Symmetric stack
pcb-stack-symmetric-tip = Shows whether the stack mirrors about its centre line: dielectric material and thickness, and copper weight.
pcb-stack-unit-mm = mm
pcb-stack-unit-um = µm
pcb-stack-unit-mil = mil
pcb-stack-fab-profile = Fab profile
pcb-stack-fab-open = Open Rules
pcb-stack-fab-tip = The fab profile is set in the project's Rules.

## Cross-section and preview.
pcb-stack-section = Cross-section
pcb-stack-section-hint = to scale · paste and silkscreen drawn as marks
pcb-stack-preview = 3D preview
pcb-stack-preview-hint = follows the stack

## Via Types / Drill Pairs.
pcb-stack-drill-pairs = Drill pairs
pcb-stack-drill-pairs-hint = from the board's vias · depth through the stack
pcb-stack-via-types = Via types
pcb-stack-via-types-hint = the types the Via panel picks from
pcb-stack-col-from = From
pcb-stack-col-to = To
pcb-stack-col-kind = Kind
pcb-stack-col-depth = Depth ({ $unit })
pcb-stack-col-min-drill = Min drill ({ $unit })
pcb-stack-col-aspect = Aspect ratio
pcb-stack-col-vias = Vias
pcb-stack-col-pair = Drill pair
pcb-stack-col-drill = Drill ({ $unit })
pcb-stack-col-finished = Finished hole ({ $unit })
pcb-stack-col-pad = Pad ({ $unit })
pcb-stack-col-protection = Protection (IPC-4761)
pcb-stack-col-used = Used
# $value is already formatted.
pcb-stack-aspect-value = { $value } : 1

## Undo label and notices.
pcb-stack-edit = Layer Stack
# $why is the reason.
pcb-stack-refused = The layer stack was not changed: { $why }
pcb-stack-not-a-number = "{ $text }" is not a number.
