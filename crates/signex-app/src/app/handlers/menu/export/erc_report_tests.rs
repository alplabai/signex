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

#[test]
fn html_report_from_the_panel_rows_escapes_net_names() {
    let mut app = app_with_erc();
    app.ui_state.erc_violations_by_path.insert(
        PathBuf::from("c.snxsch"),
        vec![violation(
            RuleKind::SinglePinNet,
            Severity::Warning,
            "Net '<script>x()</script>' connects only one pin, R1.1",
            7.0,
        )],
    );
    let opts = app.erc_report_options(ErcReportFormat::Html);
    let ctx = ExportContext {
        sheets: Vec::new(),
        metadata: ProjectMetadata::default(),
        netlist: None,
        netlist_sheets: Vec::new(),
        current_sheet: None,
        netlist_placements: Vec::new(),
    };
    let html = String::from_utf8(ErcReportExporter.export(&ctx, &opts).expect("html").bytes)
        .expect("utf-8");
    assert!(!html.contains("<script"));
    assert!(html.contains("Net &#39;&lt;script&gt;x()&lt;/script&gt;&#39;"));
    assert!(html.contains("c.snxsch"));
}

#[test]
fn waivers_map_into_the_report_with_their_status() {
    use crate::panels::{ErcViolationWaiverEntry, ErcWaiverStatus};
    let entry = ErcViolationWaiverEntry {
        rule_id: "builtin::unused_pin".into(),
        objects: vec!["u".into()],
        message: "Pin on TP1 is not connected".into(),
        reason: "test point".into(),
        author: "A. Designer".into(),
        date: "2026-10-29".into(),
        status: ErcWaiverStatus::Lapsed,
        severity: Some(ErcSeverityLite::Warning),
    };
    let w = report_waiver(&entry);
    assert_eq!(w.status, ErcReportWaiverStatus::Lapsed);
    assert_eq!((w.reason.as_str(), w.waived_count), ("test point", 0));
    assert_eq!(w.severity, Some(ErcReportSeverity::Warning));
    let net = report_net_waiver(&crate::panels::ErcWaiverEntry {
        rule_id: "user::r".into(),
        net: "VBUS".into(),
        hidden: 2,
    });
    assert_eq!(net.kind, ErcReportWaiverKind::PerNet);
    assert_eq!(net.waived_count, 2);
}

#[test]
fn report_sheet_paths_are_relative_to_the_project_folder() {
    let dir = std::path::Path::new("/home/me/proj");
    assert_eq!(
        report_sheet_path(&dir.join("sheets").join("power.snxsch"), Some(dir)),
        std::path::Path::new("sheets")
            .join("power.snxsch")
            .display()
            .to_string()
    );
    let outside = std::path::Path::new("/elsewhere/x.snxsch");
    assert_eq!(
        report_sheet_path(outside, Some(dir)),
        outside.display().to_string()
    );
}

#[test]
fn the_report_is_formatted_and_written_off_the_update_thread() {
    let mut app = app_with_erc();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("erc.html");
    let task = app.handle_erc_report_path_chosen(Ok(path.clone()));
    assert!(!path.exists(), "nothing is written inside update");
    crate::app::handlers::menu::export::write::settle(&mut app, task);
    let html = std::fs::read_to_string(&path).expect("written");
    assert!(html.starts_with("<!DOCTYPE html>"));
}
