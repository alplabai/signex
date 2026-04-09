# Signex — AI-First Electronic Design Automation

## Project Overview
Desktop EDA tool with Altium Designer-class UX.
Target: schematic capture, PCB layout, 3D viewer, SI simulation, AI copilot (Signal).

## Architecture (Current — migrating to Bevy)
- **Desktop shell:** Tauri v2 (Rust backend) → migrating to Bevy + bevy_egui
- **Frontend:** React 19 + TypeScript + Vite + Tailwind CSS 4 → migrating to egui panels via bevy_egui
- **Canvas:** Canvas2D → migrating to Bevy 2D (wgpu, bevy_prototype_lyon, Gizmos)
- **Camera:** → Bevy Camera2d + bevy_pancam (pan/zoom)
- **Parser:** Pure Rust S-expression parser for KiCad format (.kicad_sch, .kicad_sym) — unchanged
- **3D:** Three.js (future) → Bevy Camera3d + PBR (built-in)
- **AI:** Claude API via Rust reqwest client — branded "Signal"
- **State:** Zustand (4 stores) → Bevy ECS (entities + components + systems)
- **Picking:** → Bevy MeshPickingPlugin (built-in 0.15+)

## Project Structure
```
src-tauri/src/          Rust backend
  commands/             Tauri IPC commands (project, schematic, save, library)
  engine/               KiCad S-expr parser, document model, writer
    parser.rs           Schematic + symbol library parser
    sexpr.rs            Generic S-expression tokenizer
    writer.rs           KiCad S-expr serializer
    document.rs         Document model (future)
src/                    React frontend
  components/           Shell: MenuBar, ToolbarStrip, TabBar, StatusBar, ComponentSearch
  panels/               Dockable panels: Project, Properties, Messages, Signal
  canvas/               SchematicRenderer (Canvas2D), EditorCanvas, hitTest
  stores/               Zustand state: layout, project, editor, schematic
  hooks/                useResizable, useTauriCommand
  types/                TypeScript type definitions
  lib/                  Utilities (cn)
```

## Commands
- `npm run dev` — Vite dev server (frontend only)
- `npm run tauri dev` — Full Tauri dev (frontend + Rust)
- `npm run build` — Frontend production build
- `npx tsc --noEmit` — TypeScript check

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
- Bevy + bevy_egui chosen: Bevy for 2D/3D rendering + ECS, egui for UI panels/toolbars
- Bevy over pure egui: proper rendering engine with GPU batching, built-in picking, native 3D support
- Workspace split: signex-engine (parser, types, no Bevy dep) + signex-gui (Bevy app)
- Pure Rust parser instead of KiCad C++ FFI — simpler build, no C++ toolchain dependency
- ECS entity-per-element: each wire/symbol/junction/label is a Bevy Entity with typed Components
- Command-based undo (not full-state snapshots) — better for ECS architecture
- bevy_prototype_lyon for static 2D shapes, Bevy Gizmos for dynamic overlays (grid, selection, cursor)
- bevy_pancam for camera controls (right-click pan matches Altium UX)

---

## Bevy + egui Porting Plan

**Decision:** Migrate from Tauri + React + Canvas2D → Bevy (wgpu) + bevy_egui.
**Why:** GPU-rendered 2D/3D viewport, ECS data model for schematic entities, zero IPC overhead, single Rust process, 3D PCB viewer built-in.
**Detailed reference:** `docs/gui-framework-comparison.md`

### Architecture Split
- **Bevy** → schematic/PCB canvas rendering, camera (pan/zoom), entity management, picking, 3D viewer
- **egui (via bevy_egui)** → UI panels, menus, toolbars, property inspector, dialogs, status bar
- **Engine crate** → KiCad parser/writer, library search, export (unchanged)

### Migration Phases

#### Phase 0: Scaffold & Bevy App Shell (Week 1)
- Create `signex-gui` crate with Bevy app
- Add dependencies: bevy, bevy_egui, bevy_prototype_lyon, bevy_pancam, rfd, egui_dock
- Set up Bevy App with: DefaultPlugins, EguiPlugin, ShapePlugin (lyon), PanCamPlugin
- Create empty egui side panels (left, right, bottom) + menu bar + status bar
- Spawn Camera2d with OrthographicProjection + PanCam (right-click pan, scroll zoom)
- Verify builds on Windows, macOS, Linux
- Port color palette to Bevy `Color` / egui `Color32` constants

