# Canon LaserShot LBP-1120 Modern Driver & Spooler Suite

A modern, high-performance 64-bit Windows 10/11 native driver and spooler daemon for the **Canon LaserShot LBP-1120** USB laser printer, reverse-engineered and written in Rust.

---

## 🌟 Overview

The Canon LaserShot LBP-1120 (USB VID: `0x04A9`, PID: `0x261A`) is a legacy GDI host-based laser printer running Canon's proprietary **CAPT v1** (Canon Advanced Printing Technology) protocol with **SCoA** (Smart Compression Architecture) raster streaming.

Official driver support ended with 32-bit Windows XP/Vista. This project provides a production-grade, 64-bit native replacement for Windows 10 and Windows 11 with zero legacy dependencies, featuring:

- **Dual-Mode Spooler Interception** (Standard TCP Raw Port 9100 + Overlapped Named Pipes).
- **Native PDF Rasterization** via Windows Runtime (`Windows.Data.Pdf`) at crisp 600 DPI.
- **Dual Halftone Engines** (Normal fast thresholding for documents & Floyd-Steinberg error diffusion for photos).
- **Bit-Accurate SCoA Compression** with 16-bit word alignment and hardware chunk framing.
- **Bidirectional WinUSB Transport** with flow control, backpressure polling, and error recovery.
- **Authentic Canon Voice Alerts** (playback of original CAPT status WAV audio notifications).
- **Background Detached Service** running silently in user-space or as a Windows daemon.

---

## 📂 Repository Structure

```
├── Cargo.toml            # Rust package manifest & dependencies
├── Cargo.lock            # Locked crate dependencies
├── src/                  # Core driver source code
│   ├── main.rs           # CLI interface & command router
│   ├── usb.rs            # WinUSB device communication & flow control
│   ├── spool.rs          # Spooler server (TCP 9100 & Named Pipe listener)
│   ├── pdf.rs            # WinRT 600 DPI PDF rasterizer & halftoning
│   ├── scoa.rs           # SCoA compression RLE encoder & decoder
│   ├── protocol.rs       # CAPT packet structures & opcode serialization
│   ├── sound.rs          # Authentic Canon audio alert engine
│   ├── pbm.rs            # 1-bit monochrome P4 Netpbm parser
│   └── a4.rs             # A4 page geometry constants & generators
├── assets/               # Binary assets extracted from Canon firmware
│   ├── Cap3ht.bin        # Canon official 600 DPI halftone dither matrix
│   └── sounds/           # Original Canon audio notification WAV files
├── scripts/              # PowerShell diagnostic, reverse-engineering & test scripts
│   ├── run_service.ps1       # Launch background spooler daemon
│   ├── send_test_job.ps1     # Dispatch print jobs to spooler queues
│   ├── check_job_status.ps1  # Real-time spooler & print queue diagnostics
│   ├── test_tcp_print.ps1    # Test raw TCP 9100 printing pipeline
│   ├── compare_drivers.ps1   # Compare original Canon output vs modern driver
│   ├── dump_exports.ps1      # PE export table analyzer for CAP3K.dll
│   ├── extract_wavs.ps1      # WAV audio extractor from CAP3ZWAV.BIN
│   └── ...                   # Additional reverse engineering utilities
├── fixtures/             # Sample test files & test print jobs
│   ├── test-3-pages.pdf  # Multi-page PDF test document
│   ├── test-3-pages.html # Multi-page HTML source
│   └── diag_test.txt     # Plaintext test print payload
└── reference/            # Reference materials & extracted original driver files
    ├── drivers/
    │   ├── LBP1120_WinXP/    # Original 32-bit Windows XP INF, DLLs, and drivers
    │   └── extracted/        # Vista 32-bit driver package components
    └── installers/           # Official Canon driver setup executables
```

---

## 🚀 Quick Start

### 1. Prerequisites

- **Windows 10 / 11** (x64)
- **Rust Toolchain**: 1.80+ (`rustup default stable`)
- **USB Driver**: Ensure WinUSB is installed for the printer device (VID `0x04A9`, PID `0x261A`) using [Zadig](https://zadig.akeo.ie/).

### 2. Build

```powershell
# Build optimized release binary
cargo build --release

# Run the 64 automated unit & integration tests
cargo test
```

### 3. Usage & Commands

```powershell
# Probe the printer USB connection and query hardware status
.\target\release\capt-poc.exe probe

# Install the dual-mode Windows print queues (Normal & Photo)
.\target\release\capt-poc.exe install

# Run the background spooler service
.\target\release\capt-poc.exe service

# Or run foreground server to watch real-time print logs:
.\target\release\capt-poc.exe spool-server

# Print a PDF directly from CLI
.\target\release\capt-poc.exe print-pdf "fixtures\test-3-pages.pdf" normal
.\target\release\capt-poc.exe print-pdf "fixtures\test-3-pages.pdf" photo

# Compare halftone algorithms on a specific page
.\target\release\capt-poc.exe compare-halftone "fixtures\test-3-pages.pdf" 0 "preview"
```

---

## ⚙️ Architecture & Technical Highlights

### 1. Spooler Interception Pipeline
- **Raw TCP Port 9100**: Emulates a JetDirect network print server on `127.0.0.1:9100`. Any standard Windows PostScript/PDF driver pointing to this port automatically prints through our driver.
- **Named Pipes**: Exposes `\\.\pipe\LBP1120_SpoolPipe` (Normal mode) and `\\.\pipe\LBP1120_PhotoPipe` (Photo mode) configured with open DACL (`D:(A;;GA;;;WD)`) to allow non-elevated print spooler processes to send raw PDF streams.

### 2. PDF Rendering Engine (`src/pdf.rs`)
- Loads PDF files or memory streams via `Windows.Data.Pdf.PdfDocument`.
- Renders to full-bleed **4960 × 7016** pixels (600 DPI A4) or proportional fit for US-Legal/Letter.
- Automatically detects landscape pages and applies 90° clockwise rotation.

### 3. SCoA Compression Engine (`src/scoa.rs`)
- Implements the exact hardware RLE encoding required by Canon's ASIC.
- Chunks raster data into 4,096-byte video packets with 4-byte CAPT packet framing (`0xD0E0` opcode).

### 4. Bidirectional USB Control (`src/usb.rs`)
- WinUSB Bulk pipe `0x01` (Out) and `0x82` (In).
- Real-time status monitoring: checks paper out, door open, video readiness, and engine errors before each page.
- Partial write re-assembly loop ensuring large page buffers are transmitted with zero packet loss.

---

## 🧪 Testing

The codebase includes comprehensive test coverage verifying:
- End-to-end PDF to SCoA encoding pipelines.
- Halftone pattern fidelity (thresholding vs Floyd-Steinberg).
- Multi-page document lifecycle and state cleanup.
- WinUSB mock transport under simulated disconnections and buffer timeouts.

Run all tests:
```powershell
cargo test
```

---

## 📄 License & Disclaimer

This project is developed for educational, archival, and hardware preservation purposes under clean-room reverse-engineering principles to restore functionality to abandoned hardware on modern operating systems.
All original Canon binary assets, drivers, and trademarks remain the property of Canon Inc.
