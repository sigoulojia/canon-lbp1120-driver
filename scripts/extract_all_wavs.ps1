$binPath = "extracted\WinVista\CAP3ZWAV.BIN"
$outDir = "capt-driver\assets\sounds"
if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir -Force | Out-Null }

$names = @(
    "Cap3addp.wav",
    "Cap3cln.wav",
    "Cap3come.wav",
    "Cap3covr.wav",
    "Cap3done.wav",
    "Cap3err.wav",
    "Cap3jam.wav",
    "Cap3mem.wav",
    "Cap3paus.wav",
    "Cap3potb.wav",
    "Cap3fail.wav",
    "Cap3resm.wav",
    "Cap3serr.wav",
    "Cap3cart.wav",
    "Cap3strt.wav",
    "Cap3stop.wav",
    "Cap3chgp.wav"
)

$bytes = [System.IO.File]::ReadAllBytes($binPath)
$riffOffsets = @()
for ($i = 0; $i -lt ($bytes.Length - 4); $i++) {
    if ($bytes[$i] -eq 0x52 -and $bytes[$i+1] -eq 0x49 -and $bytes[$i+2] -eq 0x46 -and $bytes[$i+3] -eq 0x46) {
        $riffOffsets += $i
    }
}

for ($j = 0; $j -lt $riffOffsets.Count; $j++) {
    $off = $riffOffsets[$j]
    $size = [BitConverter]::ToUInt32($bytes, $off + 4) + 8
    $dest = Join-Path $outDir $names[$j]
    $wavBytes = $bytes[$off..($off + $size - 1)]
    [System.IO.File]::WriteAllBytes($dest, $wavBytes)
    Write-Host ("Extracted {0,-14} ({1,5} bytes) -> {2}" -f $names[$j], $size, $dest)
}
Write-Host "All 17 Canon authentic sound cues extracted successfully!"
