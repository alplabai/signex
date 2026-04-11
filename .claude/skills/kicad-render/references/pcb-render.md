# PCB Render — Primitives, Footprint, PADSTACK

> Source: `pcbnew/pcb_painter.cpp`, `pcbnew/pad.cpp`, `pcbnew/pcb_track.cpp`,
> `pcbnew/zone.cpp`, `common/eda_shape.cpp` — KiCad source code, April 2026

---

## PCB_PAINTER::Draw() dispatcher

KiCad `pcb_painter.cpp::Draw()` dispatches by item type.
Render methods for each item type:

| C++ class | Token / type | Draw method |
|-----------|------------|------------|
| `PCB_TRACK` | `segment` | `draw(PCB_TRACK*, layer)` — `DrawSegment()` |
| `PCB_ARC` | `arc` (track) | `draw(PCB_ARC*, layer)` — calculate arc + DrawArc |
| `PCB_VIA` | `via` | `draw(PCB_VIA*, layer)` — via + hole |
| `FOOTPRINT` | `footprint` | `draw(FOOTPRINT*, layer)` — each child item |
| `PAD` | `pad` | `draw(PAD*, layer)` — PADSTACK'e per shape |
| `PCB_SHAPE` | `gr_*`, `fp_*` | `draw(PCB_SHAPE*, layer)` — EDA_SHAPE dispatch |
| `ZONE` | `zone` | `draw(ZONE*, layer)` — filled_polygon |
| `PCB_TEXT` | `gr_text`, `fp_text` | `draw(PCB_TEXT*, layer)` |
| `PCB_TEXTBOX` | `fp_text_box` | `draw(PCB_TEXTBOX*, layer)` |
| `PCB_DIMENSION_*` | `dimension` | `draw(PCB_DIMENSION*, layer)` |

---

## Coordinate system

**PCB: Y axis DOWN positive** — same as Canvas Y, no flipping needed.

```python
def pcb_to_px(mm_x, mm_y, scale, origin_x, origin_y):
    return (mm_x - origin_x) * scale, (mm_y - origin_y) * scale

def bounding_box(items):
    """All item'lardan bounding box hesapla."""
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

`PCB_TRACK` C++ class default 0.2mm width uses.
Canvas'ta `round` lineCap ile are drawn (KiCad GAL DrawSegment).

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
    ctx.lineWidth   = max(w, 0.5)   # minimum 0.5px visibility
    ctx.lineCap     = 'round'
    ctx.lineJoin    = 'round'
    ctx.stroke()
```

---

## PCB_ARC (arc track) — PCB_PAINTER::draw(PCB_ARC*)

Arc track: `(arc (start X Y)(mid X Y)(end X Y)(width W)(layer L))`.
KiCad GAL `DrawArc()` center+r+angle uses.

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
        # Fallback: straight line
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

Via types: `through`, `blind_buried`, `micro`.
KiCad `draw(PCB_VIA*)` draws two circles: outer ring (copper) + inner hole.

Special layer'lar: `LAYER_VIA_HOLES` (hole color), `LAYER_VIA_HOLEWALLS` (wall color).

