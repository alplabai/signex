# Official KiCad Symbol Library Catalog
> Source: https://kicad.github.io/symbols/ | Updated with KiCad 9 (kicad-symbols master)

For each library: **library name**, brief description, symbol count.
Library file name: `<NAME>.kicad_sym`
Repo: `https://gitlab.com/kicad/libraries/kicad-symbols`

---

## Digital Logic

| Library | Description | # |
|---------|-------------|---|
| `4xxx` | 4000-series CMOS logic | 48 |
| `4xxx_IEEE` | 4000-series IEEE symbols | 99 |
| `74xGxx` | 74xGxx logic | 163 |
| `74xx` | 74xx TTL/CMOS logic | 235 |
| `74xx_IEEE` | 74xx IEEE symbols | 185 |

## Microcontrollers and MCUs

| Library | Description | # |
|---------|-------------|---|
| `MCU_AnalogDevices` | Analog Devices MCU | 1 |
| `MCU_Cypress` | Cypress MCU | 36 |
| `MCU_Dialog` | Dialog Semiconductor MCU | 2 |
| `MCU_Espressif` | ESP8266/ESP32 (Espressif) | 1 |
| `MCU_Intel` | Intel MCU | 26 |
| `MCU_Microchip_ATmega` | AVR ATmega | 440 |
| `MCU_Microchip_ATtiny` | AVR ATtiny | 199 |
| `MCU_Microchip_AVR` | AVR general | 19 |
| `MCU_Microchip_8051` | 8051 series | 12 |
| `MCU_Microchip_PIC10` | PIC10 | 24 |
| `MCU_Microchip_PIC12` | PIC12 | 114 |
| `MCU_Microchip_PIC16` | PIC16 | 252 |
| `MCU_Microchip_PIC18` | PIC18 | 172 |
| `MCU_Microchip_PIC24` | PIC24 | 3 |
| `MCU_Microchip_PIC32` | PIC32 | 17 |
| `MCU_Microchip_SAMD` | SAMD series | 87 |
| `MCU_Microchip_SAME` | SAME series | 16 |
| `MCU_Microchip_SAML` | SAML series | 20 |
| `MCU_Microchip_SAMV` | SAMV series | 3 |
| `MCU_Module` | Arduino, Raspberry Pi, breakout modules | 50 |
| `MCU_Nordic` | nRF51, nRF52 | 7 |
| `MCU_NXP_Kinetis` | NXP Kinetis | 155 |
| `MCU_NXP_LPC` | NXP LPC | 105 |
| `MCU_ST_STM32F0` | STM32F0 | 111 |
| `MCU_ST_STM32F1` | STM32F1 | 125 |
| `MCU_ST_STM32F2` | STM32F2 | 47 |
| `MCU_ST_STM32F3` | STM32F3 | 93 |
| `MCU_ST_STM32F4` | STM32F4 | 211 |
| `MCU_ST_STM32F7` | STM32F7 | 113 |
| `MCU_ST_STM32G0` | STM32G0 | 2 |
| `MCU_ST_STM32H7` | STM32H7 | 15 |
| `MCU_ST_STM32L0` | STM32L0 | 150 |
| `MCU_ST_STM32L1` | STM32L1 | 130 |
| `MCU_ST_STM32WL` | STM32WL (LoRa MCU) | — |
| `MCU_Texas` | Texas Instruments MCU | — |

## CPU, DSP, FPGA

| Library | Description | # |
|---------|-------------|---|
| `CPU` | Various CPUs | 6 |
| `CPU_NXP_68000` | Motorola/NXP 68000 | 5 |
| `DSP_AnalogDevices` | ADI DSP | 5 |
| `DSP_Microchip_DSPIC33` | dsPIC33 | 19 |
| `FPGA_Lattice` | Lattice FPGA (iCE40, ECP5) | 24 |
| `FPGA_Microsemi` | Microsemi FPGA | 18 |
| `FPGA_Xilinx` | Xilinx general | 34 |
| `FPGA_Xilinx_Artix7` | Xilinx Artix-7 | 27 |
| `FPGA_Xilinx_Kintex7` | Xilinx Kintex-7 | 18 |
| `FPGA_Xilinx_Spartan6` | Xilinx Spartan-6 | 45 |
| `CPLD_Altera` | Altera (Intel) CPLD | 26 |
| `CPLD_Xilinx` | Xilinx CPLD | 17 |

## Analog / Amplifiers

