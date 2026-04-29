# Contributor record — 2026-04-29

Snapshot taken at the start of the issue-62 Apache-only remediation.
Lists every author whose work appears in the main `signex` repo's git
history since 2024-01-01, captured for the public audit trail.

Snapshot ref: `audit-baseline-2026-04-29` (tag) → `dev` tip `0e74ebc`.

## Authors

| Name | Email | Apache-2.0 patches stay Apache | LLM-tagged commits |
|---|---|---|---|
| Caner Alp | `caner.alp@alpelectronix.com` | yes | yes (LLM-assisted, Caner is the human author of record) |
| Caner Alp (GitHub no-reply) | `109098482+alpCaner@users.noreply.github.com` | yes | yes |
| Hakan Gülen | `5780826+hkngln@users.noreply.github.com` | yes | no |

All commits since `2024-01-01` were authored by one of the three identities above. No third-party patches landed during this period.

## Notes

- The Apache-2.0 grants on existing patches stay in force — this is a record of consent for the **Apache-only direction** going forward, not a relicense.
- LLM-assisted commits remain attributable to the human author who reviewed and merged them. Per `.claude/CLAUDE.md`, Signex never adds AI as a commit author or co-author; LLMs are tools used by the human author.
- Going forward (Phase 8), every PR declares whether it consulted KiCad source. PRs that consult KiCad source belong in the `signex-kicad-import` GPL-3.0 companion repo, not the main repo.

## Reproducing this snapshot

```bash
git log --since=2024-01-01 --pretty="%aN <%aE>" | sort -u
```
