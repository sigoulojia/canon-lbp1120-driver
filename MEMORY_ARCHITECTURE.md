# MEMORY ARCHITECTURE & ADDRESS SPACE SPECIFICATION
## Canon LaserShot LBP-1120 System Memory Subsystems

**Document ID:** `MEM-ARCH-LBP1120-2026-10-31`  
**Classification:** Silicon Memory Architecture & Buffer Management  
**Scope:** Interface DRAM, On-Chip MCU Memories, EEPROM, and Host Memory Models

---

## 1. Overall System Memory Hierarchy

The Canon LaserShot LBP-1120 memory architecture is designed around extreme silicon economy, typical of early 2000s host-based "WinPrinters". Instead of possessing tens of megabytes of PostScript/PCL frame buffer memory, the printer possesses **only 512 KB of physical DRAM** on the interface board and **2 KB of fast SRAM** inside the engine microcontroller.

```mermaid
flowchart TD
    subgraph HostMem ["Host Memory Space (Windows x64 Driver Process)"]
        PdfBuf["WinRT Vector Direct2D Surface (~128 MB RGBA32)"]
        MonoBuf["Canonical 1-Bit Monochrome Frame Buffer (4,012,576 Bytes / 3.82 MB)"]
        ScoaBuf["Host SCoA Compressed Video Stream (80 KB - 220 KB Typical)"]
    end

    subgraph InterfaceMem ["Interface Controller Board (RG1-4305)"]
        DramFifo["512 KB DRAM Hardware Ring FIFO (M51V18165)<br/>- 0x00000 to 0x7FFFF (524,288 Bytes)<br/>- Safe Governor Threshold: 480,000 Bytes<br/>- Emergency Backpressure: Buffer Full Bit (0x04)"]
        Eeprom["1 Kbit / 2 Kbit Serial EEPROM (AT93C46 / 24C02)<br/>- USB Descriptors & Serial Number<br/>- Engine Alignment & Optical Calibration Constants"]
    end

    subgraph EngineMem ["Engine Controller MCU (Hitachi H8/300 HD6433292)"]
        McuRom["64 KB On-Chip Mask ROM (0x0000 - 0xDFFF)<br/>- Reset Vectors & Interrupt Handlers<br/>- Electrophotographic State Machine Logic<br/>- Fuser PID Temperature Curves & Timing Tables"]
        McuRam["2 KB Fast On-Chip Scratchpad SRAM (0xFB80 - 0xFF7F)<br/>- Stack Pointer (Top at 0xFF7E)<br/>- Sensor State Latches & Millisecond Timers"]
        McuIom["Memory-Mapped I/O Space (0xFF80 - 0xFFFF)<br/>- ITU, TMR, SCI, ADC, and Port Data Direction Registers"]
    end

    PdfBuf --> MonoBuf
    MonoBuf --> ScoaBuf
    ScoaBuf -->|USB Bulk OUT Chunks (4 KB)| DramFifo
    DramFifo -->|Scanline DMA| SCoADecompressor["ASIC SCoA Hardware Core"]
```

---

## 2. Interface Controller DRAM Ring FIFO (IC3)

### 2.1 Physical Characteristics
- **Component Ref:** `IC3`
- **Part Numbers:** `M51V18165` (OKI) / `IS42S16100` (ISSI)
- **Total Physical Capacity:** 524,288 Bytes (512 Kilobytes / 4 Megabits).
- **Organization:** 262,144 words $\times$ 16 bits.
- **Access Time:** $50\text{ ns} - 70\text{ ns}$.
- **Refresh Cycle:** CAS-before-RAS (CBR) auto-refresh managed transparently by the `NH4-5014` ASIC.

### 2.2 Ring FIFO Partitioning & Governor Backpressure

```text
Physical DRAM Address Map (512 KB Total)
+-------------------------------------------------------------+ 0x00000
|                                                             |
|           Active SCoA Compressed Video Ring Buffer          |
|                 (Capacity: 480,000 Bytes)                   |
|                                                             |
+-------------------------------------------------------------+ 0x75300 (480 KB)
|       Emergency Safety Margin / Backpressure Band (32 KB)   |
+-------------------------------------------------------------+ 0x7D000 (500 KB)
|    Line Cache Scratchpad (Current & Previous Reference)     |
|              (2 Lines x 592 Bytes = 1,184 Bytes)            |
+-------------------------------------------------------------+ 0x7FFFF (512 KB)
```

1. **Host-Side Governor Invariant:**
   - Single-page compressed SCoA streams transmitted prior to `IC_END_PAGE` must never exceed **480,000 bytes** (`LBP1120_SAFE_GOVERNOR_BYTES`).
   - If a complex synthetic image produces an SCoA stream exceeding 480 KB, the host driver automatically re-halftones using a coarser cluster screen to prevent hardware FIFO overrun.
