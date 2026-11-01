//! #1824: Save As of a project sheet into the project folder re-points the
//! project at the new file and writes the `.snxprj`; the old file stays.

use std::path::{Path, PathBuf};

use signex_types::format::SnxSchematic;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, sheet_with_net};
use crate::app::path_ops::settle_task;

struct Fx {
    app: Signex,
    top: PathBuf,
    child: PathBuf,
    project: PathBuf,
}

fn write_sheet(path: &Path, children: &[&str], reference: &str) {
    let text = SnxSchematic::new(sheet_with_net(reference, "N", children))
        .write_string()
        .expect("encode");
    std::fs::write(path, text).expect("write sheet");
}

/// `top.snxsch` (root) places `child.snxsch`; both are listed pages and both
/// are on disk, as is the project file; `top` is open and active.
fn fixture(dir: &Path) -> Fx {
    let (top, child) = (dir.join("top.snxsch"), dir.join("child.snxsch"));
    write_sheet(&top, &["child.snxsch"], "R1");
    write_sheet(&child, &[], "R2");
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "child.snxsch"]);
    let project = app.document_state.projects[0].path.clone();
    let bytes = signex_types::project::encode_project(&app.document_state.projects[0].data)
        .expect("encode project");
    std::fs::write(&project, bytes).expect("write project");
    app.open_or_focus_sheet_path(top.clone());
    Fx {
        app,
        top,
        child,
        project,
    }
}

/// An edit that makes the open sheet differ from its file.
fn edit(app: &mut Signex, path: &Path) {
    let mut label = app.document_state.engines[path].document().labels[0].clone();
    label.uuid = uuid::Uuid::now_v7();
    label.text = "EDITED".into();
    assert!(app.apply_engine_command(signex_engine::Command::PlaceLabel { label }, true, true,));
}

fn save_as(app: &mut Signex, from: &Path, to: PathBuf) {
    let task = app
        .save_active_document_as(from, to)
        .expect("Save As starts");
    settle_task(app, task);
}

/// What reopening the project would show.
fn reopened(project: &Path) -> signex_types::project::ProjectData {
    signex_types::project::parse_project(project).expect("project parses")
}

fn listed(data: &signex_types::project::ProjectData) -> Vec<String> {
    data.sheets.iter().map(|s| s.filename.clone()).collect()
}

#[test]
fn save_as_of_the_root_sheet_inside_the_project_re_points_the_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    let old_bytes = std::fs::read(&fx.top).unwrap();
    edit(&mut fx.app, &fx.top.clone());
    let to = dir.path().join("Renamed Güç.snxsch");

    save_as(&mut fx.app, &fx.top.clone(), to.clone());

    let data = reopened(&fx.project);
    assert_eq!(listed(&data), ["Renamed Güç.snxsch", "child.snxsch"]);
    assert_eq!(data.schematic_root.as_deref(), Some("Renamed Güç.snxsch"));
    assert_eq!(std::fs::read(&fx.top).unwrap(), old_bytes, "old file");
    assert!(std::fs::read_to_string(&to).unwrap().contains("EDITED"));
    assert!(
        !fx.app.document_state.is_document_dirty(&fx.project),
        "the project file is patched on disk, not left dirty"
    );
}

fn child_files(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).expect("read sheet");
    SnxSchematic::parse(&text)
        .expect("parse sheet")
        .sheet
        .child_sheets
        .iter()
        .map(|c| c.filename.clone())
        .collect()
}

#[test]
fn save_as_of_a_child_sheet_re_points_the_open_parent_on_disk_and_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    fx.app.open_or_focus_sheet_path(fx.child.clone());
    let to = dir.path().join("kid.snxsch");

    save_as(&mut fx.app, &fx.child.clone(), to.clone());

    assert_eq!(listed(&reopened(&fx.project)), ["top.snxsch", "kid.snxsch"]);
    assert_eq!(
        child_files(&fx.top),
        ["kid.snxsch"],
        "Discard All must not leave the parent on the old file (#1849)"
    );
    let parent = fx.app.document_state.engines[&fx.top].document();
    assert_eq!(parent.child_sheets[0].filename, "kid.snxsch");
    assert!(
        !fx.app.document_state.is_document_dirty(&fx.top),
        "memory equals disk, so the parent has nothing to save"
    );
    assert!(fx.child.exists(), "old file stays on disk");
}

#[test]
fn save_as_of_a_child_sheet_re_points_an_unopened_parent_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    // `top` is the active tab; close it so no engine holds the parent.
    fx.app.document_state.engines.remove(&fx.top);
    fx.app.document_state.tabs.clear();
    fx.app.open_or_focus_sheet_path(fx.child.clone());
    let to = dir.path().join("kid.snxsch");

    save_as(&mut fx.app, &fx.child.clone(), to.clone());

    assert_eq!(child_files(&fx.top), ["kid.snxsch"]);
    assert!(!fx.app.document_state.engines.contains_key(&fx.top));
}

