# PCB Color System

> Source: `common/settings/builtin_color_themes.h` — KiCad source code, April 2026
> Taken directly from the `s_defaultTheme` ve `s_classicTheme` map'lerinden maps.

---

## Default theme (modern dark — CSS colors)

```python
# builtin_color_themes.h::s_defaultTheme — CSS_COLOR(r,g,b,a) format

PCB_THEME_DEFAULT = {
    # Background
    'LAYER_PCB_BACKGROUND': 'rgba(0,16,35,1)',

    # Copper katmanlar
    'F.Cu':    'rgba(200,52,52,1)',
    'In1.Cu':  'rgba(127,200,127,1)',
    'In2.Cu':  'rgba(206,125,44,1)',
    'In3.Cu':  'rgba(79,203,203,1)',
    'In4.Cu':  'rgba(219,98,139,1)',
    'In5.Cu':  'rgba(167,165,198,1)',
    'In6.Cu':  'rgba(40,204,217,1)',
    'In7.Cu':  'rgba(232,178,167,1)',
    'In8.Cu':  'rgba(242,237,161,1)',
    'In9.Cu':  'rgba(141,203,129,1)',
    'In10.Cu': 'rgba(237,124,51,1)',
    'In11.Cu': 'rgba(91,195,235,1)',
    'In12.Cu': 'rgba(247,111,142,1)',
    'In13.Cu': 'rgba(167,165,198,1)',
    'In14.Cu': 'rgba(40,204,217,1)',
    'In15.Cu': 'rgba(232,178,167,1)',
    'In16.Cu': 'rgba(242,237,161,1)',
    'In17.Cu': 'rgba(237,124,51,1)',
    'In18.Cu': 'rgba(91,195,235,1)',
    'In19.Cu': 'rgba(247,111,142,1)',
    'In20.Cu': 'rgba(167,165,198,1)',
    # In21..In30: cyclic, same 7-color seti
    'B.Cu':    'rgba(77,127,196,1)',

    # Teknik katmanlar
    'B.Adhes': 'rgba(0,0,132,1)',
    'F.Adhes': 'rgba(132,0,132,1)',
    'B.Paste': 'rgba(0,194,194,0.9)',
    'F.Paste': 'rgba(180,160,154,0.9)',
    'B.SilkS': 'rgba(232,178,167,1)',
    'F.SilkS': 'rgba(242,237,161,1)',
    'B.Mask':  'rgba(2,255,238,0.4)',
    'F.Mask':  'rgba(216,100,255,0.4)',

    # User layers
    'Dwgs.User': 'rgba(194,194,194,1)',
    'Cmts.User': 'rgba(89,148,220,1)',
    'Eco1.User': 'rgba(180,219,210,1)',
    'Eco2.User': 'rgba(216,200,82,1)',
    'Edge.Cuts': 'rgba(208,210,205,1)',
    'Margin':    'rgba(255,38,226,1)',
    'B.CrtYd':   'rgba(38,233,255,1)',
    'F.CrtYd':   'rgba(255,38,226,1)',
    'B.Fab':     'rgba(88,93,132,1)',
    'F.Fab':     'rgba(175,175,175,1)',

    # GAL virtual layer'lar
    'LAYER_VIA_HOLES':       'rgba(227,183,46,1)',
    'LAYER_VIA_HOLEWALLS':   'rgba(236,236,236,1)',
    'LAYER_PAD_PLATEDHOLES': 'rgba(194,194,0,1)',
    'LAYER_NON_PLATEDHOLES': 'rgba(26,196,210,1)',
    'LAYER_RATSNEST':        'rgba(0,248,255,0.35)',
    'LAYER_DRC_ERROR':       'rgba(215,91,107,0.8)',
    'LAYER_DRC_WARNING':     'rgba(255,208,66,0.8)',
    'LAYER_SELECT_OVERLAY':  'rgba(4,255,67,1)',
    'LAYER_DRAWINGSHEET':    'rgba(200,114,171,1)',
    'LAYER_GRID':            'rgba(132,132,132,1)',
    'LAYER_CURSOR':          'rgba(255,255,255,1)',
}

# Background rengi short yol
PCB_BACKGROUND = PCB_THEME_DEFAULT['LAYER_PCB_BACKGROUND']
```

