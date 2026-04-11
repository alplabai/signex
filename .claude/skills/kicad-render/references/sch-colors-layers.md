# Schematic Layer, Color, and Theme System

> Source: `eeschema/sch_painter.cpp::getRenderColor()`, `getLineWidth()`,
> `sch_label.h::SPIN_STYLE`, KiCad color_settings

---

## Layer constants (LAYER_* enum)

KiCad `sch_painter.cpp::Draw()` dispatcher item tipine per hangi layer'da
to draw belirler.

| Layer | Constant | Content |
|-------|-------|--------|
| `LAYER_WIRE` | 10 | Wire segments |
| `LAYER_BUS` | 11 | Bus segments |
| `LAYER_BUS_JUNCTION` | 12 | Bus junction points |
| `LAYER_JUNCTION` | 13 | Wire junction points |
| `LAYER_NOCONNECT` | 14 | No-connect X marks |
| `LAYER_LOCLABEL` | 16 | Local label text |
| `LAYER_GLOBLABEL` | 17 | Global label shape+metin |
| `LAYER_HIERLABEL` | 18 | Hierarchical label |
| `LAYER_PINNUM` | 20 | Pin number |
| `LAYER_PINNAM` | 21 | Pin name |
| `LAYER_FIELDS` | 25 | Symbol field metinleri |
| `LAYER_DEVICE` | 26 | Symbol body grafikleri |
| `LAYER_DEVICE_BACKGROUND` | 27 | Symbol background dolgusu |
| `LAYER_NOTES` | 30 | Grafik polyline + text |
| `LAYER_RULE_AREAS` | 31 | Rule area frames |
| `LAYER_SHEET` | 40 | Sheet box |
| `LAYER_SHEETNAME` | 41 | Sheet name text |
| `LAYER_SHEETFILENAME` | 42 | Sheet file text |
| `LAYER_SHEETFIELDS` | 43 | Sheet field metinleri |
| `LAYER_SHEETLABEL` | 44 | Sheet pin/label |
| `LAYER_ERC_WARN` | 50 | ERC warning marker |
| `LAYER_ERC_ERR` | 51 | ERC error marker |
| `LAYER_SELECTION_SHADOWS` | 99 | Selection vurgulama |

---

## getRenderColor — color priority hierarchy

KiCad `sch_painter.cpp::getRenderColor()` bu in this order color belirler:

```python
def get_render_color(item, layer, settings):
    """
    KiCad getRenderColor() logic:
    1. DNP (Do Not Populate) → desaturated gray
    2. Highlighted net → highlight rengi
    3. Item override rengi varsa → onu kullan
    4. Net class rengi varsa → onu kullan
    5. Layer default rengi
    """
    # DNP checking
    if item.get('dnp'):
        return '#888888'

    # Item-specific color override
    item_color = item.get('color')
    if item_color and item_color != (0, 0, 0, 0):
        return rgba_to_hex(*item_color)

    # Layer rengi (tema)
    return LAYER_COLORS.get(layer, '#FFFFFF')
```

---

## getLineWidth — line width

```python
def get_line_width(item, is_selected=False):
    """KiCad getLineWidth() logic."""
    w = item.get('stroke', {}).get('width', 0)
    if w == 0:
        # Tip'e per default
        t = item['type']
        if t == 'wire':           w = DEFAULT_WIRE_WIDTH     # 0.0 → tema
        elif t == 'bus':          w = DEFAULT_BUS_WIDTH
        elif t == 'pin':          w = DEFAULT_LINE_WIDTH
        elif t in ('polyline', 'rectangle', 'circle', 'arc'):
            w = DEFAULT_LINE_WIDTH
        else:                     w = DEFAULT_LINE_WIDTH
    if is_selected:
        w = max(w, SELECTION_THICKNESS)   # selectionde thicken when selected
    return w
```

---

## Theme colors — KiCad default (Padrino dark)

```python
# Schematic elements
WIRE_COLOR          = '#15BD6F'
BUS_COLOR           = '#1FB8D3'
JUNCTION_COLOR      = '#15BD6F'
NO_CONNECT_COLOR    = '#EE4040'
LABEL_COLOR         = '#F5F524'
GLOBAL_LABEL_COLOR  = '#A8A800'
HIER_LABEL_COLOR    = '#A8A800'
PIN_COLOR           = '#FF8000'
SYMBOL_COLOR        = '#A8A800'   # LAYER_DEVICE (body)
SYMBOL_BG_COLOR     = '#1A1A28'   # background fill
FIELD_REF_COLOR     = '#4D9CCA'   # Reference
FIELD_VAL_COLOR     = '#A8A800'   # Value
TEXT_COLOR          = '#FFFFFF'
NOTES_COLOR         = '#A0A0A0'
SHEET_COLOR         = '#5E76C5'
SHEET_PIN_COLOR     = '#5E76C5'
SHEET_BG_COLOR      = 'rgba(0,0,0,0)'  # transparent

# Background
SCH_BACKGROUND      = '#1A1A28'

# Default widthler (mm)
DEFAULT_LINE_WIDTH  = 0.1524   # 6 mil
DEFAULT_WIRE_WIDTH  = 0.0      # tema'dan gelir ≈ 0.1524
DEFAULT_BUS_WIDTH   = 0.0      # tema'dan gelir ≈ 0.3
DEFAULT_JUNCTION_RADIUS = 0.5  # mm
SELECTION_THICKNESS = 0.5      # mm

# Net class colorleri (default)
NET_CLASS_COLORS = {
    'Default': WIRE_COLOR,
    'Power':   '#CC0000',
    'HV':      '#FF6600',
    'FastSignal': '#00CC00',
}
```

