# CANON LASERSHOT LBP-1120 INSTALLATION & DEPLOYMENT GUIDE
## User & Administrator Installation Instructions for Windows 10 & Windows 11

**Document ID:** `GUIDE-INSTALL-LBP1120-2026-10-31`  
**Classification:** User & Deployment Documentation  
**Target Hardware:** Canon LaserShot LBP-1120 (`USB\VID_04A9&PID_262B`)  
**Supported Platforms:** Windows 10 (x64 / ARM64), Windows 11 (x64 / ARM64)

---

## 1. System Requirements & Prerequisites

This driver distribution is 100% self-contained. It requires **zero external runtimes or developer tools**:
- **No Rust compiler required.**
- **No Node.js / npm required.**
- **No Python required.**
- **No legacy 32-bit Windows XP software required.**
- **Operating System:** Clean installation of Windows 10 (Build 1809 or higher) or Windows 11 (all versions).
- **Architecture:** 64-bit x86_64 (`x64`) or ARM64.
- **Interface:** USB 1.1, 2.0, or 3.0 port.

---

## 2. Installation Methods

### Method 1: Turnkey Graphical Installer (Recommended for Users)
1. Download or locate `CanonLBP1120_Setup_v1.0.0.exe`.
2. Double-click the installer. When the Windows User Account Control (UAC) dialog appears, click **Yes** to grant administrator privileges.
3. Follow the setup wizard (defaults to `C:\Program Files\CanonLBP1120\`).
4. Click **Install**. The wizard will:
   - Stage the Microsoft WinUSB driver.
   - Configure the Normal Mode and Photo Mode printer queues.
   - Register and start the background Windows Service (`CanonLBP1120Service`).
   - Create Start Menu shortcuts.
5. Click **Finish**. The printer is now ready for use.

---

### Method 2: One-Click Administrator Batch Setup (For Portable / IT Deployment)
1. Open the driver package folder.
2. Right-click [`CanonLBP1120_Setup.bat`](file:///f:/Games/canon-lbp1120-driver/installer/CanonLBP1120_Setup.bat) and select **Run as Administrator**.
3. The script automatically executes [`Install-CanonLBP1120.ps1`](file:///f:/Games/canon-lbp1120-driver/installer/Install-CanonLBP1120.ps1), staging binaries, setting up ports, and starting the service.
4. When the console displays `INSTALLATION COMPLETED SUCCESSFULLY!`, press any key to exit.

---

## 3. Physical Printer Hardware Setup

1. **Connect USB Cable:** Plug the Canon LaserShot LBP-1120 USB cable into a USB port on your PC.
2. **Power On:** Flip the rocker power switch on the right side of the printer to the **ON** (`I`) position.
3. **Load Paper:** Place standard A4 paper into the cassette tray, pushing it gently until it rests against the feed rollers. Adjust the side guide clips to fit snugly against the paper.
4. **Close Top Cover:** Ensure the top toner cover is firmly closed.

---

## 4. Verifying Successful Installation

### 4.1 Verify Windows Printer Queues
1. Open **Settings** $\rightarrow$ **Bluetooth & devices** $\rightarrow$ **Printers & scanners**.
2. Confirm the presence of two new printers:
   - **`Canon LaserShot LBP-1120`** (Default queue for text, Word documents, PDFs, and web pages).
   - **`Canon LaserShot LBP-1120 (Photo)`** (Optimized queue for continuous-tone photography and halftones).

### 4.2 Verify Service Status
1. Press `Win + R`, type `services.msc`, and press **Enter**.
2. Locate **Canon LaserShot LBP-1120 Print Service**.
3. Verify that the **Status** is `Running` and **Startup Type** is `Automatic`.

### 4.3 Run Hardware Diagnostics
1. Click the Windows **Start** button $\rightarrow$ All Apps $\rightarrow$ **Canon LaserShot LBP-1120**.
2. Click **Printer Status Diagnostics**.
3. A console window will open, communicating directly with the printer via WinUSB:
   ```text
   ==================================================
      CANON LBP-1120 NON-DESTRUCTIVE HARDWARE PROBE  
   ==================================================
   Connecting to printer via WinUSB... CONNECTED!
   IEEE 1284 Device ID: Canon LASER SHOT LBP-1120
   Port Status        : Selected=YES (Online), Healthy=YES
   Top Door / Cover   : CLOSED (OK)
   Toner Cartridge    : INSTALLED (OK)
   Paper Presence     : LOADED (OK)
   Engine State       : READY
   ```

---

## 5. Printing Your First Page

1. Open **Notepad**, type a few lines of text (or open any Word / PDF document).
2. Click **File** $\rightarrow$ **Print** (or press `Ctrl + P`).
3. Select **Canon LaserShot LBP-1120**.
4. Click **Print**.
5. **Notice:**
   - **No "Save As" file prompt appears.**
   - The document spools instantly.
   - An authentic voice alert announces *"Printing has started"*.
   - The pickup roller engages, the sheet is fed, toner is fused at 185°C, and the completed page is delivered into the face-down output tray.

---

## 6. Troubleshooting Common Situations

| Symptom | Cause | Solution |
| :--- | :--- | :--- |
| **Voice alert says "Load paper into the printer"** | Sensor `PS202` detects empty cassette tray. | Add A4 paper to the paper cassette. The printer automatically resumes once paper is detected. |
| **Voice alert says "Cover is open"** | Microswitch `SW301` open. | Push the top door down until it clicks securely into place. |
| **Job fails with "Printer not detected"** | USB cable disconnected or printer powered off. | Ensure the power switch is ON and the USB cable is firmly seated. The service automatically reconnects. |
| **Printer red error LED flashes** | Transient ASIC buffer state. | Run the **Reset Printer Hardware** shortcut from the Start Menu, or power cycle the printer for 5 seconds. |
| **Jobs stuck in Windows queue** | Prior failed print session. | Run `canon-lbp1120-service.exe reset` or restart the `CanonLBP1120Service` in `services.msc`. |

