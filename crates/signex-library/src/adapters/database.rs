//! `LibraryAdapter` over the HTTP API exposed by `signex-library-server`.
//!
//! Synchronous facade for the trait. Row CRUD speaks to the
//! `/tables` / `/rows` routes; primitive (`/symbols` / `/footprints`
//! / `/sims`) wiring is unchanged because primitives stay
//! file-shaped under the DBLib model.
//!
//! Routes are addressed by a `library_id` query parameter — the
//! adapter sources its own from `manifest().library.library_id`.
//! Mutating calls carry their commit message in the
//! `x-signex-message` header so the server-side audit log has it
//! (the DB backend doesn't have its own commit graph the way
//! `LocalGitAdapter` does — see TODO around the `audit_log` table
//! below).

use std::sync::OnceLock;
use std::time::Duration;

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::adapter::{LibraryAdapter, LibraryError, PrimitiveSummary};
use crate::component::ComponentRow;
use crate::identity::{InternalPn, RowId};
use crate::library_file::SnxlibManifest;
use crate::manifest::{LibraryMeta, LibraryMode, Manifest};
use crate::primitive::{Footprint, SimModel, Symbol};

/// Connect-timeout for the HTTP client. A hung TLS handshake should
/// not freeze the iced runtime — fail fast and let the dispatcher
/// surface the error.
const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Total request timeout (header + body). Protects against the
/// server accepting the connection then never replying. 30s is
/// generous enough for a large `read_table` over slow links but
/// short enough to bound the worst-case freeze.
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub struct DatabaseAdapter {
    manifest: Manifest,
    base_url: String,
    /// Bearer token sent via `Authorization: Bearer <token>`. Static
    /// credential for now; OIDC lands later.
    token: Option<String>,
    /// Caller-facing string used for advisory locks — defaults to the
    /// bearer token's caller identity. When OIDC lands this becomes the
    /// JWT `sub`.
    holder: String,
    client: reqwest::blocking::Client,
}

