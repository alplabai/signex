# KiCad Board File Format — Full Reference

> Extension: `.kicad_pcb` | Available since KiCad 4.0; this reference covers 6.0+

---

## Top-Level File Structure

```scheme
(kicad_pcb
  (version YYYYMMDD)
  (generator "YOUR_TOOL_NAME")   ; ⚠️ Do NOT use "pcbnew"
  (generator_version "1.0")      ; Required KiCad 8+

  (general
    (thickness MM)               ; board thickness
  )

  (paper ...)
  (title_block ...)

  (layers
    (ORDINAL "CANONICAL_NAME" TYPE ["USER_NAME"])
    ...
  )

  (setup ...)

  ; Optional properties
  (property "KEY" "VALUE")

  ; Nets (required — at minimum net 0)
  (net 0 "")
  (net 1 "GND")
  (net 2 "+3V3")

  ; Content sections (order is not critical except header)
  FOOTPRINTS...
  GRAPHIC_ITEMS...
  IMAGES...
  TRACK_SEGMENTS...
  TRACK_VIAS...
  TRACK_ARCS...
  ZONES...
  GROUPS...
)
```

> ⚠️ Do NOT use `"pcbnew"` as the `generator` value.
> ⚠️ The header (`kicad_pcb version generator`) must be the **first token**;
> other sections can appear in any order.

---

## Layers Section

```scheme
(layers
  (0  "F.Cu"      signal)
  (1  "In1.Cu"    signal)
  (31 "B.Cu"      signal)
  (32 "B.Adhes"   user "B.Adhesive")   ; optional user-visible name
  (44 "Edge.Cuts" user)
  ...
)
```

Layer types: `jumper` | `mixed` | `power` | `signal` | `user`

---

## Setup Section

```scheme
(setup
  [(stackup ...)]
  (pad_to_mask_clearance MM)
  [(solder_mask_min_width MM)]
  [(pad_to_paste_clearance MM)]
  [(pad_to_paste_clearance_ratio RATIO)]
  [(aux_axis_origin X Y)]
  [(grid_origin X Y)]
  (pcbplotparams ...)
)
```

### Stackup

```scheme
(stackup
  (layer "F.Cu" 1
    (type "copper")
    [(color "...")]
    [(thickness MM)]
    [(material "...")]
    [(epsilon_r DIELECTRIC_CONSTANT)]
    [(loss_tangent LOSS_TANGENT)]
  )
  (layer "dielectric 1" 2
    (type "core")
    (thickness 1.51)
    (material "FR4")
    (epsilon_r 4.5)
    (loss_tangent 0.02)
  )
  ; ... other layers
  [(copper_finish "ENIG")]
  [(dielectric_constraints yes|no)]
  [(edge_connector yes|bevelled)]
  [(castellated_pads yes)]
  [(edge_plating yes)]
)
```

### pcbplotparams (Plot Settings)

```scheme
(pcbplotparams
  (layerselection 0x...)        ; hex bit set — which layers to plot
  (outputformat 0|1|2|3|4|5)   ; 0=gerber 1=PS 2=SVG 3=DXF 4=HPGL 5=PDF
  (usegerberextensions true|false)
  (usegerberattributes true|false)
  (usegerberadvancedattributes true|false)
  (creategerberjobfile true|false)
  (excludeedgelayer true|false)
  (plotframeref true|false)
  (viasonmask true|false)
  (mode 1|2)                    ; 1=normal, 2=outline/sketch
  (useauxorigin true|false)
  (plotreference true|false)
  (plotvalue true|false)
  (subtractmaskfromsilk true|false)
  (mirror true|false)
  (drillshape SHAPE)
  (outputdirectory "./gerber/")
)
```

---

## Nets Section

```scheme
(net 0 "")          ; empty net — always ordinal 0
(net 1 "GND")
(net 2 "+3V3")
(net 3 "/MCU/PA0")
```

> Net class definitions were moved to `.kicad_dru` (design rules) in KiCad 6.

---

## Track Segment

```scheme
(segment
  (start X Y)
  (end X Y)
  (width MM)
  (layer LAYER)
  [(locked)]
  (net NET_NUMBER)
  (tstamp UUID)           ; ⚠️ uses `tstamp`, not `uuid`
)
```

---

## Track Via

```scheme
(via
  [blind|micro]           ; omit for through-hole
  [(locked)]
  (at X Y)
  (size ANNULAR_RING_DIAMETER)
  (drill HOLE_DIAMETER)
  (layers "F.Cu" "B.Cu")  ; layers it connects
  [(remove_unused_layers)]
  [(keep_end_layers)]     ; only with remove_unused_layers
  [(free)]                ; can move independently from net
  (net NET_NUMBER)
  (tstamp UUID)
)
```

---

## Track Arc (KiCad 7+)

```scheme
(arc
  (start X Y)
  (mid X Y)
  (end X Y)
  (width MM)
  (layer LAYER)
  [(locked)]
  (net NET_NUMBER)
  (tstamp UUID)
)
```

---

## Minimum Valid KiCad 10 Board File

```scheme
(kicad_pcb (version 20250324) (generator "my-tool") (generator_version "1.0")
  (general
    (thickness 1.6)
  )
  (paper "A4")
  (layers
    (0  "F.Cu"      signal)
    (31 "B.Cu"      signal)
    (36 "B.SilkS"   user)
    (37 "F.SilkS"   user)
    (38 "B.Paste"   user)
    (39 "F.Paste"   user)
    (40 "B.Mask"    user)
    (41 "F.Mask"    user)
    (44 "Edge.Cuts" user)
    (45 "Margin"    user)
    (46 "B.CrtYd"   user)
    (47 "F.CrtYd"   user)
    (48 "B.Fab"     user)
    (49 "F.Fab"     user)
  )
  (setup
    (pad_to_mask_clearance 0)
  )
  (net 0 "")
)
```

---

## Python: Reading and Writing Boards

```python
import pcbnew

# Load
board = pcbnew.LoadBoard("board.kicad_pcb")

# Iterate footprints
for fp in board.GetFootprints():
    ref = fp.GetReference()
    pos = fp.GetPosition()
    layer = fp.GetLayerName()
    print(f"{ref} @ ({pcbnew.ToMM(pos.x):.3f}, {pcbnew.ToMM(pos.y):.3f}) on {layer}")

# Filter tracks by net
for track in board.GetTracks():
    if track.GetNetname() == "GND":
        print(f"  Track: {track.GetStart()} → {track.GetEnd()}")

# Add a new track
track = pcbnew.PCB_TRACK(board)
track.SetStart(pcbnew.FromMM(10), pcbnew.FromMM(20))
track.SetEnd(pcbnew.FromMM(30), pcbnew.FromMM(20))
track.SetWidth(pcbnew.FromMM(0.25))
track.SetLayer(pcbnew.F_Cu)
board.Add(track)

# Save
pcbnew.SaveBoard("board_new.kicad_pcb", board)
pcbnew.Refresh()
```
