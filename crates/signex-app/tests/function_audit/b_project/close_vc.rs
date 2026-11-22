//! Rows m25 (Close Project, unsaved edits) and m26 (Enable Version Control).
//! A confirmed Enable is driven up to its worker and finished with the
//! worker's answer, so the row does not depend on this machine's git identity.

use signex_app::app::{
    EnableVersionControlMsg as Vc, EnableVersionControlOutcome, Message, ProjectCloseChoice,
    ProjectMsg, ProjectSection, ProjectTabMsg, ProjectTreeAction, VersionControlScope,
};

use super::support::{Bench, bench};

const TWO_SHEETS: &[(&str, &str)] = &[("root.snxsch", ""), ("b.snxsch", "")];

/// Close Project with an unsaved description: the prompt is up.
fn prompt_with_edit() -> Bench {
    let mut b = bench(TWO_SHEETS);
    b.open_section(ProjectSection::General);
    let project_id = b.id;
    b.tab(ProjectTabMsg::DescriptionEdited {
        project_id,
        text: "unsaved".into(),
    });
    let root = b.root_key();
    b.action(ProjectTreeAction::CloseProject(root));
    assert!(b.app.ui_state.project_close_confirm.is_some());
    b
}

fn answer(b: &mut Bench, choice: ProjectCloseChoice) {
    b.send(Message::Project(ProjectMsg::CloseConfirm(choice)));
}

#[test]
fn m25_the_prompt_lists_the_unsaved_project_file() {
    let b = prompt_with_edit();
    let prompt = b.app.ui_state.project_close_confirm.as_ref().expect("up");
    assert!(prompt.dirty_paths.contains(&b.prj));
    assert_eq!(prompt.project_name, "Proj");
}

#[test]
fn m25_cancel_keeps_the_project_open_with_its_edit() {
    let mut b = prompt_with_edit();
    answer(&mut b, ProjectCloseChoice::Cancel);
    assert!(b.app.ui_state.project_close_confirm.is_none());
    assert_eq!(b.app.document_state.projects.len(), 1);
    assert_eq!(b.data().description.as_deref(), Some("unsaved"));
    assert!(b.dirty());
    assert_eq!(b.on_disk().description, None, "nothing was written");
}

#[test]
fn m25_save_all_writes_the_edit_and_closes_the_project() {
    let mut b = prompt_with_edit();
    answer(&mut b, ProjectCloseChoice::SaveAll);
    assert!(b.app.document_state.projects.is_empty());
    assert_eq!(b.on_disk().description.as_deref(), Some("unsaved"));
}

#[test]
fn m25_discard_all_closes_the_project_and_writes_nothing() {
    let mut b = prompt_with_edit();
    answer(&mut b, ProjectCloseChoice::DiscardAll);
    assert!(b.app.document_state.projects.is_empty());
    assert!(b.app.document_state.tabs.is_empty());
    assert_eq!(b.on_disk().description, None);
    assert!(!b.app.document_state.is_document_dirty(&b.prj));
}

#[test]
fn m25_a_clean_project_closes_without_a_prompt() {
    let mut b = bench(TWO_SHEETS);
    let root = b.root_key();
    b.action(ProjectTreeAction::CloseProject(root));
    assert!(b.app.ui_state.project_close_confirm.is_none());
    assert!(b.app.document_state.projects.is_empty());
}

fn open_vc(b: &mut Bench) {
    let root = b.root_key();
    b.action(ProjectTreeAction::OpenEnableVersionControl(root));
    assert!(
        b.app.ui_state.enable_version_control.is_some(),
        "the dialog opens on a folder with no repository"
    );
}

fn vc(b: &mut Bench, msg: Vc) {
    b.send(Message::EnableVersionControl(msg));
}

fn state(b: &Bench) -> &signex_app::app::EnableVersionControlState {
    b.app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("the dialog")
}

