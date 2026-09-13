[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [int]$ExpectedSlides = 12
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$source = (Resolve-Path -LiteralPath $Path).Path
$before = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
if (-not $PSCmdlet.ShouldProcess($source, 'Verify generated PPTX through a disposable PowerPoint copy')) { return }

$temporary = Join-Path ([System.IO.Path]::GetTempPath()) ('AISlide-verify-' + [guid]::NewGuid().ToString('N'))
[System.IO.Directory]::CreateDirectory($temporary) | Out-Null
$copy = Join-Path $temporary 'verification-copy.pptx'
Copy-Item -LiteralPath $source -Destination $copy
$hadPowerPoint = @(Get-Process -Name POWERPNT -ErrorAction SilentlyContinue).Count -gt 0
$application = $null
$presentation = $null
$priorSecurity = $null
$stage = 'COM activation'
try {
    $application = New-Object -ComObject PowerPoint.Application
    $stage = 'macro security configuration'
    $priorSecurity = $application.AutomationSecurity
    $application.AutomationSecurity = 3
    $stage = 'opening disposable PPTX'
    $presentation = $application.Presentations.Open($copy, -1, 0, 0)
    $stage = 'native object inspection'
    if ($presentation.Slides.Count -ne $ExpectedSlides) { throw 'Unexpected slide count' }
    $textShapes = 0
    $tables = 0
    $notes = 0
    foreach ($slide in $presentation.Slides) {
        foreach ($shape in $slide.Shapes) {
            if ($shape.HasTextFrame -eq -1) { $textShapes++ }
            if ($shape.HasTable -eq -1) { $tables++ }
        }
        foreach ($shape in $slide.NotesPage.Shapes) {
            if ($shape.HasTextFrame -eq -1 -and $shape.TextFrame.HasText -eq -1) { $notes++ }
        }
    }
    if ($textShapes -lt 12 -or $tables -lt 1 -or $notes -lt 12) { throw 'Native object or notes inspection failed' }
    $stage = 'slide PNG export'
    $presentation.Slides.Item(1).Export((Join-Path $temporary 'cover.png'), 'PNG', 1280, 720)
    $presentation.Slides.Item(4).Export((Join-Path $temporary 'table.png'), 'PNG', 1280, 720)
    $captureCount = @(Get-ChildItem -LiteralPath $temporary -Filter '*.png').Count
    [pscustomobject]@{
        Slides = $presentation.Slides.Count
        TextShapes = $textShapes
        Tables = $tables
        NoteTextShapes = $notes
        Captures = $captureCount
        ArtifactDirectory = $temporary
        SourceSha256 = $before
        Scope = 'Generated sample only; no general Office visual parity assertion'
    } | ConvertTo-Json
} catch {
    throw "PowerPoint verification failed during ${stage}: $($_.Exception.Message)"
} finally {
    if ($null -ne $presentation) {
        $presentation.Close()
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($presentation)
    }
    if ($null -ne $application) {
        if ($null -ne $priorSecurity) { $application.AutomationSecurity = $priorSecurity }
        if (-not $hadPowerPoint) { $application.Quit() }
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($application)
    }
    $after = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    if ($before -ne $after) { throw 'Source presentation changed during verification' }
}