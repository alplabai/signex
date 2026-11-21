//! The pieces of the multi-channel slice that need no project on disk: the
//! naming presets, the Repeat switch's default and the out-of-date rule.

use std::path::PathBuf;
use std::sync::Arc;

use signex_types::schematic::check_format;

use super::state::CompiledResult;
use super::{DESIGNATOR_PRESETS, NET_PRESETS};
use crate::app::Signex;

#[test]
fn every_preset_is_a_valid_custom_format() {
    for preset in DESIGNATOR_PRESETS.iter().chain(NET_PRESETS.iter()) {
        assert_eq!(check_format(preset), Ok(()), "{preset}");
    }
}

#[test]
fn the_menu_presets_hold_the_default_naming() {
    let naming = signex_types::project::ChannelNaming::default();
    assert!(DESIGNATOR_PRESETS.contains(&naming.designator_format.as_str()));
    assert!(NET_PRESETS.contains(&naming.net_format.as_str()));
}

#[test]
fn the_switch_repeats_a_sheet_four_times_on_ch() {
    assert_eq!(
        super::repeat_edit::default_repeat().to_designator(),
        "Repeat(CH, 1, 4)"
    );
}

fn app_with_a_compile(path: &std::path::Path) -> Signex {
    let (mut app, _task) = Signex::new();
    let project = app.document_state.mint_project_id();
    let key = signex_net::SheetKey::new("child.snxsch");
    app.document_state.compiled_design.results.insert(
        project,
        CompiledResult {
            design: Arc::new(signex_net::CompiledDesign::default()),
            paths: std::collections::HashMap::from([(key, path.to_path_buf())]),
            compiled_at: "12:41".to_string(),
            stale: Default::default(),
        },
    );
    app
}

#[test]
fn an_electrical_edit_marks_a_compiled_sheet_out_of_date() {
    let path = PathBuf::from("/p/child.snxsch");
    let mut app = app_with_a_compile(&path);
    app.document_state
        .mark_compiled_stale(&path, signex_engine::DocumentPatch::SYMBOLS);
    let result = app.document_state.compiled_design.results.values().next();
    assert!(result.is_some_and(|r| r.stale.contains(&path)));
}

#[test]
fn a_colour_only_edit_or_another_sheet_leaves_the_compile_fresh() {
    let path = PathBuf::from("/p/child.snxsch");
    let mut app = app_with_a_compile(&path);
    app.document_state
        .mark_compiled_stale(&path, signex_engine::DocumentPatch::NET_COLORS);
    app.document_state.mark_compiled_stale(
        &PathBuf::from("/p/other.snxsch"),
        signex_engine::DocumentPatch::WIRES,
    );
    let result = app.document_state.compiled_design.results.values().next();
    assert!(result.is_some_and(|r| r.stale.is_empty()));
}
