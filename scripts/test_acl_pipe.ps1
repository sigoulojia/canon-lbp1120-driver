Add-Type @"
using System;
using System.IO;
using System.IO.Pipes;
using System.Security.AccessControl;
using System.Security.Principal;

public class PipeTest {
    public static void RunServer() {
        PipeSecurity ps = new PipeSecurity();
        ps.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.WorldSid, null), PipeAccessRights.ReadWrite, AccessControlType.Allow));
        ps.AddAccessRule(new PipeAccessRule(new SecurityIdentifier(WellKnownSidType.LocalSystemSid, null), PipeAccessRights.ReadWrite, AccessControlType.Allow));

        using (NamedPipeServerStream server = new NamedPipeServerStream("CanonLBP1120_Test", PipeDirection.In, 1, PipeTransmissionMode.Byte, PipeOptions.None, 65536, 65536, ps)) {
            Console.WriteLine("Server listening on CanonLBP1120_Test with permissive ACL...");
            server.WaitForConnection();
            Console.WriteLine("Client connected successfully!");
            byte[] buf = new byte[1024];
            int n = server.Read(buf, 0, buf.Length);
            Console.WriteLine("Read " + n + " bytes: " + System.Text.Encoding.ASCII.GetString(buf, 0, Math.Min(n, 64)));
        }
    }
}
"@

[PipeTest]::RunServer()
