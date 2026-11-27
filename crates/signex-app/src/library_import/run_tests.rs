//! Fake converters run through the real runner: `/bin/sh <script> …`, so
//! the contract arguments start at `$2` (`library`) and no test execs a
//! file it has just written.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use super::error::ImportError;
use super::events::{ExitKind, ProgressEvent};
use super::run::{
    ConverterCommand, DESCRIBE_TIMEOUT, LibraryRunRequest, ModelsMode, RunFailure, RunLimits,
    RunMode, RunOutcome, SilenceWatch, run_describe, run_library,
};

/// Fast timing: one second of silence stands in for 120.
pub(super) const FAST: RunLimits = RunLimits {
    silence: Duration::from_secs(1),
    poll: Duration::from_millis(10),
};

/// Finds `--output`'s value and sets `$out`.
pub(crate) const PREAMBLE: &str = r#"out=""; prev=""
for a in "$@"; do [ "$prev" = "--output" ] && out="$a"; prev="$a"; done
"#;

pub(crate) fn sh(dir: &Path, body: &str) -> ConverterCommand {
    let script = dir.join("fake-converter.sh");
    std::fs::write(&script, format!("{PREAMBLE}{body}")).unwrap();
    ConverterCommand {
        program: PathBuf::from("/bin/sh"),
        leading_args: vec![script.into()],
    }
}

pub(crate) fn request(output: PathBuf, mode: RunMode) -> LibraryRunRequest {
    LibraryRunRequest {
        inputs: vec![PathBuf::from("/in/Passives.lib1")],
        output,
        name: Some("Passives".into()),
        db: None,
        search_paths: vec![],
        models: ModelsMode::Include,
        mode,
    }
}

/// A summary line for a library written to `$out`.
pub(crate) fn summary(exit: &str, ok: bool) -> String {
    format!(
        r#"echo '{{"event":"summary","ok":{ok},"error":null,"exit":"{exit}","library":"'"$out"'","report":"'"$out"'/import-report.json","files":[],"counts":{{"parts":3}},"warnings":[]}}'"#
    )
}

const REPORT: &str = include_str!("../../../../docs/fixtures/library-converter/import-report.json");

pub(crate) fn write_report_line() -> String {
    format!("mkdir -p \"$out\" && cat > \"$out/import-report.json\" <<'EOF'\n{REPORT}\nEOF\n")
}

fn run(
    body: &str,
    mode: RunMode,
    limits: RunLimits,
) -> (
    tempfile::TempDir,
    PathBuf,
    Vec<ProgressEvent>,
    Result<RunOutcome, RunFailure>,
) {
    let dir = tempfile::tempdir().unwrap();
    let command = sh(dir.path(), body);
    let out = dir.path().join("Passives");
    let mut events = Vec::new();
    let result = run_library(
        &command,
        &request(out.clone(), mode),
        limits,
        &AtomicBool::new(false),
        &mut |e| events.push(e.clone()),
    );
    (dir, out, events, result)
}

#[test]
fn the_silence_rule_counts_from_the_last_event() {
    let start = Instant::now();
    let mut watch = SilenceWatch::new(Duration::from_secs(120), start);
    // Ten minutes of events every 10 s never trip it.
    for tick in 1..=60 {
        let now = start + Duration::from_secs(10 * tick);
        assert!(!watch.expired(now), "tripped at {} s", 10 * tick);
        watch.heard(now);
    }
    let last = start + Duration::from_secs(600);
    assert!(!watch.expired(last + Duration::from_secs(119)));
    assert!(watch.expired(last + Duration::from_secs(120)));
}

#[test]
fn arguments_follow_the_contract() {
    let mut req = request(PathBuf::from("/o/P"), RunMode::Update);
    req.db = Some("Driver=x".into());
    req.search_paths = vec![PathBuf::from("/s1"), PathBuf::from("/s2")];
    req.models = ModelsMode::Skip;
    let args: Vec<String> = req
        .args()
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        [
            "library",
            "/in/Passives.lib1",
            "--output",
            "/o/P",
            "--name",
            "Passives",
            "--db",
            "Driver=x",
            "--search-path",
            "/s1",
            "--search-path",
            "/s2",
            "--models",
            "skip",
            "--update",
            "--progress"
        ]
    );
    let preview = request(PathBuf::from("/o/P"), RunMode::Preview).args();
    assert!(preview.iter().any(|a| a == "--dry-run"));
    assert!(!preview.iter().any(|a| a == "--update"));
}

