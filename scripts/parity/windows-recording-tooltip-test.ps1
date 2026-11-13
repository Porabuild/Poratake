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
$safeToDeleteProfile = $true
$overlapWindow = [IntPtr]::Zero

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

    [StructLayout(LayoutKind.Sequential)]
    public struct Point { public int X, Y; }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct MonitorInfo {
        public uint Size;
        public Rect Monitor;
        public Rect Work;
        public uint Flags;
    }

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct ToolInfo {
        public uint Size;
        public uint Flags;
        public IntPtr Owner;
        public UIntPtr Id;
        public Rect Bounds;
        public IntPtr Instance;
        public IntPtr Text;
        public IntPtr Parameter;
        public IntPtr Reserved;
    }

    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc callback, IntPtr parameter);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder name, int capacity);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int capacity);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr hwnd, ref Point point);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll", EntryPoint="WindowFromPoint")] public static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll", EntryPoint="CreateWindowExW", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateWindowEx(uint exStyle, string className, string windowName, uint style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr parameter);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool DestroyWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool GetWindowDisplayAffinity(IntPtr hwnd, out uint affinity);
    [DllImport("user32.dll", EntryPoint="SendMessageTimeoutW", SetLastError = true)] public static extern IntPtr SendMessageTimeout(IntPtr hwnd, uint message, UIntPtr wParam, IntPtr lParam, uint flags, uint timeout, out IntPtr result);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr OpenProcess(uint access, bool inherit, uint processId);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern IntPtr VirtualAllocEx(IntPtr process, IntPtr address, UIntPtr size, uint allocationType, uint protection);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool VirtualFreeEx(IntPtr process, IntPtr address, UIntPtr size, uint freeType);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool WriteProcessMemory(IntPtr process, IntPtr address, IntPtr buffer, UIntPtr size, out UIntPtr written);
    [DllImport("kernel32.dll", SetLastError = true)] public static extern bool ReadProcessMemory(IntPtr process, IntPtr address, IntPtr buffer, UIntPtr size, out UIntPtr read);
    [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll")] public static extern void SetLastError(uint error);
    [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr hwnd, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);

    public static uint MonitorInfoSize() { return (uint)Marshal.SizeOf(typeof(MonitorInfo)); }

    public static IntPtr CreateCoverWindow(int x, int y) {
        var hwnd = CreateWindowEx(0x08000008, "BUTTON", "", 0x90000000, x, y, 4, 4, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
        if (hwnd == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "Could not create tooltip overlap window.");
        if (!SetWindowPos(hwnd, new IntPtr(-1), x, y, 4, 4, 0x0050) || !IsWindowVisible(hwnd)) {
            var error = Marshal.GetLastWin32Error();
            DestroyWindow(hwnd);
            throw new System.ComponentModel.Win32Exception(error, "Could not show tooltip overlap window.");
        }
        return hwnd;
    }

    public static bool IsPointInsideClient(IntPtr hwnd, int x, int y) {
        var point = new Point { X = x, Y = y };
        Rect rect;
        return ScreenToClient(hwnd, ref point) && GetClientRect(hwnd, out rect) &&
            point.X >= rect.Left && point.X < rect.Right && point.Y >= rect.Top && point.Y < rect.Bottom;
    }

    public static void DestroyOwnedWindow(IntPtr hwnd, uint processId) {
        uint ownerProcessId;
        GetWindowThreadProcessId(hwnd, out ownerProcessId);
        if (ownerProcessId != processId) throw new InvalidOperationException("Refusing to destroy a window outside the test process.");
        if (!DestroyWindow(hwnd)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "Could not destroy tooltip overlap window.");
    }

    public static void FreeRemote(IntPtr process, IntPtr address) {
        if (address != IntPtr.Zero && !VirtualFreeEx(process, address, UIntPtr.Zero, 0x8000)) {
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "VirtualFreeEx failed.");
        }
    }

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

    public static string TooltipText(IntPtr tooltip, IntPtr owner, uint processId) {
        if (IntPtr.Size != 8) throw new InvalidOperationException("The hosted tooltip probe requires a 64-bit process.");
        var process = OpenProcess(0x0008 | 0x0010 | 0x0020, false, processId);
        if (process == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "OpenProcess failed.");
        var localInfo = Marshal.AllocHGlobal(Marshal.SizeOf(typeof(ToolInfo)));
        var localText = Marshal.AllocHGlobal(2048);
        var remoteInfo = VirtualAllocEx(process, IntPtr.Zero, new UIntPtr((uint)Marshal.SizeOf(typeof(ToolInfo))), 0x3000, 0x04);
        var remoteText = VirtualAllocEx(process, IntPtr.Zero, new UIntPtr(2048), 0x3000, 0x04);
        var canFreeRemote = true;
        try {
            if (remoteInfo == IntPtr.Zero || remoteText == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "VirtualAllocEx failed.");
            var info = new ToolInfo {
                Size = (uint)Marshal.SizeOf(typeof(ToolInfo)),
                Flags = 0x00A0,
                Owner = owner,
                Id = new UIntPtr(1),
                Text = remoteText
            };
            Marshal.StructureToPtr(info, localInfo, false);
            UIntPtr transferred;
            if (!WriteProcessMemory(process, remoteInfo, localInfo, new UIntPtr((uint)Marshal.SizeOf(typeof(ToolInfo))), out transferred) || transferred.ToUInt64() != (ulong)Marshal.SizeOf(typeof(ToolInfo))) {
                throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "WriteProcessMemory for TOOLINFO failed.");
            }
            IntPtr result;
            SetLastError(0);
            var sent = SendMessageTimeout(tooltip, 1080, new UIntPtr(1024), remoteInfo, 0x0001 | 0x0002, 1000, out result);
            if (sent == IntPtr.Zero) {
                canFreeRemote = false;
                var error = Marshal.GetLastWin32Error();
                throw new System.ComponentModel.Win32Exception(error == 0 ? 31 : error, "SendMessageTimeoutW for TTM_GETTEXTW failed or timed out.");
            }
            if (!ReadProcessMemory(process, remoteText, localText, new UIntPtr(2048), out transferred) || transferred.ToUInt64() != 2048) {
                throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "ReadProcessMemory for tooltip text failed.");
            }
            return Marshal.PtrToStringUni(localText) ?? String.Empty;
        } finally {
            Marshal.FreeHGlobal(localInfo);
            Marshal.FreeHGlobal(localText);
            try {
                if (canFreeRemote) FreeRemote(process, remoteInfo);
            } finally {
                try {
                    if (canFreeRemote) FreeRemote(process, remoteText);
                } finally {
                    if (!CloseHandle(process)) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "CloseHandle failed.");
                }
            }
        }
    }

}
'@

