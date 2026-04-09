# GUI Framework Decision: Bevy + bevy_egui for Signex

## Final Architecture Decision

**Bevy 0.18 (wgpu viewport) + bevy_egui 0.39 (panels/toolbars) + Simulation Stack**

| Layer | Technology | Version | Role |
|---|---|---|---|
| **Rendering engine** | Bevy (wgpu) | 0.18 | Schematic canvas, PCB layout, 3D viewer |
| **UI panels** | egui via bevy_egui | egui 0.34 / bevy_egui 0.39 | Property inspector, component browser, menus, toolbars, dialogs |
| **2D shapes** | bevy_prototype_lyon + Bevy Gizmos | 0.16 | Wires, pads, tracks, symbol graphics, grid |
| **Camera 2D** | bevy_pancam | 0.20 | Pan/zoom/fit-to-screen (right-click pan = Altium UX) |
| **Camera 3D** | bevy_panorbit_camera | 0.34 | 3D PCB viewer orbit |
| **Hit testing** | Bevy built-in picking | 0.18+ | Click-to-select on schematic/PCB elements |
| **Dockable panels** | egui_dock | 0.19 | Altium-style panel rearrangement |
| **PCB geometry** | Clipper2 | 0.5 | Polygon boolean, copper pour, thermal relief |
| **STEP import** | truck-modeling | 0.6 | 3D component models |
| **SPICE sim** | ngspice (subprocess) | 46 | DC/AC/Transient circuit simulation |
| **RF/EM sim** | OpenEMS (subprocess) | 0.0.36 | FDTD → S-parameters, signal integrity |
| **Thermal sim** | Elmer FEM (subprocess) | 26.1 | FEM → temperature distribution |
| **Mesh gen** | GMSH | 4.15.2 | Mesh generation for Elmer FEM input |
| **EM output** | HDF5 (hdf5 crate) | 0.8 | OpenEMS result parsing |
| **FEM output** | VTK (vtkio crate) | 0.6 | Elmer result parsing |
| **WASM plugins** | Extism | 1.21 | Third-party plugin API |
| **AI copilot** | Claude API (reqwest) | — | Signal AI panel with sim tool use |

---

## Why Bevy + egui Over Pure egui

| Concern | Pure egui (eframe) | Bevy + bevy_egui |
|---|---|---|
| **2D rendering** | egui::Painter (CPU tessellation each frame) | Bevy Mesh/Sprite (GPU-resident, batched) |
| **Pan/zoom** | Manual transform math or Scene container | Bevy Camera2d + OrthographicProjection (native) |
| **Hit testing** | Custom math per element type | Bevy MeshPickingPlugin (built-in) |
| **3D viewer** | Not possible — need separate wgpu integration | Same app, same window — just add Camera3d + PBR |
| **ECS data model** | Manual state management (AppState struct) | Entities with Components — scalable, queryable, parallelized |
| **Large schematics** | Degrades — CPU re-tessellates everything per frame | GPU-batched — thousands of entities at 60fps |
| **PCB rendering** | Would need raw wgpu code for instanced tracks/pads | Bevy instanced rendering + custom WGSL shaders |
| **Simulation integration** | Background thread + request_repaint() | Bevy AsyncComputeTaskPool + ECS event system |
| **Thermal overlay** | Manual wgpu vertex color update | Bevy StandardMaterial vertex colors, hot-swappable |

### The Key Insight

egui is an **immediate-mode UI toolkit** — excellent for panels, forms, and toolbars. But Signex's core is a **2D/3D graphics application** with simulation visualization that needs a proper rendering engine. Bevy provides that engine, while bevy_egui lets us keep egui for the UI chrome. Best of both worlds.

---

## How bevy_egui Works

bevy_egui renders egui as an overlay on top of Bevy's rendering pipeline:

```
┌─────────────────────────────────────────────────────┐
│  Bevy Window                                        │
│  ┌──────────┬──────────────────────┬──────────────┐ │
│  │ egui     │   Bevy 2D Viewport   │ egui         │ │
│  │ Project  │   (Camera2d)         │ Properties   │ │
│  │ Panel    │                      │ Panel        │ │
│  │          │   Schematic canvas   │              │ │
│  │          │   rendered by Bevy   │              │ │
│  │          │   meshes/sprites     │              │ │
│  ├──────────┴──────────────────────┴──────────────┤ │
│  │ egui Bottom: Messages / Waveform / Thermal     │ │
│  └────────────────────────────────────────────────┘ │
│  egui MenuBar + Toolbar (top)                       │
│  egui StatusBar (bottom)                            │
└─────────────────────────────────────────────────────┘
```

