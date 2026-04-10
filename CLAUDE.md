# Signex Iced — AI-first EDA

## Project

Signex is an AI-first electronics design automation tool targeting Altium Designer feature parity.
Built with **Iced 0.14** (Elm architecture) + **wgpu** for GPU-accelerated rendering.

## Architecture

- `crates/signex-app/` — Main binary. Iced Application, panels, dock system, menus, toolbars.
- `crates/signex-types/` — Domain types. Schematic, PCB, net, layer, theme. NO rendering deps.
- `crates/kicad-parser/` — S-expression parser for .kicad_sch, .kicad_pcb, .kicad_sym files.
- `crates/kicad-writer/` — S-expression serializer (write KiCad format back).
- `crates/signex-render/` — wgpu rendering primitives. Bridges types → Iced Canvas draw calls.

## Build

```bash
cargo build --workspace          # Debug build
cargo build --workspace --release # Release build
cargo test --workspace           # All tests
cargo clippy --workspace -- -D warnings  # Lint
```

## Rules

- **Never add Claude as a commit author, co-author, or contributor.** No `Co-Authored-By` lines. No mentions in CONTRIBUTORS files. Claude is a tool, not a contributor.
- **Never push directly to `main`.** All work goes through `dev` via feature branches and PRs.

## Git Workflow

```
main                    # Protected. Stable releases only. Tagged vX.Y.Z.
└── dev                 # Default branch. All feature branches merge here via PR.
    ├── feature/...     # New features: feature/phase-3-canvas, feature/add-grid
    ├── fix/...         # Bug fixes: fix/parser-unicode, fix/dock-collapse
    └── hotfix/...      # Urgent fixes branched from main, merged to both main and dev
```

- `main` is protected — requires PR with 1 approval, no direct pushes, no force pushes.
- `dev` is the integration branch. Feature/fix branches merge here via PR.
- Feature branches: `feature/<description>` — create from dev, PR back to dev.
- Bug fix branches: `fix/<description>` — create from dev, PR back to dev.
- Hotfixes: `hotfix/<description>` — branch from main for critical production fixes, merge to both main and dev.
- Every merge to main gets a version tag (e.g., `v0.3.0`).

## Conventions

- **Coordinate system:** i64 nanometers internally. KiCad uses mm floats — convert at parse/write boundary.
- **Types crate has ZERO rendering deps.** All rendering goes through signex-render.
- **Iced patterns:** Elm architecture — Message enum, update(), view(). No interior mutability.
- **Theme:** 6 built-in themes (Catppuccin Mocha, VS Code Dark, Altium Dark, GitHub Dark, Solarized Light, Nord).
- **Canvas:** Use `iced::widget::Canvas` for schematic (CPU tessellation OK for <10K elements).
  Use `iced::widget::Shader` for PCB (100K+ elements need GPU instanced rendering).
- **Panel docking:** Custom DockArea wrapping panels with tabs. No floating panels yet.
- **Keyboard shortcuts:** Altium-compatible defaults. See `shortcuts.rs`.

## Versioning

- v0.1.0 — Scaffold (panels, themes, status bar)
- v0.2.0 — Parser (KiCad format read/write)
- v0.3.0 — Canvas (wgpu pan/zoom/grid)
- v0.4.0 — Schematic Viewer
- v0.5.0 — Schematic Editor
- v1.0.0 — Community Release (full schematic + PCB editor)
