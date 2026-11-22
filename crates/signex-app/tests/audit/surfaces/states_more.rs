//! More states the mockup rows draw: objects picked with real canvas clicks,
//! panels moved to a dock tall enough for them, and dialog states. `open`
//! fails when the state was not reached.

use super::canvas_pick::pick_in;
use super::helpers_d::{self, reached};
use super::helpers_f as h;
use super::pointer::PointerStep;
use super::states;
use super::states_drawn::rename_to_vbus;
use super::view_click::{click_view_stepped, view_offers};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{OpenFn, Surface, WindowSel};
use iced::mouse::Button;
use signex_app::active_bar::ActiveBarAction;
use signex_app::app::unions::{UnionPopoverMsg, UnionsMsg};
use signex_app::app::{
    EditMsg, Message, PcbMsg, PreferencesMsg, PrintPreviewMsg, ProjectMsg, RulesTabMsg, Signex,
    StatusBarRequest, TablesMsg, UiMsg,
};
use signex_app::dock::{DockMessage, DockTabAction, PanelPosition};
use signex_app::layer_strip_prefs::LayerSwitcher;
use signex_app::panels::{DrcPanelMsg, PanelKind, PanelMsg};
use signex_app::preferences::{PrefMsg, PrefNav};
use signex_app::status_bar::BoardCounter;
use signex_app::toolbar::ToolMessage;
use signex_app::workspace_layouts::SaveLayoutMsg;
use signex_types::schematic::{SchDirective, SchDrawing, SchematicSheet, SelectedKind};