---

## Classic theme (white background)

```python
# builtin_color_themes.h::s_classicTheme

PCB_THEME_CLASSIC = {
    'LAYER_PCB_BACKGROUND': 'rgba(0,0,0,1)',    # siyah

    'F.Cu':    'rgba(255,0,0,1)',
    'In1.Cu':  'rgba(255,255,0,1)',
    'In2.Cu':  'rgba(255,0,255,1)',     # lightmagenta
    'In3.Cu':  'rgba(255,128,128,1)',   # lightred
    'In4.Cu':  'rgba(0,255,255,1)',     # cyan
    'In5.Cu':  'rgba(0,255,0,1)',       # green
    'In6.Cu':  'rgba(0,0,255,1)',       # blue
    'In7.Cu':  'rgba(64,64,64,1)',      # darkgray
    'B.Cu':    'rgba(0,255,0,1)',       # green

    'B.SilkS': 'rgba(255,0,255,1)',
    'F.SilkS': 'rgba(0,255,255,1)',
    'B.Mask':  'rgba(165,42,42,1)',     # brown
    'F.Mask':  'rgba(255,0,255,1)',
    'Edge.Cuts':'rgba(255,255,0,1)',
    'F.CrtYd': 'rgba(192,192,192,1)',
    'B.CrtYd': 'rgba(64,64,64,1)',
    'F.Fab':   'rgba(64,64,64,1)',
    'B.Fab':   'rgba(0,0,255,1)',

    'LAYER_VIA_HOLES':       'rgba(128,102,0,0.8)',
    'LAYER_VIA_HOLEWALLS':   'rgba(255,255,255,1)',
    'LAYER_PAD_PLATEDHOLES': 'rgba(255,255,0,1)',
    'LAYER_NON_PLATEDHOLES': 'rgba(255,255,0,1)',
    'LAYER_PCB_BACKGROUND':  'rgba(0,0,0,1)',
}
```

---

## PCB_RENDER_SETTINGS::GetColor() — color priority hierarchy

```python
def get_pcb_render_color(item, layer, settings, is_selected=False, is_highlighted=False):
    """
    PCB_PAINTER::PCB_RENDER_SETTINGS::GetColor() logic:
    1. Highlighted net → HIGHLIGHT rengi
    2. High contrast mod (aktif olmayan layer dim)
    3. Net-specific color override
    4. Layer rengi
    """
    base_color = settings['layer_colors'].get(layer, '#FFFFFF')

    # Net rengi override
    net_name = item.get('net_name')
    if net_name and net_name in settings.get('net_colors', {}):
        base_color = settings['net_colors'][net_name]

    # High contrast: aktif layer if not dim
    if settings.get('high_contrast'):
        active = settings.get('active_layer')
        if layer != active and not is_highlighted:
            base_color = dim_color(base_color, settings.get('hi_contrast_factor', 0.3))

    # Selection
    if is_selected:
        base_color = mix_color(base_color, settings.get('selection_color', '#4AB8FF'), 0.4)

    return base_color


def dim_color(hex_color, factor=0.3):
    """Rengi karart (high contrast mod)."""
    rgba = parse_rgba(hex_color)
    return f"rgba({int(rgba[0]*factor)},{int(rgba[1]*factor)},{int(rgba[2]*factor)},{rgba[3]})"


def mix_color(base, overlay, alpha):
    """Blend two colors."""
    b = parse_rgba(base); o = parse_rgba(overlay)
    r = int(b[0]*(1-alpha) + o[0]*alpha)
    g = int(b[1]*(1-alpha) + o[1]*alpha)
    bl= int(b[2]*(1-alpha) + o[2]*alpha)
    return f"rgba({r},{g},{bl},{b[3]})"


def parse_rgba(color_str):
    """rgba(r,g,b,a) or #RRGGBB string → (r,g,b,a) tuple."""
    import re
    if color_str.startswith('rgba'):
        m = re.match(r'rgba\((\d+),(\d+),(\d+),([\d.]+)\)', color_str)
        if m:
            return int(m[1]), int(m[2]), int(m[3]), float(m[4])
    elif color_str.startswith('#'):
        h = color_str.lstrip('#')
        return int(h[0:2],16), int(h[2:4],16), int(h[4:6],16), 1.0
    return 255, 255, 255, 1.0


def load_layer_colors(theme='default'):
    """Tema selectionine per layer colorleri returns."""
    if theme == 'classic':
        colors = dict(PCB_THEME_CLASSIC)
    else:
        colors = dict(PCB_THEME_DEFAULT)
    # Eksik layer'lar for fallback
    for layer in COPPER_LAYERS:
        if layer not in colors:
            idx = COPPER_LAYERS.index(layer) % 7
            colors[layer] = COPPER_LOOPING_COLORS[idx]
    return colors


# builtin_color_themes.h::s_copperColors (cyclic inner copper colorleri)
COPPER_LOOPING_COLORS = [
    'rgba(237,124,51,1)',
    'rgba(91,195,235,1)',
    'rgba(247,111,142,1)',
    'rgba(167,165,198,1)',
    'rgba(40,204,217,1)',
    'rgba(232,178,167,1)',
    'rgba(242,237,161,1)',
]
```

