# KiCad Library Convention (KLC) — Sembol Kuralları
> Kaynak: https://klc.kicad.org | Versiyon 3.0.64

Bu dosya, resmi KiCad kütüphanesine katkıda bulunmak VEYA
KiCad-uyumlu kütüphaneler üretmek için geçerli tüm kuralları kapsar.

---

## G1 — Genel Kurallar

- **G1.1** Kütüphane ve sembol adlarında yalnızca şu karakterler: `A-Z a-z 0-9 _ - . ( )`
  — boşluk, `/`, `#`, `@`, `!` yasak
- **G1.3** Kütüphaneler işlevselliğe göre organize edilir (üretici × kategori matrisi)
- **G1.4** Tüm içerik İngilizce olmalı
- **G1.5** Çoğul isimlendirmeden kaçın — `Resistors` değil `Resistor`
- **G1.6** CamelCase kullan — `MCU_ST_STM32F4`, `Transistor_BJT`
- **G1.7** Unix satır sonları (`\n`, CRLF değil)
- **G1.9** Birimler: mil (imperial) / mm — pin konumları için 100 mil grid

---

## S1 — Kütüphane İsimlendirme

```
[ÜRETİCİ_]KATEGORİ[_ALT_KATEGORİ]
```

Örnekler:
```
Device                    # genel aygıtlar (üretici yok)
Transistor_BJT            # BJT transistörler
MCU_ST_STM32F4            # ST üretici, STM32F4 ailesi
Amplifier_Operational     # işlevsel kategori
Interface_CAN_LIN         # protokol bazlı
```

---

## S2 — Sembol İsimlendirme

```
[ÜRETİCİ_]PARÇA_NUMARASI[_VARYANT]
```

Kurallar:
- Sembol adı kütüphane adındaki kelimeleri tekrar etmemeli
  (`Transistor_BJT:BC547` değil `Transistor_BJT:Transistor_BC547`)
- Parça no. varyantları wildcard ile birleştirilebilir: `LM358x` (`LM358A`, `LM358B`…)
- Farklı footprint seçenekleri → ayrı sembol: `ATmega328P-PU` ve `ATmega328P-AU`
- Aktif düşük pinler üzeri çizgi ile: `~{RESET}`, `~{CS}`, `~{OE}`

---

## S3 — Genel Sembol Gereksinimleri

### S3.1 Orijin
- Simetrik semboller: origin `(0, 0)` tam ortada olmalı
- Asimetrik semboller: 100 mil grid'e uymak için kaydırılabilir

### S3.2 Yazı Boyutları
- Tüm text field'ları: **50 mil (1.27 mm)**
- Pin adı/numarası: en küçük **20 mil** (çok kalabalık sembollerde)

### S3.3 Outline ve Fill
```
Çizgi kalınlığı: 10 mil (0.254 mm)
```
- **Black-box IC** (gizli iç yapı): `fill (type background)` — arka plan rengiyle doldur
- **Ayrık bileşen** (R, C, L, diyot…): `fill (type none)` — doldurma

### S3.5 Pin Bağlantı Noktaları
Pin uçları (bağlantı noktası) sembol gövdesinin **dışında** olmalı —
gövde kenarından en az 0 mm uzakta (üst üste binmemeli)

### S3.6 Pin İsim Offset
Varsayılan offset: `1.016 mm` (40 mil)
```scheme
(pin_names (offset 1.016))
```

### S3.8 Çok Birimli (Multi-unit) Semboller
- Güç pinleri (`VCC`, `GND`) → tüm birimlerde ortak `unit 0` sembolüne
- Her birim aynı footprint ile ilişkilendirilmeli
- Birim sayısı bakımından simetrik tercih edilir (2, 4, eşit dağılım)

### S3.9 De Morgan (Alternatif Gövde)
- Resmi kütüphane: **De Morgan kullanılmaz** (`S3.9` kuralı)
- Kişisel kütüphaneler için opsiyonel olarak kullanılabilir

---

