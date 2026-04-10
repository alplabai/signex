# PCB Render — Primitifler, Footprint, PADSTACK

> Kaynak: `pcbnew/pcb_painter.cpp`, `pcbnew/pad.cpp`, `pcbnew/pcb_track.cpp`,
> `pcbnew/zone.cpp`, `common/eda_shape.cpp` — KiCad kaynak kodu, Nisan 2026

---

## PCB_PAINTER::Draw() dispatcher

KiCad `pcb_painter.cpp::Draw()` item tipine göre dispatch eder.
Her item tipi için render metodları:

| C++ sınıfı | Token / tip | Draw metodu |
|-----------|------------|------------|
| `PCB_TRACK` | `segment` | `draw(PCB_TRACK*, layer)` — `DrawSegment()` |
| `PCB_ARC` | `arc` (track) | `draw(PCB_ARC*, layer)` — arc hesapla + DrawArc |
| `PCB_VIA` | `via` | `draw(PCB_VIA*, layer)` — via + hole |
| `FOOTPRINT` | `footprint` | `draw(FOOTPRINT*, layer)` — her child item |
| `PAD` | `pad` | `draw(PAD*, layer)` — PADSTACK'e göre şekil |
| `PCB_SHAPE` | `gr_*`, `fp_*` | `draw(PCB_SHAPE*, layer)` — EDA_SHAPE dispatch |
| `ZONE` | `zone` | `draw(ZONE*, layer)` — filled_polygon |
| `PCB_TEXT` | `gr_text`, `fp_text` | `draw(PCB_TEXT*, layer)` |
| `PCB_TEXTBOX` | `fp_text_box` | `draw(PCB_TEXTBOX*, layer)` |
| `PCB_DIMENSION_*` | `dimension` | `draw(PCB_DIMENSION*, layer)` |

---

## Koordinat sistemi

**PCB: Y ekseni AŞAĞI pozitif** — Canvas Y ile aynı, çevirme gerekmez.

```python
def pcb_to_px(mm_x, mm_y, scale, origin_x, origin_y):
    return (mm_x - origin_x) * scale, (mm_y - origin_y) * scale

def bounding_box(items):
    """Tüm item'lardan bounding box hesapla."""
    xs, ys = [], []
    for item in items:
        for key in ('start','end','center','at'):
            pt = item.get(key)
            if isinstance(pt, list) and len(pt) >= 2:
                xs.append(pt[0]); ys.append(pt[1])
        for pt in item.get('pts', []):
            xs.append(pt[0]); ys.append(pt[1])
    if not xs: return 0, 0, 100, 100
    return min(xs), min(ys), max(xs), max(ys)
```

---

## PCB_TRACK (segment) — PCB_PAINTER::draw(PCB_TRACK*)

`PCB_TRACK` C++ sınıfı varsayılan 0.2mm genişlik kullanır.
Canvas'ta `round` lineCap ile çizilir (KiCad GAL DrawSegment).

```python
def render_segment(ctx, seg, layer_color, scale):
    """segment token: start, end, width, layer"""
    x1, y1 = seg['start']
    x2, y2 = seg['end']
    w = seg.get('width', 0.2) * scale

    ctx.beginPath()
    ctx.moveTo(x1*scale, y1*scale)
    ctx.lineTo(x2*scale, y2*scale)
    ctx.strokeStyle = layer_color
    ctx.lineWidth   = max(w, 0.5)   # minimum 0.5px görünürlük
    ctx.lineCap     = 'round'
    ctx.lineJoin    = 'round'
    ctx.stroke()
```

---

## PCB_ARC (arc track) — PCB_PAINTER::draw(PCB_ARC*)

Arc track: `(arc (start X Y)(mid X Y)(end X Y)(width W)(layer L))`.
KiCad GAL `DrawArc()` merkez+r+açı kullanır.

```python
def render_arc_track(ctx, item, layer_color, scale):
    s = item['start']; m = item['mid']; e = item['end']
    w = item.get('width', 0.2) * scale

    # arc_center_radius SKILL.md'den
    center, r = arc_center_radius(
        (s[0]*scale, s[1]*scale),
        (m[0]*scale, m[1]*scale),
        (e[0]*scale, e[1]*scale)
    )
    if not center:
        # Fallback: düz çizgi
        return render_segment(ctx, {'start':s,'end':e,'width':item.get('width',0.2)}, layer_color, scale)

    t1, t2, ccw = arc_sweep(center,
        (s[0]*scale, s[1]*scale),
        (e[0]*scale, e[1]*scale),
        (m[0]*scale, m[1]*scale)
    )
    ctx.beginPath()
    ctx.arc(center[0], center[1], r, t1, t2, ccw)
    ctx.strokeStyle = layer_color
    ctx.lineWidth   = max(w, 0.5)
    ctx.lineCap     = 'round'
    ctx.stroke()
```

