//! #742 — library context-menu actions name their library by identity (the
//! owning project's id and the resolved `.snxlib` path), so rows shifting
//! between opening the menu and choosing an action cannot retarget another
//! library.

use std::path::PathBuf;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{
    ContextMenuMsg, LibraryRowRef, Message, ProjectTreeAction, RemoveChoice, RemoveMsg, Signex,
};
use signex_types::project::{LibraryEntry, LibraryEntryKind};

fn power(dir: &str) -> PathBuf {
    PathBuf::from(dir).join("Power.snxlib")
}

/// One project listing two shared libraries that are both named `Power.snxlib`.
fn app_with_two_power_libraries() -> (Signex, LibraryRowRef, LibraryRowRef) {
    let mut app = app_workspace("/proj", &[]);
    for dir in ["/libs_a", "/libs_b"] {
        app.document_state.projects[0]
            .data
            .libraries
            .push(LibraryEntry {
                path: power(dir),
                kind: LibraryEntryKind::Shared,
                library_id: None,
                unknown: Default::default(),
            });
    }
    app.refresh_panel_ctx();
    let project_id = app.document_state.projects[0].id;
    let row = |dir| LibraryRowRef {
        project_id,
        root: power(dir),
    };
    (app, row("/libs_a"), row("/libs_b"))
}

fn pick(app: &mut Signex, action: ProjectTreeAction) {
    let _ = app.update(Message::ContextMenu(ContextMenuMsg::ProjectTreeAction(
        action,
    )));
}

#[test]
fn remove_follows_the_library_when_a_row_before_it_is_removed() {
    let (mut app, _first, second) = app_with_two_power_libraries();

    // The menu was opened on the second row; the first row then disappears,
    // so the second library now sits at index 0.
    app.document_state.projects[0].data.libraries.remove(0);
    app.refresh_panel_ctx();
    pick(&mut app, ProjectTreeAction::RemoveLibrary(second.clone()));

    let dialog = app.ui_state.remove_dialog.as_ref().expect("remove dialog");
    assert_eq!(dialog.target_path, second.root);
}

#[test]
fn remove_refuses_a_library_that_left_the_project() {
    let _ = crate::diagnostics::init_logging();
    let (mut app, _first, second) = app_with_two_power_libraries();

    app.document_state.projects[0].data.libraries.remove(1);
    app.refresh_panel_ctx();
    pick(&mut app, ProjectTreeAction::RemoveLibrary(second));

    assert!(
        app.ui_state.remove_dialog.is_none(),
        "must not fall onto the library now at that index"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("library no longer in the project")),
        "the refusal must reach the Messages panel"
    );
}

#[test]
fn confirming_remove_drops_only_the_targeted_library_of_a_shared_name() {
    let (mut app, first, second) = app_with_two_power_libraries();

    pick(&mut app, ProjectTreeAction::RemoveLibrary(second));
    let _ = app.update(Message::Remove(RemoveMsg::Confirm(
        RemoveChoice::ExcludeFromProject,
    )));

    let left: Vec<_> = app.document_state.projects[0]
        .data
        .libraries
        .iter()
        .map(|e| e.path.clone())
        .collect();
    assert_eq!(left, [first.root]);
}
