# Main menu bar

The bar is plain data: `menu_bar_model(&MenuContext)` in
`crates/signex-app/src/menu_bar/model/` builds it, and `menu_bar/view.rs`
renders it as iced_aw menus. Its content is pinned by
`menu_bar/testdata/menu_bar.golden.tsv` (Place on a board in
`menu_bar_place_board.golden.tsv`, [below](#place-board)); changing a menu, label, shortcut,
order or message is a content change and needs the maintainers' sign-off.

## How a row runs {#row-routing}

A row that carries a command id (`cmd_item`, `cmd_item_gated`; every row with
a shortcut) publishes `MenuMessage::Command(id)`. The main window sends it to
`handle_menu_message`, which runs the id through `Signex::dispatch_command`,
the same entry the shortcut, the palette and the context menu use, so an id has
one behaviour on every surface. An undocked window sends it as
`UiMsg::MenuInWindow`, and the native menu routes it the same way for the
focused window; both dispatch the id against that window, exactly as the
shortcut typed there does. Only some ids act on that window's own document:
Save, Save As, Print, Close Document and the tab steppers, Undo and Redo, and
the commands that send a canvas event (Fit All). Cut, Copy, Paste, Delete,
Select All, Find, the Place rows, Zoom In / Out and AutoFocus still act on the
main window's active tab, as their shortcuts do. Every board Tools row (Via
Stitching, Add and Remove Teardrops, Gloss, Board Shape, Keepout, Arrange),
Repour Selected, Repour All and the two Tune rows edit the board an undocked
board window shows, never the main window's (#2629).

A menu click that cannot run is not silent: `run_menu_command` logs a command
the window cannot act on to the Messages panel ("Menu: <id> did nothing:
<reason>"), and the bridge already logs an unknown or unmapped id.

No button without a function: `menu_bar/tests/walk.rs` also presses every live
row in each window state (no document, a project alone, a schematic with and
without selection, a dirty one, a symbol editor, a footprint editor, a PCB tab,
an undocked window) and fails if a click moves nothing, starts no job and says
nothing. A row whose click posts a Messages line on the clicking thread
(`diagnostics::said_on_this_thread`) counts as live: a board tool with nothing
to act on says so (Repour on a board with no pour, #2216) instead of changing
the board. Any `info` or `warn` line counts, so the check does not tell a
real answer from a "not implemented" line; a handler that only says that is
still a dead row and a review finding. `KNOWN_INERT` lists the rows allowed to do that and is empty; it may only
shrink.

A live row's command must run: `menu_bar/tests/walk.rs` checks, for every row
in each kind of window state, that a live row's `CommandOutcome` is `Ran` and a
greyed row's is `Disabled`. The golden fixture still names each such row by the
message it used to send; `COMMAND_ROW_NAMES` in `menu_bar/golden_tests.rs` maps
the id back to that name.

## Header entries {#header-entries}

A header is a small caption over a group of rows, such as BUILT-IN and YOUR
LAYOUTS in [View ▸ Window ▸ Layouts](#window-layouts). It is not a command
row and not a greyed row (maintainer decision, Caner, 2026-11-20). In the
model it is a `MenuItem` in the `ItemState::Header` state, not a `MenuEntry`
variant of its own (`MenuEntry` stays `Item`, `Submenu`, `Separator`), built
by `header()` in
`model/items.rs`, with catalog text, no command, no shortcut and no check
mark. `MenuItem::is_header()` says which rows are headers.

- **In-window bar.** The caption is drawn at the caption type step in the
  muted text colour, with the row padding and the shared menu card's header
  height. It is a plain container, not a button, so it has no hover fill and
  no press handler, and iced_aw never makes it the active row.
- **Command walks.** A header never dispatches (`dispatches()` is `None`), so
  it is not a live row, and it is not a planned or unexplained greyed row:
  the `stub` and `unavailable` ratchets in `menu_bar/planned_tests.rs` do not
  count it. It has no command id, so the shortcut, catalog and accelerator
  checks skip it too.
- **Golden fixture.** `menu_bar.golden.tsv` lists it with the kind `header`,
  enablement `0` and gate `never`.
- **Native macOS menu.** An OS menu has no caption row, so the header shows
  as inert text there.

## Root menus {#root-menus}

Eight roots, left to right: File, Edit, View, Project, Place, Design, Tools,
Help (`ROOT_TITLES`). The model is shared, so every theme and the native macOS
bar show the same eight. Project was added on 2026-11-07 (maintainer decision,
`DECISIONS.md`), built only from existing commands and messages. On
2026-11-19 the Reports root became the **Tools ▸ Reports** submenu, directly
under Tools ▸ Annotation, and the Window root became the **View ▸ Window**
submenu, the last entry of View after a separator (#2524). Both submenus keep
the old root's rows, order, catalog ids, shortcut hints and enablement, and
their titles are the old root titles (`menu-reports`, `menu-window`); no
command left the app. A submenu row is never greyed itself: with no document
open, Tools ▸ Reports and View ▸ Window still open, and every row in them is
greyed.

**Project** (project-wide actions):

- Electrical Rules Check (`open_erc_dialog`; F8 `run_erc` runs at once)
- Add Table of Contents Sheet (`add_table_of_contents`, #1300): writes
  `contents.snxsch` next to the root sheet (refused, and said so, when that
  file is there) and adds it to the project; gated like Electrical Rules
  Check. See `SCHEMATIC_TABLES.md`.
- Compare Board with Schematic… (`compare_board_with_schematic`, see
  [below](#project-compare-board))
- Design Notebook ▸ Open Design Notebook, Generate from Project… (see
  [below](#project-design-notebook))
- Project Options... (greyed with no project, like Close Project)
- Design Checklist (`open_design_checklist`, #1291, mockup row 206): the
  project's checklist as a document tab, greyed with no project open. See
  [DESIGN_CHECKLIST.md](DESIGN_CHECKLIST.md).
- Close Project

**Tools ▸ Annotation** (top of Tools, then Tools ▸ Reports, then a
separator): Annotate Schematics..., Reset Schematic Designators...,
Reset Duplicate Schematic Designators..., Annotate Schematics Quietly (Alt+A),
Force Annotate All Schematics (Shift+Alt+A), Back Annotate Schematics... (see
[below](#back-annotate)), and Number Schematic Sheets... (see
[below](#number-schematic-sheets)).

**Tools ▸ Reports** (outputs of the open schematic, all greyed without one):

- Bill of Materials... (the one BOM dialog). File ▸ Export ▸ Export BOM…
  opens the same dialog. This row keeps its name until BOM Studio (mockup
  row 113) takes the Reports row.
- Netlist (.NET)...
- ERC Report...

Then, after a separator, the board's reports (#2404), greyed without a board
in view: Board Bill of Materials..., Board Netlist..., Net Lengths...,
Component Report... (each opens Board Reports on its table) and Export Report
as CSV... (the table in view, or the BOM). The Board panel's Reports button
opens the same dialog. Last, Assembly Drawings as SVG... (#2486) writes the
board's top and bottom assembly drawings into a folder the person picks; the
PDF pages are Board Print / PDF's Assembly Drawing sets. Rules:
`BOARD_OUTPUT.md`.

**View ▸ Window** (the last entry of View, after the panel rows and a
separator): Tile Horizontally, Tile Vertically, Tile Grid, Untile (see
[below](#window-tile) and [Panes](#panes)), Split Right, Split Down, Open
Second View (mockup row 207, see [below](#window-split)), a separator, the
Layouts submenu (mockup row 166, see
[below](#window-layouts)), and Close All Documents (see
[below](#tools-library-editor)). Their built-in keys are unchanged.

**Design** has Update PCB Document (see
[Design ▸ Update PCB Document](#design-update-pcb)), Update Schematics (see
[Back Annotate](#back-annotate)), Board Sketch (see
[Design ▸ Board Sketch](#design-board-sketch)), Layer Stack Manager... (see
[Design ▸ Layer Stack Manager](#design-layer-stack)), the Rooms submenu (see
[Design ▸ Rooms](#design-rooms)), a separator, then
Parameter Manager..., Net Classes (see
[below](#find-similar-filter-net-classes)), Differential Pair Classes…
(`open_diff_pair_classes`, #1282: the project's differential pair classes in
a dialog, greyed with no project open; see
[SCHEMATIC_DIRECTIVES.md](SCHEMATIC_DIRECTIVES.md#differential-pair-classes))
and Rules... (the project's design rules as a document tab,
greyed with no project open; see [DRC_RULES.md](DRC_RULES.md#rules-manager), #2398),
then, after a separator, Create Sheet Symbol From Sheet... and Synchronize
Sheet Entries and Ports..., greyed with no sheet in view (see
[Bottom-up hierarchy](#bottom-up-hierarchy), #1276). **File ▸ New** has Schematic (`new_schematic_sheet`) and PCB
(`new_pcb_document`), each opening its setup dialog (#2232), and Block
Diagram (`new_block_diagram`, mockup row 171, #2796): an empty `.snxblk`
named after the project, in its folder, opened unsaved; greyed with no
project open. **File ▸
Import** adds DXF to Current Layer... (`import_dxf`) and DXF as Board
Outline... (`import_dxf_outline`), greyed without a board in view; each asks
for the file and imports it as one undo step (#2410, see
[BOARD_IMPORT_AND_REUSE.md](BOARD_IMPORT_AND_REUSE.md)). Library…
(`import_library`, also in the command palette as "import library") sits
after the project import row and is always enabled: it opens the Import
Library dialog, which turns a library made in another tool into a native
library through an installed library converter (mockup row 211, #3117; see
[LIBRARY_CONVERTER_CONTRACT.md](LIBRARY_CONVERTER_CONTRACT.md)). **File ▸
Export** has PDF…, Export BOM… (the BOM export dialog, named by its result,
greyed without a schematic; mockup row 112, Caner 2026-11-20), STEP 3D
Model… and Design Notebook PDF… (see
[Project ▸ Design Notebook](#project-design-notebook)).
Project tree actions with no menu message or command id (add or remove a
document, Enable Version Control, History, Validate Project) stay in the tree's
context menu; the menu gets no stub for them.

## Extension rows {#extension-rows}

A binary built on `signex-app` can install extensions that add rows (#2495,
`docs/EXTENSION_API.md`, "Menu bar and Active Bar rows"). Each one goes at the
end of its root menu, after every built-in row and below one separator, and
runs the extension's own `ext.<name>.` command through `dispatch_command` like
any command row. The Community binary installs none, so its bar and
`menu_bar.golden.tsv` are the built-in rows alone.

## Rows that name a different command {#own-action}

Every row that carries a command id runs that id; no row keeps an action of its
own. Two rows point at an id other than the one their key suggests (#1991):

- View ▸ Cycle Grid Size runs `next_grid`, which steps the snap grid (the `N`
  key in the Classic profile). `cycle_snap_grid_forward` (`G` in the Signex
  Standard profile) still opens the footprint editor's grid picker.
- Project ▸ Electrical Rules Check runs `open_erc_dialog`, which opens the ERC
  dialog to choose the rules. `run_erc` (F8) runs ERC at once. The row shows no
  key, as `open_erc_dialog` has no default binding.

## Tools ▸ Design Rule Check {#tools-drc}

Tools ▸ Design Rule Check runs `run_drc` (#2222) on the PCB in view and is
greyed while no PCB is open. The command is also in the command palette and
as the Run DRC button of the Messages panel. On a board the key is `T D` in
the default profile and `F7` in Classic.

## Tools ▸ DRC and Tools ▸ Net Classes {#tools-drc-results}

Right after Design Rule Check, Tools ▸ DRC (`open_drc_panel`, #2399; named
like the ERC panel since mockup row 87) and Tools ▸ Net Classes
(`open_net_classes_panel`, #2400) open the board's DRC panel and the Net
Classes panel. DRC is greyed while no
PCB is open. Net Classes is an alias of Design ▸ Net Classes
(`open_net_classes`), live on a sheet or a board: see
[Design ▸ Net Classes](#find-similar-filter-net-classes). No keymap preset
binds them.

The DRC results panel lists the last run's violations grouped by rule, each
group with its count. A row shows the severity, what broke, the nets, the
objects involved and the location; a click selects its objects and centres
the view on it at the zoom in use (`show_drc_violation`). The filters (rule, layer, severity,
Show waived) are `filter_drc_results`. Waive opens a note field; its Waive
stores the waiver with the note in the board (`waive_drc_violation`, one
board undo step), and Unwaive takes it back (`unwaive_drc_violation`). A
waived violation is hidden until Show waived is on. The results are marked
stale when the board or its project's net classes change after the run (a
waiver alone, or its Undo or Redo, does not). A
group's Rule link opens the rule in the Rules tab; Short, Unconnected and
Keepout groups have none, since no written rule sets them.

The Net Classes panel is a table of the board's project net classes (the
same table the schematic's net classes and DRC read): name, colour, track
width, clearance, via diameter and drill, diff-pair gap and width, and the
member nets as chips. Add Class, Delete, a typed cell (Enter) and a net
picked into or removed from a class are each one step of the project's
net-class history, with the panel's Undo and Redo. The commands are
`add_net_class`, `rename_net_class`, `delete_net_class`,
`set_net_class_rules` and `assign_board_net_class`.

## Tools ▸ Re-Annotate Board {#tools-reannotate-board}

Tools ▸ Re-Annotate Board... runs `reannotate_board` (#1312), right after
Design Rule Check with the other board rows, and is greyed while no PCB is
open. It opens the Re-Annotate Board dialog (subtitle "Positional · <board>")
for the board in view:

- **Order**: by X, then by Y (the default), or by Y, then by X. The sort is
  exact at 1 µm: parts are not grouped into columns or rows by a tolerance,
  so a part 0.01 mm left of another comes first whatever its Y. The board's y
  grows downwards on screen.
- **Bottom side**: after the top side (the default), or together with it.
  Bottom-side parts are ordered as seen from the top.
- Parts are numbered from 1 within each prefix (`R1, R2, ...`, `C1, ...`);
  the prefix stays. Surrounding spaces are dropped, and `R01` becomes `R1`.
  A locked footprint keeps its designator, and its number is skipped; so
  does a footprint with no link to a schematic part, because Update PCB
  matches such a footprint to a part by its designator and renumbering it
  would swap parts. A footprint whose designator is not a prefix and an
  ASCII number (`12`, `R1A`, `R１`, none) is kept and listed. A `R?` gets a
  number.
- The list shows each rename (designator, new designator), how many parts
  are already in order and the parts kept. Execute runs the renames as one
  board undo step at the board revision the list was made from; a board
  that changed meanwhile gets a new list to check first, with a notice.
  Nothing is written to disk until the board is saved.

The dialog keeps the options it was last used with until the app quits. The
same command is in the command palette; no keymap preset binds it.

**Update PCB renames parts back.** A designator changed on the board, here
or in Properties, is not carried to the schematic: the next Update PCB lists
a rename back to the schematic's designator, ticked like every other row,
because the command picks the direction (`DECISIONS.md`, 2026-11-10). Use
[Back Annotate](#back-annotate) to keep them. The dialog says so under its
fields. Properties has no standing note: it warns through the designator
mismatch callout, with its Update PCB button (`DECISIONS.md`, 2026-11-23).

## Tools ▸ Repour Selected, Repour All, Unfill Selected {#tools-repour}

Right after Re-Annotate Board, Tools ▸ Repour Selected and Repour All run
`repour_selected` and `repour_all` (#2216) on the board in view, greyed while
no PCB is open. Both fill the board's polygon pours again and say in Messages
how many pours, pieces of copper and removed islands they came to; Repour
Selected reports the selected pours and says so when none is selected.
Rules in `docs/PCB_TOOLS.md`, "Repour".

Tools ▸ Unfill Selected, right after Repour All (#2506, DECISIONS
2026-11-25), runs `unfill_selected` on the board in view, greyed while no
PCB is open: the selected pours keep their outline and hold no copper until
the next Repour, as one undo step, and with no pour selected Messages says
to select one. Rules in `docs/PCB_TOOLS.md`, "Polygon Pour".

Tools ▸ Polygon Manager..., after them, runs `polygon_manager` (mockup row
178): it opens the board's Polygon Manager tab, or focuses the one that is
open, greyed while no PCB is open. The board menu offers the same row on a
polygon pour, and the PCB panel's Polygons mode links to it. See
`docs/PCB_POLYGON_MANAGER.md`.

## Tools ▸ Panelize Board and Remove Panel {#tools-panelize-board}

Tools ▸ Panelize Board runs `panelize_board` (#2410) and Tools ▸ Remove
Panel runs `remove_board_panel`, after Arrange in Box; both are greyed
while no PCB is open. Panelize Board gives the board in view the default
panel (two by two, rails top and bottom, mouse bites) as one undo step; on a
board that already has a panel it changes nothing and says so in Messages.
Remove Panel makes the board single again as one undo step, and says in
Messages when there is no panel to remove. The panel's settings are edited in
the Panel section of the board's Properties. Rules:
[PANELISATION.md](PANELISATION.md).

## Back Annotate {#back-annotate}

Tools ▸ Annotation ▸ Back Annotate Schematics... and Design ▸ Update
Schematics run `back_annotate_schematics` (#1312). The Design row reads
Update Schematics in `<project>` on a board of a project. Both are greyed
unless a schematic or a board of an open project is in view; the command is
also in the palette, and no keymap preset binds it. It opens the Back
Annotate dialog for the project of the document in view, opening the
project's board first when it is not open.

The dialog has the Update PCB dialog's look with the board as the source:

- **Matching**: a board footprint matches its part only by its stored link
  (sheet placement + unit UUID), never by designator. An unlinked footprint
  or one whose part is gone is a note, and so is a second footprint linked
  to a part already matched (only the first is carried back). Surrounding
  spaces on the board designator are ignored.
- **Rows**: Designators (the board's designator for the part at that
  placement), Values (the board's value for every unit of the part) and
  Footprints. The board wins on every ticked row; rows start ticked.
- **Blocked rows**: a designator row while the part's schematic designator
  is not annotated or is on another part (as in Update PCB), or while the
  board's designator is empty or holds `?`; a value row on a sheet placed
  more than once, or on a sheet another open project reaches, since the
  value is stored once for every placement and project (the shared-sheet
  rule); a value row of a part that is not annotated; every footprint row,
  until #2248. Pin and gate swaps have no board model and are
  not listed.
- **Notes**: a designator the ticked rows would leave on several parts is
  noted, following the ticks; nothing blocks it.
- **Validate** runs the ticked rows on copies of the sheets.
- **Execute** writes every changed sheet as one undo group: a designator row
  writes that placement only (the symbol's reference on a sheet placed once
  in this project and reached by no other open project, a per-placement
  entry otherwise). Execute plans the writes from the ticks when pressed,
  states the closed sheets of that plan again, and writes exactly that
  plan; the ticks, Refresh and Validate wait while the check runs. A sheet
  that is not open is loaded into memory from the copy the read took, after
  its file is stated again, and is left unsaved. Nothing is written to disk until the sheets are saved.
  A list the board or a sheet has moved past since the read is refused
  until Refresh; so is a closed sheet whose file changed on disk.
- Opening the board for the read brings its tab to the front, as Update
  PCB does.

## Project ▸ Compare Board with Schematic {#project-compare-board}

Project ▸ Compare Board with Schematic… (right after Electrical Rules
Check) and the board right-click menu's Compare with Schematic run
`compare_board_with_schematic` (#2462). Both are gated like Update PCB:
greyed unless a schematic or a board of an open project is in view. The
command is also in the palette; no keymap preset binds it. It opens a modal
report of every difference between the project's schematic and its board,
opening the board first when it is not open; Esc closes it (or, while it is
open, the confirmation before Apply).

Each row picks Update PCB, Update schematic or Skip; Extra on board and
Footprint mismatches start skipped. Apply asks first when a chosen
row deletes from the board, then runs the PCB side as one board undo step
and the schematic side as one undo group. Rows, defaults and Apply:
[UPDATE_PCB.md](UPDATE_PCB.md#compare-board-with-schematic).

## Project ▸ Design Notebook {#project-design-notebook}

A project has one Design Notebook, `notebook/design.snxnbk` (mockup row 195).
These rows and commands reach it; all are in the palette. Only the badges
toggle has a default key, `B` in Signex Standard (Classic keeps `B` for
Place Bus):

| Row | Command | Greyed |
|---|---|---|
| Project ▸ Design Notebook ▸ Open Design Notebook | `open_design_notebook` | with no project open |
| Project ▸ Design Notebook ▸ Generate from Project… | `generate_design_notebook` | with no project open |
| File ▸ Export ▸ Design Notebook PDF… | `export_notebook_pdf` | with no project open |
| View ▸ Notebook Badges, after AutoFocus, checked while the badges show | `toggle_notebook_badges` | with no sheet in view |

Each acts on the project of the active document, else the active project,
by the same rule as Design ▸ Rules. The Projects tree reaches the same
notebook for the project it is on: Add New ▸ Design Notebook (live since
the notebook landed; before, a planned row) and a double-click on the
notebook's row. The row shows under a Notebook group, after PCB, once the
project probe finds the file; a notebook that was never saved has no row.
A palette search for "notebook" or "wiki" finds `open_design_notebook`:
"Wiki" is the old dock panel's name (mockup row 60b). Open Design Notebook
and Notebook Badges are window-only, so a headless run is refused with a
reason; `generate_design_notebook` and `export_notebook_pdf` also run
headless with operands (`docs/DESIGN_NOTEBOOK_GENERATE.md`,
`docs/NOTEBOOK_PDF.md`). Each request is also said in Messages.

## Board Properties: designator and value {#board-properties}

With exactly one footprint selected on a board, the Properties panel shows
its designator and value as fields and its footprint name. Each field
keeps its own draft (another selection on the same board, a tab switch away
from the board, closing it, or an undo or redo of it drops them; a selection
on another board, in another window, does not); Enter applies that field as
one board undo step (`SetFootprintText`, labelled Designator or Value). A locked
footprint or a read-only board shows the
fields read-only with the reason; an empty designator is refused with a
notice; a designator another footprint already has is applied, warned about
in the Messages panel and noted under the fields. The board canvas
right-click menu has Properties... (`edit_selected_object_properties`), live
with anything selected, which shows the Properties panel; the same command
(and `edit_object_properties`, Altium's double-click) is now enabled by a
board selection. In an undocked board window the fields edit that window's
board.

## Tools ▸ Cross Probe and Cross Select Mode {#tools-cross-probe}

Tools ▸ Cross Probe runs `cross_probe` (#1312): the part or net selected on
the sheet or the board in view is selected on the other side, which is brought
forward (a hidden tab) or opened (the project's board, or the part's sheet)
and framed. It is greyed with nothing selected. The same command is the Cross
Probe row of the sheet canvas menu and of the board canvas menu, and is in the
command palette; no keymap preset binds it.

Tools ▸ Cross Select Mode runs `toggle_cross_select_mode` and shows a check
mark while the mode is on, which it is by default. While on, a selection on
one side is selected in every sheet or board window on screen on the other
side; it never opens a document or takes focus. The Signex Standard profile binds it to
Ctrl+Shift+X. How parts and nets are matched: `DECISIONS.md`, 2026-11-09.

## Design ▸ Update PCB Document, File ▸ New ▸ PCB {#design-update-pcb}

Design ▸ Update PCB Document runs `update_pcb_from_schematic` (#1312) and
opens the Update PCB dialog for the project of the document in view. Its
label follows that document: on a schematic it is Update PCB Document
`<board>` (the project's board file, or no name when the project has none), on
a board Import Changes From `<project>`. It is greyed unless a schematic or a
board is in view and a project is open. The undocked windows' menu bars word
and gate it by the window's own document. The same command is in the command
palette, and the Projects tree's project row has an Update PCB Document row
for the clicked project. The board component Properties panel runs it too:
when a selected component's designator differs from the schematic's, a warn
callout in Schematic Reference Information carries an Update PCB button,
greyed on a read-only board (#2523). No keymap preset binds it. The dialog:
[UPDATE_PCB.md](UPDATE_PCB.md).

File ▸ New ▸ PCB runs `new_pcb_document`: it opens the New PCB setup dialog
for the project in view, prefilled from the project's setup (#2232); its
Create PCB makes the project's board, named after the project, with the values
shown. It is greyed with no project open. Projects ▸ Add New ▸ PCB does the
same for the clicked project. The board opens as an unsaved tab; nothing is
written until Save. File ▸ New ▸ Schematic runs `new_schematic_sheet`: the New
Schematic setup dialog, whose Create asks where to save the sheet; also greyed
with no project. Both dialogs: `PROJECT_SETUP.md`, "New PCB and New
Schematic".

## Design ▸ Board Sketch {#design-board-sketch}

Design ▸ Board Sketch runs `board_sketch` (#2234): the board in view enters
sketch mode, where the footprint editor's sketch tools draw its outline,
keepouts, cutouts and mounting holes; run again, it leaves the mode and
commits the changes as one undo step. It is greyed while no PCB is open.
The same command is the PCB Active Bar's Board Sketch button, a row of the
board canvas menu and a palette row; on a board both built-in profiles bind
it to `1`. The mode:
[PCB_SKETCH.md](PCB_SKETCH.md#sketch-mode).

## Design ▸ Layer Stack Manager {#design-layer-stack}

Design ▸ Layer Stack Manager... runs `board_layer_stack` (#2401): it opens the
Layer Stack panel for the board in view, a table of its physical stack where
each edit is one undo step. It is greyed while no PCB is open. The same
command is a palette row and the layer tab menu's "Layer stack..." row; no
keymap preset binds it. The table and its rules:
[PCB_LAYER_STACK.md](PCB_LAYER_STACK.md#stack).

## Design ▸ Rooms {#design-rooms}

Design ▸ Rooms (#2457) holds Create Rooms from Sheets..., which runs
`create_rooms_from_sheets`: it reads the sheets of the board's project off
the update thread and opens its dialog. It is greyed unless a board that is
not read-only is in view and belongs to an open project (`Needs::PcbInProject`).
No keymap preset binds it. Rooms, the dialog and its plan:
[PCB_ROOMS.md](PCB_ROOMS.md#create-rooms-from-sheets).

## File ▸ Library {#file-library}

File ▸ Library carries only "Open Library..." (libraries outside the active
project) and the canvas-side "Place Component..." picker. Creating a new
component moved to the project tree: right-click ▸ Add New to Project ▸
Component Library, then Add New ▸ Component on the library node.

## File ▸ Export ▸ PDF {#file-export-pdf}

Print Preview used to be a top-level File row that duplicated Export ▸ PDF's
shortcut, so it was folded into Export ▸ PDF. File ▸ Print... (#1765) and
Export ▸ PDF open the same Export PDF dialog (`print`, Ctrl+P,
goes through the keymap command bridge in `app/command/bridge.rs`; Export ▸ PDF
runs as `print` in an undocked window, so both preview that window's document). Only
File ▸ Print... shows Ctrl+P, so one shortcut appears on one row. The menu rows
keep their names; whether to rename them to "Export PDF…" is still open.

The dialog is one screen (#2430): the sheets as thumbnails on the left (an
unticked sheet's thumbnail is dimmed, not removed), the page in the middle with
a toolbar for page stepping and zoom and a caption in paper units ("A4
landscape · 297 × 210 mm · Fit to Page"; ISO sizes go by their bare names, as
in the Size picker), and the settings on the right. The
settings sections, top to bottom: Sheets (Select All · Clear, "N of M"), Pages
(All / Current / Specific), Page Setup (size, orientation, scale, the margins
as two rows of two in mm from the page edge), Output (schematic and PCB colour
mode, quality, title block), Variant (one variant per PDF), Structure (folded,
"Logical" or "Physical"; the expanded names greyed until Use Physical
Structure is ticked), Watermark, PDF Layers, Bookmarks, Bookmarks and Nets
(folded, "N on") and Schematics include (folded, "N of 7"). The footer says
how many pages print, on which paper and at which quality, or the last
export's outcome; Cancel closes the dialog (Close once an export finished)
and Export PDF… asks where to save.

Watermark, PDF Layers and Bookmarks (#1298) carry a NEW pill on their
headings (mockup row 168). "Print a watermark" and "Write PDF layers" are
toggle switches heading their sections. "Print a watermark" is off by
default; on, it prints DRAFT, CONFIDENTIAL or a custom text at the chosen
opacity and angle (0, 45, 90 or typed), in the chosen colour (#2609: four
quick swatches, the default red #C0392B, a neutral grey, black and white,
then a field for any `#RRGGBB` code; the colour is remembered for the
document with its page setup, for the session), on every page or the first only,
optionally with a "Rev · date" line in English (the revision from the title
block, the date of the export). The page preview draws it where the PDF
prints it; switched off, its settings are kept for when it is on again.
"Write PDF layers" is off by default, so the PDF stays flat as before; on,
each group (wires and buses, parts, net labels and ports, power ports, title
block and border, watermark) is a layer a PDF reader can switch, its box says
whether it is visible when the PDF opens; the watermark row is greyed while no
watermark prints. Bookmarks: one per sheet, one per part (grouped by sheet or
A–Z) and one per net (the nets information the PDF has always carried; it is
no longer also under Bookmarks and Nets). Part bookmarks follow "One bookmark
per part" only; Bookmarks and Nets has no Include Component Parameters box
any more, because the exporter writes no parameter rows yet. The PDF carries
links (#1298): ports, sheet symbols, global labels, web datasheets and text
links, as set out in `PDF_EXPORT.md`, "Links"; the dialog has no switch for
them. The palette names the sheet export `export_schematic_pdf`; headless it
writes the named sheet's pages with the watermark and links its operands ask
for.

On a board (#2402) both rows are live and open Board Print / PDF instead: the
layer sets, one page per set or one composite page, the bottom side mirrored,
1:1 or fitted, colour or black, the outline, drill holes and the title block.
The palette names it `export_board_pdf`. Rules: `BOARD_OUTPUT.md`.

## File ▸ Export ▸ STEP 3D Model {#file-export-step}

On a board (#2488), File ▸ Export ▸ STEP 3D Model... (`export_board_step`)
asks where to save and writes the board body and its parts as a STEP model;
greyed without a board in view. Rules: `BOARD_OUTPUT.md`, "STEP export".

Ctrl+F4 (Close Document, Signex Standard profile) is a macOS system default ("Move
focus to active or next window") and may never reach Signex there; the menu
row always works.

## File ▸ Fabrication Outputs {#file-fab-outputs}

File ▸ Fabrication Outputs… runs `fabrication_outputs` (#2388), after the
Export submenu: it opens the Fabrication Outputs dialog on the board in view,
greyed while no PCB is in view. The same command is a palette row; it is not
on the PCB Active Bar, and no keymap preset binds it. Nothing is written until the dialog's Generate. Files,
formats and frame: [FABRICATION_OUTPUTS.md](FABRICATION_OUTPUTS.md).

## File: close, save all {#file-close-save-all}

Close Document (the `close_active_document` command, Ctrl+F4 in the Signex Standard
profile, closes the active tab like the tab menu's Close and parks a dirty
draft) is greyed when no tab is open. Project ▸ Close Project (the project tree's Close
Project for the active project, behind its unsaved-changes gate) is greyed when
no project is open. Save All writes every unsaved document and Component
Preview row through the same Save All flow the close and quit prompts use
(`SaveAllNext::Nothing`: no close, no quit afterwards). It runs the `save_all`
command, bound to `Mod+Alt+S` in both keyboard profiles (Ctrl+Alt+S on Windows
and Linux, Cmd+Alt+S on macOS; Save As keeps `Mod+Shift+S`). On a layout where
AltGr+S types a character, that key types it and Save All is reached from the
File menu. The row and the key are off when nothing is unsaved
(`CommandContext::has_dirty_documents`). It reports "Save All: saved N
item(s)" or the failures in the Messages panel.

## File ▸ Save Project As… {#file-save-project-as}

Save Project As… (`save_project_as`, #2908) sits right after Save All, before
the separator above Library; the File menu had no Save Project row to follow,
so it goes after the last Save row. It is greyed with no project open
(`Needs::OpenProject`) and no keymap preset binds it. It opens the Save Project
As dialog on the active document's project (else the first open one): the New
Project name and location fields, the `.snxprj` the copy lands on, and Save
copy. The copy always goes in its own folder, `<location>/<name>/`; a folder
that exists and is not empty, or a place inside a project, is refused as New
Project refuses it. The same dialog opens from the Save Failed card of a
project written by a newer build and from the Close Project and Exit prompts
that list one. What the copy holds: [PROJECT_SETUP.md](PROJECT_SETUP.md#save-project-as).

## Place ▸ Sheet Entry, Tools ▸ Library Editor, View ▸ Window ▸ Close All Documents {#tools-library-editor}

These three rows were planned stubs until #2247; each now runs a catalog
command, so the palette and a user key binding run the same id (none of the
three has a default key).

- **Place ▸ Sheet Entry** runs `place_sheet_entry`, the Active Bar's Sheet
  Entry tool. Greyed without a schematic, like the other Place rows. The
  Active Bar dropdown sends `ActiveBarAction::PlaceSheetEntry` directly and is
  not gated. The canvas Place flyout sends `ContextAction::Command` with
  `place_sheet_entry`, dispatched with `window = None`, so it is gated by the
  same `Needs::Schematic`. A flyout in an undocked sheet window runs inside
  that window's document swap, so the gate reads that sheet even while the
  main window shows a PCB (`place_design_rows` pins both). Passing the menu's
  own window instead of `None` is #2267.
- **Tools ▸ Library Editor** runs `open_library_browser` (maintainer decision,
  2026-11-09, `DECISIONS.md`). It focuses the active Library Browser tab, else
  the library of the active symbol or footprint editor tab, else the last
  Library Browser tab in the strip, else the first open library (the top row
  of the Libraries panel); a library with no browser tab gets a new one. From
  there the user picks a symbol or footprint to edit. Greyed when no library
  is open.
- **View ▸ Window ▸ Close All Documents** runs `close_all_documents`, the project
  tree's Close Project Documents: every tab closes, and an unsaved document is
  parked as a draft exactly as the tab menu's Close All Documents does. Greyed
  when no tab is open.

## Place on a sheet {#place-sheet}

Place on a sheet lists every schematic placement tool the Active Bar has
(#2415). The top rows are the wiring, part and text tools: Wire, Bus, Bus
Entry, Net Label, Component, Power Port, Junction, Text, Text Frame, Note, No
Connect and Sheet Entry. Design Block... (`place_design_block`) and Import
Graphics... (`import_graphics`) follow (#1667): each asks for a file and puts
its content on the cursor. Three submenus follow, grouped like the bar's
buttons (`menu_bar/model/sheet_place.rs`):

- **Ports & Connectors**: Port, Off Sheet Connector; Sheet Symbol, Device
  Sheet Symbol, Reuse Block; Signal Harness, Harness Connector, Harness Entry.
- **Directives**: Net Parameters, Generic No ERC, Differential Pair, Net
  Blanket, Compile Mask (#1489: Net Parameters was the planned Parameter Set
  row; the commands keep their ids `place_parameter_set` and
  `place_blanket`).
- **Graphics**: Arc, Full Circle, Elliptical Arc, Ellipse; Line, Rectangle,
  Round Rectangle, Polygon, Bezier, Table, then the **Dimension** submenu
  (Linear, Aligned); Graphic. Bezier (`draw_bezier`) went live and Table
  (`place_table`), Linear (`place_linear_dimension`) and Aligned
  (`place_aligned_dimension`) were added with #1267 / #1300.

A live row runs its catalog command, the one the Active Bar's row names, and
is gated by it (`Needs::Schematic`: greyed without a sheet). Its shortcut is
the command's key in the active profile. A tool that is a planned placeholder
(`active_bar::PLANNED_TOOLS`) is an `ItemState::Placeholder` with the planned
tooltip, read from the same list the bar greys its rows by, so the two never
disagree (`items::tool_item`). Global and hierarchical labels have no rows of
their own: `place_global_label` and `place_hierarchical_label` arm the same
Port and Off Sheet Connector tools. No Connect runs `place_no_connect`, so it
shows that command's key (`P X` in the default profile, `Q` in Classic); Text
still sends its own message (`MenuMessage::PlaceText`) and shows no key.

## Edit ▸ Find Similar Objects, View ▸ Selection Filter, Design ▸ Net Classes {#find-similar-filter-net-classes}

Rows added by #2416, each a catalog command greyed without a sheet:

- **Edit ▸ Find Similar Objects** runs `find_similar_objects`, the canvas
  menu's Find Similar: the object under the cursor, else the one selected,
  seeds the dialog.
- **View ▸ Selection Filter** runs `open_selection_filter`, which opens the
  Active Bar's Filter dropdown of the sheet in view.
- **Design ▸ Net Classes** runs `open_net_classes`, which opens the Net
  Classes panel: the project's net-class table, the same section the sheet's
  Properties shows. It is live on a sheet or a board (`Needs::NetClassTable`),
  and Tools ▸ Net Classes (`open_net_classes_panel`) is its alias: one route,
  one gate, and in the catalog both are `General` commands whose keys work on
  a sheet or a board tab.

## Place on a board {#place-board}

Place follows the document in view (#2360). While the window shows a board
(`CommandContext::pcb.shown`), it lists the board's tools: Component..., Pad,
Via, Interactive Routing, Track, Arc, Fill, Solid Region, Polygon Pour,
String, Dimension, Keepout and Room (`place_board_room`, #2457), the
Graphics submenu (Line, Arc, Full Circle,
Rectangle and Text Frame on non-copper layers, #2424, the PCB Active Bar's
Graphics group), then Board Sketch (`board_sketch`, the same command as
Design ▸ Board Sketch and the PCB Active Bar's button). Every other tab keeps
the sheet's Place rows.

The rows come from one table, `app/pcb_tools.rs` (`BoardPlaceTool::command`),
which the PCB Active Bar builds its buttons from (#2352), so the two surfaces
run the same set. Each tool (#2373) is a command row that runs the same
command as its Active Bar button and arms the tool on the board in view;
Component... (`place_board_component`, #2505) opens the part picker instead
and the pick goes on the cursor; its row carries the ellipsis as on a sheet,
while the Active Bar row reads "Component" (Caner, 2026-11-25). See "Board placement tools" in
`docs/PCB_TOOLS.md`. No board tool is a greyed placeholder any more.

The golden fixture marks the sheet's Place rows `-` in the `pcb` column: that
variant's bar does not have them. `menu_bar/testdata/menu_bar_place_board.golden.tsv`
pins the board's Place rows instead.

## Tools ▸ Assign Footprints {#tools-assign-footprints}

Runs `assign_footprints` (#2248; maintainer decision, Caner 2026-11-10 on
#2248, `DECISIONS.md`). Greyed without a schematic. The dialog lists every
placed part of the active schematic's project (a loose sheet lists its own
parts) with its designator, value, library row and current footprint, sorted
by designator. The project's sheets are read off the UI thread when it opens.

- A part chooses only among the footprints its own library row offers: the
  row's default and its alternates, the same list the Properties panel shows
  (`footprint_choices_for`). Its link to the row does not change. A part that
  cannot change shows its footprint as text with a reason pill: Sheet not
  open, Read-only sheet, Library row not open (it links to a library that is
  not open), No library link, or No alternates. A changed footprint shows the
  old name struck through and the new one in accent, with a dot on the row.
- A filter field (designator, value or library row) and All / Changed / No
  alternates chips with their counts narrow the table (Graphite #80).
- Tick boxes select several parts; the header box ticks the listed parts.
  "Tick all from this row" is a link at the end of the Library row cell of
  the hovered or previewed row and ticks every part of the same library row.
  While parts are ticked a bar shows their count, "Set ticked parts to" with
  the footprints every ticked part's row offers (so a bulk pick never gives a
  part a footprint its row does not offer, and the bar says so when they share
  none) and Clear ticks.
- A Footprint preview column draws the clicked row's footprint read-only on
  the board canvas colours, with a Top copper / Silkscreen / Courtyard legend
  and its pad count, pad 1 size and courtyard. A changed row draws the current
  and the new footprint side by side, each given room in proportion to its
  width so both come out near one scale, with the pin map: every current pad
  number must be on the new footprint. The footprints are read off the UI
  thread through the library adapters.
- Apply checks each change again against the part's live document and its row
  (the Properties path, `footprint_binding_commands`), then changes every open
  sheet in one `update` as one undo group (`UndoGroupKind::Footprints`): one
  Ctrl+Z on any changed sheet takes all of them back. A refusal on any sheet
  changes nothing. Nothing is written to disk; the sheets are left unsaved.
- The dialog does not block the editor, so a footprint can change behind it
  (Properties, an undo). Apply first compares every listed editable part's
  live footprint and binding with what the dialog read; if one differs, Apply
  is refused as stale and nothing changes. When every pick turns out to be a
  no-op, Messages says nothing changed.
- A multi-unit part is one row, its units grouped across sheets by the same
  rule the engine uses within a sheet (`group_into_parts`: one designator, one
  library symbol, different unit numbers; an unannotated `U?` part is never
  grouped). Apply changes every unit on every sheet, so one designator never
  gets two footprints; a part with any unit on a sheet that is not open, or
  read-only, is read-only whole.
- A sheet that is not open is listed read-only ("Sheet not open"): its parts
  are shown for the overview and changed by opening the sheet. This is the
  safer of the two existing patterns: unlike Replace and Number Schematic
  Sheets it loads no tabless engine, so nothing outside the open tabs changes
  and no closed file can drift between the read and Apply. A sheet written by
  a newer Signex is listed read-only too.

## View ▸ Annotations {#view-annotations}

A panel row (`MenuMessage::OpenAnnotationsPanel`, mockup row 165) right under
Navigator: it opens (or reveals) the Annotations panel, docked on the right
by default, and is checked while the panel is open anywhere. Always
available, like the other panel rows; without a project in view the panel
says so. It has no command id and no default key. The panel lists the
annotation blocks of the project's `notebook/*.snxnbk` files; see
[NOTEBOOK_ANNOTATIONS.md](NOTEBOOK_ANNOTATIONS.md).

## View ▸ Notebook Badges {#view-notebook-badges}

A toggle row (`toggle_notebook_badges`, mockup row 197) after AutoFocus: it
shows or hides the Notebook badges on every sheet and is checked while they
show, which they do by default. It is greyed with no sheet in view, as the
Design Notebook rows table above says; the catalog gives the command the
same rule (`RequiresDocument(Schematic)`), so the command palette greys it
too. B in the Signex Standard
profile, on a schematic; Classic keeps B for Place Bus. See
[NOTEBOOK_ANNOTATIONS.md](NOTEBOOK_ANNOTATIONS.md), "Notebook badges".

## Tools ▸ Repair Pin Junctions {#tools-repair-pin-junctions}

Right after Assign Footprints, Tools ▸ Repair Pin Junctions runs
`repair_pin_junctions` (maintainer decision 2026-11-19, `DECISIONS.md`):
greyed without a schematic in view, it adds an automatic junction dot at every
pin end of the sheet in view that touches a wire mid-span with no dot, as one
undo step, and says in Messages how many it added, or that there were none. A
touch where a dot would short the pin's own part, or where wires of two nets
meet (overlapping, crossing, or a bare T), is left alone. Opening a sheet that
holds such touches never changes it: Messages says "N pins touch a wire without
a junction", and that row has a Repair Pin Junctions link. The link carries the
sheet the row names (`diagnostics::MessageLink`): it brings that sheet's tab to
the front and repairs it, whatever sheet is in view; a sheet closed since is
refused with a notice (`panels::messages_row::row_action`).

## View ▸ Net Inspector {#view-net-inspector}

Runs `open_net_inspector` (#1269), which opens (or reveals) the Net Inspector
panel, docked on the right by default. Always available; with no schematic of
a project in view the panel says so. The row sits under the panel rows of
View since #2416 (it was in Tools); the key is `V N` in the default profile and
`Shift+I` in Classic.

- The list is every net of the active document's project, read from the
  cached stitched project net index (`signex_net::NetIndex`, the one net
  navigation and highlighting read; `handlers/net_nav/index_job.rs`), never
  derived again. While the panel is open, a project with no current index has
  one built off the UI thread; after an edit the last list stays shown until
  the new index lands.
- Each row shows the net's name, its first matching net class as a pill and,
  on the right, how many component pins it connects (Graphite mockup row 58).
  Named nets come first, then auto-named ones. The filter matches the net
  name or the class name.
- Clicking a net expands it, on the selection fill, to its pins, labels,
  power ports and sheet entries, each with a kind icon (power ports in the
  power colour, the kind named in its tooltip), a pin's name and its sheet. A
  long list shows six rows, then "+ N more pins".
- No project, a read in progress, an index that could not be read (with Open
  Messages) and a filter with no match (with Clear filter) each show a card.
  The summary footer stays under the list in both tabs. Clicking a member highlights the net on every sheet and
  goes to the member through net navigation (`go_to_member`), opening its
  sheet when needed.
- Two tabs at the top (#1274), each with the count of its list: By Net, the
  list above, and By Component. By Component lists every part of the project
  by designator, with its value, footprint and pin count, from
  `NetIndex::components` (the same index). The units of one designator are
  one part; a part not annotated yet is listed per symbol. Clicking a part
  expands it to every pin in pin-number order, each with its net, pin name
  and the sheet its unit is on; a pin wired to nothing reads
  "— unconnected". A long list shows six
  rows, then "+ N more pins". Clicking a wired pin goes to it and highlights
  its net, as a member click does. The filter matches the designator, value
  or footprint. The tab is panel state in the Model and is not saved. With a
  board in view there are no tabs.
- With a board in view (#2403) the panel lists the board's nets instead,
  from `pcb_geom::net_reports`: routed length, vias, pads and whether the
  net is routed (or how many connections are still open). Each row's Select
  and Highlight run what Select Net and Highlight Net run, for that net.

## View ▸ Unions {#view-unions}

Opens (or reveals) the Unions panel (`PanelKind::Unions`, named unions),
docked on the right. The row sits after Layers, among the panel rows, and
behaves like them; the panel is also in the status bar's Panels list
(DECISIONS 2026-11-19: every dock panel is listed in both). No key is bound
and there is no command id: the row sends `OpenUnionsPanel`, as the other
panel rows do. Always available.

- The panel lists the unions of the sheet in view (This sheet) or of every
  open sheet of the project (Project), filters by name or `#tag`, and locks,
  hides, renames, adds or removes members and breaks a union. Rules:
  `docs/SCHEMATIC_UNIONS.md`.

## View ▸ Compiled Design {#view-compiled-design}

Runs `open_compiled_design` (#1308), which opens (or reveals) the Compiled
Design panel, docked on the left as a tab beside Projects. The row sits under
Net Inspector; the panel is also in the status bar's Panels list. No key is
bound. Always available; with no sheet of a project open the panel says so.

- The panel shows the last compile of the active document's project: its
  hierarchy with every repeated sheet expanded into its channels (CH1, CH2,
  ...), each row with the parts and nets of that sheet occurrence, and a
  Nets mode listing every compiled net with the number of sheets it crosses.
  Rules and states: `docs/MULTI_CHANNEL.md`, "Compiled Design panel".
- `compile_project` (palette only, no menu row; Design category) compiles the
  project off the UI thread with the project's channel naming. It needs a
  sheet of a project (`Needs::ProjectSheet`, as Number Schematic Sheets). The
  panel's Recompile button and its Compile project button run it too. The
  result lives in memory until the project closes; nothing is written.
  Recompile is never automatic: an edit to a compiled sheet only marks its
  rows out of date.

## View ▸ Command Log {#view-command-log}

A panel row right after View ▸ Messages (#2458): it opens (or reveals) the
Command Log panel, docked at the bottom by default, and carries a check mark
while the panel is open, like the other panel rows. It runs
`MenuMessage::OpenCommandLogPanel`, not a catalog command, so it has no key
and is never greyed. The panel lists every catalog command the session ran,
from the GUI, the CLI and the agent socket; see
[COMMAND_LOG.md](COMMAND_LOG.md).

## View ▸ Extensions {#view-extensions}

A submenu after the built-in panel rows, before the separator that ends them
and the Window submenu (#2495). It shows only when an installed extension
added a dock panel: one row per extension panel, titled by the extension's own
message, which opens (or reveals) that panel and carries a check mark while it
is open, like the other panel rows (`MenuMessage::OpenExtensionPanel`, not a
catalog command, so no key). With no extension panel installed, View is the
built-in menu row for row; the Community build installs none. The panels are
also in the status bar's Panels list. See
[EXTENSION_API.md](EXTENSION_API.md), "Dock panels".

## Tools: board edits {#tools-board-edits}

Seven rows under Net Inspector (#2408, #2409) edit the board in view, each
as one undo step: Via Stitching (`stitch_vias`), Add Teardrops
(`add_teardrops`), Remove Teardrops (`remove_teardrops`), Gloss Selected
Routes (`gloss_selected_routes`), Board Shape from Selection
(`board_shape_from_selection`), Keepout from Selection
(`keepout_from_selection`) and Arrange in Box (`arrange_in_box`).

- Via Stitching and the two teardrop rows are live with a board open; with
  nothing selected they act on every pour or every net.
- Gloss, Board Shape and Keepout need something selected on the board in
  view; Arrange in Box needs unlocked components selected. Otherwise the
  rows are greyed with the reason.
- Gloss, Add Teardrops, Board Shape, Keepout and Arrange in Box are also on
  the board's right-click menu, above Cross Probe.
- Each command takes operands (`document` and the items by uuid) on the
  agent channel and the CLI; no keymap preset binds them.

What each one does: [PCB_TOOLS.md](PCB_TOOLS.md), "Board Tools".

## Tools ▸ Annotation ▸ Number Schematic Sheets {#number-schematic-sheets}

Runs `number_schematic_sheets` (#2247; editable dialog, maintainer decision
2026-11-09, `DECISIONS.md`). Greyed unless the active document (or, with no
document open, the active project's root sheet) belongs to a project; a loose
sheet has no project numbering. The dialog reads the active document's project off the UI
thread and lists every page in hierarchy order (the order and numbers of
`signex_net::sheet_numbering`, the ones PDF export prints), indented by depth,
with its file and its number.

- A page placed by a sheet symbol has a number field. A top-level page has no
  symbol to hold a number and keeps its position; it is shown read-only.
- Auto-number (footer left, and as the fix in the error banner) puts every
  page back at its position and clears the hand numbers. A number that is not
  a whole number from 1, a clash or a number above the page count greys
  Apply (mockup row 78): the clashing rows take the error fill and a red field,
  and an error banner names the clash or high number. A gap only warns, as a
  warning line under the table (Caner 2026-11-24). An automatic number
  (one that follows the hierarchy order) is drawn in faint ink, a typed one in
  full ink. Export still only warns.
- Apply writes each changed number into the `ChildSheet.instances` entry of
  the parent sheet's sheet symbol (`Command::SetChildSheetInstances`), scoped
  to the project name. A number equal to the page's position clears the entry
  instead. An entry with no project is shared by every project that uses the
  file, so it is never changed or dropped: the project's own entry is written
  ahead of it (a clear writes the position there). Every parent sheet takes the change in one undo group, so one
  Ctrl+Z on a changed sheet takes all of them back. A parent sheet that is not
  open is loaded into a tabless engine and left unsaved, as project Replace
  does; nothing is written to disk until the user saves.
- Apply is refused, with the reason in the dialog and the Messages panel, when
  a sheet was opened, closed or had a sheet symbol changed since the read, or
  when a parent sheet is being saved or moved, or is read-only (an open one,
  or a closed one written by a newer Signex), or when a closed parent's file
  changed on disk after the read: the read stamps each closed parent's file
  (modified time and length) just before reading it, and Apply states those
  files again on a worker before it writes; the numbers are frozen while it
  waits. These checks run before any
  sheet changes; if a sheet still fails part-way, every sheet already changed
  is put back and the sheets it loaded are dropped.

## View ▸ Window ▸ Tile Horizontally / Tile Vertically / Untile {#window-tile}

Run `tile_windows_horizontally`, `tile_windows_vertically` (#2247, #2249) and
`untile_documents` (#2249). Tile Horizontally stacks in equal rows, one above
the other, and Tile Vertically puts side by side in equal columns (Caner
2026-11-10).

In the main window, with two documents or more docked, Tile shows every docked
document (the tabs not in a window of their own, in tab-strip order) in a pane
of its own. The focused pane is the active tab, a full editor with the accent
ring; every other pane draws its document (sheet, board, symbol, footprint,
library browser, Project or Rules tab) with a hairline edge, and a press on it
makes that document the active one, as a tab click does. Untile shows the
active document alone again; closing or undocking down to one docked document
does the same, and a document opened later is not tiled again. Untile is
greyed unless the documents are tiled.

With undocked document windows, Tile also arranges the main window and every
undocked window (main window first, then the undocked ones by file path) on
the main window's monitor. Each window is un-maximized, resized and moved with
`iced::window` tasks; this part waits until the main window's monitor has been
read (the settled geometry reading, shortly after start-up). iced reports the
monitor's full size, not its work area, so the tiles cover the whole monitor.
Tile is greyed when it has neither to do. The Messages panel names how many
documents or windows were arranged.

## View ▸ Window ▸ Split {#window-split}

Mockup row 207 (#1307). The main window's document area splits into panes
that hold tabs, beside Tile's one pane per document.

- **Dragging a tab onto the document area.** Past the tab strip's drag
  threshold, a tab held over the outer quarter of the document area (left,
  right, top or bottom) fills the half that would receive it with
  `accent-soft` and a 2 px accent border, named "Split right · <tab>" (or
  "Second view · <tab>" while Ctrl is held). Letting go there splits; the
  middle draws nothing and keeps the tab where it is.
- **What a drop does** (decided as drawn): the document moves into a new
  pane on that edge, which takes the focus; every other tab stays in the
  pane it was in. With Ctrl held (Cmd on macOS) it opens a second view
  instead: the document stays where it was and shows in the new pane too.
  Moving the only tab of the main window is refused (its pane would be
  empty); a second view of it is not.
- **Panes.** A tab no pane lists sits in the first pane, so a document
  opened while split joins it. A pane that is not focused draws the tab it
  showed last, and a press on it moves the focus there (a second view of the
  active document only takes the focus). Clicking a tab in the strip moves
  the focus to the pane holding it. Up to four panes, all in one direction:
  a drop across them (top or bottom while side by side) is not offered.
  Closing a pane's last tab removes the pane; one pane left ends the split.
- **Menu rows and commands.** Split Right (`split_pane_right`) and Split Down
  (`split_pane_down`) move the active document into a new pane on the right
  or below; Open Second View (`open_second_view`) opens it on the right as
  well. Split Right and Split Down share Tile's gate (greyed with fewer than
  two documents and no undocked window); Open Second View needs a document.
  A split that cannot be made (across the panes, past four, the only tab) is
  said in Messages. Untile merges every pane back into one; Tile Vertically
  or Tile Horizontally replaces the split with one pane per document. The
  three commands are on the agent channel with Tile and Untile, answer with
  the same layout JSON, and are refused with the split's reason.
- **Kept per project** (decided as drawn). A drop or a pane command keeps
  the split for the active tab's project, per user, in `prefs.json` under
  `document_panes`, keyed by the project file:
  `{"direction": "side_by_side", "panes": [{"id": 0, "tabs": [{"path": "power.snxsch", "page": "schematic"}], "shown": {…}}, …], "rest": 0, "focused": 1, "next_id": 2}`,
  documents relative to the project folder. Each pane has an id that stays
  with it while the split lasts (an entry without ids reads them as the
  panes' places; two panes with one id are refused), so an earlier pane
  closing never moves the view kept for a later one to another pane. The
  split comes back, once a
  session, when two of its panes have a document open again. Untile, or
  Tile's one pane per document, forgets it; closing the documents does not.
  An entry this build cannot read is left as it is, and a key it does not
  know survives a rewrite of its entry. A loose file keeps nothing. The
  write runs off the UI thread through the ordered preference writer; a
  failure is said in Messages. Pane sizes after a splitter drag ride in the
  entry as `"shares"`, whole thousandths per pane (a grid: its first
  column and first row); equal panes write none.
- **A tab let go over the canvas.** The canvas takes the pointer's moves and
  the release while a tab is dragged over it. The drag subscription still
  hears both (`app/bootstrap/drag_events.rs`): a move updates the drop
  target, and a release over the document area ends the drag there,
  splitting when it is over an edge. A release over the tab strip is the
  strip's own (a reorder).
- **Not built**: Merge All Panes as its own row and Ctrl+Alt+M (Untile does
  the merge, DECISIONS 2026-11-26), and Tile Vertically's grouping by
  document kind (Tile keeps one pane per document, DECISIONS 2026-11-16).

## Panes {#panes}

Mockup row 207 (#1307): how the panes look and what they do, whether made
by Tile, Tile Grid or a split.

- **Tile Grid** (`tile_windows_grid`, View ▸ Window after Tile Vertically,
  no default key, Tile's gate). Up to four panes in a grid, two to a row;
  with three, the third spans the bottom row. Up to four documents get one
  pane each. With more, one document kind each where it can: the tabs are
  grouped by kind (sheet, board, library, and so on) in the order the kinds
  first appear, the largest group is halved while there are fewer than four
  panes, and the last groups share a pane while there are more. The active
  tab's pane has the focus, and documents opened later join it. A grid is a
  split: it is kept for the project as `"direction": "tile_grid"`, a drop on
  an edge is not offered, and Untile merges it. With undocked windows, Tile
  Grid also arranges the main window and those windows in a grid on the
  main window's monitor.
- **Tab strip.** Each pane has its own strip on top, 26 px: the pane's tabs
  in strip order and, at the right end, the ⋯ button of the pane menu. The
  tab the pane draws carries a 2 px line on top, the accent on the focused
  pane and grey on the others. A press on a tab focuses the pane and shows
  that tab there. The window's own tab strip stays above the document area.
- **Pane menu** (⋯): Split Right, Split Down, Move Tab to Other Pane,
  Untile (Merge All Panes), Close Pane, Sync Zoom with Other Pane and Equal
  Pane Sizes, greyed as their commands are. A row first focuses its pane,
  then runs its command, as the palette does. A press outside closes it.
  - Move Tab to Other Pane (`move_tab_to_other_pane`): the active tab moves
    to the next pane (the first after the last), which takes the focus. A
    pane left with no tab closes; refused when one pane would be left.
  - Close Pane (`close_pane`): the focused pane closes and its tabs join the
    next pane (the one before, for the last). No document closes. Down to
    one pane the split ends and the project forgets it, as Untile does.
  - Sync Zoom with Other Pane (`sync_pane_zoom`): every other pane that
    shows a sheet or a board takes the focused pane's zoom, about its own
    centre. Refused when the focused pane shows neither.
  - Equal Pane Sizes (`equalize_panes`): the panes share the area equally.
- **Splitters.** The line between two panes drags (a 6 px grip; a grid has
  a column line and a row line); each pane keeps at least 80 px. A double
  click makes the panes equal. The sizes are kept with the project's split.
- **Each pane keeps its own view.** The focused pane draws on the main
  canvases and every other pane on a canvas of its own. When the focus moves
  to another pane, the main canvas takes that pane's pan and zoom, and the
  pane left behind keeps the view it had. Two views of one sheet also keep
  their own selection.
- **Docks follow the focused pane.** Properties, Messages and the other
  panels show the focused pane's document and, for two views of one sheet,
  that view's selection.
- **Cross probe across panes.** While Cross Select Mode is on, what the
  focused pane has selected is outlined in every other pane on the other
  side: a part on the sheet outlines its footprint on a board pane, a
  footprint or a net on the board outlines its symbol or members on a sheet
  pane. The outline is drawn again only when the selection, the panes or the
  board change.
- **Hover preview.** Hovering Tile Horizontally, Tile Vertically or Tile
  Grid shows a card beside the row: "Tile Vertically · 4 open documents",
  the panes in their arrangement with the names of the documents each would
  receive, and what the arrangement does. The native macOS menu shows none.
- **Commands.** All five are GUI only, in the palette and on the agent
  channel; they answer with the layout JSON (see `docs/AGENT_SOCKET.md`) and
  are refused with the pane's reason.

## View ▸ Window ▸ Layouts {#window-layouts}

Workspace layouts (mockup row 166), between the tiling rows' separator and Close All
Documents: the [header](#header-entries) BUILT-IN, the four built-ins Schematic,
Layout, Review and Notebook (`apply_layout_schematic`, `apply_layout_board`,
`apply_layout_review`, `apply_layout_notebook`; check rows, the active
layout checked), then, when you have saved any, the header YOUR LAYOUTS
and one row per layout of yours in your order (`MenuMessage::
WorkspaceLayoutChosen` with the layout's id, checked while active), a
separator, Save Layout… (`save_workspace_layout`), Manage Layouts…
(`manage_workspace_layouts`), and Reset "<name>" (`reset_workspace_layout`)
only while the active built-in carries your edit. Every row other than the
two headers is live in every context; none has a key by default. The native macOS bar shows the same
three-level submenu. Rules, dialogs and storage:
[WORKSPACE_LAYOUTS.md](WORKSPACE_LAYOUTS.md).

## Sheet right-click menu {#sheet-context-menu}

The schematic canvas menu (#2419, Graphite mockup row 62, `app/view/context_menu/menus.rs`) shows
rows by what was right-clicked and makes each command row live by the
command's own enablement rule (`app/command/needs.rs`), the rule the menu bar
greys the same row by and a keystroke is dropped by. A muted first line names
what the menu stands on: `schematic sheet · <file>`, or the selection
(`R3 · 100k · 1 item selected`). Rows that need a selection are hidden, not
greyed, on an empty spot.

With nothing selected, top to bottom: Find; Find Similar Objects only when the
right-click hit an object; the Place and Sheet Actions submenus; Undo, Redo;
Paste, Paste Special, Select All; Highlight Net (`highlight_net_under_cursor`)
only when the right-click hit an object; Net Color... (`open_net_color_palette`);
Clear Filter (`clear_net_highlighting`) only while a net highlight is on;
Project Options, Preferences.

With a selection: Properties (bold, the default row), Find Similar Objects,
Cross Probe; Cut, Copy, Duplicate, Delete; Rotate, Mirror X, Mirror Y, the
Align submenu, Arrange (a drawing selected) and the Unions submenu; the Part Actions,
In variant (when the project has a variant besides Base), Sheet Actions
and Place submenus; Highlight Net, Net Color..., Assign Net Class..., Clear
Filter while a net highlight is on; Undo; then a greyed "Planned (N)" submenu
that opens on hover and lists the planned rows below.

The Place submenu groups its rows under Wiring, Ports and power, Parts and
sheets, and Directives and text. Its power rows show the short port name in
the power colour, with the full name as the tooltip. The planned tools of
`active_bar::PLANNED_TOOLS` it would list are gathered in one greyed "N planned
tools…" row whose tooltip names them.

Part Actions holds Open Datasheet, Edit Symbol and Change Component, then
Autoplace Fields and Copy Attributes (#1667), live with one placed part
selected. Copy Attributes copies the object at the point the menu was opened
over, else the selected part. Swap Pins and Swap Gates (#1261) follow: Swap
Pins is live when the clicked pin of the selected part has one other pin in
its swap group, Swap Gates with two units of one part selected, or one unit
whose gate swap group has one other gate placed on the sheet (see
`docs/PCB_TOOLS.md`, "Pin and gate swap"). Sheet Actions holds Open Child Sheet, Create Sheet
From Sheet Symbol, Sync Sheet Pins and Synchronize Sheet Entries and Ports...,
live with a sheet symbol selected, then Create Sheet Symbol From Sheet... and
Number Schematic Sheets (#1276). When the symbol's file is missing, Open Child
Sheet is disabled with the reason, a double-click or the harness panel's Go to
logs the same reason and opens nothing, and Create Sheet From Sheet Symbol makes
the file (#2847).

In variant: <name> (mockup row 62) names the project's active variant, Base
when none is, and shows only when the project defines a variant. Its flyout is
headed "<part> in <variant>" for one selected part. On a repeated sheet Fitted
and Not fitted set this channel's copy or every channel's (#2662); on a sheet
that is not repeated they are greyed with a "Planned" tooltip. Alternate…
opens the Alternate part picker and Edit value for this variant… a popover on
the part (mockup rows 208, 209; `docs/PROJECT_TAB.md`, "Alternate part and
variant value on the canvas"); both are greyed in Base, without one part
selected, and on a repeated sheet before a compile, each saying why.
Show in Variant Manager opens the project's tab on its Variants section; New
variant… opens it and adds the first free "Variant N" from Base with its name
open for typing, as the section's `+` does.

Unions (mockup row 158) holds Create Union… (`create_union`, live with a selection,
`Ctrl+G` in the default profile), Add Selection to Union (`add_to_union`, the
union picked in the Unions panel), Remove Selection from Union
(`remove_from_union`) and Break Union (`break_union`), the last two live when
the selection holds a member of a union. See
[SCHEMATIC_UNIONS.md](SCHEMATIC_UNIONS.md).

Paste Special (`smart_paste`, mockup row 155) opens the Smart Paste dialog in a window:
one copy at an offset or an array of copies, the designators Next free or
kept, the net labels kept or rewritten with a pattern (power nets never). A
rewritten name that already exists is named in the dialog and stops Paste. So
does, when the paste rewrites a net name, a project sheet that cannot be read,
a file a sheet symbol places that is missing from disk, or a sheet reference
that is refused: their names are unknown, so the dialog names the sheet. A
missing declared page no symbol places does not stop it, and keeping the net
names rewrites nothing and is not stopped. Its Paste puts every copy on the cursor and one click places them as one undo
step; Esc cancels. A headless run of `smart_paste` places the copy at once
beside the sheet's content, as before; it keeps the copied net names, so an
unread sheet does not stop it. With operands (`at`, `delta`, `copies`,
`designators`, `nets`, `net_pattern`, `net_start`, #2665) it places the
dialog's choices at once instead, with the dialog's checks; see
[CLI.md](CLI.md#smart-paste). Duplicate keeps its fixed offset.

## Bottom-up hierarchy {#bottom-up-hierarchy}

Design ▸ Create Sheet Symbol From Sheet... (`create_sheet_symbol_from_sheet`,
#1276) asks for a `.snxsch` file, reads it off the UI thread and places a sheet
symbol for it on the sheet in view as one undo step: named after the file
(`name_2`, ... when taken), referring to the file relative to this sheet, at
the cursor's last point on this sheet (else under the sheet's content), with an
auto-generated entry for every port of the file (outputs on the right edge,
every other direction on the left). It refuses the sheet itself, a file that
places this sheet, a sheet above this one at any depth, and a file this sheet
already places. The sheet in view is the one the command was asked on (an
undocked window's sheet when picked there, #2267): the symbol lands on it
whichever tab is active when the file is read, and nothing is placed, with a
line in Messages, when that sheet was closed meanwhile. Both commands also run
from operands; see [CLI.md](CLI.md#bottom-up-hierarchy).

Design ▸ Synchronize Sheet Entries and Ports... (`sync_sheet_entries`) opens a
dialog for the selected sheet symbol. Choose the side that wins: **Sheet to
sheet symbol** changes the symbol's entries, **Sheet symbol to sheet** changes
the sheet's hierarchical ports. The dialog draws the written side after Apply
or as it is now (After / Before, with an expand button) and lists each entry
that would be added, removed or renamed with its I/O type. None of this changes
a sheet: Apply makes exactly the listed changes as one undo step.
Esc and Cancel close it without a change. A rename is read only when, for one
direction (input, output, ...), exactly one name is gone and one is new; it
keeps the entry's place and wires, or the port label's place. Sheet to sheet
symbol renames and removes auto-generated entries only, as Sync Sheet Pins
does, and renames on the selected sheet symbol only: a hand-placed entry, or
an entry of another symbol of the same file, keeps its name; Sheet
symbol to sheet removes and renames Hierarchical labels only and never touches
a Global label. A sheet that is not open is changed in memory, left unsaved,
and refused if its file changed on disk after the dialog read it. The sheet
symbol's sheet is the one the command was asked on (an undocked window's sheet
when picked there, #2618): Apply edits it whichever tab is active, and nothing
is changed, with a line in Messages, when that sheet was closed meanwhile. The
rules: `signex_engine::sheet_sync`; decisions: `DECISIONS.md` (#1276).

Assign Net Class... is greyed: `assign_net_class` takes the net and the class
as operands, which the menu cannot supply yet; its tooltip points at the Net
Class row of Properties. The "Planned (N)" submenu holds Snippets and Comment
with the shared planned tooltip, and these rows, greyed with a tooltip saying
what each will do:

- **References**: list where the selected objects are used across the project (designators, net labels, sheet entries).
- **Pin Mapping**: edit the selected part's symbol-pin to footprint-pad map, the map Update PCB reads.
- **Supplier Links**: link the selected part to supplier part numbers and prices through the library's distributor adapter.

## Rows still planned {#rows-still-planned}

Every other menu row runs a command (#2247). The only greyed tool rows are the
planned tools of Place on a sheet and on a board, each with its reason.

Place on a sheet has six planned commands (#2415; Bezier left the list with
#1267 and Parameter Set with #1489), greyed with the planned
tooltip in every state until their tools are built
(`menu_bar/model/sheet_place.rs`, `active_bar::PLANNED_TOOLS`). The three
harness rows went live with the harness tools (#1280, see
`docs/SIGNAL_HARNESSES.md`):

| Submenu | Row | Command |
|---|---|---|
| Ports & Connectors | Device Sheet Symbol | `place_device_sheet_symbol` |
| Ports & Connectors | Reuse Block... | `place_reuse_block` |
| Graphics | Elliptical Arc | `draw_elliptical_arc` |
| Graphics | Ellipse | `draw_ellipse` |
| Graphics | Round Rectangle | `draw_round_rectangle` |
| Graphics | Graphic... | `place_graphic` |

Each one is the row of a planned Active Bar tool. It is not a
`stub()` row: it names its catalog command and goes live when that tool is
built, with no menu change.

## Save and Save As {#file-save}

Both are also enabled for a standalone symbol or footprint editor tab
(`.snxsym` / `.snxfpt`), which `save_active_document` already handles, so the
gate is the command's enablement, not "a schematic is open".

## Edit ▸ Move {#edit-move}

Edit ▸ Move (#2240) groups the commands that move, turn and mirror the
selection: Move (`move_object`), Move Selection (`move_selection`), Rotate
(`rotate_counterclockwise`), Rotate Clockwise (`rotate_clockwise`), Mirror X
and Mirror Y (`mirror_x` / `mirror_y`) and Flip Selection (`flip_selection`).
They run the same ids as the keys, the palette and the canvas menus, on a sheet
and on a board alike.

On a sheet a row is live when there is something to act on: a selection that
is not all locked, or for Rotate and Mirror a part on the cursor or the Bus
Entry tool. A selection of locked objects only greys Move Selection, Rotate,
Mirror, Cut and Delete (#2419); their keys still reach the handler, which skips
the locked objects and names them in the Messages panel. Move needs an
open sheet or board. On a PCB tab the rows act on the selected footprints (a
selected pad stands for its footprint). They are greyed with nothing selected
and when a selected footprint is locked; the key for a greyed row on a locked
footprint still reaches the handler, which names the locked footprint in the
Messages panel and changes nothing. Mirror X (the `X` key) flips a footprint
to the other board side, like Flip Selection; Mirror Y flips it and turns it
half a turn, so it mirrors top to bottom. A moved footprint takes the track
ends on its pads along (#2379), unless Ctrl is held during a drag; vias stay.
Tracks and vias on a rotated, flipped or deleted footprint stay where they
are. Flip Selection is `L` on a board in both built-in profiles
(#2406), which also flips a part on the cursor, and is greyed outside a board.

Edit ▸ Arrange orders schematic drawings, so it is not in the Edit menu while
a board is in view (#2405); the golden fixture marks its rows `-` in the
`pcb` column.

## Edit ▸ Origin {#edit-origin}

Edit ▸ Origin (decided 2026-11-18) follows Move and Arrange: **Set**
(`set_board_origin`) arms a click on the board that becomes the origin, on
the non-component grid; Esc cancels it. **Reset** (`reset_board_origin`)
moves the origin to the bottom-left of the board outline, or of the board
area without one. **Snap to Outline Corner** (`snap_board_origin_to_corner`)
moves it to the outline corner nearest the pointer; before the pointer was
on the board, Messages says to point at it first. Each is one undo step on
the board's options and marks the board changed. The rows are shown on a
sheet too, greyed, and are greyed on a read-only board. Set is `E O S` in
the chord profile and `Shift+O` in Classic. The board's right-click menu
does not list them.

## The board's right-click menu {#board-context-menu}

The board canvas menu (#2240, #2405) follows what is under the pointer
(maintainer request 2026-11-18, `app/view/context_menu/board_menu.rs`), as
the sheet menu shows its selection rows only with a selection. On an empty
area with nothing selected it lists, in order: Find; the Place ▸ flyout (the
board's tools as Place on a board lists them, then Board Sketch); Undo and
Redo; Paste, Paste Special and Select All; Board Sketch and the reuse block
Place rows; then Toggle Ratsnest. On an object or with a selection, Find
Similar Objects comes first and, with a selection, Align ▸ (the Align rows of
the PCB Active Bar, live for unlocked footprints), Cut, Copy and Duplicate,
Move Selection, Rotate, Rotate Clockwise, Flip Selection, Mirror X and
Mirror Y, Save as Reuse Block, Delete, Board Shape and Keepout from
Selection follow. Change Footprint, Swap Pins, Swap Gates and Arrange in Box
show only with a footprint selected; Lock, Unlock, Gloss Selected Routes and
Add Teardrops only with a track or via selected; Select Net
(`select_board_net`) and Highlight Net (`highlight_board_net`) only when the
selection or the object under the pointer has a net; Cross Probe,
Properties and Compare with Schematic (`compare_board_with_schematic` under
a shorter name, see [above](#project-compare-board)) with an object. Each row
runs its catalog command and is live exactly when that command is enabled in
the window's `CommandContext`, so the menu greys what the keys and the menu
bar grey. A board written by a newer Signex is read-only
(`PcbTargets::read_only`): every row that edits it, the Place tools and
Board Sketch included, is greyed, and the rows that only read it stay live.

## Edit clipboard on a board {#edit-clipboard-board}

On a PCB tab (#2361) Cut, Copy, Paste, Paste Special, Duplicate and Select
All act on the board, from the Edit menu, their keys and the board's
right-click menu. Copy and Duplicate are live with footprints, tracks or vias
selected; Cut also needs no selected footprint to be locked. Paste and Paste
Special are live once board objects were copied. Select All is live on any
board and selects every footprint, track and via on a shown layer.

Paste puts the copy on the pointer, on the board grid, and a click places it;
Esc or a right-click drops it. Paste Special and Duplicate place the copy at
once, shifted from where it was copied. Each edit is one undo step, and the
placed copy is selected. A pasted footprint whose designator is taken gets
one by the paste designator preference, as on a sheet; a pasted pad, track or
via keeps its net only when the board has a net of that name. One clipboard
serves sheets and boards: a copy on one replaces the other's, and pasting
the other kind does nothing and says why in the Messages panel.

## View: board view {#view-board}

Five View rows act on the board in view and are greyed without one (#2223):
Toggle Ratsnest, Next Layer, Previous Layer, Toggle Layer Visibility (the
active layer) and Show All Layers. Next and Previous Layer show the keys of
the active keymap: `Shift++` and `-` in the Signex Standard and Classic profiles, which
bind `+`, `Shift++`, `*` and `Shift+*` to Next Layer and `-` to Previous Layer
in the `pcb` context. The Layers panel row sits with the other panel rows and,
like them, works without a document (the panel shows its empty state). In an
undocked PCB window the keys and rows act on that window's board. What a row
changes is the board view in `UiState::pcb_layer_views`: never the board,
never its file. See `docs/DECISIONS.md` (2026-11-09).

Toggle Origin Marker (`toggle_origin_marker`, #2351) sits under Toggle
Ratsnest: it shows or hides the circle-and-X marker at the board origin,
which is shown by default. It is greyed without a board, has no default key,
and like the rows above it changes only the board view.

Every board canvas menu ends with a View group holding the same Toggle
Ratsnest row (#2240, #2223); the menu opens only on a board, so the row is
always live there.

Four more View rows turn the board view on screen, requested by the
maintainer in #2316: Rotate View 90° Clockwise (`view_rotate`), Rotate View
90° Counter-Clockwise (`view_rotate_ccw`), Rotate View 180°
(`view_rotate_180`) and Reset View Rotation (`view_rotate_reset`). They are
greyed without a board, Reset also while the view is at 0°, and they have no
default keys. Any other angle is typed in the status bar: a click on its View
readout opens a focused field, Enter applies a number (other text keeps the
field open), and Esc or a second click closes it. The field also closes when
the view turns by another route and when the tab leaves the board. The turn
is view state like pan and zoom: it lives in the board canvas's camera, makes
no undo step and does not mark the board dirty; a board opened into a tab
starts at 0°. In an undocked PCB window the rows, the palette, the keys and
its own status bar act on that window's board and leave the main window's as
it is. See `docs/DECISIONS.md` (2026-11-12).

View ▸ Home View (`home_view`, mockup row 194) sits after Fit All. It does
what the Home button left of the canvas zoom pill does: on a board, rotation
0°, Flip off, then Fit; on a sheet, Fit. It is live with a sheet or a board in
view, acts on the main window, and has no default key. See
`docs/PCB_TOOLS.md`, "Orientation tile and Home View".

Two View rows come from the layer tab menu (#2394): Show Layer Numbers
(`toggle_layer_numbers`) under Show All Layers, a per-user setting of the
layer tabs, and Flip Board View (`flip_board_view`) under Reset View
Rotation, which shows the board from below. Both are greyed without a board
and have no default key; the flip is board view state like the turn. The
other rows of that menu take a layer, a kind or a set name and are run from
the menu, the layer set button or a command with operands
(`docs/PCB_TOOLS.md`, "Layer sets").

3D View (`view_board_3d`, #2390) follows Flip Board View: it shows the board
in view in 3D in the main window, or flat again. It is greyed without a board
and bound to `3` on a board in both built-in profiles. In 3D a floating
toolbar at the top centre of the view holds the "3D View" title, the 2D | 3D
switch (2D shows the board flat again), the viewpoints Top, Iso, Front, Left,
Right and Back, Reset camera and Fit, Flip and Explode, the lighting
preset and the screenshot button with its menu (Copy to clipboard, Save
PNG…). View ▸ Fit All frames the 3D view too. See `docs/RENDERING_RULES.md`,
"board-3d".

## Native macOS menu {#native-macos-menu}

On macOS the main menu lives in the system menu bar (NSMenu), described as
data by `iced::menu::Menu` from the same `menu_bar_model` the in-window row
renders, so a model edit shows up natively with no change to the native code.
Windows and Linux keep the in-window row; the iced runtime never asks them
for the native menu.

**Layers.** `menu_bar/native/mod.rs` turns the model into an
`iced::menu::Menu<Message>`: titles, labels, check marks, separators,
submenus, and for each live row the message it sends. It compiles on every OS
and carries the tests. `Signex::native_menu` (`app/handlers/native_menu.rs`)
reads the Model and builds it; `main.rs` hands it to `iced::daemon(..).menu`,
and the iced fork (alplabai/iced, `iced::menu`) syncs it to the NSMenu after
every `update`: flags (enabled, checked) in place, rows rebuilt only when
they changed, a stable id per row, a failed rebuild keeping the old menu.
Signex has no AppKit or `muda` code of its own.

**Build timing.** `main.rs` sets `Settings::default_menu: false`, so winit
installs no default menu and Signex needs no "install after the main window
opened" step: the iced runtime installs the menu once the application has
launched, and the in-window row is already left out from the first frame.

**App menu.** The first menu, titled by the process: About Signex (a custom
row, `MenuMessage::OpenAbout`), Preferences... (a custom row with the one real
key equivalent, Command+comma), Hide / Hide Others / Show All and Quit (the
stock items, `iced::menu::Predefined`).

**Quit.** `Settings::exit_on_quit_request` is `false`, so every quit the
system asks for reaches the app as `iced::system::quit_requests()`: Command+Q
and the app menu's Quit, Dock ▸ Quit, and logout (#2224; the old custom Quit
row guarded only the menu item and Command+Q, and the other two sent
`terminate:` past the guard). The subscription (`QuitRequested`) is always
running, since the runtime answers a request nobody listens to with "quit" at
once. It runs `handle_app_quit_requested`, the same unsaved-changes guard File
▸ Exit and the window close use: nothing unsaved closes the main window, which
ends the process (the runtime answers the pending request when the app exits);
otherwise the prompt opens. A request stays pending until it is answered, and
macOS waits (at logout only for a limited time), so every path that keeps the
app open answers it with `iced::system::cancel_quit()` (a no-op when nothing
is pending): the prompt's Cancel, a quit held back by a running rename or
delete or by a New Project still being written (#2280), a Save All that left
files unsaved (also when a write task panicked and ended the writes early: the
settle still arrives, #2269), and the prompt closing itself
because everything was saved behind it, and a prompt window that vanishes
without a close request (treated as Cancel). Each decision is logged to the
Messages panel ("Quit: the system asked Signex to quit", "Quit: not quitting,
Signex stays open"). The second line is written on every OS whenever a quit
is declined, a system request or not.

**Check marks.** The View menu's panel rows carry `MenuItem::checked`, read
from `MenuContext::open_panels` (`Signex::open_panels`, the same
`is_panel_open` answer the status-bar panel list uses), and become check rows
(`Item::checked`). AppKit flips a check item itself on a click; the fork
writes the flags of a chosen row again on the next sync, so the mark is
redrawn from the Model. A panel row opens or reveals its panel; the panel's
shortcut toggles it.

**Shortcuts on model rows are text, not key equivalents.** Each native row
shows its keymap shortcut (`Shortcut::text`, from the active keymap, so a
rebind shows up, and the changed label makes the fork rebuild the row) as
text after its label, four spaces apart. It registers no AppKit key
equivalent (no `Item::shortcut`): one fires before iced sees the
key, which bypasses the widget-capture rule in `app/dispatch/input.rs` (a
focused text input owns Ctrl/Command + C, V, X, A, Z), and the keymap treats
`Ctrl` as the physical Control key on every OS (#1441), so a native Command
key equivalent would disagree with the profile. Keys keep going through the
keymap, so a key is never dispatched twice. The four app-menu key equivalents
(Preferences' Command+comma and the stock Quit, Hide and Hide Others) are the
only registered ones, and none collides with editing.

**Exit.** The native File menu leaves out Exit and the separator above it:
Quit in the app menu replaces it. Windows and Linux keep Exit.

**Ids.** The fork derives a row's id from its label path, `File/Save` or
`File/Export/PDF` (with the shortcut text, which is part of the label), never
from an index, so an event that arrives after a rebuild cannot fire a
different row; a repeated label within one menu gets a `#2` suffix, which
matters for Open Recent, where two projects can share a file name. Each
Open Recent row carries its own project path in its message.

**Activation.** A model row's message is
`WindowMsg::NativeMenuRowChosen(MenuMessage)`; `update` refuses it while a
confirm dialog is open (a backstop for a click on a row the bar had not yet
greyed), routes it to the focused window as below and dispatches. A greyed row
has no message, so a disabled row cannot fire. The app menu's About and
Preferences send `Message::Menu` directly, so the lock never blocks them.

**Focused window.** The bar acts on the focused window (`UiState::focused_window`,
fed by window focus events), falling back to the main window. Enablement comes
from that window's `CommandContext`. The per-window rows (Undo, Redo, Save,
Save As, Print, Export ▸ PDF, Close Document, the tab stepper) from a non-main
window route through `UiMsg::MenuInWindow`, as the in-window row does (#1546,
#1794, #1813), and Save, Save As and Print enable by that window's own document.

**Confirm dialogs lock the bar.** While a confirm or warning dialog is open
(`Signex::has_locking_confirm_modal`: quit and project-close confirms, error
notice, netlist-incomplete prompt, rename, remove, annotate-reset, close-library
and library-delete confirms, library and autosave recovery) the model rows are
greyed and refuse activation; About, Preferences and Quit stay live. Tool
windows (ERC, BOM, Preferences, Annotate, Print Preview, calculator, find /
replace, shortcuts) do not lock it.

**Sync.** The runtime calls `Signex::native_menu` after every `update`
(macOS only), so the menu reads the Model like a view: `CommandContext` of the
focused window, recent projects, the lock, open panels, the keymap and the
language. The built menu is cached in `NativeMenuState` with the
`MenuInputs` it came from and rebuilt only when one of them changed (the
runtime asks after every update batch, canvas mouse moves included); the fork
then decides what, if anything, to touch in AppKit.

**Window strip.** With the native menu in use the in-window row and the
wordmark are not drawn from the first frame on (so they do not flash before the
first install): the strip is the traffic lights, the drag zones and the centred
search.

**Fallback.** The app subscribes to `iced::system::menu_failures()`, which
fires once for each distinct menu the runtime could not build, with the error
text. A failed first install leaves no menu bar at all (`default_menu` is
off), so on a report `WindowMsg::NativeMenuFailed` sets a sticky flag
(`NativeMenuState::fall_back`), logs the reason as a warning in the Messages
panel, and `hides_in_window_menu()` turns false, so the in-window row and the
wordmark come back. The flag is not cleared if a later menu builds, so the
bar does not switch back mid-session. A failed rebuild of a later menu leaves
the previous system menu in place, and the in-window row shows as well.
