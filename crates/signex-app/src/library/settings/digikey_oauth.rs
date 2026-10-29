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
//! - Setting the shared flag (`CancelHandle::cancel`, or the dispatcher's
//!   `DigiKeyCancel`) is observed between `accept` polls, before the
//!   exchange, and at the persist veto; the flow then returns
//!   [`Outcome::Cancelled`]. A token stored before the cancel landed is
//!   removed by the stale-result handler.

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
    /// fall back to "DigiKey" here and let the panel tweak the
    /// label later when an account-info call lands).
    Connected { account_label: String },
    /// User-visible reason. Surfaced as "Failed: <reason>".
    Failed { reason: String },
    /// Cancel landed before the refresh token was stored.
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
        return fail_and_log(format!(
            "DigiKey OAuth client_id missing — set ${ENV_CLIENT_ID} before connecting."
        ));
    }

    let (listener, redirect_uri) = match bind_callback_listener() {
        Ok(t) => t,
        Err(e) => {
            return fail_and_log(format!("could not bind localhost callback: {e}"));
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
    // Consumed by the one real exchange; an idle connection never takes it.
    let mut verifier = Some(verifier);

    if open_browser && let Err(e) = webbrowser::open(auth_url.as_str()) {
        return fail_and_log(format!("could not open browser: {e}"));
    }

    // Block on the listener with a short polling loop so cancellation
    // fires inside a few hundred ms. `set_nonblocking` would also
    // work, but the polling pattern is more readable.
    if let Err(e) = listener.set_nonblocking(true) {
        return fail_and_log(format!("listener nonblocking failed: {e}"));
    }

    let timeout = Duration::from_secs(5 * 60); // 5 minutes
    let started = std::time::Instant::now();
    loop {
        if cancel.is_cancelled() {
            return cancelled_outcome();
        }
        if started.elapsed() > timeout {
            return fail_and_log("timed out waiting for browser callback".into());
        }
        match listener.accept() {
            Ok((stream, addr)) => {
                // MD-14: only loopback should be hitting this listener.
                // Reject any non-loopback peer (DNS-rebinding hardening).
                if !addr.ip().is_loopback() {
                    stream
                        .shutdown(std::net::Shutdown::Both)
                        .ignore("rejected peer; the connection is abandoned either way");
                    continue;
                }
                // An idle speculative connection (a browser preconnect) yields
                // `None`: keep accepting until the real redirect or the
                // overall deadline, rather than failing the whole flow.
                let step = handle_callback(
                    stream,
                    &cancel,
                    ACCEPTED_STREAM_IO_TIMEOUT,
                    |code, state, may_persist| {
                        let Some(verifier) = verifier.take() else {
                            return Err(DigiKeyAuthError::Request(
                                "the PKCE verifier was already used".into(),
                            ));
                        };
                        auth.exchange_code_guarded(code, verifier, state, &csrf_token, may_persist)
                    },
                );
                if let Some(outcome) = step {
                    return outcome;
                }
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(e) => {
                return fail_and_log(format!("listener accept: {e}"));
            }
        }
    }
}

/// How long the accepted callback connection may stall a read or write.
/// A browser sends the request line immediately; a peer that connects and
/// says nothing must not hold the flow (and the Cancel button) hostage.
/// Kept short: the settings panel's busy flag blocks Connect for this long
/// after a silent peer.
const ACCEPTED_STREAM_IO_TIMEOUT: Duration = Duration::from_secs(2);

/// Make the accepted connection safe to read synchronously.
///
/// The listener is non-blocking so the accept loop can poll for
/// cancellation, and on Windows (and the BSDs) an accepted socket inherits
/// that mode — a bare `read` then fails with `WouldBlock` before the
/// browser's bytes arrive. Reset it to blocking and bound every I/O call.
fn prepare_accepted_stream(
    stream: &std::net::TcpStream,
    io_timeout: Duration,
) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(io_timeout))?;
    stream.set_write_timeout(Some(io_timeout))?;
    Ok(())
}

/// The user cancelled: nothing was stored. Reported to the Messages panel
/// so a cancelled attempt is visible rather than silent.
fn cancelled_outcome() -> Outcome {
    tracing::info!(
        target: "signex::distributor",
        distributor = "digikey",
        "the DigiKey OAuth flow was cancelled; no token was stored"
    );
    Outcome::Cancelled
}

