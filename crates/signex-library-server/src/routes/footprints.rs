//! `/footprints` routes — primitive CRUD mirror of `routes::symbols`.
//!
//! Per `v0.9-library-refactor-plan.md` §9 Step D4. The wire format is the
//! JSON-serialised `Footprint` struct (which itself embeds `Body3D`, optional
//! `StepAttachment`, and the pad list — all handled transparently by serde).

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use signex_library::primitive::Footprint;
use uuid::Uuid;

use crate::db::{AppState, PrimitiveSummary};
use crate::routes::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/footprints", get(list_footprints).post(create_footprint))
        .route("/footprints/:uuid", get(get_footprint))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    library_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    library_id: Uuid,
    #[serde(flatten)]
    footprint: Footprint,
}

async fn create_footprint(
    State(state): State<AppState>,
    Json(body): Json<CreateBody>,
) -> Result<impl IntoResponse, ApiError> {
    state
        .insert_footprint(body.library_id, &body.footprint)
        .await?;
    Ok((StatusCode::CREATED, Json(body.footprint)))
}

async fn get_footprint(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Footprint>, ApiError> {
    let library_id = q
        .library_id
        .ok_or_else(|| ApiError::bad_request("missing library_id query parameter"))?;
    let uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .fetch_footprint(library_id, uuid)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("footprint {library_id}/{uuid}")))
}

async fn list_footprints(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PrimitiveSummary>>, ApiError> {
    let summaries = state.list_footprints(q.library_id).await?;
    Ok(Json(summaries))
}
