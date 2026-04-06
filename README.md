# Signex

**AI-First Electronic Design Automation**

Signex is an open-source desktop EDA tool built for hardware engineers who want Altium Designer-class UX without the Altium price tag. It reads and writes KiCad files natively, so you can use existing KiCad libraries and schematics while enjoying a modern, fast editing experience.

> **Status:** Alpha. Schematic capture is functional. PCB layout, simulation, and AI features are on the roadmap.

## Why Signex?

The EDA landscape has two camps: expensive commercial tools (Altium, OrCAD, PADS) with polished UX, and free tools (KiCad, gEDA) with steeper learning curves. Signex bridges this gap:

- **Altium-class UX** on an open-source foundation
- **KiCad file compatibility** (.kicad_sch, .kicad_sym, KiCad 8/9/10 supported)
- **Native desktop app** via Tauri v2 (Rust backend, React frontend) -- fast startup, low memory
- **AI copilot (Signal)** -- Claude-powered design assistant (coming soon)
- **Built by hardware engineers** for hardware engineers

We started Signex because we design Edge AI hardware at [Alp Lab](https://alplab.ai) and wanted a tool that combined KiCad's openness with Altium's workflow speed. We're vibe coding it with Claude Code.

## Features

### Working Now
- Schematic capture with Canvas2D rendering
- KiCad format read/write (.kicad_sch, .kicad_sym) including KiCad 10
- 226 KiCad symbol libraries with search and preview
- Wire drawing (Manhattan, diagonal, free routing modes)
- Bus drawing and bus entry support
- Component placement with auto-designator generation
- Net labels, power ports, hierarchical ports, no-connect markers
- Selection (click, shift-click, box select with crossing/inside modes)
- Move with rubber-banding, copy/cut/paste, duplicate
- Rotate, mirror, align (6 directions), distribute
- Undo/redo (50 levels)
- In-place text editing (double-click or F2)
- Find/Replace, Find Similar Objects (Shift+F)
- Right-click context menu (context-aware)
- Wire endpoint dragging and break wire
- Drawing objects (line, rectangle, circle, arc, polyline)
- Net connectivity resolution (union-find algorithm)
- ERC with 8 violation types
- Auto-annotation (Design > Annotate)
- BOM generation (CSV export)
- Netlist export (KiCad format)
- PNG export
- Net Color Override (F5)
- Measure distance tool (Ctrl+M)
- Altium-style Properties panel for all object types
- Altium-style selection highlights with corner handles
- Dark theme (Catppuccin Mocha-inspired)
- Unit conversion (mm/mil/inch) everywhere
- Preferences dialog
- 30+ keyboard shortcuts matching Altium conventions

### Roadmap

| Phase | Status | Features |
|-------|--------|----------|
| Phase 0: Viewer | Done | KiCad parser, Canvas2D renderer, symbol transforms, multi-sheet nav |
| Phase 1: Editor | Done | Selection, move, wire, delete, rotate, undo/redo, save, properties |
| Phase 2: Core | Done | Drag-box select, auto-junction, rubber-band, copy/paste, net labels, power ports, ERC, BOM |
| Phase 3: Validation | Next | Full ERC (50+ rules), annotation tools, cross-reference, output jobs |
| Phase 4: Advanced | Planned | Library editor, drawing tools, BOM/PDF/ODB++ export, template system |
| Phase 5: Signal AI | Planned | Claude API integration, design review, component suggestion, ERC fix |
| Phase 6: PCB Layout | Planned | Layer stack, interactive routing, DRC, copper pour, 3D viewer |
| Phase 7: Simulation | Planned | SPICE integration, signal integrity, power analysis |
| Phase 8: Manufacturing | Planned | Gerber/drill export, assembly drawings, pick-and-place |
| Phase 9: Collaboration | Planned | Real-time multi-user editing, version control, cloud storage |
| Phase 10: Marketplace | Planned | Community libraries, design templates, plugin system |

See [docs/master-plan.md](docs/master-plan.md) for the full 10-phase roadmap.

## Installation

### Prerequisites

- [Node.js](https://nodejs.org/) 18+ (LTS recommended)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Platform-specific dependencies:
  - **Windows:** Visual Studio Build Tools 2022 with C++ workload
  - **macOS:** Xcode Command Line Tools (`xcode-select --install`)
  - **Linux:** `sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev`

### Build from Source

```bash
# Clone the repository
git clone https://github.com/alpCaner/signex.git
cd signex

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

### Quick Start

1. Launch Signex
2. File > Open Project (Ctrl+O) -- select a `.kicad_pro` file
3. Click a schematic sheet in the Projects panel to open it
4. Start editing: W=wire, L=label, T=text, B=bus, Space=rotate, Del=delete

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| W | Draw wire |
| B | Draw bus |
| L | Place net label |
| T | Place text |
| Space | Rotate |
| X / Y | Flip horizontal / vertical |
| Ctrl+C/X/V | Copy / Cut / Paste |
| Ctrl+D | Duplicate |
| Ctrl+Z/Y | Undo / Redo |
| Ctrl+A | Select all |
| Ctrl+F/H | Find / Replace |
| Ctrl+M | Measure distance |
| Ctrl+Q | Toggle mm/mil/inch |
| Shift+F | Find similar objects |
| F2 | Edit text in-place |
| F5 | Net color override |
| G | Cycle grid size |
| Del | Delete selected |
| Esc | Cancel / deselect |
| Home | Fit to view |
| Tab | Open properties panel |
| Right-click | Context menu / finish wire |

## Tech Stack

| Layer | Technology |
|-------|------------|
| Desktop shell | [Tauri v2](https://tauri.app/) (Rust) |
| Frontend | React 19 + TypeScript + Vite 7 |
| Styling | Tailwind CSS 4 |
| Canvas | Canvas2D (wgpu planned for PCB) |
| State | Zustand |
| Parser | Pure Rust S-expression parser (no KiCad C++ dependency) |
| AI (planned) | Claude API via Rust reqwest |
| Testing | Vitest (53 tests) + cargo test (11 tests) |

## Project Structure

```
src-tauri/src/          Rust backend
  commands/             Tauri IPC commands (project, schematic, save, library, export)
  engine/               KiCad S-expr parser, document model, writer
src/                    React frontend
  canvas/               SchematicRenderer (Canvas2D), hitTest
  components/           MenuBar, ToolbarStrip, StatusBar, ContextMenu, FindReplace
  panels/               Properties, Components, Messages, Project, Signal
  stores/               Zustand state: layout, project, editor, schematic
  lib/                  Net resolver, ERC, geometry utilities
  __tests__/            Vitest test suite
docs/                   Feature roadmap, master plan, Altium reference
```

## Contributing

We welcome contributions from hardware engineers, EDA developers, and vibe coders!

### How to Contribute

1. **Fork** the repository
2. **Create a branch** for your feature (`git checkout -b feature/my-feature`)
3. **Make your changes** and add tests
4. **Run tests** (`npm run test` and `cd src-tauri && cargo test`)
5. **Submit a PR** with a clear description

### Good First Issues

- Add more ERC violation types (see `src/lib/erc.ts`)
- Improve Properties panel for specific object types
- Add new drawing tools (ellipse, bezier, polygon)
- Improve keyboard shortcut coverage
- Write more tests
- PDF export support

### Development

```bash
# Run tests
npm run test                    # 53 TypeScript tests
cd src-tauri && cargo test      # 11 Rust tests

# Type check
npx tsc --noEmit

# Dev mode with hot reload
npm run tauri dev
```

### Architecture Notes

- **Canvas2D** for schematic rendering -- pure JavaScript, no WebGL dependency
- **Zustand stores** for state -- `schematic.ts` is the main store (~1200 lines)
- **Pure Rust parser** reads KiCad S-expression format without C++ FFI
- **structuredClone** for undo snapshots -- simple and reliable
- **Wire cursor uses refs** (not Zustand) to avoid 60Hz state churn during drawing

## License

**GPL-3.0** -- same as KiCad, since we build on KiCad's file format and libraries.

See [LICENSE](LICENSE) for the full text.

## Disclaimer

- Signex is in **alpha**. It may have bugs that affect your schematic files. **Always keep backups.**
- Signex is **not affiliated with** Altium, KiCad, or any other EDA vendor. "Altium" is a trademark of Altium Limited. "KiCad" is a trademark of the KiCad project.
- The KiCad file format compatibility is provided on a best-effort basis. Not all KiCad features are supported yet.
- The AI features (Signal) are not yet implemented. When available, they will require a Claude API key.
- This software is provided "as is", without warranty of any kind, express or implied. Use at your own risk.
- Always verify your designs with the manufacturer's tools before sending to production.

## Credits

Built by [Caner Alp](https://github.com/alpCaner) at [Alp Lab AB](https://alplab.ai) with [Claude Code](https://claude.ai/code).

---

*Signex: Signal + Nexus. Connecting hardware engineers to better design tools.*
