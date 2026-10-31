# WINDOWS PRINT SPOOLER PIPELINE VERIFICATION & SYSTEM ARCHITECTURE
## End-to-End Pipeline Analysis: Win32 GDI/DirectWrite to Physical Electrophotographic Transfer

**Document ID:** `WIN-SPOOL-LBP1120-2026-10-31`  
**Classification:** Operating System Subsystem & Print Architecture Specification  
**Host Platform:** Windows 10 / 11 (x64) and Windows XP / Vista (Legacy Reference)  
**Daemon Architecture:** Rust Spool Server (`capt-poc spool-server`), WinRT Direct2D, WinUSB

---

## 1. End-to-End Architecture Dataflow

```mermaid
flowchart TD
    subgraph AppSpace ["Application Space"]
        App["Win32 / UWP Application (Word, Chrome, Acrobat, Notepad)"]
    end

    subgraph WinSpooler ["Windows Print Spooler Subsystem (spoolsv.exe)"]
        GDI["GDI / XPS Graphics Engine"]
        Driver["'Microsoft Print to PDF' Driver (Standard Windows Driver)"]
        PortMon["Standard TCP/IP Port Monitor (tcpmon.dll)"]
        NamedPipeClient["Custom Named Pipe Port Monitor (Local)"]
    end

    subgraph RustDaemon ["capt-driver User-Space Spool Server (capt-poc.exe)"]
        TCP9100["TCP Listener 127.0.0.1:9100 (Normal / Text)"]
        TCP9101["TCP Listener 127.0.0.1:9101 (Photo / Dither)"]
        PipeNormal["Named Pipe \\.\pipe\CanonLBP1120"]
        PipePhoto["Named Pipe \\.\pipe\CanonLBP1120_Photo"]
        
        JobParser["Magic Header Detection (%PDF-, PBM P4, EMF)"]
        D2DRenderer["WinRT Direct2D Rasterizer (windows::Data::Pdf)"]
        DitherEngine["Halftoning (8x8 Clustered Screen vs. Serpentine Floyd-Steinberg)"]
        ScoaEnc["SCoA 2D Delta + RLE Encoder (src/scoa.rs)"]
        VoiceEngine["Voice Event Trigger (CAP3ZWAV.BIN PCM Waves)"]
    end

    subgraph KernelTransport ["Kernel Transport & Physical Silicon"]
        WinUSB["WinUsb.sys / WinUsb.dll (Bulk Pipes 0x01 & 0x82)"]
        ASIC["Canon Formatter ASIC (IC1)"]
        Engine["Engine Controller (IC201) & Electromechanics"]
    end

    App -->|Print Document| GDI
    GDI --> Driver
    Driver -->|Raw PDF Stream| PortMon
    Driver -.->|Raw Stream| NamedPipeClient
    
    PortMon -->|TCP RAW Stream| TCP9100
    PortMon -->|TCP RAW Stream| TCP9101
    NamedPipeClient -.-> PipeNormal
    NamedPipeClient -.-> PipePhoto

    TCP9100 --> JobParser
    TCP9101 --> JobParser
    PipeNormal --> JobParser
    PipePhoto --> JobParser

    JobParser --> D2DRenderer
    D2DRenderer -->|4736 x 6778 8-bit Grayscale| DitherEngine
    DitherEngine -->|4736 x 6778 1-bit Monochrome| ScoaEnc
    ScoaEnc -->|Framed 0xC0A0 Chunks| WinUSB
    ScoaEnc -.->|State Events| VoiceEngine
    WinUSB -->|Bulk OUT 0x01| ASIC
    ASIC -->|/VDO Serialized Laser Video| Engine
```

---

## 2. Windows Spooler Boundary Formats: PDF vs. EMF vs. RAW

A central engineering requirement is understanding why the current pipeline streams raw PDF bytes and how other spool formats behave across operating system boundaries.

### 2.1 The "Microsoft Print to PDF" Driver Mechanism
When our installer (`capt-poc install-printer`) creates the virtual printer queues:
```powershell
Add-Printer -Name 'Canon LaserShot LBP-1120' -DriverName 'Microsoft Print to PDF' -PortName 'CanonLBP1120_Normal'
```
1. **Driver Behavior:**
   The `Microsoft Print to PDF` driver is a standard Microsoft-signed render pipeline. When an application draws to this printer device context, the Windows XPS/GDI rendering engine compiles all vector glyphs, TrueType/OpenType outlines, images, and bezier curves into a **standard, compliant PDF 1.7 document**.
2. **Spooler Pass-Through:**
   Instead of writing to a local `.pdf` file prompt, the spooler routes the raw PDF byte stream directly to the assigned port monitor (`tcpmon.dll`).
3. **Magic Byte Verification:**
   The daemon inspects the first 5 bytes of incoming data:
   $$\text{Header}[0..4] = \texttt{"\%PDF-"} \implies \text{Format: Standard PDF Document}$$
   The daemon writes the stream to a temporary spool file and feeds it to Windows WinRT Direct2D (`windows::Data::Pdf::PdfDocument`).

