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
at top level, per sheet and per library, plus the `format` token).
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
| `mixed-tht-smd` | 12 V to 5 V supply: barrel jack, SOD-123 diode, TO-220 7805, radial electrolytics, screw terminal, 45-degree LED and resistor, bottom-side 0805 decoupling. 2 layers, 50 x 40 mm | outline with a 10 mm arc corner (an `arc` on `EdgeCuts`); oval slot drills; `[board_sketch]` with four M3 mounting holes (two plated to GND) and a top-copper keepout |

The files are generated, never hand-edited: `../integration/real_boards/`
builds them deterministically, and `real_boards_match_their_generator`
fails when a committed file differs from what the generator writes. To
change a board, edit its builder and run
`SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test integration real_boards_match`.

What is checked, and where:

- Format (`signex-types`, `real_boards`): each file reloads and re-saves
  byte-identical, a save and reload leaves the board unchanged, the item
  counts are pinned, every pad, track end, via, zone corner and text sits
  inside the outline, courtyards on one side do not overlap, each `.snxprj`
  round-trips, and the regulator's links resolve to the sample schematic.
- Render (`signex-renderer`, `pcb_real_boards`): each board builds a
  `PcbSnapshot` and a scene without panic, with every copper layer as the
  active one too; drawn item counts, fill triangles and a hash of the drawn
  geometry (colours excluded) are pinned.
- DRC and ratsnest (`signex-drc`, `real_boards`): run against each board's
  own net classes, every board has zero shorts, clearance, unconnected and
  keepout violations and an empty ratsnest; a positive control shows a
  track moved to another net shorts on each board.

Not covered by the format: a slot's length (a drill is one diameter and a
shape token, so a slot is as long as its oval pad says) and the zone fill
(a zone stores its outline and holes, not a filled polygon). The board
sketch on `mixed-tht-smd` is written as a bake would store it; it is not
re-baked in this test.

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
