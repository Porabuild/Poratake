param(
  [int]$Runs = 2,
  [int]$Fresh = 0
)

Add-Type -AssemblyName System.Drawing
Add-Type '
using System;
using System.Runtime.InteropServices;
public static class KbM {
  [DllImport("user32.dll")] public static extern uint SendInput(uint n, INPUT[] pInputs, int cbSize);
  [StructLayout(LayoutKind.Sequential)] public struct INPUT { public uint type; public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  public static void Scan(ushort scan, bool up) {
    INPUT[] i = new INPUT[1];
    i[0].type = 1; i[0].wScan = scan; i[0].dwFlags = 0x0008 | (up ? 0x0002u : 0u);
    SendInput(1, i, Marshal.SizeOf(typeof(INPUT)));
  }
}
'

function Snap([int]$x, [int]$y, [int]$w, [int]$h) {
  $b = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.CopyFromScreen($x, $y, 0, 0, $b.Size)
  $g.Dispose(); $b
}

function DiffCount($a, $b) {
  $rect = [System.Drawing.Rectangle]::FromLTRB(0, 0, $a.Width, $a.Height)
  $da = $a.LockBits($rect, 'ReadOnly', $a.PixelFormat)
  $db = $b.LockBits($rect, 'ReadOnly', $b.PixelFormat)
  $bytes = [Math]::Abs($da.Stride) * $a.Height
  $ma = New-Object byte[] $bytes; $mb = New-Object byte[] $bytes
  [System.Runtime.InteropServices.Marshal]::Copy($da.Scan0, $ma, 0, $bytes)
  [System.Runtime.InteropServices.Marshal]::Copy($db.Scan0, $mb, 0, $bytes)
  $a.UnlockBits($da); $b.UnlockBits($db)
  $diff = 0
  for ($i = 0; $i -lt $bytes; $i += 499) { if ($ma[$i] -ne $mb[$i]) { $diff++ } }
  $diff
}

function Trigger {
  [KbM]::Scan(0x38, $false); [KbM]::Scan(0x2A, $false); [KbM]::Scan(0x1F, $false)
  Start-Sleep -Milliseconds 40
  [KbM]::Scan(0x1F, $true); [KbM]::Scan(0x2A, $true); [KbM]::Scan(0x38, $true)
}

function Cancel {
  [KbM]::Scan(0x01, $false); Start-Sleep -Milliseconds 25; [KbM]::Scan(0x01, $true)
  Start-Sleep -Milliseconds 60
  [KbM]::Scan(0x01, $false); Start-Sleep -Milliseconds 25; [KbM]::Scan(0x01, $true)
}

function MeasureOnce([string]$label) {
  $cx = 3440 / 2
  $base = Snap ($cx - 350) 40 700 260
  Start-Sleep -Milliseconds 300
  $base = Snap ($cx - 350) 40 700 260
  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  Trigger
  $found = -1
  while ($sw.ElapsedMilliseconds -lt 6000) {
    $cur = Snap ($cx - 350) 40 700 260
    if ((DiffCount $base $cur) -gt 25) { $found = $sw.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 25
  }
  Write-Output "$label : $found ms"
  Start-Sleep -Milliseconds 800
  Cancel
  Start-Sleep -Milliseconds 1500
  $found
}

if ($Fresh -eq 1) {
  Stop-Process -Name Poratake -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 3
  Start-Process 'C:\Users\sdsle\AppData\Local\Programs\Poratake\Poratake.exe'
  Start-Sleep -Seconds 14
  MeasureOnce 'cold(first-after-launch)'
}

for ($r = 1; $r -le $Runs; $r++) {
  MeasureOnce "warm-$r"
}
