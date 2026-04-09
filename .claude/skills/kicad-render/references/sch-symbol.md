# Şematik Sembol Render — TRANSFORM, Flatten, Pin

> Kaynak: `eeschema/sch_symbol.cpp`, `sch_painter.cpp`, `sch_pin.cpp`, `lib_symbol.cpp`
> (KiCad kaynak kodu, Nisan 2026)

---

## Temel mimari (KiCad kaynak kodundan)

```
LIB_SYMBOL  ──── grafik tanımı (polyline, arc, circle, rectangle, pin)
     │            lib koordinatlarında, origin'de
     │ flatten()
SCH_SYMBOL  ──── instance: at(x,y), m_transform, m_unit, m_bodyStyle
     │            m_libSymbol = flatten edilmiş kopya
     │
SCH_PIN     ──── instance pin, LIB_PIN'e referans,
                  pozisyon m_transform ile dönüştürülmüş
```

**Kritik:** `SCH_SYMBOL::m_libSymbol` flatten edilmiş bir `LIB_SYMBOL`'dür.
Şematik dosyası `lib_symbols` bölümünde bu flatten edilmiş kopyayı saklar.
`extends` keyword ile kalıtım alan semboller için `Flatten()` çağrısı gerekir.

---

## TRANSFORM matrix — KiCad'ın integer matrisi

KiCad C++ kodu `TRANSFORM` sınıfını kullanır:

```cpp
struct TRANSFORM {
    int x1, x2;  // sütun 1
    int y1, y2;  // sütun 2
    // Uygulama: x' = x1*x + x2*y,  y' = y1*x + y2*y
};
```

Değerler yalnızca `{-1, 0, 1}` — izometrik dönüşüm.

### Standart dönüşüm tablosu

| Durum | x1 | x2 | y1 | y2 | S-expr `at` açısı |
|-------|----|----|----|----|-------------------|
| Normal (0°) | 1 | 0 | 0 | -1 | 0 |
| 90° CCW | 0 | 1 | 1 | 0 | 90 |
| 180° | -1 | 0 | 0 | 1 | 180 |
| 270° CCW | 0 | -1 | -1 | 0 | 270 |
| Mirror X | -1 | 0 | 0 | -1 | 0 + mirror x |
| Mirror Y | 1 | 0 | 0 | 1 | 0 + mirror y |

**Neden y2=-1 normal durumda?** Şematik Y yukarı pozitif, canvas Y aşağı pozitif.
Normal transform zaten Y'yi çevirir — dolayısıyla `sch_to_px` ile çakışmamak için
lib koordinatlarını transform'dan geçirirken Y çevirme **yapma**.

### Python TRANSFORM uygulaması

```python
# S-expr'den TRANSFORM oluştur
TRANSFORMS = {
    (0,   False, False): (1, 0, 0, -1),
    (90,  False, False): (0, 1, 1,  0),
    (180, False, False): (-1,0, 0,  1),
    (270, False, False): (0,-1,-1,  0),
    (0,   True,  False): (-1,0, 0, -1),   # mirror x
    (0,   False, True ): (1, 0, 0,  1),   # mirror y
    (90,  True,  False): (0,-1, 1,  0),
    (90,  False, True ): (0, 1,-1,  0),
    # ... vb. (kombinasyonlar için rotate sonra mirror uygula)
}

def make_transform(angle_deg, mirror_x=False, mirror_y=False):
    """Açı ve mirror'dan TRANSFORM (x1,x2,y1,y2) döndür."""
    a = math.radians(angle_deg)
    cos_a, sin_a = round(math.cos(a)), round(math.sin(a))
    x1, x2 = cos_a, -sin_a
    y1, y2 = sin_a,  cos_a
    if mirror_x: x1, x2 = -x1, -x2   # X ekseninde ayna
    if mirror_y: y1, y2 = -y1, -y2   # Y ekseninde ayna
    return (x1, x2, y1, y2)

def transform_point(x1,x2,y1,y2, lx, ly):
    """Lib koordinatını instance koordinatına çevir."""
    return x1*lx + x2*ly, y1*lx + y2*ly
```

---

## LIB_SYMBOL::Flatten — kalıtım çözümleme

