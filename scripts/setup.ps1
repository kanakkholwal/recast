<#
.SYNOPSIS
  One-shot local development setup for the Recast desktop app (Windows).

.DESCRIPTION
  Bootstraps a fresh machine so a contributor can build and run the Tauri
  desktop app. It:
    1. Detects the OS architecture and Rust target triple.
    2. Auto-installs missing toolchains via winget: Node.js LTS, Rust
       (rustup), and the Visual Studio C++ build tools Tauri needs.
    3. Enables pnpm through corepack (pinned to package.json#packageManager).
    4. Downloads the FFmpeg + ffprobe sidecar binaries and drops them into
       apps/desktop/src-tauri/binaries/ with Tauri's target-triple names.
    5. Runs `pnpm install` for the whole workspace.
    6. Produces a debug build of the desktop app to verify the toolchain.

.PARAMETER SkipBuild
  Stop after `pnpm install` + FFmpeg download; skip the debug build.

.PARAMETER SkipToolchains
  Don't auto-install Node/Rust/VS build tools - only verify they exist.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/setup.ps1
#>
[CmdletBinding()]
param(
  [switch]$SkipBuild,
  [switch]$SkipToolchains
)

$ErrorActionPreference = 'Stop'

# --- helpers ----------------------------------------------------------------

function Write-Step  ($m) { Write-Host "`n==> $m" -ForegroundColor Cyan }
function Write-Ok    ($m) { Write-Host "    OK  $m" -ForegroundColor Green }
function Write-Info  ($m) { Write-Host "    $m" -ForegroundColor Gray }
function Write-Warn  ($m) { Write-Host "    !   $m" -ForegroundColor Yellow }
function Fail        ($m) { Write-Host "`nFAILED: $m" -ForegroundColor Red; exit 1 }

function Test-Cmd ($name) {
  return [bool](Get-Command $name -ErrorAction SilentlyContinue)
}

# Re-read PATH from the registry so tools installed in this session are found.
function Sync-Path {
  $machine = [System.Environment]::GetEnvironmentVariable('Path', 'Machine')
  $user    = [System.Environment]::GetEnvironmentVariable('Path', 'User')
  $env:Path = ($machine, $user | Where-Object { $_ }) -join ';'
}

function Install-WingetPackage ($id, $label, $override) {
  Write-Info "Installing $label ..."
  $wingetArgs = @('install', '--id', $id, '--exact', '--silent',
                  '--accept-package-agreements', '--accept-source-agreements')
  if ($override) { $wingetArgs += @('--override', $override) }
  & winget @wingetArgs
  # winget exits 0 on success; -1978335189 means "already installed".
  if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne -1978335189) {
    Fail "winget failed to install $label (exit $LASTEXITCODE)."
  }
  Sync-Path
}

# --- locate repo root -------------------------------------------------------

$RepoRoot   = Split-Path -Parent $PSScriptRoot
$DesktopDir = Join-Path $RepoRoot 'apps\desktop'
$BinDir     = Join-Path $DesktopDir 'src-tauri\binaries'

if (-not (Test-Path (Join-Path $RepoRoot 'pnpm-workspace.yaml'))) {
  Fail "Could not locate the repo root from $PSScriptRoot."
}

Write-Host "Recast desktop - local setup (Windows)" -ForegroundColor White
Write-Info "Repo: $RepoRoot"

# --- 1. detect architecture / target triple --------------------------------

Write-Step "Detecting platform"
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
  'AMD64' { $triple = 'x86_64-pc-windows-msvc';  $ffArch = 'win64'   }
  'ARM64' { $triple = 'aarch64-pc-windows-msvc'; $ffArch = 'winarm64' }
  default { Fail "Unsupported architecture: $arch" }
}
Write-Ok "Windows / $arch  ->  target triple: $triple"

# --- 2. toolchains ----------------------------------------------------------

Write-Step "Checking toolchains"

if ($SkipToolchains) {
  Write-Warn "-SkipToolchains set; only verifying."
}

if (-not $SkipToolchains -and -not (Test-Cmd 'winget')) {
  Fail "winget is required for auto-install. Update 'App Installer' from the Microsoft Store, or re-run with -SkipToolchains after installing Node/Rust/VS Build Tools manually."
}

# Node.js
if (Test-Cmd 'node') {
  Write-Ok "Node.js $(node --version)"
} elseif ($SkipToolchains) {
  Fail "Node.js not found. Install Node.js LTS (v18+) from https://nodejs.org/."
} else {
  Install-WingetPackage 'OpenJS.NodeJS.LTS' 'Node.js LTS'
  if (-not (Test-Cmd 'node')) {
    Fail "Node.js installed but not on PATH. Close and reopen the terminal, then re-run."
  }
  Write-Ok "Node.js $(node --version)"
}

# Rust
if (Test-Cmd 'rustc') {
  Write-Ok "Rust $(rustc --version)"
} elseif ($SkipToolchains) {
  Fail "Rust not found. Install it from https://rustup.rs/."
} else {
  Install-WingetPackage 'Rustlang.Rustup' 'Rust (rustup)'
  $cargoBin = Join-Path $env:USERPROFILE '.cargo\bin'
  if (Test-Path $cargoBin) { $env:Path = "$cargoBin;$env:Path" }
  if (-not (Test-Cmd 'rustc')) {
    Fail "Rust installed but not on PATH. Close and reopen the terminal, then re-run."
  }
  Write-Ok "Rust $(rustc --version)"
}

