//! #1178 (mockup row 205): the Fonts section, driven through `Signex::update`
//! (the file the picker answered) and through the catalog commands (a
//! headless run). An add copies the file into `assets/fonts/` and lists it as
//! one undo step that dirties the `.snxprj` and writes nothing else until
//! Save; a restricted, malformed or oversized file is refused and nothing is
//! copied or listed; a save writes the list and reads back the same.

use std::path::{Path, PathBuf};

use serde_json::Value;
use signex_i18n::RenderTranslatable;
use signex_types::project::{MAX_PROJECT_FONT_BYTES, parse_project, write_project};

use super::ProjectFontsMsg;
use crate::app::command::{CommandArgs, CommandOutcome};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{Message, ProjectMsg, ProjectSection, ProjectTabMsg, Signex};
use crate::keymap::AppCommandId;

const ROBOTO: &[u8] = include_bytes!("../../../../assets/fonts/Roboto-Regular.ttf");

/// A project tab over a project file written to a folder of its own; the
/// folder lives as long as the returned guard.
fn app() -> (Signex, ProjectId, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("a folder");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let project = &app.document_state.projects[0];
    write_project(&project.path, &project.data).expect("the project file");
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id, dir)
}

/// `font` with its `OS/2.fsType` set to `fs_type`.
fn with_fs_type(font: &[u8], fs_type: u16) -> Vec<u8> {
    let mut bytes = font.to_vec();
    let tables = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let os2 = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&record| &bytes[record..record + 4] == b"OS/2")
        .map(|record| {
            let at = &bytes[record + 8..record + 12];
            u32::from_be_bytes([at[0], at[1], at[2], at[3]]) as usize
        })
        .expect("the font has an OS/2 table");
    bytes[os2 + 8..os2 + 10].copy_from_slice(&fs_type.to_be_bytes());
    bytes
}

/// `bytes` as `name` in a folder outside the project.
fn source(outside: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = outside.join(name);
    std::fs::write(&path, bytes).expect("a font file");
    path
}

fn project_dir(app: &Signex) -> PathBuf {
    app.document_state.projects[0].dir().to_path_buf()
}

fn copied(app: &Signex) -> Vec<PathBuf> {
    match std::fs::read_dir(project_dir(app).join("assets").join("fonts")) {
        Ok(entries) => entries.map(|entry| entry.expect("entry").path()).collect(),
        Err(_) => Vec::new(),
    }
}

