# PCB design-rule check (#2222)

`signex_drc::run(&PcbBoard, &DrcRules) -> DrcReport`: a read-only check of
a board in integer nm. Nothing in it edits the board, so it does not wait on
the document-hosting decision (#661).

The check lives in its own domain crate, `crates/signex-drc` (#2307), the
DRC twin of `signex-erc`. It measures with `pcb-geom` (index, shapes,
distances, copper layers, ratsnest); `pcb-geom` does not depend on it.

## What it reports

| Kind | Meaning |
|---|---|
| `Clearance` | Copper of two different nets that share a copper layer and are closer than the rule. `measured_nm` is the gap, rounded down, always under `required_nm`. |
| `Short` | Copper of two different nets that touches or overlaps on a shared layer (`Shape::overlaps`). Reported once, never also as a clearance. |
| `Unconnected` | One violation per link `ratsnest` still draws. `measured_nm` is the link length, `required_nm` is 0, `items` are the pads at its ends. |
| `Keepout` | A track, via, pad or footprint inside a board-sketch keepout that forbids it, on a copper layer the keepout lists (#2234). One per keepout and item; `items` is the item, `keepout` the keepout's uuid, both distances 0. See "Keepouts" below. |

Each violation carries its items (sorted), the two nets, the shared layer
mask (`copper_layers` lists them), a location and the two distances. The
report is sorted, so a run is the same on every machine.

A gap equal to the rule passes; one nm under fails. The comparison is exact
(`Separation::is_under` compares squared nm) for every pad shape:

| Pad shape | Read as | Effect on the check |
|---|---|---|
| Rect, round pad, oval, track, via | itself | exact; the only rounding is the foot of a perpendicular on a diagonal, under 1 nm |
| Round-rect | its inset rectangle swollen by the corner radius | exact, the same way |

Pads read their real, rotated outline from `pcb_geom::pad_shape` (#680), so
the check measures the copper as drawn. Trapezoid and custom pads are not in
the format (#2242).

## Rules

`DrcRules::for_board(board, net_classes)`:

- the board-wide minimum is `PcbSetup.clearance` (mm, in the `.snxpcb`
  setup); **0 or absent means unset and the default 0.2 mm
  (`DEFAULT_CLEARANCE_MM`) applies**. No file format changed.
- when the default is used, the report says so (`DrcRules::from_default`): the
  Messages panel shows an info line at the top of the results and the log
  summary repeats it. Rules are to be asked for at board creation (#2232).
- a net class (`NetClass.clearance`, the project's net-class table) raises the
  rule for its nets: a pair of nets needs the largest of the board value and
  the two nets' class values. The class of a net is the first matching one,
  as everywhere else. A class value of 0 is unset.
- a value that is not a finite length is an error, not a silent default.

## How it stays fast

Candidate pairs come from the `BoardIndex` R-tree: one bounding-box query per
item, grown by the largest rule, then the exact test on the pairs it returns.
`crates/signex-drc/tests/drc_corpus.rs` times a 10,000-track board in both a
clean and a 29,502-violation run and asserts a loose bound (10 s in debug CI;
a debug run takes about 0.35 s).

## In the app

`run_drc` (command catalog, group PCB, enabled with a PCB document), the
Tools > Design Rule Check menu row (greyed with no PCB), the command palette
and the Run DRC button in the Messages panel send `Message::Pcb(PcbMsg::DrcRequested)`.
The board is copied and checked in `spawn_blocking`; a result is applied only
if no newer run began. The Messages panel lists the violations (first 500)
above the log, and clicking a row selects its items with the #2191
selection. The summary and the first 20 violations are also logged, which is
what a headless run reports. A report is marked stale when the board's
revision moved after it.

## Zones

`Unconnected` is the ratsnest, so it joins copper through zones the way the
ratsnest does (#2215): a pad island joined only through a same-net pour is not
reported open. The pour itself is not checked: a zone's clearance to other
nets (#2216) and its fill are not modelled, so a zone is never a `Clearance`
or `Short` party.

## Keepouts

The keepouts are the ones the board sketch baked (`PcbBoard::sketch`,
`docs/PCB_SKETCH.md`). Any net counts, net 0 included: a keepout keeps out
copper, not a net.

| Keepout forbids | Reported |
|---|---|
| `no_routing` | tracks on a listed layer |
| `no_vias` | vias reaching a listed layer |
| `no_copper` | tracks, vias and pads on a listed layer |
| `no_drilling` | vias and drilled pads, whatever their layers |
| `no_components` | a footprint whose body (`footprint_body`) meets the keepout, when the outer copper layer of its side is listed |

An item meets a keepout when its copper touches the keepout polygon or lies
inside it (`Shape::overlaps_polygon`). A keepout that lists no copper layer
forbids nothing this check sees. Each keepout is tested against every item,
with a bounding-box test first; an index lookup is a follow-up if boards with
many keepouts make it slow.

## Not checked yet

Hole-to-hole, silk-to-pad, courtyard overlap, zone clearance (#2216), annular
ring, copper with no net (net 0 is skipped, like in the ratsnest), and a rule
per layer or per object kind. Each is its own follow-up.

From the board sketch (#2234): `no_pours` keepouts (zones are not tested
against keepouts), board mounting holes in a `no_drilling` keepout, and
copper-to-board-edge and copper-to-cutout clearance: the rules hold no edge
clearance yet, so the baked outline and cutouts are not checked.
