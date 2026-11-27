//! Discovery over a made-up `PATH`: fake converters are run as
//! `/bin/sh <file>`, so no test execs a file it has just written.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::discovery::{DiscoveryEnv, candidates, discover_with};
use super::error::ImportError;
use super::run::ConverterCommand;

fn describe_script(converter: &str) -> String {
    format!(
        "[ \"$1\" = describe ] || exit 1\necho '{{\"contract\":\"1.0\",\"converter\":\"{converter}\",\"version\":\"1.0\",\"inputs\":[{{\"label\":\"Lib\",\"extensions\":[\"lib1\"],\"kind\":\"library\"}}]}}'\n"
    )
}

fn write(dir: &Path, name: &str, body: &str, executable: bool) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    let mode = if executable { 0o755 } else { 0o644 };
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    path
}

fn sh(path: &Path) -> ConverterCommand {
    ConverterCommand {
        program: PathBuf::from("/bin/sh"),
        leading_args: vec![path.into()],
    }
}

fn path_of(dirs: &[&Path]) -> OsString {
    std::env::join_paths(dirs).unwrap()
}

#[test]
fn converters_are_found_by_name_and_describe_and_the_rest_skipped() {
    let bin = tempfile::tempdir().unwrap();
    let a = write(bin.path(), "signex-a-import", &describe_script("a"), true);
    let b = write(bin.path(), "signex-b-import", &describe_script("b"), true);
    write(
        bin.path(),
        "signex-broken-import",
        "echo '{not json'\n",
        true,
    );
    // An old single-file importer: no describe command.
    write(
        bin.path(),
        "signex-old-import",
        "echo 'usage: signex-old-import <file> --output <dir>' >&2\nexit 2\n",
        true,
    );
    write(
        bin.path(),
        "signex-plain-import",
        &describe_script("p"),
        false,
    );
    write(bin.path(), "other-tool", &describe_script("o"), true);

    let env = DiscoveryEnv {
        // The same folder twice, and once more relative: listed once.
        path: Some(path_of(&[bin.path(), bin.path(), Path::new("rel")])),
        ..Default::default()
    };
    let found = discover_with(&env, Duration::from_secs(5), sh);

    let names: Vec<&str> = found
        .converters
        .iter()
        .map(|c| c.description.converter.as_str())
        .collect();
    assert_eq!(names, ["a", "b"]);
    assert_eq!(found.converters[0].path, a);
    let skipped: Vec<&PathBuf> = found.skipped.iter().map(|(p, _)| p).collect();
    assert_eq!(skipped.len(), 2, "{skipped:?}");
    assert!(
        found
            .skipped
            .iter()
            .all(|(_, e)| matches!(e, ImportError::DescribeFailed { .. }))
    );
    // Both claim .lib1: both are offered.
    let for_lib = found.converters_for(Path::new("/in/X.LIB1"));
    assert_eq!(for_lib.len(), 2);
    assert_eq!(for_lib[1].path, b);
    assert!(found.converters_for(Path::new("/in/x.txt")).is_empty());
}

#[test]
fn the_list_and_single_variables_add_converters_deduplicated() {
    let bin = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let a = write(bin.path(), "signex-a-import", &describe_script("a"), true);
    let custom = write(
        elsewhere.path(),
        "my-converter",
        &describe_script("c"),
        true,
    );
    let single = write(
        elsewhere.path(),
        "binary-importer",
        &describe_script("s"),
        true,
    );

    let env = DiscoveryEnv {
        path: Some(path_of(&[bin.path()])),
        list: Some(path_of(&[&custom, &a])),
        singles: vec![("SNX_SINGLE".into(), single.clone().into())],
    };
    let (paths, bad) = candidates(&env);
    assert!(bad.is_empty());
    assert_eq!(paths, [single, custom, a]);
    let found = discover_with(&env, Duration::from_secs(5), sh);
    assert_eq!(found.converters.len(), 3);
}

#[test]
fn a_newer_contract_major_is_skipped() {
    let bin = tempfile::tempdir().unwrap();
    write(
        bin.path(),
        "signex-new-import",
        &describe_script("n").replace("\"1.0\",\"converter\"", "\"2.0\",\"converter\""),
        true,
    );
    let env = DiscoveryEnv {
        path: Some(path_of(&[bin.path()])),
        ..Default::default()
    };
    let found = discover_with(&env, Duration::from_secs(5), sh);
    assert!(found.converters.is_empty());
    assert!(matches!(
        found.skipped[0].1,
        ImportError::UnsupportedContract { .. }
    ));
}

#[test]
fn stuck_candidates_are_asked_in_parallel_and_the_answer_is_cached() {
    use super::discovery::{PARALLEL_DESCRIBES, discover_cached};
    let bin = tempfile::tempdir().unwrap();
    let asked = bin.path().join("asked");
    for name in [
        "signex-s1-import",
        "signex-s2-import",
        "signex-s3-import",
        "signex-s4-import",
    ] {
        write(bin.path(), name, "exec sleep 30\n", true);
    }
    write(
        bin.path(),
        "signex-a-import",
        &format!("echo x >> '{}'\n{}", asked.display(), describe_script("a")),
        true,
    );
    let env = DiscoveryEnv {
        path: Some(path_of(&[bin.path()])),
        ..Default::default()
    };
    const { assert!(PARALLEL_DESCRIBES >= 4) };
    let started = std::time::Instant::now();
    let first = discover_cached(None, &env, Duration::from_secs(1), sh);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "four stuck candidates cost one timeout, took {:?}",
        started.elapsed()
    );
    assert_eq!(first.discovery.converters.len(), 1);
    assert_eq!(first.discovery.skipped.len(), 4);

    let again = discover_cached(Some(&first), &env, Duration::from_secs(1), sh);
    assert_eq!(again, first);
    assert_eq!(
        std::fs::read_to_string(&asked).unwrap().lines().count(),
        1,
        "asked once"
    );

    // A changed candidate is asked again.
    write(bin.path(), "signex-b-import", &describe_script("b"), true);
    let changed = discover_cached(Some(&first), &env, Duration::from_secs(1), sh);
    assert_eq!(changed.discovery.converters.len(), 2);
}

#[cfg(target_os = "linux")]
#[test]
fn describe_runs_in_a_neutral_folder_and_leaves_nothing_running() {
    let bin = tempfile::tempdir().unwrap();
    let cwd_file = bin.path().join("cwd");
    let pid_file = bin.path().join("pid");
    let body = format!(
        "pwd > '{}'\nsleep 30 > /dev/null 2>&1 &\necho $! > '{}'\n{}",
        cwd_file.display(),
        pid_file.display(),
        describe_script("a")
    );
    let script = write(bin.path(), "signex-a-import", &body, true);
    super::run::run_describe(&sh(&script), "a", Duration::from_secs(5)).unwrap();
    let cwd = PathBuf::from(std::fs::read_to_string(&cwd_file).unwrap().trim());
    assert_eq!(
        cwd.canonicalize().unwrap(),
        std::env::temp_dir().canonicalize().unwrap()
    );
    let proc_dir = PathBuf::from(format!(
        "/proc/{}",
        std::fs::read_to_string(&pid_file).unwrap().trim()
    ));
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while proc_dir.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!proc_dir.exists(), "what describe started was killed");
}