#[test]
fn save_as_does_not_write_a_parents_other_unsaved_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    let top = fx.top.clone();
    edit(&mut fx.app, &top);
    fx.app.open_or_focus_sheet_path(fx.child.clone());
    let to = dir.path().join("kid.snxsch");

    save_as(&mut fx.app, &fx.child.clone(), to.clone());

    assert_eq!(child_files(&fx.top), ["kid.snxsch"]);
    let on_disk = std::fs::read_to_string(&fx.top).unwrap();
    assert!(
        !on_disk.contains("EDITED"),
        "the unsaved label is not written"
    );
    let held = fx.app.document_state.engines[&fx.top].document();
    assert_eq!(held.child_sheets[0].filename, "kid.snxsch");
    assert!(held.labels.iter().any(|l| l.text == "EDITED"));
    assert!(fx.app.document_state.is_document_dirty(&fx.top));
}

#[test]
fn save_as_with_an_unwritable_project_leaves_the_parent_on_the_old_file_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    fx.app.open_or_focus_sheet_path(fx.child.clone());
    let to = dir.path().join("kid.snxsch");

    let done = completion(&fx, &to, Err("disk full".to_string()));
    let done = crate::app::contracts::SaveAsProjectPatched {
        from: fx.child.clone(),
        ..done
    };
    fx.app.handle_save_as_project_patched(done);

    assert_eq!(
        child_files(&fx.top),
        ["child.snxsch"],
        "disk pair unchanged"
    );
    let held = fx.app.document_state.engines[&fx.top].document();
    assert_eq!(held.child_sheets[0].filename, "kid.snxsch");
    assert!(fx.app.document_state.is_document_dirty(&fx.top));
}

/// Mark the project as holding an unsaved edit that is not a sheet entry.
fn dirty_the_project(fx: &mut Fx) {
    fx.app.document_state.projects[0].data.description = Some("UNSAVED".to_string());
    let project = fx.project.clone();
    fx.app.document_state.mark_document_dirty(&project);
}

#[test]
fn save_as_does_not_write_the_projects_unrelated_unsaved_edits() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    dirty_the_project(&mut fx);

    save_as(&mut fx.app, &fx.top.clone(), dir.path().join("new.snxsch"));

    let on_disk = reopened(&fx.project);
    assert_eq!(listed(&on_disk), ["new.snxsch", "child.snxsch"]);
    assert_eq!(on_disk.description, None, "the unsaved edit is not on disk");
    let project = fx.app.document_state.projects[0].data.clone();
    assert_eq!(project.description.as_deref(), Some("UNSAVED"));
    assert_eq!(listed(&project), ["new.snxsch", "child.snxsch"]);
    assert!(
        fx.app.document_state.is_document_dirty(&fx.project),
        "the edit is still unsaved, so Discard All reverts it"
    );
}

#[test]
fn save_as_into_a_subfolder_leaves_the_project_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let project_bytes = std::fs::read(&fx.project).unwrap();

    save_as(
        &mut fx.app,
        &fx.top.clone(),
        dir.path().join("sub/x.snxsch"),
    );

    assert!(dir.path().join("sub/x.snxsch").exists());
    assert_eq!(std::fs::read(&fx.project).unwrap(), project_bytes);
    assert_eq!(
        listed(&fx.app.document_state.projects[0].data),
        ["top.snxsch", "child.snxsch"]
    );
}

#[test]
fn save_as_while_the_project_is_being_saved_keeps_the_change_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    let project_bytes = std::fs::read(&fx.project).unwrap();
    let _running = fx
        .app
        .document_state
        .document_saves
        .begin(fx.project.clone(), Vec::new());

    save_as(&mut fx.app, &fx.top.clone(), dir.path().join("new.snxsch"));

    assert_eq!(std::fs::read(&fx.project).unwrap(), project_bytes);
    assert_eq!(
        listed(&fx.app.document_state.projects[0].data),
        ["new.snxsch", "child.snxsch"]
    );
    assert!(fx.app.document_state.is_document_dirty(&fx.project));
}

#[test]
fn save_as_outside_the_project_folder_leaves_the_project_alone() {
    let dir = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    let project_bytes = std::fs::read(&fx.project).unwrap();

    save_as(
        &mut fx.app,
        &fx.top.clone(),
        elsewhere.path().join("copy.snxsch"),
    );

    assert!(elsewhere.path().join("copy.snxsch").exists());
    assert_eq!(std::fs::read(&fx.project).unwrap(), project_bytes);
    assert_eq!(
        listed(&fx.app.document_state.projects[0].data),
        ["top.snxsch", "child.snxsch"]
    );
}

