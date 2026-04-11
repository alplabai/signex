# PCB Layer System

> Source: `include/layer_ids.h`, `pcbnew/pcb_draw_panel_gal.cpp` (GAL_LAYER_ORDER)
> KiCad source code, April 2026

---

## PCB_LAYER_ID enum — physical layers

```python
# include/layer_ids.h values
PCB_LAYER_IDS = {
    # Copper katmanlar
    'F.Cu':    0,    # F_Cu
    'In1.Cu':  1,    # In1_Cu
    'In2.Cu':  2,    # In2_Cu
    # ... In3..In29 range
    'In30.Cu': 31,   # In30_Cu
    'B.Cu':    31,   # B_Cu (same numara not, B_Cu = 31)
    # Not: Actual B_Cu value = PCB_LAYER_IDS["B.Cu"] for downya bak

    # Teknik katmanlar
    'B.Adhes':   32,
    'F.Adhes':   33,
    'B.Paste':   34,
    'F.Paste':   35,
    'B.SilkS':   36,
    'F.SilkS':   37,
    'B.Mask':    38,
    'F.Mask':    39,

    # User layers
    'Dwgs.User': 40,
    'Cmts.User': 41,
    'Eco1.User': 42,
    'Eco2.User': 43,
    'Edge.Cuts': 44,
    'Margin':    45,
    'B.CrtYd':   46,
    'F.CrtYd':   47,
    'B.Fab':     48,
    'F.Fab':     49,

    # User layers (KiCad 7+)
    'User.1':  50, 'User.2':  51, 'User.3':  52, 'User.4':  53,
    'User.5':  54, 'User.6':  55, 'User.7':  56, 'User.8':  57,
    'User.9':  58, 'User.10': 59, 'User.11': 60, 'User.12': 61,
    # ... User.13..User.45
}

# Actual KiCad C++ values (include/layer_ids.h):
# F_Cu = 0, B_Cu = 2 (copper layer count dependent — 2 layer board for B_Cu=1)
# Pratik: string ismi kullan, ID conversionne gerek yok
```

### Canonical layer names (used in S-expr)

```python
# Renderer'da kullan — bu string'ler S-expr'de passer
COPPER_LAYERS = ['F.Cu', 'In1.Cu', 'In2.Cu', 'In3.Cu', 'In4.Cu',
                 'In5.Cu', 'In6.Cu', 'In7.Cu', 'In8.Cu', 'In9.Cu',
                 'In10.Cu','In11.Cu','In12.Cu','In13.Cu','In14.Cu',
                 'In15.Cu','In16.Cu','In17.Cu','In18.Cu','In19.Cu',
                 'In20.Cu','In21.Cu','In22.Cu','In23.Cu','In24.Cu',
                 'In25.Cu','In26.Cu','In27.Cu','In28.Cu','In29.Cu',
                 'In30.Cu','B.Cu']

TECHNICAL_LAYERS = [
    'B.Adhes', 'F.Adhes',
    'B.Paste', 'F.Paste',
    'B.SilkS', 'F.SilkS',
    'B.Mask',  'F.Mask',
    'Dwgs.User', 'Cmts.User', 'Eco1.User', 'Eco2.User',
    'Edge.Cuts', 'Margin',
    'B.CrtYd', 'F.CrtYd',
    'B.Fab',   'F.Fab',
]

def is_copper_layer(layer_name):
    return layer_name in COPPER_LAYERS or layer_name.endswith('.Cu')

def is_front_layer(layer_name):
    return layer_name.startswith('F.')

def is_back_layer(layer_name):
    return layer_name.startswith('B.')
```

---

## GAL Virtual Layers

GAL layer'lar fiziksel PCB layers not, render z-order for virtual layers for render z-order.
`pcbnew/pcb_draw_panel_gal.cpp` `GAL_LAYER_ORDER` listesini defines.

```python
# include/layer_ids.h — GAL_LAYER_ID enum (summary)
GAL_LAYERS = {
    'LAYER_VIAS':            230,   # via'lar
    'LAYER_VIA_HOLES':       231,
    'LAYER_VIA_HOLEWALLS':   232,
    'LAYER_VIA_NETNAMES':    233,
    'LAYER_PADS':            234,   # pad'ler (genel)
    'LAYER_PAD_PLATEDHOLES': 235,
    'LAYER_NON_PLATEDHOLES': 236,
    'LAYER_PAD_NETNAMES':    237,
    'LAYER_TRACKS':          238,   # track'lar (genel)
    'LAYER_RATSNEST':        239,
    'LAYER_GRID':            240,
    'LAYER_GRID_AXES':       241,
    'LAYER_DRAWINGSHEET':    246,
    'LAYER_PCB_BACKGROUND':  247,
    'LAYER_CURSOR':          248,
    'LAYER_DRC_ERROR':       249,
    'LAYER_DRC_WARNING':     250,
    'LAYER_SELECT_OVERLAY':  253,
    'LAYER_SELECTION_SHADOWS':254,
    'LAYER_CONFLICTS_SHADOW':255,
}
```

---

## Render Z-order (bottom → top)

