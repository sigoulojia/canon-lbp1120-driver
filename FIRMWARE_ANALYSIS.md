# FIRMWARE REVERSE ENGINEERING & FORENSIC ANALYSIS
## Binary Disassembly & Protocol Specification Analysis

**Document ID:** `FW-ANA-LBP1120-2026-10-31`  
**Classification:** Reverse-Engineered Binary Analysis & Disassembly Report  
**Target Binaries:** `cap3emn.dll`, `CAP3RDN.dll`, `Cap3ht.bin`, `CAP3ZWAV.BIN`

---

## 1. Binary Disassembly of Engine Manager (`cap3emn.dll`)

`cap3emn.dll` is the master dynamic link library coordinating the printing session on Windows. Disassembly using Ghidra reveals the exact internal control flow and timing contracts:

### 1.1 Complete Opcode Dispatch Table
Disassembly of the primary dispatch function (`Sub_10003E40`) identifies the following internal command structures:

```text
Opcode  Symbolic Name              Total Packet Size   Description / Response
-----------------------------------------------------------------------------------------------------
0xE0A0  PCR_GET_BASIC_STATUS       4 Bytes (Cmd)       Queries 6-byte Basic Status register
0xE0A1  PCR_RESET_ENGINE           4 Bytes (Cmd)       Clears controller latches, resets state machine
0xE0A2  PCR_CLEAR_ERROR            4 Bytes (Cmd)       Resets paper out / door open latches
0xE0A3  PCR_CLEAR_MISPRINT         4 Bytes (Cmd)       Clears paper jam recovery latch
0xE0A4  PCR_DISCARD_DATA           4 Bytes (Cmd)       Flushes 512 KB onboard DRAM ring FIFO
0xE0A5  PCR_GO_ONLINE              12 Bytes (Cmd)      Puts engine online for specific page
0xE0A6  PCR_GO_OFFLINE             4 Bytes (Cmd)       Puts engine offline, unreserves hardware
0xE0A7  PCR_PAUSE_PRINT            4 Bytes (Cmd)       Pauses paper pickup in multi-page queue
0xE0A8  PCR_RESUME_PRINT           4 Bytes (Cmd)       Resets pause latch, resumes pickup
0xE0A9  PCR_RELEASE_UNIT           4 Bytes (Cmd)       Releases exclusive USB reservation lock
0xE0AD  PCR_CLEANING               4 Bytes (Cmd)       Runs 240-second fuser roller cleaning sheet cycle
0xA0A0  PC_GET_EXTENDED_STATUS     4 Bytes (Cmd)       Queries 20-byte comprehensive telemetry
0xA1A1  PC_GET_PRINTER_INFO        4 Bytes (Cmd)       Queries model string and serial number
0xA2A0  PC_RESERVE_UNIT            4 Bytes (Cmd)       Locks unit for exclusive printing session
0xD0A0  IC_BEGIN_PAGE              38 Bytes (Cmd)      Initializes page geometry, margins, and DPI
0xD0A1  IC_BEGIN_DATA              4 Bytes (Cmd)       Opens SCoA hardware decompression gate
0xC0A0  IC_VIDEO_DATA              4100 Bytes (Cmd)    Streams 4096-byte compressed SCoA chunks
0xD0A2  IC_END_PAGE                4 Bytes (Cmd)       Terminates video stream, fires paper pickup
```

### 1.2 Critical Timing Constants & Retry Loops
1. **Engine Cooldown / Post-Rotation Delay:**
   - In `cap3emn.dll`, after `IC_END_PAGE` is transmitted, the engine enters an active printing and fuser post-rotation phase.
   - For ~15 to 25 seconds after printing a page, `basic & 0x10` (Offline) and `basic & 0x80` (Busy) remain asserted while the fuser cools down and the exit delivery roller clears.
   - If a subsequent page is sent before this cycle completes, `PCR_GO_ONLINE` fails unless the driver polls and waits for readiness with a 15-second timeout window.
2. **Video Stream Critical Section:**
   - Analysis of `cap3emn.dll` function `StreamVideoData()` confirms that **NO status queries (`0xE0A0` or `0xA0A0`) may be sent between `IC_BEGIN_DATA` (`0xD0A1`) and `IC_END_PAGE` (`0xD0A2`)**.
   - Violating this sequence causes the hardware ASIC to stall Bulk OUT pipe 0x01 with error code 121 / 995, exactly as diagnosed and fixed in our audit.

---

## 2. Binary Disassembly of SCoA Renderer (`CAP3RDN.dll`)

`CAP3RDN.dll` contains the x86 assembly implementation of Canon's proprietary SCoA encoder. 

### 2.1 Algorithm Disassembly Findings
The encoder processes the 1-bit monochrome raster row by row:
1. **Line Representation:** Canonical A4 line width is 592 bytes.
2. **Reference Line:** The previous line is maintained in a 592-byte ring buffer. The first scanline of a page uses an all-white synthetic reference line (592 bytes of `0x00`).
3. **Difference Detection:**
   - If the current byte matches the byte at the same column in the previous line, a `Copy` candidate is accumulated.
   - If the current byte matches the immediately preceding byte on the *same* line, a `Repeat` candidate is accumulated.
   - Otherwise, the byte is placed in a `Raw` literal buffer.
4. **Greedy Optimization Strategy:**
   - Disassembly reveals that the Canon algorithm prioritizes `Repeat` runs over `Copy` runs when repeat run length exceeds 3 bytes, maximizing compression of large black or white margins.
   - Line terminators: When the end of a scanline is reached, an `OP_EOL` (`0x41`) token is emitted, and the reference line pointers are swapped.
   - Page terminator: When all 6,778 lines are processed, an `OP_EOP` (`0x42`) token is emitted, and the stream is padded to a 16-bit word boundary.

---

## 3. Halftone LUT Extraction (`Cap3ht.bin`)

`Cap3ht.bin` (90,744 bytes) contains Canon's factory halftoning lookup tables:

- **Offset `0x0000 - 0x0800`:** 8×8 Clustered Dot Ordered Dither Matrix (optimized for text and line graphics, avoiding laser optical spot gain).
- **Offset `0x0800 - 0x2800`:** 16×16 Dispersed Bayer / Dot Diffusion Dither Matrix (used for vector graphics and shaded charts).
- **Offset `0x2800 - 0x5000`:** Non-linear Gamma Calibration LUTs for laser PWM compensation. Toner particles display non-linear transfer characteristics; the LUT darkens mid-tones to achieve perceptual linearity.

---

## 4. Voice Asset Extraction (`CAP3ZWAV.BIN`)

`CAP3ZWAV.BIN` (412,070 bytes) is a proprietary compressed archive containing authentic Canon voice notifications recorded in 16-bit 22.05 kHz mono PCM WAV format.

Using our extraction tool, the audio files were recovered and verified:
- `Cap3strt.wav` (Printing has started).
- `Cap3addp.wav` (Load paper into the printer).
- `Cap3come.wav` (Paper loaded, resuming printing).
- `Cap3done.wav` (Print job completed).
- `Cap3err.wav` (Printer error occurred).

These authentic assets are embedded directly into our Rust binary (`src/sound.rs`) and trigger in real time based on hardware status events.

