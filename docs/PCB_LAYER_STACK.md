# Board layer stack, per-layer sizes and item locks

Issues #2401 and #2407.

## The stack {#stack}

A board stores its physical stack in the `.snxpcb` `[stackup]` section as
`[[stackup.rows]]`, top to bottom: kind (`signal`, `plane`, `dielectric`,
`solder_mask`, `paste`, `overlay`), name, the board layer of a copper row,
material, thickness in mm, Er and copper weight in oz. A board saved before
the manager has no rows and shows `LayerStack::derived`: overlay, paste and
mask on each face, its copper layers with FR-4 between them, sized so the
total is the board thickness. The first edit stores the stack.

Copper is named by place: the first copper row is `TopCopper`, the last
`BottomCopper`, the rest `Inner1Copper`, `Inner2Copper`, ... in order. A
copper row names the board layer it was (`layer`), or none for a new one.
`BoardCommand::SetLayerStack` (`signex_engine::pcb::stack`) gives each row
its token by place and renames the copper of every item (tracks, vias, their
per-layer sizes, zones, texts, graphics, footprints and pads) whose layer
moved place, as one undo step. A per-layer size on a removed copper layer is
dropped (a list left with no entry becomes the item's one size). It also
writes the copper entries of the layer list (the layer tabs read it), each
in the place of a copper entry in turn, so other entries keep their place and
an edit that keeps the copper leaves the list as it was; it sets the board
thickness to the stack's total. It is refused for a number that is negative or not finite,
fewer than 2 or more than 32 copper rows, a copper row naming a layer the
board does not have or one named twice, an outer copper row moved inward,
and a copper layer left out while items or a board-sketch keepout are on
it. Its inverse, `RestoreLayerStack`, puts back the stack, layer list,
thickness, copper names and dropped sizes exactly.

The manager (Design ▸ Layer Stack Manager..., `board_layer_stack`) is a
table, one row per layer: Name, Type (a copper row switches between Signal
and Plane), Material, Thickness, Er (dielectric and mask rows) and Weight
(copper rows), with the total thickness and copper count above it. Add
Signal, Add Plane and Add Dielectric insert below the selected row, kept
between the outer copper rows; a new copper layer brings a dielectric with
it. Remove takes the selected row, and a dielectric next to a copper one.
Move Up and Move Down swap the row with its neighbour. Each is one step
through `signex_engine::pcb::stack_edit`. An impedance column waits for a
trace-width choice.

## Per-layer sizes {#sizes}

A via (`Via::layer_sizes`, a diameter) or pad (`Pad::layer_sizes`, an x/y
size) may size its copper by layer: top/middle/bottom (middle is every inner
layer) or a size per copper layer, where a layer not listed keeps the item's
own size. Saved in the optional `layer_sizes` via and pad column, in nm:
`top=<v>;middle=<v>;bottom=<v>` or `<layer>=<v>;...`, a pad size `<x>x<y>`.
A malformed cell is a load error. One rule (`LayerSizes::list_fault`,
`size_storable`) holds for the engine and the writer: a per-layer list names
at least one copper layer, each once, and every size is at least 1 nm, so an
edit the engine takes always saves. The Via and Pad panels' Simple /
Top-Middle-Bottom / Full Stack tiles pick the form (each layer keeps its
size) and show a size row per layer. The renderer draws a via or pad with its
size on the active copper layer; DRC checks a via and a pad with their
copper on each layer (`pcb_geom::via_pieces`, `pcb_geom::pad_pieces`), and a
via's smallest ring against a minimum diameter rule (`Via::min_diameter`:
the layers its span passes, the via's own diameter only where its sizes do
not cover a layer); the keepout check tests each layer's copper against a
keepout on that layer. Pour fills, the board tools, the board PDF (each page
draws the size on its layers) and the fabrication outputs read the size on
each layer too (`docs/FABRICATION_OUTPUTS.md`).

## Locks and the via span {#locks}

Tracks and vias carry a lock (`Segment::locked`, `Via::locked`), saved in
the optional `locked` track and via column. `BoardCommand::SetItemsLocked`
sets it as one step. A locked item refuses a checked replace (any Properties
edit), Delete refuses it, and a moved footprint leaves a locked track where
it is (`pcb_geom::tracks_on_pads`). Routing adds copper; push and shove
never moves or stretches a locked track, so the route walks around it. The
Properties Lock box, the board menu's Lock
and Unlock rows (`lock_board_items`, `unlock_board_items`) and an agent with
`ids` set it.

A via's span is edited with the From and To rows of the Via panel or
`set_via_span`: both ends must be different copper layers of the board; Top
to Bottom is a through via, any other span a blind (or buried) one, and a
micro via stays micro. DRC reads the span (`pcb_geom::via_layers`).

Free pads (a pad that belongs to no footprint) do not exist on a board yet,
so a pad's lock is its footprint's and its layer is not edited here.
