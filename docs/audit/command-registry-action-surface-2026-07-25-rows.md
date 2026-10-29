# Command Registry action audit — row table

Companion to
[`command-registry-action-surface-2026-07-25.md`](./command-registry-action-surface-2026-07-25.md).
Measured on `trunk` @ `f211e7d1`, 2026-07-25. Read the parent document first —
it carries the method, the admission rule, and the gap analysis.

199 canonical commands, keyed by **command id** (the stable thing), with every
`Message` variant that dedups into each. The variant→id direction is derivable
from the last column.

## Columns

- **command id** — existing catalog id where one is bridged; otherwise a
  *proposed* id derived from the variant name. Proposed ids are not decided:
  they become public API once minted, and naming is Caner's call.
- **catalog ids** — further catalog ids the bridge maps to the same action.
  For `args? = yes` rows these are the ids the parameterised command would
  subsume, and the list includes the primary.
- **args?** — `yes` where a family of variants enumerates a data domain and
  should become one command plus an argument (§4 of the parent).
- **group** — `CommandGroup`. Inferred from the owning enum, not read from the
  catalog: `ActiveBarAction` / annotate / ERC / net-colour → `Schematic`;
  footprint- and symbol-editor enums → `ThreeD`; everything else → `General`.
  Existing catalog entries keep whatever they already declare.
- **enable (proposed)** — a **heuristic, not a measurement**. Derived from the id
  matching a selection-dependent verb (delete / rotate / mirror / flip / align /
  distribute / copy / cut / duplicate / drag / bring / send / move). The catalog
  records `Enablement::Always` for all 134 entries today (§6 of the parent), so
  there is nothing to read; every value in this column needs a human call.
- **status** — `bridged` (a catalog id reaches it through `core_to_message`),
  `dead key` (a catalog id exists and is key-bound but has no arm), `new` (no
  catalog id at all).

## Known imperfections

Stated rather than silently smoothed over.

**Dual-route duplicates.** Eleven rows look like duplicates of a bridged row and
are not. The same user-facing command reaches the app through *different message
variants* depending on the surface, and only one of those routes is bridged:

| menu / context route (unbridged) | keyboard route (bridged) |
| --- | --- |
| row 160 `place_wire` ← `MenuMessage::PlaceWire` | `place_wire` → `ToolMessage::SelectTool(Tool::Wire)` |
| row 132 `place_bus` ← `MenuMessage::PlaceBus` | `place_bus` → `SelectTool(Tool::Bus)` |
| row 142 `place_label` ← `MenuMessage::PlaceLabel` | `place_net_label` → `SelectTool(Tool::Label)` |
| row 157 `place_text` ← `SymbolToolMsg::PlaceText` | `place_text` → `SelectTool(Tool::Text)` |
| row 120 `open_components_panel` ← `MenuMessage::OpenComponentsPanel` | `open_components_panel` → `SelectTool(Tool::Component)` |
| row 167 `save` ← `MenuMessage::Save` | `save_document` → `FileMsg::Save` |
| row 78 `delete` ← `ContextAction::Delete`, `MenuMessage::Delete` | `delete_selection` → `EditMsg::DeleteSelected` |
| row 186 `smart_paste` ← `ContextAction::SmartPaste`, `MenuMessage::SmartPaste` | `paste_special` → `EditMsg::SmartPaste` |
| row 164 `rotate_selected` ← `ContextAction::RotateSelected` | `rotate_clockwise` → `EditMsg::RotateSelected` |
| row 191 `toggle_grid` ← `MenuMessage::ToggleGrid` | `toggle_visible_grid` → `UiMsg::GridToggle` |
| row 196 `zoom_fit` ← `MenuMessage::ZoomFit` | `zoom_to_fit` → `CanvasEvent::FitAll` |

This is a finding, not an artifact: **"the command is bridged" does not mean
"the menu row is registry-driven".** A menu row and its shortcut can diverge in
behaviour without anything failing, because they do not share a code path.
Collapsing each pair onto one command id is exactly what #271 has to do, and it
is why #271 is a behaviour change rather than a rewiring.

