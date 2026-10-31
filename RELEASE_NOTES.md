# CANON LASERSHOT LBP-1120 DRIVER SUITE — RELEASE NOTES
## Version: v1.0.0-rc.2 (Release Candidate 2) — 64-bit Windows Edition

**Release Date:** October 31, 2026  
**Target Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`, Device Serial `0353S4cQ`)  
**Target OS Platform:** Windows 10 & 11 (64-bit x86_64)  
**Distribution Channel:** GitHub Releases (Prerelease Candidate)

---

## 1. Release Highlights & Overview

Release Candidate 2 (**v1.0.0-rc.2**) provides critical installation lifecycle fixes, atomic rollback verification, and non-destructive USB driver preservation for the Canon LaserShot LBP-1120 reverse-engineered driver and Windows background service.

### Key Enhancements in RC2
- **Strict 64-bit Program Files Path:** Enforced `{commonpf64}\CanonLBP1120` (`C:\Program Files\CanonLBP1120`) and locked `PrivilegesRequiredOverridesAllowed=` to eliminate per-user `AppData\Local\Programs` fallbacks.
- **Fail-Safe Rollback Engine:** The installer will never report success unless all 5 core post-installation invariants pass. If service startup, queue creation, port binding, or listener activation fails, Inno Setup immediately surfaces the exact error message and executes an atomic rollback.
- **USB Driver Provenance Preservation:** Automatically checks whether `USB\VID_04A9&PID_262B` is already operational under WinUSB (e.g., via `libwdi`/Zadig or existing OEM INF) and preserves the working binding without blind reinstallation.
- **Dynamic Driver Binding:** Queries installed print drivers (`*PDF*`, `*Generic*`, `*Virtual*`) dynamically, eliminating failure on machines with localized or alternate in-box print drivers.
- **Port Conflict Elimination:** Removed legacy background user-mode process spawning from queue installation, preventing `WSAEADDRINUSE (10048)` collisions with the SCM service.


---

## 2. Release Artifacts & SHA256 Verification

| File Name | Size | SHA256 Checksum | Purpose |
| :--- | :--- | :--- | :--- |
| **`CanonLBP1120_Setup_v1.0.0-rc.2_x64.exe`** | ~2.4 MB | See `SHA256SUMS.txt` | Full GUI Windows installer wizard |
| **`canon-lbp1120-service.exe`** | ~855 KB | See `SHA256SUMS.txt` | Standalone SCM service & CLI diagnostic binary |

To verify artifact integrity in PowerShell:
```powershell
Get-FileHash -Path CanonLBP1120_Setup_v1.0.0-rc.2_x64.exe -Algorithm SHA256
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
1. Download `CanonLBP1120_Setup_v1.0.0-rc.2_x64.exe`.
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

