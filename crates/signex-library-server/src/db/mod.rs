//! Database layer — pool management, migrations, and component/revision
//! persistence helpers used by the route handlers.
//!
//! The pool is a thin enum over SQLite (default for tests + offline) and
//! Postgres (production). Schema is portable across both — see
//! `migrations/0001_initial.sql`.

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use signex_library::adapter::ComponentSummary;
use signex_library::component::{Component, Revision};
use signex_library::embed::{ParamMap, ParamValue};
use signex_library::identity::{ComponentId, InternalPn, Version};
use signex_library::lifecycle::LifecycleState;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Executor, Row};

use crate::locks::LockManager;

/// Pool variant selected by URL scheme.
#[derive(Clone)]
pub enum DbPool {
    Sqlite(sqlx::SqlitePool),
    Postgres(sqlx::PgPool),
}

impl DbPool {
    pub fn sqlite(&self) -> Option<&sqlx::SqlitePool> {
        match self {
            DbPool::Sqlite(p) => Some(p),
            _ => None,
        }
    }

    pub fn postgres(&self) -> Option<&sqlx::PgPool> {
        match self {
            DbPool::Postgres(p) => Some(p),
            _ => None,
        }
    }
}

/// Server-side state shared across all axum handlers.
#[derive(Clone)]
pub struct AppState {
    pool: DbPool,
    locks: Arc<LockManager>,
}

impl AppState {
    /// Open an in-memory SQLite database. The pool is held to a single
    /// connection so tables persist for the lifetime of `AppState`.
    pub async fn new_sqlite_memory() -> sqlx::Result<Self> {
        let opts = SqliteConnectOptions::from_str("sqlite::memory:")?
            .journal_mode(SqliteJournalMode::Memory)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(opts)
            .await?;
        Ok(Self {
            pool: DbPool::Sqlite(pool),
            locks: Arc::new(LockManager::new(Duration::from_secs(10 * 60))),
        })
    }

    /// Connect to a remote backend. URL scheme picks the driver:
    /// `sqlite://...` or `postgres://...`.
    pub async fn connect(url: &str) -> sqlx::Result<Self> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(8)
                .connect(url)
                .await?;
            Ok(Self {
                pool: DbPool::Postgres(pool),
                locks: Arc::new(LockManager::new(Duration::from_secs(10 * 60))),
            })
        } else {
            let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(8)
                .connect_with(opts)
                .await?;
            Ok(Self {
                pool: DbPool::Sqlite(pool),
                locks: Arc::new(LockManager::new(Duration::from_secs(10 * 60))),
            })
        }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    pub fn locks(&self) -> &LockManager {
        &self.locks
    }

    /// Apply embedded migrations.
    pub async fn migrate(&self) -> sqlx::Result<()> {
        match &self.pool {
            DbPool::Sqlite(p) => MIGRATOR.run(p).await?,
            DbPool::Postgres(p) => MIGRATOR.run(p).await?,
        }
        Ok(())
    }

    /// Insert (or upsert) a component plus its revisions. Used by routes and
    /// by tests building fixture data.
    pub async fn insert_component(&self, comp: &Component) -> sqlx::Result<()> {
        match &self.pool {
            DbPool::Sqlite(pool) => insert_component_sqlite(pool, comp).await,
            DbPool::Postgres(pool) => insert_component_postgres(pool, comp).await,
        }
    }

    pub async fn fetch_component(&self, uuid: ComponentId) -> sqlx::Result<Option<Component>> {
        match &self.pool {
            DbPool::Sqlite(pool) => fetch_component_sqlite(pool, uuid).await,
            DbPool::Postgres(pool) => fetch_component_postgres(pool, uuid).await,
        }
    }

    pub async fn list_components(&self) -> sqlx::Result<Vec<ComponentSummary>> {
        match &self.pool {
            DbPool::Sqlite(pool) => list_components_sqlite(pool).await,
            DbPool::Postgres(pool) => list_components_postgres(pool).await,
        }
    }

    pub async fn save_revision(
        &self,
        uuid: ComponentId,
        revision: &Revision,
        internal_pn_default: &InternalPn,
    ) -> sqlx::Result<()> {
        // Upsert the parent component (cheap: ignores if it already exists).
        let now = Utc::now();
        let parent = Component {
            uuid,
            internal_pn: internal_pn_default.clone(),
            revisions: vec![revision.clone()],
            head: revision.version,
        };
        // Insert component header (ignored on conflict) then the revision row.
        match &self.pool {
            DbPool::Sqlite(pool) => {
                upsert_component_header_sqlite(pool, &parent, now).await?;
                insert_revision_sqlite(pool, uuid, revision).await?;
                update_head_sqlite(pool, uuid, revision.version, now).await?;
            }
            DbPool::Postgres(pool) => {
                upsert_component_header_postgres(pool, &parent, now).await?;
                insert_revision_postgres(pool, uuid, revision).await?;
                update_head_postgres(pool, uuid, revision.version, now).await?;
            }
        }
        Ok(())
    }
}

