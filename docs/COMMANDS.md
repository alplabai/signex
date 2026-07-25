# Signex command reference

Every command the application exposes by a stable id, with the default
binding each shipped keyboard profile gives it. A keymap profile binds
keys to these ids — see [KEYBOARD_SHORTCUTS.md](KEYBOARD_SHORTCUTS.md)
for the file format.

**Generated — do not edit by hand.** Produced from
`crates/signex-app/src/keymap/catalog/` and
`crates/signex-app/assets/keyboard-shortcuts/` by
`crates/signex-app/tests/command_reference.rs`, which fails if this
file drifts. Regenerate with:

```sh
UPDATE_DOCS=1 cargo test -p signex-app --test command_reference
```

A command listed here resolves in the keymap. Not all of them reach an
action yet — the ones that do not are pinned in
`crates/signex-app/src/app/command/bridge.rs`, and that set may only
shrink.

## General

| command id | label | category | Altium | Classic |
| --- | --- | --- | --- | --- |
| `autoplace_fields` | Autoplace fields | edit | — | `O` |
| `break_wire` | Break wire | modify | `E W` | — |
| `cancel_current_tool` | Cancel current stage / exit placement mode | interactive | `Escape` | `Escape` |
| `center_on_cursor` | Center on cursor | view | — | `F4` |
| `center_view_at_cursor` | Center/redraw view around cursor | view | `Home` | — |
| `close_active_document` | Close active document | file | `Ctrl+F4` | — |
| `copy` | Copy | edit | `Ctrl+C` | `Ctrl+C` |
| `copy_attributes_or_add_vertex` | Copy attributes from object under cursor or add vertex while drawing | interactive | `Insert` | — |
| `cut` | Cut | edit | `Ctrl+X` | `Ctrl+X` |
| `cycle_fast_grid` | Cycle fast grid | view | — | `Alt+4` |
| `cycle_selection_mode` | Cycle selection mode | select | — | — |
| `cycle_snap_grid_backward` | Cycle backward through snap grids | view | `Shift+G` | — |
| `cycle_snap_grid_forward` | Cycle forward through snap grids | view | `G` | — |
| `cycle_unit` | Cycle display unit | view | `Ctrl+Q` | `Ctrl+Q` |
| `cycle_wire_bus_graphic_mode` | Cycle wire/bus/graphic-line mode: free, 90 degrees, 45 degrees | interactive | — | `Shift+Space` |
| `cycle_wiring_mode` | Change wiring mode while dragging connected electrical objects | interactive | `Ctrl+Space` | — |
| `delete_selection` | Delete selection | edit | `Delete` | `Delete` |
| `drag_keep_connections` | Drag while keeping connections | modify | — | `G` |
| `duplicate` | Duplicate | edit | `Ctrl+D` | `Ctrl+D` |
| `edit_footprint_field` | Edit footprint field | edit | — | `F` |
| `edit_library_symbol` | Edit library symbol | library | — | `Ctrl+Shift+E` |
| `edit_object_properties` | Edit properties of object under cursor | edit | `DoubleClick` | — |
| `edit_reference_designator` | Edit reference designator | edit | — | `U` |
| `edit_selected_object_properties` | Properties / edit selected object | edit | — | `E` |
| `edit_selected_symbol_in_symbol_editor` | Edit selected symbol in Symbol Editor | library | — | `Ctrl+E` |
| `edit_text_in_place` | In-place edit selected text | edit | `F2` | — |
| `edit_value` | Edit value | edit | — | `V` |
| `fast_grid_1` | Fast grid 1 | view | — | `Alt+1` |
| `fast_grid_2` | Fast grid 2 | view | — | `Alt+2` |
| `find` | Find | search | — | `Ctrl+F` |
| `find_and_replace` | Find and replace | search | `Ctrl+H` | — |
| `find_next` | Find next | search | `F3` | `F3` |
| `find_previous` | Find previous | search | — | `Shift+F3` |
| `find_similar_objects` | Find Similar Objects | select | `Shift+F` | — |
| `find_text` | Find text | search | `Ctrl+F` | — |
| `import_graphics` | Import graphics | library | — | `Ctrl+Shift+F` |
| `measure_distance` | Measure distance | view | `Ctrl+M` | — |
| `mirror_x` | Mirror along X-axis | modify | `X` | `X` |
| `mirror_y` | Mirror along Y-axis | modify | `Y` | `Y` |
| `move_object` | Move object | modify | `M M` | — |
| `move_selection` | Move | modify | — | `M` |
| `new_document` | New | file | — | `Ctrl+N` |
| `next_document_tab` | Next open document tab | window | `Ctrl+Tab` | — |
| `next_grid` | Next grid | view | — | `N` |
| `open_command_palette` | Open command palette | commands | `Ctrl+Shift+P` | `Ctrl+Shift+P` |
| `open_datasheet` | Open datasheet | edit | — | `D` |
| `open_document` | Open document | file | `Ctrl+O` | `Ctrl+O` |
| `open_grid_picker` | Open grid picker | view | — | — |
| `open_grid_properties` | Open grid properties | view | — | — |
| `open_preferences` | Open preferences | preferences | `Ctrl+,` | `Ctrl+,` |
| `open_schematic_preferences` | Open schematic preferences | preferences | `T P` | — |
| `paste` | Paste | edit | `Ctrl+V` | `Ctrl+V` |
| `paste_special` | Paste special | edit | — | `Ctrl+Shift+V` |
| `placement_accept` | Accept current placement or move stage | interactive | `Enter` | — |
| `placement_properties` | Edit properties of the object being placed or moved | interactive | `Tab` | — |
| `previous_document_tab` | Previous open document tab | window | `Ctrl+Shift+Tab` | — |
| `previous_grid` | Previous grid | view | — | `Shift+N` |
| `print` | Print | file | `Ctrl+P` | `Ctrl+P` |
| `redo` | Redo | edit | `Ctrl+Y` | `Ctrl+Y` |
| `refresh_view` | Refresh/redraw | view | `End` | `F5` |
| `remove_last_vertex` | Remove last placed vertex while drawing | interactive | `Backspace`, `Delete` | — |
| `repeat_last_item` | Repeat last item | edit | — | `Insert` |
| `report_manager_bom` | Report Manager / BOM | reports | `R I` | — |
| `reset_local_coordinates` | Reset local coordinates | view | — | `Space` |
| `rotate_clockwise` | Rotate clockwise by 90 degrees | modify | `Shift+Space` | `Shift+R` |
| `rotate_counterclockwise` | Rotate counterclockwise by 90 degrees | modify | `Space` | `R` |
| `rubber_stamp_copy` | Rubber-stamp copy / repeated paste | edit | `Ctrl+R` | — |
| `save_document` | Save document | file | `Ctrl+S` | `Ctrl+S` |
| `save_document_as` | Save as | file | — | `Ctrl+Shift+S` |
| `select_all` | Select all | select | `Ctrl+A` | `Ctrl+A` |
| `select_expand_connection` | Select / expand connection | select | — | `Ctrl+4` |
| `select_node_or_connection_item` | Select node / connection item under cursor | select | — | `Alt+3` |
| `show_all_design_objects` | Show all design objects | view | `Ctrl+PageDown` | — |
| `show_current_command_hotkeys` | Show graphical editing hotkey list for current command | help | `F1` | — |
| `show_current_command_shortcuts` | Show valid shortcuts for the current interactive command | help | `Shift+F1` | — |
| `smart_paste` | Smart Paste | edit | `Ctrl+Shift+V` | — |
| `switch_segment_posture` | Switch current segment posture | interactive | — | `/` |
| `toggle_auto_focus` | Toggle AutoFocus | view | `F9` | `F9` |
| `toggle_cross_select_mode` | Toggle Cross Select Mode | select | `Ctrl+Shift+X` | — |
| `toggle_electrical_grid` | Toggle electrical grid | view | `Shift+E` | — |
| `toggle_floating_panels` | Toggle floating panels | window | `F4` | — |
| `toggle_properties_panel` | Toggle Properties panel | panels | `F11` | — |
| `toggle_schematic_filter_panel` | Toggle schematic filter panel | panels | `F12` | — |
| `toggle_schematic_list_panel` | Toggle schematic list panel | panels | `Shift+F12` | — |
| `toggle_search_panel` | Show/hide search panel | search | — | `Ctrl+G` |
| `toggle_selection` | Add/remove object from selection | select | `Shift+Click` | — |
| `toggle_visible_grid` | Toggle visible grid | view | `Ctrl+Shift+G` | `Ctrl+Shift+G` |
| `undo` | Undo | edit | `Ctrl+Z` | `Ctrl+Z` |
| `undo_last_segment` | Undo last segment while drawing | interactive | — | `Backspace` |
| `unselect_all` | Unselect all | select | — | `Ctrl+Shift+A` |
| `zoom_in_at_cursor` | Zoom in at cursor | view | `PageUp` | `F1` |
| `zoom_out_at_cursor` | Zoom out at cursor | view | `PageDown` | `F2` |
| `zoom_to_all_objects` | Zoom to all objects | view | — | `Ctrl+Home` |
| `zoom_to_fit` | Zoom to fit | view | — | `Home` |
| `zoom_to_selection_area` | Zoom to selection area | view | — | `Ctrl+F5` |