/// The whole main window, for a scan that does not know where a dialog sits.
const WINDOW: Crop = Crop {
    x: 0,
    y: 0,
    w: render::WIDTH,
    h: render::HEIGHT,
};
/// Scan steps for wide rows and buttons.
const COARSE: (usize, usize) = (40, 8);
/// The PDF sections folded so Watermark, PDF Layers and Bookmarks show.
const PDF_FOLDED: [&str; 5] = ["Sheets", "Pages", "PageSetup", "Output", "Variant"];
/// A filled pin table for the placed table's cells.
const TABLE_CELLS: [[&str; 4]; 4] = [
    ["Pin", "Name", "Type", "Net"],
    ["1", "VIN", "Power In", "VBUS"],
    ["2", "GND", "Power In", "GND"],
    ["3", "VOUT", "Power Out", "+3V3"],
];

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
    vec![
        row("m5-dock-menu", "Dock tab context menu", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            app.interaction_state.last_mouse_pos = (120.0, 64.0);
            let _ = app.update(Message::Dock(DockMessage::TabContextMenuRequested(
                PanelPosition::Left,
                0,
            )));
            reached(
                app.document_state.dock.tab_menu.is_some(),
                "the dock tab opened no menu",
            )
        }),
        row("m6-layer-list", "Status bar: layer list", |app, r, fx| {
            fixture::open_board(app, r, fx);
            // The list belongs to the Dropdown switcher; under Strip the
            // pill opens the Active layer popover.
            app.ui_state.layer_switcher = LayerSwitcher::Dropdown;
            let window = app.ui_state.main_window_id;
            let _ = app.update(Message::Ui(UiMsg::StatusBar(
                StatusBarRequest::BoardCounterClicked {
                    window,
                    counter: BoardCounter::Layer,
                },
            )));
            reached(
                app.ui_state.board_status.popover.is_some(),
                "the layer pill opened no list",
            )
        }),
        row("m74-board", "Placement paused on a board", |app, r, fx| {
            fixture::open_board(app, r, fx);
            helpers_d::command(app, "place_board_via")?;
            let _ = app.update(Message::Tool(ToolMessage::PrePlacementTab));
            reached(
                app.interaction_state.pcb_tool.is_some(),
                "place_board_via armed no board tool",
            )
        }),
        row("m88-waive", "DRC: waive form open", open_drc_waive),
        row("m89-board", "Rules tab on a project board", |app, r, fx| {
            h::open_board_project(app, r, fx, false)?;
            let project = app
                .document_state
                .active_project
                .or_else(|| app.document_state.projects.first().map(|p| p.id))
                .ok_or("the board project is not loaded")?;
            settle_support::settle(
                app,
                Message::Project(ProjectMsg::Rules(RulesTabMsg::OpenRequested {
                    project: Some(project),
                })),
            );
            helpers_d::rules_tab_active(app)
        }),
        // Preferences is its own window, so these capture with `Auto`.
        Surface {
            window: WindowSel::Auto,
            ..row(
                "m143-dirty",
                "Preferences: search and edits",
                |app, _, _| {
                    settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
                    for msg in [
                        PrefMsg::NavSearchChanged("an".into()),
                        PrefMsg::DraftIncrementStep(2),
                    ] {
                        let _ = app.update(Message::Preferences(PreferencesMsg::Inner(msg)));
                    }
                    reached(app.ui_state.preferences_open, "Preferences closed")
                },
            )
        },
        Surface {
            window: WindowSel::Auto,
            ..row("m147-error", "Editing: fast grid refused", |app, _, _| {
                settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
                settle_support::settle(
                    app,
                    Message::Preferences(PreferencesMsg::Nav(PrefNav::Editing)),
                );
                let _ = app.update(Message::Preferences(PreferencesMsg::Inner(
                    PrefMsg::DraftFastGrid(1, "0.0000001".into()),
                )));
                reached(app.ui_state.preferences_open, "Preferences closed")
            })
        },
        row("m155-change", "Change Component picker", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            fixture::mount_stock(app, fx)?;
            h::select_parts(app, &["R1"])?;
            h::command(app, "change_component")?;
            reached(
                app.library.picker.is_some(),
                "change_component opened no part picker",
            )
        }),
        row("m156-contents", "Table of contents sheet", |app, r, fx| {
            states::open_hierarchy(app, r, fx)?;
            helpers_d::sample_project(app, fx);
            h::command(app, "add_table_of_contents")?;
            let listed = app.document_state.projects.iter().any(|p| {
                p.data
                    .sheets
                    .iter()
                    .any(|s| s.filename.contains("contents"))
            });
            reached(listed, "the project lists no contents sheet")?;
            // The sheet itself in front, so its table is what the row shows.
            let contents = h::new_dir(fx, "hierarchy")?.join("contents.snxsch");
            h::open(app, &contents)?;
            h::show_sheet(app, r);
            Ok(())
        }),
        // First point clicked; the pointer on the canvas draws the dimension
        // to it, with its value tag and the hint for the second click.
        Surface {
            window: WindowSel::MainPointer(&[PointerStep::HoverAt(760, 420)]),
            ..row("m157-placing", "Dimension being placed", |app, r, fx| {
                fixture::open_sample(app, r, fx);
                h::arm(app, ActiveBarAction::PlaceLinearDimension);
                h::click(app, 63.5, 132.08);
                Ok(())
            })
        },
        row("m158-popover", "Create Union popover", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            h::select_parts(app, &["C1", "U1", "C2"])?;
            h::open_panel(app, PanelKind::Unions);
            // The popover opens at the pointer: over the selected parts.
            app.interaction_state.last_mouse_pos = (640.0, 400.0);
            let mut msgs = vec![
                UnionsMsg::CreateRequested,
                UnionsMsg::Popover(UnionPopoverMsg::NameEdited("Regulator".into())),
            ];
            msgs.extend(tag_messages());
            for msg in msgs {
                let _ = app.update(Message::Edit(EditMsg::Unions(msg)));
            }
            Ok(())
        }),
        row("m158-hidden", "Union hidden", |app, r, fx| {
            open_tagged_union(app, r, fx)?;
            let sheet = app
                .document_state
                .active_path
                .clone()
                .ok_or("no active sheet")?;
            let union = active_sheet(app)?
                .unions
                .first()
                .map(|u| u.uuid)
                .ok_or("the union was not made")?;
            let _ = app.update(Message::Edit(EditMsg::Unions(UnionsMsg::HideToggled {
                sheet,
                union,
            })));
            Ok(())
        }),
        row("m166-save", "Save Layout dialog", |app, r, fx| {
            fixture::open_sample(app, r, fx);
            settle_support::settle(app, Message::from(SaveLayoutMsg::Requested));
            let _ = app.update(Message::from(SaveLayoutMsg::NameEdited("Review".into())));
            reached(
                app.ui_state.workspace_layouts.save.is_some(),
                "Save Layout did not open",
            )
        }),
    ]
}

