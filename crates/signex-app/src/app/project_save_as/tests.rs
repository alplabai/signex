//! File ▸ Save Project As… (#2908): the copy holds every document the project
//! lists byte for byte and the project file in this build's format, the
//! original folder is never written, a target folder that is not empty is
//! refused, and a read-only project's refusals offer it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use signex_types::project::{SNXPRJ_FORMAT_V1, encode_project, parse_project};

use super::SaveProjectAsMsg;
use super::plan::{listed, plan_copy};
use super::write::{CopyOutcome, write_copy};
use crate::app::handlers::menu::export::tests::app_workspace;
use crate::app::new_project::{TargetProblem, TargetStatus, check_target};
use crate::app::state::NoticeAction;
use crate::app::{
    AppQuitConfirmState, Message, ProjectCloseChoice, ProjectCloseConfirmState, ProjectMsg,
};
use crate::persist::run_task::outputs;

/// A minor this build does not know: the project opens read-only.
pub(super) const NEWER: &str = "snxprj/1.9";

/// The documents the fixture project lists, below its folder.
const LISTED: [&str; 7] = [
    "P.snxsch",
    "sub/child.snxsch",
    "P.snxpcb",
    "libs/L.snxlib",
    "libs/symbols/r.snxsym",
    "libs/3dmodels/r.step",
    "libs/assets/datasheets/r.pdf",
];

pub(super) fn put(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("folder");
    fs::write(path, bytes).expect("write");
}

/// Every file under `dir` with its bytes, keyed by its path below `dir`.
pub(super) fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("read folder") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path.strip_prefix(root).expect("below root").to_path_buf();
                out.insert(rel, fs::read(&path).expect("read"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn json(path: &Path) -> String {
    serde_json::to_string(&path.to_string_lossy()).expect("a JSON string")
}

/// Project `P` in `<root>/P`, written by a newer build (`NEWER`) with a key
/// this build does not know: a root and a child sheet, a board and a project
/// library with its folders, and a file it does not list. Returns its folder
/// and the `.snxprj` bytes on disk.
pub(super) fn newer_project(root: &Path) -> (PathBuf, String) {
    let dir = root.join("P");
    put(&dir.join("P.snxsch"), b"root sheet\n");
    put(&dir.join("sub/child.snxsch"), b"child\tsheet\r\n");
    put(&dir.join("P.snxpcb"), b"board");
    put(&dir.join("libs/L.snxlib"), b"library manifest");
    put(&dir.join("libs/symbols/r.snxsym"), b"symbol");
    put(&dir.join("libs/3dmodels/r.step"), &[0, 1, 2, 255]);
    put(&dir.join("libs/assets/datasheets/r.pdf"), b"%PDF-1.7");
    put(&dir.join("notes.txt"), b"not listed");
    let written = serde_json::json!({
        "format": SNXPRJ_FORMAT_V1,
        "name": "P",
        "dir": dir.to_string_lossy(),
        "schematic_root": "P.snxsch",
        "pcb_file": "P.snxpcb",
        "sheets": [
            {"name": "Root", "filename": "P.snxsch"},
            {"name": "Child", "filename": "sub/child.snxsch"}
        ],
        "libraries": [{"path": "libs/L.snxlib", "kind": "project_local"}],
        "future_setting": {"kept": true}
    });
    let prj = dir.join("P.snxprj");
    put(&prj, &serde_json::to_vec(&written).expect("json"));
    // The bytes the writer makes, then stamped with the newer minor.
    let encoded = encode_project(&parse_project(&prj).expect("parse")).expect("encode");
    let newer = String::from_utf8(encoded).expect("utf-8").replace(
        &format!("\n  \"format\": \"{SNXPRJ_FORMAT_V1}\","),
        &format!("\n  \"format\": \"{NEWER}\","),
    );
    assert!(newer.contains(NEWER), "{newer}");
    put(&prj, newer.as_bytes());
    (dir, newer)
}

#[test]
fn the_copy_holds_every_listed_document_byte_for_byte_and_the_project_in_the_current_format() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, on_disk) = newer_project(tmp.path());
    let before = snapshot(&dir);
    let data = parse_project(&dir.join("P.snxprj")).expect("parse");
    assert!(data.is_from_newer_minor());
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");

    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out).expect("plan");
    let CopyOutcome::Written(written) = write_copy(&plan).expect("the copy") else {
        panic!("the copy keeps every key, so it is written");
    };

    let copy = out.join("Copy");
    assert_eq!(written, copy.join("Copy.snxprj"));
    for rel in LISTED {
        assert_eq!(
            fs::read(copy.join(rel)).expect(rel),
            before[Path::new(rel)],
            "{rel}"
        );
    }
    assert!(
        !copy.join("notes.txt").exists(),
        "only what the project lists"
    );
    // The project file: the same bytes, but for this build's token and the
    // copy's name and folder.
    let expected = on_disk
        .replace(
            &format!("\n  \"format\": \"{NEWER}\","),
            &format!("\n  \"format\": \"{SNXPRJ_FORMAT_V1}\","),
        )
        .replace("\n  \"name\": \"P\",", "\n  \"name\": \"Copy\",")
        .replace(
            &format!("\n  \"dir\": {},", json(&dir)),
            &format!("\n  \"dir\": {},", json(&copy)),
        );
    assert_eq!(fs::read_to_string(&written).expect("copy"), expected);
    let reread = parse_project(&written).expect("the copy parses");
    assert!(!reread.is_from_newer_minor(), "the copy can be edited");
    assert_eq!(
        reread.unknown.get("future_setting"),
        Some(&serde_json::json!({"kept": true})),
        "a newer build's key is kept"
    );
    assert_eq!(snapshot(&dir), before, "the original folder is untouched");
}

