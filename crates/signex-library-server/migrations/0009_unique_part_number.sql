-- #1613 — a part number is unique per library, enforced by the database.
--
-- `pn_key` is the trimmed `internal_pn` (the same key the client adapter
-- compares), written by the server on every insert and update. A blank key
-- (an unfilled draft) is exempt, and the index spans every table of a
-- library, as the adapter's check does, so two writers racing the same new
-- PN cannot both win. Case-sensitive.
--
-- Existing rows are backfilled, never rewritten or dropped. If a library
-- already holds two rows with the same PN the unique index cannot be built,
-- the migration fails and rolls back, and `migrate()` reports the clashing
-- PNs so an operator can renumber them and retry. Same DDL for SQLite and
-- Postgres.

ALTER TABLE component_rows ADD COLUMN pn_key TEXT NOT NULL DEFAULT '';

UPDATE component_rows SET pn_key = TRIM(internal_pn);

CREATE UNIQUE INDEX idx_component_rows_pn_key
    ON component_rows(library_id, pn_key)
    WHERE pn_key <> '';
