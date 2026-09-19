[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [int]$ExpectedSlides = 12,
    [int]$ExpectedCharts = 0,
    [int]$ExpectedPictures = 0,
    [int]$ExpectedGraphics = 0,
    [int]$ExpectedGroups = 0,
    [int]$ExpectedConnectors = 0,
    [int]$MinimumTables = 1,
    [switch]$VerifyChartData,
    [ValidatePattern('^[A-E][1-9][0-9]{0,3}$')]
    [string]$ChartValueCell = 'B2',
    [switch]$FirstNumericChartCell,
    [switch]$InspectHistogramBins,
    [switch]$VerifyConnections,
    [int[]]$CaptureSlides = @()
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
$hadExcel = @(Get-Process -Name EXCEL -ErrorAction SilentlyContinue).Count -gt 0
$application = $null
$presentation = $null
$priorSecurity = $null
$stage = 'COM activation'
function Get-VerificationShape {
    param([object]$Shapes)
    foreach ($item in $Shapes) {
        Write-Output -NoEnumerate $item
        if ($item.Type -eq 6) { Get-VerificationShape -Shapes $item.GroupItems }
    }
}
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
    $charts = 0
    $pictures = 0
    $graphics = 0
    $groups = 0
    $connectors = 0
    $attachedConnectors = 0
    $connectionMoves = 0
    $editedWorkbooks = 0
    $histogramBins = @()
    foreach ($slide in $presentation.Slides) {
        foreach ($shape in (Get-VerificationShape -Shapes $slide.Shapes)) {
            if ($shape.HasTextFrame -eq -1) { $textShapes++ }
            if ($shape.HasTable -eq -1) { $tables++ }
            if ($shape.Type -eq 13) { $pictures++ }
            if ($shape.Type -eq 28) { $graphics++ }
            if ($shape.Type -eq 6) { $groups++ }
            if ($shape.Connector -eq -1) {
                $connectors++
                if ($VerifyConnections) {
                    if ($shape.ConnectorFormat.BeginConnected -ne -1 -or $shape.ConnectorFormat.EndConnected -ne -1) { throw 'Graph connector must be attached at both endpoints' }
                    $attachedConnectors++
                    $stage = 'moving an attached shape in the disposable presentation'
                    $target = $shape.ConnectorFormat.BeginConnectedShape
                    $targetId = $target.Id
                    $endId = $shape.ConnectorFormat.EndConnectedShape.Id
                    $originalLeft = [double]$target.Left
                    $originalTop = [double]$target.Top
                    $beforeGeometry = @($shape.Left, $shape.Top, $shape.Width, $shape.Height) -join ','
                    try {
                        $target.Left = $originalLeft + 6
                        $target.Top = $originalTop + 6
                        $afterGeometry = @($shape.Left, $shape.Top, $shape.Width, $shape.Height) -join ','
                        if ($beforeGeometry -eq $afterGeometry) { throw 'Attached connector did not follow the moved shape' }
                        if ($shape.ConnectorFormat.BeginConnectedShape.Id -ne $targetId -or $shape.ConnectorFormat.EndConnectedShape.Id -ne $endId) { throw 'Moving a shape changed connector identities' }
                        $connectionMoves++
                    } finally {
                        $target.Left = $originalLeft
                        $target.Top = $originalTop
                    }
                }
            }
            if ($shape.HasChart -eq -1) {
                $charts++
                if ($InspectHistogramBins) {
                    if ($shape.Chart.ChartType -ne 118) { throw 'Expected a native histogram chart' }
                    $histogram = $shape.Chart.ChartGroups(1)
                    $histogramBins += [pscustomobject]@{
                        ChartType = $shape.Chart.ChartType
                        Mode = [int]$histogram.BinsType
                        Count = $histogram.BinsCountValue
                        Width = $histogram.BinWidthValue
                        UnderflowEnabled = $histogram.BinsUnderflowEnabled
                        Underflow = $histogram.BinsUnderflowValue
                        OverflowEnabled = $histogram.BinsOverflowEnabled
                        Overflow = $histogram.BinsOverflowValue
                    }
                }
                if ($VerifyChartData) {
                    $stage = 'editing a chart workbook in the disposable presentation'
                    if ($shape.Chart.ChartData.IsLinked) { throw 'Chart data must be embedded, not externally linked' }
                    $workbook = $null
                    $excel = $null
                    try {
                        $shape.Chart.ChartData.Activate()
                        $workbook = $shape.Chart.ChartData.Workbook
                        $excel = $workbook.Application
                        $cell = $workbook.Worksheets.Item(1).Range($ChartValueCell)
                        if ($FirstNumericChartCell) {
                            $cell = $null
                            foreach ($address in @('B2', 'C2', 'D2', 'E2')) {
                                $candidate = $workbook.Worksheets.Item(1).Range($address)
                                if ($candidate.Value2 -is [double] -or $candidate.Value2 -is [int]) { $cell = $candidate; break }
                            }
                            if ($null -eq $cell) { throw 'No numeric chart sample/leaf cell in B2:E2' }
                        }
                        if ($cell.Value2 -isnot [double] -and $cell.Value2 -isnot [int]) { throw 'Chart value cell must contain a numeric sample or leaf value' }
                        $originalValue = [double]$cell.Value2
                        $cell.Value2 = $originalValue + 1
                        if ([double]$cell.Value2 -ne $originalValue + 1) { throw 'Embedded chart data did not accept an edit' }
                        $cell.Value2 = $originalValue
                        $editedWorkbooks++
                    } finally {
                        if ($null -ne $workbook) { $workbook.Close($false); [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($workbook) }
                        if ($null -ne $excel) {
                            if (-not $hadExcel) { $excel.Quit() }
                            [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($excel)
                        }
                    }
                }
            }
        }
        foreach ($shape in $slide.NotesPage.Shapes) {
            if ($shape.HasTextFrame -eq -1 -and $shape.TextFrame.HasText -eq -1) { $notes++ }
        }
    }
    if ($textShapes -lt $ExpectedSlides -or $tables -lt $MinimumTables -or $notes -lt $ExpectedSlides) { throw 'Native object or notes inspection failed' }
    if ($charts -ne $ExpectedCharts -or $pictures -ne $ExpectedPictures -or $graphics -ne $ExpectedGraphics) { throw "Unexpected native count: charts=$charts pictures=$pictures graphics=$graphics" }
    if ($groups -ne $ExpectedGroups -or $connectors -ne $ExpectedConnectors) { throw 'Unexpected native group or connector count' }
    if ($VerifyChartData -and $editedWorkbooks -ne $ExpectedCharts) { throw 'Chart workbook editing verification incomplete' }
    if ($VerifyConnections -and $attachedConnectors -ne $ExpectedConnectors) { throw 'Connector attachment verification incomplete' }
    $stage = 'slide PNG export'
    $pageWidth = [double]$presentation.PageSetup.SlideWidth
    $pageHeight = [double]$presentation.PageSetup.SlideHeight
    if ($pageWidth -le 0 -or $pageHeight -le 0) { throw 'Invalid native page dimensions' }
    $longest = [Math]::Max($pageWidth, $pageHeight)
    $captureWidth = [Math]::Max(1, [int][Math]::Round(1280 * $pageWidth / $longest))
    $captureHeight = [Math]::Max(1, [int][Math]::Round(1280 * $pageHeight / $longest))
    $presentation.Slides.Item(1).Export((Join-Path $temporary 'cover.png'), 'PNG', $captureWidth, $captureHeight)
    if ($ExpectedSlides -ge 4) { $presentation.Slides.Item(4).Export((Join-Path $temporary 'table.png'), 'PNG', $captureWidth, $captureHeight) }
    if ($ExpectedCharts -gt 0 -and $ExpectedSlides -ge 5) { $presentation.Slides.Item(5).Export((Join-Path $temporary 'chart.png'), 'PNG', $captureWidth, $captureHeight) }
    if ($ExpectedGroups -gt 0 -and $ExpectedSlides -ge 8) { $presentation.Slides.Item(8).Export((Join-Path $temporary 'diagram.png'), 'PNG', $captureWidth, $captureHeight) }
    if ($ExpectedPictures + $ExpectedGraphics -gt 0 -and $ExpectedSlides -ge 11) { $presentation.Slides.Item(11).Export((Join-Path $temporary 'picture.png'), 'PNG', $captureWidth, $captureHeight) }
    foreach ($slideNumber in $CaptureSlides) {
        if ($slideNumber -lt 1 -or $slideNumber -gt $presentation.Slides.Count) { throw 'Capture slide number is outside presentation' }
        $presentation.Slides.Item($slideNumber).Export((Join-Path $temporary ('slide-{0:d2}.png' -f $slideNumber)), 'PNG', $captureWidth, $captureHeight)
    }
    $captureCount = @(Get-ChildItem -LiteralPath $temporary -Filter '*.png').Count
    [pscustomobject]@{
        Slides = $presentation.Slides.Count
        TextShapes = $textShapes
        Tables = $tables
        Charts = $charts
        Pictures = $pictures
        Graphics = $graphics
        Groups = $groups
        Connectors = $connectors
        AttachedConnectorsVerified = $attachedConnectors
        ConnectionMovesVerified = $connectionMoves
        EmbeddedWorkbooksEdited = $editedWorkbooks
        HistogramBins = $histogramBins
        NoteTextShapes = $notes
        Captures = $captureCount
        CaptureDimensions = @($captureWidth, $captureHeight)
        ArtifactDirectory = $temporary
        SourceSha256 = $before
        Scope = 'Generated sample only; no general Office visual parity assertion'
    } | ConvertTo-Json
} catch {
    throw "PowerPoint verification failed during ${stage}: $($_.Exception.Message)"
} finally {
    if ($null -ne $presentation) {
        $presentation.Saved = -1
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