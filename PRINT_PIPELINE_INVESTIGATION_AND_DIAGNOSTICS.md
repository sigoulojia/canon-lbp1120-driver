# CANON LASERSHOT LBP-1120 — PRINT PIPELINE TRACER & REAL-WORLD FAILURE INVESTIGATION

**Document Version:** 1.0.0  
**Target Architecture:** Windows 10 & 11 (x86_64)  
**Hardware ID:** `USB\VID_04A9&PID_262B` (Canon LaserShot LBP-1120)  
**Associated Release:** `v1.0.0-rc.4`  

---

## 1. Executive Summary & Problem Statement

### 1.1 The Reported Symptom
Following the successful installation of Release Candidate 3 (`v1.0.0-rc.3`) on a secondary Windows computer, Windows applications and the Windows Print Spooler accepted print jobs normally. The Windows Print Queue displayed the job status as **"Imprimé" (Printed)** and purged the queue entry. However, the physical Canon LaserShot LBP-1120 printer produced **no paper output, no mechanical pickup, and no engine motor activity**.

### 1.2 Core Finding: The "Imprimé" Fallacy
In the Windows printing architecture, standard Port Monitors (`winspool.drv` / `tcpmon.dll`) consider a print job **"Printed"** the moment the last byte is successfully accepted into the socket buffer of the receiving port (TCP port 9100/9101 or Named Pipe). 

The Windows Spooler **has zero knowledge of downstream processing**. A status of "Printed" confirms only that the job successfully traversed **Stage 1 (Queue)** and **Stage 2 (Local Port Connection)**. It provides no guarantee that:
1. The byte stream is in a valid format (PDF).
2. The service successfully rasterized the document.
3. The WinUSB device handle was opened.
4. The USB packets were physically clocked into the Canon ASIC.
5. The print engine completed the paper feed.

---

## 2. Root Cause Analysis: Why Failures Were Silent

Prior to RC4, an operational failure on a secondary machine left no visible trace due to three structural factors:

1. **Standard I/O Redirection Under SCM (`SERVICE_WIN32_OWN_PROCESS`):**
   When `canon-lbp1120-service.exe` runs as a Windows Service under `NT AUTHORITY\LocalSystem`, its standard output and standard error streams are nullified by the OS. All previous console print statements (`println!`) inside the worker thread were discarded.
2. **Silent Server Mode:**
   The socket server was invoked with `silent = true`, causing thread panics or error results in `run_print_job` to be swallowed silently without writing to `C:\ProgramData\CanonLBP1120\service.log`.
3. **Immediate Spool File Deletion:**
   The spool listener unlinked the incoming spool file immediately after processing, regardless of whether processing succeeded or failed. If a format mismatch occurred (e.g., an application sending raw XPS or text), the offending stream was destroyed before it could be inspected.

---

## 3. The 7-Stage End-to-End Tracing Architecture

Release Candidate 4 introduces exhaustive, deterministic observability across all 7 stages of the print pipeline.

```
[Stage 1: Windows Spooler]
       │
       ▼ (RAW TCP 9100 / Named Pipe)
[Stage 2: Connection Acceptance]
       │
       ▼ (%ProgramData%\CanonLBP1120\spool\)
[Stage 3: Stream Ingestion & Magic Byte Analysis]
       │
       ├── Non-PDF -> Preserved as <job_id>_rejected_<fmt>.bin -> FAIL
       ▼ (WinRT Direct2D @ 600 DPI)
[Stage 4: PDF Rasterization]
       │
       ▼ (2D Delta + SCoA RLE)
[Stage 5: SCoA Encoding & CAPT 1.0 Packet Framing]
       │
       ▼ (Dynamic Multi-Candidate Discovery)
[Stage 6: WinUSB Device Opening & ASIC Telemetry]
       │
       ▼ (Bulk Out 0x01 / 0x02)
[Stage 7: USB Packet Transfer & Mechanical Feed Confirmation]
       │
       ▼
[PHYSICAL PAPER OUTPUT]
```

### Stage 1: Windows Print Spooler Submission
- **Check:** Validates that the Windows printer queue is correctly configured with `DriverName = "Microsoft Print To PDF"`, `Datatype = "RAW"`, and points to `CanonLBP1120_Normal` or `CanonLBP1120_Photo`.
- **Failure Mode Caught:** Third-party driver or XPS-based driver writing incompatible stream formats to the port.

### Stage 2: RAW TCP / Named Pipe Connection Acceptance
- **Check:** Detects incoming connections on `127.0.0.1:9100` (Normal Mode), `127.0.0.1:9101` (Photo Mode), or `\\.\pipe\CanonLBP1120`.
- **Unique Job ID:** Immediately assigns a timestamped, structured Job ID:
  ```text
  JOB-HHMMSS-NORM-P9100-NNNN
  ```
