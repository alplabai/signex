//! Part-number uniqueness: the key every writer normalises to, and the
//! start-up step between migration 0009 (adds `pn_key`) and 0010 (unique
//! index on it).

use std::borrow::Cow;
use std::collections::BTreeMap;

use sqlx::migrate::Migrator;

use super::{AppState, DbPool, MIGRATOR, POSTGRES_MIGRATOR};

/// Name of the unique index migration 0010 builds.
pub(super) const PN_INDEX: &str = "idx_component_rows_pn_key";

/// Migration that adds the `pn_key` column.
pub(super) const KEY_COLUMN_VERSION: i64 = 9;

/// Migration that builds the unique index.
const INDEX_VERSION: i64 = 10;

/// The key a part number is unique by: trimmed with Rust's `str::trim`
/// (all Unicode white space), the same rule the client adapter applies. The
/// one place that rule lives on the server.
pub(super) fn pn_key(internal_pn: &str) -> &str {
    internal_pn.trim()
}

type StoredRow = (String, String, String, String, String);
type StoredKeys = Vec<StoredRow>;

impl AppState {
    /// Run the embedded migrations up to and including `max_version`.
    pub(super) async fn migrate_through(&self, max_version: i64) -> sqlx::Result<()> {
        let all = match &self.pool {
            DbPool::Sqlite(_) => &MIGRATOR,
            DbPool::Postgres(_) => &POSTGRES_MIGRATOR,
        };
        let migrator = Migrator {
            migrations: Cow::Owned(
                all.migrations
                    .iter()
                    .filter(|m| m.version <= max_version)
                    .cloned()
                    .collect(),
            ),
            // Later migrations may already be applied.
            ignore_missing: true,
            locking: all.locking,
            no_tx: all.no_tx,
        };
        match &self.pool {
            DbPool::Sqlite(p) => migrator.run(p).await?,
            DbPool::Postgres(p) => migrator.run(p).await?,
        }
        Ok(())
    }

    /// Before the unique index exists: recompute every row's `pn_key` from
    /// the part number in its payload (what every read returns), and refuse
    /// if a library holds the same key twice. Nothing is dropped; a refusal
    /// changes no row.
    pub(super) async fn refuse_duplicate_part_numbers(&self) -> sqlx::Result<()> {
        if self.index_migration_applied().await? {
            return Ok(());
        }
        let stored = self.stored_part_numbers().await?;
        let mut by_key: BTreeMap<(&str, &str), Vec<&StoredRow>> = BTreeMap::new();
        let keys: Vec<String> = stored
            .iter()
            .map(|(.., payload, _)| payload_key(payload))
            .collect();
        for (row, key) in stored.iter().zip(&keys) {
            if !key.is_empty() {
                by_key.entry((&row.0, key)).or_default().push(row);
            }
        }
        let clashes: Vec<String> = by_key
            .iter()
            .filter(|(_, rows)| rows.len() > 1)
            .take(20)
            .map(|((library, key), rows)| {
                let at: Vec<String> = rows.iter().map(|r| format!("{}/{}", r.1, r.2)).collect();
                format!("{key:?} in library {library}: {}", at.join(", "))
            })
            .collect();
        if !clashes.is_empty() {
            return Err(sqlx::Error::Protocol(format!(
                "part numbers must be unique per library, but these are held by more than one \
                 row (table/row_id, first {}): {}. No row was changed. Give all but one of each \
                 a new part number in the row payload, which is what readers return (also keep \
                 the internal_pn column in step), then start the server again. SQLite: UPDATE \
                 component_rows SET payload = json_set(payload, '$.internal_pn', 'NEW-PN'), \
                 internal_pn = 'NEW-PN' WHERE library_id = '...' AND table_name = '...' AND \
                 row_id = '...'. Postgres: UPDATE component_rows SET payload = \
                 jsonb_set(payload::jsonb, '{{internal_pn}}', to_jsonb('NEW-PN'::text))::text, \
                 internal_pn = 'NEW-PN' WHERE library_id = '...' AND table_name = '...' AND \
                 row_id = '...'. The key is recomputed from the payload at the next start.",
                clashes.len(),
                clashes.join("; ")
            )));
        }
        for (row, key) in stored.iter().zip(&keys) {
            if row.4 != *key {
                self.set_pn_key(&row.0, &row.1, &row.2, key).await?;
            }
        }
        Ok(())
    }

    async fn index_migration_applied(&self) -> sqlx::Result<bool> {
        let n: i64 = match &self.pool {
            DbPool::Sqlite(p) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = ?")
                    .bind(INDEX_VERSION)
                    .fetch_one(p)
                    .await?
            }
            DbPool::Postgres(p) => {
                sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = $1")
                    .bind(INDEX_VERSION)
                    .fetch_one(p)
                    .await?
            }
        };
        Ok(n > 0)
    }

    /// `(library_id, table_name, row_id, payload, pn_key)` of every row.
    async fn stored_part_numbers(&self) -> sqlx::Result<StoredKeys> {
        let sql = "SELECT library_id, table_name, row_id, payload, pn_key FROM component_rows \
                   ORDER BY library_id, table_name, row_id";
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as(sql).fetch_all(p).await,
            DbPool::Postgres(p) => sqlx::query_as(sql).fetch_all(p).await,
        }
    }

    async fn set_pn_key(
        &self,
        library: &str,
        table: &str,
        row: &str,
        key: &str,
    ) -> sqlx::Result<()> {
        match &self.pool {
            DbPool::Sqlite(p) => {
                sqlx::query(
                    "UPDATE component_rows SET pn_key = ? \
                     WHERE library_id = ? AND table_name = ? AND row_id = ?",
                )
                .bind(key)
                .bind(library)
                .bind(table)
                .bind(row)
                .execute(p)
                .await?;
            }
            DbPool::Postgres(p) => {
                sqlx::query(
                    "UPDATE component_rows SET pn_key = $1 \
                     WHERE library_id = $2 AND table_name = $3 AND row_id = $4",
                )
                .bind(key)
                .bind(library)
                .bind(table)
                .bind(row)
                .execute(p)
                .await?;
            }
        }
        Ok(())
    }
}

/// The key of the part number in a stored payload; blank when absent.
fn payload_key(payload: &str) -> String {
    serde_json::from_str::<serde_json::Value>(payload)
        .ok()
        .and_then(|v| v["internal_pn"].as_str().map(|pn| pn_key(pn).to_string()))
        .unwrap_or_default()
}
