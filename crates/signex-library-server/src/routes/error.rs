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
    /// Set on a stale-revision 409 so the client can tell the user which
    /// revision the row is at now.
    pub current_revision: Option<i64>,
    /// Stable machine-checkable marker clients match on.
    pub code: Option<&'static str>,
}

impl ApiError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
            current_revision: None,
            code: None,
        }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
            current_revision: None,
            code: None,
        }
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.into(),
            current_revision: None,
            code: None,
        }
    }
    /// 428: a row write arrived without the `If-Match` base revision.
    pub fn precondition_required(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::PRECONDITION_REQUIRED,
            message: msg.into(),
            current_revision: None,
            code: None,
        }
    }
    /// 400 with `code: row_schema_too_old`: the client build is older than
    /// the server's row schema.
    pub fn row_schema_too_old(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
            current_revision: None,
            code: Some(signex_library::ROW_SCHEMA_REFUSED_CODE),
        }
    }
    /// 409: the row moved past the writer's base revision.
    pub fn stale_revision(msg: impl Into<String>, current_revision: i64) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: msg.into(),
            current_revision: Some(current_revision),
            code: None,
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
            current_revision: None,
            code: None,
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
        let mut body = serde_json::json!({ "error": self.message });
        if let Some(current_revision) = self.current_revision {
            body["current_revision"] = current_revision.into();
        }
        if let Some(code) = self.code {
            body["code"] = code.into();
        }
        (self.status, Json(body)).into_response()
    }
}
