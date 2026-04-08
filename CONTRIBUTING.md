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

| File | Purpose |
|------|---------|
| `src/stores/schematic.ts` | Main schematic state store — all editing operations |
| `src/canvas/SchematicRenderer.tsx` | Schematic Canvas2D rendering + mouse/keyboard handlers |
| `src/canvas/schematicDrawHelpers.ts` | Schematic draw helpers (shapes, symbols, labels) |
| `src/canvas/PcbRenderer.tsx` | PCB Canvas2D rendering |
| `src/canvas/hitTest*.ts` | Hit detection (point, area, connectivity, flood fill) |
| `src/lib/themes.ts` | 6 built-in themes with full canvas color definitions |
| `src/stores/theme.ts` | Theme state store with persistence |
| `src/lib/pcbRouter.ts` | PCB interactive routing engine |
| `src/lib/pcbDrc.ts` | PCB DRC engine (15 check types) |
| `src-tauri/src/engine/parser.rs` | KiCad schematic S-expression parser |
| `src-tauri/src/engine/pcb_parser.rs` | KiCad PCB S-expression parser |
| `src-tauri/src/engine/writer.rs` | KiCad S-expression writer |

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
| Themes | Easy | Add new color themes to `src/lib/themes.ts` |
| Properties | Easy | Improve property editing for specific object types |
| Panels | Medium | Enhance Navigator, Inspector, Variants panels |
| Export | Medium | Improve Gerber/ODB++ output fidelity |
| 3D | Medium | STEP/VRML model import for PCB 3D viewer |
| Simulation | Hard | SPICE netlist generation and waveform viewer |
| Collaboration | Hard | Comment threads, design diff, real-time editing |

## Questions?

Open an issue or start a discussion on GitHub.
