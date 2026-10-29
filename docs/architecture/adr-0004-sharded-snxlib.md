# ADR-0004 — Sharded `.snxlib`: bucket rows by a hash of `row_id`

- **Status:** Proposed, not adopted (2026-10-29). Recommendation: do not build it yet; see "Trigger criteria"
- **Scope:** `signex-library` — the `.snxlib` on-disk layout, `LocalGitAdapter` (`adapters/local_git/`), the history and restore paths that read it
- **Extends:** the v0.9 single-file layout (`.snxlib` holds the manifest and every `[tables.<name>]` block)
- **Tracking:** #1438. Deferred from #416; measured here with the bench the #416 work left in `crates/signex-library/tests/integration/table_write_cost.rs`
- **Code change:** none. This ADR and its measurements only

## Context

A local library is one `.snxlib` text file in a git working tree. Every row edit rewrites that file and commits it:

1. reload check (stat, plus a content hash for a mutation),
2. copy-on-write clone of the parsed file (rows are shared `Arc`s, so the clone is cheap),
3. encode, read back and verify,
4. atomic write (temp file, fsync, rename),
5. `git add` of the one path and a commit, which hashes and deflates the **whole blob**.

#416 removed the avoidable costs (row-level encode cache, verified fast path). What is left scales with the file size, not with the edit: steps 3 to 5 touch every byte. #1437 (this PR) removed the same shape from the read side (`read_row` converts one row; `snapshot_table` reuses converted rows), so the write side is now the only whole-file cost on the hot path.

