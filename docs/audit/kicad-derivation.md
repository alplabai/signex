# KiCad-derivation audit — issue #62

**Audit basis:** `audit-baseline-2026-04-29` tag (`dev` tip `0e74ebc`),
plus the strategy doc at `.claude/PRPs/issue-62-licensing-remediation.md`
§2.

**Method:** Token-level grep across `crates/`, cross-referenced against
the public KiCad source mirror. Each item scored:

- **Strong** — verbatim or near-verbatim match against KiCad source; clear-cut derivation.
- **Partial** — same syntactic shape but different surface; arguable.
- **Independent** — Signex-original or generic enough that derivation can't be inferred.

**Remediation legend:**

- **Move-to-companion** — relocate to `signex-kicad-import` GPL-3.0 companion repo.
- **Rewrite-clean-room** — design Signex-curated replacement, retire KiCad-flavoured original.
- **Delete** — drop entirely, replaced by Signex-original behaviour.
- **Keep** — generic enough to remain in the Apache main repo.

---

## Summary by remediation choice

| Remediation | Items | Phase |
|---|---|---|
| Move-to-companion | `crates/kicad-parser/` (whole crate, 3,938 LOC); `crates/kicad-writer/` (whole crate, 2,274 LOC); `crates/signex-output/src/netlist/kicad_sexpr.rs` (336 LOC) | Phase 4 (clone) + Phase 5 (delete from main) |
| Rewrite-clean-room | `signex-types::layer::{F_CU,…}` constants → `SignexLayer` enum; `signex-types::schematic::PinElectricalType` → `PinDirection`; `signex-types::schematic::PinShape` → `PinShapeStyle`; `signex-types::markup::parse_markup` → `parse_signex_markup` (Markdown-extension); `signex-types::markup::kicad_auto_net_name_from_pins` → `auto_net_name` (`unnamed-<sheet>:<ref>:<pin>` format) | Phase 2.1, 2.2, 2.3 |
| Delete | KiCad-format auto-net-name format string `Net-(<r>-Pad<p>)` (cannot be retained even renamed — direct format-string match) | Phase 2.3 |
| Keep | `signex-types::layer::LayerId(u8)` newtype itself (just a u8 wrapper, generic); `signex-types::layer::LayerKind` enum (Signex-original variant set); `kicad-parser::sexpr` (generic Lisp parser, but moves with the crate to companion); ERC rule kinds (Altium-flavoured); coordinate system; net types; theme types; almost all of `signex-render`, `signex-engine`, `signex-app`, `signex-erc`, `signex-output` (excluding netlist exporter), `signex-widgets`, `signex-library` | (no action) |

---

## Item-by-item findings

### 1. `crates/kicad-parser/` — whole crate

- **Lines:** 3,938 LOC across `lib.rs`, `schematic.rs`, `pcb.rs`, `symbol_lib.rs`, `sexpr.rs`, …
- **KiCad reference:** Implements KiCad's S-expression file formats (`.kicad_sch`, `.kicad_pcb`, `.kicad_sym`). Token names match KiCad's grammar (`effects`, `lib_id`, `sym_name`, `stroke`, `pin_names`, `hide`, …).
- **Match:** **Strong.** Format-spec-only clean-room not credible given AI-assisted authorship.
- **Remediation:** **Move-to-companion** in Phase 4; **Delete from main** in Phase 5.

### 2. `crates/kicad-writer/` — whole crate