---

## PCB_VIA — PCB_PAINTER::draw(PCB_VIA*, layer)

Via türleri: `through`, `blind_buried`, `micro`.
KiCad `draw(PCB_VIA*)` iki daire çizer: dış halka (bakır) + iç delik.

Özel layer'lar: `LAYER_VIA_HOLES` (delik rengi), `LAYER_VIA_HOLEWALLS` (duvar rengi).

```python
# Varsayılan tema renkleri (builtin_color_themes.h'dan):
VIA_HOLE_COLOR    = 'rgb(227,183,46,1)'   # LAYER_VIA_HOLES
VIA_HOLEWALLS     = 'rgb(236,236,236,1)'  # LAYER_VIA_HOLEWALLS
PLATED_HOLE_COLOR = 'rgb(194,194,0,1)'    # LAYER_PAD_PLATEDHOLES
NPTHOLE_COLOR     = 'rgb(26,196,210,1)'   # LAYER_NON_PLATEDHOLES

def render_via(ctx, via, layer_colors, scale):
    """
    via token: at, size (dış çap), drill (delik çapı),
               layers ["F.Cu","B.Cu"] veya via_type
    """
    x, y    = via['at'][0]*scale, via['at'][1]*scale
    size    = via.get('size', 1.6)
    drill   = via.get('drill', 0.8)

    outer_r  = size  / 2 * scale
    drill_r  = drill / 2 * scale
    wall_r   = drill_r + 0.1 * scale   # holewalls ince ring

    # 1. Bakır halka (aktif layer rengi veya via rengi)
    via_layer = via.get('layers', ['F.Cu', 'B.Cu'])
    color = layer_colors.get(via_layer[0], VIA_HOLE_COLOR)
    ctx.beginPath()
    ctx.arc(x, y, outer_r, 0, 2*math.pi)
    ctx.fillStyle = color
    ctx.fill()

    # 2. Holewalls ring (ince, via_holewalls rengi)
    ctx.beginPath()
    ctx.arc(x, y, wall_r, 0, 2*math.pi)
    ctx.strokeStyle = VIA_HOLEWALLS
    ctx.lineWidth   = (outer_r - drill_r) * 0.3
    ctx.stroke()

    # 3. Delik
    ctx.beginPath()
    ctx.arc(x, y, drill_r, 0, 2*math.pi)
    ctx.fillStyle = VIA_HOLE_COLOR
    ctx.fill()
```

---

## PCB_SHAPE (EDA_SHAPE) — gr_* ve fp_* grafikleri

`EDA_SHAPE` sınıfı `SHAPE_T` enum'ı ile tip tutar.
`pcb_painter.cpp::draw(PCB_SHAPE*)` → `getLineWidth()` + `getFillColor()` + GAL çağrısı.

