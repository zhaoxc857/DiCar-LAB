param()

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$versionText = (Get-Content -Raw -LiteralPath (Join-Path $projectRoot "VERSION.txt")).Trim()
$version = $versionText -replace '^DiCAR LAB v', ''

if ($version -notmatch '^\d+\.\d+\.\d+$') {
    throw "VERSION.txt 格式无效：$versionText"
}

$venvPython = Join-Path $projectRoot "CAR_LAB\.venv\Scripts\python.exe"
$rootVenvPython = Join-Path $projectRoot ".venv\Scripts\python.exe"
$python = if (Test-Path -LiteralPath $venvPython) {
    $venvPython
} elseif (Test-Path -LiteralPath $rootVenvPython) {
    $rootVenvPython
} else {
    (Get-Command python -ErrorAction Stop).Source
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$buildRoot = Join-Path $projectRoot "build\onefile-$stamp"
$distPath = Join-Path $buildRoot "dist"
$workPath = Join-Path $buildRoot "work"
$releasePath = Join-Path $projectRoot "release"
$exeName = "DiCAR LAB.exe"
$artifactName = "DiCAR-LAB-v$version-Windows-x64-onefile.exe"
$artifactPath = Join-Path $releasePath $artifactName
$checksumPath = Join-Path $releasePath "SHA256SUMS-v$version-onefile.txt"

foreach ($output in ($artifactPath, $checksumPath)) {
    if (Test-Path -LiteralPath $output) {
        throw "发布输出已存在，未覆盖：$output"
    }
}

New-Item -ItemType Directory -Path $distPath -Force | Out-Null
New-Item -ItemType Directory -Path $workPath -Force | Out-Null
New-Item -ItemType Directory -Path $releasePath -Force | Out-Null

# Restrict PATH to the build interpreter and Windows system directories so
# host toolchain DLLs (ucrtbase, api-ms-win-*) never leak into the bundle.
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
        & $python -m PyInstaller "dicar_lab_onefile.spec" --distpath $distPath --workpath $workPath
        if ($LASTEXITCODE -ne 0) {
            throw "PyInstaller 构建失败，退出码：$LASTEXITCODE"
        }
    } finally {
        Pop-Location
    }

    $exePath = Join-Path $distPath $exeName
    if (!(Test-Path -LiteralPath $exePath)) {
        throw "构建完成但未找到程序：$exePath"
    }

    # One-file startup extracts the bundle first, so allow a generous timeout.
    $env:QT_QPA_PLATFORM = "offscreen"
    $env:DICAR_SMOKE_TEST = "1"
    $smoke = Start-Process -FilePath $exePath -Wait -PassThru
    if ($smoke.ExitCode -ne 0) {
        throw "单文件版启动冒烟失败，退出码：$($smoke.ExitCode)"
    }
} finally {
    $env:Path = $previousPath
    $env:QT_QPA_PLATFORM = $previousQtPlatform
    $env:DICAR_SMOKE_TEST = $previousSmokeFlag
}

Copy-Item -LiteralPath $exePath -Destination $artifactPath
$hash = (Get-FileHash -LiteralPath $artifactPath -Algorithm SHA256).Hash.ToLowerInvariant()
"$hash  $artifactName" | Set-Content -LiteralPath $checksumPath -Encoding ascii

Write-Host "One-file release created:"
Write-Host $artifactPath
Write-Host $checksumPath
