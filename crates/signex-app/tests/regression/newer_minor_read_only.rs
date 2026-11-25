//! #104 — a schematic written by a newer minor format loads for viewing
//! but Save is refused: the document stays dirty, the file is untouched,
//! and the refusal reaches the Save Failed card and the Messages panel.

use signex_app::app::{FileMsg, Message, Signex, TabInfo, TabKind};
use signex_i18n::RenderTranslatable;
use signex_types::schematic::SchematicSheet;
use tempfile::TempDir;

#[test]
fn saving_a_newer_minor_schematic_is_refused_and_stays_dirty() {
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let path = tmp.path().join("Newer.snxsch");
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    };
    let text = signex_types::format::SnxSchematic::new(sheet)
        .write_string()
        .expect("serialise");
    let newer = text.replacen("format = \"snxsch/1.1\"", "format = \"snxsch/1.7\"", 1);
    assert_ne!(newer, text, "fixture must carry the newer token");
    std::fs::write(&path, &newer).expect("write fixture");

    let engine = signex_engine::Engine::open(&path).expect("a newer minor still loads");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "Newer".into(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path.clone());
    app.document_state.mark_document_dirty(&path);

    let _ = app.update(Message::File(FileMsg::Save));

    assert_eq!(std::fs::read_to_string(&path).unwrap(), newer, "untouched");
    assert!(app.document_state.is_document_dirty(&path));
    assert!(
        app.document_state
            .is_document_dirty(&app.document_state.tabs[0].path)
    );
    // The card carries the full cause: the operation as its sentence and
    // the error chain in its system block (mockup row 11). The Messages panel line is
    // trimmed to 160 bytes, so only its leading context is matched.
    let notice = app.document_state.error_notice.as_ref().expect("card");
    assert_eq!(
        notice.detail.render_en_us(),
        "Failed to save active document"
    );
    let system = notice.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    assert!(
        system.contains("newer Signex") && system.contains("snxsch/1.7"),
        "{notice:?}"
    );
    assert!(
        signex_app::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("Failed to save active document")),
        "refusal must reach the Messages panel"
    );
}

/// A project with nothing in it, as this build writes it.
fn bare_project() -> signex_types::project::ProjectData {
    signex_types::project::ProjectData {
        document_id: None,
        description: Some("as saved".to_string()),
        name: "Newer".to_string(),
        dir: String::new(),
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
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// Write `Newer.snxprj` into `dir` with the format token `snxprj/1.2`, a
/// newer minor than this build writes, and return the bytes on disk.
fn write_newer_minor_project(dir: &std::path::Path) -> (std::path::PathBuf, Vec<u8>) {
    let path = dir.join("Newer.snxprj");
    signex_types::project::write_project(&path, &bare_project()).expect("write project");
    let mut body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).expect("project JSON");
    body["format"] = serde_json::json!("snxprj/1.2");
    let bytes = serde_json::to_vec_pretty(&body).expect("encode");
    std::fs::write(&path, &bytes).expect("write fixture");
    (path, bytes)
}

fn file_names(dir: &std::path::Path) -> Vec<std::ffi::OsString> {
    let mut names: Vec<_> = std::fs::read_dir(dir)
        .expect("list")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    names.sort();
    names
}

/// A project read from a newer minor opens, but its Save is refused before
/// anything is written: the `.snxprj` keeps its bytes, the pending library
/// is not created, the project stays modified and the refusal is shown.
#[test]
fn saving_a_newer_minor_project_writes_nothing_and_creates_no_library() {
    use signex_app::library::messages::LibraryMessage;
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let (file, before) = write_newer_minor_project(tmp.path());
    let (mut app, _boot) = Signex::new();
    crate::settle_support::settle(&mut app, Message::File(FileMsg::Opened(Some(file.clone()))));
    let prj = app
        .document_state
        .projects
        .first()
        .expect("a newer minor project still opens")
        .path
        .clone();
    assert!(app.document_state.projects[0].data.is_from_newer_minor());

    app.document_state.projects[0].data.description = Some("typed".to_string());
    app.document_state.mark_document_dirty(&prj);
    let lib = tmp.path().join("NewerParts.snxlib");
    let _ = app.update(Message::Library(LibraryMessage::CreateLibraryAtPath {
        project_path: prj.clone(),
        lib_path: lib.clone(),
    }));
    let _ = app.update(Message::Library(
        LibraryMessage::LibraryCreateOptionsConfirm,
    ));
    assert_eq!(
        app.document_state.projects[0].pending_libraries.len(),
        1,
        "the library waits for the save"
    );
    let listed = file_names(tmp.path());
    app.document_state.error_notice = None;

    crate::settle_support::settle(&mut app, Message::File(FileMsg::Save));

    assert_eq!(std::fs::read(&file).unwrap(), before, "byte-identical");
    assert!(!lib.exists(), "no library is created");
    assert_eq!(file_names(tmp.path()), listed, "no file was created");
    assert_eq!(
        app.document_state.projects[0].pending_libraries.len(),
        1,
        "the library is still waiting"
    );
    assert!(app.document_state.is_document_dirty(&prj));
    let notice = app.document_state.error_notice.as_ref().expect("card");
    let system = notice.system.as_ref().map(|s| s.as_str()).unwrap_or("");
    assert!(
        system.contains("read-only") && system.contains("snxprj/1.2"),
        "{notice:?}"
    );
    // Its own sentence names the project and why (#2904).
    let detail = notice.detail.render_en_us();
    assert!(
        detail.contains("Newer") && detail.contains("was not saved"),
        "{detail}"
    );
}

/// A newer-minor project with unsaved settings beside an unsaved sheet it
/// lists, and both files' bytes on disk as they were.
struct ReadOnlyWithSheet {
    app: Signex,
    _tmp: TempDir,
    prj: std::path::PathBuf,
    prj_bytes: Vec<u8>,
    sheet: std::path::PathBuf,
    sheet_bytes: Vec<u8>,
}

fn blank_sheet() -> SchematicSheet {
    SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        ..Default::default()
    }
}

