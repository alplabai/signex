// Standalone binary: the test must plant its stale folder before anything in this process resolves the config root, which a shared binary cannot promise.
//! #1820 — the test config root `<tmp>/signex-test-prefs-<pid>/` must start
//! empty. A folder left by an earlier process that had the same PID (PIDs
//! get reused) used to leak its files into this run.

use std::fs;

#[test]
fn a_stale_root_left_by_an_earlier_process_with_this_pid_is_cleared() {
    let root = std::env::temp_dir().join(format!("signex-test-prefs-{}", std::process::id()));
    let stale_file = root.join("global_libraries.toml");
    let stale_dir = root.join("keyboard_shortcuts.toml");
    fs::create_dir_all(&stale_dir).expect("plant a stale directory");
    fs::write(&stale_file, "libraries = [\"stale.snxlib\"]\n").expect("plant a stale file");

    let resolved = signex_app::config_root::config_root().expect("the test redirect resolves");

    assert_eq!(resolved, root);
    assert!(
        !stale_file.exists(),
        "stale global_libraries.toml survived into this process"
    );
    assert!(
        !stale_dir.exists(),
        "stale keyboard_shortcuts.toml directory survived into this process"
    );

    fs::create_dir_all(&resolved).expect("recreate the root");
    fs::write(resolved.join("prefs.json"), "{}").expect("write a fresh file");
    assert_eq!(
        signex_app::config_root::config_root().expect("the test redirect resolves"),
        resolved
    );
    assert!(
        resolved.join("prefs.json").exists(),
        "a second resolution in the same process must not clear the root"
    );
    fs::remove_dir_all(&resolved).expect("clean up the root");
}
