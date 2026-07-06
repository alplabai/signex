# Branch-protection ruleset for `trunk`

`trunk-protection.json` is an importable GitHub **ruleset** that encodes the
protection policy for the default branch. Repo settings can't be committed to
git, so this file is the version-controlled record of intent — a repo admin
applies it once through the UI.

## What it enforces on `trunk`

- **No direct pushes** — changes land through a pull request.
- **1 approving review** before merge (author can't approve their own PR).
- **Required status checks** must pass:
  - `Test` — `cargo test --workspace`
  - `Check (ubuntu-latest)` — `cargo check --workspace`
  - `License + dependency audit (cargo-deny)` — license audit (advisories are informational)
  - `PR-description self-declaration` — the license-compliance block
  - `No KiCad-shaped names anywhere in crates/` — the KiCad guard
- **No branch deletion** and **no force-push** (non-fast-forward blocked).
- Merge via **merge commit** or **squash** (rebase disabled).

## How to apply

Repo → **Settings → Rules → Rulesets → New ruleset → Import a ruleset** →
choose `trunk-protection.json` → set enforcement to **Active** → save.

(Equivalent REST call for automation:
`POST /repos/alplabai/signex/rulesets` with this JSON body.)

## Tuning notes

- `required_approving_review_count`: bump to `2` for stricter review, or keep
  `1` for a small team.
- `require_code_owner_review`: set `true` to force the `CODEOWNERS` owner to
  sign off on changes in their area.
- `strict_required_status_checks_policy`: set `true` to require the PR branch be
  up to date with `trunk` before merge (more friction, fewer surprise breakages).
- To require the full KiCad-guard suite, add each guard job's name to
  `required_status_checks`. The single representative check above is the
  backstop; the rest still run and show on every PR.
