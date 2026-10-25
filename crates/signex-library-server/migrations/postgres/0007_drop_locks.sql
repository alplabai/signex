-- #115 — the advisory lock endpoints are gone; row writes are guarded by the
-- row revision (0006) instead. Drop the table nothing reads or writes.
-- Same DDL for SQLite and Postgres.

DROP TABLE IF EXISTS locks;
