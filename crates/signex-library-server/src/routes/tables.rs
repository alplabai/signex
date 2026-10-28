//! `/tables` routes — DBLib row model.
//!
//! Two endpoints under this prefix:
//!
//! * `GET  /tables                 ?library_id=<uuid>`
//!   → `[String]` — distinct table names with at least one row.
//! * `GET  /tables/:name           ?library_id=<uuid>&limit=&offset=`
//!   → `[ComponentRow]` — rows in `name`, ordered by `internal_pn`, one
//!   page at a time: `limit` (default 500, max 1000) and `offset`. A page
//!   shorter than `limit` is the last. `/symbols`, `/footprints` and
//!   `/sims` page the same way.
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

use crate::db::{AppState, Page};
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

#[derive(Debug, Deserialize)]
struct RowsQuery {
    library_id: Uuid,
    /// Page size; defaults to and is capped by the server.
    limit: Option<i64>,
    offset: Option<i64>,
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
    Query(q): Query<RowsQuery>,
) -> Result<Json<Vec<ComponentRow>>, ApiError> {
    let rows = state
        .list_rows_in_table(q.library_id, &name, Page::new(q.limit, q.offset))
        .await?;
    Ok(Json(rows))
}
