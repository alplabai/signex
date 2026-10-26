//! Client side of the row-schema write guard for the Database backend.
//!
//! A server older than this build would store a row without the fields it
//! does not know. Before a row write the adapter reads the server's
//! `row_schema` from `/version`; a value below [`ROW_SCHEMA`] refuses the
//! write. Only an acceptable answer is cached, and only for a short while.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::adapter::LibraryError;
use crate::component::{ROW_SCHEMA, ROW_SCHEMA_REFUSED_CODE};

/// How long an acceptable `/version` answer is trusted before it is re-read.
const ROW_SCHEMA_RECHECK: Duration = Duration::from_secs(60);

/// Message shown whenever a row write is refused for a schema mismatch.
const UPGRADE_HINT: &str = "upgrade signex and the library server together";

#[derive(Default)]
pub(super) struct RowSchemaGuard {
    /// When the server last answered with an acceptable `row_schema`.
    accepted_at: Mutex<Option<Instant>>,
}

impl RowSchemaGuard {
    /// Ok when the server accepts this build's rows; reads `/version` again
    /// unless an acceptable answer is still fresh.
    pub(super) fn check(
        &self,
        client: &reqwest::blocking::Client,
        version_url: &str,
    ) -> Result<(), LibraryError> {
        if self.is_fresh() {
            return Ok(());
        }
        let server = match fetch_row_schema(client, version_url) {
            Ok(v) => v,
            Err(e) => {
                self.forget();
                return Err(e);
            }
        };
        if server < ROW_SCHEMA {
            self.forget();
            return Err(LibraryError::Backend(format!(
                "the library server is too old for this signex (row schema {server}, need \
                 {ROW_SCHEMA}); {UPGRADE_HINT}"
            )));
        }
        self.set(Some(Instant::now()));
        Ok(())
    }

    /// Drop the cached answer so the next write re-checks `/version`.
    pub(super) fn forget(&self) {
        self.set(None);
    }

    fn is_fresh(&self) -> bool {
        self.accepted_at
            .lock()
            .map(|g| g.is_some_and(|at| at.elapsed() < ROW_SCHEMA_RECHECK))
            .unwrap_or(false)
    }

    fn set(&self, value: Option<Instant>) {
        if let Ok(mut g) = self.accepted_at.lock() {
            *g = value;
        }
    }
}

fn fetch_row_schema(
    client: &reqwest::blocking::Client,
    version_url: &str,
) -> Result<u32, LibraryError> {
    let fail =
        |e: &dyn std::fmt::Display| LibraryError::Backend(format!("library server /version: {e}"));
    let resp = client.get(version_url).send().map_err(|e| fail(&e))?;
    if !resp.status().is_success() {
        return Err(fail(&resp.status()));
    }
    let body: serde_json::Value = resp.json().map_err(|e| fail(&e))?;
    Ok(body["row_schema"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(1))
}

/// The error for a row write the server refused as a schema mismatch, when
/// the response body carries the stable marker.
pub(super) fn schema_refusal(body: &serde_json::Value) -> Option<LibraryError> {
    (body["code"].as_str() == Some(ROW_SCHEMA_REFUSED_CODE)).then(|| {
        LibraryError::Backend(format!(
            "the library server refused the row write as a schema mismatch; {UPGRADE_HINT}"
        ))
    })
}
