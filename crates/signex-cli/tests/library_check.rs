//! End to end (#3111): `signex-cli library check` and its exit codes.

use std::path::Path;
use std::process::Command;

fn check(args: &[&str]) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_signex-cli"))
        .arg("library")
        .args(args)
        .output()
        .expect("run signex-cli");
    (
        out.status.code().expect("exit code"),
        String::from_utf8(out.stdout).expect("utf-8 stdout"),
    )
}

fn repo(rel: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn a_clean_library_exits_0_even_with_warnings() {
    let (code, stdout) = check(&["check", &repo("assets/stock-library"), "--json"]);
    assert_eq!(code, 0, "{stdout}");
    let report: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["errors"], 0);
    assert!(
        report["warnings"].as_u64().unwrap() > 0,
        "the legacy footprint tokens"
    );
}

#[test]
fn a_library_with_errors_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    let (code, stdout) = check(&["check", dir.path().to_str().unwrap()]);
    assert_eq!(code, 1);
    assert!(stdout.contains("error library.not_found"), "{stdout}");
}

#[test]
fn a_usage_error_exits_2() {
    let (code, _) = check(&["check"]);
    assert_eq!(code, 2);
    let (code, _) = check(&["check", "lib", "--bogus"]);
    assert_eq!(code, 2);
}

#[test]
fn strict_fails_on_warnings() {
    let (code, stdout) = check(&["check", &repo("assets/stock-library"), "--strict", "--json"]);
    assert_eq!(code, 1, "{stdout}");
    let report: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["errors"], 0);
    let (code, stdout) = check(&[
        "check",
        &repo("docs/fixtures/library-format/library"),
        "--strict",
    ]);
    assert_eq!(code, 0, "the conformance fixtures are clean: {stdout}");
}
