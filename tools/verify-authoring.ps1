[CmdletBinding(SupportsShouldProcess = $true)]
param([Parameter(Mandatory = $true)][string]$Path)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$source = (Resolve-Path -LiteralPath $Path).Path
$sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
if (-not $PSCmdlet.ShouldProcess($source, 'Verify generated authoring fixture in a disposable PowerPoint copy')) { return }
$directory = Join-Path ([System.IO.Path]::GetTempPath()) ('AISlide-authoring-' + [guid]::NewGuid().ToString('N'))
[System.IO.Directory]::CreateDirectory($directory) | Out-Null
$copy = Join-Path $directory 'authoring-copy.pptx'
Copy-Item -LiteralPath $source -Destination $copy
$hadPowerPoint = @(Get-Process -Name POWERPNT -ErrorAction SilentlyContinue).Count -gt 0
$application = $null
$presentation = $null
$priorSecurity = $null
try {
    $application = New-Object -ComObject PowerPoint.Application
    $priorSecurity = $application.AutomationSecurity
    $application.AutomationSecurity = 3
    $presentation = $application.Presentations.Open($copy, 0, 0, 0)
    if ($presentation.FullName -ne $copy) { throw 'Disposable presentation ownership check failed' }
    if ($presentation.Designs.Count -ne 2 -or $presentation.Slides.Count -ne 18) { throw 'Unexpected authoring fixture structure' }
    $slide = $presentation.Slides.Item(1)
    $layout = $slide.CustomLayout
    if ($layout.Name -ne 'Title and content') { throw 'Slide did not select its native layout' }
    if ($presentation.Slides.Item(18).CustomLayout.Name -ne 'Alternate content') { throw 'Second master layout assignment was lost' }
    $title = $slide.Shapes.Item('title')
    $originalText = $title.TextFrame.TextRange.Text
    $beforeLeft = [double]$title.Left
    $beforeSize = [double]$title.TextFrame.TextRange.Font.Size
    $slide.Export((Join-Path $directory 'before.png'), 'PNG', 1280, 720)
    $layout.Shapes.Item('title').Left = $beforeLeft + 18
    $afterLeft = [double]$slide.Shapes.Item('title').Left
    if ([math]::Abs($afterLeft - $beforeLeft - 18) -gt 0.01) { throw 'Native placeholder position did not inherit a layout edit' }
    $layout.Shapes.Item('title').TextFrame.TextRange.Font.Size = $beforeSize + 3
    $afterSize = [double]$slide.Shapes.Item('title').TextFrame.TextRange.Font.Size
    if ([math]::Abs($afterSize - $beforeSize - 3) -gt 0.01) { throw 'Native placeholder font did not inherit a layout edit' }
    if ($slide.Shapes.Item('title').TextFrame.TextRange.Text -ne $originalText) { throw 'Layout edit replaced the slide content' }
    $master = $slide.Master
    $master.Shapes.Item('master-footer').TextFrame.TextRange.Text = 'Native master updated without editing slide content'
    $slide.Export((Join-Path $directory 'after.png'), 'PNG', 1280, 720)
    $shape = $presentation.Slides.Item(2).Shapes.Item('shape-rect')
    $beforeColor = [int]$shape.Line.ForeColor.RGB
    $master.Theme.ThemeColorScheme.Colors(5).RGB = 0x447755
    $afterColor = [int]$shape.Line.ForeColor.RGB
    if ($afterColor -ne 0x447755 -or $beforeColor -eq $afterColor) { throw 'Native shape outline did not follow its theme color' }
    $presentation.Slides.Item(2).Export((Join-Path $directory 'theme-after.png'), 'PNG', 1280, 720)
    [pscustomobject]@{
        Masters = $presentation.Designs.Count
        MainLayouts = $presentation.Designs.Item(1).SlideMaster.CustomLayouts.Count
        AlternateLayouts = $presentation.Designs.Item(2).SlideMaster.CustomLayouts.Count
        PlaceholderPositionDeltaPoints = $afterLeft - $beforeLeft
        PlaceholderFontDeltaPoints = $afterSize - $beforeSize
        ContentPreserved = $true
        ThemeColorInherited = $true
        ArtifactDirectory = $directory
        SourceSha256 = $sourceHash
    } | ConvertTo-Json
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
    if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $sourceHash) { throw 'Source presentation changed during authoring verification' }
}