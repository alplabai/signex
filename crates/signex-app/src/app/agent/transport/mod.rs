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

/// How many broken instances of the pipe in a row (a client that connects and
/// closes at once) are replaced before `accept` reports the error.
#[cfg(any(windows, test))]
const MAX_BROKEN_INSTANCES: u32 = 64;

/// Whether the `broken_in_a_row`-th broken instance is replaced and the wait
/// goes on, rather than reported as a failure of the listener.
#[cfg(any(windows, test))]
fn replace_broken_instance(broken_in_a_row: u32) -> bool {
    broken_in_a_row <= MAX_BROKEN_INSTANCES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_client_that_connects_and_closes_at_once_never_stops_the_listener() {
        // More of them than the listener tolerates failed accepts: each is
        // replaced and none is counted.
        for broken in 1..=MAX_BROKEN_INSTANCES {
            assert!(replace_broken_instance(broken), "{broken}");
        }
        const { assert!(MAX_BROKEN_INSTANCES > 20, "above the failed-accept limit") };
        // A listener that cannot keep an instance alive still gives up.
        assert!(!replace_broken_instance(MAX_BROKEN_INSTANCES + 1));
    }
}
