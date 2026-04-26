-- WS-D: primitive tables for the v0.9 library refactor.
--
-- Per `v0.9-library-refactor-plan.md` §9 Step D1, each primitive type lives in
-- its own table keyed on `(library_id, uuid)`. Payload is the JSON-serialised
-- primitive struct (Symbol / Footprint / SimModel) — the server doesn't index
-- the inner shape; it relays bytes to/from clients with the lib_id+uuid tuple
-- as the lookup key.
--
-- Schema is portable between SQLite and Postgres: TEXT for UUIDs and JSON,
-- TIMESTAMPTZ-equivalent ISO-8601 strings, no JSONB / native uuid types.
-- Postgres deployments may upgrade columns to `uuid`/`jsonb` later via a
-- non-destructive ALTER without breaking the current schema.

CREATE TABLE IF NOT EXISTS symbols (
    library_id     TEXT NOT NULL,
    uuid           TEXT NOT NULL,
    name           TEXT NOT NULL,
    payload        TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, uuid)
);

CREATE INDEX IF NOT EXISTS symbols_name_idx ON symbols (library_id, name);

CREATE TABLE IF NOT EXISTS footprints (
    library_id     TEXT NOT NULL,
    uuid           TEXT NOT NULL,
    name           TEXT NOT NULL,
    payload        TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, uuid)
);

CREATE INDEX IF NOT EXISTS footprints_name_idx ON footprints (library_id, name);

CREATE TABLE IF NOT EXISTS sims (
    library_id     TEXT NOT NULL,
    uuid           TEXT NOT NULL,
    name           TEXT NOT NULL,
    payload        TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL,
    PRIMARY KEY (library_id, uuid)
);

CREATE INDEX IF NOT EXISTS sims_name_idx ON sims (library_id, name);
