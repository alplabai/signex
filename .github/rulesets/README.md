# Branch-protection rulesets

Importable GitHub **rulesets** for the protected branches. Repo settings can't
be committed to git, so these files are the version-controlled copy of the
live rulesets — a repo admin can import or re-apply them. The full
policy (and the same settings in prose, for audit) lives in
[`docs/branching-and-merge-policy.md`](../../docs/branching-and-merge-policy.md).

| File | Branch | Ruleset name | Live id |
|------|--------|--------------|---------|
| `trunk-protection.json` | default (`trunk`) | `Protect trunk` | `18991702` |
| `main-protection.json` | `main` | `main-protection` | `14749484` |
| `dev-protection.json` | legacy `dev` | `dev-protection` | `14749564` |

> **The live rulesets are authoritative; these files are exports of them.**
> Each file is the ruleset as `gh api repos/alplabai/signex/rulesets/<id>`
> returns it, with only `name`, `target`, `enforcement`, `conditions`, `rules`
> and `bypass_actors` kept. They were last re-exported on 2026-10-30 (#1723),
> after drifting for months: they listed eleven per-job status checks, no merge
> queue and no bypass actors. **When you change a ruleset in Settings,
> re-export its file in the same PR** (see below), so the two never drift again.

## What they enforce

- **No direct pushes** — `trunk` and `main` change through a pull request.
- **1 approving review from a Code Owner**, stale approvals dismissed on new
  pushes, conversations resolved. `main` also requires approval of the most
  recent push; `trunk` does not.
- **Branch up to date** before merge (strict).
- **Required status checks** on both: `ci · required` (the aggregate of the CI
  jobs), `license guard · clean-room` and `PR-description self-declaration`.
- **`trunk` merges through a merge queue** — method `SQUASH`, grouping
  `ALLGREEN`, at most 5 entries per merge, 60-minute check timeout.
- **Merge methods:** `merge` or `squash` on `trunk`; **merge commit only** on
  `main`, because `trunk` → `main` promotion is a `--no-ff` release gate.
- **No branch deletion** and **no force-push** on `trunk` and `main`; `dev`
  only blocks force-push.
- **Bypass:** the Organization admin role on `trunk` (always), and one named
  maintainer account on `main` (always). Both are deliberate; nobody else can
  bypass.

## How to re-export

```sh
for p in "18991702 trunk-protection" "14749484 main-protection" "14749564 dev-protection"; do
  set -- $p
  gh api repos/alplabai/signex/rulesets/$1 \
    | jq '{name, target, enforcement, conditions, rules, bypass_actors}' \
    > .github/rulesets/$2.json
done
```

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
