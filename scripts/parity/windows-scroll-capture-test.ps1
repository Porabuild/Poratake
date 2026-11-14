$ErrorActionPreference = 'Stop'

if ($env:GITHUB_ACTIONS -ne 'true' -or $env:PORATAKE_RUN_WINDOWS_SCROLL_CAPTURE_TEST -ne '1') {
    Write-Output 'Skipping hosted Windows scroll capture test without CI opt-in.'
    exit 0
}

$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$manifest = Join-Path $root 'src/main/Cargo.toml'
$daemon = Join-Path $root 'src/main/target/debug/poratake-daemon.exe'
$runId = [guid]::NewGuid().ToString('N')
$evidenceRoot = Join-Path $env:RUNNER_TEMP "poratake-scroll-capture-$runId"
$outputPath = Join-Path $env:RUNNER_TEMP "poratake-scroll-unverified-$runId.png"
$verifiedOutputPath = Join-Path $evidenceRoot 'stitched.png'
New-Item -ItemType Directory -Path $evidenceRoot -Force | Out-Null

$fixtureSource = @'
using System;
using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows.Forms;

public sealed class PoratakeScrollFixture : Form
{
    const int FooterHeight = 40;
    const int WheelMessage = 0x020A;
    const int WheelDelta = -120;
    const int BodyColorTag = 0x424F44;
    const int FooterMarkerWidth = 16;
    const int FooterMarkerHeight = 16;
    readonly ManualResetEventSlim ready;
    readonly ManualResetEventSlim wheelChanged = new(false);
    Thread uiThread;
    int wheelCount;
    int scrollOffset;
    string fixtureError;
    int targetMovement;
    int wheelDetents;
    int scrollFrameTransitions;
    Bitmap bodyBitmap;

    [DllImport("user32.dll", SetLastError = true)]
    static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
    [DllImport("user32.dll", SetLastError = true)]
    static extern bool SetWindowPos(IntPtr hwnd, IntPtr after, int x, int y, int width, int height, uint flags);
    [DllImport("dwmapi.dll")]
    static extern int DwmFlush();
    [DllImport("user32.dll")]
    static extern IntPtr WindowFromPoint(Point point);
    [DllImport("user32.dll")]
    static extern IntPtr MonitorFromPoint(Point point, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool GetMonitorInfo(IntPtr monitor, ref MonitorInfo info);
    [DllImport("user32.dll")]
    static extern bool GetWindowRect(IntPtr hwnd, out NativeRect rect);
    [DllImport("user32.dll")]
    static extern bool GetClientRect(IntPtr hwnd, out NativeRect rect);
    [DllImport("user32.dll")]
    static extern bool ClientToScreen(IntPtr hwnd, ref NativePoint point);
    [DllImport("user32.dll")]
    static extern uint GetDpiForWindow(IntPtr hwnd);
    [StructLayout(LayoutKind.Sequential)]
    struct NativeRect { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)]
    struct NativePoint { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct MonitorInfo
    {
        public int Size;
        public NativeRect Monitor;
        public NativeRect Work;
        public uint Flags;
    }

    public int WheelCount => Volatile.Read(ref wheelCount);
    public int ScrollOffset => Volatile.Read(ref scrollOffset);
    public IntPtr FixtureHandle => Handle;
    public int FixtureLeft => Left;
    public int FixtureTop => Top;
    public int FixtureWidth => Width;
    public int FixtureHeight => Height;
    public string FixtureError => Volatile.Read(ref fixtureError);
    public double ScaleFactor => GetDpiForWindow(Handle) / 96.0;

    public static Rectangle PrimaryMonitorBounds()
    {
        SetThreadDpiAwarenessContext(new IntPtr(-4));
        var monitor = MonitorFromPoint(new Point(0, 0), 1);
        var info = new MonitorInfo { Size = Marshal.SizeOf<MonitorInfo>() };
        if (monitor == IntPtr.Zero || !GetMonitorInfo(monitor, ref info))
            throw new InvalidOperationException("Could not query the primary display bounds.");
        return Rectangle.FromLTRB(info.Monitor.Left, info.Monitor.Top, info.Monitor.Right, info.Monitor.Bottom);
    }

