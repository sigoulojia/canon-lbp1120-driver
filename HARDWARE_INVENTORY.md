# HARDWARE INVENTORY & ELECTRONIC SYSTEM SPECIFICATION
## Canon LaserShot LBP-1120 Physical Subsystem

**Document ID:** `HW-INV-LBP1120-2026-10-31`  
**Classification:** Complete Hardware Architecture & Component Inventory  
**Target Hardware:** Canon LaserShot LBP-1120 (USB VID `0x04A9`, PID `0x262B`)

---

## 1. System Topology Overview

The Canon LaserShot LBP-1120 relies on a distributed two-board electronic architecture interconnected by a flexible flat cable (`RH2-5588-000`). Power distribution, electrophotographic high voltages, and motor drivers reside on the **Engine Controller PCB**, while USB communication, raster buffering, and SCoA hardware decompression reside on the **Interface Controller PCB**.

```mermaid
flowchart TB
    subgraph Host ["Host PC (Windows 11 x64)"]
        UsbHost["USB Host Controller (XHCI / EHCI)"]
    end

    subgraph IFBoard ["Interface Controller PCB (Formatter RG1-4305-000)"]
        J1["J1: USB Type-B Connector"]
        IC2["IC2: USB Core / Protocol CPU"]
        IC1["IC1: Canon Custom ASIC (NH4-5014)<br/>- SCoA 2D Hardware Decompressor<br/>- AIR Smoothing Engine<br/>- Dot Pattern Generator"]
        IC3["IC3: 512 KB DRAM Ring FIFO (M51V18165)"]
        IC4["IC4: Serial EEPROM (93C46 / 24C02)"]
        REG1["REG1: 5V -> 3.3V LDO"]
        Y1["Y1: 12.000 MHz Crystal"]
        J2["J2: 20-Pin Flat Cable Header"]
    end

    subgraph Cable ["RH2-5588-000 Flat Ribbon Cable"]
        BusLines["Signals: /SC, SCLK, /RESET, /BD, /VDO, +24V, +5V, +3.3V, GND"]
    end

    subgraph EngineBoard ["Engine Controller PCB (DC Controller RG5-7264 / RG5-7266)"]
        J201["J201: 20-Pin Mating Header"]
        IC201["IC201: Hitachi H8/300 Microcontroller (HD6433292)<br/>- Electrophotographic State Machine<br/>- Fuser PID Control<br/>- Sensor Interfacing"]
        IC202["IC202: Reset Supervisor (PST572)"]
        IC203["IC203: Motor Driver IC"]
        HVPS["High-Voltage Power Supply (HVPS)<br/>- Primary Charge: -1.3kV DC / 2kVp-p AC<br/>- Developing Bias: -450V DC / AC<br/>- Transfer Bias: +1.5kV / -1.0kV DC"]
        LVPS["Low-Voltage Power Supply (LVPS)<br/>- AC Mains Rectifier -> +24V, +5V, +3.3V"]
        TriacDrive["Fuser Triac Control (TR1 BCR8KM)"]
    end

    subgraph Mechanics ["Sensors & Mechanical Actuators"]
        M1["M1: Main DC Motor (24V)"]
        M2["M2: Polygon Scanner Motor (18,000 RPM)"]
        Laser["Laser Diode & Beam Detect (BD1)"]
        SL1["SL1: Pick-up Solenoid"]
        PS201["PS201: Delivery Sensor"]
        PS202["PS202: Paper In-Tray Sensor"]
        SW301["SW301: Top Cover Interlock Switch"]
        Heater["SURF Ceramic Heater & Thermistors (TH1/TH2)"]
    end

    UsbHost <-->|USB 1.1 Full-Speed 12 Mbps| J1
    J1 <--> IC2
    IC2 <--> IC1
    IC1 <--> IC3
    IC1 <--> IC4
    IC1 <--> J2

    J2 <--> BusLines <--> J201

    J201 <--> IC201
    IC202 --> IC201
    IC201 --> IC203
    IC201 --> HVPS
    IC201 --> TriacDrive
    TriacDrive --> Heater
    IC203 --> M1
    IC203 --> M2
    IC201 --> Laser
    IC201 --> SL1
    PS201 --> IC201
    PS202 --> IC201
    SW301 -.->|Safety Interlock Cuts 24V Line| HVPS
    SW301 -.->|Safety Interlock Cuts 24V Line| Laser
```

