$bytes = [System.IO.File]::ReadAllBytes('capt-driver\assets\Cap3ht.bin')
$stream = New-Object System.IO.MemoryStream(,$bytes)
$reader = New-Object System.IO.BinaryReader($stream)

$numTables = $reader.ReadUInt32()
$t1_off = $reader.ReadUInt32(); $t1_sz = $reader.ReadUInt32()
$t2_off = $reader.ReadUInt32(); $t2_sz = $reader.ReadUInt32()
$t3_off = $reader.ReadUInt32(); $t3_sz = $reader.ReadUInt32()
$t4_off = $reader.ReadUInt32(); $t4_sz = $reader.ReadUInt32()

Write-Host ("Tables: {0}" -f $numTables)
Write-Host ("T1: off=0x{0:X4}, sz={1}" -f $t1_off, $t1_sz)
Write-Host ("T2: off=0x{0:X4}, sz={1}" -f $t2_off, $t2_sz)
Write-Host ("T3: off=0x{0:X4}, sz={1}" -f $t3_off, $t3_sz)
Write-Host ("T4: off=0x{0:X4}, sz={1}" -f $t4_off, $t4_sz)

# Let's inspect the first 64 bytes of Table 1
$stream.Seek($t1_off, [System.IO.SeekOrigin]::Begin) | Out-Null
$t1_header = $reader.ReadBytes(32)
$hex = ($t1_header | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
Write-Host ("T1 header: " + $hex)

$stream.Seek($t2_off, [System.IO.SeekOrigin]::Begin) | Out-Null
$t2_header = $reader.ReadBytes(32)
$hex = ($t2_header | ForEach-Object { '{0:X2}' -f $_ }) -join ' '
Write-Host ("T2 header: " + $hex)
