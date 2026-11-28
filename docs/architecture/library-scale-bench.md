# Library at scale: 25 000 parts, measured (#3115)

Standard libraries reach about 23 000 parts in one library file, so mount and
search time decide whether one library file is the right output shape for a
converter. This records what Signex costs at 25 000 parts. Measured on an AMD
Ryzen 7 5800X (8 cores, 16 threads) running Linux 7.0, release build, one core
(`taskset -c 0`), 2026-11-26, branch base `231dad972`.

## Result

Both thresholds hold, with room. No library code needs to change, and the
split-library fallback is not needed at this size.

| Measure | Threshold | Measured | |
|---|---|---|---|
| Cold mount to a listed Components panel (25 000 rows) | < 3 s | 1.46 s (0.61 s on an idle run) | passes |
| Slowest search (`100`, a bare number, 4 410 hits) | < 150 ms | 28 ms median, 56 ms max | passes |
| Slowest keystroke typing `100n` | < 150 ms | 77 ms | passes |
| Peak resident set after mount and search | none | 349 MiB | |

Cold mount, the run of the table above:

```text
BENCH cold_mount rows=25000 listed=25000 prepare_ms=1167 mount_ms=4 first_list_ms=283 total_ms=1459 peak_rss_mib=349
```

- `prepare_ms`: `prepare_mount`, the off-thread half: `LocalGitAdapter::open`
  and the read of every table (25 000 rows over 8 tables).
- `mount_ms`: `LibraryState::mount_prepared`, the `LibrarySet` mount.
- `first_list_ms`: the first `sync_view_caches`, which builds the search keys
  of every row and lists them.

The same bench on an idle host gave `prepare_ms=514 first_list_ms=89
total_ms=607`. The host is shared with other build jobs, so the figures move
by a factor of two between runs; the margin to the thresholds is larger than
that (2x on mount, 2.7x on the slowest search).

Search, median and max of 7 runs, each typed after a different query so the
filter really runs (`BENCH search ...`):

| Query | Hits | Median | Max |
|---|---|---|---|
| `100n` | 37 | 0.8 ms | 3.7 ms |
| `10k` | 19 | 0.7 ms | 3.8 ms |
| `4k7` | 19 | 0.7 ms | 3.7 ms |
| `1u 16V` | 19 | 0.5 ms | 0.6 ms |
| `TDK-001234` | 1 | 0.8 ms | 0.9 ms |
| `resistor 0603` | 1 042 | 4.3 ms | 4.4 ms |
| `100` | 4 410 | 27.6 ms | 55.9 ms |
| `murata 0805 capacitor` | 0 | 5.1 ms | 8.1 ms |
| `no such part` | 0 | 0.8 ms | 3.9 ms |

A value word (`100n`, `4k7`) stops at the first part it fails; a bare number
(`100`) is the slow case because it matches any measurement that displays that
number in any prefix, then the names. `murata 0805 capacitor` finds nothing
only because the generator ties the maker and the package to the same index;
it is a full scan with a miss.

## What is not in the numbers

- The OS page cache is warm: the library was just written. "Cold" means no
  in-process cache of the library, not a cold disk.
- Components panel thumbnails and the rest of the app's frame. The list the
  panel shows is built; drawing it is virtualised and not measured here.
- The Components panel table (#2785) is built in the same `sync_view_caches`:
  the category's rows, then the search, filters, sort and grouping, each stage
  only when its inputs change (docs/architecture/components-panel.md). The
  figures above predate it; rerun the bench to include its first build.
- A library under git history: the generated one is written by
  `LibraryWriter`, which does no git operation, so `LocalGitAdapter::open`
  reads a library with no commits. A library with a long history opens with the
  same table read; the history is not walked at mount.

## Method

- **Library.** Written by `LibraryWriter` (`signex-library/src/writer`), the
  converter's own path, into a temp folder, deterministically (fixed ids and
  times, so two runs write the same bytes): 25 000 part rows in 8 tables
  (resistors, capacitors, inductors, diodes, transistors, ics, connectors,
  misc), 2 500 symbol envelopes of one symbol, 1 250 footprint files of one
  footprint each. Each row has a description, a primary and one alternate
  manufacturer part, a distributor listing, a datasheet URL and five
  parameters: a measurement that walks the E24 mantissas and seven decades (so
  values repeat as a catalogue's do), a rated voltage, and tolerance, package
  and AEC flags.
- **Mount.** `library::mount::prepare_mount` then `LibraryState::mount_prepared`
  then `LibraryState::sync_view_caches`: the calls the app makes when it opens a
  library, and the first update after it.
- **Search.** `sync_view_caches` after setting the Components panel's filter
  string: `PanelQuery::parse` over the cached `SearchKeys` of every row, the
  panel's own code (`panels/components_panel/query.rs`,
  `library/state/panel_rows.rs`), not a reimplementation. The cache is warm, as
  it is while typing.
- **Harness.** `crates/signex-app/src/library/state/scale_bench_tests.rs`, an
  `#[ignore]`d test. It prints one `BENCH` line per measurement and a last
  `verdict` line.

## Rerun

```text
bash scripts/bench_library_scale.sh out.log
SIGNEX_BENCH_ROWS=5000 SIGNEX_BENCH_CORE=3 bash scripts/bench_library_scale.sh
cargo test -p signex-app --release --lib scale_bench -- --ignored --nocapture --test-threads=1
```

The script builds first, then runs the test pinned to one core
(`SIGNEX_BENCH_CORE`, default 0). On the pool, build with several cores and run
the timing with two:

```text
srun -p build -w alplabgw2 -c 2 bash -lc 'cd <worktree> && taskset -c 0 cargo test -p signex-app --release --lib scale_bench -- --ignored --nocapture --test-threads=1'
```
