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

The file is about a megabyte, so it is generated, not committed.

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
