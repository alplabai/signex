---
name: kicad-sexpr
description: >
  KiCad S-expression (sexpr) dosya formatını okumak, yazmak, parse etmek, üretmek veya
  manipüle etmek için kapsamlı referans. KiCad .kicad_pcb, .kicad_sch, .kicad_sym,
  .kicad_mod, .kicad_wks dosyalarıyla çalışırken; footprint/sembol/şematik üretici
  scriptleri yazarken; Action Plugin'ler için dosya manipülasyonu yaparken; KiCad
  s-expression tokenlarını anlamak/doğrulamak/dönüştürmek istediğinde mutlaka bu skili
  kullan. "kicad dosya", "kicad format", "sexpr", "s-expression", "kicad parse",
  "kicad pcb oku/yaz", "footprint üret", "netlist", "schematic format" gibi konularda
  bu skill tetiklenmeli.
---

# KiCad S-Expression Format — Kapsamlı Referans

## Genel Bakış

KiCad, tüm dosya formatları için S-expression (sexpr) kullanır:

| Uzantı | İçerik |
|--------|--------|
| `.kicad_pcb` | Printed Circuit Board (PCB) |
| `.kicad_sch` | Şematik |
| `.kicad_sym` | Sembol kütüphanesi |
| `.kicad_mod` | Footprint kütüphanesi |
| `.kicad_wks` | Çalışma sayfası (worksheet) |

---

## Sözdizimi Temelleri

```
(token attribute1 attribute2 (nested_token ...) ...)
```

**Kurallar:**
- Her token `(` ve `)` ile çevrilir
- Tüm tokenlar **küçük harf** (`lowercase`)
- Token isimlerinde sadece `_` özel karakter kullanılabilir (boşluk yok)
- String'ler `"çift tırnak"` ile, UTF-8 kodlamalı
- Sayılar **milimetre** cinsinden, üstel gösterim (`1e-3`) **kullanılmaz**
- PCB/Footprint hassasiyeti: 6 ondalık (0.000001 mm = 1 nm)
- Şematik/Sembol hassasiyeti: 4 ondalık (0.0001 mm)
- İsteğe bağlı nitelikler `[köşeli parantez]` ile gösterilir (bu dokümanda)
- Birden fazla seçenek `|` ile ayrılır: `yes|no`

**Koordinat sistemi:**
- Tüm koordinatlar **üst nesnenin origin'ine** göre görecelidir (relative)
- PCB Y ekseni aşağı pozitif (screen coordinates)
- Şematik Y ekseni yukarı pozitif

---

## Ortak Token Referansı (Common Syntax)

### `at` — Konum Tanımlayıcı

```scheme
(at X Y [ANGLE])
```

- `X`, `Y`: mm cinsinden koordinat
- `ANGLE`: derece cinsinden dönme açısı (opsiyonel)
- ⚠️ Sembol `text` ANGLE'ları **1/10 derece** cinsinden saklanır; diğerleri **tam derece**

```scheme
; Örnek: 10mm, 20mm noktasında, 90 derece döndürülmüş
(at 10 20 90)
```

### `pts` — Koordinat Noktası Listesi

```scheme
(pts
  (xy X1 Y1)
  (xy X2 Y2)
  ...
)
```

### `stroke` — Çizgi Stili

```scheme
(stroke
  (width WIDTH)
  (type solid|dash|dot|dash_dot|dash_dot_dot|default)
  (color R G B A)    ; 0-255 veya 0.0-1.0
)
```

Geçerli `type` değerleri:
- `solid`, `dash`, `dot`, `dash_dot` — tüm versiyonlar
- `dash_dot_dot` — KiCad 7+
- `default` — tema varsayılanı

### `effects` — Yazı Efektleri

```scheme
(effects
  (font
    [(face "FONT_FAMILY")]          ; KiCad 7+; "KiCad Font" veya TTF ismi
    (size HEIGHT WIDTH)             ; mm cinsinden
    [(thickness THICKNESS)]
    [bold]
    [italic]
    [(line_spacing LINE_SPACING)]   ; henüz desteklenmiyor
  )
  [(justify [left|right] [top|bottom] [mirror])]
  [hide]
)
```

- `justify` tanımlanmazsa: yatay + dikey ortalı, ayna yok
- `mirror` sadece PCB Editor ve Footprint'te desteklenir

