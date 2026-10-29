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
//!   backing the `/tables` and `/rows` HTTP routes. Every row carries a
//!   `revision`; updates and deletes are conditional on it ([`RowWrite`]).
//!
//! The pool is a thin enum over SQLite (default for tests + offline) and
//! Postgres (production). Each backend has its own migration set with
//! identical versions and tables: `migrations/*.sql` (SQLite) and
//! `migrations/postgres/*.sql`. sqlx checksums applied migrations, so a
//! shipped file is never edited; a DDL change is a new, higher-numbered
//! migration in BOTH sets.

pub mod page;
pub(crate) mod pn_unique;
mod rows;

use std::str::FromStr;

pub use page::{DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT, Page};

use chrono::Utc;
use signex_library::component::ComponentRow;
use signex_library::primitive::{Footprint, SimModel, Symbol};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use uuid::Uuid;

/// Summary record for a primitive (Symbol / Footprint / SimModel) — what the
/// `GET /symbols` etc. routes return when listing a library.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PrimitiveSummary {
    pub library_id: Uuid,
    pub uuid: Uuid,
    pub name: String,
}

/// Outcome of a conditional row update or delete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowWrite {
    /// Written. An update leaves the row at `revision`; a delete reports
    /// the revision it removed.
    Applied { revision: i64 },
    /// Refused: the stored row is at `current_revision`, not the base the
    /// caller read.
    Stale { current_revision: i64 },
    /// No such row.
    Missing,
    /// Refused: another row of the library already holds this part number.
    PnTaken,
}

