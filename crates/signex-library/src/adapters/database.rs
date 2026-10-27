//! `LibraryAdapter` over the HTTP API exposed by `signex-library-server`.
//!
//! Synchronous facade for the trait. Row CRUD speaks to the
//! `/tables` / `/rows` routes; primitives (`/symbols` / `/footprints`
//! / `/sims`) are created with `POST` and replaced with `PUT` +
//! `If-Match`, the same revision scheme as rows. The base revision travels
//! with the caller's copy ([`Versioned`]), never in a cache on the adapter.
//!
//! Routes are addressed by a `library_id` query parameter — the
//! adapter sources its own from `manifest().library.library_id`.
//! Mutating calls carry their commit message in the
//! `x-signex-message` header so the server-side audit log has it
//! (the DB backend doesn't have its own commit graph the way
//! `LocalGitAdapter` does — see TODO around the `audit_log` table
//! below).

use std::time::Duration;

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use super::database_row_schema::{RowSchemaGuard, schema_refusal};
use crate::adapter::{LibraryAdapter, LibraryError, PrimitiveSummary, Versioned};
use crate::component::{ComponentRow, ROW_SCHEMA, ROW_SCHEMA_HEADER};
use crate::identity::{InternalPn, RowId};
use crate::library_file::SnxlibManifest;
use crate::manifest::{LibraryMeta, LibraryMode, Manifest};
use crate::primitive::{Footprint, PrimitiveKind, SimModel, Symbol};

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
    client: reqwest::blocking::Client,
    /// Row-schema write guard; see `database_row_schema`.
    row_schema: RowSchemaGuard,
}

/// Manifest `auth` prefix that names an OS-keychain item instead of a token.
const KEYCHAIN_SIGIL: &str = "@signex-keychain:";

/// Refuse to send a bearer token over cleartext HTTP to a non-loopback host.
/// Loopback http (local dev server) and any https URL pass.
fn check_transport_security(base_url: &str, has_token: bool) -> Result<(), LibraryError> {
    if !has_token {
        return Ok(());
    }
    let url = url::Url::parse(base_url)
        .map_err(|e| LibraryError::Backend(format!("invalid library server url: {e}")))?;
    let loopback = match url.host() {
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    };
    if url.scheme() == "https" || (url.scheme() == "http" && loopback) {
        Ok(())
    } else {
        Err(LibraryError::Backend(format!(
            "refusing to send the bearer token to `{}://{}{}` over cleartext HTTP; \
             use https:// (or a loopback address for local development)",
            url.scheme(),
            url.host_str().unwrap_or("?"),
            url.port().map(|p| format!(":{p}")).unwrap_or_default(),
        )))
    }
}

/// Whether a redirect hop may be followed: reqwest only strips
/// `Authorization` when host or port changes, so an `https` -> `http`
/// downgrade on the same host:port would resend the bearer token in
/// cleartext. Apply the same transport rule to every hop.
fn redirect_hop_allowed(target: &url::Url, has_token: bool) -> bool {
    check_transport_security(target.as_str(), has_token).is_ok()
}

/// Redirect policy for the library client: at most 5 hops, each of which
/// must satisfy [`redirect_hop_allowed`].
fn redirect_policy(has_token: bool) -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(move |attempt| {
        if attempt.previous().len() >= 5 {
            attempt.error("too many redirects")
        } else if redirect_hop_allowed(attempt.url(), has_token) {
            attempt.follow()
        } else {
            attempt.error("refusing redirect to a cleartext non-loopback url")
        }
    })
}

/// A server built before labelled documents (#1231) stores the row without
/// them and echoes it back that way. Say so instead of reporting a save
/// that silently lost them.
fn refuse_lost_documents(
    sent: &ComponentRow,
    stored: &ComponentRow,
    table: &str,
) -> Result<(), LibraryError> {
    if sent.documents_lost_in(stored) {
        return Err(LibraryError::Backend(format!(
            "{table}/{}: the library server is too old to store document labels or more \
             than one datasheet; the row was saved without them. Upgrade the library server.",
            sent.row_id
        )));
    }
    Ok(())
}

/// Attach the row's base revision as `If-Match: "<revision>"`. Without one
/// the server answers 428, which [`write_error`] reports.
fn with_base_revision(
    req: reqwest::blocking::RequestBuilder,
    base_revision: Option<u64>,
) -> reqwest::blocking::RequestBuilder {
    match base_revision {
        Some(revision) => req.header(reqwest::header::IF_MATCH, format!("\"{revision}\"")),
        None => req,
    }
}

