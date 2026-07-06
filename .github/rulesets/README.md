# Branch-protection ruleset for `trunk`

`trunk-protection.json` is an importable GitHub **ruleset** that encodes the
protection policy for the default branch. Repo settings can't be committed to
git, so this file is the version-controlled record of intent — a repo admin
applies it once through the UI. The full policy (and the same settings in
prose, for audit) lives in
[`docs/branching-and-merge-policy.md`](../../docs/branching-and-merge-policy.md).

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
  - `No KiCad-shaped names anywhere in crates/` — the KiCad guard
- **No branch deletion** and **no force-push** (non-fast-forward blocked).
- **No bypass** (admins included) — this is the load-bearing setting.
- Merge via **merge commit** or **squash** (rebase disabled).

## How to apply

Repo → **Settings → Rules → Rulesets → New ruleset → Import a ruleset** →
choose `trunk-protection.json` → set enforcement to **Active** → save.

(Equivalent REST call for automation:
`POST /repos/alplabai/signex/rulesets` with this JSON body.)

## Tuning notes

- `required_approving_review_count`: bump to `2` for `main` / stricter review,
  or keep `1` on `trunk` for a small team's velocity.
- `require_code_owner_review`: `true` here — the matching `CODEOWNERS` owner
  must sign off on changes in their area. Set `false` to relax.
- `strict_required_status_checks_policy`: `true` here — the PR branch must be up
  to date with `trunk` before merge. Set `false` to trade a little safety for
  less rebase friction.
- To require the full KiCad-guard suite, add each guard job's name to
  `required_status_checks`. The single representative check above is the
  backstop; the rest still run and show on every PR.
