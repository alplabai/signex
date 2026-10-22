# Gerbolyze protoboard fixtures

These fabrication archives are test fixtures derived from the board definitions
in Gerbolyze's
[`generate_protoboards.py`](https://github.com/jaseg/gerbolyze/blob/main/generate_protoboards.py).
They are intended to exercise Gerber and Excellon import without depending on
KiCad source code.

## Archives

| Archive                                | Coverage                                               |
|----------------------------------------|--------------------------------------------------------| 
| `tht_pitch100mil_20x30.zip`            | Through-hole pads and plated drills on a 100 mil grid  |
| `smd_sop650_double_side_20x30.zip`     | Double-sided 0.65 mm SMD pad arrays                    |
| `mixed_tht_smd_double_side_20x30.zip`  | Through-hole pads plus double-sided 1.27 mm SMD arrays |

Each ZIP contains:

- `proto-F_Cu.gbr` and `proto-B_Cu.gbr`
- `proto-F_Mask.gbr` and `proto-B_Mask.gbr`
- `proto-F_Paste.gbr` and `proto-B_Paste.gbr`
- `proto-F_SilkS.gbr` and `proto-B_SilkS.gbr`
- `proto-Edge_Cuts.gbr`
- `proto-PTH.drl`
- `proto-NPTH.drl`

All member paths were checked for absolute paths and parent traversal. Each
archive was reopened by `gerbonara.LayerStack.open`, which recognized the
fabrication layers and reported bounds of approximately 20.05 mm x 30.05 mm.
The parser warns that the generated edge-cut arcs do not explicitly enable
multi-quadrant interpolation with `G75`. The files are retained this way as a
useful robustness fixture for older or stricter Gerber interpreters.

## Licensing

Gerbolyze's source code is licensed under AGPL-3.0-or-later. Its generated
protoboard index separately states that the downloadable protoboard outputs are
provided under the Unlicense. These archives contain generated board output,
not copied Gerbolyze source code.
