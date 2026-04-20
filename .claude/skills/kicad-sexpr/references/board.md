# KiCad Board Dosya Formatı — Tam Referans

> Uzantı: `.kicad_pcb` | KiCad 4.0'dan itibaren mevcut, 6.0+ için bu referans

---

## Dosya Üst Düzey Yapısı

```scheme
(kicad_pcb
  (version YYYYMMDD)
  (generator ÜRETICI_ADI)   ; ⚠️ "pcbnew" KULLANMA

  (general
    (thickness MM)           ; kart kalınlığı
  )

  (paper ...)
  (title_block ...)

  (layers
    (ORDINAL "CANONICAL_NAME" TYPE ["USER_NAME"])
    ...
  )

  (setup ...)

  ; Opsiyonel properties
  (property "KEY" "VALUE")

  ; Netler (zorunlu — en azından net 0)
  (net 0 "")
  (net 1 "GND")
  (net 2 "+3V3")

  ; İçerik bölümleri (sıra kritik değil, header hariç)
  FOOTPRINTLER...
  GRAFIK_ÖGELER...
  IMAGELER...
  TRACK_SEGMENTLER...
  TRACK_VIALAR...
  TRACK_ARCLAR...
  ZONLAR...
  GRUPLAR...
)
```

> ⚠️ `generator` için `pcbnew` kullanma.
> ⚠️ Header (`kicad_pcb version generator`) **ilk token** olmak zorunda;
> diğer section'ların sırası kritik değil.

---

## Layers Section

```scheme
(layers
  (0  "F.Cu"      signal)
  (1  "In1.Cu"    signal)
  (31 "B.Cu"      signal)
  (32 "B.Adhes"   user "B.Adhesive")   ; opsiyonel kullanıcı adı
  (44 "Edge.Cuts" user)
  ...
)
```

Layer tipleri: `jumper` | `mixed` | `power` | `signal` | `user`

---

## Setup Section

```scheme
(setup
  [(stackup ...)]
  (pad_to_mask_clearance MM)
  [(solder_mask_min_width MM)]
  [(pad_to_paste_clearance MM)]
  [(pad_to_paste_clearance_ratio ORAN)]
  [(aux_axis_origin X Y)]
  [(grid_origin X Y)]
  (pcbplotparams ...)
)
```

### Stackup (Katman İstifleme)

```scheme
(stackup
  (layer "F.Cu" 1
    (type "copper")
    [(color "...")]
    [(thickness MM)]
    [(material "...")]
    [(epsilon_r DIELEKTRIK_SABİTİ)]
    [(loss_tangent KAYIP_TANJANT)]
  )
  (layer "dielectric 1" 2
    (type "core")
    (thickness 1.51)
    (material "FR4")
    (epsilon_r 4.5)
    (loss_tangent 0.02)
  )
  ; ... diğer katmanlar
  [(copper_finish "ENIG")]
  [(dielectric_constraints yes|no)]
  [(edge_connector yes|bevelled)]
  [(castellated_pads yes)]
  [(edge_plating yes)]
)
```

### pcbplotparams (Plot Ayarları)

```scheme
(pcbplotparams
  (layerselection 0x...)        ; hex bit seti — hangi katmanlar plot edilir
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
  ; ... diğer plot parametreleri
)
```

---

## Nets Section

```scheme
(net 0 "")          ; boş net — her zaman ordinal 0
(net 1 "GND")
(net 2 "+3V3")
(net 3 "/MCU/PA0")
```

> ℹ️ Net class tanımları KiCad 6'da `.kicad_dru` (design rules) dosyasına taşınmıştır.

---

## Track Segment

```scheme
(segment
  (start X Y)
  (end X Y)
  (width MM)
  (layer KATMAN)
  [(locked)]
  (net NET_NUMARASI)
  (tstamp UUID)           ; ⚠️ `uuid` değil, `tstamp` kullanılır
)
```

---

## Track Via

