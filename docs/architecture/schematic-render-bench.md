# Schematic render paths, measured (#2040)

The iced canvas (`canvas::Program`, `geometry::Cache`, lyon on the CPU) against
the GPU mount (`SchematicSurface` through `scene_shader.rs`, #199). Both stay;
this records what each costs. Measured on an Apple M2 Pro (Metal), release
build, 2026-11-06, trunk `685badd3b`.

## Decision

Hakan, 2026-11-06: the schematic moves to the GPU path (#199), and the canvas
stays the default until it does. The blocker for both paths is rotated text
(finding 1 below), tracked in signex #2128 and alplabai/iced#38. Direction, per
the #2022 Lyon decision: rotated glyphs come from Lyon outlines tessellated
once and cached per (font, glyph, tolerance bucket) in glyph-local coordinates,
with only the transform applied per frame (iced#38 step 2 plus the outline cache
of iced#21). signex-gfx draws those cached meshes instanced; axis-aligned text
stays on the atlas. Rotating bitmap glyphs in the atlas is not the direction.
Nothing is deleted. The row is in `docs/DECISIONS.md`.

## Rerun

```text
bash scripts/bench_schematic_render.sh out.log          # one process per path / MSAA / view
SIGNEX_BENCH_SIZE=medium|small  SIGNEX_BENCH_DPR=2  SIGNEX_BENCH_VARIANT=norot|notext
SIGNEX_BENCH_AAS="off msaa4"  SIGNEX_BENCH_AUX=0|1  SIGNEX_BENCH_GRID=0
cargo test -p signex-app --release --lib render_bench -- --ignored --nocapture --test-threads=1   # one process
```

The harness is `crates/signex-app/src/canvas/render_bench_tests.rs` (an
`#[ignore]`d test; `pan_bench_tests.rs` is the older #2102 pan bench). Release
needs `library/editor/inert.rs`'s tests gated on `debug_assertions`, because
iced's `()` renderer only exists in debug builds.

## Method

- **Sheet.** Generated, deterministic (uuids count up). 24 x 20 cells of 60 x 40
  mm: 2400 symbols (480 8-pin ICs, 1920 resistors), 7680 pins
  with names and numbers, 7680 wires, 1920 junctions, 1920 net labels (half
  turned 90 degrees), 480 text notes, reference and value on every symbol. No
  corpus file comes close: the largest is `linear-regulator.snxsch` (28 KB).
  Variants: `full` (as above), `norot` (no text turned 90 degrees), `notext`.
- **Window.** 1600 x 1300 logical, dpr 1 (dpr 2 below). Views: `fit` (whole
  sheet, 0.97 x fit scale) and `work` (5.6 px/mm, 148%). The scene and the
  retained canvas layer are built over one viewport of margin on every side, so
  `work` still lowers about half the sheet (12 244 texts, 8211 lines).
- **Code path.** The real widgets: `UserInterface::build`, `update` (the drag is
  real `CursorMoved`/`ButtonPressed` events), `draw`, then
  `iced_wgpu::Renderer::screenshot`, which uploads, renders and reads back. So
  GPU work is inside the timing. Never tiny-skia.
- **Floor.** An empty sheet through the same stack: 2.0 ms at dpr 1, 6.7 ms at
  dpr 2. It is the readback plus the empty backdrop (paper, grid, frame), not
  the sheet; read every number net of it. No
  vsync or surface present is measured.
- **Interactions.** Pan, pan-frac and zoom set the camera directly through
  `slot.camera_mut()`, not through the app's update path; only the drag sends
  real events. `cold` fresh slot and tree; `idle` same camera, cache hit;
  `pan` whole-pixel steps (6, 4 px) over 60 frames; `pan-frac` 6.37, 4.21 px
  steps (no retained reuse); `zoom` 30 steps of x1.05 about the window centre
  (a cache miss each); `drag` press on a selected wire, then 4, 2 px per frame
  with 270 items selected (45 symbols and 144 wires move). MSAA is
  `Antialiasing::MSAAx4` on the engine.
- **Stats.** Median / p95 of 20 to 60 frames after 3 warm-up frames
  (`cold`: 5). Memory is the `footprint` of the process (unified memory, so
  GPU buffers are in it) and the peak RSS; a process per case. Metal's own
  allocated size needs an `objc2-metal` dependency and was not taken.

## What the numbers say

### 1. Rotated text decides everything

Text turned 90 degrees (vertical pin names, net labels) cannot use the glyph
atlas. On the canvas it costs about **0.14 ms per text per frame, even when the
geometry is cached**; the GPU mount draws it on a CPU canvas layer
(`SchematicStaticTextLayer`) that rebuilds on every camera change at the same
cost. (Measured before #2128. Since then the canvas draws atlas text before
rotated text, so a cached frame no longer pays per rotated text, and the GPU
mount's text layer is retained across whole-pixel pans; the numbers are in
#2128.)

| `full`, 960 rotated texts, total ms median / p95 | idle | pan | zoom |
|---|---|---|---|
| canvas, `fit` | 135 / 147 | 145 / 159 | 308 / 318 |
| gpu mount, `fit` | 7.0 / 7.2 | 145 / 148 | 149 / 159 |
| canvas, `work` | 56 / 57 | 59 / 60 | 37 / 117 |
| gpu mount, `work` | 2.6 / 2.7 | 76 / 78 | 22 / 65 |

Attribution on the `full` sheet, `work`, pan: backdrop 2.0 ms, the two shaders
2.5, overlay 2.0, **rotated-text layer 74.9**. Medium (240 rotated) is 27 ms
idle on the canvas, small (60 rotated) 8 ms: linear. Canvas + MSAA on this
sheet at `fit` loses the Metal device (`Device is lost`, first frame); the
GPU mount does not.

### 2. Without rotated text (`norot`), `work` view, dpr 1, total ms median / p95

| path | MSAA | cold | idle | pan | pan-frac | zoom | drag | footprint |
|---|---|---|---|---|---|---|---|---|
| canvas | off | 17.8 / 17.9 | 2.1 / 2.3 | 3.5 / 3.6 | 9.7 / 10.0 | 9.2 / 16.8 | 3.2 / 3.4 | 462 MB |
| canvas | 4x | 18.2 / 18.4 | 2.3 / 2.5 | 3.8 / 3.9 | 10.0 / 10.4 | 9.6 / 17.1 | 3.5 / 3.8 | 531 MB |
| gpu mount | off | 4.2 / 4.2 | 2.6 / 2.8 | 2.8 / 2.9 | 2.7 / 2.9 | 7.3 / 10.0 | 3.5 / 3.8 | 529 MB |
| gpu mount | 4x | 4.5 / 4.6 | 2.8 / 2.9 | 3.0 / 3.3 | 3.0 / 3.3 | 7.6 / 10.1 | 4.1 / 4.5 | 583 MB |

`fit` view, same variant:

| path | MSAA | cold | idle | pan | pan-frac | zoom |
|---|---|---|---|---|---|---|
| canvas | off | 33.0 / 33.1 | 2.2 / 2.4 | 6.6 / 6.8 | 19.5 / 20.5 | 32.1 / 35.7 |
| canvas | 4x | 33.1 / 34.2 | 2.4 / 2.7 | 6.8 / 7.1 | 19.6 / 20.0 | 32.3 / 34.7 |
| gpu mount | off | 11.8 / 11.9 | 7.3 / 7.6 | 8.7 / 9.4 | 9.0 / 9.3 | 18.1 / 35.1 |
| gpu mount | 4x | 11.7 / 12.5 | 7.8 / 8.7 | 8.6 / 9.4 | 8.7 / 9.2 | 17.8 / 35.6 |

- MSAA 4x adds 0.2 to 0.4 ms at `work` on both paths. It is not the issue.
- The retained layers work: a whole-pixel pan tessellates once for 60 frames
  (canvas `draw` 0.02 ms), and a drag builds its static layer once. Both paths
  are equal on drag (3.2 vs 3.5 ms), because #1335's static/moving split is on
  the canvas too.
- What the canvas pays that the GPU path does not: **a cache miss**. Tessellating
  the 3x3-viewport clip rectangle costs 6.2 ms at `work` and 12.7 ms at `fit`
  on every zoom step and every pan that is not a whole device pixel; the GPU
  scene build for the same step is 1.3 and 2.0 ms (the rest of its zoom frame is
  the upload, 3 to 5 ms).
- What the GPU path pays that the canvas does not: 22 560 glyphs prepared per
  frame at `fit` (idle 7 ms vs 2 ms), and a backdrop that is not retained
  (1.3 ms at `fit` on every camera move; the canvas retains it).
- `notext`: canvas `work` is cold 8.7, idle 2.1, pan 2.3, pan-frac 8.0, zoom
  4.6, drag 2.9 ms; GPU 2.8, 2.1, 2.2, 2.1, 3.1, 2.7. Text, not geometry, is
  the cost on the GPU path's `fit` numbers.

### 3. Retina (dpr 2), `norot`, `work`, floor 6.7 ms

canvas: idle 7.1, pan 8.7, pan-frac 15.1, zoom 13.5 / 21.3, drag 8.3, cold 22.9.
gpu mount: idle 7.5, pan 8.0, pan-frac 8.0, zoom 12.4 / 15.9, drag 8.8, cold 8.9.
Net of the floor the picture of section 2 holds. `fit` canvas pan-frac 24.0 and
zoom 35.9; gpu 14.4 and 21.9 / 40.

### 4. Memory

Footprint of a process, baseline after an empty sheet 240 to 300 MB at dpr 1.
`norot` and `notext`: canvas 460 to 620 MB, gpu mount 490 to 700 MB, no
difference beyond noise. `full`: canvas **3.3 GB (`work`) to 5.7 GB (`fit`)**
(rotated text geometry), gpu mount 0.2 to 0.65 GB (the fallback layer is not
retained). At dpr 2 the windowed surfaces dominate (1.0 to 2.4 GB for both).

## What the GPU path gives

- **Instancing / no re-tessellation on zoom:** scene build 1.3 ms (`work`) and
  2.0 ms (`fit`) against 6 to 15 ms of lyon, and no mesh to rebuild on a
  fractional pan. The p95 zoom frame is 10 ms against 17 ms at `work`.
- **Partial updates while dragging:** the moving set is rebuilt per frame and
  uploaded alone (static layer once, `drag_static_builds=1`). Equal to the
  canvas today, not better.
- **Rotated text:** nothing yet; the cached-outline direction above is what changes it. The fallback has the same per-text cost as the
  canvas, and loses the canvas's pan reuse (76 vs 59 ms on `full`).

## Not measurable here

Vsync, surface present and the compositor; a live trackpad pan (whether the
deltas are whole device pixels decides between `pan` and `pan-frac`); another
GPU, DX12 and Vulkan; rubber-banded wires (`move_keeps_wires`) are computed in
the app's update, not in the canvas, so the drag here moves the selection
without them. To measure live, run the app with `SIGNEX_FRAME_PROBE=1` (`frame_probe.rs`, spans `canvas`, `canvas.content`, ...) on a real large sheet and pan, zoom and drag by hand.