    PoratakeScrollFixture(int x, int y, int width, int height, ManualResetEventSlim ready)
    {
        this.ready = ready;
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        TopMost = true;
        StartPosition = FormStartPosition.Manual;
        AutoScaleMode = AutoScaleMode.None;
        DoubleBuffered = true;
        BackColor = Color.Black;
        SetBounds(x, y, width, height);
        Shown += (_, _) =>
        {
            SetWindowPos(Handle, new IntPtr(-1), x, y, width, height, 0x0010 | 0x0040);
            Invalidate();
            Update();
            DwmFlush();
            ready.Set();
        };
    }

    public static PoratakeScrollFixture Start(int x, int y, int width, int height)
    {
        var ready = new ManualResetEventSlim(false);
        PoratakeScrollFixture fixture = null;
        Exception startupError = null;
        Thread uiThread = null;
        uiThread = new Thread(() =>
        {
            try
            {
                SetThreadDpiAwarenessContext(new IntPtr(-4));
                fixture = new PoratakeScrollFixture(x, y, width, height, ready);
                fixture.uiThread = Thread.CurrentThread;
                Application.Run(fixture);
            }
            catch (Exception error)
            {
                if (fixture == null)
                    startupError = error;
                else
                    fixture.SetFixtureError(error.ToString());
                ready.Set();
            }
        });
        uiThread.IsBackground = true;
        uiThread.SetApartmentState(ApartmentState.STA);
        uiThread.Start();
        if (!ready.Wait(TimeSpan.FromSeconds(10)))
            throw new TimeoutException("Fixture window did not become visible.");
        if (startupError != null)
            throw new InvalidOperationException("Fixture window failed to start.", startupError);
        if (fixture == null)
            throw new InvalidOperationException("Fixture window did not initialize.");
        return fixture;
    }

    public bool IsOwnedVisibleTopmostAtCenter()
    {
        if (!Visible)
            return false;
        if (!GetWindowRect(Handle, out var rect))
            return false;
        if (rect.Left != Left || rect.Top != Top || rect.Right != Left + Width || rect.Bottom != Top + Height)
            return false;
        if (!GetClientRect(Handle, out var client))
            return false;
        var clientTopLeft = new NativePoint { X = client.Left, Y = client.Top };
        var clientBottomRight = new NativePoint { X = client.Right, Y = client.Bottom };
        if (!ClientToScreen(Handle, ref clientTopLeft) || !ClientToScreen(Handle, ref clientBottomRight))
            return false;
        if (clientTopLeft.X != Left || clientTopLeft.Y != Top || clientBottomRight.X != Left + Width || clientBottomRight.Y != Top + Height)
            return false;
        int[] xs = { Left + 1, Left + Width / 2, Left + Width - 2 };
        int[] ys = { Top + 1, Top + Height / 2, Top + Height - 2 };
        foreach (int y in ys)
            foreach (int x in xs)
                if (WindowFromPoint(new Point(x, y)) != Handle)
                    return false;
        return true;
    }

    public void ConfigureScroll(int movement, int detents, int frameTransitions)
    {
        if (WheelCount != 0 || movement <= 0 || detents <= 0 || frameTransitions <= 0)
            throw new InvalidOperationException("Scroll fixture cannot accept this scroll plan.");
        var bitmap = new Bitmap(Width, Height - FooterHeight + movement * frameTransitions, PixelFormat.Format32bppArgb);
        var rect = new Rectangle(0, 0, bitmap.Width, bitmap.Height);
        var data = bitmap.LockBits(rect, ImageLockMode.WriteOnly, PixelFormat.Format32bppArgb);
        try
        {
            var row = new byte[Math.Abs(data.Stride)];
            for (int y = 0; y < bitmap.Height; y++)
            {
                for (int x = 0; x < bitmap.Width; x++)
                {
                    Color color = BodyColor(y, x / 16);
                    int offset = x * 4;
                    row[offset] = color.B;
                    row[offset + 1] = color.G;
                    row[offset + 2] = color.R;
                    row[offset + 3] = 255;
                }
                Marshal.Copy(row, 0, IntPtr.Add(data.Scan0, y * data.Stride), row.Length);
            }
        }
        finally
        {
            bitmap.UnlockBits(data);
        }
        var painted = new ManualResetEventSlim(false);
        targetMovement = movement;
        wheelDetents = detents;
        scrollFrameTransitions = frameTransitions;
        BeginInvoke(new Action(() =>
        {
            bodyBitmap = bitmap;
            Invalidate();
            Update();
            DwmFlush();
            painted.Set();
        }));
        if (!painted.Wait(TimeSpan.FromSeconds(10)))
            throw new TimeoutException("Scroll fixture body did not paint.");
        painted.Dispose();
    }

