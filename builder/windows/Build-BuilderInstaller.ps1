[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageDirectory,
    [string]$Compiler = 'ISCC.exe'
)

$ErrorActionPreference = 'Stop'
$package = (Resolve-Path -LiteralPath $PackageDirectory).Path
$version = & (Join-Path $PSScriptRoot '../Get-BuilderVersion.ps1')
$stamp = (Get-Content -LiteralPath (Join-Path $package 'App/builder-version.txt') -Raw).Trim()
if ($stamp -ne $version) { throw "Expected a staged Builder $version package." }
$definition = Join-Path $PSScriptRoot '../../packaging/windows/builder.iss'
& $Compiler '/Qp' "/DBuilderVersion=$version" "/DPackagePath=$package" "/O$package" $definition
if ($LASTEXITCODE) { throw "Inno Setup failed with exit code $LASTEXITCODE." }
Get-Item -LiteralPath (Join-Path $package "Cinnabar-Builder-v$version-windows-x64-setup.exe")
