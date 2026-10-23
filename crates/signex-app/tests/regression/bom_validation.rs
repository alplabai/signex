//! #355 — the BOM export modal surfaces the `signex-bom` validation report
//! and per-rule toggles. Drives the real message path (`ExportBom` menu ->
//! `BomPreviewMsg::SetRule` -> `ExportMsg::BomFinished`) against a real
//! engine and a real temp dir.

use signex_app::app::{BomPreviewMsg, ExportMsg, Message, Signex};
use signex_app::menu_bar::MenuMessage;
use signex_output::BomRule;
use signex_types::schematic::{SchematicSheet, Symbol};
use std::path::PathBuf;

fn symbol(reference: &str, footprint: &str) -> Symbol {
    Symbol {
        uuid: uuid::Uuid::new_v4(),
        lib_id: "Device:R".to_string(),
        reference: reference.to_string(),
        value: "10k".to_string(),
        footprint: footprint.to_string(),
        ..Default::default()
    }
}

/// Two `R1`s, one without a footprint: missing-footprint + duplicate
/// designator (plus missing-MPN, which each test switches off).
fn app_with_faulty_bom() -> Signex {
    let sheet = SchematicSheet {
        uuid: uuid::Uuid::new_v4(),
        paper_size: "A4".to_string(),
        symbols: vec![symbol("R1", ""), symbol("R1", "R_0603")],
        ..Default::default()
    };
    let path = PathBuf::from("bom-validation.snxsch");
    let engine = signex_engine::Engine::new(sheet).expect("engine");
    let (mut app, _t) = Signex::new();
    app.document_state.engines.insert(path.clone(), engine);
    app.document_state.tabs.push(signex_app::app::TabInfo {
        title: "bom-validation".to_string(),
        path: path.clone(),
        cached_document: None,
        dirty: false,
        project_id: None,
        kind: signex_app::app::TabKind::Schematic,
    });
    app.document_state.active_tab = 0;
    app.document_state.active_path = Some(path);
    app
}

fn issue_rules(app: &Signex) -> Vec<BomRule> {
    let preview = app
        .document_state
        .bom_preview
        .as_ref()
        .expect("preview open");
    preview
        .validation_report
        .issues
        .iter()
        .map(|i| i.rule)
        .collect()
}

fn set_rule(app: &mut Signex, rule: BomRule, on: bool) {
    let _ = app.update(Message::BomPreview(BomPreviewMsg::SetRule(rule, on)));
}

#[test]
fn opening_the_modal_runs_validation_and_toggles_change_the_report() {
    let mut app = app_with_faulty_bom();
    let _ = app.update(Message::Menu(MenuMessage::ExportBom));
    set_rule(&mut app, BomRule::MissingMpn, false);
    let rules = issue_rules(&app);
    assert_eq!(rules.len(), 2, "{rules:?}");
    assert!(rules.contains(&BomRule::MissingFootprint));
    assert!(rules.contains(&BomRule::DuplicateDesignator));

    set_rule(&mut app, BomRule::DuplicateDesignator, false);
    assert_eq!(issue_rules(&app), vec![BomRule::MissingFootprint]);

    set_rule(&mut app, BomRule::DuplicateDesignator, true);
    assert_eq!(issue_rules(&app).len(), 2);
}

#[test]
fn a_rollup_change_refreshes_the_report() {
    let mut app = app_with_faulty_bom();
    let _ = app.update(Message::Menu(MenuMessage::ExportBom));
    set_rule(&mut app, BomRule::MissingMpn, false);
    assert_eq!(issue_rules(&app).len(), 2);
    // Clear the stored report, then trigger a rollup: the handler must
    // recompute it rather than leave it stale.
    app.document_state
        .bom_preview
        .as_mut()
        .expect("preview open")
        .validation_report
        .issues
        .clear();
    let _ = app.update(Message::BomPreview(BomPreviewMsg::SetIncludeDnp(true)));
    assert_eq!(issue_rules(&app).len(), 2);
}

#[test]
fn export_with_issues_still_writes_and_reports_the_summary() {
    let dir = std::env::temp_dir().join(format!("signex-bom-355-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let out = dir.join("bom.csv");

    let mut app = app_with_faulty_bom();
    let _ = app.update(Message::Menu(MenuMessage::ExportBom));
    set_rule(&mut app, BomRule::MissingMpn, false);
    let options = app
        .document_state
        .bom_preview
        .as_ref()
        .expect("preview open")
        .options
        .clone();
    app.document_state.pending_bom_options = Some(options);

    let _ = app.update(Message::Export(ExportMsg::BomFinished(Ok(out.clone()))));

    assert!(out.exists(), "advisory validation must not block the write");
    let notice = app
        .document_state
        .error_notice
        .as_ref()
        .expect("summary must be surfaced, not dropped");
    assert!(notice.title.contains("Issues"), "{notice:?}");
    assert!(notice.detail.contains("1 error, 1 warning"), "{notice:?}");
    std::fs::remove_dir_all(&dir).expect("cleanup");
}
