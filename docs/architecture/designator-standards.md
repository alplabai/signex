# Designator standards: class letters and sources

`signex_types::designator::standard::class_letters` maps a component class to
the reference letters each supported standard gives it. This page records where
every letter comes from, so a change can be checked against the source rather
than memory.

**Rules.**

- **One letter class per standard, as the standard names it.** For EN IEC
  81346-2 that is the main class, the first letter. The 2019 edition also defines
  two- and three-letter subclasses (for example `QAC`), which Signex does not
  use.
- **No guessing.** If a standard does not name a class, the table holds `None`
  for it. A part of that class then keeps its library symbol's own letters,
  exactly as with no standard selected.
- **IEEE 315.** IEEE 315 gives integrated-circuit packages the letter `U`. The
  classes that are normally an IC package (op-amp, comparator, regulator, MCU,
  logic, memory, sensor) use `U`. The discrete-circuit letters (`AR` amplifier,
  `VR` voltage regulator, `A` / `MT` transducer) are not used.

## Sources

| Short name | Document | Where |
|---|---|---|
| IEEE | IEEE Std 315-1975 (R1993), *Graphic Symbols for Electrical and Electronics Diagrams* | §22.4 "Class Designation Letters: Alphabetical List", printed pp. 212-219 |
| DIN | T. Bindel, D. Hofmanns, *EMSR-Stellenplan: Symbolik und Übergang von DIN 40719-2 zu …* | Abb. 3.13 and 3.14 (electrical, single letters), printed pp. 33-34 |
| IEC | EN IEC 81346-2:2019, Edition 2.0 (2019-06) | Table 1 (classes) and Table 3 (objects), printed pages below |

## Table

| Class | IEEE 315 | DIN 40719-2 | EN IEC 81346-2 |
|---|---|---|---|
| resistor | `R` "resistor", p. 217 | `R` "Festwiderstände", p. 34 | `R` (RAC "resistor"), p. 53 |
| capacitor | `C` "capacitor", p. 213 | `C` "Kondensatoren", p. 33 | `C` (CAA "capacitor"), p. 32 |
| inductor | `L` "inductor", p. 216 | `L` "Induktivitäten und Drosseln", p. 33 | `C` (CBA "inductor", storing), p. 32 |
| diode | `D` "D or CR", p. 214 | `V` "Diode", p. 34 | `R` (RAA "diode"), p. 53 |
| led | `DS` "light-emitting solid-state device", p. 214 | `H` "Leuchtdiode", p. 33 | `P` (PFA "visible state indicator"), p. 49 |
| transistor (BJT/MOSFET/JFET) | `Q` "transistor", p. 217 | `V` "Transistor", p. 34 | `Q` (QAC "transistor"), p. 51 |
| opamp | `U` IC package, p. 218 | `N` "Operationsverstärker", p. 34 | `T` (TFA "amplifier"), p. 57 |
| comparator | `U` IC package, p. 218 | `N` analog IC, p. 34 | not named, so `None` |
| regulator_linear | `U` IC package, p. 218 | `N` "elektronische Regler", p. 34 | not named, so `None` |
| regulator_switching | `U` IC package, p. 218 | `U` "Gleichspannungswandler", p. 34 | `T` (TAB "DC/DC converter"), p. 57 |
| mcu / logic | `U` "integrated-circuit package", p. 218 | `D` "Integrierte digitale Schaltkreise", p. 33 | `K` (KEB "CPU"), p. 44 |
| memory | `U` IC package, p. 218 | `D` "Speichereinrichtungen", p. 33 | `C` (CFA "information storing media"), p. 32 |
| connector | `J` "jack, receptacle connector" (a plug is `P`), pp. 216-217 | `X` "Stecker, Steckdose", p. 34 | `X` (XDB "plug, socket"), p. 65 |
| switch | `S` "switch", p. 218 | `S` "Taster, Wahlschalter", p. 34 | `S` (SJA "pushbutton switch"), p. 56 |
| relay | `K` "relay", p. 216 | `K` "Relais, Schütze", p. 33 | `K` (KFB "relay"), p. 44 |
| crystal | `Y` "quartz crystal unit", p. 219 | `G` "Quarzoszillator", p. 33 | not named, so `None` |
| transformer | `T` "transformer", p. 218 | `T` "Transformator", p. 34 | `T` (TAA "isolating transformer"), p. 56 |
| fuse | `F` "fuse", p. 215 | `F` "Sicherung", p. 33 | `F` (FCA "fuse"), p. 36 |
| antenna | `E` "antenna", p. 215 | `W` "Antennen", p. 34 | `T` (TFB "antenna"), p. 57 |
| display | `DS` "alphanumeric display device", p. 214 | `P` "Datensichtgeräte", p. 34 | `P` (PH "graphical display"), p. 50 |
| sensor | `U` IC package, p. 218 | `B` "Messumformer", p. 33 | `B` (BTA, BLA…) |
| motor | `B` "motor", p. 213 | `M` "Motoren", p. 33 | `M` (MAA "electric motor"), p. 46 |
| battery | `BT` "battery", p. 213 | `G` "Batterie", p. 33 | `G` (GBA "battery") |

Under EN IEC 81346-2, resistors and diodes share `R`, and inductors, capacitors
and memory share `C`. Annotation numbers each letter with one counter, so they
never collide.
