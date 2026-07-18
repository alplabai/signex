//! Database layer — pool management, migrations, and component-row /
//! primitive persistence helpers used by the route handlers.
//!
//! Components live as rows inside category tables (Altium DBLib
//! model). This module exposes:
//!
//! * primitive CRUD (`insert_symbol` / `fetch_symbol` / …) —
//!   primitives stay file-shaped under the row model;
//! * row CRUD (`insert_row` / `fetch_row` / `update_row` / `delete_row`) +
//!   table-name listing (`list_table_names` / `list_rows_in_table`) —
//!   backing the `/tables` and `/rows` HTTP routes.
//!
//! The pool is a thin enum over SQLite (default for tests + offline) and
//! Postgres (production). Schema is portable across both — see
//! `migrations/0001_initial.sql` (legacy, retained for forward-compat) +
//! `migrations/0005_tabular_components.sql` (the row table).

use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use signex_library::component::ComponentRow;
use signex_library::identity::RowId;
use signex_library::primitive::{Footprint, SimModel, Symbol};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use uuid::Uuid;

use crate::locks::LockManager;

/// Summary record for a primitive (Symbol / Footprint / SimModel) — what the
/// `GET /symbols` etc. routes return when listing a library.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PrimitiveSummary {
    pub library_id: Uuid,
    pub uuid: Uuid,
    pub name: String,
}

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

    /// Hand out a clone of the `Arc<LockManager>` for background tasks
    /// (the periodic `sweep_expired` sweeper spawned in `router_with_state`
    /// holds one of these).
    pub fn locks_arc(&self) -> Arc<LockManager> {
        Arc::clone(&self.locks)
    }

    /// Apply embedded migrations.
    pub async fn migrate(&self) -> sqlx::Result<()> {
        match &self.pool {
            DbPool::Sqlite(p) => MIGRATOR.run(p).await?,
            DbPool::Postgres(p) => MIGRATOR.run(p).await?,
        }
        Ok(())
    }

    // ── Component-row CRUD ─────────────────────────────────────────────────
    //
    // `component_rows` is the unified DBLib row table. `(library_id,
    // table_name, row_id)` is the primary key — same shape across SQLite
    // and Postgres so the route handlers can stay backend-agnostic.
    //
    // The row body is round-tripped as JSON via `ComponentRow`'s serde impl
    // so adding fields later (e.g. when v3.0 lifts `PlmReserved` into wire
    // format) doesn't require a schema migration.

    /// Insert a brand-new row. Returns `Ok(false)` when a row with the
    /// same `(library_id, table, row_id)` already exists so the caller
    /// can answer `409` — POST must never silently overwrite an
    /// existing row (an upsert would let one client clobber another's
    /// component with no warning). Replacement goes through
    /// [`update_row`] (PUT).
    pub async fn insert_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row: &ComponentRow,
    ) -> sqlx::Result<bool> {
        let payload = serde_json::to_string(row).map_err(decode_err)?;
        let now = Utc::now().to_rfc3339();
        let row_id = row.row_id.to_string();
        let internal_pn = row.internal_pn.as_str().to_string();
        let result = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO component_rows \
                       (library_id, table_name, row_id, internal_pn, payload, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&row_id)
                .bind(&internal_pn)
                .bind(&payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map(|_| ())
            }
            DbPool::Postgres(pool) => {
                sqlx::query(
                    "INSERT INTO component_rows \
                       (library_id, table_name, row_id, internal_pn, payload, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&row_id)
                .bind(&internal_pn)
                .bind(&payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map(|_| ())
            }
        };
        match result {
            Ok(_) => Ok(true),
            Err(e) if is_unique_violation(&e) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Update an existing row. Returns `Ok(false)` if no row with the
    /// supplied `(library_id, table, row_id)` exists; the caller maps that
    /// to a 404.
    pub async fn update_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row: &ComponentRow,
    ) -> sqlx::Result<bool> {
        let payload = serde_json::to_string(row).map_err(decode_err)?;
        let now = Utc::now().to_rfc3339();
        let row_id = row.row_id.to_string();
        let internal_pn = row.internal_pn.as_str().to_string();
        let affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "UPDATE component_rows SET \
                         internal_pn = ?, payload = ?, updated_at = ? \
                     WHERE library_id = ? AND table_name = ? AND row_id = ?",
            )
            .bind(&internal_pn)
            .bind(&payload)
            .bind(&now)
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&row_id)
            .execute(pool)
            .await?
            .rows_affected(),
            DbPool::Postgres(pool) => sqlx::query(
                "UPDATE component_rows SET \
                         internal_pn = $1, payload = $2, updated_at = $3 \
                     WHERE library_id = $4 AND table_name = $5 AND row_id = $6",
            )
            .bind(&internal_pn)
            .bind(&payload)
            .bind(&now)
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&row_id)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        Ok(affected > 0)
    }

    pub async fn fetch_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row_id: RowId,
    ) -> sqlx::Result<Option<ComponentRow>> {
        let id_str = row_id.to_string();
        let payload: Option<String> = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_scalar(
                    "SELECT payload FROM component_rows \
                 WHERE library_id = ? AND table_name = ? AND row_id = ?",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&id_str)
                .fetch_optional(pool)
                .await?
            }
            DbPool::Postgres(pool) => {
                sqlx::query_scalar(
                    "SELECT payload FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 AND row_id = $3",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&id_str)
                .fetch_optional(pool)
                .await?
            }
        };
        payload
            .map(|p| serde_json::from_str(&p).map_err(decode_err))
            .transpose()
    }

    /// Delete a row. Returns `Ok(false)` if no matching row existed.
    pub async fn delete_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row_id: RowId,
    ) -> sqlx::Result<bool> {
        let id_str = row_id.to_string();
        let affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "DELETE FROM component_rows \
                 WHERE library_id = ? AND table_name = ? AND row_id = ?",
            )
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&id_str)
            .execute(pool)
            .await?
            .rows_affected(),
            DbPool::Postgres(pool) => sqlx::query(
                "DELETE FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 AND row_id = $3",
            )
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&id_str)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        Ok(affected > 0)
    }

    /// List the names of every distinct table that has at least one row
    /// inside `library_id`.
    pub async fn list_table_names(&self, library_id: Uuid) -> sqlx::Result<Vec<String>> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_scalar(
                    "SELECT DISTINCT table_name FROM component_rows \
                 WHERE library_id = ? ORDER BY table_name",
                )
                .bind(library_id.to_string())
                .fetch_all(pool)
                .await
            }
            DbPool::Postgres(pool) => {
                sqlx::query_scalar(
                    "SELECT DISTINCT table_name FROM component_rows \
                 WHERE library_id = $1 ORDER BY table_name",
                )
                .bind(library_id.to_string())
                .fetch_all(pool)
                .await
            }
        }
    }

    /// Read every row in `table_name` for `library_id`, ordered by
    /// `internal_pn`.
    pub async fn list_rows_in_table(
        &self,
        library_id: Uuid,
        table_name: &str,
    ) -> sqlx::Result<Vec<ComponentRow>> {
        let payloads: Vec<String> = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_scalar(
                    "SELECT payload FROM component_rows \
                 WHERE library_id = ? AND table_name = ? \
                 ORDER BY internal_pn",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .fetch_all(pool)
                .await?
            }
            DbPool::Postgres(pool) => {
                sqlx::query_scalar(
                    "SELECT payload FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 \
                 ORDER BY internal_pn",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .fetch_all(pool)
                .await?
            }
        };
        payloads
            .into_iter()
            .map(|p| serde_json::from_str(&p).map_err(decode_err))
            .collect()
    }

    // ── Primitive CRUD ────────────────────────────────────────────────────
    //
    // The primitives table layout is identical for all three kinds, so we
    // share one generic helper per backend with the table name as a parameter.
    // sqlx doesn't templatise table names, so we route through a `match`.

    pub async fn insert_symbol(&self, library_id: Uuid, sym: &Symbol) -> sqlx::Result<()> {
        let payload = serde_json::to_string(sym).map_err(decode_err)?;
        upsert_primitive(
            &self.pool, "symbols", library_id, sym.uuid, &sym.name, &payload,
        )
        .await
    }

    pub async fn fetch_symbol(&self, library_id: Uuid, uuid: Uuid) -> sqlx::Result<Option<Symbol>> {
        fetch_primitive_payload(&self.pool, "symbols", library_id, uuid)
            .await?
            .map(|p| serde_json::from_str(&p).map_err(decode_err))
            .transpose()
    }

    pub async fn list_symbols(
        &self,
        library_id: Option<Uuid>,
    ) -> sqlx::Result<Vec<PrimitiveSummary>> {
        list_primitive_summaries(&self.pool, "symbols", library_id).await
    }

    pub async fn insert_footprint(&self, library_id: Uuid, fp: &Footprint) -> sqlx::Result<()> {
        let payload = serde_json::to_string(fp).map_err(decode_err)?;
        upsert_primitive(
            &self.pool,
            "footprints",
            library_id,
            fp.uuid,
            &fp.name,
            &payload,
        )
        .await
    }

    pub async fn fetch_footprint(
        &self,
        library_id: Uuid,
        uuid: Uuid,
    ) -> sqlx::Result<Option<Footprint>> {
        fetch_primitive_payload(&self.pool, "footprints", library_id, uuid)
            .await?
            .map(|p| serde_json::from_str(&p).map_err(decode_err))
            .transpose()
    }

    pub async fn list_footprints(
        &self,
        library_id: Option<Uuid>,
    ) -> sqlx::Result<Vec<PrimitiveSummary>> {
        list_primitive_summaries(&self.pool, "footprints", library_id).await
    }

    pub async fn insert_sim(&self, library_id: Uuid, sm: &SimModel) -> sqlx::Result<()> {
        let payload = serde_json::to_string(sm).map_err(decode_err)?;
        upsert_primitive(&self.pool, "sims", library_id, sm.uuid, &sm.name, &payload).await
    }

    pub async fn fetch_sim(&self, library_id: Uuid, uuid: Uuid) -> sqlx::Result<Option<SimModel>> {
        fetch_primitive_payload(&self.pool, "sims", library_id, uuid)
            .await?
            .map(|p| serde_json::from_str(&p).map_err(decode_err))
            .transpose()
    }

    pub async fn list_sims(&self, library_id: Option<Uuid>) -> sqlx::Result<Vec<PrimitiveSummary>> {
        list_primitive_summaries(&self.pool, "sims", library_id).await
    }
}

