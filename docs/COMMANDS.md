# Signex command reference

Every command the application exposes by a stable id, with the default
binding each shipped keyboard profile gives it. A keymap profile binds
keys to these ids — see [KEYBOARD_SHORTCUTS.md](KEYBOARD_SHORTCUTS.md)
for the file format.

**Generated — do not edit by hand.** Produced from
`crates/signex-commands/src/catalog/` and
`crates/signex-app/assets/keyboard-shortcuts/` by
`crates/signex-app/tests/integration/command_reference.rs`, which fails if this
file drifts. Regenerate with:

```sh
UPDATE_DOCS=1 cargo test -p signex-app --test integration command_reference
```

A command listed here resolves in the keymap. Not all of them reach an
action yet — the ones that do not are pinned in
`crates/signex-app/src/app/command/bridge.rs`, and that set may only
shrink.

## Running a command from a script or an agent

A command run by id can take `key=value` operands in place of a click or a
dialog answer. A command that acts on a document (`undo`, `redo`,
`save_document`, `save_document_as`, `close_active_document`, `color_net`,
`clear_all_net_colors`) names it with the required `document` operand, an
id or an absolute path, and acts on that document alone: never the active
one, and it changes no tab, focus, panel or selection. A person's palette or
keybinding call has no operands and keeps the active document.

`list_documents`, `document_info`, `resolve`, `find_items`, `read_document` and
`query_pins` are read-only queries that answer with data
(`CommandOutcome::Answered`); `find` is still the Find dialog.
`query_pins document=<doc> designator=<ref>` lists the pins of a part on
one sheet: number, name, electrical type, hidden, unit, shared and the tip
in mm. `check_erc document=<sheet> [baseline=<run>]` is a query too: it
runs ERC on the sheet's project, answers with a check report once the run
lands and writes nothing to the ERC panel or Messages;
`check_drc document=<board> [baseline=<run>]` does the same for DRC on
one board. Rules in [CLI.md](CLI.md), "Checks on demand".
`check_requirements document=<project or one of its documents>
[baseline=<run>]` runs the project's requirements list as checks and
answers in the same shape; rules in [CLI.md](CLI.md), "Requirement
checks".
`list_annotations document=<project or one of its documents> [id=<block
id>] [kind=<kind>] [part=<designator>]` answers, once the notebook files
are read, with every annotation block of the project's Design Notebook
and its chapters (mockup row 198): per file its blocks, each with its
typed id (`DEC-002`), kind, title, status, date, line, references, the
review record of its kind (a Measurement's rows with their `pass`,
`warn` or `fail` verdict) and `cited_by` (the Issues it opens, the
Decision that supersedes it, the Calculations reading it), and its
problems by name (`bad_id`, `duplicate_id`, `bad_field`, ...); then every
citation (`links`) and the failing Measurements that open no Issue yet
(`unopened_failures`). Fields in
[NOTEBOOK_ANNOTATIONS.md](NOTEBOOK_ANNOTATIONS.md), "Review records".
`notebook_measurements`, `notebook_timeline` and `notebook_attachments`
read the named project's Design Notebook once its files are read: every
reading with its result, one net's, part's or pin's timeline, every
attachment with its stale mark; `export_notebook_timeline`,
`mark_notebook_attachment_valid` and `attach_notebook_file` write a file.
Rules in [NOTEBOOK_MEASUREMENTS.md](NOTEBOOK_MEASUREMENTS.md).
`measure_board`, `query_board_nets` and `find_similar_items` are board
queries: a distance on one board, point to point or edge to edge between
two items; each net's routed length, members and routed state; and the
objects that agree with an item on the properties named.
`select_board_net` and `highlight_board_net` take an optional
`net=<name>` (compared without case); without it they act on the net of
the selection or of what is under the cursor.

A point operand (`at`, `start`, `end`) also takes a pin reference
`<designator>.<pin>` (`R1.2`) or `<sheet name>.<entry name>` (`Charger.SDA`):
the pin's tip, or the sheet entry's point, on the sheet the edit targets.
`place_wire`, `draw_wire`, `place_bus` and `draw_bus` check a run before it
lands and answer with a report (optional operands `allow_merge`,
`allow_open`, `dry_run`); rules in [CLI.md](CLI.md), "Wire intent".

`place_component library=.. part=.. at=x,y` places a row of an open library
and answers with its designator, unit and pin tips; `place_sheet_symbol`,
`place_sheet_entry` and `sync_sheet_pins` edit the hierarchy without writing
a file. Without operands, in a window, they open the picker, arm the tool or
act on the selection as before; rules in [CLI.md](CLI.md), "Placing parts
and hierarchy".

Ids and where each file kind stores one: [DOCUMENT_IDS.md](DOCUMENT_IDS.md).

## General

