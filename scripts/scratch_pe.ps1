param()
$files = Get-ChildItem -Path "extracted\WinVista" -Include *.dll,*.exe -Recurse
foreach ($f in $files) {
    $stream = [System.IO.File]::OpenRead($f.FullName)
    $reader = New-Object System.IO.BinaryReader($stream)
    $mz = $reader.ReadUInt16()
    if ($mz -eq 0x5A4D) {
        $stream.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
        $peOffset = $reader.ReadUInt32()
        $stream.Seek($peOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
        $peSig = $reader.ReadUInt32()
        if ($peSig -eq 0x00004550) {
            $machine = $reader.ReadUInt16()
            $machineStr = switch ($machine) {
                0x014c { "x86 (32-bit)" }
                0x8664 { "x64 (64-bit)" }
                0x0200 { "IA64" }
                default { "Unknown (0x{0:X4})" -f $machine }
            }
            Write-Host ("{0,-20} : {1} ({2} bytes)" -f $f.Name, $machineStr, $f.Length)
        }
    }
    $reader.Close()
    $stream.Close()
}