/// Confirm, leaving the worker unrun, then its answer.
fn confirm_and_finish(b: &mut Bench, outcome: EnableVersionControlOutcome) {
    let _unrun = b.app.update(Message::EnableVersionControl(Vc::Confirm));
    assert!(state(b).working, "the dialog shows the run");
    let project_dir = state(b).project_dir.clone();
    vc(
        b,
        Vc::Finished {
            scope: VersionControlScope::Project,
            project_dir,
            outcome,
        },
    );
}

fn created() -> EnableVersionControlOutcome {
    EnableVersionControlOutcome::Created {
        adapter_error: None,
        commit_warning: None,
    }
}

#[test]
fn m26_the_dialog_opens_with_every_file_ticked_and_a_first_commit_message() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    let dialog = state(&b);
    let rows: Vec<&str> = dialog.items.iter().map(|i| i.relative.as_str()).collect();
    assert!(rows.contains(&"Proj.snxprj"));
    assert!(rows.contains(&"root.snxsch"));
    assert!(rows.contains(&"b.snxsch"));
    assert!(dialog.items.iter().all(|item| item.tracked));
    assert!(dialog.commit_message.contains("Proj"));
    assert!(dialog.commit_every_save);
    assert!(!dialog.use_lfs);
}

#[test]
fn m26_a_row_unticks_and_select_all_ticks_every_row() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    vc(&mut b, Vc::ToggleItem(1));
    assert!(!state(&b).items[1].tracked);
    vc(&mut b, Vc::AllTicked);
    assert!(state(&b).items.iter().all(|item| item.tracked));
}

#[test]
fn m26_the_lfs_switch_message_and_commit_switch_follow_the_controls() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    vc(&mut b, Vc::ToggleLfs);
    vc(&mut b, Vc::CommitEverySaveToggled);
    vc(&mut b, Vc::CommitMessageChanged("First board".into()));
    let dialog = state(&b);
    assert!(dialog.use_lfs);
    assert!(!dialog.commit_every_save);
    assert_eq!(dialog.commit_message, "First board");
}

#[test]
fn m26_close_writes_nothing() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    vc(&mut b, Vc::ToggleLfs);
    vc(&mut b, Vc::Close);
    assert!(b.app.ui_state.enable_version_control.is_none());
    assert!(!b.dir.join(".git").exists());
    assert!(!b.data().enable_git);
    assert!(!b.dirty());
}

#[test]
fn m26_a_blank_first_commit_message_is_refused() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    vc(&mut b, Vc::CommitMessageChanged("   ".into()));
    vc(&mut b, Vc::Confirm);
    assert!(!state(&b).working, "no run starts");
    assert!(!b.dir.join(".git").exists());
}

#[test]
fn m26_the_dialog_cannot_be_closed_or_edited_while_a_run_is_working() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    let _unrun = b.app.update(Message::EnableVersionControl(Vc::Confirm));
    let _ = b
        .app
        .update(Message::EnableVersionControl(Vc::CommitEverySaveToggled));
    let _ = b.app.update(Message::EnableVersionControl(Vc::Close));
    assert!(state(&b).working);
    assert!(state(&b).commit_every_save, "the edit was ignored");
}

#[test]
fn m26_a_finished_run_turns_commit_on_save_on_and_closes_the_dialog() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    confirm_and_finish(&mut b, created());
    assert!(b.app.ui_state.enable_version_control.is_none());
    assert!(b.data().enable_git);
    assert!(b.dirty(), "the flag is saved with the project");
}

#[test]
fn m26_commit_every_save_off_leaves_saves_uncommitted() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    vc(&mut b, Vc::CommitEverySaveToggled);
    confirm_and_finish(&mut b, created());
    assert!(b.app.ui_state.enable_version_control.is_none());
    assert!(!b.data().enable_git);
}

#[test]
fn m26_a_failed_run_keeps_the_dialog_with_the_reason() {
    let mut b = bench(TWO_SHEETS);
    open_vc(&mut b);
    confirm_and_finish(
        &mut b,
        EnableVersionControlOutcome::InitFailed("git lfs is not installed".into()),
    );
    let dialog = state(&b);
    assert!(!dialog.working);
    assert_eq!(dialog.error.as_deref(), Some("git lfs is not installed"));
    assert!(!b.data().enable_git);
}