| Library | Description | # |
|---------|-------------|---|
| `Amplifier_Audio` | Audio amplifiers | 94 |
| `Amplifier_Buffer` | Buffer amplifiers | 7 |
| `Amplifier_Current` | Current sense amplifiers (shunt) | 74 |
| `Amplifier_Difference` | Difference amplifiers | 12 |
| `Amplifier_Instrumentation` | Instrumentation amplifiers | 36 |
| `Amplifier_Operational` | General op-amps | 321 |
| `Amplifier_Video` | Video amplifiers | 2 |
| `Analog` | Various analog | 16 |
| `Analog_ADC` | ADC | 158 |
| `Analog_DAC` | DAC | 126 |
| `Analog_Switch` | Analog switches | 123 |
| `Comparator` | Comparators | 54 |

## Power Management

| Library | Description | # |
|---------|-------------|---|
| `Battery_Management` | Battery management ICs | 120 |
| `Converter_ACDC` | AC/DC converters | 137 |
| `Converter_DCDC` | DC/DC converters | 538 |
| `power` | Power symbols (GND, VCC, PWR_FLAG…) | ~50 |
| `Regulator_Linear` | Linear regulators (LM78xx, LDO…) | — |
| `Regulator_Switching` | Switching regulators | — |

## Discrete Semiconductors

| Library | Description | # |
|---------|-------------|---|
| `Device` | **General components: R, C, L, D, Q, MOSFET, crystal…** | 564 |
| `Diode` | Diodes (general) | 538 |
| `Diode_Bridge` | Bridge diodes/rectifiers | 148 |
| `Diode_Laser` | Laser diodes | 5 |
| `LED` | LED symbols | 54 |
| `Transistor_BJT` | BJT transistors | — |
| `Transistor_FET` | MOSFET/JFET | — |
| `Transistor_IGBT` | IGBT | — |
| `Triac_Thyristor` | TRIAC, SCR, DIAC | — |

## Interface / Communication

| Library | Description | # |
|---------|-------------|---|
| `Interface` | Various interface ICs | 100 |
| `Interface_CAN_LIN` | CAN / LIN | 85 |
| `Interface_CurrentLoop` | 4-20 mA current loop | 2 |
| `Interface_Ethernet` | Ethernet PHY, magnetics | 26 |
| `Interface_Expansion` | I/O expander, shift register | 64 |
| `Interface_HDMI` | HDMI | 2 |
| `Interface_HID` | USB HID | 5 |
| `Interface_LineDriver` | RS-232, RS-485 line drivers | 16 |
| `Interface_Optical` | IR transmitters/receivers | 42 |
| `Interface_UART` | UART ICs | 133 |
| `Interface_USB` | USB controllers, PHY, hubs | 95 |
| `Isolator` | Optocouplers, digital isolation | 315 |
| `Isolator_Analog` | Analog isolation | 7 |

## Connectors

| Library | Description | # |
|---------|-------------|---|
| `Connector` | General connectors (USB, D-SUB, DIN…) | 361 |
| `Connector_Generic` | General-purpose connectors | 274 |
| `Connector_Generic_MountingPin` | With mechanical mounting pins | 274 |
| `Connector_Generic_Shielded` | Shielded connectors | 274 |

## Memory / Storage

| Library | Description |
|---------|-------------|
| `Memory_EEPROM` | EEPROM |
| `Memory_Flash` | Flash memory |
| `Memory_RAM` | SRAM, DRAM |
| `Memory_UniqueID` | 1-Wire ID chips |

## Sensors

| Library | Description |
|---------|-------------|
| `Sensor` | General sensors |
| `Sensor_Current` | Current sensors |
| `Sensor_Humidity` | Humidity sensors |
| `Sensor_Motion` | IMU, accelerometers |
| `Sensor_Optical` | Optical sensors |
| `Sensor_Pressure` | Pressure sensors |
| `Sensor_Temperature` | Temperature sensors |

## RF and Wireless

| Library | Description |
|---------|-------------|
| `RF_Module` | RF modules (LoRa, WiFi, BT…) |
| `RF_Amplifier` | RF amplifiers |
| `RF_Filter` | RF filters |
| `RF_Mixer` | RF mixers |
| `RF_Switch` | RF switches |
| `Wireless` | Wireless ICs |

## Drivers

| Library | Description | # |
|---------|-------------|---|
| `Driver_Display` | Display drivers | 10 |
| `Driver_FET` | MOSFET / gate drivers | 194 |
| `Driver_LED` | LED drivers | 78 |
| `Driver_Motor` | Motor driver ICs | 65 |
| `Driver_Relay` | Relay drivers | 5 |
| `Driver_TEC` | Peltier (TEC) drivers | 2 |

## Displays

