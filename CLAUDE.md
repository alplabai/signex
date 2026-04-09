# Signex — AI-First Electronic Design Automation

## Project Overview
Desktop EDA tool with Altium Designer-class UX.
Target: schematic capture, PCB layout, 3D viewer, SI simulation, AI copilot (Signal).

## Architecture — Target Stack
- **Rendering:** Bevy 0.18 (wgpu) — 2D schematic + 2D PCB + 3D PCB viewer
- **UI Panels:** egui 0.34 via bevy_egui 0.39 — toolbars, inspectors, panels, dialogs
- **2D Shapes:** bevy_prototype_lyon 0.16 — schematic symbols, wires, pads, tracks
- **Camera 2D:** bevy_pancam 0.20 — right-click pan, scroll zoom (Altium-style)
- **Camera 3D:** bevy_panorbit_camera 0.34 — orbit for 3D PCB viewer
- **Parser:** Pure Rust S-expression parser for KiCad format (.kicad_sch, .kicad_pcb, .kicad_sym)
- **Simulation:** ngspice 46 (SPICE), OpenEMS 0.0.36 (RF/EM FDTD), Elmer FEM 26.1 (thermal)
- **AI:** Claude API via reqwest — branded "Signal" copilot
- **State:** Bevy ECS — entities + components + systems (replaces Zustand)
- **Picking:** Bevy MeshPickingPlugin (built-in)
- **Plugins:** Extism 1.21 (WASM) — third-party plugin API

## Source Stack (being migrated)
- Tauri v2 + React 19 + TypeScript + Canvas2D + Zustand
- Legacy code moved to `_legacy/` for reference during port

## Workspace Structure (Target)
```
Cargo.toml                       # [workspace] manifest
libs/
  kicad-parser/                  # S-expr parser — .kicad_sch, .kicad_pcb, .kicad_sym
  kicad-writer/                  # S-expr serializer — write back to KiCad format
  eda-types/                     # Domain types — schematic, pcb, net, layer, sim, violation
  pcb-geom/                      # Polygon boolean (Clipper2), copper pour, ratsnest, mesh gen
  spice-gen/                     # SchematicDoc → .cir netlist, .raw parser
  openems-bridge/                # PCB → CSX XML, HDF5 S-param reader
  elmer-bridge/                  # PCB → .sif, GMSH mesh gen, VTK thermal reader
  step-loader/                   # STEP import via truck-modeling
  plugin-api/                    # WASM host functions + types (Extism)
src/                             # Bevy application
  main.rs                        # App::new() + plugin chain
  plugins/                       # Bevy plugins (viewport_2d, viewport_3d, view_mode, sim)
  render/schematic/              # Wire, symbol, pin, label, junction, sheet, text
  render/pcb/                    # Track, pad, via, zone, silkscreen, layer compositing, shaders
  render/mesh_3d/                # PCB extruder, layer stack, PBR materials, STEP, thermal overlay
  systems/                       # ERC, DRC, router, ratsnest, annotation, undo
  ui/schematic/                  # egui panels: properties, components, messages, navigator, signal AI
  ui/pcb/                        # egui panels: layer stack, DRC, net class, inspector
  ui/sim/                        # egui panels: waveform, S-params, thermal, job queue
  state/                         # Bevy Resources: tool_state, selection, theme
  api/                           # WASM plugin host functions + gateway
```

## Commands (Target)
- `cargo run` — Run Signex (debug, with dynamic_linking for fast iteration)
- `cargo build --workspace --release` — Release build
- `cargo test --workspace` — All tests
- `cargo clippy --workspace -- -D warnings` — Lint

## Conventions
- Dark theme (Catppuccin Mocha-inspired palette)
- 13px base font size (dense EDA UI)
- All panels collapsible, layout persisted to localStorage
- Altium-compatible keyboard shortcuts (see docs/altium-schematic-reference.md)
- KiCad file format compatibility (.kicad_sch read/write)
- Native format: .alpsch/.alppcb (future)
- GPL-3.0 license (KiCad derivative)

