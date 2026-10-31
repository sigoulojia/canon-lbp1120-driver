$bytes = [System.IO.File]::ReadAllBytes('capt-driver\test.capt')
$pos = 0
$pkt = 0
while ($pos -lt $bytes.Length -and $pkt -lt 10) {
    $op = [BitConverter]::ToUInt16($bytes, $pos)
    $sz = [BitConverter]::ToUInt16($bytes, $pos + 2)
    Write-Host ("Pkt {0}: Opcode 0x{1:X4}, Size {2} bytes" -f $pkt, $op, $sz)
    $pos += $sz
    $pkt++
}
