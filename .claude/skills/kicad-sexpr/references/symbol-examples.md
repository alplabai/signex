# Real-World Symbol Examples — Annotated
> KiCad 9 format (.kicad_sym) — follows official library patterns exactly

---

## 1. Passive: Resistor (Device:R)

```scheme
(symbol "R"
  (pin_numbers hide)                          ; pin numbers hidden (simple 2-pin)
  (pin_names
    (offset 0)                                ; pin name offset = 0 (overlapping)
    hide                                      ; pin names hidden
  )
  (in_bom yes)
  (on_board yes)

  (property "Reference" "R" (id 0)
    (at 1.524 0 90)                           ; 90° rotated, on the right
    (effects (font (size 1.27 1.27))))
  (property "Value" "R" (id 1)
    (at -1.524 0 90)
    (effects (font (size 1.27 1.27))))
  (property "Footprint" "" (id 2)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "Datasheet" "~" (id 3)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "ki_keywords" "R res resistor" (id 4)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "ki_description" "Resistor" (id 5)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "ki_fp_filters" "R_*" (id 6)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))

  (symbol "R_0_1")                            ; unit 0, style 1 — empty (no shared graphics)

  (symbol "R_1_1"                             ; unit 1, style 1 — actual drawing
    ; IEC rectangular body
    (rectangle
      (start -1.016 -2.54)
      (end 1.016 2.54)
      (stroke (width 0.254) (type default))
      (fill (type none))                      ; discrete component → NO fill
    )
    ; Pin 1 — top (at Y=3.81, 270° = pointing downward, length 1.27)
    (pin passive line
      (at 0 3.81 270) (length 1.27)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
    ; Pin 2 — bottom (at Y=-3.81, 90° = pointing upward)
    (pin passive line
      (at 0 -3.81 90) (length 1.27)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "2" (effects (font (size 1.27 1.27))))
    )
  )
)
```

**Notes:**
- `~` pin name → invisible name (IEC notation)
- `pin_numbers hide` + `pin_names hide` → standard for 2-pin components
- `fill (type none)` → discrete component rule (S3.3)
- Pin length: 1.27 mm (50 mil) — short pin exception (S4.1)

---

## 2. Active IC: Op-Amp — Single Unit (Amplifier_Operational:LM358)

```scheme
(symbol "LM358"
  (pin_names (offset 1.016))
  (in_bom yes) (on_board yes)

  (property "Reference" "U" (id 0) (at 0 8.89 0)
    (effects (font (size 1.27 1.27))))
  (property "Value" "LM358" (id 1) (at 0 6.35 0)
    (effects (font (size 1.27 1.27))))
  (property "Footprint" "" (id 2)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "Datasheet" "https://www.ti.com/lit/ds/symlink/lm158-n.pdf" (id 3)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "ki_fp_filters" "DIP*W7.62* SOIC*3.9x4.9mm*Pitch1.27mm*" (id 4)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))

  ; UNIT A — first op-amp
  (symbol "LM358_1_1"
    ; Triangle body (standard op-amp shape)
    (polyline
      (pts (xy -3.81 5.08) (xy -3.81 -5.08) (xy 3.81 0) (xy -3.81 5.08))
      (stroke (width 0.254) (type default))
      (fill (type background))               ; IC → background fill
    )
    ; IN+ input
    (pin input line (at -6.35 2.54 0) (length 2.54)
      (name "IN+" (effects (font (size 1.27 1.27))))
      (number "3" (effects (font (size 1.27 1.27))))
    )
    ; IN- input
    (pin input line (at -6.35 -2.54 0) (length 2.54)
      (name "IN-" (effects (font (size 1.27 1.27))))
      (number "2" (effects (font (size 1.27 1.27))))
    )
    ; OUT output
    (pin output line (at 6.35 0 180) (length 2.54)
      (name "OUT" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
    ; VCC — power, hidden pin
    (pin power_in line (at 0 7.62 270) (length 2.54) hide
      (name "V+" (effects (font (size 1.27 1.27))))
      (number "8" (effects (font (size 1.27 1.27))))
    )
    ; GND — power, hidden pin
    (pin power_in line (at 0 -7.62 90) (length 2.54) hide
      (name "V-" (effects (font (size 1.27 1.27))))
      (number "4" (effects (font (size 1.27 1.27))))
    )
  )

  ; UNIT B — second op-amp (same package)
  (symbol "LM358_2_1"
    (polyline
      (pts (xy -3.81 5.08) (xy -3.81 -5.08) (xy 3.81 0) (xy -3.81 5.08))
      (stroke (width 0.254) (type default))
      (fill (type background))
    )
    (pin input line (at -6.35 2.54 0) (length 2.54)
      (name "IN+" (effects (font (size 1.27 1.27))))
      (number "5" (effects (font (size 1.27 1.27))))
    )
    (pin input line (at -6.35 -2.54 0) (length 2.54)
      (name "IN-" (effects (font (size 1.27 1.27))))
      (number "6" (effects (font (size 1.27 1.27))))
    )
    (pin output line (at 6.35 0 180) (length 2.54)
      (name "OUT" (effects (font (size 1.27 1.27))))
      (number "7" (effects (font (size 1.27 1.27))))
    )
    (pin power_in line (at 0 7.62 270) (length 2.54) hide
      (name "V+" (effects (font (size 1.27 1.27))))
      (number "8" (effects (font (size 1.27 1.27))))
    )
    (pin power_in line (at 0 -7.62 90) (length 2.54) hide
      (name "V-" (effects (font (size 1.27 1.27))))
      (number "4" (effects (font (size 1.27 1.27))))
    )
  )
)
```