`extends` keyword ile parent sembolden kalıtım alan semboller flatten edilmelidir.
KiCad C++ `LIB_SYMBOL::Flatten()` parent'ın drawing item'larını kopyalar.

```python
def flatten_lib_symbol(sym, lib_symbols):
    """
    sym: parse edilmiş sembol node'u
    lib_symbols: tüm lib_symbols dict'i {id: node}
    Döner: flatten edilmiş çizim item listesi
    """
    items = list(sym.get('drawing', []))
    parent_id = sym.get('extends')
    while parent_id:
        parent = lib_symbols.get(parent_id)
        if not parent:
            break
        # parent'ın çizim item'larını ekle (pinler dahil)
        items = list(parent.get('drawing', [])) + items
        parent_id = parent.get('extends')
    return items
```

---

## Unit ve Body Style filtreleme

KiCad C++ `SCH_PAINTER`: `m_unit` ve `m_bodyStyle` değerleriyle eşleşen
alt-semboller çizilir.

Alt-sembol isimlendirme: `"PARENT_UNIT_STYLE"`
- `UNIT=0` → tüm unitlerde ortak
- `STYLE=1` → normal, `STYLE=2` → De Morgan

```python
def filter_units(lib_sym_items, unit, body_style=1):
    """
    lib_sym içindeki child symbol token'larından
    unit ve body_style eşleşen primitifleri döndür.
    """
    result = []
    for child in lib_sym_items:
        if child['type'] != 'symbol':
            continue
        name   = child['id']          # ör: "Device_R_1_1"
        parts  = name.rsplit('_', 2)
        if len(parts) == 3:
            try:
                child_unit  = int(parts[1])
                child_style = int(parts[2])
            except ValueError:
                continue
            # unit=0 → her yerde geç; style=0 → her iki style'da geç
            if child_unit not in (0, unit):
                continue
            if child_style not in (0, body_style):
                continue
        result.extend(child.get('drawing', []))
    return result
```

---

## Sembol render — tam akış

```python
def render_symbol(ctx, instance, lib_symbols, scale):
    lib_id  = instance['lib_id']           # ör: "Device:R"
    lib_sym = lib_symbols.get(lib_id)
    if not lib_sym:
        return                             # lib bulunamadı

    at_x    = instance['at'][0]
    at_y    = instance['at'][1]
    angle   = instance['at'][2] if len(instance['at']) > 2 else 0
    mirror  = instance.get('mirror')       # 'x', 'y', veya None
    unit    = instance.get('unit', 1)

    # TRANSFORM
    mx = mirror == 'x'
    my = mirror == 'y'
    tx = make_transform(angle, mx, my)     # (x1,x2,y1,y2)

    # Flatten ve unit filtrele
    all_items  = flatten_lib_symbol(lib_sym, lib_symbols)
    draw_items = filter_units(all_items, unit)

    ctx.save()
    ctx.translate(at_x*scale, -at_y*scale)

    # Lib primitifleri çiz
    for prim in draw_items:
        render_lib_primitive(ctx, prim, tx, scale)

    # Property metinlerini (reference, value) çiz
    for prop in instance.get('properties', []):
        if not prop.get('hide') and prop['key'] in ('Reference','Value'):
            render_field_text(ctx, prop, tx, scale)

    ctx.restore()


def render_lib_primitive(ctx, prim, tx, scale):
    """tx = (x1,x2,y1,y2) TRANSFORM tuple'ı."""
    t = prim['type']
    x1,x2,y1,y2 = tx

    def tp(lx, ly):  # transform + Y çevirme (canvas Y aşağı)
        nx, ny = transform_point(x1,x2,y1,y2, lx, ly)
        return nx*scale, -ny*scale   # lib'de Y yukarı, ekrana Y çevir

    if t == 'polyline':
        pts = prim['pts']
        ctx.beginPath()
        ctx.moveTo(*tp(pts[0][0], pts[0][1]))
        for p in pts[1:]: ctx.lineTo(*tp(p[0], p[1]))
        apply_stroke_fill(ctx, prim, scale)

    elif t == 'rectangle':
        s  = prim['start'];  e = prim['end']
        # 4 köşeyi transform'dan geçir
        corners = [tp(s[0],s[1]), tp(e[0],s[1]),
                   tp(e[0],e[1]), tp(s[0],e[1])]
        ctx.beginPath()
        ctx.moveTo(*corners[0])
        for c in corners[1:]: ctx.lineTo(*c)
        ctx.closePath()
        apply_stroke_fill(ctx, prim, scale)

    elif t == 'circle':
        cx,cy  = prim['center']
        radius = prim['radius']
        # Merkezi transform'dan geçir; radius scale ile büyür
        px, py = tp(cx, cy)
        ctx.beginPath()
        ctx.arc(px, py, radius*scale, 0, 2*math.pi)
        apply_stroke_fill(ctx, prim, scale)

    elif t == 'arc':
        s = prim['start']; m = prim['mid']; e = prim['end']
        # Transform'dan geç, sonra arc hesapla
        ps = tp(s[0],s[1]); pm = tp(m[0],m[1]); pe = tp(e[0],e[1])
        center, r = arc_center_radius(ps, pm, pe)
        if center:
            t1, t2, ccw = arc_sweep(center, ps, pe, pm)
            ctx.beginPath()
            ctx.arc(center[0], center[1], r, t1, t2, ccw)
            ctx.strokeStyle = SYMBOL_COLOR
            ctx.lineWidth   = DEFAULT_LINE_WIDTH * scale
            ctx.stroke()

    elif t == 'pin':
        render_pin(ctx, prim, tp, scale)


def apply_stroke_fill(ctx, prim, scale):
    """Stroke ve fill uygula."""
    w     = prim.get('stroke',{}).get('width', DEFAULT_LINE_WIDTH)
    fill  = prim.get('fill',{}).get('type', 'none')
    ctx.lineWidth   = max(w, DEFAULT_LINE_WIDTH) * scale
    ctx.strokeStyle = SYMBOL_COLOR
    if fill == 'outline':
        ctx.fillStyle = SYMBOL_COLOR
        ctx.fill()
    elif fill == 'background':
        ctx.fillStyle = SYMBOL_BG_COLOR
        ctx.fill()
    ctx.stroke()
```

