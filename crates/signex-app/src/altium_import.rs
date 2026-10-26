//! File ▸ Import ▸ Altium Project: run the `signex-altium-import` companion.
//!
//! The converter is a separate binary from the `alplabai/signex-altium-import`
//! repo, reached only across a process boundary; nothing of it is linked in.
//! The contract (flags, JSON summary) is that repo's
//! `docs/APP_INTEGRATION.md`.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Binary name looked up on `PATH`.
pub const BINARY: &str = "signex-altium-import";

/// Overrides the `PATH` lookup with an explicit binary path.
pub const ENV_OVERRIDE: &str = "SIGNEX_ALTIUM_IMPORT";

/// How long a conversion may run before it is killed. The converter has no
/// deadline of its own, and a malformed file must not hang the import.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// Written next to the converted files so the review list outlives the
/// session.
pub const REPORT_FILE: &str = "IMPORT_REPORT.txt";

/// Bound on the converter's stdout (its JSON summary is one line).
const STDOUT_LIMIT: usize = 1 << 20;
/// Bound on the stderr tail kept for the error message.
const STDERR_TAIL: usize = 4 << 10;

/// What a finished import produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    /// The file to open: the `.snxprj` for a project, else the first output
    /// (`.snxsch`, `.snxpcb`, `.snxsym` or `.snxfpt`).
    pub open: PathBuf,
    /// What the converter approximated or skipped, with counts.
    pub warnings: Vec<(String, usize)>,
}

/// The converter binary: `$SIGNEX_ALTIUM_IMPORT`, else `PATH`. `Err` is the
/// user-facing reason it could not be found.
pub fn locate() -> Result<PathBuf, String> {
    locate_in(std::env::var_os(ENV_OVERRIDE), std::env::var_os("PATH"))
}

fn locate_in(override_path: Option<OsString>, path: Option<OsString>) -> Result<PathBuf, String> {
    if let Some(p) = override_path.map(PathBuf::from) {
        return if p.is_file() {
            Ok(p)
        } else {
            Err(format!(
                "{ENV_OVERRIDE} is set to {}, which is not a file.\n\
                 Point it at the {BINARY} binary, or unset it to search PATH.",
                p.display()
            ))
        };
    }
    let name = format!("{BINARY}{}", std::env::consts::EXE_SUFFIX);
    // An empty or `.` entry would resolve against the working directory.
    path.iter()
        .flat_map(std::env::split_paths)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(&name))
        .find(|p| p.is_file())
        .ok_or_else(not_installed_message)
}

/// User-facing text for a missing converter.
pub fn not_installed_message() -> String {
    format!(
        "The Altium importer ({BINARY}) was not found on PATH.\n\
         Install it with:\n  cargo install --git https://github.com/alplabai/signex-altium-import {BINARY}\n\
         or set {ENV_OVERRIDE} to the binary's path."
    )
}

/// Converted files go in a sibling folder so the Altium tree is untouched.
pub fn output_dir_for(input: &Path) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "altium".into());
    input
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{stem}-signex"))
}

/// Run the converter. Blocking; call from `spawn_blocking`.
///
/// Never passes `--force`: if the output folder already holds converted
/// files, the converter refuses and that refusal is the error shown. A run
/// longer than `timeout` is killed.
pub fn run(
    binary: &Path,
    input: &Path,
    out_dir: &Path,
    timeout: Duration,
) -> Result<ImportOutcome, String> {
    let result = run_converter(binary, input, out_dir, timeout).and_then(|finished| {
        let outcome = parse_summary(&finished.stdout).map_err(|e| {
            if reported_failure(&finished.stdout) {
                e
            } else {
                with_diagnostics(e, finished.status, &finished.stderr)
            }
        })?;
        let open = confine(&outcome.open, out_dir)?;
        Ok(ImportOutcome { open, ..outcome })
    });
    match result {
        Ok(outcome) => {
            write_report(input, out_dir, &outcome);
            Ok(outcome)
        }
        Err(e) if out_dir.exists() => Err(format!(
            "{e}\n\nIf {} holds a partial or earlier import, delete it and import again.",
            out_dir.display()
        )),
        Err(e) => Err(e),
    }
}

/// A converter run that ended on its own.
struct Finished {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: Vec<u8>,
}

