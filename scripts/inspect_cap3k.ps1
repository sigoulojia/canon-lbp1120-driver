$bytes = [System.IO.File]::ReadAllBytes("F:\Games\Nouveau dossier (3)\extracted\WinVista\CAP3K.DLL")
$str = ""
for ($i=0; $i -lt $bytes.Length; $i++) {
    $c = $bytes[$i]
    if ($c -ge 32 -and $c -le 126) {
        $str += [char]$c
    } else {
        if ($str.Length -ge 4 -and ($str -match "WinUsb|Write|Read|Pipe|IOCTL|DeviceIoControl|Usb|Bulk|Timeout")) {
            Write-Host $str
        }
        $str = ""
    }
}
