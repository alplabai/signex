# Pin design rationale

Documents why `signex-types::schematic::PinDirection` and
`PinShapeStyle` look the way they do, and how the variant set differs
from KiCad's `ELECTRICAL_PINTYPE` / `GRAPHIC_PINSHAPE`.

The previous Signex versions exposed `PinElectricalType` (12 variants,
identical canonical strings to KiCad's enum) and `PinShape` (9
variants, same set as KiCad's). Those types were strong derivations
of KiCad's `pin_type.h` headers and were removed in the issue #62
Apache-clean remediation.

## `PinDirection` — 14 variants

| Variant | Purpose | Origin |
|---|---|---|
| `Input` | Drives signal in | Generic |
| `Output` | Drives signal out | Generic |
| `Bidirectional` | Drives in/out depending on context | Generic |
| `ThreeStatable` | Can be high-Z | Generic (renamed from `TriState`) |
| `Passive` | Resistor/cap/inductor terminal | Generic |
| `PowerInput` | Power supply input | Generic (renamed from `PowerIn`) |
| `PowerOutput` | Power supply output | Generic (renamed from `PowerOut`) |
| `GroundReference` | Distinguishes ground from generic power | **Signex-original** |
| `OpenDrainLow` | Open-drain active-low output | Generic (renamed from `OpenCollector`) |
| `OpenDrainHigh` | Open-drain active-high output | Generic (renamed from `OpenEmitter`) |
| `Differential` | Differential pair member | **Signex-original** (HSD-friendly) |
| `Clock` | Clock pin (modeled as direction, not shape) | **Signex-original** |
| `DoNotConnect` | Manufacturer-marked NC | Generic (renamed from `NotConnected`) |
| `Unclassified` | Default for new pins | Generic (collapses `Free` + `Unspecified`) |

### Differences from other EDA tools

Same direction concepts apply across all major EDA tools — pin types
existed long before any one of them — but the Signex curation is:

- **14 vs 12 variants.** Three Signex-original additions
  (`GroundReference`, `Differential`, `Clock`) and one
  collapsed variant (`Unclassified` covers two prior concepts).
- **`OpenDrainLow` / `OpenDrainHigh`** uses the modern industry term
  (open drain) and the polarity is part of the variant name. Other
  EDA tools historically split this as `OpenCollector` / `OpenEmitter`,
  which conflates the technology (BJT vs MOSFET) with the polarity
  (active-low vs active-high). Signex picks the polarity-tagged
  semantics.
- **`Clock` is a direction, not a shape.** Other EDA tools express
  clock-ness via the pin shape (`ClockTriangle`, `EdgeClockHigh`,
  `ClockLow`). Signex models it at the directional level — a clock
  is a kind of input/output, not just a graphic decoration. The
  shape decoration follows from `PinShapeStyle`, but the semantic
  identity ("this pin is a clock") is in `PinDirection`.
- **`GroundReference` and `Differential`** are Signex-original
  additions. Ground is technically a kind of power input, but
  treating it distinctly enables ERC checks (e.g. "every chip has at
  least one `GroundReference` pin connected") and BOM/datasheet
  conventions (ground symbols use a different glyph). Differential
  pairs are common in modern HSD design (USB, LVDS, MIPI, PCIe);
  marking them at the pin level lets ERC verify pair integrity.

## `PinShapeStyle` — 7 variants

| Variant | Purpose | Origin |
|---|---|---|
| `Plain` | No decoration | Generic (renamed from `Line` / `NonLogic`) |
| `InvertedBubble` | Active-low marker (small circle) | Generic |
| `ClockTriangle` | Clock indicator | Generic |
| `InvertedClockBubble` | Inverted-clock indicator | Generic |
| `HysteresisInput` | Schmitt trigger input | **Signex-original** |
| `HysteresisOutput` | Schmitt trigger output | **Signex-original** |
| `Schmitt` | Generic Schmitt symbol | **Signex-original** |

### Differences from other EDA tools

- **7 vs 9 variants.** Drops the per-direction "low" shape modifiers
  (`InputLow`, `OutputLow`, `ClockLow`) — those are redundant with
  `PinDirection::OpenDrainLow` / `OpenDrainHigh`. Drops
  `EdgeClockHigh` (falling-edge clock variant) as too niche; modern
  designs typically encode edge sensitivity as a netlist annotation
  rather than on the symbol pin.
- **`HysteresisInput` / `HysteresisOutput` / `Schmitt`** are
  Signex-original additions for symbols that include explicit
  hysteresis marking. Mostly visible on Schmitt-trigger inverters
  and comparators.

## Round-trip with foreign formats

When Signex Community reads a foreign file (e.g. via the
`signex-kicad-import` companion tool), a translation layer maps the
foreign tool's enum to Signex's curated set. Some information loss
is acceptable in that direction — for example, `Free` and
`Unspecified` from KiCad both collapse to `Unclassified` in Signex.

Going the other way (Signex → foreign format), Signex-original
variants (`GroundReference`, `Differential`, `Clock`,
`HysteresisInput/Output`, `Schmitt`) collapse to the closest foreign
equivalent — typically `power_in`, `passive`, or `unspecified`. The
Signex-native `.snxsch` format preserves all 14 variants losslessly;
foreign formats are best-effort.

## Adding new variants

`PinDirection` and `PinShapeStyle` are not `#[non_exhaustive]` —
match arms must cover all variants. Adding a variant is a workspace-
wide change that surfaces every consumer that needs to handle the
new case. This is intentional: pin types affect ERC, rendering,
BOM substitution, and netlist output, and all of those should be
exercised before a new variant ships.

When adding a new variant:

1. Add it here with a one-line rationale.
2. Add the variant to the enum.
3. Build the workspace; the compiler will list every match arm
   needing extension.
4. Add ERC, render, output, and serde fixtures.
5. Confirm the variant survives a round-trip through `.snxsch`.