#### Phase 1: Engine Extraction (Week 1)
- Extract `engine/` into standalone `signex-engine` crate (parser.rs, writer.rs, sexpr.rs, library.rs, export.rs)
- Remove `#[tauri::command]` wrappers → plain `pub fn` API
- Engine crate has zero dependency on Bevy or egui — pure data types + I/O
- `signex-gui` depends on `signex-engine`

#### Phase 2: ECS Data Model (Week 1-2)
- Define Bevy Components for every schematic element:
  - `SchematicWire { start: Vec2, end: Vec2 }` + `Transform` + `Mesh2d`
  - `SchematicSymbol { lib_id, reference, value, unit, ... }` + child entities for graphics/pins
  - `SchematicJunction` (marker) + `Transform`
  - `SchematicLabel { text, label_type, shape }` + `Transform` + `Text2d`
  - `SchematicNoConnect` + `SchematicBus` + `SchematicBusEntry` + `SchematicTextNote`
  - `Selected` marker component, `Hoverable` marker, `NetId(String)`
- Write `spawn_schematic()` system: takes parsed `SchematicSheet` → spawns all entities
- Write `collect_schematic()` system: queries all entities → builds `SchematicSheet` for saving
- Port Zustand schematic store actions (1,320 LOC) → Bevy systems + commands
- Undo/redo: snapshot `World` state or command-based undo stack

#### Phase 3: Schematic Renderer — Bevy Systems (Week 2-3)
Port SchematicRenderer.tsx (1,885 LOC) → Bevy rendering systems:

**Static shapes via bevy_prototype_lyon** (spawned once, updated on change):
- Symbol graphics: `PathBuilder` → polylines, rectangles, circles, arcs
- Wires: `Mesh2d` line strips with configurable width
- Junctions: `Circle` shape fill
- Labels: polygon outlines (input/output/bidirectional shapes)
- No-connect: X-shaped line pairs
- Bus entries: angled line segments

**Dynamic overlays via Bevy Gizmos** (redrawn each frame, zero allocation):
- Grid lines (major/minor, adaptive to zoom level)
- Selection box (dashed rectangle)
- Wire drawing preview (Manhattan/diagonal/free routing)
- Crosshair cursor
- Selection handles (corner + midpoint squares)
- Rubber-band wire preview

**Text via Text2d**:
- Reference designators (R1, C1)
- Values (10kΩ, 100nF)
- Pin names and numbers
- Net label text
- Text notes

**Canvas2D → Bevy Mapping:**
| Canvas2D | Bevy Equivalent |
|---|---|
| `fillRect` | `lyon::shapes::Rectangle` + `Fill` |
| `strokeRect` | `lyon::shapes::Rectangle` + `Stroke` |
| `beginPath+lineTo+stroke` | `lyon::PathBuilder` + `Stroke` or `Gizmos::line_2d()` |
| `arc(full circle)` | `lyon::shapes::Circle` + `Fill`/`Stroke` |
| `arc(partial)` | `lyon::PathBuilder::arc()` |
| `fillText` | `Text2d` + `TextFont` + `TextColor` |
| `setLineDash` | `Gizmos::line_2d()` with manual dash segmentation |
| `translate/rotate` | `Transform` component (position, rotation) |
| `globalAlpha` | `Color::with_alpha()` or `Visibility` |
| `measureText` | `TextPipeline::measure()` or layout query |

#### Phase 4: Input & Interaction (Week 3)
- **Picking**: Use Bevy's built-in `MeshPickingPlugin` for click-to-select on entities
  - Wire picking: spawn invisible mesh strip along wire path as pick target
  - Symbol picking: bounding box mesh as pick target
  - Pin picking: small circle mesh at pin endpoint
- **Keyboard shortcuts**: Bevy `ButtonInput<KeyCode>` in systems
  - Port all 40+ shortcuts (W=wire, R=rotate, Space=rotate, Delete, Ctrl+Z, etc.)
- **Mouse interaction**:
  - Left-click: select (via picking events)
  - Right-click + drag: pan (via bevy_pancam)
  - Middle-drag: pan (via bevy_pancam)
  - Scroll: zoom (via bevy_pancam)
  - Left-drag: box select or move selected
  - Alt+click: select entire net
