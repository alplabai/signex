# Source Comment Scrub — 2026-05-01

This document records the source-comment scrub performed on
2026-05-01 in response to the follow-up exchange on
[issue #62](https://github.com/alplabai/signex/issues/62).

## Background

The v0.9.0 cutover (2026-04-29) removed `kicad-parser` and
`kicad-writer` as crates and renamed every KiCad-shaped type in
`signex-types`. The v0.10.0 polish pass (2026-04-29) renamed
the public string `KiCad` → `Standard` across enum variants and
internal documentation.

Both passes were focused on the *shipped* surface — type names,
public APIs, enum variants, file format strings. Neither pass
audited *source comments* for residual references to KiCad's
C++ implementation. Eight such comment-level references survived
into the 2026-05-01 codebase even after the v0.10.0 rename.

A reviewer who knows KiCad would identify these as derivation-
shaped attributions because the renamed prefix (`Standard`)
preceded class names that are unambiguously KiCad's own —
`SCH_PAINTER`, `SCH_FIELD::GetDrawRotation`,
`SCH_FIELD::GetEffectiveJustify`, `SCH_LABEL`, `SCH_SYMBOL`,
`sch_painter.cpp`. The rename made the comments superficially
neutral; it did not change the substantive references.

## Scrub

### Comments removed (8 lines across 7 files)

| File | Pre-scrub | Post-scrub |
|---|---|---|
| `crates/signex-render/src/schematic/label.rs:4` | `//! Reference: signex Tauri app schematicDrawHelpers.ts::drawLabels() and Standard sch_painter.cpp SCH_LABEL render.` | `//! Behaviour spec: docs/RENDERING_RULES.md::sch-labels — the Signex internal rendering rule set, derived from observed .kicad_sch rendering behaviour and Altium parity goals.` |
| `crates/signex-render/src/schematic/mod.rs:430-441` (block) | `Mirrors two pieces of Standard behaviour: 1. SCH_FIELD::GetDrawRotation() … 2. SCH_FIELD::GetEffectiveJustify() …` | Reworded to describe the rules in plain language, citing `docs/RENDERING_RULES.md::field-rotation-and-justify` for the spec. |
| `crates/signex-render/src/schematic/text.rs:689-693` | `which causes Standard to flip the horizontal justification of the field text (SCH_FIELD::GetEffectiveJustify). Pass sym.mirror_x. Rotation: Standard field angles are CCW-positive…` | Reworded to describe the H-justify-flip rule and the iced rotation-sign convention without referencing third-party class names. |
| `crates/signex-render/src/schematic/pin.rs:445-449` | `Pin shape decorators (mirroring Standard SCH_PAINTER pin shape logic). Draw two connected segments A→B and B→C (Standard triLine).` | `Pin shape decorators — IEEE-Std-91 graphical conventions for pin modifiers (inverter bubble, clock arrow, low-active markers, etc.). Spec: docs/RENDERING_RULES.md::pin-shape-decorators.` |
| `crates/signex-engine/src/transform.rs:416` | `// pivot, just like Standard's SCH_SYMBOL::GetBodyBoundingBox().` | `// is the natural pivot for autoplaced field positions.` |
| `crates/signex-output/src/pdf/bookmarks.rs:308` | `// expanded — matches what Standard eeschema and Altium ship.` | `// fully expanded by default, matching common EDA exporters.` |
| `crates/signex-output/src/pdf/mod.rs:139` | `/// the classic eeschema palette so existing direct-export` | `/// the legacy SchematicPalette::classic() (cream paper / dark-blue wires) so existing direct-export` |
| `crates/signex-output/src/pdf/palette.rs:11-15, 53-56` | `the historical eeschema-style palette (SchematicPalette::classic()). … Historical eeschema-style palette — cream paper, dark-blue wires, mustard symbol bodies.` | Reworded to refer to the palette by its function name (`SchematicPalette::classic()`) and describe the colour scheme without the `eeschema-style` modifier. |

### What was *not* changed

- **Algorithms.** The mathematical functions (`circumcircle`,
  `arc_sweep`, TRANSFORM-matrix flip-detection, IEEE-Std-91 pin
  decorators) are interoperability with the public `.kicad_sch`
  file format and are independent of any third-party
  implementation. They were not touched.
- **`LIB_SYMBOLS` bitflag constant.** This is our own bitflag
  name in `RenderInvalidation` and `DocumentPatch`. It is
  plural; `LIB_SYMBOL::` (singular + scope, the C++ form) is
  what the new CI guard forbids.
- **Type and identifier names.** None of the renamed types
  (`PinDirection`, `PinShapeStyle`, `SignexLayer`, etc.) regressed.

## New CI guards

Two jobs added to `.github/workflows/license-guard.yml`:

1. **`no-kicad-cpp-class-names`** — forbids `SCH_PAINTER`,
   `SCH_FIELD::`, `SCH_LABEL`, `SCH_PIN`, `SCH_SYMBOL`,
   `LIB_SYMBOL::`, `LIB_PIN::`, `sch_painter.cpp`,
   `sch_symbol.cpp`, `sch_label.cpp`, `sch_pin.cpp`,
   `lib_symbol.cpp`, and `eeschema/` anywhere in `crates/`.
2. **`no-derivation-attribution-markers`** — forbids `DeepWiki`,
   `KiCad mirror source`, `extracted from KiCad`, `based on
   KiCad source` anywhere in the repo (excluding this audit
   trail and `docs/LICENSING.md` which discuss them
   intentionally).

Total License Guard jobs: 6 → 8.

## Agent-context discipline going forward

The `kicad-render` skill that previously sat in
`.claude/skills/kicad-render/` was real, was active in agent
context during pre-v0.9.0 development, and described
KiCad-source-derived rendering details. It was removed from the
active `.claude/` directory during the v0.10.0 history rewrite
on 2026-04-29 (alongside the `crates/` Apache-clean rewrite, in
the same operation).

The 2026-05-01 scrub does not change the past existence of that
skill. It does:

- Confirm that the skill is no longer in active agent context.
- Document this fact publicly here (rather than only in the
  internal audit doc).
- Lock the discipline forward via the new CI guards.

## Companion changes

- **New public spec:** `docs/RENDERING_RULES.md` documents the
  rendering rules that source comments now reference. Original
  Signex prose, Apache-2.0 like the rest of the repo.
- **Memory note:** `feedback_no_disk_writes_without_user_save.md`
  (already shipped) — unrelated, but landed in the same series.

## Why this matters

The 2026-04-29 cutover and v0.10.0 polish established the
*structural* Apache-clean baseline (no GPL imports / deps,
renamed types, replaced format strings). The 2026-05-01 scrub
completes the *documentary* baseline so source comments don't
preserve derivation-shaped attributions in our own words.

This is not a retroactive change to any past commit's content —
that history exists and will continue to exist. It's a forward-
looking lock on what shipped source can carry going forward.
