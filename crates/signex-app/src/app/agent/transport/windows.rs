//! The Windows named pipe: `\\.\pipe\signex-agent-<boot id>`. Its security
//! descriptor allows only the current user's SID, remote clients are rejected,
//! and the client's process token is read at accept and must name the same
//! user. Not compiled on other systems; see `docs/AGENT_SOCKET.md`.

use std::ffi::c_void;
use std::io;
use std::os::windows::io::AsRawHandle as _;
use std::path::Path;

use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::{
    EqualSid, GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER, TokenUser,
};
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// The pipe's name for this boot: the endpoint a registry entry names.
pub(in crate::app::agent) fn endpoint_name(_dir: &Path, boot_id: &str) -> String {
    format!(r"\\.\pipe\signex-agent-{boot_id}")
}

pub(in crate::app::agent) enum Accepted {
    Connection {
        conn: NamedPipeServer,
        pid: Option<u32>,
    },
    /// A client whose user is not this one, or could not be read.
    Refused { pid: Option<u32>, reason: String },
}

/// A token handle, closed on drop.
struct Token(HANDLE);

impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: the handle was opened by this program and is closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// The user of a token: `TOKEN_USER` and the bytes it points into, kept
/// 8-aligned.
struct TokenUserInfo(Vec<u64>);

impl TokenUserInfo {
    fn of(token: &Token) -> io::Result<Self> {
        let mut len = 0_u32;
        // SAFETY: a size query: no buffer, the length comes back in `len`.
        unsafe { GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut len) };
        if len == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buffer = vec![0_u64; (len as usize).div_ceil(8)];
        // SAFETY: `buffer` holds at least `len` bytes and lives past the call.
        let read = unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                len,
                &mut len,
            )
        };
        if read == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(buffer))
    }

    fn user(&self) -> &TOKEN_USER {
        // SAFETY: filled in by `GetTokenInformation(TokenUser)` into a buffer
        // aligned for it, and it lives as long as `self`.
        unsafe { &*self.0.as_ptr().cast::<TOKEN_USER>() }
    }

    fn same_user_as(&self, other: &Self) -> bool {
        // SAFETY: both SIDs are valid and live in the two buffers.
        unsafe { EqualSid(self.user().User.Sid, other.user().User.Sid) != 0 }
    }

    /// The user's SID as `S-1-5-...`.
    fn sid_text(&self) -> io::Result<String> {
        let mut text: *mut u16 = std::ptr::null_mut();
        // SAFETY: the SID is valid; the system allocates `text`.
        if unsafe { ConvertSidToStringSidW(self.user().User.Sid, &mut text) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `text` is a NUL-terminated wide string until `LocalFree`.
        let sid = unsafe {
            let mut len = 0;
            while *text.add(len) != 0 {
                len += 1;
            }
            let sid = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
            LocalFree(text.cast());
            sid
        };
        Ok(sid)
    }
}

fn own_token() -> io::Result<Token> {
    let mut handle: HANDLE = std::ptr::null_mut();
    // SAFETY: the current process pseudo-handle is always valid.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Token(handle))
}

/// The token of process `pid`, if this program may query it. A process of
/// another user is not open to us, which refuses it.
fn process_token(pid: u32) -> io::Result<Token> {
    // SAFETY: plain call; the handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(io::Error::last_os_error());
    }
    let process = Token(process);
    let mut handle: HANDLE = std::ptr::null_mut();
    // SAFETY: `process` is a valid handle with query access.
    if unsafe { OpenProcessToken(process.0, TOKEN_QUERY, &mut handle) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(Token(handle))
}

/// A security descriptor from SDDL text, freed on drop.
struct Descriptor(PSECURITY_DESCRIPTOR);

// SAFETY: the descriptor is immutable data owned by this value.
unsafe impl Send for Descriptor {}

