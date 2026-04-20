# KiCad Schematic File Format — Full Reference

> Extension: `.kicad_sch` | Valid for KiCad 6.0+

---

## Top-Level File Structure

```scheme
(kicad_sch
  (version YYYYMMDD)                    ; e.g.: 20211123
  (generator "YOUR_TOOL_NAME")          ; ⚠️ Do NOT use "eeschema"
  (generator_version "1.0")             ; Required KiCad 8+

  (uuid UUID)                           ; unique ID of this schematic file

  (paper ...)
  (title_block ...)

  (lib_symbols                          ; inline copies of all symbols used
    SYMBOL_DEFINITIONS...
  )

  JUNCTION_DEFINITIONS...
  NO_CONNECT_DEFINITIONS...
  BUS_ENTRY_DEFINITIONS...
  WIRE_AND_BUS_DEFINITIONS...
  IMAGE_DEFINITIONS...
  POLYLINE_DEFINITIONS...
  TEXT_DEFINITIONS...
  LABEL_DEFINITIONS...
  GLOBAL_LABEL_DEFINITIONS...
  HIERARCHICAL_LABEL_DEFINITIONS...
  SYMBOL_PLACEMENTS...
  SHEET_DEFINITIONS...

  (sheet_instances                      ; root sheet instance (required)
    (path "/"
      (page "1")
    )
  )
)
```

> ⚠️ Do NOT use `"eeschema"` as the `generator` value — use your own tool's name.

---

## Instance Path Concept

Shared schematics can have multiple instances. The hierarchical path is
formed by joining the UUIDs of the relevant sheets with `/`:

```
"/00000000-0000-0000-0000-00004b3a13a4/00000000-0000-0000-0000-00004b617b88"
```