impl DatabaseAdapter {
    /// Construct from a manifest. The manifest's `auth` field is treated as
    /// the bearer token; the holder is derived from it.
    pub fn new(manifest: Manifest) -> Result<Self, LibraryError> {
        let (base_url, auth) = match &manifest.mode {
            LibraryMode::Database { url, auth } => {
                (url.trim_end_matches('/').to_string(), auth.clone())
            }
            other => {
                return Err(LibraryError::Backend(format!(
                    "DatabaseAdapter requires LibraryMode::Database, got {other:?}"
                )));
            }
        };
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| LibraryError::Backend(format!("reqwest client: {e}")))?;
        let token = if auth.is_empty() {
            None
        } else {
            Some(auth.clone())
        };
        Ok(Self {
            manifest,
            base_url,
            token,
            holder: auth,
            client,
        })
    }

    /// Construct from a [`SnxlibManifest`] — the v0.9 manifest shape.
    ///
    /// The DB adapter has no `.snxlib` file on disk, but it still
    /// carries the same library metadata in [`SnxlibManifest::mode`]
    /// (must be [`LibraryMode::Database`]). This constructor mirrors
    /// [`crate::adapters::local_git::LocalGitAdapter::init`]'s API
    /// surface so callers reaching for one or the other can use the
    /// same manifest type.
    ///
    /// Synthesises the legacy [`Manifest`] internally for the
    /// [`LibraryAdapter::manifest`] callers — Stage 5+ retires those
    /// and lets us drop the synthesis.
    pub fn from_snxlib(manifest: SnxlibManifest) -> Result<Self, LibraryError> {
        Self::new(synthesize_legacy_manifest(manifest))
    }

    /// Explicit bearer-token + holder constructor.
    pub fn with_token(
        url: impl Into<String>,
        token: impl Into<String>,
        holder: impl Into<String>,
    ) -> Result<Self, LibraryError> {
        let base_url = url.into().trim_end_matches('/').to_string();
        let token = token.into();
        let token = if token.is_empty() { None } else { Some(token) };
        let holder = holder.into();
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| LibraryError::Backend(format!("reqwest client: {e}")))?;
        // We don't have a real Manifest here — fabricate the minimal one
        // callers might inspect. The `auth` slot is filled with the holder
        // (not the token) so logging never accidentally leaks the credential.
        let manifest = Manifest {
            library: crate::manifest::LibraryMeta {
                name: "remote".into(),
                library_id: uuid::Uuid::nil(),
                description: None,
            },
            mode: LibraryMode::Database {
                url: base_url.clone(),
                auth: holder.clone(),
            },
            workflow: Default::default(),
            users: Default::default(),
            tables: Vec::new(),
        };
        Ok(Self {
            manifest,
            base_url,
            token,
            holder,
            client,
        })
    }

    /// Borrow the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Borrow the holder identity (logged but never the bearer secret).
    pub fn holder(&self) -> &str {
        &self.holder
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// Percent-encode a single path segment per RFC 3986.
    /// Used for every user-controlled string that flows into the URL
    /// path (table names, primitive collection names) so a value like
    /// `"Discrete Passives"` or `"resistors?evil"` can't reshape the
    /// request URL.
    fn encode_segment(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for b in s.bytes() {
            match b {
                b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    out.push(b as char)
                }
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    /// Apply the `Authorization: Bearer <token>` header when configured.
    fn auth(&self, req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if let Some(token) = &self.token {
            req.header("authorization", format!("Bearer {token}"))
        } else {
            req
        }
    }

    /// Generic GET → JSON for a primitive at `/{collection}/{uuid}`.
    /// `collection` is percent-encoded; `uuid` is hex (Display) so it
    /// is already URL-safe.
    fn get_primitive_json<T: DeserializeOwned>(
        &self,
        collection: &str,
        uuid: Uuid,
        kind_label: &str,
    ) -> Result<T, LibraryError> {
        let coll = Self::encode_segment(collection);
        let resp = self
            .auth(self.client.get(self.url(&format!("/{coll}/{uuid}"))))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => resp
                .json::<T>()
                .map_err(|e| LibraryError::Backend(e.to_string())),
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("{kind_label} {uuid}")))
            }
            other => Err(LibraryError::Backend(format!("get {kind_label}: {other}"))),
        }
    }

    /// Generic POST primitive JSON to `/{collection}` with the supplied
    /// commit message in the `x-signex-message` header.
    fn post_primitive_json<T: Serialize>(
        &self,
        collection: &str,
        body: &T,
        message: &str,
    ) -> Result<(), LibraryError> {
        let coll = Self::encode_segment(collection);
        let resp = self
            .auth(
                self.client
                    .post(self.url(&format!("/{coll}")))
                    .header("x-signex-message", message)
                    .json(body),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "save {collection}: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    /// Generic GET → list at `/{collection}` returning [`PrimitiveSummary`].
    fn list_primitives_json(
        &self,
        collection: &str,
    ) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        let coll = Self::encode_segment(collection);
        let resp = self
            .auth(self.client.get(self.url(&format!("/{coll}"))))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "list {collection}: {}",
                resp.status()
            )));
        }
        resp.json::<Vec<PrimitiveSummary>>()
            .map_err(|e| LibraryError::Backend(e.to_string()))
    }

    /// `library_id` query string segment used by every row/table call. The
    /// server keys row storage by `(library_id, table_name, row_id)`; a
    /// missing or wrong id surfaces as a 404 from the route, which we map
    /// to `NotFound` at the call site.
    fn library_id_query(&self) -> String {
        format!("library_id={}", self.manifest.library.library_id)
    }
}

