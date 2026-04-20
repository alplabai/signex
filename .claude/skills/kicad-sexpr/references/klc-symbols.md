# KiCad Library Convention (KLC) — Symbol Rules
> Source: https://klc.kicad.org | Version 3.0.64

This file covers all rules for contributing to the official KiCad library OR
for generating KiCad-compatible libraries.

---

## G1 — General Rules

- **G1.1** Library and symbol names: only `A-Z a-z 0-9 _ - . ( )` — no spaces, `/`, `#`, `@`, `!`
- **G1.3** Libraries are organized by function (manufacturer × category matrix)
- **G1.4** All content must be in English
- **G1.5** Avoid plural naming — use `Resistor`, not `Resistors`
- **G1.6** Use CamelCase — `MCU_ST_STM32F4`, `Transistor_BJT`
- **G1.7** Unix line endings (`\n`, not CRLF)
- **G1.9** Units: mil (imperial) / mm — 100 mil grid for pin positions

---

## S1 — Library Naming

```
[MANUFACTURER_]CATEGORY[_SUB_CATEGORY]
```

Examples:
```
Device                    # general components (no manufacturer)
Transistor_BJT            # BJT transistors
MCU_ST_STM32F4            # ST manufacturer, STM32F4 family
Amplifier_Operational     # functional category
Interface_CAN_LIN         # protocol-based
```

---

## S2 — Symbol Naming

```
[MANUFACTURER_]PART_NUMBER[_VARIANT]
```

Rules:
- Symbol name must not repeat words from the library name
  (`Transistor_BJT:BC547`, not `Transistor_BJT:Transistor_BC547`)
- Part number variants can use wildcards: `LM358x` (covers `LM358A`, `LM358B`…)
- Different footprint options → separate symbols: `ATmega328P-PU` and `ATmega328P-AU`
- Active-low pins use overbar notation: `~{RESET}`, `~{CS}`, `~{OE}`

---

## S3 — General Symbol Requirements

### S3.1 Origin
- Symmetric symbols: origin at `(0, 0)` exactly centered
- Asymmetric symbols: may be offset to align to 100 mil grid

### S3.2 Text Sizes
- All text fields: **50 mil (1.27 mm)**
- Pin name/number: minimum **20 mil** (for very dense symbols)

### S3.3 Outline and Fill
```
Line width: 10 mil (0.254 mm)
```
- **Black-box IC** (hidden internals): `fill (type background)` — fill with background color
- **Discrete component** (R, C, L, diode…): `fill (type none)` — no fill

### S3.5 Pin Connection Points
Pin endpoints (connection point) must be **outside** the symbol body —
at least 0 mm from the body edge (must not overlap)

### S3.6 Pin Name Offset
Default offset: `1.016 mm` (40 mil)
```scheme
(pin_names (offset 1.016))
```

### S3.8 Multi-Unit Symbols
- Power pins (`VCC`, `GND`) → common to all units, in `unit 0` symbol
- All units must associate with the same footprint
- Symmetric unit count is preferred (2, 4, equal distribution)

### S3.9 De Morgan (Alternate Body)
- Official library: **De Morgan is not used** (S3.9 rule)
- Optional for personal libraries

---

## S4 — Pin Requirements

### S4.1 General Pin Rules

| Rule | Value |
|------|-------|
| Grid (pin origin) | **100 mil (2.54 mm)** — IEC-60617 |
| Minimum pin length | **100 mil (2.54 mm)** |
| Step increment | 50 mil (1.27 mm) |
| Maximum pin length | **300 mil (7.62 mm)** |
| Pin no. 2 chars → | 100 mil |
| Pin no. 3 chars → | 150 mil |
| Pin no. 4 chars → | 200 mil |
| All pins on a symbol | **must be the same length** |

```scheme
; 100 mil pin example (2.54 mm)
(pin input line (at -5.08 2.54 0) (length 2.54)
  (name "IN+" (effects (font (size 1.27 1.27))))
  (number "3"  (effects (font (size 1.27 1.27))))
)
```

### S4.2 Pin Grouping
Pins should be grouped **by function** (not physical order in datasheet):
1. Power (`VCC`, `GND`, `AGND`, `DVDD`…)
2. Inputs (left side)
3. Outputs (right side)
4. Control/configuration
5. I/O
6. Special functions