- **Wire drawing mode**: state machine resource + Gizmos preview + click-to-place
- **Component placement mode**: ghost entity follows cursor, click to commit
- **Rubber-band**: query wires connected to moved symbols, update endpoints
- **In-place text editing**: egui `TextEdit` overlay positioned at entity's screen coords

#### Phase 5: UI Panels via egui (Week 3-4)
All panels render via bevy_egui — egui systems that access Bevy world via `EguiContexts`:

- **MenuBar** → `egui::TopBottomPanel::top()` with `menu::bar()`
- **ToolbarStrip** → `egui::TopBottomPanel::top()` (second row) with icon buttons
- **StatusBar** → `egui::TopBottomPanel::bottom()` — cursor pos, grid, zoom, units
- **ProjectPanel** → `egui::SidePanel::left()` — file tree with CollapsingHeader
- **ComponentPanel** → TextEdit search + ScrollArea list → spawns symbols on select
- **PropertiesPanel** → `egui::SidePanel::right()` — reads `Selected` entities, edits components
  - Largest port (~1,044 LOC React → ~500 LOC egui). Use `egui::Grid` for property rows
  - Directly mutates Bevy components via `Query<&mut SchematicSymbol, With<Selected>>`
- **MessagesPanel** → `egui::TopBottomPanel::bottom()` or dock tab — ERC markers
- **NavigatorPanel** → render minimap to `egui::ColorImage`
- **Dialogs**: Preferences, Find/Replace, About → `egui::Window` (floating)
- **Context menu** → `egui::Area` positioned at cursor on right-click
- **File dialog** → rfd (already works with Bevy, no Tauri needed)
- **Docking** → egui_dock for panel rearrangement (Altium-style)

#### Phase 6: State & Logic Systems (Week 4)
- Port ERC (erc.ts 284 LOC + ercMatrix.ts 217 LOC) → Bevy system querying entities
- Port net resolver (netResolver.ts 196 LOC) → Bevy system with `NetId` components
- Port auto-annotation → system that queries `SchematicSymbol` entities, assigns designators
- Port clipboard → `arboard` crate + custom serialize/deserialize of selected entities
- Port undo/redo → command pattern: each action records its inverse
  - Better than full-state snapshots for ECS (entities are transient)
  - Alternative: use `bevy_undo` crate if mature enough
- Layout persistence → serialize panel state (egui_dock) + camera transform to JSON

#### Phase 7: Polish & Parity (Week 4-5)
- Theme: Catppuccin Mocha palette → `egui::Visuals` + Bevy `ClearColor`
- Font: load Roboto/Inter via Bevy `AssetServer`, configure 13px base for egui
- Tab bar: multi-sheet navigation (egui tab strip or custom)
- Window title: `Window { title: "Signex — project.kicad_pro *", .. }`
- Dirty indicator: track `Changed<SchematicSymbol>` etc.
- Zoom-to-fit (Home key): compute bounding box of all entities, set camera transform
- Focus mode: dim non-focused entities via `Visibility` or alpha tint
- Grid snap: round cursor world position to grid increments
- Test all keyboard shortcuts match Altium reference doc

#### Phase 8: Cleanup & Cutover (Week 5-6)
- Remove Tauri, React, TypeScript, npm dependencies
- Remove src/, package.json, vite.config.ts, tsconfig.json, tailwind.config.ts, src-tauri/
- Project structure becomes workspace: `signex-engine` + `signex-gui`
- Write Rust tests (port 53 Vitest tests + add Bevy system tests)
- Verify KiCad file round-trip: open → edit → save → reopen in KiCad
- Cross-platform build verification (Windows, macOS, Linux)
- Binary size and startup time benchmarks

### Key Dependencies
```toml
[workspace]
members = ["signex-engine", "signex-gui"]

# signex-engine/Cargo.toml — zero Bevy dependency
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
uuid = { version = "1", features = ["v4"] }

# signex-gui/Cargo.toml
[dependencies]
signex-engine = { path = "../signex-engine" }
bevy = "0.15"                    # Game engine (wgpu, ECS, windowing)
bevy_egui = "0.35"               # egui integration for Bevy
bevy_prototype_lyon = "0.13"     # 2D vector shapes (lyon tessellation)
bevy_pancam = "0.14"             # Pan/zoom camera controls
rfd = "0.15"                     # Native file dialogs (already used)
egui_dock = "0.15"               # Dockable panel layout
arboard = "3"                    # Clipboard
tokio = { version = "1", features = ["rt-multi-thread"] }  # Async I/O, ngspice
reqwest = { version = "0.12", features = ["json"] }        # Claude API
```

