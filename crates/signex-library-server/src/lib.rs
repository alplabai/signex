//! Signex library DB-flavour server.
//!
//! Exposes a JSON HTTP API over a shared `AppState` (DB pool + lock manager).
//! Liveness checks (`/health`, `/version`) stay anonymous so process
//! supervisors don't need credentials. Every other route — the WS-4
//! `/tables` + `/rows` row tier, the WS-D primitive routes, and the
//! advisory `/rows/:row_id/locks` endpoint — is gated behind a bearer
//! token check sourced from the `SIGNEX_API_TOKEN` env var.
//!
//! ## v0.9-refactor-2 (DBLib row model)
//!
//! Per `v0.9-refactor-2-plan.md` §9, components live as rows inside a
//! shared `component_rows` table. The legacy `/components` +
//! `/components/:uuid/revisions` family is gone; in its place:
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

use axum::{Json, Router, routing::get};
use serde_json::json;
use tower_http::validate_request::ValidateRequestHeaderLayer;

pub mod db;
pub mod locks;
pub mod routes;

pub use db::AppState;

/// Env var that holds the bearer token for the protected routes.
/// Unset → unauthenticated mode (with a startup warning).
pub const API_TOKEN_ENV: &str = "SIGNEX_API_TOKEN";

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
/// H1: routes that read/write component data require a `Bearer <token>`
/// auth header matching `SIGNEX_API_TOKEN`. `/health` and `/version` are
/// always reachable so process supervisors can probe liveness without
/// holding a credential.
pub fn router_with_state(state: AppState) -> Router {
    let liveness = Router::new()
        .route("/health", get(health))
        .route("/version", get(version));

    let mut protected = Router::new()
        .merge(routes::tables::router())
        .merge(routes::rows::router())
        .merge(routes::locks::router())
        .merge(routes::symbols::router())
        .merge(routes::footprints::router())
        .merge(routes::sims::router());

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
                "server unauthenticated — set {API_TOKEN_ENV} for production",
            );
        }
    }

    liveness.merge(protected).with_state(state)
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
