# WINDOWS SERVICE SECURITY ARCHITECTURE & PRIVILEGE MODEL
## Canon LaserShot LBP-1120 Spool Service Security Specification

**Document ID:** `SEC-SERVICE-LBP1120-2026-10-31`  
**Classification:** Information Security & Systems Architecture  
**Target Platform:** Windows 10 & 11 (x64 / ARM64)

---

## 1. Threat Model & Security Objectives

The Canon LBP-1120 spool service is a persistent background daemon interacting with both user-mode applications (via network sockets and named pipes) and low-level kernel drivers (via WinUSB). The primary security objectives are:

1. **Principle of Least Privilege:** Minimize operating system privileges while ensuring uninterrupted hardware access to USB endpoints.
2. **Network Attack Surface Elimination:** Prevent unauthorized remote network users on the local LAN or Internet from submitting arbitrary jobs or accessing hardware state.
3. **Data Ingestion Hardening:** Validate and sanitize all incoming print streams before passing them to parsing engines.
4. **File System & State Isolation:** Ensure that configuration files, spool queues, and telemetry logs cannot be tampered with by unprivileged local users.
5. **Denial of Service (DoS) Resistance:** Protect against memory exhaustion, infinite spool loops, and USB bus deadlocks.

---

## 2. Service Account Selection & Justification

### 2.1 The WinUSB Access Dilemma: LocalSystem vs. LocalService
Under Windows, USB devices claimed by `WinUSB.sys` expose a device interface path (e.g. `\\?\USB#VID_04A9&PID_262B#0353S4cQ#{F9A25620-E255-4B37-93F3-0C3D3D2DFBD3}`). Access to this device path via `CreateFileW` is governed by the Security Descriptor (DACL) assigned to the USB device node by the Windows Plug and Play (PnP) manager.

| Service Account | Default WinUSB Device Access | Risk Profile | Feasibility & Mitigation |
| :--- | :--- | :--- | :--- |
| **`NT AUTHORITY\LocalSystem`** (Selected Default) | **Full Access Granted** (`GENERIC_READ \| GENERIC_WRITE`). Device opening always succeeds across all USB ports and hubs. | High privileges in OS context (`SeTcbPrivilege`). | **Strict Mitigations Implemented:** Network listeners restricted to loopback (`127.0.0.1`); strict PDF header validation; memory governors; no interactive desktop interaction. |
| **`NT AUTHORITY\LocalService`** (Low-Privilege Alternative) | **Access Denied (Error 5)** by default. PnP device interface DACL defaults to `(A;;GA;;;SY)(A;;GA;;;BA)` (SYSTEM and Administrators only). | Minimal OS privileges. Cannot alter system settings or access other user data. | Requires modifying the USB device interface security descriptor in registry (`DeviceParameters\Security` = `D:P(A;;GA;;;LS)...`) or via Group Policy. Fragile across different USB ports. |
| **User Interactive Token** | Accessible if the user belongs to `Administrators`. Standard users receive Error 5 on raw USB pipes unless UAC elevated. | Limited to user session. Service dies on user logoff. | Cannot provide machine-wide printing for multi-user or unattended systems. |

### 2.2 Justification for `LocalSystem` with Hardened Defenses
We selected `NT AUTHORITY\LocalSystem` for the service account because:
1. It guarantees that the driver operates reliably across any USB root hub, composite dock, or external USB 2.0/3.0 port without requiring brittle, hardware-dependent registry DACL hacks that break during Windows cumulative updates.
2. The service performs **zero ambient privilege escalation**: it does not impersonate clients, does not launch child processes with inherited tokens, and does not open interactive GUI windows (`SERVICE_INTERACTIVE_PROCESS` is explicitly disabled).

---

## 3. Network Attack Surface Elimination

