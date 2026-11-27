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

The stackup has an optional name (mockup row 105, #2833), the `stack_name` key
of `[stackup]`. A named board writes `snxpcb/1.8`; without a name the key is
absent and the file keeps its lower token, byte for byte
(`docs/FORMAT_VERSIONS.md`). `BoardCommand::SetStackupName` is the only edit
of the name, one undo step that trims it and takes a blank as no name; a
`SetLayerStack` keeps it. Naming a board with no stored stack stores the
derived one, and removing the name of a stack equal to the derived one stores
none again. The toolbar field keeps a draft until Enter and shows the board's
file name as its placeholder.

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

The manager (Design ▸ Layer Stack Manager..., `board_layer_stack`) is a page
tab of the board (mockup row 105; docs/PROJECT_TAB.md, "Page tabs"), opened by
every Layer Stack entry point and never docked. The tab has the board's path,
so it shows the board's dirty dot, and Save and Undo act on the board. It
always sits after the board's tab, because path lookups take the first tab;
closing it leaves the board alone, and closing the board closes it.

Its Stack sub-tab is a table, one row per layer: Name, Type (a copper row
switches between Signal and Plane), Material, Thickness, Er (dielectric and
mask rows) and Weight (copper rows). Beside it are a to-scale cross-section
with the drill spans and an oblique preview of the board slab; a row with no
thickness is drawn as a mark. The toolbar's Add inserts a signal, plane or
dielectric below the selected row, kept between the outer copper rows; a new
copper layer brings a dielectric with it. Remove takes the selected row, and
a dielectric next to a copper one. Move Up and Move Down swap the row with its
neighbour. Each is one step through `signex_engine::pcb::stack_edit`. The
mm / µm / mil switch is view state: the board stays in mm. Symmetric stack
reads whether the stack mirrors; Presets, Import and Export are greyed with
their reason. An impedance column waits for a trace-width choice.

The Via Types / Drill Pairs sub-tab lists what the board's vias make
(`signex_engine::pcb::drill_pairs`, `via_types`): each pair's span, depth
through the stack, smallest drill, aspect ratio and via count, and each type's
drill, finished hole, pad, protection and use count. Neither table is stored.

## Materials and finish {#materials}

The Materials and Finish sub-tabs follow Stack and Via Types / Drill Pairs
(mockup rows 176 and 177; decision 2026-11-24: "Built-in plus a user file;
the board stores a copy of the values it uses so it opens the same
anywhere").

**Materials.** The library (`signex_types::material_library`) lists cores,
prepregs, copper foils and solder masks with thickness, Dk and Df. The
built-in entries are a generic list in the code. The person's own entries are
one file in the config folder, `materials.snxmat` (token `snxmat/1`; a newer
or missing token is refused). It is read in a task the first time Materials
shows: a missing file is an empty library, a file that fails to read stays
unwritten and the tab says why. Add material and Remove write the whole file
atomically in a task, one write at a time. Use for selected layer copies the
picked entry into the row selected in Stack, if the entry's kind fits the
row (`MaterialKind::fits`), as one undo step (`StackEdit::UseMaterial`): a
dielectric or mask row takes the name, thickness, Dk and Df, a copper row the
thickness and the weight it makes. The board keeps the copy, not a link, so
a changed or missing library changes no board.

A row's Df is `loss_tangent` in `[[stackup.rows]]`, omitted at 0. The Stack
table's Df column shows it on dielectric and mask rows and edits it with
Enter (`StackEdit::SetLossTangent`); 0 clears it.

**Finish.** The board's solder mask (type, colour, gloss, sides, thickness
over laminate), silkscreen (colour, sides) and surface finish are
`LayerStack::finish`, saved as `[stackup.finish]` with `.mask` and
`.silkscreen` tables; a field left out takes its default. Until the first
edit the page shows the defaults (ENIG, green glossy LPI mask on both sides,
white silkscreen) and stores nothing. Each pick, and the thickness over
laminate on Enter, is one undo step (`StackEdit::SetFinish`). The mask's
thickness over copper and its Dk are the Solder Mask rows' and are edited in
Stack. A `SetLayerStack` sent without a finish (an agent's row list) keeps
the board's. Board materials lists each set of dielectric and mask values
the rows store, once.

A board with a finish or any row Df writes `snxpcb/1.10`; a board without
either keeps its lower token, byte for byte (`docs/FORMAT_VERSIONS.md`).
Sides are stored as fab data only: they do not add or remove stack rows.

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

A via's span comes from the via type it picks in the Via panel (mockup row
96); the free From and To rows are gone. An agent still sets a span with
`set_via_span`: both ends must be different copper layers of the board; Top
to Bottom is a through via, any other span a blind (or buried) one, and a
micro via stays micro. DRC reads the span (`pcb_geom::via_layers`).

## Via types {#via-types}

Until the Layer Stack Manager stores via types, the board's via types are
derived from its vias (`signex_engine::pcb::via_types`): one type per kind,
span and tenting the vias have, sized by the diameter and drill most of them
share (on a tie the smaller), plus the through via the Via tool places
(`PcbSetup::via_diameter`, `via_drill`) when no via is through. Through types
come first, then by top layer; a type is named with the span name the panel
shows ("Thru 1:4"). Picking a type in the Via panel sets the vias' kind,
span, diameter, drill and tenting as one undo step. Diameter, Drill and
Finished Hole are always editable. A via whose diameter or drill differs from
its type's is an override: that size is in bold with a reset arrow, and its
"from type" note goes. Which size differs is the engine's judgement alone
(`via_types::overridden_fields`, sizes within 1e-7 mm are the same, #2735);
the panel reads it and does not compare values itself. Typing the type's
value clears the override. The finished hole is the drill less twice
`VIA_PLATING_MM` (0.025 mm), and typing one sets the drill that leaves it.

A drill below the via's minimum shows a warning. The minimum is the one the
design rule check holds that via to, a written `via` rule's `drill >=` for
its net, class and layers (`DrcRules::via_drill_limit_mm`), so the panel and
the check agree; where no written rule applies it is
`PcbSetup::via_min_drill`, which the check does not enforce yet
(`via_types::via_min_drill`, #2735). With several vias selected the largest
of their minimums is shown.

A known trap of derived types: the type's size is the one most of its vias
share, so an override on most vias of a type moves the type's size with it.
Five vias at 0.6/0.3 with three of them overridden to 0.8/0.4 make a 0.8/0.4
type; the two untouched vias then read as overridden, and their reset arrows
size them to 0.8/0.4. A type with a single via never reads as overridden:
typing a new size on it changes the type's size. Stored via types (mockup row 105) end this; until then
`via_types_tests::overriding_most_vias_of_a_derived_type_moves_the_type`
pins it.

The Protection section names the IPC-4761 type the via's fill, plug side, cap
and per-side tenting make (`signex_types::pcb_via_protection`). A via stores
only its tenting, so fill and cap read as none and the Type, Fill, Plug From
and Cap rows are read-only; Tented Top and Bottom stay editable there until
via types are stored. Type II is listed greyed: its second cover coat is not
stored.

Free pads (a pad that belongs to no footprint) do not exist on a board yet,
so a pad's lock is its footprint's and its layer is not edited here.
