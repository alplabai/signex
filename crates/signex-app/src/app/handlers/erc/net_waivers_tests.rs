//! Per-net ERC waivers (#1581): hide on the next run, persist through a
//! save + load, restore, stay scoped to one net, and undo with the rule set.

use signex_types::format::SnxSchematic;
use uuid::Uuid;

use crate::app::Signex;
use crate::app::handlers::menu::export::tests::{app_workspace, open_with, sheet_with_net};

const RULE: &str = "user::net_check";

/// Two sheets, nets `NET_T` and `NET_C`, and a project DSL rule that fires on
/// every net whose name starts with `NET_`.
fn app_with_two_failing_nets() -> (Signex, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("signex-erc-waiver-{}", Uuid::now_v7()));
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
    let top = dir.join("top.snxsch");
    open_with(
        &mut app.document_state,
        &top,
        sheet_with_net("R_T", "NET_T", &["child.snxsch"]),
    );
    app.document_state.active_path = Some(top);
    app.ui_state.erc_severity_override.clear();
    app.ui_state.pin_matrix_overrides.clear();
    (app, dir)
}

/// Names of the nets the rule currently flags, sorted.
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

fn waive(app: &mut Signex, net: &str, on: bool) {
    let _ = app.handle_erc_net_waiver_set(RULE, net, on);
    app.run_erc_blocking();
}

#[test]
fn waiving_hides_the_finding_on_the_next_run_and_unwaiving_restores_it() {
    let (mut app, dir) = app_with_two_failing_nets();
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C", "NET_T"]);

    waive(&mut app, "NET_T", true);
    assert_eq!(flagged(&app), ["NET_C"], "waiver for A must not hide B");
    let entries = app.erc_waiver_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].net.as_str(), entries[0].hidden), ("NET_T", 1));

    waive(&mut app, "NET_T", false);
    assert_eq!(flagged(&app), ["NET_C", "NET_T"]);
    assert!(app.erc_waiver_entries().is_empty());
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_waiver_persists_through_save_and_load() {
    let (mut app, dir) = app_with_two_failing_nets();
    waive(&mut app, "NET_T", true);
    let project = &app.document_state.projects[0];
    assert!(app.document_state.dirty_paths.contains(&project.path));

    signex_types::project::write_project(&project.path, &project.data).expect("save");
    let loaded = signex_types::project::parse_project(&project.path).expect("load");
    assert_eq!(loaded.rules, project.data.rules);

    app.document_state.projects[0].data = loaded;
    app.run_erc_blocking();
    assert_eq!(flagged(&app), ["NET_C"]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_waiver_for_a_net_that_no_longer_exists_hides_nothing() {
    // Renaming a net leaves its old name in the waivers: the finding returns
    // under the new name and the stale entry lists with zero hidden.
    let (mut app, dir) = app_with_two_failing_nets();
    waive(&mut app, "NET_OLD", true);
    assert_eq!(flagged(&app), ["NET_C", "NET_T"]);
    let entries = app.erc_waiver_entries();
    assert_eq!((entries[0].net.as_str(), entries[0].hidden), ("NET_OLD", 0));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn waiving_is_one_undoable_rule_edit() {
    let (mut app, dir) = app_with_two_failing_nets();
    waive(&mut app, "NET_T", true);
    assert!(app.undo_project_rules());
    assert!(app.erc_waiver_entries().is_empty());
    assert!(app.redo_project_rules());
    assert_eq!(app.erc_waiver_entries().len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}
