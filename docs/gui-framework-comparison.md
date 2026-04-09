# GUI Framework Comparison: egui vs Slint for Signex

## Context

Signex is an AI-First EDA tool (Altium-class schematic/PCB editor) currently built on Tauri + React + Canvas2D, with plans for wgpu rendering, ngspice simulation integration, and waveform visualization. This document compares egui and Slint as potential Rust-native GUI frameworks.

## Summary Table

| Dimension | egui | Slint | Better for Signex |
|---|---|---|---|
| **Rendering approach** | Immediate mode, CPU tessellation + GPU raster (wgpu/glow) | Retained mode, GPU-accelerated (Skia/FemtoVG/software) | Depends on use case |
| **Custom waveform rendering** | Direct WGPU render pass injection; Painter API; Scene pan/zoom | Image-buffer pattern; Path element; no GPU canvas callback | **egui** |
| **Large dataset plotting** | egui_plot for <50K pts; custom WGPU path for 1M+ pts | Image-buffer regeneration on each viewport change | **egui** |
| **Standard UI performance** | 1-2ms/frame typical; can degrade with huge layouts | Skia backend: ~2% CPU; reactive dirty-region updates only | **Slint** |
| **Cross-platform** | Windows, macOS, Linux, Web, Android | Windows, macOS, Linux, Web, Embedded | Tie |
| **Native look-and-feel** | Not a goal; consistent custom style | Fluent/Material/Cupertino themes | **Slint** |
| **API stability** | Breaking changes between releases | Stable 1.x API with backward compatibility | **Slint** |
| **Community/ecosystem** | ~22K stars, 14M+ downloads, large third-party crate ecosystem | ~18.5K stars, 195+ contributors, commercial backing | Tie |
| **Licensing** | MIT / Apache-2.0 (fully permissive) | Royalty-free for desktop, GPLv3, or commercial license | **egui** |
| **Hot reload** | Not built-in | First-class live preview of .slint files | **Slint** |
| **Developer ergonomics** | Pure Rust, no DSL, low learning curve | .slint DSL + Rust bindings, visual tooling, steeper curve | **egui** |
| **Backend thread integration** | Send+Sync context, request_repaint() from any thread | invoke_from_event_loop(), !Send components | **egui** |
| **Accessibility** | AccessKit (Windows/macOS) | AccessKit (Windows/macOS), Qt backend on Linux | Tie |

---

## 1. Performance

### egui (Immediate Mode)

- Redraws entire UI from scratch every frame; typical frame takes 1-2ms on CPU
- Only repaints on user interaction or active animation (idle CPU usage is low)
- Rendering backends: `egui-wgpu` (WebGPU, GPU-accelerated) and `egui_glow` (OpenGL)
- Layout and tessellation happen on CPU every frame; `rayon` feature enables parallel tessellation
- Version 0.31+ improved tessellator to produce fewer vertices and less overdraw
- **Risk**: `egui_plot` struggles with >100K points due to per-frame CPU tessellation. For million-point waveforms, use `egui_wgpu_plot` (GPU-side vertex buffers that only update on data change)

### Slint (Retained Mode)

- Declarative `.slint` markup compiled ahead of time; UI components laid out in contiguous memory
- Reactive property system: only changed parts of UI are re-evaluated
- Three rendering backends:
  - **Skia** (recommended for desktop): OpenGL, Vulkan, Metal, Direct3D. ~2% CPU for typical UIs vs ~30% with FemtoVG for animated demos
  - **FemtoVG**: Lightweight, OpenGL ES 2.0 or WGPU. Aimed at embedded/WASM
  - **Software renderer**: Pure CPU, <300 KiB RAM, for embedded targets
- Natural advantage for complex, mostly-static UIs with occasional updates

### Verdict

For Signex's data-heavy waveform views with constantly changing simulation data, egui's immediate mode is a natural fit. For the standard UI panels (properties, component browser, project navigator), Slint's retained mode would be more efficient.

---

## 2. Custom 2D Rendering (Critical for EDA)

This is the **most important factor** for Signex. Schematics, waveforms, PCB layouts, and net highlighting are all custom 2D canvases.

### egui

- **Painter API**: `egui::Painter` provides direct access to draw lines, circles, rectangles, Bezier curves, and arbitrary `Shape` objects anywhere in the UI
- **Scene container** (v0.31+): A pannable, zoomable canvas that can contain widgets and child UIs - directly useful for a waveform viewer
- **egui_plot**: Dedicated 2D plotting widget (line, scatter, bar). Works well for <50K points; bottleneck beyond that
- **WGPU callback**: Inject a custom WGPU render pass into any UI region. Write fully custom GPU shaders for million-point waveforms with persistent vertex buffers. `egui_wgpu_plot` demonstrates this pattern
- **Recommended architecture**: egui for standard UI (toolbars, menus, panels, property editors) + custom WGPU render pass for the waveform/schematic canvas

### Slint

- **Path element**: `.slint` markup supports `Path` with MoveTo, LineTo, ArcTo, CubicTo, Close. Suitable for static/semi-static vector graphics
- **Image-buffer pattern**: Render plots in Rust (e.g., via `plotters` crate) into a pixel buffer, wrap in `slint::Image`, display in `Image` element. CPU-to-GPU copy on every data/viewport change
- **No direct GPU canvas API**: No equivalent to egui's WGPU callback. Open GitHub discussions requesting this (#676, #1080). The image-buffer workaround is the only option
- Ships a plotter demo using the image-buffer pattern

### Verdict

