-- v0.9 library refactor (WS-C step C3) — primitive storage tables.
--
-- Reusable shape primitives (Symbol/Footprint/SimModel) addressed by a
-- (library_id, uuid) tuple per `v0.9-refactor-2-plan.md` §2 / §8.
--
-- Mirrors the existing migrations' SQLite-friendly portability: stick to TEXT
-- for UUIDs and JSON payloads so the same DDL applies to both SQLite and
-- Postgres deployments. (`PRIMARY KEY (library_id, uuid)` indexes the lookup
-- key; the secondary name index covers the `list_symbols` / `list_footprints`
-- / `list_sims` UI surface that sorts alphabetically.)

CREATE TABLE IF NOT EXISTS symbols (
    library_id     TEXT NOT NULL,
    uuid           TEXT NOT NULL,
    name           TEXT NOT NULL,
    payload        TEXT NOT NULL,           -- full Symbol JSON
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, uuid)
);
CREATE INDEX IF NOT EXISTS idx_symbols_library_name ON symbols(library_id, name);

CREATE TABLE IF NOT EXISTS footprints (
    library_id     TEXT NOT NULL,
    uuid           TEXT NOT NULL,
    name           TEXT NOT NULL,
    payload        TEXT NOT NULL,           -- full Footprint JSON
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, uuid)
);
CREATE INDEX IF NOT EXISTS idx_footprints_library_name ON footprints(library_id, name);

CREATE TABLE IF NOT EXISTS sims (
    library_id     TEXT NOT NULL,
    uuid           TEXT NOT NULL,
    name           TEXT NOT NULL,
    payload        TEXT NOT NULL,           -- full SimModel JSON
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, uuid)
);
CREATE INDEX IF NOT EXISTS idx_sims_library_name ON sims(library_id, name);
