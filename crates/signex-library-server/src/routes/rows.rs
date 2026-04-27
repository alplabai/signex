//! `/tables/:name/rows` routes — per-row CRUD over `ComponentRow` (WS-4).
//!
//! All routes carry `?library_id=<uuid>` to scope into one library inside
//! the shared `component_rows` table. JSON body shape is `ComponentRow`
//! (already serializable per WS-1) — no envelope wrapper.
//!
//! ```text
//! POST   /tables/:name/rows             insert row, body=ComponentRow
//! GET    /tables/:name/rows/:row_id     fetch one row
//! PUT    /tables/:name/rows/:row_id     replace, body=ComponentRow
//! DELETE /tables/:name/rows/:row_id     delete, 204 on success
//! ```
//!
//! `:row_id` is parsed as a [`RowId`] — UUIDv7 stringified per the WS-1
//! identity newtype.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use signex_library::component::ComponentRow;
use signex_library::identity::RowId;
use uuid::Uuid;

use crate::db::AppState;
use crate::routes::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tables/:name/rows", post(create_row))
        .route(
            "/tables/:name/rows/:row_id",
            get(get_row).put(update_row).delete(delete_row),
        )
}

#[derive(Debug, Deserialize)]
struct LibraryQuery {
    library_id: Uuid,
}

async fn create_row(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(q): Query<LibraryQuery>,
    Json(row): Json<ComponentRow>,
) -> Result<impl IntoResponse, ApiError> {
    state.insert_row(q.library_id, &name, &row).await?;
    Ok((StatusCode::CREATED, Json(row)))
}

async fn get_row(
    State(state): State<AppState>,
    Path((name, row_id)): Path<(String, String)>,
    Query(q): Query<LibraryQuery>,
) -> Result<Json<ComponentRow>, ApiError> {
    let row_id = parse_row_id(&row_id)?;
    state
        .fetch_row(q.library_id, &name, row_id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("row {row_id} in table {name}")))
}

async fn update_row(
    State(state): State<AppState>,
    Path((name, row_id)): Path<(String, String)>,
    Query(q): Query<LibraryQuery>,
    Json(row): Json<ComponentRow>,
) -> Result<Json<ComponentRow>, ApiError> {
    let url_row_id = parse_row_id(&row_id)?;
    // Body row_id must agree with URL row_id — refuse the request rather
    // than silently overwrite a different row.
    if row.row_id != url_row_id.as_uuid() {
        return Err(ApiError::bad_request(format!(
            "row_id mismatch: url={url_row_id}, body={}",
            row.row_id
        )));
    }
    let updated = state.update_row(q.library_id, &name, &row).await?;
    if !updated {
        return Err(ApiError::not_found(format!(
            "row {url_row_id} in table {name}"
        )));
    }
    Ok(Json(row))
}

async fn delete_row(
    State(state): State<AppState>,
    Path((name, row_id)): Path<(String, String)>,
    Query(q): Query<LibraryQuery>,
) -> Result<StatusCode, ApiError> {
    let row_id = parse_row_id(&row_id)?;
    let deleted = state.delete_row(q.library_id, &name, row_id).await?;
    if !deleted {
        return Err(ApiError::not_found(format!("row {row_id} in table {name}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

fn parse_row_id(raw: &str) -> Result<RowId, ApiError> {
    raw.parse()
        .map_err(|e: uuid::Error| ApiError::bad_request(format!("row_id: {e}")))
}