The cascade of a primitive save is already one write and one commit for all its rows (#1362, #1436), so the cost below is per write, however many rows that write moves.

## Measurements

Release build, local SSD, macOS (Apple silicon), one process, `cargo test -p signex-library --test integration --release -- table_write_cost git_objects --ignored --nocapture`. Library: one `resistors` table, every tenth row carrying a cached price. Latency figures are the best of 5 runs; the growth figures are one run of 10 edits. Two full runs of the bench agreed to within 5 percent.

**One-row edit latency** (`update_row`; insert and delete cost the same within 5 percent):

| Rows | `.snxlib` bytes | `update_row` | stat + read + hash of the file |
|---:|---:|---:|---:|
| 1,000 | 635,262 | 15.5 ms | 0.23 ms |
| 10,000 | 6,357,456 | 47.2 ms | 2.3 ms |
| 50,000 | 31,829,441 | 191.7 ms | 14.1 ms |

Reading the table: about 11 ms is fixed cost (git open, ref and index work, fsync) and the rest is about 6 ms per MB of file.

**`.git/objects` growth per edit** (10 edits after a warm-up edit; loose objects, nothing runs `gc` or `repack` on a library today):

| Rows | `.snxlib` bytes | growth over 10 edits | per edit | per edit as share of file |
|---:|---:|---:|---:|---:|
| 1,000 | 611,462 | 177,133 | 17,713 | 2.9 % |
| 10,000 | 6,119,462 | 1,663,612 | 166,361 | 2.7 % |
| 50,000 | 30,639,462 | 8,252,976 | 825,297 | 2.7 % |

Each edit stores a new compressed copy of the whole file. At 50k rows that is 0.83 MB per edit, so 1,000 edits is about 0.83 GB of loose objects.

**What a repack leaves** (the same repository after the command-line `git gc`, which delta-compresses near-identical blobs):

| Rows | `.git/objects` bytes before `gc` | `.git/objects` bytes after `gc` |
|---:|---:|---:|
| 1,000 | 196,118 | 22,941 |
| 10,000 | 1,831,246 | 149,754 |
| 50,000 | 9,079,544 | 712,472 |

**Caveat on the data.** The bench rows are copies of one row with a new id and part number, so they compress far better than a real library (zlib and delta both). The growth numbers are therefore a **floor**, not a typical figure. Before acting on the growth criterion, repeat the growth measurement on a real library of the target size.

For the read side (#1437, same bench, `adapter_read_cost_by_library_size`): `read_row` went from 4.6 / 47.0 / 233.7 ms to 7.8 µs / 32 µs / 42 µs at 1k / 10k / 50k rows, and a warm `read_table` from 4.6 / 46.7 / 232.8 ms to 0.35 / 5.5 / 25.6 ms. A cold `read_table` (the first call, which fills the cache) is about 1.3 times slower than before (5.1 / 76.7 / 321 ms against 4.7 / 48.1 / 239.7 ms).

The app's primitive-save cascade opens a new `LocalGitAdapter` for each save, so it always reads cold (about 0.3 s at 50k rows, the cold `read_table` figure above). The warm figures above apply to a mounted adapter that stays open, not to that path.

## Problem

Two costs track the whole file:

- **Latency.** A single-row edit costs 47 ms at 10k rows and about 190 ms at 50k, against a floor of about 11 ms. An interactive edit under about 100 ms feels immediate; 190 ms is noticeable but usable, and bulk operations are one write, so they pay it once.
- **Repository growth.** Loose-object growth is 2.7 percent of the file per edit on this data. It is unbounded because nothing repacks; whether it is a problem depends on the real compressibility of a library and on how often it is edited.

Neither is a correctness problem, and neither affects a library of a few thousand rows.

## Proposal: shard by a hash of `row_id`

### Layout

Rows move out of the `.snxlib` into shard files; the `.snxlib` keeps the manifest and the shard index.

```text
mylib/
├── mylib.snxlib                  manifest, class list, [shards] index (generation + per-shard digest)
├── shards/<table>/<bb>.snxshard  one TSV block per bucket, <bb> = bucket id in hex
├── symbols/ footprints/ sims/ models/   unchanged
└── .git/
```

- **Bucket = hash of `row_id`, not a uuid prefix.** Row ids are UUID v7, which are time-ordered: a prefix would put every new row in the newest bucket and leave the others cold. Use a fixed hash (for example the first byte of SHA-256 of the 16 id bytes) modulo a bucket count chosen at migration time and stored in the manifest. 256 buckets is the working figure: about 195 rows per bucket at 50k rows.
- **Bucket count is fixed per library.** A rehash on growth would be a migration of its own; if a library outgrows its count, a re-shard is the same explicit migration run again with a larger count.
- **Pricing sidecars** (`[tables.<t>.pricing.*]`) move with their row into its shard, so a price write stays one shard.
- **The `layout` / unknown-key preservation (#1435)** applies per file, so a shard keeps keys it does not model.

### Format token and migration

The current token is `snxlib/2` (introduced with the description column, #1474, `FORMAT_TOKEN` in `library_file/mod.rs`); a build refuses any other token today. The sharded layout is therefore **`snxlib/3`**. (The text of #1438 calls it `/2`; that was written before `/2` existed.)

- **Read support for `/2`.** A `/3`-capable build opens a `/2` file unchanged, as one implicit "bucket 0" holding every row, so existing libraries keep working with no action.
- **Explicit migration, never implicit.** `/2` to `/3` runs only when the user asks, from a library action. It rewrites the library as shards in one manifest-swap (below), commits once with a message that says so, and leaves a `/2` copy reachable in git history. A `/3` file is never written back to `/2`.
- **Older builds refuse `/3`** with the existing `UnsupportedFormat` error. That is the break the token exists for; the release note for the migration says so.
- Migration is the only operation that touches every row; it is as slow as today's full write, once.

### Atomic multi-shard operations: the manifest swap

A single-row edit writes one shard file and the manifest's digest for it. Operations that touch many rows (bulk import, table rename, pricing prune, a primitive-save cascade, migration) touch many shards and must not be seen half done, by readers or after a crash.

Protocol, under the library write lock (the one used today):

1. Write each changed shard to a new file next to its target (temp, fsync), keyed by the new generation: `shards/<table>/<bb>.<gen>.tmp`.
2. Write the new manifest (`mylib.snxlib`) to a temp file. It names the new generation and the digest of every shard it expects.
3. **Rename the shard temp files into place, then rename the manifest last.** The manifest rename is the commit point. A reader checks each shard against the digest in the manifest it read.
4. fsync the directory; commit all changed paths in **one git commit** (the staged path set is the shards plus the manifest).

Crash cases: before step 3's manifest rename, the old manifest points at digests of shards that may already have been replaced; the reader sees a digest mismatch, and recovery restores each mismatched shard from its `.tmp` (new generation complete) or from HEAD (it is the only committed copy). The recovery rule is "the manifest is the truth, shards that disagree with it are stale". A crash after the manifest rename is a complete operation whose commit is missing, which is exactly the state the single-file adapter already handles (`record_commit_warning`: the edit is kept, the next commit includes it).

A single-row edit does not need the swap: it writes one shard and the manifest's digest for it, in the same order, so the same recovery applies.

### History and restore over a path set

`history(path)` and the History panel today read `git log -- <one rel_path>` (`git_history::path_history`). Under sharding a row's history is its shard's history, and a library-wide event (migration, cascade, import) touches many paths.

- Row history: bucket of `row_id`, then `path_history` on that one shard path. Unchanged cost.
- Library history: `path_history` over the set (manifest plus shards); commits are de-duplicated by id, so a cascade that touched 40 shards is still one entry.
- **Restore** of a commit restores the manifest and every shard of the commit's path set together, in one manifest-swap. Restoring one shard alone would leave it disagreeing with the manifest digest, so it is refused: restore is per commit, and a per-row restore is "read the row at that commit, write it as a new edit".
- `commit_path` stages one path today; it grows to take a set (`commit_change_with` already takes extra paths).

## Expected effect

Not measured; this is arithmetic on the measurements above and must be re-measured on a prototype before it is trusted.

- Per-edit write at 50k rows: shard of about 125 KB (31.8 MB / 256) instead of 31.8 MB. At about 6 ms per MB plus the fixed 11 ms, an edit lands near 12 ms instead of 190 ms.
- Per-edit repository growth at 50k rows: about 3 KB instead of 825 KB (the same 2.7 percent of a 125 KB shard, plus the small manifest).
- The fixed 11 ms does not shrink; sharding stops helping once the shard is small.
- Reads cost a row lookup after #1437.

## Costs

- **Format break.** `/3` is refused by every older build; teams on mixed versions must upgrade together before one user migrates.
- **Reverses the v0.9 single-file design.** One file you can open, diff and email becomes a directory of up to 256 files per table. `git diff` on a library gets more files, each smaller.
- **More failure modes.** Digest mismatch, a missing or orphaned shard and a crash between the renames are new states with new recovery code and tests. The single-file layout has none of these because a rename of one file is atomic by itself.
- **Merge behaviour changes.** Two branches editing different rows now touch different files (fewer conflicts); two branches editing rows in the same bucket still conflict as text, now inside a shard. The manifest digest line conflicts on **every** concurrent edit, so the digests must not live in the manifest text that merges (see "Open questions").
- **Every whole-library feature must learn the shard set**: pricing prune, class rename, cascade, search index, `where_used`, the pinned-asset commit (#1471), the oracle tests in `write_oracle_tests.rs`.
- **Cost of the prototype** is the real cost: the write path, the recovery path, the migration, history and restore, and their test matrix. It is a multi-week change in the one module the app's data safety depends on.

## Alternatives

1. **Do nothing.** 190 ms at 50k rows is usable, and libraries of that size are not a stated target. Growth is the only open cost.
2. **Repack automatically** (`git gc` or an in-process repack, for example every N edits or on library close). The measurement above shows a repack shrinks the repository by about 12x at 10k and 50k rows and 8.4x at 1k rows on this data with no format change, no migration and no new failure mode. It does nothing for latency. This is the cheapest answer to the growth criterion and should be tried first. Risks to check: a repack under a concurrent `LocalGitAdapter` commit, repository size on Windows, and the time a repack takes at 50k rows (not measured).
3. **Split by table instead of by row hash** (one file per `[tables.<t>]`). Simple, and a table is already the user's unit. It does not help the case that matters (one very large table), and a single `resistors` table of 50k rows stays at 190 ms.
4. **A database file instead of text** (SQLite or similar). O(row) writes and no growth problem, but it gives up text diffs and merges, which are the reason the library lives in git at all, and git stores a binary blob whole.
5. **Shard by bucket count that grows** (a directory trie by hash prefix). No fixed count to outgrow, at the price of a re-split protocol that is more complicated than the migration above. Not needed until a library outgrows 256 buckets, which is more than 60k rows per table at 250 rows per bucket.

## Trigger criteria

Write the code (move this ADR to Accepted) when **either** holds:

1. **Latency:** 50k-row (or larger) libraries with frequent edits are a real target workload and a single-row edit above about 150 ms is a reported problem. Equivalently, a measured edit above 100 ms at the library sizes users actually have.
2. **Growth:** `.git/objects` growth per edit on a **real** library, **after the automatic repack of alternative 2 is in place**, is still a problem for users (a library repository over an agreed size, for example 1 GB, in normal use).

## Recommendation

**The triggers are not met today. Do not build the sharded layout now.**

- **Latency:** 47 ms at 10k rows is under the bar, and 10k rows is already a large component library. 190 ms at 50k rows is noticeable but usable, and nothing in the issue tracker or the roadmap names 50k-row libraries as a target. Re-evaluate if that changes.
- **Growth:** 0.83 MB per edit at 50k rows (0.17 MB at 10k) is real but is the loose-object figure. The same repository after `git gc` is about 12x smaller at 10k and 50k rows (8.4x at 1k) on this data, so the first step is alternative 2: repack automatically. That needs no format change. Measure it on a real library before deciding the growth criterion is met; the bench data is more compressible than a real library, so the growth numbers here are a floor and the post-`gc` figure may be too optimistic.
- **Next steps, in order:** (1) a real-library growth measurement; (2) automatic repack, measured; (3) only if criterion 1 or 2 still holds, prototype shards and measure the Expected effect section before accepting this ADR.

## Open questions

- **Where the shard digests live.** In the manifest they conflict on every concurrent edit; outside it (a generated, git-ignored index rebuilt from the shards) there is no manifest swap to make atomic, but there is also no cheap way to detect a half-done multi-shard operation. Decide by prototype.
- **Bucket count.** 256 is a working figure. A table with under a few thousand rows gains nothing from sharding; whether small tables stay one shard is open.
- **Windows.** Rename-over-existing and directory fsync behave differently from POSIX; the protocol needs a Windows pass before acceptance.