### `paper` — Kağıt Ayarları

```scheme
(paper A4|A3|A2|A1|A0|A|B|C|D|E [portrait])
; VEYA özel boyut:
(paper WIDTH HEIGHT [portrait])
```

### `title_block` — Başlık Bloğu

```scheme
(title_block
  (title "BAŞLIK")
  (date "YYYY-MM-DD")
  (rev "REV")
  (company "ŞİRKET")
  (comment 1 "YORUM1")
  (comment 2 "YORUM2")
  ; ... 9'a kadar
)
```

### `property` — Genel Amaçlı Özellik (Key-Value)

```scheme
(property "ANAHTAR" "DEĞER")
```

Anahtarlar unique olmalı. Sembol içindeki `property` tokeni farklı bir yapı kullanır — bkz. [Sembol Özellikleri](#sembol-özellikleri).

### `uuid` — Evrensel Benzersiz Tanımlayıcı

```scheme
(uuid XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX)
```

- Version 4 (random) UUID, mt19937 Mersenne Twister ile üretilir
- KiCad 6 öncesi dosyalarda timestamp → UUID dönüşümü yapılmıştır

### `image` — Gömülü Görsel

```scheme
(image
  (at X Y)
  [(scale SCALAR)]
  [(layer LAYER_NAME)]    ; sadece PCB/Footprint
  (uuid UUID)
  (data BASE64_PNG_DATA)
)
```

---

## PCB / Footprint Ortak Sözdizimi

### Layer Kapasitesi

| Kategori | Adet |
|----------|------|
| Toplam | 60 |
| Bakır (copper) | 32 |
| Teknik çiftli (silk/mask/paste/adhesive) | 8 |
| Kullanıcı tanımlı önceden hazır | 4 |
| Board outline + margin | 2 |
| İsteğe bağlı kullanıcı | 9 |

### Canonical Layer İsimleri

> Detaylı tablo için → `references/layers.md`

Sık kullanılanlar:

| İsim | Açıklama |
|------|----------|
| `F.Cu` | Ön bakır |
| `B.Cu` | Arka bakır |
| `In1.Cu`…`In30.Cu` | İç bakır katmanları |
| `F.SilkS` / `B.SilkS` | Ön/arka serigrafi |
| `F.Mask` / `B.Mask` | Ön/arka lehim maskesi |
| `F.Paste` / `B.Paste` | Ön/arka lehim pastası |
| `F.Fab` / `B.Fab` | Üretim katmanı |
| `F.CrtYd` / `B.CrtYd` | Courtyard (koruma alanı) |
| `Edge.Cuts` | Kart kenarı |
| `Dwgs.User` | Çizim katmanı |
| `User.1`…`User.9` | Kullanıcı tanımlı |

Wildcard kullanımı: `*.Cu` → tüm bakır katmanlar

---

## Footprint Tokeni

> Detaylı footprint formatı → `references/footprint.md`

```scheme
(footprint ["LIB:FOOTPRINT_NAME"]
  [locked] [placed]
  (layer F.Cu|B.Cu)
  (tedit TIMESTAMP)
  [(uuid UUID)]
  [(at X Y [ANGLE])]
  [(descr "AÇIKLAMA")]
  [(tags "ETIKETLER")]
  [(property "KEY" "VALUE") ...]
  [(path "SEMATIK_YOLU")]
  [(solder_mask_margin MM)]
  [(solder_paste_margin MM)]
  [(solder_paste_ratio ORAN)]
  [(clearance MM)]
  [(zone_connect 0|1|2)]          ; 0=bağlı değil, 1=thermal, 2=solid
  [(thermal_width MM)]
  [(thermal_gap MM)]
  [(attr TYPE [board_only] [exclude_from_pos_files] [exclude_from_bom])]
  GRAFIK_OGELER...                ; fp_text, fp_line, fp_rect, fp_circle, fp_arc, fp_poly
  PADLER...                       ; pad token listesi
  ZONLAR...
  GRUPLAR...
  [(model "3D_DOSYA" (at (xyz X Y Z)) (scale (xyz X Y Z)) (rotate (xyz X Y Z)))]
)
```

**`attr` TYPE değerleri:** `smd`, `through_hole`

### Footprint Grafik Ögeleri

```scheme
; Metin
(fp_text reference|value|user "METİN" (at X Y [ANGLE])
  (layer LAYER) [hide] (effects ...) (uuid UUID))

; Çizgi
(fp_line (start X Y) (end X Y) (layer LAYER)
  (stroke ...) [(locked)] (uuid UUID))

; Dikdörtgen
(fp_rect (start X Y) (end X Y) (layer LAYER)
  (stroke ...) [(fill yes|no)] [(locked)] (uuid UUID))

; Daire (center + end of radius)
(fp_circle (center X Y) (end X Y) (layer LAYER)
  (stroke ...) [(fill yes|no)] [(locked)] (uuid UUID))

; Yay (start + midpoint + end)
(fp_arc (start X Y) (mid X Y) (end X Y) (layer LAYER)
  (stroke ...) [(locked)] (uuid UUID))

; Çokgen
(fp_poly (pts (xy X Y) ...) (layer LAYER)
  (stroke ...) [(fill yes|no)] [(locked)] (uuid UUID))

; Bezier eğrisi (4 kontrol noktası)
(fp_curve (pts (xy X Y) (xy X Y) (xy X Y) (xy X Y))
  (layer LAYER) (stroke ...) [(locked)] (uuid UUID))
```

### Pad Tokeni

> Tam pad detayı → `references/pad.md`

```scheme
(pad "NUMARA"
  thru_hole|smd|connect|np_thru_hole
  circle|rect|oval|trapezoid|roundrect|custom
  (at X Y [ANGLE])
  [(locked)]
  (size WIDTH HEIGHT)
  [(drill [oval] DIAMETER [SLOT_WIDTH] [(offset X Y)])]
  (layers "LAYER_LIST")
  [(net NUMARA "NET_ADI")]
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

## Grafik Ögeler (Board-level)

```scheme
; Metin
(gr_text "METİN" (at X Y) (layer LAYER [(knockout)])
  (uuid UUID) (effects ...))

; Çizgi
(gr_line (start X Y) (end X Y) [(angle A)] (layer LAYER) (width W) (uuid UUID))

; Dikdörtgen
(gr_rect (start X Y) (end X Y) (layer LAYER) (width W) [(fill yes|no)] (uuid UUID))

; Daire
(gr_circle (center X Y) (end X Y) (layer LAYER) (width W) [(fill yes|no)] (uuid UUID))

; Yay (mid-point yöntemi)
(gr_arc (start X Y) (mid X Y) (end X Y) (layer LAYER) (width W) (uuid UUID))

; Çokgen
(gr_poly (pts ...) (layer LAYER) (width W) [(fill yes|no)] (uuid UUID))

; Bezier (KiCad 7+)
(bezier (pts (xy X Y) (xy X Y) (xy X Y) (xy X Y)) (layer LAYER) (width W) (uuid UUID))
```

---

## Zone Tokeni

```scheme
(zone
  (net NET_NUMARASI)
  (net_name "NET_ADI")
  (layer LAYER)
  (uuid UUID)
  [(name "ADI")]
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
    [(island_removal_mode 0|1|2)] [(island_area_min ALAN)]
  )
  (polygon (pts (xy X Y) ...))
  [(filled_polygon (layer LAYER) (pts ...))]
)
```

---

## Şematik / Sembol Kütüphanesi Ortak Sözdizimi

### Sembol Token Yapısı

```scheme
(symbol "LIB_KIMLIK" | "BIRIM_KIMLIK"
  [(extends "LIB_KIMLIK")]
  [(pin_numbers hide)]
  [(pin_names [(offset MM)] [hide])]
  (in_bom yes|no)
  (on_board yes|no)
  SEMBOL_OZELLIKLERI...
  GRAFIK_OGELER...
  PINLER...
  BIRIMLER...
  [(unit_name "BIRIM_ADI")]
)
```

**Birim ID formatı:** `"SEMBOL_ADI_BIRIM_STIL"`
- `BIRIM`: hangi birimi, `0` = tüm birimlerde ortak
- `STIL`: 1 veya 2 (sadece iki body style desteklenir)

### Sembol Özellikleri

```scheme
(property "ANAHTAR" "DEĞER"
  (id N)                  ; integer, benzersiz olmalı
  (at X Y [ANGLE])
  (effects ...)
)
```

**Zorunlu özellikler (parent semboller için):**

| Anahtar | id | Açıklama | Boş olabilir mi? |
|---------|----|----------|-----------------|
| `Reference` | 0 | Referans tanımlayıcı | Hayır |
| `Value` | 1 | Değer string'i | Hayır |
| `Footprint` | 2 | Footprint lib ID | Evet |
| `Datasheet` | 3 | Datasheet linki | Evet |

**KiCad rezerve anahtarlar** (kullanıcı property olarak kullanılamaz):
`ki_keywords`, `ki_description`, `ki_locked`, `ki_fp_filters`

### Sembol Grafik Ögeleri

```scheme
; Yay
(arc (start X Y) (mid X Y) (end X Y) STROKE_DEF FILL_DEF)

