//! Signex library DB-flavour server.
//!
//! Exposes a JSON HTTP API over a shared `AppState` (DB pool + lock manager).
//! The original `/health` and `/version` routes from Phase 0 are kept for
//! liveness checks; WS-B layers `/components`, `/revisions`, and `/locks` on
//! top.

use axum::{Json, Router, routing::get};
use serde_json::json;

pub mod db;
pub mod git_export;
pub mod locks;
pub mod routes;

pub use db::AppState;

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
pub fn router_with_state(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .merge(routes::components::router())
        .merge(routes::revisions::router())
        .merge(routes::locks::router())
        .with_state(state)
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
