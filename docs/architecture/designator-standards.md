# Designator standards: class letters and sources

`signex_types::designator::standard::class_letters` maps a component class to
the reference letters each supported standard gives it. This page records where
every letter comes from, so a change can be checked against the source rather
than memory.

**Rules.**

- **One letter class per standard, as the standard names it.** For EN IEC
  81346-2 that is the main class, the first letter. The 2019 edition also defines
  two- and three-letter class codes (for example `QAC`); those are a standard
  of their own, **EN IEC 81346-2 (full codes)** (`iec81346_2_full`, #1416),
  listed under [Full class codes](#full-class-codes).
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

## Full class codes

The **EN IEC 81346-2 (full codes)** standard (#1416) gives each class the
three-letter code of EN IEC 81346-2:2019, Table 3. Every code begins with its
main-class letter from the table above. The short meanings below are written
in our own words; the page is where the code appears in the standard.

| Class | Code | Meaning | Page |
|---|---|---|---|
| resistor | `RAC` | limits current by resistance | p. 53 |
| capacitor | `CAA` | stores electric charge | p. 32 |
| inductor | `CBA` | stores energy in a magnetic field | p. 32 |
| diode | `RAA` | lets current pass one way | p. 53 |
| led | `PFA` | shows a state by light | p. 49 |
| transistor (BJT/MOSFET/JFET) | `QAC` | switches or controls current electronically | p. 51 |
| opamp | `TFA` | amplifies a signal | p. 57 |
| regulator_switching | `TAB` | converts one DC voltage to another | p. 57 |
| mcu / logic | `KEB` | processes information (processor) | p. 44 |
| memory | `CFA` | stores information | p. 32 |
| connector | `XDB` | connects by plug and socket | p. 65 |
| switch | `SJA` | switch operated by hand (push-button) | p. 56 |
| relay | `KFB` | switches a circuit from a control signal | p. 44 |
| transformer | `TAA` | transforms an AC voltage, with isolation | p. 56 |
| fuse | `FCA` | protects against overcurrent by melting | p. 36 |
| antenna | `TFB` | sends or receives radio waves | p. 57 |
| display | `PH` | shows information graphically (subclass; the table names no three-letter code) | p. 50 |
| sensor | `B` | main class only: sensors span several codes (`BTA`, `BLA`, ...), so no one code fits the whole class | - |
| motor | `MAA` | converts electric energy to rotation | p. 46 |
| battery | `GBA` | supplies energy from a chemical store | - |

Classes the standard does not name (comparator, regulator_linear, crystal,
generic) keep the symbol's own letters, as under the single-letter option.

**The product-aspect sign.** The full reference designation is written with
the product-aspect prefix sign, `-QAC1`. Signex stores the reference without
it (`QAC1`): annotation, the duplicate-designator ERC check, Re-prefix, Find
Similar and every other tool work on the stored form, and each code counts on
its own (`QAC1`, `QAC2` beside `Q1`). The sign is added only where a reference
is shown or exported, by one helper,
`signex_types::designator::display_reference`:

- the schematic canvas, where the reference field is also boxed and hit on
  the drawn text;
- the schematic outputs built from the export scope (PDF, SVG, print preview,
  BOM, netlist, ERC report);
- the PCB canvas designators (`PcbSnapshot::with_designator_standard`);
- the board outputs, written from a copy of the board whose footprint
  references and designator texts are signed
  (`signex_output::show_board_references`): pick and place, the legend text
  on the Gerbers, IPC-2581 (`refDes`), the ODB++ components, the board STEP
  model, the board PDF and the board reports.

**The IPC-D-356 test netlist is written unsigned** (maintainer decision,
2026-11-18). Its designator field holds 6 characters, so a signed `-CAA100`
would be cut to `-CAA10` and read as a second `CAA10`. The test netlist
writes the stored designator (`CAA100`), even though Fabrication Outputs
generates it from the signed copy: `FabOptions::designator_standard` names
the standard the copy was signed under, and the writer turns each
designator back with `stored_reference`.

The PCB selection index still boxes a designator on its stored text.

**Spreadsheets.** A pick-and-place or BOM CSV cell that starts with `-`
(`-QAC1`) can be read as a formula by a spreadsheet program, which then shows
an error or a number instead of the designator. Signex writes the cell
unchanged; import the file as text, or set the designator column to text,
when you open it in a spreadsheet.

Going the other way, `stored_reference` drops a `-` typed in front of a
designator, as it is shown, wherever one is entered or named: the schematic
reference edit (Properties and in place), the PCB designator edit in
Properties, and the automation operands that take a designator
(`place_component designator=`, `set_footprint_text designator=`,
`place_board_reuse_block designator=`, the part-swap commands). So `-QAC9`
is stored, or found, as `QAC9`, and annotation keeps working.

**Provisional codes.** Three entries of the table are provisional (maintainer
accepted 2026-11-18): display `PH`, sensor `B` and battery `GBA`. They ship
as listed and may change once they are checked against the text of the
standard.

Re-prefix counts a full code and its main-class letter as issued for each
other, so a project can move between the single-letter and the full-code
option and back with **Re-prefix to standard**.
