# Canon LaserShot LBP-1120 — Complete CAPT 1.0 & SCoA Protocol Specification

> **Verification Basis**: Clean-room binary reverse engineering of `cap3emn.dll` (Engine Manager), `CAP3RDN.dll` (Host Rendering Engine), `CAP3LMK.DLL` (Language Monitor), `CAP3SMK.DLL` (Status Monitor), `Cap3ht.bin` (Halftone LUT & Dither Matrices), `Cap3setk.inf` (Driver Manifest), and verified USB Bulk/Control captures against physical Canon LaserShot LBP-1120 hardware (`04A9:262B`).

---

## 1. Hardware & USB Transport Topology

| Parameter | Verified Specification |
| :--- | :--- |
| **Vendor ID (VID)** | `0x04A9` (Canon Inc.) |
| **Product ID (PID)** | `0x262B` (Physical LBP-1120 USB enumeration) / `0x261A` (Alternate regional firmware) |
| **Target Model Word** | `0x01A4` (`420` decimal — embedded at bytes `2..3` of `IC_BEGIN_PAGE`) |
| **IEEE 1284 Device ID** | `MFG:Canon;MDL:LASER SHOT LBP-1120;CMD:CAPT;CLS:PRINTER;` |
| **Control Endpoint** | `0x00` — Class-specific requests `GET_DEVICE_ID` (`0xA1`, Req `0`) and `GET_PORT_STATUS` (`0xA1`, Req `1`) |
| **Bulk OUT Pipe** | `0x01` — MaxPacketSize `64` bytes; carries CAPT command packets and SCoA video chunks |
| **Bulk IN Pipe** | `0x82` — MaxPacketSize `64` bytes; carries 6-byte basic status and 20-byte extended status replies |
| **Onboard Printer RAM** | `512 KB` (`524,288` bytes) hardware ring FIFO; single-page compressed video streams must stay `<= 480,000` bytes to avoid FIFO stall prior to `IC_END_PAGE` |
| **Native Engine Resolution** | `600 × 600 DPI` (`0x11`) or `300 × 300 DPI` (`0x00`) |
| **Canonical A4 Raster** | `4,736 × 6,778` pixels (`592` bytes/line × `6,778` lines = `4,012,576` bytes uncompressed) |

---

## 2. Packet Framing Format

Every CAPT 1.0 packet over Bulk OUT (`0x01`) and Bulk IN (`0x82`) uses a 4-byte Little-Endian header:

```text
Offset  Type    Field        Description
------  ------  -----------  --------------------------------------------------------------
0x00    u16 LE  opcode       Command or response identifier (e.g., 0xD0A0, 0xC0A0, 0xE0A0)
0x02    u16 LE  total_size   Total packet byte length INCLUDING this 4-byte header (>= 4)
0x04..  u8[]    payload      Variable-length payload (total_size - 4 bytes)
```

---

## 3. Complete Opcode Table (Verified in `cap3emn.dll`)

### 3.1 Printer Control & Reset Commands (`PCR_*` — `0xE0xx`)
| Opcode | Symbolic Name | Payload | Direction | Function & Verified Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `0xE0A0` | `PCR_GET_BASIC_STATUS` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Queries 6-byte Basic & Changed status registers. |
| `0xE0A1` | `PCR_RESET_ENGINE` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Performs controller & fuser state reset. |
| `0xE0A2` | `PCR_CLEAR_ERROR` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Clears recoverable latch errors (paper empty, cover open). |
| `0xE0A3` | `PCR_CLEAR_MISPRINT` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Clears misprint / paper jam recovery condition after jam removal. |
| `0xE0A4` | `PCR_DISCARD_DATA` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Flushes onboard 512 KB video FIFO buffer. |
| `0xE0A5` | `PCR_GO_ONLINE` | `8 B` (Total 12B) | Host → Dev → Host (6B) | Places engine online for page `N`. Payload: `u32(0xADEADBEE)` + `u16(page_idx)` + `u16(0)`. |
| `0xE0A6` | `PCR_GO_OFFLINE` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Exits online print mode after job completion or error abort. |
| `0xE0A7` | `PCR_PAUSE_PRINT` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Pauses paper pickup before next sheet in multipage queue. |
| `0xE0A8` | `PCR_RESUME_PRINT` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Resumes paused print queue. |
| `0xE0A9` | `PCR_RELEASE_UNIT` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Releases exclusive host reservation acquired via `0xA2A0`. |
| `0xE0AD` | `PCR_CLEANING` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Initiates fuser roller cleaning sheet cycle. |