/// `Newer.snxprj`, written as `snxprj/1.2` and listing `one.snxsch`, loaded
/// as an open leaves it, then its description edited; `one.snxsch` is open
/// with an unsaved sheet that differs from the file.
fn read_only_project_with_dirty_sheet() -> ReadOnlyWithSheet {
    use signex_types::project::{SheetEntry, parse_project, write_project};
    let _ = signex_app::diagnostics::init_logging();
    let tmp = TempDir::new().expect("tempdir");
    let sheet = tmp.path().join("one.snxsch");
    let sheet_text = signex_types::format::SnxSchematic::new(blank_sheet())
        .write_string()
        .expect("serialise");
    std::fs::write(&sheet, &sheet_text).expect("write sheet");
    let prj = tmp.path().join("Newer.snxprj");
    let mut data = bare_project();
    data.schematic_root = Some("one.snxsch".to_string());
    data.sheets = vec![SheetEntry {
        name: "one.snxsch".to_string(),
        filename: "one.snxsch".to_string(),
        symbols_count: 0,
        wires_count: 0,
        labels_count: 0,
        unknown: Default::default(),
    }];
    write_project(&prj, &data).expect("write project");
    let mut body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&prj).unwrap()).expect("project JSON");
    body["format"] = serde_json::json!("snxprj/1.2");
    let prj_bytes = serde_json::to_vec_pretty(&body).expect("encode");
    std::fs::write(&prj, &prj_bytes).expect("write fixture");
    let loaded = parse_project(&prj).expect("a newer minor opens");
    assert!(loaded.is_from_newer_minor());

    let (mut app, _boot) = Signex::new();
    let id = app.document_state.mint_project_id();
    app.document_state
        .projects
        .push(signex_app::app::LoadedProject {
            id,
            path: prj.clone(),
            data: loaded,
            pending_libraries: Default::default(),
            case_fold: Default::default(),
            case_probed: false,
            case_decided: false,
        });
    app.document_state.active_project = Some(id);
    app.document_state.projects[0].data.description = Some("typed".to_string());
    app.document_state.mark_document_dirty(&prj);

    let mut engine = signex_engine::Engine::new(blank_sheet()).expect("engine");
    engine.set_path(Some(sheet.clone()));
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.tabs.push(TabInfo {
        title: "one".into(),
        path: sheet.clone(),
        cached_document: None,
        project_id: Some(id),
        kind: TabKind::Schematic,
    });
    app.document_state.mark_document_dirty(&sheet);
    ReadOnlyWithSheet {
        app,
        _tmp: tmp,
        prj,
        prj_bytes,
        sheet,
        sheet_bytes: sheet_text.into_bytes(),
    }
}

fn logged(fragment: &str) -> bool {
    signex_app::diagnostics::recent_entries()
        .iter()
        .any(|e| e.message.contains(fragment))
}

