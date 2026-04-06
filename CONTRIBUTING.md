# Contributing to Signex

Thanks for your interest in contributing to Signex! This guide will help you get started.

## Branching Strategy

```
main          Stable releases only. Protected — no direct pushes.
  |
  dev         Active development. All PRs merge here first.
  |
  feature/*   New features (branch from dev)
  fix/*       Bug fixes (branch from dev)
  docs/*      Documentation changes (branch from dev)
```

### Rules

- **`main`** is always stable and deployable. Only merged from `dev` after testing.
- **`dev`** is the integration branch. All feature/fix branches merge here via PR.
- **Never push directly to `main` or `dev`** — always use pull requests.
- PRs require at least 1 review before merging.

### Workflow

```bash
# 1. Fork the repo (external contributors) or create a branch (maintainers)
git checkout dev
git pull origin dev
git checkout -b feature/my-feature

# 2. Make your changes
# ...

# 3. Run tests before committing
npm run test                    # 53 TypeScript tests
cd src-tauri && cargo test      # 11 Rust tests
npx tsc --noEmit                # Type check

# 4. Commit with a descriptive message
git add -A
git commit -m "Add feature X: brief description"

# 5. Push and create PR against dev
git push origin feature/my-feature
# Then open a PR on GitHub: feature/my-feature → dev
```

## Development Setup

### Prerequisites

- Node.js 18+
- Rust stable
- Windows: Visual Studio Build Tools 2022 (C++ workload)
- macOS: `xcode-select --install`
- Linux: `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`

### First Time Setup

```bash
git clone https://github.com/alplabai/signex.git
cd signex
npm install
npm run tauri dev    # Start dev mode
```

## Code Style

- **TypeScript**: Follow existing patterns. No explicit `any` types.
- **Rust**: Standard `rustfmt` formatting. Run `cargo fmt` before committing.
- **CSS**: Tailwind CSS utility classes. Dark theme colors defined in `tailwind.config`.
- **No emojis** in code or commit messages unless explicitly requested.
- **Concise commits**: focus on the "why", not the "what".

## Architecture Overview

| File | Purpose | Lines |
|------|---------|-------|
| `src/stores/schematic.ts` | Main state store — all editing operations | ~1300 |
| `src/canvas/SchematicRenderer.tsx` | Canvas2D rendering + mouse/keyboard handlers | ~1900 |
| `src/canvas/hitTest.ts` | Hit detection and box selection | ~300 |
| `src/lib/netResolver.ts` | Net connectivity (union-find) | ~200 |
| `src/lib/erc.ts` | Electrical rules check (11 checks) | ~250 |
| `src/lib/ercMatrix.ts` | Pin-to-pin connection matrix (12x12) | ~200 |
| `src/panels/PropertiesPanel.tsx` | Altium-style properties for all types | ~900 |
| `src-tauri/src/engine/parser.rs` | KiCad S-expression parser | ~900 |
| `src-tauri/src/engine/writer.rs` | KiCad S-expression writer | ~300 |

### Key Patterns

- **Zustand stores** — state management, no Redux
- **Canvas2D** — rendering via `useCallback` + `requestAnimationFrame`
- **structuredClone** — undo/redo snapshots
- **Refs for cursors** — wire/placement cursor positions use `useRef` to avoid 60Hz Zustand updates
- **Pure Rust parser** — no KiCad C++ dependency, reads S-expressions directly

## Testing

### Running Tests

```bash
npm run test          # Vitest — geometry, netResolver, ERC, hitTest, store
npm run test:watch    # Watch mode

cd src-tauri
cargo test            # Rust — parser, writer, round-trip
```

### Adding Tests

- Test files go in `src/__tests__/`
- Test fixtures in `src/__tests__/fixtures/`
- Use `createSimpleSchematic()` or `createErcTestSchematic()` from fixtures
- Every new store action should have a test
- Every new ERC check should have positive and negative tests

## What to Work On

Check the [Issues](https://github.com/alplabai/signex/issues) tab for open tasks. Good areas:

| Area | Difficulty | Description |
|------|-----------|-------------|
| ERC | Easy | Add more violation types to `src/lib/erc.ts` |
| Properties | Easy | Improve property editing for specific object types |
| Drawing | Medium | Add ellipse, bezier, polygon placement tools |
| Export | Medium | PDF export, improved netlist format |
| Panels | Medium | Enhance Filter/List/Navigator panels |
| PCB | Hard | Start Phase 6 — PCB layout editor |
| AI | Hard | Start Phase 5 — Claude API integration |

## Questions?

Open an issue or start a discussion on GitHub.
