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
//!
//! POST and PUT also require `x-signex-row-schema: <n>` with `n` at least
//! the server's `ROW_SCHEMA`; an older build would drop row fields it does
//! not know, so a missing, malformed or lower value answers 400 with
//! `code: "row_schema_too_old"`.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::IF_MATCH},
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use signex_library::component::{ComponentRow, ROW_SCHEMA, ROW_SCHEMA_HEADER};
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
    headers: HeaderMap,
    Json(mut row): Json<ComponentRow>,
) -> Result<impl IntoResponse, ApiError> {
    require_row_schema(&headers)?;
    signex_library::validate_row_for_write(&row)
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
    require_row_schema(&headers)?;
    let url_row_id = parse_row_id(&row_id)?;
    // Body row_id must agree with URL row_id — refuse the request rather
    // than silently overwrite a different row.
    if row.row_id != url_row_id.as_uuid() {
        return Err(ApiError::bad_request(format!(
            "row_id mismatch: url={url_row_id}, body={}",
            row.row_id
        )));
    }
    let base = base_revision(&headers, "row")?;
    signex_library::validate_row_for_write(&row)
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let revision = applied(
        state.update_row(q.library_id, &name, &row, base).await?,
        &row_subject(url_row_id, &name),
    )?;
    row.server_revision = Some(revision.unsigned_abs());
    Ok(Json(row))
}

async fn delete_row(
    State(state): State<AppState>,
    Path((name, row_id)): Path<(String, String)>,
    Query(q): Query<LibraryQuery>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let row_id = parse_row_id(&row_id)?;
    let base = base_revision(&headers, "row")?;
    applied(
        state.delete_row(q.library_id, &name, row_id, base).await?,
        &row_subject(row_id, &name),
    )?;
    Ok(StatusCode::NO_CONTENT)
}

/// Refuse a row write from a build older than [`ROW_SCHEMA`].
fn require_row_schema(headers: &HeaderMap) -> Result<(), ApiError> {
    let sent = headers
        .get(ROW_SCHEMA_HEADER)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u32>().ok());
    match sent {
        Some(n) if n >= ROW_SCHEMA => Ok(()),
        _ => Err(ApiError::row_schema_too_old(format!(
            "this server needs row schema {ROW_SCHEMA}: send {ROW_SCHEMA_HEADER}: {ROW_SCHEMA}. \
             Upgrade signex, an older build would drop row fields it does not know"
        ))),
    }
}

/// The revision a PUT/DELETE was based on, from `If-Match: "<revision>"`.
/// `noun` names what is being changed ("row", "symbol", ...) in the refusal.
pub(crate) fn base_revision(headers: &HeaderMap, noun: &str) -> Result<i64, ApiError> {
    let raw = headers.get(IF_MATCH).ok_or_else(|| {
        ApiError::precondition_required(format!(
            "If-Match with the {noun}'s revision is required to change a {noun}"
        ))
    })?;
    raw.to_str()
        .ok()
        .and_then(|v| v.trim().strip_prefix('"')?.strip_suffix('"'))
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "If-Match must be a quoted {noun} revision, e.g. \"3\""
            ))
        })
}

/// The revision a conditional write left, or the error it was refused with.
/// `subject` names the row or primitive in the refusal. Shared by the row
/// and primitive routes.
pub(crate) fn applied(write: RowWrite, subject: &str) -> Result<i64, ApiError> {
    match write {
        RowWrite::Applied { revision } => Ok(revision),
        RowWrite::Stale { current_revision } => Err(ApiError::stale_revision(
            format!("{subject} changed since it was read"),
            current_revision,
        )),
        RowWrite::Missing => Err(ApiError::not_found(subject.to_string())),
    }
}

fn row_subject(row_id: RowId, table: &str) -> String {
    format!("row {row_id} in table {table}")
}

fn parse_row_id(raw: &str) -> Result<RowId, ApiError> {
    raw.parse()
        .map_err(|e: uuid::Error| ApiError::bad_request(format!("row_id: {e}")))
}
