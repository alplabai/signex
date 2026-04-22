<p align="center">
  <img src="assets/screenshots/hero.png" alt="Signex — schematic editor" width="900">
</p>

<h1 align="center">Signex</h1>
<p align="center">
  Open-source, AI-first electronics design automation
</p>

<p align="center">
  <a href="https://github.com/alplabai/signex/blob/dev/LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License"></a>
  <a href="https://github.com/alplabai/signex/releases/tag/v0.7.0"><img src="https://img.shields.io/badge/version-v0.7.0-green.svg" alt="Version"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.80%2B-orange.svg" alt="Rust"></a>
  <a href="https://github.com/alplabai/signex/wiki"><img src="https://img.shields.io/badge/wiki-user%20guide-blueviolet.svg" alt="Wiki"></a>
  <a href="https://github.com/alplabai/signex/discussions"><img src="https://img.shields.io/badge/discussions-join-brightgreen.svg" alt="Discussions"></a>
</p>

<p align="center">
  <a href="#features">Features</a> &middot;
  <a href="#screenshots">Screenshots</a> &middot;
  <a href="#building">Building</a> &middot;
  <a href="#roadmap">Roadmap</a> &middot;
  <a href="https://github.com/alplabai/signex/wiki">Wiki</a> &middot;
  <a href="#contributing">Contributing</a> &middot;
  <a href="#license">License</a>
</p>

---

Signex is a **KiCad-compatible** schematic and PCB editor built in Rust with
GPU-accelerated rendering. It opens your existing KiCad projects, edits them
through an Altium Designer-quality UI, and saves them back — so KiCad users
get a better editor without leaving the ecosystem they trust.

**Two editions from one codebase:**

- **Signex Community** (Apache-2.0, free forever) — full schematic + PCB
  editor, 3D viewer, simulation, plugin system
- **Signex Pro** (subscription) — adds Signal AI (Claude-powered design
  copilot), real-time collaboration, and Signex 365 cloud PLM

