//! States of the `annot-valid`, `output-prefs` and `new-release` mockup rows
//! that their own entries do not reach. `open` fails when the state was not
//! reached.

use super::canvas_pick::pick_in;
use super::helpers_d::{self, reached};
use super::helpers_f as h;
use super::view_click::{click_view_stepped, view_offers};
use super::{states, states_drawn};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{OpenFn, Surface, WindowSel};
use iced::mouse::Button;
use signex_app::app::{
    AnnotateMsg, ErcMsg, Message, PreferencesMsg, ProjectId, ProjectMsg, RulesTabMsg, SheetSyncMsg,
    Signex, TileLayout, WindowMsg,
};
use signex_app::panels::PanelKind;
use signex_app::preferences::{PrefMsg, PrefNav};
use signex_app::workspace_layouts::SaveLayoutMsg;
use signex_drc_dsl::table::catalog::Domain;
use signex_engine::SheetSyncDirection;
use signex_types::format::SnxPcb;
use signex_types::net::NetClass;
use signex_types::pcb::{Room, RoomLayer, RoomOutline, RoomSource, SymbolLink};
use signex_types::schematic::{
    InstancePath, Label, LabelType, Point, SchematicSheet, SelectedKind, SheetPin,
};
use uuid::Uuid;

/// A height that shows a whole Preferences page without its scrollbar.
const TALL_PAGE: u32 = 1800;
/// The dialog window, for a scan that does not know where a control sits.
const DIALOG: Crop = Crop {
    x: 0,
    y: 0,
    w: render::DETACHED_SIZE.0,
    h: render::DETACHED_SIZE.1,
};
/// The limits a project stores, one rule per domain the grid lists.
const PROJECT_RULES: &str = "rule gap on clearance require clearance >= 0.2mm then error\n\
rule tr on track require width >= 0.15mm then error\n\
rule ho on hole require diameter >= 0.25mm then error\n\
rule mx on mask require expansion >= 0.05mm then error\n";

fn row(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    Surface {
        id,
        title,
        open,
        window: WindowSel::Auto,
        checks: &[],
    }
}

fn tall(id: &'static str, title: &'static str, open: OpenFn) -> Surface {
    Surface {
        window: WindowSel::DetachedTall(TALL_PAGE),
        ..row(id, title, open)
    }
}

pub fn rows() -> Vec<Surface> {
    vec![
        row(
            "m75-partial",
            "Annotate: child active",
            open_annotate_partial,
        ),
        row("m79-changes", "Sync: every kind of change", |app, r, fx| {
            open_sheet_sync(app, r, fx, SheetSyncDirection::SheetToSymbol)
        }),
        row("m79-removal", "Sync: removals to tick", open_sync_removal),
        row("m81-problems", "Update PCB: problems", open_update_problems),
        row("m84-stored", "ERC: project stores rules", open_erc_stored),
        row("m89-stored", "Rules: stored limits", open_rules_stored),
        row("m89-board-open", "Rules: board open", open_rules_board),
        row(
            "m91-presets-tier",
            "Presets: tier picked",
            open_preset_picked,
        ),
        row(
            "m143-dirty-appearance",
            "Preferences: dirty page kept by the filter",
            open_prefs_appearance_dirty,
        ),
        tall("m144-lower", "Appearance: whole page", |app, _, _| {
            prefs_page(app, PrefNav::Appearance)
        }),
        tall("m147-lower", "Editing: whole page", |app, _, _| {
            prefs_page(app, PrefNav::Editing)
        }),
        tall(
            "m147-error-lower",
            "Editing: refused fast grid, whole page",
            |app, _, _| {
                prefs_page(app, PrefNav::Editing)?;
                let _ = app.update(inner(PrefMsg::DraftFastGrid(1, "0.0000001".into())));
                reached(app.ui_state.preferences_open, "Preferences closed")
            },
        ),
        row("m163-room", "Room selected, part outside", open_room),
        row("m166-save-split", "Save Layout: tiled", open_save_split),
    ]
}

fn inner(msg: PrefMsg) -> Message {
    Message::Preferences(PreferencesMsg::Inner(msg))
}

fn prefs_page(app: &mut Signex, nav: PrefNav) -> Result<(), String> {
    settle_support::settle(app, Message::Preferences(PreferencesMsg::Open));
    settle_support::settle(app, Message::Preferences(PreferencesMsg::Nav(nav)));
    reached(app.ui_state.preferences_open, "Preferences did not open")
}

