$files = Get-ChildItem -Path "extracted\WinVista" -File | Where-Object { $_.Extension -in '.dat','.afe','.bin','.cat' }
foreach ($f in $files) {
    $bytes = [System.IO.File]::ReadAllBytes($f.FullName)
    Write-Host ("`nFile: {0} ({1} bytes)" -f $f.Name, $bytes.Length)
    $hex = ($bytes[0..[Math]::Min(31, $bytes.Length - 1)] | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
    Write-Host ("  Hex: " + $hex)
    $ascii = [System.Text.Encoding]::ASCII.GetString($bytes[0..[Math]::Min(127, $bytes.Length - 1)]) -replace '[^\x20-\x7E]', '.'
    Write-Host ("  Ascii: " + $ascii)
}
