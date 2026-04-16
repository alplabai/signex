# KiCad Library Convention (KLC) — Symbol Rules
> Source: https://klc.kicad.org | Versiyon 3.0.64

This file covers all rules for contributing to the official KiCad library OR
generating KiCad-compatible libraries.

---

## G1 — General Rules

- **G1.1** Only these characters in library and symbol names: `A-Z a-z 0-9 _ - . ( )`
  — space, `/`, `#`, `@`, `!` forbidden
- **G1.3** Libraries are organized by function (manufacturer x category matrix)
- **G1.4** All content must be in English
- **G1.5** Avoid plural naming — `Resistors` not `Resistor`
- **G1.6** CamelCase kullan — `MCU_ST_STM32F4`, `Transistor_BJT`
- **G1.7** Unix line endings (`\n`, CRLF not)
- **G1.9** Units: mil (imperial) / mm — 100 mil grid for pin positions

---

## S1 — Library Namelendirme

```
[MANUFACTURER_]CATEGORY[_SUB_CATEGORY]
```

Examples:
```
Device                    # generic devices (no manufacturer)
Transistor_BJT            # BJT transistors
MCU_ST_STM32F4            # ST manufacturer, STM32F4 family
Amplifier_Operational     # functional category
Interface_CAN_LIN         # protocol-based
```

---

## S2 — Symbol Namelendirme

```
[MANUFACTURER_]PART_NUMBER[_VARIANT]
```

Rules:
- Symbol name should not repeat words from the library name
  (`Transistor_BJT:BC547` not `Transistor_BJT:Transistor_BC547`)
- Part number variants can be combined with wildcards: `LM358x` (`LM358A`, `LM358B`…)
- Different footprint options -> separate symbols: `ATmega328P-PU` ve `ATmega328P-AU`
- Active-low pins with overbar: `~{RESET}`, `~{CS}`, `~{OE}`

---

## S3 — General Symbol Requirements

### S3.1 Origin
- Symmetric symbols: origin `(0, 0)` must be centered
- Asymmetric symbols: can be offset to fit 100 mil grid

### S3.2 Text Sizes
- All text fields: **50 mil (1.27 mm)**
- Pin name/number: minimum **20 mil** (in very crowded symbols)

### S3.3 Outline and Fill
```
Line width: 10 mil (0.254 mm)
```
- **Black-box IC** (hidden internal structure): `fill (type background)` — fill with background color
- **Discrete component** (R, C, L, diyot…): `fill (type none)` — no fill

### S3.5 Pin Connection Points
Pin endpoints (connection points) must be **outside** the symbol body —
at least 0 mm from body edge (must not overlap)

### S3.6 Pin Name Offset
Default offset: `1.016 mm` (40 mil)
```scheme
(pin_names (offset 1.016))
```

### S3.8 Multi-unit Symbols
- Power pins (`VCC`, `GND`) -> common to all units `unit 0` symbol
- Each unit should be associated with the same footprint
- Symmetric unit counts are preferred (2, 4, equal distribution)

### S3.9 De Morgan (Alternative Body)
- Resmi library: **De Morgan is **not used** (`S3.9` rule)
- Can optionally be used for personal libraries

---

## S4 — Pin Requirements

### S4.1 General Pin Rules

| Rule | Value |
|-------|-------|
| Grid (pin origin) | **100 mil (2.54 mm)** — IEC-60617 |
| Minimum pin length | **100 mil (2.54 mm)** |
| Step increment | 50 mil (1.27 mm) |
| Maximum pin length | **300 mil (7.62 mm)** |
| Pin number 2 chars -> | 100 mil |
| Pin number 3 chars -> | 150 mil |
| Pin number 4 chars -> | 200 mil |
| Discrete component | short pin allowed |
| All pins | **must be **same length** |

```scheme
; 100 mil pin example (2.54 mm)
(pin input line (at -5.08 2.54 0) (length 2.54)
  (name "IN+" (effects (font (size 1.27 1.27))))
  (number "3"  (effects (font (size 1.27 1.27))))
)
```

### S4.2 Pin Grouping
Pins should be grouped **by function** (not by physical order in datasheet):
1. Power (`VCC`, `GND`, `AGND`, `DVDD`…)
2. Inputler (sol taraf)
3. Outputs (right side)
4. Control/configuration
5. I/O
6. Special functions

### S4.3 Pin Stacking
Multiple pins can be placed at the same position (e.g. multiple GND):
- Same `number` is forbidden — each pin must have a unique number
- Same `name` + different `number` -> valid stacking

