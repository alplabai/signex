# Power tree (#1313)

The power tree is the second view of a block diagram (mockup row 171): sources,
converters, rails and loads, with the current each part draws and a budget per
block. It is generated from the schematic every time it is asked for; nothing
of it is stored. The domain code is `signex_net::power_tree`; the command is
`query_power_tree` ([CLI.md](CLI.md), "Power tree"). The block diagram tab
draws it ([BLOCK_DIAGRAM.md](BLOCK_DIAGRAM.md), "Power tree view") from the
same run.

## Rails

A rail is a net, by name, that is one of:

- named by a power port or a Power label (the same test the compiled design
  uses, `NetMember::names_power_net`);
- listed in the project's rail list (see "Open seam" below);
- named by a fitted part's `Rail Supply <net>` or `Rail Load <net>` parameter.

A ground is no rail. A net is a ground when one of its words (split at any
character that is not a letter or a digit) is `GND` or starts or ends with
it (`AGND`, `PGND`, `GND_ISO`), or is `VSS`, `EARTH` or `0V`. Grounds are
listed apart in the answer.

## Parts

Only parts fitted in the variant read count (`signex_output::part_is_fitted`).
A part's units are one part; its parameters are its units' fields, the first
unit's value winning. A part is on a rail when one of its pins is on the rail's
net or one of its rail parameters names the rail. On a rail, a part is:

- a **source** when it has `Rail Supply <net>`: the most current it can
  supply, which is the rail's limit when it is the rail's only source;
- a **converter** when it is a source fed from another rail it is on;
- a **load** otherwise. A load's current is its `Rail Load <net>`. A converter
  without one draws what its outputs deliver over its efficiency:
  `sum(output volts x output amps) / Efficiency / input volts`. That is worked
  out only when the converter has one input rail, an `Efficiency`, a voltage
  on every rail involved, and is the only source of each output whose whole
  draw is known. A loop of converters stops with the current not known.

The parameter names `Rail Load <net>` and `Rail Supply <net>` are the ones the
rail requirement kinds read (#2943).

Currents read as a number with an optional `A`, `mA`, `uA` or `µA`, finite and
not negative. `Efficiency` reads as `88 %` or as a fraction (`0.88`), above 0
and at most 1; a bare number above 1 is refused rather than read as a
percentage.

## Voltages

A rail's voltage is the project's (its rail list), else the voltage its net
name spells in its last word that spells one: `3V3`, `+3V3`, `1V8`, `5V0`,
`12V`, `3.3V`, `-12V` (`VOUT_3V3` is 3.3 V, `VIN_12V` is 12 V). Words are split
at `_`, `/` and spaces. The answer says which (`voltage_from`).

## Problems

Missing data is never a guess and never a pass. Each rail lists what keeps its
budget from being told, as `{"kind": ...}`:

| Kind | Meaning |
|---|---|
| `no_voltage` | The project lists no voltage and the name spells none. |
| `no_source` | No fitted part supplies the rail. |
| `several_sources` | More than one part supplies it (`count`); it has no one limit. |
| `unknown_load` | A part on it (`designator`) says nothing of its current. |
| `unread_parameter` | A parameter (`designator`, `parameter`, `text`) is no current or no efficiency. |
| `unknown_input` | A converter's (`designator`) input current cannot be worked out. |

## Budget levels

A use of a limit (a rail's total against its one source's limit) has a level,
as mockup row 171 draws it: `fine` under 50 %, `close` from 50 % to 90 %, `over`
above 90 %. A load's share of its rail is a number (`share_of_rail`) with no
level.

## Budget per block

The budget per sheet adds up, per rail, what the sheet's parts draw (`draws`)
and supply (`feeds`). Two occurrences of a sheet add up. A generated block
diagram has one block per sheet, linked by the sheet's document id
(`SheetLink::sheet`), so a block's budget is its sheet's
(`signex_net::power_tree::block_budgets`); a block no sheet backs has none.
`power_w` is the sheet's draws times their rails' voltages, when every draw is
known.

## The answer

```json
{
  "project": "/abs/dcdc-buck.snxprj",
  "variant": null,
  "roots": ["VIN_12V"],
  "grounds": ["GND"],
  "rails": [{
    "net": "VOUT_3V3", "voltage": 3.3, "tolerance_percent": null,
    "voltage_from": "net_name",
    "sources": [{"part": {...}, "limit_a": 3.0, "converter": true}],
    "loads": [{"part": {...}, "current_a": 0.14, "current_from": "parameter",
               "share_of_rail": 0.22}],
    "total_a": 0.64, "complete": true, "limit_a": 3.0, "headroom_a": 2.36,
    "level": "fine", "fed_from": ["VIN_12V"], "problems": []
  }],
  "converters": [{"part": {...}, "inputs": ["VIN_12V"], "outputs": ["VOUT_3V3"],
                  "efficiency": 0.88, "output_w": 2.11, "input_w": 2.4,
                  "loss_w": 0.29}],
  "sheet_budgets": [{"sheet": "<sheet id>", "sheet_path": "dcdc.snxsch",
              "draws": [{"net": "VIN_12V", "current_a": 0.2, "complete": true,
                         "parts": ["U1"]}],
              "feeds": [{"net": "VOUT_3V3", "limit_a": 3.0, "parts": ["U1"]}],
              "power_w": 2.4}],
  "warnings": []
}
```

A `part` is `{"designator", "value", "sheet", "sheet_path", "symbol"}`: `sheet`
is the sheet's document id and `symbol` the part's first placed unit, where a
click on the part goes. `current_from` is `parameter` or `converter`.
`voltage_from` is `project` or `net_name`. Numbers not known are `null`.

## Open seam

The project's rail list (`.snxprj` `rails`, #2943) is not on trunk yet. The job
passes an empty list, so every voltage comes from the net name until it lands;
`PowerTreeJob::listed` is where it goes. Manual overrides stored in the block
diagram file and several load scenarios are not built (mockup row 171, "Needs a
decision").
