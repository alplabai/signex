# Third-party KiCad parser search — 2026-04-29

Per `.claude/PRPs/issue-62-execution-plan.md` §0.4, before committing
to a hand-rolled `kicad-parser`/`kicad-writer` in the
`signex-kicad-import` companion repo, we search crates.io for a
maintained MIT/Apache-licensed Rust KiCad parser that could simplify
the companion tool — or, if it cleanly covers all three formats,
allow consolidation back to a single Apache repo.

## Candidates surveyed

`cargo search kicad` + crates.io review on 2026-04-29.

| Crate | Latest version | License | Repo | Last commit | Maintained? | Coverage |
|---|---|---|---|---|---|---|
| `kicad_parse_gen` | 7.0.2 | MIT/Apache-2.0 | `productize/kicad-parse-gen` | 2022-09-20 | **No** (3.5 years stale) | KiCad 7-era; coverage unverified |
| `kiutils-rs` | 0.2.0 | MIT | `Milind220/kiutils-rs` | 2026-03-29 | **Yes** (1 month) | Claims `.kicad_pcb`, `.kicad_mod`, `.kicad_sch`, `.kicad_sym`, `fp-lib-table`, `sym-lib-table`, `.kicad_dru`, `.kicad_pro`; "lossless read/modify/write" |
| `kiutils_kicad` | 0.3.0 | MIT | (subcrate of `kiutils-rs`) | 2026-03-29 | **Yes** | Same as above |
| `kiutils_sexpr` | 0.1.1 | MIT | (subcrate of `kiutils-rs`) | 2026-03-29 | **Yes** | S-expression CST only |
| `serde_kicad_sexpr` | 0.1.0 | Apache-2.0 OR LGPL-3.0 | `kicad-rs/serde_kicad_sexpr` | 2025-04-04 | Borderline (1 year) | KiCad v6 S-expression schema |
| `kicad-ipc-rs` | 0.4.4 | (not surveyed) | — | — | — | IPC API client, not file format |
| `kicad-api-rs` | 0.1.0 | (not surveyed) | — | — | — | IPC API client, not file format |
| `easyeda2kicad-rs` | 1.1.2 | (not surveyed) | — | — | — | Different domain (EasyEDA → KiCad), not relevant |
| `elektron_sexp` | 0.0.0 | (not surveyed) | — | — | — | Version 0.0.0 — abandoned |

## Outcome

**Decision: stay with two-repo GPL companion structure** (Q4 option B).

Despite finding one viable candidate (`kiutils-rs`, MIT, maintained),
we keep the two-repo split for the following reasons:

### 1. Structural-derivation residual risk

Even if Signex depended on a third-party MIT parser, the **translation
logic** between that parser's KiCad-shaped data model and Signex's
native types would still encode KiCad's file-format structure inside
the Apache main repo. Seth's audit was about KiCad-derived **structure**,
not just verbatim copying — replacing our hand-rolled parser with someone
else's MIT-licensed equivalent doesn't remove the KiCad-shaped data
model from the Apache repo, it just relocates the authorship.

The structurally cleanest answer remains: **no KiCad-format-aware code
in the Apache main repo at all.** That's the two-repo split.

### 2. Sole-maintainer + low validation

`kiutils-rs` has 7 stars and a single maintainer (`Milind220`). For a
foundational migration tool, a single point of failure is too thin —
if upstream stalls, we'd inherit the maintenance burden anyway, which
defeats the "depend, don't fork" benefit of choosing it.

### 3. Reversibility

The execution plan instructs Claude to take the lower-risk choice when
hitting unanticipated decision points. Reversibility analysis:

- **Two-repo (B)**: if `kiutils-rs` matures or Seth accepts a different
  arrangement later, consolidating is a relatively contained
  refactoring (move companion-repo logic back into a single workspace,
  swap the parser).
- **Single-repo with kiutils-rs (A)**: if Seth pushes back on the
  structural-derivation point, we'd need to extract everything KiCad-
  format-aware out of the main repo — repeating the Phase 5 cutover
  retroactively under pressure.

Two-repo is the lower-risk path.

## Future revisit conditions

The decision can be revisited if:

- `kiutils-rs` reaches v1.0.0 with a stable user base and explicit
  clean-room provenance documentation.
- An organisation-backed (not solo-maintainer) MIT/Apache KiCad
  parser appears.
- KiCad upstream itself ships an Apache-licensed format library (e.g.
  if the IPC API at `kicad-ipc-rs` matures into a full file-format
  surface).

Until then: hand-rolled `kicad-parser` + `kicad-writer` move to
`signex-kicad-import` (GPL-3.0-or-later) per Phase 4.

## Reproducing the search

```bash
cargo search kicad --limit 20
cargo info kicad_parse_gen
cargo info kiutils-rs
cargo info kiutils_kicad
cargo info serde_kicad_sexpr

gh api repos/productize/kicad-parse-gen --jq '.pushed_at'
gh api repos/Milind220/kiutils-rs        --jq '.pushed_at'
gh api repos/kicad-rs/serde_kicad_sexpr  --jq '.pushed_at'
```
