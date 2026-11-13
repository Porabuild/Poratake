$ErrorActionPreference = 'Stop'

if ($env:GITHUB_ACTIONS -ne 'true') {
    throw 'This desktop interaction test runs only on an isolated GitHub Actions Windows runner.'
}

$root = if ($env:GITHUB_WORKSPACE) { (Resolve-Path $env:GITHUB_WORKSPACE).Path } else { 'E:\work\poratake\.poracode\worktrees\poracode-nimble-badger-314d2ec2' }
$profileRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'poratake-tooltip-' + [guid]::NewGuid().ToString('N')))
$previewProfileRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'poratake-preview-tooltip-' + [guid]::NewGuid().ToString('N')))
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath()).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
if (-not $profileRoot.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing profile outside the runner temporary directory: $profileRoot"
}
if (-not $previewProfileRoot.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing preview profile outside the runner temporary directory: $previewProfileRoot"
}
$runnerTempRoot = [System.IO.Path]::GetFullPath($env:RUNNER_TEMP).TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $root 'src/main/target' }
$executable = Join-Path $targetDir 'debug/poratake-gpui.exe'
$daemonCandidates = @()
$daemonExecutable = $null
$stdoutPath = Join-Path $profileRoot 'stdout.log'
$stderrPath = Join-Path $profileRoot 'stderr.log'
$previewStdoutPath = Join-Path $previewProfileRoot 'stdout.log'
$previewStderrPath = Join-Path $previewProfileRoot 'stderr.log'
$application = $null
$previewApplication = $null
$previousProfile = $env:PORATAKE_CONFIG_DIR
$previousDaemonPath = $env:PORATAKE_DAEMON_PATH
$previousTooltipProof = $env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF
$previousTooltipVideoProof = $env:PORATAKE_NATIVE_TOOLTIP_VIDEO_PROOF
$ownedDaemons = [System.Collections.Generic.List[object]]::new()
$safeToDeleteProfile = $true
$overlapWindow = [IntPtr]::Zero
$backdropWindow = [IntPtr]::Zero
$markerBackdropWindow = [IntPtr]::Zero
$markerWindow = [IntPtr]::Zero
$markerBitmapHandle = [IntPtr]::Zero
$markerPreviousBitmapHandle = [IntPtr]::Zero
$markerAttachedBitmapHandle = [IntPtr]::Zero
$captureDaemon = $null
$captureDaemonIdentity = $null
$captureDaemonReady = $false
$captureRpcTimedOut = $false
$recordingActive = $false
$testFailure = $false
$previousDpiAwarenessContext = [IntPtr]::Zero
$evidenceRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($runnerTempRoot, 'poratake-visual-tooltip-' + [guid]::NewGuid().ToString('N')))
$captureRoot = Join-Path $evidenceRoot 'captures'
$previewTooltipProofPath = Join-Path $evidenceRoot 'capture-preview-native-tooltip-paint.png'
if (-not $evidenceRoot.StartsWith($runnerTempRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing evidence outside the runner temporary directory: $evidenceRoot"
}
function Capture-OwnedDaemonChildren([System.Diagnostics.Process]$parentApplication) {
    if (-not $parentApplication) { $parentApplication = $script:application }
    if (-not $parentApplication) { return }
    try {
        $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($parentApplication.Id)" -ErrorAction Stop)
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
    [DllImport("user32.dll", EntryPoint="ClientToScreen")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
    [DllImport("user32.dll")] public static extern bool ScreenToClient(IntPtr hwnd, ref Point point);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr hwnd, uint command);
    [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr hwnd, uint flags);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll", EntryPoint="WindowFromPoint")] public static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
    [DllImport("user32.dll", EntryPoint="CreateWindowExW", CharSet=CharSet.Unicode, SetLastError=true)] public static extern IntPtr CreateWindowEx(uint exStyle, string className, string windowName, uint style, int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr parameter);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool SetWindowPos(IntPtr hwnd, IntPtr insertAfter, int x, int y, int width, int height, uint flags);
    [DllImport("user32.dll", SetLastError=true)] public static extern bool DestroyWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool GetWindowDisplayAffinity(IntPtr hwnd, out uint affinity);
    [DllImport("user32.dll", SetLastError = true)] public static extern bool GetLayeredWindowAttributes(IntPtr hwnd, out uint colorKey, out byte alpha, out uint flags);
    [DllImport("dwmapi.dll", PreserveSig = true)] public static extern int DwmGetWindowAttribute(IntPtr hwnd, uint attribute, out int value, uint size);
    [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    [DllImport("user32.dll", SetLastError = true)] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr dpiContext);
    [DllImport("user32.dll")] public static extern IntPtr GetThreadDpiAwarenessContext();
    [DllImport("user32.dll")] public static extern int GetAwarenessFromDpiAwarenessContext(IntPtr dpiContext);
    [DllImport("user32.dll")] public static extern bool AreDpiAwarenessContextsEqual(IntPtr first, IntPtr second);
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

    public static int ZOrderRelation(IntPtr target, IntPtr owner, int maxSteps) {
        if (target == IntPtr.Zero || owner == IntPtr.Zero || target == owner || maxSteps <= 0) return 0;
        var current = owner;
        for (var step = 0; step < maxSteps; step++) {
            current = GetWindow(current, 3);
            if (current == IntPtr.Zero) return 0;
            if (current == target) return 1;
        }
        return -1;
    }

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

function Assert-NativeTooltipWindowState([IntPtr]$tooltip, [IntPtr]$owner, [string]$surface) {
    $cloaked = -1
    $dwmResult = [PoratakeTooltipProbe]::DwmGetWindowAttribute($tooltip, 14, [ref]$cloaked, 4)
    if ($dwmResult -ne 0) { throw "DwmGetWindowAttribute failed for the $surface tooltip with HRESULT $dwmResult." }
    if ($cloaked -ne 0) { throw "The $surface tooltip is DWM-cloaked." }
    $colorKey = [uint32]0
    $alpha = [byte]0
    $flags = [uint32]0
    if (-not [PoratakeTooltipProbe]::GetLayeredWindowAttributes($tooltip, [ref]$colorKey, [ref]$alpha, [ref]$flags)) {
        throw "GetLayeredWindowAttributes failed for the $surface tooltip."
    }
    if ($alpha -eq 0 -or ($flags -band 2) -eq 0) { throw "The $surface tooltip has no nonzero layered alpha." }
    $zOrder = [PoratakeTooltipProbe]::ZOrderRelation($tooltip, $owner, 512)
    if ($zOrder -eq 0) { throw "The $surface tooltip is not above its owner in the top-level z-order." }
    if ($zOrder -lt 0) { throw "Top-level z-order search exceeded its 512-window bound for the $surface tooltip." }
    Write-Host "Native $surface tooltip state checks passed: uncloaked, layered-alpha=$alpha, above owner. These state checks do not prove final on-screen pixels."
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
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint processId);
    [DllImport("user32.dll", EntryPoint="SendMessageW")] static extern IntPtr SendMessage(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam);
    [DllImport("gdi32.dll", SetLastError=true)] static extern bool DeleteObject(IntPtr obj);

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

    public static IntPtr CreateBlackMarker(Rect rect) {
        var hwnd = CreateWindowEx(0x08000080, "STATIC", "", 0x9000010E, rect.Left, rect.Top,
            rect.Right - rect.Left, rect.Bottom - rect.Top, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
        if (hwnd == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "Could not create the video marker.");
        return hwnd;
    }

    public static IntPtr SetBlackMarkerBitmap(IntPtr hwnd, IntPtr bitmap, uint processId) {
        if (hwnd == IntPtr.Zero || bitmap == IntPtr.Zero) throw new InvalidOperationException("The video marker window or bitmap is empty.");
        uint ownerProcessId;
        GetWindowThreadProcessId(hwnd, out ownerProcessId);
        if (ownerProcessId != processId) throw new InvalidOperationException("Refusing to set a bitmap on a marker outside the test process.");
        return SendMessage(hwnd, 0x0172, IntPtr.Zero, bitmap);
    }

    public static IntPtr DetachBlackMarkerBitmap(IntPtr hwnd, uint processId) {
        if (hwnd == IntPtr.Zero) return IntPtr.Zero;
        uint ownerProcessId;
        GetWindowThreadProcessId(hwnd, out ownerProcessId);
        if (ownerProcessId != processId) throw new InvalidOperationException("Refusing to detach a bitmap from a marker outside the test process.");
        return SendMessage(hwnd, 0x0172, IntPtr.Zero, IntPtr.Zero);
    }

    public static void ShowBlackMarker(IntPtr hwnd, Rect rect, uint processId) {
        uint ownerProcessId;
        GetWindowThreadProcessId(hwnd, out ownerProcessId);
        if (ownerProcessId != processId) throw new InvalidOperationException("Refusing to show a marker outside the test process.");
        if (!SetWindowPos(hwnd, new IntPtr(-1), rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top, 0x0050) ||
            !IsWindowVisible(hwnd) || !UpdateWindow(hwnd)) {
            int error = Marshal.GetLastWin32Error();
            throw new System.ComponentModel.Win32Exception(error, "Could not paint the video marker.");
        }
    }

    public static void DeleteBlackMarkerBitmap(IntPtr bitmap) {
        if (bitmap != IntPtr.Zero && !DeleteObject(bitmap)) {
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "Could not release the test-owned marker bitmap.");
        }
    }

    public static void MoveBlackMarker(IntPtr hwnd, Rect rect, uint processId) {
        uint ownerProcessId;
        GetWindowThreadProcessId(hwnd, out ownerProcessId);
        if (ownerProcessId != processId) throw new InvalidOperationException("Refusing to move a marker outside the test process.");
        if (!SetWindowPos(hwnd, new IntPtr(-1), rect.Left, rect.Top, rect.Right - rect.Left, rect.Bottom - rect.Top, 0x0050) ||
            !IsWindowVisible(hwnd) || !UpdateWindow(hwnd)) {
            throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error(), "Could not move and repaint the video marker.");
        }
        Rect actual;
        if (!GetWindowRect(hwnd, out actual) || actual.Left != rect.Left || actual.Top != rect.Top || actual.Right != rect.Right || actual.Bottom != rect.Bottom) {
            throw new InvalidOperationException("The video marker did not reach its expected test-owned bounds.");
        }
    }

    [DllImport("user32.dll", SetLastError=true)] static extern bool DestroyWindow(IntPtr hwnd);
}
'@

function Release-BlackMarker {
    if ($script:markerWindow -ne [IntPtr]::Zero) {
        try {
            $detachedBitmap = [PoratakeTooltipFixture]::DetachBlackMarkerBitmap($script:markerWindow, [uint32]$PID)
            if ($detachedBitmap -ne [IntPtr]::Zero) { $script:markerAttachedBitmapHandle = $detachedBitmap }
        } catch { $script:safeToDeleteProfile = $false }
        try {
            [PoratakeTooltipProbe]::DestroyOwnedWindow($script:markerWindow, [uint32]$PID)
            $script:markerWindow = [IntPtr]::Zero
        } catch { $script:safeToDeleteProfile = $false }
    }
    if ($script:markerWindow -ne [IntPtr]::Zero) { return }
    $releasedHandles = [System.Collections.Generic.HashSet[long]]::new()
    foreach ($bitmap in @($script:markerBitmapHandle, $script:markerPreviousBitmapHandle, $script:markerAttachedBitmapHandle)) {
        if ($bitmap -eq [IntPtr]::Zero -or -not $releasedHandles.Add($bitmap.ToInt64())) { continue }
        try {
            [PoratakeTooltipFixture]::DeleteBlackMarkerBitmap($bitmap)
            if ($script:markerBitmapHandle -eq $bitmap) { $script:markerBitmapHandle = [IntPtr]::Zero }
            if ($script:markerPreviousBitmapHandle -eq $bitmap) { $script:markerPreviousBitmapHandle = [IntPtr]::Zero }
            if ($script:markerAttachedBitmapHandle -eq $bitmap) { $script:markerAttachedBitmapHandle = [IntPtr]::Zero }
        } catch { $script:safeToDeleteProfile = $false }
    }
}

function Invoke-CaptureDaemonRequest([hashtable]$request, [double]$timeoutSeconds = 35.0, [DateTime]$deadline = [DateTime]::MinValue) {
    if (-not $script:captureDaemon -or $script:captureDaemon.HasExited -or $script:captureRpcTimedOut) { throw 'Screenshot daemon is unavailable.' }
    $hasDeadline = $deadline -ne [DateTime]::MinValue
    $script:captureDaemon.StandardInput.WriteLine(($request | ConvertTo-Json -Compress -Depth 8))
    $script:captureDaemon.StandardInput.Flush()
    if (-not $hasDeadline) { $deadline = [DateTime]::UtcNow.AddSeconds($timeoutSeconds) }
    while ([DateTime]::UtcNow -lt $deadline) {
        $remaining = [int]($deadline - [DateTime]::UtcNow).TotalMilliseconds
        if ($remaining -le 0) { $script:captureRpcTimedOut = $true; break }
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

function Capture-TooltipArea([string]$id, [PoratakeTooltipProbe+Rect]$rect, [string]$path, [int]$timeoutSeconds = 35) {
    $result = Invoke-CaptureDaemonRequest @{
        id = $id
        module = 'screenshot'
        method = 'capture-area'
        params = @{ x = $rect.Left; y = $rect.Top; width = $rect.Right - $rect.Left; height = $rect.Bottom - $rect.Top; path = $path; cached = $false }
    } $timeoutSeconds
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

function Assert-PreviewPixelsCaptured([string]$path, [PoratakeTooltipProbe+Rect]$rect, [switch]$Quiet) {
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Bitmap]::new($path)
    try {
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($image.Width -ne $width -or $image.Height -ne $height) { throw 'Preview screenshot dimensions do not match the requested image region.' }
        $magenta = 0
        $checked = 0
        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $pixel = $image.GetPixel($x, $y)
                if ($pixel.R -ge 110 -and $pixel.G -le 80 -and $pixel.B -ge 100) { $magenta++ }
                $checked++
            }
        }
        if ($checked -lt 256 -or $magenta -lt [Math]::Ceiling($checked * 0.7)) {
            throw "The daemon screenshot did not positively capture the deterministic preview image: magenta=$magenta/$checked."
        }
        if (-not $Quiet) { Write-Host "Daemon screenshot positively captured the preview image: magenta=$magenta/$checked pixels." }
    } finally { $image.Dispose() }
}

function Test-RectIntersects([PoratakeTooltipProbe+Rect]$first, [PoratakeTooltipProbe+Rect]$second) {
    return $first.Left -lt $second.Right -and $first.Right -gt $second.Left -and $first.Top -lt $second.Bottom -and $first.Bottom -gt $second.Top
}

function Set-PreviewNeutralPointer([PoratakeTooltipProbe+Point]$point, [IntPtr]$owner) {
    if (-not [PoratakeTooltipProbe]::SetCursorPos($point.X, $point.Y)) { throw 'Could not move the runner pointer to the blank preview area.' }
    $actual = New-Object PoratakeTooltipProbe+Point
    if (-not [PoratakeTooltipProbe]::GetCursorPos([ref]$actual) -or $actual.X -ne $point.X -or $actual.Y -ne $point.Y) {
        throw 'The runner pointer did not reach the blank preview point.'
    }
    $hit = [PoratakeTooltipProbe]::WindowFromPoint($point)
    if ($hit -eq [IntPtr]::Zero -or [PoratakeTooltipProbe]::GetAncestor($hit, 2) -ne $owner) {
        throw 'Native hit-testing at the blank preview point did not reach the preview owner.'
    }
}

function ConvertTo-FixtureRect([PoratakeTooltipProbe+Rect]$rect) {
    $fixtureRect = New-Object PoratakeTooltipFixture+Rect
    $fixtureRect.Left = $rect.Left
    $fixtureRect.Top = $rect.Top
    $fixtureRect.Right = $rect.Right
    $fixtureRect.Bottom = $rect.Bottom
    return $fixtureRect
}

function Assert-WhiteMarkerPanel([string]$path, [PoratakeTooltipProbe+Rect]$rect) {
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Bitmap]::new($path)
    try {
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        if ($image.Width -ne $width -or $image.Height -ne $height) { throw 'Marker underlay capture dimensions do not match the test-owned panel.' }
        $white = 0
        $checked = $width * $height
        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $pixel = $image.GetPixel($x, $y)
                if ($pixel.R -ge 240 -and $pixel.G -ge 240 -and $pixel.B -ge 240) { $white++ }
            }
        }
        if ($checked -lt 256 -or $white -lt [Math]::Ceiling($checked * 0.98)) { throw "The captured marker underlay is not a uniform white positive control: white=$white/$checked." }
    } finally { $image.Dispose() }
}

