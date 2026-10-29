//! Client side of the row-schema write guard for the Database backend.
//!
//! A server older than this build would store a row without the fields it
//! does not know. Before a row write the adapter reads the server's
//! `row_schema` from `/version`; a value below [`ROW_SCHEMA`] refuses the
//! write. Primitive requests are guarded the same way through
//! `primitive_revisions`: a server without it upserts on `POST`, so the check
//! must come before any primitive request. Only an acceptable answer is
//! cached, and only for a short while. A request that fails in transport, or
//! a reply that lacks the primitive `ETag`, drops the cache; a server
//! downgraded with no failed request in between is an accepted TOCTOU of at
//! most one recheck window.

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
    /// When the server last answered that it keeps primitive revisions.
    primitives_at: Mutex<Option<Instant>>,
}

impl RowSchemaGuard {
    /// Ok when the server accepts this build's rows; reads `/version` again
    /// unless an acceptable answer is still fresh.
    pub(super) fn check(
        &self,
        client: &reqwest::blocking::Client,
        version_url: &str,
    ) -> Result<(), LibraryError> {
        if Self::is_fresh(&self.accepted_at) {
            return Ok(());
        }
        let (server, _) = match fetch_version(client, version_url) {
            Ok(v) => v,
            Err(e) => {
                self.forget();
                return Err(e);
            }
        };
        if server < ROW_SCHEMA {
            self.forget();
            return Err(LibraryError::InvalidData(format!(
                "the library server is too old for this signex (row schema {server}, need \
                 {ROW_SCHEMA}); {UPGRADE_HINT}"
            )));
        }
        Self::set(&self.accepted_at, Some(Instant::now()));
        Ok(())
    }

    /// Ok when the server keeps primitive revisions. A server from before
    /// them treats a primitive `POST` as an upsert, so this runs before any
    /// primitive request: the refusal must come before the write, not after.
    pub(super) fn check_primitives(
        &self,
        client: &reqwest::blocking::Client,
        version_url: &str,
    ) -> Result<(), LibraryError> {
        if Self::is_fresh(&self.primitives_at) {
            return Ok(());
        }
        let (_, revisions) = match fetch_version(client, version_url) {
            Ok(v) => v,
            Err(e) => {
                self.forget();
                return Err(e);
            }
        };
        if !revisions {
            self.forget();
            return Err(LibraryError::InvalidData(format!(
                "the library server is too old for this signex (it keeps no primitive \
                 revisions, so a save could overwrite someone else's version); {UPGRADE_HINT}"
            )));
        }
        Self::set(&self.primitives_at, Some(Instant::now()));
        Ok(())
    }

    /// Drop the cached answers so the next request re-checks `/version`.
    pub(super) fn forget(&self) {
        Self::set(&self.accepted_at, None);
        Self::set(&self.primitives_at, None);
    }

    fn is_fresh(slot: &Mutex<Option<Instant>>) -> bool {
        slot.lock()
            .map(|g| g.is_some_and(|at| at.elapsed() < ROW_SCHEMA_RECHECK))
            .unwrap_or(false)
    }

    fn set(slot: &Mutex<Option<Instant>>, value: Option<Instant>) {
        if let Ok(mut g) = slot.lock() {
            *g = value;
        }
    }
}

/// The server's `(row_schema, primitive_revisions)` from `/version`; a
/// missing field reads as the oldest value.
fn fetch_version(
    client: &reqwest::blocking::Client,
    version_url: &str,
) -> Result<(u32, bool), LibraryError> {
    let fail =
        |e: &dyn std::fmt::Display| LibraryError::Backend(format!("library server /version: {e}"));
    let resp = client.get(version_url).send().map_err(|e| fail(&e))?;
    if !resp.status().is_success() {
        return Err(fail(&resp.status()));
    }
    let body: serde_json::Value = resp.json().map_err(|e| fail(&e))?;
    let row_schema = body["row_schema"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(1);
    Ok((
        row_schema,
        body["primitive_revisions"].as_bool() == Some(true),
    ))
}

/// The error for a row write the server refused as a schema mismatch, when
/// the response body carries the stable marker.
pub(super) fn schema_refusal(body: &serde_json::Value) -> Option<LibraryError> {
    (body["code"].as_str() == Some(ROW_SCHEMA_REFUSED_CODE)).then(|| {
        LibraryError::InvalidData(format!(
            "the library server refused the row write as a schema mismatch; {UPGRADE_HINT}"
        ))
    })
}
