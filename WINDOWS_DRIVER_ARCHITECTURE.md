# WINDOWS DRIVER ARCHITECTURE & MANAGEMENT ECOSYSTEM
## 64-Bit Modern Driver Architecture for Canon LaserShot LBP-1120

**Document ID:** `WIN-DRV-LBP1120-2026-10-31`  
**Classification:** System Software Architecture & Integration Guide  
**Target OS:** Windows 10 & Windows 11 (x64)

---

## 1. System Integration Overview

Canon ceased updating official drivers for the LaserShot LBP-1120 after 32-bit Windows XP/Vista. Our independent modern architecture replaces the obsolete 32-bit vendor software with a modular, 64-bit native Rust printing daemon and automated Windows Print Spooler integration:

```mermaid
flowchart TD
    subgraph Client ["Windows Printing Client"]
        App["Windows Desktop Application (Word, Chrome, Acrobat)"]
    end

    subgraph Spooler ["Windows Print Spooler (spoolsv.exe)"]
        PrintDialog["Windows Print Dialog (Ctrl + P)"]
        QueueNormal["Print Queue: 'Canon LaserShot LBP-1120'<br/>(Default: Normal Text 8x8 Mode)"]
        QueuePhoto["Print Queue: 'Canon LaserShot LBP-1120 (Photo)'<br/>(Photo Dithered Mode)"]
        PdfDriver["Driver Core: 'Microsoft Print to PDF'"]
        TcpPort9100["Standard TCP/IP Port: 127.0.0.1:9100"]
        TcpPort9101["Standard TCP/IP Port: 127.0.0.1:9101"]
    end

    subgraph Service ["capt-poc Spool Server Daemon (Background Service)"]
        TcpListen9100["TCP Listener 9100 (Normal Mode)"]
        TcpListen9101["TCP Listener 9101 (Photo Mode)"]
        NamedPipe["Named Pipe: \\.\\pipe\\CanonLBP1120"]
        
        JobCoordinator["Thread-Safe Job Queue & State Coordinator"]
        
        Rasterizer["Direct2D WinRT Rasterizer (600 DPI)"]
        Halftoner["Halftone Engine (Normal Cluster vs Photo Floyd-Steinberg)"]
        ScoaEncoder["SCoA 2D Delta + RLE Encoder"]
        SoundEngine["Authentic Sound System (WinMM PlaySound)"]
        WinUsbBackend["WinUSB Hardware Transport Layer"]
    end

    subgraph Hardware ["Physical Printer Hardware"]
        LBP1120["Canon LaserShot LBP-1120 (USB VID 04A9, PID 262B)"]
    end

    App --> PrintDialog
    PrintDialog --> QueueNormal
    PrintDialog --> QueuePhoto
    QueueNormal --> PdfDriver
    QueuePhoto --> PdfDriver
    PdfDriver --> TcpPort9100
    PdfDriver --> TcpPort9101

    TcpPort9100 -->|Raw PDF Byte Stream| TcpListen9100
    TcpPort9101 -->|Raw PDF Byte Stream| TcpListen9101
    NamedPipe --> JobCoordinator

    TcpListen9100 --> JobCoordinator
    TcpListen9101 --> JobCoordinator

    JobCoordinator --> Rasterizer
    Rasterizer --> Halftoner
    Halftoner --> ScoaEncoder
    ScoaEncoder --> WinUsbBackend
    JobCoordinator --> SoundEngine

    WinUsbBackend -->|Bulk OUT Packets (0x01)| LBP1120
    LBP1120 -->|Bulk IN Telemetry (0x82)| WinUsbBackend
```

---

## 2. Zero-Prompt Dual-Mode Print Queues

To achieve a seamless user experience indistinguishable from a factory-supported printer, the spooler uses a standard Windows TCP/IP Raw Port monitor targeting local loopback:

1. **"Canon LaserShot LBP-1120" (Port `127.0.0.1:9100`):**
   - Configured with `HalftoneMode::Normal` (8×8 Clustered Dot screen).
   - Ideal for documents, office reports, vector typography, and contracts.
   - Laser spot enhancement optimizes crisp black text reproduction.
2. **"Canon LaserShot LBP-1120 (Photo)" (Port `127.0.0.1:9101`):**
   - Configured with `HalftoneMode::Photo` (Serpentine Floyd-Steinberg error diffusion).
   - Eliminates contour banding and preserves subtle continuous-tone gradients in photographs and illustrations.
3. **Automated Installation (`capt-poc install`):**
   - Programmatically executes PowerShell and Windows Print Management APIs to create the loopback TCP ports, register the printer queues using the built-in Microsoft PDF rendering engine, and configure spool permissions without prompting the user.

---

## 3. Authentic Canon Audio Feedback Engine

The driver restores Canon's original voice feedback system, invoking the Windows Multimedia API (`winmm::PlaySoundW`) asynchronously from memory:

| Voice Prompt | Extracted Audio File | Triggering Event in Spool Server |
| :--- | :--- | :--- |
| **"Printing has started"** | `Cap3strt.wav` | Spool daemon begins streaming `IC_VIDEO_DATA` frames to the printer. |
| **"Load paper into the printer"** | `Cap3addp.wav` | Basic Status bit `0x02` asserts before or during a job. Daemon pauses and waits. |
| **"Paper loaded, resuming..."** | `Cap3come.wav` | Paper is placed into tray (`payload[2] & 0x80`). Daemon resumes printing automatically. |
| **"Print job completed"** | `Cap3done.wav` | Trailing edge clears delivery sensor; hardware page counter increments. |
| **"Printer error occurred"** | `Cap3err.wav` | Hardware door opened (`0x4000`) or paper jam encountered. |

---

## 4. Modern Management Studio (Web/React GUI)

The repository includes a modern management dashboard built with React, Vite, Tailwind CSS, and Lucide Icons (`src/App.tsx` and `src/components/`):

- **Dashboard Tab (`DashboardTab.tsx`):** Displays real-time printer telemetry (Online, Tray status, Door state, Page counter, Fuser temperature), spool queue metrics, and one-click test page triggers.
- **Halftone Lab Tab (`HalftoneLabTab.tsx`):** Interactive comparison studio for testing all 9 monochrome halftoning algorithms on imported images with real-time visual zoom.
- **Protocol RE Lab Tab (`ProtocolReLabTab.tsx`):** Live CAPT packet inspector displaying opcodes, payloads, hex dumps, and roundtrip latency.
- **Architecture & Drivers Tab (`ArchitectureDriversTab.tsx`):** Interactive system sitemap, board diagrams, and driver installation controls.
