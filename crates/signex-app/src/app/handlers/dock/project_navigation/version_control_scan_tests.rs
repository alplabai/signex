//! The library-scope Enable Version Control rows are listed on a worker
//! (#859): what the dialog does while they are on their way, and when they
//! arrive stale or fail.

use std::path::PathBuf;

use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::{EnableVersionControlState, Signex, TrackItem, VersionControlScope};

fn scanning_dialog(dir: &str) -> Signex {
    let mut app = app_workspace("/w/a", &["top.snxsch"]);
    app.ui_state.enable_version_control = Some(EnableVersionControlState {
        scope: VersionControlScope::Library,
        project_path: PathBuf::from(dir).join("library.toml"),
        project_dir: PathBuf::from(dir),
        project_name: "Lib".into(),
        items: Vec::new(),
        use_lfs: false,
        intro_text: String::new(),
        error: None,
        working: false,
        scanning: true,
    });
    app
}

fn row(name: &str) -> TrackItem {
    TrackItem {
        absolute: PathBuf::from("/lib").join(name),
        relative: name.to_string(),
        label: "Config".into(),
        is_directory: false,
        tracked: true,
    }
}

#[test]
fn the_rows_fill_the_dialog_that_asked_for_them() {
    let mut app = scanning_dialog("/lib");
    app.handle_enable_version_control_items_scanned(
        std::path::Path::new("/lib"),
        Ok(vec![row("library.toml")]),
    );
    let state = app.ui_state.enable_version_control.expect("dialog");
    assert!(!state.scanning);
    assert_eq!(state.items.len(), 1);
}

#[test]
fn rows_for_another_directory_or_a_closed_dialog_are_dropped() {
    let mut app = scanning_dialog("/lib");
    app.handle_enable_version_control_items_scanned(
        std::path::Path::new("/other"),
        Ok(vec![row("library.toml")]),
    );
    let state = app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("dialog");
    assert!(state.scanning && state.items.is_empty());
    app.ui_state.enable_version_control = None;
    app.handle_enable_version_control_items_scanned(
        std::path::Path::new("/lib"),
        Ok(vec![row("library.toml")]),
    );
    assert!(app.ui_state.enable_version_control.is_none());
}

#[test]
fn a_failed_listing_is_reported_in_the_dialog_and_the_messages_panel() {
    let _ = crate::diagnostics::init_logging();
    let mut app = scanning_dialog("/lib");
    let since = crate::diagnostics::newest_entry_id();
    app.handle_enable_version_control_items_scanned(
        std::path::Path::new("/lib"),
        Err("the background task failed".into()),
    );
    let state = app.ui_state.enable_version_control.expect("dialog");
    assert!(!state.scanning);
    assert!(state.error.is_some_and(|e| e.contains("Could not read")));
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| Some(e.id) > since && e.message.contains("Could not read the library folder"))
    );
}

#[test]
fn confirm_and_edits_wait_for_the_rows() {
    let mut app = scanning_dialog("/lib");
    let task = app.handle_enable_version_control_confirm();
    let state = app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("dialog");
    assert!(!state.working, "no run starts before the rows arrive");
    drop(task);
    let _ = app.update(crate::app::Message::EnableVersionControl(
        crate::app::EnableVersionControlMsg::ToggleLfs,
    ));
    let state = app
        .ui_state
        .enable_version_control
        .as_ref()
        .expect("dialog");
    assert!(!state.use_lfs, "edits are ignored while the rows load");
}
