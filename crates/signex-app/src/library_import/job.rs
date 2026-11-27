//! The Windows Job object a converter runs in, so a stop reaches everything
//! it started (#3134). Created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`:
//! closing the last handle, which a Signex crash or exit also does, kills
//! every process still in the job. The converter is started suspended
//! (`CREATE_SUSPENDED`), assigned to the job, then resumed ([`resume`]), so
//! nothing it starts can escape the job. Not compiled on other systems; see
//! `docs/LIBRARY_CONVERTER_CONTRACT.md` §6.

use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, OwnedHandle};
use std::process::Child;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE;
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

/// The creation flag the converter is started with, so it runs nothing
/// before it is in its job.
pub use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

/// A job handle. `OwnedHandle` is `Send + Sync` and closes the handle once,
/// on drop, so a job shared through an `Arc` closes with its last owner.
#[derive(Debug)]
pub struct Job(OwnedHandle);

impl Job {
    /// A new job holding `child`, still suspended, which inherits it to
    /// every process it starts. `None`, logged as a warning, when the system
    /// refuses either step; the run then goes on without one, and a stop
    /// reaches only the converter itself.
    pub fn holding(child: &Child) -> Option<Self> {
        let job = Self::new()?;
        let process = child.as_raw_handle();
        // SAFETY: both are live handles: the job owned here, the process
        // owned by `child`, which outlives the call.
        if unsafe { AssignProcessToJobObject(job.raw(), process) } == 0 {
            log::warn!(
                "library import: AssignProcessToJobObject: {}",
                std::io::Error::last_os_error()
            );
            return None;
        }
        Some(job)
    }

    fn new() -> Option<Self> {
        // SAFETY: no security attributes, so the handle is not inheritable
        // (a converter holding it would keep the job open), and no name.
        let raw = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if raw.is_null() {
            log::warn!(
                "library import: CreateJobObjectW: {}",
                std::io::Error::last_os_error()
            );
            return None;
        }
        // SAFETY: `raw` is the job just created; nothing else owns it.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(raw) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = u32::try_from(size_of_val(&limits)).ok()?;
        // SAFETY: `limits` is the structure the information class names, and
        // `size` its size.
        let set = unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size,
            )
        };
        if set == 0 {
            log::warn!(
                "library import: SetInformationJobObject: {}",
                std::io::Error::last_os_error()
            );
            return None;
        }
        Some(job)
    }

    fn raw(&self) -> windows_sys::Win32::Foundation::HANDLE {
        self.0.as_raw_handle()
    }

    /// Kill every process in the job. Returns at once; the processes end
    /// shortly after ([`Self::wait_empty`]).
    pub fn terminate(&self) {
        // SAFETY: the job handle is live while `self` is.
        if unsafe { TerminateJobObject(self.raw(), 1) } == 0 {
            log::debug!(
                "library import: TerminateJobObject: {}",
                std::io::Error::last_os_error()
            );
        }
    }

    /// Wait up to `bound` until no process in the job is left, so the files
    /// they held are closed before the run cleans up or restores.
    pub fn wait_empty(&self, bound: Duration) {
        let deadline = Instant::now() + bound;
        while self.active() > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Processes in the job now; `0` when it cannot be read.
    fn active(&self) -> u32 {
        let mut info = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let Ok(size) = u32::try_from(size_of_val(&info)) else {
            return 0;
        };
        // SAFETY: `info` is the structure the information class names, and
        // `size` its size; the returned length is not wanted.
        let read = unsafe {
            QueryInformationJobObject(
                self.raw(),
                JobObjectBasicAccountingInformation,
                (&raw mut info).cast(),
                size,
                std::ptr::null_mut(),
            )
        };
        if read == 0 { 0 } else { info.ActiveProcesses }
    }
}

/// Resume the converter started with [`CREATE_SUSPENDED`]: every thread of
/// its process (a suspended process has one), found through a thread
/// snapshot since std keeps the main thread's handle to itself. An error
/// when no thread could be resumed; the caller then kills the process.
pub fn resume(child: &Child) -> std::io::Result<()> {
    let pid = child.id();
    // SAFETY: a snapshot of every thread in the system; no process handle.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `raw` is the snapshot just created; nothing else owns it.
    let snapshot = unsafe { OwnedHandle::from_raw_handle(raw) };
    let mut entry = THREADENTRY32 {
        dwSize: u32::try_from(size_of::<THREADENTRY32>()).map_err(std::io::Error::other)?,
        ..THREADENTRY32::default()
    };
    let mut resumed = 0_usize;
    // SAFETY: the snapshot is live and `entry` carries its own size.
    let mut more = unsafe { Thread32First(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            resume_thread(entry.th32ThreadID)?;
            resumed += 1;
        }
        // SAFETY: as above.
        more = unsafe { Thread32Next(snapshot.as_raw_handle(), &raw mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(std::io::Error::other(format!(
            "no thread of process {pid} to resume"
        )));
    }
    Ok(())
}

fn resume_thread(thread_id: u32) -> std::io::Result<()> {
    // SAFETY: opens the thread by id for suspend and resume only; the handle
    // is not inheritable.
    let raw = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    if raw.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `raw` was just opened; nothing else owns it.
    let thread = unsafe { OwnedHandle::from_raw_handle(raw) };
    // SAFETY: the thread handle is live and has THREAD_SUSPEND_RESUME.
    if unsafe { ResumeThread(thread.as_raw_handle()) } == u32::MAX {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}
