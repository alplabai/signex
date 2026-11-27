//! #1821 — renaming a sheet or adding a new one changes a file on disk at
//! once, so the saved `.snxprj` must name it too. Discard All, a quit without
//! saving or a crash must never leave the project pointing at a missing file
//! or leave a sheet file the project does not list, and the commit must not
//! write other unsaved project edits the user may want to discard.

use signex_app::app::{
    LoadedProject, Message, ProjectId, ProjectMsg, RenameDialogState, RenameMsg, Signex,
};
use signex_i18n::RenderTranslatable;
use signex_types::project::{ProjectData, SheetEntry, parse_project, write_project};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Fixture {
    app: Signex,
    _tmp: TempDir,
    dir: PathBuf,
    prj: PathBuf,
    project: ProjectId,
}

fn sheet_entry(filename: &str) -> SheetEntry {
    SheetEntry {
        name: filename.to_string(),
        filename: filename.to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }
}

fn project_data(dir: &Path) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "Proj".to_string(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: Some("Second.snxsch".to_string()),
        pcb_file: None,
        sheets: vec![sheet_entry("Second.snxsch")],
        variant_definitions: Vec::new(),
        active_variant: None,
        variant_settings: Default::default(),
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
        bom_alternates: Vec::new(),
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A saved project listing `Second.snxsch` (which exists on disk) and loaded
/// in the app, clean. `on_disk: false` leaves no `.snxprj` behind (a project
/// never saved).
fn fixture(on_disk: bool) -> Fixture {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let dir = tmp.path().to_path_buf();
    let prj = dir.join("Proj.snxprj");
    std::fs::write(dir.join("Second.snxsch"), b"x").expect("write sheet");
    let data = project_data(&dir);
    if on_disk {
        write_project(&prj, &data).expect("write project");
    }
    let (mut app, _task) = Signex::new();
    let project = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id: project,
        path: prj.clone(),
        data,
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    app.document_state.active_project = Some(project);
    Fixture {
        app,
        _tmp: tmp,
        dir,
        prj,
        project,
    }
}

fn run(task: iced::Task<Message>) -> Vec<Message> {
    use futures::StreamExt as _;
    let Some(mut stream) = iced_runtime::task::into_stream(task) else {
        return Vec::new();
    };
    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let mut out = Vec::new();
        while let Some(action) = stream.next().await {
            if let iced_runtime::Action::Output(message) = action {
                out.push(message);
            }
        }
        out
    })
}

fn complete(app: &mut Signex, task: iced::Task<Message>) {
    for message in run(task) {
        let _follow_up = app.update(message);
    }
}

fn rename(app: &mut Signex, target: &Path, buffer: &str) {
    app.ui_state.rename_dialog = Some(Box::new(RenameDialogState {
        project_id: None,
        target_path: target.to_path_buf(),
        tree_path: vec![0],
        buffer: buffer.to_string(),
        error: None,
        is_project_rename: false,
        placing_symbols: Vec::new(),
        name_clash: None,
    }));
    let task = app.update(Message::Rename(RenameMsg::Submit));
    complete(app, task);
}

fn add_new(f: &mut Fixture, name: &str) -> PathBuf {
    let path = f.dir.join(name);
    let task = f
        .app
        .update(Message::Project(ProjectMsg::AddNewSchematicPicked {
            project_id: f.project,
            path: Some(path.clone()),
            choice: None,
        }));
    complete(&mut f.app, task);
    path
}

/// How many entries of the process-wide diagnostics ring carry `fragment`:
/// compare before and after, so another test's line cannot satisfy this one.
fn logged_count(fragment: &str) -> usize {
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(fragment))
        .count()
}

/// What Discard All then reopen shows: the project as saved on disk.
fn reopened(f: &Fixture) -> ProjectData {
    parse_project(&f.prj).expect("the saved project parses")
}

fn names(data: &ProjectData) -> Vec<String> {
    data.sheets.iter().map(|s| s.filename.clone()).collect()
}

#[test]
fn rename_then_discard_all_reopens_with_the_renamed_sheet() {
    let said =
        "Renamed Second.snxsch to ThirdA.snxsch; the saved project file now lists the new name";
    let before = logged_count(said);
    let mut f = fixture(true);
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "ThirdA.snxsch");

    assert!(f.dir.join("ThirdA.snxsch").exists());
    let saved = reopened(&f);
    assert_eq!(names(&saved), ["ThirdA.snxsch"], "no (missing) sheet");
    assert_eq!(saved.schematic_root.as_deref(), Some("ThirdA.snxsch"));
    assert!(
        !f.app.document_state.is_document_dirty(&f.prj),
        "the project was committed with the move, so nothing is left unsaved"
    );
    assert_eq!(logged_count(said), before + 1);
}

