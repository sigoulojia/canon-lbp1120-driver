try {
    $client = New-Object System.IO.Pipes.NamedPipeClientStream(".", "CanonLBP1120", [System.IO.Pipes.PipeDirection]::Out)
    $client.Connect(2000)
    Write-Host "SUCCESS: Connected to named pipe \\.\pipe\CanonLBP1120!"
    $writer = New-Object System.IO.BinaryWriter($client)
    $writer.Write([System.Text.Encoding]::ASCII.GetBytes("%PDF-1.4 TEST"))
    $writer.Flush()
    $client.Close()
    Write-Host "Data written and connection closed."
} catch {
    Write-Host "FAILED to connect to named pipe: $_"
}
