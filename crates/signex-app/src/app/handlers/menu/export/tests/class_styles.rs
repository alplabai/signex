//! Net-class styles in an export come from the export's own project netlist,
//! and the canvas resolves the same styles from the same stitching.

use std::path::PathBuf;

use signex_types::net_class::{NetClass, NetLineStyle};
use signex_types::schematic::{LabelType, Point, SchematicSheet, SheetPin};
use uuid::Uuid;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::Signex;

/// A parent whose `VCC` wire ends in a sheet pin `VIN`, and a child whose
/// wire carries only the hierarchical label `VIN`: on its own the child wire
/// is a net named `VIN`; through the sheet entry it is `VCC`.
fn parent_and_child() -> (SchematicSheet, SchematicSheet, Uuid, Uuid) {
    let mut parent = sheet_with_net("R_TOP", "VCC", &["child.snxsch"]);
    parent.child_sheets[0].pins.push(SheetPin {
        uuid: Uuid::nil(),
        name: "VIN".to_string(),
        direction: String::new(),
        position: Point::new(10.0, 0.0),
        rotation: 0.0,
        auto_generated: false,
        user_moved: false,
    });
    let mut child = sheet_with_net("R_CHILD", "VIN", &[]);
    child.labels[0].label_type = LabelType::Hierarchical;
    let parent_wire = parent.wires[0].uuid;
    let child_wire = child.wires[0].uuid;
    (parent, child, parent_wire, child_wire)
}

