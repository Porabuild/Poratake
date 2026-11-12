param([long]$TooltipHwnd = 0)

if ($TooltipHwnd -ne 0) {
    $ErrorActionPreference = 'Stop'
    Add-Type -AssemblyName UIAutomationClient -ErrorAction Stop
    Add-Type -AssemblyName UIAutomationTypes -ErrorAction Stop
    $accessible = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$TooltipHwnd)
    if (-not $accessible -or $accessible.Current.ControlType -ne [System.Windows.Automation.ControlType]::ToolTip) { exit 1 }
    [Console]::Out.Write($accessible.Current.Name)
    exit 0
}

$ErrorActionPreference = 'Stop'

if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'This desktop interaction test runs only on an isolated GitHub Actions Windows runner.'
}

$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$profileRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'poratake-tooltip-' + [guid]::NewGuid().ToString('N')))
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $profileRoot.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing profile outside the runner temporary directory: $profileRoot"
}

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'src/main/target' }
$executable = Join-Path $targetDir 'debug/poratake-gpui.exe'
$daemonCandidates = @()
$daemonExecutable = $null
$stdoutPath = Join-Path $profileRoot 'stdout.log'
$stderrPath = Join-Path $profileRoot 'stderr.log'
$application = $null
$previousProfile = $env:PORATAKE_CONFIG_DIR
$previousDaemonPath = $env:PORATAKE_DAEMON_PATH
$ownedDaemons = [System.Collections.Generic.List[object]]::new()
$ownedHelpers = [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
$safeToDeleteProfile = $true

function Capture-OwnedDaemonChildren {
    if (-not $script:application) { return }
    try {
        $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($script:application.Id)" -ErrorAction Stop)
    } catch {
        $script:safeToDeleteProfile = $false
        return
    }
    foreach ($child in $children) {
        if ($child.Name -ne 'poratake-daemon.exe') { continue }
        if ($script:ownedDaemons.ProcessId -contains [int]$child.ProcessId) { continue }
        try {
            $process = Get-Process -Id $child.ProcessId -ErrorAction Stop
            $imagePath = [System.IO.Path]::GetFullPath($process.MainModule.FileName)
            $created = [datetime]$child.CreationDate
            if ($imagePath -notin $script:daemonCandidates -or [Math]::Abs(($process.StartTime - $created).TotalSeconds) -gt 1) {
                $script:safeToDeleteProfile = $false
                continue
            }
            $script:ownedDaemons.Add(@{
                Process = $process
                ProcessId = [int]$child.ProcessId
                StartTime = $process.StartTime
                ImagePath = $imagePath
                ParentProcessId = [int]$child.ParentProcessId
            })
        } catch {
            $script:safeToDeleteProfile = $false
        }
    }
}

Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class PoratakeTooltipProbe {
    public delegate bool EnumWindowsProc(IntPtr hwnd, IntPtr parameter);

    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct MonitorInfo {
        public uint Size;
        public Rect Monitor;
        public Rect Work;
        public uint Flags;
    }

    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder name, int capacity);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int capacity);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetWindowDisplayAffinity(IntPtr hwnd, out uint affinity);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);

    public static uint MonitorInfoSize() { return (uint)Marshal.SizeOf(typeof(MonitorInfo)); }

    public static List<IntPtr> Windows(uint processId) {
        var result = new List<IntPtr>();
        EnumWindows((hwnd, _) => {
            uint owner;
            GetWindowThreadProcessId(hwnd, out owner);
            if (owner == processId) result.Add(hwnd);
            return true;
        }, IntPtr.Zero);
        return result;
    }

    public static string ClassName(IntPtr hwnd) {
        var text = new StringBuilder(256);
        GetClassName(hwnd, text, text.Capacity);
        return text.ToString();
    }

    public static string WindowText(IntPtr hwnd) {
        var text = new StringBuilder(512);
        GetWindowText(hwnd, text, text.Capacity);
        return text.ToString();
    }

}
'@