```python
def render_pcb_shape(ctx, shape, layer_color, scale):
    t = shape['type']   # 'gr_line','gr_arc','gr_circle','gr_rect','gr_poly','bezier'...

    w    = get_stroke_width(shape) * scale
    fill = shape.get('fill', {}).get('type', 'none')

    ctx.strokeStyle = layer_color
    ctx.lineWidth   = max(w, 0.5)
    ctx.lineCap     = 'round'
    ctx.lineJoin    = 'round'

    if t in ('gr_line', 'fp_line'):
        s, e = shape['start'], shape['end']
        ctx.beginPath()
        ctx.moveTo(s[0]*scale, s[1]*scale)
        ctx.lineTo(e[0]*scale, e[1]*scale)
        ctx.stroke()

    elif t in ('gr_arc', 'fp_arc'):
        s = shape['start']; m = shape['mid']; e = shape['end']
        c, r = arc_center_radius(
            (s[0]*scale,s[1]*scale),
            (m[0]*scale,m[1]*scale),
            (e[0]*scale,e[1]*scale)
        )
        if c:
            t1,t2,ccw = arc_sweep(c,
                (s[0]*scale,s[1]*scale),
                (e[0]*scale,e[1]*scale),
                (m[0]*scale,m[1]*scale)
            )
            ctx.beginPath()
            ctx.arc(c[0], c[1], r, t1, t2, ccw)
            ctx.stroke()

    elif t in ('gr_circle', 'fp_circle'):
        cx, cy = shape['center']
        ex, ey = shape['end']
        r = math.hypot((ex-cx)*scale, (ey-cy)*scale)
        ctx.beginPath()
        ctx.arc(cx*scale, cy*scale, r, 0, 2*math.pi)
        if fill != 'none':
            ctx.fillStyle = layer_color; ctx.fill()
        ctx.stroke()

    elif t in ('gr_rect', 'fp_rect'):
        s, e = shape['start'], shape['end']
        x = min(s[0],e[0])*scale; y = min(s[1],e[1])*scale
        w_rect = abs(e[0]-s[0])*scale; h_rect = abs(e[1]-s[1])*scale
        if fill != 'none':
            ctx.fillStyle = layer_color
            ctx.fillRect(x, y, w_rect, h_rect)
        ctx.strokeRect(x, y, w_rect, h_rect)

    elif t in ('gr_poly', 'fp_poly'):
        pts = shape['pts']
        ctx.beginPath()
        ctx.moveTo(pts[0][0]*scale, pts[0][1]*scale)
        for p in pts[1:]: ctx.lineTo(p[0]*scale, p[1]*scale)
        ctx.closePath()
        if fill != 'none':
            ctx.fillStyle = layer_color; ctx.fill()
        ctx.stroke()

    elif t in ('bezier', 'fp_curve'):
        pts = shape['pts']  # 4 kontrol noktası
        p0,p1,p2,p3 = [(p[0]*scale,p[1]*scale) for p in pts]
        ctx.beginPath()
        ctx.moveTo(*p0)
        ctx.bezierCurveTo(p1[0],p1[1], p2[0],p2[1], p3[0],p3[1])
        ctx.stroke()


def get_stroke_width(shape):
    """stroke.width veya width token'ından genişlik al."""
    stroke = shape.get('stroke', {})
    w = stroke.get('width', shape.get('width', 0.1524))
    return max(w, 0.0)   # negatif olamaz
```

---

## PAD — PCB_PAINTER::draw(PAD*, layer)

### PADSTACK mimarisi (KiCad 9+)

`PAD` → `PADSTACK` → `COPPER_LAYER_PROPS` per-layer geometry.
`PADSTACK::Mode`: `NORMAL` (tek şekil), `FRONT_INNER_BACK` (3 şekil), `CUSTOM` (her layer ayrı).

Basit render için sadece `shape` + `size` + `angle` + `drill` kullan.

### Pad şekilleri

