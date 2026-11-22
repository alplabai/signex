//! Mockup group `global` (rows m1-m18).

use super::{checks, helpers_f, states_drawn};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{Check, Surface, WindowSel};
use signex_app::app::{
    ContextMenuMsg, EditMsg, FileMsg, Message, OverlayMsg, Signex, UiMsg, WindowMsg,
};
use signex_app::autosave::sidecar;
use signex_app::dock::{DockMessage, PanelPosition};
use signex_app::menu_bar::MenuMessage;
use signex_app::recent_projects::RecentProjects;
use signex_app::start_screen::{self, StartScreenMsg};

pub fn rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m1",
            title: "Start screen (New/Open/Sample/Shortcuts + recent projects)",
            // Libraries the app has loaded, as the start screen counts them.
            open: |app, _, fx| fixture::mount_stock(app, fx),
            window: WindowSel::Auto,
            checks: START,
        },
        Surface {
            id: "m2",
            title: "Project hint (project loaded, no document)",
            open: |app, _, fx| {
                settle_support::settle(
                    app,
                    Message::File(FileMsg::Opened(Some(fx.sample.clone()))),
                );
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PROJECT_HINT,
        },
        Surface {
            id: "m3",
            title: "Start row context menu",
            open: |app, _, fx| {
                app.ui_state.recent_projects = RecentProjects::from_paths([fx.sample.clone()]);
                // The menu opens at the pointer: on the recent project's row.
                app.interaction_state.last_mouse_pos = (720.0, 420.0);
                let _ = app.update(start_screen::message(StartScreenMsg::RowRightClicked(
                    fx.sample.clone(),
                )));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: START_ROW_MENU,
        },
        Surface {
            id: "m4",
            title: "Title bar + menu bar",
            // An open menu is iced_aw MenuBar state, not a Message, so only
            // the closed bar is captured.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                Ok(())
            },
            window: WindowSel::MainCrop(Crop {
                x: 0,
                y: 0,
                w: render::WIDTH,
                h: 100,
            }),
            checks: SCHEMATIC,
        },
        Surface {
            id: "m5",
            title: "Tab bar + tab context menu",
            // The document tab menu; the dock panel tab menu
            // (DockMessage::TabContextMenuRequested) is the other state.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                // The menu opens at the pointer: on the active tab.
                app.interaction_state.last_mouse_pos = (420.0, 64.0);
                let tab = app.document_state.active_tab;
                let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowTab(tab)));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: TAB_MENU,
        },
        Surface {
            id: "m6",
            title: "Status bar",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                Ok(())
            },
            window: WindowSel::MainCrop(Crop {
                x: 0,
                y: render::HEIGHT - 60,
                w: render::WIDTH,
                h: 60,
            }),
            checks: SCHEMATIC,
        },
        Surface {
            id: "m7",
            title: "Panel list (View > Panels)",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::Overlay(OverlayMsg::TogglePanelList));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: PANEL_LIST,
        },
        Surface {
            id: "m8",
            title: "Dock drag zones + detached panel window",
            // A left panel pulled out and dragged to the right edge, so the
            // floating card and the right drop zone show. The panel in its
            // own OS window is the other state.
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let _ = app.update(Message::Dock(DockMessage::UndockPanel(
                    PanelPosition::Left,
                    0,
                )));
                let _ = app.update(Message::Ui(UiMsg::DragMove(
                    render::WIDTH as f32 - 10.0,
                    450.0,
                )));
                Ok(())
            },
            window: WindowSel::Main,
            checks: DOCK_DRAG,
        },
        Surface {
            id: "m9",
            title: "Command palette",
            open: states_drawn::open_palette_vbus,
            window: WindowSel::Auto,
            checks: PALETTE,
        },
        Surface {
            id: "m10",
            title: "Hover tooltip",
            // The symbol card; the net card on a wire, pin or label is the
            // other state.
            open: open_hover_tooltip,
            window: WindowSel::Main,
            checks: SCHEMATIC,
        },
        Surface {
            id: "m11",
            title: "Error notice card",
            open: open_save_failed,
            window: WindowSel::Auto,
            checks: ERROR_NOTICE,
        },
        Surface {
            id: "m12",
            title: "About Signex",
            open: |app, _, _| {
                settle_support::settle(app, Message::Menu(MenuMessage::OpenAbout));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: ABOUT,
        },
        Surface {
            id: "m13",
            title: "Keyboard shortcuts reference",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                settle_support::settle(app, Message::Menu(MenuMessage::OpenKeyboardShortcuts));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: SHORTCUTS,
        },
        Surface {
            id: "m14",
            title: "First-run onboarding tour",
            // `fixture::fresh` turns the tour off; a first launch has it on.
            // No Dismiss check: dismissing records the tour as seen in prefs.
            open: |app, _, _| {
                app.ui_state.first_run_tour_open = true;
                Ok(())
            },
            window: WindowSel::Main,
            checks: TOUR,
        },
        Surface {
            id: "m15",
            title: "Autosave recovery prompt",
            open: open_autosave_recovery,
            window: WindowSel::Auto,
            checks: RECOVERY,
        },
        Surface {
            id: "m16",
            title: "Exit Signex - Unsaved Edits",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                let path = app
                    .document_state
                    .active_path
                    .clone()
                    .ok_or("no active document after opening the sample")?;
                app.document_state.mark_document_dirty(&path);
                settle_support::settle(app, Message::Window(WindowMsg::CloseMainWindow));
                Ok(())
            },
            window: WindowSel::Auto,
            checks: QUIT,
        },
        Surface {
            id: "m17",
            title: "Passive Network Calculator",
            open: states_drawn::open_calculator_result,
            window: WindowSel::Auto,
            checks: CALCULATOR,
        },
        Surface {
            id: "m18",
            title: "Signex AI Agent pane / journals",
            open: |_, _, _| {
                Err("not built: the agent pane is Pro only, not in the Community build".into())
            },
            window: WindowSel::Auto,
            checks: &[],
        },
    ]
}

