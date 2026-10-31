# MECHANICAL PRINTING LIFECYCLE & ENGINE TIMING
## Canon LaserShot LBP-1120 Mechanical State Sequence

**Document ID:** `MECH-LIFE-LBP1120-2026-10-31`  
**Classification:** Mechanical Operations & Timing Sequences  
**Verification:** Live Hardware Cycle Tracking (Physical Counter `0 -> 1`)

---

## 1. Engine Mechanical Operation Periods

The mechanical lifecycle of a print job proceeds through five distinct temporal periods governed by the engine microcontroller (IC201):

```mermaid
flowchart LR
    Standby["1. Standby Period<br/>(Motor stopped, Fuser cold/standby)"]
    Wait["2. Wait / Pre-Rotation Period<br/>(Main motor runs, Drum charges, Fuser ramps to 185°C)"]
    Print["3. Printing / Laser Exposure Period<br/>(Pickup solenoid fires, Laser scans, Transfer active)"]
    PostRotate["4. Post-Rotation Period<br/>(Fuser clearing, Delivery sensor trips, Drum cleaning)"]
    Cooling["5. Cooldown / Ready Period<br/>(Motor stops, Fuser maintains idle, Status -> Ready)"]

    Standby -->|Job Arrives / PCR_GO_ONLINE| Wait
    Wait -->|Engine Warmed & Polygon Locked| Print
    Print -->|Trailing Edge Clears Delivery Sensor| PostRotate
    PostRotate -->|Exit Path Clear (~15s)| Cooling
    Cooling --> Standby
```

### Period 1: Standby (STBY)
- The main drive motor and polygon scanner motor are completely stopped.
- The low-voltage power supply supplies +5V and +3.3V to the controllers; the +24V line to the motors and high-voltage supply is idle.
- Fixing heater ceramic element is unpowered; temperature resides at ambient or idle standby (~50°C).
- Basic Status byte returns `0x40` (Unit Free, Offline).

### Period 2: Wait / Pre-Rotation (WAIT / PRERO)
- Initiated upon receipt of `PCR_GO_ONLINE` (`0xE0A5`) from the host driver.
- The Engine MCU (IC201) engages the low-voltage relay and powers the +24V rail.
- Main Drive Motor `M1` accelerates to full operating speed; the gear train rotates the OPC drum, developing cylinder, and fixing pressure roller.
- Polygon Scanner Motor `M2` spins up, locking to 18,000 RPM within 1.5 seconds.
- Fixing Heater Triac `TR1` engages full-wave AC conduction, ramping the ceramic heater to the target printing temperature ($185^\circ\text{C}$).
- Primary Charge Roller and Developing Roller receive bias voltages to stabilize the electrostatic surface layer.
- Duration: Typically 3.5 to 6.0 seconds depending on initial fuser temperature.

### Period 3: Printing Period (PRINT)
- When the engine signals `/PPRDY` low, the Formatter asserts `/PRINT` low.
- **Pickup Phase:** The MCU pulses the Paper Pick-up Solenoid `SL1` for ~50 ms. The pickup clutch engages, turning the D-cut roller one complete revolution. The top sheet is drawn into the feed guide.
- **Registration Phase:** The leading edge of the paper strikes the registration shutter to square the page, then feeds between the registration roller pair.
- **Laser Writing Phase:** The optical beam detector `BD1` pulses `/BD` every 885 microseconds. The Formatter streams `/VDO` bits synchronously.
- **Transfer & Fusing:** The sheet passes through the transfer roller (+1.5 kV) and enters the SURF fixing unit.

### Period 4: Post-Rotation (POSTRO)
- The trailing edge of the sheet clears the photosensitive drum and enters the fixing sleeve nip.
- As the paper emerges, it actuates the Paper Delivery Sensor `PS201`.
- The main motor continues running to drive the sheet fully out onto the face-down delivery tray.
- Once `PS201` flag resets to the unblocked position, the MCU verifies the paper path is clear.
- The high-voltage generator turns off.
- The triac heater switches off.
- Duration: Approximately 15 to 20 seconds. During this entire time, the engine reports `CAPT_FL_BUSY` (`basic & 0x80`) to prevent thermal stress and paper bunching.

### Period 5: Cooldown / Return to Standby
- The main motor decelerates to a stop.
- The polygon scanner motor spins down.
- The printed page hardware counter is incremented in extended status.
- The engine enters healthy Standby, ready for the next job.

