$bytes = [System.IO.File]::ReadAllBytes('extracted\WinVista\CAP3HT.BIN')
$stream = New-Object System.IO.MemoryStream(,$bytes)
$reader = New-Object System.IO.BinaryReader($stream)

# First few 32-bit integers
Write-Host "Total size: $($bytes.Length) bytes"
for ($i = 0; $i -lt 16; $i++) {
    $val = $reader.ReadUInt32()
    Write-Host ("Offset 0x{0:X2} (word {1,2}): 0x{2:X8} ({2})" -f ($i * 4), $i, $val)
}