    void SetFixtureError(string error)
    {
        Volatile.Write(ref fixtureError, error);
    }

    public bool WaitForWheelCount(int target, int timeoutMilliseconds)
    {
        var deadline = Environment.TickCount64 + timeoutMilliseconds;
        while (WheelCount < target)
        {
            var remaining = (int)Math.Max(0, deadline - Environment.TickCount64);
            if (remaining == 0 || !wheelChanged.Wait(remaining))
                return WheelCount >= target;
            wheelChanged.Reset();
        }
        return true;
    }

    public void CloseFixture()
    {
        if (IsHandleCreated && !IsDisposed)
        {
            try { BeginInvoke(new Action(Close)); } catch (InvalidOperationException) { }
        }
        if (uiThread != null && Thread.CurrentThread != uiThread)
            uiThread.Join(TimeSpan.FromSeconds(10));
        if (uiThread != null && uiThread.IsAlive)
            throw new TimeoutException("Fixture window did not close.");
        bodyBitmap?.Dispose();
        ready.Dispose();
        wheelChanged.Dispose();
    }

    protected override void WndProc(ref Message message)
    {
        if (message.Msg == WheelMessage)
        {
            int delta = unchecked((short)((message.WParam.ToInt64() >> 16) & 0xffff));
            if (delta != WheelDelta)
            {
                SetFixtureError($"Unexpected wheel delta {delta}.");
                wheelChanged.Set();
                message.Result = IntPtr.Zero;
                return;
            }
            int count = Interlocked.Increment(ref wheelCount);
            int detents = Volatile.Read(ref wheelDetents);
            int movement = Volatile.Read(ref targetMovement);
            int transitions = Volatile.Read(ref scrollFrameTransitions);
            if (detents <= 0 || count > detents * transitions)
            {
                SetFixtureError($"Received wheel detent {count} with a {detents}-detent, {transitions}-transition plan.");
                wheelChanged.Set();
                message.Result = IntPtr.Zero;
                return;
            }
            Interlocked.Exchange(ref scrollOffset, movement * count / detents);
            Invalidate();
            Update();
            DwmFlush();
            wheelChanged.Set();
            message.Result = IntPtr.Zero;
            return;
        }
        base.WndProc(ref message);
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        e.Graphics.SmoothingMode = SmoothingMode.None;
        e.Graphics.PixelOffsetMode = PixelOffsetMode.None;
        if (bodyBitmap != null)
        {
            int bodyHeight = Height - FooterHeight;
            e.Graphics.InterpolationMode = InterpolationMode.NearestNeighbor;
            e.Graphics.DrawImage(bodyBitmap, new Rectangle(0, 0, Width, bodyHeight), new Rectangle(0, ScrollOffset, Width, bodyHeight), GraphicsUnit.Pixel);
        }
        using var footer = new SolidBrush(Color.FromArgb(64, 64, 64));
        e.Graphics.FillRectangle(footer, 0, Height - FooterHeight, Width, FooterHeight);
        int detents = Volatile.Read(ref wheelDetents);
        int markerFrame = detents > 0 ? WheelCount / detents : 0;
        if (markerFrame > 0)
        {
            using var marker = new SolidBrush(FooterMarkerColor(markerFrame));
            e.Graphics.FillRectangle(marker, 1, Height - FooterHeight, FooterMarkerWidth, FooterMarkerHeight);
        }
    }

    static Color FooterMarkerColor(int frame)
    {
        int red = 64 ^ ((frame * 64) & 255);
        return Color.FromArgb(red, 64, 64);
    }

    static Color BodyColor(int row, int column)
    {
        uint value = unchecked((uint)(row * 0x45d9f3b) ^ (uint)(column * 0x119de1f3) ^ BodyColorTag);
        return Color.FromArgb((int)(value & 255), (int)((value >> 8) & 255), (int)((value >> 16) & 255));
    }

