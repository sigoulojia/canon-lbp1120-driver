# CANON LASERSHOT LBP-1120 DRIVER SUITE — RELEASE NOTES
## Version: v1.0.0-rc.3 (Release Candidate 3) — 64-bit Windows Edition

**Release Date:** October 31, 2026  
**Target Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`, Device Serial `0353S4cQ`)  
**Target OS Platform:** Windows 10 & 11 (64-bit x86_64)  
**Distribution Channel:** GitHub Releases (Prerelease Candidate)

---

## 1. Release Highlights & Overview

Release Candidate 3 (**v1.0.0-rc.3**) resolves the Windows Service installation failure encountered during RC2 testing on secondary Windows systems. It eliminates command-line quoting issues, avoids Service Control Manager deletion race conditions (`ERROR_SERVICE_MARKED_FOR_DELETE` 1072), introduces persistent automated installation diagnostics surviving installer rollback, and enhances GUI error reporting.

### Key Enhancements in RC3
- **Command-Line Argument Parsing Fix:** Resolved quoting discrepancies when registering `canon-lbp1120-service.exe` with `sc.exe`. The binary path and its `service-run` parameter are serialized using exact unquoted option key grammar (`binPath= "\"C:\Program Files\CanonLBP1120\canon-lbp1120-service.exe\" service-run"`), eliminating `ERROR_INVALID_PARAMETER` (87 / 1639).
- **In-Place SCM Updates (Fix Error 1072):** Removed the `sc delete` $\rightarrow$ `sc create` anti-pattern. If `CanonLBP1120Service` is already present, the installer updates configuration in-place via `sc config`, preventing `ERROR_SERVICE_MARKED_FOR_DELETE` (1072) when open handles exist in management tools (`services.msc`, Task Manager).
- **Automated Rollback-Resistant Diagnostics:** The installer automatically generates:
  ```text
  %TEMP%\CanonLBP1120_Install_Diagnostics.txt
  ```
  This report records OS version, CPU architecture, installation directory, binary integrity, TCP port 9100/9101 availability, SCM initial state, and complete stdout/stderr/exit codes for every command executed. Because `%TEMP%` is not rolled back, the report remains accessible on disk even if installation aborts.
- **Enhanced GUI Error Reporting:** When a post-installation step fails, the installer reads `%TEMP%\CanonLBP1120_Install_Error.txt` and displays the exact Windows/SCM error and diagnostic path in the message dialog before triggering rollback.
- **Audited Conflict Resolution:** Safely checks TCP ports 9100 and 9101. If and only if an orphaned background process belonging to this driver suite (`canon-lbp1120-service.exe` or `capt-poc.exe`) is holding the port, it terminates that specific PID, logs the action, and leaves all third-party processes untouched.
- **Standalone Diagnostic Tooling:** Added `canon-lbp1120-service.exe diagnose-service` command for testing SCM permissions, querying existing service state, and checking port bindings without running Inno Setup.
- **Hardware & Driver Preservation:** Continues to strictly preserve operational WinUSB bindings (`oem5.inf` / `libwdi`) without modification. SCoA compression and the printing pipeline remain unaltered.

---

## 2. Release Artifacts & SHA256 Verification

| File Name | Size | Purpose |
| :--- | :--- | :--- |
| **`CanonLBP1120_Setup_v1.0.0-rc.3_x64.exe`** | ~2.5 MB | Full GUI Windows installer wizard |
| **`canon-lbp1120-service.exe`** | ~920 KB | Standalone SCM service & CLI diagnostic binary |
| **`SHA256SUMS.txt`** | Text | Cryptographic SHA256 checksums |
| **`RC2_SERVICE_INSTALLATION_ROOT_CAUSE_AUDIT.md`** | Markdown | Complete technical audit of the service installation pipeline |

To verify artifact integrity in PowerShell:
```powershell
Get-FileHash -Path CanonLBP1120_Setup_v1.0.0-rc.3_x64.exe -Algorithm SHA256
```

---

## 3. Architecture & Security Invariants

- **Architecture:** 64-bit AMD64 (`IMAGE_FILE_MACHINE_AMD64`, PE32+).
- **Service Account:** `NT AUTHORITY\LocalSystem` (required to guarantee WinUSB hardware access across dynamic root hub enumerations).
- **Network Attack Surface:** Strict IPv4 loopback (`127.0.0.1`). Sockets are never bound to `0.0.0.0`. Remote network or LAN access is completely blocked.
- **Payload Governor:** Hard ceiling of 100 MB per spool job to prevent local disk space exhaustion.
- **Hardware ID Match:** Strict matching to `USB\VID_04A9&PID_262B` without generic class wildcards.

---

## 4. Installation & Deployment Quick Start

### Graphical Installation (Recommended)
1. Download `CanonLBP1120_Setup_v1.0.0-rc.3_x64.exe`.
2. Double-click to launch. Accept the User Account Control (UAC) prompt.
3. Follow the wizard steps to complete staging, WinUSB registration, queue setup, and service startup.
4. Ensure the Canon LaserShot LBP-1120 is connected via USB and powered ON.
5. Print a test page from Settings or Notepad.

### Uninstallation
- Run **Add/Remove Programs** $\rightarrow$ **Canon LaserShot LBP-1120 Driver Suite** $\rightarrow$ **Uninstall**.
- Alternatively, right-click `CanonLBP1120_Uninstall.bat` as Administrator.

---

## 5. Prerelease Status & Notice

> [!IMPORTANT]
> This build is released as a **Release Candidate (RC3)** for independent verification on secondary Windows systems. Clean-machine and physical printer validation remain active and pending on external hardware.
