//! #2740: every output of a repeated sheet names its parts by channel, through
//! the project's saved channel naming, from the real export scope.

use std::path::PathBuf;

use signex_output::{BomOptions, Exporter, NetlistExporter, NetlistOptions, bom};
use signex_types::project::ChannelNaming;

use super::{app_workspace, open_with, sheet_with_net};
use crate::app::Signex;
use crate::app::handlers::menu::export::{build_bom_export_context, build_export_scope};
use crate::app::project_sheets::plan_project_sheets_for;
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
        naming: alpha_naming(),
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