## S4 — Pin Gereksinimleri

### S4.1 Genel Pin Kuralları

| Kural | Değer |
|-------|-------|
| Grid (pin origin) | **100 mil (2.54 mm)** — IEC-60617 |
| Minimum pin uzunluğu | **100 mil (2.54 mm)** |
| Artış adımı | 50 mil (1.27 mm) |
| Maksimum pin uzunluğu | **300 mil (7.62 mm)** |
| Pin no. 2 karakter → | 100 mil |
| Pin no. 3 karakter → | 150 mil |
| Pin no. 4 karakter → | 200 mil |
| Ayrık bileşen | kısa pin izinli |
| Tüm pinler | **aynı uzunlukta** olmalı |

```scheme
; 100 mil pin örneği (2.54 mm)
(pin input line (at -5.08 2.54 0) (length 2.54)
  (name "IN+" (effects (font (size 1.27 1.27))))
  (number "3"  (effects (font (size 1.27 1.27))))
)
```

### S4.2 Pin Gruplama
Pinler **işleve göre** gruplanmalı (datasheetteki fiziksel sıraya göre değil):
1. Güç (`VCC`, `GND`, `AGND`, `DVDD`…)
2. Girişler (sol taraf)
3. Çıkışlar (sağ taraf)
4. Kontrol/konfigürasyon
5. I/O
6. Özel işlevler

### S4.3 Pin Yığma (Stacking)
Aynı konuma birden fazla pin konulabilir (örn. birden fazla GND):
- Aynı `number` yasak — her pin benzersiz numara almalı
- Aynı `name` + farklı `number` → geçerli yığma

### S4.4 Pin Elektrik Tipi Seçimi

| Tip | Kullanım |
|-----|----------|
| `input` | Giriş pinleri |
| `output` | Çıkış pinleri |
| `bidirectional` | I/O pinleri |
| `tri_state` | Üç durumlu çıkış |
| `passive` | Pasif bileşen pinleri (R, C, L uçları) |
| `power_in` | Güç girişi (VCC, VDD) |
| `power_out` | Regülatör çıkışı, güç üretici |
| `open_collector` | OC çıkış |
| `open_emitter` | OE çıkış |
| `no_connect` | NC pinler |
| `free` | Dahili bağlantısız, serbest |
| `unspecified` | Belirsiz (son çare) |

### S4.6 Gizli Pinler
- Güç pinleri (`VCC`, `GND`) tek tip semboller için **gizlenebilir** (hidden)
- Gizli pin `power_in` tipinde olmalı
- Gizli pinlerin net adı net listede görünür

```scheme
(pin power_in line (at 0 0 270) (length 0) hide
  (name "VCC" (effects (font (size 1.27 1.27))))
  (number "8"  (effects (font (size 1.27 1.27))))
)
```

### S4.7 Aktif Düşük Pin İsimleri
Aktif düşük sinyaller tilde+süslü parantez ile gösterilir:
```
~{RESET}   ~{CS}   ~{OE}   ~{WR}   ~{IRQ}
```
Bunlar KiCad'de otomatik çizgi olarak render edilir.

---

## S5 — Footprint İlişkilendirme

- Varsayılan footprint varsa → `"LIB:FOOTPRINT"` formatında doldurulmalı
- Footprint filter'lar tüm uygun footprint'leri kapsamalı:

```scheme
(property "ki_fp_filters" "R_* C_0402* C_0603*")
```

Wildcard kuralları:
- `TO*220*` → TO220, TO-220_Reverse, TO-220-5 hepsini yakalar
- `_HandSoldering` varyantları için sonuna `*` ekle
- Pin sayısını filtreye **koyma** — KiCad bunu kendi yapar

---

## S6 — Sembol Metadata

### S6.1 Reference Designator (RefDes) Tablosu

