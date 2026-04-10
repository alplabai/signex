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
