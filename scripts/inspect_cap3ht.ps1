$b = [System.IO.File]::ReadAllBytes("F:\Games\Nouveau dossier (3)\capt-driver\assets\Cap3ht.bin")
Write-Host ("Cap3ht.bin size: {0} bytes" -f $b.Length)
Write-Host "First 64 bytes:"
$hex = ""
for ($i=0; $i -lt 64; $i++) { $hex += ("{0:X2} " -f $b[$i]) }
Write-Host $hex
