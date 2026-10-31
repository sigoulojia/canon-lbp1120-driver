# ==============================================================================
# Canon LaserShot LBP-1120 Release Integrity & Packaging Test Suite
# ==============================================================================

$ErrorActionPreference = "Stop"

Write-Host "=======================================================" -ForegroundColor Cyan
Write-Host "   CANON LBP-1120 RELEASE INTEGRITY AUTOMATED TEST     " -ForegroundColor Cyan
Write-Host "=======================================================" -ForegroundColor Cyan

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

$PassCount = 0
$FailCount = 0

function Assert-Condition($name, $condition, $details) {
    if ($condition) {
        Write-Host "  [PASS] $name" -ForegroundColor Green
        $script:PassCount++
    } else {
        Write-Host "  [FAIL] ${name} - $details" -ForegroundColor Red
        $script:FailCount++
    }
}

Write-Host "`n[Suite 1: Binary Existence & Architecture Checks]" -ForegroundColor Yellow
$releaseExe = Join-Path $ProjectRoot "target\release\capt-poc.exe"
Assert-Condition "Release binary exists" (Test-Path $releaseExe) "File not found at $releaseExe"

if (Test-Path $releaseExe) {
    $item = Get-Item $releaseExe
    Assert-Condition "Release binary size is non-trivial" ($item.Length -gt 500000) "Size was $($item.Length) bytes"
    
    # Check PE header machine type 0x8664
    $bytes = [System.IO.File]::ReadAllBytes($item.FullName)
    $e_lfanew = [BitConverter]::ToInt32($bytes, 0x3C)
    $machine = [BitConverter]::ToUInt16($bytes, $e_lfanew + 4)
    Assert-Condition "PE Machine Type is AMD64 (0x8664)" ($machine -eq 0x8664) ("Machine type was 0x{0:X4}" -f $machine)
}

Write-Host "`n[Suite 2: Installer Compilation & Executable Checks]" -ForegroundColor Yellow
$installerExe = (Get-ChildItem -Path (Join-Path $ProjectRoot "installer") -Filter "CanonLBP1120_Setup_*_x64.exe" | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
Assert-Condition "Inno Setup compiled installer exists" ($installerExe -and (Test-Path $installerExe)) "No installer found in installer\ directory"

if (Test-Path $installerExe) {
    $instItem = Get-Item $installerExe
    Assert-Condition "Compiled installer size is > 1 MB" ($instItem.Length -gt 1000000) "Size was $($instItem.Length) bytes"
    
    # Check PE header of installer
    $instBytes = [System.IO.File]::ReadAllBytes($instItem.FullName)
    $inst_lfanew = [BitConverter]::ToInt32($instBytes, 0x3C)
    $instSig = [BitConverter]::ToUInt32($instBytes, $inst_lfanew)
    Assert-Condition "Installer has valid PE signature" ($instSig -eq 0x00004550) ("Signature was 0x{0:X8}" -f $instSig)
}

Write-Host "`n[Suite 3: Hardware ID & INF Configuration Checks]" -ForegroundColor Yellow
$infPath = Join-Path $ProjectRoot "installer\CanonLBP1120_WinUSB.inf"
Assert-Condition "WinUSB INF exists" (Test-Path $infPath) "File not found at $infPath"

if (Test-Path $infPath) {
    $infContent = Get-Content $infPath -Raw
    Assert-Condition "INF matches VID 04A9" ($infContent -match "VID_04A9") "VID 04A9 not found in INF"
    Assert-Condition "INF matches PID 262B" ($infContent -match "PID_262B") "PID 262B not found in INF"
    Assert-Condition "INF sets DeviceInterfaceGUID" ($infContent -match "F9A25620-E255-4B37-93F3-0C3D3D2DFBD3") "Interface GUID not found in INF"
    Assert-Condition "INF contains zero broad Class_07 wildcards" (-not ($infContent -match "USB\\Class_07")) "Disallowed broad wildcard found in INF"
}

Write-Host "`n[Suite 4: Network Loopback Invariant Checks]" -ForegroundColor Yellow
$spoolRsPath = Join-Path $ProjectRoot "src\spool.rs"
if (Test-Path $spoolRsPath) {
    $spoolContent = Get-Content $spoolRsPath -Raw
    Assert-Condition "Normal mode binds to 127.0.0.1" ($spoolContent -match '127\.0\.0\.1') "Loopback address missing"
    Assert-Condition "Normal port 9100 configured" ($spoolContent -match '9100') "Port 9100 missing"
    Assert-Condition "Photo port 9101 configured" ($spoolContent -match '9101') "Port 9101 missing"
    Assert-Condition "Sockets never bind to INADDR_ANY (0.0.0.0)" (-not ($spoolContent -match '0\.0\.0\.0')) "Disallowed 0.0.0.0 binding found"
    Assert-Condition "Payload ceiling is enforced (100 MB)" ($spoolContent -match 'MAX_SPOOL_BYTES') "Payload ceiling constant not found"
}

Write-Host "`n[Suite 5: Runtime Asset & Sound Archive Checks]" -ForegroundColor Yellow
$wavPath = Join-Path $ProjectRoot "reference\drivers\LBP1120_WinXP\CAP3ZWAV.BIN"
Assert-Condition "Authentic Canon CAP3ZWAV.BIN exists" (Test-Path $wavPath) "File not found at $wavPath"
if (Test-Path $wavPath) {
    $wavItem = Get-Item $wavPath
    Assert-Condition "CAP3ZWAV.BIN size matches authentic archive" ($wavItem.Length -eq 412070) "Size was $($wavItem.Length) bytes (expected 412,070)"
}

Write-Host "`n=======================================================" -ForegroundColor Cyan
Write-Host "   TEST RESULTS: $PassCount PASSED, $FailCount FAILED   " -ForegroundColor $(if ($FailCount -eq 0) { "Green" } else { "Red" })
Write-Host "=======================================================" -ForegroundColor Cyan

if ($FailCount -gt 0) {
    exit 1
} else {
    exit 0
}
