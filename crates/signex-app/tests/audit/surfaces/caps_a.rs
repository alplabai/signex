//! States the `global` and `project` mockup rows draw that their own entries
//! do not reach: pinned and missing projects, the full tab strip, a schematic
//! selection, a recovery copy newer than the file, a requirements run and a
//! commit picked in the history graph. `open` fails when the state was not
//! reached.

use super::helpers_c;
use super::helpers_d::reached;
use super::helpers_f as h;
use super::pointer::PointerStep;
use super::view_click::click_view;
use super::{drawn_fixtures, project};
use crate::fixture::{self, Fixture};
use crate::render::{self, Crop};
use crate::settle_support;
use crate::surface::{OpenFn, Surface, WindowSel};
use iced::mouse::Button;
use serde_json::json;
use signex_app::app::{
    EditMsg, Message, ProjectId, ProjectMsg, ProjectSection, ProjectTreeAction, RulesTabMsg,
    Signex, TabKind,
};
use signex_app::autosave::sidecar;
use signex_app::canvas::CanvasEvent;
use signex_app::dock::DockMessage;
use signex_app::library::LibraryMessage;
use signex_app::panels::history::{HistoryCommitMsg, HistoryPanelMsg};
use signex_app::panels::{PanelKind, PanelMsg, ProjectsMsg};
use signex_app::recent_projects::RecentProjects;
use signex_app::start_screen::{self, StartScreenMsg};
use signex_app::tab_bar::TabMessage;
use signex_types::schematic::SelectedKind;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// The status bar along the bottom of the window.
const STATUS_BAR: Crop = Crop {
    x: 0,
    y: render::HEIGHT - 60,
    w: render::WIDTH,
    h: 60,
};
/// Where the Requirements header and its Run checks button sit.
const REQUIREMENTS_HEADER: Crop = Crop {
    x: 240,
    y: 40,
    w: 1200,
    h: 200,
};
/// The recent projects the Start screen lists, oldest first, with how many
/// hours ago each was opened.
const RECENT: [(&str, i64); 3] = [
    ("sensor-hub-rev-b", 14 * 24),
    ("motor-ctrl", 9 * 24),
    ("dcdc", 24),
];
/// Sheets of the overflow project: more tabs than a 1440 px strip fits.
const OVERFLOW_SHEETS: usize = 14;
/// How long before its recovery copy the sheet was last saved.
const SAVED_BEFORE_COPY: Duration = Duration::from_secs(30 * 60);

fn surface(id: &'static str, title: &'static str, open: OpenFn, window: WindowSel) -> Surface {
    Surface {
        id,
        title,
        open,
        window,
        checks: &[],
    }
}

pub fn rows() -> Vec<Surface> {
    let mut rows = start_rows();
    rows.extend(chrome_rows());
    rows.extend(project_rows());
    rows
}

fn start_rows() -> Vec<Surface> {
    vec![
        surface(
            "m1-pinned",
            "Start screen: pinned, missing and recent rows",
            |app, _, fx| seed_start(app, fx).map(|_| ()),
            WindowSel::Main,
        ),
        surface(
            "m1-hover",
            "Start screen: Pin on a hovered recent row",
            |app, _, fx| seed_start(app, fx).map(|_| ()),
            WindowSel::MainPointer(&[PointerStep::Hover("dcdc.snxprj")]),
        ),
        surface(
            "m3-pinned",
            "Start row menu: pinned",
            |app, _, fx| open_row_menu(app, fx, false),
            WindowSel::Auto,
        ),
        surface(
            "m3-missing",
            "Start row menu: pinned, missing on disk",
            |app, _, fx| open_row_menu(app, fx, true),
            WindowSel::Auto,
        ),
    ]
}

fn chrome_rows() -> Vec<Surface> {
    vec![
        surface(
            "m5-strip",
            "Tab strip: PRJ, dirty SCH, PCB, read-only SCH, LIB",
            open_tab_strip,
            WindowSel::Main,
        ),
        surface(
            "m5-read-only",
            "Tab strip: read-only tab tooltip",
            open_tab_strip,
            WindowSel::MainPointer(&[PointerStep::Hover("usb-c")]),
        ),
        surface(
            "m5-overflow",
            "Tab strip: tool tabs and the overflow menu",
            open_tab_overflow,
            WindowSel::Main,
        ),
        surface(
            "m6-selection-sch",
            "Status bar: schematic selection pill",
            open_schematic_selection,
            WindowSel::MainCrop(STATUS_BAR),
        ),
        surface(
            "m15-copy-newer",
            "Autosave recovery: the copy is newer",
            open_copy_newer,
            WindowSel::Auto,
        ),
    ]
}