function Assert-MarkerPanel([string]$path, [PoratakeTooltipProbe+Rect]$panel, [PoratakeTooltipProbe+Rect]$blackRect, [PoratakeTooltipProbe+Rect]$whiteRect) {
    Add-Type -AssemblyName System.Drawing
    $image = [System.Drawing.Bitmap]::new($path)
    try {
        $width = $panel.Right - $panel.Left
        $height = $panel.Bottom - $panel.Top
        if ($image.Width -ne $width -or $image.Height -ne $height) { throw 'Marker capture dimensions do not match the test-owned panel.' }
        $black = 0
        $white = 0
        $blackCount = ($blackRect.Right - $blackRect.Left) * ($blackRect.Bottom - $blackRect.Top)
        $whiteCount = ($whiteRect.Right - $whiteRect.Left) * ($whiteRect.Bottom - $whiteRect.Top)
        for ($y = 0; $y -lt $height; $y++) {
            for ($x = 0; $x -lt $width; $x++) {
                $screenX = $panel.Left + $x
                $screenY = $panel.Top + $y
                $pixel = $image.GetPixel($x, $y)
                if ($screenX -ge $blackRect.Left -and $screenX -lt $blackRect.Right -and $screenY -ge $blackRect.Top -and $screenY -lt $blackRect.Bottom -and $pixel.R -le 16 -and $pixel.G -le 16 -and $pixel.B -le 16) { $black++ }
                if ($screenX -ge $whiteRect.Left -and $screenX -lt $whiteRect.Right -and $screenY -ge $whiteRect.Top -and $screenY -lt $whiteRect.Bottom -and $pixel.R -ge 240 -and $pixel.G -ge 240 -and $pixel.B -ge 240) { $white++ }
            }
        }
        if ($black -lt [Math]::Ceiling($blackCount * 0.95) -or $white -lt [Math]::Ceiling($whiteCount * 0.95)) {
            throw "The captured marker state did not show black at its active position and white at its inactive position: black=$black/$blackCount, white=$white/$whiteCount."
        }
        Write-Host "Marker screenshot positive control passed: black=$black/$blackCount, white=$white/$whiteCount."
    } finally { $image.Dispose() }
}

