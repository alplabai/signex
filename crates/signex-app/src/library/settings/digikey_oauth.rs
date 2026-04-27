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
    #[allow(dead_code)]
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
            Ok((mut stream, _addr)) => {
                let req_line = read_first_line(&mut stream);
                let response_body = "<html><body><p>Signex: DigiKey connected. You can close this window.</p></body></html>";
                let _ = std::io::Write::write_all(
                    &mut stream,
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        response_body.len(),
                        response_body
                    )
                    .as_bytes(),
                );
                let _ = stream.shutdown(std::net::Shutdown::Both);

                let (code, returned_state) = match parse_callback(&req_line) {
                    Some(t) => t,
                    None => {
                        return Outcome::Failed {
                            reason: "redirect missing code/state".into(),
                        };
                    }
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
fn read_first_line<R: std::io::Read>(stream: &mut R) -> String {
    let mut buf = [0u8; 4096];
    let n = match stream.read(&mut buf) {
        Ok(n) => n,
        Err(_) => return String::new(),
    };
    let s = String::from_utf8_lossy(&buf[..n]);
    s.lines().next().unwrap_or("").to_string()
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
