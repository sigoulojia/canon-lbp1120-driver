Add-Type @"
using System;
using System.IO;
using System.Runtime.InteropServices;

public class PipeCreateFileTest {
    [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    public static extern IntPtr CreateFile(
        string lpFileName,
        uint dwDesiredAccess,
        uint dwShareMode,
        IntPtr lpSecurityAttributes,
        uint dwCreationDisposition,
        uint dwFlagsAndAttributes,
        IntPtr hTemplateFile
    );

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr hObject);

    public const uint GENERIC_WRITE = 0x40000000;
    public const uint FILE_SHARE_READ = 1;
    public const uint FILE_SHARE_WRITE = 2;
    public const uint CREATE_NEW = 1;
    public const uint CREATE_ALWAYS = 2;
    public const uint OPEN_EXISTING = 3;
    public const uint OPEN_ALWAYS = 4;
    public const uint TRUNCATE_EXISTING = 5;

    public static void TestDispositions(string pipePath) {
        uint[] disps = { OPEN_EXISTING, OPEN_ALWAYS, CREATE_ALWAYS, CREATE_NEW };
        string[] names = { "OPEN_EXISTING", "OPEN_ALWAYS", "CREATE_ALWAYS", "CREATE_NEW" };

        for (int i = 0; i < disps.Length; i++) {
            IntPtr h = CreateFile(pipePath, GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE, IntPtr.Zero, disps[i], 0, IntPtr.Zero);
            if (h == (IntPtr)(-1) || h == IntPtr.Zero) {
                int err = Marshal.GetLastWin32Error();
                Console.WriteLine(names[i] + " -> FAILED with Win32 error: " + err);
            } else {
                Console.WriteLine(names[i] + " -> SUCCESS!");
                CloseHandle(h);
            }
        }
    }
}
"@

# Start a named pipe server in background
$ps = New-Object System.IO.Pipes.PipeSecurity
$ps.AddAccessRule((New-Object System.IO.Pipes.PipeAccessRule([System.Security.Principal.SecurityIdentifier]::new([System.Security.Principal.WellKnownSidType]::WorldSid, $null), [System.IO.Pipes.PipeAccessRights]::ReadWrite, [System.Security.AccessControl.AccessControlType]::Allow)))
$server = New-Object System.IO.Pipes.NamedPipeServerStream("TestDispPipe", [System.IO.Pipes.PipeDirection]::In, 1, [System.IO.Pipes.PipeTransmissionMode]::Byte, [System.IO.Pipes.PipeOptions]::Asynchronous, 65536, 65536, $ps)

[PipeCreateFileTest]::TestDispositions("\\.\pipe\TestDispPipe")
$server.Dispose()
