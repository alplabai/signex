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
