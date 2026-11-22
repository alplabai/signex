//! Mockup group `sch-b` (rows m58-m74).
//!
//! Every row opens the sample project and its sheet. Dialogs that act on a
//! selection select R1, C1 and the +5V label by id first.

use super::helpers_c::{self, drive, drive_in};
use crate::fixture;
use crate::render::Crop;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::active_bar::{ActiveBarMenu, ActiveBarMsg};
use signex_app::app::{
    ContextMenuMsg, ContextSubmenu, EditMsg, InputMsg, Message, MoveSelectionMsg, NetColorMsg,
    OverlayMsg, ParameterManagerMsg, PasteIntent, PasteRequest, SelectionFilterMsg, Signex,
    TextEditMsg, TextEditTarget, Tool, UiMsg,
};
use signex_app::dock::DockMessage;
use signex_app::find_replace::{FindReplaceMsg, FindScope};
use signex_app::panels::net_inspector::NetListStatus;
use signex_app::panels::{NetInspectorMsg, PanelKind, PanelMsg};
use signex_app::paste_array::PasteArrayMsg;
use signex_app::task_driver::TaskDriver;
use signex_app::toolbar::ToolMessage;
use signex_types::schematic::{SelectedItem, SelectedKind};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m58",
            title: "Net Inspector",
            // By Net with +3V3 expanded; the By Component, filter-no-match
            // and no-project states are not driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::NetInspector)?;
                inspector(app, NetInspectorMsg::NetToggled("+3V3".into()))
            },
            window: WindowSel::Auto,
            checks: NET_INSPECTOR,
        },
        Surface {
            id: "m59",
            title: "Variant quick actions",
            open: |_, _, _| {
                Err(
                    "not built: no status-bar variant switcher or In variant menu; \
                     PanelKind::Variants is the planned stub card"
                        .into(),
                )
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m60",
            title: "Favorites / Snippets / To-do panels + Design Notebook tab",
            // The Favorites stub; Snippets and To-Do show the same planned
            // card. The Design Notebook tab is row m60b.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Favorites)
            },
            window: WindowSel::Auto,
            checks: FAVORITES,
        },
        Surface {
            id: "m60b",
            title: "Design Notebook tab",
            open: |_, _, _| {
                Err(
                    "not built: no Design Notebook tab (Project > Design Notebook, \
                     .snxnbk pages); only the Annotations panel reads notebook files"
                        .into(),
                )
            },
            window: WindowSel::Auto,
            checks: &[],
        },
        Surface {
            id: "m61",
            title: "Schematic active bar + menus",
            // The Wire tool armed (corner-mode pill) and the Wiring menu
            // open; Net Color, Selection Filter and Power are the same
            // ToggleMenu with another ActiveBarMenu.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
                let _ = app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(
                    ActiveBarMenu::Wiring,
                )));
                Ok(())
            },
            window: WindowSel::Main,
            checks: ACTIVE_BAR,
        },
        Surface {
            id: "m62",
            title: "Canvas context menu (+ placement submenu)",
            // Empty area, nothing selected, Place flyout open. The "with
            // selection" state is the same menu over a selected part.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(720.0, 360.0)));
                let _ = app.update(Message::ContextMenu(ContextMenuMsg::SubmenuOpen(
                    ContextSubmenu::Place,
                )));
                Ok(())
            },
            window: WindowSel::Main,
            checks: CONTEXT_MENU,
        },
        Surface {
            id: "m63",
            title: "Grid picker",
            // The mockup's step ladder (`grid_picker_entries`). The app mounts
            // it only on a footprint tab, so the row opens one; the schematic
            // status-bar entry the mockup names does not exist.
            open: |app, r, fx| {
                helpers_c::open_footprint(app, r, fx)?;
                app.interaction_state.last_mouse_pos = (360.0, 420.0);
                // The `open_grid_picker` command.
                let _ = app.update(Message::Ui(UiMsg::GridPickerOpen));
                if app.interaction_state.grid_picker.is_none() {
                    return Err("GridPickerOpen mounted no ladder on a footprint tab".into());
                }
                Ok(())
            },
            window: WindowSel::Main,
            checks: &[],
        },
        Surface {
            id: "m64",
            title: "Zoom pill",
            // The percentage clicked, presets open; the resting pill is the
            // same corner without the toggle.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
                Ok(())
            },
            // The bottom-right quarter of the window, where the pill sits.
            window: WindowSel::MainCrop(Crop {
                x: 720,
                y: 450,
                w: 720,
                h: 450,
            }),
            checks: ZOOM_PILL,
        },
        Surface {
            id: "m65",
            title: "In-place text edit",
            // R1's designator, as a double-click on it opens.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                select(app, &["R1"], &[])?;
                let _ = app.update(Message::TextEdit(TextEditMsg::Requested(
                    TextEditTarget::Designator,
                )));
                Ok(())
            },
            window: WindowSel::Main,
            checks: TEXT_EDIT,
        },
        Surface {
            id: "m66",
            title: "Move Selection (dX/dY)",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                select(app, &["R1", "C1"], &["+5V"])?;
                drive(app, Message::MoveSelection(MoveSelectionMsg::Open))?;
                let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DxChanged(
                    "5.08".into(),
                )));
                let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DyChanged(
                    "2.54".into(),
                )));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: MOVE_SELECTION,
        },
        Surface {
            id: "m67",
            title: "Find and Replace",
            // Replace mode, project scope, a query with matches.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                drive(app, Message::Overlay(OverlayMsg::OpenReplace))?;
                drive(
                    app,
                    Message::FindReplaceMsg(FindReplaceMsg::ScopeChanged(FindScope::Project)),
                )?;
                drive(
                    app,
                    Message::FindReplaceMsg(FindReplaceMsg::QueryChanged("GND".into())),
                )
            },
            window: WindowSel::Auto,
            checks: FIND_REPLACE,
        },
        Surface {
            id: "m68",
            title: "Find Similar Objects",
            // Seeded from the one selected part, as the menu command is.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                select(app, &["R1"], &[])?;
                drive(app, helpers_c::command("find_similar_objects")?)
            },
            window: WindowSel::Auto,
            checks: FIND_SIMILAR,
        },
        Surface {
            id: "m69",
            title: "Paste Array",
            // Two parts and a label copied; four copies 10.16 mm apart on X.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                select(app, &["R1", "C1"], &["+5V"])?;
                // One driver, so the copy is on its clipboard for the paste.
                let mut driver = TaskDriver::new();
                drive_in(&mut driver, app, Message::Edit(EditMsg::Copy))?;
                drive_in(
                    &mut driver,
                    app,
                    Message::Edit(EditMsg::PasteArrayRequested),
                )?;
                if app.ui_state.paste_array.is_none() {
                    // The clipboard read went unanswered: hand back "nothing
                    // usable", so the copy made in the window is pasted.
                    let request = PasteRequest {
                        intent: PasteIntent::Array,
                        epoch: app.interaction_state.placement_epoch,
                        path: app.document_state.active_path.clone(),
                        dialog: 0,
                        from_agent: false,
                    };
                    drive(
                        app,
                        Message::Edit(EditMsg::PasteTextRead {
                            request,
                            text: None,
                        }),
                    )?;
                }
                for msg in [
                    PasteArrayMsg::CountChanged("4".into()),
                    PasteArrayMsg::XSpacingChanged("10.16".into()),
                    PasteArrayMsg::YSpacingChanged("0".into()),
                ] {
                    let _ = app.update(Message::Edit(EditMsg::PasteArray(msg)));
                }
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PASTE_ARRAY,
        },
        Surface {
            id: "m70",
            title: "Parameter Manager",
            // The table on the sample sheet; the filter-no-match and
            // no-schematic states are not driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                drive(app, Message::ParameterManager(ParameterManagerMsg::Open))
            },
            window: WindowSel::Auto,
            checks: PARAMETER_MANAGER,
        },
        Surface {
            id: "m71",
            title: "Net colour palette (F5)",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                drive(app, Message::NetColor(NetColorMsg::Open))
            },
            window: WindowSel::Auto,
            checks: NET_PALETTE,
        },
        Surface {
            id: "m72",
            title: "Custom net colour picker",
            // Custom… from the +3V3 row of the palette.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                drive(app, Message::NetColor(NetColorMsg::Open))?;
                drive(
                    app,
                    Message::NetColor(NetColorMsg::CustomOpenedFor("+3V3".into())),
                )
            },
            // The palette has its own window; the picker draws on the main one.
            window: WindowSel::Main,
            checks: NET_CUSTOM,
        },
        Surface {
            id: "m73",
            title: "Custom Selection Filter",
            // It opens from the footprint editor's Properties only, so the
            // row opens a stock footprint.
            open: |app, r, fx| {
                helpers_c::open_footprint(app, r, fx)?;
                drive(
                    app,
                    Message::SelectionFilter(SelectionFilterMsg::OpenCustom),
                )
            },
            window: WindowSel::Auto,
            checks: CUSTOM_FILTER,
        },
        Surface {
            id: "m74",
            title: "Placement paused banner / read-only badge",
            // Placement paused with Tab while drawing a wire. The read-only
            // badge needs a sheet written by a newer Signex and is not
            // driven.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                helpers_c::show_panel(app, PanelKind::Properties)?;
                let _ = app.update(Message::Tool(ToolMessage::SelectTool(Tool::Wire)));
                let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
                Ok(())
            },
            window: WindowSel::Main,
            checks: PLACEMENT_PAUSED,
        },
    ]
}