| command id | label | category | Signex Standard | Classic |
| --- | --- | --- | --- | --- |
| `apply_board_schematic_differences` | Apply board and schematic differences by id | pcb_sync | — | — |
| `apply_edits` | Apply edits (agent) | edit | — | — |
| `apply_layout_board` | Workspace layout: Layout | window | — | — |
| `apply_layout_notebook` | Workspace layout: Notebook | window | — | — |
| `apply_layout_review` | Workspace layout: Review | window | — | — |
| `apply_layout_schematic` | Workspace layout: Schematic | window | — | — |
| `attach_notebook_file` | Copy a file into the notebook as an attachment | design | — | — |
| `autoplace_fields` | Autoplace fields | edit | — | `O` |
| `board_status` | Board status counters | validation | — | — |
| `bom_add_alternate` | Add an alternate part to a BOM line | design | — | — |
| `bom_remove_alternate` | Remove an alternate part from a BOM line | design | — | — |
| `bom_set_alternate_order` | Move an alternate part in a BOM line's order | design | — | — |
| `break_wire` | Break wire | modify | `E W` | — |
| `bring_drawings_forward` | Bring drawings forward | edit | — | — |
| `bring_drawings_to_front` | Bring drawings to front | edit | — | — |
| `cancel_current_tool` | Cancel current stage / exit placement mode | interactive | `Escape` | `Escape` |
| `center_on_cursor` | Center on cursor | view | — | `F4` |
| `center_view_at_cursor` | Center/redraw view around cursor | view | `Home` | — |
| `check_board_against_schematic` | Check the board against the schematic | pcb_sync | — | — |
| `check_drc` | Check design rules | validation | — | — |
| `check_erc` | Check electrical rules | validation | — | — |
| `check_requirements` | Check project requirements | validation | — | — |
| `close_active_document` | Close active document | file | `Ctrl+F4`, `Mod+W` (macOS) | — |
| `close_all_documents` | Close all documents | window | — | — |
| `copy` | Copy | edit | `Mod+C` | `Mod+C` |
| `copy_attributes_or_add_vertex` | Copy attributes from object under cursor or add vertex while drawing | interactive | `Insert`, `Alt+I` (macOS) | — |
| `cross_probe` | Cross probe the selection to the board or schematic | select | `T C` | `Shift+C` |
| `cut` | Cut | edit | `Mod+X` | `Mod+X` |
| `cycle_fast_grid` | Cycle fast grid | view | — | `Alt+4` |
| `cycle_selection_mode` | Cycle selection mode | select | — | — |
| `cycle_snap_grid_backward` | Cycle backward through snap grids | view | `Shift+G` | — |
| `cycle_snap_grid_forward` | Open the snap grid picker (footprint editor) | view | `G` | — |
| `cycle_unit` | Cycle display unit | view | `Q`, `Ctrl+Q` | `Q`, `Ctrl+Q` |
| `cycle_wire_bus_graphic_mode` | Cycle the wire/bus corner mode: 90 degrees, 45 degrees start, 45 degrees end, any angle | interactive | — | `Shift+Space` |
| `cycle_wiring_mode` | Change wiring mode while dragging connected electrical objects | interactive | `Ctrl+Space`, `Shift+Tab` (macOS) | — |
| `delete_selection` | Delete selection | edit | `Delete` | `Delete` |
| `document_info` | Document info | file | — | — |
| `drag_keep_connections` | Drag while keeping connections | modify | — | `G` |
| `duplicate` | Duplicate | edit | `Mod+D` | `Mod+D` |
| `edit_footprint_field` | Edit footprint field | edit | — | `F` |
| `edit_library_symbol` | Edit library symbol | library | — | `Mod+Shift+E` |
| `edit_object_properties` | Edit properties of object under cursor | edit | `DoubleClick` | — |
| `edit_reference_designator` | Edit reference designator | edit | — | `U` |
| `edit_selected_object_properties` | Properties / edit selected object | edit | — | `E` |
| `edit_selected_symbol_in_symbol_editor` | Edit selected symbol in Symbol Editor | library | — | `Mod+E` |
| `edit_text_in_place` | In-place edit selected text | edit | `F2` | — |
| `edit_value` | Edit value | edit | — | `V` |
| `export_notebook_pdf` | Export the Design Notebook as PDF | file | — | — |
| `export_notebook_timeline` | Export a measurement timeline as a CSV file | reports | — | — |
| `fast_grid_1` | Fast grid 1 | view | — | `Alt+1` |
| `fast_grid_2` | Fast grid 2 | view | — | `Alt+2` |
| `find` | Find | search | `Mod+F` | `Mod+F` |
| `find_and_replace` | Find and replace | search | `Ctrl+H`, `Mod+Alt+F` (macOS) | — |
| `find_items` | Find documents or items | search | — | — |
| `find_next` | Find next | search | `F3` | `F3` |
| `find_previous` | Find previous | search | `Shift+F3` | `Shift+F3` |
| `find_similar_objects` | Find Similar Objects | select | `Shift+F` | `Shift+F` |
| `find_text` | Find text | search | — | — |
| `generate_design_notebook` | Generate a design notebook from the project | design | — | — |
| `home_view` | Reset the view rotation and Flip, then fit the board or sheet | view | — | — |
| `import_graphics` | Import graphics | place | — | `Mod+Shift+F` |
| `keymap_assign` | Give a command a key in a custom keyboard profile | preferences | — | — |
| `keymap_clear` | Take a command's keys away in a custom keyboard profile | preferences | — | — |
| `list_annotations` | List the annotation blocks of the project's Design Notebook | design | — | — |
| `list_documents` | List documents | file | — | — |
| `manage_workspace_layouts` | Manage workspace layouts | window | — | — |
| `mark_notebook_attachment_valid` | Mark a stale attachment or reading as still valid | design | — | — |
| `measure_distance` | Measure distance | view | `Ctrl+M` | `Ctrl+M` |
| `mirror_x` | Mirror along X-axis | modify | `X` | `X` |
| `mirror_y` | Mirror along Y-axis | modify | `Y` | `Y` |
| `move_object` | Move object | modify | `M M` | — |
| `move_selection` | Move selection | modify | — | `M` |
| `new_document` | New | file | `Mod+N` | `Mod+N` |
| `new_project_with_profile` | New project from a profile | file | — | — |
| `next_document_tab` | Next open document tab | window | `Ctrl+Tab` | — |
| `next_grid` | Next grid | view | — | `N` |
| `notebook_attachments` | List the notebook's attachments and their stale marks | design | — | — |
| `notebook_badges` | List the Notebook badges and their blocks | design | — | — |
| `notebook_compile` | Compile the project's Design Notebook and report its pages and problems | reports | — | — |
| `notebook_fix_reference` | Fix a stale notebook reference | design | — | — |
| `notebook_insert` | Insert a reference or a block into the open Design Notebook | edit | — | — |
| `notebook_measurements` | List the notebook's measurements with their results | design | — | — |
| `notebook_reference_card` | Read the hover card of a notebook reference | design | — | — |
| `notebook_reference_picker` | List what the @ reference picker offers | design | — | — |
| `notebook_references` | List the notebook's references and the stale ones | design | — | — |
| `notebook_timeline` | Show the measurement timeline of a net, part or pin | design | — | — |
| `notebook_view` | Set the view, the side panel or the next problem of the Design Notebook tab | view | — | — |
| `open_command_palette` | Open command palette | commands | `Mod+Shift+P` | `Ctrl+K`, `Mod+Shift+P` |
| `open_compiled_design` | Open Compiled Design panel | panels | — | — |
| `open_datasheet` | Open datasheet | edit | — | `D` |
| `open_design_notebook` | Open the Design Notebook of the project (Wiki) | design | — | — |
| `open_diff_pair_classes` | Open the project's differential pair classes | design | — | — |
| `open_document` | Open document | file | `Mod+O` | `Mod+O` |
| `open_grid_picker` | Open grid picker | view | — | — |
| `open_grid_properties` | Open grid properties | view | — | — |
| `open_library_browser` | Open library browser | library | — | — |
| `open_net_classes` | Open the project's Net Classes panel | design | — | — |
| `open_net_classes_panel` | Open the Net Classes panel of the project | design | — | — |
| `open_net_inspector` | Open Net Inspector panel | panels | `V N` | `Shift+I` |
| `open_preferences` | Open preferences | preferences | `Mod+,` | `Mod+,` |
| `open_schematic_preferences` | Open schematic preferences | preferences | `T P` | — |
| `paste` | Paste | edit | `Mod+V` | `Mod+V` |
| `paste_array` | Paste array | edit | — | — |
| `paste_special` | Paste special | edit | — | `Mod+Shift+V` |
| `placement_accept` | Accept current placement or move stage | interactive | `Enter` | — |
| `placement_properties` | Edit properties of the object being placed or moved | interactive | `Tab` | — |
| `previous_document_tab` | Previous open document tab | window | `Ctrl+Shift+Tab` | — |
| `previous_grid` | Previous grid | view | — | `Shift+N` |
| `print` | Print | file | `Mod+P` | `Mod+P` |
| `project_add_requirement` | Add a requirement to the project | design | — | — |
| `project_add_variant` | Add a variant to the project | design | — | — |
| `project_enable_requirement` | Turn a project requirement on or off | design | — | — |
| `project_move_library` | Move a library in the library order of the project | design | — | — |
| `project_move_sheet` | Move a sheet in the page order of the project | design | — | — |
| `project_remove_requirement` | Remove a requirement from the project | design | — | — |
| `project_remove_variant` | Remove a variant of the project | design | — | — |
| `project_rename_variant` | Rename a variant of the project | design | — | — |
| `project_run_requirements` | Run the project's requirement checks in the Project tab | validation | — | — |
| `project_set_active_variant` | Choose the active variant of the project | design | — | — |
| `project_set_annotate_undo` | Set how many undo steps an annotate run of the project is | design | — | — |
| `project_set_rail` | Set a power rail's voltage and tolerance | design | — | — |
| `project_set_requirement` | Set a field of a project requirement | design | — | — |
| `project_set_version_control` | Turn commit on save on or off for the project | design | — | — |
| `project_settings` | Read the settings of the project | design | — | — |
| `query_bom` | Query the priced BOM | design | — | — |
| `query_library` | Query a library | library | — | — |
| `query_pins` | Query pins | file | — | — |
| `query_rail_budget` | Query the project's rail budget | validation | — | — |
| `read_document` | Read document | file | — | — |
| `redo` | Redo | edit | `Mod+Shift+Z` (macOS), `Ctrl+Shift+Z`, `Ctrl+Y` | `Ctrl+Shift+Z`, `Mod+Shift+Z` (macOS), `Ctrl+Y` |
| `refresh_view` | Refresh/redraw | view | `End` | `F5` |
| `remove_last_vertex` | Remove last placed vertex while drawing | interactive | `Backspace` | — |
| `repeat_last_item` | Repeat last item | edit | — | `Insert` |
| `report_manager_bom` | Report Manager / BOM | reports | `R I` | — |
| `reset_local_coordinates` | Reset local coordinates | view | — | `Space` |
| `reset_workspace_layout` | Reset the edited built-in workspace layout | window | — | — |
| `resolve` | Resolve id | file | — | — |
| `revert` | Revert an agent's edit step | edit | — | — |
| `rotate_clockwise` | Rotate clockwise by 90 degrees | modify | `Shift+Space` | `Shift+R` |
| `rotate_counterclockwise` | Rotate counterclockwise by 90 degrees | modify | `Space` | `R` |
| `rubber_stamp_copy` | Rubber-stamp copy / repeated paste | edit | `Mod+R` | — |
| `save_all` | Save all unsaved documents | file | `Mod+Alt+S` | `Mod+Alt+S` |
| `save_document` | Save document | file | `Mod+S` | `Mod+S` |
| `save_document_as` | Save as | file | `Mod+Shift+S` | `Mod+Shift+S` |
| `save_project_as` | Save the project to a new folder | file | — | — |
| `save_workspace_layout` | Save the panel arrangement as a workspace layout | window | — | — |
| `search_parts` | Search parts by parameters | library | — | — |
| `select_all` | Select all | select | `Mod+A` | `Mod+A` |
| `send_drawings_backward` | Send drawings backward | edit | — | — |
| `send_drawings_to_back` | Send drawings to back | edit | — | — |
| `show_all_design_objects` | Show all design objects | view | — | — |
| `show_current_command_hotkeys` | Show graphical editing hotkey list for current command | help | `F1` | — |
| `show_current_command_shortcuts` | Show valid shortcuts for the current interactive command | help | `Shift+F1` | — |
| `smart_paste` | Smart Paste | edit | `Mod+Shift+V` | — |
| `switch_segment_posture` | Flip the wire/bus corner: horizontal or vertical first | interactive | — | `/` |
| `tile_windows_horizontally` | Tile windows one above the other | window | `W H` | `Shift+Alt+H` |
| `tile_windows_vertically` | Tile windows side by side | window | `W V` | `Shift+Alt+V` |
| `toggle_auto_focus` | Toggle AutoFocus | view | `F9` | `F9` |
| `toggle_cross_select_mode` | Toggle Cross Select Mode | select | `Mod+Shift+X` | — |
| `toggle_electrical_grid` | Toggle electrical grid | view | `Shift+E` | — |
| `toggle_floating_panels` | Toggle floating panels | window | `F4` | — |
| `toggle_properties_panel` | Toggle Properties panel | panels | `F11` | — |
| `toggle_schematic_filter_panel` | Toggle schematic filter panel | panels | `F12` | — |
| `toggle_schematic_list_panel` | Toggle Object List panel | panels | `Shift+F12` | — |
| `toggle_search_panel` | Show/hide search panel | search | — | `Ctrl+G` |
| `toggle_selection` | Add/remove object from selection | select | `Shift+Click` | — |
| `toggle_visible_grid` | Toggle visible grid | view | `Mod+Shift+G` | `Mod+Shift+G` |
| `undo` | Undo | edit | `Mod+Z` | `Mod+Z` |
| `undo_last_segment` | Undo last segment while drawing | interactive | — | `Backspace` |
| `unselect_all` | Unselect all | select | — | `Mod+Shift+A` |
| `untile_documents` | Untile documents | window | `W U` | `Shift+Alt+U` |
| `zoom_in_at_cursor` | Zoom in at cursor | view | `PageUp` | `F1` |
| `zoom_out_at_cursor` | Zoom out at cursor | view | `PageDown` | `F2` |
| `zoom_to_all_objects` | Zoom to all objects | view | — | `Ctrl+Home` |
| `zoom_to_fit` | Zoom to fit | view | `Ctrl+PageDown` | `Home` |
| `zoom_to_selection_area` | Zoom to selection area | view | — | `Ctrl+F5` |

