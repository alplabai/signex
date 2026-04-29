# Git history rewrite — 2026-04-29

This document records the rationale, scope, and audit trail for the
git history rewrite performed on the Signex repository on
**2026-04-29**, the same day v0.10.0 shipped. It is published as part
of the audit corpus alongside `kicad-derivation.md` and
`third-party-kicad-parsers.md` so the substantive licensing position
remains independently verifiable.

---

## TL;DR

- **What was rewritten:** the substring `kicad` (case-insensitive) inside
  `crates/`, in our own source code — function/identifier names, doc
  comments, and inline references. Each affected commit's tree was
  edited in place; commits and authorship were preserved.
- **What was *not* rewritten:** GPL-licensed KiCad source. None has
  ever been vendored into this repository. The two GPL-3.0 KiCad parser
  crates that briefly existed in-tree (`crates/kicad-parser/`,
  `crates/kicad-writer/`) were *Cargo dependencies wrapped in a local
  workspace member* and were removed wholesale in v0.9.0 — long before
  the rewrite. The rewrite touched residual *naming*, not the
  derivation surface.
- **The rewrite is not a license remediation.** It is a cosmetic
  alignment with the Apache-clean position established at v0.9.0. The
  legal claim that no GPL'd code was ever distributed under Apache 2.0
  rests on the v0.9.0 dependency removal and on the pre-v0.7 derivation
  audit (`kicad-derivation.md`), not on the post-v0.9 commit chain.
- **The pre-rewrite chain is preserved.** A complete bare-repo backup
  lives at `~/Desktop/signex-backup-post-history-rewrite-2026-04-29.git`
  on the maintainer's workstation. It contains the original 425
  commits with their original SHAs and is recoverable on request.

---

## What was actually changed

`git filter-repo` was run with a tree-rewrite rule that scrubbed the
substring `kicad`/`KiCad`/`KICAD` from text files inside `crates/`.
Concretely, the rewrite:

- **Renamed identifiers** that referenced the optional KiCad parser
  crates (e.g. `find_kicad_symbols_dir` → `find_symbols_dir`,
  `kicad_lib_dir` → `lib_dir`, `from_kicad_str` → `from_str`).
- **Edited comments and doc strings** that mentioned KiCad as a format
  reference (e.g. "matches KiCad's `pin_name_offset` semantics" →
  "matches the legacy editor's `pin_name_offset` semantics").
- **Did not touch** files outside `crates/`. `docs/`, `README.md`,
  `CHANGELOG.md`, `CONTRIBUTING.md`, and the `.github/` workflow
  directory all still contain `KiCad` mentions where that mention is
  intentional (audit docs, migration history, importer companion repo
  references). The doc layer is the *correct* place for those mentions.

The rewrite was not a delete. It was a rename + comment edit applied
across history so the active source tree matches the post-v0.9.0
Apache-clean naming convention from the very first commit.

## What was *never* in the tree

The v0.9.0 release (commit `856dd45a` on `main`) removed the following
Cargo dependencies from the workspace:

- `crates/kicad-parser/` — GPL-3.0, S-expression parser for `.kicad_sch`
  / `.kicad_pcb` / `.kicad_sym`.
- `crates/kicad-writer/` — GPL-3.0, mirror of the parser for output.
- `crates/signex-output/src/netlist/kicad_sexpr.rs` — KiCad-format
  netlist exporter that used the parser's sexpr builder.

These were the only GPL-licensed surfaces ever to touch the workspace,
and they were *Cargo workspace members* with their own `LICENSE` files
declaring GPL-3.0. They were never re-exported under Apache 2.0; users
who built against them got a clear GPL declaration in `Cargo.lock` and
in `cargo deny check licenses` output.

The v0.9.0 cutover relocated those crates to the `signex-kicad-import`
companion repository (kept under GPL-3.0 to honour upstream KiCad's
licence) and deleted them from the main tree. After v0.9.0, Signex
ships zero GPL-licensed source code under its Apache 2.0 envelope.

The history rewrite on 2026-04-29 happened *after* v0.9.0 — i.e. on a
tree that already had no GPL'd code in it. Its target was naming, not
licence boundaries.

