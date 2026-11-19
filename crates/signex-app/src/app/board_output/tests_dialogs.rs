//! #116 / #117 through `Signex::update`: Mirror the Bottom Side waits for a
//! bottom set; Board Reports' filter and sort are view only (Export CSV
//! writes the same bytes), Net Lengths opens longest first, a heading click
//! turns the order, and another kind opens unfiltered.

use signex_output::{
    BoardLayerSet, BoardPrintOptions, BoardReport, BoardReportKind, ReportCell, ReportColumn,
};

use super::tests::{app_with_board, output, run};
use super::{BoardOutputDialog, BoardOutputMsg, BoardPrintFlag, ReportSort, shown_rows};
use crate::app::Signex;
use crate::app::handlers::menu::export::settle;

fn print_options(app: &Signex) -> BoardPrintOptions {
    match app.ui_state.board_output.dialog.as_deref() {
        Some(BoardOutputDialog::Print(state)) => state.options.clone(),
        _ => panic!("Board Print / PDF is open"),
    }
}

fn reports(app: &Signex) -> (Option<ReportSort>, String, BoardReport) {
    match app.ui_state.board_output.dialog.as_deref() {
        Some(BoardOutputDialog::Reports(state)) => {
            (state.sort, state.filter.clone(), state.report.clone())
        }
        _ => panic!("Board Reports is open"),
    }
}

#[test]
fn mirror_the_bottom_side_waits_for_a_bottom_set() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "print");
    let mirror = BoardOutputMsg::PrintFlagToggled(BoardPrintFlag::MirrorBottom);
    assert!(print_options(&app).mirror_bottom, "on by default");
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::Bottom),
    );
    assert!(!print_options(&app).has_bottom_set());
    let _ = output(&mut app, mirror.clone());
    assert!(
        print_options(&app).mirror_bottom,
        "no bottom set: the box is deaf and keeps its value"
    );
    let _ = output(
        &mut app,
        BoardOutputMsg::PrintSetToggled(BoardLayerSet::AssemblyBottom),
    );
    let _ = output(&mut app, mirror);
    assert!(
        !print_options(&app).mirror_bottom,
        "the bottom assembly drawing is a bottom set"
    );
}

#[test]
fn net_lengths_open_longest_first_and_a_heading_turns_the_order() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "board_report_net_length");
    let (sort, _, report) = reports(&app);
    assert_eq!(
        sort,
        Some(ReportSort {
            column: ReportColumn::Length,
            descending: true
        })
    );
    let longest = shown_rows(&report, "", sort)[0];
    let lengths: Vec<f64> = report
        .rows
        .iter()
        .filter_map(|row| match row[1] {
            ReportCell::Millimetres(mm) => Some(mm),
            _ => None,
        })
        .collect();
    let max = lengths.iter().copied().fold(f64::MIN, f64::max);
    assert_eq!(report.rows[longest][1], ReportCell::Millimetres(max));

    let _ = output(
        &mut app,
        BoardOutputMsg::ReportSortPicked(ReportColumn::Length),
    );
    assert_eq!(reports(&app).0.map(|s| s.descending), Some(false));
    let _ = output(
        &mut app,
        BoardOutputMsg::ReportSortPicked(ReportColumn::Net),
    );
    assert_eq!(
        reports(&app).0,
        Some(ReportSort {
            column: ReportColumn::Net,
            descending: false
        })
    );
}

#[test]
fn another_kind_opens_unfiltered_in_its_own_order() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, _) = app_with_board(dir.path());
    run(&mut app, "board_report_net_length");
    let _ = output(&mut app, BoardOutputMsg::ReportFilterEdited("vcc".into()));
    let _ = output(
        &mut app,
        BoardOutputMsg::ReportPicked(BoardReportKind::NetLength),
    );
    assert_eq!(reports(&app).1, "vcc", "the same kind keeps the filter");
    let _ = output(
        &mut app,
        BoardOutputMsg::ReportPicked(BoardReportKind::Components),
    );
    let (sort, filter, report) = reports(&app);
    assert_eq!(report.kind, BoardReportKind::Components);
    assert!(filter.is_empty() && sort.is_none());
}

#[test]
fn the_filter_and_the_sort_are_view_only_and_export_writes_the_same_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let (mut app, board) = app_with_board(dir.path());
    run(&mut app, "board_report_components");
    let write = |app: &mut Signex, name: &str| {
        let path = dir.path().join(name);
        let task = output(
            app,
            BoardOutputMsg::CsvPathChosen {
                board: board.clone(),
                kind: BoardReportKind::Components,
                path: path.clone(),
            },
        );
        settle(app, task);
        std::fs::read(&path).expect("the CSV was written")
    };
    let plain = write(&mut app, "plain.csv");
    // C1 alone is 100n.
    let _ = output(&mut app, BoardOutputMsg::ReportFilterEdited("100N".into()));
    let _ = output(&mut app, BoardOutputMsg::ReportSortPicked(ReportColumn::X));
    let _ = output(&mut app, BoardOutputMsg::ReportSortPicked(ReportColumn::X));
    let (sort, filter, report) = reports(&app);
    let shown = shown_rows(&report, &filter, sort);
    assert!(
        !shown.is_empty() && shown.len() < report.rows.len(),
        "the filter hides rows on screen: {shown:?}"
    );
    assert_eq!(write(&mut app, "viewed.csv"), plain, "every row, in order");
}

#[test]
fn the_filter_ignores_case_and_ties_keep_the_report_order() {
    let report = BoardReport {
        kind: BoardReportKind::Components,
        columns: vec![ReportColumn::Designator, ReportColumn::X],
        rows: vec![
            vec![ReportCell::Text("R10".into()), ReportCell::Millimetres(1.0)],
            vec![ReportCell::Text("R2".into()), ReportCell::Missing],
            vec![ReportCell::Text("C1".into()), ReportCell::Millimetres(1.0)],
            vec![ReportCell::Text("r3".into()), ReportCell::Millimetres(5.0)],
        ],
    };
    assert_eq!(shown_rows(&report, "", None), [0, 1, 2, 3]);
    assert_eq!(shown_rows(&report, " R ", None), [0, 1, 3], "case-blind");
    let by = |column, descending| Some(ReportSort { column, descending });
    assert_eq!(
        shown_rows(&report, "", by(ReportColumn::Designator, false)),
        [2, 1, 3, 0],
        "designator order: R2 before r3 before R10"
    );
    assert_eq!(
        shown_rows(&report, "", by(ReportColumn::X, false)),
        [0, 2, 3, 1],
        "ties keep their order; a missing value goes last"
    );
    assert_eq!(
        shown_rows(&report, "", by(ReportColumn::X, true)),
        [3, 0, 2, 1],
        "the missing value stays last the other way round"
    );
}