- egui panels use `SidePanel`, `TopBottomPanel`, `Window`
- Bevy viewport fills the `CentralPanel` remainder
- bevy_egui handles input routing: clicks on egui panels go to egui, clicks on viewport go to Bevy
- Both share the same wgpu surface — zero overhead for compositing

---

## Bevy ECS Architecture for Schematic + PCB

### Entity-Component Mapping

Every schematic/PCB element becomes a Bevy Entity with typed Components:

```rust
// Schematic Components
#[derive(Component)] struct SchematicWire { start: Vec2, end: Vec2 }
#[derive(Component)] struct SchematicSymbol { lib_id: String, reference: String, value: String }
#[derive(Component)] struct SchematicJunction;
#[derive(Component)] struct SchematicLabel { text: String, label_type: LabelType }
#[derive(Component)] struct SchematicPin { name: String, number: String, pin_type: PinType }

// PCB Components
#[derive(Component)] struct PcbTrack { start: Vec2, end: Vec2, width: f32, layer: u8 }
#[derive(Component)] struct PcbPad { shape: PadShape, size: Vec2, drill: f32 }
#[derive(Component)] struct PcbVia { position: Vec2, drill: f32, layers: (u8, u8) }
#[derive(Component)] struct PcbZone { polygon: Vec<Vec2>, net: NetId, layer: u8 }

// Shared markers
#[derive(Component)] struct Selected;
#[derive(Component)] struct Hoverable;
#[derive(Component)] struct NetId(String);
```

### Systems Architecture

```rust
// Rendering
fn render_grid(gizmos: Gizmos, camera: Query<&OrthographicProjection>) { ... }
fn render_wires(query: Query<(&SchematicWire, &Transform, Option<&Selected>)>, gizmos: Gizmos) { ... }
fn render_selection_overlay(query: Query<&Transform, With<Selected>>, gizmos: Gizmos) { ... }

// Input
fn handle_keyboard(keys: Res<ButtonInput<KeyCode>>, ...) { ... }
fn handle_mouse_click(picks: Query<&PickingInteraction>, ...) { ... }

// Logic
fn erc_system(symbols: Query<&SchematicSymbol>, wires: Query<&SchematicWire>, ...) { ... }
fn drc_system(tracks: Query<&PcbTrack>, pads: Query<&PcbPad>, constraints: Res<DesignConstraints>) { ... }

// Simulation (async)
fn poll_sim_results(mut sim_store: ResMut<SimResultStore>, mut events: EventWriter<SimCompleteEvent>) { ... }
```

### Advantages Over Current Architecture

1. **Parallel systems**: Bevy automatically parallelizes non-conflicting systems
2. **Spatial queries**: Bevy's query system replaces manual iteration over arrays
3. **Entity lifecycle**: Spawn/despawn replaces array push/splice
4. **Change detection**: `Changed<T>` filter — only process entities that actually changed
5. **Events**: `EventWriter<T>` / `EventReader<T>` for decoupled communication
6. **Async tasks**: `AsyncComputeTaskPool` for simulation without blocking rendering

---

## Simulation Architecture

### Simulation Stack Overview

```
┌─────────────┐    ┌───────────┐    ┌────────────┐
│ Schematic    │───→│ spice-gen │───→│ ngspice 46 │───→ Waveform Panel
│ (ECS)       │    │ .cir      │    │ subprocess │    (time, V, I)
└─────────────┘    └───────────┘    └────────────┘

┌─────────────┐    ┌────────────────┐    ┌──────────────┐
│ PCB Layout   │───→│ openems-bridge │───→│ OpenEMS 0.0.36│───→ S-Param Panel
│ (ECS)       │    │ .xml (CSX)     │    │ subprocess   │    (S11/S21 dB, Smith)
└─────────────┘    └────────────────┘    └──────────────┘

┌─────────────┐    ┌──────────────┐    ┌─────────────┐    ┌──────────────┐
│ PCB Layout   │───→│ elmer-bridge │───→│ GMSH 4.15.2 │───→│ Elmer 26.1   │───→ Thermal Panel
│ + Power (W) │    │ .sif + .msh  │    │ mesh gen    │    │ FEM solver   │    (3D overlay)
└─────────────┘    └──────────────┘    └─────────────┘    └──────────────┘
```

### ngspice (SPICE Circuit Simulation)