fn families(app: &Signex) -> Vec<String> {
    app.document_state.projects[0]
        .data
        .project_fonts()
        .expect("readable")
        .into_iter()
        .map(|entry| entry.family)
        .collect()
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn on_disk(app: &Signex) -> Vec<u8> {
    std::fs::read(&app.document_state.projects[0].path).expect("the project file")
}

fn send(app: &mut Signex, id: ProjectId, msg: ProjectFontsMsg) {
    let task = app.update(Message::Project(ProjectMsg::Tab(ProjectTabMsg::Fonts {
        project_id: id,
        msg,
    })));
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
}

/// The file the picker answered with, as Add font file... sends it.
fn pick(app: &mut Signex, id: ProjectId, path: &Path) {
    send(
        app,
        id,
        ProjectFontsMsg::FilePicked(Some(path.to_path_buf())),
    );
}

fn problem(app: &Signex, id: ProjectId) -> Option<String> {
    app.document_state.project_tabs[&id]
        .fonts
        .problem
        .as_ref()
        .map(|reason| reason.render_en_us())
}

fn run(app: &mut Signex, command: &str, operands: &[&str]) -> CommandOutcome {
    let document = format!("document={}", app.document_state.projects[0].path.display());
    let mut all = vec![document.as_str()];
    all.extend_from_slice(operands);
    let command = AppCommandId::new(command).expect("a catalog id");
    let args = CommandArgs::parse_operands(&command, &all).expect("operands parse");
    let (outcome, task) = match app.run_query(&command, &args) {
        Some(ran) => ran,
        None => app.run_operands_settled(&command, &args),
    };
    crate::task_driver::TaskDriver::new()
        .settle(app, task)
        .expect("the task settles");
    outcome
}

fn answer(app: &mut Signex, command: &str, operands: &[&str]) -> Value {
    match run(app, command, operands) {
        CommandOutcome::Answered(value) => value,
        other => panic!("{command} did not answer: {other:?}"),
    }
}

fn refusal(app: &mut Signex, command: &str, operands: &[&str]) -> String {
    match run(app, command, operands) {
        CommandOutcome::Disabled { reason } => reason.render_en_us(),
        other => panic!("{command} was not refused: {other:?}"),
    }
}

#[test]
fn a_picked_font_is_copied_listed_as_one_undo_step_and_written_on_save_only() {
    let (mut app, id, _dir) = app();
    let outside = tempfile::tempdir().expect("a folder");
    let before = on_disk(&app);
    pick(&mut app, id, &source(outside.path(), "House.ttf", ROBOTO));
    assert_eq!(families(&app), ["Roboto"]);
    let files = copied(&app);
    assert_eq!(files.len(), 1, "one content-addressed copy");
    assert_eq!(std::fs::read(&files[0]).unwrap(), ROBOTO);
    assert!(dirty(&app));
    assert_eq!(on_disk(&app), before, "the .snxprj waits for Save");
    assert!(app.step_project_tab_history(id, true));
    assert!(families(&app).is_empty());
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(app.step_project_tab_history(id, false));
    assert_eq!(families(&app), ["Roboto"]);
    let project = &app.document_state.projects[0];
    write_project(&project.path, &project.data).expect("save");
    let reread = parse_project(&project.path).expect("reads back");
    assert_eq!(
        reread.project_fonts().unwrap(),
        project.data.project_fonts().unwrap()
    );
}

#[test]
fn the_same_font_twice_is_one_file_and_one_listing() {
    let (mut app, id, _dir) = app();
    let outside = tempfile::tempdir().expect("a folder");
    pick(&mut app, id, &source(outside.path(), "A.ttf", ROBOTO));
    pick(&mut app, id, &source(outside.path(), "B.TTF", ROBOTO));
    assert_eq!(copied(&app).len(), 1);
    let fonts = app.document_state.projects[0].data.project_fonts().unwrap();
    assert_eq!(fonts.len(), 1);
    assert_eq!(fonts[0].files.len(), 1);
}

#[test]
fn a_restricted_malformed_oversized_or_foreign_file_is_refused_and_nothing_is_copied() {
    let (mut app, id, _dir) = app();
    let outside = tempfile::tempdir().expect("a folder");
    let restricted = source(outside.path(), "Locked.ttf", &with_fs_type(ROBOTO, 0x0002));
    pick(&mut app, id, &restricted);
    assert!(
        problem(&app, id).is_some_and(|why| why.contains("does not allow embedding")),
        "{:?}",
        problem(&app, id)
    );
    let malformed = source(
        outside.path(),
        "Broken.otf",
        b"OTTO and then nothing useful",
    );
    pick(&mut app, id, &malformed);
    assert!(
        problem(&app, id).is_some_and(|why| why.contains("is not a font this build can read")),
        "{:?}",
        problem(&app, id)
    );
    let oversized = outside.path().join("Huge.ttf");
    std::fs::File::create(&oversized)
        .and_then(|file| file.set_len(MAX_PROJECT_FONT_BYTES + 1))
        .expect("a sparse file");
    pick(&mut app, id, &oversized);
    assert!(
        problem(&app, id).is_some_and(|why| why.contains("is larger than 32 MiB")),
        "{:?}",
        problem(&app, id)
    );
    pick(&mut app, id, &source(outside.path(), "House.woff2", ROBOTO));
    assert!(
        problem(&app, id).is_some_and(|why| why.contains("is not a .ttf, .otf or .ttc")),
        "{:?}",
        problem(&app, id)
    );
    assert!(copied(&app).is_empty());
    assert_eq!(
        app.document_state.projects[0].data.project_fonts_raw(),
        None
    );
    assert!(!dirty(&app));
}

#[test]
fn a_license_commits_on_enter_and_remove_drops_the_family() {
    let (mut app, id, _dir) = app();
    let outside = tempfile::tempdir().expect("a folder");
    pick(&mut app, id, &source(outside.path(), "House.ttf", ROBOTO));
    let family = || "Roboto".to_string();
    send(
        &mut app,
        id,
        ProjectFontsMsg::LicenseEdited {
            family: family(),
            text: "OFL-1.1".into(),
        },
    );
    let stored = |app: &Signex| {
        app.document_state.projects[0]
            .data
            .project_font_family("Roboto")
            .and_then(|entry| entry.license)
    };
    assert_eq!(stored(&app), None, "typing is not a commit");
    send(
        &mut app,
        id,
        ProjectFontsMsg::LicenseSubmitted { family: family() },
    );
    assert_eq!(stored(&app).as_deref(), Some("OFL-1.1"));
    send(&mut app, id, ProjectFontsMsg::Removed { family: family() });
    assert!(families(&app).is_empty());
    assert_eq!(copied(&app).len(), 1, "the file stays for an undo");
    assert!(app.step_project_tab_history(id, true));
    assert_eq!(stored(&app).as_deref(), Some("OFL-1.1"));
}

#[test]
fn the_commands_add_set_and_remove_and_answer_with_the_list() {
    let (mut app, _id, _dir) = app();
    let outside = tempfile::tempdir().expect("a folder");
    let font = source(outside.path(), "House.ttf", ROBOTO);
    let path = format!("path={}", font.display());
    let added = answer(&mut app, "project_add_font", &[path.as_str()]);
    assert_eq!(added["changed"], true);
    assert_eq!(added["added"]["embedding"], "installable");
    assert_eq!(
        added["added"]["license_hint"],
        "https://openfontlicense.org"
    );
    assert_eq!(added["fonts"][0]["family"], "Roboto");
    let licensed = answer(
        &mut app,
        "project_set_font_license",
        &["family=roboto", "value=OFL-1.1"],
    );
    assert_eq!(licensed["fonts"][0]["license"], "OFL-1.1");
    let settings = answer(&mut app, "project_settings", &[]);
    assert_eq!(settings["fonts"], licensed["fonts"]);
    assert_eq!(settings["fonts_unreadable"], false);
    assert_eq!(
        refusal(&mut app, "project_remove_font", &["family=Other"]),
        "The project has no font Other"
    );
    let removed = answer(&mut app, "project_remove_font", &["family=Roboto"]);
    assert_eq!(removed["fonts"], serde_json::json!([]));
    assert_eq!(removed["changed"], true);
}

#[test]
fn a_headless_add_of_a_restricted_font_is_refused_with_the_reason() {
    let (mut app, _id, _dir) = app();
    let outside = tempfile::tempdir().expect("a folder");
    let font = source(outside.path(), "Locked.ttf", &with_fs_type(ROBOTO, 0x0002));
    let path = format!("path={}", font.display());
    assert_eq!(
        refusal(&mut app, "project_add_font", &[path.as_str()]),
        "Roboto does not allow embedding, so it is not copied into the project"
    );
    assert!(copied(&app).is_empty());
}

#[test]
fn the_fonts_section_renders_with_and_without_fonts() {
    let (mut app, id, _dir) = app();
    let select = ProjectTabMsg::SectionSelected {
        project_id: id,
        section: ProjectSection::Fonts,
    };
    let task = app.update(Message::Project(ProjectMsg::Tab(select)));
    crate::task_driver::TaskDriver::new()
        .settle(&mut app, task)
        .expect("the task settles");
    let _ = app.view_project_tab(id);
    let outside = tempfile::tempdir().expect("a folder");
    pick(&mut app, id, &source(outside.path(), "House.ttf", ROBOTO));
    let _ = app.view_project_tab(id);
}
