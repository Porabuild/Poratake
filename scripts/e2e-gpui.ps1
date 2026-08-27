param(
    [string]$Mode = "capture"
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms,System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class PT {
  public delegate bool EnumProc(IntPtr hwnd, IntPtr lparam);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lparam);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  public struct RECT { public int Left, Top, Right, Bottom; }
  public static List<IntPtr> WindowsOf(uint pid) {
    var list = new List<IntPtr>();
    EnumWindows((h, lp) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid && IsWindowVisible(h)) list.Add(h);
      return true;
    }, IntPtr.Zero);
    return list;
  }
  public static void Click(int x, int y) {
    SetCursorPos(x, y); System.Threading.Thread.Sleep(80);
    mouse_event(2,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(60);
    mouse_event(4,0,0,0,UIntPtr.Zero); System.Threading.Thread.Sleep(150);
  }
  public static void HotkeyAltShift4() {
    keybd_event(0x12,0,0,UIntPtr.Zero);
    keybd_event(0x10,0,0,UIntPtr.Zero);
    System.Threading.Thread.Sleep(80);
    keybd_event(0x34,0,0,UIntPtr.Zero);
    System.Threading.Thread.Sleep(60);
    keybd_event(0x34,0,2,UIntPtr.Zero);
    keybd_event(0x10,0,2,UIntPtr.Zero);
    keybd_event(0x12,0,2,UIntPtr.Zero);
  }
}
'@

$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $PSScriptRoot '..\src\main\target' }
$exe = Join-Path $targetDir 'debug\poratake-gpui.exe'
# Always rebuild so the binary matches the sources.
Get-Process poratake-gpui,poratake-daemon -ErrorAction SilentlyContinue | Stop-Process -Force
Remove-Item $exe -ErrorAction SilentlyContinue
Push-Location (Join-Path $PSScriptRoot '..\src\main\app-gpui')
& cargo build
if ($LASTEXITCODE -ne 0) { throw "cargo build failed with exit code $LASTEXITCODE" }
Pop-Location
if (-not (Test-Path $exe)) { throw 'build failed: binary missing' }
$stamp = Get-Date -Format "yyyyMMddHHmmss"
$outLog = "$env:TEMP\opencode\pt-$stamp-out.log"
$traceLog = "$env:TEMP\opencode\pt-$stamp-daemon.log"
$errLog = "$env:TEMP\opencode\pt-$stamp-err.log"
$env:PORATAKE_TRACE_FILE = $traceLog
Remove-Item $outLog, $errLog -ErrorAction SilentlyContinue
$proc = Start-Process -FilePath $exe -RedirectStandardOutput $outLog -RedirectStandardError $errLog -PassThru
Start-Sleep -Seconds 3

function Show-Windows($label) {
    Write-Host "== $label =="
    foreach ($h in [PT]::WindowsOf($proc.Id)) {
        $r = New-Object PT+RECT
        [PT]::GetWindowRect($h, [ref]$r) | Out-Null
        Write-Host ("  {0} -> {1},{2} {3}x{4}" -f $h, $r.Left, $r.Top, ($r.Right-$r.Left), ($r.Bottom-$r.Top))
    }
}

switch ($Mode) {
    "capture" {
        Show-Windows "before hotkey"
        [PT]::HotkeyAltShift4()
        Start-Sleep -Seconds 2
        Show-Windows "after hotkey (overlay expected)"

        # Drag a selection across the middle of the screen.
        [PT]::SetCursorPos(500, 400) | Out-Null; Start-Sleep -Milliseconds 150
        [PT]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
        for ($i = 1; $i -le 15; $i++) {
            [PT]::SetCursorPos(500 + $i * 30, 400 + $i * 12) | Out-Null
            Start-Sleep -Milliseconds 30
        }
        [PT]::mouse_event(4,0,0,0,[UIntPtr]::Zero)
        Start-Sleep -Seconds 36
        Show-Windows "after selection (editor expected)"
    }
    "settings" {
        Show-Windows "before"
        foreach ($h in [PT]::WindowsOf($proc.Id)) {
            [PT]::SetForegroundWindow($h) | Out-Null
            break
        }
        # Click the Settings button (bottom-left area of the shell window).
        $wins = [PT]::WindowsOf($proc.Id)
        if ($wins.Count -gt 0) {
            $r = New-Object PT+RECT
            [PT]::GetWindowRect($wins[0], [ref]$r) | Out-Null
            [PT]::Click($r.Left + 90, $r.Bottom - 60)
        }
        Start-Sleep -Seconds 2
        Show-Windows "after settings click"
    }
}

Start-Sleep -Milliseconds 300
Write-Host "--- stdout ---"; Get-Content $outLog -ErrorAction SilentlyContinue
Write-Host "--- stderr ---"; Get-Content $errLog -ErrorAction SilentlyContinue
Write-Host "--- daemon trace ---"; Get-Content $traceLog -ErrorAction SilentlyContinue
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