function Get-VisibleTooltipWindow([int]$processId, [IntPtr]$owner) {
    foreach ($handle in [PoratakeTooltipProbe]::Windows([uint32]$processId)) {
        if ([PoratakeTooltipProbe]::ClassName($handle) -ne 'tooltips_class32') { continue }
        if (-not [PoratakeTooltipProbe]::IsWindowVisible($handle)) { continue }
        if ([PoratakeTooltipProbe]::GetWindow($handle, 4) -ne $owner) { continue }
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
    $hoverX = $null
    for ($x = $bounds.Left + 4; $x -lt $bounds.Right - 4 -and -not $visibleTooltip; $x += 8) {
        if (-not [PoratakeTooltipProbe]::SetCursorPos($x, $hoverY)) { throw "Could not move the runner pointer to $x,$hoverY" }
        $tooltip = Wait-Until {
            $tooltipHandle = Get-VisibleTooltipWindow $application.Id $owner
            if (-not $tooltipHandle) { return $null }
            $tooltipText = [PoratakeTooltipProbe]::TooltipText($tooltipHandle, $owner, [uint32]$application.Id)
            if (-not $tooltipText) { return $null }
            return @{ Handle = $tooltipHandle; Text = $tooltipText }
        } 350 ''
        if ($tooltip) {
            $visibleTooltip = $tooltip
            $hoverX = $x
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
    $exitPoint = New-Object PoratakeTooltipProbe+Point
    if (-not [PoratakeTooltipProbe]::GetCursorPos([ref]$exitPoint)) { throw 'Could not read the runner pointer after pointer exit.' }
    if ($exitPoint.X -ge $bounds.Left -and $exitPoint.X -lt $bounds.Right -and $exitPoint.Y -ge $bounds.Top -and $exitPoint.Y -lt $bounds.Bottom) {
        throw "Runner pointer did not leave the recording controls: $($exitPoint.X),$($exitPoint.Y)"
    }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($visibleTooltip.Handle) } 2000 'The native recording tooltip stayed visible after pointer exit.' | Out-Null
    Write-Host 'Native GPUI tooltip hid after pointer exit.'

    if (-not [PoratakeTooltipProbe]::SetCursorPos($hoverX, $hoverY)) { throw "Could not move the runner pointer back to the tooltip trigger at $hoverX,$hoverY" }
    $reenteredTooltip = Wait-Until {
        $tooltipHandle = Get-VisibleTooltipWindow $application.Id $owner
        if (-not $tooltipHandle) { return $null }
        $tooltipText = [PoratakeTooltipProbe]::TooltipText($tooltipHandle, $owner, [uint32]$application.Id)
        if ($tooltipText -ne $visibleTooltip.Text) { return $null }
        return @{ Handle = $tooltipHandle; Text = $tooltipText }
    } 2000 'The native GPUI tooltip did not return on pointer re-entry.'
    if (-not [PoratakeTooltipProbe]::SetCursorPos($bounds.Left - 20, $bounds.Top - 20)) { throw 'Could not move the runner pointer away after tooltip re-entry.' }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($reenteredTooltip.Handle) } 2000 'The native GPUI tooltip stayed visible after its second pointer exit.' | Out-Null
    Write-Host "Native GPUI tooltip '$($reenteredTooltip.Text)' hid and reappeared on exit/re-entry."

    if (-not [PoratakeTooltipProbe]::SetCursorPos($hoverX, $hoverY)) { throw 'Could not move the runner pointer back to the tooltip trigger before overlap testing.' }
    $beforeOverlap = Wait-Until {
        $tooltipHandle = Get-VisibleTooltipWindow $application.Id $owner
        if (-not $tooltipHandle) { return $null }
        $tooltipText = [PoratakeTooltipProbe]::TooltipText($tooltipHandle, $owner, [uint32]$application.Id)
        if ($tooltipText -ne $visibleTooltip.Text) { return $null }
        return @{ Handle = $tooltipHandle; Text = $tooltipText }
    } 2000 'The native GPUI tooltip did not return before overlap testing.'
    $coverX = $hoverX + 1
    if (-not [PoratakeTooltipProbe]::IsPointInsideClient($owner, $coverX, $hoverY)) { throw 'The overlap test point is outside the recording controls client area.' }
    $overlapWindow = [PoratakeTooltipProbe]::CreateCoverWindow($coverX, $hoverY)
    if (-not [PoratakeTooltipProbe]::SetCursorPos($coverX, $hoverY)) { throw 'Could not move the runner pointer onto the owned overlap window.' }
    $coveredPoint = New-Object PoratakeTooltipProbe+Point
    $coveredPoint.X = $coverX
    $coveredPoint.Y = $hoverY
    if ([PoratakeTooltipProbe]::WindowFromPoint($coveredPoint) -ne $overlapWindow) { throw 'The overlap window did not receive native hit testing at the covered point.' }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($beforeOverlap.Handle) } 2000 'The native GPUI tooltip stayed visible when another HWND covered its trigger.' | Out-Null
    [PoratakeTooltipProbe]::DestroyOwnedWindow($overlapWindow, [uint32]$PID)
    $overlapWindow = [IntPtr]::Zero
    Write-Host 'Native GPUI tooltip hid when a test-owned HWND covered its trigger.'

    if (-not [PoratakeTooltipProbe]::SetCursorPos($bounds.Left - 20, $bounds.Top - 20)) { throw 'Could not move the runner pointer away after overlap testing.' }
    if (-not [PoratakeTooltipProbe]::SetCursorPos($hoverX, $hoverY)) { throw 'Could not move the runner pointer back after removing the overlap window.' }
    $afterOverlap = Wait-Until {
        $tooltipHandle = Get-VisibleTooltipWindow $application.Id $owner
        if (-not $tooltipHandle) { return $null }
        $tooltipText = [PoratakeTooltipProbe]::TooltipText($tooltipHandle, $owner, [uint32]$application.Id)
        if ($tooltipText -ne $visibleTooltip.Text) { return $null }
        return @{ Handle = $tooltipHandle; Text = $tooltipText }
    } 2000 'The native GPUI tooltip did not return after removing the overlap window.'
    if (-not [PoratakeTooltipProbe]::SetCursorPos($bounds.Left - 20, $bounds.Top - 20)) { throw 'Could not move the runner pointer away after overlap re-entry.' }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($afterOverlap.Handle) } 2000 'The native GPUI tooltip stayed visible after overlap re-entry.' | Out-Null
    Write-Host 'Native GPUI tooltip returned after the overlap window closed.'
} catch {
    if (Test-Path -LiteralPath $stdoutPath) { Get-Content -LiteralPath $stdoutPath | Write-Host }
    if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath | Write-Host }
    throw
} finally {
    if ($overlapWindow -ne [IntPtr]::Zero) {
        try { [PoratakeTooltipProbe]::DestroyOwnedWindow($overlapWindow, [uint32]$PID) } catch { $safeToDeleteProfile = $false }
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
