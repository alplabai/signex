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
    let dir = std::env::temp_dir().join(format!("signex-{tag}-{}", Uuid::new_v4()));
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
    app.refresh_net_class_styles();
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

    app.refresh_net_class_styles();

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

    app.refresh_net_class_styles();
    let (ctx, _issues) =
        super::super::build_page_scope_for_source(&app.document_state, &top).expect("scope");
    let exported = super::super::class_styles::export_class_styles(&app.document_state, &top, &ctx);

    assert_eq!(&exported, app.ui_state.net_class_styles.styles());
}

#[test]
fn an_update_that_edits_no_sheet_and_no_class_table_does_not_refresh() {
    let dir = PathBuf::from("/w/class-gate");
    let (app, ..) = app_with_open_pair(&dir);
    let mut app = app;
    app.refresh_net_class_styles();
    let stamp_before = app.ui_state.net_class_styles.styles().len();
    assert!(stamp_before > 0);

    // Same state again, as after a pointer move: the gate must say "current".
    let stamp = {
        use std::hash::{Hash, Hasher};
        app.document_state
            .engines
            .iter()
            .fold(0u64, |acc, (path, engine)| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                (path, engine.revision()).hash(&mut hasher);
                acc.wrapping_add(hasher.finish())
            })
    };
    assert!(
        !app.ui_state
            .net_class_styles
            .needs_refresh(stamp, &app.document_state.projects)
    );
}
