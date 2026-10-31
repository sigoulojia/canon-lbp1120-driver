Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public class SpoolCheck {
    [DllImport("winspool.drv", CharSet = CharSet.Auto, SetLastError = true)]
    public static extern bool OpenPrinter(string pPrinterName, out IntPtr phPrinter, IntPtr pDefault);

    [DllImport("winspool.drv", SetLastError = true)]
    public static extern bool ClosePrinter(IntPtr hPrinter);

    [DllImport("winspool.drv", SetLastError = true)]
    public static extern bool EnumJobs(IntPtr hPrinter, uint FirstJob, uint NoJobs, uint Level, IntPtr pJob, uint cbBuf, out uint pcbNeeded, out uint pcReturned);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Auto)]
    public struct JOB_INFO_2 {
        public uint JobId;
        public string pPrinterName;
        public string pMachineName;
        public string pUserName;
        public string pDocument;
        public string pNotifyName;
        public string pDatatype;
        public string pPrintProcessor;
        public string pParameters;
        public string pDriverName;
        public IntPtr pDevMode;
        public string pStatus;
        public IntPtr pSecurityDescriptor;
        public uint Status;
        public uint Priority;
        public uint Position;
        public uint StartTime;
        public uint UntilTime;
        public uint TotalPages;
        public uint Size;
        public SystemTime Submitted;
        public uint Time;
        public uint PagesPrinted;
    }

    [StructLayout(LayoutKind.Sequential)]
    public struct SystemTime {
        public short wYear;
        public short wMonth;
        public short wDayOfWeek;
        public short wDay;
        public short wHour;
        public short wMinute;
        public short wSecond;
        public short wMilliseconds;
    }

    public static void InspectJobs(string printerName) {
        IntPtr hPrinter;
        if (!OpenPrinter(printerName, out hPrinter, IntPtr.Zero)) {
            Console.WriteLine("OpenPrinter failed: " + Marshal.GetLastWin32Error());
            return;
        }

        uint bytesNeeded, count;
        EnumJobs(hPrinter, 0, 10, 2, IntPtr.Zero, 0, out bytesNeeded, out count);
        if (bytesNeeded > 0) {
            IntPtr buf = Marshal.AllocHGlobal((int)bytesNeeded);
            if (EnumJobs(hPrinter, 0, 10, 2, buf, bytesNeeded, out bytesNeeded, out count)) {
                Console.WriteLine("Found " + count + " job(s) for printer: " + printerName);
                int structSize = Marshal.SizeOf(typeof(JOB_INFO_2));
                for (int i = 0; i < count; i++) {
                    IntPtr ptr = new IntPtr(buf.ToInt64() + (i * structSize));
                    JOB_INFO_2 job = (JOB_INFO_2)Marshal.PtrToStructure(ptr, typeof(JOB_INFO_2));
                    Console.WriteLine("  Job ID: " + job.JobId);
                    Console.WriteLine("    Document : " + job.pDocument);
                    Console.WriteLine("    Status   : 0x" + job.Status.ToString("X") + " (" + job.pStatus + ")");
                    Console.WriteLine("    Datatype : " + job.pDatatype);
                    Console.WriteLine("    PrintProc: " + job.pPrintProcessor);
                    Console.WriteLine("    Size     : " + job.Size + " bytes");
                }
            } else {
                Console.WriteLine("EnumJobs failed: " + Marshal.GetLastWin32Error());
            }
            Marshal.FreeHGlobal(buf);
        } else {
            Console.WriteLine("No jobs in queue for " + printerName);
        }

        ClosePrinter(hPrinter);
    }
}
"@

[SpoolCheck]::InspectJobs("Canon LaserShot LBP-1120")