/// Map a refused row or primitive write to a [`LibraryError`]. A 409
/// carrying `current_revision` means the stored copy moved on since it was
/// read. `subject` names what was written (`row <table>/<id>`,
/// `symbol <uuid>`).
fn write_error(
    resp: reqwest::blocking::Response,
    op: &str,
    subject: &str,
    row_schema: &RowSchemaGuard,
) -> LibraryError {
    match resp.status() {
        reqwest::StatusCode::NOT_FOUND => LibraryError::NotFound(subject.to_string()),
        reqwest::StatusCode::CONFLICT => {
            let body: serde_json::Value = resp.json().unwrap_or_default();
            match body["current_revision"].as_u64() {
                Some(current_revision) => LibraryError::RowChanged { current_revision },
                None => LibraryError::Conflict(format!(
                    "{op} {subject}: {}",
                    body["error"].as_str().unwrap_or("refused by the server")
                )),
            }
        }
        reqwest::StatusCode::BAD_REQUEST => {
            let body: serde_json::Value = resp.json().unwrap_or_default();
            if let Some(refusal) = schema_refusal(&body) {
                // The server changed under a cached check: re-read /version.
                row_schema.forget();
                return refusal;
            }
            LibraryError::Backend(format!(
                "{op} {subject}: {}",
                body["error"].as_str().unwrap_or("400 Bad Request")
            ))
        }
        reqwest::StatusCode::PRECONDITION_REQUIRED => LibraryError::Backend(format!(
            "{op} {subject}: the server requires the revision this copy was read at (If-Match); \
             reload it and save again"
        )),
        other => LibraryError::Backend(format!("{op} {subject}: {other}")),
    }
}

impl DatabaseAdapter {
    /// Construct from a manifest. The manifest's `auth` field is treated as
    /// the bearer token.
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
        if auth.starts_with(KEYCHAIN_SIGIL) {
            return Err(LibraryError::Backend(format!(
                "library auth `{KEYCHAIN_SIGIL}...` is not resolved by the database adapter yet; \
             refusing to send the sigil as a bearer token (OS keychain resolution: #976)"
            )));
        }
        check_transport_security(&base_url, !auth.is_empty())?;
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .redirect(redirect_policy(!auth.is_empty()))
            .build()
            .map_err(|e| LibraryError::Backend(format!("reqwest client: {e}")))?;
        let token = if auth.is_empty() { None } else { Some(auth) };
        Ok(Self {
            manifest,
            base_url,
            token,
            client,
            row_schema: RowSchemaGuard::default(),
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
        check_transport_security(&base_url, !token.is_empty())?;
        let token = if token.is_empty() { None } else { Some(token) };
        let holder = holder.into();
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(HTTP_CONNECT_TIMEOUT)
            .timeout(HTTP_REQUEST_TIMEOUT)
            .redirect(redirect_policy(token.is_some()))
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
            client,
            row_schema: RowSchemaGuard::default(),
        })
    }

    /// Borrow the configured base URL.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// A row write that never got an answer: forget the cached schema
    /// check, the server behind the connection may have changed.
    fn row_write_transport_error(&self, e: reqwest::Error) -> LibraryError {
        self.row_schema.forget();
        LibraryError::Backend(e.to_string())
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

    /// Generic GET → JSON for a primitive at `/{collection}/{uuid}` in this
    /// library, with the revision from its `ETag`. `collection` is
    /// percent-encoded; `uuid` is hex (Display) so it is already URL-safe.
    fn get_primitive_json<T: DeserializeOwned>(
        &self,
        collection: &str,
        uuid: Uuid,
        kind_label: &str,
    ) -> Result<Versioned<T>, LibraryError> {
        let coll = Self::encode_segment(collection);
        let resp = self
            .auth(
                self.client
                    .get(self.url(&format!("/{coll}/{uuid}?{}", self.library_id_query()))),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => {
                let revision = primitive_revision(&resp, "get", kind_label, uuid)?;
                let value = resp
                    .json::<T>()
                    .map_err(|e| LibraryError::Backend(e.to_string()))?;
                Ok(Versioned {
                    value,
                    revision: Some(revision),
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("{kind_label} {uuid}")))
            }
            other => Err(LibraryError::Backend(format!("get {kind_label}: {other}"))),
        }
    }

    /// Save a primitive and return the revision it now has. With the `base`
    /// the caller's copy was read at, it replaces the primitive with `PUT` +
    /// `If-Match`, refused with [`LibraryError::RowChanged`] when someone
    /// saved it since. Without one it creates it with `POST`, refused with
    /// [`LibraryError::Conflict`] when the uuid already exists. The body
    /// carries the owning library beside the primitive's own fields, and the
    /// commit message goes in `x-signex-message`.
    fn save_primitive_json<T: Serialize>(
        &self,
        collection: &str,
        kind_label: &str,
        uuid: Uuid,
        body: &T,
        base: Option<u64>,
        message: &str,
    ) -> Result<u64, LibraryError> {
        let coll = Self::encode_segment(collection);
        let payload = InLibrary {
            library_id: self.manifest.library.library_id,
            primitive: body,
        };
        let req = match base {
            Some(_) => {
                with_base_revision(self.client.put(self.url(&format!("/{coll}/{uuid}"))), base)
            }
            None => self.client.post(self.url(&format!("/{coll}"))),
        };
        let resp = self
            .auth(req.header("x-signex-message", message).json(&payload))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(write_error(
                resp,
                "save",
                &format!("{kind_label} {uuid}"),
                &self.row_schema,
            ));
        }
        primitive_revision(&resp, "save", kind_label, uuid)
    }

    /// Generic GET → list at `/{collection}` returning this library's
    /// [`PrimitiveSummary`]s. Without `library_id` the server lists every
    /// library's primitives. The server's rows carry no kind, so each takes
    /// the collection's `kind`.
    fn list_primitives_json(
        &self,
        collection: &str,
        kind: PrimitiveKind,
    ) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        let coll = Self::encode_segment(collection);
        let resp = self
            .auth(
                self.client
                    .get(self.url(&format!("/{coll}?{}", self.library_id_query()))),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "list {collection}: {}",
                resp.status()
            )));
        }
        let listed = resp
            .json::<Vec<ListedPrimitive>>()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        Ok(listed
            .into_iter()
            .map(|row| PrimitiveSummary {
                uuid: row.uuid,
                name: row.name,
                kind,
                used_by_count: 0,
            })
            .collect())
    }

    /// `library_id` query string segment used by every row, table and
    /// primitive read. The server keys storage by `library_id`; a missing or
    /// wrong id surfaces as a 404 from the route, which we map to `NotFound`
    /// at the call site.
    fn library_id_query(&self) -> String {
        format!("library_id={}", self.manifest.library.library_id)
    }
}

