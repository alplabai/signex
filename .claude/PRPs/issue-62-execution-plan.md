# Issue #62 — Apache-only execution plan

**Status**: ready to execute. Companion to `issue-62-licensing-remediation.md` (the strategy doc).
**Goal**: every commit below moves the main `signex` repo closer to "zero KiCad-derived code, Apache-2.0 clean" while preserving KiCad migration via the new `signex-kicad-import` GPL-3.0 companion repo.
**Branch strategy**: every phase lives on `chore/apache-clean-<phase>` off `dev`, merged via PR. v0.9 work parks until Phase 5 cutover.

---

## 0. Pre-flight (½ day, do before any code change)

### 0.1 Confirm contributor consent

```bash
git log --since=2024-01-01 --pretty="%aN <%aE>" | sort -u
```

Every contributor whose code lands in the rewritten files must consent to the Apache-only direction (their patches stay under their original Apache-2.0 grant — no relicense, just confirmation). Expected list as of 2026-04-29: Caner + Hakan + LLM-tagged commits. Document the list in `docs/audit/contributors-2026-04-29.md`.

### 0.2 Snapshot the current state

```bash
git tag --no-sign audit-baseline-2026-04-29 dev
git push origin audit-baseline-2026-04-29
```

Permanent audit snapshot. Useful if Seth ever asks "what was in the repo at the time of issue #62."

### 0.3 Park v0.9 work

```bash
git checkout feature/v0.9-snxlib-as-file
git tag --no-sign v0.9-snxlib-paused-2026-04-29
```

Don't push v0.9 until Phase 5 is in. The 19 v0.9 commits stay local + tagged; resume after the cutover when the codebase is Apache-clean.

### 0.4 Search for clean-room third-party KiCad parsers

Spend 30 minutes before Phase 4:

```bash
cargo search kicad
cargo search kicad-parser
# Check: https://crates.io/keywords/kicad
# Check: https://github.com/topics/kicad?l=rust
```

If a maintained MIT/Apache-licensed Rust KiCad parser exists with sufficient feature coverage (parses `.kicad_sch`/`.kicad_pcb`/`.kicad_sym` losslessly), the companion tool can stay Apache and Phase 4 simplifies to "depend on $CRATE." Document the search results in `docs/audit/third-party-kicad-parsers.md` regardless of outcome.

### 0.5 Acceptance criteria for the entire effort

The remediation is "done" when:

- `git grep -i "kicad\|F_CU\|B_CU\|F_SILKS\|tri_state\|Net-(" -- crates/` returns **zero results** in the main `signex` repo.
- `cargo deny check licenses` passes with a config that admits no GPL-3.0 dependencies.
- `signex-kicad-import` companion repo exists with its own `LICENSE-GPL-3.0`, builds, ships at least one binary release, and the converter round-trips a representative KiCad demo project to a `.snxsch` that Signex Community can open.
- v0.7.0 / v0.8.0 GitHub Releases are flagged as superseded with the licensing note.
- README + website + Discord describe Signex as "open EDA tooling with optional KiCad migration via [companion converter]" — not "KiCad-compatible editor".
- Issue #62 is closed by Seth (or by Caner with Seth's confirmation comment).

---

## Phase 1 — File-by-file audit (1 week)

### 1.1 Inputs

- Seth's exhaustive list (received from issue #62 reply).
- The audit findings already documented in `issue-62-licensing-remediation.md` §2.

### 1.2 Steps

```bash
git checkout -b chore/audit-completion dev
mkdir -p docs/audit
```

For each item Seth identifies:

1. Map to a file path in this repo.
2. Compare against the cited KiCad source.
3. Score derivation: **Strong** / **Partial** / **Independent**.
4. Append to `docs/audit/kicad-derivation.md` with:
   - File path
   - Lines in scope
   - KiCad source reference (file + lines)
   - Match strength + evidence
   - Remediation choice: **Remove** / **Rewrite-clean-room** / **Move-to-companion-repo**

Template entry:

