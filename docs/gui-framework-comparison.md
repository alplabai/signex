# GUI Framework Decision: Bevy + bevy_egui for Signex

## Final Architecture Decision

**Bevy (wgpu viewport) + bevy_egui (panels/toolbars)**

| Layer | Technology | Role |
|---|---|---|
| **Rendering engine** | Bevy (wgpu) | Schematic canvas, PCB layout, 3D viewer, waveform plots |
| **UI panels** | egui via bevy_egui | Property inspector, component browser, menus, toolbars, dialogs |
| **2D shapes** | bevy_prototype_lyon + Bevy Gizmos | Wires, pads, tracks, symbol graphics, grid |
| **Camera** | Bevy Camera2d + bevy_pancam | Pan/zoom/fit-to-screen |
| **Hit testing** | Bevy built-in picking (0.15+) | Click-to-select on schematic elements |
| **3D PCB viewer** | Bevy Camera3d + PBR | Future — Bevy already supports this natively |

## Why Bevy + egui Over Pure egui

| Concern | Pure egui (eframe) | Bevy + bevy_egui |
|---|---|---|
| **2D rendering** | egui::Painter (CPU tessellation each frame) | Bevy Mesh/Sprite (GPU-resident, batched) |
| **Pan/zoom** | Manual transform math or Scene container | Bevy Camera2d + OrthographicProjection (native) |
| **Hit testing** | Custom math per element type | Bevy MeshPickingPlugin or bevy_mod_picking (built-in) |
| **3D viewer** | Not possible — need separate wgpu integration | Same app, same window — just add Camera3d |
| **ECS data model** | Manual state management (AppState struct) | Entities with Components — scalable, queryable, parallelized |
| **Large schematics** | Degrades — CPU re-tessellates everything per frame | GPU-batched — thousands of entities at 60fps |
| **Waveform rendering** | WGPU callback injection (works but bolted on) | Native Bevy mesh with custom shader |
| **PCB copper pour** | Would need raw wgpu code | Bevy mesh system handles complex polygons |
| **Simulation integration** | Background thread + request_repaint() | Bevy async tasks + ECS event system |

### The Key Insight

egui is an **immediate-mode UI toolkit** — excellent for panels, forms, and toolbars. But Signex's core is a **2D/3D graphics application** that needs a proper rendering engine. Bevy provides that engine, while bevy_egui lets us keep egui for the UI chrome. Best of both worlds.

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
│  │ egui StatusBar                                 │ │
│  └────────────────────────────────────────────────┘ │
│  egui MenuBar (top)                                 │
└─────────────────────────────────────────────────────┘
```

- egui panels use `SidePanel`, `TopBottomPanel`, `Window`
- Bevy viewport fills the `CentralPanel` remainder
- bevy_egui handles input routing: clicks on egui panels go to egui, clicks on viewport go to Bevy
- Both share the same wgpu surface — zero overhead for compositing

## Bevy ECS Architecture for Schematics

### Entity-Component Mapping

Every schematic element becomes a Bevy Entity with typed Components:

```rust
// Components
#[derive(Component)] struct SchematicWire { start: Vec2, end: Vec2 }
#[derive(Component)] struct SchematicSymbol { lib_id: String, reference: String, value: String }
#[derive(Component)] struct SchematicJunction;
#[derive(Component)] struct SchematicLabel { text: String, label_type: LabelType }
#[derive(Component)] struct SchematicPin { name: String, number: String, pin_type: PinType }
#[derive(Component)] struct Selected;        // Marker component
#[derive(Component)] struct Hoverable;       // Can be hovered/picked
#[derive(Component)] struct NetId(String);   // Net membership

// Bevy built-in components used:
// Transform — position, rotation, scale
// Visibility — show/hide
// Mesh2d + MeshMaterial2d — rendered shape
```

### Systems (replace SchematicRenderer.tsx)

```rust
// Rendering systems (run every frame)
fn render_grid(gizmos: Gizmos, camera: Query<&OrthographicProjection>) { ... }
fn render_wires(query: Query<(&SchematicWire, &Transform, Option<&Selected>)>, gizmos: Gizmos) { ... }
fn render_selection_overlay(query: Query<&Transform, With<Selected>>, gizmos: Gizmos) { ... }

// Input systems
fn handle_keyboard(keys: Res<ButtonInput<KeyCode>>, ...) { ... }
fn handle_mouse_click(picks: Query<&PickingInteraction>, ...) { ... }
fn handle_pan_zoom(pancam: Query<&mut PanCam>, ...) { ... }

// Logic systems
fn auto_junction_detection(...) { ... }
fn electrical_snap(...) { ... }
fn wire_drawing_mode(...) { ... }
```

### Advantages Over Current Architecture

1. **Parallel systems**: Bevy automatically parallelizes non-conflicting systems. Rendering, hit-testing, and ERC can run concurrently
2. **Spatial queries**: Bevy's query system replaces manual iteration over arrays
3. **Entity lifecycle**: Spawn/despawn replaces array push/splice for adding/removing elements
4. **Change detection**: `Changed<T>` filter — only process entities that actually changed (replaces dirty flags)
5. **Events**: `EventWriter<T>` / `EventReader<T>` for decoupled communication (replaces callback chains)

## 2D Shape Rendering Strategy

### bevy_prototype_lyon (Vector Shapes)

For symbol graphics (polylines, rectangles, circles, arcs):

```rust
// Rectangle
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