fn project_rows() -> Vec<Surface> {
    vec![
        surface(
            "m19-filter-position",
            "Projects filtered to one sheet of two",
            open_filtered_sheet,
            WindowSel::Main,
        ),
        surface(
            "m38-run",
            "Requirements after a run",
            open_requirements_run,
            WindowSel::Main,
        ),
        surface(
            "m39-graph",
            "History: working tree row and a picked commit",
            open_history_graph,
            WindowSel::Main,
        ),
    ]
}

fn first_project(app: &Signex) -> Result<ProjectId, String> {
    app.document_state
        .projects
        .first()
        .map(|p| p.id)
        .ok_or_else(|| "no project is loaded".to_string())
}

/// Rotate U1 of the sample sheet in front: a real edit, so the sheet is dirty.
fn edit_active_sheet(app: &mut Signex) {
    h::click(app, 88.9, 101.6);
    let _ = app.update(Message::Edit(EditMsg::RotateSelectedClockwise));
}

// ---- Start screen ----------------------------------------------------------

/// The pinned rows as the app spells their paths.
struct StartRows {
    pinned: PathBuf,
    missing: PathBuf,
}

/// A project `name` of one sheet, in its own folder.
fn start_project(fx: &Fixture, name: &str) -> Result<PathBuf, String> {
    let dir = h::new_dir(fx, name)?;
    let sheet = format!("{name}.snxsch");
    h::copy_sample_sheet(fx, &dir.join(&sheet))?;
    h::write_project(&dir, name, &[sheet.as_str()], None)
}

/// Three recent projects, one pinned project and one pinned project whose
/// file is gone. The pin tasks are dropped, so the user's prefs are not
/// written; the check of the pins runs for real.
fn seed_start(app: &mut Signex, fx: &Fixture) -> Result<StartRows, String> {
    fixture::mount_stock(app, fx)?;
    for path in app.ui_state.pinned_projects.entries().to_vec() {
        let _ = app.update(start_screen::message(StartScreenMsg::ProjectUnpinned(path)));
    }
    let carrier = start_project(fx, "e1m-carrier")?;
    let gone = h::new_dir(fx, "usb-pd-sink")?.join("usb-pd-sink.snxprj");
    let now = chrono::Utc::now().timestamp();
    let mut recent = RecentProjects::from_paths(Vec::new());
    for (name, hours) in RECENT {
        recent.record_at(&start_project(fx, name)?, now - hours * 3600);
    }
    recent.record_at(&carrier, now - 3600);
    app.ui_state.recent_projects = recent;
    for path in [&carrier, &gone] {
        let _ = app.update(start_screen::message(StartScreenMsg::ProjectPinned(
            path.clone(),
        )));
    }
    let check = app.check_pinned_projects();
    settle_support::settle_task(app, check);
    let pins = &app.ui_state.pinned_projects;
    let spelled = |file: &str| {
        pins.entries()
            .iter()
            .find(|p| p.ends_with(file))
            .cloned()
            .ok_or_else(|| format!("{file} is not pinned"))
    };
    let rows = StartRows {
        pinned: spelled("e1m-carrier.snxprj")?,
        missing: spelled("usb-pd-sink.snxprj")?,
    };
    reached(
        pins.is_missing(&rows.missing) && !pins.is_missing(&rows.pinned),
        "the pin check did not mark only the gone project",
    )?;
    Ok(rows)
}

fn open_row_menu(app: &mut Signex, fx: &Fixture, missing: bool) -> Result<(), String> {
    let rows = seed_start(app, fx)?;
    let path = if missing { rows.missing } else { rows.pinned };
    // The menu opens at the pointer: on the project's row.
    app.interaction_state.last_mouse_pos = (720.0, 420.0);
    let _ = app.update(start_screen::message(StartScreenMsg::RowRightClicked(path)));
    reached(
        app.interaction_state.start_row_menu.is_some(),
        "the row opened no menu",
    )
}

// ---- Tab strip -------------------------------------------------------------

fn main_tab(app: &mut Signex, msg: TabMessage) -> Result<(), String> {
    let window_id = app.ui_state.main_window_id.ok_or("no main window")?;
    h::settle(app, Message::Tab { window_id, msg })
}

/// The document tab of `file`; a Layer Stack tab shares its board's path.
fn tab_index(app: &Signex, file: &str) -> Result<usize, String> {
    app.document_state
        .tabs
        .iter()
        .position(|t| !matches!(t.kind, TabKind::LayerStack) && t.path.ends_with(file))
        .ok_or_else(|| format!("no tab for {file}"))
}

