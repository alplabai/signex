<p align="center">
  <h1 align="center">Signex</h1>
  <p align="center">AI-first electronics design automation</p>
</p>

<p align="center">
  <a href="#features">Features</a> &middot;
  <a href="#architecture">Architecture</a> &middot;
  <a href="#building">Building</a> &middot;
  <a href="#roadmap">Roadmap</a> &middot;
  <a href="#contributing">Contributing</a> &middot;
  <a href="#license">License</a>
</p>

---

Signex is a native Rust EDA tool targeting **Altium Designer feature parity** with an AI-first design philosophy. Built on [Iced](https://iced.rs) 0.14 (Elm architecture) with [wgpu](https://wgpu.rs) for GPU-accelerated rendering.

**Two editions from one codebase:**
- **Signex Community** (Apache-2.0, free) — full schematic + PCB editor, 3D viewer, simulation, plugins
- **Signex Pro** (subscription) — adds Signal AI (Claude-powered design assistant) + live collaboration

> **Status:** Early development. See [Roadmap](#roadmap) for current progress.

## Features

### v0.1–v0.3 — Foundation

- Workspace with 6 Rust crates (`signex-app`, `signex-types`, `signex-render`, `signex-widgets`, `kicad-parser`, `kicad-writer`)
- KiCad file format support (.kicad_sch, .kicad_pcb, .kicad_sym) — parse and write
- Native Signex file formats (.snxsch, .snxpcb, .snxsym, .snxprj)
- Domain type system: schematic, PCB, net, layer, coordinate (nanometer precision)
- 6 built-in themes: Catppuccin Mocha, VS Code Dark, Altium Dark, GitHub Dark, Solarized Light, Nord
- Rich text markup parser (subscript, superscript, overbar)
- Iced application shell with docking panel system
- Altium-compatible keyboard shortcut framework
- wgpu canvas with pan/zoom/grid, camera system

### v0.4 — Schematic Viewer

- Click-to-select all element types (symbols, wires, buses, labels, junctions, sheets, text)
- Altium-style selection overlay (cyan highlight + corner grips)
- Properties panel shows selected element details (type, reference, value, position, rotation)
- Fit-to-content on file load (auto-zoom to schematic bounds)
- Components panel: KiCad 9.0 library browser (226 libs), split list/details, symbol preview canvas

### v0.5 — Schematic Editor

- Undo/redo system (command pattern, 100-step history)
- Wire drawing tool (click to place segments, grid snap)
- Bus drawing tool + label placement tool
- Delete/rotate/mirror selected elements (Del, R, X, Y keys)
- Dirty flag on tabs for modified documents

### v0.6 — Full Editor (current)

- Multi-select with Ctrl+click and Ctrl+A (select all)
- Copy/paste with Ctrl+C/Ctrl+V (offset by 2 grid units, new UUIDs)
- Save to .kicad_sch with Ctrl+S, Save As with .snxsch primary
- Altium-style tree view with SVG chevrons, persistent collapse, clickable file tabs
- 8-menu dropdown menu bar with Stack overlay
- Dock system: flat tabs with accent underline, drag-to-resize panels
- Multi-tab schematic document support with dirty-tab close protection
- Deterministic file output (sorted HashMap keys in writer)
- Path traversal protection for project file references
- 78 clippy errors resolved, full CI green (clippy -D warnings + fmt + tests)

### Next — v0.7 (Validation)

- ERC (Electrical Rules Check)
- DRC (Design Rules Check)
- Annotation (auto-designator numbering)

### Planned
- Output generation — PDF, BOM, netlist, library editor (v0.8)
- PCB rendering and interactive routing (v0.9–v0.10)
- Manufacturing output: Gerber, ODB++, STEP (v0.11)
- 3D PCB viewer with PBR materials (v1.1)
- SPICE + EM + thermal simulation (v1.4–v1.5)
- Signal AI — Claude-powered design review (v1.7, Pro)
- Live collaboration via Supabase (v2.1, Pro)

## Architecture

```
signex/
├── crates/
│   ├── signex-app/       # Main binary — Iced 0.14 application
│   ├── signex-types/     # Domain types — NO rendering deps
│   ├── signex-render/    # wgpu rendering (types → Canvas draw calls)
│   ├── signex-widgets/   # Reusable Iced widgets (tree view, icon button, status bar)
│   ├── kicad-parser/     # S-expression parser (.kicad_sch/.kicad_pcb/.kicad_sym)
│   └── kicad-writer/     # S-expression serializer (write KiCad format)
├── Cargo.toml            # Workspace manifest
└── CLAUDE.md             # AI agent instructions
```

### Design principles

- **Types crate has zero rendering deps.** All rendering goes through `signex-render`.
- **Elm architecture.** Iced's `Message → update → view` cycle. No interior mutability.
- **Nanometer coordinates.** `i64` nanometers internally; convert at the parse/write boundary.
- **KiCad compatibility first.** Open existing KiCad projects, save back losslessly.
- **Canvas for schematic, Shader for PCB.** CPU tessellation is fine for schematics (<10K elements). PCB needs GPU instanced rendering (100K+ tracks/pads).

## Building

### Prerequisites

- Rust 1.80+ (edition 2024)
- A GPU that supports Vulkan, Metal, or DX12 (for wgpu)

### Build

```bash
# Debug build
cargo build --workspace

# Release build
cargo build --workspace --release

# Run
cargo run -p signex-app

# Tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings
```

## Roadmap

| Milestone | Version | Status |
|---|---|---|
| Scaffold — Iced shell, panels, themes, dock system | v0.1 | Done |
| Parser — KiCad format read/write, domain types | v0.2 | Done |
| Canvas — wgpu pan/zoom/grid, Altium-style camera | v0.3 | Done |
| Schematic Viewer — render all elements, multi-sheet nav | v0.4 | Done |
| Schematic Editor — select, move, wire, undo/redo, save | v0.5 | Done |
| Full SCH Editor — copy/paste, labels, components, Active Bar | v0.6 | In Progress |
| Validation — ERC, annotation, pin matrix | v0.7 | |
| Output — PDF, BOM, netlist | v0.8 | |
| Library & Polish — symbol/footprint editor, installers | v0.9 | |
| **Community Preview** — schematic-only editor | **v1.0** | |
| PCB Viewer — GPU rendering, layers, cross-probe | v2.0 | |
| PCB Routing + DRC + Output | v2.1–v2.2 | |
| **Community Release** — full schematic + PCB editor | **v2.2** | |
| 3D Viewer, Advanced PCB, High-Speed Design | v2.3–v2.5 | |
| **Pro Release** — Signal AI + plugins + collaboration | **v3.0** | |
| Simulation — SPICE, EM, thermal, simulation wizards | v4.0–v4.1 | |
| **Signex 365** — cloud PLM, BOM Studio, ERP bridge | **v5.0** | |

See [docs/ROADMAP.md](docs/ROADMAP.md) for the detailed version plan.

## Contributing

Signex is open source and we welcome contributions from everyone — whether
you're an EDA professional, a Rust developer, a KiCad user, or someone who
just wants to help build a better design tool.

**Ways to contribute:**

- Report bugs or rendering discrepancies vs KiCad
- Add KiCad test fixtures from real projects
- Implement a feature from the [roadmap](docs/ROADMAP.md)
- Fix an [open issue](https://github.com/alplabai/signex/issues)
- Improve documentation

**Quick start:**

```bash
git clone https://github.com/alplabai/signex.git
cd signex
cargo build --workspace
cargo run -p signex-app
```

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full guide: branching
workflow, crate map, code style, and good first issues.

## License

Signex Community Edition is licensed under [Apache License 2.0](LICENSE).

Copyright (C) 2026 [Alp Lab AI](https://github.com/alplabai)
