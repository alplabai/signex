use super::*;
use signex_erc::{RuleKind, Severity, Violation};
use signex_types::schematic::Point;

fn violation(rule: RuleKind, severity: Severity, msg: &str, x: f64) -> Violation {
    Violation::new(
        signex_erc::RuleId::builtin(rule),
        rule,
        severity,
        msg,
        Point { x, y: 2.0 },
    )
}

fn app_with_erc() -> Signex {
    let (mut app, _task) = Signex::new();
    app.ui_state.erc_violations_by_path.insert(
        PathBuf::from("b.snxsch"),
        vec![violation(
            RuleKind::DanglingWire,
            Severity::Warning,
            "w",
            5.0,
        )],
    );
    app.ui_state.erc_violations_by_path.insert(
        PathBuf::from("a.snxsch"),
        vec![
            violation(RuleKind::UnusedPin, Severity::Error, "e", 1.0),
            violation(RuleKind::UnusedPin, Severity::Info, "i", 3.0),
        ],
    );
    app
}

#[test]
fn rows_match_the_messages_panel() {
    let app = app_with_erc();
    let rows = app.erc_report_rows().expect("ERC has run");
    let panel = app.build_erc_diagnostic_entries();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows.len(), panel.len());
    for (row, entry) in rows.iter().zip(&panel) {
        assert_eq!(row.sheet_name, entry.sheet_name);
        assert_eq!(row.message, entry.message);
        assert_eq!(row.rule, entry.rule_label);
        assert_eq!((row.x, row.y), (entry.world_x, entry.world_y));
    }
    assert_eq!(rows[0].sheet_name, "a.snxsch");
    assert_eq!(rows[0].severity, ErcReportSeverity::Error);
    assert_eq!(rows[2].sheet_name, "b.snxsch");
}

#[test]
fn menu_entry_refuses_with_a_message_when_erc_never_ran() {
    let (mut app, _task) = Signex::new();
    assert!(app.document_state.error_notice.is_none());
    let _ = app.update(Message::Menu(MenuMessage::ExportErcReport));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(
        notice.detail.contains("ERC has not been run"),
        "{}",
        notice.detail
    );
}

#[test]
fn refusal_names_the_real_menu_path() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Menu(MenuMessage::ExportErcReport));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(
        notice
            .detail
            .contains(&crate::menu_bar::run_erc_menu_path()),
        "{}",
        notice.detail
    );
    assert!(notice.detail.contains("Design"), "{}", notice.detail);
    assert!(!notice.detail.contains("Tools"), "{}", notice.detail);
}

#[test]
fn picking_a_path_without_erc_surfaces_the_refusal() {
    let (mut app, _task) = Signex::new();
    let _ = app.update(Message::Export(ExportMsg::ErcReportPathChosen(Ok(
        PathBuf::from("erc.csv"),
    ))));
    let notice = app.document_state.error_notice.as_ref().expect("notice");
    assert!(
        notice.detail.contains("has not been run"),
        "{}",
        notice.detail
    );
}

#[test]
fn erc_ran_clean_exports_a_header_only_report_without_refusal() {
    let (mut app, _task) = Signex::new();
    app.ui_state
        .erc_violations_by_path
        .insert(PathBuf::from("clean.snxsch"), Vec::new());
    assert_eq!(app.erc_report_rows().expect("ERC has run").len(), 0);
    let _ = app.update(Message::Export(ExportMsg::ErcReportPathChosen(Ok(
        PathBuf::from("erc.txt"),
    ))));
    assert!(app.document_state.error_notice.is_none());
}

#[test]
fn same_file_name_in_two_directories_gets_distinct_sheet_keys() {
    let (mut app, _task) = Signex::new();
    for dir in ["one", "two"] {
        app.ui_state.erc_violations_by_path.insert(
            PathBuf::from(dir).join("power.snxsch"),
            vec![violation(RuleKind::UnusedPin, Severity::Error, "e", 1.0)],
        );
    }
    let rows = app.erc_report_rows().expect("ERC has run");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].sheet_name, rows[1].sheet_name);
    assert_ne!(rows[0].sheet_key, rows[1].sheet_key);
}
