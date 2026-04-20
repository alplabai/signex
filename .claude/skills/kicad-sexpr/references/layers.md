# KiCad Canonical Layer İsimleri — Tam Liste

## Bakır Katmanlar

| Canonical İsim | Açıklama |
|----------------|----------|
| `F.Cu` | Ön (top) bakır katmanı |
| `In1.Cu` | İç bakır katman 1 |
| `In2.Cu` | İç bakır katman 2 |
| `In3.Cu` | İç bakır katman 3 |
| `In4.Cu` | İç bakır katman 4 |
| `In5.Cu` | İç bakır katman 5 |
| `In6.Cu` | İç bakır katman 6 |
| `In7.Cu` | İç bakır katman 7 |
| `In8.Cu` | İç bakır katman 8 |
| `In9.Cu` | İç bakır katman 9 |
| `In10.Cu` | İç bakır katman 10 |
| `In11.Cu` | İç bakır katman 11 |
| `In12.Cu` | İç bakır katman 12 |
| `In13.Cu` | İç bakır katman 13 |
| `In14.Cu` | İç bakır katman 14 |
| `In15.Cu` | İç bakır katman 15 |
| `In16.Cu` | İç bakır katman 16 |
| `In17.Cu` | İç bakır katman 17 |
| `In18.Cu` | İç bakır katman 18 |
| `In19.Cu` | İç bakır katman 19 |
| `In20.Cu` | İç bakır katman 20 |
| `In21.Cu` | İç bakır katman 21 |
| `In22.Cu` | İç bakır katman 22 |
| `In23.Cu` | İç bakır katman 23 |
| `In24.Cu` | İç bakır katman 24 |
| `In25.Cu` | İç bakır katman 25 |
| `In26.Cu` | İç bakır katman 26 |
| `In27.Cu` | İç bakır katman 27 |
| `In28.Cu` | İç bakır katman 28 |
| `In29.Cu` | İç bakır katman 29 |
| `In30.Cu` | İç bakır katman 30 |
| `B.Cu` | Arka (bottom) bakır katmanı |

## Teknik Katmanlar

| Canonical İsim | Açıklama |
|----------------|----------|
| `B.Adhes` | Arka yapıştırıcı (adhesive) |
| `F.Adhes` | Ön yapıştırıcı |
| `B.Paste` | Arka lehim pastası |
| `F.Paste` | Ön lehim pastası |
| `B.SilkS` | Arka serigrafi (silk screen) |
| `F.SilkS` | Ön serigrafi |
| `B.Mask` | Arka lehim maskesi |
| `F.Mask` | Ön lehim maskesi |

## Çizim / Yorum Katmanları

| Canonical İsim | Açıklama |
|----------------|----------|
| `Dwgs.User` | Kullanıcı çizim katmanı |
| `Cmts.User` | Kullanıcı yorum katmanı |
| `Eco1.User` | ECO katmanı 1 |
| `Eco2.User` | ECO katmanı 2 |

## Kart Sınır Katmanları

| Canonical İsim | Açıklama |
|----------------|----------|
| `Edge.Cuts` | Kart kesim sınırı |
| `Margin` | Kart kenar boşluğu |

## Footprint Katmanları

| Canonical İsim | Açıklama |
|----------------|----------|
| `F.CrtYd` | Ön courtyard (koruma alanı) |
| `B.CrtYd` | Arka courtyard |
| `F.Fab` | Ön fabrication katmanı |
| `B.Fab` | Arka fabrication katmanı |

## Kullanıcı Tanımlı Katmanlar

| Canonical İsim | Açıklama |
|----------------|----------|
| `User.1` | Kullanıcı katmanı 1 |
| `User.2` | Kullanıcı katmanı 2 |
| `User.3` | Kullanıcı katmanı 3 |
| `User.4` | Kullanıcı katmanı 4 |
| `User.5` | Kullanıcı katmanı 5 |
| `User.6` | Kullanıcı katmanı 6 |
| `User.7` | Kullanıcı katmanı 7 |
| `User.8` | Kullanıcı katmanı 8 |
| `User.9` | Kullanıcı katmanı 9 |

## Wildcard Kullanımı

```scheme
(layer *.Cu)       ; tüm bakır katmanlar
(layer F.*)        ; tüm ön katmanlar (sadece canonical isimler için)
```

## Python'da Layer Numaraları (pcbnew API)

```python
import pcbnew

# Layer ismi → numara
layer_num = pcbnew.GetLayerByName("F.Cu")  # → 0

# Numara → isim
layer_name = pcbnew.GetLayerName(0)         # → "F.Cu"

# Sık kullanılan sabitler
pcbnew.F_Cu    # 0
pcbnew.B_Cu    # 31
pcbnew.F_SilkS # 37
pcbnew.B_SilkS # 36
pcbnew.F_Mask  # 39
pcbnew.B_Mask  # 38
pcbnew.Edge_Cuts  # 44
```