### Architecture After Migration
```
signex-engine/                # Standalone crate — no Bevy dependency
  src/
    lib.rs                   # Public API
    parser.rs                # KiCad S-expr parser
    sexpr.rs                 # S-expression tokenizer
    writer.rs                # KiCad S-expr serializer
    types.rs                 # SchematicSheet, Symbol, Wire, etc.
    library.rs               # Symbol library search (226 KiCad libs)
    export.rs                # BOM, netlist generation

signex-gui/                  # Bevy application
  src/
    main.rs                  # Entry: App::new() + plugins
    plugins/
      mod.rs                 # Plugin registration
      schematic.rs           # SchematicPlugin — all schematic systems
      ui.rs                  # UiPlugin — all egui panel systems
      input.rs               # InputPlugin — keyboard/mouse handling
    components/              # Bevy ECS components
      mod.rs                 # Component definitions
      wire.rs                # SchematicWire
      symbol.rs              # SchematicSymbol + child graphics/pins
      label.rs               # SchematicLabel, NetLabel, PowerPort
      junction.rs            # SchematicJunction
      selection.rs           # Selected, Hoverable, Dragging
    systems/
      render/
        grid.rs              # Grid rendering (Gizmos)
        symbols.rs           # Symbol spawning & mesh generation
        wires.rs             # Wire mesh generation
        labels.rs            # Label text + shape rendering
        overlays.rs          # Selection box, handles, crosshair, wire preview
      input/
        shortcuts.rs         # Keyboard shortcut dispatch
        mouse.rs             # Click, drag, box-select
        picking.rs           # Entity picking reactions
        wire_drawing.rs      # Wire placement state machine
        placement.rs         # Component placement mode
      logic/
        erc.rs               # Electrical rules checking
        net_resolver.rs      # Net connectivity analysis
        annotation.rs        # Auto-annotation, designator assignment
        snap.rs              # Grid snap, electrical snap
        undo.rs              # Undo/redo command stack
    ui/                      # egui panels (via bevy_egui)
      menu.rs                # Menu bar
      toolbar.rs             # Toolbar strip
      status_bar.rs          # Status bar (cursor, grid, zoom)
      properties.rs          # Property inspector (reads/writes ECS components)
      components.rs          # Component library browser
      project.rs             # Project tree panel
      messages.rs            # ERC messages panel
      navigator.rs           # Sheet minimap
      dialogs.rs             # Preferences, Find/Replace, About
      dock.rs                # egui_dock layout management
    resources/               # Bevy Resources (global state)
      project.rs             # ProjectInfo, open tabs, active sheet
      editor.rs              # EditMode, grid settings, snap settings
      colors.rs              # Color palette constants
      clipboard.rs           # Clipboard contents
    startup.rs               # Asset loading, default camera, initial state
```

### Risk Mitigation
- **Bevy learning curve**: Bevy's ECS is different from React. Budget time for the team to learn the entity/component/system paradigm. Start with the rendering systems (most mechanical port), then tackle interaction
- **bevy_prototype_lyon performance**: For very large schematics (1000+ symbols), lyon tessellation at spawn time is fine. But if shapes change often, profile and consider raw Mesh generation
- **Text rendering**: Bevy's Text2d is functional but less polished than browser text. Test font scaling at extreme zoom levels. May need SDF text for crisp rendering at all scales
- **Picking tolerance on thin wires**: Bevy's mesh picking requires the pick target to have area. For wires, spawn invisible quads (2-3px wide) along wire paths as pick hitboxes
- **Undo/redo in ECS**: Full world snapshots are expensive. Use command-based undo (record each action + its inverse). More work upfront but scales better
- **Compile times**: Bevy is heavy. Use `dynamic_linking` feature during development. Split into workspace crates for incremental builds
- **egui_dock + bevy_egui**: Verify compatibility between versions. Pin exact versions in Cargo.toml
- **3D PCB viewer (future)**: Already possible in same Bevy app — just add Camera3d + 3D meshes. STEP/3D model import via community crates when needed