| Library | Description | # |
|---------|-------------|---|
| `Display_Character` | Character displays (7-seg, dot matrix) | 128 |
| `Display_Graphic` | Graphic displays | 38 |

## Miscellaneous

| Library | Description | # |
|---------|-------------|---|
| `Audio` | Audio ICs | 82 |
| `Filter` | Filter ICs | 48 |
| `Graphic` | Graphical/decorative symbols | 29 |
| `Jumper` | Solder jumpers | 9 |
| `Logic_LevelTranslator` | Level translators | 34 |
| `Mechanical` | Mechanical components | — |
| `Oscillator` | Oscillators | — |
| `Switch` | Switches | — |
| `Timer` | Timers (555, etc.) | — |
| `Transformer` | Transformers | — |

---

## Device Library — Core Components

The `Device` library is the most commonly used (564 symbols).

### Passive Components
| Symbol | Description | RefDes |
|--------|-------------|--------|
| `R` | Resistor (IEC) | R |
| `R_US` | Resistor (US) | R |
| `R_Small` | Small resistor | R |
| `R_Variable` | Variable resistor | RV |
| `R_Potentiometer` | Potentiometer | RV |
| `C` | Capacitor (non-polarized) | C |
| `C_Polarized` | Polarized capacitor | C |
| `C_Small` | Small capacitor | C |
| `C_Variable` | Variable capacitor | C |
| `L` | Inductor | L |
| `L_Small` | Small inductor | L |
| `L_Ferrite` | Ferrite-core inductor | L |
| `FerriteBead` | Ferrite bead | FB |
| `Fuse` | Fuse | F |

### Semiconductors
| Symbol | Description | RefDes |
|--------|-------------|--------|
| `D` | Diode | D |
| `D_Zener` | Zener diode | D |
| `D_Schottky` | Schottky diode | D |
| `D_TVS` | TVS diode | D |
| `LED` | LED (Device library) | D |
| `Q_NPN_BCE` | NPN BJT | Q |
| `Q_PNP_BCE` | PNP BJT | Q |
| `Q_NMOS_GSD` | N-MOSFET | Q |
| `Q_PMOS_GSD` | P-MOSFET | Q |
| `Q_NMOS_NMOS_GSD` | Dual N-MOSFET | Q |

### Mechanical / Other
| Symbol | Description | RefDes |
|--------|-------------|--------|
| `Crystal` | 2-pin crystal | Y |
| `Crystal_GND24` | 4-pin crystal | Y |
| `Battery` | Multi-cell battery | BT |
| `Battery_Cell` | Single-cell battery | BT |
| `Antenna` | Antenna | AE |
| `TestPoint` | Test point | TP |
| `MountingHole` | Mounting hole | H |
| `Transformer_1P_1S` | 1:1 transformer | T |

---

## Quick Lookup: Which Library?

| Looking for | Library |
|-------------|---------|
| STM32F4 MCU | `MCU_ST_STM32F4` |
| ESP32 | `MCU_Espressif` |
| nRF52 | `MCU_Nordic` |
| LM358 op-amp | `Amplifier_Operational` |
| LM7805 regulator | `Regulator_Linear` |
| AMS1117 LDO | `Regulator_Linear` |
| SN74HC574 | `74xx` |
| MAX485 RS-485 | `Interface_LineDriver` |
| TLP250 optocoupler | `Isolator` |
| NE555 timer | `Timer` |
| INA219 | `Amplifier_Current` |
| DS18B20 | `Sensor_Temperature` |
| GND, VCC, +3V3 | `power` |
| General R, C, L, BJT | `Device` |
| USB connector | `Connector` |
| RJ45 | `Connector` |
| Arduino Nano | `MCU_Module` |

---

## Python: Symbol Search (with kiutils)

```python
from kiutils.symbol import SymbolLib

# Load library
lib = SymbolLib.from_file("Device.kicad_sym")

# List all symbol names
for sym in lib.symbols:
    print(sym.entryName)

# Find specific symbol and get pin list
r = next(s for s in lib.symbols if s.entryName == "R")
for unit in r.units:
    for pin in unit.pins:
        print(f"  Pin {pin.number}: {pin.name} ({pin.electricalType})")
```

## Python: Library ID Normalization

```python
def normalize_lib_id(raw_id: str) -> tuple[str, str]:
    """
    'Device:R' → ('Device', 'R')
    'MCU_ST_STM32F4:STM32F407VGTx' → ('MCU_ST_STM32F4', 'STM32F407VGTx')
    """
    parts = raw_id.split(":", 1)
    if len(parts) == 2:
        return parts[0], parts[1]
    return "", raw_id  # symbol name only (inside library file)
```
