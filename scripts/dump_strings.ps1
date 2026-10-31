foreach ($name in @("CAP3PMN.DLL", "CAP3K.DLL", "CAP3RDN.DLL")) {
    Write-Host "=== $name ==="
    $bytes = [System.IO.File]::ReadAllBytes("F:\Games\Nouveau dossier (3)\extracted\WinVista\$name")
    $str = ""
    for ($i=0; $i -lt $bytes.Length; $i++) {
        $c = $bytes[$i]
        if ($c -ge 32 -and $c -le 126) {
            $str += [char]$c
        } else {
            if ($str.Length -ge 4 -and ($str -match "Page|Video|Print|Packet|Status|Flow|Buffer|Wait|Ready|Chunk|Usb|Write|Pipe|Error|Band|Timeout")) {
                Write-Host $str
            }
            $str = ""
        }
    }
}
