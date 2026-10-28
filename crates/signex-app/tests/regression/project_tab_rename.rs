//! #1202 — renaming the project from the Project tab. The name is not a
//! document edit: the General section's Rename… runs the project-tree Rename
//! flow, immediately, and no save path renames anything.

use signex_app::app::{FileMsg, Message, ProjectMsg, ProjectTabMsg, RenameMsg};

use crate::project_tab_support::{
    edit_description, fixture, on_disk, open_tab, project_id, within_watchdog,
};

fn rename_from_tab(app: &mut signex_app::app::Signex, stem: &str) {
    let project_id = project_id(app);
    let _ = app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::RenameRequested { project_id },
    )));
    let _ = app.update(Message::Rename(RenameMsg::BufferChanged(stem.to_string())));
    let _ = app.update(Message::Rename(RenameMsg::Submit));
}

/// File names in the project folder, as the volume spells them.
fn names_in(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn rename_in_the_general_section_opens_the_project_rename_dialog() {
    let mut f = fixture();
    open_tab(&mut f.app);
    let project_id = project_id(&f.app);
    let _ = f.app.update(Message::Project(ProjectMsg::Tab(
        ProjectTabMsg::RenameRequested { project_id },
    )));
    let dialog = f
        .app
        .ui_state
        .rename_dialog
        .as_ref()
        .expect("dialog opened");
    assert!(dialog.is_project_rename);
    assert_eq!(dialog.target_path, f.prj);
    assert!(
        f.app.document_state.dirty_paths.is_empty(),
        "asking to rename is not a document edit"
    );
}

#[test]
fn renaming_moves_the_file_and_the_tab_and_recent_list_follow() {
    let mut f = fixture();
    // The list is seeded from the shared test prefs; start it from this project.
    f.app.ui_state.recent_projects.clear();
    f.app.ui_state.recent_projects.record(&f.prj);
    open_tab(&mut f.app);
    rename_from_tab(&mut f.app, "Oscillator");

    let renamed = f.prj.with_file_name("Oscillator.snxprj");
    assert!(
        f.app.ui_state.rename_dialog.is_none(),
        "the rename went through"
    );
    assert!(renamed.exists() && !f.prj.exists());
    assert_eq!(f.app.document_state.projects[0].path, renamed);
    assert_eq!(f.app.document_state.projects[0].data.name, "Oscillator");
    let tab = &f.app.document_state.tabs[0];
    assert_eq!(tab.path, renamed);
    assert_eq!(tab.title, "Oscillator.snxprj");
    let recent = f.app.ui_state.recent_projects.entries();
    assert_eq!(recent.len(), 1);
    assert_eq!(
        recent[0].file_name().unwrap(),
        "Oscillator.snxprj",
        "File > Open Recent points at the renamed file"
    );
    assert_eq!(on_disk(&renamed).name, "Oscillator");
}

#[test]
fn a_case_only_rename_works() {
    let mut f = fixture();
    open_tab(&mut f.app);
    rename_from_tab(&mut f.app, "oscar");

    assert!(
        f.app.ui_state.rename_dialog.is_none(),
        "refused: {:?}",
        f.app
            .ui_state
            .rename_dialog
            .as_ref()
            .and_then(|d| d.error.clone())
    );
    let names = names_in(f.tmp.path());
    assert!(names.contains(&"oscar.snxprj".to_string()), "{names:?}");
    assert!(!names.contains(&"Oscar.snxprj".to_string()), "{names:?}");
    assert_eq!(
        f.app.document_state.projects[0].path,
        f.prj.with_file_name("oscar.snxprj")
    );
}

#[test]
fn rename_while_the_project_file_is_being_written_is_refused_not_blocked() {
    within_watchdog("rename during a save", || {
        let mut f = fixture();
        open_tab(&mut f.app);
        edit_description(&mut f.app, "being written");
        // The write is handed out; its task has not run yet.
        let save = f.app.update(Message::File(FileMsg::Save));
        rename_from_tab(&mut f.app, "Oscillator");

        let dialog = f.app.ui_state.rename_dialog.as_ref().expect("dialog kept");
        assert!(dialog.error.is_some(), "the refusal says why");
        assert!(f.prj.exists(), "nothing was moved");

        crate::settle_support::settle_task(&mut f.app, save);
        assert_eq!(
            on_disk(&f.prj).description.as_deref(),
            Some("being written")
        );
        rename_from_tab(&mut f.app, "Oscillator");
        assert!(f.prj.with_file_name("Oscillator.snxprj").exists());
    });
}