- **Logger Initialization:** Spawns a dedicated `JobLogger` streaming concurrently to:
  1. `C:\ProgramData\CanonLBP1120\jobs\<job_id>.log`
  2. `C:\ProgramData\CanonLBP1120\service.log`
  3. Interactive console stdout (when executed in CLI mode).

### Stage 3: Service Ingestion & Stream Analysis
- **Check:** Reads the initial 32 bytes of the payload and executes `analyze_stream_format`:
  - Recognizes `%PDF-` (`PDF`), `PK\x03\x04` (`XPS`), `\x1b%-12345X` (`PCL`), `%!PS` (`PostScript`), `\x01\x00\x00\x00` (`EMF`), ASCII text, or `Unknown`.
- **Enforcement:** If a non-PDF format is received, the service preserves the stream to `C:\ProgramData\CanonLBP1120\spool\<job_id>_rejected_<fmt>.bin` and logs an explicit diagnostic error:
  ```text
  ERROR: Received <fmt> stream instead of required PDF stream!
  Diagnosis: The printer queue is using an incompatible driver that outputs raw XPS or PCL.
  ```

### Stage 4: PDF / Direct2D Raster Conversion
- **Check:** Initializes the Windows Runtime (`windows::Data::Pdf`) Direct2D rendering pipeline at 600 DPI.
- **Telemetry:** Logs page count, pixel dimensions (`width x height`), total black pixels, and conversion latency in milliseconds.
- **Preservation:** If rendering fails, the input file is preserved at `C:\ProgramData\CanonLBP1120\spool\<job_id>_failed.pdf` for post-mortem analysis.

### Stage 5: SCoA Compression & CAPT 1.0 Packet Framing
- **Check:** Compresses raster lines using 2D Delta + SCoA RLE encoding.
- **Telemetry:** Logs total compressed bytes, compression ratio (e.g., `4.01 MB -> 157.21 KB`), total packet count, and 16-bit word alignment padding.
- **Integrity:** Invariant verification of `IC_BEGIN_PAGE`, video lines, `PCR_GO_ONLINE` (embedding magic `0xADEADBEE`), and `IC_END_PAGE`.

### Stage 6: WinUSB Device Discovery & ASIC Telemetry
- **Dynamic Multi-Candidate Discovery:**
  Rather than relying on a hardcoded registry string or single serial casing, the driver dynamically evaluates:
  1. Primary GUID: `\\?\USB#VID_04A9&PID_262B#<serial>#{F9A25620-E255-4B37-93F3-0C3D3D2DFBD3}`
  2. Secondary GUID: `\\?\USB#VID_04A9&PID_262B#<serial>#{a5dcbf10-6530-11d2-901f-00c04fb951ed}`
  3. Registry Enumeration: Queries `HKLM\SYSTEM\CurrentControlSet\Control\DeviceClasses` for any active symbolic link matching VID `04A9` and PID `262B`.
  4. Serial Case Insensitivity: Evaluates both lowercase (`0353S4cQ`) and uppercase (`0353S4CQ`) hardware descriptors.
- **ASIC Sensor Query:** Reads IEEE 1284 Device ID, Port Status Register (`0x18`), and Extended Hardware Registers (`Basic=0x78`, `Aux=0x00`).
- **Telemetry:** Confirms: `DoorOpen=false`, `NoCartridge=false`, `PaperJam=false`, `NoPaper=false`, `UnitFree=true`.

### Stage 7: USB Transmission & Mechanical Feed Confirmation
- **Handshake:** Sends `PCR_GO_ONLINE` and awaits engine ready state (`WAIT_FOR_VIDEO_READINESS`).
- **Data Transmission:** Streams bulk packets in calibrated 16 KB chunks to the bulk out endpoint.
- **Mechanical Monitoring:** After transmitting `IC_END_PAGE`, the driver monitors the printer's page delivery lifecycle counters (`Started`, `Printing`, `Shipped`, `Printed`) to ensure mechanical paper pickup, fuser pass-through, and tray delivery.

---

## 4. Pipeline Execution Summary Report

Upon job completion (or failure), the service logs a consolidated pipeline summary table to both the per-job log and `service.log`:

