param(
    [string]$PdfPath = "F:\Games\Nouveau dossier (3)\test-3-pages.pdf",
    [int]$Port = 9100
)

$bytes = [System.IO.File]::ReadAllBytes($PdfPath)
Write-Host ("Connecting to 127.0.0.1:{0} to stream {1} bytes..." -f $Port, $bytes.Length)

$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", $Port)
$stream = $client.GetStream()
$stream.Write($bytes, 0, $bytes.Length)
$stream.Flush()
$client.Close()

Write-Host ">>> Print stream successfully delivered to Spool Server! <<<"