fn project_id(fx: &Fx) -> crate::app::state::ProjectId {
    fx.app.document_state.projects[0].id
}

fn completion(
    fx: &Fx,
    to: &Path,
    result: Result<crate::app::project_entry_disk::EntryPatch, String>,
) -> crate::app::contracts::SaveAsProjectPatched {
    crate::app::contracts::SaveAsProjectPatched {
        project_id: project_id(fx),
        from: fx.top.clone(),
        to: to.to_path_buf(),
        result,
        plan: Default::default(),
        parent_disk: Default::default(),
    }
}

#[test]
fn a_patch_that_did_not_reach_disk_keeps_the_change_in_memory_for_a_save() {
    use crate::app::project_entry_disk::EntryPatch;
    let results = [
        Ok(EntryPatch::NotListed),
        Ok(EntryPatch::NoProjectFile),
        Ok(EntryPatch::Unreadable("damaged".to_string())),
        Err("disk full".to_string()),
    ];
    for result in results {
        let dir = tempfile::tempdir().unwrap();
        let mut fx = fixture(dir.path());
        let project_bytes = std::fs::read(&fx.project).unwrap();
        let to = dir.path().join("new.snxsch");
        let failed = result.is_err();

        let done = completion(&fx, &to, result.clone());
        fx.app.handle_save_as_project_patched(done);

        assert_eq!(
            listed(&fx.app.document_state.projects[0].data),
            ["new.snxsch", "child.snxsch"],
            "{result:?}"
        );
        assert!(fx.app.document_state.is_document_dirty(&fx.project));
        assert_eq!(std::fs::read(&fx.project).unwrap(), project_bytes);
        assert_eq!(fx.app.document_state.error_notice.is_some(), failed);
    }
}

#[test]
fn save_as_while_a_path_operation_holds_the_project_file_keeps_the_change_in_memory() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    let project_bytes = std::fs::read(&fx.project).unwrap();
    let fold = fx.app.document_state.projects[0].case_fold;
    fx.app
        .document_state
        .pending_path_ops
        .begin_with_dependents(
            crate::app::path_ops::PathOpKind::AddSheet,
            vec![dir.path().join("other.snxsch")],
            vec![fx.project.clone()],
            fold,
        );

    save_as(&mut fx.app, &fx.top.clone(), dir.path().join("new.snxsch"));

    assert_eq!(std::fs::read(&fx.project).unwrap(), project_bytes);
    assert_eq!(
        listed(&fx.app.document_state.projects[0].data),
        ["new.snxsch", "child.snxsch"]
    );
    assert!(fx.app.document_state.is_document_dirty(&fx.project));
}

#[test]
fn the_project_file_is_released_when_the_job_lands() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());

    save_as(&mut fx.app, &fx.top.clone(), dir.path().join("new.snxsch"));

    assert!(fx.app.document_state.pending_path_ops.is_empty());
    let project = fx.project.clone();
    fx.app.document_state.projects[0].data.description = Some("later".to_string());
    fx.app.document_state.mark_document_dirty(&project);
    let id = project_id(&fx);
    let save = fx
        .app
        .begin_project_save(id)
        .expect("a project save starts");
    settle_task(&mut fx.app, save);
    assert_eq!(reopened(&fx.project).description.as_deref(), Some("later"));
}

#[test]
fn a_case_variant_of_the_project_folder_is_still_inside_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    fx.app.document_state.projects[0].case_fold =
        crate::app::state::scope::CaseFold::from_probe(true);
    let shouted = PathBuf::from(dir.path().to_string_lossy().to_uppercase()).join("new.snxsch");

    let task = fx
        .app
        .follow_save_as_in_project(Some(project_id(&fx)), &fx.top.clone(), &shouted);
    settle_task(&mut fx.app, task);

    assert_eq!(
        listed(&reopened(&fx.project)),
        ["new.snxsch", "child.snxsch"]
    );
}

#[test]
fn a_target_the_project_already_lists_is_not_re_pointed() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    let project_bytes = std::fs::read(&fx.project).unwrap();

    let task =
        fx.app
            .follow_save_as_in_project(Some(project_id(&fx)), &fx.top.clone(), &fx.child.clone());
    settle_task(&mut fx.app, task);

    assert_eq!(std::fs::read(&fx.project).unwrap(), project_bytes);
    assert_eq!(
        listed(&fx.app.document_state.projects[0].data),
        ["top.snxsch", "child.snxsch"]
    );
    assert!(!fx.app.document_state.is_document_dirty(&fx.project));
}
