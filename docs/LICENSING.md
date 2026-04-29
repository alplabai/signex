# Licensing

Signex is **Apache-2.0**. The main repository contains no GPL-derived
code. KiCad migration support — when a user has existing KiCad files —
ships as a separate **GPL-3.0-or-later** companion tool,
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import),
distributed independently.

This document explains the two-repo split, the audit trail behind it,
and what it means for users and contributors.

## TL;DR

| Repository | License | Contains |
|---|---|---|
| `alplabai/signex` (this repo) | **Apache-2.0** | Native Signex EDA tooling. Schematic + PCB editor, 3D viewer, simulation, plugin system. Reads/writes `.snxsch`/`.snxpcb`/`.snxsym`/`.snxfpt`/`.snxlib`/`.snxprj` natively. |
| `alplabai/signex-kicad-import` (separate) | **GPL-3.0-or-later** | One-way converter from KiCad files (`.kicad_sch`/`.kicad_pcb`/`.kicad_sym`/`.kicad_pro`) to Signex's native formats. Optional install. |

For users:
- **No KiCad files?** Download Signex Community. Done.
- **Have existing KiCad projects?** Download Signex Community + the
  companion converter. Run the converter once against your `.kicad_pro`
  to produce native `.snxsch`/`.snxpcb`/`.snxprj` siblings; open the
  `.snxprj` in Signex from then on.

For redistributors and embedders:
- Signex Community itself has zero GPL transitive dependencies. You can
  embed, link against, or redistribute it under Apache-2.0 without
  inheriting any reciprocal-license obligations.
- The companion converter is GPL-3.0-or-later and is **not** linked into
  Signex Community. Users install it separately if they need KiCad
  migration. Apache consumers of Signex see no GPL aggregation in their
  build closure.

## Why the split?

KiCad is a great EDA project and the file formats it has popularised
(`.kicad_sch`, `.kicad_pcb`) are well-documented enough that earlier
versions of Signex implemented them as a primary read/write format. The
implementation in question (`crates/kicad-parser` and
`crates/kicad-writer`, plus a KiCad netlist exporter) was AI-assisted
and structurally derives from KiCad's GPL-3.0 source — even though no
file was copied verbatim, the choice of token names, struct shapes, and
control flow inherits enough from KiCad's headers (`pin_type.h`,
`layer_ids.h`, `connection_graph.cpp`, `markup_parser.h`) to count as
derivative work under the spirit of KiCad's reciprocal-licensing terms.

KiCad's lead Seth Hillbrand raised the issue cleanly in
[issue #62](https://github.com/alplabai/signex/issues/62). The choices
were:

1. **Dual-license the main repo** — pick GPL-3.0 alongside Apache-2.0
   for the affected crates. Simplest for KiCad-style end-users; harder
   for Apache embedders who'd then have to reason about which crates
   they could link against.
2. **Apache-only the main repo** — remove KiCad-derived code entirely
   from the main repo, ship KiCad migration as a separate GPL-3.0
   companion tool. Cleaner for everyone; one-time UX cost for users
   migrating from KiCad (run the converter once instead of opening
   `.kicad_sch` directly).

We picked **option 2** because Signex's longer-term ambition (Signex
Pro, embedded-in-CI use cases, downstream redistribution) benefits more
from a clean Apache surface than from a "compatible-with-KiCad-but-
also-GPL-in-some-crates" hybrid. The cost is a one-step migration for
existing KiCad users — see the README for the migration walkthrough.

## What was changed

The audit + remediation lived in:

- `.claude/PRPs/issue-62-licensing-remediation.md` — strategy doc.
- `.claude/PRPs/issue-62-execution-plan.md` — engineering plan + decision log.
- `docs/audit/kicad-derivation.md` — file-by-file audit of every
  KiCad-derived element with remediation choice.
- `docs/audit/contributors-2026-04-29.md` — contributor consent record.
- `docs/audit/third-party-kicad-parsers.md` — survey of clean-room
  third-party KiCad parsers as an alternative path.
- `docs/audit/release-notes-remediation-v07-v08.md` — drafts for
  retroactively flagging the v0.7.0 / v0.7.1 / v0.8.0 GitHub Releases
  with the licensing notice.

