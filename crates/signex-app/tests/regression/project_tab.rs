//! #1202 — the Project document tab (General section) replaces the read-only
//! Project Options modal. Each test drives `Signex::update` the way the UI
//! does; the off-thread `.snxprj` write is run by hand (`settle_task`).

use signex_app::app::{
    ContextMenuMsg, EditMsg, FileMsg, LoadedProject, Message, ProjectCloseChoice,
    ProjectGeneralFacts, ProjectMsg, ProjectTabMsg, ProjectTreeAction, Signex, TabContextAction,
    WindowMsg,
};
use signex_app::dock::DockMessage;
use signex_app::panels::{NetClassMsg, PanelMsg};
use signex_types::project::{ProjectData, parse_project, write_project};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    app: Signex,
    prj: PathBuf,
    _tmp: TempDir,
}

/// A saved project `Oscar.snxprj` (root sheet + PCB named, one library),
/// loaded the way project open leaves it.
fn fixture() -> Fixture {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let prj = dir.join("Oscar.snxprj");
    let data = ProjectData {
        name: "Oscar".to_string(),
        description: None,
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("Oscar.snxsch".to_string()),
        pcb_file: Some("Oscar.snxpcb".to_string()),
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: vec![signex_types::project::LibraryEntry {
            path: PathBuf::from("parts.snxlib"),
            kind: signex_types::project::LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        }],
        enable_git: false,
        net_classes: Vec::new(),
        rules: None,
        unknown: Default::default(),
    };
    write_project(&prj, &data).unwrap();
    let (mut app, _task) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: prj.clone(),
        data: parse_project(&prj).unwrap(),
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
    });
    app.document_state.active_project = Some(id);
    Fixture {
        app,
        prj,
        _tmp: tmp,
    }
}

fn open_tab(app: &mut Signex) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        ProjectTreeAction::OpenProjectOptions(vec![0]),
    )));
}

fn project_id(app: &Signex) -> signex_app::app::ProjectId {
    app.document_state.projects[0].id
}

fn edit_name(app: &mut Signex, text: &str) {
    let project_id = project_id(app);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::NameEdited {
            project_id,
            text: text.to_string(),
        },
    )));
}

fn edit_description(app: &mut Signex, text: &str) {
    let project_id = project_id(app);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::DescriptionEdited {
            project_id,
            text: text.to_string(),
        },
    )));
}

fn add_net_class(app: &mut Signex) {
    let _ = app.update(Message::Dock(DockMessage::Panel(PanelMsg::NetClass(
        NetClassMsg::Add,
    ))));
}

fn save(app: &mut Signex) {
    crate::settle_support::settle(app, Message::File(FileMsg::Save));
}

fn on_disk(path: &Path) -> ProjectData {
    parse_project(path).unwrap()
}

fn is_dirty(f: &Fixture) -> bool {
    let path = &f.app.document_state.projects[0].path;
    f.app.document_state.dirty_paths.contains(path)
}

#[test]
fn project_options_opens_the_project_tab_and_focuses_it_again() {
    let mut f = fixture();
    let id = project_id(&f.app);
    open_tab(&mut f.app);

    let tabs = &f.app.document_state.tabs;
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].kind.as_project(), Some(id));
    assert_eq!(tabs[0].path, f.prj);
    assert_eq!(tabs[0].project_id, Some(id));
    assert_eq!(f.app.document_state.active_tab, 0);

    // A second Project Options focuses the same tab instead of adding one.
    open_tab(&mut f.app);
    assert_eq!(f.app.document_state.tabs.len(), 1);
}

#[test]
fn general_facts_show_the_project_files() {
    let f = fixture();
    let facts = ProjectGeneralFacts::of(&f.app.document_state.projects[0]);
    assert_eq!(
        facts.directory,
        f.prj.parent().unwrap().display().to_string()
    );
    assert_eq!(facts.schematic_root.as_deref(), Some("Oscar.snxsch"));
    assert_eq!(facts.pcb_file.as_deref(), Some("Oscar.snxpcb"));
    assert_eq!(facts.library_count, 1);
}

#[test]
fn editing_the_name_marks_the_project_dirty_and_undo_redo_restore_it() {
    let mut f = fixture();
    open_tab(&mut f.app);
    assert!(!is_dirty(&f));

    edit_name(&mut f.app, "Osc");
    edit_name(&mut f.app, "Oscillator");
    assert_eq!(f.app.document_state.projects[0].data.name, "Oscillator");
    assert!(is_dirty(&f), "a name edit marks the .snxprj dirty");

    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        f.app.document_state.projects[0].data.name, "Oscar",
        "one undo takes back the whole typing run"
    );
    let _ = f.app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(f.app.document_state.projects[0].data.name, "Oscillator");
}

