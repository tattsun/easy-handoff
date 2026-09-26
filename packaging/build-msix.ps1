# Build the (unsigned) MSIX for Microsoft Store submission. The Store signs it on ingestion.
# Run from the repo root after `cargo build --release`:
#   pwsh packaging/build-msix.ps1 -Version 0.2.0
param(
    [Parameter(Mandatory)] [string] $Version
)
$ErrorActionPreference = "Stop"

$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" |
    Sort-Object FullName | Select-Object -Last 1
if (-not $makeappx) { throw "makeappx.exe not found (install the Windows SDK)" }

# Store packages need a four-part version whose last part is 0.
$packageVersion = "$Version.0"

$stage = "target\msix"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $stage | Out-Null
Copy-Item target\release\easy-handoff.exe $stage
Copy-Item packaging\msix\Assets $stage -Recurse
(Get-Content packaging\msix\AppxManifest.xml -Raw).Replace("{VERSION}", $packageVersion) |
    Set-Content "$stage\AppxManifest.xml" -NoNewline

New-Item -ItemType Directory dist -Force | Out-Null
$out = "dist\easy-handoff-$Version.msix"
& $makeappx.FullName pack /d $stage /p $out /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "Created $out"