/// Outcome of inserting a new row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowInsert {
    Created,
    /// A row with the same `(library_id, table, row_id)` exists.
    RowExists,
    /// Another row of the library already holds this part number.
    PnTaken,
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
            })
        } else {
            let opts = SqliteConnectOptions::from_str(url)?.create_if_missing(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(8)
                .connect_with(opts)
                .await?;
            Ok(Self {
                pool: DbPool::Sqlite(pool),
            })
        }
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }

    /// Apply embedded migrations. The unique part-number index (migration
    /// 0010) is built only after the part-number keys are recomputed from the
    /// stored rows and found free of duplicates, see [`pn_unique`].
    pub async fn migrate(&self) -> sqlx::Result<()> {
        self.migrate_through(pn_unique::KEY_COLUMN_VERSION).await?;
        self.refuse_duplicate_part_numbers().await?;
        self.migrate_through(i64::MAX).await
    }

    // ── Primitive CRUD ────────────────────────────────────────────────────
    //
    // The primitives table layout is identical for all three kinds, so we
    // share one generic helper per backend with the table name as a parameter.
    // sqlx doesn't templatise table names, so we route through a `match`.

    /// Create a symbol; `false` when `(library_id, uuid)` already exists.
    pub async fn insert_symbol(&self, library_id: Uuid, sym: &Symbol) -> sqlx::Result<bool> {
        let payload = serde_json::to_string(sym).map_err(decode_err)?;
        insert_primitive(
            &self.pool, "symbols", library_id, sym.uuid, &sym.name, &payload,
        )
        .await
    }

    /// Replace a symbol if it is still at `base_revision`.
    pub async fn update_symbol(
        &self,
        library_id: Uuid,
        sym: &Symbol,
        base_revision: i64,
    ) -> sqlx::Result<RowWrite> {
        let payload = serde_json::to_string(sym).map_err(decode_err)?;
        update_primitive(
            &self.pool,
            "symbols",
            library_id,
            sym.uuid,
            &sym.name,
            &payload,
            base_revision,
        )
        .await
    }

    /// The symbol and the revision it is stored at.
    pub async fn fetch_symbol(
        &self,
        library_id: Uuid,
        uuid: Uuid,
    ) -> sqlx::Result<Option<(Symbol, i64)>> {
        fetch_primitive_payload(&self.pool, "symbols", library_id, uuid)
            .await?
            .map(|(p, rev)| Ok((serde_json::from_str(&p).map_err(decode_err)?, rev)))
            .transpose()
    }

    pub async fn list_symbols(
        &self,
        library_id: Option<Uuid>,
        page: Page,
    ) -> sqlx::Result<Vec<PrimitiveSummary>> {
        list_primitive_summaries(&self.pool, "symbols", library_id, page).await
    }

    /// Create a footprint; `false` when `(library_id, uuid)` already exists.
    pub async fn insert_footprint(&self, library_id: Uuid, fp: &Footprint) -> sqlx::Result<bool> {
        let payload = serde_json::to_string(fp).map_err(decode_err)?;
        insert_primitive(
            &self.pool,
            "footprints",
            library_id,
            fp.uuid,
            &fp.name,
            &payload,
        )
        .await
    }

    /// Replace a footprint if it is still at `base_revision`.
    pub async fn update_footprint(
        &self,
        library_id: Uuid,
        fp: &Footprint,
        base_revision: i64,
    ) -> sqlx::Result<RowWrite> {
        let payload = serde_json::to_string(fp).map_err(decode_err)?;
        update_primitive(
            &self.pool,
            "footprints",
            library_id,
            fp.uuid,
            &fp.name,
            &payload,
            base_revision,
        )
        .await
    }

    /// The footprint and the revision it is stored at.
    pub async fn fetch_footprint(
        &self,
        library_id: Uuid,
        uuid: Uuid,
    ) -> sqlx::Result<Option<(Footprint, i64)>> {
        fetch_primitive_payload(&self.pool, "footprints", library_id, uuid)
            .await?
            .map(|(p, rev)| {
                let mut fp: Footprint = serde_json::from_str(&p).map_err(decode_err)?;
                fp.repair_stored_sketch();
                Ok((fp, rev))
            })
            .transpose()
    }

    pub async fn list_footprints(
        &self,
        library_id: Option<Uuid>,
        page: Page,
    ) -> sqlx::Result<Vec<PrimitiveSummary>> {
        list_primitive_summaries(&self.pool, "footprints", library_id, page).await
    }

    /// Create a sim model; `false` when `(library_id, uuid)` already exists.
    pub async fn insert_sim(&self, library_id: Uuid, sm: &SimModel) -> sqlx::Result<bool> {
        let payload = serde_json::to_string(sm).map_err(decode_err)?;
        insert_primitive(&self.pool, "sims", library_id, sm.uuid, &sm.name, &payload).await
    }

    /// Replace a sim model if it is still at `base_revision`.
    pub async fn update_sim(
        &self,
        library_id: Uuid,
        sm: &SimModel,
        base_revision: i64,
    ) -> sqlx::Result<RowWrite> {
        let payload = serde_json::to_string(sm).map_err(decode_err)?;
        update_primitive(
            &self.pool,
            "sims",
            library_id,
            sm.uuid,
            &sm.name,
            &payload,
            base_revision,
        )
        .await
    }

    /// The sim model and the revision it is stored at.
    pub async fn fetch_sim(
        &self,
        library_id: Uuid,
        uuid: Uuid,
    ) -> sqlx::Result<Option<(SimModel, i64)>> {
        fetch_primitive_payload(&self.pool, "sims", library_id, uuid)
            .await?
            .map(|(p, rev)| Ok((serde_json::from_str(&p).map_err(decode_err)?, rev)))
            .transpose()
    }

    pub async fn list_sims(
        &self,
        library_id: Option<Uuid>,
        page: Page,
    ) -> sqlx::Result<Vec<PrimitiveSummary>> {
        list_primitive_summaries(&self.pool, "sims", library_id, page).await
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

/// Plain `INSERT`: `false` when `(library_id, uuid)` already exists. A
/// create never overwrites a primitive; replacing goes through
/// [`update_primitive`] with the revision the writer read.
async fn insert_primitive(
    pool: &DbPool,
    table: &'static str,
    library_id: Uuid,
    uuid: Uuid,
    name: &str,
    payload: &str,
) -> sqlx::Result<bool> {
    assert_primitive_table(table);
    let now = Utc::now().to_rfc3339();
    // sqlx 0.8 doesn't templatise identifiers; format!() with the
    // whitelisted table is safe (see `assert_primitive_table`).
    let result = match pool {
        DbPool::Sqlite(pool) => {
            let sql = format!(
                "INSERT INTO {table} (library_id, uuid, name, payload, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?)",
            );
            sqlx::query(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .bind(name)
                .bind(payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map(|_| ())
        }
        DbPool::Postgres(pool) => {
            let sql = format!(
                "INSERT INTO {table} (library_id, uuid, name, payload, created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6)",
            );
            sqlx::query(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .bind(name)
                .bind(payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map(|_| ())
        }
    };
    match result {
        Ok(()) => Ok(true),
        Err(e) if is_unique_violation(&e) => Ok(false),
        Err(e) => Err(e),
    }
}

/// Replace a primitive if it is still at `base_revision`, bumping the
/// revision. One `UPDATE ... WHERE revision = ?`, so of two writers with
/// the same base only one wins.
async fn update_primitive(
    pool: &DbPool,
    table: &'static str,
    library_id: Uuid,
    uuid: Uuid,
    name: &str,
    payload: &str,
    base_revision: i64,
) -> sqlx::Result<RowWrite> {
    assert_primitive_table(table);
    let now = Utc::now().to_rfc3339();
    let affected = match pool {
        DbPool::Sqlite(pool) => {
            let sql = format!(
                "UPDATE {table} SET name = ?, payload = ?, updated_at = ?, \
                     revision = revision + 1 \
                 WHERE library_id = ? AND uuid = ? AND revision = ?",
            );
            sqlx::query(&sql)
                .bind(name)
                .bind(payload)
                .bind(&now)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .bind(base_revision)
                .execute(pool)
                .await?
                .rows_affected()
        }
        DbPool::Postgres(pool) => {
            let sql = format!(
                "UPDATE {table} SET name = $1, payload = $2, updated_at = $3, \
                     revision = revision + 1 \
                 WHERE library_id = $4 AND uuid = $5 AND revision = $6",
            );
            sqlx::query(&sql)
                .bind(name)
                .bind(payload)
                .bind(&now)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .bind(base_revision)
                .execute(pool)
                .await?
                .rows_affected()
        }
    };
    if affected > 0 {
        return Ok(RowWrite::Applied {
            revision: base_revision + 1,
        });
    }
    // Matched nothing: the primitive is gone, or moved past the base.
    Ok(
        match fetch_primitive_payload(pool, table, library_id, uuid).await? {
            Some((_, current_revision)) => RowWrite::Stale { current_revision },
            None => RowWrite::Missing,
        },
    )
}

/// The stored JSON and its revision, read in one query.
async fn fetch_primitive_payload(
    pool: &DbPool,
    table: &'static str,
    library_id: Uuid,
    uuid: Uuid,
) -> sqlx::Result<Option<(String, i64)>> {
    assert_primitive_table(table);
    match pool {
        DbPool::Sqlite(pool) => {
            let sql =
                format!("SELECT payload, revision FROM {table} WHERE library_id = ? AND uuid = ?");
            sqlx::query_as(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .fetch_optional(pool)
                .await
        }
        DbPool::Postgres(pool) => {
            let sql = format!(
                "SELECT payload, revision FROM {table} WHERE library_id = $1 AND uuid = $2"
            );
            sqlx::query_as(&sql)
                .bind(library_id.to_string())
                .bind(uuid.to_string())
                .fetch_optional(pool)
                .await
        }
    }
}

async fn list_primitive_summaries(
    pool: &DbPool,
    table: &'static str,
    library_id: Option<Uuid>,
    page: Page,
) -> sqlx::Result<Vec<PrimitiveSummary>> {
    assert_primitive_table(table);
    match pool {
        DbPool::Sqlite(pool) => {
            let rows = if let Some(lib) = library_id {
                let sql = format!(
                    "SELECT library_id, uuid, name FROM {table} \
                     WHERE library_id = ? ORDER BY name, uuid LIMIT ? OFFSET ?"
                );
                sqlx::query(&sql)
                    .bind(lib.to_string())
                    .bind(page.limit)
                    .bind(page.offset)
                    .fetch_all(pool)
                    .await?
            } else {
                let sql = format!(
                    "SELECT library_id, uuid, name FROM {table} ORDER BY name, uuid LIMIT ? OFFSET ?"
                );
                sqlx::query(&sql)
                    .bind(page.limit)
                    .bind(page.offset)
                    .fetch_all(pool)
                    .await?
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
                     WHERE library_id = $1 ORDER BY name, uuid LIMIT $2 OFFSET $3"
                );
                sqlx::query(&sql)
                    .bind(lib.to_string())
                    .bind(page.limit)
                    .bind(page.offset)
                    .fetch_all(pool)
                    .await?
            } else {
                let sql = format!(
                    "SELECT library_id, uuid, name FROM {table} ORDER BY name, uuid LIMIT $1 OFFSET $2"
                );
                sqlx::query(&sql)
                    .bind(page.limit)
                    .bind(page.offset)
                    .fetch_all(pool)
                    .await?
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

/// SQLite migrator (`migrations/*.sql`; sqlx does not recurse into `postgres/`).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Postgres migrator (`migrations/postgres/*.sql`).
pub static POSTGRES_MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations/postgres");

/// Row JSON as stored: the revision lives in its own column, never in the
/// payload, so a client-supplied value cannot leak into storage.
fn row_payload(row: &ComponentRow) -> sqlx::Result<String> {
    let stored = ComponentRow {
        server_revision: None,
        ..row.clone()
    };
    serde_json::to_string(&stored).map_err(decode_err)
}

/// Decode a stored `(payload, revision)` pair into the row a client sees.
fn read_stored_row((payload, revision): (String, i64)) -> sqlx::Result<ComponentRow> {
    let mut row: ComponentRow = serde_json::from_str(&payload).map_err(decode_err)?;
    row.server_revision =
        Some(u64::try_from(revision).map_err(|e| sqlx::Error::Decode(Box::new(e)))?);
    Ok(row)
}

/// Wrap `serde_json::Error` into the `sqlx::Error::Decode(Box<dyn StdError>)` form.
fn decode_err(e: serde_json::Error) -> sqlx::Error {
    sqlx::Error::Decode(Box::new(e))
}

/// A unique violation of the part-number index, as opposed to the row key.
/// Postgres names the index; SQLite names the `pn_key` column in its message.
fn is_pn_violation(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) if is_unique_violation(err) => match db.constraint() {
            Some(name) => name == pn_unique::PN_INDEX,
            None => db.message().contains("pn_key"),
        },
        _ => false,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The two migration sets must stay in lockstep (same versions).
    #[test]
    fn migration_sets_share_versions() {
        let v = |m: &sqlx::migrate::Migrator| m.iter().map(|m| m.version).collect::<Vec<_>>();
        assert_eq!(v(&MIGRATOR), v(&POSTGRES_MIGRATOR));
    }
}