function Test-PreviewPixelsStable([string]$firstPath, [string]$secondPath, [PoratakeTooltipProbe+Rect]$rect) {
    Add-Type -AssemblyName System.Drawing
    $first = [System.Drawing.Bitmap]::new($firstPath)
    $second = [System.Drawing.Bitmap]::new($secondPath)
    try {
        if ($first.Width -ne $second.Width -or $first.Height -ne $second.Height -or $first.Width -ne ($rect.Right - $rect.Left) -or $first.Height -ne ($rect.Bottom - $rect.Top)) {
            throw 'Preview readiness captures do not have matching expected dimensions.'
        }
        $stable = 0
        $checked = $first.Width * $first.Height
        for ($y = 0; $y -lt $first.Height; $y++) {
            for ($x = 0; $x -lt $first.Width; $x++) {
                $a = $first.GetPixel($x, $y)
                $b = $second.GetPixel($x, $y)
                if ([Math]::Abs([int]$a.R - [int]$b.R) -le 3 -and [Math]::Abs([int]$a.G - [int]$b.G) -le 3 -and [Math]::Abs([int]$a.B - [int]$b.B) -le 3) { $stable++ }
            }
        }
        return $stable -ge [Math]::Ceiling($checked * 0.995)
    } finally { $first.Dispose(); $second.Dispose() }
}

