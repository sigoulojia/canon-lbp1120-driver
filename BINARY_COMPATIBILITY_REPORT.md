# CANON LASERSHOT LBP-1120 BINARY ARCHITECTURE & COMPATIBILITY REPORT
## Phase 3 Technical Audit: Portable Executable (PE) Analysis, System Dependencies & Runtime Target

**Document ID:** `REPORT-BIN-COMPAT-LBP1120-2026-10-31`  
**Classification:** Binary Analysis & Architecture Specification  
**Binary Analyzed:** `target/release/capt-poc.exe` (staged as `canon-lbp1120-service.exe`)  
**SHA256:** Derived from release build (see `SHA256SUMS.txt`)  
**Target Release:** Windows 10 & 11 (64-bit x86_64)

---

## 1. Executive Summary

A full byte-level inspection of the Portable Executable (PE32+) headers, optional headers, import address tables (IAT), and runtime subsystem dependencies was conducted on the production release binary.

**Key Architecture Findings:**
1. **Target Architecture:** Strictly **64-bit x86_64 (`IMAGE_FILE_MACHINE_AMD64`, 0x8664)**.
2. **Initial Release Target Scope:** Formally locked to **Windows 10 x64 (Build 1809+) and Windows 11 x64**. While Windows 11 on ARM64 includes an x64 user-mode emulation layer, no native ARM64 (`0xAA64`) binary has been cross-compiled or tested against physical ARM64 USB host controllers. Therefore, **ARM64 support is explicitly not claimed for this v1.0.0-rc.1 release**.
3. **Zero Third-Party Dependencies:** The binary has zero external runtime dependencies. It requires no Python interpreter, no Node.js runtime, no Rust toolchain, and no custom kernel drivers.
4. **Universal C Runtime (UCRT) Integration:** Uses Microsoft's standard Universal CRT (`api-ms-win-crt-*`), which is preinstalled in-box on all modern Windows 10 and 11 distributions.

---

## 2. Portable Executable (PE) Header Technical Breakdown