```markdown
### `signex_types::schematic::PinElectricalType`

- **Lines**: `crates/signex-types/src/schematic.rs:91-106`
- **KiCad reference**: `common/pin_type.h::ELECTRICAL_PINTYPE` enum
- **Match**: Strong — 12 variants, same canonical lower-snake_case tokens (`tri_state`, `power_in`, `open_collector`).
- **Evidence**: `GetCanonicalElectricalTypeName()` returns identical strings to Signex's `#[serde(rename_all = "snake_case")]` output.
- **Remediation**: Rewrite-clean-room. Replace with `PinDirection` (Phase 2.2 — Signex-curated variant set with `OpenDrain`, `Differential`, etc. that don't 1:1 map to KiCad's enum).
```

### 1.3 Acceptance

- `docs/audit/kicad-derivation.md` covers every item from Seth's list.
- Every Tier 2 ambiguity is resolved into a definitive Strong / Partial / Independent score.
- A summary table at the top of the audit doc lists items grouped by remediation choice.

### 1.4 Commit

```
git add docs/audit/
git commit -m "docs(audit): file-by-file KiCad derivation audit (issue #62)"
```

PR `chore/audit-completion` → `dev`. Merge once reviewed.

---

## Phase 2 — Signex-native types in `signex-types` (2 weeks)

### 2.1 Layer abstraction (~3 days)

**Branch**: `chore/apache-clean-layer` off `dev`.

#### 2.1.1 Add new types

**New file**: `crates/signex-types/src/pcb/layer.rs`

```rust
//! Signex-native PCB layer abstraction.
//!
//! Independent of KiCad's PCB_LAYER_ID numbering. Variants are
//! semantic — they describe the layer's purpose, not its bit
//! position in any particular EDA tool's internal LSET. Concrete
//! `u8` IDs for KiCad I/O are produced by the `signex-kicad-import`
//! companion crate's translation layer; they don't live in this
//! Apache codebase.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignexLayer {
    TopCopper,
    BottomCopper,
    /// Stacked between Top and Bottom; user provides 1-based index.
    InnerCopper(u8),
    TopSilk,
    BottomSilk,
    TopSolderMask,
    BottomSolderMask,
    TopPaste,
    BottomPaste,
    TopAssembly,
    BottomAssembly,
    TopCourtyard,
    BottomCourtyard,
    BoardOutline,
    KeepOut,
    Mechanical(u8),
    User(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    Copper,
    Silk,
    Mask,
    Paste,
    Assembly,
    Courtyard,
    Outline,
    KeepOut,
    Mechanical,
    User,
}

impl SignexLayer {
    pub fn kind(self) -> LayerKind { /* … */ }

    /// Display label for the Signex UI (Altium-flavoured nomenclature
    /// per `docs/UX_REFERENCE_ALTIUM.md`). Independent of the layer's
    /// type-system identity.
    pub fn altium_label(self) -> String { /* … */ }

