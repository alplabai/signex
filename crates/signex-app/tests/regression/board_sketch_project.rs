//! #2331 review - a board a project names is that project's while it is
//! open: Close Project applies the board's sketch first, a refused bake
//! holds the close, and a board in its own window asks for its project's
//! sheet hierarchy.

use std::path::{Path, PathBuf};

use signex_app::app::{FileMsg, LoadedProject, Message, ProjectId, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_types::format::SnxSchematic;
use signex_types::project::{ProjectData, SheetEntry};
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use crate::board_sketch::{Fixture, in_sketch, live, open_board, send};
use crate::board_sketch_persist::{
    notice_says_held, sketch_a_refused_outline, sketch_an_outline, undock,
};

/// Open the project at `file` in the workspace: its root sheet, its listed
/// sheets and the board it names.
fn add_project(
    app: &mut Signex,
    file: PathBuf,
    root: Option<&str>,
    sheets: &[&str],
    pcb: Option<&str>,
) -> ProjectId {
    let id = app.document_state.mint_project_id();
    let dir = file.parent().unwrap().to_string_lossy().into_owned();
    let name = file.file_stem().unwrap().to_string_lossy().into_owned();
    app.document_state.projects.push(LoadedProject {
        id,
        path: file,
        data: ProjectData {
            document_id: None,
            description: None,
            name,
            dir,
            schematic_root: root.map(str::to_string),
            pcb_file: pcb.map(str::to_string),
            sheets: sheets
                .iter()
                .map(|sheet| SheetEntry {
                    name: (*sheet).to_string(),
                    filename: (*sheet).to_string(),
                    symbols_count: 0,
                    wires_count: 0,
                    labels_count: 0,
                    unknown: Default::default(),
                })
                .collect(),
            variant_definitions: Vec::new(),
            active_variant: None,
            variant_settings: Default::default(),
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
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
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    id
}

fn board_dir(f: &Fixture) -> PathBuf {
    f.path.parent().unwrap().to_path_buf()
}

/// The open board, named by the active project `P` beside it.
fn board_of_project() -> Fixture {
    let mut f = open_board();
    let prj = board_dir(&f).join("P.snxprj");
    let id = add_project(&mut f.app, prj, None, &[], Some("board.snxpcb"));
    f.app.document_state.active_project = Some(id);
    f
}

#[test]
fn close_project_holds_on_a_refused_bake_of_its_board() {
    let mut f = board_of_project();
    sketch_a_refused_outline(&mut f);
    send(&mut f, Message::Menu(MenuMessage::CloseProject));
    assert_eq!(f.app.document_state.projects.len(), 1, "the project stays");
    assert!(f.app.ui_state.project_close_confirm.is_none(), "no prompt");
    assert!(notice_says_held(&f), "the close says why it waits");
    assert!(in_sketch(&f), "the mode and its copy stay");
}

#[test]
fn close_project_applies_the_sketch_of_its_board_first() {
    let mut f = board_of_project();
    sketch_an_outline(&mut f);
    assert!(!f.app.document_state.is_document_dirty(&f.path));
    send(&mut f, Message::Menu(MenuMessage::CloseProject));
    assert!(
        f.app.document_state.error_notice.is_none(),
        "nothing is held"
    );
    assert_eq!(live(&f).outline.len(), 4, "the sketch reached the board");
    assert!(
        f.app.document_state.is_document_dirty(&f.path),
        "the applied sketch is the board's unsaved edit"
    );
}

/// A sketch of a board the closing project does not name is not its work:
/// a refused bake there does not hold the close.
#[test]
fn close_project_ignores_the_sketch_of_another_board() {
    let mut f = open_board();
    let other = board_dir(&f).join("other");
    std::fs::create_dir_all(&other).unwrap();
    let id = add_project(&mut f.app, other.join("Q.snxprj"), None, &[], None);
    f.app.document_state.active_project = Some(id);
    sketch_a_refused_outline(&mut f);
    send(&mut f, Message::Menu(MenuMessage::CloseProject));
    assert!(
        f.app.document_state.projects.is_empty(),
        "the project closes"
    );
    assert!(f.app.document_state.error_notice.is_none(), "nothing held");
    assert!(in_sketch(&f), "the board's sketch is untouched");
}

fn write_sheet(path: &Path) {
    let sheet = SchematicSheet {
        uuid: Uuid::from_u128(0x2331_0001),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    std::fs::write(path, SnxSchematic::new(sheet).write_string().unwrap()).unwrap();
}

/// Project B's board in its own window while the main window shows a sheet
/// of project A: the window needs B's hierarchy, so one is asked for.
#[test]
fn an_undocked_board_asks_for_its_projects_hierarchy() {
    let mut f = open_board();
    let dir = board_dir(&f);
    let a_dir = dir.join("a");
    std::fs::create_dir_all(&a_dir).unwrap();
    let a_sheet = a_dir.join("a.snxsch");
    write_sheet(&a_sheet);
    add_project(
        &mut f.app,
        dir.join("B.snxprj"),
        Some("b.snxsch"),
        &[],
        Some("board.snxpcb"),
    );
    let a = add_project(
        &mut f.app,
        a_dir.join("A.snxprj"),
        Some("a.snxsch"),
        &["a.snxsch"],
        None,
    );
    f.app.document_state.active_project = Some(a);
    undock(&mut f).expect("the board undocks");
    send(
        &mut f,
        Message::File(FileMsg::Opened(Some(a_sheet.clone()))),
    );
    let shown = f.app.document_state.active_path.clone();
    assert_eq!(shown.as_deref(), Some(a_sheet.as_path()), "main shows A");

    let hierarchies = &f.app.document_state.window_hierarchies;
    assert!(
        hierarchies.is_exhausted(&dir.join("b.snxsch")).is_some(),
        "the board's window asks for project B"
    );
    assert!(
        hierarchies.is_exhausted(&a_sheet).is_none(),
        "A is the Navigator's, not the window's"
    );
}
