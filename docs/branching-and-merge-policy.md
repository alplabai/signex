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

For both `trunk` and `main`:

| Gate                               | Required |
|------------------------------------|----------|
| At least 1 approving review        | Yes — must be a different account than the PR author (no self-approval). |
| CODEOWNERS approval                | Yes (auto-requested from [`.github/CODEOWNERS`](../.github/CODEOWNERS) for the touched paths). |
| All required CI checks pass        | Yes (see list below). |
| Branch up to date with target      | Yes. |
| Conversations resolved             | Yes. |
| Force-push to the protected branch | **Forbidden** (admins included). |
| Direct push (non-PR)               | **Forbidden** (admins included). |

## Required CI checks

Every PR to `trunk` or `main` must pass these (job names as they appear
as status-check contexts):

- `check · ubuntu-latest` — `cargo check --workspace` (Linux is the gating
  target; `check · windows-latest` / `check · macos-latest` run too and
  should stay green, but only Linux is required).
- `test · workspace` — `cargo test --workspace` (headless GPU tests run on
  lavapipe).
- `deny · licenses + deps` — `cargo deny check licenses` (no GPL transitive
  deps). `sources` and `advisories` run in the same job but are advisory.
- `PR-description self-declaration` — the license-compliance block is present
  and does not admit KiCad-derived source.
- `No KiCad-shaped names anywhere in crates/` — the representative KiCad guard
  (the full License-Guard suite runs alongside it and shows on every PR).

**Advisory, not blocking** (surfaced as annotations): `fmt · rustfmt`,
`clippy` (inside `check`), and the `cargo-deny` `sources` / `advisories`
steps. Keep them clean, but they don't hold the merge button.

## Merge method

- **Feature / fix branch → `trunk`**: **squash** for a single logical change
  (one PR = one trunk commit), or a **merge commit (`--no-ff`)** when the
  branch's individual commits carry authorship or bisect value worth keeping
  on `trunk` — e.g. integrating an external contributor's series (the v0.14
  footprint-editor integration, PR #132, was merged this way to preserve
  @alpCaner's per-commit history).
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

- Nobody — admins included — pushes directly to `trunk` or `main`. Branch
  protection enforces this server-side.
- Feature-branch pushes are unrestricted; CI runs on push for any branch.
- Force-push to a feature branch is fine while the PR is in draft; once
  review starts, prefer additional commits.
- Force-push / deletion of `trunk` or `main` is forbidden (server-side).

## Branch-protection settings (GitHub-side)

Repo settings can't live in git; this section is the audit-trail copy. The
live settings under **Settings → Rules → Rulesets** are authoritative, and
[`.github/rulesets/trunk-protection.json`](../.github/rulesets/trunk-protection.json)
is the importable source for them.

### `trunk` (and `main`, same set)

**Pull-request requirements**
- ✅ Require a pull request before merging
  - ✅ Require approvals: **1** (different account than the author; no self-approval)
  - ✅ Dismiss stale approvals when new commits are pushed
  - ✅ Require review from Code Owners
  - ✅ Require approval of the most recent push
  - ❌ Allow specified actors to bypass required PRs (no bypass list; admins included)

**Status-check requirements**
- ✅ Require status checks to pass before merging
  - ✅ Require branches to be up to date before merging
  - Required checks (all green): `check · ubuntu-latest`, `test · workspace`,
    `deny · licenses + deps`, `PR-description self-declaration`,
    `No KiCad-shaped names anywhere in crates/`

**Merge gates**
- ✅ Require conversation resolution before merging
- ❌ Require linear history (merge commits are allowed for integrations)

**Push restrictions**
- ✅ **Do not allow bypassing the above** (admins included) — the load-bearing
  setting; it removes the "merge anyway" button for every role.
- ❌ Allow force pushes (off)
- ❌ Allow deletions (off)

### Tuning knobs

- `main` may warrant **2** approvals once the team grows; `trunk` stays at 1
  for velocity.
- Set `strict` (up-to-date) off if rebase-before-merge friction outweighs the
  safety on a small team.
- To require the entire License-Guard suite (not just the representative
  check), add each guard job name to the required list.