/// A project with its Project tab, an edited sheet in front, the board, a
/// sheet written by a newer Signex and a library tab.
fn open_tab_strip(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let dir = h::new_dir(fx, "e1m-carrier")?;
    h::copy_sample_sheet(fx, &dir.join("power.snxsch"))?;
    let newer = std::fs::read_to_string(fx.sample.with_extension("snxsch"))
        .map_err(|e| format!("read sample: {e}"))?
        .replacen("format = \"snxsch/1.1\"", "format = \"snxsch/1.99\"", 1);
    std::fs::write(dir.join("usb-c.snxsch"), newer).map_err(|e| format!("write sheet: {e}"))?;
    std::fs::copy(&fx.pcb, dir.join("e1m-carrier.snxpcb"))
        .map_err(|e| format!("copy board: {e}"))?;
    let prj = h::write_project(
        &dir,
        "e1m-carrier",
        &["power.snxsch", "usb-c.snxsch"],
        Some("e1m-carrier.snxpcb"),
    )?;
    h::open(app, &prj)?;
    let id = first_project(app)?;
    project::tree_action(
        app,
        ProjectTreeAction::OpenProjectOptions(vec![id.to_string()]),
    );
    for file in ["power.snxsch", "e1m-carrier.snxpcb", "usb-c.snxsch"] {
        h::open(app, &dir.join(file))?;
    }
    fixture::mount_stock(app, fx)?;
    h::settle(
        app,
        Message::Library(LibraryMessage::OpenLibraryBrowser(fx.stock.clone())),
    )?;
    let power = tab_index(app, "power.snxsch")?;
    main_tab(app, TabMessage::Select(power))?;
    h::show_sheet(app, r);
    edit_active_sheet(app);
    let _ = render::draw_main(r, app);
    let d = &app.document_state;
    let dirty = d
        .tabs
        .get(d.active_tab)
        .is_some_and(|t| d.is_document_dirty(&t.path));
    let read_only = d
        .engines
        .iter()
        .any(|(path, e)| path.ends_with("usb-c.snxsch") && e.document().is_read_only());
    reached(dirty, "the front sheet is not dirty")?;
    reached(read_only, "usb-c.snxsch did not open read-only")
}

/// A board with its Layer Stack and Rules tabs and more sheets than fit, the
/// Layer Stack in front and the "+N" menu open.
fn open_tab_overflow(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
    let dir = h::new_dir(fx, "carrier-overflow")?;
    let sheets: Vec<String> = (1..=OVERFLOW_SHEETS)
        .map(|n| format!("sheet-{n:02}.snxsch"))
        .collect();
    for sheet in &sheets {
        h::copy_sample_sheet(fx, &dir.join(sheet))?;
    }
    let board = dir.join("carrier.snxpcb");
    std::fs::copy(&fx.pcb, &board).map_err(|e| format!("copy board: {e}"))?;
    let names: Vec<&str> = sheets.iter().map(String::as_str).collect();
    let prj = h::write_project(&dir, "carrier", &names, Some("carrier.snxpcb"))?;
    h::open(app, &prj)?;
    let id = first_project(app)?;
    h::open(app, &board)?;
    h::show_board(app, r);
    h::open_panel(app, PanelKind::LayerStack);
    h::settle(
        app,
        Message::Project(ProjectMsg::Rules(RulesTabMsg::OpenRequested {
            project: Some(id),
        })),
    )?;
    for sheet in &sheets {
        h::open(app, &dir.join(sheet))?;
    }
    let tabs = &app.document_state.tabs;
    let stack = tabs
        .iter()
        .position(|t| matches!(t.kind, TabKind::LayerStack))
        .ok_or("the board opened no Layer Stack tab")?;
    reached(
        tabs.iter().any(|t| matches!(t.kind, TabKind::Rules(_))),
        "the project opened no Rules tab",
    )?;
    main_tab(app, TabMessage::Select(stack))?;
    let _ = render::draw_main(r, app);
    main_tab(app, TabMessage::OverflowMenuToggled)?;
    reached(
        app.ui_state.tab_overflow_menu_open,
        "the overflow menu did not open",
    )
}

// ---- Status bar and recovery -----------------------------------------------

