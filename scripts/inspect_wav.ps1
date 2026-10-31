$bytes = [System.IO.File]::ReadAllBytes('extracted\WinVista\CAP3ZWAV.BIN')
$stream = New-Object System.IO.MemoryStream(,$bytes)
$reader = New-Object System.IO.BinaryReader($stream)

$sig = [System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(12))
$numWavs = $reader.ReadUInt32()
Write-Host "Sig: $sig, NumWavs: $numWavs, Total size: $($bytes.Length)"

# Each entry has: offset/size and name
for ($i = 0; $i -lt $numWavs; $i++) {
    $offset = $reader.ReadUInt32()
    $nameBytes = $reader.ReadBytes(32)
    $name = [System.Text.Encoding]::ASCII.GetString($nameBytes).Trim([char]0)
    Write-Host ("  [{0,2}] Offset: 0x{1:X6} ({1,7}), Name: {2}" -f $i, $offset, $name)
}
