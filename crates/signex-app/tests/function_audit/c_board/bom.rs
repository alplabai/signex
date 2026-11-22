//! Mockup row 112, the BOM export dialog (decision 2026-11-19): grouping, the
//! line filter, the one not-fitted option, the format, columns, rules and
//! issues, sorting, Cancel and Export. Nothing here edits the sheet.

use signex_app::app::{BomPreviewMsg, Message};
use signex_app::menu_bar::MenuMessage;
use signex_output::{BomColumn, BomFormat, BomGrouping, BomRule};

use super::support::{Bench, open_sheet};

macro_rules! bom {
    ($b:expr) => {
        $b.app
            .document_state
            .bom_preview
            .as_ref()
            .expect("the BOM dialog is open")
    };
}

fn send(b: &mut Bench, msg: BomPreviewMsg) {
    b.send(Message::BomPreview(msg));
}

fn opened() -> Bench {
    let mut b = open_sheet();
    b.send(Message::Menu(MenuMessage::ExportBom));
    b
}

fn is_open(b: &Bench) -> bool {
    b.app.document_state.bom_preview.is_some()
}

fn has_issue(b: &Bench, rule: BomRule) -> bool {
    bom!(b)
        .validation_report
        .issues
        .iter()
        .any(|issue| issue.rule == rule)
}

#[test]
fn the_dialog_opens_grouped_without_the_not_fitted_part() {
    let b = opened();
    assert_eq!(bom!(b).options.grouping, BomGrouping::Grouped);
    assert_eq!(
        bom!(b).table.rows.len(),
        2,
        "10k and 100n; D1 is not fitted"
    );
    assert_eq!(bom!(b).visible_lines.len(), 2);
    assert!(!b.dirty(), "opening the dialog is no edit");
}

#[test]
fn the_grouping_switch_changes_the_lines_of_the_table() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::SetGrouping(BomGrouping::Ungrouped));
    assert_eq!(bom!(b).table.rows.len(), 3, "one line a part");
    send(&mut b, BomPreviewMsg::SetGrouping(BomGrouping::Flat));
    assert_eq!(bom!(b).table.rows.len(), 3);
    send(&mut b, BomPreviewMsg::SetGrouping(BomGrouping::Grouped));
    assert_eq!(bom!(b).table.rows.len(), 2);
}

#[test]
fn the_not_fitted_option_sets_both_flags_and_lists_the_part() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::NotFittedIncludedToggled(true));
    assert!(bom!(b).options.include_dnp && bom!(b).options.include_not_fitted);
    assert_eq!(bom!(b).table.rows.len(), 3, "D1 is listed");
    send(&mut b, BomPreviewMsg::NotFittedIncludedToggled(false));
    assert!(!bom!(b).options.include_dnp && !bom!(b).options.include_not_fitted);
    assert_eq!(bom!(b).table.rows.len(), 2);
}

#[test]
fn the_filter_hides_lines_on_screen_only() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::FilterChanged("C_0603".into()));
    assert_eq!(bom!(b).visible_lines.len(), 1, "one line matches");
    assert_eq!(bom!(b).table.rows.len(), 2, "the table keeps every line");
    send(&mut b, BomPreviewMsg::FilterChanged(String::new()));
    assert_eq!(bom!(b).visible_lines.len(), 2);
}

#[test]
fn a_filter_that_matches_nothing_still_exports_every_line() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::FilterChanged("no such part".into()));
    assert!(bom!(b).visible_lines.is_empty());
    let out = b.out("filtered.csv");
    b.answer(&out);
    send(&mut b, BomPreviewMsg::Export);
    let csv = std::fs::read_to_string(&out).expect("the CSV is written");
    assert!(csv.contains("R1") && csv.contains("C1"), "{csv}");
}

#[test]
fn an_unticked_column_is_left_out_of_the_export() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::ToggleColumn(BomColumn::Description));
    assert!(!bom!(b).options.columns.contains(&BomColumn::Description));
    let out = b.out("columns.csv");
    b.answer(&out);
    send(&mut b, BomPreviewMsg::Export);
    let csv = std::fs::read_to_string(&out).expect("the CSV is written");
    assert!(!csv.contains(BomColumn::Description.header()), "{csv}");
    assert!(csv.contains(BomColumn::Designator.header()), "{csv}");
}

