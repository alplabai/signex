# The bundled stock library

`assets/stock-library/` is the library every Signex install ships: the
resistors, capacitors, connectors, 74HC logic and so on that the
Components panel lists under **Stock**. Installers copy it next to the
program (`<exe dir>\library\Stock.snxlib` on Windows,
`../share/signex/library/` on Linux, `../Resources/library/` on macOS).

## Users: read-only

An installed Signex always mounts the stock library read-only. Opening a
stock symbol or footprint for editing goes to **Copy to Library** (#813):
the copy lands in a library of the user's own and the stock file is never
written.

## Maintainers: edit it in Signex (#1892)

The committed files are the source of truth. To change what ships:

1. Run a build from a source checkout with `SIGNEX_EDIT_STOCK=1`, e.g.
   `SIGNEX_EDIT_STOCK=1 target/release/signex` (PowerShell:
   `$env:SIGNEX_EDIT_STOCK = "1"; target\release\signex.exe`).
   Signex mounts `assets/stock-library/Stock.snxlib` writable and logs a
   warning saying so. The variable is ignored for an installed copy: only
   a path ending in `assets/stock-library/Stock.snxlib` qualifies, so an
   update can never overwrite maintainer edits.
2. Edit the stock symbols, footprints and component rows in the normal
   editors and save. Saves write straight into the checkout.
3. Commit the changed files under `assets/stock-library/` on a branch and
   open a PR, with a `changelog.d/` fragment.

Recovery actions that write outside a save (git re-init, binding repair)
stay refused on the stock library in this mode.

## What the tests protect

Designs reference stock parts by `library_id` + primitive / row uuid, so
`crates/signex-app/tests/integration/stock_library_determinism.rs` and
`stock_library_rows.rs` check the committed files, not the generator:

- the library id stays `019df45d-af5e-7a00-8c3b-dd3bf89b40ae`;
- every pinned footprint, symbol and row id is still present (adding parts
  is fine, removing a shipped one fails);
- the library opens through the real adapter, every symbol round-trips
  byte-identically, pins sit on the schematic grid, and the 74HC and
  connector shape rules hold for any part, old or new.

Never copy the folder to make a "second stock library": two mounted
libraries may not share an id (#1894), so the copy is refused.

## The seed generator

`crates/signex-app/examples/build_stock_library/` wrote the first version
of these files and still pins their ids (`ids.rs`, `rows.rs`). Rerunning
it would discard every edit made in Signex, so it refuses to write while
`Stock.snxlib` exists unless you pass `--force`:

```text
cargo run --example build_stock_library -p signex-app -- --force
```
