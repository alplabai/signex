-- #1445 - optimistic concurrency for primitives (symbols, footprints, sims).
--
-- Same scheme as component rows (0006): revision starts at 1 and is bumped
-- by each update; a PUT names the revision it read (`If-Match`) and runs as
-- `... WHERE revision = <base>`. Existing primitives start at revision 1.
-- Same DDL for SQLite and Postgres.

ALTER TABLE symbols ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
ALTER TABLE footprints ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
ALTER TABLE sims ADD COLUMN revision BIGINT NOT NULL DEFAULT 1;