#[test]
fn ctrl_s_writes_the_description_off_thread_and_clears_dirty() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "Motor controller");

    let task = f.app.update(Message::File(FileMsg::Save));
    assert_eq!(
        on_disk(&f.prj).description,
        None,
        "the write runs in the returned task, not inside update"
    );
    assert!(is_dirty(&f), "nothing is clean before the write lands");
    crate::settle_support::settle_task(&mut f.app, task);

    assert_eq!(
        on_disk(&f.prj).description.as_deref(),
        Some("Motor controller")
    );
    assert!(!is_dirty(&f), "the landed write clears the marker");
}

#[test]
fn saving_an_edited_name_renames_the_project_file() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_name(&mut f.app, "Oscillator");
    save(&mut f.app);

    let renamed = f.prj.with_file_name("Oscillator.snxprj");
    assert!(renamed.exists(), "the name is the file stem");
    assert!(!f.prj.exists());
    assert_eq!(on_disk(&renamed).name, "Oscillator");
    let project = &f.app.document_state.projects[0];
    assert_eq!(project.path, renamed);
    let tab = &f.app.document_state.tabs[0];
    assert_eq!(tab.path, renamed, "the tab follows the file");
    assert!(!f.app.document_state.dirty_paths.contains(&renamed));
}

#[test]
fn an_invalid_name_refuses_the_save_and_keeps_the_project_dirty() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_name(&mut f.app, "a/b");
    save(&mut f.app);

    assert!(f.prj.exists(), "nothing was renamed");
    assert_eq!(on_disk(&f.prj).name, "Oscar");
    assert!(is_dirty(&f));
    assert!(
        f.app.document_state.error_notice.is_some(),
        "the refusal is reported, not swallowed"
    );
}

#[test]
fn closing_the_project_with_tab_edits_prompts() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "unsaved");

    crate::close_support::close_first_project(&mut f.app);

    let prompt = f
        .app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("unsaved Project tab edits must prompt on close");
    assert!(prompt.dirty_paths.contains(&f.prj));
}

#[test]
fn closing_the_tab_parks_the_edits_and_reopening_keeps_undo() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "parked");
    let _ = f.app.update(Message::ContextMenu(ContextMenuMsg::TabAction(
        TabContextAction::Close(0),
    )));
    assert!(f.app.document_state.tabs.is_empty());
    assert!(is_dirty(&f), "a closed dirty tab keeps its marker");

    open_tab(&mut f.app);
    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(f.app.document_state.projects[0].data.description, None);
}

#[test]
fn quit_save_all_writes_the_project_tab_edits() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "saved at quit");
    let _ = f.app.update(Message::Window(WindowMsg::CloseMainWindow));
    let modal = f
        .app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("quit with Project tab edits prompts");
    assert!(modal.dirty_paths.contains(&f.prj));

    crate::settle_support::settle(
        &mut f.app,
        Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
    );
    assert_eq!(
        on_disk(&f.prj).description.as_deref(),
        Some("saved at quit")
    );
    assert!(!is_dirty(&f));
}

#[test]
fn another_project_writer_while_the_tab_is_dirty_keeps_both_changes() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "mine");
    add_net_class(&mut f.app);

    // Undo in the Project tab takes back only its own field.
    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    let data = &f.app.document_state.projects[0].data;
    assert_eq!(data.description, None);
    assert_eq!(data.net_classes.len(), 1, "the other writer's change stays");
    let _ = f.app.update(Message::Edit(EditMsg::Redo));

    save(&mut f.app);
    let disk = on_disk(&f.prj);
    assert_eq!(disk.description.as_deref(), Some("mine"));
    assert_eq!(disk.net_classes.len(), 1);
    assert!(!is_dirty(&f));
}

#[test]
fn a_writer_during_an_in_flight_save_keeps_the_project_dirty() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "first");
    let task = f.app.update(Message::File(FileMsg::Save));
    // Another writer lands while the snapshot is being written.
    add_net_class(&mut f.app);
    crate::settle_support::settle_task(&mut f.app, task);

    assert_eq!(on_disk(&f.prj).description.as_deref(), Some("first"));
    assert!(
        is_dirty(&f),
        "the written snapshot lacks the net class: the project must stay dirty"
    );
    save(&mut f.app);
    let disk = on_disk(&f.prj);
    assert_eq!(disk.description.as_deref(), Some("first"));
    assert_eq!(disk.net_classes.len(), 1);
    assert!(!is_dirty(&f));
}

#[test]
fn the_project_tab_undocks_like_other_tabs() {
    let mut f = fixture();
    open_tab(&mut f.app);
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(0)));
    // `WindowKind` is crate-private; its Debug form names the kind and path.
    let windows: Vec<String> = f
        .app
        .ui_state
        .windows
        .values()
        .map(|kind| format!("{kind:?}"))
        .collect();
    assert_eq!(windows.len(), 1, "the Project tab opens in its own window");
    assert!(windows[0].contains("UndockedTab"), "{windows:?}");
    assert!(windows[0].contains("Oscar.snxprj"), "{windows:?}");
}
