//! Mockup row 154: the Compiled Design panel. Compile, the Sheets and Nets
//! modes, the search, fold, a row click, the out-of-date state.

use signex_app::app::{CompiledDesignMsg, EditMsg, Message};
use signex_app::panels::compiled_design::{CompiledMode, CompiledPanelView, CompiledStatus};
use signex_types::schematic::{SelectedItem, SelectedKind};

use super::support::{
    ProjectBench, empty_sheet, net_label, part, project_bench, sheet_symbol, wire,
};

/// `root` places `child` twice as `Repeat(CH, 1, 2)`; the child holds R5 and
/// the net FB.
fn repeated() -> ProjectBench {
    let mut root = empty_sheet();
    root.child_sheets.push(sheet_symbol("Repeat(CH, 1, 2)"));
    let mut child = empty_sheet();
    child.symbols.push(part("R5", (50.8, 50.8)));
    child.wires.push(wire((40.64, 40.64), (45.72, 40.64)));
    child.labels.push(net_label("FB", (40.64, 40.64)));
    project_bench(root, child)
}

fn send(b: &mut ProjectBench, msg: CompiledDesignMsg) {
    b.send(Message::from(msg));
}

fn panel(b: &ProjectBench) -> &CompiledPanelView {
    &b.app.document_state.panel_ctx.compiled_design.panel
}

fn names(b: &ProjectBench) -> Vec<String> {
    panel(b).rows.iter().map(|r| r.name.clone()).collect()
}

fn row_index(b: &ProjectBench, name: &str) -> usize {
    panel(b)
        .rows
        .iter()
        .find(|r| r.name == name)
        .unwrap_or_else(|| panic!("no row {name}"))
        .index
}

fn compiled() -> ProjectBench {
    let mut b = repeated();
    send(&mut b, CompiledDesignMsg::CompileRequested);
    assert_eq!(panel(&b).status, CompiledStatus::Ready);
    b
}

#[test]
fn m154_before_a_compile_the_panel_says_not_compiled_and_counts_the_files() {
    let mut b = repeated();
    send(&mut b, CompiledDesignMsg::FindChanged(String::new()));
    assert_eq!(panel(&b).status, CompiledStatus::NotCompiled);
    assert_eq!(panel(&b).sheet_files, 2);
    assert!(panel(&b).rows.is_empty());
}

#[test]
fn m154_compile_expands_the_repeated_sheet_into_channels_with_totals() {
    let b = compiled();
    assert_eq!(names(&b), ["root", "child", "CH1", "CH2"]);
    assert_eq!(panel(&b).totals.parts, 2, "CH1_R5 and CH2_R5");
    assert_eq!(panel(&b).totals.channels, 2);
    assert!(!panel(&b).compiled_at.is_empty());
}

#[test]
fn m154_a_compile_writes_nothing_and_marks_nothing_dirty() {
    let b = compiled();
    assert!(!b.is_dirty(&b.root));
    assert!(!b.is_dirty(&b.child));
    assert!(!b.is_dirty(&b.file));
}

#[test]
fn m154_nets_mode_lists_the_compiled_nets() {
    let mut b = compiled();
    send(&mut b, CompiledDesignMsg::ModePicked(CompiledMode::Nets));
    assert_eq!(panel(&b).mode, CompiledMode::Nets);
    assert!(!panel(&b).nets.is_empty(), "the FB net of each channel");
    send(&mut b, CompiledDesignMsg::ModePicked(CompiledMode::Sheets));
    assert_eq!(panel(&b).mode, CompiledMode::Sheets);
}

#[test]
fn m154_the_search_keeps_matching_rows_and_a_miss_shows_none() {
    let mut b = compiled();
    send(&mut b, CompiledDesignMsg::FindChanged("CH2".to_string()));
    let kept = names(&b);
    assert!(kept.iter().any(|n| n == "CH2"), "{kept:?}");
    assert!(!kept.iter().any(|n| n == "CH1"), "{kept:?}");
    send(
        &mut b,
        CompiledDesignMsg::FindChanged("no-such".to_string()),
    );
    assert!(panel(&b).rows.is_empty());
    send(&mut b, CompiledDesignMsg::FindChanged(String::new()));
    assert_eq!(names(&b).len(), 4, "clearing the search brings them back");
}

#[test]
fn m154_the_chevron_folds_and_unfolds_a_repeated_row() {
    let mut b = compiled();
    let repeated = row_index(&b, "child");
    send(&mut b, CompiledDesignMsg::RowToggled(repeated));
    assert_eq!(names(&b), ["root", "child"]);
    send(&mut b, CompiledDesignMsg::RowToggled(repeated));
    assert_eq!(names(&b), ["root", "child", "CH1", "CH2"]);
}

#[test]
fn m154_a_click_on_a_channel_row_opens_that_channel() {
    let mut b = compiled();
    let row = row_index(&b, "CH2");
    send(&mut b, CompiledDesignMsg::RowClicked(row));
    assert_eq!(
        b.app.document_state.active_path.as_deref(),
        Some(b.child.as_path())
    );
    let strip = b
        .app
        .document_state
        .panel_ctx
        .compiled_design
        .strip
        .clone()
        .expect("the channel strip");
    assert_eq!(strip.active, Some(2));
}

fn delete_r5(b: &mut ProjectBench) {
    let child = b.child.clone();
    let id = b.doc(&child).symbols[0].uuid;
    b.app.document_state.active_tab = 1;
    b.app.document_state.active_path = Some(child);
    b.app.interaction_state.active_canvas_mut().selected =
        vec![SelectedItem::new(id, SelectedKind::Symbol)];
    b.send(Message::Edit(EditMsg::DeleteSelected));
}

#[test]
fn m154_an_edit_marks_rows_out_of_date_and_never_recompiles() {
    let mut b = compiled();
    let compiled_parts = panel(&b).totals.parts;
    delete_r5(&mut b);
    assert_eq!(panel(&b).stale_count, 1);
    assert!(panel(&b).rows.iter().any(|r| r.name == "CH1" && r.is_stale));
    assert_eq!(
        panel(&b).totals.parts,
        compiled_parts,
        "the counts wait for a Recompile"
    );
}

#[test]
fn m154_recompile_clears_the_out_of_date_state() {
    let mut b = compiled();
    delete_r5(&mut b);
    send(&mut b, CompiledDesignMsg::CompileRequested);
    assert_eq!(panel(&b).stale_count, 0);
    assert_eq!(panel(&b).totals.parts, 0, "R5 is gone from both channels");
}

#[test]
fn m154_a_channel_tab_is_read_only_until_editor_is_picked() {
    let mut b = compiled();
    let child = b.child.clone();
    send(
        &mut b,
        CompiledDesignMsg::ChannelPicked {
            sheet: child.clone(),
            channel: Some(1),
        },
    );
    delete_r5(&mut b);
    assert_eq!(b.doc(&child).symbols.len(), 1, "a channel never edits");
    assert!(!b.is_dirty(&child));
    send(
        &mut b,
        CompiledDesignMsg::ChannelPicked {
            sheet: child.clone(),
            channel: None,
        },
    );
    delete_r5(&mut b);
    assert!(b.doc(&child).symbols.is_empty(), "Editor edits the sheet");
}
