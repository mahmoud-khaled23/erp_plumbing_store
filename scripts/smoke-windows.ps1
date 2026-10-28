param(
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][switch]$DisposableProfile,
    [string]$OutputDirectory = "$PSScriptRoot/../output/windows-smoke"
)
. "$PSScriptRoot/windows-common.ps1"
Assert-Windows
if (-not $DisposableProfile) { throw 'Use a disposable Windows account or CI runner for install/uninstall tests.' }
$runtime = Get-ErpWebViewVersion
$Installer = (Resolve-Path $Installer).Path
$appData = Join-Path $env:LOCALAPPDATA 'com.plumbingstore.erp'
if (Test-Path $appData) { throw "Existing ERP data must not be touched: $appData" }
if (Test-Path 'HKCU:/Software/Microsoft/Windows/CurrentVersion/Uninstall/PlumbingStore') { throw 'The ERP is already installed in this account.' }
# Keep source ASCII for Windows PowerShell 5.1 while exercising real Arabic paths.
$arabicStore = -join ([char[]](0x0645, 0x062A, 0x062C, 0x0631))
$installDirectory = Join-Path $env:LOCALAPPDATA ('ERP Test ' + $arabicStore + ' ' + [Guid]::NewGuid().ToString())
$executable = Join-Path $installDirectory 'plumbing-erp.exe'
function Install-TestPackage {
    # NSIS requires /D last; its value includes all remaining characters, including spaces.
    $setup = Start-Process -FilePath $Installer -ArgumentList "/S /NS /D=$installDirectory" -Wait -PassThru
    if ($setup.ExitCode -ne 0) { throw "NSIS failed: $($setup.ExitCode)" }
    if (-not (Test-Path $executable)) { throw 'Installer did not create the executable.' }
}
Install-TestPackage
$first = Start-ErpMeasured $executable $OutputDirectory
Stop-ErpGracefully $first.Process
& python "$PSScriptRoot/check-installed-store.py" (Join-Path $appData 'store/store.db')
if ($LASTEXITCODE -ne 0) { throw 'Installed SQLite integrity check failed.' }
$marker = Join-Path $appData 'store/preserve-on-upgrade.txt'
$token = [Guid]::NewGuid().ToString()
Set-Content -Path $marker -Value $token -Encoding UTF8
$before = (Get-FileHash (Join-Path $appData 'store/store.db') -Algorithm SHA256).Hash
Install-TestPackage
if ((Get-Content -Raw $marker).Trim() -ne $token) { throw 'Reinstallation changed user data.' }
if ((Get-FileHash (Join-Path $appData 'store/store.db') -Algorithm SHA256).Hash -ne $before) { throw 'Reinstallation changed SQLite.' }
$second = Start-ErpMeasured $executable $OutputDirectory
Stop-ErpGracefully $second.Process
$uninstaller = Join-Path $installDirectory 'uninstall.exe'
$remove = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$installDirectory" -Wait -PassThru
if ($remove.ExitCode -ne 0) { throw "Uninstaller failed: $($remove.ExitCode)" }
if (Test-Path $executable) { throw 'Uninstaller left the application executable.' }
if ((Get-Content -Raw $marker).Trim() -ne $token) { throw 'Uninstallation deleted store data.' }
& python "$PSScriptRoot/check-installed-store.py" (Join-Path $appData 'store/store.db')
if ($LASTEXITCODE -ne 0) { throw 'Retained SQLite integrity check failed.' }
Write-ErpJson @{
    installer = $Installer; sha256 = (Get-FileHash $Installer -Algorithm SHA256).Hash.ToLowerInvariant()
    webview2 = $runtime; first_launch = $first.Ready; second_launch = $second.Ready
    install = 'passed'; same_version_reinstall = 'passed'; uninstall_keeps_data = 'passed'
    installed_store_integrity = 'passed'; tested_at = [DateTimeOffset]::UtcNow.ToString('o')
    retained_test_data = $appData
} (Join-Path $OutputDirectory 'smoke.json')
Write-Output 'Windows installation smoke test passed. Test data was intentionally retained.'