## Phase Status
- [x] Phase 0: Viewer — KiCad parser, Canvas2D renderer, symbol transforms, multi-sheet nav
- [x] Phase 1: Editor foundation — selection, move, wire, delete, rotate, undo/redo, save, properties
- [x] Phase 1.5: Grid/snap toggle, component library browser (226 KiCad libs), menu wiring
- [ ] Phase 2: Core editing — drag-box select, auto-junction, electrical snap, rubber-band, copy/paste, net labels, power ports, ERC
- [ ] Phase 3: Validation — ERC, annotation, cross-reference
- [ ] Phase 4: Advanced — library editor, drawing objects, BOM, PDF export
- [ ] Phase 5: Signal AI — Claude API integration, design assistance
- [ ] Phase 6: PCB layout — layer stack, routing, DRC, copper pour, 3D viewer

## Architecture Decisions
- **Bevy + bevy_egui**: Bevy for 2D/3D rendering + ECS, egui for UI panels/toolbars via bevy_egui
- **Bevy over pure egui**: proper GPU rendering engine with batching, built-in picking, native 3D, ECS data model
- **Render philosophy**: Read KiCad source (SCH_PAINTER, PCB_PAINTER) for render logic. UX follows Altium. Do NOT reference Canvas2D Signex renderer
- **Workspace split**: 10 library crates (no Bevy dep) + 1 Bevy app crate
- **Pure Rust parser**: no KiCad C++ FFI — simpler build, no C++ toolchain dependency
- **ECS entity-per-element**: each wire/symbol/junction/label is a Bevy Entity with typed Components
- **Command-based undo**: not full-state snapshots — better for ECS architecture
- **bevy_prototype_lyon**: static 2D shapes; Bevy Gizmos for dynamic overlays (grid, selection, cursor)
- **bevy_pancam**: right-click pan matches Altium UX
- **Simulation**: ngspice (SPICE), OpenEMS (RF/EM FDTD), Elmer FEM (thermal) — all subprocess-based, AsyncComputeTaskPool, graceful fallback if not installed
- **Plugin system**: Extism WASM — 5 host function categories (Document, Mutation, UI, Query, Sim)

---

## Bevy + egui Migration Plan

**Source:** Tauri v2 + React 19 + TypeScript + Canvas2D + Zustand
**Target:** Bevy 0.18 + bevy_egui 0.39 + wgpu (native) + Rust workspace
**Reference:** `docs/gui-framework-comparison.md`, Architecture.md agent dependency tree

### Agent Dependency Tree
```
Agent 0  (Workspace Architect)
├── Agent 1  (KiCad Parser/Writer — port existing Rust)
│   └── Agent 2  (EDA Types — domain types + sim types)
│       ├── Agent 3  (PCB Geometry — Clipper2, mesh gen)
│       │   ├── Agent 8  (2D PCB Renderer — instanced tracks/pads)
│       │   │   └── Agent 9  (3D PCB Renderer — Bevy PBR, STEP, thermal overlay)
│       │   │       ├── Agent 13 (OpenEMS — RF/EM FDTD → S-params)
│       │   │       └── Agent 14 (Elmer FEM — thermal → 3D overlay)
│       │   └── Agent 11 (Router & Ratsnest)
│       └── Agent 10 (ERC + DRC Engine — 11 ERC + 15 DRC rules)
└── Agent 4  (Bevy App Scaffolder)
    ├── Agent 5  (egui Schematic Panels — 9 panels + Signal AI)
    ├── Agent 6  (egui PCB + Sim Panels — 8 PCB + 5 sim panels)
    ├── Agent 7  (2D Schematic Renderer — KiCad SCH_PAINTER reference)
    │   └── Agent 8  (2D PCB Renderer)
    └── Agent 12 (ngspice Bridge — netlist gen + .raw parser)
        ├── Agent 13 (OpenEMS)
        └── Agent 14 (Elmer FEM)
            └── Agent 15 (WASM Plugin API — Extism) ← last
```

### Agent 0 — Workspace Architect
Remove Tauri + npm entirely. Move legacy TS/TSX to `_legacy/` (port agents reference it). Create Cargo workspace:
```
Cargo.toml                    # [workspace] manifest
libs/kicad-parser/            libs/kicad-writer/
libs/eda-types/               libs/pcb-geom/
libs/spice-gen/               libs/openems-bridge/
libs/elmer-bridge/            libs/step-loader/
libs/plugin-api/
src/                          # Bevy app
```

### Agent 1 — KiCad Parser/Writer Port
Move `src-tauri/src/engine/` → workspace crates. Remove `#[tauri::command]`, `tauri::State`. Keep S-expr tokenizer, KiCad 8/9/10 support, existing tests.

