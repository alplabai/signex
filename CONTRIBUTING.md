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
- KiCad file that triggers the bug (if applicable)
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

### Improve KiCad Compatibility

Signex reads and writes KiCad files. If you find a KiCad file that doesn't
parse correctly, or if Signex's output causes issues when opened in KiCad,
that's a high-priority bug. We especially value:

- Test files from real projects (anonymized if needed)
- KiCad version-specific edge cases
- Round-trip issues (open in Signex, save, open in KiCad, something changed)

### Add Test Fixtures

We maintain a corpus of real KiCad files in `tests/fixtures/`. Adding more
fixtures from diverse projects improves our parser and writer coverage. If you
have a KiCad project you're willing to share (or can create a minimal
reproducer), PRs that add fixtures are very welcome.

### Improve Rendering Fidelity

If a schematic renders differently in Signex vs KiCad, that's a bug. Side-by-side
screenshots comparing KiCad and Signex output are extremely helpful for
identifying rendering discrepancies.

## Development Setup

### Prerequisites

- **Rust 1.80+** (edition 2024)
- A GPU supporting Vulkan, Metal, or DX12 (for wgpu)
- A KiCad installation (for test files and visual comparison)

### Build and Run

```bash
git clone https://github.com/alplabai/signex.git
cd signex
cargo build --workspace
cargo run -p signex-app
```

### Verify Your Changes

```bash
cargo test --workspace        # All tests pass
cargo clippy --workspace -- -D warnings   # Zero warnings
```

Both must pass before opening a PR.

## Git Workflow

### Branches

```
main   ← stable releases only (protected, requires PR + approval)
└─ dev ← integration branch (default, all PRs target here)
   ├─ feature/...   new features
   └─ fix/...       bug fixes
```

- **Always branch from `dev`**, not `main`
- **Always PR to `dev`**, not `main`
- Branch naming: `feature/<description>` or `fix/<description>`

### Making a PR

1. Fork the repo
2. Create a branch from `dev`: `git checkout -b feature/my-feature dev`
3. Make your changes
4. Ensure `cargo test` and `cargo clippy` pass
5. Commit with a descriptive message: `feat: add measure tool (Ctrl+M)`
6. Push and open a PR against `dev`

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

| Crate | What goes here | Dependencies |
|---|---|---|
| `signex-types` | Domain types (schematic, PCB, net, layer, theme). **No rendering deps.** | serde, uuid |
| `kicad-parser` | S-expression tokenizer + parsers for .kicad_sch/.kicad_pcb/.kicad_sym | signex-types |
| `kicad-writer` | S-expression serializer (write KiCad format back) | signex-types |
| `signex-render` | Canvas draw routines, hit-testing. Bridges types to Iced Canvas calls. | signex-types |
| `signex-widgets` | Custom Iced widgets (TreeView, symbol preview, theme extensions) | iced, iced_aw |
| `signex-app` | Main binary. Iced Application, panels, dock, menus, canvas, Active Bar. | everything above |

**Rule:** `signex-types` has zero rendering dependencies. If you need to draw
something, that code goes in `signex-render`. If you need a UI widget, that goes
in `signex-widgets` or `signex-app`.

## Good First Issues

Look for issues labeled [`good first issue`](https://github.com/alplabai/signex/labels/good%20first%20issue).
These are scoped, well-defined tasks that don't require deep knowledge of the
codebase.

Examples of good first contributions:

- Add a missing keyboard shortcut
- Fix a rendering discrepancy vs KiCad
- Add a KiCad test fixture
- Implement a menu item that's currently a stub
- Improve an error message
- Add a missing element type to the parser

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

## Questions?

- [Open a discussion](https://github.com/alplabai/signex/discussions) for
  general questions
- [Open an issue](https://github.com/alplabai/signex/issues) for bugs or
  feature requests
- Check the [Roadmap](docs/ROADMAP.md) to see what's planned and where your
  contribution might fit
