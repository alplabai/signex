//! DigiKey OAuth2 PKCE handshake — UI side.
//!
//! Flow:
//! - "Connect via OAuth" → `DigiKeyAuth::start_authorization` → open
//!   the URL in the user's default browser via `webbrowser`.
//! - Spin up a one-shot HTTP server on `127.0.0.1:<random_port>` that
//!   accepts the `?code=&state=` callback.
//! - Pass the returned `code` + the matching CSRF state back to
//!   `DigiKeyAuth::exchange_code` (which internally persists the
//!   refresh token via `KeyringStore`).
//!
//! Why blocking, not async: the underlying `oauth2`/`reqwest` calls
//! that `signex-library` exposes are blocking, and the iced runtime
//! happily spawns blocking work via `Task::perform` over `tokio`'s
//! `spawn_blocking`. Keeping the whole flow blocking inside one
//! function makes the borrow shape obvious and avoids needing a
//! parallel async branch in `signex-library`.
//!
//! Configuration:
//! - DigiKey client_id / client_secret are read from the environment
//!   (`SIGNEX_DIGIKEY_CLIENT_ID` / `SIGNEX_DIGIKEY_CLIENT_SECRET`).
//!   Unit tests use a wiremock server, so the constants are never
//!   committed to source.
//!
//! Cancellation:
//! - The caller holds a `CancelHandle` that, when dropped or via
//!   `cancel()`, asks the listener to stop blocking on the next
//!   `recv_timeout`. The handler observes cancellation and returns
//!   [`Outcome::Cancelled`].

use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use signex_library::distributors::digikey::{DigiKeyAuth, DigiKeyAuthError};

use crate::ignore::IgnoreResult;

/// Environment variable that holds the DigiKey OAuth client_id.
pub const ENV_CLIENT_ID: &str = "SIGNEX_DIGIKEY_CLIENT_ID";
/// Environment variable that holds the DigiKey OAuth client_secret.
pub const ENV_CLIENT_SECRET: &str = "SIGNEX_DIGIKEY_CLIENT_SECRET";

/// Outcome of the OAuth handshake. Returned via the iced `Task` that
/// drives the flow.
#[derive(Debug, Clone)]
pub enum Outcome {
    /// Auth succeeded — the access token is held by `DigiKeyAuth` for
    /// the duration of the process; the refresh token is persisted in
    /// the OS keyring under `signex-distributor-digikey/refresh`. The
    /// returned string is a user-facing identifier (best-effort: the
    /// canonical email isn't returned by the token endpoint, so we
    /// fall back to "Connected" here and let the panel tweak the
    /// label later when an account-info call lands).
    Connected { account_label: String },
    /// User-visible reason. Surfaced as "Failed: <reason>".
    Failed { reason: String },
    /// User clicked Cancel before the browser callback fired.
    Cancelled,
}

/// Hands the caller a way to cancel the in-flight handshake. Cloning
/// is cheap and lets the iced `Cancel` button dispatch a cancel from
/// any thread.
#[derive(Debug, Clone)]
pub struct CancelHandle {
    flag: Arc<AtomicBool>,
}

