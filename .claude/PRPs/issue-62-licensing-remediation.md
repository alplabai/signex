# Issue #62 — Apache-only licensing remediation plan

**Status**: planning, no implementation started.
**Trigger**: GitHub issue [#62](https://github.com/alplabai/signex/issues/62) raised by Seth Hillbrand (KiCad project) requesting GPL-v3 compliance for KiCad-derived code.
**Decision**: keep the main `signex` repo under **Apache-2.0**, move all KiCad-derived code to a separate GPL-3.0 companion tool.
**Estimated effort**: 10–11 calendar weeks / ≈10.8 engineer-weeks.
**Audit basis**: structural + KiCad source comparison via the GitLab/GitHub mirrors of `kicad-source-mirror`.

---

## 1. Summary

Seth Hillbrand has identified that several Signex crates derive from KiCad's GPL-v3 source code. The current Apache-2.0 license on those crates is incorrect under KiCad's reciprocal terms. To preserve Apache-2.0 for the main Signex codebase, every KiCad-derived element must be **removed** from the main repo (not relabeled). KiCad migration support continues via a separate **GPL-3.0** companion tool that ships independently.

Two-repo split:

```
alplabai/signex                 (Apache-2.0)         — main repo, no KiCad-derived code
alplabai/signex-kicad-import    (GPL-3.0-or-later)   — new repo, KiCad → Signex converter
```

Users who need to migrate existing KiCad projects download both. Apache consumers of `signex` get a clean Apache codebase with no GPL aggregation.

---

## 2. Audit findings (KiCad source compared)

The following items in the current Signex codebase derive from KiCad source. Verified against the public KiCad source mirror.

### 2.1 Verified strong derivation

| Signex location | KiCad source | Match strength |
|---|---|---|
| `signex_types::markup::kicad_auto_net_name_from_pins` produces `Net-(<r>-Pad<p>)` | `connection_graph.cpp::driverName()` → `GetDefaultNetName()` produces `Net-(<RefDes>-Pad<PinNum>)` | **Exact** — same format string, identical output. The function name even contains "kicad_". |
| `signex_types::schematic::PinElectricalType` (12 variants) | `common/pin_type.h::ELECTRICAL_PINTYPE` (12 variants) | **Same set, same canonical names** — KiCad's `GetCanonicalElectricalTypeName()` returns `"input"`, `"output"`, `"bidirectional"`, `"tri_state"`, `"passive"`, `"free"`, `"unspecified"`, `"power_in"`, `"power_out"`, `"open_collector"`, `"open_emitter"`, `"no_connect"`. Signex's serde renames produce identical lower-snake_case tokens. |
| `signex_types::schematic::PinShape` (9 variants) | `pin_type.h::GRAPHIC_PINSHAPE` (9 variants: LINE/INVERTED/CLOCK/INVERTED_CLOCK/INPUT_LOW/CLOCK_LOW/OUTPUT_LOW/FALLING_EDGE_CLOCK/NONLOGIC) | **Same set**, same naming convention with PascalCase rewrite. `EdgeClockHigh` ↔ `FALLING_EDGE_CLOCK` is the only meaningful divergence. |
| `crates/kicad-parser/` (whole crate, 3,938 LOC) | KiCad's documented S-expression file formats (`.kicad_sch`/`.kicad_pcb`/`.kicad_sym`) — token names (`effects`, `lib_id`, `sym_name`, `stroke`, `pin_names`, `hide`, …) come straight from KiCad's grammar | Strong. Format-spec-only clean-room implementation is not credible given AI-assisted development history. |
| `crates/kicad-writer/` (whole crate, 2,274 LOC) | Mirror of above for output | Strong. |
| `signex_output::netlist::kicad_sexpr` (KiCad netlist exporter, 336 LOC) | `eeschema/netlist_exporters/` | Direct format target; uses `kicad-parser`'s sexpr_builder. |

### 2.2 Strong derivation, **older KiCad release**

| Item | Note |
|---|---|
| `signex_types::layer::*` constants (`F_CU=0`, `B_CU=31`, `F_SILKS=36`, `F_MASK=38`, `F_PASTE=34`, `F_FAB=40`, `F_CRTYD=42`, `EDGE_CUTS=44`, `MARGIN=45`, `DWGS_USER=46`, `CMTS_USER=47`, `ECO1_USER=49`, `ECO2_USER=50`) | **Pre-KiCad-7 numbering scheme.** Current KiCad master uses `B_Cu=2`, `F_SilkS=5`, `F_Mask=1`, `F_Paste=13`, `F_Fab=35`, `Edge_Cuts=25`, `Dwgs_User=17` (rearranged for index efficiency). Signex's numbering matches KiCad 5/6 era. Still derivative — just from an older snapshot. Doesn't change the legal exposure but the "bit-exact match to current KiCad" claim from the initial audit was inaccurate; it's bit-exact to KiCad 5/6. |

### 2.3 Partial overlap (less certain derivation)

| Item | KiCad | Signex | Note |
|---|---|---|---|
| Markup escape syntax | `~`, `^`, `_`, `{`, `}` (overbar/superscript/subscript with curly-brace delimiters per `MARKUP_PARSER`) | `~`, `^`, `_`, `$`, `@`, `{`, `}` (Signex extends with `$`, `@`) | Signex's set is a superset of KiCad's. Same syntactic shape (sigil + braces). The extension hints at Signex-original work atop KiCad-derived foundation. |
| `signex_erc::RuleKind` (11 ERC rule kinds) | KiCad's `ERCE_*` items + Altium's matrix | Hybrid — doc comment says "Altium ERC matrix conventions". Some semantic overlap with KiCad's ERC items but rule names + implementations are independent. |

### 2.4 Likely independent

| Item | Reasoning |
|---|---|
| `kicad-parser/src/sexpr.rs` (376 LOC) | Generic Lisp-style S-expression lexer with `Atom::Raw`/`Atom::Quoted`. No KiCad-specific tokens. Could plausibly survive a clean-room scrutiny. |
| Most of `signex-erc`, `signex-engine`, `signex-render`, `signex-app`, `signex-library`, `signex-widgets`, `signex-output` (excluding the netlist exporter) | Independent Iced/wgpu rendering, panel docking, AI scaffolding, library subsystem, BOM/PDF output. The bulk of the codebase. |

### 2.5 Audit verification record

Every claim above was verified by fetching the corresponding file from `https://github.com/KiCad/kicad-source-mirror` (or its GitLab origin) and comparing token-for-token. The verification queries are reproducible:

- `gh search code "enum class ELECTRICAL_PINTYPE" --repo KiCad/kicad-source-mirror`
- `https://raw.githubusercontent.com/KiCad/kicad-source-mirror/master/common/pin_type.h`
- `https://gitlab.com/kicad/code/kicad/-/raw/master/include/layer_ids.h`
- `https://raw.githubusercontent.com/KiCad/kicad-source-mirror/master/include/markup_parser.h`
- `https://raw.githubusercontent.com/KiCad/kicad-source-mirror/master/eeschema/connection_graph.cpp`

These confirm that the items in §2.1 derive from current KiCad source and the items in §2.2 derive from a pre-KiCad-7 release.

---

## 3. Strategy: two-repo split

### 3.1 Repo separation

| Repo | License | Contents |
|---|---|---|
| `alplabai/signex` (this repo) | Apache-2.0 | Pure Signex code: Iced/wgpu rendering, schematic/PCB editor engine, library subsystem, BOM/PDF output, panel docking, AI scaffolding. **Native `.snxsch` / `.snxpcb` formats only**, no `.kicad_sch` parser, no KiCad-derived enums. |
| `alplabai/signex-kicad-import` (NEW) | GPL-3.0-or-later | One-way converter: `.kicad_sch` / `.kicad_pcb` / `.kicad_sym` → Signex native formats. Standalone CLI tool, distributed independently. |

### 3.2 Dependency direction

```
   Apache-2.0                              GPL-3.0-or-later
   signex (binary)                          signex-kicad-import (CLI)
       │                                            │
       ▼                                            ▼
   signex-types ◀────────────────────────── signex-kicad-import
   signex-engine                            (depends on Apache signex-types
   signex-render                             via crates.io / Cargo path-dep,
   signex-erc                                emits Signex-native data
   signex-output                             structures, GPL contains
   signex-library                            kicad-parser + kicad-writer)
   signex-widgets
   signex-app
```

GPL → Apache is legal; Apache → GPL is what we're avoiding. The companion tool depends on `signex-types` (Apache); the main `signex` workspace never depends on `signex-kicad-import`.

### 3.3 User experience

1. User downloads Signex Community (Apache-2.0 binary).
2. Has existing KiCad project? Download `signex-kicad-import` separately (GPL-3.0 binary).
3. Run converter once: `signex-kicad-import path/to/project.kicad_pro` → produces `.snxprj` + `.snxsch` siblings.
4. Open `.snxprj` in Signex Community.

The companion tool is a one-time migration aid, not a continuous round-trip. Signex Community reads/writes only Signex-native formats.

---

## 4. Phases

### Phase 0 — Decision + announcement (½ day)

- Reply to issue #62 confirming Apache-only direction + 8–10 week timeline + companion-tool plan.
- Take Seth up on his offer of the exhaustive list of KiCad-derived code.
- Update README's "Signex is KiCad-compatible" framing. Stage 11 finalises the wording; Phase 0 sets expectations.

### Phase 1 — Complete file-by-file audit (1 week)

- Run Seth's exhaustive list against the audit findings in §2.
- Resolve every Tier 2/Tier 3 ambiguity (markup parser, ERC rules, sexpr lexer) — clear-cut derivative or independent.
- Produce `docs/audit/kicad-derivation.md` recording the findings; ship in repo as the audit trail.

### Phase 2 — Signex-native types in `signex-types` (2 weeks)

Independent designs, **not** rewrites of KiCad's enums.

#### 2.1 Layer abstraction (~3 days)

- Delete `signex_types::layer::{F_CU, B_CU, F_SILKS, …}` constants and the `LayerId(u8)` newtype with KiCad numbering.
- Add `signex_types::pcb::SignexLayer` enum (semantic, not numeric):
  ```rust
  pub enum SignexLayer {
      TopCopper, BottomCopper, InnerCopper(u8),
      TopSilk, BottomSilk,
      TopSolderMask, BottomSolderMask,
      TopPaste, BottomPaste,
      TopAssembly, BottomAssembly,
      TopCourtyard, BottomCourtyard,
      BoardOutline, KeepOut,
      Mechanical(u8),
      User(u8),
  }
  ```
- Wire conversions throughout `signex-types::pcb::Pad`, `Footprint`, `signex-render`, `signex-engine`, `signex-output`.
- 80–120 call sites; mostly mechanical type-rename.

#### 2.2 Pin enum redesign (~2 days)

- Delete `PinElectricalType` and `PinShape` from `signex-types`.
- Add `signex_types::schematic::PinDirection` and `signex_types::schematic::PinShapeStyle` with **a curated, Signex-original variant set** — not a 1:1 rewrite of KiCad's. Suggested differences:
  - Merge `OpenCollector` + `OpenEmitter` into `OpenDrain { polarity }` (industry-standard term, not KiCad's).
  - Add `Differential` (Signex-original; useful for high-speed design).
  - Add `Clock` as a directional pin variant rather than a shape (more idiomatic to modern EDA).
  - Drop `EdgeClockHigh` from shape — niche KiCad relic.
- The new variant set is provably Signex-curated — if Seth ever questions it, the diff against KiCad's enum is meaningful (different size, different boundaries, different emergent semantics). Document the design rationale in `crates/signex-types/docs/pin-design.md`.

#### 2.3 Markup format swap (~3 days)

- Delete `signex_types::markup::parse_markup` and `kicad_auto_net_name_from_pins`.
- Adopt **Markdown subset** as Signex's native markup:
  - `**bold**`, `*italic*`, `~~strike~~`
  - `^superscript^`, `~subscript~` (single-character delimiters, Markdown-extension style — different from KiCad's `^{}`/`_{}`/`~{}`)
  - `[text](url)` for links
  - `\` to escape any of the above
- Add `signex_types::markup::parse_signex_markup`.
- For auto net names, define `signex_types::schematic::auto_net_name(sheet, ref, pin) -> String` returning `unnamed-{sheet}:{ref}:{pin}` or similar. **Don't reproduce KiCad's `Net-(<r>-Pad<p>)` format** — that's the most clear-cut derivation evidence in the codebase.
- ~15 sites, plus every test fixture using markup.

#### 2.4 Validation gate

- Add a `cargo-deny` config that fails CI if any source file in `signex-types/` contains the strings `KiCad`, `kicad`, `F_CU`, `B_CU`, `F_SILKS`, `tri_state`, `Net-(`, etc. — sentinel guard that catches regression contributions.

### Phase 3 — Native `.snxsch` / `.snxpcb` formats (3 weeks)

#### 3.1 Format design (~3 days)

- `.snxsch` — JSON serialisation of `signex_types::schematic::Schematic`. Already serde-derives.
- `.snxpcb` — JSON of `signex_types::pcb::Board`.
- `.snxprj` — already exists for project file, stays as-is (it's our format).
- Schema versioning: top-level `format = "snxsch/1"`. Stage 12a's TOML approach for `.snxlib` translates: every Signex-native format declares its version token at the top.

#### 3.2 Engine + app rewiring (~1.5 weeks)

- `signex-engine` save/load pipeline switches to `.snxsch`/`.snxpcb` JSON.
- `signex-app` file dialogs filter on `.snxsch` only. `.kicad_sch` no longer opens directly.
- All test fixtures: convert `.kicad_sch` → `.snxsch` via a one-time migration script. Probably 50–100 fixtures.

#### 3.3 v0.7.x → v0.9.0 migration shim (~3 days)

- Detect existing user projects on disk that contain `.kicad_sch` files.
- First-run wizard: "We've moved to native Signex formats. Want to convert this project?"
- Run the companion tool (Phase 4) under the hood to perform the conversion.
- Keep the original `.kicad_sch` as a backup; new file is `.snxsch`.

### Phase 4 — Companion tool repo: `signex-kicad-import` (3 weeks)

#### 4.1 New repo, GPL-3.0-or-later (~½ day)

- `git init` a fresh repo at `alplabai/signex-kicad-import`.
- `LICENSE` = GPL-3.0-or-later from day one.
- `README` explains: "One-way converter from KiCad files to Signex native format. Apache-licensed Signex Community Edition does not include this tool — install separately."

#### 4.2 Move kicad-parser + kicad-writer in (~1 day)

- Cherry-pick the existing `kicad-parser` and `kicad-writer` crates into the new repo.
- Their dependency `signex-types` is Apache-2.0 — pulled in as a published crate from crates.io, or via a Cargo path-dep when developing locally side-by-side.
- Direction is correct: GPL companion → Apache `signex-types`, not the reverse.

#### 4.3 CLI converter binary (~1.5 weeks)

- `signex-kicad-import path/to/project.kicad_pro` → produces `path/to/project.snxprj` + `.snxsch`/`.snxpcb` siblings.
- Reads existing KiCad files via the existing kicad-parser crate.
- Writes Signex-native JSON via `signex-types::Schematic::serialize_json()`.
- Tests: round-trip a representative set of KiCad demo projects through the converter; assert the resulting `.snxsch` opens cleanly in Signex Community.

#### 4.4 Distribution (~3 days)

- GitHub Release for the companion tool (Linux/Windows/macOS pre-built).
- Linked from Signex Community's first-run wizard ("Need to migrate KiCad files? Download the converter").
- The companion tool is independent — its own version cadence, its own changelog.

### Phase 5 — Drop KiCad I/O from the main repo (1 week)

- Delete `crates/kicad-parser/` and `crates/kicad-writer/` from `signex` workspace.
- Delete `crates/signex-output/src/netlist/kicad_sexpr.rs` (the KiCad netlist exporter); add it to `signex-kicad-import` instead, or to a future `signex-kicad-export` separate companion if export-back-to-KiCad is wanted.
- Remove all `use kicad_parser::…` / `use kicad_writer::…` imports from the workspace (39 sites in the audit). They're replaced by Signex-native paths.
- Workspace `Cargo.toml` no longer mentions `kicad-parser` / `kicad-writer`.
- All test fixtures that loaded `.kicad_sch` now load `.snxsch` (already migrated in Phase 3.2).

After this phase, the main repo contains zero KiCad-derived code.

### Phase 6 — v0.7.0 / v0.8.0 release remediation (½ day)

- Edit the v0.7.0 + v0.8.0 GitHub Release notes: "This release contained KiCad-derived code (kicad-parser, kicad-writer) shipped under Apache-2.0 in error. From v0.9.0 onwards, KiCad I/O is moved to a separate GPL-3.0-licensed companion tool. Users seeking KiCad migration should install [signex-kicad-import](https://github.com/alplabai/signex-kicad-import) alongside Signex Community."
- Mark v0.7.0 / v0.8.0 binaries as superseded in the GitHub release table.
- Don't delete them (legitimate downloads happened) but flag clearly.

### Phase 7 — Public communication + Seth follow-up (1 day)

- Reply to issue #62: "Thanks for the audit list. We've moved KiCad I/O out of the Apache repo into a separate GPL-3.0 companion tool ([link]). The Apache `signex` core no longer contains KiCad-derived code. v0.7.0/v0.8.0 release notes updated to reflect the historical situation. Closing the issue if you confirm the resolution is satisfactory."
- README rewording: "Signex is open EDA tooling with optional KiCad migration via the [companion converter]." The "KiCad-compatible" claim is preserved via the companion tool reference, not the main binary.
- Discord / discussions / website (`signex.dev`) updates to match.

### Phase 8 — Clean-room development discipline (ongoing)

For the companion tool — and any future Apache-side work touching file formats — establish a documented clean-room process:

- Every PR header states: "this PR was authored by [who] with reference to [allowed sources]."
- Allowed Apache-side sources: published format specs (KiCad's `dev-docs/file-formats/` if used), Wikipedia, your own prior Signex code.
- For LLM-assisted Apache-side work: prompt deliberately specifies "do not consult or reproduce KiCad source." Track which contributions are LLM-aided.
- Companion-tool work is unconstrained (it's GPL anyway).

### Phase 9 — CI + governance (1 day)

- Add a CI guard that fails any PR adding `kicad`, `KiCad`, `F_CU`, `B_CU`, `F_SILKS`, `Net-(`, etc. anywhere in `crates/` of the main repo (after extraction).
- Add `CONTRIBUTING.md` section: "Patches must not include KiCad-derived code. Use the companion tool repo for that."
- Add a `cargo-deny` license allowlist that excludes GPL-3.0 from the main workspace's dependency closure.

---

## 5. Effort summary

| Phase | Calendar | Engineer-weeks |
|---|---|---|
| 0 — Decision + announce | ½ day | 0.1 |
| 1 — File-by-file audit | 1 week | 1.0 |
| 2 — signex-types Apache-clean | 2 weeks | 2.0 |
| 3 — Native formats | 3 weeks | 3.0 |
| 4 — Companion tool | 3 weeks | 3.0 |
| 5 — Drop KiCad I/O | 1 week | 1.0 |
| 6 — Release remediation | ½ day | 0.1 |
| 7 — Communication | 1 day | 0.2 |
| 8 — Clean-room discipline | ongoing | 0.2 (setup) |
| 9 — CI guards | 1 day | 0.2 |
| **Total** | **~10–11 weeks** | **~10.8 engineer-weeks** |

---

## 6. Risk register

### 6.1 Product narrative shift

Today's README says "Signex is a KiCad-compatible schematic and PCB editor." After Phase 5, that's no longer accurate inside the main repo. The companion tool covers migration but not interop. Need to decide whether your value prop is "drop-in KiCad replacement" (Apache-incompatible long-term) or "modern EDA tool with KiCad migration" (Apache-compatible). The latter is a softer pitch but legally clean.

**Mitigation**: Phase 7 deliberately reworks the messaging. The "KiCad-compatible" promise is preserved via the migration tool, just with a one-step setup instead of native open-and-edit.

### 6.2 Phase 2.2 — PinDirection still considered derivative

Even with the curated variant set, a strict reading could argue you started from KiCad's enum.

**Mitigation**: document the design rationale in `crates/signex-types/docs/pin-design.md`, ship the diff publicly so the curation is auditable, commit to the Markdown markup in 2.3 (which is unmistakably non-KiCad), make the variant set materially different (different size, different boundaries, different emergent semantics).

### 6.3 Phase 4 — LLM contamination

If the companion tool is also AI-assisted, the LLMs that wrote the original kicad-parser get reused. That's fine (the companion tool is GPL anyway), but document explicitly that any LLM that consulted KiCad source is "tainted" and only contributes to the GPL repo from this point.

**Mitigation**: Phase 8's clean-room discipline. The PR header self-declaration creates an audit trail. For Apache-side LLM use, prompts deliberately exclude KiCad source ("do not consult or reproduce KiCad source").

### 6.4 Phase 3.3 — User-experience cost

Existing users with `.kicad_sch` projects will need to run the companion tool to convert. First-run wizard helps, but some friction is unavoidable. Some users will be unhappy.

**Mitigation**: GitHub-wiki / changelog framing matters. Explain the licensing reason transparently — most OSS users respect the upstream constraint when they understand why.

### 6.5 Phase 1 — Scoping risk

Seth's exhaustive list might include more than this audit found (e.g., specific algorithm patterns not yet checked). Don't commit to the full timeline before reviewing his list.

**Mitigation**: Phase 0 asks for Seth's list before sizing. Phase 1 expands the audit.

### 6.6 Existing users between v0.7.0 and v0.9.0

v0.7.0 and v0.8.0 binaries shipped under Apache-2.0 with KiCad-derived code. Some users have already downloaded them. Seth could push for retraction.

**Mitigation**: Phase 6 marks them superseded in the release notes with the explanation. Don't delete (preserves legitimate use); flag clearly. If Seth wants stronger remediation, negotiate.

---

## 7. Optional: third-party KiCad parsers (worth checking before Phase 4)

Before committing to the GPL companion tool with our own kicad-parser, check crates.io for an existing **MIT/Apache-licensed** Rust KiCad parser someone else has done clean-room. If one exists and is maintained, the companion tool could be Apache-licensed too, and the two repos could potentially consolidate back into one Apache codebase.

Worth a 30-minute search before Phase 4.1:

- `cargo search kicad`
- crates.io tags: `kicad`, `eda`, `schematic`
- GitHub topic search: `kicad parser rust`

If nothing exists, Phase 4 proceeds as planned with our own kicad-parser/kicad-writer in the GPL companion tool.

---

## 8. Recommendation

The Apache-only path is doable but it's **2.5× the work of dual-licensing** and changes the product story (dropping native KiCad open-and-edit; replaced by one-time migration). If you're committed, the 10–11 week timeline is realistic.

Suggested commit order:

1. **Phase 0** today — reply to Seth, set the timeline expectation.
2. **Phase 1** next week — finish auditing against Seth's list, lock in scope.
3. **Phase 4 (companion tool)** in parallel with **Phase 2 (signex-types rewrite)** — independent work, independent contributors if you're hiring.
4. **Phase 3 (native formats)** depends on Phase 2 completion.
5. **Phase 5 (drop KiCad I/O)** is the cutover — only after Phases 2/3/4 are stable.
6. **Phase 6/7** as the final closing.

---

## 9. Decision log

This file lives in `.claude/PRPs/issue-62-licensing-remediation.md`. Updates as the plan evolves should be appended below.

| Date | Decision / change | By |
|---|---|---|
| 2026-04-29 | Plan drafted in response to issue #62. Apache-only path chosen over dual-licensing. | Caner + Claude |
