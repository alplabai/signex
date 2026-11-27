//! #2350 — a board made by New PCB in an unsaved project is listed in the
//! Projects tree as an unsaved document, not as `(missing)`, until Save
//! writes it.

use std::path::{Path, PathBuf};

use signex_app::app::{
    ContextMenuMsg, DocumentSetupMsg, LoadedProject, Message, ProjectTreeAction, Signex, UiMsg,
};
use signex_app::menu_bar::MenuMessage;
use signex_types::project::ProjectData;
use signex_widgets::tree_view::TreeNode;

fn send(app: &mut Signex, message: Message) {
    let task = app.update(message);
    crate::drive::drive(app, task);
}

/// An Untitled project held in memory only: neither the `.snxprj` nor any
/// document of it is on disk.
fn unsaved_project(dir: &Path) -> Signex {
    let _ = signex_app::diagnostics::init_logging();
    let (mut app, _boot) = Signex::new();
    app.ui_state.first_run_tour_open = false;
    app.ui_state.main_window_id = Some(iced::window::Id::unique());
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("Untitled.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "Untitled".to_string(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
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
            bom_alternates: Vec::new(),
            requirements: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(id);
    app
}

/// Projects ▸ Add New ▸ PCB on the project row.
fn add_new_pcb(app: &mut Signex) {
    // A grid pick refreshes the tree, so the project row exists.
    send(app, Message::Ui(UiMsg::GridPickerSelect(1.0)));
    let key = signex_widgets::tree_view::key_of(&app.document_view.panel_ctx.project_tree, &[0])
        .expect("the project row");
    send(
        app,
        Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
            ProjectTreeAction::AddNewPcb(key),
        )),
    );
    // Since #2232 Add New PCB opens the setup dialog; Create makes the board.
    assert!(app.ui_state.document_setup.is_some(), "the setup dialog");
    send(app, Message::from(DocumentSetupMsg::SubmitRequested));
}

/// The tree row keyed `key`, searched depth first.
fn tree_row<'a>(nodes: &'a [TreeNode], key: &str) -> Option<&'a TreeNode> {
    nodes.iter().find_map(|node| {
        (node.key == key)
            .then_some(node)
            .or_else(|| tree_row(&node.children, key))
    })
}

fn board_row(app: &Signex) -> TreeNode {
    tree_row(&app.document_view.panel_ctx.project_tree, "Untitled.snxpcb")
        .cloned()
        .expect("the board row")
}

#[test]
fn a_new_board_lists_as_unsaved_until_save_writes_it() {
    let tmp = tempfile::tempdir().unwrap();
    let board: PathBuf = tmp.path().join("Untitled.snxpcb");
    let mut app = unsaved_project(tmp.path());
    add_new_pcb(&mut app);

    assert!(!board.exists(), "nothing is written until Save");
    assert!(
        app.document_state.project_fs.facts.is_missing(&board),
        "the file-status check ran and found no file"
    );
    assert!(
        app.document_state.tabs.iter().any(|tab| tab.path == board),
        "the board opens in its tab"
    );
    let info = &app.document_view.panel_ctx.projects[0];
    assert_eq!(info.pcb_file.as_deref(), Some("Untitled.snxpcb"));
    assert!(!info.pcb_file_missing, "an unsaved board is not missing");
    assert!(info.pcb_file_dirty, "it is listed as unsaved");
    let row = board_row(&app);
    assert_eq!(row.label, "Untitled.snxpcb");
    assert!(row.is_dirty && row.is_open);

    send(&mut app, Message::Menu(MenuMessage::SaveAll));

    assert!(board.exists(), "Save wrote the board");
    assert!(!app.document_state.is_document_dirty(&board));
    let info = &app.document_view.panel_ctx.projects[0];
    assert!(!info.pcb_file_missing && !info.pcb_file_dirty);
    let row = board_row(&app);
    assert_eq!(row.label, "Untitled.snxpcb");
    assert!(!row.is_dirty);
}

/// A board the project lists that no document holds and the disk lacks is
/// still `(missing)`.
#[test]
fn a_listed_board_nothing_holds_is_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = unsaved_project(tmp.path());
    app.document_state.projects[0].data.pcb_file = Some("Untitled.snxpcb".to_string());
    send(&mut app, Message::Ui(UiMsg::GridPickerSelect(1.0)));

    assert!(app.document_view.panel_ctx.projects[0].pcb_file_missing);
    assert_eq!(board_row(&app).label, "Untitled.snxpcb  (missing)");
}