### S4.3 Pin Stacking
Multiple pins can share the same position (e.g., multiple GND pins):
- Same `number` is not allowed — each pin must have a unique number
- Same `name` + different `number` → valid stacking

### S4.4 Pin Electrical Type Selection

| Type | Usage |
|------|-------|
| `input` | Input pins |
| `output` | Output pins |
| `bidirectional` | I/O pins |
| `tri_state` | Three-state output |
| `passive` | Passive component pins (R, C, L terminals) |
| `power_in` | Power input (VCC, VDD) |
| `power_out` | Regulator output, power generator |
| `open_collector` | Open-collector output |
| `open_emitter` | Open-emitter output |
| `no_connect` | NC pins |
| `free` | Internally unconnected, free |
| `unspecified` | Unknown (last resort) |

### S4.6 Hidden Pins
- Power pins (`VCC`, `GND`) may be **hidden** for single-unit symbols
- Hidden pin must be `power_in` type
- Hidden pin net names appear in the netlist

```scheme
(pin power_in line (at 0 0 270) (length 0) hide
  (name "VCC" (effects (font (size 1.27 1.27))))
  (number "8"  (effects (font (size 1.27 1.27))))
)
```

### S4.7 Active-Low Pin Names
Active-low signals use tilde+curly brace notation:
```
~{RESET}   ~{CS}   ~{OE}   ~{WR}   ~{IRQ}
```
KiCad renders these with an automatic overbar.

---

## S5 — Footprint Association

- If a default footprint exists → fill in `"LIB:FOOTPRINT"` format
- Footprint filters must cover all suitable footprints:

```scheme
(property "ki_fp_filters" "R_* C_0402* C_0603*")
```

Wildcard rules:
- `TO*220*` → catches TO220, TO-220_Reverse, TO-220-5
- Add `*` at the end for `_HandSoldering` variants
- Do **not** add pin count to the filter — KiCad handles that

---

## S6 — Symbol Metadata

### S6.1 Reference Designator (RefDes) Table

| RefDes | Component Type |
|--------|---------------|
| `A` | Sub-assembly, plug-in module |
| `AE` | Antenna |
| `BT` | Battery |
| `C` | Capacitor |
| `D` | Diode |
| `DS` | Display |
| `F` | Fuse |
| `FB` | Ferrite bead |
| `FD` | Fiducial |
| `FL` | Filter |
| `H` | Mechanical (screw, spacer) |
| `J` | Jack (fixed connector) |
| `JP` | Jumper / link |
| `K` | Relay |
| `L` | Coil, inductor, ferrite |
| `LS` | Speaker, buzzer |
| `M` | Motor |
| `MK` | Microphone |
| `P` | Plug (free connector) |
| `Q` | Transistor (BJT, MOSFET, IGBT) |
| `R` | Resistor |
| `RN` | Resistor network |
| `RT` | Thermistor |
| `RV` | Varistor |
| `SW` | Switch |
| `T` | Transformer |
| `TC` | Thermocouple |
| `TP` | Test point |
| `U` | Integrated circuit (IC) |
| `Y` | Crystal / oscillator |
| `Z` | Zener diode |

Power and graphical symbols: `#PWR`, `#SYM`

### S6.2 Required Metadata Fields

```scheme
; Required for all symbols:
(property "Reference"  "U"   (id 0) ...)   ; RefDes
(property "Value"      "..."  (id 1) ...)   ; value (should match symbol name)
(property "Footprint"  "..."  (id 2) ...)   ; may be empty
(property "Datasheet"  "..."  (id 3) ...)   ; URL or "~"

; Optional but recommended:
(property "ki_description" "Description text" ...)
(property "ki_keywords"    "space separated keywords" ...)
(property "ki_fp_filters"  "FootprintLib:Pattern*" ...)
```

---

## S7 — Special Symbols

### S7.1 Power Symbols