---

## Pin render — kaynak kodundan

KiCad `sch_painter.cpp::draw(const SCH_PIN*)` — `PIN_LAYOUT_CACHE` kullanır.

```python
def render_pin(ctx, pin, tp, scale):
    """
    pin: lib_symbol içindeki pin token'ı
    tp: transform fonksiyonu (lx,ly) → (px,py)
    """
    at        = pin['at']              # [x, y, angle]
    length    = pin.get('length', 2.54)
    graphic   = pin['graphic_style']   # line, inverted, clock, ...
    elec_type = pin['elec_type']       # input, output, passive, ...

    # Pin kök ve uç
    lx, ly    = at[0], at[1]
    angle_rad = math.radians(at[2])
    ex_lib    = lx + length * math.cos(angle_rad)
    ey_lib    = ly + length * math.sin(angle_rad)

    px, py = tp(lx, ly)
    ex, ey = tp(ex_lib, ey_lib)

    # Gövde çizgisi
    ctx.beginPath()
    ctx.moveTo(px, py)
    ctx.lineTo(ex, ey)
    ctx.strokeStyle = PIN_COLOR        # '#FF8000'
    ctx.lineWidth   = 0.15 * scale
    ctx.lineCap     = 'round'
    ctx.stroke()

    # Endpoint marker (grafik stil)
    BUBBLE_R = 0.397 * scale           # inverted daire yarıçapı

    if graphic == 'inverted' or graphic == 'inverted_clock':
        # Küçük daire, pin ucunun ötesinde
        dx = math.cos(angle_rad) * 0.397
        dy = math.sin(angle_rad) * 0.397
        bx, by = tp(ex_lib + dx, ey_lib + dy)
        ctx.beginPath()
        ctx.arc(bx, by, BUBBLE_R, 0, 2*math.pi)
        ctx.strokeStyle = PIN_COLOR
        ctx.lineWidth   = 0.15 * scale
        ctx.stroke()

    if graphic in ('clock', 'inverted_clock', 'clock_low', 'edge_clock_high'):
        # Üçgen: pin ucunda, gövde yönüne dik
        perp = angle_rad + math.pi/2
        CLOCK_SIZE = 0.794 * scale
        tx1, ty1 = tp(ex_lib, ey_lib + 0.794)
        tx2, ty2 = tp(ex_lib, ey_lib - 0.794)
        tx3, ty3 = tp(ex_lib + 0.794, ey_lib)
        ctx.beginPath()
        ctx.moveTo(tx1, ty1)
        ctx.lineTo(tx3, ty3)
        ctx.lineTo(tx2, ty2)
        ctx.strokeStyle = PIN_COLOR
        ctx.lineWidth   = 0.15 * scale
        ctx.stroke()

    if graphic in ('input_low', 'output_low', 'clock_low'):
        # Ters L şekli (active low bar)
        L = 0.794 * scale
        ctx.beginPath()
        ctx.moveTo(ex, ey)
        ctx.lineTo(ex, ey - L)
        ctx.lineTo(ex + L, ey)
        ctx.strokeStyle = PIN_COLOR
        ctx.lineWidth   = 0.15 * scale
        ctx.stroke()

    # Dangling indicator (pin bağlı değilse küçük kare)
    # Bağlantı kontrolü yapılıyorsa ekle
    # if pin.get('dangling'):
    #     ctx.strokeRect(px-3, py-3, 6, 6)

    # Pin adı ve numarası
    name_eff = pin.get('name_effects', {})
    num_eff  = pin.get('number_effects', {})
    if not name_eff.get('hide'):
        _render_pin_label(ctx, pin.get('name',''), px, py, ex, ey, 'name', scale)
    if not num_eff.get('hide'):
        _render_pin_label(ctx, pin.get('number',''), px, py, ex, ey, 'number', scale)


def _render_pin_label(ctx, text, px, py, ex, ey, role, scale):
    """Pin adı veya numarasını uygun konuma yaz."""
    if not text or text == '~':
        return
    SIZE = 1.0 * scale
    ctx.font        = f"{SIZE}px KiCad Font, monospace"
    ctx.fillStyle   = PIN_COLOR
    ctx.textBaseline = 'middle'
    # İsim: gövdenin iç tarafı (uç noktanın ötesi)
    # Numara: gövdenin orta noktası
    mid_x = (px + ex) / 2
    mid_y = (py + ey) / 2
    offset = 2
    if role == 'name':
        ctx.textAlign = 'left'
        ctx.fillText(text, ex + offset, ey)
    else:
        ctx.textAlign = 'center'
        ctx.fillText(text, mid_x, mid_y - offset)
```

