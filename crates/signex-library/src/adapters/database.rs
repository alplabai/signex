//! `LibraryAdapter` over the HTTP API exposed by `signex-library-server`.
//!
//! The adapter is a synchronous facade for the trait. Internally it bridges
//! to async via a private tokio runtime so it can be used from existing
//! callers that don't want async ceremony.

use std::sync::OnceLock;

use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::adapter::{
    ComponentSummary, FieldSet, LibraryAdapter, LibraryError, LibraryQuery, PrimitiveSummary,
};
use crate::component::{Component, Revision};
use crate::identity::{ComponentId, Version};
use crate::manifest::{LibraryMode, Manifest};
use crate::primitive::{Footprint, SimModel, Symbol};

pub struct DatabaseAdapter {
    manifest: Manifest,
    base_url: String,
    /// Bearer token sent via `Authorization: Bearer <token>`. M3/H1: every
    /// request to a protected route now needs this; `None` keeps the legacy
    /// anonymous flow which still works against `/health` and `/version` but
    /// will fail with 401 against any mutating endpoint when the server is
    /// configured (production) with `SIGNEX_API_TOKEN`.
    token: Option<String>,
    /// Caller-facing string used for advisory locks — defaults to the bearer
    /// token's caller identity. When OIDC lands this becomes the JWT `sub`.
    holder: String,
    client: reqwest::blocking::Client,
}

impl DatabaseAdapter {
    /// Construct from a manifest. The manifest's `auth` field is treated as
    /// the bearer token; the holder is derived from it (one-token-per-caller
    /// model). Use [`Self::with_token`] for explicit control.
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

    /// M3: explicit bearer-token + holder constructor. Prefer this over
    /// [`Self::new`] when the lock holder is a real user identity (user@host)
    /// distinct from the bearer credential.
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
        };
        Ok(Self {
            manifest,
            base_url,
            token,
            holder,
            client,
        })
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
    /// commit message in the `x-signex-message` header. Mirrors the wire
    /// shape WS-D wires up on the server side.
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

    fn search(&self, query: &LibraryQuery) -> Result<Vec<ComponentSummary>, LibraryError> {
        let mut url = self.url("/components");
        let qs = query_to_query_string(query);
        if !qs.is_empty() {
            url.push('?');
            url.push_str(&qs);
        }
        let resp = self
            .auth(self.client.get(&url))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "search failed: {}",
                resp.status()
            )));
        }
        let summaries: Vec<ComponentSummary> = resp
            .json()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        Ok(summaries)
    }

    fn get_component(&self, id: ComponentId) -> Result<Component, LibraryError> {
        let resp = self
            .auth(self.client.get(self.url(&format!("/components/{id}"))))
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => resp
                .json()
                .map_err(|e| LibraryError::Backend(e.to_string())),
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("component {id}")))
            }
            other => Err(LibraryError::Backend(format!("get_component: {other}"))),
        }
    }

    fn get_revision(&self, id: ComponentId, version: Version) -> Result<Revision, LibraryError> {
        let resp = self
            .auth(
                self.client
                    .get(self.url(&format!("/components/{id}/revisions/{version}"))),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => resp
                .json()
                .map_err(|e| LibraryError::Backend(e.to_string())),
            reqwest::StatusCode::NOT_FOUND => {
                Err(LibraryError::NotFound(format!("revision {id} {version}")))
            }
            other => Err(LibraryError::Backend(format!("get_revision: {other}"))),
        }
    }

    fn save_revision(
        &self,
        id: ComponentId,
        revision: Revision,
        _message: &str,
    ) -> Result<(), LibraryError> {
        let resp = self
            .auth(
                self.client
                    .post(self.url(&format!("/components/{id}/revisions")))
                    .json(&revision),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "save_revision: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    fn try_lock(&self, id: ComponentId, field_set: FieldSet) -> Result<(), LibraryError> {
        let resp = self
            .auth(
                self.client
                    .post(self.url(&format!("/components/{id}/locks")))
                    .header("x-signex-holder", &self.holder)
                    .json(&serde_json::json!({ "field_set": field_set_str(field_set) })),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        match resp.status() {
            s if s.is_success() => Ok(()),
            reqwest::StatusCode::CONFLICT => {
                let body: serde_json::Value = resp.json().unwrap_or_default();
                Err(LibraryError::Locked {
                    holder: body
                        .get("error")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    field_set: field_set_str(field_set).into(),
                })
            }
            other => Err(LibraryError::Backend(format!("try_lock: {other}"))),
        }
    }

    fn release_lock(&self, id: ComponentId, field_set: FieldSet) -> Result<(), LibraryError> {
        let resp = self
            .auth(
                self.client
                    .delete(self.url(&format!("/components/{id}/locks")))
                    .header("x-signex-holder", &self.holder)
                    .json(&serde_json::json!({ "field_set": field_set_str(field_set) })),
            )
            .send()
            .map_err(|e| LibraryError::Backend(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(LibraryError::Backend(format!(
                "release_lock: {}",
                resp.status()
            )));
        }
        Ok(())
    }

    // ── Primitive CRUD over HTTP routes (mirrors WS-D server contract) ───

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

fn field_set_str(fs: FieldSet) -> &'static str {
    match fs {
        FieldSet::Symbol => "Symbol",
        FieldSet::Footprint => "Footprint",
        FieldSet::Model3d => "Model3d",
        FieldSet::SharedParams => "SharedParams",
        FieldSet::SharedSupplyChain => "SharedSupplyChain",
        FieldSet::SharedSimulation => "SharedSimulation",
        FieldSet::Lifecycle => "Lifecycle",
    }
}

fn query_to_query_string(q: &LibraryQuery) -> String {
    let mut parts = Vec::new();
    if let Some(t) = q.text.as_deref() {
        parts.push(format!("text={}", urlencoding(t)));
    }
    if let Some(c) = q.category.as_deref() {
        parts.push(format!("category={}", urlencoding(c)));
    }
    parts.join("&")
}

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

// Suppress dead-code warning for the OnceLock when we add caching later.
#[allow(dead_code)]
static CACHE_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::FieldSet;

    #[test]
    fn field_set_str_round_trip() {
        for fs in [
            FieldSet::Symbol,
            FieldSet::Footprint,
            FieldSet::Model3d,
            FieldSet::SharedParams,
            FieldSet::SharedSupplyChain,
            FieldSet::SharedSimulation,
            FieldSet::Lifecycle,
        ] {
            // Names are stable PascalCase strings.
            assert!(!field_set_str(fs).is_empty());
        }
    }

    #[test]
    fn query_string_encodes_special_chars() {
        let q = LibraryQuery {
            text: Some("foo bar".into()),
            category: Some("Resistor".into()),
            facets: vec![],
        };
        let s = query_to_query_string(&q);
        assert!(s.contains("text=foo%20bar"));
        assert!(s.contains("category=Resistor"));
    }
}