#[test]
fn a_relative_input_is_refused_before_anything_runs() {
    let mut req = request(PathBuf::from("/o/P"), RunMode::Import);
    req.inputs = vec![PathBuf::from("-rf")];
    assert!(matches!(
        req.validate(),
        Err(ImportError::PathNotAbsolute { .. })
    ));
}

#[test]
fn a_partial_run_reports_events_summary_and_report() {
    let body = format!(
        "{}echo '{{\"event\":\"phase\",\"phase\":\"read\",\"total\":1}}'\n\
         echo '{{\"event\":\"progress\",\"done\":1,\"total\":1,\"item\":\"P:R1\"}}'\n{}\nexit 2\n",
        write_report_line(),
        summary("partial", true)
    );
    let (_dir, out, events, result) = run(&body, RunMode::Import, FAST);
    let outcome = result.unwrap();
    assert_eq!(outcome.exit, ExitKind::Partial);
    assert_eq!(outcome.library, Some(out));
    assert_eq!(outcome.report.unwrap().items.len(), 3);
    assert_eq!(events.len(), 2);
}

#[test]
fn a_converter_that_keeps_reporting_is_not_stopped() {
    // Six times the silence limit of steady progress.
    let body = format!(
        "{}i=0; while [ $i -lt 30 ]; do echo '{{\"event\":\"progress\",\"done\":1,\"total\":null}}'; sleep 0.2; i=$((i+1)); done\n{}\nexit 0\n",
        write_report_line(),
        summary("ok", true)
    );
    let started = Instant::now();
    let (_dir, _out, events, result) = run(&body, RunMode::Import, FAST);
    assert!(started.elapsed() > FAST.silence * 4, "ran past the limit");
    assert_eq!(result.unwrap().exit, ExitKind::Ok);
    assert_eq!(events.len(), 30);
}

#[test]
fn a_silent_converter_is_stopped_and_its_new_folder_removed() {
    let body = "mkdir -p \"$out\"\necho '{\"event\":\"phase\",\"phase\":\"read\",\"total\":null}'\nexec sleep 30\n";
    let started = Instant::now();
    let (_dir, out, _events, result) = run(body, RunMode::Import, FAST);
    let failure = result.unwrap_err();
    assert_eq!(*failure.error, ImportError::Silent { seconds: 1 });
    assert!(failure.removed_output);
    assert!(!out.exists());
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "did not wait it out"
    );
}

#[test]
fn a_failed_run_keeps_a_folder_that_existed_and_carries_the_report() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("Passives");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("mine.txt"), "keep").unwrap();
    let body = format!(
        "{}echo '{{\"event\":\"summary\",\"ok\":false,\"error\":\"bad input\",\"exit\":\"failed\",\"library\":null,\"report\":\"'\"$out\"'/import-report.json\",\"files\":[],\"warnings\":[]}}'\nexit 1\n",
        write_report_line()
    );
    let command = sh(dir.path(), &body);
    let failure = run_library(
        &command,
        &request(out.clone(), RunMode::Update),
        FAST,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert_eq!(
        *failure.error,
        ImportError::ConverterFailed {
            message: "bad input".into()
        }
    );
    assert!(
        failure.report.is_some(),
        "the report is read for inspection"
    );
    assert!(!failure.removed_output);
    assert!(out.join("mine.txt").exists());
}

#[test]
fn a_failed_new_import_removes_its_folder() {
    let body = format!(
        "{}{}\nexit 1\n",
        write_report_line(),
        summary("failed", false)
    );
    let (_dir, out, _e, result) = run(&body, RunMode::Import, FAST);
    let failure = result.unwrap_err();
    assert!(failure.removed_output);
    assert!(!out.exists());
}

#[test]
fn exit_code_and_summary_must_agree() {
    let body = format!(
        "{}{}\nexit 0\n",
        write_report_line(),
        summary("partial", true)
    );
    let (_d, _o, _e, result) = run(&body, RunMode::Import, FAST);
    assert!(matches!(
        *result.unwrap_err().error,
        ImportError::ExitMismatch { .. }
    ));
}

