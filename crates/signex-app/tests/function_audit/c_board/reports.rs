//! Mockup row 117, Board Reports with its Net Lengths state (decisions
//! 2026-11-16 and 2026-11-19): the four kinds, the filter, click-to-sort
//! headings, Cancel and Export CSV. Read only: no undo step, no dirty mark.

use signex_app::app::{BoardOutputMsg, Message};
use signex_output::{BoardReportKind, ReportCell, ReportColumn};

use super::support::{Bench, audit_board, open_board};

fn send(b: &mut Bench, msg: BoardOutputMsg) {
    b.send(Message::from(msg));
}

fn opened(command: &str) -> Bench {
    let mut b = open_board(audit_board());
    b.command(command);
    b
}

/// The text of column `at` in the rows the table shows, in order.
fn shown(b: &Bench, at: usize) -> Vec<String> {
    let state = b.reports();
    state
        .shown
        .iter()
        .map(|&row| state.report.rows[row][at].csv_text())
        .collect()
}

fn column(b: &Bench, wanted: ReportColumn) -> usize {
    b.reports()
        .report
        .columns
        .iter()
        .position(|c| *c == wanted)
        .expect("the report has the column")
}

/// The cell of `wanted` on the row whose first cell is `key`.
fn cell(b: &Bench, key: &str, wanted: ReportColumn) -> ReportCell {
    let at = column(b, wanted);
    b.reports()
        .report
        .rows
        .iter()
        .find(|row| row[0].csv_text() == key)
        .map(|row| row[at].clone())
        .unwrap_or_else(|| panic!("a row for {key}"))
}

#[test]
fn each_report_command_opens_its_kind() {
    for (command, kind) in [
        ("board_report_bom", BoardReportKind::Bom),
        ("board_report_netlist", BoardReportKind::Netlist),
        ("board_report_net_length", BoardReportKind::NetLength),
        ("board_report_components", BoardReportKind::Components),
    ] {
        let mut b = open_board(audit_board());
        b.leaves_document(|b| b.command(command));
        assert_eq!(b.reports().report.kind, kind, "{command}");
        assert!(!b.can_undo_board(), "a report makes no undo step");
    }
}

#[test]
fn net_lengths_open_longest_first_with_the_routed_length_tracks_and_vias() {
    let b = opened("board_report_net_length");
    assert_eq!(shown(&b, 0), ["VCC", "GND", "SPARE"], "longest first");
    assert_eq!(
        cell(&b, "VCC", ReportColumn::Length),
        ReportCell::Millimetres(30.0)
    );
    assert_eq!(
        cell(&b, "GND", ReportColumn::Length),
        ReportCell::Millimetres(10.0)
    );
    assert_eq!(cell(&b, "GND", ReportColumn::Tracks), ReportCell::Count(1));
    assert_eq!(cell(&b, "GND", ReportColumn::Vias), ReportCell::Count(1));
    assert_eq!(cell(&b, "VCC", ReportColumn::Vias), ReportCell::Count(0));
    assert_eq!(
        cell(&b, "SPARE", ReportColumn::Tracks),
        ReportCell::Count(0)
    );
}

#[test]
fn a_heading_click_turns_the_order_and_another_heading_sorts_by_it() {
    let mut b = opened("board_report_net_length");
    send(
        &mut b,
        BoardOutputMsg::ReportSortPicked(ReportColumn::Length),
    );
    assert_eq!(shown(&b, 0), ["SPARE", "GND", "VCC"], "shortest first");
    send(&mut b, BoardOutputMsg::ReportSortPicked(ReportColumn::Net));
    assert_eq!(shown(&b, 0), ["GND", "SPARE", "VCC"], "by name");
}

#[test]
fn the_filter_ignores_case_and_only_hides_rows() {
    let mut b = opened("board_report_net_length");
    b.leaves_document(|b| send(b, BoardOutputMsg::ReportFilterEdited("gNd".into())));
    assert_eq!(shown(&b, 0), ["GND"]);
    assert_eq!(b.reports().report.rows.len(), 3, "the table keeps its rows");
}

