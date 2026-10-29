# Contributing to Signex

Thanks for your interest in contributing to Signex! Whether you're fixing a bug,
adding a feature, improving docs, or just opening an issue — every contribution
helps build a better EDA tool for the community.

## Ways to Contribute

### Report Bugs

Found something broken? [Open an issue](https://github.com/alplabai/signex/issues/new)
with:

- Steps to reproduce
- Expected vs actual behavior
- A minimal `.snxsch` / `.snxpcb` that triggers the bug (if applicable)
- Screenshot if it's a rendering issue

### Suggest Features

Have an idea? [Open a discussion](https://github.com/alplabai/signex/discussions)
or issue. We're building toward Altium Designer feature parity, so if Altium
does something that Signex doesn't yet, that's a valid feature request.

### Fix Bugs or Add Features

1. Check the [open issues](https://github.com/alplabai/signex/issues) or
   [milestones](https://github.com/alplabai/signex/milestones) for things to
   work on
2. Claim the issue before you write any code — see
   [Claiming an issue](#claiming-an-issue)
3. Fork, branch, code, PR (details below)

### Improve Migration from GPL-based EDA Tools

Import of files from GPL-based EDA tools lives in the optional [import companion repository](https://github.com/alplabai/signex-kicad-import)
(GPL-3.0-or-later). PRs that improve parsing
fidelity, fix migration round-trip issues, or expand the converter's
coverage belong **there**, not in the main signex repo. The main repo
is Apache-2.0 clean and contains no code derived from GPL-based EDA tools by design — see
[docs/LICENSING.md](docs/LICENSING.md).

### Add Test Fixtures

We maintain a corpus of native Signex `.snxsch` / `.snxpcb` files under
each crate's `tests/fixtures/` (e.g. `crates/signex-types/tests/fixtures/`). Adding more fixtures from diverse designs improves
coverage. If you have a project you're willing to share (or can create
a minimal reproducer), PRs that add fixtures are very welcome.

### Improve Rendering Fidelity

Side-by-side screenshots comparing Signex's output to a reference
(Altium Designer is the canonical reference for visual fidelity per
the project's design rules) help identify rendering discrepancies.

## Development Setup

### Prerequisites

- **Rust 1.97.0** — pinned in `rust-toolchain.toml`, so `rustup` installs
  exactly this version automatically (no manual setup) and your local
  `rustfmt`/`clippy` match CI byte-for-byte. This is **also the MSRV**
  (`rust-version = "1.97"`): we build, test, and support exactly this one
  toolchain, not older ones. (Edition 2024 needs 1.85 and let-chains need 1.88 —
  both well below the pin.)
- A GPU supporting Vulkan, Metal, or DX12 (for wgpu) — CI runs headless via lavapipe

### Toolchain update cadence

The pinned toolchain is bumped to the current stable roughly **once a quarter
(every ~3 months)** so the pin never drifts far behind and we don't accumulate
toolchain debt. A bump edits three places in lockstep — `rust-toolchain.toml`
(`channel`), the `rust-version` in the root `Cargo.toml`, and every
`dtolnay/rust-toolchain@<version>` ref under `.github/workflows/` (the CI
action does **not** read `rust-toolchain.toml`; `grep -rn 'rust-toolchain@'
.github/` finds them all: `ci.yml`, `fuzz.yml` and `release.yml`) — then
`cargo fmt --all` in case the new rustfmt reformats anything.

### Build and Run

On Linux, install the development packages first (Debian and Ubuntu names):

```bash
sudo apt-get install -y \
  libxkbcommon-dev libwayland-dev libvulkan-dev \
  libx11-dev libxrandr-dev libxinerama-dev libxcursor-dev libxi-dev \
  mesa-vulkan-drivers
```

```bash
git clone https://github.com/alplabai/signex.git
cd signex
cargo build --workspace
cargo run -p signex-app
```

The release packages for Linux are built on Ubuntu 22.04 so they need glibc
2.35 or newer (`scripts/check_glibc_ceiling.sh`, #1688); a build from source
links against your own glibc.

### Verify Your Changes

```bash
cargo test --workspace        # hard gate — must pass
cargo check --workspace       # hard gate — must compile
cargo fmt --all               # hard gate — the tree must be rustfmt-clean
cargo clippy --workspace --all-targets --all-features -- -D warnings   # exactly what CI runs
```

`cargo test`, `cargo check`, `cargo fmt --all -- --check` and `clippy` are all
CI hard gates — see "Merge rules for `trunk`" below. The repo-level gates CI
runs in its `lint · repo gates` job are plain scripts you can run locally:

```bash
bash scripts/check_arch_guard.sh        # crate dependency edges vs scripts/arch-edges.txt
bash scripts/check_file_size.sh         # god-file ratchet
bash scripts/check_discarded_tasks.sh   # no `let _ =` on an iced::Task
bash scripts/check_lint_lockstep.sh     # workspace lints present in every member
bash scripts/changelog_assemble.sh --check   # changelog.d fragments
```
Clippy runs in CI with `-D warnings`, so any clippy warning fails the job and
blocks the merge (it feeds the required `ci · required` context). Run it with the
same `--all-targets --all-features` CI uses, or your local run will silently skip
`tests/`, `benches/`, `examples/`, inline `#[cfg(test)] mod tests`, and every
feature-gated module. Because the toolchain is pinned (`rust-toolchain.toml`,
Rust 1.97.0), your local `cargo fmt` output is identical to CI's — run it before
you commit (or wire it into a local `pre-commit` hook) so the formatting check in
`ci · required` stays green.

## Claiming an issue

Everyone — maintainers, contributors and AI agents — claims an issue on the
issue itself before starting work, so the issue list always shows who is
working on what. Two PRs for one issue waste a review cycle and leave one
author's work closed unmerged.

**See who is working on what:**

```sh
gh issue list --state open --assignee '*' --json number,title,assignees \
  -q '.[] | "#\(.number) \([.assignees[].login] | join(",")) — \(.title)"'
gh pr list --state open --json number,author,title \
  -q '.[] | "PR #\(.number) \(.author.login) — \(.title)"'
```

**1. Check it is free.** An issue is taken if any of these is true:

- it has an assignee;
- a comment says someone is taking it ("taking this", "working on it");
- an open PR names it (`Closes #N`, `Refs #N`, or the number in its title).

```sh
gh issue view N --json assignees,comments
gh pr list --state open --search "N"
```

If it is taken, do not start. Ask on the issue whether the holder still
wants it, or pick another one.

**2. Claim it.** Comment on the issue — "Taking this", plus one line on the
approach or the branch name. Maintainers also assign themselves
(`gh issue edit N --add-assignee @me`); contributors without triage rights
just comment and a maintainer assigns them. The assignee is what the issue
list shows; the comment is what notifies watchers.

**3. Check again before opening the PR.** Work takes hours; someone may have
claimed or fixed the same issue meanwhile. Re-run the step 1 commands right
before `gh pr create`. If another open PR already covers the issue, talk to
its author before opening yours.

**4. Release it if you stop.** Remove the assignee and leave a comment saying
what you found and where you left it. A stale claim blocks an issue just as
much as a live one. A claim with no visible activity (no branch, no PR, no
comment) for 7 days may be taken over after asking on the issue.

**Filing a follow-up you mean to do yourself?** File and claim in the same
step: assign yourself and comment "Taking this" when you open the issue. An
unassigned issue is free for anyone to take, and someone may fix it before
you get back to it.

**Your fix also covers someone else's work?** If a change you are writing
also fixes another person's claimed issue or overlaps an open PR, comment on
that issue or PR before you push, so its owner can stop or narrow their scope.

**Batch work counts too.** A script or an AI agent that works through several
issues must run steps 1–2 for each issue before it starts on that issue —
not once for the whole batch.

### Claiming fixes on a reviewed PR

When a review on a shared PR asks for changes, claim the fix work on the PR
before pushing, the same way as a review, so two sessions don't push competing
fixes to the same branch:

```
Fixing this (session <name>, head <short-sha>).
```

Pushing the fix and replying to the review releases the claim.

### Claiming a review

Reviews are claimed the same way, on the PR, so two reviewers (or two agent
sessions posting as the same account) don't review the same head twice.

**1. Check it is free.** Read the PR's comments. Skip the PR if it has a
`Reviewing this` comment that is newer than the latest push and has no
posted review after it.

```sh
gh pr view N --json comments,reviews,commits
```

**2. Claim it** before any review work starts, with the head you are
reviewing:

```
Reviewing this (session <name>, head <short-sha>).
```

Agent sessions that share one GitHub account put their session name there;
it is the only way to tell them apart.

**3. Release it.** Posting the review releases the claim. If you stop without
posting, comment `Releasing review (<reason>).` A claim older than 6 hours
with no review after it may be taken over.

## Git Workflow

### Branches

```
main     ← stable releases only (protected, requires PR + approval)
└─ trunk ← integration branch (default, all PRs target here)
   ├─ feature/...   new features
   └─ fix/...       bug fixes
```

- **Always branch from `trunk`**, not `main`
- **Always PR to `trunk`**, not `main`
- Branch naming: `feature/<description>` or `fix/<description>`

### Making a PR

1. Fork the repo
2. Create a branch from `trunk`: `git checkout -b feature/my-feature trunk`
3. Make your changes
4. Ensure `cargo test --workspace` and
   `cargo clippy --workspace --all-targets --all-features` pass
5. For a user-visible change, add a changelog fragment (see below)
6. Commit with a descriptive message: `feat: add measure tool (Ctrl+M)`
7. Re-check that no one else has claimed the issue or opened a PR for it
   ([Claiming an issue](#claiming-an-issue), step 3)
8. Push and open a PR against `trunk`, with `Closes #N` in the body

### Changelog fragments

Do **not** edit `CHANGELOG.md` in a PR: every merge would make every other open
PR conflict on it. Add `changelog.d/<number>.<section>.md` instead (`<number>` =
PR or issue number; `<section>` = `added`, `changed`, `deprecated`, `removed`,
`fixed` or `security`), holding just the bullet(s) in the existing
`CHANGELOG.md` style. Use `<number>.<section>.2.md` for a second fragment of the
same section. Naming rule and examples: [`changelog.d/README.md`](changelog.d/README.md).

Check yours with `bash scripts/changelog_assemble.sh --check`. The
`lint · repo gates` CI job runs that check on every PR (a malformed fragment
fails it), warns when a PR into `trunk` edits `CHANGELOG.md` directly (it
becomes an error after a transition window; same-repo release PRs — head
branch `chore/release-*` or `release/*` — are exempt) and warns when a
`feat`/`fix` PR has no fragment. At release time the maintainer runs
`scripts/changelog_assemble.sh --version X.Y.Z --date YYYY-MM-DD`, which folds
the fragments in and opens the dated section `release.yml` extracts. The
release PR then still adds by hand the headline, the install block and the
closing `[Full changelog](…compare/v<prev>...v<this>)` line that the Format
section of `CHANGELOG.md` requires.

### Commit Messages

We use conventional-ish commits:

```
feat: add measure tool with distance annotation
fix: wire junction not created at T-intersection
refactor: split app update into semantic modules
chore: update iced_aw to 0.13.1
docs: add a multi-sheet fixture for the netlist test
```

## Crate Map

Each crate maps to an `area:` label (auto-applied to PRs by path — see
[`.github/labeler.yml`](.github/labeler.yml)).

| Crate | What goes here | `area:` label |
|---|---|---|
| `signex-types` | Domain types (schematic, PCB, net, layer, theme) + native `.snxsch`/`.snxpcb` format codec. **No rendering deps.** | `types` |
| `signex-engine` | Edit engine + multi-window history. | `engine` |
| `signex-net` | Authoritative netlist / connectivity derivation. | `engine` |
| `signex-sketch` | 2D geometry, constraints, and the sketch solver. | `sketch` |
| `signex-bake` | Pad baking, arrays, pad/via numbering. | `bake` |
| `signex-library` / `signex-library-server` | Component library model + the library server. | `library` |
| `signex-erc` / `signex-erc-dsl` | ERC rule engine + DSL compiler. | `erc` |
| `signex-bom` | Bill-of-materials generation. | `bom` |
| `signex-output` | PDF / netlist exporters (native and neutral formats). | `output` |
| `signex-renderer` / `signex-gfx` | Canvas draw routines + GPU rendering. | `rendering` |
| `signex-3d-model-importer` | 3D model (glTF/STEP) import. | `3d` |
| `signex-widgets` / `chrome-catalog` | Custom Iced widgets + chrome catalog. | `widgets` |
| `signex-app` | Main binary — Iced app, panels, dock, menus, canvas, Active Bar, and the footprint/symbol/library editors. | `app`, `footprint-editor`, `symbol-editor`, `schematic`, `pcb` |

**Rule:** `signex-types` has zero rendering dependencies. If you need to draw
something, that code goes in `signex-renderer` / `signex-gfx`. If you need a UI
widget, that goes in `signex-widgets` or `signex-app`.

## Labels

Labels are managed as code in [`.github/labels.yml`](.github/labels.yml) and
synced automatically on merge to `trunk`. Five families plus a few signal labels:

| Family | Meaning | Who sets it |
|---|---|---|
| `type:` | Kind of change — `feature`, `bug`, `refactor`, `docs`, `ci`, `chore`, `test`, `performance` | Author / triager |
| `area:` | Subsystem touched — `sketch`, `footprint-editor`, `library`, … | **Auto** (path labeler) |
| `tier:` | The scope gate from [docs/ROADMAP.md](docs/ROADMAP.md) §4 — `p0` (required for v1.0), `p1` (v1.1–v2.0), `p2` (v2.1–v3.0), `p3` (v3.x and later) | Triager |
| `priority:` | `critical` / `high` / `medium` / `low` | Triager |
| `status:` | `needs-triage`, `in-progress`, `blocked`, `needs-review`, `on-hold` | Whoever moves it |

Signal labels: `data-loss`, `regression`, `security`, `breaking-change`,
`license-review`, `good first issue`, `help wanted`.

New issues open as `status: needs-triage` (+ `type:` from the template). On a PR,
the `area:` labels are applied for you; add a `type:` and, if warranted, a
`tier:` / `priority:` / `data-loss` / `regression` / `breaking-change` label.
Set the PR's milestone to the release it belongs to (a PR that closes an issue
takes that issue's milestone); milestones are managed as code in
[`.github/milestones.yml`](.github/milestones.yml).

## Merge rules for `trunk`

`trunk` is protected. The full policy — PR preconditions, required checks,
merge methods, and the server-side branch-protection settings — lives in
[`docs/branching-and-merge-policy.md`](docs/branching-and-merge-policy.md).
In short:

- Changes land via **pull request** — no direct pushes, no force-push, no
  branch deletion.
- **1 Code-Owner approving review** is required; you can't approve your own PR.
  Conversations must be resolved and the branch up to date with `trunk`.
- Branch protection requires three status contexts: **`ci · required`**,
  **`license guard · clean-room`** and **`PR-description self-declaration`**.
  `ci · required` aggregates `check · <os>` (check + clippy `-D warnings` on
  Linux, macOS and Windows), `test · workspace` and `lint · repo gates`
  (rustfmt, `cargo deny` licenses and bans, `arch guard`
  (`scripts/check_arch_guard.sh`: checks crate dependency edges against
  `scripts/arch-edges.txt` and keeps domain crates off the UI/GPU stack. A new
  crate edge is a design change: name the edge and its reason in the PR
  description. A domain crate never gains an edge onto the UI/GPU stack.
  `--generate` only regenerates the table after that decision), the god-file and
  discarded-Task ratchets, lint lockstep and the changelog fragment check). A
  PR that touches only prose skips the Rust jobs and still gets a green
  `ci · required` from the lint job. The `cargo-deny` sources/advisories steps
  stay informational and don't block.
- PRs merge through the **merge queue**, which squashes: one logical change,
  one commit.

## Good First Issues

Look for issues labeled [`good first issue`](https://github.com/alplabai/signex/labels/good%20first%20issue).
These are scoped, well-defined tasks that don't require deep knowledge of the
codebase.

Examples of good first contributions:

- Add a missing keyboard shortcut
- Fix a rendering discrepancy
- Add a test fixture for a corner case
- Implement a menu item that's currently a stub: add its catalog command id
  and bridge arm (see [Architecture rules](#architecture-rules)), not a handler
  wired to the menu alone
- Improve an error message
- Add a missing element type to the native format codec

## Architecture rules

These are the rules reviewers hold every PR to. The reasoning is in the linked
documents; the gates (arch guard, god-file ratchet, clippy) catch only part of
them, so the rest is on the author.

- **MVU** ([ADR-0001](docs/architecture/adr-0001-app-structure-and-boundaries.md)):
  all state lives in the model; `view` is pure; `update` is the only place
  state changes, and IO is returned as a `Task`, never run inline (the
  bounded `SaveBarrier` wait in ADR-0003 D2 is the sanctioned exception). No
  process-global app state. Message routers are exhaustive, with no `_` arm.
- **Messages:** the root `Message` gets no new flat variant; a new message
  goes into its surface's namespaced enum. Name messages as past-tense events
  (imperative names are the house style only inside `OverlayMsg`).
- **Domain logic in domain crates:** the app calls them and never re-derives
  them. Connectivity comes only from `signex-net`
  ([ADR-0002](docs/architecture/adr-0002-netlist-single-derivation.md)),
  geometry from `signex-sketch`, baking from `signex-bake`. Domain crates take
  no `iced` / `wgpu` dependency, and crate edges are the ones in
  `scripts/arch-edges.txt`.
- **Commands** ([docs/COMMANDS.md](docs/COMMANDS.md)): a new user-invokable
  action gets one catalog `AppCommandId` and one bridge arm, and every surface
  row for it (menu, context menu, keymap, palette, Active Bar) takes its label
  and shortcut from the catalog rather than a private literal. Catalog ids with
  no arm yet are listed in `UNMAPPED_CATALOG_IDS` in
  `crates/signex-app/src/app/command/bridge.rs`, a ratchet that only shrinks.
  Ids are public API: never renamed or removed without an alias. A command is
  not a `Message`.
- **Deferred work**
  ([ADR-0003](docs/architecture/adr-0003-deferred-work-and-consistent-history.md)):
  owed work is explicit model state. Every boundary (save, undo, redo,
  document or tab switch) runs the ADR-0003 D2 flush first (the `finish_pending` contract; in code,
  `settle_pending_solve` in the footprint editor and the `SaveBarrier` wait
  for document writes), and undo snapshots are only of finished states.
- **Persistence:** document writes go through
  `signex_types::atomic_io::atomic_write`. External input is never
  `unwrap`ped. Failures reach the Messages panel.
- **Files:** production files stay under the god-file cap
  (`scripts/check_file_size.sh`). Split by domain into a folder (`foo/mod.rs`
  plus `foo/<concern>.rs`) without a redundant prefix. One decision, one
  helper.

## Code Style

- Follow existing patterns in the codebase
- Use `?` for error propagation, not `unwrap()` / `expect()` in production code
- Altium Designer UX is the reference for all user-facing interactions
- If you're unsure about a design choice, open a draft PR and ask
- **Keep comments short; put long explanations in `docs/`.** A comment in code
  says what the next reader needs at that spot: the non-obvious *why*, a
  constraint, a trap. Background, history ("this used to…, until #N…"),
  alternatives that were rejected and multi-paragraph design rationale go in a
  Markdown document under `docs/` (for example `docs/architecture/` or
  `docs/RENDERING_RULES.md`). The code then points to that section in one line,
  e.g. `// Flag geometry: see docs/RENDERING_RULES.md, "sch-labels".` As a rule
  of thumb, a comment block longer than about ten lines belongs in a document.
  The history of a change belongs in its commit message and PR, not in the code.

### Object ids

Every new id is a UUID v7: call `Uuid::now_v7()`. `clippy.toml` disallows
`Uuid::new_v4()`, and CI runs clippy with `-D warnings`, so a new v4 call fails
the build. (The workspace lists `uuid` without `v4`, but a dependency turns
that feature back on, so the lint is the real guard.)

- **A v7 id carries its creation time.** Its first 48 bits are a millisecond
  Unix timestamp. `signex_types::object_id::created_at` reads it back, and
  returns `None` for any other version.
- **Ids sort by creation.** That keeps index order and the row diffs of
  `.snxsch` / `.snxlib` files steadier than random ids did.
- **Existing ids are never rewritten.** v4 ids in older files stay valid;
  nothing reads the version nibble except `created_at`.
- **Shorten an id from its tail.** Ids made in the same minute share their
  leading hex digits, so a display label uses
  `signex_types::object_id::short_id` (the last 8 digits), never the prefix.
- **An id is not a secret.** Tokens, nonces and OAuth state come from a CSPRNG
  (the `oauth2` crate's own generators), not from a UUID.

## Contributor License Agreement

By submitting a pull request, you agree that your contribution is licensed under
the same [Apache-2.0 license](LICENSE) as the rest of the project.

We require this so that the project can maintain a consistent license and offer
Signex Pro under a separate commercial license without needing to re-negotiate
with every contributor.

## License compliance for contributions

The main signex repo is **Apache-2.0 clean**. Patches must not introduce
code, data, or dependencies under any licence incompatible with
Apache-2.0 — which is a wider net than GPL, and the next section defines
it. Import from GPL-based EDA tools lives in the
[import companion repository](https://github.com/alplabai/signex-kicad-import)
(GPL-3.0-or-later) — that's where work on those formats
belongs. See [docs/LICENSING.md](docs/LICENSING.md) for the full statement
and the rationale behind the two-repo split.

### What "otherwise Apache-incompatible" means

Opening a PR affirms that **no license-gated source files** were used —
nothing under GPL/copyleft **or otherwise Apache-incompatible**. We asked
that for a long time without ever saying what the second half meant, which
was our omission, not a contributor's problem to guess at.

It cost someone. [PR #304](https://github.com/alplabai/signex/pull/304)
arrived as a skilled, careful Rust rewrite of a project licensed
"CC BY 4.0 … You may not resell this tool". That is Apache-incompatible on
two counts, and it passed all twelve of our licence CI jobs plus
`cargo deny` green. The declaration was answered honestly — CC BY reads as
permissive, and the resale restriction is a trailing sentence that isn't
part of the CC BY licence text at all. Nothing we automated would have
changed that answer. Only writing the rule down does. So:

**A port is a derivative work.** Rewriting a project's JavaScript in Rust
does not reset its copyright. Neither does re-typing its C++, renaming the
identifiers, restructuring the modules, or having an LLM do the
translation. If you read someone else's source and wrote code that follows
it, their licence governs your result — however different it looks. This is
the single point engineers most often don't know, and it is not a close
call legally.

Implementing a *published algorithm or formula* independently is a
different thing and is fine. The line is what you had in front of you when
you wrote it, not how much the output diverges.

**Licence classes that are incompatible with this repo:**

- **GPL / copyleft** — GPL-2.0/3.0, AGPL, and **LGPL**. Reciprocal terms
  relicense Signex; a binding is a link. Copyleft solvers are reached
  across a process boundary only — see
  [docs/EXTERNAL_TOOLS.md §4](docs/EXTERNAL_TOOLS.md#4-the-gpl--lgpl-bridge-boundary),
  which is the dependency-side counterpart to this section.
- **Any Creative Commons licence** — CC BY, CC BY-SA, CC BY-NC, all of
  them. CC is not a software licence; Creative Commons says so itself. CC
  BY's attribution terms don't compose with Apache-2.0's `NOTICE` model,
  and BY-SA is copyleft. "But CC BY is permissive" is the exact trap #304
  fell into. (CC0 is a public-domain dedication, not a CC licence in this
  sense, and is fine.)
- **Non-commercial / no-resale / any field-of-use restriction** — CC BY-NC,
  "you may not resell this tool", "personal use only", "not for commercial
  use". See below; this class is fatal here specifically.
- **Source-available / open-core / "fair source"** — BUSL, SSPL, Elastic,
  Commons Clause, PolyForm. Not open source, whatever the marketing says.
- **The text, tables, and figures of paywalled standards** — IPC, IEC,
  JEDEC, ISO. Important distinction: the **formulas and physical facts** in
  a standard are facts, not copyrightable, and implementing them from your
  own understanding is fine and welcome. The **document** is copyrighted —
  do not copy its prose, its tables, its figure geometry, or its worked
  examples, and don't paste it into an LLM to do it for you.

MIT, BSD, ISC, Zlib, Unlicense, CC0, and Apache-2.0 are fine. Anything on
neither list: ask.

**Why "no resale" is fatal here in particular.** Signex Community is
Apache-2.0 and free; **Signex Pro is a paid commercial edition built from
this same tree**. A field-of-use restriction on any code in `crates/` would
be violated the day Pro ships, and would break the Apache-2.0 surface we
promise every downstream redistributor and embedder. Plenty of projects
could live with a non-commercial clause. We cannot. That's a property of
our business model, not a judgement about the licence.

### If you're not sure, ask — don't PR

[Open an issue](https://github.com/alplabai/signex/issues/new), name the
source and its licence, and we'll answer. It costs you one comment. A wrong
guess discovered at review costs you the weekend you spent on the code, and
we would rather spend our time saying "yes, go" than "sorry". This is the
same rule [docs/EXTERNAL_TOOLS.md §1](docs/EXTERNAL_TOOLS.md#1-the-rule)
applies to stack choices, for the same reason.

### Mechanics

The PR template asks you to confirm the work is original or derived only
from sources whose licence you checked, and to name the source and licence
if it is derived. CI (see `.github/workflows/license-guard.yml` and the
PR-license-declaration workflow) passes unless the description explicitly
admits a license-gated source. If your contribution did draw on one, add a
line `License-gated sources: yes` — CI rejects it here with a pointer to
the companion repo, which is where that work belongs.

A PR that adds a new crate or more than ~2000 lines of Rust also gets an
automated comment asking the provenance question directly. It's advisory,
it never blocks, and it fires on plenty of entirely original work — if it
lands on yours, it means nothing more than "this PR is large".

Why this matters: large-language-model assistants that have been
trained on GPL-based EDA tool source can inadvertently produce structurally
derivative code even when generating "from scratch." If your LLM has
been exposed to such source, route the contribution to the GPL
companion (where derivation is fine) rather than the Apache main repo.

For reference, the License Guard CI also fails any push that
introduces the GPL-based EDA tool's name, matched case-insensitively as a
substring, anywhere in a `.rs` or `.toml` file under `crates/` (code,
comments and strings alike), or a `const` / `static` named after one of its
numbered layer ids (`F_CU`, `B_CU`, …). The other blocking jobs reject
GPL-licensed parser/writer imports and Cargo dependencies; the removed API
surface (`PinElectricalType`, `parse_markup(`, …); that tool's auto-net-name
format (`Net-(…-Pad…)`), C++ class and file references in comments, and
schematic/PCB/symbol file-format literals; the bare word `Standard` in
`signex-renderer` / `signex-engine` comments; solver names and attribution
(SolveSpace, FreeCAD, planegcs, OpenCascade) in `signex-sketch` /
`signex-bake`; and mirror-source markers. The patterns are in
`.github/workflows/license-guard.yml`.
This is a structural backstop on top of the PR-description self-
declaration.

Those gates are shaped around one past incident and they do not detect a
port of some project they've never heard of. There's also an advisory
`port-smell` job that greps for residue a port tends to leave — it never
blocks, and a careful port trips none of it. Treat none of this as a
clean bill of health: the section above is the actual rule, and reading
it is the thing that works.

## Questions?

- [Open a discussion](https://github.com/alplabai/signex/discussions) for
  general questions
- [Open an issue](https://github.com/alplabai/signex/issues) for bugs or
  feature requests
- Check the [Roadmap](docs/ROADMAP.md) to see what's planned and where your
  contribution might fit
