param()

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$versionText = (Get-Content -Raw -LiteralPath (Join-Path $projectRoot "VERSION.txt")).Trim()
$version = $versionText -replace '^DiCAR LAB v', ''

if ($version -notmatch '^\d+\.\d+\.\d+$') {
    throw "VERSION.txt 格式无效：$versionText"
}

$venvPython = Join-Path $projectRoot "CAR_LAB\.venv\Scripts\python.exe"
$python = if (Test-Path -LiteralPath $venvPython) {
    $venvPython
} else {
    (Get-Command python -ErrorAction Stop).Source
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$buildRoot = Join-Path $projectRoot "build\portable-$stamp"
$distPath = Join-Path $buildRoot "dist"
$workPath = Join-Path $buildRoot "work"
$releasePath = Join-Path $projectRoot "release"
$archiveName = "DiCAR-LAB-v$version-Windows-x64.zip"
$archivePath = Join-Path $releasePath $archiveName
$checksumPath = Join-Path $releasePath "SHA256SUMS.txt"

foreach ($output in ($archivePath, $checksumPath)) {
    if (Test-Path -LiteralPath $output) {
        throw "发布输出已存在，未覆盖：$output"
    }
}

New-Item -ItemType Directory -Path $distPath -Force | Out-Null
New-Item -ItemType Directory -Path $workPath -Force | Out-Null
New-Item -ItemType Directory -Path $releasePath -Force | Out-Null

$previousPath = $env:Path
$previousQtPlatform = $env:QT_QPA_PLATFORM
$previousSmokeFlag = $env:DICAR_SMOKE_TEST
$env:Path = @(
    (Split-Path -Parent $python),
    "$env:SystemRoot\System32",
    "$env:SystemRoot",
    "$env:SystemRoot\System32\Wbem",
    "$env:SystemRoot\System32\WindowsPowerShell\v1.0"
) -join ";"

try {
    Push-Location $projectRoot
    try {
        & $python -m PyInstaller "dicar_lab.spec" --distpath $distPath --workpath $workPath
        if ($LASTEXITCODE -ne 0) {
            throw "PyInstaller 构建失败，退出码：$LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $appPath = Join-Path $distPath "DiCAR LAB"
    $exePath = Join-Path $appPath "DiCAR LAB.exe"
    if (!(Test-Path -LiteralPath $exePath)) {
        throw "构建完成但未找到程序：$exePath"
    }

    $env:QT_QPA_PLATFORM = "offscreen"
    $env:DICAR_SMOKE_TEST = "1"
    & $exePath
    if ($LASTEXITCODE -ne 0) {
        throw "便携版启动冒烟失败，退出码：$LASTEXITCODE"
    }
} finally {
    $env:Path = $previousPath
    $env:QT_QPA_PLATFORM = $previousQtPlatform
    $env:DICAR_SMOKE_TEST = $previousSmokeFlag
}

Copy-Item -LiteralPath (Join-Path $projectRoot "LICENSE") -Destination $appPath
Copy-Item -LiteralPath (Join-Path $projectRoot "README.md") -Destination $appPath
Compress-Archive -LiteralPath $appPath -DestinationPath $archivePath

$hash = (Get-FileHash -LiteralPath $archivePath -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $archiveName" | Set-Content -LiteralPath $checksumPath -Encoding ascii

Write-Host "Portable release created:"
Write-Host $archivePath
Write-Host $checksumPath