## Schematic

| command id | label | category | Signex Standard | Classic |
| --- | --- | --- | --- | --- |
| `add_table_of_contents` | Add table of contents sheet | design | — | — |
| `add_to_union` | Add the selection to the selected union | edit | — | — |
| `align_bottom` | Align bottom | modify | — | — |
| `align_horizontal_centers` | Align horizontal centers | modify | — | — |
| `align_left` | Align left | modify | — | — |
| `align_right` | Align right | modify | — | — |
| `align_to_grid` | Align to grid | modify | — | — |
| `align_top` | Align top | modify | — | — |
| `align_vertical_centers` | Align vertical centers | modify | — | — |
| `annotate_schematic` | Annotate schematic | design | `T A` | — |
| `annotate_schematic_quietly` | Annotate schematic quietly | design | `Alt+A` | `Alt+A` |
| `apply_selection_filter_preset` | Apply a selection filter preset | select | — | — |
| `assign_footprint` | Set the footprint of a placed part | design | — | — |
| `assign_footprints` | Assign footprints | design | — | — |
| `assign_net_class` | Assign net to class | design | — | — |
| `back_annotate_schematics` | Back-annotate the board's designators and values to the schematic | pcb_sync | — | — |
| `break_union` | Break the selected union | edit | — | — |
| `bring_to_front` | Bring to front | modify | — | — |
| `bring_to_front_of` | Bring to front of | modify | — | — |
| `change_component` | Change the selected part's component | edit | — | — |
| `clear_all_net_colors` | Clear all net colors | view | — | — |
| `clear_net_color` | Clear net color | view | — | — |
| `clear_net_highlighting` | Clear net highlighting | select | — | `~` |
| `color_net` | Color net | view | — | — |
| `compare_board_with_schematic` | Compare board with schematic | pcb_sync | — | — |
| `compile_project` | Compile the project | design | — | — |
| `create_sheet_from_sheet_symbol` | Create a child sheet from the selected sheet symbol | design | — | — |
| `create_sheet_symbol_from_sheet` | Create a sheet symbol from a sheet file, with an entry for every port | design | — | — |
| `create_union` | Create a union from the selection | edit | `Mod+G` | — |
| `distribute_horizontally` | Distribute horizontally | modify | — | — |
| `distribute_vertically` | Distribute vertically | modify | — | — |
| `drag` | Drag | modify | — | — |
| `drag_selection` | Drag selection | modify | — | — |
| `draw_arc` | Draw arc | place | — | — |
| `draw_bezier` | Draw bezier | place | — | — |
| `draw_bus` | Draw bus | place | — | — |
| `draw_ellipse` | Draw ellipse | place | — | — |
| `draw_elliptical_arc` | Draw elliptical arc | place | — | — |
| `draw_full_circle` | Draw full circle | place | — | — |
| `draw_graphic_line` | Draw graphic line | place | — | `I` |
| `draw_hierarchical_sheet` | Draw hierarchical sheet | place | — | — |
| `draw_line` | Draw line | place | — | — |
| `draw_polygon` | Draw polygon | place | — | — |
| `draw_rectangle` | Draw rectangle | place | — | — |
| `draw_round_rectangle` | Draw round rectangle | place | — | — |
| `draw_wire` | Draw wire | place | — | — |
| `edit_blanket` | Edit net blanket parameters | place | — | — |
| `edit_no_erc_rules` | Edit No ERC rules | place | — | — |
| `export_parameters` | Export the active sheet's parameters to a CSV file | design | — | — |
| `flip_selected_x` | Flip selected x | modify | — | — |
| `flip_selected_y` | Flip selected y | modify | — | — |
| `force_annotate_all_schematics` | Force annotate all schematics | design | `Shift+Alt+A` | `Shift+Alt+A` |
| `go_to_net_partner` | Go to net partner on another sheet | navigation | — | — |
| `highlight_net_under_cursor` | Highlight net under cursor | select | — | ``` |
| `highlight_related_net_objects` | Highlight/select related net objects across sheets | select | `Alt+Click` | — |
| `import_parameters` | Import parameters from a CSV file, with a preview to apply | design | — | — |
| `lasso_select` | Lasso select | select | — | — |
| `leave_sheet` | Leave sheet / go to parent sheet | navigation | — | `Alt+Backspace` |
| `move_selection_xy` | Move selection x y | modify | — | — |
| `move_to_front` | Move to front | modify | — | — |
| `navigate_up_hierarchy` | Navigate up hierarchy | navigation | — | `Alt+Up` |
| `new_schematic_sheet` | New schematic sheet in the project | file | — | — |
| `next_highlighted_net_item` | Next item on highlighted net | select | — | `Tab` |
| `next_sheet` | Next sheet | navigation | — | `PageDown` |
| `number_schematic_sheets` | Number schematic sheets | design | — | — |
| `open_child_sheet` | Open the selected sheet symbol's child sheet | navigation | — | — |
| `open_components_panel` | Open Components panel / place components | panels | `P P` | — |
| `open_erc_dialog` | Open electrical rules check dialog | validation | — | — |
| `open_harness_definitions` | Open the harness definitions | design | — | — |
| `open_net_color_palette` | Open net color palette | view | `F5` | — |
| `open_parameter_manager` | Open Parameter Manager | design | — | — |
| `open_project_options` | Open Project Options for the active project | file | — | — |
| `open_selection_filter` | Open the selection filter of the sheet | select | — | — |
| `place_aligned_dimension` | Place aligned dimension | place | — | — |
| `place_blanket` | Place net blanket | place | — | — |
| `place_bus` | Draw bus | place | `P B` | `B` |
| `place_bus_entry` | Place bus entry | place | `P U` | `Z` |
| `place_compile_mask` | Place Compile Mask directive | place | `P V K` | — |
| `place_component` | Place component | place | — | — |
| `place_design_block` | Place design block | place | — | `Shift+B` |
| `place_device_sheet_symbol` | Place device sheet symbol | place | — | — |
| `place_diff_pair` | Place diff pair | place | — | — |
| `place_global_label` | Place global label | place | — | — |
| `place_graphic` | Place graphic | place | — | — |
| `place_harness_connector` | Place harness connector | place | — | — |
| `place_harness_entry` | Place harness entry | place | — | — |
| `place_hierarchical_label` | Place hierarchical label | place | — | — |
| `place_junction` | Place junction | place | — | `J` |
| `place_linear_dimension` | Place linear dimension | place | — | — |
| `place_local_net_label` | Place local net label | place | — | `L` |
| `place_net_label` | Place net label | place | `P N` | — |
| `place_no_connect` | Place no-connect flag | place | `P X` | `Q` |
| `place_no_erc` | Place Generic No ERC directive | place | `P V N` | — |
| `place_note` | Place note | place | — | — |
| `place_off_sheet_connector` | Place off sheet connector | place | `P C` | `H` |
| `place_parameter_set` | Place net parameters | place | — | — |
| `place_port` | Place port | place | `P R` | `Mod+L` |
| `place_power_symbol` | Place power symbol | place | — | `P` |
| `place_reuse_block` | Place reuse block | place | — | — |
| `place_sheet_entry` | Place sheet entry | place | — | — |
| `place_sheet_symbol` | Place sheet symbol | place | `P S` | `S` |
| `place_signal_harness` | Place signal harness | place | — | — |
| `place_symbol` | Place symbol | place | — | `A` |
| `place_table` | Place table | place | — | — |
| `place_text` | Place text | place | — | `T` |
| `place_text_frame` | Place text frame | place | — | — |
| `place_text_string` | Place text string | place | — | — |
| `place_wire` | Place wire | place | `Ctrl+W`, `P W` | `W` |
| `place_wire_to_bus_entry` | Place wire-to-bus entry | place | — | — |
| `previous_highlighted_net_item` | Previous item on highlighted net | select | — | `Shift+Tab` |
| `previous_sheet` | Previous sheet | navigation | — | `PageUp` |
| `query_footprint_assignments` | List the footprint of every part | design | — | — |
| `remove_from_union` | Remove the selection from its union | edit | — | — |
| `repair_pin_junctions` | Add a junction where a pin end touches a wire | validation | — | — |
| `reset_schematic_designators` | Reset schematic designators | design | `T A E` | — |
| `rotate_selection` | Rotate selection counter-clockwise | modify | — | — |
| `rotate_selection_cw` | Rotate selection clockwise | modify | — | — |
| `run_erc` | Run electrical rules check | validation | `F8` | `F8` |
| `select_connection` | Select connection | select | — | — |
| `select_expand_connection` | Select / expand connection | select | — | `Mod+4` |
| `select_node_or_connection_item` | Select node / connection item under cursor | select | — | `Alt+3` |
| `send_to_back` | Send to back | modify | — | — |
| `send_to_back_of` | Send to back of | modify | — | — |
| `set_object_position` | Set the position of a placed object | edit | — | — |
| `set_object_rotation` | Set the rotation of a placed part or label | edit | — | — |
| `set_selection_filters` | Set the schematic selection filter | select | — | — |
| `set_sheet_symbol_size` | Set the size of a sheet symbol | edit | — | — |
| `set_sheet_variable` | Set a sheet variable | edit | — | — |
| `set_symbol_parameter` | Set a parameter of a placed part | edit | — | — |
| `sheet_navigation_back` | Sheet navigation back | navigation | — | `Alt+Left` |
| `sheet_navigation_forward` | Sheet navigation forward | navigation | — | `Alt+Right` |
| `sync_sheet_entries` | Synchronize the selected sheet symbol's entries and its sheet's ports, with a preview | design | — | — |
| `sync_sheet_pins` | Sync the selected sheet symbol's pins with its child sheet | design | — | — |
| `toggle_notebook_badges` | Show or hide the Notebook badges on the schematic | view | `B` | — |
| `tool_select` | Tool select | select | — | — |
| `update_pcb_from_schematic` | Update PCB from schematic | pcb_sync | — | — |

## PCB

| command id | label | category | Signex Standard | Classic |
| --- | --- | --- | --- | --- |
| `add_board_graphic` | Add a line, arc, circle, rectangle or text frame to a board | place | — | — |
| `add_net_class` | Add a net class | design | — | — |
| `add_teardrops` | Add teardrops where tracks of the selected nets, or of every net, enter a pad or via | pcb | — | — |
| `arrange_in_box` | Arrange the selected components inside a box drawn on the board | modify | — | — |
| `arrange_in_room` | Pack the parts of the selected room inside it again | design | — | — |
| `assign_board_net_class` | Put a board net in a net class | design | — | — |
| `auto_place` | Place the parts of each room inside it by connection length, heat, noise and connector edges | design | — | — |
| `board_assembly_drawing` | Open Board Print / PDF for an assembly drawing | pcb | — | — |
| `board_layer_stack` | Open the layer stack manager of the board | design | — | — |
| `board_panel` | Describe the board's panel | pcb | — | — |
| `board_report_bom` | Show the board's bill of materials: its components grouped by value and footprint | pcb | — | — |
| `board_report_components` | Show each component's designator, footprint, position, rotation and side | pcb | — | — |
| `board_report_net_length` | Show each net's routed length, tracks and vias | pcb | — | — |
| `board_report_netlist` | Show the board's netlist: each net and the pads on it | pcb | — | — |
| `board_shape_from_selection` | Make the selected closed lines the board outline | design | — | — |
| `board_sketch` | Enter or leave the board sketch | design | `1` | `1` |
| `change_footprint` | Change the footprint of the selected components | modify | `F` | `F` |
| `copy_board_3d_screenshot` | Copy a screenshot of the 3D view | view | `Mod+Shift+C` | `Mod+Shift+C` |
| `create_rooms_from_sheets` | Make a room for each sheet of the project and move its parts into it | design | — | — |
| `cycle_arc_mode` | Switch the Arc tool between by centre, by edge and any angle | pcb | — | — |
| `cycle_board_3d_lighting` | Next lighting preset in the 3D view | view | `H` | `H` |
| `cycle_board_snapping` | Cycle board snapping | view | `Shift+E` | `Shift+E` |
| `cycle_route_mode` | Switch Interactive Routing between walking around and pushing other nets' tracks | pcb | — | — |
| `delete_layer_set` | Delete a saved layer set | view | — | — |
| `delete_net_class` | Delete a net class | design | — | — |
| `edit_layer_color` | Open the color picker of one layer in the Layers panel | view | — | — |
| `export_assembly_svg` | Write the board's top and bottom assembly drawings as SVG files | reports | — | — |
| `export_board_pdf` | Print the board's layers to a PDF file | reports | — | — |
| `export_board_report` | Export a board report as a CSV file | reports | — | — |
| `export_board_step` | Export the board and its parts as a STEP 3D model | reports | — | — |
| `fabrication_outputs` | Generate the board's fabrication outputs | file | — | — |
| `filter_drc_results` | Filter the DRC results by rule, layer and severity | pcb | — | — |
| `find_similar_items` | Find similar items on a board | search | — | — |
| `fix_drc` | Fix the DRC violations of one rule, or of every rule with a fix | pcb | — | — |
| `fix_drc_violation` | Fix one DRC violation without waiving it | pcb | — | — |
| `flip_board_3d` | Turn the board over in the 3D view | view | `F` | `F` |
| `flip_board_view` | View the board from below, or from above again | view | — | — |
| `flip_selection` | Flip selection to the other board side | modify | `L` | `L` |
| `gloss_selected_routes` | Shorten the selected routes by cutting their corners where the clearance allows | pcb | — | — |
| `hide_layer` | Hide one layer of the board view | view | — | — |
| `hide_layer_group` | Hide every layer of one kind | view | — | — |
| `highlight_board_net` | Highlight a board net | select | — | — |
| `highlight_layer` | Highlight one layer and dim the others | view | — | — |
| `import_dxf` | Import a DXF drawing onto the active layer of the board | file | — | — |
| `import_dxf_outline` | Import the largest closed shape of a DXF drawing as the board outline | file | — | — |
| `keepout_from_selection` | Turn the selected area or closed lines into a keepout on its layer | design | — | — |
| `layout_plan_get` | Read the board's layout plan | pcb | — | — |
| `layout_plan_set` | Change a stage of the board's layout plan | pcb | — | — |
| `layout_plan_status` | Say how far each stage of the board's layout plan is | pcb | — | — |
| `lock_board_items` | Lock the selected tracks and vias | modify | — | — |
| `measure_board` | Measure on a board | search | — | — |
| `new_pcb_document` | New PCB in the project | file | — | — |
| `next_layer` | Make the next layer of the board active | view | `+`, `Shift++` | `+`, `Shift++` |
| `next_signal_layer` | Make the next signal layer active | view | `*`, `Shift+*` | `*`, `Shift+*` |
| `open_drc_panel` | Open the DRC results panel | pcb | — | — |
| `open_rules` | Open the design rules of the project | design | — | — |
| `panelize_board` | Array the board into a manufacturing panel | pcb | — | — |
| `place_board_arc` | Draw an arc on the active layer | place | — | — |
| `place_board_circle` | Draw a full circle by its centre and a point on it on the active layer | place | — | — |
| `place_board_component` | Pick a library part and place its footprint on the board | place | — | — |
| `place_board_diff_pair` | Route a differential pair side by side at its coupled width and gap, around other nets | place | — | — |
| `place_board_dimension` | Place an aligned dimension on the board, measured along its two points | place | — | — |
| `place_board_dimension_horizontal` | Place a linear dimension on the board, measured across | place | — | — |
| `place_board_dimension_radial` | Place a radial dimension on an arc or circle of the board | place | — | — |
| `place_board_dimension_vertical` | Place a linear dimension on the board, measured up or down | place | — | — |
| `place_board_fill` | Place a rectangle of solid copper on the active copper layer | place | `P F` | `Shift+Alt+F` |
| `place_board_graphic_arc` | Draw an arc by its centre, start and end on the active layer, as a track arc on copper | place | — | — |
| `place_board_keepout` | Draw a keepout that keeps copper off a layer | place | `P K` | `K` |
| `place_board_line` | Draw lines on the active layer, as tracks on copper | place | — | — |
| `place_board_multi_route` | Route the selected nets side by side as a bus, around other nets | place | — | — |
| `place_board_pad` | Place a free pad on the board | place | — | — |
| `place_board_polygon_pour` | Place a polygon pour on the active copper layer, filled clear of other nets | place | `P G` | `Shift+G` |
| `place_board_pour_cutout` | Place a cutout region the polygon pours on the active copper layer leave empty | place | — | — |
| `place_board_quick_route` | Route straight to the clicked point at the rule width, without avoiding other nets | place | — | — |
| `place_board_rectangle` | Draw a rectangle by two corners on the active layer | place | — | — |
| `place_board_region` | Place a polygon of solid copper on the active copper layer | place | `P R` | `Shift+Alt+R` |
| `place_board_reuse_block` | Place a reuse block on the channel of the selected component | design | — | — |
| `place_board_room` | Draw a room by two corners: an area for the parts of one sheet | place | — | — |
| `place_board_route` | Route on the active copper layer around other nets, keeping the rule clearance | place | — | — |
| `place_board_slice_pour` | Slice the polygon pours on the active copper layer along a line | place | — | — |
| `place_board_string` | Place a string on the active copper, silkscreen, mechanical or documentation layer | place | — | — |
| `place_board_text_frame` | Place text wrapped in a box on the active layer | place | — | — |
| `place_board_track` | Route a track on the active copper layer | place | `P T` | `Shift+T` |
| `place_board_via` | Place a via on the board | place | `P V` | `Shift+V` |
| `polygon_manager` | Open the Polygon Manager of the board | pcb | — | — |
| `prev_layer` | Make the previous layer of the board active | view | `-` | `-` |
| `query_board_nets` | Query board nets | search | — | — |
| `query_part_heights` | Query part heights | validation | — | — |
| `reannotate_board` | Re-annotate the board's designators by position | design | — | — |
| `remove_board_panel` | Remove the board's panel | pcb | — | — |
| `remove_teardrops` | Remove the teardrops of the selected nets, or of every net | pcb | — | — |
| `rename_net_class` | Rename a net class | design | — | — |
| `repour_all` | Fill every polygon pour of the board again and report their copper | pcb | — | — |
| `repour_selected` | Fill the selected polygon pours again and report their copper | pcb | — | — |
| `reset_board_origin` | Move the board origin to the bottom-left of the board outline | edit | — | — |
| `reset_layer_colors` | Give every layer the board palette color again | view | — | — |
| `rules_add` | Add a design rule | design | — | — |
| `rules_conflicts` | List shadowed and conflicting design rules | validation | — | — |
| `rules_effective` | Show the rule value that governs a net | validation | — | — |
| `rules_enable` | Turn a design rule on or off | design | — | — |
| `rules_explain` | Explain the value a DRC finding asks for | validation | — | — |
| `rules_get` | Read one design rule | validation | — | — |
| `rules_list` | List the design rules | validation | — | — |
| `rules_matrix_get` | Read the class-to-class clearance matrix | validation | — | — |
| `rules_matrix_set` | Set a cell of the class-to-class clearance matrix | design | — | — |
| `rules_preset_apply` | Apply a rules preset | design | — | — |
| `rules_preset_diff` | Show what a rules preset would change | validation | — | — |
| `rules_remove` | Remove a design rule | design | — | — |
| `rules_set` | Change a field of a design rule | design | — | — |
| `rules_set_apply` | Apply a constraint set to a class or net | design | — | — |
| `rules_set_create` | Create a constraint set | design | — | — |
| `rules_set_edit` | Change the limits of a constraint set | design | — | — |
| `run_drc` | Run the design rule check on the board | pcb | `T D` | `F7` |
| `save_board_reuse_block` | Save the selected placed and routed parts as a reuse block of the project | design | — | — |
| `save_layer_set` | Save the shown layers as a layer set | view | — | — |
| `select_board_net` | Select every track, via and pad of a board net | select | — | — |
| `set_active_layer` | Make one layer the active layer | view | — | — |
| `set_board_graphic` | Set one field of a board drawing | edit | — | — |
| `set_board_origin` | Set the board origin at a clicked point | edit | `E O S` | `Shift+O` |
| `set_board_panel` | Change a setting of the board's panel | pcb | — | — |
| `set_footprint_position` | Move a component on the board to a point | edit | — | — |
| `set_footprint_rotation` | Set the rotation of a component on the board | edit | — | — |
| `set_footprint_side` | Put a component on the top or bottom side of the board | edit | — | — |
| `set_footprint_text` | Set the place, angle, height or justification of a component's designator or comment | edit | — | — |
| `set_layer_color` | Give one layer its own color, saved with the board, or the palette color again | view | — | — |
| `set_layer_sizes` | Set the per-layer copper sizes of vias or pads | modify | — | — |
| `set_layer_stack` | Replace the layer stack of a board | design | — | — |
| `set_layer_tab_names` | Name the layer tabs short, medium or long | view | — | — |
| `set_layer_visible` | Show or hide one layer of the board view | view | — | — |
| `set_net_class_rules` | Set the color and board rules of a net class | design | — | — |
| `set_object_transparency` | Set how transparent one kind of object is drawn | view | — | — |
| `set_object_visible` | Show or hide one kind of object in the board view | view | — | — |
| `set_pad_shape` | Set the copper shape of pads, with a trapezoid's delta or a custom outline | modify | — | — |
| `set_via_span` | Set the copper layers vias span | modify | — | — |
| `show_all_layers` | Show every layer | view | — | — |
| `show_drc_violation` | Select a DRC violation and centre the view on it | pcb | — | — |
| `show_layer_group` | Show every layer of one kind | view | — | — |
| `show_layer_set` | Show a layer set on the board view | view | — | — |
| `show_used_layers_only` | List only the layers the board uses in the Layers panel | view | — | — |
| `snap_board_origin_to_corner` | Move the board origin to the outline corner nearest the pointer | edit | — | — |
| `stitch_vias` | Fill the selected areas, or every pour, with vias of their net on a grid, clear of other nets | pcb | — | — |
| `swap_gates` | Swap two equivalent gates of a component, on the board and on its schematic | modify | — | — |
| `swap_pins` | Swap the nets of two equivalent pins of a component, on the board and on its schematic | modify | — | — |
| `toggle_layer_numbers` | Show or hide the numbers on the layer tabs | view | — | — |
| `toggle_layer_visibility` | Show or hide the active layer | view | — | — |
| `toggle_layers_panel` | Show or hide the Layers panel | view | — | — |
| `toggle_origin_marker` | Show or hide the board origin marker | view | — | — |
| `toggle_ratsnest` | Show or hide the ratsnest | view | `N` | `C` |
| `toggle_single_layer_mode` | Draw only the active layer, or every layer again | view | `Shift+S` | `Shift+S` |
| `tune_diff_pair` | Lengthen the shorter net of a differential pair with meanders to match the longer one | pcb | — | — |
| `tune_net_length` | Lengthen the selected tracks' nets with meanders to a target length, or to the longest of them | pcb | — | — |
| `unfill_selected` | Clear the copper of the selected polygon pours until the next repour | pcb | — | — |
| `unlock_board_items` | Unlock the selected tracks and vias | modify | — | — |
| `unwaive_drc_violation` | Take back the waiver of a DRC violation | pcb | — | — |
| `view_board_3d` | Show the board in 3D, or flat again | view | `3` | `3` |
| `view_rotate` | Rotate the board view clockwise | view | — | — |
| `view_rotate_180` | Rotate the board view 180° | view | — | — |
| `view_rotate_ccw` | Rotate the board view 90° counter-clockwise | view | — | — |
| `view_rotate_reset` | Reset the board view rotation to 0° | view | — | — |
| `waive_drc_violation` | Waive a DRC violation with a note | pcb | — | — |

## 3D

| command id | label | category | Signex Standard | Classic |
| --- | --- | --- | --- | --- |
| `footprint_mode_pads` | Switch footprint editor to Pads mode | library | `2` | `2` |
| `footprint_mode_sketch` | Switch footprint editor to Sketch mode | library | `1` | `1` |
| `footprint_mode_view_3d` | Switch footprint editor to 3D View mode | library | `3` | `3` |
| `footprint_recompute_courtyard_outline` | Recompute footprint courtyard outline from pads | library | — | — |
