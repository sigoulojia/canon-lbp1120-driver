# CANON CUSTOM ASIC REVERSE ENGINEERING & SILICON ANALYSIS
## Interface Controller ASIC (`NH4-5014-000` / IC1)

**Document ID:** `ASIC-LBP1120-2026-10-31`  
**Classification:** Proprietary Silicon Reverse Engineering  
**Target ASIC:** Canon Custom Formatter ASIC (Part: `NH4-5014-000`, Ref: `IC1`)

---

## 1. ASIC Architecture & Subsystem Block Diagram

The Canon Formatter ASIC (`IC1`) is a proprietary CMOS application-specific integrated circuit engineered to offload raster decompression, image enhancement, and microsecond-accurate laser pulse modulation from both the host computer and the engine microcontroller.

```mermaid
flowchart TD
    subgraph ASIC ["Canon Custom Formatter ASIC (NH4-5014 / IC1)"]
        subgraph USB_IF ["Host Data Ingress"]
            FifoRx["USB Bulk FIFO Receiver (64-byte chunks)"]
            PacketParser["CAPT Packet Header Parser (Opcode & Size)"]
        end

        subgraph MemoryArbiter ["DRAM Controller & DMA Arbiter"]
            DmaEngine["Dual-Port DMA Controller"]
            RefreshGen["DRAM Auto-Refresh Counter (CAS-before-RAS)"]
            WatermarkCtrl["FIFO Fullness Watermark / Backpressure Logic"]
        end

        subgraph ScoaCore ["SCoA Hardware Decompression Engine"]
            TokenDecoder["SCoA Token & Opcode Decoder (Copy, Repeat, Raw)"]
            PrevLineCache["592-Byte Previous Scanline Cache (Reference Line)"]
            CurrLineAccum["592-Byte Current Scanline Accumulator"]
            WordAligner["16-bit Word Boundary Aligner"]
        end

        subgraph ImageProc ["Image Enhancement Pipeline"]
            AirEngine["AIR (Automatic Image Refinement) 9-Pixel Edge Filter"]
            PwmModulator["Laser PWM Pulse Width Modulator (Quarter-Dot Slicing)"]
            TonerDensityLut["Toner Density / Gamma Modulation Matrix"]
        end

        subgraph VideoOut ["Real-Time Video Interface"]
            BdSync["/BD (Beam Detect) Phase-Locked Synchronizer"]
            MarginDelay["Margin Delay Counter (margin_left = 112 dots)"]
            ShiftReg["High-Speed Parallel-to-Serial Dot Shift Register"]
            LaserDriverOut["Laser Modulation Output Driver (/VDO)"]
        end
    end

    FifoRx --> PacketParser
    PacketParser --> DmaEngine
    DmaEngine <-->|16-Bit Memory Bus| ExtDram["External 512 KB DRAM (IC3)"]
    DmaEngine --> WatermarkCtrl
    WatermarkCtrl -->|Assert basic bit 2 (0x04) Buffer Full| StatusOut["Bulk IN Status Latch"]

    DmaEngine --> TokenDecoder
    TokenDecoder --> PrevLineCache
    TokenDecoder --> CurrLineAccum
    PrevLineCache <--> CurrLineAccum
    CurrLineAccum --> WordAligner
    WordAligner --> AirEngine

    AirEngine --> PwmModulator
    TonerDensityLut --> PwmModulator
    PwmModulator --> ShiftReg

    ExtBdInput["/BD Optical Input (from BD1 photodiode)"] --> BdSync
    BdSync --> MarginDelay
    MarginDelay --> ShiftReg
    ShiftReg --> LaserDriverOut
    LaserDriverOut --> ExtLaser["Engine Laser Diode Driver (/VDO)"]
```

---

## 2. Hardware SCoA Decompression Engine

The most critical intellectual property inside the `NH4-5014` ASIC is the hardware decompressor for Canon's proprietary **SCoA (Smart Compression Architecture)**.

### 2.1 Silicon Token Decoding Matrix
The hardware decompressor parses a continuous byte stream into variable-length commands using bit-field masking on the opcode byte:

