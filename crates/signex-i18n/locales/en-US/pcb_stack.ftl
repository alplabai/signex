# The Layer Stack manager of a board (#2401): one table row per layer of the
# physical stack, top to bottom.

pcb-stack-title = Layer Stack
pcb-stack-no-board-title = No board
pcb-stack-no-board = Open a board to see its layer stack.
# $value is already formatted.
pcb-stack-total = Total thickness { $value } mm
pcb-stack-copper-count = { $count ->
    [one] 1 copper layer
   *[other] { $count } copper layers
}
pcb-stack-derived = Made from the board's layers; the first edit saves it with the board.
pcb-stack-read-only = This board opened read-only (a newer format), so its stack cannot be edited.
pcb-stack-enter-hint = Press Enter to apply a typed value.

## Columns.
pcb-stack-col-name = Name
pcb-stack-col-type = Type
pcb-stack-col-material = Material
pcb-stack-col-thickness = Thickness (mm)
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

## Buttons.
pcb-stack-add-signal = Add Signal
pcb-stack-add-plane = Add Plane
pcb-stack-add-dielectric = Add Dielectric
pcb-stack-remove = Remove
pcb-stack-move-up = Move Up
pcb-stack-move-down = Move Down
pcb-stack-select-tip = Select a row first.
pcb-stack-add-tip = The new layer goes below the selected row, between the outer copper layers.
pcb-stack-remove-tip = A copper layer takes a dielectric with it; a layer with items on it stays.

## Undo label and notices.
pcb-stack-edit = Layer Stack
# $why is the reason.
pcb-stack-refused = The layer stack was not changed: { $why }
pcb-stack-not-a-number = "{ $text }" is not a number.