function Capture-StableTooltipArea([string]$id, [PoratakeTooltipProbe+Rect]$rect, [string]$path, [scriptblock]$stateCheck, [int]$timeoutMilliseconds = 5000) {
    $script:previewTooltipCaptureAttempt = 0
    $directory = [System.IO.Path]::GetDirectoryName($path)
    $name = [System.IO.Path]::GetFileNameWithoutExtension($path)
    Wait-Until {
        if (-not (& $stateCheck)) { return $null }
        $script:previewTooltipCaptureAttempt++
        $first = Join-Path $directory ("{0}-{1:D2}-a.png" -f $name, $script:previewTooltipCaptureAttempt)
        $second = Join-Path $directory ("{0}-{1:D2}-b.png" -f $name, $script:previewTooltipCaptureAttempt)
        Capture-TooltipArea "$id-$script:previewTooltipCaptureAttempt-a" $rect $first 5
        if (-not (& $stateCheck)) { return $null }
        Capture-TooltipArea "$id-$script:previewTooltipCaptureAttempt-b" $rect $second 5
        if (-not (& $stateCheck) -or -not (Test-PreviewPixelsStable $first $second $rect)) { return $null }
        Copy-Item -LiteralPath $second -Destination $path -Force -ErrorAction Stop
        return $true
    } $timeoutMilliseconds "The $id capture did not reach stable pixels while its native tooltip state held."
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

function Assert-NativeTooltipPaintProof([string]$path, [IntPtr]$tooltip, [IntPtr]$owner, [string]$text, [uint32]$affinity, [PoratakeTooltipProbe+Rect]$tip, [PoratakeTooltipProbe+Rect]$ownerBounds, [switch]$IncludeOwnerPixels) {
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
        $paintOwnerBounds = $ownerBounds
        if ($IncludeOwnerPixels) { $paintOwnerBounds = New-Object PoratakeTooltipProbe+Rect }
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
                if ($screenX -lt $paintOwnerBounds.Left -or $screenX -ge $paintOwnerBounds.Right -or $screenY -lt $paintOwnerBounds.Top -or $screenY -ge $paintOwnerBounds.Bottom) {
                    $null = $outsideColors.Add($color)
                    if ($pixel.R -ne 0 -or $pixel.G -ne 0 -or $pixel.B -ne 0) { $outsideNonBackground++ }
                }
            }
        }
        if ($colors.Count -le 2 -or $outsideColors.Count -le 1 -or $outsideNonBackground -lt 20) {
            throw "The in-process paint proof lacks multicolor pixels outside its owner: colors=$($colors.Count), outside-colors=$($outsideColors.Count), outside-non-background=$outsideNonBackground."
        }
        $foreground = Test-TooltipInteriorForeground $image ([int]$client.screenLeft) ([int]$client.screenTop) $paintOwnerBounds
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
    $previousDpiAwarenessContext = [PoratakeTooltipProbe]::SetThreadDpiAwarenessContext([IntPtr](-4))
    if ($previousDpiAwarenessContext -eq [IntPtr]::Zero) { throw 'Could not set the probe thread to per-monitor-aware V2.' }
    $probeDpiAwarenessContext = [PoratakeTooltipProbe]::GetThreadDpiAwarenessContext()
    if ([PoratakeTooltipProbe]::GetAwarenessFromDpiAwarenessContext($probeDpiAwarenessContext) -ne 2 -or -not [PoratakeTooltipProbe]::AreDpiAwarenessContextsEqual($probeDpiAwarenessContext, [IntPtr](-4))) {
        throw 'The probe thread did not enter the per-monitor-aware V2 coordinate context.'
    }
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
    Assert-NativeTooltipWindowState $shownTooltip.Handle $owner 'recording-control'
    Assert-TooltipCaptureMatchesFixture $fixtureCapture $shownCapture $shownBounds $bounds
    Write-Host "Capture artifacts preserved in $evidenceRoot; affinity=$affinity."

    $captureBounds = New-Object PoratakeTooltipProbe+Rect
    $captureBounds.Left = $monitor.Monitor.Left
    $captureBounds.Top = $monitor.Monitor.Top
    $captureWidth = $monitor.Monitor.Right - $monitor.Monitor.Left
    $captureHeight = $monitor.Monitor.Bottom - $monitor.Monitor.Top
    $captureBounds.Right = $captureBounds.Left + ($captureWidth - ($captureWidth % 2))
    $captureBounds.Bottom = $captureBounds.Top + ($captureHeight - ($captureHeight % 2))
    if ($tooltipBounds.Left -lt $captureBounds.Left -or $tooltipBounds.Top -lt $captureBounds.Top -or $tooltipBounds.Right -gt $captureBounds.Right -or $tooltipBounds.Bottom -gt $captureBounds.Bottom) {
        throw 'The tooltip bounds do not fit inside the even-sized recording monitor rectangle.'
    }
    $markerSize = 32
    $markerInitialBounds = $null
    $markerMovedBounds = $null
    $markerPanelBounds = $null
    foreach ($offsetY in @(120, 180, 240, 300, 360)) {
        $candidateA = New-Object PoratakeTooltipProbe+Rect
        $candidateA.Left = $monitor.Work.Left + 24
        $candidateA.Top = $monitor.Work.Top + $offsetY
        $candidateA.Right = $candidateA.Left + $markerSize
        $candidateA.Bottom = $candidateA.Top + $markerSize
        $candidateB = New-Object PoratakeTooltipProbe+Rect
        $candidateB.Left = $candidateA.Left + 96
        $candidateB.Top = $candidateA.Top
        $candidateB.Right = $candidateB.Left + $markerSize
        $candidateB.Bottom = $candidateB.Top + $markerSize
        $candidatePanel = New-Object PoratakeTooltipProbe+Rect
        $candidatePanel.Left = $candidateA.Left
        $candidatePanel.Top = $candidateA.Top
        $candidatePanel.Right = $candidateB.Right
        $candidatePanel.Bottom = $candidateA.Bottom
        $insideWorkArea = $candidatePanel.Left -ge $monitor.Work.Left -and $candidatePanel.Top -ge $monitor.Work.Top -and $candidatePanel.Right -le $monitor.Work.Right -and $candidatePanel.Bottom -le $monitor.Work.Bottom
        $insideCapture = $candidatePanel.Left -ge $captureBounds.Left -and $candidatePanel.Top -ge $captureBounds.Top -and $candidatePanel.Right -le $captureBounds.Right -and $candidatePanel.Bottom -le $captureBounds.Bottom
        if (-not $insideWorkArea -or -not $insideCapture -or (Test-RectIntersects $candidatePanel $bounds) -or (Test-RectIntersects $candidatePanel $tooltipBounds)) { continue }
        $markerInitialBounds = $candidateA
        $markerMovedBounds = $candidateB
        $markerPanelBounds = $candidatePanel
        break
    }
    if (-not $markerInitialBounds) { throw 'Could not find a marker location inside the monitor and outside the recording owner and tooltip.' }
    $markerBackdropWindow = [PoratakeTooltipFixture]::CreateBackdrop((ConvertTo-FixtureRect $markerPanelBounds), $owner)
    $markerBackdropBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($markerBackdropWindow, [ref]$markerBackdropBounds) -or $markerBackdropBounds.Left -ne $markerPanelBounds.Left -or $markerBackdropBounds.Top -ne $markerPanelBounds.Top -or $markerBackdropBounds.Right -ne $markerPanelBounds.Right -or $markerBackdropBounds.Bottom -ne $markerPanelBounds.Bottom) {
        throw 'The test-owned white marker panel did not match its validated geometry.'
    }
    $markerReferencePath = Join-Path $captureRoot 'tooltip-video-marker-underlay.png'
    Capture-TooltipArea 'tooltip-video-marker-underlay' $markerPanelBounds $markerReferencePath
    Assert-WhiteMarkerPanel $markerReferencePath $markerPanelBounds
    Add-Type -AssemblyName System.Drawing
    $markerBitmap = [System.Drawing.Bitmap]::new($markerSize, $markerSize)
    $markerGraphics = [System.Drawing.Graphics]::FromImage($markerBitmap)
    try {
        $markerGraphics.Clear([System.Drawing.Color]::Black)
        $markerSourcePixel = $markerBitmap.GetPixel($markerSize / 2, $markerSize / 2)
        if ($markerSourcePixel.R -ne 0 -or $markerSourcePixel.G -ne 0 -or $markerSourcePixel.B -ne 0) { throw 'The marker bitmap source is not pure black.' }
        $markerBitmapHandle = $markerBitmap.GetHbitmap()
        if ($markerBitmapHandle -eq [IntPtr]::Zero) { throw 'Could not create the fixed-color marker bitmap handle.' }
    } finally {
        $markerGraphics.Dispose()
        $markerBitmap.Dispose()
    }
    $markerWindow = [PoratakeTooltipFixture]::CreateBlackMarker((ConvertTo-FixtureRect $markerInitialBounds))
    $markerPreviousBitmapHandle = [PoratakeTooltipFixture]::SetBlackMarkerBitmap($markerWindow, $markerBitmapHandle, [uint32]$PID)
    if ($markerPreviousBitmapHandle -ne [IntPtr]::Zero) { throw 'The new video marker already had an attached bitmap.' }
    [PoratakeTooltipFixture]::ShowBlackMarker($markerWindow, (ConvertTo-FixtureRect $markerInitialBounds), [uint32]$PID)
    $markerWindowBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($markerWindow, [ref]$markerWindowBounds) -or $markerWindowBounds.Left -ne $markerInitialBounds.Left -or $markerWindowBounds.Top -ne $markerInitialBounds.Top -or $markerWindowBounds.Right -ne $markerInitialBounds.Right -or $markerWindowBounds.Bottom -ne $markerInitialBounds.Bottom) {
        throw 'The test-owned black marker did not match its validated initial geometry.'
    }
    $markerCenter = New-Object PoratakeTooltipProbe+Point
    $markerCenter.X = $markerInitialBounds.Left + [int]($markerSize / 2)
    $markerCenter.Y = $markerInitialBounds.Top + [int]($markerSize / 2)
    if (-not [PoratakeTooltipProbe]::IsWindowVisible($markerWindow)) { throw 'The fixed-color marker window is not visible.' }
    $markerCloaked = -1
    if ([PoratakeTooltipProbe]::DwmGetWindowAttribute($markerWindow, 14, [ref]$markerCloaked, 4) -ne 0 -or $markerCloaked -ne 0) { throw 'The fixed-color marker is DWM-cloaked or its cloak state is unreadable.' }
    if ([PoratakeTooltipProbe]::ZOrderRelation($markerWindow, $markerBackdropWindow, 512) -ne 1) { throw 'The fixed-color marker is not above its white underlay in the bounded z-order check.' }
    $markerHitWindow = [PoratakeTooltipProbe]::WindowFromPoint($markerCenter)
    Write-Host "Marker source/state checks passed: source bitmap center is RGB 0,0,0; HWND is visible, uncloaked, and above its white underlay; hit-test returned $markerHitWindow for marker $markerWindow."
    $markerInitialCapture = Join-Path $captureRoot 'tooltip-video-marker-initial.png'
    Capture-TooltipArea 'tooltip-video-marker-initial' $markerPanelBounds $markerInitialCapture
    Assert-MarkerPanel $markerInitialCapture $markerPanelBounds $markerInitialBounds $markerMovedBounds
    Write-Host "Video marker positive control passed at $($markerInitialBounds.Left),$($markerInitialBounds.Top) and $($markerMovedBounds.Left),$($markerMovedBounds.Top), outside tooltip and owner bounds."
    $recordingDirectory = Join-Path $evidenceRoot 'recording'
    New-Item -ItemType Directory -Path $recordingDirectory -Force | Out-Null
    $recordingPath = Join-Path $recordingDirectory 'tooltip-recording.mp4'
    $recordingStart = Invoke-CaptureDaemonRequest @{
        id = 'tooltip-video-start'
        module = 'screen-recorder'
        method = 'start'
        params = @{
            x = $captureBounds.Left
            y = $captureBounds.Top
            width = $captureBounds.Right - $captureBounds.Left
            height = $captureBounds.Bottom - $captureBounds.Top
            frameRate = 15
            keyboardEnabled = $false
            includeAudio = $false
            micEnabled = $false
            cameraEnabled = $false
            outputPath = $recordingPath
        }
    }
    if (-not $recordingStart.success -or $recordingStart.state -ne 'recording') { throw 'The native video recording did not start.' }
    $recordingActive = $true
    $recordingClock = [System.Diagnostics.Stopwatch]::StartNew()
    $recordingDeadline = [DateTime]::UtcNow.AddSeconds(20)
    $recordingDuration = 0.0
    $markerMovedAtSeconds = [double]::NaN
    do {
        if (-not [PoratakeTooltipProbe]::IsWindowVisible($shownTooltip.Handle) -or [PoratakeTooltipProbe]::TooltipText($shownTooltip.Handle, $owner, [uint32]$application.Id) -cne $visibleTooltip.Text) {
            throw 'The native tooltip stopped showing its verified text during video recording.'
        }
        if ([double]::IsNaN($markerMovedAtSeconds) -and $recordingClock.Elapsed.TotalSeconds -ge 0.5) {
            [PoratakeTooltipFixture]::MoveBlackMarker($markerWindow, (ConvertTo-FixtureRect $markerMovedBounds), [uint32]$PID)
            $markerMovedAtSeconds = $recordingClock.Elapsed.TotalSeconds
            Write-Host "Video marker moved to its second validated position at $([Math]::Round($markerMovedAtSeconds, 3))s."
        }
        $expectedMarkerBounds = if ([double]::IsNaN($markerMovedAtSeconds)) { $markerInitialBounds } else { $markerMovedBounds }
        $currentMarkerBounds = New-Object PoratakeTooltipProbe+Rect
        if (-not [PoratakeTooltipProbe]::IsWindowVisible($markerWindow) -or -not [PoratakeTooltipProbe]::GetWindowRect($markerWindow, [ref]$currentMarkerBounds) -or $currentMarkerBounds.Left -ne $expectedMarkerBounds.Left -or $currentMarkerBounds.Top -ne $expectedMarkerBounds.Top -or $currentMarkerBounds.Right -ne $expectedMarkerBounds.Right -or $currentMarkerBounds.Bottom -ne $expectedMarkerBounds.Bottom) {
            throw 'The test-owned video marker left its expected position during recording.'
        }
        if ([DateTime]::UtcNow -ge $recordingDeadline) { break }
        $status = Invoke-CaptureDaemonRequest @{ id = "tooltip-video-status-$([guid]::NewGuid().ToString('N'))"; module = 'screen-recorder'; method = 'status' } -deadline $recordingDeadline
        if ($status.state -ne 'recording') { throw "The video recorder left recording state: $($status.state)" }
        $recordingDuration = [double]$status.duration
        $markerSettled = -not [double]::IsNaN($markerMovedAtSeconds) -and $recordingClock.Elapsed.TotalSeconds - $markerMovedAtSeconds -ge 1.2
        if ($recordingDuration -ge 2.0 -and $markerSettled) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $recordingDeadline)
    if ($recordingDuration -lt 2.0 -or -not $markerSettled) {
        throw "The video did not reach its duration and marker-stability conditions before the deadline: duration=${recordingDuration}s, marker-moved-at=${markerMovedAtSeconds}s, elapsed=$($recordingClock.Elapsed.TotalSeconds)s."
    }
    if ([double]::IsNaN($markerMovedAtSeconds) -or $markerMovedAtSeconds -lt 0.5) { throw "The test-owned marker moved before the expected video time: $markerMovedAtSeconds s." }
    $recordingClock.Stop()
    $recordingStop = Invoke-CaptureDaemonRequest @{ id = 'tooltip-video-stop'; module = 'screen-recorder'; method = 'stop' }
    if (-not $recordingStop.success -or $recordingStop.outputPath -ne $recordingPath) { throw 'The native video recording did not finalize at its test-owned output path.' }
    $recordingActive = $false
    if (-not [System.IO.File]::Exists($recordingPath) -or (Get-Item -LiteralPath $recordingPath).Length -eq 0) { throw 'The native video recording is missing or empty.' }
    if (-not [PoratakeTooltipProbe]::IsWindowVisible($shownTooltip.Handle) -or [PoratakeTooltipProbe]::TooltipText($shownTooltip.Handle, $owner, [uint32]$application.Id) -cne $visibleTooltip.Text) {
        throw 'The native tooltip was not visible with its verified text through video finalization.'
    }
    $videoProofPath = Join-Path $evidenceRoot 'tooltip-video-proof.json'
    $paintProofMetadata = Get-Content -LiteralPath $tooltipProofMetadataPath -Raw | ConvertFrom-Json -ErrorAction Stop
    $videoProof = @{
        videoPath = $recordingPath
        referencePath = $fixtureCapture
        markerReferencePath = $markerReferencePath
        markerMoveSeconds = $markerMovedAtSeconds
        paintProofPath = $tooltipProofPath
        tooltipHwnd = $shownTooltip.Handle.ToInt64()
        ownerHwnd = $owner.ToInt64()
        tooltipText = $visibleTooltip.Text
        captureBounds = @{
            left = $captureBounds.Left
            top = $captureBounds.Top
            right = $captureBounds.Right
            bottom = $captureBounds.Bottom
        }
        tooltipBounds = @{
            left = $tooltipBounds.Left
            top = $tooltipBounds.Top
            right = $tooltipBounds.Right
            bottom = $tooltipBounds.Bottom
        }
        ownerBounds = @{
            left = $bounds.Left
            top = $bounds.Top
            right = $bounds.Right
            bottom = $bounds.Bottom
        }
        paintClientBounds = @{
            screenLeft = [int]$paintProofMetadata.clientBounds.screenLeft
            screenTop = [int]$paintProofMetadata.clientBounds.screenTop
            width = [int]$paintProofMetadata.clientBounds.width
            height = [int]$paintProofMetadata.clientBounds.height
        }
        markerPanelBounds = @{
            left = $markerPanelBounds.Left
            top = $markerPanelBounds.Top
            right = $markerPanelBounds.Right
            bottom = $markerPanelBounds.Bottom
        }
        markerInitialBounds = @{
            left = $markerInitialBounds.Left
            top = $markerInitialBounds.Top
            right = $markerInitialBounds.Right
            bottom = $markerInitialBounds.Bottom
        }
        markerMovedBounds = @{
            left = $markerMovedBounds.Left
            top = $markerMovedBounds.Top
            right = $markerMovedBounds.Right
            bottom = $markerMovedBounds.Bottom
        }
    }
    [System.IO.File]::WriteAllText($videoProofPath, ($videoProof | ConvertTo-Json -Compress -Depth 5), [System.Text.UTF8Encoding]::new($false))
    $env:PORATAKE_NATIVE_TOOLTIP_VIDEO_PROOF = $videoProofPath
    Release-BlackMarker
    if ($markerWindow -ne [IntPtr]::Zero -or $markerBitmapHandle -ne [IntPtr]::Zero -or $markerPreviousBitmapHandle -ne [IntPtr]::Zero -or $markerAttachedBitmapHandle -ne [IntPtr]::Zero) {
        throw 'Could not detach and release the test-owned marker bitmap.'
    }
    [PoratakeTooltipProbe]::DestroyOwnedWindow($markerBackdropWindow, [uint32]$PID)
    $markerBackdropWindow = [IntPtr]::Zero
    Push-Location (Join-Path $root 'src/main')
    try {
        & cargo test --locked --manifest-path (Join-Path $root 'src/main/Cargo.toml') --package poratake-gpui --bin poratake-gpui video::decoder::tests::recorded_tooltip_is_excluded_from_video_frames -- --ignored --exact
        if ($LASTEXITCODE -ne 0) { throw "Decoded tooltip video exclusion test failed with exit code $LASTEXITCODE" }
        & cargo test --locked --manifest-path (Join-Path $root 'src/main/Cargo.toml') --package poratake-gpui --bin poratake-gpui video::export::tests::native_recording_edit_exports_and_decodes -- --ignored --exact
        if ($LASTEXITCODE -ne 0) { throw "Native recording edit/export regression failed with exit code $LASTEXITCODE" }
    } finally {
        Pop-Location
    }
    Write-Host "Decoded native recording and edited preview/export pixels checked; video=$recordingPath."

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

    New-Item -ItemType Directory -Path $previewProfileRoot -Force | Out-Null
    $previewImagePath = Join-Path $captureRoot 'capture-preview-source.png'
    Add-Type -AssemblyName System.Drawing
    $previewImage = [System.Drawing.Bitmap]::new(200, 140)
    $previewGraphics = [System.Drawing.Graphics]::FromImage($previewImage)
    try {
        $previewGraphics.Clear([System.Drawing.Color]::FromArgb(230, 0, 200))
        $previewImage.Save($previewImagePath, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $previewGraphics.Dispose()
        $previewImage.Dispose()
    }
    [System.IO.File]::WriteAllText((Join-Path $previewProfileRoot 'config.json'), '{"onboarding":{"completed":true},"recording":{"micEnabled":false,"camera":{"enabled":false}},"preview":{"corner":"bottom-right","autoDismiss":false}}', [System.Text.UTF8Encoding]::new($false))
    $env:PORATAKE_CONFIG_DIR = $previewProfileRoot
    $env:PORATAKE_DAEMON_PATH = $daemonExecutable
    $env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF = $previewTooltipProofPath
    try {
        $previewApplication = Start-Process -FilePath $executable -ArgumentList @('--preview-window', 'capture-preview', $previewImagePath) -WorkingDirectory $root -RedirectStandardOutput $previewStdoutPath -RedirectStandardError $previewStderrPath -WindowStyle Hidden -PassThru
    } finally {
        $env:PORATAKE_CONFIG_DIR = $profileRoot
        $env:PORATAKE_DAEMON_PATH = $daemonExecutable
        $env:PORATAKE_NATIVE_TOOLTIP_PARITY_PROOF = $tooltipProofPath
    }
    if (-not $previewApplication) { throw 'Could not launch the isolated capture-preview app.' }
    Capture-OwnedDaemonChildren $previewApplication

    $previewOwner = Wait-Until {
        foreach ($handle in [PoratakeTooltipProbe]::Windows([uint32]$previewApplication.Id)) {
            if ([PoratakeTooltipProbe]::WindowText($handle) -ne 'Poratake capture preview' -or -not [PoratakeTooltipProbe]::IsWindowVisible($handle)) { continue }
            $rect = New-Object PoratakeTooltipProbe+Rect
            if (-not [PoratakeTooltipProbe]::GetWindowRect($handle, [ref]$rect)) { continue }
            if ($rect.Right -le $rect.Left -or $rect.Bottom -le $rect.Top) { continue }
            return @{ Handle = $handle; Bounds = $rect }
        }
        return $null
    } 30000 'The capture-preview window did not appear.'
    Capture-OwnedDaemonChildren $previewApplication
    $previewOwnerHandle = $previewOwner.Handle
    $previewOwnerBounds = $previewOwner.Bounds
    $previewDpi = [PoratakeTooltipProbe]::GetDpiForWindow($previewOwnerHandle)
    if ($previewDpi -lt 96) { throw "The capture-preview window returned an invalid DPI: $previewDpi" }
    $previewScale = [double]$previewDpi / 96.0
    $previewClientRect = New-Object PoratakeTooltipProbe+Rect
    $previewClientOrigin = New-Object PoratakeTooltipProbe+Point
    if (-not [PoratakeTooltipProbe]::GetClientRect($previewOwnerHandle, [ref]$previewClientRect) -or -not [PoratakeTooltipProbe]::ClientToScreen($previewOwnerHandle, [ref]$previewClientOrigin)) {
        throw 'Could not measure the capture-preview client area in screen coordinates.'
    }
    $previewClientBounds = New-Object PoratakeTooltipProbe+Rect
    $previewClientBounds.Left = $previewClientOrigin.X + $previewClientRect.Left
    $previewClientBounds.Top = $previewClientOrigin.Y + $previewClientRect.Top
    $previewClientBounds.Right = $previewClientOrigin.X + $previewClientRect.Right
    $previewClientBounds.Bottom = $previewClientOrigin.Y + $previewClientRect.Bottom
    $previewClientWidth = $previewClientRect.Right - $previewClientRect.Left
    $previewClientHeight = $previewClientRect.Bottom - $previewClientRect.Top
    $expectedPreviewClientWidth = [int][Math]::Round(208 * $previewScale)
    $expectedPreviewClientHeight = [int][Math]::Round(604 * $previewScale)
    if ([Math]::Abs($previewClientWidth - $expectedPreviewClientWidth) -gt 4 -or [Math]::Abs($previewClientHeight - $expectedPreviewClientHeight) -gt 5) {
        throw "Capture-preview client geometry does not match its expected DPI-scaled stack: ${previewClientWidth}x${previewClientHeight}, expected ${expectedPreviewClientWidth}x${expectedPreviewClientHeight}, dpi=$previewDpi; outer HWND bounds are $($previewOwnerBounds.Right - $previewOwnerBounds.Left)x$($previewOwnerBounds.Bottom - $previewOwnerBounds.Top)."
    }
    if ($previewClientBounds.Left -lt $previewOwnerBounds.Left -or $previewClientBounds.Top -lt $previewOwnerBounds.Top -or $previewClientBounds.Right -gt $previewOwnerBounds.Right -or $previewClientBounds.Bottom -gt $previewOwnerBounds.Bottom) {
        throw 'The capture-preview client bounds fall outside its outer HWND bounds.'
    }
    $previewCard = New-Object PoratakeTooltipProbe+Rect
    $previewCard.Left = $previewClientBounds.Left + [int][Math]::Round(4 * $previewScale)
    $previewCard.Top = $previewClientBounds.Top + [int][Math]::Round(460 * $previewScale)
    $previewCard.Right = $previewCard.Left + [int][Math]::Round(200 * $previewScale)
    $previewCard.Bottom = $previewCard.Top + [int][Math]::Round(140 * $previewScale)
    if ($previewCard.Left -lt $previewClientBounds.Left -or $previewCard.Top -lt $previewClientBounds.Top -or $previewCard.Right -gt $previewClientBounds.Right -or $previewCard.Bottom -gt $previewClientBounds.Bottom) {
        throw 'The test-owned preview card geometry falls outside its preview client area.'
    }
    $previewMonitor = New-Object PoratakeTooltipProbe+MonitorInfo
    $previewMonitor.Size = [PoratakeTooltipProbe]::MonitorInfoSize()
    $previewMonitorHandle = [PoratakeTooltipProbe]::MonitorFromWindow($previewOwnerHandle, 2)
    if ($previewMonitorHandle -eq [IntPtr]::Zero -or -not [PoratakeTooltipProbe]::GetMonitorInfo($previewMonitorHandle, [ref]$previewMonitor)) { throw 'Could not read capture-preview monitor bounds.' }
    if ($previewCard.Left -lt $previewMonitor.Monitor.Left -or $previewCard.Top -lt $previewMonitor.Monitor.Top -or $previewCard.Right -gt $previewMonitor.Monitor.Right -or $previewCard.Bottom -gt $previewMonitor.Monitor.Bottom) {
        throw 'The test-owned preview card falls outside monitor bounds.'
    }
    $previewPositiveControl = New-Object PoratakeTooltipProbe+Rect
    $previewPositiveControl.Left = $previewCard.Left + [int][Math]::Round(40 * $previewScale)
    $previewPositiveControl.Top = $previewCard.Top + [int][Math]::Round(40 * $previewScale)
    $previewPositiveControl.Right = $previewCard.Left + [int][Math]::Round(160 * $previewScale)
    $previewPositiveControl.Bottom = $previewCard.Top + [int][Math]::Round(100 * $previewScale)
    if (-not [PoratakeTooltipProbe]::SetCursorPos(($previewCard.Left + $previewCard.Right) / 2, ($previewCard.Top + $previewCard.Bottom) / 2)) {
        throw 'Could not move the runner pointer onto the test-owned preview image.'
    }
    $previewImageCapture = Join-Path $captureRoot 'capture-preview-image.png'
    $script:previewReadinessAttempt = 0
    $script:previewReadinessFailure = 'The preview image did not reach a stable magenta capture before the readiness deadline.'
    $previewImageReady = Wait-Until {
        if ($script:previewReadinessAttempt -ge 6) { return @{ Exhausted = $true } }
        $script:previewReadinessAttempt++
        $firstReadinessCapture = Join-Path $captureRoot ("capture-preview-readiness-{0:D2}-a.png" -f $script:previewReadinessAttempt)
        $secondReadinessCapture = Join-Path $captureRoot ("capture-preview-readiness-{0:D2}-b.png" -f $script:previewReadinessAttempt)
        Capture-TooltipArea "capture-preview-image-readiness-$($script:previewReadinessAttempt)-a" $previewPositiveControl $firstReadinessCapture 10
        try { Assert-PreviewPixelsCaptured $firstReadinessCapture $previewPositiveControl -Quiet } catch { $script:previewReadinessFailure = $_.Exception.Message; return $null }
        Capture-TooltipArea "capture-preview-image-readiness-$($script:previewReadinessAttempt)-b" $previewPositiveControl $secondReadinessCapture 10
        try {
            Assert-PreviewPixelsCaptured $secondReadinessCapture $previewPositiveControl -Quiet
            if (-not (Test-PreviewPixelsStable $firstReadinessCapture $secondReadinessCapture $previewPositiveControl)) {
                throw 'Preview image pixels changed between readiness captures.'
            }
            Copy-Item -LiteralPath $secondReadinessCapture -Destination $previewImageCapture -Force -ErrorAction Stop
            return $true
        } catch {
            $script:previewReadinessFailure = $_.Exception.Message
            return $null
        }
    } 15000 ''
    if (-not $previewImageReady -or $previewImageReady.Exhausted) { throw "Capture-preview image readiness failed after $($script:previewReadinessAttempt) bounded attempts: $script:previewReadinessFailure" }
    Assert-PreviewPixelsCaptured $previewImageCapture $previewPositiveControl

    $previewTrigger = New-Object PoratakeTooltipProbe+Point
    $previewTrigger.X = $previewCard.Left + [int][Math]::Round(20 * $previewScale)
    $previewTrigger.Y = $previewCard.Top + [int][Math]::Round(20 * $previewScale)
    if (-not [PoratakeTooltipProbe]::SetCursorPos($previewTrigger.X, $previewTrigger.Y)) { throw 'Could not move the runner pointer onto the capture-preview close control.' }
    $previewTooltip = Wait-Until {
        $handle = Get-VisibleTooltipWindow $previewApplication.Id $previewOwnerHandle
        if (-not $handle) { return $null }
        $text = [PoratakeTooltipProbe]::TooltipText($handle, $previewOwnerHandle, [uint32]$previewApplication.Id)
        if ($text -cne 'Close preview') { return $null }
        return @{ Handle = $handle; Text = $text }
    } 5000 'The capture-preview native tooltip did not show its expected text.'
    if ([PoratakeTooltipProbe]::GetWindow($previewTooltip.Handle, 4) -ne $previewOwnerHandle) { throw 'The capture-preview tooltip is not owned by the real preview window.' }
    $previewTooltipBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($previewTooltip.Handle, [ref]$previewTooltipBounds)) { throw 'Could not read capture-preview tooltip bounds.' }
    $previewTooltipWidth = $previewTooltipBounds.Right - $previewTooltipBounds.Left
    $previewTooltipHeight = $previewTooltipBounds.Bottom - $previewTooltipBounds.Top
    if ($previewTooltipWidth -le 0 -or $previewTooltipHeight -le 0) { throw 'The capture-preview tooltip has invalid geometry.' }
    if ($previewTooltipBounds.Left -lt $previewOwnerBounds.Left -or $previewTooltipBounds.Top -lt $previewOwnerBounds.Top -or $previewTooltipBounds.Right -gt $previewOwnerBounds.Right -or $previewTooltipBounds.Bottom -gt $previewOwnerBounds.Bottom) {
        throw 'The capture-preview tooltip must be measured inside the preview HWND for this no-owner-mask comparison.'
    }
    $previewTriggerBounds = New-Object PoratakeTooltipProbe+Rect
    $previewTriggerBounds.Left = $previewCard.Left + [int][Math]::Round(8 * $previewScale)
    $previewTriggerBounds.Top = $previewCard.Top + [int][Math]::Round(8 * $previewScale)
    $previewTriggerBounds.Right = $previewCard.Left + [int][Math]::Round(32 * $previewScale)
    $previewTriggerBounds.Bottom = $previewCard.Top + [int][Math]::Round(32 * $previewScale)
    if ($previewTooltipBounds.Left -lt $previewTriggerBounds.Right -and $previewTooltipBounds.Right -gt $previewTriggerBounds.Left -and $previewTooltipBounds.Top -lt $previewTriggerBounds.Bottom -and $previewTooltipBounds.Bottom -gt $previewTriggerBounds.Top) {
        throw 'The capture-preview tooltip ROI overlaps its hovered control and would make the hidden/shown comparison ambiguous.'
    }
    $previewNeutralPoint = New-Object PoratakeTooltipProbe+Point
    $previewNeutralPoint.X = $previewCard.Left + [int][Math]::Round(50 * $previewScale)
    $previewNeutralPoint.Y = $previewCard.Top + [int][Math]::Round(70 * $previewScale)
    if ($previewNeutralPoint.X -lt $previewCard.Left -or $previewNeutralPoint.X -ge $previewCard.Right -or $previewNeutralPoint.Y -lt $previewCard.Top -or $previewNeutralPoint.Y -ge $previewCard.Bottom) {
        throw 'The neutral preview hover point falls outside the preview card.'
    }
    if ($previewNeutralPoint.X -lt $previewClientBounds.Left -or $previewNeutralPoint.X -ge $previewClientBounds.Right -or $previewNeutralPoint.Y -lt $previewClientBounds.Top -or $previewNeutralPoint.Y -ge $previewClientBounds.Bottom) {
        throw 'The neutral preview hover point falls outside the preview client area.'
    }
    $neutralCardX = ($previewNeutralPoint.X - $previewCard.Left) / $previewScale
    $neutralCardY = ($previewNeutralPoint.Y - $previewCard.Top) / $previewScale
    if (($neutralCardX -ge 8 -and $neutralCardX -lt 32 -and $neutralCardY -ge 8 -and $neutralCardY -lt 32) -or
        ($neutralCardX -ge 168 -and $neutralCardX -lt 192 -and $neutralCardY -ge 8 -and $neutralCardY -lt 32) -or
        ($neutralCardX -ge 64 -and $neutralCardX -lt 136 -and $neutralCardY -ge 0 -and $neutralCardY -lt 104) -or
        ($neutralCardX -ge 8 -and $neutralCardX -lt 32 -and $neutralCardY -ge 108 -and $neutralCardY -lt 132) -or
        ($neutralCardX -ge 168 -and $neutralCardX -lt 192 -and $neutralCardY -ge 108 -and $neutralCardY -lt 132)) {
        throw 'The neutral preview hover point overlaps a preview control.'
    }
    if ($previewNeutralPoint.X -ge $previewTooltipBounds.Left -and $previewNeutralPoint.X -lt $previewTooltipBounds.Right -and $previewNeutralPoint.Y -ge $previewTooltipBounds.Top -and $previewNeutralPoint.Y -lt $previewTooltipBounds.Bottom) {
        throw 'The neutral preview hover point overlaps the tooltip comparison ROI.'
    }
    $tooltipMonitor = New-Object PoratakeTooltipProbe+MonitorInfo
    $tooltipMonitor.Size = [PoratakeTooltipProbe]::MonitorInfoSize()
    $tooltipMonitorHandle = [PoratakeTooltipProbe]::MonitorFromWindow($previewTooltip.Handle, 2)
    if ($tooltipMonitorHandle -eq [IntPtr]::Zero -or -not [PoratakeTooltipProbe]::GetMonitorInfo($tooltipMonitorHandle, [ref]$tooltipMonitor)) { throw 'Could not read capture-preview tooltip monitor bounds.' }
    if ($previewTooltipBounds.Left -lt $tooltipMonitor.Monitor.Left -or $previewTooltipBounds.Top -lt $tooltipMonitor.Monitor.Top -or $previewTooltipBounds.Right -gt $tooltipMonitor.Monitor.Right -or $previewTooltipBounds.Bottom -gt $tooltipMonitor.Monitor.Bottom) {
        throw 'The capture-preview tooltip falls outside monitor bounds.'
    }
    $previewAffinity = [uint32]0
    if (-not [PoratakeTooltipProbe]::GetWindowDisplayAffinity($previewTooltip.Handle, [ref]$previewAffinity) -or $previewAffinity -eq 0) { throw 'The capture-preview tooltip has no capture-affinity protection.' }
    Write-Host "Visible capture-preview tooltip '$($previewTooltip.Text)' at $($previewTooltipBounds.Left),$($previewTooltipBounds.Top)-$($previewTooltipBounds.Right),$($previewTooltipBounds.Bottom), affinity=$previewAffinity."

    Set-PreviewNeutralPointer $previewNeutralPoint $previewOwnerHandle
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($previewTooltip.Handle) } 2000 'Capture-preview tooltip stayed visible before its fresh paint proof.' | Out-Null
    $previewProofMetadataPath = [System.IO.Path]::ChangeExtension($previewTooltipProofPath, '.json')
    foreach ($proofFile in @($previewTooltipProofPath, $previewProofMetadataPath)) {
        if ([System.IO.File]::Exists($proofFile)) { Remove-Item -LiteralPath $proofFile -Force -ErrorAction Stop }
        if ([System.IO.File]::Exists($proofFile)) { throw "Could not clear stale capture-preview paint proof: $proofFile" }
    }
    if (-not [PoratakeTooltipProbe]::SetCursorPos($previewTrigger.X, $previewTrigger.Y)) { throw 'Could not retrigger capture-preview tooltip paint proof.' }
    $shownPreviewTooltip = Wait-Until {
        $handle = Get-VisibleTooltipWindow $previewApplication.Id $previewOwnerHandle
        if (-not $handle) { return $null }
        $text = [PoratakeTooltipProbe]::TooltipText($handle, $previewOwnerHandle, [uint32]$previewApplication.Id)
        if ($text -cne $previewTooltip.Text) { return $null }
        return @{ Handle = $handle; Text = $text }
    } 3000 'Capture-preview tooltip did not return for paint proof.'
    $shownPreviewBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($shownPreviewTooltip.Handle, [ref]$shownPreviewBounds)) { throw 'Could not read capture-preview tooltip bounds after re-entry.' }
    if ($shownPreviewBounds.Left -ne $previewTooltipBounds.Left -or $shownPreviewBounds.Top -ne $previewTooltipBounds.Top -or $shownPreviewBounds.Right -ne $previewTooltipBounds.Right -or $shownPreviewBounds.Bottom -ne $previewTooltipBounds.Bottom) {
        throw 'Capture-preview tooltip moved between paint proof and screenshot comparison.'
    }
    Assert-NativeTooltipPaintProof $previewTooltipProofPath $shownPreviewTooltip.Handle $previewOwnerHandle $shownPreviewTooltip.Text $previewAffinity $shownPreviewBounds $previewOwnerBounds -IncludeOwnerPixels

    Set-PreviewNeutralPointer $previewNeutralPoint $previewOwnerHandle
    Wait-Until { -not [PoratakeTooltipProbe]::IsWindowVisible($shownPreviewTooltip.Handle) } 2000 'Capture-preview tooltip stayed visible at the hidden baseline.' | Out-Null
    $previewHiddenCapture = Join-Path $captureRoot 'capture-preview-tooltip-hidden.png'
    $previewShownCapture = Join-Path $captureRoot 'capture-preview-tooltip-shown.png'
    $hiddenTooltipStateCheck = { -not [PoratakeTooltipProbe]::IsWindowVisible($shownPreviewTooltip.Handle) }
    $null = Capture-StableTooltipArea 'capture-preview-tooltip-hidden' $previewTooltipBounds $previewHiddenCapture $hiddenTooltipStateCheck
    Assert-PreviewPixelsCaptured $previewHiddenCapture $previewTooltipBounds
    if (-not [PoratakeTooltipProbe]::SetCursorPos($previewTrigger.X, $previewTrigger.Y)) { throw 'Could not move back onto the capture-preview tooltip trigger.' }
    $shownPreviewTooltip = Wait-Until {
        $handle = Get-VisibleTooltipWindow $previewApplication.Id $previewOwnerHandle
        if (-not $handle) { return $null }
        $text = [PoratakeTooltipProbe]::TooltipText($handle, $previewOwnerHandle, [uint32]$previewApplication.Id)
        if ($text -cne $previewTooltip.Text) { return $null }
        return @{ Handle = $handle; Text = $text }
    } 3000 'Capture-preview tooltip did not reappear for shown screenshot.'
    $shownPreviewBounds = New-Object PoratakeTooltipProbe+Rect
    if (-not [PoratakeTooltipProbe]::GetWindowRect($shownPreviewTooltip.Handle, [ref]$shownPreviewBounds)) { throw 'Could not read capture-preview tooltip bounds before shown screenshot.' }
    if ($shownPreviewBounds.Left -ne $previewTooltipBounds.Left -or $shownPreviewBounds.Top -ne $previewTooltipBounds.Top -or $shownPreviewBounds.Right -ne $previewTooltipBounds.Right -or $shownPreviewBounds.Bottom -ne $previewTooltipBounds.Bottom) {
        throw 'Capture-preview tooltip moved between hidden and shown screenshot captures.'
    }
    if (-not [PoratakeTooltipProbe]::IsWindowVisible($shownPreviewTooltip.Handle) -or [PoratakeTooltipProbe]::TooltipText($shownPreviewTooltip.Handle, $previewOwnerHandle, [uint32]$previewApplication.Id) -cne 'Close preview') {
        throw 'Capture-preview tooltip was not visible with its verified text during shown screenshot capture.'
    }
    $shownTooltipStateCheck = { [PoratakeTooltipProbe]::IsWindowVisible($shownPreviewTooltip.Handle) -and [PoratakeTooltipProbe]::TooltipText($shownPreviewTooltip.Handle, $previewOwnerHandle, [uint32]$previewApplication.Id) -ceq 'Close preview' }
    $null = Capture-StableTooltipArea 'capture-preview-tooltip-shown' $shownPreviewBounds $previewShownCapture $shownTooltipStateCheck
    if (-not [PoratakeTooltipProbe]::IsWindowVisible($shownPreviewTooltip.Handle) -or [PoratakeTooltipProbe]::TooltipText($shownPreviewTooltip.Handle, $previewOwnerHandle, [uint32]$previewApplication.Id) -cne 'Close preview') {
        throw 'Capture-preview tooltip stopped showing its verified text during the shown screenshot capture.'
    }
    Assert-NativeTooltipWindowState $shownPreviewTooltip.Handle $previewOwnerHandle 'capture-preview'
    $emptyOwner = New-Object PoratakeTooltipProbe+Rect
    $emptyOwner.Left = [int]::MaxValue
    $emptyOwner.Top = [int]::MaxValue
    $emptyOwner.Right = [int]::MaxValue
    $emptyOwner.Bottom = [int]::MaxValue
    Assert-TooltipCaptureMatchesFixture $previewHiddenCapture $previewShownCapture $shownPreviewBounds $emptyOwner
    Write-Host 'Capture-preview tooltip ROI matches with all pixels checked; no preview-owner pixels were excluded.'
} catch {
    $testFailure = $true
    if (Test-Path -LiteralPath $stdoutPath) { Get-Content -LiteralPath $stdoutPath | Write-Host }
    if (Test-Path -LiteralPath $stderrPath) { Get-Content -LiteralPath $stderrPath | Write-Host }
    if (Test-Path -LiteralPath $previewStdoutPath) { Get-Content -LiteralPath $previewStdoutPath | Write-Host }
    if (Test-Path -LiteralPath $previewStderrPath) { Get-Content -LiteralPath $previewStderrPath | Write-Host }
    if (Test-Path -LiteralPath $evidenceRoot) {
        Write-Host "Failure capture evidence retained at $evidenceRoot."
        $tracePath = Join-Path $evidenceRoot 'daemon-trace.log'
        if (Test-Path -LiteralPath $tracePath) { Get-Content -LiteralPath $tracePath | Write-Host }
    }
    throw
} finally {
    if ($recordingActive -and $captureDaemon -and -not $captureDaemon.HasExited -and -not $captureRpcTimedOut) {
        try {
            $null = Invoke-CaptureDaemonRequest @{ id = 'tooltip-video-cleanup-stop'; module = 'screen-recorder'; method = 'stop' }
            $recordingActive = $false
        } catch {
            $safeToDeleteProfile = $false
            Write-Host "Video recording cleanup failed; retained evidence at $evidenceRoot. $_"
        }
    }
    try { Stop-CaptureDaemon } catch { $safeToDeleteProfile = $false; Write-Host "Screenshot daemon cleanup failed; retained evidence at $evidenceRoot. $_" }
    if ($backdropWindow -ne [IntPtr]::Zero) {
        try { [PoratakeTooltipProbe]::DestroyOwnedWindow($backdropWindow, [uint32]$PID) } catch { $safeToDeleteProfile = $false }
    }
    if ($overlapWindow -ne [IntPtr]::Zero) {
        try { [PoratakeTooltipProbe]::DestroyOwnedWindow($overlapWindow, [uint32]$PID) } catch { $safeToDeleteProfile = $false }
    }
    Release-BlackMarker
    if ($markerBackdropWindow -ne [IntPtr]::Zero) {
        try { [PoratakeTooltipProbe]::DestroyOwnedWindow($markerBackdropWindow, [uint32]$PID) } catch { $safeToDeleteProfile = $false }
    }
    Capture-OwnedDaemonChildren
    Capture-OwnedDaemonChildren $previewApplication
    foreach ($ownedApplication in @($application, $previewApplication)) {
        if ($ownedApplication -and -not $ownedApplication.HasExited) {
            try {
                $ownedApplication.Kill()
                if (-not $ownedApplication.WaitForExit(5000)) { $safeToDeleteProfile = $false }
            } catch {
                if (-not $ownedApplication.HasExited) { $safeToDeleteProfile = $false }
            }
        }
    }
    $ownedApplicationIds = @($application, $previewApplication) | Where-Object { $_ } | ForEach-Object { $_.Id }
    foreach ($owned in $ownedDaemons) {
        $process = $owned.Process
        if (-not $process.HasExited) {
            try {
                $identity = Get-CimInstance Win32_Process -Filter "ProcessId = $($owned.ProcessId)" -ErrorAction Stop
                $imagePath = [System.IO.Path]::GetFullPath($process.MainModule.FileName)
                $created = [datetime]$identity.CreationDate
                $sameIdentity = $identity.Name -eq 'poratake-daemon.exe' -and [int]$identity.ParentProcessId -eq $owned.ParentProcessId -and $owned.ParentProcessId -in $ownedApplicationIds -and $imagePath -ieq $owned.ImagePath -and $imagePath -in $daemonCandidates -and $process.StartTime -eq $owned.StartTime -and [Math]::Abs(($process.StartTime - $created).TotalSeconds) -le 1
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
    if ($null -eq $previousTooltipVideoProof) { Remove-Item Env:PORATAKE_NATIVE_TOOLTIP_VIDEO_PROOF -ErrorAction SilentlyContinue } else { $env:PORATAKE_NATIVE_TOOLTIP_VIDEO_PROOF = $previousTooltipVideoProof }
    $resolvedProfile = [System.IO.Path]::GetFullPath($profileRoot)
    $resolvedPreviewProfile = [System.IO.Path]::GetFullPath($previewProfileRoot)
    if (($application -and -not $application.HasExited) -or ($previewApplication -and -not $previewApplication.HasExited)) { $safeToDeleteProfile = $false }
    foreach ($owned in $ownedDaemons) {
        if (-not $owned.Process.HasExited) { $safeToDeleteProfile = $false }
        $owned.Process.Dispose()
    }
    if ($previousDpiAwarenessContext -ne [IntPtr]::Zero) {
        try {
            if ([PoratakeTooltipProbe]::SetThreadDpiAwarenessContext($previousDpiAwarenessContext) -eq [IntPtr]::Zero) {
                throw 'Could not restore the probe thread DPI awareness context.'
            }
        } catch {
            if ($testFailure) { Write-Host "Could not restore the probe thread DPI awareness context: $_" }
            else { throw }
        }
    }
    if ($safeToDeleteProfile -and $resolvedProfile.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and [System.IO.Directory]::Exists($resolvedProfile)) {
        Remove-Item -LiteralPath $resolvedProfile -Recurse -Force
    }
    if ($safeToDeleteProfile -and $resolvedPreviewProfile.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and [System.IO.Directory]::Exists($resolvedPreviewProfile)) {
        Remove-Item -LiteralPath $resolvedPreviewProfile -Recurse -Force
    }
    if (-not $safeToDeleteProfile) {
        if ($testFailure) { Write-Host 'Owned GPUI or daemon process did not exit; retained its temporary profile for safe recovery.' }
        else { throw 'Owned GPUI or daemon process did not exit; retained its temporary profile for safe recovery.' }
    }
}
