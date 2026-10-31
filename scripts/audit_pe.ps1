$path = "target\release\capt-poc.exe"
if (-not (Test-Path $path)) {
    Write-Error "File not found: $path"
    exit 1
}

$bytes = [System.IO.File]::ReadAllBytes((Resolve-Path $path).Path)
$e_lfanew = [BitConverter]::ToInt32($bytes, 0x3C)
$peSig = [BitConverter]::ToUInt32($bytes, $e_lfanew)
$machine = [BitConverter]::ToUInt16($bytes, $e_lfanew + 4)
$numSections = [BitConverter]::ToUInt16($bytes, $e_lfanew + 6)
$optHeaderOffset = $e_lfanew + 24
$magic = [BitConverter]::ToUInt16($bytes, $optHeaderOffset)
$majorOS = [BitConverter]::ToUInt16($bytes, $optHeaderOffset + 40)
$minorOS = [BitConverter]::ToUInt16($bytes, $optHeaderOffset + 42)
$majorSubsys = [BitConverter]::ToUInt16($bytes, $optHeaderOffset + 48)
$minorSubsys = [BitConverter]::ToUInt16($bytes, $optHeaderOffset + 50)
$subsystem = [BitConverter]::ToUInt16($bytes, $optHeaderOffset + 68)

Write-Host "=== PE HEADER AUDIT: $path ==="
Write-Host ("PE Signature        : 0x{0:X8}" -f $peSig)
Write-Host ("Machine Type        : 0x{0:X4} ({1})" -f $machine, $(if ($machine -eq 0x8664) {"IMAGE_FILE_MACHINE_AMD64 (x64)"} elseif ($machine -eq 0xAA64) {"IMAGE_FILE_MACHINE_ARM64"} else {"Unknown"}))
Write-Host ("Magic (Architecture): 0x{0:X4} ({1})" -f $magic, $(if ($magic -eq 0x20B) {"PE32+ (64-bit)"} else {"PE32 (32-bit)"}))
Write-Host ("OS Version          : {0}.{1}" -f $majorOS, $minorOS)
Write-Host ("Subsystem Version   : {0}.{1}" -f $majorSubsys, $minorSubsys)
Write-Host ("Subsystem           : {0} ({1})" -f $subsystem, $(if ($subsystem -eq 3) {"IMAGE_SUBSYSTEM_WINDOWS_CUI (Console)"} elseif ($subsystem -eq 2) {"IMAGE_SUBSYSTEM_WINDOWS_GUI"} else {"Unknown"}))

# Import Directory
# In PE32+, DataDirectory[1] is Import Table at optHeaderOffset + 112 + 8 = optHeaderOffset + 120
$importRva = [BitConverter]::ToUInt32($bytes, $optHeaderOffset + 120)
$importSize = [BitConverter]::ToUInt32($bytes, $optHeaderOffset + 124)
Write-Host ("Import Table RVA    : 0x{0:X8} (Size: {1} bytes)" -f $importRva, $importSize)

# Section Headers start at optHeaderOffset + SizeOfOptionalHeader
$sizeOfOptHeader = [BitConverter]::ToUInt16($bytes, $e_lfanew + 20)
$sectionTableOffset = $optHeaderOffset + $sizeOfOptHeader

# Function to map RVA to File Offset
function RvaToOffset($rva) {
    for ($i = 0; $i -lt $numSections; $i++) {
        $secOffset = $sectionTableOffset + ($i * 40)
        $virtSize = [BitConverter]::ToUInt32($bytes, $secOffset + 8)
        $virtAddr = [BitConverter]::ToUInt32($bytes, $secOffset + 12)
        $rawSize  = [BitConverter]::ToUInt32($bytes, $secOffset + 16)
        $rawAddr  = [BitConverter]::ToUInt32($bytes, $secOffset + 20)
        
        if ($rva -ge $virtAddr -and $rva -lt ($virtAddr + [Math]::Max($virtSize, $rawSize))) {
            return ($rva - $virtAddr) + $rawAddr
        }
    }
    return 0
}

Write-Host "`n=== IMPORTED DLL MODULES ==="
$importOffset = RvaToOffset $importRva
if ($importOffset -gt 0) {
    $curDesc = $importOffset
    while ($true) {
        $origFirstThunk = [BitConverter]::ToUInt32($bytes, $curDesc)
        $timeDateStamp  = [BitConverter]::ToUInt32($bytes, $curDesc + 4)
        $forwarderChain = [BitConverter]::ToUInt32($bytes, $curDesc + 8)
        $nameRva        = [BitConverter]::ToUInt32($bytes, $curDesc + 12)
        $firstThunk     = [BitConverter]::ToUInt32($bytes, $curDesc + 16)
        
        if ($nameRva -eq 0 -and $firstThunk -eq 0) {
            break # NULL descriptor marks end of list
        }
        
        $nameOffset = RvaToOffset $nameRva
        $dllName = ""
        while ($bytes[$nameOffset] -ne 0) {
            $dllName += [char]$bytes[$nameOffset]
            $nameOffset++
        }
        
        Write-Host "  - $dllName"
        $curDesc += 20
    }
} else {
    Write-Host "Unable to map import table offset."
}