`pcbnew/pcb_draw_panel_gal.cpp::GAL_LAYER_ORDER` listesinden derived.
Daha small z-order = altta are drawn.

```python
# Layer render order (alt → top)
# S-expr renderer for bunu takip et:

PCB_RENDER_ORDER = [
    # 1. PCB background
    'LAYER_PCB_BACKGROUND',

    # 2. Copper pour (zone filled_polygon) — alt copper first
    'B.Cu_zone', 'In30.Cu_zone', '...', 'In1.Cu_zone', 'F.Cu_zone',

    # 3. Copper track'lar + arc'lar (segment, arc) — inner → outer
    'B.Cu', 'In30.Cu', '...', 'In1.Cu', 'F.Cu',

    # 4. Teknik katmanlar
    'B.Mask',   'F.Mask',
    'B.Paste',  'F.Paste',
    'B.SilkS',  'F.SilkS',
    'B.Adhes',  'F.Adhes',
    'B.Fab',    'F.Fab',

    # 5. Courtyard + design
    'B.CrtYd',  'F.CrtYd',
    'Dwgs.User','Cmts.User','Eco1.User','Eco2.User',

    # 6. Kart edge
    'Edge.Cuts', 'Margin',

    # 7. Vias (should appear above copper)
    'LAYER_VIAS',
    'LAYER_VIA_HOLEWALLS',
    'LAYER_VIA_HOLES',

    # 8. Pad'ler
    'LAYER_PADS',
    'LAYER_PAD_PLATEDHOLES',
    'LAYER_NON_PLATEDHOLES',

    # 9. Ratsnest + netname (debug/info)
    'LAYER_RATSNEST',
    'LAYER_VIA_NETNAMES',
    'LAYER_PAD_NETNAMES',

    # 10. Drawing sheet (title block)
    'LAYER_DRAWINGSHEET',

    # 11. DRC + selection efektleri (en top)
    'LAYER_DRC_ERROR', 'LAYER_DRC_WARNING',
    'LAYER_SELECT_OVERLAY',
    'LAYER_SELECTION_SHADOWS',
]
```

---

## Layer visibility management

Minimal renderer for hangi layersn visible  kontrol et:

```python
# Preview for default visible layer'lar
DEFAULT_VISIBLE_LAYERS = {
    'F.Cu', 'B.Cu',
    'F.SilkS', 'B.SilkS',
    'F.Mask', 'B.Mask',
    'F.Fab', 'B.Fab',
    'F.CrtYd', 'B.CrtYd',
    'Edge.Cuts',
    'Dwgs.User',
}

# Basit PCB preview (thumbnail) for minimum set:
THUMBNAIL_LAYERS = {'F.Cu', 'B.Cu', 'F.SilkS', 'Edge.Cuts'}

def render_board(ctx, board, scale,
                 visible_layers=DEFAULT_VISIBLE_LAYERS,
                 active_layer='F.Cu'):
    """
    board: parse flattened .kicad_pcb model
    Render order: zone → track/arc → via → footprint → text
    """
    layer_colors = load_layer_colors()   # pcb-colors.md'den

    # Background
    ctx.fillStyle = PCB_BACKGROUND
    ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height)

    # 1. Zone'lar (copper pour) — her layer for
    for layer in [l for l in PCB_RENDER_ORDER if l in visible_layers]:
        for zone in board.get('zones', []):
            if zone.get('layer') == layer:
                render_zone(ctx, zone, layer_colors[layer], scale)

    # 2. Graphic elements (gr_*)
    for item in board.get('graphics', []):
        layer = item.get('layer', '')
        if layer in visible_layers:
            render_pcb_shape(ctx, item, layer_colors.get(layer,'#FFF'), scale)

    # 3. Track'lar + arc'lar
    for seg in board.get('segments', []):
        layer = seg.get('layer', 'F.Cu')
        if layer in visible_layers:
            render_segment(ctx, seg, layer_colors.get(layer,'#FFF'), scale)
    for arc in board.get('arcs', []):
        layer = arc.get('layer', 'F.Cu')
        if layer in visible_layers:
            render_arc_track(ctx, arc, layer_colors.get(layer,'#FFF'), scale)

    # 4. Via'lar
    for via in board.get('vias', []):
        render_via(ctx, via, layer_colors, scale)

    # 5. Footprint'ler
    for fp in board.get('footprints', []):
        render_footprint(ctx, fp, layer_colors, visible_layers, scale)
```

---

## Layer mask (LSET) — wildcard usage

S-expr `pad` tokennda `layers "*.Cu"` gibi wildcard can be:

```python
def expand_layer_mask(mask, board_copper_count=2):
    """
    "*.Cu"      → all copper layer'lar
    "F&B.Cu"    → F.Cu + B.Cu
    "F.Cu B.Cu" → F.Cu + B.Cu
    """
    if mask == '*.Cu':
        layers = ['F.Cu', 'B.Cu']
        if board_copper_count > 2:
            layers += [f'In{i}.Cu' for i in range(1, board_copper_count-1)]
        return set(layers)
    elif mask == 'F&B.Cu':
        return {'F.Cu', 'B.Cu'}
    else:
        return set(mask.split())
```
