# signex-library-server

HTTP service that hosts Signex component libraries (rows, symbols,
footprints, simulation models) for the database library adapter in
`signex-library`.

## Running

```sh
cargo run -p signex-library-server
```

With no configuration it listens on `127.0.0.1:3535`, with no token and an
in-memory SQLite database. That is a local development mode: **every row is
lost when the process stops.**

## Configuration

| Variable | Meaning |
|---|---|
| `SIGNEX_LIBRARY_BIND` | `host:port` to listen on. Default `127.0.0.1:3535`. |
| `SIGNEX_API_TOKEN` | Bearer token required on every protected route. Unset or empty: no authentication. |
| `SIGNEX_DATABASE_URL` | Persistent database, `postgres://…` or `sqlite://<file>`. Unset: ephemeral in-memory SQLite. Migrations run at startup. |
| `SIGNEX_ALLOW_INSECURE_BIND` | `1` / `true` / `yes` (or the `--allow-insecure-bind` flag) allows a non-loopback bind. The server has no TLS. |
| `SIGNEX_TRUSTED_PROXIES` | Comma-separated proxy IPs whose forwarding header the rate limiter believes. |
| `SIGNEX_TRUSTED_PROXY_HEADER` | `x-forwarded-for` or `forwarded`: the header that proxy writes. Required when `SIGNEX_TRUSTED_PROXIES` is set. |
| `RUST_LOG` | Log filter (`tracing` env filter). |

A non-loopback bind is refused at startup unless all three hold:
`SIGNEX_ALLOW_INSECURE_BIND` is on, `SIGNEX_API_TOKEN` is set and
`SIGNEX_DATABASE_URL` is set.

## Deploying

The server speaks plain HTTP. Put it behind a reverse proxy that terminates
TLS, and either bind the server to loopback on the proxy host or allow the
insecure bind only on an interface the proxy alone can reach. Set
`SIGNEX_TRUSTED_PROXIES` / `SIGNEX_TRUSTED_PROXY_HEADER` to the proxy, or
every client shares one rate-limit bucket.

## Part numbers are unique per library

A part number (`internal_pn`, compared with surrounding white space removed and
case-sensitively) is held by at most one row in a library, across all of its
tables; a blank one is exempt. The database enforces it with a unique index on
`component_rows.pn_key`, so two writers racing the same new number end with one
row and one `409`.

On start-up the server recomputes `pn_key` from the part number in every
stored row's payload and builds the index (migration 0010). If a library
already holds the same number twice it refuses to start, names each clash
(`table/row_id`) and changes nothing. Fix it by giving all but one row of each
clash a new number **in the payload**, which is what readers return, and in the
`internal_pn` column, then start again:

```sql
-- SQLite
UPDATE component_rows
   SET payload = json_set(payload, '$.internal_pn', 'NEW-PN'), internal_pn = 'NEW-PN'
 WHERE library_id = '...' AND table_name = '...' AND row_id = '...';

-- Postgres
UPDATE component_rows
   SET payload = jsonb_set(payload::jsonb, '{internal_pn}', to_jsonb('NEW-PN'::text))::text,
       internal_pn = 'NEW-PN'
 WHERE library_id = '...' AND table_name = '...' AND row_id = '...';
```

## Known limitations (Community Preview)

- **One shared token.** Every client uses the same `SIGNEX_API_TOKEN`; there
  are no per-user or per-library credentials and no audit of who changed a
  row. Anyone holding the token can read and write every library on the
  server. Treat it like a shared password, rotate it by restarting with a new
  value, and keep the server on a network you trust. Per-client credentials
  are planned after v1.0.
- **No built-in TLS** (see Deploying).