### 3.2 Printer Configuration & Extended Queries (`PC_*` — `0xAxxx`)
| Opcode | Symbolic Name | Payload | Direction | Function & Verified Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `0xA0A0` | `PC_GET_EXTENDED_STATUS` | `0 B` (Total 4B) | Host → Dev → Host (20B) | Returns 6B header + 14B extended engine/counter telemetry. |
| `0xA0A1` | `PC_GET_VERSION_INFO` | `0 B` (Total 4B) | Host → Dev → Host | Queries controller firmware revision. |
| `0xA0A3` | `PC_GET_USER_CONFIG` | `0 B` (Total 4B) | Host → Dev → Host | Queries NVRAM user configuration flags. |
| `0xA1A0` | `PC_GET_ENGINE_CAPABILITIES` | `0 B` (Total 4B) | Host → Dev → Host | Queries engine speed, DPI, and buffer size parameters. |
| `0xA1A1` | `PC_GET_PRINTER_INFO` | `0 B` (Total 4B) | Host → Dev → Host | Queries model descriptor and serial blocks. |
| `0xA2A0` | `PC_RESERVE_UNIT` | `0 B` (Total 4B) | Host → Dev → Host (6B) | Acquires exclusive session lock before starting job. |
| `0xA3A0` | `PC_SET_SLEEP_TIMER` | `2 B` (Total 6B) | Host → Dev → Host (6B) | Configures energy-saver standby timeout. |

### 3.3 Image Control & Video Data Streaming (`IC_*` — `0xD0xx` / `0xC0xx`)
| Opcode | Symbolic Name | Payload | Direction | Function & Verified Behavior |
| :--- | :--- | :--- | :--- | :--- |
| `0xD0A0` | `IC_BEGIN_PAGE` | `34 B` (Total 38B) | Host → Dev | Transmits 34-byte page geometry, margins, DPI, and toner parameters. |
| `0xD0A1` | `IC_BEGIN_DATA` | `0 B` (Total 4B) | Host → Dev | Opens hardware SCoA decompression gate for raster video stream. |
| `0xC0A0` | `IC_VIDEO_DATA` | `<= 4096 B` | Host → Dev | Streams SCoA compressed raster in 4,096-byte payload frames (4,100B total). |
| `0xD0A2` | `IC_END_PAGE` | `0 B` (Total 4B) | Host → Dev | Closes video stream and triggers paper pickup & laser imaging cycle. |

---

## 4. Status Register Bitfields

### 4.1 Basic Status Reply (`0xE0A0`, 6 Bytes)
- **Bytes `0..1`**: `0xA0, 0xE0` (`0xE0A0`)
- **Bytes `2..3`**: `0x06, 0x00` (`6` bytes)
- **Byte `4` (`basic` register)**:
  - `Bit 7 (0x80)`: `DEVICE_ERROR` / `CAPT_FL_BUSY` (Fatal fault or unready engine)
  - `Bit 6 (0x40)`: `UNIT_FREE` (`1` = Unreserved, `0` = Reserved by active job)
  - `Bit 4 (0x10)`: `OFFLINE` (`1` = Offline, `0` = Online & ready for page stream)
  - `Bit 3 (0x08)`: IEEE-1284 `nFault` compatibility bit (Reserved on USB; **must not** be treated as buffer full)
  - `Bit 2 (0x04)`: `BUFFER_FULL` (`CAPT_FL_BUFFERFULL` — backpressure active)
  - `Bit 1 (0x02)`: `NO_PAPER` (Tray sensor empty)
  - `Bit 0 (0x01)`: `ENGINE_ACTIVE` (Laser/fuser motor running)