; Daire
(circle (center X Y) (radius R) STROKE_DEF FILL_DEF)

; Bezier
(bezier (pts (xy X Y)(xy X Y)(xy X Y)(xy X Y)) STROKE_DEF FILL_DEF)

; Çoklu çizgi (polyline — sembol çizgisi veya çokgeni)
(polyline (pts ...) STROKE_DEF FILL_DEF)

; Dikdörtgen
(rectangle (start X Y) (end X Y) STROKE_DEF FILL_DEF)

; Metin
(text "METİN" (at X Y [ANGLE]) (effects ...))
```

**`fill` token (şematik/sembol için):**
```scheme
(fill (type none|outline|background))
```

### Pin Tokeni

```scheme
(pin
  ELEKTRIKSEL_TIP
  GRAFIK_STIL
  (at X Y ANGLE)          ; sadece 0, 90, 180, 270 desteklenir
  (length MM)
  (name "AD" (effects ...))
  (number "NUMARA" (effects ...))
)
```

**Elektriksel tipler:**

| Token | Açıklama |
|-------|----------|
| `input` | Giriş |
| `output` | Çıkış |
| `bidirectional` | Çift yönlü |
| `tri_state` | Üç durumlu çıkış |
| `passive` | Pasif |
| `free` | İç bağlantısız |
| `unspecified` | Belirsiz |
| `power_in` | Güç girişi |
| `power_out` | Güç çıkışı |
| `open_collector` | Açık kollektör |
| `open_emitter` | Açık emiter |
| `no_connect` | Bağlantı yok |

**Grafik stiller:** `line`, `inverted`, `clock`, `inverted_clock`, `input_low`,
`clock_low`, `output_low`, `edge_clock_high`, `non_logic`

---

## Grup Tokeni

```scheme
(group "ADI"
  (id UUID)
  (members UUID1 UUID2 ... UUIDN)
)
```

---

## Library Identifier Formatı

```
"KUTUPHANE_TAKMA_ADI:GIRIS_ADI"
```

⚠️ Kütüphane dosyaları `KUTUPHANE_TAKMA_ADI`'nı içermez — sadece `GIRIS_ADI` saklanır.

---

## Python ile S-Expression Parse Etme

KiCad Action Plugin'lerinde veya scripting consolunda `pcbnew` modülü ile native okuma:

```python
import pcbnew