fn dashed_vcc() -> NetClass {
    let mut class = NetClass::named("Power");
    class.nets.push("VCC".to_string());
    class.style.line = NetLineStyle::Dashed;
    class
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("signex-{tag}-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    dir
}

#[test]
fn an_export_styles_a_child_sheet_that_is_not_open_in_a_tab() {
    let dir = temp_dir("class-export");
    let (parent, child, parent_wire, child_wire) = parent_and_child();
    let child_text = signex_types::format::SnxSchematic::new(child)
        .write_string()
        .expect("serialize child");
    std::fs::write(dir.join("child.snxsch"), child_text).expect("write child");

    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![dashed_vcc()];
    let top = dir.join("top.snxsch");
    open_with(&mut app.document_state, &top, parent);
    app.document_state.active_path = Some(top.clone());

    let (ctx, _issues) =
        super::super::build_page_scope_for_source(&app.document_state, &top).expect("scope");
    let styles = super::super::class_styles::export_class_styles(&app.document_state, &top, &ctx);
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(
        styles.get(&parent_wire).map(|s| s.line),
        Some(NetLineStyle::Dashed),
        "the open parent wire is styled"
    );
    assert_eq!(
        styles.get(&child_wire).map(|s| s.line),
        Some(NetLineStyle::Dashed),
        "the child is only on disk, but it reaches VCC through the sheet entry"
    );
    // The canvas cache knows only open sheets, so it cannot be the source.
    settle(&mut app);
    assert!(
        !app.ui_state
            .net_class_styles
            .styles()
            .contains_key(&child_wire),
        "the unopened child is not on the canvas, and not in its cache"
    );
}

fn app_with_open_pair(dir: &std::path::Path) -> (Signex, Uuid, Uuid) {
    let (parent, child, parent_wire, child_wire) = parent_and_child();
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch", "child.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![dashed_vcc()];
    open_with(&mut app.document_state, &dir.join("top.snxsch"), parent);
    open_with(&mut app.document_state, &dir.join("child.snxsch"), child);
    app.document_state.active_path = Some(dir.join("top.snxsch"));
    (app, parent_wire, child_wire)
}

#[test]
fn a_child_wire_reaching_the_net_through_a_sheet_entry_is_styled_on_the_canvas() {
    let dir = PathBuf::from("/w/class-canvas");
    let (mut app, parent_wire, child_wire) = app_with_open_pair(&dir);

    settle(&mut app);

    let styles = app.ui_state.net_class_styles.styles();
    assert_eq!(
        styles.get(&parent_wire).map(|s| s.line),
        Some(NetLineStyle::Dashed)
    );
    assert_eq!(
        styles.get(&child_wire).map(|s| s.line),
        Some(NetLineStyle::Dashed),
        "the child wire is VCC through the sheet entry, as on the parent"
    );
}

#[test]
fn the_canvas_and_the_export_resolve_the_same_styles() {
    let dir = PathBuf::from("/w/class-agree");
    let (mut app, ..) = app_with_open_pair(&dir);
    let top = dir.join("top.snxsch");

    settle(&mut app);
    let (ctx, _issues) =
        super::super::build_page_scope_for_source(&app.document_state, &top).expect("scope");
    let exported = super::super::class_styles::export_class_styles(&app.document_state, &top, &ctx);

    assert_eq!(&exported, app.ui_state.net_class_styles.styles());
}

#[test]
fn only_an_edit_that_can_change_connectivity_plans_a_resolution() {
    let dir = PathBuf::from("/w/class-gate");
    let (mut app, parent_wire, _) = app_with_open_pair(&dir);
    settle(&mut app);
    assert!(app.plan_net_class_refresh().is_none(), "a pointer move");

    // A paper-size edit bumps the engine revision, not its connectivity.
    let top = dir.join("top.snxsch");
    app.document_state
        .engines
        .get_mut(&top)
        .expect("engine")
        .execute(signex_engine::Command::SetPaperSize {
            paper_size: "A3".into(),
        })
        .expect("paper size");
    assert!(
        app.plan_net_class_refresh().is_none(),
        "a non-electrical edit must not re-stitch the project"
    );

    app.document_state
        .engines
        .get_mut(&top)
        .expect("engine")
        .execute(signex_engine::Command::DeleteSelection {
            items: vec![signex_types::schematic::SelectedItem::new(
                parent_wire,
                signex_types::schematic::SelectedKind::Wire,
            )],
        })
        .expect("delete wire");
    assert!(
        app.plan_net_class_refresh().is_some(),
        "deleting a wire changes connectivity"
    );
}

/// Run the planned background resolution the way the task would, on this
/// thread, and apply its answer.
fn settle(app: &mut Signex) {
    app.settle_net_class_styles_blocking();
}

#[test]
fn the_refresh_after_an_update_reads_no_disk_and_keeps_the_old_styles() {
    // The child is only on disk. Resolving it needs a file read, which must
    // happen on a worker: right after the refresh the canvas still shows
    // what it showed before (nothing), and the answer arrives later.
    let dir = temp_dir("class-nonblocking");
    let (parent, child, parent_wire, child_wire) = parent_and_child();
    let child_text = signex_types::format::SnxSchematic::new(child)
        .write_string()
        .expect("serialize child");
    std::fs::write(dir.join("child.snxsch"), child_text).expect("write child");
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    app.document_state.projects[0].data.net_classes = vec![dashed_vcc()];
    let top = dir.join("top.snxsch");
    open_with(&mut app.document_state, &top, parent);
    app.document_state.active_path = Some(top);

    let _task = app.refresh_net_class_styles();
    let during = app.ui_state.net_class_styles.styles().len();

    // The worker's half, later: reads the child from disk. A fresh gate
    // opening stands in for the next edit.
    app.document_state.projects[0].data.net_classes = vec![dashed_vcc(), NetClass::named("X")];
    let job = app.plan_net_class_refresh().expect("gate opens");
    let resolved = job.run();
    app.apply_net_class_styles(&Ok(resolved));
    std::fs::remove_dir_all(&dir).ok();

    assert_eq!(during, 0, "update applied nothing synchronously");
    let styles = app.ui_state.net_class_styles.styles();
    assert!(styles.contains_key(&parent_wire));
    assert!(styles.contains_key(&child_wire));
}

#[test]
fn a_stale_resolution_is_dropped_and_the_newer_one_kept() {
    let dir = PathBuf::from("/w/class-stale");
    let (mut app, parent_wire, _) = app_with_open_pair(&dir);

    let older = app.plan_net_class_refresh().expect("first gate opening");
    let mut dotted = dashed_vcc();
    dotted.style.line = NetLineStyle::Dotted;
    app.document_state.projects[0].data.net_classes = vec![dotted];
    let newer = app.plan_net_class_refresh().expect("second gate opening");

    let (older, newer) = (older.run(), newer.run());
    app.apply_net_class_styles(&Ok(newer));
    app.apply_net_class_styles(&Ok(older));

    assert_eq!(
        app.ui_state
            .net_class_styles
            .styles()
            .get(&parent_wire)
            .map(|s| s.line),
        Some(NetLineStyle::Dotted),
        "the late, older answer must not overwrite the newer one"
    );
}
