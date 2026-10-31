$installer = "installer\CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe"
$service = "dist\CanonLBP1120\canon-lbp1120-service.exe"

if (-not (Test-Path $installer)) {
    throw "Installer not found: $installer"
}
if (-not (Test-Path $service)) {
    throw "Service binary not found: $service"
}

$hashInstaller = (Get-FileHash -Path $installer -Algorithm SHA256).Hash.ToLower()
$hashService = (Get-FileHash -Path $service -Algorithm SHA256).Hash.ToLower()

$installerItem = Get-Item $installer
$serviceItem = Get-Item $service

$lines = @(
    "$hashInstaller  CanonLBP1120_Setup_v1.0.0-rc.1_x64.exe"
    "$hashService  canon-lbp1120-service.exe"
)

$content = $lines -join "`r`n"
Set-Content -Path "SHA256SUMS.txt" -Value $content -Encoding utf8

Write-Host "Generated SHA256SUMS.txt:" -ForegroundColor Green
Get-Content "SHA256SUMS.txt"
Write-Host "`nFile Sizes:" -ForegroundColor Cyan
Write-Host "  $installer : $($installerItem.Length) bytes ($([Math]::Round($installerItem.Length / 1MB, 2)) MB)"
Write-Host "  $service   : $($serviceItem.Length) bytes ($([Math]::Round($serviceItem.Length / 1KB, 1)) KB)"
