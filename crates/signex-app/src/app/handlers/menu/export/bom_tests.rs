//! Mockup row 112: the BOM dialog's table state (filter, flags, issue lines) and the
//! one not-fitted option.

use std::path::PathBuf;

use signex_output::{
    BomColumn, BomIssueSeverity, BomRow, BomRule, BomValidationIssue, BomValidationReport,
};
use signex_types::schematic::{SchematicSheet, Symbol};

use super::rows::{issue_lines, kept_lines, row_flags};
use crate::app::state::BomIssueLine;
use crate::app::{BomPreviewMsg, Message, Signex};

fn line(references: &[&str], value: &str, footprint: &str) -> BomRow {
    BomRow {
        references: references.iter().map(|r| r.to_string()).collect(),
        qty: references.len() as u32,
        value: value.to_string(),
        footprint: footprint.to_string(),
        ..BomRow::default()
    }
}

fn issue(rule: BomRule, severity: BomIssueSeverity, references: &[&str]) -> BomValidationIssue {
    BomValidationIssue {
        rule,
        severity,
        message: String::new(),
        references: references.iter().map(|r| r.to_string()).collect(),
    }
}

fn table() -> Vec<BomRow> {
    vec![
        line(&["C1", "C2"], "10uF", "C_0805"),
        line(&["J2"], "Conn_01x04", "PinHeader_1x04"),
        line(&["TP1", "TP2"], "TestPoint", ""),
    ]
}

/// The filter keeps the lines with a shown cell that contains it, in any
/// case; a column that is not shown is not searched, and blank text keeps
/// every line.
#[test]
fn the_filter_keeps_lines_whose_shown_cells_match() {
    let rows = table();
    let order = vec![2, 0, 1];
    let shown = [BomColumn::Designator, BomColumn::Value];
    assert_eq!(kept_lines(&rows, &order, &shown, ""), vec![0, 1, 2]);
    assert_eq!(kept_lines(&rows, &order, &shown, "   "), vec![0, 1, 2]);
    // "conn" is in J2's Value, the third line of `order`.
    assert_eq!(kept_lines(&rows, &order, &shown, "CONN"), vec![2]);
    assert_eq!(kept_lines(&rows, &order, &shown, " tp2 "), vec![0]);
    // The footprint column is not shown, so its text does not match.
    assert!(kept_lines(&rows, &order, &shown, "0805").is_empty());
    let with_footprint = [BomColumn::Designator, BomColumn::Footprint];
    assert_eq!(kept_lines(&rows, &order, &with_footprint, "0805"), vec![1]);
}

/// A warning or error flags each line holding one of its designators, the
/// error winning; notes flag nothing.
#[test]
fn issues_flag_the_lines_of_their_designators() {
    let rows = table();
    let report = BomValidationReport {
        issues: vec![
            issue(BomRule::MissingMpn, BomIssueSeverity::Warning, &["J2"]),
            issue(
                BomRule::MissingFootprint,
                BomIssueSeverity::Warning,
                &["TP1"],
            ),
            issue(
                BomRule::DuplicateDesignator,
                BomIssueSeverity::Error,
                &["TP2"],
            ),
            issue(
                BomRule::MissingFootprint,
                BomIssueSeverity::Warning,
                &["TP2"],
            ),
            issue(BomRule::MissingMpn, BomIssueSeverity::Info, &["C1"]),
        ],
    };
    assert_eq!(
        row_flags(&rows, &report),
        vec![
            None,
            Some(BomIssueSeverity::Warning),
            Some(BomIssueSeverity::Error),
        ]
    );
}

/// Each issue points at the line of its first designator on the table, by
/// the number the gutter shows; one whose parts are not listed points at
/// nothing.
#[test]
fn each_issue_names_the_line_it_is_on() {
    let rows = table();
    let order = vec![2, 0, 1];
    let report = BomValidationReport {
        issues: vec![
            issue(BomRule::MissingMpn, BomIssueSeverity::Warning, &["J2"]),
            issue(
                BomRule::MissingFootprint,
                BomIssueSeverity::Warning,
                &["R9", "TP2"],
            ),
            issue(BomRule::MissingMpn, BomIssueSeverity::Warning, &["R9"]),
        ],
    };
    assert_eq!(
        issue_lines(&rows, &order, &report),
        vec![
            Some(BomIssueLine { row: 1, line: 3 }),
            Some(BomIssueLine { row: 2, line: 1 }),
            None,
        ]
    );
}

fn symbol(reference: &str, footprint: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::now_v7(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: footprint.to_string(),
        ..Default::default()
    }
}

/// An app with one sheet open: R1 without a footprint, R2 with one.
fn app_with_sheet() -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::now_v7(),
        paper_size: "A4".to_string(),
        symbols: vec![symbol("R1", ""), symbol("R2", "R_0603")],
        ..Default::default()
    };
    let path = PathBuf::from("bom-112.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _boot) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(crate::app::TabInfo {
        title: "bom-112".to_string(),
        path: path.clone(),
        cached_document: None,
        project_id: None,
        kind: crate::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    let _ = app.handle_bom_preview_open();
    app
}

fn options(app: &Signex) -> signex_output::BomOptions {
    app.document_state
        .bom_preview
        .as_ref()
        .expect("preview open")
        .options
        .clone()
}

