<p align="center">
  <h1 align="center">Signex</h1>
  <p align="center"><strong>AI-First Electronic Design Automation</strong></p>
  <p align="center">Altium-class UX. KiCad compatibility. Open source.</p>
</p>

<p align="center">
  <a href="https://github.com/alplabai/signex/blob/main/LICENSE"><img src="https://img.shields.io/github/license/alplabai/signex?color=blue&label=License" alt="License"></a>
  <a href="https://github.com/alplabai/signex/releases"><img src="https://img.shields.io/github/v/release/alplabai/signex?include_prereleases&label=Release&color=orange" alt="Release"></a>
  <a href="https://github.com/alplabai/signex/stargazers"><img src="https://img.shields.io/github/stars/alplabai/signex?style=flat&color=yellow" alt="Stars"></a>
  <a href="https://github.com/alplabai/signex/issues"><img src="https://img.shields.io/github/issues/alplabai/signex?color=green" alt="Issues"></a>
  <a href="https://github.com/alplabai/signex/pulls"><img src="https://img.shields.io/github/issues-pr/alplabai/signex?color=purple" alt="PRs"></a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-v2-blue?logo=tauri&logoColor=white" alt="Tauri v2">
  <img src="https://img.shields.io/badge/React-19-61dafb?logo=react&logoColor=white" alt="React 19">
  <img src="https://img.shields.io/badge/Rust-stable-orange?logo=rust&logoColor=white" alt="Rust">
  <img src="https://img.shields.io/badge/TypeScript-5.8-3178c6?logo=typescript&logoColor=white" alt="TypeScript">
  <img src="https://img.shields.io/badge/KiCad-8%2F9%2F10-blue?logo=data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAyNCAyNCI+PHBhdGggZD0iTTEyIDJMMyAxNGgxOEwxMiAyeiIgZmlsbD0id2hpdGUiLz48L3N2Zz4=" alt="KiCad">
  <img src="https://img.shields.io/badge/Tests-64_passing_(53_TS_%2B_11_Rust)-brightgreen" alt="Tests">
  <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey" alt="Platform">
</p>

---

Signex is an open-source desktop EDA tool built for hardware engineers who want Altium Designer-class UX without the Altium price tag. It reads and writes KiCad files natively, so you can use existing KiCad libraries and schematics while enjoying a modern, fast editing experience.

> **Status:** Alpha. Schematic capture is functional. PCB layout, simulation, and AI features are on the roadmap.

<!-- TODO: Add screenshot here when UI is polished -->

## Why Signex?

The EDA landscape has two camps: expensive commercial tools (Altium, OrCAD, PADS) with polished UX, and free tools (KiCad, gEDA) with steeper learning curves. Signex bridges this gap:

- **Altium-class UX** on an open-source foundation
- **KiCad file compatibility** (.kicad_sch, .kicad_sym, KiCad 8/9/10 supported)
- **Native desktop app** via Tauri v2 (Rust backend, React frontend) -- fast startup, low memory
- **AI copilot (Signal)** -- Claude-powered design assistant (coming soon)
- **Built by hardware engineers** for hardware engineers