### Agent 2 — EDA Types
Extract all domain types to `libs/eda-types/`. No Bevy dependency. Includes:
- `schematic.rs` — Wire, Label, Symbol, Sheet, Junction, BusEntry
- `pcb.rs` — Track, Pad, Via, Zone, Footprint
- `net.rs` — Net, NetClass, Pin, DiffPair
- `layer.rs` — LayerId (0-63), LayerKind, LayerStackup
- `violation.rs` — ErcViolation (11 types), DrcViolation (15 types)
- `sim.rs` — SpiceModelRef, SimJob, SimKind, SimStatus, WaveformData, SParamData, ThermalMap

### Agent 3 — PCB Geometry (Clipper2)
Port TS geometry → Rust. polygon boolean (Clipper2), copper pour, ratsnest (MST/UnionFind), extrude 2D→3D, GMSH mesh gen (Elmer input), hit testing. All coords in KiCad IU (1 IU = 1nm).

### Agent 4 — Bevy App Scaffolder
Tauri main.rs → Bevy App. Plugins: viewport_2d (Camera2d + bevy_pancam), viewport_3d (PerspectiveCamera + bevy_panorbit_camera), view_mode (Schematic | Pcb2D | Pcb3D), schematic/pcb/sim (empty). Theme resource (CatppuccinMocha, VsCodeDark, AltiumDark, etc.).

### Agent 5 — egui Schematic Panels (9 panels)
Properties, Components (226 KiCad libs), Messages (ERC), Filter, List, Navigator, Signal AI (Claude streaming + tool use: run_spice_sim, show_waveform, analyze_thermal, check_si), OutputJobs, Variants. Pattern: `EventWriter` for mutations, never direct Resource mutate.

### Agent 6 — egui PCB + Simulation Panels
**PCB (8):** LayerStack, DRCResults, PCBProperties, PCBLibrary, NetClass, NetInspector, CrossSection, Inspector.
**Sim (5):** Waveform (ngspice — multi-trace, cursor, PNG/CSV export), SParams (OpenEMS — S11/S21 dB + Smith chart), Thermal (Elmer — color ramp, min/max, CSV), SimConfig (DC/AC/Transient/SI/Thermal), JobQueue (UUID, tool, status, duration). Sim panels show placeholder until engines complete.

### Agent 7 — 2D Schematic Renderer
**Reference KiCad source: eeschema/sch_painter.cpp, sch_symbol.cpp, sch_label.cpp, common/gal/opengl_gal.cpp**
Render order (from KiCad SCH_PAINTER): 1.Sheet borders 2.Drawing objects 3.Wires & buses 4.Bus entries 5.Junctions 6.No-connect 7.Net labels 8.Global/hier labels 9.Power ports 10.Symbols 11.Pins 12.Text
Static shapes: bevy_prototype_lyon. Dynamic overlays: Bevy Gizmos. Text: Text2d.

### Agent 8 — 2D PCB Renderer (Instanced)
**Reference KiCad source: pcbnew/pcb_painter.cpp, zone.cpp, pad.cpp**
Instanced rendering mandatory — 100K+ tracks/pads. Custom WGSL shaders for track.wgsl, pad.wgsl. Layer compositing order (KiCad): B.Cu→...→F.Cu (32 copper), paste, mask, silk, courtyard, fab, edge cuts.

### Agent 9 — 3D PCB Renderer (Bevy PBR)
Three.js → Bevy PBR. pcb-geom::extrude → Bevy Mesh. Layer stack: FR4 1.6mm, copper 35/70µm, soldermask 25µm, silkscreen 10µm. PBR materials: copper (metallic=1.0, roughness=0.3), soldermask (green/red/blue/black, alpha=0.95), FR4 (roughness=0.9). STEP async load (truck, AsyncComputeTaskPool). Thermal overlay hook for Agent 14.

### Agent 10 — ERC & DRC Engine
**ERC (11 rules):** unconnected wire/pin, conflicting nets, duplicate refs, pin conflict (12x12 matrix), missing power pin, label/hier pin not connected.
**DRC (15 rules):** clearance, min trace width, min via, annular ring, hole spacing, short circuit, solder mask sliver, courtyard overlap, footprint overlap, pad not connected, via not on copper, drill range, net stub, diff pair skew, length mismatch.

### Agent 11 — Router & Ratsnest
Interactive router: walkaround, push/shove, diff pair (gap control), meander (length tuning). Ratsnest: MST + UnionFind.