---

## Kritik tuzaklar (kaynak koddan çıkarıldı)

1. **TRANSFORM Y çevirme:** Lib koordinatları Y yukarı pozitif. `tp()` fonksiyonu
   hem transform uygular hem `-ny` ile canvas Y'ye çevirir. Bunu iki kez yapma.

2. **unit=0 ortak grafikler:** Alt-sembol ismi `_0_` içeriyorsa tüm unit'lerde
   çizilir. `filter_units` fonksiyonunda `child_unit == 0` kontrolü zorunlu.

3. **LIB_SYMBOL::Flatten:** `extends` ile kalıtım alan sembollerde parent'ın
   çizim item'ları **önce** gelmeli (child üstüne çizilir). Özellikle `Device:C`
   gibi semboller başka bir base sembolden extend eder.

4. **Pin açıları tam derece:** Lib pin `at[2]` tam derece (90, 180, 270, 0).
   Sembol instance `at[2]` de tam derece. `round(angle/90)*90` ile snaple.

5. **De Morgan (bodyStyle=2):** Mantık kapılarında alternatif sembol şekli.
   `_X_2` alt-sembolü. Basit render için `body_style=1` yeterli.

6. **PIN_LAYOUT_CACHE:** KiCad C++ pahalı text extent hesaplarını cache'ler.
   Python'da `ctx.measureText()` ile approximation yeterli; her pin için
   ayrıca cache tutmana gerek yok.

7. **Sembol fields (Reference, Value):** `at` koordinatları GLOBAL koordinattır —
   instance `at`'e göre offset DEĞİL. Direkt sch_to_px ile çevir.