- **Input**: SchematicDoc → .cir netlist (component values → SPICE elements, nets → node numbers)
- **Execution**: `ngspice -b -r output.raw netlist.cir` (subprocess)
- **Output**: .raw binary/ASCII → WaveformData (t, V(node), I(branch))
- **UI**: Multi-trace waveform viewer, dual cursor, delta measurement, PNG/CSV export
- **AI**: Results fed to Signal AI panel as Claude context

### OpenEMS (RF/EM FDTD — Signal Integrity)

- **Input**: PCB traces/vias/planes → CSX XML (conductors, substrate, ports)
- **Execution**: OpenEMS binary (subprocess), auto-mesh with refinement near conductors
- **Output**: HDF5 → SParamData (S11/S21 magnitude dB, phase, complex S-matrix)
- **UI**: S-parameter plot, Smith chart, E-field 3D overlay (Bevy gizmo)
- **Use case**: 50Ω trace impedance, diff pair matching, via transition analysis

### Elmer FEM (Thermal Analysis)

- **Input**: PCB geometry → GMSH mesh (.msh) + Elmer .sif (HeatEquation solver)
- **Materials**: FR4 (k=0.3 W/m·K), copper (k=385), solder (k=50)
- **Boundary**: Component → heat source (W), board bottom → convection (h=5 natural, h=50 forced)
- **Output**: VTK → ThermalMap (mesh points + temperatures)
- **UI**: Temperature color ramp (blue 20°C → red 100°C) as 3D PCB overlay vertex colors
- **Integration**: Agent 9's `thermal_overlay.rs` reads ThermalMap, updates Bevy StandardMaterial

### Simulation Rules

- All simulations run in Bevy `AsyncComputeTaskPool` — main thread never blocks
- Every job gets a UUID, tracked in `SimResultStore` resource
- Tool not found → graceful error message ("OpenEMS not installed. See: https://openems.de"), never crash
- Results feed Signal AI panel for Claude-powered analysis

---

## View Modes

```rust
pub enum ViewMode {
    Schematic,   // Camera2d + bevy_pancam, schematic entities visible
    Pcb2D,       // Camera2d + bevy_pancam, PCB entities visible, layer compositing
    Pcb3D,       // Camera3d + bevy_panorbit_camera, extruded PCB + PBR materials
}
```

Switching view modes: despawn/hide current view entities, spawn/show target view entities. Camera switches between 2D and 3D. Same Bevy window, same egui panels (panels adapt to view mode).

---

## 2D Shape Rendering Strategy

### bevy_prototype_lyon 0.16 (Static Vector Shapes)

```rust
// Rectangle (symbol body)
commands.spawn((
    ShapeBundle {
        path: GeometryBuilder::build_as(&shapes::Rectangle {
            extents: Vec2::new(width, height), ..default()
        }),
        ..default()
    },
    Stroke::new(Color::hex("9fa8da"), 0.15),
    Fill::color(Color::hex("1e2035")),
));

// Circle (junction)
commands.spawn((
    ShapeBundle {
        path: GeometryBuilder::build_as(&shapes::Circle { radius: 0.3, ..default() }),
        ..default()
    },
    Fill::color(Color::hex("4fc3f7")),
));

// Arc (via lyon's arc_to)
let mut builder = PathBuilder::new();
builder.move_to(start);
builder.arc_to(radius, radius, 0.0, flags, end);
```

### Bevy Gizmos (Dynamic Overlays — zero allocation)

```rust
fn render_grid(mut gizmos: Gizmos, camera: Query<&OrthographicProjection>) {
    for x in grid_range_x {
        gizmos.line_2d(Vec2::new(x, min_y), Vec2::new(x, max_y), GRID_COLOR);
    }
}

fn render_crosshair(mut gizmos: Gizmos, cursor: Res<WorldCursor>) {
    gizmos.line_2d(Vec2::new(cursor.x - 3.0, cursor.y), Vec2::new(cursor.x + 3.0, cursor.y), CURSOR_COLOR);
    gizmos.line_2d(Vec2::new(cursor.x, cursor.y - 3.0), Vec2::new(cursor.x, cursor.y + 3.0), CURSOR_COLOR);
}
```

### PCB Instanced Rendering (100K+ tracks/pads)

```rust
#[derive(Component)]
struct TrackInstance {
    start: Vec2, end: Vec2, width: f32,
    layer: u8, net_color: Color,
}
// Custom WGSL shaders: track.wgsl, pad.wgsl
// bevy_render::render_resource::InstanceBuffer for GPU instancing
```

---

## Camera & Viewport

### bevy_pancam 0.20 for 2D Pan/Zoom

