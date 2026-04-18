# Signex — AI-first EDA

## Project

Signex is an AI-first electronics design automation tool targeting Altium Designer feature parity.
Built with **Iced 0.14** (Elm architecture) + **iced_aw 0.13** (additional widgets) + **wgpu** for GPU-accelerated rendering.

## Architecture

- `crates/signex-app/` — Main binary. Iced Application, panels, dock system, menus, Active Bar, canvas.
- `crates/signex-types/` — Domain types. Schematic, PCB, net, layer, theme. NO rendering deps.
- `crates/kicad-parser/` — S-expression parser for .kicad_sch, .kicad_pcb, .kicad_sym files.
- `crates/kicad-writer/` — S-expression serializer (write KiCad format back).
- `crates/signex-render/` — Rendering primitives. Bridges types → Iced Canvas draw calls. Hit-testing.
- `crates/signex-widgets/` — Custom widgets (TreeView, symbol preview, theme extensions).

## Build

```bash
cargo build --workspace          # Debug build
cargo build --workspace --release # Release build
cargo test --workspace           # All tests
cargo clippy --workspace -- -D warnings  # Lint
```

## Rules

- **Never add AI as a commit author, co-author, or contributor.** No `Co-Authored-By` lines. No mentions in CONTRIBUTORS files.
- **Never push directly to `main`.** All work goes through `dev` via feature branches and PRs.
- **Only replicate what Altium Designer shows.** Never invent UI sections or features that don't exist in Altium.
- **Altium Dark theme is the default.** White chevrons, no blue/purple tints, neutral gray chrome.

## Git Workflow

```
main                    # Protected. Stable releases only. Tagged vX.Y.Z.
└── dev                 # Default branch. All feature branches merge here via PR.
    ├── feature/...     # New features: feature/v0.6-full-editor
    ├── fix/...         # Bug fixes: fix/parser-unicode
    └── hotfix/...      # Urgent fixes branched from main, merged to both main and dev
```

- `main` is protected — requires PR with 1 approval, no direct pushes, no force pushes.
- `dev` is the integration branch. Feature/fix branches merge here via PR.
- Feature branches: `feature/<description>` — create from dev, PR back to dev.
- Each version gets its own branch from dev when work starts. No placeholder branches.
- Every merge to main gets a version tag (e.g., `v0.6.0`).

## Conventions

- **Coordinate system:** i64 nanometers internally. KiCad uses mm floats — convert at parse/write boundary.
- **Types crate has ZERO rendering deps.** All rendering goes through signex-render.
- **Iced patterns:** Elm architecture — Message enum, update(), view(). No interior mutability.
- **iced_aw 0.13:** MenuBar for menus, NumberInput for numeric fields. Available: Tabs, Card, ContextMenu, ColorPicker, DropDown, SelectionList, Spinner, Sidebar, Badge, Wrap.
- **Theme:** 6 built-in themes (Catppuccin Mocha, VS Code Dark, Altium Dark, GitHub Dark, Solarized Light, Nord). Altium Dark is default.
- **Canvas:** Use `iced::widget::Canvas` for schematic (CPU tessellation, 3-layer cache: bg/content/overlay).
  Use `iced::widget::Shader` for PCB (100K+ elements need GPU instanced rendering).
- **Panel docking:** Custom DockArea with 3 regions (left/right/bottom) + floating panels. Tabs with collapse/undock.
- **Active Bar:** 14-button Altium-style floating toolbar on canvas via Stack overlay. SVG icons with LazyLock handles.
- **Keyboard shortcuts:** Altium-compatible defaults. W=Wire, B=Bus, L=Label, P=Component, Space=Rotate, etc.
- **Styles:** Reusable style helpers in `styles.rs` — `dock_tab()`, `rail_tab()`, `menu_item()`, `floating_title_bar()`, etc.

## Versioning

### Schematic Phase
- v0.1.0 — Scaffold (panels, themes, status bar) ✅
- v0.2.0 — Parser (KiCad format read/write) ✅
- v0.3.0 — Canvas (wgpu pan/zoom/grid) ✅
- v0.4.0 — Schematic Viewer ✅
- v0.5.0 — Schematic Editor (selection, wire drawing, undo/redo) ✅
- v0.6.0 — Full Schematic Editor (drag-move, properties editing, placement tools, iced_aw, Active Bar) 🔄
- v0.7.0 — Validation & ERC (11 violation types, annotation, pin connection matrix)
- v0.8.0 — Output Generation (PDF, BOM, netlist)
- v0.9.0 — Library & Polish (symbol/footprint editor, installers)
- v1.0.0 — Community Preview (schematic-only early access)

### Schematic Polish
- v1.1.0 — Advanced Schematic (variants, multi-channel, harness, parameter manager)
- v1.2.0 — SCH Tables & Docs (tables, ToC, drawing tools)
- v1.3.0 — Enhanced Output (smart PDF, BOM studio, output jobs)
- v1.4.0 — Design Notebook (Typst editor, component-linked annotations, measurements)
- v1.5.0 — Block Diagram (system-level blocks, signal flow, power tree)

### PCB Phase
- v2.0.0 — PCB Viewer (Shader widget, layer stack, ratsnest, cross-probe)
- v2.1.0 — Router Stage 1: Greedy single-trace (corners, vias, net-class widths, live DRC)
- v2.1.1 — Router Stage 2: Walkaround (A* + obstacle graph, incremental DRC)
- v2.1.2 — Router Stage 3: Push-and-shove (topology-preserving convergence)
- v2.1.3 — Router Stage 4: Diff pair + length tuning (accordion/trombone/sawtooth)
- v2.1.4 — Copper pour (zone fill, thermal relief, island removal)
- v2.2.0 — PCB Output (Gerber, drill, ODB++, STEP) — Community Release

See `docs/internal/docs/PCB_ROUTER_PLAN.md` for the detailed router plan.

### Post-v2.2 (see MASTER_PLAN.md for full detail)
- v2.3–v2.5 — 3D viewer, advanced PCB, high-speed design
- v3.0–v3.4 — Pro Release (Signal AI, plugins, collaboration)
- v4.0–v4.4 — Simulation, advanced output, import
- v5.0–v5.4 — Signex 365 (cloud PLM, BOM Studio, ERP bridge)

## CI/CD

- `.github/workflows/ci.yml` — check, clippy, test, fmt on push/PR to dev and main
- `.github/workflows/release.yml` — triggered by version tags (v*), builds release binaries for Windows/Linux/macOS, creates GitHub Release with checksums

## Skills

- `.claude/skills/iced-guide/` — Comprehensive Iced 0.14 + iced_aw reference (11 files)
- `.claude/skills/kicad-render/` — KiCad rendering pipeline reference
- `.claude/skills/kicad-sexpr/` — KiCad S-expression format reference
- `.claude/skills/rust-book-skill/` — Complete Rust language reference
- `.claude/skills/iced-rust/` — Iced Rust API patterns and EDA conventions
- `.claude/skills/wgpu-rust/` — wgpu GPU programming reference
