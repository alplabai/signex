//! States the mockup rows draw that a row's own entry does not capture,
//! named `<row>-<state>`. `open` fails when the state was not reached.

use super::helpers_c::drive;
use super::helpers_d::reached;
use super::helpers_f as h;
use super::view_click::click_view;
use super::{annot_valid, board_counters, project};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{OpenFn, Surface, WindowSel};
use iced::mouse::Button;
use signex_app::active_bar::{ActiveBarMenu, ActiveBarMsg};
use signex_app::app::{
    CommandPaletteMsg, ContextMenuMsg, LayerSetMenuMsg, Message, NetColorMsg, OverlayMsg,
    PcbBarMenu, PcbBarMsg, PcbMsg, ProjectMsg, ProjectSection, RulesTabMsg, Signex,
    StatusBarRequest, UiMsg,
};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelKind;
use signex_app::status_bar::BoardCounter;
use signex_drc_dsl::table::catalog::Domain;
use signex_widgets::passive_calculator::{CalculatorMessage, SiPrefix};

/// Where the Rules tab's toolbar can sit, below the tab strip.
const RULES_TOOLBAR: Crop = Crop {
    x: 240,
    y: 40,
    w: 1200,
    h: 320,
};
/// The PCB active bar.
const PCB_BAR: Crop = Crop {
    x: 300,
    y: 40,
    w: 840,
    h: 160,
};
/// The status bar along the bottom of the window.
const STATUS_BAR: Crop = Crop {
    x: 0,
    y: render::HEIGHT - 60,
    w: render::WIDTH,
    h: 60,
};

fn row(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    Surface {
        id,
        title,
        open,
        window: WindowSel::Main,
        checks: &[],
    }
}

pub fn rows() -> Vec<Surface> {
    let mut rows = global_rows();
    rows.extend(schematic_rows());
    rows.extend(board_rows());
    rows
}

fn global_rows() -> Vec<Surface> {
    vec![
        Surface {
            id: "m6-pcb",
            title: "Status bar on a board",
            open: |app, r, fx| {
                fixture::open_board(app, r, fx);
                Ok(())
            },
            window: WindowSel::MainCrop(STATUS_BAR),
            checks: &[],
        },
        row("m13-filter", "Shortcuts filtered by wire", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            settle_support::settle(app, Message::Menu(MenuMessage::OpenKeyboardShortcuts));
            let _ = app.update(Message::Overlay(OverlayMsg::ShortcutsFilterChanged(
                "wire".into(),
            )));
            reached(
                app.ui_state.keyboard_shortcuts_open,
                "the shortcuts reference closed",
            )
        }),
        row("m20-leaf", "Project tree: sheet menu", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            fixture::select_panel(app, PanelKind::Projects)?;
            app.interaction_state.last_mouse_pos = (160.0, 180.0);
            let leaf = project::sheet_key(app)?;
            let _ = app.update(Message::ContextMenu(ContextMenuMsg::ShowProjectTree(Some(
                leaf,
            ))));
            reached(
                app.interaction_state.project_tree_context_menu.is_some(),
                "the sheet row opened no menu",
            )
        }),
        Surface {
            id: "m35-none",
            title: "Project tab: no repository",
            open: |app, r, fx| {
                fixture::open_sample(app, r, fx);
                project::show_section(app, r, ProjectSection::VersionControl)
            },
            window: WindowSel::Main,
            checks: project::VERSION_CONTROL,
        },
    ]
}

fn schematic_rows() -> Vec<Surface> {
    vec![
        row("m61-net-color", "Active bar: Net Color", |app, r, fx| {
            active_bar_menu(app, r, fx, ActiveBarMenu::NetColor)
        }),
        row(
            "m61-filter",
            "Active bar: Selection Filter",
            |app, r, fx| active_bar_menu(app, r, fx, ActiveBarMenu::Filter),
        ),
        row("m61-power", "Active bar: Power", |app, r, fx| {
            active_bar_menu(app, r, fx, ActiveBarMenu::Power)
        }),
        row(
            "m62-selection",
            "Canvas menu with a selection",
            |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::select_parts(app, &["U1"])?;
                let _ = app.update(Message::ContextMenu(ContextMenuMsg::Show(720.0, 360.0)));
                Ok(())
            },
        ),
        row("m64-full", "Zoom presets, whole menu", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            let _ = app.update(Message::Ui(UiMsg::ZoomPresetsToggled));
            Ok(())
        }),
        Surface {
            // The palette has its own window.
            window: WindowSel::Auto,
            ..row("m71-chooser", "Net colour row chooser", |app, r, fx| {
                fixture::open_sample(app, r, fx);
                drive(app, Message::NetColor(NetColorMsg::Open))?;
                drive(
                    app,
                    Message::NetColor(NetColorMsg::RowOpened("+3V3".into())),
                )?;
                reached(
                    app.ui_state.net_color_palette.chooser.is_some(),
                    "the row opened no chooser",
                )
            })
        },
        row(
            "m91-presets",
            "Rules tab: Presets panel",
            open_rules_presets,
        ),
    ]
}

