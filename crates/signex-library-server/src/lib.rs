//! Signex library DB-flavour server.
//!
//! Exposes a JSON HTTP API over a shared `AppState` (DB pool).
//! Liveness checks (`/health`, `/version`) stay anonymous so process
//! supervisors don't need credentials. Every other route — the
//! `/tables` + `/rows` row tier and the primitive
//! (`/symbols` / `/footprints` / `/sims`) routes — is gated behind a bearer-token
//! check sourced from the `SIGNEX_API_TOKEN` env var (compared in constant time).
//!
//! ## Transport security
//!
//! The server speaks plain HTTP and has no TLS stack. Anything beyond
//! loopback MUST sit behind a TLS-terminating reverse proxy. Behind a proxy
//! the per-IP rate limiter sees the proxy's address, so all clients share
//! one bucket unless the proxy addresses are listed in `SIGNEX_TRUSTED_PROXIES`
//! (see [`with_rate_limit_trusted`]); `SIGNEX_TRUSTED_PROXY_HEADER`
//! (`x-forwarded-for` or `forwarded`) is then REQUIRED and names the one header
//! the proxy overwrites (or appends to) -- the other is ignored.
//!
//! `main` refuses a non-loopback plaintext bind unless `--allow-insecure-bind`
//! / `SIGNEX_ALLOW_INSECURE_BIND=1` is given. The client refuses to send its
//! token over `http://` to a non-loopback host, including via a redirect.
//!
//! ## DBLib row model
//!
//! Components live as rows inside a shared `component_rows` table:
//!
//! ```text
//! GET    /tables                      list table names
//! GET    /tables/:name                list rows in table
//! POST   /tables/:name/rows           insert row
//! GET    /tables/:name/rows/:row_id   read row
//! PUT    /tables/:name/rows/:row_id   replace row
//! DELETE /tables/:name/rows/:row_id   delete row
//! ```
//!
//! ## Authentication (H1)
//!
//! Mutating routes are gated behind a bearer-token check sourced from
//! `SIGNEX_API_TOKEN`. If unset on startup the auth layer is omitted entirely
//! and a `tracing::warn!` fires telling operators they are running
//! unauthenticated — fine for local dev, never for production.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Json, Router,
    extract::Request,
    http::{HeaderValue, StatusCode, header::AUTHORIZATION},
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
};
use serde_json::json;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;

pub mod db;
mod rate_limit_key;
pub mod routes;

pub use db::AppState;
pub use rate_limit_key::{ProxyHeader, TrustedProxyKeyExtractor};

/// Env var that holds the bearer token for the protected routes.
/// Unset → unauthenticated mode (with a startup warning).
pub const API_TOKEN_ENV: &str = "SIGNEX_API_TOKEN";

/// Env var holding the persistent database URL (`postgres://…` or
/// `sqlite://<file>`). Unset → an ephemeral in-memory SQLite that
/// loses every row on restart; the binary only allows that on a
/// loopback bind and logs a prominent warning.
pub const DATABASE_URL_ENV: &str = "SIGNEX_DATABASE_URL";

/// Maximum request body in bytes accepted on protected mutation routes.
/// 1 MiB is generous for component / primitive payloads (typical row JSON
/// is ~5 KiB, primitives ~50 KiB) and bounded enough to stop unbounded
/// allocation if a client (auth'd or not) tries to OOM the server.
pub const MAX_REQUEST_BODY_BYTES: usize = 1 << 20;

/// HI-2: per-IP rate limit for protected mutation routes. 60 req/min/IP
/// is generous for the documented multi-user library workflow (a human
/// rarely exceeds ~10 row edits per minute) and tight enough to stop
/// the unbounded request flood. Read paths (`/tables/*` GET) inherit the same gate; if that
/// proves too tight in practice, split into per-route configs.
const RATE_LIMIT_PER_SECOND: u64 = 1; // i.e. 60 req/min/IP, replenished 1/sec
#[doc(hidden)] // exposed so tests/rate_limit.rs asserts the real contract
pub const RATE_LIMIT_BURST_SIZE: u32 = 30; // accommodate a normal UI burst

/// Router with no shared state — used by the legacy `/health` + `/version`
/// integration tests in `tests/health.rs`.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
}

/// Router wired up with a fresh in-memory SQLite. Production callers should
/// build their own `AppState` and use [`router_with_state`] directly.
pub async fn router_with_in_memory_state() -> anyhow::Result<Router> {
    let state = AppState::new_sqlite_memory().await?;
    state.migrate().await?;
    Ok(router_with_state(state))
}

