//! #1689-#1693: the Sheets, Variants, Libraries, ERC and annotation and
//! Version control sections, driven through `Signex::update`. Each edit
//! dirties the `.snxprj`, writes nothing until Save, and is one undo step.

use std::path::PathBuf;

use signex_types::project::{
    AnnotateUndo, LibraryEntry, LibraryEntryKind, ListMove, VariantProblem,
};

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::state::project_fs::git_marker;
use crate::app::{Message, ProjectMsg, ProjectTabMsg, Signex};

fn app() -> (Signex, ProjectId) {
    let dir = std::env::temp_dir().join(format!("signex-prj-sections-{}", uuid::Uuid::now_v7()));
    let mut app = app_workspace(
        &dir.to_string_lossy(),
        &["a.snxsch", "b.snxsch", "c.snxsch"],
    );
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    app.open_project_tab(0);
    let id = app.document_state.projects[0].id;
    (app, id)
}

fn send(app: &mut Signex, msg: ProjectTabMsg) {
    let _ = app.update(Message::Project(ProjectMsg::Tab(msg)));
}

fn dirty(app: &Signex) -> bool {
    let path = &app.document_state.projects[0].path;
    app.document_state.is_document_dirty(path)
}

fn undo(app: &mut Signex, id: ProjectId) -> bool {
    app.step_project_tab_history(id, true)
}

fn sheets(app: &Signex) -> Vec<String> {
    app.document_state.projects[0].data.sheet_order()
}

#[test]
fn a_sheet_moves_one_page_as_one_undo_step() {
    let (mut app, id) = app();
    send(
        &mut app,
        ProjectTabMsg::SheetMoved {
            project_id: id,
            filename: "c.snxsch".into(),
            step: ListMove::Up,
        },
    );
    assert_eq!(sheets(&app), ["a.snxsch", "c.snxsch", "b.snxsch"]);
    assert!(dirty(&app));
    // The first page cannot move up: nothing changes, no step is recorded.
    send(
        &mut app,
        ProjectTabMsg::SheetMoved {
            project_id: id,
            filename: "a.snxsch".into(),
            step: ListMove::Up,
        },
    );
    assert_eq!(sheets(&app), ["a.snxsch", "c.snxsch", "b.snxsch"]);
    assert!(undo(&mut app, id));
    assert_eq!(sheets(&app), ["a.snxsch", "b.snxsch", "c.snxsch"]);
    assert!(!dirty(&app), "undo returned to the file's content");
    assert!(!undo(&mut app, id));
}

#[test]
fn variants_are_added_renamed_picked_and_removed_through_the_tab() {
    let (mut app, id) = app();
    // `+` adds "Variant 1" with its name open; typing " Lite " and Enter
    // names it.
    send(
        &mut app,
        ProjectTabMsg::NewVariantRequested { project_id: id },
    );
    send(
        &mut app,
        ProjectTabMsg::VariantRenameEdited {
            project_id: id,
            from: "Variant 1".into(),
            text: " Lite ".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::VariantRenameSubmitted { project_id: id },
    );
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.variant_definitions, ["Lite"]);
    assert!(app.document_view.project_tabs[&id].variant_rename.is_none());

    send(
        &mut app,
        ProjectTabMsg::ActiveVariantPicked {
            project_id: id,
            name: Some("Lite".into()),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::VariantRenameEdited {
            project_id: id,
            from: "Lite".into(),
            text: "Mini".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::VariantRenameSubmitted { project_id: id },
    );
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.variant_definitions, ["Mini"]);
    assert_eq!(data.active_variant.as_deref(), Some("Mini"));

    send(
        &mut app,
        ProjectTabMsg::VariantRemoved {
            project_id: id,
            name: "Mini".into(),
        },
    );
    let data = &app.document_state.projects[0].data;
    assert!(data.variant_definitions.is_empty());
    assert_eq!(data.active_variant, None, "Base is active again");

    // Five steps back: the removal, the rename, the pick, the naming, the
    // add.
    for _ in 0..5 {
        assert!(undo(&mut app, id));
    }
    let data = &app.document_state.projects[0].data;
    assert!(data.variant_definitions.is_empty());
    assert!(!dirty(&app));
}

#[test]
fn a_refused_variant_name_shows_its_reason_and_changes_nothing() {
    let (mut app, id) = app();
    // `+` adds "Variant 1" (one undo step); naming it "base" is refused.
    send(
        &mut app,
        ProjectTabMsg::NewVariantRequested { project_id: id },
    );
    send(
        &mut app,
        ProjectTabMsg::VariantRenameEdited {
            project_id: id,
            from: "Variant 1".into(),
            text: "base".into(),
        },
    );
    send(
        &mut app,
        ProjectTabMsg::VariantRenameSubmitted { project_id: id },
    );
    let tab = &app.document_view.project_tabs[&id];
    assert_eq!(tab.variant_problem, Some(VariantProblem::Reserved));
    assert_eq!(
        tab.variant_rename.as_ref().map(|draft| draft.text.as_str()),
        Some("base"),
        "the typed name stays"
    );
    assert_eq!(
        app.document_state.projects[0].data.variant_definitions,
        ["Variant 1"],
        "the refused name changed nothing"
    );
    // The add is the only step: one undo returns to the file's content.
    assert!(undo(&mut app, id));
    assert!(!dirty(&app));
    assert!(!undo(&mut app, id));
}

#[test]
fn a_library_moves_as_one_undo_step() {
    let (mut app, id) = app();
    app.document_state.projects[0].data.libraries = ["x", "y"]
        .iter()
        .map(|name| LibraryEntry {
            path: PathBuf::from(format!("{name}.snxlib")),
            kind: LibraryEntryKind::ProjectLocal,
            library_id: None,
            unknown: Default::default(),
        })
        .collect();
    let project = &app.document_state.projects[0];
    let rows = app.project_library_rows(project);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name(), "x");
    send(
        &mut app,
        ProjectTabMsg::LibraryMoved {
            project_id: id,
            path: PathBuf::from("x.snxlib"),
            step: ListMove::Down,
        },
    );
    let order = app.document_state.projects[0].data.library_order();
    assert_eq!(
        order,
        [PathBuf::from("y.snxlib"), PathBuf::from("x.snxlib")]
    );
    assert!(dirty(&app));
    assert!(undo(&mut app, id));
    let order = app.document_state.projects[0].data.library_order();
    assert_eq!(
        order,
        [PathBuf::from("x.snxlib"), PathBuf::from("y.snxlib")]
    );
}

