# HARDWARE SIGNAL MAP & ELECTRICAL TIMING DIAGRAMS
## Canon LaserShot LBP-1120 Signal Analysis

**Document ID:** `SIG-MAP-LBP1120-2026-10-31`  
**Classification:** Electrical Signal Map & Timing Specifications  
**Confidence:** Officially Documented & Experimentally Verified

---

## 1. Complete System Signal Routing Map

```text
Host PC (WinUSB 64-bit Driver)
  │
  ├── [USB D+ / D-] (Full Speed 12 Mbps, Diff. ±400mV) ───────────┐
  │                                                               │
  ▼                                                               ▼
Interface Controller PCB (RG1-4305)                  Laser / Scanner Assembly
  │                                                               ▲
  ├── [/BD] (Beam Detect Pulse, Active Low, 50ns) ────────────────┤
  │                                                               │
  ├── [/VDO] (Video Dot Stream, ~37.88 MHz, NRZ) ─────────────────┘
  │
  ├── [SCLK] (Serial Clock, ~1 MHz, 0-3.3V Square Wave) ──────────┐
  │                                                               │
  ├── [/SC] (Command / Status Data, Half-Duplex Bi-directional) ──┤
  │                                                               │
  ├── [/PRINT] (Print Strobe, Active Low, ~100ms) ────────────────┤
  │                                                               │
  ├── [/PPRDY] (Printer Ready, Active Low, Open Collector) ───────┤
  │                                                               │
  ├── [/RESET] (Master Reset, Active Low, >100ms on brownout) ────┤
  │                                                               │
  ▼                                                               ▼
Engine Controller PCB (RG5-7264 / RG5-7266) <─────────────────────┘
  │
  ├── [SL1 Drive] (Pickup Solenoid, 24V Active-Low Pulse, ~50ms)
  ├── [Main Motor Run] (24V DC Brushless/Stepper Phase Drive)
  ├── [Scanner Motor Run & CLK] (24V DC, Frequency-Locked to 18,000 RPM)
  ├── [Fuser Triac Gate] (Zero-Crossing AC Pulse, Optical Triac Drive)
  ├── [High Voltage Biases] (Primary Charge, Developing, Transfer, Needle)
  └── [Sensor Feedback] (PS201 Delivery, PS202 Tray, SW301 Interlock)
```

---

## 2. Electrical Signal Characteristics

| Signal Name | Source | Destination | Nominal Voltage | Logic High (`VIH`) | Logic Low (`VIL`) | Timing / Frequency |
| :--- | :--- | :--- | :---: | :---: | :---: | :--- |
| **`USB D+ / D-`** | Host / IC2 | IC2 / Host | 3.3V Diff | $> 2.8\text{V}$ | $< 0.3\text{V}$ | 12.0 Mbps $\pm 0.25\%$. |
| **`/BD`** | Laser BD1 | Formatter IC1 | 3.3V | $> 2.4\text{V}$ | $< 0.4\text{V}$ | $885\ \mu\text{s}$ period; $50\text{ ns}$ active-low pulse width. |
| **`/VDO`** | Formatter IC1 | Laser Diode | 3.3V | $> 2.4\text{V}$ | $< 0.4\text{V}$ | $37.88\text{ MHz}$ bit clock ($26.4\text{ ns}$ per dot cell). |
| **`SCLK`** | Formatter IC1 | Engine IC201 | 3.3V | $> 2.4\text{V}$ | $< 0.4\text{V}$ | $500\text{ kHz} - 1.0\text{ MHz}$ 50% duty cycle. |
| **`/SC`** | Bi-directional | Bi-directional | 3.3V | $> 2.4\text{V}$ (Pull-up) | $< 0.4\text{V}$ | Half-duplex synchronous with SCLK. |
| **`/RESET`** | Engine IC202 | Formatter IC1 | 3.3V | $> 2.8\text{V}$ | $< 0.8\text{V}$ | Held low for $\ge 100\text{ ms}$ upon power-up. |
| **`/PRINT`** | Formatter IC1 | Engine IC201 | 3.3V | $> 2.4\text{V}$ | $< 0.4\text{V}$ | Held low for $\ge 10\text{ ms}$ to initiate sheet pickup. |
| **`/PPRDY`** | Engine IC201 | Formatter IC1 | 3.3V | $> 2.4\text{V}$ | $< 0.4\text{V}$ | Low indicates engine warm and polygon at speed. |

---

## 3. High-Speed Scanline Optical Timing

```text
Polygon Facet Enters Scan Window
      │
      ▼
[/BD] ──┐      ┌──────────────────────────────────────────────────────────
        └──────┘ (50ns low pulse)
        |<-- margin_left delay -->|<------------ Active Video Data Stream ------------>|
        |    (112 dot clocks)     |                 (4,736 dot clocks)                  |
        |      (~2.95 µs)         |                    (~125 µs)                        |
[/VDO] ───────────────────────────█████_█_████___██_████████____██████████████──────────
                                  Pixel 0                                     Pixel 4735
```

1. **Optical Sync:** The rotating mirror facet reflects the laser beam past the PIN photodiode `BD1` at the left edge of the optical assembly, driving `/BD` low.
2. **Left Margin Window:** The ASIC's hardware counter holds video output inactive for exactly `112` dot clocks ($\approx 2.95\ \mu\text{s}$), creating a crisp, unprinted $4.74\text{ mm}$ physical margin.
3. **Active Video Scanline:** Over the subsequent $125\ \mu\text{s}$, 4,736 pixels (592 bytes) are modulated onto `/VDO`.
4. **Scanline Dead Time:** Over the remaining $\approx 757\ \mu\text{s}$ of the scanline period, the laser is extinguished as the polygon facet rotates to the next mirror facet.