/// docs/DECISIONS.md, 2026-11-19: the one not-fitted option sets both
/// domain flags, on and off, and reads as ticked only while both are on.
#[test]
fn the_not_fitted_option_sets_both_flags_together() {
    let mut app = app_with_sheet();
    let start = options(&app);
    assert!(!start.include_dnp && !start.include_not_fitted);

    let _ = app.update(Message::BomPreview(
        BomPreviewMsg::NotFittedIncludedToggled(true),
    ));
    let on = options(&app);
    assert!(on.include_dnp && on.include_not_fitted);
    assert!(
        app.document_state
            .bom_preview
            .as_ref()
            .unwrap()
            .includes_not_fitted()
    );

    let _ = app.update(Message::BomPreview(
        BomPreviewMsg::NotFittedIncludedToggled(false),
    ));
    let off = options(&app);
    assert!(!off.include_dnp && !off.include_not_fitted);

    // A state with the flags apart reads as clear, so no part enters the
    // export behind a box that looks ticked off.
    let preview = app.document_state.bom_preview.as_mut().unwrap();
    preview.options.include_dnp = true;
    assert!(!preview.includes_not_fitted());
}

fn written(result: Result<(), String>) -> crate::app::contracts::ExportWriteOutcome {
    crate::app::contracts::ExportWriteOutcome {
        path: PathBuf::from("bom-112.csv"),
        result,
        success_log: String::new(),
        advisory: None,
        warnings: Vec::new(),
        preview_source: None,
    }
}

/// Mockup rows 111 and 112: Export and a cancelled save leave the dialog and
/// its settings; a failed write leaves them too; a completed one closes it.
#[test]
fn the_dialog_closes_only_after_a_completed_export() {
    let mut app = app_with_sheet();
    let source = app.document_state.active_path.clone().expect("active");
    let _ = app.update(Message::BomPreview(BomPreviewMsg::SetGrouping(
        signex_output::BomGrouping::Flat,
    )));
    let _ = app.handle_bom_preview_export();
    assert!(app.document_state.bom_preview.is_some(), "Export keeps it");

    let opts = options(&app);
    let _ = app.handle_export_bom_finished(source.clone(), opts, Err("cancelled".to_string()));
    assert_eq!(options(&app).grouping, signex_output::BomGrouping::Flat);

    let _ = app.handle_bom_written(source.clone(), written(Err("disk full".to_string())));
    assert!(app.document_state.bom_preview.is_some(), "a failed write");

    let other = PathBuf::from("another-sheet.snxsch");
    let _ = app.handle_bom_written(other, written(Ok(())));
    assert!(app.document_state.bom_preview.is_some(), "another document");

    let _ = app.handle_bom_written(source, written(Ok(())));
    assert!(app.document_state.bom_preview.is_none(), "written, closed");
}

/// The filter hides lines on screen only: the options Export writes from
/// are unchanged, and a filter that matches nothing leaves the table whole.
#[test]
fn the_filter_never_changes_the_export() {
    let mut app = app_with_sheet();
    let before = options(&app);
    let rows_before = app
        .document_state
        .bom_preview
        .as_ref()
        .unwrap()
        .table
        .rows
        .len();
    let _ = app.update(Message::BomPreview(BomPreviewMsg::FilterChanged(
        "no such part".into(),
    )));
    let preview = app.document_state.bom_preview.as_ref().unwrap();
    assert!(preview.visible_lines.is_empty());
    assert_eq!(preview.table.rows.len(), rows_before);
    assert_eq!(preview.options.columns, before.columns);
    assert_eq!(preview.options.grouping, before.grouping);
    assert_eq!(preview.options.format, before.format);

    let _ = app.update(Message::BomPreview(BomPreviewMsg::FilterChanged(
        String::new(),
    )));
    let preview = app.document_state.bom_preview.as_ref().unwrap();
    assert_eq!(preview.visible_lines.len(), preview.row_order.len());
}

/// A click on an issue selects its line; a filter hiding that line is
/// cleared so the selection is on screen.
#[test]
fn an_issue_click_selects_its_line_and_clears_a_hiding_filter() {
    let mut app = app_with_sheet();
    let (issue, row) = {
        let preview = app.document_state.bom_preview.as_ref().unwrap();
        let at = preview
            .validation_report
            .issues
            .iter()
            .position(|i| i.rule == BomRule::MissingFootprint)
            .expect("R1 has no footprint");
        (at, preview.issue_lines[at].expect("R1 is on a line").row)
    };
    let _ = app.update(Message::BomPreview(BomPreviewMsg::FilterChanged(
        "no such part".into(),
    )));
    let _ = app.update(Message::BomPreview(BomPreviewMsg::IssueSelected(issue)));
    let preview = app.document_state.bom_preview.as_ref().unwrap();
    assert_eq!(preview.selected_row, Some(row));
    assert!(preview.filter.is_empty());
    assert!(!preview.visible_lines.is_empty());
}

/// A flagged line carries its flag once the dialog opens; a grouping change
/// re-rolls the table and drops a selection that named an old line.
#[test]
fn flags_follow_the_report_and_a_reroll_drops_the_selection() {
    let mut app = app_with_sheet();
    {
        let preview = app.document_state.bom_preview.as_ref().unwrap();
        assert_eq!(preview.row_flags.len(), preview.table.rows.len());
        assert!(
            preview.row_flags.iter().any(Option::is_some),
            "R1 is flagged"
        );
    }
    let _ = app.update(Message::BomPreview(BomPreviewMsg::RowSelected(0)));
    assert_eq!(
        app.document_state
            .bom_preview
            .as_ref()
            .unwrap()
            .selected_row,
        Some(0)
    );
    let _ = app.update(Message::BomPreview(BomPreviewMsg::SetGrouping(
        signex_output::BomGrouping::Flat,
    )));
    assert_eq!(
        app.document_state
            .bom_preview
            .as_ref()
            .unwrap()
            .selected_row,
        None
    );
}
