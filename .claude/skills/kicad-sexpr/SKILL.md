---
name: kicad-sexpr
description: >
  KiCad S-expression (sexpr) file format for reading, writing, parsing, generating, or
  manipulating - comprehensive reference. KiCad .kicad_pcb, .kicad_sch, .kicad_sym,
  .kicad_mod, .kicad_wks files; when writing footprint/symbol/schematic generator
  scripts; for Action Plugin file manipulation; when you need to
  understand/validate/convert KiCad s-expression tokens, always use this
  skill. "kicad file", "kicad format", "sexpr", "s-expression", "kicad parse",
  "kicad pcb read/write", "generate footprint", "netlist", "schematic format" should trigger this skill.
---

# KiCad S-Expression Format — Comprehensive Reference

## Overview

KiCad, uses S-expression (sexpr) for all file formats:

| Extension | Content |
|--------|--------|
| `.kicad_pcb` | Printed Circuit Board (PCB) |
| `.kicad_sch` | Schematic |
| `.kicad_sym` | Symbol library |
| `.kicad_mod` | Footprint library |
| `.kicad_wks` | Worksheet |

---

## Syntax Fundamentals

```
(token attribute1 attribute2 (nested_token ...) ...)
```

**Rules:**
- Each token is wrapped with `(` and `)`
- All tokens are **lowercase** (`lowercase`)
- Only `_` special character allowed in token names (no spaces)
- Strings use `"double quotes"`, UTF-8 encoded
- Numbers in **millimeters**, exponential notation (`1e-3`) **is **not used**
- PCB/Footprint precision: 6 decimals (0.000001 mm = 1 nm)
- Schematic/Symbol precision: 4 decimals (0.0001 mm)
- Optional attributes shown with `[square brackets]` (in this document)
- Multiple options separated by `|`: `yes|no`

**Coordinate system:**
- All coordinates are **relative** to the parent origin
- PCB Y-axis down positive (screen coordinates)
- Schematic Y-axis up positive

---

## Common Token Reference (Common Syntax)

### `at` — Position Descriptor

```scheme
(at X Y [ANGLE])
```

- `X`, `Y`: coordinate in mm
- `ANGLE`: rotation angle in degrees (optional)
- Warning: Symbol `text` ANGLEs are stored in **1/10 degree**; others in **full degrees**

```scheme
; Example: 10mm, 20mm at point, rotated 90 degrees
(at 10 20 90)
```

### `pts` — Coordinate Point List

```scheme
(pts
  (xy X1 Y1)
  (xy X2 Y2)
  ...
)
```

### `stroke` — Line Style

```scheme
(stroke
  (width WIDTH)
  (type solid|dash|dot|dash_dot|dash_dot_dot|default)
  (color R G B A)    ; 0-255 or 0.0-1.0
)
```

Valid `type` values:
- `solid`, `dash`, `dot`, `dash_dot` — all versions
- `dash_dot_dot` — KiCad 7+
- `default` — theme default

### `effects` — Text Effects

```scheme
(effects
  (font
    [(face "FONT_FAMILY")]          ; KiCad 7+; "KiCad Font" or TTF ismi
    (size HEIGHT WIDTH)             ; in mm
    [(thickness THICKNESS)]
    [bold]
    [italic]
    [(line_spacing LINE_SPACING)]   ; not yet supported
  )
  [(justify [left|right] [top|bottom] [mirror])]
  [hide]
)
```

- `justify` if not defined: horizontal + vertical centered, no mirror
- `mirror` only supported in PCB Editor and Footprint

### `paper` — Paper Settings

```scheme
(paper A4|A3|A2|A1|A0|A|B|C|D|E [portrait])
; OR custom size:
(paper WIDTH HEIGHT [portrait])
```

### `title_block` — Title Block

```scheme
(title_block
  (title "TITLE")
  (date "YYYY-MM-DD")
  (rev "REV")
  (company "COMPANY")
  (comment 1 "COMMENT1")
  (comment 2 "COMMENT2")
  ; ... 9'a up to
)
```

