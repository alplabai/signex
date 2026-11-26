# Performance measurement

How to measure Signex's frame rate on a real machine. These are dev/QA
tools: they add no menu, button or setting, and a normal launch never runs
them.

Other benches, all run from `cargo test`:

- PCB render baseline and the GPU frame bench on a 10,000-track board:
  `docs/architecture/pcb-render-bench.md`.
- Schematic render bench: `docs/architecture/schematic-render-bench.md`.
- Router soak and bench: "Router soak and bench" at the end of this page.

## Pan/zoom bench in the real app (#2472)

The PCB plan's exit criterion is 60 fps pan and zoom on the 10,000-track
board. The `cargo test` benches time frames offscreen; this mode times them in
the shipped binary, in its own window, through the real present path.

### Switches

Environment variables, read once at launch:

| Variable | Meaning | Default |
|---|---|---|
| `SIGNEX_BENCH_PAN_ZOOM` | The `.snxpcb` to open. Setting it turns the bench on. | unset: off |
| `SIGNEX_BENCH_OUT` | Where the per-frame CSV goes. | `<board>.pan-zoom.csv` beside the board |
| `SIGNEX_BENCH_SECONDS` | How long the timed run lasts, whole seconds above 0. | `20` |

A missing board file or a bad duration stops the launch with exit code 2 and
a line on stderr. When `SIGNEX_BENCH_PAN_ZOOM` is unset nothing below runs:
no subscription, no state, the launch is the normal one.

### What it does

1. Opens the board through the normal open flow (command-line documents
   are ignored).
2. Waits until the board is the active document and its view is laid out,
   fits the board to the view, then lets 60 untimed frames pass so the first
   upload and tessellation are not counted. If the board has not opened after
   120 s it gives up and writes a `FAILED` summary.
3. Runs a fixed script, one step per frame: zoom in 40 frames (x1.04 each),
   pan right two view widths (240 frames), down one view height (120), back
   left, back up, zoom out 40 frames. The cycle is 800 frames and ends where
   it started; it repeats until the time is up. Pans are fractions of the
   view, so the script does the same at any window size. Each step moves the
   board camera the way a right-button drag or a pinch does and then sends the
   same `ViewMoved` event the canvas sends.
4. Writes the CSV and the summary, prints the summary line on stdout and
   quits through the normal quit path.

The script and the maths are in `crates/signex-app/src/pan_zoom_bench/`; the
app side is `crates/signex-app/src/app/handlers/pan_zoom_bench.rs`. Headless
tests in `pan_zoom_bench/tests.rs` pin the script, the state machine and the
summary maths.

### What a frame time is

Each frame's start comes from iced's frame clock (the `RedrawRequested`
instant of the main window), and a frame's time is the gap to the frame
before. The clock is snapped to the display's refresh period and presenting
waits for vsync, so frame times are whole multiples of the refresh period:
16.67 ms on a 60 Hz panel, 8.33 ms on a 120 Hz one. A frame time of two
periods is a dropped frame. fps can therefore not exceed the refresh rate.

### Output

The CSV, one row per timed frame:

```text
frame,t_ms,frame_ms,action
0,16.667,16.667,zoom_in
```

`t_ms` is the frame's start from the start of the timed run, `frame_ms` the
gap to the frame before, `action` the script step the frame shows
(`zoom_in`, `pan_right`, `pan_down`, `pan_left`, `pan_up`, `zoom_out`).

The summary line, printed and saved as `<csv name>.summary.txt`:

```text
PAN-ZOOM-BENCH frames=1199 seconds=20.00 p50_ms=16.67 p95_ms=16.67 p99_ms=16.68 max_ms=33.34 fps=60.0 budget_60fps=ok mount=cpu msaa=x4 view=1544x861 adapter="AMD Radeon 880M (Vulkan)"
```

- Percentiles are nearest-rank over the frame times; `fps` is frames over
  the time they took.
- `budget_60fps` is `ok` when p95 is at most 16.67 ms.
- `mount` is the board mount Preferences selects (`cpu`: GPU render off,
  the default; `gpu`: on). `msaa` is the anti-aliasing preference. Change
  either in Preferences, quit, and run again to measure the other.
- `view` is the board view's size in logical pixels.
- `adapter` is the `GPU-ADAPTER-SELECTED` diagnostic: adapter name and
  backend.

### The 10,000-track board

The board is generated, not committed (about 1.3 MB). The generator is
`crates/signex-renderer/tests/integration/pcb_10k_track_fixture.rs`; write
it out with:

```text
SIGNEX_PCB_BENCH_OUT=/path/pcb-10k.snxpcb cargo test -p signex-renderer --test integration pcb_10k_write_snxpcb -- --ignored --nocapture
```

What the board holds: `docs/architecture/pcb-render-bench.md`, "The board".

### Reproduce on Windows

1. Build or download a release build of `signex.exe` (a debug build is far
   too slow to measure).
2. Put `pcb-10k.snxpcb` next to it.
3. In PowerShell, from that folder:

   ```powershell
   $env:SIGNEX_BENCH_PAN_ZOOM = "$PWD\pcb-10k.snxpcb"
   $env:SIGNEX_BENCH_OUT = "$PWD\pan-zoom.csv"
   $env:SIGNEX_BENCH_SECONDS = "20"
   Start-Process -FilePath .\signex.exe -Wait -NoNewWindow
   Get-Content .\pan-zoom.summary.txt
   Remove-Item Env:SIGNEX_BENCH_PAN_ZOOM, Env:SIGNEX_BENCH_OUT, Env:SIGNEX_BENCH_SECONDS
   ```

