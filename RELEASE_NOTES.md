# CANON LASERSHOT LBP-1120 DRIVER SUITE — RELEASE NOTES
## Version: v1.0.0-rc.1 (Release Candidate 1) — 64-bit Windows Edition

**Release Date:** October 31, 2026  
**Target Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`, Device Serial `0353S4cQ`)  
**Target OS Platform:** Windows 10 & 11 (64-bit x86_64)  
**Distribution Channel:** GitHub Releases (Prerelease Candidate)

---

## 1. Release Highlights & Overview

The Canon LaserShot LBP-1120 Open Engineering Team is proud to announce the first Release Candidate (**v1.0.0-rc.1**) of the independent, reverse-engineered driver and Windows service ecosystem for the Canon LaserShot LBP-1120.

This release supersedes the obsolete 32-bit Windows XP drivers with a modern, zero-dependency, 64-bit user-mode architecture that integrates natively with the Windows Print Spooler subsystem (`spoolsv.exe`), the Service Control Manager (SCM), and Microsoft's in-box `WinUSB.sys` driver.

### Key Capabilities & Achievements
- **Turnkey Executable Installer:** Packaged as a professional, self-contained Windows installer (`CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe`, 2.38 MB).
- **Zero External Runtimes:** 100% self-contained. Requires no Python, no Node.js, no Rust toolchain, and no Microsoft Visual C++ redistributables.
- **Native Windows Printing:** Print directly from any Windows desktop application (Microsoft Word, Google Chrome, Adobe Acrobat, Notepad) using standard `Ctrl + P`.
- **Zero "Save As" Prompts:** Employs Windows `TCPMON.DLL` standard port monitors (`127.0.0.1:9100` and `9101`), eliminating all interactive PDF file dialogs.
- **Dual Printing Queues:**
  - `Canon LaserShot LBP-1120`: Normal Mode using high-contrast 8x8 clustered Bayer dot screen halftoning for crisp vector typography, lines, and schematics.
  - `Canon LaserShot LBP-1120 (Photo)`: Photo Mode using serpentine Floyd-Steinberg error diffusion for smooth continuous-tone photography and gradients.
- **Authentic Canon Voice Prompts:** Integrates 17 PCM audio voice alerts extracted from authentic Canon driver archives (`CAP3ZWAV.BIN`), giving spoken alerts for paper exhaustion, cover interlocks, and print completion.
- **Hardware Fault Resilience:** Automatic 3-tier SCM crash restart policy (`sc.exe failure`), dynamic USB handle acquisition (supports hot-unplugging and replugging), and hardware software reset (`canon-lbp1120-service.exe reset`).

---

## 2. Release Artifacts & SHA256 Verification

| File Name | Size | SHA256 Checksum | Purpose |
| :--- | :--- | :--- | :--- |
| **`CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe`** | 2,493,548 bytes (2.38 MB) | `4c06943c7768afb76f327d6946664b445ccba156b79aa09b3135631a5d7f1f8c` | Full GUI Windows installer wizard |
| **`canon-lbp1120-service.exe`** | 855,552 bytes (835.5 KB) | `ce9454519513fd40cf03ab459b7321668a8e9fcb7554a92d8b560580d032ff89` | Standalone SCM service & CLI diagnostic binary |

To verify artifact integrity in PowerShell:
```powershell
Get-FileHash -Path CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe -Algorithm SHA256
```

---

## 3. Architecture & Security Invariants

- **Architecture:** 64-bit AMD64 (`IMAGE_FILE_MACHINE_AMD64`, PE32+).
- **Service Account:** `NT AUTHORITY\LocalSystem` (technically required to guarantee unprivileged WinUSB access across dynamic USB root hub topologies).
- **Network Attack Surface:** Strict IPv4 loopback (`127.0.0.1`). Sockets are never bound to `0.0.0.0`. Remote network or LAN access is completely blocked.
- **Payload Governor:** Hard ceiling of 100 MB per spool job to prevent local disk space exhaustion.
- **Hardware ID Match:** INF matches strictly to `USB\VID_04A9&PID_262B` without generic class wildcards, preventing collateral interference with other USB devices.

---

## 4. Installation & Deployment Quick Start

### Graphical Installation (Recommended)
1. Download `CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe`.
2. Double-click to launch. Accept the User Account Control (UAC) prompt.
3. Follow the wizard steps to complete staging, WinUSB registration, queue setup, and service startup.
4. Ensure the Canon LaserShot LBP-1120 is connected via USB and powered ON.
5. Print a test page from Settings or Notepad.

### Uninstallation
- Run **Add/Remove Programs** $\rightarrow$ **Canon LaserShot LBP-1120 Driver Suite** $\rightarrow$ **Uninstall**.
- Alternatively, right-click `CanonLBP1120_Uninstall.bat` as Administrator.

---

## 5. Prerelease Status & Known Limitations

> [!IMPORTANT]
> This release is classified as a **Release Candidate (RC1)**. While it has undergone exhaustive automated test suites (69 Rust tests, 7 TypeScript tests, 18 integrity tests) and physical printing validation on the reference hardware (`Serial: 0353S4cQ`), users are advised:
> 1. Clean-slate VM certification on fresh Windows 10/11 ISOs is documented in the test matrix and remains active for external verification.
> 2. If the printer was previously configured with Microsoft's generic `USB Printing Support`, Device Manager will show the device under *Universal Serial Bus devices* as `Canon LaserShot LBP-1120 (WinUSB Interface)`. Driver rollback instructions are provided in `WINUSB_COMPATIBILITY_AND_SAFETY.md`.
