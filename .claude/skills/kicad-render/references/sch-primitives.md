# Şematik Primitifler — wire, bus, junction, label, sheet

> Kaynak: `eeschema/sch_painter.cpp`, `sch_label.cpp`, `sch_label.h`

---

## Wire ve Bus

```python
# wire: (pts (xy X1 Y1)(xy X2 Y2))
def render_wire(ctx, item, scale):
    p = item['pts']                         # [[x1,y1],[x2,y2]]
    ctx.beginPath()
    ctx.moveTo( p[0][0]*scale, -p[0][1]*scale)
    ctx.lineTo( p[1][0]*scale, -p[1][1]*scale)
    ctx.strokeStyle = WIRE_COLOR            # '#15BD6F'
    ctx.lineWidth   = DEFAULT_WIRE_WIDTH * scale   # ~0.15mm
    ctx.lineCap     = 'round'
    ctx.stroke()

# bus: tamamen aynı, sadece daha kalın + farklı renk
def render_bus(ctx, item, scale):
    p = item['pts']
    ctx.beginPath()
    ctx.moveTo( p[0][0]*scale, -p[0][1]*scale)
    ctx.lineTo( p[1][0]*scale, -p[1][1]*scale)
    ctx.strokeStyle = BUS_COLOR             # '#1FB8D3'
    ctx.lineWidth   = DEFAULT_BUS_WIDTH * scale    # wire × 3 ≈ 0.45mm
    ctx.lineCap     = 'round'
    ctx.stroke()
```

---

## Bus Entry

Bus_entry: `at(X,Y)` → `(at.x + size.w, at.y + size.h)` köşegeni.
`size` bir delta'dır, negatif olabilir (eğim yönünü belirler).

```python
def render_bus_entry(ctx, item, scale):
    x, y = item['at']
    sw, sh = item['size']                   # delta, negatif olabilir
    ctx.beginPath()
    ctx.moveTo( x*scale,       -y*scale)
    ctx.lineTo((x+sw)*scale,  -(y+sh)*scale)
    ctx.strokeStyle = BUS_COLOR
    ctx.lineWidth   = DEFAULT_WIRE_WIDTH * scale
    ctx.stroke()
```

---

## Junction

Dolgu daire. `diameter=0` → varsayılan (~1mm).

```python
def render_junction(ctx, item, scale):
    x, y = item['at']
    d    = item.get('diameter', 0)
    r    = (d/2 if d > 0 else DEFAULT_JUNCTION_RADIUS) * scale
    # renk overridi veya tema rengi
    color = item.get('color') or JUNCTION_COLOR   # '#15BD6F'
    ctx.beginPath()
    ctx.arc(x*scale, -y*scale, r, 0, 2*math.pi)
    ctx.fillStyle = color
    ctx.fill()
```

---

## No Connect

X işareti: merkezden ±`NO_CONNECT_SIZE` (≈1mm) iki çapraz çizgi.

```python
NO_CONNECT_SIZE = 1.0   # mm

def render_no_connect(ctx, item, scale):
    x, y = item['at']
    d    = NO_CONNECT_SIZE * scale
    px, py = x*scale, -y*scale
    ctx.beginPath()
    ctx.moveTo(px-d, py-d);  ctx.lineTo(px+d, py+d)
    ctx.moveTo(px+d, py-d);  ctx.lineTo(px-d, py+d)
    ctx.strokeStyle = NO_CONNECT_COLOR   # '#EE4040'
    ctx.lineWidth   = 0.2 * scale
    ctx.stroke()
```

---

## Polyline (grafik çizgi)

```python
def render_polyline(ctx, item, scale):
    pts  = item['pts']
    fill = item.get('fill', {}).get('type', 'none')
    ctx.beginPath()
    ctx.moveTo(pts[0][0]*scale, -pts[0][1]*scale)
    for p in pts[1:]:
        ctx.lineTo(p[0]*scale, -p[1]*scale)
    if fill != 'none':
        ctx.closePath()
        ctx.fillStyle = NOTES_COLOR if fill == 'outline' else SCH_BACKGROUND
        ctx.fill()
    w = item.get('stroke',{}).get('width', DEFAULT_LINE_WIDTH)
    ctx.strokeStyle = NOTES_COLOR        # '#FFFFFF' veya tema
    ctx.lineWidth   = max(w, DEFAULT_LINE_WIDTH) * scale
    ctx.stroke()
```

---

## Text