2. **Hardware Backpressure Latch:**
   - When the write pointer approaches the read pointer within 32 KB, the ASIC asserts **Bit 2 (`0x04`)** of the Basic Status register (`CAPT_FL_BUFFERFULL`).
   - The host driver queries basic status and pauses transmission until the ASIC decompressor consumes scanlines during laser writing.

---

## 3. Serial Configuration EEPROM (IC4)

The serial EEPROM (`AT93C46` or `24C02`) retains non-volatile factory calibration and device identification parameters across power cycles:

| Word Offset | Byte Length | Stored Configuration Parameter | Value on Evaluated Unit |
| :--- | :--- | :--- | :--- |
| `0x00` | 2 Bytes | USB Vendor Identification (VID) | `0x04A9` (Canon Inc.) |
| `0x01` | 2 Bytes | USB Product Identification (PID) | `0x262B` (LaserShot LBP-1120) |
| `0x02` | 2 Bytes | Model Classification ID Word | `0x01A4` (420 Decimal) |
| `0x03` | 2 Bytes | Firmware Major & Minor Revision | `0x0110` (Release 1.10) |
| `0x04..0x0B` | 16 Bytes | USB Serial Number String (ASCII) | Factory unique manufacturing serial |
| `0x0C` | 2 Bytes | Left Margin Electrical Offset Calibration | Offset added to nominal `margin_left` |
| `0x0D` | 2 Bytes | Top Margin Pickup Solenoid Timing Trim | Milliseconds delay after registration trip |
| `0x0E` | 2 Bytes | Laser Power Calibration Trim (PWM DAC) | Factory laser intensity calibration word |
| `0x0F` | 2 Bytes | CRC-16 Header Checksum | Validates NVRAM block integrity at boot |

---

## 4. Engine Controller Microcontroller Memory Map (IC201)

The Hitachi H8/300 microcontroller operates in **Normal Expanded / Single-Chip Mode** with a flat 16-bit address space:

| Address Range | Size | Memory Type | Function / Description |
| :--- | :--- | :--- | :--- |
| `0x0000 - 0x003F` | 64 Bytes | Mask ROM | Interrupt & Exception Vector Table (31 Vectors $\times$ 2 Bytes). |
| `0x0040 - 0xDFFF` | 57,280 Bytes | Mask ROM | Core Engine Firmware: Electrophotographic sequence, motor PID, fuser thermistor curves, safety checks. |
| `0xE000 - 0xFB7F` | 7,040 Bytes | Reserved | Unmapped / Factory test memory aperture. |
| `0xFB80 - 0xFF7F` | 2,048 Bytes | High-Speed SRAM | Internal Zero-Wait-State Data Memory: Job state registers, error bitmasks, fuser temperature moving averages. |
| `0xFF7E - 0xFF7F` | 2 Bytes | SRAM | Hardware Stack Pointer (`SP` / `R7`) top initialization boundary. |
| `0xFF80 - 0xFFFF` | 128 Bytes | Memory-Mapped I/O | Internal Peripheral Registers: ITU0..ITU2 control, Port data direction (`P1DDR..P9DDR`), ADC data registers (`ADDRA..ADDRD`). |

---

## 5. Host-Side Canonical Frame Buffer Metrics

At 600 $\times$ 600 DPI on canonical A4 paper ($210\text{ mm} \times 297\text{ mm}$):

- **Horizontal Resolution:** $600\text{ dots/inch} = 23.622\text{ dots/mm}$.
- **Vertical Resolution:** $600\text{ lines/inch} = 23.622\text{ lines/mm}$.
- **A4 Physical Area:** $4,960\text{ dots wide} \times 7,015\text{ dots high}$.
- **Printable Image Area:**
  - `margin_left` = 112 dots ($\approx 4.74\text{ mm}$).
  - `margin_top` = 120 dots ($\approx 5.08\text{ mm}$).
  - `image_line_size` = 592 bytes = **4,736 pixels wide** ($\approx 200.48\text{ mm}$).
  - `image_lines` = **6,778 scanlines** ($\approx 286.93\text{ mm}$).
- **Uncompressed Frame Buffer Size:**
  $$\text{Total Bytes} = 592\text{ bytes/line} \times 6,778\text{ lines} = 4,012,576\text{ bytes} \approx 3.826\text{ MB}$$
- **SCoA Compression Ratio:**
  $$\text{Text Document}: \approx 109\text{ KB} \implies \text{Compression Ratio} \approx 36.7:1\text{ (97.3\% reduction)}$$
  $$\text{Complex Photograph}: \approx 210\text{ KB} \implies \text{Compression Ratio} \approx 19.1:1\text{ (94.8\% reduction)}$$
