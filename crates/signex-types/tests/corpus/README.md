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
2. Run `SIGNEX_BLESS_CORPUS=1 cargo test -p signex-types --test round_trip_corpus`
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
| Extras-only data | `symbol-extras-only-data` (`fields`, `pin_uuids`, `custom_properties`, field texts, instances) |
| Symbol flags | `symbol-dnp`, `symbol-not-in-bom`, `symbol-not-on-board`, `symbol-exclude-from-sim`, `symbol-locked`, `symbol-power` |
| Units / orientation | `symbol-multi-unit`, `symbol-mirror-x`, `symbol-mirror-y`, `symbol-rotations-axis` (0/90/180/270), `symbol-rotation-non-axis` |
| Connectivity | `wires-basic`, `junctions-user-and-minted`, `wires-t-junction`, `buses`, `bus-entries` |
| Labels | `label-net`, `label-global`, `label-hierarchical`, `label-power`, `label-alignment-and-size` |
| Markers / annotation | `no-connect-markers`, `no-erc-directives`, `text-notes`, `drawings-all-primitives` |
| Sheet metadata | `title-block`, `lib-symbols-cache` (every `Graphic` variant, several pin shapes) |
| Text hostility | `text-adversarial-label`, `text-adversarial-fields`, `hostile_label_text` (backslash, quote, bare `-`, tab, newline) |
| Numeric | `coordinates-nanometre-rounding`, `regulator_stage` (sorted map keys) |

Holes (known, deliberately absent): hierarchical child sheets and
multi-sheet projects (`.snxprj`, separate task); unknown vendor keys
(dropped on parse today, blocked on forward-compat work); a symbol whose
extras are entirely default, because it does not round-trip today
(data loss: the writer omits `[extras.symbols.<uuid>]` for a default
`SymbolExtras`, and the parser's derived `Default` yields `unit = 0`,
`in_bom = false`, `on_board = false`, so a plain symbol drops out of the BOM
and off the board after one save/reload; fix is a manual
`impl Default for SymbolExtras` with `unit` 1, `in_bom` and `on_board` true;
tracked as #789, fixed by its own PR, which also adds the bare-symbol fixture).
Every symbol fixture therefore carries a non-default `footprint`.

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