```python
def render_pad(ctx, pad, layer_color, scale):
    """pad token: at, size, shape, angle, drill, layers"""
    at    = pad['at']
    x, y  = at[0]*scale, at[1]*scale
    size  = pad.get('size', [1.6, 1.6])
    w, h  = size[0]*scale, size[1]*scale
    angle = pad.get('angle', at[2] if len(at)>2 else 0)
    shape = pad.get('shape', 'circle')

    ctx.save()
    ctx.translate(x, y)
    ctx.rotate(math.radians(angle))
    ctx.fillStyle   = layer_color
    ctx.strokeStyle = layer_color

    if shape == 'circle':
        ctx.beginPath()
        ctx.arc(0, 0, w/2, 0, 2*math.pi)
        ctx.fill()

    elif shape == 'rect':
        ctx.fillRect(-w/2, -h/2, w, h)

    elif shape == 'roundrect':
        rratio = pad.get('roundrect_rratio', 0.25)
        r_corner = min(w, h) / 2 * rratio
        _draw_rounded_rect(ctx, -w/2, -h/2, w, h, r_corner)
        ctx.fill()

    elif shape == 'oval':
        _draw_oval(ctx, w, h)
        ctx.fill()

    elif shape == 'trapezoid':
        # delta parametresi: (delta_x, delta_y)
        delta = pad.get('rect_delta', [0, 0])
        dx, dy = delta[0]*scale/2, delta[1]*scale/2
        # köşe koordinatları
        pts = [(-w/2+dy, -h/2+dx), (w/2-dy, -h/2-dx),
               ( w/2+dy,  h/2-dx), (-w/2-dy,  h/2+dx)]
        ctx.beginPath()
        ctx.moveTo(*pts[0])
        for p in pts[1:]: ctx.lineTo(*p)
        ctx.closePath(); ctx.fill()

    elif shape == 'custom':
        # primitives token'ı kullan
        for prim in pad.get('primitives', []):
            render_pcb_shape(ctx, prim, layer_color, 1.0)  # scale=1, zaten dönüştürülmüş
            # TODO: Bu koordinatlar pad local'inde — transform gerekebilir

    # Drill deliği (plated veya non-plated)
    drill = pad.get('drill')
    if drill:
        _draw_drill(ctx, drill, pad.get('type',''), scale)

    ctx.restore()


def _draw_oval(ctx, w, h):
    """Oval pad: iki yarım daire + iki düz kenar."""
    if w >= h:
        r = h / 2
        ctx.beginPath()
        ctx.arc(-(w/2-r), 0, r,  math.pi/2, 3*math.pi/2)
        ctx.lineTo( w/2-r, -r)
        ctx.arc( w/2-r,  0, r, -math.pi/2,  math.pi/2)
        ctx.closePath()
    else:
        r = w / 2
        ctx.beginPath()
        ctx.arc(0, -(h/2-r), r, math.pi, 0)
        ctx.lineTo( r,  h/2-r)
        ctx.arc(0,  h/2-r,  r, 0, math.pi)
        ctx.closePath()


def _draw_rounded_rect(ctx, x, y, w, h, r):
    """Köşe yarıçaplı dikdörtgen."""
    r = min(r, w/2, h/2)
    ctx.beginPath()
    ctx.moveTo(x+r, y)
    ctx.lineTo(x+w-r, y)
    ctx.arc(x+w-r, y+r,   r, -math.pi/2, 0)
    ctx.lineTo(x+w, y+h-r)
    ctx.arc(x+w-r, y+h-r, r,  0, math.pi/2)
    ctx.lineTo(x+r, y+h)
    ctx.arc(x+r,   y+h-r, r,  math.pi/2, math.pi)
    ctx.lineTo(x,   y+r)
    ctx.arc(x+r,   y+r,   r,  math.pi, 3*math.pi/2)
    ctx.closePath()


def _draw_drill(ctx, drill, pad_type, scale):
    """Drill deliği — plated veya non-plated."""
    if isinstance(drill, dict):
        diameter = drill.get('diameter', drill.get('size', 0))
        oval     = drill.get('oval', False)
        offset   = drill.get('offset', [0, 0])
    else:
        diameter = drill; oval = False; offset = [0,0]

    dr = diameter / 2 * scale
    ox, oy = offset[0]*scale, offset[1]*scale

    color = PLATED_HOLE_COLOR if pad_type in ('thru_hole','np_thru_hole') else PLATED_HOLE_COLOR
    if pad_type == 'np_thru_hole':
        color = NPTHOLE_COLOR

    ctx.beginPath()
    ctx.arc(ox, oy, dr, 0, 2*math.pi)
    ctx.fillStyle = color
    ctx.fill()
```

---

## ZONE / copper pour — PCB_PAINTER::draw(ZONE*, layer)

`ZONE` C++ sınıfı `m_FilledPolysList` ile per-layer dolgu saklar.
S-expr'de: `zone → filled_polygon → pts`.

```python
def render_zone(ctx, zone, layer_color, scale):
    """
    zone token: layer, filled_polygon veya polygon (outline)
    PCB_PAINTER: ZONE_DISPLAY_MODE_FILLED (default) → filled_polygon kullan
    """
    filled_list = zone.get('filled_polygons', [])

    if filled_list:
        # filled_polygon token'larını çiz
        for filled in filled_list:
            if filled.get('layer') != zone.get('layer'):
                continue   # sadece hedef layer
            pts = filled['pts']
            ctx.beginPath()
            ctx.moveTo(pts[0][0]*scale, pts[0][1]*scale)
            for p in pts[1:]:
                ctx.lineTo(p[0]*scale, p[1]*scale)
            ctx.closePath()
            ctx.fillStyle = layer_color
            ctx.fill()
    else:
        # filled_polygon yoksa sadece outline çiz
        pts = zone.get('polygon', {}).get('pts', [])
        if pts:
            ctx.beginPath()
            ctx.moveTo(pts[0][0]*scale, pts[0][1]*scale)
            for p in pts[1:]:
                ctx.lineTo(p[0]*scale, p[1]*scale)
            ctx.closePath()
            ctx.strokeStyle = layer_color
            ctx.lineWidth   = 0.3
            ctx.stroke()
```