| Command Type | Bit Pattern (`D7..D0`) | Silicon Decoding Logic & Hardware Action |
| :--- | :--- | :--- |
| **`RepeatX`** | `1 1 c c c 0 0 0` | If `count == 0`, opcode byte `0x43` indicates single-byte repeat. Repeats previous byte `c` times (1 to 7). |
| **`RepeatX Long`** | `0 0 0 0 0 c c c` | Preceded by length extension byte `(count >> 3)`. Repeats previous byte `8..255` times. |
| **`Repeat then Raw`** | `1 1 c c c r r r` | Repeats byte at index `c` times (2 to 7), followed by streaming `r` raw bytes directly from payload. |
| **`Repeat then Raw Long`** | `0 1 c c c r r r` | Preceded by length extension byte for raw count. Repeats `c` times, then writes `8..255` raw bytes. |
| **`Copy then Repeat`** | `0 1 c c c r r r` | Copies `c` bytes (0 to 7) identically from Previous Line Cache, then writes `r` repeated bytes. |
| **`Copy then Repeat Long`**| `1 0 c c c r r r` | Preceded by length extension byte for repeat count. Copies `c` bytes, then repeats byte `8..255` times. |
| **`Copy then Raw`** | `0 0 c c c r r r` | Copies `c` bytes (0 to 7) from Previous Line Cache, then streams `r` raw bytes from payload. |
| **`Copy then Raw Long`** | `1 0 c c c r r r` | Preceded by length extension byte for raw count. Copies `c` bytes, then streams `8..255` raw bytes. |
| **`OP_NOP` (`0x40`)** | `0 1 0 0 0 0 0 0` | No-operation; padding byte. |
| **`OP_EOL` (`0x41`)** | `0 1 0 0 0 0 0 1` | **End of Line**: Commits `CurrLineAccum` to `PrevLineCache`, resets scanline column counter to 0. |
| **`OP_EOP` (`0x42`)** | `0 1 0 0 0 0 1 0` | **End of Page**: Hardware terminates decompression, asserts page ready to engine controller. |

### 2.2 Word Alignment & Buffer Invariants
1. **16-Bit Word Alignment:** SCoA streams in CAPT 1.0 are strictly word-aligned. If the total byte count of encoded tokens for a page is odd, a trailing zero pad byte (`0x00`) is appended before the final packet is framed. The ASIC asserts an encoding error if a packet boundary truncates a multi-byte token.
2. **Deterministic Scanline Width:** For canonical A4 @ 600 DPI, each scanline must reconstruct exactly 592 bytes (4,736 pixels). If an `OP_EOL` occurs before byte 592 is populated, the ASIC automatically zero-fills the remainder of the scanline with white background pixels (`0x00`).

---

## 3. AIR (Automatic Image Refinement) Smoothing Unit

Canon's patented **Automatic Image Refinement (AIR)** is implemented entirely in dedicated combinational logic inside the ASIC:

- **Input Window:** $3 \times 3$ pixel sliding window sampled across three consecutive raster lines (Line $N-1$, Line $N$, Line $N+1$).
- **Algorithm:** Detects isolated staircase patterns along diagonal vector lines, curved font glyphs, and fine serifs.
- **Pulse Width Modulation (Quarter-Dot Slicing):**
  - Standard 600 DPI laser dot: 100% full-width laser emission pulse ($\approx 26.4\text{ ns}$).
  - AIR Enhanced dot: Modulates the laser turn-on and turn-off edge timing to produce $1/4$, $1/2$, or $3/4$ fractional dots positioned on the left, center, or right of the 600 DPI cell.
  - Perceived Result: Smooth curves equivalent to 1200 DPI horizontal resolution without increasing memory footprint.
- **Hardware Control:** Controlled by byte index 18 of the `IC_BEGIN_PAGE` (`0xD0A0`) packet:
  - `0x02`: AIR hardware smoothing enabled.
  - `0x00`: Direct 1:1 binary 600 DPI reproduction (preferred for dithered photographs and halftone screens).

---

## 4. Video Shift Register & Beam Detect (`/BD`) Synchronization

Real-time video timing is strictly slave to the physical rotating polygon mirror:

1. As the 4-faceted mirror spins at 18,000 RPM, the laser beam sweeps horizontally across the photodiode sensor `BD1` located outside the paper margin.
2. `BD1` generates a sharp negative pulse on the `/BD` input pin of the ASIC (pulse width $\approx 50\text{ ns}$).
3. The ASIC's internal hardware counter starts counting dot clock cycles ($\approx 37.88\text{ MHz}$ for 600 DPI @ 10 PPM).
4. After a programmable delay defined by `margin_left` (default `112` dot clocks $\approx 4.74\text{ mm}$ of physical paper margin), the ASIC begins shifting out the decompressed bits of scanline $N$ from its internal shift register onto `/VDO`.
5. The laser diode emits light for `1` bits (toner attracts to exposed areas on the photoconductive drum) and turns off for `0` bits (white paper background).