---

## 3. Power Symbol: GND (power:GND)

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
  (property "Footprint" "" (id 2)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))
  (property "Datasheet" "" (id 3)
    (at 0 0 0) (effects (font (size 1.27 1.27)) hide))

  (symbol "GND_0_1"
    ; Inverted triangle
    (polyline
      (pts (xy 0 0) (xy 1.27 -1.27) (xy -1.27 -1.27) (xy 0 0))
      (stroke (width 0) (type default))
      (fill (type none))
    )
    ; Vertical line
    (polyline
      (pts (xy 0 0) (xy 0 -1.27))
      (stroke (width 0) (type default))
      (fill (type none))
    )
    ; KiCad 8+ → visible pin (no hide)
    (pin power_in line (at 0 0 270) (length 0)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
  )
)
```

---

## 4. Multi-Pin IC: STM32-Style MCU (partial view)

```scheme
(symbol "STM32F103C8Tx"
  (pin_names (offset 1.016))
  (in_bom yes) (on_board yes)

  (property "Reference" "U" (id 0) (at 0 27.94 0) ...)
  (property "Value" "STM32F103C8Tx" (id 1) (at 0 25.4 0) ...)
  (property "Footprint" "Package_QFP:LQFP-48_7x7mm_P0.5mm" (id 2) ... hide)
  (property "Datasheet" "https://www.st.com/resource/en/datasheet/stm32f103c8.pdf" (id 3) ... hide)
  (property "ki_fp_filters" "LQFP*48*" (id 4) ... hide)

  (symbol "STM32F103C8Tx_1_1"
    ; Body rectangle
    (rectangle (start -12.7 -25.4) (end 12.7 25.4)
      (stroke (width 0.254) (type default))
      (fill (type background))
    )
    ; POWER PINS
    (pin power_in line (at -17.78 22.86 0) (length 5.08)
      (name "VDD" (effects (font (size 1.27 1.27))))
      (number "24" (effects (font (size 1.27 1.27))))
    )
    (pin power_in line (at -17.78 20.32 0) (length 5.08)
      (name "VDD" (effects (font (size 1.27 1.27))))
      (number "36" (effects (font (size 1.27 1.27))))
    )
    (pin power_in line (at -17.78 -22.86 0) (length 5.08)
      (name "VSS" (effects (font (size 1.27 1.27))))
      (number "23" (effects (font (size 1.27 1.27))))
    )
    ; RESET — active low
    (pin input line (at 17.78 20.32 180) (length 5.08)
      (name "~{NRST}" (effects (font (size 1.27 1.27))))
      (number "7" (effects (font (size 1.27 1.27))))
    )
    ; GPIO
    (pin bidirectional line (at 17.78 15.24 180) (length 5.08)
      (name "PA0" (effects (font (size 1.27 1.27))))
      (number "10" (effects (font (size 1.27 1.27))))
    )
    ; BOOT
    (pin input line (at -17.78 12.7 0) (length 5.08)
      (name "BOOT0" (effects (font (size 1.27 1.27))))
      (number "44" (effects (font (size 1.27 1.27))))
    )
    ; ... other pins
  )
)
```

---

## 5. Active-Low Pin Names

```scheme
; CORRECT — tilde+curly brace (KiCad 6+):
(name "~{CS}"    ...)    ; → overbar over CS
(name "~{OE}"    ...)    ; → overbar over OE
(name "~{RESET}" ...)    ; → overbar over RESET
(name "~{WR}"    ...)    ; → overbar over WR

