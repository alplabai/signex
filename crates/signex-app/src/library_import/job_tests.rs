//! Windows: a stopped converter takes what it started with it (#3134). The
//! fake converter is a PowerShell script that starts a long `ping`, writes
//! its process id, and sleeps; the Unix twins are in `run_tests.rs`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};

use super::error::ImportError;
use super::process::ProcessGroup;
use super::run::{
    ConverterCommand, LibraryRunRequest, ModelsMode, RunFailure, RunLimits, RunMode, RunOutcome,
    run_library_in,
};

/// PowerShell is slow to start on a loaded runner.
const STARTED_WITHIN: Duration = Duration::from_secs(30);

/// A converter whose child `ping` runs for 30 s; its id goes to `pid_file`.
fn converter_with_child(dir: &Path, pid_file: &Path) -> ConverterCommand {
    let script = dir.join("fake-converter.ps1");
    let body = format!(
        "$child = Start-Process -FilePath ping.exe -ArgumentList '-n','30','127.0.0.1' \
         -WindowStyle Hidden -PassThru\n\
         Set-Content -LiteralPath '{}' -Value $child.Id -Encoding Ascii\n\
         Start-Sleep -Seconds 30\n",
        pid_file.display()
    );
    std::fs::write(&script, body).unwrap();
    ConverterCommand {
        program: PathBuf::from("powershell.exe"),
        leading_args: [
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ]
        .into_iter()
        .map(Into::into)
        .chain([script.into_os_string()])
        .collect(),
    }
}

fn request(dir: &Path) -> LibraryRunRequest {
    LibraryRunRequest {
        inputs: vec![dir.join("Passives.lib1")],
        output: dir.join("P"),
        name: Some("Passives".into()),
        db: None,
        search_paths: vec![],
        models: ModelsMode::Include,
        mode: RunMode::Import,
    }
}

/// The child's id once the converter has written it.
fn child_pid(pid_file: &Path) -> u32 {
    let deadline = Instant::now() + STARTED_WITHIN;
    loop {
        let read = std::fs::read_to_string(pid_file).ok();
        if let Some(pid) = read.and_then(|text| text.trim().parse().ok()) {
            return pid;
        }
        assert!(
            Instant::now() < deadline,
            "the converter never started its child"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Whether process `pid` has ended within `bound`.
fn ended_within(pid: u32, bound: Duration) -> bool {
    // SAFETY: opening a process by id only for waiting on it.
    let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
    if process.is_null() {
        return true;
    }
    let millis = u32::try_from(bound.as_millis()).unwrap_or(u32::MAX);
    // SAFETY: `process` was just opened and is closed once, right after.
    let waited = unsafe { WaitForSingleObject(process, millis) };
    // SAFETY: as above.
    unsafe { CloseHandle(process) };
    waited == WAIT_OBJECT_0
}

/// Run the converter on a thread, call `stop` once its child runs, and
/// return the result and the child's id.
fn stop_once_started(
    stop: impl FnOnce(&AtomicBool, &ProcessGroup),
) -> (Result<RunOutcome, RunFailure>, u32) {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("child.pid");
    let command = converter_with_child(dir.path(), &pid_file);
    let request = request(dir.path());
    let cancel = AtomicBool::new(false);
    let group = ProcessGroup::default();
    let limits = RunLimits {
        silence: Duration::from_secs(120),
        poll: Duration::from_millis(10),
    };
    std::thread::scope(|scope| {
        let handle = scope
            .spawn(|| run_library_in(&command, &request, limits, &cancel, &group, &mut |_| {}));
        let pid = child_pid(&pid_file);
        stop(&cancel, &group);
        (handle.join().unwrap(), pid)
    })
}

#[test]
fn cancel_kills_what_the_converter_started() {
    let (result, pid) = stop_once_started(|cancel, _| cancel.store(true, Ordering::Relaxed));
    assert_eq!(*result.unwrap_err().error, ImportError::Cancelled);
    assert!(
        ended_within(pid, Duration::from_secs(5)),
        "the converter's child was killed"
    );
}

/// Quitting reaches the job through the published group, without the
/// run's own stop.
#[test]
fn the_published_group_kills_the_converter_and_its_child() {
    let started = Instant::now();
    let (result, pid) = stop_once_started(|_, group| {
        assert!(group.current().is_some(), "published while it runs");
        group.kill();
    });
    assert!(result.is_err(), "killed, so no library");
    assert!(started.elapsed() < STARTED_WITHIN + Duration::from_secs(10));
    assert!(
        ended_within(pid, Duration::from_secs(5)),
        "the converter's child was killed"
    );
}