impl CancelHandle {
    pub fn new() -> Self {
        Self {
            flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wrap an existing flag — used by the dispatcher so the UI can
    /// hold the same `AtomicBool` it later mutates from the Cancel
    /// button.
    pub fn from_flag(flag: Arc<AtomicBool>) -> Self {
        Self { flag }
    }

    /// Mark the flow as cancelled. The owning thread mutates the
    /// shared flag directly via `Arc<AtomicBool>` in production; this
    /// helper is kept as a convenient handle for tests + future
    /// in-process cancel paths.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl Default for CancelHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Bind a localhost port for the OAuth callback server. Returns the
/// listener (used to receive exactly one redirect) and the URL to
/// register with `DigiKeyAuth` as the redirect target.
///
/// We bind to `127.0.0.1` (loopback) only — never `0.0.0.0`. The
/// kernel chooses a random free port via `:0` so multiple parallel
/// flows don't collide.
fn bind_callback_listener() -> Result<(TcpListener, String), std::io::Error> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}/callback");
    Ok((listener, url))
}

/// Body of the synchronous handshake. Runs on a worker thread (caller
/// wraps this in `Task::perform` over `tokio::task::spawn_blocking`).
///
/// `auth_url_endpoint` / `token_url_endpoint` let tests redirect at a
/// wiremock instance; production callers pass the DigiKey constants
/// from `signex-library`.
///
/// The function is split so the `cargo test` path can drive it end-
/// to-end against wiremock without needing a real browser.
pub fn run_blocking(
    client_id: String,
    client_secret: String,
    auth_url_endpoint: String,
    token_url_endpoint: String,
    cancel: CancelHandle,
    open_browser: bool,
) -> Outcome {
    if client_id.is_empty() {
        return Outcome::Failed {
            reason: format!(
                "DigiKey OAuth client_id missing — set ${ENV_CLIENT_ID} before connecting."
            ),
        };
    }

    let (listener, redirect_uri) = match bind_callback_listener() {
        Ok(t) => t,
        Err(e) => {
            return Outcome::Failed {
                reason: format!("could not bind localhost callback: {e}"),
            };
        }
    };

    let auth = match DigiKeyAuth::with_endpoints(
        client_id,
        client_secret,
        redirect_uri,
        &auth_url_endpoint,
        &token_url_endpoint,
    ) {
        Ok(a) => a,
        Err(e) => {
            return failure_from(e);
        }
    };

    let (auth_url, csrf_token, verifier) = auth.start_authorization();

    if open_browser && let Err(e) = webbrowser::open(auth_url.as_str()) {
        return Outcome::Failed {
            reason: format!("could not open browser: {e}"),
        };
    }

    // Block on the listener with a short polling loop so cancellation
    // fires inside a few hundred ms. `set_nonblocking` would also
    // work, but the polling pattern is more readable.
    if let Err(e) = listener.set_nonblocking(true) {
        return Outcome::Failed {
            reason: format!("listener nonblocking failed: {e}"),
        };
    }

    let timeout = Duration::from_secs(5 * 60); // 5 minutes
    let started = std::time::Instant::now();
    loop {
        if cancel.is_cancelled() {
            return Outcome::Cancelled;
        }
        if started.elapsed() > timeout {
            return Outcome::Failed {
                reason: "timed out waiting for browser callback".into(),
            };
        }
        match listener.accept() {
            Ok((mut stream, addr)) => {
                // MD-14: only loopback should be hitting this listener.
                // Reject any non-loopback peer (DNS-rebinding hardening).
                if !addr.ip().is_loopback() {
                    stream
                        .shutdown(std::net::Shutdown::Both)
                        .ignore("rejected peer; the connection is abandoned either way");
                    continue;
                }
                let req_line = read_first_line(&mut stream);
                let response_body = "<html><body><p>Signex: DigiKey connected. You can close this window.</p></body></html>";
                // Courtesy page only. `req_line` was already read above and
                // `parse_callback` below works entirely from it, so the OAuth
                // exchange completes whether or not this reaches the browser
                // — a browser that closed early costs the user the "you can
                // close this window" page and nothing else.
                std::io::Write::write_all(
                    &mut stream,
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    )
                    .as_bytes(),
                )
                .ignore("confirmation page only; the callback is already captured in `req_line`");
                stream.shutdown(std::net::Shutdown::Both).ignore(
                    "connection is finished with; a failed shutdown means the peer closed first",
                );

                let (code, returned_state) = match callback_params(req_line) {
                    Ok(t) => t,
                    Err(outcome) => return outcome,
                };

                return match auth.exchange_code(&code, verifier, &returned_state, &csrf_token) {
                    Ok(_access_token) => Outcome::Connected {
                        // No identity claim is returned by the token
                        // endpoint in DigiKey's spec; the panel
                        // labels the connection generically until a
                        // follow-up `/me` call lands.
                        account_label: "DigiKey".to_string(),
                    },
                    Err(e) => failure_from(e),
                };
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => {
                return Outcome::Failed {
                    reason: format!("listener accept: {e}"),
                };
            }
        }
    }
}

fn failure_from(e: DigiKeyAuthError) -> Outcome {
    Outcome::Failed {
        reason: e.to_string(),
    }
}

/// Read the first line of the HTTP request — only the line we need
/// to extract `?code=&state=` from. We intentionally avoid pulling in
/// the full `tiny_http` server here because it's overkill for one
/// request and adds a fork of the request lifecycle that doesn't
/// blend with the polling/cancel pattern. (We still link to it via
/// `Cargo.toml` for symmetry with the WS specs in case the flow
/// grows; the polled-listener version above is what runs.)
///
/// The read error is returned rather than folded into an empty string:
/// an empty line parses as "the redirect carried no code/state", which
/// is a completely different diagnosis from "the socket could not be
/// read at all" and sends the user to inspect the wrong thing.
fn read_first_line<R: std::io::Read>(stream: &mut R) -> Result<String, std::io::Error> {
    let mut buf = [0u8; 4096];
    let n = stream.read(&mut buf)?;
    let s = String::from_utf8_lossy(&buf[..n]);
    Ok(s.lines().next().unwrap_or("").to_string())
}

/// Turn what came off the callback socket into the `(code, state)` pair,
/// or into the [`Outcome::Failed`] that names which of the two very
/// different failures happened: the socket could not be read, or a
/// redirect did arrive and carries no `code`/`state`.
///
/// Both arms report to the Messages panel at `error!` — the connect
/// attempt ended with no account connected either way, and the status
/// line in the settings panel is overwritten by the next attempt.
fn callback_params(req_line: Result<String, std::io::Error>) -> Result<(String, String), Outcome> {
    let line = match req_line {
        Ok(line) => line,
        Err(error) => {
            tracing::error!(
                target: "signex::distributor",
                distributor = "digikey",
                error = %error,
                error_kind = ?error.kind(),
                "the DigiKey OAuth callback connection could not be read, so the account was \
                 not connected; the redirect itself was never seen — this is a socket failure \
                 on the loopback listener, not a problem with the redirect URI registered on \
                 the DigiKey app"
            );
            return Err(Outcome::Failed {
                reason: format!(
                    "could not read the browser's redirect from the callback socket: {error}"
                ),
            });
        }
    };
    match parse_callback(&line) {
        Some(pair) => Ok(pair),
        None => {
            tracing::error!(
                target: "signex::distributor",
                distributor = "digikey",
                query_keys = %callback_query_keys(&line),
                "the browser's redirect reached Signex but carries no code/state pair, so the \
                 DigiKey account was not connected; check the redirect URI registered on the \
                 DigiKey app"
            );
            Err(Outcome::Failed {
                reason: "redirect missing code/state".into(),
            })
        }
    }
}

/// Names of the query parameters on a callback request line, joined for
/// the log record. Values are deliberately left out — one of them is the
/// authorization code, and the Messages panel is read out loud in bug
/// reports.
fn callback_query_keys(req_line: &str) -> String {
    let Some(target) = req_line.split_whitespace().nth(1) else {
        return "<no request target>".to_string();
    };
    let Some((_, query)) = target.split_once('?') else {
        return "<no query string>".to_string();
    };
    let keys: Vec<&str> = query
        .split('&')
        .map(|pair| pair.split_once('=').map_or(pair, |(k, _)| k))
        .collect();
    keys.join(",")
}

/// Extract `code` and `state` query params from the first request
/// line `GET /callback?code=...&state=... HTTP/1.1`.
fn parse_callback(req_line: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let target = parts[1];
    let q = target.split_once('?')?.1;
    let mut code = None;
    let mut state = None;
    for pair in q.split('&') {
        let (k, v) = pair.split_once('=')?;
        let v_owned = url_decode(v);
        match k {
            "code" => code = Some(v_owned),
            "state" => state = Some(v_owned),
            _ => {}
        }
    }
    Some((code?, state?))
}

fn url_decode(s: &str) -> String {
    // Tiny URL decoder — the callback only carries opaque tokens and
    // a state string. Full encoding gymnastics are unnecessary here;
    // we cover `+` → space and `%XX` decoding.
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                let hi = hex_digit(bytes[i + 1]);
                let lo = hex_digit(bytes[i + 2]);
                if let (Some(h), Some(l)) = (hi, lo) {
                    out.push((h << 4) | l);
                    i += 2;
                } else {
                    out.push(bytes[i]);
                }
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Read environment-supplied DigiKey credentials. Returns empty
/// strings when unset — the caller treats empty client_id as "not
/// configured" and surfaces a clear failure.
pub fn read_env_credentials() -> (String, String) {
    let id = std::env::var(ENV_CLIENT_ID).unwrap_or_default();
    let secret = std::env::var(ENV_CLIENT_SECRET).unwrap_or_default();
    (id, secret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_callback_extracts_code_and_state() {
        let line = "GET /callback?code=abc&state=xyz HTTP/1.1";
        let got = parse_callback(line);
        assert_eq!(got, Some(("abc".into(), "xyz".into())));
    }

    #[test]
    fn parse_callback_handles_url_encoding() {
        let line = "GET /callback?code=a%2Bb&state=q%20w HTTP/1.1";
        let got = parse_callback(line).expect("parses");
        assert_eq!(got.0, "a+b");
        assert_eq!(got.1, "q w");
    }

    #[test]
    fn parse_callback_returns_none_on_missing_query() {
        assert!(parse_callback("GET /callback HTTP/1.1").is_none());
    }

    /// A socket that refuses every read, standing in for a connection
    /// reset, a timeout, or a port scanner that opens and drops.
    struct FailingReader(std::io::ErrorKind);

    impl std::io::Read for FailingReader {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(self.0, "connection reset by peer"))
        }
    }

    fn failed_reason(outcome: Outcome) -> String {
        match outcome {
            Outcome::Failed { reason } => reason,
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn read_first_line_returns_the_request_line() {
        let mut stream = "GET /callback?code=abc&state=xyz HTTP/1.1\r\nHost: x\r\n\r\n".as_bytes();
        let got = read_first_line(&mut stream).expect("reads");
        assert_eq!(got, "GET /callback?code=abc&state=xyz HTTP/1.1");
    }

    #[test]
    fn read_first_line_propagates_a_socket_read_error() {
        let mut stream = FailingReader(std::io::ErrorKind::ConnectionReset);
        let error = read_first_line(&mut stream).expect_err("read fails");
        assert_eq!(error.kind(), std::io::ErrorKind::ConnectionReset);
    }

    #[test]
    fn callback_params_returns_code_and_state_on_a_good_redirect() {
        let got = callback_params(Ok("GET /callback?code=abc&state=xyz HTTP/1.1".to_string()))
            .expect("parses");
        assert_eq!(got, ("abc".to_string(), "xyz".to_string()));
    }

    /// The row this test pins: a socket that could not be read and a
    /// redirect that genuinely carries no `code`/`state` are two
    /// different diagnoses and must not share one reason string. Before
    /// the fix both produced "redirect missing code/state", sending a
    /// user whose loopback connection was reset off to inspect the
    /// redirect URI registered on their DigiKey app.
    #[test]
    fn socket_read_error_and_malformed_redirect_report_different_reasons() {
        let socket = failed_reason(
            callback_params(Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionReset,
                "connection reset by peer",
            )))
            .expect_err("socket failure"),
        );
        let malformed = failed_reason(
            callback_params(Ok("GET /callback HTTP/1.1".to_string())).expect_err("no code/state"),
        );

        assert_ne!(socket, malformed);
        assert_eq!(malformed, "redirect missing code/state");
        assert!(
            socket.contains("could not read"),
            "socket failure must say the redirect was never read, got {socket:?}"
        );
        assert!(
            socket.contains("connection reset by peer"),
            "socket failure must carry the underlying I/O error, got {socket:?}"
        );
        assert!(
            !socket.contains("redirect missing code/state"),
            "socket failure must not be reported as a malformed redirect, got {socket:?}"
        );
    }

    /// An empty first line is what a failed read used to look like. It
    /// still means "malformed redirect" when it really is one — a peer
    /// that connected and sent nothing — so the two paths stay apart
    /// only because the error is carried, not inferred from emptiness.
    #[test]
    fn an_empty_request_line_is_still_a_malformed_redirect() {
        let reason = failed_reason(callback_params(Ok(String::new())).expect_err("no code/state"));
        assert_eq!(reason, "redirect missing code/state");
    }

    #[test]
    fn callback_query_keys_logs_names_without_values() {
        let keys = callback_query_keys("GET /callback?code=s3cret&state=xyz HTTP/1.1");
        assert_eq!(keys, "code,state");
        assert!(
            !keys.contains("s3cret"),
            "the authorization code must not be logged"
        );
        assert_eq!(
            callback_query_keys("GET /callback HTTP/1.1"),
            "<no query string>"
        );
        assert_eq!(callback_query_keys(""), "<no request target>");
    }

    #[test]
    fn missing_client_id_fails_clearly() {
        let cancel = CancelHandle::new();
        let outcome = run_blocking(
            String::new(),
            "secret".into(),
            "http://127.0.0.1/auth".into(),
            "http://127.0.0.1/token".into(),
            cancel,
            false,
        );
        match outcome {
            Outcome::Failed { reason } => assert!(reason.contains("client_id")),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn cancel_handle_observable_across_clones() {
        let h = CancelHandle::new();
        let h2 = h.clone();
        assert!(!h.is_cancelled());
        h2.cancel();
        assert!(h.is_cancelled());
    }
}
