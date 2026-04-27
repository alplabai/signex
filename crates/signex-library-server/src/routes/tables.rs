//! `/tables` routes — DBLib row model (`v0.9-refactor-2-plan.md` §9, WS-4).
//!
//! Two endpoints under this prefix:
//!
//! * `GET  /tables                 ?library_id=<uuid>`
//!   → `[String]` — distinct table names with at least one row.
//! * `GET  /tables/:name           ?library_id=<uuid>`
//!   → `[ComponentRow]` — every row in `name`, ordered by `internal_pn`.
//!
//! `library_id` rides on the query string the same way it does for the
//! primitive routes (`/symbols` / `/footprints` / `/sims`). Once OIDC lands
//! in v0.9.x the value can be derived from the bearer token claims; until
//! then it stays explicit so the wire contract is symmetric across the
//! refactor.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::Deserialize;
use signex_library::component::ComponentRow;
use uuid::Uuid;

use crate::db::AppState;
use crate::routes::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tables", get(list_tables))
        .route("/tables/:name", get(list_rows_in_table))
}

#[derive(Debug, Deserialize)]
struct LibraryQuery {
    library_id: Uuid,
}

async fn list_tables(
    State(state): State<AppState>,
    Query(q): Query<LibraryQuery>,
) -> Result<Json<Vec<String>>, ApiError> {
    let names = state.list_table_names(q.library_id).await?;
    Ok(Json(names))
}

async fn list_rows_in_table(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<LibraryQuery>,
) -> Result<Json<Vec<ComponentRow>>, ApiError> {
    let rows = state.list_rows_in_table(q.library_id, &name).await?;
    Ok(Json(rows))
}
