$dlls = Get-ChildItem "F:\Games\Nouveau dossier (3)\extracted\WinVista\*.dll"
foreach ($dll in $dlls) {
    $bytes = [System.IO.File]::ReadAllBytes($dll.FullName)
    for ($i = 0; $i -lt ($bytes.Length - 4); $i++) {
        if ($bytes[$i] -eq 0xA0 -and $bytes[$i+1] -eq 0xD0) {
            Write-Host ("Found 0xD0A0 in {0} at offset 0x{1:X}" -f $dll.Name, $i)
        }
        if ($bytes[$i] -eq 0xA0 -and $bytes[$i+1] -eq 0xC0) {
            Write-Host ("Found 0xC0A0 in {0} at offset 0x{1:X}" -f $dll.Name, $i)
        }
    }
}