```text
======================================================================
CANON LBP-1120 PRINT PIPELINE EXECUTION SUMMARY: JOB-211800-NORM-P9100-0001
======================================================================
Stage 1: Windows Print Spooler Submission    -> ACCEPTED
Stage 2: RAW TCP / Named Pipe Connection      -> SUCCESS
Stage 3: Service Ingestion & Stream Analysis  -> SUCCESS (Format: PDF, Bytes: 240,912)
Stage 4: PDF / Direct2D Raster Conversion     -> SUCCESS (Pages: 1, Dims: 4736x6778 @ 600 DPI)
Stage 5: SCoA Compression & CAPT Framing      -> SUCCESS (Packets: 2, Bytes: 157,210, Ratio: 96.1%)
Stage 6: WinUSB Device Discovery & Interface  -> SUCCESS (Handle: Valid, ASIC: Ready)
Stage 7: USB Transmission & Mechanical Feed   -> SUCCESS (Transmitted: 157,210 bytes, Shipped: 1)
----------------------------------------------------------------------
FINAL RESULT: PRINT JOB COMPLETED SUCCESSFULLY
======================================================================
```

If an error occurs at *any* stage, the table explicitly pinpoints the failing stage:
```text
======================================================================
CANON LBP-1120 PRINT PIPELINE EXECUTION SUMMARY: JOB-212215-NORM-P9100-0002
======================================================================
Stage 1: Windows Print Spooler Submission    -> ACCEPTED
Stage 2: RAW TCP / Named Pipe Connection      -> SUCCESS
Stage 3: Service Ingestion & Stream Analysis  -> FAILED: Stream is XPS (requires PDF)
Stage 4: PDF / Direct2D Raster Conversion     -> SKIPPED
Stage 5: SCoA Compression & CAPT Framing      -> SKIPPED
Stage 6: WinUSB Device Discovery & Interface  -> SKIPPED
Stage 7: USB Transmission & Mechanical Feed   -> SKIPPED
----------------------------------------------------------------------
FINAL RESULT: REJECTED INCOMPATIBLE STREAM FORMAT
======================================================================
```

---

## 5. Diagnostic Tooling on the Target PC

Release Candidate 4 provides immediate, zero-friction diagnostic tools for users and engineers on secondary PCs:

### 5.1 One-Click Diagnostic Batch Script
Located in `C:\Program Files\CanonLBP1120\CanonLBP1120_TracePipeline.bat`:
- Running this script immediately executes the 7-stage diagnostic trace and pauses the console for review.

### 5.2 Start Menu Shortcuts
The installer registers the following Start Menu entries under `Canon LaserShot LBP-1120 Driver Suite`:
- **Run Print Pipeline Diagnostics:** Executes `canon-lbp1120-service.exe trace-pipeline`.
- **View Print Pipeline Diagnostics Log:** Opens `%TEMP%\CanonLBP1120_Print_Trace.txt` in Notepad.
- **Open Print Jobs Directory:** Opens `C:\ProgramData\CanonLBP1120\jobs\` in Windows Explorer.
- **View Service Log:** Opens `C:\ProgramData\CanonLBP1120\service.log` in Notepad.

### 5.3 CLI Subcommand
```powershell
& "C:\Program Files\CanonLBP1120\canon-lbp1120-service.exe" trace-pipeline
```
Outputs an exhaustive audit of:
- Print queue status, drivers, and port bindings.
- Windows Service status (`RUNNING` vs `STOPPED`).
- Active TCP listeners on ports 9100 and 9101.
- PnP hardware status and live WinUSB communication with the printer ASIC.
- Recent job logs and service log events.
- Writes full results to `%TEMP%\CanonLBP1120_Print_Trace.txt`.

---

## 6. Target Machine Diagnostic Protocol (Step-by-Step)

When testing RC4 on the secondary computer:

1. **Install RC4:** Run `CanonLBP1120_Setup_v1.0.0-rc.4_x64.exe` as Administrator.
2. **Submit a Print Job:** Print a test document from Notepad or Windows Settings to `Canon LaserShot LBP-1120`.
3. **Inspect Output:**
   - If paper prints: Mission accomplished!
   - If paper does not print:
     1. Open `C:\ProgramData\CanonLBP1120\jobs\`.
     2. Open the latest `.log` file (e.g., `JOB-*.log`).
     3. Check the **Execution Summary Table** at the bottom of the log:
        - If **Stage 3** failed: The print driver sent non-PDF data (check queue driver configuration).
        - If **Stage 4** failed: Direct2D rasterization failed (inspect the preserved `spool\<job_id>_failed.pdf`).
        - If **Stage 6** failed: WinUSB device path could not be opened (driver binding or permissions).
        - If **Stage 7** failed: Printer hardware interlock (paper empty, cover open, or video sync timeout).
     4. Run `CanonLBP1120_TracePipeline.bat` to capture the complete system diagnostic state.