---

## Footprint — draw(FOOTPRINT*, layer)

`FOOTPRINT` sınıfı child items koleksiyonu tutar:
`m_pads` (PAD listesi), `m_drawings` (PCB_SHAPE listesi), `m_fields` (PCB_TEXT).

```python
def render_footprint(ctx, fp, layer_colors, active_layers, scale):
    """
    fp token: at, layer (birincil), footprint (lib_id)
    İçerik: fp_line, fp_arc, fp_circle, fp_text, pad tokenları
    """
    at_x = fp['at'][0]
    at_y = fp['at'][1]
    angle = fp['at'][2] if len(fp['at']) > 2 else 0
    fp_layer = fp.get('layer', 'F.Cu')

    # Footprint koordinat sistemi: at konumu + rotation
    ctx.save()
    ctx.translate(at_x*scale, at_y*scale)
    ctx.rotate(math.radians(angle))

    # Grafik ögeleri (fp_line, fp_arc, fp_circle, fp_rect, fp_poly)
    for drawing in fp.get('graphics', []):
        layer = drawing.get('layer', fp_layer)
        if layer not in active_layers:
            continue
        color = layer_colors.get(layer, '#FFFFFF')
        render_pcb_shape(ctx, drawing, color, scale)

    # Metinler (fp_text: reference, value, user)
    for text in fp.get('texts', []):
        layer = text.get('layer', 'F.SilkS')
        if layer in active_layers and not text.get('hide'):
            color = layer_colors.get(layer, '#FFFFFF')
            render_pcb_text(ctx, text, color, scale)

    ctx.restore()   # ← footprint transform kapatılıyor

    # Pad'ler: GLOBAL koordinatlarda çizilir (footprint transform sonrası hesaplanmış)
    for pad in fp.get('pads', []):
        layer = determine_pad_layer(pad, fp_layer)
        if layer in active_layers:
            color = layer_colors.get(layer, '#FFFFFF')
            render_pad(ctx, apply_footprint_transform(pad, at_x, at_y, angle), color, scale)
```

> ⚠️ **Kritik:** KiCad pad koordinatları footprint local'indedir.
> S-expr parse sonrası `at` değerleri footprint `at` + rotation ile globalleştirilmeli,
> VEYA footprint transform'un içinde çizilmeli.

---

## Kritik tuzaklar (kaynak koddan)

1. **Segment vs Arc track:** S-expr'de `(arc ...)` kopper arc track'tır,
   `(gr_arc ...)` grafik arc'tır. İkisi de aynı `start/mid/end` formatını kullanır.

2. **Via layer filtreleme:** Via `LAYER_VIAS` (GAL virtual layer) üzerinde çizilir,
   `F.Cu` veya `B.Cu` üzerinde değil. Render sırasında via'yı PCB layer değil
   GAL layer olarak ele al.

3. **PAD oval drill:** Oval drill hem `diameter` hem slot width içerir:
   `(drill oval DIAMETER SLOT_WIDTH)`. `oval=True` ise eliptik delik çiz.

4. **PADSTACK:** KiCad 9+ ile pad şekli layer'a göre farklılaşabilir.
   `padstack.mode == 'front_inner_back'` ise `F.Cu`, inner ve `B.Cu` için
   ayrı `size`/`shape` değerleri olabilir. Basit render için normal mod yeterli.

5. **Footprint mirroring (B side):** `(layer B.Cu)` olan footprint'ler
   X ekseninde aynılanmış demektir. `ctx.scale(-1, 1)` uygula. Tüm child item
   layer'ları da `F.*` → `B.*` ve tersine çevrilir.

6. **gr_text angle:** PCB metin açısı derece cinsinden ama yön şematikten farklı.
   PCB Y aşağı pozitif, dolayısıyla `ctx.rotate(math.radians(angle))` direkt çalışır.

7. **Thermal relief:** Zone fill'de pad çevresindeki thermal relief boşlukları
   `filled_polygon` içine zaten dahil edilmiş. Ayrıca hesaplaman gerekmez.