impl LibraryAdapter for DatabaseAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    // The new (Stage 2) `library_file` / `root_dir` / `library_file_path`
    // trait methods all default to `None`. The DB adapter inherits those
    // defaults intentionally — there is no `.snxlib` file on disk for a
    // remote-DB-backed library; the tables live server-side. Callers
    // that need the parsed view should go through the row-CRUD methods
    // (`list_tables` / `read_table` / etc.) which work for both
    // backends.

    // ── Row + table CRUD ─────────────────────────────────────────────────
    //
    // The adapter forwards each method to its route on
    // `signex-library-server`. The server-side DB schema lives in
    // `migrations/0005_tabular_components.sql`; the wire format is the
    // `ComponentRow` JSON serialisation defined in `component::ComponentRow`.
    //
    // TODO(audit): mutating routes pass a commit message via
    // `x-signex-message`, but the DB backend has no audit_log table yet —
    // the message currently shows up only in `tracing::info!` lines.
    // v0.9.x can add an `audit_log (library_id, row_id, actor, message,
    // occurred)` row per mutation when the workflow grows server-side
    // history beyond what the route handler logs surface.

    fn list_tables(&self) -> Result<Vec<String>, LibraryError> {
        let url = self.url(&format!("/tables?{}", self.library_id_query()));
        let resp = self
            .auth(self.client.get(url))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "list_tables: {}",
                resp.status()
            )));
        }
        resp.json::<Vec<String>>()
            .map_err(|e| LibraryError::Backend(e.to_string()))
    }

    fn read_table(&self, name: &str) -> Result<Vec<ComponentRow>, LibraryError> {
        let n = Self::encode_segment(name);
        let url = self.url(&format!("/tables/{n}?{}", self.library_id_query()));
        let resp = self
            .auth(self.client.get(url))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => resp
                .json::<Vec<ComponentRow>>()
                .map_err(|e| LibraryError::Backend(e.to_string())),
            reqwest::StatusCode::NOT_FOUND => Err(LibraryError::NotFound(format!("table {name}"))),
            other => Err(LibraryError::Backend(format!("read_table {name}: {other}"))),
        }
    }

    /// Composed from `list_tables` + `read_table` per plan §9 (the server
    /// only ships the 6 row/table routes; no aggregate `/rows` endpoint).
    /// Cost is one round-trip per table, then one per non-empty table —
    /// fine at v0.9 scale.
    fn iter_rows(&self) -> Result<Vec<(String, ComponentRow)>, LibraryError> {
        let mut out = Vec::new();
        for name in self.list_tables()? {
            let rows = self.read_table(&name)?;
            for row in rows {
                out.push((name.clone(), row));
            }
        }
        Ok(out)
    }

    fn read_row(&self, table: &str, row_id: RowId) -> Result<ComponentRow, LibraryError> {
        let t = Self::encode_segment(table);
        let url = self.url(&format!(
            "/tables/{t}/rows/{row_id}?{}",
            self.library_id_query()
        ));
        let resp = self
            .auth(self.client.get(url))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => resp
                .json::<ComponentRow>()
                .map_err(|e| LibraryError::Backend(e.to_string())),
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("row {table}/{row_id}")))
            }
            other => Err(LibraryError::Backend(format!(
                "read_row {table}/{row_id}: {other}"
            ))),
        }
    }

    /// Linear scan via `iter_rows` — same composition rationale as
    /// `iter_rows`. The server has no PN index endpoint; v0.9 acceptable.
    fn read_row_by_pn(&self, pn: &InternalPn) -> Result<(String, ComponentRow), LibraryError> {
        for (table, row) in self.iter_rows()? {
            if &row.internal_pn == pn {
                return Ok((table, row));
            }
        }
        Err(LibraryError::NotFound(format!("internal_pn {pn}")))
    }

    fn insert_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        // The DB backend has no commit log — surface the audit message at
        // the `tracing` layer so it shows up in operator logs even before
        // the planned `audit_log` table lands.
        tracing::info!(
            target: "signex_library::database",
            library_id = %self.manifest.library.library_id,
            table = table,
            row_id = %row.row_id,
            internal_pn = %row.internal_pn,
            message = msg,
            "insert_row",
        );
        let t = Self::encode_segment(table);
        let url = self.url(&format!("/tables/{t}/rows?{}", self.library_id_query()));
        let resp = self
            .auth(
                self.client
                    .post(url)
                    .header("x-signex-message", msg)
                    .json(&row),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "insert_row {table}: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn update_row(&self, table: &str, row: ComponentRow, msg: &str) -> Result<(), LibraryError> {
        tracing::info!(
            target: "signex_library::database",
            library_id = %self.manifest.library.library_id,
            table = table,
            row_id = %row.row_id,
            internal_pn = %row.internal_pn,
            message = msg,
            "update_row",
        );
        let row_id = row.row_id;
        let t = Self::encode_segment(table);
        let url = self.url(&format!(
            "/tables/{t}/rows/{row_id}?{}",
            self.library_id_query()
        ));
        let resp = self
            .auth(
                self.client
                    .put(url)
                    .header("x-signex-message", msg)
                    .json(&row),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => Ok(()),
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("row {table}/{row_id}")))
            }
            other => Err(LibraryError::Backend(format!(
                "update_row {table}/{row_id}: {other}"
            ))),
        }
    }

    fn delete_row(&self, table: &str, row_id: RowId, msg: &str) -> Result<(), LibraryError> {
        tracing::info!(
            target: "signex_library::database",
            library_id = %self.manifest.library.library_id,
            table = table,
            row_id = %row_id,
            message = msg,
            "delete_row",
        );
        let t = Self::encode_segment(table);
        let url = self.url(&format!(
            "/tables/{t}/rows/{row_id}?{}",
            self.library_id_query()
        ));
        let resp = self
            .auth(self.client.delete(url).header("x-signex-message", msg))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => Ok(()),
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("row {table}/{row_id}")))
            }
            other => Err(LibraryError::Backend(format!(
                "delete_row {table}/{row_id}: {other}"
            ))),
        }
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.get_primitive_json::<Symbol>("symbols", uuid, "symbol")
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.get_primitive_json::<Footprint>("footprints", uuid, "footprint")
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.get_primitive_json::<SimModel>("sims", uuid, "sim")
    }

    fn save_symbol(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        self.post_primitive_json("symbols", &sym, message)
    }

    fn save_footprint(&self, fp: Footprint, message: &str) -> Result<(), LibraryError> {
        self.post_primitive_json("footprints", &fp, message)
    }

    fn save_sim(&self, sm: SimModel, message: &str) -> Result<(), LibraryError> {
        self.post_primitive_json("sims", &sm, message)
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitives_json("symbols")
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitives_json("footprints")
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitives_json("sims")
    }
}

