$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$buildScript = Join-Path $scriptDir 'build-ffmpeg-win.sh'
$bashCandidates = @()
if ($env:CAPTY_MSYS2_ROOT) {
    $bashCandidates += Join-Path $env:CAPTY_MSYS2_ROOT 'usr\bin\bash.exe'
}
$bashCandidates += @(
    'C:\msys64\usr\bin\bash.exe'
    'C:\tools\msys64\usr\bin\bash.exe'
)
$bash = $bashCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

if (-not $bash) {
    throw 'MSYS2 UCRT64 is required to build FFmpeg. Install MSYS2 with the UCRT64 gcc, make, nasm, pkg-config, curl, and tar packages.'
}

$env:MSYSTEM = 'UCRT64'
$env:CHERE_INVOKING = '1'
$env:CAPTY_PROJECT_ROOT = $projectRoot
$env:CAPTY_FFMPEG_BUILD_SCRIPT = $buildScript
$command = 'export PATH=/ucrt64/bin:/usr/bin:$PATH; cd "$(cygpath -u "$CAPTY_PROJECT_ROOT")"; bash "$(cygpath -u "$CAPTY_FFMPEG_BUILD_SCRIPT")"'

$processInfo = [System.Diagnostics.ProcessStartInfo]::new()
$processInfo.FileName = $bash
$processInfo.UseShellExecute = $false
$processInfo.Arguments = '--login -s'
$processInfo.RedirectStandardInput = $true
$processInfo.StandardInputEncoding = [System.Text.UTF8Encoding]::new($false)
$process = [System.Diagnostics.Process]::Start($processInfo)
$process.StandardInput.WriteLine($command)
$process.StandardInput.Close()
$process.WaitForExit()

if ($process.ExitCode -ne 0) {
    throw "FFmpeg build failed with exit code $($process.ExitCode)"
}
