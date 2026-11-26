# Native-format fixture corpus

Checked-in `.snxsch` / `.snxpcb` files walked by
`../round_trip_corpus.rs`. The files here are the **golden**: the harness
never regenerates them at test time, so a diff against the file is the
whole signal.

## What it guarantees

For every fixture, found recursively:

1. it **parses** (`SnxSchematic::parse` / `SnxPcb::parse`);
2. it is **byte-stable** — re-serialising the parsed value reproduces the
   on-disk bytes exactly (the no-edit-save gate).

All failing fixtures are listed in one run, each with its path and, for a
byte mismatch, the first differing line.

A `MIN_CORPUS_FILES` constant guards the count so the corpus cannot
shrink silently. Raise it in the same change that adds fixtures.

## Adding a fixture

1. Write the file by hand or from a builder, put it under
   `schematic/` or `pcb/` (any subdirectory works).
2. Run `SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration round_trip_corpus`
   to rewrite it into canonical form.
3. Review the diff, then run the test without the variable.
4. Bump `MIN_CORPUS_FILES`.

## Schematic coverage matrix

Each `schematic/*.snxsch` fixture was produced by the real writer from a
sheet built in code (never hand-edited) and checked to re-save
byte-identically. Filenames name the feature:

| Area | Fixtures |
|---|---|
| Baseline | `empty_sheet`, `symbol-single`, `symbol-all-tsv-columns` |
| Wire colour | `wire-colour` (optional `color` wire column: empty cell, `#rrggbbaa`) |
| Extras-only data | `symbol-extras-only-data` (`fields`, `pin_uuids`, `custom_properties`, field texts, instances) |
| Symbol flags | `symbol-dnp`, `symbol-not-in-bom`, `symbol-not-on-board`, `symbol-exclude-from-sim`, `symbol-locked`, `symbol-power` |
| Units / orientation | `symbol-multi-unit`, `symbol-mirror-x`, `symbol-mirror-y`, `symbol-rotations-axis` (0/90/180/270), `symbol-rotation-non-axis` |
| Connectivity | `wires-basic`, `junctions-user-and-minted`, `wires-t-junction`, `buses`, `bus-entries` |
| Labels | `label-net`, `label-global`, `label-hierarchical`, `label-power`, `label-alignment-and-size` |
| Markers / annotation | `no-connect-markers`, `no-erc-directives`, `text-notes`, `drawings-all-primitives` |
| Sheet metadata | `title-block`, `lib-symbols-cache` (every `Graphic` variant, several pin shapes) |
| Text hostility | `text-adversarial-label`, `text-adversarial-fields`, `hostile_label_text` (backslash, quote, bare `-`, tab, newline) |
| Numeric | `coordinates-nanometre-rounding`, `regulator_stage` (sorted map keys) |
| Schematic Refinement (`snxsch/1.3`) | `refinement-objects` (two harness connectors with entries, one locked; a harness wire; a harness-typed sheet entry on a sheet symbol; two unions; a locked Bezier with line style, end arrow and colour; a dimension; a table bound to a part's pins with a hidden column; a Net Parameters directive with every rule; a Blanket with track width, keep-short pour and edge nets; a Differential Pair with a class). Written by hand to the writer's layout, see `docs/architecture/snxsch-refinement-objects.md` |

Hierarchy: `hierarchy-root-two-children` (two child-sheet symbols with
pins), `hierarchy-nested-middle` (a child that is itself a parent) and
`hierarchy-child-leaf` (the bottom of the chain).

Holes (known, deliberately absent): unknown vendor keys
(dropped on parse today, blocked on forward-compat work); a symbol whose
extras are entirely default, because it does not round-trip today
(data loss: the writer omits `[extras.symbols.<uuid>]` for a default
`SymbolExtras`, and the parser's derived `Default` yields `unit = 0`,
`in_bom = false`, `on_board = false`, so a plain symbol drops out of the BOM
and off the board after one save/reload; fix is a manual
`impl Default for SymbolExtras` with `unit` 1, `in_bom` and `on_board` true;
tracked as #789, fixed by its own PR, which also adds the bare-symbol fixture).
Every symbol fixture therefore carries a non-default `footprint`.

Hierarchy sheets: `hierarchy-root-two-children`, `hierarchy-nested-middle`,
`hierarchy-child-leaf` (hierarchical label `VIN` matching the parent's sheet
pin) and `hierarchy-repeated-child-instances` (one child file instantiated
twice, non-empty `instances`, non-default `stroke_color`/`fill_color`).

## Project files (`.snxprj`)

`project/*.snxprj` are walked by `../round_trip_project_corpus.rs`, which
loads each with `parse_project`, saves with `write_project`, and requires
the exact same bytes. `parse_project` rewrites `dir` to the file's real
location, so fixtures store `"dir": ""` and the harness restores that
placeholder before comparing. Coverage: `minimal-empty`,
`two-flat-sheets`, `hierarchical-three-sheets` (the sheet list for the
hierarchy fixtures: root -> middle -> leaf all resolve in the corpus),
`full-with-libraries-and-variants`, `unknown-keys-preserved` (unknown keys
at top level, per sheet and per library, plus the `format` token),
`schematic-design` (the `schematic_design` key: changed channel naming,
harness definitions `SPI_BUS`, `UART` and `DEBUG` with a nested `UART`,
diff-pair classes `USB` and `LVDS_100R`).
`MIN_PROJECT_FIXTURES` guards the count. These are not blessable: edit by
hand and keep keys alphabetical (the writer sorts them).

## Realistic boards (`pcb/real/`)

The single-feature `pcb/*.snxpcb` cases above test the format one column at
a time. `pcb/real/` holds five complete small designs, the PCB Layout exit
criterion "five real `.snxpcb` fixtures render". Each is a `.snxpcb` plus a
`.snxprj` that names it and carries its net classes (the board rules live in
`[stackup.setup]`, the per-net clearances and track widths in the project).

| Board | What it is | Covers |
|---|---|---|
| `linear-regulator` | 5 V to 3.3 V LDO, the sample project's schematic (`crates/signex-app/assets/sample-project/linear-regulator.snxsch`) laid out: 2 headers, SOT-223, 0805 caps, 0603 R and LED, footprint names and values as the schematic gives them. 2 layers, 36 x 20 mm | every footprint linked to its schematic symbol (`sch_link`); locked connectors; GND vias into a bottom pour |
| `usb-c-power-input` | USB-C power-only receptacle, 1206 polyfuse, SOD-123 TVS, bulk cap, power LED, output header. 2 layers, 30 x 22 mm, rounded corners | plated shell slots (oval drills); CC escapes through vias to bottom-side 5.1 k pull-downs; a footprint turned 270 degrees |
| `mcu-breakout-tqfp32` | ATmega328P-AU pin-out on a TQFP-32, each side fanned out to its own 1x8 header; decoupling, AVCC / AREF caps, 16 MHz crystal and load caps on the bottom. 2 layers, 40 x 44 mm | 0.8 mm pitch fan-out (96 track segments), seven bottom-side parts, a net per pin |
| `sensor-4layer` | BME280-style LGA-8 sensor, SOT-23-5 LDO, I2C pull-ups, 1x4 header. 4 layers, 26 x 18 mm, chamfered | inner GND and +3V3 planes as zones with 22 antipad holes; plane layer types and display names in the stackup; supply vias |
| `mixed-tht-smd` | 12 V to 5 V supply: barrel jack, SOD-123 diode, TO-220 7805, radial electrolytics, screw terminal, 45-degree LED and resistor, bottom-side 0805 decoupling. 2 layers, 50 x 40 mm | outline with a 10 mm arc corner (an `arc` on `EdgeCuts`); oval slot drills; `[board_sketch]` with four M3 mounting holes (two plated to GND), a top-copper keepout and a routed 8 x 3 mm cable-tie slot (a through cutout) |

Between them the five cover two- and four-layer stack-ups, SMD and
through-hole parts, vias, pours with thermal relief (every zone is written
with `thermal_relief = true`), a keepout, a cutout, top and bottom
placement, footprint silkscreen and board text on both silkscreen layers.

Origin and licence: all five are original work of the Signex project,
designed for this corpus with Signex's own types and writer (the builders
in `../integration/real_boards/`). No third-party board file, footprint
or layout was imported or copied; the part outlines follow the generic
package dimensions their names give. They are covered by the repository
licence, like the rest of the tree.

The files are generated, never hand-edited: `../integration/real_boards/`
builds them deterministically, and `real_boards_match_their_generator`
fails when a committed file differs from what the generator writes. To
change a board, edit its builder and run
`SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration real_boards_match`.

What is checked, and where:

- Format (`signex-types`, `real_boards`): each file reloads and re-saves
  byte-identical, a save and reload leaves the board unchanged, the item
  counts are pinned, every pad, track end, via, zone corner and text sits
  inside the outline and outside every cutout, no courtyard reaches into a
  cutout, courtyards on one side do not overlap, each `.snxprj`
  round-trips, and the regulator's links resolve to the sample schematic.
- Render (`signex-renderer`, `pcb_real_boards`): each board builds a
  `PcbSnapshot` and a scene without panic, with every copper layer as the
  active one too. Each board's render golden,
  `crates/signex-renderer/tests/golden/pcb_real_boards/<board>.json`, is
  drawn in the board palette and pins the drawn item counts, fill
  triangles, cutouts, keepouts, holes and texts, and hashes of the copper
  geometry, the overlay geometry (outline, openings, keepouts, silkscreen
  and other drawings), the texts and the colours. Regenerate with
  `UPDATE_GOLDEN=1 cargo test -p signex-renderer --test integration real_boards`.
- DRC (`signex-drc`, `real_boards`): run against each board's own net
  classes, every board has zero violations of any kind; a positive control
  shows a track moved to another net shorts on each board. Ratsnest
  (`pcb-geom`, `ratsnest_real_boards`): every board's is empty.

Not covered by the format: a slot's length (a drill is one diameter and a
shape token, so a slot is as long as its oval pad says) and the zone fill
(a zone stores its outline and holes, not a filled polygon). The board
sketch on `mixed-tht-smd` is written as a bake would store it; it is not
re-baked in this test.

