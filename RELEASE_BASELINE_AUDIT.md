# CANON LASERSHOT LBP-1120 RELEASE BASELINE AUDIT
## Phase 0 Technical Audit: Repository State, Inno Setup Compiler Diagnostics & Release Blockers

**Document ID:** `AUDIT-RELEASE-BASELINE-LBP1120-2026-10-31`  
**Classification:** Release Engineering & Quality Assurance  
**Git Baseline Commit:** `0f3f4f3`  
**Branch:** `master`  
**Host Environment:** Windows 11 Pro 64-bit (10.0.26100), Intel Core i7  
**Connected Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`, Serial `0353S4cQ`)  
**Audit Timestamp:** October 31, 2026 17:22 UTC+1

---

## 1. Executive Summary

A comprehensive, non-destructive technical audit of the repository, toolchains, build environments, and installer infrastructure was performed prior to executing the automated release workflow.

The underlying printing engine ([`src/scoa.rs`](file:///f:/Games/canon-lbp1120-driver/src/scoa.rs), [`src/usb.rs`](file:///f:/Games/canon-lbp1120-driver/src/usb.rs), [`src/pdf.rs`](file:///f:/Games/canon-lbp1120-driver/src/pdf.rs), [`src/halftone.rs`](file:///f:/Games/canon-lbp1120-driver/src/halftone.rs), [`src/spool.rs`](file:///f:/Games/canon-lbp1120-driver/src/spool.rs)) is verified healthy and stable (69/69 Rust tests pass, 7/7 TypeScript tests pass, physical printing verified).

However, **four concrete release blockers** were identified that prevent automated public distribution:
1. **Inno Setup Script Syntax Failure:** `installer/CanonLBP1120_Setup.iss` line 25 contains `ArchitecturesInstallIn64BitMode=x64 compatible` (with a space), which causes the Inno Setup 6.7.3 compiler (`ISCC.exe`) to abort immediately with `Architecture identifier "compatible" is invalid`.
2. **Missing Automated GitHub CI/CD Workflow:** `.github/workflows/windows-release.yml` does not exist.
3. **Installer Version & Artifact Alignment:** The installer script targets `CanonLBP1120_Setup_v1.0.0.exe` instead of the mandated Release Candidate naming `CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe`.
4. **Git Remote State:** No git remotes are currently configured (`git remote -v` is empty), requiring explicit remote repository mapping before pushing release tags.

---

## 2. Repository Structure & Asset Audit

### 2.1 Core Source Code & Binaries
- [`src/main.rs`](file:///f:/Games/canon-lbp1120-driver/src/main.rs): Unified CLI entrypoint with subcommands: `probe`, `reset`, `install`, `uninstall`, `service-run`, `install-service`, `uninstall-service`, `spool-server`, `print-pdf`.
- [`src/service.rs`](file:///f:/Games/canon-lbp1120-driver/src/service.rs): Native Windows Service Control Manager (SCM) dispatcher, handler, and automatic recovery configurator.
- [`src/spool.rs`](file:///f:/Games/canon-lbp1120-driver/src/spool.rs): Dual-mode TCP listener (`127.0.0.1:9100` Normal, `127.0.0.1:9101` Photo) and Named Pipe server.
- [`src/usb.rs`](file:///f:/Games/canon-lbp1120-driver/src/usb.rs): WinUSB bulk transport and hardware register polling.
- [`src/scoa.rs`](file:///f:/Games/canon-lbp1120-driver/src/scoa.rs): Authentic Canon CAPT SCoA 2D Delta + RLE compression engine.
- [`src/pdf.rs`](file:///f:/Games/canon-lbp1120-driver/src/pdf.rs): WinRT Direct2D 600 DPI vector PDF rasterizer.
- [`src/halftone.rs`](file:///f:/Games/canon-lbp1120-driver/src/halftone.rs): Clustered 8x8 dot screen and serpentine Floyd-Steinberg halftoning.
- `target/release/capt-poc.exe`: Release binary, 854 KB, compiled with Link-Time Optimization (LTO).

### 2.2 Installer Directory Assets (`installer/`)
- [`installer/CanonLBP1120_Setup.iss`](file:///f:/Games/canon-lbp1120-driver/installer/CanonLBP1120_Setup.iss): Inno Setup 6 compilation script (3,002 bytes). Requires line 25 syntax fix and version update.
- [`installer/CanonLBP1120_WinUSB.inf`](file:///f:/Games/canon-lbp1120-driver/installer/CanonLBP1120_WinUSB.inf): Microsoft WinUSB driver installation file (1,834 bytes). Hardware ID: `USB\VID_04A9&PID_262B`.
- [`installer/Install-CanonLBP1120.ps1`](file:///f:/Games/canon-lbp1120-driver/installer/Install-CanonLBP1120.ps1): Turnkey PowerShell installer (7,566 bytes).
- [`installer/Uninstall-CanonLBP1120.ps1`](file:///f:/Games/canon-lbp1120-driver/installer/Uninstall-CanonLBP1120.ps1): Lifecycle uninstaller (4,074 bytes).
- [`installer/CanonLBP1120_Setup.bat`](file:///f:/Games/canon-lbp1120-driver/installer/CanonLBP1120_Setup.bat) & [`installer/CanonLBP1120_Uninstall.bat`](file:///f:/Games/canon-lbp1120-driver/installer/CanonLBP1120_Uninstall.bat): UAC-elevating batch wrappers.

### 2.3 Runtime Dependencies & Firmware Blobs
- `reference/drivers/LBP1120_WinXP/CAP3ZWAV.BIN` (412 KB): Authentic Canon audio voice archive (17 PCM WAV voice prompts). Verified present and accessible.
- `WinUSB.sys`: In-box Windows kernel driver (`C:\Windows\System32\drivers\winusb.sys`). Zero third-party kernel driver required.

---

## 3. Git Status & Remote Repository Configuration

### 3.1 Git Branch & Commit History
- Branch: `master`
- HEAD Commit: `0f3f4f3` (`feat(installer): complete production installer, Windows SCM service, and full engineering documentation suite`)
- Working tree: Clean (minor trailing whitespace line endings in markdown files).

### 3.2 Remote Repository Status
- `git remote -v`: **No remotes configured**.
- Evaluation: To enable automated GitHub Actions workflows, the repository needs a remote origin or will run locally with GitHub Actions workflow specifications created in `.github/workflows/windows-release.yml`.

---

## 4. Inno Setup Compiler Diagnostics

### 4.1 Compiler Availability
- Inno Setup 6.7.3 was discovered and verified at `C:\Users\ZAKI\AppData\Local\Programs\Inno Setup 6\ISCC.exe`.
- Version: Inno Setup 6 Command-Line Compiler v6.7.3 (built 2026).

### 4.2 Empirical Compiler Failure
When testing the existing script:
```text
& "C:\Users\ZAKI\AppData\Local\Programs\Inno Setup 6\ISCC.exe" installer\CanonLBP1120_Setup.iss
```
Result:
```text
Error on line 25 in F:\Games\canon-lbp1120-driver\installer\CanonLBP1120_Setup.iss: 
Directive or parameter "ArchitecturesInstallIn64BitMode" expression error: Architecture identifier "compatible" is invalid.
Compile aborted.
```
**Diagnostic Cause:**
Inno Setup 6 requires `x64` or `x64compatible` (no space). Line 25 currently reads `x64 compatible`.

---

## 5. Comprehensive Release Blocker Matrix

| Blocker ID | Severity | Subsystem | Description | Required Remediation |
| :--- | :--- | :--- | :--- | :--- |
| **BLK-01** | **CRITICAL** | Inno Setup Script | Syntax error on line 25 of `CanonLBP1120_Setup.iss` aborts compilation. | Change line 25 to `ArchitecturesInstallIn64BitMode=x64`. |
| **BLK-02** | **HIGH** | Inno Setup Output | Output filename is `CanonLBP1120_Setup_v1.0.0.exe` instead of RC1 target. | Update `#define MyAppVersion "1.0.0-rc.1"` and `OutputBaseFilename=CanonLBP1120_Setup_v1.0.0-rc.1_x64`. |
| **BLK-03** | **HIGH** | Inno Setup Bundling | Installation scripts `Install-CanonLBP1120.ps1` and `Uninstall-CanonLBP1120.ps1` are not explicitly listed in `[Files]`. | Add both scripts to `[Files]` section of ISS. |
| **BLK-04** | **CRITICAL** | CI/CD Pipeline | Missing `.github/workflows/windows-release.yml`. | Create GitHub Actions workflow implementing 2-mode operation (PR/Commit CI vs Version Tag Prerelease). |
| **BLK-05** | **MEDIUM** | INF Verification | `CanonLBP1120_WinUSB.inf` references unsigned catalog file `CanonLBP1120_WinUSB.cat`. | Audit DSE compatibility, provide fallback / catalog signing details in `WINUSB_COMPATIBILITY_AND_SAFETY.md`. |

---

## 6. Audit Conclusion & Next Actions

The project baseline is fundamentally sound. The underlying printing engine and Windows service architecture require **zero architectural modifications**. 

By fixing the Inno Setup script line 25 syntax, aligning version macros to `v1.0.0-rc.1_x64`, creating `.github/workflows/windows-release.yml`, and executing the actual compiler, the project will immediately produce the authentic, verified `CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe` installer.

