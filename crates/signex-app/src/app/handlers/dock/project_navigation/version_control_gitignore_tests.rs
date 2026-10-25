//! #1117 — Enable Version Control merges the managed `.gitignore` block
//! (plus the unticked picker rows, outside it) into the project's file.

use super::*;
use signex_types::managed_block::{BEGIN_MARKER, END_MARKER};

fn dialog_with_unticked_row(project_dir: &std::path::Path) -> Signex {
    // SAFETY: the same values every time; no test in this binary relies on
    // an unset git identity.
    unsafe {
        std::env::set_var("GIT_AUTHOR_NAME", "Ada Lovelace");
        std::env::set_var("GIT_AUTHOR_EMAIL", "ada@example.org");
    }
    let (mut app, _task) = Signex::new();
    app.ui_state.enable_version_control = Some(crate::app::EnableVersionControlState {
        scope: crate::app::VersionControlScope::Project,
        project_path: project_dir.join("demo.snxprj"),
        project_dir: project_dir.to_path_buf(),
        project_name: "demo".to_string(),
        items: vec![crate::app::TrackItem {
            absolute: project_dir.join("scratch"),
            relative: "scratch".to_string(),
            label: "Folder".to_string(),
            is_directory: true,
            tracked: false,
        }],
        use_lfs: false,
        intro_text: String::new(),
        error: None,
    });
    app
}

fn project_dir() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("demo.snxprj"), "x").unwrap();
    tmp
}

#[test]
fn without_a_gitignore_it_holds_the_block_then_the_unticked_rows() {
    let tmp = project_dir();
    let mut app = dialog_with_unticked_row(tmp.path());

    app.handle_enable_version_control_confirm();

    let text = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(text.starts_with(BEGIN_MARKER), "{text}");
    let after_block = &text[text.find(END_MARKER).unwrap()..];
    assert!(after_block.contains("scratch/\n"), "{text}");
    assert!(!text[..text.find(END_MARKER).unwrap()].contains("scratch"));
}

#[test]
fn with_a_gitignore_the_user_lines_survive_and_the_rows_stay_outside_the_block() {
    let tmp = project_dir();
    std::fs::write(tmp.path().join(".gitignore"), "mine\n*.log\n").unwrap();
    let mut app = dialog_with_unticked_row(tmp.path());

    app.handle_enable_version_control_confirm();

    let text = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
    assert!(text.starts_with("mine\n*.log\n"), "{text}");
    assert_eq!(text.matches(BEGIN_MARKER).count(), 1);
    assert!(text.contains("scratch/\n"));
}

#[test]
fn unbalanced_markers_keep_the_file_and_the_failure_reaches_the_log() {
    let _ = crate::diagnostics::init_logging();
    let tmp = project_dir();
    let broken = format!("mine\n{BEGIN_MARKER}\nno end\n");
    std::fs::write(tmp.path().join(".gitignore"), &broken).unwrap();
    let mut app = dialog_with_unticked_row(tmp.path());
    let marker = tmp.path().display().to_string();

    app.handle_enable_version_control_confirm();

    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        broken
    );
    assert!(
        app.ui_state
            .enable_version_control
            .as_ref()
            .is_some_and(|s| s.error.is_some()),
        "the dialog stays open with the reason"
    );
    assert!(
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains(&marker)
                && e.message.contains("enable version control failed")),
        "the failure must reach the Messages panel"
    );
}