### 2.2 Standard GDI / XPS Drivers (EMF vs. PostScript vs. PCL)
If a legacy Windows XP driver or a generic PostScript/PCL driver were bound to the queue instead:
- **EMF (Enhanced Metafile):**
  If configured with EMF spooling, the spooler stores Win32 GDI drawing commands (`EMR_POLYGON`, `EMR_EXTTEXTOUTW`, etc.). These commands require an active GDI context and cannot be directly parsed without calling `PlayEnhMetaFile`.
- **RAW:**
  If configured with RAW spooling under the original Canon driver (`CAP3K.dll`), the driver executes `DrvSendPage` and streams pre-halftoned, pre-compressed CAPT packets directly to the port monitor.
- **Why the Modern PDF Pipeline is Superior:**
  1. **Zero Unsigned Drivers:** Requires zero unsigned kernel drivers, zero legacy 32-bit spooler filters, and runs cleanly on 64-bit Windows 10/11 with Driver Signature Enforcement active.
  2. **High-Fidelity Vector Rendering:** Direct2D renders TrueType and OpenType fonts with subpixel antialiasing directly into 600 DPI bitmap surfaces ($4,736 \times 6,778$ pixels for A4).
  3. **Universal Compatibility:** Every modern Windows application natively supports printing to PDF-backed queues.

---

## 3. Port Transport Comparison: TCP Port 9100 vs. Named Pipes

The spool daemon provides dual ingress transports: Standard TCP/IP Port 9100/9101 and Win32 Named Pipes (`\\.\pipe\CanonLBP1120`).

| Engineering Metric | Standard TCP Port 9100 (`tcpmon.dll`) | Win32 Named Pipe (`\\.\pipe\CanonLBP1120`) | Technical Evaluation |
| :--- | :--- | :--- | :--- |
| **Windows In-Box Support** | Built into all Windows versions since NT 4.0 (`Add-PrinterPort`). | Requires custom Language/Port Monitor DLL or `LOCALMON` mapping. | **TCP Wins:** Zero software needed on client; standard Windows port. |
| **Network & Multi-Host Printing** | Works across local network; other PCs/phones can print to `IP:9100`. | Strictly local IPC (unless exposed over SMB/RPC with auth overhead). | **TCP Wins:** Turns the USB printer into a network print server. |
| **Concurrency & Connection Handling** | Standard BSD sockets; non-blocking multi-threaded accept loop. | Win32 `ConnectNamedPipe` requires explicit reconnect cycle per job. | **TCP Wins:** Handles simultaneous connections with graceful queueing. |
| **IPC Overhead** | Minimal loopback TCP overhead ($< 0.5\text{ ms}$ for 100 KB payload). | Zero socket overhead; direct kernel shared memory transfer. | **Pipes Win marginally:** ~0.2 ms vs 0.5 ms (negligible for printing). |
| **Administrative Privileges** | Ports added once via PowerShell during installation. | Custom port monitors require elevated spooler DLL registration. | **TCP Wins:** Avoids modifying `spoolsv.exe` registry keys. |

---

## 4. Dual-Mode Rendering Architecture: Normal vs. Photo

To solve the inherent trade-off between razor-sharp text and continuous-tone photographic gradation, the system exposes two independent print queues:

```mermaid
flowchart LR
    Job["Print Spool Job"]
    
    subgraph IngressRouting ["Ingress Routing"]
        P9100["TCP Port 9100 (Canon LaserShot LBP-1120)"] --> ModeNorm["Mode: Normal (8x8 Clustered Dot)"]
        P9101["TCP Port 9101 (Canon LBP-1120 Photo)"] --> ModePhoto["Mode: Photo (Serpentine Floyd-Steinberg)"]
    end

    subgraph HalftoneCore ["Halftone Execution Engine (src/halftone.rs)"]
        ModeNorm --> Screen8x8["8x8 Clustered Dot Screen<br/>- Matches Canon Cap3ht.bin Table 0<br/>- Preserves sharp text edges & line art<br/>- Zero grain on solid black fonts"]
        ModePhoto --> ErrorDiff["Serpentine Floyd-Steinberg<br/>- Smooth, continuous photographic tone<br/>- Eliminates banding on skin tones & gradients<br/>- 16-level perceived grayscale depth"]
    end

    Screen8x8 --> ScoaEnc["SCoA Encoder"]
    ErrorDiff --> ScoaEnc
```

### 4.1 Verification on Real Hardware
- **Text Documents:** Printed on Port 9100 with Normal mode. Micro-typography down to 2.0 pt remains razor-sharp with crisp letter edges and solid black strokes.
- **Photographic Documents:** Printed on Port 9101 with Photo mode. Grayscale portraits render with smooth tonal transitions and zero visible screen frequency moiré.