> **Status:** Early development — v0.7.0 shipped; next up v0.8 (PDF / BOM / netlist output)
> in progress. [Join the discussion](https://github.com/alplabai/signex/discussions)
> or check the [roadmap](#roadmap).

## Features

**What works today (v0.1–v0.7):**

- Open and render any KiCad schematic (.kicad_sch, .kicad_sym, .kicad_pro)
- Full schematic editing: select, move, wire (W), bus (B), label (L),
  component placement (P), delete, rotate (Space), mirror (X/Y)
- Advanced shape tools — Line, Rectangle, Circle, Arc (3-click), Polygon
  (click-by-click), editable drawing Properties with live preview
- Copy/paste, undo/redo (100 levels), save back to KiCad format
- 6 built-in themes with customizable theme editor
- Altium-style docking panels with drag-to-undock/dock
- Active Bar — 14-button floating toolbar with dropdown menus
- Context menu, in-place text editing (F2), selection filter
- Properties panel with context-aware field editing, Parameter Manager
- **Multi-window editing (v0.7)** — undock any tab into its own OS window
  and edit it independently; each window keeps its own pan/zoom, selection,
  and undo/redo history
- **ERC validation (v0.7)** — 11 Altium-style rules including cross-sheet
  hierarchy checks and net-label conflicts; Messages panel with click-to-zoom
- **Annotation (v0.7)** — four modes, review-and-confirm dialog,
  lock/unlock per designator, project-wide consistency
- F5 net-color palette, F8 ERC, F9 AutoFocus (dim unrelated objects)
- Pin connection matrix (12×12, per-cell severity override)
- Lasso + Inside/Outside/TouchingLine selection modes (Shift+S to cycle)
- KiCad 8/9 format support with round-trip fidelity
- 60fps pan/zoom on schematics with 500+ components

**What's next:**

| Version | Milestone |
|---|---|
| v0.8–v0.9 | PDF/BOM output, library editor, installers |
| **v1.0** | **Community Preview** — schematic-only release |
| **v2.0–v2.2** | **Community Release** — full PCB editor |
| **v3.0** | **Pro Release** — Signal AI + collaboration |
| **v4.0** | Unified simulation view with SPICE, EM, thermal |
| **v5.0** | Signex 365 cloud PLM |

## Screenshots

<p align="center">
  <img src="assets/screenshots/editor.png" alt="Active Bar with power port dropdown and Properties panel" width="800">
  <br>
  <em>Active Bar with dropdown menus, Selection Filter tags, Properties panel with document options</em>
</p>

<details>
<summary><strong>More themes</strong></summary>

<p align="center">
  <img src="assets/screenshots/cappuchine-theme.png" alt="Catppuccin Mocha theme" width="400">
  <img src="assets/screenshots/gtihubdark-theme.png" alt="GitHub Dark theme" width="400">
  <br>
  <img src="assets/screenshots/solarized-theme.png" alt="Solarized Light theme" width="400">
  <br>
  <em>Catppuccin Mocha, GitHub Dark, Solarized Light — 6 themes built in, fully customizable</em>
</p>

</details>

## Architecture

```
signex/
├── crates/
│   ├── signex-app/       # Main binary — Iced 0.14 application
│   ├── signex-types/     # Domain types — NO rendering deps
│   ├── signex-render/    # wgpu rendering (types → Canvas draw calls)
│   ├── signex-widgets/   # Reusable Iced widgets (tree view, icon button)
│   ├── kicad-parser/     # S-expression parser (.kicad_sch/.kicad_pcb/.kicad_sym)
│   └── kicad-writer/     # S-expression serializer (write KiCad format)
└── Cargo.toml
```

**Design principles:**

- **KiCad compatibility first.** Open existing KiCad projects, save back losslessly. No proprietary format.
- **Elm architecture.** Iced's `Message -> update -> view` cycle. No interior mutability.
- **Multi-window by default.** Built on `iced::daemon`; every undocked tab gets its own engine + canvas keyed by window id, so two schematics can be edited in parallel without cross-talk.
- **Nanometer coordinates.** `i64` nanometers internally; exact in both metric and imperial.
- **Canvas for schematic, Shader for PCB.** CPU tessellation for schematics, GPU instanced rendering for 100K+ PCB elements.
- **Types crate has zero rendering deps.** Clean separation between domain and display.

## Building

**Prerequisites:** Rust 1.80+ and a GPU supporting Vulkan, Metal, or DX12.

```bash
git clone https://github.com/alplabai/signex.git
cd signex
cargo run -p signex-app          # Run
cargo test --workspace           # Test
cargo clippy --workspace -- -D warnings  # Lint
```

## Roadmap

| Milestone | Version | Status |
|---|---|---|
| Scaffold — Iced shell, panels, themes, dock system | v0.1 | Done |
| Parser — KiCad format read/write, domain types | v0.2 | Done |
| Canvas — wgpu pan/zoom/grid, Altium-style camera | v0.3 | Done |
| Schematic Viewer — render all elements, multi-sheet nav | v0.4 | Done |
| Schematic Editor — select, move, wire, undo/redo, save | v0.5 | Done |
| Full SCH Editor — copy/paste, labels, components, Active Bar | v0.6 | Done |
| Validation + Multi-Window — ERC, annotation, pin matrix, undockable tabs | v0.7 | Done |
| Output — PDF, BOM, netlist | v0.8 | **In Progress** |
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

## Documentation

The **[Signex Wiki](https://github.com/alplabai/signex/wiki)** is the user
guide — installation, quick start, keyboard shortcuts, feature-by-feature
walkthroughs for every v0.1–v0.7 capability (ERC, annotation, multi-window
editing, hierarchical sheets, net-color pen, themes, and more), plus an FAQ
and roadmap.

Start with **[Quick Start](https://github.com/alplabai/signex/wiki/Quick-Start)**
to open your first KiCad project, or jump straight to
**[Keyboard Shortcuts](https://github.com/alplabai/signex/wiki/Keyboard-Shortcuts)**
for the full Altium-compatible reference.

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

Signex Community Edition is licensed under the [Apache License 2.0](LICENSE).

Copyright 2026 [Alp Lab AI](https://github.com/alplabai)
