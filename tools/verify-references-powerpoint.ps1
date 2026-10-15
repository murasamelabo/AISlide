[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [Parameter(Mandatory = $true)]
    [string]$Path,
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$source = (Resolve-Path -LiteralPath $Path).Path
$output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw 'Output directory must not already exist' }
if (@(Get-Process -Name POWERPNT -ErrorAction SilentlyContinue).Count -gt 0) { throw 'Close PowerPoint before this isolated verification; existing sessions are not modified' }
if (-not $PSCmdlet.ShouldProcess($output, 'Verify generated reference fixture and export PowerPoint PDF/PNG')) { return }
[void][System.IO.Directory]::CreateDirectory($output)
$before = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
$application = $null
$presentation = $null
$result = $null
$failure = $null
$cleanupFailures = [System.Collections.Generic.List[string]]::new()
try {
    $application = New-Object -ComObject PowerPoint.Application
    $application.AutomationSecurity = 3
    $presentation = $application.Presentations.Open($source, -1, 0, 0)
    if ($presentation.Slides.Count -ne 2) { throw 'Expected the two-slide reference fixture' }
    $referenceSlide = $presentation.Slides.Item(2)
    $text = @($referenceSlide.Shapes | Where-Object { $_.HasTextFrame -eq -1 } | ForEach-Object { $_.TextFrame.TextRange.Text }) -join "`n"
    $urls = @('https://learn.microsoft.com/azure/', 'https://learn.microsoft.com/azure/architecture/')
    foreach ($url in $urls) {
        if (-not $text.Contains($url)) { throw 'A readable reference URL is missing in PowerPoint' }
        if (-not @($referenceSlide.Hyperlinks | Where-Object { $_.Address -eq $url }).Count) { throw 'A reference hyperlink target is missing in PowerPoint' }
    }
    if ($text.Contains('DO_NOT_PUBLISH')) { throw 'Unapproved reference leaked into the slide' }
    $pdf = Join-Path $output 'references-powerpoint.pdf'
    $presentation.SaveAs($pdf, 32)
    foreach ($index in 1..2) { $presentation.Slides.Item($index).Export((Join-Path $output "slide-$index.png"), 'PNG', 1280, 720) }
    if ((Get-Item -LiteralPath $pdf).Length -le 0) { throw 'PowerPoint PDF was not generated' }
    $result = [pscustomobject]@{ Slides = 2; UrlTextPresent = $urls.Count; Hyperlinks = $referenceSlide.Hyperlinks.Count; Pdf = $pdf; Captures = $output; SourceSha256 = $before; Scope = 'Generated reference fixture only; inspect captures separately for readability, not general Office visual parity' }
} catch {
    $failure = $_
} finally {
    if ($null -ne $presentation) {
        try { $presentation.Saved = -1; $presentation.Close() } catch { $cleanupFailures.Add('Presentation close failed') }
        try { [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($presentation) } catch { $cleanupFailures.Add('Presentation COM release failed') }
    }
    if ($null -ne $application) {
        try { $application.Quit() } catch { $cleanupFailures.Add('PowerPoint quit failed') }
        try { [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($application) } catch { $cleanupFailures.Add('PowerPoint COM release failed') }
    }
    try { if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -ne $before) { $cleanupFailures.Add('Source presentation changed during verification') } }
    catch { $cleanupFailures.Add('Source integrity could not be checked') }
}
if ($null -ne $failure) { throw $failure }
if ($cleanupFailures.Count -gt 0) { throw ($cleanupFailures -join '; ') }
$result | ConvertTo-Json