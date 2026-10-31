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
        "the project file is written, not left dirty"
    );
}

#[test]
fn save_as_of_a_child_sheet_re_points_the_open_parent_reference() {
    let dir = tempfile::tempdir().unwrap();
    let mut fx = fixture(dir.path());
    fx.app.open_or_focus_sheet_path(fx.child.clone());
    let to = dir.path().join("kid.snxsch");

    save_as(&mut fx.app, &fx.child.clone(), to.clone());

    assert_eq!(listed(&reopened(&fx.project)), ["top.snxsch", "kid.snxsch"]);
    let parent = fx.app.document_state.engines[&fx.top].document();
    assert_eq!(parent.child_sheets[0].filename, "kid.snxsch");
    assert!(fx.app.document_state.is_document_dirty(&fx.top));
    assert!(fx.child.exists(), "old file stays on disk");
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
