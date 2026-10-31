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

Write-Output "Harness compiled successfully."

