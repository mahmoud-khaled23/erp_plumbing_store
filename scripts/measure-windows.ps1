param(
    [Parameter(Mandatory)][string]$Executable,
    [string]$OutputDirectory = "$PSScriptRoot/../output/windows-performance",
    [ValidateRange(1, 10)][int]$Runs = 5,
    [switch]$ColdAfterReboot,
    [switch]$EnforceBudgets
)
. "$PSScriptRoot/windows-common.ps1"
Assert-Windows
$Executable = (Resolve-Path $Executable).Path
$runtime = Get-ErpWebViewVersion
if ($ColdAfterReboot -and $Runs -ne 1) { throw 'ColdAfterReboot requires Runs=1; reboot immediately before running it.' }
if ($EnforceBudgets -and -not $ColdAfterReboot) { throw 'A warm launch cannot validate the cold-start budget.' }
$results = @()
for ($i = 0; $i -lt $Runs; $i++) {
    $launch = Start-ErpMeasured $Executable $OutputDirectory
    try {
        Start-Sleep -Seconds 15
        $samples = @()
        for ($sample = 0; $sample -lt 10; $sample++) {
            $samples += Get-ErpMemory $launch.Process.Id
            Start-Sleep -Seconds 1
        }
        $results += @{
            launch = $launch.Ready
            wall_startup_ms = $launch.WallMilliseconds
            idle_samples = $samples
            max_idle_private_working_set_bytes = [long](($samples.private_working_set_bytes | Measure-Object -Maximum).Maximum)
        }
    } finally { Stop-ErpGracefully $launch.Process }
}
$os = Get-CimInstance Win32_OperatingSystem
$hardware = Get-CimInstance Win32_ComputerSystem
$maximumMemory = [long](($results.max_idle_private_working_set_bytes | Measure-Object -Maximum).Maximum)
$coldPass = $null
if ($ColdAfterReboot) { $coldPass = $results[0].wall_startup_ms -lt 2000 }
$report = @{
    executable = $Executable
    sha256 = (Get-FileHash $Executable -Algorithm SHA256).Hash.ToLowerInvariant()
    os = $os.Caption; os_version = $os.Version; last_boot = $os.LastBootUpTime.ToString('o')
    cpu = @(Get-CimInstance Win32_Processor | ForEach-Object Name)
    total_physical_memory_bytes = [long]$hardware.TotalPhysicalMemory
    webview2 = $runtime
    cold_after_reboot_asserted_by_operator = [bool]$ColdAfterReboot
    cold_start_below_2000_ms = $coldPass
    idle_below_150000000_bytes = $maximumMemory -lt 150000000
    max_idle_private_working_set_bytes = $maximumMemory
    runs = $results
}
$destination = Join-Path $OutputDirectory ('performance-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss') + '.json')
Write-ErpJson $report $destination
Write-Output "Performance evidence: $destination"
if ($EnforceBudgets -and (-not $coldPass -or $maximumMemory -ge 150000000)) { throw 'The measured application did not meet the configured performance budgets.' }
