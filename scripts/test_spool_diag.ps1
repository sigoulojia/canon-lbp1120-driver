$portName = "\\.\pipe\CanonLBP1120_Diag"
$printerName = "Diag-PDF-Printer"

# 1. Clean up old test printer if exists
Get-Printer -Name $printerName -ErrorAction SilentlyContinue | Remove-Printer -ErrorAction SilentlyContinue
Get-PrinterPort -Name $portName -ErrorAction SilentlyContinue | Remove-PrinterPort -ErrorAction SilentlyContinue

# 2. Add port and printer
Add-PrinterPort -Name $portName
Add-Printer -Name $printerName -DriverName "Microsoft Print to PDF" -PortName $portName

Write-Host "Diagnostic printer created. Starting pipe listener with open ACL..."

$ps = New-Object System.IO.Pipes.PipeSecurity
$ps.AddAccessRule((New-Object System.IO.Pipes.PipeAccessRule([System.Security.Principal.SecurityIdentifier]::new([System.Security.Principal.WellKnownSidType]::WorldSid, $null), [System.IO.Pipes.PipeAccessRights]::ReadWrite, [System.Security.AccessControl.AccessControlType]::Allow)))
$ps.AddAccessRule((New-Object System.IO.Pipes.PipeAccessRule([System.Security.Principal.SecurityIdentifier]::new([System.Security.Principal.WellKnownSidType]::LocalSystemSid, $null), [System.IO.Pipes.PipeAccessRights]::ReadWrite, [System.Security.AccessControl.AccessControlType]::Allow)))

$server = New-Object System.IO.Pipes.NamedPipeServerStream("CanonLBP1120_Diag", [System.IO.Pipes.PipeDirection]::In, 1, [System.IO.Pipes.PipeTransmissionMode]::Byte, [System.IO.Pipes.PipeOptions]::Asynchronous, 65536, 65536, $ps)

# 3. Print a test text to this printer in background
$testFile = "$PSScriptRoot\diag_test.txt"
"Hello World Canon LBP1120 Test" | Out-File -FilePath $testFile -Encoding ascii

$job = Start-Job -ScriptBlock {
    param($prn, $txt)
    Start-Sleep -Seconds 1
    Get-Content $txt | Out-Printer -Name $prn
} -ArgumentList $printerName, $testFile

Write-Host "Waiting for print spooler connection on $portName..."
$async = $server.BeginWaitForConnection($null, $null)
$success = $async.AsyncWaitHandle.WaitOne(8000)

if ($success) {
    $server.EndWaitForConnection($async)
    Write-Host ">>> CONNECTION ACCEPTED FROM SPOOLER! <<<"
    $ms = New-Object System.IO.MemoryStream
    $buf = New-Object byte[] 4096
    do {
        $read = $server.Read($buf, 0, $buf.Length)
        if ($read -gt 0) { $ms.Write($buf, 0, $read) }
    } while ($read -gt 0)
    
    $receivedBytes = $ms.ToArray()
    Write-Host "Received total: $($receivedBytes.Length) bytes!"
    if ($receivedBytes.Length -ge 16) {
        $hex = ($receivedBytes[0..15] | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
        $ascii = [System.Text.Encoding]::ASCII.GetString($receivedBytes[0..15]) -replace '[^\x20-\x7E]', '.'
        Write-Host "Header Hex  : $hex"
        Write-Host "Header Ascii: $ascii"
    }
    [System.IO.File]::WriteAllBytes("$PSScriptRoot\diag_received.bin", $receivedBytes)
} else {
    Write-Host "TIMEOUT: Spooler did not connect within 8 seconds."
    # Check print queue status
    Get-PrintJob -PrinterName $printerName | Format-List *
}

$server.Dispose()
Remove-Printer -Name $printerName -ErrorAction SilentlyContinue
Remove-PrinterPort -Name $portName -ErrorAction SilentlyContinue
Remove-Job $job -Force -ErrorAction SilentlyContinue
