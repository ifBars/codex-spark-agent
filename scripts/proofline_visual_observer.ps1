[CmdletBinding()]
param(
    [Parameter(ParameterSetName = 'Direct', Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [int[]]$ProcessId,

    [Parameter(ParameterSetName = 'Control', Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ControlPath,

    [Parameter(ParameterSetName = 'Control', Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ReadyPath,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$ArtifactDirectory,

    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string]$AnchorImagePath,

    [Parameter(ParameterSetName = 'Direct', Mandatory = $true)]
    [ValidateRange(1, [long]::MaxValue)]
    [long]$OriginTimestamp,

    [ValidateRange(1, 120)]
    [int]$TimeoutSeconds = 20,

    [ValidateRange(50, 2000)]
    [int]$FrameIntervalMilliseconds = 150,

    [ValidateRange(0.0, 1.0)]
    [double]$MaximumChangedPixelRatio = 0.02
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if ($env:OS -ne 'Windows_NT') {
    [ordered]@{
        schema = 'spark.proofline.visual-observer.raw.v1'
        eligible = $false
        stable_visible_chrome = $false
        reason = 'windows_required'
        frame_count = 0
        changed_pixel_ratio = $null
        process_id = $null
        window_handle = $null
        frames = @()
        first_stable_visible_ms = $null
        anchor_verified = $false
        anchor_score = $null
    } | ConvertTo-Json -Depth 6 -Compress
    exit 2
}

New-Item -ItemType Directory -Force -Path $ArtifactDirectory | Out-Null

Add-Type -AssemblyName System.Drawing
if (-not ('ProoflineVisualObserver.NativeMethods' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace ProoflineVisualObserver {
    public static class NativeMethods {
        [StructLayout(LayoutKind.Sequential)]
        public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool IsWindowVisible(IntPtr hWnd);

        [DllImport("user32.dll")]
        [return: MarshalAs(UnmanagedType.Bool)]
        public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);

        [StructLayout(LayoutKind.Sequential)]
        public struct POINT { public int X; public int Y; }

        [DllImport("user32.dll")]
        public static extern IntPtr WindowFromPoint(POINT point);

        [DllImport("user32.dll")]
        public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint processId);

        [DllImport("user32.dll")]
        public static extern IntPtr GetAncestor(IntPtr hWnd, uint flags);
    }
}
'@
}

if ($PSCmdlet.ParameterSetName -eq 'Control') {
    $controlAbsolute = [IO.Path]::GetFullPath($ControlPath)
    $readyAbsolute = [IO.Path]::GetFullPath($ReadyPath)
    [IO.File]::WriteAllText($readyAbsolute, 'observer-ready', [Text.UTF8Encoding]::new($false))
    $controlWait = [Diagnostics.Stopwatch]::StartNew()
    while (-not [IO.File]::Exists($controlAbsolute) -and $controlWait.Elapsed.TotalSeconds -lt 15) {
        Start-Sleep -Milliseconds 10
    }
    if (-not [IO.File]::Exists($controlAbsolute)) {
        [ordered]@{
            schema = 'spark.proofline.visual-observer.raw.v1'
            eligible = $false
            stable_visible_chrome = $false
            reason = 'launch_control_unavailable'
            frame_count = 0
            changed_pixel_ratio = $null
            process_id = $null
            window_handle = $null
            frames = @()
            first_stable_visible_ms = $null
            anchor_verified = $false
            anchor_score = $null
        } | ConvertTo-Json -Depth 6 -Compress
        exit 6
    }
    try {
        $control = [IO.File]::ReadAllText($controlAbsolute) | ConvertFrom-Json
        $controlNames = @($control.PSObject.Properties.Name)
        if ($controlNames.Count -ne 2 -or 'process_id' -notin $controlNames -or 'origin_timestamp' -notin $controlNames) { throw 'invalid control keys' }
        $ProcessId = @([int]$control.process_id)
        $OriginTimestamp = [long]$control.origin_timestamp
        if ($ProcessId[0] -le 0 -or $OriginTimestamp -le 0) { throw 'invalid control values' }
    }
    catch {
        [ordered]@{
            schema = 'spark.proofline.visual-observer.raw.v1'
            eligible = $false
            stable_visible_chrome = $false
            reason = 'launch_control_malformed'
            frame_count = 0
            changed_pixel_ratio = $null
            process_id = $null
            window_handle = $null
            frames = @()
            first_stable_visible_ms = $null
            anchor_verified = $false
            anchor_score = $null
        } | ConvertTo-Json -Depth 6 -Compress
        exit 7
    }
}

function Get-CandidateWindow {
    foreach ($id in @($ProcessId | Sort-Object -Unique)) {
        $process = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($null -eq $process -or $process.MainWindowHandle -eq [IntPtr]::Zero) { continue }
        $handle = $process.MainWindowHandle
        if (-not [ProoflineVisualObserver.NativeMethods]::IsWindowVisible($handle)) { continue }
        $rect = New-Object ProoflineVisualObserver.NativeMethods+RECT
        if (-not [ProoflineVisualObserver.NativeMethods]::GetWindowRect($handle, [ref]$rect)) { continue }
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($width -lt 640 -or $height -lt 400) { continue }
        if ($process.MainWindowTitle -notmatch 'Proofline') { continue }
        $point = New-Object ProoflineVisualObserver.NativeMethods+POINT
        $point.X = $rect.Left + [Math]::Floor($width / 2)
        $point.Y = $rect.Top + [Math]::Floor($height / 2)
        $topHandle = [ProoflineVisualObserver.NativeMethods]::WindowFromPoint($point)
        $topRootHandle = [ProoflineVisualObserver.NativeMethods]::GetAncestor($topHandle, 2)
        return [pscustomobject]@{
            ProcessId = $id
            Handle = $handle
            Left = $rect.Left
            Top = $rect.Top
            Width = $width
            Height = $height
            Unoccluded = $topRootHandle -eq $handle
        }
    }
    return $null
}

function Save-WindowFrame([object]$Window, [string]$Path) {
    $bitmap = [System.Drawing.Bitmap]::new($Window.Width, $Window.Height)
    try {
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($Window.Left, $Window.Top, 0, 0, $bitmap.Size)
        }
        finally {
            $graphics.Dispose()
        }
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    }
    finally {
        $bitmap.Dispose()
    }
}

function Get-ChangedPixelRatio([string]$FirstPath, [string]$SecondPath) {
    $first = [System.Drawing.Bitmap]::FromFile($FirstPath)
    $second = [System.Drawing.Bitmap]::FromFile($SecondPath)
    try {
        if ($first.Width -ne $second.Width -or $first.Height -ne $second.Height) { return 1.0 }
        $step = [Math]::Max(1, [Math]::Floor([Math]::Min($first.Width, $first.Height) / 120))
        $sampled = 0
        $changed = 0
        for ($y = 0; $y -lt $first.Height; $y += $step) {
            for ($x = 0; $x -lt $first.Width; $x += $step) {
                $a = $first.GetPixel($x, $y)
                $b = $second.GetPixel($x, $y)
                $delta = [Math]::Abs($a.R - $b.R) + [Math]::Abs($a.G - $b.G) + [Math]::Abs($a.B - $b.B)
                $sampled++
                if ($delta -gt 24) { $changed++ }
            }
        }
        if ($sampled -eq 0) { return 1.0 }
        return [double]$changed / [double]$sampled
    }
    finally {
        $first.Dispose()
        $second.Dispose()
    }
}

function Get-FrameContentSignal([string]$Path) {
    $bitmap = [System.Drawing.Bitmap]::FromFile($Path)
    try {
        $step = [Math]::Max(1, [Math]::Floor([Math]::Min($bitmap.Width, $bitmap.Height) / 120))
        $minimum = 255.0
        $maximum = 0.0
        $colors = [Collections.Generic.HashSet[int]]::new()
        for ($y = 0; $y -lt $bitmap.Height; $y += $step) {
            for ($x = 0; $x -lt $bitmap.Width; $x += $step) {
                $pixel = $bitmap.GetPixel($x, $y)
                $luminance = (0.2126 * $pixel.R) + (0.7152 * $pixel.G) + (0.0722 * $pixel.B)
                $minimum = [Math]::Min($minimum, $luminance)
                $maximum = [Math]::Max($maximum, $luminance)
                [void]$colors.Add(($pixel.R -shl 16) -bor ($pixel.G -shl 8) -bor $pixel.B)
            }
        }
        [pscustomobject]@{
            luminance_range = $maximum - $minimum
            distinct_sampled_colors = $colors.Count
            determinate = ($maximum - $minimum) -ge 12.0 -and $colors.Count -ge 4
        }
    }
    finally {
        $bitmap.Dispose()
    }
}

function Get-ProoflineAnchorSignal([string]$FramePath, [string]$AnchorPath) {
    if (-not [IO.File]::Exists($AnchorPath)) { return [pscustomobject]@{ verified = $false; score = 0.0 } }
    $frame = [System.Drawing.Bitmap]::FromFile($FramePath)
    $anchor = [System.Drawing.Bitmap]::FromFile($AnchorPath)
    try {
        $best = 0.0
        foreach ($size in @(36, 42, 48, 53, 58, 63)) {
            $scale = [double]$size / 42.0
            $expectedX = [int][Math]::Round(26 * $scale)
            $expectedY = [int][Math]::Round(63 * $scale)
            for ($left = [Math]::Max(0, $expectedX - 8); $left -le [Math]::Min($frame.Width - $size, $expectedX + 8); $left += 2) {
                for ($top = [Math]::Max(0, $expectedY - 8); $top -le [Math]::Min($frame.Height - $size, $expectedY + 8); $top += 2) {
                    $positive = 0; $positiveMatches = 0; $negative = 0; $negativeMatches = 0
                    for ($y = 0; $y -lt $size; $y += 3) {
                        for ($x = 0; $x -lt $size; $x += 3) {
                            $sourceX = [Math]::Min($anchor.Width - 1, [int](($x + 0.5) * $anchor.Width / $size))
                            $sourceY = [Math]::Min($anchor.Height - 1, [int](($y + 0.5) * $anchor.Height / $size))
                            $mask = $anchor.GetPixel($sourceX, $sourceY)
                            $pixel = $frame.GetPixel($left + $x, $top + $y)
                            $orange = $pixel.R -ge 210 -and $pixel.G -ge 55 -and $pixel.G -le 170 -and $pixel.B -le 90
                            if ($mask.A -ge 96) { $positive++; if ($orange) { $positiveMatches++ } }
                            elseif ($mask.A -le 8) { $negative++; if (-not $orange) { $negativeMatches++ } }
                        }
                    }
                    if ($positive -gt 0 -and $negative -gt 0) {
                        $score = (0.8 * $positiveMatches / $positive) + (0.2 * $negativeMatches / $negative)
                        $best = [Math]::Max($best, $score)
                    }
                }
            }
        }
        return [pscustomobject]@{ verified = $best -ge 0.78; score = $best }
    }
    finally {
        $frame.Dispose()
        $anchor.Dispose()
    }
}

$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
$window = $null
while ([DateTime]::UtcNow -lt $deadline -and $null -eq $window) {
    $window = Get-CandidateWindow
    if ($null -eq $window) { Start-Sleep -Milliseconds 100 }
}

if ($null -eq $window) {
    [ordered]@{
        schema = 'spark.proofline.visual-observer.raw.v1'
        eligible = $false
        stable_visible_chrome = $false
        reason = 'proofline_window_unavailable'
        frame_count = 0
        changed_pixel_ratio = $null
        process_id = $null
        window_handle = $null
        frames = @()
        first_stable_visible_ms = $null
        anchor_verified = $false
        anchor_score = $null
    } | ConvertTo-Json -Depth 6 -Compress
    exit 3
}

if (-not $window.Unoccluded) {
    [ordered]@{
        schema = 'spark.proofline.visual-observer.raw.v1'
        eligible = $false
        stable_visible_chrome = $false
        reason = 'blank_or_indeterminate'
        frame_count = 0
        changed_pixel_ratio = $null
        process_id = $window.ProcessId
        window_handle = $window.Handle.ToInt64()
        frames = @()
        first_stable_visible_ms = $null
        anchor_verified = $false
        anchor_score = $null
    } | ConvertTo-Json -Depth 6 -Compress
    exit 5
}

$firstPath = Join-Path $ArtifactDirectory 'visual-frame-1.png'
$secondPath = Join-Path $ArtifactDirectory 'visual-frame-2.png'
Save-WindowFrame -Window $window -Path $firstPath
Start-Sleep -Milliseconds $FrameIntervalMilliseconds
$secondWindow = Get-CandidateWindow
if ($null -eq $secondWindow -or -not $secondWindow.Unoccluded -or $secondWindow.Handle -ne $window.Handle -or $secondWindow.Width -ne $window.Width -or $secondWindow.Height -ne $window.Height) {
    [ordered]@{
        schema = 'spark.proofline.visual-observer.raw.v1'
        eligible = $false
        stable_visible_chrome = $false
        reason = 'window_changed_between_frames'
        frame_count = 1
        changed_pixel_ratio = $null
        process_id = $window.ProcessId
        window_handle = $window.Handle.ToInt64()
        frames = @($firstPath)
        first_stable_visible_ms = $null
        anchor_verified = $false
        anchor_score = $null
    } | ConvertTo-Json -Depth 6 -Compress
    exit 4
}

Save-WindowFrame -Window $secondWindow -Path $secondPath
$secondFrameTimestamp = [Diagnostics.Stopwatch]::GetTimestamp()
$changedRatio = Get-ChangedPixelRatio -FirstPath $firstPath -SecondPath $secondPath
$firstSignal = Get-FrameContentSignal -Path $firstPath
$secondSignal = Get-FrameContentSignal -Path $secondPath
$firstAnchor = Get-ProoflineAnchorSignal -FramePath $firstPath -AnchorPath $AnchorImagePath
$secondAnchor = Get-ProoflineAnchorSignal -FramePath $secondPath -AnchorPath $AnchorImagePath
$contentDeterminate = $firstSignal.determinate -and $secondSignal.determinate
$anchorVerified = $firstAnchor.verified -and $secondAnchor.verified
$anchorScore = [Math]::Min($firstAnchor.score, $secondAnchor.score)
$stable = $changedRatio -le $MaximumChangedPixelRatio -and $contentDeterminate -and $anchorVerified
$stableVisibleMilliseconds = if ($stable) {
    [long][Math]::Round((($secondFrameTimestamp - $OriginTimestamp) * 1000.0) / [Diagnostics.Stopwatch]::Frequency)
} else { $null }
[ordered]@{
    schema = 'spark.proofline.visual-observer.raw.v1'
    eligible = $stable
    stable_visible_chrome = $stable
    reason = if (-not $anchorVerified) { 'proofline_anchor_unavailable' } elseif (-not $contentDeterminate) { 'blank_or_indeterminate' } elseif ($stable) { $null } else { 'visible_frames_disagreed' }
    frame_count = 2
    changed_pixel_ratio = $changedRatio
    process_id = $window.ProcessId
    window_handle = $window.Handle.ToInt64()
    frames = @($firstPath, $secondPath)
    content_signal = @($firstSignal, $secondSignal)
    first_stable_visible_ms = $stableVisibleMilliseconds
    anchor_verified = $anchorVerified
    anchor_score = $anchorScore
} | ConvertTo-Json -Depth 6 -Compress
if (-not $stable) { exit 5 }
