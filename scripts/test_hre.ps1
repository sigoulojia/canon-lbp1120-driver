$src = @"
using System;
using System.Runtime.InteropServices;

public class NativeHre {
    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Ansi)]
    public static extern IntPtr LoadLibraryA(string lpLibFileName);

    [DllImport("kernel32.dll", SetLastError=true, CharSet=CharSet.Ansi)]
    public static extern IntPtr GetProcAddress(IntPtr hModule, string lpProcName);

    [DllImport("kernel32.dll", SetLastError=true)]
    public static extern bool FreeLibrary(IntPtr hModule);
}
"@

Add-Type -TypeDefinition $src

$dllPath = "F:\Games\canon-lbp1120-driver\reference\drivers\LBP1120_WinXP\CAP3RDN.dll"
$mod = [NativeHre]::LoadLibraryA($dllPath)
$err = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()

Write-Output ("Process Bits: " + ([IntPtr]::Size * 8))
Write-Output ("Loaded Module: 0x{0:X}" -f $mod.ToInt64())
Write-Output ("Win32 Error: " + $err)

if ($mod -ne [IntPtr]::Zero) {
    $pOpen = [NativeHre]::GetProcAddress($mod, "hHREOpen")
    $pExec = [NativeHre]::GetProcAddress($mod, "uiHREExecute")
    $pClose = [NativeHre]::GetProcAddress($mod, "uiHREClose")
    $pDelete = [NativeHre]::GetProcAddress($mod, "uiHREDelete")
    Write-Output ("hHREOpen RVA/Ptr: 0x{0:X}" -f $pOpen.ToInt64())
    Write-Output ("uiHREExecute RVA/Ptr: 0x{0:X}" -f $pExec.ToInt64())
    Write-Output ("uiHREClose RVA/Ptr: 0x{0:X}" -f $pClose.ToInt64())
    Write-Output ("uiHREDelete RVA/Ptr: 0x{0:X}" -f $pDelete.ToInt64())
    [NativeHre]::FreeLibrary($mod) | Out-Null
}