#[test]
fn a_column_dragged_onto_another_takes_its_place() {
    let mut b = opened();
    let first = bom!(b).options.columns[0].clone();
    send(&mut b, BomPreviewMsg::ColumnDragStart(0));
    send(&mut b, BomPreviewMsg::ColumnDragDrop(2));
    assert_eq!(bom!(b).options.columns[2], first);
    assert_eq!(bom!(b).column_drag, None);
}

#[test]
fn a_heading_click_cycles_ascending_descending_and_no_sort() {
    let mut b = opened();
    let designator = bom!(b)
        .options
        .columns
        .iter()
        .position(|c| *c == BomColumn::Designator)
        .expect("a designator column");
    assert_eq!(
        bom!(b).sort,
        Some((designator, true)),
        "opens by designator"
    );
    send(&mut b, BomPreviewMsg::SortColumn(designator));
    assert_eq!(bom!(b).sort, Some((designator, false)));
    send(&mut b, BomPreviewMsg::SortColumn(designator));
    assert_eq!(bom!(b).sort, None);
}

#[test]
fn a_rule_turned_off_takes_its_issues_out_and_back() {
    let mut b = opened();
    assert!(has_issue(&b, BomRule::MissingMpn));
    send(&mut b, BomPreviewMsg::SetRule(BomRule::MissingMpn, false));
    assert!(!has_issue(&b, BomRule::MissingMpn));
    send(&mut b, BomPreviewMsg::SetRule(BomRule::MissingMpn, true));
    assert!(has_issue(&b, BomRule::MissingMpn));
}

#[test]
fn clicking_an_issue_selects_its_line_even_behind_a_filter() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::FilterChanged("no such part".into()));
    send(&mut b, BomPreviewMsg::IssueSelected(0));
    assert!(bom!(b).selected_row.is_some(), "its line is selected");
    assert!(bom!(b).filter.is_empty(), "the filter no longer hides it");
    assert!(!bom!(b).visible_lines.is_empty());
}

#[test]
fn the_variant_picker_sets_the_variant_and_base_clears_it() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::SetVariant(Some("Lite".into())));
    assert_eq!(bom!(b).options.active_variant.as_deref(), Some("Lite"));
    send(&mut b, BomPreviewMsg::SetVariant(None));
    assert_eq!(bom!(b).options.active_variant, None);
}

#[test]
fn cancel_closes_the_dialog_and_changes_nothing() {
    let mut b = opened();
    b.leaves_document(|b| {
        send(b, BomPreviewMsg::SetGrouping(BomGrouping::Flat));
        send(b, BomPreviewMsg::Close);
    });
    assert!(!is_open(&b), "the dialog closed");
    assert_eq!(std::fs::read_dir(b.dir.path()).expect("the dir").count(), 0);
}

#[test]
fn export_writes_the_format_picked() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::SetFormat(BomFormat::Xlsx));
    assert_eq!(bom!(b).options.format, BomFormat::Xlsx);
    let out = b.out("parts.xlsx");
    b.answer(&out);
    b.leaves_document(|b| send(b, BomPreviewMsg::Export));
    let bytes = std::fs::read(&out).expect("the workbook is written");
    assert!(bytes.starts_with(b"PK"), "an XLSX is a zip");
}

/// Row 112 follows the footer rule of its file: Cancel before an action,
/// Close after a completed one. So Export does not dismiss the dialog, and
/// a save dialog the user cancels loses none of its settings.
#[test]
fn a_cancelled_save_dialog_keeps_the_bom_dialog_and_its_settings() {
    let mut b = opened();
    send(&mut b, BomPreviewMsg::SetGrouping(BomGrouping::Flat));
    b.cancel_dialog();
    send(&mut b, BomPreviewMsg::Export);
    assert!(is_open(&b), "the dialog stays open");
    assert_eq!(bom!(b).options.grouping, BomGrouping::Flat);
}
