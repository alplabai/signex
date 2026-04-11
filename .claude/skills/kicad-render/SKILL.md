---
name: kicad-render
description: >
  KiCad S-expression (.kicad_pcb, .kicad_sch) files to visual output
  Comprehensive render pipeline guide. KiCad kmirrork kodundan (SCH_PAINTER, sch_symbol.cpp,
  sch_label.cpp) real extracted implementasyon bilgisi contains. Schematic render (wire,
  bus, symbol, pin, label, junction, sheet), PCB render (track, pad, via, zone), SVG/Canvas
  output, TRANSFORM matrix calculation, layer compositing, global_label shape geometrisi,
  PIN_LAYOUT_CACHE, LIB_SYMBOL::Flatten(), unit/bodyStyle filterme, koordinat conversion
  gibi topics kapsar. "kicad render", "kicad svg", "kicad preview", "schematic to image",
  "pcb to image", "kicad thumbnail", "kicad canvas", "kicad draw", "kicad visual" gibi
  ifadelerde mutlaka tetiklenmeli. KiCanvas kullanmadan standalone renderer yazmak
  when you want de bu skili kullan.
---

# KiCad S-Expression → Render Guide

> Source: KiCad `eeschema/sch_painter.cpp`, `sch_symbol.cpp`, `sch_label.cpp`,
> `sch_pin.cpp`, `lib_symbol.cpp` — DeepWiki analysis, April 2026

---

## Which reference to read

| Task | Reference |
|-------|---------|
| Schematic — wire/bus/junction/label/sheet drawing | `references/sch-primitives.md` |
| Schematic — symbol + pin + TRANSFORM matrix | `references/sch-symbol.md` |
| Schematic — layer/color system, draw order | `references/sch-colors-layers.md` |
| PCB — track/arc/pad/via/zone/footprint render | `references/pcb-render.md` |
| PCB — PCB_LAYER_ID enum, GAL layers, z-order | `references/pcb-layers.md` |
| PCB — builtin theme colors, color management | `references/pcb-colors.md` |

**Schematic render:** read sch-primitives + sch-symbol + sch-colors-layers.
**Without symbols:** sch-primitives + sch-colors-layers is sufficient.
**PCB render:** read pcb-render + pcb-layers + pcb-colors.
**Quick PCB preview:** pcb-render + pcb-colors is sufficient.

---

## Coordinate conversion (critical difference)

```python
# PCB: Y axis DOWN positive — same as screen
def pcb_to_px(mm_x, mm_y, scale, ox, oy):
    return (mm_x - ox) * scale, (mm_y - oy) * scale

# Schematic: Y axis UP positive — flip!
def sch_to_px(mm_x, mm_y, scale, ox, oy):
    return (mm_x - ox) * scale, -(mm_y - oy) * scale
```

Bounding box all item'lardan is calculated → `scale = canvas_px / bbox_mm`

---

## Arc math (shared by PCB and schematic)

KiCad 6+ all arc'larda `(start)(mid)(end)` verilir — merkez verilmez.

```python
import math

def arc_center_radius(s, m, e):
    """Circumcircle center + radius from 3 points. (x,y) tuple'lar."""
    ax, ay = s[0]-e[0], s[1]-e[1]
    bx, by = m[0]-e[0], m[1]-e[1]
    D = 2 * (ax*(m[1]-e[1]) - ay*(m[0]-e[0]))
    if abs(D) < 1e-10:
        return None, None          # 3 points are collinear
    ux = (ax*ax+ay*ay)*by - (bx*bx+by*by)*ay
    uy = (bx*bx+by*by)*ax - (ax*ax+ay*ay)*bx
    cx, cy = ux/D + e[0], uy/D + e[1]
    return (cx, cy), math.hypot(s[0]-cx, s[1]-cy)

def arc_sweep(center, s, e, m):
    """Canvas arc(cx,cy,r,t1,t2,ccw) for parameters."""
    cx, cy = center
    t1  = math.atan2(s[1]-cy, s[0]-cx)
    t2  = math.atan2(e[1]-cy, e[0]-cx)
    # CW/CCW detection via cross-product with mid point
    cross = (e[0]-s[0])*(m[1]-s[1]) - (e[1]-s[1])*(m[0]-s[0])
    return t1, t2, cross > 0      # True = CCW
```

---

## Minimal render dispatcher

```python
def render_item(ctx, item, lib_syms, scale):
    t = item['type']
    # schematic primitives
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
