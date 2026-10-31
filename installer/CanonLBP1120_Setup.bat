@echo off
setlocal
cd /d "%~dp0"
echo =======================================================
echo    Canon LaserShot LBP-1120 Driver Setup
echo =======================================================
echo.
echo Launching elevated PowerShell installer...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Install-CanonLBP1120.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Installation reported error code %ERRORLEVEL%.
    pause
)

