$listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 9100)
$listener.Start()
Write-Host "TCP Listener active on 127.0.0.1:9100. Sending print job to Test_TCP_PDF..."

$testFile = "$PSScriptRoot\diag_test.txt"
"Hello World from Notepad TCP Test" | Out-File -FilePath $testFile -Encoding ascii

$job = Start-Job -ScriptBlock {
    param($txt)
    Start-Sleep -Milliseconds 800
    Get-Content $txt | Out-Printer -Name "Test_TCP_PDF"
} -ArgumentList $testFile

$async = $listener.BeginAcceptTcpClient($null, $null)
$success = $async.AsyncWaitHandle.WaitOne(8000)

if ($success) {
    $client = $listener.EndAcceptTcpClient($async)
    Write-Host ">>> TCP CLIENT CONNECTED FROM SPOOLER! <<<"
    $stream = $client.GetStream()
    $ms = New-Object System.IO.MemoryStream
    $buf = New-Object byte[] 4096
    $stream.ReadTimeout = 4000
    try {
        do {
            $read = $stream.Read($buf, 0, $buf.Length)
            if ($read -gt 0) { $ms.Write($buf, 0, $read) }
        } while ($read -gt 0)
    } catch {}
    
    $bytes = $ms.ToArray()
    Write-Host "Received $($bytes.Length) bytes over TCP!"
    if ($bytes.Length -ge 16) {
        $hex = ($bytes[0..15] | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
        $ascii = [System.Text.Encoding]::ASCII.GetString($bytes[0..[Math]::Min(128, $bytes.Length - 1)]) -replace '[^\x20-\x7E]', '.'
        Write-Host "Header Hex  : $hex"
        Write-Host "Header Ascii: $ascii"
        [System.IO.File]::WriteAllBytes("$PSScriptRoot\tcp_received.bin", $bytes)
    }
    $client.Close()
} else {
    Write-Host "TIMEOUT: No TCP connection on 9100 within 8s."
    Get-PrintJob -PrinterName "Test_TCP_PDF" | Format-List *
}

$listener.Stop()
Remove-Printer -Name "Test_TCP_PDF" -ErrorAction SilentlyContinue
Remove-PrinterPort -Name "IP_127.0.0.1_9100" -ErrorAction SilentlyContinue
Remove-Job $job -Force -ErrorAction SilentlyContinue
