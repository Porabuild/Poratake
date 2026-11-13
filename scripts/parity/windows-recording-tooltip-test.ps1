$ErrorActionPreference = 'Stop'

if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'This desktop interaction test runs only on an isolated GitHub Actions Windows runner.'
}

$root = if ($env:GITHUB_WORKSPACE) { (Resolve-Path $env:GITHUB_WORKSPACE).Path } else { 'E:\work\poratake\.poracode\worktrees\poracode-nimble-badger-314d2ec2' }
$profileRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'poratake-tooltip-' + [guid]::NewGuid().ToString('N')))
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $profileRoot.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing profile outside the runner temporary directory: $profileRoot"
}
$runnerTempRoot = [System.IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'src/main/target' }
$executable = Join-Path $targetDir 'debug/poratake-gpui.exe'
$daemonCandidates = @()
$daemonExecutable = $null
$stdoutPath = Join-Path $profileRoot 'stdout.log'
$stderrPath = Join-Path $profileRoot 'stderr.log'
$application = $null
$previousProfile = $env:PORATAKE_CONFIG_DIR
$previousDaemonPath = $env:PORATAKE_DAEMON_PATH
$previousTooltipProof = $env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF
$ownedDaemons = [System.Collections.Generic.List[object]]::new()
$safeToDeleteProfile = $true
$overlapWindow = [IntPtr]::Zero
$backdropWindow = [IntPtr]::Zero
$captureDaemon = $null
$captureDaemonIdentity = $null
$captureDaemonReady = $false
$captureRpcTimedOut = $false
$testFailure = $false
$evidenceRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($runnerTempRoot, 'poratake-visual-tooltip-' + [guid]::NewGuid().ToString('N')))
$captureRoot = Join-Path $evidenceRoot 'captures'
if (-not $evidenceRoot.StartsWith($runnerTempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing evidence outside the runner temporary directory: $evidenceRoot"
}
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

Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class PoratakeTooltipFixture {
    [StructLayout(LayoutKind.Sequential)]
    public struct Rect { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll", EntryPoint="CreateWindowExW", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr CreateWindowEx(uint exStyle, string className, string title, uint style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr parameter);
    [DllImport("user32.dll", SetLastError=true)] static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll")] static extern bool UpdateWindow(IntPtr hwnd);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr hwnd);

    public static IntPtr CreateBackdrop(Rect rect, IntPtr owner) {
        uint style = 0x98000006u;
        IntPtr hwnd = CreateWindowEx(0x080000A0, "STATIC", "", style, rect.Left, rect.Top,
            rect.Right - rect.Left, rect.Bottom - rect.Top, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
        if (hwnd == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "Could not create tooltip backdrop.");
        if (!SetWindowPos(hwnd, owner, rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top, 0x0050) ||
            !IsWindowVisible(hwnd) || !UpdateWindow(hwnd)) {
            int error = Marshal.GetLastWin32Error();
            DestroyWindow(hwnd);
            throw new System.ComponentModel.Win32Exception(error, "Could not paint tooltip backdrop.");
        }
        return hwnd;
    }

    [DllImport("user32.dll", SetLastError=true)] static extern bool DestroyWindow(IntPtr hwnd);
}
'@

function Invoke-CaptureDaemonRequest([hashtable]$request) {
    if (-not $script:captureDaemon -or $script:captureDaemon.HasExited -or $script:captureRpcTimedOut) { throw 'Screenshot daemon is unavailable.' }
    $script:captureDaemon.StandardInput.WriteLine(($request | ConvertTo-Json -Compress -Depth 8))
    $script:captureDaemon.StandardInput.Flush()
    $deadline = [DateTime]::UtcNow.AddSeconds(35)
    while ([DateTime]::UtcNow -lt $deadline) {
        $remaining = [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds
        if ($remaining -le 0) { break }
        $line = $script:captureDaemon.StandardOutput.ReadLineAsync()
        if (-not $line.Wait($remaining)) { $script:captureRpcTimedOut = $true; break }
        if ($null -eq $line.Result) { throw 'Screenshot daemon closed stdout.' }
        if ([string]::IsNullOrWhiteSpace($line.Result)) { continue }
        $response = $line.Result | ConvertFrom-Json -ErrorAction Stop
        if ($response.id -ne $request.id) { continue }
        if (-not $response.success) { throw "Screenshot RPC failed: $($response.error.message)" }
        return $response.result
    }
    throw "Timed out waiting for screenshot RPC $($request.id)."
}

function Start-CaptureDaemon {
    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $daemonExecutable
    $startInfo.WorkingDirectory = $root
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $false
    $startInfo.Environment['PORATAKE_TRACE_FILE'] = Join-Path $evidenceRoot 'daemon-trace.log'
    $script:captureDaemon = [System.Diagnostics.Process]::new()
    $script:captureDaemon.StartInfo = $startInfo
    if (-not $script:captureDaemon.Start()) { throw 'Could not start screenshot daemon.' }
    $script:captureDaemonIdentity = @{ ProcessId = $script:captureDaemon.Id; ParentProcessId = $PID; StartTime = $script:captureDaemon.StartTime; ImagePath = [System.IO.Path]::GetFullPath($daemonExecutable) }
    $identity = Get-CimInstance Win32_Process -Filter "ProcessId = $($script:captureDaemon.Id)" -ErrorAction Stop
    $imagePath = [System.IO.Path]::GetFullPath($script:captureDaemon.MainModule.FileName)
    $created = [datetime]$identity.CreationDate
    if ($identity.Name -ne 'poratake-daemon.exe' -or [int]$identity.ParentProcessId -ne $PID -or $imagePath -ine $script:captureDaemonIdentity.ImagePath -or [Math]::Abs(($script:captureDaemon.StartTime - $created).TotalSeconds) -gt 1) {
        throw 'Screenshot daemon identity did not match its owned launch.'
    }
    $script:captureDaemonReady = $true
    $null = Invoke-CaptureDaemonRequest @{ id = 'tooltip-probe-ping'; module = 'system'; method = 'ping' }
}

function Capture-TooltipArea([string]$id, [PoratakeTooltipProbe+Rect]$rect, [string]$path) {
    $result = Invoke-CaptureDaemonRequest @{
        id = $id
        module = 'screenshot'
        method = 'capture-area'
        params = @{ x = $rect.Left; y = $rect.Top; width = $rect.Right - $rect.Left; height = $rect.Bottom - $rect.Top; path = $path; cached = $false }
    }
    if ($result.width -ne ($rect.Right - $rect.Left) -or $result.height -ne ($rect.Bottom - $rect.Top) -or -not [System.IO.File]::Exists($path)) {
        throw "Daemon capture did not match the requested rectangle: $path"
    }
}

function Test-BackdropCapture([string]$basePath, [string]$fixturePath, [PoratakeTooltipProbe+Rect]$tip, [PoratakeTooltipProbe+Rect]$owner) {
    Add-Type -AssemblyName System.Drawing
    $base = [System.Drawing.Bitmap]::new($basePath)
    $fixture = [System.Drawing.Bitmap]::new($fixturePath)
    try {
        $width = $tip.Right - $tip.Left
        $height = $tip.Bottom - $tip.Top
        if ($base.Width -ne $width -or $base.Height -ne $height -or $fixture.Width -ne $width -or $fixture.Height -ne $height) { throw 'Backdrop screenshot dimensions do not match the tooltip rectangle.' }
        $changed = 0
        $checked = 0
        $target = 255
        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $sx = $tip.Left + $x
                $sy = $tip.Top + $y
                if ($sx -ge $owner.Left -and $sx -lt $owner.Right -and $sy -ge $owner.Top -and $sy -lt $owner.Bottom) { continue }
                $before = $base.GetPixel($x, $y)
                $pixel = $fixture.GetPixel($x, $y)
                if ([Math]::Abs($target - $pixel.R) -gt 3 -or [Math]::Abs($target - $pixel.G) -gt 3 -or [Math]::Abs($target - $pixel.B) -gt 3) { return $false }
                if ([Math]::Abs([int]$before.R - $target) -gt 8 -or [Math]::Abs([int]$before.G - $target) -gt 8 -or [Math]::Abs([int]$before.B - $target) -gt 8) { $changed++ }
                $checked++
            }
        }
        return $checked -ge 64 -and $changed -ge [Math]::Max(16, [Math]::Ceiling($checked * 0.01))
    } finally { $base.Dispose(); $fixture.Dispose() }
}

function Assert-TooltipCaptureMatchesFixture([string]$fixturePath, [string]$shownPath, [PoratakeTooltipProbe+Rect]$tip, [PoratakeTooltipProbe+Rect]$owner) {
    Add-Type -AssemblyName System.Drawing
    $fixture = [System.Drawing.Bitmap]::new($fixturePath)
    $shown = [System.Drawing.Bitmap]::new($shownPath)
    try {
        $width = $tip.Right - $tip.Left
        $height = $tip.Bottom - $tip.Top
        if ($fixture.Width -ne $width -or $fixture.Height -ne $height -or $shown.Width -ne $width -or $shown.Height -ne $height) { throw 'Tooltip screenshot dimensions do not match its measured bounds.' }
        $checked = 0
        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $sx = $tip.Left + $x
                $sy = $tip.Top + $y
                if ($sx -ge $owner.Left -and $sx -lt $owner.Right -and $sy -ge $owner.Top -and $sy -lt $owner.Bottom) { continue }
                $a = $fixture.GetPixel($x, $y)
                $b = $shown.GetPixel($x, $y)
                if ([Math]::Abs([int]$a.R - [int]$b.R) -gt 3 -or [Math]::Abs([int]$a.G - [int]$b.G) -gt 3 -or [Math]::Abs([int]$a.B - [int]$b.B) -gt 3) { throw "Native tooltip pixels leaked into screenshot at $x,$y." }
                $checked++
            }
        }
        if ($checked -lt 64) { throw "Only $checked outside-owner pixels were tested." }
    } finally { $fixture.Dispose(); $shown.Dispose() }
}