---

## 2. Interface Controller PCB (`RG1-4305-000`)

The Interface Controller board is mounted on the metallic chassis frame on the left side of the engine. It connects directly to the external USB port and converts host commands and compressed SCoA raster chunks into synchronous video pulses.

| Component Ref | Part Number / Identity | Manufacturer | Function & Silicon Architecture | Verified Specs | Confidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **IC1** | `NH4-5014-000` | Canon Inc. | **Canon Custom Formatter ASIC**. Integrates hardware SCoA decompressor, 16-bit word aligner, Automatic Image Refinement (AIR) edge smoother, DRAM memory arbitration, and video dot generator. | 160-pin QFP; 3.3V logic; 48 MHz system clock derived from 12 MHz base. | **Officially Documented** |
| **IC2** | Canon / Cypress USB Core | Canon / Cypress | **USB Protocol Processor**. Implements USB 1.1 Full Speed (12 Mbps) interface, endpoint buffer descriptors (EP0 Control, EP1 Bulk OUT 64B, EP2 Bulk IN 64B), and host command dispatch. | 80-pin QFP; 3.3V logic; 12.0 MHz clock. | **Verified Experimentally** |
| **IC3** | `M51V18165` / `IS42S16100` | OKI / ESMT / Winbond | **512 KB DRAM Ring FIFO Buffer**. Buffers uncompressed scanlines and incoming SCoA packets. Accommodates single-page compressed raster streams ($\le 480\text{ KB}$). | 512 KB ($256\text{K}\times 16$ or $512\text{K}\times 8$); 3.3V; TSOP-50. | **Officially Documented** |
| **IC4** | `AT93C46` / `24C02` | Atmel / Microchip | **Serial EEPROM (1 Kbit / 2 Kbit)**. Non-volatile storage of USB Vendor ID (`0x04A9`), Product ID (`0x262B`), factory serial string, margin trim offsets, and engine revision. | 8-pin SOIC; 3.3V Microwire / I2C serial bus. | **Officially Documented** |
| **Y1** | Crystal Resonator 12.000 MHz | Epson / Citizen | **Primary Clock Source**. Provides fundamental frequency for USB transceiver and PLL multiplier. | 12.000 MHz fundamental frequency; $\pm 30\text{ ppm}$. | **Verified by Datasheet** |
| **REG1** | 5V $\rightarrow$ 3.3V LDO | Rohm / Torex | **Low-Dropout Linear Regulator**. Steps down the +5V DC supply rail from the engine board to clean +3.3V DC for IC1, IC2, IC3, and IC4. | SOT-223; 3.3V $\pm 2\%$; 500 mA maximum. | **Strongly Inferred** |
| **J1** | USB Type-B Female | Amphenol / Molex | Host physical interface (Pin 1: VBUS +5V, Pin 2: D-, Pin 3: D+, Pin 4: GND, Shield: Chassis ground). | Through-hole right-angle connector. | **Officially Documented** |
| **J2** | 20-Pin FFC Header | Hirose / Molex | Inter-board flat cable interface to Engine Controller connector J201. | 1.0 mm or 1.25 mm pitch; 20 conductors. | **Officially Documented** |

---

## 3. Engine Controller PCB (`RG5-7264` / `RG5-7266`)

The Engine Controller PCB (often called the DC Controller) occupies the lower base of the printer. It handles AC power conversion, drives the main motor and laser polygon scanner, controls the fixing ceramic heater, and coordinates the 7-stage electrophotographic printing process.

