# Gerçek Dünya Sembol Örnekleri — Annotated
> KiCad 9 format (.kicad_sym) — resmi kütüphane kalıplarına birebir uygun

---

## 1. Pasif: Direnç (Device:R)

```scheme
(symbol "R"
  (pin_numbers hide)                          ; pin numaraları gizli (2-pin basit)
  (pin_names
    (offset 0)                                ; pin adı offset = 0 (üst üste)
    hide                                      ; pin adları gizli
  )
  (in_bom yes)
  (on_board yes)

  (property "Reference" "R" (id 0)
    (at 1.524 0 90)                           ; 90° döndürülmüş, sağda
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

  (symbol "R_0_1")                            ; unit 0, stil 1 — boş (ortak grafik yok)

  (symbol "R_1_1"                             ; unit 1, stil 1 — asıl çizim
    ; IEC dikdörtgen gövde
    (rectangle
      (start -1.016 -2.54)
      (end 1.016 2.54)
      (stroke (width 0.254) (type default))
      (fill (type none))                      ; ayrık bileşen → doldurma YOK
    )
    ; Pin 1 — üst (at Y=3.81, 270° yani aşağıya bakan, uzunluk 1.27)
    (pin passive line
      (at 0 3.81 270) (length 1.27)           ; 270° = aşağı yönlü bağlantı
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
    ; Pin 2 — alt (at Y=-3.81, 90° yani yukarıya bakan)
    (pin passive line
      (at 0 -3.81 90) (length 1.27)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "2" (effects (font (size 1.27 1.27))))
    )
  )
)
```

**Dikkat:**
- `~` pin adı → görünmez isim (IEC notasyonu)
- `pin_numbers hide` + `pin_names hide` → 2-pin bileşenlerde standart
- `fill (type none)` → ayrık bileşen kuralı (S3.3)
- Pin uzunluğu: 1.27 mm (50 mil) — kısa pin istisnası (S4.1)

---