We started Signex because we design Edge AI hardware at [Alp Lab](https://alplab.ai) and wanted a tool that combined KiCad's openness with Altium's workflow speed.

## Features

<details>
<summary><strong>Schematic Capture</strong> (click to expand)</summary>

- Canvas2D rendering with pan/zoom
- 226 KiCad symbol libraries with search and preview
- Wire drawing (Manhattan, diagonal, free routing modes)
- Bus drawing and bus entry support
- Component placement with auto-designator generation
- Net labels, power ports, hierarchical ports, no-connect markers
- Sheet symbols with pin support
- Drawing objects (line, rectangle, circle, arc, polyline)
- Text string placement
</details>

<details>
<summary><strong>Editing</strong></summary>

- Selection (click, shift-click, box select with crossing/inside modes)
- Move with rubber-banding, copy/cut/paste, duplicate
- Rotate, mirror, align (6 directions), distribute
- Undo/redo (50 levels)
- In-place text editing (double-click or F2)
- Wire endpoint dragging and break wire
- Find/Replace, Find Similar Objects (Shift+F)
- Right-click context menu (context-aware)
- Z-ordering (Bring to Front / Send to Back)
- Batch property editing for multi-select same-type objects
- Alt+Click to select entire net
</details>

<details>
<summary><strong>Validation & Output</strong></summary>

- Net connectivity resolution (union-find algorithm)
- ERC with 11 violation types including pin-to-pin connection matrix (12x12)
- No ERC directives to suppress individual violations
- Auto-annotation with lock/unlock designators
- Reset designators / reset duplicates only
- BOM generation (CSV export)
- Netlist export (KiCad format)
- PNG export
- Net Color Override (F5) with 12-color palette
- Measure distance tool (Ctrl+M)
- ERC markers rendered on canvas (red errors, yellow warnings)
- AutoFocus: dim non-related objects when inspecting violations
</details>

<details>
<summary><strong>Panels (Altium-style)</strong></summary>

- **Properties** -- context-aware editing for all object types (Altium layout)
- **Components** -- 226 KiCad libraries with search, preview, and place
- **SCH Filter** -- toggle visibility/selectability per object type
- **SCH List** -- spreadsheet view with sortable columns, click-to-select
- **Navigator** -- schematic overview with object tree and stats
- **Messages** -- ERC violations with click-to-select and Run ERC button
- **Projects** -- project tree with sheet navigation
- All panels tabbed: Left (Projects/Components/Nav), Right (Props/Filter/List), Bottom (Messages)
</details>

<details>
<summary><strong>UI & UX</strong></summary>

- Dark theme (Catppuccin Mocha-inspired)
- Unit conversion (mm/mil/inch) everywhere
- Preferences dialog (General/Display/ERC tabs)
- 30+ keyboard shortcuts matching Altium conventions
- Centered Active Bar with all placement tools
- Altium-style selection highlights with green corner handles
- Right-click context menu with context-aware actions
</details>

## Roadmap

| Phase | Status | Features |
|-------|--------|----------|
| **Phase 0: Viewer** | :white_check_mark: Done | KiCad parser, Canvas2D renderer, symbol transforms, multi-sheet nav |
| **Phase 1: Editor** | :white_check_mark: Done | Selection, move, wire, delete, rotate, undo/redo, save, properties |
| **Phase 2: Core** | :white_check_mark: Done | Drag-box select, auto-junction, rubber-band, copy/paste, net labels, power ports, ERC, BOM |
| **Phase 3: Validation** | :white_check_mark: Done | ERC (11 checks + connection matrix), annotation, No ERC directives, AutoFocus, lock designators |
| **Phase 4: Advanced** | :hourglass: Planned | Library editor, drawing tools, BOM/PDF/ODB++ export, template system |
| **Phase 5: Signal AI** | :hourglass: Planned | Claude API integration, design review, component suggestion, ERC fix |
| **Phase 6: PCB Layout** | :hourglass: Planned | Layer stack, interactive routing, DRC, copper pour, 3D viewer |
| **Phase 7: Simulation** | :hourglass: Planned | SPICE integration, signal integrity, power analysis |
| **Phase 8: Manufacturing** | :hourglass: Planned | Gerber/drill export, assembly drawings, pick-and-place |
| **Phase 9: Collaboration** | :hourglass: Planned | Real-time multi-user editing, version control, cloud storage |
| **Phase 10: Marketplace** | :hourglass: Planned | Community libraries, design templates, plugin system |

See [docs/master-plan.md](docs/master-plan.md) for the full 10-phase roadmap.

## Quick Start

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- **Windows:** Visual Studio Build Tools 2022 (C++ workload)
- **macOS:** `xcode-select --install`
- **Linux:** `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`

### Install & Run

```bash
git clone https://github.com/alplabai/signex.git
cd signex
npm install
npm run tauri dev
```

### First Steps

1. **Open** a KiCad project: File > Open Project (Ctrl+O) -- select a `.kicad_pro` file
2. **Navigate** -- click a schematic sheet in the Projects panel
3. **Edit** -- W=wire, L=label, T=text, B=bus, Space=rotate, Del=delete
4. **Save** -- Ctrl+S writes back to KiCad format

## Keyboard Shortcuts

<details>
<summary>Full shortcut table</summary>

| Shortcut | Action |
|----------|--------|
| `W` | Draw wire |
| `B` | Draw bus |
| `L` | Place net label |
| `T` | Place text |
| `Space` | Rotate |
| `X` / `Y` | Flip horizontal / vertical |
| `Ctrl+C/X/V` | Copy / Cut / Paste |
| `Ctrl+D` | Duplicate |
| `Ctrl+Z/Y` | Undo / Redo |
| `Ctrl+A` | Select all |
| `Ctrl+F/H` | Find / Replace |
| `Ctrl+M` | Measure distance |
| `Ctrl+Q` | Toggle mm/mil/inch |
| `Shift+F` | Find similar objects |
| `F2` | Edit text in-place |
| `F5` | Net color override |
| `G` | Cycle grid size |
| `Del` | Delete selected |
| `Esc` | Cancel / deselect |
| `Home` | Fit to view |
| `Tab` | Open properties panel |

</details>

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop | [Tauri v2](https://tauri.app/) (Rust) |
| Frontend | React 19 + TypeScript + Vite 7 |
| Styling | Tailwind CSS 4 |
| Canvas | Canvas2D (wgpu planned for PCB) |
| State | Zustand |
| Parser | Pure Rust S-expression parser |
| AI | Claude API via Rust reqwest (planned) |
| Testing | Vitest (53 tests) + cargo test (11 tests) = 64 total |

## Project Structure

```
src-tauri/src/
  commands/           Tauri IPC (project, schematic, save, library, export)
  engine/             KiCad S-expr parser + writer
src/
  canvas/             SchematicRenderer (Canvas2D), hitTest
  components/         MenuBar, ToolbarStrip, StatusBar, ContextMenu
  panels/             Properties, Components, Messages, Filter, List, Navigator, Project
  stores/             Zustand: layout, project, editor, schematic
  lib/                Net resolver, ERC (11 checks), connection matrix, geometry
  __tests__/          Vitest test suite (53 tests)
docs/                 Roadmap, master plan, Altium reference
```

## Contributing

We welcome contributions from hardware engineers, EDA developers, and vibe coders! See [CONTRIBUTING.md](CONTRIBUTING.md) for the full guide.

### How to Contribute

1. **Fork** the repo
2. **Branch from `dev`** (`git checkout dev && git checkout -b feature/my-feature`)
3. **Code** and add tests
4. **Test** (`npm run test` + `cd src-tauri && cargo test`)
5. **PR against `dev`** with a clear description

> **Branches:** `main` = stable releases, `dev` = active development. All PRs go to `dev`.

### Good First Issues

| Area | Task |
|------|------|
| ERC | Add more violation types (see `src/lib/erc.ts`) |
| UI | Improve Properties panel for specific object types |
| Drawing | Add ellipse, bezier, polygon tools |
| Export | PDF export support |
| Testing | Increase test coverage |
| Docs | Improve inline documentation |

### Development

```bash
npm run test                    # 53 TypeScript tests
cd src-tauri && cargo test      # 11 Rust tests
npx tsc --noEmit                # Type check
npm run tauri dev               # Dev mode with hot reload
```

## License

[GPL-3.0](LICENSE) -- compatible with KiCad's license.

## Disclaimer

> [!CAUTION]
> Signex is in **alpha**. It may have bugs that affect your schematic files. **Always keep backups.**

- Not affiliated with Altium, KiCad, or any other EDA vendor
- KiCad file format compatibility is best-effort; not all features supported yet
- AI features (Signal) are not yet implemented
- Always verify designs with manufacturer tools before production

## Credits

Built by [Caner Alp](https://github.com/alpCaner) at [Alp Lab AB](https://alplab.ai).

Tools: [Claude Code](https://claude.ai/code), [Tauri](https://tauri.app/), [Vite](https://vite.dev/), [KiCad](https://www.kicad.org/) (file format).

---

<p align="center">
  <strong>Signex</strong>: Signal + Nexus<br>
  <em>Connecting hardware engineers to better design tools.</em>
</p>