- **Byte `5` (`changed` register)**: Latched state transition mask since last status query.

### 4.2 Extended Status Reply (`0xA0A0`, 20 Bytes)
Following the 6-byte header (`0xA0A0`, size `0x0014`, `basic`, `changed`), the 14-byte payload contains:
- **`payload[0]` (`aux`)**:
  - `0x04`: Paper feed solenoid active (sheet currently moving through pickup/fuser path)
  - `0x02`: Engine busy processing page
- **`payload[1]` (`controller`)**: Formatter state flags
- **`payload[2]` (`paper_available_bits`)**: `0x80` = Paper present in main input tray; `0x00` = Tray empty
- **`payload[4..5]` (`engine_raw`, `u16 LE`)**:
  - `0x4000`: Top door / toner cover open (`door_open`)
  - `0x2000`: Toner cartridge missing (`no_cartridge`)
  - `0x1000`: Fuser warming up / waiting (`waiting`)
  - `0x0200`: Paper out (`no_paper`)
  - `0x0100`: Paper jam in path (`paper_jam`)
  - `0x0004`: Roller cleaning active (`cleaning`)
  - `0x0002`: Service call / fuser hardware fault (`service_call`)
- **`payload[6..7]` (`start_pages`, `u16 LE`)**: Pages initiated by formatter
- **`payload[8..9]` (`printing_pages`, `u16 LE`)**: Pages currently in laser/paper path
- **`payload[10..11]` (`shipped_pages`, `u16 LE`)**: Pages physically delivered past exit sensor
- **`payload[12..13]` (`printed_pages`, `u16 LE`)**: Total confirmed page count

---

## 5. `IC_BEGIN_PAGE` (`0xD0A0`) 34-Byte Geometry Payload

```text
Byte(s)  Value (A4 Default)  Field Name        Description
-------  ------------------  ----------------  --------------------------------------------------
0..1     0x0000              reserved_0        Constant 0
2..3     0x01A4 (420)        target_model      Canon LBP-1120 ASIC model identifier
4        0x02                paper_size        1=Letter, 2=A4, 3=Legal, 4=Executive, 5=B5
5        0x01                media_type        1=Plain Paper, 2=Heavy, 3=Transparency, 4=Envelope
6        0x00                input_slot        0=Main Tray
7        0x00                reserved_7        Constant 0
8..11    0x1F (31) x 4       toner_density     4-zone laser PWM density (0..63, default 31)
12       0x00                color_mode        0=Monochrome
13       0x11                resolution        0x11=600 DPI, 0x00=300 DPI
14..17   0x03,0x01,0x01,0x01 engine_lut_flags  Constant [3, 1, 1, 1]
18       0x02 / 0x00         smooth_enable     2=AIR Image Refinement Smooth ON, 0=OFF
19       0x01 / 0x00         toner_saving      1=Economy Toner Save ON (~38% reduction), 0=OFF
20..21   0x0000              reserved_20       Constant 0
22..23   112 (u16 LE)        margin_left       Left hardware margin in pixels (4.74 mm @ 600 DPI)
24..25   120 (u16 LE)        margin_top        Top hardware margin in pixels (5.08 mm @ 600 DPI)
26..27   592 (u16 LE)        image_line_size   Bytes per scanline (592 B = 4,736 pixels)
28..29   6778 (u16 LE)       image_lines       Active scanlines per page
30..31   4960 (u16 LE)       paper_width       Physical page width in pixels (210 mm @ 600 DPI)
32..33   7015 (u16 LE)       paper_height      Physical page height in pixels (297 mm @ 600 DPI)
```

---

## 6. SCoA (Smart Compression Architecture) 2D Delta + RLE Codec