### `property` — General Purpose Property (Key-Value)

```scheme
(property "KEY" "VALUE")
```

Keys must be unique. The `property` token inside a symbol uses a different structure — see [Symbol Properties](#symbol-properties).

### `uuid` — Universally Unique Identifier

```scheme
(uuid XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX)
```

- Version 4 (random) UUID, generated with mt19937 Mersenne Twister
- Pre-KiCad 6 files had timestamp to UUID conversion

### `image` — Embedded Image

```scheme
(image
  (at X Y)
  [(scale SCALAR)]
  [(layer LAYER_NAME)]    ; only PCB/Footprint
  (uuid UUID)
  (data BASE64_PNG_DATA)
)
```

---

## PCB / Footprint Common Syntax

### Layer Capacity

| Category | Count |
|----------|------|
| Total | 60 |
| Copper (copper) | 32 |
| Technical paired (silk/mask/paste/adhesive) | 8 |
| Pre-defined user layers | 4 |
| Board outline + margin | 2 |
| Optional user layers | 9 |

### Canonical Layer Names

> For detailed table see `references/layers.md`

Commonly used:

| Name | Description |
|------|----------|
| `F.Cu` | Front copper |
| `B.Cu` | Back copper |
| `In1.Cu`…`In30.Cu` | Inner copper layers |
| `F.SilkS` / `B.SilkS` | Front/back silkscreen |
| `F.Mask` / `B.Mask` | Front/back solder mask |
| `F.Paste` / `B.Paste` | Front/back solder paste |
| `F.Fab` / `B.Fab` | Fabrication layer |
| `F.CrtYd` / `B.CrtYd` | Courtyard (keep-out area) |
| `Edge.Cuts` | Board edge |
| `Dwgs.User` | Drawing layer |
| `User.1`…`User.9` | User-defined |

Wildcard usage: `*.Cu` -> all copper layers

---

## Footprint Token

> For detailed footprint format see `references/footprint.md`

```scheme
(footprint ["LIB:FOOTPRINT_NAME"]
  [locked] [placed]
  (layer F.Cu|B.Cu)
  (tedit TIMESTAMP)
  [(uuid UUID)]
  [(at X Y [ANGLE])]
  [(descr "DESCRIPTION")]
  [(tags "TAGS")]
  [(property "KEY" "VALUE") ...]
  [(path "SCHEMATIC_PATH")]
  [(solder_mask_margin MM)]
  [(solder_paste_margin MM)]
  [(solder_paste_ratio RATIO)]
  [(clearance MM)]
  [(zone_connect 0|1|2)]          ; 0=not connected, 1=thermal, 2=solid
  [(thermal_width MM)]
  [(thermal_gap MM)]
  [(attr TYPE [board_only] [exclude_from_pos_files] [exclude_from_bom])]
  GRAPHIC_ITEMS...                ; fp_text, fp_line, fp_rect, fp_circle, fp_arc, fp_poly
  PADS...                       ; pad token list
  ZONES...
  GROUPS...
  [(model "3D_FILE" (at (xyz X Y Z)) (scale (xyz X Y Z)) (rotate (xyz X Y Z)))]
)
```

**`attr` TYPE values:** `smd`, `through_hole`

### Footprint Graphic Items

```scheme
; Text
(fp_text reference|value|user "TEXT" (at X Y [ANGLE])
  (layer LAYER) [hide] (effects ...) (uuid UUID))

; Line
(fp_line (start X Y) (end X Y) (layer LAYER)
  (stroke ...) [(locked)] (uuid UUID))

; Rectangle
(fp_rect (start X Y) (end X Y) (layer LAYER)
  (stroke ...) [(fill yes|no)] [(locked)] (uuid UUID))

; Circle (center + end of radius)
(fp_circle (center X Y) (end X Y) (layer LAYER)
  (stroke ...) [(fill yes|no)] [(locked)] (uuid UUID))

; Arc (start + midpoint + end)
(fp_arc (start X Y) (mid X Y) (end X Y) (layer LAYER)
  (stroke ...) [(locked)] (uuid UUID))

; Polygon
(fp_poly (pts (xy X Y) ...) (layer LAYER)
  (stroke ...) [(fill yes|no)] [(locked)] (uuid UUID))

; Bezier curve (4 control points)
(fp_curve (pts (xy X Y) (xy X Y) (xy X Y) (xy X Y))
  (layer LAYER) (stroke ...) [(locked)] (uuid UUID))
```

### Pad Token

> For full pad details see `references/pad.md`

```scheme
(pad "NUMBER"
  thru_hole|smd|connect|np_thru_hole
  circle|rect|oval|trapezoid|roundrect|custom
  (at X Y [ANGLE])
  [(locked)]
  (size WIDTH HEIGHT)
  [(drill [oval] DIAMETER [SLOT_WIDTH] [(offset X Y)])]
  (layers "LAYER_LIST")
  [(net NUMBER "NET_NAME")]
  (uuid UUID)
  [(roundrect_rratio 0.0-1.0)]
  [(chamfer_ratio 0.0-1.0)]
  [(chamfer top_left top_right bottom_left bottom_right)]
  [(solder_mask_margin MM)]
  [(solder_paste_margin MM)]
  [(clearance MM)]
  [(zone_connect 0|1|2)]
)
```

---

## Graphic Items (Board-level)

```scheme
; Text
(gr_text "TEXT" (at X Y) (layer LAYER [(knockout)])
  (uuid UUID) (effects ...))

; Line
(gr_line (start X Y) (end X Y) [(angle A)] (layer LAYER) (width W) (uuid UUID))

; Rectangle
(gr_rect (start X Y) (end X Y) (layer LAYER) (width W) [(fill yes|no)] (uuid UUID))

; Circle
(gr_circle (center X Y) (end X Y) (layer LAYER) (width W) [(fill yes|no)] (uuid UUID))

; Arc (mid-point method)
(gr_arc (start X Y) (mid X Y) (end X Y) (layer LAYER) (width W) (uuid UUID))

; Polygon
(gr_poly (pts ...) (layer LAYER) (width W) [(fill yes|no)] (uuid UUID))

; Bezier (KiCad 7+)
(bezier (pts (xy X Y) (xy X Y) (xy X Y) (xy X Y)) (layer LAYER) (width W) (uuid UUID))
```

---

## Zone Token

```scheme
(zone
  (net NET_NUMBER)
  (net_name "NET_NAME")
  (layer LAYER)
  (uuid UUID)
  [(name "NAME")]
  (hatch none|edge|full PITCH)
  [(priority N)]
  (connect_pads [thru_hole_only|full|no] (clearance MM))
  (min_thickness MM)
  [(keepout (tracks allowed|not_allowed) (vias allowed|not_allowed)
            (pads allowed|not_allowed) (copperpour allowed|not_allowed)
            (footprints allowed|not_allowed))]
  (fill [yes]
    [(mode hatched)]
    (thermal_gap MM) (thermal_bridge_width MM)
    [(smoothing chamfer|fillet)] [(radius R)]
    [(island_removal_mode 0|1|2)] [(island_area_min AREA)]
  )
  (polygon (pts (xy X Y) ...))
  [(filled_polygon (layer LAYER) (pts ...))]
)
```

---

## Schematic / Symbol Library Common Syntax

### Symbol Token Structure

```scheme
(symbol "LIB_ID" | "UNIT_ID"
  [(extends "LIB_ID")]
  [(pin_numbers hide)]
  [(pin_names [(offset MM)] [hide])]
  (in_bom yes|no)
  (on_board yes|no)
  SYMBOL_PROPERTIES...
  GRAPHIC_ITEMS...
  PINS...
  UNITS...
  [(unit_name "UNIT_NAME")]
)
```

**Unit ID format:** `"SYMBOL_NAME_UNIT_STYLE"`
- `UNIT`: which unit, `0` = common to all units
- `STYLE`: 1 or 2 (only two body styles supported)

### Symbol Properties

```scheme
(property "KEY" "VALUE"
  (id N)                  ; integer, must be unique
  (at X Y [ANGLE])
  (effects ...)
)
```

**Required properties (for parent symbols):**

| Key | id | Description | Can be empty? |
|---------|----|----------|-----------------|
| `Reference` | 0 | Reference designator | No |
| `Value` | 1 | Value string | No |
| `Footprint` | 2 | Footprint lib ID | Yes |
| `Datasheet` | 3 | Datasheet link | Yes |

**KiCad reserved keys** (cannot be used as user properties):
`ki_keywords`, `ki_description`, `ki_locked`, `ki_fp_filters`

### Symbol Graphic Items

```scheme
; Arc
(arc (start X Y) (mid X Y) (end X Y) STROKE_DEF FILL_DEF)

; Circle
(circle (center X Y) (radius R) STROKE_DEF FILL_DEF)

; Bezier
(bezier (pts (xy X Y)(xy X Y)(xy X Y)(xy X Y)) STROKE_DEF FILL_DEF)

; Multi-line (polyline — symbol line or polygon)
(polyline (pts ...) STROKE_DEF FILL_DEF)

; Rectangle
(rectangle (start X Y) (end X Y) STROKE_DEF FILL_DEF)

; Text
(text "TEXT" (at X Y [ANGLE]) (effects ...))
```

**`fill` token (for schematic/symbol):**
```scheme
(fill (type none|outline|background))
```

### Pin Token

```scheme
(pin
  ELECTRICAL_TYPE
  GRAPHIC_STYLE
  (at X Y ANGLE)          ; only 0, 90, 180, 270 supported
  (length MM)
  (name "AD" (effects ...))
  (number "NUMBER" (effects ...))
)
```

**Electrical types:**

| Token | Description |
|-------|----------|
| `input` | Input |
| `output` | Output |
| `bidirectional` | Bidirectional |
| `tri_state` | Three-state output |
| `passive` | Passive |
| `free` | Internally unconnected |
| `unspecified` | Unspecified |
| `power_in` | Power input |
| `power_out` | Power output |
| `open_collector` | Open collector |
| `open_emitter` | Open emitter |
| `no_connect` | No connection |

**Graphic styles:** `line`, `inverted`, `clock`, `inverted_clock`, `input_low`,
`clock_low`, `output_low`, `edge_clock_high`, `non_logic`

---

## Group Token

```scheme
(group "NAME"
  (id UUID)
  (members UUID1 UUID2 ... UUIDN)
)
```

---

## Library Identifier Format

```
"LIBRARY_ALIAS:ENTRY_NAME"
```

Warning: Library files do not contain `LIBRARY_ALIAS` - only `ENTRY_NAME` is stored.

---

## Parsing S-Expression with Python

In KiCad Action Plugins or the scripting console, use the `pcbnew` module for native reading:

```python
import pcbnew

# Load PCB
board = pcbnew.LoadBoard("circuit.kicad_pcb")

# Read footprints
for fp in board.GetFootprints():
    print(fp.GetReference(), fp.GetPosition())

# Add footprint
fp = pcbnew.FootprintLoad("MyCoolLib", "SOT23")
board.Add(fp)
pcbnew.Refresh()
```

Lightweight Python parser for raw S-expression parsing:

```python
def parse_sexpr(text):
    """Minimal KiCad sexpr parser. Nested list returns."""
    tokens = []
    current = []
    stack = [current]
    i = 0
    while i < len(text):
        c = text[i]
        if c == '(':
            new = []
            stack[-1].append(new)
            stack.append(new)
        elif c == ')':
            stack.pop()
        elif c == '"':
            j = text.index('"', i+1)
            stack[-1].append(text[i+1:j])
            i = j
        elif c in ' \t\n\r':
            pass
        else:
            j = i
            while j < len(text) and text[j] not in ' \t\n\r()':
                j += 1
            stack[-1].append(text[i:j])
            i = j - 1
        i += 1
    return current[0] if current else []

# Usage:
with open("circuit.kicad_pcb", encoding="utf-8") as f:
    tree = parse_sexpr(f.read())
```

### S-Expression Generation (Python)

```python
def to_sexpr(obj, indent=0):
    """Convert Python list to KiCad sexpr format."""
    pad = "  " * indent
    if isinstance(obj, list):
        if not obj:
            return "()"
        inner = " ".join(to_sexpr(x) for x in obj)
        # Break long lines
        if len(inner) > 80:
            child_pad = "  " * (indent + 1)
            lines = "\n".join(f"{child_pad}{to_sexpr(x, indent+1)}" for x in obj)
            return f"(\n{lines}\n{pad})"
        return f"({inner})"
    elif isinstance(obj, str):
        # Token or string?
        if obj.replace('_', '').replace('.', '').isalnum():
            return obj
        return f'"{obj}"'
    elif isinstance(obj, float):
        return f"{obj:.6g}"
    elif isinstance(obj, int):
        return str(obj)
    return str(obj)
```

---

## Critical Notes and Pitfalls

1. **Coordinate precision:** `round(val, 6)` for PCB, `round(val, 4)` for schematic
2. **UUID generation:** `uuid.uuid4()` is sufficient in Python, produces KiCad-compatible v4 UUID
3. **Timestamp (tedit):** `format(int(time.time()), 'X')` — in hex format
4. **fp_text requirement:** `reference` and `value` required in every footprint; KiCad will complain if missing
5. **Layer names:** canonical names are always in English — user names are display-only
6. **KiCad 7 changes:** `width` token -> `stroke` token; `dash_dot_dot` added; TrueType `face` token added
7. **Version compatibility:** Pre-KiCad 6 used `module` instead of `footprint`
8. **Wire/Bus syntax:** `(start X Y)(end X Y)` NOT — `(pts (xy X1 Y1)(xy X2 Y2))` uses
9. **Track/Via UUID difference:** PCB tracks and vias use `tstamp UUID` not `uuid`
10. **Symbol `instances` block:** Schematic symbol placement token, in hierarchical designs `instances -> project -> path -> reference/unit` chain; if not filled correctly in third-party generators, netlist output corrupts
11. **Schematic `generator` warning:** `eeschema` and `kicad_symbol_editor` are reserved for KiCad only; use your own identifier in third-party tools
12. **lib_symbols:** Schematic file stores a copy of all used symbols in `lib_symbols` - can be opened without library
13. **Hierarchical sheet pin -> label matching:** Sheet `pin` name must be **letter-for-letter identical** to the `hierarchical_label` name in the sub-schematic; otherwise connection fails

---

## Reference Files

For more details, read these files:

- `references/layers.md` — All canonical layer names, wildcard usage, Python pcbnew constants
- `references/pad.md` — Full pad token reference, drill, custom pad, zone connection types
- `references/schematic.md` — Schematic format (wire, bus, junction, label, symbol instance, hierarchical sheet, instances block)
- `references/board.md` — PCB board format (segment, via, arc, net, stackup, real example)
- `references/klc-symbols.md` — **KiCad Library Convention (KLC)** — symbol creation rules, pin grid, fill, RefDes table, power symbol structure, Python generator template
- `references/symbol-libraries.md` — **Official library catalog** — 130+ library names/descriptions, Device library contents, "which library?" quick search table
- `references/symbol-examples.md` — **Annotated real symbol examples** — R, op-amp, GND power, MCU, crystal, active-low pin, pin stacking, extends pattern
