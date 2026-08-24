# Captures every visible top-level window of a process to PNG files.
#
# The GPUI shell's popovers and overlays are created without a title, and
# window-enumeration tools that list "targetable" windows skip those, so they
# cannot be screenshotted the usual way. This walks the process's own windows
# instead and BitBlts each one. It is a passive read: no clicks, no keystrokes,
# no focus change.
#
#   pwsh -File scripts/parity/capture-window.ps1 -ProcessId 1234 -OutDir C:\tmp
param(
    [Parameter(Mandatory = $true)][int]$ProcessId,
    [string]$OutDir = $env:TEMP,
    [int]$MinWidth = 120,
    [int]$MinHeight = 80
)

Add-Type -AssemblyName System.Drawing

# Saving fails with an unhelpful COM error if the directory is absent.
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

$signature = @'
using System;
using System.Text;
using System.Runtime.InteropServices;

public class ParityWin {
    public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr p);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
if (-not ('ParityWin' -as [type])) { Add-Type -TypeDefinition $signature }

# DWMWA_EXTENDED_FRAME_BOUNDS: the visible frame, without the invisible resize
# border GetWindowRect includes.
$EXTENDED_FRAME_BOUNDS = 9

$handles = New-Object System.Collections.ArrayList
$callback = {
    param($handle, $lparam)
    $owner = 0
    [ParityWin]::GetWindowThreadProcessId($handle, [ref]$owner) | Out-Null
    if ($owner -eq $ProcessId -and [ParityWin]::IsWindowVisible($handle)) {
        $handles.Add($handle) | Out-Null
    }
    return $true
}
[ParityWin]::EnumWindows($callback, [IntPtr]::Zero) | Out-Null

$index = 0
foreach ($handle in $handles) {
    $rect = New-Object ParityWin+RECT
    if ([ParityWin]::DwmGetWindowAttribute($handle, $EXTENDED_FRAME_BOUNDS, [ref]$rect, 16) -ne 0) {
        [ParityWin]::GetWindowRect($handle, [ref]$rect) | Out-Null
    }
    # `PrintWindow` draws the *client* area at the origin of our DC, so the
    # bitmap has to be the client size. Sizing it to the DWM frame bounds
    # instead shifts the content and crops both ends -- which reads as missing
    # buttons rather than as a capture bug.
    $client = New-Object ParityWin+RECT
    if ([ParityWin]::GetClientRect($handle, [ref]$client) -and $client.Right -gt 0) {
        $width = $client.Right - $client.Left
        $height = $client.Bottom - $client.Top
    } else {
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
    }
    if ($width -lt $MinWidth -or $height -lt $MinHeight) { continue }

    $title = New-Object System.Text.StringBuilder 256
    [ParityWin]::GetWindowTextW($handle, $title, 256) | Out-Null

    $bitmap = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    # PW_RENDERFULLCONTENT asks the window to redraw itself into our DC, so an
    # occluded window still captures. Reading the screen instead would grab
    # whatever is on top -- a fullscreen app, another window.
    $hdc = $graphics.GetHdc()
    $printed = [ParityWin]::PrintWindow($handle, $hdc, 2)
    $graphics.ReleaseHdc($hdc)
    if (-not $printed) {
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
    }
    $path = Join-Path $OutDir ("parity-window-$index.png")
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()

    [pscustomobject]@{
        Index = $index
        Handle = $handle
        Title = $title.ToString()
        Width = $width
        Height = $height
        Path = $path
    }
    $index++
}
