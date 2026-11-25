//! A project read from a newer `.snxprj` minor (#2904): its Save, rename and
//! Add New are refused up front, Close Project and Exit withhold Save All,
//! and its Project tab carries the read-only banner.

use std::path::PathBuf;

use signex_i18n::RenderTranslatable;

use super::{ReadOnlyProjectSave, save_all_withheld_line, save_failure_detail};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::state::ProjectId;
use crate::app::{AppQuitConfirmState, ProjectCloseChoice, ProjectCloseConfirmState, Signex};
use crate::persist::run_task::outputs;

/// One project, read as `snxprj/1.2`, with unsaved settings.
fn read_only_workspace() -> (Signex, ProjectId, PathBuf) {
    let mut app = app_workspace("/ro", &[]);
    let project = &mut app.document_state.projects[0];
    project.data.unknown.newer_minor_source = Some("snxprj/1.2".into());
    let (id, path) = (project.id, project.path.clone());
    app.document_state.mark_document_dirty(&path);
    (app, id, path)
}

fn close_prompt(app: &mut Signex, project_id: ProjectId, dirty: Vec<PathBuf>) {
    app.ui_state.project_close_confirm = Some(Box::new(ProjectCloseConfirmState {
        project_id,
        project_name: "ProjectA".into(),
        dirty_paths: dirty,
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs: Default::default(),
    }));
}

fn plain(text: &str) -> String {
    text.replace(['\u{2068}', '\u{2069}'], "")
}

#[test]
fn a_read_only_project_save_is_refused_with_its_own_sentence() {
    let (mut app, id, path) = read_only_workspace();
    let Err(error) = app.begin_project_save(id) else {
        panic!("a read-only project's save is refused");
    };
    assert!(ReadOnlyProjectSave::find(&error).is_some(), "{error:#}");
    let chain = format!("{error:#}");
    assert!(
        chain.contains("is read-only") && chain.contains("snxprj/1.2"),
        "{chain}"
    );

    app.report_save_failure("Failed to save project file", &error);
    let notice = app.document_state.error_notice.as_ref().expect("card");
    let detail = plain(&notice.detail.render_en_us());
    assert!(
        detail.contains("'ProjectA'")
            && detail.contains("read-only")
            && detail.contains("snxprj/1.2"),
        "{detail}"
    );
    assert!(app.document_state.is_document_dirty(&path));
}

#[test]
fn any_other_save_failure_keeps_the_callers_sentence() {
    let error = anyhow::anyhow!("disk full");
    let detail = save_failure_detail("Failed to save active document", &error);
    assert_eq!(detail.render_en_us(), "Failed to save active document");
}

#[test]
fn only_a_read_only_project_with_unsaved_settings_withholds_save_all() {
    let (mut app, _, path) = read_only_workspace();
    let sheet = PathBuf::from("/ro/a.snxsch");
    assert_eq!(
        app.read_only_dirty_projects(&[sheet.clone(), path.clone()]),
        ["ProjectA"]
    );
    assert!(app.read_only_dirty_projects(&[sheet]).is_empty());
    app.document_state.projects[0]
        .data
        .unknown
        .newer_minor_source = None;
    assert!(app.read_only_dirty_projects(&[path]).is_empty());
}

#[test]
fn the_withheld_note_names_the_projects_and_offers_discard() {
    let en = signex_i18n::I18n::en_us_ref();
    assert!(save_all_withheld_line(en, &[]).is_none());
    let one = plain(
        &save_all_withheld_line(en, &["P".into()])
            .unwrap()
            .into_string(),
    );
    assert!(one.starts_with("'P' was written by a newer"), "{one}");
    assert!(one.contains("Discard All"), "{one}");
    let two = plain(
        &save_all_withheld_line(en, &["P".into(), "Q".into()])
            .unwrap()
            .into_string(),
    );
    assert!(two.starts_with("'P', 'Q' were written"), "{two}");
}