#[test]
fn rename_does_not_write_other_unsaved_project_edits() {
    let mut f = fixture(true);
    f.app.document_state.projects[0].data.description = Some("unsaved note".to_string());
    f.app.document_state.mark_document_dirty(&f.prj);

    rename(&mut f.app, &f.dir.join("Second.snxsch"), "Third.snxsch");

    let saved = reopened(&f);
    assert_eq!(names(&saved), ["Third.snxsch"]);
    assert_eq!(
        saved.description, None,
        "the unsaved description stays unsaved"
    );
    assert_eq!(
        f.app.document_state.projects[0].data.description.as_deref(),
        Some("unsaved note"),
        "the edit is still held in memory"
    );
    assert!(
        f.app.document_state.is_document_dirty(&f.prj),
        "the project still has the unsaved edit"
    );
}

#[test]
fn rename_in_a_project_never_saved_leaves_it_dirty_and_says_so() {
    let said = "renamed to ThirdB.snxsch); the project has not been saved yet";
    let before = logged_count(said);
    let mut f = fixture(false);
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "ThirdB.snxsch");

    assert!(!f.prj.exists(), "no project file is created by a rename");
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["ThirdB.snxsch"]
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(logged_count(said), before + 1);
}

/// A damaged `.snxprj` is never overwritten and never makes the rename
/// impossible: the move stands and the in-memory project is left dirty.
#[test]
fn rename_with_an_unreadable_saved_project_moves_the_file_and_leaves_it_dirty() {
    let said = "renamed to ThirdC.snxsch); the saved project file could not be read";
    let before = logged_count(said);
    let mut f = fixture(true);
    std::fs::write(&f.prj, b"{ broken").expect("damage the project");
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "ThirdC.snxsch");

    assert!(f.dir.join("ThirdC.snxsch").exists());
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        b"{ broken",
        "not overwritten"
    );
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["ThirdC.snxsch"]
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(logged_count(said), before + 1);
}

#[cfg(unix)]
fn read_only(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o444)).expect("chmod");
}

/// The saved project cannot be written: the move is undone.
#[cfg(unix)]
#[test]
fn rename_is_undone_when_the_saved_project_cannot_be_written() {
    let mut f = fixture(true);
    read_only(&f.prj);
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "Third.snxsch");

    assert!(f.dir.join("Second.snxsch").exists(), "the file moved back");
    assert!(!f.dir.join("Third.snxsch").exists());
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Second.snxsch"],
        "memory still names the old file"
    );
}

/// Rewrite the saved project with the format token `snxprj/1.2`, a newer
/// minor than this build writes, and load it into the app as an open would.
/// Returns the bytes on disk.
fn make_newer_minor(f: &mut Fixture) -> Vec<u8> {
    let mut body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&f.prj).unwrap()).expect("project JSON");
    body["format"] = serde_json::json!("snxprj/1.2");
    let bytes = serde_json::to_vec_pretty(&body).expect("encode");
    std::fs::write(&f.prj, &bytes).expect("write fixture");
    let data = parse_project(&f.prj).expect("a newer minor opens");
    assert!(data.is_from_newer_minor());
    f.app.document_state.projects[0].data = data;
    bytes
}

/// What the user is told about a failed rename: the dialog's error while it
/// is open for the file, the card otherwise.
fn rename_failure(app: &Signex) -> String {
    app.ui_state
        .rename_dialog
        .as_ref()
        .and_then(|d| d.error.clone())
        .or_else(|| {
            app.document_view
                .error_notice
                .as_ref()
                .map(|n| n.detail.render_en_us())
        })
        .unwrap_or_default()
}

/// A project read from a newer minor cannot be written, so a sheet rename in
/// it is refused before the file moves (#2904): the file stays, the saved
/// project keeps its bytes and the project in memory still names the old
/// file.
#[test]
fn rename_in_a_newer_minor_project_is_undone() {
    let mut f = fixture(true);
    let saved = make_newer_minor(&mut f);
    rename(&mut f.app, &f.dir.join("Second.snxsch"), "ThirdNM.snxsch");

    assert!(f.dir.join("Second.snxsch").exists(), "the file moved back");
    assert!(!f.dir.join("ThirdNM.snxsch").exists());
    assert_eq!(std::fs::read(&f.prj).unwrap(), saved, "byte-identical");
    let memory = &f.app.document_state.projects[0].data;
    assert_eq!(names(memory), ["Second.snxsch"]);
    assert_eq!(memory.schematic_root.as_deref(), Some("Second.snxsch"));
    let told = rename_failure(&f.app);
    assert!(
        told.contains("Nothing was changed") && told.contains("snxprj/1.2"),
        "{told}"
    );
    // Refused up front, not moved and moved back.
    assert!(told.contains("Not renamed"), "{told}");
}