```rust
app.add_plugins(PanCamPlugin);

commands.spawn((
    Camera2d,
    OrthographicProjection { scale: 1.0, ..OrthographicProjection::default_2d() },
    PanCam {
        grab_buttons: vec![MouseButton::Right, MouseButton::Middle],
        zoom_to_cursor: true,
        min_scale: 0.01,   // max zoom in
        max_scale: 100.0,  // max zoom out
        ..default()
    },
));
```

### bevy_panorbit_camera 0.34 for 3D Orbit

```rust
commands.spawn((
    Camera3d::default(),
    PanOrbitCamera {
        button_orbit: MouseButton::Right,
        button_pan: MouseButton::Middle,
        ..default()
    },
));
```

---

## 3D PCB Renderer (Bevy PBR)

```rust
// Layer stack dimensions
const FR4_THICKNESS: f32 = 1.6;        // mm
const COPPER_1OZ: f32 = 0.035;         // mm (35µm)
const SOLDERMASK: f32 = 0.025;         // mm (25µm)
const SILKSCREEN: f32 = 0.010;         // mm (10µm)

// PBR materials
fn copper_material() -> StandardMaterial {
    StandardMaterial { base_color: Color::hex("B87333"), metallic: 1.0, roughness: 0.3, ..default() }
}
fn soldermask_green() -> StandardMaterial {
    StandardMaterial { base_color: Color::hex("1B5E20").with_alpha(0.95), ..default() }
}
fn fr4_material() -> StandardMaterial {
    StandardMaterial { base_color: Color::hex("8B7355"), metallic: 0.0, roughness: 0.9, ..default() }
}
```

STEP models loaded async via `truck-modeling` 0.6 + `AsyncComputeTaskPool` (UI never freezes).

---

## Comparison With Alternatives Considered

### vs Slint
Slint has no custom rendering canvas, no 3D support, GPL license for open-source. Not viable for an EDA tool that needs canvas-heavy rendering and simulation visualization.

### vs Pure egui (eframe)
egui alone handles UI well but lacks a proper 2D/3D rendering engine. Would require bolting on raw wgpu for schematics, PCB, and 3D viewer. Bevy provides this natively with GPU batching, spatial queries, instanced rendering, and an ECS data model that fits EDA entities.

### vs Current Stack (Tauri + React + Canvas2D)
Canvas2D hits a wall at PCB complexity and has no 3D capability. IPC serialization boundary wastes CPU. Two-process architecture. No path to simulation visualization (waveforms, S-params, thermal maps). Bevy + bevy_egui is a single Rust process with GPU rendering and native simulation integration.

---

## Workspace Dependencies (Latest Versions — April 2026)

```toml
[workspace.dependencies]
# Core
bevy                 = { version = "0.18", default-features = false, features = [
                         "bevy_winit","bevy_render","bevy_core_pipeline",
                         "bevy_pbr","bevy_asset","bevy_sprite","bevy_text",
                         "bevy_gizmos","multi_threaded","hdr"] }
bevy_egui            = "0.39"
bevy_pancam          = "0.20"
bevy_panorbit_camera = "0.34"
bevy_prototype_lyon  = "0.16"
egui_dock            = "0.19"

# Geometry & Math
clipper2             = "0.5"          # Polygon boolean ops
nalgebra             = "0.34"         # Linear algebra
truck-modeling       = "0.6"          # STEP import

# Simulation I/O
hdf5                 = "0.8"          # OpenEMS HDF5 output
vtkio                = "0.6"          # Elmer VTK output

# Plugin System
extism               = "1.21"         # WASM plugin framework

# Serialization & I/O
serde                = { version = "1", features = ["derive"] }
serde_json           = "1"
nom                  = "8"            # Parser combinator
tokio                = { version = "1", features = ["full"] }
reqwest              = { version = "0.12", features = ["json","rustls-tls","stream"], default-features = false }

# Utilities
uuid                 = { version = "1", features = ["v4","serde"] }
chrono               = { version = "0.4", features = ["serde"] }
thiserror            = "1"
anyhow               = "1"
rfd                  = "0.17"         # Native file dialogs
arboard              = "3.6"          # Clipboard
```

### External Tools

| Tool | Version | License | Purpose |
|---|---|---|---|
| **ngspice** | 46 | BSD-3 | SPICE circuit simulation (DC, AC, Transient) |
| **OpenEMS** | 0.0.36 | GPL-3.0 | RF/EM FDTD simulation → S-parameters |
| **Elmer FEM** | 26.1 | GPL-2.0 | Thermal FEM simulation → temperature map |
| **GMSH** | 4.15.2 | GPL-2.0 | Mesh generation for Elmer FEM input |
