//! #765 — a History restore reloads the footprint editor from disk. A load
//! repair found on that reload must not be logged a second time, and the
//! tab and `dirty_paths` must agree with the editor, which is dirty because
//! the repair is not saved yet.

use signex_app::app::{FootprintDocument, LoadedProject, Message, Signex, TabInfo, TabKind};
use signex_app::dock::DockMessage;
use signex_app::panels::PanelMsg;
use signex_library::adapters::local_git_project::LocalGitProjectAdapter;
use signex_library::{Footprint, FootprintFile};
use signex_sketch::SketchData;
use signex_sketch::entity::{Entity, EntityKind};
use signex_sketch::id::SketchEntityId;
use signex_sketch::plane::{Plane, PlaneId, PlaneKind};
use signex_types::project::ProjectData;
use std::collections::HashMap;
use std::path::Path;
use tempfile::TempDir;

const REPAIR_WARNING: &str = "moved the end point of 1 arc(s)";

/// A footprint whose arc ends off its circle: the loader repairs it.
fn off_circle_footprint() -> Footprint {
    let plane = Plane {
        id: PlaneId::new(),
        kind: PlaneKind::BoardTop,
    };
    let pid = plane.id;
    let mut sketch = SketchData::default();
    sketch.planes.push(plane);
    let mut point = |x, y| {
        let id = SketchEntityId::new();
        sketch
            .entities
            .push(Entity::new(id, pid, EntityKind::Point { x, y }));
        id
    };
    let ids = [point(2.0, 3.0), point(7.0, 3.0), point(2.0, 10.0)];
    sketch.entities.push(Entity::new(
        SketchEntityId::new(),
        pid,
        EntityKind::Arc {
            center: ids[0],
            start: ids[1],
            end: ids[2],
            sweep_ccw: true,
        },
    ));
    let mut fp = Footprint::empty("arc");
    fp.sketch = Some(sketch);
    fp
}

fn repair_warnings(marker: &str) -> usize {
    signex_app::diagnostics::recent_entries()
        .iter()
        .filter(|e| e.message.contains(REPAIR_WARNING) && e.message.contains(marker))
        .count()
}

fn restore_clicked(app: &mut Signex, sha: &str) {
    crate::settle_support::settle(
        app,
        Message::Dock(DockMessage::Panel(PanelMsg::HistoryRestoreClicked {
            sha: sha.into(),
        })),
    );
}

fn add_project(app: &mut Signex, dir: &Path) {
    let id = app.document_state.mint_project_id();
    app.document_state.projects.push(LoadedProject {
        id,
        path: dir.join("P.snxprj"),
        data: ProjectData {
            name: "P".into(),
            dir: dir.to_string_lossy().into_owned(),
            schematic_root: None,
            pcb_file: None,
            sheets: Vec::new(),
            variant_definitions: Vec::new(),
            active_variant: None,
            libraries: Vec::new(),
            enable_git: false,
            net_classes: Vec::new(),
            rules: None,
            annotation: None,
            unknown: Default::default(),
        },
        pending_libraries: HashMap::new(),
        case_fold: Default::default(),
        case_probed: false,
        case_decided: false,
    });
}

#[test]
fn history_reload_does_not_repeat_the_repair_warning_and_keeps_the_tab_dirty() {
    drop(signex_app::diagnostics::init_logging());
    signex_library::git_identity::test_identity::install();
    let tmp = TempDir::new().unwrap();
    let marker = tmp
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .expect("temp dir name")
        .to_string();
    let path = tmp.path().join("arc.snxfpt");
    let text = FootprintFile::from_footprint(off_circle_footprint())
        .to_toml_string()
        .expect("serialise");
    std::fs::write(&path, &text).unwrap();
    let adapter = LocalGitProjectAdapter::open_or_init(tmp.path().to_path_buf()).unwrap();
    let sha = adapter
        .commit_path(Path::new("arc.snxfpt"), "arc")
        .unwrap()
        .expect("a commit")
        .to_string();

    let (mut app, _t) = Signex::new();
    add_project(&mut app, tmp.path());
    // Open: the loader repairs the arc and warns once.
    let file = FootprintFile::from_toml_str(&text).unwrap();
    app.document_state
        .footprint_editors
        .insert(path.clone(), FootprintDocument::new(path.clone(), file));
    app.document_state.tabs.push(TabInfo {
        title: "arc".into(),
        path: path.clone(),
        cached_document: None,
        dirty: true,
        project_id: None,
        kind: TabKind::FootprintEditor(path.clone()),
    });
    app.document_state.dirty_paths.insert(path.clone());
    app.document_state.active_tab = app.document_state.tabs.len() - 1;
    assert_eq!(repair_warnings(&marker), 1, "opening warns once");

    // The first update after the tab appears makes the History panel adopt
    // it, which drops any armed confirmation; let that settle first.
    let _ = app.update(Message::Dock(DockMessage::Panel(
        PanelMsg::HistoryRestoreCancelled,
    )));
    // The editor is dirty, so the first click arms the confirmation and the
    // second restores and reloads the editor from disk.
    restore_clicked(&mut app, &sha);
    assert_eq!(
        app.document_state.history.pending_restore.as_deref(),
        Some(sha.as_str()),
        "the first click arms the confirmation"
    );
    restore_clicked(&mut app, &sha);

    assert!(
        app.document_state.history.pending_restore.is_none(),
        "the restore ran"
    );
    assert_eq!(
        repair_warnings(&marker),
        1,
        "the reload must not log the repair again"
    );
    let editor_dirty = app.document_state.footprint_editors[&path].dirty;
    assert!(editor_dirty, "the unsaved repair leaves the editor dirty");
    let tab_idx = app.document_state.active_tab;
    assert_eq!(app.document_state.tabs[tab_idx].dirty, editor_dirty);
    assert!(app.document_state.dirty_paths.contains(&path));
}
