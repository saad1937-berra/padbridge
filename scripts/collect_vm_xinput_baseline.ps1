# PadBridge VM XInput baseline: read-only and fail-closed on the real PC.
# This DOES NOT create checkpoints, install a service, sign/load a driver,
# change system policies, send vibration, or modify Bluetooth devices.
# Use ONLY in a disposable Windows 11 Hyper-V guest with a checkpoint
# already created from the Hyper-V host management UI.

[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [ValidateSet('before', 'after-idle', 'after-activity')]
    [string]$Stage,

    [string]$ProbePath = (Join-Path $PSScriptRoot '..\src-tauri\target\debug\padbridge-xinput-probe.exe'),

    [string]$OutputDirectory = (Join-Path $PSScriptRoot '..\vm-evidence')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$system = Get-CimInstance -ClassName Win32_ComputerSystem
$os = Get-CimInstance -ClassName Win32_OperatingSystem
$manufacturer = [string]$system.Manufacturer
$model = [string]$system.Model
$inHyperVGuest = (
    $manufacturer -eq 'Microsoft Corporation' -and
    ($model -match 'Virtual Machine|Hyper-V')
)
if (-not $inHyperVGuest) {
    throw 'Refusing to run outside an identified Microsoft Hyper-V guest. No probe executed.'
}
if (-not ([string]$os.Caption -match 'Windows 11')) {
    throw 'Refusing to run on a guest that does not report Windows 11.'
}

# A guest cannot reliably inspect or create its own host checkpoint.
Write-Host 'Hyper-V guest identified. Verify that a host-side checkpoint exists before proceeding.'
Write-Host 'This command only collects the four XInput slots. No controller commands are sent.'

$binary = [System.IO.Path]::GetFullPath($ProbePath)
if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) {
    throw "Missing XInput probe: $binary. Compile the read-only CLI first."
}

# The standalone executable prints JSON on stdout and a warning on stderr.
$json = (& $binary --snapshot | Out-String)
if ($LASTEXITCODE -ne 0) {
    throw "The read-only XInput probe failed with exit code $LASTEXITCODE."
}
$slots = @($json | ConvertFrom-Json)
if ($slots.Count -ne 4) {
    throw 'Unexpected XInput snapshot length (wanted exactly four slots).'
}
$indices = @($slots | ForEach-Object { $_.slot } | Sort-Object)
if (($indices -join ',') -ne '0,1,2,3') {
    throw 'Unexpected XInput slot indices; refusing to record evidence.'
}

# Only write raw XInput slot snapshots, no usernames, paths, serials or
# PnP identifiers. Python comparator rejects extra fields.
$destination = [System.IO.Path]::GetFullPath($OutputDirectory)
[System.IO.Directory]::CreateDirectory($destination) | Out-Null
$file = Join-Path $destination ($Stage + '.json')
$slots | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $file -Encoding utf8
Write-Host "XInput $Stage snapshot written to $file"
Write-Host 'WARNING: a connected XInput slot is NOT proof of PadBridge attribution.'
