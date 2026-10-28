Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-Windows {
    if ($env:OS -ne 'Windows_NT') { throw 'Run this script on Windows.' }
}

function Get-ErpWebViewVersion {
    $key = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $versions = @()
    foreach ($hive in @([Microsoft.Win32.RegistryHive]::LocalMachine, [Microsoft.Win32.RegistryHive]::CurrentUser)) {
        foreach ($view in @([Microsoft.Win32.RegistryView]::Registry32, [Microsoft.Win32.RegistryView]::Registry64)) {
            $base = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive, $view)
            $entry = $null
            try {
                $entry = $base.OpenSubKey($key)
                if ($null -ne $entry) {
                    $parsed = $null
                    if ([Version]::TryParse([string]$entry.GetValue('pv'), [ref]$parsed)) { $versions += $parsed }
                }
            } finally {
                if ($null -ne $entry) { $entry.Dispose() }
                $base.Dispose()
            }
        }
    }
    if ($versions.Count -eq 0) { throw 'Install the offline WebView2 Evergreen x64 runtime first.' }
    $version = $versions | Sort-Object -Descending | Select-Object -First 1
    if ($version -lt [Version]'109.0.0.0') { throw 'WebView2 109 or later is required.' }
    return $version.ToString()
}

function Start-ErpMeasured([string]$Executable, [string]$OutputDirectory) {
    if (Get-Process -Name 'plumbing-erp' -ErrorAction SilentlyContinue) { throw 'Close the running ERP before this check.' }
    New-Item -ItemType Directory -Force $OutputDirectory | Out-Null
    $report = Join-Path (Resolve-Path $OutputDirectory) (([Guid]::NewGuid().ToString()) + '.startup.json')
    $previous = $env:PLUMBING_ERP_STARTUP_REPORT
    $watch = [Diagnostics.Stopwatch]::StartNew()
    try {
        $env:PLUMBING_ERP_STARTUP_REPORT = $report
        $process = Start-Process -FilePath $Executable -WorkingDirectory (Split-Path $Executable) -PassThru
    } finally { $env:PLUMBING_ERP_STARTUP_REPORT = $previous }
    while ($watch.Elapsed.TotalSeconds -lt 60) {
        $process.Refresh()
        if ($process.HasExited) { throw "ERP exited before its frontend was ready: $($process.ExitCode)" }
        if (Test-Path $report) {
            $value = $null
            try { $value = Get-Content -Raw $report | ConvertFrom-Json } catch { }
            if ($null -ne $value) {
                if ($value.pid -ne $process.Id) { throw 'Unexpected startup report PID.' }
                $watch.Stop()
                return @{ Process = $process; Ready = $value; WallMilliseconds = $watch.Elapsed.TotalMilliseconds }
            }
        }
        Start-Sleep -Milliseconds 50
    }
    throw "Frontend readiness timed out. Inspect the application window; PID $($process.Id)."
}

function Stop-ErpGracefully([Diagnostics.Process]$Process) {
    $Process.Refresh()
    if ($Process.HasExited) { return }
    if (-not $Process.CloseMainWindow()) { throw "Close the test application manually; PID $($Process.Id)." }
    if (-not $Process.WaitForExit(15000)) { throw 'ERP did not close; it has not been force-killed.' }
}

function Get-ErpMemory([int]$RootProcessId) {
    $processes = @(Get-CimInstance Win32_Process)
    $ids = [Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootProcessId)
    do {
        $added = $false
        foreach ($entry in $processes) {
            if ($ids.Contains([int]$entry.ParentProcessId) -and $ids.Add([int]$entry.ProcessId)) { $added = $true }
        }
    } while ($added)
    $counters = @(Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Where-Object { $ids.Contains([int]$_.IDProcess) })
    $observed = [Collections.Generic.HashSet[int]]::new()
    foreach ($counter in $counters) { [void]$observed.Add([int]$counter.IDProcess) }
    if (-not $ids.SetEquals($observed)) { throw 'Incomplete memory counters for the ERP process tree; repeat the measurement.' }
    return @{
        process_ids = @($ids)
        private_working_set_bytes = [long](($counters | Measure-Object WorkingSetPrivate -Sum).Sum)
        private_commit_bytes = [long](($counters | Measure-Object PrivateBytes -Sum).Sum)
        measured_at = [DateTimeOffset]::UtcNow.ToString('o')
    }
}

function Write-ErpJson([object]$Value, [string]$Path) {
    New-Item -ItemType Directory -Force (Split-Path $Path) | Out-Null
    $Value | ConvertTo-Json -Depth 12 | Set-Content -Encoding UTF8 $Path
}