// ---------- Primitive query helpers ----------------------------------------

/// Whitelist of primitive table names — guards against SQL injection through
/// the `table` parameter that callers in this module supply. `assert!` (not
/// `debug_assert!`) so the guard remains in release builds — every caller
/// passes `&'static str` literals today, but a future refactor that injects
/// non-literal data would land in a release-mode SQL injection without this.
fn assert_primitive_table(table: &str) {
    assert!(
        matches!(table, "symbols" | "footprints" | "sims"),
        "primitive table name `{table}` is not whitelisted",
    );
}

async fn upsert_primitive(
    pool: &DbPool,
    table: &'static str,
    library_id: Uuid,
    uuid: Uuid,
    name: &str,
    payload: &str,
) -> sqlx::Result<()> {
    assert_primitive_table(table);
    let now = Utc::now().to_rfc3339();
    match pool {
        DbPool::Sqlite(pool) => {
            // sqlx 0.8 doesn't templatise identifiers; format!() with the
            // whitelisted table is safe (see `assert_primitive_table`).
            let sql = format!(
                "INSERT INTO {table} (library_id, uuid, name, payload, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?) \
                 ON CONFLICT(library_id, uuid) DO UPDATE SET \
                     name = excluded.name, \
                     payload = excluded.payload, \
                     updated_at = excluded.updated_at",
            );
            sqlx::query(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .bind(name)
                .bind(payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await?;
        }
        DbPool::Postgres(pool) => {
            let sql = format!(
                "INSERT INTO {table} (library_id, uuid, name, payload, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6) \
                 ON CONFLICT (library_id, uuid) DO UPDATE SET \
                     name = EXCLUDED.name, \
                     payload = EXCLUDED.payload, \
                     updated_at = EXCLUDED.updated_at",
            );
            sqlx::query(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .bind(name)
                .bind(payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

async fn fetch_primitive_payload(
    pool: &DbPool,
    table: &'static str,
    library_id: Uuid,
    uuid: Uuid,
) -> sqlx::Result<Option<String>> {
    assert_primitive_table(table);
    match pool {
        DbPool::Sqlite(pool) => {
            let sql = format!("SELECT payload FROM {table} WHERE library_id = ? AND uuid = ?");
            let row = sqlx::query(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .fetch_optional(pool)
                .await?;
            Ok(row.map(|r| r.get::<String, _>("payload")))
        }
        DbPool::Postgres(pool) => {
            let sql = format!("SELECT payload FROM {table} WHERE library_id = $1 AND uuid = $2");
            let row = sqlx::query(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .fetch_optional(pool)
                .await?;
            Ok(row.map(|r| r.get::<String, _>("payload")))
        }
    }
}

async fn list_primitive_summaries(
    pool: &DbPool,
    table: &'static str,
    library_id: Option<Uuid>,
) -> sqlx::Result<Vec<PrimitiveSummary>> {
    assert_primitive_table(table);
    match pool {
        DbPool::Sqlite(pool) => {
            let rows = if let Some(lib) = library_id {
                let sql = format!(
                    "SELECT library_id, uuid, name FROM {table} \
                     WHERE library_id = ? ORDER BY name"
                );
                sqlx::query(&sql)
                    .bind(lib.to_string())
                    .fetch_all(pool)
                    .await?
            } else {
                let sql = format!("SELECT library_id, uuid, name FROM {table} ORDER BY name");
                sqlx::query(&sql).fetch_all(pool).await?
            };
            rows.into_iter()
                .map(|r| {
                    let lib: String = r.get("library_id");
                    let id: String = r.get("uuid");
                    let name: String = r.get("name");
                    // HI-9: surface decode errors instead of mapping to
                    // Uuid::nil() — corrupt rows would otherwise alias
                    // and confuse caller-side aggregation.
                    let library_id = Uuid::parse_str(&lib).map_err(uuid_decode_err)?;
                    let uuid = Uuid::parse_str(&id).map_err(uuid_decode_err)?;
                    Ok(PrimitiveSummary {
                        library_id,
                        uuid,
                        name,
                    })
                })
                .collect()
        }
        DbPool::Postgres(pool) => {
            let rows = if let Some(lib) = library_id {
                let sql = format!(
                    "SELECT library_id, uuid, name FROM {table} \
                     WHERE library_id = $1 ORDER BY name"
                );
                sqlx::query(&sql)
                    .bind(lib.to_string())
                    .fetch_all(pool)
                    .await?
            } else {
                let sql = format!("SELECT library_id, uuid, name FROM {table} ORDER BY name");
                sqlx::query(&sql).fetch_all(pool).await?
            };
            rows.into_iter()
                .map(|r| {
                    let lib: String = r.get("library_id");
                    let id: String = r.get("uuid");
                    let name: String = r.get("name");
                    // HI-9: surface decode errors instead of mapping to
                    // Uuid::nil() — corrupt rows would otherwise alias
                    // and confuse caller-side aggregation.
                    let library_id = Uuid::parse_str(&lib).map_err(uuid_decode_err)?;
                    let uuid = Uuid::parse_str(&id).map_err(uuid_decode_err)?;
                    Ok(PrimitiveSummary {
                        library_id,
                        uuid,
                        name,
                    })
                })
                .collect()
        }
    }
}

// SQLx migrator pulling from the on-disk `migrations/` folder.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Wrap `serde_json::Error` into the `sqlx::Error::Decode(Box<dyn StdError>)` form.
fn decode_err(e: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(e))
}

/// True when a sqlx error is a primary-key / unique-constraint
/// violation — the signal that a plain `INSERT` hit an existing row.
/// Backend-agnostic via `DatabaseError::kind()` (SQLite + Postgres).
fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db) if db.kind() == sqlx::error::ErrorKind::UniqueViolation
    )
}

/// HI-9: surface UUID parse failures as `sqlx::Error::Decode` instead of
/// silently mapping to `Uuid::nil()`. A corrupt row is a real problem the
/// operator needs to see, not a row that aliases with every other corrupt
/// row in the result set.
fn uuid_decode_err(e: uuid::Error) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(e))
}
