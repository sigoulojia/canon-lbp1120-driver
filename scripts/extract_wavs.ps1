$binPath = "extracted\WinVista\CAP3ZWAV.BIN"
$outDir = "capt-driver\assets\sounds"
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }

$bytes = [System.IO.File]::ReadAllBytes($binPath)
$stream = New-Object System.IO.MemoryStream(,$bytes)
$reader = New-Object System.IO.BinaryReader($stream)

$sig = [System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(12))
$numWavs = $reader.ReadUInt32()
Write-Host "Unpacking $numWavs audio files from $binPath..."

$entries = @()
for ($i = 0; $i -lt $numWavs; $i++) {
    $size = $reader.ReadUInt32()
    $nameBytes = $reader.ReadBytes(32)
    # find null terminator
    $nameLen = 0
    while ($nameLen -lt 32 -and $nameBytes[$nameLen] -ne 0) { $nameLen++ }
    $name = [System.Text.Encoding]::ASCII.GetString($nameBytes[0..($nameLen - 1)])
    $entries += [PSCustomObject]@{ Index = $i; Size = $size; Name = $name }
}

$headerEnd = 16 + ($numWavs * 36)
Write-Host "Audio data starts at offset 0x$($headerEnd.ToString('X'))"

$currentOffset = $headerEnd
foreach ($e in $entries) {
    if ($e.Size -gt 0 -and ($currentOffset + $e.Size) -le $bytes.Length) {
        $wavBytes = $bytes[$currentOffset..($currentOffset + $e.Size - 1)]
        $destFile = Join-Path $outDir $e.Name
        [System.IO.File]::WriteAllBytes($destFile, $wavBytes)
        Write-Host ("  Saved {0,-16} ({1,5} bytes) -> {2}" -f $e.Name, $e.Size, $destFile)
        $currentOffset += $e.Size
    }
}
Write-Host "Extraction complete!"
