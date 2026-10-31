param()

function Dump-Exports($path) {
    Write-Host "`n=== Exports for $([System.IO.Path]::GetFileName($path)) ==="
    $bytes = [System.IO.File]::ReadAllBytes($path)
    $stream = New-Object System.IO.MemoryStream(,$bytes)
    $reader = New-Object System.IO.BinaryReader($stream)
    
    $stream.Seek(0x3C, [System.IO.SeekOrigin]::Begin) | Out-Null
    $peOffset = $reader.ReadUInt32()
    
    $exportRvaOffset = $peOffset + 120
    $stream.Seek($exportRvaOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
    $exportRva = $reader.ReadUInt32()
    $exportSize = $reader.ReadUInt32()
    
    if ($exportRva -eq 0) {
        Write-Host "No export directory found."
        $reader.Close(); $stream.Close(); return
    }
    
    $numSectionsOffset = $peOffset + 4 + 2
    $stream.Seek($numSectionsOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
    $numSections = $reader.ReadUInt16()
    
    $optHeaderSizeOffset = $peOffset + 4 + 16
    $stream.Seek($optHeaderSizeOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
    $optHeaderSize = $reader.ReadUInt16()
    
    $sectionTableOffset = $peOffset + 4 + 20 + $optHeaderSize
    $sections = @()
    for ($i = 0; $i -lt $numSections; $i++) {
        $secOffset = $sectionTableOffset + ($i * 40)
        $stream.Seek($secOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
        $nameBytes = $reader.ReadBytes(8)
        $secName = [System.Text.Encoding]::ASCII.GetString($nameBytes).Trim([char]0)
        $vSize = $reader.ReadUInt32()
        $vAddr = $reader.ReadUInt32()
        $rSize = $reader.ReadUInt32()
        $rOffset = $reader.ReadUInt32()
        $sections += [PSCustomObject]@{ Name = $secName; VirtualAddress = $vAddr; VirtualSize = $vSize; RawOffset = $rOffset; RawSize = $rSize }
    }
    
    function RvaToOffset($rva) {
        foreach ($sec in $sections) {
            $span = [Math]::Max($sec.VirtualSize, $sec.RawSize)
            if ($rva -ge $sec.VirtualAddress -and $rva -lt ($sec.VirtualAddress + $span)) {
                return $sec.RawOffset + ($rva - $sec.VirtualAddress)
            }
        }
        return 0
    }
    
    $exportFileOffset = RvaToOffset $exportRva
    if ($exportFileOffset -eq 0) {
        Write-Host "Could not map export RVA 0x$($exportRva.ToString('X'))"
        $reader.Close(); $stream.Close(); return
    }
    
    $stream.Seek($exportFileOffset + 20, [System.IO.SeekOrigin]::Begin) | Out-Null
    $numFunctions = $reader.ReadUInt32()
    $numNames = $reader.ReadUInt32()
    $addrOfFunctionsRva = $reader.ReadUInt32()
    $addrOfNamesRva = $reader.ReadUInt32()
    $addrOfOrdinalsRva = $reader.ReadUInt32()
    
    Write-Host "Total exported functions: $numFunctions, Named: $numNames"
    
    $namesOffset = RvaToOffset $addrOfNamesRva
    for ($i = 0; $i -lt [Math]::Min($numNames, 30); $i++) {
        $stream.Seek($namesOffset + ($i * 4), [System.IO.SeekOrigin]::Begin) | Out-Null
        $nameRva = $reader.ReadUInt32()
        $nameFileOffset = RvaToOffset $nameRva
        $stream.Seek($nameFileOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
        $fnName = ""
        while (($b = $reader.ReadByte()) -ne 0) {
            $fnName += [char]$b
        }
        Write-Host "  [$i] $fnName"
    }
    $reader.Close()
    $stream.Close()
}

Dump-Exports "extracted\WinVista\CAP3K.DLL"
Dump-Exports "extracted\WinVista\CAP3LMK.DLL"
Dump-Exports "extracted\WinVista\CAP3UIK.DLL"
Dump-Exports "extracted\WinVista\CAP3RDN.DLL"
Dump-Exports "extracted\WinVista\CAP3SMK.DLL"
Dump-Exports "extracted\WinVista\CAP3PMN.DLL"