/// Exit with a newer-minor project and a sheet both unsaved (#2904, Caner
/// 2026-11-25): Discard All is not offered and a forced one drops nothing;
/// Save All saves the sheet and never writes the `.snxprj`, which keeps its
/// bytes and its unsaved settings; the prompt then asks about the project
/// alone, and its discard drops only that project's settings.
#[test]
fn exit_save_all_saves_the_sheet_and_never_writes_a_newer_minor_project() {
    use signex_app::app::{ProjectCloseChoice, ProjectMsg, WindowMsg};
    let mut f = read_only_project_with_dirty_sheet();
    let choose = |choice| Message::Project(ProjectMsg::AppQuitConfirm(choice));
    let _ = f.app.update(Message::Window(WindowMsg::CloseMainWindow));
    let prompt = f.app.ui_state.app_quit_confirm.as_ref().expect("prompt");
    assert!(
        prompt.dirty_paths.contains(&f.prj) && prompt.dirty_paths.contains(&f.sheet),
        "{:?}",
        prompt.dirty_paths
    );

    let _ = f.app.update(choose(ProjectCloseChoice::DiscardAll));
    assert!(f.app.ui_state.app_quit_confirm.is_some(), "refused");
    assert!(f.app.document_state.is_document_dirty(&f.sheet));
    assert!(f.app.document_state.is_document_dirty(&f.prj));

    crate::settle_support::settle(&mut f.app, choose(ProjectCloseChoice::SaveAll));

    assert_ne!(
        std::fs::read(&f.sheet).unwrap(),
        f.sheet_bytes,
        "the sheet is saved"
    );
    assert!(!f.app.document_state.is_document_dirty(&f.sheet));
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        f.prj_bytes,
        "byte-identical"
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert!(logged("Save All skips 'Newer'"), "the skip is noted");
    let again = f
        .app
        .ui_state
        .app_quit_confirm
        .as_ref()
        .expect("asked again");
    assert_eq!(again.dirty_paths, [f.prj.clone()]);

    let _ = f.app.update(choose(ProjectCloseChoice::DiscardReadOnly));
    assert!(!f.app.document_state.is_document_dirty(&f.prj));
    assert!(f.app.ui_state.app_quit_confirm.is_none(), "nothing is left");
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        f.prj_bytes,
        "byte-identical"
    );
}

/// Close Project the same way: Save All saves the sheet and leaves the
/// `.snxprj` byte for byte; the project stays open and the prompt asks about
/// its settings alone, and their discard closes it without writing them.
#[test]
fn close_project_save_all_saves_the_sheet_and_never_writes_a_newer_minor_project() {
    use signex_app::app::{ProjectCloseChoice, ProjectMsg};
    let mut f = read_only_project_with_dirty_sheet();
    let choose = |choice| Message::Project(ProjectMsg::CloseConfirm(choice));
    crate::close_support::close_first_project(&mut f.app);
    let prompt = f
        .app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("prompt");
    assert!(
        prompt.dirty_paths.contains(&f.prj) && prompt.dirty_paths.contains(&f.sheet),
        "{:?}",
        prompt.dirty_paths
    );

    let _ = f.app.update(choose(ProjectCloseChoice::DiscardAll));
    assert!(f.app.ui_state.project_close_confirm.is_some(), "refused");
    assert!(f.app.document_state.is_document_dirty(&f.sheet));

    crate::settle_support::settle(&mut f.app, choose(ProjectCloseChoice::SaveAll));

    assert_ne!(
        std::fs::read(&f.sheet).unwrap(),
        f.sheet_bytes,
        "the sheet is saved"
    );
    assert!(!f.app.document_state.is_document_dirty(&f.sheet));
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        f.prj_bytes,
        "byte-identical"
    );
    assert_eq!(f.app.document_state.projects.len(), 1, "still open");
    let again = f
        .app
        .ui_state
        .project_close_confirm
        .as_ref()
        .expect("asked again");
    assert_eq!(again.dirty_paths, [f.prj.clone()]);

    crate::settle_support::settle(&mut f.app, choose(ProjectCloseChoice::DiscardReadOnly));
    assert!(f.app.document_state.projects.is_empty(), "closed");
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        f.prj_bytes,
        "byte-identical"
    );
}

/// A forced Save All with only the newer-minor project unsaved writes
/// nothing, from either prompt, and leaves the prompt up.
#[test]
fn a_forced_save_all_never_writes_a_newer_minor_project() {
    use signex_app::app::{AppQuitConfirmState, ProjectCloseChoice, ProjectMsg};
    let mut f = read_only_project_with_dirty_sheet();
    f.app.document_state.clear_document_dirty(&f.sheet);
    f.app.ui_state.app_quit_confirm = Some(AppQuitConfirmState {
        dirty_paths: vec![f.prj.clone()],
        dirty_rows: Vec::new(),
    });
    crate::settle_support::settle(
        &mut f.app,
        Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
    );
    assert_eq!(
        std::fs::read(&f.prj).unwrap(),
        f.prj_bytes,
        "byte-identical"
    );
    assert!(
        f.app.ui_state.app_quit_confirm.is_some(),
        "the prompt stays"
    );
    assert!(f.app.document_state.is_document_dirty(&f.prj));
    assert_eq!(std::fs::read(&f.sheet).unwrap(), f.sheet_bytes);
}
