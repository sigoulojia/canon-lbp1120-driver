$tools = "C:\Users\ZAKI\AppData\Local\Programs\gh"
New-Item -Path $tools -ItemType Directory -Force | Out-Null
$zip = Join-Path $tools "gh.zip"

Write-Host "Downloading GitHub CLI portable zip..."
curl.exe -L -o $zip https://github.com/cli/cli/releases/download/v2.60.1/gh_2.60.1_windows_amd64.zip

Write-Host "Extracting GitHub CLI..."
Expand-Archive -Path $zip -DestinationPath $tools -Force
Remove-Item $zip -Force -ErrorAction SilentlyContinue

$ghExe = Get-ChildItem -Path $tools -Recurse -Filter "gh.exe" | Select-Object -First 1
if ($ghExe) {
    Write-Host "GitHub CLI installed successfully at: $($ghExe.FullName)" -ForegroundColor Green
    & "$($ghExe.FullName)" --version
} else {
    Write-Error "gh.exe not found after extraction"
}