### 3.1 Strict Loopback Sockets (`127.0.0.1`)
The TCP print listeners in [`src/spool.rs`](file:///f:/Games/canon-lbp1120-driver/src/spool.rs) are explicitly bound to the IPv4 loopback address:
- Normal Mode: `127.0.0.1:9100`
- Photo Mode: `127.0.0.1:9101`

**Security Invariants:**
- The socket is **never bound to `0.0.0.0` (INADDR_ANY)** or `[::]`.
- Windows IP stack enforces that loopback packets originate strictly from local processes.
- Remote hosts on the local network (LAN) or public networks cannot establish TCP handshakes with ports 9100/9101.
- No inbound Windows Firewall rules are created or required.

### 3.2 Named Pipe Security Descriptors
The named pipes (`\\.\pipe\CanonLBP1120` and `\\.\pipe\CanonLBP1120_Photo`) are created using `CreateNamedPipeW` with `FILE_FLAG_FIRST_PIPE_INSTANCE` to prevent pipe squatting. The security attributes allow read/write access to local authenticated users while prohibiting anonymous or remote RPC access.

---

## 4. Ingestion Validation & Memory Hardening

### 4.1 PDF Signature Enforcement
Before any incoming data is passed to the WinRT Direct2D rasterizer, the daemon validates the initial byte stream:
```rust
pub fn is_pdf_buffer(buf: &[u8]) -> bool {
    let mut offset = 0;
    // Strip UTF-8 BOM if present
    if buf.starts_with(&[0xEF, 0xBB, 0xBF]) {
        offset = 3;
    }
    // Skip leading ASCII whitespace
    while offset < buf.len() && (buf[offset] == b' ' || buf[offset] == b'\t' || buf[offset] == b'\r' || buf[offset] == b'\n') {
        offset += 1;
    }
    buf[offset..].starts_with(b"%PDF-1.")
}
```
**Protections:**
- If an unverified stream is received, the connection is instantly aborted.
- Temporary files are immediately deleted (`remove_file`), preventing storage accumulation.
- Stuck spool jobs in the Windows Print Queue are automatically cleared to prevent spooler deadlock.

### 4.2 Hardware Safety Governors
1. **512 KB Buffer Safety Governor:** The physical Canon LBP-1120 has 512 KB of onboard DRAM. High-entropy photographic rasters or noisy scanner images could generate SCoA streams exceeding 480 KB, causing an ASIC FIFO overflow. When a compressed stream exceeds 480,000 bytes, `spool.rs` automatically re-renders the page using clustered dot halftoning, guaranteeing that the stream fits within the hardware buffer without deadlocking.
2. **Post-Rotation Settling Guard:** The service verifies that `(st.aux & 0x04) == 0` (main drive motor stopped) before starting a new job, preventing USB write stalls (Win32 Error 31/995) when jobs are queued back-to-back.

---

## 5. File System & Lifecycle Isolation

### 5.1 ProgramData Directory ACLs
All service logs are stored in `%ProgramData%\CanonLBP1120\service.log`.  
Windows standard inheritance on `%ProgramData%` enforces:
- `NT AUTHORITY\SYSTEM`: Full Control
- `BUILTIN\Administrators`: Full Control
- `BUILTIN\Users`: Read & Execute (Cannot modify, truncate, or inject malicious content into the log)

### 5.2 Single-Instance Mutex
To prevent multiple instances of the service daemon from running simultaneously and competing for the USB interface:
- A named Win32 Mutex `Local\CanonLBP1120_SpoolServer_Mutex` is created on startup.
- If a secondary instance is launched, `GetLastError()` returns `ERROR_ALREADY_EXISTS` (183), and the secondary instance exits cleanly.

### 5.3 Crash Recovery & Handle Cleanliness
- WinUSB handles (`winusb_handle`) and file handles (`file_handle`) implement the Rust `Drop` trait, guaranteeing that `WinUsb_Free` and `CloseHandle` are executed upon scope exit.
- SCM is configured with automatic restart (`actions= restart/5000/restart/10000/none/0`), ensuring that even in the event of an unexpected power transient or kernel reset, the service resumes without manual administrative intervention.