// Suppress dead-code warning for the OnceLock when we add caching later.
#[allow(dead_code)]
static CACHE_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

/// Build a legacy [`Manifest`] from the v0.9 [`SnxlibManifest`] header.
///
/// The DB adapter's `manifest()` trait method continues to return a
/// `&Manifest` so existing callers (`new_component`, `dispatch/library`,
/// `state.rs`) keep working. Stage 5+ will retire this synthesis once
/// those callers move onto the new accessors.
fn synthesize_legacy_manifest(snx: SnxlibManifest) -> Manifest {
    Manifest {
        library: LibraryMeta {
            name: snx.library.name,
            library_id: snx.library_id,
            description: snx.library.description,
        },
        mode: snx.mode,
        workflow: snx.workflow,
        users: snx.users,
        // The new model carries tables in `LibraryFile.tables`, not
        // in the manifest header — Stage 12 retires the legacy field.
        tables: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::library_file::{FORMAT_TOKEN, LibrarySection};
    use crate::manifest::{UsersConfig, WorkflowConfig};

    /// `from_snxlib` mirrors `LocalGitAdapter::init`'s manifest API
    /// shape. The DB adapter requires `LibraryMode::Database`; passing
    /// the default `LocalGit` mode must fail loudly so a misconfigured
    /// project doesn't silently bring up a remote-shaped adapter
    /// pointed at nothing.
    #[test]
    fn from_snxlib_round_trips_database_mode() {
        let manifest = SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::nil(),
            library: LibrarySection {
                name: "Remote".into(),
                description: None,
            },
            mode: LibraryMode::Database {
                url: "https://example.com/api".into(),
                auth: "remote-token".into(),
            },
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        };
        let adapter = DatabaseAdapter::from_snxlib(manifest).unwrap();
        assert_eq!(adapter.base_url(), "https://example.com/api");
        assert_eq!(adapter.manifest().library.name, "Remote");
    }

    #[test]
    fn from_snxlib_rejects_non_database_mode() {
        let manifest = SnxlibManifest {
            format: FORMAT_TOKEN.into(),
            library_id: Uuid::nil(),
            library: LibrarySection {
                name: "OopsLocalGit".into(),
                description: None,
            },
            mode: LibraryMode::default(), // LocalGit — wrong for DB adapter
            workflow: WorkflowConfig::default(),
            users: UsersConfig::default(),
            classes: Vec::new(),
        };
        match DatabaseAdapter::from_snxlib(manifest) {
            Err(LibraryError::Backend(_)) => {}
            Err(other) => panic!("expected Backend error, got {other:?}"),
            Ok(_) => panic!("non-database mode should have been rejected"),
        }
    }

    #[test]
    fn with_token_round_trips_holder_and_url() {
        let adapter = DatabaseAdapter::with_token(
            "https://example.com/api/",
            "secret-bearer",
            "alice@example",
        )
        .unwrap();
        assert_eq!(adapter.base_url(), "https://example.com/api");
        assert_eq!(adapter.holder(), "alice@example");
    }

    #[test]
    fn encode_segment_passes_through_unreserved() {
        assert_eq!(
            DatabaseAdapter::encode_segment("Discrete-Passives_v2.0~final"),
            "Discrete-Passives_v2.0~final"
        );
    }

    #[test]
    fn encode_segment_escapes_path_breakers() {
        // Spaces, slashes, query separators, anchors — every byte that
        // could reshape the URL has to come back percent-encoded.
        assert_eq!(
            DatabaseAdapter::encode_segment("Discrete Passives"),
            "Discrete%20Passives"
        );
        assert_eq!(
            DatabaseAdapter::encode_segment("resistors?evil"),
            "resistors%3Fevil"
        );
        assert_eq!(DatabaseAdapter::encode_segment("a/b#c"), "a%2Fb%23c");
    }

    #[test]
    fn encode_segment_handles_utf8() {
        // Multi-byte UTF-8 must escape every byte.
        assert_eq!(DatabaseAdapter::encode_segment("π"), "%CF%80");
    }
}
