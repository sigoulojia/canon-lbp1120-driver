# USB TRANSACTION & PROTOCOL REGRESSION CORPUS
## Canon LaserShot LBP-1120 Packet-by-Packet Protocol Specification

**Document ID:** `USB-CORPUS-LBP1120-2026-10-31`  
**Classification:** Low-Level USB Transaction Reference & Regression Corpus  
**Target Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`)

---

## 1. Complete Packet Framing Format

Every CAPT 1.0 transaction over Bulk OUT (`0x01`) and Bulk IN (`0x82`) consists of a 4-byte Little-Endian header followed by payload:

```text
Offset  Type    Field        Description
------  ------  -----------  --------------------------------------------------------------
0x00    u16 LE  opcode       Command or response identifier (e.g. 0xD0A0, 0xE0A0, 0xC0A0)
0x02    u16 LE  total_size   Total packet length INCLUDING this 4-byte header (>= 4)
0x04..  u8[]    payload      Variable-length payload (total_size - 4 bytes)
```

---

## 2. Canonical Transaction Corpus

### 2.1 Transaction 1: Control Endpoint 0 Device ID Query
- **Direction:** Host $\rightarrow$ Device (Control EP0)
- **Setup Packet:** `bmRequestType = 0xA1`, `bRequest = 0x00`, `wValue = 0x0000`, `wIndex = 0x0000`, `wLength = 0x0100`
- **Device Return (IEEE 1284 String, Preceded by 2-byte Big-Endian Length):**
  ```text
  00 48 4D 46 47 3A 43 61 6E 6F 6E 3B 4D 44 4C 3A   .HMFG:Canon;MDL:
  4C 41 53 45 52 20 53 48 4F 54 20 4C 42 50 2D 31   LASER SHOT LBP-1
  31 32 30 3B 43 4D 44 3A 43 41 50 54 3B 56 45 52   120;CMD:CAPT;VER
  3A 31 2E 30 3B 43 4C 53 3A 50 52 49 4E 54 45 52   :1.0;CLS:PRINTER
  3B 44 45 53 3A 43 61 6E 6F 6E 20 4C 41 53 45 52   ;DES:Canon LASER
  20 53 48 4F 54 20 4C 42 50 2D 31 31 32 30         SHOT LBP-1120
  ```

### 2.2 Transaction 2: Basic Status Query (`PCR_GET_BASIC_STATUS`)
- **Host Query (4 Bytes to Pipe 0x01):**
  ```text
  A0 E0 04 00
  [Opcode: 0xE0A0] [Size: 4]
  ```
- **Device Response (6 Bytes from Pipe 0x82):**
  ```text
  A0 E0 06 00 50 00
  [Opcode: 0xE0A0] [Size: 6] [basic: 0x50] [changed: 0x00]
  ```
  *(0x50 = UnitFree 0x40 | Offline 0x10)*

### 2.3 Transaction 3: Extended Status Query (`PC_GET_EXTENDED_STATUS`)
- **Host Query (4 Bytes to Pipe 0x01):**
  ```text
  A0 A0 04 00
  [Opcode: 0xA0A0] [Size: 4]
  ```
- **Device Response (20 Bytes from Pipe 0x82):**
  ```text
  A0 A0 14 00 50 00 00 00 80 00 00 00 01 01 01 01 00 00 00 00
  [Opcode: 0xA0A0] [Size: 0x0014 (20B)]
  [basic: 0x50] [changed: 0x00]
  [aux: 0x00] [controller: 0x00]
  [paper_available_bits: 0x80 (Paper Loaded)] [reserved: 0x00]
  [engine_raw: 0x0000 (Cover Closed, Cartridge OK, Clear)]
  [printed_pages: 0x01] [printing_pages: 0x01] [shipped_pages: 0x01] [start_pages: 0x01]
  [padding: 00 00 00 00]
  ```

### 2.4 Transaction 4: Online Mode Activation (`PCR_GO_ONLINE`)
- **Host Command (12 Bytes to Pipe 0x01):**
  ```text
  A5 E0 0C 00 EE DB EA AD 00 00 00 00
  [Opcode: 0xE0A5] [Size: 12]
  [Magic: 0xADEADBEE (Little-Endian: EE DB EA AD)]
  [Page Index: 0x0000 (u16 LE)]
  [Reserved: 0x0000 (u16 LE)]
  ```
- **Device Response (6 Bytes from Pipe 0x82):**
  ```text
  A0 E0 06 00 00 10
  [Opcode: 0xE0A0] [Size: 6]
  [basic: 0x00 (Online=1, UnitFree=0)] [changed: 0x10 (Online Latch Flipped)]
  ```

### 2.5 Transaction 5: Page Geometry Configuration (`IC_BEGIN_PAGE`)
- **Host Command (38 Bytes to Pipe 0x01 — Canonical A4 @ 600 DPI):**
  ```text
  A0 D0 26 00                                      --> Opcode 0xD0A0, Size 38 (0x0026)
  00 00                                            --> [0..1] const 0
  A4 01                                            --> [2..3] TargetModel: 0x01A4 (420 dec)
  02                                               --> [4] PaperSize: 2 (A4)
  01 00 00                                         --> [5..7] const 1, slot 0, const 0
  1F 1F 1F 1F                                      --> [8..11] TonerDensity: 31 (4 bytes)
  00 11 03 01 01 01                                --> [12..17] mode 0, res 0x11 (600 DPI), consts
  02 00 00 00                                      --> [18..21] smooth_enable: 2, toner_saving: 0
  70 00                                            --> [22..23] margin_left: 112 (0x0070)
  78 00                                            --> [24..25] margin_top: 120 (0x0078)
  50 02                                            --> [26..27] image_line_size: 592 (0x0250)
  7A 1A                                            --> [28..29] image_lines: 6,778 (0x1A7A)
  60 13                                            --> [30..31] paper_width: 4,960 (0x1360)
  67 1B                                            --> [32..33] paper_height: 7,015 (0x1B67)
  ```
- **Device Response:** None (One-way command to ASIC).

### 2.6 Transaction 6: Decompression Gate Arming (`IC_BEGIN_DATA`)
- **Host Command (4 Bytes to Pipe 0x01):**
  ```text
  A1 D0 04 00
  [Opcode: 0xD0A1] [Size: 4]
  ```
- **Device Response:** None (ASIC enters Streaming DMA mode).

### 2.7 Transaction 7: Compressed SCoA Video Chunk (`IC_VIDEO_DATA`)
- **Host Command (4,100 Bytes to Pipe 0x01):**
  ```text
  A0 C0 04 10 [4,096 Bytes of SCoA Compressed Stream]
  [Opcode: 0xC0A0] [Size: 4,100 (0x1004)]
  ```
- **Device Response:** None (Direct DMA transfer into 512 KB DRAM).

### 2.8 Transaction 8: Page Termination & Pickup Trigger (`IC_END_PAGE`)
- **Host Command (4 Bytes to Pipe 0x01):**
  ```text
  A2 D0 04 00
  [Opcode: 0xD0A2] [Size: 4]
  ```
- **Device Action:** Closes SCoA hardware decompressor; asserts `/PRINT` low; fires pickup solenoid `SL1` for 50 ms.
- **Device Response:** None.

### 2.9 Transaction 9: Session Release (`PCR_GO_OFFLINE`)
- **Host Command (4 Bytes to Pipe 0x01):**
  ```text
  A6 E0 04 00
  [Opcode: 0xE0A6] [Size: 4]
  ```
- **Device Response (6 Bytes from Pipe 0x82):**
  ```text
  A0 E0 06 00 50 10
  [Opcode: 0xE0A0] [Size: 6] [basic: 0x50 (Offline=1, UnitFree=1)] [changed: 0x10]
  ```

