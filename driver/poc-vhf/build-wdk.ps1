# Compilation uniquement du prototype PadBridge KMDF/VHF.
# N'installe RIEN, ne signe RIEN, ne modifie ni Secure Boot ni le registre.
[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Debug"
)

$ErrorActionPreference = "Stop"
$project = Join-Path $PSScriptRoot "PadBridgeVhf.vcxproj"

$programFilesX86 = [Environment]::GetFolderPath("ProgramFilesX86")
$vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw "Visual Studio Installer introuvable. Installer Visual Studio avec le workload C++ et le composant Windows Driver Kit."
}

$installation = & $vswhere -latest -products "*" -requires Microsoft.Component.MSBuild -property installationPath
if ($LASTEXITCODE -ne 0 -or -not $installation) {
    throw "MSBuild non detecte par vswhere. Verifier l'installation Visual Studio."
}

$msbuild = Join-Path ($installation | Select-Object -First 1) "MSBuild\Current\Bin\MSBuild.exe"
if (-not (Test-Path -LiteralPath $msbuild)) {
    throw "MSBuild.exe introuvable a l'emplacement attendu : $msbuild"
}

Write-Host "Projet : $project"
Write-Host "Compilateur : $msbuild"
Write-Host "Configuration : $Configuration | x64"
Write-Host "Compilation source seulement : aucun INF, aucun ajout de pilote au magasin Windows."

& $msbuild $project "/t:Build" "/p:Configuration=$Configuration" "/p:Platform=x64" "/m:1" "/nr:false" "/v:minimal" "/nologo"
if ($LASTEXITCODE -ne 0) {
    throw "Compilation WDK echouee (code $LASTEXITCODE). Copier les premieres erreurs du journal."
}

$binary = Get-ChildItem -LiteralPath $PSScriptRoot -Filter "PadBridgeVhf.sys" -Recurse -File -ErrorAction SilentlyContinue | 
    Where-Object { $_.FullName -match [regex]::Escape($Configuration) } |
    Select-Object -First 1
if ($null -eq $binary) {
    Write-Warning "MSBuild a termine sans erreur mais le binaire .sys n'a pas ete localise automatiquement. Consulter le journal."
} else {
    Write-Host "Binaire de compilation : $($binary.FullName)"
}
Write-Host "ATTENTION : .sys NON signe, NON installe et NON qualifie pour un test de chargement."
