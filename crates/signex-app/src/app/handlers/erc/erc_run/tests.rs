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