```python
# Default tema colorleri (builtin_color_themes.h'dan):
VIA_HOLE_COLOR    = 'rgb(227,183,46,1)'   # LAYER_VIA_HOLES
VIA_HOLEWALLS     = 'rgb(236,236,236,1)'  # LAYER_VIA_HOLEWALLS
PLATED_HOLE_COLOR = 'rgb(194,194,0,1)'    # LAYER_PAD_PLATEDHOLES
NPTHOLE_COLOR     = 'rgb(26,196,210,1)'   # LAYER_NON_PLATEDHOLES

def render_via(ctx, via, layer_colors, scale):
    """
    via token: at, size (outer diameter), drill (hole diameter),
               layers ["F.Cu","B.Cu"] or via_type
    """
    x, y    = via['at'][0]*scale, via['at'][1]*scale
    size    = via.get('size', 1.6)
    drill   = via.get('drill', 0.8)

    outer_r  = size  / 2 * scale
    drill_r  = drill / 2 * scale
    wall_r   = drill_r + 0.1 * scale   # holewalls thin ring

    # 1. Copper ring (active layer color or via color)
    via_layer = via.get('layers', ['F.Cu', 'B.Cu'])
    color = layer_colors.get(via_layer[0], VIA_HOLE_COLOR)
    ctx.beginPath()
    ctx.arc(x, y, outer_r, 0, 2*math.pi)
    ctx.fillStyle = color
    ctx.fill()

    # 2. Holewalls ring (thin, via_holewalls rengi)
    ctx.beginPath()
    ctx.arc(x, y, wall_r, 0, 2*math.pi)
    ctx.strokeStyle = VIA_HOLEWALLS
    ctx.lineWidth   = (outer_r - drill_r) * 0.3
    ctx.stroke()

    # 3. Hole
    ctx.beginPath()
    ctx.arc(x, y, drill_r, 0, 2*math.pi)
    ctx.fillStyle = VIA_HOLE_COLOR
    ctx.fill()
```

---

## PCB_SHAPE (EDA_SHAPE) — gr_* and fp_* graphics

`EDA_SHAPE` class `SHAPE_T` enum stores type.
`pcb_painter.cpp::draw(PCB_SHAPE*)` → `getLineWidth()` + `getFillColor()` + GAL call.

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
        pts = shape['pts']  # 4 control points
        p0,p1,p2,p3 = [(p[0]*scale,p[1]*scale) for p in pts]
        ctx.beginPath()
        ctx.moveTo(*p0)
        ctx.bezierCurveTo(p1[0],p1[1], p2[0],p2[1], p3[0],p3[1])
        ctx.stroke()


def get_stroke_width(shape):
    """stroke.width or width token get width."""
    stroke = shape.get('stroke', {})
    w = stroke.get('width', shape.get('width', 0.1524))
    return max(w, 0.0)   # cannot be negative
```

---

## PAD — PCB_PAINTER::draw(PAD*, layer)

### PADSTACK architecture (KiCad 9+)

`PAD` → `PADSTACK` → `COPPER_LAYER_PROPS` per-layer geometry.
`PADSTACK::Mode`: `NORMAL` (single shape), `FRONT_INNER_BACK` (3 shapes), `CUSTOM` (each layer separate).

For simple rendering, just use `shape` + `size` + `angle` + `drill` kullan.

### Pad shapes

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
        # delta parameter: (delta_x, delta_y)
        delta = pad.get('rect_delta', [0, 0])
        dx, dy = delta[0]*scale/2, delta[1]*scale/2
        # corner coordinates
        pts = [(-w/2+dy, -h/2+dx), (w/2-dy, -h/2-dx),
               ( w/2+dy,  h/2-dx), (-w/2-dy,  h/2+dx)]
        ctx.beginPath()
        ctx.moveTo(*pts[0])
        for p in pts[1:]: ctx.lineTo(*p)
        ctx.closePath(); ctx.fill()

    elif shape == 'custom':
        # primitives token kullan
        for prim in pad.get('primitives', []):
            render_pcb_shape(ctx, prim, layer_color, 1.0)  # scale=1, already transformed
            # TODO: Bu koordinatlar pad local'inde — transform may be needed

    # Drill hole (plated or non-plated)
    drill = pad.get('drill')
    if drill:
        _draw_drill(ctx, drill, pad.get('type',''), scale)

    ctx.restore()


def _draw_oval(ctx, w, h):
    """Oval pad: two semicircles + two straight edges."""
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
    """Rectangle with corner radius."""
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
    """Drill hole — plated or non-plated."""
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

`ZONE` C++ class `m_FilledPolysList` ile stores per-layer fill.
S-expr'de: `zone → filled_polygon → pts`.

```python
def render_zone(ctx, zone, layer_color, scale):
    """
    zone token: layer, filled_polygon or polygon (outline)
    PCB_PAINTER: ZONE_DISPLAY_MODE_FILLED (default) → filled_polygon kullan
    """
    filled_list = zone.get('filled_polygons', [])

    if filled_list:
        # filled_polygon draw tokens
        for filled in filled_list:
            if filled.get('layer') != zone.get('layer'):
                continue   # target layer only
            pts = filled['pts']
            ctx.beginPath()
            ctx.moveTo(pts[0][0]*scale, pts[0][1]*scale)
            for p in pts[1:]:
                ctx.lineTo(p[0]*scale, p[1]*scale)
            ctx.closePath()
            ctx.fillStyle = layer_color
            ctx.fill()
    else:
        # no filled_polygon, draw outline only
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

