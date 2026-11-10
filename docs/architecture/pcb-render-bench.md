# PCB render baseline: a 10,000-track board (#2176)

The baseline for the 60 fps pan/zoom target of the PCB plan (P5). It measures
the CPU side of a frame: the board-to-scene emit and the viewport-culled upload.
GPU time, present and the compositor are not in it.

## Run

```text
cargo test -p signex-renderer --release --test integration pcb_10k -- --nocapture --test-threads=1
```

`--release` gives the numbers worth quoting; the same tests run in debug under
`cargo nextest` in CI with loose bounds. Each timed test prints one line:

- `pcb_10k emit (snapshot + scene)`: median of 7 runs of
  `PcbSnapshot::from_board` plus `PcbRenderer::build_scene` with lines, circles
  and polygons dirty, and the draw-call count.
- `pcb_10k cull (20 x 20 mm)`: median of 7 runs of
  `apply_dirty_uploads_with_culling` for a zoomed-in window, and what it kept.
- `pcb_10k snxpcb`: the size of the saved file, and its write and parse time.

To open the board in the app, write it out:

```text
SIGNEX_PCB_BENCH_OUT=/tmp/pcb-10k.snxpcb cargo test -p signex-renderer --test integration pcb_10k_write_snxpcb -- --ignored --nocapture
```

The file is about 1.3 MB, so it is generated, not committed.

## The board

`crates/signex-renderer/tests/integration/pcb_10k_track_fixture.rs`, generated
and deterministic (uuids count up per object kind, coordinates are whole
nanometres, so a save and reopen gives the same board back).

- 205 x 130 mm, four copper layers.
- 100 rows, one net each, of 100 segments: 10,000 tracks, 0.15 to 0.40 mm wide,
  horizontal runs joined by jogs every fifth segment. Rows cycle over all four
  layers.
- 1,000 through vias, one every tenth joint.
- 200 two-pad SMD parts, one at each end of every row: 400 pads.
- Two GND pours covering the board, on the inner layers.

## What the tests hold

- Counts: 10,000 lines, 1,000 circles, 400 top polygons (pads), 2 polygons
  (pours).
- Draw calls: 4 (pours, traces, vias, pads). The GPU path instances one draw per
  non-empty bucket, so the count does not grow with the board.
- Culling: a 20 x 20 mm window keeps between 100 and 400 of the tracks.
- Time: emit and cull each under 2 s. This bound only catches an
  order-of-magnitude regression (a quadratic pass, a per-track allocation
  storm); read the printed median for the baseline.

## GPU frames

The exit criterion "10k-track board at 60 fps", measured on a real GPU. The
emit and cull numbers above are CPU only; this bench times whole frames.

```text
cargo test -p signex-app --release --lib pcb_frame_bench -- --ignored --nocapture --test-threads=1
SIGNEX_BENCH_DPR=2  SIGNEX_BENCH_PATHS=cpu|gpu|cpu,gpu  SIGNEX_BENCH_AA=off|msaa4|off,msaa4
```

The harness is `crates/signex-app/src/pcb_canvas/frame_bench_tests.rs`, an
`#[ignore]`d test built like the schematic one
(`docs/architecture/schematic-render-bench.md`). It includes the board
generator above by `#[path]`, so both benches draw the same board.

- **Mount.** The stack `app/view/mod.rs` builds for a board: the `PcbCanvas`
  canvas (background, grid and, with GPU render off, the board), the
  `SceneShaderProgram<PcbSurface>` shader when GPU render is on, and the
  highlight layer. The active bar and the layer strip are left out. `cpu` is
  the default mount (GPU render off in Preferences), `gpu` the shader mount.
- **Frame.** `UserInterface::build`, `update` with the frame's mouse events, a
  second build when there were events (the app's `view` runs again after the
  canvas's message), `draw`, then `iced_wgpu::Renderer::screenshot`, which
  uploads, renders and reads back. GPU time is inside the frame; vsync and
  surface present are not. Never tiny-skia.
- **Window.** 1600 x 1300 logical, dpr 1 (`SIGNEX_BENCH_DPR` overrides).
- **Snapshot.** `PcbSnapshot::from_board_with_view` with the default layer
  view, no ratsnest and no DRC markers.

Scenarios, each after three untimed warm-up frames:

1. `idle`: the whole board fitted, 60 frames with no input.
2. `pan`: a right-button drag from the window centre, 6, 3 px per frame over
   60 frames. The press and release frames are not timed.
3. `zoom`: 15 wheel notches in about the window centre, then 15 out (30
   frames), from the board fitted.
4. `zoom-pan`: a 20 x 20 mm patch in the middle of the board fitted (the zoom
   limit, 48 px/mm, clamps it, so the window shows about 33 x 27 mm), then the
   same 60-frame drag.

Each line gives the frame count, median, p95 and max frame time, the fps the
median and the p95 imply, and `ok` when p95 is at most 16.7 ms (`OVER`
otherwise), then median/p95 per phase. The test does not assert the 60 fps
bound, since machines differ. It asserts that the board has its counts, that
each frame reads back a full window that is not a flat fill, that each
scenario timed every frame, and that the drags and the zoom moved the camera
as far as the input says.

Results (fill in from a run; adapter name and backend from the
`BENCH adapter` line):

| Adapter (backend) | Mount | AA | Scenario | Median ms | p95 ms | Max ms | fps (median) | ok |
|---|---|---|---|---|---|---|---|---|
| | cpu | off | idle | | | | | |
| | cpu | off | pan | | | | | |
| | cpu | off | zoom | | | | | |
| | cpu | off | zoom-pan | | | | | |
| | gpu | off | idle | | | | | |
| | gpu | off | pan | | | | | |
| | gpu | off | zoom | | | | | |
| | gpu | off | zoom-pan | | | | | |
| | cpu | msaa4 | idle | | | | | |
| | cpu | msaa4 | pan | | | | | |
| | cpu | msaa4 | zoom | | | | | |
| | cpu | msaa4 | zoom-pan | | | | | |
| | gpu | msaa4 | idle | | | | | |
| | gpu | msaa4 | pan | | | | | |
| | gpu | msaa4 | zoom | | | | | |
| | gpu | msaa4 | zoom-pan | | | | | |

## Realistic boards

The 10,000-track board measures scale; it is not a design. Five complete
small boards (a linear regulator, a USB-C power input, a TQFP-32 breakout, a
four-layer sensor board with inner planes, a mixed through-hole board with a
board sketch) live in `crates/signex-types/tests/corpus/pcb/real/`.
`crates/signex-renderer/tests/integration/pcb_real_boards.rs` renders each and
pins its drawn counts and a geometry hash; DRC and the ratsnest run on them in
`crates/pcb-geom/tests/real_boards.rs`. What each board covers: the corpus
README, "Realistic boards".