    public static void AssertStitchedPng(string path, int bodyRows, int expectedWheelCount, int wheelDetents)
    {
        using var image = new Bitmap(path);
        int expectedWidth = 1024;
        int expectedHeight = bodyRows + FooterHeight;
        int markerFrame = wheelDetents > 0 ? expectedWheelCount / wheelDetents : 0;
        if (image.Width != expectedWidth || image.Height != expectedHeight)
            throw new InvalidOperationException($"Stitched dimensions were {image.Width}x{image.Height}; expected {expectedWidth}x{expectedHeight}.");
        for (int y = 0; y < expectedHeight; y++)
        {
            for (int x = 0; x < expectedWidth; x++)
            {
                Color expected;
                if (y < bodyRows)
                    expected = BodyColor(y, x / 16);
                else if (y - bodyRows < FooterMarkerHeight && markerFrame > 0 &&
                         x >= 1 && x <= FooterMarkerWidth)
                    expected = FooterMarkerColor(markerFrame);
                else
                    expected = Color.FromArgb(64, 64, 64);
                Color actual = image.GetPixel(x, y);
                if (actual.R != expected.R || actual.G != expected.G || actual.B != expected.B)
                    throw new InvalidOperationException($"Stitched pixel mismatch at ({x},{y}): got #{actual.R:X2}{actual.G:X2}{actual.B:X2}, expected #{expected.R:X2}{expected.G:X2}{expected.B:X2}.");
            }
        }
    }