# PCB yükle
board = pcbnew.LoadBoard("devre.kicad_pcb")

# Footprint'leri oku
for fp in board.GetFootprints():
    print(fp.GetReference(), fp.GetPosition())

# Footprint ekle
fp = pcbnew.FootprintLoad("MyCoolLib", "SOT23")
board.Add(fp)
pcbnew.Refresh()
```

Ham S-expression parse etmek için lightweight Python parser:

```python
def parse_sexpr(text):
    """Minimal KiCad sexpr parser. Nested list döner."""
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

# Kullanım:
with open("devre.kicad_pcb", encoding="utf-8") as f:
    tree = parse_sexpr(f.read())
```

### S-Expression Üretme (Python)

```python
def to_sexpr(obj, indent=0):
    """Python listesini KiCad sexpr formatına çevir."""
    pad = "  " * indent
    if isinstance(obj, list):
        if not obj:
            return "()"
        inner = " ".join(to_sexpr(x) for x in obj)
        # Uzun satırları break et
        if len(inner) > 80:
            child_pad = "  " * (indent + 1)
            lines = "\n".join(f"{child_pad}{to_sexpr(x, indent+1)}" for x in obj)
            return f"(\n{lines}\n{pad})"
        return f"({inner})"
    elif isinstance(obj, str):
        # Token mu string mi?
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