---

## SPIN_STYLE — label direction system

KiCad `sch_label.h::SPIN_STYLE` labels metin direction belirler.
`at[2]` (angle) → SPIN_STYLE:

```python
# SPIN_STYLE values (KiCad kmirrork)
SPIN_RIGHT  = 0   # 0°   — metin to the right, connection point solda
SPIN_UP     = 1   # 90°  — metin up
SPIN_LEFT   = 2   # 180° — metin sola, connection point on the right
SPIN_DOWN   = 3   # 270° — metin down

def angle_to_spin(angle_deg):
    """at[2] anglendan SPIN_STYLE hesapla."""
    a = int(angle_deg) % 360
    if a == 0:   return SPIN_RIGHT
    if a == 90:  return SPIN_UP
    if a == 180: return SPIN_LEFT
    if a == 270: return SPIN_DOWN
    return SPIN_RIGHT  # fallback

def spin_to_text_anchor(spin):
    """SPIN_STYLE'dan canvas textAlign/rotation returns."""
    return {
        SPIN_RIGHT: {'align': 'left',   'rotation': 0,    'baseline': 'middle'},
        SPIN_UP:    {'align': 'left',   'rotation': -90,  'baseline': 'middle'},
        SPIN_LEFT:  {'align': 'right',  'rotation': 0,    'baseline': 'middle'},
        SPIN_DOWN:  {'align': 'left',   'rotation': 90,   'baseline': 'middle'},
    }[spin]
```

---

## Draw order (z-order)

KiCad's layer z-order from low to high (bottom drawn first):

```
LAYER_DEVICE_BACKGROUND   ← symbol background (fill)
LAYER_NOTES               ← grafik polyline, text
LAYER_WIRE                ← wireler
LAYER_BUS                 ← buslar
LAYER_DEVICE              ← symbol bodyleri
LAYER_SHEET               ← sheet boxes
LAYER_SHEETNAME/FILENAME  ← sheet labels
LAYER_PINNUM              ← pin numbers
LAYER_PINNAM              ← pin names
LAYER_FIELDS              ← reference, value, ...
LAYER_LOCLABEL            ← local labellar
LAYER_GLOBLABEL           ← global labellar
LAYER_HIERLABEL           ← hierarchical labellar
LAYER_SHEETLABEL          ← sheet pin'leri
LAYER_JUNCTION            ← junction points (en top visible on top)
LAYER_NOCONNECT           ← no-connect X'leri
LAYER_ERC_WARN/ERR        ← ERC marker'lar
LAYER_SELECTION_SHADOWS   ← selection highlight (en top)
```

---

## Theme JSON format (KiCad 6+)

KiCad color themes `~/.config/kicad/7.0/colors/` under JSON:

```json
{
  "meta": { "version": 2 },
  "schematic": {
    "background": "#1A1A28",
    "wire": "#15BD6F",
    "bus": "#1FB8D3",
    "junction": "#15BD6F",
    "no_connect": "#EE4040",
    "net_name": "#F5F524",
    "pin": "#FF8000",
    "reference": "#4D9CCA",
    "value": "#A8A800",
    "component_outline": "#A8A800",
    "component_body_background": "#1A1A28",
    "sheet": "#5E76C5",
    "sheet_background": "#00000000",
    "hierarchical_label": "#A8A800",
    "global_label": "#A8A800",
    "note": "#A0A0A0",
    "bus_junction": "#1FB8D3"
  }
}
```

Tema file is parsed, parse edilirse `LAYER_COLORS` dict'ini bu values doldur.

---

## Color manipulation helpers

```python
def rgba_to_hex(r, g, b, a=255):
    """KiCad 0-255 color → hex string."""
    if a < 255:
        return f"rgba({r},{g},{b},{a/255:.2f})"
    return f"#{r:02X}{g:02X}{b:02X}"

def dim_color(hex_color, factor=0.4):
    """Unselected element karartma (KiCad dimmed items)."""
    # hex → rgb → karart → hex
    h = hex_color.lstrip('#')
    r, g, b = int(h[0:2],16), int(h[2:4],16), int(h[4:6],16)
    return f"#{int(r*factor):02X}{int(g*factor):02X}{int(b*factor):02X}"

def highlight_color(hex_color, factor=1.5):
    """Net highlight for brighten."""
    h = hex_color.lstrip('#')
    r, g, b = int(h[0:2],16), int(h[2:4],16), int(h[4:6],16)
    return f"#{min(255,int(r*factor)):02X}{min(255,int(g*factor)):02X}{min(255,int(b*factor)):02X}"
```
