//! The ERC run is asynchronous: only the newest run's result may land.

use std::collections::HashMap;
use std::path::PathBuf;

use super::super::erc_job::{ErcRunOutcome, run_erc_job};
use crate::app::handlers::menu::export::tests::app_workspace;

fn outcome_for(path: &str) -> ErcRunOutcome {
    ErcRunOutcome {
        by_path: HashMap::from([(PathBuf::from(path), Vec::new())]),
        warnings: Vec::new(),
        infos: Vec::new(),
        rule_set: signex_output::ErcReportRuleSet {
            profile_name: None,
            sha256: "outcome-set".into(),
        },
        waived_hidden: Default::default(),
    }
}

#[test]
fn a_superseded_erc_run_is_dropped() {
    let dir = std::env::temp_dir().join(format!("signex-erc-gen-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);

    let (first, _) = app.begin_erc_run();
    let (second, inputs) = app.begin_erc_run();
    assert!(app.ui_state.erc_running, "a run is in flight");

    assert!(!app.apply_erc_run_result(first, &Ok(outcome_for("stale.snxsch"))));
    assert!(
        app.ui_state.erc_violations_by_path.is_empty(),
        "the superseded run must not apply"
    );
    assert!(app.ui_state.erc_running, "the newest run is still pending");

    assert!(app.apply_erc_run_result(second, &Ok(run_erc_job(inputs))));
    assert!(!app.ui_state.erc_running);
}

#[test]
fn a_failed_erc_run_clears_running_and_keeps_old_results() {
    let dir = std::env::temp_dir().join(format!("signex-erc-fail-{}", uuid::Uuid::new_v4()));
    let mut app = app_workspace(&dir.to_string_lossy(), &["a_root.snxsch"]);
    app.ui_state
        .erc_violations_by_path
        .insert(PathBuf::from("kept.snxsch"), Vec::new());

    let (generation, _) = app.begin_erc_run();
    assert!(!app.apply_erc_run_result(generation, &Err("worker died".to_string())));

    assert!(!app.ui_state.erc_running);
    assert!(
        app.ui_state
            .erc_violations_by_path
            .contains_key(&PathBuf::from("kept.snxsch"))
    );
}

/// #859 — the project's `erc.dsl` is read by the worker, not by `update`: the
/// read failure is reported only when the run's completion lands.
#[test]
fn the_erc_dsl_is_read_by_the_worker_not_in_update() {
    let _ = crate::diagnostics::init_logging();
    let dir = tempfile::tempdir().expect("tempdir");
    // A directory named like the rules file: it exists, but cannot be read.
    std::fs::create_dir(dir.path().join("erc.dsl")).expect("dsl dir");
    let mut app = app_workspace(&dir.path().to_string_lossy(), &["a.snxsch"]);
    let sheet = dir.path().join("a.snxsch");
    let engine = signex_engine::Engine::new(signex_types::schematic::SchematicSheet::default())
        .expect("engine");
    app.document_state.engines.insert(sheet.clone(), engine);
    app.document_state.active_path = Some(sheet);

    let task = app.handle_run_erc();
    let reported = || {
        crate::diagnostics::recent_entries()
            .iter()
            .any(|e| e.message.contains("ERC DSL: failed to read"))
    };
    assert!(app.ui_state.erc_running);
    assert!(!reported(), "update() did not read the rules file");

    let mut messages =
        crate::app::handlers::document_files::restore_commit_tests::collect_messages(task);
    assert_eq!(messages.len(), 1);
    let _ = app.update(messages.remove(0));
    assert!(!app.ui_state.erc_running);
    assert!(reported(), "the worker reported the unreadable rules file");
}
