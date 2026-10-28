//! Component-row CRUD for [`AppState`]: insert, conditional update and
//! delete, fetch, and the table and row listings behind the `/tables` and
//! `/rows` routes.

use chrono::Utc;
use signex_library::component::ComponentRow;
use signex_library::identity::RowId;
use uuid::Uuid;

use super::pn_unique::pn_key;
use super::{
    AppState, DbPool, Page, RowInsert, RowWrite, is_pn_violation, is_unique_violation,
    read_stored_row, row_payload,
};

impl AppState {
    // ── Component-row CRUD ─────────────────────────────────────────────────
    //
    // `component_rows` is the unified DBLib row table. `(library_id,
    // table_name, row_id)` is the primary key — same shape across SQLite
    // and Postgres so the route handlers can stay backend-agnostic.
    //
    // The row body is round-tripped as JSON via `ComponentRow`'s serde impl
    // so a new `ComponentRow` field needs no SQL migration. New fields are
    // required in the JSON: rows stored before a field existed are not
    // supported.

    /// Insert a brand-new row. Returns `Ok(false)` when a row with the
    /// same `(library_id, table, row_id)` already exists so the caller
    /// can answer `409` — POST must never silently overwrite an
    /// existing row (an upsert would let one client clobber another's
    /// component with no warning). Replacement goes through
    /// [`update_row`] (PUT). A part number another row of the library holds
    /// is refused by the unique index, so racing writers cannot both win.
    pub async fn insert_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row: &ComponentRow,
    ) -> sqlx::Result<RowInsert> {
        signex_library::validate_row_for_write(row)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
        let payload = row_payload(row)?;
        let now = Utc::now().to_rfc3339();
        let row_id = row.row_id.to_string();
        let internal_pn = row.internal_pn.as_str().to_string();
        let key = pn_key(&internal_pn).to_string();
        let result = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO component_rows \
                       (library_id, table_name, row_id, internal_pn, pn_key, payload, created_at, updated_at) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&row_id)
                .bind(&internal_pn)
                .bind(&key)
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
                       (library_id, table_name, row_id, internal_pn, pn_key, payload, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&row_id)
                .bind(&internal_pn)
                .bind(&key)
                .bind(&payload)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map(|_| ())
            }
        };
        match result {
            Ok(_) => Ok(RowInsert::Created),
            Err(e) if is_pn_violation(&e) => Ok(RowInsert::PnTaken),
            Err(e) if is_unique_violation(&e) => Ok(RowInsert::RowExists),
            Err(e) => Err(e),
        }
    }

    /// Replace a row if it is still at `base_revision`, bumping the
    /// revision. The check and the write are one `UPDATE ... WHERE
    /// revision = ?`, so of two writers with the same base only one wins.
    pub async fn update_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row: &ComponentRow,
        base_revision: i64,
    ) -> sqlx::Result<RowWrite> {
        signex_library::validate_row_for_write(row)
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
        let payload = row_payload(row)?;
        let now = Utc::now().to_rfc3339();
        let row_id = row.row_id.to_string();
        let internal_pn = row.internal_pn.as_str().to_string();
        let key = pn_key(&internal_pn).to_string();
        let affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "UPDATE component_rows SET \
                         internal_pn = ?, pn_key = ?, payload = ?, updated_at = ?, revision = revision + 1 \
                     WHERE library_id = ? AND table_name = ? AND row_id = ? AND revision = ?",
            )
            .bind(&internal_pn)
            .bind(&key)
            .bind(&payload)
            .bind(&now)
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&row_id)
            .bind(base_revision)
            .execute(pool)
            .await
            .map(|r| r.rows_affected()),
            DbPool::Postgres(pool) => sqlx::query(
                "UPDATE component_rows SET \
                         internal_pn = $1, pn_key = $2, payload = $3, updated_at = $4, revision = revision + 1 \
                     WHERE library_id = $5 AND table_name = $6 AND row_id = $7 AND revision = $8",
            )
            .bind(&internal_pn)
            .bind(&key)
            .bind(&payload)
            .bind(&now)
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&row_id)
            .bind(base_revision)
            .execute(pool)
            .await
            .map(|r| r.rows_affected()),
        };
        let affected = match affected {
            Ok(n) => n,
            Err(e) if is_pn_violation(&e) => return Ok(RowWrite::PnTaken),
            Err(e) => return Err(e),
        };
        if affected > 0 {
            return Ok(RowWrite::Applied {
                revision: base_revision + 1,
            });
        }
        self.refused_write(library_id, table_name, &row_id).await
    }

    /// Why a conditional write matched no row: the row is gone, or it has
    /// moved past the caller's base revision.
    async fn refused_write(
        &self,
        library_id: Uuid,
        table_name: &str,
        row_id: &str,
    ) -> sqlx::Result<RowWrite> {
        let current: Option<i64> = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_scalar(
                    "SELECT revision FROM component_rows \
                 WHERE library_id = ? AND table_name = ? AND row_id = ?",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(row_id)
                .fetch_optional(pool)
                .await?
            }
            DbPool::Postgres(pool) => {
                sqlx::query_scalar(
                    "SELECT revision FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 AND row_id = $3",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(row_id)
                .fetch_optional(pool)
                .await?
            }
        };
        Ok(match current {
            Some(current_revision) => RowWrite::Stale { current_revision },
            None => RowWrite::Missing,
        })
    }

    pub async fn fetch_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row_id: RowId,
    ) -> sqlx::Result<Option<ComponentRow>> {
        let id_str = row_id.to_string();
        let stored: Option<(String, i64)> = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_as(
                    "SELECT payload, revision FROM component_rows \
                 WHERE library_id = ? AND table_name = ? AND row_id = ?",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&id_str)
                .fetch_optional(pool)
                .await?
            }
            DbPool::Postgres(pool) => {
                sqlx::query_as(
                    "SELECT payload, revision FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 AND row_id = $3",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(&id_str)
                .fetch_optional(pool)
                .await?
            }
        };
        stored.map(read_stored_row).transpose()
    }

    /// Delete a row if it is still at `base_revision`; conditional the
    /// same way as [`Self::update_row`].
    pub async fn delete_row(
        &self,
        library_id: Uuid,
        table_name: &str,
        row_id: RowId,
        base_revision: i64,
    ) -> sqlx::Result<RowWrite> {
        let id_str = row_id.to_string();
        let affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query(
                "DELETE FROM component_rows \
                 WHERE library_id = ? AND table_name = ? AND row_id = ? AND revision = ?",
            )
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&id_str)
            .bind(base_revision)
            .execute(pool)
            .await?
            .rows_affected(),
            DbPool::Postgres(pool) => sqlx::query(
                "DELETE FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 AND row_id = $3 AND revision = $4",
            )
            .bind(library_id.to_string())
            .bind(table_name)
            .bind(&id_str)
            .bind(base_revision)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        if affected > 0 {
            return Ok(RowWrite::Applied {
                revision: base_revision,
            });
        }
        self.refused_write(library_id, table_name, &id_str).await
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

    /// Read one [`Page`] of the rows in `table_name` for `library_id`,
    /// ordered by `internal_pn` (then `row_id`, so pages never overlap).
    pub async fn list_rows_in_table(
        &self,
        library_id: Uuid,
        table_name: &str,
        page: Page,
    ) -> sqlx::Result<Vec<ComponentRow>> {
        let stored: Vec<(String, i64)> = match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query_as(
                    "SELECT payload, revision FROM component_rows \
                 WHERE library_id = ? AND table_name = ? \
                 ORDER BY internal_pn, row_id LIMIT ? OFFSET ?",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(page.limit)
                .bind(page.offset)
                .fetch_all(pool)
                .await?
            }
            DbPool::Postgres(pool) => {
                sqlx::query_as(
                    "SELECT payload, revision FROM component_rows \
                 WHERE library_id = $1 AND table_name = $2 \
                 ORDER BY internal_pn, row_id LIMIT $3 OFFSET $4",
                )
                .bind(library_id.to_string())
                .bind(table_name)
                .bind(page.limit)
                .bind(page.offset)
                .fetch_all(pool)
                .await?
            }
        };
        stored.into_iter().map(read_stored_row).collect()
    }
}
