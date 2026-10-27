//! #1202 — the Project document tab (General section) replaces the read-only
//! Project Options modal. Each test drives `Signex::update` the way the UI
//! does; the off-thread `.snxprj` write is run by hand (`settle_task`).

use signex_app::app::{
    ContextMenuMsg, EditMsg, ErcMsg, FileMsg, Message, ProjectCloseChoice, ProjectGeneralFacts,
    ProjectMsg, TabContextAction, WindowMsg,
};

use crate::project_tab_support::{
    add_net_class, edit_description, fixture, is_dirty, on_disk, open_dirty_root_sheet, open_tab,
    project_id, save, within_watchdog,
};

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
fn description_edits_undo_and_redo_as_one_typing_run() {
    let mut f = fixture();
    open_tab(&mut f.app);
    for text in ["M", "Mo", "Motor"] {
        edit_description(&mut f.app, text);
    }
    assert!(is_dirty(&f));
    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(f.app.document_state.projects[0].data.description, None);
    let _ = f.app.update(Message::Edit(EditMsg::Redo));
    assert_eq!(
        f.app.document_state.projects[0].data.description.as_deref(),
        Some("Motor")
    );
}

#[test]
fn typing_after_a_save_is_a_new_undo_step() {
    let mut f = fixture();
    open_tab(&mut f.app);
    edit_description(&mut f.app, "saved");
    save(&mut f.app);
    edit_description(&mut f.app, "saved and more");
    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    assert_eq!(
        f.app.document_state.projects[0].data.description.as_deref(),
        Some("saved"),
        "undo stops at what was saved"
    );
}

#[test]
fn undo_follows_the_window_it_came_from_with_the_erc_dialog_over_a_project_tab() {
    use signex_erc::{RuleKind, Severity};
    let mut f = fixture();
    open_tab(&mut f.app);
    let _ = f.app.update(Message::Erc(ErcMsg::OpenDialog));
    let _ = f.app.update(Message::Erc(ErcMsg::StoreRulesInProject));
    let seeded = f.app.document_state.projects[0].data.rules.clone();
    edit_description(&mut f.app, "kept");
    let _ = f.app.update(Message::Erc(ErcMsg::SeverityChanged(
        RuleKind::UnusedPin,
        Severity::Info,
    )));
    let edited = f.app.document_state.projects[0].data.rules.clone();
    assert_ne!(edited, seeded);

    // The ERC window's Ctrl+Z steps the rule history, not the description.
    let _ = f.app.update(Message::Erc(ErcMsg::RulesUndone));
    let data = &f.app.document_state.projects[0].data;
    assert_eq!(
        data.rules, seeded,
        "Ctrl+Z in the ERC window steps the rules"
    );
    assert_eq!(data.description.as_deref(), Some("kept"));

    // The main window's Ctrl+Z undoes the Project tab, and leaves the rules.
    let _ = f.app.update(Message::Edit(EditMsg::Undo));
    let data = &f.app.document_state.projects[0].data;
    assert_eq!(data.description, None, "the main window undoes the tab");
    assert_eq!(data.rules, seeded, "the hidden rule history is untouched");
}

#[test]
fn the_project_tab_stays_docked() {
    let mut f = fixture();
    open_tab(&mut f.app);
    let _ = f.app.update(Message::Window(WindowMsg::UndockTab(0)));
    assert!(
        f.app.ui_state.windows.is_empty(),
        "a Project tab is not undocked in this slice"
    );
    assert_eq!(f.app.document_state.tabs.len(), 1);
}

#[test]
fn ctrl_s_in_a_dirty_sheet_with_project_tab_edits_writes_both_and_returns() {
    within_watchdog("Ctrl+S from the sheet", || {
        let mut f = fixture();
        open_tab(&mut f.app);
        edit_description(&mut f.app, "from the sheet");
        let sheet = open_dirty_root_sheet(&mut f);
        save(&mut f.app);
        assert!(sheet.exists(), "the sheet was written");
        assert_eq!(
            on_disk(&f.prj).description.as_deref(),
            Some("from the sheet")
        );
        assert!(f.app.document_state.dirty_paths.is_empty());
    });
}

#[test]
fn quit_save_all_with_a_dirty_sheet_and_project_tab_edits_writes_both_and_returns() {
    within_watchdog("quit Save All", || {
        let mut f = fixture();
        open_tab(&mut f.app);
        edit_description(&mut f.app, "at quit");
        let sheet = open_dirty_root_sheet(&mut f);
        let _ = f.app.update(Message::Window(WindowMsg::CloseMainWindow));
        crate::settle_support::settle(
            &mut f.app,
            Message::Project(ProjectMsg::AppQuitConfirm(ProjectCloseChoice::SaveAll)),
        );
        assert!(sheet.exists());
        assert_eq!(on_disk(&f.prj).description.as_deref(), Some("at quit"));
        assert!(f.app.document_state.dirty_paths.is_empty());
    });
}