## 2. Aktif IC: Op-Amp — Tek Birim (Amplifier_Operational:LM358)

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

  ; UNIT A — ilk op-amp
  (symbol "LM358_1_1"
    ; Üçgen gövde (op-amp standart şekli)
    (polyline
      (pts (xy -3.81 5.08) (xy -3.81 -5.08) (xy 3.81 0) (xy -3.81 5.08))
      (stroke (width 0.254) (type default))
      (fill (type background))               ; IC → arka plan dolgusu
    )
    ; IN+ giriş
    (pin input line (at -6.35 2.54 0) (length 2.54)
      (name "IN+" (effects (font (size 1.27 1.27))))
      (number "3" (effects (font (size 1.27 1.27))))
    )
    ; IN- giriş
    (pin input line (at -6.35 -2.54 0) (length 2.54)
      (name "IN-" (effects (font (size 1.27 1.27))))
      (number "2" (effects (font (size 1.27 1.27))))
    )
    ; OUT çıkış
    (pin output line (at 6.35 0 180) (length 2.54)
      (name "OUT" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
    ; VCC — güç, gizli pin
    (pin power_in line (at 0 7.62 270) (length 2.54) hide
      (name "V+" (effects (font (size 1.27 1.27))))
      (number "8" (effects (font (size 1.27 1.27))))
    )
    ; GND — güç, gizli pin
    (pin power_in line (at 0 -7.62 90) (length 2.54) hide
      (name "V-" (effects (font (size 1.27 1.27))))
      (number "4" (effects (font (size 1.27 1.27))))
    )
  )

  ; UNIT B — ikinci op-amp (aynı package)
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
    ; Güç pinleri unit 0'da değil, A ile aynı hidden pinler burada da var
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

## 3. Power Sembol: GND (power:GND)

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
    ; Üçgen (ters)
    (polyline
      (pts (xy 0 0) (xy 1.27 -1.27) (xy -1.27 -1.27) (xy 0 0))
      (stroke (width 0) (type default))
      (fill (type none))
    )
    ; Dikey çizgi
    (polyline
      (pts (xy 0 0) (xy 0 -1.27))
      (stroke (width 0) (type default))
      (fill (type none))
    )
    ; KiCad 8+ → görünür pin
    (pin power_in line (at 0 0 270) (length 0)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
  )
)
```

---

## 4. Çok Pinli IC: STM32 Tarzı MCU (parçalı gösterim)

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
    ; Gövde dikdörtgeni
    (rectangle (start -12.7 -25.4) (end 12.7 25.4)
      (stroke (width 0.254) (type default))
      (fill (type background))
    )
    ; GÜÇPINLERI
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
    ; RESET
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
    ; ... diğer pinler
  )
)
```

---

## 5. Aktif Düşük Pin İsimleri

```scheme
; DOĞRU — tilde+süslü parantez (KiCad 6+):
(name "~{CS}"    ...)    ; → CS üstünde çizgi
(name "~{OE}"    ...)    ; → OE üstünde çizgi
(name "~{RESET}" ...)    ; → RESET üstünde çizgi
(name "~{WR}"    ...)    ; → WR üstünde çizgi

; YANLIŞ (eski format, artık desteklenmez):
(name "/CS"  ...)
(name "!OE"  ...)
```

---

## 6. Pin Yığma (Stacking) Örneği

Aynı bileşende birden fazla GND pini var — hepsi aynı konuma yerleştirilir:

```scheme
; Tüm GND pinleri aynı (at 0 -5.08 90) konumuna yığılır
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

## 7. Genişletilmiş (Extends) Sembol

Mevcut sembolden türetme — yalnızca property değişir, grafik aynı:

```scheme
(symbol "LM358A"
  (extends "LM358")                           ; LM358'den türetilmiş
  (property "Reference" "U" (id 0) ...)
  (property "Value" "LM358A" (id 1) ...)     ; sadece değer değişti
  (property "Datasheet" "https://..." (id 3) ...)
  ; grafik, pin ve diğer tüm özellikler parent'tan miras alınır
)
```

---

## 8. Kristal Sembolü (Device:Crystal)

```scheme
(symbol "Crystal"
  (pin_numbers hide)
  (pin_names (offset 0.254) hide)
  (in_bom yes) (on_board yes)
  ...
  (symbol "Crystal_1_1"
    ; Çizgiler (kristal sembolü şekli)
    (polyline (pts (xy 0 -1.778) (xy 0 1.778))     ; dikey çizgi
      (stroke (width 0.508) (type default)) (fill (type none)))
    (rectangle (start -0.762 -0.889) (end 0.762 0.889)  ; dikdörtgen
      (stroke (width 0.254) (type default)) (fill (type background)))
    (polyline (pts (xy -1.778 -0.889) (xy -1.778 0.889)) ; sol plaka
      (stroke (width 0.508) (type default)) (fill (type none)))
    (polyline (pts (xy 1.778 -0.889) (xy 1.778 0.889))   ; sağ plaka
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

## 9. Hızlı Koordinat Referansı

Pin yönleri (angle):
```
0°   → pin sola bakan  (bağlantı noktası solda) → IC sol kenarı için
90°  → pin aşağı bakan (bağlantı noktası altta) → IC alt kenarı için
180° → pin sağa bakan  (bağlantı noktası sağda) → IC sağ kenarı için
270° → pin yukarı bakan (bağlantı noktası üstte) → IC üst kenarı için
```

Tipik IC pin konumları (pin uzunluğu 2.54 mm ile):
```python
# Sol kenar: x = -BOX_W - 2.54, angle = 0
# Sağ kenar: x = +BOX_W + 2.54, angle = 180
# Üst kenar: y = +BOX_H + 2.54, angle = 270
# Alt kenar: y = -BOX_H - 2.54, angle = 90
```