/// Select the parts with designators `parts` and the labels reading
/// `labels` on the active sheet.
fn select(app: &mut Signex, parts: &[&str], labels: &[&str]) -> Result<(), String> {
    let doc = app
        .document_state
        .active_engine()
        .ok_or("no active schematic engine")?
        .document();
    let mut items = Vec::new();
    for reference in parts {
        let symbol = doc
            .symbols
            .iter()
            .find(|s| s.reference == *reference)
            .ok_or_else(|| format!("no {reference} on the sheet"))?;
        items.push(SelectedItem::new(symbol.uuid, SelectedKind::Symbol));
    }
    for text in labels {
        let label = doc
            .labels
            .iter()
            .find(|l| l.text == *text)
            .ok_or_else(|| format!("no label {text} on the sheet"))?;
        items.push(SelectedItem::new(label.uuid, SelectedKind::Label));
    }
    app.interaction_state.active_canvas_mut().selected = items;
    Ok(())
}

fn inspector(app: &mut Signex, msg: NetInspectorMsg) -> Result<(), String> {
    drive(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::NetInspector(msg))),
    )
}

// ── Checks ──

const NET_INSPECTOR: &[Check] = &[
    ("the net list is ready", net_list_ready),
    ("a filter narrows the list", net_filter_narrows),
];

