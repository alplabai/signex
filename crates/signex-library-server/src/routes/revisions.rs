//! `/components/:uuid/revisions` — append, fetch.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use signex_library::component::Revision;
use signex_library::identity::{ComponentId, InternalPn, Version};

use crate::db::AppState;
use crate::routes::components::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/components/:uuid/revisions",
            post(post_revision).get(list_revisions),
        )
        .route("/components/:uuid/revisions/:version", get(get_revision))
}

async fn post_revision(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Json(rev): Json<Revision>,
) -> Result<impl IntoResponse, ApiError> {
    let uuid = ComponentId::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let internal_pn = state
        .fetch_component(uuid)
        .await?
        .map(|c| c.internal_pn)
        .unwrap_or_else(|| InternalPn::new(format!("UNNAMED-{}", uuid.as_simple())));
    state.save_revision(uuid, &rev, &internal_pn).await?;
    Ok((StatusCode::CREATED, Json(rev)))
}

async fn list_revisions(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Vec<Revision>>, ApiError> {
    let uuid = ComponentId::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let comp = state.fetch_component(uuid).await?;
    let revisions = comp.map(|c| c.revisions).unwrap_or_default();
    Ok(Json(revisions))
}

async fn get_revision(
    State(state): State<AppState>,
    Path((uuid, version)): Path<(String, String)>,
) -> Result<Json<Revision>, ApiError> {
    let uuid = ComponentId::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let version: Version = version
        .parse()
        .map_err(|e: signex_library::identity::ParseVersionError| {
            ApiError::bad_request(e.to_string())
        })?;
    let comp = state
        .fetch_component(uuid)
        .await?
        .ok_or_else(|| ApiError::not_found(format!("component {uuid}")))?;
    let rev = comp
        .revisions
        .into_iter()
        .find(|r| r.version == version)
        .ok_or_else(|| ApiError::not_found(format!("revision {version}")))?;
    Ok(Json(rev))
}
