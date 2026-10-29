//! Per-finding ERC waivers with a reason (#1285): the panel flow, reason
//! required, waived findings stay listed, lapse on an object edit, undo, and
//! a save + load round trip.

use signex_types::format::SnxSchematic;
use signex_types::schematic::SchematicSheet;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};
use crate::panels::{ErcWaiverMsg, ErcWaiverStatus};

const RULE: &str = "user::net_check";

/// Top sheet `top` with net `NET_T` (part `R_T`), child with `NET_C`, and a
/// DSL rule that flags both nets.
fn app_with_two_failing_nets() -> (Signex, std::path::PathBuf, SchematicSheet) {
    let dir = std::env::temp_dir().join(format!("signex-erc-vwaiver-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let text = SnxSchematic::new(sheet_with_net("R_C", "NET_C", &[]))
        .write_string()
        .expect("serialize");
    std::fs::write(dir.join("child.snxsch"), text).expect("write child");
    std::fs::write(
        dir.join("erc.dsl"),
        r#"rule net_check on net when net.name matches "^NET_" then error "bad net""#,
    )
    .expect("write dsl");
    let mut app = app_workspace(&dir.to_string_lossy(), &["top.snxsch"]);
    let top_path = dir.join("top.snxsch");
    let top = sheet_with_net("R_T", "NET_T", &["child.snxsch"]);
    open_with(&mut app.document_state, &top_path, top.clone());
    app.document_state.active_path = Some(top_path);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    (app, dir, top)
}

fn flagged(app: &Signex) -> Vec<String> {
    let mut nets: Vec<String> = app
        .ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .filter(|v| v.rule_id.to_string() == RULE)
        .filter_map(|v| v.net.as_ref().map(|n| n.name.clone()))
        .collect();
    nets.sort();
    nets.dedup();
    nets
}

fn send(app: &mut Signex, msg: ErcWaiverMsg) {
    let _ = app.handle_erc_waiver(&msg);
}

/// Waive the `NET_T` finding through the panel flow with `reason`.
fn waive_net_t(app: &mut Signex, reason: &str) {
    let entry = app
        .build_erc_diagnostic_entries()
        .into_iter()
        .find(|e| e.rule_id == RULE && e.net.as_ref().is_some_and(|n| n.name == "NET_T"))
        .expect("NET_T row");
    send(
        app,
        ErcWaiverMsg::Started {
            key: entry.waiver_key.expect("waivable"),
            message: entry.message,
        },
    );
    send(app, ErcWaiverMsg::ReasonChanged(reason.into()));
    send(app, ErcWaiverMsg::AuthorChanged("A. Designer".into()));
    send(app, ErcWaiverMsg::Confirmed);
    app.run_erc_blocking();
}

#[test]
fn a_waived_finding_is_hidden_from_results_but_listed_as_waived() {
    let (mut app, dir, _) = app_with_two_failing_nets();
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C", "NET_T"]);

    waive_net_t(&mut app, "pulled up off-board");
    assert_eq!(flagged(&app), ["NET_C"]);
    assert_eq!(app.ui_state.erc_waived_findings.len(), 1);
    let entries = app.erc_violation_waiver_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].status, ErcWaiverStatus::Waiving(1));
    assert_eq!(entries[0].reason, "pulled up off-board");
    assert_eq!(entries[0].author, "A. Designer");
    assert!(app.ui_state.erc_waive_draft.is_none());

    send(
        &mut app,
        ErcWaiverMsg::Removed {
            rule_id: entries[0].rule_id.clone(),
            objects: entries[0].objects.clone(),
        },
    );
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C", "NET_T"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_blank_reason_is_refused_and_keeps_the_editor_open() {
    let (mut app, dir, _) = app_with_two_failing_nets();
    app.run_erc_blocking();
    waive_net_t(&mut app, "   ");
    assert_eq!(flagged(&app), ["NET_C", "NET_T"]);
    let draft = app
        .ui_state
        .erc_waive_draft
        .as_ref()
        .expect("editor stays open");
    assert!(draft.error.is_some());
    assert!(app.erc_violation_waiver_entries().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn editing_an_involved_part_lapses_the_waiver() {
    let (mut app, dir, top) = app_with_two_failing_nets();
    app.run_erc_blocking();
    waive_net_t(&mut app, "accepted");
    assert_eq!(flagged(&app), ["NET_C"]);

    // A value edit is outside what a net finding depends on.
    let top_path = dir.join("top.snxsch");
    let mut revalued = top.clone();
    revalued.symbols[0].value = "22k".into();
    open_with(&mut app.document_state, &top_path, revalued);
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C"], "a value edit keeps the waiver");

    // Renaming the part changes the net's pin set.
    let mut edited = top;
    edited.symbols[0].reference = "R_X".into();
    open_with(&mut app.document_state, &top_path, edited);
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C", "NET_T"], "the finding is back");
    assert_eq!(
        app.erc_violation_waiver_entries()[0].status,
        ErcWaiverStatus::Lapsed
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_waiver_persists_through_save_and_load_and_undoes() {
    let (mut app, dir, _) = app_with_two_failing_nets();
    app.run_erc_blocking();
    waive_net_t(&mut app, "accepted");
    let project = &app.document_state.projects[0];
    signex_types::project::write_project(&project.path, &project.data).expect("save");
    let loaded = signex_types::project::parse_project(&project.path).expect("load");
    assert_eq!(loaded.rules, project.data.rules);
    app.document_state.projects[0].data = loaded;
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C"]);

    assert!(app.undo_project_rules());
    assert!(app.erc_violation_waiver_entries().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

/// Waive the first finding `pick` selects, through the panel flow.
fn waive_where(
    app: &mut Signex,
    pick: impl Fn(&crate::panels::ErcDiagnosticEntry) -> bool,
) -> crate::panels::ErcDiagnosticEntry {
    let entry = app
        .build_erc_diagnostic_entries()
        .into_iter()
        .find(|e| pick(e))
        .expect("finding to waive");
    send(
        app,
        ErcWaiverMsg::Started {
            key: entry.waiver_key.clone().expect("waivable"),
            message: entry.message.clone(),
        },
    );
    send(app, ErcWaiverMsg::ReasonChanged("accepted".into()));
    send(app, ErcWaiverMsg::AuthorChanged("A. Designer".into()));
    send(app, ErcWaiverMsg::Confirmed);
    entry
}

/// The top sheet with a lone wire at y = 3.81 mm, which reads back from its
/// stored nm as 3.8099999999999996.
fn top_with_drifting_wire(top: &SchematicSheet) -> (SchematicSheet, Uuid) {
    let mut sheet = top.clone();
    let id = Uuid::new_v4();
    sheet.wires.push(signex_types::schematic::Wire {
        uuid: id,
        start: signex_types::schematic::Point::new(25.4, 3.81),
        end: signex_types::schematic::Point::new(38.1, 3.81),
        ..Default::default()
    });
    (sheet, id)
}

fn dangling_on(app: &Signex, wire: Uuid) -> usize {
    app.ui_state
        .erc_violations_by_path
        .values()
        .flatten()
        .filter(|v| v.builtin_kind() == Some(signex_erc::RuleKind::DanglingWire))
        .filter(|v| v.primary.is_some_and(|p| p.uuid == wire))
        .count()
}

#[test]
fn a_waiver_still_holds_after_the_sheet_is_saved_and_reloaded() {
    // A geometry finding, so the fingerprint hashes the drifting coordinate.
    let (mut app, dir, top) = app_with_two_failing_nets();
    let top_path = dir.join("top.snxsch");
    let (sheet, wire) = top_with_drifting_wire(&top);
    open_with(&mut app.document_state, &top_path, sheet.clone());
    app.run_erc_blocking();
    assert_eq!(dangling_on(&app, wire), 2);
    waive_where(&mut app, |e| {
        e.rule_kind == Some(signex_erc::RuleKind::DanglingWire)
            && e.select.is_some_and(|s| s.uuid == wire)
    });
    app.run_erc_blocking();
    assert_eq!(dangling_on(&app, wire), 0);

    let text = SnxSchematic::new(sheet.clone())
        .write_string()
        .expect("save sheet");
    let reloaded = SnxSchematic::parse(&text).expect("load sheet").sheet;
    let y = |s: &SchematicSheet| {
        s.wires
            .iter()
            .find(|w| w.uuid == wire)
            .map(|w| w.start.y.to_bits())
    };
    assert_ne!(
        y(&reloaded),
        y(&sheet),
        "the coordinate must drift on reload"
    );
    open_with(&mut app.document_state, &top_path, reloaded);
    app.run_erc_blocking();
    assert_eq!(dangling_on(&app, wire), 0, "no edit, so the waiver holds");
    assert_eq!(
        app.erc_violation_waiver_entries()[0].status,
        ErcWaiverStatus::Waiving(2)
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_part_joining_a_waived_net_lapses_it_in_the_panel_and_the_report() {
    let (mut app, dir, top) = app_with_two_failing_nets();
    app.run_erc_blocking();
    waive_net_t(&mut app, "accepted");
    assert_eq!(flagged(&app), ["NET_C"]);

    let mut grown = top;
    let mut extra = grown.symbols[0].clone();
    extra.uuid = Uuid::new_v4();
    extra.reference = "R_U".into();
    grown.symbols.push(extra);
    open_with(&mut app.document_state, &dir.join("top.snxsch"), grown);
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C", "NET_T"], "the finding is back");
    assert_eq!(
        app.erc_violation_waiver_entries()[0].status,
        ErcWaiverStatus::Lapsed
    );
    let opts = app.erc_report_options(signex_output::ErcReportFormat::Json);
    assert_eq!(
        opts.waivers[0].status,
        signex_output::erc_report::ErcReportWaiverStatus::Lapsed
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn confirm_fingerprints_the_sheet_as_it_is_now() {
    let (mut app, dir, top) = app_with_two_failing_nets();
    let top_path = dir.join("top.snxsch");
    let (sheet, wire) = top_with_drifting_wire(&top);
    open_with(&mut app.document_state, &top_path, sheet.clone());
    app.run_erc_blocking();
    let entry = app
        .build_erc_diagnostic_entries()
        .into_iter()
        .find(|e| e.select.is_some_and(|s| s.uuid == wire))
        .expect("dangling row");
    send(
        &mut app,
        ErcWaiverMsg::Started {
            key: entry.waiver_key.clone().expect("waivable"),
            message: entry.message.clone(),
        },
    );
    // The wire moves after the row was drawn, before Confirm.
    let mut moved = sheet.clone();
    if let Some(w) = moved.wires.iter_mut().find(|w| w.uuid == wire) {
        w.end.x = 50.8;
    }
    open_with(&mut app.document_state, &top_path, moved);
    send(&mut app, ErcWaiverMsg::ReasonChanged("ok".into()));
    send(&mut app, ErcWaiverMsg::AuthorChanged("me".into()));
    send(&mut app, ErcWaiverMsg::Confirmed);
    app.run_erc_blocking();
    assert_eq!(
        dangling_on(&app, wire),
        0,
        "waived as it is now, not lapsed"
    );

    // A wire deleted before Confirm is refused, with the editor kept open.
    let (mut app, dir2, top) = app_with_two_failing_nets();
    let (sheet, wire) = top_with_drifting_wire(&top);
    open_with(&mut app.document_state, &dir2.join("top.snxsch"), sheet);
    app.run_erc_blocking();
    let entry = app
        .build_erc_diagnostic_entries()
        .into_iter()
        .find(|e| e.select.is_some_and(|s| s.uuid == wire))
        .expect("dangling row");
    send(
        &mut app,
        ErcWaiverMsg::Started {
            key: entry.waiver_key.clone().expect("waivable"),
            message: entry.message.clone(),
        },
    );
    open_with(&mut app.document_state, &dir2.join("top.snxsch"), top);
    send(&mut app, ErcWaiverMsg::ReasonChanged("ok".into()));
    send(&mut app, ErcWaiverMsg::AuthorChanged("me".into()));
    send(&mut app, ErcWaiverMsg::Confirmed);
    let draft = app
        .ui_state
        .erc_waive_draft
        .as_ref()
        .expect("editor stays open");
    assert!(
        draft
            .error
            .as_deref()
            .is_some_and(|e| e.contains("no longer on its sheet"))
    );
    assert!(app.erc_violation_waiver_entries().is_empty());
    std::fs::remove_dir_all(&dir).ok();
    std::fs::remove_dir_all(&dir2).ok();
}

#[test]
fn a_waive_started_before_a_rerun_still_targets_its_finding() {
    let (mut app, dir, _) = app_with_two_failing_nets();
    app.run_erc_blocking();
    let entry = app
        .build_erc_diagnostic_entries()
        .into_iter()
        .find(|e| e.net.as_ref().is_some_and(|n| n.name == "NET_T"))
        .expect("NET_T row");
    // A run lands between render and click; the message carries the key.
    app.run_erc_blocking();
    send(
        &mut app,
        ErcWaiverMsg::Started {
            key: entry.waiver_key.clone().expect("waivable"),
            message: entry.message.clone(),
        },
    );
    send(&mut app, ErcWaiverMsg::ReasonChanged("ok".into()));
    send(&mut app, ErcWaiverMsg::AuthorChanged("me".into()));
    send(&mut app, ErcWaiverMsg::Confirmed);
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn undoing_a_waiver_reruns_erc() {
    use crate::app::contracts::{ErcMsg, Message};
    let (mut app, dir, _) = app_with_two_failing_nets();
    app.run_erc_blocking();
    waive_net_t(&mut app, "accepted");
    assert!(!app.ui_state.erc_running);
    let _ = app.update(Message::Erc(ErcMsg::RulesUndone));
    assert!(app.ui_state.erc_running, "undo starts a fresh run");
    std::fs::remove_dir_all(&dir).ok();
}