const FAVORITES: &[Check] = &[("the Favorites panel is docked", favorites_docked)];

const ACTIVE_BAR: &[Check] = &[
    ("the Wiring menu is open", wiring_menu_open),
    ("the wire tool is armed", wire_tool_armed),
];

const CONTEXT_MENU: &[Check] = &[("the canvas menu is open", context_menu_open)];

const ZOOM_PILL: &[Check] = &[
    ("the zoom presets are open", zoom_presets_open),
    ("dismissing closes the presets", zoom_presets_dismiss),
];

const TEXT_EDIT: &[Check] = &[
    ("R1's designator is being edited", designator_editing),
    ("Esc cancels the edit", text_edit_escape),
];

const MOVE_SELECTION: &[Check] = &[
    ("the dialog is open", move_selection_open),
    ("the typed offset is kept", move_selection_dx),
];

const FIND_REPLACE: &[Check] = &[
    (
        "Find and Replace is open in Replace mode",
        find_replace_open,
    ),
    ("the query has matches", find_has_matches),
    ("a query with no match lists none", find_no_match),
];

const FIND_SIMILAR: &[Check] = &[("the dialog is open", find_similar_open)];

const PASTE_ARRAY: &[Check] = &[("the dialog is open", paste_array_open)];

const PARAMETER_MANAGER: &[Check] = &[("the Parameter Manager is open", parameter_manager_open)];

const NET_PALETTE: &[Check] = &[
    ("the palette is open", net_palette_open),
    ("the filter is kept", net_palette_filter),
];

const NET_CUSTOM: &[Check] = &[
    ("the picker is open for +3V3", net_custom_open),
    ("a typed hex is kept", net_custom_hex),
];

const CUSTOM_FILTER: &[Check] = &[("the dialog is open", custom_filter_open)];

const PLACEMENT_PAUSED: &[Check] = &[("placement is paused", placement_paused)];

fn net_list_ready(app: &mut Signex) -> Result<(), String> {
    let status = app.document_state.panel_ctx.net_inspector.status;
    if status == NetListStatus::Ready {
        Ok(())
    } else {
        Err(format!("net list status is {status:?}"))
    }
}

fn net_filter_narrows(app: &mut Signex) -> Result<(), String> {
    inspector(app, NetInspectorMsg::FilterChanged("gnd".into()))?;
    let names: Vec<String> = app
        .document_state
        .panel_ctx
        .net_inspector
        .nets
        .iter()
        .map(|n| n.name.clone())
        .collect();
    if names.is_empty() || names.iter().any(|n| !n.to_lowercase().contains("gnd")) {
        return Err(format!("filter \"gnd\" lists {names:?}"));
    }
    Ok(())
}

fn favorites_docked(app: &mut Signex) -> Result<(), String> {
    match app.document_state.dock.locate(PanelKind::Favorites) {
        Some(_) => Ok(()),
        None => Err("no Favorites tab in any dock".into()),
    }
}

fn wiring_menu_open(app: &mut Signex) -> Result<(), String> {
    match app.interaction_state.active_bar_menu {
        Some(ActiveBarMenu::Wiring) => Ok(()),
        other => Err(format!("active_bar_menu is {other:?}")),
    }
}