**egui wins by a wide margin.** The ability to inject custom WGPU render passes directly into the UI is architecturally ideal for waveform visualization. Slint's image-buffer approach adds latency and CPU overhead on every viewport change, which is problematic for interactive pan/zoom of large waveforms.

---

## 3. Cross-Platform Support

Both cover Windows, macOS, and Linux well. egui additionally targets Android (via eframe) and has experimental iOS support. Slint additionally targets embedded microcontrollers (STM32, RP2040, RPi) which is not relevant for Signex.

Both compile to WebAssembly for browser deployment.

**Native look-and-feel**: egui explicitly does not aim for native appearance. Slint ships Fluent, Material, and Cupertino themes. For an EDA tool, pixel-perfect native appearance is less important than functional polish, so this is not a deciding factor.

---

## 4. Licensing

| | egui | Slint |
|---|---|---|
| **License** | MIT / Apache-2.0 | Royalty-free (desktop), GPLv3, or commercial |
| **Commercial use** | Unrestricted | Free for desktop via royalty-free license |
| **Open source** | Fully permissive | Must be GPLv3 if using free license for open-source |
| **Future risk** | None (permissive, irrevocable) | Dependent on SixtyFPS GmbH maintaining royalty-free tier |

**egui wins.** MIT/Apache-2.0 is simpler, more permissive, and has zero ambiguity for any use case.

---

## 5. Ecosystem & Maturity

### egui

- ~22K GitHub stars, 14M+ crates.io downloads
- Actively developed; sponsored by Rerun (funded startup using egui in production for multimodal data visualization)
- Rich third-party ecosystem: `egui_plot`, `egui_dock` (dockable panels), `egui_extras`, `egui-notify`, `egui_graphs`
- API still evolving with breaking changes between releases

### Slint

- ~18.5K GitHub stars, 195+ contributors
- Backed by SixtyFPS GmbH with commercial customers in embedded/automotive
- Stable 1.x API with backward compatibility guarantees
- Multi-language: Rust, C++, JavaScript, Python
- Tooling: VS Code extension with live preview, Figma plugin, SlintPad online editor
- Smaller third-party ecosystem

---

## 6. Developer Experience

### egui

- **Pure Rust**: No DSL, no macros, no separate markup files. `ui.label("Hello")` and `ui.button("Click")` directly in Rust
- **Hot reload**: Not built-in (community solutions exist via `hot-lib-reloader`)
- **Learning curve**: Very low for Rust developers
- **Compilation**: Fast by Rust standards, targets 60Hz in debug builds

### Slint

- **Declarative DSL**: `.slint` markup language for UI, Rust for business logic
- **Hot reload**: First-class. VS Code extension provides live preview that updates as you edit `.slint` files
- **Learning curve**: Moderate (new DSL + bridging between .slint and Rust)
- **Tooling**: VS Code extension with autocomplete, go-to-definition, syntax highlighting, and visual live preview

For Signex, where most complexity is in custom rendering (not standard UI layouts), egui's "everything in Rust" approach is more productive overall.

---

## 7. Integration with ngspice / Heavy Backends

### egui

- Heavy computation runs on background threads
- Standard Rust patterns: `mpsc::channel`, `Arc<Mutex<T>>`, tokio on separate thread
- `egui::Context` is `Send + Sync`: worker threads call `ctx.request_repaint()` to wake UI when new data arrives
- Immediate mode naturally picks up new state next frame
- **Precedent**: Rerun Viewer handles streaming multimodal data with this exact pattern

### Slint

- `slint::invoke_from_event_loop()` lets background threads schedule closures on UI thread
- Slint components are `!Send` (cannot be moved across threads)
- All UI updates must go through `invoke_from_event_loop`, adding indirection
- **Precedent**: `cargo-ui` uses tokio on a separate thread communicating with Slint UI

**egui is slightly more ergonomic** for the "simulation thread pushes waveform data to UI" pattern, thanks to `Send + Sync` context and direct `request_repaint()`.

---

## Recommendation

### For Signex: **egui**

1. **Custom GPU rendering is the killer feature.** An EDA waveform viewer's core is a high-performance, interactive, zoomable display of potentially millions of data points. egui's WGPU render pass injection is architecturally ideal. Slint's image-buffer workaround adds unacceptable latency for smooth pan/zoom.

2. **The Rerun precedent.** Rerun Viewer is a production application on egui handling exactly this class of problem: streaming large-scale time-series and multidimensional data visualization with interactive scrubbing. The Rerun team achieved 30-120x rendering speedups through sub-pixel aggregation, GPU-side transforms, and query caching.

3. **Simpler backend integration.** `Send + Sync` egui context and `request_repaint()` from worker threads maps cleanly to the "simulation thread pushes new waveform data to UI" pattern.

4. **Licensing simplicity.** MIT/Apache-2.0 gives zero constraints.

5. **Ecosystem for EDA.** `egui_dock` for dockable panels, `egui_plot` for quick plotting, custom WGPU for high-performance rendering.

### When Slint would be preferable

If Signex were primarily a forms-and-panels UI with minimal custom rendering (e.g., standard widgets doing most of the work), Slint's retained-mode efficiency, live preview tooling, and native styling would provide a better developer experience.

### Consideration: Current Architecture

Signex currently uses Tauri + React + Canvas2D, which is already functional. Before switching to either framework, evaluate whether the current stack has actual problems that need solving, or whether the better path is to evolve the existing architecture (e.g., add wgpu rendering to the Tauri shell for the PCB phase, as the current roadmap plans).
