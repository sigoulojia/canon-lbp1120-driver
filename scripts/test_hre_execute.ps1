$src = @"
using System;
using System.Runtime.InteropServices;

public class HreTester {
    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="hHREOpen", CallingConvention=CallingConvention.StdCall)]
    public static extern IntPtr hHREOpen(uint uParam);

    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="uiHREExecute", CallingConvention=CallingConvention.StdCall)]
    public static extern uint uiHREExecute(IntPtr hHre, IntPtr lpIn, IntPtr lpOut);

    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="uiHREClose", CallingConvention=CallingConvention.StdCall)]
    public static extern uint uiHREClose(IntPtr hHre);

    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="uiHREDelete", CallingConvention=CallingConvention.StdCall)]
    public static extern uint uiHREDelete(IntPtr hHre, uint uFlags);
}
"@

Add-Type -TypeDefinition $src

$h = [HreTester]::hHREOpen(0)
Write-Output ("hHREOpen returned: 0x{0:X}" -f $h.ToInt64())

if ($h -ne [IntPtr]::Zero) {
    # Allocate a 592-byte raster buffer (all white)
    $raster = [System.Runtime.InteropServices.Marshal]::AllocHGlobal(592)
    for ($i = 0; $i -lt 592; $i++) {
        [System.Runtime.InteropServices.Marshal]::WriteByte($raster, $i, 0)
    }

    # Allocate 64 bytes for lpIn
    $lpIn = [System.Runtime.InteropServices.Marshal]::AllocHGlobal(64)
    # Zero lpIn
    for ($i = 0; $i -lt 64; $i++) { [System.Runtime.InteropServices.Marshal]::WriteByte($lpIn, $i, 0) }

    # +0x00: pointer to raster
    [System.Runtime.InteropServices.Marshal]::WriteIntPtr($lpIn, 0, $raster)
    # +0x04: width in bytes = 592 (0x0250)
    [System.Runtime.InteropServices.Marshal]::WriteInt16($lpIn, 4, 592)
    # +0x08: pitch = 592
    [System.Runtime.InteropServices.Marshal]::WriteInt16($lpIn, 8, 592)
    # +0x0C: line count = 1
    [System.Runtime.InteropServices.Marshal]::WriteInt16($lpIn, 12, 1)

    # Allocate 64 bytes for lpOut
    $lpOut = [System.Runtime.InteropServices.Marshal]::AllocHGlobal(64)
    for ($i = 0; $i -lt 64; $i++) { [System.Runtime.InteropServices.Marshal]::WriteByte($lpOut, $i, 0) }

    Write-Output "Calling uiHREExecute..."
    try {
        $ret = [HreTester]::uiHREExecute($h, $lpIn, $lpOut)
        Write-Output ("uiHREExecute returned: " + $ret)
    } catch {
        Write-Output ("uiHREExecute exception: " + $_.Exception.ToString())
    }

    [HreTester]::uiHREClose($h) | Out-Null
    [System.Runtime.InteropServices.Marshal]::FreeHGlobal($raster)
    [System.Runtime.InteropServices.Marshal]::FreeHGlobal($lpIn)
    [System.Runtime.InteropServices.Marshal]::FreeHGlobal($lpOut)
}

