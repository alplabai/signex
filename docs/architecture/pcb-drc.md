# PCB design-rule check (#2222)

`pcb_geom::run_drc(&PcbBoard, &DrcRules) -> DrcReport`: a read-only check of
a board in integer nm. Nothing in it edits the board, so it does not wait on
the document-hosting decision (#661).

## What it reports

| Kind | Meaning |
|---|---|
| `Clearance` | Copper of two different nets that share a copper layer and are closer than the rule. `measured_nm` is the gap, rounded down, always under `required_nm`. |
| `Short` | Copper of two different nets that touches or overlaps on a shared layer (`Shape::overlaps`). Reported once, never also as a clearance. |
| `Unconnected` | One violation per link `ratsnest` still draws. `measured_nm` is the link length, `required_nm` is 0, `items` are the pads at its ends. |

Each violation carries its items (sorted), the two nets, the shared layer
mask (`copper_layers` lists them), a location and the two distances. The
report is sorted, so a run is the same on every machine.

A gap equal to the rule passes; one nm under fails. The test is exact
(`Separation::is_under` compares squared nm); the only rounding is the foot
of a perpendicular on a diagonal track, under 1 nm (see `shape.rs`).

## Rules

`DrcRules::for_board(board, net_classes)`:

- the board-wide minimum is `PcbSetup.clearance` (mm, in the `.snxpcb`
  setup); **0 or absent means unset and the default 0.2 mm
  (`DEFAULT_CLEARANCE_MM`) applies**. No file format changed.
- a net class (`NetClass.clearance`, the project's net-class table) raises the
  rule for its nets: a pair of nets needs the largest of the board value and
  the two nets' class values. The class of a net is the first matching one,
  as everywhere else. A class value of 0 is unset.
- a value that is not a finite length is an error, not a silent default.

## How it stays fast

Candidate pairs come from the `BoardIndex` R-tree: one bounding-box query per
item, grown by the largest rule, then the exact test on the pairs it returns.
`crates/pcb-geom/tests/drc_corpus.rs` times a 10,000-track board in both a
clean and a 29,502-violation run and asserts a loose bound (10 s in debug CI;
a debug run takes about 0.35 s).

## In the app

`run_drc` (command catalog, group PCB, enabled with a PCB document) and the
Run DRC button in the Messages panel send `Message::Pcb(PcbMsg::DrcRequested)`.
The board is copied and checked in `spawn_blocking`; a result is applied only
if no newer run began. The Messages panel lists the violations (first 500)
above the log, and clicking a row selects its items with the #2191
selection. The summary and the first 20 violations are also logged, which is
what a headless run reports. A report is marked stale when the board's
revision moved after it.

## Not checked yet

Hole-to-hole, silk-to-pad, courtyard overlap, zone fill clearance (#2216),
annular ring, copper with no net (net 0 is skipped, like in the ratsnest),
and a rule per layer or per object kind. Each is its own follow-up.