function Read-TooltipNameBounded([IntPtr]$handle) {
    $outputPath = Join-Path $profileRoot ('uia-' + [guid]::NewGuid().ToString('N') + '.out')
    $errorPath = Join-Path $profileRoot ('uia-' + [guid]::NewGuid().ToString('N') + '.err')
    $helper = $null
    try {
        $arguments = "-NoLogo -NoProfile -NonInteractive -File `"$PSCommandPath`" -TooltipHwnd $($handle.ToInt64())"
        $helper = Start-Process -FilePath (Join-Path $PSHOME 'pwsh.exe') -ArgumentList $arguments -RedirectStandardOutput $outputPath -RedirectStandardError $errorPath -WindowStyle Hidden -PassThru
        $script:ownedHelpers.Add($helper)
        if (-not $helper.WaitForExit(2500)) {
            $helper.Kill()
            if (-not $helper.WaitForExit(5000)) { $script:safeToDeleteProfile = $false }
            throw 'The Windows UI Automation tooltip query exceeded 2.5 seconds.'
        }
        if ($helper.ExitCode -ne 0) {
            $details = if (Test-Path -LiteralPath $errorPath) { Get-Content -LiteralPath $errorPath -Raw } else { '' }
            throw "The Windows UI Automation tooltip query failed: $details"
        }
        return (Get-Content -LiteralPath $outputPath -Raw).Trim()
    } finally {
        if ($helper) {
            if (-not $helper.HasExited) {
                try {
                    $helper.Kill()
                    if (-not $helper.WaitForExit(5000)) { $script:safeToDeleteProfile = $false }
                } catch {
                    if (-not $helper.HasExited) { $script:safeToDeleteProfile = $false }
                }
            }
            if (-not $helper.HasExited) { $script:safeToDeleteProfile = $false }
        }
    }
}

function Get-VisibleTooltipWindow([int]$processId) {
    foreach ($handle in [PoratakeTooltipProbe]::Windows([uint32]$processId)) {
        if ([PoratakeTooltipProbe]::ClassName($handle) -ne 'tooltips_class32') { continue }
        if (-not [PoratakeTooltipProbe]::IsWindowVisible($handle)) { continue }
        return $handle
    }
    return $null
}

function Wait-Until([scriptblock]$condition, [int]$timeoutMilliseconds, [string]$failure) {
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    while ($timer.ElapsedMilliseconds -lt $timeoutMilliseconds) {
        $result = & $condition
        if ($result) { return $result }
        [System.Threading.Thread]::Sleep(20)
    }
    if ($failure) { throw $failure }
    return $null
}

try {
    New-Item -ItemType Directory -Path $profileRoot | Out-Null
    [System.IO.File]::WriteAllText((Join-Path $profileRoot 'config.json'), '{"onboarding":{"completed":true},"recording":{"micEnabled":false,"camera":{"enabled":false}}}', [System.Text.UTF8Encoding]::new($false))
    Push-Location (Join-Path $root 'src/main')
    try {
        & cargo build --locked --manifest-path (Join-Path $root 'src/main/Cargo.toml') --package poratake-gpui --package poratake-daemon
        if ($LASTEXITCODE -ne 0) { throw "GPUI and daemon build failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
    if (-not (Test-Path -LiteralPath $executable)) { throw "GPUI executable is missing: $executable" }
    $daemonCandidates = @(@(
        (Join-Path $targetDir 'debug/poratake-daemon.exe'),
        (Join-Path $targetDir 'debug/daemon/poratake-daemon.exe'),
        (Join-Path $root 'src/main/daemon/poratake-daemon.exe')
    ) | ForEach-Object { [System.IO.Path]::GetFullPath($_) } | Where-Object { Test-Path -LiteralPath $_ })
    if (-not $daemonCandidates) { throw 'Cargo did not produce a repository-owned Windows daemon binary.' }
    $daemonExecutable = $daemonCandidates[0]

    $env:PORATAKE_CONFIG_DIR = $profileRoot
    $env:PORATAKE_DAEMON_PATH = $daemonExecutable
    $application = Start-Process -FilePath $executable -ArgumentList @('--preview-window', 'recording-control') -WorkingDirectory $root -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath -WindowStyle Hidden -PassThru
    $control = Wait-Until {
        foreach ($handle in [PoratakeTooltipProbe]::Windows($application.Id)) {
            if ([PoratakeTooltipProbe]::WindowText($handle) -eq 'Poratake recording controls' -and [PoratakeTooltipProbe]::IsWindowVisible($handle)) {
                $rect = New-Object PoratakeTooltipProbe+Rect
                if ([PoratakeTooltipProbe]::GetWindowRect($handle, [ref]$rect)) { return @{ Handle = $handle; Bounds = $rect } }
            }
        }
        return $null
    } 30000 'The recording controls window did not appear.'
    Capture-OwnedDaemonChildren

    $owner = $control.Handle
    $bounds = $control.Bounds
    $hoverY = [int](($bounds.Top + $bounds.Bottom) / 2)
    $visibleTooltip = $null
    for ($x = $bounds.Left + 4; $x -lt $bounds.Right - 4 -and -not $visibleTooltip; $x += 8) {
        if (-not [PoratakeTooltipProbe]::SetCursorPos($x, $hoverY)) { throw "Could not move the runner pointer to $x,$hoverY" }
        $tooltipHandle = Wait-Until { Get-VisibleTooltipWindow $application.Id } 350 ''
        if ($tooltipHandle) {
            $tooltipText = Read-TooltipNameBounded $tooltipHandle
            if ($tooltipText) { $visibleTooltip = @{ Handle = $tooltipHandle; Text = $tooltipText } }
        }
    }
    if (-not $visibleTooltip) { throw 'No tooltip appeared while scanning the recording toolbar.' }
    $nativeOwner = [PoratakeTooltipProbe]::GetWindow($visibleTooltip.Handle, 4)
    if ($nativeOwner -ne $owner) { throw "Tooltip owner HWND $nativeOwner does not match the recording-control HWND $owner." }

    $expectedText = @('Start recording', 'Select microphone', 'Turn system sounds off', 'Turn system sounds on', 'Select camera', 'Close')
    if ($visibleTooltip.Text -notin $expectedText) { throw "Unexpected recording tooltip text: '$($visibleTooltip.Text)'" }
    $tooltipBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($visibleTooltip.Handle, [ref]$tooltipBounds)) { throw 'Could not read the native tooltip bounds.' }
    $tooltipWidth = $tooltipBounds.Right - $tooltipBounds.Left
    $tooltipHeight = $tooltipBounds.Bottom - $tooltipBounds.Top
    if ($tooltipWidth -le 0 -or $tooltipHeight -le 0) { throw "Native tooltip has invalid dimensions: ${tooltipWidth}x${tooltipHeight}" }
    $extendsOutsideOwner = $tooltipBounds.Left -lt $bounds.Left -or $tooltipBounds.Top -lt $bounds.Top -or $tooltipBounds.Right -gt $bounds.Right -or $tooltipBounds.Bottom -gt $bounds.Bottom
    if (-not $extendsOutsideOwner) { throw "Native tooltip did not extend beyond its owner window: tooltip=$($tooltipBounds.Left),$($tooltipBounds.Top)-$($tooltipBounds.Right),$($tooltipBounds.Bottom); owner=$($bounds.Left),$($bounds.Top)-$($bounds.Right),$($bounds.Bottom)" }
    $monitor = New-Object PoratakeTooltipProbe+MonitorInfo
    $monitor.Size = [PoratakeTooltipProbe]::MonitorInfoSize()
    $monitorHandle = [PoratakeTooltipProbe]::MonitorFromWindow($visibleTooltip.Handle, 2)
    if ($monitorHandle -eq [IntPtr]::Zero -or -not [PoratakeTooltipProbe]::GetMonitorInfo($monitorHandle, [ref]$monitor)) { throw 'Could not read the tooltip monitor bounds.' }
    if ($tooltipBounds.Left -lt $monitor.Monitor.Left -or $tooltipBounds.Top -lt $monitor.Monitor.Top -or $tooltipBounds.Right -gt $monitor.Monitor.Right -or $tooltipBounds.Bottom -gt $monitor.Monitor.Bottom) {
        throw "Tooltip is outside monitor bounds: tooltip=$($tooltipBounds.Left),$($tooltipBounds.Top)-$($tooltipBounds.Right),$($tooltipBounds.Bottom); monitor=$($monitor.Monitor.Left),$($monitor.Monitor.Top)-$($monitor.Monitor.Right),$($monitor.Monitor.Bottom)"
    }
    $affinity = [uint32]0
    if (-not [PoratakeTooltipProbe]::GetWindowDisplayAffinity($visibleTooltip.Handle, [ref]$affinity) -or $affinity -eq 0) { throw 'The visible tooltip has no capture-affinity protection.' }
    Write-Host "Visible native GPUI tooltip '$($visibleTooltip.Text)' at $($tooltipBounds.Left),$($tooltipBounds.Top), ${tooltipWidth}x${tooltipHeight}, outside owner bounds, affinity=$affinity"

    if (-not [PoratakeTooltipProbe]::SetCursorPos($bounds.Left - 20, $bounds.Top - 20)) { throw 'Could not move the runner pointer away from the recording controls.' }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($visibleTooltip.Handle) } 2000 'The native recording tooltip stayed visible after pointer exit.' | Out-Null
    Write-Host 'Native GPUI tooltip hid after pointer exit.'
} catch {
    if (Test-Path -LiteralPath $stdoutPath) { Get-Content -LiteralPath $stdoutPath | Write-Host }
    if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath | Write-Host }
    throw
} finally {
    foreach ($helper in $ownedHelpers) {
        if (-not $helper.HasExited) {
            try {
                $helper.Kill()
                if (-not $helper.WaitForExit(5000)) { $safeToDeleteProfile = $false }
            } catch {
                if (-not $helper.HasExited) { $safeToDeleteProfile = $false }
            }
        }
        if (-not $helper.HasExited) { $safeToDeleteProfile = $false }
        $helper.Dispose()
    }
    Capture-OwnedDaemonChildren
    if ($application -and -not $application.HasExited) {
        try {
            $application.Kill()
            if (-not $application.WaitForExit(5000)) { $safeToDeleteProfile = $false }
        } catch {
            if (-not $application.HasExited) { $safeToDeleteProfile = $false }
        }
    }
    foreach ($owned in $ownedDaemons) {
        $process = $owned.Process
        if (-not $process.HasExited) {
            try {
                $identity = Get-CimInstance Win32_Process -Filter "ProcessId = $($owned.ProcessId)" -ErrorAction Stop
                $imagePath = [System.IO.Path]::GetFullPath($process.MainModule.FileName)
                $created = [datetime]$identity.CreationDate
                $sameIdentity = $identity.Name -eq 'poratake-daemon.exe' -and [int]$identity.ParentProcessId -eq $owned.ParentProcessId -and $owned.ParentProcessId -eq $application.Id -and $imagePath -ieq $owned.ImagePath -and $imagePath -in $daemonCandidates -and $process.StartTime -eq $owned.StartTime -and [Math]::Abs(($process.StartTime - $created).TotalSeconds) -le 1
                if (-not $sameIdentity) {
                    $safeToDeleteProfile = $false
                    continue
                }
                $process.Kill()
                if (-not $process.WaitForExit(5000)) { $safeToDeleteProfile = $false }
            } catch {
                $safeToDeleteProfile = $false
            }
        }
    }
    if ($null -eq $previousProfile) { Remove-Item Env:PORATAKE_CONFIG_DIR -ErrorAction SilentlyContinue } else { $env:PORATAKE_CONFIG_DIR = $previousProfile }
    if ($null -eq $previousDaemonPath) { Remove-Item Env:PORATAKE_DAEMON_PATH -ErrorAction SilentlyContinue } else { $env:PORATAKE_DAEMON_PATH = $previousDaemonPath }
    $resolvedProfile = [System.IO.Path]::GetFullPath($profileRoot)
    if ($application -and -not $application.HasExited) { $safeToDeleteProfile = $false }
    foreach ($owned in $ownedDaemons) {
        if (-not $owned.Process.HasExited) { $safeToDeleteProfile = $false }
        $owned.Process.Dispose()
    }
    if ($safeToDeleteProfile -and $resolvedProfile.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and [System.IO.Directory]::Exists($resolvedProfile)) {
        Remove-Item -LiteralPath $resolvedProfile -Recurse -Force
    }
    if (-not $safeToDeleteProfile) { throw 'Owned GPUI or daemon process did not exit; retained its temporary profile for safe recovery.' }
}
