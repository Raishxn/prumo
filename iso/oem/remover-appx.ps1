$lista = Get-Content "$PSScriptRoot\remover-appx.txt" |
    ForEach-Object { ($_ -split '#')[0].Trim() } | Where-Object { $_ }
$instalados = Get-AppxProvisionedPackage -Online
foreach ($p in $lista) {
    $alvo = $instalados | Where-Object { $_.DisplayName -eq $p }
    if (-not $alvo) { "ausente: $p"; continue }
    try { $alvo | Remove-AppxProvisionedPackage -Online -ErrorAction Stop | Out-Null; "removido: $p" }
    catch { "falhou: $p - $($_.Exception.Message)" }
}

# Paginação fixa de 2 GB: com 32 GB de RAM o automático reservaria bem mais.
try {
    Get-CimInstance Win32_ComputerSystem | Set-CimInstance -Property @{ AutomaticManagedPagefile = $false }
    Get-CimInstance Win32_PageFileSetting | Remove-CimInstance
    New-CimInstance -ClassName Win32_PageFileSetting -Property @{ Name = 'C:\pagefile.sys'; InitialSize = [uint32]2048; MaximumSize = [uint32]2048 } | Out-Null
    "paginacao: 2 GB"
} catch { "paginacao falhou: $($_.Exception.Message)" }
