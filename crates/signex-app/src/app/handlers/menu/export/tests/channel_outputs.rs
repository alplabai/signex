//! #2740: every output of a repeated sheet names its parts by channel, through
//! the project's saved channel naming, from the real export scope.

use std::path::PathBuf;

use signex_output::{BomOptions, Exporter, NetlistExporter, NetlistOptions, bom};
use signex_types::project::ChannelNaming;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::Signex;
use crate::app::back_annotate::read::{BackRequest, compute as back_compute};
use crate::app::handlers::menu::export::{build_bom_export_context, build_export_scope};
use crate::app::project_sheets::plan_project_sheets_for;
use crate::app::saved_channel_naming::SavedChannelNaming;
use crate::app::update_pcb::design::{DesignRequest, compute};

/// `top` places `buck` as `Repeat(CH, 1, 2)`; `buck` holds R5.
fn repeated(dir: &str) -> Signex {
    let mut app = app_workspace(dir, &["top.snxsch", "buck.snxsch"]);
    let top = PathBuf::from(dir).join("top.snxsch");
    let mut root = sheet_with_net("R_TOP", "TOP_NET", &["buck.snxsch"]);
    root.child_sheets[0].name = "Repeat(CH, 1, 2)".to_string();
    open_with(&mut app.document_state, &top, root);
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("buck.snxsch"),
        sheet_with_net("R5", "FB", &[]),
    );
    app.document_state.active_path = Some(top);
    app
}

fn alpha_naming() -> ChannelNaming {
    ChannelNaming {
        designator_format: "${Designator}_${ChannelAlpha}".to_string(),
        ..ChannelNaming::default()
    }
}

fn bom_references(app: &Signex) -> Vec<String> {
    let ctx = build_bom_export_context(&app.document_state).expect("scope");
    let table = bom::rollup(&ctx, &BomOptions::default());
    let mut refs: Vec<String> = table
        .rows
        .iter()
        .flat_map(|row| row.references.clone())
        .collect();
    refs.sort();
    refs
}

#[test]
fn the_bom_lists_each_channels_designator() {
    let app = repeated("/w/ch-bom");
    assert_eq!(bom_references(&app), ["CH1_R5", "CH2_R5", "R_TOP"]);
}

#[test]
fn the_bom_follows_the_saved_naming() {
    let mut app = repeated("/w/ch-bom-named");
    app.document_state.projects[0]
        .data
        .schematic_design
        .channel_naming = alpha_naming();
    assert_eq!(bom_references(&app), ["R5_A", "R5_B", "R_TOP"]);
}

#[test]
fn the_netlist_file_exports_with_a_component_per_channel() {
    let app = repeated("/w/ch-net");
    let (ctx, _) = build_export_scope(&app.document_state).expect("scope");

    let out = NetlistExporter
        .export(&ctx, &NetlistOptions::default())
        .expect("channel parts are not duplicates");

    let text = String::from_utf8(out.bytes).unwrap();
    for block in ["[\r\nCH1_R5\r\n", "[\r\nCH2_R5\r\n"] {
        assert!(text.contains(block), "{block:?} in:\n{text}");
    }
    for terminal in ["CH1_R5-1", "CH2_R5-1"] {
        assert!(text.contains(terminal), "{terminal} in:\n{text}");
    }
    assert!(!text.contains("[\r\nR5\r\n"), "no bare R5 block:\n{text}");
}

#[test]
fn each_printed_channel_page_shows_its_own_designator() {
    let app = repeated("/w/ch-pdf");
    let (ctx, _) = build_export_scope(&app.document_state).expect("scope");
    let mut printed: Vec<String> = ctx
        .sheets
        .iter()
        .filter(|page| page.path.ends_with("buck.snxsch"))
        .flat_map(|page| page.schematic.symbols.iter())
        .map(|symbol| symbol.reference.clone())
        .collect();
    printed.sort();
    assert_eq!(printed, ["CH1_R5", "CH2_R5"]);
}

