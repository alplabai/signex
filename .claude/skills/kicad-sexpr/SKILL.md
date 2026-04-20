---
name: kicad-sexpr
description: >
  Comprehensive reference for reading, writing, parsing, generating, and manipulating
  KiCad S-expression file formats. Use this skill whenever working with .kicad_pcb,
  .kicad_sch, .kicad_sym, .kicad_mod, or .kicad_wks files; writing footprint/symbol/
  schematic generator scripts; performing file manipulation for Action Plugins; or
  understanding, validating, and converting KiCad s-expression tokens. Trigger on:
  "kicad file", "kicad format", "sexpr", "s-expression", "kicad parse",
  "kicad pcb read/write", "generate footprint", "netlist", "schematic format",
  "kicad rust", "kicad python", "kicad macro", "kicad token", "kicad version".
---

# KiCad S-Expression Format — Comprehensive Reference

## Overview

KiCad uses S-expressions for all file formats:

| Extension | Content |
|-----------|---------|
| `.kicad_pcb` | Printed Circuit Board |
| `.kicad_sch` | Schematic |
| `.kicad_sym` | Symbol library |
| `.kicad_mod` | Footprint library |
| `.kicad_wks` | Worksheet (title block / border) |

---

## Syntax Basics

```
(token attribute1 attribute2 (nested_token ...) ...)
```

**Rules:**
- Every token is wrapped in `(` and `)`
- All tokens are **lowercase**
- Token names may only contain `_` as a special character (no spaces)
- Strings use `"double quotes"`, UTF-8 encoded
- Numbers are in **millimeters**; exponential notation (`1e-3`) is **not used**
- PCB/Footprint precision: 6 decimal places (0.000001 mm = 1 nm)
- Schematic/Symbol precision: 4 decimal places (0.0001 mm)
- Optional attributes shown in `[square brackets]` in this document
- Multiple choices separated by `|`: `yes|no`

**Coordinate system:**
- All coordinates are **relative** to the origin of their parent object
- PCB: Y axis is positive downward (screen coordinates)
- Schematic: Y axis is positive upward

---

## Common Token Reference

### `at` — Position Identifier

```scheme
(at X Y [ANGLE])
```

- `X`, `Y`: coordinate in mm
- `ANGLE`: rotation angle in degrees (optional)
- ⚠️ Symbol `text` ANGLEs are stored in **tenths of a degree**; all other objects use **whole degrees**

```scheme
; Example: at 10 mm, 20 mm, rotated 90 degrees
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
  (color R G B A)    ; 0–255 or 0.0–1.0
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
    [(face "FONT_FAMILY")]          ; KiCad 7+; "KiCad Font" or TTF name
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

- If `justify` is omitted: horizontally and vertically centered, no mirror
- `mirror` is only supported in the PCB Editor and Footprints

### `paper` — Page Settings

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
  ; ... up to 9
)
```

### `property` — General-Purpose Key-Value

```scheme
(property "KEY" "VALUE")
```