; WRONG (old format, no longer supported):
(name "/CS"  ...)
(name "!OE"  ...)
```

---

## 6. Pin Stacking Example

Multiple GND pins on the same component — all placed at the same location:

```scheme
; All GND pins stacked at (at 0 -5.08 90)
(pin power_in line (at 0 -5.08 90) (length 2.54)
  (name "GND" (effects (font (size 1.27 1.27))))
  (number "3" (effects (font (size 1.27 1.27))))
)
(pin power_in line (at 0 -5.08 90) (length 2.54)
  (name "GND" (effects (font (size 1.27 1.27))))
  (number "11" (effects (font (size 1.27 1.27))))
)
(pin power_in line (at 0 -5.08 90) (length 2.54)
  (name "GND" (effects (font (size 1.27 1.27))))
  (number "19" (effects (font (size 1.27 1.27))))
)
```

---

## 7. Extends (Derived) Symbol

Derive from an existing symbol — only properties change, graphics are inherited:

```scheme
(symbol "LM358A"
  (extends "LM358")                           ; derived from LM358
  (property "Reference" "U" (id 0) ...)
  (property "Value" "LM358A" (id 1) ...)     ; only value changed
  (property "Datasheet" "https://..." (id 3) ...)
  ; graphics, pins and all other properties inherited from parent
)
```

---

## 8. Crystal Symbol (Device:Crystal)

```scheme
(symbol "Crystal"
  (pin_numbers hide)
  (pin_names (offset 0.254) hide)
  (in_bom yes) (on_board yes)
  ...
  (symbol "Crystal_1_1"
    (polyline (pts (xy 0 -1.778) (xy 0 1.778))      ; vertical line
      (stroke (width 0.508) (type default)) (fill (type none)))
    (rectangle (start -0.762 -0.889) (end 0.762 0.889)   ; rectangle
      (stroke (width 0.254) (type default)) (fill (type background)))
    (polyline (pts (xy -1.778 -0.889) (xy -1.778 0.889)) ; left plate
      (stroke (width 0.508) (type default)) (fill (type none)))
    (polyline (pts (xy 1.778 -0.889) (xy 1.778 0.889))   ; right plate
      (stroke (width 0.508) (type default)) (fill (type none)))

    (pin passive line (at -3.81 0 0) (length 2.032)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
    (pin passive line (at 3.81 0 180) (length 2.032)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "2" (effects (font (size 1.27 1.27))))
    )
  )
)
```

---

## 9. Quick Coordinate Reference

Pin angles:
```
0°   → pin faces left  (connection point on the left)  → left edge of IC body
90°  → pin faces down  (connection point at the bottom) → bottom edge of IC body
180° → pin faces right (connection point on the right) → right edge of IC body
270° → pin faces up    (connection point at the top)   → top edge of IC body
```

Typical IC pin positions (with 2.54 mm pin length):
```python
# Left edge:   x = -BOX_W - 2.54, angle = 0
# Right edge:  x = +BOX_W + 2.54, angle = 180
# Top edge:    y = +BOX_H + 2.54, angle = 270
# Bottom edge: y = -BOX_H - 2.54, angle = 90
```
