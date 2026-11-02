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

## Chip passives

R, C, L, Ferrite and Fuse default to `Chip-0805`; D, D_Schottky, D_Zener
and LED default to `Chip-0805-Polarised` (pad 1 = cathode, silkscreen
bar). Each row also lists the sizes 0201, 0402, 0603, 1206, 1210 and 2512
(the matching `-Polarised` ones for the diodes) as alternate footprints,
so the footprint picker offers them without a pin-map override: pads 1 and
2 match the symbol pins.

IPC-7351 itself gives fillet goals, not fixed per-size lands, so the
reference is a manufacturer's IPC-7351 Density Level B table. Sources:

- 0201 to 1210: KEMET, "Surface Mount Multilayer Ceramic Chip Capacitors
  (SMD MLCCs) - C0G Dielectric, 10-250 VDC (Commercial Grade)", C1003_C0G,
  3/30/2016, p.10, "Table 3 - Chip Capacitor Land Pattern Design
  Recommendations per IPC-7351", Density Level B.
- 2512: Vishay D/CRCW e3 thick-film chip resistors, Doc 20035,
  "Recommended Solder Pad Dimensions", reflow (no IPC-labelled nominal 2512
  land was found).
- Courtyard excess: IPC-7351 (Feb 2005) Tables 3-5 and 3-6, nominal (0.15
  mm for 0201 and 0402, 0.25 mm from 0603 up).

The per-size numbers are in
`crates/signex-app/examples/build_stock_library/chip.rs`. `Chip-0805`
follows the same table (1.15 x 1.45 mm pads, 1.80 mm centres) since #1938;
the two 0805 footprints and the nine rows that use them moved from version 0.0.1 to 0.0.2, so placed parts show up in Library Updates and get the new pads when the update is applied.