    public static void AssertInitialFixturePixels(int x, int y, int width, int height)
    {
        using var image = new Bitmap(width, height, PixelFormat.Format32bppArgb);
        using var graphics = Graphics.FromImage(image);
        graphics.CopyFromScreen(x, y, 0, 0, new Size(width, height), CopyPixelOperation.SourceCopy);
        int bodyHeight = height - FooterHeight;
        for (int row = 0; row < height; row++)
        {
            for (int column = 0; column < width; column++)
            {
                Color expected = row < bodyHeight ? BodyColor(row, column / 16) : Color.FromArgb(64, 64, 64);
                Color actual = image.GetPixel(column, row);
                if (actual.R != expected.R || actual.G != expected.G || actual.B != expected.B)
                    throw new InvalidOperationException($"Initial fixture pixel mismatch at ({column},{row}): got #{actual.R:X2}{actual.G:X2}{actual.B:X2}, expected #{expected.R:X2}{expected.G:X2}{expected.B:X2}.");
            }
        }
    }
}
'@

Add-Type -AssemblyName System.Drawing -ErrorAction Stop
Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
$desktopAssemblies = Split-Path ([System.Windows.Forms.Form].Assembly.Location)
$fixtureReferences = @((Get-ChildItem (Join-Path $PSHOME 'ref') -Filter '*.dll').FullName) + @(
    [System.Windows.Forms.Form].Assembly.Location,
    (Join-Path $desktopAssemblies 'System.Windows.Forms.Primitives.dll'),
    [System.Drawing.Bitmap].Assembly.Location,
    [System.Drawing.Point].Assembly.Location,
    (Join-Path $desktopAssemblies 'System.Private.Windows.GdiPlus.dll'),
    (Join-Path $desktopAssemblies 'System.Private.Windows.Core.dll')
)
Add-Type -TypeDefinition $fixtureSource -ReferencedAssemblies @($fixtureReferences | Select-Object -Unique) -ErrorAction Stop

$screen = [PoratakeScrollFixture]::PrimaryMonitorBounds()
if ($screen.Width -lt 1024 -or $screen.Height -lt 472) {
    throw "Hosted primary display is too small for the deterministic scroll fixture: $($screen.Width)x$($screen.Height)."
}

$width = 1024
$height = 400
$footerHeight = 40
$bodyViewportHeight = $height - $footerHeight
$scrollFrameTransitions = 3
$x = $screen.Left + [int](($screen.Width - $width) / 2)
$y = $screen.Top + [int](($screen.Height - $height - 72) / 2)
$fixture = $null
$daemonProcess = $null
$script:stdoutReadTask = $null
$stderrReadTask = $null
$capturedLines = [System.Collections.Generic.List[string]]::new()
$pendingEvents = [System.Collections.Generic.Queue[object]]::new()
$observedEvents = [System.Collections.Generic.List[object]]::new()
$fixtureEvidence = $null
$script:requestNumber = 0

try {
    Push-Location (Join-Path $root 'src/main')
    try {
        & cargo build --locked --manifest-path $manifest --package poratake-daemon --bin poratake-daemon
        if ($LASTEXITCODE -ne 0) { throw "Windows daemon build failed with exit code $LASTEXITCODE." }
    } finally {
        Pop-Location
    }
    if (-not (Test-Path -LiteralPath $daemon)) { throw "Windows daemon executable is missing: $daemon" }

    $fixture = [PoratakeScrollFixture]::Start($x, $y, $width, $height)
    if (-not $fixture.IsOwnedVisibleTopmostAtCenter()) {
        throw 'Scroll fixture is not the visible topmost window over the complete capture ROI.'
    }
    $scaleFactor = $fixture.ScaleFactor
    $targetLogicalMovement = [int][Math]::Max(1, [Math]::Round($height / $scaleFactor * 0.7, [MidpointRounding]::AwayFromZero))
    $targetMovement = [int][Math]::Round($targetLogicalMovement * $scaleFactor, [MidpointRounding]::AwayFromZero)
    $wheelDetents = [int][Math]::Ceiling($targetLogicalMovement / 48.0)
    $fixture.ConfigureScroll($targetMovement, $wheelDetents, $scrollFrameTransitions)
    if (-not $fixture.IsOwnedVisibleTopmostAtCenter()) {
        throw 'The painted scroll fixture no longer owns the full capture ROI.'
    }
    [PoratakeScrollFixture]::AssertInitialFixturePixels($fixture.FixtureLeft, $fixture.FixtureTop, $width, $height)
    $totalWheelDetents = $wheelDetents * $scrollFrameTransitions
    $totalMovement = $targetMovement * $scrollFrameTransitions
    $expectedFrameCount = $scrollFrameTransitions + 1
    $bodyRows = $bodyViewportHeight + $totalMovement
    $expectedHeight = $height + $totalMovement
    $expectedLogicalHeight = [int][Math]::Ceiling($expectedHeight / $scaleFactor)

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $daemon
    $startInfo.WorkingDirectory = $root
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.Environment['PORATAKE_TRACE_FILE'] = Join-Path $evidenceRoot 'daemon-trace.log'
    $daemonProcess = [System.Diagnostics.Process]::new()
    $daemonProcess.StartInfo = $startInfo
    if (-not $daemonProcess.Start()) { throw 'Could not start the Windows daemon.' }
    $stderrReadTask = $daemonProcess.StandardError.ReadToEndAsync()

    function Read-DaemonLine([datetime]$deadline) {
        while ([datetime]::UtcNow -lt $deadline) {
            if (-not $script:stdoutReadTask) { $script:stdoutReadTask = $daemonProcess.StandardOutput.ReadLineAsync() }
            $remaining = [int]($deadline - [datetime]::UtcNow).TotalMilliseconds
            if ($remaining -le 0 -or -not $script:stdoutReadTask.Wait($remaining)) { break }
            $line = $script:stdoutReadTask.Result
            $script:stdoutReadTask = $null
            if ($null -eq $line) { throw 'Windows daemon closed stdout.' }
            [void]$capturedLines.Add($line)
            if (-not [string]::IsNullOrWhiteSpace($line)) { return $line | ConvertFrom-Json -ErrorAction Stop }
        }
        throw 'Timed out waiting for a Windows daemon message.'
    }

    function Invoke-DaemonRpc([string]$module, [string]$method, [hashtable]$params, [int]$timeoutSeconds = 20) {
        $script:requestNumber++
        $id = "scroll-test-$script:requestNumber"
        $request = @{ id = $id; module = $module; method = $method; params = $params }
        $daemonProcess.StandardInput.WriteLine(($request | ConvertTo-Json -Compress -Depth 8))
        $daemonProcess.StandardInput.Flush()
        $deadline = [datetime]::UtcNow.AddSeconds($timeoutSeconds)
        while ([datetime]::UtcNow -lt $deadline) {
            $message = Read-DaemonLine $deadline
            if ($message.id -ne $id) {
                if ($message.event) {
                    $pendingEvents.Enqueue($message)
                    $observedEvents.Add($message)
                }
                continue
            }
            if (-not $message.success) { throw "Daemon $module.$method failed: $($message.error.code): $($message.error.message)" }
            return $message.result
        }
        throw "Timed out waiting for RPC $module.$method."
    }

    function Wait-ScrollEnd([int]$timeoutSeconds = 20) {
        $deadline = [datetime]::UtcNow.AddSeconds($timeoutSeconds)
        while ([datetime]::UtcNow -lt $deadline) {
            $message = if ($pendingEvents.Count -gt 0) { $pendingEvents.Dequeue() } else { Read-DaemonLine $deadline }
            if ($message.event -eq 'scroll-capture:scroll-ended') { return $message.data }
            if ($message.event) { $observedEvents.Add($message) }
        }
        throw 'Timed out waiting for scroll-capture:scroll-ended.'
    }

    $null = Invoke-DaemonRpc 'system' 'ping' @{}
    $startResult = Invoke-DaemonRpc 'scroll-capture' 'start' @{
        x = $fixture.FixtureLeft
        y = $fixture.FixtureTop
        width = $width
        height = $height
        scaleFactor = $scaleFactor
        autoScrollSpeed = 'slow'
        maxHeight = $expectedLogicalHeight
        nativeControls = $true
    }
    if (-not $startResult.started) { throw 'Scroll capture did not start.' }
    $null = Invoke-DaemonRpc 'scroll-capture' 'startAutoScroll' @{}
    if (-not $fixture.WaitForWheelCount($totalWheelDetents, 10000)) {
        throw "Fixture acknowledged $($fixture.WheelCount) wheel detents; expected $totalWheelDetents."
    }
    $end = Wait-ScrollEnd
    if ($fixture.FixtureError) { throw $fixture.FixtureError }
    if ($end.reason -ne 'max-height' -or $end.frameCount -ne $expectedFrameCount -or $end.estimatedHeight -ne $expectedLogicalHeight) {
        throw "Unexpected native scroll stop: $($end | ConvertTo-Json -Compress)."
    }
    $observedFrameCounts = @($observedEvents | Where-Object event -eq 'scroll-capture:frame-captured' | ForEach-Object { $_.data.frameCount })
    if ($observedFrameCounts -notcontains 2 -or $observedFrameCounts -notcontains 3) {
        throw "Expected intermediate frame events for counts 2 and 3; observed $($observedFrameCounts -join ',')."
    }
    if ($fixture.WheelCount -ne $totalWheelDetents -or $fixture.ScrollOffset -ne $totalMovement) {
        throw "Fixture ended at wheelCount=$($fixture.WheelCount), offset=$($fixture.ScrollOffset); expected $totalWheelDetents and $totalMovement."
    }
    $finish = Invoke-DaemonRpc 'scroll-capture' 'finish' @{ outputPath = $outputPath }
    if (-not $finish.success -or $finish.frameCount -ne $expectedFrameCount -or $finish.width -ne $width -or $finish.height -ne $expectedHeight) {
        throw "Unexpected stitched result: $($finish | ConvertTo-Json -Compress)."
    }
    [PoratakeScrollFixture]::AssertStitchedPng($outputPath, $bodyRows, $fixture.WheelCount, $wheelDetents)
    Copy-Item -LiteralPath $outputPath -Destination $verifiedOutputPath
    Write-Output "Hosted Windows native scroll capture passed; evidence: $evidenceRoot"
} finally {
    if ($daemonProcess -and -not $daemonProcess.HasExited) {
        try {
            $script:requestNumber++
            $quit = @{ id = "scroll-test-$script:requestNumber"; module = 'system'; method = 'quit'; params = @{} }
            $daemonProcess.StandardInput.WriteLine(($quit | ConvertTo-Json -Compress -Depth 8))
            $daemonProcess.StandardInput.Flush()
            if (-not $daemonProcess.WaitForExit(5000)) { $daemonProcess.Kill($true); $daemonProcess.WaitForExit() }
        } catch {
            if (-not $daemonProcess.HasExited) { $daemonProcess.Kill($true); $daemonProcess.WaitForExit() }
        }
        $daemonProcess.Dispose()
    }
    if ($fixture) {
        $fixtureEvidence = @{
            roi = @{ x = $fixture.FixtureLeft; y = $fixture.FixtureTop; width = $width; height = $height }
            wheelCount = $fixture.WheelCount
            scrollOffset = $fixture.ScrollOffset
            scaleFactor = $fixture.ScaleFactor
            stop = $end
            result = $finish
            expected = @{ width = $width; height = $expectedHeight; frameCount = $expectedFrameCount; fixedHeaderIncluded = $false; bodyRows = $bodyRows; footerHeight = $footerHeight; wheelDetents = $wheelDetents; totalWheelDetents = $totalWheelDetents }
            events = @($observedEvents)
        }
    }
    [System.IO.File]::WriteAllLines((Join-Path $evidenceRoot 'daemon-stdout.jsonl'), $capturedLines)
    if ($fixtureEvidence) { [System.IO.File]::WriteAllText((Join-Path $evidenceRoot 'fixture.json'), ($fixtureEvidence | ConvertTo-Json -Depth 8)) }
    if ($stderrReadTask) {
        $null = $stderrReadTask.Wait(5000)
        if ($stderrReadTask.IsCompleted) { [System.IO.File]::WriteAllText((Join-Path $evidenceRoot 'daemon-stderr.log'), $stderrReadTask.Result) }
    }
    if ($fixture) { $fixture.CloseFixture() }
}