```python
def render_text(ctx, item, scale):
    x, y    = item['at'][:2]
    angle   = item['at'][2] if len(item['at']) > 2 else 0
    effects = item.get('effects', {})
    font    = effects.get('font', {})
    size_h  = font.get('size', [1.27, 1.27])[0]     # mm, height
    bold    = 'bold' in font
    italic  = 'italic' in font
    justify = effects.get('justify', 'left')

    ctx.save()
    ctx.translate(x*scale, -y*scale)
    ctx.rotate(-math.radians(angle))

    ctx.font        = f"{'bold ' if bold else ''}{'italic ' if italic else ''}{size_h*scale}px KiCad Font, monospace"
    ctx.fillStyle   = TEXT_COLOR
    ctx.textAlign   = 'left' if 'left' in justify else ('right' if 'right' in justify else 'center')
    ctx.textBaseline = 'middle'
    ctx.fillText(item['text'], 0, 0)
    ctx.restore()
```

---

## Local Label

Local label: metin + `at` noktasından küçük bağlantı noktası.
KiCad kaynak: `sch_painter.cpp` → `draw(const SCH_LABEL*)`.
Bağlantı noktası metnin sol-alt köşesidir.

```python
def render_label(ctx, item, scale):
    x, y  = item['at'][:2]
    angle = item['at'][2] if len(item['at']) > 2 else 0

    ctx.save()
    ctx.translate(x*scale, -y*scale)
    ctx.rotate(-math.radians(angle))

    # Metin
    effects = item.get('effects', {})
    size_h  = effects.get('font',{}).get('size',[1.27,1.27])[0]
    ctx.font        = f"{size_h*scale}px KiCad Font, monospace"
    ctx.fillStyle   = LABEL_COLOR        # '#F5F524'
    ctx.textAlign   = 'left'
    ctx.textBaseline = 'bottom'
    ctx.fillText(item['label'], 0, 0)

    # Bağlantı noktası işareti (küçük kare veya nokta — opsiyonel)
    ctx.fillStyle = WIRE_COLOR
    ctx.fillRect(-2, -2, 4, 4)

    ctx.restore()
```

---

## Global Label ve Hierarchical Label

KiCad kaynak: `sch_label.cpp::GetSchematicTextOffset`, `sch_painter.cpp`.
`SPIN_STYLE` yönü belirler: RIGHT=0, UP=1, LEFT=2, DOWN=3 (angle/90).

### Shape poligonları

`shape` değerine göre farklı poligon çerçevesi çizilir.
`tw` = metin piksel genişliği (mm cinsinden), `h` = margin (≈1.27mm).

```python
def label_polygon(shape, tw_mm, h=1.27):
    """
    shape: 'input'|'output'|'bidirectional'|'tri_state'|'passive'
    Döndürülen pts: (x,y) listesi, origin bağlantı noktası.
    Tüm ölçüler mm cinsinden; render sırasında scale ile çarp.
    """
    w = tw_mm + h          # toplam genişlik
    if shape == 'input':
        # Sol ok ucu içe bakan
        return [(0,0), (h,-h), (w+h,-h), (w+h,h), (h,h)]
    elif shape == 'output':
        # Sağ ok ucu dışa bakan
        return [(0,0), (0,-h), (w,-h), (w+h,0), (w,h), (0,h)]
    elif shape == 'bidirectional':
        # Her iki tarafta ok
        return [(0,0), (h,-h), (w,-h), (w+h,0), (w,h), (h,h)]
    elif shape == 'tri_state':
        # bidirectional ile aynı şekil
        return label_polygon('bidirectional', tw_mm, h)
    else:  # passive
        # Düz dikdörtgen
        return [(0,-h), (w+h,-h), (w+h,h), (0,h)]
```

```python
def render_global_label(ctx, item, scale):
    x, y    = item['at'][:2]
    angle   = item['at'][2] if len(item['at']) > 2 else 0
    shape   = item.get('shape', 'passive')
    effects = item.get('effects', {})
    size_h  = effects.get('font',{}).get('size',[1.27,1.27])[0]

    # Metin genişliğini tahmin et (karakter başı ~0.6 × height)
    text      = item['label']
    tw_mm     = len(text) * size_h * 0.6
    pts_mm    = label_polygon(shape, tw_mm)

    ctx.save()
    ctx.translate(x*scale, -y*scale)
    ctx.rotate(-math.radians(angle))

    # Çerçeve
    ctx.beginPath()
    ctx.moveTo(pts_mm[0][0]*scale, -pts_mm[0][1]*scale)
    for p in pts_mm[1:]:
        ctx.lineTo(p[0]*scale, -p[1]*scale)
    ctx.closePath()
    ctx.strokeStyle = GLOBAL_LABEL_COLOR  # '#A8A800'
    ctx.lineWidth   = 0.15 * scale
    ctx.stroke()

    # Metin (çerçeve içinde, margin kadar içeride)
    margin = 1.27 * scale
    ctx.font        = f"{size_h*scale}px KiCad Font, monospace"
    ctx.fillStyle   = GLOBAL_LABEL_COLOR
    ctx.textAlign   = 'left'
    ctx.textBaseline = 'middle'
    ctx.fillText(text, margin, 0)

    ctx.restore()

# Hierarchical label: aynı mantık, farklı renk + biraz farklı şekil
def render_hier_label(ctx, item, scale):
    item = dict(item)
    item['label'] = item.get('label', item.get('text',''))
    render_global_label(ctx, item, scale)   # rengi hier_label_color yap
```

