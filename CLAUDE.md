# Signex Iced — AI-first EDA

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

- **Never add Claude as a commit author, co-author, or contributor.** No `Co-Authored-By` lines. No mentions in CONTRIBUTORS files. Claude is a tool, not a contributor.
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
- v0.6.0 — Full Schematic Editor (drag-move, properties editing, placement tools, iced_aw, Active Bar) 🔄 PR #5 open
- v0.7.0 — Validation & ERC (11 violation types, annotation, pin connection matrix)
- v0.8.0 — Output Generation (PDF, BOM, netlist, library editor)

### PCB Phase
- v0.9.0 — PCB Viewer (Shader widget, layer stack, ratsnest)
- v0.10.0 — PCB Editor (interactive routing, DRC, copper pour)
- v0.11.0 — PCB Output (Gerber, drill, ODB++, STEP)

### Release
- v1.0.0 — Community Release (full schematic + PCB editor)

### Post-v1.0 (parallel tracks)
- v1.1 — 3D viewer
- v1.2 — Advanced schematic (variants, multi-channel)
- v1.3 — Advanced PCB (impedance, constraints)
- v1.4 — SPICE simulation
- v1.7 — Signal AI (Pro)
- v1.8 — Plugin system
- v2.0 — Pro release
- v2.1 — Live collaboration (Pro)

## Skills

- `.claude/skills/iced-guide/` — Comprehensive Iced 0.14 + iced_aw reference (11 files)
- `.claude/skills/kicad-render/` — KiCad rendering pipeline reference
- `.claude/skills/kicad-sexpr/` — KiCad S-expression format reference
