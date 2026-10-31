# CANON LASERSHOT LBP-1120 DRIVER SUITE — RELEASE NOTES
## Version: v1.0.0-rc.4 (Release Candidate 4) — 64-bit Windows Edition

**Release Date:** October 31, 2026  
**Target Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`, Device Serial `0353S4cQ` / `0353S4CQ`)  
**Target OS Platform:** Windows 10 & 11 (64-bit x86_64)  
**Distribution Channel:** GitHub Releases (Prerelease Candidate)

---

## 1. Release Highlights & Overview

Release Candidate 4 (**v1.0.0-rc.4**) addresses real-world printing execution on secondary Windows PCs where print jobs are submitted and marked as "Printed" / "Imprimé" by Windows, but no physical paper is output by the printer.

RC4 eliminates the observability blind spot in the background service, introduces dynamic multi-candidate USB device discovery, establishes comprehensive 7-stage end-to-end tracing, preserves rejected and failed print streams for analysis, and provides one-click diagnostic tools for target systems.

### Key Enhancements in RC4
- **7-Stage End-to-End Pipeline Tracing:**
  Traces every print job through:
  - Stage 1: Windows Print Spooler submission (driver & port binding)
  - Stage 2: RAW TCP / Named Pipe socket connection acceptance (127.0.0.1:9100/9101 or Named Pipe)
  - Stage 3: Service stream ingestion & magic byte analysis (PDF vs XPS/PCL/EMF)
  - Stage 4: WinRT Direct2D raster conversion (600 DPI rendering, dimensions, pixel count)
  - Stage 5: SCoA compression & CAPT 1.0 packet framing (video lines, alignment, headers)
  - Stage 6: WinUSB device opening & ASIC telemetry registers
  - Stage 7: USB bulk packet streaming & mechanical paper feed confirmation
- **Multi-Candidate WinUSB Device Discovery:**
  Dynamically evaluates multiple device interface GUIDs (`{F9A25620-E255-4B37-93F3-0C3D3D2DFBD3}` and `{a5dcbf10-6530-11d2-901f-00c04fb951ed}`), queries `HKLM\SYSTEM\CurrentControlSet\Control\DeviceClasses` dynamically, and transparently accommodates serial number casing discrepancies (`0353S4CQ` vs `0353S4cQ`).
- **Persistent Per-Job Logging & Spool Preservation:**
  - Each job is assigned a unique timestamped ID (e.g. `JOB-211800-NORM-P9100-0001`).
  - Writes detailed logs to `C:\ProgramData\CanonLBP1120\jobs\<job_id>.log` and `service.log`.
  - Non-PDF streams (e.g. raw XPS from incompatible drivers) are automatically preserved as `<job_id>_rejected_<fmt>.bin` with actionable error diagnosis.
  - Failed raster jobs are preserved as `<job_id>_failed.pdf` instead of being silently deleted.
- **Integrated Diagnostic Subcommand & Batch Script:**
  - Added CLI command: `canon-lbp1120-service.exe trace-pipeline`
  - Added one-click script `C:\Program Files\CanonLBP1120\CanonLBP1120_TracePipeline.bat`
  - Added Start Menu shortcuts for "Run Print Pipeline Diagnostics", "View Print Pipeline Diagnostics Log", and "Open Print Jobs Directory".
- **Preserved Core Engine:** SCoA compression, 9-mode halftoning algorithms, and the verified USB packet protocol remain untouched.

---

## 2. Release Artifacts & SHA256 Verification

| File Name | Size | Purpose |
| :--- | :--- | :--- |
| **`CanonLBP1120_Setup_v1.0.0-rc.4_x64.exe`** | ~2.5 MB | Full GUI Windows installer wizard with diagnostic suite |
| **`canon-lbp1120-service.exe`** | ~1.0 MB | Standalone SCM service, printer server & CLI diagnostic binary |
| **`SHA256SUMS.txt`** | Text | Cryptographic SHA256 checksums |
| **`PRINT_PIPELINE_INVESTIGATION_AND_DIAGNOSTICS.md`** | Markdown | Complete technical audit and forensic diagnostic documentation |
| **`SERVICE_INSTALLATION_ROOT_CAUSE_AUDIT.md`** | Markdown | Technical audit of the service installation pipeline |

To verify artifact integrity in PowerShell:
```powershell
Get-FileHash -Path CanonLBP1120_Setup_v1.0.0-rc.4_x64.exe -Algorithm SHA256
```

---

## 3. Architecture & Security Invariants

- **Architecture:** 64-bit AMD64 (`IMAGE_FILE_MACHINE_AMD64`, PE32+).
- **Service Account:** `NT AUTHORITY\LocalSystem` (required to guarantee WinUSB hardware access across dynamic root hub enumerations).
- **Network Attack Surface:** Strict IPv4 loopback (`127.0.0.1`). Sockets are never bound to `0.0.0.0`. Remote network or LAN access is completely blocked.
- **Payload Governor:** Hard ceiling of 100 MB per spool job to prevent local disk space exhaustion.
- **Hardware ID Match:** Strict matching to `USB\VID_04A9&PID_262B` without generic class wildcards.

---

## 4. Target Machine Testing Protocol

1. Download and run **`CanonLBP1120_Setup_v1.0.0-rc.4_x64.exe`** as Administrator.
2. Submit a print job from Notepad or Windows Settings to **`Canon LaserShot LBP-1120`**.
3. If the document prints: The pipeline is fully verified on your hardware.
4. If no paper feeds:
   - Check `C:\ProgramData\CanonLBP1120\jobs\` for the generated job log.
   - Run the Start Menu shortcut **Run Print Pipeline Diagnostics** (or `CanonLBP1120_TracePipeline.bat`).
   - The diagnostic report will pinpoint the exact stage of the stall.

---

## 5. Prerelease Status & Notice

> [!IMPORTANT]
> This build is released as a **Release Candidate (RC4)** for diagnostic and operational verification on secondary Windows hardware. Clean-machine and physical printer validation remain active and pending on external hardware.