#[test]
fn a_crash_without_summary_names_its_status_and_stderr() {
    let (_d, _o, _e, result) = run(
        "echo '{\"event\":\"phase\",\"phase\":\"read\",\"total\":1}'\necho 'reader panicked' >&2\nexit 101\n",
        RunMode::Import,
        FAST,
    );
    match *result.unwrap_err().error {
        ImportError::NoSummary { status, stderr } => {
            assert!(status.contains("101"), "{status}");
            assert!(stderr.contains("reader panicked"), "{stderr}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_library_outside_the_output_is_refused() {
    let body = format!(
        "{}echo '{{\"event\":\"summary\",\"ok\":true,\"error\":null,\"exit\":\"ok\",\"library\":\"/tmp\",\"report\":\"'\"$out\"'/import-report.json\",\"files\":[],\"warnings\":[]}}'\nexit 0\n",
        write_report_line()
    );
    let (_d, out, _e, result) = run(&body, RunMode::Import, FAST);
    let failure = result.unwrap_err();
    assert!(matches!(*failure.error, ImportError::PathOutside { .. }));
    assert!(!out.exists(), "the folder this run made is removed");
}

#[test]
fn cancel_stops_the_run_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let command = sh(dir.path(), "exec sleep 30\n");
    let cancel = AtomicBool::new(false);
    let limits = RunLimits {
        silence: Duration::from_secs(60),
        ..FAST
    };
    let started = Instant::now();
    let result = std::thread::scope(|scope| {
        let handle = scope.spawn(|| {
            run_library(
                &command,
                &request(dir.path().join("P"), RunMode::Import),
                limits,
                &cancel,
                &mut |_| {},
            )
        });
        std::thread::sleep(Duration::from_millis(200));
        cancel.store(true, Ordering::Relaxed);
        handle.join().unwrap()
    });
    assert_eq!(*result.unwrap_err().error, ImportError::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(10));
}

#[test]
fn describe_is_parsed_and_a_newer_major_refused() {
    let dir = tempfile::tempdir().unwrap();
    let fixture = include_str!("../../../../docs/fixtures/library-converter/describe.json");
    let ok = sh(dir.path(), &format!("cat <<'EOF'\n{fixture}\nEOF\n"));
    let d = run_describe(&ok, "example-import", DESCRIBE_TIMEOUT).unwrap();
    assert_eq!(d.picker_filters().len(), 3);
    let newer = sh(
        dir.path(),
        &format!(
            "cat <<'EOF'\n{}\nEOF\n",
            fixture.replace("\"1.0\"", "\"2.1\"")
        ),
    );
    assert_eq!(
        run_describe(&newer, "example-import", DESCRIBE_TIMEOUT).unwrap_err(),
        ImportError::UnsupportedContract {
            converter: "example-import".into(),
            contract: "2.1".into()
        }
    );
    let hung = sh(dir.path(), "exec sleep 30\n");
    assert!(matches!(
        run_describe(&hung, "c", Duration::from_millis(300)).unwrap_err(),
        ImportError::DescribeFailed { .. }
    ));
}

#[test]
fn a_library_other_than_the_output_folder_is_refused() {
    let body = format!(
        "{}mkdir -p \"$out/inner\"\necho '{{\"event\":\"summary\",\"ok\":true,\"error\":null,\"exit\":\"ok\",\"library\":\"'\"$out\"'/inner\",\"report\":\"'\"$out\"'/import-report.json\",\"files\":[],\"warnings\":[]}}'\nexit 0\n",
        write_report_line()
    );
    let (_d, out, _e, result) = run(&body, RunMode::Import, FAST);
    let failure = result.unwrap_err();
    assert!(
        matches!(*failure.error, ImportError::LibraryNotOutput { .. }),
        "{:?}",
        failure.error
    );
    assert!(!out.exists());
}

#[test]
fn a_process_left_holding_stdout_does_not_hold_a_finished_run() {
    // The background sleep inherits stdout and stderr.
    let body = format!(
        "{}sleep 30 &\n{}\nexit 0\n",
        write_report_line(),
        summary("ok", true)
    );
    let limits = RunLimits {
        silence: Duration::from_secs(60),
        ..FAST
    };
    let started = Instant::now();
    let (_d, _o, _e, result) = run(&body, RunMode::Import, limits);
    assert_eq!(result.unwrap().exit, ExitKind::Ok);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "did not wait it out"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn cancel_kills_what_the_converter_started() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("child.pid");
    let command = sh(
        dir.path(),
        &format!(
            "sleep 30 &\necho $! > '{}'\necho '{{\"event\":\"phase\",\"phase\":\"read\",\"total\":null}}'\nwait\n",
            pid_file.display()
        ),
    );
    let cancel = AtomicBool::new(false);
    let mut asked = false;
    let result = run_library(
        &command,
        &request(dir.path().join("P"), RunMode::Import),
        RunLimits {
            silence: Duration::from_secs(60),
            ..FAST
        },
        &cancel,
        &mut |_| {
            if !asked {
                asked = true;
                cancel.store(true, Ordering::Relaxed);
            }
        },
    );
    assert_eq!(*result.unwrap_err().error, ImportError::Cancelled);
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    let proc_dir = PathBuf::from(format!("/proc/{}", pid.trim()));
    let deadline = Instant::now() + Duration::from_secs(5);
    while proc_dir.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!proc_dir.exists(), "the converter's child was killed");
}

#[test]
fn a_library_run_works_in_the_outputs_parent_folder() {
    let body = format!(
        "pwd > \"$out.cwd\"\n{}{}\nexit 0\n",
        write_report_line(),
        summary("ok", true)
    );
    let (dir, out, _e, result) = run(&body, RunMode::Import, FAST);
    result.unwrap();
    let cwd = std::fs::read_to_string(out.with_extension("cwd")).unwrap();
    assert_eq!(
        PathBuf::from(cwd.trim()).canonicalize().unwrap(),
        dir.path().canonicalize().unwrap()
    );
}

/// Run `body` as a new import on a thread, set cancel after a moment, and
/// return the result, how long it took, and the output folder.
fn cancel_after_a_moment(body: &str) -> (Result<RunOutcome, RunFailure>, Duration, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let command = sh(dir.path(), body);
    let out = dir.path().join("P");
    let cancel = AtomicBool::new(false);
    let limits = RunLimits {
        silence: Duration::from_secs(60),
        ..FAST
    };
    let started = Instant::now();
    let result = std::thread::scope(|scope| {
        let handle = scope.spawn(|| {
            run_library(
                &command,
                &request(out.clone(), RunMode::Import),
                limits,
                &cancel,
                &mut |_| {},
            )
        });
        std::thread::sleep(Duration::from_millis(300));
        cancel.store(true, Ordering::Relaxed);
        handle.join().unwrap()
    });
    let gone = !out.exists();
    assert!(gone, "the folder the run made is removed");
    (result, started.elapsed(), out)
}

/// Cancel ends a run whatever the converter does: repeat one event
/// forever, say nothing, or ignore termination and never exit.
#[test]
fn cancel_ends_a_converter_that_repeats_is_silent_or_never_exits() {
    let heartbeat = "while :; do echo '{\"event\":\"progress\",\"done\":1,\"total\":3,\"item\":\"A\"}'; sleep 0.05; done\n";
    let silent = "mkdir -p \"$out\"; echo x > \"$out/half.snxsym\"; sleep 30\n";
    let stubborn = "trap '' TERM INT HUP; while :; do sleep 0.1; done\n";
    for body in [heartbeat, silent, stubborn] {
        let (result, took, _out) = cancel_after_a_moment(body);
        assert_eq!(*result.unwrap_err().error, ImportError::Cancelled, "{body}");
        assert!(took < Duration::from_secs(5), "{body}: took {took:?}");
    }
}

#[test]
fn a_new_run_creates_its_folder_and_refuses_one_that_holds_anything() {
    use super::run::claim_output;
    let dir = tempfile::tempdir().unwrap();
    let new = dir.path().join("a/b/New");
    assert_eq!(
        claim_output(&request(new.clone(), RunMode::Import)),
        Ok(true)
    );
    assert!(new.is_dir(), "created with its parents");
    // An empty folder is taken over: removed and created by this run.
    assert_eq!(
        claim_output(&request(new.clone(), RunMode::Preview)),
        Ok(true)
    );
    std::fs::write(new.join("theirs.txt"), "keep").unwrap();
    assert_eq!(
        claim_output(&request(new.clone(), RunMode::Import)),
        Err(ImportError::OutputNotEmpty { path: new.clone() })
    );
    assert!(new.join("theirs.txt").exists());
    let file = dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    assert!(claim_output(&request(file.clone(), RunMode::Import)).is_err());
    assert_eq!(
        claim_output(&request(new.clone(), RunMode::Update)),
        Ok(false)
    );
}

/// A link is refused as a link, not as a folder that holds files, and
/// neither it nor the folder it points at is touched.
#[test]
fn a_new_run_refuses_a_link_as_a_link() {
    use super::run::claim_output;
    let dir = tempfile::tempdir().unwrap();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert_eq!(
        claim_output(&request(link.clone(), RunMode::Import)),
        Err(ImportError::OutputIsLink { path: link.clone() })
    );
    assert!(real.is_dir() && link.exists(), "both left as they were");
    let shown = ImportError::OutputIsLink { path: link }.to_string();
    assert!(shown.contains("is a link"), "{shown}");
}

#[test]
fn a_failed_run_into_an_empty_folder_removes_it_and_a_full_one_never_runs() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("P");
    std::fs::create_dir(&out).unwrap();
    let ran = dir.path().join("ran");
    let body = format!(
        ": > '{}'\necho x > \"$out/half.snxsym\"\n{}\nexit 1\n",
        ran.display(),
        summary("failed", false)
    );
    let command = sh(dir.path(), &body);
    let failure = run_library(
        &command,
        &request(out.clone(), RunMode::Import),
        FAST,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(failure.removed_output);
    assert!(!out.exists(), "what the run wrote is gone");
    // A folder that holds something is refused before the converter runs.
    std::fs::create_dir(&out).unwrap();
    std::fs::write(out.join("mine.txt"), "keep").unwrap();
    std::fs::remove_file(&ran).unwrap();
    let failure = run_library(
        &command,
        &request(out.clone(), RunMode::Import),
        FAST,
        &AtomicBool::new(false),
        &mut |_| {},
    )
    .unwrap_err();
    assert!(matches!(*failure.error, ImportError::OutputNotEmpty { .. }));
    assert!(
        failure.refused && !failure.removed_output,
        "nothing changed"
    );
    assert!(!ran.exists(), "not run");
    assert!(out.join("mine.txt").exists());
}

/// Quitting kills the converter through its published process group,
/// without the run's own stop.
#[test]
fn the_published_group_kills_a_converter_that_never_exits() {
    use super::process::ProcessGroup;
    let dir = tempfile::tempdir().unwrap();
    let command = sh(dir.path(), "trap '' TERM; while :; do sleep 0.1; done\n");
    let group = ProcessGroup::default();
    let limits = RunLimits {
        silence: Duration::from_secs(60),
        ..FAST
    };
    let started = Instant::now();
    let result = std::thread::scope(|scope| {
        let handle = scope.spawn(|| {
            super::run::run_library_in(
                &command,
                &request(dir.path().join("P"), RunMode::Import),
                limits,
                &AtomicBool::new(false),
                &group,
                &mut |_| {},
            )
        });
        while group.current().is_none() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(10));
        }
        group.kill();
        handle.join().unwrap()
    });
    assert!(result.is_err(), "killed, so no library");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(group.current().is_none(), "cleared once it ended");
}

