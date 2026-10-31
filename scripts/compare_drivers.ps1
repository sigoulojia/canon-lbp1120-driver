param()
$xpDir = "LBP1120_WinXP"
$vistaDir = "extracted\WinVista"

$xpFiles = Get-ChildItem $xpDir
$vistaFiles = Get-ChildItem $vistaDir

Write-Host "=== Comparison between XP driver and Win7/Vista driver ==="
foreach ($vf in $vistaFiles) {
    $xf = $xpFiles | Where-Object { $_.Name -ieq $vf.Name }
    if ($xf) {
        $vHash = (Get-FileHash $vf.FullName -Algorithm SHA256).Hash
        $xHash = (Get-FileHash $xf.FullName -Algorithm SHA256).Hash
        $status = "DIFFERENT"
        if ($vHash -eq $xHash) { $status = "IDENTICAL" }
        Write-Host ("{0,-15} : XP={1,8} B, Vista/7={2,8} B | {3}" -f $vf.Name, $xf.Length, $vf.Length, $status)
    } else {
        Write-Host ("{0,-15} : ONLY IN Vista/7 ({1} B)" -f $vf.Name, $vf.Length)
    }
}
foreach ($xf in $xpFiles) {
    $vf = $vistaFiles | Where-Object { $_.Name -ieq $xf.Name }
    if (-not $vf) {
        Write-Host ("{0,-15} : ONLY IN XP ({1} B)" -f $xf.Name, $xf.Length)
    }
}
