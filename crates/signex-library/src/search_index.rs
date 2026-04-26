//! Tantivy-backed implementation of [`SearchIndex`] (Phase 1 WS-E).
//!
//! Schema:
//! - `uuid`         STRING | STORED  (primary key — used for delete-then-add updates)
//! - `internal_pn`  TEXT   | STORED
//! - `mpn`          TEXT   | STORED
//! - `manufacturer` TEXT   | STORED
//! - `description`  TEXT   | STORED
//! - `category`     STRING | STORED  (exact-match facet; surfaced from the
//!   shared parameter `category`)
//! - `head_major`, `head_minor`  u64 INDEXED | STORED | FAST
//! - `state`        STRING | STORED  (`LifecycleState` JSON name)
//! - `parameters`   JSON   | STORED | TEXT  (text parameters; supports
//!   `parameters.dielectric:X7R`-style equality/contains queries via the
//!   `QueryParser`)
//! - **`param_<key>`** f64  INDEXED | STORED | FAST  (one per well-known
//!   numeric parameter key — see [`NUMERIC_PARAM_KEYS`]; supports `RangeQuery`)
//!
//! Tantivy 0.22 does **not** support range queries on JSON-field subpaths via
//! the QueryParser, so each numeric parameter key gets its own typed f64
//! column in the schema. Adding a new numeric param key requires extending
//! [`NUMERIC_PARAM_KEYS`] and rebuilding the index from scratch (the schema
//! check at [`TantivySearchIndex::open`] catches mismatches).
//!
//! Persisted to whatever directory the caller passes — typically
//! `<snxlib>/index/search.tantivy/`.

use std::collections::{BTreeMap, HashMap};
use std::ops::Bound;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, BooleanQuery, Occur, Query, QueryParser, RangeQuery, TermQuery};
use tantivy::schema::{
    FAST, Field, INDEXED, IndexRecordOption, JsonObjectOptions, OwnedValue, STORED, STRING, Schema,
    TantivyDocument, TextFieldIndexing, TextOptions, Value,
};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyError, Term};

use crate::adapter::ComponentSummary;
use crate::component::Component;
use crate::embed::ParamValue;
use crate::identity::{InternalPn, Version};
use crate::lifecycle::LifecycleState;
use crate::search::{Facet, FacetOp, SearchIndex, SearchQuery};

/// Well-known numeric parameter keys promoted to dedicated `f64` Tantivy
/// fields. Range/Lt/Gt queries on `parameters.<key>` route through these.
///
/// Adding a new key here is a schema-breaking change — existing indexes built
/// without the new key will be detected at [`TantivySearchIndex::open`] and
/// must be rebuilt. Order is irrelevant.
pub const NUMERIC_PARAM_KEYS: &[&str] = &[
    // Passive components
    "capacitance",
    "resistance",
    "inductance",
    "tolerance_pct",
    // Power
    "voltage",
    "current",
    "power",
    // Frequency / timing
    "frequency",
    "time_ns",
    // Misc
    "rating",
    "temperature_min",
    "temperature_max",
];

/// Default writer heap (50 MB) — tantivy's recommended minimum.
const WRITER_HEAP_BYTES: usize = 50_000_000;

