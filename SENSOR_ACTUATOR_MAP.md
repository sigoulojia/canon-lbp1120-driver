# SENSOR & ACTUATOR HARDWARE MAPPING
## Canon LaserShot LBP-1120 Electromechanical Devices

**Document ID:** `SENS-ACT-LBP1120-2026-10-31`  
**Classification:** Complete Hardware Sensor and Actuator Directory  
**Engine Reference:** Canon P160 Assembly

---

## 1. Physical Actuators (Motors, Solenoids, Lasers, Heaters)

| Ref | Actuator Name | Part Number | Operating Electrical Rating | Mechanical Subsystem Driven | Controlled By |
| :---: | :--- | :--- | :--- | :--- | :--- |
| **M1** | **Main Drive DC Motor** | Canon `RH7-1428` | +24.0V DC; 1.2A Peak | Drives main gear train: OPC drum, magnetic roller, registration rollers, and fuser pressure roller. | IC201 through IC203 (Motor Driver) |
| **M2** | **Polygon Scanner Motor** | Integrated in Laser Optics | +24.0V DC; Brushless 3-phase | Spins 4-faceted optical polygon mirror at 18,000 RPM ($\pm 0.1\%$ frequency lock). | IC201 via J202 (`MTR_ON`, `MTR_CLK`) |
| **LD1** | **Semiconductor Laser Diode** | Integrated in Laser Optics | +5.0V DC; 780 nm AlGaAs; 5 mW | Discharges drum surface along horizontal scanline to form electrostatic latent image. | Formatter ASIC IC1 via `/VDO` |
| **SL1** | **Paper Pick-up Solenoid** | Canon `RH7-5284` | +24.0V DC; $120\ \Omega$ coil | Disengages pickup roller latch pawl, allowing spring clutch to complete one $360^\circ$ rotation. | IC201 Port 3 via power transistor |
| **H1** | **SURF Ceramic Fixing Heater** | Integrated in Fixing Film Unit | 120V / 230V AC mains; ~600W peak | Instantaneous thermal heating of polyimide fixing film to melt toner particles into paper. | IC201 via Triac switch TR1 (`BCR8KM`) |
| **LED1** | **Front Panel Status LED** | Display PCB | +3.3V DC; Dual-Color (Green / Amber) | Visual printer status: Solid Green = Ready; Blinking Green = Printing; Blinking Amber = Paper out / Jam. | Formatter ASIC / MCU |

---

## 2. Physical Sensors & Feedback Transducers

| Ref | Transducer Name | Type | Location | Monitored Parameter & Action |
| :---: | :--- | :--- | :--- | :--- |
| **BD1** | **Beam Detect Sensor** | High-speed PIN Photodiode | Optical scanner assembly start-of-scan | Detects laser sweep arrival; asserts `/BD` low pulse to synchronize horizontal dot shift clock. |
| **PS201** | **Paper Delivery Sensor** | Optical Photointerrupter with swinging plastic flag | Paper exit nip behind fixing assembly | Monitors sheet arrival and trailing edge exit; detects paper jams in fuser unit. |
| **PS202** | **Paper In-Tray Sensor** | Optical Photointerrupter with swinging plastic feeler | Multi-purpose tray floor | Feeler drops into slot when paper is depleted, asserting Basic Status bit 1 (`0x02` No Paper). |
| **SW301** | **Top Cover Interlock Switch** | SPST Heavy-Duty Push Microswitch | Front-top right cartridge door frame | Physically interrupts the +24V power line to the laser diode and high-voltage generator when opened. |
| **TH1** | **Main Fixing Thermistor** | Negative Temperature Coefficient (NTC) resistor | Centered beneath ceramic heater strip | Primary analog feedback for fuser PID temperature loop ($25^\circ\text{C} \approx 300\text{ k}\Omega$; $185^\circ\text{C} \approx 5\text{ k}\Omega$). |
| **TH2** | **Sub-Thermistor** | NTC Thermistor | Edge of ceramic heater strip | Monitors thermal gradient across fixing sleeve to prevent edge overheating on narrow media (B5/Executive). |
| **TF1** | **Thermal Cutoff (Thermal Fuse)** | Bimetal fusible alloy link | Affixed above ceramic heater substrate | Secondary physical emergency safety: Permanently opens AC mains circuit at $228^\circ\text{C}$ on runaway. |
| **ANT1**| **Toner Level Antenna** | Conductive metal capacitive probe | Cartridge mounting rail | Senses magnetic permeability of remaining iron-oxide toner inside the EP-22 hopper. |