`FOOTPRINT` class holds a collection of child items:
`m_pads` (PAD list), `m_drawings` (PCB_SHAPE list), `m_fields` (PCB_TEXT).

```python
def render_footprint(ctx, fp, layer_colors, active_layers, scale):
    """
    fp token: at, layer (primary), footprint (lib_id)
    Contents: fp_line, fp_arc, fp_circle, fp_text, pad tokens
    """
    at_x = fp['at'][0]
    at_y = fp['at'][1]
    angle = fp['at'][2] if len(fp['at']) > 2 else 0
    fp_layer = fp.get('layer', 'F.Cu')

    # Footprint coordinate system: at position + rotation
    ctx.save()
    ctx.translate(at_x*scale, at_y*scale)
    ctx.rotate(math.radians(angle))

    # Graphic elements (fp_line, fp_arc, fp_circle, fp_rect, fp_poly)
    for drawing in fp.get('graphics', []):
        layer = drawing.get('layer', fp_layer)
        if layer not in active_layers:
            continue
        color = layer_colors.get(layer, '#FFFFFF')
        render_pcb_shape(ctx, drawing, color, scale)

    # Textler (fp_text: reference, value, user)
    for text in fp.get('texts', []):
        layer = text.get('layer', 'F.SilkS')
        if layer in active_layers and not text.get('hide'):
            color = layer_colors.get(layer, '#FFFFFF')
            render_pcb_text(ctx, text, color, scale)

    ctx.restore()   # ← closing footprint transform

    # Pad'ler: drawn in GLOBAL coordinates (calculated after footprint transform)
    for pad in fp.get('pads', []):
        layer = determine_pad_layer(pad, fp_layer)
        if layer in active_layers:
            color = layer_colors.get(layer, '#FFFFFF')
            render_pad(ctx, apply_footprint_transform(pad, at_x, at_y, angle), color, scale)
```

> ⚠️ **Critical:** KiCad pad coordinates are in footprint local space'.
> S-expr parse after `at` values must be globalized with footprint `at` + rotation,
> VEYA footprint transform'un drawn inside the transform.

---

## Critical pitfalls (from source code)

1. **Segment vs Arc track:** S-expr'de `(arc ...)` kopper arc is a copper arc track,
   `(gr_arc ...)` grafik is a graphic arc. Both use the de same `start/mid/end` format. uses.

2. **Via layer filterme:** Via `LAYER_VIAS` (GAL virtual layer) on are drawn,
   `F.Cu` or `B.Cu` on not. Render ordernda vias as PCB layer not
   GAL layer olarak ele al.

3. **PAD oval drill:** Oval drill hem `diameter` hem slot width contains:
   `(drill oval DIAMETER SLOT_WIDTH)`. `oval=True` ise eliptik delik draw.

4. **PADSTACK:** KiCad 9+ ile pad shape layer'a per vary.
   `padstack.mode == 'front_inner_back'` ise `F.Cu`, inner ve `B.Cu` for
   separate `size`/`shape` values can be. Basit render for normal mod sufficient.

5. **Footprint mirroring (B side):** `(layer B.Cu)` olan footprint'ler
   X ekseninde mirrored demektir. `ctx.scale(-1, 1)` apply. All child item
   layers da `F.*` → `B.*` ve tersine flipped.

6. **gr_text angle:** PCB metin angle derece cinsinden ama direction schematicten different.
   PCB Y down positive, therefore `ctx.rotate(math.radians(angle))` directly works.

7. **Thermal relief:** Zone fill'de pad around thermal relief gaps
   `filled_polygon` in already dahil included. Additionally, hesaplaman gerekmez.