---

## Opacity (transparency) settings

`PCB_RENDER_SETTINGS` opacity values:

```python
# Default opacity values
TRACK_OPACITY = 1.0
VIA_OPACITY   = 1.0
PAD_OPACITY   = 1.0
ZONE_OPACITY  = 0.6    # Zone fill biraz transparent visible daha iyi
SILKSCREEN_OPACITY = 1.0

# Mask layers: genellikle alpha < 1 already builtin_color_themes.h'da
# F.Mask: rgba(216,100,255,0.4) — %40 opak
# B.Mask: rgba(2,255,238,0.4)   — %40 opak
```

---

## Theme JSON format (user themes)

KiCad `~/.config/kicad/9.0/colors/*.json` tema file format:

```json
{
  "meta": { "name": "My Theme", "version": 5 },
  "pcb": {
    "background": "rgba(0, 16, 35, 1)",
    "F_Cu":    "rgba(200, 52, 52, 1)",
    "B_Cu":    "rgba(77, 127, 196, 1)",
    "F_SilkS": "rgba(242, 237, 161, 1)",
    "B_SilkS": "rgba(232, 178, 167, 1)",
    "F_Mask":  "rgba(216, 100, 255, 0.4)",
    "B_Mask":  "rgba(2, 255, 238, 0.4)",
    "Edge_Cuts": "rgba(208, 210, 205, 1)",
    "via_holes":       "rgba(227, 183, 46, 1)",
    "via_holewalls":   "rgba(236, 236, 236, 1)",
    "pad_plated_holes":"rgba(194, 194, 0, 1)",
    "non_plated_holes":"rgba(26, 196, 210, 1)"
  }
}
```

> **Not:** Tema JSON'daki key'ler nokta yerine alt line uses: `F_Cu` not `F.Cu`.
> Parse during `layer_name.replace('.','_')` with convert.

```python
def parse_theme_json(json_data):
    """KiCad tema JSON'unu layer_colors dict'e convert."""
    pcb = json_data.get('pcb', {})
    colors = {}
    for key, value in pcb.items():
        # Alt line → nokta: F_Cu → F.Cu
        layer = key.replace('_', '.', 1)
        # Special GAL layer'lar for mapping
        GAL_MAP = {
            'via.holes':        'LAYER_VIA_HOLES',
            'via.holewalls':    'LAYER_VIA_HOLEWALLS',
            'pad.plated.holes': 'LAYER_PAD_PLATEDHOLES',
            'non.plated.holes': 'LAYER_NON_PLATEDHOLES',
            'ratsnest':         'LAYER_RATSNEST',
            'grid':             'LAYER_GRID',
            'background':       'LAYER_PCB_BACKGROUND',
        }
        layer = GAL_MAP.get(layer, layer)
        colors[layer] = value
    return colors
```