/// An Appearance draft under the `an` filter, which keeps that page listed.
fn open_prefs_appearance_dirty(
    app: &mut Signex,
    _: &mut iced::Renderer,
    _: &Fixture,
) -> Result<(), String> {
    prefs_page(app, PrefNav::Appearance)?;
    let pan = !app.ui_state.trackpad_pan;
    for msg in [
        PrefMsg::NavSearchChanged("an".into()),
        PrefMsg::DraftTrackpadPan(pan),
    ] {
        let _ = app.update(inner(msg));
    }
    reached(
        app.ui_state
            .preferences_pages
            .dirty_pages
            .contains(&PrefNav::Appearance),
        "the trackpad draft left Appearance clean",
    )
}

/// Annotate opened from a child sheet, with one sheet unticked in its table.
fn open_annotate_partial(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    states::open_hierarchy(app, r, fx)?;
    h::open(app, &h::new_dir(fx, "hierarchy")?.join("power.snxsch"))?;
    h::show_sheet(app, r);
    settle_support::settle(app, Message::Annotate(AnnotateMsg::OpenDialog));
    click_view_stepped(app, r, DIALOG, Button::Left, (10, 6), |message| {
        matches!(
            message,
            Message::Annotate(AnnotateMsg::SheetTickToggled(path))
                if path.ends_with("sensors.snxsch")
        )
    })?;
    reached(
        app.ui_state.annotate_session.unticked.len() == 1,
        "the sensors row left every sheet ticked",
    )
}

fn sheet_pin(name: &str, direction: &str, y: f64) -> SheetPin {
    SheetPin {
        uuid: Uuid::now_v7(),
        name: name.to_string(),
        direction: direction.to_string(),
        position: Point::new(50.8, y),
        rotation: 0.0,
        auto_generated: true,
        user_moved: false,
    }
}

fn port(name: &str, shape: &str, y: f64) -> Label {
    Label {
        shape: shape.to_string(),
        ..Label::new(name, Point::new(25.4, y), LabelType::Hierarchical)
    }
}

/// U_Power against a sheet that added ports, dropped VSENSE and renamed VIN.
/// Two new inputs keep VSENSE from pairing with one of them as a rename.
fn open_sheet_sync(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    direction: SheetSyncDirection,
) -> Result<(), String> {
    let dir = h::new_dir(fx, "sync")?;
    let child = SchematicSheet {
        uuid: Uuid::now_v7(),
        paper_size: "A4".to_string(),
        labels: vec![
            port("PGOOD", "output", 25.4),
            port("EN_5V", "input", 30.48),
            port("SYNC", "input", 35.56),
            port("VIN_12V", "bidirectional", 40.64),
        ],
        ..Default::default()
    };
    states::write_sheet(&dir.join("power.snxsch"), child)?;
    let mut symbol = states::child_sheet("U_Power", "power.snxsch", 50.8);
    symbol.pins = vec![
        sheet_pin("VSENSE", "input", symbol.position.y + 5.08),
        sheet_pin("VIN", "bidirectional", symbol.position.y + 10.16),
    ];
    let (id, x, y, (w, h_mm)) = (
        symbol.uuid,
        symbol.position.x,
        symbol.position.y,
        symbol.size,
    );
    let top = dir.join("top.snxsch");
    states::write_sheet(&top, states::root_sheet(vec![symbol]))?;
    h::open(app, &top)?;
    h::show_sheet(app, r);
    pick_in(app, (x, y), (x + w, y + h_mm), id, SelectedKind::ChildSheet)?;
    helpers_d::command(app, "sync_sheet_entries")?;
    reached(
        app.ui_state.sheet_sync.is_some(),
        "sync_sheet_entries opened no dialog",
    )?;
    let _ = app.update(Message::from(SheetSyncMsg::DirectionPicked(direction)));
    Ok(())
}

fn open_sync_removal(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_sheet_sync(app, r, fx, SheetSyncDirection::SymbolToSheet)?;
    reached(
        app.ui_state
            .sheet_sync
            .as_ref()
            .is_some_and(|state| state.shows_removal_ticks()),
        "Symbol to Sheet shows no removal ticks",
    )
}

