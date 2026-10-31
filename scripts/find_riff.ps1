$bytes = [System.IO.File]::ReadAllBytes('extracted\WinVista\CAP3ZWAV.BIN')

# Let's search for "RIFF" headers in the whole file!
$riffOffsets = @()
for ($i = 0; $i -lt ($bytes.Length - 4); $i++) {
    if ($bytes[$i] -eq 0x52 -and $bytes[$i+1] -eq 0x49 -and $bytes[$i+2] -eq 0x46 -and $bytes[$i+3] -eq 0x46) {
        $riffOffsets += $i
    }
}

Write-Host "Found $($riffOffsets.Count) RIFF headers:"
for ($j = 0; $j -lt $riffOffsets.Count; $j++) {
    $off = $riffOffsets[$j]
    $size = [BitConverter]::ToUInt32($bytes, $off + 4) + 8
    Write-Host ("  RIFF at 0x{0:X6} ({0,7}), size: {1,5} bytes" -f $off, $size)
}