**`tool_select` vs `select_tool`.** Row 195 `tool_select`
(`ActiveBarAction::ToolSelect`) is the active bar's *select-tool mode*; row 35
`select_tool` is the parameterised tool picker. Different commands, confusingly
adjacent names — one of them should be renamed before either is minted.

**Acronym-derived names** were corrected by hand where the mechanical conversion
mangled them (`place_no_e_r_c` → `place_no_erc`, `rotate_selection_c_w` →
`rotate_selection_cw`, `mint_body3d` → `mint_body_3d`, `set_selection_mode2d` →
`set_selection_mode_2d`, `delete_silk_f` → `delete_silkscreen_front`). Any
remaining awkward name is a proposal to argue with, not a decision.

---

| # | command id | args? | group | enable (proposed) | status | message variant(s) |
|---:|---|---|---|---|---|---|
| 1 | `annotate_schematic` | no | Schematic | Always | bridged | `AnnotateMsg::OpenDialog` |
| 2 | `annotate_schematic_quietly` (catalog ids: `force_annotate_all_schematics`) | no | Schematic | Always | bridged | `AnnotateMsg::Run` |
| 3 | `cancel_current_tool` | no | General | Always | bridged | `Message::EscapePressed` |
| 4 | `center_view_at_cursor` (catalog ids: `show_all_design_objects`, `zoom_to_fit`) | no | General | Always | bridged | `CanvasEvent::FitAll` |
| 5 | `copy` | no | General | RequiresSelection | bridged | `ContextAction::Copy`, `EditMsg::Copy`, `MenuMessage::Copy` |
| 6 | `cut` | no | General | RequiresSelection | bridged | `ContextAction::Cut`, `EditMsg::Cut`, `MenuMessage::Cut` |
| 7 | `cycle_selection_mode` | no | General | Always | bridged | `Message::CycleSelectionMode` |
| 8 | `cycle_snap_grid_forward` (catalog ids: `open_grid_picker`) | no | General | Always | bridged | `UiMsg::GridPickerOpen` |
| 9 | `cycle_unit` | no | General | Always | bridged | `UiMsg::UnitCycled` |
| 10 | `cycle_wire_bus_graphic_mode` | no | General | Always | bridged | `ToolMessage::CycleDrawMode` |
| 11 | `delete_selection` (catalog ids: `remove_last_vertex`) | no | General | RequiresSelection | bridged | `EditMsg::DeleteSelected` |
| 12 | `duplicate` | no | General | RequiresSelection | bridged | `EditMsg::Duplicate`, `MenuMessage::Duplicate` |
| 13 | `find` (catalog ids: `find_text`) | no | General | Always | bridged | `MenuMessage::Find`, `OverlayMsg::OpenFind` |
| 14 | `find_and_replace` | no | General | Always | bridged | `OverlayMsg::OpenReplace` |
| 15 | `mirror_x` | no | General | RequiresSelection | bridged | `ContextAction::MirrorX`, `EditMsg::MirrorSelectedY` |
| 16 | `mirror_y` | no | General | RequiresSelection | bridged | `ContextAction::MirrorY`, `EditMsg::MirrorSelectedX` |
| 17 | `new_document` | no | General | Always | bridged | `MenuMessage::NewProject` |
| 18 | `open_command_palette` | no | General | Always | bridged | `CommandPaletteMsg::Open` |
| 19 | `open_document` | no | General | Always | bridged | `MenuMessage::OpenProject` |
| 20 | `open_grid_properties` | no | General | Always | bridged | `GridPropertiesMsg::Open` |
| 21 | `open_net_color_palette` | no | Schematic | Always | bridged | `NetColorMsg::Open` |
| 22 | `open_preferences` | no | General | Always | bridged | `MenuMessage::OpenPreferences`, `PreferencesMsg::Open` |
| 23 | `paste` | no | General | Always | bridged | `ContextAction::Paste`, `EditMsg::Paste`, `MenuMessage::Paste` |
| 24 | `paste_special` (catalog ids: `smart_paste`) | no | General | Always | bridged | `EditMsg::SmartPaste` |
| 25 | `placement_accept` | no | General | Always | bridged | `Message::LassoCommit` |
| 26 | `placement_properties` | no | General | Always | bridged | `ToolMessage::PrePlacementTab` |
| 27 | `print` | no | General | Always | bridged | `PrintPreviewMsg::Requested` |
| 28 | `redo` | no | General | Always | bridged | `EditMsg::Redo`, `MenuMessage::Redo` |
| 29 | `reset_schematic_designators` | no | Schematic | Always | bridged | `AnnotateMsg::OpenResetConfirm` |
| 30 | `rotate_clockwise` (catalog ids: `rotate_counterclockwise`) | no | General | RequiresSelection | bridged | `EditMsg::RotateSelected` |
| 31 | `run_erc` (catalog ids: `update_pcb_from_schematic`) | no | Schematic | Always | bridged | `ErcMsg::Run` |
| 32 | `save_document` | no | General | Always | bridged | `FileMsg::Save` |
| 33 | `save_document_as` | no | General | Always | bridged | `MenuMessage::SaveAs` |
| 34 | `select_all` | no | Schematic | Always | bridged | `ActiveBarAction::SelectAll`, `FootprintEditorMsg::ActiveBarSelectAll`, `MenuMessage::SelectAll`, `SelectionRequest::SelectAll` |
| 35 | `select_tool` (catalog ids: `open_components_panel`, `place_bus`, `place_local_net_label`, `place_net_label`, `place_symbol`, `place_text`, `place_wire`) | yes | General | Always | bridged | `ToolMessage::SelectTool` |
| 36 | `show_current_command_hotkeys` (catalog ids: `show_current_command_shortcuts`) | no | General | Always | bridged | `MenuMessage::OpenKeyboardShortcuts` |
| 37 | `toggle_auto_focus` | no | General | Always | bridged | `MenuMessage::ToggleAutoFocus`, `OverlayMsg::ToggleAutoFocus` |
| 38 | `toggle_electrical_grid` | no | General | Always | bridged | `UiMsg::ToggleSnapHotspots` |
| 39 | `toggle_visible_grid` | no | General | Always | bridged | `UiMsg::GridToggle` |
| 40 | `undo` | no | General | Always | bridged | `EditMsg::Undo`, `MenuMessage::Undo` |
| 41 | `add_component_library` | no | General | Always | **new** | `MenuMessage::AddComponentLibrary` |
| 42 | `add_constraint_for_selection` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SketchAddConstraintForSelection` |
| 43 | `add_library_component` | no | General | Always | **new** | `MenuMessage::AddLibraryComponent` |
| 44 | `add_library_footprint` | no | General | Always | **new** | `MenuMessage::AddLibraryFootprint` |
| 45 | `add_library_symbol` | no | General | Always | **new** | `MenuMessage::AddLibrarySymbol` |
| 46 | `add_new_sibling` | no | ThreeD | Always | **new** | `FootprintEditorMsg::AddNewSibling` |
| 47 | `add_pin` | no | ThreeD | Always | **new** | `SymbolToolMsg::AddPin` |
| 48 | `align_bottom` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignBottom` |
| 49 | `align_horizontal_centers` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignHorizontalCenters` |
| 50 | `align_left` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignLeft` |
| 51 | `align_open` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::AlignOpen` |
| 52 | `align_pads` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::AlignPads` |
| 53 | `align_right` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignRight` |
| 54 | `align_selected_to_grid` | no | ThreeD | RequiresSelection | **new** | `SymbolEditorMsg::AlignSelectedToGrid` |
| 55 | `align_selection_to_grid` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::ActiveBarAlignSelectionToGrid` |
| 56 | `align_to_grid` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignToGrid` |
| 57 | `align_top` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignTop` |
| 58 | `align_vertical_centers` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::AlignVerticalCenters` |
| 59 | `annotate_back` | no | General | Always | **new** | `MenuMessage::AnnotateBack` |
| 60 | `annotate_force_all` | no | General | Always | **new** | `MenuMessage::AnnotateForceAll` |
| 61 | `annotate_quietly` | no | General | Always | **new** | `MenuMessage::AnnotateQuietly` |
| 62 | `annotate_reset` | no | General | Always | **new** | `MenuMessage::AnnotateReset` |
| 63 | `annotate_reset_duplicates` | no | General | RequiresSelection | **new** | `MenuMessage::AnnotateResetDuplicates` |
| 64 | `annotate_sheets` | no | General | Always | **new** | `MenuMessage::AnnotateSheets` |
| 65 | `apply_custom_filter` | no | General | Always | **new** | `ActiveBarMsg::ApplyCustomFilter` |
| 66 | `apply_filter_preset` | no | ThreeD | Always | **new** | `FootprintEditorMsg::ApplyFilterPreset` |
| 67 | `bring_to_front` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::BringToFront` |
| 68 | `bring_to_front_of` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::BringToFrontOf` |
| 69 | `capture_filter_preset` | no | ThreeD | Always | **new** | `FootprintEditorMsg::CaptureFilterPreset` |
| 70 | `clear_all_net_colors` | no | Schematic | Always | **new** | `ActiveBarAction::ClearAllNetColors` |
| 71 | `clear_net_color` | no | Schematic | Always | **new** | `ActiveBarAction::ClearNetColor` |
| 72 | `clear_selection` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::ActiveBarClearSelection` |
| 73 | `close_main_window` | no | General | Always | **new** | `WindowMsg::CloseMainWindow` |
| 74 | `copy_pad` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::CopyPad` |
| 75 | `create_library_at` | no | General | Always | **new** | `LibraryMessage::CreateLibraryAt` |
| 76 | `cut_pad` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::CutPad` |
| 77 | `cycle_grid` | no | General | Always | **new** | `MenuMessage::CycleGrid` |
| 78 | `delete` | no | General | RequiresSelection | **new** | `ContextAction::Delete`, `MenuMessage::Delete` |
| 79 | `delete_selected` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::DeleteSelected`, `SymbolEditorMsg::DeleteSelected` |
| 80 | `delete_silkscreen_front` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::DeleteSilkF` |
| 81 | `deselect_all` | no | ThreeD | Always | **new** | `FootprintContextAction::DeselectAll` |
| 82 | `distribute_horizontally` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::DistributeHorizontally` |
| 83 | `distribute_vertically` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::DistributeVertically` |
| 84 | `document_options` | no | General | Always | **new** | `MenuMessage::ToolsDocumentOptions` |
| 85 | `drag` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::Drag` |
| 86 | `drag_selection` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::DragSelection` |
| 87 | `draw_arc` | no | Schematic | Always | **new** | `ActiveBarAction::DrawArc` |
| 88 | `draw_bezier` | no | Schematic | Always | **new** | `ActiveBarAction::DrawBezier` |
| 89 | `draw_bus` | no | Schematic | Always | **new** | `ActiveBarAction::DrawBus` |
| 90 | `draw_ellipse` | no | Schematic | Always | **new** | `ActiveBarAction::DrawEllipse` |
| 91 | `draw_elliptical_arc` | no | Schematic | Always | **new** | `ActiveBarAction::DrawEllipticalArc` |
| 92 | `draw_full_circle` | no | Schematic | Always | **new** | `ActiveBarAction::DrawFullCircle` |
| 93 | `draw_line` | no | Schematic | Always | **new** | `ActiveBarAction::DrawLine` |
| 94 | `draw_polygon` | no | Schematic | Always | **new** | `ActiveBarAction::DrawPolygon` |
| 95 | `draw_rectangle` | no | Schematic | Always | **new** | `ActiveBarAction::DrawRectangle` |
| 96 | `draw_round_rectangle` | no | Schematic | Always | **new** | `ActiveBarAction::DrawRoundRectangle` |
| 97 | `draw_wire` | no | Schematic | Always | **new** | `ActiveBarAction::DrawWire` |
| 98 | `exit` | no | General | Always | **new** | `MenuMessage::Exit` |
| 99 | `export_bom` | no | General | Always | **new** | `MenuMessage::ExportBom` |
| 100 | `export_netlist` | no | General | Always | **new** | `MenuMessage::ExportNetlist` |
| 101 | `export_pdf` | no | General | Always | **new** | `MenuMessage::ExportPdf` |
| 102 | `fit_to_window` | no | ThreeD | Always | **new** | `FootprintContextAction::FitToWindow` |
| 103 | `flip_selected_x` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::FlipSelectedX` |
| 104 | `flip_selected_y` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::FlipSelectedY` |
| 105 | `flip_selection` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::ActiveBarFlipSelection` |
| 106 | `generate_bom` | no | General | Always | **new** | `MenuMessage::GenerateBom` |
| 107 | `join_selection_into_polygon` | no | ThreeD | Always | **new** | `SymbolEditorMsg::JoinSelectionIntoPolygon` |
| 108 | `lasso_select` | no | Schematic | Always | **new** | `ActiveBarAction::LassoSelect` |
| 109 | `make_pad_from_profile` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SketchMakePadFromProfile` |
| 110 | `mint_body_3d` | no | ThreeD | Always | **new** | `FootprintEditorMsg::MintBody3d` |
| 111 | `mint_extruded_body_3d` | no | ThreeD | Always | **new** | `FootprintEditorMsg::MintExtrudedBody3d` |
| 112 | `move_by_open` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::MoveByOpen` |
| 113 | `move_origin_to_grid` | no | ThreeD | RequiresSelection | **new** | `FootprintEditorMsg::ActiveBarMoveOriginToGrid` |
| 114 | `move_selection` | no | Schematic | RequiresSelection | **dead key** | `ActiveBarAction::MoveSelection` |
| 115 | `move_selection_x_y` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::MoveSelectionXY` |
| 116 | `move_to_front` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::MoveToFront` |
| 117 | `new_component` | no | General | Always | **new** | `LibraryMessage::NewComponent` |
| 118 | `new_part` | no | General | Always | **new** | `MenuMessage::ToolsNewPart` |
| 119 | `open_child_sheet` | no | General | Always | **new** | `ContextAction::OpenChildSheet` |
| 120 | `open_components_panel` | no | General | Always | **new** | `MenuMessage::OpenComponentsPanel` |
| 121 | `open_erc_panel` | no | General | Always | **new** | `MenuMessage::OpenErcPanel` |
| 122 | `open_library` | no | General | Always | **new** | `MenuMessage::LibraryOpenLibrary` |
| 123 | `open_messages_panel` | no | General | Always | **new** | `MenuMessage::OpenMessagesPanel` |
| 124 | `open_navigator_panel` | no | General | Always | **new** | `MenuMessage::OpenNavigatorPanel` |
| 125 | `open_passive_calculator` | no | General | Always | **new** | `MenuMessage::OpenPassiveCalculator` |
| 126 | `open_projects_panel` | no | General | Always | **new** | `MenuMessage::OpenProjectsPanel` |
| 127 | `open_properties_panel` | no | General | Always | **new** | `MenuMessage::OpenPropertiesPanel` |
| 128 | `open_signal_panel` | no | General | Always | **new** | `MenuMessage::OpenSignalPanel` |
| 129 | `paste_pad` | no | ThreeD | Always | **new** | `FootprintEditorMsg::PastePad` |
| 130 | `place_arc` | no | ThreeD | Always | **new** | `SymbolToolMsg::PlaceArc` |
| 131 | `place_blanket` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceBlanket` |
| 132 | `place_bus` | no | General | Always | **new** | `MenuMessage::PlaceBus` |
| 133 | `place_bus_entry` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceBusEntry` |
| 134 | `place_circle` | no | ThreeD | Always | **new** | `SymbolToolMsg::PlaceCircle` |
| 135 | `place_compile_mask` | no | Schematic | Always | **dead key** | `ActiveBarAction::PlaceCompileMask` |
| 136 | `place_component` | no | General | Always | **new** | `MenuMessage::LibraryPlaceComponent`, `MenuMessage::PlaceComponent` |
| 137 | `place_device_sheet_symbol` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceDeviceSheetSymbol` |
| 138 | `place_diff_pair` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceDiffPair` |
| 139 | `place_graphic` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceGraphic` |
| 140 | `place_harness_connector` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceHarnessConnector` |
| 141 | `place_harness_entry` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceHarnessEntry` |
| 142 | `place_label` | no | General | Always | **new** | `MenuMessage::PlaceLabel` |
| 143 | `place_line` | no | ThreeD | Always | **new** | `SymbolToolMsg::PlaceLine` |
| 144 | `place_net_label` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceNetLabel` |
| 145 | `place_no_erc` | no | Schematic | Always | **dead key** | `ActiveBarAction::PlaceNoERC` |
| 146 | `place_note` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceNote` |
| 147 | `place_off_sheet_connector` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceOffSheetConnector` |
| 148 | `place_parameter_set` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceParameterSet` |
| 149 | `place_polygon` | no | ThreeD | Always | **new** | `SymbolToolMsg::PlacePolygon` |
| 150 | `place_port` | no | Schematic | Always | **new** | `ActiveBarAction::PlacePort` |
| 151 | `place_power_port` | yes | Schematic | Always | **new** | `ActiveBarAction::PlacePowerArrow`, `ActiveBarAction::PlacePowerBar`, `ActiveBarAction::PlacePowerCircle`, `ActiveBarAction::PlacePowerEarth`, `ActiveBarAction::PlacePowerGND`, `ActiveBarAction::PlacePowerMinus5`, `ActiveBarAction::PlacePowerPlus12`, `ActiveBarAction::PlacePowerPlus5`, `ActiveBarAction::PlacePowerSignalGND`, `ActiveBarAction::PlacePowerVCC`, `ActiveBarAction::PlacePowerWave` |
| 152 | `place_rectangle` | no | ThreeD | Always | **new** | `SymbolToolMsg::PlaceRectangle` |
| 153 | `place_reuse_block` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceReuseBlock` |
| 154 | `place_sheet_entry` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceSheetEntry` |
| 155 | `place_sheet_symbol` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceSheetSymbol` |
| 156 | `place_signal_harness` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceSignalHarness` |
| 157 | `place_text` | no | ThreeD | Always | **new** | `SymbolToolMsg::PlaceText` |
| 158 | `place_text_frame` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceTextFrame` |
| 159 | `place_text_string` | no | Schematic | Always | **new** | `ActiveBarAction::PlaceTextString` |
| 160 | `place_wire` | no | General | Always | **new** | `MenuMessage::PlaceWire` |
| 161 | `refresh_all_pricing` | no | General | Always | **new** | `LibraryMessage::LibraryRefreshAllPricing` |
| 162 | `remove_part` | no | General | RequiresSelection | **new** | `MenuMessage::ToolsRemovePart` |
| 163 | `replace` | no | General | Always | **new** | `MenuMessage::Replace` |
| 164 | `rotate_selected` | no | General | RequiresSelection | **new** | `ContextAction::RotateSelected` |
| 165 | `rotate_selection` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::RotateSelection`, `FootprintEditorMsg::ActiveBarRotateSelection` |
| 166 | `rotate_selection_cw` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::RotateSelectionCW` |
| 167 | `save` | no | General | Always | **new** | `MenuMessage::Save` |
| 168 | `select_all_on_layer` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SelectAllOnLayer` |
| 169 | `select_all_pads` | no | ThreeD | Always | **new** | `FootprintContextAction::SelectAllPads` |
| 170 | `select_all_symbol_objects` | no | ThreeD | Always | **new** | `SymbolSelectionMsg::All` |
| 171 | `select_connection` | no | Schematic | Always | **new** | `ActiveBarAction::SelectConnection` |
| 172 | `select_next_overlapped` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SelectNextOverlapped` |
| 173 | `select_off_grid_pads` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SelectOffGridPads` |
| 174 | `select_overlapped` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SelectOverlapped` |
| 175 | `send_to_back` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::SendToBack` |
| 176 | `send_to_back_of` | no | Schematic | RequiresSelection | **new** | `ActiveBarAction::SendToBackOf` |
| 177 | `set_footprint_editor_mode` | yes | ThreeD | Always | **new** | `FootprintEditorMsg::SetMode`, `Message::FootprintModeShortcut` |
| 178 | `set_net_color` | yes | Schematic | Always | **new** | `ActiveBarAction::NetColorBlue`, `ActiveBarAction::NetColorCustom`, `ActiveBarAction::NetColorDarkGreen`, `ActiveBarAction::NetColorFuchsia`, `ActiveBarAction::NetColorLightBlue`, `ActiveBarAction::NetColorLightGreen`, `ActiveBarAction::NetColorRed`, `ActiveBarAction::NetColorYellow` |
| 179 | `set_pads_tool` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SetPadsTool` |
| 180 | `set_selection_mode` | yes | Schematic | Always | **new** | `ActiveBarAction::InsideArea`, `ActiveBarAction::OutsideArea`, `ActiveBarAction::TouchingLine`, `ActiveBarAction::TouchingRectangle` |
| 181 | `set_selection_mode_2d` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SetSelectionMode2d` |
| 182 | `set_sketch_tool` | no | ThreeD | Always | **new** | `FootprintEditorMsg::ActiveBarSetSketchTool` |
| 183 | `set_snapping_mode` | no | ThreeD | Always | **new** | `FootprintEditorMsg::ActiveBarSetSnappingMode` |
| 184 | `set_tool_footprint_editor` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SketchSetTool` |
| 185 | `set_tool_symbol_editor` | no | ThreeD | Always | **new** | `SymbolEditorMsg::SetTool` |
| 186 | `smart_paste` | no | General | Always | **new** | `ContextAction::SmartPaste`, `MenuMessage::SmartPaste` |
| 187 | `toggle_all_filters` | no | General | Always | **new** | `ActiveBarMsg::ToggleAllFilters`, `FootprintEditorMsg::ToggleAllFilters` |
| 188 | `toggle_centerline` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SketchToggleCenterline` |
| 189 | `toggle_construction` | no | ThreeD | Always | **new** | `FootprintEditorMsg::SketchToggleConstruction` |
| 190 | `toggle_filter` | no | General | Always | **new** | `ActiveBarMsg::ToggleFilter` |
| 191 | `toggle_grid` | no | General | Always | **new** | `MenuMessage::ToggleGrid` |
| 192 | `toggle_selection` | no | Schematic | Always | **new** | `ActiveBarAction::ToggleSelection` |
| 193 | `toggle_selection_filter` | no | ThreeD | Always | **new** | `FootprintEditorMsg::ToggleSelectionFilter`, `SymbolEditorMsg::ToggleSelectionFilter` |
| 194 | `toggle_snap` | no | ThreeD | Always | **new** | `FootprintEditorMsg::ActiveBarToggleSnap` |
| 195 | `tool_select` | no | Schematic | Always | **new** | `ActiveBarAction::ToolSelect` |
| 196 | `zoom_fit` | no | General | Always | **new** | `MenuMessage::ZoomFit` |
| 197 | `zoom_in` | no | General | Always | **new** | `MenuMessage::ZoomIn` |
| 198 | `zoom_out` | no | General | Always | **new** | `MenuMessage::ZoomOut` |
| 199 | `zoom_to_fit_symbol` | no | ThreeD | Always | **new** | `SymbolEditorMsg::Fit` |
