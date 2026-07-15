# Branch-protection ruleset for `trunk`

`trunk-protection.json` is an importable GitHub **ruleset** that encodes the
protection policy for the default branch. Repo settings can't be committed to
git, so this file is the version-controlled record of intent — a repo admin
applies it once. The full policy (and the same settings in prose, for audit)
lives in
[`docs/branching-and-merge-policy.md`](../../docs/branching-and-merge-policy.md).

> **This file is a declaration, not the live state.** It described `trunk`'s
> protection for months while nothing enforced it: the ruleset had never been
> applied, the only active rulesets targeted the legacy `main` and `dev`
> branches, and `trunk` — the default, where every PR lands — was wide open.
> Applied 2026-07-15 as ruleset "Protect trunk". **If you edit this file,
> re-apply it** (see below) and verify with `gh api repos/alplabai/signex/rules/branches/trunk`.

## What it enforces on `trunk`

- **No direct pushes** — changes land through a pull request.
- **1 approving review** before merge (author can't approve their own PR),
  **from a Code Owner**, with stale approvals dismissed on new pushes and the
  most-recent push required to be approved.
- **Conversations resolved** before merge.
- **Branch up to date** with `trunk` before merge (strict).
- **Required status checks** must pass:
  - `check · ubuntu-latest` — `cargo check --workspace`
  - `test · workspace` — `cargo test --workspace`
  - `deny · licenses + deps` — license audit (sources/advisories are informational)
  - `PR-description self-declaration` — the license-compliance block
  - `No GPL-tool-shaped names anywhere in crates/` — the licence guard
- **No branch deletion** and **no force-push** (non-fast-forward blocked).
- **No bypass** (admins included) — this is the load-bearing setting.
- Merge via **merge commit** or **squash** (rebase disabled).

## How to apply

Repo → **Settings → Rules → Rulesets → New ruleset → Import a ruleset** →
choose `trunk-protection.json` → set enforcement to **Active** → save.

Or, to create it:

```sh
gh api repos/alplabai/signex/rulesets --method POST \
  --input .github/rulesets/trunk-protection.json
```

To update the ruleset in place after editing this file (`<id>` from
`gh api repos/alplabai/signex/rulesets`):

```sh
gh api repos/alplabai/signex/rulesets/<id> --method PUT \
  --input .github/rulesets/trunk-protection.json
```

**Validate the required check names first.** `required_status_checks` blocks a
merge until every listed context reports, so a context that matches no job name
never reports and the branch deadlocks — unmergeable, with no obvious cause.
This is not hypothetical: the list here referenced `No KiCad-shaped names
anywhere in crates/` long after the guard was renamed, and the only reason it
never bit anyone is that the ruleset was never applied.

Check the contexts against a **pull-request** run, not a `trunk` commit —
`PR-description self-declaration` only runs on `pull_request` events and is
absent from push-event check-runs:

```sh
gh api repos/alplabai/signex/commits/<pr-head-sha>/check-runs -q '.check_runs[].name'
```

## Tuning notes

- `required_approving_review_count`: bump to `2` for `main` / stricter review,
  or keep `1` on `trunk` for a small team's velocity.
- `require_code_owner_review`: `true` here — the matching `CODEOWNERS` owner
  must sign off on changes in their area. Set `false` to relax.
- `strict_required_status_checks_policy`: `true` here — the PR branch must be up
  to date with `trunk` before merge. Set `false` to trade a little safety for
  less rebase friction.
- To require the full License-Guard suite, add each guard job's name to
  `required_status_checks`. The single representative check above is the
  backstop; the rest still run and show on every PR.