/// Delete the DigiKey refresh token from the OS credential store. Blocking:
/// call it from `spawn_blocking`, never from `update`.
pub fn remove_stored_token() -> Result<(), String> {
    signex_library::distributors::digikey::DigiKeyAuth::remove_stored_refresh_token()
        .map_err(|e| e.to_string())
}

/// Report the outcome of removing a token a cancelled flow stored anyway.
///
/// Cancel can land after the persist veto granted and the refresh token was
/// written; the flow's `Connected` result is then dropped as stale, and
/// nothing else would ever show the user that a valid token sits in the
/// keyring. Best effort: a failed delete is reported with the manual remedy.
/// Neither the token nor the authorization code is ever logged.
pub fn log_stale_token_removal(result: Result<(), String>) {
    match result {
        Ok(()) => tracing::info!(
            target: "signex::distributor",
            distributor = "digikey",
            "a DigiKey token stored by a cancelled OAuth flow was removed"
        ),
        Err(error) => tracing::warn!(
            target: "signex::distributor",
            distributor = "digikey",
            error = %error,
            "a cancelled DigiKey OAuth flow stored a token and it could not be removed; \
             remove the `signex-distributor-digikey` / `refresh` entry from the OS credential store"
        ),
    }
}

/// Build an [`Outcome::Failed`] and put the reason in the Messages panel.
fn fail_and_log(reason: String) -> Outcome {
    tracing::error!(
        target: "signex::distributor",
        distributor = "digikey",
        reason = %reason,
        "the DigiKey OAuth flow failed, so the account was not connected"
    );
    Outcome::Failed { reason }
}

/// Serve one accepted callback connection and finish the handshake.
///
/// `exchange(code, state, may_persist)` performs the token exchange; it is
/// injected so tests can drive this without a network. Cancellation is
/// re-checked after accept, before the exchange, and (through
/// `may_persist`) immediately before the token is stored.
///
/// Returns `None` when the connection sent nothing — it timed out after
/// `io_timeout` or the peer closed it, as a browser preconnect does — so the caller
/// keeps accepting instead of ending the flow.
fn handle_callback<E>(
    mut stream: std::net::TcpStream,
    cancel: &CancelHandle,
    io_timeout: Duration,
    exchange: E,
) -> Option<Outcome>
where
    E: FnOnce(&str, &str, &dyn Fn() -> bool) -> Result<String, DigiKeyAuthError>,
{
    if let Err(e) = prepare_accepted_stream(&stream, io_timeout) {
        return Some(fail_and_log(format!(
            "could not configure the callback connection: {e}"
        )));
    }
    if cancel.is_cancelled() {
        stream
            .shutdown(std::net::Shutdown::Both)
            .ignore("cancelled; the connection is abandoned either way");
        return Some(cancelled_outcome());
    }

    let req_line = read_first_line(&mut stream);
    if is_idle_read(&req_line) {
        stream
            .shutdown(std::net::Shutdown::Both)
            .ignore("idle or closed speculative connection; abandoned either way");
        tracing::debug!(
            target: "signex::distributor",
            distributor = "digikey",
            "ignored an idle connection on the OAuth callback listener; still waiting for the redirect"
        );
        return None;
    }
    if is_stray_request(&req_line) {
        // A favicon probe or another local client: not the redirect, so
        // answer 404 and keep waiting rather than end the flow.
        std::io::Write::write_all(
            &mut stream,
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        )
        .ignore("courtesy 404; the stray peer may already be gone");
        stream
            .shutdown(std::net::Shutdown::Both)
            .ignore("stray connection; abandoned either way");
        return None;
    }
    Some(finish_callback(stream, cancel, req_line, exchange))
}

/// True when a request line was read but its path is not `/callback`. A read
/// error is not stray: it stays on the failure path so it is reported.
fn is_stray_request(req_line: &Result<String, std::io::Error>) -> bool {
    match req_line {
        Ok(line) => {
            let target = line.split_whitespace().nth(1).unwrap_or("");
            target.split('?').next() != Some("/callback")
        }
        Err(_) => false,
    }
}

/// A read that produced no request: it timed out (Unix reports `WouldBlock`,
/// Windows `TimedOut`) or the peer closed without sending a byte (`Ok(0)`,
/// which `read_first_line` yields as an empty line). Both are a speculative
/// preconnect, not a redirect.
fn is_idle_read(req_line: &Result<String, std::io::Error>) -> bool {
    match req_line {
        Ok(line) => line.is_empty(),
        Err(e) => matches!(
            e.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ),
    }
}

