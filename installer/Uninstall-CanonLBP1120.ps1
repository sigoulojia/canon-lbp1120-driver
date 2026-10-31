# ==============================================================================
# Canon LaserShot LBP-1120 Safe Lifecycle Uninstaller
# ==============================================================================

[CmdletBinding()]
param(
    [string]$TargetDir = "C:\Program Files\CanonLBP1120",
    [switch]$PurgeLogs
)

$ErrorActionPreference = "Continue"

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "   CANON LASERSHOT LBP-1120 UNINSTALLER               " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

# 1. Elevation Check
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Warning "Administrator privileges required. Requesting elevation..."
    Start-Process powershell.exe -ArgumentList ("-NoProfile -ExecutionPolicy Bypass -File `"{0}`"" -f $MyInvocation.MyCommand.Path) -Verb RunAs
    exit
}

$ExePath = Join-Path $TargetDir "canon-lbp1120-service.exe"

# 2. Stop and unregister Windows Service
Write-Host "`n[1/5] Stopping and removing Windows Service 'CanonLBP1120Service'..." -ForegroundColor Yellow
if (Test-Path $ExePath) {
    & "$ExePath" uninstall-service | Out-Null
} else {
    sc.exe stop CanonLBP1120Service | Out-Null
    sc.exe delete CanonLBP1120Service | Out-Null
}
Get-Process -Name capt-poc, canon-lbp1120-service -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# 3. Remove printer queues and ports
Write-Host "[2/5] Removing printer queues & TCP ports..." -ForegroundColor Yellow
if (Test-Path $ExePath) {
    & "$ExePath" uninstall | Out-Null
} else {
    Remove-Printer -Name "Canon LaserShot LBP-1120" -ErrorAction SilentlyContinue
    Remove-Printer -Name "Canon LaserShot LBP-1120 (Photo)" -ErrorAction SilentlyContinue
    Remove-PrinterPort -Name "CanonLBP1120_Normal" -ErrorAction SilentlyContinue
    Remove-PrinterPort -Name "CanonLBP1120_Photo" -ErrorAction SilentlyContinue
    Remove-PrinterPort -Name "\\.\pipe\CanonLBP1120" -ErrorAction SilentlyContinue
    Remove-PrinterPort -Name "\\.\pipe\CanonLBP1120_Photo" -ErrorAction SilentlyContinue
}
Write-Host "      Printer queues and ports removed." -ForegroundColor Green

# 4. Remove Start Menu folder
Write-Host "[3/5] Cleaning Start Menu shortcuts..." -ForegroundColor Yellow
$StartMenuDir = Join-Path $env:ProgramData "Microsoft\Windows\Start Menu\Programs\Canon LaserShot LBP-1120"
if (Test-Path $StartMenuDir) {
    Remove-Item -Path $StartMenuDir -Recurse -Force -ErrorAction SilentlyContinue
    Write-Host "      Start Menu folder removed." -ForegroundColor Green
}

# 5. Remove application files
Write-Host "[4/5] Removing application binaries in '$TargetDir'..." -ForegroundColor Yellow
if (Test-Path $TargetDir) {
    Remove-Item -Path "$TargetDir\*" -Recurse -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 300
    Remove-Item -Path $TargetDir -Force -ErrorAction SilentlyContinue
    Write-Host "      Application directory removed." -ForegroundColor Green
}

# 6. Log cleanup
Write-Host "[5/5] Managing log records..." -ForegroundColor Yellow
$LogDir = "C:\ProgramData\CanonLBP1120"
if ($PurgeLogs) {
    if (Test-Path $LogDir) {
        Remove-Item -Path $LogDir -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "      Log directory purged." -ForegroundColor Green
    }
} else {
    Write-Host "      Preserved telemetry logs at '$LogDir\service.log' (Use -PurgeLogs to delete)." -ForegroundColor Cyan
}

Write-Host "`n=======================================================" -ForegroundColor Green
Write-Host "   UNINSTALLATION COMPLETE                             " -ForegroundColor Green
Write-Host "=======================================================" -ForegroundColor Green
Write-Host "Canon LaserShot LBP-1120 driver and service have been completely removed from this system."

