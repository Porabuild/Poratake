$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$daemonDir = Join-Path $projectRoot 'src\main\daemon-win'
$outputFile = Join-Path $projectRoot 'src\main\daemon\capty-daemon.exe'

Write-Host 'Building capty-daemon for Windows...' -ForegroundColor Yellow

if ((Test-Path -LiteralPath $outputFile) -and $env:CI) {
    Write-Host 'capty-daemon.exe already built (CI), skipping.'
    exit 0
}

cargo build --release --manifest-path (Join-Path $daemonDir 'Cargo.toml')
if ($LASTEXITCODE -ne 0) {
    Write-Host 'Error: cargo build failed' -ForegroundColor Red
    exit 1
}

Copy-Item (Join-Path $daemonDir 'target\release\capty-daemon.exe') $outputFile -Force

Write-Host "Successfully built: $outputFile" -ForegroundColor Green