// SQLx migrator pulling from the on-disk `migrations/` folder.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Wrap `serde_json::Error` into the `sqlx::Error::Decode(Box<dyn StdError>)` form.
fn decode_err(e: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(e))
}

// ---------- SQLite implementations -------------------------------------------------

async fn insert_component_sqlite(pool: &sqlx::SqlitePool, comp: &Component) -> sqlx::Result<()> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;
    upsert_component_header_tx_sqlite(&mut tx, comp, now).await?;
    for rev in &comp.revisions {
        insert_revision_tx_sqlite(&mut tx, comp.uuid, rev).await?;
    }
    sqlx::query("UPDATE components SET head_major = ?, head_minor = ?, updated = ? WHERE uuid = ?")
        .bind(comp.head.major as i64)
        .bind(comp.head.minor as i64)
        .bind(now.to_rfc3339())
        .bind(comp.uuid.to_string())
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn upsert_component_header_sqlite(
    pool: &sqlx::SqlitePool,
    comp: &Component,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    let mut conn = pool.acquire().await?;
    sqlx::query(
        "INSERT INTO components (uuid, internal_pn, head_major, head_minor, created, updated) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(uuid) DO UPDATE SET internal_pn = excluded.internal_pn, updated = excluded.updated",
    )
    .bind(comp.uuid.to_string())
    .bind(comp.internal_pn.as_str())
    .bind(comp.head.major as i64)
    .bind(comp.head.minor as i64)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&mut *conn)
    .await?;
    Ok(())
}

async fn upsert_component_header_tx_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    comp: &Component,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO components (uuid, internal_pn, head_major, head_minor, created, updated) \
         VALUES (?, ?, ?, ?, ?, ?) \
         ON CONFLICT(uuid) DO UPDATE SET internal_pn = excluded.internal_pn, updated = excluded.updated",
    )
    .bind(comp.uuid.to_string())
    .bind(comp.internal_pn.as_str())
    .bind(comp.head.major as i64)
    .bind(comp.head.minor as i64)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_revision_sqlite(
    pool: &sqlx::SqlitePool,
    uuid: ComponentId,
    rev: &Revision,
) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    insert_revision_tx_sqlite(&mut tx, uuid, rev).await?;
    tx.commit().await
}

async fn insert_revision_tx_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    uuid: ComponentId,
    rev: &Revision,
) -> sqlx::Result<()> {
    let payload = serde_json::to_string(rev).map_err(decode_err)?;
    let hash_hex = hex_encode(&rev.content_hash);
    sqlx::query(
        "INSERT INTO revisions (component_uuid, major, minor, state, author, message, created, content_hash, payload) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(component_uuid, major, minor) DO UPDATE SET state = excluded.state, payload = excluded.payload",
    )
    .bind(uuid.to_string())
    .bind(rev.version.major as i64)
    .bind(rev.version.minor as i64)
    .bind(serde_json::to_string(&rev.state).map_err(decode_err)?.trim_matches('"').to_string())
    .bind(&rev.author)
    .bind(&rev.message)
    .bind(rev.created.to_rfc3339())
    .bind(hash_hex)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    insert_parameters_tx_sqlite(tx, uuid, rev).await?;
    insert_suppliers_tx_sqlite(tx, uuid, rev).await?;
    Ok(())
}

