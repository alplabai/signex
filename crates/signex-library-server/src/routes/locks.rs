//! `/components/:uuid/locks` — advisory locking.
//!
//! The caller identifies itself with the `x-signex-holder` header. Body picks
//! the field-set:
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
use signex_library::identity::ComponentId;

use crate::db::AppState;
use crate::locks::LockErrorKind;
use crate::routes::components::ApiError;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/components/:uuid/locks",
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
    Path(uuid): Path<String>,
    headers: HeaderMap,
    Json(body): Json<LockBody>,
) -> Result<impl IntoResponse, ApiError> {
    let uuid = ComponentId::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let holder = holder_from(&headers)?;
    state
        .locks()
        .try_lock(uuid, body.field_set.into(), &holder)
        .map_err(|e| match e.kind {
            LockErrorKind::Held { holder } => ApiError::conflict(format!("lock held by {holder}")),
            LockErrorKind::UnknownHolder => ApiError::bad_request("unknown holder"),
        })?;
    Ok(StatusCode::CREATED)
}

async fn release_lock(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    Json(body): Json<LockBody>,
) -> Result<impl IntoResponse, ApiError> {
    let uuid = ComponentId::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let holder = holder_from(&headers)?;
    state
        .locks()
        .release(uuid, body.field_set.into(), &holder)
        .map_err(|e| match e.kind {
            LockErrorKind::Held { holder } => ApiError::conflict(format!("lock held by {holder}")),
            LockErrorKind::UnknownHolder => ApiError::bad_request("not lock holder"),
        })?;
    Ok(StatusCode::NO_CONTENT)
}