4. Do not touch the window while it runs; a resize or a move changes what
   is measured. Plug the laptop in, since battery power lowers GPU clocks.

On Linux or macOS: `SIGNEX_BENCH_PAN_ZOOM=pcb-10k.snxpcb ./signex`.

The bench uses the person's normal settings folder: opening the board adds
it to the recent files, as File > Open would.

## Router soak and bench (#2482, #2483)

The router's quality bar, from the PCB Routing exit criteria in
`docs/ROADMAP.md`: on every routing corpus board, 10,000 random routing and
editing actions with no panic, stuck history or corrupted board; shoves that
converge within 8 rounds on at least 95% of actions; a median action latency
of at most 16 ms and a 95th percentile of at most 33 ms on the median board.
What the soak and the bench do: `docs/PCB_TOOLS.md`, "Router soak and bench".
The code is `crates/signex-engine/tests/routing/`.

### Runs

| Run | Where | What is asserted |
|---|---|---|
| `soak_short`, `bench_short`, `a_soak_seed_replays_exactly` | every `cargo test`, the fixed boards | the soak invariants, the file round trip, the DRC delta, shove convergence |
| `soak_long` (ignored) | the weekly `routing-soak` workflow, every board | the soak invariants on 10,000 actions, the sampled DRC delta and round trip |
| `bench_thresholds` (ignored) | the weekly `routing-soak` workflow, every board | the DRC delta and shove convergence; latency only with `SIGNEX_ROUTING_ASSERT_LATENCY=1` |

Latency is asserted only in the scheduled job, which runs the bench alone on
the runner with one test thread. On a shared or debug machine the timings
are too noisy to fail on, so there the bench measures and reports them only.
The workflow is not a required check and never runs on a PR or a push.

Run the long versions locally in release:

```text
cargo test --release -p signex-engine --test routing -- --ignored --nocapture --test-threads 1
```

### Switches

| Variable | Meaning | Default |
|---|---|---|
| `SIGNEX_SOAK_SEED` | Seed of the soak and bench, decimal or `0x` hex. | `0x2482`; the workflow uses the run number |
| `SIGNEX_SOAK_ACTIONS` | Actions per board in `soak_long`. | `10000` |
| `SIGNEX_SOAK_BOARD` | One board by name, searched in every board. | unset: every board |
| `SIGNEX_SOAK_DRC_EVERY` | `soak_long` checks the DRC delta of every n-th route; `0` never. | `25` |
| `SIGNEX_SOAK_ROUND_TRIP_EVERY` | `soak_long` checks the file round trip every n actions and after the last; `0` after the last only. | `1000` |
| `SIGNEX_SOAK_JOBS` | Boards `soak_long` soaks side by side. | the machine's cores |
| `SIGNEX_BENCH_ACTIONS` | Route actions per board in `bench_thresholds`. | `300` |
| `SIGNEX_ROUTING_ASSERT_LATENCY` | `1` fails `bench_thresholds` on the latency thresholds. | unset: reported only |
| `SIGNEX_ROUTING_REPORT` | File the report lines are appended to. | unset: printed only |
| `SIGNEX_REQUIRE_ROUTING_CORPUS` | `1` fails a run that finds a board left as a Git LFS pointer, or no boards. | unset: such a board is skipped |

An empty variable counts as unset. Every empty dispatch input reaches the tests
as an empty variable.

### Output

One line per board, printed and appended to `SIGNEX_ROUTING_REPORT`. The
scheduled job shows them in the run summary and uploads them, with the full
logs, as the artifact `routing-report-<run number>`.

```text
ROUTING-SOAK board=odrive-v3 seed=0x2a actions=10000 result=ok landed=2210 refused=771 edits=2381 undos=1290 redos=601 drc_checks=88 round_trips=10 seconds=412.7
ROUTING-BENCH board=odrive-v3 seed=0x2a actions=300 copper=4120 legs_landed=211 legs_refused=89 shoves=151 converged_pct=97.4 rounds_limit=8 cleared=96 max_rounds=11 p50_ms=6.12 p95_ms=21.40 latency_budget=ok breaks=0
ROUTING-BENCH-MEDIAN board=odrive-v3 p50_ms=6.12 p95_ms=21.40 latency_budget=ok latency_asserted=yes
```

The numbers above show the format only. They are not measurements.

- `ROUTING-SOAK`: `result` is `ok` or `FAILED`. A failed line has
  `at_action` and `action` in place of the counts and is followed by the
  failure and the command that replays it. `landed` and `refused` count
  route actions, and `edits` counts moves and deletes. `drc_checks` and
  `round_trips` count the costlier checks that ran. A `round_trips` of 0 means
  the board did not round-trip as loaded, so the soak could not check it.
  `seconds` is the board's wall time.
- `ROUTING-BENCH`: `legs_landed` and `legs_refused` count the routed legs.
  `shoves` counts the shove actions that tried a push, and `converged_pct` is
  the share of them that settled within `rounds_limit` rounds. `cleared`
  counts the shoves that cleared the way, and `max_rounds` is the most rounds
  any shove took. `p50_ms` and `p95_ms` are nearest-rank percentiles of the
  click-to-landed-step time per leg. `latency_budget` is `ok` when the median
  is at most 16 ms and p95 is at most 33 ms. It is reported for every board,
  but asserted only on the median board. `breaks` counts routes whose DRC
  delta was off the route's own copper, or that the router itself refused to
  land.
- `ROUTING-BENCH-MEDIAN`: the median board by copper count, which the
  latency thresholds apply to. `latency_asserted` says whether this run
  failed on them.
