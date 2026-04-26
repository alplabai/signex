-- Signex library DB-flavour initial schema.
--
-- Tables (per LIBRARY_PLAN §7 + WS-B contract):
--   components        — one row per logical component (uuid, internal_pn, head version)
--   revisions         — one row per (component_uuid, version) — full revision JSON blob
--   parameters        — flattened (component_uuid, version, key) for fast facet queries
--   suppliers         — flattened supplier links per revision
--   lifecycle_log     — append-only audit of state transitions
--   locks             — advisory locks per (uuid, field_set) with idle TTL
--   review_requests   — submitted-but-not-yet-released revisions awaiting reviewer signoff
--
-- Schema is portable between SQLite and Postgres: text + integer + timestamp text.
-- Postgres-specific types (uuid, jsonb) are intentionally NOT used here so the
-- same migration applies to both backends.

CREATE TABLE IF NOT EXISTS components (
    uuid           TEXT PRIMARY KEY NOT NULL,
    internal_pn    TEXT NOT NULL,
    head_major     INTEGER NOT NULL,
    head_minor     INTEGER NOT NULL,
    created        TEXT NOT NULL,
    updated        TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_components_internal_pn ON components(internal_pn);

CREATE TABLE IF NOT EXISTS revisions (
    component_uuid TEXT NOT NULL,
    major          INTEGER NOT NULL,
    minor          INTEGER NOT NULL,
    state          TEXT NOT NULL,
    author         TEXT NOT NULL,
    message        TEXT NOT NULL,
    created        TEXT NOT NULL,
    content_hash   TEXT NOT NULL,
    payload        TEXT NOT NULL,           -- full Revision JSON
    PRIMARY KEY (component_uuid, major, minor),
    FOREIGN KEY (component_uuid) REFERENCES components(uuid) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_revisions_state ON revisions(state);

CREATE TABLE IF NOT EXISTS parameters (
    component_uuid TEXT NOT NULL,
    major          INTEGER NOT NULL,
    minor          INTEGER NOT NULL,
    side           TEXT NOT NULL,           -- 'shared' | 'schematic' | 'pcb'
    key            TEXT NOT NULL,
    value          TEXT NOT NULL,
    PRIMARY KEY (component_uuid, major, minor, side, key),
    FOREIGN KEY (component_uuid, major, minor)
        REFERENCES revisions(component_uuid, major, minor) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_parameters_key ON parameters(key);

CREATE TABLE IF NOT EXISTS suppliers (
    component_uuid TEXT NOT NULL,
    major          INTEGER NOT NULL,
    minor          INTEGER NOT NULL,
    distributor    TEXT NOT NULL,
    sku            TEXT NOT NULL,
    PRIMARY KEY (component_uuid, major, minor, distributor, sku),
    FOREIGN KEY (component_uuid, major, minor)
        REFERENCES revisions(component_uuid, major, minor) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_suppliers_distributor_sku ON suppliers(distributor, sku);

CREATE TABLE IF NOT EXISTS lifecycle_log (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    component_uuid TEXT NOT NULL,
    major          INTEGER NOT NULL,
    minor          INTEGER NOT NULL,
    from_state     TEXT,                    -- NULL on initial create
    to_state       TEXT NOT NULL,
    actor          TEXT NOT NULL,
    occurred       TEXT NOT NULL,
    note           TEXT
);

CREATE INDEX IF NOT EXISTS idx_lifecycle_log_component ON lifecycle_log(component_uuid);

CREATE TABLE IF NOT EXISTS locks (
    component_uuid TEXT NOT NULL,
    field_set      TEXT NOT NULL,
    holder         TEXT NOT NULL,
    acquired       TEXT NOT NULL,
    last_renewed   TEXT NOT NULL,
    PRIMARY KEY (component_uuid, field_set)
);

CREATE TABLE IF NOT EXISTS review_requests (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    component_uuid TEXT NOT NULL,
    major          INTEGER NOT NULL,
    minor          INTEGER NOT NULL,
    submitter      TEXT NOT NULL,
    submitted      TEXT NOT NULL,
    state          TEXT NOT NULL,           -- 'pending' | 'approved' | 'rejected'
    reviewer       TEXT,
    decided        TEXT,
    note           TEXT,
    FOREIGN KEY (component_uuid, major, minor)
        REFERENCES revisions(component_uuid, major, minor) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_review_requests_state ON review_requests(state);