/// Add New Schematic in a project read from a newer minor is refused before
/// the blank sheet is written (#2904): no sheet, the saved project keeps its
/// bytes, the project in memory lists no new sheet, and a card says why.
#[test]
fn add_new_schematic_in_a_newer_minor_project_leaves_no_sheet() {
    let kept = "the project file could not be written, so AddedNM.snxsch was not kept";
    let said = "Add New Schematic refused: AddedNM.snxsch was not created";
    let (kept_before, before) = (logged_count(kept), logged_count(said));
    let mut f = fixture(true);
    let saved = make_newer_minor(&mut f);
    let path = add_new(&mut f, "AddedNM.snxsch");

    assert!(!path.exists(), "no sheet file is left unlisted");
    assert_eq!(std::fs::read(&f.prj).unwrap(), saved, "byte-identical");
    let memory = &f.app.document_state.projects[0].data;
    assert_eq!(names(memory), ["Second.snxsch"]);
    assert_eq!(memory.schematic_root.as_deref(), Some("Second.snxsch"));
    assert_eq!(logged_count(said), before + 1);
    assert_eq!(logged_count(kept), kept_before, "no sheet was written");
    let notice = f.app.document_view.error_notice.as_ref().expect("card");
    assert!(
        notice.detail.render_en_us().contains("was not created"),
        "{notice:?}"
    );
}

#[test]
fn add_new_schematic_then_discard_all_reopens_with_the_sheet() {
    let said = "Added AddedA.snxsch; the saved project file now lists it";
    let before = logged_count(said);
    let mut f = fixture(true);
    let path = add_new(&mut f, "AddedA.snxsch");

    assert!(path.exists());
    assert_eq!(names(&reopened(&f)), ["Second.snxsch", "AddedA.snxsch"]);
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(logged_count(said), before + 1);
}

#[test]
fn add_new_schematic_does_not_write_other_unsaved_project_edits() {
    let mut f = fixture(true);
    f.app.document_state.projects[0].data.description = Some("unsaved note".to_string());
    f.app.document_state.mark_document_dirty(&f.prj);
    add_new(&mut f, "Added.snxsch");

    let saved = reopened(&f);
    assert_eq!(names(&saved), ["Second.snxsch", "Added.snxsch"]);
    assert_eq!(saved.description, None);
    assert!(f.app.document_state.is_document_dirty(&f.prj));
}

#[cfg(unix)]
#[test]
fn add_new_schematic_leaves_no_orphan_when_the_project_cannot_be_written() {
    let said = "the project file could not be written, so AddedB.snxsch was not kept";
    let before = logged_count(said);
    let mut f = fixture(true);
    read_only(&f.prj);
    let path = add_new(&mut f, "AddedB.snxsch");

    assert!(!path.exists(), "no sheet file is left unlisted");
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Second.snxsch"]
    );
    assert_eq!(logged_count(said), before + 1);
}

#[test]
fn add_new_schematic_with_an_unreadable_saved_project_keeps_the_sheet_and_stays_dirty() {
    let said = "list the new sheet AddedC.snxsch; the saved project file could not be read";
    let before = logged_count(said);
    let mut f = fixture(true);
    std::fs::write(&f.prj, b"{ broken").expect("damage the project");
    let path = add_new(&mut f, "AddedC.snxsch");

    assert!(path.exists());
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        b"{ broken",
        "not overwritten"
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(logged_count(said), before + 1);
}

#[test]
fn a_rename_whose_move_back_also_fails_tells_the_user_where_the_file_is() {
    use signex_app::app::path_ops::RenameIoError;

    let mut f = fixture(true);
    let from = f.dir.join("Second.snxsch");
    f.app.ui_state.rename_dialog = Some(Box::new(RenameDialogState {
        project_id: None,
        target_path: from.clone(),
        tree_path: vec![0],
        buffer: "Third.snxsch".to_string(),
        error: None,
        is_project_rename: false,
        placing_symbols: Vec::new(),
        name_clash: None,
    }));
    let task = f.app.update(Message::Rename(RenameMsg::Submit));
    let mut messages = run(task);
    // The rename's completion, wherever it lands among the update's tasks
    // (the project in view also starts its notebook read).
    let done = messages
        .iter_mut()
        .find_map(|m| match m {
            Message::Rename(RenameMsg::FileRenamed(done)) => Some(done),
            _ => None,
        })
        .expect("the rename completes");
    done.result = Err(RenameIoError::ProjectNotPatched {
        reason: "the project file could not be updated".to_string(),
        moved_back: Err("denied".to_string()),
    });
    f.app.ui_state.rename_dialog = None;
    for message in messages {
        let _ = f.app.update(message);
    }
    let notice = f
        .app
        .document_view
        .error_notice
        .as_ref()
        .expect("the stranded file raises a card");
    assert!(
        notice
            .detail
            .render_en_us()
            .contains("could not be moved back (denied)")
    );
    assert!(
        notice
            .detail
            .render_en_us()
            .contains("Move it back by hand")
    );
    assert!(notice.detail.render_en_us().contains("Third.snxsch"));
    assert!(!notice.detail.render_en_us().contains("Nothing was changed"));
}