function Test-TooltipInteriorForeground([System.Drawing.Bitmap]$image, [int]$screenLeft, [int]$screenTop, [PoratakeTooltipProbe+Rect]$ownerBounds) {
    if ($image.Width -le 8 -or $image.Height -le 8) { return [pscustomobject]@{ Passed = $false; Pixels = 0; Rows = 0; DistinctRows = 0 } }
    $histogram = @{}
    for ($y = 2; $y -lt $image.Height - 2; $y++) {
        for ($x = 2; $x -lt $image.Width - 2; $x++) {
            $xScreen = $screenLeft + $x
            $yScreen = $screenTop + $y
            if ($xScreen -ge $ownerBounds.Left -and $xScreen -lt $ownerBounds.Right -and $yScreen -ge $ownerBounds.Top -and $yScreen -lt $ownerBounds.Bottom) { continue }
            $pixel = $image.GetPixel($x, $y)
            $color = [uint32]([int]$pixel.R -bor ([int]$pixel.G -shl 8) -bor ([int]$pixel.B -shl 16))
            if (-not $histogram.ContainsKey($color)) { $histogram[$color] = 0 }
            $histogram[$color]++
        }
    }
    if ($histogram.Count -eq 0) { return [pscustomobject]@{ Passed = $false; Pixels = 0; Rows = 0; DistinctRows = 0 } }
    [uint32]$background = 0
    $backgroundCount = -1
    foreach ($entry in $histogram.GetEnumerator()) {
        [uint32]$candidate = $entry.Key
        if ($entry.Value -gt $backgroundCount -or ($entry.Value -eq $backgroundCount -and $candidate -lt $background)) {
            $background = $candidate
            $backgroundCount = $entry.Value
        }
    }
    $backgroundChannels = [int[]]@(
        [int]($background -band 255),
        [int](($background -shr 8) -band 255),
        [int](($background -shr 16) -band 255)
    )
    $foregroundPixels = 0
    $foregroundRows = 0
    $foregroundRowCounts = [System.Collections.Generic.HashSet[int]]::new()
    for ($y = 2; $y -lt $image.Height - 2; $y++) {
        $rowForeground = 0
        for ($x = 2; $x -lt $image.Width - 2; $x++) {
            $xScreen = $screenLeft + $x
            $yScreen = $screenTop + $y
            if ($xScreen -ge $ownerBounds.Left -and $xScreen -lt $ownerBounds.Right -and $yScreen -ge $ownerBounds.Top -and $yScreen -lt $ownerBounds.Bottom) { continue }
            $pixel = $image.GetPixel($x, $y)
            $contrast = [Math]::Max([Math]::Abs([int]$pixel.R - $backgroundChannels[0]), [Math]::Max([Math]::Abs([int]$pixel.G - $backgroundChannels[1]), [Math]::Abs([int]$pixel.B - $backgroundChannels[2])))
            if ($contrast -ge 48) { $rowForeground++ }
        }
        if ($rowForeground -gt 0) {
            $foregroundPixels += $rowForeground
            if ($rowForeground -ge 2 -and $rowForeground -le [Math]::Floor(($image.Width - 4) * 0.7)) {
                $foregroundRows++
                $null = $foregroundRowCounts.Add($rowForeground)
            }
        }
    }
    return [pscustomobject]@{
        Passed = $foregroundPixels -ge 20 -and $foregroundRows -ge 4 -and $foregroundRowCounts.Count -ge 3
        Pixels = $foregroundPixels
        Rows = $foregroundRows
        DistinctRows = $foregroundRowCounts.Count
    }
}