/// The revision a successful primitive reply carries in its `ETag`. A
/// server that sends none predates primitive revisions and treats a `POST`
/// as an upsert, so a save through it could silently replace someone
/// else's version: refuse to go on, as the row-schema guard does for rows.
fn primitive_revision(
    resp: &reqwest::blocking::Response,
    op: &str,
    kind_label: &str,
    uuid: Uuid,
) -> Result<u64, LibraryError> {
    etag_revision(resp).ok_or_else(|| {
        LibraryError::Backend(format!(
            "{op} {kind_label} {uuid}: the library server is too old for this signex (it sends \
             no primitive revision, so a save could overwrite someone else's version); upgrade \
             signex and the library server together"
        ))
    })
}

/// The revision in a response's `ETag: "<revision>"` header, if any.
fn etag_revision(resp: &reqwest::blocking::Response) -> Option<u64> {
    resp.headers()
        .get(reqwest::header::ETAG)?
        .to_str()
        .ok()?
        .trim()
        .strip_prefix('"')?
        .strip_suffix('"')?
        .parse()
        .ok()
}

/// One row of the server's primitive list (`GET /{collection}`).
#[derive(Deserialize)]
struct ListedPrimitive {
    uuid: Uuid,
    name: String,
}

/// A primitive's JSON with the owning library beside its fields — the body
/// the server's primitive `POST` and `PUT` routes expect.
#[derive(Serialize)]
struct InLibrary<'a, T> {
    library_id: Uuid,
    #[serde(flatten)]
    primitive: &'a T,
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
        crate::validate_row_for_write(&row)?;
        self.row_schema.check(&self.client, &self.url("/version"))?;
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
                    .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string())
                    .json(&row),
            )
            .send()
            .map_err(|e| self.row_write_transport_error(e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body: serde_json::Value = resp.json().unwrap_or_default();
            if let Some(refusal) = schema_refusal(&body) {
                self.row_schema.forget();
                return Err(refusal);
            }
            return Err(LibraryError::Backend(format!(
                "insert_row {table}: {status}"
            )));
        }
        let stored = resp
            .json::<ComponentRow>()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        refuse_lost_documents(&row, &stored, table)
    }

    fn update_row(
        &self,
        table: &str,
        row: ComponentRow,
        msg: &str,
    ) -> Result<Option<u64>, LibraryError> {
        crate::validate_row_for_write(&row)?;
        self.row_schema.check(&self.client, &self.url("/version"))?;
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
        let req = self
            .client
            .put(url)
            .header("x-signex-message", msg)
            .header(ROW_SCHEMA_HEADER, ROW_SCHEMA.to_string());
        let resp = self
            .auth(with_base_revision(req, row.server_revision).json(&row))
            .send()
            .map_err(|e| self.row_write_transport_error(e))?;
        if !resp.status().is_success() {
            return Err(write_error(
                resp,
                "update_row",
                &format!("row {table}/{row_id}"),
                &self.row_schema,
            ));
        }
        let stored = resp
            .json::<ComponentRow>()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        refuse_lost_documents(&row, &stored, table)?;
        match stored.server_revision {
            Some(revision) => Ok(Some(revision)),
            None => Err(LibraryError::Backend(format!(
                "update_row {table}/{row_id}: the server reply carries no revision"
            ))),
        }
    }

    fn delete_row(
        &self,
        table: &str,
        row_id: RowId,
        base_revision: Option<u64>,
        msg: &str,
    ) -> Result<(), LibraryError> {
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
        let req = self.client.delete(url).header("x-signex-message", msg);
        let resp = self
            .auth(with_base_revision(req, base_revision))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(write_error(
                resp,
                "delete_row",
                &format!("row {table}/{row_id}"),
                &self.row_schema,
            ));
        }
        Ok(())
    }

    fn get_symbol(&self, uuid: Uuid) -> Result<Symbol, LibraryError> {
        self.get_symbol_versioned(uuid).map(|read| read.value)
    }

    fn get_footprint(&self, uuid: Uuid) -> Result<Footprint, LibraryError> {
        self.get_footprint_versioned(uuid).map(|read| read.value)
    }

    fn get_sim(&self, uuid: Uuid) -> Result<SimModel, LibraryError> {
        self.get_sim_versioned(uuid).map(|read| read.value)
    }

    fn get_symbol_versioned(&self, uuid: Uuid) -> Result<Versioned<Symbol>, LibraryError> {
        self.get_primitive_json("symbols", uuid, "symbol")
    }

    fn get_footprint_versioned(&self, uuid: Uuid) -> Result<Versioned<Footprint>, LibraryError> {
        let mut read = self.get_primitive_json::<Footprint>("footprints", uuid, "footprint")?;
        read.value.repair_stored_sketch();
        Ok(read)
    }

    fn get_sim_versioned(&self, uuid: Uuid) -> Result<Versioned<SimModel>, LibraryError> {
        self.get_primitive_json("sims", uuid, "sim")
    }

    /// Creates the symbol; to replace one, read it with
    /// [`get_symbol_versioned`](LibraryAdapter::get_symbol_versioned) and
    /// save with its revision.
    fn save_symbol(&self, sym: Symbol, message: &str) -> Result<(), LibraryError> {
        self.save_symbol_versioned(sym, None, message).map(drop)
    }

    /// Creates the footprint; see [`Self::save_symbol`].
    fn save_footprint(&self, fp: Footprint, message: &str) -> Result<(), LibraryError> {
        self.save_footprint_versioned(fp, None, message).map(drop)
    }

    /// Creates the sim model; see [`Self::save_symbol`].
    fn save_sim(&self, sm: SimModel, message: &str) -> Result<(), LibraryError> {
        self.save_sim_versioned(sm, None, message).map(drop)
    }

    fn save_symbol_versioned(
        &self,
        sym: Symbol,
        base: Option<u64>,
        message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.save_primitive_json("symbols", "symbol", sym.uuid, &sym, base, message)
            .map(Some)
    }

    fn save_footprint_versioned(
        &self,
        fp: Footprint,
        base: Option<u64>,
        message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.save_primitive_json("footprints", "footprint", fp.uuid, &fp, base, message)
            .map(Some)
    }

    fn save_sim_versioned(
        &self,
        sm: SimModel,
        base: Option<u64>,
        message: &str,
    ) -> Result<Option<u64>, LibraryError> {
        self.save_primitive_json("sims", "sim", sm.uuid, &sm, base, message)
            .map(Some)
    }

    fn list_symbols(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitives_json("symbols", PrimitiveKind::Symbol)
    }

    fn list_footprints(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitives_json("footprints", PrimitiveKind::Footprint)
    }

    fn list_sims(&self) -> Result<Vec<PrimitiveSummary>, LibraryError> {
        self.list_primitives_json("sims", PrimitiveKind::Sim)
    }
}

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
mod tests;
