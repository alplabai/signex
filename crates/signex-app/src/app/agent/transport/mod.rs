//! Where an agent connects (#1924): a Unix socket in the per-user runtime
//! directory, or on Windows a named pipe only the current user can open. No
//! network, no port. Each side checks who the peer is before a byte is read.
//! See `docs/AGENT_SOCKET.md`, "Transport and peer check".

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
pub(super) use unix::{Accepted, Listener, endpoint_name};
#[cfg(windows)]
pub(super) use windows::{Accepted, Listener, endpoint_name};
