//! `LibraryAdapter` over the HTTP API exposed by `signex-library-server`.
//!
//! The adapter is a synchronous facade for the trait. Per
//! `v0.9-refactor-2-plan.md` §8, WS-3 will replace this with row CRUD
//! against the new `/tables` / `/rows` routes. WS-1 keeps just the
//! primitive CRUD wiring intact (still correct under the row model);
//! component / revision routes are gone.

use std::sync::OnceLock;

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::adapter::{LibraryAdapter, LibraryError, PrimitiveSummary};
use crate::manifest::{LibraryMode, Manifest};
use crate::primitive::{Footprint, SimModel, Symbol};

pub struct DatabaseAdapter {
    manifest: Manifest,
    base_url: String,
    /// Bearer token sent via `Authorization: Bearer <token>`. WS-3 may
    /// extend this; for now it's a static credential.
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

    /// Apply the `Authorization: Bearer <token>` header when configured.
    fn auth(&self, req: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if let Some(token) = &self.token {
            req.header("authorization", format!("Bearer {token}"))
        } else {
            req
        }
    }

    /// Generic GET → JSON for a primitive at `/{collection}/{uuid}`.
    fn get_primitive_json<T: DeserializeOwned>(
        &self,
        collection: &str,
        uuid: Uuid,
        kind_label: &str,
    ) -> Result<T, LibraryError> {
        let resp = self
            .auth(self.client.get(self.url(&format!("/{collection}/{uuid}"))))
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
        let resp = self
            .auth(
                self.client
                    .post(self.url(&format!("/{collection}")))
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
        let resp = self
            .auth(self.client.get(self.url(&format!("/{collection}"))))
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
}

impl LibraryAdapter for DatabaseAdapter {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    // Row CRUD lands in WS-3; until then the trait defaults
    // (`Backend("not impl")`) cover every row method so the adapter still
    // satisfies the trait shape.

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
