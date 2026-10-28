//! `/symbols` routes — primitive CRUD for the v0.9 library refactor.
//!
//! Per `v0.9-refactor-2-plan.md` §9 Step D3, primitives are addressed by
//! `(library_id, uuid)` tuples. The wire format is the JSON-serialised
//! `Symbol` struct from `signex-library`. Routes are bearer-token gated like
//! the other routes — `router_with_state` slots them into the protected
//! sub-router.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::get,
};
use serde::Deserialize;
use signex_library::primitive::Symbol;
use uuid::Uuid;

use crate::db::{AppState, Page, PrimitiveSummary};
use crate::routes::error::ApiError;
use crate::routes::primitives::{already_exists, require_same_uuid, subject, with_etag};
use crate::routes::rows::{applied, base_revision};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/symbols", get(list_symbols).post(create_symbol))
        .route("/symbols/:uuid", get(get_symbol).put(update_symbol))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    /// Filter to a single library — handy when the editor surfaces just the
    /// open library's primitives. Unset → return every symbol the server
    /// knows about.
    library_id: Option<Uuid>,
    /// Page size; defaults to and is capped by the server.
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CreateBody {
    /// Library this symbol belongs to. Pulled from the request body rather
    /// than the URL because libraries are conceptually owners but routes are
    /// flat (per §9 Step D3 — `POST /symbols`).
    library_id: Uuid,
    #[serde(flatten)]
    symbol: Symbol,
}

async fn create_symbol(
    State(state): State<AppState>,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    let inserted = state.insert_symbol(body.library_id, &body.symbol).await?;
    if !inserted {
        return Err(already_exists("symbol", body.library_id, body.symbol.uuid));
    }
    Ok(with_etag(StatusCode::CREATED, 1, body.symbol))
}

async fn update_symbol(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    let url_uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    require_same_uuid(url_uuid, body.symbol.uuid)?;
    let base = base_revision(&headers, "symbol")?;
    let write = state
        .update_symbol(body.library_id, &body.symbol, base)
        .await?;
    let revision = applied(write, &subject("symbol", body.library_id, url_uuid))?;
    Ok(with_etag(StatusCode::OK, revision, body.symbol))
}

async fn get_symbol(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Response, ApiError> {
    let library_id = q
        .library_id
        .ok_or_else(|| ApiError::bad_request("missing library_id query parameter"))?;
    let uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .fetch_symbol(library_id, uuid)
        .await?
        .map(|(item, revision)| with_etag(StatusCode::OK, revision, item))
        .ok_or_else(|| ApiError::not_found(format!("symbol {library_id}/{uuid}")))
}

async fn list_symbols(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PrimitiveSummary>>, ApiError> {
    let summaries = state
        .list_symbols(q.library_id, Page::new(q.limit, q.offset))
        .await?;
    Ok(Json(summaries))
}