#[test]
fn a_target_folder_that_exists_and_is_not_empty_is_refused_and_nothing_is_written() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, _) = newer_project(tmp.path());
    let before = snapshot(&dir);
    let out = tmp.path().join("out");
    let taken = out.join("Copy");
    put(&taken.join("keep.txt"), b"mine");
    let data = parse_project(&dir.join("P.snxprj")).expect("parse");

    let status = check_target("Copy", &out.to_string_lossy(), true);
    assert!(
        matches!(&status, TargetStatus::Problem(TargetProblem::FolderNotEmpty(path)) if *path == taken),
        "the dialog refuses it before Save: {status:?}"
    );
    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out).expect("plan");
    let why = write_copy(&plan).expect_err("refused");
    assert!(why.contains("not empty"), "{why}");
    assert_eq!(snapshot(&taken).len(), 1, "nothing was added");
    assert_eq!(fs::read(taken.join("keep.txt")).expect("kept"), b"mine");
    assert_eq!(snapshot(&dir), before);
}

#[test]
fn the_copy_is_never_written_inside_the_original_folder() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, _) = newer_project(tmp.path());
    let before = snapshot(&dir);
    let data = parse_project(&dir.join("P.snxprj")).expect("parse");
    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &dir).expect("plan");
    let why = write_copy(&plan).expect_err("refused");
    assert!(why.contains("inside"), "{why}");
    assert_eq!(snapshot(&dir), before);
}

#[test]
fn a_listed_file_missing_on_disk_refuses_the_copy_before_anything_is_written() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, _) = newer_project(tmp.path());
    fs::remove_file(dir.join("P.snxpcb")).expect("remove");
    let data = parse_project(&dir.join("P.snxprj")).expect("parse");
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");
    let plan = plan_copy(&data, &dir.join("P.snxprj"), "Copy", &out).expect("plan");
    let why = write_copy(&plan).expect_err("refused");
    assert!(
        why.contains("P.snxpcb") && why.contains("not on disk"),
        "{why}"
    );
    assert!(!out.join("Copy").exists(), "no folder was made");
}

#[test]
fn an_entry_outside_the_project_folder_is_refused_when_planned() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (dir, _) = newer_project(tmp.path());
    let mut data = parse_project(&dir.join("P.snxprj")).expect("parse");
    data.pcb_file = Some("../elsewhere.snxpcb".into());
    let why = listed(&data).expect_err("refused");
    assert!(why.contains("../elsewhere.snxpcb"), "{why}");
}

/// The read-only project of `app_workspace`, read as `snxprj/1.2`, with
/// unsaved settings.
fn read_only_app() -> (crate::app::Signex, crate::app::ProjectId, PathBuf) {
    let mut app = app_workspace("/ro", &[]);
    let project = &mut app.document_state.projects[0];
    project.data.unknown.newer_minor_source = Some("snxprj/1.2".into());
    let (id, path) = (project.id, project.path.clone());
    app.document_state.mark_document_dirty(&path);
    (app, id, path)
}