#[test]
fn a_forced_close_save_all_is_refused_and_the_prompt_stays() {
    let (mut app, id, path) = read_only_workspace();
    close_prompt(&mut app, id, vec![path.clone()]);

    let task = app.handle_project_close_choice(ProjectCloseChoice::SaveAll);

    assert!(outputs(task).is_empty(), "nothing is written");
    assert!(
        app.ui_state.project_close_confirm.is_some(),
        "the prompt stays"
    );
    assert!(app.document_state.is_document_dirty(&path));
    assert_eq!(app.document_state.projects.len(), 1, "still open");
}

#[test]
fn a_forced_exit_save_all_is_refused_and_cancel_still_answers() {
    let (mut app, _, path) = read_only_workspace();
    app.ui_state.app_quit_confirm = Some(AppQuitConfirmState {
        dirty_paths: vec![path.clone()],
        dirty_rows: Vec::new(),
    });

    let task = app.handle_app_quit_choice(ProjectCloseChoice::SaveAll);
    assert!(outputs(task).is_empty(), "nothing is written");
    assert!(app.ui_state.app_quit_confirm.is_some(), "the prompt stays");
    assert!(app.document_state.is_document_dirty(&path));

    let _ = app.handle_app_quit_choice(ProjectCloseChoice::Cancel);
    assert!(app.ui_state.app_quit_confirm.is_none(), "Cancel closes it");
}

#[test]
fn enter_does_not_save_all_while_it_is_withheld() {
    use crate::app::state::ModalId;
    let (mut app, id, path) = read_only_workspace();
    close_prompt(&mut app, id, vec![path]);
    assert!(
        app.confirm_enter_message(ModalId::ProjectCloseConfirm)
            .is_none()
    );
    close_prompt(&mut app, id, vec![PathBuf::from("/ro/a.snxsch")]);
    assert!(
        app.confirm_enter_message(ModalId::ProjectCloseConfirm)
            .is_some()
    );
}

#[test]
fn a_rename_in_a_read_only_project_is_refused_before_anything_moves() {
    let (mut app, _, _) = read_only_workspace();
    let refusal = plain(
        &app.read_only_project_rename_refusal(Some(0))
            .expect("refused"),
    );
    assert!(
        refusal.contains("'ProjectA'")
            && refusal.contains("snxprj/1.2")
            && refusal.contains("Nothing was changed"),
        "{refusal}"
    );
    assert!(app.read_only_project_rename_refusal(None).is_none());
    app.document_state.projects[0]
        .data
        .unknown
        .newer_minor_source = None;
    assert!(app.read_only_project_rename_refusal(Some(0)).is_none());
}

#[test]
fn add_new_in_a_read_only_project_is_refused_with_a_card() {
    let (mut app, id, _) = read_only_workspace();
    let sheet = PathBuf::from("/ro/Added.snxsch");
    assert!(app.refuse_add_in_read_only_project(id, &sheet));
    let notice = app.document_state.error_notice.as_ref().expect("card");
    let detail = plain(&notice.detail.render_en_us());
    assert!(
        detail.contains("'ProjectA'") && detail.contains("Added.snxsch was not created"),
        "{detail}"
    );

    app.document_state.error_notice = None;
    app.document_state.projects[0]
        .data
        .unknown
        .newer_minor_source = None;
    assert!(!app.refuse_add_in_read_only_project(id, &sheet));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn the_project_tab_of_a_read_only_project_carries_the_banner() {
    let (mut app, id, _) = read_only_workspace();
    app.open_project_tab(0);
    let banner = |app: &Signex| {
        let texts = crate::layout_probe::lay_out_texts(
            app.view_project_tab(id),
            iced::Size::new(1200.0, 800.0),
        );
        texts
            .iter()
            .map(|t| plain(&t.text))
            .find(|t| t.starts_with("Read-only: this project"))
    };
    let shown = banner(&app).expect("the banner");
    assert!(shown.contains("snxprj/1.2"), "{shown}");
    app.document_state.projects[0]
        .data
        .unknown
        .newer_minor_source = None;
    assert!(banner(&app).is_none(), "a writable project has none");
}