Keys must be unique. The `property` token inside a symbol definition uses a different structure — see [Symbol Properties](#symbol-properties).

### `uuid` — Universally Unique Identifier

```scheme
(uuid XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX)
```

- Version 4 (random) UUID, generated with mt19937 Mersenne Twister
- Legacy KiCad files (pre-6.0) had timestamps re-encoded as UUIDs

### `image` — Embedded Image

```scheme
(image
  (at X Y)
  [(scale SCALAR)]
  [(layer LAYER_NAME)]    ; PCB/Footprint only
  (uuid UUID)
  (data BASE64_PNG_DATA)
)
```

---

## PCB / Footprint Common Syntax

### Layer Capacity

| Category | Count |
|----------|-------|
| Total | 60 |
| Copper | 32 |
| Paired technical (silk/mask/paste/adhesive) | 8 |
| Pre-defined user layers | 4 |
| Board outline + margin | 2 |
| Optional user layers | 9 |

### Canonical Layer Names

> Full table → `references/layers.md`

Frequently used:

| Name | Description |
|------|-------------|
| `F.Cu` | Front copper |
| `B.Cu` | Back copper |
| `In1.Cu`…`In30.Cu` | Inner copper layers |
| `F.SilkS` / `B.SilkS` | Front/back silk screen |
| `F.Mask` / `B.Mask` | Front/back solder mask |
| `F.Paste` / `B.Paste` | Front/back solder paste |
| `F.Fab` / `B.Fab` | Fabrication layer |
| `F.CrtYd` / `B.CrtYd` | Courtyard |
| `Edge.Cuts` | Board outline |
| `Dwgs.User` | Drawing layer |
| `User.1`…`User.9` | User-defined |

Wildcard usage: `*.Cu` → all copper layers

---

## Footprint Token

> Full footprint format → `references/board.md`

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
  [(zone_connect 0|1|2)]          ; 0=none, 1=thermal, 2=solid
  [(thermal_width MM)]
  [(thermal_gap MM)]
  [(attr TYPE [board_only] [exclude_from_pos_files] [exclude_from_bom] [dnp])]
  [(private_layers LAYER_LIST)]   ; KiCad 8+
  [(net_tie_pad_groups "P1,P2")]  ; KiCad 8+
  GRAPHIC_ITEMS...                ; fp_text, fp_line, fp_rect, fp_circle, fp_arc, fp_poly
  PADS...
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

> Full pad reference → `references/pad.md`

```scheme
(pad "NUMBER"
  thru_hole|smd|connect|np_thru_hole
  circle|rect|oval|trapezoid|roundrect|custom
  (at X Y [ANGLE])
  [(locked)]
  (size WIDTH HEIGHT)
  [(drill [oval] DIAMETER [SLOT_WIDTH] [(offset X Y)])]
  (layers "LAYER_LIST")
  [(property PROPERTY)]
  [(remove_unused_layer)]
  [(keep_end_layers)]
  [(roundrect_rratio 0.0-1.0)]
  [(chamfer_ratio 0.0-1.0)]
  [(chamfer top_left top_right bottom_left bottom_right)]
  [(net NUMBER "NET_NAME")]
  (uuid UUID)
  [(pinfunction "PIN_FUNCTION")]
  [(pintype "PIN_TYPE")]
  [(die_length LENGTH)]
  [(solder_mask_margin MM)]
  [(solder_paste_margin MM)]
  [(clearance MM)]
  [(zone_connect 0|1|2|3)]
  [(thermal_width MM)]
  [(thermal_gap MM)]
)
```

### PCB Graphic Items

```scheme
; Line
(gr_line (start X Y) (end X Y) (layer LAYER) (stroke ...) (uuid UUID))

; Rectangle
(gr_rect (start X Y) (end X Y) (layer LAYER) (stroke ...) [(fill yes|no)] (uuid UUID))

; Circle
(gr_circle (center X Y) (end X Y) (layer LAYER) (stroke ...) [(fill yes|no)] (uuid UUID))

; Arc (mid-point method)
(gr_arc (start X Y) (mid X Y) (end X Y) (layer LAYER) (stroke ...) (uuid UUID))

; Polygon
(gr_poly (pts ...) (layer LAYER) (stroke ...) [(fill yes|no)] (uuid UUID))

; Bezier (KiCad 7+)
(bezier (pts (xy X Y) (xy X Y) (xy X Y) (xy X Y)) (layer LAYER) (stroke ...) (uuid UUID))
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

## Schematic and Symbol Library Common Syntax

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
- `UNIT`: which unit; `0` = common to all units
- `STYLE`: 1 or 2 (only two body styles supported)

### Symbol Properties

```scheme
(property "KEY" "VALUE"
  (id N)                  ; integer, must be unique
  (at X Y [ANGLE])
  (effects ...)
)
```

**Required properties (parent symbols):**

| Key | id | Description | May be empty? |
|-----|----|-------------|---------------|
| `Reference` | 0 | Reference designator | No |
| `Value` | 1 | Value string | No |
| `Footprint` | 2 | Footprint lib ID | Yes |
| `Datasheet` | 3 | Datasheet URL | Yes |

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

; Polyline (symbol line or polygon)
(polyline (pts ...) STROKE_DEF FILL_DEF)

; Rectangle
(rectangle (start X Y) (end X Y) STROKE_DEF FILL_DEF)

; Text
(text "TEXT" (at X Y [ANGLE]) (effects ...))
```

**`fill` token (schematic/symbol):**
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
  (name "NAME" (effects ...))
  (number "NUMBER" (effects ...))
)
```

**Electrical types:** `input`, `output`, `bidirectional`, `tri_state`, `passive`,
`free`, `unspecified`, `power_in`, `power_out`, `open_collector`, `open_emitter`, `no_connect`

**Graphic styles:** `line`, `inverted`, `clock`, `inverted_clock`, `input_low`,
`clock_low`, `output_low`, `edge_clock_high`, `non_logic`

---

## Group Token

```scheme
(group "NAME"
  (uuid UUID)              ; KiCad 8+: uuid not id
  (members UUID1 UUID2 ... UUIDN)
)
```

---

## Library Identifier Format

```
"LIBRARY_NICKNAME:ENTRY_NAME"
```

⚠️ Library files do **not** store the `LIBRARY_NICKNAME` — only the `ENTRY_NAME` is saved.

---

## Python S-Expression Parsing

Native reading via `pcbnew` module in KiCad Action Plugins or scripting console:

```python
import pcbnew

board = pcbnew.LoadBoard("board.kicad_pcb")

for fp in board.GetFootprints():
    print(fp.GetReference(), fp.GetPosition())