---

## Sheet (hierarchical)

Sheet kutusu: `at(X,Y)`, `size(W,H)`, pin'ler kenarlarda.

```python
def render_sheet(ctx, item, scale):
    x, y  = item['at']
    w, h  = item['size']

    # Arka plan dolgusu (opsiyonel)
    ctx.fillStyle   = SHEET_BG_COLOR     # '#1A1A28' veya şeffaf
    ctx.fillRect(x*scale, -(y+h)*scale, w*scale, h*scale)

    # Kenar
    ctx.strokeStyle = SHEET_COLOR        # '#5E76C5'
    ctx.lineWidth   = 0.15 * scale
    ctx.strokeRect(x*scale, -(y+h)*scale, w*scale, h*scale)

    # İsim (üstte, dışarıda) ve dosya (altta, dışarıda)
    for prop in item.get('properties', []):
        if prop['key'] == 'Sheet name':
            draw_label_text(ctx, prop['value'], x, y, scale, 'top')
        elif prop['key'] == 'Sheet file':
            draw_label_text(ctx, prop['value'], x, y+h, scale, 'bottom')

    # Pin'ler
    for pin in item.get('pins', []):
        render_sheet_pin(ctx, pin, scale)

def render_sheet_pin(ctx, pin, scale):
    """Sheet pin: kenar üzerinde, ok şekli."""
    px, py  = pin['at'][:2]
    angle   = pin['at'][2] if len(pin['at']) > 2 else 0
    shape   = pin.get('shape', 'input')
    name    = pin['name']

    ctx.save()
    ctx.translate(px*scale, -py*scale)
    ctx.rotate(-math.radians(angle))

    # Küçük ok poligonu (pin ucu)
    H = 1.0 * scale   # pin yüksekliği
    ctx.beginPath()
    ctx.moveTo(0, 0)
    ctx.lineTo(-H*2, -H); ctx.lineTo(-H*2, H); ctx.closePath()
    ctx.fillStyle   = SHEET_PIN_COLOR    # '#5E76C5'
    ctx.fill()

    # Pin adı
    size_h = pin.get('effects',{}).get('font',{}).get('size',[1.27,1.27])[0]
    ctx.font        = f"{size_h*scale}px KiCad Font, monospace"
    ctx.fillStyle   = SHEET_PIN_COLOR
    ctx.textAlign   = 'right'
    ctx.textBaseline = 'middle'
    ctx.fillText(name, -H*2.5, 0)

    ctx.restore()
```

---

## Tam şematik render sırası

KiCad `sch_painter.cpp` layer sırasından türetildi:

```python
def render_schematic(ctx, sch, scale):
    # 0. Arka plan
    ctx.fillStyle = SCH_BACKGROUND
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height)

    # 1. Wire, bus, bus_entry (LAYER_WIRE / LAYER_BUS)
    for item in sch.get('wires', []):      render_wire(ctx, item, scale)
    for item in sch.get('buses', []):      render_bus(ctx, item, scale)
    for item in sch.get('bus_entries', []): render_bus_entry(ctx, item, scale)

    # 2. Grafik polyline + text (LAYER_NOTES)
    for item in sch.get('polylines', []):  render_polyline(ctx, item, scale)
    for item in sch.get('texts', []):      render_text(ctx, item, scale)

    # 3. Semboller — gövde + pin'ler (LAYER_DEVICE + LAYER_PIN)
    lib = sch.get('lib_symbols', {})
    for item in sch.get('symbols', []):
        render_symbol(ctx, item, lib, scale)

    # 4. Junction + no_connect (LAYER_JUNCTION / en üstte)
    for item in sch.get('junctions', []):  render_junction(ctx, item, scale)
    for item in sch.get('no_connects', []): render_no_connect(ctx, item, scale)

    # 5. Etiketler (LAYER_LOCLABEL / LAYER_GLOBLABEL)
    for item in sch.get('labels', []):       render_label(ctx, item, scale)
    for item in sch.get('global_labels', []): render_global_label(ctx, item, scale)
    for item in sch.get('hier_labels', []):   render_hier_label(ctx, item, scale)

    # 6. Sheet kutuları (LAYER_HIERLABEL — en son)
    for item in sch.get('sheets', []):     render_sheet(ctx, item, scale)
```
