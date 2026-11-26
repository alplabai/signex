//! The measurement commands that write, with the notebook tab open (mockup
//! row 199): an edit of the notebook file goes into the tab and Save keeps
//! it, an edit of a chapter is refused, and so is a notebook a newer Signex
//! wrote. The exports' files are checked.

use std::path::Path;

use super::tests::{answer, project, refusal, run};
use crate::app::notebook_tab::NotebookMsg;
use crate::app::{Message, Signex};
use crate::task_driver::TaskDriver;

/// The notebook file with a reading of its own that stores L1's value.
const MAIN_READING: &str = "+++\nformat = \"snxnbk/1\"\n+++\n#import \"@signex/notebook:0.1.0\": *\n= Bring-up\n\n#measurement(id: \"MEAS-007\", target: net(\"+3V3\"), quantity: \"ripple\", value: 20.0, unit: \"mV\", taken-with: (\"L1\": \"3.3 µH\"))\n\n#include \"chapters/bench.snxnbk\"\n";

fn send(app: &mut Signex, msg: NotebookMsg) {
    TaskDriver::new()
        .send(app, Message::from(msg))
        .expect("settled");
}

fn open_tab(app: &mut Signex) -> crate::app::state::ProjectId {
    let id = app.document_state.projects[0].id;
    send(app, NotebookMsg::OpenRequested { project: Some(id) });
    assert!(app.notebook_tab_file_text(id).is_some(), "the tab holds it");
    id
}

fn on_disk(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("notebook/design.snxnbk")).expect("the notebook")
}

#[test]
fn with_the_tab_open_an_attachment_goes_into_the_tab_and_save_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let id = open_tab(&mut app);
    let outside = tempfile::tempdir().unwrap();
    let capture = outside.path().join("ripple.csv");
    std::fs::write(&capture, "t,v\n0,1\n").expect("the capture");
    let path = format!("path={}", capture.display());
    let done = answer(run(
        &mut app,
        &project,
        "attach_notebook_file",
        &[path.as_str(), "part=C15"],
    ));
    assert_eq!(done["in_tab"], true);
    let call = done["call"].as_str().expect("the call").to_string();
    assert!(
        !on_disk(dir.path()).contains(&call),
        "not written behind the tab"
    );
    let held = app.notebook_tab_file_text(id).expect("the tab's text");
    assert!(held.contains(&call), "{held}");
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    assert!(on_disk(dir.path()).contains(&call), "Save wrote it");
}

#[test]
fn with_the_tab_open_mark_still_valid_goes_into_the_tab_and_save_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    std::fs::write(dir.path().join("notebook/design.snxnbk"), MAIN_READING).unwrap();
    let id = open_tab(&mut app);
    let marked = answer(run(
        &mut app,
        &project,
        "mark_notebook_attachment_valid",
        &["id=MEAS-007"],
    ));
    assert_eq!(marked["in_tab"], true);
    assert!(
        on_disk(dir.path()).contains("\"3.3 µH\""),
        "not written behind it"
    );
    let held = app.notebook_tab_file_text(id).expect("the tab's text");
    assert!(held.contains("taken-with: (\"L1\": \"4.7 µH\")"), "{held}");
    send(&mut app, NotebookMsg::SavePressed { project_id: id });
    assert!(on_disk(dir.path()).contains("taken-with: (\"L1\": \"4.7 µH\")"));
}

#[test]
fn with_the_tab_open_a_chapter_is_not_written_behind_it() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let _id = open_tab(&mut app);
    let chapter = dir.path().join("notebook/chapters/bench.snxnbk");
    let before = std::fs::read_to_string(&chapter).unwrap();
    let why = refusal(run(
        &mut app,
        &project,
        "mark_notebook_attachment_valid",
        &["id=MEAS-005"],
    ));
    assert!(why.contains("chapters/bench.snxnbk"), "{why}");
    assert!(why.contains("open in its tab"), "{why}");
    assert_eq!(std::fs::read_to_string(&chapter).unwrap(), before);
}

#[test]
fn a_notebook_a_newer_signex_wrote_is_not_edited() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let newer = MAIN_READING.replace("snxnbk/1\"", "snxnbk/1.9\"");
    std::fs::write(dir.path().join("notebook/design.snxnbk"), &newer).unwrap();
    let why = refusal(run(
        &mut app,
        &project,
        "mark_notebook_attachment_valid",
        &["id=MEAS-007"],
    ));
    assert!(why.contains("read-only"), "{why}");
    assert_eq!(on_disk(dir.path()), newer);
}

#[test]
fn the_timeline_csv_must_be_a_new_csv() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, project) = project(&dir);
    let at = |name: &str| format!("path={}", dir.path().join(name).display());
    let export = |app: &mut Signex, extra: &[&str]| {
        let mut operands = vec!["net=+3V3"];
        operands.extend_from_slice(extra);
        run(app, &project, "export_notebook_timeline", &operands)
    };
    let not_csv = refusal(export(&mut app, &[at("ripple.txt").as_str()]));
    assert!(not_csv.contains(".csv"), "{not_csv}");
    let snx = refusal(export(&mut app, &[at("top.snxsch").as_str()]));
    assert!(snx.contains("Signex document"), "{snx}");
    let target = at("out/ripple.csv");
    let done = answer(export(&mut app, &[target.as_str()]));
    let written = dir.path().join("out/ripple.csv");
    assert_eq!(done["path"], written.display().to_string());
    assert!(written.exists(), "its folder is made");
    let exists = refusal(export(&mut app, &[target.as_str()]));
    assert!(exists.contains("overwrite=true"), "{exists}");
    let again = answer(export(&mut app, &[target.as_str(), "overwrite=true"]));
    assert_eq!(again["readings"], 2);
}