fp = pcbnew.FootprintLoad("MyCoolLib", "SOT23")
board.Add(fp)
pcbnew.Refresh()
```

Lightweight Python parser for raw S-expressions:

```python
def parse_sexpr(text):
    """Minimal KiCad sexpr parser. Returns nested list."""
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
```

### Generating S-Expressions (Python)

```python
def to_sexpr(obj, indent=0):
    """Convert Python list to KiCad sexpr format."""
    pad = "  " * indent
    if isinstance(obj, list):
        if not obj:
            return "()"
        inner = " ".join(to_sexpr(x) for x in obj)
        if len(inner) > 80:
            child_pad = "  " * (indent + 1)
            lines = "\n".join(f"{child_pad}{to_sexpr(x, indent+1)}" for x in obj)
            return f"(\n{lines}\n{pad})"
        return f"({inner})"
    elif isinstance(obj, str):
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

1. **Coordinate precision:** `round(val, 6)` for PCB; `round(val, 4)` for schematic
2. **UUID generation:** `uuid.uuid4()` in Python is sufficient — produces compatible v4 UUIDs
3. **Timestamp (tedit):** `format(int(time.time()), 'X')` — hex format
4. **fp_text requirement:** `reference` and `value` are mandatory in every footprint
5. **Layer names:** canonical names are always English — user names are display-only
6. **KiCad 7 change:** `width` token → `stroke` token; `dash_dot_dot` added; TrueType `face` token added
7. **Version compat:** KiCad pre-6 used `module` instead of `footprint`
8. **Wire/Bus syntax:** uses `(pts (xy X1 Y1)(xy X2 Y2))`, **not** `(start X Y)(end X Y)`
9. **Track/Via UUID:** PCB tracks and vias use `tstamp UUID`, not `uuid UUID`
10. **Symbol `instances` block:** hierarchical designs use `instances → project → path → reference/unit` chain; missing or incorrect `instances` breaks netlist output from third-party generators
11. **`generator` warning:** `eeschema` and `kicad_symbol_editor` are reserved for KiCad; use your own identifier in third-party tools
12. **lib_symbols:** Schematic files embed copies of all used symbols in `lib_symbols` — files can be opened without the original library
13. **Hierarchical sheet pin→label matching:** the `pin` name inside a sheet must match the `hierarchical_label` name in the sub-schematic **character for character**

---

## Version Compatibility (KiCad 8 / 9 / 10)

> 📄 `references/version-compat.md` — **Read this first if KiCad version matters**

**Summary:**

| Feature | KiCad 8 | KiCad 9 | KiCad 10 |
|---------|:-------:|:-------:|:--------:|
| File version | `20240108` | `20241229` | `20250324` |
| `generator_version` | ✅ Required | ✅ | ✅ |
| `private_layers` / `net_tie_pad_groups` | ✅ | ✅ | ✅ |
| `uuid` (not `id`) for groups/generators | ✅ | ✅ | ✅ |
| Arbitrary user layer count | ❌ | ✅ | ✅ |
| Shape hatching fill | ❌ | ❌ | ✅ `20250222` |
| IPC-4761 via tenting/plugging | ❌ | ❌ | ✅ `20250228` |
| Jumper pad token | ❌ | ❌ | ✅ `20250324` |
| `barcode` / `point` token | ❌ | ❌ | ✅ |
| Footprint inner-layer objects | ❌ | ❌ | ✅ |
| Textbox `knockout` | ❌ | ❌ | ✅ `20250210` |

**Target:** KiCad 10 → `version 20250324`

---

## Generating S-Expressions in Rust

> 📄 `references/rust-macro.md` — Read this before writing any Rust generator:
> `Node/Atom` AST enum, `macro_rules! sexpr!` DSL, `Display` serialize,
> pretty-printer, and ready-to-use helpers:
> `at()`, `layer()`, `layers()`, `pts()`, `stroke()`, `fp_line()`, `fp_text()`,
> `smd_pad()`, `wire()`, `junction()`, full `build_r0603()` example

---

## Reference Files

- `references/version-compat.md` — **KiCad 8/9/10 compatibility** — file version numbers, `generator_version`, new tokens (knockout, hatching, IPC-4761, jumper, barcode, point), Rust `KiCadVersion` enum, minimum valid PCB file
- `references/rust-macro.md` — **Rust macro & AST** — `Node/Atom` enum, `sexpr!` macro, helper API, full footprint example
- `references/layers.md` — All canonical layer names, wildcard usage, Python pcbnew constants
- `references/pad.md` — Full pad token reference, drill, custom pad, zone connection types
- `references/schematic.md` — Schematic format (wire, bus, junction, label, symbol instance, hierarchical sheet, instances block)
- `references/board.md` — PCB board format (segment, via, arc, net, stackup, real example)
- `references/klc-symbols.md` — **KiCad Library Convention (KLC)** — symbol creation rules, pin grid, fill, RefDes table, power symbol structure, Python generator template
- `references/symbol-libraries.md` — **Official library catalog** — 130+ library names/descriptions, Device library contents, quick "which library?" lookup table
- `references/symbol-examples.md` — **Annotated real symbol examples** — R, op-amp, GND power, MCU, crystal, active-low pin, pin stacking, extends pattern
