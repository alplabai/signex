# KiCad Şematik Dosya Formatı — Tam Referans

> Uzantı: `.kicad_sch` | KiCad 6.0+ geçerli

---

## Dosya Üst Düzey Yapısı

```scheme
(kicad_sch
  (version YYYYMMDD)                    ; örn: 20211123
  (generator ÜRETICI_ADI)              ; ⚠️ 3. parti: "eeschema" KULLANMA

  (uuid UUID)                           ; bu şematik dosyasının benzersiz ID'si

  (paper ...)                           ; kağıt ayarları
  (title_block ...)                     ; başlık bloğu

  (lib_symbols                          ; şematikte kullanılan sembollerin kütüphanesi
    SEMBOL_TANIMLARI...
  )

  JUNCTION_TANIMLARI...
  NO_CONNECT_TANIMLARI...
  BUS_ENTRY_TANIMLARI...
  WIRE_VE_BUS_TANIMLARI...
  IMAGE_TANIMLARI...
  POLYLINE_TANIMLARI...
  TEXT_TANIMLARI...
  LABEL_TANIMLARI...
  GLOBAL_LABEL_TANIMLARI...
  HIERARCHICAL_LABEL_TANIMLARI...
  SEMBOL_YERLESTIRILERI...
  SHEET_TANIMLARI...

  (sheet_instances                      ; root sheet instance (zorunlu)
    (path "/"
      (page "1")
    )
  )
)
```

> ⚠️ `generator` için `eeschema` kullanma — kendi aracının adını yaz.

---

## Instance Path Kavramı

Paylaşılan şematikler birden fazla instance'a sahip olabilir. Hiyerarşik yol,
ilgili sheet'lerin UUID'lerinin `/` ile birleştirilmesiyle oluşur:

```
"/00000000-0000-0000-0000-00004b3a13a4/00000000-0000-0000-0000-00004b617b88"
```

- **İlk UUID** her zaman root sheet UUID'si olmalıdır (root `.kicad_sch` dosyasının UUID'si)

---

## Junction

```scheme
(junction
  (at X Y)
  (diameter MM)    ; 0 = sistem varsayılanı
  (color R G B A)  ; 0 0 0 0 = varsayılan renk
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
  (size GENİŞLİK YÜKSEKLİK)   ; end point, start'tan delta offset
  (stroke (width W) (type TYPE))
  (uuid UUID)
)
```

---

## Wire ve Bus

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

> ⚠️ `(start)(end)` değil — wire/bus **`pts` + `xy`** çifti kullanır.

---

## Graphical Line (Polyline)

```scheme
(polyline
  (pts (xy X1 Y1) (xy X2 Y2) ...)   ; minimum 2 nokta
  (stroke ...)
  (uuid UUID)
)
```

---

## Graphical Text

```scheme
(text "METİN"
  (at X Y [AÇI])
  (effects ...)
  (uuid UUID)
)
```

---

## Etiketler

### Local Label

```scheme
(label "AD"
  (at X Y [AÇI])
  (effects ...)
  (uuid UUID)
)
```

### Global Label

```scheme
(global_label "AD"
  (shape input|output|bidirectional|tri_state|passive)
  [(fields_autoplaced)]
  (at X Y [AÇI])
  (effects ...)
  (uuid UUID)
  ÖZELLIKLER...        ; (property ...) tokenları — inter-sheet ref dahil
)
```

### Hierarchical Label

```scheme
(hierarchical_label "AD"
  (shape input|output|bidirectional|tri_state|passive)
  (at X Y [AÇI])
  (effects ...)
  (uuid UUID)
)
```

**Label/Pin şekilleri:** `input` | `output` | `bidirectional` | `tri_state` | `passive`

---

## Symbol (Şematik Sembol Yerleştirmesi)

`lib_symbols` içindeki bir sembolün şematikteki instance'ı.

