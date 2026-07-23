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
2. Comment on the issue to let us know you're working on it
3. Fork, branch, code, PR (details below)

### Improve the KiCad Migration Path

KiCad import / export lives in the optional [signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
companion repo (GPL-3.0-or-later). PRs that improve KiCad parsing
fidelity, fix migration round-trip issues, or expand the converter's
coverage belong **there**, not in the main signex repo. The main repo
is Apache-2.0 clean and contains no KiCad-derived code by design — see
[docs/LICENSING.md](docs/LICENSING.md).

### Add Test Fixtures

We maintain a corpus of native Signex `.snxsch` / `.snxpcb` files in
`tests/fixtures/`. Adding more fixtures from diverse designs improves
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
(`channel`), the `rust-version` in the root `Cargo.toml`, and the four
`dtolnay/rust-toolchain@<version>` refs in `.github/workflows/ci.yml` (the CI
action does **not** read `rust-toolchain.toml`) — then `cargo fmt --all` in case
the new rustfmt reformats anything.

### Build and Run

```bash
git clone https://github.com/alplabai/signex.git
cd signex
cargo build --workspace
cargo run -p signex-app
```

### Verify Your Changes

```bash
cargo test --workspace        # hard gate — must pass
cargo check --workspace       # hard gate — must compile
cargo fmt --all               # hard gate — the tree must be rustfmt-clean
cargo clippy --workspace      # advisory in CI, but review the warnings
```

`cargo test`, `cargo check`, and `cargo fmt --all -- --check` are the CI hard
gates; `clippy` is surfaced but doesn't block — see "Merge rules for `trunk`"
below. Because the toolchain is pinned (`rust-toolchain.toml`, Rust 1.97.0),
your local `cargo fmt` output is identical to CI's — run it before you commit
(or wire it into a local `pre-commit` hook) so the required `fmt · rustfmt`
check stays green.

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
4. Ensure `cargo test` and `cargo clippy` pass
5. Commit with a descriptive message: `feat: add measure tool (Ctrl+M)`
6. Push and open a PR against `trunk`

### Commit Messages

We use conventional-ish commits:

```
feat: add measure tool with distance annotation
fix: wire junction not created at T-intersection
refactor: split app update into semantic modules
chore: update iced_aw to 0.13.1
docs: add KiCad 9 fixture for multi-sheet test
```

## Crate Map

Each crate maps to an `area:` label (auto-applied to PRs by path — see
[`.github/labeler.yml`](.github/labeler.yml)).

| Crate | What goes here | `area:` label |
|---|---|---|
| `signex-types` | Domain types (schematic, PCB, net, layer, theme) + native `.snxsch`/`.snxpcb` format codec. **No rendering deps.** | `types` |
| `signex-engine` | Edit engine + multi-window history. | `engine` |
| `signex-sketch` | 2D geometry, constraints, and the sketch solver. | `sketch` |
| `signex-bake` | Pad baking, arrays, pad/via numbering. | `bake` |
| `signex-library` / `signex-library-server` | Component library model + the library server. | `library` |
| `signex-erc` / `signex-erc-dsl` | ERC rule engine + DSL compiler. | `erc` |
| `signex-bom` | Bill-of-materials generation. | `bom` |
| `signex-output` | PDF / netlist exporters (non-KiCad formats). | `output` |
| `signex-renderer` / `signex-gfx` | Canvas draw routines + GPU rendering. | `rendering` |
| `signex-3d-model-importer` | 3D model (glTF/STEP) import. | `3d` |
| `signex-widgets` / `chrome-catalog` | Custom Iced widgets + chrome catalog. | `widgets` |
| `signex-app` | Main binary — Iced app, panels, dock, menus, canvas, Active Bar, and the footprint/symbol/library editors. | `app`, `footprint-editor`, `symbol-editor`, `schematic`, `pcb` |

**Rule:** `signex-types` has zero rendering dependencies. If you need to draw
something, that code goes in `signex-renderer` / `signex-gfx`. If you need a UI
widget, that goes in `signex-widgets` or `signex-app`.

## Labels

Labels are managed as code in [`.github/labels.yml`](.github/labels.yml) and
synced automatically on merge to `trunk`. Four families plus a few signal labels:

| Family | Meaning | Who sets it |
|---|---|---|
| `type:` | Kind of change — `feature`, `bug`, `refactor`, `docs`, `ci`, `chore`, `test`, `performance` | Author / triager |
| `area:` | Subsystem touched — `sketch`, `footprint-editor`, `library`, … | **Auto** (path labeler) |
| `priority:` | `critical` / `high` / `medium` / `low` | Triager |
| `status:` | `needs-triage`, `in-progress`, `blocked`, `needs-review`, `on-hold` | Whoever moves it |

Signal labels: `data-loss`, `regression`, `security`, `breaking-change`,
`license-review`, `good first issue`, `help wanted`.

New issues open as `status: needs-triage` (+ `type:` from the template). On a PR,
the `area:` labels are applied for you; add a `type:` and, if warranted, a
`priority:` / `data-loss` / `regression` / `breaking-change` label.

## Merge rules for `trunk`

`trunk` is protected. The full policy — PR preconditions, required checks,
merge methods, and the server-side branch-protection settings — lives in
[`docs/branching-and-merge-policy.md`](docs/branching-and-merge-policy.md);
the importable ruleset is in [`.github/rulesets/`](.github/rulesets/). In short:

- Changes land via **pull request** — no direct pushes, no force-push, no
  branch deletion (admins included).
- **1 Code-Owner approving review** is required; you can't approve your own PR.
  Conversations must be resolved and the branch up to date with `trunk`.
- CI hard gates must be green before merge: `check · ubuntu-latest`,
  `test · workspace`, `deny · licenses + deps`, `PR-description self-declaration`,
  `No GPL-tool-shaped names anywhere in crates/`, and `fmt · rustfmt`. `clippy`
  and the advisory `cargo-deny` (sources/advisories) steps are informational and
  don't block.
- Merge with a **merge commit** (to preserve a contributor's per-commit history)
  or **squash** (one logical change → one commit).

## Good First Issues

Look for issues labeled [`good first issue`](https://github.com/alplabai/signex/labels/good%20first%20issue).
These are scoped, well-defined tasks that don't require deep knowledge of the
codebase.

Examples of good first contributions:

- Add a missing keyboard shortcut
- Fix a rendering discrepancy
- Add a test fixture for a corner case
- Implement a menu item that's currently a stub
- Improve an error message
- Add a missing element type to the native format codec

## Code Style

- Follow existing patterns in the codebase
- Use `?` for error propagation, not `unwrap()` / `expect()` in production code
- Altium Designer UX is the reference for all user-facing interactions
- If you're unsure about a design choice, open a draft PR and ask

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
it. KiCad import / export lives in the
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
GPL-3.0-or-later companion repo — that's where KiCad-related work
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
trained on KiCad source can inadvertently produce structurally
derivative code even when generating "from scratch." If your LLM has
been exposed to KiCad source, route the contribution to the GPL
companion (where derivation is fine) rather than the Apache main repo.

For reference, the License Guard CI also fails any push that
introduces KiCad-flavoured identifiers (`kicad`, `KiCad`, `F_CU`,
`B_CU`, `F_SILKS`, `tri_state`, `Net-(`, …) anywhere under `crates/`.
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
