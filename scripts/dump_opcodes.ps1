$bytes = [System.IO.File]::ReadAllBytes("F:\Games\Nouveau dossier (3)\extracted\WinVista\CAP3EMN.DLL")
$offsets = @(0x45B5, 0x677A, 0xAE1B, 0xB14D, 0xB2A4, 0xB502)
foreach ($off in $offsets) {
    Write-Host ("--- Offset 0x{0:X} ---" -f $off)
    $start = [Math]::Max(0, $off - 32)
    $len = [Math]::Min(96, $bytes.Length - $start)
    $hex = ""
    for ($j=0; $j -lt $len; $j++) {
        $hex += ("{0:X2} " -f $bytes[$start + $j])
    }
    Write-Host $hex
}