### Agent 12 — ngspice Bridge
`libs/spice-gen/`: SchematicDoc → .cir netlist (component values → SPICE elements, nets → node numbers, GND → node 0). .raw binary/ASCII parser. Subprocess: `ngspice -b -r output.raw netlist.cir`. AsyncComputeTaskPool. Waveform panel: multi-trace, dual cursor, delta readout, PNG/CSV export. Signal AI integration: waveform data as Claude context.

### Agent 13 — OpenEMS (RF/EM FDTD)
`libs/openems-bridge/`: PCB → CSX XML (traces → rectangular conductors, vias → cylinders, planes → copper sheets, board → FR4 substrate). Port definitions (lumped, waveguide, diff). Auto-mesh + refinement near conductors. HDF5 output → SParamData. UI: S11/S21 magnitude dB, phase, Smith chart, E-field 3D overlay (Bevy gizmo). Graceful error if OpenEMS not installed.

### Agent 14 — Elmer FEM (Thermal)
`libs/elmer-bridge/`: .sif generator (HeatEquation solver). Materials: FR4 k=0.3 W/(m·K), copper k=385, solder k=50. Boundary: component → heat source (W), board bottom → convection (h=5 natural, h=50 forced). GMSH bridge for mesh (.msh). VTK output → ThermalMap. 3D overlay: temperature → vertex color (blue=20°C → red=100°C). Component power input UI.

### Agent 15 — WASM Plugin API (Extism)
5 host function categories: Document (get_schematic, get_pcb, get_netlist), Mutation (add/delete/move/route), UI (toast, panel, menu, toolbar), Query (run_erc, run_drc, query_entities), Sim (run_spice, get_waveform, run_thermal, get_thermal_map). Permission gateway + undo stack integration.

### Workspace Dependencies (Latest Versions)
```toml
[workspace.dependencies]
bevy                 = { version = "0.18", default-features = false, features = [
                         "bevy_winit","bevy_render","bevy_core_pipeline",
                         "bevy_pbr","bevy_asset","bevy_sprite","bevy_text",
                         "bevy_gizmos","multi_threaded","hdr"] }
bevy_egui            = "0.39"
bevy_pancam          = "0.20"
bevy_panorbit_camera = "0.34"
bevy_prototype_lyon  = "0.16"
egui_dock            = "0.19"
extism               = "1.21"
serde                = { version = "1", features = ["derive"] }
serde_json           = "1"
tokio                = { version = "1", features = ["full"] }
reqwest              = { version = "0.12", features = ["json","rustls-tls","stream"], default-features = false }
uuid                 = { version = "1", features = ["v4","serde"] }
chrono               = { version = "0.4", features = ["serde"] }
thiserror            = "1"
anyhow               = "1"
nom                  = "8"
hdf5                 = "0.8"          # OpenEMS HDF5 output
vtkio                = "0.6"          # Elmer VTK output
truck-modeling       = "0.6"          # STEP import
nalgebra             = "0.34"
clipper2             = "0.5"          # Polygon boolean ops
rfd                  = "0.17"         # Native file dialogs
arboard              = "3.6"          # Clipboard
```

### External Tool Versions
| Tool | Version | Purpose |
|---|---|---|
| ngspice | 46 | SPICE circuit simulation |
| OpenEMS | 0.0.36 | RF/EM FDTD simulation |
| Elmer FEM | 26.1 | Thermal FEM simulation |
| GMSH | 4.15.2 | Mesh generation (Elmer input) |

### Simulation Rules (All Agents)
- All sim tools run in `AsyncComputeTaskPool` — main thread never blocks
- Every job gets UUID, writes to `SimResultStore` resource
- Tool not found → graceful error message, never crash
- Sim results feed Signal AI panel (Claude context)

### Quality Constraints
- `unwrap()`/`expect()` forbidden in production — use `?` or `match`
- `clippy::all` + `clippy::pedantic` zero warnings
- `unsafe` forbidden (WASM FFI exception, must be commented)
- Every public function has doc comment
- Commit format: `feat(agent-N): description` / `fix(agent-N): description`

### Final Validation
```bash
cargo build --workspace --release
cargo test --workspace
cargo clippy --workspace -- -D warnings
# 1. Open .kicad_pro → schematic renders correctly
# 2. PCB edit → DRC 15 rules pass
# 3. SPICE sim → waveform visible
# 4. OpenEMS → S-param plot visible
# 5. Elmer → 3D thermal overlay visible
# 6. Signal AI → tool use works (sim tools included)
# 7. WASM plugin load → toast visible
```
