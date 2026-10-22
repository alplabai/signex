//! Shared blocking-HTTP client construction for the distributor adapters.
//!
//! Every adapter builds its `reqwest::blocking::Client` through
//! [`client_builder`] so the connect and overall-request timeouts are
//! defined once. The 30 s overall bound matches reqwest's implicit blocking
//! default but is now explicit; the 10 s connect cap is additional.

use std::time::Duration;

/// Maximum time to establish the TCP/TLS connection.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Maximum time for one whole request (connect + send + response body).
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

const USER_AGENT: &str = "signex-library/0.9 (+https://signex.dev)";

/// A client builder preloaded with the user agent and both timeouts.
/// Callers add adapter-specific options (e.g. redirect policy), then pass
/// it to [`finish`].
pub(super) fn client_builder() -> reqwest::blocking::ClientBuilder {
    builder_with_timeouts(HTTP_CONNECT_TIMEOUT, HTTP_REQUEST_TIMEOUT)
}

fn builder_with_timeouts(connect: Duration, request: Duration) -> reqwest::blocking::ClientBuilder {
    reqwest::blocking::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(connect)
        .timeout(request)
}

/// Default adapter client: [`client_builder`] with no extra options.
pub(super) fn build_client() -> reqwest::blocking::Client {
    finish(client_builder())
}

/// Build the client. Failure only occurs when the platform TLS backend
/// cannot initialise, which no caller can recover from.
pub(super) fn finish(builder: reqwest::blocking::ClientBuilder) -> reqwest::blocking::Client {
    builder
        .build()
        .expect("reqwest::blocking::Client::build fails only if the TLS backend cannot initialise")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::net::TcpListener;
    use std::time::Instant;

    #[test]
    fn built_client_times_out_against_a_server_that_never_answers() {
        // Accepts the connection, reads the request, never responds.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let _server = std::thread::spawn(move || {
            if let Ok((mut s, _)) = listener.accept() {
                let mut buf = [0u8; 1024];
                let _ = s.read(&mut buf);
                std::thread::sleep(Duration::from_secs(5));
            }
        });
        // Same code path as `client_builder`, with a short overall timeout.
        let client = finish(builder_with_timeouts(
            Duration::from_secs(2),
            Duration::from_millis(300),
        ));
        let t0 = Instant::now();
        let err = client.get(format!("http://{addr}/")).send().unwrap_err();
        assert!(err.is_timeout(), "expected timeout, got {err}");
        assert!(t0.elapsed() < Duration::from_secs(4));
    }

    #[test]
    fn built_client_times_out_connecting_to_a_non_routable_address() {
        // 10.255.255.1 is in a private range nothing answers on; the SYN is
        // dropped (or the network is unreachable), so only the connect
        // timeout can end the call. Either outcome below is accepted as long
        // as it is a prompt error rather than a hang; on a host that routes
        // it and fails fast the error is still a connect failure.
        let client = finish(builder_with_timeouts(
            Duration::from_millis(300),
            Duration::from_secs(20),
        ));
        let t0 = Instant::now();
        let err = client.get("http://10.255.255.1/").send().unwrap_err();
        assert!(
            err.is_timeout() || err.is_connect(),
            "expected a connect failure, got {err}"
        );
        assert!(t0.elapsed() < Duration::from_secs(15));
    }
}