/// Build the full router around an existing `AppState`.
///
/// HI-1: mutation routes require a `Bearer <token>` matching `SIGNEX_API_TOKEN`.
/// HI-4: every protected route is body-size-capped at [`MAX_REQUEST_BODY_BYTES`].
/// MD-16: a CORS layer is wired so we don't ride the axum-default permissive
///   behaviour if the bind ever lands on a non-loopback interface.
///
/// `/health` and `/version` are always reachable so process supervisors can
/// probe liveness without holding a credential.
pub fn router_with_state(state: AppState) -> Router {
    let liveness = Router::new()
        .route("/health", get(health))
        .route("/version", get(version));

    let mut protected = Router::new()
        .merge(routes::tables::router())
        .merge(routes::rows::router())
        .merge(routes::symbols::router())
        .merge(routes::footprints::router())
        .merge(routes::sims::router())
        // HI-4: cap protected mutation bodies before serde_json buffers
        // the entire payload into memory.
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES));

    match std::env::var(API_TOKEN_ENV) {
        Ok(token) if !token.is_empty() => {
            let token: Arc<str> = Arc::from(token);
            let layer = middleware::from_fn(move |req: Request, next: Next| {
                let token = Arc::clone(&token);
                async move {
                    if bearer_matches(req.headers().get(AUTHORIZATION), &token) {
                        next.run(req).await
                    } else {
                        StatusCode::UNAUTHORIZED.into_response()
                    }
                }
            });
            protected = protected.layer(layer);
        }
        _ => {
            tracing::warn!(
                env = API_TOKEN_ENV,
                "server unauthenticated — set {API_TOKEN_ENV} for production (loopback only)",
            );
        }
    }

    // MD-16: explicit CORS — we own the front-end so allow only signex.dev
    // origins in production. The loopback dev origin is allowed so the
    // local UI can hit the local server during development.
    let cors = CorsLayer::new()
        .allow_origin([
            HeaderValue::from_static("http://127.0.0.1:3535"),
            HeaderValue::from_static("http://localhost:3535"),
            HeaderValue::from_static("https://signex.dev"),
            HeaderValue::from_static("https://www.signex.dev"),
        ])
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    liveness.merge(protected).layer(cors).with_state(state)
}

/// Production hardening — wraps a router built by [`router_with_state`]
/// with the per-IP rate-limit layer (HI-2). Kept separate from the base
/// router so the in-tree integration tests can call routes via
/// `tower::ServiceExt::oneshot` without the governor rejecting them
/// for missing `ConnectInfo`. Production callers MUST also serve via
/// `axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())`
/// so the governor can see peer IPs.
///
/// Also schedules a periodic `retain_recent` sweep on the limiter so
/// dormant IP entries are dropped (parallels [`LOCK_SWEEP_INTERVAL`]).
pub fn with_rate_limit(router: Router) -> Router {
    with_rate_limit_trusted(router, Vec::new(), ProxyHeader::default())
}

/// Like [`with_rate_limit`], but a request whose TCP peer is in
/// `trusted_proxies` is bucketed by the client address from `header` only (the
/// other forwarding header is ignored). Any other peer is bucketed by its own
/// address.
pub fn with_rate_limit_trusted(
    router: Router,
    trusted_proxies: Vec<IpAddr>,
    header: ProxyHeader,
) -> Router {
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(RATE_LIMIT_PER_SECOND)
            .burst_size(RATE_LIMIT_BURST_SIZE)
            .key_extractor(TrustedProxyKeyExtractor::new(trusted_proxies, header))
            .finish()
            .expect("governor config: per_second/burst_size both nonzero"),
    );
    let governor_for_cleanup = Arc::clone(&governor_conf);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        interval.tick().await;
        loop {
            interval.tick().await;
            governor_for_cleanup.limiter().retain_recent();
        }
    });
    let probes = router.clone();
    let limited = router.layer(GovernorLayer {
        config: governor_conf,
    });
    // Probe endpoints bypass the governor so a health check is never answered
    // with 429. They are still served by the same router, so its CORS layer
    // and its own /health and /version handlers apply unchanged.
    Router::new()
        .route_service("/health", probes.clone().into_service())
        .route_service("/version", probes.into_service())
        .fallback_service(limited)
}

/// Constant-time check of an `Authorization: Bearer <token>` header against
/// the configured token. The length is not secret; the bytes are compared
/// without early exit.
fn bearer_matches(header: Option<&HeaderValue>, token: &str) -> bool {
    use subtle::ConstantTimeEq;
    let Some(presented) = header.and_then(|h| h.as_bytes().strip_prefix(b"Bearer ")) else {
        return false;
    };
    presented.ct_eq(token.as_bytes()).into()
}

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn version() -> Json<serde_json::Value> {
    Json(json!({
        "name": "signex-library-server",
        "version": env!("CARGO_PKG_VERSION"),
        "row_schema": signex_library::ROW_SCHEMA,
        // Primitive POST refuses an existing uuid and PUT needs If-Match;
        // a client checks this before any primitive request.
        "primitive_revisions": true,
    }))
}

#[cfg(test)]
mod auth_tests {
    use super::*;

    #[test]
    fn bearer_matches_only_exact_token() {
        let ok = HeaderValue::from_static("Bearer s3cret");
        assert!(bearer_matches(Some(&ok), "s3cret"));
        assert!(!bearer_matches(Some(&ok), "s3cre"));
        assert!(!bearer_matches(
            Some(&HeaderValue::from_static("Bearer s3cre")),
            "s3cret"
        ));
        assert!(!bearer_matches(
            Some(&HeaderValue::from_static("s3cret")),
            "s3cret"
        ));
        assert!(!bearer_matches(
            Some(&HeaderValue::from_static("Basic s3cret")),
            "s3cret"
        ));
        assert!(!bearer_matches(None, "s3cret"));
    }
}
