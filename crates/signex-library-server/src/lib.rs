//! Signex library DB-flavour server.
//!
//! Exposes a JSON HTTP API over a shared `AppState` (DB pool + lock manager).
//! Liveness checks (`/health`, `/version`) stay anonymous so process
//! supervisors don't need credentials. Every other route — the
//! `/tables` + `/rows` row tier, the primitive
//! (`/symbols` / `/footprints` / `/sims`) routes, and the advisory
//! `/rows/:row_id/locks` endpoint — is gated behind a bearer-token
//! check sourced from the `SIGNEX_API_TOKEN` env var.
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

use std::sync::Arc;
use std::time::Duration;

use axum::{Json, Router, http::HeaderValue, routing::get};
use serde_json::json;
use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder};
use tower_http::cors::CorsLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::validate_request::ValidateRequestHeaderLayer;

pub mod db;
pub mod locks;
pub mod routes;

pub use db::AppState;

/// Env var that holds the bearer token for the protected routes.
/// Unset → unauthenticated mode (with a startup warning).
pub const API_TOKEN_ENV: &str = "SIGNEX_API_TOKEN";

/// Maximum request body in bytes accepted on protected mutation routes.
/// 1 MiB is generous for component / primitive payloads (typical row JSON
/// is ~5 KiB, primitives ~50 KiB) and bounded enough to stop unbounded
/// allocation if a client (auth'd or not) tries to OOM the server.
pub const MAX_REQUEST_BODY_BYTES: usize = 1 << 20;

/// HI-2: per-IP rate limit for protected mutation routes. 60 req/min/IP
/// is generous for the documented multi-user library workflow (a human
/// rarely exceeds ~10 row edits per minute) and tight enough to stop
/// the unbounded lock-acquire flood that grew the in-memory `LockManager`
/// map. Read paths (`/tables/*` GET) inherit the same gate; if that
/// proves too tight in practice, split into per-route configs.
const RATE_LIMIT_PER_SECOND: u64 = 1; // i.e. 60 req/min/IP, replenished 1/sec
const RATE_LIMIT_BURST_SIZE: u32 = 30; // accommodate a normal UI burst

/// How often to drop expired entries from the in-memory `LockManager`.
const LOCK_SWEEP_INTERVAL: Duration = Duration::from_secs(5 * 60);

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
/// HI-3: the `LockManager.sweep_expired` task is spawned here so expired
///   locks are evicted from memory every [`LOCK_SWEEP_INTERVAL`].
/// HI-4: every protected route is body-size-capped at [`MAX_REQUEST_BODY_BYTES`].
/// MD-16: a CORS layer is wired so we don't ride the axum-default permissive
///   behaviour if the bind ever lands on a non-loopback interface.
///
/// `/health` and `/version` are always reachable so process supervisors can
/// probe liveness without holding a credential.
pub fn router_with_state(state: AppState) -> Router {
    // HI-3: schedule periodic sweep of expired lock entries. Without this
    // the in-memory `LockManager` map grows monotonically with every
    // unique row id ever locked.
    let locks_handle: Arc<crate::locks::LockManager> = state.locks_arc();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(LOCK_SWEEP_INTERVAL);
        // Skip the immediate-first-tick so we don't sweep during startup.
        tick.tick().await;
        loop {
            tick.tick().await;
            locks_handle.sweep_expired();
        }
    });

    let liveness = Router::new()
        .route("/health", get(health))
        .route("/version", get(version));

    let mut protected = Router::new()
        .merge(routes::tables::router())
        .merge(routes::rows::router())
        .merge(routes::locks::router())
        .merge(routes::symbols::router())
        .merge(routes::footprints::router())
        .merge(routes::sims::router())
        // HI-4: cap protected mutation bodies before serde_json buffers
        // the entire payload into memory.
        .layer(RequestBodyLimitLayer::new(MAX_REQUEST_BODY_BYTES));

    match std::env::var(API_TOKEN_ENV) {
        Ok(token) if !token.is_empty() => {
            // tower-http 0.6 marks `bearer` as "too basic" but it's the
            // documented escape hatch for env-var-driven static tokens.
            // Once OIDC lands we'll replace it with a custom validator.
            #[allow(deprecated)]
            let layer = ValidateRequestHeaderLayer::bearer(&token);
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
    let governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(RATE_LIMIT_PER_SECOND)
            .burst_size(RATE_LIMIT_BURST_SIZE)
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
    router.layer(GovernorLayer {
        config: governor_conf,
    })
}

pub async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

pub async fn version() -> Json<serde_json::Value> {
    Json(json!({
        "name": "signex-library-server",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