#[test]
fn a_filter_that_matches_nothing_shows_no_row_and_keeps_its_text() {
    let mut b = opened("board_report_net_length");
    send(
        &mut b,
        BoardOutputMsg::ReportFilterEdited("no such net".into()),
    );
    assert!(b.reports().shown.is_empty(), "the empty state shows");
    assert_eq!(b.reports().filter, "no such net", "the text is kept");
}

#[test]
fn another_kind_opens_unfiltered_and_the_same_kind_keeps_the_filter() {
    let mut b = opened("board_report_net_length");
    send(&mut b, BoardOutputMsg::ReportFilterEdited("vcc".into()));
    send(
        &mut b,
        BoardOutputMsg::ReportPicked(BoardReportKind::NetLength),
    );
    assert_eq!(b.reports().filter, "vcc", "the same kind keeps it");
    send(
        &mut b,
        BoardOutputMsg::ReportPicked(BoardReportKind::Components),
    );
    assert_eq!(b.reports().report.kind, BoardReportKind::Components);
    assert!(b.reports().filter.is_empty());
    assert_eq!(b.reports().sort, None);
}

#[test]
fn the_bom_report_groups_parts_by_value_and_footprint() {
    let b = opened("board_report_bom");
    assert_eq!(shown(&b, 0), ["C1", "R1"]);
    assert_eq!(cell(&b, "R1", ReportColumn::Quantity), ReportCell::Count(1));
    assert_eq!(
        cell(&b, "C1", ReportColumn::Value),
        ReportCell::Text("100n".to_string())
    );
}

#[test]
fn the_netlist_report_lists_each_nets_pads() {
    let b = opened("board_report_netlist");
    assert_eq!(
        cell(&b, "GND", ReportColumn::NodeCount),
        ReportCell::Count(2)
    );
    assert_eq!(
        cell(&b, "GND", ReportColumn::Nodes),
        ReportCell::Text("C1.1 R1.1".to_string())
    );
    assert_eq!(
        cell(&b, "SPARE", ReportColumn::NodeCount),
        ReportCell::Count(0)
    );
}

#[test]
fn the_component_report_gives_each_parts_place() {
    let b = opened("board_report_components");
    assert_eq!(shown(&b, 0), ["C1", "R1"], "designator order");
    assert_eq!(
        cell(&b, "R1", ReportColumn::X),
        ReportCell::Millimetres(5.0)
    );
    assert_eq!(
        cell(&b, "C1", ReportColumn::X),
        ReportCell::Millimetres(15.0)
    );
}

#[test]
fn cancel_closes_the_dialog_and_changes_nothing() {
    let mut b = opened("board_report_net_length");
    b.leaves_document(|b| send(b, BoardOutputMsg::Closed));
    assert!(b.app.ui_state.board_output.dialog.is_none(), "it closed");
    assert!(!b.out("audit-net_length.csv").exists());
}

#[test]
fn export_csv_writes_every_row_whatever_the_filter_and_sort_show() {
    let mut b = opened("board_report_net_length");
    let plain = b.out("plain.csv");
    b.answer(&plain);
    send(&mut b, BoardOutputMsg::CsvExportRequested);
    let unfiltered = std::fs::read(&plain).expect("the CSV is written");

    send(&mut b, BoardOutputMsg::ReportFilterEdited("vcc".into()));
    send(&mut b, BoardOutputMsg::ReportSortPicked(ReportColumn::Net));
    let viewed = b.out("viewed.csv");
    b.answer(&viewed);
    b.leaves_document(|b| send(b, BoardOutputMsg::CsvExportRequested));
    assert_eq!(
        std::fs::read(&viewed).expect("the CSV is written"),
        unfiltered,
        "the export is the same bytes"
    );
    let text = String::from_utf8_lossy(&unfiltered);
    for net in ["GND", "VCC", "SPARE"] {
        assert!(text.contains(net), "{net} is in the file");
    }
    assert!(b.app.ui_state.board_output.dialog.is_some(), "it stays");
}

#[test]
fn a_cancelled_csv_save_writes_nothing_and_keeps_the_dialog() {
    let mut b = opened("board_report_net_length");
    b.cancel_dialog();
    b.leaves_document(|b| send(b, BoardOutputMsg::CsvExportRequested));
    assert!(b.app.ui_state.board_output.dialog.is_some(), "it stays");
    assert!(!b.out("audit-net_length.csv").exists());
}