/// U1 clicked, then C1 and a wire added with Ctrl: two parts and one wire.
fn open_schematic_selection(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    fixture::open_sample(app, r, fx);
    helpers_c::click_pick(app, (88.9, 101.6), SelectedKind::Symbol)?;
    for (world_x, world_y) in [(63.5, 102.87), (72.39, 99.06)] {
        helpers_c::drive(
            app,
            Message::CanvasEvent(CanvasEvent::CtrlClicked { world_x, world_y }),
        )?;
    }
    let kinds = helpers_c::selected_kinds(app);
    let count = |want: SelectedKind| kinds.iter().filter(|k| **k == want).count();
    let picked = count(SelectedKind::Symbol) == 2 && count(SelectedKind::Wire) == 1;
    reached(
        picked && kinds.len() == 3,
        "the selection is not two parts and one wire",
    )
}

/// A recovery copy written after the sheet was last saved.
fn open_copy_newer(app: &mut Signex, r: &mut iced::Renderer, fx: &Fixture) -> Result<(), String> {
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
    let saved = SystemTime::now() - SAVED_BEFORE_COPY;
    std::fs::File::options()
        .write(true)
        .open(&sheet)
        .and_then(|file| file.set_modified(saved))
        .map_err(|e| format!("age the sheet: {e}"))?;
    app.document_state.autosave.set_dir(Some(dir));
    fixture::open_sample(app, r, fx);
    let offer = app.document_state.autosave.current_offer();
    reached(
        offer.is_some_and(|o| !o.older_than_file),
        "no offer of a copy newer than the file",
    )
}

// ---- Project group ---------------------------------------------------------

/// A two-sheet project filtered to its second sheet, which keeps its place.
fn open_filtered_sheet(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    h::open_multichannel(app, r, fx)?;
    fixture::select_panel(app, PanelKind::Projects)?;
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::Projects(
        ProjectsMsg::FilterChanged("channel".to_string()),
    ))));
    Ok(())
}

/// One requirement of each kind with readable values.
fn requirement_entries() -> [serde_json::Value; 6] {
    [
        json!({"id": "erc-clean", "kind": "erc_clean", "allow_warnings": true}),
        json!({"id": "drc-clean", "kind": "drc_clean"}),
        json!({"id": "fits-enclosure", "kind": "board_size",
               "max_width_mm": 150, "max_height_mm": 120}),
        json!({"id": "released-parts", "kind": "parts_state", "states": ["released"]}),
        json!({"id": "power-tracks", "kind": "net_class", "nets": "GND",
               "min_trace_width_mm": 0.2, "min_clearance_mm": 0.15}),
        json!({"id": "unit-cost", "kind": "bom_cost", "quantity": 1000,
               "currency": "EUR", "max_unit_cost": 5}),
    ]
}

/// The Requirements section of a project with a board, after Run checks. The
/// run's message type is not exported, so its button is clicked.
fn open_requirements_run(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    h::open_board_project(app, r, fx, true)?;
    let id = first_project(app)?;
    let loaded = app
        .document_state
        .projects
        .first_mut()
        .ok_or("the board project is not loaded")?;
    for entry in requirement_entries() {
        let entry = serde_json::from_value(entry).map_err(|e| format!("requirement: {e}"))?;
        loaded.data.requirements.push(entry);
    }
    project::show_section(app, r, ProjectSection::Requirements)?;
    click_view(app, r, REQUIREMENTS_HEADER, Button::Left, |message| {
        format!("{message:?}").contains("RunRequested")
    })?;
    let _ = render::draw_main(r, app);
    let finished = app
        .document_state
        .project_tabs
        .get(&id)
        .and_then(|tab| tab.requirements.last_run.as_ref())
        .is_some_and(|run| run.report.is_ok());
    reached(finished, "Run checks did not finish with a report")
}

/// The merged history with the sheet edited, so the working-tree row shows,
/// and a commit picked, so its detail strip lists the changed files.
fn open_history_graph(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    drawn_fixtures::open_history_branches(app, r, fx)?;
    edit_active_sheet(app);
    let sha = app
        .document_state
        .panel_ctx
        .history
        .visible
        .iter()
        .find(|commit| commit.subject.starts_with("Swap Q2"))
        .map(|commit| commit.sha.clone())
        .ok_or("the graph lists no Swap Q2 commit")?;
    h::settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::History(
            HistoryPanelMsg::Commit(HistoryCommitMsg::Selected(sha)),
        ))),
    )?;
    let _ = render::draw_main(r, app);
    let history = &app.document_state.panel_ctx.history;
    reached(history.dirty, "the panel shows no working-tree row")?;
    reached(
        history.ui.selected.is_some() && !history.ui.files.is_empty(),
        "the picked commit lists no files",
    )
}
