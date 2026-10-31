@echo off
setlocal
cd /d "%~dp0"
echo ======================================================================
echo    CANON LASERSHOT LBP-1120 PRINT PIPELINE DIAGNOSTIC TRACER
echo ======================================================================
echo.
"%~dp0canon-lbp1120-service.exe" trace-pipeline
echo.
pause
