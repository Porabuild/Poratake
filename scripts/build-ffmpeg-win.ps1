$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$buildScript = Join-Path $scriptDir 'build-ffmpeg-win.sh'
$bashCandidates = @()
if ($env:PORATAKE_MSYS2_ROOT) {
    $bashCandidates += Join-Path $env:PORATAKE_MSYS2_ROOT 'usr\bin\bash.exe'
}
$bashCandidates += @(
    'C:\msys64\usr\bin\bash.exe'
    'C:\tools\msys64\usr\bin\bash.exe'
)
$bash = $bashCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1

if (-not $bash) {
    throw 'MSYS2 is required to build FFmpeg. Install UCRT64 gcc (x64) or CLANGARM64 clang (arm64) plus make, nasm, pkg-config, curl, and tar.'
}

$hostArch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$arch = if ($env:PORATAKE_WIN_ARCH) { $env:PORATAKE_WIN_ARCH } else { $hostArch }
if ($arch -ne 'x64' -and $arch -ne 'arm64') {
    throw "Unsupported PORATAKE_WIN_ARCH: $arch"
}

if ($arch -eq 'arm64') {
    $env:MSYSTEM = 'CLANGARM64'
    $mingwPath = '/clangarm64/bin'
} else {
    $env:MSYSTEM = 'UCRT64'
    $mingwPath = '/ucrt64/bin'
}

$env:CHERE_INVOKING = '1'
$env:PORATAKE_PROJECT_ROOT = $projectRoot
$env:PORATAKE_FFMPEG_BUILD_SCRIPT = $buildScript
$env:PORATAKE_WIN_ARCH = $arch
$command = "export PATH=$mingwPath" + ':/usr/bin:$PATH; cd "$(cygpath -u "$PORATAKE_PROJECT_ROOT")"; bash "$(cygpath -u "$PORATAKE_FFMPEG_BUILD_SCRIPT")"'
$commandPath = Join-Path ([System.IO.Path]::GetTempPath()) ("poratake-ffmpeg-" + [guid]::NewGuid().ToString('N') + '.sh')
[System.IO.File]::WriteAllText($commandPath, $command + "`n", [System.Text.UTF8Encoding]::new($false))

try {
    & $bash $commandPath
    if ($LASTEXITCODE -ne 0) {
        throw "FFmpeg build failed with exit code $LASTEXITCODE"
    }
} finally {
    Remove-Item -LiteralPath $commandPath -Force
}
