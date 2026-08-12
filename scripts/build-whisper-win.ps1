$ErrorActionPreference = 'Stop'

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$projectRoot = Split-Path -Parent $scriptDir
$outputDir = Join-Path $projectRoot 'src\main\binaries\whisper'
$outputPath = Join-Path $outputDir 'whisper.exe'
$tempRoot = [System.IO.Path]::GetFullPath([System.IO.Path]::GetTempPath())
$buildRoot = Join-Path $tempRoot ("poratake-whisper-" + [guid]::NewGuid().ToString('N'))
$sourceDir = Join-Path $buildRoot 'whisper.cpp'
$buildDir = Join-Path $sourceDir 'build'
$whisperCommit = '2eeeba56e9edd762b4b38467bab96c2517163158'

if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    throw 'git is required to build whisper.cpp'
}

if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    throw 'CMake with the Visual Studio 2022 C++ toolchain is required to build whisper.cpp'
}

New-Item -ItemType Directory -Path $buildRoot | Out-Null

try {
    git init $sourceDir
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to initialize the whisper.cpp source directory'
    }
    git -C $sourceDir remote add origin https://github.com/ggml-org/whisper.cpp.git
    git -C $sourceDir fetch --depth 1 origin $whisperCommit
    git -C $sourceDir checkout --detach FETCH_HEAD
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to fetch the pinned whisper.cpp commit'
    }

    cmake -S $sourceDir -B $buildDir -A x64 -DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded -DBUILD_SHARED_LIBS=OFF -DGGML_NATIVE=OFF -DWHISPER_BUILD_TESTS=OFF -DWHISPER_BUILD_EXAMPLES=ON
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to configure whisper.cpp'
    }

    cmake --build $buildDir --config Release --parallel
    if ($LASTEXITCODE -ne 0) {
        throw 'Failed to build whisper.cpp'
    }

    $builtCli = Get-ChildItem -Path $buildDir -Recurse -File -Filter whisper-cli.exe |
        Where-Object { $_.FullName -match '[\\/]Release[\\/]' } |
        Select-Object -First 1

    if (-not $builtCli) {
        throw 'whisper-cli.exe was not produced'
    }

    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
    Copy-Item -LiteralPath $builtCli.FullName -Destination $outputPath -Force

    $helpOutput = (& $outputPath --help 2>&1 | Out-String)
    if (-not $helpOutput.Contains('-dtw')) {
        throw 'whisper-cli.exe does not support DTW timestamps'
    }
    if (-not $helpOutput.Contains('-ojf')) {
        throw 'whisper-cli.exe does not support JSON full output'
    }

    Write-Host "Built $outputPath"
} finally {
    $resolvedBuildRoot = [System.IO.Path]::GetFullPath($buildRoot)
    if ($resolvedBuildRoot.StartsWith($tempRoot, [System.StringComparison]::OrdinalIgnoreCase) -and (Test-Path -LiteralPath $resolvedBuildRoot)) {
        Remove-Item -LiteralPath $resolvedBuildRoot -Recurse -Force
    }
}
