---
name: kicad-render
description: >
  KiCad S-expression (.kicad_pcb, .kicad_sch) dosyalarını görsel çıktıya dönüştürmek için
  kapsamlı render pipeline rehberi. KiCad kaynak kodundan (SCH_PAINTER, sch_symbol.cpp,
  sch_label.cpp) çıkarılmış gerçek implementasyon bilgisi içerir. Şematik render (wire,
  bus, symbol, pin, label, junction, sheet), PCB render (track, pad, via, zone), SVG/Canvas
  çıktısı, TRANSFORM matrix hesabı, layer compositing, global_label şekil geometrisi,
  PIN_LAYOUT_CACHE, LIB_SYMBOL::Flatten(), unit/bodyStyle filtreleme, koordinat dönüşümü
  gibi konuları kapsar. "kicad render", "kicad svg", "kicad preview", "schematic to image",
  "pcb to image", "kicad thumbnail", "kicad canvas", "kicad draw", "kicad görsel" gibi
  ifadelerde mutlaka tetiklenmeli. KiCanvas kullanmadan bağımsız renderer yazmak
  istediğinde de bu skili kullan.
---

# KiCad S-Expression → Render Rehberi

> Kaynak: KiCad `eeschema/sch_painter.cpp`, `sch_symbol.cpp`, `sch_label.cpp`,
> `sch_pin.cpp`, `lib_symbol.cpp` — DeepWiki analizi, Nisan 2026

---

## Hangi referansı oku

| Görev | Referans |
|-------|---------|
| Şematik — wire/bus/junction/label/sheet çizimi | `references/sch-primitives.md` |
| Şematik — sembol + pin + TRANSFORM matrix | `references/sch-symbol.md` |
| Şematik — layer/renk sistemi, çizim sırası | `references/sch-colors-layers.md` |
| PCB — track/arc/pad/via/zone/footprint render | `references/pcb-render.md` |
| PCB — PCB_LAYER_ID enum, GAL layers, z-order | `references/pcb-layers.md` |
| PCB — builtin tema renkleri, renk yönetimi | `references/pcb-colors.md` |

**Şematik render:** sch-primitives + sch-symbol + sch-colors-layers oku.
**Sembol olmadan:** sadece sch-primitives + sch-colors-layers yeterli.
**PCB render:** pcb-render + pcb-layers + pcb-colors oku.
**Hızlı PCB önizleme:** sadece pcb-render + pcb-colors yeterli.

---

## Koordinat dönüşümü (kritik fark)

```python
# PCB: Y ekseni AŞAĞI pozitif — screen ile aynı
def pcb_to_px(mm_x, mm_y, scale, ox, oy):
    return (mm_x - ox) * scale, (mm_y - oy) * scale

# Şematik: Y ekseni YUKARI pozitif — çevir!
def sch_to_px(mm_x, mm_y, scale, ox, oy):
    return (mm_x - ox) * scale, -(mm_y - oy) * scale
```

Bounding box tüm item'lardan hesaplanır → `scale = canvas_px / bbox_mm`

---

## Arc matematik (PCB ve şematik ortakı)

KiCad 6+ tüm arc'larda `(start)(mid)(end)` verilir — merkez verilmez.

```python
import math

def arc_center_radius(s, m, e):
    """3 noktadan circumcircle merkezi + yarıçap. (x,y) tuple'lar."""
    ax, ay = s[0]-e[0], s[1]-e[1]
    bx, by = m[0]-e[0], m[1]-e[1]
    D = 2 * (ax*(m[1]-e[1]) - ay*(m[0]-e[0]))
    if abs(D) < 1e-10:
        return None, None          # 3 nokta kolinear
    ux = (ax*ax+ay*ay)*by - (bx*bx+by*by)*ay
    uy = (bx*bx+by*by)*ax - (ax*ax+ay*ay)*bx
    cx, cy = ux/D + e[0], uy/D + e[1]
    return (cx, cy), math.hypot(s[0]-cx, s[1]-cy)

def arc_sweep(center, s, e, m):
    """Canvas arc(cx,cy,r,t1,t2,ccw) için parametreler."""
    cx, cy = center
    t1  = math.atan2(s[1]-cy, s[0]-cx)
    t2  = math.atan2(e[1]-cy, e[0]-cx)
    # mid noktası cross-product ile CW/CCW tespiti
    cross = (e[0]-s[0])*(m[1]-s[1]) - (e[1]-s[1])*(m[0]-s[0])
    return t1, t2, cross > 0      # True = CCW
```

---

## Minimal render dispatcher

```python
def render_item(ctx, item, lib_syms, scale):
    t = item['type']
    # şematik primitifleri
    dispatch = {
        'wire':              render_wire,
        'bus':               render_bus,
        'bus_entry':         render_bus_entry,
        'junction':          render_junction,
        'no_connect':        render_no_connect,
        'polyline':          render_polyline,
        'text':              render_text,
        'label':             render_label,
        'global_label':      render_global_label,
        'hierarchical_label':render_hier_label,
        'symbol':            lambda c,i,s: render_symbol(c,i,lib_syms,s),
        'sheet':             render_sheet,
        # pcb
        'segment':           render_segment,
        'gr_arc':            render_arc,
        'fp_arc':            render_arc,
        'via':               render_via,
        'pad':               render_pad,
    }
    fn = dispatch.get(t)
    if fn: fn(ctx, item, scale)
```