/// Spawn and wait at most `timeout`, killing the converter past it.
fn run_converter(
    binary: &Path,
    input: &Path,
    out_dir: &Path,
    timeout: Duration,
) -> Result<Finished, String> {
    let mut child = Command::new(binary)
        .arg(input)
        .arg("--output")
        .arg(out_dir)
        .arg("--json")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not start {}: {e}", binary.display()))?;
    // Drain both pipes on their own threads so a chatty child cannot block
    // on a full pipe while we wait for it.
    let stdout = drain(child.stdout.take(), Keep::Head(STDOUT_LIMIT));
    let stderr = drain(child.stderr.take(), Keep::Tail(STDERR_TAIL));
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                stop(&mut child);
                return Err(format!(
                    "the importer timed out after {} s and was stopped",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                stop(&mut child);
                return Err(format!("could not wait for the importer: {e}"));
            }
        }
    };
    Ok(Finished {
        status,
        stdout: String::from_utf8_lossy(&stdout.join().unwrap_or_default()).into_owned(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Kill and reap the converter. It may have exited in the meantime, so a
/// failure here only means there was nothing left to stop.
fn stop(child: &mut std::process::Child) {
    if let Err(e) = child.kill() {
        log::debug!("Altium import: kill: {e}");
    }
    if let Err(e) = child.wait() {
        log::debug!("Altium import: reap: {e}");
    }
}

/// The converter printed its own `ok: false` result, whose `error` already
/// says what went wrong.
fn reported_failure(stdout: &str) -> bool {
    summary_line(stdout)
        .and_then(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .is_some_and(|v| v["ok"].as_bool() == Some(false))
}

fn summary_line(stdout: &str) -> Option<&str> {
    stdout
        .lines()
        .rev()
        .find(|l| l.trim_start().starts_with('{'))
}

/// Read `pipe` to its end on a thread, keeping at most `keep` bytes: the
/// first ones for stdout (the summary is near the start), the last ones for
/// stderr (the error is near the end).
fn drain<R: Read + Send + 'static>(
    pipe: Option<R>,
    keep: Keep,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let Some(mut pipe) = pipe else {
            return kept;
        };
        let mut chunk = [0u8; 8192];
        loop {
            let n = match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            match keep {
                Keep::Head(limit) => {
                    let room = limit.saturating_sub(kept.len());
                    kept.extend_from_slice(&chunk[..n.min(room)]);
                }
                Keep::Tail(limit) => {
                    kept.extend_from_slice(&chunk[..n]);
                    let excess = kept.len().saturating_sub(limit);
                    kept.drain(..excess);
                }
            }
        }
        kept
    })
}

#[derive(Clone, Copy)]
enum Keep {
    Head(usize),
    Tail(usize),
}

fn with_diagnostics(error: String, status: std::process::ExitStatus, stderr: &[u8]) -> String {
    let mut message = format!("{error} ({status})");
    let tail = String::from_utf8_lossy(stderr);
    if !tail.trim().is_empty() {
        message.push('\n');
        message.push_str(tail.trim());
    }
    message
}

/// The reported file, resolved against `out_dir` when relative, and refused
/// unless it lies inside `out_dir`.
fn confine(reported: &Path, out_dir: &Path) -> Result<PathBuf, String> {
    let joined = out_dir.join(reported);
    let outside = || {
        format!(
            "the importer reported {}, which is not inside {}",
            reported.display(),
            out_dir.display()
        )
    };
    let dir = out_dir.canonicalize().map_err(|_| outside())?;
    let file = joined.canonicalize().map_err(|e| {
        format!(
            "the importer reported {}, which cannot be read: {e}",
            joined.display()
        )
    })?;
    if file.starts_with(&dir) {
        Ok(joined)
    } else {
        Err(outside())
    }
}

/// Keep the review list beside the converted files. Best effort: a failure
/// is logged, and the list still reaches the Messages panel.
fn write_report(input: &Path, out_dir: &Path, outcome: &ImportOutcome) {
    let mut text = format!("Imported from {}\n\n", input.display());
    if outcome.warnings.is_empty() {
        text.push_str("Nothing was approximated or skipped.\n");
    } else {
        text.push_str("Review before fabrication:\n");
        text.push_str(&warnings_text(&outcome.warnings));
        text.push('\n');
    }
    let path = out_dir.join(REPORT_FILE);
    if let Err(e) = signex_types::atomic_io::atomic_write(&path, text.as_bytes()) {
        log::warn!("Altium import: could not write {}: {e}", path.display());
    }
}

/// Parse the converter's `--json` summary.
pub fn parse_summary(stdout: &str) -> Result<ImportOutcome, String> {
    let line = summary_line(stdout).ok_or_else(|| "the importer printed no result".to_string())?;
    let v: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("unreadable importer result: {e}"))?;
    if v["ok"].as_bool() != Some(true) {
        return Err(v["error"]
            .as_str()
            .unwrap_or("the importer failed without a message")
            .to_string());
    }
    let open = v["project"]
        .as_str()
        .or_else(|| v["files"].as_array()?.first()?.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| "the importer reported success but wrote no files".to_string())?;
    let warnings = v["warnings"]
        .as_array()
        .map(|ws| {
            ws.iter()
                .filter_map(|w| {
                    let msg = w["message"].as_str()?.to_string();
                    let n = w["count"].as_u64().unwrap_or(1) as usize;
                    Some((msg, n))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ImportOutcome { open, warnings })
}

/// One line per warning, for the advisory notice.
pub fn warnings_text(warnings: &[(String, usize)]) -> String {
    warnings
        .iter()
        .map(|(m, n)| {
            if *n > 1 {
                format!("• {m} (×{n})")
            } else {
                format!("• {m}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_result_opens_the_project() {
        let out = parse_summary(
            r#"{"ok":true,"error":null,"project":"/o/B.snxprj","files":["/o/A.snxsch","/o/B.snxprj"],"warnings":[{"message":"copper arc imported as straight track segments","count":3}]}"#,
        )
        .unwrap();
        assert_eq!(out.open, PathBuf::from("/o/B.snxprj"));
        assert_eq!(
            out.warnings,
            [(
                "copper arc imported as straight track segments".to_string(),
                3
            )]
        );
    }

    /// Verbatim output of `signex-altium-import Demo.PrjPcb --json` (paths shortened).
    #[test]
    fn parses_real_converter_output() {
        let out = parse_summary(
            r#"{"error":null,"files":["/o/simple.snxsch","/o/simple.snxpcb","/o/Demo.snxprj"],"ok":true,"project":"/o/Demo.snxprj","warnings":[{"count":1,"message":"embedded image not imported"}]}"#,
        )
        .unwrap();
        assert_eq!(out.open, PathBuf::from("/o/Demo.snxprj"));
        assert_eq!(
            out.warnings,
            [("embedded image not imported".to_string(), 1)]
        );
    }

    #[test]
    fn single_document_opens_the_first_file() {
        let out =
            parse_summary(r#"{"ok":true,"project":null,"files":["/o/A.snxsch"],"warnings":[]}"#)
                .unwrap();
        assert_eq!(out.open, PathBuf::from("/o/A.snxsch"));
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn failure_carries_the_importer_error() {
        let err = parse_summary(
            r#"{"ok":false,"error":"/o/A.snxsch already exists; pass --force to replace it","files":[]}"#,
        )
        .unwrap_err();
        assert!(err.contains("already exists"));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_summary("").is_err());
        assert!(parse_summary("{not json").is_err());
        assert!(parse_summary(r#"{"ok":true,"files":[]}"#).is_err());
    }

    #[test]
    fn output_goes_beside_the_input() {
        assert_eq!(
            output_dir_for(Path::new("/d/Board.PrjPcb")),
            PathBuf::from("/d/Board-signex")
        );
    }

    #[test]
    fn warnings_render_with_counts() {
        let t = warnings_text(&[("a".into(), 1), ("b".into(), 4)]);
        assert_eq!(t, "• a\n• b (×4)");
    }

    #[test]
    fn a_bad_override_is_named_not_reported_as_missing_from_path() {
        let err = locate_in(Some("/no/such/importer".into()), None).unwrap_err();
        assert!(err.contains("/no/such/importer"), "{err}");
        assert!(!err.contains("not found on PATH"), "{err}");
    }

    #[cfg(unix)]
    mod process {
        use super::*;

        fn tmp() -> tempfile::TempDir {
            tempfile::Builder::new()
                .prefix("snx-altium")
                .tempdir()
                .unwrap()
        }

        /// A fake converter: `/bin/sh <script> --output <dir> --json`, so
        /// `$2` is the output folder. Run through `sh` rather than executed,
        /// so no test execs a file it has just written.
        fn fake(dir: &Path, body: &str) -> PathBuf {
            let script = dir.join("fake-importer.sh");
            std::fs::write(&script, body).unwrap();
            script
        }

        fn run_fake(
            body: &str,
            timeout: Duration,
        ) -> (tempfile::TempDir, Result<ImportOutcome, String>) {
            let dir = tmp();
            let script = fake(dir.path(), body);
            let out = dir.path().join("Board-signex");
            let result = run(Path::new("/bin/sh"), &script, &out, timeout);
            (dir, result)
        }

        #[test]
        fn path_lookup_skips_relative_entries() {
            let dir = tmp();
            let name = format!("{BINARY}{}", std::env::consts::EXE_SUFFIX);
            std::fs::write(dir.path().join(&name), "").unwrap();
            // The same folder, spelled relative to the working directory.
            let cwd = std::env::current_dir().unwrap();
            let up = "../".repeat(cwd.components().count() - 1);
            let relative = PathBuf::from(up).join(dir.path().strip_prefix("/").unwrap());
            assert!(relative.join(&name).is_file(), "fixture resolves");

            let err = locate_in(None, Some(relative.into_os_string())).unwrap_err();
            assert!(err.contains("not found on PATH"), "{err}");
            let found = locate_in(None, Some(dir.path().as_os_str().to_owned())).unwrap();
            assert_eq!(found, dir.path().join(&name));
        }

        #[test]
        fn a_successful_run_opens_the_reported_file_and_writes_the_report() {
            let (_dir, result) = run_fake(
                r#"mkdir -p "$2" && : > "$2/A.snxsch"
echo '{"ok":true,"project":null,"files":["A.snxsch"],"warnings":[{"message":"arc flattened","count":2}]}'
"#,
                TIMEOUT,
            );
            let outcome = result.unwrap();
            let out_dir = outcome.open.parent().unwrap().to_path_buf();
            assert_eq!(outcome.open, out_dir.join("A.snxsch"));
            let report = std::fs::read_to_string(out_dir.join(REPORT_FILE)).unwrap();
            assert!(report.contains("arc flattened (×2)"), "{report}");
        }

        #[test]
        fn a_reported_file_outside_the_output_folder_is_refused() {
            let (_dir, result) = run_fake(
                r#"mkdir -p "$2"
echo '{"ok":true,"project":"/etc/hosts","files":[],"warnings":[]}'
"#,
                TIMEOUT,
            );
            let err = result.unwrap_err();
            assert!(err.contains("not inside"), "{err}");
        }

        #[test]
        fn a_hung_importer_is_killed_at_the_deadline() {
            let started = Instant::now();
            let (_dir, result) = run_fake("sleep 30\n", Duration::from_secs(1));
            let err = result.unwrap_err();
            assert!(err.contains("timed out after 1 s"), "{err}");
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "did not wait it out"
            );
        }

        #[test]
        fn a_crash_reports_its_exit_status_and_stderr() {
            let (_dir, result) = run_fake("echo 'parser panicked' >&2\nexit 3\n", TIMEOUT);
            let err = result.unwrap_err();
            assert!(err.contains("printed no result"), "{err}");
            assert!(err.contains('3'), "exit status named: {err}");
            assert!(err.contains("parser panicked"), "{err}");
        }

        #[test]
        fn a_refusal_says_the_folder_can_be_deleted() {
            let (_dir, result) = run_fake(
                r#"mkdir -p "$2"
echo '{"ok":false,"error":"A.snxsch already exists; pass --force to replace it","files":[]}'
exit 1
"#,
                TIMEOUT,
            );
            let err = result.unwrap_err();
            assert!(
                err.starts_with("A.snxsch already exists"),
                "no exit-status noise: {err}"
            );
            assert!(err.contains("delete it and import again"), "{err}");
        }

        #[test]
        fn stderr_is_cut_to_its_tail() {
            let (_dir, result) = run_fake(
                "i=0; while [ $i -lt 2000 ]; do echo 'noise noise noise noise' >&2; i=$((i+1)); done\necho LAST >&2\nexit 1\n",
                TIMEOUT,
            );
            let err = result.unwrap_err();
            assert!(err.contains("LAST"), "the tail is kept");
            assert!(
                err.len() < STDERR_TAIL + 512,
                "bounded: {} bytes",
                err.len()
            );
        }
    }
}