    /// Iterate the canonical layer set in stable display order.
    pub fn all() -> impl Iterator<Item = SignexLayer> { /* … */ }
}
```

#### 2.1.2 Delete the old

**File**: `crates/signex-types/src/layer.rs`

Delete the file. The `LayerId(u8)` newtype and every `F_CU` / `B_CU` / `F_SILKS` const goes.

#### 2.1.3 Migrate every call site

```bash
git grep -l "LayerId\|F_CU\|B_CU\|F_SILKS\|F_MASK\|F_PASTE\|F_FAB\|F_CRTYD\|EDGE_CUTS\|MARGIN\|DWGS_USER\|CMTS_USER\|ECO1_USER\|ECO2_USER" -- crates/
```

Expected hits (from earlier audit):
- `crates/signex-types/src/pcb.rs` — `Pad.layers: Vec<LayerId>` → `Vec<SignexLayer>`
- `crates/signex-render/src/pcb/*` — colour map keys
- `crates/signex-render/src/colors.rs` — theme map
- `crates/signex-engine/src/pcb/*` — engine layer ops
- `crates/signex-output/src/pdf/*` — PDF layer rendering
- `crates/signex-output/src/svg/*` — SVG layer rendering
- `crates/signex-app/src/library/editor/footprint/layers.rs` — UI picker

For each: replace `LayerId(N)` with `SignexLayer::TopCopper` (etc.). The mapping is documented in the audit doc.

#### 2.1.4 Tests

```bash
cargo test --workspace
# Specific layer-related tests:
cargo test -p signex-types layer
cargo test -p signex-render pcb
```

#### 2.1.5 Commit

```
git add -A
git commit -m "refactor(types): replace LayerId(u8) with semantic SignexLayer enum

Stage 2.1 of the issue-62 Apache-clean remediation. The KiCad-
inherited PCB_LAYER_ID numbering (F_CU=0, B_CU=31, …) is removed
from signex-types in favour of a Signex-native semantic enum.
KiCad I/O — when it returns via the signex-kicad-import companion
repo — translates u8 IDs to/from SignexLayer at the boundary.

Touches ~80 sites across types/render/engine/output/app. No user-
facing behaviour change; the rendering, picking, and persistence
all keep working through the new enum's altium_label() / kind()
methods."
```

PR → `dev`. Tests must be green.

### 2.2 Pin enum redesign (~2 days)

**Branch**: `chore/apache-clean-pins` off `dev` (after 2.1 lands).

#### 2.2.1 Add new types

**New file**: `crates/signex-types/src/schematic/pin.rs`

```rust
//! Signex-native schematic pin types.
//!
//! Curated variant set — not a 1:1 rewrite of KiCad's
//! ELECTRICAL_PINTYPE / GRAPHIC_PINSHAPE enums. The differences
//! are intentional and documented in `docs/pin-design.md`.

use serde::{Deserialize, Serialize};

/// Pin direction / electrical role. Curated for modern HW design;
/// not derived from any specific EDA tool's enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinDirection {
    /// Drives signal in only.
    Input,
    /// Drives signal out only.
    Output,
    /// Drives signal both ways depending on context.
    Bidirectional,
    /// Tri-statable output — can be high-Z.
    ThreeStatable,
    /// Passive electrical (resistor, capacitor, inductor terminal).
    Passive,
    /// Open-drain / open-collector output, polarity-tagged.
    OpenDrain { polarity: DrainPolarity },
    /// Power supply input.
    PowerInput,
    /// Power supply output (regulator output, battery positive, etc.).
    PowerOutput,
    /// Ground reference.
    GroundReference,
    /// Differential pair member — Signex extension for HSD.
    Differential { polarity: DiffPolarity },
    /// Clock input — not a shape detail, a directional kind.
    Clock,
    /// Pin must remain unconnected (manufacturer-marked NC).
    DoNotConnect,
    /// User has not classified it yet.
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DrainPolarity { ActiveLow, ActiveHigh }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffPolarity { Positive, Negative }

/// Pin graphic style — visual decoration on the symbol pin tip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PinShapeStyle {
    Plain,
    InvertedBubble,
    ClockTriangle,
    InvertedClockBubble,
    HysteresisInput,
    HysteresisOutput,
    Schmitt,
}
```

The diff against KiCad's enum is meaningful:

| KiCad ELECTRICAL_PINTYPE | Signex PinDirection |
|---|---|
| PT_INPUT | Input |
| PT_OUTPUT | Output |
| PT_BIDI | Bidirectional |
| PT_TRISTATE | ThreeStatable |
| PT_PASSIVE | Passive |
| PT_NIC | (no equivalent — DoNotConnect or Unclassified) |
| PT_UNSPECIFIED | Unclassified |
| PT_POWER_IN | PowerInput |
| PT_POWER_OUT | PowerOutput |
| PT_OPENCOLLECTOR | OpenDrain { ActiveLow } |
| PT_OPENEMITTER | OpenDrain { ActiveHigh } |
| PT_NC | DoNotConnect |
| **(none in KiCad)** | GroundReference *(Signex extension)* |
| **(none in KiCad)** | Differential *(Signex extension)* |
| **(none in KiCad)** | Clock *(Signex extension — moved from shape to direction)* |

Same with PinShapeStyle: 7 variants vs KiCad's 9, with `Schmitt` and `HysteresisInput/Output` being Signex-original.

#### 2.2.2 Delete the old

In `crates/signex-types/src/schematic.rs`:
- Delete `enum PinElectricalType { … }` (lines 91-106).
- Delete `enum PinShape { … }` (lines 110-120).
- Update every `Pin.electrical_type: PinElectricalType` field to `Pin.direction: PinDirection`.
- Update every `Pin.shape: PinShape` field to `Pin.shape_style: PinShapeStyle`.

#### 2.2.3 Migrate call sites

```bash
git grep -l "PinElectricalType\|PinShape\b" -- crates/
```

Expected ~40 sites:
- `crates/signex-engine/src/schematic/*` — pin construction + analysis
- `crates/signex-render/src/schematic/*` — pin rendering
- `crates/signex-erc/src/rules/*` — ERC pin checks
- `crates/signex-app/src/library/editor/symbol/*` — pin editor

For each: rename type + rename variants per the table above.

#### 2.2.4 Design rationale doc

**New file**: `crates/signex-types/docs/pin-design.md`

Documents the rationale for each variant, why the set differs from KiCad, and the "tainted-LLM" PR-discipline expectation.

#### 2.2.5 Tests

```bash
cargo test --workspace
cargo test -p signex-types pin
cargo test -p signex-engine schematic
cargo test -p signex-erc
```

#### 2.2.6 Commit

```
refactor(types): replace KiCad-derived pin enums with Signex-native PinDirection / PinShapeStyle

Stage 2.2 of issue-62 remediation. The KiCad-inherited 12-variant
ELECTRICAL_PINTYPE / 9-variant GRAPHIC_PINSHAPE are replaced by
curated Signex-native enums:

- PinDirection collapses OpenCollector + OpenEmitter into
  OpenDrain { polarity }; adds GroundReference, Differential, Clock
  as Signex-original concepts.
- PinShapeStyle has 7 variants vs KiCad's 9, with Schmitt /
  Hysteresis* added as Signex-original.

Design rationale in crates/signex-types/docs/pin-design.md. The
diff against KiCad's enum is intentional and auditable — different
size, different boundaries, different emergent semantics.

Touches ~40 sites; no user-facing behaviour change.
```

### 2.3 Markup format swap (~3 days)

**Branch**: `chore/apache-clean-markup` off `dev`.

#### 2.3.1 Define Signex markup

**New file**: `crates/signex-types/src/markup.rs` (replaces existing)

```rust
//! Signex schematic-text markup.
//!
//! Markdown-extension style — a small subset of Markdown plus
//! Signex extensions for overbar/superscript/subscript.
//!
//!   `**bold**`           — bold span
//!   `*italic*`           — italic span
//!   `~~strike~~`         — strikethrough
//!   `^superscript^`      — superscript (Signex extension; not GFM)
//!   `~subscript~`        — subscript (Signex extension; not GFM)
//!   `[label](https://…)` — link
//!   `\X`                 — literal X (escape any sigil)
//!
//! NOT KiCad's `^{}` / `_{}` / `~{}` curly-brace syntax.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignexRichText {
    pub spans: Vec<RichSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RichSpan {
    Plain(String),
    Bold(Vec<RichSpan>),
    Italic(Vec<RichSpan>),
    Strike(Vec<RichSpan>),
    Superscript(Vec<RichSpan>),
    Subscript(Vec<RichSpan>),
    Link { label: Vec<RichSpan>, url: String },
}

pub fn parse_signex_markup(input: &str) -> SignexRichText { /* … */ }
pub fn render_to_plain(text: &SignexRichText) -> String { /* … */ }
```

#### 2.3.2 Delete KiCad-derived bits

- Delete `parse_markup()` (KiCad escape syntax).
- Delete `kicad_auto_net_name_from_pins()`.

#### 2.3.3 Add Signex auto net name

```rust
// crates/signex-types/src/schematic/auto_net.rs
pub fn auto_net_name(sheet: &str, ref_designator: &str, pin_number: &str) -> String {
    format!("unnamed-{sheet}:{ref_designator}:{pin_number}")
}
```

Format intentionally **not** `Net-(<r>-Pad<p>)` — the KiCad format string is the most clear-cut derivation evidence in the codebase, so we cannot keep it.

#### 2.3.4 Migrate call sites

`signex-render::schematic::text`, `signex-output::expression`, every test fixture using markup.

#### 2.3.5 Test fixture migration

Existing fixtures use KiCad markup syntax (`{slash}`, `~{}`, `^{}`, etc.). Migration:

```bash
# Discovery query:
git grep -l "\\\\\\{\\|\\^{\\|~{" -- crates/*/tests/ crates/*/test_data/
```

Convert by hand or via a one-time perl script:
```bash
# KiCad ~{X} → Signex ^X^ (overbar→superscript, since Markdown has no overbar)
# Actually Signex defines ^…^ as superscript, not overbar. Choose:
#   - Overbar in Signex spec: drop entirely (rare in modern designs), OR
#   - Add Overbar(Vec<RichSpan>) variant + use a third sigil (e.g. `_~text~_`)
# Decide before migration. Recommendation: drop overbar, replace with strike-through visual where strict overbar isn't required.
```

#### 2.3.6 Commit

```
refactor(types): swap KiCad markup for Markdown-extension Signex format