#[test]
fn a_read_only_save_refusal_offers_save_project_as_and_it_opens_on_that_project() {
    let (mut app, id, _) = read_only_app();
    let Err(error) = app.begin_project_save(id) else {
        panic!("a read-only project's save is refused");
    };
    app.report_save_failure("Failed to save project file", &error);
    let notice = app.document_state.error_notice.as_ref().expect("card");
    assert_eq!(
        notice.action,
        Some(NoticeAction::SaveProjectAs { project: id })
    );
    let _ = app.update(Message::from(SaveProjectAsMsg::Requested {
        project: Some(id),
    }));
    assert!(app.document_state.error_notice.is_none(), "the card closes");
    let dialog = app.ui_state.save_project_as.as_deref().expect("dialog");
    assert_eq!(dialog.project, id);
    assert_eq!(dialog.read_only_format.as_deref(), Some("snxprj/1.2"));
    assert_eq!(dialog.name, "ProjectA copy");
}

#[test]
fn any_other_save_failure_offers_nothing() {
    let (app, _, _) = read_only_app();
    let notice = crate::app::read_only_project::save_failure_notice(
        &app.document_state.projects,
        "Failed to save active document",
        &anyhow::anyhow!("disk full"),
    );
    assert_eq!(notice.action, None);
}

#[test]
fn save_project_as_in_the_close_prompt_closes_it_and_opens_the_dialog() {
    let (mut app, id, path) = read_only_app();
    app.ui_state.project_close_confirm = Some(Box::new(ProjectCloseConfirmState {
        project_id: id,
        project_name: "ProjectA".into(),
        dirty_paths: vec![path.clone()],
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs: Default::default(),
    }));
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveProjectAs,
    )));
    assert!(app.ui_state.project_close_confirm.is_none());
    assert_eq!(
        app.ui_state.save_project_as.as_deref().map(|s| s.project),
        Some(id)
    );
    assert!(
        app.document_state.is_document_dirty(&path),
        "nothing is discarded"
    );
    assert_eq!(app.document_state.projects.len(), 1, "nothing is closed");
}

#[test]
fn save_project_as_in_the_exit_prompt_declines_the_exit_and_opens_the_dialog() {
    let (mut app, id, path) = read_only_app();
    app.ui_state.app_quit_confirm = Some(AppQuitConfirmState {
        dirty_paths: vec![path.clone()],
        dirty_rows: Vec::new(),
    });
    let _ = app.update(Message::Project(ProjectMsg::AppQuitConfirm(
        ProjectCloseChoice::SaveProjectAs,
    )));
    assert!(app.ui_state.app_quit_confirm.is_none());
    assert_eq!(
        app.ui_state.save_project_as.as_deref().map(|s| s.project),
        Some(id)
    );
    assert!(
        app.document_state.is_document_dirty(&path),
        "nothing is discarded"
    );
}

#[test]
fn a_forced_save_project_as_with_nothing_read_only_leaves_the_prompt() {
    let mut app = app_workspace("/rw", &["a.snxsch"]);
    let id = app.document_state.projects[0].id;
    let sheet = PathBuf::from("/rw/a.snxsch");
    app.ui_state.project_close_confirm = Some(Box::new(ProjectCloseConfirmState {
        project_id: id,
        project_name: "ProjectA".into(),
        dirty_paths: vec![sheet],
        dirty_rows: Vec::new(),
        kept_paths: Vec::new(),
        sheet_refs: Default::default(),
    }));
    let _ = app.update(Message::Project(ProjectMsg::CloseConfirm(
        ProjectCloseChoice::SaveProjectAs,
    )));
    assert!(
        app.ui_state.project_close_confirm.is_some(),
        "the prompt stays"
    );
    assert!(app.ui_state.save_project_as.is_none());
}

#[test]
fn save_copy_writes_the_copy_and_opens_it_beside_the_project() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = tmp.path().join("ProjectA");
    put(&dir.join("a.snxsch"), b"sheet bytes");
    let out = tmp.path().join("out");
    fs::create_dir(&out).expect("out");
    let mut app = app_workspace(&dir.to_string_lossy(), &["a.snxsch"]);
    let id = app.document_state.projects[0].id;
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
    let copy = out.join("ProjectA copy");
    assert_eq!(
        fs::read(copy.join("a.snxsch")).expect("sheet"),
        b"sheet bytes"
    );
    assert!(copy.join("ProjectA copy.snxprj").is_file());
    assert!(app.ui_state.save_project_as.is_none(), "the dialog closes");
    assert!(
        app.document_state
            .projects
            .iter()
            .any(|p| p.path == copy.join("ProjectA copy.snxprj")),
        "the copy opens"
    );
    assert_eq!(app.document_state.projects.len(), 2, "beside the project");
    assert_eq!(snapshot(&dir).len(), 1, "the original folder is untouched");
}
