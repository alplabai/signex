//! #2740: a footprint of a channel part with its parent sheet closed. The
//! channel name comes from the hierarchy in memory; with no hierarchy it is
//! unknown, and an unknown name is not a designator mismatch.

use std::path::{Path, PathBuf};

use signex_types::pcb::{Footprint, Point, SymbolLink};
use signex_types::schematic::InstancePath;
use uuid::Uuid;

use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::app::sheet_hierarchy::{SheetHierarchy, SheetNode, SheetNodeKind};
use crate::app::{Signex, TabInfo, TabKind};
use crate::panels::PartIdentity;

const REPEAT_SYMBOL: Uuid = Uuid::from_u128(0x2740);

/// A project whose `top` places `buck` as `Repeat(CH, 1, 2)`, with only
/// `buck` open, and a footprint `CH1_R5` linked to R5 in channel 1.
fn closed_parent(dir: &str) -> (Signex, PathBuf, Footprint) {
    let mut app = app_workspace(dir, &["top.snxsch", "buck.snxsch"]);
    let sheet = sheet_with_net("R5", "FB", &[]);
    let r5 = sheet.symbols[0].uuid;
    open_with(
        &mut app.document_state,
        &Path::new(dir).join("buck.snxsch"),
        sheet,
    );
    let board = Path::new(dir).join("board.snxpcb");
    let project = app.document_state.projects[0].id;
    app.document_state.tabs.push(TabInfo {
        title: "board".to_string(),
        path: board.clone(),
        cached_document: None,
        project_id: Some(project),
        kind: TabKind::Schematic,
    });
    let channel = signex_net::channel_uuid(REPEAT_SYMBOL, 1);
    let footprint = Footprint {
        uuid: Uuid::from_u128(0x2741),
        reference: "CH1_R5".into(),
        value: String::new(),
        footprint_id: "R_0603".into(),
        position: Point::new(0.0, 0.0),
        rotation: 0.0,
        layer: "TopCopper".into(),
        locked: false,
        pads: Vec::new(),
        graphics: Vec::new(),
        properties: Vec::new(),
        schematic_link: Some(SymbolLink::new(InstancePath::root().child(channel), [r5])),
        library_ref: None,
        component: Default::default(),
        unknown: Default::default(),
        unknown_columns: Default::default(),
    };
    (app, board, footprint)
}

fn node(name: &str, placed_as: Option<Uuid>, path: PathBuf, children: Vec<SheetNode>) -> SheetNode {
    SheetNode {
        name: name.to_string(),
        filename: path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
        placed_as,
        path,
        kind: SheetNodeKind::Present,
        detail: None,
        children,
    }
}

#[test]
fn a_closed_parent_in_the_hierarchy_keeps_the_channel_designator() {
    let dir = "/w/ch-closed-known";
    let (mut app, board, footprint) = closed_parent(dir);
    let buck = node(
        "Repeat(CH, 1, 2)",
        Some(REPEAT_SYMBOL),
        Path::new(dir).join("buck.snxsch"),
        Vec::new(),
    );
    let top = node("top", None, Path::new(dir).join("top.snxsch"), vec![buck]);
    app.document_state.panel_ctx.navigator.hierarchy = Some(SheetHierarchy {
        root: Some(top),
        ..Default::default()
    });

    let identity = app.part_identity(&board, &footprint);

    assert!(
        matches!(&identity, PartIdentity::Read { designator, .. } if designator == "CH1_R5"),
        "{identity:?}"
    );
}

#[test]
fn an_unreadable_saved_naming_is_not_read_so_it_is_no_mismatch() {
    let dir = "/w/ch-unreadable-naming";
    let (mut app, board, footprint) = closed_parent(dir);
    let buck = node(
        "Repeat(CH, 1, 2)",
        Some(REPEAT_SYMBOL),
        Path::new(dir).join("buck.snxsch"),
        Vec::new(),
    );
    let top = node("top", None, Path::new(dir).join("top.snxsch"), vec![buck]);
    app.document_state.panel_ctx.navigator.hierarchy = Some(SheetHierarchy {
        root: Some(top),
        ..Default::default()
    });
    app.document_state.projects[0].data.unknown =
        serde_json::from_value(serde_json::json!({"schematic_design": "garbled"}))
            .expect("a kept section");

    assert_eq!(app.part_identity(&board, &footprint), PartIdentity::NotRead);
}

#[test]
fn an_unknown_parent_is_not_read_so_it_is_no_mismatch() {
    let (app, board, footprint) = closed_parent("/w/ch-closed-unknown");

    let identity = app.part_identity(&board, &footprint);

    assert_eq!(identity, PartIdentity::NotRead);
}
