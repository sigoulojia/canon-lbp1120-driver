@echo off
setlocal
cd /d "%~dp0"
echo =======================================================
echo    Canon LaserShot LBP-1120 Driver Uninstaller
echo =======================================================
echo.
echo Launching elevated PowerShell uninstaller...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Uninstall-CanonLBP1120.ps1"
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo Uninstallation reported error code %ERRORLEVEL%.
    pause
)

