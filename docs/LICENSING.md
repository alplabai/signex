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

Opening a PR affirms **no license-gated source files** were used —
nothing under GPL/copyleft or otherwise Apache-incompatible. A
contribution that did use one adds a line `License-gated sources: yes`
and belongs in signex-kicad-import instead.

### What "otherwise Apache-incompatible" means

This document is the canonical statement of that phrase. It was
undefined until issue #305, and the omission was expensive for someone
other than us.

[PR #304](https://github.com/alplabai/signex/pull/304) was a substantial,
competent Rust rewrite of a project licensed "CC BY 4.0 … You may not
resell this tool". Both halves of that are disqualifying here. It passed
all twelve licence-guard jobs and `cargo deny` clean — a port is not a
Cargo dependency, so `deny` never sees it, and it contained no
KiCad-shaped identifier, so the guards never fired. The self-declaration
was answered in good faith: CC BY reads as permissive, and the resale
restriction is a trailing sentence appended by the author that appears
nowhere in the CC BY licence text. No amount of CI would have changed
that answer. The defect was that we had never written the rule down.

**A port or translation is a derivative work.** Rewriting a project's
JavaScript as Rust does not reset its copyright. Neither does re-typing
its C++, renaming its identifiers, reorganising its modules, or routing
the translation through an LLM. Copyright derivation attaches to what the
author had in front of them at the point of authorship — the same
principle stated in "LLM context discipline" above, applied outward
rather than inward. Independent implementation of a *published algorithm
or formula* is a different act and is not derivation; the distinction is
the source you worked from, not the distance between the outputs.

**Incompatible classes:**

| Class | Examples | Why it fails here |
|---|---|---|
| GPL / copyleft | GPL-2.0/3.0, AGPL, LGPL | Reciprocal terms relicense Signex. LGPL included — a binding is a link. Copyleft solvers are reached across a process boundary only; see [EXTERNAL_TOOLS.md §4](EXTERNAL_TOOLS.md#4-the-gpl--lgpl-bridge-boundary), the dependency-side counterpart to this section. |
| **Any Creative Commons licence** | CC BY, CC BY-SA, CC BY-NC | CC is not a software licence — Creative Commons states this itself and recommends against using CC for code. CC BY's attribution terms do not compose with Apache-2.0's `NOTICE` model; BY-SA is copyleft; BY-NC adds a field-of-use bar. CC0 is a public-domain dedication rather than a licence in this sense and is fine. |
| Non-commercial / no-resale / any field-of-use restriction | CC BY-NC, "you may not resell this tool", "personal use only" | **Signex Pro is sold from this tree.** See below. |
| Source-available / open-core / "fair source" | BUSL, SSPL, Elastic, Commons Clause, PolyForm | Use restrictions and/or reciprocal terms; not open source. |
| Text, tables, and figures of paywalled standards | IPC, IEC, JEDEC, ISO | The **document** is copyrighted. The **formulas and physical facts** in it are facts and are not — implementing the maths from your own understanding is fine and welcome. Copying prose, tables, figure geometry, or worked examples is not, including via an LLM. |

MIT, BSD, ISC, Zlib, Unlicense, CC0 and Apache-2.0 are compatible.
Anything on neither list is a question worth asking before it is a PR
worth writing.

**Why a field-of-use restriction is fatal to this repo specifically.**
Signex Community is Apache-2.0 and free. **Signex Pro is a paid
commercial edition built from the same source tree.** A "non-commercial"
or "no resale" term on any code under `crates/` would be breached the day
Pro ships, and would void the unencumbered Apache-2.0 surface promised to
redistributors and embedders in the TL;DR above — the same promise that
motivated the two-repo split in the first place. Many projects could
accept such a term. This one cannot. That is a fact about our business
model, not a criticism of the licence or of anyone who chose it.

**If you are unsure about a source, open an issue and ask.** Name the
project and its licence; we will answer. That is cheaper for you than
discovering it at review, and considerably cheaper for us than saying no
to finished work.

### LLM provenance

LLMs that have been trained on KiCad source CAN contribute to the GPL
companion repo. They should NOT contribute to the Apache main repo. If
your LLM has consulted KiCad source, route the work to
signex-kicad-import. The same rule generalises: if you or your assistant
worked from the source of *any* project in an incompatible class above,
the result carries that project's licence and does not belong here.

See [CONTRIBUTING.md](../CONTRIBUTING.md#license-compliance-for-contributions)
for the contributor-facing version, the PR template, and the CI
mechanics. Note that the CI gates in `license-guard.yml` are shaped
around the KiCad incident and cannot detect a port of an arbitrary
project; the advisory `port-smell` job added for #305 is a heuristic and
is honest about being one. The written policy above is the control.

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
