# Changelog fragments

Do not edit `CHANGELOG.md` in a PR. Add a fragment here instead, so two open
PRs never conflict on the same lines (#748).

## Naming

```
changelog.d/<number>.<section>.md        # 748.changed.md
changelog.d/<number>.<section>.<k>.md    # 748.fixed.2.md  (second fragment, same number + section)
```

- `<number>` is the PR number (or the issue number if you do not have one yet).
- `<section>` is one of `added`, `changed`, `deprecated`, `removed`, `fixed`,
  `security` (the Keep a Changelog types, as in `CHANGELOG.md`).
- One PR may add several fragments, one per section.

## Content

Only the bullet(s), in the style of the existing `CHANGELOG.md` entries — no
heading:

```
- **Bold lead (#748)** — one or two sentences on what changed for a user.
```

## What happens to them

At release time `scripts/changelog_assemble.sh` folds every fragment into
`CHANGELOG.md` and deletes it. Validate yours with
`scripts/changelog_assemble.sh --check`. `README.md` and `.gitkeep` are not
fragments. CI (`changelog · fragments`) rejects PRs that edit `CHANGELOG.md`,
except release PRs (head branch `chore/release-*` or `release/*`).