/// The published group id is cleared as soon as the converter is reaped,
/// not after the pipe drain that may follow for `PIPE_GRACE`: a quit in
/// that window must not signal a group id the system handed out again.
#[test]
fn the_published_group_is_cleared_once_the_converter_is_reaped() {
    use super::process::{PIPE_GRACE, ProcessGroup};
    let dir = tempfile::tempdir().unwrap();
    // The shell exits after a moment; the sleep it leaves holds stdout
    // open, so the run drains for the whole grace period.
    let command = sh(dir.path(), "sleep 5 &\nsleep 0.3\nexit 0\n");
    let group = ProcessGroup::default();
    let started = Instant::now();
    let (cleared_at, ended_at) = std::thread::scope(|scope| {
        let handle = scope.spawn(|| {
            let result = super::run::run_library_in(
                &command,
                &request(dir.path().join("P"), RunMode::Import),
                FAST,
                &AtomicBool::new(false),
                &group,
                &mut |_| {},
            );
            (result, Instant::now())
        });
        while group.current().is_none() && started.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(5));
        }
        while group.current().is_some() && started.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        let cleared_at = Instant::now();
        let (result, ended_at) = handle.join().unwrap();
        assert!(result.is_err(), "no summary, so no library");
        (cleared_at, ended_at)
    });
    assert!(
        ended_at.saturating_duration_since(cleared_at) >= PIPE_GRACE / 2,
        "the id was published until the drain ended"
    );
}
