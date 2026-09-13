$ErrorActionPreference = 'Stop'
node tools/cargo.mjs build -p aislide-cli
if ($LASTEXITCODE -ne 0) { throw 'CLI build failed' }
$path = node tools/demo.mjs
if ($LASTEXITCODE -ne 0) { throw 'Demo generation failed' }
Write-Output "Generated: $path"
& (Join-Path $PSScriptRoot 'validate-openxml.ps1') -Path $path
& (Join-Path $PSScriptRoot 'verify-powerpoint.ps1') -Path $path