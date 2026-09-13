[CmdletBinding()]
param([Parameter(Mandatory = $true)][string]$Path)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path -Parent $PSScriptRoot
$tools = Join-Path $root '.tools/openxml'
[System.IO.Directory]::CreateDirectory($tools) | Out-Null
foreach ($package in @(
    @{ Name = 'system.io.packaging'; Version = '10.0.0' },
    @{ Name = 'documentformat.openxml.framework'; Version = '3.5.1' },
    @{ Name = 'documentformat.openxml'; Version = '3.5.1' }
)) {
    $directory = Join-Path $tools $package.Name
    if (-not (Test-Path -LiteralPath $directory)) {
        $archive = Join-Path $tools ($package.Name + '.zip')
        $uri = 'https://pkgs.dev.azure.com/dnceng/public/_packaging/dotnet-public/nuget/v3/flat2/{0}/{1}/{0}.{1}.nupkg' -f $package.Name, $package.Version
        Invoke-WebRequest -Uri $uri -OutFile $archive
        [System.IO.Compression.ZipFile]::ExtractToDirectory($archive, $directory)
    }
}

Add-Type -Path (Join-Path $tools 'system.io.packaging/lib/net8.0/System.IO.Packaging.dll')
Add-Type -Path (Join-Path $tools 'documentformat.openxml.framework/lib/net8.0/DocumentFormat.OpenXml.Framework.dll')
Add-Type -Path (Join-Path $tools 'documentformat.openxml/lib/net8.0/DocumentFormat.OpenXml.dll')
$document = [DocumentFormat.OpenXml.Packaging.PresentationDocument]::Open((Resolve-Path -LiteralPath $Path).Path, $false)
try {
    $validator = [DocumentFormat.OpenXml.Validation.OpenXmlValidator]::new()
    $issues = @($validator.Validate($document))
    foreach ($issue in $issues) {
        [pscustomobject]@{ Description = $issue.Description; Part = $issue.Part.Uri.ToString(); XPath = $issue.Path.XPath } | ConvertTo-Json -Compress
    }
    if ($issues.Count -gt 0) { throw "$($issues.Count) Open XML validation errors" }
    Write-Output 'Open XML schema validation passed (not visual parity).'
} finally { $document.Dispose() }