fn finish_callback<E>(
    mut stream: std::net::TcpStream,
    cancel: &CancelHandle,
    req_line: Result<String, std::io::Error>,
    exchange: E,
) -> Outcome
where
    E: FnOnce(&str, &str, &dyn Fn() -> bool) -> Result<String, DigiKeyAuthError>,
{
    let outcome = callback_outcome(cancel, req_line, exchange);
    // Courtesy page only, written after the exchange so the browser never
    // claims "connected" for a failed or cancelled handshake. `req_line` was
    // consumed above, so the outcome does not depend on this reaching the
    // browser.
    let response_body = match outcome {
        Outcome::Connected { .. } => {
            "<html><body><p>Signex: DigiKey connected. You can close this window.</p></body></html>"
        }
        _ => {
            "<html><body><p>Signex: DigiKey was not connected. Return to Signex for details.</p></body></html>"
        }
    };
    std::io::Write::write_all(&mut stream, ok_response(response_body).as_bytes())
        .ignore("confirmation page only; the outcome is already decided");
    stream
        .shutdown(std::net::Shutdown::Both)
        .ignore("connection is finished with; a failed shutdown means the peer closed first");
    outcome
}

fn callback_outcome<E>(
    cancel: &CancelHandle,
    req_line: Result<String, std::io::Error>,
    exchange: E,
) -> Outcome
where
    E: FnOnce(&str, &str, &dyn Fn() -> bool) -> Result<String, DigiKeyAuthError>,
{
    let (code, returned_state) = match callback_params(req_line) {
        Ok(t) => t,
        Err(outcome) => return outcome,
    };

    if cancel.is_cancelled() {
        return cancelled_outcome();
    }

    match exchange(&code, &returned_state, &|| !cancel.is_cancelled()) {
        Ok(_access_token) => Outcome::Connected {
            // No identity claim is returned by the token endpoint in
            // DigiKey's spec; the panel labels the connection generically
            // until a follow-up `/me` call lands.
            account_label: "DigiKey".to_string(),
        },
        Err(DigiKeyAuthError::Cancelled) => cancelled_outcome(),
        Err(e) => failure_from(e),
    }
}

fn failure_from(e: DigiKeyAuthError) -> Outcome {
    fail_and_log(e.to_string())
}

/// Read the first line of the HTTP request — only the line we need
/// to extract `?code=&state=` from. A plain polled `TcpListener` is
/// used instead of an HTTP server crate: one request, and it blends
/// with the polling/cancel pattern.
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
    if let Some(error) = callback_error(&line) {
        tracing::error!(
            target: "signex::distributor",
            distributor = "digikey",
            error = %error,
            "DigiKey redirected back with an authorization error, so the account was not connected"
        );
        return Err(Outcome::Failed {
            reason: if error == "access_denied" {
                "authorization was denied in the browser".into()
            } else {
                format!("DigiKey returned an authorization error: {error}")
            },
        });
    }
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

/// The `error` query parameter of the callback request line, if any
/// (RFC 6749 §4.1.2.1, e.g. `access_denied` when the user declines).
fn callback_error(req_line: &str) -> Option<String> {
    let query = req_line.split_whitespace().nth(1)?.split_once('?')?.1;
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(k, _)| *k == "error")
        .map(|(_, v)| url_decode(v))
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
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
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

