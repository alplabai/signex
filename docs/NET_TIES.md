# Net ties

A net tie is a 2-pin part that joins two named nets at one point on purpose:
a star ground, a Kelvin sense connection, an analog and a digital ground
meeting under one part (#1277; Caner, 2026-11-25). The nets stay two nets on
the schematic, and on the board the part becomes a copper bridge footprint.
ERC and DRC treat the tie as an intended short.

## On the schematic

- A placed part carries `net_tie`. A part placed from a library symbol of
  type Net Tie is a tie; `set_net_tie id=.. on=true|false` (`docs/CLI.md`,
  "Net ties") makes any placed part one, or an ordinary part again, in one
  undo step. A power port names its net and is never a tie.
- Connectivity is unchanged: each pin of a tie is on its own net, so a tie
  between `GND` and `AGND` leaves both nets in the netlist. The flag changes
  no net.
- ERC reports no short and no second net name for the tie, since its nets
  stay apart. Joining the same two rails with a wire is still the short ERC
  reports.
- `signex_types::net_tie::net_ties` reads each tie off the netlist, per
  sheet occurrence, with the nets its pins are on and a status: `joins` (two
  or more nets), `one_net` (both pins on one net, the tie joins nothing) or
  `open` (fewer than two pins on a net).

## In the file

The flag is an extras key of the part, `net_tie = true`, written only when
set and after every other known key: a sheet with no tie saves byte for byte
as before, and a build that predates the key keeps it among the unknown keys
of the table. The schematic format token does not move (`FORMAT_VERSIONS.md`).

## Netlist export

The netlist listing carries the ties after the nets: a `# <n> net ties`
banner, then one `tie <designator> "<net>" "<net>"` line per tie, its nets
sorted. A design with no tie lists exactly what it listed before.

## On the board

- Update PCB adds, or replaces, a net-tie part's footprint as a net tie:
  component type Net Tie (in BOM) when the part is in the BOM, else Net Tie.
  A footprint the library types as a tie already keeps its type. A footprint
  already on the board keeps its type until a change row replaces it
  (`docs/UPDATE_PCB.md`).
- DRC never pairs two pads of one net-tie footprint: they are the bridge, so
  they may overlap or sit closer than the clearance. A tie's pad against any
  other copper, a pad of another tie included, is checked as every pad is
  (`docs/architecture/pcb-drc.md`).
- `query_net_ties document=<board>` lists each net-tie footprint with the
  nets its pads bridge.

## Not yet

- No Properties row or menu row sets the flag; there is no mockup row for
  one. The CLI, an agent, and placement from a Net Tie library symbol set it.
- A part with more or fewer than two pins may be marked; its status says what
  it joins.