async fn insert_parameters_tx_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    uuid: ComponentId,
    rev: &Revision,
) -> sqlx::Result<()> {
    // wipe the old rows for this revision then re-insert.
    sqlx::query("DELETE FROM parameters WHERE component_uuid = ? AND major = ? AND minor = ?")
        .bind(uuid.to_string())
        .bind(rev.version.major as i64)
        .bind(rev.version.minor as i64)
        .execute(&mut **tx)
        .await?;
    insert_param_set_tx_sqlite(tx, uuid, rev.version, "shared", &rev.shared.parameters).await?;
    insert_param_set_tx_sqlite(
        tx,
        uuid,
        rev.version,
        "schematic",
        &rev.schematic.schematic_params,
    )
    .await?;
    insert_param_set_tx_sqlite(tx, uuid, rev.version, "pcb", &rev.pcb.pcb_params).await?;
    Ok(())
}

async fn insert_param_set_tx_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    uuid: ComponentId,
    version: Version,
    side: &str,
    params: &ParamMap,
) -> sqlx::Result<()> {
    for (key, value) in params {
        let serialised = serialise_param(value);
        sqlx::query(
            "INSERT INTO parameters (component_uuid, major, minor, side, key, value) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(uuid.to_string())
        .bind(version.major as i64)
        .bind(version.minor as i64)
        .bind(side)
        .bind(key)
        .bind(serialised)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

fn serialise_param(value: &ParamValue) -> String {
    match value {
        ParamValue::Text(s) => s.clone(),
        ParamValue::Number(n) => n.to_string(),
        ParamValue::Bool(b) => b.to_string(),
        ParamValue::Measurement { value, unit } => format!("{value} {unit}"),
    }
}

async fn insert_suppliers_tx_sqlite(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    uuid: ComponentId,
    rev: &Revision,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM suppliers WHERE component_uuid = ? AND major = ? AND minor = ?")
        .bind(uuid.to_string())
        .bind(rev.version.major as i64)
        .bind(rev.version.minor as i64)
        .execute(&mut **tx)
        .await?;
    for link in &rev.shared.suppliers {
        sqlx::query(
            "INSERT OR IGNORE INTO suppliers (component_uuid, major, minor, distributor, sku) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(uuid.to_string())
        .bind(rev.version.major as i64)
        .bind(rev.version.minor as i64)
        .bind(&link.distributor)
        .bind(&link.sku)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

async fn update_head_sqlite(
    pool: &sqlx::SqlitePool,
    uuid: ComponentId,
    version: Version,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE components SET head_major = ?, head_minor = ?, updated = ? WHERE uuid = ?")
        .bind(version.major as i64)
        .bind(version.minor as i64)
        .bind(now.to_rfc3339())
        .bind(uuid.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

async fn fetch_component_sqlite(
    pool: &sqlx::SqlitePool,
    uuid: ComponentId,
) -> sqlx::Result<Option<Component>> {
    let row =
        sqlx::query("SELECT internal_pn, head_major, head_minor FROM components WHERE uuid = ?")
            .bind(uuid.to_string())
            .fetch_optional(pool)
            .await?;
    let Some(row) = row else { return Ok(None) };
    let internal_pn: String = row.get("internal_pn");
    let head_major: i64 = row.get("head_major");
    let head_minor: i64 = row.get("head_minor");

    let revs =
        sqlx::query("SELECT payload FROM revisions WHERE component_uuid = ? ORDER BY major, minor")
            .bind(uuid.to_string())
            .fetch_all(pool)
            .await?;
    let mut revisions = Vec::with_capacity(revs.len());
    for r in revs {
        let payload: String = r.get("payload");
        let rev: Revision = serde_json::from_str(&payload).map_err(decode_err)?;
        revisions.push(rev);
    }
    Ok(Some(Component {
        uuid,
        internal_pn: InternalPn::new(internal_pn),
        revisions,
        head: Version::new(head_major as u32, head_minor as u32),
    }))
}

async fn list_components_sqlite(pool: &sqlx::SqlitePool) -> sqlx::Result<Vec<ComponentSummary>> {
    let rows = pool
        .fetch_all(
            "SELECT c.uuid, c.internal_pn, c.head_major, c.head_minor, \
                    r.state, r.payload \
             FROM components c \
             LEFT JOIN revisions r \
               ON r.component_uuid = c.uuid \
              AND r.major = c.head_major \
              AND r.minor = c.head_minor \
             ORDER BY c.internal_pn",
        )
        .await?;

    rows_to_summaries(rows).await
}

async fn rows_to_summaries(
    rows: Vec<sqlx::sqlite::SqliteRow>,
) -> sqlx::Result<Vec<ComponentSummary>> {
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let uuid_str: String = r.get("uuid");
        let internal_pn: String = r.get("internal_pn");
        let head_major: i64 = r.get("head_major");
        let head_minor: i64 = r.get("head_minor");
        let state: Option<String> = r.try_get("state").ok();
        let payload: Option<String> = r.try_get("payload").ok();

        let (state, description, mpn) = match (state, payload) {
            (Some(s), Some(p)) => {
                let rev: Revision = serde_json::from_str(&p).map_err(decode_err)?;
                (
                    parse_lifecycle(&s).unwrap_or(LifecycleState::Draft),
                    rev.shared.description.clone(),
                    rev.shared.mpn.clone(),
                )
            }
            _ => (LifecycleState::Draft, String::new(), String::new()),
        };

        out.push(ComponentSummary {
            uuid: ComponentId::from_str(&uuid_str).unwrap_or_else(|_| ComponentId::nil()),
            internal_pn: InternalPn::new(internal_pn),
            mpn,
            head: Version::new(head_major as u32, head_minor as u32),
            state,
            description,
        });
    }
    Ok(out)
}

fn parse_lifecycle(raw: &str) -> Option<LifecycleState> {
    match raw {
        "Draft" => Some(LifecycleState::Draft),
        "InReview" => Some(LifecycleState::InReview),
        "Released" => Some(LifecycleState::Released),
        "Deprecated" => Some(LifecycleState::Deprecated),
        "Obsolete" => Some(LifecycleState::Obsolete),
        _ => None,
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

// ---------- Postgres implementations -----------------------------------------------
//
// Postgres uses `$1, $2, ...` placeholders and `ON CONFLICT` syntax.

async fn insert_component_postgres(pool: &sqlx::PgPool, comp: &Component) -> sqlx::Result<()> {
    let now = Utc::now();
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO components (uuid, internal_pn, head_major, head_minor, created, updated) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (uuid) DO UPDATE SET internal_pn = EXCLUDED.internal_pn, updated = EXCLUDED.updated",
    )
    .bind(comp.uuid.to_string())
    .bind(comp.internal_pn.as_str())
    .bind(comp.head.major as i64)
    .bind(comp.head.minor as i64)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(&mut *tx)
    .await?;
    for rev in &comp.revisions {
        let payload = serde_json::to_string(rev).map_err(decode_err)?;
        let hash_hex = hex_encode(&rev.content_hash);
        sqlx::query(
            "INSERT INTO revisions (component_uuid, major, minor, state, author, message, created, content_hash, payload) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (component_uuid, major, minor) DO UPDATE SET state = EXCLUDED.state, payload = EXCLUDED.payload",
        )
        .bind(comp.uuid.to_string())
        .bind(rev.version.major as i64)
        .bind(rev.version.minor as i64)
        .bind(serde_json::to_string(&rev.state).map_err(decode_err)?.trim_matches('"').to_string())
        .bind(&rev.author)
        .bind(&rev.message)
        .bind(rev.created.to_rfc3339())
        .bind(hash_hex)
        .bind(payload)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn upsert_component_header_postgres(
    pool: &sqlx::PgPool,
    comp: &Component,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO components (uuid, internal_pn, head_major, head_minor, created, updated) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (uuid) DO UPDATE SET internal_pn = EXCLUDED.internal_pn, updated = EXCLUDED.updated",
    )
    .bind(comp.uuid.to_string())
    .bind(comp.internal_pn.as_str())
    .bind(comp.head.major as i64)
    .bind(comp.head.minor as i64)
    .bind(now.to_rfc3339())
    .bind(now.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_revision_postgres(
    pool: &sqlx::PgPool,
    uuid: ComponentId,
    rev: &Revision,
) -> sqlx::Result<()> {
    let payload = serde_json::to_string(rev).map_err(decode_err)?;
    let hash_hex = hex_encode(&rev.content_hash);
    sqlx::query(
        "INSERT INTO revisions (component_uuid, major, minor, state, author, message, created, content_hash, payload) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) \
         ON CONFLICT (component_uuid, major, minor) DO UPDATE SET state = EXCLUDED.state, payload = EXCLUDED.payload",
    )
    .bind(uuid.to_string())
    .bind(rev.version.major as i64)
    .bind(rev.version.minor as i64)
    .bind(serde_json::to_string(&rev.state).map_err(decode_err)?.trim_matches('"').to_string())
    .bind(&rev.author)
    .bind(&rev.message)
    .bind(rev.created.to_rfc3339())
    .bind(hash_hex)
    .bind(payload)
    .execute(pool)
    .await?;
    Ok(())
}

async fn update_head_postgres(
    pool: &sqlx::PgPool,
    uuid: ComponentId,
    version: Version,
    now: DateTime<Utc>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE components SET head_major = $1, head_minor = $2, updated = $3 WHERE uuid = $4",
    )
    .bind(version.major as i64)
    .bind(version.minor as i64)
    .bind(now.to_rfc3339())
    .bind(uuid.to_string())
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_component_postgres(
    pool: &sqlx::PgPool,
    uuid: ComponentId,
) -> sqlx::Result<Option<Component>> {
    let row =
        sqlx::query("SELECT internal_pn, head_major, head_minor FROM components WHERE uuid = $1")
            .bind(uuid.to_string())
            .fetch_optional(pool)
            .await?;
    let Some(row) = row else { return Ok(None) };
    let internal_pn: String = row.get("internal_pn");
    let head_major: i64 = row.get("head_major");
    let head_minor: i64 = row.get("head_minor");

    let revs = sqlx::query(
        "SELECT payload FROM revisions WHERE component_uuid = $1 ORDER BY major, minor",
    )
    .bind(uuid.to_string())
    .fetch_all(pool)
    .await?;
    let mut revisions = Vec::with_capacity(revs.len());
    for r in revs {
        let payload: String = r.get("payload");
        let rev: Revision = serde_json::from_str(&payload).map_err(decode_err)?;
        revisions.push(rev);
    }
    Ok(Some(Component {
        uuid,
        internal_pn: InternalPn::new(internal_pn),
        revisions,
        head: Version::new(head_major as u32, head_minor as u32),
    }))
}

async fn list_components_postgres(pool: &sqlx::PgPool) -> sqlx::Result<Vec<ComponentSummary>> {
    let rows = sqlx::query(
        "SELECT c.uuid, c.internal_pn, c.head_major, c.head_minor, \
                r.state, r.payload \
         FROM components c \
         LEFT JOIN revisions r \
           ON r.component_uuid = c.uuid \
          AND r.major = c.head_major \
          AND r.minor = c.head_minor \
         ORDER BY c.internal_pn",
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let uuid_str: String = r.get("uuid");
        let internal_pn: String = r.get("internal_pn");
        let head_major: i64 = r.get("head_major");
        let head_minor: i64 = r.get("head_minor");
        let state: Option<String> = r.try_get("state").ok();
        let payload: Option<String> = r.try_get("payload").ok();
        let (state, description, mpn) = match (state, payload) {
            (Some(s), Some(p)) => {
                let rev: Revision = serde_json::from_str(&p).map_err(decode_err)?;
                (
                    parse_lifecycle(&s).unwrap_or(LifecycleState::Draft),
                    rev.shared.description.clone(),
                    rev.shared.mpn.clone(),
                )
            }
            _ => (LifecycleState::Draft, String::new(), String::new()),
        };
        out.push(ComponentSummary {
            uuid: ComponentId::from_str(&uuid_str).unwrap_or_else(|_| ComponentId::nil()),
            internal_pn: InternalPn::new(internal_pn),
            mpn,
            head: Version::new(head_major as u32, head_minor as u32),
            state,
            description,
        });
    }
    Ok(out)
}
