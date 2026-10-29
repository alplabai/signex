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

/// `(library_id, table_name, row_id, internal_pn from the payload, pn_key,
/// revision)` of a row as the start-up scan saw it.
type StoredRow = (String, String, String, Option<String>, String, i64);
type StoredKeys = Vec<StoredRow>;
/// A row whose key must change, and the key it should have.
type Stale<'a> = (&'a StoredRow, &'a String);

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
                 updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now') WHERE library_id = '...' \
                 AND table_name = '...' AND row_id = '...'. Postgres: UPDATE component_rows SET \
                 payload = jsonb_set(payload::jsonb, '{{internal_pn}}', \
                 to_jsonb('NEW-PN'::text))::text, internal_pn = 'NEW-PN', \
                 revision = revision + 1, updated_at = to_char(now() AT TIME ZONE 'utc', \
                 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"+00:00\"') WHERE library_id = '...' AND \
                 table_name = '...' AND row_id = '...'. The key is recomputed from the payload \
                 at the next start.",
                clashes.len(),
                clashes.join("; ")
            )));
        }
        let stale: Vec<Stale> = stored
            .iter()
            .zip(&keys)
            .filter(|(r, k)| r.4 != **k)
            .collect();
        if !stale.is_empty() {
            tracing::warn!(
                "repairing {} stale part-number keys: a library server of an older version was \
                 writing; stop every library server before upgrading to this version",
                stale.len()
            );
            self.repair_part_number_keys(&stale).await?;
        }
        Ok(())
    }

    /// Set each stale row's key, all in one transaction so a concurrent
    /// insert of the same part number waits and then gets its 409. A row
    /// that changed since the scan (its revision moved) is skipped; the next
    /// start checks it again. Blank first (blank keys never clash), so keys
    /// that swap places cannot collide on the way.
    pub(super) async fn repair_part_number_keys(&self, stale: &[Stale<'_>]) -> sqlx::Result<()> {
        macro_rules! repair {
            ($pool:expr, $sql:expr) => {{
                let mut tx = $pool.begin().await?;
                for repair_pass in [false, true] {
                    for (row, key) in stale {
                        let value = if repair_pass { key.as_str() } else { "" };
                        let done = sqlx::query($sql)
                            .bind(value)
                            .bind(&row.0)
                            .bind(&row.1)
                            .bind(&row.2)
                            .bind(row.5)
                            .execute(&mut *tx)
                            .await?;
                        if repair_pass && done.rows_affected() == 0 {
                            tracing::warn!(
                                library_id = %row.0, table = %row.1, row_id = %row.2,
                                "row changed since the part-number scan; left for the next start"
                            );
                        }
                    }
                }
                tx.commit().await?;
            }};
        }
        match &self.pool {
            DbPool::Sqlite(p) => repair!(
                p,
                "UPDATE component_rows SET pn_key = ? \
                 WHERE library_id = ? AND table_name = ? AND row_id = ? AND revision = ?"
            ),
            DbPool::Postgres(p) => repair!(
                p,
                "UPDATE component_rows SET pn_key = $1 \
                 WHERE library_id = $2 AND table_name = $3 AND row_id = $4 AND revision = $5"
            ),
        }
        Ok(())
    }

    /// The part number in every row's payload, with its key and revision.
    pub(super) async fn stored_part_numbers(&self) -> sqlx::Result<StoredKeys> {
        match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as(
                "SELECT library_id, table_name, row_id, json_extract(payload, '$.internal_pn'), \
                     pn_key, revision FROM component_rows ORDER BY library_id, table_name, row_id",
            )
            .fetch_all(p)
            .await,
            DbPool::Postgres(p) => {
                sqlx::query_as(
                    "SELECT library_id, table_name, row_id, payload::jsonb->>'internal_pn', \
                     pn_key, revision FROM component_rows ORDER BY library_id, table_name, row_id",
                )
                .fetch_all(p)
                .await
            }
        }
    }
}

/// The key of a row's part number; blank (with a warning naming the row)
/// when the payload has no `internal_pn`.
fn payload_key(row: &StoredRow) -> String {
    match &row.3 {
        Some(pn) => pn_key(pn).to_string(),
        None => {
            tracing::warn!(
                library_id = %row.0, table = %row.1, row_id = %row.2,
                "stored row has no readable internal_pn; its part-number key is blank"
            );
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row that changed after the scan keeps what the writer gave it.
    #[tokio::test]
    async fn a_row_changed_after_the_scan_is_not_overwritten_by_the_repair() {
        let state = AppState::new_sqlite_memory().await.unwrap();
        state.migrate().await.unwrap();
        let pool = state.pool().sqlite().unwrap();
        sqlx::query(
            "INSERT INTO component_rows \
             (library_id, table_name, row_id, internal_pn, payload, created_at, updated_at) \
             VALUES ('lib', 't', 'r', 'A', '{\"internal_pn\":\"A\"}', 'x', 'x')",
        )
        .execute(pool)
        .await
        .unwrap();

        let stored = state.stored_part_numbers().await.unwrap();
        let key = pn_key("A").to_string();
        assert_eq!(stored[0].4, "");
        // A writer saves the row between the scan and the repair.
        sqlx::query(
            "UPDATE component_rows SET pn_key = 'B', revision = revision + 1, \
             payload = '{\"internal_pn\":\"B\"}' WHERE row_id = 'r'",
        )
        .execute(pool)
        .await
        .unwrap();
        state
            .repair_part_number_keys(&[(&stored[0], &key)])
            .await
            .unwrap();
        let kept: String = sqlx::query_scalar("SELECT pn_key FROM component_rows")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(kept, "B");
    }
}
