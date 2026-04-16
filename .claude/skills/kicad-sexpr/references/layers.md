# KiCad Canonical Layer Names — Full List

## Copper Layers

| Canonical Name | Description |
|----------------|-------------|
| `F.Cu` | Front (top) copper layer |
| `In1.Cu` | Inner copper layer 1 |
| `In2.Cu` | Inner copper layer 2 |
| `In3.Cu` | Inner copper layer 3 |
| `In4.Cu` | Inner copper layer 4 |
| `In5.Cu` | Inner copper layer 5 |
| `In6.Cu` | Inner copper layer 6 |
| `In7.Cu` | Inner copper layer 7 |
| `In8.Cu` | Inner copper layer 8 |
| `In9.Cu` | Inner copper layer 9 |
| `In10.Cu` | Inner copper layer 10 |
| `In11.Cu` | Inner copper layer 11 |
| `In12.Cu` | Inner copper layer 12 |
| `In13.Cu` | Inner copper layer 13 |
| `In14.Cu` | Inner copper layer 14 |
| `In15.Cu` | Inner copper layer 15 |
| `In16.Cu` | Inner copper layer 16 |
| `In17.Cu` | Inner copper layer 17 |
| `In18.Cu` | Inner copper layer 18 |
| `In19.Cu` | Inner copper layer 19 |
| `In20.Cu` | Inner copper layer 20 |
| `In21.Cu` | Inner copper layer 21 |
| `In22.Cu` | Inner copper layer 22 |
| `In23.Cu` | Inner copper layer 23 |
| `In24.Cu` | Inner copper layer 24 |
| `In25.Cu` | Inner copper layer 25 |
| `In26.Cu` | Inner copper layer 26 |
| `In27.Cu` | Inner copper layer 27 |
| `In28.Cu` | Inner copper layer 28 |
| `In29.Cu` | Inner copper layer 29 |
| `In30.Cu` | Inner copper layer 30 |
| `B.Cu` | Back (bottom) copper layer |

## Technical Layers

| Canonical Name | Description |
|----------------|-------------|
| `B.Adhes` | Back adhesive |
| `F.Adhes` | Front adhesive |
| `B.Paste` | Back solder paste |
| `F.Paste` | Front solder paste |
| `B.SilkS` | Back silkscreen |
| `F.SilkS` | Front silkscreen |
| `B.Mask` | Back solder mask |
| `F.Mask` | Front solder mask |

## Drawing / Comment Layers

| Canonical Name | Description |
|----------------|-------------|
| `Dwgs.User` | User drawing layer |
| `Cmts.User` | User comment layer |
| `Eco1.User` | ECO layer 1 |
| `Eco2.User` | ECO layer 2 |

## Board Edge Layers

| Canonical Name | Description |
|----------------|-------------|
| `Edge.Cuts` | Board cut boundary |
| `Margin` | Board edge margin |

## Footprint Layers

| Canonical Name | Description |
|----------------|-------------|
| `F.CrtYd` | Front courtyard (keep-out area) |
| `B.CrtYd` | Back courtyard |
| `F.Fab` | Front fabrication layer |
| `B.Fab` | Back fabrication layer |

## User-Defined Layers

| Canonical Name | Description |
|----------------|-------------|
| `User.1` | User layer 1 |
| `User.2` | User layer 2 |
| `User.3` | User layer 3 |
| `User.4` | User layer 4 |
| `User.5` | User layer 5 |
| `User.6` | User layer 6 |
| `User.7` | User layer 7 |
| `User.8` | User layer 8 |
| `User.9` | User layer 9 |

## Wildcard Usage

```scheme
(layer *.Cu)       ; all copper layers
(layer F.*)        ; all front layers (for canonical names only)
```

## Python Layer Numbers (pcbnew API)

```python
import pcbnew

# Layer name -> number
layer_num = pcbnew.GetLayerByName("F.Cu")  # -> 0

# Number -> name
layer_name = pcbnew.GetLayerName(0)         # -> "F.Cu"

# Frequently used constants
pcbnew.F_Cu    # 0
pcbnew.B_Cu    # 31
pcbnew.F_SilkS # 37
pcbnew.B_SilkS # 36
pcbnew.F_Mask  # 39
pcbnew.B_Mask  # 38
pcbnew.Edge_Cuts  # 44
```