#[derive(Debug, thiserror::Error)]
pub enum TantivyIndexError {
    #[error("tantivy: {0}")]
    Tantivy(#[from] TantivyError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("directory open: {0}")]
    OpenDirectory(#[from] tantivy::directory::error::OpenDirectoryError),
    #[error("directory read: {0}")]
    OpenRead(#[from] tantivy::directory::error::OpenReadError),
    /// M10: schema drift recovery hint — see `wipe_and_recreate`.
    #[error("schema mismatch on existing index at {path}: {reason}")]
    SchemaMismatch { path: PathBuf, reason: String },
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("invalid facet value for {field}: {value} ({reason})")]
    InvalidFacetValue {
        field: String,
        value: String,
        reason: String,
    },
    /// M8: catch-all for non-query, non-IO library errors that operators need
    /// to see — currently used for poisoned mutexes (writer lock guard) which
    /// were previously misreported as `InvalidQuery` and skewed telemetry.
    #[error("internal index error: {0}")]
    Internal(String),
}

#[derive(Clone)]
struct SchemaFields {
    uuid: Field,
    internal_pn: Field,
    mpn: Field,
    manufacturer: Field,
    description: Field,
    category: Field,
    head_major: Field,
    head_minor: Field,
    state: Field,
    parameters: Field,
    /// `key -> f64 field` for every known numeric parameter.
    numeric_params: HashMap<String, Field>,
}

impl SchemaFields {
    fn build() -> (Schema, SchemaFields) {
        let mut b = Schema::builder();

        let uuid = b.add_text_field("uuid", STRING | STORED);

        let text_opts = TextOptions::default().set_stored().set_indexing_options(
            TextFieldIndexing::default()
                .set_tokenizer("default")
                .set_index_option(IndexRecordOption::WithFreqsAndPositions),
        );
        let internal_pn = b.add_text_field("internal_pn", text_opts.clone());
        let mpn = b.add_text_field("mpn", text_opts.clone());
        let manufacturer = b.add_text_field("manufacturer", text_opts.clone());
        let description = b.add_text_field("description", text_opts);

        let category = b.add_text_field("category", STRING | STORED);

        let head_major = b.add_u64_field("head_major", INDEXED | STORED | FAST);
        let head_minor = b.add_u64_field("head_minor", INDEXED | STORED | FAST);

        let state = b.add_text_field("state", STRING | STORED);

        // JSON field for free-form text/bool/measurement params.
        let json_opts = JsonObjectOptions::default()
            .set_stored()
            .set_indexing_options(
                TextFieldIndexing::default()
                    .set_tokenizer("default")
                    .set_index_option(IndexRecordOption::WithFreqsAndPositions),
            )
            .set_expand_dots_enabled();
        let parameters = b.add_json_field("parameters", json_opts);

        // Dedicated f64 columns per well-known numeric param key.
        let mut numeric_params = HashMap::new();
        for key in NUMERIC_PARAM_KEYS {
            let field_name = numeric_param_field_name(key);
            let field = b.add_f64_field(&field_name, INDEXED | STORED | FAST);
            numeric_params.insert((*key).to_string(), field);
        }

        let schema = b.build();
        (
            schema,
            SchemaFields {
                uuid,
                internal_pn,
                mpn,
                manufacturer,
                description,
                category,
                head_major,
                head_minor,
                state,
                parameters,
                numeric_params,
            },
        )
    }
}

/// Canonical Tantivy field name for a numeric parameter key.
fn numeric_param_field_name(key: &str) -> String {
    format!("param_num_{key}")
}

/// Tantivy-backed [`SearchIndex`].
pub struct TantivySearchIndex {
    index: Index,
    fields: SchemaFields,
    reader: IndexReader,
    writer: Mutex<IndexWriter>,
}

impl TantivySearchIndex {
    /// Open or create a Tantivy index rooted at `path`.
    ///
    /// - If `path` already contains a Tantivy index, it is reopened.
    /// - Otherwise the directory is created and a fresh index is initialised
    ///   with the schema described in the module docs.
    /// - If an existing index has a different schema than the current
    ///   [`NUMERIC_PARAM_KEYS`] would produce, [`TantivyIndexError::SchemaMismatch`]
    ///   is returned.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, TantivyIndexError> {
        let path = path.as_ref();
        std::fs::create_dir_all(path)?;

        let dir = tantivy::directory::MmapDirectory::open(path)?;
        let (schema, fields) = SchemaFields::build();

        let index = if Index::exists(&dir)? {
            let existing = Index::open(dir)?;
            if existing.schema() != schema {
                return Err(TantivyIndexError::SchemaMismatch {
                    path: path.to_path_buf(),
                    reason: "existing index schema differs from current".into(),
                });
            }
            existing
        } else {
            Index::open_or_create(dir, schema.clone())?
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let writer = index.writer(WRITER_HEAP_BYTES)?;

        Ok(Self {
            index,
            fields,
            reader,
            writer: Mutex::new(writer),
        })
    }

    /// M10: nuke the on-disk index at `path` and re-open with the current
    /// schema. The canonical recovery flow when [`TantivyIndexError::SchemaMismatch`]
    /// is returned by [`TantivySearchIndex::open`]:
    ///
    /// ```ignore
    /// match TantivySearchIndex::open(&p) {
    ///     Ok(idx) => idx,
    ///     Err(TantivyIndexError::SchemaMismatch { .. }) => {
    ///         TantivySearchIndex::wipe_and_recreate(&p)?
    ///         // caller must re-ingest every component
    ///     }
    ///     Err(e) => return Err(e.into()),
    /// }
    /// ```
    ///
    /// Wipes only the directory contents — the directory itself is reused.
    /// Callers are responsible for re-populating the index from the canonical
    /// component store; this helper does **not** rebuild documents.
    pub fn wipe_and_recreate(path: impl AsRef<Path>) -> Result<Self, TantivyIndexError> {
        let path = path.as_ref();
        if path.exists() {
            // Walk one level deep — Tantivy stores its files flat under
            // `path`, and we must not nuke the directory itself in case a
            // caller has a file watcher pinned to it.
            for entry in std::fs::read_dir(path)? {
                let entry = entry?;
                let p = entry.path();
                if p.is_dir() {
                    std::fs::remove_dir_all(&p)?;
                } else {
                    std::fs::remove_file(&p)?;
                }
            }
        }
        Self::open(path)
    }

    fn writer(&self) -> Result<MutexGuard<'_, IndexWriter>, TantivyIndexError> {
        // M8: a poisoned mutex is an internal failure, not a malformed query —
        // route it through `Internal` so operator dashboards can distinguish
        // user errors from process-level corruption.
        self.writer
            .lock()
            .map_err(|_| TantivyIndexError::Internal("writer mutex poisoned".into()))
    }

    /// Add or replace the doc for a single component.
    ///
    /// Replacement is by `uuid` term; safe to call on the same component
    /// repeatedly. Caller must `commit()` to make changes visible to
    /// subsequent queries.
    pub fn add_or_update(&self, component: &Component) -> Result<(), TantivyIndexError> {
        let writer = self.writer()?;

        // Delete any existing doc with this uuid; harmless on first insert.
        let uuid_str = component.uuid.to_string();
        let term = Term::from_field_text(self.fields.uuid, &uuid_str);
        writer.delete_term(term);

        let Some(head) = component.head_revision() else {
            return Ok(());
        };

        // Surface `category` parameter into a dedicated facet field if present.
        let category_value = head
            .shared
            .parameters
            .get("category")
            .and_then(|v| match v {
                ParamValue::Text(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let mut doc = TantivyDocument::default();
        doc.add_text(self.fields.uuid, &uuid_str);
        doc.add_text(self.fields.internal_pn, component.internal_pn.as_str());
        doc.add_text(self.fields.mpn, &head.shared.mpn);
        doc.add_text(self.fields.manufacturer, &head.shared.manufacturer);
        doc.add_text(self.fields.description, &head.shared.description);
        doc.add_text(self.fields.category, &category_value);
        doc.add_u64(self.fields.head_major, component.head.major as u64);
        doc.add_u64(self.fields.head_minor, component.head.minor as u64);
        doc.add_text(self.fields.state, lifecycle_token(head.state));

        // Push numeric values into their dedicated f64 columns; non-numeric
        // values fall through to the JSON blob.
        let mut json_blob: BTreeMap<String, OwnedValue> = BTreeMap::new();
        for (k, v) in &head.shared.parameters {
            if k == "category" {
                continue;
            }
            if let Some(field) = self.fields.numeric_params.get(k) {
                let n = match v {
                    ParamValue::Number(n) => Some(*n),
                    ParamValue::Measurement { value, .. } => Some(*value),
                    _ => None,
                };
                if let Some(n) = n {
                    doc.add_f64(*field, n);
                    continue;
                }
            }
            // Non-numeric or non-registered numeric → JSON blob.
            let owned: OwnedValue = match v {
                ParamValue::Text(s) => OwnedValue::Str(s.clone()),
                ParamValue::Number(n) => OwnedValue::F64(*n),
                ParamValue::Bool(b) => OwnedValue::Bool(*b),
                ParamValue::Measurement { value, .. } => OwnedValue::F64(*value),
            };
            json_blob.insert(k.clone(), owned);
        }
        if !json_blob.is_empty() {
            doc.add_object(self.fields.parameters, json_blob);
        }

        writer.add_document(doc)?;
        Ok(())
    }

    /// Flush pending writes; required before queries see new docs.
    pub fn commit(&self) -> Result<(), TantivyIndexError> {
        let mut writer = self.writer()?;
        writer.commit()?;
        // Pick up the new commit immediately.
        self.reader.reload()?;
        Ok(())
    }

    fn build_query(&self, q: &SearchQuery) -> Result<Box<dyn Query>, TantivyIndexError> {
        let mut clauses: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        if let Some(text) = &q.text {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let parser = QueryParser::for_index(
                    &self.index,
                    vec![
                        self.fields.internal_pn,
                        self.fields.mpn,
                        self.fields.manufacturer,
                        self.fields.description,
                    ],
                );
                match parser.parse_query(trimmed) {
                    Ok(parsed) => clauses.push((Occur::Must, parsed)),
                    Err(_) => clauses.push((Occur::Must, Box::new(AllQuery))),
                }
            }
        }

        if let Some(cat) = &q.category
            && !cat.is_empty()
        {
            let term = Term::from_field_text(self.fields.category, cat);
            let term_q: Box<dyn Query> = Box::new(TermQuery::new(term, IndexRecordOption::Basic));
            clauses.push((Occur::Must, term_q));
        }

        for facet in &q.facets {
            let q = self.facet_to_query(facet)?;
            clauses.push((Occur::Must, q));
        }

        if clauses.is_empty() {
            return Ok(Box::new(AllQuery));
        }
        // H6: collapse the `len == 1` and `into_iter().next().unwrap()` pair
        // into a single shape that is panic-free even if a future refactor
        // drops the length guard. `unwrap_or_else` returns the same `AllQuery`
        // sentinel as the empty branch above, preserving observable behaviour
        // while making the invariant invisible-to-the-compiler explicit.
        if clauses.len() == 1 {
            let q = clauses
                .into_iter()
                .next()
                .map(|(_, q)| q)
                .unwrap_or_else(|| Box::new(AllQuery));
            return Ok(q);
        }
        Ok(Box::new(BooleanQuery::new(clauses)))
    }

    fn facet_to_query(&self, facet: &Facet) -> Result<Box<dyn Query>, TantivyIndexError> {
        // Top-level dedicated fields take precedence.
        if let Some(field) = self.top_level_facet_field(&facet.field) {
            return self.top_level_facet_query(field, &facet.field, facet);
        }

        // `parameters.<key>` — try the dedicated numeric f64 column, fall
        // back to the JSON blob for text params.
        let key = facet
            .field
            .strip_prefix("parameters.")
            .unwrap_or(&facet.field);

        if let Some(numeric_field) = self.fields.numeric_params.get(key) {
            return self.numeric_param_query(*numeric_field, key, facet);
        }

        self.json_param_query(key, facet)
    }

    fn top_level_facet_field(&self, name: &str) -> Option<Field> {
        match name {
            "internal_pn" => Some(self.fields.internal_pn),
            "mpn" => Some(self.fields.mpn),
            "manufacturer" => Some(self.fields.manufacturer),
            "description" => Some(self.fields.description),
            "category" => Some(self.fields.category),
            "state" => Some(self.fields.state),
            _ => None,
        }
    }

    fn top_level_facet_query(
        &self,
        field: Field,
        name: &str,
        facet: &Facet,
    ) -> Result<Box<dyn Query>, TantivyIndexError> {
        match facet.op {
            FacetOp::Eq => {
                let term = Term::from_field_text(field, &facet.value);
                Ok(Box::new(TermQuery::new(term, IndexRecordOption::Basic)))
            }
            FacetOp::Contains => {
                let parser = QueryParser::for_index(&self.index, vec![field]);
                let parsed = parser.parse_query(&facet.value).map_err(|e| {
                    TantivyIndexError::InvalidFacetValue {
                        field: name.into(),
                        value: facet.value.clone(),
                        reason: e.to_string(),
                    }
                })?;
                Ok(parsed)
            }
            FacetOp::Lt | FacetOp::Gt => Err(TantivyIndexError::InvalidFacetValue {
                field: name.into(),
                value: facet.value.clone(),
                reason: "ordering ops only supported on numeric parameters".into(),
            }),
        }
    }

    fn numeric_param_query(
        &self,
        field: Field,
        key: &str,
        facet: &Facet,
    ) -> Result<Box<dyn Query>, TantivyIndexError> {
        let n = parse_f64(facet)?;
        match facet.op {
            FacetOp::Eq => {
                // f64 equality via a single-value [n, n] inclusive range to
                // avoid bit-pattern comparison surprises with `TermQuery`.
                Ok(Box::new(RangeQuery::new_f64_bounds(
                    self.field_name_owned(field),
                    Bound::Included(n),
                    Bound::Included(n),
                )))
            }
            FacetOp::Lt => Ok(Box::new(RangeQuery::new_f64_bounds(
                self.field_name_owned(field),
                Bound::Unbounded,
                Bound::Excluded(n),
            ))),
            FacetOp::Gt => Ok(Box::new(RangeQuery::new_f64_bounds(
                self.field_name_owned(field),
                Bound::Excluded(n),
                Bound::Unbounded,
            ))),
            FacetOp::Contains => Err(TantivyIndexError::InvalidFacetValue {
                field: format!("parameters.{key}"),
                value: facet.value.clone(),
                reason: "Contains is not defined on numeric parameters".into(),
            }),
        }
    }

    fn json_param_query(
        &self,
        key: &str,
        facet: &Facet,
    ) -> Result<Box<dyn Query>, TantivyIndexError> {
        let parser = QueryParser::for_index(&self.index, vec![self.fields.parameters]);
        let escape = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let q_str = match facet.op {
            FacetOp::Eq => {
                if facet.value.parse::<f64>().is_ok() {
                    format!("parameters.{}:{}", key, facet.value)
                } else {
                    format!("parameters.{}:\"{}\"", key, escape(&facet.value))
                }
            }
            FacetOp::Contains => format!("parameters.{}:{}", key, facet.value),
            FacetOp::Lt | FacetOp::Gt => {
                return Err(TantivyIndexError::InvalidFacetValue {
                    field: facet.field.clone(),
                    value: facet.value.clone(),
                    reason: format!(
                        "ordering ops on parameters.{} need a registered numeric field — \
                         add `{}` to NUMERIC_PARAM_KEYS",
                        key, key
                    ),
                });
            }
        };

        parser
            .parse_query(&q_str)
            .map_err(|e| TantivyIndexError::InvalidFacetValue {
                field: facet.field.clone(),
                value: facet.value.clone(),
                reason: e.to_string(),
            })
    }

    fn field_name_owned(&self, field: Field) -> String {
        self.index.schema().get_field_name(field).to_string()
    }

    fn doc_to_summary(&self, doc: &TantivyDocument) -> Option<ComponentSummary> {
        let uuid = read_text(doc, self.fields.uuid).and_then(|s| uuid::Uuid::parse_str(&s).ok())?;
        let internal_pn = read_text(doc, self.fields.internal_pn).unwrap_or_default();
        let mpn = read_text(doc, self.fields.mpn).unwrap_or_default();
        let description = read_text(doc, self.fields.description).unwrap_or_default();
        let major = read_u64(doc, self.fields.head_major).unwrap_or(0) as u32;
        let minor = read_u64(doc, self.fields.head_minor).unwrap_or(0) as u32;
        let state_raw = read_text(doc, self.fields.state).unwrap_or_default();
        let state = parse_lifecycle_token(&state_raw).unwrap_or(LifecycleState::Released);

        Some(ComponentSummary {
            uuid,
            internal_pn: InternalPn::new(internal_pn),
            mpn,
            head: Version::new(major, minor),
            state,
            description,
        })
    }
}

impl SearchIndex for TantivySearchIndex {
    fn query(&self, q: &SearchQuery) -> Vec<ComponentSummary> {
        let limit = if q.limit == 0 { 50 } else { q.limit };

        // M6: no manual reload — `ReloadPolicy::OnCommitWithDelay` (set in
        // `open_internal`) already refreshes the searcher after each commit.
        // The previous `let _ = self.reader.reload();` swallowed errors and
        // doubled the work; trusting the policy keeps reads consistent and
        // surfaces real reload failures via the policy's own logging.
        let searcher = self.reader.searcher();

        let query = match self.build_query(q) {
            Ok(qq) => qq,
            Err(e) => {
                if std::env::var("SIGNEX_TANTIVY_TRACE").is_ok() {
                    eprintln!("[tantivy] build_query error: {e}");
                }
                return Vec::new();
            }
        };

        let top = match searcher.search(&query, &TopDocs::with_limit(limit)) {
            Ok(t) => t,
            Err(e) => {
                if std::env::var("SIGNEX_TANTIVY_TRACE").is_ok() {
                    eprintln!("[tantivy] search error: {e}");
                }
                return Vec::new();
            }
        };

        top.into_iter()
            .filter_map(|(_score, addr)| {
                let doc: TantivyDocument = searcher.doc(addr).ok()?;
                self.doc_to_summary(&doc)
            })
            .collect()
    }
}

// ── helpers ────────────────────────────────────────────────────────────

fn parse_f64(facet: &Facet) -> Result<f64, TantivyIndexError> {
    facet
        .value
        .parse::<f64>()
        .map_err(|e| TantivyIndexError::InvalidFacetValue {
            field: facet.field.clone(),
            value: facet.value.clone(),
            reason: e.to_string(),
        })
}

fn lifecycle_token(s: LifecycleState) -> &'static str {
    match s {
        LifecycleState::Draft => "Draft",
        LifecycleState::InReview => "InReview",
        LifecycleState::Released => "Released",
        LifecycleState::Deprecated => "Deprecated",
        LifecycleState::Obsolete => "Obsolete",
    }
}

fn parse_lifecycle_token(s: &str) -> Option<LifecycleState> {
    Some(match s {
        "Draft" => LifecycleState::Draft,
        "InReview" => LifecycleState::InReview,
        "Released" => LifecycleState::Released,
        "Deprecated" => LifecycleState::Deprecated,
        "Obsolete" => LifecycleState::Obsolete,
        _ => return None,
    })
}

fn read_text(doc: &TantivyDocument, field: Field) -> Option<String> {
    doc.get_first(field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn read_u64(doc: &TantivyDocument, field: Field) -> Option<u64> {
    doc.get_first(field).and_then(|v| v.as_u64())
}