### S4.4 Pin Electrical Type Selection

| Type | Usage |
|-----|----------|
| `input` | Input pinleri |
| `output` | Output pinleri |
| `bidirectional` | I/O pins |
| `tri_state` | Three-state output |
| `passive` | Passive component pins (R, C, L terminals) |
| `power_in` | Power input (VCC, VDD) |
| `power_out` | Regulator output, power generator |
| `open_collector` | OC output |
| `open_emitter` | OE output |
| `no_connect` | NC pins |
| `free` | Internally unconnected, free |
| `unspecified` | Unspecified (last resort) |

### S4.6 Hidden Pins
- Power pins (`VCC`, `GND`) can be **hidden** for single-unit symbols
- Hidden pin `power_in` must be of type
- Hidden pin net names appear in the netlist

```scheme
(pin power_in line (at 0 0 270) (length 0) hide
  (name "VCC" (effects (font (size 1.27 1.27))))
  (number "8"  (effects (font (size 1.27 1.27))))
)
```

### S4.7 Active-Low Pin Names
Active-low signals are shown with tilde+curly braces:
```
~{RESET}   ~{CS}   ~{OE}   ~{WR}   ~{IRQ}
```
Bunlar KiCad'de are automatically rendered as overbar.

---

## S5 — Footprint Association

- If a default footprint exists -> `"LIB:FOOTPRINT"` format should be filled in
- Footprint filters should cover all suitable footprints:

```scheme
(property "ki_fp_filters" "R_* C_0402* C_0603*")
```

Wildcard rules:
- `TO*220*` -> TO220, TO-220_Reverse, TO-220-5 matches all
- `_HandSoldering` variants add `*` at the end
- Do **not** put pin count in the filter — KiCad does this automatically

---

## S6 — Symbol Metadata

### S6.1 Reference Designator (RefDes) Table

| RefDes | Component Type |
|--------|-------------|
| `A` | Sub-assembly, plug-in module |
| `AE` | Antennana |
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
| `L` | Inductor, coil, ferrite |
| `LS` | Speaker, buzzer |
| `M` | Motor |
| `MK` | Microphone |
| `P` | Plug (movable connector) |
| `Q` | Transistor (BJT, MOSFET, IGBT) |
| `R` | Resistor |
| `RN` | Resistor network |
| `RT` | Thermistor |
| `RV` | Varistor |
| `SW` | Key |
| `T` | Transformer |
| `TC` | Thermocouple |
| `TP` | Test point |
| `U` | Integrated circuit (IC) |
| `Y` | Crystal / oscillator |
| `Z` | Zener diode |

Power and graphic symbols: `#PWR`, `#SYM`

### S6.2 Required Metadata Fields

```scheme
; For all symbols:
(property "Reference"  "U"   (id 0) ...)   ; RefDes
(property "Value"      "..."  (id 1) ...)   ; value (should match symbol name)
(property "Footprint"  "..."  (id 2) ...)   ; can be left empty
(property "Datasheet"  "..."  (id 3) ...)   ; URL or "~"

; Optional but recommended:
(property "ki_description" "Description metni" ...)
(property "ki_keywords"    "keywords separated by spaces" ...)
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
    ; SINGLE AND VISIBLE PIN (KiCad 8+)
    (pin power_in line (at 0 0 270) (length 0)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
  )
)
```

**Rulelar (S7.1):**
- `Reference` -> `#PWR`
- Exactly **1 pin**, type: `power_in`
- Pin name: `~`
- KiCad 8+: pin is **visible** (`hide` without)
- KiCad 7 and earlier: pin is **hidden** (`hide` with)
- `Define as power symbol` marked -> `in_bom no` + `on_board no`
- `Value` field should match symbol name

### S7.2 Grafik Symbols
- `Reference` -> `#SYM`
- `in_bom no`, `on_board no`
- No pins (or `no_connect` type pins)

---

## Coordinate Conversions (mil ↔ mm)

```python
MIL_TO_MM = 0.0254

def mil_to_mm(mils):
    return round(mils * MIL_TO_MM, 4)

def mm_to_mil(mm):
    return round(mm / MIL_TO_MM)

# Commonly used values:
# 50 mil  = 1.27 mm   (text size, pin name offset)
# 100 mil = 2.54 mm   (grid, pin length)
# 150 mil = 3.81 mm   (long pin)
# 200 mil = 5.08 mm   (very long pin)
# 300 mil = 7.62 mm   (max pin length)
```

---

## Full Symbol Generation Template (Python)

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

    # Body rectangle
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

{body}
)
"""
```