function Assert-NativeTooltipPaintProof([string]$path, [IntPtr]$tooltip, [IntPtr]$owner, [string]$text, [uint32]$affinity, [PoratakeTooltipProbe+Rect]$tip, [PoratakeTooltipProbe+Rect]$ownerBounds) {
    $metadataPath = [System.IO.Path]::ChangeExtension($path, '.json')
    Wait-Until { [System.IO.File]::Exists($path) -and [System.IO.File]::Exists($metadataPath) } 5000 'The in-process native tooltip paint proof was not written.' | Out-Null
    $proof = Get-Content -LiteralPath $metadataPath -Raw | ConvertFrom-Json -ErrorAction Stop
    if ([int64]$proof.tooltipHwnd -ne $tooltip.ToInt64() -or [int64]$proof.ownerHwnd -ne $owner.ToInt64() -or [string]$proof.text -cne $text) {
        throw 'The in-process paint proof does not identify the visible tooltip HWND, owner HWND, and text.'
    }
    $geometryChecks = @(
        [pscustomobject]@{ Actual = [int]$proof.tooltipBounds.left; Expected = $tip.Left },
        [pscustomobject]@{ Actual = [int]$proof.tooltipBounds.top; Expected = $tip.Top },
        [pscustomobject]@{ Actual = [int]$proof.tooltipBounds.right; Expected = $tip.Right },
        [pscustomobject]@{ Actual = [int]$proof.tooltipBounds.bottom; Expected = $tip.Bottom },
        [pscustomobject]@{ Actual = [int]$proof.ownerBounds.left; Expected = $ownerBounds.Left },
        [pscustomobject]@{ Actual = [int]$proof.ownerBounds.top; Expected = $ownerBounds.Top },
        [pscustomobject]@{ Actual = [int]$proof.ownerBounds.right; Expected = $ownerBounds.Right },
        [pscustomobject]@{ Actual = [int]$proof.ownerBounds.bottom; Expected = $ownerBounds.Bottom }
    )
    foreach ($check in $geometryChecks) {
        if ($check.Actual -ne $check.Expected) { throw 'The in-process paint proof geometry does not match the live tooltip and owner windows.' }
    }
    if ([uint32]$proof.affinity -ne $affinity -or [uint32]$proof.affinity -eq 0) { throw 'The in-process paint proof affinity does not match the live protected tooltip.' }
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Bitmap]::new($path)
    try {
        $client = $proof.clientBounds
        if ($image.Width -ne [int]$client.width -or $image.Height -ne [int]$client.height -or $image.Width -le 0 -or $image.Height -le 0) {
            throw 'The in-process paint proof image dimensions do not match its client geometry.'
        }
        $colors = [System.Collections.Generic.HashSet[string]]::new()
        $outsideColors = [System.Collections.Generic.HashSet[string]]::new()
        $outsideNonBackground = 0
        for ($y = 0; $y -lt $image.Height; $y++) {
            for ($x = 0; $x -lt $image.Width; $x++) {
                $pixel = $image.GetPixel($x, $y)
                $color = '{0},{1},{2}' -f $pixel.R, $pixel.G, $pixel.B
                $null = $colors.Add($color)
                $screenX = [int]$client.screenLeft + $x
                $screenY = [int]$client.screenTop + $y
                if ($screenX -lt $ownerBounds.Left -or $screenX -ge $ownerBounds.Right -or $screenY -lt $ownerBounds.Top -or $screenY -ge $ownerBounds.Bottom) {
                    $null = $outsideColors.Add($color)
                    if ($pixel.R -ne 0 -or $pixel.G -ne 0 -or $pixel.B -ne 0) { $outsideNonBackground++ }
                }
            }
        }
        if ($colors.Count -le 2 -or $outsideColors.Count -le 1 -or $outsideNonBackground -lt 20) {
            throw "The in-process paint proof lacks multicolor pixels outside its owner: colors=$($colors.Count), outside-colors=$($outsideColors.Count), outside-non-background=$outsideNonBackground."
        }
        $foreground = Test-TooltipInteriorForeground $image ([int]$client.screenLeft) ([int]$client.screenTop) $ownerBounds
        if (-not $foreground.Passed) {
            throw "The in-process paint proof lacks contrasting interior foreground across several rows: pixels=$($foreground.Pixels), rows=$($foreground.Rows), distinct-row-counts=$($foreground.DistinctRows)."
        }
        $negative = [System.Drawing.Bitmap]::new($image.Width, $image.Height)
        $graphics = [System.Drawing.Graphics]::FromImage($negative)
        $black = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::Black)
        $flat = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(240, 240, 240))
        try {
            $graphics.FillRectangle($black, 0, 0, $negative.Width, $negative.Height)
            $graphics.FillRectangle($flat, 2, 2, [Math]::Max(10, [int]($negative.Width * 0.4)), [Math]::Max(6, [int]($negative.Height * 0.4)))
            $negativeOwner = New-Object PoratakeTooltipProbe+Rect
            $negativeOwner.Left = $negative.Width + 1
            $negativeOwner.Right = $negative.Width + 2
            $negativeOwner.Bottom = $negative.Height
            $negativeResult = Test-TooltipInteriorForeground $negative 0 0 $negativeOwner
            if ($negativeResult.Passed) { throw 'Flat colored paint over untouched black passed the interior foreground negative control.' }
        } finally {
            $flat.Dispose()
            $black.Dispose()
            $graphics.Dispose()
            $negative.Dispose()
        }
    } finally { $image.Dispose() }
    Write-Host "In-process WM_PRINTCLIENT tooltip foreground proof verified for '$text' at $($tip.Left),$($tip.Top)-$($tip.Right),$($tip.Bottom), affinity=$($proof.affinity)."
}

