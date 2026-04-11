# Pad Token — Detaylı Referans

## Tam Pad Yapısı

```scheme
(pad "NUMARA"
  PAD_TİPİ
  PAD_ŞEKLİ
  (at X Y [AÇI])
  [(locked)]
  (size GENİŞLİK YÜKSEKLİK)
  [(drill DRILL_TANIMI)]
  (layers "KATMAN_LİSTESİ")
  [(property PAD_ÖZELLIĞI)]
  [(remove_unused_layer)]
  [(keep_end_layers)]
  [(roundrect_rratio 0.0-1.0)]
  [(chamfer_ratio 0.0-1.0)]
  [(chamfer top_left top_right bottom_left bottom_right)]
  [(net NUMARA "NET_ADI")]
  (uuid UUID)
  [(pinfunction "PIN_FONKSİYONU")]
  [(pintype "PIN_TİPİ")]
  [(die_length UZUNLUK)]
  [(solder_mask_margin MM)]
  [(solder_paste_margin MM)]
  [(solder_paste_margin_ratio ORAN)]
  [(clearance MM)]
  [(zone_connect 0|1|2|3)]
  [(thermal_width MM)]
  [(thermal_gap MM)]
  [(options (clearance outline|convexhull) (anchor rect|circle))]   ; sadece custom pad
  [(primitives                                                        ; sadece custom pad
    GRAFIK_ÖGELER...
    (width MM)
    [(fill yes)]
  )]
)
```

## Pad Tipleri

| Token | Açıklama |
|-------|----------|
| `thru_hole` | Delikli (through-hole) pad |
| `smd` | Yüzey montaj (SMD) pad |
| `connect` | Bağlantı pad'i (net için) |
| `np_thru_hole` | Delikli ama elektriksiz (non-plated) |

## Pad Şekilleri

| Token | Açıklama |
|-------|----------|
| `circle` | Daire |
| `rect` | Dikdörtgen |
| `oval` | Oval |
| `trapezoid` | Yamuk |
| `roundrect` | Yuvarlatılmış dikdörtgen (`roundrect_rratio` gerekir) |
| `custom` | Özel şekil (`primitives` gerekir) |

## Drill Tanımı

```scheme
; Yuvarlak delik
(drill ÇAPI)

; Oval delik (slot)
(drill oval ÇAPI SLOT_GENİŞLİĞİ)

; Offset ile
(drill [oval] ÇAPI [SLOT_GENİŞLİĞİ] (offset X Y))
```

## Pad Özel Özellikleri (`property` token)

| Token | Açıklama |
|-------|----------|
| `pad_prop_bga` | BGA pad |
| `pad_prop_fiducial_glob` | Global fiducial |
| `pad_prop_fiducial_loc` | Lokal fiducial |
| `pad_prop_testpoint` | Test noktası |
| `pad_prop_heatsink` | Isı emici |
| `pad_prop_castellated` | Castellated pad |

## Zone Bağlantı Tipleri

| Değer | Açıklama |
|-------|----------|
| `0` | Zone'a bağlı değil |
| `1` | Thermal relief ile bağlı |
| `2` | Solid fill ile bağlı |
| `3` | Sadece through-hole thermal, SMD solid |

## Layer Listesi Örnekleri

```scheme
; SMD pad — ön yüz
(layers "F.Cu F.Paste F.Mask")

; Through-hole pad — her iki yüz + masker
(layers "*.Cu *.Mask")

; Via benzeri — sadece bakır
(layers "*.Cu")
```

## Custom Pad Örneği

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

## pcbnew API ile Pad Oluşturma

```python
import pcbnew

board = pcbnew.GetBoard()
fp = board.FindFootprintByReference("U1")

# Yeni pad
pad = pcbnew.PAD(fp)
pad.SetNumber("1")
pad.SetAttribute(pcbnew.PAD_ATTRIB_SMD)
pad.SetShape(pcbnew.PAD_SHAPE_RECT)
pad.SetSize(pcbnew.FromMM(1.5), pcbnew.FromMM(1.0))
pad.SetLayerSet(pcbnew.F_Cu)

fp.Add(pad)
pcbnew.Refresh()
```