SCoA encodes each scanline relative to both the preceding byte in the current scanline (`Repeat`) and the vertically aligned byte in the immediately preceding scanline (`Copy`), falling back to literal bytes (`Raw`).

### 6.1 Control & Framing Opcodes
- `0x40` (`OP_NOP`): No-operation padding byte inserted before `OP_EOP` if needed so the total video stream is **16-bit word-aligned** (even byte length).
- `0x41` (`OP_EOL`): End-of-line copy — copies the remainder of the current scanline from `prev_line`.
- `0x42` (`OP_EOP`): End-of-page marker (must be the final byte of the 16-bit aligned stream).
- `0x43` (`REPEAT_X_BYTE`): Dedicated repeat byte constant used by `RepeatX` (`0xC0 | (count << 3)`) and `RepeatXLong`.

### 6.2 Command Byte Bit-Encoding Table
| High Bits | Command Type | Bit Layout | Additional Bytes |
| :--- | :--- | :--- | :--- |
| `00_rrr_ccc` | `CopyThenRaw` | `copy = c (0..7)`, `raw = r (1..7)` | `r` raw bytes |
| `01_rrr_ccc` | `CopyThenRepeat` | `copy = c (0..7)`, `rep = r (2..7)` | `1` repeat byte |
| `100_ccccc` | `CopyLong` | `copy = (c << 3)` (`8..248`, multiple of 8) | `0` bytes |
| `101_eeeee` | `Extend` Prefix | `ext = (e << 3)` (`8..248`) | Followed by 2nd command byte (`00`=RepeatXLong, `01`=RepeatThenRawLong, `10`=CopyThenRepeatLong, `11`=CopyThenRawLong) |
| `11_000_ccc` | `CopyShort` | `copy = c (1..7)` | `0` bytes |
| `11_rrr_000` | `RepeatX` | `rep = r (1..7)` of `0x43` | `0` bytes |
| `11_rrr_www` | `RepeatThenRaw` | `rep = r (2..7)`, `raw = w (1..7)` | `1` repeat byte + `w` raw bytes |

---

## 7. Reverse Engineering of `Cap3ht.bin` (90,744 Bytes)

Binary inspection of `assets/Cap3ht.bin` (identical across Windows XP and Windows Vista/7 driver packages) reveals two distinct regions:

1. **Precomputed Halftone Pattern Lookup Tables (`0x00000` .. `0x15FE3`, 90,084 bytes)**:
   - `0x0000`: `u32 LE = 4` (4 LUT sections)
   - Section 0 (`0x0024`, `9,648` bytes): 8×8 bit-packed density patterns (300 DPI Graphics)
   - Section 1 (`0x25D4`, `35,376` bytes): 16×16 bit-packed density patterns (600 DPI Gradation)
   - Section 2 (`0xB004`, `9,648` bytes): 8×8 bit-packed density patterns (300 DPI Text/Line)
   - Section 3 (`0xD5B4`, `35,376` bytes): 16×16 bit-packed density patterns (600 DPI Fine Halftone)
2. **Raw Threshold Dither Matrices (`0x15FE4` .. `0x16278`, 660 bytes)**:
   - `0x15FE4`: `u32 LE = 4` (4 matrices)
   - **Matrix 0 (`0x15FE8`, 64 bytes)**: `8 × 8` threshold screen (`min=1, max=62`), 45-degree dual-spiral cluster.
   - **Matrix 1 (`0x1602C`, 256 bytes)**: `16 × 16` supercell threshold screen (`min=1, max=254`), 106 LPI photographic gradation screen.
   - **Matrix 2 (`0x16130`, 64 bytes)**: `8 × 8` high-frequency threshold screen (`min=1, max=62`), 141 LPI text/graphics screen.
   - **Matrix 3 (`0x16174`, 256 bytes)**: `16 × 16` quad-cluster threshold screen (`min=1, max=254`), 141 LPI high-resolution detail screen.
