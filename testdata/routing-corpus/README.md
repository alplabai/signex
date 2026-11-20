# Routing corpus

Fifty real boards for testing the router (#2481): dense BGA and DDR
boards, multi-layer flight controllers, RF and power boards, small MCU
boards and two-layer keyboards. The routing soak and bench
(`crates/signex-engine/tests/routing/`) run on them; how is in
`docs/PCB_TOOLS.md`, "Router soak and bench".

## Licences

These boards are third-party designs under their own licences. They are
not covered by the Signex repository licence, and nothing in this
directory relicenses them.

- `MANIFEST.tsv` lists, for each board: the upstream repository, the
  pinned commit, the source file, the SPDX licence, where upstream states
  it, the licence text in `LICENSES/`, any notice the licence requires
  (`LICENSES/notices/`), the attribution, and the converter with its loss
  report.
- `LICENSES/<SPDX>.txt` holds the text of every licence used.
- Boards under a reciprocal licence (CERN-OHL-S-2.0, CERN-OHL-W-2.0,
  CERN-OHL-1.2) stay under that licence in their converted form; anyone
  who modifies and shares one must follow that licence.
- Each converted file is a modified version of its source: it was
  converted to the Signex board format, and the loss report column says
  what the conversion did not carry over exactly.
- `ulx3s` carries one condition beyond the MIT terms: the EMARD, RADIONA
  and FER logotypes on the top silkscreen stay unmodified and in place.

Only these licences are accepted: MIT (and the ULX3S variant above),
CC-BY-3.0, CC-BY-4.0, CERN-OHL-P-2.0, CERN-OHL-S-2.0, CERN-OHL-W-2.0 and
CERN-OHL-1.2. `scripts/check_routing_corpus.sh` enforces the list and
checks that every board has a manifest row and every row a board.

These boards are test inputs only: they must not become shipped examples,
sample projects, templates, documentation figures or demo content, and
only the routing tests read them; the gate fails on any other reader. The
full rule: `docs/LICENSING.md`, "Third-party test data".

## Storage

The `.snxpcb` files are stored in Git LFS (`.gitattributes`). A clone
without the LFS objects has pointer files in their place; the routing
tests skip such a board and say so, unless
`SIGNEX_REQUIRE_ROUTING_CORPUS=1`, which makes the load check fail on it
(CI sets it when it fetches the corpus). `git lfs pull` fetches them.

## Adding or replacing a board

Pin the upstream commit, check the licence against the list above,
convert the board, and add its manifest row with the converter's loss
report, the copper layer count and the size class (`S` under 1 MB,
`M` up to 5 MB, `L` above, by converted file size).