Stage 2.3 of issue-62 remediation.

- Remove parse_markup() (KiCad ~{}/^{}/_{} curly-brace syntax).
- Remove kicad_auto_net_name_from_pins() (`Net-(<r>-Pad<p>)`).
- Add parse_signex_markup() (Markdown subset + ^…^ / ~…~ extensions).
- Add auto_net_name() returning `unnamed-<sheet>:<ref>:<pin>`.

Test fixtures migrated from KiCad markup to Signex markup.
Render tests assert byte-equal output for representative inputs.
```

### 2.4 Validation gate (~½ day)

**Branch**: `chore/apache-clean-ci` off `dev`.

**New file**: `.github/workflows/license-guard.yml`

```yaml
name: License Guard
on: [push, pull_request]

jobs:
  no-kicad-derivation:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Forbid KiCad-derived strings in main repo crates
        run: |
          set -e
          if git grep -nE "kicad|KiCad|F_CU\b|B_CU\b|F_SILKS\b|F_MASK\b|F_PASTE\b|F_FAB\b|F_CRTYD\b|EDGE_CUTS\b|tri_state|Net-\(" -- crates/; then
            echo ""
            echo "::error::KiCad-derived string detected. The main repo is Apache-2.0 clean — KiCad I/O lives in the signex-kicad-import companion repo."
            exit 1
          fi
          echo "License guard passed."