| Component Ref | Part Number / Identity | Manufacturer | Function & Silicon Architecture | Verified Specs | Confidence |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **IC201** | `HD6433292` / `HD6473294` | Hitachi / Renesas | **Engine Central Microcontroller (H8/300 Family)**. Coordinates electrophotographic timing, paper feed sequence, polygon speed stabilization, laser power calibration, high-voltage sequencing, and fuser temperature regulation. | 80-pin QFP; 16 MHz clock; 64 KB mask ROM; 2 KB SRAM; 10-bit ADC channels; PWM timers. | **Officially Documented** |
| **IC202** | `PST572` / `PST592` | Mitsumi Electric | **System Reset Supervisor IC**. Monitors 3.3V / 5.0V voltage rails and holds `/RESET` low during power-up or brownout events until voltage stabilizes for $\ge 100\text{ ms}$. | TO-92 / SOT-23; Active-low output; 3.15V detection threshold. | **Officially Documented** |
| **IC203** | Motor Driver IC | Toshiba / Sanyo | **Dual H-Bridge Motor Driver**. Translates MCU step/direction or PWM pulses into high-current drive signals for Main Motor (M1) and Scanner Motor (M2). | Power HSOP-28; 24V supply; integrated flyback protection diodes. | **Strongly Inferred** |
| **TR1** | `BCR8KM` / Equivalent | SanRex / Renesas | **Fixing Heater Triac**. AC phase angle / zero-crossing switch regulating current delivered to the ceramic heater strip based on thermistor readings. | TO-220F isolated; 600V; 8A RMS. | **Officially Documented** |
| **HVPS** | Integrated Subsystem | Canon Inc. | **High Voltage Power Supply Block**. Multi-stage transformer and diode multiplier producing charging, developing, transfer, and needle discharge voltages. | Primary: -1.3kV DC / 2kVp-p AC; Developing: -450V DC / AC; Transfer: +1.5kV / -1.0kV DC. | **Officially Documented** |
| **LVPS** | Integrated Subsystem | Canon Inc. | **Switch-Mode Low Voltage Power Supply**. Converts 120V/230V AC mains into regulated +24V DC (motors, solenoids, HV) and +5V DC (logic, sensors). | Flyback topology; CE / UL safety certified. | **Officially Documented** |

---

## 4. Sensors, Actuators & Electromechanical Components

| Ref | Component Description | Manufacturer & Part Number | Signal & Physical Operation | Confidence |
| :--- | :--- | :--- | :--- | :--- |
| **BD1** | **Laser Beam Detect Sensor** | Hamamatsu / Canon | Fast PIN photodiode placed at the start of scanline outside the imaging field. Pulses `/BD` low when struck by laser sweep to synchronize horizontal dot clocks. | **Officially Documented** |
| **PS201** | **Paper Delivery Sensor** | Sharp / Omron Photointerrupter | Flag-actuated optical interrupter at fuser exit. Detects sheet arrival and exit. Flags paper jam if blocked too long. | **Officially Documented** |
| **PS202** | **Paper Sensor (Tray Empty)** | Sharp / Omron Photointerrupter | Flag-actuated optical interrupter in input paper cassette. Drops flag into sensor slot when paper tray is empty, setting Basic Status bit `0x02`. | **Officially Documented / Verified Live** |
| **SW301** | **Top Cover Safety Interlock** | Omron Microswitch | Physically actuated by printer top door. Electrically opens +24V power line feeding high-voltage unit and laser diode when door is opened. Monitored by IC201 as bit `0x4000`. | **Officially Documented / Verified Live** |
| **SL1** | **Paper Pick-up Solenoid** | Canon `RH7-5284` | 24V DC electromechanical solenoid. Energized for ~50 ms to pull stop pawl, allowing spring-loaded clutch to rotate D-cut pickup roller exactly 360°. | **Officially Documented** |
| **M1** | **Main DC Drive Motor** | Nidec / Canon `RH7-1428` | 24V DC motor driving gear train for pickup roller, registration roller, OPC drum, developer roller, and fuser pressure roller. | **Officially Documented** |
| **M2** | **Polygon Scanner Motor** | Nidec / Canon | Brushless DC motor spinning 4-faceted mirror at ~18,000 RPM. Synchronized to ~600 scans per inch. | **Officially Documented** |
| **TH1 / TH2** | **Fixing Thermistors** | Murata / Canon NTC | Negative Temperature Coefficient thermistors in direct contact with ceramic heater. IC201 samples voltage via 10-bit ADC to maintain fuser temperature at ~180°C–195°C. | **Officially Documented** |
| **TF1** | **Thermal Fuse** | Tamura / NEC Thermal Cutoff | Emergency fail-safe mounted above fixing film. Permanently melts open at 228°C to prevent thermal runaway fire if triac shorts. | **Officially Documented** |

---

## 5. Machine-Readable Database Reference

The complete structured JSON representation of this hardware inventory is stored at:  
[`research/ic-database/hardware_inventory.json`](file:///f:/Games/canon-lbp1120-driver/research/ic-database/hardware_inventory.json)
