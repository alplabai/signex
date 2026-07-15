# Branch-protection rulesets

Importable GitHub **rulesets** encoding the protection policy for the two
protected branches. Repo settings can't be committed to git, so these files are
the version-controlled record of intent — a repo admin applies them. The full
policy (and the same settings in prose, for audit) lives in
[`docs/branching-and-merge-policy.md`](../../docs/branching-and-merge-policy.md).

| File | Branch | Ruleset name |
|------|--------|--------------|
| `trunk-protection.json` | default (`trunk`) | `Protect trunk` |
| `main-protection.json` | `main` | `main-protection` |

> **These files are declarations, not the live state.** `trunk-protection.json`
> described `trunk`'s protection for months while nothing enforced it: it had
> never been applied, the only active rulesets targeted `main` and the legacy
> `dev`, and `trunk` — the default, where every PR lands — was wide open.
> `main-protection` *was* live but far weaker than the policy claimed: it
> required a pull request and **zero approvals**, with no status checks and
> rebase merges allowed, on the branch where release tags are cut.
>
> Both were reconciled on 2026-07-15 and now match the policy. **If you edit
> either file, re-apply it** (see below) and verify:
>
> ```sh
> gh api repos/alplabai/signex/rules/branches/trunk
> gh api repos/alplabai/signex/rules/branches/main
> ```

## What they enforce

Both branches carry the **same** set, per the policy's "`trunk` (and `main`,
same set)". The one deliberate difference is the merge method: `trunk` takes
squash or merge commits, while `main` accepts **merge commits only**, because
`trunk` → `main` promotion is a `--no-ff` release gate.

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
- Merge via **merge commit** or **squash** on `trunk`; **merge commit only** on
  `main` (rebase disabled on both).

## How to apply

Repo → **Settings → Rules → Rulesets → New ruleset → Import a ruleset** →
choose the JSON → set enforcement to **Active** → save.

Or, to create one:

```sh
gh api repos/alplabai/signex/rulesets --method POST \
  --input .github/rulesets/trunk-protection.json
```

To update a ruleset in place after editing its file (`<id>` from
`gh api repos/alplabai/signex/rulesets`):

```sh
gh api repos/alplabai/signex/rulesets/<id> --method PUT \
  --input .github/rulesets/main-protection.json
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
