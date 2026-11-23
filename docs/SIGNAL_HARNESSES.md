# Signal harnesses

A signal harness carries a named group of signals (`SPI_BUS`: SCK, MOSI, MISO,
CS) as one thick wire, so one harness wire replaces four. Issue #1280.

## Objects

| Object | Where it lives | What it is |
|---|---|---|
| Harness connector | sheet (`SheetHarness::connectors`) | A body with a flat side and a pointed tip. Signal wires end on its entries on the flat side; the harness wire leaves at the tip. It has a harness type (`SPI_BUS`), a width, an entry pitch, a quarter turn and a lock. |
| Harness entry | inside its connector | One named signal of the connector, at a slot on the flat side. Deleting one closes the gap. |
| Harness wire | sheet (`SheetHarness::wires`) | One straight segment that carries a whole harness, drawn thicker than a bus in the harness colour. |
| Harness-typed sheet entry | sheet (`SheetHarness::entry_types`) | A sheet entry of a sheet symbol with a harness type: it carries a harness into the child sheet. Set in Properties > Harness type. |
| Harness definition | project (`.snxprj` `schematic_design.harness_definitions`) | A harness type and its entries, in order. An entry may itself be a harness (`UART` inside `DEBUG`). |

The tools are Place > Ports & Connectors > Signal Harness, Harness Connector
and Harness Entry, and the Active Bar's Harness group:

- **Signal Harness** draws harness wire segments click by click, bent like a
  bus.
- **Harness Connector** is two clicks: the corner of the body, then its width.
  A second click left of the first points the tip left. The connector takes
  the type last placed, else the project's first definition, else `HARNESS`,
  and starts with one entry per entry of its definition.
- **Harness Entry** is one click on a connector: the entry goes to the nearest
  slot and takes the next definition entry the connector lacks, else
  `ENTRY_n`.

Every placement and every Properties edit is one undo step. Moving, turning,
mirroring, locking, deleting, copying and pasting work on connectors (with
their entries) and harness wires as on any object. A pasted connector and its
entries get new ids.

## Connectivity

Per sheet, a **bundle** joins, by the 1 µm point key:

- harness wires end to end, and a harness wire ending on another one (a T
  needs no dot: a bundle is typed, never two nets crossing);
- connector tips, harness-typed sheet entries and Hierarchical or Global
  labels lying on a harness wire. Such a label is a **harness port**, named
  by its text.

Within a bundle, the nets wired to the entry named `E` on every connector are
one net. An entry whose point a harness wire or a connector tip touches
**nests** that other bundle: its entries join as `E.F`.

Across the hierarchy, a harness-typed sheet entry `N` on a sheet symbol joins
the bundle it touches with the child sheet's bundle that carries the harness
port `N`, entry by entry by path. A child that only passes a harness on to a
grandchild (a port and a typed sheet entry on one bundle, no connector) does
not carry it further yet.

A sheet without harness objects takes none of these steps: its netlist is the
same as before harnesses existed.

## Net names

A harness net no label names is called `<Type>.<Entry>` (`SPI_BUS.SCK`), a
nested one `<Type>.<Entry>.<Entry>` (`DEBUG.UART.TX`). A net label (or any
other name the naming rule ranks, see `docs/NET_NAMING.md`) outranks it.

## ERC

| Rule | Severity | Finds |
|---|---|---|
| `harness_entry_mismatch` | Warning | An entry the far side of the harness carries and this connector lacks (`sensor expects entry CS_ADC; this harness has CS`), an entry only this connector carries (`sensor has no entry CS2`), compared by full path so nested entries count (`sensor has no entry UART.RX`) and reported on the parent sheet, a definition entry the connector lacks, or a connector entry its definition does not list. Located on the entry. |
| `harness_type_conflict` | Warning | Connectors or typed sheet entries of two types on one harness. |

Waivers and No-ERC directives work as for every rule. A port on a harness
wire is not a disconnected port, a typed sheet entry on a harness wire is not
an unconnected sheet pin, and a wire into a harness entry neither dangles nor
ends its net on the sheet. The built-in rule profile moved to version 1.2 with
these two rules.

The connector's Properties show a far-side mismatch that pairs two plain
entries as a warn banner with two buttons: **Rename** renames this side's entry
(one undo step) and **Go to** opens the child sheet. An unpaired or nested
difference appears in the ERC results only, because renaming a nested entry
would move every signal under it. The canvas marks the entry with the ERC warning
marker.

## Colour

Harnesses have one project colour, the `harness` token of the canvas palette
(Graphite Light `#6B4FA0`, Dark `#B79CF0`), with the `harness_soft` fill
(`#EFEAF8`, `#261F36`). PDF and SVG exports print harnesses the same way.

## Files and older builds

Harness objects are saved in a sheet's `[extras.sheet]` and need the
`snxsch/1.3` format; the definitions are saved in the `.snxprj`. A build that
reads only an older format opens a sheet with harnesses read-only, so it never
drops them on save. See `docs/architecture/snxsch-refinement-objects.md` and
`docs/FORMAT_VERSIONS.md`.

## Test fixture

`signex-net` `harness_fixture.rs` builds a three-sheet project as mockup row
152 draws it, and its netlist is pinned by a golden netlist:

- `mcu.snxsch` (root): U3's SPI pins go into an `SPI_BUS` connector whose
  harness wire runs to the `sensor` sheet symbol's harness-typed entry
  `SPI_BUS`. U3's SWD pins go into a `DEBUG` connector; its `UART` entry
  nests a `UART` connector wired to U3's TX / RX; the `DEBUG` harness runs to
  the `connectors` sheet symbol's typed entry `DEBUG`.
- `sensor.snxsch`: a Hierarchical port `SPI_BUS` on a harness wire to an
  `SPI_BUS` connector wired to U1.
- `connectors.snxsch`: a port `DEBUG` on a harness wire to a `DEBUG`
  connector wired to J2, with a nested `UART` connector wired to J2.

Wires end exactly on the entry points and tips the model computes, so the
fixture holds whatever the connector geometry is.
