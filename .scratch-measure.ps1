param(
  [int]$Runs = 2,
  [int]$Fresh = 0
)

$ErrorActionPreference = 'Continue'

Add-Type -AssemblyName System.Drawing

$marker = Join-Path $env:TEMP 'poratake-aio-trigger'
$ack = Join-Path $env:TEMP 'poratake-aio-ack'

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

function Trigger([string]$command) {
  Remove-Item $ack -Force -ErrorAction SilentlyContinue
  'triggered' | Set-Content -Path $marker
  if ($command -eq 'esc') { 'esc' | Set-Content -Path $marker }
}

function MeasureOnce([string]$label) {
  $cx = 3440 / 2
  $base = Snap ($cx - 350) 40 700 260
  Start-Sleep -Milliseconds 300
  $base = Snap ($cx - 350) 40 700 260

  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  Trigger 'aio'

  $ackMs = -1
  while ($sw.ElapsedMilliseconds -lt 3000) {
    if (Test-Path $ack) { $ackMs = $sw.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 5
  }

  $found = -1
  while ($sw.ElapsedMilliseconds -lt 8000) {
    $cur = Snap ($cx - 350) 40 700 260
    if ((DiffCount $base $cur) -gt 25) { $found = $sw.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 25
  }

  Write-Output "$label : ack=$ackMs ms visible=$found ms"
  Start-Sleep -Milliseconds 800
  Trigger 'esc'
  Start-Sleep -Milliseconds 1500
}

if ($Fresh -eq 1) {
  Stop-Process -Name Poratake -Force -ErrorAction SilentlyContinue
  Start-Sleep -Seconds 3
  $env:PORATAKE_AIO_TRIGGER = '1'
  Start-Process 'C:\Users\sdsle\AppData\Local\Programs\Poratake\Poratake.exe'
  Start-Sleep -Seconds 14
  MeasureOnce 'cold(first-after-launch)'
}

for ($r = 1; $r -le $Runs; $r++) {
  MeasureOnce "warm-$r"
}
