//! #1924, on every platform: the flags that choose where a run works, and the
//! mode every result line names. Temp-dir paths only.

mod support;

use support::{fixture, run_cli, temp_root};

#[test]
fn a_run_with_no_running_signex_is_headless_and_says_so_on_every_line() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let out = run_cli(&["run", fx.project.to_str().unwrap(), "list_documents"], "");
    assert_eq!(out.code, Some(0), "{}", out.stderr);
    assert_eq!(out.lines[0]["mode"], "headless");
    assert_eq!(out.lines[0]["status"], "answered");
}

#[test]
fn attach_fails_when_no_running_signex_has_the_project_and_the_flags_exclude_each_other() {
    let (_guard, root) = temp_root();
    let fx = fixture(&root);
    let project = fx.project.to_str().unwrap();
    let attach = run_cli(&["run", project, "list_documents", "--attach"], "");
    assert_eq!(attach.code, Some(2));
    assert!(
        attach.stderr.contains("--attach: no running Signex has"),
        "{}",
        attach.stderr
    );
    assert!(attach.lines.is_empty(), "it ran headless anyway");

    let both = run_cli(&["run", project, "--attach", "--headless"], "");
    assert_eq!(both.code, Some(2));
    assert!(
        both.stderr.contains("exclude each other"),
        "{}",
        both.stderr
    );
}