In code:

- `crates/kicad-parser/` and `crates/kicad-writer/` removed from the
  main workspace; relocated to `signex-kicad-import` under GPL-3.0.
- `crates/signex-output/src/netlist/kicad_sexpr.rs` — KiCad netlist
  exporter — relocated to the companion repo.
- `signex_types::schematic::PinElectricalType` (KiCad-shaped 12-variant
  enum) → replaced by `signex_types::schematic::PinDirection` (14
  variants, Signex-curated; rationale in
  `crates/signex-types/docs/pin-design.md`).
- `signex_types::schematic::PinShape` → replaced by `PinShapeStyle`.
- `signex_types::layer::{F_CU, B_CU, F_SILKS, ...}` constants (mirroring
  pre-KiCad-7 `PCB_LAYER_ID` numbering) → replaced by semantic
  `SignexLayer` enum.
- `signex_types::markup::parse_markup` (KiCad's curly-brace markup
  syntax) → replaced by `parse_signex_markup` using Markdown-extension
  syntax with `_~text~_` overbar for active-low signal naming.
- `kicad_auto_net_name_from_pins` (returning `Net-(<r>-Pad<p>)`) →
  replaced by `auto_net_name(sheet, pins)` returning
  `unnamed-<sheet>:<ref>:<pin>`.
- Native file formats `.snxsch` and `.snxpcb` defined as TOML envelopes
  with TSV bulk-block bodies (same format family as `.snxlib` /
  `.snxsym` / `.snxfpt`).

CI gates that enforce the post-cutover state:

- `.github/workflows/license-guard.yml` — fails any push/PR introducing
  KiCad-derived identifiers (`kicad`, `KiCad`, `F_CU`, `B_CU`,
  `F_SILKS`, `tri_state`, `Net-(`, …) anywhere under `crates/`.
- `deny.toml` — `cargo-deny` config rejecting GPL / AGPL / LGPL /
  unlicensed crates from the transitive dependency closure.

## Contributing

Patches to the main `signex` repo must not contain KiCad-derived code.
The repository is Apache-2.0 clean; KiCad I/O lives in the GPL-3.0
companion repo.

In particular, when you open a PR, declare:

- **Source basis:** [my own work | Signex's prior code | published
  format specs | other (specify)]
- **LLM-assisted:** [yes/no — if yes, list which models]
- **KiCad source consulted:** [yes/no — if yes, the PR belongs in
  signex-kicad-import, not here]

LLMs that have been trained on KiCad source CAN contribute to the GPL
companion repo. They should NOT contribute to the Apache main repo. If
your LLM has consulted KiCad source, route the work to
signex-kicad-import.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full contribution
guide and PR template.

## Acknowledgements

Thanks to Seth Hillbrand ([@sethhillbrand](https://github.com/sethhillbrand))
of the KiCad project for raising the licensing issue cleanly and giving
us a window to fix the code rather than scrub it. The two-repo
structure is a healthier outcome for both projects than the original
"Apache-2.0 everywhere" stance would have been.

## Versions affected

- **v0.7.0** (2026-04-22) — released with KiCad-derived code under
  Apache-2.0 in error. Marked superseded.
- **v0.7.1** (2026-04-24) — macOS Apple-Silicon ad-hoc codesign hotfix;
  same KiCad-derivation as v0.7.0. Marked superseded.
- **v0.8.0** (2026-04-26) — released with KiCad-derived code under
  Apache-2.0 in error. Marked superseded.
- **v0.9.0** (forthcoming) — first Apache-clean release. Bundles the
  Apache-clean cutover, native `.snxsch` / `.snxpcb` formats, and the
  library subsystem (Library Browser, SCH Library editor, Component
  Editor) that was in flight on `feature/v0.9-snxlib-as-file` before
  the licensing remediation. KiCad I/O via the optional
  `signex-kicad-import` companion.

The v0.7.0–v0.8.0 binaries remain available on GitHub Releases for
historical use but are flagged with the licensing notice. Please prefer
v0.9.0 (or later) for new installations.

See [`docs/audit/release-notes-remediation-v07-v08.md`](audit/release-notes-remediation-v07-v08.md)
for the precise text added to each release body.