## Schematic

| command id | label | category | Altium | Classic |
| --- | --- | --- | --- | --- |
| `annotate_schematic` | Annotate schematic | design | `T A` | — |
| `annotate_schematic_quietly` | Annotate schematic quietly | design | `Alt+A` | `Alt+A` |
| `clear_net_highlighting` | Clear net highlighting | select | — | `~` |
| `draw_graphic_line` | Draw graphic line | place | — | `I` |
| `draw_hierarchical_sheet` | Draw hierarchical sheet | place | — | `S` |
| `force_annotate_all_schematics` | Force annotate all schematics | design | `Shift+Alt+A` | `Shift+Alt+A` |
| `highlight_net_under_cursor` | Highlight net under cursor | select | — | ``` |
| `highlight_related_net_objects` | Highlight/select related net objects across sheets | select | `Alt+Click` | — |
| `leave_sheet` | Leave sheet / go to parent sheet | navigation | — | `Alt+Backspace` |
| `navigate_up_hierarchy` | Navigate up hierarchy | navigation | — | `Alt+Up` |
| `next_highlighted_net_item` | Next item on highlighted net | select | — | `Tab` |
| `next_sheet` | Next sheet | navigation | — | `PageDown` |
| `open_components_panel` | Open Components panel / place components | panels | `P P` | — |
| `open_net_color_palette` | Open net color palette | view | `F5` | — |
| `place_bus` | Draw bus | place | — | `B` |
| `place_compile_mask` | Place Compile Mask directive | place | `P V K` | — |
| `place_design_block` | Place design block | place | — | `Shift+B` |
| `place_global_label` | Place global label | place | — | `Ctrl+L` |
| `place_hierarchical_label` | Place hierarchical label | place | — | `H` |
| `place_junction` | Place junction | place | — | `J` |
| `place_local_net_label` | Place local net label | place | — | `L` |
| `place_net_label` | Place net label | place | `P N` | — |
| `place_no_connect` | Place no-connect flag | place | — | `Q` |
| `place_no_erc` | Place Generic No ERC directive | place | `P V N` | — |
| `place_power_symbol` | Place power symbol | place | — | `P` |
| `place_symbol` | Place symbol | place | — | `A` |
| `place_text` | Place text | place | — | `T` |
| `place_wire` | Place wire | place | `P W`, `Ctrl+W` | `W` |
| `place_wire_to_bus_entry` | Place wire-to-bus entry | place | — | `Z` |
| `previous_highlighted_net_item` | Previous item on highlighted net | select | — | `Shift+Tab` |
| `previous_sheet` | Previous sheet | navigation | — | `PageUp` |
| `reset_schematic_designators` | Reset schematic designators | design | `T A E` | — |
| `run_erc` | Run electrical rules check | validation | `F8` | `F8` |
| `sheet_navigation_back` | Sheet navigation back | navigation | — | `Alt+Left` |
| `sheet_navigation_forward` | Sheet navigation forward | navigation | — | `Alt+Right` |
| `update_pcb_from_schematic` | Update PCB from schematic | pcb_sync | — | — |

## PCB

_No commands in this group yet._

## 3D

| command id | label | category | Altium | Classic |
| --- | --- | --- | --- | --- |
| `footprint_mode_pads` | Switch footprint editor to Pads mode | library | `2` | `2` |
| `footprint_mode_sketch` | Switch footprint editor to Sketch mode | library | `1` | `1` |
| `footprint_mode_view_3d` | Switch footprint editor to 3D View mode | library | `3` | `3` |