```scheme
(symbol "LIB:SEMBOL_ADI"
  (at X Y [AÇI])
  [(mirror x|y)]
  (unit N)
  (in_bom yes|no)
  (on_board yes|no)
  (uuid UUID)

  (property "Reference" "R1" (id 0) (at X Y [AÇI]) (effects ...))
  (property "Value" "10k"    (id 1) (at X Y [AÇI]) (effects ...))
  (property "Footprint" "Resistor_SMD:R_0402" (id 2) (at X Y [AÇI]) (effects ...))
  (property "Datasheet" ""   (id 3) (at X Y [AÇI]) (effects ...))

  ; Pin UUID eşlemesi
  (pin "1" (uuid PIN1_UUID))
  (pin "2" (uuid PIN2_UUID))

  ; Proje bazlı instance verileri
  (instances
    (project "PROJE_ADI"
      (path "/ROOT_UUID"                  ; tek sayfa
        (reference "R1")
        (unit 1)
      )
      (path "/ROOT_UUID/SHEET_UUID"       ; alt sayfadaki instance
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
  (size GENİŞLİK YÜKSEKLİK)
  [(fields_autoplaced)]
  (stroke ...)
  (fill (type none|outline|background))
  (uuid UUID)

  ; Zorunlu özellikler
  (property "Sheet name" "ALT_DEVRE"            (id 0) (at X Y) (effects ...))
  (property "Sheet file" "alt_devre.kicad_sch"  (id 1) (at X Y) (effects ...))

  ; Hierarchical pin'ler
  (pin "SİNYAL_ADI" input|output|bidirectional|tri_state|passive
    (at X Y AÇI)
    (effects ...)
    (uuid PIN_UUID)
  )

  ; Instance verileri
  (instances
    (project "PROJE_ADI"
      (path "/ROOT_UUID"
        (page "2")
      )
    )
  )
)
```

> ⚠️ Sheet `pin` adı, ilişkili `.kicad_sch` dosyasındaki
> `hierarchical_label` adıyla **birebir aynı** olmalıdır.

---

## Root Sheet Instance Section

Her root şematik dosyasının en sonunda bulunur:

```scheme
(sheet_instances
  (path "/"
    (page "1")
  )
)
```

---

## lib_symbols Section

Şematikde kullanılan tüm sembollerin **inline kopyası** burada saklanır.
Kütüphane bağımlılığı olmadan dosya açılabilir.

```scheme
(lib_symbols
  (symbol "LIB_ADI:SEMBOL_ADI"
    (pin_names (offset 1.016))
    (in_bom yes) (on_board yes)
    (property "Reference" "R" (id 0) (at 0 1.27 0) (effects ...))
    (property "Value" "R"     (id 1) (at 0 -1.27 0) (effects ...))
    (symbol "SEMBOL_ADI_1_1"
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
  (generator ÜRETICI_ADI)   ; ⚠️ "kicad_symbol_editor" KULLANMA

  (symbol "SEMBOL_ADI" ...)
  (symbol "SEMBOL_ADI_2" ...)
  ; sıfır veya daha fazla sembol
)
```

---

## Python: Şematik Okuma

### kiutils ile (önerilen)

```python
# pip install kiutils
from kiutils.schematic import Schematic

sch = Schematic.from_file("devre.kicad_sch")

# Semboller
for sym in sch.schematicSymbols:
    props = {p.key: p.value for p in sym.properties}
    print(f"{props.get('Reference')}: {props.get('Value')}")

# Wireler
for wire in sch.wires:
    print(wire.startPoint, wire.endPoint)
```

### Instance yolu çözümlemesi

```python
# root UUID = şematik dosyasının uuid token'ı
# Sembol instance'larını dolaş:
for sym in symbols:
    for project_instance in sym.instances:
        project_name = project_instance.name
        for path_entry in project_instance.paths:
            hier_path = path_entry.path   # "/root_uuid" veya "/root_uuid/sheet_uuid"
            reference = path_entry.reference
            unit = path_entry.unit
```
