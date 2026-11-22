//! m39, the History panel: Restore this version asks first over unsaved
//! edits, Cancel keeps them, and a revision that is not there changes
//! nothing.

use std::path::Path;

use signex_app::app::{LoadedProject, ProjectId};
use signex_app::panels::PanelMsg;
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_library::{Footprint, FootprintFile};
use signex_types::project::ProjectData;

use super::support::{FpRig, open_footprint};
use crate::settle_support::list_active_history;

const COMMITTED: &str = "the committed description";
const UNSAVED: &str = "an unsaved description";

fn project(dir: &Path) -> ProjectData {
    ProjectData {
        document_id: None,
        description: None,
        name: "P".into(),
        dir: dir.to_string_lossy().into_owned(),
        schematic_root: None,
        pcb_file: None,
        sheets: Vec::new(),
        variant_definitions: Vec::new(),
        active_variant: None,
        libraries: Vec::new(),
        enable_git: false,
        rules: Default::default(),
        net_classes: Default::default(),
        annotation: None,
        schematic_defaults: Default::default(),
        project_defaults: Default::default(),
        agent_journal: Default::default(),
        schematic_design: Default::default(),
        fab_outputs: None,
        router_tool_diameter: None,
        requirements: Default::default(),
        parameters: Default::default(),
        unknown: Default::default(),
    }
}

/// A footprint committed to its project's repository; the sha of that commit.
fn committed() -> (FpRig, String) {
    signex_library::git_identity::test_identity::install();
    let mut footprint = Footprint::empty("history");
    footprint.description = COMMITTED.into();
    let text = FootprintFile::from_footprint(footprint.clone())
        .to_toml_string()
        .expect("serialise");
    let mut rig = open_footprint(footprint);
    std::fs::write(&rig.path, &text).expect("write the file");
    let dir = rig.path.parent().expect("a folder").to_path_buf();
    let adapter = LocalGitProjectAdapter::open_or_init(dir.clone()).expect("a repository");
    let sha = adapter
        .commit_path(Path::new("audit.snxfpt"), "first")
        .expect("commit")
        .expect("a commit")
        .to_string();
    let id: ProjectId = rig.app.document_state.mint_project_id();
    rig.app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: project(&dir),
        pending_libraries: Default::default(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
    // The first update makes the panel adopt the tab, which drops any armed
    // confirmation; let that happen before a test clicks.
    rig.panel(PanelMsg::HistoryRestoreCancelled);
    (rig, sha)
}

fn restore(rig: &mut FpRig, sha: &str) {
    list_active_history(&mut rig.app);
    rig.panel(PanelMsg::HistoryRestoreClicked { sha: sha.into() });
}

fn description(rig: &FpRig) -> String {
    rig.editor().primitive().description.clone()
}

fn armed(rig: &FpRig) -> Option<String> {
    rig.app.document_state.history.pending_restore.clone()
}

#[test]
fn m39_restore_over_unsaved_edits_asks_first_and_changes_nothing_yet() {
    let (mut rig, sha) = committed();
    rig.panel(PanelMsg::FpEditorSetFootprintDescription(UNSAVED.into()));

    restore(&mut rig, &sha);

    assert_eq!(armed(&rig).as_deref(), Some(sha.as_str()));
    assert_eq!(description(&rig), UNSAVED, "the edit is still there");
    assert!(rig.dirty());
}

#[test]
fn m39_cancel_on_the_restore_prompt_keeps_the_unsaved_edits() {
    let (mut rig, sha) = committed();
    rig.panel(PanelMsg::FpEditorSetFootprintDescription(UNSAVED.into()));
    restore(&mut rig, &sha);

    rig.panel(PanelMsg::HistoryRestoreCancelled);

    assert_eq!(armed(&rig), None);
    assert_eq!(description(&rig), UNSAVED);
    assert!(rig.dirty(), "Cancel does not clear the unsaved mark");
}

#[test]
fn m39_discard_and_restore_brings_the_committed_version_into_the_editor() {
    let (mut rig, sha) = committed();
    rig.panel(PanelMsg::FpEditorSetFootprintDescription(UNSAVED.into()));
    restore(&mut rig, &sha);

    restore(&mut rig, &sha);

    assert_eq!(armed(&rig), None, "the restore ran");
    assert_eq!(description(&rig), COMMITTED);
}

#[test]
fn m39_restoring_a_revision_that_is_not_there_changes_nothing() {
    let (mut rig, _sha) = committed();
    let on_disk = std::fs::read_to_string(&rig.path).expect("read");

    restore(&mut rig, "0123456789012345678901234567890123456789");

    assert_eq!(description(&rig), COMMITTED);
    assert!(!rig.dirty());
    assert_eq!(
        std::fs::read_to_string(&rig.path).expect("read"),
        on_disk,
        "the file is untouched"
    );
}
