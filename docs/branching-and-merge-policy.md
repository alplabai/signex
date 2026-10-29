<!-- SPDX-License-Identifier: Apache-2.0 -->

# Branching & merge policy

The PR preconditions for `signex`: which branch work targets, which
checks gate a merge, how branches are merged, and the server-side
branch-protection settings that enforce it. Mirrors the org policy used
across Alp Lab repos (see `alp-sdk`'s equivalent doc), adapted to this
repo's `trunk`/`main` model and Apache-clean license posture.

## Branch model

```
main   ← tested, releasable baseline. Tags are cut here. Protected.
└─ trunk ← shared integration branch (default). All feature PRs land here.
   ├─ feature/…   new features
   ├─ fix/…       bug fixes
   └─ chore/… · ci/… · docs/…   housekeeping
```

- **Branch from `trunk`**, never from `main`.
- **All PRs target `trunk`.** The `trunk` → `main` promotion is itself a
  PR, held to the full gate set below.
- `main` is only ever advanced by promoting tested `trunk`.

## PR requirements (gated by branch protection)

| Gate                               | `trunk` | `main` |
|------------------------------------|---------|--------|
| Pull request required (no direct push) | Yes | Yes |
| At least 1 approving review        | Yes. A different account than the PR author; no self-approval. | Yes. Same. |
| CODEOWNERS approval                | Yes (auto-requested from [`.github/CODEOWNERS`](../.github/CODEOWNERS) for the touched paths). | Yes. |
| Stale approvals dismissed on a new push | Yes | Yes |
| Approval of the most recent push   | No | Yes |
| Conversations resolved             | Yes | Yes |
| Required status checks, branch up to date | Yes (below) | Yes (below) |
| Merge queue                        | Yes, squash | No |
| Allowed merge methods              | merge, squash (the queue squashes) | merge only |
| Force-push / deletion              | **Forbidden** | **Forbidden** |
| Bypass                             | Organization admins (always) | One named user (always) |

The bypass lists are part of the live rulesets: the earlier text of this file
said no one, admins included, could bypass. That was never true of the live
settings.

## Required CI checks

Branch protection requires exactly three status contexts on both branches:

- `ci · required` (`.github/workflows/ci.yml`) — one aggregator job. It needs
  the `check`, `test` and `lint` jobs and passes when each of them succeeded,
  or when the `changes` job skipped the Rust jobs on purpose (a draft PR, or a
  PR that only touches prose; see `scripts/ci_build_needed.sh`). The lint job
  and the aggregator always run. Adding or renaming a job under it needs no
  ruleset edit.
- `license guard · clean-room` (`.github/workflows/license-guard.yml`) — the
  case-insensitive grep for GPL-tool-shaped names under `crates/` and the other
  clean-room checks. It also runs on the merge queue.
- `PR-description self-declaration` (`.github/workflows/pr-license-declaration.yml`)
  — the license-compliance block is present and does not admit a license-gated
  source.

What the aggregated jobs run:

- `check · ubuntu-latest`, `check · macos-latest`, `check · windows-latest` —
  `cargo check --workspace` on all three targets, and `cargo clippy
  --workspace --all-targets --all-features -- -D warnings`.
- `test · workspace` — `cargo test --workspace` (headless GPU tests run on
  lavapipe).
- `lint · repo gates` — as steps of one job: the path-filter controls,
  `cargo fmt --all -- --check`, `cargo deny` (`licenses` and `bans` block;
  `sources` and `advisories` are informational), the architecture guard
  (`scripts/check_arch_guard.sh`), the god-file ratchet
  (`scripts/check_file_size.sh`), the discarded-Task ratchet
  (`scripts/check_discarded_tasks.sh`), the lint-lockstep gate
  (`scripts/check_lint_lockstep.sh`: every `[workspace.lints.*]` entry is
  present, at the same level, in each member that does not inherit with
  `[lints] workspace = true`; only `signex-app` opts out, #570) and the
  changelog fragment check (`scripts/changelog_assemble.sh --check`).

`test · library-server postgres` and `test · <os>` run on PRs but are not part
of `ci · required`. Warn-level clippy output, the `cargo deny` `sources` and
`advisories` steps, the `CHANGELOG.md should not be edited directly` warning and
the missing-fragment warning annotate a PR and do not block it.

**Clippy is half a gate, by design.** It has no status-check context of its
own; it is a step inside the three `check` jobs. Clippy's deny-by-default
lints (the whole `correctness` group), and every warn-level lint through
`-D warnings`, fail the step and so `ci · required`.

## Merge method

- **Feature / fix branch → `trunk`**: through the **merge queue**, which
  squashes (one PR = one trunk commit). The queue builds the merge group with
  all checks (`ALLGREEN`), with at most 2 entries building and 5 merging at a
  time and a 60-minute check timeout.
- **`trunk` → `main`** (release gate): **merge commit (`--no-ff`)**, crossed
  only after `trunk` is green and tested. Tags are then cut on `main`.
- **Never** a plain fast-forward that erases the branch boundary on a
  protected branch. If a bad merge lands, **revert via PR** — don't
  force-reset.

## Commit-message style

Conventional-ish commits:

```
<type>(<scope>): <short summary>

<body explaining WHY, not WHAT>
```

`<type>` ∈ `feat`, `fix`, `refactor`, `docs`, `test`, `ci`, `chore`, `perf`.
`<scope>` is the touched subtree (`footprint`, `sketch`, `library`, `ci`, …).

**Do not add `Co-Authored-By` / assistant footers.** Commits attribute
solely to the human who ran the work; LLM assistance is disclosed in the
PR's license-compliance block, not in commit trailers.

## Push policies

- Nobody pushes directly to `trunk` or `main` in the normal flow; the rulesets
  require a pull request. The bypass actors listed above can override that, so
  it is not a guarantee against an admin.
- Feature-branch pushes are unrestricted; CI runs on push for any branch.
- Force-push to a feature branch is fine while the PR is in draft; once
  review starts, prefer additional commits.
- Force-push / deletion of `trunk` or `main` is forbidden (server-side).

## Branch-protection settings (GitHub-side)

Repo settings can't live in git; this section is the audit-trail copy. The
live settings under **Settings → Rules → Rulesets** are authoritative. Three
rulesets exist: `Protect trunk` (the default branch), `main-protection`, and
`dev-protection` (blocks force-push on the legacy `dev` branch only).

[`.github/rulesets/trunk-protection.json`](../.github/rulesets/trunk-protection.json)
and [`.github/rulesets/main-protection.json`](../.github/rulesets/main-protection.json)
are the importable declarations, but they have **drifted from the live
rulesets**: they still list the eleven per-job status checks, no merge queue and
no bypass actors. Re-export the live rulesets before re-applying either file;
the tables above describe the live state as read from the GitHub API on
2026-10-29.

### `Protect trunk`

- Targets `~DEFAULT_BRANCH`. Blocks deletion and non-fast-forward updates.
- Pull request: 1 approval, Code Owner review, stale approvals dismissed on
  push, conversation resolution; the most recent push does **not** need a
  separate approval. Merge methods `merge` and `squash`.
- Status checks (strict, so the branch must be up to date): `ci · required`,
  `license guard · clean-room`, `PR-description self-declaration`.
- Merge queue: method `SQUASH`, grouping `ALLGREEN`, min 1 / max 5 entries to
  merge, max 2 to build, 60-minute check timeout.
- Bypass: the Organization admin role, always.

### `main-protection`

- Targets `refs/heads/main`. Blocks deletion and non-fast-forward updates.
- Pull request: 1 approval, Code Owner review, stale approvals dismissed,
  approval of the most recent push required, conversation resolution. Merge
  method `merge` only.
- Same three status checks, strict. No merge queue.
- Bypass: one named user account, always.

### Tuning knobs

- `main` may warrant **2** approvals once the team grows; `trunk` stays at 1
  for velocity.
- Set `strict` (up-to-date) off if rebase-before-merge friction outweighs the
  safety on a small team; the merge queue already tests each entry on top of
  the queue ahead of it.
- To require the entire License-Guard suite as separate contexts, add each
  guard job name to the required list; today `license guard · clean-room` is
  the only context from that workflow.
