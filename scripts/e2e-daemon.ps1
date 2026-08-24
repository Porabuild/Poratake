$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
Get-Process poratake-daemon,poratake-gpui -ErrorAction SilentlyContinue | Stop-Process -Force

$stamp = Get-Date -Format "yyyyMMddHHmmss"
$traceLog = "$env:TEMP\opencode\daemon-trace-$stamp.log"
$outputPath = "$env:TEMP\opencode\manual-shot.png"
New-Item -ItemType Directory -Force (Split-Path $outputPath) | Out-Null
Remove-Item $outputPath -ErrorAction SilentlyContinue
$env:PORATAKE_TRACE_FILE = $traceLog

# Host the daemon exactly like the shells do: keep stdin open for its whole
# lifetime, send requests over it, then quit.
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = (Resolve-Path (Join-Path $PSScriptRoot '..\src\main\daemon\poratake-daemon.exe')).Path
$psi.RedirectStandardInput = $true
$psi.RedirectStandardOutput = $true
$psi.UseShellExecute = $false
$proc = [System.Diagnostics.Process]::Start($psi)
Start-Sleep -Milliseconds 800

$request = @{
    id = 't1'
    module = 'screenshot'
    method = 'capture-area'
    params = @{ x = 20; y = 20; width = 300; height = 200; path = $outputPath; cached = $false }
} | ConvertTo-Json -Compress -Depth 4
$proc.StandardInput.WriteLine($request)
Start-Sleep -Seconds 12

"STDOUT:"
try { while (-not $proc.StandardOutput.EndOfStream) { $proc.StandardOutput.ReadLine() } } catch {}
"--- TRACE ($traceLog) ---"
Get-Content $traceLog
"PNG bytes:"
(Get-Item $outputPath).Length

if (-not $proc.HasExited) {
    try {
        $proc.StandardInput.WriteLine('{"id":"bye","module":"system","method":"quit"}')
        Start-Sleep -Milliseconds 800
    } catch {}
    if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force }
}
