//! `/components` routes — list, fetch, create.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use signex_library::adapter::{ComponentSummary, LibraryQuery};
use signex_library::component::Component;
use signex_library::identity::ComponentId;

use crate::db::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/components", get(list_components).post(create_component))
        .route("/components/:uuid", get(get_component))
}

#[derive(Debug, Deserialize, Default)]
struct ListQuery {
    text: Option<String>,
    category: Option<String>,
}

async fn list_components(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<ListQuery>,
) -> Result<Json<Vec<ComponentSummary>>, ApiError> {
    let mut all = state.list_components().await?;
    if let Some(text) = q.text.as_deref() {
        let needle = text.to_lowercase();
        all.retain(|s| {
            s.internal_pn.as_str().to_lowercase().contains(&needle)
                || s.mpn.to_lowercase().contains(&needle)
                || s.description.to_lowercase().contains(&needle)
        });
    }
    if let Some(_cat) = q.category.as_deref() {
        // Categories are encoded in parameters; v0.9.2 server-side facet
        // filtering can refine this. For now return the unfiltered set.
    }
    Ok(Json(all))
}

async fn get_component(
    State(state): State<AppState>,
    Path(uuid): Path<String>,
) -> Result<Json<Component>, ApiError> {
    let uuid = ComponentId::parse_str(&uuid).map_err(|e| ApiError::bad_request(e.to_string()))?;
    let comp = state.fetch_component(uuid).await?;
    comp.map(Json)
        .ok_or_else(|| ApiError::not_found(format!("component {uuid}")))
}

async fn create_component(
    State(state): State<AppState>,
    Json(comp): Json<Component>,
) -> Result<impl IntoResponse, ApiError> {
    state.insert_component(&comp).await?;
    Ok((StatusCode::CREATED, Json(comp)))
}

/// Helper to plug `LibraryQuery` into the URL `?text=...` form.
#[allow(dead_code)]
fn query_to_url_string(q: &LibraryQuery) -> String {
    let mut s = String::new();
    if let Some(t) = q.text.as_deref() {
        s.push_str("text=");
        s.push_str(&urlencoding(t));
    }
    if let Some(c) = q.category.as_deref() {
        if !s.is_empty() {
            s.push('&');
        }
        s.push_str("category=");
        s.push_str(&urlencoding(c));
    }
    s
}

#[allow(dead_code)]
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            for byte in ch.to_string().as_bytes() {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

// ---------- error envelope ---------------------------------------------------

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.into(),
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::internal(format!("db: {e}"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}