/// Bring the panel `kind` to the front of a tall dock: one already outside
/// the bottom dock stays; a bottom one moves right through its tab menu, as
/// a user does.
pub(super) fn dock_right(app: &mut Signex, kind: PanelKind) -> Result<(), String> {
    let index = app
        .document_state
        .dock
        .panel_kinds(PanelPosition::Bottom)
        .iter()
        .position(|k| *k == kind);
    let Some(index) = index else {
        return fixture::select_panel(app, kind);
    };
    h::settle(
        app,
        Message::Dock(DockMessage::TabContextMenuRequested(
            PanelPosition::Bottom,
            index,
        )),
    )?;
    h::settle(
        app,
        Message::Dock(DockMessage::TabMenuActionPicked(DockTabAction::MoveTo(
            PanelPosition::Right,
        ))),
    )?;
    fixture::select_panel(app, kind)
}

fn active_sheet(app: &Signex) -> Result<&SchematicSheet, String> {
    Ok(app
        .document_state
        .active_engine()
        .ok_or("no active sheet")?
        .document())
}

fn tag_messages() -> Vec<UnionsMsg> {
    ["power-in", "emc", "usb"]
        .into_iter()
        .flat_map(|tag| {
            [
                UnionsMsg::Popover(UnionPopoverMsg::TagInputEdited(tag.into())),
                UnionsMsg::Popover(UnionPopoverMsg::TagSubmitted),
            ]
        })
        .collect()
}

/// The sample with a union of C1, U1 and C2 carrying three tags, the Unions
/// panel open.
pub(super) fn open_tagged_union(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    h::select_parts(app, &["C1", "U1", "C2"])?;
    let mut msgs = vec![
        UnionsMsg::CreateRequested,
        UnionsMsg::Popover(UnionPopoverMsg::NameEdited("Regulator".into())),
    ];
    msgs.extend(tag_messages());
    msgs.push(UnionsMsg::Popover(UnionPopoverMsg::Confirmed));
    for msg in msgs {
        let _ = app.update(Message::Edit(EditMsg::Unions(msg)));
    }
    h::open_panel(app, PanelKind::Unions);
    Ok(())
}

/// A harness connector with one entry, picked with a real click.
pub(super) fn open_harness_selected(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    h::arm(app, ActiveBarAction::PlaceHarnessConnector);
    h::click(app, 170.18, 50.8);
    h::click(app, 180.34, 50.8);
    h::arm(app, ActiveBarAction::PlaceHarnessEntry);
    h::click(app, 170.68, 51.8);
    let (id, from, to) = {
        let connector = active_sheet(app)?
            .harness
            .connectors
            .first()
            .ok_or("the connector tool placed nothing")?;
        let rows = connector.entries.len().max(1) as f64 + 1.0;
        let (x, y) = (connector.position.x, connector.position.y);
        (
            connector.uuid,
            (x, y),
            (x + connector.width, y + connector.entry_pitch * rows),
        )
    };
    pick_in(app, from, to, id, SelectedKind::HarnessConnector).map(|_| ())
}

/// A blanket, a differential pair mark and Net Parameters on VBUS, the Net
/// Parameters flag picked with a real click.
pub(super) fn open_net_parameters_selected(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    rename_to_vbus(fx)?;
    fixture::open_sample(app, r, fx);
    h::arm(app, ActiveBarAction::PlaceBlanket);
    h::click(app, 38.1, 88.9);
    h::click(app, 147.32, 127.0);
    h::arm(app, ActiveBarAction::PlaceDiffPair);
    h::click(app, 129.54, 99.06);
    h::arm(app, ActiveBarAction::PlaceParameterSet);
    h::click(app, 71.12, 99.06);
    let (id, x, y) = active_sheet(app)?
        .directives
        .iter()
        .find_map(|d| match d {
            SchDirective::NetParameters { uuid, position, .. } => {
                Some((*uuid, position.x, position.y))
            }
            _ => None,
        })
        .ok_or("the Net Parameters tool placed nothing")?;
    pick_in(
        app,
        (x - 6.0, y - 8.0),
        (x + 10.0, y + 2.0),
        id,
        SelectedKind::Directive,
    )
    .map(|_| ())
}

