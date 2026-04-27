-- 0005_tabular_components.sql
--
-- v0.9-refactor-2 step 3.1 — DBLib row model. Replaces the v0.9-original
-- `revisions` + `components` tables with a generic component_rows table
-- (`library_id`, `table_name`, `row_id`, `internal_pn`, `payload` JSON,
-- `created_at`, `updated_at`) plus `component_tables` for per-table config.
--
-- Schema parity with the LocalGit TSV layout (see `signex_library::tables`)
-- — the same `ComponentRow` JSON serialises into the `payload` column on
-- both backends.
--
-- Portable across SQLite + Postgres: only TEXT columns are used and the
-- `payload` JSON is stored as TEXT (not jsonb) so SQLite tests stay
-- representative of production behaviour.
--
-- The `revisions` and `components` tables are dropped here — they were
-- introduced by `0001_initial.sql` for the per-component-revision-chain
-- model that the DBLib refactor supersedes. SQLite's `DROP TABLE IF EXISTS`
-- is a no-op when the tables are absent (e.g., on a clean install where
-- v0.9-refactor-2 is the first build to ever touch the DB).

DROP TABLE IF EXISTS revisions;
DROP TABLE IF EXISTS components;

CREATE TABLE component_rows (
    library_id      TEXT NOT NULL,
    table_name      TEXT NOT NULL,
    row_id          TEXT NOT NULL,
    internal_pn     TEXT NOT NULL,
    payload         TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (library_id, table_name, row_id)
);

CREATE INDEX component_rows_pn_idx
    ON component_rows (library_id, internal_pn);
CREATE INDEX component_rows_table_idx
    ON component_rows (library_id, table_name);

CREATE TABLE component_tables (
    library_id      TEXT NOT NULL,
    name            TEXT NOT NULL,
    classes_json    TEXT NOT NULL,
    PRIMARY KEY (library_id, name)
);
