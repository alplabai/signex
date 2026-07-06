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

- **Rust 1.85+** (edition 2024)
- A GPU supporting Vulkan, Metal, or DX12 (for wgpu) — CI runs headless via lavapipe

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
cargo fmt --all               # advisory in CI, but keep it clean
cargo clippy --workspace      # advisory in CI, but review the warnings
```

`cargo test` and `cargo check` are the CI hard gates. `fmt` and `clippy`
are surfaced but don't block a merge — see "Merge rules for `trunk`" below.

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

`trunk` is protected (see [`.github/rulesets/`](.github/rulesets/)):

- Changes land via **pull request** — no direct pushes.
- **1 approving review** is required; you can't approve your own PR.
- CI hard gates must be green before merge: **Test**, **Check (ubuntu-latest)**,
  **License audit (cargo-deny)**, **PR-description self-declaration**, and the
  **KiCad-name guard**. `Format`, `clippy`, and advisory `cargo-deny` runs are
  informational and don't block.
- No force-push, no branch deletion. Merge with a merge commit or squash.

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
KiCad-derived code or any GPL-licensed dependency. KiCad import / export
lives in the [signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
GPL-3.0-or-later companion repo — that's where KiCad-related work
belongs. See [docs/LICENSING.md](docs/LICENSING.md) for the rationale
behind the two-repo split.

When you open a PR against the main `signex` repo, include this block
in the PR description:

```
Source basis: [my own work | Signex's prior code | published format
specs | other (specify)]
LLM-assisted: [yes/no — if yes, list which models]
KiCad source consulted: [yes/no — if yes, the PR belongs in
signex-kicad-import, not here]
```

CI will check the PR description for this block (see
`.github/workflows/license-guard.yml` and the PR-license-declaration
workflow). If the third field is `yes`, CI rejects the PR with a
pointer to the companion repo.

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

## Questions?

- [Open a discussion](https://github.com/alplabai/signex/discussions) for
  general questions
- [Open an issue](https://github.com/alplabai/signex/issues) for bugs or
  feature requests
- Check the [Roadmap](docs/ROADMAP.md) to see what's planned and where your
  contribution might fit
