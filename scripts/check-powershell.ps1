Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$failures = @()
Get-ChildItem "$PSScriptRoot/*.ps1" | ForEach-Object {
    $tokens = $null
    $errors = $null
    [void][Management.Automation.Language.Parser]::ParseFile($_.FullName, [ref]$tokens, [ref]$errors)
    $failures += $errors
}
if ($failures.Count -gt 0) { throw ($failures | Out-String) }
. "$PSScriptRoot/windows-common.ps1"
# Verify process-tree accounting without inspecting or stopping any host process.
function Get-CimInstance([string]$ClassName) {
    if ($ClassName -eq 'Win32_Process') {
        return @(
            [pscustomobject]@{ ProcessId = 100; ParentProcessId = 1 },
            [pscustomobject]@{ ProcessId = 101; ParentProcessId = 100 },
            [pscustomobject]@{ ProcessId = 102; ParentProcessId = 101 },
            [pscustomobject]@{ ProcessId = 200; ParentProcessId = 1 }
        )
    }
    return @(
        [pscustomobject]@{ IDProcess = 100; WorkingSetPrivate = 10; PrivateBytes = 20 },
        [pscustomobject]@{ IDProcess = 101; WorkingSetPrivate = 30; PrivateBytes = 40 },
        [pscustomobject]@{ IDProcess = 102; WorkingSetPrivate = 50; PrivateBytes = 60 },
        [pscustomobject]@{ IDProcess = 200; WorkingSetPrivate = 999; PrivateBytes = 999 }
    )
}
$sample = Get-ErpMemory 100
if ($sample.private_working_set_bytes -ne 90 -or $sample.private_commit_bytes -ne 120 -or $sample.process_ids.Count -ne 3) {
    throw 'Process-tree memory accounting failed.'
}
Write-Output 'PowerShell syntax and process-tree accounting passed.'