/// The names in `dir`, sorted: what a refused change must leave as it was.
fn listing(dir: &Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("list")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    names.sort();
    names
}

/// Add Existing in a project read from a newer minor is refused before
/// anything is copied (#2904): the project folder and the picked file are as
/// they were, the saved project keeps its bytes, the project in memory lists
/// nothing new and stays clean, and a card says why.
#[test]
fn add_existing_in_a_newer_minor_project_copies_nothing() {
    let said = "Add Existing refused: OutsideNM.snxsch not added";
    let before = logged_count(said);
    let mut f = fixture(true);
    let saved = make_newer_minor(&mut f);
    let elsewhere = TempDir::new().expect("tempdir");
    let picked = elsewhere.path().join("OutsideNM.snxsch");
    std::fs::write(&picked, b"outside").expect("write picked file");
    let listed = listing(&f.dir);

    let task = f
        .app
        .update(Message::Project(ProjectMsg::AddExistingFilePicked {
            project_id: f.project,
            paths: Some(vec![picked.clone()]),
        }));
    complete(&mut f.app, task);

    assert_eq!(std::fs::read(&f.prj).unwrap(), saved, "byte-identical");
    assert_eq!(listing(&f.dir), listed, "nothing was copied in");
    assert_eq!(std::fs::read(&picked).unwrap(), b"outside");
    assert_eq!(
        names(&f.app.document_state.projects[0].data),
        ["Second.snxsch"]
    );
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(logged_count(said), before + 1);
    let notice = f.app.document_view.error_notice.as_ref().expect("card");
    let detail = notice.detail.render_en_us();
    assert!(
        detail.contains("OutsideNM.snxsch")
            && detail.contains("was not added")
            && detail.contains("Nothing was copied"),
        "{detail}"
    );
}

/// Remove from Project and Delete in a project read from a newer minor are
/// refused before anything is excluded, moved or deleted (#2904): the sheet
/// and the folder are as they were, the saved project keeps its bytes, the
/// project in memory still lists the sheet, and a card says why.
#[test]
fn remove_and_delete_in_a_newer_minor_project_leave_every_file() {
    use signex_app::app::{RemoveChoice, RemoveDialogState, RemoveMsg};
    for (choice, action) in [
        (RemoveChoice::ExcludeFromProject, "Remove from Project"),
        (RemoveChoice::DeleteFile, "Delete"),
    ] {
        let said = format!("{action} refused: Second.snxsch was not removed");
        let before = logged_count(&said);
        let mut f = fixture(true);
        let saved = make_newer_minor(&mut f);
        let sheet = f.dir.join("Second.snxsch");
        let listed = listing(&f.dir);
        f.app.ui_state.remove_dialog = Some(RemoveDialogState {
            target_path: sheet.clone(),
            tree_path: vec![0, 0],
            project_id: Some(f.project),
            display_name: "Second.snxsch".to_string(),
            is_outside_project: false,
            unsaved_prompt: false,
            is_directory_package: false,
            choice: None,
        });

        let task = f.app.update(Message::Remove(RemoveMsg::Confirm(choice)));
        complete(&mut f.app, task);

        assert_eq!(
            std::fs::read(&f.prj).unwrap(),
            saved,
            "{action}: byte-identical"
        );
        assert_eq!(std::fs::read(&sheet).unwrap(), b"x", "{action}: sheet kept");
        assert_eq!(listing(&f.dir), listed, "{action}: nothing moved");
        assert_eq!(
            names(&f.app.document_state.projects[0].data),
            ["Second.snxsch"]
        );
        assert!(f.app.ui_state.remove_dialog.is_none());
        assert_eq!(logged_count(&said), before + 1, "{action}");
        let notice = f.app.document_view.error_notice.as_ref().expect("card");
        let detail = notice.detail.render_en_us();
        assert!(
            detail.contains("was not removed") && detail.contains("Nothing was moved or deleted"),
            "{action}: {detail}"
        );
    }
}
