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
`stock_library_rows.rs` check the committed files, not the generator
(`committed_footprints_equal_the_generator_output` covers only the 14 chip
footprints, `Chip-*` plain and `-Polarised`; the rest can be edited in the
app with `SIGNEX_EDIT_STOCK=1`):

- the library id stays `019df45d-af5e-7a00-8c3b-dd3bf89b40ae`;
- every pinned footprint, symbol and row id is still present (adding parts
  is fine, removing a shipped one fails);
- the library opens through the real adapter, every symbol round-trips
  byte-identically, pins sit on the schematic grid, and the 74HC and
  connector shape rules hold for any part, old or new.

Never copy the folder to make a "second stock library": two mounted
libraries may not share an id (#1894), so the copy is refused.
Giving a copy its own library id lets it open, but it then breaks design
references: a design names a row by library id, so a part placed from the
copy points at the copy's id, and a stock reference never resolves to the
copy's rows. A copy with a new id is a different library, not stock.

## The seed generator

`crates/signex-app/examples/build_stock_library/` wrote the first version
of these files and still pins their ids (`ids.rs`, `rows.rs`). Rerunning
it would discard every edit made in Signex, so it refuses to write while
`Stock.snxlib` exists unless you pass `--force`:

```text
cargo run --example build_stock_library -p signex-app -- --force
```

The four power ports (GND, VCC, +3V3, +5V) take their stem and head
geometry from `power_port_lib_symbol` in `signex-types`, so a stock port and
a built-in one are the same size (2.54 mm stem, #2091). Their committed
files were edited in place to match the generator, which keeps any other
edit made in Signex.

No symbol version was bumped for it, for the reason given under Chip
passives below: drift is decided by content hash, not by version. A sheet
that embedded a copy of one of the four from the Stock library reads it as
drifted, which is correct since the library changed, and shows the library
update notice for it. A placed port keeps connecting: its tip stays at the
origin and the sheet's own entry wins.

## Chip passives

R, C, L, Ferrite and Fuse default to `Chip-0805`; D, D_Schottky, D_Zener
and LED default to `Chip-0805-Polarised` (pad 1 = cathode, silkscreen
bar). Each row also lists the sizes 0201, 0402, 0603, 1206, 1210 and 2512
(the matching `-Polarised` ones for the diodes) as alternate footprints,
so the footprint picker offers them without a pin-map override: pads 1 and
2 match the symbol pins.

IPC-7351 itself gives fillet goals, not fixed per-size lands, so the
reference is "IPC-7351 Density Level B (manufacturer tables)". Sources:

- 0201 to 1210: KEMET, "Surface Mount Multilayer Ceramic Chip Capacitors
  (SMD MLCCs) - C0G Dielectric, 10-250 VDC (Commercial Grade)", C1003_C0G,
  3/30/2016, p.10, "Table 3 - Chip Capacitor Land Pattern Design
  Recommendations per IPC-7351", Density Level B.
- 2512: Vishay D/CRCW e3 thick-film chip resistors, Doc 20035,
  "Recommended Solder Pad Dimensions", reflow (no IPC-labelled nominal 2512
  land was found).
- Courtyard: KEMET V1 x V2 (rounded up to 0.1 mm) for 0201 to 1210; 2512
  follows the same rule (pad extent plus 0.25 per side, rounded up to 0.1).
- 3D body height: the thickest KEMET Table 2A code of the size (0201 to
  1210), Vishay D/CRCW H for 2512.

The per-size numbers are in
`crates/signex-app/examples/build_stock_library/chip.rs`. `Chip-0805`
follows the same table (1.15 x 1.45 mm pads, 1.80 mm centres) since #1938.
No footprint or row version was bumped for it: new placements and
footprint loads by name get the new 0805 land, but a part already placed
keeps its footprint and an existing PCB keeps its copper, and Library
Updates shows no entry. (Stock is Personal mode, so a version bump would
be applied silently on open and dirty every design using a stock part.)