/// The sample with an unannotated part, a duplicate and a missing footprint.
fn open_update_problems(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    states::edit_sample_file(fx, |sheet| {
        for part in &mut sheet.symbols {
            match part.reference.as_str() {
                "R1" => part.reference = "R?".to_string(),
                "C2" => part.reference = "C1".to_string(),
                "U1" => part.footprint = String::new(),
                _ => {}
            }
        }
    })?;
    helpers_d::sample_with_board(app, r, fx)?;
    helpers_d::command(app, "update_pcb_from_schematic")?;
    reached(
        app.ui_state
            .update_pcb
            .as_ref()
            .is_some_and(|state| state.is_ready()),
        "the Update PCB list is not ready",
    )?;
    let card = view_offers(app, r, DIALOG, (12, 6), |message| {
        format!("{message:?}").contains("AnnotateRequested")
    });
    reached(card, "the dialog shows no Problems card")
}

/// A project with its own rule text, net classes and named sets, its sheet
/// open and, with `board_tab`, its board too.
fn open_rules_project(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    board_tab: bool,
) -> Result<ProjectId, String> {
    let dir = h::new_dir(fx, "rules-project")?;
    let sheet = dir.join("linear-regulator.snxsch");
    h::copy_sample_sheet(fx, &sheet)?;
    let board = dir.join("Proj.snxpcb");
    std::fs::copy(&fx.pcb, &board).map_err(|e| format!("copy board: {e}"))?;
    let prj = h::write_project(
        &dir,
        "Proj",
        &["linear-regulator.snxsch"],
        Some("Proj.snxpcb"),
    )?;
    let mut data =
        signex_types::project::parse_project(&prj).map_err(|e| format!("read project: {e}"))?;
    data.rules = Some(
        serde_json::from_value(serde_json::json!({
            "drc": {
                "dsl": PROJECT_RULES,
                "sets": {
                    "Power": {"limits": {"min_clearance": "0.3mm", "min_width": "0.5mm"}},
                    "High speed": {"limits": {"min_clearance": "0.15mm"}},
                    "Fine pitch": {"limits": {"min_width": "0.1mm"}}
                }
            }
        }))
        .map_err(|e| format!("project rules: {e}"))?,
    );
    data.net_classes = vec![NetClass {
        name: "Power".to_string(),
        nets: vec!["GND".to_string()],
        clearance: 0.3,
        trace_width: 0.5,
        ..Default::default()
    }];
    signex_types::project::write_project(&prj, &data).map_err(|e| format!("write project: {e}"))?;
    h::open(app, &prj)?;
    h::open(app, &sheet)?;
    h::show_sheet(app, r);
    if board_tab {
        h::open(app, &board)?;
        h::show_board(app, r);
    }
    let id = helpers_d::sample_project(app, fx);
    let stored = helpers_d::project_mut(app, id).is_some_and(|p| p.data.rules.is_some());
    reached(stored, "the project loaded without its rules")?;
    Ok(id)
}

fn open_rules_tab(app: &mut Signex, project: ProjectId, domain: Domain) -> Result<(), String> {
    for msg in [
        RulesTabMsg::OpenRequested {
            project: Some(project),
        },
        RulesTabMsg::DomainPicked {
            project_id: project,
            domain,
        },
    ] {
        settle_support::settle(app, Message::Project(ProjectMsg::Rules(msg)));
    }
    helpers_d::rules_tab_active(app)
}

fn open_erc_stored(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    open_rules_project(app, r, fx, false)?;
    settle_support::settle(app, Message::Erc(ErcMsg::OpenDialog));
    reached(app.ui_state.erc_dialog_open, "the ERC dialog did not open")
}

fn open_rules_stored(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let project = open_rules_project(app, r, fx, true)?;
    open_rules_tab(app, project, Domain::Physical)
}

/// The board in a tab, so Run DRC is live and Clearance lists its layers.
fn open_rules_board(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    h::open_board_project(app, r, fx, true)?;
    let project = app
        .document_state
        .active_project
        .or_else(|| app.document_state.projects.first().map(|p| p.id))
        .ok_or("the board project is not loaded")?;
    open_rules_tab(app, project, Domain::Clearance)
}

fn open_preset_picked(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    states_drawn::open_rules_presets(app, r, fx)?;
    let project = app
        .document_state
        .active_project
        .ok_or("no active project")?;
    settle_support::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::PresetPicked {
            project_id: project,
            preset: "tier:advanced".into(),
        })),
    );
    let previewed = app
        .document_state
        .project_tabs
        .get(&project)
        .is_some_and(|tab| tab.rules.preview.is_some());
    reached(previewed, "the Advanced tier opened no preview")
}