// Polyline (symbol outline)
let mut path_builder = PathBuilder::new();
path_builder.move_to(points[0]);
for p in &points[1..] { path_builder.line_to(*p); }
commands.spawn((ShapeBundle { path: path_builder.build(), ..default() }, Stroke::new(color, width)));

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

### Bevy Gizmos (Lightweight Overlays)

For grid, selection box, crosshair cursor, wire preview:

```rust
fn render_grid(mut gizmos: Gizmos, camera: Query<&OrthographicProjection>) {
    // Grid lines — drawn every frame, zero allocation
    for x in grid_range_x {
        gizmos.line_2d(Vec2::new(x, min_y), Vec2::new(x, max_y), GRID_COLOR);
    }
}

fn render_selection_box(mut gizmos: Gizmos, drag: Res<DragState>) {
    if let Some(rect) = drag.selection_rect() {
        gizmos.rect_2d(rect.center(), rect.size(), SELECTION_COLOR);
    }
}

fn render_crosshair(mut gizmos: Gizmos, cursor: Res<WorldCursor>) {
    gizmos.line_2d(Vec2::new(cursor.x - 3.0, cursor.y), Vec2::new(cursor.x + 3.0, cursor.y), CURSOR_COLOR);
    gizmos.line_2d(Vec2::new(cursor.x, cursor.y - 3.0), Vec2::new(cursor.x, cursor.y + 3.0), CURSOR_COLOR);
}
```

### Text Rendering

```rust
// Symbol reference designator
commands.spawn((
    Text2d::new("R1"),
    TextFont { font_size: 1.27, ..default() },
    TextColor(Color::hex("e8c66a")),
    Transform::from_translation(Vec3::new(x, y, Z_TEXT)),
));
```

## Camera & Viewport

### bevy_pancam for Pan/Zoom

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

### Viewport Restriction (Don't Render Behind egui Panels)

```rust
fn update_viewport(
    mut camera: Query<&mut Camera>,
    egui_ctx: Query<&EguiContext>,
) {
    // bevy_egui reports how much space egui panels occupy
    // Adjust camera viewport to the remaining area
    let available = egui_ctx.single().available_rect();
    camera.single_mut().viewport = Some(Viewport {
        physical_position: UVec2::new(available.min.x as u32, available.min.y as u32),
        physical_size: UVec2::new(available.width() as u32, available.height() as u32),
        ..default()
    });
}
```

## Hit Testing / Picking

### Bevy Built-in Picking (0.15+)

```rust
app.add_plugins(MeshPickingPlugin);

// Make entities pickable
commands.spawn((
    Mesh2d(mesh_handle),
    MeshMaterial2d(material_handle),
    PickableBundle::default(),  // Makes this entity clickable
    SchematicWire { start, end },
));

// React to picks
fn handle_selection(
    mut picks: EventReader<Pointer<Click>>,
    mut commands: Commands,
) {
    for event in picks.read() {
        let entity = event.target;
        commands.entity(entity).insert(Selected);
    }
}
```

For wire segments (line picking with tolerance), add invisible mesh strips along wire paths as pick targets.

## 3D PCB Viewer (Future — Already Built Into Bevy)

```rust
// Same app, just add a 3D camera and meshes
fn setup_3d_view(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 50.0, 100.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // PCB board as a box mesh
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(100.0, 1.6, 80.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::hex("1a5c1a"),
            ..default()
        })),
    ));

    // Components as 3D meshes, traces as flat quads on the board surface
    // STEP file import via bevy_stl or custom loader
}
```

No separate rendering engine needed — Bevy handles 2D schematics and 3D PCB views in the same application.

## Comparison With Alternatives Considered

### vs Slint
Slint has no custom rendering canvas, no 3D support, GPL license. Not viable for an EDA tool.

### vs Pure egui (eframe)
egui alone handles UI well but lacks a proper 2D/3D rendering engine. Would require bolting on raw wgpu for schematics and PCB. Bevy provides this natively with batching, spatial queries, and an ECS data model that fits EDA entities.

### vs Current Stack (Tauri + React + Canvas2D)
Canvas2D hits a wall at PCB complexity. No 3D. IPC serialization boundary wastes CPU. Two-process architecture. Bevy + bevy_egui is a single Rust process with GPU rendering.

## Key Dependencies

```toml
[dependencies]
bevy = "0.15"
bevy_egui = "0.35"
bevy_prototype_lyon = "0.13"   # 2D vector shapes (lyon tessellation)
bevy_pancam = "0.14"           # Pan/zoom camera controls
rfd = "0.15"                   # Native file dialogs
serde = "1"                    # Serialization
serde_json = "1"               # JSON
arboard = "3"                  # Clipboard
tokio = "1"                    # Async (file I/O, ngspice)
uuid = "1"                     # UUIDs
egui_dock = "0.15"             # Dockable panels (works with bevy_egui)
```
