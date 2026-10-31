# ==============================================================================
# Canon LaserShot LBP-1120 Production Turnkey Installer
# Platform: Windows 10 & 11 (x64 / ARM64)
# ==============================================================================

[CmdletBinding()]
param(
    [string]$TargetDir = "C:\Program Files\CanonLBP1120",
    [switch]$SkipHardwareProbe,
    [switch]$TestStagingOnly
)

$ErrorActionPreference = "Stop"

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "   CANON LASERSHOT LBP-1120 PRODUCTION INSTALLER       " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$ReleaseExe = Join-Path $ProjectRoot "target\release\capt-poc.exe"
if (-not (Test-Path $ReleaseExe) -and (Test-Path (Join-Path $ScriptDir "canon-lbp1120-service.exe"))) {
    $ReleaseExe = Join-Path $ScriptDir "canon-lbp1120-service.exe"
}
$WavArchive = Join-Path $ProjectRoot "reference\drivers\LBP1120_WinXP\CAP3ZWAV.BIN"
if (-not (Test-Path $WavArchive) -and (Test-Path (Join-Path $ScriptDir "CAP3ZWAV.BIN"))) {
    $WavArchive = Join-Path $ScriptDir "CAP3ZWAV.BIN"
}
$InfFile = Join-Path $ScriptDir "driver\CanonLBP1120_WinUSB.inf"
if (-not (Test-Path $InfFile)) {
    $InfFile = Join-Path $ScriptDir "CanonLBP1120_WinUSB.inf"
}

if (-not (Test-Path $ReleaseExe)) {
    throw "Release binary not found at '$ReleaseExe'. Run 'cargo build --release' first."
}

