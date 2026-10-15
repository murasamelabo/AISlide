[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [switch]$AllowOfficeHistogramBinning,
    [ValidateSet('Office2016', 'Office2019', 'Office2021', 'Microsoft365')]
    [string]$TargetVersion = 'Office2016'
)

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

function Test-OfficeHistogramBinningNode {
    param($Node, $Part)

    $namespace = 'http://schemas.microsoft.com/office/drawing/2014/chartex'
    if ($null -eq $Node -or $null -eq $Part -or $Node.NamespaceUri -cne $namespace -or
        $Node.LocalName -cnotin @('binCount', 'binSize')) { return $false }
    $attributes = @($Node.GetAttributes())
    if ($attributes.Count -ne 1 -or $attributes[0].LocalName -cne 'val' -or
        $attributes[0].NamespaceUri -cne '' -or $Node.InnerXml -cne '' -or $Node.InnerText -cne '') { return $false }

    $ancestor = $Node.Parent
    $binning = $ancestor
    $series = $null
    $chartSpace = $null
    foreach ($name in @('binning', 'layoutPr', 'series', 'plotAreaRegion', 'plotArea', 'chart', 'chartSpace')) {
        if ($null -eq $ancestor -or $ancestor.LocalName -cne $name -or $ancestor.NamespaceUri -cne $namespace) { return $false }
        if ($name -ceq 'series') { $series = $ancestor }
        if ($name -ceq 'chartSpace') { $chartSpace = $ancestor }
        $ancestor = $ancestor.Parent
    }
    $layoutAttributes = @($series.GetAttributes() | Where-Object { $_.LocalName -ceq 'layoutId' -and $_.NamespaceUri -ceq '' })
    if ($null -ne $ancestor -or -not [object]::ReferenceEquals($chartSpace, $Part.RootElement) -or
        $layoutAttributes.Count -ne 1 -or $layoutAttributes[0].Value -cne 'clusteredColumn' -or
        $binning.ChildElements.Count -ne 1 -or -not [object]::ReferenceEquals($binning.FirstChild, $Node)) { return $false }

    try {
        if ($Node.LocalName -ceq 'binCount') {
            $count = [System.Xml.XmlConvert]::ToInt32($attributes[0].Value)
            return $count -ge 1 -and $count -le 128
        }
        $width = [System.Xml.XmlConvert]::ToDouble($attributes[0].Value)
        return -not [double]::IsNaN($width) -and -not [double]::IsInfinity($width) -and $width -gt 0 -and $width -le 1e15
    } catch [System.FormatException] { return $false }
    catch [System.OverflowException] { return $false }
}

$document = [DocumentFormat.OpenXml.Packaging.PresentationDocument]::Open((Resolve-Path -LiteralPath $Path).Path, $false)
try {
    $validator = [DocumentFormat.OpenXml.Validation.OpenXmlValidator]::new([DocumentFormat.OpenXml.FileFormatVersions]$TargetVersion)
    $validator.MaxNumberOfErrors = 256
    $issues = @($validator.Validate($document))
    $knownCount = 0
    foreach ($issue in $issues) {
        $known = $false
        if ($issue.ErrorType -eq [DocumentFormat.OpenXml.Validation.ValidationErrorType]::Schema -and
            $issue.Id -cin @('Sch_UndeclaredAttribute', 'Sch_ElementValueDataTypeDetailed') -and
            (Test-OfficeHistogramBinningNode -Node $issue.Node -Part $issue.Part)) {
            $pair = @($issues | Where-Object {
                [object]::ReferenceEquals($_.Node, $issue.Node) -and [object]::ReferenceEquals($_.Part, $issue.Part)
            })
            $known = $pair.Count -eq 2 -and
                @($pair | Where-Object { $_.ErrorType -eq [DocumentFormat.OpenXml.Validation.ValidationErrorType]::Schema -and $_.Id -ceq 'Sch_UndeclaredAttribute' }).Count -eq 1 -and
                @($pair | Where-Object { $_.ErrorType -eq [DocumentFormat.OpenXml.Validation.ValidationErrorType]::Schema -and $_.Id -ceq 'Sch_ElementValueDataTypeDetailed' }).Count -eq 1
        }
        if ($known) { $knownCount++ }
        [pscustomobject]@{
            Description = $issue.Description
            Part = if ($null -ne $issue.Part) { $issue.Part.Uri.ToString() } else { $null }
            XPath = if ($null -ne $issue.Path) { $issue.Path.XPath } else { $null }
            Id = $issue.Id
            Classification = if ($known) { 'OfficeHistogramBinning' } else { 'Unexpected' }
            Severity = if ($known -and $AllowOfficeHistogramBinning) { 'Warning' } else { 'Error' }
        } | ConvertTo-Json -Compress
    }
    $issueLimitReached = $issues.Count -ge $validator.MaxNumberOfErrors
    $unexpectedCount = $issues.Count - $knownCount
    $gateAccepted = -not $issueLimitReached -and $unexpectedCount -eq 0 -and ($issues.Count -eq 0 -or $AllowOfficeHistogramBinning)
    [pscustomobject]@{
        Mode = if ($AllowOfficeHistogramBinning) { 'OfficeHistogramBinningCompatibility' } else { 'Strict' }
        TargetVersion = $TargetVersion
        SchemaValid = $issues.Count -eq 0 -and -not $issueLimitReached
        IssueCount = $issues.Count
        KnownCompatibilityIssueCount = $knownCount
        UnexpectedIssueCount = $unexpectedCount
        IssueLimitReached = $issueLimitReached
        GateAccepted = $gateAccepted
        CompatibilityAccepted = $gateAccepted -and $knownCount -gt 0 -and $AllowOfficeHistogramBinning
    } | ConvertTo-Json -Compress
    if ($knownCount -gt 0 -and $AllowOfficeHistogramBinning) {
        Write-Warning "Known Office histogram binning schema mismatch: $knownCount known schema deviations (not schema-valid; not visual parity)." 3>&1 |
            ForEach-Object { [Console]::Error.WriteLine("WARNING: $_") }
    }
    if ($issueLimitReached) { throw 'Open XML validation issue limit reached (256); result may be incomplete.' }
    if (-not $gateAccepted) { throw "$($issues.Count) Open XML validation errors ($unexpectedCount unexpected)" }
    if ($knownCount -gt 0) {
        Write-Output 'Office compatibility validation accepted WITH known schema deviations (not schema-valid; not visual parity).'
    } else {
        Write-Output 'Open XML schema validation passed (not visual parity).'
    }
} finally { $document.Dispose() }