#[test]
fn update_pcb_reads_the_saved_naming_on_parts_and_terminals() {
    let dir = "/w/ch-pcb";
    let mut app = repeated(dir);
    app.document_state.projects[0]
        .data
        .schematic_design
        .channel_naming = alpha_naming();
    let root = PathBuf::from(dir).join("top.snxsch");

    let computed = compute(DesignRequest {
        plan: plan_project_sheets_for(&app.document_state, &root),
        project_dir: PathBuf::from(dir),
        board: serde_json::from_value(serde_json::json!({
            "uuid": uuid::Uuid::from_u128(1),
            "outline": [{"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0}, {"x": 50.0, "y": 30.0}]
        }))
        .expect("an empty board"),
        revision: 0,
        libraries: Vec::new(),
        open_revisions: Vec::new(),
        previous: None,
        naming: alpha_naming().into(),
    })
    .expect("the design reads");

    let snapshot = &computed.snapshot;
    let mut parts: Vec<&str> = snapshot
        .parts
        .iter()
        .map(|part| part.part.designator.as_str())
        .collect();
    parts.sort_unstable();
    assert_eq!(parts, ["R5_A", "R5_B", "R_TOP"]);
    let terminals: Vec<&str> = snapshot
        .netlist
        .nets
        .iter()
        .flat_map(|net| net.terminals.iter())
        .map(|t| t.reference.as_str())
        .collect();
    for part in ["R5_A", "R5_B"] {
        assert!(terminals.contains(&part), "{part} in {terminals:?}");
    }
}

fn mark_naming_unreadable(app: &mut Signex) {
    app.document_state.projects[0].data.unknown =
        serde_json::from_value(serde_json::json!({"schematic_design": "garbled"}))
            .expect("a kept section");
}

fn empty_board() -> signex_types::pcb::PcbBoard {
    serde_json::from_value(serde_json::json!({
        "uuid": uuid::Uuid::from_u128(1),
        "outline": [{"x": 0.0, "y": 0.0}, {"x": 50.0, "y": 0.0}, {"x": 50.0, "y": 30.0}]
    }))
    .expect("an empty board")
}

fn update_pcb_read(app: &Signex, dir: &str) -> Result<(), String> {
    let root = PathBuf::from(dir).join("top.snxsch");
    compute(DesignRequest {
        plan: plan_project_sheets_for(&app.document_state, &root),
        project_dir: PathBuf::from(dir),
        board: empty_board(),
        revision: 0,
        libraries: Vec::new(),
        open_revisions: Vec::new(),
        previous: None,
        naming: SavedChannelNaming::of(&app.document_state.projects[0].data),
    })
    .map(|_| ())
}

fn banner() -> String {
    signex_i18n::I18n::en_us_ref()
        .message_plain(signex_i18n::ids::DIALOG_SCHEMATIC_DESIGN_UNREADABLE, &[])
}

#[test]
fn an_unreadable_naming_refuses_update_pcb_on_a_project_with_channels() {
    let dir = "/w/ch-unreadable-pcb";
    let mut app = repeated(dir);
    mark_naming_unreadable(&mut app);

    assert_eq!(update_pcb_read(&app, dir), Err(banner()));
}

#[test]
fn an_unreadable_naming_refuses_back_annotate_on_a_project_with_channels() {
    let dir = "/w/ch-unreadable-back";
    let mut app = repeated(dir);
    mark_naming_unreadable(&mut app);
    let root = PathBuf::from(dir).join("top.snxsch");

    let read = back_compute(BackRequest {
        plan: plan_project_sheets_for(&app.document_state, &root),
        project_dir: PathBuf::from(dir),
        board: empty_board(),
        revision: 0,
        libraries: Vec::new(),
        open_revisions: Vec::new(),
        previous: None,
        naming: SavedChannelNaming::of(&app.document_state.projects[0].data),
    });

    assert_eq!(read.err(), Some(banner()));
}

#[test]
fn an_unreadable_naming_leaves_a_project_without_channels_working() {
    let dir = "/w/ch-unreadable-plain";
    let mut app = repeated(dir);
    let mut root = sheet_with_net("R_TOP", "TOP_NET", &["buck.snxsch"]);
    root.child_sheets[0].name = "buck".to_string();
    open_with(
        &mut app.document_state,
        &PathBuf::from(dir).join("top.snxsch"),
        root,
    );
    mark_naming_unreadable(&mut app);

    assert_eq!(update_pcb_read(&app, dir), Ok(()));
    let (_, issues) = build_export_scope(&app.document_state).expect("scope");
    assert!(!issues.channel_naming_unreadable);
}

#[test]
fn an_unreadable_naming_refuses_the_netlist_file_and_writes_nothing() {
    let dir = "/w/ch-unreadable-net";
    let mut app = repeated(dir);
    mark_naming_unreadable(&mut app);
    let source = PathBuf::from(dir).join("top.snxsch");
    let target = std::env::temp_dir().join(format!("signex-{}.net", uuid::Uuid::now_v7()));

    let _ = app.handle_export_netlist_finished(source, Ok(target.clone()));

    assert!(app.document_state.error_notice.is_some());
    assert!(!target.exists());
}

#[test]
fn an_unreadable_naming_refuses_the_bom_preview() {
    let mut app = repeated("/w/ch-unreadable-bom");
    mark_naming_unreadable(&mut app);

    let _ = app.handle_bom_preview_open();

    assert!(app.document_state.bom_preview.is_none());
    assert!(app.document_state.error_notice.is_some());
}

#[test]
fn an_unreadable_naming_refuses_the_print_preview() {
    let mut app = repeated("/w/ch-unreadable-pdf");
    mark_naming_unreadable(&mut app);

    let _ = app.handle_print_preview_requested();

    assert!(app.document_state.preview.is_none());
    assert!(app.document_state.error_notice.is_some());
}