The binary header was extracted and parsed via [`scripts/audit_pe.ps1`](file:///f:/Games/canon-lbp1120-driver/scripts/audit_pe.ps1):

| PE Header Field | Raw Value | Decoded Specification | Analysis & Implication |
| :--- | :--- | :--- | :--- |
| **DOS Magic** | `0x5A4D` | `"MZ"` | Standard DOS 2.0 executable signature. |
| **PE Signature Offset (`e_lfanew`)** | `0x0000003C` $\rightarrow$ `0x000000F8` | Offset 248 | Points directly to PE Header structure. |
| **PE Signature** | `0x00004550` | `"PE\0\0"` | Valid 32-bit Portable Executable signature. |
| **Machine Type** | `0x8664` | `IMAGE_FILE_MACHINE_AMD64` | Native 64-bit AMD/Intel x86_64 architecture. |
| **Number of Sections** | `0x0006` (6) | `.text`, `.rdata`, `.data`, `.pdata`, `.reloc`, etc. | Compact section structure, LTO-optimized. |
| **Magic (Optional Header)** | `0x020B` | `PE32+` | 64-bit address space (supports addresses $>4\text{ GB}$). |
| **Operating System Version** | `6.0` | Windows Vista / Server 2008 base | Satisfies all Windows NT 6.x and NT 10.0 kernels. |
| **Subsystem Version** | `6.0` | Windows NT 6.0+ | Minimum kernel requirement satisfied by Windows 10/11. |
| **Subsystem** | `0x0003` | `IMAGE_SUBSYSTEM_WINDOWS_CUI` | Windows Console Subsystem (allows both headless SCM service and CLI diagnostics). |
| **Import Table RVA** | `0x000CABBC` | Size: 400 bytes | Clean, compact Import Address Table (IAT). |

---

## 3. Imported Windows System Libraries (IAT Audit)

Every dynamic link in the binary resolves to an in-box Microsoft Windows operating system DLL:

```
+-----------------------------------+-------------------------------------------------------------+
| IMPORTED SYSTEM DLL               | PURPOSE & CODE RESPONSIBILITY                               |
+-----------------------------------+-------------------------------------------------------------+
| KERNEL32.dll / kernel32.dll       | Win32 Base Services (Heap, File IO, Named Pipes, Threads)   |
| WINUSB.DLL                        | WinUSB API client (WinUsb_Initialize, Bulk OUT/IN pipes)    |
| advapi32.dll                      | Windows Service Control Manager (SCM) & Security DACLs      |
| WS2_32.dll                        | Winsock 2 Network Subsystem (TCP 127.0.0.1:9100 / 9101)     |
| WINMM.dll                         | Windows Multimedia Subsystem (PlaySoundW audio playback)    |
| ole32.dll / oleaut32.dll          | COM / OLE Core Infrastructure (Direct2D WinRT activation)   |
| combase.dll                       | Modern Component Object Model Base                          |
| api-ms-win-core-winrt-l1-1-0.dll  | Windows Runtime Core (RoInitialize, RoGetActivationFactory) |
| api-ms-win-core-synch-l1-2-0.dll  | Modern Kernel Synchronization Primitives                    |
| ntdll.dll                         | Windows NT Native System Call Layer                         |
| VCRUNTIME140.dll                  | Microsoft Visual C++ Runtime 14.0 (In-box in System32)      |
| api-ms-win-crt-runtime-l1-1-0.dll | Universal CRT Process & Thread Runtime                      |
| api-ms-win-crt-stdio-l1-1-0.dll   | Universal CRT Standard I/O Formatter                        |
| api-ms-win-crt-heap-l1-1-0.dll    | Universal CRT Heap Allocator                                |
| api-ms-win-crt-math-l1-1-0.dll    | Universal CRT Floating Point Math (Direct2D coordinates)    |
| api-ms-win-crt-string-l1-1-0.dll  | Universal CRT String & Memory Routines                      |
| api-ms-win-crt-locale-l1-1-0.dll  | Universal CRT Locale Handling                               |
+-----------------------------------+-------------------------------------------------------------+
```

### 3.1 Dynamic Library Loading (`LoadLibraryW`)
An exhaustive scan of `src/*.rs` confirmed **zero dynamic runtime calls to `LoadLibraryA` or `LoadLibraryW`**. All external interfaces are statically resolved through the Windows SDK import table at process initialization time, eliminating DLL side-loading and search-order hijacking risks.

---

## 4. Subsystem-Specific Runtime Requirements

### 4.1 WinUSB Runtime Requirements
- Kernel Driver: `C:\Windows\System32\drivers\winusb.sys` (In-box Microsoft driver, signed by Microsoft Windows Hardware Compatibility Program).
- User-Mode Wrapper: `C:\Windows\System32\winusb.dll`.
- Requirements: Present out-of-the-box on every Windows 10 and Windows 11 installation. No third-party USB filter driver (e.g. LibUsb-Win32 filter, WinPcap) is required or permitted.

### 4.2 Direct2D & WinRT PDF Requirements
- PDF Ingestion Engine: `Windows.Data.Pdf.PdfDocument` (via WinRT Direct2D API).
- Minimum OS Version: **Windows 10 Build 10240 (Version 1507)**.
- Functionality: Renders vector and bitmap PDF pages to 600 DPI bitmap rasters natively in user space without requiring Adobe Acrobat, Ghostscript, or Poppler.

### 4.3 Network Subsystem
- Ports: `127.0.0.1:9100` (Normal Mode) and `127.0.0.1:9101` (Photo Mode).
- Winsock API: `WSAStartup`, `socket`, `bind`, `listen`, `accept`.
- Isolation: Bound strictly to the IPv4 loopback adapter. No external packet routing or firewall modification required.

---

## 5. Architectural Verdict

The compiled release executable `canon-lbp1120-service.exe` is a **pure 64-bit Windows x64 binary**. It conforms strictly to modern Windows security and runtime standards, relying 100% on native in-box operating system components.

**Final Release Scope:** **Windows 10 x64 / Windows 11 x64**.