## Why the rewrite happened

Three reasons, in order of weight:

1. **Search hygiene.** After v0.9.0, the licence-check CI began to flag
   any reintroduction of the substring `kicad` under `crates/` as a
   regression. The historical commits still contained the substring
   in residual identifiers and comments. A grep against history would
   surface those even though the active tree was clean — confusing for
   contributors trying to understand the post-v0.9 naming convention.
2. **Cosmetic alignment with the Apache-clean stance.** A reader
   browsing `git log -p` on a v0.9.0+ tree shouldn't have to mentally
   filter out residual KiCad mentions to confirm the Apache-clean
   position. The active source today should look the same as the
   active source from day one.
3. **Reduce the footprint a hostile reader could cite.** Even though
   residual names are not licence-bearing surfaces, they create work
   for a reviewer trying to confirm derivation status. Removing them
   reduces ambiguity.

None of these reasons is "we discovered GPL'd code we needed to hide".
That is not what happened.

## Defence against hostile readings

A hostile reader might claim: "The 2026-04-29 rewrite is evidence of
prior GPL contamination — they wouldn't rewrite history if there were
nothing to hide." The substantive defence:

- **Distribution argument.** Apache 2.0 obligations attach to the
  *artifacts you distribute*. Signex's release tarballs, installers,
  and crates.io / cargo-registry uploads are the distribution surface.
  None of those artifacts has ever contained KiCad source (pre- or
  post-rewrite). The rewrite changed nothing about what users
  download.
- **Dependency record.** `Cargo.lock` and `cargo deny check licenses`
  output at the v0.9.0 tag — both reproducible from the *original*
  pre-rewrite chain in the backup repo — show the GPL'd dep removal.
  That record is what a licensee or court would look at first; it is
  unaltered.
- **Audit trail.** `docs/audit/kicad-derivation.md` (created before
  the rewrite, preserved across it) inventories every previously-
  KiCad-shaped item, the remediation chosen, and the phase in which it
  shipped. `docs/audit/third-party-kicad-parsers.md` documents the
  parser crates' separation. This document closes the loop on the
  rewrite itself.
- **Backup chain.** The pre-rewrite chain is preserved bit-for-bit in
  the backup repo. Anyone can clone it, run the same `cargo deny`, and
  confirm the licence position independently.

The defence does not depend on the rewrite. It would hold equally well
if the rewrite had never happened — the rewrite is a cosmetic improve-
ment on top of an already-clean position, not the position itself.

## Backup location and recovery

- **Location:** `C:\Users\caner\Desktop\signex-backup-post-history-rewrite-2026-04-29.git`
  on the maintainer's workstation. Bare repository, 425 commits.
- **Inspect:**
  ```sh
  git --git-dir=/path/to/backup.git log --all --oneline
  git --git-dir=/path/to/backup.git fsck --lost-found
  ```
- **Recover an old SHA:** `git fetch /path/to/backup.git <sha>` from a
  working clone, then `git checkout <sha>`.
- **Restore the entire pre-rewrite chain (not recommended):** would
  invalidate the v0.10.0 tag already pushed to `alplabai/signex`, force
  every cloned working copy to reset, and reintroduce the residual
  KiCad identifiers that the rewrite cleaned up. Only do this under
  written legal advice.

## Forward policy

- **No further history rewrites.** The Apache-clean naming convention
  is now CI-enforced via the `no-kicad-shaped-symbols` job in License
  Guard. Any future regression is fixed at the *new commit* layer
  (revert or follow-up commit), not by rewriting history.
- **Audit docs stay versioned.** `docs/audit/*.md` is part of the
  release tree. Editing or removing these docs is a code-review-
  visible change, not a history-rewrite-able operation.
- **Pre-Pro / Pre-365 legal review.** Before the first commercial
  release (Signex Pro or Signex 365 PLM), this document, the audit
  trail, and the licence position should be reviewed by OSS counsel
  for written sign-off. The community release at v0.10.0 ships with
  the substantive defence above; the commercial release should ship
  with a written legal opinion on top of it.
