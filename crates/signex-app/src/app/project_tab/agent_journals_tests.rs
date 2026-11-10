//! #2253: the Agent journals settings of the Project setup section are a
//! normal project edit: they dirty the `.snxprj`, write nothing until Save,
//! undo, persist, and the managed `.gitignore` follows "keep in git" only when
//! the project is saved.

use signex_types::agent_journal::AgentJournalSettings;
use signex_types::project::parse_project;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};
use crate::task_driver::TaskDriver;

fn app() -> (Signex, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-prj-aj-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(dir.join(".gitignore"), "mine\n").unwrap();
    let mut app = app_workspace(&dir.to_string_lossy(), &["root.snxsch"]);
    app.open_project_tab(0);
    (app, dir)
}

fn settings(app: &Signex) -> AgentJournalSettings {
    app.document_state.projects[0].data.agent_journal
}

fn pick(app: &mut Signex, settings: AgentJournalSettings) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::AgentJournalPicked {
            project_id,
            settings,
        },
    )));
}

fn type_lines(app: &mut Signex, text: &str) {
    let project_id = app.document_state.projects[0].id;
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::AgentJournalLinesEdited {
            project_id,
            text: text.to_string(),
        },
    )));
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn save(app: &mut Signex) {
    let project_id = app.document_state.projects[0].id;
    let task = app.begin_project_save(project_id).unwrap();
    TaskDriver::new().settle(app, task).unwrap();
}

/// A malformed `agent_journal` value is not silently reset: Messages says so.
#[test]
fn a_malformed_agent_journal_value_is_said_in_messages() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Proj.snxprj");
    std::fs::write(
        &path,
        r#"{"name":"Proj","dir":"","schematic_root":null,"pcb_file":null,"agent_journal":"garbage-8c1d"}"#,
    )
    .unwrap();
    let mark = crate::diagnostics::newest_entry_id();

    let data = signex_types::project::parse_project(&path).unwrap();

    assert!(data.agent_journal.is_default());
    let said: Vec<String> = crate::diagnostics::entries_since(mark)
        .entries
        .into_iter()
        .map(|entry| entry.message)
        .collect();
    assert!(
        said.iter()
            .any(|m| m.contains("agent journal settings are not readable")),
        "{said:?}"
    );
}

#[test]
fn the_defaults_are_git_on_summary_off_ten_thousand() {
    let (app, _dir) = app();
    assert_eq!(settings(&app), AgentJournalSettings::default());
}

#[test]
fn a_checkbox_is_a_project_edit_that_writes_nothing_and_undoes() {
    let (mut app, dir) = app();
    assert!(!dirty(&app));
    let off = AgentJournalSettings {
        keep_in_git: false,
        ..settings(&app)
    };

    pick(&mut app, off);

    assert!(!settings(&app).keep_in_git);
    assert!(dirty(&app));
    assert!(!dir.join(".signex").exists());
    let id = app.document_state.projects[0].id;
    assert!(app.step_project_tab_history(id, true));
    assert!(settings(&app).keep_in_git, "undo restores the setting");
    assert!(!dirty(&app));
}

#[test]
fn the_line_cap_is_stored_when_it_is_a_number_in_range_and_kept_as_typed_otherwise() {
    let (mut app, _dir) = app();

    type_lines(&mut app, "2500");
    assert_eq!(settings(&app).max_lines, 2_500);

    // Typing "5" on the way to "500": the stored value stays, the text stays.
    type_lines(&mut app, "5");
    assert_eq!(settings(&app).max_lines, 2_500);
    let id = app.document_state.projects[0].id;
    assert_eq!(
        app.document_state.project_tabs[&id]
            .journal_lines_draft
            .as_deref(),
        Some("5")
    );
    type_lines(&mut app, "500");
    assert_eq!(settings(&app).max_lines, 500);

    for bad in ["", "abc", "99", "-4", "10000000"] {
        type_lines(&mut app, bad);
        assert_eq!(settings(&app).max_lines, 500, "{bad:?} is not stored");
    }
}

#[test]
fn the_settings_are_saved_with_the_project_and_read_back() {
    let (mut app, _dir) = app();
    pick(
        &mut app,
        AgentJournalSettings {
            keep_in_git: false,
            summarise_in_commit: true,
            max_lines: 100,
        },
    );
    let want = settings(&app);

    save(&mut app);

    let path = app.document_state.projects[0].path.clone();
    assert_eq!(parse_project(&path).unwrap().agent_journal, want);
    assert!(!dirty(&app), "saved");
}

#[test]
fn the_gitignore_follows_keep_in_git_when_the_project_is_saved_not_when_toggled() {
    let (mut app, dir) = app();
    let ignore = dir.join(".gitignore");
    let off = AgentJournalSettings {
        keep_in_git: false,
        ..settings(&app)
    };

    pick(&mut app, off);

    // Toggled, not saved: the person may still discard it.
    assert!(
        !std::fs::read_to_string(&ignore)
            .unwrap()
            .contains("agent-*.jsonl")
    );

    save(&mut app);
    let saved = std::fs::read_to_string(&ignore).unwrap();
    assert!(saved.contains(".signex/agent-*.jsonl"), "{saved}");
    assert!(saved.contains("mine"), "the person's lines stay");

    pick(
        &mut app,
        AgentJournalSettings {
            keep_in_git: true,
            ..off
        },
    );
    save(&mut app);
    assert!(
        !std::fs::read_to_string(&ignore)
            .unwrap()
            .contains("agent-*.jsonl")
    );
}