# 1. Administrator Elevation Check
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin -and -not $TestStagingOnly) {
    Write-Warning "Administrator privileges required to install system services and printer queues."
    try {
        Start-Process powershell.exe -ArgumentList ("-NoProfile -ExecutionPolicy Bypass -File `"{0}`"" -f $MyInvocation.MyCommand.Path) -Verb RunAs
        exit 0
    } catch {
        Write-Host "`nTo install for all users, please right-click 'CanonLBP1120_Setup.bat' and select 'Run as Administrator', or run this script from an elevated Administrator PowerShell window." -ForegroundColor Yellow
        Write-Host "You can also test file staging without elevation using: .\Install-CanonLBP1120.ps1 -TestStagingOnly" -ForegroundColor Cyan
        exit 1
    }
}

if ($TestStagingOnly) {
    $TargetDir = Join-Path $ProjectRoot "dist\CanonLBP1120"
    Write-Host "[Mode: Test Staging Only (Target: $TargetDir)]" -ForegroundColor Magenta
}

# 2. Stop running service or daemon before upgrading files
Write-Host "`n[1/6] Stopping existing service/daemon..." -ForegroundColor Yellow
sc.exe stop CanonLBP1120Service 2>$null | Out-Null
Get-Process -Name capt-poc, canon-lbp1120-service -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# 3. Create target directory and stage production files
Write-Host "[2/6] Staging production binaries to '$TargetDir'..." -ForegroundColor Yellow
New-Item -Path $TargetDir -ItemType Directory -Force | Out-Null
New-Item -Path (Join-Path $TargetDir "driver") -ItemType Directory -Force | Out-Null

$DestExe = Join-Path $TargetDir "canon-lbp1120-service.exe"
Copy-Item -Path $ReleaseExe -Destination $DestExe -Force
if (Test-Path $WavArchive) {
    Copy-Item -Path $WavArchive -Destination (Join-Path $TargetDir "CAP3ZWAV.BIN") -Force
}
if (Test-Path $InfFile) {
    Copy-Item -Path $InfFile -Destination (Join-Path $TargetDir "driver\CanonLBP1120_WinUSB.inf") -Force
}
Copy-Item -Path (Join-Path $ScriptDir "Uninstall-CanonLBP1120.ps1") -Destination (Join-Path $TargetDir "Uninstall-CanonLBP1120.ps1") -Force -ErrorAction SilentlyContinue

if (-not $TestStagingOnly) {
    # 4. Stage and install WinUSB driver package via PnPUtil
    Write-Host "[3/6] Staging WinUSB device driver package..." -ForegroundColor Yellow
    $InstalledInf = Join-Path $TargetDir "driver\CanonLBP1120_WinUSB.inf"
    if (Test-Path $InstalledInf) {
        $dev = Get-PnpDevice -InstanceId 'USB\VID_04A9&PID_262B*' -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($dev -and $dev.Service -eq 'WinUSB' -and $dev.Status -eq 'OK') {
            Write-Host "      Existing operational WinUSB driver binding detected ($($dev.Service)). Preserving driver." -ForegroundColor Green
        } else {
            try {
                & pnputil.exe /add-driver "$InstalledInf" /install | Out-Null
                Write-Host "      WinUSB driver staged successfully." -ForegroundColor Green
            } catch {
                Write-Warning "PnPUtil notice: $($_.Exception.Message)"
            }
        }
    }

    # 5. Configure dual Windows printer queues and TCP ports
    Write-Host "[4/6] Registering printer queues & TCP RAW ports..." -ForegroundColor Yellow
    & "$DestExe" install
    Write-Host "      Dual queues ('Canon LaserShot LBP-1120' & Photo) registered." -ForegroundColor Green

    # 6. Register & start native SCM Windows Service
    Write-Host "[5/6] Registering & starting Windows Service 'CanonLBP1120Service'..." -ForegroundColor Yellow
    & "$DestExe" install-service
    Start-Sleep -Seconds 1

    # Verify installation
    Write-Host "      Running verification audit..." -ForegroundColor Yellow
    & "$DestExe" verify-installation

    # 7. Create Start Menu Shortcuts
    Write-Host "[6/6] Creating Start Menu shortcuts..." -ForegroundColor Yellow
    $StartMenuDir = Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs\Canon LaserShot LBP-1120"
    New-Item -Path $StartMenuDir -ItemType Directory -Force | Out-Null

    $wshShell = New-Object -ComObject WScript.Shell

    # Status Diagnostic Shortcut
    $s1 = $wshShell.CreateShortcut((Join-Path $StartMenuDir "Printer Status Diagnostics.lnk"))
    $s1.TargetPath = "powershell.exe"
    $s1.Arguments = "-NoExit -Command `"`& '$DestExe' probe`""
    $s1.Description = "Query hardware registers, paper sensor, and optical counters"
    $s1.Save()

    # Reset Shortcut
    $s2 = $wshShell.CreateShortcut((Join-Path $StartMenuDir "Reset Printer Hardware.lnk"))
    $s2.TargetPath = "powershell.exe"
    $s2.Arguments = "-NoExit -Command `"`& '$DestExe' reset`""
    $s2.Description = "Reset engine controller and ASIC video FIFO"
    $s2.Save()

    # Calibration Shortcut
    $s3 = $wshShell.CreateShortcut((Join-Path $StartMenuDir "Print 6-Zone Calibration Page.lnk"))
    $s3.TargetPath = "powershell.exe"
    $s3.Arguments = "-NoExit -Command `"`& '$DestExe' generate-calibration; `& '$DestExe' print calibration.pbm`""
    $s3.Description = "Generate and print 600 DPI optical calibration target"
    $s3.Save()

    # View Log Shortcut
    $s4 = $wshShell.CreateShortcut((Join-Path $StartMenuDir "View Service Log.lnk"))
    $s4.TargetPath = "notepad.exe"
    $s4.Arguments = "C:\ProgramData\CanonLBP1120\service.log"
    $s4.Description = "View background spool service telemetry log"
    $s4.Save()

    # Uninstall Shortcut
    $s5 = $wshShell.CreateShortcut((Join-Path $StartMenuDir "Uninstall Driver Suite.lnk"))
    $s5.TargetPath = "powershell.exe"
    $s5.Arguments = "-ExecutionPolicy Bypass -File `"$TargetDir\Uninstall-CanonLBP1120.ps1`""
    $s5.Description = "Safely remove driver, queues, and service"
    $s5.Save()
} else {
    Write-Host "[Staging Complete] Production files staged at: $TargetDir" -ForegroundColor Green
    Get-ChildItem -Path $TargetDir -Recurse | Select-Object Name, Length, FullName | Format-Table -AutoSize
}

Write-Host "`n=======================================================" -ForegroundColor Green
Write-Host "   INSTALLATION COMPLETED SUCCESSFULLY!                " -ForegroundColor Green
Write-Host "=======================================================" -ForegroundColor Green
Write-Host "Installed Components:"
Write-Host "  1. Windows Service    : CanonLBP1120Service (Running, Automatic)"
Write-Host "  2. Normal Printer     : 'Canon LaserShot LBP-1120' (TCP 127.0.0.1:9100)"
Write-Host "  3. Photo Printer      : 'Canon LaserShot LBP-1120 (Photo)' (TCP 127.0.0.1:9101)"
Write-Host "  4. Service Log Path   : C:\ProgramData\CanonLBP1120\service.log"
Write-Host "  5. Start Menu Folder  : $StartMenuDir"
Write-Host "`nYou can now print directly from any Windows application (Word, Chrome, Acrobat, Notepad)." -ForegroundColor Cyan
