//! #2908: a newer-minor project's copy never drops a key of its `.snxprj`
//! silently. This build reads `fab_outputs` into a struct with no bag for
//! keys it does not know, so an unknown field there would be lost: the Save
//! stops before writing, the dialog names the key, and only the person's
//! agreement writes the copy.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use signex_types::project::parse_project;

use super::SaveProjectAsMsg;
use super::plan::plan_copy;
use super::tests::{newer_project, snapshot};
use super::write::{CopyOutcome, write_copy};
use crate::app::Message;
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::new_project::TargetStatus;
use crate::persist::run_task::outputs;

/// The key the copy cannot keep, as the dialog names it.
const LOST: &str = "fab_outputs.laser_marking";

/// `newer_project` with a `fab_outputs` object carrying a field this build
/// does not know. Returns the project folder.
fn newer_project_with_unknown_fab_field(root: &Path) -> PathBuf {
    let (dir, on_disk) = newer_project(root);
    let mut value: Value = serde_json::from_str(&on_disk).expect("json");
    value.as_object_mut().expect("an object").insert(
        "fab_outputs".into(),
        json!({"gerber": false, "laser_marking": true}),
    );
    let text = serde_json::to_string_pretty(&value).expect("json");
    fs::write(dir.join("P.snxprj"), text).expect("write");
    dir
}

#[test]
fn an_unknown_fab_outputs_field_stops_the_copy_until_it_is_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = newer_project_with_unknown_fab_field(tmp.path());
    let before = snapshot(&dir);
    let data = parse_project(&dir.join("P.snxprj")).expect("parse");
    assert!(data.is_from_newer_minor());
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");

    let mut plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out).expect("plan");
    assert_eq!(
        write_copy(&plan).expect("checked"),
        CopyOutcome::NotKept(vec![LOST.to_string()])
    );
    assert!(!out.join("Copy").exists(), "nothing is written before");

    plan.keys_check
        .as_mut()
        .expect("a newer minor is checked")
        .accepted = vec![LOST.into()];
    let CopyOutcome::Written(written) = write_copy(&plan).expect("the copy") else {
        panic!("the one key it drops was accepted");
    };
    let copy: Value = serde_json::from_slice(&fs::read(&written).expect("read")).expect("json");
    assert_eq!(
        copy["fab_outputs"]["gerber"],
        json!(false),
        "what it reads is kept"
    );
    assert!(copy["fab_outputs"].get("laser_marking").is_none());
    assert_eq!(snapshot(&dir), before, "the original folder is untouched");
}

#[test]
fn a_project_this_build_reads_whole_is_not_checked() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, _) = newer_project(tmp.path());
    let mut data = parse_project(&dir.join("P.snxprj")).expect("parse");
    data.unknown.newer_minor_source = None;
    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", tmp.path()).expect("plan");
    assert!(plan.keys_check.is_none());
}

/// The dialog: Save names the key and writes nothing; Save copy without them
/// writes the copy and opens it.
#[test]
fn the_dialog_names_the_key_and_writes_only_once_it_is_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = newer_project_with_unknown_fab_field(tmp.path());
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");
    let mut app = app_workspace(&dir.to_string_lossy(), &[]);
    let project = &mut app.document_state.projects[0];
    project.path = dir.join("P.snxprj");
    project.data = parse_project(&project.path).expect("parse");
    let id = project.id;
    let _ = app.update(Message::from(SaveProjectAsMsg::Requested {
        project: Some(id),
    }));
    let _ = app.update(Message::from(SaveProjectAsMsg::LocationEdited(
        out.display().to_string(),
    )));
    let generation = app
        .ui_state
        .save_project_as
        .as_deref()
        .expect("dialog")
        .generation;
    let _ = app.update(Message::from(SaveProjectAsMsg::TargetChecked {
        generation,
        status: TargetStatus::Clear,
    }));

    let task = app.update(Message::from(SaveProjectAsMsg::SubmitRequested));
    for message in outputs(task) {
        let _ = app.update(message);
    }
    let dialog = app
        .ui_state
        .save_project_as
        .as_deref()
        .expect("the dialog stays");
    assert_eq!(dialog.not_kept.as_deref(), Some(&[LOST.to_string()][..]));
    assert!(dialog.saving.is_none());
    assert!(dialog.can_save(), "the person can go on");
    assert!(!out.join("P copy").exists(), "nothing was written");

    // Enter in a field is a plain Save: it asks again, it does not agree.
    let task = app.update(Message::from(SaveProjectAsMsg::SubmitRequested));
    for message in outputs(task) {
        let _ = app.update(message);
    }
    assert!(!out.join("P copy").exists(), "a plain Save never agrees");

    let task = app.update(Message::from(SaveProjectAsMsg::KeysNotKeptAccepted));
    for message in outputs(task) {
        let _ = app.update(message);
    }
    assert!(app.ui_state.save_project_as.is_none(), "the dialog closes");
    assert!(out.join("P copy").join("P copy.snxprj").is_file());
}