/// A real failed Save: the sheet's folder refuses the write, so the card
/// carries the path and the OS error. The folder is writable again after.
fn open_save_failed(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    helpers_f::click(app, 88.9, 101.6);
    let _ = app.update(Message::Edit(EditMsg::RotateSelectedClockwise));
    let folder = fx.sample.parent().ok_or("the sample has no folder")?;
    let writable = std::fs::metadata(folder)
        .map_err(|e| format!("read folder: {e}"))?
        .permissions();
    let mut refused = writable.clone();
    refused.set_readonly(true);
    std::fs::set_permissions(folder, refused).map_err(|e| format!("lock folder: {e}"))?;
    let saved = helpers_f::unlogged(|| helpers_f::settle(app, Message::File(FileMsg::Save)));
    std::fs::set_permissions(folder, writable).map_err(|e| format!("unlock folder: {e}"))?;
    saved?;
    let has_system = app
        .document_state
        .error_notice
        .as_ref()
        .is_some_and(|notice| notice.system.is_some());
    flag(has_system, "Save reported no path and OS error")
}

/// The sample sheet with the pointer dwelled on U1 past the 700 ms delay.
fn open_hover_tooltip(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    let path = app
        .document_state
        .active_path
        .clone()
        .ok_or("no active document after opening the sample")?;
    let uuid = app
        .document_state
        .engines
        .get(&path)
        .ok_or("no engine for the active sheet")?
        .document()
        .symbols
        .iter()
        .find(|s| s.reference == "U1")
        .map(|s| s.uuid)
        .ok_or("no U1 on the sample sheet")?;
    let i = &mut app.interaction_state;
    i.hover_symbol_uuid = Some(uuid);
    i.hover_started_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(2));
    i.hover_screen_pos = Some((720.0, 420.0));
    Ok(())
}

/// The sample sheet opened while a recovery copy that differs from it waits
/// in a recovery directory outside the project folder.
fn open_autosave_recovery(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let sheet = fx.sample.with_extension("snxsch");
    let dir = fx
        .sample
        .parent()
        .and_then(|p| p.parent())
        .ok_or("the sample has no parent folder")?
        .join("recovery");
    let mut copy = std::fs::read(&sheet).map_err(|e| format!("read sample sheet: {e}"))?;
    copy.extend_from_slice(b"\n");
    sidecar::write(&dir, &sheet, &copy)?;
    app.document_state.autosave.set_dir(Some(dir));
    fixture::open_sample(app, r, fx);
    Ok(())
}

const START: &[Check] = &[("no document tab is open", checks::no_document_tab)];

const PROJECT_HINT: &[Check] = &[
    ("no document tab is open", checks::no_document_tab),
    ("a project is loaded", project_loaded),
];

const START_ROW_MENU: &[Check] = &[("the row menu is open", start_row_menu_open)];

const SCHEMATIC: &[Check] = &[("a schematic tab is active", checks::schematic_tab_active)];

const TAB_MENU: &[Check] = &[("the tab context menu is open", tab_menu_open)];

const PANEL_LIST: &[Check] = &[
    ("the panel list is open", panel_list_open),
    ("a second toggle closes it", panel_list_toggles_closed),
];

const DOCK_DRAG: &[Check] = &[
    ("a panel is floating", panel_floating),
    ("the right drop zone is previewed", right_drop_preview),
];

