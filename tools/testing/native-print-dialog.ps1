param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [Parameter(Mandatory = $true)][string]$Executable,
    [Parameter(Mandatory = $true)][guid]$Owner,
    [Parameter(Mandatory = $true)][ValidateSet('probe', 'cancel')][string]$Action,
    [long]$ExpectedMainHandle = 0
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$clock = [System.Diagnostics.Stopwatch]::StartNew()
function Write-Stage([string]$Name) {
    [Console]::Error.WriteLine("AISLIDE_PRINT_STAGE $Name $($clock.ElapsedMilliseconds)")
}
Write-Stage 'win32_begin'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class AISlidePrintWindow {
    public delegate bool Callback(IntPtr handle, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(Callback callback, IntPtr data);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr handle, out uint processId);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr handle, uint command);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr handle);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr handle, StringBuilder value, int length);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr handle, StringBuilder value, int length);
}
'@
Write-Stage 'win32_ready'
$process = Get-Process -Id $ProcessId
Write-Stage 'process_ready'
$metadata = Get-CimInstance Win32_Process -Filter "ProcessId = $ProcessId"
Write-Stage 'cim_ready'
if ($process.Path -ne $Executable -or $metadata.CommandLine -notlike "*--aislide-owned-test=$Owner*") { throw 'Owned native process identity mismatch' }
$mainHandle = $process.MainWindowHandle
if ($Action -eq 'cancel') {
    if ($ExpectedMainHandle -eq 0) { throw 'Cancel requires the preflight-owned HWND' }
    $mainHandle = [IntPtr]::new($ExpectedMainHandle)
}
$windowProcess = [uint32]0
[void][AISlidePrintWindow]::GetWindowThreadProcessId($mainHandle, [ref]$windowProcess)
if ($windowProcess -ne $ProcessId) { throw 'Owned HWND no longer belongs to the test process' }
if ($mainHandle -eq [IntPtr]::Zero -or -not [AISlidePrintWindow]::IsWindowVisible($mainHandle)) { throw 'Owned native main window is not visible' }
Write-Stage 'hwnd_verified'
if ($Action -eq 'probe') {
    Write-Stage 'probe_complete'
    @{ phase = 'armed'; processId = $ProcessId; mainHandle = $mainHandle.ToInt64(); path = $process.Path; cancelOnly = $true } | ConvertTo-Json -Compress
    exit 0
}
Write-Stage 'uia_begin'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$mainElement = [System.Windows.Automation.AutomationElement]::FromHandle($mainHandle)
if ($mainElement.Current.ProcessId -ne $ProcessId) { throw 'Owned UIA root mismatch' }
Write-Stage 'uia_ready'
$ownedWindows = [System.Collections.Generic.List[System.IntPtr]]::new()
$ownedWindows.Add($mainHandle)
$ownedProcesses = [System.Collections.Generic.HashSet[uint32]]::new()
[void]$ownedProcesses.Add([uint32]$ProcessId)
$parents = @($ProcessId)
for ($depth = 0; $depth -lt 6 -and $parents.Count -gt 0; $depth++) {
    $children = @()
    foreach ($parentId in $parents) {
        foreach ($child in @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $parentId")) {
            if ($child.Name -eq 'msedgewebview2.exe' -and $child.CreationDate -ge $metadata.CreationDate) {
                [void]$ownedProcesses.Add([uint32]$child.ProcessId)
                $children += $child.ProcessId
            }
        }
    }
    if ($ownedProcesses.Count -gt 64) { throw 'Owned WebView process budget exceeded' }
    $parents = $children
}
Write-Stage 'descendants_ready'
$callback = [AISlidePrintWindow+Callback]{ param($handle, $data)
    if ($handle -eq $mainHandle -or -not [AISlidePrintWindow]::IsWindowVisible($handle)) { return $true }
    $windowProcess = [uint32]0
    [void][AISlidePrintWindow]::GetWindowThreadProcessId($handle, [ref]$windowProcess)
    if ($ownedProcesses.Contains($windowProcess)) { $ownedWindows.Add($handle); return $true }
    $ancestor = $handle
    for ($depth = 0; $depth -lt 16; $depth++) {
        $ancestor = [AISlidePrintWindow]::GetWindow($ancestor, 4)
        if ($ancestor -eq $mainHandle) { $ownedWindows.Add($handle); break }
        if ($ancestor -eq [IntPtr]::Zero) { break }
    }
    return $true
}
[void][AISlidePrintWindow]::EnumWindows($callback, [IntPtr]::Zero)
Write-Stage 'windows_ready'
$candidates = @()
$observed = @()
foreach ($handle in $ownedWindows) {
    $title = [System.Text.StringBuilder]::new(512)
    $className = [System.Text.StringBuilder]::new(256)
    [void][AISlidePrintWindow]::GetWindowText($handle, $title, 512)
    [void][AISlidePrintWindow]::GetClassName($handle, $className, 256)
    $observed += @{ handle = $handle.ToInt64(); title = $title.ToString(); className = $className.ToString() }
    $element = [System.Windows.Automation.AutomationElement]::FromHandle($handle)
    $buttons = $element.FindAll([System.Windows.Automation.TreeScope]::Descendants,
        [System.Windows.Automation.PropertyCondition]::new([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button))
    if ($buttons.Count -gt 100) { throw 'Print dialog control budget exceeded' }
    $observed[-1].buttons = @($buttons | ForEach-Object { $_.Current.Name })
    $cancelButtons = @($buttons | Where-Object { $_.Current.Name -match '^(Cancel|キャンセル)$' -and $_.Current.IsEnabled -and -not $_.Current.IsOffscreen })
    $printButtons = @($buttons | Where-Object { $_.Current.Name -match '^(Print|印刷)$' -and -not $_.Current.IsOffscreen })
    if ($cancelButtons.Count -eq 1 -and $printButtons.Count -ge 1) {
        $candidates += @{ handle = $handle; element = $element; cancel = $cancelButtons[0]; title = $title.ToString(); className = $className.ToString(); printControls = $printButtons.Count }
    }
}
Write-Stage 'controls_ready'
if ($candidates.Count -ne 1) { throw "Expected one owned Print dialog with an enabled Cancel control; owned windows=$($observed | ConvertTo-Json -Compress), matching dialogs=$($candidates.Count)" }
$dialog = $candidates[0]
$cancel = $dialog.cancel
$pattern = $cancel.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
$bounds = $dialog.element.Current.BoundingRectangle
$proof = @{ phase = 'shown'; ownerProcessId = $ProcessId; processId = $dialog.element.Current.ProcessId; mainHandle = $mainHandle.ToInt64(); dialogHandle = $dialog.handle.ToInt64(); title = $dialog.title; className = $dialog.className; width = $bounds.Width; height = $bounds.Height; visible = [AISlidePrintWindow]::IsWindowVisible($dialog.handle); cancel = $cancel.Current.Name; printControls = $dialog.printControls; action = 'Cancel only' }
if (-not $proof.visible -or $bounds.Width -le 0 -or $bounds.Height -le 0) { throw 'Print dialog is not visibly laid out' }
Write-Stage 'cancel_begin'
$pattern.Invoke()
Write-Stage 'cancel_complete'
$proof | ConvertTo-Json -Compress