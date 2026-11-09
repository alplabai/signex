- # Changelog fragments

Do not edit `CHANGELOG.md` in a PR. Add a fragment here instead, so two open
PRs never conflict on the same lines (#748).

## Naming

```
changelog.d/<number>.<section>.md        # 748.changed.md
changelog.d/<number>.<section>.<k>.md    # 748.fixed.2.md  (second fragment, same number + section)
```

- `<number>` is the PR number (preferred); use the issue number only if you
  need to name the file before the PR exists. If the name is taken, use
  `.2` / `.3`.
- `<section>` is one of `added`, `changed`, `deprecated`, `removed`, `fixed`,
  `security` (the Keep a Changelog types, as in `CHANGELOG.md`).
- One PR may add several fragments, one per section.

## Content

Only the bullet(s), in the style of the existing `CHANGELOG.md` entries — no
heading:

```
- **Bold lead (#748)** — one or two sentences on what changed for a user.
```

A breaking change (a file older Signex cannot read, a removed command or
shortcut, a changed default) is a `changed` or `removed` fragment whose bullet
starts with **Breaking:**, so the release notes can lift it into their
`### Breaking changes` section:

```
- **Breaking: `.snxsch` sheets saved by this version need 26.11 or later (#748)** — one or two sentences on what a user must do.
```

## When CI requires one

On every PR into `trunk` the `lint · repo gates` job fails when:

- the PR title starts with the lowercase type `feat`, `fix` or `perf` (with
  or without scope or `!`) and the PR adds no fragment. A change that is not user-visible takes another
  type (`chore`, `refactor`, `test`, `ci`, `docs`); after a retitle, re-run the
  job, it reads the current title;
- the PR edits `CHANGELOG.md` directly, except same-repo release PRs (head
  branch `chore/release-*` or `release/*`).

## What happens to them

At release time `scripts/changelog_assemble.sh` folds every fragment into
`CHANGELOG.md` and deletes it. Validate yours with
`scripts/changelog_assemble.sh --check`. `README.md` and `.gitkeep` are not
fragments. The `lint · repo gates` CI job runs the `--check` on every PR.

After a `--version` cut the release PR still adds, by hand, the headline, the
install block and the closing
`[Full changelog](…/compare/v<prev>...v<this>)` line that CHANGELOG.md's Format
section requires; the script prints a reminder.
