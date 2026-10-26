//! `/tables/:name/rows` routes — per-row CRUD over `ComponentRow`.
//!
//! All routes carry `?library_id=<uuid>` to scope into one library
//! inside the shared `component_rows` table. JSON body shape is
//! `ComponentRow` directly — no envelope wrapper.
//!
//! ```text
//! POST   /tables/:name/rows             insert row, body=ComponentRow
//! GET    /tables/:name/rows/:row_id     fetch one row
//! PUT    /tables/:name/rows/:row_id     replace, body=ComponentRow
//! DELETE /tables/:name/rows/:row_id     delete, 204 on success
//! ```
//!
//! `:row_id` is parsed as a [`RowId`] — a UUIDv7 newtype.
//!
//! Rows carry `server_revision`. PUT and DELETE require
//! `If-Match: "<revision>"` naming the revision the client read: missing
//! answers 428, a row that has moved on answers 409 with
//! `{"error", "current_revision"}`. A successful PUT returns the row at its
//! new revision.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::IF_MATCH},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use signex_library::component::ComponentRow;
use signex_library::identity::RowId;
use uuid::Uuid;

use crate::db::{AppState, RowWrite};
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
    Json(mut row): Json<ComponentRow>,
) -> Result<impl IntoResponse, ApiError> {
    signex_library::ensure_row_params_finite(&row)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let inserted = state.insert_row(q.library_id, &name, &row).await?;
    if !inserted {
        // POST is create-only; an existing row must not be silently
        // overwritten. Clients replace via PUT.
        return Err(ApiError::conflict(format!(
            "row {} already exists in table {name}",
            row.row_id
        )));
    }
    row.server_revision = Some(1);
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
    headers: HeaderMap,
    Json(mut row): Json<ComponentRow>,
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
    let base = base_revision(&headers)?;
    signex_library::ensure_row_params_finite(&row)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    // A client built before labelled documents (#1231) sends a row without
    // them; keep the stored ones rather than erase them on its write.
    if row_schema(&headers) < signex_library::ROW_SCHEMA
        && let Some(stored) = state.fetch_row(q.library_id, &name, url_row_id).await?
    {
        row.keep_documents_from(&stored)
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
    }
    let revision = applied(
        state.update_row(q.library_id, &name, &row, base).await?,
        url_row_id,
        &name,
    )?;
    row.server_revision = Some(revision);
    Ok(Json(row))
}

async fn delete_row(
    State(state): State<AppState>,
    Path((name, row_id)): Path<(String, String)>,
    Query(q): Query<LibraryQuery>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let row_id = parse_row_id(&row_id)?;
    let base = base_revision(&headers)?;
    applied(
        state.delete_row(q.library_id, &name, row_id, base).await?,
        row_id,
        &name,
    )?;
    Ok(StatusCode::NO_CONTENT)
}

/// The row shape the client declares in [`signex_library::ROW_SCHEMA_HEADER`];
/// 1 when it sends none (a client built before the header).
fn row_schema(headers: &HeaderMap) -> u32 {
    headers
        .get(signex_library::ROW_SCHEMA_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(1)
}

/// The revision a PUT/DELETE was based on, from `If-Match: "<revision>"`.
fn base_revision(headers: &HeaderMap) -> Result<i64, ApiError> {
    let raw = headers.get(IF_MATCH).ok_or_else(|| {
        ApiError::precondition_required(
            "If-Match with the row's revision is required to change a row",
        )
    })?;
    raw.to_str()
        .ok()
        .and_then(|v| v.trim().strip_prefix('"')?.strip_suffix('"'))
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .ok_or_else(|| ApiError::bad_request("If-Match must be a quoted row revision, e.g. \"3\""))
}

/// The revision a conditional write left, or the error it was refused with.
fn applied(write: RowWrite, row_id: RowId, table: &str) -> Result<u64, ApiError> {
    match write {
        RowWrite::Applied { revision } => Ok(revision.unsigned_abs()),
        RowWrite::Stale { current_revision } => Err(ApiError::stale_revision(
            format!("row {row_id} in table {table} changed since it was read"),
            current_revision,
        )),
        RowWrite::Missing => Err(ApiError::not_found(format!(
            "row {row_id} in table {table}"
        ))),
    }
}

fn parse_row_id(raw: &str) -> Result<RowId, ApiError> {
    raw.parse()
        .map_err(|e: uuid::Error| ApiError::bad_request(format!("row_id: {e}")))
}