- The **first UUID** must always be the root sheet UUID (the `.kicad_sch` file's own `uuid`)

---

## Junction

```scheme
(junction
  (at X Y)
  (diameter MM)    ; 0 = system default
  (color R G B A)  ; 0 0 0 0 = default color
  (uuid UUID)
)
```

---

## No Connect

```scheme
(no_connect
  (at X Y)
  (uuid UUID)
)
```

---

## Bus Entry

```scheme
(bus_entry
  (at X Y)
  (size WIDTH HEIGHT)   ; end point delta offset from start
  (stroke (width W) (type TYPE))
  (uuid UUID)
)
```

---

## Wire and Bus

```scheme
(wire
  (pts (xy X1 Y1) (xy X2 Y2))
  (stroke (width 0) (type default))
  (uuid UUID)
)

(bus
  (pts (xy X1 Y1) (xy X2 Y2))
  (stroke (width 0) (type default))
  (uuid UUID)
)
```

> ⚠️ Wire/bus does NOT use `(start)(end)` — it uses **`pts` + `xy`** pairs.

---

## Graphical Line (Polyline)

```scheme
(polyline
  (pts (xy X1 Y1) (xy X2 Y2) ...)   ; minimum 2 points
  (stroke ...)
  (uuid UUID)
)
```

---

## Graphical Text

```scheme
(text "TEXT"
  (at X Y [ANGLE])
  (effects ...)
  (uuid UUID)
)
```

---

## Labels

### Local Label

```scheme
(label "NAME"
  (at X Y [ANGLE])
  (effects ...)
  (uuid UUID)
)
```

### Global Label

```scheme
(global_label "NAME"
  (shape input|output|bidirectional|tri_state|passive)
  [(fields_autoplaced)]
  (at X Y [ANGLE])
  (effects ...)
  (uuid UUID)
  PROPERTIES...        ; (property ...) tokens — includes inter-sheet ref
)
```

### Hierarchical Label

```scheme
(hierarchical_label "NAME"
  (shape input|output|bidirectional|tri_state|passive)
  (at X Y [ANGLE])
  (effects ...)
  (uuid UUID)
)
```

**Label/pin shapes:** `input` | `output` | `bidirectional` | `tri_state` | `passive`

---

## Symbol (Schematic Symbol Placement)

An instance of a symbol from `lib_symbols` placed on the schematic.

```scheme
(symbol "LIB:SYMBOL_NAME"
  (at X Y [ANGLE])
  [(mirror x|y)]
  (unit N)
  (in_bom yes|no)
  (on_board yes|no)
  (uuid UUID)

  (property "Reference" "R1" (id 0) (at X Y [ANGLE]) (effects ...))
  (property "Value" "10k"    (id 1) (at X Y [ANGLE]) (effects ...))
  (property "Footprint" "Resistor_SMD:R_0402" (id 2) (at X Y [ANGLE]) (effects ...))
  (property "Datasheet" ""   (id 3) (at X Y [ANGLE]) (effects ...))

  ; Pin UUID mapping
  (pin "1" (uuid PIN1_UUID))
  (pin "2" (uuid PIN2_UUID))

  ; Per-project instance data
  (instances
    (project "PROJECT_NAME"
      (path "/ROOT_UUID"                  ; single-sheet design
        (reference "R1")
        (unit 1)
      )
      (path "/ROOT_UUID/SHEET_UUID"       ; instance on a sub-sheet
        (reference "R2")
        (unit 1)
      )
    )
  )
)
```

---

## Hierarchical Sheet

```scheme
(sheet
  (at X Y)
  (size WIDTH HEIGHT)
  [(fields_autoplaced)]
  (stroke ...)
  (fill (type none|outline|background))
  (uuid UUID)

  ; Required properties
  (property "Sheet name" "SUB_CIRCUIT"          (id 0) (at X Y) (effects ...))
  (property "Sheet file" "sub_circuit.kicad_sch" (id 1) (at X Y) (effects ...))

  ; Hierarchical pins
  (pin "SIGNAL_NAME" input|output|bidirectional|tri_state|passive
    (at X Y ANGLE)
    (effects ...)
    (uuid PIN_UUID)
  )

  ; Instance data
  (instances
    (project "PROJECT_NAME"
      (path "/ROOT_UUID"
        (page "2")
      )
    )
  )
)
```

> ⚠️ The sheet `pin` name must match the `hierarchical_label` name in the
> referenced `.kicad_sch` file **character for character**.

---

## Root Sheet Instances Section

Found at the end of every root schematic file:

```scheme
(sheet_instances
  (path "/"
    (page "1")
  )
)
```

---

## lib_symbols Section

An **inline copy** of every symbol used in the schematic is stored here.
The file can be opened without the original library.

```scheme
(lib_symbols
  (symbol "LIB_NAME:SYMBOL_NAME"
    (pin_names (offset 1.016))
    (in_bom yes) (on_board yes)
    (property "Reference" "R" (id 0) (at 0 1.27 0) (effects ...))
    (property "Value" "R"     (id 1) (at 0 -1.27 0) (effects ...))
    (symbol "SYMBOL_NAME_1_1"
      (polyline
        (pts (xy -1.778 -0.889)(xy -1.778 0.889))
        (stroke (width 0.254)(type default))
        (fill (type none))
      )
      (pin passive line (at -3.81 0 0) (length 1.524)
        (name "~" (effects (font (size 1.27 1.27))))
        (number "1" (effects (font (size 1.27 1.27))))
      )
      (pin passive line (at 3.81 0 180) (length 1.524)
        (name "~" (effects (font (size 1.27 1.27))))
        (number "2" (effects (font (size 1.27 1.27))))
      )
    )
  )
)
```

---

## Symbol Library File (.kicad_sym)

```scheme
(kicad_symbol_lib
  (version YYYYMMDD)
  (generator "YOUR_TOOL_NAME")   ; ⚠️ Do NOT use "kicad_symbol_editor"

  (symbol "SYMBOL_NAME" ...)
  (symbol "SYMBOL_NAME_2" ...)
  ; zero or more symbols
)
```

---

## Python: Reading Schematics

### Using kiutils (recommended)

```python
# pip install kiutils
from kiutils.schematic import Schematic

sch = Schematic.from_file("board.kicad_sch")

# Symbols
for sym in sch.schematicSymbols:
    props = {p.key: p.value for p in sym.properties}
    print(f"{props.get('Reference')}: {props.get('Value')}")

# Wires
for wire in sch.wires:
    print(wire.startPoint, wire.endPoint)
```

### Resolving instance paths

```python
# root UUID = the schematic file's own uuid token
for sym in symbols:
    for project_instance in sym.instances:
        project_name = project_instance.name
        for path_entry in project_instance.paths:
            hier_path = path_entry.path   # "/root_uuid" or "/root_uuid/sheet_uuid"
            reference = path_entry.reference
            unit = path_entry.unit
```