fn board_rows() -> Vec<Surface> {
    vec![
        row("m104-layer-set", "Layer set menu", |app, r, fx| {
            fixture::open_board(app, r, fx);
            app.interaction_state.last_mouse_pos = (420.0, 860.0);
            settle_support::settle(
                app,
                Message::Pcb(PcbMsg::LayerSetMenu(LayerSetMenuMsg::Opened {
                    window: None,
                })),
            );
            reached(
                app.interaction_state.pcb_layer_set_menu.is_some(),
                "the layer set menu did not open",
            )
        }),
        row("m108-copper", "PCB bar: Copper Areas", |app, r, fx| {
            fixture::open_board(app, r, fx);
            click_view(app, r, PCB_BAR, Button::Right, |message| {
                matches!(
                    message,
                    Message::Pcb(PcbMsg::Bar(PcbBarMsg::MenuRightPressed(PcbBarMenu::Group(g))))
                        if format!("{g:?}") == "CopperAreas"
                )
            })?;
            reached(
                app.interaction_state.pcb_bar_menu.is_some(),
                "the Copper Areas dropdown did not open",
            )
        }),
        row("m164-layer", "Counter: Active layer", |app, r, fx| {
            counter_popover(app, r, fx, BoardCounter::ActiveLayer)
        }),
        row("m164-grid", "Counter: Grid", |app, r, fx| {
            counter_popover(app, r, fx, BoardCounter::Grid)
        }),
        row(
            "m164-unrouted",
            "Counter: Unrouted",
            board_counters::open_unrouted,
        ),
        row("m164-drc", "Counter: DRC", board_counters::open_drc),
        row(
            "m164-selection",
            "Counter: Selection",
            board_counters::open_selection,
        ),
    ]
}

/// The sample's +5V net renamed VBUS, the net the mockup rows name.
pub(super) fn rename_to_vbus(fx: &Fixture) -> Result<(), String> {
    super::states::edit_sample_file(fx, |sheet| {
        for label in sheet.labels.iter_mut().filter(|l| l.text == "+5V") {
            label.text = "VBUS".to_string();
        }
        for part in sheet.symbols.iter_mut().filter(|s| s.value == "+5V") {
            part.value = "VBUS".to_string();
        }
    })
}

/// The palette searching a net the sample carries as VBUS.
pub(super) fn open_palette_vbus(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    rename_to_vbus(fx)?;
    fixture::open_sample(app, r, fx);
    super::helpers_d::sample_project(app, fx);
    let _ = app.update(Message::CommandPalette(CommandPaletteMsg::Open));
    settle_support::settle(
        app,
        Message::CommandPalette(CommandPaletteMsg::QueryChanged("vbu".into())),
    );
    reached(app.ui_state.command_palette.open, "the palette closed")
}

/// The calculator solved for a 4.52 k target.
pub(super) fn open_calculator_result(
    app: &mut Signex,
    _: &mut iced::Renderer,
    _: &Fixture,
) -> Result<(), String> {
    settle_support::settle(app, Message::Menu(MenuMessage::OpenPassiveCalculator));
    for msg in [
        CalculatorMessage::TargetChanged("4.52".into()),
        CalculatorMessage::PrefixChanged(SiPrefix::Kilo),
        CalculatorMessage::Calculate,
    ] {
        settle_support::settle(app, Message::PassiveCalculator(msg));
    }
    reached(
        app.ui_state.passive_calculator_open,
        "the calculator closed",
    )
}

/// The Clearance domain switched to its matrix by the toolbar's own chip.
pub(super) fn open_clearance_matrix(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let project = super::drawn_fixtures::open_rules_with_classes(app, r, fx)?;
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::DomainPicked {
            project_id: project,
            domain: Domain::Clearance,
        })),
    );
    let _ = render::draw_main(r, app);
    click_view(app, r, RULES_TOOLBAR, Button::Left, |message| {
        matches!(
            message,
            Message::Project(ProjectMsg::Rules(RulesTabMsg::ClearanceViewPicked { .. }))
        )
    })?;
    let shown = app
        .document_state
        .project_tabs
        .get(&project)
        .is_some_and(|tab| tab.rules.is_matrix_shown());
    reached(shown, "the Matrix chip did not show the matrix")?;
    super::drawn_fixtures::fill_clearance_matrix(app, project)?;
    let _ = render::draw_main(r, app);
    Ok(())
}

/// The Rules tab with its Presets panel, opened by the toolbar button.
fn open_rules_presets(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let project = annot_valid::open_rules(app, r, fx)?;
    let _ = render::draw_main(r, app);
    click_view(app, r, RULES_TOOLBAR, Button::Left, |message| {
        matches!(
            message,
            Message::Project(ProjectMsg::Rules(RulesTabMsg::PanelPicked {
                panel: Some(panel),
                ..
            })) if format!("{panel:?}") == "Presets"
        )
    })?;
    let shown = app
        .document_state
        .project_tabs
        .get(&project)
        .is_some_and(|tab| tab.rules.panel.is_some());
    reached(shown, "the Presets button opened no panel")
}

fn active_bar_menu(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    menu: ActiveBarMenu,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::ToggleMenu(menu)));
    Ok(())
}

pub(super) fn counter_popover(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    counter: BoardCounter,
) -> Result<(), String> {
    fixture::open_board(app, r, fx);
    show_counter(app, counter)
}

/// Click `counter` on the status bar of the board in view.
pub(super) fn show_counter(app: &mut Signex, counter: BoardCounter) -> Result<(), String> {
    let window = app.ui_state.main_window_id;
    let _ = app.update(Message::Ui(UiMsg::StatusBar(
        StatusBarRequest::BoardCounterClicked { window, counter },
    )));
    reached(
        app.ui_state.board_status.popover.is_some(),
        "the counter opened no popover",
    )
}