## Kritik Notlar ve Tuzaklar

1. **Koordinat hassasiyeti:** `round(val, 6)` kullan (PCB), `round(val, 4)` kullan (şematik)
2. **UUID üretimi:** `uuid.uuid4()` Python'da yeterli, KiCad uyumlu v4 UUID üretir
3. **Timestamp (tedit):** `format(int(time.time()), 'X')` — hex formatında
4. **fp_text zorunluluğu:** `reference` ve `value` her footprint'te zorunlu; bulunmazsa KiCad şikayet eder
5. **Layer isimleri:** canonical isimler her zaman İngilizce — kullanıcı isimleri sadece görüntülemedir
6. **KiCad 7 değişiklikleri:** `width` token → `stroke` token; `dash_dot_dot` eklendi; TrueType `face` token eklendi
7. **Sürüm uyumluluğu:** KiCad 6 öncesi `footprint` yerine `module` kullanırdı
8. **Wire/Bus sözdizimi:** `(start X Y)(end X Y)` DEĞİL — `(pts (xy X1 Y1)(xy X2 Y2))` kullanır
9. **Track/Via UUID farkı:** PCB track ve via'larda `uuid` değil `tstamp UUID` kullanılır
10. **Symbol `instances` bloğu:** Şematik sembol yerleştirme token'ı, hiyerarşik tasarımlarda `instances → project → path → reference/unit` zinciri içerir; üçüncü parti üreticide bu blok doğru doldurulmazsa netlist çıktısı bozulur
11. **Şematik `generator` uyarısı:** `eeschema` ve `kicad_symbol_editor` yalnızca KiCad'a ayrılmıştır; 3. parti araçlarda kendi kimliğini kullan
12. **lib_symbols:** Şematik dosyası, kullandığı tüm sembollerin bir kopyasını `lib_symbols` içinde saklar — kütüphane olmadan da açılabilir
13. **Hierarchical sheet pin → label eşleşmesi:** Sheet içindeki `pin` adı, alt şematikteki `hierarchical_label` adıyla **harf harf aynı** olmalı; aksi hâlde bağlantı kurulmaz

---

## Rust ile S-Expression Üretme

KiCad dosyaları Rust'ta üretmek veya manipüle etmek istiyorsan **önce şu dosyayı oku:**

> 📄 `references/rust-macro.md` — Rust `Node/Atom` AST enum'u, `macro_rules! sexpr!` DSL,
> `Display` serialize, pretty-print, ve hazır helper fonksiyonlar:
> `at()`, `layer()`, `layers()`, `pts()`, `stroke()`, `fp_line()`, `fp_text()`,
> `smd_pad()`, `wire()`, `junction()`, `build_r0603()` tam örneği

**Hangi durumda oku:**
- Rust'ta footprint / şematik üretici yazıyorsan
- `macro_rules!` S-expression DSL'i lazımsa
- `Node::List` / `Atom::Raw` vs `Atom::Str` farkını anlamak istiyorsan
- UUID üretimi veya `tedit` timestamp formatı lazımsa

---

## Referans Dosyaları

Daha fazla detay için bu dosyaları oku:

- `references/rust-macro.md` — **Rust macro & AST** — `Node/Atom` enum, `sexpr!` macro, helper API, tam footprint örneği
- `references/layers.md` — Tüm canonical layer isimleri, wildcard kullanımı, Python pcbnew sabitleri
- `references/pad.md` — Pad token tam referansı, drill, custom pad, zone bağlantı tipleri
- `references/schematic.md` — Şematik format (wire, bus, junction, label, symbol instance, hierarchical sheet, instances bloğu)
- `references/board.md` — PCB board formatı (segment, via, arc, net, stackup, gerçek örnek)
- `references/klc-symbols.md` — **KiCad Library Convention (KLC)** — sembol oluşturma kuralları, pin grid, fill, RefDes tablosu, power sembol yapısı, Python üretici şablonu
- `references/symbol-libraries.md` — **Resmi kütüphane kataloğu** — 130+ kütüphane adı/açıklaması, Device kütüphanesi içeriği, "hangi kütüphanede?" hızlı arama tablosu
- `references/symbol-examples.md` — **Annotated gerçek sembol örnekleri** — R, op-amp, GND power, MCU, kristal, aktif düşük pin, pin yığma, extends kalıbı
