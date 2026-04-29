//! `/rows/:row_id/locks` — advisory locking over the row tier.
//!
//! Locks key off `RowId`. The caller identifies itself with the
//! `x-signex-holder` header and the body picks the field-set.
//!
//! ```json
//! { "field_set": "Symbol" }
//! ```

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use serde::{Deserialize, Serialize};
use signex_library::adapter::FieldSet;
use signex_library::identity::RowId;

use crate::db::AppState;
use crate::locks::LockErrorKind;
use crate::routes::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/rows/:row_id/locks",
        post(acquire_lock).delete(release_lock),
    )
}

#[derive(Debug, Deserialize)]
struct LockBody {
    field_set: FieldSetWire,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "PascalCase")]
enum FieldSetWire {
    Symbol,
    Footprint,
    Model3d,
    SharedParams,
    SharedSupplyChain,
    SharedSimulation,
    Lifecycle,
}

impl From<FieldSetWire> for FieldSet {
    fn from(value: FieldSetWire) -> Self {
        match value {
            FieldSetWire::Symbol => FieldSet::Symbol,
            FieldSetWire::Footprint => FieldSet::Footprint,
            FieldSetWire::Model3d => FieldSet::Model3d,
            FieldSetWire::SharedParams => FieldSet::SharedParams,
            FieldSetWire::SharedSupplyChain => FieldSet::SharedSupplyChain,
            FieldSetWire::SharedSimulation => FieldSet::SharedSimulation,
            FieldSetWire::Lifecycle => FieldSet::Lifecycle,
        }
    }
}

fn holder_from(headers: &HeaderMap) -> Result<String, ApiError> {
    headers
        .get("x-signex-holder")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::bad_request("missing x-signex-holder header"))
}

async fn acquire_lock(
    State(state): State<AppState>,
    Path(row_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<LockBody>,
) -> Result<impl IntoResponse, ApiError> {
    let row_id: RowId = row_id
        .parse()
        .map_err(|e: uuid::Error| ApiError::bad_request(e.to_string()))?;
    let holder = holder_from(&headers)?;
    state
        .locks()
        .try_lock(row_id.as_uuid(), body.field_set.into(), &holder)
        .map_err(|e| match e.kind {
            LockErrorKind::Held { holder } => ApiError::conflict(format!("lock held by {holder}")),
            LockErrorKind::UnknownHolder => ApiError::bad_request("unknown holder"),
        })?;
    Ok(StatusCode::CREATED)
}

async fn release_lock(
    State(state): State<AppState>,
    Path(row_id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<LockBody>,
) -> Result<impl IntoResponse, ApiError> {
    let row_id: RowId = row_id
        .parse()
        .map_err(|e: uuid::Error| ApiError::bad_request(e.to_string()))?;
    let holder = holder_from(&headers)?;
    state
        .locks()
        .release(row_id.as_uuid(), body.field_set.into(), &holder)
        .map_err(|e| match e.kind {
            LockErrorKind::Held { holder } => ApiError::conflict(format!("lock held by {holder}")),
            LockErrorKind::UnknownHolder => ApiError::bad_request("not lock holder"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}
