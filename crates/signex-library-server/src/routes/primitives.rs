//! Shared plumbing for the `/symbols`, `/footprints` and `/sims` routes.
//!
//! Same concurrency contract as the row tier (`rows.rs`): POST is
//! create-only (409 on an existing `(library_id, uuid)`), PUT replaces and
//! needs `If-Match: "<revision>"` (428 missing, 409 stale with
//! `current_revision`). GET and the write replies carry the revision as an
//! `ETag: "<revision>"` header, the value a client sends back as `If-Match`.

use axum::Json;
use axum::http::{HeaderName, StatusCode, header::ETAG};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use uuid::Uuid;

use crate::db::RowWrite;
use crate::routes::error::ApiError;

/// `body` with its revision as an `ETag` header.
pub(crate) fn with_etag<T: Serialize>(status: StatusCode, revision: i64, body: T) -> Response {
    let tag: [(HeaderName, String); 1] = [(ETAG, format!("\"{revision}\""))];
    (status, tag, Json(body)).into_response()
}

/// `409` for a create that hit an existing primitive.
pub(crate) fn already_exists(kind: &str, library_id: Uuid, uuid: Uuid) -> ApiError {
    // POST is create-only; replacing is PUT with the revision that was read.
    ApiError::conflict(format!(
        "{kind} {library_id}/{uuid} already exists; replace it with PUT and If-Match"
    ))
}

/// The revision a conditional write left, or the error it was refused with.
pub(crate) fn applied(
    write: RowWrite,
    kind: &str,
    library_id: Uuid,
    uuid: Uuid,
) -> Result<i64, ApiError> {
    match write {
        RowWrite::Applied { revision } => Ok(revision),
        RowWrite::Stale { current_revision } => Err(ApiError::stale_revision(
            format!("{kind} {library_id}/{uuid} changed since it was read"),
            current_revision,
        )),
        RowWrite::Missing => Err(ApiError::not_found(format!("{kind} {library_id}/{uuid}"))),
    }
}

/// Refuse a PUT whose body names a different primitive than the URL.
pub(crate) fn require_same_uuid(url: Uuid, body: Uuid) -> Result<(), ApiError> {
    if url == body {
        return Ok(());
    }
    Err(ApiError::bad_request(format!(
        "uuid mismatch: url={url}, body={body}"
    )))
}