impl Descriptor {
    /// Allow only `sid` (a protected DACL with one ACE), full access.
    fn only(sid: &str) -> io::Result<Self> {
        let sddl: Vec<u16> = format!("D:P(A;;GA;;;{sid})")
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut descriptor: PSECURITY_DESCRIPTOR = std::ptr::null_mut();
        // SAFETY: `sddl` is NUL-terminated; the system allocates `descriptor`.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                std::ptr::null_mut(),
            )
        };
        if converted == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self(descriptor))
    }
}

impl Drop for Descriptor {
    fn drop(&mut self) {
        // SAFETY: allocated by the system call above and freed once.
        unsafe { LocalFree(self.0.cast()) };
    }
}

pub(in crate::app::agent) struct Listener {
    name: String,
    next: NamedPipeServer,
    descriptor: Descriptor,
    own: TokenUserInfo,
}

impl Listener {
    pub(in crate::app::agent) fn bind(dir: &Path, boot_id: &str) -> io::Result<Self> {
        let name = endpoint_name(dir, boot_id);
        let own = TokenUserInfo::of(&own_token()?)?;
        let descriptor = Descriptor::only(&own.sid_text()?)?;
        let next = create_instance(&name, &descriptor, true)?;
        Ok(Self {
            name,
            next,
            descriptor,
            own,
        })
    }

    pub(in crate::app::agent) async fn accept(&mut self) -> io::Result<Accepted> {
        // A client that connects and closes at once breaks the waiting
        // instance. It is replaced with a fresh one (same descriptor, local
        // clients only, not the first instance), and that is not a failure
        // of the listener: only a run of them, or a failed replacement, is.
        let mut broken = 0_u32;
        while let Err(error) = self.next.connect().await {
            broken += 1;
            if !super::replace_broken_instance(broken) {
                return Err(error);
            }
            self.next = create_instance(&self.name, &self.descriptor, false)?;
        }
        let fresh = create_instance(&self.name, &self.descriptor, false)?;
        let conn = std::mem::replace(&mut self.next, fresh);
        let mut pid = 0_u32;
        // SAFETY: the pipe handle is valid for the call.
        let got = unsafe { GetNamedPipeClientProcessId(conn.as_raw_handle() as HANDLE, &mut pid) };
        if got == 0 {
            return Ok(Accepted::Refused {
                pid: None,
                reason: format!(
                    "the client's process could not be read: {}",
                    io::Error::last_os_error()
                ),
            });
        }
        let same_user = process_token(pid)
            .and_then(|token| TokenUserInfo::of(&token))
            .map(|peer| peer.same_user_as(&self.own));
        match same_user {
            Ok(true) => Ok(Accepted::Connection {
                conn,
                pid: Some(pid),
            }),
            Ok(false) => Ok(Accepted::Refused {
                pid: Some(pid),
                reason: "the client is another user".into(),
            }),
            Err(error) => Ok(Accepted::Refused {
                pid: Some(pid),
                reason: format!("the client's user could not be read: {error}"),
            }),
        }
    }

    /// A pipe has no file to remove.
    pub(in crate::app::agent) fn remove(_dir: &Path, _boot_id: &str) -> Result<(), String> {
        Ok(())
    }
}