function Stop-CaptureDaemon {
    if (-not $script:captureDaemon) { return }
    if (-not $script:captureDaemon.HasExited) {
        if ($script:captureDaemonReady -and -not $script:captureRpcTimedOut) {
            try {
                $null = Invoke-CaptureDaemonRequest @{ id = 'tooltip-probe-quit'; module = 'system'; method = 'quit' }
                if ($script:captureDaemon.WaitForExit(5000)) { $script:captureDaemon.Dispose(); $script:captureDaemon = $null; return }
            } catch { }
        }
        try {
            $identity = Get-CimInstance Win32_Process -Filter "ProcessId = $($script:captureDaemon.Id)" -ErrorAction Stop
        } catch { $identity = $null }
        $owned = $script:captureDaemonIdentity -and $script:captureDaemon.Id -eq $script:captureDaemonIdentity.ProcessId -and $script:captureDaemon.StartTime -eq $script:captureDaemonIdentity.StartTime
        if ($identity) { $owned = $owned -and [int]$identity.ParentProcessId -eq $PID -and $identity.Name -eq 'poratake-daemon.exe' }
        try { $owned = $owned -and [System.IO.Path]::GetFullPath($script:captureDaemon.MainModule.FileName) -ieq $script:captureDaemonIdentity.ImagePath } catch { $owned = $false }
        if (-not $owned) { $script:safeToDeleteProfile = $false; return }
        $script:captureDaemon.Kill()
        if (-not $script:captureDaemon.WaitForExit(5000)) { $script:safeToDeleteProfile = $false }
    }
    $script:captureDaemon.Dispose()
    $script:captureDaemon = $null
}