/// Two documents tiled, so the Save Layout dialog has a split to store.
fn open_save_split(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    fixture::open_board(app, r, fx);
    settle_support::settle(
        app,
        Message::Window(WindowMsg::TileRequested(TileLayout::SideBySide)),
    );
    reached(app.tiled_panes().is_some(), "Tile made no panes")?;
    settle_support::settle(app, Message::from(SaveLayoutMsg::Requested));
    let _ = app.update(Message::from(SaveLayoutMsg::NameEdited("Review".into())));
    reached(
        app.ui_state.workspace_layouts.save.is_some(),
        "Save Layout did not open",
    )
}

/// A saved board whose parts are linked to the root sheet, with one room
/// round the first part, so the others count as outside it. The room is
/// picked by a click on its name, as a user selects it.
fn open_room(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    const HALF_SIDE_MM: f64 = 6.0;
    let dir = h::new_dir(fx, "room-project")?;
    let sheet = dir.join("linear-regulator.snxsch");
    h::copy_sample_sheet(fx, &sheet)?;
    let text = std::fs::read_to_string(&fx.pcb).map_err(|e| format!("read board: {e}"))?;
    let mut board = SnxPcb::parse(&text)
        .map_err(|e| format!("parse board: {e:?}"))?
        .board;
    for part in &mut board.footprints {
        part.schematic_link = Some(SymbolLink::new(InstancePath::root(), [part.uuid]));
    }
    let centre = board
        .footprints
        .first()
        .ok_or("the board has no parts")?
        .position;
    let room = Room {
        uuid: Uuid::now_v7(),
        name: "dcdc".to_string(),
        source: Some(RoomSource {
            sheet: InstancePath::root(),
            channel: None,
        }),
        layer: RoomLayer::Top,
        outline: RoomOutline::Rect {
            x_mm: centre.x - HALF_SIDE_MM,
            y_mm: centre.y - HALF_SIDE_MM,
            w_mm: 2.0 * HALF_SIDE_MM,
            h_mm: 2.0 * HALF_SIDE_MM,
        },
        colour: "#A058C8".to_string(),
        keep_parts_inside: true,
        margin_mm: 0.5,
        locked: false,
    };
    let (lo, hi) = pcb_geom::room_label_box(&room).ok_or("the room has no label")?;
    board.rooms.push(room);
    let path = dir.join("Proj.snxpcb");
    let text = SnxPcb::new(board)
        .write_string()
        .map_err(|e| format!("encode board: {e:?}"))?;
    std::fs::write(&path, text).map_err(|e| format!("write board: {e}"))?;
    let prj = h::write_project(
        &dir,
        "Proj",
        &["linear-regulator.snxsch"],
        Some("Proj.snxpcb"),
    )?;
    h::open(app, &prj)?;
    h::open(app, &sheet)?;
    h::open(app, &path)?;
    h::show_board(app, r);
    fixture::select_panel(app, PanelKind::Properties)?;
    pick_room(app, r, (lo.x, lo.y), (hi.x, hi.y));
    let shown = app
        .document_state
        .panel_ctx
        .pcb_board
        .as_ref()
        .and_then(|summary| summary.room.as_ref())
        .is_some_and(|facts| {
            let outside = facts.outside().len();
            outside > 0 && outside < facts.parts.len()
        });
    reached(shown, "no room is selected with parts both in and out")
}

/// Click across the room's name until Properties shows the room.
fn pick_room(app: &mut Signex, r: &mut iced::Renderer, lo: (f64, f64), hi: (f64, f64)) {
    const STEPS: u32 = 4;
    for row in 0..=STEPS {
        for column in 0..=STEPS {
            let x = lo.0 + (hi.0 - lo.0) * f64::from(column) / f64::from(STEPS);
            let y = lo.1 + (hi.1 - lo.1) * f64::from(row) / f64::from(STEPS);
            helpers_d::board_click(app, r, x, y);
            let picked = app
                .document_state
                .panel_ctx
                .pcb_board
                .as_ref()
                .is_some_and(|summary| summary.room.is_some());
            if picked {
                return;
            }
        }
    }
}
