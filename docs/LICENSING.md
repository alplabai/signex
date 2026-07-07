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

## LLM context discipline

Post-cutover development on this repository uses LLM-assisted workflows.
That comes with a specific operational rule: **KiCad source code is
never placed in an agent's context window, prompt, retrieval index,
or reference material when work is being produced for this repo**. The
LLM is given only the existing `signex` codebase plus internal
specifications (audit docs, format design notes, type schemas) when
asked to refactor, write new code, or design new types.

This matters for the derivation question. Copyright derivation
attaches at the point of authorship — what the author had in front of
them when producing the work. An LLM asked to refactor an Apache-2.0
codebase, given only that codebase as input, produces output derived
from that input. Training-data exposure is a separate concern that
does not by itself create a derivation chain to any specific GPL
project — that is the working consensus of the relevant case law (the
GitHub Copilot litigation has narrowed rather than broadened that
theory) and is also the assumption every permissively-licensed Rust
project relying on LLM-assisted development implicitly makes.

The discipline is a process rule, not a marketing claim. It shapes
agent prompts and retrieval scopes. Opening a PR against this repo
affirms no license-gated source files were used; CI blocks only a
description that explicitly admits one (a `License-gated sources: yes`
line), routing it to the companion repo.

If you are reviewing this codebase and have a specific file or specific
lines you believe are derivative of KiCad source despite the above —
that is actionable feedback we want. The broader claim "any
LLM-assisted refactor of formerly-derivative code remains derivative"
is, as far as we can see, not the legal standard, and accepting it
would make permissively-licensed software impossible to develop with
modern code-LLMs at all.

## Contributing

Patches to the main `signex` repo must not contain KiCad-derived code.
The repository is Apache-2.0 clean; KiCad I/O lives in the GPL-3.0
companion repo.

In particular, opening a PR affirms **no license-gated source files**
were used — nothing under GPL/copyleft or otherwise Apache-incompatible.
No declaration fields are required; a contribution that did use one adds
a line `License-gated sources: yes` and belongs in signex-kicad-import
instead.

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
- **v0.9.0** (2026-04-29) — first Apache-clean release. Apache-clean
  cutover only: native `.snxsch` / `.snxpcb` formats + signex-types
  Apache-clean enums + KiCad I/O moved to the optional
  `signex-kicad-import` GPL-3.0 companion. Library subsystem work
  ships separately starting at v0.10.0.
- **v0.9.1** (2026-04-29) — async save + borrow-based serialise patch.
  No licensing surface change; same Apache-clean invariants as v0.9.0.
- **v0.10.0** (2026-04-29) — Library Browser tab scaffold (read-only
  table over `.snxlib` packages) bundled with an Apache-clean residual
  polish pass: vestigial KiCad-shaped state field and helper names
  renamed to neutral terms, neutral name for the
  `MultisheetStyle::KiCad` enum variant, License-Guard regex tightened
  with additional forbidden patterns, this LLM-context-discipline
  section added. First slice of the v0.10 Library milestone;
  remaining sub-releases (v0.10.1–v0.10.9) build the SCH Library
  editor, Pin Properties dialog, drawing tools, multi-symbol
  containers, and Component Editor on top of the same foundations.

The v0.7.0–v0.8.0 binaries remain available on GitHub Releases for
historical use but are flagged with the licensing notice. Please prefer
v0.9.0 (or later) for new installations.

See [`docs/audit/release-notes-remediation-v07-v08.md`](audit/release-notes-remediation-v07-v08.md)
for the precise text added to each release body.
