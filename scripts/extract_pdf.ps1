$content = [System.IO.File]::ReadAllBytes('extracted\Windows 7_Notice.pdf')
$chars = [System.Text.Encoding]::ASCII.GetString($content)
# Find printable strings of length >= 4
$matches = [regex]::Matches($chars, '[\x20-\x7E]{4,}')
foreach ($m in $matches) {
    if ($m.Value -match 'Windows|Canon|LBP|64|32|driver|Notice|support|Vista|7') {
        Write-Host $m.Value
    }
}
