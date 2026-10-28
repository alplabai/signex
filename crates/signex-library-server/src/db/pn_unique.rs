//! Explains a failed migration that stopped on duplicate part numbers.

use super::{AppState, DbPool};

impl AppState {
    /// A migration error as the operator should see it. When the unique
    /// part-number index could not be built because a library already holds
    /// the same PN twice, name the clashing PNs; no row is changed or
    /// dropped, the operator renumbers them and starts the server again.
    pub(super) async fn explain_migration_failure(
        &self,
        err: sqlx::migrate::MigrateError,
    ) -> sqlx::Error {
        let sql = "SELECT library_id, TRIM(internal_pn), COUNT(*) FROM component_rows \
                   WHERE TRIM(internal_pn) <> '' \
                   GROUP BY library_id, TRIM(internal_pn) HAVING COUNT(*) > 1 \
                   ORDER BY library_id, 2 LIMIT 20";
        let clashes: Result<Vec<(String, String, i64)>, _> = match &self.pool {
            DbPool::Sqlite(p) => sqlx::query_as(sql).fetch_all(p).await,
            DbPool::Postgres(p) => sqlx::query_as(sql).fetch_all(p).await,
        };
        match clashes {
            Ok(rows) if !rows.is_empty() => {
                let list: Vec<String> = rows
                    .iter()
                    .map(|(lib, pn, n)| format!("{pn:?} x{n} in library {lib}"))
                    .collect();
                sqlx::Error::Protocol(format!(
                    "part numbers must be unique per library, but these are held by more than \
                     one row (first {}): {}. Renumber the duplicates, then start the server \
                     again; no row was changed. Migration error: {err}",
                    rows.len(),
                    list.join("; ")
                ))
            }
            _ => err.into(),
        }
    }
}
