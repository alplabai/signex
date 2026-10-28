//! `/sims` routes — primitive CRUD mirror of `routes::symbols`.
//!
//! Per `v0.9-refactor-2-plan.md` §9 Step D4. The wire format is the
//! JSON-serialised `SimModel` struct (SPICE/Verilog-A body + default
//! pin → SPICE-node mapping).

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use serde::Deserialize;
use signex_library::primitive::SimModel;
use uuid::Uuid;

use crate::db::{AppState, PrimitiveSummary};
use crate::routes::error::ApiError;
use crate::routes::primitives::{already_exists, require_same_uuid, subject, with_etag};
use crate::routes::rows::{applied, base_revision};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sims", get(list_sims).post(create_sim))
        .route("/sims/:uuid", get(get_sim).put(update_sim))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    library_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    library_id: Uuid,
    #[serde(flatten)]
    sim: SimModel,
}

async fn create_sim(
    State(state): State<AppState>,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    let inserted = state.insert_sim(body.library_id, &body.sim).await?;
    if !inserted {
        return Err(already_exists("sim", body.library_id, body.sim.uuid));
    }
    Ok(with_etag(StatusCode::CREATED, 1, body.sim))
}

async fn update_sim(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    let url_uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    require_same_uuid(url_uuid, body.sim.uuid)?;
    let base = base_revision(&headers, "sim")?;
    let write = state.update_sim(body.library_id, &body.sim, base).await?;
    let revision = applied(write, &subject("sim", body.library_id, url_uuid))?;
    Ok(with_etag(StatusCode::OK, revision, body.sim))
}

async fn get_sim(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Response, ApiError> {
    let library_id = q
        .library_id
        .ok_or_else(|| ApiError::bad_request("missing library_id query parameter"))?;
    let uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .fetch_sim(library_id, uuid)
        .await?
        .map(|(item, revision)| with_etag(StatusCode::OK, revision, item))
        .ok_or_else(|| ApiError::not_found(format!("sim {library_id}/{uuid}")))
}

async fn list_sims(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PrimitiveSummary>>, ApiError> {
    let summaries = state.list_sims(q.library_id).await?;
    Ok(Json(summaries))
}
