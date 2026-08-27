$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$daemonDir = Join-Path $projectRoot 'src\main\daemon-win'
$outputFile = Join-Path $projectRoot 'src\main\daemon\poratake-daemon.exe'
$targetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $projectRoot 'src\main\target' }
$hostArch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x64' }
$arch = if ($env:PORATAKE_WIN_ARCH) { $env:PORATAKE_WIN_ARCH } else { $hostArch }

if ($arch -ne 'x64' -and $arch -ne 'arm64') {
    throw "Unsupported PORATAKE_WIN_ARCH: $arch"
}

$rustTarget = if ($arch -eq 'arm64') { 'aarch64-pc-windows-msvc' } else { 'x86_64-pc-windows-msvc' }

Write-Host "Building poratake-daemon for Windows $arch..." -ForegroundColor Yellow

if ((Test-Path -LiteralPath $outputFile) -and $env:CI) {
    Write-Host 'poratake-daemon.exe already built (CI), skipping.'
    exit 0
}

rustup target add $rustTarget
if ($LASTEXITCODE -ne 0) {
    Write-Host 'Error: rustup target add failed' -ForegroundColor Red
    exit 1
}

cargo build --release --target $rustTarget --manifest-path (Join-Path $daemonDir 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    Write-Host 'Error: cargo build failed' -ForegroundColor Red
    exit 1
}

Copy-Item (Join-Path $targetDir "$rustTarget\release\poratake-daemon.exe") $outputFile -Force

Write-Host "Successfully built: $outputFile" -ForegroundColor Green