```

**New file**: `deny.toml`

```toml
[licenses]
unlicensed = "deny"
copyleft = "deny"  # Forbids GPL, AGPL, etc. transitively
allow = ["Apache-2.0", "MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-DFS-2016", "OFL-1.1"]
```

#### 2.4.1 Commit

```
ci(license): forbid KiCad-derived strings + GPL transitive deps

Stage 2.4 of issue-62 remediation. Two CI gates ensure the main
signex repo stays Apache-2.0 clean post-cutover:

- License Guard workflow fails any push/PR introducing KiCad-flavoured
  identifiers (kicad, F_CU, tri_state, Net-(, …) in crates/.
- cargo-deny config rejects any GPL / AGPL / unlicensed transitive
  dependency.

Run locally: `cargo deny check licenses`.
```

---

## Phase 3 — Native `.snxsch` / `.snxpcb` formats (3 weeks)

### 3.1 Format design (~3 days)

**Branch**: `chore/apache-clean-formats` off `dev`.

#### 3.1.1 Schema

`.snxsch` (JSON):

```json
{
  "format": "snxsch/1",
  "library_id": "0192a8c0-…",
  "page_size": { "width_mm": 297.0, "height_mm": 210.0 },
  "title_block": { … },
  "sheets": [
    {
      "name": "Power Supply",
      "uuid": "…",
      "graphics": [ /* wires, junctions, labels, no-connects */ ],
      "components": [ /* placed component instances */ ]
    }
  ],
  "lib_symbols": [ /* shared symbol cache for this design */ ]
}
```

`.snxpcb` (JSON):

```json
{
  "format": "snxpcb/1",
  "library_id": "0192a8c0-…",
  "stackup": { "layer_kinds": [ "TopCopper", "InnerCopper(1)", … ] },
  "footprints": [ … ],
  "tracks": [ … ],
  "vias": [ … ],
  "zones": [ … ]
}
```

`.snxprj` already exists from earlier work — stays.

#### 3.1.2 Tests

`crates/signex-types/tests/format_round_trip.rs` — for every Signex-native data type, round-trip through JSON, assert byte-equal serialisation.

#### 3.1.3 Commit

```
feat(types): native .snxsch / .snxpcb JSON formats

Stage 3.1 of issue-62 remediation. signex-types defines the
canonical Signex schema for schematic + PCB persistence; engine
and app rewire onto it in Stages 3.2–3.3.
```

### 3.2 Engine + app rewiring (~1.5 weeks)

**Branch**: `chore/apache-clean-persistence` off `dev`.

#### 3.2.1 Engine save/load

```rust
// crates/signex-engine/src/persistence.rs
pub fn save_schematic(path: &Path, schematic: &Schematic) -> Result<(), Error> {
    let json = serde_json::to_string_pretty(schematic)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load_schematic(path: &Path) -> Result<Schematic, Error> {
    let bytes = std::fs::read(path)?;
    let schematic: Schematic = serde_json::from_slice(&bytes)?;
    Ok(schematic)
}
```

#### 3.2.2 App file dialogs

`crates/signex-app/src/app/handlers/menu/file_commands.rs` — change file picker filters from `*.kicad_sch` to `*.snxsch`. Same for `*.snxpcb`, `*.snxprj`.

#### 3.2.3 Test fixture migration

Convert ~50–100 `.kicad_sch` fixtures via the companion tool (Phase 4). Keeps CI green during migration.

Strategy: build companion tool first (Phase 4), then run it as a one-shot batch convert against all fixtures, commit the resulting `.snxsch` alongside the original `.kicad_sch` for one transition release, then delete the originals after Phase 5.

#### 3.2.4 Tests

```bash
cargo test --workspace
cargo test -p signex-engine persistence
cargo test -p signex-app file_commands
```

#### 3.2.5 Commit

```
refactor(engine,app): native .snxsch / .snxpcb persistence

Stage 3.2 of issue-62 remediation. Engine save/load go through
serde_json over the Signex-native schema. App file dialogs filter
on *.snxsch / *.snxpcb / *.snxprj only.

Test fixtures migrated from .kicad_sch via the signex-kicad-import
companion tool (Stage 4). Originals kept alongside for one transition
release; deleted in Stage 5.
```

### 3.3 First-run migration shim (~3 days)

**Branch**: `chore/apache-clean-migration-wizard` off `dev`.

When Signex Community starts and detects a project root containing `.kicad_sch` / `.kicad_pcb` files, show a modal:

```
Migrate KiCad project?

This project contains KiCad files. Signex Community reads only
Signex-native (.snxsch / .snxpcb) formats. Use the optional
companion tool to convert your project — your originals stay
intact alongside the new files.

[ Download converter… ]   [ Skip for now ]
```

The "Download converter" button opens the GitHub Releases page for `signex-kicad-import`. The converter runs as a separate CLI; Signex Community doesn't shell out to it (keeps Apache-side LLM-tainted code from leaking back in).

After conversion, the user re-opens the project; Signex sees `.snxsch`/`.snxpcb` siblings and proceeds.

**File**: new `crates/signex-app/src/library/migrations/kicad_detect.rs`.

#### 3.3.1 Commit

```
feat(app): first-run KiCad-project migration prompt

Stage 3.3 of issue-62 remediation. When opening a project root
containing .kicad_sch/.kicad_pcb files, surface a modal pointing
the user to the signex-kicad-import companion tool. Apache-side
code never invokes the converter directly (keeps LLM-tainted GPL
code from leaking back in via shell-out); the user runs the tool
themselves and re-opens.
```

---

## Phase 4 — Companion repo `signex-kicad-import` (3 weeks, parallel with Phase 2)

### 4.1 New repo (~½ day)

```bash
mkdir -p ../signex-kicad-import
cd ../signex-kicad-import
git init
gh repo create alplabai/signex-kicad-import --public --description "GPL-3.0 KiCad → Signex one-way converter"

# Seed the LICENSE
curl -sSf https://www.gnu.org/licenses/gpl-3.0.txt -o LICENSE
```

`README.md`:

```markdown
# signex-kicad-import

One-way converter from KiCad files (`.kicad_sch`, `.kicad_pcb`,
`.kicad_sym`) to Signex's native formats (`.snxsch`, `.snxpcb`).

**Licensed GPL-3.0-or-later** because it implements KiCad's file
format with structure derived from KiCad's GPL-3.0 source.

## Usage

```bash
signex-kicad-import path/to/project.kicad_pro
```

Produces `project.snxprj` + `.snxsch` / `.snxpcb` siblings. Open
the resulting `.snxprj` in [Signex Community](https://github.com/alplabai/signex)
(Apache-2.0).
```

`Cargo.toml`:

```toml
[workspace]
members = ["crates/kicad-parser", "crates/kicad-writer", "crates/cli"]

[workspace.package]
license = "GPL-3.0-or-later"
edition = "2024"
version = "0.1.0"

[workspace.dependencies]
signex-types = "0.x"  # from crates.io once published, or path-dep during dev
serde_json = "1"
uuid = "1"
```

### 4.2 Move kicad-parser + kicad-writer (~1 day)

```bash
# In the main repo:
git checkout -b chore/extract-kicad-crates dev
git rm -r crates/kicad-parser crates/kicad-writer
git rm crates/signex-output/src/netlist/kicad_sexpr.rs

# In the companion repo:
cp -r ../signex/crates/kicad-parser crates/kicad-parser
cp -r ../signex/crates/kicad-writer crates/kicad-writer
# update each crate's Cargo.toml to depend on signex-types from crates.io
git add -A
git commit -m "chore: import kicad-parser + kicad-writer from signex"
git push -u origin main
```

### 4.3 CLI converter (~1.5 weeks)

`crates/cli/src/main.rs`:

```rust
use clap::Parser;

#[derive(Parser)]
struct Cli {
    /// Path to a .kicad_pro project file.
    project: PathBuf,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let kicad_project = kicad_parser::parse_project(&args.project)?;
    let signex_project = convert_project(kicad_project)?;
    write_signex_project(&signex_project)?;
    Ok(())
}

fn convert_project(kp: kicad_parser::Project) -> Result<signex_types::project::Project> {
    // Walk every sheet, every footprint, every primitive. Produce
    // signex_types data structures using the SignexLayer /
    // PinDirection translation maps.
    todo!()
}
```

Translation maps (`crates/cli/src/translate/`):

- `layer.rs` — `kicad_layer_id_u8_to_signex(id) -> SignexLayer`
- `pin.rs` — `kicad_canonical_string_to_signex(s) -> PinDirection`
- `markup.rs` — `parse_kicad_markup_to_signex_richtext(s) -> SignexRichText`
- `auto_net.rs` — re-implement KiCad's `Net-(<r>-Pad<p>)` for use only when reading existing KiCad netlist annotations

### 4.4 Distribution (~3 days)

GitHub Actions workflow that builds release binaries on tag:

```yaml
name: Release
on:
  push:
    tags: ['v*']
jobs:
  build:
    strategy:
      matrix:
        os: [ubuntu-latest, windows-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release
      - uses: softprops/action-gh-release@v2
        with:
          files: target/release/signex-kicad-import*
```

First release: `v0.1.0` once it converts the bundled KiCad demo projects round-trip-clean.

---

## Phase 5 — Drop KiCad I/O from main repo (1 week)

**Branch**: `chore/apache-clean-cutover` off `dev`.

### 5.1 Remove imports

```bash
# In main signex repo:
sed -i 's/use kicad_parser::/use signex_types::/g' \
    crates/signex-app/src/**/*.rs \
    crates/signex-engine/src/**/*.rs \
    crates/signex-erc/src/**/*.rs \
    crates/signex-output/src/**/*.rs

# Note: this is mechanical; some uses won't compile and need manual fixes
# where kicad_parser exposed types that now live differently in signex-types.
```

### 5.2 Remove references in Cargo.toml

```bash
# Workspace Cargo.toml: remove kicad-parser / kicad-writer from members + workspace.dependencies.
# Per-crate Cargo.toml: remove kicad-parser / kicad-writer dependencies.
```

### 5.3 Delete the crates

```bash
git rm -r crates/kicad-parser crates/kicad-writer
git rm crates/signex-output/src/netlist/kicad_sexpr.rs
```

### 5.4 Fix what doesn't compile

Various app code calls `kicad_parser::parse_schematic_file` directly. Replace with `signex_engine::persistence::load_schematic` which now reads `.snxsch`. The `.kicad_sch` path is gone from the main repo.

### 5.5 Tests

```bash
cargo test --workspace
# All test fixtures are .snxsch by now (Phase 3.2 migration).
# License guard CI passes (Phase 2.4 lint catches any leftover kicad references).
```

### 5.6 Commit

```
refactor: remove KiCad I/O from main repo (Apache-clean cutover)

Stage 5 of issue-62 remediation. The main signex repo no longer
contains KiCad-derived code:

- crates/kicad-parser/ deleted (3,938 LOC) — moved to GPL-3.0
  signex-kicad-import companion repo.
- crates/kicad-writer/ deleted (2,274 LOC) — same.
- crates/signex-output/src/netlist/kicad_sexpr.rs deleted (336 LOC) —
  moved to signex-kicad-export (future companion).
- 39 use kicad_parser:: / use kicad_writer:: imports replaced with
  Signex-native paths.
- Workspace Cargo.toml no longer references the KiCad crates.

The signex-kicad-import companion at github.com/alplabai/signex-kicad-import
provides one-way KiCad→Signex migration; the main repo's first-run
wizard (Stage 3.3) prompts the user to install it on first encounter
of a .kicad_sch file.

License guard CI (Stage 2.4) passes — `git grep -i kicad` returns zero
hits across crates/.
```

---

## Phase 6 — Release-notes remediation (½ day)

For v0.7.0 and v0.8.0 GitHub Releases, edit the release body in place:

```markdown
> ⚠️ **Licensing notice (added 2026-XX-XX)**
>
> This release contained KiCad-derived code (`kicad-parser`,
> `kicad-writer`) shipped under Apache-2.0 in error. From v0.9.0
> onwards, KiCad I/O is moved to a separate GPL-3.0-licensed
> companion tool. See [issue #62](https://github.com/alplabai/signex/issues/62)
> for context. Users seeking KiCad migration should install
> [signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
> alongside Signex Community.
>
> This release remains available for historical use; please prefer
> v0.9.0 for new installations.
```

---

## Phase 7 — Public communication (1 day)

### 7.1 Reply to issue #62

```markdown
@sethhillbrand — quick update.

We've completed the Apache-clean remediation. The main signex repo
no longer contains KiCad-derived code; KiCad I/O lives in a separate
GPL-3.0 companion tool at github.com/alplabai/signex-kicad-import.
Specifically:

- `kicad-parser` and `kicad-writer` crates moved to the companion
  repo under GPL-3.0-or-later.
- KiCad-flavoured enums (PinElectricalType, PinShape, LayerId
  numbering, KiCad-format auto-net-naming, KiCad markup parser)
  removed from signex-types and replaced with Signex-curated
  designs.
- v0.7.0 / v0.8.0 release notes flagged with the licensing notice
  pointing here.
- v0.9.0 ships the Apache-clean main binary + the companion tool
  for migration.

Happy to take feedback on whether this resolves the concern. Will
close #62 once you confirm.

Thanks for raising it cleanly — the dual-repo structure ends up
healthier than the original Apache-2.0-everything claim.
```

### 7.2 README update

```markdown
## License

Signex is **Apache-2.0**. The main repository contains no GPL-derived
code. KiCad migration is provided via the optional
[signex-kicad-import](https://github.com/alplabai/signex-kicad-import)
companion tool, which is GPL-3.0-or-later (KiCad's reciprocal terms).
The two are distributed independently — Apache consumers of Signex
Community Edition have no GPL aggregation in their builds.
```

### 7.3 Discord / discussions / website

Update the marketing copy at `signex.dev` and the GitHub Discussions sticky to match.

---

## Phase 8 — Clean-room discipline (ongoing — set up once)

**New file**: `CONTRIBUTING.md` addition

```markdown
## License compliance for contributions

Patches to the main `signex` repo must not contain KiCad-derived
code. The repository is Apache-2.0; KiCad I/O lives in the GPL-3.0
companion repo `signex-kicad-import`.

When opening a PR, declare in the PR description:

```
Source basis: [my own work | Signex's prior code | published format
specs | other (specify)]
LLM-assisted: [yes/no — if yes, list which models]
KiCad source consulted: [yes/no — if yes, the PR belongs in
signex-kicad-import, not here]
```

PRs without this declaration block are auto-blocked by CI.

LLMs that have been trained on KiCad source CAN contribute to the
GPL companion repo. They should NOT contribute to the Apache main
repo. If your LLM has consulted KiCad source, route the work to
signex-kicad-import.
```

---

## Phase 9 — CI guards (final wiring)

Ensures Phase 5's cleanup doesn't regress.

### 9.1 PR description guard

`.github/workflows/pr-license-declaration.yml`:

```yaml
name: PR License Declaration
on: [pull_request]
jobs:
  check-declaration:
    runs-on: ubuntu-latest
    steps:
      - run: |
          BODY="${{ github.event.pull_request.body }}"
          for line in "Source basis:" "LLM-assisted:" "KiCad source consulted:"; do
            if ! echo "$BODY" | grep -q "$line"; then
              echo "::error::PR description missing required field: $line"
              exit 1
            fi
          done
          if echo "$BODY" | grep -qi "KiCad source consulted: yes"; then
            echo "::error::This PR consults KiCad source; it belongs in signex-kicad-import, not the main signex repo."
            exit 1
          fi
```

### 9.2 cargo-deny in CI

`.github/workflows/ci.yml` — add a step:

```yaml
- name: License + dependency audit
  run: |
    cargo install cargo-deny
    cargo deny check licenses
```

---

## Quick-look execution timeline

| Week | Phase work |
|---|---|
| 1 | Phase 0 (½ day) + Phase 1 (1 week) |
| 2-3 | Phase 2.1 (Layer abstraction) + Phase 2.2 (Pin enums) |
| 3-4 | Phase 2.3 (Markup) + Phase 2.4 (CI gate) |
| 4-7 | Phase 3 (native formats) + Phase 4 (companion repo, in parallel) |
| 8 | Phase 5 (cutover) |
| 9 | Phase 6 + Phase 7 (release notes + public communication) |
| ongoing | Phase 8 + Phase 9 |

**Critical-path watch**: Phase 5 cannot start until Phase 4.4 (companion tool released) has produced converted test fixtures for Phase 3.2. Plan Phase 4 to run in parallel with Phase 2 so the timeline doesn't slip.

---

## Decision log

| Date | Decision | By |
|---|---|---|
| 2026-04-29 | Apache-only path chosen over dual-license. Two-repo split (signex Apache-2.0 + signex-kicad-import GPL-3.0). | Caner |
| 2026-04-29 | This execution plan written; ready to start Phase 0 once Caner's reply to Seth is acknowledged. | Caner + Claude |
| 2026-04-29 | Q1 (overbar in markup): add a third sigil `_~text~_` rather than dropping overbar; active-low signal naming is too common in HW to lose. | Caner |
| 2026-04-29 | Q2 (v0.9 stack): cherry-pick still-valid commits onto post-cutover dev, rewriting commits that referenced removed KiCad types. | Caner |
| 2026-04-29 | Q3 (test fixtures): replace KiCad-demo-derived fixtures with Signex-original equivalents that exercise equivalent code paths. | Caner |
| 2026-04-29 | Q4 (third-party parser): conditional on maintenance + lossless coverage. Search outcome: only `kiutils-rs` (MIT, last commit 2026-03-29) clears the bar but has 7 stars and a sole maintainer. Stayed with two-repo GPL companion structure (Q4=B); the structural-derivation residual risk argument trumps maintenance test alone. Documented in `docs/audit/third-party-kicad-parsers.md`. | Claude (lower-risk path per autonomous instructions) |
| 2026-04-29 | Phase 0 outputs committed on `chore/apache-clean-phase-0`. Tags: `audit-baseline-2026-04-29` → dev tip `0e74ebc`; `v0.9-snxlib-paused-2026-04-29` → `bbc68ce`. | Claude |