/// One server instance of the pipe, for the current user only, local clients
/// only.
fn create_instance(
    name: &str,
    descriptor: &Descriptor,
    first: bool,
) -> io::Result<NamedPipeServer> {
    let mut attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    // SAFETY: `attributes` and the descriptor it points to outlive the call,
    // which copies what it needs.
    unsafe {
        ServerOptions::new()
            .first_pipe_instance(first)
            .reject_remote_clients(true)
            .create_with_security_attributes_raw(
                name,
                std::ptr::from_mut(&mut attributes).cast::<c_void>(),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};
    use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

    use super::*;

    /// `ERROR_PIPE_BUSY`: every instance of the pipe is in use for a moment.
    const PIPE_BUSY: i32 = 231;

    /// A boot id no other test or run shares.
    fn boot(tag: &str) -> String {
        format!("wintest-{tag}-{}", std::process::id())
    }

    /// Open the pipe as a client, waiting out the moments it is busy.
    async fn open_client(name: &str) -> io::Result<NamedPipeClient> {
        for _ in 0..500 {
            match ClientOptions::new().open(name) {
                Ok(client) => return Ok(client),
                Err(error) if error.raw_os_error() == Some(PIPE_BUSY) => {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::other("the pipe stayed busy"))
    }

    enum Event {
        Peer(Option<u32>),
        Failure(String),
    }

    /// A stand-in for the listener's connection handling: answers a line with
    /// `ok`, then waits for the client to close. A client that closed before
    /// it was read (a refusal) is skipped; only an `accept` error is reported.
    async fn serve_replies(mut listener: Listener, events: UnboundedSender<Event>) {
        loop {
            match listener.accept().await {
                Ok(Accepted::Connection { conn, pid }) => {
                    events.send(Event::Peer(pid)).ok();
                    tokio::spawn(async move {
                        let mut conn = BufReader::new(conn);
                        let mut line = String::new();
                        if matches!(conn.read_line(&mut line).await, Ok(read) if read > 0) {
                            conn.get_mut().write_all(b"ok\n").await.ok();
                            conn.get_mut().flush().await.ok();
                            line.clear();
                            conn.read_line(&mut line).await.ok();
                        }
                    });
                }
                Ok(Accepted::Refused { .. }) => {}
                Err(error) => {
                    events.send(Event::Failure(error.to_string())).ok();
                    return;
                }
            }
        }
    }

    async fn ask(name: &str, text: &str) -> String {
        let mut client = BufReader::new(open_client(name).await.expect("a client"));
        client.get_mut().write_all(text.as_bytes()).await.unwrap();
        let mut reply = String::new();
        tokio::time::timeout(Duration::from_secs(10), client.read_line(&mut reply))
            .await
            .expect("an answer in time")
            .expect("a readable answer");
        reply
    }

    #[tokio::test]
    async fn a_request_round_trips_over_the_pipe_and_its_client_is_this_process() {
        let id = boot("round");
        let listener = Listener::bind(Path::new(""), &id).expect("bound");
        let name = endpoint_name(Path::new(""), &id);
        let (events, mut seen) = unbounded_channel();
        let server = tokio::spawn(serve_replies(listener, events));

        assert_eq!(ask(&name, "hello\n").await, "ok\n");

        let Some(Event::Peer(pid)) = seen.recv().await else {
            panic!("the listener did not accept the client");
        };
        assert_eq!(pid, Some(std::process::id()), "the peer check read us");
        server.abort();
    }

    #[tokio::test]
    async fn clients_that_connect_and_close_at_once_never_stop_the_listener() {
        let id = boot("churn");
        let listener = Listener::bind(Path::new(""), &id).expect("bound");
        let name = endpoint_name(Path::new(""), &id);
        let (events, mut seen) = unbounded_channel();
        let server = tokio::spawn(serve_replies(listener, events));

        for _ in 0..100 {
            drop(open_client(&name).await.expect("a client"));
            tokio::task::yield_now().await;
        }
        // A real request still gets its answer, and the listener never
        // reported a failure.
        assert_eq!(ask(&name, "hello\n").await, "ok\n");
        while let Ok(event) = seen.try_recv() {
            if let Event::Failure(why) = event {
                panic!("the listener stopped: {why}");
            }
        }
        assert!(!server.is_finished(), "the listener is still running");
        server.abort();
    }

    #[tokio::test]
    async fn a_second_pipe_of_the_same_name_cannot_be_made_while_one_is_open() {
        let id = boot("squat");
        let _first = Listener::bind(Path::new(""), &id).expect("bound");
        assert!(
            Listener::bind(Path::new(""), &id).is_err(),
            "another process could not take the name either"
        );
    }
}
