-- #1613 — a part number is unique per library, enforced by the database.
--
-- `pn_key` is the trimmed `internal_pn` (Rust `str::trim`, the key the
-- client adapter compares), written by the server on every insert and
-- update. This migration only adds the column; `AppState::migrate` then
-- recomputes it in Rust from every stored row's payload and refuses to go on
-- while two rows of a library share a key (no row is changed or dropped),
-- and 0010 builds the unique index. Same DDL for SQLite and Postgres.

ALTER TABLE component_rows ADD COLUMN pn_key TEXT NOT NULL DEFAULT '';
