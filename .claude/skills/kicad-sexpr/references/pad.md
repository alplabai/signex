# Pad Token — Detailed Reference

## Full Pad Structure

```scheme
(pad "NUMBER"
  PAD_TYPE
  PAD_SHAPE
  (at X Y [ANGLE])
  [(locked)]
  (size WIDTH HEIGHT)
  [(drill DRILL_DEFINITION)]
  (layers "LAYER_LIST")
  [(property PAD_PROPERTY)]
  [(remove_unused_layer)]
  [(keep_end_layers)]
  [(roundrect_rratio 0.0-1.0)]
  [(chamfer_ratio 0.0-1.0)]
  [(chamfer top_left top_right bottom_left bottom_right)]
  [(net NUMBER "NET_NAME")]
  (uuid UUID)
  [(pinfunction "PIN_FUNCTION")]
  [(pintype "PIN_TYPE")]
  [(die_length LENGTH)]
  [(solder_mask_margin MM)]
  [(solder_paste_margin MM)]
  [(solder_paste_margin_ratio RATIO)]
  [(clearance MM)]
  [(zone_connect 0|1|2|3)]
  [(thermal_width MM)]
  [(thermal_gap MM)]
  [(options (clearance outline|convexhull) (anchor rect|circle))]   ; custom pad only
  [(primitives                                                        ; custom pad only
    GRAPHIC_ITEMS...
    (width MM)
    [(fill yes)]
  )]
)
```

## Pad Types

| Token | Description |
|-------|-------------|
| `thru_hole` | Through-hole pad |
| `smd` | Surface-mount pad |
| `connect` | Connection pad (for nets) |
| `np_thru_hole` | Non-plated through-hole |

## Pad Shapes

| Token | Description |
|-------|-------------|
| `circle` | Circle |
| `rect` | Rectangle |
| `oval` | Oval |
| `trapezoid` | Trapezoid |
| `roundrect` | Rounded rectangle (requires `roundrect_rratio`) |
| `custom` | Custom shape (requires `primitives`) |

## Drill Definition

```scheme
; Round hole
(drill DIAMETER)

; Oval hole (slot)
(drill oval DIAMETER SLOT_WIDTH)

; With offset
(drill [oval] DIAMETER [SLOT_WIDTH] (offset X Y))
```

## Pad Special Properties (`property` token)

| Token | Description |
|-------|-------------|
| `pad_prop_bga` | BGA pad |
| `pad_prop_fiducial_glob` | Global fiducial |
| `pad_prop_fiducial_loc` | Local fiducial |
| `pad_prop_testpoint` | Test point |
| `pad_prop_heatsink` | Heatsink pad |
| `pad_prop_castellated` | Castellated pad |

## Zone Connection Types

| Value | Description |
|-------|-------------|
| `0` | Not connected to zone |
| `1` | Connected with thermal relief |
| `2` | Connected with solid fill |
| `3` | Through-hole thermal only, SMD solid |

## Layer List Examples

```scheme
; SMD pad — front side
(layers "F.Cu F.Paste F.Mask")

; Through-hole pad — both sides + mask
(layers "*.Cu *.Mask")

; Via-like — copper only
(layers "*.Cu")
```

## Custom Pad Example

```scheme
(pad "1" smd custom
  (at 0 0)
  (size 1 1)
  (layers "F.Cu F.Paste F.Mask")
  (options (clearance outline) (anchor circle))
  (primitives
    (gr_poly (pts
      (xy 0.5 0) (xy 0 0.5) (xy -0.5 0) (xy 0 -0.5)
    ) (width 0))
    (width 0.1)
    (fill yes)
  )
  (net 1 "GND")
  (uuid xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)
)
```

## Creating Pads via pcbnew API

```python
import pcbnew

board = pcbnew.GetBoard()
fp = board.FindFootprintByReference("U1")

pad = pcbnew.PAD(fp)
pad.SetNumber("1")
pad.SetAttribute(pcbnew.PAD_ATTRIB_SMD)
pad.SetShape(pcbnew.PAD_SHAPE_RECT)
pad.SetSize(pcbnew.FromMM(1.5), pcbnew.FromMM(1.0))
pad.SetLayerSet(pcbnew.F_Cu)

fp.Add(pad)
pcbnew.Refresh()
```
