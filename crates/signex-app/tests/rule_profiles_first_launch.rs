//! #1608: the first launch after an upgrade folds newer `prefs.json` ERC
//! rules into an older User Default profile, once. Its own binary (#2302): it
//! writes the per-process prefs file by hand, and in the shared regression
//! process another test's background prefs write could load the file before
//! that seed and save it back after, dropping it.

use std::path::PathBuf;

use signex_app::app::Signex;
use signex_app::rule_profiles;
use signex_app::task_driver::TaskDriver;
use signex_types::project::{ErcRules, RuleProfile};

fn library() -> PathBuf {
    rule_profiles::profiles_dir().expect("test config root")
}

fn rules(rule: &str, severity: &str) -> ErcRules {
    let mut erc = ErcRules::default();
    erc.severity.insert(rule.into(), severity.into());
    erc
}

fn read_profile(file: &str) -> RuleProfile {
    RuleProfile::from_json_bytes(&std::fs::read(library().join(file)).unwrap()).unwrap()
}

/// Launch the window app and run every boot task to completion.
fn launch() {
    let (mut app, task) = Signex::boot(Vec::new());
    TaskDriver::new()
        .settle(&mut app, task)
        .expect("the boot tasks settled");
}

#[test]
fn the_first_launch_after_upgrade_folds_newer_prefs_into_an_older_user_default() {
    // v0.15 state: a User Default made once, then prefs.json kept changing.
    rule_profiles::write_user_default(&library(), rules("dangling_wire", "off")).unwrap();
    let prefs = signex_app::fonts::prefs_file_path();
    std::fs::create_dir_all(prefs.parent().unwrap()).unwrap();
    std::fs::write(
        &prefs,
        br#"{"erc_severity": {"dangling_wire": "error", "unused_pin": "info"}}"#,
    )
    .unwrap();

    launch();
    let folded = read_profile("user-default.snxrules");
    assert_eq!(
        folded.erc.severity["dangling_wire"], "error",
        "the newer prefs value wins"
    );
    assert_eq!(folded.erc.severity["unused_pin"], "info");
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
    assert_eq!(marker["erc_rules_in_profile"], true);

    // Edit the profile, relaunch: the stale prefs keys do not win again.
    rule_profiles::write_user_default(&library(), rules("dangling_wire", "warning")).unwrap();
    launch();
    assert_eq!(
        read_profile("user-default.snxrules").erc.severity["dangling_wire"],
        "warning"
    );
}
