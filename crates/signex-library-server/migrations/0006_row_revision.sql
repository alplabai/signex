-- #115 — optimistic concurrency for component rows.
--
-- Every row carries a revision, starting at 1 and bumped by each update.
-- A PUT or DELETE names the revision it was based on (`If-Match`); the
-- write runs as `... WHERE revision = <base>`, so a row someone else saved
-- in the meantime is refused instead of overwritten. Existing rows start
-- at revision 1. Same DDL for SQLite and Postgres.

ALTER TABLE component_rows ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