# Visual Studio C++ build tools (required by Tauri to compile the Rust crate).
# A reliable signal is the presence of the MSVC linker via vswhere.
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$hasMsvc = $false
if (Test-Path $vswhere) {
  $vc = & $vswhere -latest -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath
  if ($vc) { $hasMsvc = $true }
}
if ($hasMsvc) {
  Write-Ok "Visual Studio C++ build tools"
} elseif ($SkipToolchains) {
  Write-Warn "MSVC C++ build tools not detected - Tauri builds may fail. See https://v2.tauri.app/start/prerequisites/"
} else {
  Install-WingetPackage 'Microsoft.VisualStudio.2022.BuildTools' 'VS 2022 C++ Build Tools' `
    '--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
  Write-Ok "Visual Studio C++ build tools"
}

# WebView2 runtime - present by default on Windows 11; install if missing.
$wv2Key = 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
if ((Test-Path $wv2Key) -or (Test-Path ($wv2Key -replace 'WOW6432Node\\',''))) {
  Write-Ok "WebView2 runtime"
} elseif (-not $SkipToolchains) {
  Install-WingetPackage 'Microsoft.EdgeWebView2Runtime' 'WebView2 runtime'
} else {
  Write-Warn "WebView2 runtime not detected - install it from https://developer.microsoft.com/microsoft-edge/webview2/"
}

# pnpm via corepack (version resolved from packageManager in package.json).
Write-Step "Enabling pnpm (corepack)"
$env:COREPACK_ENABLE_DOWNLOAD_PROMPT = '0'
if (-not (Test-Cmd 'pnpm')) {
  if (Test-Cmd 'corepack') { & corepack enable pnpm 2>$null }
  Sync-Path
  if (-not (Test-Cmd 'pnpm')) {
    Write-Info "corepack unavailable; installing pnpm via npm ..."
    & npm install -g pnpm
    Sync-Path
  }
}
if (-not (Test-Cmd 'pnpm')) {
  Fail "pnpm not available after corepack. Run 'corepack enable pnpm' in a new terminal, then re-run."
}
Push-Location $RepoRoot
try { Write-Ok "pnpm $(pnpm --version)" } finally { Pop-Location }

# --- 3. FFmpeg + ffprobe sidecars ------------------------------------------

Write-Step "Setting up FFmpeg sidecar binaries"

$ffmpegDst  = Join-Path $BinDir "ffmpeg-$triple.exe"
$ffprobeDst = Join-Path $BinDir "ffprobe-$triple.exe"

if ((Test-Path $ffmpegDst) -and (Test-Path $ffprobeDst)) {
  Write-Ok "Sidecars already present in src-tauri/binaries/"
} else {
  New-Item -ItemType Directory -Force -Path $BinDir | Out-Null
  $url = "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-$ffArch-gpl.zip"
  $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("ffmpeg-" + [guid]::NewGuid())
  New-Item -ItemType Directory -Force -Path $tmp | Out-Null
  $zip = Join-Path $tmp 'ffmpeg.zip'
  try {
    Write-Info "Downloading $url"
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    Write-Info "Extracting ..."
    Expand-Archive -Path $zip -DestinationPath $tmp -Force

    $ffmpegSrc  = Get-ChildItem -Path $tmp -Recurse -Filter 'ffmpeg.exe'  | Select-Object -First 1
    $ffprobeSrc = Get-ChildItem -Path $tmp -Recurse -Filter 'ffprobe.exe' | Select-Object -First 1
    if (-not $ffmpegSrc -or -not $ffprobeSrc) {
      Fail "Could not find ffmpeg.exe / ffprobe.exe inside the downloaded archive."
    }
    Copy-Item $ffmpegSrc.FullName  $ffmpegDst  -Force
    Copy-Item $ffprobeSrc.FullName $ffprobeDst -Force
    Write-Ok "ffmpeg-$triple.exe"
    Write-Ok "ffprobe-$triple.exe"
  } finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
  }
}

# --- 4. install workspace dependencies -------------------------------------

Write-Step "Installing workspace dependencies (pnpm install)"
Push-Location $RepoRoot
try {
  & pnpm install
  if ($LASTEXITCODE -ne 0) { Fail "pnpm install failed." }
  Write-Ok "Dependencies installed"
} finally {
  Pop-Location
}

# --- 5. debug build ---------------------------------------------------------

if ($SkipBuild) {
  Write-Step "Skipping build (-SkipBuild)"
} else {
  Write-Step "Building the desktop app (debug - this takes a few minutes)"
  Push-Location $RepoRoot
  try {
    & pnpm --filter recast-desktop run build:debug
    if ($LASTEXITCODE -ne 0) { Fail "Desktop debug build failed." }
    Write-Ok "Debug build complete"
  } finally {
    Pop-Location
  }
}

# --- done -------------------------------------------------------------------

Write-Host "`nSetup complete." -ForegroundColor Green
Write-Host "Start the desktop app in dev mode with:" -ForegroundColor White
Write-Host "    pnpm --filter recast-desktop dev`n" -ForegroundColor Cyan