```scheme
(via
  [blind|micro]           ; tip belirtilmezse through-hole
  [(locked)]
  (at X Y)
  (size HALKA_ÇAPI)
  (drill DELIK_ÇAPI)
  (layers "F.Cu" "B.Cu")  ; bağladığı katmanlar
  [(remove_unused_layers)]
  [(keep_end_layers)]     ; sadece remove_unused_layers ile birlikte
  [(free)]                ; net dışında serbest hareket edebilir
  (net NET_NUMARASI)
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
  (layer KATMAN)
  [(locked)]
  (net NET_NUMARASI)
  (tstamp UUID)
)
```

---

## Gerçek Board Örneği

```scheme
(kicad_pcb (version 3) (host pcbnew "(2013-02-20 BZR 3963)-testing")

  (general
    (thickness 1.6)
    (drawings 5)
    (tracks 5)
    (zones 0)
    (modules 2)
    (nets 3)
  )

  (page A4)
  (layers
    (15 top_side.Cu signal)
    (0  bottom_side.Cu signal)
    (28 Edge.Cuts user)
  )

  (net 0 "")
  (net 1 /SIGNAL)
  (net 2 GND)

  (module R3 (layer top_side.Cu) (tedit 4E4C0E65) (tstamp 5127A136)
    (at 66.04 33.3502)
    (fp_text reference R1 (at 0 0.127) (layer F.SilkS) hide
      (effects (font (size 1.397 1.27) (thickness 0.2032)))
    )
    (fp_text value 330K (at 0 0.127) (layer F.SilkS)
      (effects (font (size 1.397 1.27) (thickness 0.2032)))
    )
    (fp_line (start -3.81 0) (end -3.302 0) (layer F.SilkS) (width 0.2032))
    (pad 1 thru_hole circle (at -3.81 0) (size 1.397 1.397) (drill 0.812799)
      (layers *.Cu *.Mask F.SilkS)
      (net 1 /SIGNAL)
    )
    (pad 2 thru_hole circle (at 3.81 0) (size 1.397 1.397) (drill 0.812799)
      (layers *.Cu *.Mask F.SilkS)
      (net 2 GND)
    )
    (model discret/resistor.wrl
      (at (xyz 0 0 0))
      (scale (xyz 0.3 0.3 0.3))
      (rotate (xyz 0 0 0))
    )
  )

  (gr_line (start 58 42) (end 58 29) (layer Edge.Cuts) (width 0.15))
  (gr_line (start 74 42) (end 58 42) (layer Edge.Cuts) (width 0.15))
  (gr_line (start 74 29) (end 74 42) (layer Edge.Cuts) (width 0.15))
  (gr_line (start 58 29) (end 74 29) (layer Edge.Cuts) (width 0.15))

  (segment (start 61.0616 36.8808) (end 61.0616 34.5186)
    (width 0.254) (layer bottom_side.Cu) (net 1))

  (zone (net 2) (net_name GND) (layer bottom_side.Cu)
    (tstamp 5127A1B2) (hatch edge 0.508)
    (connect_pads (clearance 0.2))
    (min_thickness 0.1778)
    (fill (thermal_gap 0.254) (thermal_bridge_width 0.4064))
    (polygon (pts
      (xy 59 30) (xy 73 30) (xy 73 41) (xy 59 41)
    ))
  )
)
```

---

## Python: Board Okuma/Yazma

```python
import pcbnew

# Yükle
board = pcbnew.LoadBoard("devre.kicad_pcb")

# Footprint'leri dolaş
for fp in board.GetFootprints():
    ref = fp.GetReference()
    pos = fp.GetPosition()
    layer = fp.GetLayerName()
    print(f"{ref} @ ({pcbnew.ToMM(pos.x):.3f}, {pcbnew.ToMM(pos.y):.3f}) on {layer}")

# Net'e göre track'leri filtrele
for track in board.GetTracks():
    if track.GetNetname() == "GND":
        print(f"  Track: {track.GetStart()} → {track.GetEnd()}")

# Yeni track ekle
track = pcbnew.PCB_TRACK(board)
track.SetStart(pcbnew.FromMM(10), pcbnew.FromMM(20))
track.SetEnd(pcbnew.FromMM(30), pcbnew.FromMM(20))
track.SetWidth(pcbnew.FromMM(0.25))
track.SetLayer(pcbnew.F_Cu)
board.Add(track)

# Kaydet
pcbnew.SaveBoard("devre_yeni.kicad_pcb", board)
pcbnew.Refresh()
```
