//! `/footprints` routes — primitive CRUD mirror of `routes::symbols`.
//!
//! Per `v0.9-refactor-2-plan.md` §9 Step D4. The wire format is the
//! JSON-serialised `Footprint` struct (which itself embeds `Body3D`, optional
//! `StepAttachment`, and the pad list — all handled transparently by serde).

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use serde::Deserialize;
use signex_library::primitive::Footprint;
use uuid::Uuid;

use crate::db::{AppState, Page, PrimitiveSummary};
use crate::routes::error::ApiError;
use crate::routes::primitives::{already_exists, require_same_uuid, subject, with_etag};
use crate::routes::rows::{applied, base_revision};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/footprints", get(list_footprints).post(create_footprint))
        .route(
            "/footprints/:uuid",
            get(get_footprint).put(update_footprint),
        )
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    library_id: Option<Uuid>,
    /// Page size; defaults to and is capped by the server.
    limit: Option<i64>,
    offset: Option<i64>,
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
) -> Result<Response, ApiError> {
    let inserted = state
        .insert_footprint(body.library_id, &body.footprint)
        .await?;
    if !inserted {
        return Err(already_exists(
            "footprint",
            body.library_id,
            body.footprint.uuid,
        ));
    }
    Ok(with_etag(StatusCode::CREATED, 1, body.footprint))
}

async fn update_footprint(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    let url_uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    require_same_uuid(url_uuid, body.footprint.uuid)?;
    let base = base_revision(&headers, "footprint")?;
    let write = state
        .update_footprint(body.library_id, &body.footprint, base)
        .await?;
    let revision = applied(write, &subject("footprint", body.library_id, url_uuid))?;
    Ok(with_etag(StatusCode::OK, revision, body.footprint))
}

async fn get_footprint(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Response, ApiError> {
    let library_id = q
        .library_id
        .ok_or_else(|| ApiError::bad_request("missing library_id query parameter"))?;
    let uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .fetch_footprint(library_id, uuid)
        .await?
        .map(|(item, revision)| with_etag(StatusCode::OK, revision, item))
        .ok_or_else(|| ApiError::not_found(format!("footprint {library_id}/{uuid}")))
}

async fn list_footprints(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PrimitiveSummary>>, ApiError> {
    let summaries = state
        .list_footprints(q.library_id, Page::new(q.limit, q.offset))
        .await?;
    Ok(Json(summaries))
}
