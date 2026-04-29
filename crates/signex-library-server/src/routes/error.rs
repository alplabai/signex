//! Shared `ApiError` envelope reused by every route module.
//!
//! Sits in its own module so the `tables` / `rows` row tier and the
//! `symbols` / `footprints` / `sims` primitive routes can share one
//! status-code → JSON-body contract.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{Json, response::Response};

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
    /// M4: never echo sqlx::Error verbatim — it leaks table/column/constraint
    /// names that help attackers map the schema. Log server-side at error
    /// level so operators still see the underlying failure.
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        Self::internal("internal server error")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}
