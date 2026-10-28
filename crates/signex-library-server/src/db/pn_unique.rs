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

    /// On every start, before the unique index is built (and again once it
    /// exists, because an older server replica writing during an upgrade
    /// leaves blank or stale keys): recompute every row's `pn_key` from the
    /// part number in its payload (what every read returns), and refuse if a
    /// library holds the same key twice. A stale key with no clash is
    /// repaired. Nothing is dropped; a refusal changes no row.
    pub(super) async fn refuse_duplicate_part_numbers(&self) -> sqlx::Result<()> {
        tracing::info!(
            "checking part-number keys; stop every library server before upgrading to a version \
             with unique part numbers, an older replica writing meanwhile leaves keys unchecked"
        );
        let stored = self.stored_part_numbers().await?;
        let mut by_key: BTreeMap<(&str, &str), Vec<&StoredRow>> = BTreeMap::new();
        let keys: Vec<String> = stored.iter().map(payload_key).collect();
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
                 the internal_pn column in step, and bump revision and updated_at so open \
                 copies reload), then start the server again. SQLite: UPDATE component_rows \
                 SET payload = json_set(payload, '$.internal_pn', 'NEW-PN'), \
                 internal_pn = 'NEW-PN', revision = revision + 1, \
                 updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE library_id = '...' \
                 AND table_name = '...' AND row_id = '...'. Postgres: UPDATE component_rows SET \
                 payload = jsonb_set(payload::jsonb, '{{internal_pn}}', \
                 to_jsonb('NEW-PN'::text))::text, internal_pn = 'NEW-PN', \
                 revision = revision + 1, updated_at = to_char(now() AT TIME ZONE 'utc', \
                 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') WHERE library_id = '...' AND \
                 table_name = '...' AND row_id = '...'. The key is recomputed from the payload \
                 at the next start.",
                clashes.len(),
                clashes.join("; ")
            )));
        }
        let stale: Vec<_> = stored
            .iter()
            .zip(&keys)
            .filter(|(r, k)| r.4 != **k)
            .collect();
        // Blank first (blank keys never clash), so keys that swap places
        // cannot collide on the way.
        for (row, _) in &stale {
            self.set_pn_key(&row.0, &row.1, &row.2, "").await?;
        }
        for (row, key) in &stale {
            tracing::warn!(
                library_id = %row.0, table = %row.1, row_id = %row.2,
                "repairing a stale part-number key"
            );
            self.set_pn_key(&row.0, &row.1, &row.2, key).await?;
        }
        Ok(())
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

/// The key of the part number in a stored payload; blank (with a warning
/// naming the row) when the payload does not parse or has no `internal_pn`.
fn payload_key(row: &StoredRow) -> String {
    let key = serde_json::from_str::<serde_json::Value>(&row.3)
        .ok()
        .and_then(|v| v["internal_pn"].as_str().map(|pn| pn_key(pn).to_string()));
    key.unwrap_or_else(|| {
        tracing::warn!(
            library_id = %row.0, table = %row.1, row_id = %row.2,
            "stored row has no readable internal_pn; its part-number key is blank"
        );
        String::new()
    })
}
