//! A headless run never opens, creates or commits to a git repository (#1923);
//! a GUI app still queues the commit.

use signex_app::app::{LoadedProject, Signex};
use signex_app::diagnostics;
use signex_app::io_ports::{GitCommits, IoPorts};
use signex_types::project::ProjectData;
use std::path::Path;
use tempfile::TempDir;

fn push_git_project(app: &mut Signex, dir: &Path) {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("p.snxprj"),
        data: ProjectData {
            document_id: None,
            description: None,
            name: "p".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: true,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            schematic_defaults: Default::default(),
            project_defaults: Default::default(),
            parameters: Default::default(),
            unknown: Default::default(),
        },
        pending_libraries: std::collections::HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
}

#[test]
fn the_native_ports_commit_and_the_headless_ones_skip_with_a_message() {
    diagnostics::init_logging().ok();
    assert_eq!(IoPorts::default().git, GitCommits::Native);
    assert_eq!(IoPorts::headless(Default::default()).git, GitCommits::Skip);

    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("a.snxsch");

    let (mut gui, _task) = Signex::new();
    push_git_project(&mut gui, tmp.path());
    gui.commit_save_to_project_git(&file, "m");
    assert_eq!(
        gui.document_state.pending_git_commits.len(),
        1,
        "the GUI path commits"
    );

    let (mut headless, _task) = Signex::new();
    push_git_project(&mut headless, tmp.path());
    headless.ui_state.io = IoPorts::headless(Default::default());
    let mark = diagnostics::newest_entry_id();
    headless.commit_save_to_project_git(&file, "m");
    assert!(headless.document_state.pending_git_commits.is_empty());
    assert!(headless.document_state.inflight_git_commits.is_empty());
    assert!(
        diagnostics::entries_since(mark)
            .entries
            .iter()
            .any(|e| e.message.contains("commit skipped in a headless run")),
        "the Messages log says the commit was skipped"
    );
    assert!(
        !tmp.path().join(".git").exists(),
        "no repository was created"
    );
}
