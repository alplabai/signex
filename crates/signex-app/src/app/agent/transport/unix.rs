//! The Unix socket: `<runtime dir>/<boot id>.sock`, mode 0600, in a folder that
//! is mode 0700 and ours. The peer's user id comes from the kernel
//! (`getpeereid` on macOS, `SO_PEERCRED` on Linux, through tokio) and must be
//! ours.

use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};

use super::super::files;

/// The socket's path for this boot, as text: the endpoint a registry entry
/// names.
pub(in crate::app::agent) fn endpoint_name(dir: &Path, boot_id: &str) -> String {
    socket_path(dir, boot_id).to_string_lossy().into_owned()
}

fn socket_path(dir: &Path, boot_id: &str) -> PathBuf {
    dir.join(format!("{boot_id}.sock"))
}

/// Whether a connection from user `peer_uid` may be served by a process of
/// user `own_uid`: only the same user. The one decision, apart from reading
/// the ids, so it is tested alone.
pub(in crate::app::agent) fn peer_allowed(peer_uid: u32, own_uid: u32) -> bool {
    peer_uid == own_uid
}

pub(in crate::app::agent) enum Accepted {
    Connection {
        conn: UnixStream,
        pid: Option<u32>,
    },
    /// A peer that is not the user running the app, or whose user the system
    /// could not tell. Nothing was read from it.
    Refused {
        pid: Option<u32>,
        reason: String,
    },
}

pub(in crate::app::agent) struct Listener {
    inner: UnixListener,
    own_uid: u32,
}

impl Listener {
    /// Bind the socket for `boot_id` in `dir` and make it 0600. A stale file
    /// at the path is removed only if it is our own plain file or socket; a
    /// link or another user's file stops the start.
    ///
    /// The socket is made with the process's umask and `chmod`ed right after.
    /// A `umask` call would be process-wide and race every other thread that
    /// creates a file, so it is not used: until the `chmod` the socket is
    /// still unreachable to others, because `dir` is ours and mode 0700
    /// (checked by `files::prepare_dir` before this runs).
    pub(in crate::app::agent) fn bind(dir: &Path, boot_id: &str) -> io::Result<Self> {
        let path = socket_path(dir, boot_id);
        files::remove_if_ours(&path).map_err(io::Error::other)?;
        let inner = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            inner,
            own_uid: files::own_uid(),
        })
    }

    pub(in crate::app::agent) async fn accept(&mut self) -> io::Result<Accepted> {
        let (conn, _) = self.inner.accept().await?;
        let cred = match conn.peer_cred() {
            Ok(cred) => cred,
            Err(error) => {
                return Ok(Accepted::Refused {
                    pid: None,
                    reason: format!("the peer's user could not be read: {error}"),
                });
            }
        };
        let pid = cred.pid().and_then(|pid| u32::try_from(pid).ok());
        if peer_allowed(cred.uid(), self.own_uid) {
            Ok(Accepted::Connection { conn, pid })
        } else {
            Ok(Accepted::Refused {
                pid,
                reason: format!(
                    "user {} is not the user running Signex ({})",
                    cred.uid(),
                    self.own_uid
                ),
            })
        }
    }

    /// Remove the socket file of `boot_id` (the listener is closing).
    pub(in crate::app::agent) fn remove(dir: &Path, boot_id: &str) -> Result<(), String> {
        files::remove_if_ours(&socket_path(dir, boot_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The peer's user comes from the kernel; faking another one in a test is
    /// not possible, so the decision made on it is tested alone, and that the
    /// listener reads the kernel's answer for our own connection.
    #[test]
    fn only_the_user_running_the_app_is_served() {
        assert!(peer_allowed(501, 501));
        assert!(peer_allowed(0, 0));
        assert!(!peer_allowed(0, 501), "root is another user");
        assert!(!peer_allowed(502, 501));
        assert!(!peer_allowed(501, 0));
        assert!(!peer_allowed(u32::MAX, 501));
    }

    #[test]
    fn a_bound_socket_is_0600_and_sits_in_a_0700_folder_whatever_the_umask() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = tempfile::Builder::new()
            .prefix("snx-1924-bind")
            .tempdir()
            .unwrap();
        let dir = root.path().join("run");
        files::prepare_dir(&dir).unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();
        let _listener = Listener::bind(&dir, "bound").expect("bound");
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir), 0o700);
        assert_eq!(mode(&socket_path(&dir, "bound")), 0o600);
    }

    #[test]
    fn our_own_connection_reports_our_user_and_pid() {
        let dir = tempfile::Builder::new()
            .prefix("snx-1924-peer")
            .tempdir()
            .unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut listener = Listener::bind(dir.path(), "peer").expect("bound");
            let socket = socket_path(dir.path(), "peer");
            let client = tokio::net::UnixStream::connect(&socket).await.unwrap();
            let accepted = listener.accept().await.unwrap();
            let Accepted::Connection { pid, .. } = accepted else {
                panic!("our own connection was refused");
            };
            assert_eq!(pid, Some(std::process::id()));
            drop(client);
        });
    }
}
