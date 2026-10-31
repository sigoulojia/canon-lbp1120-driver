$src = @"
using System;
using System.Runtime.InteropServices;

public class HreHarness {
    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="hHREOpen", CallingConvention=CallingConvention.StdCall)]
    public static extern IntPtr hHREOpen(uint uParam);

    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="uiHREClose", CallingConvention=CallingConvention.StdCall)]
    public static extern uint uiHREClose(IntPtr hHre);

    [DllImport("reference\\drivers\\LBP1120_WinXP\\CAP3RDN.dll", EntryPoint="uiHREDelete", CallingConvention=CallingConvention.StdCall)]
    public static extern uint uiHREDelete(IntPtr hHre, uint uFlags);
}
"@

Add-Type -TypeDefinition $src

Write-Output "Calling hHREOpen(0)..."
try {
    $h = [HreHarness]::hHREOpen(0)
    Write-Output ("hHREOpen(0) returned handle: 0x{0:X}" -f $h.ToInt64())
    if ($h -ne [IntPtr]::Zero) {
        $resClose = [HreHarness]::uiHREClose($h)
        Write-Output ("uiHREClose returned: " + $resClose)
        $resDel = [HreHarness]::uiHREDelete($h, 0)
        Write-Output ("uiHREDelete returned: " + $resDel)
    }
} catch {
    Write-Output ("Exception: " + $_.Exception.ToString())
}