try {
    New-Item -ItemType Directory -Path $profileRoot | Out-Null
    New-Item -ItemType Directory -Path $evidenceRoot -Force | Out-Null
    $tooltipProofPath = Join-Path $evidenceRoot 'native-tooltip-paint.png'
    $env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF = $tooltipProofPath
    [System.IO.File]::WriteAllText((Join-Path $profileRoot 'config.json'), '{"onboarding":{"completed":true},"recording":{"micEnabled":false,"camera":{"enabled":false}}}', [System.Text.UTF8Encoding]::new($false))
    Push-Location (Join-Path $root 'src/main')
    try {
        & cargo build --locked --manifest-path (Join-Path $root 'src/main/Cargo.toml') --package poratake-gpui --package poratake-daemon --features poratake-gpui/parity
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

    if (-not [PoratakeTooltipProbe]::SetCursorPos($bounds.Left - 20, $bounds.Top - 20)) { throw 'Could not move pointer away before hidden capture.' }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($visibleTooltip.Handle) } 2000 'Tooltip stayed visible before hidden capture.' | Out-Null
    New-Item -ItemType Directory -Path $evidenceRoot -Force | Out-Null
    Start-CaptureDaemon
    $captureRoot = Join-Path $evidenceRoot 'captures'
    New-Item -ItemType Directory -Path $captureRoot -Force | Out-Null
    $baseCapture = Join-Path $captureRoot 'tooltip-before-fixture.png'
    $fixtureCapture = Join-Path $captureRoot 'tooltip-fixture.png'
    $shownCapture = Join-Path $captureRoot 'tooltip-shown.png'
    Capture-TooltipArea 'tooltip-base-capture' $tooltipBounds $baseCapture
    $fixtureRect = New-Object PoratakeTooltipFixture+Rect
    $fixtureRect.Left = $tooltipBounds.Left
    $fixtureRect.Top = $tooltipBounds.Top
    $fixtureRect.Right = $tooltipBounds.Right
    $fixtureRect.Bottom = $tooltipBounds.Bottom
    $backdropWindow = [PoratakeTooltipFixture]::CreateBackdrop($fixtureRect, $owner)
    Capture-TooltipArea 'tooltip-fixture-white-capture' $tooltipBounds $fixtureCapture
    if (-not (Test-BackdropCapture $baseCapture $fixtureCapture $tooltipBounds $bounds)) {
        throw 'The daemon did not positively capture the ordinary white test window across the comparison ROI; fixture contrast was inconclusive.'
    }
    Write-Host 'Daemon screenshot positively captured the ordinary white fixture window.'

    $tooltipProofMetadataPath = [System.IO.Path]::ChangeExtension($tooltipProofPath, '.json')
    foreach ($proofFile in @($tooltipProofPath, $tooltipProofMetadataPath)) {
        if ([System.IO.File]::Exists($proofFile)) { Remove-Item -LiteralPath $proofFile -Force -ErrorAction Stop }
        if ([System.IO.File]::Exists($proofFile)) { throw "Could not clear stale test-owned tooltip proof: $proofFile" }
    }
    if (-not [PoratakeTooltipProbe]::SetCursorPos($hoverX, $hoverY)) { throw 'Could not re-enter the tooltip trigger.' }
    $shownTooltip = Wait-Until {
        $handle = Get-VisibleTooltipWindow $application.Id $owner
        if (-not $handle) { return $null }
        $text = [PoratakeTooltipProbe]::TooltipText($handle, $owner, [uint32]$application.Id)
        if ($text -ne $visibleTooltip.Text) { return $null }
        return @{ Handle = $handle; Text = $text }
    } 2000 'Tooltip did not return for screenshot exclusion capture.'
    $shownBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($shownTooltip.Handle, [ref]$shownBounds)) { throw 'Could not read shown tooltip bounds.' }
    if ($shownBounds.Left -ne $tooltipBounds.Left -or $shownBounds.Top -ne $tooltipBounds.Top -or $shownBounds.Right -ne $tooltipBounds.Right -or $shownBounds.Bottom -ne $tooltipBounds.Bottom) {
        throw 'Tooltip moved between hidden and shown captures.'
    }
    Assert-NativeTooltipPaintProof $tooltipProofPath $shownTooltip.Handle $owner $visibleTooltip.Text $affinity $shownBounds $bounds
    if (-not [PoratakeTooltipProbe]::IsWindowVisible($shownTooltip.Handle)) { throw 'Native tooltip became hidden before the production screenshot.' }
    Capture-TooltipArea 'tooltip-shown-capture' $shownBounds $shownCapture
    if (-not [PoratakeTooltipProbe]::IsWindowVisible($shownTooltip.Handle) -or [PoratakeTooltipProbe]::TooltipText($shownTooltip.Handle, $owner, [uint32]$application.Id) -ne $visibleTooltip.Text) {
        throw 'Native tooltip was not visible with the expected text throughout the production screenshot.'
    }
    Assert-TooltipCaptureMatchesFixture $fixtureCapture $shownCapture $shownBounds $bounds
    Write-Host "Capture artifacts preserved in $evidenceRoot; affinity=$affinity."
    $tracePath = Join-Path $evidenceRoot 'daemon-trace.log'
    $traceDeadline = [DateTime]::UtcNow.AddSeconds(5)
    $captureTrace = @()
    do {
        try { if (Test-Path -LiteralPath $tracePath) { $captureTrace = Get-Content -LiteralPath $tracePath } } catch { }
        if (@($captureTrace | Where-Object { $_ -match 'WGC finished' }).Count -ge 3) { break }
        Start-Sleep -Milliseconds 25
    } while ([DateTime]::UtcNow -lt $traceDeadline)
    if (@($captureTrace | Where-Object { $_ -match 'WGC finished' }).Count -lt 3) { Write-Host 'Capture pixel assertion completed; trace markers were not readable before daemon shutdown.' }
    if (-not [PoratakeTooltipProbe]::SetCursorPos($bounds.Left - 20, $bounds.Top - 20)) { throw 'Could not hide tooltip after screenshot capture.' }
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($shownTooltip.Handle) } 2000 'Tooltip stayed visible after screenshot capture.' | Out-Null
    [PoratakeTooltipProbe]::DestroyOwnedWindow($backdropWindow, [uint32]$PID)
    $backdropWindow = [IntPtr]::Zero
    Stop-CaptureDaemon
    try { $captureTrace = Get-Content -LiteralPath $tracePath } catch { $captureTrace = @() }
    if ($captureTrace -match 'GDI fallback starting') {
        Write-Host 'Production screenshot used GDI fallback; the pixel assertion covers that capture path.'
    } elseif ($captureTrace -match 'WGC finished') {
        Write-Host 'Production screenshot completed through WGC without GDI fallback.'
    } else {
        Write-Host 'Daemon trace did not classify the capture backend; pixel assertions remain authoritative.'
    }

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
    $testFailure = $true
    if (Test-Path -LiteralPath $stdoutPath) { Get-Content -LiteralPath $stdoutPath | Write-Host }
    if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath | Write-Host }
    if (Test-Path -LiteralPath $evidenceRoot) {
        Write-Host "Failure capture evidence retained at $evidenceRoot."
        $tracePath = Join-Path $evidenceRoot 'daemon-trace.log'
        if (Test-Path -LiteralPath $tracePath) { Get-Content -LiteralPath $tracePath | Write-Host }
    }
    throw
} finally {
    try { Stop-CaptureDaemon } catch { $safeToDeleteProfile = $false; Write-Host "Screenshot daemon cleanup failed; retained evidence at $evidenceRoot. $_" }
    if ($backdropWindow -ne [IntPtr]::Zero) {
        try { [PoratakeTooltipProbe]::DestroyOwnedWindow($backdropWindow, [uint32]$PID) } catch { $safeToDeleteProfile = $false }
    }
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
    if ($null -eq $previousTooltipProof) { Remove-Item Env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF -ErrorAction SilentlyContinue } else { $env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF = $previousTooltipProof }
    $resolvedProfile = [System.IO.Path]::GetFullPath($profileRoot)
    if ($application -and -not $application.HasExited) { $safeToDeleteProfile = $false }
    foreach ($owned in $ownedDaemons) {
        if (-not $owned.Process.HasExited) { $safeToDeleteProfile = $false }
        $owned.Process.Dispose()
    }
    if ($safeToDeleteProfile -and $resolvedProfile.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and [System.IO.Directory]::Exists($resolvedProfile)) {
        Remove-Item -LiteralPath $resolvedProfile -Recurse -Force
    }
    if (-not $safeToDeleteProfile) {
        if ($testFailure) { Write-Host 'Owned GPUI or daemon process did not exit; retained its temporary profile for safe recovery.' }
        else { throw 'Owned GPUI or daemon process did not exit; retained its temporary profile for safe recovery.' }
    }
}