fn wire_tool_armed(app: &mut Signex) -> Result<(), String> {
    match app.interaction_state.current_tool {
        Tool::Wire => Ok(()),
        tool => Err(format!("current tool is {tool:?}")),
    }
}

fn context_menu_open(app: &mut Signex) -> Result<(), String> {
    if app.interaction_state.context_menu.is_some() {
        Ok(())
    } else {
        Err("context_menu is None".into())
    }
}

fn zoom_presets_open(app: &mut Signex) -> Result<(), String> {
    if app.interaction_state.zoom_presets_open {
        Ok(())
    } else {
        Err("zoom_presets_open is false".into())
    }
}

fn zoom_presets_dismiss(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Ui(UiMsg::ZoomPresetsDismissed));
    if app.interaction_state.zoom_presets_open {
        Err("zoom_presets_open stayed true".into())
    } else {
        Ok(())
    }
}

fn designator_editing(app: &mut Signex) -> Result<(), String> {
    match app.interaction_state.editing_text.as_ref() {
        Some(edit) if edit.kind == SelectedKind::SymbolRefField && edit.text == "R1" => Ok(()),
        Some(edit) => Err(format!("editing {:?} {:?}", edit.kind, edit.text)),
        None => Err("editing_text is None".into()),
    }
}

fn text_edit_escape(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Input(InputMsg::EscapePressed { window: None }));
    if app.interaction_state.editing_text.is_some() {
        Err("the editor stayed open".into())
    } else {
        Ok(())
    }
}

fn move_selection_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.move_selection.open {
        Ok(())
    } else {
        Err("move_selection.open is false".into())
    }
}

fn move_selection_dx(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::MoveSelection(MoveSelectionMsg::DxChanged(
        "7.62".into(),
    )));
    match app.ui_state.move_selection.dx.as_str() {
        "7.62" => Ok(()),
        dx => Err(format!("dx is {dx:?}")),
    }
}

fn find_replace_open(app: &mut Signex) -> Result<(), String> {
    let state = &app.ui_state.find_replace;
    if state.open && state.replace_mode {
        Ok(())
    } else {
        Err(format!(
            "open {}, replace_mode {}",
            state.open, state.replace_mode
        ))
    }
}

fn find_has_matches(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.find_replace.matches.is_empty() {
        Err("no matches for GND".into())
    } else {
        Ok(())
    }
}

fn find_no_match(app: &mut Signex) -> Result<(), String> {
    drive(
        app,
        Message::FindReplaceMsg(FindReplaceMsg::QueryChanged("NO_SUCH_NET_42".into())),
    )?;
    match app.ui_state.find_replace.matches.len() {
        0 => Ok(()),
        n => Err(format!("{n} matches for a query that is nowhere")),
    }
}

fn find_similar_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.find_similar.is_some() {
        Ok(())
    } else {
        Err("find_similar is None".into())
    }
}

fn paste_array_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.paste_array.is_some() {
        Ok(())
    } else {
        Err("paste_array is None".into())
    }
}

fn parameter_manager_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.parameter_manager_open {
        Ok(())
    } else {
        Err("parameter_manager_open is false".into())
    }
}

fn net_palette_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.net_color_palette_open {
        Ok(())
    } else {
        Err("net_color_palette_open is false".into())
    }
}

fn net_palette_filter(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::NetColor(NetColorMsg::FilterChanged("GND".into())));
    match app.ui_state.net_color_palette.filter.as_str() {
        "GND" => Ok(()),
        filter => Err(format!("filter is {filter:?}")),
    }
}

fn net_custom_open(app: &mut Signex) -> Result<(), String> {
    let custom = &app.ui_state.net_color_custom;
    if custom.show && custom.net.as_deref() == Some("+3V3") {
        Ok(())
    } else {
        Err(format!("show {}, net {:?}", custom.show, custom.net))
    }
}

fn net_custom_hex(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::NetColor(NetColorMsg::CustomHexChanged(
        "#14B8A6".into(),
    )));
    // A hex that parses may move straight into the draft.
    let custom = &app.ui_state.net_color_custom;
    let typed = iced::Color::from_rgb8(0x14, 0xB8, 0xA6);
    if custom.hex.as_deref() == Some("#14B8A6") || custom.draft == typed {
        Ok(())
    } else {
        Err(format!("hex {:?}, draft {:?}", custom.hex, custom.draft))
    }
}

fn custom_filter_open(app: &mut Signex) -> Result<(), String> {
    if app.ui_state.selection_filter_custom.is_some() {
        Ok(())
    } else {
        Err("selection_filter_custom is None".into())
    }
}

fn placement_paused(app: &mut Signex) -> Result<(), String> {
    if app.interaction_state.active_canvas().placement_paused {
        Ok(())
    } else {
        Err("placement_paused is false".into())
    }
}
