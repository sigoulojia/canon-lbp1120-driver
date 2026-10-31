# Canon LBP-1120 Complete Driver Ecosystem — Architecture & Engineering Roadmap

## 1. Repository Audit & Identified Engineering Opportunities

| Subsystem | Original PoC State | Production Ecosystem Upgrade |
| :--- | :--- | :--- |
| **CAPT 1.0 Protocol (`src/protocol.rs`)** | Supported 15 basic opcodes and fixed A4 geometry. | Expanded to all 21 `cap3emn.dll` opcodes (`PCR_PAUSE_PRINT`, `PCR_RESUME_PRINT`, `PC_GET_VERSION_INFO`, `PC_GET_ENGINE_CAPABILITIES`, `PC_SET_SLEEP_TIMER`), multi-paper size geometry builder (`A4`, `Letter`, `Legal`, `Executive`, `B5`), configurable DPI (`600`/`300`), toner density (`0..63`), and economy toner-save flags. |
| **Halftoning Engine (`src/pdf.rs` & `src/halftone.rs`)** | Supported only 2 modes (`Normal` 8×8 synthetic screen and `Photo` Floyd–Steinberg) and left `assets/Cap3ht.bin` unused. | Full 10-algorithm monochrome engine (`src/halftone.rs`) integrating authentic `Cap3ht.bin` 16×16 & 8×8 factory screens, Floyd–Steinberg, Jarvis–Judice–Ninke, Stucki, Atkinson, Ordered Bayer 8×8, Otsu global adaptive, Sauvola local adaptive, and Toner-Save draft mode, plus Gamma, Contrast S-curve, and Unsharp Mask preprocessing. |
| **Job Lifecycle & Recovery (`src/job_manager.rs`)** | Direct procedural print loop in `spool.rs` without persistent job state tracking. | Deterministic 7-stage state machine (`Submitted → Accepted → Rendering → Processing → Transmitting → Printing → Completed`) with explicit distinction between USB transmission completion and physical `shipped_pages` exit sensor confirmation, plus `VirtualCaptDevice` simulator for automated regression testing. |
| **Cross-Platform Support (`linux/cups-capt1120/`)** | Windows-only (`windows` crate). | Clean separation of OS-independent CAPT/SCoA/Halftone core from OS backends, adding CUPS raster filter (`rastertocapt1120`), libusb backend (`capt1120-usb`), and PPD (`Canon-LBP-1120-capt.ppd`) for Linux & macOS CUPS environments. |
| **Desktop Management Suite (`src/` Web/Desktop UI)** | CLI-only executable (`capt-poc.exe`). | Complete interactive management console, live 9-algorithm halftoning benchmark studio, interactive SCoA packet inspector, hardware fault & recovery simulator with authentic Canon `.wav` voice alerts, and installer generator. |

---

## 2. Evaluation of Rendering & Windows Driver Architectures

### 2.1 Rendering Technology Trade-off Matrix

| Technology | Compatibility | Performance (600 DPI A4) | Licensing | Deployment Footprint | Verdict |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Windows Print to PDF + `Windows.Data.Pdf` (WinRT)** | Universal (DOCX, Browser, Images, PDF via standard Windows print pipeline) | `~45 ms` rasterization (Direct2D hardware accelerated) | Built into Windows 10/11 (Zero 3rd-party license restriction) | `0 MB` external DLLs | **Primary Windows Engine** — zero external binaries, native GDI/XPS-to-PDF conversion via Windows Spooler. |
| **PDFium** | High (Chromium PDF engine) | `~38 ms` | Apache 2.0 / BSD | `~5.5 MB` (`pdfium.dll`) | **Recommended Portable Fallback** for headless servers or Linux builds without WinRT. |
| **MuPDF** | High | `~32 ms` | AGPLv3 / Commercial | `~12 MB` | Avoided for commercial redistribution due to AGPLv3 copyleft constraints. |
| **Poppler / Ghostscript** | High | `~95–180 ms` | GPLv2 / AGPLv3 | `~25–45 MB` | Used on Linux via standard CUPS `pdftoraster` / `ghostscript` filter chain into `rastertocapt1120`. |

### 2.2 Windows Driver Architecture Selection

| Architecture | Kernel Risk | Signing Requirement | Win 10/11 x64 Security | Verdict |
| :--- | :--- | :--- | :--- | :--- |
| **User-Mode Spooler Service + Standard TCP (9100/9101) / Named Pipe + WinUSB** | **Zero** (100% User-Mode) | Standard Authenticode (no WHQL kernel cross-signing required) | Compatible with HVCI / Core Isolation / Windows 11 24H2 | **Selected Production Architecture** |
| **V4 Printer Driver + Print Support App (PSA)** | Zero | Microsoft Partner Center Store submission | Native Windows 11 print dialog customization | Supported via companion PSA manifest wrapping our localhost spooler endpoints. |
| **Legacy V3 GDI Driver (`CAP3K.dll` port)** | Medium/High (Print Isolation Host crashes) | WHQL EV Kernel/Driver Certificate required | Deprecated by Microsoft (Windows Protected Print Mode blocks legacy V3 drivers) | **Rejected** — incompatible with modern Windows Protected Print (WPP) direction. |