- **Lines:** 2,274 LOC across `lib.rs`, `sexpr_render.rs`, `schematic.rs`, `pcb.rs`.
- **KiCad reference:** Mirror of #1 for output direction.
- **Match:** **Strong** (same reasoning).
- **Remediation:** **Move-to-companion** + **Delete from main** (same phases as #1).

### 3. `crates/signex-output/src/netlist/kicad_sexpr.rs`

- **Lines:** 336.
- **KiCad reference:** `eeschema/netlist_exporters/`; emits KiCad netlist S-expression format.
- **Match:** **Strong** (direct format target, uses `kicad-parser`'s sexpr_builder).
- **Remediation:** **Move-to-companion** in Phase 4 (in the same companion or a future `signex-kicad-export` sibling); **Delete from main** in Phase 5. Other netlist formats (`signex-output::netlist::xml`, `…::orcad`, etc.) stay — they're independent of KiCad.

### 4. `signex-types::schematic::PinElectricalType` (12 variants)

- **Lines:** `crates/signex-types/src/schematic.rs:91-106` (definition); 72 `PinElectricalType::*` variant uses across 7 files (`signex-types`, `signex-output`, `signex-erc`, `signex-erc-dsl` — kicad-parser/writer use sites disappear with the crates in Phase 5).
- **KiCad reference:** `common/pin_type.h::ELECTRICAL_PINTYPE` enum.
- **Match:** **Strong.** Same 12-variant set, identical canonical lower-snake-case strings (`tri_state`, `power_in`, `open_collector`).
- **Evidence:** `GetCanonicalElectricalTypeName()` returns identical strings to Signex's `#[serde(rename_all = "snake_case")]` output.
- **Remediation:** **Rewrite-clean-room** (Phase 2.2). Replace with `PinDirection` — curated 13-variant set with `OpenDrain { polarity }`, `Differential`, `Clock`, `GroundReference` as Signex-original additions and `OpenCollector`/`OpenEmitter` collapsed.

### 5. `signex-types::schematic::PinShape` (9 variants)

- **Lines:** `crates/signex-types/src/schematic.rs:110-120` (definition); 36 `PinShape::*` variant uses across 5 files (4 of which are kicad-parser/writer + render/output).
- **KiCad reference:** `pin_type.h::GRAPHIC_PINSHAPE`.
- **Match:** **Strong.** 9 variants, same naming convention (`Line`, `Inverted`, `Clock`, `InvertedClock`, `InputLow`, `ClockLow`, `OutputLow`, `EdgeClockHigh`, `NonLogic`).
- **Remediation:** **Rewrite-clean-room** (Phase 2.2). Replace with `PinShapeStyle` — 7 variants, includes `Schmitt`/`Hysteresis*` as Signex-original, drops `EdgeClockHigh`.

### 6. `signex-types::layer::{F_CU, B_CU, F_SILKS, …}` constants

- **Lines:** `crates/signex-types/src/layer.rs:28-43` (16 KiCad-numbered `pub const`); the file's only KiCad-derived content. The `LayerId(pub u8)` newtype + `LayerKind` enum are independent.
- **KiCad reference:** Pre-KiCad-7 `PCB_LAYER_ID` numbering (current KiCad master uses different numbers; Signex's bit-exact match is to KiCad 5/6).
- **Match:** **Strong** (older KiCad release, but still derivative).
- **Important scope correction:** The constants **are not propagated** through the rest of the workspace — `git grep F_CU|B_CU|… -- crates/` returns hits only inside `crates/signex-types/src/layer.rs` itself. `crates/signex-types/src/pcb.rs` stores layer references as `Vec<String>` / `String`, not `Vec<LayerId>`. The strategy doc's "80–120 sites" estimate was wrong; actual scope is "rewrite this one file."
- **Remediation:** **Rewrite-clean-room** (Phase 2.1). Replace constants + `DEFAULT_LAYER_COLORS` table with `SignexLayer` enum + semantic colour map.

### 7. `signex-types::markup::parse_markup` + `kicad_auto_net_name_from_pins`

- **Lines:** `crates/signex-types/src/markup.rs` (whole module). 16 `Net-(` / `kicad_auto_net_name` references across 4 files (signex-types, signex-render text, signex-output expression + netlist).
- **KiCad reference:**
  - `parse_markup` uses KiCad's `~{}` / `^{}` / `_{}` curly-brace syntax (`include/markup_parser.h`).
  - `kicad_auto_net_name_from_pins` produces `Net-(<r>-Pad<p>)` matching KiCad's `connection_graph.cpp::driverName` → `GetDefaultNetName()`. Function name even contains "kicad_".
- **Match:** **Strong.** Most clear-cut derivation evidence in the codebase.
- **Remediation:** **Rewrite-clean-room** + **Delete** (Phase 2.3).
  - `parse_markup` → `parse_signex_markup` (Markdown subset: `**bold**`, `*italic*`, `~~strike~~`, `^sup^`, `~sub~`, `[label](url)`, `\X` escape; per Q1 decision **adds `_~text~_` for overbar** to preserve digital-logic naming).
  - `kicad_auto_net_name_from_pins` deleted; replaced with `auto_net_name` returning `unnamed-<sheet>:<ref>:<pin>` (KiCad format string cannot be kept — direct match).

### 8. `signex-erc::RuleKind` (11 ERC rule kinds)

- **Lines:** various across `crates/signex-erc/`.
- **KiCad reference:** KiCad's `ERCE_*` set.
- **Match:** **Partial.** Doc comment says "Altium ERC matrix conventions"; rule names + impls are independent. Some semantic overlap with KiCad's set, but that's expected for any ERC system.
- **Remediation:** **Keep.** Independent enough.

### 9. `crates/kicad-parser/src/sexpr.rs` — generic S-expression lexer

- **Lines:** 376.
- **KiCad reference:** None. Generic Lisp-style lexer with `Atom::Raw` / `Atom::Quoted`. Could survive clean-room scrutiny.
- **Match:** **Independent.**
- **Remediation:** Moves with the rest of `kicad-parser` to the companion repo (Phase 4); not needed in main repo because main repo no longer parses KiCad. If a future Apache use case for generic S-expressions arises, this file's logic can be lifted out under a clear permissive licence claim.

### 10. Independent / no remediation

| Component | Reasoning |
|---|---|
| `signex-types::coord` (i64-nm coordinate system, transforms) | Generic; not KiCad-specific. |
| `signex-types::net` (Net types) | Generic graph theory; Signex-curated for AI-readiness. |
| `signex-types::theme` (theme types) | 6 built-in themes, Signex-curated palette including Altium Dark default. |
| `signex-types::project` (project types) | Signex-native `.snxprj` schema. |
| `signex-types::property` (component property types) | Independent design. |
| `signex-types::violation` (ERC/DRC violation types) | Altium-flavoured. |
| `signex-render` (~all of it) | Iced/wgpu rendering; bridges Signex types to draw calls. |
| `signex-engine` | Command/event/undo bus, multi-window engine map. |
| `signex-app` | Iced application, panels, dock system, menus, Active Bar, canvas. |
| `signex-erc` | Validation rules with Altium-flavoured naming. |
| `signex-output` (excluding netlist/kicad_sexpr.rs) | PDF, BOM, other netlist formats — independent code. |
| `signex-widgets` | Custom Iced widgets (TreeView, symbol preview, theme extensions). |
| `signex-library` | Signex-native library subsystem (`.snxlib`/`.snxsym`/`.snxfpt`). |

---

## Items needing follow-up if Seth provides a more granular list

The strategy doc anticipated Seth supplying an exhaustive list. As of
the autonomous remediation run (2026-04-29) we have not received it,
so this audit is based on the structural comparison in the strategy
doc plus token-level greps against the codebase. If Seth identifies
items not covered above, append them here with the same five fields
and choose a remediation; the License Guard CI in Phase 2.4 enforces
the new state, and Phase 5 is the cutover.

## Reproducing the audit

```bash
git grep -nE "kicad|KiCad|F_CU|B_CU|F_SILKS|F_MASK|F_PASTE|F_FAB|F_CRTYD|EDGE_CUTS|tri_state|Net-\(" -- crates/
git grep -l "use kicad_parser\|use kicad_writer" -- crates/
git grep -l "PinElectricalType\|PinShape\b" -- crates/
git grep -l "F_CU\|B_CU\|F_SILKS\|LayerId" -- crates/
git grep -l "parse_markup\|kicad_auto_net_name" -- crates/
```
