//! `/symbols` routes — primitive CRUD for the v0.9 library refactor.
//!
//! Per `v0.9-library-refactor-plan.md` §9 Step D3, primitives are addressed by
//! `(library_id, uuid)` tuples. The wire format is the JSON-serialised
//! `Symbol` struct from `signex-library`. Routes are bearer-token gated like
//! the existing `/components` family — `router_with_state` slots them into
//! the protected sub-router.

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use signex_library::primitive::Symbol;
use uuid::Uuid;

use crate::db::{AppState, PrimitiveSummary};
use crate::routes::error::ApiError;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/symbols", get(list_symbols).post(create_symbol))
        .route("/symbols/:uuid", get(get_symbol))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    /// Filter to a single library — handy when the editor surfaces just the
    /// open library's primitives. Unset → return every symbol the server
    /// knows about.
    library_id: Option<Uuid>,
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
) -> Result<impl IntoResponse, ApiError> {
    state.insert_symbol(body.library_id, &body.symbol).await?;
    Ok((StatusCode::CREATED, Json(body.symbol)))
}

async fn get_symbol(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Symbol>, ApiError> {
    let library_id = q
        .library_id
        .ok_or_else(|| ApiError::bad_request("missing library_id query parameter"))?;
    let uuid = Uuid::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    state
        .fetch_symbol(library_id, uuid)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("symbol {library_id}/{uuid}")))
}

async fn list_symbols(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<PrimitiveSummary>>, ApiError> {
    let summaries = state.list_symbols(q.library_id).await?;
    Ok(Json(summaries))
}