#[test]
fn the_project_annotate_choice_overrides_preferences_and_none_follows_them() {
    let (mut app, id) = app();
    app.ui_state.annotate_undo = AnnotateUndo::PerSheet;
    let root = app.document_state.projects[0]
        .path
        .with_file_name("a.snxsch");
    assert_eq!(app.annotate_undo_of(Some(&root)), AnnotateUndo::PerSheet);
    send(
        &mut app,
        ProjectTabMsg::AnnotateUndoPicked {
            project_id: id,
            choice: Some(AnnotateUndo::Project),
        },
    );
    let data = &app.document_state.projects[0].data;
    assert_eq!(data.annotate_undo(), Some(AnnotateUndo::Project));
    assert_eq!(app.annotate_undo_of(Some(&root)), AnnotateUndo::Project);
    assert_eq!(
        app.ui_state.annotate_undo,
        AnnotateUndo::PerSheet,
        "Preferences are not touched"
    );
    // A loose sheet follows Preferences.
    assert_eq!(app.annotate_undo_of(None), AnnotateUndo::PerSheet);
    assert!(undo(&mut app, id));
    assert_eq!(app.document_state.projects[0].data.annotate_undo(), None);
}

#[test]
fn storing_erc_rules_from_the_tab_is_a_rule_history_step() {
    let (mut app, id) = app();
    assert!(app.document_state.projects[0].data.rules.is_none());
    send(
        &mut app,
        ProjectTabMsg::ErcRulesStoreRequested { project_id: id },
    );
    assert!(app.document_state.projects[0].data.rules.is_some());
    assert!(app.project_step_is_rules(id, true));
    assert!(dirty(&app));
    // A second request stores nothing more.
    assert_eq!(app.store_erc_rules_of(id), Ok(false));
    assert!(app.undo_project_rules());
    assert!(app.document_state.projects[0].data.rules.is_none());
}

#[test]
fn commit_on_save_needs_a_repository_and_turns_off_without_one() {
    let (mut app, id) = app();
    let folder = app.document_state.projects[0]
        .path
        .parent()
        .unwrap()
        .to_path_buf();
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), false);
    send(
        &mut app,
        ProjectTabMsg::VersionControlToggled {
            project_id: id,
            on: true,
        },
    );
    assert!(!app.document_state.projects[0].data.enable_git);
    assert!(!dirty(&app));

    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), true);
    send(
        &mut app,
        ProjectTabMsg::VersionControlToggled {
            project_id: id,
            on: true,
        },
    );
    assert!(app.document_state.projects[0].data.enable_git);
    assert!(dirty(&app));

    // Off is allowed even when the repository is gone.
    app.document_state
        .project_fs
        .facts
        .set_exists(git_marker(&folder), false);
    send(
        &mut app,
        ProjectTabMsg::VersionControlToggled {
            project_id: id,
            on: false,
        },
    );
    assert!(!app.document_state.projects[0].data.enable_git);
    assert!(undo(&mut app, id));
    assert!(app.document_state.projects[0].data.enable_git);
}

#[test]
fn every_section_renders() {
    let (mut app, id) = app();
    for section in crate::app::ProjectSection::ALL {
        send(
            &mut app,
            ProjectTabMsg::SectionSelected {
                project_id: id,
                section,
            },
        );
        let _ = app.view_project_tab(id);
    }
}

/// #1693: the Enable Version Control dialog's success turns commit on save
/// on as a Project tab step, so the project is dirty and Ctrl+Z in the tab
/// turns it off again.
#[test]
fn enable_version_control_success_is_a_general_step() {
    let (mut app, id) = app();
    let project_dir = PathBuf::from(&app.document_state.projects[0].data.dir);
    let _ = app.update(Message::EnableVersionControl(
        crate::app::EnableVersionControlMsg::Finished {
            scope: crate::app::VersionControlScope::Project,
            project_dir: project_dir.clone(),
            outcome: crate::app::EnableVersionControlOutcome::Created {
                adapter_error: None,
                commit_warning: None,
            },
        },
    ));
    assert!(app.document_state.projects[0].data.enable_git);
    assert!(dirty(&app));
    assert!(
        !app.document_state.project_fs.facts.lacks_git(&project_dir),
        "the repository is recorded"
    );
    assert!(undo(&mut app, id));
    assert!(!app.document_state.projects[0].data.enable_git);
    assert!(!dirty(&app));
}