/// Courtesy HTML page shown in the browser after the redirect.
fn ok_response(body: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_response_has_one_header_terminator() {
        let r = ok_response("<p>x</p>");
        assert_eq!(r.matches("\r\n\r\n").count(), 1);
        assert!(r.starts_with("HTTP/1.1 200 OK\r\nContent-Type: text/html"));
        assert!(r.ends_with("\r\n\r\n<p>x</p>"));
    }

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
    fn stray_connections_are_not_the_callback() {
        let ok = |l: &str| Ok::<String, std::io::Error>(l.to_string());
        assert!(!is_stray_request(&ok(
            "GET /callback?code=a&state=b HTTP/1.1"
        )));
        assert!(is_stray_request(&ok("GET /favicon.ico HTTP/1.1")));
        let reset = Err(std::io::Error::from(std::io::ErrorKind::ConnectionReset));
        assert!(!is_stray_request(&reset));
    }

    #[test]
    fn parse_callback_skips_valueless_params() {
        let line = "GET /callback?flag&code=abc&state=xyz HTTP/1.1";
        assert_eq!(parse_callback(line), Some(("abc".into(), "xyz".into())));
    }

    #[test]
    fn denied_redirect_is_reported_as_denial_not_missing_pair() {
        let line = "GET /callback?error=access_denied&state=xyz HTTP/1.1";
        assert_eq!(callback_error(line).as_deref(), Some("access_denied"));
        let Err(Outcome::Failed { reason }) = callback_params(Ok(line.to_string())) else {
            panic!("expected failure");
        };
        assert!(reason.contains("denied"), "{reason}");
        assert_eq!(
            callback_error("GET /callback?code=a&state=b HTTP/1.1"),
            None
        );
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

    const GOOD_REQUEST: &str = "GET /callback?code=the-code&state=the-state HTTP/1.1\r\n\r\n";

    /// Accept one loopback connection whose peer has already sent
    /// `request`, the way the real listener sees a browser redirect.
    fn accepted_stream_with(request: &str) -> std::net::TcpStream {
        use std::io::Write;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let mut client =
            std::net::TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
        client.write_all(request.as_bytes()).expect("send request");
        let (stream, _) = listener.accept().expect("accept");
        stream
    }

    #[test]
    fn callback_listener_binds_loopback_only() {
        let (listener, url) = bind_callback_listener().expect("bind");
        let ip = listener.local_addr().expect("addr").ip();
        assert!(ip.is_loopback(), "bound to non-loopback {ip}");
        assert!(!ip.is_unspecified(), "bound to the wildcard address {ip}");
        assert!(url.starts_with("http://127.0.0.1:"), "redirect uri {url}");
    }

    /// #834: an accepted socket inherits the listener's non-blocking mode
    /// on Windows/BSD, so the first read raced the browser and failed with
    /// `WouldBlock`. Force the inherited state, then check the prepared
    /// stream waits for a late sender instead of erroring.
    #[test]
    fn accepted_stream_is_blocking_with_a_bounded_read_timeout() {
        use std::io::{Read, Write};
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let mut client =
            std::net::TcpStream::connect(listener.local_addr().expect("addr")).expect("connect");
        let (mut stream, _) = listener.accept().expect("accept");
        stream.set_nonblocking(true).expect("simulate inheritance");

        prepare_accepted_stream(&stream, ACCEPTED_STREAM_IO_TIMEOUT).expect("prepare");

        assert_eq!(
            stream.read_timeout().expect("read timeout"),
            Some(ACCEPTED_STREAM_IO_TIMEOUT)
        );
        let writer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            client.write_all(b"hello").expect("late write");
            client
        });
        let mut buf = [0u8; 5];
        stream
            .read_exact(&mut buf)
            .expect("a blocking read waits for the late bytes");
        assert_eq!(&buf, b"hello");
        drop(writer.join().expect("writer"));
    }

    #[test]
    fn cancel_after_accept_never_reaches_the_exchange() {
        let cancel = CancelHandle::new();
        cancel.cancel();
        let stream = accepted_stream_with(GOOD_REQUEST);

        let outcome = handle_callback(stream, &cancel, ACCEPTED_STREAM_IO_TIMEOUT, |_, _, _| {
            panic!("a cancelled flow must not exchange the code")
        })
        .expect("a cancel is a final outcome");

        assert!(matches!(outcome, Outcome::Cancelled), "got {outcome:?}");
    }

    #[test]
    fn cancel_during_the_exchange_persists_nothing() {
        let cancel = CancelHandle::new();
        let stream = accepted_stream_with(GOOD_REQUEST);
        let persisted = std::cell::Cell::new(false);

        let outcome = handle_callback(
            stream,
            &cancel,
            ACCEPTED_STREAM_IO_TIMEOUT,
            |code, state, may_persist| {
                assert_eq!((code, state), ("the-code", "the-state"));
                // The user presses Cancel while the token request is in flight.
                cancel.cancel();
                if !may_persist() {
                    return Err(DigiKeyAuthError::Cancelled);
                }
                persisted.set(true);
                Ok("access".into())
            },
        )
        .expect("a cancel is a final outcome");

        assert!(matches!(outcome, Outcome::Cancelled), "got {outcome:?}");
        assert!(!persisted.get(), "a cancelled flow stored a token");
    }

    /// Positive control: without a cancel the same path connects and the
    /// persist veto grants.
    #[test]
    fn uncancelled_callback_connects_and_may_persist() {
        let cancel = CancelHandle::new();
        let stream = accepted_stream_with(GOOD_REQUEST);

        let outcome = handle_callback(
            stream,
            &cancel,
            ACCEPTED_STREAM_IO_TIMEOUT,
            |_, _, may_persist| {
                assert!(may_persist());
                Ok("access".into())
            },
        )
        .expect("a redirect is a final outcome");

        assert!(
            matches!(outcome, Outcome::Connected { .. }),
            "got {outcome:?}"
        );
    }

    /// #834 review: Cancel lands after `may_persist` granted and the token
    /// was stored. The flow still reports `Connected`; the handler's stale
    /// branch (tested in `dispatch/library/settings_tests.rs`) is what
    /// removes the token.
    #[test]
    fn cancel_after_persist_still_reports_connected_for_the_stale_path() {
        let cancel = CancelHandle::new();
        let stream = accepted_stream_with(GOOD_REQUEST);
        let keyring = std::cell::RefCell::new(None::<String>);

        let outcome = handle_callback(
            stream,
            &cancel,
            ACCEPTED_STREAM_IO_TIMEOUT,
            |_, _, may_persist| {
                assert!(may_persist());
                *keyring.borrow_mut() = Some("refresh-tok".to_string());
                // Cancel arrives right after the write.
                cancel.cancel();
                Ok("access".into())
            },
        )
        .expect("a redirect is a final outcome");
        assert!(matches!(outcome, Outcome::Connected { .. }));
        assert!(keyring.borrow().is_some(), "precondition: token was stored");
    }

    /// A preconnect the browser closed before sending anything reads as
    /// `Ok(0)`; it is idle like a timeout, and the real redirect that
    /// follows still completes the flow.
    #[test]
    fn a_connection_closed_without_a_request_is_skipped_and_the_next_one_connects() {
        let cancel = CancelHandle::new();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let closed_client = std::net::TcpStream::connect(addr).expect("connect");
        let (closed, _) = listener.accept().expect("accept");
        drop(closed_client);

        let closed_step = handle_callback(closed, &cancel, Duration::from_secs(5), |_, _, _| {
            panic!("a closed connection must not exchange")
        });
        assert!(
            closed_step.is_none(),
            "a closed preconnect must not end the flow, got {closed_step:?}"
        );

        let real = accepted_stream_with(GOOD_REQUEST);
        let step = handle_callback(real, &cancel, Duration::from_secs(5), |_, _, _| {
            Ok("access".into())
        });
        assert!(
            matches!(step, Some(Outcome::Connected { .. })),
            "got {step:?}"
        );
    }

    /// A browser preconnect that never sends is skipped, and the real
    /// redirect on the next connection still completes the flow.
    #[test]
    fn an_idle_connection_is_skipped_and_the_next_one_connects() {
        let cancel = CancelHandle::new();
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let _idle_client = std::net::TcpStream::connect(addr).expect("idle connect");
        let (idle, _) = listener.accept().expect("accept idle");

        let idle_step = handle_callback(idle, &cancel, Duration::from_millis(100), |_, _, _| {
            panic!("an idle connection must not exchange")
        });
        assert!(idle_step.is_none(), "idle connection must not end the flow");

        let real = accepted_stream_with(GOOD_REQUEST);
        let step = handle_callback(real, &cancel, Duration::from_secs(5), |_, _, _| {
            Ok("access".into())
        });
        assert!(
            matches!(step, Some(Outcome::Connected { .. })),
            "got {step:?}"
        );
    }

    /// A `GET /favicon.ico` is answered 404 and skipped (#1373); the real
    /// redirect afterwards still completes the flow.
    #[test]
    fn a_stray_path_is_skipped_and_the_redirect_still_connects() {
        let cancel = CancelHandle::new();
        let stray = accepted_stream_with("GET /favicon.ico HTTP/1.1\r\nHost: x\r\n\r\n");
        let step = handle_callback(stray, &cancel, Duration::from_secs(5), |_, _, _| {
            panic!("a stray request must not exchange")
        });
        assert!(step.is_none(), "stray path must not end the flow");

        let real = accepted_stream_with(GOOD_REQUEST);
        let step = handle_callback(real, &cancel, Duration::from_secs(5), |_, _, _| {
            Ok("access".into())
        });
        assert!(
            matches!(step, Some(Outcome::Connected { .. })),
            "got {step:?}"
        );
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