## Advanced Schematic projects (`advanced/`)

The exit criteria of `docs/ROADMAP.md`, "Advanced Schematic (P1)", as
projects on disk. Each folder is a `.snxprj` with its sheets (and a board
for the classes), generated by `../integration/advanced_fixtures/` the way
the real boards are: `advanced_fixtures_match_their_generator` fails when a
committed file differs from the generator, and
`SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration advanced_fixtures`
rewrites them. The sheets and the board are walked by the round-trip
harness above; each project file is checked to save back to its own bytes.

| Project | What it is | Golden (where, `UPDATE_GOLDEN=1` rewrites it) |
|---|---|---|
| `four-channel-variants` | `top.snxsch` (U1) places `channel.snxsch` (R1, C1, C2) as `Repeat(CH, 1, 4)`. Variants `Full` (CH1's R1 is 4k7) and `Lite` (C2 not fitted on any channel by its `Fitted@Lite` field, R1 not fitted on CH3 and CH4 by the project's variant settings) | a BOM (CSV) and a PDF per variant, `crates/signex-app/tests/golden/advanced_schematic/` |
| `nested-harness` | the signal harness fixture of `docs/SIGNAL_HARNESSES.md`: `SPI_BUS` to `sensor.snxsch`, `DEBUG` with `UART` nested in its `UART` entry to `connectors.snxsch` | the netlist, `crates/signex-net/tests/golden/nested_harness.netlist.tsv` (the same nets as the in-memory fixture's golden) |
| `nested-harness-carry` | the same design with `DEBUG` carried through `bridge.snxsch`, a sheet with no connector | the same golden netlist |
| `net-class-directives` | one sheet, U1 and J1 on `VIN` (Net Blanket, class `POWER`), `CLK` (Net Parameters, class `HS`, width and clearance), `USB_P` / `USB_N` (Net Parameters, diff-pair class `USB`) and `SIG`; an empty board; the project's `POWER` and `HS` classes list no nets | the netlist with classes, `crates/signex-net/tests/golden/net_class_directives.netlist.tsv`; what Update PCB reads and writes, `crates/signex-app/tests/golden/advanced_schematic/net_class_directives.update_pcb.txt` |

## Design Notebooks (`.snxnbk`)

`notebook/**/*.snxnbk` are walked by the `signex-notebook` crate tests
(`src/tests_file.rs`, `src/tests_compile.rs`), not by this crate. Each file
reads with `NotebookFile::parse` and saves back byte for byte; every file
with a header is in the canonical form `Header::to_toml` writes, and every
reference a header stores is one its text makes. `MIN_NOTEBOOK_FIXTURES`
guards the count. Written by hand; keep headers canonical (format token,
unknown keys sorted, then one `[[references]]` table per reference).

| Fixture | Covers |
|---|---|
| `minimal` | a header with only the format token |
| `no-header` | plain Typst with no header, read as `snxnbk/1` and kept without one; an `#annotation` block |
| `kept-unknown-keys` | unknown keys at the top of the header and in a reference entry, a net with no id, a pin with one |
| `with-chapters/design` | the notebook file: the package import, the report layout, an `#include` of a chapter file |
| `with-chapters/chapters/power` | a chapter with its own header; references in markup and in block targets, both pin forms; one block of each kind with a typed id; the earlier `#annotation` form |
| `power-stage` | the "Power stage" page of mockup row 60b as review records: the page block, a Calculation with its target band and result, a Measurement table of three rails (pass, pass, warn), an Issue raised from it, a Decision and a Note naming a removed part (`src/tests_records.rs`) |
| `with-measurements/design` | measurements (mockup row 199): a `#measurement` block of four readings (nominal and tolerance, max and min limits), a reading on its own that stores the part values it was taken with, and an `#attachment` of the scope capture in `with-measurements/attachments/scope/` (a CSV, not walked as a notebook) |

`with-chapters` is also the compile fixture: it compiles with no
diagnostic, and two PDF exports of it are the same bytes. `power-stage`
compiles with no diagnostic too. `with-measurements` compiles with its
capture read the way the app reads it
(`NotebookSources::missing_attachments`).

## Intentional format change

Run the same bless command, review every fixture diff as the format
change it is, and commit them together with the writer change. Blessing
an unmodified tree produces no diff.

## Map ordering

`regulator_stage.snxsch` carries a populated `title_block`, symbol
`fields`, `pin_uuids` and several `lib_symbols`. These maps are written in
sorted key order, so a fixture that stays byte-stable proves the writer
is deterministic for them. Keep such data in the corpus.

## Line endings

`.gitattributes` pins this directory to LF so byte comparison does not
depend on a checkout's `core.autocrlf`.