| RefDes | Bileşen Türü |
|--------|-------------|
| `A` | Alt-montaj, plug-in modül |
| `AE` | Anten |
| `BT` | Batarya |
| `C` | Kondansatör |
| `D` | Diyot |
| `DS` | Ekran / Display |
| `F` | Sigorta |
| `FB` | Ferrite bead |
| `FD` | Fiducial |
| `FL` | Filtre |
| `H` | Mekanik (vida, spacer) |
| `J` | Jack (sabit konektör) |
| `JP` | Jumper / link |
| `K` | Röle |
| `L` | Bobin, indüktör, ferrite |
| `LS` | Hoparlör, buzzer |
| `M` | Motor |
| `MK` | Mikrofon |
| `P` | Plug (hareketli konektör) |
| `Q` | Transistör (BJT, MOSFET, IGBT) |
| `R` | Direnç |
| `RN` | Direnç ağı |
| `RT` | Termistör |
| `RV` | Varistör |
| `SW` | Anahtar |
| `T` | Transformatör |
| `TC` | Termoçift |
| `TP` | Test noktası |
| `U` | Entegre devre (IC) |
| `Y` | Kristal / osilatör |
| `Z` | Zener diyot |

Güç ve grafik semboller: `#PWR`, `#SYM`

### S6.2 Zorunlu Metadata Alanları

```scheme
; Tüm semboller için:
(property "Reference"  "U"   (id 0) ...)   ; RefDes
(property "Value"      "..."  (id 1) ...)   ; değer (sembol adıyla eşleşmeli)
(property "Footprint"  "..."  (id 2) ...)   ; boş bırakılabilir
(property "Datasheet"  "..."  (id 3) ...)   ; URL veya "~"

; Opsiyonel ama önerilen:
(property "ki_description" "Açıklama metni" ...)
(property "ki_keywords"    "anahtar kelimeler boşlukla ayrılmış" ...)
(property "ki_fp_filters"  "FootprintLib:Pattern*" ...)
```

---

## S7 — Özel Semboller

### S7.1 Power Semboller

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
    ; Grafik: ters üçgen
    (polyline
      (pts (xy 0 0) (xy 0 -1.27) (xy 1.27 -1.27))
      (stroke (width 0) (type default))
      (fill (type none))
    )
    ; TEK VE GÖRÜNÜR PIN (KiCad 8+)
    (pin power_in line (at 0 0 270) (length 0)
      (name "~" (effects (font (size 1.27 1.27))))
      (number "1" (effects (font (size 1.27 1.27))))
    )
  )
)
```

**Kurallar (S7.1):**
- `Reference` → `#PWR`
- Tam olarak **1 pin**, tip: `power_in`
- Pin adı: `~`
- KiCad 8+: pin **görünür** (`hide` olmadan)
- KiCad 7 ve öncesi: pin **gizli** (`hide` ile)
- `Define as power symbol` işaretli → `in_bom no` + `on_board no`
- `Value` alanı sembol adıyla eşleşmeli

### S7.2 Grafik Semboller
- `Reference` → `#SYM`
- `in_bom no`, `on_board no`
- Pin yok (veya `no_connect` tipinde pin)

---

## Koordinat Dönüşümleri (mil ↔ mm)

```python
MIL_TO_MM = 0.0254

def mil_to_mm(mils):
    return round(mils * MIL_TO_MM, 4)

def mm_to_mil(mm):
    return round(mm / MIL_TO_MM)

# Sık kullanılan değerler:
# 50 mil  = 1.27 mm   (text boyutu, pin name offset)
# 100 mil = 2.54 mm   (grid, pin uzunluğu)
# 150 mil = 3.81 mm   (uzun pin)
# 200 mil = 5.08 mm   (çok uzun pin)
# 300 mil = 7.62 mm   (maks pin uzunluğu)
```

---

## Tam Sembol Üretme Şablonu (Python)

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

    # Gövde dikdörtgeni
    xs = [x for x, y in body_pts]
    ys = [y for x, y in body_pts]
    x1, y1 = min(xs), max(ys)   # sol-üst
    x2, y2 = max(xs), min(ys)   # sağ-alt

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
