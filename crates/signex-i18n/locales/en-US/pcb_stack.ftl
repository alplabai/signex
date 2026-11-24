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

## Columns (mockup row 105). $unit is mm, µm or mil.
# The copper place of a row, 1 at the top.
pcb-stack-col-number = #
pcb-stack-col-name = Name
pcb-stack-col-type = Type
pcb-stack-col-material = Material
pcb-stack-col-thickness = Thickness ({ $unit })
pcb-stack-col-weight = Weight (oz)
pcb-stack-col-dk = Dk
pcb-stack-col-dk-tip = Dielectric constant of the layer.
pcb-stack-col-df = Df
pcb-stack-col-df-tip = Loss tangent of the layer. The stack does not store one yet.
pcb-stack-col-net = Net
pcb-stack-col-net-tip = The nets poured on a plane layer.
pcb-stack-col-copper = Cu %
pcb-stack-col-copper-tip = Copper coverage of the layer. It is not measured yet.
pcb-stack-col-items = Items
pcb-stack-col-items-tip = Items on the layer. A layer with items on it cannot be removed.
pcb-stack-col-drill-pairs = Drill pairs
pcb-stack-col-drill-pairs-tip = Drill pairs that start or end on this layer.
# A cell that does not apply to the row's kind.
pcb-stack-none = —
# The Net cell of a signal layer.
pcb-stack-net-signal = signal
# A drill pair's copper places, as in "1–4".
pcb-stack-span = { $from }–{ $to }

## Under the table: the drill pair legend and the layer pairs.
pcb-stack-no-drill-pairs = No drill pairs: the board has no vias yet.
pcb-stack-edit-hint = Click a row's number to select it, a cell to edit it. Enter applies the typed value.
pcb-stack-layer-pairs = Layer pairs
pcb-stack-layer-pairs-hint = signal → reference plane
# $signal and $plane are layer names.
pcb-stack-layer-pair = { $signal } → { $plane }
pcb-stack-no-plane = No plane layer to reference.

## Row kinds.
pcb-stack-kind-signal = Signal
pcb-stack-kind-plane = Plane
pcb-stack-kind-dielectric = Dielectric
pcb-stack-kind-solder-mask = Solder Mask
pcb-stack-kind-paste = Paste
pcb-stack-kind-overlay = Silkscreen

## Toolbar.
pcb-stack-stackup = Stackup
pcb-stack-name-tip = The stackup's name, saved with the board. Press Enter to apply; empty shows the board's file name.
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
# The Fab profile link when the project names no profile.
pcb-stack-fab-none = None set
pcb-stack-fab-no-project = No project
pcb-stack-fab-tip = The project's fab profile. Click to open the project's Rules, where it is set.
pcb-stack-fab-no-project-tip = A fab profile belongs to a project; this board is not in one.

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

# layer-stack-materials
## The Materials and Finish sub-tabs (mockup rows 176 and 177).
pcb-stack-sub-materials = Materials
pcb-stack-sub-finish = Finish
pcb-stack-material-library = Material library
pcb-stack-material-library-hint = Dk and Df at 1 GHz · a layer keeps a copy of the values it uses
pcb-stack-col-source = Source
pcb-stack-material-built-in = Built-in
pcb-stack-material-user = My library
pcb-stack-material-kind-core = Core
pcb-stack-material-kind-prepreg = Prepreg
pcb-stack-material-kind-copper = Copper
pcb-stack-material-kind-solder-mask = Solder mask
pcb-stack-material-use = Use for selected layer
pcb-stack-material-use-tip = Copies the material's thickness, Dk and Df into the layer selected in Stack. The board keeps the copy, so it opens the same anywhere. Undo works per edit.
pcb-stack-material-use-unavailable = Select a material, and a layer it fits in Stack.
# $name is the layer's name.
pcb-stack-material-layer = Selected layer: { $name }
pcb-stack-material-no-layer = No layer selected: pick one in Stack.
pcb-stack-material-name = Name
pcb-stack-material-add = Add material
pcb-stack-material-add-tip = Saves the material in your library in the config folder, for every board.
pcb-stack-material-remove = Remove
pcb-stack-material-remove-tip = Removes the selected material from your library. Boards keep the values they copied.
pcb-stack-material-reading = Reading your material library…
# $why is the reason.
pcb-stack-material-read-failed = Your material library could not be read, so it is not changed: { $why }
# $why is the reason.
pcb-stack-material-write-failed = Your material library was not saved: { $why }
pcb-stack-material-user-empty = Your library is empty. A material you add here is offered on every board.
# $why is the reason.
pcb-stack-material-refused = The material was not added: { $why }
pcb-stack-material-flex-note = Flex and rigid-flex materials (polyimide, adhesives, coverlay) are out of scope for this library.
pcb-stack-board-materials = Board materials
pcb-stack-board-materials-hint = the values the stack's layers store
pcb-stack-board-materials-none = No layer names a material yet.
pcb-stack-finish-mask = Solder mask
pcb-stack-finish-type = Type
pcb-stack-finish-lpi = LPI
pcb-stack-finish-dry-film = Dry film
pcb-stack-finish-colour = Colour
pcb-stack-finish-gloss = Finish
pcb-stack-finish-glossy = Glossy
pcb-stack-finish-matte = Matte
pcb-stack-finish-sides = Sides
pcb-stack-finish-both = Both
pcb-stack-finish-top = Top
pcb-stack-finish-bottom = Bottom
pcb-stack-finish-over-laminate = Thickness over laminate ({ $unit })
# $value is already formatted; $unit is mm, µm or mil; $dk is the Dk.
pcb-stack-finish-over-copper = Over copper { $value } { $unit } · Dk { $dk } · set on the Solder Mask rows in Stack
pcb-stack-finish-no-mask-row = The stack has no Solder Mask row.
pcb-stack-finish-silkscreen = Silkscreen
pcb-stack-finish-surface = Surface finish
pcb-stack-finish-enig = ENIG
pcb-stack-finish-enepig = ENEPIG
pcb-stack-finish-hasl-lead-free = HASL, lead-free
pcb-stack-finish-hasl-leaded = HASL, leaded
pcb-stack-finish-immersion-silver = Immersion silver
pcb-stack-finish-immersion-tin = Immersion tin
pcb-stack-finish-osp = OSP
pcb-stack-finish-hard-gold = Hard gold
pcb-stack-finish-hasl-note = HASL is not recommended below about 0.65 mm pitch or for fine-pitch BGA.
pcb-stack-finish-green = Green
pcb-stack-finish-red = Red
pcb-stack-finish-blue = Blue
pcb-stack-finish-black = Black
pcb-stack-finish-white = White
pcb-stack-finish-yellow = Yellow
pcb-stack-finish-purple = Purple
pcb-stack-finish-note = Mask, silkscreen and finish are saved in the board. Undo works per edit.
pcb-stack-finish-default = Not set yet: these are the defaults, and the first change saves them with the board.
pcb-stack-material-no-config = this system has no config folder to keep it in
pcb-stack-col-df-stored-tip = Loss tangent of the layer, copied from the material library or typed here. 0 clears it.
# $value is already formatted; $unit is mm, µm or mil.
pcb-stack-board-material-thickness = { $value } { $unit }
# $value is the Dk, or a dash when none is set.
pcb-stack-board-material-dk = Dk { $value }
# $value is the Df, or a dash when none is set.
pcb-stack-board-material-df = Df { $value }