const PALETTE: &[Check] = &[("the palette is open", palette_open)];

const ERROR_NOTICE: &[Check] = &[
    ("the error card is shown", error_notice_shown),
    ("Dismiss clears it", error_notice_dismisses),
];

const ABOUT: &[Check] = &[
    ("About is open", about_open),
    ("Close closes it", about_closes),
];

const SHORTCUTS: &[Check] = &[
    ("the shortcuts reference is open", shortcuts_open),
    ("Close closes it", shortcuts_close),
];

const TOUR: &[Check] = &[("the tour card is open", tour_open)];

const RECOVERY: &[Check] = &[("a recovery copy is offered", recovery_offered)];

const QUIT: &[Check] = &[("the quit prompt lists the unsaved sheet", quit_prompt_open)];

const CALCULATOR: &[Check] = &[
    ("the calculator is open", calculator_open),
    ("Close closes it", calculator_closes),
];

fn flag(on: bool, what: &str) -> Result<(), String> {
    if on {
        Ok(())
    } else {
        Err(format!("{what} is false"))
    }
}

fn project_loaded(app: &mut Signex) -> Result<(), String> {
    flag(!app.document_state.projects.is_empty(), "a loaded project")
}

fn start_row_menu_open(app: &mut Signex) -> Result<(), String> {
    flag(
        app.interaction_state.start_row_menu.is_some(),
        "start_row_menu.is_some()",
    )
}

fn tab_menu_open(app: &mut Signex) -> Result<(), String> {
    flag(
        app.interaction_state.tab_context_menu.is_some(),
        "tab_context_menu.is_some()",
    )
}

fn panel_list_open(app: &mut Signex) -> Result<(), String> {
    flag(app.ui_state.panel_list_open, "panel_list_open")
}

fn panel_list_toggles_closed(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Overlay(OverlayMsg::TogglePanelList));
    flag(!app.ui_state.panel_list_open, "!panel_list_open")
}

fn panel_floating(app: &mut Signex) -> Result<(), String> {
    flag(
        !app.document_state.dock.floating.is_empty(),
        "a floating panel",
    )
}

fn right_drop_preview(app: &mut Signex) -> Result<(), String> {
    match app.document_state.dock.drop_preview {
        Some(PanelPosition::Right) => Ok(()),
        other => Err(format!("drop_preview is {other:?}, not Right")),
    }
}

fn palette_open(app: &mut Signex) -> Result<(), String> {
    flag(app.ui_state.command_palette.open, "command_palette.open")
}

fn error_notice_shown(app: &mut Signex) -> Result<(), String> {
    flag(
        app.document_state.error_notice.is_some(),
        "error_notice.is_some()",
    )
}

fn error_notice_dismisses(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Overlay(OverlayMsg::DismissErrorNotice));
    flag(
        app.document_state.error_notice.is_none(),
        "error_notice.is_none() after Dismiss",
    )
}

fn about_open(app: &mut Signex) -> Result<(), String> {
    flag(app.ui_state.about_open, "about_open")
}

fn about_closes(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Overlay(OverlayMsg::CloseAbout));
    flag(!app.ui_state.about_open, "!about_open after Close")
}

fn shortcuts_open(app: &mut Signex) -> Result<(), String> {
    flag(
        app.ui_state.keyboard_shortcuts_open,
        "keyboard_shortcuts_open",
    )
}

fn shortcuts_close(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Overlay(OverlayMsg::CloseKeyboardShortcuts));
    flag(
        !app.ui_state.keyboard_shortcuts_open,
        "!keyboard_shortcuts_open after Close",
    )
}

fn tour_open(app: &mut Signex) -> Result<(), String> {
    flag(app.ui_state.first_run_tour_open, "first_run_tour_open")
}

fn recovery_offered(app: &mut Signex) -> Result<(), String> {
    flag(
        app.document_state.autosave.current_offer().is_some(),
        "autosave.current_offer().is_some()",
    )
}

fn quit_prompt_open(app: &mut Signex) -> Result<(), String> {
    match &app.ui_state.app_quit_confirm {
        Some(modal) if !modal.dirty_paths.is_empty() => Ok(()),
        Some(_) => Err("the quit prompt lists no unsaved file".into()),
        None => Err("app_quit_confirm is None".into()),
    }
}

fn calculator_open(app: &mut Signex) -> Result<(), String> {
    flag(
        app.ui_state.passive_calculator_open,
        "passive_calculator_open",
    )
}

fn calculator_closes(app: &mut Signex) -> Result<(), String> {
    let _ = app.update(Message::Overlay(OverlayMsg::ClosePassiveCalculator));
    flag(
        !app.ui_state.passive_calculator_open,
        "!passive_calculator_open after Close",
    )
}