```scheme
(symbol "GND"
  (pin_numbers hide)
  (pin_names (offset 0) hide)
  (in_bom no)
  (on_board no)

  (property "Reference" "#PWR" (id 0) (at 0 -6.35 0)
    (effects (font (size 1.27 1.27)) hide))
  (property "Value" "GND" (id 1) (at 0 -3.81 0)
    (effects (font (size 1.27 1.27))))

  (symbol "GND_0_1"
    ; Graphic: inverted triangle
    (polyline
      (pts (xy 0 0) (xy 0 -1.27) (xy 1.27 -1.27))
      (stroke (width 0) (type default))
      (fill (type none))
    )
    ; SINGLE VISIBLE PIN (KiCad 8+)
    (pin power_in line (at 0 0 270) (length 0)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
  )
)
```

**Rules (S7.1):**
- `Reference` → `#PWR`
- Exactly **1 pin**, type: `power_in`
- Pin name: `~`
- KiCad 8+: pin is **visible** (no `hide`)
- KiCad 7 and earlier: pin is **hidden** (with `hide`)
- `Define as power symbol` checked → `in_bom no` + `on_board no`
- `Value` field must match symbol name

### S7.2 Graphical Symbols
- `Reference` → `#SYM`
- `in_bom no`, `on_board no`
- No pins (or `no_connect` type pin)

---

## Coordinate Conversions (mil ↔ mm)

```python
MIL_TO_MM = 0.0254

def mil_to_mm(mils):
    return round(mils * MIL_TO_MM, 4)

def mm_to_mil(mm):
    return round(mm / MIL_TO_MM)

# Common values:
# 50 mil  = 1.27 mm   (text size, pin name offset)
# 100 mil = 2.54 mm   (grid, pin length)
# 150 mil = 3.81 mm   (long pin)
# 200 mil = 5.08 mm   (extra long pin)
# 300 mil = 7.62 mm   (max pin length)
```

---

## Full Symbol Generator Template (Python)

```python
import uuid

def make_symbol(name, refdes, description, keywords,
                pins, body_pts, lib_name="MyLib"):
    """
    pins: list of dicts:
      { 'num': '1', 'name': 'IN+', 'type': 'input',
        'x': -5.08, 'y': 2.54, 'angle': 0, 'length': 2.54 }
    body_pts: list of (x, y) tuples for rectangle/polygon
    """
    pin_sexpr = []
    for p in pins:
        pin_sexpr.append(f"""    (pin {p['type']} line
      (at {p['x']} {p['y']} {p.get('angle',0)}) (length {p.get('length',2.54)})
      (name "{p['name']}" (effects (font (size 1.27 1.27))))
      (number "{p['num']}" (effects (font (size 1.27 1.27))))
    )""")

    xs = [x for x, y in body_pts]
    ys = [y for x, y in body_pts]
    x1, y1 = min(xs), max(ys)   # top-left
    x2, y2 = max(xs), min(ys)   # bottom-right

    return f"""(symbol "{name}"
  (in_bom yes) (on_board yes)
  (property "Reference" "{refdes}" (id 0) (at 0 {y1 + 2.54:.2f} 0)
    (effects (font (size 1.27 1.27))))
  (property "Value" "{name}" (id 1) (at 0 {y2 - 2.54:.2f} 0)
    (effects (font (size 1.27 1.27))))
  (property "Footprint" "" (id 2) (at 0 0 0)
    (effects (font (size 1.27 1.27)) hide))
  (property "Datasheet" "~" (id 3) (at 0 0 0)
    (effects (font (size 1.27 1.27)) hide))
  (property "ki_description" "{description}" (id 4) (at 0 0 0)
    (effects (font (size 1.27 1.27)) hide))
  (property "ki_keywords" "{keywords}" (id 5) (at 0 0 0)
    (effects (font (size 1.27 1.27)) hide))
  (symbol "{name}_1_1"
    (rectangle (start {x1} {y1}) (end {x2} {y2})
      (stroke (width 0.254) (type default))
      (fill (type background))
    )
{chr(10).join(pin_sexpr)}
  )
)"""


def make_kicad_sym(lib_name, symbols):
    body = "\n\n".join(symbols)
    return f"""(kicad_symbol_lib
  (version 20231120)
  (generator "my_generator")
  (generator_version "1.0")

{body}
)
"""
```
