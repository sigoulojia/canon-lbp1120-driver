$proc = Start-Process -FilePath "F:\Games\Nouveau dossier (3)\capt-driver\target\release\capt-poc.exe" -ArgumentList "service" -PassThru
Start-Sleep -Seconds 1
if (Get-Process -Id $proc.Id -ErrorAction SilentlyContinue) {
    Write-Host "Service running successfully with PID $($proc.Id)!"
} else {
    Write-Host "Process exited. ExitCode: $($proc.ExitCode)"
}
