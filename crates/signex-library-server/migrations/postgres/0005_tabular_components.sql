-- v0.9-refactor-2 (WS-3 / WS-4) — DBLib row model.
--
-- Per `v0.9-refactor-2-plan.md` §2.1, components are now rows inside category
-- tables (Altium DBLib parity). The legacy per-revision schema in
-- `0001_initial.sql` and `004_primitives.sql` (components / revisions /
-- parameters / suppliers) is superseded by a single `component_rows` table
-- whose payload is the JSON-serialised `ComponentRow` struct from
-- `signex-library`.
--
-- Layout intentionally mirrors the WS-D primitives tables: `(library_id, …)`
-- is the partition key, the row's `row_id` (UUIDv7 stringified) is the
-- intra-table identifier, and `payload` carries the canonical JSON. The
-- `table_name` column groups rows by category (e.g. "resistors",
-- "Discrete_Passives") matching the LocalGit `tables/<name>.tsv` filename
-- stem from `Manifest::table_for_class`.
--
-- Schema portability: same DDL applies to SQLite and Postgres — TEXT for
-- UUIDs, JSON, and timestamps. Postgres-native `uuid`/`jsonb` are deferred
-- until the cross-backend test matrix is in CI.

CREATE TABLE IF NOT EXISTS component_rows (
    library_id     TEXT NOT NULL,
    table_name     TEXT NOT NULL,
    row_id         TEXT NOT NULL,
    internal_pn    TEXT NOT NULL,
    payload        TEXT NOT NULL,           -- full ComponentRow JSON
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, table_name, row_id)
);

-- The picker UI sorts rows alphabetically by internal_pn within a table; a
-- composite index over (library_id, table_name, internal_pn) covers that
-- access pattern without forcing a full scan + in-memory sort.
CREATE INDEX IF NOT EXISTS idx_component_rows_pn
    ON component_rows(library_id, table_name, internal_pn);

-- `GET /tables` runs `SELECT DISTINCT table_name FROM component_rows
-- WHERE library_id = ?`; the index above starts on `library_id` so the
-- distinct prefix scan is index-only.