/// A placed table with its cells typed in, picked with a real click.
pub(super) fn open_table_selected(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    h::arm(app, ActiveBarAction::DrawTable);
    h::click(app, 162.56, 139.7);
    let (table, rows, columns, x, y) = active_sheet(app)?
        .drawings
        .iter()
        .find_map(|d| match d {
            SchDrawing::Table(t) => Some((
                t.uuid,
                t.rows.len(),
                t.columns.len(),
                t.position.x,
                t.position.y,
            )),
            _ => None,
        })
        .ok_or("the table tool placed nothing")?;
    for (row, cells) in TABLE_CELLS.iter().enumerate().take(rows) {
        for (column, text) in cells.iter().enumerate().take(columns) {
            let _ = app.update(Message::from(TablesMsg::CellEdited {
                table,
                row,
                column,
                text: (*text).to_string(),
            }));
        }
    }
    pick_in(
        app,
        (x, y),
        (x + 40.0, y + 25.0),
        table,
        SelectedKind::Drawing,
    )
    .map(|_| ())
}

/// A bezier and a dimension, the bezier picked with a real click so its
/// handles and Properties show.
pub(super) fn open_bezier_selected(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    h::arm(app, ActiveBarAction::DrawBezier);
    for (x, y) in [
        (167.64, 63.5),
        (172.72, 45.72),
        (190.5, 45.72),
        (195.58, 63.5),
    ] {
        h::click(app, x, y);
    }
    h::arm(app, ActiveBarAction::PlaceLinearDimension);
    for (x, y) in [(63.5, 132.08), (121.92, 132.08), (93.98, 139.7)] {
        h::click(app, x, y);
    }
    let id = active_sheet(app)?
        .drawings
        .iter()
        .find(|d| matches!(d, SchDrawing::Bezier(_)))
        .map(SchDrawing::uuid)
        .ok_or("the bezier tool placed nothing")?;
    pick_in(
        app,
        (167.64, 45.72),
        (195.58, 63.5),
        id,
        SelectedKind::Drawing,
    )
    .map(|_| ())
}

/// The PDF dialog with its options on and the earlier sections folded, each
/// by a click on its own heading (`PdfSection` is not exported).
pub(super) fn fold_pdf_sections(app: &mut Signex, r: &mut iced::Renderer) -> Result<(), String> {
    for name in PDF_FOLDED {
        let _ = render::draw_main(r, app);
        click_view_stepped(app, r, WINDOW, Button::Left, COARSE, |message| {
            matches!(
                message,
                Message::PrintPreview(PrintPreviewMsg::SectionToggled(section))
                    if format!("{section:?}") == name
            )
        })?;
    }
    Ok(())
}

/// A project whose root places two sheets, with its board in front and
/// Create Rooms from Sheets run on it.
pub(super) fn open_create_rooms(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let dir = h::new_dir(fx, "rooms")?;
    for file in ["dcdc.snxsch", "mcu.snxsch"] {
        h::copy_sample_sheet(fx, &dir.join(file))?;
    }
    let top = dir.join("top.snxsch");
    states::write_sheet(
        &top,
        states::root_sheet(vec![
            states::child_sheet("dcdc", "dcdc.snxsch", 50.8),
            states::child_sheet("mcu", "mcu.snxsch", 127.0),
        ]),
    )?;
    let board = dir.join("rooms.snxpcb");
    std::fs::copy(&fx.pcb, &board).map_err(|e| format!("copy board: {e}"))?;
    let prj = h::write_project(
        &dir,
        "rooms",
        &["top.snxsch", "dcdc.snxsch", "mcu.snxsch"],
        Some("rooms.snxpcb"),
    )?;
    h::open(app, &prj)?;
    h::open(app, &top)?;
    h::open(app, &board)?;
    h::show_board(app, r);
    h::command(app, "create_rooms_from_sheets")?;
    // The dialog's state is private to the crate: its Cancel button proves it.
    let open = view_offers(app, r, WINDOW, COARSE, |message| {
        matches!(message, Message::Pcb(PcbMsg::CreateRooms(_)))
            && format!("{message:?}").contains("Cancelled")
    });
    reached(open, "Create Rooms from Sheets opened no dialog")
}

/// The DRC findings in the right dock with the first row's waive form open.
fn open_drc_waive(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    states::open_drc_findings(app, r, fx)?;
    dock_right(app, PanelKind::Drc)?;
    let key = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .and_then(|drc| drc.rows.first())
        .map(|row| row.key.clone())
        .ok_or("the run listed no finding")?;
    h::settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::Drc(
            DrcPanelMsg::WaiveStarted(key),
        ))),
    )?;
    let open = app
        .document_state
        .panel_ctx
        .pcb_drc
        .as_ref()
        .is_some_and(|drc| drc.waive_draft.is_some());
    reached(open, "WaiveStarted opened no form")
}
