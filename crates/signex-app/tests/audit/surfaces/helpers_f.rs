//! Set-up steps the `new_features` rows share: a catalog command, a panel,
//! a settle that never saves the dock layout, canvas clicks, and the extra
//! projects some rows need, written beside the fixture's copies so nothing
//! in the repo or the user's config is written.

use crate::fixture::Fixture;
use crate::render;
use signex_app::active_bar::{ActiveBarAction, ActiveBarMsg};
use signex_app::app::{FileMsg, Message, OverlayMsg, Signex};
use signex_app::canvas::CanvasEvent;
use signex_app::keymap::AppCommandId;
use signex_app::library::LibraryMessage;
use signex_app::library::messages::{FootprintEditorMsg, PrimitiveEdit};
use signex_app::menu_bar::MenuMessage;
use signex_app::panels::PanelKind;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::{
    ChildSheet, FillType, Point, SchematicSheet, SelectedItem, SelectedKind,
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// The sheet symbol of the multi-channel project.
pub const REPEATED_SHEET: Uuid = Uuid::from_u128(0x0001_5101);

/// Send `message` and run every task it produces, the way the runtime
/// would, dropping the dock layout save so no panel change reaches the
/// config folder (as `annotations_panel.rs` does).
pub fn settle(app: &mut Signex, message: Message) -> Result<(), String> {
    let mut driver = TaskDriver::new();
    let mut queue = std::collections::VecDeque::from([app.update(message)]);
    let mut steps = 0usize;
    while let Some(task) = queue.pop_front() {
        for message in driver.outputs(task).map_err(|e| e.to_string())? {
            steps += 1;
            if steps > 1_000 {
                return Err("the task chain did not settle".into());
            }
            if !matches!(message, Message::DockLayout(_)) {
                queue.push_back(app.update(message));
            }
        }
    }
    Ok(())
}

/// Run catalog command `id` as its menu row does.
pub fn command(app: &mut Signex, id: &str) -> Result<(), String> {
    let id = AppCommandId::new(id)?;
    settle(app, Message::Menu(MenuMessage::Command(id)))
}

/// Open the dock panel `kind` as View ▸ Panels does.
pub fn open_panel(app: &mut Signex, kind: PanelKind) {
    let _ = app.update(Message::Overlay(OverlayMsg::OpenPanel(kind)));
}

/// Whether the dock holds a tab of `kind`.
pub fn docked(app: &Signex, kind: PanelKind) -> Result<(), String> {
    match app.document_state.dock.locate(kind) {
        Some(_) => Ok(()),
        None => Err(format!("no {kind:?} tab in the dock")),
    }
}

/// Arm an active-bar tool.
pub fn arm(app: &mut Signex, action: ActiveBarAction) {
    let _ = app.update(Message::ActiveBar(ActiveBarMsg::Action(action)));
}

/// A left click on the schematic canvas at a sheet position in mm.
pub fn click(app: &mut Signex, x: f64, y: f64) {
    let _ = app.update(Message::CanvasEvent(CanvasEvent::Clicked {
        world_x: x,
        world_y: y,
    }));
}

/// Select the parts of the active sheet with these designators.
pub fn select_parts(app: &mut Signex, refs: &[&str]) -> Result<(), String> {
    let doc = app
        .document_state
        .active_engine()
        .ok_or("no active sheet")?
        .document();
    let items = refs
        .iter()
        .map(|r| {
            doc.symbols
                .iter()
                .find(|s| s.reference == *r)
                .map(|s| SelectedItem::new(s.uuid, SelectedKind::Symbol))
                .ok_or_else(|| format!("no {r} on the sheet"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    app.interaction_state.active_canvas_mut().selected = items;
    Ok(())
}

/// Select one object of the active sheet.
pub fn select_one(app: &mut Signex, id: Uuid, kind: SelectedKind) {
    app.interaction_state.active_canvas_mut().selected = vec![SelectedItem::new(id, kind)];
}

/// One footprint-editor (or board-sketch) event for the document at `path`.
pub fn fp_edit(app: &mut Signex, path: &Path, msg: FootprintEditorMsg) {
    let _ = app.update(Message::Library(LibraryMessage::PrimitiveEditorEvent {
        path: path.to_path_buf(),
        msg: PrimitiveEdit::Footprint(msg),
    }));
}

/// Draw once so the canvas knows its size, then fit the sheet paper.
pub fn show_sheet(app: &mut Signex, r: &mut iced::Renderer) {
    let _ = render::draw_main(r, app);
    app.interaction_state.active_canvas().fit_to_sheet();
    let _ = render::draw_main(r, app);
}

/// Draw once, then fit the board.
pub fn show_board(app: &mut Signex, r: &mut iced::Renderer) {
    let _ = render::draw_main(r, app);
    app.interaction_state.pcb_canvas.fit_to_board();
    let _ = render::draw_main(r, app);
}

fn io(what: &str, e: impl std::fmt::Display) -> String {
    format!("{what}: {e}")
}

/// A new folder `name` beside the fixture's copies.
fn new_dir(fx: &Fixture, name: &str) -> Result<PathBuf, String> {
    let root = fx
        .sample
        .parent()
        .and_then(Path::parent)
        .ok_or("the fixture has no temp root")?;
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).map_err(|e| io("project folder", e))?;
    Ok(dir)
}

fn sheet_entry(file: &str) -> SheetEntry {
    SheetEntry {
        name: file.to_string(),
        filename: file.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

/// `<name>.snxprj` in `dir` over `sheets` (the first is the root) and an
/// optional board.
fn write_project(
    dir: &Path,
    name: &str,
    sheets: &[&str],
    pcb: Option<&str>,
) -> Result<PathBuf, String> {
    let data = ProjectData {
        document_id: None,
        description: None,
        name: name.to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: sheets.first().map(|s| s.to_string()),
        pcb_file: pcb.map(str::to_string),
        sheets: sheets.iter().copied().map(sheet_entry).collect(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    };
    let prj = dir.join(format!("{name}.snxprj"));
    signex_types::project::write_project(&prj, &data).map_err(|e| io("write project", e))?;
    Ok(prj)
}

fn copy_sample_sheet(fx: &Fixture, to: &Path) -> Result<(), String> {
    std::fs::copy(fx.sample.with_extension("snxsch"), to)
        .map(|_| ())
        .map_err(|e| io("copy the sample sheet", e))
}

fn open(app: &mut Signex, path: &Path) -> Result<(), String> {
    settle(
        app,
        Message::File(FileMsg::Opened(Some(path.to_path_buf()))),
    )
}

/// A project whose root `top.snxsch` places the sample sheet once as the
/// repeated sheet symbol `Repeat(CH, 1, 4)`; both sheets open, the root in
/// front. Returns the root's path.
pub fn open_multichannel(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<PathBuf, String> {
    let dir = new_dir(fx, "multichannel")?;
    let child = dir.join("channel.snxsch");
    copy_sample_sheet(fx, &child)?;
    let mut root = SchematicSheet {
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    root.child_sheets.push(ChildSheet {
        uuid: REPEATED_SHEET,
        name: "Repeat(CH, 1, 4)".to_string(),
        filename: "channel.snxsch".to_string(),
        position: Point::new(101.6, 76.2),
        size: (50.8, 38.1),
        stroke_width: 0.0,
        fill: FillType::None,
        stroke_color: None,
        fill_color: None,
        fields_autoplaced: false,
        pins: Vec::new(),
        instances: Vec::new(),
    });
    let top = dir.join("top.snxsch");
    let text = signex_types::format::SnxSchematic::new(root)
        .write_string()
        .map_err(|e| io("write the root sheet", e))?;
    std::fs::write(&top, text).map_err(|e| io("write the root sheet", e))?;
    let prj = write_project(&dir, "dcdc-buck", &["top.snxsch", "channel.snxsch"], None)?;
    open(app, &prj)?;
    open(app, &child)?;
    open(app, &top)?;
    show_sheet(app, r);
    Ok(top)
}

/// A project with the sample sheet and a copy of the fixture board as its
/// PCB, the sheet open in front. With `board_tab` the board opens too, in
/// front.
pub fn open_board_project(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
    board_tab: bool,
) -> Result<(), String> {
    let dir = new_dir(fx, "board-project")?;
    let sheet = dir.join("linear-regulator.snxsch");
    copy_sample_sheet(fx, &sheet)?;
    let board = dir.join("Proj.snxpcb");
    std::fs::copy(&fx.pcb, &board).map_err(|e| io("copy the board", e))?;
    let prj = write_project(
        &dir,
        "Proj",
        &["linear-regulator.snxsch"],
        Some("Proj.snxpcb"),
    )?;
    open(app, &prj)?;
    open(app, &sheet)?;
    show_sheet(app, r);
    if board_tab {
        open(app, &board)?;
        show_board(app, r);
    }
    Ok(())
}

/// The sample sheet in a project with a Design Notebook folder of three
/// annotation blocks on its parts.
pub fn open_notebook_project(
    app: &mut Signex,
    r: &mut iced::Renderer,
    fx: &Fixture,
) -> Result<(), String> {
    let dir = new_dir(fx, "notebook-project")?;
    let sheet = dir.join("linear-regulator.snxsch");
    copy_sample_sheet(fx, &sheet)?;
    let notebook = dir.join("notebook");
    std::fs::create_dir_all(&notebook).map_err(|e| io("notebook folder", e))?;
    let text = "#annotation(kind: \"measurement\", status: \"done\", date: \"2026-11-19\", title: \"Output ripple\")[Ripple on #net(\"+3V3\"): 18 mV at #component(\"U1\").]\n\
\n\
#annotation(kind: \"issue\", status: \"open\", date: \"2026-11-20\")[U1 runs warm: #component(\"U1\") at 300 mA.]\n\
\n\
#annotation(kind: \"note\", date: \"2026-11-01\")[Rework: #component(\"R1\") replaced, #component(\"D1\") checked.]\n";
    std::fs::write(notebook.join("bringup.snxnbk"), text).map_err(|e| io("write notebook", e))?;
    let prj = write_project(&dir, "Notebook", &["linear-regulator.snxsch"], None)?;
    open(app, &prj)?;
    open(app, &sheet)?;
    show_sheet(app, r);
    Ok(())
}
