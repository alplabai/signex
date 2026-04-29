//! `/sims` routes — primitive CRUD mirror of `routes::symbols`.
//!
//! Per `v0.9-refactor-2-plan.md` §9 Step D4. The wire format is the
//! JSON-serialised `SimModel` struct (SPICE/Verilog-A body + default
//! pin → SPICE-node mapping).

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use signex_library::primitive::SimModel;
use uuid::Uuid;

use crate::db::{AppState, PrimitiveSummary};
use crate::routes::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/sims", get(list_sims).post(create_sim))
        .route("/sims/:uuid", get(get_sim))
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
) -> Result<impl IntoResponse, ApiError> {
    state.insert_sim(body.library_id, &body.sim).await?;
    Ok((StatusCode::CREATED, Json(body.sim)))
}

async fn get_sim(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<SimModel>, ApiError> {
    let library_id = q
        .library_id
        .ok_or_else(|| ApiError::bad_request("missing library_id query parameter"))?;
    let uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .fetch_sim(library_id, uuid)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("sim {library_id}/{uuid}")))
}

async fn list_sims(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PrimitiveSummary>>, ApiError> {
    let summaries = state.list_sims(q.library_id).await?;
    Ok(Json(summaries))
}
