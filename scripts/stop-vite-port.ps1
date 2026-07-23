$port = 5173
$owners = @()

try {
    $owners += Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction Stop |
        Select-Object -ExpandProperty OwningProcess
} catch {
    $owners += netstat -ano |
        Select-String ":$port\s+.*LISTENING\s+\d+" |
        ForEach-Object {
            $parts = ($_.Line -replace '^\s+', '') -split '\s+'
            [int]$parts[-1]
        }
}

$owners | Sort-Object -Unique | ForEach-Object {
    if ($_ -and $_ -ne 0) {
        Write-Host "Stopping process $_ on dev port $port"
        Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue
    }
}
